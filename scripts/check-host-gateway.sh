#!/usr/bin/env bash
# check-host-gateway.sh — lexical host-command guard (ADR-0001).
# Suppression: add `# allow: host-gateway` on the offending line, with a reason.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
HOST_TOOL_DENYLIST=(
  proton umu-run gamescope mangohud winetricks protontricks gamemoderun
  xdg-open gio kde-open gnome-open flatpak-spawn
)
SCAN_DIRS=(
  "$REPO_ROOT/src/crosshook-native/crates/crosshook-core/src"
  "$REPO_ROOT/src/crosshook-native/src-tauri/src"
  "$REPO_ROOT/src/crosshook-native/crates/crosshook-app/src"
  "$REPO_ROOT/src/crosshook-native/crates/crosshook-ui/src"
  "$REPO_ROOT/src/crosshook-native/crates/crosshook-widgets/src"
  "$REPO_ROOT/src/crosshook-native/crates/crosshook-view-model/src"
)
GATEWAY_FILES=(
  "$REPO_ROOT/src/crosshook-native/crates/crosshook-core/src/platform/gateway.rs"
)

usage() {
  cat <<'EOF'
Usage: ./scripts/check-host-gateway.sh [OPTIONS]

Flags denylisted Command::new literals and open::that/opener::open calls
outside explicit platform gateway files (ADR-0001).

Options:
  -h, --help              Print help.
  --list                  Print denylisted tool names.
  --self-test, --selftest  Run isolated scanner fixtures.

Suppression: add `# allow: host-gateway` on the line, with a reason.
Exit codes: 0 = clean, 1 = violations or scan/self-test failure.
EOF
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    -h|--help) usage; exit 0 ;;
    --list) printf '%s\n' "${HOST_TOOL_DENYLIST[@]}"; exit 0 ;;
    --self-test|--selftest)
      exec bash "$REPO_ROOT/scripts/tests/check-host-gateway/run.sh" ;;
    *) echo "Unknown argument: $1" >&2; usage >&2; exit 1 ;;
  esac
done

for file in "${GATEWAY_FILES[@]}"; do
  if [[ ! -f "$file" || -L "$file" ]]; then
    echo "error: gateway exclusion must be a regular file: $file" >&2
    exit 1
  fi
done

# Fail-closed symlink policy (documented in ADR-0001): default find skips symlink
# roots and sources, which would report a false "clean". Reject symlinks instead.
reject_symlink() {
  echo "error: $1: symlinked scan roots and Rust sources are rejected (fail-closed; see ADR-0001)." >&2
  exit 1
}

EXISTING_DIRS=()
for dir in "${SCAN_DIRS[@]}"; do
  if [[ -L "$dir" ]]; then
    reject_symlink "$dir"
  elif [[ -d "$dir" ]]; then
    EXISTING_DIRS+=("$dir")
  elif [[ -e "$dir" ]]; then
    echo "error: scan root is not a directory: $dir" >&2
    exit 1
  fi
done
if [[ ${#EXISTING_DIRS[@]} -eq 0 ]]; then
  echo "error: no Rust source directories found." >&2
  exit 1
fi

# ponytail: single-line lexical guard, not Rust parsing. Literal plus turbofish,
# optional & and trailing comma; aliases, multiline calls, raw strings, const
# indirection and macros need review; use Rust-aware check if scope grows.
ALT="$(IFS='|'; echo "${HOST_TOOL_DENYLIST[*]}")"
SPACE='[[:blank:]]*'
PATTERN="(^|[^[:alnum:]_])(Command|StdCommand)${SPACE}::${SPACE}new${SPACE}(::${SPACE}<[^>]*>${SPACE})?\\(${SPACE}&?${SPACE}\"(${ALT})\"${SPACE},?${SPACE}\\)|(^|[^[:alnum:]_])(open${SPACE}::${SPACE}(that|that_detached|with)|opener${SPACE}::${SPACE}(open|open_browser|open_detached))${SPACE}\\("

TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT
# Fail-closed symlink policy (ADR-0001): `find` follows no symlinks by default and
# would silently skip them. Outside tests/ we reject symlinked directories and
# *.rs sources (dangling included); non-Rust symlinks are allowed as assets.
if ! find "${EXISTING_DIRS[@]}" -type l -print0 >"$TMP/links" 2>"$TMP/find-err"; then
  echo "error: cannot enumerate Rust sources." >&2
  cat "$TMP/find-err" >&2
  exit 1
fi
while IFS= read -r -d '' e; do
  case "$e" in */tests|*/tests/*) continue ;; esac
  if [[ "${e##*/}" == *.rs || -d "$e" ]]; then
    reject_symlink "$e"
  fi
done <"$TMP/links"
if ! find "${EXISTING_DIRS[@]}" -type f -name '*.rs' -not -path '*/tests/*' -print0 >"$TMP/files"; then
  echo "error: cannot enumerate Rust sources." >&2
  exit 1
fi
SCANNER='grep'
if command -v rg >/dev/null 2>&1; then SCANNER='rg'; fi
VIOLATION_COUNT=0
while IFS= read -r -d '' file; do
  excluded=0
  for gateway in "${GATEWAY_FILES[@]}"; do
    if [[ "$file" == "$gateway" ]]; then excluded=1; break; fi
  done
  if (( excluded )); then continue; fi
  status=0
  if [[ "$SCANNER" == rg ]]; then
    rg --no-config --color never --no-heading -nH -e "$PATTERN" -- "$file" >"$TMP/matches" || status=$?
  else
    grep -nHE "$PATTERN" -- "$file" >"$TMP/matches" || status=$?
  fi
  if (( status > 1 )); then
    echo "error: $SCANNER failed scanning $file (status $status)." >&2
    exit 1
  fi
  while IFS= read -r match; do
    if [[ "$match" == *"# allow: host-gateway"* ]]; then continue; fi
    echo "$match: bypasses platform host-command/portal contract (ADR-0001)."
    VIOLATION_COUNT=$((VIOLATION_COUNT + 1))
  done <"$TMP/matches"
done <"$TMP/files"

if (( VIOLATION_COUNT )); then
  echo "host-gateway check FAILED: ${VIOLATION_COUNT} violation(s) found."
  exit 1
fi
echo "host-gateway check passed: no direct host-tool bypasses found."
