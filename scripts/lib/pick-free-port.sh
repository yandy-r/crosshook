#!/usr/bin/env bash
# Pick the first free TCP port on loopback starting from a preferred port.
#
# Usage:
#   pick_free_port [start_port] [denylist_csv]
#
# Denylist entries are skipped even when free. Port 5173 is always denied
# (reserved for browser dev mode). Scans up to 100 consecutive ports.
# Prints the chosen port to stdout; exits 1 if none found.

pick_free_port() {
  local start_port="${1:-1420}"
  local denylist_csv="${2:-}"
  local max_attempts=100
  local port candidate
  local -a denylist=(5173)

  if [[ ! "$start_port" =~ ^[0-9]+$ ]] || (( start_port < 1 || start_port > 65535 )); then
    echo "pick_free_port: invalid start port: $start_port" >&2
    return 1
  fi

  if [[ -n "$denylist_csv" ]]; then
    IFS=',' read -r -a extra_denied <<<"$denylist_csv"
    for candidate in "${extra_denied[@]}"; do
      candidate="${candidate// /}"
      [[ -n "$candidate" ]] && denylist+=("$candidate")
    done
  fi

  _pick_free_port_is_denied() {
    local check_port="$1"
    local denied
    for denied in "${denylist[@]}"; do
      if [[ "$check_port" == "$denied" ]]; then
        return 0
      fi
    done
    return 1
  }

  _pick_free_port_in_use() {
    local check_port="$1"
    if command -v ss >/dev/null 2>&1; then
      ss -ltn "sport = :$check_port" 2>/dev/null | grep -q ":$check_port"
      return $?
    fi
    # bash /dev/tcp probe (connect succeeds when something is listening)
    (echo >/dev/tcp/127.0.0.1/"$check_port") 2>/dev/null
  }

  for ((i = 0; i < max_attempts; i++)); do
    port=$((start_port + i))
    if (( port > 65535 )); then
      break
    fi
    if _pick_free_port_is_denied "$port"; then
      continue
    fi
    if ! _pick_free_port_in_use "$port"; then
      echo "$port"
      return 0
    fi
  done

  echo "pick_free_port: no free port found in range $start_port-$((start_port + max_attempts - 1)) (denylist: ${denylist[*]})" >&2
  return 1
}
