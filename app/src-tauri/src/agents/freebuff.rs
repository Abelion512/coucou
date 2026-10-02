// Freebuff / Codebuff adapter — the hard one: no server, no hooks, no API.
//
// Verified live: both CLIs drop runtime state into `~/.config/manicode/`:
//   * `freebuff-live-<pid>.json` — { instanceId, model, ownerPid, tokenKey,
//     expiresAt } — appears when a session starts, disappears when it ends;
//   * `freebuff-relaunch-<pid>.json` — present while a relaunch/update runs;
//   * `message-history.json` — mtime moves as the conversation grows.
//
// So this adapter is pure filesystem polling. Hard rule from the spec: `tokenKey`
// is a secret. It is parsed with the rest of the file and dropped on the floor —
// never stored, never logged, never emitted.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use tokio::sync::mpsc::Sender;

use super::{Agent, AgentEvent, AgentState};
use crate::log;

/// Poll cadence. Five seconds is coarse but the state it produces is coarse too,
/// and an agent that is merely running does not need to be tracked to the
/// millisecond.
const TICK: Duration = Duration::from_secs(5);

/// Whether a live session or a relaunch was seen on the last tick. The poll
/// already knows this; re-deriving it from `healthy()` meant a second directory
/// scan and a file read per call, on the main thread.
static LIVE: AtomicBool = AtomicBool::new(false);

/// One live session as the island sees it.
#[derive(Clone)]
struct LiveSession {
    model: String,
    expires_at: u64,
}

fn config_dir() -> PathBuf {
    std::env::var_os("HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".config")
        .join("manicode")
}

pub fn start(_app: tauri::AppHandle, tx: Sender<AgentEvent>) {
    tauri::async_runtime::spawn(async move {
        // Sessions we have already announced, keyed by file name.
        let mut seen: HashMap<String, LiveSession> = HashMap::new();
        let mut last_history_mtime: Option<std::time::SystemTime> = None;
        let mut interval = tokio::time::interval(TICK);
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);

        loop {
            interval.tick().await;
            let dir = config_dir();

            // ── live sessions ────────────────────────────────────────────────
            if let Ok(entries) = std::fs::read_dir(&dir) {
                let mut live: HashMap<String, LiveSession> = HashMap::new();
                for entry in entries.flatten() {
                    let name = entry.file_name().to_string_lossy().into_owned();
                    if !(name.starts_with("freebuff-live-") && name.ends_with(".json")) {
                        continue;
                    }
                    if let Ok(bytes) = std::fs::read(entry.path()) {
                        if let Ok(v) = serde_json::from_slice::<serde_json::Value>(&bytes) {
                            // tokenKey is parsed with the rest and then dropped
                            // on the floor — it never reaches a field we keep.
                            let model = v
                                .get("model")
                                .and_then(|m| m.as_str())
                                .unwrap_or("freebuff")
                                .to_string();
                            let expires_at = v
                                .get("expiresAt")
                                .and_then(|e| e.as_u64())
                                .unwrap_or(0);
                            live.insert(name, LiveSession { model, expires_at });
                        }
                    }
                }

                // New sessions → SessionStart; expired/removed ones → Finish.
                for (name, session) in &live {
                    if !seen.contains_key(name) {
                        let pid = name
                            .trim_start_matches("freebuff-live-")
                            .trim_end_matches(".json");
                        let _ = tx
                            .send(AgentEvent::SessionStart {
                                agent: Agent::Freebuff,
                                session_id: pid.to_string(),
                                project: session.model.clone(),
                                cwd: String::new(),
                            })
                            .await;
                        log::line(format!("freebuff: session {pid} ({}) started", session.model));
                    }
                }
                for name in seen.keys() {
                    if !live.contains_key(name) {
                        let pid = name
                            .trim_start_matches("freebuff-live-")
                            .trim_end_matches(".json");
                        let _ = tx
                            .send(AgentEvent::Finish {
                                agent: Agent::Freebuff,
                                session_id: pid.to_string(),
                                message: String::new(),
                            })
                            .await;
                        log::line(format!("freebuff: session {pid} finished"));
                    }
                }
                seen = live;
            }

            // Expiry: a file still present but past expiresAt counts as done.
            let now_ms = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_millis() as u64)
                .unwrap_or(0);
            let any_live = seen.values().any(|s| s.expires_at == 0 || s.expires_at > now_ms);

            // ── relaunch / update state ──────────────────────────────────────
            let relaunching = std::fs::read_dir(&dir)
                .map(|entries| {
                    entries.flatten().any(|e| {
                        let n = e.file_name().to_string_lossy().into_owned();
                        n.starts_with("freebuff-relaunch-") && n.ends_with(".json")
                    })
                })
                .unwrap_or(false);
            if relaunching {
                let _ = tx
                    .send(AgentEvent::State { agent: Agent::Freebuff, state: AgentState::Working })
                    .await;
            } else {
                let state = if any_live { AgentState::Working } else { AgentState::Idle };
                let _ = tx.send(AgentEvent::State { agent: Agent::Freebuff, state }).await;
            }

            // ── conversation growth ──────────────────────────────────────────
            let history = dir.join("message-history.json");
            let mtime = std::fs::metadata(&history).and_then(|m| m.modified()).ok();
            if let (Some(now), Some(last)) = (mtime, last_history_mtime) {
                if now != last {
                    if let Ok(size) = std::fs::metadata(&history).map(|m| m.len()) {
                        let _ = tx
                            .send(AgentEvent::Step {
                                agent: Agent::Freebuff,
                                tool: "conversation".into(),
                                detail: format!("history updated ({size} bytes)"),
                            })
                            .await;
                    }
                }
            }
            last_history_mtime = mtime;

            LIVE.store(any_live || relaunching, Ordering::Relaxed);
        }
    });
}

/// Liveness: a live session or a relaunch was seen on the last poll.
#[allow(dead_code)]
pub fn healthy() -> bool {
    LIVE.load(Ordering::Relaxed)
}
