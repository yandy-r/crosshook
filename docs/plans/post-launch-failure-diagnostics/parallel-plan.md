# Post-Launch Failure Diagnostics Implementation Plan

CrossHook's launch pipeline currently discards the child process `ExitStatus` at `stream_log_lines()` line 149 (`Ok(Some(_)) => break`). This plan implements a new `crosshook-core::launch::diagnostics` submodule that captures exit status, reads the last 2 MiB of log output via `safe_read_tail()`, runs a pure `analyze()` function that maps exit codes and pattern-matches known WINE/Proton failure signatures against a static `FAILURE_PATTERN_DEFINITIONS` catalog, and emits a `DiagnosticReport` as a Tauri event. The frontend hooks into this via `listen()` in `useLaunchState` and renders diagnostic findings in the existing `LaunchPanel` feedback area using the same severity badge pattern already used for validation issues. Zero new crate or npm dependencies are required — all analysis uses `str::contains()` pattern matching, `std::os::unix::process::ExitStatusExt` for signal introspection, and the existing `ValidationSeverity` enum.

## Critically Relevant Files and Documentation

- src/crosshook-native/src-tauri/src/commands/launch.rs: Primary integration point — `stream_log_lines()` (lines 121-172) where exit status capture, safe_read_tail, analyze(), and event emission are added
- src/crosshook-native/crates/crosshook-core/src/launch/mod.rs: Module root — add `pub mod diagnostics;` and re-export `analyze`, `DiagnosticReport`
- src/crosshook-native/crates/crosshook-core/src/launch/request.rs: Defines `ValidationSeverity` (line 143), `LaunchValidationIssue` (line 151), method constants — all reused directly
- src/crosshook-native/crates/crosshook-core/src/launch/optimizations.rs: `LAUNCH_OPTIMIZATION_DEFINITIONS` (line 40) — the exact data-driven catalog pattern to replicate for `FAILURE_PATTERN_DEFINITIONS`
- src/crosshook-native/crates/crosshook-core/src/steam/diagnostics.rs: `DiagnosticCollector` — collector + dedup pattern to inform new diagnostics module design
- src/crosshook-native/src/hooks/useLaunchState.ts: React state machine — add `diagnosticReport` state and event listeners
- src/crosshook-native/src/components/LaunchPanel.tsx: Feedback rendering (lines 683-757; JSX feedback container at 730-757) — add diagnostic banner using existing severity badge CSS
- src/crosshook-native/src/components/ConsoleView.tsx: Reference `listen()` event pattern (lines 45-73) to copy for new listeners
- src/crosshook-native/src/types/launch.ts: `LaunchFeedback` discriminated union (line 42) — extend with diagnostic kind
- src/crosshook-native/crates/crosshook-cli/src/main.rs: CLI `launch_profile()` (lines 68-76) — wire `analyze()` after `child.wait()`
- docs/plans/post-launch-failure-diagnostics/feature-spec.md: Authoritative feature spec with business rules, data models, phasing, and security requirements
- docs/plans/post-launch-failure-diagnostics/research-security.md: 4 WARNING findings (bounded reads, path sanitization, frontend cap, non-UTF-8 handling)
- docs/features/steam-proton-trainer-launch.doc.md: Launch method differences and `steam_applaunch` exit code unreliability

## Implementation Plan

### Phase 1: Exit Code Foundation

#### Task 1.1: Define diagnostic data models Depends on [none]

**READ THESE BEFORE TASK**

- src/crosshook-native/crates/crosshook-core/src/launch/request.rs (lines 143-200 for ValidationSeverity, LaunchValidationIssue, method constants)
- src/crosshook-native/crates/crosshook-core/src/launch/optimizations.rs (lines 31-60 for LaunchOptimizationDefinition struct layout)
- docs/plans/post-launch-failure-diagnostics/feature-spec.md (data models section)

**Instructions**

Files to Create

- src/crosshook-native/crates/crosshook-core/src/launch/diagnostics/models.rs

