#!/usr/bin/env bash
#
# check-mock-coverage.sh — keep the browser-dev mock layer in sync with the backend IPC surface
#
# CI gate: verifies that the browser-dev-mode mocks (src/crosshook-native/src/lib/mocks/)
# cover the backend IPC surface. Exits 1 on any drift:
#
#   Commands  every #[tauri::command] has a `map.set('<name>', …)` mock handler, and
#             every mock handler (except dev-only `_mock_*` helpers) has a command.
#   Events    every backend event (a `*_EVENT: &str` constant in src-tauri or
#             crosshook-app, possibly wrapped by rustfmt) has an `emitMockEvent('<name>', …)` call in a mock
#             handler or an entry with a reason in `unmocked-events.txt`; mocks emit
#             no event the backend does not; every backend event has a payload type
#             in `types/backend-events.ts`; `WireEvent::new` only takes a `*_EVENT`
#             constant (or a `name` binding of one), no `WireEvent { … }` literal or
#             string-literal `emit` names an event; mock handlers emit through the
#             typed `../eventBus` emitter.
#
# Limitation: an `emitMockEvent('<name>', …)` in a comment or in unreachable code
# still counts as a mock emitter.
#
# Usage:
#   ./scripts/check-mock-coverage.sh
#   ./scripts/check-mock-coverage.sh --self-test   # run the isolated fixtures
#   npm run dev:browser:check                      # equivalent, from src/crosshook-native/
#
set -euo pipefail
# `sort` and `comm` must agree on collation regardless of the caller's locale.
export LC_ALL=C

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

case "${1:-}" in
  "") ;;
  --self-test|--selftest)
    exec bash "$REPO_ROOT/scripts/tests/check-mock-coverage/run.sh" ;;
  *) echo "Unknown argument: $1" >&2; exit 1 ;;
esac

cd "$REPO_ROOT"

if ! command -v rg >/dev/null 2>&1; then
  echo "error: ripgrep (rg) is required but was not found in PATH" >&2
  echo "       install via your package manager (e.g. 'pacman -S ripgrep')" >&2
  exit 1
fi

NATIVE=src/crosshook-native

# Rust source roots that may host #[tauri::command] declarations.
COMMAND_DIRS=(
  "$NATIVE/src-tauri/src"
  "$NATIVE/crates/crosshook-core/src"
)
# Rust source roots that name or emit webview events.
EVENT_DIRS=(
  "$NATIVE/src-tauri/src"
  "$NATIVE/crates/crosshook-app/src"
)

FRONTEND_SRC="$NATIVE/src"
MOCK_HANDLERS_DIR="$FRONTEND_SRC/lib/mocks/handlers"
EVENT_ALLOWLIST="$FRONTEND_SRC/lib/mocks/unmocked-events.txt"
EVENT_TYPES="$FRONTEND_SRC/types/backend-events.ts"

existing_dirs() {
  local dir
  for dir in "$@"; do
    [[ -d "$dir" ]] && printf '%s\n' "$dir"
  done
  return 0
}

mapfile -t EXISTING_COMMAND_DIRS < <(existing_dirs "${COMMAND_DIRS[@]}")
mapfile -t EXISTING_EVENT_DIRS < <(existing_dirs "${EVENT_DIRS[@]}")

