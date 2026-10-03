// Island views — DOM ports of IslandViewContent.swift: paddings, font sizes,
// colours and wording are copied so both platforms read identically.

import { h, svg, clear, dot } from "./dom";
import { ICONS } from "./icons";
import { Ticker } from "./ticker";
import { State, type AgentTask } from "../core/state";
import { agentName } from "../core/agents-meta";
import { washRGBA, type IslandViewName, type Wash } from "../core/layout";
import { createMiniBot, pruneMiniBots } from "../mochi/minibots";
import { buildPrompt } from "./chat";
import { buildChoose, buildUpload, buildUploading } from "./upload";
import { renderIntegrationCard, type IntegrationCardHooks } from "./integrations";

export interface ViewActions {
  setView(v: IslandViewName): void;
  collapse(): void;
  setFocus(id: string): void;
  openTerminal(): void;
  /** The ↗ button: opens whatever the focused pill points at. */
  openTarget(): void;
  openUrl(url: string): void;
  decide(d: "allow" | "deny" | "always"): void;
  /** Single-select: one click answers. Multi-select: a toggle. */
  pickQuestion(label: string): void;
  /** The free-text "Other…" field. */
  setQuestionText(text: string): void;
  /** Send / Next. */
  submitQuestion(): void;
  /** "Reply in terminal" — release it and let Claude Code ask again. */
  replyInTerminal(): void;
  toggleSound(): void;
  setVolume(v: number): void;
  setAutoClose(seconds: number): void;
  openSettingsWindow(): void;
  blip(): void;
}

export interface ViewHost {
  el: HTMLElement;
  sync(): void;
  /** Called when the view becomes active, for views with a text field. */
  focus?(): void;
  /** Called every frame while the view is on screen. */
  tick?(nowMs: number): void;
  /** True while the view has a transition of its own in flight, which keeps
   *  the frame loop alive until it lands (#43). */
  readonly animating?: boolean;
}

// ── Shared pieces ─────────────────────────────────────────────────────────────

function card(wash: Wash, ...children: (Node | string)[]): HTMLElement {
  const el = h("div", { class: wash ? "card wash" : "card" }, ...children);
  if (wash) el.style.setProperty("--wash", washRGBA(wash));
  return el;
}

function btn(
  label: string,
  kind: "primary" | "secondary",
  onClick: () => void,
  kbd?: string,
): HTMLElement {
  return h(
    "button",
    { class: `btn ${kind}`, onclick: onClick },
    h("span", { text: label }),
    kbd ? h("span", { class: "kbd", text: kbd }) : null,
  );
}

/**
 * The grey chip beside an agent's name. It used to hard-code "Claude Code" for
 * anything that was not an n8n integration, so every adapter pill was labelled as
 * the wrong tool; a pill already carries its own name.
 */
function sourceLabel(task: AgentTask | null): string {
  if (!task) return "";
  if (task.source === "n8n") return "n8n";
  // The session or model the adapter reported rides the ticker, where it has room;
  // as a chip it was the same word twice ("Hermes hermes-aux").
  if (task.source === "agent") return agentName(task.id);
  return "Claude Code";
}

/**
 * The name on a card. A Claude Code card used to show the *folder* the session was
 * in, so an approval asked from ~/…/coucou read "coucou needs permission" — as
 * though a project were the one asking.
 */
function whoName(task: AgentTask | null): string {
  if (!task) return "Claude Code";
  return task.source === "agent" ? agentName(task.id) : "Claude Code";
}

function agentWho(task: AgentTask | null, label: string): HTMLElement {
  const row = h("div", { class: "who-row" });
  if (task) {
    row.append(dot(task.color, 8), h("span", { class: "n", text: whoName(task) }));
  }
  row.append(h("span", { text: label }));
  return row;
}

function stack(padLeft: number, padRight: number, ...children: Node[]): HTMLElement {
  const el = h("div", { class: "stack" }, ...children);
  el.style.padding = `4px ${padRight}px 4px ${padLeft}px`;
  return el;
}

// ── Header ────────────────────────────────────────────────────────────────────

