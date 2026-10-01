#!/usr/bin/env bash
# sync_upstream.sh — keep this fork aligned with Louis-CFM/coucou, cheaply.
#
# Modes:
#   --check   (default, read-only) analyse the next sync and rewrite SYNC-TODO.md:
#             files that may conflict, upstream fixes worth porting, and what to do.
#   --merge   do the merge: auto-stash WIP if needed, merge upstream/main, restore.
#             On conflict it leaves the merge in progress and tells you (or your AI
#             agent) exactly which files to resolve and how to finish.
#
# The script never rebases, never resets, never discards anything.

set -u

UPSTREAM_URL="https://github.com/Louis-CFM/coucou.git"
UPSTREAM="upstream"
UPSTREAM_BRANCH="main"
TODO_FILE="SYNC-TODO.md"

# Files only the fork touches → can never conflict with upstream.
FORK_ONLY='docs/LINUX.md|docs/SPEC-linux-mult-agent.md|scripts/verify_coucou_linux.sh|scripts/sync_upstream.sh|SYNC-TODO.md'

MODE="check"
[ "${1:-}" = "--merge" ] && MODE="merge"
if [ "${1:-}" = "--help" ] || [ "${1:-}" = "-h" ]; then sed -n '2,16p' "${BASH_SOURCE[0]}"; exit 0; fi

cd "$(git rev-parse --show-toplevel 2>/dev/null || echo .)" || exit 1

if ! git rev-parse --git-dir >/dev/null 2>&1; then
    echo "error: not inside a git repository" >&2; exit 1
fi

git remote get-url "$UPSTREAM" >/dev/null 2>&1 || {
    echo "adding remote $UPSTREAM → $UPSTREAM_URL"
    git remote add "$UPSTREAM" "$UPSTREAM_URL"
}

echo "fetching $UPSTREAM …"
git fetch "$UPSTREAM" --prune --quiet || { echo "error: fetch failed" >&2; exit 1; }

BASE=$(git merge-base HEAD "$UPSTREAM/$UPSTREAM_BRANCH") || { echo "error: no common ancestor" >&2; exit 1; }
BEHIND=$(git rev-list --count HEAD.."$UPSTREAM/$UPSTREAM_BRANCH")
AHEAD=$(git rev-list --count "$UPSTREAM/$UPSTREAM_BRANCH"..HEAD)

echo ""
echo "═══ upstream sync status ═══"
echo "  base:     $(git log -1 --format='%h %s' "$BASE" | cut -c1-72)"
echo "  ahead:    $AHEAD commit(s) di fork"
echo "  behind:   $BEHIND commit(s) di upstream"
[ "$BEHIND" -eq 0 ] && { echo "  → sudah sinkron, tidak ada yang perlu dilakukan."; exit 0; }

# ── overlap analysis ──────────────────────────────────────────────────────────
CHANGED_UP=$(git diff --name-only "$BASE".."$UPSTREAM/$UPSTREAM_BRANCH")
CHANGED_FORK=$(git diff --name-only "$BASE"..HEAD)
OVERLAP=$(comm -12 <(echo "$CHANGED_FORK" | sort) <(echo "$CHANGED_UP" | sort))
# WIP yang penting: perubahan TERTRACK saja. Untracked baru (termasuk file buatan
# script ini sendiri) tidak bisa mengganggu merge, dan stash default pun tidak
# menyentuh mereka — menghitungnya hanya menghasilkan stash kosong yang menyesatkan.
WIP=$(git status --porcelain | grep -v '^??' || true)

# Upstream log sejak base, untuk deteksi fix yang mungkin perlu dicerminkan.
UP_LOG=$(git log --format='%h %s' "$BASE".."$UPSTREAM/$UPSTREAM_BRANCH")

