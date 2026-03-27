# Post-Launch Failure Diagnostics — Task Structure Analysis

## Executive Summary

The feature decomposes cleanly into four sequential phases (A → B → D, with C deferred). Phase A produces foundational Rust types and exit-code translation; Phase B builds the pattern-matching engine and the public `analyze()` API; Phase D wires the frontend. The critical path is **A1 → A2+B1+B2 (parallel) → B3 → B4+D1 (parallel) → D2+D3 (parallel) → D4**. The largest parallelism window is between A2/B1/B2, all of which depend only on A1's type definitions.

The most significant implementation risk is the **`stream_log_lines()` async timing window**: the function reads the full file on every poll (not seek-based), and the final drain runs after the loop exits. The `analyze()` call and both new events (`launch-diagnostic`, `launch-complete`) must fire _after_ the final drain completes, inside the same async block, to guarantee all log lines are emitted before the diagnostic report arrives on the frontend.

---

## Recommended Phase Structure

### Phase A — Exit Code Foundation (prerequisite for everything)

**Goal**: Capture `ExitStatus` and emit a `launch-complete` event. Pure types + pure function.

| Task                                | Files                             | Parallelizable After  |
| ----------------------------------- | --------------------------------- | --------------------- |
| A1: Define core types               | `diagnostics/models.rs`           | — (start immediately) |
| A2: Exit code translator            | `diagnostics/exit_codes.rs`       | A1                    |
| A3: Capture status in Tauri command | `commands/launch.rs` (partial)    | A1                    |
| A4: Wire CLI exit hook              | `crosshook-cli/src/main.rs`       | A2 + B3 complete      |
| A5: Unit tests for exit codes       | `diagnostics/exit_codes.rs` tests | A2                    |

**Note**: A3 is a partial edit to `stream_log_lines()` — only capture the `ExitStatus` and emit `launch-complete`. Do NOT wire `analyze()` here; that belongs to B4. Splitting the two edits to the same file across phases keeps each task focused and reviewable.

### Phase B — Pattern Matching Engine (depends on A1)

**Goal**: Implement the `analyze()` public API, pattern catalog, and `safe_read_tail()`.

| Task                                           | Files                                 | Parallelizable After |
| ---------------------------------------------- | ------------------------------------- | -------------------- |
| B1: Pattern catalog + scanner                  | `diagnostics/patterns.rs`             | A1                   |
| B2: Bounded log reader + path sanitizer        | `commands/launch.rs` (new helper fns) | A1                   |
| B3: `analyze()` public API + module root       | `diagnostics/mod.rs`, `launch/mod.rs` | A2 + B1 + B2         |
| B4: Wire `analyze()` into `stream_log_lines()` | `commands/launch.rs` (second pass)    | B3 + A3              |
| B5: Pattern tests with log fixtures            | `diagnostics/patterns.rs` tests       | B1                   |

**Note**: B2 (`safe_read_tail()` and `sanitize_display_path()`) can be implemented as private functions in `commands/launch.rs` — they are async I/O helpers, not business logic, so they belong in the Tauri command layer rather than `crosshook-core`. This keeps `crosshook-core` free of async Tokio I/O.

### Phase D — Frontend Integration (depends on B3+)

**Goal**: TypeScript types, state extension, and diagnostic banner rendering.

| Task                                     | Files                                                       | Parallelizable After        |
| ---------------------------------------- | ----------------------------------------------------------- | --------------------------- |
| D1: TypeScript types                     | `types/diagnostics.ts`, `types/launch.ts`, `types/index.ts` | B3 (to mirror Rust structs) |
| D2: Extend `useLaunchState`              | `hooks/useLaunchState.ts`                                   | D1                          |
| D3: Diagnostic banner in `LaunchPanel`   | `components/LaunchPanel.tsx`                                | D1 + D2                     |
| D4: Progressive disclosure + copy action | `components/LaunchPanel.tsx` (same file)                    | D3                          |

**Note**: D2 and D3 can proceed in parallel if D2 is restricted to state/action additions only (no render changes). D3's render changes then import the new state shape.

### Phase C — Crash Reports (deferred to Phase 2)

Not in this plan. See `feature-spec.md §Phase C`.

---

## Task Granularity Recommendations

### Ideal Task Size: 1–3 Files, Single Responsibility

Each task below maps to at most 3 files and one conceptual unit of work:

| Task ID | Files Touched               | Lines Estimate | Conceptual Unit                        |
| ------- | --------------------------- | -------------- | -------------------------------------- |
| A1      | 1 new file                  | ~80            | Data model definitions only            |
| A2      | 1 new file                  | ~60            | Pure function, no I/O                  |
| A3      | 1 modified file             | ~20            | Stream loop modification, no new logic |
| A4      | 1 modified file             | ~15            | CLI callsite addition                  |
| A5      | Within A2 file              | ~60            | Table-driven unit tests                |
| B1      | 1 new file                  | ~120           | Catalog + scan function                |
| B2      | 1 modified file             | ~50            | Two private helper functions           |
| B3      | 2 files (1 new, 1 modified) | ~60            | Composition + re-exports               |
| B4      | 1 modified file             | ~25            | Call `analyze()`, emit event           |
| B5      | Within B1 file              | ~80            | Fixture-based tests                    |
| D1      | 3 modified files            | ~80            | TypeScript mirroring only              |
| D2      | 1 modified file             | ~50            | State actions + reducer branches       |
| D3      | 1 modified file             | ~40            | JSX feedback branch                    |
| D4      | 1 modified file             | ~60            | Expand/collapse + clipboard            |

**Do not merge** B1 and B3 into one task. The catalog definition and the `analyze()` composition function have different authors in a parallel plan and different test surfaces.

**Do merge** D3 and D4 only if the progressive disclosure is simple enough to implement in one PR round-trip. If the copy-to-clipboard state management requires non-trivial logic (e.g., a `useRef`-based timer for the "Copied!" label reset), split them.

---

## Dependency Analysis

```
A1 (models)
  ├── A2 (exit_codes)        ──┐
  ├── A3 (stream capture)      │
  └── B1 (patterns)            ├── B3 (analyze API)
      B2 (safe_read_tail)    ──┘        │
                                        ├── B4 (wire analyze)  ←── A3
                                        └── D1 (TS types)
                                                 │
                                           D2 (state) ──┐
                                                         ├── D3 (banner)
                                                         │       │
                                                         │      D4 (disclosure + copy)
                                           A4 (CLI) ←── B3
```

**Hard dependencies** (cannot start until predecessor is complete):

- A2, A3, B1, B2 all require A1 (type definitions must exist)
- B3 requires A2 + B1 + B2 (compose all analysis functions)
- B4 requires B3 + A3 (both the API and the capture point must be ready)
- D1 requires B3 (mirrors Rust struct layout)
- D2 requires D1 (uses `DiagnosticReport` type)
- D3 requires D1 + D2 (renders state that D2 manages)

**Soft dependencies** (ordering preferred but not strictly required):

- A5 can be written alongside A2 and merged in the same PR
- B5 can be written alongside B1 and merged in the same PR
- A4 can be written after B3 and merged in Phase B or Phase D

---

## File-to-Task Mapping

### New Files

| File                                                         | Task | Notes                                                                                     |
| ------------------------------------------------------------ | ---- | ----------------------------------------------------------------------------------------- |
| `crates/crosshook-core/src/launch/diagnostics/models.rs`     | A1   | All data types; no logic                                                                  |
| `crates/crosshook-core/src/launch/diagnostics/exit_codes.rs` | A2   | `analyze_exit_status()` + signal map + unit tests                                         |
| `crates/crosshook-core/src/launch/diagnostics/patterns.rs`   | B1   | `FailurePatternDef`, `FAILURE_PATTERN_DEFINITIONS`, `scan_log_patterns()` + fixture tests |
| `crates/crosshook-core/src/launch/diagnostics/mod.rs`        | B3   | `analyze()` public API + re-exports                                                       |
| `src/crosshook-native/src/types/diagnostics.ts`              | D1   | TypeScript IPC mirror                                                                     |

### Modified Files

| File                                                  | Tasks      | Change Description                                                      |
| ----------------------------------------------------- | ---------- | ----------------------------------------------------------------------- |
| `crates/crosshook-core/src/launch/mod.rs`             | B3         | Add `pub mod diagnostics;` + re-export `analyze`, `DiagnosticReport`    |
| `src-tauri/src/commands/launch.rs`                    | A3, B2, B4 | Three incremental edits (capture, helpers, wire) — keep in separate PRs |
| `crates/crosshook-cli/src/main.rs`                    | A4         | Add `analyze()` call after `child.wait()` line 70                       |
| `src/crosshook-native/src/types/launch.ts`            | D1         | Extend `LaunchFeedback` union with `diagnostic` kind                    |
| `src/crosshook-native/src/types/index.ts`             | D1         | Add `export * from './diagnostics'`                                     |
| `src/crosshook-native/src/hooks/useLaunchState.ts`    | D2         | Add state field, action types, `useEffect` listeners                    |
| `src/crosshook-native/src/components/LaunchPanel.tsx` | D3, D4     | Feedback branch + progressive disclosure                                |

