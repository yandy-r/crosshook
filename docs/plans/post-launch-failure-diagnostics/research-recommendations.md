# Post-Launch Failure Diagnostics — Research Recommendations

**Issue**: #36
**Phase**: 2 (Diagnostics & Health)
**Dependencies**: #39 (Actionable validation errors — Done), #40 (Dry run / preview — Done)
**Generated**: 2026-03-27
**Last updated**: 2026-03-27 (cross-team synthesis incorporated)

---

## Executive Summary

CrossHook's launch pipeline currently discards process exit codes and streams raw log lines without structured interpretation. When a game or trainer launch fails through Proton, users see hundreds of WINE debug lines with no indication of what failed or how to fix it. This feature adds three layers of post-launch intelligence: exit code analysis, Proton error pattern detection, and crash report collection. The codebase is well-positioned for this — the `LaunchValidationIssue` pattern from #39, the `DiagnosticCollector` from steam diagnostics, and the `LAUNCH_OPTIMIZATION_DEFINITIONS` data-driven catalog all provide reusable patterns. The critical gap is in `stream_log_lines()` (`src-tauri/src/commands/launch.rs:121`), which currently discards the child process exit status after the polling loop ends.

---

## Implementation Recommendations

### Approach: Data-Driven Pattern Catalog

**Recommended**: Follow the `LAUNCH_OPTIMIZATION_DEFINITIONS` pattern in `optimizations.rs:40` — a `const` array of struct definitions that drive behavior declaratively.

```rust
const FAILURE_PATTERN_DEFINITIONS: &[FailurePatternDefinition] = &[
    FailurePatternDefinition {
        id: "missing_dll_dependency",
        pattern: "err:module:import_dll",  // str::contains() match
        category: FailureCategory::MissingDependency,
        severity: ValidationSeverity::Fatal,
        message: "A required DLL could not be loaded.",
        help: "The game or trainer depends on a Windows DLL that is not available in this Proton prefix. Try a newer Proton version or install the dependency with protontricks.",
        applies_to_methods: &["proton_run", "steam_applaunch"],
    },
    // ... 9 more entries
];
```

