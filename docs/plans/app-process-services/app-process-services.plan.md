# Plan: app-process-services (YAN-815 / M1-31 + YAN-1538)

Paths are relative to `src/crosshook-native/`. `core/` means `crates/crosshook-core/src`, `app/` means `crates/crosshook-app/src`, and `tauri/` means `src-tauri/src`.

Inputs:

- Binding decisions: [decisions.md](./decisions.md), D1–D15.
- [feature-spec.md](./feature-spec.md).
- Signatures: [b0-contracts.md](./b0-contracts.md). Read it before any task.

## Rules for every task

- **Worktree.** Use the absolute root `/home/yandy/Projects/github.com/yandy-r/crosshook-yan-815`, and resolve every path absolutely. Never touch the main checkout. Never run `git stash/checkout/reset/commit`.
- **Ownership.** Tasks in a batch own **disjoint files**, including `mod.rs` and `tests/mod.rs`. A file not listed under a task's **Owns** is read-only for that task. Report a wrong contract to Main; never edit a sibling's file.
- **Verification.** Subagents skip build, test, lint and format. Main runs each batch gate once after the batch lands.
- **Code rules.**
  - Files stay at about 500 lines or fewer.
  - No `#[allow(dead_code)]`.
  - No stubs.
  - No `Command::new("winetricks"|"protontricks"|"proton"|"flatpak-spawn"…)`.
  - Tests use fake `sh` scripts with other names.
  - No timing sleeps in tests. Gate with `Notify`/oneshot.
  - Fakes must honour and assert their arguments.
- **Intermediate compile rule.**
  - B1 adds new core APIs **alongside** the old ones, so `src-tauri` keeps compiling until B3. The old ones are deleted in B4 (4.1).
  - The B1 and B2 gates use `cargo check` (warnings allowed for not-yet-used items). From B3 on, `clippy -D warnings` is required.

## Batches

| Batch | Tasks    | Depends on | Contents                                                                                                                  |
| ----- | -------- | ---------- | ------------------------------------------------------------------------------------------------------------------------- |
| P0    | 0.1      | —          | Wire-shape goldens and IPC pins captured from **unmodified** code                                                         |
| B0    | planning | research   | spec, b0, this plan (done)                                                                                                |
| B1    | 1.1–1.7  | P0         | Core builders (lifecycle Proton, prepared commands, YAN-1538), metadata list, app job primitives, shared process plumbing |
| B2    | 2.1–2.6  | B1         | Four services and critical tests, wiring, startup recovery                                                                |
| B3    | 3.1–3.5  | B2         | Tauri adapters/cutover, frontend, mocks                                                                                   |
| B4    | 4.1–4.2  | B3         | Delete obsolete core API; docs                                                                                            |

---

## Task 0.1: Golden capture (P0)

- **Owns:**
  - `tauri/commands/fixtures/process/{run_command_error_validation,run_command_error_runtime,prefix_dep_log,prefix_dep_complete,prefix_version_repair_status}.json` (new)
  - append-only to the `#[cfg(test)] mod tests` blocks of `tauri/commands/run_executable.rs` and `tauri/commands/prefix_deps.rs`
- **Change:**
  - Serialize with today's types and assert equality with the fixture files (`include_str!` + `serde_json::Value` compare):
    - `RunCommandError::from(RunExecutableValidationError::ExecutablePathMissing)`
    - `RunCommandError::Runtime{message:"boom"}`
    - `DepLogPayload{profile_name:"p",prefix_path:"/raw/prefix",line:"l"}`
    - `completion_payload("p","/raw/prefix",outcome(true, Failed, Some("e")))`
    - `PrefixVersionRepairStatus{required:true,state:Some(Pending),last_error:None}`
  - B3 re-runs the same fixtures against the moved types. The repair-status fixture is updated **only** by adding `in_progress:false,last_recovery:null` (additive check).
- **Acceptance:** `cargo test -p crosshook-native fixture` passes on unmodified code.

### P0 gate

```sh
cargo test -p crosshook-native run_executable
cargo test -p crosshook-native prefix_deps
```

---

## B1: foundations

### Task 1.1: lifecycle Proton builder (D3, H2)

