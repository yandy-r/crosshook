#!/usr/bin/env bash
#
# check-schema-version.sh — fail when SUPPORTED_MAX_VERSION drifts from docs.
#
# Compares (each value must appear exactly once):
#   src/crosshook-native/crates/crosshook-core/src/metadata/migrations/mod.rs
#     pub const SUPPORTED_MAX_VERSION: u32 = N;
#   CLAUDE.md:  **Current schema version**: **N** (number bold or plain)
#   AGENTS.md:  **Current schema version**: N   (number bold or plain)
#
# The invariant "latest migration number == SUPPORTED_MAX_VERSION" is covered
# by a Rust regression test in migrations/tests/ (core lane), not here.
#
# Usage:  ./scripts/check-schema-version.sh [--help|-h] [--selftest]
#
# Exit codes: 0 = in sync, 1 = mismatch, duplicate, or missing value.
#
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
MOD_RS="$REPO_ROOT/src/crosshook-native/crates/crosshook-core/src/metadata/migrations/mod.rs"
CLAUDE_MD="$REPO_ROOT/CLAUDE.md"
AGENTS_MD="$REPO_ROOT/AGENTS.md"

usage() {
  cat <<'EOF'
Usage: ./scripts/check-schema-version.sh [OPTIONS]

Fails when SUPPORTED_MAX_VERSION in migrations/mod.rs drifts from the
**Current schema version** recorded in CLAUDE.md and AGENTS.md, or when
any value is missing or duplicated.

Options:
  -h, --help    Print this help and exit 0.
  --selftest    Run built-in pass/fail checks against scratch files in a
                mktemp directory (never touches the repo) and exit.

Exit codes: 0 = in sync, 1 = mismatch, duplicate, or missing value.
EOF
}

grab_matches() {
  # $1 = file, $2 = ERE pattern with capture group 2 = value. Prints all matches.
  sed -nE "s/$2/\\2/p" "$1"
}

count_lines() {
  if [[ -n "$1" ]]; then
    grep -c '' <<<"$1"
  else
    echo 0
  fi
}

# Pattern: start of line, optional leading whitespace/list hyphen, then
# "**Current schema version**: N" with the number bold or plain; the line must
# end after the value (no mid-line prose matches). Group 2 is the number.
DOC_PATTERN='^[[:space:]]*-?[[:space:]]*\*\*Current schema version\*\*:[[:space:]]*(\*\*[[:space:]]*)?([0-9]+)([[:space:]]*\*\*)?[[:space:]]*$'
CONST_PATTERN='^[[:space:]]*(pub const SUPPORTED_MAX_VERSION: u32 = )([0-9]+);[[:space:]]*$'

exactly_one() {
  # $1 = label, $2 = file, $3 = matches. Sets EXACTLY_ONE_VALUE. Returns 0/1.
  local label="$1" file="$2" matches="$3" n
  n="$(count_lines "$matches")"
  if (( n == 0 )); then
    echo "error: $label not found in $file" >&2
    return 1
  fi
  if (( n > 1 )); then
    echo "error: $label found $n times in $file; must appear exactly once" >&2
    return 1
  fi
  EXACTLY_ONE_VALUE="$matches"
  return 0
}

check_tree() {
  # $1 = label prefix, $2 = mod.rs, $3 = CLAUDE.md, $4 = AGENTS.md. Prints diagnostics; returns 0/1.
  local prefix="$1" mod="$2" claude="$3" agents="$4"
  local code_v claude_v agents_v rc=0

  code_v="$(grab_matches "$mod" "$CONST_PATTERN")"
  claude_v="$(grab_matches "$claude" "$DOC_PATTERN")"
  agents_v="$(grab_matches "$agents" "$DOC_PATTERN")"

  exactly_one "${prefix}SUPPORTED_MAX_VERSION" "$mod" "$code_v" || rc=1
  code_v="${EXACTLY_ONE_VALUE:-}"
  exactly_one "${prefix}**Current schema version** (CLAUDE.md)" "$claude" "$claude_v" || rc=1
  claude_v="${EXACTLY_ONE_VALUE:-}"
  exactly_one "${prefix}**Current schema version** (AGENTS.md)" "$agents" "$agents_v" || rc=1
  agents_v="${EXACTLY_ONE_VALUE:-}"
  (( rc != 0 )) && return 1

  if [[ "$code_v" != "$claude_v" ]]; then
    echo "${prefix}error: mismatch: migrations/mod.rs SUPPORTED_MAX_VERSION=$code_v but CLAUDE.md says $claude_v" >&2
    rc=1
  fi
  if [[ "$code_v" != "$agents_v" ]]; then
    echo "${prefix}error: mismatch: migrations/mod.rs SUPPORTED_MAX_VERSION=$code_v but AGENTS.md says $agents_v" >&2
    rc=1
  fi
  (( rc != 0 )) && return 1

  echo "${prefix}schema-version check passed: SUPPORTED_MAX_VERSION=$code_v matches CLAUDE.md and AGENTS.md."
  return 0
}

