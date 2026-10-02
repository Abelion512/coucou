// Hermes adapter — the gateway control socket.
//
// Verified live against Hermes v0.21.5: `~/.hermes/gateway.sock` speaks one
// JSON line per connection — send `{"verb":"status"}` **plus a trailing newline**,
// read one line back, the server closes. The newline is the frame delimiter:
// without it the gateway waits for a complete line and answers nothing at all
// (silently, after ~2 s), so a client that forgets it looks exactly like a dead
// gateway. `identify` and `status` are the verified verbs; `status`
// carries `gateway_state`, `active_agents`, `code_version` and
// `platforms.<name>.state`.
//
// Spec rules: poll every 5 s with a 2 s timeout; a missing socket means
// `unavailable`; `active_agents = 0` while running means Idle (never Working);
// an api_server platform that is not `connected` surfaces as Error. Hermes has
// its own approval flow (`tools/approval.py`) — Coucou only watches, it never
// sends decisions, so this adapter has no write path at all.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use std::time::Duration;

use tokio::sync::mpsc::Sender;

use super::{emit, Agent, AgentEvent, AgentState};
use crate::log;

const POLL: Duration = Duration::from_secs(5);
const TIMEOUT: Duration = Duration::from_secs(2);

/// Whether the gateway actually answered on the last poll. The socket file alone
/// is not liveness: a crashed gateway leaves it behind, and `Path::exists` would
/// report a dead gateway as healthy.
static RUNNING: AtomicBool = AtomicBool::new(false);

/// The API server address the gateway reports, kept because it is the only place
/// it is ever known. The status payload carries `platforms.api_server.listener_base`
/// every 5 s and it used to be dropped on the floor, leaving "where is Hermes"
/// answerable only from a comment in a spec.
static API_BASE: Mutex<Option<String>> = Mutex::new(None);

/// Where Hermes serves HTTP, if the gateway said so.
pub fn api_base() -> Option<String> {
    API_BASE.lock().ok().and_then(|v| v.clone())
}

fn socket_path() -> String {
    let home = std::env::var_os("HOME")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from("."));
    home.join(".hermes").join("gateway.sock").to_string_lossy().into_owned()
}

pub fn start(app: tauri::AppHandle, tx: Sender<AgentEvent>) {
    tauri::async_runtime::spawn(async move {
        let mut interval = tokio::time::interval(POLL);
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        let mut was_up = false;

        loop {
            interval.tick().await;
            match query_status().await {
                Some(reply) => {
                    if !was_up {
                        was_up = true;
                        log::line("hermes: gateway online");
                    }
                    RUNNING.store(true, Ordering::Relaxed);
                    report(app.clone(), &tx, reply).await;
                }
                None => {
                    if was_up {
                        was_up = false;
                        log::line("hermes: gateway offline");
                    }
                    RUNNING.store(false, Ordering::Relaxed);
                    let _ = tx
                        .send(AgentEvent::State { agent: Agent::Hermes, state: AgentState::Unavailable })
                        .await;
                }
            }
        }
    });
}

/// One JSON line in, one JSON line out. Any failure — socket absent, timeout,
/// bad JSON — is `None` and nothing else.
async fn query_status() -> Option<serde_json::Value> {
    let mut stream = tokio::time::timeout(TIMEOUT, tokio::net::UnixStream::connect(socket_path()))
        .await
        .ok()?
        .ok()?;
    let request = serde_json::json!({ "verb": "status" });
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    tokio::time::timeout(
        TIMEOUT,
        stream.write_all(format!("{}\n", request).as_bytes()),
    )
    .await
    .ok()?
    .ok()?;
    let mut buf = Vec::new();
    let mut chunk = [0u8; 1024];
    loop {
        let n = tokio::time::timeout(TIMEOUT, stream.read(&mut chunk))
            .await
            .ok()?
            .ok()?;
        if n == 0 {
            break;
        }
        buf.extend_from_slice(&chunk[..n]);
        if buf.contains(&b'\n') {
            break;
        }
        if buf.len() > 1 << 20 {
            return None;
        }
    }
    serde_json::from_slice::<serde_json::Value>(&buf).ok()
}

/// Map the verified `status` payload onto AgentEvents.
async fn report(
    app: tauri::AppHandle,
    tx: &Sender<AgentEvent>,
    reply: serde_json::Value,
) {
    let Some(result) = reply.get("result") else { return };
    let gateway_state = result.get("gateway_state").and_then(|s| s.as_str()).unwrap_or("");
    let active_agents = result
        .get("active_agents")
        .and_then(|a| a.as_u64())
        .unwrap_or(0);
    let version = result
        .get("code_version")
        .and_then(|v| v.as_str())
        .unwrap_or("?")
        .to_string();

    let state = if gateway_state != "running" {
        AgentState::Error
    } else if active_agents > 0 {
        AgentState::Working
    } else {
        AgentState::Idle
    };

    let _ = tx
        .send(AgentEvent::Step {
            agent: Agent::Hermes,
            tool: "gateway".into(),
            detail: format!("v{version} · {active_agents} active"),
        })
        .await;

    // Platform health: a non-connected api_server is the error the island shows.
    if let Some(platforms) = result.get("platforms").and_then(|p| p.as_object()) {
        // The address is the only way to reach Hermes over HTTP; keep it.
        if let Some(base) = platforms
            .get("api_server")
            .and_then(|a| a.get("listener_base"))
            .and_then(|b| b.as_str())
        {
            if let Ok(mut slot) = API_BASE.lock() {
                if slot.as_deref() != Some(base) {
                    log::line(format!("hermes: api server at {base}"));
                }
                *slot = Some(base.to_string());
            }
        }
        for (name, info) in platforms {
            let pstate = info.get("state").and_then(|s| s.as_str()).unwrap_or("");
            if pstate != "connected" {
                let _ = tx
                    .send(AgentEvent::Notification {
                        agent: Agent::Hermes,
                        kind: "platform".into(),
                        message: format!("{name}: {pstate}"),
                    })
                    .await;
                if name == "api_server" && state != AgentState::Error {
                    let _ = tx
                        .send(AgentEvent::State { agent: Agent::Hermes, state: AgentState::Error })
                        .await;
                }
            }
        }
    }

    let _ = tx.send(AgentEvent::State { agent: Agent::Hermes, state }).await;
    emit(&app, &AgentEvent::State { agent: Agent::Hermes, state });
}

/// Liveness: the gateway answered on the last poll, not merely "the file is there".
pub fn healthy() -> bool {
    RUNNING.load(Ordering::Relaxed)
}