Define all data types for the diagnostics system. This is pure type definitions with serde derives — no logic. All types that cross the IPC boundary must derive `Debug, Clone, PartialEq, Eq, Serialize, Deserialize`. Enum variants must use `#[serde(rename_all = "snake_case")]`.

Types to define:

- `DiagnosticReport` struct: `severity: ValidationSeverity`, `summary: String`, `exit_info: ExitCodeInfo`, `pattern_matches: Vec<PatternMatch>`, `suggestions: Vec<ActionableSuggestion>`, `launch_method: String`, `log_tail_path: Option<String>`, `analyzed_at: String`
- `ExitCodeInfo` struct: `code: Option<i32>`, `signal: Option<i32>`, `signal_name: Option<String>`, `core_dumped: bool`, `failure_mode: FailureMode`, `description: String`, `severity: ValidationSeverity`
- `FailureMode` enum: all 15 variants from feature spec (`CleanExit`, `NonZeroExit`, `Segfault`, `Abort`, `Kill`, `BusError`, `IllegalInstruction`, `FloatingPointException`, `BrokenPipe`, `Terminated`, `CommandNotFound`, `PermissionDenied`, `UnknownSignal`, `Indeterminate`, `Unknown`)
- `PatternMatch` struct: `pattern_id: String`, `summary: String`, `severity: ValidationSeverity`, `matched_line: Option<String>`, `suggestion: String`
- `ActionableSuggestion` struct: `title: String`, `description: String`, `severity: ValidationSeverity`
- `FailurePatternDef` struct (private, used by patterns.rs): `id: &'static str`, `markers: &'static [&'static str]`, `failure_mode: FailureMode`, `severity: ValidationSeverity`, `summary: &'static str`, `suggestion: &'static str`, `applies_to_methods: &'static [&'static str]`

Security constants (define at top of file):

```rust
pub const MAX_LOG_TAIL_BYTES: u64 = 2 * 1024 * 1024;
pub const MAX_DIAGNOSTIC_ENTRIES: usize = 50;
pub const MAX_LINE_DISPLAY_CHARS: usize = 500;
```

Import `ValidationSeverity` from `crate::launch::request::ValidationSeverity` — do NOT define a new severity enum.

#### Task 1.2: Implement exit code analysis Depends on [1.1]

**READ THESE BEFORE TASK**

- src/crosshook-native/crates/crosshook-core/src/launch/diagnostics/models.rs (from Task 1.1)
- docs/plans/post-launch-failure-diagnostics/research-external.md (ExitStatusExt API, Unix signal codes)
- src/crosshook-native/crates/crosshook-core/src/launch/request.rs (method constants: METHOD_STEAM_APPLAUNCH, METHOD_PROTON_RUN, METHOD_NATIVE)

**Instructions**

Files to Create

- src/crosshook-native/crates/crosshook-core/src/launch/diagnostics/exit_codes.rs

Implement `analyze_exit_status()` as a pure function that translates a raw `ExitStatus` into a structured `ExitCodeInfo`. Use `std::os::unix::process::ExitStatusExt` for `signal()`, `core_dumped()`, and `code()`.

Key logic:

- `code() == Some(0)` and method is `steam_applaunch` → `FailureMode::Indeterminate` with `Info` severity (helper script exits 0 even on game crash)
- `code() == Some(0)` and other methods → `FailureMode::CleanExit` with `Info` severity
- `signal() == Some(11)` → `FailureMode::Segfault`, `Fatal`
- `signal() == Some(6)` → `FailureMode::Abort`, `Fatal`
- `signal() == Some(9)` → `FailureMode::Kill`, `Warning`
- `signal() == Some(15)` → `FailureMode::Terminated`, `Warning`
- `code() == Some(127)` → `FailureMode::CommandNotFound`, `Fatal`
- `code() == Some(126)` → `FailureMode::PermissionDenied`, `Fatal`
- Other non-zero codes → `FailureMode::NonZeroExit`, `Warning`
- Unknown/other → `FailureMode::Unknown`, `Warning`

Include signal name mapping: `signal_name_from_number(signal: i32) -> &'static str` (e.g., 11 → "SIGSEGV").

