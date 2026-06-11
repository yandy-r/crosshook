#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
RELEASE_BINARY_SCRIPT="$ROOT_DIR/scripts/build-release-binary.sh"
FORWARD_ARGS=()

usage() {
  cat <<'EOF'
Usage: ./scripts/build-native.sh [--binary-only] [--install-deps] [--yes] [--print-paths]

Compatibility shim for the former native/AppImage build helper.

AppImage bundling has been removed. Use ./scripts/build-release-binary.sh
to build the release binary consumed by Flatpak packaging.

Legacy options:
  --binary-only   Accepted for compatibility; release binary build is now the only behavior
  --install-deps  Forward to build-release-binary.sh
  --yes, -y       Forward to build-release-binary.sh
  --print-paths   Forward to build-release-binary.sh
  --help, -h      Show this help text
EOF
}

die() {
  echo "Error: $*" >&2
  exit 1
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --binary-only)
      shift
      ;;
    --install-deps|--yes|-y|--print-paths)
      FORWARD_ARGS+=("$1")
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

echo "scripts/build-native.sh is deprecated; use scripts/build-release-binary.sh." >&2
exec "$RELEASE_BINARY_SCRIPT" "${FORWARD_ARGS[@]}"
