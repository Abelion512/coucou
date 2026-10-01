# Coucou — guide for AI coding agents

Coucou is a small animated character — Mochi — that lives in the MacBook notch (macOS) or at
the top of the screen (Windows, **Linux**). It shows Claude Code sessions and a few
integrations, and lets the user approve, answer, chat and drop files from the island.

**This fork is maintained Linux-first** (Linux Mint 22.3, Rust/Tauri port + 3 agent adapters).
The platform target for new features here is Linux. See `docs/LINUX.md`.

## Where things are
- **Linux (the fork's own work)**: `windows/src-tauri/src/socket.rs` (Unix-socket hook relay),
  `windows/src-tauri/src/agents/` (AgentBus + OpenCode/Hermes/Freebuff adapters),
  `windows/src-tauri/src/island/unix.rs`, `windows/hook/src/unix.rs`.
- `windows/` — the Tauri app shared by Windows and Linux; `NotchBuddy/Sources/App/` — all macOS Swift code.
- `docs/SPEC.md`, `docs/INTEGRATIONS.md` — behaviour, views, states, integrations (in French).
  `docs/LINUX.md`, `docs/SPEC-linux-mult-agent.md` — the Linux fork's docs.
- `design/prototype/notch-buddy.html` — original prototype, the visual source of truth. `design/captures/` — target screenshots.
- `docs/*.html` — the GitHub Pages site (privacy, terms, support, legal notice).

## Build (Linux)
```
cd windows && npm install && cargo build --release -p coucou-hook && npm run tauri build
./scripts/verify_coucou_linux.sh          # spec §6 checks, PASS/FAIL/SKIP
```

## Upstream sync (IMPORTANT — read before any multi-file change)
Upstream (`Louis-CFM/coucou`) moves weekly and this fork must keep merging it cheaply.
**Before starting an edit session, run `./scripts/sync_upstream.sh --check` and read
`SYNC-TODO.md`** — it lists the files where fork and upstream changes overlap and any WIP.
- Merge, never rebase. Conflicts live almost only in `windows/**`; there, keep the Linux
  side but mirror upstream fixes (read their commit for the intent).
- Do not delete `NotchBuddy/` or the Win32 parts of `windows/`: they don't compile on Linux
  and deleting them turns every upstream touch into a deleted-vs-modified conflict, forever.
- `git stash` is only for uncommitted WIP; `sync_upstream.sh --merge` auto-stashes and pops.

## Rules
- Before writing code, take the lazy-senior ladder (ponytail): does it need to exist? reuse what the codebase has, then stdlib, then platform, then an installed dependency, then one line, then the minimum that works. Never cut validation, error handling, security or accessibility to get there.
- Linux is the target platform: Rust + Tauri for the app; keep macOS (`SwiftUI + AppKit`, Canvas/TimelineView) and Windows code paths compiling untouched when the file is shared.
- Secrets live in the Keychain / Credential Manager / Secret Service, never on disk or in git. The Freebuff adapter's `tokenKey` is parsed and immediately dropped; it never reaches logs, state or UI.
- No telemetry. Network calls only to services the user configured; every agent adapter is loopback-only.
- Never block Claude Code: if the app doesn't answer, the hook exits immediately (300 ms connect, 2 s fire-and-forget, 110 s decision budget).
- Never overwrite `~/.claude/settings.json`: dated backup, merge, show the diff, write only after the user confirms.
- Never send an email or approve a Claude Code permission without an explicit click. Only Claude-Code events may carry `PermissionReq`; the other adapters are observe-only.
- Performance: 0 % CPU when the island is hidden.
- Keep the bundle identifiers (`fr.louisraille.NotchBuddy` on macOS, `fr.louisraille.coucou` on Windows/Linux).
- Visual changes must match the prototype and the screenshots in `design/captures/`.
