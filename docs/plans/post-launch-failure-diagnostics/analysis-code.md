# Post-Launch Failure Diagnostics — Code Analysis

## Executive Summary

The feature integrates at a single async function (`stream_log_lines`), captures the discarded `ExitStatus` from `child.try_wait()`, reads the log tail, and calls a new pure `analyze()` function that returns a `DiagnosticReport` emitted as a Tauri event. The frontend wires it into the existing `useReducer` state machine and renders it in the existing feedback area using the same `data-severity` CSS pattern already in place.

---

## Existing Code Structure

### Rust Backend

- `src-tauri/src/commands/launch.rs` — 206 lines. Two async Tauri commands (`launch_game`, `launch_trainer`), both spawning via `spawn_log_stream()` → `stream_log_lines()`. The entire feature integrates here.
- `crates/crosshook-core/src/launch/mod.rs` — Module root with explicit `pub use` re-exports; 26 lines.
- `crates/crosshook-core/src/launch/request.rs` — `ValidationSeverity`, `LaunchValidationIssue`, `ValidationError`, method constants. Key types to reuse.
- `crates/crosshook-core/src/launch/optimizations.rs` — `LAUNCH_OPTIMIZATION_DEFINITIONS` catalog pattern. 493 lines including tests; the **exact pattern** to replicate for `FAILURE_PATTERN_DEFINITIONS`.
- `crates/crosshook-core/src/steam/diagnostics.rs` — `DiagnosticCollector` with dedup-preserving-order pattern. 65 lines.
- `src-tauri/src/commands/shared.rs` — `create_log_path()` — log path at `/tmp/crosshook-logs/{prefix}-{slug}-{ms}.log`.
- `crates/crosshook-cli/src/main.rs` (lines 60–76) — already calls `child.wait()` and checks `status.success()`; secondary integration hook.
- `crates/crosshook-core/src/launch/runtime_helpers.rs` — `attach_log_stdio()` routes both stdout/stderr to same log file in append mode.

### React Frontend

- `src/hooks/useLaunchState.ts` — 304 lines. `useReducer` state machine managing `LaunchPhase`, `LaunchFeedback | null`, `helperLogPath`. No Tauri event listeners yet (only `invoke()` calls).
- `src/components/LaunchPanel.tsx` — Feedback area at lines 683–757. Discriminates `feedback.kind` to render validation vs runtime branches with `data-severity` badge.
- `src/components/ConsoleView.tsx` — Reference `useEffect` + `listen()` pattern with `active` guard and Promise-based `unlisten()` cleanup (lines 45–73).
- `src/types/launch.ts` — `LaunchFeedback` discriminated union (line 42), `LaunchValidationSeverity` string union (line 34), `LaunchPhase` enum (line 4).
- `src/types/index.ts` — Pure barrel re-export: `export * from './...'`.

---

## Implementation Patterns

### Pattern 1: Data-Driven Catalog with `const` Static Slice

`optimizations.rs:31–177` defines the exact structure to replicate:

```rust
// PRIVATE struct — never pub
struct LaunchOptimizationDefinition {
    id: &'static str,
    applies_to_method: &'static str,
    env: &'static [(&'static str, &'static str)],
    // ... all 'static fields — zero runtime allocation
}

const LAUNCH_OPTIMIZATION_DEFINITIONS: &[LaunchOptimizationDefinition] = &[
    LaunchOptimizationDefinition { id: "disable_steam_input", ... },
    // ...
];
```

For `FAILURE_PATTERN_DEFINITIONS`, replicate as:

```rust
struct FailurePatternDefinition {
    id: &'static str,
    pattern: &'static str,           // substring matched via str::contains()
    severity: ValidationSeverity,
    summary: &'static str,
    suggestion: &'static str,
    applies_to_methods: &'static [&'static str],  // empty = all methods
}

const FAILURE_PATTERN_DEFINITIONS: &[FailurePatternDefinition] = &[ ... ];
```

### Pattern 2: Pure Function Computation

`preview.rs:272` — `build_launch_preview()` takes inputs, returns a value, no I/O. The `analyze()` function must follow this exactly:

