#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
RENDER_SCRIPT="$ROOT_DIR/scripts/render-release-notes.sh"
DEFAULT_CHANGELOG_PATH="$ROOT_DIR/CHANGELOG.md"

die() {
  echo "Error: $*" >&2
  exit 1
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

main() {
  local changelog_path="$DEFAULT_CHANGELOG_PATH"

  while (($# > 0)); do
    case "$1" in
      --changelog)
        (($# >= 2)) || die "--changelog requires a value"
        changelog_path="$2"
        shift 2
        ;;
      *)
        break
        ;;
    esac
  done

  (($# == 1)) || die "usage: $0 [--changelog PATH] <tag>"
  [[ -x "$RENDER_SCRIPT" ]] || die "missing render script: $RENDER_SCRIPT"

  local tag
  tag="$(normalize_tag "$1")"

  local notes_file=""
  notes_file="$(mktemp "${TMPDIR:-/tmp}/crosshook-release-notes.XXXXXX")"
  trap 'rm -f "${notes_file:-}"' EXIT

  "$RENDER_SCRIPT" --changelog "$changelog_path" "$tag" > "$notes_file"

  grep -Eq "^## \\[$tag\\]" "$notes_file" || die "release notes do not start with the expected heading for $tag"

  local heading_count
  heading_count="$(grep -c '^## \[' "$notes_file")"
  [[ "$heading_count" == "1" ]] || die "release notes for $tag contain multiple release headings"

  grep -q '^- ' "$notes_file" || die "release notes for $tag contain no bullet entries"

  while IFS= read -r heading; do
    case "$heading" in
      "Bug Fixes"|"Features"|"Documentation"|"Refactoring"|"Performance"|"Tests"|"Build"|"CI"|"Reverts"|"Release"|"Security")
        ;;
      *)
        die "release notes for $tag contain a disallowed section heading: $heading"
        ;;
    esac
  done < <(grep '^### ' "$notes_file" | sed 's/^### //')

  local forbidden_patterns=(
    'Update README(\.md)?'
    'release notes'
    'launcher delete plans'
    'line count check'
  )

  local pattern
  for pattern in "${forbidden_patterns[@]}"; do
    if grep -Eiq "$pattern" "$notes_file"; then
      die "release notes for $tag contain a forbidden pattern: $pattern"
    fi
  done
}

main "$@"
