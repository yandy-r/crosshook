#!/usr/bin/env bash
# Shared defaults for CrossHook native build output locations.
# Source after ROOT_DIR is set. Pre-set DIST_DIR / CARGO_TARGET_DIR always win.
#
# Modes:
# - Default (host): XDG data for artifacts, XDG cache for Cargo target dir.
# - CROSSHOOK_BUILD_EPHEMERAL=1: everything under /tmp/crosshook-$UID/
# - CROSSHOOK_CONTAINER_BUILD=1: paths under $ROOT_DIR/.crosshook-build/ (bind-mount only)

crosshook_build_paths_init() {
  if [[ -z "${ROOT_DIR:-}" ]]; then
    echo "crosshook_build_paths_init: ROOT_DIR must be set" >&2
    return 1
  fi

  local skip_mkdir="${CROSSHOOK_SKIP_MKDIR:-0}"

  if [[ "${CROSSHOOK_CONTAINER_BUILD:-0}" == "1" ]]; then
    local base="$ROOT_DIR/.crosshook-build"
    if [[ -z "${DIST_DIR:-}" ]]; then
      export DIST_DIR="$base/artifacts"
    fi
    if [[ -z "${CARGO_TARGET_DIR:-}" ]]; then
      export CARGO_TARGET_DIR="$base/cargo-target"
    fi
    export CROSSHOOK_DATA_HOME="${CROSSHOOK_DATA_HOME:-$base}"
    export CROSSHOOK_CACHE_HOME="${CROSSHOOK_CACHE_HOME:-$base/cache}"
    if [[ "$skip_mkdir" != "1" ]]; then
      mkdir -p "$DIST_DIR" "$CARGO_TARGET_DIR"
    fi
    return 0
  fi

  if [[ "${CROSSHOOK_BUILD_EPHEMERAL:-0}" == "1" ]]; then
    local ebase="/tmp/crosshook-${UID}"
    if [[ -z "${DIST_DIR:-}" ]]; then
      export DIST_DIR="$ebase/artifacts"
    fi
    if [[ -z "${CARGO_TARGET_DIR:-}" ]]; then
      export CARGO_TARGET_DIR="$ebase/build/cargo-target"
    fi
    export CROSSHOOK_DATA_HOME="${CROSSHOOK_DATA_HOME:-$ebase}"
    export CROSSHOOK_CACHE_HOME="${CROSSHOOK_CACHE_HOME:-$ebase/cache}"
    if [[ "$skip_mkdir" != "1" ]]; then
      mkdir -p "$DIST_DIR" "$CARGO_TARGET_DIR"
    fi
    return 0
  fi

  local data_home="${XDG_DATA_HOME:-$HOME/.local/share}/crosshook"
  local cache_home="${XDG_CACHE_HOME:-$HOME/.cache}/crosshook"
  if [[ -z "${DIST_DIR:-}" ]]; then
    export DIST_DIR="$data_home/artifacts"
  fi
  if [[ -z "${CARGO_TARGET_DIR:-}" ]]; then
    export CARGO_TARGET_DIR="$cache_home/build/cargo-target"
  fi
  export CROSSHOOK_DATA_HOME="${CROSSHOOK_DATA_HOME:-$data_home}"
  export CROSSHOOK_CACHE_HOME="${CROSSHOOK_CACHE_HOME:-$cache_home}"
  if [[ "$skip_mkdir" != "1" ]]; then
    mkdir -p "$DIST_DIR" "$CARGO_TARGET_DIR"
  fi
  return 0
}

# Print candidate AppImage bundle directories (newest layout first). Requires
# NATIVE_DIR, TARGET_TRIPLE, CARGO_TARGET_DIR.
crosshook_appimage_bundle_dirs() {
  local native_dir="${1:?}"
  local triple="${2:?}"
  local cargo_target="${3:?}"

  printf '%s\n' \
    "$cargo_target/$triple/release/bundle/appimage" \
    "$cargo_target/release/bundle/appimage" \
    "$native_dir/src-tauri/target/$triple/release/bundle/appimage" \
    "$native_dir/src-tauri/target/release/bundle/appimage" \
    "$native_dir/target/$triple/release/bundle/appimage" \
    "$native_dir/target/release/bundle/appimage"
}
