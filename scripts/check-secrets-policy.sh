#!/usr/bin/env bash
# Secrets policy guard (YAN-2499): local secrets come from 1Password (op://).
# Fails when, in tracked files:
#   1. a file outside the historical record mentions the retired encrypted-env
#      tool, its key file, its encrypted env file, or its DOTENV_* variables;
#   2. an env file is tracked, other than the .env.op template and *.example;
#   3. a .env.op line is anything but a comment, a blank line, or
#      NAME=op://<vault>/<item>/<field> (no literal values, no duplicate names).
# Exits 1 on a policy failure and 2 when the check cannot run.
set -euo pipefail
REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
SELF_PATH="scripts/check-secrets-policy.sh"

usage() {
  cat <<'EOF'
Usage: ./scripts/check-secrets-policy.sh [--help|-h] [--selftest] [REPO_DIR]

Checks the tracked files of REPO_DIR (default: this repository) against the
1Password secrets policy. See CONTRIBUTING.md § Local secrets.
--selftest runs the check against scratch repositories.
EOF
}

# Built from parts so this file does not match its own pattern.
RETIRED_TOOL="dotenv""x"
RETIRED_PATTERN="${RETIRED_TOOL}|DOTENV_[A-Z0-9_]+|\\.env\\.(keys|encrypted)"

