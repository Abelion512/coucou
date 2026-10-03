// AgentBus → State: the other end of the one `agent` event the Rust adapters emit
// (src-tauri/src/agents/mod.rs). Without it three live adapters run, log, and are
// thrown away.
//
// Per docs/SPEC-agent-pills.md: observe-only, one pill per agent, no front-end
// polling, no sound, no PermissionReq path.

import { Bridge, onEvent } from "../core/bridge";
import { AGENTS_META } from "../core/agents-meta";
import { State, type AgentTask } from "../core/state";
import type { BotStateName } from "../core/layout";

/** Serde `Agent` is kebab-case: claude-code | opencode | hermes | freebuff. */
type AgentId = "opencode" | "hermes" | "freebuff" | "claude-code";

/** The three this fork watches. Claude Code keeps its own hook path. */
const AGENTS: Record<AgentId, boolean> = {
  opencode: true,
  hermes: true,
  freebuff: true,
  // Claude Code events arrive on the `hook` socket, not here.
  "claude-code": false,
};

/** Serde `AgentState` → the bot states the island already knows how to draw. */
const STATE_MAP: Record<string, BotStateName> = {
  idle: "idle",
  working: "working",
  blocked: "approval",
  done: "finished",
  error: "error",
};

/** Serde tags AgentEvent with `type` in camelCase. */
interface AgentEventPayload {
  type: string;
  agent: AgentId;
  sessionId?: string;
  project?: string;
  cwd?: string;
  tool?: string;
  detail?: string;
  state?: string;
  kind?: string;
  message?: string;
}

const pillId = (agent: AgentId) => `agent_${agent}`;

/**
 * Adapters re-send the same state on every poll (Hermes every 5 s), so reacting
 * to each one would redraw 0.2×/s per agent for nothing. React only on change.
 */
const lastState = new Map<AgentId, string>();

function ensurePill(agent: AgentId): AgentTask | null {
  const meta = AGENTS_META[agent];
  if (!meta) return null;
  State.upsertExternalAgent(pillId(agent), meta.name, meta.color);
  return State.tasks.find((t) => t.id === pillId(agent)) ?? null;
}

/** An agent that is not running has no pill — no placeholder, no empty row. */
function dropIfUnavailable(agent: AgentId) {
  lastState.set(agent, "unavailable");
  State.removeTask(pillId(agent));
}

function applyState(agent: AgentId, raw: string) {
  const meta = AGENTS[agent];
  if (!meta) return;
  if (lastState.get(agent) === raw) return;
  lastState.set(agent, raw);
  if (raw === "unavailable") {
    dropIfUnavailable(agent);
    return;
  }
  const task = ensurePill(agent);
  if (!task) return;
  const next = STATE_MAP[raw];
  if (next) State.updateTask(task.id, next);
}

export function registerAgentHandlers() {
  void onEvent<AgentEventPayload>("agent", (event) => {
    const agent = event.agent;
    if (!agent || !AGENTS[agent]) return;
    const id = pillId(agent);

    switch (event.type) {
      case "sessionStart": {
        const task = ensurePill(agent);
        if (!task) return;
        // The pill keeps the agent's name: what the adapter reports as the project
        // is a session slug ("hermes-aux"), and it rides the ticker instead.
        if (event.cwd) task.sessionCwd = event.cwd;
        const label = event.project?.trim();
        if (label) task.steps = [label.slice(0, 60)];
        if (event.sessionId && event.sessionId !== "server") {
          task.steps.push(`session ${event.sessionId.slice(0, 8)}`);
        }
        task.stepIndex = 0;
        State.updateTask(id, "idle");
        State.notify();
        break;
      }

      case "step": {
        const task = ensurePill(agent);
        if (!task) return;
        const detail = event.detail?.trim();
        const label = detail ? `${event.tool ?? "step"} · ${detail}` : event.tool ?? "step";
        State.appendStep(id, label.slice(0, 60));
        State.updateTask(id, "working");
        break;
      }

      case "state":
        if (event.state) applyState(agent, event.state);
        break;

      case "notification": {
        const task = ensurePill(agent);
        if (!task || !event.message) return;
        const lower = event.message.toLowerCase();
        if (lower.includes("rate")) State.updateTask(id, "ratelimit");
        else if (event.message.endsWith("?")) {
          State.updateTask(id, "question");
          State.appendStep(id, event.message.slice(0, 60));
        } else {
          State.appendStep(id, event.message.slice(0, 60));
        }
        break;
      }

      case "finish": {
        const task = State.tasks.find((t) => t.id === id);
        if (!task) break;
        State.updateTask(id, "finished");
        if (event.message) State.appendStep(id, event.message.slice(0, 60));
        State.setPillBadge(id, "finished");
        // Spec: finished for 5 s, then the pill is gone.
        window.setTimeout(() => {
          lastState.delete(agent);
          State.removeTask(id);
        }, 5200);
        break;
      }

      // Observe-only: the adapters have no write path, and PermissionReq is
      // Claude-Code-only by guardrail.
      case "permissionReq":
        break;

      default:
        break;
    }
  })
    // Everything above this line was a race: the window exists before the webview
    // subscribes, and an adapter answering in that gap — an OpenCode server that
    // was already running connects in about a second — is heard by nobody and
    // never repeats itself.
    //
    // It has to wait for the `listen` promise above: Tauri drops an event whose
    // listener is not registered yet, so syncing in parallel with subscribing
    // reproduces the very bug this is fixing.
    .then(() => Bridge.agentsSync());
}
