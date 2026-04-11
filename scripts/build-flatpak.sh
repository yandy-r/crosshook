#!/usr/bin/env bash
# Build a CrossHook Flatpak bundle from the native release binary.
#
# Stages the pre-built binary, runtime helper scripts, branding icons,
# desktop entry, and AppStream metadata into a temporary directory alongside
# a copy of the committed manifest, then runs flatpak-builder + flatpak
# build-bundle to produce an installable .flatpak file at
# $DIST_DIR/CrossHook_<arch>.flatpak.
#
# This is the Phase 1 bundle path (see docs/prps/prds/flatpak-distribution.prd.md).
# It uses the manifest's `simple` buildsystem and a pre-built binary so there
# is no Rust build inside the Flatpak sandbox. A Flathub-ready manifest that
# builds from source with flatpak-cargo-generator lives in Phase 4 work.
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
# shellcheck source=lib/build-paths.sh
source "$ROOT_DIR/scripts/lib/build-paths.sh"

APP_ID="dev.crosshook.CrossHook"
MANIFEST_NAME="${APP_ID}.yml"
DESKTOP_NAME="${APP_ID}.desktop"
METAINFO_NAME="${APP_ID}.metainfo.xml"

FLATPAK_DIR="$ROOT_DIR/packaging/flatpak"
MANIFEST_SRC="$FLATPAK_DIR/$MANIFEST_NAME"
DESKTOP_SRC="$FLATPAK_DIR/$DESKTOP_NAME"
METAINFO_SRC="$FLATPAK_DIR/$METAINFO_NAME"

NATIVE_DIR="$ROOT_DIR/src/crosshook-native"
HELPERS_DIR="$NATIVE_DIR/runtime-helpers"
ASSETS_DIR="$ROOT_DIR/assets"

TARGET_TRIPLE="${TARGET_TRIPLE:-x86_64-unknown-linux-gnu}"
RUNTIME_VERSION="${CROSSHOOK_FLATPAK_RUNTIME_VERSION:-50}"

BINARY_ONLY=0
INSTALL_DEPS=0
INSTALL_DEPS_YES=0
KEEP_STAGING=0
INSTALL_BUNDLE=0
VALIDATE_STRICT=0

usage() {
  cat <<'EOF'
Usage: ./scripts/build-flatpak.sh [options]

Build a Flatpak bundle for CrossHook from the pre-built native release
binary. If the binary is missing, falls back to running
./scripts/build-native.sh --binary-only first.

Options:
  --binary-only     Skip automatic build-native.sh invocation; require the
                    binary to exist at $DIST_DIR/crosshook-native already
  --install-deps    Install flatpak, flatpak-builder, and the GNOME runtime
                    + SDK on the host before building
  --yes, -y         Forward non-interactive install mode to apt/dnf/pacman
  --keep-staging    Do not delete the staging directory after the build
  --install         After building, flatpak install --user the bundle
  --strict          Fail the build if desktop-file-validate or appstreamcli
                    validate reports errors (default: warn and continue)
  --help, -h        Show this help text

Environment:
  DIST_DIR                            Output directory for the bundle
                                      (default: XDG data via build-paths.sh)
  CARGO_TARGET_DIR                    Cargo artifact dir (default: XDG cache)
  CROSSHOOK_BUILD_EPHEMERAL=1         Use /tmp/crosshook-$UID/ for outputs
  CROSSHOOK_FLATPAK_RUNTIME_VERSION   Override the GNOME runtime version
                                      (default: 50, matches the manifest)
  CROSSHOOK_FLATPAK_VALIDATE_STRICT   If set to 1/true/yes/on, same as --strict
                                      for metadata validators
EOF
}

die() {
  echo "Error: $*" >&2
  exit 1
}

log() {
  echo "[build-flatpak] $*"
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --binary-only) BINARY_ONLY=1; shift ;;
    --install-deps) INSTALL_DEPS=1; shift ;;
    --yes|-y) INSTALL_DEPS_YES=1; shift ;;
    --keep-staging) KEEP_STAGING=1; shift ;;
    --install) INSTALL_BUNDLE=1; shift ;;
    --strict) VALIDATE_STRICT=1; shift ;;
    --help|-h) usage; exit 0 ;;
    *) die "unknown argument: $1" ;;
  esac
done

case "${CROSSHOOK_FLATPAK_VALIDATE_STRICT:-}" in
  1|true|TRUE|yes|YES|on|ON) VALIDATE_STRICT=1 ;;
