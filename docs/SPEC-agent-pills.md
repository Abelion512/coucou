# SPEC — Agent pills + adapter health line

Status: **v1 (pill) sudah diimplementasikan** di `app/src/island/agents.ts`;
health row (§5) masih ditunda. Ditulis sebelum eksekusi supaya tidak membangun
sesuatu yang ternyata tidak kepakai. Prinsip: ponytail (rung 1–7) — tidak ada kode
untuk kebutuhan yang belum terbukti, dan tidak ada fitur spekulatif.

> Catatan implementasi: pada display berskala, island sempat terpotong di kedua sisi
> karena jendela digambar memakai px fisik sementara front end menata diri terhadap
> 720×320 **CSS px**. Fix-nya di `Bridge.reportViewport` + `island::apply_geometry`,
> bukan di spec ini — lihat `docs/LINUX.md` § Ukuran jendela island.

Pasangan spec: [`SPEC-linux-mult-agent.md`](SPEC-linux-mult-agent.md) (audit + adapter),
[`AGENTS.md`](../AGENTS.md) (guardrail), preview `/design-preview/agent-pills.html`.

---

## 1. Tujuan

Satu barang: **event dari AgentBus (`agent`) sampai ke island**, sehingga OpenCode,
Hermes dan Freebuff/Codebuff tampil sebagai pill dan kelihatan hidup/t mati.

Bukan tujuan: membuat Claude Code lebih pintar, menambah adapter, menambah setting.

## 2. Ukuran island: TIDAK BERUBAH

`islandSize("expanded", "overview")` = **640 × 160** (`core/layout.ts`). Angka itu
dipakai apa adanya; tidak ada proposal menaikkan tinggi island.

Budget area view (`#content` padding 8/10 + header 34):

```
160 − 8 − 34 − 10 = 108 px
```

| Elemen | Tinggi | Sisa |
|---|---|---|
| grid pills, 2 baris (`28 + 4 + 28`) | 60 px | 48 px |
| health row | 24 px | **24 px** |
| total | 84 px | 24 px lega |

Aturan keras: **pills maksimum 4** (2 baris). Kode sudah membatasi ini —
`State.otherTasks.slice(0, 4)` (`views.ts:234`). Kalau nanti agent + integrasi
membuat 6 pill, baris ke-3 terpotong — itu perilaku yang sama dengan hari ini, bukan
regresi. Health row tidak boleh tumbuh (font 10.5 px, `padding-bottom: 8 px`).

## 3. Alamat data

```
adapter (Rust) → AgentBus → emit("agent") → island/island/agents.ts → State
```

Rust **tidak berubah** di v1. Yang berubah: 1 file baru + 3 file disentuh kecil.

| Event Rust | Efek di State |
|---|---|
| `SessionStart { session_id, project, cwd }` | pill agent muncul (state `idle`), `name` = nama platform, `sessionCwd` = cwd (bila ada), langkah_stepsKosong |
| `Step { tool, detail }` | `appendStep`, state → `working` |
| `State { state }` | `updateTask`; `Unavailable` **tidak** membuat pill, hanya health row |
| `Notification { kind, message }` | `ratelimit` / `question` bila relevan, tanpa menunggu manusia |
| `Finish { message }` | `finished` 5 detik + badge centang, lalu pill dihapus (`removeTask`) |
| `PermissionReq` | **tidak pernah** — observe-only; `socket.rs` sudah menolak payload bertanda sebelum pending decision dibuka |

## 4. Kebijakan pill

- **Satu pill per agent** (`agent_opencode`, `agent_hermes`, `agent_freebuff`), bukan
  per session. Kalau agent punya beberapa session: session terakhir yang menang, dan
  `who .count` (yang sudah ada) menampilkan jumlah langkahnya. Alasannya: 3 agent ×
  beberapa session akan memakan budget 2 baris pills, dan urutan jadi tidak stabil.
- Label pill = nama platform (`OpenCode`, `Hermes`, `Freebuff`) — bukan `Agent`.
  Label "tool" di kartu kiri memakai nama platform yang sama (perubahan kecil pada
  `sourceLabel()` di `views.ts`).
- Pill **tidak pernah** dibuat untuk agent yang `Unavailable` — agent yang tidak jalan
  simplesmente tidak ada. Tidak ada baris kosong, tidak ada tekad placeholder.
- Urutan: `integration_claude` → agent pills → integrasi (sudah ada di
  `State.loadIntegrationTasks`).
- Badge error memakai `pillBadge: "error"` yang sudah ada.
- Warna: Claude Code `#D97757` (oranye Claude), OpenCode `#5D9CFF`, Hermes `#FFD700`,
  Freebuff `#2DD4BF`. Integrasi default: Notion, n8n, Vercel, GitHub — Resend
  dinonaktifkan (butuh key sendiri, tidak ada yang memakainya).

## 5. Health row — rekomendasi: **hapus dari v1**

Awalnya health row selalu-on di footer kartu kanan. Setelah dihitung ulang:

- Yang paling informatif hanya "kenapa agent tidak muncul". Tanpa alasan
  detail (v1 tidak membawanya), informasinya cuma "tidak jalan" — dan itu sudah
  terlihat dari pill yang tidak ada.
