# External Tool Archaeological Report

Deep-dive analysis of how CrossHook detects, invokes, and handles every external tool dependency. Each tool is documented across six dimensions: detection method, invocation method, fallback behavior, error handling, configuration options, and criticality classification.

## Core Abstraction Layer

All external tool invocations route through `crates/crosshook-core/src/platform.rs`, which provides the Flatpak sandbox abstraction:

- **`host_command()`** / **`host_std_command()`**: Wraps all host tool invocations with `flatpak-spawn --host` when `is_flatpak()` is true; uses `Command::new()` natively.
- **`host_command_with_env()`** / **`host_command_with_env_and_directory()`**: Threads environment variables via `--env=K=V` flatpak-spawn args (since `.env()` on Command is silently dropped by flatpak-spawn). Uses `--clear-env` for clean environments.
- **Custom env file handoff**: For user-defined env vars that may contain sensitive values, writes a temporary `0600`-permission env file sourced by `bash -c 'source /tmp/file && exec ...'` to avoid exposing values on process argv.
- **`host_command_exists()`** (line 526): Under Flatpak runs `which <binary>` via `host_std_command`; natively scans `PATH` directly.
- **`host_path_is_dir()`** / **`host_path_is_file()`** / **`host_read_dir_names()`**: Probe host filesystem via `flatpak-spawn --host test/ls/cat`.
- **`normalize_flatpak_host_path()`**: Strips `/run/host` prefix from paths; handles document portal xattr resolution.
- **`override_xdg_for_flatpak_host_access()`**: Called at app startup (`src-tauri/src/lib.rs`) to redirect sandbox XDG paths to host paths.

---

## Tool-by-Tool Analysis

### 1. Proton / Wine

- **Detection**: `crates/crosshook-core/src/steam/proton.rs` — `discover_compat_tools_with_roots()` scans Steam library roots (`<steam_root>/compatibilitytools.d/`) and system compat-tool roots (`/usr/share/steam/compatibilitytools.d`, `/usr/local/share/steam/compatibilitytools.d`, and variants without `.d`). Parses `compatibilitytool.vdf` files for tool metadata. Under Flatpak, all filesystem probing uses `platform::host_path_is_dir()`, `host_read_dir_names()`, `host_path_is_file()`, `host_read_file_bytes_if_system_path()`.
- **Invocation**: `crates/crosshook-core/src/launch/script_runner.rs` — `build_proton_game_command()` (line 408) and `build_proton_trainer_command()` (line 529) build the full Proton launch command. Proton binary is executed via `host_command_with_env_and_directory()` with a carefully constructed env map (from `host_environment_map()` in `runtime_helpers.rs` line 381) including `STEAM_COMPAT_DATA_PATH`, `STEAM_COMPAT_CLIENT_INSTALL_PATH`, `WINEPREFIX`, etc.
- **Fallback**: `prefer_user_local_compat_tool_path()` redirects system compat-tool paths to user-local installs when under Flatpak. If no Proton is discovered, the onboarding readiness check (`onboarding/readiness.rs`) reports "Proton not available" with install guidance.
- **Error handling**: Returns `Result` with descriptive errors for VDF parse failures, missing directories, and inaccessible paths. Onboarding surfaces missing Proton as a readiness warning, not a hard block.
- **Configuration**: `settings/mod.rs` — `AppSettingsData::default_proton_path` allows user override of the default Proton version. `umu_preference` (Auto/Umu/Proton) controls whether umu-run or direct Proton is preferred.
- **Criticality**: **Critical** — CrossHook cannot launch Windows games without Proton/Wine. Core functionality.

### 2. umu-run (umu-launcher)

- **Detection**: `crates/crosshook-core/src/launch/runtime_helpers.rs` — `resolve_umu_run_path()` (line 715) implements multi-strategy discovery:
  1. Read `/run/host/env/PATH` (Flatpak) or use `$PATH` (native) to scan for `umu-run` binary.
  2. Under Flatpak, `probe_flatpak_host_umu_candidates()` checks ~20 hardcoded candidate paths (`/usr/bin/umu-run`, `/usr/local/bin/umu-run`, `~/.local/bin/umu-run`, distro-specific paths like `/app/bin/umu-run`, etc.).
  3. Onboarding check in `onboarding/readiness.rs` calls `resolve_umu_run_path()` and reports availability.
