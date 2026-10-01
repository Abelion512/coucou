# SPEC: Coucou Multi-Agent — Rust/Tauri Linux Port + 3 Adapter Baru

> Hasil audit live mesin Abelion (2026-10-01). Dikirim ke agent coding di cloud
> sebagai dasar implementasi. Sumber asli: audit VERIFIED pada mesin
> `abelion@mint223`, transkrip di cache Hermes lokal.

**Dasar:** adopsi `Louis-CFM/coucou`. Semua fitur bawaan Coucou (Mochi UI,
Claude-Code monitoring, permission approve/deny, 28 sounds, animations,
integrations) **dipertahankan apa adanya**. Yang ditambahkan hanya:

1. **Port Tauri Windows → Linux native**
2. **3 adapter agent baru**: opencode, freebuff/codebuff, hermes

**Tidak ada perubahan** ke macOS Swift. Tidak ada rewrite dari nol.

---

## 1. Fakta Audit (VERIFIED di mesin ini, 2026-10-01)

### 1.1 Claude-Code — bawaan Coucou, TIDAK diubah
Coucou sudah punya `nb-hook` (Python relay) → Unix socket `nb.sock` → `HookServer.swift`.
Event: SessionStart, SessionEnd, UserPromptSubmit, PreToolUse, PostToolUse,
PostToolUseFailure, PermissionRequest, Notification, Stop, StopFailure,
SubagentStart, SubagentStop.
Timeout 10s, PermissionRequest 120s (hold FD sampai user jawab).

### 1.2 OpenCode 1.18.34 — **adapter paling bersih**
```bash
opencode serve --port userspace 54321
# Output: "opencode server listening on http://127.0.0.1:54321"
# Warning: OPENCODE_SERVER_PASSWORD is not set; server is unsecured.
```

