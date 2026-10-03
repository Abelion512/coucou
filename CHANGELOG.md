# Changelog

Two streams. **Unreleased** is this fork's Linux work, which has not shipped yet.
Everything below the rule is upstream's own macOS history, kept so a future
`sync_upstream.sh` has a merge base — none of it describes a build you can install
from this repository.

## Unreleased

- **Answer Claude's questions from the island.** When Claude Code asks a
  multiple-choice question, pick an option or type your own answer right in the
  notch — single or multi-select, up to four questions, with a 1/N counter and
  an "Other…" field for anything not on the list. "Reply in terminal" hands it
  back to the CLI. This needs the updated hooks: Settings → Claude Code Hooks
  re-installs them (a second `PreToolUse` entry with an `AskUserQuestion`
  matcher, a 130 s timeout, and a relay that carries the answer back as
  `updatedInput`). Claude Code 2.1.85+ sends the question on `PreToolUse`;
  older versions still ask in the terminal
- **The Mochi on the left wears the agent's colour.** Focusing an OpenCode,
  Hermes or Freebuff pill used to leave it the default white body, because the
  island only tinted itself for integration pills — the three adapters this fork
  exists for were the one case it skipped. It now follows whatever pill is
  focused, the same way the pill's own mini Mochi already did
- **The model list is what you use, not what the relay sells.** A relay like 9router
  offers 1013 models; rendering them in a dropdown is unreadable and slow. Typed model
  ids are remembered (newest first, twelve deep) and the dropdown is the Claude
  defaults plus those. The relay is asked periodically which of *your* ids are still
  there — "Relay lists 1013 models — all 2 of yours are on it" — and a model it has
  dropped is marked rather than silently removed, because a typo in a config is more
  likely than a deliberate deletion. The check arms only after Settings has been
  opened once: that window is created hidden and never destroyed, so a timer running
  from launch would be exactly the background work the 0 %-when-hidden rule forbids
