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

list_modified_repo_paths() {
  local prefix="$1"
  shift

  local -a modified_repo_files=()
  mapfile -t modified_repo_files < <(list_modified_repo_files)

  local path suffix
  for path in "${modified_repo_files[@]}"; do
    [[ -n "$prefix" && "$path" != "$prefix"* ]] && continue

    for suffix in "$@"; do
      if [[ "$path" == *"$suffix" ]]; then
        printf '%s\n' "$ROOT_DIR/$path"
        break
      fi
    done
  done
}
