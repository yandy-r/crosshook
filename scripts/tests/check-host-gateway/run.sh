#!/usr/bin/env bash
# Self-test for scripts/check-host-gateway.sh: runs the real script copy in isolated scratch repos.
set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
SRC_SCRIPT="$HERE/../../check-host-gateway.sh"
CRATES=src/crosshook-native/crates
GATEWAY=$CRATES/crosshook-core/src/platform/gateway.rs
ROOTS=(
  "$CRATES/crosshook-core/src"
  src/crosshook-native/src-tauri/src
  "$CRATES/crosshook-app/src"
  "$CRATES/crosshook-ui/src"
  "$CRATES/crosshook-widgets/src"
  "$CRATES/crosshook-view-model/src"
)
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT
FAILURES=0

new_repo() { # $1 = name; creates repo with script copy and gateway containing allowed literals
  local r="$TMP/$1"
  mkdir -p "$r/scripts" "$r/$(dirname "$GATEWAY")"
  cp "$SRC_SCRIPT" "$r/scripts/check-host-gateway.sh"
  cp "$HERE/denied.rs.fixture" "$r/$GATEWAY"
  echo "$r"
}

# expect <name> <repo> <want-exit> [stdout/stderr substring]
expect() {
  local name="$1" repo="$2" want="$3" needle="${4:-}" out rc=0
  out="$(bash "$repo/scripts/check-host-gateway.sh" 2>&1)" || rc=$?
  if [[ "$rc" != "$want" ]] || { [[ -n "$needle" ]] && [[ "$out" != *"$needle"* ]]; }; then
    echo "FAIL: $name (exit $rc, want $want, needle '$needle')" >&2
    echo "$out" >&2
    FAILURES=$((FAILURES + 1))
  else
    echo "ok: $name"
  fi
}

put() { mkdir -p "$(dirname "$1/$2")"; cp "$3" "$1/$2"; }

# Gateway-only repo (future dirs absent) is clean even though gateway holds every denied literal.
r="$(new_repo clean)"; expect "gateway excluded, absent future dirs tolerated" "$r" 0 "passed"

# Allowed fixture and suppression contract in every scan root.
r="$(new_repo allowed)"
for d in "${ROOTS[@]}"; do put "$r" "$d/ok.rs" "$HERE/allowed.rs.fixture"; put "$r" "$d/sup.rs" "$HERE/suppressed.rs.fixture"; done
expect "allowed + suppressed pass in all roots" "$r" 0 "passed"

