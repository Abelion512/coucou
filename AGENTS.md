# AGENTS.md — Coucou Linux

Conventions for every AI agent working in this repository (OpenCode, Codex,
Qoder, Codewhale, Swival, Claude Code — all of you). `CLAUDE.md` carries the
same rules; this file is the agent-neutral copy.

## What this repo is

**Coucou for Linux** — Mochi, an animated character at the top of the screen,
watching Claude Code sessions plus three more agents (OpenCode, Hermes,
Freebuff/Codebuff). Rust/Tauri + TypeScript, built with **bun**, shipped as
`.deb` + AppImage. **Linux-only**: there is no macOS or Windows code in this
tree, and reintroducing `cfg(windows)` branches is a mistake, not portability.

## Layout

| Path | What |
|---|---|
| `app/src-tauri/src/` | Rust backend — `socket.rs` (hook relay), `agents/` (AgentBus + 3 adapters), `island/` (GTK window/cursor), `hooks.rs`, `secrets.rs` |
| `app/hook/` | `coucou-hook` — the Claude Code relay binary (Unix socket, `SO_PEERCRED`) |
| `app/src/` | Island front end — TypeScript, no framework, Canvas 2D Mochi |
| `app/src/island/agents.ts` | `agent` event → agent pills. Delete it and the three adapters go silent |
| `shared/sounds/` | The 28 WAVs (path declared once in `app/vite.config.ts`) |
| `scripts/verify_coucou_linux.sh` | Spec §6 live verification (PASS/FAIL/SKIP) |
| `scripts/sync_upstream.sh` | Upstream merge tool; writes `SYNC-TODO.md` |
| `docs/LINUX.md` | Build, verify, upstream-sync policy, conflict map |
| `docs/SPEC-linux-mult-agent.md` | The port + adapters spec |
| `docs/SPEC-agent-pills.md` | Agent-pills design — v1 (pill) **built**; health row deferred |

## Build & verify

```bash
cd app && bun install && cargo build --release -p coucou-hook
TMPDIR=/tmp bun run tauri build                            # .deb + AppImage
cargo test -p coucou --lib && cargo test -p coucou-hook    # all must pass
cargo clippy --all-targets                                # 0 warnings is the bar
cd .. && ./scripts/verify_coucou_linux.sh --no-build
```

**Build the bundle before you call a task done.** Not just the checks — the
release build, and run the resulting binary. Half the bugs in this fork's history
only existed in the packaged app: the island was drawn under the desktop panel,
the window was too small for its own layout, and the AppImage was silent because
it bundles `libgstreamer` without a single plugin. None of those is visible to
`tsc`, to `cargo test`, or to a dev build. `TMPDIR=/tmp` is not optional inside a
sandbox — the default `TMPDIR` makes bundling fail with `os error 2`.

Then look at it. `scripts/ui_drive.sh` drives the running app with real X input
and measures what happened, which is the only way to catch click-through,
clipped panels and invisible headers — all of them window-manager behaviour that
no DOM assertion can see.

`cargo build -p coucou` is **not** a runnable app: `tauri-build` emits
`rustc-cfg=dev` for any non-release build, so the webview loads
`http://localhost:1420` and shows an empty island unless vite happens to be
running. Build release (`bunx tauri build --no-bundle` if you do not need the
deb and AppImage) when you intend to look at the thing.

**Per-language gates.** Rust: `cargo clippy` (clean) and `cargo test`. TypeScript:
`tsc --noEmit` under `strict` + `noUnusedLocals` (via `bun run build`). `cargo fmt` is
**not** enforced: the tree is written in a deliberately compact style (one-line
let-else, dense asserts) and `cargo fmt` would rewrite ~100 lines across 16 files for
no functional gain. Do not "fix the formatting" — that is churn, not debt.

## Committing

Batch by intent, not by keystroke. One commit for one idea; a typo, a renamed local,
or a two-line tweak riding along with real work goes in the same commit, not its own.
10 000 commits of typos is a history nobody can bisect.

- A commit is worth making when it is a **fix, a deletion, or a finished change**.
  A commit that only renames or rewraps code is not.
- Do not commit to look busy, and do not ask the user to pull work that is ready to
  land — commit and push it yourself.
- Land the whole idea in one commit even if it took several passes; a half-finished
  commit is worse than a slightly larger one.
- Keep the tree clean between tasks: if `git status` is dirty at the start of a new
  request, that is a leftover to finish first, not a base to build on.

## Non-negotiable rules

1. **Never block Claude Code.** The hook relay exits 0 immediately (300 ms
   connect budget) when the app is not there; a permission waits at most 110 s
   and then the terminal takes over. Any change that can make a Claude Code
   session wait on Coucou is wrong.
2. **`PermissionReq` is Claude-Code-only.** The OpenCode/Hermes/Freebuff
   adapters are observe-only — no write path, no decisions, ever.
3. **Secrets stay out.** Keys live in the Secret Service; the Freebuff
   `tokenKey` is parsed and dropped, never logged, stored, or emitted.
4. **Loopback-only.** Adapters talk to `127.0.0.1` and local Unix sockets.
   No cloud, no telemetry.
5. **0 % CPU when the island is hidden.** Pollers park on condvars; front-end
   animations pause via `#content.away`. In the frame loop, write a DOM style only
   when its value actually changed — `applyGeometry()` and `updateBotTargets()`
   memoise what they last wrote; on WebKitGTK every write invalidates style and
   layout.
6. **Hook installs are sacred**: dated backup of `~/.claude/settings.json`,
   show the diff, write only after an explicit click, refuse a stale preview.

## Style: the lazy-senior ladder (ponytail)

**Classify first — one word, no debate:**

| | what it is | the move |
|---|---|---|
| **DEBT** | dead code, a doc that lies, a stale dependency, a bug in an existing path | pay it down; **deleting is a fix** |
| **GAIN** | something is broken or impossible today without it | build the minimum that works |
| **REVIEW** | someone else's change, or one of your own assumptions | read it, verify it, then decide — never assume |

Deletion beats addition: an addition must name the thing it replaces or the
failure it prevents. Deferred twice is a delete candidate, not a TODO. Debt and
gain ship together when they touch the same file; a cleanup that only tidies is
wasted motion.

Then stop at the first rung that holds:

1. Does this need to exist? → no: skip it (YAGNI)
2. Already in this codebase? → reuse it
3. Stdlib does it? → use it
4. Native platform feature? → use it
5. Installed dependency? → use it
6. One line? → one line
7. Only then: the minimum that works

Never cut validation, error handling, security, or accessibility to get there.
Lazy about the solution, never about reading: trace the real flow first.

## Upstream sync (Louis-CFM/coucou)

- **Before any multi-file change**: `./scripts/sync_upstream.sh --check`,
  then read `SYNC-TODO.md`. It lists overlap files and WIP.
- Merge, never rebase. `--merge` auto-stashes tracked WIP and pops after.
- This fork deleted `NotchBuddy/` and `windows/`: upstream edits there resolve
  as deleted-by-us — **take the deletion**, unless the change is something
  Linux needs (then mirror its intent into `app/`, like the macOS socket
  hardening that became part of `socket.rs`).
- Watch upstream PRs/issues (`gh pr list --repo Louis-CFM/coucou`). Front-end
  fixes are adopted directly (done: #43, #56); upstream Linux PRs (#42, #44)
  lose to our implementation — cherry-pick only what we lack.
