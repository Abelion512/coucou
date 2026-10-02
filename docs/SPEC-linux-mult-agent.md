# SPEC: Coucou Multi-Agent — Rust/Tauri Linux + 3 Adapter Baru

> Ditulis di mesin kerja lokal (Linux Mint, X11). Angka di §1 berasal dari audit
> live di mesin itu sendiri; jalankan ulang `./scripts/verify_coucou_linux.sh` kalau
>_infonya sudah berbeda.

**Dasar:** adopsi `Louis-CFM/coucou`, **tidak rewrite dari nol**. Semua yang sudah
bisa jalan (Mochi UI, Claude-Code monitoring, permission approve/deny, 28 sounds,
animations, integrasi) dipakai apa adanya. Yang ditambahkan hanya:

1. **Dukungan Rust untuk Linux** — Swift hanya compile di Android/macOS, jadi tidak
   bisa jadi backend Linux sama sekali; Rust/Tauri yang Brigades cross-platform.
2. **3 adapter agent baru**: OpenCode, Freebuff/Codebuff, Hermes.

Tidak ada perubahan ke upstream. Yang "disesuaikan" hanya bagian yang tidak punya
padanan di Linux (mis. Win32 socket → Unix socket).

---

## 1. Fakta Audit (VERIFIED di mesin kerja lokal ini)

### 1.1 Claude Code — bawaan Coucou, tetap ada (disesuaikan seperlunya)
Di Linux, relay-nya `coucou-hook` (Rust, `app/hook/`) → Unix socket
`$XDG_RUNTIME_DIR/coucou/coucou.sock`. Event: SessionStart, SessionEnd,
UserPromptSubmit, PreToolUse, PostToolUse, PostToolUseFailure, PermissionRequest,
Notification, Stop, StopFailure, SubagentStart, SubagentStop.

Budget waktu (dari `app/hook/src/main.rs`): connect 300 ms, fire-and-forget 2 s,
PermissionRequest 110 s. Angka 110 s ini yang membuat Claude Code tidak pernah
freeze — lewat atau tidak dijawab, terminal mengambil alih.

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
Claude Code ──coucou-hook──▶ Unix socket ──┐
                                        │
OpenCode ────GET /event (SSE)────────────┤
                                        ├──▶ AgentBus (Rust) ──┐
Hermes ─────gateway.sock (JSON verbs)────┤                      │
                                        │                      ▼
Freebuff/Codebuff ──file-watch + pgrep────┘            island ("agent" event)
                                                               │
                                                               ▼
                                                        pills + Mochi