- **Invocation**: Used as a wrapper command in Proton launch chains. `build_proton_game_command()` and `build_proton_trainer_command()` in `script_runner.rs` prepend `umu-run` to the Proton command when the umu preference is active. Executed via `host_command_with_env_and_directory()`.
- **Fallback**: If umu-run is not found, falls back to direct Proton execution. The `umu_preference` setting controls this: `Auto` tries umu first then falls back, `Proton` skips umu entirely. Onboarding provides distro-specific install advice via `build_umu_install_advice()`.
- **Error handling**: `resolve_umu_run_path()` returns `Option<PathBuf>` — `None` means not found, no panic. Launch code gracefully degrades to direct Proton.
- **Configuration**: `settings/mod.rs` — `umu_preference` enum: `Auto` (default), `Umu` (force), `Proton` (skip).
- **Criticality**: **High** — Preferred launch method for non-Steam games; direct Proton fallback exists but umu provides better compatibility management.

### 3. gamescope

- **Detection**: `crates/crosshook-core/src/launch/optimizations.rs` — `is_command_available()` (line 254) checks for `gamescope` binary. Under Flatpak uses `platform::host_command_exists()`, native scans PATH. Also checked via the optimization catalog TOML entries where `required_binary = "gamescope"`.
- **Invocation**: `crates/crosshook-core/src/launch/runtime_helpers.rs` — `build_gamescope_command()` constructs the gamescope wrapper command with resolution, refresh rate, and backend args. Under Flatpak, uses a special bash script for PID capture (`build_proton_command_with_gamescope_pid_capture_in_directory_inner()`, line 271) that captures the host PID for the watchdog to signal.
- **Fallback**: `should_skip_gamescope()` (script_runner.rs line 89) skips if `GAMESCOPE_WAYLAND_DISPLAY` is already set and `allow_nested` is false (avoids nested gamescope). If gamescope binary is missing, the optimization catalog marks it as `ValidationError::LaunchOptimizationDependencyMissing` and the optimization is skipped.
- **Error handling**: Missing gamescope surfaces as a validation error in the optimization resolution chain. Launch proceeds without gamescope wrapping. Gamescope crash during runtime is handled by the watchdog fallback.
- **Configuration**: Optimization catalog TOML entries control gamescope args (resolution, refresh rate, backend). Per-game optimization profiles can enable/disable gamescope.
- **Criticality**: **Medium** — Performance optimization layer. Games launch and run without it; Steam Deck users benefit most.

### 4. mangohud / mangoapp

- **Detection**: `is_command_available()` in `optimizations.rs` checks for `mangohud` binary via PATH/`host_command_exists()`. Catalog entries specify `required_binary = "mangohud"`.
- **Invocation**: When gamescope is NOT wrapping the process, `mangohud` is prepended as a command wrapper. When gamescope IS wrapping the process, mangohud is replaced with `--mangoapp` as a gamescope arg (`build_steam_launch_options_command()` line 194 in `optimizations.rs`). This swap is critical — running both mangohud wrapper + gamescope causes conflicts.
- **Fallback**: If mangohud is missing, the optimization is skipped via `LaunchOptimizationDependencyMissing`. Game launches without HUD overlay.
- **Error handling**: Validation error surfaced to UI; non-blocking.
- **Configuration**: Optimization catalog TOML. Per-game toggle.
- **Criticality**: **Low** — Cosmetic/diagnostic overlay. No functional impact on game execution.

### 5. unshare (network isolation)

- **Detection**: `crates/crosshook-core/src/launch/runtime_helpers.rs` — `is_unshare_net_available()` (line 816) probes by running `unshare --net true` and caching the result in a `OnceLock<bool>` for the process lifetime.
- **Invocation**: `build_flatpak_unshare_bash_command()` (script_runner.rs line 93) wraps the trainer launch command with `host_command_with_env("unshare", &["--net", ...])` under Flatpak. Native uses `Command::new("unshare")`.
- **Fallback**: If `unshare --net` probe fails (returns false), network isolation is silently skipped. Trainer launches without network isolation. The `OnceLock` cache means the probe only runs once per app session.
- **Error handling**: Probe failure is not an error — just disables the feature. No user-facing warning.
- **Configuration**: None currently — automatic detection only.
- **Criticality**: **Medium** — Security feature for trainer isolation. Trainers work without it but may have network access.

### 6. git