esac

crosshook_build_paths_init || exit 1
export CARGO_TARGET_DIR

arch_suffix_for_triple() {
  case "$1" in
    x86_64-*)        printf 'amd64\n' ;;
    aarch64-*|arm64-*) printf 'arm64\n' ;;
    armv7-*)         printf 'armv7\n' ;;
    *)               printf '%s\n' "${1%%-*}" ;;
  esac
}

flatpak_arch_for_triple() {
  case "$1" in
    x86_64-*)        printf 'x86_64\n' ;;
    aarch64-*|arm64-*) printf 'aarch64\n' ;;
    *)               printf '%s\n' "${1%%-*}" ;;
  esac
}

ARCH_SUFFIX="$(arch_suffix_for_triple "$TARGET_TRIPLE")"
FLATPAK_ARCH="$(flatpak_arch_for_triple "$TARGET_TRIPLE")"
BUNDLE_NAME="CrossHook_${ARCH_SUFFIX}.flatpak"
BUNDLE_PATH="$DIST_DIR/$BUNDLE_NAME"

# ---- Optional: install host tooling + runtime ------------------------------
if (( INSTALL_DEPS )); then
  log "installing flatpak toolchain and GNOME ${RUNTIME_VERSION} runtime"
  if command -v pacman >/dev/null 2>&1; then
    sudo_cmd=()
    (( EUID != 0 )) && sudo_cmd=(sudo)
    pacman_args=(-S --needed flatpak flatpak-builder)
    (( INSTALL_DEPS_YES )) && pacman_args+=(--noconfirm)
    "${sudo_cmd[@]}" pacman "${pacman_args[@]}"
  elif command -v dnf >/dev/null 2>&1; then
    sudo_cmd=()
    (( EUID != 0 )) && sudo_cmd=(sudo)
    dnf_args=(install flatpak flatpak-builder)
    (( INSTALL_DEPS_YES )) && dnf_args+=(-y)
    "${sudo_cmd[@]}" dnf "${dnf_args[@]}"
  elif command -v apt-get >/dev/null 2>&1; then
    sudo_cmd=()
    (( EUID != 0 )) && sudo_cmd=(sudo)
    apt_args=(install flatpak flatpak-builder)
    (( INSTALL_DEPS_YES )) && apt_args+=(-y)
    "${sudo_cmd[@]}" apt-get update
    "${sudo_cmd[@]}" apt-get "${apt_args[@]}"
  else
    die "no supported package manager found; install flatpak + flatpak-builder manually"
  fi

  if ! flatpak remote-list --user 2>/dev/null | grep -q '^flathub'; then
    flatpak remote-add --user --if-not-exists flathub \
      https://flathub.org/repo/flathub.flatpakrepo
  fi

  flatpak install --user --noninteractive flathub \
    "org.gnome.Platform//${RUNTIME_VERSION}" \
    "org.gnome.Sdk//${RUNTIME_VERSION}"
fi

# ---- Preflight -------------------------------------------------------------
command -v flatpak-builder >/dev/null 2>&1 \
  || die "flatpak-builder not installed (try --install-deps)"
command -v flatpak >/dev/null 2>&1 \
  || die "flatpak not installed (try --install-deps)"

[[ -f "$MANIFEST_SRC"  ]] || die "manifest not found: $MANIFEST_SRC"
[[ -f "$DESKTOP_SRC"   ]] || die "desktop entry not found: $DESKTOP_SRC"
[[ -f "$METAINFO_SRC"  ]] || die "metainfo not found: $METAINFO_SRC"

if command -v desktop-file-validate >/dev/null 2>&1; then
  if ! desktop-file-validate "$DESKTOP_SRC"; then
    if (( VALIDATE_STRICT )); then
      die "desktop-file-validate failed on $DESKTOP_SRC"
    fi
    log "warning: desktop-file-validate failed on $DESKTOP_SRC (ignored; use --strict or CROSSHOOK_FLATPAK_VALIDATE_STRICT=1 to fail)"
  fi
fi
if command -v appstreamcli >/dev/null 2>&1; then
  if ! appstreamcli validate "$METAINFO_SRC"; then
    if (( VALIDATE_STRICT )); then
      die "appstreamcli validate failed on $METAINFO_SRC"
    fi
    log "warning: appstreamcli validate failed on $METAINFO_SRC (ignored; use --strict or CROSSHOOK_FLATPAK_VALIDATE_STRICT=1 to fail)"
  fi
