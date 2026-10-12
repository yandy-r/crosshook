#!/usr/bin/env bash
# Shared dependency installation and preflight for the host-built Tauri binary.

native_build_deps_manager() {
  local os_release="${CROSSHOOK_OS_RELEASE_FILE:-/etc/os-release}"
  local ID="" ID_LIKE="" family
  [[ -r "$os_release" ]] || { echo "Cannot read $os_release" >&2; return 1; }
  # shellcheck disable=SC1090
  source "$os_release"
  if [[ -e /run/ostree-booted ]]; then
    echo "Immutable OSTree host: build in a mutable Toolbox/Distrobox container; host package installation requires layering and a reboot." >&2
    return 1
  fi
  for family in $ID $ID_LIKE; do
    case "$family" in
      arch|manjaro) echo pacman; return ;;
      debian|ubuntu) echo apt-get; return ;;
      fedora|rhel|centos) echo dnf; return ;;
      opensuse*|suse|sles) echo zypper; return ;;
    esac
  done
  echo "Unsupported Linux distribution: ${ID:-unknown}. Use a supported mutable Debian/Ubuntu, Fedora, Arch, or openSUSE build environment." >&2
  return 1
}

native_build_deps_privileged() {
  if (( EUID == 0 )); then
    "$@"
  elif command -v sudo >/dev/null 2>&1; then
    sudo "$@"
  else
    echo "sudo is required to install system packages" >&2
    return 1
  fi
}

native_build_deps_install() {
  local assume_yes="$1" with_flatpak="$2" manager
  manager="$(native_build_deps_manager)" || return 1
  command -v "$manager" >/dev/null 2>&1 || {
    echo "Expected package manager $manager is unavailable" >&2
    return 1
  }
  local -a packages=() cmd=() rust_packages=(rust cargo)
  case "$manager" in
    pacman)
      packages=(base-devel curl file gtk3 libsoup3 openssl pkgconf webkit2gtk-4.1)
      rust_packages=(rust)
      (( with_flatpak )) && packages+=(flatpak flatpak-builder librsvg imagemagick desktop-file-utils appstream)
      cmd=(pacman -S --needed)
      (( assume_yes )) && cmd+=(--noconfirm)
      ;;
    apt-get)
      packages=(build-essential curl file libgtk-3-dev libsoup-3.0-dev libssl-dev libwebkit2gtk-4.1-dev pkg-config)
      rust_packages=(rustc cargo)
      (( with_flatpak )) && packages+=(flatpak flatpak-builder librsvg2-bin imagemagick desktop-file-utils appstream)
      native_build_deps_privileged apt-get update || return 1
      cmd=(apt-get install)
      (( assume_yes )) && cmd+=(-y)
      ;;
    dnf)
      packages=(curl file gcc gcc-c++ make gtk3-devel libsoup3-devel openssl-devel pkgconf-pkg-config webkit2gtk4.1-devel)
      (( with_flatpak )) && packages+=(flatpak flatpak-builder librsvg2-tools ImageMagick desktop-file-utils appstream)
      cmd=(dnf install)
      (( assume_yes )) && cmd+=(-y)
      ;;
    zypper)
      packages=(curl file gcc gcc-c++ make gtk3-devel libsoup-3_0-devel libopenssl-devel pkgconf-pkg-config webkit2gtk3-devel)
      (( with_flatpak )) && packages+=(flatpak flatpak-builder rsvg-convert ImageMagick desktop-file-utils AppStream)
      cmd=(zypper)
      (( assume_yes )) && cmd+=(--non-interactive)
      cmd+=(install)
      ;;
  esac
  command -v cargo >/dev/null 2>&1 && command -v rustc >/dev/null 2>&1 || packages+=("${rust_packages[@]}")
  command -v node >/dev/null 2>&1 || packages+=(nodejs)
  command -v npm >/dev/null 2>&1 || packages+=(npm)
  echo "Installing host build dependencies with $manager..."
  native_build_deps_privileged "${cmd[@]}" "${packages[@]}"
}

native_build_deps_check() {
  local with_flatpak="${1:-0}" tool library missing=0
  local -a tools=(cargo rustc node npm cc c++ make pkg-config curl file)
  # Development packages above bring these in transitively. Check the complete
  # GTK/WebKit sys-crate chain so one preflight reports all missing .pc files.
  local -a libraries=(
    'glib-2.0 >= 2.70' 'gobject-2.0 >= 2.70' 'gio-2.0 >= 2.70'
    'cairo >= 1.14' 'cairo-gobject >= 1.14' 'pango >= 1.40'
    'atk >= 2.28' 'gdk-pixbuf-2.0 >= 2.36.8'
    'gdk-3.0 >= 3.22' 'gdk-x11-3.0 >= 3.22' 'gdk-wayland-3.0 >= 3.22'
    'gtk+-3.0 >= 3.22'
    'libsoup-3.0 >= 3.0' 'javascriptcoregtk-4.1 >= 2.38'
    'webkit2gtk-4.1 >= 2.40' openssl
  )
  (( with_flatpak )) && tools+=(flatpak flatpak-builder rsvg-convert desktop-file-validate appstreamcli)
  for tool in "${tools[@]}"; do
    if ! command -v "$tool" >/dev/null 2>&1; then
      echo "Missing build tool: $tool" >&2
      missing=1
    fi
  done
  if command -v pkg-config >/dev/null 2>&1; then
    for library in "${libraries[@]}"; do
      if ! pkg-config --exists "$library"; then
        echo "Missing or incompatible development library: $library" >&2
        pkg-config --print-errors --exists "$library" >&2 || true
        missing=1
      fi
    done
  fi
  if (( with_flatpak )) && ! command -v magick >/dev/null 2>&1 && ! command -v convert >/dev/null 2>&1; then
    echo "Missing build tool: ImageMagick (magick or convert)" >&2
    missing=1
  fi
  if (( missing )); then
    echo "Run npm run flatpak:setup (or scripts/install-native-build-deps.sh). If packages are installed, check PKG_CONFIG_PATH/PKG_CONFIG_LIBDIR and the selected build environment." >&2
    return 1
  fi
}
