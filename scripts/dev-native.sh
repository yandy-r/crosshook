#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
# shellcheck source=lib/build-paths.sh
source "$ROOT_DIR/scripts/lib/build-paths.sh"
NATIVE_DIR="$ROOT_DIR/src/crosshook-native"

usage() {
  cat <<'EOF'
Usage: ./scripts/dev-native.sh [--browser|--web]

Run the native Tauri dev app with the local WebKit workaround enabled.
If the first launch fails in a Wayland session, retry once with X11.

  --browser, --web
      Browser-only dev mode: starts Vite at http://localhost:5173 with mock IPC.
      Does not require cargo or the Rust toolchain.
      Loopback only (--host 0.0.0.0 unsupported per security policy).
      Real Tauri behavior must be re-verified with ./scripts/dev-native.sh before merge.

  Cargo artifacts use CARGO_TARGET_DIR (default: XDG cache). Override with env if needed.
EOF
}

case "${1:-}" in
  --browser|--web)
    cd "$NATIVE_DIR"
    if [[ ! -x "$NATIVE_DIR/node_modules/.bin/vite" ]]; then
      echo "Installing local npm dependencies..."
      npm ci
    fi
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

crosshook_build_paths_init || exit 1
export CARGO_TARGET_DIR
echo "Starting CrossHook Native dev app..."
echo "  WEBKIT_DISABLE_DMABUF_RENDERER=1"
echo "  CARGO_TARGET_DIR=$CARGO_TARGET_DIR"

if WEBKIT_DISABLE_DMABUF_RENDERER=1 CARGO_TARGET_DIR="$CARGO_TARGET_DIR" npm exec tauri dev; then
  exit 0
fi

if [[ -n "${WAYLAND_DISPLAY:-}" || "${XDG_SESSION_TYPE:-}" == "wayland" ]]; then
  echo
  echo "Wayland launch failed. Retrying with X11 fallback..."
  echo "  GDK_BACKEND=x11 WEBKIT_DISABLE_DMABUF_RENDERER=1"
  exec env GDK_BACKEND=x11 WEBKIT_DISABLE_DMABUF_RENDERER=1 CARGO_TARGET_DIR="$CARGO_TARGET_DIR" npm exec tauri dev
fi

exit 1
