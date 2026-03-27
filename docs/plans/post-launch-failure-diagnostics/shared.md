# Post-Launch Failure Diagnostics

CrossHook's launch pipeline spawns game/trainer child processes via `tokio::process::Child`, streams their log output over Tauri events, and currently discards the process `ExitStatus` at `stream_log_lines()` line 149 (`Ok(Some(_)) => break`). This feature adds a new `crosshook-core::launch::diagnostics` submodule containing a pure `analyze()` function that receives captured exit status, log tail, and launch method, then returns a `DiagnosticReport` emitted as a `launch-diagnostic` Tauri event. The frontend hooks into this via `listen()` in `useLaunchState` and renders diagnostic findings in the existing `LaunchPanel` feedback area using the same severity badge pattern already used for validation issues. No new dependencies are required — all analysis uses `str::contains()` pattern matching against a `FAILURE_PATTERN_DEFINITIONS` static catalog modeled on the existing `LAUNCH_OPTIMIZATION_DEFINITIONS`.

## Relevant Files

### Rust Backend — Primary Modification Targets

- `src/crosshook-native/src-tauri/src/commands/launch.rs`: **Primary integration point** — `stream_log_lines()` (lines 121-172) where exit status capture, `safe_read_tail()`, `analyze()` call, and event emission are added
- `src/crosshook-native/crates/crosshook-core/src/launch/mod.rs`: Module root — add `pub mod diagnostics;` and re-export `analyze`, `DiagnosticReport`
- `src/crosshook-native/crates/crosshook-core/src/launch/request.rs`: Defines `ValidationSeverity` (line 143), `LaunchValidationIssue` (line 151), method constants — all reused directly
- `src/crosshook-native/crates/crosshook-core/src/launch/optimizations.rs`: `LAUNCH_OPTIMIZATION_DEFINITIONS` (line 40) — the exact data-driven catalog pattern to replicate for `FAILURE_PATTERN_DEFINITIONS`
- `src/crosshook-native/crates/crosshook-core/src/launch/runtime_helpers.rs`: `attach_log_stdio()` — both stdout/stderr go to same log file (append mode)
- `src/crosshook-native/crates/crosshook-core/src/launch/script_runner.rs`: Command builders for each launch method; env var injection patterns and `const` naming convention
- `src/crosshook-native/crates/crosshook-core/src/steam/diagnostics.rs`: `DiagnosticCollector` — collector + dedup pattern to inform new diagnostics module design
- `src/crosshook-native/src-tauri/src/commands/shared.rs`: `create_log_path()` — log file naming at `/tmp/crosshook-logs/{prefix}-{slug}-{ms}.log`
- `src/crosshook-native/crates/crosshook-cli/src/main.rs`: CLI `launch_profile()` at line 68-76 — uses `child.wait()`, the CLI hook for `analyze()` integration

### React Frontend — Modification Targets

- `src/crosshook-native/src/hooks/useLaunchState.ts`: State machine (reducer pattern) — add `diagnosticReport` state, `listen('launch-diagnostic')` and `listen('launch-complete')` event listeners
- `src/crosshook-native/src/components/LaunchPanel.tsx`: Feedback rendering in `crosshook-launch-panel__feedback` (lines 730-757) — add diagnostic banner branch using existing severity badge CSS
- `src/crosshook-native/src/components/ConsoleView.tsx`: `listen()` event pattern (lines 45-73) — reference implementation for wiring Tauri event listeners
- `src/crosshook-native/src/types/launch.ts`: `LaunchFeedback` union (line 42), `LaunchValidationSeverity` (line 34), `LaunchPhase` enum — extend with diagnostic kind
- `src/crosshook-native/src/types/index.ts`: Barrel re-export — add `export * from './diagnostics'`

### New Files to Create

- `src/crosshook-native/crates/crosshook-core/src/launch/diagnostics/mod.rs`: Public `analyze()` API and re-exports
- `src/crosshook-native/crates/crosshook-core/src/launch/diagnostics/models.rs`: `DiagnosticReport`, `ExitCodeInfo`, `FailureMode`, `PatternMatch`, `ActionableSuggestion`
- `src/crosshook-native/crates/crosshook-core/src/launch/diagnostics/exit_codes.rs`: `analyze_exit_status()` pure function, signal name mapping
- `src/crosshook-native/crates/crosshook-core/src/launch/diagnostics/patterns.rs`: `FAILURE_PATTERN_DEFINITIONS` catalog + `scan_log_patterns()`
- `src/crosshook-native/src/types/diagnostics.ts`: TypeScript IPC types mirroring Rust structs

