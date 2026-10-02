// Unix-socket relay server — the pipe.rs of Linux.
//
// `$XDG_RUNTIME_DIR/coucou/coucou.sock` (fallback `/tmp/coucou-<uid>.sock` when
// the path would not fit a 108-byte sun_path). Every hook event is forwarded to
// the island as a `hook` event. `PermissionRequest` is the only one that keeps
// its connection open: it waits for the island's decision and writes it back on
// the same connection, which is how approving from the island works.
//
// Claude Code is never blocked by us. Three things guarantee it:
//   * coucou-hook gives the connection 300 ms and exits cleanly if we are closed;
//   * we only wait for a human once the island has *confirmed* the card is on
//     screen, so a paused island or a webview that is not listening costs a few
//     hundred milliseconds, not two minutes;
//   * whatever happens we drop the connection after the decision timeout, and
//     the terminal takes over.
//
// What we write back is the bare word `allow` or `deny`. Turning that into the
// documented hookSpecificOutput JSON is coucou-hook's job, so the wire format
// Claude Code expects lives in exactly one place.

use std::collections::HashMap;
use std::os::unix::fs::PermissionsExt;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::Duration;

use serde_json::{json, Value};
use tauri::{AppHandle, Emitter, Manager};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{UnixListener, UnixStream};
use tokio::sync::mpsc;

use crate::island::WINDOW_LABEL;
use crate::log;

/// Slightly under coucou-hook's own 110 s wait, so we always answer first.
const DECISION_TIMEOUT: Duration = Duration::from_secs(108);
/// How long the island gets to say "the card is up". This is the whole point:
/// without it, an island that is paused, hidden behind a crashed webview or
/// simply not listening would leave Claude Code staring at a prompt nobody can
/// see for nearly two minutes.
const ACK_TIMEOUT: Duration = Duration::from_millis(800);
const MAX_PAYLOAD: usize = 1 << 20;
/// A client that stops writing is dropped after this long.
const READ_TIMEOUT: Duration = Duration::from_secs(5);

/// What the island can say about a permission request.
pub enum Reply {
    /// The card is on screen and a human can act on it.
    Ack,
    /// A human clicked: `allow` or `deny`.
    Decision(String),
    /// Nobody can act on it — paused, or another request already holds the card.
    Decline,
}

/// Permission requests the island has been told about.
#[derive(Default)]
pub struct Pending(pub Mutex<HashMap<String, mpsc::Sender<Reply>>>);

static COUNTER: AtomicU64 = AtomicU64::new(1);

/// `$XDG_RUNTIME_DIR/coucou/coucou.sock`, or `/tmp/coucou-<uid>.sock` when the
/// XDG path would not fit a 108-byte `sun_path` (guard: ≤ 107 bytes).
/// Must match the hook's `socket_path()` exactly.
pub fn socket_path() -> String {
    if let Some(runtime) = std::env::var_os("XDG_RUNTIME_DIR") {
        let path = std::path::Path::new(&runtime)
            .join("coucou")
            .join("coucou.sock");
        let text = path.to_string_lossy().into_owned();
        if text.len() <= 107 {
            return text;
        }
    }
    format!("/tmp/coucou-{}.sock", unsafe { libc::getuid() })
}

pub fn start(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        let path = socket_path();
        let sock = std::path::Path::new(&path);

        // A leftover socket file from a crash would make bind() fail with
        // EADDRINUSE; a live server from another instance is refused below by
        // first-listener discipline (single-instance plugin guards this too).
        let _ = std::fs::remove_file(sock);

        if let Some(dir) = sock.parent() {
            if std::fs::create_dir_all(dir).is_err() {
                log::line("cannot create the relay socket directory");
                return;
            }
            // 0700 — not world-readable, as the macOS port hardened it.
            let _ = std::fs::set_permissions(dir, std::fs::Permissions::from_mode(0o700));
        }
        let listener = match UnixListener::bind(&path) {
            Ok(l) => l,
            Err(err) => {
                log::line(format!("cannot open the relay socket: {err}"));
                return;
            }
        };
        // Only our uid may even connect — defence in depth behind SO_PEERCRED.
        let _ = std::fs::set_permissions(sock, std::fs::Permissions::from_mode(0o600));

        log::line(format!("relay socket at {path}"));
        loop {
            match listener.accept().await {
                Ok((stream, _addr)) => {
                    // Connection ceiling, as the macOS port: a runaway hook
                    // storm must not exhaust the runtime's task budget.
                    let mut live = LIVE_CONNECTIONS.lock().unwrap();
                    if *live >= MAX_CONNECTIONS {
                        drop(live);
                        continue;
                    }
                    *live += 1;
                    drop(live);
                    let app = app.clone();
                    tauri::async_runtime::spawn(async move {
                        handle(app, stream).await;
                        *LIVE_CONNECTIONS.lock().unwrap() -= 1;
                    });
                }
                Err(_) => {
                    tokio::time::sleep(Duration::from_millis(200)).await;
                }
            }
        }
    });
}

