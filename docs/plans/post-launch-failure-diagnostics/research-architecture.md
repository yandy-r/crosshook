# Architecture Research: post-launch-failure-diagnostics

## System Overview

CrossHook is a Tauri v2 desktop app with a Rust backend (`crosshook-core` library + `src-tauri` shell) and a React/TypeScript frontend. The launch pipeline spawns child processes via `tokio::process`, streams their log output over Tauri events, and currently discards the process `ExitStatus`. The diagnostics feature adds a new submodule `crosshook-core::launch::diagnostics` that receives captured exit status and log tail after child exit, runs a pure analysis function, and emits a `launch-diagnostic` Tauri event for the frontend to render in the existing `LaunchPanel` feedback area.

## Relevant Components

### Rust Backend

- `src/crosshook-native/crates/crosshook-core/src/launch/mod.rs`: Module root for all launch primitives; re-exports `ValidationSeverity`, `LaunchValidationIssue`, method constants. **Add `pub mod diagnostics;` here.**
- `src/crosshook-native/crates/crosshook-core/src/launch/request.rs:143-156`: Defines `ValidationSeverity` enum (`Fatal`, `Warning`, `Info`, serde `snake_case`) and `LaunchValidationIssue { message, help, severity }`. Reused as-is in `DiagnosticReport`.
- `src/crosshook-native/crates/crosshook-core/src/launch/optimizations.rs:31-80`: `LaunchOptimizationDefinition` struct and `LAUNCH_OPTIMIZATION_DEFINITIONS` const array — the direct template for `FailurePatternDef` / `FAILURE_PATTERN_DEFINITIONS`.
- `src/crosshook-native/crates/crosshook-core/src/steam/diagnostics.rs`: `DiagnosticCollector` with dedup — provides the inspiration for collecting and deduplicating pattern matches; not directly reused but shows the idiom.
- `src/crosshook-native/src-tauri/src/commands/launch.rs:121-172`: **Primary integration point.** `stream_log_lines()` polls `child.try_wait()` (line 149). Currently `Ok(Some(_)) => break` discards the exit status. After the final log read (line 162), add: capture `ExitStatus`, call `safe_read_tail()`, call `analyze()`, emit `launch-diagnostic`, emit `launch-complete`.
- `src/crosshook-native/src-tauri/src/commands/shared.rs`: `create_log_path()` produces the log path passed to `stream_log_lines()` — already available as `log_path: PathBuf`.
- `src/crosshook-native/crates/crosshook-cli/src/main.rs:68-76`: CLI calls `child.wait()` and tests `status.success()`. Quick win: wire in `analyze()` here with ~5 lines.

### React Frontend

- `src/crosshook-native/src/hooks/useLaunchState.ts`: State machine (reducer pattern) for launch lifecycle. Currently uses `invoke()` only — no Tauri event listeners. **Add `listen('launch-diagnostic', ...)` and `listen('launch-complete', ...)` here.** Expose `diagnosticReport` from the hook.
- `src/crosshook-native/src/components/LaunchPanel.tsx:704-821`: Renders `feedback` from `useLaunchState` in `crosshook-launch-panel__feedback` div (line 730-757). Existing `validation` feedback renders `badge + title + help`. New `diagnostic` kind renders here using same CSS classes.
- `src/crosshook-native/src/components/ConsoleView.tsx:45-73`: Listens to `launch-log` and `update-log` events via `listen()`. Pattern to follow for adding `launch-diagnostic` listener in `useLaunchState`.
- `src/crosshook-native/src/types/launch.ts:42-44`: `LaunchFeedback` union type — currently `validation | runtime`. **Extend with `| { kind: 'diagnostic'; report: DiagnosticReport }`.**
- `src/crosshook-native/src/types/index.ts`: Barrel re-export. **Add `export * from './diagnostics';`** once `diagnostics.ts` is created.

## Data Flow

```
child process exits
    → stream_log_lines(): child.try_wait() returns Some(status)       [launch.rs:149]
    → final log drain (existing)                                        [launch.rs:162-171]
    → [NEW] capture ExitStatus: code + signal via ExitStatusExt
    → [NEW] safe_read_tail(log_path, 2 MiB) → &str
    → [NEW] launch::diagnostics::analyze(exit_code, signal, core_dumped, log_content, method)
        → analyze_exit_status() → ExitCodeInfo
        → scan_log_patterns() → Vec<PatternMatch>
        → build_suggestions() → Vec<ActionableSuggestion>
        → returns DiagnosticReport
    → [NEW] app.emit("launch-diagnostic", &report)
    → [NEW] app.emit("launch-complete", { code, signal })

Frontend:
    useLaunchState: listen('launch-diagnostic') → dispatch({ type: 'diagnostic', report })
    useLaunchState: listen('launch-complete') → dispatch({ type: 'launch-complete', ... })
    LaunchPanel: renders feedback with kind='diagnostic' → DiagnosticBanner component
```

## Integration Points

### Where new code connects

1. **`crosshook-core/src/launch/mod.rs`** — add `pub mod diagnostics;` and re-export `analyze`, `DiagnosticReport`.