Include `#[cfg(test)] mod tests` with table-driven unit tests covering: exit code 0, exit code 1, signal 11 (SIGSEGV), signal 6 (SIGABRT), signal 9 (SIGKILL), signal 15 (SIGTERM), code 127, code 126, `steam_applaunch` with code 0 (indeterminate), and unknown positive codes.

#### Task 1.3: Capture ExitStatus in stream_log_lines Depends on [none]

**READ THESE BEFORE TASK**

- src/crosshook-native/src-tauri/src/commands/launch.rs (full file — understand stream_log_lines loop, spawn_log_stream, launch_game, launch_trainer)

**Instructions**

Files to Modify

- src/crosshook-native/src-tauri/src/commands/launch.rs

This is a minimal, surgical edit — only capture the exit status that is currently discarded. Do NOT wire `analyze()` or emit `launch-diagnostic` yet (that is Task 3.2).

Changes:

1. Add `use std::os::unix::process::ExitStatusExt;` import
2. In `stream_log_lines()`, add `method: &str` parameter to the function signature
3. Declare `let mut exit_status: Option<std::process::ExitStatus> = None;` before the poll loop
4. Change line 149-150 from `Ok(Some(_)) => break` to `Ok(Some(status)) => { exit_status = Some(status); break; }`
5. After the existing final log drain block (after ~line 171), add:

   ```rust
   let exit_code = exit_status.and_then(|s| s.code());
   let signal = exit_status.and_then(|s| s.signal());
   if let Err(error) = app.emit("launch-complete", serde_json::json!({ "code": exit_code, "signal": signal })) {
       tracing::warn!(%error, "failed to emit launch-complete event");
   }
   ```

6. Update `spawn_log_stream()` to accept `method: &str` and pass it through to `stream_log_lines()`
7. Update callers in `launch_game()` and `launch_trainer()` to pass the method string

### Phase 2: Pattern Matching Engine

#### Task 2.1: Implement failure pattern catalog and scanner Depends on [1.1]

**READ THESE BEFORE TASK**

- src/crosshook-native/crates/crosshook-core/src/launch/optimizations.rs (lines 31-177 for LAUNCH_OPTIMIZATION_DEFINITIONS pattern — replicate this exactly)
- src/crosshook-native/crates/crosshook-core/src/launch/diagnostics/models.rs (FailurePatternDef, PatternMatch, FailureMode)
- docs/plans/post-launch-failure-diagnostics/research-business.md (12 failure pattern definitions)
- src/crosshook-native/crates/crosshook-core/src/steam/diagnostics.rs (DiagnosticCollector dedup pattern)

**Instructions**

Files to Create

- src/crosshook-native/crates/crosshook-core/src/launch/diagnostics/patterns.rs

Implement the data-driven pattern catalog and scanner. Follow `LAUNCH_OPTIMIZATION_DEFINITIONS` exactly for the catalog structure.

1. Define `FAILURE_PATTERN_DEFINITIONS: &[FailurePatternDef]` as a `const` static slice. Include these 10 initial patterns (from feature spec):
   - `wine_ntdll_missing`: markers `["ntdll.dll not found"]`, `Fatal`, proton_run only
   - `wine_vulkan_init_fail`: markers `["Failed to init Vulkan", "winevulkan"]`, `Fatal`, proton_run only
   - `wine_prefix_missing`: markers `["WINEPREFIX", "does not exist"]`, `Fatal`, proton_run only
   - `proton_version_mismatch`: markers `["Proton: No compatibility tool"]`, `Fatal`, steam_applaunch only
   - `steam_not_running`: markers `["Steam is not running"]`, `Fatal`, steam_applaunch only
   - `permission_denied`: markers `["Permission denied"]`, `Fatal`, all methods
   - `exe_not_found`: markers `["cannot find", "No such file"]`, `Fatal`, all methods
   - `wine_crash_dump`: markers `["Unhandled exception", "backtrace:"]`, `Warning`, proton_run only
   - `dxvk_state_cache`: markers `["DXVK: State cache"]`, `Info`, proton_run only
   - `wine_fixme_noise`: markers `["fixme:"]`, `Info`, proton_run only (only surface when non-zero exit)

