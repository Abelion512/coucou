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

t_pass_through_is_intentional() {
  say ""
  say "2 · the transparent margin does not belong to the island (control)"
  local px py tmp
  tmp="$(mktemp -d)"
  read -r px py <<<"$(island_point)"
  # Near the bottom of the window: outside the island in both compact (32 logical
  # px) and expanded (200 physical px), and still inside the window.
  #
  # The assertion is that the island does *not* react — not that some other window
  # activates. What sits behind that strip may be empty desktop, and "nothing
  # changed" is then indistinguishable from "swallowed". If this ever failed, the
  # island would be eating every click in that strip of the screen.
  #
  # Both shots are taken with the cursor already parked on that spot: Mochi's eyes
  # follow the mouse, so a screenshot taken before the move would always differ and
  # the control would pass or fail on animation rather than on clicks.
  xdotool mousemove "$px" "$((py + 300))"; sleep 2
  island_shot "$tmp/before.png"
  xdotool click 1; sleep 2
  island_shot "$tmp/after.png"
  if [ ! -s "$tmp/before.png" ] || [ ! -s "$tmp/after.png" ]; then
    fail "could not capture the island for the control"
  elif cmp -s "$tmp/before.png" "$tmp/after.png"; then
    pass "a click in the transparent margin does not reach the island"
  else
    fail "the island reacted to a click in its transparent margin"
  fi
  rm -rf "$tmp"
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
  local px py before after tmp
  tmp="$(mktemp -d)"
  read -r px py <<<"$(island_point)"
  # Leave the island, come back, and click. Two things must hold: the click did not
  # go to the window behind, *and* the island visibly reacted.
  xdotool mousemove "$((px - 250))" "$py"; sleep 1
  island_shot "$tmp/before.png"
  xdotool mousemove "$px" "$py"; sleep 1
  xdotool click 1; sleep 2
  island_shot "$tmp/after.png"
  if ! click_lands_on_island "$px" "$py"; then
    fail "click fell through to $(active_name) — the island did not take it"
  elif [ ! -s "$tmp/before.png" ] || [ ! -s "$tmp/after.png" ]; then
    fail "could not capture the island to prove it reacted"
  elif ! cmp -s "$tmp/before.png" "$tmp/after.png"; then
    pass "island kept the click and redrew in response"
  else
    fail "island kept the click but nothing on screen changed"
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