```rust
pub fn analyze(
    exit_status: Option<std::process::ExitStatus>,
    log_tail: &str,
    method: &str,
) -> DiagnosticReport {
    // pure: no I/O, no global mutable state, deterministic
}
```

### Pattern 3: Submodule with `mod.rs` Re-exports

`launch/mod.rs:1–26` — pattern for the new diagnostics submodule:

```rust
// In launch/mod.rs — ADD:
pub mod diagnostics;
pub use diagnostics::{analyze, DiagnosticReport};

// In launch/diagnostics/mod.rs:
pub mod exit_codes;
pub mod models;
pub mod patterns;
pub use models::DiagnosticReport;
pub fn analyze(...) -> DiagnosticReport { ... }
```

### Pattern 4: Tauri Event Emission in Async Task

`launch.rs:135`:

```rust
app.emit("launch-log", line.to_string())  // existing pattern
// New events follow exactly:
app.emit("launch-diagnostic", &report)    // DiagnosticReport must derive Serialize
app.emit("launch-complete", ())           // unit payload
```

### Pattern 5: `ExitStatus` Capture in `stream_log_lines()`

Current code at `launch.rs:149–150`:

```rust
match child.try_wait() {
    Ok(Some(_)) => break,       // ← discard here
```

Modification required — declare before loop, capture in arm:

```rust
let mut exit_status: Option<std::process::ExitStatus> = None;
// ... inside loop:
match child.try_wait() {
    Ok(Some(status)) => { exit_status = Some(status); break }
```

Then after the final read block (after line 171):

```rust
let log_tail = safe_read_tail(&log_path, 8 * 1024).await;
let report = analyze(exit_status, &log_tail, &method);
let _ = app.emit("launch-diagnostic", &report);
let _ = app.emit("launch-complete", ());
```

### Pattern 6: `ValidationSeverity` Reuse

`request.rs:143–149` — already `Serialize`/`Deserialize` with `serde(rename_all = "snake_case")`:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ValidationSeverity { Fatal, Warning, Info }
```

Serializes as `"fatal"`, `"warning"`, `"info"` — exactly matching `LaunchValidationSeverity` in TypeScript. **Import this directly; do not create a new severity enum.**

### Pattern 7: `DiagnosticCollector` Dedup Pattern

`steam/diagnostics.rs:32–43` — `dedupe_preserving_order()` via `HashSet` + ordered `Vec`:

```rust
fn dedupe_preserving_order(values: Vec<String>) -> Vec<String> {
    let mut seen = HashSet::new();
    values.into_iter().filter(|v| seen.insert(v.clone())).collect()
}
```

Use this inside `scan_log_patterns()` to deduplicate matched patterns before returning.

### Pattern 8: `useReducer` Action Union Extension

`useLaunchState.ts:20–26` — typed action union:

```typescript
type LaunchAction =
  | { type: 'reset' }
  | { type: 'game-start' }
  // ...
  | { type: 'failure'; fallbackPhase: LaunchPhase; feedback: LaunchFeedback };
```

Add two new actions:

```typescript
  | { type: "diagnostic-received"; report: DiagnosticReport }
  | { type: "launch-complete" }
```

### Pattern 9: `listen()` in `useEffect` with `active` Guard

`ConsoleView.tsx:45–73` — canonical Tauri event listener pattern:

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
}, []); // empty deps — lifecycle of component, not profile/method
```

**Do not add `method` or `profileId` to the deps array.** These listeners must persist for the component lifetime; the `reset` action on profile/method change already clears feedback state.

### Pattern 10: Feedback Discriminant in `LaunchPanel.tsx`

`LaunchPanel.tsx:683–685`:

```typescript
const validationFeedback = feedback?.kind === 'validation' ? feedback.issue : null;
const runtimeFeedback = feedback?.kind === 'runtime' ? feedback.message : null;
// ADD:
const diagnosticFeedback = feedback?.kind === 'diagnostic' ? feedback.report : null;
```

The existing `data-severity` CSS attribute at line 734 is driven by `feedbackSeverity`. For diagnostic feedback, set `feedbackSeverity` from `diagnosticFeedback.severity` using the same `?? 'fatal'` fallback pattern.

---

