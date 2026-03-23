#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
NATIVE_DIR="$ROOT_DIR/src/crosshook-native"

usage() {
  cat <<'EOF'
Usage: ./scripts/dev-native.sh

Run the native Tauri dev app with the local WebKit workaround enabled.
If the first launch fails in a Wayland session, retry once with X11.
EOF
}

case "${1:-}" in
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

echo "Starting CrossHook Native dev app..."
echo "  WEBKIT_DISABLE_DMABUF_RENDERER=1"

if WEBKIT_DISABLE_DMABUF_RENDERER=1 npm exec tauri dev; then
  exit 0
fi

if [[ -n "${WAYLAND_DISPLAY:-}" || "${XDG_SESSION_TYPE:-}" == "wayland" ]]; then
  echo
  echo "Wayland launch failed. Retrying with X11 fallback..."
  echo "  GDK_BACKEND=x11 WEBKIT_DISABLE_DMABUF_RENDERER=1"
  exec GDK_BACKEND=x11 WEBKIT_DISABLE_DMABUF_RENDERER=1 npm exec tauri dev
fi

exit 1