2. Implement `pub fn scan_log_patterns(log_tail: &str, method: &str) -> Vec<PatternMatch>`:
   - Filter patterns by `applies_to_methods` (empty slice = all methods)
   - For each pattern, check if ANY marker in `markers` matches via `str::contains()`
   - Build `PatternMatch` with matched line (first matching line, truncated to `MAX_LINE_DISPLAY_CHARS`)
   - Deduplicate by pattern_id (use `HashSet` + ordered Vec, like `DiagnosticCollector`)
   - Sort by severity (fatal → warning → info)
   - Truncate to `MAX_DIAGNOSTIC_ENTRIES`

3. Include `#[cfg(test)] mod tests` with:
   - Table-driven test asserting all patterns have non-empty `id`, `markers`, and `suggestion`
   - Individual fixture tests: pass a literal WINE log string and assert the correct pattern matches
   - Test that `applies_to_methods` filtering works correctly
   - Test deduplication and max-entry cap

#### Task 2.2: Implement safe_read_tail and sanitize_display_path Depends on [1.1, 1.3]

**READ THESE BEFORE TASK**

- src/crosshook-native/src-tauri/src/commands/launch.rs (current stream_log_lines final read at lines 162-171)
- src/crosshook-native/src-tauri/src/commands/shared.rs (create_log_path for context on log file paths)
- docs/plans/post-launch-failure-diagnostics/research-security.md (W1: bounded reads, W4: path sanitization)

**Instructions**

Files to Modify

- src/crosshook-native/src-tauri/src/commands/launch.rs (add private helper functions)

Implement two private async helper functions in `launch.rs`:

1. `async fn safe_read_tail(path: &Path, max_bytes: u64) -> String`:
   - Open file with `tokio::fs::File::open()`
   - Get file length with `metadata().await?.len()`
   - If len > max_bytes, seek to `SeekFrom::End(-(max_bytes as i64))`
   - Read remaining bytes with `read_to_end()`
   - Convert with `String::from_utf8_lossy()` — WINE can produce non-UTF-8 bytes
   - On any error, return empty string with `tracing::warn!` (non-fatal)
   - Define `const MAX_LOG_TAIL_BYTES: u64 = 2 * 1024 * 1024;` locally in `launch.rs` (the canonical constant in `diagnostics::models` is not importable until Task 2.3 wires the module — Task 3.1 will switch to the proper import when composing everything)

2. `fn sanitize_display_path(path: &str) -> String`:
   - Replace `$HOME` prefix (from `std::env::var("HOME")`) with `~`
   - Apply to all user-visible path strings in `DiagnosticReport` before emission
   - Pure function, no I/O

#### Task 2.3: Implement analyze() public API and module root Depends on [1.1, 1.2, 2.1]

**READ THESE BEFORE TASK**

- src/crosshook-native/crates/crosshook-core/src/launch/diagnostics/models.rs (all types)
- src/crosshook-native/crates/crosshook-core/src/launch/diagnostics/exit_codes.rs (analyze_exit_status)
- src/crosshook-native/crates/crosshook-core/src/launch/diagnostics/patterns.rs (scan_log_patterns)
- src/crosshook-native/crates/crosshook-core/src/launch/mod.rs (existing re-export pattern)
- src/crosshook-native/crates/crosshook-core/src/launch/preview.rs (build_launch_preview pattern for pure composition function)

**Instructions**

Files to Create

- src/crosshook-native/crates/crosshook-core/src/launch/diagnostics/mod.rs

Files to Modify

- src/crosshook-native/crates/crosshook-core/src/launch/mod.rs