- **Owns:** `core/platform/gateway.rs`, `core/platform/mod.rs`, `core/platform/tests/gateway.rs`, `core/launch/runtime_helpers/proton_command.rs`, `core/launch/runtime_helpers/mod.rs`, `core/launch/runtime_helpers/tests/proton_command.rs`.
- **Change:**
  - Make `host_command_with_env_and_directory_lifecycle_inner` `pub(crate)` and re-export it.
  - Add `build_direct_proton_command_lifecycle` and `_with(flatpak)` (b0 §1).
  - Do not change the existing builders.
- **Tests:**
  - The flatpak argv starts `flatpak-spawn --host --watch-bus --clear-env` and ends `<proton> run`.
  - The native argv is `<proton> run` with env applied.
  - `--directory=` is present when a dir is given.

### Task 1.2: core prefix_deps split + YAN-1538 (D3, D13, H4/H5)

- **Owns:** `core/prefix_deps/runner.rs` → delete it and create `core/prefix_deps/runner/{mod,command,snapshot,check,install,restore,sanitize}.rs`, `core/prefix_deps/runner/tests/{mod,check,install,restore,snapshot,normalize,sanitize}.rs`, plus `core/prefix_deps/validation.rs` and `core/prefix_deps/mod.rs`.
- **Change:**
  - Split the file without changing behaviour. Move all existing tests verbatim.
  - Add `build_check_command_with(…, flatpak)`. `check_installed` must use the lifecycle builder.
  - Delete `build_prefix_dep_command` and the `HostCommandBuilder` indirection.
  - The install command gets `process_group(0)` and null stdin.
  - Add `PreparedPrefixDependencyInstall::into_parts`.
  - Make `is_windows_version_verb` `pub` and `restore_prefix_windows_version_with_timeout` `pub(crate)`.
  - Move `parse_nonzero_steam_app_id` into `validation.rs`, with its tests (S1 table).
  - Keep `spawn`, `PrefixDependencyInstall`, `install_packages` and `lock` until 4.1.
- **Tests:**
  - YAN-1538: `check_command_flatpak_argv_has_watch_bus_after_host` (argv[0..3] == `flatpak-spawn --host --watch-bus`, ends `list-installed`; protontricks variant has the app id before it).
  - `into_parts_command_leads_its_process_group` (spawn the fake binary, `getpgid(pid)==pid`).

### Task 1.3: core install + update prepared builders (D3)

- **Owns:** `core/install/{service,mod}.rs`, new `core/install/tests.rs` (move `service.rs` tests here so the file stays ≤500 lines), `core/update/{service,mod}.rs`.
- **Change:**
  - b0 §1: `PreparedInstall`, `prepare_install`, `finish_install` (wrapping the private discovery/profile helpers), and `build_update_command(request)` with the lifecycle builder and no stdio.
  - The old `install_game`/`update_game` keep working, rebuilt on the new functions plus `attach_log_stdio`.
  - The `block_on` in the old `install_game` stays until 4.1 deletes the function.
- **Tests:**
  - `prepare_install` provisions the prefix and returns a command whose Debug output has `run <installer>`.
  - The `finish_install` result equals today's `install_game_creates_prefix_and_prefers_discovered_executable` expectations.
  - Lifecycle flatpak argv for update via `build_direct_proton_command_lifecycle_with`.

### Task 1.4: core run_executable prepared builder (D3, D5 unique dir)

- **Owns:** `core/run_executable/{mod,models}.rs`, `core/run_executable/service/{mod,adhoc_prefix,command_builder,runner,validation,tests}.rs`.
- **Change:**
  - b0 §1: `adhoc_prefix_root`, `resolve_adhoc_prefix_path(root, exe, suffix)`, `is_throwaway_prefix_path_in`, `PreparedRunExecutable`, `prepare_run_executable`, and `build_run_executable_command(request, prefix)` with the lifecycle builder and no stdio.
  - `is_throwaway_prefix_path` delegates.
  - The old `run_executable` and `resolve_default_adhoc_prefix_path` stay until 4.1.
- **Tests:**
  - Two calls with different suffixes give different dirs under the same root.
  - `is_throwaway_prefix_path_in` accepts `<root>/<slug>-<sfx>` and rejects the root, `..` and a sibling.
  - `.msi` argv is unchanged.

### Task 1.5: metadata list (D7)

