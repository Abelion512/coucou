// The agent bus: one event vocabulary for every coding agent Coucou watches.
//
// Claude-Code keeps its privileged path — its hook relay is the only source that
// can carry a PermissionReq, because it is the only agent whose CLI can be
// answered from the island. The three new adapters (OpenCode, Hermes,
// Freebuff/Codebuff) are observe-only: they never gate a session, never send a
// decision, and simply report what they see.
//
// Every adapter is loopback-only and degrades to `unavailable` — an agent that
// is not running must never crash the island, never spam the log, and never
// block anything.

pub mod freebuff;
pub mod hermes;
pub mod opencode;

use serde::Serialize;
use std::sync::Mutex;
use tokio::sync::mpsc::{Receiver, Sender};

/// Which agent a task or event belongs to. Serialized camelCase for the front
/// end, which keys colours and labels off it. The full vocabulary is the
/// contract: ClaudeCode events are produced by the hook relay (the Unix socket)
/// and joined into this bus in the front-end state machine, so some variants
/// stay reserved for it.
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Agent {
    ClaudeCode,
    Opencode,
    Hermes,
    Freebuff,
}

/// Runtime state of one agent as the island should show it.
/// `Blocked` and `Done` are part of the vocabulary the front end maps, but no
/// adapter emits them yet; the allow keeps that honest without inventing a caller.
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum AgentState {
    Idle,
    Working,
    Blocked,
    Done,
    Error,
    Unavailable,
}

/// One normalized event from any agent. PermissionReq is reserved for
/// Claude-Code — the only agent whose permissions the island may answer.
#[allow(dead_code)]
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase", tag = "type")]
pub enum AgentEvent {
    /// A session appeared. `project` is the label the island shows.
    SessionStart { agent: Agent, session_id: String, project: String, cwd: String },
    /// A step in the task: "Read · foo.ts", "5 messages", "gateway running".
    Step { agent: Agent, tool: String, detail: String },
    /// Coarse state change.
    State { agent: Agent, state: AgentState },
    /// Rate limits, questions, platform health — nothing that waits on a human.
    Notification { agent: Agent, kind: String, message: String },
    /// The session is over.
    Finish { agent: Agent, session_id: String, message: String },
    /// ONLY Claude Code produces this: a permission the island may answer.
    PermissionReq { agent: Agent, session_id: String, tool: String, command: String, request_id: String },
}

/// Everything an adapter has in common. Each adapter is one tokio task; the
/// bus owns nothing but routing.
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentStatus {
    /// Serde id: `opencode`, `hermes`, `freebuff`. The front end keys off this.
    pub agent: Agent,
    /// Whether the agent answered on its last probe. "Reachable", not "useful":
    /// an OpenCode server with no session is reachable and doing nothing.
    pub connected: bool,
    /// Whether there is actually a live session right now — the difference the
    /// island pill already draws but this screen could not.
    pub working: bool,
    /// How to reach it, when there is a way to say.
    pub endpoint: Option<String>,
    /// What to show when it is not connected, or None when there is nothing to add.
    pub detail: Option<String>,
}

/// Reads all three adapters' liveness. Cheap: every adapter already keeps the
/// answer in an atomic from its poll, so this is three relaxed loads and no I/O.
pub fn status() -> Vec<AgentStatus> {
    vec![
        AgentStatus {
            agent: Agent::Opencode,
            connected: opencode::healthy(),
            working: opencode::busy(),
            endpoint: Some(opencode::endpoint()),
            detail: None,
        },
        AgentStatus {
            agent: Agent::Hermes,
            connected: hermes::healthy(),
            working: hermes::busy(),
            endpoint: hermes::api_base(),
            detail: None,
        },
        AgentStatus {
            agent: Agent::Freebuff,
            connected: freebuff::healthy(),
            working: freebuff::busy(),
            endpoint: None,
            detail: Some("~/.config/manicode".into()),
        },
    ]
}

/// A shared event channel: every adapter pushes here, the island front end
/// receives `agent` events per agent id.
pub struct AgentBus {
    tx: Sender<AgentEvent>,
}

impl AgentBus {
    pub fn new() -> (Self, Receiver<AgentEvent>) {
        let (tx, rx) = tokio::sync::mpsc::channel(256);
        (Self { tx }, rx)
    }

    pub fn sender(&self) -> Sender<AgentEvent> {
        self.tx.clone()
    }
}

impl Default for AgentBus {
    fn default() -> Self {
        // Dropped immediately; the real one comes from new().
        let (bus, _) = Self::new();
        bus
    }
}

/// Spawns the observe-only adapters. Claude-Code is not here — its events come
/// from the hook socket, already in the island's own vocabulary.
pub fn start(app: tauri::AppHandle, bus: &AgentBus) {
    opencode::start(app.clone(), bus.sender());
    hermes::start(app.clone(), bus.sender());
    freebuff::start(app, bus.sender());
}

/// Forwards an adapter event to the island as an `agent` Tauri event.
pub fn emit(app: &tauri::AppHandle, event: &AgentEvent) {
    remember(event);
    use tauri::Emitter;
    let _ = app.emit("agent", event);
}

/// The most recent `SessionStart` per agent, so a front end that subscribes
/// *after* an adapter found its session still draws the pill.
///
/// The window exists before the webview has finished loading, and an adapter
/// that answers in that gap announces itself to nobody: OpenCode's server is a
/// loopback HTTP call away and connects in about a second, so on a machine
/// where `opencode serve` was already running its `SessionStart` was the very
/// first event of the app's life. The adapter's own `LAST_SESSION` guard then
/// stops it repeating itself, and the agent stays invisible for the rest of the
/// session — which is what "why can't I see OpenCode in the island" was.
static LAST_SESSION_START: Mutex<Vec<(Agent, AgentEvent)>> = Mutex::new(Vec::new());

fn remember(event: &AgentEvent) {
    let Ok(mut remembered) = LAST_SESSION_START.lock() else { return };
    match event {
        AgentEvent::SessionStart { agent, .. } => {
            remembered.retain(|(a, _)| a != agent);
            remembered.push((*agent, event.clone()));
        }
        // An agent that stopped must not come back as a pill on the next replay.
        AgentEvent::State { agent, state: AgentState::Unavailable } => {
            remembered.retain(|(a, _)| a != agent);
        }
        _ => {}
    }
}

/// Re-sends every session the adapters have already found. The island calls
/// this once its handlers are registered, which turns "whatever happened before
/// I existed" from a permanent hole into a snapshot.
pub fn replay(app: &tauri::AppHandle) {
    let snapshot = LAST_SESSION_START
        .lock()
        .map(|remembered| remembered.iter().map(|(_, e)| e.clone()).collect::<Vec<_>>())
        .unwrap_or_default();
    for event in &snapshot {
        use tauri::Emitter;
        let _ = app.emit("agent", event);
    }
}
