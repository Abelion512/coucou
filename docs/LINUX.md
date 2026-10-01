# Coucou Linux — fork Abelion (Linux Mint 22.3)

Fork ini adalah port **native Linux (Rust/Tauri)** dari [Louis-CFM/coucou](https://github.com/Louis-CFM/coucou):
Mochi di atas layar, monitoring Claude-Code, approve/deny permission, plus **3 adapter agent baru**
(OpenCode, Hermes, Freebuff/Codebuff). Spesifikasi lengkap: [`docs/SPEC-linux-mult-agent.md`](SPEC-linux-mult-agent.md).

> Platform target: **Linux Mint 22.3 (X11 + Wayland)**. Kode macOS (`NotchBuddy/`) dan
> Windows (`windows/` bagian Win32) dibiarkan utuh — mereka "penumpang" yang tidak
> dikompilasi di Linux, dan justru membuat merge upstream nyaris bebas konflik.

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
cd windows          # ya: port Linux hidup di tree windows/ — strukturnya sengaja dipakai ulang
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
| `windows/**` (logika Win32 + file yang kita porting) | **shared** | satu-satunya zona konflik nyata — resolv manual, biasanya fix upstream perlu dicerminkan ke sisi Unix |
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

Fix frontend dari PR upstream yang menyentuh `windows/src/` (file yang tidak kita
ubah) **koadopsi langsung** tanpa menunggu merge — contoh yang sudah diterapkan:
#43 (ticker overlap, khusus dilaporkan terjadi di WebKitGTK/Linux) dan #56 (pause
animasi saat island terlipat → 0 % CPU).

### Kenapa macOS & Windows tidak dihapus

Menghapus `NotchBuddy/` / `windows/` memang "bersih", tapi setiap commit upstream yang
menyentuh dir itu lalu menjadi konflik *deleted-by-us vs modified-by-them* — selamanya.
Kode itu tidak dikompilasi di Linux dan tidak menambah biaya apa pun. Ponytail rung 1:
*(does this need to exist? no → skip)*. Kalau suatu hari benar-benar mau:
`git rm -r NotchBuddy && git commit -m "drop macOS"` — tapi kamu sudah diperingatkan. 🙂

## Arsitektur singkat

- `windows/src-tauri/src/socket.rs` — relay hook via Unix socket (`$XDG_RUNTIME_DIR/coucou/coucou.sock`,
  guard 107-byte + fallback `/tmp/coucou-<uid>.sock`), `SO_PEERCRED`, ceiling 32 koneksi, timeout 5 s.
- `windows/src-tauri/src/agents/` — AgentBus + adapter OpenCode (SSE), Hermes (`gateway.sock`),
  Freebuff/Codebuff (file-watch `~/.config/manicode/`). Observe-only; hanya Claude-Code punya `PermissionReq`.
- `windows/hook/src/unix.rs` — relay `coucou-hook`: ENOENT → exit 0 instan (Claude Code tak pernah diblokir).
