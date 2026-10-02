// Settings window — the place where anything that writes to disk is confirmed.
// Stage 2 covers the Claude Code hooks and the general preferences; API keys and
// integrations land here too in a later stage.

import "./settings.css";
import { Bridge, onEvent, type AgentStatus, type HookStatus } from "../core/bridge";
import { DEFAULT_SETTINGS, type Settings } from "../core/state";
import { h, clear } from "../views/dom";

let settings: Settings = { ...DEFAULT_SETTINGS };
let version = "";

const root = document.getElementById("settings-root")!;

async function save() {
  await Bridge.saveSettings(settings);
}

// ── Reusable bits ─────────────────────────────────────────────────────────────

function toggle(on: boolean, onChange: (v: boolean) => void): HTMLElement {
  const el = h("button", { class: on ? "switch on" : "switch", "aria-pressed": on });
  el.addEventListener("click", () => {
    const next = !el.classList.contains("on");
    el.classList.toggle("on", next);
    onChange(next);
  });
  return el;
}

function statusDot(ok: boolean): HTMLElement {
  return h("i", { class: "dot", style: `background:${ok ? "#22c55e" : "#f4505e"}` });
}

function renderDiff(text: string): HTMLElement {
  const box = h("div", { class: "diff" });
  for (const line of text.split("\n")) {
    const cls = line.startsWith("+") ? "add" : line.startsWith("-") ? "del" : "ctx";
    box.append(h("div", { class: cls, text: line }));
  }
  return box;
}

// ── Claude Code section ───────────────────────────────────────────────────────

function claudeSection(status: HookStatus): HTMLElement {
  const body = h("div", { style: "display:flex;flex-direction:column;gap:12px" });
  const section = h(
    "section",
    {},
    h("h2", {}, statusDot(status.installed), h("span", { text: "Claude Code" })),
    body,
  );

  const rebuild = async () => {
    const fresh = await Bridge.hooksStatus();
    if (fresh) Object.assign(status, fresh);
    clear(body);
    draw();
    const head = section.querySelector("h2")!;
    clear(head);
    head.append(statusDot(status.installed), h("span", { text: "Claude Code" }));
  };

  function draw() {
    body.append(
      h("div", {
        class: "hint",
        text: status.installed
          ? "Coucou is hooked into your Claude Code sessions. Tool calls, questions and permission requests show up in the island, and you can answer them there."
          : "Install the hooks to see your Claude Code sessions in the island and approve permissions without leaving what you are doing.",
      }),
      h("div", { class: "row" },
        h("label", { text: "settings.json" }),
        h("span", { class: "path", text: status.settingsPath }),
      ),
      h("div", { class: "row" },
        h("label", { text: "Relay" }),
        h("span", { class: "path", text: status.hookPath }),
        statusDot(status.hookReady),
      ),
    );

    if (!status.hookReady) {
      body.append(h("div", {
        class: "notice warn",
        text: "The coucou-hook relay is not in place yet. Restart Coucou; if it still fails, build it with `cargo build -p coucou-hook`.",
      }));
    }

    const actions = h("div", { class: "row" });
    const install = h("button", {
      class: "primary",
      text: status.installed ? "Reinstall hooks…" : "Install hooks…",
      onclick: () => showPreview(true),
    });
    // Writing hook commands that point at a relay which isn't there would give
    // every Claude Code session a broken hook and nothing to show for it.
    if (!status.hookReady) {
      install.disabled = true;
      install.title = "The relay isn't installed yet.";
    }
    actions.append(install);
    if (status.installed) {
      actions.append(h("button", {
        class: "danger",
        text: "Uninstall hooks…",
        onclick: () => showPreview(false),
      }));
    }
    body.append(actions);
  }

  async function showPreview(install: boolean) {
    let preview;
    try {
      preview = await Bridge.hooksPreview(install);
    } catch (err) {
      // An unreadable or invalid settings.json stops here rather than being
      // treated as empty and written over.
      clear(body);
      body.append(
        h("div", { class: "notice err", text: String(err).replace(/^Error:\s*/, "") }),
        h("div", { class: "row" }, h("button", {
          text: "Back",
          onclick: () => { clear(body); draw(); },
        })),
      );
      return;
    }
    if (!preview) return;
    clear(body);
    body.append(
      h("div", {
        class: "hint",
        text: install
          ? "This is exactly what will change in your settings.json. Your own hooks are left untouched."
          : "This removes Coucou's entries only. Your own hooks are left untouched.",
      }),
      renderDiff(preview.diff),
      h("div", { class: "row" },
        h("span", { class: "path", text: `Backup → ${preview.backup}` }),
      ),
    );
    const confirm = h("button", {
      class: install ? "primary" : "danger",
      text: install ? "Back up and write" : "Back up and remove",
    });
    confirm.addEventListener("click", async () => {
      confirm.disabled = true;
      try {
        const backup = await Bridge.hooksApply(install, preview.fingerprint);
        clear(body);
        body.append(h("div", {
          class: "notice ok",
          text: `Done. Previous settings saved as ${backup}. Open a new Claude Code session to pick the hooks up.`,
        }));
        window.setTimeout(() => void rebuild(), 2600);
      } catch (err) {
        confirm.disabled = false;
        body.append(h("div", { class: "notice err", text: `Could not write: ${String(err)}` }));
      }
    });
    body.append(h("div", { class: "row" }, confirm, h("button", {
      text: "Cancel",
      onclick: () => { clear(body); draw(); },
    })));
  }

  draw();
  return section;
}

