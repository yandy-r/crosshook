#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
NATIVE_DIR="$ROOT_DIR/src/crosshook-native"
DIST_DIR="${DIST_DIR:-$ROOT_DIR/dist}"
TARGET_TRIPLE="${TARGET_TRIPLE:-x86_64-unknown-linux-gnu}"

die() {
  echo "Error: $*" >&2
  exit 1
}

command -v cargo >/dev/null 2>&1 || die "cargo is required"

cd "$NATIVE_DIR"

echo "Building CrossHook Native AppImage for $TARGET_TRIPLE..."
cargo tauri build --target "$TARGET_TRIPLE"

APPIMAGE_SOURCE=""
for bundle_dir in \
  "$NATIVE_DIR/src-tauri/target/$TARGET_TRIPLE/release/bundle/appimage" \
  "$NATIVE_DIR/src-tauri/target/release/bundle/appimage" \
  "$NATIVE_DIR/target/$TARGET_TRIPLE/release/bundle/appimage" \
  "$NATIVE_DIR/target/release/bundle/appimage"
do
  if [[ -d "$bundle_dir" ]]; then
    candidate="$(find "$bundle_dir" -maxdepth 1 -type f -name '*.AppImage' | sort | tail -n 1)"
    if [[ -n "$candidate" ]]; then
      APPIMAGE_SOURCE="$candidate"
      break
    fi
  fi
done

[[ -n "$APPIMAGE_SOURCE" ]] || die "AppImage output not found after build"

mkdir -p "$DIST_DIR"
cp -f "$APPIMAGE_SOURCE" "$DIST_DIR/"

echo "Copied AppImage to $DIST_DIR/$(basename "$APPIMAGE_SOURCE")"
