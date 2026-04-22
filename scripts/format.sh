#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
NATIVE_DIR="$ROOT_DIR/src/crosshook-native"
# shellcheck source=lib/modified-files.sh
source "$ROOT_DIR/scripts/lib/modified-files.sh"

usage() {
  cat <<'EOF'
Usage: ./scripts/format.sh [--staged] [--unstaged] [--modified]
                           [--rust] [--ts] [--docs] [--all]

Format code across the full stack.

  --staged    Limit formatting to staged files (git diff --cached)
  --unstaged  Limit formatting to unstaged + untracked files
  --modified  Shorthand for --staged --unstaged (staged ∪ unstaged ∪ untracked)
              Scope flags are additive; combining --staged --unstaged equals --modified.
  --rust      Rust only (rustfmt)
  --ts        TypeScript/React only (biome)
  --docs      Markdown/JSON only (prettier)
  --all       All formatters (default)
EOF
}

SCOPE_STAGED=0
SCOPE_UNSTAGED=0
RUN_RUST=0
RUN_TS=0
RUN_DOCS=0

while [[ $# -gt 0 ]]; do
  case "$1" in
    --staged) SCOPE_STAGED=1; shift ;;
    --unstaged) SCOPE_UNSTAGED=1; shift ;;
    --modified) SCOPE_STAGED=1; SCOPE_UNSTAGED=1; shift ;;
    --rust) RUN_RUST=1; shift ;;
    --ts) RUN_TS=1; shift ;;
    --docs) RUN_DOCS=1; shift ;;
    --all) RUN_RUST=1; RUN_TS=1; RUN_DOCS=1; shift ;;
    --help|-h) usage; exit 0 ;;
    *) echo "Unknown arg: $1" >&2; usage >&2; exit 1 ;;
  esac
done

if (( SCOPE_STAGED && SCOPE_UNSTAGED )); then
  SCOPE="modified"
elif (( SCOPE_STAGED )); then
  SCOPE="staged"
elif (( SCOPE_UNSTAGED )); then
  SCOPE="unstaged"
else
  SCOPE=""
fi
SCOPED=$(( SCOPE_STAGED || SCOPE_UNSTAGED ))

# Default to all if nothing specified
if (( !RUN_RUST && !RUN_TS && !RUN_DOCS )); then
  RUN_RUST=1
  RUN_TS=1
  RUN_DOCS=1
fi

if (( RUN_RUST )); then
  if (( SCOPED )); then
    rust_files=()
    mapfile -t rust_files < <(list_scoped_repo_paths "$SCOPE" "src/crosshook-native/" ".rs")

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
  if (( SCOPED )); then
    ts_files=()
    # biome.json scopes formatting to src/**/*.{ts,tsx}; listing other
    # extensions here makes the invocation error out when only those are
    # staged (biome: "no files were processed"). Keep this list aligned
    # with biome's `files.includes` glob in src/crosshook-native/biome.json.
    mapfile -t ts_files < <(list_scoped_repo_paths "$SCOPE" "src/crosshook-native/src/" \
      ".ts" ".tsx")

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
  if (( SCOPED )); then
    docs_files=()
    mapfile -t docs_files < <(list_scoped_repo_paths "$SCOPE" "" ".md")

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
    docs_files=()
    mapfile -t docs_files < <(list_repo_paths "" ".md")

    if (( ${#docs_files[@]} == 0 )); then
      echo "=== Markdown/JSON ==="
      echo "No Markdown files found."
    else
      echo "=== Markdown/JSON: prettier ==="
      (cd "$NATIVE_DIR" && npx prettier --write \
        "${docs_files[@]}" \
        --ignore-path "$ROOT_DIR/.prettierignore" \
        --config "$ROOT_DIR/.prettierrc")
    fi
  fi
fi

echo "All formatting complete."