// ── Claude API section ────────────────────────────────────────────────────────

const MODELS: [string, string][] = [
  ["claude-opus-5", "Claude Opus 5"],
  ["claude-sonnet-5", "Claude Sonnet 5"],
  ["claude-haiku-4-5", "Claude Haiku 4.5"],
];

function apiSection(hasKey: boolean): HTMLElement {
  // A non-empty API base means a Messages-compatible relay: the key becomes
  // optional because many relays authenticate themselves.
  const usingRelay = () => settings.apiBase.trim().length > 0;
  const dot = statusDot(hasKey || usingRelay());
  const state = h("span", { class: "hint" });

  const baseField = h("input", {
    type: "text",
    placeholder: "https://api.anthropic.com",
    value: settings.apiBase,
    style: "flex:1 1 auto;min-width:0",
    autocomplete: "off",
    spellcheck: "false",
  }) as HTMLInputElement;
  baseField.addEventListener("change", () => {
    settings.apiBase = baseField.value.trim();
    void save();
    void refresh();
  });

  const field = h("input", {
    type: "password",
    placeholder: hasKey ? "••••••••••••  (stored)" : "sk-ant-...",
    style: "flex:1 1 auto;min-width:0",
    autocomplete: "off",
    spellcheck: "false",
  }) as HTMLInputElement;

  const saveBtn = h("button", { class: "primary", text: "Save key" });
  const clearBtn = h("button", { class: "danger", text: "Remove" });
  const feedback = h("div", {});

  async function refresh() {
    const present = (await Bridge.secretPresent("anthropic-api-key")) ?? false;
    const relay = usingRelay();
    dot.style.background = present || relay ? "#22c55e" : "#f4505e";
    state.textContent = relay
      ? present
        ? "Relay active — requests go to the base above, with the stored key."
        : "Relay active — requests go to the base above, no API key needed."
      : present
        ? "Key saved in the Secret Service keyring."
        : "No key yet — the chat needs one, or point the base at a relay.";
    field.placeholder = present ? "••••••••••••  (stored)" : "sk-ant-...";
    clearBtn.style.display = present ? "" : "none";
  }

  saveBtn.addEventListener("click", async () => {
    const value = field.value.trim();
    if (!value) return;
    clear(feedback);
    try {
      await Bridge.secretSet("anthropic-api-key", value);
      field.value = "";
      feedback.append(h("div", { class: "notice ok", text: "Saved. It never touches disk." }));
      await refresh();
    } catch (err) {
      feedback.append(h("div", { class: "notice err", text: `Could not save: ${String(err)}` }));
    }
  });

  clearBtn.addEventListener("click", async () => {
    clear(feedback);
    try {
      await Bridge.secretClear("anthropic-api-key");
      feedback.append(h("div", { class: "notice ok", text: "Key removed." }));
      await refresh();
    } catch (err) {
      feedback.append(h("div", { class: "notice err", text: `Could not remove: ${String(err)}` }));
    }
  });

  // Known models in a dropdown, plus "Custom…" for whatever a relay exposes.
  //
  // A relay can offer a thousand-plus models (9router lists 1013). Rendering
  // that is useless to read and slow to open, so the dropdown is only the
  // defaults plus the ids this machine has actually used, and the relay is asked
  // periodically which of *those* still exist. Nothing is ever silently removed:
  // a model the relay dropped is marked, because a typo in a config is more
  // likely than a deliberate deletion.
  const CUSTOM = "__custom__";
  const model = h("select", {}) as HTMLSelectElement;
  const missing = new Set<string>();

  const knownIds = () => {
    const ids = [...settings.recentModels];
    if (settings.model && !ids.includes(settings.model)) ids.unshift(settings.model);
    return ids;
  };

  const rebuildModelList = () => {
    const previous = model.value;
    clear(model);
    for (const [id, label] of MODELS) {
      model.append(h("option", { value: id, text: label }));
    }
    for (const id of knownIds()) {
      if (MODELS.some(([m]) => m === id)) continue;
      const gone = missing.has(id);
      model.append(h("option", {
        value: id,
        // "not on the relay" rather than a strike-through: a select option cannot
        // be styled reliably, and the label has to mean something when read aloud.
        text: gone ? `${id}  (not on the relay)` : id,
      }));
    }
    model.append(h("option", { value: CUSTOM, text: "Custom…" }));
    model.value = [...model.options].some((o) => o.value === previous) ? previous : settings.model;
  };

  const rememberModel = (id: string) => {
    settings.recentModels = [id, ...settings.recentModels.filter((m) => m !== id)].slice(0, 12);
  };
  const isKnown = (id: string) => MODELS.some(([m]) => m === id) || knownIds().includes(id);

  rebuildModelList();

  const customField = h("input", {
    type: "text",
    placeholder: "model id, e.g. hermes-aux or deepseek-v4",
    style: "flex:1 1 auto;min-width:0",
    autocomplete: "off",
    spellcheck: "false",
  }) as HTMLInputElement;
  const customRow = h("div", { class: "row" }, h("label", { text: "Model id" }), customField);
  customRow.style.display = "none";
  const syncCustom = () => {
    const show = model.value === CUSTOM;
    customRow.style.display = show ? "" : "none";
    if (show && !isKnown(settings.model)) customField.value = settings.model;
  };
  customField.addEventListener("change", () => {
    const value = customField.value.trim();
    if (!value) return; // an empty model id would silently break every request
    rememberModel(value);
    rebuildModelList();
    model.value = value;
    settings.model = value;
    void save();
  });
  model.addEventListener("change", () => {
    if (model.value === CUSTOM) {
      syncCustom();
      customField.focus();
      return;
    }
    if (model.value !== CUSTOM && !isKnown(model.value)) rememberModel(model.value);
    settings.model = model.value;
    void save();
    syncCustom();
  });

  // Ask the relay which of our ids are still there. Armed on the first use and
  // then slow: the settings window is created hidden and never destroyed, so a
  // timer started here would outlive every window the user ever opens.
  let armed = false;
  const catalogue = h("div", { class: "hint" });
  const checkModels = async () => {
    armed = true;
    const known = knownIds();
    if (!known.length || !usingRelay()) {
      catalogue.textContent = usingRelay()
        ? "Add a model id and it is checked against the relay from then on."
        : "";
      return;
    }
    const res = await Bridge.chatModelsCheck(known);
    if (!res) return;
    if (!res.reachable) {
      catalogue.textContent = "Relay did not answer — model list not verified.";
      return;
    }
    missing.clear();
    for (const id of res.missing) missing.add(id);
    if (res.missing.length) rebuildModelList();
    catalogue.textContent = res.missing.length
      ? `Relay lists ${res.total} models. Not on it: ${res.missing.join(", ")}.`
      : `Relay lists ${res.total} models — all ${known.length} of yours are on it.`;
  };
  // The window is hidden most of the time; a timer that outlives every visible
  // second of the app is the kind of thing the 0 %-when-hidden rule is about, so
  // it is armed only once the window has been opened, and it never fires while
  // `armed` is false.
  window.setInterval(() => {
    if (armed) void checkModels();
  }, 5 * 60_000);
  void onEvent("settings-opened", () => void checkModels());
  void checkModels();

  clearBtn.style.display = hasKey ? "" : "none";
  void refresh();

  return h(
    "section",
    {},
    h("h2", {}, dot, h("span", { text: "Claude" })),
    state,
    h("div", {
      class: "hint",
      text: "Point the base at a Messages-compatible relay (a gateway, LiteLLM, a Chinese model relay) to use custom models — the key becomes optional. Empty = the official Anthropic API.",
    }),
    h("div", { class: "row" }, h("label", { text: "API base" }), baseField),
    h("div", { class: "row" }, h("label", { text: "API key" }), field, saveBtn, clearBtn),
    h("div", { class: "row" }, h("label", { text: "Model" }), model),
    customRow,
    catalogue,
    feedback,
  );
}