export function buildHeader(actions: ViewActions): ViewHost {
  const tabHome = h("button", { class: "tab", title: "Overview", onclick: () => go("overview") }, svg(ICONS.house, 13));
  const tabChat = h("button", { class: "tab", title: "Ask", onclick: () => go("prompt") }, svg(ICONS.bubble, 13));
  const tabDrop = h("button", { class: "tab", title: "Drop", onclick: () => go("upload") }, svg(ICONS.plus, 13));

  const gearBtn = h("button", { title: "Settings", onclick: () => go("settings") }, svg(ICONS.gear, 14));
  const soundBtn = h("button", { title: "Mute", onclick: () => actions.toggleSound() }, svg(ICONS.speakerOn, 14));

  function go(v: IslandViewName) {
    actions.blip();
    actions.setView(v);
  }

  const el = h(
    "div",
    { id: "header" },
    h("div", { class: "tabs" }, tabHome, tabChat, tabDrop),
    h("div", { class: "header-actions" }, gearBtn, soundBtn),
  );

  return {
    el,
    sync() {
      const v = State.view;
      tabHome.classList.toggle("on", v === "overview" || v === "empty");
      tabChat.classList.toggle("on", v === "prompt");
      tabDrop.classList.toggle("on", v === "upload");
      gearBtn.classList.toggle("on", v === "settings");
      clear(gearBtn);
      gearBtn.append(svg(v === "settings" ? ICONS.gearFill : ICONS.gear, 14));
      clear(soundBtn);
      soundBtn.append(svg(State.settings.soundEnabled ? ICONS.speakerOn : ICONS.speakerOff, 14));
      el.style.opacity = v === "confused" ? "0" : "1";
    },
  };
}

// ── Overview ──────────────────────────────────────────────────────────────────

function buildOverview(actions: ViewActions): ViewHost {
  const ticker = new Ticker();
  const who = h("div", { class: "who" });
  const tickerBody = h("div", { class: "card-body" }, who, ticker.el);
  const leftBody = h("div", { class: "left-body" });
  const jump = h(
    "button",
    { class: "icon-btn jump", title: "Open", onclick: () => actions.openTarget() },
    svg(ICONS.arrowUpRight, 8),
  );
  const left = card(null, leftBody, jump);
  const pills = h("div", { class: "pills" });
  const right = card(null, pills);

  const el = h("div", { class: "view overview" },
    h("div", { class: "left" }, left),
    h("div", { class: "right" }, right),
  );

  let pillIds = "";
  let detailOpen = false;
  let lastFocus: string | null = null;
  let mode: "ticker" | "card" | null = null;
  let cardKey = "";

  const hooks: IntegrationCardHooks = {
    get detailOpen() {
      return detailOpen;
    },
    openDetail() {
      detailOpen = true;
      cardKey = "";
      State.notify();
    },
    closeDetail() {
      detailOpen = false;
      cardKey = "";
      State.notify();
    },
    openSettings: () => actions.openSettingsWindow(),
  };

  return {
    el,
    tick(nowMs: number) {
      if (mode === "ticker") ticker.tick(nowMs);
    },
    get animating() {
      return mode === "ticker" && ticker.animating;
    },
    sync() {
      const task = State.focusTask;
      if (task?.id !== lastFocus) {
        lastFocus = task?.id ?? null;
        detailOpen = false;
        cardKey = "";
        mode = null;
      }

      // A live session keeps the ticker; an integration pill shows its own card.
      // Agent pills are sessions, not pollers — without this they rendered as an
      // empty integration card.
      const sessionActive =
        task != null &&
        (task.id === "integration_claude" || task.source === "agent") &&
        (task.state !== "idle" || task.steps.length > 0);

      if (task && sessionActive) {
        if (mode !== "ticker") {
          clear(leftBody);
          leftBody.append(tickerBody);
          mode = "ticker";
          cardKey = "";
        }
        clear(who);
        who.append(
          dot(task.color, 7),
          h("span", { class: "name", text: task.name }),
          h("span", { class: "tool", text: sourceLabel(task) }),
        );
        if (task.steps.length > 1) {
          who.append(h("span", {
            class: "count",
            text: `${Math.min(task.stepIndex + 1, task.steps.length)}/${task.steps.length}`,
          }));
        }
        ticker.sync(task);
      } else if (task) {
        const info = State.integrations[task.id];
        const key = [
          task.id, detailOpen, task.state, task.steps.join("|"),
          info?.loaded, info?.error, info?.configured,
          JSON.stringify(info?.data ?? {}),
        ].join("~");
        if (key !== cardKey) {
          cardKey = key;
          mode = "card";
          clear(leftBody);
          leftBody.append(renderIntegrationCard(task, hooks));
        }
      }

      jump.style.display = detailOpen ? "none" : "";

      const others = State.otherTasks.slice(0, 4);
      const pillKey = others.map((t) => `${t.id}:${t.pillBadge ?? ""}`).join("|");
      if (pillKey !== pillIds) {
        pillIds = pillKey;
        clear(pills);
        for (const t of others) pills.append(buildPill(t, actions));
        pruneMiniBots();
      }
    },
  };
}

