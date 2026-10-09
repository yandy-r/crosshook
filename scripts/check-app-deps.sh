#!/usr/bin/env bash
#
# check-app-deps.sh — fail if crosshook-app's normal dependency tree pulls in a
# banned UI toolkit (tauri, wry, webkit2gtk, gtk, glib, iced, gpui, winit and
# their families: gtk4, glib-sys, tauri-runtime, iced_core, ...).
#
# Usage: ./scripts/check-app-deps.sh [--help|-h] [--selftest]
# Exit codes: 0 = clean, 1 = banned dependency / cargo failure / selftest failure.
#
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
MANIFEST="src/crosshook-native/Cargo.toml"
PACKAGE="crosshook-app"

# Exact name, or name followed by - _ or a digit (gtk4, glib-sys, iced_core).
# `glibc`, `winitx`, etc. do not match.
BANNED_RE='^(tauri|wry|webkit2gtk|gtk|glib|iced|gpui|winit)([-_0-9].*)?$'

usage() {
  cat <<'EOF'
Usage: ./scripts/check-app-deps.sh [OPTIONS]

Fails if the transitive normal dependencies of crosshook-app include a banned
UI toolkit crate (tauri, wry, webkit2gtk, gtk, glib, iced, gpui, winit and
families such as gtk4, glib-sys, tauri-runtime, iced_core).

Options:
  -h, --help    Print this help and exit 0.
  --selftest    Run the checker against canned dependency trees; exit 0 on success.

Exit codes: 0 = clean, 1 = banned dependency, cargo failure, or selftest failure.
EOF
}

# check_tree: reads `cargo tree --prefix none --format '{p}'` output on stdin.
# Prints offending package names; returns 1 on any hit or empty input.
check_tree() {
  local line name seen=0 bad=0
  while IFS= read -r line; do
    name="${line%% *}"
    [[ -z "$name" ]] && continue
    seen=1
    if [[ "$name" =~ $BANNED_RE ]]; then
      echo "banned dependency: $line" >&2
      bad=1
    fi
  done
  if (( ! seen )); then
    echo "error: empty dependency tree" >&2
    return 1
  fi
  return "$bad"
}

selftest() {
  local ok=1
  expect() { # expect <0|1> <label> <tree>
    local want="$1" label="$2" got=0
    check_tree <<<"$3" 2>/dev/null || got=$?
    if [[ "$got" != "$want" ]]; then
      echo "selftest FAILED: $label (want $want, got $got)" >&2
      ok=0
    fi
  }

  local clean=$'crosshook-app v0.1.0 (/x/tauri-app)\ncrosshook-core v0.1.0 (/x/crates/crosshook-core)\nserde v1.0.200\nglibc-compat v1.0.0\nwinitx v0.1.0\ntaurine v1.0.0\nwryly v2.0.0'
  expect 0 "clean tree" "$clean"
  expect 1 "empty tree" ""
  expect 1 "direct tauri" $'crosshook-app v0.1.0\ntauri v2.0.0'
  expect 1 "transitive webkit2gtk-sys" $'crosshook-app v0.1.0\nfoo v1.0.0\nwebkit2gtk-sys v2.0.0'
  expect 1 "gtk4 family" $'crosshook-app v0.1.0\ngtk4 v0.9.0'
  expect 1 "glib-sys" $'crosshook-app v0.1.0\nglib-sys v0.20.0'
  expect 1 "tauri-runtime" $'crosshook-app v0.1.0\ntauri-runtime v2.0.0'
  expect 1 "iced_core underscore" $'crosshook-app v0.1.0\niced_core v0.13.0'
  expect 1 "winit proc-macro marker" $'crosshook-app v0.1.0\nwinit v0.30.0 (proc-macro)'
  expect 1 "renamed dep (shows real package)" $'crosshook-app v0.1.0\nmy-ui-shim v1.0.0\nwry v0.45.0 (*)'
  expect 1 "git source" $'crosshook-app v0.1.0\ngpui v0.1.0 (https://github.com/zed-industries/zed#abc)'

  if (( ok )); then
    echo "selftest passed: allowed trees accepted, banned trees rejected."
    return 0
  fi
  return 1
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    -h|--help)  usage; exit 0 ;;
    --selftest) selftest; exit $? ;;
    *)          echo "Unknown argument: $1" >&2; usage >&2; exit 1 ;;
  esac
done

cd "$REPO_ROOT"
tree_status=0
tree="$(cargo tree --manifest-path "$MANIFEST" -p "$PACKAGE" -e normal --prefix none --format '{p}')" || tree_status=$?
if (( tree_status != 0 )); then
  echo "error: cargo tree failed (exit $tree_status)" >&2
  exit 1
fi

if check_tree <<<"$tree"; then
  echo "app-deps check passed: no banned UI toolkit dependencies in $PACKAGE."
else
  echo "app-deps check FAILED: banned dependencies found in $PACKAGE." >&2
  exit 1
fi