// ── Integrations section ──────────────────────────────────────────────────────

interface IntegrationDef {
  id: string;
  name: string;
  color: string;
  /** Secret Service keys, in the order they are shown. */
  fields: { key: string; label: string; placeholder: string; secret: boolean }[];
}

const INTEGRATIONS: IntegrationDef[] = [
  { id: "integration_stripe", name: "Stripe", color: "#0570DE",
    fields: [{ key: "stripe-api-key", label: "Secret key", placeholder: "sk_live_…", secret: true }] },
  { id: "integration_github", name: "GitHub", color: "#F4505E",
    fields: [{ key: "github-token", label: "Token", placeholder: "ghp_…", secret: true }] },
  { id: "integration_vercel", name: "Vercel", color: "#7C5CFF",
    fields: [{ key: "vercel-token", label: "Token", placeholder: "…", secret: true }] },
  { id: "integration_n8n", name: "n8n", color: "#F29B38",
    fields: [
      { key: "n8n-url", label: "Instance URL", placeholder: "https://n8n.example.com", secret: false },
      { key: "n8n-api-key", label: "API key", placeholder: "…", secret: true },
    ] },
  { id: "integration_resend", name: "Resend", color: "#22C55E",
    fields: [{ key: "resend-api-key", label: "API key", placeholder: "re_…", secret: true }] },
  { id: "integration_notion", name: "Notion", color: "#8C8C8C",
    fields: [{ key: "notion-api-key", label: "Integration token", placeholder: "ntn_…", secret: true }] },
  { id: "integration_calcom", name: "Cal.com", color: "#C9956A",
    fields: [{ key: "calcom-api-key", label: "API key", placeholder: "cal_…", secret: true }] },
];

