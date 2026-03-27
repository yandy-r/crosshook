# Pattern Research: post-launch-failure-diagnostics

## Architectural Patterns

**Data-driven catalog with `const` static slice**: Business logic encoded as a slice of definition structs resolved at runtime — no dynamic allocation. The new `FAILURE_PATTERN_DEFINITIONS` should follow this exactly.

- Example: `src/crosshook-native/crates/crosshook-core/src/launch/optimizations.rs:40` (`LAUNCH_OPTIMIZATION_DEFINITIONS`)

**Pure function computation**: All non-trivial logic lives in pure `fn` that take inputs and return a value — no I/O, no global state. Enables direct unit testing without mocking.

- Example: `build_launch_preview()` in `src/crosshook-native/crates/crosshook-core/src/launch/preview.rs:272`
- The `analyze()` function must follow this pattern.

**Submodule with `mod.rs` re-exports**: Each new submodule (`diagnostics/`) exposes a flat public API through `mod.rs`, hiding internal file layout from callers.

- Example: `src/crosshook-native/crates/crosshook-core/src/launch/mod.rs` — re-exports `validate`, `LaunchValidationIssue`, `ValidationSeverity`, etc.

**Builder/collector pattern for accumulation**: The `DiagnosticCollector` in `steam/diagnostics.rs` accumulates messages and finalizes with deduplication. The `analyze()` function can use an internal `Vec` accumulation with similar deduplication.

- Example: `src/crosshook-native/crates/crosshook-core/src/steam/diagnostics.rs:6`

**Tauri event emission (`app.emit()`)**: Long-running async tasks emit events to the frontend using `app.emit("event-name", payload)` instead of polling.

- Example: `src/crosshook-native/src-tauri/src/commands/launch.rs:135` (`app.emit("launch-log", ...)`)
- The `launch-diagnostic` event follows the same `app.emit("launch-diagnostic", report)` pattern.

**Partial results on independent failure**: Complex structs use `Option<T>` for sections that can fail independently, returning partial results rather than hard-failing.

- Example: `LaunchPreview.environment: Option<Vec<PreviewEnvVar>>` — `None` when directive resolution fails, not an error return.

## Code Conventions

### Rust

- **Struct fields**: All IPC-crossing types derive `Debug, Clone, PartialEq, Eq, Serialize, Deserialize`. Enum variants use `#[serde(rename_all = "snake_case")]`.
  - Example: `ValidationSeverity` at `request.rs:143`
- **Serde defaults on all IPC fields**: Every field in types that cross the IPC boundary uses `#[serde(default)]` to handle missing keys from older frontend payloads.
  - Example: `LaunchRequest` at `request.rs:16`
- **Module public API**: The `mod.rs` file uses `pub use` to flatten internal structure. All new types in `diagnostics/models.rs`, `diagnostics/exit_codes.rs`, and `diagnostics/patterns.rs` are re-exported from `diagnostics/mod.rs`.
- **Constants**: Security constants (caps, limits) are `const` at module scope with named identifiers, not magic numbers inline.
  - Example: `BASH_EXECUTABLE`, `DEFAULT_GAME_STARTUP_DELAY_SECONDS` in `script_runner.rs`
- **`tracing` for logging**: `tracing::warn!(%error, "description")` for non-fatal failures; `tracing::error!(%error, "description")` for task failures. Field syntax `key = %value` for structured logging.
  - Example: `src-tauri/src/commands/launch.rs:116,146,164`
- **`chrono::Utc::now().to_rfc3339()`** for ISO 8601 timestamps, consistent with `LaunchPreview.generated_at`.
  - Example: `preview.rs:337`

### TypeScript

- **Discriminated unions for feedback kinds**: `LaunchFeedback` uses `kind` as the discriminant (`'validation' | 'runtime'`). The new `'diagnostic'` kind follows this pattern.
  - Example: `src/crosshook-native/src/types/launch.ts:42`
- **Type guard functions**: Runtime type checking uses explicit guard functions like `isLaunchValidationIssue()`.
  - Example: `launch.ts:46`
