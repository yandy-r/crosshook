#!/usr/bin/env bash
#
# check-app-deps.sh — fail if a UI-free crate's dependency tree pulls in a
# banned crate. Each package in CHECKS has its own cargo-tree edge set and
# banned-name regexes:
#   - crosshook-app (normal edges): no UI toolkit (tauri, wry, webkit2gtk, gtk,
#     glib, iced, gpui, winit and their families: gtk4, glib-sys,
#     tauri-runtime, iced_core, ...).
#   - crosshook-view-model (normal, build and dev edges): no UI toolkit and no
#     async runtime (tokio, mio, async-std, smol, async-executor,
#     futures-executor and their families: tokio-util, tokio_stream).
# Trees cover every target platform (`--target all`).
#
# Usage: ./scripts/check-app-deps.sh [--help|-h] [--selftest]
# Exit codes: 0 = clean, 1 = banned dependency / cargo failure / selftest failure.
#
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
MANIFEST="src/crosshook-native/Cargo.toml"

# Exact name, or name followed by - _ or a digit (gtk4, glib-sys, iced_core).
# `glibc`, `winitx`, etc. do not match.
TOOLKIT_RE='^(tauri|wry|webkit2gtk|gtk|glib|iced|gpui|winit)([-_0-9].*)?$'
# Exact name, or name followed by - or _ (tokio-util, tokio_stream,
# async-std-macros). `tokioz`, `smolder`, `futures-util`, etc. do not match.
ASYNC_RE='^(tokio|mio|async-std|smol|async-executor|futures-executor)([-_].*)?$'

# package|cargo-tree edges|space-separated banned regexes. The regexes contain
# `|`, so split only on the first two separators (see the main loop).
CHECKS=(
  "crosshook-app|normal|$TOOLKIT_RE"
  "crosshook-view-model|normal,build,dev|$TOOLKIT_RE $ASYNC_RE"
)

usage() {
  cat <<'EOF'
Usage: ./scripts/check-app-deps.sh [OPTIONS]

Fails if a UI-free crate's transitive dependencies include a banned crate:
  crosshook-app         normal edges; no UI toolkit crate (tauri, wry,
                        webkit2gtk, gtk, glib, iced, gpui, winit and families
                        such as gtk4, glib-sys, tauri-runtime, iced_core).
  crosshook-view-model  normal, build and dev edges; no UI toolkit crate and
                        no async runtime (tokio, mio, async-std, smol,
                        async-executor, futures-executor and families such
                        as tokio-util, tokio_stream).
Dependency trees cover every target platform (cargo tree --target all).

Options:
  -h, --help    Print this help and exit 0.
  --selftest    Run every CHECKS row against canned dependency trees; exit 0 on success.

Exit codes: 0 = clean, 1 = banned dependency, cargo failure, or selftest failure.
EOF
}

# check_tree <regex>...: reads `cargo tree --prefix none --format '{p}'` output
# on stdin. Prints offending package names; returns 1 on any hit or empty input.
check_tree() {
  local line name re seen=0 bad=0
  while IFS= read -r line; do
    name="${line%% *}"
    [[ -z "$name" ]] && continue
    seen=1
    for re in "$@"; do
      if [[ "$name" =~ $re ]]; then
        echo "banned dependency: $line" >&2
        bad=1
        break
      fi
    done
  done
  if (( ! seen )); then
    echo "error: empty dependency tree" >&2
    return 1
  fi
  return "$bad"
}