const MAX_ACTIVE = 4;

function integrationsSection(present: Record<string, boolean>): HTMLElement {
  const note = h("div", { class: "hint" });
  const list = h("div", { style: "display:flex;flex-direction:column;gap:14px" });

  function updateNote() {
    const used = settings.activeIntegrations.length;
    note.textContent = `Pick up to ${MAX_ACTIVE} pills to show next to Mochi — ${used}/${MAX_ACTIVE} in use. Keys are stored in the Secret Service keyring, never on disk.`;
  }

  for (const def of INTEGRATIONS) {
    const active = settings.activeIntegrations.includes(def.id);
    const sw = h("button", { class: active ? "switch on" : "switch" });
    sw.addEventListener("click", () => {
      const on = settings.activeIntegrations.includes(def.id);
      if (on) {
        settings.activeIntegrations = settings.activeIntegrations.filter((x) => x !== def.id);
      } else {
        if (settings.activeIntegrations.length >= MAX_ACTIVE) return;
        settings.activeIntegrations = [...settings.activeIntegrations, def.id];
      }
      sw.classList.toggle("on", !on);
      updateNote();
      void save();
    });

    const rows = h("div", { style: "display:flex;flex-direction:column;gap:6px;flex:1 1 auto;min-width:0" });
    for (const field of def.fields) {
      const input = h("input", {
        type: field.secret ? "password" : "text",
        placeholder: present[field.key] ? "••••••••  (stored)" : field.placeholder,
        autocomplete: "off",
        spellcheck: "false",
        style: "flex:1 1 auto;min-width:0",
      }) as HTMLInputElement;
      const saveBtn = h("button", { text: "Save" });
      const dotEl = statusDot(present[field.key] ?? false);
      saveBtn.addEventListener("click", async () => {
        const value = input.value.trim();
        try {
          await Bridge.secretSet(field.key, value);
          present[field.key] = value.length > 0;
          input.value = "";
          input.placeholder = value ? "••••••••  (stored)" : field.placeholder;
          dotEl.style.background = value ? "#22c55e" : "#f4505e";
        } catch {
          dotEl.style.background = "#f5a524";
        }
      });
      rows.append(
        h("div", { class: "row" },
          h("label", { style: "min-width:104px", text: field.label }),
          input, saveBtn, dotEl,
        ),
      );
    }

    list.append(
      h("div", { style: "display:flex;gap:12px;align-items:flex-start" },
        h("div", { style: "display:flex;align-items:center;gap:8px;min-width:132px;padding-top:4px" },
          sw,
          h("i", { class: "dot", style: `background:${def.color}` }),
          h("span", { style: "font-size:12.5px", text: def.name }),
        ),
        rows,
      ),
    );
  }

  updateNote();
  return h("section", {}, h("h2", {}, h("span", { text: "Integrations" })), note, list);
}

