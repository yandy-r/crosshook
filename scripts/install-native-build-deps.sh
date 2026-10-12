#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
# shellcheck source=lib/native-build-deps.sh
source "$ROOT_DIR/scripts/lib/native-build-deps.sh"

ASSUME_YES=0
WITH_FLATPAK=0
CHECK_ONLY=0

usage() {
  cat <<'EOF'
Usage: ./scripts/install-native-build-deps.sh [--yes] [--flatpak] [--check]

Install and verify host dependencies for the Tauri release binary.
Selects apt, dnf, pacman, or zypper from /etc/os-release (including ID_LIKE).
Package managers resolve transitive development dependencies such as Cairo,
GLib, Pango, ATK, and GDK alongside GTK3, libsoup3, WebKitGTK 4.1, and OpenSSL.

Options:
  --flatpak   Also install Flatpak, icon tools, and metadata validators
  --check     Check tools and pkg-config libraries without installing anything
  --yes, -y   Run package installation non-interactively (sudo may prompt)
  --help, -h  Show this help text

For the complete build/install workflow, run npm run flatpak:setup.
Immutable hosts should run builds inside a mutable development container.
EOF
}

die() {
  echo "Error: $*" >&2
  exit 1
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --yes|-y) ASSUME_YES=1; shift ;;
    --flatpak) WITH_FLATPAK=1; shift ;;
    --check) CHECK_ONLY=1; shift ;;
    --help|-h) usage; exit 0 ;;
    *) die "unknown argument: $1" ;;
  esac
done

if (( ! CHECK_ONLY )); then
  native_build_deps_install "$ASSUME_YES" "$WITH_FLATPAK"
fi
native_build_deps_check "$WITH_FLATPAK"
echo "Release-binary build dependencies are ready."