/// Concurrent relay connections, matching the macOS port's ceiling.
const MAX_CONNECTIONS: usize = 32;
static LIVE_CONNECTIONS: Mutex<usize> = Mutex::new(0);

async fn handle(app: AppHandle, stream: UnixStream) {
    // Refuse to serve another account's relay before reading a byte: trusting a
    // foreign peer could hand it the contents of every tool call.
    if !same_user(&stream) {
        log::line("relay connection from another uid — refused");
        return;
    }

    let mut buf = Vec::new();
    let mut chunk = [0u8; 4096];
    let mut stream = stream;
    // 5 s receive timeout — a client that connects and then stops writing must
    // not hold a task slot forever (mirrors the macOS port's SO_RCVTIMEO).
    loop {
        match tokio::time::timeout(READ_TIMEOUT, stream.read(&mut chunk)).await {
            Ok(Ok(0)) => break,
            Ok(Ok(n)) => {
                buf.extend_from_slice(&chunk[..n]);
                if buf.contains(&b'\n') || buf.len() > MAX_PAYLOAD {
                    break;
                }
            }
            // Timeout or IO error: drop the connection, the hook has its own
            // deadline and will have moved on.
            _ => return,
        }
    }
    let line = match buf.iter().position(|b| *b == b'\n') {
        Some(i) => &buf[..i],
        None => &buf[..],
    };
    let Ok(mut payload) = serde_json::from_slice::<Value>(line) else { return };
    if !payload.is_object() {
        return;
    }

    let event = payload
        .get("hook_event_name")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();

    // Guardrail: approval cards are Claude-Code-only. A payload tagged with a
    // third-party `coucou_agent` is observe-only, so it is forwarded like any
    // other event and never opens a pending decision — the relay writes nothing
    // back and the agent re-asks in its own terminal.
    if event != "PermissionRequest" || is_external(&payload) {
        if is_external(&payload) {
            log::line(format!("hook {event} (external agent, no approval)"));
        } else {
            log::line(format!("hook {event}"));
        }
        let _ = app.emit_to(WINDOW_LABEL, "hook", payload);
        return;
    }

    let id = format!("{}-{}", std::process::id(), COUNTER.fetch_add(1, Ordering::Relaxed));
    let (tx, mut rx) = mpsc::channel::<Reply>(4);
    {
        let pending = app.state::<Pending>();
        pending.0.lock().unwrap().insert(id.clone(), tx);
    }
    payload["request_id"] = json!(id);
    log::line(format!("hook PermissionRequest id={id}"));
    let _ = app.emit_to(WINDOW_LABEL, "hook", payload);

    let decision = wait_for_decision(&id, &mut rx).await;
    app.state::<Pending>().0.lock().unwrap().remove(&id);

    // No decision: say nothing at all. coucou-hook then writes nothing to stdout
    // and Claude Code asks in the terminal, exactly as if Coucou were closed.
    if let Some(d) = decision {
        let _ = stream.write_all(format!("{d}\n").as_bytes()).await;
        let _ = stream.flush().await;
    }
    let _ = stream.shutdown().await;
}