fi

# ---- Ensure the release binary exists --------------------------------------
BINARY_PATH="$DIST_DIR/crosshook-native"
if [[ ! -x "$BINARY_PATH" ]]; then
  if (( BINARY_ONLY )); then
    die "release binary not found at $BINARY_PATH and --binary-only was set"
  fi
  log "release binary missing, running build-native.sh --binary-only"
  "$ROOT_DIR/scripts/build-native.sh" --binary-only
fi
[[ -x "$BINARY_PATH" ]] || die "release binary still missing at $BINARY_PATH"

# ---- Ensure icon sizes exist -----------------------------------------------
for size in 128 256 512; do
  if [[ ! -f "$ASSETS_DIR/icon-${size}.png" ]]; then
    log "generating branding assets"
    "$ROOT_DIR/scripts/generate-assets.sh"
    break
  fi
done
for size in 128 256 512; do
  [[ -f "$ASSETS_DIR/icon-${size}.png" ]] \
    || die "expected $ASSETS_DIR/icon-${size}.png after generate-assets.sh"
done

# ---- Stage --------------------------------------------------------------
STAGE_DIR="$(mktemp -d -t crosshook-flatpak-stage.XXXXXX)"
cleanup() {
  if (( KEEP_STAGING )); then
    log "keeping staging dir: $STAGE_DIR"
  else
    rm -rf "$STAGE_DIR"
  fi
}
trap cleanup EXIT

log "staging -> $STAGE_DIR"
install -Dm755 "$BINARY_PATH"             "$STAGE_DIR/crosshook-native"
install -Dm755 "$HELPERS_DIR/steam-launch-helper.sh"        "$STAGE_DIR/runtime-helpers/steam-launch-helper.sh"
install -Dm755 "$HELPERS_DIR/steam-launch-trainer.sh"       "$STAGE_DIR/runtime-helpers/steam-launch-trainer.sh"
install -Dm755 "$HELPERS_DIR/steam-host-trainer-runner.sh"  "$STAGE_DIR/runtime-helpers/steam-host-trainer-runner.sh"
install -Dm644 "$ASSETS_DIR/icon-128.png" "$STAGE_DIR/icon-128.png"
install -Dm644 "$ASSETS_DIR/icon-256.png" "$STAGE_DIR/icon-256.png"
install -Dm644 "$ASSETS_DIR/icon-512.png" "$STAGE_DIR/icon-512.png"
install -Dm644 "$DESKTOP_SRC"             "$STAGE_DIR/$DESKTOP_NAME"
install -Dm644 "$METAINFO_SRC"            "$STAGE_DIR/$METAINFO_NAME"
install -Dm644 "$MANIFEST_SRC"            "$STAGE_DIR/$MANIFEST_NAME"

# ---- Build ------------------------------------------------------------
BUILD_DIR="$STAGE_DIR/.flatpak-build"
REPO_DIR="$STAGE_DIR/.flatpak-repo"
STATE_DIR="$STAGE_DIR/.flatpak-state"
mkdir -p "$(dirname "$BUNDLE_PATH")"

log "flatpak-builder -> $BUILD_DIR"
(
  cd "$STAGE_DIR"
  flatpak-builder \
    --force-clean \
    --user \
    --arch="$FLATPAK_ARCH" \
    --state-dir="$STATE_DIR" \
    --repo="$REPO_DIR" \
    "$BUILD_DIR" \
    "$MANIFEST_NAME"
)

log "flatpak build-bundle -> $BUNDLE_PATH"
flatpak build-bundle \
  --arch="$FLATPAK_ARCH" \
  "$REPO_DIR" \
  "$BUNDLE_PATH" \
  "$APP_ID"

[[ -f "$BUNDLE_PATH" ]] || die "bundle was not produced at $BUNDLE_PATH"

bundle_size="$(du -h "$BUNDLE_PATH" | cut -f1)"
log "bundle ready: $BUNDLE_PATH ($bundle_size)"
log "install with: flatpak install --user --reinstall $BUNDLE_PATH"
log "run with:     flatpak run $APP_ID"

if (( INSTALL_BUNDLE )); then
  log "installing bundle locally"
  flatpak install --user --noninteractive --reinstall "$BUNDLE_PATH"
fi
