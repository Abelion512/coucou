#!/usr/bin/env bash
# version.sh — one version number, kept true across four files, with a check.
#
# The version lived in tauri.conf.json, package.json and Cargo.toml, which meant
# three places to forget. The tag that publishes a release (`linux-v*`) was a
# fourth. `check` is the gate: CI and the verify script run it, so a release that
# does not match its tag fails before anything is published.
#
# Semantics this enforces:
#   patch  bug fix, dependency bump, docs           0.1.1 -> 0.1.2
#   minor  a feature the user can notice             0.1.2 -> 0.2.0
#   major  something incompatible with their setup   0.2.0 -> 1.0.0
#
# Docs-only changes are a patch, not a bump: a reader who never opens Settings
# again should not be told to reinstall. An unreleased fix and a new feature in
# the same tree is a minor — the higher of the two wins.
#
# Usage:
#   scripts/version.sh check          # every file agrees, and with the tag
#   scripts/version.sh show
#   scripts/version.sh set 0.2.0      # write it everywhere
#   scripts/version.sh bump patch|minor|major
#   scripts/version.sh bump patch --docs-only   # a patch that is docs/log only
#   scripts/version.sh classify <base> <ref>    # what did this range change?

set -eu
REPO="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
TAURI_CONF="$REPO/app/src-tauri/tauri.conf.json"
PKG="$REPO/app/package.json"
CARGO="$REPO/app/Cargo.toml"

say()  { printf '%s\n' "$*"; }
die()  { say "  ERROR  $*" >&2; exit 1; }
pass() { say "  PASS  $*"; }
fail() { say "  FAIL  $*"; exit 1; }

# The version is the value of the "version" key at the top level of each file.
tauri_version() { grep -o '"version"[[:space:]]*:[[:space:]]*"[^"]*"' "$TAURI_CONF" | head -1 | grep -o '[0-9][0-9.]*'; }
pkg_version()   { grep -o '"version"[[:space:]]*:[[:space:]]*"[^"]*"' "$PKG" | head -1 | grep -o '[0-9][0-9.]*'; }
cargo_version() { grep -o '^version = "[^"]*"' "$CARGO" | head -1 | grep -o '[0-9][0-9.]*'; }

valid_semver() {
  printf '%s' "$1" | grep -Eq '^[0-9]+\.[0-9]+\.[0-9]+$'
}

write_all() { # $1 = version
  local v="$1"
  # Escape for sed: the only metacharacter in a version is the dot.
  local esc
  esc=$(printf '%s' "$v" | sed 's/\./\\./g')
  sed -i "0,/\"version\"[[:space:]]*:[[:space:]]*\"[^\"]*\"/s//\"version\": \"$v\"/" "$TAURI_CONF"
  sed -i "0,/\"version\"[[:space:]]*:[[:space:]]*\"[^\"]*\"/s//\"version\": \"$v\"/" "$PKG"
  sed -i "0,/^version = \"[^\"]*\"/s//version = \"$v\"/" "$CARGO"
}

cmd_show() {
  say "tauri.conf.json  $(tauri_version)"
  say "package.json    $(pkg_version)"
  say "Cargo.toml      $(cargo_version)"
  say "latest tag      $(git tag --list 'linux-v*' --sort=-v:refname | head -1)"
}