## Integration Points

### Files to Modify

| File                                      | Change                                                                                                                                              |
| ----------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------- |
| `src-tauri/src/commands/launch.rs`        | Capture `ExitStatus` in `stream_log_lines()`; add `safe_read_tail()`, `analyze()`, and event emission after final read; pass `method` into function |
| `crates/crosshook-core/src/launch/mod.rs` | Add `pub mod diagnostics;` and `pub use` re-exports                                                                                                 |
| `src/hooks/useLaunchState.ts`             | Add `diagnosticReport` state field, two new action types, `useEffect` for `listen('launch-diagnostic')` and `listen('launch-complete')`             |
| `src/components/LaunchPanel.tsx`          | Add `diagnosticFeedback` branch in feedback rendering block (lines 683–757)                                                                         |
| `src/types/launch.ts`                     | Extend `LaunchFeedback` union with `                                                                                                                | { kind: 'diagnostic'; report: DiagnosticReport }` |
| `src/types/index.ts`                      | Add `export * from './diagnostics'`                                                                                                                 |
| `crates/crosshook-cli/src/main.rs`        | After `child.wait()` at line 70, call `analyze()` and print findings to stderr                                                                      |

### Files to Create

| File                                                         | Purpose                                                                                                                                           |
| ------------------------------------------------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------- |
| `crates/crosshook-core/src/launch/diagnostics/mod.rs`        | Public `analyze()` API, `pub mod` declarations, `pub use` re-exports                                                                              |
| `crates/crosshook-core/src/launch/diagnostics/models.rs`     | `DiagnosticReport`, `ExitCodeInfo`, `FailureMode`, `PatternMatch`, `ActionableSuggestion` — all `#[derive(Debug, Clone, Serialize, Deserialize)]` |
| `crates/crosshook-core/src/launch/diagnostics/exit_codes.rs` | `analyze_exit_status(status: ExitStatus, method: &str) -> ExitCodeInfo`; signal name mapping                                                      |
| `crates/crosshook-core/src/launch/diagnostics/patterns.rs`   | `FAILURE_PATTERN_DEFINITIONS` catalog + `scan_log_patterns(log_tail: &str) -> Vec<PatternMatch>`                                                  |
| `src/types/diagnostics.ts`                                   | TypeScript IPC mirrors of all Rust models                                                                                                         |

---

## Code Conventions

### Rust Conventions (from codebase)

- Struct fields: all `pub` when crossing IPC boundary; private when internal-only (`LaunchOptimizationDefinition`)
- Catalog structs: `'static` lifetime on all string fields, never `String`
- Error propagation: `Result<T, E>` with `anyhow` at call sites; pure functions return values directly (no `Result` unless fallible)
- Logging: `tracing::warn!(%error, "message")` for recoverable errors; `tracing::error!` for non-recoverable
- Test infrastructure: use `#[cfg(test)]` module at bottom of file; `tempfile::tempdir()` for filesystem tests
- Serde on IPC types: always `#[derive(Serialize, Deserialize)]`; use `#[serde(rename_all = "snake_case")]` on enums

### TypeScript Conventions (from codebase)

- Type guards: explicit `isXxx(value: unknown): value is Xxx` functions (see `isLaunchValidationIssue` at `launch.ts:46`)
- Discriminated unions: `kind` field as string literal discriminant
- Hook return shape: flat object with named functions and state fields (no class instances)
- Severity strings: lowercase `'fatal' | 'warning' | 'info'` matching Rust serde output

---

## Dependencies and Services

### Rust — No New Crates Required

All analysis uses:

- `std::process::ExitStatus` — already in scope via `tokio::process::Child`
- `str::contains()` — no regex crate needed
- `crosshook_core::launch::request::ValidationSeverity` — import from sibling module

### TypeScript — No New Packages

Uses existing:

- `@tauri-apps/api/event` → `listen` (already imported in `ConsoleView.tsx`)
- React `useEffect`, `useReducer` — already in `useLaunchState.ts`

---

## Gotchas and Warnings

1. **`Ok(Some(_))` discards `ExitStatus` — this is the core bug** (`launch.rs:150`). The variable must be declared as `let mut exit_status: Option<std::process::ExitStatus> = None;` before the loop and populated in the match arm.