1. Create `diagnostics/mod.rs`:
   - Declare `pub mod exit_codes;`, `pub mod models;`, `pub mod patterns;`
   - Re-export key types: `pub use models::{DiagnosticReport, ExitCodeInfo, PatternMatch, ActionableSuggestion, FailureMode, MAX_LOG_TAIL_BYTES, MAX_DIAGNOSTIC_ENTRIES, MAX_LINE_DISPLAY_CHARS};`
   - Implement `pub fn analyze(exit_status: Option<std::process::ExitStatus>, log_tail: &str, method: &str) -> DiagnosticReport`:
     - Call `exit_codes::analyze_exit_status(exit_status, method)` → `ExitCodeInfo`
     - Call `patterns::scan_log_patterns(log_tail, method)` → `Vec<PatternMatch>`
     - Build `suggestions` from pattern matches and exit info
     - Determine overall `severity` as max of exit_info.severity and pattern severities
     - Generate `summary` string from exit info and pattern count
     - Set `analyzed_at` with `chrono::Utc::now().to_rfc3339()`
     - Return assembled `DiagnosticReport`
   - This is a **pure function** — zero I/O, zero async, fully deterministic for the same inputs
   - Include `#[cfg(test)] mod tests` with integration tests: pass realistic combinations of exit status + log content + method and verify the complete `DiagnosticReport`

2. Modify `launch/mod.rs`:
   - Add `pub mod diagnostics;`
   - Add `pub use diagnostics::{analyze, DiagnosticReport};`

### Phase 3: Backend Integration

#### Task 3.1: Wire analyze() into stream_log_lines Depends on [1.3, 2.2, 2.3]

**READ THESE BEFORE TASK**

- src/crosshook-native/src-tauri/src/commands/launch.rs (full file — understand the state after Tasks 1.3 and 2.2)
- src/crosshook-native/crates/crosshook-core/src/launch/diagnostics/mod.rs (analyze API from Task 2.3)

**Instructions**

Files to Modify

- src/crosshook-native/src-tauri/src/commands/launch.rs

Wire the complete diagnostic pipeline into `stream_log_lines()`. This builds on the exit status capture from Task 1.3 and the helpers from Task 2.2.

After the existing final log drain block and before the `launch-complete` emit (from Task 1.3), add:

```rust
// Read log tail for analysis
let log_tail = safe_read_tail(&log_path, crosshook_core::launch::diagnostics::MAX_LOG_TAIL_BYTES).await;
// Run diagnostic analysis (pure function — no I/O)
let report = crosshook_core::launch::diagnostics::analyze(exit_status, &log_tail, method);
// Sanitize paths in report before emission
// ... apply sanitize_display_path to relevant fields
// Emit diagnostic event BEFORE launch-complete (order matters for frontend)
if let Err(error) = app.emit("launch-diagnostic", &report) {
    tracing::warn!(%error, "failed to emit launch-diagnostic event");
}
```

Ensure event emission order: all `launch-log` events → `launch-diagnostic` → `launch-complete`. The frontend's `launch-complete` handler may transition state, so diagnostics must arrive first.

#### Task 3.2: Wire analyze() into CLI Depends on [2.3]

**READ THESE BEFORE TASK**

- src/crosshook-native/crates/crosshook-cli/src/main.rs (lines 60-76 — launch_profile function, child.wait())
- src/crosshook-native/crates/crosshook-core/src/launch/diagnostics/mod.rs (analyze API)

**Instructions**

Files to Modify

- src/crosshook-native/crates/crosshook-cli/src/main.rs

Small change (~10 lines). After `child.wait().await?` at line 70, before the success check:

1. Read the log tail using `tokio::fs::read(&log_path).await` + `String::from_utf8_lossy()` (the CLI is async — `launch_profile()` is an `async fn` with `#[tokio::main]`). Reference the existing `log_path` variable already declared at line 65 (`let log_path = launch_log_path(&profile_name)`)
2. Call `crosshook_core::launch::diagnostics::analyze(Some(status), &log_tail, &method)`
3. Print `report.summary` to stderr via `eprintln!`
4. If any pattern matches exist, print each with severity prefix

The CLI has no event system — output goes directly to stderr. Keep it simple: summary line + pattern list.

### Phase 4: Frontend Integration

#### Task 4.1: Define TypeScript diagnostic types Depends on [2.3]

**READ THESE BEFORE TASK**