- **Detection**: Not explicitly detected ahead of time. Used on-demand for community tap operations.
- **Invocation**: `crates/crosshook-core/src/community/taps.rs` — `git_command()` (line 511): Under Flatpak uses `host_std_command_with_env("git", ...)`, native uses `Command::new("git")`. Operations include `clone`, `fetch`, `pull`, `rev-parse`, `log`.
- **Fallback**: If git clone/fetch fails but a local clone already exists, returns `CachedFallback` status — the app uses the stale local tap data rather than failing entirely. This is a graceful degradation for offline or git-unavailable scenarios.
- **Error handling**: Git operation failures return `Result` errors with stderr context. The `CachedFallback` path logs a warning but continues. Initial clone failure with no cached data returns a hard error to the UI.
- **Configuration**: None — git is expected to be available on the host.
- **Criticality**: **Medium** — Required for community tap sync. App functions without taps but loses community trainer definitions.

### 7. winetricks / protontricks

- **Detection**: `crates/crosshook-core/src/prefix_deps/detection.rs` — `detect_binary()` implements a priority chain:
  1. User-configured path from settings (highest priority).
  2. `winetricks` on PATH.
  3. `protontricks` on PATH.
     Returns `BinaryDetectionResult` with `found: bool`, `path`, `name`, `tool_type` (Winetricks/Protontricks), and `source` (Settings/PathScan).
- **Invocation**: `crates/crosshook-core/src/prefix_deps/runner.rs` — Runs detected binary with `WINEPREFIX` set. `check_installed()` runs `<binary_path> list-installed` to query installed components.
- **Fallback**: If neither winetricks nor protontricks is found, `detect_binary()` returns `found: false`. The prefix dependency feature is disabled in the UI but does not block game launching.
- **Error handling**: `BinaryDetectionResult` cleanly represents missing state. Runner operations return `Result` with command execution errors.
- **Configuration**: `settings/mod.rs` — `protontricks_binary_path` and user-configurable override path. Priority: settings override > winetricks > protontricks.
- **Criticality**: **Low** — Only needed for Wine prefix dependency management. Games launch fine without it.

### 8. protonup-qt / protonup

- **Detection**: Not explicitly probed at runtime. Referenced in settings as a user-configurable path.
- **Invocation**: Managed externally by the user. CrossHook stores the path in settings for potential future integration.
- **Fallback**: N/A — currently a settings placeholder.
- **Error handling**: N/A.
- **Configuration**: `settings/mod.rs` — `protonup_binary_path`.
- **Criticality**: **Low** — External tool reference only. No runtime dependency.

### 9. lspci (GPU detection)

- **Detection**: Not pre-checked. Invoked on demand during diagnostics export.
- **Invocation**: `crates/crosshook-core/src/export/diagnostics.rs` (lines 220-300) — Under Flatpak: `platform::host_std_command("lspci")` with args `["-nn"]`. Native: `Command::new("lspci")`.
- **Fallback**: If lspci execution fails, the diagnostics report includes the string `"(lspci not available: {error})"` instead of GPU information. Diagnostics export continues with partial data.
- **Error handling**: Catches command execution error, formats it into the output string. Non-fatal.
- **Configuration**: None.
- **Criticality**: **Low** — Diagnostic-only. No functional impact on app operation.

### 10. kill / ps / cat / test (watchdog process management)

- **Detection**: Not pre-checked. These are assumed to be universally available POSIX utilities.
- **Invocation**: `crates/crosshook-core/src/launch/watchdog.rs`:
  - `host_std_command("kill")` — sends SIGTERM/SIGKILL to game/trainer processes.
  - `host_std_command("ps")` — scans for process names/PIDs during watchdog monitoring.
  - `host_std_command("cat")` — reads `/proc/<pid>/cmdline` for process identification.
  - `host_std_command("test")` — checks PID liveness via `test -d /proc/<pid>`.
    All route through `platform::host_std_command()` for Flatpak transparency.
- **Fallback**: If any utility fails, watchdog logs the error and continues its monitoring loop. Watchdog is designed to be resilient to transient failures (process already dead, permission denied on kill, etc.).
- **Error handling**: Each invocation checks exit status. Failures are logged but do not crash the watchdog. SIGTERM failure escalates to SIGKILL attempt.
- **Configuration**: None.
- **Criticality**: **High** — Watchdog relies on these for process lifecycle management. Without them, CrossHook cannot reliably monitor or terminate launched processes.

### 11. steam (Steam client)