/// A payload from a third-party agent: `coucou_agent` set to anything but the
/// reserved `claude`. The relay forwards the tag as given; the island validates
/// the name itself and drops it into the Claude Code pill when it is malformed.
fn is_external(payload: &Value) -> bool {
    payload
        .get("coucou_agent")
        .and_then(Value::as_str)
        .is_some_and(|agent| !agent.is_empty() && agent != "claude")
}

/// True when the process on the other end runs as the same user we do
/// (SO_PEERCRED). A failure to answer is treated as "not ours": refusing to
/// talk to a peer we cannot vouch for costs one hook event.
fn same_user(stream: &UnixStream) -> bool {
    use std::os::fd::AsRawFd;
    #[repr(C)]
    struct Ucred {
        pid: libc::pid_t,
        uid: libc::uid_t,
        gid: libc::gid_t,
    }
    let mut cred = Ucred { pid: 0, uid: u32::MAX, gid: 0 };
    let mut len = std::mem::size_of::<Ucred>() as libc::socklen_t;
    let ok = unsafe {
        libc::getsockopt(
            stream.as_raw_fd(),
            libc::SOL_SOCKET,
            libc::SO_PEERCRED,
            std::ptr::from_mut(&mut cred).cast(),
            &mut len as *mut libc::socklen_t,
        ) == 0
    };
    ok && cred.uid == unsafe { libc::getuid() }
}

/// Two waits: a short one for "the card is up", then the long one for a human.
async fn wait_for_decision(id: &str, rx: &mut mpsc::Receiver<Reply>) -> Option<String> {
    match tokio::time::timeout(ACK_TIMEOUT, rx.recv()).await {
        Ok(Some(Reply::Ack)) => {}
        // A click that beats the ack is still a click.
        Ok(Some(Reply::Decision(d))) => {
            log::line(format!("hook id={id} answered {d}"));
            return Some(d);
        }
        Ok(Some(Reply::Decline)) => {
            log::line(format!("hook id={id} not shown — terminal takes over"));
            return None;
        }
        Ok(None) => return None,
        Err(_) => {
            log::line(format!("hook id={id} island never acknowledged — terminal takes over"));
            return None;
        }
    }

    match tokio::time::timeout(DECISION_TIMEOUT, rx.recv()).await {
        Ok(Some(Reply::Decision(d))) => {
            log::line(format!("hook id={id} answered {d}"));
            Some(d)
        }
        Ok(Some(Reply::Decline)) => {
            log::line(format!("hook id={id} released without a decision"));
            None
        }
        _ => {
            log::line(format!("hook id={id} timed out — terminal takes over"));
            None
        }
    }
}

fn send(app: &AppHandle, request_id: &str, reply: Reply, keep: bool) {
    let sender = {
        let pending = app.state::<Pending>();
        let mut map = pending.0.lock().unwrap();
        if keep { map.get(request_id).cloned() } else { map.remove(request_id) }
    };
    match sender {
        Some(tx) => {
            let _ = tx.try_send(reply);
        }
        None => log::line(format!("reply for id={request_id} — no pending request")),
    }
}

/// The island has the card on screen; the long wait may begin.
pub fn acknowledge(app: &AppHandle, request_id: &str) {
    send(app, request_id, Reply::Ack, true);
}

/// Nobody can act on this one — paused, or another card already holds the view.
pub fn decline(app: &AppHandle, request_id: &str) {
    log::line(format!("decline id={request_id}"));
    send(app, request_id, Reply::Decline, false);
}

/// Called by the island's Allow / Deny buttons. Only ever a bare word: turning
/// it into Claude Code's JSON is coucou-hook's job.
pub fn answer(app: &AppHandle, request_id: &str, decision: &str) {
    let word = match decision {
        "allow" | "always" => "allow",
        _ => "deny",
    };
    log::line(format!("decision id={request_id} {word}"));
    send(app, request_id, Reply::Decision(word.to_string()), false);
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn only_claude_code_payloads_may_carry_an_approval() {
        assert!(!is_external(&json!({})));
        assert!(!is_external(&json!({ "coucou_agent": "" })));
        assert!(!is_external(&json!({ "coucou_agent": "claude" })));
        assert!(is_external(&json!({ "coucou_agent": "my-tool" })));
    }
}