- src/crosshook-native/crates/crosshook-core/src/launch/diagnostics/models.rs (Rust type definitions — TypeScript must mirror exactly)
- src/crosshook-native/src/types/launch.ts (existing LaunchFeedback, LaunchValidationSeverity, LaunchValidationIssue patterns)
- src/crosshook-native/src/types/index.ts (barrel re-export pattern)

**Instructions**

Files to Create

- src/crosshook-native/src/types/diagnostics.ts

Files to Modify

- src/crosshook-native/src/types/launch.ts
- src/crosshook-native/src/types/index.ts

1. Create `diagnostics.ts` mirroring all Rust structs:
   - `DiagnosticReport` interface: all fields use `snake_case` matching Rust serde output
   - `ExitCodeInfo` interface
   - `FailureMode` type (string union of all 15 variant names in snake_case)
   - `PatternMatch` interface
   - `ActionableSuggestion` interface
   - Optional fields use `T | null` not `T | undefined` (Rust `Option<T>` serializes as `null`)
   - Add type guard: `isDiagnosticReport(value: unknown): value is DiagnosticReport`

2. Extend `launch.ts`:
   - Import `DiagnosticReport` from `./diagnostics`
   - Extend `LaunchFeedback` union: `| { kind: 'diagnostic'; report: DiagnosticReport }`

3. Extend `index.ts`:
   - Add `export * from './diagnostics'`

#### Task 4.2: Extend useLaunchState with diagnostic events Depends on [4.1]

**READ THESE BEFORE TASK**

- src/crosshook-native/src/hooks/useLaunchState.ts (full file — understand state shape, reducer, actions)
- src/crosshook-native/src/components/ConsoleView.tsx (lines 45-73 — reference listen()/unlisten pattern)
- src/crosshook-native/src/types/diagnostics.ts (DiagnosticReport type from Task 4.1)

**Instructions**

Files to Modify

- src/crosshook-native/src/hooks/useLaunchState.ts

1. Extend `LaunchState` type: add `diagnosticReport: DiagnosticReport | null` (initialize as `null`)

2. Add two new action types to the `LaunchAction` union:
   - `| { type: 'diagnostic-received'; report: DiagnosticReport }`
   - `| { type: 'launch-complete' }`

3. Add reducer cases:
   - `'diagnostic-received'`: set `diagnosticReport` to `action.report`, set `feedback` to `{ kind: 'diagnostic', report: action.report }`
   - `'launch-complete'`: no-op for now (future: could transition to a completed state)
   - `'reset'`: clear `diagnosticReport` to `null`

4. Add `useEffect` for Tauri event listeners — follow the `ConsoleView.tsx:45-73` pattern exactly:

   ```typescript
   useEffect(() => {
     let active = true;
     const unlistenDiagnostic = listen<DiagnosticReport>('launch-diagnostic', (event) => {
       if (active) dispatch({ type: 'diagnostic-received', report: event.payload });
     });
     const unlistenComplete = listen<void>('launch-complete', () => {
       if (active) dispatch({ type: 'launch-complete' });
     });
     return () => {
       active = false;
       void unlistenDiagnostic.then((fn) => fn());
       void unlistenComplete.then((fn) => fn());
     };
   }, []); // empty deps — listeners persist for component lifetime
   ```

   Do NOT add `method` or `profileId` to deps — the existing reset effect already clears state on those changes.

5. Expose `diagnosticReport` from the hook's return value.

#### Task 4.3: Render diagnostic banner in LaunchPanel Depends on [4.1, 4.2]

**READ THESE BEFORE TASK**

- src/crosshook-native/src/components/LaunchPanel.tsx (lines 683-757; JSX feedback container at 730-757 — feedback rendering area with data-severity badges)
- src/crosshook-native/src/hooks/useLaunchState.ts (diagnosticReport from Task 4.2)
- src/crosshook-native/src/styles/variables.css (existing severity color tokens)

**Instructions**

Files to Modify

- src/crosshook-native/src/components/LaunchPanel.tsx

1. In the feedback derivation block (~line 683), add:

   ```typescript
   const diagnosticFeedback = feedback?.kind === 'diagnostic' ? feedback.report : null;
   ```

