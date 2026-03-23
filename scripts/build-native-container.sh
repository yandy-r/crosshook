#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
RUNTIME=""
IMAGE="${IMAGE:-rust:1-bookworm}"
TARGET_TRIPLE="${TARGET_TRIPLE:-x86_64-unknown-linux-gnu}"
DIST_DIR="${DIST_DIR:-$ROOT_DIR/dist}"
INSTALL_NODE_MODULES=0
KEEP_WORKTREE_ARTIFACTS=0

usage() {
  cat <<'EOF'
Usage: ./scripts/build-native-container.sh [--runtime docker|podman] [--image IMAGE] [--install-node-modules] [--keep-worktree-artifacts]

Build the native AppImage inside a container to avoid host linuxdeploy/AppImage toolchain issues.

Options:
  --runtime RUNTIME         Explicitly choose docker or podman
  --image IMAGE             Override the container image (default: rust:1-bookworm)
  --install-node-modules    Force npm ci inside the container even if node_modules already exists
  --keep-worktree-artifacts Keep src/crosshook-native build artifacts after the container build
  --help, -h                Show this help text
EOF
}

die() {
  echo "Error: $*" >&2
  exit 1
}

while [[ $# -gt 0 ]]; do
  case "$1" in
    --runtime)
      RUNTIME="${2:-}"
      [[ -n "$RUNTIME" ]] || die "--runtime requires a value"
      shift 2
      ;;
    --image)
      IMAGE="${2:-}"
      [[ -n "$IMAGE" ]] || die "--image requires a value"
      shift 2
      ;;
    --install-node-modules)
      INSTALL_NODE_MODULES=1
      shift
      ;;
    --keep-worktree-artifacts)
      KEEP_WORKTREE_ARTIFACTS=1
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

if [[ -z "$RUNTIME" ]]; then
  if command -v podman >/dev/null 2>&1; then
    RUNTIME="podman"
  elif command -v docker >/dev/null 2>&1; then
    RUNTIME="docker"
  else
    die "docker or podman is required"
  fi
fi

command -v "$RUNTIME" >/dev/null 2>&1 || die "$RUNTIME is not installed"

HOST_UID="$(id -u)"
HOST_GID="$(id -g)"

read -r -d '' CONTAINER_SCRIPT <<'EOF' || true
set -euo pipefail

export DEBIAN_FRONTEND=noninteractive
apt-get update
apt-get install -y --no-install-recommends \
  ca-certificates \
  curl \
  file \
  git \
  libayatana-appindicator3-dev \
  libgtk-3-dev \
  librsvg2-dev \
  libsoup-3.0-dev \
  libwebkit2gtk-4.1-dev \
  nodejs \
  npm \
  patchelf \
  pkg-config
rm -rf /var/lib/apt/lists/*

export PATH="/usr/local/cargo/bin:$HOME/.cargo/bin:$PATH"

if ! command -v cargo >/dev/null 2>&1 || \
   ! command -v rustc >/dev/null 2>&1 || \
   ! cargo metadata --manifest-path /workspace/src/crosshook-native/Cargo.toml --format-version 1 --locked >/dev/null 2>&1
then
  curl https://sh.rustup.rs -sSf | sh -s -- -y --profile minimal --default-toolchain stable
  export PATH="$HOME/.cargo/bin:/usr/local/cargo/bin:$PATH"
fi

cd /workspace/src/crosshook-native
if [[ ! -x node_modules/.bin/tauri || "${INSTALL_NODE_MODULES}" == "1" ]]; then
  npm ci
fi

cd /workspace
APPIMAGE_EXTRACT_AND_RUN=1 TARGET_TRIPLE="${TARGET_TRIPLE}" ./scripts/build-native.sh

for path in \
  /workspace/dist
do
  if [[ -e "$path" ]]; then
    chown -R "${HOST_UID}:${HOST_GID}" "$path"
  fi
done

if [[ "${KEEP_WORKTREE_ARTIFACTS}" != "1" ]]; then
  rm -rf \
    /workspace/src/crosshook-native/dist \
    /workspace/src/crosshook-native/node_modules \
    /workspace/src/crosshook-native/src-tauri/target \
    /workspace/src/crosshook-native/target
fi
EOF

"$RUNTIME" run --rm \
  -e HOST_UID="$HOST_UID" \
  -e HOST_GID="$HOST_GID" \
  -e TARGET_TRIPLE="$TARGET_TRIPLE" \
  -e INSTALL_NODE_MODULES="$INSTALL_NODE_MODULES" \
  -e KEEP_WORKTREE_ARTIFACTS="$KEEP_WORKTREE_ARTIFACTS" \
  -e APPIMAGE_EXTRACT_AND_RUN=1 \
  -v "$ROOT_DIR:/workspace" \
  -w /workspace \
  "$IMAGE" \
  bash -lc "$CONTAINER_SCRIPT"

echo "Containerized native build complete."
echo "AppImage location:"
echo "  $DIST_DIR"