# parse_check <entry>: splits a CHECKS row into the globals pkg, edges and
# banned (array). Returns 1 if the row is malformed or has no banned regexes.
parse_check() {
  local regexes
  IFS='|' read -r pkg edges regexes <<<"$1"
  read -r -a banned <<<"$regexes"
  if [[ -z "$pkg" || -z "$edges" || ${#banned[@]} -eq 0 ]]; then
    echo "error: CHECKS row has no package, edges or banned regexes: $1" >&2
    return 1
  fi
}

# selftest_cases <package>: prints `<0|1>|<label>|<tree>` cases (tree uses
# literal \n separators) for a CHECKS package; prints nothing for unknown ones.
selftest_cases() {
  case "$1" in
    crosshook-app)
      cat <<'EOF'
0|clean tree|crosshook-app v0.1.0 (/x/tauri-app)\ncrosshook-core v0.1.0 (/x/crates/crosshook-core)\nserde v1.0.200\nglibc-compat v1.0.0\nwinitx v0.1.0\ntaurine v1.0.0\nwryly v2.0.0
1|empty tree|
1|direct tauri|crosshook-app v0.1.0\ntauri v2.0.0
1|transitive webkit2gtk-sys|crosshook-app v0.1.0\nfoo v1.0.0\nwebkit2gtk-sys v2.0.0
1|gtk4 family|crosshook-app v0.1.0\ngtk4 v0.9.0
1|glib-sys|crosshook-app v0.1.0\nglib-sys v0.20.0
1|tauri-runtime|crosshook-app v0.1.0\ntauri-runtime v2.0.0
1|iced_core underscore|crosshook-app v0.1.0\niced_core v0.13.0
1|winit proc-macro marker|crosshook-app v0.1.0\nwinit v0.30.0 (proc-macro)
1|renamed dep (shows real package)|crosshook-app v0.1.0\nmy-ui-shim v1.0.0\nwry v0.45.0 (*)
1|git source|crosshook-app v0.1.0\ngpui v0.1.0 (https://github.com/zed-industries/zed#abc)
0|async runtimes allowed|crosshook-app v0.1.0\ntokio v1.40.0\nmio v1.0.0\nasync-std v1.13.0\nsmol v2.0.0\nfutures-executor v0.3.31
EOF
      ;;
    crosshook-view-model)
      cat <<'EOF'
0|clean tree (tokioz, miox, smolder, futures-util accepted)|crosshook-view-model v0.1.0 (/x/crates/crosshook-view-model)\ncrosshook-core v0.1.0 (/x/crates/crosshook-core)\nproptest v1.5.0\ntokioz v0.1.0\nmiox v0.1.0\nsmolder v0.1.0\nfutures-util v0.3.31\nfutures-core v0.3.31\nglibc-compat v1.0.0
1|empty tree|
1|tokio|crosshook-view-model v0.1.0\ntokio v1.40.0
1|tokio-util|crosshook-view-model v0.1.0\nfoo v1.0.0\ntokio-util v0.7.0
1|tokio_stream underscore|crosshook-view-model v0.1.0\ntokio_stream v0.1.0
1|mio|crosshook-view-model v0.1.0\nmio v1.0.0
1|tokio-macros proc-macro|crosshook-view-model v0.1.0\ntokio-macros v2.4.0 (proc-macro)
1|async-std|crosshook-view-model v0.1.0\nasync-std v1.13.0
1|smol|crosshook-view-model v0.1.0\nsmol v2.0.0
1|async-executor|crosshook-view-model v0.1.0\nasync-executor v1.13.0
1|futures-executor|crosshook-view-model v0.1.0\nfutures-executor v0.3.31
1|toolkit gtk4|crosshook-view-model v0.1.0\ngtk4 v0.9.0
EOF
      ;;
  esac
}

# selftest: parses every CHECKS row exactly as the main loop does and runs that
# package's canned trees against the parsed regexes.
selftest() {
  local ok=1 entry cases want label tree got
  for entry in "${CHECKS[@]}"; do
    if ! parse_check "$entry"; then
      ok=0
      continue
    fi
    cases="$(selftest_cases "$pkg")"
    if [[ -z "$cases" ]]; then
      echo "selftest FAILED: no cases for CHECKS package $pkg" >&2
      ok=0
      continue
    fi
    while IFS='|' read -r want label tree; do
      got=0
      check_tree "${banned[@]}" <<<"$(printf '%b' "$tree")" 2>/dev/null || got=$?
      if [[ "$got" != "$want" ]]; then
        echo "selftest FAILED: $pkg: $label (want $want, got $got)" >&2
        ok=0
      fi
    done <<<"$cases"
  done

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
failed=0
for entry in "${CHECKS[@]}"; do
  parse_check "$entry" || exit 1
  tree_status=0
  tree="$(cargo tree --manifest-path "$MANIFEST" -p "$pkg" -e "$edges" --target all --prefix none --format '{p}')" || tree_status=$?
  if (( tree_status != 0 )); then
    echo "error: cargo tree failed for $pkg (exit $tree_status)" >&2
    exit 1
  fi
  if check_tree "${banned[@]}" <<<"$tree"; then
    echo "app-deps check passed: no banned dependencies in $pkg ($edges edges)."
  else
    echo "app-deps check FAILED: banned dependencies found in $pkg ($edges edges)." >&2
    failed=1
  fi
done
exit "$failed"
