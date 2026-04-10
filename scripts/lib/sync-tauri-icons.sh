#!/usr/bin/env bash
# Copy generated branding PNG into Tauri bundle icon path for AppImage/desktop integration.
# Expects ./scripts/generate-assets.sh to have run first (creates assets/icon-512.png).
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
ASSETS="$ROOT_DIR/assets"
TAURI_ICONS="$ROOT_DIR/src/crosshook-native/src-tauri/icons"
# High-res monogram matches bundle expectations; tauri.conf.json lists icons/icon.png
if [[ -n "${CROSSHOOK_TAURI_ICON_SOURCE:-}" ]]; then
  if [[ "$CROSSHOOK_TAURI_ICON_SOURCE" == /* ]]; then
    SOURCE_PNG="$CROSSHOOK_TAURI_ICON_SOURCE"
  else
    SOURCE_PNG="$ROOT_DIR/${CROSSHOOK_TAURI_ICON_SOURCE#./}"
  fi
else
  SOURCE_PNG="$ASSETS/icon-512.png"
fi
DEST_PNG="$TAURI_ICONS/icon.png"

if [[ ! -f "$SOURCE_PNG" ]]; then
  echo "sync-tauri-icons: missing source icon: $SOURCE_PNG" >&2
  echo "Run ./scripts/generate-assets.sh first (requires rsvg-convert and ImageMagick)." >&2
  exit 1
fi

mkdir -p "$TAURI_ICONS"
cp -f "$SOURCE_PNG" "$DEST_PNG"
echo "sync-tauri-icons: updated $DEST_PNG from $SOURCE_PNG"
