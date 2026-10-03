// Claude Code hook events → island state.
// Port of HookServer.processEvent / processPermissionRequest from the macOS app.
// No terminal filter: the hook fires from any terminal and all of them are handled.

import { Bridge, onEvent } from "../core/bridge";
import { Sound } from "../core/sound";
import { State, type AskOption, type AskQuestion } from "../core/state";
import type { Island } from "./island";

const CLAUDE_ID = "integration_claude";

/** Clears the approval card if no decision was made before the hook gave up. */
let pendingTimeout: number | null = null;

/** The same deadline the relay uses (108 s), minus the time to get there. */
const QUESTION_TIMEOUT_MS = 110_000;

interface HookPayload {
  hook_event_name?: string;
  request_id?: string;
  session_id?: string;
  cwd?: string;
  message?: string;
  /** UserPromptSubmit carries `prompt`; `message` belongs to Notification/Stop. */
  prompt?: string;
  tool_name?: string;
  tool_input?: Record<string, unknown>;
  /** Optional agent tag: lowercase, digits and hyphens, ≤ 24 chars. */
  coucou_agent?: string;
}

/** Guardrail for the Linux relay: "claude" stays reserved for the Claude Code pill,
 *  so no third-party agent can shadow it. See docs/AGENTS.md. */
function validateAgent(raw: string | undefined): string | null {
  if (!raw || raw.length > 24 || raw === "claude") return null;
  if (!/^[a-z0-9-]+$/.test(raw)) return null;
  return raw;
}

const FALLBACK_COLORS = ["#22C55E", "#EAB308", "#60A5FA", "#E879F9"];
function agentColor(name: string): string {
  let h = 0;
  for (let i = 0; i < name.length; i++) {
    h = (Math.imul(31, h) + name.charCodeAt(i)) | 0;
  }
  return FALLBACK_COLORS[Math.abs(h) % FALLBACK_COLORS.length];
}

const PROJECT_ALIASES: Record<string, string> = {
  "notch-buddy": "Notch Buddy",
  notchbuddy: "Notch Buddy",
  notch_buddy: "Notch Buddy",
};

function aliasProjectName(name: string): string {
  return PROJECT_ALIASES[name.toLowerCase()] ?? name;
}

function lastPathComponent(p: string): string {
  const cleaned = p.replace(/[\\/]+$/, "");
  const idx = Math.max(cleaned.lastIndexOf("\\"), cleaned.lastIndexOf("/"));
  return idx >= 0 ? cleaned.slice(idx + 1) : cleaned;
}

/** frenchStep() — same labels as the macOS app. */
const TOOL_LABELS: Record<string, string> = {
  Bash: "Exécute",
  Read: "Lit",
  Write: "Écrit",
  Edit: "Modifie",
  Glob: "Cherche",
  Grep: "Recherche",
  WebSearch: "Recherche web",
  WebFetch: "Récupère",
  TodoWrite: "Tâches",
  Task: "Agent",
  LS: "Liste",
  MultiEdit: "Modifie",
  NotebookEdit: "Notebook",
  PowerShell: "Exécute",
};

function stepLabel(tool: string, input: Record<string, unknown>): string {
  const label = TOOL_LABELS[tool] ?? tool;
  const str = (k: string) => (typeof input[k] === "string" ? (input[k] as string) : null);
  const cmd = str("command");
  if (cmd) return `${label} · ${cmd.slice(0, 40)}`;
  const path = str("path");
  if (path) return `${label} · ${lastPathComponent(path)}`;
  const file = str("file_path");
  if (file) return `${label} · ${lastPathComponent(file)}`;
  const query = str("query");
  if (query) return `${label} · ${query.slice(0, 40)}`;
  return label;
}

/**
 * What the Allow button actually authorises. Approving "Write" tells you nothing;
 * approving `Write · …\.env` tells you everything, which is the whole point of
 * approving from here. Ordered by how specific the field is, so an unfamiliar tool
 * still shows whatever identifying string it carries.
 */
const APPROVAL_FIELDS = [
  "command", // Bash, PowerShell
  "file_path", // Write, Edit, MultiEdit, NotebookEdit
  "path", // Read, LS
  "url", // WebFetch
  "query", // WebSearch
  "pattern", // Glob, Grep
  "prompt", // Task
] as const;

/**
 * The text of an AskUserQuestion arriving the *old* way, as a PermissionRequest
 * (Claude Code < 2.1.85). There is nothing to approve and this relay cannot
 * answer, so the card shows the question and points at the terminal.
 */
function questionText(tool: string, input: Record<string, unknown>): string | null {
  if (tool !== "AskUserQuestion") return null;
  const questions = input.questions;
  if (Array.isArray(questions) && questions.length > 0) {
    const first = questions[0] as Record<string, unknown>;
    if (typeof first?.question === "string" && first.question.trim()) return first.question.trim();
  }
  return typeof input.question === "string" && input.question.trim() ? input.question.trim() : null;
}

