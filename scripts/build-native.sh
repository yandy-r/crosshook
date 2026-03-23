#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
NATIVE_DIR="$ROOT_DIR/src/crosshook-native"
DIST_DIR="${DIST_DIR:-$ROOT_DIR/dist}"
TARGET_TRIPLE="${TARGET_TRIPLE:-x86_64-unknown-linux-gnu}"
export APPIMAGE_EXTRACT_AND_RUN="${APPIMAGE_EXTRACT_AND_RUN:-1}"
INSTALL_DEPS=0
INSTALL_DEPS_YES=0
BINARY_ONLY=0

stable_appimage_name() {
  local target_triple="$1"
  local arch_suffix

  case "$target_triple" in
    x86_64-*)
      arch_suffix="amd64"
      ;;
    aarch64-*|arm64-*)
      arch_suffix="arm64"
      ;;
    armv7-*)
      arch_suffix="armv7"
      ;;
    *)
      arch_suffix="${target_triple%%-*}"
      ;;
  esac

  printf 'CrossHook_%s.AppImage\n' "$arch_suffix"
}

usage() {
  cat <<'EOF'
Usage: ./scripts/build-native.sh [--binary-only] [--install-deps] [--yes]

Build the native CrossHook target locally.

Options:
  --binary-only   Build the release binary only and skip AppImage bundling
  --install-deps  Install missing host build dependencies first
  --yes, -y       Forward non-interactive install mode to install-native-build-deps.sh
  --help, -h      Show this help text
EOF
}

die() {
  echo "Error: $*" >&2
  exit 1
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --install-deps)
      INSTALL_DEPS=1
      shift
      ;;
    --binary-only)
      BINARY_ONLY=1
      shift
      ;;
    --yes|-y)
      INSTALL_DEPS_YES=1
      shift
      ;;
    --help|-h)
      usage
      exit 0
      ;;
    *)
      die "unknown argument: $1"
      ;;
  esac
done

if (( INSTALL_DEPS )); then
  install_args=()
  (( INSTALL_DEPS_YES )) && install_args+=(--yes)
  "$ROOT_DIR/scripts/install-native-build-deps.sh" "${install_args[@]}"
fi

command -v cargo >/dev/null 2>&1 || die "cargo is required"
command -v npm >/dev/null 2>&1 || die "npm is required"
if (( ! BINARY_ONLY )); then
  command -v patchelf >/dev/null 2>&1 || die "patchelf is required for AppImage bundling"
fi

cd "$NATIVE_DIR"

if [[ ! -x "$NATIVE_DIR/node_modules/.bin/tauri" ]]; then
  echo "Installing local npm dependencies..."
  npm ci
fi

if (( BINARY_ONLY )); then
  echo "Building CrossHook Native release binary for $TARGET_TRIPLE..."
  npm run build
  cargo build \
    --manifest-path src-tauri/Cargo.toml \
    --release \
    --target "$TARGET_TRIPLE"

  BINARY_PATH="$NATIVE_DIR/target/$TARGET_TRIPLE/release/crosshook-native"
  [[ -x "$BINARY_PATH" ]] || die "release binary not found at $BINARY_PATH"

  mkdir -p "$DIST_DIR"
  cp -f "$BINARY_PATH" "$DIST_DIR/"

  echo "Built release binary:"
  echo "  $BINARY_PATH"
  echo "Copied binary to:"
  echo "  $DIST_DIR/$(basename "$BINARY_PATH")"
  exit 0
fi

echo "Building CrossHook Native AppImage for $TARGET_TRIPLE..."
if cargo tauri --help >/dev/null 2>&1; then
  cargo tauri build --target "$TARGET_TRIPLE"
elif [[ -x "$NATIVE_DIR/node_modules/.bin/tauri" ]]; then
  "$NATIVE_DIR/node_modules/.bin/tauri" build --target "$TARGET_TRIPLE"
elif command -v npx >/dev/null 2>&1; then
  npx tauri build --target "$TARGET_TRIPLE"
else
  die "neither cargo-tauri nor a local tauri CLI is available"
fi

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

STABLE_APPIMAGE_NAME="$(stable_appimage_name "$TARGET_TRIPLE")"
cp -f "$APPIMAGE_SOURCE" "$DIST_DIR/$STABLE_APPIMAGE_NAME"

echo "Copied AppImage to $DIST_DIR/$(basename "$APPIMAGE_SOURCE")"
echo "Copied stable AppImage alias to $DIST_DIR/$STABLE_APPIMAGE_NAME"