**Security hardening constants** (from tech-designer's revised spec):

```rust
const MAX_LOG_TAIL_BYTES: u64 = 2 * 1024 * 1024;  // 2 MiB (security W1)
const MAX_DIAGNOSTIC_ENTRIES: usize = 50;            // (security W3)
const MAX_LINE_DISPLAY_CHARS: usize = 500;           // truncate matched lines
```

**Why this approach wins over alternatives**:

| Criterion                 | Data-Driven Catalog                              | Match Arms                                             | External Config (TOML/JSON)           |
| ------------------------- | ------------------------------------------------ | ------------------------------------------------------ | ------------------------------------- |
| Consistency with codebase | Identical to `LAUNCH_OPTIMIZATION_DEFINITIONS`   | Common Rust idiom but different from existing patterns | New pattern, adds parser dependency   |
| Testability               | Each pattern tested independently via unit tests | Harder to test individual branches                     | Requires fixture files                |
| Extensibility             | Add a struct entry                               | Add a match arm + message + help                       | Edit external file, rebuild           |
| Compile-time safety       | Full type checking                               | Full type checking                                     | Runtime errors possible               |
| Community contributions   | Clear struct template to follow                  | Must understand match logic                            | Easier to edit but harder to validate |

### Technology Choices

1. **Pattern matching engine**: Start with `str::contains()` for the initial 10 patterns. Most Proton/WINE error signatures are simple substring matches (`err:module:import_dll`, `fixme:ntdll:`, `X11 error`). Introduce `regex` only for patterns that genuinely need it (e.g., extracting DLL names from error messages). This avoids adding `regex` as a compile-time dependency for v1.

2. **Exit code interpretation**: Pure function mapping `i32 -> DiagnosticMessage`. Unix signals are well-defined: 128+N means killed by signal N. Map the common ones (134=SIGABRT, 137=SIGKILL, 139=SIGSEGV, 143=SIGTERM). This requires zero dependencies.

3. **Crash report detection**: Use `std::fs::read_dir()` to scan `$STEAM_COMPAT_DATA_PATH/crashreports/` (or `$prefix_path/crashreports/`). For v1, report presence + count + most recent timestamp. Do NOT parse minidump binary format — just surface metadata.

4. **Diagnostic data model**: Reuse `ValidationSeverity` (`Fatal`, `Warning`, `Info`) from `launch/request.rs:143-149`. The practices-researcher and tech-designer (revised spec) agreed that reusing the existing enum avoids type proliferation and maintains UI consistency with #39. The frontend already renders these severity levels — no new styling needed.

```rust
// Reuse existing enum from launch/request.rs
// pub enum ValidationSeverity { Fatal, Warning, Info }

pub struct LaunchDiagnostic {
    pub id: String,
    pub category: DiagnosticCategory,
    pub message: String,
    pub help: String,
    pub severity: ValidationSeverity,
    pub matched_line: Option<String>,  // The log line that triggered this diagnostic
    pub line_number: Option<usize>,
}

pub struct LaunchDiagnosticReport {
    pub target_kind: String,           // "game" or "trainer" (phase-aware, BR-5)
    pub launch_method: String,         // "proton_run", "steam_applaunch", "native"
    pub exit_code: Option<i32>,
    pub exit_signal: Option<String>,
    pub exit_interpretation: Option<String>,
    pub core_dumped: bool,             // from ExitStatusExt::core_dumped()
    pub proton_version: Option<String>, // from $STEAM_COMPAT_DATA_PATH/version (BR-10)
    pub diagnostics: Vec<LaunchDiagnostic>,
    pub crash_reports: Vec<CrashReportInfo>,  // Phase 2
    pub log_path: String,
    pub analyzed_at: String,
}
```

### Phasing Strategy

**Phase 1 (ship as #36)**: **Phase A (Exit codes)** -> **Phase B (Pattern matching)** -> **Phase D (ConsoleView UX)**
**Phase 2 (follow-up)**: **Phase C (Crash reports)** — deferred per tech-designer's revised spec

This order maximizes value delivery because:

- Phase A is standalone (pure function, no log parsing) and immediately useful
- Phase B builds on A (exit code context + pattern context = full picture)
- Phase D integrates A+B into the UI — this is the shippable unit for #36
- Phase C (crash reports) is deferred to reduce scope and avoid path traversal complexity with user-controlled prefix paths. It benefits from the diagnostic data model being settled in Phase 1.

### Quick Wins

1. **Capture exit code now** (hours): Modify `stream_log_lines()` to capture `child.try_wait()` exit status and emit a `launch-exit` event with `{exit_code, signal_name}`. This is a 10-line change that provides immediate value.

2. **Exit code translation table** (hours): Pure function `fn interpret_exit_code(code: i32) -> Option<ExitCodeInterpretation>`. No dependencies, immediately testable.

3. **Wire into CLI first** (already partially done): `crosshook-cli/src/main.rs:70-73` already captures `child.wait().await` and checks `status.success()`. Add the interpretation there as a proof of concept.

---

## Improvement Ideas

### Related Feature Integration

This feature's diagnostic data model should be designed to serve three downstream features:

| Feature                          | How Diagnostics Feeds Into It                                             | Data Model Requirement                                                           |
| -------------------------------- | ------------------------------------------------------------------------- | -------------------------------------------------------------------------------- |
| **#49 Diagnostic bundle export** | Bundle includes diagnostic report + log file + crash dumps + profile TOML | `LaunchDiagnosticReport` must be serializable to JSON/TOML                       |
| **#38 Profile health dashboard** | Dashboard surfaces recent launch outcomes per profile                     | Need a `profile_id` field and persistent storage (or re-analysis from log files) |
| **#37 Onboarding guidance**      | First-time failures should trigger contextual guidance                    | `DiagnosticCategory` enum allows the onboarding flow to branch on failure type   |

### Enhancement Opportunities

1. **Diagnostic history per profile**: Store the last N diagnostic reports per profile in `~/.config/crosshook/diagnostics/`. This feeds #38's health dashboard without re-analyzing logs.

2. **Pattern confidence scoring**: Some patterns (exact string match on `err:module:import_dll`) are high-confidence. Others (generic `fixme:` lines) are low-confidence. A confidence field prevents noisy false positives from cluttering the UI.

3. **Community-contributed patterns**: The data-driven catalog could eventually load additional patterns from community taps, allowing the pattern database to grow without CrossHook releases. This is a Phase 6+ enhancement.

4. **Proton version-aware patterns**: Some failure patterns are specific to certain Proton versions. The `LaunchRequest` already carries Proton path information. Extracting the Proton version string and filtering patterns by version range would reduce false positives.

---

## Risk Assessment

### Technical Risks

| Risk                                        | Likelihood | Impact | Mitigation                                                                                                                                                                                                                               |
| ------------------------------------------- | ---------- | ------ | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **Pattern false positives**                 | High       | Medium | Use confidence scoring; only surface high-confidence matches by default. `fixme:` lines are WINE debug noise, not errors — filter aggressively. Combined with non-zero exit code gating where reliable (proton_run).                     |
| **Pattern false negatives**                 | Medium     | Medium | Start with the top 10 known failure modes. Track unmatched failures via diagnostic reports to build the pattern database iteratively.                                                                                                    |
| **Proton version differences**              | High       | Medium | WINE/Proton error message formats change between versions. Pin patterns to known-good strings from Proton 8.x/9.x. Add version metadata to patterns.                                                                                     |
| **Crash dump format instability**           | Low        | Low    | For v1, only detect presence/metadata — don't parse binary format. This is immune to format changes.                                                                                                                                     |
| **Regex performance on large logs**         | Medium     | Medium | WINE debug output can be 100K+ lines. Use `str::contains()` for initial patterns; benchmark before adding regex. If regex is needed, compile patterns once at startup, not per-line. Rust `regex` crate is ReDoS-safe (no backtracking). |
| **Log file size**                           | Medium     | Medium | Cap at last 2MB (tech-designer recommendation). Errors cluster near crash time. Prevents memory spikes from 50MB+ debug logs on Steam Deck.                                                                                              |
| **Scope creep into general WINE debugging** | Medium     | High   | Hard boundary: diagnostics serve trainer orchestration only. Do not surface general game compatibility issues unless they affect trainer injection.                                                                                      |

### Security Risks (from security-researcher)

**Overall assessment: LOW-MEDIUM risk.** No critical blockers. Four WARNING items require implementation:

| ID     | Risk                             | Severity | Mitigation                                                                                                                                                                                                                             |
| ------ | -------------------------------- | -------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **W1** | Unbounded file reads             | WARNING  | Enforce max file size before reading: 2 MiB for logs, 10 MiB for crash dumps. Implement `safe_read_file(path, max_bytes)` utility. Existing `stream_log_lines` has this latent issue.                                                  |
| **W2** | Crash dump path traversal        | WARNING  | Paths from profile `compatdata_path` + `crashreports/` must be canonicalized and verified with `canonicalize()` + `starts_with()`. Codebase already blocks traversal chars in `validate_name`; canonicalization adds defense-in-depth. |
| **W3** | Frontend diagnostic accumulation | WARNING  | Cap diagnostic output at ~50 findings per analysis. Existing `ConsoleView` already accumulates log lines without limit — diagnostics must not compound this.                                                                           |
| **W4** | Information disclosure           | WARNING  | Sanitize filesystem paths in diagnostic suggestions (replace `$HOME` with `~`). Never echo raw environment variable values. Users share diagnostic screenshots on forums.                                                              |

**Advisory items** (best practices, not blockers):

- **A1**: Use `symlink_metadata()` before reading crash report files (existing codebase pattern).
- **A2**: Rust `regex` crate is inherently ReDoS-safe. No backtracking, O(m\*n) worst case. Safe for untrusted log input. Do NOT use `fancy-regex`.
- **A3**: Minidumps contain stack memory (may include passwords/tokens). Display only metadata (existence, size, timestamp), never raw memory content.
- **A4**: `PROTON_CRASH_REPORT_DIR` env var may point anywhere. If used, validate resolved directory is within expected tree.
- **A5**: Clamp exit codes to i32 range, map known signal codes to messages, use generic message for unknowns. No array indexing by exit code.

### Integration Challenges

1. **`stream_log_lines()` modification**: This async function in launch.rs currently has no return path for the exit code. It runs in a `tauri::async_runtime::spawn()` fire-and-forget block. To surface exit codes, either:
   - (a) Emit a dedicated Tauri event (`launch-exit`) from within the function, OR
   - (b) Restructure to return the exit status to the caller

   Option (a) is lower-risk and consistent with the existing `launch-log` event pattern.

2. **Frontend state management**: The `useLaunchState` hook tracks `LaunchPhase` and `LaunchFeedback`. Post-launch diagnostics introduce a new state: the launch completed but with diagnostic findings. This could be a new `LaunchPhase.SessionCompleted` state, or diagnostics could be attached to the existing `SessionActive` phase.

3. **Two-step launch flow**: Game and trainer are separate processes with separate log files. Diagnostics need to be collected per-process and presented together. The `create_log_path()` function already namespaces logs by prefix ("game-" vs "trainer-").

### Performance Considerations

- Exit code analysis: <1ms (pure enum match)
- Pattern matching (2MB log, 30 markers): ~50ms on Steam Deck
- Crash report scan (`read_dir`): ~10ms
- Total post-exit budget: <100ms — well within acceptable latency
- Post-hoc analysis (after process exit) avoids any impact on the live log streaming experience.

---

## Alternative Approaches

### Approach A: Real-time Pattern Matching During Log Streaming

**How**: Modify `stream_log_lines()` to run each log line through the pattern catalog before emitting. Emit diagnostic events alongside raw log lines.

| Pros                                                 | Cons                                                        |
| ---------------------------------------------------- | ----------------------------------------------------------- |
| Users see diagnostics in real-time as problems occur | Adds latency to every log line emission                     |
| Can highlight problematic lines as they stream       | Pattern matching in a tight poll loop (500ms) risks jank    |
| More responsive UX                                   | Complex state management — diagnostics arrive incrementally |
|                                                      | Harder to test — requires mocking async event emission      |

**Effort**: Medium-High. Requires careful async design.

### Approach B: Post-Hoc Analysis After Process Exit (RECOMMENDED)

**How**: After `child.try_wait()` returns `Some(status)`, read the full log file, run pattern matching, collect crash reports, and emit a single `launch-diagnostics` event with the complete `LaunchDiagnosticReport`.

| Pros                                                             | Cons                                            |
| ---------------------------------------------------------------- | ----------------------------------------------- |
| Zero impact on live log streaming                                | Diagnostics appear only after process exits     |
| Full log context available (patterns can look at multiple lines) | Slight delay after exit before diagnostics show |
| Simple to implement — pure function on a string                  | Users don't see real-time pattern highlights    |
| Easy to test — deterministic input/output                        |                                                 |
| Consistent with CLI usage (CLI already waits for exit)           |                                                 |

**Effort**: Low-Medium. Clean separation of concerns.

### Approach C: Hybrid (Deferred)

**How**: Post-hoc analysis for v1. Add real-time highlights as a v2 enhancement.

| Pros                                               | Cons                                  |
| -------------------------------------------------- | ------------------------------------- |
| Gets value shipped quickly with post-hoc           | Two implementation passes             |
| Can evaluate real-time need based on user feedback | Slight code duplication between modes |

**Effort**: Low initially, Medium later.

**Recommendation**: **Approach B (Post-hoc) for v1**, with the architecture allowing Approach C later. The `stream_log_lines()` function's final read block (lines 162-171 in launch.rs) is the natural insertion point for post-hoc analysis.

### Pattern Matching Technology

| Option                  | Pros                                                                                  | Cons                                                                        | Recommendation                                                                       |
| ----------------------- | ------------------------------------------------------------------------------------- | --------------------------------------------------------------------------- | ------------------------------------------------------------------------------------ |
| `str::contains()`       | Zero dependencies, fastest, simplest, all 10+ known patterns are fixed literals       | No case-insensitive without `to_lowercase()`, can't extract substrings      | **Use for v1** (full team consensus)                                                 |
| `regex` with `RegexSet` | Single-pass multi-pattern, linear-time (ReDoS-safe), case-insensitive, capture groups | Adds ~300KB compiled dependency, compilation cost (mitigated by `LazyLock`) | **Deferred** — trigger: case-insensitive need, hex address matching, or 50+ patterns |
| `aho-corasick`          | Fastest literal matching, SIMD                                                        | No regex flexibility, comes free with `regex`                               | Use alongside `regex` if added                                                       |
| `nom` parser            | Powerful structured parsing                                                           | Massive overkill for line-based scanning                                    | Wrong tool                                                                           |

**Full team consensus**: All researchers agree on `str::contains()` for v1. Api-researcher revised original `regex` recommendation after confirming all known patterns are fixed literal strings. The data-driven `FAILURE_PATTERN_DEFINITIONS` table isolates matching logic, making future migration to `RegexSet` a one-function swap without changing catalog entries.

### Exit Code Technology (from api-researcher)

| Option                                  | Pros                                                                                             | Cons                                                   | Recommendation   |
| --------------------------------------- | ------------------------------------------------------------------------------------------------ | ------------------------------------------------------ | ---------------- |
| `std::os::unix::process::ExitStatusExt` | Zero dependencies, stdlib, stable since Rust 1.0, provides `signal()`, `core_dumped()`, `code()` | Unix-only (CrossHook is Linux-only)                    | **Use this**     |
| `nix` crate                             | Richer `WaitStatus` enum                                                                         | Unnecessary dependency weight for what stdlib provides | Over-engineering |

### Competitive Landscape (from api-researcher)

**No competitor does this.** Lutris, Bottles, and Heroic all pass raw WINE logs to users without structured analysis. This means:

- **Opportunity**: CrossHook's feature is genuinely novel and differentiating
- **Risk**: No established patterns to follow; CrossHook will be defining the standard
- **Mitigation**: Start with high-confidence detections (signal codes, DLL failures) before adding heuristic patterns

---

## Task Breakdown Preview

### Phase A: Exit Code Analysis (Standalone)

| Task                                           | Complexity | Details                                                                                                                                                                  |
| ---------------------------------------------- | ---------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| A1: Define exit code interpretation types      | Low        | `ExitCodeInterpretation { code, signal_name, message, help }` in `crosshook-core/src/launch/`                                                                            |
| A2: Implement `interpret_exit_code()` function | Low        | Pure function using `std::os::unix::process::ExitStatusExt` for `signal()`, `core_dumped()`, `code()`. Cover signals 6/9/11/15 and common WINE codes. Zero dependencies. |
| A3: Capture exit code in `stream_log_lines()`  | Low        | Use `child.try_wait()` return value, emit `launch-exit` Tauri event                                                                                                      |
| A4: Add exit code interpretation to CLI        | Low        | Wire into existing `main.rs:70-73` status check                                                                                                                          |
| A5: Unit tests for exit code interpretation    | Low        | Table-driven tests for all mapped signals                                                                                                                                |

**Estimated complexity**: Low. Pure functions, no external dependencies.

### Phase B: Pattern Matching Engine + Initial 10 Patterns

| Task                                                        | Complexity | Details                                                                                                                                                  |
| ----------------------------------------------------------- | ---------- | -------------------------------------------------------------------------------------------------------------------------------------------------------- |
| B1: Define `FailurePatternDefinition` struct and catalog    | Medium     | Follow `LAUNCH_OPTIMIZATION_DEFINITIONS` pattern                                                                                                         |
| B2: Implement `analyze_log()` function                      | Medium     | Takes log content + launch method, returns `Vec<LaunchDiagnostic>`. Read last 2MB of log (security W1 cap). Cap output at ~50 diagnostics (security W3). |
| B3: Define initial 10 failure patterns                      | Medium     | Research top WINE/Proton failure modes (see list below)                                                                                                  |
| B4: Define `LaunchDiagnosticReport` aggregate type          | Low        | Combines exit code + patterns + metadata                                                                                                                 |
| B5: Wire analysis into `stream_log_lines()` post-exit block | Medium     | Call analysis after final log read, emit `launch-diagnostics` event                                                                                      |
| B6: Add Tauri command for on-demand log analysis            | Low        | `analyze_launch_log(log_path: String) -> LaunchDiagnosticReport`                                                                                         |
| B7: Comprehensive unit tests                                | Medium     | Test each pattern against known WINE log samples                                                                                                         |

**Initial 12 failure patterns** (research-informed, with detectability tiers from business-analyzer):

**HIGH detectability** (zero known false positives required):

1. `err:module:import_dll` — Missing DLL dependency
2. `Bad EXE format` — Wrong executable format / corrupt binary
3. `status c0000135` / `could not load ntdll` — Missing .NET/vcredist runtime
4. `vkd3d-proton: ERROR` / `DXVK: Failed to create` — Vulkan/GPU initialization failure
5. `wine: cannot find` / `Application could not be started` — Binary not found
6. Signal-based crash (exit codes 134/137/139) — SIGABRT/SIGKILL/SIGSEGV

**MEDIUM detectability** (may need exit-code context): 7. `err:virtual:virtual_setup_exception` — Unhandled exception / crash 8. `Permission denied` — File permission issue (gated on non-zero exit) 9. `MESA-INTEL: warning` or `MESA: error` — GPU driver issue 10. `X11 error` / `BadDrawable` / `BadWindow` — Display server error

**LOW detectability** (heuristic, deferred to v2): 11. Short runtime (<5s) + non-zero exit — Immediate crash heuristic 12. Trainer version mismatch — Requires game version correlation (#41)

**Estimated complexity**: Medium. Core logic is straightforward; research and validation of patterns is the main effort.

### Phase C: Crash Report Collection (Deferred to Phase 2)

| Task                                                    | Complexity | Details                                                                                                                                     |
| ------------------------------------------------------- | ---------- | ------------------------------------------------------------------------------------------------------------------------------------------- |
| C1: Implement `safe_read_file(path, max_bytes)` utility | Low        | Bounded file reads (security W1). 10 MiB cap for crash dumps. Reusable across codebase.                                                     |
| C2: Implement `validate_crash_report_path()`            | Low        | Canonicalize + `starts_with()` (security W2). Ensure path stays within expected prefix tree.                                                |
| C3: Define `CrashReportInfo` type                       | Low        | `{ path, size_bytes, modified_at, file_name }` — metadata only, never raw dump content (security A3)                                        |
| C4: Implement `collect_crash_reports()` function        | Medium     | Scan `{prefix_path}/crashreports/` directory with path validation, return metadata. Use `symlink_metadata()` (security A1).                 |
| C5: Resolve correct crash report path per launch method | Medium     | `steam_applaunch` uses `compatdata_path`, `proton_run` uses `runtime.prefix_path`. Validate `PROTON_CRASH_REPORT_DIR` if set (security A4). |
| C6: Filter by launch start timestamp                    | Low        | Use launch-start-relative filtering instead of fixed 5-minute window (business-analyzer recommendation).                                    |
| C7: Integrate into `LaunchDiagnosticReport`             | Low        | Add crash reports to the aggregate report                                                                                                   |
| C8: Unit tests with temp directory fixtures             | Low        | Create fake crash dump files, verify collection + path validation                                                                           |

**Estimated complexity**: Low-Medium. File system operations with security validation, no binary parsing.

### Phase D: ConsoleView Integration + UX

| Task                                                         | Complexity | Details                                                                                                  |
| ------------------------------------------------------------ | ---------- | -------------------------------------------------------------------------------------------------------- |
| D1: Listen for `launch-exit` and `launch-diagnostics` events | Medium     | New event listeners in ConsoleView or sibling component                                                  |
| D2: Render diagnostic summary panel in ConsoleView           | Medium     | Collapsible panel below log output showing diagnostics. Sanitize paths (`$HOME` -> `~`) per security W4. |
| D3: Render exit code interpretation                          | Low        | Display signal name + human message at bottom of console                                                 |
| D4: Render crash report indicators                           | Low        | Badge or list showing available crash dumps                                                              |
| D5: Update `LaunchFeedback` type for diagnostic feedback     | Low        | Extend discriminated union with `kind: 'diagnostic'`                                                     |
| D6: Update `useLaunchState` for post-exit diagnostic state   | Medium     | Handle transition from active session to completed-with-diagnostics                                      |

**Estimated complexity**: Medium. Primarily frontend work, reusing existing component patterns.

---

## Cross-Team Synthesis

Findings from all completed research teammates have been incorporated. Key refinements:

### From Business Analyzer

- **`steam_applaunch` diagnostic blind spot**: The helper script exits 0 after launching Steam, but the actual game crash happens in a separate process tree managed by Steam. For this launch method, exit code analysis is unreliable for game failures — pattern detection in logs is the primary diagnostic signal. This is a critical constraint that shapes the architecture: exit code analysis and log pattern analysis must be independent, not gated on each other.
- **Log format heterogeneity**: Logs contain `[steam-helper]` prefixed lines (from shell scripts), raw WINE/Proton debug output, and game stderr. Pattern matching must handle all three sources and potentially attribute diagnostics to the correct source.
- **Exit code 0 still needs analysis**: WINE processes can exit 0 even when the game crashed internally. Business rule: pattern scanning should run regardless of exit code for Proton launches.

### From Tech Designer

- **Severity model (revised)**: Initially proposed separate `DiagnosticSeverity`. After practices-researcher feedback, **revised to reuse `ValidationSeverity`** (`Fatal/Warning/Info`). Avoids type proliferation; frontend already renders these levels.
- **Log size cap**: Read last 2MB of log file via `safe_read_tail()`. WINE errors cluster near crash time. Prevents memory spikes from 50MB+ debug logs.
- **Performance budget (Steam Deck)**: Exit code analysis <1ms, pattern matching (2MB, 30 markers) ~50ms, crash report scan ~10ms. Total <100ms post-exit — well within acceptable latency.
- **Event-driven frontend integration**: `launch-diagnostic` and `launch-complete` events follow the existing `launch-log` / `update-complete` patterns. No polling needed.
- **Crash reports deferred to Phase 2**: Reduces Phase 1 scope and avoids path traversal risks with user-controlled prefix paths. Phase 1 ships exit codes + pattern matching + ConsoleView integration.
- **`applies_to_methods` field**: Added to pattern definitions to filter patterns by launch method. Aligns with our recommendation.
- **`LaunchFeedback` extension**: New `diagnostic` kind alongside existing `validation` and `runtime` discriminated union members.
- **Security hardening**: `safe_read_tail()` for bounded reads, `sanitize_display_path()` for `$HOME` -> `~`, line truncation at 500 chars, `MAX_DIAGNOSTIC_ENTRIES = 50`.
- **Pure analysis function**: `analyze(exit_code, signal, log_content, method) -> DiagnosticReport` — no I/O, no side effects, testable without WINE.

### From Practices Researcher

- **KISS validation**: Phase 1 (exit codes + 5-10 substring patterns) delivers 80% of the value. Regex, crash reports, and separate modules are v2 concerns.
- **No new crate dependencies for v1**: Exit code translation (15-line match), log pattern matching (`str::contains()`), and crash report discovery (`std::fs`) all use stdlib. Do not add `regex`, `nix`, or `libc`.
- **Module placement**: `crates/crosshook-core/src/launch/diagnostics/` with 3 files: `mod.rs`, `exit_codes.rs`, `patterns.rs`. Core function must be pure: `analyze_launch_result(exit_code, log_lines, method) -> DiagnosticReport`.
- **Table-driven tests**: Iterate `FAILURE_PATTERN_DEFINITIONS` for invariant checks. Use static test fixtures with known Proton error log snippets.

### From API Researcher

- **`str::contains()` confirmed for v1 (revised position)**: Api-researcher initially recommended `regex` with `RegexSet` but revised after team feedback. All 10+ known Proton error patterns are fixed literal strings (`"could not load ntdll"`, `"Bad EXE format"`, `"status c0000135"`). None require regex features. The data-driven `FAILURE_PATTERN_DEFINITIONS` table isolates matching logic, making future migration to `regex` a one-function swap.
- **When to add `regex`**: If/when patterns need wildcards, hex address matching, version-agnostic pattern matching, or 50+ patterns where `RegexSet` single-pass becomes measurably faster.
- **`std::os::unix::process::ExitStatusExt`**: Confirmed as the right API for exit code analysis. Provides `signal()`, `core_dumped()`, `code()` from stdlib. No `nix` crate needed.
- **Wine TEB bug**: Crash dumps can be 500MB+ due to incorrect stack boundary reporting (Sentry research). Reinforces Phase 1's "metadata only" approach. `minidump` crate has 10+ transitive dependencies and unreliable stack traces.
- **WINE debug format insight**: Errors follow `err:channel:function message` format. Enables more precise substring markers (check for `err:` prefix rather than broad matching).
- **Flatpak detection**: `/.flatpak-info` or `FLATPAK_ID` env var can boost `flatpak_sandbox` pattern confidence. Tech-designer adding to spec.
- **No competitor does structured WINE log analysis**: Lutris, Bottles, Heroic all show raw logs. CrossHook is defining the standard — start with high-confidence detections.
- **`nom` rejected**: Overkill for line-based text scanning. `aho-corasick` comes free with `regex` if added later.

### From Security Researcher

- **Overall assessment: LOW-MEDIUM risk.** No critical blockers. Four WARNING-level items require implementation.
- **Unbounded file reads (W1)**: Must enforce max file size before reading (2 MiB logs, 10 MiB crash dumps). Implement `safe_read_file(path, max_bytes)` utility. This latent issue also exists in current `stream_log_lines`.
- **Path traversal (W2)**: Crash dump paths from profile `compatdata_path` must be canonicalized with `canonicalize()` + `starts_with()`.
- **Information disclosure (W4)**: Sanitize `$HOME` to `~` in diagnostic output. Users share screenshots on forums.
- **Minidump sensitivity (A3)**: Never display raw crash dump memory content — only metadata (existence, size, timestamp).
- **Regex safety confirmed (A2)**: Rust `regex` crate is inherently ReDoS-safe. No backtracking. Safe for untrusted WINE log input. Do NOT use `fancy-regex`.

### From Business Analyzer (Final Update)

- **Business rules updated**: BR-1 (always scan on Proton), BR-2 (method-aware patterns), BR-4 (method-dependent crash path resolution), BR-5 (phase-aware diagnostics), BR-6 (scope creep guard) all incorporated.
- **Crash report freshness**: Considering launch-start-relative filtering instead of fixed 5-minute window. This eliminates the heuristic problem of stale crash reports from previous sessions.

### Consensus Decisions

| Decision                    | Team Consensus                       | Rationale                                                                                                                                    |
| --------------------------- | ------------------------------------ | -------------------------------------------------------------------------------------------------------------------------------------------- |
| Pattern matching approach   | `str::contains()` for v1             | Business, tech, practices all agree. No `regex` dependency needed.                                                                           |
| Analysis timing             | Post-hoc (after exit)                | Tech + practices confirm <100ms budget. No gaming CPU impact.                                                                                |
| Severity enum               | Reuse `ValidationSeverity`           | Tech-designer revised: practices-researcher argued reuse avoids type proliferation. Frontend already renders these levels.                   |
| Log size cap                | Last 2MB                             | Tech-designer: errors cluster near end. Prevents memory spikes. Security confirms W1.                                                        |
| Frontend integration        | Tauri events (`launch-diagnostic`)   | Follows existing `launch-log` pattern. No polling.                                                                                           |
| Pattern gating on exit code | Run patterns regardless of exit code | Business-analyzer: WINE exits 0 on crashes. `steam_applaunch` always exits 0.                                                                |
| New dependencies            | None for v1                          | Practices-researcher: stdlib is sufficient.                                                                                                  |
| File read safety            | Enforce max size caps                | Security W1: 2 MiB logs, 10 MiB crash dumps. Implement `safe_read_file()`.                                                                   |
| Path validation             | Canonicalize + starts_with           | Security W2: defense-in-depth for crash dump path resolution.                                                                                |
| Diagnostic output cap       | Max ~50 findings per analysis        | Security W3: prevent frontend state accumulation.                                                                                            |
| Path sanitization           | Replace `$HOME` with `~`             | Security W4: users share diagnostic screenshots on forums.                                                                                   |
| Crash reports               | Defer to Phase 2                     | Tech-designer revised: reduces Phase 1 scope, avoids path traversal complexity with user-controlled prefix paths.                            |
| LaunchFeedback extension    | New `diagnostic` kind                | Tech-designer: alongside existing `validation` and `runtime` kinds. Clean discriminated union extension.                                     |
| Line truncation             | 500 char max for matched lines       | Tech-designer: prevents oversized diagnostic entries from WINE debug output.                                                                 |
| Exit code API               | `ExitStatusExt` from stdlib          | Api-researcher: provides `signal()`, `core_dumped()`, `code()`. No `nix` crate needed.                                                       |
| Pattern migration path      | Design struct for future `RegexSet`  | Full team consensus on `str::contains()` for v1. Api-researcher revised position. Struct isolates matching for future swap.                  |
| Crash dump parsing          | Filesystem detection only            | Api-researcher: `minidump` crate has 10+ transitive deps, Wine TEB bug makes stack traces unreliable. Metadata-only.                         |
| Proton version context      | Include in diagnostic reports        | Business-analyzer BR-10: read `version` file from `$STEAM_COMPAT_DATA_PATH/`. Version-agnostic patterns but version string for user context. |
| Pattern detectability tiers | HIGH/MEDIUM/LOW                      | Business-analyzer: HIGH patterns must have zero known false positives. LOW patterns deferred to v2.                                          |
| Success criteria            | Zero false positives on HIGH tier    | Business-analyzer: all HIGH-detectability patterns must pass this bar before shipping.                                                       |

---

## Key Decisions Needed

### Resolved by Team Consensus

| #   | Decision                       | Resolution                                                                                                                                                               |
| --- | ------------------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| 1   | Post-hoc vs real-time analysis | **Post-hoc for v1.** Tech-designer confirmed <100ms budget. Practices confirmed KISS alignment.                                                                          |
| 4   | Exit code capture scope        | **Events (fire-and-forget).** Emit `launch-diagnostic` + `launch-complete` events. Consistent with existing architecture.                                                |
| 6   | Diagnostic severity levels     | **Reuse `ValidationSeverity`** (`Fatal/Warning/Info`). Tech-designer revised per practices-researcher: avoids type proliferation, frontend already renders these levels. |
| 7   | Crash report timing            | **Defer crash reports to Phase 2.** Tech-designer revised: reduces Phase 1 scope, avoids path traversal complexity.                                                      |

### Still Need Owner Input

2. **Diagnostic persistence**: Should diagnostic reports be stored on disk for profile health dashboard (#38), or re-analyzed from log files on demand? Storing reports is simpler for #38 but adds disk I/O. Re-analyzing is cleaner but slower. Practices-researcher leans toward re-analysis from existing logs to avoid new persistence concerns.

3. **Event model**: Single `launch-diagnostic` event with full `LaunchDiagnosticReport`, or separate events for exit code, pattern matches, and crash reports? Team leans toward single event for simplicity but UX research may have input on progressive disclosure.

4. **Pattern versioning**: Should patterns carry minimum/maximum Proton version metadata, or is this premature for v1? Business-analyzer notes Proton version differences are a real risk, but practices-researcher flags this as potential over-engineering for v1.

---

## Open Questions

1. **What exit codes does the `steam-launch-helper.sh` shell script return for different failure modes?** The script captures `$exit_code` from `proton run` (line 387-389) but the semantics of the helper's own exit code vs the game's exit code need clarification. Business-analyzer confirmed: for `steam_applaunch`, the helper exits 0 after launching Steam — the game crash happens in a separate process tree. This means **exit code is unreliable for `steam_applaunch` game failures** and pattern detection is the primary signal.

2. **Are there Proton-specific exit codes beyond Unix signals?** WINE may use custom exit codes that don't map to standard signal numbers. API research (pending from api-researcher) may clarify.

3. **What is the actual directory structure of `$STEAM_COMPAT_DATA_PATH/crashreports/`?** Need to verify this path exists in practice and what file formats are found there (Windows minidump .dmp files? WINE crash logs?). This is deferred to Phase C — not needed for v1.

4. **How large are typical WINE debug logs?** Tech-designer recommends a 2MB tail cap. Need empirical validation. If games routinely produce 100MB+ logs, the cap is critical for Steam Deck memory constraints.

5. **Should the CLI (`crosshook-cli`) also receive diagnostic analysis output?** The CLI already checks exit status — adding pattern analysis would benefit headless/scripted usage. Recommendation: yes, since all logic lives in `crosshook-core`. Practices-researcher confirmed the pure function design supports this with no additional work.

6. **How does the `update-log` event stream interact with diagnostics?** The `ConsoleView` listens to both `launch-log` and `update-log`. Should update operations (installer runs) also receive diagnostic analysis, or is this strictly for launch failures? Recommendation: launch-only for v1, extend to updates in a follow-up if needed.

7. **Crash report timing window** (from business-analyzer): Using a 5-minute window for crash report freshness is a heuristic. Long game loading times could produce stale crash reports from previous sessions. What's the right threshold, or should freshness be relative to the launch start timestamp instead?

---

## Relevant Files

| File                                                               | Role                                                                    |
| ------------------------------------------------------------------ | ----------------------------------------------------------------------- |
| `crates/crosshook-core/src/launch/mod.rs`                          | Module root — new `diagnostics` submodule goes here                     |
| `crates/crosshook-core/src/launch/request.rs`                      | `LaunchValidationIssue` pattern to reuse                                |
| `crates/crosshook-core/src/launch/optimizations.rs`                | `LAUNCH_OPTIMIZATION_DEFINITIONS` catalog pattern to follow             |
| `crates/crosshook-core/src/steam/diagnostics.rs`                   | `DiagnosticCollector` — existing diagnostic collection pattern          |
| `src-tauri/src/commands/launch.rs`                                 | `stream_log_lines()` — critical integration point for exit code capture |
| `src-tauri/src/commands/shared.rs`                                 | `create_log_path()` — log file location (`/tmp/crosshook-logs/`)        |
| `src/components/ConsoleView.tsx`                                   | UI integration point for diagnostic display                             |
| `src/hooks/useLaunchState.ts`                                      | `LaunchPhase` state machine — needs post-exit diagnostic state          |
| `src/types/launch.ts`                                              | `LaunchFeedback` / `LaunchResult` types — need diagnostic extensions    |
| `src/utils/log.ts`                                                 | `normalizeLogMessage()` — extend for diagnostic event payloads          |
| `crates/crosshook-core/src/launch/runtime_helpers.rs`              | `resolve_wine_prefix_path()` — needed for crash report path resolution  |
| `crates/crosshook-core/src/launch/env.rs`                          | `WINE_ENV_VARS_TO_CLEAR` — env var context for diagnostics              |
| `crates/crosshook-cli/src/main.rs`                                 | CLI exit code handling — already captures `child.wait()`                |
| `runtime-helpers/steam-launch-helper.sh`                           | Shell script exit code flow (line 387-389)                              |
| `docs/research/additional-features/implementation-guide.md`        | Phase 2 context and dependency chain                                    |
| `docs/plans/post-launch-failure-diagnostics/research-technical.md` | Technical spec (data model, API, wiring)                                |
| `docs/plans/post-launch-failure-diagnostics/research-security.md`  | Security risk assessment (W1-W4, A1-A5)                                 |
| `docs/plans/post-launch-failure-diagnostics/research-practices.md` | Modularity, KISS, and reuse assessment                                  |
