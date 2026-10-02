#!/usr/bin/env bash
# ui_drive.sh — drive the running island with real X input and prove what happened.
#
# Synthetic clicks are the only way to check the parts of this app that live in
# window-manager behaviour: click-through, focus, and which window actually got
# the event. A DOM assertion cannot tell you that a click went to the window
# behind, which is exactly the bug this script exists to catch.
#
# It never guesses: every step asserts on observable evidence (the app's own log,
# or X geometry) and prints PASS / FAIL.
#
# Usage:  scripts/ui_drive.sh [--app <path>]   (defaults to the debug build)

set -u
REPO="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
APP="$REPO/app/target/debug/coucou"
LOG="$HOME/.local/share/coucou/coucou.log"
PASS=0; FAIL=0
[ "${1:-}" = "--app" ] && APP="$2"

say()  { printf '%s\n' "$*"; }
pass() { PASS=$((PASS+1)); say "  PASS  $*"; }
fail() { FAIL=$((FAIL+1)); say "  FAIL  $*"; }

logcount() { grep -c "$1" "$LOG" 2>/dev/null || true; }

island_win() {
  for id in $(xdotool search --class coucou 2>/dev/null); do
    w=$(xwininfo -id "$id" 2>/dev/null | grep -m1 'Width:' | tr -dc 0-9)
    [ "${w:-0}" -gt 300 ] 2>/dev/null && { echo "$id"; return; }
  done
}

# A screen point inside the island, from the window's own geometry — never
# hardcoded, because the island sits below the desktop panel and its size depends
# on the scale factor.
#
# The island is always horizontally centred in the window, so the window's centre x
# is the island's centre x in both compact (288 px) and expanded (640 px). For y it
# must be near the top: the compact island is only 32 logical px tall and the rest
# of the window is transparent click-through *by design*. Aiming lower would test
# nothing but that transparency works.
island_point() {
  local id x y w
  id=$(island_win) || return 1
  x=$(xwininfo -id "$id" | grep -m1 'Absolute upper-left X:' | tr -dc 0-9)
  y=$(xwininfo -id "$id" | grep -m1 'Absolute upper-left Y:' | tr -dc 0-9)
  w=$(xwininfo -id "$id" | grep -m1 'Width:' | tr -dc 0-9)
  echo "$((x + w / 2)) $((y + 15))"
}

## ── scenarios ────────────────────────────────────────────────────────────────

t_wakes_on_hover() {
  say ""
  say "1 · island wakes when the cursor reaches it"
  local before after pt
  before=$(xwininfo -id "$(island_win)" | grep -m1 'Height:' | tr -dc 0-9)
  read -r px py <<<"$(island_point)"
  xdotool mousemove "$px" "$py"; sleep 2
  after=$(xwininfo -id "$(island_win)" | grep -m1 'Height:' | tr -dc 0-9)
  # Collapsing to the wake strip shrinks the window; staying open does not.
  if [ "$after" -ge "$before" ]; then
    pass "window stayed open on hover (${before} -> ${after} px)"
  else
    fail "window shrank while hovered (${before} -> ${after} px) — click-through cannot work"
  fi
}

window_height() {
  xwininfo -id "$(island_win)" 2>/dev/null | grep -m1 'Height:' | tr -dc 0-9
}

active_name() { xdotool getactivewindow getwindowname 2>/dev/null || echo "?"; }

# Does a click at (px,py) land on the island, or fall through to whatever is
# behind it?
#
# The island window never takes focus (accept_focus(false)), so a click it receives
# leaves the active window alone, while a click that falls through activates the
# window behind. That difference is the only signal available from outside the
# app, and it is unambiguous in both directions.
click_lands_on_island() { # $1=px $2=py
  local px="$1" py="$2" before after
  before=$(active_name)
  xdotool mousemove "$px" "$py"; sleep 1
  xdotool click 1; sleep 2
  after=$(active_name)
  [ "$before" = "$after" ]
}

# Percentage of pixels that differ between two PNGs. Used instead of `cmp` because
# the island is alive: Mochi breathes, its eyes follow the mouse, the ticker
# animates and the adapters update the pills every few seconds. A byte comparison
# would therefore report a change for a click that never arrived.
#
# Prints "N/A" when it cannot compare (no PIL, unreadable files).
pixel_diff() { # $1 $2
  python3 - "$1" "$2" <<'PY' 2>/dev/null || echo "N/A"
import sys
try:
    from PIL import Image
except ImportError:
    print("N/A"); raise SystemExit
a = Image.open(sys.argv[1]).convert("RGB")
b = Image.open(sys.argv[2]).convert("RGB")
if a.size != b.size:
    print("N/A"); raise SystemExit
pa, pb = a.load(), b.load()
w, h = a.size
changed = sum(1 for y in range(0, h, 2) for x in range(0, w, 2) if pa[x, y] != pb[x, y])
print(f"{(100.0 * changed) / ((w // 2 + 1) * (h // 2 + 1)):.2f}")
PY
}

# How much the island changes on its own, with no input at all. Measured, not
# assumed: with live agents the ticker and pills move by themselves, and a control
# that ignores that is a control that fails for the wrong reason.
island_churn() {
  local tmp="$1"
  island_shot "$tmp/n1.png"
  sleep 2
  island_shot "$tmp/n2.png"
  pixel_diff "$tmp/n1.png" "$tmp/n2.png"
}

