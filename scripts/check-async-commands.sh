#!/usr/bin/env bash
# check-async-commands.sh — every #[tauri::command] in src-tauri must be async (YAN-832, D10).
# Sync Tauri commands run on the main thread and can freeze the webview; thin
# delegates to crosshook-app services must be `async fn` so Tauri runs them on
# its async runtime.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DEFAULT_SCAN_DIR="$REPO_ROOT/src/crosshook-native/src-tauri/src"

usage() {
  cat <<'EOF_USAGE'
Usage: ./scripts/check-async-commands.sh [OPTIONS]

Fails when a #[tauri::command] attribute in src-tauri/src/**/*.rs is followed
(skipping other #[...] attributes, including multi-line ones, // and ///
comment lines, and blank lines) by a non-async fn declaration. `pub async fn`
and `async fn` pass; `fn`, `pub fn` and `pub(crate) fn` are reported as
file:line: fn <name>.

This is a line-based lexical guard, not a Rust parser: a command attribute and
its fn on one line, macro-generated commands and block comments are not modelled.

Options:
  -h, --help               Print help.
  --self-test, --selftest  Run isolated fixture cases.

Environment:
  CROSSHOOK_ASYNC_COMMANDS_DIR  Directory to scan instead of src-tauri/src
                                (used by --self-test).

Exit codes: 0 = clean, 1 = violations or scan/self-test failure.
EOF_USAGE
}

# scan_dir <dir>: prints one "file:line: fn <name>: ..." line per violation to
# stderr; returns 1 when any violation is found or the scan cannot run.
scan_dir() {
  local dir="$1" file count=0 total=0 out
  if [[ -L "$dir" || ! -d "$dir" ]]; then
    echo "error: scan root missing or symlinked: $dir" >&2
    return 1
  fi
  while IFS= read -r -d '' file; do
    count=$((count + 1))
    out="$(scan_file "$file")" || return 1
    if [[ -n "$out" ]]; then
      printf '%s\n' "$out" >&2
      total=$((total + $(wc -l <<<"$out")))
    fi
  done < <(find "$dir" -type f -name '*.rs' -print0 | sort -z)
  if (( count == 0 )); then
    echo "error: no Rust sources found under $dir" >&2
    return 1
  fi
  if (( total > 0 )); then
    echo "async-commands check FAILED: ${total} sync #[tauri::command] fn(s) found." >&2
    return 1
  fi
  echo "async-commands check passed: all #[tauri::command] fns are async."
}

# scan_file <file>: state machine over lines. After a #[tauri::command...]
# attribute, skip attributes (bracket depth tracked for multi-line ones),
# comments and blank lines; the first other line must be an async fn.
scan_file() {
  awk -v file="$1" '
    function brackets(s,   o, c, t) {
      t = s; o = gsub(/\[/, "", t)
      t = s; c = gsub(/\]/, "", t)
      return o - c
    }
    {
      line = $0
      if (depth > 0) { depth += brackets(line); next }
      if (line ~ /^[[:space:]]*#!?\[tauri::command([](]|[[:space:]]*[(])/) {
        pending = 1
      }
      if (line ~ /^[[:space:]]*#!?\[/) { depth = brackets(line); if (depth < 0) depth = 0; next }
      if (!pending) next
      if (line ~ /^[[:space:]]*$/ || line ~ /^[[:space:]]*\/\//) next
      pending = 0
      if (line ~ /^[[:space:]]*(pub(\([^)]*\))?[[:space:]]+)?async[[:space:]]+fn[[:space:]]/) next
      if (match(line, /(^|[^[:alnum:]_])fn[[:space:]]+[[:alnum:]_]+/)) {
        decl = substr(line, RSTART, RLENGTH)
        sub(/^[^f]*fn[[:space:]]+/, "", decl)
        printf "%s:%d: fn %s: #[tauri::command] must be async\n", file, NR, decl
      } else {
        printf "%s:%d: #[tauri::command] not followed by a fn declaration\n", file, NR
      }
    }
  ' "$1"
}

selftest() (
  local scratch dir
  scratch="$(mktemp -d)" || exit 1
  trap 'rm -rf "$scratch"' EXIT
  dir="$scratch/src"
  die() { echo "selftest FAILED: $1" >&2; exit 1; }
  fixture() { rm -rf "$dir"; mkdir -p "$dir/commands"; printf '%b' "$1" >"$dir/commands/f.rs"; }
  expect_pass() {
    local out
    fixture "$2"
    out="$(scan_dir "$dir" 2>&1)" || die "$1 (rejected: $out)"
  }
  expect_fail() {
    local out
    fixture "$3"
    if out="$(scan_dir "$dir" 2>&1)"; then die "$1 (unexpectedly passed)"; fi
    [[ "$out" == *"$2"* ]] || die "$1 (expected '$2', got: $out)"
  }

  expect_fail 'sync command' 'fn sync_cmd' \
    '#[tauri::command]\npub fn sync_cmd() {}\n'
  expect_pass 'async with doc comments between' \
    '#[tauri::command]\n/// Docs.\n// plain\n\n#[allow(dead_code)]\npub async fn ok_cmd() {}\n'
  expect_fail 'multi-line attribute before sync fn' 'fn multi_cmd' \
    '#[tauri::command]\n#[allow(\n    clippy::too_many_arguments\n)]\npub fn multi_cmd(a: u8) {}\n'
  expect_pass 'multi-line attribute before async fn' \
    '#[tauri::command]\n#[allow(\n    clippy::too_many_arguments\n)]\nasync fn multi_ok(a: u8) {}\n'
  expect_fail 'pub(crate) sync fn' 'fn crate_cmd' \
    '#[tauri::command]\npub(crate) fn crate_cmd() {}\n'
  expect_fail 'private sync fn' 'fn private_cmd' \
    '#[tauri::command]\nfn private_cmd() {}\n'
  expect_pass 'pub(crate) async fn' \
    '#[tauri::command]\npub(crate) async fn crate_ok() {}\n'
  expect_pass 'tauri::command with args' \
    '#[tauri::command(rename_all = "snake_case")]\npub async fn args_ok() {}\n'
  expect_fail 'tauri::command with args, sync' 'fn args_bad' \
    '#[tauri::command(rename_all = "snake_case")]\npub fn args_bad() {}\n'
  expect_pass 'non-command sync fn and commented attribute' \
    'pub fn helper() {}\n// #[tauri::command]\npub fn other() {}\n#[derive(Debug)]\nstruct S;\n'
  expect_fail 'sync fn after an async command' 'fn second_cmd' \
    '#[tauri::command]\npub async fn first_cmd() {}\n\n#[tauri::command]\npub fn second_cmd() {}\n'
  expect_fail 'line number is reported' 'f.rs:2:' \
    '#[tauri::command]\npub fn sync_cmd() {}\n'

  rm -rf "$dir"; mkdir -p "$dir"
  if scan_dir "$dir" >/dev/null 2>&1; then die 'empty scan root unexpectedly passed'; fi
  if scan_dir "$scratch/missing" >/dev/null 2>&1; then die 'missing scan root unexpectedly passed'; fi

  echo 'async-commands selftest passed.'
)

while [[ $# -gt 0 ]]; do
  case "$1" in
    -h|--help) usage; exit 0 ;;
    --self-test|--selftest) selftest; exit $? ;;
    *) echo "Unknown argument: $1" >&2; usage >&2; exit 1 ;;
  esac
done

scan_dir "${CROSSHOOK_ASYNC_COMMANDS_DIR:-$DEFAULT_SCAN_DIR}"