# Each root is scanned.
for d in "${ROOTS[@]}"; do
  r="$(new_repo "root-${d//\//-}")"
  put "$r" "$d/bad.rs" "$HERE/denied.rs.fixture"
  expect "violation detected in $d" "$r" 1 "21 violation(s)"
done

# No blanket platform/ exclusion: sibling of gateway.rs is scanned.
r="$(new_repo sibling)"; put "$r" "$CRATES/crosshook-core/src/platform/other.rs" "$HERE/denied.rs.fixture"
expect "platform/ sibling is not excluded" "$r" 1 "platform/other.rs"

# Legacy platform.rs path is no longer exempt.
r="$(new_repo legacy)"; put "$r" "$CRATES/crosshook-core/src/platform.rs" "$HERE/denied.rs.fixture"
expect "legacy platform.rs not exempt" "$r" 1 "platform.rs"

# tests/ directories remain exempt (existing contract).
r="$(new_repo tests)"; put "$r" "$CRATES/crosshook-core/src/foo/tests/t.rs" "$HERE/denied.rs.fixture"
expect "tests/ dir exempt" "$r" 0 "passed"

# Missing gateway file is an error (symlink is invalid too).
r="$(new_repo nogw)"; rm "$r/$GATEWAY"
expect "missing gateway errors" "$r" 1 "gateway exclusion must be a regular file"
r="$(new_repo gwlink)"; mv "$r/$GATEWAY" "$r/gateway-target.rs"; ln -s "$r/gateway-target.rs" "$r/$GATEWAY"
expect "symlink gateway errors" "$r" 1 "gateway exclusion must be a regular file"

# Default find skips symlinks: rejecting them is fail-closed, including roots and .rs.
r="$(new_repo rootlink)"; mkdir -p "$r/ui-target" "$r/$CRATES/crosshook-ui"; ln -s "$r/ui-target" "$r/$CRATES/crosshook-ui/src"
expect "symlink scan root errors" "$r" 1 "symlinked scan roots"
r="$(new_repo dirlink)"; mkdir -p "$r/nested"; ln -s "$r/nested" "$r/$CRATES/crosshook-core/src/linked"
expect "nested symlink directory errors" "$r" 1 "symlinked scan roots"
r="$(new_repo rslink)"; cp "$HERE/denied.rs.fixture" "$r/target.rs"; ln -s "$r/target.rs" "$r/$CRATES/crosshook-core/src/linked.rs"
expect "symlink .rs source errors" "$r" 1 "symlinked scan roots"

# Non-Rust symlink assets, and any symlink under tests/, are allowed (no overbroad rejection).
r="$(new_repo assetlink)"; printf 'x' >"$r/asset.png"; mkdir -p "$r/$CRATES/crosshook-core/src/tests" "$r/real"
ln -s "$r/asset.png" "$r/$CRATES/crosshook-core/src/asset.png"
ln -s /nonexistent "$r/$CRATES/crosshook-core/src/dangling.txt"
ln -s "$r/real" "$r/$CRATES/crosshook-core/src/tests/dirlink"
ln -s /nonexistent "$r/$CRATES/crosshook-core/src/tests/dangling.rs"
expect "asset and tests/ symlinks allowed" "$r" 0 "passed"
r="$(new_repo testslink)"; mkdir -p "$r/test-target"; cp "$HERE/denied.rs.fixture" "$r/test-target/bad.rs"
ln -s "$r/test-target" "$r/$CRATES/crosshook-core/src/tests"
expect "tests directory symlink allowed" "$r" 0 "passed"
r="$(new_repo danglingroot)"; mkdir -p "$r/$CRATES/crosshook-ui"; ln -s /nonexistent "$r/$CRATES/crosshook-ui/src"
expect "dangling scan root errors" "$r" 1 "symlinked scan roots"
r="$(new_repo danglingrs)"; ln -s /nonexistent "$r/$CRATES/crosshook-core/src/dangling.rs"
expect "dangling .rs symlink errors" "$r" 1 "symlinked scan roots"
r="$(new_repo danglingdir)"; ln -s /nonexistent "$r/$CRATES/crosshook-core/src/dangling-dir"
expect "dangling non-.rs symlink allowed" "$r" 0 "passed"

# Scan root that is not a directory fails closed.
r="$(new_repo notdir)"; mkdir -p "$r/$CRATES/crosshook-ui"; : >"$r/$CRATES/crosshook-ui/src"
expect "non-directory scan root errors" "$r" 1 "not a directory"

# Whitespace variants and each ban category are covered by denied.rs.fixture (21 hits above);
# assert the unsuppressed count precisely for the combined fixtures.
r="$(new_repo combined)"
put "$r" "$CRATES/crosshook-app/src/a.rs" "$HERE/denied.rs.fixture"
put "$r" "$CRATES/crosshook-app/src/b.rs" "$HERE/suppressed.rs.fixture"
expect "suppression does not hide other violations" "$r" 1 "21 violation(s)"

out="$(bash "$SRC_SCRIPT" --list)"; [[ "$out" == *xdg-open* && "$out" == *flatpak-spawn* ]] || { echo "FAIL: --list" >&2; FAILURES=$((FAILURES + 1)); }

if (( FAILURES )); then echo "check-host-gateway self-test FAILED: $FAILURES failure(s)." >&2; exit 1; fi
echo "check-host-gateway self-test passed."
