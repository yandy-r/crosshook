# PR #396 — Code Review

**PR**: [feat(launch): trainer watchdog cleanup parity with game launches](https://github.com/yandy-r/crosshook/pull/396)
**Head**: `feat/trainer-watchdog-parity` @ `e12fe29` (2 commits: `7f13532` feat + `e12fe29` review-fix refactor)
**Scope**: 17 files changed, ~1205 insertions — Resolves #230
**Reviewer**: `ycc:code-review --parallel` (3 reviewers fanned out: correctness, security, quality)
**Date**: 2026-04-20

## Validation

| Check                                                                             | Result                                                          |
| --------------------------------------------------------------------------------- | --------------------------------------------------------------- |
| `./scripts/check-host-gateway.sh`                                                 | pass                                                            |
| `cargo fmt --check`                                                               | clean                                                           |
| `cargo clippy -p crosshook-core -p crosshook-native --all-targets -- -D warnings` | clean                                                           |
| `cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core`    | 1117 lib + 8 integration + 25 supplementary tests **all green** |

## Verdict

**COMMENT — approve-with-notes**. Zero Critical, zero High. Six Medium findings are worth addressing before merge (or in a follow-up PR before the field is consumed by frontend/tooling) — notably the `TeardownReason` serde-style inconsistency with persisted data and the two "orphan cancel channel" code paths. Seven Low + one Info are polish.

## Summary

| Severity | Count |
| -------- | ----- |
| Critical | 0     |
| High     | 0     |
| Medium   | 6     |
| Low      | 7     |
| Info     | 1     |

## Findings

All findings use a `Status:` line that a follow-up `/ycc:review-fix` run can flip from `Open` → `Fixed` / `Deferred` in place.

---

### F001 — `TeardownReason` missing `#[serde(rename_all = "snake_case")]`

- **Severity**: Medium
- **Lane**: Correctness
- **File**: `src/crosshook-native/crates/crosshook-core/src/launch/session/types.rs:44-61`
- **Issue**: `TeardownReason` derives `Serialize`/`Deserialize` without `rename_all`. Every peer enum in the diagnostics layer (`FailureMode`, `ValidationSeverity`) uses snake_case. `TeardownReason` will therefore persist into `launch_operations.diagnostic_json` as `"NaturalExit"`, `"LinkedSessionExit"`, etc., inconsistent with the rest of the schema. `as_str()` already returns snake_case strings — the intent was clearly snake_case, but the Serde derive diverges.
- **Fix**: Add `#[serde(rename_all = "snake_case")]` to the enum. Update the round-trip integration test (`teardown_reason_round_trips_through_diagnostic_report_json`) to expect `"linked_session_exit"` in the JSON substring. The `"receiver_closed_serializes_distinctly_from_linked_session_exit"` test needs its string literal updated too.
- **Status**: Fixed — added `#[serde(rename_all = "snake_case")]` on `TeardownReason`; test assertions updated to expect `"linked_session_exit"` and `"receiver_closed"`.

---

### F002 — Game session without gamescope drops its cancel receiver silently

- **Severity**: Medium
- **Lane**: Security / Completeness
- **File**: `src/crosshook-native/src-tauri/src/commands/launch/execution.rs:168–179`
- **Issue**: `launch_game` registers a session and obtains `cancel_rx` unconditionally, but only hands it to `spawn_gamescope_watchdog` when `gamescope_active && child_pid.is_some()`. For a non-gamescope game launch (or a launch where `child.id()` returns `None`), `cancel_rx` is dropped at function end. The session remains registered and a future `cancel_session` or linked-child cascade targeting this game sees `send()` succeed (broadcast records the signal even with zero live receivers) but the outcome is never stamped. `diagnostic_json` for that launch then reports `teardown_reason: natural_exit` even if a cancel was explicitly requested.
- **Fix**: Mirror the trainer path: when `gamescope_active` is false (or `child_pid` is `None`) spawn `drain_cancel_on_trainer_no_watchdog` (or a renamed `drain_cancel_when_watchdog_absent`) so the game's cancel channel has a live receiver that records the reason into `watchdog_outcome` via `record_reason`.
- **Status**: Fixed — both launch paths now route cancel receivers through a shared `consume_cancel_channel` helper that falls back to `drain_cancel_into_outcome` (now in `crosshook-core`) whenever no gamescope watchdog is spawned.

---

### F003 — Trainer with `trainer_gamescope_active=true` but missing `child.id()` also drops `cancel_rx`

- **Severity**: Medium
- **Lane**: Correctness
- **File**: `src/crosshook-native/src-tauri/src/commands/launch/execution.rs:366–391`
- **Issue**: When `trainer_gamescope_active` is true but `child.id()` returns `None`, neither the watchdog-spawn branch nor the drain-helper `else` branch runs. `cancel_rx` is dropped. Trainer is registered, possibly linked to a parent game, but its cancel channel has no receiver — a cascade silently no-ops on the outcome. Same class of bug as F002 but on the trainer side.
- **Fix**: Move the drain-helper spawn out of the `trainer_gamescope_active` `else` branch and into an unconditional fallback when no watchdog is spawned (guard: trainer watchdog was gated, OR `child_pid` missing).
- **Status**: Fixed — consolidated into `consume_cancel_channel`; if `gamescope_active && child_pid.is_none()` the helper logs a warning and still spawns the drain task.

---

### F004 — Arbitrary parent selection when multiple game sessions share a profile

- **Severity**: Medium
- **Lane**: Correctness
- **File**: `src/crosshook-native/src-tauri/src/commands/launch/execution.rs:306–331`
- **Issue**: `sessions_for_profile` returns session IDs in `HashMap` iteration order (non-deterministic). When more than one game session is active for the same profile (rare — a double-launch — but not prevented), the trainer links to an arbitrary game. PR body's "Risk I'd like extra eyes on" explicitly flagged this.
- **Fix**: Either enforce "one game session per profile at a time" in `register` (return an error, or document as precondition), or switch the registry's inner map to `indexmap::IndexMap` so `sessions_for_profile` returns insertion-ordered results, and pick the most-recently-registered game as parent.
- **Status**: Fixed — added `SessionEntry.registered_at: Instant`; `sessions_for_profile` now sorts most-recent-first; new test `sessions_for_profile_returns_most_recent_first` guards ordering.

---

### F005 — Register → link is a two-step lock sequence (race window)

- **Severity**: Medium
- **Lane**: Security / Concurrency
- **File**: `src/crosshook-native/src-tauri/src/commands/launch/execution.rs:304–316`
- **Issue**: Trainer `register` and `sessions_for_profile` + `link_to_parent` acquire the registry mutex three separate times. If a game session's `finalize_launch_session` runs between the trainer's `register` and its `link_to_parent`, the lookup returns a session ID that is gone by the time `link_to_parent` runs → `LinkError::ParentNotFound`, warn-log, and the trainer runs orphaned (no cancel plumbed). Narrow window but real; leaves the trainer requiring manual teardown.
- **Fix**: Add an atomic `register_and_link_to_parent_of_kind(kind, profile_key, parent_kind)` method to the registry that performs register + candidate lookup + link under one lock. Alternatively, document the warn-log path as the accepted degraded behavior and add a test exercising the race.
- **Status**: Fixed — added atomic `register_and_link_to_parent_of_kind`; `launch_trainer` uses it in place of register + sessions_for_profile + link_to_parent. Three new tests cover happy path, no-candidate, and illegal pairings.

---

### F006 — `execution.rs` past the ~500-line soft cap; duplicated preamble between handlers

- **Severity**: Medium
- **Lane**: Maintainability
- **File**: `src/crosshook-native/src-tauri/src/commands/launch/execution.rs` (whole file, 549 lines post-PR)
- **Issue**: `launch_game` and `launch_trainer` share duplicated warning-collection preamble (lines 55–69 / 203–217), method-resolution match, and snap-variable block. PR body flagged this. File sits ~10% past the soft cap.
- **Fix**: Follow-up refactor — extract `collect_launch_warnings` and `resolve_method_str` helpers so both handlers delegate the preamble. Does not need to block this PR if team accepts current size, but track as a follow-on issue (the refactor is natural now that game/trainer paths are symmetric).
- **Status**: Open (deferred — follow-up refactor)

---

### F007 — `sessions_for_profile_filters_by_kind` test uses order-sensitive assertions

- **Severity**: Low
- **Lane**: Correctness / Maintainability (reported by two reviewers)
- **File**: `src/crosshook-native/crates/crosshook-core/src/launch/session/registry.rs:291–295`
- **Issue**: `assert_eq!(games, vec![game_id])` and `assert_eq!(trainers, vec![trainer_id])` depend on `HashMap` iteration order. Single-element case passes today but a future test that adds a second matching session will flake without warning.
- **Fix**: `assert_eq!(result.len(), 1); assert!(result.contains(&id));` for both.
- **Status**: Fixed — `sessions_for_profile_filters_by_kind` test now uses `len() + contains` assertions and an inline comment notes the rationale.

---

### F008 — "Game never appeared" standdown path doesn't mark outcome

- **Severity**: Low
- **Lane**: Correctness
- **File**: `src/crosshook-native/crates/crosshook-core/src/launch/watchdog/tasks.rs:71–77`
- **Issue**: When `GAMESCOPE_STARTUP_POLL_ITERATIONS` expires without observing the game exe inside the gamescope subtree, the watchdog returns silently. `teardown_reason` then falls through to `NaturalExit` in the finalizer — conflating "game exited cleanly" with "game never started".
- **Fix**: Either accept as-is (watchdog made no intervention, `NaturalExit` is honest) and add a comment on the `return`, or introduce a `WatchdogStandDown` variant that the finalizer maps into `diagnostic_json` distinctly.
- **Status**: Fixed (confirm intentional) — added an explanatory comment at the standdown `return` documenting why `outcome` is intentionally not marked; chose the honest `NaturalExit` attribution rather than a new variant.

---

### F009 — `cancel_reason` maps `Lagged` to `LinkedSessionExit` — may mask `UserRequest`

- **Severity**: Low
- **Lane**: Security / Diagnostics
- **File**: `src/crosshook-native/crates/crosshook-core/src/launch/watchdog/tasks.rs:173–179`
- **Issue**: If the broadcast channel lags (capacity 4; requires ≥ 4 rapid sends), the most-recent signal is discarded and `cancel_reason` returns `LinkedSessionExit`. If the dropped signal was a user-requested teardown, the audit trail misattributes it. Rare in practice.
- **Fix**: Before defaulting, call `try_recv()` once to drain any queued message and return that reason if present. Or lift the capacity (16) and document the rationale.
- **Status**: Fixed — added `cancel_reason_after_lag` helper that peeks past the lag via `try_recv`, recovering a queued `UserRequest` (or similar) instead of flattening it to `LinkedSessionExit`. Wired into both `gamescope_watchdog`'s `tokio::select!` arms and the new `drain_cancel_into_outcome` helper. Added unit tests covering the recover / empty / closed branches.

---

### F010 — `.expect("launch session registry poisoned")` — any panic-in-lock kills the registry

- **Severity**: Low
- **Lane**: Safety
- **File**: `src/crosshook-native/crates/crosshook-core/src/launch/session/registry.rs:40, 48, 61, 89, 106, 129, 144`
- **Issue**: Every method `.expect`s on `Mutex::lock`. If the lock ever poisons (a thread panics while holding it — requires OOM during HashMap ops in practice), every subsequent registry call propagates a secondary panic.
- **Fix**: Accept as designed — the registry has no recoverable degraded state. Add a struct-level doc comment that poison is treated as an unrecoverable invariant violation. Alternative: swap the registry to `RwLock` so reader paths (`sessions_for_profile`, `cancel_linked_children` sender collection) don't share lock-poison contagion with writers.
- **Status**: Fixed — added a **Poison policy** paragraph on the `LaunchSessionRegistry` doc comment explaining the unrecoverable-invariant-violation semantics. Kept `Mutex` (not `RwLock`) since cross-reader contention is not a measured concern for a single-user desktop app.

---

### F011 — `LaunchStreamContext` `session_*` fields are `Option<…>` but always `Some`

- **Severity**: Low
- **Lane**: Maintainability
- **File**: `src/crosshook-native/src-tauri/src/commands/launch/shared.rs:76–78`
- **Issue**: `session_id`, `session_kind`, `session_registry` are wrapped in `Option` but every call site in this PR populates them with `Some(...)`. `finalize_launch_session` immediately destructures all three with `let (Some(...), Some(...), Some(...)) = ... else { return; };` — a `None` is a silent no-op. The `Option` wrapping communicates a capability no code exercises.
- **Fix**: Make them required fields. Or, if kept optional, add an `// INVARIANT:` comment explaining when `None` is legitimate (e.g. pre-#230 code paths that haven't been migrated).
- **Status**: Fixed — `session_id`, `session_kind`, `session_registry` are now required fields on `LaunchStreamContext`; finalizer reads them directly without the nested destructure. Struct doc comment explains the invariant.

---

### F012 — `drain_cancel_on_trainer_no_watchdog` splits cancel-channel semantics across crate boundary

- **Severity**: Low
- **Lane**: Maintainability
- **File**: `src/crosshook-native/src-tauri/src/commands/launch/execution.rs:401–432`
- **Issue**: The helper encodes cancel-channel semantics (mapping `Lagged` to `LinkedSessionExit`, calling `record_reason`) that mirror `cancel_reason()` in `crosshook-core`. Two code paths now interpret broadcast outcomes — one in core, one in the Tauri layer.
- **Fix**: Move the helper (or its channel-to-reason mapping) into `crosshook-core` alongside `cancel_reason`. The `src-tauri` side spawns a thin wrapper that calls the core function.
- **Status**: Fixed — new `crates/crosshook-core/src/launch/session/drain.rs` exposes `drain_cancel_into_outcome(session_id, outcome, cancel_rx)`. Tauri's `execution.rs` imports and spawns the core helper; the private local wrapper is gone.

---

### F013 — `DiagnosticReport.teardown_reason` doc comment inaccurate for the `record_reason` path

- **Severity**: Low
- **Lane**: Documentation
- **File**: `src/crosshook-native/crates/crosshook-core/src/launch/diagnostics/models.rs:21–25`
- **Issue**: The field doc says "Populated by the stream finalizer when the gamescope watchdog fires." But the `record_reason` path (`drain_cancel_on_trainer_no_watchdog` for trainer-without-gamescope) also populates the field without a watchdog. A reader inspecting a `diagnostic_json` row for a non-gamescope trainer teardown would be misled.
- **Fix**: Broaden the comment: _"Populated by the stream finalizer to record why this launch was torn down — set by the gamescope watchdog when it fires, or by the cancel-drain path for trainers without gamescope."_
- **Status**: Fixed — `DiagnosticReport.teardown_reason` doc now covers both paths (watchdog + cancel-drain).

---

### F014 — TypeScript `DiagnosticReport` interface missing `teardown_reason`

- **Severity**: Low
- **Lane**: Type Safety
- **File**: `src/crosshook-native/src/types/diagnostics.ts` (or wherever `DiagnosticReport` is declared frontend-side)
- **Issue**: The Rust side adds `teardown_reason: Option<TeardownReason>`. The TS twin was not updated. Not a runtime breakage — `skip_serializing_if = "Option::is_none"` keeps the field absent from most events. Future TS code reading/filtering on teardown reason gets no type checking.
- **Fix**: Add `teardown_reason?: string` (or a typed string literal union matching the snake_case variants after F001 is resolved) to the TS interface. Add a corresponding Vitest mock update if any test exercises this report shape.
- **Status**: Fixed — exported a typed `TeardownReason` union plus `TEARDOWN_REASONS` readonly array; `DiagnosticReport.teardown_reason?: TeardownReason`; `isDiagnosticReport` type guard validates the field when present. TS typecheck + Vitest (36/36) green.

---

### F015 — TOCTOU window in `kill_remaining_descendants` (acknowledged)

- **Severity**: Info
- **Lane**: Safety
- **File**: `src/crosshook-native/crates/crosshook-core/src/launch/watchdog/tasks.rs:265–283`
- **Issue**: Liveness-check-then-kill window is documented in an existing comment at line 274. Same-UID kills on a desktop system; PID recycle rate is slow.
- **Fix**: No action required — confirmation that the risk was reviewed and accepted.
- **Status**: Accepted

---

## Recommendations before merge

Priority order if addressing before merge:

1. **F001** — `#[serde(rename_all = "snake_case")]` — 1-line fix + test updates. Blocking for consistency with persisted data.
2. **F002 + F003** — unify `drain_cancel_when_watchdog_absent` across both launch paths. Closes the diagnostic blind-spot for non-gamescope sessions. Medium impact, ~30 lines.
3. **F007** — test assertion order-invariance. 2-line fix.
4. **F014** — TypeScript `teardown_reason?: string` — couple with F001.

Defer-friendly (follow-up PR):

- F004 (multi-game parent selection) — design decision; enforce-one-game or insertion-order.
- F005 (register-then-link race) — atomic `register_and_link`.
- F006 (execution.rs split) — refactor-only; no behavior change.
- F010 (Mutex poison policy) — doc-only or RwLock refactor.
- F011 (Option vestigial) — invariant cleanup.
- F012 (drain helper relocation) — cross-crate move.
- F013 (doc accuracy) — comment-only.

Accepted:

- F015 — TOCTOU window already documented.

## Reviewer Attribution

- **correctness-reviewer** → F001, F002 (via F003), F004, F007 (also), F008, F014
- **security-reviewer** → F002, F005, F009, F010, F015
- **quality-reviewer** → F006, F007, F011, F012, F013