```

### 2.1 Yang DIPAKAI dari Coucou (path sekarang — `windows/` sudah dihapus)
| File | Aksi |
|---|---|
| `app/src-tauri/src/island/` | **REWRITE** — Win32 calls → Tauri WebviewWindow API; call yang butuh X11 ada di `unix.rs` |
| `app/src-tauri/src/hooks.rs` | **PORT** — named pipe → Unix socket, path dari $XDG_RUNTIME_DIR |
| `app/src-tauri/tauri.conf.json` | **GANTI** — `targets: ["nsis"]` → `["deb", "appimage"]` |
| `app/src-tauri/src/lib.rs`, `settings.rs`, `claude.rs`, `integrations.rs` | **PORT** — platform-agnostic, hampir tanpa ubah |
| `app/src/**` (TypeScript, Canvas 2D Mochi) | **REUSE** — canvas 2D sudah cross-platform |
| `app/hook/src/main.rs` | **PORT** — pipe path → socket path, `win.rs` → `unix.rs` (SO_PEERCRED) |

### 2.2 Yang BARU
```
app/src-tauri/src/agents/
  mod.rs        — AgentBus + enum AgentEvent/Agent/AgentState
  opencode.rs   — SSE client ke GET /event
  hermes.rs     — Unix socket client, poll verb "status" tiap N detik
  freebuff.rs   — file-watch ~/.config/manicode/*.json + pgrep
app/src/island/agents.ts   — ujung event "agent": AgentEvent → pill di island
```

Tidak ada `claude.rs` di sini: Claude Code bukan adapter. Event-nya datang lewat
relay hook yang sudah ada, dan language-nya sendiri.

**`AgentEvent` (enum di `agents/mod.rs`):**
```rust
enum AgentEvent {
    SessionStart  { agent, session_id, project, cwd },
    Step          { agent, tool, detail },      // "Read · foo.ts", "Edit · bar.py"
    State         { agent, state },             // Idle|Working|Blocked|Done|Error|Unavailable
    Notification  { agent, kind, message },     // rate-limit, platform health
    Finish        { agent, session_id, message },
    PermissionReq { agent, session_id, tool, command, request_id },  // HANYA Claude Code
}
```

Ini yang bikin Coucou jadi multi-agent: adapter push ke `AgentBus`, bukan
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

Dari `app/hook/src/main.rs` doc comment, **jangan dilanggar**:
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

## 6. Verifikasi

Semua cek ini sudah diotomatisasi — jalankan satu perintah, bukan daftar manual:

```bash
./scripts/verify_coucou_linux.sh              # build + semua cek §6
./scripts/verify_coucou_linux.sh --no-build   # kalau sudah ada binarynya
```

Script mencetak PASS/FAIL/SKIP per cek dan keluar dengan ringkasan di akhir. SKIP
adalah kondisi yang memang butuh campur tangan (mis. Freebuff belum dijalankan),
bukan cek yang lolos diam-diam.

Manual, kalau mau melihat apa yang sebenarnya dicek:

```bash
# hook relay hidup
ls -la "$XDG_RUNTIME_DIR/coucou/coucou.sock"

# Coucou mati → Claude Code tidak boleh tertahan (jalankan claude, pastikan tidak freeze)

# hook install tidak menimpa settings.json tanpa backup
ls ~/.claude/settings.json.bak-*

# tiap adapter bisa connect
curl -s http://127.0.0.1:54321/global/health                 # opencode
printf '{"verb":"status"}\n' | nc -U ~/.hermes/gateway.sock # hermes
ls ~/.config/manicode/freebuff-live-*.json                   # freebuff

# tidak ada secret di binary atau di disk
strings app/target/release/coucou | grep -i tokenKey   # harus kosong
grep -ri tokenKey ~/.local/share/coucou/               # harus kosong
```

§4/5/6 berubah dari SKIP ke PASS begitu OpenCode/Hermes/Freebuff benar-benar jalan.
Kalau agent-nya hidup tapi pill-nya tidak muncul, itu bug Coucou — bukan setup.
Cek `~/.local/share/coucou/coucou.log`: `hermes: gateway online` + pill kosong =
bug; `gateway offline` = agent-nya belum jalan.

---

## 7. Out of Scope

- Backend cloud / Redis / Railway (**tidak perlu** — semua adapter loopback, dan
  menjaga Coucou tidakanh tanpa akun/telemetry adalah salah satu guardrail-nya)
- **Auto-updater**: yang di-update hanya Coucou fork ini, dan hanya lewat .deb /
  AppImage yang kamu pasang sendiri. Tidak ada update dari upstream ke user — sync
  upstream adalah urusan repo (`./scripts/sync_upstream.sh`), bukan runtime app.
- **Adapter Codex / AGY / Antigravity / Gemini** — cuman OpenCode, Freebuff, Hermes
- Integrasi baru: yang dipakai cuma **Linear, Notion, GitHub**. Stripe/n8n/Vercel/
  Resend/Cal.com bawaan upstream dibiarkan apa adanya di kode, tapi tidak diaktifkan
  secara default — Configuring key-nya adalah keputusan tiap orang, bukan default kita.
- Perubahan ke front end yang sudah ada:-Mochi dipakai apa adanya.

---

## 8. Success Criteria

- [x] `bun run tauri build` sukses → `.deb` + AppImage
- [x] Coucou jalan di Linux Mint, X11 (Wayland = fallback tray, tidak crash)
- [x] Claude-Code monitoring **identik** dengan bawaan Coucou (regression check)
- [x] OpenCode: session/tool live di Mochi, update < 1 detik
- [x] Hermes: status gateway + active agents + platform health live
- [x] Freebuff/Codebuff: session start/finish terdeteksi via file-watch
- [x] `tokenKey` freebuff tidak pernah muncul di log/UI/event
- [x] Coucou dimatikan → Claude Code tetap jalan normal (no block, no wait)
- [x] Tidak ada crash saat semua 4 agent mati
- [x] `active_agents` = 0 saat idle → State Idle (jangan Working salah label)

---

## 9. Data Sources Summary (Untuk Developer)

| Agent | Transport | Path/URL | Verification Command |
|---|---|---| committed |
| Claude Code | Unix socket | `$XDG_RUNTIME_DIR/coucou/coucou.sock` (fallback `/tmp/coucou-<uid>.sock`) | `nc -U "$XDG_RUNTIME_DIR/coucou/coucou.sock"` |
| OpenCode | HTTP/SSE | `http://127.0.0.1:<port>` (dari `$OPENCODE_PORT`, default 54321) | `curl -s http://127.0.0.1:54321/global/health` |
| Hermes | Unix socket | `~/.hermes/gateway.sock` | `printf '{"verb":"status"}\n' \| nc -U ~/.hermes/gateway.sock` |
| Freebuff/Codebuff | File system | `glob ~/.config/manicode/*.json` | `ls ~/.config/manicode/freebuff-live-*.json` |
