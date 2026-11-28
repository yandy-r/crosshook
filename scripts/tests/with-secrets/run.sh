#!/usr/bin/env bash
# Tests for scripts/with-secrets.sh. A stub `op` on PATH stands in for the
# 1Password CLI, so no real op, account or secret is ever used.
set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
LOADER="$HERE/../../with-secrets.sh"
TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT
FAILURES=0

mkdir -p "$TMP/bin-op" "$TMP/bin-none"
# Stub: `op whoami` succeeds unless OP_STUB_SIGNED_OUT=1; `op run --env-file=F -- cmd`
# records its arguments and runs cmd with STUB_RESOLVED=1 (as if refs resolved).
cat >"$TMP/bin-op/op" <<'EOF'
#!/usr/bin/env bash
case "$1" in
  whoami) [[ "${OP_STUB_SIGNED_OUT:-0}" != 1 ]] ;;
  run)
    shift
    printf '%s\n' "$@" >"$OP_STUB_LOG"
    while (($#)) && [[ "$1" != -- ]]; do shift; done
    shift
    STUB_RESOLVED=1 exec "$@"
    ;;
  *) exit 64 ;;
esac
EOF
chmod +x "$TMP/bin-op/op"
# PATH without op: only the tools the loader and the test commands need.
for tool in bash env grep printf cat dirname; do
  ln -s "$(command -v "$tool")" "$TMP/bin-none/$tool"
done

printf '# comment only\n#   X=op://vault/item/X\n\n' >"$TMP/norefs.env"
printf '# refs\nAPI_TOKEN=op://Dev/crosshook/API_TOKEN\n' >"$TMP/refs.env"
export OP_STUB_LOG="$TMP/op.log"

# expect <name> <want-exit> <needle> <PATH> [OP_STUB_SIGNED_OUT] -- loader args...
expect() {
  local name="$1" want="$2" needle="$3" path="$4" signed_out="$5" out rc=0
  shift 5
  [[ "$1" == -- ]] && shift
  rm -f "$OP_STUB_LOG"
  out="$(PATH="$path" OP_STUB_SIGNED_OUT="$signed_out" "$TMP/bin-none/bash" "$LOADER" "$@" 2>&1)" || rc=$?
  if [[ "$rc" != "$want" ]] || [[ "$out" != *"$needle"* ]]; then
    echo "FAIL: $name (exit $rc, want $want, needle '$needle')" >&2
    echo "$out" >&2
    FAILURES=$((FAILURES + 1))
  else
    echo "ok: $name"
  fi
}

WITH_OP="$TMP/bin-op:$TMP/bin-none"
NO_OP="$TMP/bin-none"
SHOW='printf "resolved=%s args=%s\n" "${STUB_RESOLVED:-0}" "$*"'

expect 'no refs runs plainly without op' 0 'resolved=0 args=a b' "$NO_OP" 0 -- \
  --env-file "$TMP/norefs.env" -- bash -c "$SHOW" _ a b
[[ ! -e "$OP_STUB_LOG" ]] || { echo "FAIL: op called with no refs" >&2; FAILURES=$((FAILURES + 1)); }

expect 'missing template runs plainly' 0 'resolved=0' "$NO_OP" 0 -- \
  --env-file "$TMP/absent.env" bash -c "$SHOW"

expect 'refs run through op run' 0 'resolved=1 args=x' "$WITH_OP" 0 -- \
  --env-file="$TMP/refs.env" -- bash -c "$SHOW" _ x
if [[ "$(head -n1 "$OP_STUB_LOG" 2>/dev/null)" != "--env-file=$TMP/refs.env" ]]; then
  echo "FAIL: op run did not get --env-file=<template>" >&2
  FAILURES=$((FAILURES + 1))
fi

expect 'refs with op missing fail fast' 3 'not installed' "$NO_OP" 0 -- \
  --env-file "$TMP/refs.env" -- bash -c 'echo SHOULD-NOT-RUN'
expect 'refs with op signed out fail fast' 4 'not signed in' "$WITH_OP" 1 -- \
  --env-file "$TMP/refs.env" -- bash -c 'echo SHOULD-NOT-RUN'

rc=0
out="$(PATH="$NO_OP" CROSSHOOK_SECRETS_TEMPLATE="$TMP/refs.env" "$TMP/bin-none/bash" "$LOADER" bash -c 'echo SHOULD-NOT-RUN' 2>&1)" || rc=$?
if [[ "$rc" == 3 && "$out" == *"$TMP/refs.env"* && "$out" != *SHOULD-NOT-RUN* ]]; then
  echo "ok: CROSSHOOK_SECRETS_TEMPLATE override"
else
  echo "FAIL: CROSSHOOK_SECRETS_TEMPLATE override (exit $rc): $out" >&2
  FAILURES=$((FAILURES + 1))
fi

expect '--check with no refs' 0 'no op:// references' "$NO_OP" 0 -- --check --env-file "$TMP/norefs.env"
expect '--check with refs signed in' 0 '1 op:// reference(s)' "$WITH_OP" 0 -- --check --env-file "$TMP/refs.env"
expect '--check with refs signed out' 4 'not signed in' "$WITH_OP" 1 -- --check --env-file "$TMP/refs.env"
expect 'no command is a usage error' 2 'Usage:' "$NO_OP" 0 -- --env-file "$TMP/norefs.env"
expect 'directory template rejected' 2 'not a readable file' "$NO_OP" 0 -- --env-file "$TMP" true

if ((FAILURES > 0)); then
  echo "with-secrets tests: $FAILURES failure(s)" >&2
  exit 1
fi
echo 'with-secrets tests passed.'
