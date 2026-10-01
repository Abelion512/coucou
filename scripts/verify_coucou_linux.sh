#!/usr/bin/env bash
# verify_coucou_linux.sh — live verification for the Coucou Linux port.
#
# Implements the checks from docs/SPEC-linux-mult-agent.md §6. Every step
# prints PASS / FAIL / SKIP; SKIP means "cannot be tested right now" (agent
# not running on this machine), never a failure. The script never modifies
# anything outside its own temp files: it builds, probes and reads only.
#
# Usage:
#   scripts/verify_coucou_linux.sh              # build (if needed) + all checks
#   scripts/verify_coucou_linux.sh --no-build   # skip the cargo build steps
#
# Exit code: 0 when nothing FAILED (SKIPs are allowed), 1 otherwise.

set -u

REPO="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
HOOK_BIN="$REPO/app/target/release/coucou-hook"
APP_LIB_DIR="$REPO/app/target/release"
PASS=0; FAIL=0; SKIP=0

CARGO=""
if command -v cargo >/dev/null 2>&1; then CARGO="cargo"
elif [ -x "$HOME/.cargo/bin/cargo" ]; then CARGO="$HOME/.cargo/bin/cargo"; PATH="$HOME/.cargo/bin:$PATH"; fi

say()  { printf '%s\n' "$*"; }
pass() { PASS=$((PASS+1)); say "  PASS  $*"; }
fail() { FAIL=$((FAIL+1)); say "  FAIL  $*"; }
skip() { SKIP=$((SKIP+1)); say "  SKIP  $*"; }

# Socket talk without hard dependencies: python3 if present, else nc -U.
socket_send() { # $1 = socket path, $2 = payload (one line), $3 = read timeout s
    if command -v python3 >/dev/null 2>&1; then
        python3 - "$1" "$2" "$3" <<'PYEOF'
import socket, sys
path, payload, timeout = sys.argv[1], sys.argv[2], float(sys.argv[3])
s = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
s.settimeout(timeout)
try:
    s.connect(path)
    s.sendall(payload.encode())
    data = b""
    while True:
        chunk = s.recv(65536)
        if not chunk: break
        data += chunk
    sys.stdout.write(data.decode(errors="replace"))
except Exception:
    pass
PYEOF
    elif command -v nc >/dev/null 2>&1 && nc -h 2>&1 | grep -q -- -U; then
        printf '%s\n' "$2" | nc -U -w "$3" "$1"
    else
        return 2   # no tool available
    fi
}

