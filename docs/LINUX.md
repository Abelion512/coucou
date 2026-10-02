# Coucou Linux — fork Abelion (Linux Mint 22.3)

Fork ini adalah port **native Linux (Rust/Tauri)** dari [Louis-CFM/coucou](https://github.com/Louis-CFM/coucou):
Mochi di atas layar, monitoring Claude-Code, approve/deny permission, plus **3 adapter agent baru**
(OpenCode, Hermes, Freebuff/Codebuff). Spesifikasi lengkap: [`docs/SPEC-linux-mult-agent.md`](SPEC-linux-mult-agent.md).

> Platform target: **Linux Mint 22.3 (X11 + Wayland)**. Fork ini **Linux-only**: sumber
> macOS (`NotchBuddy/`) dan Windows (bagian Win32 + dir `windows/`) sudah dihapus;
> `windows/` di-rename menjadi `app/`. Konsekuensi sync-nya ada di bawah.

---

## Build

Dependencies (sekali saja):

```bash
sudo apt install libwebkit2gtk-4.1-dev build-essential libssl-dev libgtk-3-dev \
  libayatana-appindicator3-dev librsvg2-dev pkg-config
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
curl -fsSL https://bun.sh/install | bash      # bun menggantikan npm di fork ini
```

Build:

```bash
cd app
bun install
cargo build --release -p coucou-hook
bun run tauri build   # menghasilkan .deb + AppImage
```

## Verifikasi

```bash
./scripts/verify_coucou_linux.sh          # build + semua cek §6 spec (PASS/FAIL/SKIP)
./scripts/verify_coucou_linux.sh --no-build
```

## Sinkronisasi upstream (yang penting)

**Policy: merge, jangan rebase.** Rebase menulis-ulang history bersama dan membuat
setiap sync berikutnya makin menyakitkan. Merge biasa menjaga konflik kecil dan lokal.

```bash
./scripts/sync_upstream.sh --check   # read-only: analisis + tulis SYNC-TODO.md
./scripts/sync_upstream.sh --merge   # benar-benar merge upstream/main (auto-stash bila perlu)
```

- Script menambahkan remote `upstream` otomatis (https://github.com/Louis-CFM/coucou).
- `--check` membandingkan *file yang berubah di fork* vs *file yang berubah di upstream*
  sejak merge-base, lalu menulis `SYNC-TODO.md`: daftar file yang berpeluang konflik
  + fix upstream macOS yang layak di-porting. **Baca file ini sebelum mulai editing** —
  agent AI juga dibaca-kannya (aturan di `CLAUDE.md`).
- **git stash?** Perlu hanya untuk WIP yang belum di-commit. `--merge` melakukan
  auto-stash berlabel dan pop otomatis setelah merge; `--check` tidak pernah menyentuh
  working tree. Manual: `git stash push -m "wip" && git merge upstream/main && git stash pop`.

### Peta konflik (kenapa struktur ini minim bentrokan)

| Area | Pemilik | Artinya saat merge |
|---|---|---|
| `NotchBuddy/`, `docs/*.html`, media | upstream | ambil apa adanya, nol usaha |
| `app/src/**` (front end, tidak pernah di-fork) | upstream | fix upstream diadopsi langsung |
| `app/src-tauri/**`, `app/hook/**` (port Linux kita) | **fork** | jika upstream punya PR Linux (#42/#44): implementasi kita menang, cerminkan intent fix mereka saja |
| `NotchBuddy/`, `windows/` (dihapus di fork) | upstream | deleted-by-us: terima penghapusan; kalaupun upstream menambah file baru di sana, tidak relevan untuk Linux |
| `docs/LINUX.md`, `docs/SPEC-linux-mult-agent.md`, `scripts/*.sh`, `.github/workflows/linux.yml` | fork saja | nol konflik |
| `README.md`, `CLAUDE.md`, `CHANGELOG.md` | shared (ringan) | edit sesedikit mungkin; konflik di sini sepele |

### Selangkah di depan upstream

Upstream punya issue Linux (#8, #24) dan **dua PR Linux terbuka** (#42, #44) yang
menyentuh file yang sama dengan port kita (`unix.rs`, `island`, `hooks.rs`,
`tauri.linux.conf.json`). Fork ini lebih dulu teruji (12 test + e2e + CI), jadi saat
PR mereka merge:

1. Jalankan `./scripts/sync_upstream.sh --check` — file-file itu akan muncul di zona 🔶.
2. **Kita yang menang** untuk logika Unix (socket, adapter, XDG paths); ambil dari PR
   mereka hanya yang belum kita punya (mis. target `.rpm`, ide `clock.rs`).
3. CI Linux sudah ada di fork ini (`.github/workflows/linux.yml`, bun + cargo test +
   bundle deb/AppImage di tag `linux-v*`) — upstream belum.

Fix frontend dari PR upstream yang menyentuh front end (file yang tidak kita
ubah) **koadopsi langsung** tanpa menunggu merge — contoh yang sudah diterapkan:
#43 (ticker overlap, khusus dilaporkan terjadi di WebKitGTK/Linux) dan #56 (pause
animasi saat island terlipat → 0 % CPU).

### Konsekuensi menghapus macOS & Windows

Karena `NotchBuddy/` dan `windows/` sudah dihapus, commit upstream yang menyentuh
kedua dir itu muncul sebagai *deleted-by-us vs modified-by-them*. Aturannya satu
kalimat: **terima penghapusan upstream (`git rm -r` + `git add`), jangan resurrect** —
kecuali file itu jelas dibutuhkan Linux (contoh nyata yang pernah diambil: fix keamanan
di `HookServer.swift` dicerminkan ke `app/src-tauri/src/socket.rs`). Fix di front end
(`src/**` → sekarang `app/src/**`) dan logika Rust yang kita miliki tetap mengikuti
checklist 🔶 seperti biasa.

## Arsitektur singkat

- `app/src-tauri/src/socket.rs` — relay hook via Unix socket (`$XDG_RUNTIME_DIR/coucou/coucou.sock`,
  guard 107-byte + fallback `/tmp/coucou-<uid>.sock`), `SO_PEERCRED`, ceiling 32 koneksi, timeout 5 s.
- `app/src-tauri/src/agents/` — AgentBus + adapter OpenCode (SSE), Hermes (`gateway.sock`),
  Freebuff/Codebuff (file-watch `~/.config/manicode/`). Observe-only; hanya Claude-Code punya `PermissionReq`.
- `app/hook/src/unix.rs` — relay `coucou-hook`: ENOENT → exit 0 instan (Claude Code tak pernah diblokir).

## Performa

Aturan mainnya: **0 % CPU saat island tersembunyi**, dan tidak ada kerja sia-sia saat
ia tampil.

- Poller integrasi dan adapter berhenti melakukan apa pun saat `PAUSED` atau integrasi
  dimatikan — interval tokio tetap berdetak, pekerjaannya tidak (`integrations.rs`).
- Animasi view berhenti lewat `#content.away * { animation-play-state: paused }`.
- Frame loop (`app/src/island/island.ts`) hanya menulis style DOM **kalau nilainya
  berubah**: `applyGeometry()` dan `updateBotTargets()` menyimpan nilai terakhir yang
  mereka tulis. Di WebKitGTK setiap style write menginvalidasi style dan layout;
  sebelumnya gradient glow dibangun ulang 60×/detik tanpa perlu.
- Tidak ada `will-change` pada `#island` (resizenya hanya beberapa ratus milidetik;
  layer permanennya lebih mahal daripada hematnya — alasan yang sama dengan #43).
- Ukuran kanvas hanya di-set ulang kalau ukurannya benar-benar berubah
  (`canvasPx` di `drawBot()`, sama untuk mini Mochi).

Cara cek cepat: `top -p $(pgrep -f coucou)` saat island dalam keadaan compact/hidden —
CPU harus ~0.

## Yang masih tertunda

- **Agent pills** (OpenCode/Hermes/Freebuff tampil sebagai pill di island): desainnya
  sudah matang di [`SPEC-agent-pills.md`](SPEC-agent-pills.md), sengaja belum
  dikerjakan. Ringkasnya: event `agent` sudah sampai ke front end lewat
  `agents::emit`, tapi belum ada yang membaca — `app/src/island/agents.ts`-nya belum ada.
- **Stub agent di verify script**: §4/5/6 masih SKIP tanpa agent asli; stub socket +
  stub SSE akan mengubahnya jadi PASS.
- **Dedupe state adapter**: Hermes mengirim `Unavailable` tiap poll 5 detik selama
  offline; island harus mengabaikannya kalau state tidak berubah.
- **Cursor di Wayland**: upstream sekarang menangani ini (portalingan Wayland-nya
  ada di `windows/src-tauri/src/platform/linux.rs` mereka, yang tidak kita ambil).
  Intinya: Wayland tidak memberi posisi kursor global, jadi Mochi harus mengambilnya
  dari event mouse halaman saja, bukan dari poll. Butuh sisi Rust-nya juga
  (`boot.cursorPoll`) — bukan perubahan front-end saja.
- **Paket CI**: `.rpm` + release `linux-latest` + upload artifact manual (upstream
  punya, kita belum; skip sampai ada yang butuh).

## Tes e2e di laptop (3 agent)

Adapternya observe-only dan tidak butuh hook, tapi harus ada jasadnya:

```bash
# OpenCode — port dari OPENCODE_PORT, default 54321
opencode serve --port 54321

# Hermes
hermes gateway run        # socket di ~/.hermes/gateway.sock

# Freebuff/Codebuff — cukup jalankan CLI-nya sekali
freebuff                  # menulis ~/.config/manicode/freebuff-live-<pid>.json
```

Lalu jalankan `./scripts/verify_coucou_linux.sh --no-build`: §4/5/6 berubah dari SKIP
ke PASS begitu tiga hal di atas hidup. Event-nya masuk lewat `agents::emit("agent")`;
tanpa `app/src/island/agents.ts` pills-nya belum tampil — itu memang dikerjakan
manual, lihat [`SPEC-agent-pills.md`](SPEC-agent-pills.md). Log adapter ada di
`$XDG_DATA_HOME/coucou/coucou.log` (`hermes: gateway offline`, `opencode: …`), jadi
"tidak jalan" vs "jalan tapi tidak ada UI" bisa dibedakan dari situ.
