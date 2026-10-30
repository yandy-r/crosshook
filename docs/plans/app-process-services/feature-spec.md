# Feature Spec: install, update, run-executable and prefix-deps services in `crosshook-app`

Tracking: Linear YAN-815 (M1-31) + YAN-1538, target `crosshook v0.7.0-alpha.1`, one PR to `main`, worktree `crosshook-yan-815`. Paths are relative to `src/crosshook-native/`. The binding decisions in [decisions.md](./decisions.md) (D1–D15) override research recommendations. Exact signatures are in [b0-contracts.md](./b0-contracts.md) and the batches in [app-process-services.plan.md](./app-process-services.plan.md).

## Problem

There are 16 Tauri commands in `src-tauri/src/commands/{install,update,run_executable,prefix_deps}.rs`, and each set carries its own process supervision:

- **Ad hoc PID state.** `UpdateProcessState` and `RunExecutableProcessState` store a numeric PID. Cancel runs host `kill <pid>`, which under Flatpak signals an unrelated PID in the host namespace (security H1).
- **Slow, misordered log streaming.** Logs come from a 500 ms whole-file poll (`log_stream.rs`) that emits completion before the final flush.
- **Blocking install.** `install_game` runs `block_on(child.wait())` inside core with no cancel.
- **No `--watch-bus` for Proton children.** Proton-based children are built without `--watch-bus` (H2). `check_installed` is also built without it (YAN-1538 / H5).
- **One global prefix-deps lock.** `PrefixDepsInstallLock` is a single global slot, so install-game, update and run-exe ignore it (M5). Two ad-hoc runs with the same executable stem share one throwaway prefix and kill each other (H3).
- **Prefix-deps pipe readers stop on invalid UTF-8**, which can hang the install and hold the lock forever (H4).
- **No journal recovery.** The Windows-version restore journal is never recovered on startup. A delete failure on the install path leaves the row `pending`. A replayed `binary_path` is not re-validated (M4).
- **Untested logic.** The protontricks→winetricks fallback is duplicated and untested.
- **Run-executable log has no listener.** `run-executable-log` has no frontend listener, and `useRunExecutable.ts`/`useUpdateGame.ts` carry complete-before-invoke workarounds.

## Scope

**Services (D1).** Four `#[derive(Clone)]` services under `crates/crosshook-app/src/services/{install,update,run_executable,prefix_deps}/`:

- `InstallService`, `UpdateService`, `RunExecutableService`, `PrefixDepsService`.
- Each has a positional `new()`, a `#[cfg(test)] with_seams()`, async methods, `run_blocking` for store and filesystem work, `AppError` only, a `tests/` directory, files of about 500 lines or fewer, and `assert_impl_all!(…: Send, Sync)`.
- Shared process plumbing lives in `services/process/`.
- `Services` gains four fields. `src-tauri` `.manage()`s them.

**Removed.** `UpdateProcessState`, `RunExecutableProcessState`, `PrefixDepsInstallState`, core `prefix_deps::lock`, `log_stream.rs`, host-kill-by-PID, `runner::install_packages` and `build_prefix_dep_command`.

**Core changes (D3).** Core exposes prepared-command builders and a lifecycle Proton builder. The app owns spawn and supervision:

- spawn uses `process_group(0)` and `kill_on_drop(true)`;
- `supervise_child` gains a second "kill now" token (D4);
- pipe drains are bounded and lossy (D4);
- every flow holds a per-prefix `JobKey` (D5).

**`PrefixVersionJournal` (D7).** The journal recovers on startup (D8) and before each install. `PrefixVersionRepairStatus` gains `in_progress` and `last_recovery` (D9).

**Run-executable UI (D10).** The Run executable panel shows live log lines, and both workarounds are removed.

## Acceptance criteria (verbatim) and evidence

