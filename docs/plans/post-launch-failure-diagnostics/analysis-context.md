# Context Analysis: post-launch-failure-diagnostics

## Executive Summary

This feature adds a post-launch diagnostic layer to CrossHook's launch pipeline: after a child process exits, it captures the exit status, reads the last 2MB of log output via `safe_read_tail()`, runs a pure `analyze()` function that maps exit codes and pattern-matches known WINE/Proton failure signatures, and emits structured `DiagnosticReport` events to the frontend. The entire implementation reuses three proven codebase patterns (`ValidationSeverity`, `LAUNCH_OPTIMIZATION_DEFINITIONS`, `DiagnosticCollector`) and requires zero new crate dependencies for Phase 1.

## Architecture Context

- **System Structure**: New `crosshook-core/src/launch/diagnostics/` submodule (4 files: `mod.rs`, `models.rs`, `exit_codes.rs`, `patterns.rs`) co-located with the launch domain — **pure sync, zero I/O**. `safe_read_tail()` lives in `src-tauri/src/commands/launch.rs`, NOT in `crosshook-core`, because the core crate is I/O-free by design (the CLI's `drain_log()` shows the seek pattern to follow). Frontend state extension in `useLaunchState` + render in `LaunchPanel`.
- **Data Flow**: `stream_log_lines()` poll loop detects child exit → capture `ExitStatus` → `safe_read_tail(log_path, 2MB)` → call pure `diagnostics::analyze(exit_code, signal, log_content, method)` → emit `launch-diagnostic` event (with `DiagnosticReport`) → emit `launch-complete` event (with exit code/signal). Frontend `listen()` in `useLaunchState` updates state → `LaunchPanel` renders diagnostic banner.
- **Integration Points**: `stream_log_lines()` line 149 is the primary surgery site (touched by 3 separate tasks — implementors must coordinate). Exact fix: declare `let mut exit_status: Option<ExitStatus> = None;` before the poll loop, capture it in the `Ok(Some(status)) => { exit_status = Some(status); break }` arm, then post-loop call `analyze()` and emit events. `crosshook-cli/main.rs` line 68-76 already calls `child.wait()` — trivial hook point. `useLaunchState` reducer gets two new action types; `LaunchFeedback` union gains `'diagnostic'` kind. **Note**: `useLaunchState` currently has ZERO Tauri event listeners — adding them is a new pattern for this hook; `ConsoleView.tsx:45-73` is the only reference implementation.

## Critical Files Reference

- `src-tauri/src/commands/launch.rs` (lines 121-172): `stream_log_lines()` — the single most important modification site; exit capture + analyze call + event emit all happen here
- `crates/crosshook-core/src/launch/optimizations.rs` (line 40): `LAUNCH_OPTIMIZATION_DEFINITIONS` — the exact data-driven pattern to replicate for `FAILURE_PATTERN_DEFINITIONS`
- `crates/crosshook-core/src/launch/request.rs` (lines 143-156): `ValidationSeverity` + `LaunchValidationIssue` — import and reuse directly; do NOT create new severity types
- `crates/crosshook-core/src/launch/mod.rs`: Add `pub mod diagnostics;` here + re-export `analyze`, `DiagnosticReport`
- `crates/crosshook-core/src/steam/diagnostics.rs` (lines 1-43): `DiagnosticCollector` with dedup — reference for collector pattern; use for internal deduplication during `analyze()`
- `src/hooks/useLaunchState.ts` (line 20): `useReducer` action union + state — extend with `diagnosticReport` field + new action types
- `src/components/LaunchPanel.tsx` (lines 730-757): Feedback rendering — add diagnostic banner branch here using existing severity badge CSS
- `src/components/ConsoleView.tsx` (lines 45-73): Reference `listen()`/`unlisten` pattern — copy exactly for new event listeners
- `src/types/launch.ts` (line 42): `LaunchFeedback` discriminated union — extend with `| { kind: 'diagnostic'; report: DiagnosticReport }`
- `crates/crosshook-core/src/launch/env.rs` (lines 8-40): `WINE_ENV_VARS_TO_CLEAR` — useful context for diagnostic suggestions about env var presence

## Patterns to Follow

- **Data-Driven Static Catalog**: `const FAILURE_PATTERN_DEFINITIONS: &[FailurePatternDef]` — mirror `LAUNCH_OPTIMIZATION_DEFINITIONS` exactly (id, match criteria, severity, suggestions, applies_to_methods). Each row is independently testable. `optimizations.rs:40`
- **Pure Function Computation**: `analyze(exit_code, signal, core_dumped, log_content, launch_method) -> DiagnosticReport` — zero I/O, zero side effects, fully testable without WINE. Follow `build_launch_preview()` in `preview.rs:272`
- **Submodule with mod.rs Re-exports**: New `diagnostics/` dir exposes flat public API through `mod.rs`. Follow existing `launch/mod.rs` structure.
- **Tauri Event Emission**: `app.emit("launch-diagnostic", payload)` pattern already used for `launch-log`. New events: `launch-diagnostic` (full `DiagnosticReport`) and `launch-complete` (`{code, signal}` for UI state transitions).
- **Discriminated Union for Feedback**: Extend, don't replace. `LaunchFeedback` already has `'validation'` and `'runtime'` kinds — add `'diagnostic'` as a third branch. `types/launch.ts:42`
- **useEffect + unlisten Cleanup**: All Tauri `listen()` calls must return unlisten and be called in the cleanup callback. `ConsoleView.tsx:65` is the reference. **Critical**: the `listen()` effect for diagnostics must use empty deps `[]` — NOT `[method, profileId]` — to avoid tearing down the listener mid-session between launch phases.
- **str::contains() Pattern Matching**: All 10 v1 patterns use fixed literal substrings — no regex dependency. Future: add `regex` crate only when capture groups or case-insensitive matching genuinely needed.

## Cross-Cutting Concerns

- **Security (W1)**: `safe_read_tail(path, 2MB)` is mandatory — must be a seek-based read, not `read_to_string()`. Use `MAX_LOG_TAIL_BYTES: u64 = 2 * 1024 * 1024` constant.
- **Security (W3)**: Cap `DiagnosticReport.pattern_matches` at `MAX_DIAGNOSTIC_ENTRIES = 50`. Frontend reducer must not accumulate unboundedly.
- **Security (W4)**: All paths in user-visible diagnostic strings must go through `sanitize_display_path()` (`$HOME` → `~`). Apply only to diagnostic output, NOT to existing raw ConsoleView log stream.
- **Non-UTF-8 handling**: Use `String::from_utf8_lossy()` on log content — WINE can produce binary bytes in logs. `read_to_string()` will panic/error; byte read + lossy conversion is required.
- **`steam_applaunch` exit code unreliability**: Exit code 0 is ALWAYS returned for this method even on game crash. `analyze_exit_status()` must classify exit code 0 from `steam_applaunch` as `"indeterminate"`. Critically: `scan_log_patterns()` must run regardless of exit code for this method — severity is just downgraded to `info` on exit-0, but patterns still execute.
- **ValidationSeverity reuse is mandatory**: Serializes as `"fatal"/"warning"/"info"` (snake_case) — frontend CSS already maps `data-severity` attributes to badge colors. No new styles needed. Creating a new severity enum would break rendering and violate BR-3.
- **Testing**: `analyze()` must be testable with static string fixtures — no process spawning. Use `const WINE_VIRTUAL_MAP_LOG: &[&str]` style test fixtures. Table-driven tests over `FAILURE_PATTERN_DEFINITIONS` to assert uniqueness and completeness.

## Parallelization Opportunities

- **Phase A (parallel)**: `models.rs` type definitions + `exit_codes.rs` pure function + unit tests can be written in parallel with the `stream_log_lines()` modification in `launch.rs`
- **Phase B (parallel within phase)**: `FAILURE_PATTERN_DEFINITIONS` catalog entries (10 patterns) + `scan_log_patterns()` function + `safe_read_tail()` + `sanitize_display_path()` are all independent; can parallelize pattern authoring with utility function implementation
- **Phase D (parallel)**: TypeScript `diagnostics.ts` types + `useLaunchState` reducer changes + `LaunchPanel` render changes are largely independent after the event shape is agreed upon
- **CLI integration**: `crosshook-cli` wiring is 5 lines and can be done concurrently with any Phase A/B work since the API surface is a pure function call

## Implementation Constraints

- **No new crate dependencies for Phase 1**: `str::contains()` for patterns, `std::os::unix::process::ExitStatusExt` for signals, `chrono` (already present) for timestamps, `serde` (workspace dep) for serialization
- **Phase C deferred**: Crash report collection (`crashreports/` directory scanning) is explicitly Phase 2 — do not implement in this feature. It adds path traversal complexity (W2) requiring `canonicalize()` + `starts_with()` guards.
- **<100ms analysis budget on Steam Deck**: Pure function with substring matching on 2MB tail is well within budget. Do not add synchronous I/O inside `analyze()`.
- **No persistence in Phase 1**: Do not write diagnostic reports to disk (`~/.config/crosshook/diagnostics/`). Phase 1 is in-memory only; history dashboard is future scope (#38).
- **`useLaunchState` feedback is a single slot**: The `feedback` field holds one `LaunchFeedback` at a time, not an array. The diagnostic event arrives asynchronously and may fire while phase is `WaitingForTrainer` or `SessionActive` — reducer must handle this timing without overwriting active state incorrectly. Add a separate `diagnosticReport` field rather than overloading `feedback`.
- **`safe_read_tail()` placement**: Implement in `src-tauri/src/commands/launch.rs` alongside `stream_log_lines()`, NOT in `crosshook-core`. The core crate must remain sync/I/O-free.
- **Phasing order is A→B→D** (not alphabetical): Exit code capture (A) is the foundation; pattern engine (B) depends on exit status types; frontend (D) depends on the event shape from B.
- **High-detectability patterns must have zero known false positives before ship**: Gate on non-zero exit for all methods except `steam_applaunch`. Downgrade severity to `info` when exit code is 0 regardless of pattern match.

## Key Recommendations

- **Start with the 10-line quick win**: Change `Ok(Some(_)) => break` to capture `ExitStatus` and emit `launch-complete`. This delivers immediate value with zero new modules.
- **Implement `safe_read_tail()` before wiring `analyze()`**: It's a security requirement (W1), not optional. Implement it as the first step of Phase B.
- **Use `ExitStatusExt::signal()` separately from `code()`**: `code()` returns `None` on signal-kill; `signal()` returns the signal number. Both are needed for complete `ExitCodeInfo`.
- **Copy `ConsoleView.tsx:65` listener pattern verbatim**: The `useEffect` cleanup with `unlisten` is subtle. Don't reinvent it.
- **Inline suggestions in pattern definitions for v1**: No `suggestions.rs` module needed until patterns exceed 10 or suggestions need runtime context. Keep it in the `FailurePatternDef` struct.
- **Task breakdown should follow A→B→D phases**: Phase A is a single PR (small, low risk). Phase B is the most complex (patterns, safe_read_tail, sanitize, wire into launch.rs). Phase D is frontend-only (can be parallelized across types/state/render). Keep phases as separate PRs to enable early testing.
