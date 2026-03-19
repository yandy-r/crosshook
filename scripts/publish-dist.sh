#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
PROJECT_PATH="$ROOT_DIR/src/CrossHookEngine.App/CrossHookEngine.App.csproj"
DIST_DIR="${DIST_DIR:-$ROOT_DIR/dist}"
CONFIGURATION="${CONFIGURATION:-Release}"
RIDS=(win-x64 win-x86)

# Use project-local SDK if present
if [[ -d "$ROOT_DIR/.dotnet" ]]; then
  export PATH="$ROOT_DIR/.dotnet:$PATH"
  export DOTNET_CLI_HOME="$ROOT_DIR/.dotnet-cli-home"
  mkdir -p "$DOTNET_CLI_HOME"
fi

if (($# > 0)); then
  RIDS=("$@")
fi

if [[ ! -f "$PROJECT_PATH" ]]; then
  echo "Project file not found: $PROJECT_PATH" >&2
  exit 1
fi

mkdir -p "$DIST_DIR"
STAGING_DIR="$(mktemp -d "${TMPDIR:-/tmp}/crosshook-dist.XXXXXX")"
cleanup() {
  rm -rf "$STAGING_DIR"
}
trap cleanup EXIT

for rid in "${RIDS[@]}"; do
  ARTIFACT_NAME="crosshook-${rid}"
  RID_STAGE_DIR="$STAGING_DIR/$ARTIFACT_NAME"
  ZIP_PATH="$DIST_DIR/$ARTIFACT_NAME.zip"

  rm -rf "$RID_STAGE_DIR" "$DIST_DIR/$ARTIFACT_NAME" "$ZIP_PATH"
  mkdir -p "$RID_STAGE_DIR"

  echo "Publishing $rid..."
  dotnet publish "$PROJECT_PATH" \
    -c "$CONFIGURATION" \
    -r "$rid" \
    --self-contained true \
    -o "$RID_STAGE_DIR"

  rm -rf "$RID_STAGE_DIR/Profiles" "$RID_STAGE_DIR/Settings" "$RID_STAGE_DIR/settings.ini"
  rm -f "$RID_STAGE_DIR"/*.pdb

  mv "$RID_STAGE_DIR" "$DIST_DIR/$ARTIFACT_NAME"
  (
    cd "$DIST_DIR"
    zip -qr "$ZIP_PATH" "$ARTIFACT_NAME"
  )

  echo "Created:"
  echo "  $DIST_DIR/$ARTIFACT_NAME/"
  echo "  $ZIP_PATH"
done
