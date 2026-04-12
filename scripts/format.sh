#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
NATIVE_DIR="$ROOT_DIR/src/crosshook-native"

usage() {
  cat <<'EOF'
Usage: ./scripts/format.sh [--rust] [--ts] [--docs] [--all]

Format code across the full stack.

  --rust      Rust only (rustfmt)
  --ts        TypeScript/React only (biome)
  --docs      Markdown/JSON only (prettier)
  --all       All formatters (default)
EOF
}

RUN_RUST=0
RUN_TS=0
RUN_DOCS=0

while [[ $# -gt 0 ]]; do
  case "$1" in
    --rust) RUN_RUST=1; shift ;;
    --ts) RUN_TS=1; shift ;;
    --docs) RUN_DOCS=1; shift ;;
    --all) RUN_RUST=1; RUN_TS=1; RUN_DOCS=1; shift ;;
    --help|-h) usage; exit 0 ;;
    *) echo "Unknown arg: $1" >&2; usage >&2; exit 1 ;;
  esac
done

# Default to all if nothing specified
if (( !RUN_RUST && !RUN_TS && !RUN_DOCS )); then
  RUN_RUST=1
  RUN_TS=1
  RUN_DOCS=1
fi

if (( RUN_RUST )); then
  echo "=== Rust: rustfmt ==="
  cargo fmt --manifest-path "$NATIVE_DIR/Cargo.toml" --all
fi

if (( RUN_TS )); then
  echo "=== TypeScript/React: biome ==="
  (cd "$NATIVE_DIR" && npx @biomejs/biome format --write src/)
  (cd "$NATIVE_DIR" && npx @biomejs/biome check --fix src/)
fi

if (( RUN_DOCS )); then
  echo "=== Markdown/JSON: prettier ==="
  (cd "$NATIVE_DIR" && npx prettier --write \
    "$ROOT_DIR/**/*.md" \
    --ignore-path "$ROOT_DIR/.prettierignore" \
    --config "$ROOT_DIR/.prettierrc")
fi

echo "All formatting complete."