- **Type file per concern**: Each domain gets its own file (`launch.ts`, `diagnostics.ts`) with a barrel re-export in `index.ts`.
  - Example: `src/crosshook-native/src/types/index.ts`
- **`snake_case` field names in IPC types**: TypeScript interfaces mirror Rust's serde output exactly — `exit_info`, `failure_mode`, `analyzed_at`.
- **`null` not `undefined` for optional IPC fields**: Rust `Option<T>` serializes as `null`, not `undefined`. TypeScript interfaces use `T | null`.
  - Example: `LaunchPreview.proton_setup: ProtonSetup | null` in `launch.ts:110`

### Frontend State Management

- **`useReducer` + action union**: `useLaunchState` manages complex multi-step state with a typed action union (`LaunchAction`). Adding `diagnosticReport` state follows the same reducer pattern.
  - Example: `src/crosshook-native/src/hooks/useLaunchState.ts:20`
- **`listen<T>('event-name', handler)` pattern**: Tauri event listeners are registered in `useEffect` and cleaned up by returning the unlisten function.
  - Example: `ConsoleView.tsx:65` — `listen<LogPayload>('launch-log', handler)`

## Error Handling

**`ValidationError` → `LaunchValidationIssue` conversion**: Internal error enums implement `message()`, `help()`, and `severity()` methods, then expose `.issue()` to construct the IPC-ready struct. The `DiagnosticReport` should follow this: internal computation produces rich types, converted to IPC struct before emission.

- Example: `request.rs:201` (`ValidationError::issue()`)

**`map_err(|error| error.to_string())` for Tauri commands**: Tauri command functions return `Result<T, String>` — all internal errors are converted to strings at the boundary.

- Example: `commands/launch.rs:57` (`map_err(|error| format!("failed to build Proton game launch: {error}"))`)

**`tracing::warn!` for recoverable stream failures**: When log streaming or process status checks fail non-fatally, use `tracing::warn!` and continue or break — never panic.

- Example: `commands/launch.rs:146,152`

**Exit status capture vs. discard**: The critical integration point is `stream_log_lines()` at `commands/launch.rs:149`:

```rust
Ok(Some(_)) => break,  // ← captures status into binding, triggers analyze()
```

The `_` must become a named binding to pass to `analyze()`.

## Testing Approach

**`#[cfg(test)] mod tests` at file bottom**: All test modules are gated with `#[cfg(test)]` and placed as a submodule at the end of the file. Pure functions are tested directly without integration setup.

- Example: `optimizations.rs:353`, `diagnostics.rs:45`, `preview.rs:604`

**`tempfile::tempdir()` for filesystem fixtures**: Tests requiring real paths use `tempfile::tempdir()` to get an isolated temp directory. Files are created with `fs::write()`.

- Example: `optimizations.rs:381` (`write_executable_file()`), `preview.rs:628` (`fixture()`)

**`ScopedCommandSearchPath` RAII guard**: Tests that affect global process state (command PATH search) use a scoped guard that restores state on drop via `Drop` impl. Ensures test isolation without `setup/teardown` functions.

- Example: `src/crosshook-native/crates/crosshook-core/src/launch/test_support.rs`

**`OnceLock<Mutex<Option<T>>>` for injectable global state in tests**: Test-only global state is stored behind `OnceLock<Mutex<Option<PathBuf>>>` and swapped in/out via `swap_test_command_search_path()`.

- Example: `optimizations.rs:321`

**Table-driven tests for pure functions**: Diagnostic `analyze()` should have one test per signal, per exit code range, and per pattern. Each test is a standalone `#[test]` fn with a descriptive name, not a loop over a data table.

- Example: `optimizations.rs:395` (multiple named tests vs. one parameterized loop)

**WINE log fixtures as inline string literals**: Pattern matching tests pass literal WINE log strings directly. No fixture files needed since patterns use `str::contains()`.

## Patterns to Follow

