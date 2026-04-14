# Fix Report: pr-228-review

**Source**: `docs/prps/reviews/pr-228-review.md`
**Applied**: 2026-04-13T20:06:35-04:00
**Mode**: Parallel (2 batches, max width 1)
**Severity threshold**: all

## Summary

- **Total findings in source**: 13
- **Already processed before this run**:
  - Fixed: 0
  - Failed: 0
- **Eligible this run**: 12
- **Applied this run**:
  - Fixed: 12
  - Failed: 0
- **Skipped this run**:
  - Below severity threshold: 0
  - No suggested fix: 1
  - Missing file: 0

## Fixes Applied

| ID   | Severity | File                                    | Line     | Status | Notes                                                                                                                                  |
| ---- | -------- | --------------------------------------- | -------- | ------ | -------------------------------------------------------------------------------------------------------------------------------------- |
| F001 | MEDIUM   | `src-tauri/src/commands/launch.rs`      | 865,1207 | Fixed  | `watchdog_killed` now uses `Release`/`Acquire` synchronization across the watchdog and finalize paths.                                 |
| F002 | MEDIUM   | `src-tauri/src/commands/launch.rs`      | 858-870  | Fixed  | Clean-exit override now updates `report.summary` alongside the exit metadata.                                                          |
| F003 | MEDIUM   | `crosshook-core/src/launch/watchdog.rs` | 1        | Fixed  | Blocking `/proc` scans, descendant cleanup, and signal delivery now run off the async worker thread via `tokio::task::spawn_blocking`. |
| F004 | MEDIUM   | `crosshook-core/src/launch/watchdog.rs` | 1        | Fixed  | Watchdog logic moved out of `src-tauri` into `crosshook-core::launch::watchdog`; Tauri keeps only the spawn shim.                      |
| F005 | MEDIUM   | `src-tauri/src/commands/launch.rs`      | 89       | Fixed  | `LaunchStreamContext` replaces the long positional argument thread through the launch stream helpers.                                  |
| F006 | MEDIUM   | `crosshook-core/src/launch/watchdog.rs` | 1        | Fixed  | Gamescope timing values are now named module constants.                                                                                |
| F007 | LOW      | `crosshook-core/src/launch/watchdog.rs` | 1        | Fixed  | Added an explicit accepted-risk comment for the low-probability PID reuse TOCTOU window.                                               |
| F008 | LOW      | `crosshook-core/src/launch/watchdog.rs` | 1        | Fixed  | Phase 1 and Phase 2 now scope executable detection to descendants of the tracked gamescope PID.                                        |
| F009 | LOW      | `src-tauri/src/commands/launch.rs`      | 345      | Fixed  | Watchdog activation now requires `METHOD_PROTON_RUN`.                                                                                  |
| F010 | LOW      | `src-tauri/src/commands/launch.rs`      | 1058     | Fixed  | Empty executable-name cases now emit a warning before the watchdog stands down.                                                        |
| F011 | LOW      | `crosshook-core/src/launch/watchdog.rs` | 290      | Fixed  | Added unit tests for parent-PID parsing, descendant BFS traversal, and the `TASK_COMM_LEN` cmdline fallback.                           |
| F012 | INFO     | `src-tauri/src/commands/shared.rs`      | 176      | Fixed  | Added explicit coverage for a bare `/foo/bar` environ entry.                                                                           |

## Files Changed

- `src/crosshook-native/crates/crosshook-core/src/launch/watchdog.rs` (F003, F004, F006, F007, F008, F011)
- `src/crosshook-native/crates/crosshook-core/src/launch/mod.rs` (F004 export wiring)
- `src/crosshook-native/src-tauri/src/commands/launch.rs` (F001, F002, F005, F009, F010)
- `src/crosshook-native/src-tauri/src/commands/shared.rs` (F012)

## Failed Fixes

None.

## Validation Results

| Check      | Result |
| ---------- | ------ |
| Type check | Pass   |
| Tests      | Pass   |

## Next Steps

- Re-run `$code-review 228` to verify the remaining open finding (`F013`) is still the only follow-up.
- Use `$git-workflow` when you want to commit the fixes.