- **Detection**: `runtime-helpers/steam-launch-helper.sh` — `command -v steam` or falls back to checking for `steam.sh`. Onboarding readiness checks Steam installation status.
- **Invocation**: Shell helper scripts invoke `steam` command for Steam URL protocol launches (`steam://rungameid/...`). Uses `run_host()` wrapper in Flatpak context (`flatpak-spawn --host`).
- **Fallback**: If Steam is not detected, onboarding readiness reports it as missing with install guidance. Steam-profile game launches will fail without it (expected — Steam games need Steam).
- **Error handling**: Shell scripts check command existence before invocation. Missing Steam is surfaced as an onboarding readiness issue.
- **Configuration**: None — Steam is expected at its standard location.
- **Criticality**: **Critical** — Required for Steam-profile game launches. Non-Steam launches (direct Proton) work without it.

### 12. Shell helpers (pgrep, realpath, basename, setsid)

- **Detection**: Not pre-checked. Assumed available as standard POSIX/Linux utilities.
- **Invocation**: Used within `runtime-helpers/*.sh` scripts:
  - `pgrep` — process discovery in `steam-launch-helper.sh`.
  - `realpath` — path canonicalization in shell helpers.
  - `basename` — filename extraction.
  - `setsid` — detached process execution in `steam-launch-trainer.sh` for trainer spawning.
    All use `run_host()` / `run_host_in_directory()` wrappers for Flatpak (`flatpak-spawn --host --clear-env --directory=...`).
- **Fallback**: No explicit fallback. These are expected to be available on any Linux system.
- **Error handling**: Shell scripts use `set -e` or explicit exit-code checks. Failures propagate as script exit codes.
- **Configuration**: None.
- **Criticality**: **High** — Shell helpers depend on these for correct trainer detachment and process management.

### 13. flatpak-spawn (Flatpak-only meta-tool)

- **Detection**: Implicitly available inside any Flatpak sandbox. `is_flatpak()` in `platform.rs` checks for `/.flatpak-info` file existence.
- **Invocation**: Every `host_command()` / `host_std_command()` call wraps the target command with `flatpak-spawn --host [--clear-env] [--env=K=V ...] [--directory=DIR] <cmd> [args...]`. This is the foundational abstraction — all other tools listed above route through it when inside Flatpak.
- **Fallback**: If `is_flatpak()` returns false, `flatpak-spawn` is never used. Native commands execute directly.
- **Error handling**: If `flatpak-spawn` itself fails (e.g., D-Bus portal unavailable), the error propagates from the wrapped command's `Result`. No special recovery.
- **Configuration**: None — automatically engaged based on `is_flatpak()`.
- **Criticality**: **Critical (Flatpak only)** — Without it, no host tool access is possible from within the Flatpak sandbox. It is the single gateway for all host interactions.

---

## Relevant Files

- `crates/crosshook-core/src/platform.rs` — Central Flatpak abstraction layer; `host_command*()`, `host_command_exists()`, `is_flatpak()`, env threading, path normalization
- `crates/crosshook-core/src/launch/runtime_helpers.rs` — Gamescope command building, umu-run resolution, unshare detection, host env map construction
- `crates/crosshook-core/src/launch/script_runner.rs` — Main launch command building for games and trainers (Proton, umu, gamescope, unshare integration)
- `crates/crosshook-core/src/launch/optimizations.rs` — Optimization catalog resolution; `is_command_available()` for gamescope/mangohud/etc.
- `crates/crosshook-core/src/launch/watchdog.rs` — Process lifecycle management via kill/ps/cat/test
- `crates/crosshook-core/src/steam/proton.rs` — Proton discovery, VDF parsing, compat-tool root scanning
- `crates/crosshook-core/src/settings/mod.rs` — User-configurable tool paths (protontricks, protonup, default Proton, umu preference)
- `crates/crosshook-core/src/prefix_deps/detection.rs` — Winetricks/protontricks binary detection with priority chain
- `crates/crosshook-core/src/prefix_deps/runner.rs` — Winetricks/protontricks execution with WINEPREFIX
- `crates/crosshook-core/src/community/taps.rs` — Git-based community tap management with cached fallback
- `crates/crosshook-core/src/onboarding/readiness.rs` — System readiness checks (Steam, Proton, umu-run)
- `crates/crosshook-core/src/export/diagnostics.rs` — lspci GPU detection for diagnostics
- `runtime-helpers/steam-launch-helper.sh` — Shell helper for Steam game+trainer launch (pgrep, steam, realpath, basename)
- `runtime-helpers/steam-host-trainer-runner.sh` — Trainer runner with gamescope fallback and env file handoff
- `runtime-helpers/steam-launch-trainer.sh` — Trainer launcher with setsid for detached execution
- `src-tauri/src/paths.rs` — Script path resolution with Flatpak `/app/resources/` fallback
- `src-tauri/src/lib.rs` — App entry point; calls `override_xdg_for_flatpak_host_access()`

