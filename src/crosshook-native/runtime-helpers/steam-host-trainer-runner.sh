#!/usr/bin/env bash
set -euo pipefail

compatdata=""
proton=""
steam_client=""
trainer_path=""
trainer_host_path=""
trainer_loading_mode="source_directory"
log_file=""
staged_trainer_host_path=""
staged_trainer_windows_path=""
gamescope_enabled="0"
gamescope_args=()
steam_app_id=""

log() {
  printf '[steam-trainer-runner] %s\n' "$*"
}

fail() {
  log "$*" >&2
  exit 1
}

ensure_standard_path() {
  local current_path="${PATH:-}"
  if [[ -z "$current_path" ]]; then
    export PATH="/usr/bin:/bin"
    return
  fi

  case ":$current_path:" in
    *:/usr/bin:*|*:/bin:*)
      ;;
    *)
      export PATH="/usr/bin:/bin:$current_path"
      ;;
  esac
}

run_host() {
  if [[ -n "${FLATPAK_ID:-}" ]]; then
    flatpak-spawn --host "$@"
  else
    "$@"
  fi
}

run_host_in_directory() {
  local directory="$1"
  shift

  if [[ -n "${FLATPAK_ID:-}" ]]; then
    flatpak-spawn --host --directory="$directory" "$@"
  else
    (
      cd "$directory"
      "$@"
    )
  fi
}

host_test() {
  local flag="$1"
  local path="$2"

  if [[ -n "${FLATPAK_ID:-}" ]]; then
    run_host test "$flag" "$path"
  else
    test "$flag" "$path"
  fi
}

host_realpath() {
  local path="$1"

  if [[ -n "${FLATPAK_ID:-}" ]]; then
    run_host realpath "$path"
  else
    realpath "$path"
  fi
}

log_shell_process() {
  if [[ -x /usr/bin/ps ]]; then
    log "shell_process=$(/usr/bin/ps -o pid=,ppid=,comm=,args= -p $$)"
  elif command -v ps >/dev/null 2>&1; then
    log "shell_process=$(ps -o pid=,ppid=,comm=,args= -p $$)"
  else
    log "shell_process=unavailable (ps not found)"
  fi
}

log_staged_trainer_status() {
  local trainer_size_bytes
  if [[ -n "$staged_trainer_host_path" && -f "$staged_trainer_host_path" ]]; then
    trainer_size_bytes="$(wc -c <"$staged_trainer_host_path" 2>/dev/null || printf 'unknown')"
    log "staged_trainer_host_path=$staged_trainer_host_path"
    log "staged_trainer_size_bytes=$trainer_size_bytes"
  else
    log "staged_trainer_host_path_missing=${staged_trainer_host_path:-unset}"
  fi
}

copy_support_directory_if_present() {
  local source_dir="$1"
  local target_dir="$2"
  local child_name="$3"

  if [[ -d "$source_dir/$child_name" ]]; then
    mkdir -p "$target_dir"
    cp -R "$source_dir/$child_name" "$target_dir/"
    log "Staged trainer support directory: $child_name"
  fi
}

