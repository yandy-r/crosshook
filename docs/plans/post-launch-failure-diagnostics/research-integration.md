# Integration Research: Post-Launch Failure Diagnostics

## Overview

The post-launch failure diagnostics feature integrates at the intersection of three layers: the `stream_log_lines()` async loop in `src-tauri/src/commands/launch.rs` (where `child.try_wait()` currently discards the `ExitStatus`), the `crosshook-core` launch module (where new `diagnostics` submodule lives), and the React frontend (`useLaunchState` + `LaunchPanel`). All computation is local — no external APIs, no new crate dependencies. The critical integration point is line 150 of `launch.rs` where `Ok(Some(_)) => break` must be replaced with status capture + `analyze()` call + event emit.

---

## Relevant Files

- `src/crosshook-native/src-tauri/src/commands/launch.rs` — Tauri IPC commands; contains `stream_log_lines()` — the exact modification point
- `src/crosshook-native/src-tauri/src/commands/shared.rs` — `create_log_path()`: log file naming (`/tmp/crosshook-logs/{prefix}-{slug}-{ms}.log`)
- `src/crosshook-native/crates/crosshook-core/src/launch/mod.rs` — Launch module root; needs `pub mod diagnostics;` added
- `src/crosshook-native/crates/crosshook-core/src/launch/request.rs` — Defines `ValidationSeverity`, `LaunchValidationIssue`, `ValidationError`, method constants, `LaunchRequest`
- `src/crosshook-native/crates/crosshook-core/src/launch/optimizations.rs` — `LAUNCH_OPTIMIZATION_DEFINITIONS` data-driven catalog — the exact pattern to replicate for `FAILURE_PATTERN_DEFINITIONS`
- `src/crosshook-native/crates/crosshook-core/src/launch/runtime_helpers.rs` — `attach_log_stdio()`: both stdout and stderr are redirected to the same log file (append mode)
- `src/crosshook-native/crates/crosshook-core/src/launch/script_runner.rs` — Command builders for each launch method; shows env var injection patterns
- `src/crosshook-native/crates/crosshook-core/src/steam/diagnostics.rs` — `DiagnosticCollector` struct — collector + dedup pattern to inform new diagnostics module design
- `src/crosshook-native/src/hooks/useLaunchState.ts` — React state machine for launch phases; where `diagnosticReport` state and new event listeners must be added
- `src/crosshook-native/src/components/LaunchPanel.tsx` — Feedback rendering in `crosshook-launch-panel__feedback`; the exact insertion point for the diagnostic banner
- `src/crosshook-native/src/components/ConsoleView.tsx` — `listen('launch-log', ...)` and `listen('update-log', ...)`; the event listener pattern for new `launch-diagnostic` / `launch-complete` events
- `src/crosshook-native/src/types/launch.ts` — `LaunchFeedback` union type, `LaunchValidationIssue`, `LaunchPhase` enum — all need extending
- `src/crosshook-native/crates/crosshook-cli/src/main.rs` — CLI `launch_profile()`: calls `child.wait()` at line 70; the CLI hook for `analyze()` integration

---

## API Endpoints and Architectural Patterns

### Tauri IPC Commands

- **Command registration**: `src-tauri/src/lib.rs` registers handlers via `.invoke_handler(tauri::generate_handler![...])`. New Tauri _events_ (not commands) require no handler registration — they use `app.emit("event-name", payload)` directly.
- **Existing launch commands**: `validate_launch`, `preview_launch`, `launch_game`, `launch_trainer`, `build_steam_launch_options_command` — all in `commands/launch.rs`.
- **Phase 1 adds no new commands** — only two new emitted events: `launch-diagnostic` (DiagnosticReport payload) and `launch-complete` ({code, signal} payload).
- **IPC error serialization**: Commands return `Result<T, String>` — the `Err(String)` side maps to Tauri's `LaunchFeedback { kind: 'runtime', message }` on the frontend. Diagnostics use the separate event channel.

### Launch Orchestration (Rust Backend)

- **Process spawn pattern**: All three methods (`steam_applaunch`, `proton_run`, `native`) call `spawn()` → `tokio::process::Child`, then hand off to `spawn_log_stream(app, log_path, child)`.
- **`stream_log_lines()` loop** (`launch.rs:121–172`): polls `tokio::fs::read_to_string(&log_path)` every 500ms, emits `launch-log` events per line, checks `child.try_wait()`. **The critical integration point** is line 150: `Ok(Some(_)) => break` — this discards the `ExitStatus`. It must become:

  ```rust
  Ok(Some(status)) => { captured_status = Some(status); break; }
  ```