## Relevant Patterns

**Data-Driven Catalog with Static Slice**: Business logic encoded as `const` slice of definition structs — no dynamic allocation. See [src/crosshook-native/crates/crosshook-core/src/launch/optimizations.rs:40](src/crosshook-native/crates/crosshook-core/src/launch/optimizations.rs) for `LAUNCH_OPTIMIZATION_DEFINITIONS`. The new `FAILURE_PATTERN_DEFINITIONS` follows this exactly.

**Pure Function Computation**: All non-trivial logic lives in pure `fn` taking inputs and returning a value — no I/O, no global state. See [src/crosshook-native/crates/crosshook-core/src/launch/preview.rs:272](src/crosshook-native/crates/crosshook-core/src/launch/preview.rs) for `build_launch_preview()`. The `analyze()` function must follow this pattern.

**Submodule with mod.rs Re-exports**: Each submodule exposes a flat public API through `mod.rs`, hiding internal layout. See [src/crosshook-native/crates/crosshook-core/src/launch/mod.rs](src/crosshook-native/crates/crosshook-core/src/launch/mod.rs).

**Tauri Event Emission**: Long-running async tasks emit events via `app.emit("event-name", payload)`. See [src/crosshook-native/src-tauri/src/commands/launch.rs:135](src/crosshook-native/src-tauri/src/commands/launch.rs) for `launch-log` event. New `launch-diagnostic` and `launch-complete` events follow this pattern.

**Discriminated Union for Feedback**: `LaunchFeedback` uses `kind` as discriminant (`'validation' | 'runtime'`). See [src/crosshook-native/src/types/launch.ts:42](src/crosshook-native/src/types/launch.ts). Extend with `| { kind: 'diagnostic'; report: DiagnosticReport }`.

**useReducer + Action Union**: `useLaunchState` uses typed action union for multi-step state. See [src/crosshook-native/src/hooks/useLaunchState.ts:20](src/crosshook-native/src/hooks/useLaunchState.ts). Add new action types for diagnostic events.

**Tauri listen() in useEffect**: Event listeners registered in `useEffect` with cleanup via unlisten. See [src/crosshook-native/src/components/ConsoleView.tsx:65](src/crosshook-native/src/components/ConsoleView.tsx) for the pattern.

**ValidationSeverity Reuse**: Import `ValidationSeverity` from `crosshook_core::launch::request` — do not create new severity enum. Frontend already renders it with `data-severity` CSS attributes.

## Relevant Docs

**docs/plans/post-launch-failure-diagnostics/feature-spec.md**: You _must_ read this before any implementation — contains all business rules (BR-1 through BR-11), architecture diagram, data models, API design, phasing strategy (A/B/D/C), and complete task breakdown.

**docs/features/steam-proton-trainer-launch.doc.md**: You _must_ read this when working on launch method differences — covers all three launch methods, `steam_applaunch` exit code unreliability, console view, and log file locations.

**docs/plans/post-launch-failure-diagnostics/research-technical.md**: You _must_ read this for full Rust type definitions, TypeScript types, Tauri event design, and step-by-step `stream_log_lines()` integration walkthrough.

**docs/plans/post-launch-failure-diagnostics/research-practices.md**: You _must_ read this for reusable code identification, KISS assessment, `str::contains()` vs `regex` decision, and testability patterns.

**docs/plans/post-launch-failure-diagnostics/research-security.md**: You _must_ read this before shipping — 4 WARNING findings: `safe_read_tail()` bounded reads, `sanitize_display_path()`, frontend state cap (50 entries), non-UTF-8 handling.

**docs/plans/post-launch-failure-diagnostics/research-recommendations.md**: Reference for 20 consensus decisions across architecture, patterns, security, and UX dimensions.

**docs/plans/post-launch-failure-diagnostics/research-ux.md**: Reference for progressive disclosure design (summary/details/raw log), Steam Deck layout constraints (48px touch targets).

**CLAUDE.md**: Reference for project conventions, commit hygiene, module layout, and code quality standards.