/** Claude Code sends at most four questions of 2–4 options; outside that we
 *  decline rather than show a card we cannot finish. */
const MAX_QUESTIONS = 4;
const MAX_OPTIONS = 4;

/**
 * `tool_input.questions` into the card's model, or null when it is not something
 * we can answer honestly. A missing option label is dropped rather than guessed:
 * the answer is keyed by label and goes straight back into Claude Code's own
 * `updatedInput`, so a label we invented would be an answer nobody chose.
 */
export function parseAskQuestions(raw: unknown): AskQuestion[] | null {
  if (!Array.isArray(raw) || raw.length === 0 || raw.length > MAX_QUESTIONS) return null;
  const out: AskQuestion[] = [];
  for (const item of raw) {
    const q = item as Record<string, unknown>;
    const question = typeof q?.question === "string" ? q.question.trim() : "";
    if (!question) return null;
    const options: AskOption[] = [];
    if (Array.isArray(q.options)) {
      for (const opt of q.options) {
        const o = opt as Record<string, unknown>;
        const label = typeof o?.label === "string" ? o.label.trim() : "";
        if (!label) continue;
        options.push({
          label: label.slice(0, 120),
          description: typeof o.description === "string" ? o.description.trim().slice(0, 200) : "",
        });
      }
    }
    out.push({
      header: typeof q.header === "string" ? q.header.trim().slice(0, 24) : "",
      question,
      options: options.slice(0, MAX_OPTIONS),
      multiSelect: q.multiSelect === true,
    });
  }
  return out;
}

function approvalTarget(tool: string, input: Record<string, unknown>): string {
  const question = questionText(tool, input);
  if (question) return question;
  for (const field of APPROVAL_FIELDS) {
    const value = input[field];
    if (typeof value === "string" && value.trim()) {
      return `${tool} · ${value.trim()}`;
    }
  }
  return tool;
}

function upsert(projectName: string, cwd: string) {
  const t = State.tasks.find((x) => x.id === CLAUDE_ID);
  if (!t) return;
  t.name = projectName;
  if (cwd) t.sessionCwd = cwd;
}

function clearSession() {
  const t = State.tasks.find((x) => x.id === CLAUDE_ID);
  if (!t) return;
  t.steps = [];
  t.stepIndex = 0;
  t.name = "Claude Code";
  t.pillBadge = null;
}

export function registerHookHandlers(island: Island) {
  void onEvent<HookPayload>("hook", (payload) => handleHook(island, payload));
}

