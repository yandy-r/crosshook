#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
# shellcheck source=lib/build-paths.sh
source "$ROOT_DIR/scripts/lib/build-paths.sh"

desktop_data_home="${XDG_DATA_HOME:-$HOME/.local/share}"
DESKTOP_PATH_DEFAULT="$desktop_data_home/applications/crosshook.desktop"
ICON_PATH_DEFAULT="$ROOT_DIR/assets/icon-256.png"
DESKTOP_NAME="CrossHook"
DESKTOP_COMMENT="Game launcher and trainer manager for Steam Deck and Linux"
TERMINAL_MODE="false"
DRY_RUN=0

appimage_arch_suffix() {
  case "$(uname -m)" in
    x86_64)
      printf 'amd64\n'
      ;;
    aarch64|arm64)
      printf 'arm64\n'
      ;;
    armv7l|armv7*)
      printf 'armv7\n'
      ;;
    *)
      printf 'amd64\n'
      ;;
  esac
}

usage() {
  cat <<'EOF'
Usage: ./scripts/generate-crosshook-desktop.sh [options]

Generate a reproducible CrossHook desktop launcher entry.

Defaults:
  Output file: ${XDG_DATA_HOME:-$HOME/.local/share}/applications/crosshook.desktop
  AppImage:    $DIST_DIR/CrossHook_<arch>.AppImage (resolved via build-paths)
  Icon:        ./assets/icon-256.png

Options:
  --appimage PATH     AppImage to launch in Exec= (default: stable alias in DIST_DIR)
  --output PATH       Desktop file output path (default: ~/.local/share/applications/crosshook.desktop)
  --icon PATH         Icon path in desktop entry (default: assets/icon-256.png)
  --name VALUE        Desktop Name= value (default: CrossHook)
  --comment VALUE     Desktop Comment= value
  --terminal true|false
                      Desktop Terminal= value (default: false)
  --dry-run           Print the desktop file content, do not write to disk
  --help, -h          Show this help text
EOF
}

DESKTOP_PATH="$DESKTOP_PATH_DEFAULT"
ICON_PATH="$ICON_PATH_DEFAULT"
APPIMAGE_PATH=""

while [[ $# -gt 0 ]]; do
  case "$1" in
    --appimage)
      APPIMAGE_PATH="${2:-}"
      [[ -n "$APPIMAGE_PATH" ]] || {
        echo "Error: --appimage requires a value" >&2
        exit 1
      }
      shift 2
      ;;
    --output)
      DESKTOP_PATH="${2:-}"
      [[ -n "$DESKTOP_PATH" ]] || {
        echo "Error: --output requires a value" >&2
        exit 1
      }
      shift 2
      ;;
    --icon)
      ICON_PATH="${2:-}"
      [[ -n "$ICON_PATH" ]] || {
        echo "Error: --icon requires a value" >&2
        exit 1
      }
      shift 2
      ;;
    --name)
      DESKTOP_NAME="${2:-}"
      [[ -n "$DESKTOP_NAME" ]] || {
        echo "Error: --name requires a value" >&2
        exit 1
      }
      shift 2
      ;;
    --comment)
      DESKTOP_COMMENT="${2:-}"
      [[ -n "$DESKTOP_COMMENT" ]] || {
        echo "Error: --comment requires a value" >&2
        exit 1
      }
      shift 2
      ;;
    --terminal)
      TERMINAL_MODE="${2:-}"
      [[ "$TERMINAL_MODE" == "true" || "$TERMINAL_MODE" == "false" ]] || {
        echo "Error: --terminal must be true or false" >&2
        exit 1
      }
      shift 2
      ;;
    --dry-run)
      DRY_RUN=1
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

if [[ -z "$APPIMAGE_PATH" ]]; then
  CROSSHOOK_SKIP_MKDIR=1 crosshook_build_paths_init
  APPIMAGE_PATH="$DIST_DIR/CrossHook_$(appimage_arch_suffix).AppImage"
fi

if [[ ! -f "$APPIMAGE_PATH" ]]; then
  echo "Error: AppImage not found: $APPIMAGE_PATH" >&2
  echo "Build first with ./scripts/build-native.sh or pass --appimage PATH." >&2
  exit 1
fi

if [[ ! -f "$ICON_PATH" ]]; then
  echo "Warning: icon file not found: $ICON_PATH" >&2
fi

desktop_content="$(cat <<EOF
[Desktop Entry]
Name=$DESKTOP_NAME
Comment=$DESKTOP_COMMENT
Exec=$APPIMAGE_PATH
Icon=$ICON_PATH
Terminal=$TERMINAL_MODE
Type=Application
Categories=Game;Utility;
StartupWMClass=crosshook
EOF
)"

if (( DRY_RUN )); then
  printf '%s\n' "$desktop_content"
  exit 0
fi

mkdir -p "$(dirname "$DESKTOP_PATH")"
printf '%s\n' "$desktop_content" > "$DESKTOP_PATH"
chmod 0644 "$DESKTOP_PATH"

if command -v update-desktop-database >/dev/null 2>&1; then
  update-desktop-database "$(dirname "$DESKTOP_PATH")" >/dev/null 2>&1 || true
fi

echo "Wrote desktop entry:"
echo "  $DESKTOP_PATH"
echo "Exec target:"
echo "  $APPIMAGE_PATH"