| #      | Criterion (issue text)                                                                                                                                   | Evidence (test)                                                                                                                                                                                                                                                                                               |
| ------ | -------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 815-1  | Journal recovery tests cover a crash after the version change and before the restore, a crash after the restore, a failed restore, and delete decisions. | `services/prefix_deps/tests/journal.rs` J1a–J1c (crash after change), J2a–J2b (crash after restore / before spawn), J3a–J3f (failed restore incl. not-auto-retried and install blocked), J4a–J4h (`decide`, delete-fail → `failed`, corrupt/invalid `binary_path` never executed, live prefix left `pending`) |
| 815-2  | Tests cover protontricks-to-winetricks fallback selection and Steam App ID validation.                                                                   | `services/prefix_deps/tests/tools.rs` S1–S5 (`parse_nonzero_steam_app_id`: empty, spaces, `0`, `00`, non-digit, `-1`, `+5`, `--help`, `4294967296` → None; `" 12345 "`, `"007"`); `resolve_tool` fallback matrix                                                                                              |
| 815-3  | Tests show that per-prefix exclusivity rejects a second concurrent install on the same prefix.                                                           | `services/prefix_deps/tests/exclusivity.rs` L1 (same prefix and symlink alias → `Conflict`, one spawn, `job_counts()==(1,_)`), L2 (other prefix allowed), L3 (repair/startup vs install), cross-service (`run_executable`/`update` on a held prefix → `Conflict`)                                             |
| 815-4  | The Run executable UI shows live log lines, and `run-executable-log` has a listener.                                                                     | `hooks/__tests__/useRunExecutable.test.ts` (lines accumulate, capped, foreign `run_id` ignored); `components/__tests__/RunExecutablePanel.test.tsx` (`emitMockEvent('run-executable-log')` → text in `role="log"`); `a11y/routes.a11y.test.tsx` stays green                                                   |
| 815-5  | No `setTimeout`/flag workaround for complete-before-invoke remains.                                                                                      | `useRunExecutable.test.ts` "complete before invoke resolves keeps final stage"; grep gate in the plan: no `NO_EXIT_RECEIVED`, `exitHolder`, `invokeResolved` or `completedBeforeInvoke` in `src/`                                                                                                             |
| 815-6  | `./scripts/check-host-gateway.sh` passes.                                                                                                                | Final gate; new services spawn only through `crosshook_core::platform` lifecycle builders                                                                                                                                                                                                                     |
| 1538-1 | `check_installed` uses the lifecycle-bound builder, so the Flatpak argv contains `--watch-bus` right after `--host`.                                     | core `prefix_deps/runner/tests` `check_command_flatpak_argv_has_watch_bus_after_host`                                                                                                                                                                                                                         |
| 1538-2 | Existing `prefix_deps::` tests pass.                                                                                                                     | `cargo test -p crosshook-core prefix_deps`                                                                                                                                                                                                                                                                    |
| 1538-3 | `./scripts/check-host-gateway.sh` passes.                                                                                                                | Final gate                                                                                                                                                                                                                                                                                                    |

Test plan items from the issue: `cargo test -p crosshook-app` and `npm run test:smoke` run in the final gate. The manual items (install a dependency; kill CrossHook mid-install and restart; run and cancel an executable; Flatpak) are listed as unperformed in the PR (D15).

## Non-goals (D15)

- Native env clearing (L2).
- A Rust-aware gateway lint for `Command::new(<variable>)` (L6).
- Splitting core `prefix_deps/runner.rs` beyond what YAN-1538 touches. Note that B1 does split it into `runner/` submodules, because edits there require it (file-size rule). This does not change behaviour.
- ~~Narrowing `kill_processes_using_prefix` (M2)~~: done during review (see [decisions.md](./decisions.md) § _Changes made during review_).
- ~~`/tmp/crosshook-logs` ownership/`O_NOFOLLOW` hardening (M3)~~: done during review.
- Host-side `wineserver -k` under Flatpak.
- An install-game UI cancel button.
- Any new IPC command.
- Any schema/TOML change.

## Decisions summary

| D   | Summary                                                                                                                                                                                                                                                                                                                                                                                |
| --- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| D1  | Four service dirs plus shared `services/process/`; `Services` fields; `.manage()` in src-tauri; ad hoc states deleted.                                                                                                                                                                                                                                                                 |
| D2  | 16 command names, camelCase args and event names frozen. Results/payloads gain only `run_id`/`op_id`. `RunCommandError` wire shape kept. `update-log` stays a bare string. `prefix-dep-*` echo the raw `prefix_path`. A pin test per command file.                                                                                                                                     |
| D3  | Core returns prepared `Command`s (`prepare_install_command`, `prepare_run_executable`, `build_update_command`, `PreparedPrefixDependencyInstall::into_parts`, check/restore command builders) and a lifecycle Proton builder. The install `block_on` is deleted. The app spawns with `process_group(0)`/`kill_on_drop(true)`. Behaviour change: quitting CrossHook ends host children. |
| D4  | Every flow is a `JobRegistry` job. Cancel = TERM→grace→KILL on the group. Stop = immediate group SIGKILL via the new `supervise_child_with_kill` second token. No stored PIDs, no host `kill <pid>`. Bounded lossy drains. Completion is published after the final log drain.                                                                                                          |
| D5  | `JobKey("prefix:<canonical pfx>")` is shared by prefix-deps install/repair/recovery, install-game, update and run-exe (including throwaway). `Conflict` on collision. A unique throwaway dir is used per run, and the key is held through cleanup. Check takes no key.                                                                                                                 |
| D6  | Cancel/stop keep no-arg IPC. Each service keeps an `ActiveJobSlot { job_id, owner }`. Foreign owner or no job → `Ok(false)` (IPC `Ok(())`).                                                                                                                                                                                                                                            |
| D7  | `PrefixVersionJournal` over `JournalStore`/`PrefixVersionRestorer` with `status/record/resolve/decide/recover/recover_before_install/recover_on_startup`. Invariants I1–I14 hold, and `binary_path` is re-validated before replay. Adds `MetadataStore::list_prefix_version_restores()`.                                                                                               |
| D8  | A `prefix-journal-recovery` startup job (not a `StartupJobKind`) runs and is skipped under `CROSSHOOK_BENCH=1` or when metadata is unavailable. It retries `pending` rows only, holds the prefix key, and skips live prefixes. Pre-install recovery runs inside the install job.                                                                                                       |
| D9  | `PrefixVersionRepairStatus.in_progress` and `last_recovery: Option<{outcome, message}>` are runtime-only. The panel shows a note for `restored` and the alert for `failed`.                                                                                                                                                                                                            |
| D10 | The run-exe panel subscribes once on mount, filters by `run_id`, and renders lines in `role="log"` (text only, capped). Workarounds are removed in both hooks. Mocks emit lines and completion. Copy is fixed.                                                                                                                                                                         |
| D11 | Logs go to `/tmp/crosshook-logs` via `create_log_file_in` (returns the `create_new` handle). Child stdio is a `try_clone` of that handle. Tail via `log_tail::follow`. `log_stream.rs` is deleted.                                                                                                                                                                                     |
| D12 | `offline_mode` → `install_prefix_dependency` returns `Unavailable "offline mode is enabled"` before any job. Other flows are ungated.                                                                                                                                                                                                                                                  |
| D13 | Pure `resolve_tool`. `parse_nonzero_steam_app_id` moves to core `prefix_deps::validation`. `check_installed` uses the lifecycle builder, with an argv test.                                                                                                                                                                                                                            |
| D14 | Critical tests only: journal, tools, exclusivity, cancel; run-exe unique dir and foreign-owner; frontend hook tests. Fakes honour their arguments.                                                                                                                                                                                                                                     |
| D15 | Out of scope; follow-ups are filed by Main.                                                                                                                                                                                                                                                                                                                                            |

