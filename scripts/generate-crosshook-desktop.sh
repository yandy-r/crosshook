#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
# shellcheck source=lib/build-paths.sh
source "$ROOT_DIR/scripts/lib/build-paths.sh"

desktop_data_home="${XDG_DATA_HOME:-$HOME/.local/share}"
DESKTOP_PATH_DEFAULT="$desktop_data_home/applications/crosshook.desktop"
# Default icon name; when --icon is not provided the script extracts and installs this
# icon from the target AppImage into $XDG_DATA_HOME/icons/hicolor/... for reliable lookup.
ICON_ENTRY_DEFAULT="crosshook-native"
DESKTOP_NAME="CrossHook (native)"
DESKTOP_COMMENT="Game launcher and trainer manager for Steam Deck and Linux"
TERMINAL_MODE="false"
DRY_RUN=0
ICON_OVERRIDDEN=0

appimage_arch_suffix() {
  case "$(uname -m)" in
  x86_64)
    printf 'amd64\n'
    ;;
  aarch64 | arm64)
    printf 'arm64\n'
    ;;
  armv7l | armv7*)
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
  Icon:        extracted from AppImage, installed as 'crosshook-native' in local icon theme

Options:
  --appimage PATH     AppImage to launch in Exec= (default: stable alias in DIST_DIR)
  --output PATH       Desktop file output path (default: ~/.local/share/applications/crosshook.desktop)
  --icon VALUE        Icon= value override (freedesktop name or absolute path to PNG)
  --name VALUE        Desktop Name= value (default: CrossHook)
  --comment VALUE     Desktop Comment= value
  --terminal true|false
                      Desktop Terminal= value (default: false)
  --dry-run           Print the desktop file content, do not write to disk
  --help, -h          Show this help text
EOF
}

DESKTOP_PATH="$DESKTOP_PATH_DEFAULT"
ICON_ENTRY="$ICON_ENTRY_DEFAULT"
APPIMAGE_PATH=""

install_embedded_icon_from_appimage() {
  local appimage_path="$1"
  local icon_name="${2:-$ICON_ENTRY_DEFAULT}"
  local tmp_dir appimage_copy icon_from_desktop desktop_file icon_source icon_dest icon_size_dir

  tmp_dir="$(mktemp -d)"
  appimage_copy="$tmp_dir/CrossHook.AppImage"
  cp -f "$appimage_path" "$appimage_copy"
  chmod +x "$appimage_copy"

  if ! (cd "$tmp_dir" && ./CrossHook.AppImage --appimage-extract >/dev/null 2>&1); then
    echo "Warning: failed to extract AppImage icon payload; using Icon=$icon_name" >&2
    rm -rf "$tmp_dir"
    printf '%s\n' "$icon_name"
    return 0
  fi

  icon_from_desktop=""
  for desktop_file in "$tmp_dir"/squashfs-root/*.desktop "$tmp_dir"/squashfs-root/usr/share/applications/*.desktop; do
    [[ -f "$desktop_file" ]] || continue
    icon_from_desktop="$(awk -F= '/^Icon=/{print $2; exit}' "$desktop_file")"
    if [[ -n "$icon_from_desktop" ]]; then
      icon_name="$icon_from_desktop"
      break
    fi
  done

  icon_source="$(find "$tmp_dir/squashfs-root/usr/share/icons/hicolor" -type f -path "*/apps/${icon_name}.png" 2>/dev/null | sort -V | tail -n 1)"
  if [[ -z "$icon_source" ]]; then
    icon_source="$(find "$tmp_dir/squashfs-root" -type f -name "${icon_name}.png" 2>/dev/null | sort -V | tail -n 1)"
  fi
  if [[ -z "$icon_source" ]]; then
    icon_source="$(find "$tmp_dir/squashfs-root" -maxdepth 2 -type f -name '*.png' 2>/dev/null | sort -V | tail -n 1)"
  fi

  if [[ -z "$icon_source" ]]; then
    echo "Warning: AppImage did not expose a PNG icon; using Icon=$icon_name" >&2
    rm -rf "$tmp_dir"
    printf '%s\n' "$icon_name"
    return 0
  fi

  icon_size_dir="$(sed -n 's#.*hicolor/\([^/]*\)/apps/.*#\1#p' <<<"$icon_source")"
  [[ -n "$icon_size_dir" ]] || icon_size_dir="512x512"
  icon_dest="$desktop_data_home/icons/hicolor/$icon_size_dir/apps/${icon_name}.png"

  mkdir -p "$(dirname "$icon_dest")"
  cp -f "$icon_source" "$icon_dest"

  if command -v gtk-update-icon-cache >/dev/null 2>&1; then
    gtk-update-icon-cache -f -t "$desktop_data_home/icons/hicolor" >/dev/null 2>&1 || true
  fi

  rm -rf "$tmp_dir"
  printf '%s\n' "$icon_name"
}

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
    ICON_ENTRY="${2:-}"
    [[ -n "$ICON_ENTRY" ]] || {
      echo "Error: --icon requires a value" >&2
      exit 1
    }
    ICON_OVERRIDDEN=1
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
  --help | -h)
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

# If user did not override --icon, extract the embedded AppImage icon into local hicolor theme
# and use the embedded icon name as Icon= value for launcher reliability.
if ((!ICON_OVERRIDDEN)); then
  ICON_ENTRY="$(install_embedded_icon_from_appimage "$APPIMAGE_PATH" "$ICON_ENTRY_DEFAULT")"
fi

# When override is an absolute path, verify file exists.
if [[ "$ICON_ENTRY" == /* ]] && [[ ! -f "$ICON_ENTRY" ]]; then
  echo "Warning: icon file not found: $ICON_ENTRY" >&2
fi

desktop_content="$(
  cat <<EOF
[Desktop Entry]
Name=$DESKTOP_NAME
Comment=$DESKTOP_COMMENT
Exec=$APPIMAGE_PATH
Icon=$ICON_ENTRY
Terminal=$TERMINAL_MODE
Type=Application
Categories=Game;Utility;
StartupWMClass=crosshook
EOF
)"

if ((DRY_RUN)); then
  printf '%s\n' "$desktop_content"
  exit 0
fi

mkdir -p "$(dirname "$DESKTOP_PATH")"
printf '%s\n' "$desktop_content" >"$DESKTOP_PATH"
chmod 0644 "$DESKTOP_PATH"

if command -v update-desktop-database >/dev/null 2>&1; then
  update-desktop-database "$(dirname "$DESKTOP_PATH")" >/dev/null 2>&1 || true
fi

echo "Wrote desktop entry:"
echo "  $DESKTOP_PATH"
echo "Exec target:"
echo "  $APPIMAGE_PATH"
