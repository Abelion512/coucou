// OpenCode adapter — the cleanest of the three: a plain HTTP/SSE server on the
// loopback.
//
// Verified live against OpenCode 1.18.34 and documented at
// https://opencode.ai/docs/server/ :
//   * `GET /global/health` → { healthy: true, version }
//   * `GET /event` → SSE stream; first event is `server.connected`, then bus
//     events; heartbeats keep the stream alive;
//   * `GET /session` → Session[].
//
// Event vocabulary cross-checked against upstream's own OpenCode integration
// (Louis-CFM/coucou PR #47, `permission.asked`, `tool.execute.before/after`,
// `session.created/idle/error`, `message.updated`). Properties ride one level
// down, inside `properties`.
//
// House rules from the spec: an absent server means `unavailable`, never a
// crash and never a block; reconnect with 1s→2s→4s→…→30s backoff; the port is
// never hardcoded — it is discovered from env (`OPENCODE_PORT`, what the TUI
// and `opencode serve` print) with 54321 as the audited fallback; idle is
// 30 s without events. Observe-only: nothing is ever sent to OpenCode. Its
// permission flow (`permission.asked`) is upstream's plugin approach; this
// adapter deliberately has no write path at all.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::OnceLock;
use std::time::Duration;

use tokio::sync::mpsc::Sender;

use super::{emit, Agent, AgentEvent, AgentState};
use crate::log;

/// The audited default; only used when discovery finds nothing.
const FALLBACK_PORT: u16 = 54321;
/// No events for this long and the session counts as idle.
const IDLE_AFTER: Duration = Duration::from_secs(30);
const POLL_SESSIONS: Duration = Duration::from_secs(10);

static RUNNING: AtomicBool = AtomicBool::new(false);
/// Slot for a discovered server URL. Nothing writes it yet — discovery reads
/// `$OPENCODE_PORT` — so it always falls through to the audited default.
static BASE_URL: OnceLock<String> = OnceLock::new();

/// True while the SSE loop is connected — the "healthy" of this adapter.
pub fn healthy() -> bool {
    RUNNING.load(Ordering::Relaxed)
}

fn client() -> reqwest::Client {
    reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .unwrap_or_default()
}

/// Where the server is: $OPENCODE_PORT, else the audited 54321. A server that
/// is not there is simply unavailable — this must never start one itself.
fn base_url() -> String {
    BASE_URL
        .get()
        .cloned()
        .unwrap_or_else(|| format!("http://127.0.0.1:{}", discovery_port()))
}

/// The loopback address this adapter talks to, for the settings health line.
pub fn endpoint() -> String {
    base_url()
}

fn discovery_port() -> u16 {
    std::env::var("OPENCODE_PORT")
        .ok()
        .and_then(|p| p.parse().ok())
        .unwrap_or(FALLBACK_PORT)
}

pub fn start(app: tauri::AppHandle, tx: Sender<AgentEvent>) {
    tauri::async_runtime::spawn(async move {
        let http = client();
        let mut backoff = Duration::from_secs(1);
        let mut last_activity = tokio::time::Instant::now();
        let mut session_poll = tokio::time::interval(POLL_SESSIONS);

        loop {
            // Probe. Unavailable → mark, wait one backoff step, probe again.
            let health = http.get(format!("{}/global/health", base_url())).send().await;
            match health {
                Ok(resp) if resp.status().is_success() => {
                    RUNNING.store(true, Ordering::Relaxed);
                    backoff = Duration::from_secs(1);
                    log::line("opencode: connected");
                    stream_events(&app, &http, &tx, &mut last_activity, &mut session_poll).await;
                    RUNNING.store(false, Ordering::Relaxed);
                    log::line("opencode: stream ended — reconnecting");
                }
                _ => {
                    RUNNING.store(false, Ordering::Relaxed);
                    let _ = tx
                        .send(AgentEvent::State { agent: Agent::Opencode, state: AgentState::Unavailable })
                        .await;
                }
            }
            tokio::time::sleep(backoff).await;
            backoff = (backoff * 2).min(Duration::from_secs(30));
        }
    });
}

/// One SSE connection: parse `data:` lines as JSON envelopes `{type: ...}`,
/// map them into AgentEvents, and poll `/session` for titles while alive.
/// Returns when the stream ends; the caller reconnects with backoff.
async fn stream_events(
    app: &tauri::AppHandle,
    http: &reqwest::Client,
    tx: &Sender<AgentEvent>,
    last_activity: &mut tokio::time::Instant,
    session_poll: &mut tokio::time::Interval,
) {
    let request = match http
        .get(format!("{}/event", base_url()))
        .timeout(Duration::from_secs(3600))
        .send()
        .await
    {
        Ok(r) if r.status().is_success() => r,
        _ => return,
    };
    // The stream is not Unpin, so pin it in place before `next()`-ing it.
    let mut body = std::pin::pin!(request.bytes_stream());
    let mut buffer = Vec::new();

    loop {
        tokio::select! {
            read = futures_util::StreamExt::next(&mut body) => {
                let Some(Ok(bytes)) = read else { return };
                buffer.extend_from_slice(&bytes);
                while let Some(pos) = buffer.iter().position(|b| *b == b'\n') {
                    let line: Vec<u8> = buffer.drain(..=pos).collect();
                    if let Ok(text) = std::str::from_utf8(&line[..line.len() - 1]) {
                        if let Some(payload) = text.strip_prefix("data:") {
                            if let Ok(v) = serde_json::from_str::<serde_json::Value>(payload.trim()) {
                                handle_event(app, tx, v, last_activity).await;
                            }
                        }
                    }
                }
            }
            _ = session_poll.tick() => {
                poll_sessions(tx).await;
            }
            _ = tokio::time::sleep_until(*last_activity + IDLE_AFTER) => {
                let _ = tx
                    .send(AgentEvent::State { agent: Agent::Opencode, state: AgentState::Idle })
                    .await;
                *last_activity = tokio::time::Instant::now();
            }
        }
    }
}

