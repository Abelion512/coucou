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

## auto-ponytail — DEBT sweep

```bash
./scripts/auto_ponytail.sh            # tulis docs/auto-ponytail.md, cetak ringkasan
./scripts/auto_ponytail.sh --quiet    # ringkasan saja
```

Snapshotberkas yang **kode** dan **repo** bisa buktikan sendiri,without network dan
dalam ~1 detik: modulyangtidak di-import, path di docs yangtidak ada, skrip yang
tidak disebut siapa pun, marker TODO/FIXME, dan klaim `#[allow(dead_code)]`.

Yang **tidak** dicek, dan alasannya: apakah aplikasinya jalan (itu
`verify_coucou_linux.sh`, yang melakukan probe live dan melapor PASS/FAIL/SKIP),
status sync upstream (itu `sync_upstream.sh --check`), apa pun yang butuh jaringan,
dan penilaian rasa — "ini akan lebih bagus" adalah taste, dan skrip yang Issue taste
adalah skrip yang pendapatnya bertahan lebih lama dari gunanya.

False positive diketahui dan ditulis di output, bukan disembunyikan: check "modul
yang tidak di-import" bisa salah pada entry point dan modul yang hanya/rujukan dari
HTML, dan check "path docs" **akan** memunculkan path yang memang sengaja dihapus
(`NotchBuddy/`, `windows/`) — itu bukan temuan, itu bukti bahwa check-nya bekerja.

Report-nya di-gitignore: dia dibuat ulang setiap kali dijalankan.

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
- `app/src/island/agents.ts` — ujung yang lain dari event `agent`: memetakan
  `AgentEvent` Rust ke pill di island. Tanpa file ini ketiga adapter tetap jalan,
  tetap nulis ke log, dan tidak terlihat sama sekali.
- `app/hook/src/unix.rs` — relay `coucou-hook`: ENOENT → exit 0 instan (Claude Code tak pernah diblokir).

### Ukuran jendela island

Panel adalah 720×320 **CSS px**, dan front end menata diri terhadap angka itu. GTK
tidak bisa dipercaya melaporkan scale factor saat startup — pada display 1.25×
`Monitor::scale_factor` dan `Window::scale_factor` sama-sama bilang 1.0 — sehingga
jendela pernah digambar 720 px fisik, WebKit membaginya 1.25, dan island 640 px
terpotong di kedua sisi.

Jadi webview mengukur dirinya sendiri (`Bridge.reportViewport`) dan memberitahu Rust
seberapa px CSS yang benar-benar dia dapat; Rust memakai rasio itu untuk memperbesar
jendelanya. WebKit adalah satu-satunya pihak yang tahu scale factor yang sebenarnya,
karena dia yang membagi px fisik menjadi px CSS.

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

### Ukuran jendela island

Panel adalah 720×320 **CSS px**, dan front end menata diri terhadap angka itu. Dua
hal harus benar supaya tidak terpotong:

- **Jendela mengikuti area kerja, bukan layar penuh.** Cinnamon menaruh jam dan
  notification centre di tengah atas — persis di tempat notch macOS. Kalau island
  digambar di `y` monitor, ia tenggelam di balik panel, dan bagian paling atas
  (header: home / chat / upload / sound / settings) tidak pernah terlihat.
  `Monitor::work_area()` sudah dikurangi panel oleh WM, jadi ini berlaku untuk
  Cinnamon, GNOME dan apa pun tanpa konfigurasi per-DE.
- **Skala diambil dari webview, bukan dari GTK.** Pada display 1.25×,
  `Monitor::scale_factor` dan `Window::scale_factor` sama-sama melaporkan 1.0, jadi
  panel 720 px menjadi viewport 576 px dan island 640 px kehilangan 64 px di kedua
  sisi. Webview mengukur dirinya sendiri (`Bridge.reportViewport`) dan Rasio
  dihitung sebagai **px fisik ÷ px CSS** — bukan "yang diminta ÷ yang diukur",
  karena koreksi itu membuat laporan kedua membaca keluarannya sendiri dan
  mengembalikan faktor ke 1.0.

### Membuka island

