#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
NATIVE_DIR="$ROOT_DIR/src/crosshook-native"
# shellcheck source=lib/modified-files.sh
source "$ROOT_DIR/scripts/lib/modified-files.sh"

usage() {
  cat <<'EOF'
Usage: ./scripts/lint.sh [--fix] [--modified] [--rust] [--ts] [--shell] [--host-gateway] [--all]

Run linters across the full stack.

  --fix           Apply auto-fixes where possible
  --modified      Limit file-based linting to modified git files (staged, unstaged, untracked)
  --rust          Rust only (clippy + rustfmt check)
  --ts            TypeScript only (biome + tsc)
  --shell         Shell scripts only (shellcheck)
  --host-gateway  Host-command gateway check only (ADR-0001)
  --all           All checks (default)
EOF
}

FIX=0
MODIFIED_ONLY=0
RUN_RUST=0
RUN_TS=0
RUN_SHELL=0
RUN_HOST_GATEWAY=0
EXIT_CODE=0

while [[ $# -gt 0 ]]; do
  case "$1" in
    --fix) FIX=1; shift ;;
    --modified) MODIFIED_ONLY=1; shift ;;
    --rust) RUN_RUST=1; shift ;;
    --ts) RUN_TS=1; shift ;;
    --shell) RUN_SHELL=1; shift ;;
    --host-gateway) RUN_HOST_GATEWAY=1; shift ;;
    --all) RUN_RUST=1; RUN_TS=1; RUN_SHELL=1; RUN_HOST_GATEWAY=1; shift ;;
    --help|-h) usage; exit 0 ;;
    *) echo "Unknown arg: $1" >&2; usage >&2; exit 1 ;;
  esac
done

# Default to all if nothing specified
if (( !RUN_RUST && !RUN_TS && !RUN_SHELL && !RUN_HOST_GATEWAY )); then
  RUN_RUST=1
  RUN_TS=1
  RUN_SHELL=1
  RUN_HOST_GATEWAY=1
fi

if (( RUN_RUST )); then
  if (( MODIFIED_ONLY )); then
    rust_files=()
    mapfile -t rust_files < <(list_modified_repo_paths "src/crosshook-native/" ".rs")

    if (( ${#rust_files[@]} == 0 )); then
      echo "=== Rust ==="
      echo "No modified Rust files."
    else
      echo "=== Rust: rustfmt ==="
      if (( FIX )); then
        cargo fmt --manifest-path "$NATIVE_DIR/Cargo.toml" --all -- "${rust_files[@]}" || EXIT_CODE=1
      else
        cargo fmt --manifest-path "$NATIVE_DIR/Cargo.toml" --all -- --check "${rust_files[@]}" || EXIT_CODE=1
      fi

      echo "=== Rust: clippy (workspace scope) ==="
      if (( FIX )); then
        cargo clippy --manifest-path "$NATIVE_DIR/Cargo.toml" --all-targets --fix --allow-dirty -- -D warnings || EXIT_CODE=1
      else
        cargo clippy --manifest-path "$NATIVE_DIR/Cargo.toml" --all-targets -- -D warnings || EXIT_CODE=1
      fi
    fi
  else
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
fi

if (( RUN_TS )); then
  if (( MODIFIED_ONLY )); then
    ts_biome_files=()
    ts_typecheck_files=()
    mapfile -t ts_biome_files < <(list_modified_repo_paths "src/crosshook-native/src/" \
      ".ts" ".tsx" ".js" ".jsx" ".mjs" ".cjs" ".mts" ".cts" ".json" ".jsonc" ".css")
    mapfile -t ts_typecheck_files < <(list_modified_repo_paths "src/crosshook-native/src/" \
      ".ts" ".tsx" ".mts" ".cts")

    if (( ${#ts_biome_files[@]} == 0 )); then
      echo "=== TypeScript ==="
      echo "No modified frontend source files."
    else
      echo "=== TypeScript: biome ==="
      if (( FIX )); then
        (cd "$NATIVE_DIR" && npx @biomejs/biome check --fix "${ts_biome_files[@]}") || EXIT_CODE=1
      else
        (cd "$NATIVE_DIR" && npx @biomejs/biome ci "${ts_biome_files[@]}") || EXIT_CODE=1
      fi
    fi

    if (( ${#ts_typecheck_files[@]} > 0 )); then
      echo "=== TypeScript: tsc (project scope) ==="
      (cd "$NATIVE_DIR" && npx tsc --noEmit) || EXIT_CODE=1
    fi
  else
    echo "=== TypeScript: biome ==="
    if (( FIX )); then
      (cd "$NATIVE_DIR" && npx @biomejs/biome check --fix src/) || EXIT_CODE=1
    else
      (cd "$NATIVE_DIR" && npx @biomejs/biome ci src/) || EXIT_CODE=1
    fi

    echo "=== TypeScript: tsc ==="
    (cd "$NATIVE_DIR" && npx tsc --noEmit) || EXIT_CODE=1
  fi
fi

if (( RUN_SHELL )); then
  if (( MODIFIED_ONLY )); then
    shell_files=()
    mapfile -t shell_files < <(list_modified_repo_paths "scripts/" ".sh")

    if (( ${#shell_files[@]} == 0 )); then
      echo "=== Shell ==="
      echo "No modified shell scripts."
    else
      echo "=== Shell: shellcheck ==="
      shellcheck --severity=warning "${shell_files[@]}" || EXIT_CODE=1
    fi
  else
    echo "=== Shell: shellcheck ==="
    shellcheck --severity=warning "$ROOT_DIR"/scripts/*.sh "$ROOT_DIR"/scripts/lib/*.sh || EXIT_CODE=1
  fi
fi

if (( RUN_HOST_GATEWAY )); then
  echo "=== Host-gateway ==="
  "$ROOT_DIR/scripts/check-host-gateway.sh" || EXIT_CODE=1
fi

exit "$EXIT_CODE"
