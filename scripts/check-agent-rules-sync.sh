#!/usr/bin/env bash
#
# check-agent-rules-sync.sh — fail when the agent rule files drift apart.
#
# Checks, relative to the repo root (or $CROSSHOOK_AGENT_RULES_ROOT):
#   1. The block between `<!-- agent-rules:sync:begin` and
#      `<!-- agent-rules:sync:end -->` appears exactly once in CLAUDE.md and in
#      AGENTS.md, and both copies are identical.
#   2. The pointer files (.cursorrules, .github/copilot-instructions.md,
#      .ai/rules/project.md) exist and link to CLAUDE.md.
#   3. The "Crate map and layering" table in CLAUDE.md lists exactly the
#      workspace members of src/crosshook-native/Cargo.toml, each as
#      `<package name>` (`<member path>`) in the first column.
#
# Usage:  ./scripts/check-agent-rules-sync.sh [--help|-h] [--selftest]
# Exit codes: 0 = in sync, 1 = drift, missing file or selftest failure.
#
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
BEGIN_MARK='<!-- agent-rules:sync:begin'
END_MARK='<!-- agent-rules:sync:end -->'
POINTER_FILES=(.cursorrules .github/copilot-instructions.md .ai/rules/project.md)

usage() {
  cat <<'EOF'
Usage: ./scripts/check-agent-rules-sync.sh [OPTIONS]

Fails when:
  - the agent-rules:sync block in CLAUDE.md and AGENTS.md is missing,
    duplicated or not identical;
  - .cursorrules, .github/copilot-instructions.md or .ai/rules/project.md is
    missing or does not link to CLAUDE.md;
  - the CLAUDE.md "Crate map and layering" table does not list exactly the
    workspace members of src/crosshook-native/Cargo.toml, as
    `<package name>` (`<member path>`) in its first column.

Options:
  -h, --help    Print this help and exit 0.
  --selftest    Run the checks against fixture trees; exit 0 on success.

Environment:
  CROSSHOOK_AGENT_RULES_ROOT  Repo root to check instead of this checkout.

Exit codes: 0 = in sync, 1 = drift, missing file or selftest failure.
EOF
}

# extract_block <file>: prints the sync block (markers included); returns 1
# unless exactly one begin and one end marker exist, in that order.
extract_block() {
  local file="$1" begins ends
  begins="$(grep -c -F -- "$BEGIN_MARK" "$file" || true)"
  ends="$(grep -c -F -x -- "$END_MARK" "$file" || true)"
  if [[ "$begins" != 1 || "$ends" != 1 ]]; then
    echo "error: $file must contain exactly one agent-rules:sync begin and end marker (found $begins/$ends)" >&2
    return 1
  fi
  awk -v b="$BEGIN_MARK" -v e="$END_MARK" '
    index($0, b) == 1 { on = 1 }
    on { print }
    on && $0 == e { found = 1; exit }
    END { exit found ? 0 : 1 }
  ' "$file" || {
    echo "error: $file: end marker precedes begin marker" >&2
    return 1
  }
}