function handleHook(island: Island, payload: HookPayload) {
  if (State.paused) {
    // Silence used to cost Claude Code nearly two minutes: the relay waited on an
    // island that had already decided not to look.
    if (payload.request_id) void Bridge.approvalDecline(payload.request_id);
    return;
  }

  const name = payload.hook_event_name ?? "";
  const cwd = payload.cwd ?? "";
  const raw = lastPathComponent(cwd);
  const projectName = aliasProjectName(raw || "Session");

  // Route to the right pill. Valid coucou_agent → dynamic "agent_<name>" pill.
  // "claude" is reserved; absent or invalid → Claude Code pill unchanged.
  const validAgent = validateAgent(payload.coucou_agent);
  const agentId = validAgent ? `agent_${validAgent}` : CLAUDE_ID;
  const isExternalAgent = validAgent !== null;

  const focused = State.focusId === agentId;

  /** Alerts force the island open; work events only reveal the compact island. */
  const surface = (view: Parameters<Island["alert"]>[0], isAlert: boolean) => {
    if (State.mode === "expanded") {
      if (isAlert) island.setView(view);
    } else if (isAlert) {
      island.alert(view);
    } else if (State.mode === "hidden") {
      island.reveal();
    }
  };

  /** Ensure the agent pill exists (no-op for Claude Code). */
  const ensurePill = () => {
    if (isExternalAgent) {
      State.upsertExternalAgent(agentId, validAgent!, agentColor(validAgent!));
    } else {
      upsert(projectName, cwd);
    }
  };

  /**
   * Open the question card, or decline when we cannot answer it honestly.
   *
   * One card, one request, exactly like the approval path: a second question
   * that arrives while one is on screen is declined so the first is not
   * silently replaced. Anything the parser rejects goes back to the terminal,
   * which is where Claude Code would have asked anyway.
   */
  const openQuestion = (): boolean => {
    const requestId = payload.request_id ?? "";
    const questions = parseAskQuestions(payload.tool_input?.questions);
    if (!questions || questions.some((q) => q.options.length === 0)) {
      if (requestId) void Bridge.approvalDecline(requestId);
      return false;
    }
    if (State.pendingQuestion && State.pendingQuestion.requestId !== requestId) {
      if (requestId) void Bridge.approvalDecline(requestId);
      return false;
    }
    if (pendingTimeout != null) window.clearTimeout(pendingTimeout);
    upsert(projectName, cwd);
    State.pendingQuestion = {
      requestId,
      sessionId: payload.session_id ?? "",
      questions,
      index: 0,
      answers: {},
      freeText: "",
    };
    // The relay's ack window closes in 800 ms and everything below is
    // synchronous, so the card is really up by the time it lands.
    if (requestId) void Bridge.approvalAck(requestId);
    State.updateTask(CLAUDE_ID, "approval");
    State.isPinned = true;
    Sound.play("approval");
    surface("question", true);
    // Coucou answers within 108 s or not at all; after that the terminal has
    // taken over and a card still on screen would be lying.
    pendingTimeout = window.setTimeout(() => {
      pendingTimeout = null;
      if (!State.pendingQuestion) return;
      State.pendingQuestion = null;
      State.isPinned = false;
      island.dropPin();
      State.updateTask(CLAUDE_ID, "working");
      State.setPillBadge(CLAUDE_ID, null);
      if (State.view === "question") island.setView(State.defaultView());
      State.notify();
    }, QUESTION_TIMEOUT_MS);
    return true;
  };

  switch (name) {
    case "SessionStart":
      ensurePill();
      surface("overview", false);
      Sound.play("work");
      break;

    case "UserPromptSubmit": {
      ensurePill();
      State.updateTask(agentId, "thinking");
      // The field is `prompt`; reading `message` meant this step was always blank.
      const asked = payload.prompt ?? payload.message;
      if (asked) State.appendStep(agentId, asked.slice(0, 60));
      surface("overview", false);
      break;
    }

    case "PreToolUse": {
      ensurePill();
      State.updateTask(agentId, "working");
      const tool = payload.tool_name ?? "Tool";
      // The one PreToolUse that waits: Claude Code 2.1.85+ sends
      // AskUserQuestion here, on the dedicated `--ask` hook. The general
      // PreToolUse entry also fires for it and is the fire-and-forget path
      // below, which is why the island only opens a card when the relay sent a
      // request id — that id only exists on the ask hook's connection.
      if (tool === "AskUserQuestion" && payload.request_id) {
        if (openQuestion()) break;
      }
      State.appendStep(agentId, stepLabel(tool, payload.tool_input ?? {}));
      surface("overview", false);
      break;
    }

    case "PostToolUse":
      State.updateTask(agentId, "working");
      break;

    case "PostToolUseFailure":
      State.updateTask(agentId, "working");
      State.appendStep(agentId, "⚠ failed");
      break;

    case "Notification": {
      const message = payload.message ?? "";
      const lower = message.toLowerCase();
      if (lower.includes("rate limit") || lower.includes("limite d")) {
        State.updateTask(agentId, "ratelimit");
        Sound.play("rate");
      } else if (message.endsWith("?")) {
        State.updateTask(agentId, "question");
        State.appendStep(agentId, message);
      }
      break;
    }

    case "Stop":
      State.updateTask(agentId, "finished");
      if (payload.message) State.appendStep(agentId, payload.message.slice(0, 60));
      Sound.play("finish");
      if (focused) surface("finished", true);
      else State.setPillBadge(agentId, "finished");
      window.setTimeout(() => {
        if (isExternalAgent) {
          State.removeTask(agentId);
        } else {
          State.updateTask(agentId, "idle");
          State.setPillBadge(agentId, null);
        }
      }, 5200);
      break;

    case "StopFailure":
      State.updateTask(agentId, "error");
      Sound.play("error");
      if (focused) surface("error", true);
      else State.setPillBadge(agentId, "error");
      break;

    case "SessionEnd":
      if (isExternalAgent) {
        State.removeTask(agentId);
      } else {
        State.updateTask(agentId, "idle");
        clearSession();
      }
      break;

    case "SubagentStart":
      State.appendStep(agentId, "+ subagent");
      break;

    case "SubagentStop":
      State.appendStep(agentId, "• subagent done");
      break;

    case "PermissionRequest": {
      // External agents get no approval card — one would look like a Claude Code
      // request. Decline immediately so the agent re-asks in its own terminal.
      if (isExternalAgent) {
        if (payload.request_id) void Bridge.approvalDecline(payload.request_id);
        break;
      }

      const requestId = payload.request_id ?? "";
      const tool = payload.tool_name ?? "Tool";
      const input = payload.tool_input ?? {};
      const command = approvalTarget(tool, input);
      const sessionId = payload.session_id ?? "";
      // "Always" was already clicked for this tool and target in this Claude Code
      // session, so the card never appears — which is the point of the button.
      if (State.approvalRemembered(sessionId, tool, command)) {
        if (requestId) void Bridge.approvalAck(requestId);
        if (requestId) void Bridge.approvalDecision(requestId, "allow");
        void Bridge.log(`auto-allow ${command}`);
        upsert(projectName, cwd);
        break;
      }
      // One card, one request: a second one must never quietly replace the first,
      // which would leave a human staring at B while A waits. Back to the terminal.
      if (State.pendingApproval && State.pendingApproval.requestId !== requestId) {
        if (requestId) void Bridge.approvalDecline(requestId);
        break;
      }
      upsert(projectName, cwd);
      if (pendingTimeout != null) window.clearTimeout(pendingTimeout);
      State.pendingApproval = {
        requestId,
        sessionId,
        tool,
        command,
      };
      // The relay's short ack window closes in 800 ms; everything below this
      // line is synchronous, so the card really is up by the time it lands.
      if (requestId) void Bridge.approvalAck(requestId);
      State.updateTask(CLAUDE_ID, "approval");
      State.isPinned = true;
      Sound.play("approval");
      if (focused) {
        island.alert("approval");
      } else {
        // Another agent holds the view, so the card would yank it away. The badge
        // is the signal instead — but only if it is on screen, and we just told the
        // relay a human can act.
        State.setPillBadge(CLAUDE_ID, "approval");
        island.reveal();
      }
      // Coucou answers within 108 s or not at all; after that the terminal has
      // taken over and the card would be lying.
      pendingTimeout = window.setTimeout(() => {
        pendingTimeout = null;
        if (!State.pendingApproval) return;
        State.pendingApproval = null;
        State.isPinned = false;
        island.dropPin();
        State.updateTask(CLAUDE_ID, "working");
        State.setPillBadge(CLAUDE_ID, null);
        if (State.view === "approval") island.setView(State.defaultView());
        State.notify();
      }, 110_000);
      break;
    }

    default:
      break;
  }
  State.notify();
}

