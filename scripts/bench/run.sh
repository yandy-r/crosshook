#!/usr/bin/env bash
# Bench runner entry. Thin wrapper: captures original environment for the
# safety guard, then execs the stdlib helper. Performs NO mkdir/write/copy.
set -euo pipefail

# Capture live identity; stale overrides must never weaken the path guard.
for name in HOME XDG_CONFIG_HOME XDG_DATA_HOME XDG_CACHE_HOME XDG_STATE_HOME XDG_RUNTIME_DIR; do
  key="BENCH_ORIG_$name"
  if [[ -v "$key" ]]; then
    echo "BENCH_REFUSE: unset stale $key before launching" >&2
    exit 10
  fi
done
export BENCH_ORIG_HOME="${HOME:-}"
export BENCH_ORIG_XDG_CONFIG_HOME="${BENCH_ORIG_XDG_CONFIG_HOME:-${XDG_CONFIG_HOME:-}}"
export BENCH_ORIG_XDG_DATA_HOME="${BENCH_ORIG_XDG_DATA_HOME:-${XDG_DATA_HOME:-}}"
export BENCH_ORIG_XDG_CACHE_HOME="${BENCH_ORIG_XDG_CACHE_HOME:-${XDG_CACHE_HOME:-}}"
export BENCH_ORIG_XDG_STATE_HOME="${BENCH_ORIG_XDG_STATE_HOME:-${XDG_STATE_HOME:-}}"
export BENCH_ORIG_XDG_RUNTIME_DIR="${BENCH_ORIG_XDG_RUNTIME_DIR:-${XDG_RUNTIME_DIR:-}}"

if ! command -v python3 >/dev/null 2>&1; then
  echo "BENCH_REFUSE: python3 not found in PATH" >&2
  exit 10
fi

BENCH_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
exec python3 "$BENCH_DIR/bench.py" "$@"
