# PR Review #228 — fix(launch): clean up stale gamescope processes after proton_run exits

**Reviewed**: 2026-04-13T21:30:00-04:00
**Mode**: PR (parallel — 3 reviewers: correctness, security/performance, pattern/maintainability)
**Author**: yandy-r
**Branch**: fix/224-stale-gamescope-processes → main
**Decision**: APPROVE (with recommendations)

## Summary

PR #228 addresses issue #224 — gamescope wrapping a Proton launch does not exit when the game exits, leaving lingering clients (mangoapp, winedevice.exe, gamescopereaper) alive indefinitely. The fix adds a gamescope watchdog that polls for the game executable, detects exit, and terminates gamescope through a phased shutdown (SIGTERM → SIGKILL) with scoped descendant cleanup. It also extracts `kill_processes_using_prefix` to `shared.rs` with improved boundary-aware `/proc/environ` matching.

The implementation is correct for the primary deployment scenario (x86_64 AppImage, desktop Linux). No critical or high-severity issues were found. Six medium findings identify formal correctness improvements (atomic ordering, async hygiene, summary consistency) and architectural refinements (core extraction, parameter grouping, named constants). These are recommended for follow-up but are not merge-blocking.

## Validation

| Check                              | Result             |
| ---------------------------------- | ------------------ |
| `cargo test -p crosshook-core`     | pass (all tests)   |
| `cargo clippy -p crosshook-native` | pass (no warnings) |
| Build                              | pass (from PR CI)  |

## Findings

### CRITICAL

None.

### HIGH

None.

### MEDIUM

- **[F001]** `src/crosshook-native/src-tauri/src/commands/launch.rs:865,1207` — `Ordering::Relaxed` on `watchdog_killed` is formally insufficient for cross-thread synchronization between the watchdog task and `finalize_launch_stream`. On x86_64 (TSO) this works in practice, but the Rust memory model permits compiler reordering of `Relaxed` stores relative to subsequent non-atomic operations. On ARM64 targets (potential future support), a speculative read could observe `false` after the process has already exited. [correctness]
  - **Status**: Fixed
  - **Category**: Correctness
  - **Suggested fix**: Change to `Release` on the store (line 1207) and `Acquire` on the load (line 865). Zero-cost on x86_64, one memory barrier on ARM64:

    ```rust
    // watchdog
    killed_flag.store(true, Ordering::Release);
    // finalize
    if watchdog_killed.load(Ordering::Acquire) {
    ```

- **[F002]** `src/crosshook-native/src-tauri/src/commands/launch.rs:858–870` — The watchdog-killed override updates `failure_mode`, `description`, `severity`, and clears `suggestions`, but does not update `report.summary`. The persisted `diagnostic_json` in SQLite will contain `failure_mode = "clean_exit"` but `summary = "Process terminated by SIGTERM."` — a contradiction. Not user-visible (the report is not surfaced for `CleanExit`) but metadata history is misleading. [correctness]
  - **Status**: Fixed
  - **Category**: Correctness
  - **Suggested fix**: Add `report.summary = "Game exited; gamescope compositor cleaned up.".to_string();` to the override block.

- **[F003]** `src/crosshook-native/src-tauri/src/commands/launch.rs:1154–1233` — Blocking synchronous `/proc` scans (`is_process_running`, `collect_descendant_pids`) and subprocess spawns (`host_std_command("kill").status()`) are called directly inside the `async fn gamescope_watchdog` without `spawn_blocking`. This can stall a Tokio worker thread for the duration of each scan (up to 120 polls × full `/proc` reads in Phase 1+2). [performance]
  - **Status**: Fixed
  - **Category**: Performance
  - **Suggested fix**: Wrap `is_process_running` calls in `tokio::task::spawn_blocking` in the polling loops. Same for `collect_descendant_pids` and `kill_remaining_descendants` at teardown:

    ```rust
    let running = tokio::task::spawn_blocking({
        let name = exe_name.to_string();
        move || is_process_running(&name)
    }).await.unwrap_or(false);
    ```

- **[F004]** `src/crosshook-native/src-tauri/src/commands/launch.rs:1139–1297` — ~165 lines of watchdog logic (`gamescope_watchdog`, `collect_descendant_pids`, `is_pid_alive`, `kill_remaining_descendants`) lives in the Tauri command layer. CLAUDE.md states "Business logic lives in `crosshook-core`. Keep `src-tauri` thin (IPC and CLI only)." None of these functions depend on Tauri types — they use only `tokio`, `std::fs`, `std::path`, and `crosshook_core::platform`. [pattern-compliance]
  - **Status**: Fixed
  - **Category**: Architecture
  - **Suggested fix**: Extract to `crosshook_core::launch::watchdog` module. Keep only `spawn_gamescope_watchdog` in `launch.rs` as a one-line `tauri::async_runtime::spawn` shim. This also unlocks unit testing without a Tauri runtime.

- **[F005]** `src/crosshook-native/src-tauri/src/commands/launch.rs:560–597,832–870` — `spawn_log_stream` and `finalize_launch_stream` now take 11 positional parameters each, threaded through 4 functions. Positional confusion risk is high and grows with each addition. [maintainability]
  - **Status**: Fixed
  - **Category**: Maintainability
  - **Suggested fix**: Group the metadata/snapshot parameters into a `LaunchStreamContext` struct:

    ```rust
    struct LaunchStreamContext {
        metadata_store: MetadataStore,
        operation_id: Option<String>,
        steam_app_id: String,
        trainer_host_path: Option<String>,
        profile_name: Option<String>,
        steam_client_path: String,
        watchdog_killed: Arc<AtomicBool>,
    }
    ```

