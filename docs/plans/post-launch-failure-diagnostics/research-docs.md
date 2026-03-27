# Documentation Research: post-launch-failure-diagnostics

**Issue**: #36
**Generated**: 2026-03-27

---

## Overview

CrossHook has a well-documented launch pipeline (feature doc, feature spec, prior research reports, and inline code comments) covering Steam/Proton launch methods, log streaming, and the validation severity model from #39. The primary integration point — `stream_log_lines()` in `src-tauri/src/commands/launch.rs:150` — is clearly identified in multiple research docs. The frontend severity badge pattern from `LaunchPanel.tsx` and the data-driven catalog pattern from `optimizations.rs` are both documented as reuse targets.

---

## Architecture Docs

| Document                                                                 | Type                  | Relevance                                                                                                                                                                 |
| ------------------------------------------------------------------------ | --------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `docs/plans/post-launch-failure-diagnostics/research-architecture.md`    | Architecture research | **Critical** — System overview, component map, data flow, integration points, and dependencies (written by architecture-researcher)                                       |
| `docs/plans/post-launch-failure-diagnostics/research-technical.md`       | Architecture spec     | **Critical** — Full component diagram, Rust type definitions, Tauri event design, `stream_log_lines()` integration walkthrough                                            |
| `docs/plans/post-launch-failure-diagnostics/feature-spec.md`             | Feature spec          | **Critical** — Executive summary, architecture overview diagram, data models, API design, phasing strategy                                                                |
| `docs/plans/post-launch-failure-diagnostics/research-recommendations.md` | Synthesis doc         | **Critical** — 20 consensus decisions, cross-team synthesis of architecture, patterns, security, and UX                                                                   |
| `docs/features/steam-proton-trainer-launch.doc.md`                       | Feature guide         | **Required** — Covers all three launch methods, their behavioral differences (especially `steam_applaunch` exit code unreliability), console view, and log file locations |
| `CLAUDE.md` (project)                                                    | Project guidelines    | **Required** — Architecture diagram, module layout (`crosshook-core`, `src-tauri`, frontend), code conventions, commit hygiene                                            |

---

## Feature Specs and Prior Research

All files are under `docs/plans/post-launch-failure-diagnostics/`:

| File                          | Summary                                                                                                                                                                                                                     |
| ----------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `feature-spec.md`             | Complete feature spec: business rules (BR-1 through BR-11), edge cases, success criteria, architecture overview, data models, API design, phasing (A/B/D/C), task breakdown                                                 |
| `research-technical.md`       | Architecture detail: full Rust type definitions, TypeScript types, Tauri event design, `stream_log_lines()` modification, integration with `useLaunchState`, `DiagnosticCollector`                                          |
| `research-practices.md`       | 11 reusable code modules identified; KISS assessment; `str::contains()` vs `regex` decision; testability patterns (pure function, static fixtures, table-driven); `LAUNCH_OPTIMIZATION_DEFINITIONS` pattern rationale       |
| `research-security.md`        | 0 CRITICAL, 4 WARNING, 5 ADVISORY findings: bounded reads (`safe_read_tail()`), path sanitization (`sanitize_display_path()`), path traversal via `canonicalize()`, frontend state cap (50 entries), non-UTF-8 log handling |
| `research-business.md`        | 3 user stories; 12 business rules; 12 failure pattern definitions with detectability tiers (HIGH/MEDIUM/LOW); edge cases for all three launch methods                                                                       |
| `research-ux.md`              | Competitive analysis (Lutris/Bottles/Heroic all have silent failure anti-pattern); progressive disclosure design (summary → details → raw log); Steam Deck layout constraints (48px touch targets, 280px drawer)            |
| `research-external.md`        | `ExitStatusExt` API reference (`signal()`, `core_dumped()`, `code()`); Proton env vars and log locations; `regex` crate evaluation; 9 Unix signal codes with exit code mapping                                              |
| `research-recommendations.md` | 20 consensus decisions across all research dimensions; risk assessment; phasing strategy (A → B → D → C)                                                                                                                    |

---

## API / IPC Docs