function buildPill(task: AgentTask, actions: ViewActions): HTMLElement {
  const label = task.id === "integration_claude" ? "Claude Code" : task.name;
  const canvas = createMiniBot(task, 24);
  const pill = h(
    "div",
    { class: "pill", onclick: () => actions.setFocus(task.id) },
    canvas,
    h("span", { class: "lbl", text: label }),
  );
  pill.style.borderColor = `${task.color}24`;
  pill.addEventListener("mouseenter", () => {
    pill.style.background = `${task.color}2e`;
    pill.style.borderColor = `${task.color}8c`;
    pill.style.boxShadow = `0 2px 10px ${task.color}59`;
    (pill.querySelector(".lbl") as HTMLElement).style.color = lighten(task.color, 0.3);
  });
  pill.addEventListener("mouseleave", () => {
    pill.style.background = "";
    pill.style.borderColor = `${task.color}24`;
    pill.style.boxShadow = "";
    (pill.querySelector(".lbl") as HTMLElement).style.color = "";
  });

  if (task.pillBadge) {
    const colors = { approval: "#F5A524", finished: "#22C55E", error: "#F4505E" } as const;
    const icons = { approval: ICONS.bang, finished: ICONS.check, error: ICONS.xmark } as const;
    const inner = h("i", { style: `background:${colors[task.pillBadge]}` }, svg(icons[task.pillBadge], 6, { stroke: task.pillBadge === "finished" ? 3 : 0 }));
    const badge = h("div", { class: "pill-badge" }, inner);
    badge.style.boxShadow = `0 0 4px ${colors[task.pillBadge]}99`;
    pill.append(badge);
  }
  return pill;
}

function lighten(hex: string, amount: number): string {
  const v = parseInt(hex.replace("#", ""), 16);
  const c = [(v >> 16) & 255, (v >> 8) & 255, v & 255].map((x) =>
    Math.min(255, Math.round(x + amount * 255)),
  );
  return `rgb(${c[0]},${c[1]},${c[2]})`;
}

// ── Empty ─────────────────────────────────────────────────────────────────────

function buildEmpty(actions: ViewActions): ViewHost {
  const body = h(
    "div",
    { class: "stack", style: "padding:0 18px 0 118px;flex-direction:row;align-items:center;gap:16px" },
    h(
      "div",
      { style: "display:flex;flex-direction:column;gap:5px" },
      h("div", { class: "title", text: "Nothing running right now." }),
      h("div", { class: "sub", text: "Drop a file or window, or ask me anything." }),
    ),
    h("div", { class: "grow" }),
    btn("Ask Claude", "primary", () => actions.setView("prompt")),
  );
  return { el: h("div", { class: "view" }, card(null, body)), sync() {} };
}

// ── Approval ──────────────────────────────────────────────────────────────────

