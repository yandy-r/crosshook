#!/usr/bin/env bash
# Install Lefthook-managed git hooks for this repository.
# Lefthook is a Go binary — it is not published on crates.io (do not use cargo install).
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

prepend_go_bin_to_path() {
  if command -v go >/dev/null 2>&1; then
    local go_bin
    go_bin="$(go env GOPATH)/bin"
    if [[ -d "$go_bin" ]] && [[ ":$PATH:" != *":${go_bin}:"* ]]; then
      PATH="${go_bin}:$PATH"
      export PATH
    fi
  fi
}

prepend_npm_global_bin_to_path() {
  if command -v npm >/dev/null 2>&1; then
    local pfx
    pfx="$(npm config get prefix 2>/dev/null)"
    if [[ -n "$pfx" && -d "$pfx/bin" ]] && [[ ":$PATH:" != *":${pfx}/bin:"* ]]; then
      PATH="${pfx}/bin:$PATH"
      export PATH
    fi
  fi
}

prepend_user_local_bin_to_path() {
  local b="${HOME}/.local/bin"
  if [[ -d "$b" ]] && [[ ":$PATH:" != *":${b}:"* ]]; then
    PATH="${b}:$PATH"
    export PATH
  fi
}

# Non-login shells often omit tool bins; find an already-installed lefthook.
prepend_standard_tool_paths() {
  prepend_go_bin_to_path
  prepend_npm_global_bin_to_path
  prepend_user_local_bin_to_path
}

usage() {
  cat <<'EOF'
Usage: ./scripts/setup-dev-hooks.sh [--check] [--no-install]

  (default)  Run `lefthook install` so pre-commit runs format/lint on staged files.
             If `lefthook` is missing, tries (in order): go install, npm -g, pipx.

  --check       Exit 0 if hooks are installed, else print install instructions and exit 1.
  --no-install  Do not auto-install lefthook; fail immediately if it is missing.

Install docs: https://lefthook.dev/install/
EOF
}

CHECK_ONLY=0
NO_INSTALL=0
while [[ $# -gt 0 ]]; do
  case "$1" in
    --check)
      CHECK_ONLY=1
      shift
      ;;
    --no-install)
      NO_INSTALL=1
      shift
      ;;
    --help|-h)
      usage
      exit 0
      ;;
    *)
      echo "Error: unknown argument: $1" >&2
      usage >&2
      exit 1
      ;;
  esac
done

hooks_hint() {
  cat <<'EOF'
Lefthook is not installed or git hooks are missing.

  go install github.com/evilmartians/lefthook/v2@latest
  # or: npm install -g lefthook
  # or: pipx install lefthook
  # or: https://lefthook.dev/install/

  ./scripts/setup-dev-hooks.sh

This runs the same format/lint checks as CI before each commit.
EOF
}

try_install_lefthook() {
  if command -v go >/dev/null 2>&1; then
    echo "lefthook not found; installing with: go install github.com/evilmartians/lefthook/v2@latest" >&2
    go install github.com/evilmartians/lefthook/v2@latest
    prepend_go_bin_to_path
    command -v lefthook >/dev/null 2>&1 && return 0
  fi
  if command -v npm >/dev/null 2>&1; then
    echo "lefthook not found; installing with: npm install -g lefthook" >&2
    npm install -g lefthook
    prepend_npm_global_bin_to_path
    command -v lefthook >/dev/null 2>&1 && return 0
  fi
  if command -v pipx >/dev/null 2>&1; then
    echo "lefthook not found; installing with: pipx install lefthook" >&2
    pipx install lefthook
    prepend_user_local_bin_to_path
    command -v lefthook >/dev/null 2>&1 && return 0
  fi
  return 1
}

prepend_standard_tool_paths

if ! command -v lefthook >/dev/null 2>&1; then
  if (( CHECK_ONLY )); then
    hooks_hint >&2
    exit 1
  fi
  if (( NO_INSTALL )); then
    echo "Error: lefthook not found on PATH (--no-install)." >&2
    hooks_hint >&2
    exit 1
  fi
  if ! try_install_lefthook; then
    echo "Error: could not install lefthook automatically (need go, npm, or pipx)." >&2
    hooks_hint >&2
    exit 1
  fi
fi

if ! command -v lefthook >/dev/null 2>&1; then
  echo "Error: lefthook still not on PATH after install. Extend PATH (e.g. ~/.local/bin, Go GOPATH/bin, npm global bin) and re-run." >&2
  exit 1
fi

if (( CHECK_ONLY )); then
  if ! git -C "$ROOT_DIR" rev-parse --git-dir >/dev/null 2>&1; then
    hooks_hint >&2
    exit 1
  fi
  hooks_path=$(git -C "$ROOT_DIR" config --get core.hooksPath 2>/dev/null || true)
  if [[ -z "$hooks_path" ]]; then
    hooks_dir="$(git -C "$ROOT_DIR" rev-parse --git-dir)/hooks"
  elif [[ "$hooks_path" = /* ]]; then
    hooks_dir="$hooks_path"
  else
    hooks_dir="$(git -C "$ROOT_DIR" rev-parse --show-toplevel)/$hooks_path"
  fi
  if command -v realpath >/dev/null 2>&1; then
    hooks_dir="$(realpath -m "$hooks_dir")"
  elif [[ -d "$hooks_dir" ]]; then
    hooks_dir="$(cd "$hooks_dir" && pwd -P)"
  fi
  if [[ -f "$hooks_dir/pre-commit" ]] && grep -q lefthook "$hooks_dir/pre-commit" 2>/dev/null; then
    exit 0
  fi
  hooks_hint >&2
  exit 1
fi

lefthook install
echo "Git hooks installed (lefthook pre-commit)."