| Endpoint | Status (audit) | Isi |
|---|---|---|
| `GET /event` | **200** | SSE stream. Verified events: `{"type":"server.connected"}`, `{"type":"server.heartbeat" |
| `GET /session` | **200** | JSON array contoh: `[{"id":"ses_f50a...","slug":"stellar-squid","directory":"/media/abelion/Wave/.hermes/hermes-agent","title":"Rencana audit Hermes self healing",...}]` |
| `GET /global/health` | **200** | health check |
| `GET /config` | **Global** | config JSON |

Catatan: server loopback-only by default (`--hostname 127.0.0.1`).
Adapter perlu **mulai opencode serve** atau detect yang sudah jalan.

### 1.3 Hermes v0.21.5 — **control socket, sudah live**
Socket: `~/.hermes/gateway.sock` (srw-------, **ada sekarang**)
Protokol: 1 JSON line in → 1 JSON line out → server close.

**Verb tersedia (VERIFIED via audit):**
- `identify` → `{"ok":true,"protocol":1,"result":{"protocol":1,"kind":"hermes-gateway","pid":728390,"start_time":3904393,"hermes_home":"/home/abelion/.hermes","profile":"default","supervisor":"systemd","code_sha":"f42f579cf8bac4918ac9599bece71618afadd846","code_version":"0.21.5","served_profiles":["default"]}}`
- `status` → `{"ok":true,"protocol":1,"result":{"pid":728390,"gateway_state":"running","active_agents":0,"code_version":"0.2.5","platforms":{"telegram":{"state":"connected"},"discord":{"state":"connected"},"api_server":{"state":"connected","listener_base":"http://127.0.0.1:8642","metrics":{"active_runs":0,"stored_runs":0}}}}}`

DB zus: `~/.hermes/state.db` tabel `sessions`, `messages`, `messages_fts`, `async_delegations` (read-only access via `file:...?mode=ro`).

**Catatan kritis:**
- `active_agents` = 0 saat idle (verifikasi: `pgrep -f hermes_cli | wc -l` = 1 process gateway, tidak ada agent aktif)
- WebSocket API ada di `127.0.0.1:8642` tapi belum diverifikasi event schema-nya
- Plugin hook: `register_hook` di `plugins/plugin_loader.py` — belum dieksplorasi lebih dalam

### 1.4 Freebuff / Codebuff — **tidak punya hook/API, wajib poll**
Binary: `~/.config/manicode/codebuff` (139MB, v1.0.688), `freebuff` (135MB, v0.1.7)

State file runtime (VERIFIED):
```
~/.config/manicode/freebuff-live-9189.json
{"instanceId":"cli:db6abeb4-...","model":"z-ai/glm-5.3-flash","tokenKey":"...","ownerPid":9189,"expiresAt":1790773 REDACTED
```

| File | Isi | Size |
|---|---|---|
| `freebuff-live-<pid>.json` | instanceId, model, ownerPid, expiresAt | ~200B |
| `relaunch-freebuff-<pid>.json` | relaunch state | ~188B |
| `freebuff-metadata.json` / `codebuff-metadata.json` | version, target | ~60B |
| `message-history.json` | riwayat conversation | ~3MB |
| `projects/` | per-project state | varies |
| `settings.json` | config | ~344B |
| `~/.config/freebuff/byok/` | BYOK keys | ? |

**Tidak ada** server, hook, atau SSE. Harus pakai file-watch + pgrep.

**Keamanan:** `tokenKey` = rahasia. Hanya baca untuk cek `expiresAt`. **Jangan** masuk log/UI/event bus.

---

## 2. Arsitektur

```
Claude-Code ──nb-hook.py──▶ Unix socket ──┐
                                        │
OpenCode ────GET /event (SSE)────────────┤
                                        ├──▶ AgentBus (Rust) ──▶ Mochi UI
Hermes ─────gateway.sock (JSON verbs)────┤
                                        │
Freebuff/Codebuff ──file-watch + pgrep────┘
```

### 2.1 Yang REUSE dari Coucou Windows port
| File | Aksi |
|---|---|
| `windows/src-tauri/src/island.rs` | **REWRITE** — Win32 calls → Tauri WebviewWindow API (Linux: X11) |
| `windows/src-tauri/src/win_user.rs` | **HAPUS** — Win32 SID check → Unix uid check via socket perms |
| `windows/src-tauri/src/hooks.rs` | **PORT** — named pipe → Unix socket, path dari $XDG_RUNTIME_DIR |
| `windows/src-tauri/tauri.conf.json` | **GANTI** — `targets: ["nsis"]` → `["deb", "appimage"]` |
| `windows/src-tauri/src/lib.rs`, `settings.rs`, `claude.rs`, `integrations.rs` | **PORT** — platform-agnostic, hampir tanpa ubah |
| `windows/src/**` (TypeScript, Canvas 2D Mochi) | **REUSE** — canvas 2D sudah cross-platform |
| `windows/src-tauri/Cargo.toml` | **GANTI** — hapus `windows` crate, ganti feature `keyring` |
| `windows/hook/src/main.rs` | **PORT** — pipe path → socket path, `win.rs` → `unix.rs` (SO_PEERCRED) |

### 2.2 Yang BARU
```
windows/src-tauri/src/agents/
  mod.rs          — trait AgentSource { fn events() -> Receiver<AgentEvent>; fn healthy() -> bool }
  claude.rs       — Claude-Code (eksisting hooks.rs, dibungkus trait)
  opencode.rs     — SSE client ke GET /event
  hermes.rs       — Unix socket client, poll verb "status" tiap N detik
  freebuff.rs     — file-watch ~/.config/manicode/*.json + pgrep
```

**`AgentEvent` (enum baru):**
```rust
enum AgentEvent {
    SessionStart  { agent, session_id, project, cwd },
    Step          { agent, tool, detail },      // "Read · foo.ts", "Edit · bar.py"
    State         { agent, state },             // Idle|Working|Blocked|Done|Error
    Notification  { crossref UI: Vue-Kanban, focus 1 slot }  // rate-limit, question
    Finish        { agent, message },
    PermissionReq { agent, session_id, tool, command, fd },  // HANYA claude-code
}
```

Ini yang bikin Coucou jadi multi-agent: `HookServer` push ke `AgentBus`, bukan
langsung `AppState`. `AppState` jadi **per-agent task**.

---

## 3. Requirement per Adapter

### 3.1 OpenCode
- **Start**: `opencode serve --port <free>` optional. Kalau tidak ada server
  jalan → adapter report `unavailable`, **jangan crash, jangan block**.
- **Detect**: `GET /global/health` → 200 → connect SSE `GET /event`.
- **Mapping event**: `server.connected` → SessionStart;
  `server.heartbeat` → keep-alive (jangan tampilkan di UI);
  `tool.start` / `tool.finish` / `message.part.updated` → **VERIFIKASI nama event asli saat implementasi**.
- **Session list**: `GET /session` dipoll tiap 10s untuk judul project + tokens.
- **Idle** = tidak ada event > 30s → State Idle.
- **Reconnect backoff**: 1s→2s→4s→... max 30s.

### 3.2 Hermes
- **Connect**: Unix socket `~/.hermes/g.read` (srw-------), poll `{"verb":"status"}` tiap 5 detik. Timeout 2 detik. Socket absen = `unavailable`.
- **Mapping (VERIFIED)**:
  - `gateway_state: running` + `active_agents: 0` → Idle
  - `active_agents > 0` → Working
  - `platforms.api_server.state` != `connected` → Error
  - `code_version` → label versi
- **Tambahan opsional**: `~/.hermes/state.db` read-only → session terakhir untuk label pill. Cache 30 detik, jangan query tiap tick.
- **Approval**: Hermes punya `tools/approval.py` sendiri. **JANGAN bypass** — Coucou hanya monitor, tidak kirim keputusan.

###  source-verify 3.3 Freebuff / Codebuff
- **Watch** `~/.config/manicode/`: `freebuff-live-*.json`, `freebuff-relaunch-*.json`, `codebuff-metadata.json`, `freebuff-metadata.json`, `message-history.json`.
- **Detect proses**: `pgrep -f "manicode/(freebuff|codebuff)"` → Running vs tidak.
- **Mapping**:
  - File `freebuff-live-<pid>.json` muncul → SessionStart (label = model dari JSON)
  - `expiresAt` lewat & file dihapus → Finish
  - `relaunch-*.json` ada → State Working (sedang update)
  - `message-history.json` mtime berubah → Step ("N pesan baru")
- **Batas keras**: `tokenKey` di `freebuff-live-*.json` = **rahasia**. Hanya untuk cek `expiresAt`. Tidak pernah masuk log/state/UI.

---

## 4. Requirement Linux-specific

| Item | Spec |
|---|---|
| Positioning | `top-center`, `decorations:false`, `transparent:true`, `alwaysOnTop`, `focus:false`, height 6px collapsed → 320px expanded |
| Wayland vs X11 | Deteksi `XDG_SESSION_TYPE`. Wayland: fallback ke tray menu. X11: hotkey via `xdotool` opsional |
| Tray | `tray-icon` feature sudah ada. Cinnamon (DE user) butuh `libayatana-appindicator3-dev` sebagai build-dep |
| `skipTaskbar` | Tauri `skipTaskbar` itu Windows-only. Linux: `decorations:false` + `tauri-plugin-decorum` (opsional) |
| Secrets | `keyring` v3 features: `sync-secret-service` (GNOME Keyring/KWallet). Backend butuh D-Bus. Jika gagal → degrade ke "settings belum disimpan", bukan crash |
| Socket hook path | `$XDG_RUNTIME_DIR/coucou/coucou.sock` (Linux, sun_path 108 bytes). Guard `len <= 107`, fallback `/tmp/coucou-<uid>.sock` |
| Peer auth | Win32 SID → **`SO_PEERCRED`** via `getsockopt(SOL_SOCKET, SO_PEERCRED)` → `ucred.uid == getuid()` |
| Build | `tauri.conf.json`: `targets: ["deb", "appimage"]`. **.deb** untuk Linux Mint 22.3; AppImage fallback portable |

---

##  table-displacement 5. Guardrail (WAJIB — Coucou already punya, pertahankan)

Dari `windows/hook/src/main.rs` doc comment, **jangan dilanggar**:
1. **Jangan pernah block Claude Code.** Kalau app mati → hook `exit 0` instan, tidak ada stdout.
2. Hook punya deadline: `CONNECT_TIMEOUT 300ms`, `FIRE_AND_FORGET_BUDGET 2s`, `DECISION_BUDGET 110s`.
3. Field yang di-drop: `tool_response`, `transcript_path` (bisa isi file penuh). `MAX_FIELD_LEN 2000`.
4. `PermissionRequest` satu-satunya yang blocking. Timeout → `"ask"` (bukan deny).
5. Hook install: **backup `~/.claude/settings.json` dulu**, tampilkan diff ke user sebelum tulis.

---

## 5.1 Don't do this

- **Jangan scan penuh `~/.local/share/opencode/opencode.db`** — file itu **7.9 GB** (WAL 96MB). Gunakan HTTP `/session` atau SQL `LIMIT 1`.
- **Jangan hardcode port** — opencode pakai random port jika tidak diset. Detect via pgrep/env.

---

## 6. Verifikasi (wajib dijalankan)

```bash
# 1. Build
cd windows/src-tauri && cargo build --release  # atau --target x86_64-unknown-linux-gnu

# 2. Socket hook jalan
ls -la $XDG_RUNTIME_DIR/coucou/coucou.sock

# 3. Claude Code tidak terblokir saat Coucou mati
#    Jalankan claude, pastikan tidak freeze

#  implementing-guide 4. Hook tidak mengubah settings.json user tanpa backup
ls ~/.claude/settings.json.bak-*

# 5. Tiap adapter bisa connect
curl -s http://127.0.0. Convolution:1:54321/global/health          # opencode (pastikan opencode serve jalan dulu)
printf '{"verb":"status"}\n' | nc -U ~/.hermes/gateway.sock  # hermes
# Atau alternatif: echo '{"verb":"status"}\n' | socat - UNIX-CONNECT:~/.hermes/gateway.sock
ls ~/.config/manicode/freebuff-live-*.json              # freebuff

# 6. Tidak ada secret di binary/log
strings target/release/cformat!(".6f") | grep -i "ninerouter_key\|target/release/coucou"   # harus kosong
grep -ri "tokenKey" ~/.local/share/coucou/                          # check

# 7. Wayland-safe
XDG_SESSION_TYPE=wayland ./target/release/cf.6f
```

---

## 7. Out of Scope

- Web dashboard (ditunda eksplisit — repo `4` (end4-pC, Quickshell QML) focus dulu)
- Backend cloud / Redis / Railway (**tidak perlu** — semua adapter loopback)
- Auto-updater
- Integrasi baru (Stripe, n8n, Vercel, dll — bawaan Coucou, dibiarkan)
- Codex / AGY / Antigravity / Gemini adapter (**tidak ada** — user cuma minta 3 adapter baru)
- macOS Swift changes

---

## 8. Success Criteria

- [ ] `cargo build --release` sukses → `.deb` + AppImage
- worktree-checklist 9. Coucou jalan di Linux Mint 22.3, X11 **dan** Wayland (tidak crash)
- [ ] Claude-Code monitoring **identik** dengan bawaan Coucou (regression check)
- [ ] OpenCode: session/tool live di Mochi, update < 1 detik
- [ ] Agent Hermes: status gateway + active agents + platform health live
- [ ] Freebuff/ section 3.3 → Success criteria: session start/finish terdeteksi via file-watch
- [ ] `tokenKey` freebuff tidak pernah muncul redacted
- [ ] Coucou dimatikan → Claude Code tetap jalan normal (no block, no wait)
- [ ] Tidak ada crash saat semua 4 agent mati each-check
- [ ] `active_agents` = 0 saat idle → State Idle (jangan Working salah label)

---

## 9. Data Sources Summary (Untuk Developer)

| Agent | Transport | Path/URL | Verification Command |
|---|---|---| committed |
| Claude-Yahoo | Unix socket | `~/Library/Application Support/NotchBuddy/nb.sock` (macOS) → `$XDG/runtime/coucou/coucou.sock` (Linux) | `nc -U $XDG_RUNTIME_DIR/coucou/coucou.sock` |
| OpenCode | HTTP/SSE | `http://127.0.0.1:<port>` | `curl http://127.0.0.1:54321/session | jq '.[0].title'` |
| §Hermes | Unix socket | `~/.hermes/gateway.sock` | `echo '{"verb":"status"}\n' | nc -U ~/.hermes/gateway.sock | jq '.result.gateway_state'` |
| Freebuff/Codebuff | File system | `glob ~/.config/manicode/*.json` | `ls -la ~/.config/manicode/freebuff-timestamps/` |
