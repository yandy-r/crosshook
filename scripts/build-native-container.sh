#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
RUNTIME=""
IMAGE="${IMAGE:-}"
BASE_IMAGE="${BASE_IMAGE:-ubuntu:24.04}"
BUILDER_IMAGE_REPO="${BUILDER_IMAGE_REPO:-crosshook-native-builder}"
TARGET_TRIPLE="${TARGET_TRIPLE:-x86_64-unknown-linux-gnu}"
DIST_DIR="${DIST_DIR:-$ROOT_DIR/dist}"
DOCKERFILE_PATH="$ROOT_DIR/scripts/build-native-container.Dockerfile"
INSTALL_NODE_MODULES=0
KEEP_WORKTREE_ARTIFACTS=0
REBUILD_IMAGE=0

usage() {
  cat <<'EOF'
Usage: ./scripts/build-native-container.sh [--runtime docker|podman] [--image IMAGE] [--base-image IMAGE] [--rebuild-image] [--install-node-modules] [--keep-worktree-artifacts]

Build the native AppImage inside a container to avoid host linuxdeploy/AppImage toolchain issues.

Options:
  --runtime RUNTIME         Explicitly choose docker or podman
  --image IMAGE             Use IMAGE directly instead of the managed cached builder image
  --base-image IMAGE        Base image for the managed cached builder image (default: ubuntu:24.04)
  --rebuild-image           Force rebuilding the managed cached builder image
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
    --base-image)
      BASE_IMAGE="${2:-}"
      [[ -n "$BASE_IMAGE" ]] || die "--base-image requires a value"
      shift 2
      ;;
    --rebuild-image)
      REBUILD_IMAGE=1
      shift
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

hash_file() {
  local file_path="$1"

  if command -v sha256sum >/dev/null 2>&1; then
    sha256sum "$file_path" | awk '{print $1}'
  elif command -v shasum >/dev/null 2>&1; then
    shasum -a 256 "$file_path" | awk '{print $1}'
  else
    die "sha256sum or shasum is required"
  fi
}

ensure_builder_image() {
  local dockerfile_hash builder_image_tag

  [[ -f "$DOCKERFILE_PATH" ]] || die "builder Dockerfile not found: $DOCKERFILE_PATH"

  dockerfile_hash="$(hash_file "$DOCKERFILE_PATH")"
  builder_image_tag="${BUILDER_IMAGE_REPO}:${dockerfile_hash:0:12}"

  if (( REBUILD_IMAGE )) || ! "$RUNTIME" image inspect "$builder_image_tag" >/dev/null 2>&1; then
    echo "Building cached native builder image: $builder_image_tag" >&2
    "$RUNTIME" build \
      --build-arg "BASE_IMAGE=$BASE_IMAGE" \
      -f "$DOCKERFILE_PATH" \
      -t "$builder_image_tag" \
      "$ROOT_DIR"
  else
    echo "Reusing cached native builder image: $builder_image_tag" >&2
  fi

  printf '%s\n' "$builder_image_tag"
}

if [[ -z "$IMAGE" ]]; then
  IMAGE="$(ensure_builder_image)"
else
  echo "Using explicit container image: $IMAGE"
fi

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
  bash -c '
set -euo pipefail
export PATH="/usr/local/cargo/bin:/root/.cargo/bin:$PATH"

fix_ownership() {
  local path

  for path in \
    /workspace/dist \
    /workspace/src/crosshook-native/dist \
    /workspace/src/crosshook-native/node_modules \
    /workspace/src/crosshook-native/src-tauri/target \
    /workspace/src/crosshook-native/target
  do
    if [[ -e "$path" ]]; then
      chown -R "${HOST_UID}:${HOST_GID}" "$path" || true
    fi
  done
}

cleanup() {
  local exit_code=$?

  fix_ownership

  if (( exit_code == 0 )) && [[ "${KEEP_WORKTREE_ARTIFACTS}" != "1" ]]; then
    rm -rf \
      /workspace/src/crosshook-native/dist \
      /workspace/src/crosshook-native/node_modules \
      /workspace/src/crosshook-native/src-tauri/target \
      /workspace/src/crosshook-native/target
  fi

  exit "$exit_code"
}

trap cleanup EXIT

cd /workspace/src/crosshook-native
if [[ ! -x node_modules/.bin/tauri || "${INSTALL_NODE_MODULES}" == "1" ]]; then
  npm ci
fi

cd /workspace
APPIMAGE_EXTRACT_AND_RUN=1 TARGET_TRIPLE="${TARGET_TRIPLE}" ./scripts/build-native.sh
'

echo "Containerized native build complete."
echo "AppImage location:"
echo "  $DIST_DIR"