2. **`steam_applaunch` exit code is always 0** — the helper script exits 0 even when the game crashes inside Steam. `analyze_exit_status()` must check for `method == METHOD_STEAM_APPLAUNCH` and mark the `ExitCodeInfo` as "indeterminate" rather than "clean exit" when status is 0. Pattern matching on the log tail is the primary signal for this method.

3. **`stream_log_lines()` currently has no `method` parameter** — the function signature is `(app, log_path, child)`. To pass the method through, either add a `method: String` parameter or embed it in a struct. The callers `spawn_log_stream()` → `stream_log_lines()` chain must be updated consistently.

4. **Non-UTF-8 bytes in log files** — `tokio::fs::read_to_string()` fails on non-UTF-8. The `safe_read_tail()` function must use `tokio::fs::read()` + `String::from_utf8_lossy()` to handle lossy decoding. Never call `read_to_string()` on the tail.

5. **`safe_read_tail()` must be bounded** — read from end of file, at most 8KB (8192 bytes), to avoid memory DoS from huge log files. Read the raw bytes, then `String::from_utf8_lossy()` the slice.

6. **Emit order matters** — emit `launch-diagnostic` before `launch-complete`. The frontend's `launch-complete` handler may transition state that ignores subsequent events. Ordering: `analyze()` → `emit("launch-diagnostic")` → `emit("launch-complete")`.

7. **`listen()` effect deps must be `[]`** — the diagnostic listeners must live for the full component lifetime, not reset on profile/method changes. The existing `useEffect(() => dispatch({ type: "reset" }), [method, profileId])` already clears `feedback` state. Separate concerns cleanly.

8. **`app.emit()` ignores errors silently** — existing code does `if let Err(error) = app.emit(...)` with `tracing::warn!`. Follow the same pattern for `launch-diagnostic` and `launch-complete` events.

9. **`DiagnosticReport` must be `Serialize`** — `app.emit("launch-diagnostic", &report)` requires `report` to implement `serde::Serialize`. All nested types (`ExitCodeInfo`, `PatternMatch`, etc.) must also derive `Serialize`.

10. **Frontend state cap** — per `research-security.md`: cap accumulated diagnostic entries at 50 to prevent unbounded UI state growth. If `useLaunchState` accumulates multiple diagnostics over time, apply this cap in the reducer.

---

## Task-Specific Guidance

### Phase A — Rust Core (New `diagnostics` Module)

Build in this order to enable testing each part independently:

1. `models.rs` first — defines all types, no logic
2. `exit_codes.rs` — pure function, testable without file I/O
3. `patterns.rs` — `FAILURE_PATTERN_DEFINITIONS` catalog + `scan_log_patterns()`; test with literal log strings
4. `diagnostics/mod.rs` — wires `analyze()` from parts 2 and 3
5. `launch/mod.rs` — add `pub mod diagnostics;` and re-exports

### Phase B — `stream_log_lines()` Integration

Exact changes to `launch.rs`:

- Add `method: String` parameter to `stream_log_lines()` and `spawn_log_stream()`
- Update callers in `launch_game()` and `launch_trainer()` to pass `request.resolved_method().to_string()`
- Declare `let mut exit_status: Option<std::process::ExitStatus> = None;` before loop
- Change `Ok(Some(_)) => break` → `Ok(Some(status)) => { exit_status = Some(status); break }`
- After the final read block: implement `safe_read_tail()` (new helper in `launch.rs` or `commands/shared.rs`), call `analyze()`, emit both events

### Phase D — TypeScript Types

- Create `src/types/diagnostics.ts` mirroring all Rust structs
- Extend `LaunchFeedback` in `launch.ts`
- Add barrel export in `index.ts`

### Phase C — Frontend Wiring

- Extend `useLaunchState.ts` state type and reducer
- Add `useEffect` listener block (empty deps, `active` guard, Promise cleanup)
- Add `diagnosticFeedback` branch in `LaunchPanel.tsx` feedback rendering
- Render `DiagnosticReport` using existing severity badge CSS; use progressive disclosure (summary → details on expand)
