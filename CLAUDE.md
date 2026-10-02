# Coucou — guide for AI coding agents

Coucou is a small animated character — Mochi — that lives at the top of the screen on
**Linux** (this fork's only platform). It shows Claude Code sessions plus OpenCode,
Hermes and Freebuff/Codebuff, and lets the user approve, answer, chat and drop files
from the island.

**This fork is Linux-only** (Linux Mint 22.3, Rust/Tauri + bun, X11 + Wayland).
Upstream's macOS/Windows sources were removed in favour of a clean single-platform
tree — see the sync rules below for how that trade-off is managed.

## Where things are
- `app/src-tauri/src/` — Rust backend: `socket.rs` (Unix-socket hook relay),
  `agents/` (AgentBus + OpenCode/Hermes/Freebuff adapters), `island/` (window +
  cursor/click-through, GTK), `hooks.rs` (settings.json installer), `secrets.rs`.
- `app/hook/` — `coucou-hook`, the Claude Code relay (Unix socket, SO_PEERCRED).
- `app/src/` — island front end (TypeScript, no framework; Canvas 2D Mochi).
- `app/src/island/agents.ts` — maps the `agent` event to agent pills. Without it the
  three adapters run, log, and stay invisible.
- `shared/sounds/` — the 28 WAVs, shared repo assets (path declared once in `app/vite.config.ts`).
- `docs/SPEC-linux-mult-agent.md` — the port + adapters spec. `docs/LINUX.md` — build, verify, sync.
- `docs/SPEC-agent-pills.md` — agent-pills design: v1 (the pills) **built**; health row deferred.
- `docs/SPEC.md`, `docs/INTEGRATIONS.md` — upstream behaviour spec (French, still useful for views/states).
- `design/prototype/notch-buddy.html` — original prototype, the visual source of truth.

## Build (Linux)
```
cd app && bun install && cargo build --release -p coucou-hook && bun run tauri build
cd app && cargo test -p coucou --lib && cargo test -p coucou-hook   # 15 tests
cd app && cargo clippy --all-targets                                # 0 warnings
./scripts/verify_coucou_linux.sh          # spec §6 checks, PASS/FAIL/SKIP
```

`cargo fmt` is not enforced on purpose: the tree uses a compact style (one-line
let-else, dense asserts) and rustfmt would rewrite ~100 lines in 16 files for no
functional gain. Fixing formatting is churn, not debt.

## Committing
Batch by intent, not by keystroke. A commit is worth making when it is a fix, a
deletion, or a finished change — never for a typo or a rename on its own; that is how
you get 10 000 commits nobody can bisect. Land a whole idea in one commit even if it
took several passes, and commit + push it yourself instead of asking the user to pull.

## Upstream sync (IMPORTANT — read before any multi-file change)
Upstream (`Louis-CFM/coucou`) moves weekly and this fork must keep merging it cheaply.
**Before starting an edit session, run `./scripts/sync_upstream.sh --check` and read
`SYNC-TODO.md`** — it lists the files where fork and upstream changes overlap and any WIP.
- Merge, never rebase. `--merge` auto-stashes tracked WIP and pops after.
- Conflicts land on the files both sides still have — `README.md`, `CHANGELOG.md`,
  `app/src/**`, `docs/**`. There, read the upstream commit for the intent, then keep
  the Linux side where the two disagree.
- Upstream's `NotchBuddy/` and `windows/` are deleted here on purpose: take the
  deletion when they collide, unless a change is something Linux needs (then mirror
  its intent into `app/`, like the macOS socket hardening that became part of
  `socket.rs`).
- `git stash` is only for uncommitted WIP; `sync_upstream.sh --merge` auto-stashes and pops.

## Staying ahead of upstream
Watch upstream PRs/issues (`gh pr list --repo Louis-CFM/coucou`). Fixes touching the
shared front end are adopted directly; Linux PRs conflict with our port — our Unix
logic wins, cherry-pick only what we lack.

## Rules
- Classify the work before writing it: **DEBT** (dead code, a lying doc, a stale dependency, a bug in an existing path — pay it down, deleting is a fix), **GAIN** (something broken or impossible today — build the minimum that works), **REVIEW** (someone else's change or your own assumption — read it, verify, then decide). Deletion beats addition: an addition must name what it replaces or which failure it prevents; deferred twice means delete.
- Then take the lazy-senior ladder (ponytail): does it need to exist? reuse what the codebase has, then stdlib, then platform, then an installed dependency, then one line, then the minimum that works. Never cut validation, error handling, security or accessibility to get there.
- Linux is the only platform: Rust + Tauri + bun. There is no macOS or Windows code left in the tree; do not reintroduce cfg(windows) branches.
- Secrets live in the Secret Service keyring, never on disk or in git. The Freebuff adapter's `tokenKey` is parsed and immediately dropped; it never reaches logs, state or UI.
- No telemetry. Network calls only to services the user configured; every agent adapter is loopback-only.
- Never block Claude Code: if the app doesn't answer, the hook exits immediately (300 ms connect, 2 s fire-and-forget, 110 s decision budget).
- Never overwrite `~/.claude/settings.json`: dated backup, merge, show the diff, write only after the user confirms.
- Never send an email or approve a Claude Code permission without an explicit click. Only Claude-Code events may carry `PermissionReq`; the other adapters are observe-only.
- Performance: 0 % CPU when the island is hidden. In the frame loop, write a DOM style only when its value changed — `applyGeometry()` and `updateBotTargets()` memoise their last write; on WebKitGTK each write invalidates style and layout.
- Keep the bundle identifier `fr.louisraille.coucou`.
- Visual changes must match the prototype and the screenshots in `design/captures/`.