- **Post-exit final read** (lines 162–171): A second `read_to_string` captures lines written between last poll and exit. Diagnostic analysis runs _after_ this final drain.
- **Log file location**: `/tmp/crosshook-logs/{type}-{slug}-{ms}.log` — created by `create_log_path()` before spawn. Both stdout and stderr of the child process are appended to this same file via `attach_log_stdio()`.
- **`steam_applaunch` log content**: For this method, the child is a shell script (`steam-launch-helper.sh`) that itself launches Steam — script output goes to log but the game's WINE/Proton output does NOT (game is launched as a Steam subprocess). Pattern detection on the helper log only.
- **`proton_run` log content**: Direct `proton run` command — stdout+stderr of Proton/WINE process goes to log. This is where WINE `err:`, `fixme:` debug output appears.

### Process Management

- **No PID tracking**: The app spawns `tokio::process::Child` and polls `try_wait()`. There is no process group tracking, no timeout, and no SIGKILL mechanism in the Tauri side.
- **Exit status access**: `child.try_wait()` returns `Ok(Some(ExitStatus))`. Use `ExitStatusExt::signal()` and `ExitStatusExt::code()` on the captured status. `core_dumped()` is also available via `ExitStatusExt`.
- **Signal exits**: `ExitStatus::code()` returns `None` when killed by a signal. `ExitStatusExt::signal()` returns the signal number. Both must be captured before logging or analysis.
- **CLI difference**: `crosshook-cli/src/main.rs` uses `child.wait()` (blocking) at line 70, not `try_wait()`. It checks `status.success()` only — no signal introspection currently. CLI integration adds `analyze()` call after line 70.

### `safe_read_tail()` Integration Point

The `stream_log_lines()` final read (lines 162–171) uses `tokio::fs::read_to_string()` — reads the entire file. The new `safe_read_tail()` function reads only the last 2MiB using seek-based access (`AsyncSeekExt::seek(SeekFrom::End(-MAX_LOG_TAIL_BYTES))`). This is the implementation pattern already used in `crosshook-cli/src/main.rs`'s `drain_log()` (lines 268–289) which uses `seek(SeekFrom::Start(offset))`.

### Steam/Proton Integration

- **`WINEPREFIX`**: Set to `{compatdata_path}/pfx` for Steam method, or resolved via `resolve_wine_prefix_path()` for standalone Proton. Crash dumps land at `{WINEPREFIX}/drive_c/users/{user}/AppData/Local/CrashDumps/` (Phase 2).
- **`STEAM_COMPAT_DATA_PATH`**: Set to the compatdata directory. Crash reports also at `{STEAM_COMPAT_DATA_PATH}/crashes/` (Phase 2).
- **Proton log env vars**: `PROTON_LOG=1` enables Proton's own log file at `{game_exe_path}.log` (outside the helper log). The app does not set this currently — diagnostic patterns target what's already in the helper log.
- **`steam_applaunch` exit code unreliability**: The helper script (`steam-launch-helper.sh`) exits after calling `steam steam://rungameid/{appid}` — the helper exit code reflects Steam launch initiation, not game outcome. Game crash only shows in pattern matching on subsequent log lines.
- **Method constants** (used in `applies_to_methods` pattern filtering): `"steam_applaunch"`, `"proton_run"`, `"native"` — from `request.rs` constants `METHOD_STEAM_APPLAUNCH`, `METHOD_PROTON_RUN`, `METHOD_NATIVE`.

### Frontend State Management

- **`useLaunchState` reducer** (`useLaunchState.ts`): Simple 5-phase state machine with `LaunchPhase` enum (`Idle`, `GameLaunching`, `WaitingForTrainer`, `TrainerLaunching`, `SessionActive`). State shape is `{ phase, feedback, helperLogPath }`. **Modification needed**: add `diagnosticReport: DiagnosticReport | null` to `LaunchState` and new action types for diagnostic events.
- **`LaunchFeedback` union** (`types/launch.ts:42–44`): Currently `{ kind: 'validation'; issue } | { kind: 'runtime'; message }`. Extend with `| { kind: 'diagnostic'; report: DiagnosticReport }`.
- **Feedback rendering** (`LaunchPanel.tsx:730–757`): The `crosshook-launch-panel__feedback` div already handles `validationFeedback` (badge + message + help) and `runtimeFeedback` (message only). The diagnostic banner should be a third branch in this same container, reusing `data-severity` and the badge pattern.
- **No existing `listen()` for launch events in `useLaunchState`**: The current hook invokes commands via `invoke()` only — it does not listen for Tauri events. New `launch-diagnostic` and `launch-complete` listeners must be registered with `useEffect` + `listen()` from `@tauri-apps/api/event`, matching the pattern in `ConsoleView.tsx:45–73`.
- **`ConsoleView` event pattern**: `listen<LogPayload>('launch-log', handler)` returns an unlisten function; cleanup via `void unlistenLaunch.then((u) => u())`. Same pattern applies for new event listeners.

### Severity Reuse