selftest() {
  local rc=0
  # Prefer the approved scratch dir in this environment; fall back to TMPDIR.
  if [[ -d /tmp/opencode ]]; then
    SCRATCH_DIR="$(mktemp -d /tmp/opencode/schema-version-XXXXXX)"
  else
    SCRATCH_DIR="$(mktemp -d "${TMPDIR:-/tmp}/schema-version-XXXXXX")"
  fi
  trap 'rm -rf "$SCRATCH_DIR"' EXIT
  local dir="$SCRATCH_DIR"

  printf 'pub const SUPPORTED_MAX_VERSION: u32 = 27;\n' >"$dir/mod-pass.rs"
  printf -- '- **Current schema version**: **27**\n' >"$dir/claude-pass.md"
  printf -- '**Current schema version**: 27\n' >"$dir/agents-pass.md"

  expect_pass() {
    local label="$1"; shift
    if check_tree "[$label] " "$@"; then
      echo "[$label] ok: passed as expected."
    else
      echo "[$label] FAILED: expected pass but check failed" >&2
      rc=1
    fi
  }

  expect_fail() {
    local label="$1"; shift
    if check_tree "[$label] " "$@" >/dev/null 2>&1; then
      echo "[$label] FAILED: expected failure but check passed" >&2
      rc=1
    else
      echo "[$label] ok: failed as expected."
    fi
  }

  # pass: bold CLAUDE number, plain AGENTS number.
  expect_pass "pass" "$dir/mod-pass.rs" "$dir/claude-pass.md" "$dir/agents-pass.md"

  # pass: plain numbers in both docs.
  printf -- '- **Current schema version**: 27\n' >"$dir/claude-plain.md"
  printf -- '**Current schema version**: **27**\n' >"$dir/agents-bold.md"
  expect_pass "pass-plain" "$dir/mod-pass.rs" "$dir/claude-plain.md" "$dir/agents-bold.md"

  # mismatch: code higher than docs.
  printf 'pub const SUPPORTED_MAX_VERSION: u32 = 28;\n' >"$dir/mod-mis.rs"
  expect_fail "mismatch" "$dir/mod-mis.rs" "$dir/claude-pass.md" "$dir/agents-pass.md"

  # missing constant.
  printf '// no constant here\n' >"$dir/mod-missing.rs"
  expect_fail "missing-const" "$dir/mod-missing.rs" "$dir/claude-pass.md" "$dir/agents-pass.md"

  # duplicate constant.
  cat "$dir/mod-pass.rs" "$dir/mod-pass.rs" >"$dir/mod-dup.rs"
  expect_fail "dup-const" "$dir/mod-dup.rs" "$dir/claude-pass.md" "$dir/agents-pass.md"

  # missing CLAUDE.md marker.
  printf '# nothing here\n' >"$dir/claude-missing.md"
  expect_fail "missing-claude" "$dir/mod-pass.rs" "$dir/claude-missing.md" "$dir/agents-pass.md"

  # missing AGENTS.md marker.
  expect_fail "missing-agents" "$dir/mod-pass.rs" "$dir/claude-pass.md" "$dir/claude-missing.md"

  # duplicate CLAUDE.md marker.
  cat "$dir/claude-pass.md" "$dir/claude-pass.md" >"$dir/claude-dup.md"
  expect_fail "dup-claude" "$dir/mod-pass.rs" "$dir/claude-dup.md" "$dir/agents-pass.md"

  # duplicate AGENTS.md marker.
  cat "$dir/agents-pass.md" "$dir/agents-pass.md" >"$dir/agents-dup.md"
  expect_fail "dup-agents" "$dir/mod-pass.rs" "$dir/claude-pass.md" "$dir/agents-dup.md"

  # mid-line prose mentioning the marker must not satisfy the CLAUDE.md check.
  printf 'see **Current schema version**: 27 now\n' >"$dir/claude-prose.md"
  expect_fail "prose-claude" "$dir/mod-pass.rs" "$dir/claude-prose.md" "$dir/agents-pass.md"

  # mid-line prose mentioning the marker must not satisfy the AGENTS.md check.
  printf 'see **Current schema version**: 27 now\n' >"$dir/agents-prose.md"
  expect_fail "prose-agents" "$dir/mod-pass.rs" "$dir/claude-pass.md" "$dir/agents-prose.md"

  # per-doc drift: only CLAUDE.md disagrees with the code.
  printf -- '- **Current schema version**: **28**\n' >"$dir/claude-only28.md"
  expect_fail "claude-only28" "$dir/mod-pass.rs" "$dir/claude-only28.md" "$dir/agents-pass.md"

  # per-doc drift: only AGENTS.md disagrees with the code.
  printf '**Current schema version**: 28\n' >"$dir/agents-only28.md"
  expect_fail "agents-only28" "$dir/mod-pass.rs" "$dir/claude-pass.md" "$dir/agents-only28.md"

  (( rc == 0 )) && echo "selftest passed." || echo "selftest FAILED." >&2
  return "$rc"
}

SCRATCH_DIR=""

SELFTEST=0
while [[ $# -gt 0 ]]; do
  case "$1" in
    -h|--help) usage; exit 0 ;;
    --selftest) SELFTEST=1; shift ;;
    *) echo "Unknown argument: $1" >&2; usage >&2; exit 1 ;;
  esac
done

if (( SELFTEST )); then
  selftest
  exit $?
fi

check_tree "" "$MOD_RS" "$CLAUDE_MD" "$AGENTS_MD"