### Tauri Command Layer (`src-tauri/src/commands/launch.rs`)

- **`launch_game()`** (line 44): Validates request, creates log path via `create_log_path()`, spawns child, calls `spawn_log_stream()`. Returns `LaunchResult { succeeded, message, helper_log_path }`.
- **`launch_trainer()`** (line 76): Same pattern as `launch_game()` for trainer process.
- **`spawn_log_stream()`** (line 109): Fire-and-forget async spawn that calls `stream_log_lines()`.
- **`stream_log_lines()`** (lines 121–172): **Primary integration point.** Polls log file every 500ms, emits `launch-log` events for each new line, then calls `child.try_wait()` at line 149. Currently discards exit status at line 150 (`Ok(Some(_)) => break`). The diagnostic hook goes here — capture the `ExitStatus`, call `safe_read_tail()`, call `diagnostics::analyze()`, emit `launch-diagnostic` and `launch-complete` events.

### Existing Tauri Events

| Event                          | Payload                        | Source                                    |
| ------------------------------ | ------------------------------ | ----------------------------------------- |
| `launch-log`                   | `String` (log line)            | `stream_log_lines()` in `launch.rs:135`   |
| (proposed) `launch-diagnostic` | `DiagnosticReport`             | After child exit, in `stream_log_lines()` |
| (proposed) `launch-complete`   | `{ code: i32?, signal: i32? }` | After any child exit                      |

### Frontend TypeScript Types (`src/types/launch.ts`)

- **`LaunchValidationSeverity`** (line 34): `'fatal' | 'warning' | 'info'` — reuse directly for diagnostic severity
- **`LaunchValidationIssue`** (line 36): `{ message, help, severity }` — the shape diagnostic findings should mirror
- **`LaunchFeedback`** (line 42): Discriminated union `validation | runtime` — extend with `| { kind: 'diagnostic'; report: DiagnosticReport }`
- **`LaunchPhase`** enum (line 4): Tracks current launch state (`Idle`, `GameLaunching`, etc.) — `useLaunchState` should add `diagnosticReport` alongside this
- **`LaunchResult`** (line 62): `{ succeeded, message, helper_log_path }` — note `succeeded` reflects spawn success, not execution success

### Frontend Hook (`src/hooks/useLaunchState.ts`)

- State: `{ phase: LaunchPhase, feedback: LaunchFeedback | null, helperLogPath: string | null }` — add `diagnosticReport: DiagnosticReport | null`
- Actions: `reset | game-start | game-success | trainer-start | trainer-success | failure` — add `diagnostic-received`
- Currently does not listen for any Tauri events (event listeners are wired elsewhere). The `launch-diagnostic` listener goes in the `useEffect` here.

---

## Development Guides

### Core Rust Patterns (crosshook-core)

#### `ValidationSeverity` — The severity enum to reuse

- **Location**: `crates/crosshook-core/src/launch/request.rs:143–149`
- **Type**: `enum ValidationSeverity { Fatal, Warning, Info }` with `#[serde(rename_all = "snake_case")]`
- **Reuse**: Import directly for `ExitCodeInfo.severity` and `PatternMatch.severity` — do not create a new severity enum

#### `LaunchValidationIssue` — The structured error shape to mirror

- **Location**: `crates/crosshook-core/src/launch/request.rs:151–156`
- **Shape**: `{ message: String, help: String, severity: ValidationSeverity }`
- **Reuse**: `DiagnosticFinding` should have the same serialization shape so the frontend can render it with the same component

#### `LAUNCH_OPTIMIZATION_DEFINITIONS` — The data-driven table pattern

- **Location**: `crates/crosshook-core/src/launch/optimizations.rs:40–177`
- **Pattern**: `const LAUNCH_OPTIMIZATION_DEFINITIONS: &[LaunchOptimizationDefinition] = &[...]`
- **Reuse**: `FAILURE_PATTERN_DEFINITIONS` should follow the identical pattern — `const` array of structs with `id`, match criteria, severity, suggestion, `applies_to_methods`

#### `DiagnosticCollector` — Deduplication utility