relay_socket_path() {
    if [ -n "${XDG_RUNTIME_DIR:-}" ]; then
        local p="$XDG_RUNTIME_DIR/coucou/coucou.sock"
        if [ ${#p} -le 107 ]; then echo "$p"; return; fi
    fi
    echo "/tmp/coucou-$(id -u).sock"
}

say ""
say "═══ Coucou Linux port — live verification (spec §6) ═══"
say "Machine: $(uname -sr) · session: ${XDG_SESSION_TYPE:-unknown} · uid: $(id -u)"
say ""

# ── 1. Build ──────────────────────────────────────────────────────────────────
say "1 · Build"
if [ "${1:-}" = "--no-build" ]; then
    skip "build steps skipped by request"
elif [ -z "$CARGO" ]; then
    if [ -x "$HOOK_BIN" ]; then
        pass "coucou-hook binary already built ($HOOK_BIN) — cargo not found"
    else
        fail "no cargo and no prebuilt hook binary — install rustup, then rerun"
    fi
else
    if (cd "$REPO/app" && "$CARGO" build --release -p coucou-hook >/dev/null 2>&1); then
        pass "cargo build --release -p coucou-hook"
    else
        fail "cargo build --release -p coucou-hook"
    fi
fi
if [ -x "$HOOK_BIN" ]; then
    pass "hook binary present: $HOOK_BIN ($(du -h "$HOOK_BIN" | cut -f1))"
else
    fail "hook binary missing — every relay test below will fail"
fi
say ""

# ── 2. Claude Code is never blocked when Coucou is closed ────────────────────
say "2 · Hook never blocks a closed app (the golden rule)"
if [ -x "$HOOK_BIN" ]; then
    START=$(date +%s%N)
    OUT=$(echo '{"hook_event_name":"SessionStart","cwd":"/tmp"}' | timeout 5 "$HOOK_BIN" SessionStart 2>/dev/null)
    RC=$?
    ELAPSED_MS=$(( ($(date +%s%N) - START) / 1000000 ))
    if [ "$RC" -eq 0 ] && [ -z "$OUT" ] && [ "$ELAPSED_MS" -lt 2000 ]; then
        pass "closed app → exit 0, empty stdout, ${ELAPSED_MS} ms"
    else
        fail "closed app → exit=$RC stdout=[${OUT}] ${ELAPSED_MS} ms"
    fi
else
    skip "hook binary missing"
fi
say ""

# ── 3. Relay wire protocol ────────────────────────────────────────────────────
say "3 · Relay wire protocol (hook ⇄ socket)"
SOCK="$(relay_socket_path)"
if [ -S "$SOCK" ]; then
    say "  (live socket found at $SOCK — testing against the running app)"
    OUT=$(echo '{"hook_event_name":"SessionStart","cwd":"/tmp"}' | timeout 5 "$HOOK_BIN" SessionStart 2>/dev/null)
    RC=$?
    if [ "$RC" -eq 0 ]; then
        pass "fire-and-forget event accepted by the live app (exit 0)"
    else
        fail "live app refused a fire-and-forget event (exit $RC)"
    fi
else
    say "  (app not running — exercising the protocol with a throwaway listener)"
    if command -v python3 >/dev/null 2>&1; then
        SOCK_DIR="$(dirname "$SOCK")"
        MADE_DIR=0; [ -d "$SOCK_DIR" ] || { mkdir -p "$SOCK_DIR"; MADE_DIR=1; }
        FAKE="$(mktemp /tmp/coucou-verify-XXXXXX.py)"
        cat > "$FAKE" <<'PYEOF'
import socket, os, sys, threading, json
path = sys.argv[1]
if os.path.exists(path): os.unlink(path)
s = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
s.bind(path); os.chmod(path, 0o600); s.listen(4)
def handle(c):
    d = b""
    while b"\n" not in d:
        k = c.recv(65536)
        if not k: break
        d += k
    try: p = json.loads(d.decode())
    except Exception: p = {}
    if p.get("hook_event_name") == "PermissionRequest":
        c.sendall(b"allow\n")
    c.close()
while True:
    c, _ = s.accept()
    threading.Thread(target=handle, args=(c,), daemon=True).start()
PYEOF
        python3 "$FAKE" "$SOCK" 2>/dev/null & FAKE_PID=$!
        sleep 0.5
        OUT=$(echo '{"hook_event_name":"PermissionRequest","tool_name":"Bash","tool_input":{"command":"ls"},"cwd":"/tmp"}' | timeout 8 "$HOOK_BIN" PermissionRequest 2>/dev/null)
        RC=$?
        kill $FAKE_PID 2>/dev/null; wait $FAKE_PID 2>/dev/null
        rm -f "$FAKE" "$SOCK"
        [ "$MADE_DIR" -eq 1 ] && rmdir "$SOCK_DIR" 2>/dev/null
        if [ "$RC" -eq 0 ] && printf '%s' "$OUT" | grep -q '"hookSpecificOutput".*"decision":{"behavior":"allow"}'; then
            pass "PermissionRequest round-trip returned the documented hookSpecificOutput"
        else
            fail "PermissionRequest round-trip broken (exit=$RC stdout=[${OUT}])"
        fi
    else
        skip "python3 unavailable — cannot stage a throwaway listener"
    fi
fi
say ""

# ── 4. OpenCode adapter source ────────────────────────────────────────────────
say "4 · OpenCode (HTTP/SSE)"
OC_PORT="${OPENCODE_PORT:-54321}"
OC_BASE="http://127.0.0.1:$OC_PORT"
if command -v curl >/dev/null 2>&1; then
    HEALTH=$(curl -s -m 3 -o /dev/null -w '%{http_code}' "$OC_BASE/global/health" 2>/dev/null)
    if [ "$HEALTH" = "200" ]; then
        pass "GET /global/health → 200 on $OC_BASE"
        SESSIONS=$(curl -s -m 3 "$OC_BASE/session" 2>/dev/null | head -c 200000)
        if printf '%s' "$SESSIONS" | grep -q '"id"'; then
            COUNT=$(printf '%s' "$SESSIONS" | grep -o '"id":"ses_' | wc -l)
            pass "GET /session reachable ($COUNT+ session ids in first page)"
        else
            fail "GET /session did not look like a session list"
        fi
    elif pgrep -f "opencode serve" >/dev/null 2>&1; then
        fail "opencode serve is running but $OC_BASE did not answer — check its port"
    else
        skip "no opencode server on :$OC_PORT (start one: opencode serve --port $OC_PORT)"
    fi
else
    skip "curl unavailable"
fi
say ""

# ── 5. Hermes gateway socket ──────────────────────────────────────────────────
say "5 · Hermes (control socket)"
HERMES_SOCK="$HOME/.hermes/gateway.sock"
if [ -S "$HERMES_SOCK" ]; then
    REPLY=$(socket_send "$HERMES_SOCK" '{"verb":"status"}' 3)
    RC=$?
    if [ "$RC" = "2" ]; then
        skip "neither python3 nor nc -U available to talk to a Unix socket"
    elif printf '%s' "$REPLY" | grep -q '"gateway_state"'; then
        STATE=$(printf '%s' "$REPLY" | grep -o '"gateway_state":"[^"]*"' | head -1 | cut -d'"' -f4)
        AGENTS=$(printf '%s' "$REPLY" | grep -o '"active_agents":[0-9]*' | head -1 | cut -d: -f2)
        pass "status verb answered: gateway_state=$STATE active_agents=${AGENTS:-?}"
    else
        fail "socket exists but the status verb gave no gateway_state (reply: $(printf '%s' "$REPLY" | head -c 120))"
    fi
else
    skip "no socket at $HERMES_SOCK (hermes gateway not running)"
fi
say ""

# ── 6. Freebuff / Codebuff state files ────────────────────────────────────────
say "6 · Freebuff/Codebuff (manicode state)"
MANI="$HOME/.config/manicode"
if [ -d "$MANI" ]; then
    LIVE=$(find "$MANI" -maxdepth 1 -name 'freebuff-live-*.json' 2>/dev/null | wc -l)
    NOW_MS=$(date +%s%3N)
    ACTIVE=0
    for f in "$MANI"/freebuff-live-*.json; do
        [ -f "$f" ] || continue
        # Expired files are ignored — only unexpired ones count as live.
        if command -v python3 >/dev/null 2>&1; then
            python3 - "$f" "$NOW_MS" <<'PYEOF' && ACTIVE=$((ACTIVE+1))
import json, sys
try:
    v = json.load(open(sys.argv[1]))
    sys.exit(0 if int(v.get("expiresAt", 0)) > int(sys.argv[2]) else 1)
except Exception:
    sys.exit(1)
PYEOF
        else
            ACTIVE=$((ACTIVE+1))
        fi
    done
    if [ "$ACTIVE" -gt 0 ]; then
        pass "$ACTIVE live session file(s) under $MANI"
    elif [ "$LIVE" -gt 0 ]; then
        skip "$LIVE file(s) present but all expired — no live session right now"
    else
        skip "no freebuff-live-*.json — freebuff/codebuff not running"
    fi
else
    skip "no $MANI directory — freebuff/codebuff never ran here"
fi
say ""

# ── 7. Secrets never leak ─────────────────────────────────────────────────────
say "7 · Secrets never reach binary or disk"
if [ -x "$HOOK_BIN" ]; then
    if grep -aq "tokenKey" "$HOOK_BIN" 2>/dev/null; then
        fail "hook binary contains the string tokenKey"
    else
        pass "hook binary clean of tokenKey"
    fi
else
    skip "no hook binary to inspect"
fi
APP_BIN="$APP_LIB_DIR/coucou"
if [ -x "$APP_BIN" ]; then
    if grep -aq "tokenKey" "$APP_BIN" 2>/dev/null; then
        fail "app binary contains the string tokenKey"
    else
        pass "app binary clean of tokenKey"
    fi
else
    skip "app binary not built (build the app for this check)"
fi
LEAK=0
for d in "$HOME/.local/share/coucou" "$HOME/.config/coucou"; do
    [ -d "$d" ] || continue
    if grep -rqa "tokenKey" "$d" 2>/dev/null | grep -vq '^Binary'; then :; fi
    if grep -rqa "tokenKey" "$d" >/dev/null 2>&1; then LEAK=1; fail "state/log under $d mentions tokenKey"; fi
done
[ "$LEAK" -eq 0 ] && pass "no tokenKey in coucou state/log directories"
say ""

# ── 8. Hook install guardrails (static) ───────────────────────────────────────
say "8 · Hook install guardrails"
HOOKS_SRC="$REPO/app/src-tauri/src/hooks.rs"
if [ -f "$HOOKS_SRC" ]; then
    OK=1
    grep -q "settings.json.bak" "$HOOKS_SRC" || OK=0
    grep -q "changed since the preview" "$HOOKS_SRC" || OK=0
    grep -q "fingerprint" "$HOOKS_SRC" || OK=0
    if [ "$OK" -eq 1 ]; then
        pass "installer takes a dated backup and refuses a stale preview"
    else
        fail "backup/fingerprint guardrails missing from hooks.rs"
    fi
else
    skip "hooks.rs not found"
fi
BAKS=$(find "$HOME/.claude" -maxdepth 1 -name 'settings.json.bak-*' 2>/dev/null | wc -l)
say "  (info) existing backups of ~/.claude/settings.json: $BAKS"
say ""

# ── 9. Unit tests ─────────────────────────────────────────────────────────────
say "9 · Unit tests"
if [ -n "$CARGO" ] && [ "${1:-}" != "--no-build" ]; then
    if (cd "$REPO/app" && "$CARGO" test -p coucou-hook >/dev/null 2>&1); then
        pass "coucou-hook tests"
    else
        fail "coucou-hook tests"
    fi
    if (cd "$REPO/app" && "$CARGO" test -p coucou --lib >/dev/null 2>&1); then
        pass "coucou app tests"
    else
        fail "coucou app tests"
    fi
else
    skip "cargo unavailable or --no-build"
fi
say ""

# ── 10. Wayland / X11 (manual) ────────────────────────────────────────────────
say "10 · Display session (manual step)"
case "${XDG_SESSION_TYPE:-unknown}" in
    x11)     say "  (info) X11 session — the island and cursor poll should be fully live."; pass "X11 session detected" ;;
    wayland) say "  (info) Wayland — global cursor is unavailable by design; the tray is the way in. Launch the app and confirm it does not crash:"; say "         XDG_SESSION_TYPE=wayland app/target/release/coucou" ; skip "Wayland needs a manual GUI launch" ;;
    *)       skip "unknown session type — run the app and try the tray" ;;
esac
say ""

say "═══ Summary: $PASS passed · $FAIL failed · $SKIP skipped ═══"
[ "$FAIL" -eq 0 ]