Tidak perlu tray sama sekali. Arahkan kursor ke **tepi atas tengah** — island
muncul sebagai bar kecil (288×32), lalu **klik** untuk membuka. Tray ("Open
Coucou") tetap ada sebagai jalan lain, bukan syarat.

## Yang masih tertunda

- **Health row di island**: kenapa satu agent mati (`api_server down`, socket
  hilang). Sudah hidup di Settings sebagai section "Agents" — di island tetap pill
  saja, karena island punya 108 px area view dan pill kosong akan jadi kebohongan.
- **Stub agent di verify script**: §4/5/6 masih SKIP tanpa agent asli; stub socket +
  stub SSE akan mengubahnya jadi PASS.
- **Cursor di Wayland**: upstream sekarang menangani ini (portalingan Wayland-nya
  ada di `windows/src-tauri/src/platform/linux.rs` mereka, yang tidak kita ambil).
  Intinya: Wayland tidak memberi posisi kursor global, jadi Mochi harus mengambilnya
  dari event mouse halaman saja, bukan dari poll. Butuh sisi Rust-nya juga
  (`boot.cursorPoll`) — bukan perubahan front-end saja.
- **Paket CI**: `.rpm` + release `linux-latest` + upload artifact manual (upstream
  punya, kita belum; skip sampai ada yang butuh).

## Cara pakai (pertanyaan yang paling sering muncul)

### Apakah harus jalan dulu, atau bisa langsung dipakai?

**Coucou bisa langsung dipakai** — tidak ada agent yang harus dijalankan lebih dulu.
App-nya langsung boot,>(), dan Claude Code langsung terhubung begitu hook-nya terpasang.

Agent tambahan **t sensed**: pill OpenCode/Hermes/Freebuff **muncul hanya kalau agent
itu benar-benar jalan**. Kalau tidak, tidak ada pill sama sekali — bukan placeholder,
baris kosong, atau "not running". Ini disengaja (lihat §5 `SPEC-agent-pills.md`):
pill yang selalu ada membuat island berbohong tentang apa yang sedang terjadi.

Jadi urutannya: **install → pakai Coucou → kalau mau lihat agent lain, nyalakan agent
itu.** Tidak ada langkah urutan lain.

### Claude Code

```bash
# 1. Buka Settings dari tray Coucou → bagian Hooks
# 2. Klik "Preview" — Coucou menampilkan diff lebih dulu
# 3. Klik "Install" (menulis ~/.claude/settings.json, dengan backup bertanggal)
```

Alternatif manual: `~/.local/share/coucou/bin/coucou-hook <EventName>`.

### Tiga agent tambahan

| Agent | Yang perlu jalan |ockup cara cek |
|---|---|---|
| OpenCode | `opencode serve --port 54321` | `curl -s localhost:54321/global/health` |
| Hermes | `hermes gateway run` | `ls ~/.hermes/gateway.sock` |
| Freebuff/Codebuff | CLI-nya dijalankan sekali | `ls ~/.config/manicode/freebuff-live-*.json` |

Setelah itu pill-nya muncul tanpa restart Coucou — adapter reconnect sendiri dengan
backoff 1s→30s.

### Kalau pill tidak muncul

Bedakan "tidak jalan" vs "jalan tapi tidak ada UI" dari log:

```bash
grep -E "hermes:|opencode:|freebuff:" ~/.local/share/coucou/coucou.log | tail
```

`hermes: gateway online` + pill tidak muncul = bug. `gateway offline` = Hermes-nya
memang belum jalan.

## Tes e2e di laptop (3 agent)

Adapternya observe-only dan tidak butuh hook, tapi harus ada jasadnya:

```bash
# OpenCode — port dari OPENCODE_PORT, default 54321
opencode serve --port 54321

# Hermes
hermes gateway run        # socket di ~/.hermes/gateway.sock

# Freebuff/Codebuff — TUI-nya harus benar-benar dipakai
freebuff                  # lalu KIRIM satu pesan, baru ia menulis
                          # ~/.config/manicode/freebuff-live-<pid>.json
```

Freebuff tidak menulis file live saat start: yang muncul cuma "Your first message
starts the session". Jadi §6 tetap SKIP kalau CLI-nya cuma dibuka lalu ditinggal —
harus ada pesan yang benar-benar terkirim (butuh kuota, jadi `0/25 Freebucks`
belum tentu cukup).

Lalu jalankan `./scripts/verify_coucou_linux.sh --no-build`: §4/5/6 berubah dari SKIP
ke PASS begitu tiga hal di atas hidup. Event-nya masuk lewat `agents::emit("agent")` dan
diterima `app/src/island/agents.ts`, yang memetakannya ke pill di island.

### Dua jebakan yang sudah pernah menipu

Adapter Hermes dan harness-nya bicara dua bahasa berbeda, dan itu sudah beberapa kali
menipu:

- **Newline itu frame delimiter.** `~/.hermes/gateway.sock` tidak menjawab apa pun
  kalau JSON-nya tidak diakhiri `\n` — diam saja ~2 detik lalu nutup. Relay dan
  `hermes.rs` sudah mengirim `\n`; harness pernah tidak, dan itu terlihat seperti
  gateway mati.
- **Socket yang sudah mati masih berbentuk socket.** Kalau Coucou crash, inode-nya
  tetap ada di `$XDG_RUNTIME_DIR/coucou/coucou.sock` dan `-S` tetap match, padahal
  `connect()` dapat `ECONNREFUSED`. Semua probe harus connect dulu, bukan hanya
  cek `-S`.
