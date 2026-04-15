#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
NATIVE_DIR="$ROOT_DIR/src/crosshook-native"
# shellcheck source=lib/modified-files.sh
source "$ROOT_DIR/scripts/lib/modified-files.sh"

usage() {
  cat <<'EOF'
Usage: ./scripts/format.sh [--modified] [--rust] [--ts] [--docs] [--all]

Format code across the full stack.

  --modified  Limit file-based formatting to modified git files (staged, unstaged, untracked)
  --rust      Rust only (rustfmt)
  --ts        TypeScript/React only (biome)
  --docs      Markdown/JSON only (prettier)
  --all       All formatters (default)
EOF
}

MODIFIED_ONLY=0
RUN_RUST=0
RUN_TS=0
RUN_DOCS=0
# shellcheck disable=SC2034 # Consumed by filter_modified_repo_paths() from the sourced helper.
MODIFIED_REPO_FILES=()

while [[ $# -gt 0 ]]; do
  case "$1" in
    --modified) MODIFIED_ONLY=1; shift ;;
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

if (( MODIFIED_ONLY )); then
  mapfile -t MODIFIED_REPO_FILES < <(list_modified_repo_files)
fi

if (( RUN_RUST )); then
  if (( MODIFIED_ONLY )); then
    rust_files=()
    filter_modified_repo_paths rust_files "src/crosshook-native/" ".rs"

    if (( ${#rust_files[@]} == 0 )); then
      echo "=== Rust ==="
      echo "No modified Rust files."
    else
      echo "=== Rust: rustfmt ==="
      cargo fmt --manifest-path "$NATIVE_DIR/Cargo.toml" --all -- "${rust_files[@]}"
    fi
  else
    echo "=== Rust: rustfmt ==="
    cargo fmt --manifest-path "$NATIVE_DIR/Cargo.toml" --all
  fi
fi

if (( RUN_TS )); then
  if (( MODIFIED_ONLY )); then
    ts_files=()
    filter_modified_repo_paths ts_files "src/crosshook-native/src/" \
      ".ts" ".tsx" ".js" ".jsx" ".mjs" ".cjs" ".mts" ".cts" ".json" ".jsonc" ".css"

    if (( ${#ts_files[@]} == 0 )); then
      echo "=== TypeScript/React ==="
      echo "No modified frontend source files."
    else
      echo "=== TypeScript/React: biome ==="
      (cd "$NATIVE_DIR" && npx @biomejs/biome format --write "${ts_files[@]}")
      (cd "$NATIVE_DIR" && npx @biomejs/biome check --fix "${ts_files[@]}")
    fi
  else
    echo "=== TypeScript/React: biome ==="
    (cd "$NATIVE_DIR" && npx @biomejs/biome format --write src/)
    (cd "$NATIVE_DIR" && npx @biomejs/biome check --fix src/)
  fi
fi

if (( RUN_DOCS )); then
  if (( MODIFIED_ONLY )); then
    docs_files=()
    filter_modified_repo_paths docs_files "" ".md"

    if (( ${#docs_files[@]} == 0 )); then
      echo "=== Markdown/JSON ==="
      echo "No modified Markdown files."
    else
      echo "=== Markdown/JSON: prettier ==="
      (cd "$NATIVE_DIR" && npx prettier --write \
        "${docs_files[@]}" \
        --ignore-path "$ROOT_DIR/.prettierignore" \
        --config "$ROOT_DIR/.prettierrc")
    fi
  else
    echo "=== Markdown/JSON: prettier ==="
    (cd "$NATIVE_DIR" && npx prettier --write \
      "$ROOT_DIR/**/*.md" \
      --ignore-path "$ROOT_DIR/.prettierignore" \
      --config "$ROOT_DIR/.prettierrc")
  fi
fi

echo "All formatting complete."
