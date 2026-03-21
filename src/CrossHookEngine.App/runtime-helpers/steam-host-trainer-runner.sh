#!/usr/bin/env bash
set -euo pipefail

compatdata=""
proton=""
steam_client=""
trainer_path=""
trainer_host_path=""
log_file=""
staged_trainer_host_path=""
staged_trainer_windows_path=""

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

stage_trainer_into_compatdata() {
  local trainer_file_name staged_trainer_directory_path

  [[ -n "$trainer_host_path" ]] || fail "Missing trainer host path."
  [[ -f "$trainer_host_path" ]] || fail "Trainer host path does not exist as a file: $trainer_host_path"

  trainer_file_name="$(basename "$trainer_host_path")"
  staged_trainer_directory_path="$compatdata/pfx/drive_c/CrossHook/StagedTrainers"
  staged_trainer_host_path="$staged_trainer_directory_path/$trainer_file_name"
  staged_trainer_windows_path="C:\\CrossHook\\StagedTrainers\\$trainer_file_name"

  mkdir -p "$staged_trainer_directory_path"
  cp -f "$trainer_host_path" "$staged_trainer_host_path"

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
    --log-file)
      log_file="${2:-}"
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

compatdata="$(realpath "$compatdata")"
proton="$(realpath "$proton")"
steam_client="$(realpath "$steam_client")"
trainer_host_path="$(realpath "$trainer_host_path")"

[[ -d "$compatdata" ]] || fail "Compatdata path does not exist: $compatdata"
[[ -x "$proton" ]] || fail "Proton path is not executable: $proton"
[[ -f "$trainer_host_path" ]] || fail "Trainer host path does not exist: $trainer_host_path"

for fd in /proc/self/fd/*; do
  fd_num="$(basename "$fd")"
  if ((fd_num > 2)); then
    eval "exec ${fd_num}>&-" 2>/dev/null || true
  fi
done

unset WINESERVER WINELOADER WINEDLLPATH WINEDLLOVERRIDES WINEDEBUG
unset WINEESYNC WINEFSYNC WINELOADERNOEXEC WINEPREFIX
unset WINE_LARGE_ADDRESS_AWARE WINE_DISABLE_KERNEL_WRITEWATCH
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
log_runtime_context
log "Launching trainer with direct proton run."
if "$proton" run "$trainer_path"; then
  log "Trainer proton run exited successfully."
  exit 0
fi

exit_code=$?
log "Trainer proton run exited with code $exit_code"
exit "$exit_code"
