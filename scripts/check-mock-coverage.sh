#!/usr/bin/env bash
#
# check-mock-coverage.sh — diff #[tauri::command] handlers against mock handler registry
#
# Contributor convenience tool: shows drift between Rust command handlers and the
# browser-dev-mode mock handler registry (src/crosshook-native/src/lib/mocks/).
# Not a CI gate. Always exits 0 unless an internal tool call fails.
#
# Usage:
#   ./scripts/check-mock-coverage.sh
#   npm run dev:browser:check     # equivalent, from src/crosshook-native/
#
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$REPO_ROOT"

if ! command -v rg >/dev/null 2>&1; then
  echo "error: ripgrep (rg) is required but was not found in PATH" >&2
  echo "       install via your package manager (e.g. 'pacman -S ripgrep')" >&2
  exit 1
fi

# Rust source roots that may host #[tauri::command] declarations.
# Both `src-tauri/src/` and `crates/crosshook-core/src/` are checked so that
# this script keeps working if commands are migrated into the core crate later.
RUST_SRC_DIRS=(
  "src/crosshook-native/src-tauri/src"
  "src/crosshook-native/crates/crosshook-core/src"
)

MOCK_HANDLERS_DIR="src/crosshook-native/src/lib/mocks/handlers"

EXISTING_RUST_DIRS=()
for dir in "${RUST_SRC_DIRS[@]}"; do
  if [[ -d "$dir" ]]; then
    EXISTING_RUST_DIRS+=("$dir")
  fi
done

if [[ ${#EXISTING_RUST_DIRS[@]} -eq 0 ]]; then
  echo "error: no Rust source directories found; expected one of:" >&2
  for dir in "${RUST_SRC_DIRS[@]}"; do
    echo "       $dir" >&2
  done
  exit 1
fi

if [[ ! -d "$MOCK_HANDLERS_DIR" ]]; then
  echo "error: mock handlers directory not found: $MOCK_HANDLERS_DIR" >&2
  exit 1
fi

# Extract Rust command names.
#
# Each handler is declared as:
#     #[tauri::command]
#     pub (async )?fn <name>(
#
# A multiline regex captures the function name following the macro. The
# `[^}]*?` non-greedy span tolerates intervening attribute macros (for example
# `#[specta::specta]`) on a separate line between the macro and the `fn`.
RUST_TMP="$(mktemp)"
trap 'rm -f "$RUST_TMP" "$MOCK_TMP"' EXIT

rg -U --multiline --no-filename \
   '#\[tauri::command\][^}]*?\bpub (?:async )?fn ([a-zA-Z_][a-zA-Z_0-9]*)' \
   -r '$1' \
   -o \
   "${EXISTING_RUST_DIRS[@]}" \
   | sort -u > "$RUST_TMP"

# Extract mock handler keys.
#
# Each handler is registered as:
#     map.set('command_name', ...)
# or, occasionally, on multiple lines:
#     map.set(
#       'command_name',
#       ...
#     )
#
# A multiline regex captures the first string-literal argument to `map.set(`,
# accepting either single or double quotes and arbitrary whitespace.
MOCK_TMP="$(mktemp)"

rg -U --multiline --no-filename \
   "map\.set\(\s*['\"]([a-zA-Z_][a-zA-Z_0-9]*)['\"]" \
   -r '$1' \
   -o \
   "$MOCK_HANDLERS_DIR" \
   | sort -u > "$MOCK_TMP"

RUST_COUNT=$(wc -l < "$RUST_TMP" | tr -d ' ')
MOCK_COUNT=$(wc -l < "$MOCK_TMP" | tr -d ' ')

# Diff: missing mocks (Rust commands without a mock handler).
MISSING="$(comm -23 "$RUST_TMP" "$MOCK_TMP")"
MISSING_COUNT=0
if [[ -n "$MISSING" ]]; then
  MISSING_COUNT=$(printf '%s\n' "$MISSING" | wc -l | tr -d ' ')
fi

# Diff: orphaned mocks (mock handlers without a backing Rust command).
ORPHANED="$(comm -13 "$RUST_TMP" "$MOCK_TMP")"
ORPHANED_COUNT=0
if [[ -n "$ORPHANED" ]]; then
  ORPHANED_COUNT=$(printf '%s\n' "$ORPHANED" | wc -l | tr -d ' ')
fi

# Pretty-print the report.
echo "== Mock Coverage Report =="
echo "Source: $(IFS=, ; echo "${EXISTING_RUST_DIRS[*]}")"
echo "Target: $MOCK_HANDLERS_DIR"
echo
echo "Rust commands found: $RUST_COUNT"
echo "Mock handlers found: $MOCK_COUNT"
echo
echo "## Missing mock handlers ($MISSING_COUNT)"
if [[ -z "$MISSING" ]]; then
  echo "(none)"
else
  printf '%s\n' "$MISSING"
fi
echo
echo "## Orphaned mock handlers ($ORPHANED_COUNT)"
if [[ -z "$ORPHANED" ]]; then
  echo "(none)"
else
  printf '%s\n' "$ORPHANED"
fi

# Per Task 3.3 spec: contributor convenience tool, not a CI gate.
# Always exit 0 once the report has been emitted.
exit 0