## Architectural Patterns

- **Single abstraction gateway**: All host tool access routes through `platform.rs`. No module bypasses this layer — it is the sole Flatpak/native branch point.
- **Probe-then-cache**: `is_unshare_net_available()` uses `OnceLock` to probe once and cache for the process lifetime. `is_flatpak()` similarly caches its check.
- **Multi-strategy resolution**: umu-run and Proton discovery both use cascading search strategies (PATH scan, known paths, host filesystem probes) rather than a single lookup.
- **Graceful degradation chain**: Most tools follow the pattern: check availability → use if present → skip/degrade if missing → surface in UI. Hard failures are reserved for truly critical dependencies (Proton for game launch, flatpak-spawn in Flatpak).
- **Env file handoff**: Custom user environment variables use temporary file sourcing rather than process argv to avoid leaking sensitive values in `/proc/<pid>/cmdline`.
- **Mangohud/mangoapp swap**: Context-dependent tool substitution — mangohud as a wrapper vs. mangoapp as a gamescope arg, depending on whether gamescope is active.
- **Cached fallback for network-dependent tools**: Git tap operations degrade to stale cached data rather than hard-failing when the network or git is unavailable.
- **Shell script Flatpak parity**: `runtime-helpers/*.sh` scripts mirror the Rust abstraction with their own `run_host()` / `run_host_in_directory()` bash wrappers that prepend `flatpak-spawn --host` when `FLATPAK_ID` is set.

## Criticality Summary

| Tool                           | Criticality               | Fallback                                        |
| ------------------------------ | ------------------------- | ----------------------------------------------- |
| Proton/Wine                    | Critical                  | None — required for Windows game execution      |
| flatpak-spawn                  | Critical (Flatpak)        | N/A — only used in Flatpak; is the gateway      |
| steam                          | Critical (Steam profiles) | N/A for Steam launches; non-Steam works without |
| umu-run                        | High                      | Direct Proton execution                         |
| kill/ps/cat/test               | High                      | Watchdog degraded but resilient                 |
| pgrep/realpath/basename/setsid | High                      | Shell helpers fail without these                |
| gamescope                      | Medium                    | Launch without compositor wrapping              |
| unshare                        | Medium                    | Launch without network isolation                |
| git                            | Medium                    | Cached tap fallback; no taps if never cloned    |
| mangohud/mangoapp              | Low                       | Launch without HUD overlay                      |
| winetricks/protontricks        | Low                       | Prefix dep feature disabled                     |
| lspci                          | Low                       | Partial diagnostics output                      |
| protonup                       | Low                       | Settings placeholder only                       |

## Edge Cases and Gotchas

- **flatpak-spawn silently drops `.env()`**: Environment variables set via Rust's `Command::env()` are not passed through `flatpak-spawn`. The `--env=K=V` args or env file handoff must be used instead. This is the reason for the entire env threading infrastructure in `platform.rs`.
- **`/run/host` path prefix**: Flatpak remaps host paths under `/run/host`. `normalize_flatpak_host_path()` must strip this prefix before passing paths to `flatpak-spawn --host` commands, or the host command will fail to find the file.
- **Nested gamescope detection**: `should_skip_gamescope()` checks `GAMESCOPE_WAYLAND_DISPLAY` to avoid nested gamescope sessions, which cause rendering issues. This env var check must happen at launch time, not at optimization resolution time.
- **OnceLock probe timing**: `is_unshare_net_available()` caches its result for the process lifetime. If system state changes (e.g., user gains/loses `CAP_SYS_ADMIN`), the cached result is stale until app restart.
- **System compat-tool path redirect**: Under Flatpak, `prefer_user_local_compat_tool_path()` redirects system Proton paths (e.g., `/usr/share/steam/...`) to user-local equivalents because the Flatpak sandbox cannot access system paths directly.
- **Gamescope PID capture in Flatpak**: The watchdog needs the host PID of gamescope, not the sandbox PID. A special bash script captures the PID after `flatpak-spawn` executes, using process scanning rather than simple `$!` capture.
- **Git CachedFallback**: Tap sync can silently serve stale data after a network failure. There is no staleness expiry — cached data is used indefinitely until a successful sync replaces it.
- **Trainer Proton parity**: Steam profiles still launch trainers through Proton. The trainer launch path must maintain parity with the game launch path for env vars, working directory, and gamescope integration, even though trainers are separate executables.
