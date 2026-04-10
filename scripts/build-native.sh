#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
# shellcheck source=lib/build-paths.sh
source "$ROOT_DIR/scripts/lib/build-paths.sh"

NATIVE_DIR="$ROOT_DIR/src/crosshook-native"
TARGET_TRIPLE="${TARGET_TRIPLE:-x86_64-unknown-linux-gnu}"
export APPIMAGE_EXTRACT_AND_RUN="${APPIMAGE_EXTRACT_AND_RUN:-1}"
INSTALL_DEPS=0
INSTALL_DEPS_YES=0
BINARY_ONLY=0
PRINT_PATHS=0

stable_appimage_name() {
  local target_triple="$1"
  local arch_suffix

  arch_suffix="$(appimage_arch_suffix "$target_triple")"

  printf 'CrossHook_%s.AppImage\n' "$arch_suffix"
}

appimage_arch_suffix() {
  local target_triple="$1"

  case "$target_triple" in
    x86_64-*)
      printf 'amd64\n'
      ;;
    aarch64-*|arm64-*)
      printf 'arm64\n'
      ;;
    armv7-*)
      printf 'armv7\n'
      ;;
    *)
      printf '%s\n' "${target_triple%%-*}"
      ;;
  esac
}

usage() {
  cat <<'EOF'
Usage: ./scripts/build-native.sh [--binary-only] [--install-deps] [--yes] [--print-paths]

Build the native CrossHook target locally.

Options:
  --binary-only   Build the release binary only and skip AppImage bundling
  --install-deps  Install missing host build dependencies first
  --yes, -y       Forward non-interactive install mode to install-native-build-deps.sh
  --print-paths   Print resolved DIST_DIR and CARGO_TARGET_DIR and exit
  --help, -h      Show this help text

Environment:
  DIST_DIR              Output directory for binary/AppImage copies (default: XDG data)
  CARGO_TARGET_DIR      Cargo artifact directory (default: XDG cache)
  CROSSHOOK_BUILD_EPHEMERAL=1  Use /tmp/crosshook-$UID for outputs
  CROSSHOOK_CONTAINER_BUILD=1  Internal override for container-only path mode
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
    --print-paths)
      PRINT_PATHS=1
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

if (( PRINT_PATHS )); then
  CROSSHOOK_SKIP_MKDIR=1 crosshook_build_paths_init || exit 1
  export CARGO_TARGET_DIR
  echo "DIST_DIR=$DIST_DIR"
  echo "CARGO_TARGET_DIR=$CARGO_TARGET_DIR"
  exit 0
fi

crosshook_build_paths_init || exit 1
export CARGO_TARGET_DIR

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
  echo "  CARGO_TARGET_DIR=$CARGO_TARGET_DIR"
  echo "  DIST_DIR=$DIST_DIR"
  npm run build
  cargo build \
    --manifest-path src-tauri/Cargo.toml \
    --release \
    --target "$TARGET_TRIPLE"

  BINARY_PATH="$CARGO_TARGET_DIR/$TARGET_TRIPLE/release/crosshook-native"
  [[ -x "$BINARY_PATH" ]] || die "release binary not found at $BINARY_PATH"

  mkdir -p "$DIST_DIR"
  cp -f "$BINARY_PATH" "$DIST_DIR/"

  echo "Built release binary:"
  echo "  $BINARY_PATH"
  echo "Copied binary to:"
  echo "  $DIST_DIR/$(basename "$BINARY_PATH")"
  exit 0
fi

echo "Generating branding assets and syncing Tauri AppImage icon..."
"$ROOT_DIR/scripts/generate-assets.sh"
"$ROOT_DIR/scripts/lib/sync-tauri-icons.sh"

echo "Building CrossHook Native AppImage for $TARGET_TRIPLE..."
echo "  CARGO_TARGET_DIR=$CARGO_TARGET_DIR"
echo "  DIST_DIR=$DIST_DIR"
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
while IFS= read -r bundle_dir; do
  [[ -n "$bundle_dir" ]] || continue
  if [[ -d "$bundle_dir" ]]; then
    candidate="$(find "$bundle_dir" -maxdepth 1 -type f -name '*.AppImage' | sort | tail -n 1)"
    if [[ -n "$candidate" ]]; then
      APPIMAGE_SOURCE="$candidate"
      break
    fi
  fi
done < <(crosshook_appimage_bundle_dirs "$NATIVE_DIR" "$TARGET_TRIPLE" "$CARGO_TARGET_DIR")

[[ -n "$APPIMAGE_SOURCE" ]] || die "AppImage output not found after build"

mkdir -p "$DIST_DIR"
ARCH_SUFFIX="$(appimage_arch_suffix "$TARGET_TRIPLE")"
find "$DIST_DIR" -maxdepth 1 -type f \
  \( -name "CrossHook_*_${ARCH_SUFFIX}.AppImage" -o -name "CrossHook_${ARCH_SUFFIX}.AppImage" \) \
  -delete
cp -f "$APPIMAGE_SOURCE" "$DIST_DIR/"

STABLE_APPIMAGE_NAME="$(stable_appimage_name "$TARGET_TRIPLE")"
cp -f "$APPIMAGE_SOURCE" "$DIST_DIR/$STABLE_APPIMAGE_NAME"

echo "Copied AppImage to $DIST_DIR/$(basename "$APPIMAGE_SOURCE")"
echo "Copied stable AppImage alias to $DIST_DIR/$STABLE_APPIMAGE_NAME"
