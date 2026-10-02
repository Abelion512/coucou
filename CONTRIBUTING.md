# Contributing to Coucou (Linux)

Thanks for wanting to help Mochi grow up! 🫶

This is the **Linux fork**: Rust + Tauri, built with bun, shipped as `.deb` and
AppImage. There is no Swift here — the macOS app is upstream and lives elsewhere.
See [`docs/LINUX.md`](docs/LINUX.md) for the full build and the upstream-sync story.

## Getting started

```bash
sudo apt install libwebkit2gtk-4.1-dev build-essential libssl-dev libgtk-3-dev \
  libayatana-appindicator3-dev librsvg2-dev pkg-config
cd app && bun install && cargo build --release -p coucou-hook && bun run tauri build
```

Front end only, with hot reload, against a running app is not supported — `bun run dev`
serves the island at <http://localhost:1420>, which is enough to iterate on views:

```bash
cd app && bun run dev
```

## Before you finish anything

```bash
cd app && bun run build              # tsc --noEmit, strict + noUnusedLocals
cargo test -p coucou --lib && cargo test -p coucou-hook
cargo clippy --all-targets          # zero warnings is the bar
./scripts/verify_coucou_linux.sh --no-build
./scripts/auto_ponytail.sh          # DEBT snapshot: dead modules, docs that lie
```

`cargo fmt` is deliberately **not** enforced — the tree is written in a compact style
and reformatting it would churn ~100 lines for no functional gain.

## Good first contributions

- A new integration (a poller + a pill + a detail card). `app/src-tauri/src/integrations.rs` is the compact example.
- A new adapter in `app/src-tauri/src/agents/` — observe-only, loopback only, and it must emit an existing `AgentEvent` rather than a new shape.
- A new emote or sound for Mochi.
- Bug fixes — please describe how to reproduce, and say which build you saw it in
  (`.deb` and AppImage have genuinely differed before; see `docs/LINUX.md`).

## Rules of the house

- Rust + TypeScript, **no third-party dependencies** unless there is really no other way.
- Secrets go in the Secret Service keyring, never on disk or in git. The Freebuff `tokenKey` is parsed and dropped.
- No telemetry, no network calls except to services the user configured, all on loopback.
- Never block Claude Code: if the app doesn't answer, the hook must exit right away (300 ms).
- Only Claude Code may be answered from the island. The other agents are observe-only.
- Never write `~/.claude/settings.json` without a dated backup and the user's confirmation.
- Keep it light: 0 % CPU when the island is hidden.

## Pull requests

- One topic per PR, with a short GIF or screenshot for anything visual.
- Build must pass with no new warnings.
