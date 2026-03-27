# Feature Spec: Post-Launch Failure Diagnostics

## Executive Summary

CrossHook's launch pipeline currently discards process exit codes and streams raw WINE/Proton debug output with no structured interpretation — the #1 UX complaint about competing Linux game launchers. This feature adds a post-launch diagnostic analysis layer that translates exit codes into human-readable messages, detects the top 10 WINE/Proton failure modes via substring pattern matching on log output, and surfaces actionable suggestions in the existing `LaunchPanel` feedback area. The implementation reuses three proven codebase patterns: `ValidationSeverity`/`LaunchValidationIssue` from #39, the data-driven `LAUNCH_OPTIMIZATION_DEFINITIONS` catalog from `optimizations.rs`, and the `DiagnosticCollector` from `steam/diagnostics.rs`. No new crate dependencies are required — exit code analysis uses `std::os::unix::process::ExitStatusExt`, pattern matching uses `str::contains()`, and all computation is a pure function testable without WINE/Proton. **No competing Linux launcher (Lutris, Bottles, Heroic) performs structured WINE log parsing, making this a genuine differentiator.**

## External Dependencies

### APIs and Services

**None.** All diagnostic computation is local. No network APIs, no external services, no telemetry.

### Libraries and SDKs

| Library                                 | Version   | Purpose                                                | New Dependency?            |
| --------------------------------------- | --------- | ------------------------------------------------------ | -------------------------- |
| `std::os::unix::process::ExitStatusExt` | stdlib    | Signal analysis: `signal()`, `core_dumped()`, `code()` | No                         |
| `chrono`                                | 0.4.x     | Diagnostic timestamp (`analyzed_at` field)             | No (already in Cargo.toml) |
| `serde` / `serde_json`                  | workspace | Serialization for IPC                                  | No (workspace dep)         |
| `tokio::io`                             | workspace | `safe_read_tail()` seek-based log reads                | No (workspace dep)         |

**Deferred dependencies:**

- `regex` (1.12.x) — only if patterns need wildcards, case-insensitive matching, or 50+ patterns. Currently all patterns are fixed literal strings.
- `minidump` (0.26.x) — only if Phase 2 crash report parsing is needed. Wine TEB bug makes stack traces unreliable; 10+ transitive deps.

### External Documentation

