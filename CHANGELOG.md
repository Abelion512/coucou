# Changelog

## Unreleased

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