2. Add a new rendering branch in the `crosshook-launch-panel__feedback` container for `diagnosticFeedback`:
   - Render severity badge using `data-severity={diagnosticFeedback.severity}` (reuse existing CSS)
   - Show `diagnosticFeedback.summary` as the banner title
   - Show `diagnosticFeedback.exit_info.description` as subtitle
   - If `pattern_matches.length > 0`, render each as a list item with severity badge + summary + suggestion
   - Sort pattern matches using the same severity order as `sortIssuesBySeverity()` at line 69: `{ fatal: 0, warning: 1, info: 2 }`

3. Add progressive disclosure (expand/collapse):
   - Default: show summary + top 3 pattern matches
   - Expand: show all pattern matches + suggestions + exit info details
   - Use a local `useState<boolean>` for expand/collapse state

4. Add copy-to-clipboard for the full diagnostic report:
   - Button in the expanded view: "Copy Report"
   - Use `navigator.clipboard.writeText(JSON.stringify(diagnosticFeedback, null, 2))`
   - Show inline "Copied!" state change with `useState<boolean>` + `setTimeout(reset, 2000)`

5. Use existing CSS classes and custom properties — no new CSS files:
   - `--crosshook-color-danger` for fatal, `--crosshook-color-warning` for warning, `--crosshook-color-accent-strong` for info
   - `crosshook-launch-panel__feedback-badge`, `crosshook-launch-panel__feedback-title`, `crosshook-launch-panel__feedback-help`

## Advice

- **Event emission order is critical**: `launch-diagnostic` MUST emit before `launch-complete`. The frontend's `launch-complete` handler may transition state that would ignore a subsequent diagnostic event. Verify this ordering in Task 3.1.
- **`steam_applaunch` exit code 0 is NOT success**: The helper script always exits 0 after handing off to Steam. `analyze_exit_status()` must classify this as `FailureMode::Indeterminate`, not `CleanExit`. Pattern matching on the helper log is the primary failure signal for this method. Test this explicitly.
- **`stream_log_lines()` signature change ripples**: Adding `method: &str` to the function signature requires updating `spawn_log_stream()` and both callers (`launch_game`, `launch_trainer`). The method string is already available at the call sites from `request.resolved_method()`.
- **Do NOT use `read_to_string()` in `safe_read_tail()`**: WINE logs can contain non-UTF-8 binary bytes. Use `read_to_end()` → `String::from_utf8_lossy()`. The existing `stream_log_lines()` silently ignores read errors on the main loop; the tail reader should do the same.
- **`listen()` effect deps must be `[]` (empty)**: Diagnostic event listeners must persist for the full component lifetime. The existing `useEffect(() => dispatch({ type: "reset" }), [method, profileId])` already clears feedback state on profile/method changes. Do not duplicate this cleanup in the listener effect.
- **`commands/launch.rs` is touched by three tasks (1.3, 2.2, 3.1)**: These must execute sequentially. Task 1.3 captures exit status + emits `launch-complete`. Task 2.2 adds `safe_read_tail()` and `sanitize_display_path()` as private helpers. Task 3.1 wires everything together with `analyze()` and `launch-diagnostic` emit.
- **No `regex` crate in Phase 1**: All 10 initial patterns use fixed literal substrings via `str::contains()`. `regex` is not a direct dependency of `crosshook-core`. Only add it if pattern count exceeds ~50 or case-insensitive matching becomes necessary.
- **`ValidationSeverity` reuse is mandatory**: Import from `crate::launch::request` — never create a new severity enum. The frontend CSS already maps `data-severity` attributes to badge colors. Creating a duplicate would break the rendering pipeline.
- **Task 2.2 `safe_read_tail()` lives in the Tauri command layer**: It's an async I/O helper, not business logic. `crosshook-core` is intentionally I/O-free; `analyze()` is a pure sync function. Keep the async file reading in `commands/launch.rs`.
- **Security constants must be named `const` items**: Follow the `script_runner.rs` convention of named constants (`BASH_EXECUTABLE`, `DEFAULT_GAME_STARTUP_DELAY_SECONDS`). Never use magic numbers inline.