- [Linux signal(7) man page](https://man7.org/linux/man-pages/man7/signal.7.html): Signal codes and exit status conventions
- [Proton runtime config options](https://github.com/ValveSoftware/Proton#runtime-config-options): `PROTON_LOG`, `STEAM_COMPAT_DATA_PATH`, crash report paths
- [WineHQ Debug Channels](https://wiki.winehq.org/Debug_Channels): `err:`/`fixme:`/`warn:`/`trace:` output format (stable across WINE versions)
- [Rust `regex` ReDoS safety guarantee](https://docs.rs/regex/latest/regex/): Finite automata, O(m\*n) worst-case — safe for untrusted log input if added later
- [Sentry: Wine crash dump analysis](https://blog.sentry.io/not-so-mini-dumps-how-we-found-missing-crashes-on-steamos/): Wine TEB bug causing 500MB+ crash dumps

## Business Requirements

### User Stories

**Primary User: Steam Deck Gamer**

- As a Steam Deck user launching trainers through CrossHook, I want to understand **why** my game or trainer failed to launch so I can fix it without Googling raw WINE error codes
- As a Steam Deck user, I want diagnostic results in a gamepad-navigable panel with touch-friendly controls

**Secondary User: Linux Desktop Gamer**

- As a Linux desktop gamer, I want CrossHook to tell me when a failure is caused by a known issue (wrong Proton version, missing .NET, bad prefix) so I can fix the configuration without trial and error

**Tertiary User: Community Profile Author**

- As a profile author, I want detailed failure diagnostics I can copy to clipboard and include in bug reports or share with users who report compatibility issues

### Business Rules

1. **Exit code analysis (BR-1)**: Any non-zero exit from `child.try_wait()` triggers diagnostic analysis. Signal-based exits (128+N) are always translated. `ExitStatus::code()` returns `None` when killed by signal — use `ExitStatusExt::signal()` separately. For `steam_applaunch`, exit code is unreliable for game failures (helper exits 0 after launching Steam); pattern detection is the primary signal.

2. **Pattern detection (BR-2)**: 10 initial failure patterns matched via case-insensitive `str::contains()` against the last 2MB of log output. Patterns are method-aware (`applies_to_methods` field). Multiple patterns can match; results sorted by severity then priority. HIGH-detectability patterns must have zero known false positives before shipping.

3. **Severity model (BR-3)**: Reuse existing `ValidationSeverity` enum (`Fatal`, `Warning`, `Info`). Exit code 0 never produces `fatal`. Signal-based exits (SIGSEGV, SIGABRT, SIGKILL) are always `fatal`. Pattern severity is downgraded when exit code is 0.

4. **Crash report collection (BR-4)**: **Deferred to Phase 2.** Report crash dump existence and metadata only (never contents). Crash dumps may contain passwords/tokens in memory snapshots.

5. **Trainer vs game differentiation (BR-5)**: `target_kind` parameter (`"game"` or `"trainer"`) tailors diagnostic messages. Game and trainer are separate processes with separate log files.

6. **Scope boundary (BR-6)**: Diagnostics focus on trainer orchestration failures. Must NOT expand into general-purpose WINE debugging. Suggestions reference CrossHook-specific remediation first.

7. **Timing (BR-7)**: Post-hoc analysis only — runs after `child.try_wait()` returns `Some(status)`, never during active streaming. <100ms total budget on Steam Deck.

8. **Data sensitivity (BR-11)**: Path sanitization (`$HOME` → `~`) in all diagnostic output. Environment variable presence only, never values. `matched_pattern` capped at 512 chars. No external data transmission.

### Edge Cases

| Scenario                                   | Expected Behavior                                            | Notes                             |
| ------------------------------------------ | ------------------------------------------------------------ | --------------------------------- |
| `steam_applaunch` exits 0 but game crashes | Pattern matching runs (exit code unreliable for this method) | Helper script returns immediately |
| WINE `fixme:` lines in successful run      | Not surfaced (exit 0 caps severity to `info`)                | False positive prevention         |
| Log file >50MB                             | Only last 2MB analyzed via `safe_read_tail()`                | Errors cluster near crash time    |
| Non-UTF-8 bytes in log                     | `String::from_utf8_lossy()` conversion                       | WINE can produce binary data      |
| Multiple patterns match same failure       | All shown, sorted by severity then priority                  | Count badge: "N issues found"     |
| `crashreports/` directory missing          | Not an error; simply no crash reports                        | Normal for many games             |
| Shell script intermediate failure          | Exit codes from inner commands (not signals)                 | `set -euo pipefail`               |

### Success Criteria

- [ ] Every non-zero exit code produces at least one human-readable diagnostic entry
- [ ] All 10 HIGH-detectability patterns implemented with zero known false positives
- [ ] Diagnostic severity uses same enum and visual treatment as validation severity (#39)
- [ ] Zero disruption to existing launch flow, log streaming, and validation
- [ ] Diagnostic summary can be copied to clipboard for bug reports
- [ ] All diagnostic output complies with path sanitization and data sensitivity rules
- [ ] Pure `analyze()` function testable without WINE/Proton installation
- [ ] CLI (`crosshook-cli`) can consume same diagnostic API

## Technical Specifications

### Architecture Overview

```text
┌─────────────────────────────────────────────────────────┐
│  Frontend (React/TypeScript)                            │
│                                                         │
│  ConsoleView ← listen("launch-log")                    │
│  DiagnosticBanner ← listen("launch-diagnostic")  [NEW] │
│  useLaunchState ← LaunchPhase + DiagnosticReport  [MOD]│
└────────────────┬────────────────────────────────────────┘
                 │ Tauri IPC / Events
┌────────────────┴────────────────────────────────────────┐
│  src-tauri/src/commands/launch.rs                [MOD]  │
│                                                         │
│  stream_log_lines() → on child exit:                    │
│    1. Capture ExitStatus (code + signal)                │
│    2. safe_read_tail(log_path, 2 MiB)                   │
│    3. Call launch::diagnostics::analyze()               │
│    4. Emit "launch-diagnostic" event                    │
│    5. Emit "launch-complete" event                      │
└────────────────┬────────────────────────────────────────┘
                 │ Uses
┌────────────────┴────────────────────────────────────────┐
│  crosshook-core/src/launch/diagnostics/  [NEW SUBMOD]   │
│                                                         │
│  mod.rs          — Public API: analyze()                │
│  exit_codes.rs   — Unix signal + exit code translation  │
│  patterns.rs     — Data-driven failure pattern catalog  │
│  models.rs       — Data types (DiagnosticReport, etc.)  │
└─────────────────────────────────────────────────────────┘
```

### Data Models

#### Rust: `DiagnosticReport` (crosshook-core)

```rust
pub struct DiagnosticReport {
    pub exit_info: ExitCodeInfo,
    pub failure_mode: Option<FailureMode>,
    pub pattern_matches: Vec<PatternMatch>,  // max 50
    pub suggestions: Vec<ActionableSuggestion>,
    pub summary: String,
    pub severity: ValidationSeverity,  // reused from launch/request.rs
    pub analyzed_at: String,           // ISO 8601
    pub launch_method: String,
    pub target: String,
}
```

#### Rust: `ExitCodeInfo`

```rust
pub struct ExitCodeInfo {
    pub code: Option<i32>,
    pub signal: Option<i32>,
    pub label: String,          // e.g., "SIGSEGV (Segmentation fault)"
    pub description: String,
    pub core_dumped: bool,
    pub severity: ValidationSeverity,
}
```

#### Rust: `FailureMode` enum

15 variants organized by category: `SignalKill`, `OutOfMemory`, `ProtonDllLoadFailure`, `BadExeFormat`, `WinePrefixInvalid`, `ProtonVersionMismatch`, `ArchitectureMismatch`, `FilePermissionDenied`, `FlatpakSandboxRestriction`, `DotNetMissing`, `VcRedistMissing`, `AntiCheatInterference`, `TrainerVersionMismatch`, `LaunchTimingFailure`, `UnknownCrash`.

#### Rust: `FailurePatternDefinition` (data-driven catalog)

```rust
const FAILURE_PATTERN_DEFINITIONS: &[FailurePatternDef] = &[
    FailurePatternDef {
        id: "ntdll_load_failure",
        title: "Failed to load ntdll.dll",
        markers: &["could not load ntdll.dll", "ntdll.dll not found"],
        failure_mode: FailureMode::ProtonDllLoadFailure,
        severity: ValidationSeverity::Fatal,
        suggestions: &[("Reinstall the Proton version", "...")],
        applies_to_methods: &["steam_applaunch", "proton_run"],
    },
    // ... 9 more entries
];
```

Follows the `LAUNCH_OPTIMIZATION_DEFINITIONS` pattern from `optimizations.rs`.

#### TypeScript: Frontend types

```typescript
export interface DiagnosticReport {
  exit_info: ExitCodeInfo;
  failure_mode: FailureMode | null;
  pattern_matches: PatternMatch[];
  suggestions: ActionableSuggestion[];
  summary: string;
  severity: LaunchValidationSeverity;
  analyzed_at: string;
  launch_method: string;
  target: string;
}

// LaunchFeedback extension
export type LaunchFeedback =
  | { kind: 'validation'; issue: LaunchValidationIssue }
  | { kind: 'runtime'; message: string }
  | { kind: 'diagnostic'; report: DiagnosticReport }; // NEW
```

### API Design

#### New Tauri Events (no new commands in Phase 1)

| Event               | Payload                              | When                                      |
| ------------------- | ------------------------------------ | ----------------------------------------- |
| `launch-diagnostic` | `DiagnosticReport`                   | After child exits with non-zero status    |
| `launch-complete`   | `{ code: number?, signal: number? }` | After any child exit (success or failure) |

#### Core API: `crosshook_core::launch::diagnostics::analyze()`

```rust
pub fn analyze(
    exit_code: Option<i32>,
    signal: Option<i32>,
    core_dumped: bool,
    log_content: &str,
    launch_method: &str,
) -> DiagnosticReport
```

Pure function. No I/O, no side effects. Fully testable.

#### Security Helper: `safe_read_tail()`

```rust
pub async fn safe_read_tail(path: &Path, max_bytes: usize) -> String
```

Seek-based read of last N bytes. Avoids loading entire multi-MB log files.

### System Integration

#### Files to Create

| File                                                         | Purpose                                                                         |
| ------------------------------------------------------------ | ------------------------------------------------------------------------------- |
| `crates/crosshook-core/src/launch/diagnostics/mod.rs`        | Public `analyze()` API, re-exports                                              |
| `crates/crosshook-core/src/launch/diagnostics/models.rs`     | DiagnosticReport, FailureMode, ExitCodeInfo, PatternMatch, ActionableSuggestion |
| `crates/crosshook-core/src/launch/diagnostics/exit_codes.rs` | Exit code + signal → ExitCodeInfo (pure function)                               |
| `crates/crosshook-core/src/launch/diagnostics/patterns.rs`   | `FAILURE_PATTERN_DEFINITIONS` catalog + `scan_log_patterns()`                   |
| `src/types/diagnostics.ts`                                   | TypeScript type definitions for IPC                                             |

#### Files to Modify

| File                                                  | Change                                                                     |
| ----------------------------------------------------- | -------------------------------------------------------------------------- |
| `crates/crosshook-core/src/launch/mod.rs`             | Add `pub mod diagnostics;`                                                 |
| `src-tauri/src/commands/launch.rs`                    | Capture exit status in `stream_log_lines()`, call `analyze()`, emit events |
| `src/types/launch.ts`                                 | Extend `LaunchFeedback` with `diagnostic` kind                             |
| `src/types/index.ts`                                  | Re-export diagnostics types                                                |
| `src/hooks/useLaunchState.ts`                         | Add `diagnosticReport` to state, listen for new events                     |
| `src/components/ConsoleView.tsx` or `LaunchPanel.tsx` | Display diagnostic banner in feedback area                                 |

#### Security Constants

```rust
const MAX_LOG_TAIL_BYTES: u64 = 2 * 1024 * 1024;  // 2 MiB (W1)
const MAX_DIAGNOSTIC_ENTRIES: usize = 50;            // (W3)
const MAX_LINE_DISPLAY_CHARS: usize = 500;           // (W4)
```

## UX Considerations

### User Workflows

#### Primary Workflow: Launch Failure with Diagnostics

1. **Launch**: User clicks "Launch Game" or "Launch Trainer"
2. **Streaming**: ConsoleView streams log lines as usual
3. **Exit**: Process exits with non-zero code
4. **Analysis**: Backend runs `analyze()` (~50ms on Steam Deck)
5. **Display**: `LaunchPanel` feedback area immediately shows diagnostic panel — no navigation required
6. **Summary**: Severity badge + one-line title + actionable suggestion (always visible)
7. **Details**: "Show Details" toggle reveals matched log lines, category, additional entries
8. **Action**: User follows suggestion or copies diagnostic report to clipboard

#### Power User: Bug Report

Same steps 1-7, then "Copy Report" button generates Markdown-formatted summary with exit code, signal, Proton version, pattern matches, and sanitized environment context.

### UI Patterns

| Component              | Pattern                                                                                                                                                 | Notes                                                       |
| ---------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------- |
| Diagnostic panel       | Reuse `crosshook-launch-panel__feedback` container                                                                                                      | Same severity badge + title + help as #39 validation errors |
| Severity colors        | `fatal` → `--crosshook-color-danger` (#ff758f), `warning` → `--crosshook-color-warning` (#f5c542), `info` → `--crosshook-color-accent-strong` (#2da3ff) | No new CSS tokens needed                                    |
| Multiple issues        | Count badge "N issues found", highest-severity shown as headline                                                                                        | Expandable list sorted fatal → warning → info               |
| Progressive disclosure | Level 1: summary (always visible), Level 2: details (collapsed), Level 3: raw log (ConsoleView)                                                         |                                                             |
| Confidence language    | HIGH: assertive ("A required DLL could not be loaded"), MEDIUM: softened ("may be"), LOW: advisory ("might indicate")                                   | No numeric scores                                           |
| Copy button            | First-class focusable button, "Copied!" inline state change (2s)                                                                                        | No toast component dependency                               |

### Steam Deck Considerations

- **Layout**: Diagnostic panel fits within 280px console drawer height (collapsed: ~80px, expanded: ~240px)
- **Touch targets**: All interactive elements ≥48px (`--crosshook-touch-target-min`)
- **Gamepad focus order**: [Show Details] → [Copy Report] → [Try Again] → [Dismiss]
- **No hover-dependent interactions**: All toggles work via keyboard/gamepad activation
- **Readable at 1280×800**: 14px body min, 16px titles, ≤3 visible entries before scroll

### Performance UX

- No "analyzing..." spinner during active log streaming
- Brief "Analyzing..." state (pulsing dot) after log stream ends if analysis exceeds 200ms
- Full `DiagnosticReport` rendered at once (not streamed) — analysis is fast enough

## Recommendations

### Implementation Approach

**Recommended Strategy**: Data-driven pattern catalog with post-hoc analysis, phased delivery.

**Phasing (Phase 1 ships as #36):**

1. **Phase A — Exit Code Analysis** (hours): Pure function, zero deps, immediately useful. Capture exit status in `stream_log_lines()`, emit `launch-complete` event.
2. **Phase B — Pattern Matching** (days): `FAILURE_PATTERN_DEFINITIONS` catalog with 10 patterns. `str::contains()` on last 2MB of log. `launch-diagnostic` event.
3. **Phase D — ConsoleView Integration** (days): Frontend rendering in feedback area, `useLaunchState` extension, clipboard support.
4. **Phase C — Crash Reports** (follow-up): Deferred. Adds filesystem I/O with path validation complexity.

### Technology Decisions

| Decision         | Recommendation                     | Rationale                                                              |
| ---------------- | ---------------------------------- | ---------------------------------------------------------------------- |
| Pattern matching | `str::contains()` for v1           | All 10 patterns are fixed literal strings. Full team consensus.        |
| Exit code API    | `ExitStatusExt` (stdlib)           | Provides `signal()`, `core_dumped()`, `code()`. No `nix` crate needed. |
| Severity enum    | Reuse `ValidationSeverity`         | Frontend already renders it. Zero type proliferation.                  |
| Analysis timing  | Post-hoc (after exit)              | <100ms budget. Zero CPU during gaming. Testable pure function.         |
| Crash reports    | Defer to Phase 2                   | Reduces scope, avoids path traversal complexity.                       |
| Dependencies     | None new for v1                    | stdlib + existing workspace deps sufficient.                           |
| Event model      | Separate `launch-diagnostic` event | Follows `update-complete` pattern. Clean separation from `launch-log`. |

### Quick Wins

- **Capture exit code now** (10-line change): Modify `stream_log_lines()` line 150 from `Ok(Some(_)) => break` to capture status and emit event
- **Exit code translation** (20 lines): Pure match function, immediately testable
- **Wire into CLI** (5 lines): `crosshook-cli` already captures `child.wait()` — add `analyze()` call

### Future Enhancements

- **#49 Diagnostic bundle export**: `DiagnosticReport` serializes to JSON/TOML for bundling with log files
- **#38 Profile health dashboard**: Store diagnostic history per profile for health scoring
- **#37 Onboarding guidance**: Branch on `FailureMode` enum to provide contextual first-time guidance
- **Community-contributed patterns**: Load additional patterns from community taps (Phase 6+)
- **Proton version-aware patterns**: Filter by version range to reduce false positives

## Risk Assessment

### Technical Risks

| Risk                                      | Likelihood | Impact | Mitigation                                                                                        |
| ----------------------------------------- | ---------- | ------ | ------------------------------------------------------------------------------------------------- |
| Pattern false positives                   | High       | Medium | Non-zero exit gating, confidence tiers, HIGH patterns must have zero false positives              |
| Proton version differences                | High       | Medium | Version-agnostic patterns targeting common WINE-layer error messages. Include version in reports. |
| Log file size (50MB+)                     | Medium     | Medium | `safe_read_tail()` caps at 2MB. Errors cluster near crash time.                                   |
| Scope creep into WINE debugging           | Medium     | High   | Hard boundary: diagnostics serve trainer orchestration only (BR-6)                                |
| `steam_applaunch` exit code unreliability | High       | Medium | Pattern detection is primary signal for this method; exit code is secondary                       |
| No competitor patterns to follow          | Medium     | Medium | Start with high-confidence detections; iterate based on community feedback                        |

### Security Considerations

#### Critical — Hard Stops

| Finding         | Risk | Required Mitigation |
| --------------- | ---- | ------------------- |
| None identified | —    | —                   |

#### Warnings — Must Address

| #   | Finding                     | Risk                                           | Mitigation                                        | Alternatives                          |
| --- | --------------------------- | ---------------------------------------------- | ------------------------------------------------- | ------------------------------------- |
| W1  | Unbounded file reads        | Memory exhaustion from large logs              | `safe_read_tail()` with 2 MiB cap                 | Fixed — must implement                |
| W2  | Crash dump path traversal   | Arbitrary file reads via crafted profile paths | `canonicalize()` + `starts_with()`                | Phase 2 (deferred with crash reports) |
| W3  | Frontend state accumulation | UI memory growth from unbounded diagnostics    | Cap at 50 entries per analysis                    | Fixed — must implement                |
| W4  | Path information disclosure | Username/directory leak in screenshots         | `$HOME` → `~` sanitization in all display strings | Fixed — must implement                |

#### Advisories — Best Practices

- **A1**: Use `symlink_metadata()` before reading crash report files (existing codebase pattern, Phase 2)
- **A2**: Rust `regex` crate is inherently ReDoS-safe — document this guarantee if added later
- **A3**: Never display raw crash dump memory contents — metadata only (Phase 2)
- **A5**: Clamp exit codes to i32 range, generic message for unknowns

## Task Breakdown Preview

### Phase A: Exit Code Analysis

**Focus**: Capture and translate process exit codes
**Tasks**:

- Define `ExitCodeInfo` type in `launch/diagnostics/models.rs`
- Implement `analyze_exit_status()` pure function in `exit_codes.rs`
- Modify `stream_log_lines()` to capture `ExitStatus` and emit `launch-complete` event
- Wire into `crosshook-cli` exit handling
- Table-driven unit tests for all mapped signals
  **Parallelization**: Types and tests can be written in parallel with `stream_log_lines()` modification

### Phase B: Pattern Matching Engine

**Focus**: Detect known WINE/Proton failure modes in log output
**Dependencies**: Phase A (exit code types used in diagnostic report)
**Tasks**:

- Define `FailurePatternDef`, `FailureMode`, `PatternMatch`, `ActionableSuggestion` types
- Implement `FAILURE_PATTERN_DEFINITIONS` catalog (10 patterns)
- Implement `scan_log_patterns()` with method filtering and output cap
- Implement `safe_read_tail()` for bounded log reads
- Implement `sanitize_display_path()` for `$HOME` → `~`
- Wire `analyze()` into `stream_log_lines()` post-exit block, emit `launch-diagnostic` event
- Comprehensive pattern tests with known WINE log fixtures

### Phase D: Frontend Integration

**Focus**: Display diagnostics in LaunchPanel/ConsoleView
**Dependencies**: Phase B (diagnostic event and types)
**Tasks**:

- Create TypeScript types in `src/types/diagnostics.ts`
- Extend `LaunchFeedback` with `diagnostic` kind
- Add `launch-diagnostic` and `launch-complete` event listeners in `useLaunchState`
- Render diagnostic banner in LaunchPanel feedback area (reuse #39 severity badge pattern)
- Implement progressive disclosure (summary → details toggle)
- Implement "Copy Report" clipboard action
- Gamepad focus management and touch targets

### Phase C: Crash Report Collection (Phase 2 — Follow-up)

**Focus**: Detect and surface Proton crash dump metadata
**Dependencies**: Phase B diagnostic data model
**Tasks**:

- Implement `safe_read_file()` utility with size bounds
- Implement `validate_crash_report_path()` with canonicalization
- Implement `collect_crash_reports()` directory scanner
- Method-dependent path resolution (compatdata_path vs prefix_path)
- Launch-start-relative freshness filtering
- Integration into `DiagnosticReport`
- Unit tests with temp directory fixtures

## Decisions Needed

Before proceeding to implementation planning, clarify:

1. **Diagnostic persistence**
   - Options: (A) Store reports to `~/.config/crosshook/diagnostics/` for #38 health dashboard, (B) Re-analyze from log files on demand
   - Impact: (A) adds disk I/O but enables history; (B) is simpler but slower for dashboard
   - Recommendation: (B) for now — avoid new persistence concerns in Phase 1

2. **Event model granularity**
   - Options: (A) Single `launch-diagnostic` event with full report, (B) Separate events for exit code, patterns, crash reports
   - Impact: (A) is simpler; (B) enables progressive disclosure on frontend
   - Recommendation: (A) — analysis is <100ms, no benefit to streaming individual findings

3. **Pattern versioning**
   - Options: (A) Add min/max Proton version metadata to patterns, (B) Keep patterns version-agnostic
   - Impact: (A) reduces false positives across versions; (B) is simpler and KISS
   - Recommendation: (B) for v1 — version-agnostic patterns targeting common WINE error messages

## Research References

For detailed findings, see:

- [research-external.md](./research-external.md): Proton/WINE APIs, Rust crate evaluation (regex, aho-corasick, minidump), signal codes, competing tool analysis
- [research-business.md](./research-business.md): User stories, 11 business rules, failure modes taxonomy, existing codebase integration points
- [research-technical.md](./research-technical.md): Architecture design, Rust struct definitions, TypeScript types, Tauri event design, `stream_log_lines()` modification
- [research-ux.md](./research-ux.md): Competitive analysis (Lutris/Bottles/Heroic/Steam), progressive disclosure, Steam Deck layout, gamepad navigation
- [research-security.md](./research-security.md): 0 CRITICAL, 4 WARNING, 5 ADVISORY findings — bounded reads, path sanitization, crash dump sensitivity
- [research-practices.md](./research-practices.md): 11 reusable code modules identified, KISS assessment, `str::contains()` vs `regex`, testability patterns
- [research-recommendations.md](./research-recommendations.md): 20 consensus decisions, phasing strategy, risk assessment, cross-team synthesis
