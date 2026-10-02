# Changelog

## Unreleased

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