- **Owns:** `core/metadata/prefix_ops.rs`, `core/metadata/prefix_version_restore_store.rs`.
- **Change:** `list_restores` + `MetadataStore::list_prefix_version_restores` (read-only, `with_sqlite_conn`).
- **Tests:**
  - Two rows are listed in sorted order.
  - An empty table gives `[]`.
  - The disabled store behaves like `load` (match the existing `disabled_store_journal_operations_fail_closed_with_typed_errors` expectation for reads).

### Task 1.6: app job primitives (D4, D11, D5)

- **Owns:** `app/jobs.rs`, `app/jobs/process.rs` → split into `app/jobs/process/{mod,supervise,logs,sweep}.rs`, `app/jobs/process/tests.rs`, `app/jobs/registry.rs`, `app/jobs/tests.rs`.
- **Change:**
  - b0 §2: `supervise_child_with_kill`. `supervise_child` delegates to it.
  - `create_log_file_in` (0o600, returns the handle). `create_log_path_in` delegates to it.
  - `processes_using_prefix`. `kill_processes_using_prefix` uses it, with behaviour unchanged (D15).
  - `JobRegistry::is_key_held`.
- **Tests:**
  - `kill` → `Killed` with SIGKILL and no grace wait (real `sh -c 'trap "" TERM; sleep 30'`).
  - `kill` during the TERM grace gives an immediate KILL.
  - The log file mode is 0o600, and writes through the returned handle land in the file.
  - `processes_using_prefix` finds a child spawned with `env X=<tmp>/p` and not `<tmp>/p-other`.
  - `is_key_held` is true while running and false after finish.

### Task 1.7: shared process plumbing (D4, D5, D6, D11)

- **Owns:** `app/services/process/{mod,owner,run,child,tee}.rs`, `app/services/process/tests/{mod,run,child,owner}.rs`, `app/services/launch/streaming.rs`, `app/services/mod.rs` (add only `pub(crate) mod process;` plus `pub use process::{JobOwner, ProcessRun, RunEvent};`).
- **Change:**
  - b0 §3. The pipe tee moves from `streaming.rs` to `process/tee.rs`. The `stream_child_log` public signature and behaviour stay unchanged (crosshook-cli).
  - `run_child` runs supervision, tee and `follow` concurrently, and returns after the final drain.
- **Tests:**
  - Lines arrive incrementally before exit. `Finished` comes after the last `Log`.
  - Invalid UTF-8 plus a 1 MiB newline-free flood does not stall (the child exits, the job finishes).
  - A descendant holding the pipe does not block past `DRAIN_GRACE`.
  - The owner slot: register/get/guard removes only a matching id.
  - `ProcessRun::started()` returns before the first log line.

### B1 gate

```sh
cargo check --workspace --all-targets
cargo test -p crosshook-core prefix_deps
cargo test -p crosshook-core platform
cargo test -p crosshook-core runtime_helpers
cargo test -p crosshook-core install
cargo test -p crosshook-core update
cargo test -p crosshook-core run_executable
cargo test -p crosshook-core metadata
cargo test -p crosshook-app jobs
cargo test -p crosshook-app process
cargo test -p crosshook-app launch
cargo test -p crosshook-cli
```

---

## B2: services

### Task 2.1: InstallService (D1, D3, D5)

- **Owns:** `app/services/install/{mod,start}.rs`, `app/services/install/tests/{mod,start}.rs`.
- **Change:** b0 §4. The job is keyed on the prefix. The completion carries `InstallGameResult` or the failure text. Cancel (shutdown) gives `Cancelled`.
- **Tests:**
  - A fake proton script that writes `drive_c/Games/X/X.exe` then exits 0 gives a result with that candidate.
  - Exit 3 gives a failure text containing "status code 3".
  - A second start on the same prefix gives `Conflict` while the first is gated.
  - Afterwards `job_counts()==(0,0)`.

### Task 2.2: UpdateService (D1, D4, D5, D6)

- **Owns:** `app/services/update/{mod,types,start}.rs`, `app/services/update/tests/{mod,start}.rs`.
- **Change:** b0 §5.
- **Tests:**
  - Lines stream, then completion `{code:Some(0)}`.
  - `cancel(own)` → TERM → `signal:Some(15)` and `true`.
  - `cancel(foreign)` → `false`, and the job keeps running.
  - `cancel` with no job → `false`.
  - Same prefix as a held key → `Conflict`.
  - The `UpdateGameResponse` JSON equals the old result fields plus `run_id`.