// ── Agents section ────────────────────────────────────────────────────────────

/** Serde ids. Colours match the pills in the island. */
const AGENT_META: Record<string, { name: string; color: string; how: string }> = {
  opencode: { name: "OpenCode", color: "#5D9CFF", how: "opencode serve --port 54321" },
  hermes: { name: "Hermes", color: "#FFD700", how: "hermes gateway run" },
  freebuff: { name: "Freebuff", color: "#2DD4BF", how: "run freebuff and send a message" },
};

/**
 * The three watched agents.
 *
 * The island shows a pill only while an agent is running, so "no pill" is correct
 * behaviour and also the only symptom when something is broken. This is the one
 * surface that tells the two apart. Nothing here polls: the window is created at
 * startup and never destroyed, so a timer would run forever behind a closed window —
 * it reads once and then follows the `agent` event, deduplicated exactly like the
 * island does.
 */
function agentsSection(initial: AgentStatus[] | null): HTMLElement {
  const rows = h("div", { style: "display:flex;flex-direction:column;gap:14px" });
  const note = h("div", { class: "hint" });

  // Adapters re-send the same state every few seconds; redraw only on a change.
  // The sentinel must not be a possible key: an empty list produces "", and a ""
  // initial value made the first paint a no-op.
  let lastKey: string | null = null;
  const paint = (list: AgentStatus[] | null) => {
    const key = (list ?? []).map((a) => `${a.agent}:${a.connected}:${a.endpoint ?? ""}`).join("|");
    if (key === lastKey) return;
    lastKey = key;
    clear(rows);
    note.textContent = "";

    // No list means the command did not answer — `bun run dev` in a plain browser,
    // or a build without the command. An empty section would read as "nothing is
    // configured", which is a different and wrong thing to say.
    if (!list) {
      rows.append(
        h("div", {
          class: "hint",
          text: "Agent status is unavailable outside the app. Start Coucou to see which agents are running.",
        }),
      );
      return;
    }

    for (const id of Object.keys(AGENT_META)) {
      const meta = AGENT_META[id];
      const s = list.find((x) => x.agent === id);
      const connected = s?.connected ?? false;
      const where = s?.endpoint || s?.detail || "";
      rows.append(
        h("div", { style: "display:flex;gap:12px;align-items:center" },
          h("div", { style: "display:flex;align-items:center;gap:8px;min-width:132px" },
            statusDot(connected),
            h("i", { class: "dot", style: `background:${meta.color}` }),
            h("span", { style: "font-size:12.5px", text: meta.name }),
          ),
          h("div", { style: "flex:1 1 auto;min-width:0;font-size:12px;color:#8e939c" },
            h("div", {
              text: connected ? (where || "connected") : `not running — ${meta.how}`,
              style: connected ? "" : "color:#6b7079",
            }),
          ),
        ),
      );
    }
    note.textContent = "Each agent gets a pill in the island while it is running. Coucou only watches: it never sends anything to these agents.";
  };

  paint(initial);

  void onEvent<{ type: string; agent: string }>("agent", () => {
    void Bridge.agentsStatus().then((s) => paint(s));
  });

  return h("section", {}, h("h2", {}, h("span", { text: "Agents" })), note, rows);
}

