#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
# shellcheck source=lib/build-paths.sh
source "$ROOT_DIR/scripts/lib/build-paths.sh"

NATIVE_DIR="$ROOT_DIR/src/crosshook-native"
TARGET_TRIPLE="${TARGET_TRIPLE:-x86_64-unknown-linux-gnu}"
INSTALL_DEPS=0
INSTALL_DEPS_YES=0
PRINT_PATHS=0

usage() {
  cat <<'EOF'
Usage: ./scripts/build-release-binary.sh [--install-deps] [--yes] [--print-paths]

Build the CrossHook release binary used as a Flatpak packaging input.

The script runs `tauri build --no-bundle` so Tauri embeds the production
frontend into DIST_DIR/crosshook-native without producing a distribution
bundle.

Options:
  --install-deps  Install missing host build dependencies first
  --yes, -y       Forward non-interactive install mode to install-native-build-deps.sh
  --print-paths   Print resolved DIST_DIR and CARGO_TARGET_DIR and exit
  --help, -h      Show this help text

Environment:
  DIST_DIR              Output directory for the release binary (default: XDG data)
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

cd "$NATIVE_DIR"

if [[ ! -x "$NATIVE_DIR/node_modules/.bin/tauri" ]]; then
  echo "Installing local npm dependencies..."
  npm ci
fi

echo "Building CrossHook release binary for $TARGET_TRIPLE..."
echo "  CARGO_TARGET_DIR=$CARGO_TARGET_DIR"
echo "  DIST_DIR=$DIST_DIR"

# Invoke `tauri build --no-bundle` so Tauri's generate_context!() macro
# embeds the production frontendDist into the binary. A plain
# `cargo build --release` does not: the resulting binary falls back
# to the devUrl at runtime (http://localhost:5173) and fails with
# "Could not connect to localhost: Connection refused" inside the
# Flatpak sandbox.
if cargo tauri --help >/dev/null 2>&1; then
  cargo tauri build --target "$TARGET_TRIPLE" --no-bundle
elif [[ -x "$NATIVE_DIR/node_modules/.bin/tauri" ]]; then
  "$NATIVE_DIR/node_modules/.bin/tauri" build --target "$TARGET_TRIPLE" --no-bundle
elif command -v npx >/dev/null 2>&1; then
  npx tauri build --target "$TARGET_TRIPLE" --no-bundle
else
  die "neither cargo-tauri nor a local tauri CLI is available"
fi

BINARY_PATH="$CARGO_TARGET_DIR/$TARGET_TRIPLE/release/crosshook-native"
[[ -x "$BINARY_PATH" ]] || die "release binary not found at $BINARY_PATH"

mkdir -p "$DIST_DIR"
cp -f "$BINARY_PATH" "$DIST_DIR/"
printf '%s\n' "$TARGET_TRIPLE" > "$DIST_DIR/crosshook-native.target-triple"

echo "Built release binary:"
echo "  $BINARY_PATH"
echo "Copied binary to:"
echo "  $DIST_DIR/$(basename "$BINARY_PATH")"
