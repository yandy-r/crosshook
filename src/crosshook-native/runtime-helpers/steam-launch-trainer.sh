#!/usr/bin/env bash
set -euo pipefail

compatdata=""
proton=""
steam_client=""
trainer_path=""
trainer_host_path=""
trainer_loading_mode="source_directory"
log_file=""
gamescope_enabled="0"
gamescope_args=()

log() {
  printf '[steam-trainer-launcher] %s\n' "$*"
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

resolve_runner_script() {
  local script_dir
  script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
  printf '%s\n' "$script_dir/steam-host-trainer-runner.sh"
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

case "$trainer_loading_mode" in
  source_directory|copy_to_prefix)
    ;;
  *)
    fail "Unknown trainer loading mode: $trainer_loading_mode"
    ;;
esac

runner_script="$(resolve_runner_script)"
[[ -f "$runner_script" ]] || fail "Host runner script not found: $runner_script"

mkdir -p "$(dirname "$log_file")"
exec >>"$log_file" 2>&1

compatdata="$(realpath "$compatdata")"
proton="$(realpath "$proton")"
steam_client="$(realpath "$steam_client")"
trainer_host_path="$(realpath "$trainer_host_path")"
runner_script="$(realpath "$runner_script")"

[[ -d "$compatdata" ]] || fail "Compatdata path does not exist: $compatdata"
[[ -x "$proton" ]] || fail "Proton path is not executable: $proton"
[[ -f "$trainer_host_path" ]] || fail "Trainer host path does not exist: $trainer_host_path"

log "Launching detached host runner."

runner_pid=""
if runner_pid="$(
  runner_command=(
    /bin/bash "$runner_script"
      --compatdata "$compatdata"
      --proton "$proton"
      --steam-client "$steam_client"
      --trainer-path "$trainer_path"
      --trainer-host-path "$trainer_host_path"
      --trainer-loading-mode "$trainer_loading_mode"
      --log-file "$log_file"
  )
  if [[ "$gamescope_enabled" == "1" ]]; then
    runner_command+=(--gamescope-enabled)
    for arg in "${gamescope_args[@]}"; do
      runner_command+=(--gamescope-arg "$arg")
    done
  fi

  setsid env -i \
    HOME="${HOME:-}" \
    USER="${USER:-}" \
    LOGNAME="${LOGNAME:-}" \
    SHELL="${SHELL:-/bin/bash}" \
    PATH="/usr/bin:/bin" \
    DISPLAY="${DISPLAY:-}" \
    WAYLAND_DISPLAY="${WAYLAND_DISPLAY:-}" \
    XDG_RUNTIME_DIR="${XDG_RUNTIME_DIR:-}" \
    DBUS_SESSION_BUS_ADDRESS="${DBUS_SESSION_BUS_ADDRESS:-}" \
    "${runner_command[@]}" \
      </dev/null >/dev/null 2>&1 &
  printf '%s' "$!"
)"; then
  log "Detached host runner pid=$runner_pid"
  exit 0
fi

fail "Failed to start detached host runner."