write_todo() {
    cat > "$TODO_FILE" <<EOF
# SYNC-TODO — dibuat otomatis oleh scripts/sync_upstream.sh
#
# $(date '+%Y-%m-%d %H:%M') · behind $BEHIND / ahead $AHEAD · base $BASE
#
# Baca ini SEBELUM mulai editing. Agent AI di repo ini juga wajib membacanya
# (lihat CLAUDE.md). Hapus file ini setelah sync selesai.
EOF

    if [ -n "$WIP" ]; then
        {
            echo ""
            echo "## ⚠ WIP belum di-commit (jangan merge sambil begini)"
            echo ""
            echo '```'
            echo "$WIP"
            echo '```'
            echo ""
            echo "- [ ] commit dulu (\`git add -A && git commit -m wip\`) atau \`git stash push -m wip\`"
        } >> "$TODO_FILE"
    fi

    if [ -n "$OVERLAP" ]; then
        {
            echo ""
            echo "## 🔶 File yang berubah di KEDUA sisi — zona konflik nyata"
            echo ""
            echo "Resolusinya: sisi fork biasanya menang untuk kode Linux; cek diff upstream"
            echo "per file (\`git diff $BASE..$UPSTREAM/$UPSTREAM_BRANCH -- <file>\`) dan"
            echo "cerminkan fix-nya ke sisi Unix bila relevan."
            echo ""
            echo '```'
            echo "$OVERLAP"
            echo '```'
            echo ""
        } >> "$TODO_FILE"
        i=0
        while IFS= read -r f; do
            [ -z "$f" ] && continue
            i=$((i+1))
            echo "- [ ] resolusi manual: \`$f\` (lihat \`git diff $BASE..$UPSTREAM/$UPSTREAM_BRANCH -- $f\`)" >> "$TODO_FILE"
        done <<< "$OVERLAP"
    else
        {
            echo ""
            echo "## ✅ Tidak ada overlap file fork↔upstream sejak base"
            echo ""
            echo "Merge seharusnya bersih tanpa konflik:"
            echo ""
            echo '```'
            echo "./scripts/sync_upstream.sh --merge"
            echo '```'
            echo ""
        } >> "$TODO_FILE"
    fi

    {
        echo ""
        echo "## Upstream commits sejak base"
        echo ""
        echo '```'
        echo "$UP_LOG"
        echo '```'
        echo ""
        echo "## Checklist sync"
        echo ""
        echo "- [ ] baca daftar commit di atas; tandai fix macOS yang layak di-porting ke sisi Unix"
        echo "- [ ] jalankan \`./scripts/sync_upstream.sh --merge\`"
        echo "- [ ] selesaikan semua item 🔶 di atas (kalau ada)"
        echo "- [ ] \`cargo test -p coucou --lib && cargo test -p coucou-hook\` (di app/)"
        echo "- [ ] \`./scripts/verify_coucou_linux.sh --no-build\`"
        echo "- [ ] commit merge, hapus file ini"
    } >> "$TODO_FILE"
}

case "$MODE" in
check)
    write_todo
    echo ""
    echo "analisis ditulis ke $TODO_FILE:"
    if [ -n "$OVERLAP" ]; then
        echo "  🔶 $OVERLAP file berpotensi konflik"
    else
        echo "  ✅ tidak ada file yang berubah di kedua sisi — merge akan bersih"
    fi
    [ -n "$WIP" ] && echo "  ⚠ ada WIP belum di-commit"
    echo ""
    echo "lanjutkan dengan: ./scripts/sync_upstream.sh --merge"
    ;;
merge)
    if [ -n "$WIP" ]; then
        echo "working tree kotor → auto-stash …"
        STASHED=1
        git stash push -m "sync_upstream auto-stash $(date '+%F %T')" --quiet || { echo "error: stash gagal" >&2; exit 1; }
    else
        STASHED=0
    fi

    write_todo   # segarkan TODO sebelum merge, agar checklist siap dipakai saat konflik

    MERGE_OUT=$(git merge --no-edit "$UPSTREAM/$UPSTREAM_BRANCH" 2>&1)
    MERGE_RC=$?
    if [ $MERGE_RC -eq 0 ]; then
        echo "$MERGE_OUT"
        echo ""
        echo "✅ merge bersih. Unit tests:"
        if command -v cargo >/dev/null 2>&1 || [ -x "$HOME/.cargo/bin/cargo" ]; then
            (cd app && cargo test -p coucou-hook --quiet >/dev/null 2>&1 \
                && echo "  ✅ coucou-hook tests" || echo "  ❌ coucou-hook tests — jalankan manual: (cd app && cargo test -p coucou-hook)")
        else
            echo "  (cargo tidak ditemukan — jalankan test manual)"
        fi
        echo ""
        echo "selesaikan checklist di $TODO_FILE, lalu hapus filenya."
        [ "$STASHED" -eq 1 ] && { echo "restoring stash …"; git stash pop; }
        exit 0
    fi

    if [ $MERGE_RC -ne 1 ]; then
        # Merge tidak pernah dimulai (tree kotor, remote rusak, dst) — bukan konflik.
        echo "❌ merge gagal dimulai (exit $MERGE_RC):"
        echo "$MERGE_OUT"
        [ "$STASHED" -eq 1 ] && { echo "memulihkan stash …"; git stash pop; }
        exit 1
    fi

    echo ""
    echo "🔶 KONFLIK — merge masih berjalan. File yang harus diresolusikan:"
    echo ""
    git diff --name-only --diff-filter=U | sed 's/^/    /'
    echo ""
    cat <<HINT
Cara selesai:
  1. untuk tiap file: sisi fork (Linux) biasanya menang, tapi cerminkan fix upstream
     — lihat: git diff $BASE..$UPSTREAM/$UPSTREAM_BRANCH -- <file>
  2. git add <file-yang-sudah>
  3. git commit --no-edit        (menyelesaikan merge; JANGAN reset/abort kecuali bermaksud)
  4. cargo test di app/, lalu ./scripts/verify_coucou_linux.sh --no-build
  5. stash dipulihkan otomatis hanya saat merge bersih; setelah konflik selesai:
     git stash pop
HINT
    [ "$STASHED" -eq 1 ] && echo "⚠ WIP-mu aman di stash (\`git stash list\`) — pop setelah merge selesai."
    exit 2
    ;;
esac