- Alasan detailnya ada di log (`coucou.log`), dan membawanya ke UI berarti mengubah
  struct event di Rust.
- Biayanya nyata: 24 px dari 108 px area view, CSS baru, satu komponen baru yang harus
  dirawat.

Jadi v1 = **pill saja**. Kalau ternyata sering perlu "kenapa mati", health row
kembali sebagai tambahan — dan saat itu ia harus **kondisional**: muncul hanya
ketika ada agent yang tidak live, jadi jalur bahagia memakai 0 px.

Kalau ternyata tetap dipertahankan, desainnya sudah siap di
`app/design-preview/agent-pills.html`: footer kartu kanan, tiga slot fixed
(OpenCode · Hermes · Freebuff), 24 px.

| State | Dot | Teks |
|---|---|---|
| `working` | biru agent, menyala | `working` |
| `idle` / terhubung | hijau | `idle` · `0 agents` / `1 session` |
| `Unavailable` | redup | `not running` |
| `Error` (mis. `api_server` putus) | merah | `api_server down` |

v1 **tidak** membawa alasan detail ("socket tidak ada di ~/.hermes/gateway.sock") —
string itu hanya ada di log (`$XDG_DATA_HOME/coucou/coucou.log`). Menambahkannya
berarti mengubah struct event di Rust; ditunda sampai ada yang benar-benar butuh.

Interaksi: klik nama di health row = `setFocus` ke pill agent (kalau ada). Tidak ada
kartu detail, tidak ada tombol.

## 6. Aturan implementationsional (guardrail)

1. **Tanpa polling di front end.** Health datang dari event Rust; tidak ada
   `setInterval` di island.
2. **Dedupe di `agents.ts`.** Adapter mengirim `State::Unavailable` tiap poll (5 s)
   selama offline (`hermes.rs:53`). Island hanya bereaksi saat state benar-benar
   berubah — satu `Map<agent, state>` di handler. Tanpa ini: redraw 0.2×/detik per
   agent saat offline.
3. **Tanpa suara** untuk event agent (Claude Code tetap berbunyi). Emote/animasi
   hanya lewat state yang sudah ada.
4. **Izin hanya Claude Code** — tidak ada jalur kode baru untuk itu; `PermissionReq`
   dari agent tidak pernah sampai ke handler.
5. **Saat island tersembunyi**, `#content.away *` sudah menghentikan animasi; tidak
   ada timer baru yang ditambahkan.
6. **0 Rust change** di v1.

## 7. File yang disentuh

| File | Perubahan |
|---|---|
| `app/src/island/agents.ts` | **baru** — subscribe `"agent"`, petakan ke `State` (reuses `upsertExternalAgent`, `updateTask`, `appendStep`, `setPillBadge`, `removeTask`) |
| `app/src/main.ts` | 1 baris: `registerAgentHandlers(island)` |
| `app/src/core/state.ts` | `sessionActive` dibuat berlaku untuk agent pill juga (sekarang hanya `integration_claude`), supaya kartu kiri memakai ticker yang sama |
| `app/src/views/views.ts` | `sourceLabel()` memakai nama platform |
| `app/src/style.css` | blok `.agent-health` (sudah ada di preview, dipindah) |

Tidak ada test baru: tidak ada guardrail baru, dan `bun run build` +
`cargo test` yang ada sudah menutup regresi. Bukti manual lewat skenario preview.

## 8. Acceptance criteria

1. `bun run build` (tsc + vite) hijau, tanpa perubahan di Rust.
2. Preview `/design-preview/agent-pills.html` memakai fungsi mapping yang sama dengan
   handler produksi, jadi yang di-preview bukan mock terpisah.
3. Dengan `opencode serve` aktif: pill OpenCode muncul < 1 s setelah `session.created`.
4. Hermes mati → tidak ada pill, health row `not running`, island tetap 0 % CPU.
5. `#island` tetap 640 × 160 (tidak ada scrollbar, tidak ada baris terpotong di 4 pill).

## 9. Di luar lingkup v1

- health row (§5), naikkan tinggi island, divider pills, suara per agent,
  "kenapa mati" detail, pill per session, sorting by state, Rust dedupe/backoff/jitter,
  stub agent di `verify_coucou_linux.sh`.
- `app/design-preview/` dihapus setelah fitur ini hidup — harness ini hanya cara
  cheaply melihat UI tanpa Tauri, bukan aset.

## 10. Keputusan yang sudah diambil

1. Label pill = **Freebuff** (satu adapter untuk dua CLI).
2. Island **tetap 640 × 160**; pills maksimum 4 (2 baris).
3. Claude Code oranye `#D97757`; Resend nonaktif, Notion aktif.
4. Alasan "kenapa mati" tidak di v1.

## 11. Yang masih perlu diputuskan

1. **Health row**: hapus (rekomendasi §5), atau tetap selalu-on?
2. **`cwd` untuk tombol ↗**: `SessionStart` membawa `cwd`, tapi kalau island baru
   hidup saat agent sudah jalan, `cwd` bisa kosong → tombol ↗ nonaktif. Setuju?