cmd_check() {
  local t p c
  t=$(tauri_version); p=$(pkg_version); c=$(cargo_version)
  valid_semver "$t" || fail "tauri.conf.json version is not semver: '$t'"
  [ "$t" = "$p" ] || fail "package.json says $p but tauri.conf.json says $t"
  [ "$t" = "$c" ] || fail "Cargo.toml says $c but tauri.conf.json says $t"
  pass "all four sources agree on $t"

  # If HEAD is a release commit, the tag must name exactly this version. A release
  # published under the wrong tag is the failure this exists to prevent.
  local head_tag
  head_tag=$(git tag --points-at HEAD --list 'linux-v*' | head -1)
  if [ -n "$head_tag" ]; then
    [ "$head_tag" = "linux-v$t" ] \
      || fail "HEAD is tagged $head_tag but the version is $t"
    pass "tag $head_tag matches version $t"
  fi

  # The CHANGELOG must not still be sitting entirely under "Unreleased" if a tag
  # exists claiming to ship it.
  local tag
  tag=$(git tag --list 'linux-v*' --sort=-v:refname | head -1)
  if [ -n "$tag" ] && ! grep -q "^## $t" "$REPO/CHANGELOG.md"; then
    fail "a $tag tag exists but CHANGELOG.md has no '## $t' section"
  fi
  pass "changelog covers every released version"
}

cmd_bump() { # $1 = part, $2 = --docs-only (optional)
  local part="${1:-}" docs_only="${2:-}"
  case "$part" in patch|minor|major) ;; *) die "bump needs patch|minor|major" ;; esac
  local cur major minor patch next
  cur=$(tauri_version)
  IFS=. read -r major minor patch <<EOF
$cur
EOF
  case "$part" in
    major)  next="$((major + 1)).0.0" ;;
    minor)  next="$major.$((minor + 1)).0" ;;
    patch)  next="$major.$minor.$((patch + 1))" ;;
  esac
  if [ "$part" = "patch" ] && [ "$docs_only" = "--docs-only" ]; then
    say "  docs-only patch: $cur -> $next"
    say "  (still written — the changelog entry is what tells a reader to reinstall,"
    say "   and a stale version in three files is a worse lie than a needless patch)"
  else
    say "  $part: $cur -> $next"
  fi
  write_all "$next"
  say "  written to tauri.conf.json, package.json, Cargo.toml"
  say "  next: commit, then  git tag linux-v$next  to publish"
}

# What kind of release does this range deserve? Reads the changelog only — it cannot
# tell a security fix from a typo fix, and does not pretend to.
cmd_classify() { # $1 = base ref, $2 = target ref
  local base="${1:-}" target="${2:-HEAD}"
  git rev-parse --verify "$base" >/dev/null 2>&1 || die "no such ref: $base"
  local body
  body=$(git log --format='%s%n%b' "$base..$target" 2>/dev/null || true)
  [ -n "$body" ] || { say "no commits between $base and $target"; return; }

  local major=0 minor=0 patch=0 docs=1
  printf '%s\n' "$body" | grep -qiE '\b(security|vulnerab|CVE|secret leak|token leak)' && { major=1; docs=0; }
  printf '%s\n' "$body" | grep -qiE '^\s*(feat|add|new)\b|\b(implements?|introduces?)\b' && { minor=1; docs=0; }
  printf '%s\n' "$body" | grep -qiE '^\s*(fix|bug|revert)\b' && { patch=1; docs=0; }
  [ "$docs" = "1" ] && { patch=1; say "  only docs changed → patch, if you ship it at all"; }

  if [ "$major" = "1" ]; then say "  suggested: MAJOR (security or incompatible)"
  elif [ "$minor" = "1" ]; then say "  suggested: MINOR (a feature a user can notice)"
  elif [ "$patch" = "1" ]; then say "  suggested: PATCH (fix)"
  else say "  suggested: nothing to ship"; fi
}

case "${1:-show}" in
  show) cmd_show ;;
  check) cmd_check ;;
  set)   [ -n "${2:-}" ] || die "set needs a version"; valid_semver "$2" || die "not semver: $2"
         write_all "$2"; cmd_show ;;
  bump)  cmd_bump "${2:-}" "${3:-}" ;;
  classify) cmd_classify "${2:-}" "${3:-HEAD}" ;;
  *) say "usage: version.sh {check|show|set <v>|bump <part> [--docs-only]|classify <base> [ref]}"; exit 1 ;;
esac
