#!/usr/bin/env bash
set -euo pipefail

appid=""
compatdata=""
proton=""
steam_client=""
game_exe_name=""
trainer_path=""
trainer_host_path=""
log_file=""
game_startup_delay_seconds="30"
game_timeout_seconds="90"
trainer_timeout_seconds="10"
trainer_only="0"
game_only="0"
staged_trainer_host_path=""
staged_trainer_windows_path=""

log() {
  printf '[steam-helper] %s\n' "$*"
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
  log "game_exe_name=$game_exe_name"
  log_shell_process
  log_staged_trainer_status
}

fail() {
  log "$*" >&2
  exit 1
}

while (($# > 0)); do
  case "$1" in
    --appid)
      appid="${2:-}"
      shift 2
      ;;
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
    --game-exe-name)
      game_exe_name="${2:-}"
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
    --log-file)
      log_file="${2:-}"
      shift 2
      ;;
    --game-startup-delay-seconds)
      game_startup_delay_seconds="${2:-30}"
      shift 2
      ;;
    --game-timeout-seconds)
      game_timeout_seconds="${2:-90}"
      shift 2
      ;;
    --trainer-timeout-seconds)
      trainer_timeout_seconds="${2:-10}"
      shift 2
      ;;
    --trainer-only)
      trainer_only="1"
      shift 1
      ;;
    --game-only)
      game_only="1"
      shift 1
      ;;
    *)
      fail "Unknown argument: $1"
      ;;
  esac
done

[[ -n "$appid" ]] || fail "Missing Steam App ID."
[[ -n "$compatdata" ]] || fail "Missing compatdata path."
[[ -n "$proton" ]] || fail "Missing Proton path."
[[ -n "$steam_client" ]] || fail "Missing Steam client install path."
[[ -n "$game_exe_name" ]] || fail "Missing game executable name."
[[ -n "$log_file" ]] || fail "Missing helper log path."
[[ -d "$compatdata" ]] || fail "Compatdata path does not exist: $compatdata"
[[ -x "$proton" ]] || fail "Proton path is not executable: $proton"

if [[ "$game_only" != "1" ]]; then
  [[ -n "$trainer_path" ]] || fail "Missing trainer path."
  [[ -n "$trainer_host_path" ]] || fail "Missing trainer host path."
  [[ -f "$trainer_host_path" ]] || fail "Trainer host path does not exist: $trainer_host_path"
fi

mkdir -p "$(dirname "$log_file")"
exec >>"$log_file" 2>&1

compatdata="$(realpath "$compatdata")"
proton="$(realpath "$proton")"
steam_client="$(realpath "$steam_client")"

if [[ "$game_only" != "1" ]]; then
  trainer_host_path="$(realpath "$trainer_host_path")"
fi

if command -v steam >/dev/null 2>&1; then
  steam_command="steam"
elif [[ -x "$steam_client/steam.sh" ]]; then
  steam_command="$steam_client/steam.sh"
else
  fail "Could not find a Steam CLI launch command."
fi

export STEAM_COMPAT_DATA_PATH="$compatdata"
export STEAM_COMPAT_CLIENT_INSTALL_PATH="$steam_client"
export WINEPREFIX="$compatdata/pfx"

log_runtime_context

linux_process_visible() {
  local process_name="$1"
  local process_name_without_extension="${process_name%.exe}"

  if pgrep -x -- "$process_name" >/dev/null 2>&1; then
    return 0
  fi

  if [[ "$process_name_without_extension" != "$process_name" ]] && pgrep -x -- "$process_name_without_extension" >/dev/null 2>&1; then
    return 0
  fi

  return 1
}

process_visible() {
  local process_name="$1"
  linux_process_visible "$process_name"
}

wait_for_process() {
  local process_name="$1"
  local timeout_seconds="$2"
  local elapsed=0

  while ((elapsed < timeout_seconds)); do
    if process_visible "$process_name"; then
      log "Detected process: $process_name after ${elapsed}s"
      return 0
    fi

    if ((elapsed > 0)) && ((elapsed % 5 == 0)); then
      log "Still waiting for $process_name (${elapsed}s elapsed)"
    fi

    sleep 1
    elapsed=$((elapsed + 1))
  done

  return 1
}

wait_for_startup_delay() {
  local startup_delay_seconds="$1"
  local elapsed=0

  while ((elapsed < startup_delay_seconds)); do
    if ((elapsed > 0)) && ((elapsed % 5 == 0)); then
      log "Steam launch warm-up in progress (${elapsed}s elapsed)"
    fi

    sleep 1
    elapsed=$((elapsed + 1))
  done
}

run_proton_with_clean_env() {
  local target_path="$1"

  # Close all file descriptors inherited from CrossHook's wineserver.
  local fd_num
  for fd in /proc/self/fd/*; do
    fd_num="$(basename "$fd")"
    if ((fd_num > 2)); then
      eval "exec ${fd_num}>&-" 2>/dev/null || true
    fi
  done

  # Strip WINE/Proton-specific variables inherited from CrossHook's
  # WINE session so Proton can rebuild its own session state cleanly.
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

  stage_trainer_into_compatdata
  log "Launching trainer with direct proton run."
  if setsid "$proton" run "$target_path"; then
    log "Trainer proton run exited successfully."
    return 0
  fi

  local exit_code=$?
  log "Trainer proton run exited with code $exit_code"
  return "$exit_code"
}

trainer_exe_name="${trainer_path##*\\}"

if [[ "$trainer_only" != "1" ]]; then
if process_visible "$game_exe_name"; then
  log "Game process already visible in compatdata for $game_exe_name"
else
  log "Launching Steam AppID $appid"
  "$steam_command" -applaunch "$appid" >/dev/null 2>&1 &

  log "Allowing ${game_startup_delay_seconds}s for Steam startup before trainer launch"
  wait_for_startup_delay "$game_startup_delay_seconds"

  if process_visible "$game_exe_name"; then
    log "Detected process: $game_exe_name after startup delay"
  else
    if [[ "$game_only" == "1" ]]; then
      log "Game process $game_exe_name was not confirmed after startup delay; continuing with game-only exit."
    else
      log "Game process $game_exe_name was not confirmed after startup delay; continuing with trainer launch."
    fi
  fi
fi
else
  log "Trainer-only mode requested; skipping Steam game launch."
fi

if [[ "$game_only" == "1" ]]; then
  log "Game-only mode requested; skipping trainer launch."
  exit 0
fi

if process_visible "$trainer_exe_name"; then
  log "Trainer already running: $trainer_exe_name"
  exit 0
fi

log "Launching trainer $trainer_exe_name in compatdata $compatdata"
log "Executing trainer with a clean Proton environment."
run_proton_with_clean_env "$trainer_path"