stage_trainer_support_files() {
  local trainer_source_dir="$1"
  local staged_target_dir="$2"
  local trainer_file_name="$3"
  local trainer_base_name="$4"
  local sibling_file
  local sibling_name

  shopt -s nullglob

  for sibling_file in "$trainer_source_dir"/*; do
    sibling_name="$(basename "$sibling_file")"

    if [[ "$sibling_name" == "$trainer_file_name" ]]; then
      continue
    fi

    if [[ -f "$sibling_file" ]]; then
      case "$sibling_name" in
        "$trainer_base_name".*.json|\
        "$trainer_base_name".*.config|\
        "$trainer_base_name".*.ini|\
        "$trainer_base_name".*.dll|\
        "$trainer_base_name".*.bin|\
        "$trainer_base_name".*.dat|\
        "$trainer_base_name".*.pak)
          cp -f "$sibling_file" "$staged_target_dir/"
          log "Staged trainer sidecar file: $sibling_name"
          ;;
        *.dll|*.json|*.config|*.ini|*.pak|*.dat|*.bin)
          cp -f "$sibling_file" "$staged_target_dir/"
          log "Staged shared trainer dependency: $sibling_name"
          ;;
      esac
    fi
  done

  shopt -u nullglob

  for support_dir in assets data lib bin runtimes plugins locales cef resources; do
    copy_support_directory_if_present "$trainer_source_dir" "$staged_target_dir" "$support_dir"
  done
}

stage_trainer_into_compatdata() {
  local trainer_file_name trainer_base_name trainer_source_dir
  local staged_trainer_root_path staged_trainer_directory_path

  [[ -n "$trainer_host_path" ]] || fail "Missing trainer host path."
  [[ -f "$trainer_host_path" ]] || fail "Trainer host path does not exist as a file: $trainer_host_path"

  trainer_file_name="$(basename "$trainer_host_path")"
  trainer_base_name="${trainer_file_name%.*}"
  trainer_source_dir="$(dirname "$trainer_host_path")"
  staged_trainer_root_path="$compatdata/pfx/drive_c/CrossHook/StagedTrainers"
  staged_trainer_directory_path="$staged_trainer_root_path/$trainer_base_name"
  staged_trainer_host_path="$staged_trainer_directory_path/$trainer_file_name"
  staged_trainer_windows_path="C:\\CrossHook\\StagedTrainers\\$trainer_base_name\\$trainer_file_name"

  rm -rf "$staged_trainer_directory_path"
  mkdir -p "$staged_trainer_directory_path"
  cp -f "$trainer_host_path" "$staged_trainer_host_path"
  stage_trainer_support_files "$trainer_source_dir" "$staged_trainer_directory_path" "$trainer_file_name" "$trainer_base_name"

  trainer_path="$staged_trainer_windows_path"
  run_directory="$staged_trainer_directory_path"
  log "Staged Steam trainer to $trainer_path"
}

log_runtime_context() {
  ensure_standard_path
  log "pwd: $(pwd)"
  log "id: $(id)"
  log "PATH=$PATH"
  log "STEAM_COMPAT_DATA_PATH=$STEAM_COMPAT_DATA_PATH"
  log "STEAM_COMPAT_CLIENT_INSTALL_PATH=$STEAM_COMPAT_CLIENT_INSTALL_PATH"
  log "WINEPREFIX=$WINEPREFIX"
  log "proton=$proton"
  log "trainer_path=$trainer_path"
  log "trainer_host_path=$trainer_host_path"
  log "trainer_loading_mode=$trainer_loading_mode"
  log_shell_process
  log_staged_trainer_status
}

while (($# > 0)); do
  case "$1" in
    --compatdata)
      compatdata="${2:-}"
      shift 2
      ;;
    --proton)
      proton="${2:-}"
      shift 2
      ;;
    --steam-client)
      steam_client="${2:-}"
      shift 2
      ;;
    --trainer-path)
      trainer_path="${2:-}"
      shift 2
      ;;
    --trainer-host-path)
      trainer_host_path="${2:-}"
      shift 2
      ;;
    --trainer-loading-mode)
      trainer_loading_mode="${2:-source_directory}"
      shift 2
      ;;
    --log-file)
      log_file="${2:-}"
      shift 2
      ;;
    --gamescope-enabled)
      gamescope_enabled="1"
      shift
      ;;
    --gamescope-arg)
      gamescope_args+=("${2:-}")
      shift 2
      ;;
    --steam-app-id)
      steam_app_id="${2:-}"
      shift 2
      ;;
    *)
      fail "Unknown argument: $1"
      ;;
  esac
done

ensure_standard_path

[[ -n "$compatdata" ]] || fail "Missing compatdata path."
[[ -n "$proton" ]] || fail "Missing Proton path."
[[ -n "$steam_client" ]] || fail "Missing Steam client install path."
[[ -n "$trainer_path" ]] || fail "Missing trainer path."
[[ -n "$trainer_host_path" ]] || fail "Missing trainer host path."
[[ -n "$log_file" ]] || fail "Missing helper log path."

mkdir -p "$(dirname "$log_file")"
exec >>"$log_file" 2>&1

compatdata="$(host_realpath "$compatdata")" || fail "Failed to resolve compatdata path: $compatdata"
proton="$(host_realpath "$proton")" || fail "Failed to resolve Proton path: $proton"
steam_client="$(host_realpath "$steam_client")" || fail "Failed to resolve Steam client path: $steam_client"
trainer_host_path="$(host_realpath "$trainer_host_path")" || fail "Failed to resolve trainer host path: $trainer_host_path"

host_test -d "$compatdata" || fail "Compatdata path does not exist: $compatdata"
host_test -x "$proton" || fail "Proton path is not executable: $proton"
host_test -f "$trainer_host_path" || fail "Trainer host path does not exist: $trainer_host_path"

case "$trainer_loading_mode" in
  source_directory|copy_to_prefix)
    ;;
  *)
    fail "Unknown trainer loading mode: $trainer_loading_mode"
    ;;
esac

for fd in /proc/self/fd/*; do
  fd_num="$(basename "$fd")"
  if ((fd_num > 2)); then
    eval "exec ${fd_num}>&-" 2>/dev/null || true
  fi
done

# Strip WINE/Proton-specific variables inherited from the host's WINE session
# so Proton can rebuild its own session state cleanly.
# Keep in sync with WINE_ENV_VARS_TO_CLEAR in crosshook-core/src/launch/env.rs.
# WINEPREFIX is unset here (inherited from host) and re-exported below for the
# trainer's own prefix — it is listed in REQUIRED_PROTON_VARS in env.rs, not
# WINE_ENV_VARS_TO_CLEAR, because the Rust path sets rather than clears it.
unset WINESERVER WINELOADER WINEDLLPATH WINEDLLOVERRIDES WINEDEBUG
unset WINEESYNC WINEFSYNC WINELOADERNOEXEC WINEPREFIX
unset WINE_LARGE_ADDRESS_AWARE WINE_DISABLE_KERNEL_WRITEWATCH
unset WINE_HEAP_DELAY_FREE WINEFSYNC_SPINCOUNT
unset LD_PRELOAD LD_LIBRARY_PATH
unset GST_PLUGIN_PATH GST_PLUGIN_SYSTEM_PATH GST_PLUGIN_SYSTEM_PATH_1_0
unset SteamGameId SteamAppId GAMEID
unset PROTON_LOG PROTON_DUMP_DEBUG_COMMANDS PROTON_USE_WINED3D
unset PROTON_NO_ESYNC PROTON_NO_FSYNC PROTON_ENABLE_NVAPI
unset DXVK_CONFIG_FILE DXVK_STATE_CACHE_PATH DXVK_LOG_PATH
unset VKD3D_CONFIG VKD3D_DEBUG

export STEAM_COMPAT_DATA_PATH="$compatdata"
export STEAM_COMPAT_CLIENT_INSTALL_PATH="$steam_client"
export WINEPREFIX="$compatdata/pfx"

if [[ "$trainer_loading_mode" == "copy_to_prefix" ]]; then
  stage_trainer_into_compatdata
  trainer_path="$staged_trainer_host_path"
  cd "$run_directory" || fail "Failed to cd to staged trainer directory: $run_directory"
  log "Changed trainer working directory to $(pwd)"
  run_directory="$PWD"
else
  trainer_path="$trainer_host_path"
  run_directory="$(dirname "$trainer_host_path")"
  log "Using trainer from source directory: $trainer_path"
  cd "$run_directory"
  log "Changed trainer working directory to $(pwd)"
  run_directory="$PWD"
fi

log_runtime_context

log "Launching trainer with direct proton run."
if [[ -n "${FLATPAK_ID:-}" ]]; then
  if [[ "$gamescope_enabled" == "1" ]]; then
    if run_host_in_directory "$run_directory" gamescope "${gamescope_args[@]}" -- "$proton" run "$trainer_path"; then
      log "Trainer proton run exited successfully."
      exit 0
    else
      exit_code=$?
      log "Trainer proton run exited with code $exit_code"
      exit "$exit_code"
    fi
  fi
  if run_host_in_directory "$run_directory" "$proton" run "$trainer_path"; then
    log "Trainer proton run exited successfully."
    exit 0
  else
    exit_code=$?
    log "Trainer proton run exited with code $exit_code"
    exit "$exit_code"
  fi
fi
if [[ "$gamescope_enabled" == "1" ]]; then
  launch_command=(gamescope "${gamescope_args[@]}" -- "$proton" run "$trainer_path")
else
  launch_command=("$proton" run "$trainer_path")
fi
if "${launch_command[@]}"; then
  log "Trainer proton run exited successfully."
  exit 0
else
  exit_code=$?
  log "Trainer proton run exited with code $exit_code"
  exit "$exit_code"
fi