- **`ValidationSeverity`** (`request.rs:144–149`): `enum ValidationSeverity { Fatal, Warning, Info }` — serialized as `snake_case` (`"fatal"`, `"warning"`, `"info"`). This is the Rust type to reuse in `DiagnosticReport`.
- **Frontend severity type** (`types/launch.ts:34`): `type LaunchValidationSeverity = 'fatal' | 'warning' | 'info'` — already rendered by `LaunchPanel` with `data-severity` attribute. CSS colors are already wired: `--crosshook-color-danger` for fatal, `--crosshook-color-warning` for warning, `--crosshook-color-accent-strong` for info.
- **Severity sort order** (`LaunchPanel.tsx:69`): `{ fatal: 0, warning: 1, info: 2 }` — existing sorting utility in `sortIssuesBySeverity()` can be adapted for `PatternMatch[]`.

### Data-Driven Catalog Pattern

- **`LAUNCH_OPTIMIZATION_DEFINITIONS`** (`optimizations.rs:40`): `const` slice of `LaunchOptimizationDefinition` structs with `id`, `applies_to_method`, `env`, `wrappers`, `conflicts_with`, `required_binary` fields. The `FAILURE_PATTERN_DEFINITIONS` catalog mirrors this exactly: static `&[FailurePatternDef]` with `id`, `markers`, `failure_mode`, `severity`, `suggestions`, `applies_to_methods` fields.
- **Method filtering**: `optimizations.rs` filters by `applies_to_method == request.resolved_method()`. The diagnostic catalog filters by `applies_to_methods.contains(&launch_method)`.

### Configuration

- **No new config fields in Phase 1**: Diagnostics are always-on for non-zero exits. The existing `~/.config/crosshook/settings.toml` does not need modification.
- **Log path**: Fixed at `/tmp/crosshook-logs/` — not configurable. The `log_path` string is available in `LaunchResult.helper_log_path` returned to the frontend and stored in `useLaunchState.helperLogPath`.
- **`DiagnosticReport` is not persisted** (Phase 1 decision B): Re-analyzed from log on demand. No new config dir entries.

---

## Gotchas & Edge Cases

- **`stream_log_lines()` drops `ExitStatus` silently**: Line 150 `Ok(Some(_)) => break` — the `_` discards the exit status. This is the only status capture point; if missed, there is no other place to get it after the loop exits.
- **Final read happens before analysis**: The post-exit final read (lines 162–171) uses `read_to_string` (full file). `safe_read_tail()` must be called _after_ this drain completes or replace it — both cannot run on the same log simultaneously.
- **`steam_applaunch` helper exits 0 on success**: The shell script exits 0 after handing off to Steam. For this method, pattern matching on the helper log is the primary failure signal; exit code 0 from the helper is not "success" in the game sense.
- **Log file created before spawn**: `create_log_path()` calls `File::create()` before the child process starts. If the child never writes to it (spawn failure), the file exists but is empty — `safe_read_tail()` returns `""`, `analyze()` runs with empty log content.
- **Both stdout and stderr go to the same log file**: `attach_log_stdio()` opens the same path twice (append mode) for stdout and stderr. Log lines from both streams are interleaved by timestamp of write. For `steam_applaunch`, only the shell script's output lands here, not the game's WINE stderr.
- **CLI uses `child.wait()` not `try_wait()`**: The CLI blocks on `child.wait()` and only checks `status.success()`. ExitStatusExt signal fields are available on the returned status but currently unused. The CLI integration point is `main.rs:70–75`.
- **No Tauri event listeners in `useLaunchState`**: The hook currently only uses `invoke()`. Adding `listen()` calls requires `useEffect` cleanup to prevent listener leaks on component unmount or profile/method change. The `useEffect(..., [method, profileId])` reset at line 124 must also clean up listeners.
- **`LaunchPhase` has no `Failed` state**: Failures are represented by setting `feedback` in existing phases (Idle or WaitingForTrainer as `fallbackPhase`). Diagnostic state should follow this same pattern — store `diagnosticReport` alongside `feedback`, not as a new phase.
- **`proton_run` WINE log format**: WINE debug output format is `err:module:function message` — the `err:` prefix is stable. `fixme:` lines are common in normal runs (false positive risk). Pattern matching should require non-zero exit OR only surface fixme patterns as `info` severity.
- **Non-UTF-8 bytes**: WINE can emit binary data to stderr. `tokio::fs::read_to_string()` fails on non-UTF-8. The current code silently ignores read errors (`Err(error) => warn!(...)`). `safe_read_tail()` should use `read_to_end()` + `String::from_utf8_lossy()`.

---

## Other Docs

- Feature spec (primary reference): `docs/plans/post-launch-failure-diagnostics/feature-spec.md`
- `DiagnosticCollector` (existing pattern): `crates/crosshook-core/src/steam/diagnostics.rs`
- Optimization catalog (pattern to replicate): `crates/crosshook-core/src/launch/optimizations.rs:40`
- CLI `drain_log()` (seek-based read pattern for `safe_read_tail()`): `crates/crosshook-cli/src/main.rs:268`
- Tauri emit API: `app.emit("event-name", payload)` — `AppHandle` has `Emitter` in scope via `use tauri::{AppHandle, Emitter, Manager}`
- Frontend event listener pattern: `ConsoleView.tsx:45–73`
