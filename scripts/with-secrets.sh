#!/usr/bin/env bash
# Run a command with local secrets from 1Password (YAN-2499).
#
# The committed template (.env.op by default) holds only NAME=op://vault/item/field
# references, never values. When it has references, the command runs under
# `op run --env-file=<template>`, which resolves them into the child's environment
# only. When it has none, the command runs as-is. When it has references but `op`
# is missing or not signed in, this fails before running anything.
set -euo pipefail

REPO_ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DEFAULT_TEMPLATE="$REPO_ROOT/.env.op"

usage() {
  cat <<'EOF'
Usage: ./scripts/with-secrets.sh [--env-file FILE] [--] COMMAND [ARGS...]
       ./scripts/with-secrets.sh --check [--env-file FILE]

Runs COMMAND with the op:// references from FILE (default: .env.op at the repo
root, or $CROSSHOOK_SECRETS_TEMPLATE) resolved by `op run`.

  no op:// references        COMMAND runs directly; op is not needed
  references, op signed in   COMMAND runs under `op run --env-file=FILE`
  references, op missing     exit 3, nothing runs
  references, op signed out  exit 4, nothing runs (sign in with the 1Password
                             app integration or `op signin`, then retry)

--check only reports which case applies (exit 0 when COMMAND could run).
EOF
}

die() {
  local code="$1"
  shift
  echo "with-secrets: $*" >&2
  exit "$code"
}

# count_refs <file>: prints how many NAME=op://... lines the template has.
# Comments and blank lines are ignored; the values are never printed.
count_refs() {
  grep -c -E '^[[:space:]]*(export[[:space:]]+)?[A-Za-z_][A-Za-z0-9_]*=["'\'']?op://' "$1" || true
}

template="${CROSSHOOK_SECRETS_TEMPLATE:-$DEFAULT_TEMPLATE}"
check_only=0
while (($#)); do
  case "$1" in
    --env-file)
      (($# >= 2)) || die 2 "--env-file needs a path"
      template="$2"
      shift 2
      ;;
    --env-file=*)
      template="${1#--env-file=}"
      shift
      ;;
    --check)
      check_only=1
      shift
      ;;
    -h | --help)
      usage
      exit 0
      ;;
    --)
      shift
      break
      ;;
    *) break ;;
  esac
done

((check_only || $# > 0)) || {
  usage >&2
  exit 2
}

refs=0
if [[ -e "$template" ]]; then
  [[ -f "$template" && -r "$template" ]] || die 2 "template is not a readable file: $template"
  refs="$(count_refs "$template")"
fi

if ((refs == 0)); then
  if ((check_only)); then
    echo "with-secrets: no op:// references in $template; commands run without 1Password"
    exit 0
  fi
  exec "$@"
fi

command -v op >/dev/null 2>&1 ||
  die 3 "$template has $refs op:// reference(s) but the 1Password CLI (op) is not installed; see CONTRIBUTING.md § Local secrets"
op whoami >/dev/null 2>&1 ||
  die 4 "$template has $refs op:// reference(s) but op is not signed in; unlock the 1Password app (CLI integration) or sign in, then retry"

if ((check_only)); then
  echo "with-secrets: $refs op:// reference(s) in $template; op is signed in"
  exit 0
fi
exec op run --env-file="$template" -- "$@"