### Task 2.3: RunExecutableService (D1, D4, D5, D6, D11)

- **Owns:** `app/services/run_executable/{mod,types,start,cleanup}.rs`, `app/services/run_executable/tests/{mod,start,cleanup,cancel}.rs`.
- **Change:** b0 §6. Throwaway cleanup moves from `tauri/commands/run_executable.rs`: guard, `remove_dir_all`, sweep, two bounded retries, no `rm`. It runs in the job before completion. The `/usr/bin/rm` doc text is fixed (L3).
- **Tests (D14):**
  - Two throwaway runs of the same exe stem get distinct dirs, and both run.
  - The key is held until cleanup finishes (`is_key_held` true until completion is received).
  - The throwaway dir is removed after a natural exit, a cancel and a stop.
  - An explicit user prefix is never removed.
  - `stop(own)` → SIGKILL without grace.
  - `cancel(foreign)`/`stop(foreign)` → `false`, and the job is untouched.
  - Validation error → `RunExecutableStartError::Validation(variant)`.

### Task 2.4: prefix_deps foundation: tools, types, seams, journal (D7, D9, D13)

- **Owns:**
  - `app/services/prefix_deps/{mod,types,messages,tools,seams,journal}.rs`. `mod.rs` declares **all** prefix_deps modules, including 2.5's `{install,check,repair,recovery,status}`.
  - `app/services/prefix_deps/tests/{mod,helpers,tools,journal}.rs`. `tests/mod.rs` declares 2.5's `{exclusivity,cancel,install,recovery}`.
- **Change:**
  - b0 §7: types, texts, `resolve_tool`, `profile_steam_app_id`, the four seams with core defaults, `decide`, `JournalEntry::try_from` (including the M4 `binary_path` rule), `JournalTicket`, `PrefixVersionJournal`, and the `PrefixDepsService` struct with `new`/`with_seams`.
  - Tests moved from `tauri/commands/prefix_deps.rs` are ported here.
- **Tests:**
  - `tools.rs`: S1–S5 plus `resolve_tool` NotFound and the steam/runtime app-id order.
  - `journal.rs`: J1a–J1c, J2a–J2b, J3a–J3f, J4a–J4h (feature-spec AC 815-1). This includes delete-fail → `mark_failed("…cleanup failed…")` → `Failed`, invalid `binary_path` (relative, missing, non-executable) → `failed` with the restorer call count 0, and `check_in_use` → `SkippedInUse` with the row still `pending`.
  - Use the real `CoreRestorer` with fake script plus `system.reg` fixtures where possible.

### Task 2.5: prefix_deps operations (D5, D7, D8, D9, D12)

- **Owns:** `app/services/prefix_deps/{install,check,repair,recovery,status}.rs`, `app/services/prefix_deps/tests/{exclusivity,cancel,install,recovery}.rs`.
- **Change:**
  - b0 §7 methods. Offline gate first.
  - The install job is keyed on the prefix, in this order: `recover_before_install`, `lookup_profile_id` (required, legacy text), prepare, `record`, `run_child(Piped)` with the sanitized sink, restore on a fresh token, dep-state upserts, `resolve`, completion.
  - `in_progress` = `jobs.is_key_held(prefix key)`. `last_recovery` comes from `RecoveryLedger`.
  - `recover_on_startup` handles pending rows only.
- **Tests:**
  - `exclusivity.rs`: L1 (the same prefix and a symlink alias give `Conflict`; one installer call), L2 (another prefix is allowed), L3 (repair → `Conflict`; startup skips with `SkippedBusy`), and a run-exe/update-style `jobs.start` on the same key gets `Conflict`.
  - `cancel.rs`: C1 (`jobs.cancel(id)` mid-install → TERM to the group; restore `-q win10` ran; row deleted; outcome `Cancelled`).
  - `install.rs`:
    - offline → `Unavailable "offline mode is enabled"`, `job_counts()==(0,0)`;
    - the `prefix-dep` log payload carries the raw `prefix_path`;
    - J4f record fail → no spawn;
    - J4g spawn fail keeps `pending`;
    - `in_progress` is true during the install.
  - `recovery.rs`:
    - a restored row → `last_recovery.outcome==Restored` and `required:false`;
    - metadata disabled → no-op;
    - a `failed` row is untouched.