function buildApproval(actions: ViewActions): ViewHost {
  const who = h("div");
  const code = h("div", { class: "code" });
  const row = h("div", { class: "actions" });
  const el = h("div", { class: "view" }, card("amber", stack(116, 16, who, code, row)));
  let rowKey = "";
  return {
    el,
    sync() {
      const approval = State.pendingApproval;
      // AskUserQuestion rides in on PermissionRequest, but the hook protocol cannot
      // carry a multiple-choice answer — approving it just lets the terminal ask, so
      // the button names where the answer actually goes.
      const isQuestion = approval?.tool === "AskUserQuestion";
      clear(who);
      who.append(agentWho(State.focusTask, isQuestion ? "is asking a question" : "needs permission"));
      // The point of approving here rather than in the terminal: this line is the
      // command, the file path or the URL being authorised.
      code.textContent = approval?.command || approval?.tool || "…";
      // Built once per mode: rebuilding between mouse-down and mouse-up would
      // swallow the click, and nothing varies within a mode. The mode does change
      // between cards, so it is part of the key rather than a rebuild per sync.
      const key = isQuestion ? "question" : "permission";
      if (rowKey === key) return;
      rowKey = key;
      clear(row);
      // The guidance rides in the button row: a fourth child squeezed the question
      // text — the one line on this card that must be readable — into half height.
      if (isQuestion) row.append(h("div", { class: "sub", text: "Answer in your terminal." }));
      // No key badges, deliberately: the island is a Dock window with
      // accept_focus(false) and this desktop's window manager never hands it the
      // keyboard — a global X11 grab, accept_focus(true) and a Dialog type hint
      // were all tried. A letter on a button that cannot be pressed is a promise
      // the app breaks exactly when the user needs it to hold.
      //
      // A question cannot be answered from here yet — upstream carries it (#165),
      // this relay does not — so it gets one button and a way home. An approval
      // gets the three choices it actually has: reject it, allow once, or stop
      // asking for the rest of the session.
      row.append(
        btn(isQuestion ? "Cancel" : "Reject", "secondary", () => actions.decide("deny")),
        ...(isQuestion
          ? [btn("Reply in terminal", "primary", () => actions.decide("allow"))]
          : [
              btn("Once", "secondary", () => actions.decide("allow")),
              btn("Always", "primary", () => actions.decide("always")),
            ]),
      );
    },
  };
}

// ── Question ────────────────────────────────────────────────────────────────

/**
 * Claude Code's `AskUserQuestion`, answered from the island (#165).
 *
 * One question at a time with a 1/N counter, its options as buttons, and a free
 * "Other…" field for anything not on the list. Single-select sends on the click;
 * multi-select and multi-question need Send/Next, so a half-answered question
 * is never sent by accident.
 */
