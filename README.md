<div align="center">

<img src="app/src-tauri/icons/128x128.png" width="96" alt="Coucou icon">

# Coucou for Linux

**A tiny friend that lives at the top of your Linux screen and keeps an eye on your Claude Code sessions — plus OpenCode, Hermes and Freebuff/Codebuff.**

Approve permissions, watch your agents work, drop a file, chat with Claude — all without leaving what you're doing.

![Linux Mint 22+](https://img.shields.io/badge/Linux%20Mint-22%2B-87CF3E?logo=linuxmint&logoColor=white)
![X11 / Wayland](https://img.shields.io/badge/X11%20·%20Wayland-both-blue)
![Tauri 2](https://img.shields.io/badge/Tauri-2-FFC131?logo=tauri&logoColor=black)
![Rust](https://img.shields.io/badge/Rust-backend-000?logo=rust&logoColor=white)
![Bun](https://img.shields.io/badge/bun-frontend-f472b6?logo=bun&logoColor=white)
![License: MIT](https://img.shields.io/badge/license-MIT-green)

<img src="docs/media/demo.gif" width="760" alt="Coucou in action">

</div>

---

## Why

Some studios showed off gorgeous notch companions… and never let anyone use them.
This fork brings the idea to Linux.
**Coucou is the open version.** Every line of code, every animation, every sound — free to use, read, fork and remix.

Meet **Mochi**: a soft little squircle with big eyes that pops out of the top edge of your screen, waves hello, follows your cursor with its eyes, gets annoyed when you poke it (and dizzy if you insist), and tells you the moment Claude Code needs you.

## Features

- 🤖 **Claude Code, live** — see every session at the top of your screen: what it reads, edits and runs, step by step. Finished? Mochi does a happy little jump.
- ✅ **Approve from the island** — Claude Code permission requests show up with **Allow / Deny**. One click, back to work.
- 💬 **Ask Claude anything** — built-in chat, straight from the island.
- 📎 **Drop a file on the island** — Mochi turns into a box and swallows it, then you can ask a question about it.
- 🔌 **Integrations** — Stripe payments, n8n workflows, GitHub, Vercel deployments, Resend emails, Notion, Cal.com. Each one gets its own little colored Mochi.
- 🎭 **A real character** — idle breathing, blinks, eyes that follow your mouse, emotes, 28 handcrafted sounds, a greeting on launch.
- 🫥 **Invisible when idle** — hides away when nothing is running, peeks out when you hover the top edge of the screen.
- 🤖 **Multi-agent** — watches OpenCode, Hermes and Freebuff/Codebuff alongside Claude Code, each in its own pill.
- 🔒 **Private by design** — no telemetry, no account. Keys live in the Linux Secret Service (GNOME Keyring / KWallet). The app only talks to the services you plug in.

<table>
<tr>
<td><img src="docs/media/claude-code.png" alt="Claude Code session"></td>
<td><img src="docs/media/stripe.png" alt="Stripe payments"></td>
</tr>
<tr>
<td><img src="docs/media/chat.png" alt="Chat with Claude"></td>
<td><img src="docs/media/dizzy.png" alt="Too many hits"></td>
</tr>
</table>

## Install

### Linux (this fork's platform)

This fork is **Linux-only**: the Tauri app lives in `app/`, builds with bun + Rust,
and ships as a `.deb` and an AppImage. Install, build and verification are in
[`docs/LINUX.md`](docs/LINUX.md).

### Build from source (Linux)

Requirements: [Rust](https://rustup.rs), [bun](https://bun.sh), WebKitGTK 4.1 and GTK 3 dev packages.

```bash
git clone https://github.com/Abelion512/coucou.git
cd coucou/app
bun install
cargo build --release -p coucou-hook
bun run tauri build        # .deb + AppImage land in app/target/release/bundle/
```

## Setup

Click the Coucou icon in the system tray → **Settings…**

| What | Why | Where the key goes |
|---|---|---|
| **Claude Code hooks** | live sessions and approvals | **Install hooks** — Coucou backs up `~/.claude/settings.json`, merges its hooks and shows you the diff before writing anything |
| **Anthropic API key / API base** | chat and questions about files | Linux Secret Service (GNOME Keyring / KWallet). Point **API base** at any Messages-compatible relay (LiteLLM, a gateway, a Chinese model relay) to run custom models — with a relay the key is optional |
| Stripe, n8n, GitHub, Vercel, Resend, Notion, Cal.com | the integration pills | Secret Service, all optional |

If Coucou isn't running, the hook exits immediately: **Claude Code is never blocked.**

## Things to try

| Do this | Mochi does that |
|---|---|
| Hover the top edge of the screen | peeks out and says hi 👋 |
| Click it | opens |
| Hover Mochi | blinks, eyes grow |
| Click Mochi | squish + annoyed |
| Click 3 times fast | 😵‍💫 dizzy for a few seconds |
| Drag a file onto the island | turns into a box and swallows it |

## How it works

- A [Tauri 2](https://tauri.app) app (Rust + TypeScript): the island is a transparent, always-on-top window that never steals focus; Mochi is drawn in Canvas 2D.
- Claude Code hooks go through a tiny `coucou-hook` relay and a per-user Unix socket
  (`$XDG_RUNTIME_DIR/coucou/coucou.sock`, `SO_PEERCRED`-checked). If the app doesn't answer
  within 300 ms the relay exits: **Claude Code is never blocked.**
- The other agents are watched by three observe-only adapters: OpenCode (SSE), Hermes
  (gateway control socket), Freebuff/Codebuff (manicode file watching).
- Integrations are lightweight pollers, paused when nothing is watching.
- Sounds: 28 short WAVs served from `shared/sounds/`.

## Contributing

Issues and PRs are very welcome — new integrations, new emotes, new sounds, bug fixes. See [CONTRIBUTING.md](CONTRIBUTING.md).

## Credits

Built by [Louis Raillé](https://louisraille.fr) with Claude Code.
Built by [Louis Raillé](https://louisraille.fr) with Claude Code; Linux port and multi-agent fork maintained here.
Inspired by the notch-companion concepts shared by design studios — this project is independent and not affiliated with any of them.

## License

- **Code:** [MIT](LICENSE) — use it, fork it, learn from it, just keep the copyright notice.
- **Name, Mochi character, icon, sounds and media:** © Louis Raillé, all rights reserved — see [LICENSE-ASSETS.md](LICENSE-ASSETS.md). Shipping your own fork? Give it your own name and character.

## Upstream sync

This fork tracks [Louis-CFM/coucou](https://github.com/Louis-CFM/coucou) weekly:

- `./scripts/sync_upstream.sh --check` writes a `SYNC-TODO.md` you (or your AI agent)
  read before editing — it lists the files where fork and upstream changes overlap.
- `--merge` merges `upstream/main` with auto-stash when needed. Merge, never rebase.
- The playbook for staying a step ahead is in [`docs/LINUX.md`](docs/LINUX.md).

<div align="center">

**If Mochi made you smile, a ⭐ helps a lot.**

[Website](https://louis-cfm.github.io/coucou/) · [Privacy](https://louis-cfm.github.io/coucou/privacy.html) · [Terms](https://louis-cfm.github.io/coucou/terms.html) · [Support](https://louis-cfm.github.io/coucou/support.html)

</div>