# workspace_packages <root>: prints sorted "<package name> <member path>" lines
# for the members of src/crosshook-native/Cargo.toml (glob members unsupported).
workspace_packages() {
  local root="$1" ws="$1/src/crosshook-native" member name
  [[ -f "$ws/Cargo.toml" ]] || { echo "error: missing $ws/Cargo.toml" >&2; return 1; }
  while IFS= read -r member; do
    [[ -f "$ws/$member/Cargo.toml" ]] || { echo "error: missing $ws/$member/Cargo.toml" >&2; return 1; }
    name="$(awk '
      /^\[package\]/ { pkg = 1; next }
      /^\[/ { pkg = 0 }
      pkg && /^name[[:space:]]*=/ { sub(/^name[[:space:]]*=[[:space:]]*"/, ""); sub(/".*/, ""); print; exit }
    ' "$ws/$member/Cargo.toml")"
    [[ -n "$name" ]] || { echo "error: no package name in $ws/$member/Cargo.toml" >&2; return 1; }
    printf '%s %s\n' "$name" "$member"
  done < <(awk '
      /^members[[:space:]]*=/ { on = 1 }
      on { sub(/#.*/, "") }
      on { while (match($0, /"[^"]+"|\047[^\047]+\047/)) { print substr($0, RSTART + 1, RLENGTH - 2); $0 = substr($0, RSTART + RLENGTH) } }
      on && /\]/ { exit }
    ' "$ws/Cargo.toml") | sort
}

# workspace_packages_checked <root>: workspace_packages, but an empty member
# list (unparsed or unsupported `members` syntax) is an explicit error.
workspace_packages_checked() {
  local out
  out="$(workspace_packages "$1")" || return 1
  if [[ -z "$out" ]]; then
    echo "error: no workspace members parsed from $1/src/crosshook-native/Cargo.toml (unsupported members syntax?)" >&2
    return 1
  fi
  printf '%s\n' "$out"
}

# crate_map_packages <file>: prints sorted "<crate> <path>" lines from the first
# column of the body rows of the first table under "## Crate map and layering".
crate_map_packages() {
  awk '
    /^## Crate map and layering/ { on = 1; next }
    on && /^## / { exit }
    on && /^\|[ :|-]+$/ { body = 1; next }
    on && body && !/^\|/ { exit }
    on && body {
      split($0, cols, "|")
      cell = cols[2]
      name = ""; path = ""
      if (match(cell, /`[^`]+`/)) { name = substr(cell, RSTART + 1, RLENGTH - 2); cell = substr(cell, RSTART + RLENGTH) }
      if (match(cell, /`[^`]+`/)) { path = substr(cell, RSTART + 1, RLENGTH - 2) }
      if (name != "") print name, path
    }
  ' "$1" | sort
}

check_root() {
  local root="$1" status=0 claude="$1/CLAUDE.md" agents="$1/AGENTS.md" f diff_out
  local claude_block agents_block expected actual
  for f in "$claude" "$agents"; do
    [[ -f "$f" ]] || { echo "error: missing $f" >&2; return 1; }
  done

  claude_block="$(extract_block "$claude")" || status=1
  agents_block="$(extract_block "$agents")" || status=1
  if (( status == 0 )); then
    if ! diff_out="$(diff <(printf '%s\n' "$claude_block") <(printf '%s\n' "$agents_block"))"; then
      echo "error: the agent-rules:sync block differs between CLAUDE.md (<) and AGENTS.md (>):" >&2
      printf '%s\n' "$diff_out" >&2
      echo "Copy the block from CLAUDE.md into AGENTS.md." >&2
      status=1
    fi
  fi

  for f in "${POINTER_FILES[@]}"; do
    if [[ ! -f "$root/$f" ]]; then
      echo "error: missing pointer file $f" >&2
      status=1
    elif ! grep -q -E '\]\(([./]*/)?CLAUDE\.md\)' "$root/$f"; then
      echo "error: $f must link to CLAUDE.md" >&2
      status=1
    fi
  done

  expected="$(workspace_packages_checked "$root")" || return 1
  actual="$(crate_map_packages "$claude")"
  if [[ "$expected" != "$actual" ]]; then
    echo "error: CLAUDE.md crate map does not match the workspace members:" >&2
    diff <(printf '%s\n' "$expected") <(printf '%s\n' "$actual") | sed 's/^</  workspace:/; s/^>/  crate map:/' >&2 || true
    status=1
  fi

  return "$status"
}

# make_fixture <dir>: writes a minimal in-sync repo tree.
make_fixture() {
  local d="$1"
  mkdir -p "$d/.github" "$d/.ai/rules" "$d/src/crosshook-native/crates/core" "$d/src/crosshook-native/shell"
  cat >"$d/src/crosshook-native/Cargo.toml" <<'EOF'
[workspace]
members = [
    "crates/core",
    "shell",
]
EOF
  printf '[package]\nname = "demo-core"\n\n[dependencies]\nname = "x"\n' >"$d/src/crosshook-native/crates/core/Cargo.toml"
  printf '[package]\nname = "demo-shell"\n' >"$d/src/crosshook-native/shell/Cargo.toml"
  cat >"$d/CLAUDE.md" <<'EOF'
# Rules

<!-- agent-rules:sync:begin — mirrored -->

## Crate map and layering

| Crate | Role |
| ----- | ---- |
| `demo-core` (`crates/core`) | domain |
| `demo-shell` (`shell`) | shell |

Planned: `demo-ui`.

<!-- agent-rules:sync:end -->

## Other
EOF
  { echo "# Agents"; echo; awk '/agent-rules:sync:begin/,/agent-rules:sync:end/' "$d/CLAUDE.md"; } >"$d/AGENTS.md"
  echo 'See [`CLAUDE.md`](CLAUDE.md).' >"$d/.cursorrules"
  echo 'See [`CLAUDE.md`](../CLAUDE.md).' >"$d/.github/copilot-instructions.md"
  echo 'See [`CLAUDE.md`](../../CLAUDE.md).' >"$d/.ai/rules/project.md"
}

selftest() {
  local tmp failures=0 name expect
  tmp="$(mktemp -d)"
  trap 'rm -rf "$tmp"' RETURN
  run_case() {
    name="$1" expect="$2"
    if check_root "$tmp/$name" >/dev/null 2>&1; then got=pass; else got=fail; fi
    if [[ "$got" == "$expect" ]]; then
      echo "ok   $name ($got)"
    else
      echo "FAIL $name: expected $expect, got $got"
      failures=$((failures + 1))
    fi
  }
  local got
  make_fixture "$tmp/clean"; run_case clean pass
  make_fixture "$tmp/drift"; sed -i 's/| domain |/| changed |/' "$tmp/drift/AGENTS.md"; run_case drift fail
  make_fixture "$tmp/no-markers"; sed -i '/agent-rules:sync:end/d' "$tmp/no-markers/AGENTS.md"; run_case no-markers fail
  make_fixture "$tmp/dup-markers"; echo '<!-- agent-rules:sync:end -->' >>"$tmp/dup-markers/CLAUDE.md"; run_case dup-markers fail
  make_fixture "$tmp/end-first"
  sed -i '/agent-rules:sync:begin/d' "$tmp/end-first/AGENTS.md"; echo '<!-- agent-rules:sync:begin -->' >>"$tmp/end-first/AGENTS.md"; run_case end-first fail
  make_fixture "$tmp/no-map"; sed -i 's/^## Crate map and layering/## Crates/' "$tmp/no-map/CLAUDE.md" "$tmp/no-map/AGENTS.md"; run_case no-map fail
  make_fixture "$tmp/wrong-path"; sed -i 's/(`crates\/core`)/(`crates\/cor`)/' "$tmp/wrong-path/CLAUDE.md" "$tmp/wrong-path/AGENTS.md"; run_case wrong-path fail
  make_fixture "$tmp/members-comment"
  printf '[workspace]\nmembers = ["crates/core", # [legacy] note\n    "shell"]\n' >"$tmp/members-comment/src/crosshook-native/Cargo.toml"; run_case members-comment pass
  make_fixture "$tmp/members-literal"
  printf "[workspace]\nmembers = ['crates/core', 'shell']\n" >"$tmp/members-literal/src/crosshook-native/Cargo.toml"; run_case members-literal pass
  make_fixture "$tmp/members-empty"
  printf '[workspace]\nmembers = []\n' >"$tmp/members-empty/src/crosshook-native/Cargo.toml"; run_case members-empty fail
  make_fixture "$tmp/pointer"; echo 'Read the rules.' >"$tmp/pointer/.cursorrules"; run_case pointer fail
  make_fixture "$tmp/no-pointer"; rm "$tmp/no-pointer/.ai/rules/project.md"; run_case no-pointer fail
  make_fixture "$tmp/missing-crate"
  sed -i '/demo-shell/d' "$tmp/missing-crate/CLAUDE.md" "$tmp/missing-crate/AGENTS.md"; run_case missing-crate fail
  make_fixture "$tmp/extra-crate"
  sed -i 's/^| `demo-shell`.*/&\n| `demo-ui` (`ui`) | planned |/' "$tmp/extra-crate/CLAUDE.md" "$tmp/extra-crate/AGENTS.md"; run_case extra-crate fail
  if (( failures > 0 )); then
    echo "agent-rules selftest FAILED: $failures case(s)." >&2
    return 1
  fi
  echo "agent-rules selftest passed."
}

case "${1:-}" in
  -h | --help) usage; exit 0 ;;
  --selftest | --self-test) selftest; exit $? ;;
  "") ;;
  *) echo "error: unknown option: $1" >&2; usage >&2; exit 1 ;;
esac

ROOT="${CROSSHOOK_AGENT_RULES_ROOT:-$REPO_ROOT}"
if check_root "$ROOT"; then
  echo "agent-rules check passed: CLAUDE.md and AGENTS.md agree, pointers link CLAUDE.md, crate map matches the workspace."
else
  echo "agent-rules check FAILED." >&2
  exit 1
fi
