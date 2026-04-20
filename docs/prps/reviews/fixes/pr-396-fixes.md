# PR #396 — Review fixes report

**Source review**: [`docs/prps/reviews/pr-396-review.md`](../pr-396-review.md)
**Invocation**: `/ycc:review-fix --parallel --severity low 396`
**Worktree**: `~/.claude-worktrees/crosshook-trainer-watchdog-parity/` (PR head, branch `feat/trainer-watchdog-parity`)
**Date**: 2026-04-20

## Severity filter

`--severity low` → fix all findings at Low and above. Info-level findings (F015) are not targeted by this run.

## Outcome

| Status   | Count | Findings                                                                     |
| -------- | ----- | ---------------------------------------------------------------------------- |
| Fixed    | 13    | F001, F002, F003, F004, F005, F007, F008, F009, F010, F011, F012, F013, F014 |
| Deferred | 1     | F006 (execution.rs split — follow-up refactor, tracked separately)           |
| Accepted | 1     | F015 (Info — TOCTOU acknowledged; no action required)                        |

## Validation

| Check                                                                             | Result                                                                               |
| --------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------ |
| `cargo build -p crosshook-core -p crosshook-native`                               | clean                                                                                |
| `cargo clippy -p crosshook-core -p crosshook-native --all-targets -- -D warnings` | clean                                                                                |
| `cargo test -p crosshook-core`                                                    | **1123 lib + 8 integration + 25 supplementary** green (was 1117 — 6 new tests added) |
| `npm run typecheck`                                                               | clean                                                                                |
| `npm test` (Vitest)                                                               | 36/36 green                                                                          |
| `./scripts/check-host-gateway.sh`                                                 | pass (verified separately during PR review)                                          |

## Change inventory

### Rust core (`crates/crosshook-core`)

- `src/launch/session/types.rs` — `TeardownReason` gains `#[serde(rename_all = "snake_case")]` (F001). `SessionEntry` gains `registered_at: Instant` (F004). `WatchdogOutcome::record_reason()` already existed from the original PR but is reused by the relocated drain helper (F012).
- `src/launch/session/registry.rs` — **Poison policy** paragraph on the `LaunchSessionRegistry` doc comment (F010). `sessions_for_profile` sorts most-recent-first via `Reverse(registered_at)` (F004). New `register_and_link_to_parent_of_kind(kind, profile_key, parent_kind)` atomic method that performs register + candidate lookup + link under a single lock (F005). `sessions_for_profile_filters_by_kind` test assertions rewritten to `len() + contains` (F007). Three new tests: `sessions_for_profile_returns_most_recent_first`, `register_and_link_to_parent_of_kind_attaches_most_recent_parent`, `register_and_link_to_parent_of_kind_returns_none_when_no_candidate`, `register_and_link_to_parent_of_kind_rejects_illegal_pairings`.
- `src/launch/session/drain.rs` — **new file** hosting `drain_cancel_into_outcome(session_id, outcome, cancel_rx)` (F012). Inline tests for the cascade-records-reason and closed-channel-no-op paths.
- `src/launch/session/mod.rs` — re-exports `drain_cancel_into_outcome`.
- `src/launch/mod.rs` — surfaces `drain_cancel_into_outcome` on the public `launch::` API.
- `src/launch/diagnostics/models.rs` — broadened `teardown_reason` doc comment to cover both watchdog and cancel-drain populations (F013).
- `src/launch/watchdog/tasks.rs` — standdown comment (F008). `cancel_reason_after_lag(&mut receiver)` introduced to peek past a lag via `try_recv` (F009). Polling `tokio::select!` arms inline-match `recv()` results and call `cancel_reason_after_lag` on `Lagged`. Obsolete `cancel_reason` removed; replacement test cases cover recover / empty / closed via the new helper.
- `src/launch/watchdog/mod.rs` — `pub(crate) use tasks::cancel_reason_after_lag` for the drain helper import.
- `tests/trainer_watchdog_parity.rs` — updated string literals to match snake_case (F001) and added an explicit `linked_session_exit` assertion to the receiver-closed test.

### Tauri layer (`src-tauri`)

- `src/commands/launch/shared.rs` — `LaunchStreamContext` fields `session_id`, `session_kind`, `session_registry` are now required (non-Option) with a struct-level invariant doc (F011).
- `src/commands/launch/streaming.rs` — `finalize_launch_session` reads required fields directly (no destructuring). `was_killed` branch keeps the session-aware summary.
- `src/commands/launch/execution.rs` — `launch_trainer` uses the new atomic `register_and_link_to_parent_of_kind` (F005). Both `launch_game` and `launch_trainer` delegate cancel-receiver plumbing to a new `consume_cancel_channel` helper (F002, F003) that spawns either `spawn_gamescope_watchdog` or `drain_cancel_into_outcome` from `crosshook-core` (F012). A tracing warning fires if `gamescope_active && child_pid.is_none()` and falls back to drain rather than dropping the receiver.

### Frontend (`src/`)

- `src/types/diagnostics.ts` — new `TEARDOWN_REASONS` readonly array + `TeardownReason` union type. `DiagnosticReport.teardown_reason?: TeardownReason` (F014). `isDiagnosticReport` validates the field when present.

## Open / deferred findings

- **F006** — `execution.rs` at 539 lines (down from 549 but still ~8% past the 500-line soft cap). The `consume_cancel_channel` helper consolidated the cancel-receiver branching but the two `#[tauri::command]` handlers still share duplicated warnings-collection / method-resolution / snap-variable preamble. Track as a follow-up refactor (`collect_launch_warnings` + `resolve_method_str` extraction).

## Not covered by this run

- **F015** (Info — `kill_remaining_descendants` TOCTOU): explicitly accepted with an in-code comment on the existing PR. No action taken.

## Next steps

1. `cd ~/.claude-worktrees/crosshook-trainer-watchdog-parity && git status` — confirm clean.
2. Commit the fixes (single `refactor(launch)` + `fix(launch)` or one combined `refactor(launch): address PR #396 review findings` — fix commit is atomic for rollback).
3. Re-run `/ycc:code-review --parallel 396` to verify no regressions (optional — the existing artifact statuses are already updated in place).
4. Drop F006 into a follow-up issue.
