#!/usr/bin/env bash
#
# check-legacy-palette.sh — flag legacy Microsoft-blue accent / bg color literals
# anywhere under src/crosshook-native/src/.
#
# Phase 2 of the Unified Desktop Redesign (docs/prps/prds/unified-desktop-redesign.prd.md)
# swaps the old `#0078d4 / #2da3ff / #1a1a2e / #20243d / #12172a` palette for the
# steel-blue tokens defined in src/crosshook-native/src/styles/variables.css.
#
# Once the sweep lands, stylesheets and TSX inline styles must reference the
# `--crosshook-color-*` tokens — never the raw literals. This sentinel keeps it
# that way in CI (wired through scripts/lint.sh).
#
# Usage:  ./scripts/check-legacy-palette.sh [--help|-h] [--list] [--selftest]
# Suppression: add `/* allow: legacy-palette */` (CSS) or `// allow: legacy-palette`
# (TSX/JS) on the offending line with a brief reason. Suppressions should be rare.
#
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

# Legacy literals that must not appear anywhere inside the scan roots.
# Keep the list in sync with docs/internal-docs/design-tokens.md and the PRD.
LEGACY_PATTERNS=(
  '#0078d4'
  '#2da3ff'
  '#1a1a2e'
  '#20243d'
  '#12172a'
  'rgba\(\s*0\s*,\s*120\s*,\s*212'
  'rgba\(\s*45\s*,\s*163\s*,\s*255'
)

SCAN_DIRS=(
  "$REPO_ROOT/src/crosshook-native/src"
)

usage() {
  cat <<'EOF'
Usage: ./scripts/check-legacy-palette.sh [OPTIONS]

Flags legacy Microsoft-blue / old-background color literals under
src/crosshook-native/src/. Enforces the Phase 2 steel-blue token migration
(see docs/internal-docs/design-tokens.md).

Options:
  -h, --help    Print this help and exit 0.
  --list        Print the legacy literal patterns (one per line) and exit 0.
  --selftest    Detect a synthetic literal; exit 0 on success, 1 on failure.

Suppression: add `/* allow: legacy-palette */` (CSS) or
`// allow: legacy-palette` (TSX/JS) on the offending line with a brief reason.

Exit codes: 0 = clean, 1 = violations found (or selftest failed).
EOF
}

build_pattern() {
  local IFS='|'
  echo "${LEGACY_PATTERNS[*]}"
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
    --list)     printf '%s\n' "${LEGACY_PATTERNS[@]}"; exit 0 ;;
    --selftest) SELFTEST=1; shift ;;
    *)          echo "Unknown argument: $1" >&2; usage >&2; exit 1 ;;
  esac
done

# ---------------------------------------------------------------------------
# Selftest — prove the detector fires on a synthetic literal.
# ---------------------------------------------------------------------------
if (( SELFTEST )); then
  TMPFILE="$(mktemp /tmp/check-legacy-palette-selftest-XXXXXX.css)"
  trap 'rm -f "$TMPFILE"' EXIT
  printf '.fake { color: #0078d4; background: rgba(0, 120, 212, 0.18); }\n' >"$TMPFILE"
  PATTERN="$(build_pattern)"
  if if_rg; then
    rg -qe "$PATTERN" "$TMPFILE" 2>/dev/null && { echo "selftest passed: synthetic legacy literal was detected."; exit 0; }
  else
    grep -qE "$PATTERN" "$TMPFILE" 2>/dev/null && { echo "selftest passed: synthetic legacy literal was detected."; exit 0; }
  fi
  echo "selftest FAILED: scanner did not detect the synthetic legacy literal." >&2
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
  echo "error: no frontend source directories found under $REPO_ROOT/src/crosshook-native/src." >&2
  exit 1
fi

PATTERN="$(build_pattern)"
EXIT_CODE=0
VIOLATION_COUNT=0

scan_output() {
  if if_rg; then
    rg -n --no-heading --pcre2 \
      --glob '*.css' --glob '*.ts' --glob '*.tsx' --glob '*.js' --glob '*.jsx' \
      --glob '*.mjs' --glob '*.cjs' --glob '*.module.css' \
      -e "$PATTERN" \
      "${EXISTING_DIRS[@]}" 2>/dev/null || true
  else
    find "${EXISTING_DIRS[@]}" \
      \( -name '*.css' -o -name '*.ts' -o -name '*.tsx' -o -name '*.js' -o -name '*.jsx' \
         -o -name '*.mjs' -o -name '*.cjs' \) \
      -exec grep -En "$PATTERN" {} /dev/null \; 2>/dev/null || true
  fi
}

while IFS= read -r match; do
  [[ -z "$match" ]] && continue
  # Skip suppressed lines (CSS or TSX/JS forms).
  [[ "$match" == *"allow: legacy-palette"* ]] && continue
  file="${match%%:*}"
  rest="${match#*:}"
  line="${rest%%:*}"
  body="${rest#*:}"
  echo "${file}:${line}: legacy palette literal found: ${body# } — reference a --crosshook-color-* token instead (see docs/internal-docs/design-tokens.md)."
  (( VIOLATION_COUNT++ )) || true
  EXIT_CODE=1
done < <(scan_output)

if (( EXIT_CODE == 0 )); then
  echo "legacy-palette check passed: no legacy accent/bg literals found."
else
  echo "legacy-palette check FAILED: ${VIOLATION_COUNT} violation(s) found. See output above."
fi

exit "$EXIT_CODE"
