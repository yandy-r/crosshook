#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
CHANGELOG_PATH="$ROOT_DIR/CHANGELOG.md"
CLIFF_CONFIG_PATH="$ROOT_DIR/.git-cliff.toml"
NATIVE_WORKSPACE_MANIFEST="$ROOT_DIR/src/crosshook-native/Cargo.toml"
NATIVE_CARGO_MANIFESTS=(
  "$ROOT_DIR/src/crosshook-native/Cargo.toml"
  "$ROOT_DIR/src/crosshook-native/crates/crosshook-core/Cargo.toml"
  "$ROOT_DIR/src/crosshook-native/crates/crosshook-cli/Cargo.toml"
  "$ROOT_DIR/src/crosshook-native/src-tauri/Cargo.toml"
)
REMOTE="${REMOTE:-origin}"
PUSH=false
VERSION_INPUT=""

usage() {
  cat <<'EOF'
Usage:
  ./scripts/prepare-release.sh --version 5.1.0 [--push] [--remote origin]
  ./scripts/prepare-release.sh --tag v5.1.0 [--push] [--remote origin]

This script:
  1. Syncs the native workspace version
  2. Regenerates CHANGELOG.md with git-cliff
  3. Commits the release metadata update
  4. Creates an annotated release tag
  5. Optionally pushes the branch first and the tag second

Examples:
  ./scripts/prepare-release.sh --version 5.1.0
  ./scripts/prepare-release.sh --tag v5.1.0 --push
EOF
}

die() {
  echo "Error: $*" >&2
  exit 1
}

set_native_workspace_version() {
  local version="$1"
  local manifest

  for manifest in "${NATIVE_CARGO_MANIFESTS[@]}"; do
    [[ -f "$manifest" ]] || die "missing native manifest: $manifest"
  done

  CROSSHOOK_RELEASE_VERSION="$version" perl -0pi -e '
    my $version = $ENV{CROSSHOOK_RELEASE_VERSION};
    my $count = 0;
    $count += s/(\[workspace\.package\]\s*version = ")[^"]+(")/${1}${version}${2}/g;
    $count += s/(\[package\]\s*name = "[^"]+"\s*version = ")[^"]+(")/${1}${version}${2}/g;
    exit($count ? 0 : 1);
  ' "${NATIVE_CARGO_MANIFESTS[@]}" || die "failed to update native Cargo manifest versions"
}

normalize_tag() {
  local raw="$1"
  raw="${raw#refs/tags/}"

  if [[ "$raw" == v* ]]; then
    printf '%s\n' "$raw"
    return
  fi

  printf 'v%s\n' "$raw"
}

while (($# > 0)); do
  case "$1" in
    --version|--tag)
      (($# >= 2)) || die "$1 requires a value"
      [[ -z "$VERSION_INPUT" ]] || die "pass only one of --version or --tag"
      VERSION_INPUT="$2"
      shift 2
      ;;
    --push)
      PUSH=true
      shift
      ;;
    --remote)
      (($# >= 2)) || die "--remote requires a value"
      REMOTE="$2"
      shift 2
      ;;
    -h|--help)
      usage
      exit 0
      ;;
    *)
      die "unknown argument: $1"
      ;;
  esac
done

[[ -n "$VERSION_INPUT" ]] || die "pass --version or --tag"

cd "$ROOT_DIR"

git rev-parse --is-inside-work-tree >/dev/null 2>&1 || die "not inside a git repository"
command -v git-cliff >/dev/null 2>&1 || die "git-cliff is required. Install it first, for example: cargo install git-cliff --locked"
[[ -f "$CLIFF_CONFIG_PATH" ]] || die "missing git-cliff config: $CLIFF_CONFIG_PATH"

if [[ -n "$(git status --porcelain)" ]]; then
  die "working tree must be clean before preparing a release"
fi

TAG="$(normalize_tag "$VERSION_INPUT")"
VERSION="${TAG#v}"
BRANCH="$(git symbolic-ref --quiet --short HEAD 2>/dev/null || true)"

git rev-parse --verify "refs/tags/$TAG" >/dev/null 2>&1 && die "tag already exists: $TAG"
git config --get "remote.$REMOTE.url" >/dev/null 2>&1 || die "remote not found: $REMOTE"

set_native_workspace_version "$VERSION"

TEMP_CHANGELOG="$(mktemp "${TMPDIR:-/tmp}/crosshook-changelog.XXXXXX")"
cleanup() {
  rm -f "$TEMP_CHANGELOG"
}
trap cleanup EXIT

git-cliff --config "$CLIFF_CONFIG_PATH" --tag "$TAG" > "$TEMP_CHANGELOG"
mv "$TEMP_CHANGELOG" "$CHANGELOG_PATH"

git add CHANGELOG.md "${NATIVE_CARGO_MANIFESTS[@]}"

if git diff --cached --quiet; then
  die "CHANGELOG.md did not change for $TAG"
fi

git commit -m "chore(release): prepare $TAG"
git tag -a "$TAG" -m "Release $TAG"

echo "Prepared release $TAG"
echo "  commit: $(git rev-parse --short HEAD)"
echo "  tag:    $TAG"

if [[ "$PUSH" == true ]]; then
  [[ -n "$BRANCH" ]] || die "cannot push from detached HEAD"

  git push "$REMOTE" "$BRANCH"
  git push "$REMOTE" "refs/tags/$TAG"

  echo "Pushed branch $BRANCH and tag $TAG to $REMOTE"
else
  if [[ -n "$BRANCH" ]]; then
    echo "Next steps:"
    echo "  git push $REMOTE $BRANCH"
    echo "  git push $REMOTE refs/tags/$TAG"
  else
    echo "Next step:"
    echo "  git push $REMOTE refs/tags/$TAG"
  fi
fi