**`commands/launch.rs` is touched by three tasks (A3, B2, B4).** Recommend ordering these as separate PRs against the same branch, or coordinating via task assignment so no two implementors edit it simultaneously.

---

## Optimization Opportunities

### High-Value Parallelism Windows

1. **After A1 merges**: A2, A3, B1, and B2 can all start simultaneously. This is the widest parallel window — four independent tasks.
2. **After B3 merges**: D1 (TS types) and B4 (wire analyze) can start simultaneously.
3. **After D1 merges**: D2 and D3 can start simultaneously (D2 restricted to state, D3 to render skeleton).

### Tasks That Can Share a PR

- A2 + A5 (exit code function + its tests — same file, same author)
- B1 + B5 (patterns catalog + fixture tests — same file, same author)
- D3 + D4 (banner render + progressive disclosure — same file, sequential within the PR)

### Tasks That Must NOT Share a PR

- A3 and B4 both edit `commands/launch.rs` but at different stages. Merging them risks the "partial analyze() wire" landing before B3 exists, causing a compile error. Keep them in separate PRs ordered A3 → B4.

---

## Implementation Strategy Recommendations

### 1. Start with Models (A1) — Unblock Parallelism Immediately

`diagnostics/models.rs` is the prerequisite for every other task. It should be the first PR, reviewed and merged before any other work begins. Keep it to pure struct/enum definitions with `serde` derives — no logic.

**Suggested A1 scope:**

- `DiagnosticReport` struct
- `ExitCodeInfo` struct
- `FailureMode` enum (all 15 variants from spec)
- `PatternMatch` struct
- `ActionableSuggestion` struct
- `FailurePatternDef` struct (the catalog entry type, used by B1)
- Security constants: `MAX_LOG_TAIL_BYTES`, `MAX_DIAGNOSTIC_ENTRIES`, `MAX_LINE_DISPLAY_CHARS`

### 2. Implement `safe_read_tail()` as an Async Helper in `commands/launch.rs` (B2)

The `crosshook-core` crate is a pure sync/logic crate — `analyze()` is intentionally sync and I/O-free. `safe_read_tail()` is an async file seek operation that belongs in the Tauri command layer. Model it on the CLI's `drain_log()` (which already uses `AsyncSeekExt`), but with a byte cap:

```rust
// Approximate shape — seek to max(0, file_len - MAX_LOG_TAIL_BYTES), read to end
async fn safe_read_tail(path: &Path, max_bytes: u64) -> String { ... }
```

### 3. The `stream_log_lines()` Integration Order (A3 → B4)

The existing function at `commands/launch.rs:121` has this structure:

```
loop { read log → emit lines → try_wait → sleep }
final drain (lines 162-171)
```

**A3 change** (capture + `launch-complete` only):

```rust
// Before the loop — initialize capture variable:
let mut exit_status: Option<ExitStatus> = None;
// Replace `Ok(Some(_)) => break` with:
Ok(Some(status)) => { exit_status = Some(status); break }
// After final drain (line 171), add:
let status_code = exit_status.as_ref().and_then(|s| s.code());
let signal = exit_status.as_ref().and_then(|s| ExitStatusExt::signal(s));
let _ = app.emit("launch-complete", serde_json::json!({ "code": status_code, "signal": signal }));
```

This is a contained, low-risk change (~10 lines). The `exit_status` variable is then available for B4's `analyze()` call without any further plumbing.

**B4 change** (add `analyze()` + `launch-diagnostic` between final drain and `launch-complete`):

```rust
// Between final drain and launch-complete:
let log_tail = safe_read_tail(&log_path, MAX_LOG_TAIL_BYTES).await;
if !is_success {
    let report = diagnostics::analyze(exit_code, signal, core_dumped, &log_tail, &method);
    let sanitized = sanitize_report_paths(report);
    let _ = app.emit("launch-diagnostic", sanitized);
}
```

This ordering guarantees all `launch-log` events arrive before `launch-diagnostic`, and `launch-diagnostic` always arrives before `launch-complete`.

### 4. Frontend State Machine Extension (D2)

`useLaunchState` currently has no Tauri event listeners. Adding them follows `ConsoleView.tsx:45-73` exactly:

```typescript
// In useLaunchState, add to useEffect or a new useEffect:
const unlistenDiagnostic = listen<DiagnosticReport>('launch-diagnostic', (event) => {
  dispatch({ type: 'diagnostic-received', report: event.payload });
});
const unlistenComplete = listen<LaunchComplete>('launch-complete', (event) => {
  dispatch({ type: 'launch-complete', payload: event.payload });
});
return () => {
  void unlistenDiagnostic.then((u) => u());
  void unlistenComplete.then((u) => u());
};
```

**Important**: The `diagnosticReport` field must be cleared on `reset` and on each new game/trainer launch action. Diagnostic reports from a previous launch should not persist into the next.

**`useEffect` deps must be `[]`** (empty array), not `[method, profileId]` or any other state. Tying listener registration to state changes would tear down and re-register the listener mid-session, creating a window where `launch-diagnostic` and `launch-complete` events can be missed. Clearing stale report state on launch-start (via the existing `game-start`/`trainer-start` actions) is sufficient isolation.

### 5. Pattern Catalog Implementation (B1)

Follow `optimizations.rs` exactly: private struct `FailurePatternDef`, `const` slice `FAILURE_PATTERN_DEFINITIONS`, public functions that iterate the slice. Do not use `HashMap` — the catalog is small (10 entries) and linear scan is O(n) with better cache behavior.

**`PatternMatch` struct should mirror `LaunchValidationIssue`** (`{ message, help, severity }`) — this ensures the frontend renders it through the same code path as existing validation issues, with zero new component work required.

The `markers` field is a `&'static [&'static str]` — multiple substring matches per pattern (any marker triggers the pattern). Sorting output by severity is a post-scan operation:

```rust
results.sort_by(|a, b| b.severity.cmp(&a.severity).then(a.priority.cmp(&b.priority)));
results.truncate(MAX_DIAGNOSTIC_ENTRIES);
```

### 6. CLI Integration (A4)

The CLI's `launch_profile()` at `crosshook-cli/src/main.rs:70` already captures `child.wait()`. The hook point is the non-success branch at lines 71-73. The CLI has no event system, so output goes to stdout/stderr:

```rust
let status = child.wait().await?;
// NEW: read tail and analyze
let log_tail = /* read log_path */ ...;
let report = diagnostics::analyze(exit_code, signal, core_dumped, &log_tail, METHOD_STEAM_APPLAUNCH);
eprintln!("{}", report.summary);  // or JSON if --json flag set
if !status.success() {
    return Err(format!("helper exited with status {status}").into());
}
```

---

## Gotchas and Edge Cases for Implementors

- **`ExitStatus::code()` returns `None` on signal death** — use `ExitStatusExt::signal()` separately; don't assume a non-None code means no signal.
- **`steam_applaunch` exits 0 even when the game fails** — `analyze_exit_status()` must return an "indeterminate" result (not success) when method is `steam_applaunch` and exit code is 0. Pattern matching is the ONLY reliable failure signal for this method; pattern scanning must always run regardless of exit code. Severity is downgraded only when exit code is confirmed 0 AND method is NOT `steam_applaunch`.
- **Non-UTF-8 bytes in logs** — use `String::from_utf8_lossy()` in `safe_read_tail()`, not the `?` operator on a `String::from_utf8()` call.
- **`feedback` state holds only one `LaunchFeedback`** — when a `launch-diagnostic` event arrives after a `game-success` action, the state machine must handle the case where phase is `WaitingForTrainer` or `SessionActive`. Diagnostic reports attach to the most recent launch, not to the current phase.
- **`useLaunchState` event listeners must be scoped** — listeners registered on mount must be cleaned up on `profileId` or `method` change; otherwise stale listeners from a previous session accumulate. The existing `useEffect` with `[method, profileId]` dependencies already resets phase state — add listener cleanup to match.
- **`LaunchPanel.tsx` `feedbackSeverity` and `feedbackLabel`** — both are derived from `feedback.kind` and `feedback.issue.severity` in the current implementation. Adding `kind: 'diagnostic'` requires extending these derivations to read from `feedback.report.severity`.
- **Copy-to-clipboard** — use `navigator.clipboard.writeText()` directly; do not add a toast library dependency. An inline `useState<boolean>` for "copied" state with a `setTimeout(reset, 2000)` is sufficient and matches the spec's "Copied! inline state change (2s)" requirement.
- **No new CSS required for the diagnostic banner** — the existing `data-severity` attribute on badge elements already drives all three severity levels (`fatal`, `warning`, `info`) via the existing stylesheet. Phase D3/D4 tasks must not add CSS work to their scope.
- **`PatternMatch` renders through `LaunchValidationIssue` path** — model the struct as `{ message, help, severity }` to reuse the existing feedback component in `LaunchPanel` without any new component code.
