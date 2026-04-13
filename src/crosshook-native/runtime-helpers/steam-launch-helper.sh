#!/usr/bin/env bash
set -euo pipefail

appid=""
compatdata=""
proton=""
steam_client=""
game_exe_name=""
trainer_path=""
trainer_host_path=""
trainer_loading_mode="source_directory"
log_file=""
game_startup_delay_seconds="30"
game_timeout_seconds="90"
trainer_timeout_seconds="10"
trainer_only="0"
game_only="0"
configured_working_directory=""
gamescope_enabled="0"
gamescope_allow_nested="0"
gamescope_args=()

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

# Host-only executables (steam, pgrep) must run on the host when CrossHook is sandboxed.
run_host() {
  if [[ -n "${FLATPAK_ID:-}" ]]; then
    flatpak-spawn --host "$@"
  else
    "$@"
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

resolve_runner_script() {
  local script_dir
  script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
  printf '%s\n' "$script_dir/steam-host-trainer-runner.sh"
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
  log "game_exe_name=$game_exe_name"
  log_shell_process
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
    --trainer-loading-mode)
      trainer_loading_mode="${2:-source_directory}"
      shift 2
      ;;
    --working-directory|--directory)
      configured_working_directory="${2:-}"
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
    --gamescope-enabled)
      gamescope_enabled="1"
      shift 1
      ;;
    --gamescope-allow-nested)
      gamescope_allow_nested="1"
      shift 1
      ;;
    --gamescope-arg)
      gamescope_args+=("${2:-}")
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
host_test -d "$compatdata" || fail "Compatdata path does not exist: $compatdata"
host_test -x "$proton" || fail "Proton path is not executable: $proton"

if [[ "$game_only" != "1" ]]; then
  [[ -n "$trainer_path" ]] || fail "Missing trainer path."
  [[ -n "$trainer_host_path" ]] || fail "Missing trainer host path."
  host_test -f "$trainer_host_path" || fail "Trainer host path does not exist: $trainer_host_path"
fi

mkdir -p "$(dirname "$log_file")"
exec >>"$log_file" 2>&1

compatdata="$(host_realpath "$compatdata")" || fail "Failed to resolve compatdata path: $compatdata"
proton="$(host_realpath "$proton")" || fail "Failed to resolve Proton path: $proton"
steam_client="$(host_realpath "$steam_client")" || fail "Failed to resolve Steam client path: $steam_client"

if [[ "$game_only" != "1" ]]; then
  trainer_host_path="$(host_realpath "$trainer_host_path")" || fail "Failed to resolve trainer host path: $trainer_host_path"
fi

case "$trainer_loading_mode" in
  source_directory|copy_to_prefix)
    ;;
  *)
    fail "Unknown trainer loading mode: $trainer_loading_mode"
    ;;
esac

steam_command=""
if [[ -n "${FLATPAK_ID:-}" ]]; then
  if run_host sh -c 'command -v steam' >/dev/null 2>&1; then
    steam_command="steam"
  elif run_host test -x "$steam_client/steam.sh"; then
    steam_command="$steam_client/steam.sh"
  fi
else
  if command -v steam >/dev/null 2>&1; then
    steam_command="steam"
  elif [[ -x "$steam_client/steam.sh" ]]; then
    steam_command="$steam_client/steam.sh"
  fi
fi
[[ -n "$steam_command" ]] || fail "Could not find a Steam CLI launch command."

export STEAM_COMPAT_DATA_PATH="$compatdata"
export STEAM_COMPAT_CLIENT_INSTALL_PATH="$steam_client"
export WINEPREFIX="$compatdata/pfx"

log_runtime_context

linux_process_visible() {
  local process_name="$1"
  local process_name_without_extension="${process_name%.exe}"

  if [[ -n "${FLATPAK_ID:-}" ]]; then
    if run_host pgrep -x -- "$process_name" >/dev/null 2>&1; then
      return 0
    fi
    if [[ "$process_name_without_extension" != "$process_name" ]] && run_host pgrep -x -- "$process_name_without_extension" >/dev/null 2>&1; then
      return 0
    fi
  else
    if pgrep -x -- "$process_name" >/dev/null 2>&1; then
      return 0
    fi
    if [[ "$process_name_without_extension" != "$process_name" ]] && pgrep -x -- "$process_name_without_extension" >/dev/null 2>&1; then
      return 0
    fi
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

trainer_exe_name=""
if [[ "$game_only" != "1" ]]; then
  trainer_exe_name="$(basename "$trainer_host_path")"
fi

if [[ "$trainer_only" != "1" ]]; then
if process_visible "$game_exe_name"; then
  log "Game process already visible in compatdata for $game_exe_name"
else
  log "Launching Steam AppID $appid"
  if [[ -n "${FLATPAK_ID:-}" ]]; then
    run_host "$steam_command" -applaunch "$appid" >/dev/null 2>&1 &
  else
    "$steam_command" -applaunch "$appid" >/dev/null 2>&1 &
  fi

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
runner_script="$(resolve_runner_script)"
[[ -f "$runner_script" ]] || fail "Host runner script not found: $runner_script"
effective_trainer_working_directory=""
if [[ -n "$configured_working_directory" ]]; then
  effective_trainer_working_directory="$configured_working_directory"
else
  effective_trainer_working_directory="$(dirname "$trainer_host_path")"
fi
runner_command=(
  /bin/bash "$runner_script"
  --compatdata "$compatdata"
  --proton "$proton"
  --steam-client "$steam_client"
  --steam-app-id "$appid"
  --trainer-path "$trainer_path"
  --trainer-host-path "$trainer_host_path"
  --trainer-loading-mode "$trainer_loading_mode"
  --working-directory "$effective_trainer_working_directory"
  --log-file "$log_file"
)
if [[ "$gamescope_enabled" == "1" ]]; then
  runner_command+=(--gamescope-enabled)
  if [[ "$gamescope_allow_nested" == "1" ]]; then
    runner_command+=(--gamescope-allow-nested)
  fi
  for arg in "${gamescope_args[@]}"; do
    runner_command+=(--gamescope-arg "$arg")
  done
fi
log "Delegating trainer leg to steam-host-trainer-runner.sh"
"${runner_command[@]}"