2. **`src-tauri/src/commands/launch.rs:149`** — change `Ok(Some(_)) => break` to `Ok(Some(status)) => { exit_status = Some(status); break; }`. After existing final log drain, add diagnostic block.

3. **`src-tauri/src/commands/launch.rs`** — `stream_log_lines()` signature needs `target_kind: &str` and `method: &str` parameters (or derive from `LaunchRequest` passed in), so `analyze()` receives the right context.

4. **`useLaunchState.ts`** — add state field `diagnosticReport: DiagnosticReport | null`, reducer action `diagnostic`, and `useEffect` with `listen('launch-diagnostic', ...)`.

5. **`LaunchPanel.tsx`** — add branch in feedback rendering for `feedback.kind === 'diagnostic'`.

6. **`crosshook-cli/src/main.rs:70-73`** — after `child.wait()`, read log tail, call `analyze()`, print summary.

### New files to create

| File                                                         | Purpose                                                                                   |
| ------------------------------------------------------------ | ----------------------------------------------------------------------------------------- |
| `crates/crosshook-core/src/launch/diagnostics/mod.rs`        | Public `analyze()` API                                                                    |
| `crates/crosshook-core/src/launch/diagnostics/models.rs`     | `DiagnosticReport`, `ExitCodeInfo`, `FailureMode`, `PatternMatch`, `ActionableSuggestion` |
| `crates/crosshook-core/src/launch/diagnostics/exit_codes.rs` | `analyze_exit_status()` pure function                                                     |
| `crates/crosshook-core/src/launch/diagnostics/patterns.rs`   | `FAILURE_PATTERN_DEFINITIONS` catalog + `scan_log_patterns()`                             |
| `src/crosshook-native/src/types/diagnostics.ts`              | TypeScript IPC types                                                                      |

## Gotchas & Edge Cases

- **`steam_applaunch` log scope**: The helper script exits 0 immediately after handing off to Steam. Game/WINE output does NOT land in the helper log for this method — only script-level failures are detectable via log pattern matching. Pattern detection on `steam_applaunch` helper log is limited to pre-handoff failures; this aligns with BR-1 in the feature spec ("exit code is unreliable for game failures").
- **`stream_log_lines()` signature change**: Adding `method: &str` and `target_kind: &str` params is required so `analyze()` receives launch context. Both are already available at the `spawn_log_stream()` call sites in `launch_game` and `launch_trainer` commands.
- **No PID tracking or process groups**: `tokio::process::Child` with `try_wait()` polling at 500ms is the only process monitoring mechanism. There is no signal forwarding or process group management to worry about.
- **`steam_applaunch` vs `proton_run` diagnostic strategy**: For `proton_run` and `native`, exit code is reliable and is the primary signal. For `steam_applaunch`, the helper exits 0 immediately after handing off to Steam — pattern matching on the helper log is the primary signal, exit code is secondary.
- **Test isolation**: `crates/crosshook-core/src/launch/test_support.rs` contains `ScopedCommandSearchPath`, a RAII guard for test isolation in the launch module. Diagnostics unit tests using log fixtures should follow this pattern (use `tempfile::tempdir()` for log fixture isolation).
- **Log file pre-created before spawn**: `create_log_path()` calls `std::fs::File::create()` — file exists before the child process starts. `safe_read_tail()` will always find a valid (possibly empty) file.

## Key Dependencies

### Internal Modules

- `crosshook_core::launch::request::ValidationSeverity` — reused directly in `DiagnosticReport.severity` and `ExitCodeInfo.severity`
- `crosshook_core::launch::request::LaunchValidationIssue` — visual/UX pattern to mirror for `ActionableSuggestion`
- `crosshook_core::launch::optimizations::LAUNCH_OPTIMIZATION_DEFINITIONS` — struct layout template for `FAILURE_PATTERN_DEFINITIONS`
- `crosshook_core::steam::diagnostics::DiagnosticCollector` — dedup idiom to apply to `pattern_matches`

### Stdlib / Workspace Deps (no new deps needed)

- `std::os::unix::process::ExitStatusExt` — `signal()`, `core_dumped()`, `code()` methods
- `chrono` (already in Cargo.toml) — `analyzed_at` ISO 8601 timestamp
- `serde` / `serde_json` (workspace) — `DiagnosticReport` crosses IPC boundary
- `tokio::fs` + `tokio::io` (workspace) — `safe_read_tail()` async seek-based read
- `@tauri-apps/api/event` `listen()` — already used in `ConsoleView.tsx`, same pattern for `useLaunchState.ts`

### CSS / UI (no new tokens needed)

- `--crosshook-color-danger`, `--crosshook-color-warning`, `--crosshook-color-accent-strong` — existing severity color tokens
- `crosshook-launch-panel__feedback`, `crosshook-launch-panel__feedback-badge`, `crosshook-launch-panel__feedback-title`, `crosshook-launch-panel__feedback-help` — existing CSS classes in `LaunchPanel.tsx`