/// SSE envelope → AgentEvent, on the vocabulary verified in upstream PR #47.
/// `server.connected` / `server.heartbeat` are transport; the bus events that
/// matter are `session.*`, `tool.execute.*` and `message.updated`. Anything
/// unknown still counts as activity (it proves the server is alive) but never
/// becomes UI content.
async fn handle_event(
    app: &tauri::AppHandle,
    tx: &Sender<AgentEvent>,
    v: serde_json::Value,
    last_activity: &mut tokio::time::Instant,
) {
    *last_activity = tokio::time::Instant::now();
    let kind = v.get("type").and_then(|t| t.as_str()).unwrap_or_default().to_string();
    let body = v.get("properties").cloned().unwrap_or(v.clone());

    match kind.as_str() {
        "server.connected" => {
            let _ = tx
                .send(AgentEvent::SessionStart {
                    agent: Agent::Opencode,
                    session_id: "server".into(),
                    project: "OpenCode server".into(),
                    cwd: String::new(),
                })
                .await;
        }
        "server.heartbeat" => {}

        "session.created" => {
            let _ = tx
                .send(AgentEvent::SessionStart {
                    agent: Agent::Opencode,
                    session_id: body.get("id").and_then(|i| i.as_str()).unwrap_or_default().into(),
                    project: body
                        .get("title")
                        .and_then(|t| t.as_str())
                        .unwrap_or("OpenCode session")
                        .into(),
                    cwd: body.get("directory").and_then(|d| d.as_str()).unwrap_or_default().into(),
                })
                .await;
        }
        "session.idle" => {
            let _ = tx
                .send(AgentEvent::State { agent: Agent::Opencode, state: AgentState::Idle })
                .await;
        }
        "session.error" => {
            let _ = tx
                .send(AgentEvent::State { agent: Agent::Opencode, state: AgentState::Error })
                .await;
        }
        "session.deleted" => {
            let _ = tx
                .send(AgentEvent::Finish {
                    agent: Agent::Opencode,
                    session_id: body.get("id").and_then(|i| i.as_str()).unwrap_or_default().into(),
                    message: String::new(),
                })
                .await;
        }

        "tool.execute.before" | "tool.execute.after" => {
            let tool = body.get("tool").and_then(|t| t.as_str()).unwrap_or("tool");
            let after = kind.ends_with("after");
            let detail = body
                .get("state")
                .and_then(|s| s.as_str())
                .map(|s| if after { s.to_string() } else { String::new() })
                .unwrap_or_default();
            let _ = tx
                .send(AgentEvent::Step {
                    agent: Agent::Opencode,
                    tool: tool.into(),
                    detail,
                })
                .await;
            if !after {
                let _ = tx
                    .send(AgentEvent::State { agent: Agent::Opencode, state: AgentState::Working })
                    .await;
            }
        }

        "message.updated" => {
            // Turns can end here without a session.idle; keep the pill alive.
            let _ = tx
                .send(AgentEvent::State { agent: Agent::Opencode, state: AgentState::Working })
                .await;
        }

        // `permission.asked` / `permission.replied`: upstream's plugin answers
        // these; we only watch. Counted as activity, shown as a notification,
        // and never answered from here.
        "permission.asked" => {
            let tool = body
                .get("tool")
                .or_else(|| body.get("toolName"))
                .and_then(|t| t.as_str())
                .unwrap_or("Tool");
            let _ = tx
                .send(AgentEvent::Notification {
                    agent: Agent::Opencode,
                    kind: "permission".into(),
                    message: format!("{tool} asks for permission (answer in the OpenCode TUI)"),
                })
                .await;
        }
        "permission.replied" => {}

        _ => {
            log::line(format!("opencode event {kind}"));
        }
    }

    emit(app, &AgentEvent::State { agent: Agent::Opencode, state: AgentState::Working });
}

/// Titles for the pills: `/session` is cheap and documented. Fetched on the
/// session-poll tick, not per event.
///
/// The island shows one pill per agent, not one per session, so this reports the
/// most recent session only — emitting every session would refill the pill's
/// step ticker from the top on every 10 s tick.
async fn poll_sessions(tx: &Sender<AgentEvent>) {
    let Ok(resp) = http_get("/session").await else { return };
    if let Ok(sessions) = serde_json::from_slice::<serde_json::Value>(&resp) {
        let Some(latest) = sessions.as_array().and_then(|list| list.first()) else { return };
        let id = latest.get("id").and_then(|i| i.as_str()).unwrap_or_default();
        let title = latest
            .get("title")
            .and_then(|t| t.as_str())
            .unwrap_or("OpenCode session");
        let _ = tx
            .send(AgentEvent::Step {
                agent: Agent::Opencode,
                tool: "session".into(),
                detail: format!("{title} ({id})"),
            })
            .await;
    }
}

/// Small helper so the poll paths share one client and base URL.
async fn http_get(path: &str) -> Result<Vec<u8>, ()> {
    let resp = client()
        .get(format!("{}{path}", base_url()))
        .send()
        .await
        .map_err(|_| ())?;
    if !resp.status().is_success() {
        return Err(());
    }
    resp.bytes().await.map(|b| b.to_vec()).map_err(|_| ())
}