## Intentional behaviour changes (PR must state them)

- Quitting CrossHook ends a running installer, updater, ad-hoc executable or winetricks run on the host.
- Different prefixes install in parallel. The same prefix across all flows gets `Conflict`.
- Cancel now signals the process group, and stop kills it.
- `install_game` honours app-shutdown cancellation.
- A crashed `pending` journal row is restored automatically on the next start.
- An install-path journal delete failure now marks the row `failed`.
- An invalid replay `binary_path` marks the row `failed`.
- `install_prefix_dependency` is refused offline.
- Run-exe and update completion payloads gain `run_id`, and the run-exe log payload becomes `{run_id, line}`.
- Ad-hoc throwaway prefixes get a unique per-run suffix.
- Added during review: a second `update_game`/`run_executable` start from the same window gets `Conflict`; a spawn failure fails the start call; relative installer, updater, executable, Proton or prefix paths are rejected; the prefix kill sweep matches only `WINEPREFIX`/`STEAM_COMPAT_DATA_PATH`, signals locally, spares CrossHook and its ancestors and is skipped under Flatpak; under Flatpak startup recovery treats every prefix as in use; quitting waits up to `App::DEFAULT_SHUTDOWN_TIMEOUT` (kill grace + `DRAIN_GRACE` + 1 s = 8 s) for cancelled jobs, and `SIGINT`/`SIGTERM`/`SIGHUP` take the same exit path. Making `protonup_install_version` observe cancel (so it no longer holds quit for the full bound) is a follow-up.

## Persistence classification

- **SQLite metadata, unchanged schema, no migration:** `prefix_dependency_state` and `prefix_version_restore_journal` (v27). The only addition is the read-only `list_prefix_version_restores()` SELECT.
- **TOML settings, read-only here:** `protontricks_binary_path`, `offline_mode`.
- **Runtime-only:** job registry entries, active-job slots, `last_recovery` map, log tails.
- **Ephemeral files:** logs in `/tmp/crosshook-logs`.

No schema bump, so `check-schema-version.sh` stays green.

## Risks

| Risk                                                      | Mitigation                                                                                                                                                                                                                                                                                                                   |
| --------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Parallel tasks clobber shared files                       | Exact per-batch ownership in the plan. Absolute worktree paths. Shared wiring files have one owner per batch.                                                                                                                                                                                                                |
| Restore bound to the job token gets killed on user cancel | Restore runs on its own token (90 s timeout). Only app shutdown cancels an install, and then the restore is skipped (a restore started during the bounded shutdown would outlive the process) and the row stays `pending` for startup recovery (C1 test). Repair and startup recovery do not start a restore once cancelled. |
| Event relay misses early lines                            | `JobRegistry::start` subscribes before spawn. The service awaits a `started` oneshot before returning the handle.                                                                                                                                                                                                            |
| Frontend listeners silently drop prefix events            | The relay echoes the raw request `prefix_path` and `profile_name`. A relay unit test pins it.                                                                                                                                                                                                                                |
| Startup recovery races a user install                     | Same prefix key. A `Conflict` makes recovery skip, and the install's `recover_before_install` handles it.                                                                                                                                                                                                                    |
| Flatpak sweep (`kill_processes_using_prefix`) is a no-op  | Now skipped explicitly under Flatpak (review change). Group kill plus `--watch-bus` is the real teardown.                                                                                                                                                                                                                    |