function buildQuestion(actions: ViewActions): ViewHost {
  const who = h("div");
  const head = h("div", { class: "q-head" });
  const title = h("div", { class: "title" });
  const options = h("div", { class: "q-options" });
  const other = h("input", {
    class: "q-other",
    type: "text",
    placeholder: "Other…",
    maxlength: "300",
    oninput: (e) => actions.setQuestionText((e.target as HTMLInputElement).value),
    onkeydown: (e) => {
      const k = e as KeyboardEvent;
      if (k.key !== "Enter") return;
      k.preventDefault();
      actions.submitQuestion();
    },
  });
  const row = h("div", { class: "actions" });
  const el = h(
    "div",
    { class: "view" },
    card("cyan", stack(116, 16, who, head, title, options, other, row)),
  );

  return {
    el,
    sync() {
      const q = State.pendingQuestion;
      const question = State.currentQuestion;
      clear(who);
      who.append(agentWho(State.focusTask, "Claude Code is asking"));

      clear(head);
      clear(options);
      clear(row);
      if (!q || !question) {
        // The old PermissionRequest shape, or a question that timed out: show
        // the text and point at the terminal, which is where it gets answered.
        title.textContent = State.focusTask?.steps.at(-1) ?? "Claude needs an answer.";
        row.append(h("div", { class: "sub", text: "Answer in your terminal." }));
        other.style.display = "none";
        options.style.display = "none";
        return;
      }

      other.style.display = "";
      options.style.display = "";

      const counter = h("span", { class: "q-count", text: `${q.index + 1}/${q.questions.length}` });
      head.append(
        ...(question.header ? [h("span", { class: "q-tag", text: question.header })] : []),
        counter,
      );
      title.textContent = question.question;

      const raw = q.answers[question.question];
      const picked = new Set<string>(typeof raw === "string" ? [raw] : raw ?? []);
      for (const option of question.options) {
        const on = picked.has(option.label);
        const b = h(
          "button",
          { class: `q-option${on ? " on" : ""}`, onclick: () => actions.pickQuestion(option.label) },
          h("span", { class: "q-label", text: option.label }),
          option.description ? h("span", { class: "q-desc", text: option.description }) : null,
        );
        options.append(b);
      }
      if (other.value !== q.freeText) other.value = q.freeText;

      const needsSend = question.multiSelect || q.questions.length > 1;
      if (needsSend) {
        row.append(btn(q.index < q.questions.length - 1 ? "Next" : "Send", "primary", () => actions.submitQuestion()));
      }
      row.append(btn("Reply in terminal", "secondary", () => actions.replyInTerminal()));
    },
    focus() {
      const q = State.pendingQuestion;
      // Typing first is the fastest path when none of the options fit, so the
      // field takes the keyboard on the last question and stays out of the way
      // on the first.
      if (q?.questions.length === 1) other.focus();
    },
  };
}

// ── Error ─────────────────────────────────────────────────────────────────────

function buildError(actions: ViewActions): ViewHost {
  const who = h("div");
  const title = h("div", { class: "title", text: "Workflow stopped." });
  const detail = h("div", { class: "detail" });
  const row = h("div", { class: "actions" },
    btn("Retry", "primary", () => actions.setView(State.defaultView())),
    btn("Open in n8n", "secondary", () => actions.openUrl("")),
  );
  const el = h("div", { class: "view" }, card("red", stack(116, 16, who, title, detail, row)));
  return {
    el,
    sync() {
      const task = State.focusTask;
      clear(who);
      who.append(agentWho(task, sourceLabel(task)));
      title.textContent = task?.source === "n8n" ? "Workflow stopped." : "Session stopped on an error.";
      detail.textContent = task?.steps.at(-1) ?? "No detail available.";
    },
  };
}

// ── Finished ──────────────────────────────────────────────────────────────────

function buildFinished(actions: ViewActions): ViewHost {
  const who = h("div");
  const title = h("div", { class: "title" });
  const row = h("div", { class: "actions" },
    btn("Open terminal", "primary", () => actions.openTerminal()),
    btn("OK", "secondary", () => actions.collapse()),
  );
  const el = h("div", { class: "view" }, card("green", stack(116, 16, who, title, row)));
  return {
    el,
    sync() {
      clear(who);
      who.append(agentWho(State.focusTask, "Claude Code finished"));
      title.textContent = State.focusTask?.steps.at(-1) ?? "Session finished";
    },
  };
}

// ── Confused ──────────────────────────────────────────────────────────────────

function buildConfused(): ViewHost {
  const body = h(
    "div",
    { class: "stack", style: "padding:0 18px 0 128px" },
    h("div", { class: "title", text: "Too many hits at once." }),
    h("div", { class: "sub", text: "Give me a sec — back to work in three seconds." }),
  );
  return { el: h("div", { class: "view" }, card("pink", body)), sync() {} };
}

// ── Note ──────────────────────────────────────────────────────────────────────

function buildNote(): ViewHost {
  const title = h("div", { class: "title" });
  const el = h("div", { class: "view" }, card(null, h("div", { class: "stack", style: "padding:0 18px 0 98px" }, title)));
  return {
    el,
    sync() {
      title.textContent = State.noteMessage ?? "";
    },
  };
}

// ── In-island settings ────────────────────────────────────────────────────────

