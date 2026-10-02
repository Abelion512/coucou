// AgentBus → State. The Rust side already funnels OpenCode, Hermes and
// Freebuff through one `agent` event (see src-tauri/src/agents/mod.rs); this is
// the other end of that wire. Without it the adapters run, log, and are thrown
// away — which is exactly how three live adapters ended up invisible.
//
// Per docs/SPEC-agent-pills.md: observe-only, one pill per agent (not per
// session), no front-end polling, no sound, and no PermissionReq path.

import { onEvent } from "../core/bridge";
import { State, type AgentTask } from "../core/state";
import type { BotStateName } from "../core/layout";

/** Serde `Agent` is kebab-case: claude-code | opencode | hermes | freebuff. */
type AgentId = "opencode" | "hermes" | "freebuff" | "claude-code";

/** The three this fork watches. Claude Code keeps its own hook path. */
interface AgentMeta {
  name: string;
  color: string;
}

const AGENTS: Record<AgentId, AgentMeta | null> = {
  opencode: { name: "OpenCode", color: "#5D9CFF" },
  hermes: { name: "Hermes", color: "#FFD700" },
  freebuff: { name: "Freebuff", color: "#2DD4BF" },
  // Claude Code events arrive on the `hook` socket, not here.
  "claude-code": null,
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
  const meta = AGENTS[agent];
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
        if (event.project) task.name = event.project;
        if (event.cwd) task.sessionCwd = event.cwd;
        if (event.sessionId && event.sessionId !== "server") {
          task.steps = [`session ${event.sessionId.slice(0, 8)}`];
          task.stepIndex = 0;
        }
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

      // Observe-only. The three adapters have no write path, so a decision can
      // never reach them; PermissionReq is Claude-Code-only by guardrail.
      case "permissionReq":
        break;

      default:
        break;
    }
  });
}
