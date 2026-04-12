#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
NATIVE_DIR="$ROOT_DIR/src/crosshook-native"

usage() {
  cat <<'EOF'
Usage: ./scripts/lint.sh [--fix] [--rust] [--ts] [--shell] [--all]

Run linters across the full stack.

  --fix       Apply auto-fixes where possible
  --rust      Rust only (clippy + rustfmt check)
  --ts        TypeScript only (biome + tsc)
  --shell     Shell scripts only (shellcheck)
  --all       All checks (default)
EOF
}

FIX=0
RUN_RUST=0
RUN_TS=0
RUN_SHELL=0
EXIT_CODE=0

while [[ $# -gt 0 ]]; do
  case "$1" in
    --fix) FIX=1; shift ;;
    --rust) RUN_RUST=1; shift ;;
    --ts) RUN_TS=1; shift ;;
    --shell) RUN_SHELL=1; shift ;;
    --all) RUN_RUST=1; RUN_TS=1; RUN_SHELL=1; shift ;;
    --help|-h) usage; exit 0 ;;
    *) echo "Unknown arg: $1" >&2; usage >&2; exit 1 ;;
  esac
done

# Default to all if nothing specified
if (( !RUN_RUST && !RUN_TS && !RUN_SHELL )); then
  RUN_RUST=1
  RUN_TS=1
  RUN_SHELL=1
fi

if (( RUN_RUST )); then
  echo "=== Rust: rustfmt ==="
  if (( FIX )); then
    cargo fmt --manifest-path "$NATIVE_DIR/Cargo.toml" --all || EXIT_CODE=1
  else
    cargo fmt --manifest-path "$NATIVE_DIR/Cargo.toml" --all -- --check || EXIT_CODE=1
  fi

  echo "=== Rust: clippy ==="
  if (( FIX )); then
    cargo clippy --manifest-path "$NATIVE_DIR/Cargo.toml" --all-targets --fix --allow-dirty -- -D warnings || EXIT_CODE=1
  else
    cargo clippy --manifest-path "$NATIVE_DIR/Cargo.toml" --all-targets -- -D warnings || EXIT_CODE=1
  fi
fi

if (( RUN_TS )); then
  echo "=== TypeScript: biome ==="
  if (( FIX )); then
    (cd "$NATIVE_DIR" && npx @biomejs/biome check --fix src/) || EXIT_CODE=1
  else
    (cd "$NATIVE_DIR" && npx @biomejs/biome ci src/) || EXIT_CODE=1
  fi

  echo "=== TypeScript: tsc ==="
  (cd "$NATIVE_DIR" && npx tsc --noEmit) || EXIT_CODE=1
fi

if (( RUN_SHELL )); then
  echo "=== Shell: shellcheck ==="
  shellcheck --severity=warning "$ROOT_DIR"/scripts/*.sh "$ROOT_DIR"/scripts/lib/*.sh || EXIT_CODE=1
fi

exit "$EXIT_CODE"