function buildSettings(actions: ViewActions): ViewHost {
  const soundSwitch = h("button", { class: "switch", onclick: () => actions.toggleSound() });
  const volume = h("input", {
    type: "range", min: "0", max: "1", step: "0.01", title: "Web Audio gain, 0–100%",
    oninput: (e: Event) => actions.setVolume(Number((e.target as HTMLInputElement).value)),
  }) as HTMLInputElement;
  const autoLabel = h("span", {});
  // The same five values Settings offers. It used to start at 10 s, so a user who
  // set 5 s came here to a segmented control with nothing highlighted.
  const AUTO_CLOSE_CHOICES = [5, 10, 15, 30, 60];
  const segButtons = AUTO_CLOSE_CHOICES.map((s) =>
    h("button", { onclick: () => actions.setAutoClose(s) }, `${s}s`),
  );
  const claudeBadge = h("span", { class: "status-badge" });
  const apiBadge = h("span", { class: "status-badge" });

  const rows = h(
    "div",
    { class: "settings-rows" },
    h("div", { class: "settings-row" }, soundSwitch, h("span", { text: "Sound" }), volume),
    h(
      "div",
      { class: "settings-row" },
      svg(ICONS.timer, 12),
      autoLabel,
      h("div", { class: "seg" }, ...segButtons),
    ),
    h(
      "div",
      { class: "settings-row", style: "gap:14px" },
      claudeBadge,
      apiBadge,
      h("div", { class: "grow" }),
      h("button", {
        class: "link-btn",
        style: "color:#8e939c;font-size:11.5px",
        text: "Settings…",
        onclick: () => actions.openSettingsWindow(),
      }),
    ),
  );

  const el = h("div", { class: "view" },
    card(null, h("div", { class: "stack", style: "padding:14px 16px 14px 84px" }, rows)));

  return {
    el,
    sync() {
      const s = State.settings;
      soundSwitch.classList.toggle("on", s.soundEnabled);
      volume.value = String(s.soundVolume);
      volume.style.opacity = s.soundEnabled ? "1" : "0.4";
      autoLabel.textContent = `Auto-close · ${Math.round(s.autoCloseInterval)}s`;
      segButtons.forEach((b, i) => b.classList.toggle("on", s.autoCloseInterval === AUTO_CLOSE_CHOICES[i]));
      clear(claudeBadge);
      claudeBadge.append(
        dot(s.hooksInstalled ? "#22C55E" : "#F4505E", 6),
        h("span", { text: "Claude Code" }),
      );
      clear(apiBadge);
      apiBadge.append(dot("#F4505E", 6), h("span", { text: "API" }));
    },
  };
}

// ── Placeholders filled in later stages ───────────────────────────────────────

function buildPlaceholder(title: string, sub: string): ViewHost {
  const body = h(
    "div",
    { class: "stack", style: "padding:0 18px 0 118px" },
    h("div", { class: "title", text: title }),
    h("div", { class: "sub", text: sub }),
  );
  return { el: h("div", { class: "view" }, card(null, body)), sync() {} };
}

// ── Registry ──────────────────────────────────────────────────────────────────

export function buildViews(
  actions: ViewActions,
  onChatHeightChange: () => void,
): Map<IslandViewName, ViewHost> {
  const map = new Map<IslandViewName, ViewHost>();
  map.set("overview", buildOverview(actions));
  map.set("empty", buildEmpty(actions));
  map.set("approval", buildApproval(actions));
  map.set("question", buildQuestion(actions));
  map.set("error", buildError(actions));
  map.set("finished", buildFinished(actions));
  map.set("confused", buildConfused());
  map.set("note", buildNote());
  map.set("settings", buildSettings(actions));
  map.set("prompt", buildPrompt(onChatHeightChange));
  map.set("upload", buildUpload());
  map.set("uploading", buildUploading());
  map.set("choose", buildChoose(actions));
  // Not in this port: sending a file by email, window attach + web result.
  map.set("mail", buildPlaceholder("Sending by email isn't in this version.", ""));
  map.set("searching", buildPlaceholder("Claude is searching…", ""));
  map.set("result", buildPlaceholder("Result", ""));
  return map;
}
