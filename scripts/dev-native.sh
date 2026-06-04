#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
# shellcheck source=lib/build-paths.sh
source "$ROOT_DIR/scripts/lib/build-paths.sh"
# shellcheck source=lib/pick-free-port.sh
source "$ROOT_DIR/scripts/lib/pick-free-port.sh"
NATIVE_DIR="$ROOT_DIR/src/crosshook-native"

TAURI_DEV_MERGE_CFG=""

warn_uninstalled_hooks() {
  if [[ -n "${CROSSHOOK_SKIP_HOOK_CHECK:-}" ]]; then
    return 0
  fi
  if ! "$ROOT_DIR/scripts/setup-dev-hooks.sh" --check >/dev/null 2>&1; then
    echo "Note: Git hooks are not installed. Run: ./scripts/setup-dev-hooks.sh (same checks as CI pre-commit)" >&2
  fi
}

cleanup_tauri_dev_port() {
  rm -f "${TAURI_DEV_MERGE_CFG:-}"
}

setup_tauri_dev_port() {
  local tauri_dev_port tauri_hmr_port

  if [[ -n "${CROSSHOOK_TAURI_DEV_PORT:-}" ]]; then
    tauri_dev_port="$CROSSHOOK_TAURI_DEV_PORT"
  else
    tauri_dev_port="$(pick_free_port 1420)"
  fi
  export CROSSHOOK_TAURI_DEV_PORT="$tauri_dev_port"

  if [[ -n "${CROSSHOOK_TAURI_HMR_PORT:-}" ]]; then
    tauri_hmr_port="$CROSSHOOK_TAURI_HMR_PORT"
  else
    tauri_hmr_port="$(pick_free_port $((tauri_dev_port + 1)) "$tauri_dev_port")"
  fi
  export CROSSHOOK_TAURI_HMR_PORT="$tauri_hmr_port"

  TAURI_DEV_MERGE_CFG="$(mktemp)"
  printf '{"build":{"devUrl":"http://localhost:%s"}}' "$tauri_dev_port" >"$TAURI_DEV_MERGE_CFG"
  trap cleanup_tauri_dev_port EXIT
}

run_tauri_dev() {
  WEBKIT_DISABLE_DMABUF_RENDERER=1 CARGO_TARGET_DIR="$CARGO_TARGET_DIR" npm exec tauri dev --config "$TAURI_DEV_MERGE_CFG"
}

usage() {
  cat <<'EOF'
Usage: ./scripts/dev-native.sh [--browser|--web]

Run the native Tauri dev app with the local WebKit workaround enabled.
If the first launch fails in a Wayland session, retry once with X11.

  --browser, --web
      Browser-only dev mode: starts Vite at http://127.0.0.1:5173 with mock IPC.
      Does not require cargo or the Rust toolchain.
      Loopback only (--host 0.0.0.0 unsupported per security policy).
      Real Tauri behavior must be re-verified with ./scripts/dev-native.sh before merge.

  Native dev (no flag) picks a free loopback port starting at 1420 (5173 is
  reserved for browser dev mode) so dev:browser and native dev can run together.

  Cargo artifacts use CARGO_TARGET_DIR (default: XDG cache). Override with env if needed.

  CROSSHOOK_SKIP_HOOK_CHECK=1
      Skip the one-line reminder to install Lefthook git hooks (./scripts/setup-dev-hooks.sh).
EOF
}

case "${1:-}" in
  --browser|--web)
    cd "$NATIVE_DIR"
    if [[ ! -x "$NATIVE_DIR/node_modules/.bin/vite" ]]; then
      echo "Installing local npm dependencies..."
      npm ci
    fi
    warn_uninstalled_hooks
    exec npm run dev:browser
    ;;
  --help|-h)
    usage
    exit 0
    ;;
  "")
    ;;
  *)
    echo "Error: unknown argument: $1" >&2
    usage >&2
    exit 1
    ;;
esac

command -v npm >/dev/null 2>&1 || {
  echo "Error: npm is required" >&2
  exit 1
}

cd "$NATIVE_DIR"

if [[ ! -x "$NATIVE_DIR/node_modules/.bin/tauri" ]]; then
  echo "Installing local npm dependencies..."
  npm ci
fi

warn_uninstalled_hooks

crosshook_build_paths_init || exit 1
export CARGO_TARGET_DIR
setup_tauri_dev_port

echo "Starting CrossHook Native dev app..."
echo "  WEBKIT_DISABLE_DMABUF_RENDERER=1"
echo "  CARGO_TARGET_DIR=$CARGO_TARGET_DIR"
echo "  CROSSHOOK_TAURI_DEV_PORT=$CROSSHOOK_TAURI_DEV_PORT (devUrl http://localhost:$CROSSHOOK_TAURI_DEV_PORT)"
echo "  CROSSHOOK_TAURI_HMR_PORT=$CROSSHOOK_TAURI_HMR_PORT"

if run_tauri_dev; then
  exit 0
fi

if [[ -n "${WAYLAND_DISPLAY:-}" || "${XDG_SESSION_TYPE:-}" == "wayland" ]]; then
  echo
  echo "Wayland launch failed. Retrying with X11 fallback..."
  echo "  GDK_BACKEND=x11 WEBKIT_DISABLE_DMABUF_RENDERER=1"
  exec env GDK_BACKEND=x11 WEBKIT_DISABLE_DMABUF_RENDERER=1 CARGO_TARGET_DIR="$CARGO_TARGET_DIR" \
    CROSSHOOK_TAURI_DEV_PORT="$CROSSHOOK_TAURI_DEV_PORT" \
    CROSSHOOK_TAURI_HMR_PORT="$CROSSHOOK_TAURI_HMR_PORT" \
    npm exec tauri dev --config "$TAURI_DEV_MERGE_CFG"
fi

exit 1
