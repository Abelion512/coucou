# Changelog

## Unreleased

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