// ── Answering a question ─────────────────────────────────────────────────────

/** Everything the card needs to finish: clear it, hand the answers over, let go
 *  of the pin. Shared by the three ways a question can end. */
function finishQuestion(island: Island, decision: "answer" | "ask") {
  const q = State.pendingQuestion;
  if (!q) return null;
  if (pendingTimeout != null) {
    window.clearTimeout(pendingTimeout);
    pendingTimeout = null;
  }
  if (decision === "answer") {
    void Bridge.questionAnswer(q.requestId, q.answers);
    void Bridge.log(`answered ${Object.keys(q.answers).length} question(s)`);
  } else {
    // No decision: the relay writes nothing back and coucou-hook prints nothing,
    // so Claude Code asks the question again in the terminal.
    void Bridge.approvalDecline(q.requestId);
    void Bridge.log("question released to the terminal");
  }
  State.pendingQuestion = null;
  State.isPinned = false;
  island.dropPin();
  State.updateTask(CLAUDE_ID, "working");
  State.setPillBadge(CLAUDE_ID, null);
  State.notify();
  island.setView(State.defaultView());
  return q;
}

/** Single-select: a click is the answer. Multi-select: a toggle, and Send. */
export function pickOption(island: Island, label: string) {
  const q = State.pendingQuestion;
  const question = State.currentQuestion;
  if (!q || !question) return;
  if (question.multiSelect) {
    const current = Array.isArray(q.answers[question.question])
      ? [...(q.answers[question.question] as string[])]
      : [];
    const at = current.indexOf(label);
    if (at >= 0) current.splice(at, 1);
    else current.push(label);
    q.answers[question.question] = current;
    Sound.play("blip");
    State.notify();
    return;
  }
  q.answers[question.question] = label;
  Sound.play("approve");
  finishQuestion(island, "answer");
}

/** The free-text "Other…" field: its text replaces whatever was picked. */
export function setFreeText(_island: Island, text: string) {
  const q = State.pendingQuestion;
  const question = State.currentQuestion;
  if (!q || !question) return;
  q.freeText = text;
  if (text.trim()) q.answers[question.question] = text.trim();
  else delete q.answers[question.question];
  State.notify();
}

/** Send / Next. The last question, or a multi-select one, is what sends. */
export function submitQuestion(island: Island) {
  const q = State.pendingQuestion;
  if (!q) return;
  const question = q.questions[q.index];
  if (!question) return;
  const answered = q.answers[question.question];
  const empty = answered === undefined || answered === "" || (Array.isArray(answered) && !answered.length);
  if (empty) return;
  if (q.index < q.questions.length - 1) {
    q.index += 1;
    q.freeText = "";
    Sound.play("blip");
    State.notify();
    return;
  }
  Sound.play("approve");
  finishQuestion(island, "answer");
}

/** "Reply in terminal" — the same thing as not answering, said out loud. */
export function replyInTerminal(island: Island) {
  finishQuestion(island, "ask");
}
