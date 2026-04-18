#!/usr/bin/env bash
# Helpers for listing files by git scope (staged / unstaged / modified).
# Requires $ROOT_DIR to be set by the sourcing script.
#
# Scopes:
#   staged    — `git diff --cached` (ACMR)
#   unstaged  — `git diff` (ACMR) + untracked (`git ls-files --others --exclude-standard`)
#   modified  — staged ∪ unstaged (de-duplicated)
#
# Public API:
#   list_repo_files_by_scope <scope>
#   list_scoped_repo_paths   <scope> <prefix> <suffix>...
#   list_modified_repo_files                                 # back-compat: scope=modified
#   list_modified_repo_paths          <prefix> <suffix>...   # back-compat: scope=modified

_list_staged_files() {
  git diff --cached --name-only --diff-filter=ACMR
}

_list_unstaged_files() {
  {
    git diff --name-only --diff-filter=ACMR
    git ls-files --others --exclude-standard
  } | awk 'NF && !seen[$0]++'
}

list_repo_files_by_scope() {
  local scope="$1"
  (
    cd "$ROOT_DIR" || return
    case "$scope" in
      staged)   _list_staged_files ;;
      unstaged) _list_unstaged_files ;;
      modified)
        {
          _list_staged_files
          _list_unstaged_files
        } | awk 'NF && !seen[$0]++'
        ;;
      *)
        printf 'list_repo_files_by_scope: unknown scope %q (expected staged|unstaged|modified)\n' "$scope" >&2
        return 1
        ;;
    esac
  )
}

list_scoped_repo_paths() {
  local scope="$1"
  local prefix="$2"
  shift 2

  local -a files=()
  mapfile -t files < <(list_repo_files_by_scope "$scope")

  local path suffix
  for path in "${files[@]}"; do
    [[ -n "$prefix" && "$path" != "$prefix"* ]] && continue

    for suffix in "$@"; do
      if [[ "$path" == *"$suffix" ]]; then
        printf '%s\n' "$ROOT_DIR/$path"
        break
      fi
    done
  done
}

list_modified_repo_files() {
  list_repo_files_by_scope modified
}

list_modified_repo_paths() {
  list_scoped_repo_paths modified "$@"
}