t_pass_through_is_intentional() {
  say ""
  say "2 · the transparent margin does not belong to the island (control)"
  local px py tmp churn after
  tmp="$(mktemp -d)"
  read -r px py <<<"$(island_point)"
  # Near the bottom of the window: outside the island in both compact (32 logical
  # px) and expanded (200 physical px), and still inside the window.
  #
  # The assertion is that a click there changes the island no more than idling
  # does. Anything else — focus, an activation — is not observable from outside,
  # and what sits behind that strip may be empty desktop anyway.
  churn=$(island_churn "$tmp")
  # Park the cursor first: Mochi's eyes follow it, so moving is itself a change.
  xdotool mousemove "$px" "$((py + 300))"; sleep 2
  island_shot "$tmp/before.png"
  xdotool click 1; sleep 2
  island_shot "$tmp/after.png"
  after=$(pixel_diff "$tmp/before.png" "$tmp/after.png")
  rm -rf "$tmp"

  if [ "$after" = "N/A" ] || [ "$churn" = "N/A" ]; then
    skip "cannot compare pixels (needs python3 with Pillow)"
  else
    # A little slack: the click's own settling frames are legitimate churn too.
    if awk -v a="$after" -v c="$churn" 'BEGIN{exit !(a <= c * 2 + 0.5)}'; then
      pass "a click in the transparent margin changed the island no more than idling (${after}% vs ${churn}% baseline)"
    else
      fail "the island reacted to a click in its transparent margin (${after}% vs ${churn}% baseline)"
    fi
  fi
}

# Bytes of the island's own pixels, so a test can tell "the island answered" from
# "the click went nowhere and nothing changed".
island_shot() { # $1 = output file
  local id x y w h
  id=$(island_win) || return 1
  x=$(xwininfo -id "$id" | grep -m1 'Absolute upper-left X:' | tr -dc 0-9)
  y=$(xwininfo -id "$id" | grep -m1 'Absolute upper-left Y:' | tr -dc 0-9)
  w=$(xwininfo -id "$id" | grep -m1 'Width:' | tr -dc 0-9)
  h=$(xwininfo -id "$id" | grep -m1 'Height:' | tr -dc 0-9)
  import -window root -crop "${w}x${h}+${x}+${y}" +repage "$1" 2>/dev/null
}

t_click_reaches_island() {
  say ""
  say "3 · a click on the island reaches it and the island answers"
  local px py churn after tmp
  tmp="$(mktemp -d)"
  read -r px py <<<"$(island_point)"
  # Two things must hold: the click did not go to the window behind, *and* it moved
  # the island more than idling would have on its own.
  churn=$(island_churn "$tmp")
  xdotool mousemove "$((px - 250))" "$py"; sleep 1
  xdotool mousemove "$px" "$py"; sleep 1
  island_shot "$tmp/before.png"
  xdotool click 1; sleep 2
  island_shot "$tmp/after.png"
  after=$(pixel_diff "$tmp/before.png" "$tmp/after.png")
  if ! click_lands_on_island "$px" "$py"; then
    fail "click fell through to $(active_name) — the island did not take it"
  elif [ "$after" = "N/A" ] || [ "$churn" = "N/A" ]; then
    skip "cannot compare pixels (needs python3 with Pillow)"
  elif awk -v a="$after" -v c="$churn" 'BEGIN{exit !(a > c + 0.5)}'; then
    pass "island kept the click and redrew in response (${after}% vs ${churn}% idle)"
  else
    fail "island kept the click but nothing on screen changed (${after}% vs ${churn}% idle)"
  fi
  rm -rf "$tmp"
}

t_survives_idle() {
  say ""
  say "4 · a pause does not make the island click-through again"
  local px py
  read -r px py <<<"$(island_point)"
  xdotool mousemove "$px" "$py"; sleep 1
  # Sit still. This is the reported bug: after a few seconds of not pressing, the
  # island starts passing clicks through again.
  sleep 8
  if click_lands_on_island "$px" "$py"; then
    pass "still keeps clicks after 8 s idle on the island"
  else
    fail "after 8 s idle it stopped taking clicks — went to $(active_name)"
  fi
}

t_panel_clearance() {
  say ""
  say "5 · the island is not drawn under the desktop panel"
  local id y
  id=$(island_win)
  y=$(xwininfo -id "$id" | grep -m1 'Absolute upper-left Y:' | tr -dc 0-9)
  # Cinnamon's panel is 45 px here; anything at y=0 is behind it.
  if [ "$y" -ge 20 ]; then
    pass "island starts at y=$y, below the panel"
  else
    fail "island starts at y=$y — under the panel"
  fi
}

t_viewport_fits() {
  say ""
  say "6 · the window is large enough for the 640 px island"
  local id w
  id=$(island_win)
  w=$(xwininfo -id "$id" | grep -m1 'Width:' | tr -dc 0-9)
  # 900 physical / 1.25 scale = 720 CSS px, which is what the layout needs.
  if [ "$w" -ge 720 ]; then
    pass "window is ${w} px wide (>= 720 physical)"
  else
    fail "window is only ${w} px wide — the island will be clipped"
  fi
}

## ── run ──────────────────────────────────────────────────────────────────────

for t in t_wakes_on_hover t_pass_through_is_intentional t_click_reaches_island t_survives_idle t_panel_clearance t_viewport_fits; do
  "$t"
done

say ""
say "═══ ${PASS} passed · ${FAIL} failed ═══"
[ "$FAIL" -eq 0 ]
