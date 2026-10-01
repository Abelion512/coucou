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

impl Agent {
    /// Front-end identifier for colours and labels.
    #[allow(dead_code)]
    pub fn id(&self) -> &'static str {
        match self {
            Agent::ClaudeCode => "claude-code",
            Agent::Opencode => "opencode",
            Agent::Hermes => "hermes",
            Agent::Freebuff => "freebuff",
        }
    }
}

/// Runtime state of one agent as the island should show it.
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
#[allow(dead_code)]
pub trait AgentSource {
    /// Where this adapter's events are sent.
    fn sender(&self) -> Sender<AgentEvent>;
    /// Quick liveness probe used by the health line in the overview.
    fn healthy(&self) -> bool;
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
    use tauri::Emitter;
    let _ = app.emit("agent", event);
}
