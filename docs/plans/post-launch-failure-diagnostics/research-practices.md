# Practices Research: post-launch-failure-diagnostics

## Executive Summary

CrossHook has strong, reusable patterns for structured error communication (`ValidationSeverity`/`LaunchValidationIssue` from #39), data-driven definition tables (`LAUNCH_OPTIMIZATION_DEFINITIONS`), and diagnostic collection (`DiagnosticCollector`). The diagnostics feature should leverage these existing patterns directly, placing a new `diagnostics` module under `launch/` in `crosshook-core`. Exit code analysis alone will deliver the majority of user value; pattern matching and crash report collection can be phased in incrementally. No new crate dependencies are needed for v1 -- `regex` is in the lockfile transitively but not as a direct `crosshook-core` dependency, and the initial 10 patterns can use simple `str::contains()` matching.

## Existing Reusable Code

| Module / Utility                     | Location                                   | Purpose                                                                                               | How to Reuse                                                                                                                                                                       |
| ------------------------------------ | ------------------------------------------ | ----------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `ValidationSeverity`                 | `launch/request.rs:143-149`                | Three-tier severity enum (`Fatal`, `Warning`, `Info`) with snake_case serde                           | Mirror directly for diagnostic severity -- same enum or type-alias. Frontend already renders it (`LaunchValidationSeverity` in `types/launch.ts:34`)                               |
| `LaunchValidationIssue`              | `launch/request.rs:151-156`                | Structured `{message, help, severity}` for user-facing error communication                            | Diagnostic findings should use the same shape so the frontend can render them identically. Consider a type alias `DiagnosticIssue = LaunchValidationIssue` or a parallel struct    |
| `ValidationError` enum               | `launch/request.rs:158-199`                | Exhaustive enum with `message()`, `help()`, `severity()` methods that produce `LaunchValidationIssue` | Model `DiagnosticFinding` similarly: enum variant per failure mode, with methods producing the structured issue                                                                    |
| `LaunchOptimizationDefinition` table | `launch/optimizations.rs:31-177`           | Data-driven static `&[Def]` array for declarative rule resolution                                     | Pattern matching rules should follow this exact approach: `const FAILURE_PATTERN_DEFINITIONS: &[FailurePatternDef]`                                                                |
| `DiagnosticCollector`                | `steam/diagnostics.rs:1-43`                | Accumulates diagnostics + hints with deduplication and structured logging                             | Reuse directly for collecting post-launch findings. Already exported from `steam/mod.rs` -- consider moving to a shared location or making launch diagnostics a parallel collector |
| `dedupe_preserving_order`            | `steam/diagnostics.rs:32-43`               | Utility for deduplicating while preserving insertion order                                            | Already used by `DiagnosticCollector.finalize()` -- reuse transitively                                                                                                             |
| `normalizeLogMessage`                | `src/utils/log.ts:17-41`                   | Extracts displayable text from various log event payload shapes                                       | Already handles log normalization for `ConsoleView` -- diagnostic overlay can reuse same utility                                                                                   |
| `LogPayload` type                    | `src/utils/log.ts:6-10`                    | Union type for backend log event payloads                                                             | New diagnostic event payloads should extend or mirror this pattern                                                                                                                 |
| `LaunchFeedback`                     | `src/types/launch.ts:42-44`                | Discriminated union for validation vs runtime feedback                                                | Extend with a `'diagnostic'` kind for post-launch analysis results                                                                                                                 |
| `stream_log_lines`                   | `src-tauri/src/commands/launch.rs:121-172` | Polls log file, emits `launch-log` events, monitors `child.try_wait()`                                | **Critical integration point**: this is where exit code capture must happen. Currently discards exit status silently                                                               |
| `spawn_log_stream`                   | `src-tauri/src/commands/launch.rs:109-119` | Fire-and-forget spawn of log streaming task                                                           | Must be modified to capture process exit and trigger diagnostic analysis                                                                                                           |
| `create_log_path`                    | `src-tauri/src/commands/shared.rs`         | Creates timestamped log file paths                                                                    | Reuse for diagnostic report file paths                                                                                                                                             |
| `WINE_ENV_VARS_TO_CLEAR`             | `launch/env.rs:8-40`                       | Known Proton/WINE environment variable names                                                          | Useful for diagnostic context -- "which WINE vars were set?"                                                                                                                       |
| `SKIP_DIRECTORY_TERMS`               | `install/discovery.rs:14-31`               | Includes `"crashreport"` in skip list                                                                 | Confirms the codebase is already aware of crash report directories                                                                                                                 |

## Modularity Design

### Recommended Module Boundaries

```
crates/crosshook-core/src/launch/
  diagnostics/
    mod.rs              # Public API: analyze_launch_result(), DiagnosticReport
    exit_codes.rs       # Exit code → signal/meaning translation (pure functions)
    patterns.rs         # Log line pattern matching definitions (data-driven table)
    suggestions.rs      # Maps findings to actionable user suggestions
```

**Why not `diagnostics/crash_reports.rs` in v1?** Crash report collection requires filesystem access to `$STEAM_COMPAT_DATA_PATH/crashreports/`, which introduces path resolution complexity and platform-specific edge cases. Phase 2.

### Shared vs Feature-Specific

| Component             | Shared or Feature-Specific                            | Rationale                                                                           |
| --------------------- | ----------------------------------------------------- | ----------------------------------------------------------------------------------- |
| `ValidationSeverity`  | **Shared** (already exists in `launch/request.rs`)    | Same severity tiers apply to diagnostics                                            |
| `DiagnosticCollector` | **Shared** (already exists in `steam/diagnostics.rs`) | Could be promoted to a crate-level utility if post-launch diagnostics also needs it |
| `exit_codes.rs`       | **Feature-specific** to `launch/diagnostics/`         | No other module needs signal-to-name translation                                    |
| `patterns.rs`         | **Feature-specific** to `launch/diagnostics/`         | Proton error patterns are launch-specific                                           |
| `suggestions.rs`      | **Feature-specific** to `launch/diagnostics/`         | Actionable help text tied to failure modes                                          |

### Integration Points (Tauri Layer)

A new Tauri event (e.g., `launch-diagnostic`) should be emitted after `stream_log_lines` detects process exit. This keeps the diagnostic analysis in `crosshook-core` (pure functions) and the event emission in `src-tauri` (thin shell). The CLI consumer (`crosshook-cli/src/main.rs:253`) can call the same `crosshook-core` diagnostic API after `stream_helper_log` completes.

## KISS Assessment

| Area                        | Current Proposal                             | Simpler Alternative                                       | Trade-off                                                                                                                                                 |
| --------------------------- | -------------------------------------------- | --------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **Exit code analysis**      | Translate all known signal codes             | Same -- this is already simple                            | None. 20 lines of match arms delivers immediate value                                                                                                     |
| **Pattern matching**        | 10 regex-based Proton error patterns         | `str::contains()` for v1 (no regex dependency)            | Regex adds a direct dependency for marginal flexibility; `contains()` is sufficient for well-known error strings like `"err:virtual:map_view_of_section"` |
| **Crash report collection** | Scan `$STEAM_COMPAT_DATA_PATH/crashreports/` | Defer to Phase 2                                          | Crash report dirs may not exist for all games; adds filesystem complexity. Exit codes + pattern matching cover 80%+                                       |
| **Suggestion engine**       | Map each finding to actionable help text     | Inline help strings in pattern definitions                | Separate `suggestions.rs` is premature until there are >10 patterns; inline is simpler                                                                    |
| **Frontend rendering**      | New DiagnosticPanel component                | Extend existing `ConsoleView` with diagnostic annotations | Building a separate panel is cleaner long-term, but annotating console lines directly may be simpler for v1                                               |

### Phase Recommendation

- **Phase 1 (MVP)**: Exit code analysis + 5-10 `str::contains()` log patterns + inline suggestions. This delivers ~80% of value.
- **Phase 2**: Crash report collection, regex patterns, dedicated DiagnosticPanel component.
- **Phase 3**: User-defined custom patterns, pattern learning from community taps.

## Abstraction vs. Repetition

### Extract: Data-Driven Pattern Table

**Recommendation: Use a `const` table like `LAUNCH_OPTIMIZATION_DEFINITIONS`.**

The `optimizations.rs` pattern (lines 40-177) is the proven codebase idiom: a `const` array of structs with ID, match criteria, and associated data. Diagnostic patterns should follow identically:

```rust
struct FailurePatternDefinition {
    id: &'static str,
    description: &'static str,
    match_text: &'static str,          // Simple contains() target
    severity: ValidationSeverity,
    suggestion: &'static str,
}

const FAILURE_PATTERN_DEFINITIONS: &[FailurePatternDefinition] = &[
    FailurePatternDefinition {
        id: "wine_virtual_map_failure",
        description: "Virtual memory mapping failure in WINE",
        match_text: "err:virtual:map_view_of_section",
        severity: ValidationSeverity::Fatal,
        suggestion: "The game may need more virtual memory. Try adding PROTON_USE_WINED3D=1 or check ulimit -v.",
    },
    // ...
];
```

**Why this is right:**

- Rule of three is already satisfied: `LAUNCH_OPTIMIZATION_DEFINITIONS` (optimizations.rs), `WINE_ENV_VARS_TO_CLEAR` (env.rs), `SKIP_DIRECTORY_TERMS` (install/discovery.rs) all use the same static-array pattern.
- Data-driven tables are testable (iterate and assert), extensible (add a row), and reviewable (diffs show one block per new pattern).
- Individual `match` arms would be harder to test in isolation and would grow linearly in code.

### Repeat: Exit Code Translation

Exit code → signal name mapping is a simple `match` expression. No abstraction needed -- just a function:

```rust
pub fn describe_exit_code(code: i32) -> Option<ExitCodeInfo> { ... }
```

A match with ~15 arms is fine. This is inherently a fixed mapping, not a data pipeline.

### Repeat: Suggestion Text

For v1, inline suggestion text directly in the `FailurePatternDefinition` struct (as shown above). A separate `suggestions.rs` module is premature until:

- Suggestions need localization
- Multiple failure modes share the same suggestion
- Suggestions depend on runtime context (e.g., installed Proton version)

## Interface Design

### Public API Surface (`launch/diagnostics/mod.rs`)

```rust
/// Core analysis function -- pure, no I/O.
pub fn analyze_launch_result(
    exit_code: Option<i32>,
    log_lines: &[String],
    launch_method: &str,
) -> DiagnosticReport;

/// Structured result.
pub struct DiagnosticReport {
    pub exit_code_info: Option<ExitCodeInfo>,
    pub findings: Vec<DiagnosticFinding>,
    pub summary: String,
}

pub struct ExitCodeInfo {
    pub code: i32,
    pub signal_name: Option<String>,   // e.g., "SIGABRT"
    pub description: String,           // e.g., "Process aborted (crash)"
    pub severity: ValidationSeverity,
}

pub struct DiagnosticFinding {
    pub id: String,
    pub message: String,
    pub help: String,
    pub severity: ValidationSeverity,
    pub matched_line: Option<String>,  // The log line that triggered this finding
}
```

### Extension Points

1. **Adding new patterns**: Add a row to `FAILURE_PATTERN_DEFINITIONS`. No other code changes.
2. **CLI consumption**: `crosshook-cli` calls `analyze_launch_result()` after `stream_helper_log` and prints findings to stdout.
3. **Tauri consumption**: `src-tauri` calls `analyze_launch_result()` after `stream_log_lines` completes, emits a `launch-diagnostic` event.
4. **Frontend consumption**: New event listener in `useLaunchState` or a dedicated `useDiagnosticState` hook.

### Why `DiagnosticFinding` parallels `LaunchValidationIssue`

Both use `{message, help, severity}`. This is intentional: the frontend already renders `LaunchValidationIssue` via the validation feedback path. Diagnostic findings can reuse the same rendering components. A shared trait or type alias could unify them later if warranted.

## Testability Patterns

### Recommended Patterns

1. **Pure function design**: `analyze_launch_result()` takes `(exit_code, log_lines, method)` and returns a struct. No I/O, no process spawning, no filesystem access. This is the primary testability win.

2. **Static test fixtures**: Create `const` test log snippets that match known Proton error patterns:

   ```rust
   const WINE_VIRTUAL_MAP_LOG: &[&str] = &[
       "wine: some normal startup line",
       "err:virtual:map_view_of_section failed to map section",
       "wine: closing connection",
   ];
   ```

3. **Table-driven tests**: Iterate over `FAILURE_PATTERN_DEFINITIONS` to verify each has:
   - A non-empty `match_text`
   - A non-empty `suggestion`
   - A unique `id`

   This mirrors the test pattern in `env.rs:79-119` that asserts on constant arrays.

4. **Exit code edge cases**: Test 0 (success), known signals (134/SIGABRT, 139/SIGSEGV, 137/SIGKILL), unknown positive codes, and `None` (process didn't report exit code).

5. **Integration test with real log output**: Store sanitized real Proton failure logs as test fixtures in `crates/crosshook-core/src/launch/diagnostics/fixtures/`.

### Anti-patterns to Avoid

- **Don't mock process execution**: Tests should never spawn WINE/Proton. The analysis function takes strings, not processes.
- **Don't couple to `DiagnosticCollector` for unit tests**: Use it for the integration layer, but unit tests should call `analyze_launch_result()` directly.
- **Don't use regex in tests to validate pattern matching**: Test with exact log line strings, not pattern assertions.
- **Don't hardcode expected finding counts**: Patterns may be added over time. Test for presence/absence of specific findings by ID.

## Build vs. Depend

| Need                               | Build Custom                               | Use Library                                      | Recommendation                       | Rationale                                                                                                                                                                                                    |
| ---------------------------------- | ------------------------------------------ | ------------------------------------------------ | ------------------------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| **Exit code → signal name**        | 15-line `match` on `libc` signal constants | `nix` crate `Signal::try_from()`                 | **Build custom**                     | Signal names are a fixed set. Adding `nix` or `libc` for 15 lines is over-engineering. Just match on integer values directly                                                                                 |
| **Log line pattern matching (v1)** | `str::contains()` per pattern              | `regex` crate (already in lockfile transitively) | **Build custom (`str::contains()`)** | 10 patterns with known, fixed error strings don't need regex. `contains()` is faster, has zero compilation cost, and needs no new dependency                                                                 |
| **Log line pattern matching (v2)** | Keep `str::contains()`                     | Add `regex` as direct dependency                 | **Defer decision**                   | Only add `regex` if/when patterns need wildcards, capture groups, or case-insensitive matching. The lockfile already has it transitively (via `tracing-subscriber`), so the cost is only a `Cargo.toml` line |
| **Crash report file discovery**    | `std::fs::read_dir()` + path joining       | None needed                                      | **Build custom**                     | Standard library is sufficient. The codebase already does this pattern extensively in `install/discovery.rs` and `steam/libraries.rs`                                                                        |
| **Structured output**              | Serde `Serialize` derive on structs        | Already a dependency                             | **Reuse existing**                   | `serde` + `serde_json` are workspace dependencies                                                                                                                                                            |
| **Diagnostic deduplication**       | `DiagnosticCollector` already exists       | N/A                                              | **Reuse existing**                   | `steam/diagnostics.rs` has exactly this utility                                                                                                                                                              |

### Key Insight: `regex` is NOT a direct dependency of `crosshook-core`

`crosshook-core/Cargo.toml` does not list `regex`. It appears in `Cargo.lock` transitively through `tracing-subscriber`. Adding it as a direct dependency would be a deliberate choice, not "free." For v1 with ~10 known patterns, `str::contains()` is the right call.

## Open Questions

1. **Where should `DiagnosticCollector` live long-term?** Currently in `steam/diagnostics.rs`, but post-launch diagnostics is in `launch/diagnostics/`. Options: (a) import cross-module, (b) promote to crate root as a shared utility, (c) create a parallel collector. Recommendation: (a) for now, (b) if a third consumer appears.

2. **Should `DiagnosticFinding` literally be `LaunchValidationIssue`?** They share the same fields. Using the same type simplifies frontend rendering. Using a distinct type allows divergence later (e.g., `matched_line` field). Recommendation: Start with a new struct that has the same serialization shape, add a `From` impl if conversion is needed.

3. **How does exit code reach the diagnostic analyzer in the Tauri path?** Currently `stream_log_lines` calls `child.try_wait()` but only uses the status to decide when to stop polling. It discards the exit code. The function signature needs to change to return or emit the exit status.

4. **Should diagnostic events be a separate event type or extend `launch-log`?** A separate `launch-diagnostic` event is cleaner (frontend can listen independently), but extending `launch-log` with a structured payload is simpler to wire. Recommendation: Separate event.

5. **CLI integration priority**: Should `crosshook-cli` get diagnostics in v1 or v2? The CLI already handles process exit in `stream_helper_log` (main.rs:253) and could call `analyze_launch_result()` with minimal wiring. Recommendation: Include in v1 since the API is the same.