### Task 2.6: wiring and startup recovery (D1, D8)

- **Owns:** `app/services/mod.rs` (append the four `pub mod`/`pub use` lines; 1.7's lines stay), `app/lib.rs`, `app/bootstrap/stores.rs`, `app/bootstrap/stores/tests.rs`, `app/bootstrap/options.rs`, `app/bootstrap/mod.rs` (re-export `PrefixJournalRecovery`), `app/bootstrap/scans.rs`, `crates/crosshook-app/tests/bootstrap_report.rs` (use `PrefixJournalRecovery::Skip` in `test_options`).
- **Change:** b0 §8. Four `Services` fields built with `jobs.clone()`. `schedule_prefix_journal_recovery`. No sleeps (startup-path guard).
- **Tests:**
  - `services_new_builds_bundle_from_stores` is extended: `install`/`update`/`run_executable`/`prefix_deps` share one registry (a key started via `prefix_deps` conflicts through `update`'s registry clone).
  - Bootstrap with `Skip` schedules no recovery job.

### B2 gate

```sh
cargo check --workspace --all-targets
cargo test -p crosshook-app install
cargo test -p crosshook-app update
cargo test -p crosshook-app run_executable
cargo test -p crosshook-app prefix_deps
cargo test -p crosshook-app bootstrap
cargo test -p crosshook-app --test bootstrap_report
cargo test -p crosshook-app --test bootstrap_ordering
./scripts/check-startup-path.sh   # from repo root
```

---

## B3: adapters and frontend

### Task 3.1: Tauri install/update, relay, managed state (D1, D2, D4, D11)

- **Owns:** `tauri/lib.rs`, `tauri/commands/mod.rs`, `tauri/commands/install.rs`, `tauri/commands/update.rs`, `tauri/commands/process_relay.rs` (new), and **delete** `tauri/commands/log_stream.rs`.
- **Change:**
  - b0 §9. Thin delegates with `map_app_error`. The `update_game` pump emits `update-log` (bare string per line via `batch_lines`), then `update-complete` (`code`) after `Finished`. Owner = `webview.label()`.
  - `lib.rs`: add `.manage(services.{install,update,run_executable,prefix_deps}.clone())`. Remove the three `*State::new()` manages. Pass `PrefixJournalRecovery::Skip{reason:"CROSSHOOK_BENCH=1"}` when bench is on.
- **Tests:**
  - `command_names_match_expected_ipc_contract` in `install.rs` (3) and `update.rs` (3).
  - `batch_lines` maps Dropped/Truncated to notice lines.
  - `update-complete` payload serializes as a bare `number|null`.

### Task 3.2: Tauri run_executable (D2, D4, D6, D10)

- **Owns:** `tauri/commands/run_executable.rs`.
- **Change:**
  - `RunCommandError` stays verbatim, plus `From<RunExecutableStartError>`.
  - Four delegates. The pump emits `run-executable-log` `{run_id, line}`, then `run-executable-complete` `{run_id, code, signal}`.
  - Delete `RunExecutableProcessState`, `cleanup_throwaway_prefix`, `try_remove_dir` and every `host_std_command("kill")`.
- **Tests:**
  - The pin test (4 commands).
  - The P0 `RunCommandError` fixtures still equal.
  - The log and complete payload JSON shapes.

### Task 3.3: Tauri prefix_deps (D2, D9, D12)

- **Owns:** `tauri/commands/prefix_deps.rs`.
- **Change:**
  - Six delegates.
  - `install_prefix_dependency` returns `Ok(())` after `start_install`. Its pump emits `prefix-dep-log`/`prefix-dep-complete` using the service DTOs with the raw request `profile_name`/`prefix_path`.
  - Delete all moved helpers, `PrefixDepsInstallState` and the moved tests.
- **Tests:**
  - The pin test (6 commands, replacing the `stringify!` test).
  - The P0 prefix fixtures still equal.
  - The repair-status fixture equals the P0 fixture plus `in_progress:false,last_recovery:null`.

### Task 3.4: Frontend run-executable (D10)

- **Owns:** `src/hooks/useRunExecutable.ts`, `src/components/RunExecutablePanel.tsx`, `src/types/run-executable.ts`, `src/lib/mocks/handlers/run_executable.ts` (new), `src/lib/mocks/handlers/system.ts` (move the run-exe block out; update the prefix-deps mocks: `MOCK_PREFIX_VERSION_REPAIR_STATUS` gains the new fields, `install_prefix_dependency` emits log lines and complete `restore_state:'not_required'`), `src/lib/mocks/index.ts`, `src/lib/mocks/README.md`, `src/styles/theme.css` (add a `.crosshook-process-log*` class set for the run-exe panel; leave the existing `.crosshook-prefix-deps__log*` rules untouched), `src/hooks/__tests__/useRunExecutable.test.ts` (new), `src/components/__tests__/RunExecutablePanel.test.tsx` (new).
- **Change:**
  - b0 §11. One mount-scoped listener for each event, filtered by `run_id`. Capped `logLines`.
  - Functional `setStage((s) => s === 'preparing' ? 'running' : s)`.
  - Delete `NO_EXIT_RECEIVED`, `exitHolder`, `invokeResolved` and `unlistenRef`.
  - A `role="log"` `<pre>` with text only. Copy says "stream its output below".
  - The mock emits 3–5 lines, then completion. Cancel/stop finish with signal 15/9. Timers clear on reset.
- **Tests:**
  - Completion before invoke resolves → final stage `complete` and `result` set.
  - Normal order.
  - Lines accumulate and are capped. Foreign `run_id` is ignored.
  - signal → `failed`.
  - `reset` clears.
  - The panel renders the mock line in `role="log"`.

### Task 3.5: Frontend update + prefix deps (D9, D10)

- **Owns:** `src/hooks/useUpdateGame.ts`, `src/types/update.ts`, `src/types/prefix-deps.ts`, `src/components/PrefixDepsPanel.tsx`, `src/components/__tests__/PrefixDepsPanel.test.tsx`, `src/lib/mocks/handlers/update.ts`.
- **Change:**
  - Remove `completedBeforeInvoke` and use the functional stage update.
  - Add `run_id` to `UpdateGameResult`.
  - Add the `PrefixVersionRepairStatus` fields.
  - The panel shows a `role="status"` note for `last_recovery.outcome==='restored'` when `!required`, and no repair banner while `in_progress`.
  - The update mock emits `update-log` lines.
- **Tests:**
  - `PrefixDepsPanel`: the restored note is a status, not an alert. In-progress shows no banner.
  - Existing cases stay green.

### B3 gate

```sh
cargo clippy --workspace --all-targets -- -D warnings
cargo test -p crosshook-native
cargo test -p crosshook-app
# repo root
./scripts/check-mock-coverage.sh
./scripts/check-host-gateway.sh
! git grep -nE 'NO_EXIT_RECEIVED|exitHolder|invokeResolved|completedBeforeInvoke' -- src/crosshook-native/src
! git grep -nE 'UpdateProcessState|RunExecutableProcessState|PrefixDepsInstallState|spawn_log_stream|host_std_command\("kill"\)' -- src/crosshook-native/src-tauri
# src/crosshook-native
npm run typecheck && npm test
```

---

## B4: cleanup and docs

### Task 4.1: Delete the obsolete core API (D3, D4, D5)

- **Owns:** `core/install/{service,models,mod,tests}.rs`, `core/update/{service,models,mod}.rs`, `core/run_executable/{mod,models}.rs`, `core/run_executable/service/*`, `core/prefix_deps/{mod.rs,lock.rs}`, `core/prefix_deps/runner/{install,mod}.rs`, `core/prefix_deps/runner/tests/install.rs`.
- **Change:**
  - Delete `install_game`, `update_game`, `run_executable`, `resolve_default_adhoc_prefix_path*`, `{Install,Update,RunExecutable}Error::{RuntimeUnavailable, LogAttachmentFailed}`, `PreparedPrefixDependencyInstall::spawn*`, `PrefixDependencyInstall`, `install_packages` and the `lock` module.
  - Port the affected core tests to `into_parts` + spawn in the test.
- **Acceptance:** `git grep -nE 'install_packages|PrefixDepsInstallLock|wait_and_restore|fn install_game|fn update_game'` returns nothing under `crates/`.

### Task 4.2: Documentation (D15 follow-ups)

- **Owns:** repo-root `CLAUDE.md`, `AGENTS.md`, `crates/crosshook-app/README.md`, `docs/architecture/adr-0001-platform-host-gateway.md`.
- **Change:**
  - `CLAUDE.md:31` / `AGENTS.md:44`: add the four services to the service and delegate lists, plus a new "Install, update, run-executable and prefix deps" bullet with the key/cancel/journal/offline rules.
  - `AGENTS.md` Stack Overview rows (:156-157) and Directory Map (:183).
  - README: Services rows, Jobs section (new kinds `install-game`, `update-game`, `run-executable`, `prefix-deps-install`, `prefix-version-repair`, `prefix-version-recovery`, `prefix-journal-recovery`; `supervise_child_with_kill`; replace "process helpers used by the Tauri shell"), and Storage boundary (journal unchanged, runtime-only slots/ledgers, `/tmp/crosshook-logs` 0600).
  - ADR-0001: add `build_direct_proton_command_lifecycle`.
  - List the D15 follow-ups for Main to file.

### Final gate (Main)

```sh
# repo root
cargo fmt --manifest-path src/crosshook-native/Cargo.toml --all -- --check
cargo clippy --manifest-path src/crosshook-native/Cargo.toml --workspace --all-targets -- -D warnings
cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core -p crosshook-app -p crosshook-native -p crosshook-cli
./scripts/lint.sh
./scripts/check-host-gateway.sh && ./scripts/check-host-gateway.sh --self-test
./scripts/check-app-deps.sh
./scripts/check-startup-path.sh && ./scripts/check-startup-path.sh --selftest
./scripts/check-schema-version.sh
./scripts/check-mock-coverage.sh
# src/crosshook-native (npm ci first; npm run test:smoke:install once)
npm run typecheck && npm test && npm run lint && npm run test:smoke
./scripts/format.sh --modified --docs   # repo root, before final check
```

## Decision coverage

| D   | Tasks                        |
| --- | ---------------------------- |
| D1  | 2.1–2.6, 3.1                 |
| D2  | 0.1, 3.1, 3.2, 3.3           |
| D3  | 1.1, 1.2, 1.3, 1.4, 4.1      |
| D4  | 1.6, 1.7, 2.2, 2.3, 3.1, 3.2 |
| D5  | 1.4, 1.7, 2.1, 2.2, 2.3, 2.5 |
| D6  | 1.7, 2.2, 2.3, 3.1, 3.2      |
| D7  | 1.5, 2.4, 2.5                |
| D8  | 2.5, 2.6, 3.1                |
| D9  | 2.4, 2.5, 3.3, 3.5           |
| D10 | 3.2, 3.4, 3.5                |
| D11 | 1.6, 1.7, 3.1                |
| D12 | 2.5                          |
| D13 | 1.2, 2.4                     |
| D14 | 2.1–2.5, 3.4, 3.5            |
| D15 | 4.2                          |

## AC → test mapping

| AC     | Test                                                                                                   | Task     |
| ------ | ------------------------------------------------------------------------------------------------------ | -------- |
| 815-1  | `app/services/prefix_deps/tests/journal.rs` J1a–J4h; `tests/recovery.rs`                               | 2.4, 2.5 |
| 815-2  | `app/services/prefix_deps/tests/tools.rs` S1–S5; core `validation` S1 table                            | 1.2, 2.4 |
| 815-3  | `app/services/prefix_deps/tests/exclusivity.rs` L1–L3                                                  | 2.5      |
| 815-4  | `src/hooks/__tests__/useRunExecutable.test.ts`, `src/components/__tests__/RunExecutablePanel.test.tsx` | 3.4      |
| 815-5  | `useRunExecutable.test.ts` complete-before-invoke case + B3 grep gate                                  | 3.4, 3.5 |
| 815-6  | `./scripts/check-host-gateway.sh` (B3 and final gates)                                                 | all      |
| 1538-1 | core `prefix_deps/runner/tests/check.rs::check_command_flatpak_argv_has_watch_bus_after_host`          | 1.2      |
| 1538-2 | `cargo test -p crosshook-core prefix_deps` (B1 and final)                                              | 1.2, 4.1 |
| 1538-3 | `./scripts/check-host-gateway.sh`                                                                      | all      |

Manual items from the issue (install a dependency, kill CrossHook mid-install and restart, run and cancel an executable, Flatpak) are reported as **not performed** in the PR body.

Squash title: `refactor(app): move install, update, run-executable and prefix-deps into supervised services`.