// ── General section ───────────────────────────────────────────────────────────

function generalSection(): HTMLElement {
  const volume = h("input", {
    type: "range", min: "0", max: "0.2", step: "0.005",
    value: String(settings.soundVolume),
  }) as HTMLInputElement;
  volume.addEventListener("input", () => {
    settings.soundVolume = Number(volume.value);
    void save();
  });

  const autoClose = h("input", {
    type: "number", min: "5", max: "120", step: "1",
    value: String(Math.round(settings.autoCloseInterval)),
    style: "width:72px",
  }) as HTMLInputElement;
  autoClose.addEventListener("change", () => {
    settings.autoCloseInterval = Math.max(5, Math.min(120, Number(autoClose.value) || 15));
    autoClose.value = String(settings.autoCloseInterval);
    void save();
  });

  const screen = h("select", {}) as HTMLSelectElement;
  screen.append(
    h("option", { value: "primary", text: "Main display" }),
    h("option", { value: "cursor", text: "Display under the cursor" }),
  );
  screen.value = settings.screen;
  screen.addEventListener("change", () => {
    settings.screen = screen.value as Settings["screen"];
    void save();
  });

  return h(
    "section",
    {},
    h("h2", {}, h("span", { text: "General" })),
    h("div", { class: "row" },
      h("label", { text: "Sound" }),
      toggle(settings.soundEnabled, (v) => { settings.soundEnabled = v; void save(); }),
      volume,
    ),
    h("div", { class: "row" },
      h("label", { text: "Auto-close" }),
      autoClose,
      h("span", { class: "hint", text: "seconds after you leave the island" }),
    ),
    h("div", { class: "row" },
      h("label", { text: "Island lives on" }),
      screen,
    ),
    h("div", { class: "row" },
      h("label", { text: "Launch at startup" }),
      toggle(settings.autostart, (v) => { settings.autostart = v; void save(); }),
    ),
  );
}

// ── Boot ──────────────────────────────────────────────────────────────────────

async function main() {
  const boot = await Bridge.boot();
  if (boot) {
    settings = { ...settings, ...boot.settings };
    version = boot.version;
  }
  const status = (await Bridge.hooksStatus()) ?? {
    installed: false, settingsPath: "", hookPath: "", hookReady: false,
  };

  const hasKey = (await Bridge.secretPresent("anthropic-api-key")) ?? false;

  const keys = [
    "stripe-api-key", "github-token", "vercel-token",
    "n8n-url", "n8n-api-key", "resend-api-key", "notion-api-key", "calcom-api-key",
  ];
  const present: Record<string, boolean> = {};
  for (const k of keys) present[k] = (await Bridge.secretPresent(k)) ?? false;

  clear(root);
  root.append(
    h("h1", {}, h("span", { text: "Coucou" }), h("span", { class: "version", text: version })),
    claudeSection(status),
    apiSection(hasKey),
    agentsSection(await Bridge.agentsStatus()),
    integrationsSection(present),
    generalSection(),
    h("div", {
      class: "hint",
      text: "No telemetry. Network requests only go to the services you configure yourself.",
    }),
  );

  void onEvent<Settings>("settings-changed", (s) => {
    settings = { ...settings, ...s };
  });
}

void main();