check_tree() (
  local repo="$1" failures=0 hits files line lineno name
  cd "$repo"
  git rev-parse --is-inside-work-tree >/dev/null 2>&1 || {
    echo "error: $repo is not a git work tree" >&2
    exit 2
  }
  fail() {
    echo "FAIL $*"
    failures=$((failures + 1))
  }
  tracked_grep() { # git grep args -> matches; exits 2 on a git error, not "no match"
    local out rc=0
    out="$(git grep "$@")" || rc=$?
    ((rc <= 1)) || {
      echo "error: git grep failed ($rc)" >&2
      exit 2
    }
    printf '%s' "$out"
  }

  # 1. Retired tooling. Release notes and internal plans/research record what
  # was true when written, so they are not rewritten.
  # -o prints only the matched token (tool, file or variable name), never the
  # rest of the line, which might hold a pasted credential value.
  hits="$(tracked_grep -n -o -I -i -E "$RETIRED_PATTERN" -- . ':!CHANGELOG.md' \
    ':!docs/plans/**' ':!docs/research/**' ':!docs/internal/**' ":!$SELF_PATH")"
  while IFS= read -r line; do
    [[ -n "$line" ]] && fail "retired env tooling referenced: $line"
  done <<<"$hits"

  # 2. Tracked env files.
  files="$(git ls-files | grep -E '(^|/)\.env(\.[^/]*)?$' | grep -v -x -F '.env.op' |
    grep -v -E '\.example$' || true)"
  while IFS= read -r line; do
    [[ -n "$line" ]] && fail "tracked env or key file: $line"
  done <<<"$files"

  # 3. The reference template holds op:// references only.
  if git ls-files --error-unmatch .env.op >/dev/null 2>&1; then
    declare -A seen=()
    lineno=0
    while IFS= read -r line || [[ -n "$line" ]]; do
      lineno=$((lineno + 1))
      [[ "$line" =~ ^[[:space:]]*(#.*)?$ ]] && continue
      if [[ "$line" =~ ^([A-Z_][A-Z0-9_]*)=op://[^/[:space:]]+/[^/[:space:]]+/[^[:space:]]+$ ]]; then
        name="${BASH_REMATCH[1]}"
        [[ -z "${seen[$name]:-}" ]] || fail ".env.op:$lineno $name is listed twice"
        seen[$name]=1
      elif [[ "$line" =~ ^[[:space:]]*(export[[:space:]]+)?([A-Za-z_][A-Za-z0-9_]*)= ]]; then
        # Print only the name: the value might be a pasted secret.
        fail ".env.op:$lineno ${BASH_REMATCH[2]} is not a NAME=op://<vault>/<item>/<field> reference"
      else
        fail ".env.op:$lineno is not a NAME=op:// reference (line not shown)"
      fi
    done <.env.op
  fi

  if ((failures > 0)); then
    echo "secrets policy: $failures problem(s); see CONTRIBUTING.md § Local secrets"
    exit 1
  fi
  echo "secrets policy: ok"
)

selftest() (
  local scratch out rc
  scratch="$(mktemp -d)" || exit 2
  trap 'rm -rf "$scratch"' EXIT
  die() {
    echo "selftest FAILED: $1" >&2
    exit 1
  }
  fresh() {
    rm -rf "${scratch:?}/repo"
    mkdir -p "$scratch/repo/docs/internal"
    git -C "$scratch/repo" init -q
    printf '# refs only\n\nAPI_TOKEN=op://Dev/crosshook/API_TOKEN\n' >"$scratch/repo/.env.op"
    printf 'hello\n' >"$scratch/repo/README.md"
  }
  run() {
    git -C "$scratch/repo" add -A
    rc=0
    out="$(check_tree "$scratch/repo" 2>&1)" || rc=$?
  }
  expect_pass() {
    run
    ((rc == 0)) || die "$1 (rejected: $out)"
  }
  expect_fail() {
    run
    ((rc == 1)) || die "$2 (exit $rc, want 1: $out)"
    [[ "$out" == *"$1"* ]] || die "$2 (expected '$1', got: $out)"
  }

  fresh
  expect_pass 'baseline'

  fresh
  rm "$scratch/repo/.env.op"
  expect_pass 'no template'

  fresh
  printf 'see docs/internal\nrun %s get\n' "$RETIRED_TOOL" >"$scratch/repo/docs/internal/old.md"
  printf 'old %s notes\n' "$RETIRED_TOOL" >"$scratch/repo/CHANGELOG.md"
  expect_pass 'historical record exempt'

  local fixture
  fresh
  printf 'DOTENV_PRIVATE_KEY=hunter2-literal\n' >"$scratch/repo/README.md"
  expect_fail 'README.md:1:DOTENV_PRIVATE_KEY' 'retired variable reported by name'
  [[ "$out" != *hunter2* ]] || die 'retired variable value printed'

  for fixture in "eval \"\$($RETIRED_TOOL get)\"" 'DOTENV_PRIVATE_KEY_DEV=x' 'DOTENV_PUBLIC_KEY' \
    '.env.keys' '.env.encrypted' "$(tr '[:lower:]' '[:upper:]' <<<"$RETIRED_TOOL")"; do
    fresh
    printf '%s\n' "$fixture" >"$scratch/repo/README.md"
    expect_fail 'retired env tooling referenced' "retired reference: $fixture"
  done

  for fixture in .env .env.local config/.env.production .env.keys; do
    fresh
    mkdir -p "$scratch/repo/$(dirname "$fixture")"
    printf 'X=1\n' >"$scratch/repo/$fixture"
    git -C "$scratch/repo" add -f "$fixture"
    expect_fail "tracked env or key file: $fixture" "tracked env file: $fixture"
  done

  fresh
  printf 'X=\n' >"$scratch/repo/.env.example"
  expect_pass '.env.example allowed'

  fresh
  printf 'API_TOKEN=hunter2-literal\n' >>"$scratch/repo/.env.op"
  expect_fail 'API_TOKEN is not a NAME=op://' 'literal value'
  [[ "$out" != *hunter2* ]] || die 'literal value printed'

  fresh
  printf 'OTHER=op://Dev/crosshook/OTHER\nOTHER=op://Dev/crosshook/OTHER\n' >>"$scratch/repo/.env.op"
  expect_fail 'OTHER is listed twice' 'duplicate name'

  fresh
  printf 'export SECRET=op://Dev/crosshook/SECRET\n' >>"$scratch/repo/.env.op"
  expect_fail 'SECRET is not a NAME=op://' 'export prefix rejected'

  fresh
  printf 'TOKEN=op://Dev/only-two\n' >>"$scratch/repo/.env.op"
  expect_fail 'TOKEN is not a NAME=op://' 'incomplete reference'

  fresh
  printf 'just some words\n' >>"$scratch/repo/.env.op"
  expect_fail 'line not shown' 'free text line'

  rc=0
  out="$(check_tree "$scratch" 2>&1)" || rc=$?
  ((rc == 2)) || die "non-repo should exit 2 (got $rc: $out)"

  echo 'secrets-policy selftest passed.'
)

case "${1:-}" in
  -h | --help) usage ;;
  --selftest)
    [[ $# == 1 ]] || {
      usage >&2
      exit 2
    }
    selftest
    ;;
  *)
    (($# <= 1)) || {
      usage >&2
      exit 2
    }
    check_tree "${1:-$REPO_ROOT}"
    ;;
esac