1. **`FAILURE_PATTERN_DEFINITIONS` catalog** — follow `LAUNCH_OPTIMIZATION_DEFINITIONS` struct layout exactly: `id`, `applies_to_method` (plural: `applies_to_methods: &[&str]`), data slices for `markers`/`suggestions`, and a `severity` field. Iterate in catalog order; first match wins or collect all matches up to the cap. The rule of three for `const &[...]` arrays is already satisfied in this codebase (`LAUNCH_OPTIMIZATION_DEFINITIONS`, `WINE_ENV_VARS_TO_CLEAR`, `SKIP_DIRECTORY_TERMS`) — this is an established, not experimental, pattern.

2. **`ValidationSeverity` reuse** — import directly from `crosshook_core::launch::request::ValidationSeverity`. Do NOT define a new enum. The frontend already renders it with CSS `data-severity` attributes.

3. **Submodule layout** — create `crates/crosshook-core/src/launch/diagnostics/` with `mod.rs`, `models.rs`, `exit_codes.rs`, `patterns.rs`. Add `pub mod diagnostics;` to `launch/mod.rs` and `pub use diagnostics::...` for the public API.

4. **`app.emit()` integration point** — in `stream_log_lines()` at `commands/launch.rs:149`, change:

   ```rust
   Ok(Some(_)) => break,
   ```

   to capture the status, call `analyze()` after the final read, and emit `"launch-diagnostic"` then `"launch-complete"` before returning. The `log_path: PathBuf` is already in scope at the `spawn_log_stream` call site in both `launch_game` and `launch_trainer` — no upstream signature changes needed. The minimal extension to `stream_log_lines` is adding `method: &str` (and optionally `target_kind: &str`) to thread context into `analyze()`.

5. **Tauri event listener in `useLaunchState`** — add `listen<DiagnosticReport>('launch-diagnostic', handler)` in a `useEffect` alongside the existing launch logic. Add `diagnosticReport: DiagnosticReport | null` to `LaunchState` and a new action type.

6. **`LaunchFeedback` discriminant extension** — add `| { kind: 'diagnostic'; report: DiagnosticReport }` to the union. Update `LaunchPanel.tsx` to render the new diagnostic kind in the `crosshook-launch-panel__feedback` container alongside existing validation/runtime kinds. Sort `PatternMatch[]` using the same severity order as `sortIssuesBySeverity()` at `LaunchPanel.tsx:69`: `{ fatal: 0, warning: 1, info: 2 }`.

7. **ISO 8601 timestamp** — use `chrono::Utc::now().to_rfc3339()` for `DiagnosticReport.analyzed_at`, consistent with `LaunchPreview.generated_at` in `preview.rs:337`.

8. **Path sanitization** — implement `sanitize_display_path()` as a standalone pure function in `diagnostics/mod.rs`. Replace `$HOME` prefix with `~` in all string fields before constructing the `DiagnosticReport`.

9. **Security cap constants** — define `MAX_LOG_TAIL_BYTES`, `MAX_DIAGNOSTIC_ENTRIES`, `MAX_LINE_DISPLAY_CHARS` as named `const` items at the top of the relevant file, following the `script_runner.rs` constant style.

10. **`String::from_utf8_lossy()` for log bytes** — use this conversion when reading raw log file bytes, consistent with the existing `tokio::fs::read_to_string` + lossy pattern.

11. **No `regex` in Phase 1** — `regex` is not a direct dependency of `crosshook-core`. Using `str::contains()` is the only option without a `Cargo.toml` change. All 10 initial patterns are fixed literal strings, so this is sufficient. Do not add `regex` unless pattern count grows beyond ~50 or case-insensitive matching is required.

## Other Docs

- `docs/plans/post-launch-failure-diagnostics/research-practices.md` — 11 reusable code modules, KISS assessment, full testability analysis
- `docs/plans/post-launch-failure-diagnostics/research-docs.md` — documentation findings and external references
- `docs/plans/post-launch-failure-diagnostics/feature-spec.md` — authoritative feature spec with data models, API design, phasing, and security requirements
