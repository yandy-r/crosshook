#!/usr/bin/env bash
set -euo pipefail

ASSUME_YES=0

usage() {
  cat <<'EOF'
Usage: ./scripts/install-native-build-deps.sh [--yes]

Install the host packages required to build the native Tauri/AppImage target.

Options:
  --yes, -y   Run the package-manager install non-interactively where supported
  --help, -h  Show this help text
EOF
}

die() {
  echo "Error: $*" >&2
  exit 1
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --yes|-y)
      ASSUME_YES=1
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

run_with_sudo() {
  if [[ ${EUID:-$(id -u)} -eq 0 ]]; then
    "$@"
  elif command -v sudo >/dev/null 2>&1; then
    sudo "$@"
  else
    die "sudo is required to install system packages"
  fi
}

install_with_pacman() {
  local -a packages=(
    base-devel
    curl
    file
    gtk3
    libsoup3
    nodejs
    npm
    openssl
    patchelf
    pkgconf
    rust
    webkit2gtk-4.1
  )
  local -a cmd=(pacman -S --needed)
  (( ASSUME_YES )) && cmd+=(--noconfirm)
  cmd+=("${packages[@]}")
  run_with_sudo "${cmd[@]}"
}

install_with_apt() {
  local -a packages=(
    build-essential
    curl
    file
    libgtk-3-dev
    libsoup-3.0-dev
    libssl-dev
    libwebkit2gtk-4.1-dev
    nodejs
    npm
    patchelf
    pkg-config
    rustc
    cargo
  )
  run_with_sudo apt-get update
  local -a cmd=(apt-get install)
  (( ASSUME_YES )) && cmd+=(-y)
  cmd+=("${packages[@]}")
  run_with_sudo "${cmd[@]}"
}

install_with_dnf() {
  local -a packages=(
    cargo
    curl
    file
    gcc
    gcc-c++
    gtk3-devel
    libsoup3-devel
    make
    nodejs
    npm
    openssl-devel
    patchelf
    pkgconf-pkg-config
    rust
    webkit2gtk4.1-devel
  )
  local -a cmd=(dnf install)
  (( ASSUME_YES )) && cmd+=(-y)
  cmd+=("${packages[@]}")
  run_with_sudo "${cmd[@]}"
}

install_with_zypper() {
  local -a packages=(
    cargo
    curl
    file
    gcc
    gcc-c++
    gtk3-devel
    libsoup-3_0-devel
    make
    nodejs
    npm
    libopenssl-devel
    patchelf
    pkgconf-pkg-config
    rust
    webkit2gtk3-devel
  )
  local -a cmd=(zypper install)
  (( ASSUME_YES )) && cmd+=(-y)
  cmd+=("${packages[@]}")
  run_with_sudo "${cmd[@]}"
}

if command -v pacman >/dev/null 2>&1; then
  echo "Installing native build dependencies with pacman..."
  install_with_pacman
elif command -v apt-get >/dev/null 2>&1; then
  echo "Installing native build dependencies with apt-get..."
  install_with_apt
elif command -v dnf >/dev/null 2>&1; then
  echo "Installing native build dependencies with dnf..."
  install_with_dnf
elif command -v zypper >/dev/null 2>&1; then
  echo "Installing native build dependencies with zypper..."
  install_with_zypper
else
  die "unsupported package manager; install cargo, npm, patchelf, GTK3, libsoup3, and webkit2gtk 4.1 manually"
fi

echo "Native build dependencies are installed."