- **Upstream merged** (0.1.1 notarized, the Linux beta announcement). The macOS tree
  came back with it and the deletions are taken again. Four Linux fixes adopted: GTK
  never shrinks a non-resizable window below its natural size, so the 6 px wake strip
  was a 200 px band that swallowed clicks (upstream #44); a view that is not on is
  transparent but not gone, so its animations kept the webview repainting for as long
  as the app ran; and the island is created unfocusable, so GTK refused the chat
  field's focus request. The cursor-poll backoff upstream gates behind a compile-time
  flag — this tree measures it instead, recovering on its own, which also covers X11
  sessions where the query merely fails. Not taken: `refresh_click_through` (a
  no-cursor-poll fallback; ours works on X11) and the macOS feature list

---

## 0.1.3 — October 3, 2026

- Answer Claude's questions from the notch: when Claude Code asks a multiple-choice question, pick an option or type your own answer right in the island, and Reply in terminal hands it back. Update your hooks in Settings to turn it on (#165) — thanks @Vega8991 for the idea (#94)
- Claude plan usage (GitHub build): turn on Settings → Agents → Plan usage to see your 5-hour and weekly limits in a small pill in the notch header, and click it for the details and reset times. Pro and Max plans; your current status line keeps working (#159)
- Chat with local models through Ollama or LM Studio, no API key needed: connect them in Settings → Chat → Local models. Answers stream in, and thinking blocks stay hidden (#156)
- Markdown in chat answers: bold, lists, headings, quotes, and code blocks with a copy button. Links open only when they are web links (#156)
- Apple Music (GitHub build): see what is playing in the notch, play, pause and skip on hover, and Mochi dances along (#144, #153)
- Settings are now organized in a sidebar (#153)
- The chat greets you by your own first name (#154)

## 0.1.2 — October 2, 2026

- Codex support (GitHub build): sessions show up live on the Codex pill, and permission requests get Allow and Deny in the notch. Install from Settings → Codex Hooks, then trust the hooks once with /hooks in Codex (#130) — thanks @lacatu5
- Cursor: Claude Code started in Cursor's terminal shows up on the Cursor pill, and you can answer its permission requests from the notch (#120).
- Pick your main coding tool in Settings → Active pills: VS Code, Cursor, Codex or Antigravity (Codex and Antigravity: GitHub build). It stays on and no longer takes one of the 4 slots (#120).
- The permission card stays in the notch until you answer it: the mouse no longer folds it, and reopening the island shows the request again (#117).
- The permission card also shows when the island is already open, and the pill you were on comes back once you answer (#120).

## 0.1.1 — October 2, 2026

- **Each agent now shows its own session, and nothing leaks between them.** OpenCode's
  session poll was sending a `Step`, and only `SessionStart` puts a session's own title
  on a pill — so a freshly started Coucou showed "OpenCode server" until the next
  session arrived over SSE. `/session` is newest-first, so the first entry is the
  session you are actually in, and it is now announced as a `SessionStart` once per
  change rather than every 10 s. Hermes had no session at all: its gateway has no
  sessions verb (only `identify` and `status`), so the label comes from
  `~/.hermes/state.db` via the `sqlite3` client in read-only mode, once every 30 s.
  Shelling out beats linking SQLite into a binary the hook relay spawns on every
  Claude Code event; if `sqlite3` or the database is missing the adapter reports no
  label rather than a wrong one. Verified on screen: Freebuff shows `m-22ff70c712`,
  Hermes `20261001_1640…`, OpenCode `ses_f03b56ba…`, all three at once, while a
  Claude hook event lands only on the Claude pill
- **ui_drive.sh now measures the island's own churn.** With live agents the ticker
  animates and the pills update every few seconds, so a byte comparison reported a
  change for a click that never arrived. Both the control and the positive check now
  take a baseline of how much the island changes while nobody touches it, and compare
  against it: a click in the transparent margin moves it 0.19 % against a 0.18 %
  baseline, a real click moves it 40 %. A test that cannot tell "clicked" from
  "not clicked" is not a test, and this one had quietly stopped being able to
- **Clicks fell through to the window behind, and pausing made it worse.** The
  click-through test asked GTK where the window is: `Window::outer_position` answered
  y=11 for a window sitting at y=45 — off by the Cinnamon panel — and
  `Window::scale_factor` answered 1.0 on a 1.25× display, so cursor coordinates came
  out 1.25× too large. The hit test therefore disagreed with the island by tens of
  pixels, `on_island` was false even with the mouse on it, and the island never took
  the click. It now uses the position and scale **it applied itself** — the window is
  not user-movable, so that is the position it has. Both of these are the same
  unreliability that clipped the window in the first place, which is why it is now
  written down once instead of being rediscovered per call site
- `scripts/ui_drive.sh` — drives the running island with real X input and proves
  what happened: hover wakes it, a click on it lands and makes it redraw, a click in
  its transparent margin does not, and idling on it for 8 s does not break it. The
  margin check is a deliberate control: a checker that cannot tell "clicked" from
  "not clicked" is not a checker. It reads nothing from the app's internals — window
  geometry, X focus and screenshots — because the bug it guards is in window-manager
  behaviour, which a DOM assertion cannot see
- **The island was drawing underneath the desktop panel.** It was placed at the
  monitor's `y`, which on Cinnamon is exactly where a notch would be — the top edge,
  where the clock and notification centre live. The island was invisible, and the
  sliver of it that peeked below the panel hid the header, so home / chat / upload /
  sound / settings looked like missing buttons. It now positions against the monitor's
  **work area**, which is the monitor minus whatever the desktop reserves, so it lands
  just under the panel on Cinnamon, GNOME and anything else without per-DE config.
  Verified: the five header icons render, and the island opens on hover + click —
  the tray was only ever a fallback, never the required way in
- **The viewport correction undid itself.** Deriving the scale factor from
  *requested ÷ measured* made the webview's second report read the corrected size and
  slide the factor back to 1.0, re-clipping the island after one frame of looking
  right. It is now derived from *physical ÷ CSS* — what the window was, against what
  the webview says it got — which is self-correcting instead of self-defeating
- **Agent status in Settings.** The three adapters connect, log, and produce a pill
  only while running — which is correct, and also the only symptom when one is
  broken. Settings now has an "Agents" section saying which are live, what their
  loopback address is, and the command to start the ones that are not. Nothing polls
  there: the settings window is created at startup and never destroyed, so a timer
  would run forever behind a closed window — it reads once and follows the `agent`
  event
- Hermes' gateway reports `platforms.api_server.listener_base` on every 5 s poll and
  the adapter threw it away, leaving "where is Hermes" answerable only from a comment in
  a spec. It is kept now, and logged once when it changes. Hermes' and Freebuff's
  `healthy()` also told the truth about a file rather than about the gateway: Hermes
  now reports whether the gateway *answered*, Freebuff reuses what its poll already
  computed instead of re-scanning the directory
- **auto-ponytail**: `scripts/auto_ponytail.sh` — our own DEBT sweep. Imports nothing,
  doc paths that do not exist, scripts no doc mentions, deferred markers, and
  `#[allow(dead_code)]` claims. ~1 s, offline, no dependencies. Its false positives are
  documented in its own output rather than hidden. On first run it found two docs that
  lied — `CONTRIBUTING.md` still telling Linux contributors to run `xcodegen` on
  `NotchBuddy.xcodeproj` and to follow "Swift 6, SwiftUI + AppKit", and a spec pointing
  at a preview file that no longer exists — both now written correctly
- `CONTRIBUTING.md` was upstream's macOS document verbatim: xcodegen, NotchBuddy,
  `StripePoller.swift`, "Swift 6, SwiftUI + AppKit", and a screen-geometry script this
  fork does not have. Rewritten for this fork's actual build, gates and guardrails
- Deleted `Agent::id()`, which duplicated what serde already emits, and the dead
  `AgentSource` trait that had zero implementations. Two comments in `freebuff.rs`
  promised a `pgrep` and an inotify watcher that were never written; the code polls,
  and now says so
- **Three agent pills now exist.** `app/src/island/agents.ts` was never written, so the
  `agent` event Rust emitted had no listener: OpenCode, Hermes and Freebuff connected,
  wrote to the log, and were invisible. Now one pill per agent, appearing only while the
  agent is actually running, and disappearing when it stops. Observed live — the pills
  show up within a second of the adapters connecting
- **The island was clipped on scaled displays.** The window was sized in physical pixels
  from GTK's reported scale factor, but on a 1.25× display both `Monitor::scale_factor`
  and `Window::scale_factor` reported 1.0, so the 720 px panel became a 576 px viewport
  and the 640 px island lost 64 px off both edges. The webview now reports the CSS size
  it actually got (`reportViewport`) and Rust sizes the window from that — WebKit is the
  only party that knows the real factor, because it is the one dividing pixels
- **Sounds were silent in the AppImage only.** It bundles `libgstreamer` but none of its
  plugins, so WebKitGTK logged "GStreamer element autoaudiosink not found" and the 28
  sounds never played — while dev was fine, which is why this hid. The AppImage now points
  at the system's plugins (and keeps its own registry file, since it is remounted at a new
  path every launch). Covered by a test
- Integration buttons now link straight to each platform's API-key page instead of its
  front page — "Key not configured" was a dead end otherwise. The tray item reads
  **Settings** (matching the menu) rather than "Settings…"
- OpenCode's session poll reported every session, refilling the pill's ticker from the top
  every 10 s. It now reports the most recent one, which is all a one-pill-per-agent design
  can show
- The e2e harness was reporting passes it had not earned. Three of the ten checks
  could go green while verifying nothing, and one was simply broken:
  - **§5 was a real FAIL.** `socket_send`'s python branch sent the payload raw while
    its `nc` branch appended a newline — and the newline is the gateway's frame
    delimiter. The Hermes gateway waits for a complete line and, given none, answers
    nothing at all after ~2 s, so a client that forgets it is indistinguishable from
    a dead gateway. `hermes.rs` already sent `\n`, which is why the app worked and
    only the script failed
  - **§5 then passed vacuously.** The gateway serialises with `", "` / `": "`, so the
    `"gateway_state":"…"` pattern never matched and the extraction silently yielded
    `gateway_state= active_agents=?` — reported as PASS. It now tolerates the space,
    and a reply it can grep but not parse fails instead of passing
  - **§3 tested a dead app.** A crashed Coucou leaves its socket inode behind, and
    `-S` matched it, so "testing against the running app" fired against nothing and
    passed because the hook correctly exited 0. Liveness is now a real connect probe,
    and a stale socket falls through to the throwaway-listener branch — which checks
    the documented `PermissionRequest` round-trip, a stronger test than the one it
    replaces
  - Deleted a provably dead line in §7: `grep -q` emits no stdout, so piping it into
    `grep -vq '^Binary'` could never take the branch. The real leak check beside it
    still fires (verified with a seeded `tokenKey`)
- Upstream merged (9 commits). Upstream now ports Linux inside its own `windows/`
  tree — a Unix-socket relay, a `platform/` module with a gtk-layer-shell layer and
  its own `linux.yml` — so the merge paired those files with our `app/` paths by
  content. We took the deletion for `NotchBuddy/**` and `windows/**` and ours for
  every `app/**` file. Taken from them: CI now builds and tests pull requests
  touching `app/**` (their #89 fix), and installs the GStreamer plugins without
  which an AppImage has no audio. Worth mirroring later, with the Rust side: on
  Wayland the island should take the cursor from page events instead of the poll
- `cargo clippy --all-targets` is clean (12 warnings fixed, mostly `to_string()` on
  `&str`, two hand-built C strings, one range loop). Documented as a per-language
  gate, with `cargo fmt` explicitly *not* enforced: the compact style would gain
  nothing from ~100 reformatted lines
- `AGENTS.md`/`CLAUDE.md` now say how to commit: batch by intent, a commit is worth
  making for a fix, a deletion or a finished change — never for a typo, and land it
  yourself instead of asking the user to pull
- `docs/LINUX.md` documents how to test the three agents end to end on a laptop
  (what each needs running) and what is deliberately still pending, plus the answer to the
  question that comes up first: nothing has to be started before using Coucou, and the
  extra pills only exist while their agent is running
- `docs/SPEC-linux-mult-agent.md` no longer describes a machine that isn't this one, and
  its paths point at `app/` rather than the deleted `windows/` tree; §6 verification is
  the `./scripts/verify_coucou_linux.sh` that actually exists instead of a hand-typed
  command list that had drifted into nonsense

- Frame loop no longer writes DOM styles it already wrote: `applyGeometry()` and
  `updateBotTargets()` memoise their last values, and the drop-canvas class toggles
  only flip on a change. On WebKitGTK every style write invalidates style and layout,
  so the idle island was doing ~8 pointless writes plus a fresh glow gradient per
  frame; `#island` also drops its permanent `will-change` layer for the same reason
  the ticker rows did (#43)
- Docs: the ponytail ladder now starts with a classification step — **DEBT** (pay it
  down, deleting is a fix), **GAIN** (build the minimum that works), **REVIEW** (read
  it, verify, then decide) — with deletion explicitly beating addition; `CLAUDE.md`'s
  duplicated, partly stale upstream-sync section is merged into one; `docs/LINUX.md`
  gains a performance section (how the 0 %-when-hidden rule is kept) and a list of
  what is deliberately still pending

- Cleanup pass, no new features: the dead `cfg(windows)` dependency block (`windows`
  crate, `keyring` Windows backend), the `windows_subsystem` attribute and the
  `.exe` branch in the hook installer are gone from a Linux-only tree; the Claude
  Code pill is Claude orange `#D97757` and Resend is no longer an integration on by
  default (Notion is)
- Dead code removed rather than kept "for later": `colorForProject` + its project
  colour table, `STATE_SOUND`, `GREETING_END`, `WAKE_STRIP_W/H`,
  `TOGGLEABLE_INTEGRATION_IDS`, `miniBotCount`, `releaseMiniBot`
- Fixed the task ordering comparator, which returned `-1` for `integration_claude`
  against itself; pills order is now Claude Code, agent pills, integrations, unknown
  ids last
- `CLAUDE.md` no longer tells agents to keep `NotchBuddy/`/`windows/` alive, and
  describes keys as Secret Service instead of Keychain/Credential Manager

- Chat can run on custom models: a configurable **API base** (any Messages-compatible
  relay — LiteLLM, a gateway, a Chinese model relay) makes the Anthropic API key
  optional, and a **Custom…** model field accepts any model id a relay exposes
- "Open terminal"/editor launcher chain now tries Zed, Kate and gedit beyond VS Code
  builds, so editor-less setups (Antigravity, plain text editors) still get a useful
  fallback; the Claude Code pill is labelled "Claude Code", not "VS Code"

- Settings window no longer claims Windows-era things on Linux: the missing-relay
  warning says `coucou-hook` (no `.exe`) and key storage is described as the Secret
  Service keyring; stale Windows/macOS comments swept from src-tauri and the front end

- npm → bun for the front end (`bun.lock`, `bunx tauri`, `bun run build`)
- Linux CI workflow (bun + cargo tests, .deb/AppImage on `linux-v*` tags), ahead of the
  upstream Linux PRs (#42, #44)
- OpenCode adapter upgraded to the verified SSE vocabulary from upstream PR #47:
  session.created/idle/error/deleted, tool.execute.before/after, permission.asked shown
  as observe-only notification
- Frontend fixes adopted from upstream PRs before they merge: #43 (ticker rows overlap
  on WebKitGTK — will-change removed, block layout, half-finished steps hold the frame
  loop) and #56 (all view animations pause while the island is folded → 0 % CPU)
- Linux-first maintenance docs: `docs/LINUX.md` (build on Mint 22.3, upstream-sync policy,
  conflict map) and `scripts/sync_upstream.sh` — analyses each sync and writes `SYNC-TODO.md`
  (overlap files, WIP warning, ported-fix candidates) so editing and merging stay cheap
- Windows/Linux hardening mirrors upstream 1f6e0d0: 0700 socket dir, 32-connection ceiling,
  5 s receive timeout, 0600 log files
- Linux port of the Tauri app (Unix-socket hook relay with SO_PEERCRED, XDG paths,
  keyring Secret Service) plus three observe-only agent adapters: OpenCode (SSE),
  Hermes (gateway control socket), Freebuff/Codebuff (manicode file watching)
- `scripts/verify_coucou_linux.sh` — spec §6 live verification, PASS/FAIL/SKIP
- Compact island on screens without a notch (#22) — thanks @Kamasoutra
- Only web links (http/https) open from the notch; other kinds of links from Claude or integrations are ignored (#16) — thanks @Cris1670
- Hook socket limited to your own user account, with size and time limits; logs no longer keep commands, n8n data or full URLs, and stay under 1 MB (#16) — thanks @Cris1670 and @Vignesh-Thangamariappan
- The island always reopens after folding, and Settings opens below it, resizable — thanks @rouderz
- Choose the Claude model for the chat in Settings — official models or a **custom model id** behind any Messages-compatible relay (see the API base entry above); `claude` stays reserved as the Claude Code pill name
- Any agent can talk to Mochi: tag a hook payload with `coucou_agent` (e.g. `coucou-hook --agent my-agent`) and it gets its own pill in the island (#7, #9) — thanks @lacatu5; documented for Linux in `docs/AGENTS.md`
