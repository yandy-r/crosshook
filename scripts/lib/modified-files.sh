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
#   is_well_known_excluded_repo_path <repo-relative-path>
#   list_repo_files_by_scope <scope>
#   list_repo_files
#   list_repo_paths          <prefix> <suffix>...
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

is_well_known_excluded_repo_path() {
  local path="${1#./}"

  case "$path" in
    .git|.git/*|*/.git/*) return 0 ;;
    node_modules|node_modules/*|*/node_modules/*) return 0 ;;
    .direnv|.direnv/*|*/.direnv/*) return 0 ;;
    .venv|.venv/*|*/.venv/*) return 0 ;;
    venv|venv/*|*/venv/*) return 0 ;;
    .cache|.cache/*|*/.cache/*) return 0 ;;
    tmp|tmp/*|*/tmp/*) return 0 ;;
    temp|temp/*|*/temp/*) return 0 ;;
    .flatpak-builder|.flatpak-builder/*) return 0 ;;
    .playwright|.playwright/*) return 0 ;;
    .playwright-mcp|.playwright-mcp/*) return 0 ;;
    dist|dist/*) return 0 ;;
    release|release/*) return 0 ;;
    target|target/*) return 0 ;;
    coverage|coverage/*) return 0 ;;
    src/crosshook-native/dist|src/crosshook-native/dist/*) return 0 ;;
    src/crosshook-native/coverage|src/crosshook-native/coverage/*) return 0 ;;
    src/crosshook-native/target|src/crosshook-native/target/*) return 0 ;;
    src/crosshook-native/src-tauri/gen|src/crosshook-native/src-tauri/gen/*) return 0 ;;
    src/crosshook-native/src-tauri/target|src/crosshook-native/src-tauri/target/*) return 0 ;;
    *)
      return 1
      ;;
  esac
}

_filter_well_known_excluded_repo_paths() {
  local path
  while IFS= read -r path; do
    [[ -n "$path" ]] || continue
    is_well_known_excluded_repo_path "$path" && continue
    printf '%s\n' "$path"
  done
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
  ) | _filter_well_known_excluded_repo_paths
}

list_repo_files() {
  (
    cd "$ROOT_DIR" || return
    {
      git ls-files
      git ls-files --others --exclude-standard
    } | awk 'NF && !seen[$0]++'
  ) | _filter_well_known_excluded_repo_paths
}

list_repo_paths() {
  local prefix="$1"
  shift

  local -a files=()
  mapfile -t files < <(list_repo_files)

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