- **Location**: `crates/crosshook-core/src/steam/diagnostics.rs:1–44`
- **API**: `add_diagnostic()`, `add_hint()`, `finalize() -> (Vec<String>, Vec<String>)`
- **Note**: Currently untyped (strings only). The new `launch/diagnostics/` module may use it for deduplication or create a typed parallel.

#### `LaunchOptimizationDefinition` struct — Struct shape to copy

- **Location**: `crates/crosshook-core/src/launch/optimizations.rs:31–38`
- Fields: `id`, `applies_to_method`, `env`, `wrappers`, `conflicts_with`, `required_binary`
- **Model**: `FailurePatternDef` should follow the same `&'static str` field approach

### Module Layout Convention

New module goes at: `crates/crosshook-core/src/launch/diagnostics/`

```
launch/
  diagnostics/
    mod.rs         ← Public API: analyze(), re-exports
    models.rs      ← DiagnosticReport, ExitCodeInfo, PatternMatch, ActionableSuggestion, FailureMode
    exit_codes.rs  ← analyze_exit_status() pure function, signal name mapping
    patterns.rs    ← FAILURE_PATTERN_DEFINITIONS const array, scan_log_patterns()
```

Register with: `pub mod diagnostics;` in `crates/crosshook-core/src/launch/mod.rs`

### Testing Patterns

From `research-practices.md` and observed in codebase:

1. **Pure function design**: `analyze(exit_code, signal, log_content, method) -> DiagnosticReport` — no I/O, no WINE/Proton needed
2. **Static log fixtures**: `const WINE_NTDLL_LOG: &str = "...ntdll.dll not found..."` for pattern tests
3. **Table-driven tests**: Iterate `FAILURE_PATTERN_DEFINITIONS` to assert non-empty `id`, `markers`, `suggestion`
4. **Exit code edge cases**: Test 0, 1, 134 (SIGABRT), 137 (SIGKILL), 139 (SIGSEGV), 143 (SIGTERM), 127, 126, unknown positive, `signal=None`
5. **Pattern from `env.rs:79–119`**: Tests that assert on constant arrays — follow this for `FAILURE_PATTERN_DEFINITIONS`

---

## Configuration References

### Cargo.toml (crosshook-core)

Key workspace dependencies already available (no new deps needed for v1):

| Dep          | Version   | Use in diagnostics                     |
| ------------ | --------- | -------------------------------------- |
| `serde`      | workspace | Serialize/Deserialize on all IPC types |
| `serde_json` | workspace | JSON for Tauri events                  |
| `tokio`      | workspace | `tokio::io` for `safe_read_tail()`     |
| `chrono`     | 0.4.x     | `analyzed_at` ISO 8601 timestamp       |
| `tracing`    | workspace | Structured logging                     |

**`regex` is NOT a direct dependency** of `crosshook-core` — it appears in `Cargo.lock` transitively via `tracing-subscriber`. For v1 patterns, use `str::contains()`.

**`std::os::unix::process::ExitStatusExt`** provides `signal()`, `core_dumped()`, `code()` — no external crate needed.

### tauri.conf.json

- No changes needed. Diagnostics use Rust backend commands and Tauri events, not the filesystem Tauri plugin.
- Current capabilities (`core:default`, `dialog:default`) are sufficient.

### Security Constants (must implement)

```rust
const MAX_LOG_TAIL_BYTES: u64 = 2 * 1024 * 1024;  // W1: 2 MiB tail read cap
const MAX_DIAGNOSTIC_ENTRIES: usize = 50;            // W3: frontend state cap
const MAX_LINE_DISPLAY_CHARS: usize = 500;           // W4: truncate matched lines
```

---

## Must-Read Documents

Priority reading order for an implementer starting fresh:

