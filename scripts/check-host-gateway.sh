#!/usr/bin/env bash
#
# check-host-gateway.sh — flag direct Command::new("<host-tool>") usage outside platform.rs
#
# Guards the platform.rs host-command gateway (ADR-0001). Any Rust source file
# that directly spawns a denylisted host-only tool without routing through
# crate::platform::host_command / host_std_command will be flagged.
#
# Usage:  ./scripts/check-host-gateway.sh [--help|-h] [--list] [--selftest]
# Suppression: add `# allow: host-gateway` on the offending line.
#
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

# Strict host-only tools that MUST route through platform.rs.
# Do NOT add: git, bash, sh, unshare, getent, lspci, flatpak-spawn.
HOST_TOOL_DENYLIST=(
  proton
  umu-run
  gamescope
  mangohud
  winetricks
  protontricks
  gamemoderun
)

SCAN_DIRS=(
  "$REPO_ROOT/src/crosshook-native/crates/crosshook-core/src"
  "$REPO_ROOT/src/crosshook-native/src-tauri/src"
)
PLATFORM_RS="$REPO_ROOT/src/crosshook-native/crates/crosshook-core/src/platform.rs"

usage() {
  cat <<'EOF'
Usage: ./scripts/check-host-gateway.sh [OPTIONS]

Flags direct Command::new("<host-tool>") literals in Rust source outside
platform.rs that bypass the host-command gateway (ADR-0001).

Options:
  -h, --help    Print this help and exit 0.
  --list        Print denylisted tool names (one per line) and exit 0.
  --selftest    Detect a synthetic bypass; exit 0 on success, 1 on failure.

Suppression: add `# allow: host-gateway` on the line (with a brief reason).

Exit codes: 0 = clean, 1 = violations found (or selftest failed).
EOF
}

build_pattern() {
  local IFS='|'
  local alt="${HOST_TOOL_DENYLIST[*]}"
  echo "(tokio::process::|std::process::)?(Command|StdCommand)::new\(\"(${alt})\"\)"
}

if_rg() {
  if command -v rg >/dev/null 2>&1; then return 0; else return 1; fi
}

# ---------------------------------------------------------------------------
# Argument parsing
# ---------------------------------------------------------------------------
SELFTEST=0
while [[ $# -gt 0 ]]; do
  case "$1" in
    -h|--help)  usage; exit 0 ;;
    --list)     printf '%s\n' "${HOST_TOOL_DENYLIST[@]}"; exit 0 ;;
    --selftest) SELFTEST=1; shift ;;
    *)          echo "Unknown argument: $1" >&2; usage >&2; exit 1 ;;
  esac
done

# ---------------------------------------------------------------------------
# Selftest
# ---------------------------------------------------------------------------
if (( SELFTEST )); then
  TMPFILE="$(mktemp /tmp/check-host-gateway-selftest-XXXXXX.rs)"
  trap 'rm -f "$TMPFILE"' EXIT
  printf 'let _cmd = std::process::Command::new("proton");\n' >"$TMPFILE"
  PATTERN="$(build_pattern)"
  if if_rg; then
    rg -qe "$PATTERN" "$TMPFILE" 2>/dev/null && { echo "selftest passed: synthetic bypass was detected."; exit 0; }
  else
    grep -qE "$PATTERN" "$TMPFILE" 2>/dev/null && { echo "selftest passed: synthetic bypass was detected."; exit 0; }
  fi
  echo "selftest FAILED: scanner did not detect the synthetic bypass." >&2
  exit 1
fi

# ---------------------------------------------------------------------------
# Main scan
# ---------------------------------------------------------------------------
EXISTING_DIRS=()
for dir in "${SCAN_DIRS[@]}"; do
  [[ -d "$dir" ]] && EXISTING_DIRS+=("$dir")
done
if [[ ${#EXISTING_DIRS[@]} -eq 0 ]]; then
  echo "error: no Rust source directories found." >&2; exit 1
fi

PATTERN="$(build_pattern)"
EXIT_CODE=0
VIOLATION_COUNT=0

scan_output() {
  if if_rg; then
    rg -n --no-heading --glob '!**/tests/**' -e "$PATTERN" "${EXISTING_DIRS[@]}" 2>/dev/null || true
  else
    find "${EXISTING_DIRS[@]}" -name '*.rs' -not -path '*/tests/*' \
      -exec grep -En "$PATTERN" {} /dev/null \; 2>/dev/null || true
  fi
}

while IFS= read -r match; do
  [[ -z "$match" ]] && continue
  file="${match%%:*}"
  # Skip platform.rs itself.
  [[ "$file" == "$PLATFORM_RS" ]] && continue
  # Skip paths containing /tests/.
  [[ "$file" == */tests/* ]] && continue
  # Skip suppressed lines.
  [[ "$match" == *"# allow: host-gateway"* ]] && continue
  rest="${match#*:}"
  line="${rest%%:*}"
  echo "${file}:${line}: direct Command::new(\"<tool>\") bypasses the platform.rs host-command gateway (ADR-0001). Route through crate::platform::host_command / host_std_command instead."
  (( VIOLATION_COUNT++ )) || true
  EXIT_CODE=1
done < <(scan_output)

if (( EXIT_CODE == 0 )); then
  echo "host-gateway check passed: no direct host-tool bypasses found."
else
  echo "host-gateway check FAILED: ${VIOLATION_COUNT} violation(s) found. See output above."
fi

exit "$EXIT_CODE"
