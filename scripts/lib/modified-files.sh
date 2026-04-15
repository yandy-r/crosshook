#!/usr/bin/env bash

list_modified_repo_files() {
  (
    cd "$ROOT_DIR" || return
    {
      git diff --name-only --diff-filter=ACMR
      git diff --cached --name-only --diff-filter=ACMR
      git ls-files --others --exclude-standard
    } | awk 'NF && !seen[$0]++'
  )
}

filter_modified_repo_paths() {
  local -n out_ref="$1"
  local prefix="$2"
  shift 2

  out_ref=()

  local path suffix
  for path in "${MODIFIED_REPO_FILES[@]}"; do
    [[ -n "$prefix" && "$path" != "$prefix"* ]] && continue

    for suffix in "$@"; do
      if [[ "$path" == *"$suffix" ]]; then
        out_ref+=("$ROOT_DIR/$path")
        break
      fi
    done
  done
}
