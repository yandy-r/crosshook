#!/usr/bin/env bash
set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
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

extract_release_section() {
  local tag="$1"
  awk -v target_prefix="## [${tag}]" '
    index($0, target_prefix) == 1 {
      in_section = 1
    }

    in_section && /^## \[/ && index($0, target_prefix) != 1 {
      exit
    }

    in_section {
      print
    }

    END {
      if (!in_section) {
        exit 1
      }
    }
  ' "$CHANGELOG_PATH"
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
  [[ -f "$changelog_path" ]] || die "missing changelog: $changelog_path"

  local tag
  tag="$(normalize_tag "$1")"

  CHANGELOG_PATH="$changelog_path" extract_release_section "$tag" || die "release notes for ${tag} were not found in $changelog_path"
}

main "$@"