1. **`docs/plans/post-launch-failure-diagnostics/feature-spec.md`** — REQUIRED. Start here. All business rules, architecture diagram, data models, phase plan, and task breakdown. 10 min read.
2. **`docs/features/steam-proton-trainer-launch.doc.md`** — REQUIRED. Understand all three launch methods (especially `steam_applaunch` exit code unreliability) and the console view. Critical context for why diagnostic analysis differs by method.
3. **`src-tauri/src/commands/launch.rs` (lines 109–172)** — REQUIRED. The `stream_log_lines()` function is the primary modification target. Read the exact code before touching it.
4. **`crates/crosshook-core/src/launch/request.rs` (lines 143–200)** — REQUIRED. `ValidationSeverity`, `LaunchValidationIssue`, and `ValidationError` — the existing patterns to mirror.
5. **`docs/plans/post-launch-failure-diagnostics/research-technical.md`** — REQUIRED. Full Rust type definitions, TypeScript types, and step-by-step integration walkthrough.
6. **`docs/plans/post-launch-failure-diagnostics/research-practices.md`** — REQUIRED. Which code to reuse vs. build custom; KISS assessment; testability patterns.
7. **`docs/plans/post-launch-failure-diagnostics/research-security.md`** — REQUIRED before shipping. All 4 WARNING findings must be addressed: `safe_read_tail()`, `sanitize_display_path()`, frontend cap, non-UTF-8 handling.
8. **`crates/crosshook-core/src/launch/optimizations.rs` (lines 31–60)** — Nice-to-have. See the exact `LaunchOptimizationDefinition` struct shape and `const` array pattern to replicate for `FAILURE_PATTERN_DEFINITIONS`.
9. **`crates/crosshook-core/src/steam/diagnostics.rs`** — Nice-to-have. See `DiagnosticCollector` implementation (44 lines total) for deduplication pattern.
10. **`docs/plans/post-launch-failure-diagnostics/research-recommendations.md`** — Nice-to-have. 20 consensus decisions, risk assessment for architectural choices.

---

## Documentation Gaps

What's missing that would help an implementer:

1. **`stream_log_lines()` is undocumented** — the async fn in `launch.rs:121` has no doc comment. It will grow significantly with the diagnostics hook; a rustdoc comment explaining the poll loop → post-exit drain → diagnostic analysis sequence should be added as part of the implementation.
2. **`analyze()` public API needs rustdoc** — the primary public API of the new `crosshook-core::launch::diagnostics` submodule. The doc comment should state the "no I/O, pure function" contract, parameter semantics, and a link to the Tauri v2 event docs for the `launch-diagnostic`/`launch-complete` events it drives.
3. **`safe_read_tail()` needs a security-rationale doc comment** — the 2 MiB cap is security finding W1; that rationale should live in the function's doc comment so future maintainers don't remove it. No existing implementation to reference — build with `tokio::io::AsyncSeekExt` + `SeekFrom::End`.
4. **`DiagnosticReport` Rust ↔ TypeScript schema co-documentation gap** — `src/types/diagnostics.ts` (to be created) should have a header comment pointing to the Rust source of truth (`crosshook-core/src/launch/diagnostics/models.rs`). The feature spec has both definitions but the source files won't without explicit comments.
5. **`FailureMode` variant-to-pattern mapping** — 15 enum variants with specific trigger conditions. A table comment in `patterns.rs` linking each `FailureMode` variant to its log pattern markers will prevent drift as patterns are added.
6. **No Tauri event listening example in hooks** — `useLaunchState.ts` currently has no `listen()` calls. The pattern for wiring `listen("launch-diagnostic", ...)` in a hook isn't demonstrated in existing hooks; implementers should reference Tauri v2 event docs directly.
7. **No documentation on `crosshook-cli` exit handling** — `crosshook-cli/src/main.rs:253` is the CLI integration point but was not examined. Read it before wiring `analyze()` into the CLI path.
8. **No test fixture directory exists yet** — Research recommends `crates/crosshook-core/src/launch/diagnostics/fixtures/` for sanitized Proton log samples. Implementers need to create representative fixtures from real Proton logs or synthetic approximations.
9. **`research-ux.md` references `LaunchPanel.tsx` lines 730–757** for the feedback container CSS/DOM structure — read the component directly for exact class names (`crosshook-launch-panel__feedback`) before implementing the diagnostic banner.