- **[F006]** `src/crosshook-native/src-tauri/src/commands/launch.rs:1159–1231` — Five distinct timing constants (60 iterations / 120s startup, 2s poll interval, 5s grace, 3s SIGTERM wait, 500ms descendant delay) are inline magic numbers. [maintainability]
  - **Status**: Fixed
  - **Category**: Maintainability
  - **Suggested fix**: Define named module-level constants:

    ```rust
    const GAMESCOPE_STARTUP_POLL_ITERATIONS: u32 = 60;
    const GAMESCOPE_POLL_INTERVAL: Duration = Duration::from_secs(2);
    const GAMESCOPE_NATURAL_EXIT_GRACE: Duration = Duration::from_secs(5);
    const GAMESCOPE_SIGTERM_WAIT: Duration = Duration::from_secs(3);
    const GAMESCOPE_DESCENDANT_CLEANUP_DELAY: Duration = Duration::from_millis(500);
    ```

### LOW

- **[F007]** `src/crosshook-native/src-tauri/src/commands/launch.rs:1289` — TOCTOU race in `kill_remaining_descendants`: `is_pid_alive` check precedes `kill -KILL` with a window for PID reuse. Practical risk is minimal (same-UID constraint, desktop fork rates), but no structural defense exists. [security]
  - **Status**: Fixed
  - **Category**: Security
  - **Suggested fix**: Document the accepted risk. For defense-in-depth, consider using `nix::sys::signal::kill` (crate already in `src-tauri/Cargo.toml`) to perform the signal delivery as a syscall instead of shelling out.

- **[F008]** `src/crosshook-native/src-tauri/src/commands/launch.rs:1177–1186` — Phase 2 uses system-wide `is_process_running(exe_name)` scan. If another unrelated process shares the same executable name, the watchdog could trigger early or late. [correctness]
  - **Status**: Fixed
  - **Category**: Correctness
  - **Suggested fix**: Scope Phase 1+2 matching to descendants of `gamescope_pid` rather than a system-wide scan. The existing `collect_descendant_pids` infrastructure already provides the tree walk. This is a more significant refactor suitable for follow-up.

- **[F009]** `src/crosshook-native/src-tauri/src/commands/launch.rs:395–397` — `gamescope_active` check does not consider the launch method. For `METHOD_STEAM_APPLAUNCH`, gamescope is not the direct child (the shell script is). The watchdog is harmlessly spawned and self-terminates on the first `is_pid_alive` check, but the intent is wrong. [correctness]
  - **Status**: Fixed
  - **Category**: Correctness
  - **Suggested fix**: Guard with `method == METHOD_PROTON_RUN`:

    ```rust
    let gamescope_active = method == METHOD_PROTON_RUN
        && request.gamescope.enabled
        && (request.gamescope.allow_nested || !crosshook_core::launch::is_inside_gamescope_session());
    ```

- **[F010]** `src/crosshook-native/src-tauri/src/commands/launch.rs:1146` — Empty `exe_name` silently disables the watchdog with no log output. If a trailing slash in `game_path` causes `file_name()` to return `None`, the user has no indication that gamescope will linger. [correctness]
  - **Status**: Fixed
  - **Category**: Correctness
  - **Suggested fix**: Add `tracing::warn!` at the early return.

- **[F011]** `src/crosshook-native/src-tauri/src/commands/launch.rs:1243–1284` — `collect_descendant_pids`, `is_pid_alive`, and `is_process_running` have no unit tests despite non-trivial logic (BFS tree walk, TASK_COMM_LEN fallback). [maintainability]
  - **Status**: Fixed
  - **Category**: Test Coverage
  - **Suggested fix**: Extract testable pure-logic helpers. If F004 (core extraction) is done, testing becomes straightforward without a Tauri runtime.

### INFO

- **[F012]** `src/crosshook-native/src-tauri/src/commands/shared.rs:64` — `environ_entry_contains_prefix_path` boundary matching is correct. One untested edge case: a bare path entry with no `KEY=` prefix (entry is literally `/foo/bar`). The `after == entry.len()` branch handles this correctly but has no explicit test.
  - **Status**: Fixed
  - **Category**: Test Coverage
  - **Suggested fix**: Add test case for `b"/foo/bar"` as a bare entry.

- **[F013]** `src/crosshook-native/src-tauri/src/commands/shared.rs:130` — `kill_processes_using_prefix` reads full `/proc/[pid]/environ` blobs with no size cap. Existing pattern, not new to this PR, and only called at teardown.
  - **Status**: Open
  - **Category**: Performance (pre-existing)

## Recommendations

**Quick wins (trivial, pre-merge or fast follow-up):**

1. F001: `Relaxed` → `Release`/`Acquire` — one-line change each site
2. F002: Add `report.summary` to the override block — one line
3. F010: Add `tracing::warn!` for empty exe_name — one line

**Follow-up (post-merge, tracked):** 4. F003: Wrap blocking calls in `spawn_blocking` 5. F004 + F005 + F006: Extract watchdog to core module, introduce `LaunchStreamContext` struct, name timing constants 6. F008 + F009: Scope watchdog to descendants and guard against non-proton_run methods 7. F011 + F012: Add test coverage for watchdog utilities and prefix matching edge cases