if [[ ${#EXISTING_COMMAND_DIRS[@]} -eq 0 ]]; then
  echo "error: no Rust command source directories found; expected one of: ${COMMAND_DIRS[*]}" >&2
  exit 1
fi
if [[ ${#EXISTING_EVENT_DIRS[@]} -eq 0 ]]; then
  echo "error: no Rust event source directories found; expected one of: ${EVENT_DIRS[*]}" >&2
  exit 1
fi
for required in "$MOCK_HANDLERS_DIR" "$EVENT_ALLOWLIST" "$EVENT_TYPES"; do
  if [[ ! -e "$required" ]]; then
    echo "error: required mock-coverage input not found: $required" >&2
    exit 1
  fi
done

TMP_DIR="$(mktemp -d)"
trap 'rm -rf "$TMP_DIR"' EXIT

# `rg` exits 1 on no match; an empty result is valid input for the diffs below.
rg_or_empty() {
  rg "$@" || [[ $? -eq 1 ]]
}

# Lines of $1 not in $2 (both sorted, unique).
only_in() {
  comm -23 "$1" "$2"
}

FAILED=0

# report <title> <lines>: prints a section; a non-empty section fails the gate.
report() {
  local title="$1" lines="$2" count=0
  if [[ -n "$lines" ]]; then
    count=$(printf '%s\n' "$lines" | wc -l | tr -d ' ')
    FAILED=1
  fi
  echo "## $title ($count)"
  if [[ -n "$lines" ]]; then printf '%s\n' "$lines"; else echo "(none)"; fi
  echo
}

# --- Commands ---------------------------------------------------------------

# `#[tauri::command]` or `#[tauri::command(…)]`, then `[pub[(…)]] [async] fn <name>(`;
# the non-greedy span tolerates attribute macros between the two.
rg_or_empty -U --multiline --no-filename -o -r '$1' \
  '#\[tauri::command(?:\([^)]*\))?\][^}]*?\b(?:pub(?:\([^)]*\))?\s+)?(?:async\s+)?fn\s+([a-zA-Z_][a-zA-Z_0-9]*)' \
  "${EXISTING_COMMAND_DIRS[@]}" | sort -u > "$TMP_DIR/rust-commands"

# `map.set('command_name', …)`, possibly split across lines; `_mock_*` are dev helpers.
rg_or_empty -U --multiline --no-filename -o -r '$1' \
  "map\.set\(\s*['\"]([a-zA-Z_][a-zA-Z_0-9]*)['\"]" \
  "$MOCK_HANDLERS_DIR" | { grep -v '^_mock_' || true; } | sort -u > "$TMP_DIR/mock-commands"

# --- Events -----------------------------------------------------------------

# `pub const LAUNCH_LOG_EVENT: &str = "launch-log";` (any visibility); rustfmt may
# move the literal to the next line.
rg_or_empty -U --multiline --no-filename -o -r '$1' \
  "const\s+[A-Z][A-Z0-9_]*_EVENT\s*:\s*&(?:'static\s+)?str\s*=\s*\"([^\"]+)\"" \
  "${EXISTING_EVENT_DIRS[@]}" | sort -u > "$TMP_DIR/rust-events"

# Event names that bypass the constant extraction above: a `WireEvent::new` whose
# first argument is not a `*_EVENT` constant or a `name` binding, a `WireEvent { … }`
# literal outside `wire.rs` and tests, or a string literal passed to `emit`. Calls
# may span lines, so offending files (or arguments) are reported.
UNNAMED_EVENT_ARGS="$(rg_or_empty -U --multiline --no-filename -o -r '$1' \
  'WireEvent::new\(\s*([^,)]+?)\s*,' "${EXISTING_EVENT_DIRS[@]}" \
  | { grep -vxE '[A-Z][A-Z0-9_]*_EVENT|name' || true; } | sort -u)"
INLINE_EVENT_LITERALS="$( {
  rg_or_empty -U --multiline -l '\.emit(_str|_to|_filter)?\(\s*"' "${EXISTING_EVENT_DIRS[@]}"
  rg_or_empty -l 'WireEvent\s*\{' -g '!wire.rs' -g '!tests.rs' -g '!**/tests/**' "${EXISTING_EVENT_DIRS[@]}"
} | sort -u)"

# Mock emitters: a string literal, or an identifier resolved through its
# `export const NAME = '…'` declaration in the frontend sources.
: > "$TMP_DIR/mock-events-raw"
UNRESOLVED_EMITTERS=""
while IFS= read -r arg; do
  [[ -z "$arg" ]] && continue
  if [[ "$arg" =~ ^[\'\"](.*)[\'\"]$ ]]; then
    printf '%s\n' "${BASH_REMATCH[1]}" >> "$TMP_DIR/mock-events-raw"
    continue
  fi
  resolved="$(rg_or_empty --no-filename -o -r '$1' \
    "export const ${arg}\s*=\s*['\"]([^'\"]+)['\"]" "$FRONTEND_SRC" | sort -u)"
  if [[ -n "$resolved" && "$(printf '%s\n' "$resolved" | wc -l)" -eq 1 ]]; then
    printf '%s\n' "$resolved" >> "$TMP_DIR/mock-events-raw"
  else
    UNRESOLVED_EMITTERS+="${arg}"$'\n'
  fi
done < <(rg_or_empty -U --multiline --no-filename -o -r '$1' \
  "emitMockEvent\(\s*(['\"][^'\"]+['\"]|[A-Za-z_][A-Za-z0-9_]*)" "$MOCK_HANDLERS_DIR")
sort -u "$TMP_DIR/mock-events-raw" > "$TMP_DIR/mock-events"
UNRESOLVED_EMITTERS="${UNRESOLVED_EMITTERS%$'\n'}"

# Handlers must use the typed emitter, never the untyped bus in lib/events.
# Imports may span lines, so offending files are reported.
UNTYPED_EMITTER_IMPORTS="$(rg_or_empty -U --multiline -l \
  "import[^;]*\bemitMockEvent\b[^;]*from\s*['\"]([^'\"]*lib/events|(\.\./)+events)['\"]" \
  "$MOCK_HANDLERS_DIR" | sort)"

# Allow-list: `<event-name> <reason…>`; `#` starts a comment line.
ALLOWLIST_NO_REASON=""
: > "$TMP_DIR/allowlisted-raw"
while IFS= read -r line || [[ -n "$line" ]]; do
  line="${line%%$'\r'}"
  [[ -z "${line//[[:space:]]/}" || "$line" =~ ^[[:space:]]*# ]] && continue
  read -r name reason <<< "$line"
  printf '%s\n' "$name" >> "$TMP_DIR/allowlisted-raw"
  [[ -z "${reason:-}" ]] && ALLOWLIST_NO_REASON+="${name}"$'\n'
done < "$EVENT_ALLOWLIST"
sort -u "$TMP_DIR/allowlisted-raw" > "$TMP_DIR/allowlisted"
ALLOWLIST_NO_REASON="${ALLOWLIST_NO_REASON%$'\n'}"

# Payload map keys: `  'launch-log': …;` lines in the BackendEventPayloads interface.
rg_or_empty --no-filename -o -r '$1' "^\s+'([^']+)'\??:" "$EVENT_TYPES" | sort -u > "$TMP_DIR/typed-events"

sort -u "$TMP_DIR/mock-events" "$TMP_DIR/allowlisted" > "$TMP_DIR/covered-events"

# --- Report -----------------------------------------------------------------

echo "== Mock Coverage Report =="
echo "Commands: $(IFS=, ; echo "${EXISTING_COMMAND_DIRS[*]}")"
echo "Events:   $(IFS=, ; echo "${EXISTING_EVENT_DIRS[*]}")"
echo "Mocks:    $MOCK_HANDLERS_DIR"
echo
echo "Rust commands found: $(wc -l < "$TMP_DIR/rust-commands" | tr -d ' ')"
echo "Mock handlers found: $(wc -l < "$TMP_DIR/mock-commands" | tr -d ' ')"
echo "Backend events found: $(wc -l < "$TMP_DIR/rust-events" | tr -d ' ')"
echo "Mocked events found: $(wc -l < "$TMP_DIR/mock-events" | tr -d ' ')"
echo "Allow-listed events: $(wc -l < "$TMP_DIR/allowlisted" | tr -d ' ')"
echo

report "Missing mock handlers" "$(only_in "$TMP_DIR/rust-commands" "$TMP_DIR/mock-commands")"
report "Orphaned mock handlers" "$(only_in "$TMP_DIR/mock-commands" "$TMP_DIR/rust-commands")"
report "Unmocked backend events (add an emitter or an unmocked-events.txt entry)" \
  "$(only_in "$TMP_DIR/rust-events" "$TMP_DIR/covered-events")"
report "Orphaned mock events (no backend emitter)" "$(only_in "$TMP_DIR/mock-events" "$TMP_DIR/rust-events")"
report "Stale unmocked-events.txt entries (not a backend event)" \
  "$(only_in "$TMP_DIR/allowlisted" "$TMP_DIR/rust-events")"
report "Redundant unmocked-events.txt entries (already mocked)" \
  "$(comm -12 "$TMP_DIR/allowlisted" "$TMP_DIR/mock-events")"
report "unmocked-events.txt entries without a reason" "$ALLOWLIST_NO_REASON"
report "Backend events without a payload type in types/backend-events.ts" \
  "$(only_in "$TMP_DIR/rust-events" "$TMP_DIR/typed-events")"
report "Payload types for unknown events in types/backend-events.ts" \
  "$(only_in "$TMP_DIR/typed-events" "$TMP_DIR/rust-events")"
report "WireEvent::new arguments that are not *_EVENT constants" "$UNNAMED_EVENT_ARGS"
report "Files naming events without a *_EVENT constant (string-literal emit or WireEvent { … })" "$INLINE_EVENT_LITERALS"
report "Unresolved mock event emitters" "$UNRESOLVED_EMITTERS"
report "Mock handlers importing the untyped emitter (import from '../eventBus')" "$UNTYPED_EMITTER_IMPORTS"

if [[ $FAILED -ne 0 ]]; then
  echo "mock coverage: FAILED"
  exit 1
fi
echo "mock coverage: passed"
exit 0
