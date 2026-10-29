# Research — Technical map (YAN-815 / M1-31 + YAN-1538)

Scope: install, update, run-executable and prefix-deps Tauri commands → `crosshook-app` services on `JobRegistry`.
Paths are relative to `src/crosshook-native/`. Read-only research; nothing was built or run.

## 1. Tauri command inventory (16 commands)

Wire keys: Tauri converts snake_case Rust args to camelCase on the wire. Request structs are serde snake_case inside `request`.

### 1.1 `src-tauri/src/commands/install.rs` (37 lines, no managed state, no events)

| Command                             | Wire args                         | Return                             | Core call                                                                                          |
| ----------------------------------- | --------------------------------- | ---------------------------------- | -------------------------------------------------------------------------------------------------- |
| `install_default_prefix_path` (:10) | `{ profileName }`                 | `Result<String,String>`            | `install::install_default_prefix_path(&str) -> Result<PathBuf, InstallGameError>` (spawn_blocking) |
| `validate_install_request` (:21)    | `{ request: InstallGameRequest }` | `Result<(),String>`                | `install::validate_install_request(&InstallGameRequest) -> Result<(), InstallGameValidationError>` |
| `install_game` (:30)                | `{ request }`                     | `Result<InstallGameResult,String>` | `install::install_game(&InstallGameRequest, &Path) -> Result<InstallGameResult, InstallGameError>` |

- `install_game` creates a log with `jobs::create_log_path("install", slugify_target(profile_name,"install"))` (:31), then runs core in `spawn_blocking`. Core (`crosshook-core/src/install/service.rs:53-130`) provisions the prefix, spawns the Proton command and calls `Handle::block_on(child.wait())` (:70). Blocking wait: **no cancel, no PID tracking, no log events**. The UI (`hooks/useInstallGame.ts:288`) waits for the invoke result. `InstallGameResult` carries `helper_log_path`, the discovered exe candidates and a reviewable `GameProfile`.
- Frontend: `useInstallGame.ts:217` (`profileName`), `:282`, `:288` (`request`).

### 1.2 `src-tauri/src/commands/update.rs` (88 lines)

Managed state: `UpdateProcessState { pid: Mutex<Option<u32>> }` (:12), registered at `src-tauri/src/lib.rs:71`.

| Command                         | Wire args                        | Return                            | Notes                                                                                                |
| ------------------------------- | -------------------------------- | --------------------------------- | ---------------------------------------------------------------------------------------------------- |
| `validate_update_request` (:25) | `{ request: UpdateGameRequest }` | `Result<(),String>`               | `update::validate_update_request`                                                                    |
| `update_game` (:34)             | `{ request }`                    | `Result<UpdateGameResult,String>` | Spawns, stores PID, `spawn_log_stream(.., "update-log", "update-complete", clear_pid)`               |
| `cancel_update` (:72)           | none                             | `Result<(),String>`               | `host_std_command("kill").arg(pid)`: SIGTERM to the stored PID only, no group, no SIGKILL escalation |

- Core: `update::update_game(&UpdateGameRequest, &Path) -> Result<(UpdateGameResult, tokio::process::Child), UpdateGameError>` (`update/service.rs:63`); `build_update_command(&req,&log) -> Result<Command, UpdateGameError>` (:29). `UpdateGameRequest { profile_name, updater_path, proton_path, prefix_path, steam_client_install_path }`; `UpdateGameResult { succeeded, message, helper_log_path }`. `succeeded` = spawned, not exited.
- Events: `update-log` payload = **bare string** line; `update-complete` payload = `Option<i32>` exit code (`null` = signal).
- Listeners: `components/ConsoleView.tsx:66` (`update-log`), `hooks/useUpdateGame.ts:235` (`update-complete`). There is a `completedBeforeInvoke` flag workaround (:230-260), the same race as run-executable.

### 1.3 `src-tauri/src/commands/run_executable.rs` (301 lines)

Managed state: `RunExecutableProcessState { info: Mutex<Option<RunningProcessInfo { pid, prefix_path }>> }` (:17-38), `lib.rs:72`. Poison-tolerant `lock_info` (:43).
Error envelope: `RunCommandError` (:53-64), serde `tag="kind"` snake_case: `Validation { variant: RunExecutableValidationError, field: &'static str, message }` | `Runtime { message }`. The frontend matches on `kind`/`field` (`useRunExecutable.ts:84-100`).

| Command                                 | Wire args                           | Return                                        | Notes                                                                                                                                                                                                                                                                                                                       |
| --------------------------------------- | ----------------------------------- | --------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `validate_run_executable_request` (:96) | `{ request: RunExecutableRequest }` | `Result<(),RunCommandError>`                  | core validate                                                                                                                                                                                                                                                                                                               |
| `run_executable` (:106)                 | `{ request }`                       | `Result<RunExecutableResult,RunCommandError>` | log `create_log_path("run-executable", slug(exe stem))`; `is_throwaway = prefix_path.trim().is_empty()`; spawn; store pid+resolved prefix; `spawn_log_stream(.., "run-executable-log", "run-executable-complete", cb)`. The callback clears state and, for throwaway runs, calls `spawn_blocking(cleanup_throwaway_prefix)` |
| `cancel_run_executable` (:180)          | none                                | `Result<(),RunCommandError>`                  | `take()` info, then `host_std_command("kill") pid` (SIGTERM, errors ignored)                                                                                                                                                                                                                                                |
| `stop_run_executable` (:204)            | none                                | `Result<(),RunCommandError>`                  | `take()` info, then `kill -KILL pid` plus `jobs::kill_processes_using_prefix(prefix)` (/proc environ sweep)                                                                                                                                                                                                                 |

- Core: `run_executable::run_executable(&RunExecutableRequest, &Path) -> Result<(RunExecutableResult, Child), RunExecutableError>` (`run_executable/service/runner.rs:12`). It resolves the throwaway prefix via `resolve_default_adhoc_prefix_path(&Path)` (`adhoc_prefix.rs`, `_run-adhoc/<slug>`) and provisions it (`provision_prefix`, pub(crate) :114). It builds with `build_run_executable_command(&req, &prefix, &log) -> Result<Command,_>` (`command_builder.rs:13`; `.msi` uses `msiexec /i <path> /qb`). Also exported: `is_throwaway_prefix_path(&Path) -> bool`, `validate_run_executable_request`. `RunExecutableRequest { executable_path, proton_path, prefix_path, working_directory, steam_client_install_path }`; `RunExecutableResult { succeeded, message, helper_log_path, resolved_prefix_path }`.
- `cleanup_throwaway_prefix` (:242-281): guard `is_throwaway_prefix_path`, then `remove_dir_all`. On failure it runs a `kill_processes_using_prefix` sweep, sleeps 300ms, retries, sleeps 200ms and retries again (the doc mentions `/usr/bin/rm -rf`, but the code does **not** use it), then warns. The cleanup is fire-and-forget, so its result is not reported to the UI.
- Events: `run-executable-log` = bare string; `run-executable-complete` = `Option<i32>`.
- Frontend: `useRunExecutable.ts` listens **only** to `run-executable-complete` (:198). No `run-executable-log` listener exists anywhere in `src/` (grep confirmed). The complete-before-invoke workaround is at :174-215: `NO_EXIT_RECEIVED` symbol + `exitHolder` + `invokeResolved` flag. There is no setTimeout in this hook. Mocks: `src/lib/mocks/handlers/system.ts:578-600` (stubs, emit nothing).

### 1.4 `src-tauri/src/commands/prefix_deps.rs` (788 lines; tests at :634-788)

Managed state: `PrefixDepsInstallState { lock: PrefixDepsInstallLock }` (:15), `lib.rs:73`. Also uses the managed `SettingsStore`, `MetadataStore` and `ProfileStore`.

| Command                                         | Wire args                               | Return                                       | Notes                                                                                                                     |
| ----------------------------------------------- | --------------------------------------- | -------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------- |
| `detect_protontricks_binary` (:215, sync)       | none                                    | `Result<BinaryDetectionResult,String>`       | `detection::detect_binary(&settings.protontricks_binary_path)`                                                            |
| `check_prefix_dependencies` (:224)              | `{ profileName, prefixPath, packages }` | `Result<Vec<PrefixDependencyStatus>,String>` | detect → fallback → `runner::check_installed` → `upsert_prefix_dep_state(..,"installed"/"missing")` fail-soft per package |
| `install_prefix_dependency` (:304)              | `{ profileName, prefixPath, packages }` | `Result<(),String>`                          | full flow in §2; returns as soon as spawned                                                                               |
| `get_prefix_version_repair_status` (:502, sync) | `{ profileName, prefixPath }`           | `Result<PrefixVersionRepairStatus,String>`   | `normalize_prefix_path` → `load_prefix_version_restore` → `repair_status_from_row`; `profileName` dropped                 |
| `repair_prefix_windows_version` (:518)          | `{ profileName, prefixPath }`           | `Result<PrefixVersionRepairStatus,String>`   | §2.4                                                                                                                      |
| `get_dependency_status` (:597, sync)            | `{ profileName, prefixPath }`           | `Result<Vec<PrefixDependencyStatus>,String>` | `load_prefix_dep_states(profile_id)` filtered by raw `prefix_path` string; state string → `DependencyState`               |

Events:

- `prefix-dep-log`: `DepLogPayload { profile_name, prefix_path, line }` (:29), where `line = runner::sanitize_output_for_ui` (ANSI strip, `/home/` `/tmp/` `/var/` → `<path>`, 500-char cap).
- `prefix-dep-complete`: `DepCompletePayload { profile_name, prefix_path, succeeded, exit_code, install_succeeded, install_exit_code, restore_state, restore_error }` (:37). `succeeded = install_succeeded && restore ∈ {NotRequired, Succeeded}` (`completion_payload` :142).
- `PrefixVersionRepairStatus { required, state: Option<PrefixVersionRestoreState>, last_error }` (:50).
- Frontend: `components/PrefixDepsPanel.tsx:98-115` filters both events by `profile_name` and `prefix_path` and keeps the last 200 lines. Invoked from `hooks/usePrefixDeps.ts:57,58,99,118,136` and `hooks/useLaunchPrefixDependencyGate.ts:40,47,55,62`. TS type: `types/prefix-deps.ts:36`.

## 2. Prefix-deps flows in detail

### 2.1 Tool selection / fallback (duplicated in check :230-243 and install :315-328)

1. `detect_binary(settings_path)` (`prefix_deps/detection.rs:18`) checks in priority order: settings override if it is an executable host file (tool by filename contains "protontricks"), then `winetricks` on host PATH, then `protontricks` on PATH (`host_command_exists`). Not found: install returns `Err("No winetricks or protontricks binary found.")`; check returns a longer message.
2. `resolve_tool_type` (:203): `detection.tool_type` or name-based.
3. `resolve_profile_steam_app_id` (:193): loads the profile; `effective.steam.app_id` else `effective.runtime.steam_app_id`, through `parse_nonzero_steam_app_id` (:178: trim, all ASCII digits, parses as u32, non-zero, re-stringified).
4. If the tool is Protontricks and there is no valid app id: `resolve_winetricks_path()` (detection.rs:97) switches to winetricks, else `Err("protontricks requires a valid nonzero Steam App ID; …")`.
   These helpers are private fns in src-tauri. Only `parse_nonzero_steam_app_id`/`restore_plan_from_row` have unit tests (:765); there is **no test of the fallback** (AC gap).

### 2.2 Lock

`crosshook_core::prefix_deps::lock::PrefixDepsInstallLock` (`lock.rs:9`) is a **global single slot** (`Arc<Mutex<Option<String>>>`), not per-prefix. `try_acquire(String) -> Result<PrefixDepsLockGuard, PrefixDepsError::AlreadyInstalling{prefix_path}>` is async-labelled but sync inside. The guard clears the slot on drop. `is_locked`/`active_prefix` have no callers outside lock.rs. It is acquired by install (:333) for the normalized prefix and held in the waiter task until after completion is emitted. Repair (:526) also acquires it. **check_prefix_dependencies does not take the lock.**

### 2.3 Install sequence (`install_prefix_dependency` :304-498)

1. Settings → detect → fallback (§2.1).
2. `runner::normalize_prefix_path(&str) -> Result<String,PrefixDepsError>` (`runner.rs:85`): `resolve_wine_prefix_path` (appends `pfx` if it is a dir), must be a dir, then `fs::canonicalize`.
3. `try_acquire(normalized)`.
4. `load_prefix_version_restore(normalized)`: an existing row means `Err("This prefix requires Windows version repair before installing dependencies.")`. This "before each install" check is block-only; it does not attempt recovery.
5. `lookup_profile_id(profile_name)` is **required** here (Err if not indexed). check/get_dependency_status fall back to profile_name.
6. `runner::prepare_install_packages(binary, prefix, &[String], PrefixDepsTool, Option<&str>) -> Result<PreparedPrefixDependencyInstall, PrefixDepsError>` (runner.rs:567). It validates verbs (`validation::validate_protontricks_verbs`: non-empty, ≤50, no leading `-`, regex). It builds the command with the **lifecycle** builder (`build_prefix_dep_mutation_command` :40 → `host_command_with_env_and_directory_lifecycle`), env `WINEPREFIX=<canonical pfx>`, args `[app_id] -q verbs…`, `kill_on_drop(true)` and piped stdout/stderr. **No `process_group(0)`.** The restore plan is `None` if any verb is itself a Windows-version verb; otherwise `snapshot_prefix_windows_version` parses `system.reg` `[Software\\Microsoft\\Windows NT\\CurrentVersion]` → verb (win11/win10/…; :101-200).
7. If `prepared.restore_plan()` is set: `upsert_prefix_version_restore(row{state:"pending"})` happens **before** spawn (durability point).
8. `prepared.spawn()` returns `PrefixDependencyInstall`. A spawn failure leaves the pending row (intentional, :385).
9. Two detached tasks read stdout and stderr line by line (`BufReader::lines`, so a non-UTF-8 line ends that stream) and emit `prefix-dep-log` per line. They are unbounded and run outside any supervision.
10. Waiter task (holds the guard): `child.wait_and_restore().await` (runner.rs:297). It waits for the install, then **always** runs the restore if a plan exists (`restore_prefix_windows_version_with_timeout`, 90s, lifecycle builder, `-q <verb>`, stderr captured, then verify via `system.reg` re-snapshot). Returns `PrefixDependencyInstallOutcome { install_succeeded, install_exit_code, install_error, restore_state, restore_error }`.
11. Per-package `upsert_prefix_dep_state(profile_id, pkg, prefix_path(raw), "installed"|"install_failed", err)`.
12. `journal_decision` (:127): `Succeeded→Delete`, `Failed→MarkFailed(err)`, `NotRequired|Pending→NoMutation`. Apply via `delete_prefix_version_restore` / `mark_prefix_version_restore_failed`. `outcome_after_journal_persistence` (:163) downgrades `Succeeded→Failed` if the delete fails.
13. Emit `prefix-dep-complete`, then drop the guard.
    There is no cancel command for prefix-dep installs; the child is dropped only at app exit (kill_on_drop).

### 2.4 Repair (`repair_prefix_windows_version` :518-593)

Normalize → acquire lock → reload row (missing = idempotent `required:false`) → validate via `repair_status_from_row` → `restore_plan_from_row` (:98: tool parse, `validate_protontricks_verbs([restore_verb])`, protontricks requires a valid non-zero app id) → `runner::restore_prefix_windows_version(&PrefixVersionRestorePlan) -> PrefixVersionRestoreOutcome` (runner.rs:331). Results:

- Succeeded: reload; if the row is gone, return `required:false`. Otherwise delete; if the delete fails, `mark_failed("Windows version was restored, but repair journal cleanup failed: …")` and return the persisted row.
- Failed: `mark_failed(error)` and return the persisted row.
- NotRequired/Pending (invalid): `mark_failed("…invalid state")`.
  Repair is synchronous to the invoke (≤90s); it has no events and no cancel.

### 2.5 Journal storage

- Table `prefix_version_restore_journal` (migration `metadata/migrations/v26_v27.rs`): `prefix_path TEXT PK` (canonical pfx), `profile_id` FK → `ON DELETE SET NULL`, `binary_path`, `tool_type CHECK IN ('winetricks','protontricks')`, `steam_app_id`, `restore_verb`, `state CHECK IN ('pending','failed')`, `last_error`, `created_at`, `updated_at`. The "succeeded" state is represented by row deletion.
- Store fns: `metadata/prefix_version_restore_store.rs` (`upsert_restore` forces state pending and resets last_error on conflict; `load_restore`; `mark_restore_failed`, Validation error if 0 rows; `delete_restore`, Validation error if 0 rows). `MetadataStore` wrappers are in `metadata/prefix_ops.rs:11-60`: `upsert/load/mark_failed/delete_prefix_version_restore`. Writes fail with `ReadOnlyDisabled` when the store is unavailable.
- Row type `PrefixVersionRestoreJournalRow` (`metadata/models.rs:545`) uses stringly-typed `tool_type`/`state`.
- **No list/scan fn** for all rows exists, so startup recovery needs a new `list_prefix_version_restores()` (or equivalent).
- Dep state: `upsert_prefix_dep_state` / `load_prefix_dep_states` (`prefix_ops.rs:63,83`).

### 2.6 Startup hooks

- There is currently **no** startup recovery of the journal. A pending row from a crash stays `pending` until the user opens the Prefix deps panel (`get_prefix_version_repair_status`) and clicks repair. `PrefixDepsPanel` blocks actions while `repairStatus.required` (:93).
- Bootstrap hook points in crosshook-app: `bootstrap/app_bootstrap.rs:272` `reconcile::run_metadata_reconciliation` (sync, pre-runtime, already sweeps abandoned launch ops via `metadata.sweep_abandoned_operations()`). After that, `bootstrap/scans.rs:38 schedule(app, startup, stores, services, umu)` starts startup jobs through `app.jobs().start(JobSpec::new("startup-scans"), …)` with `StartupJobKind` (`events.rs:16`: HealthBatch, OfflineReadiness, VersionScan, UmuRefresh, BackgroundPortal). Restore needs the host tool and up to 90s, so the natural spot is a new scheduled job (new `StartupJobKind`) or an extra step in `schedule_scans`, not the sync reconcile.
- src-tauri `lib.rs:67` `.setup` only bridges `core.startup()` events.

## 3. Shared log polling (`src-tauri/src/commands/log_stream.rs`, 143 lines)

`spawn_log_stream(app, log_path, child, event, complete_event, clear_pid)` does a whole-file `tokio::fs::read_to_string` every **500ms** and emits lines after `last_len` (it resets if the file shrank, and slicing at a byte offset can panic on a non-char boundary [INFERENCE]). After 5 consecutive read failures it emits "Log stream interrupted…" and later "Log stream resumed.". It uses `child.try_wait()`, emits `complete_event(status.code())`, calls `clear_pid()`, then does a final read. **The completion event is emitted before the final flush**, so UI may receive log lines after "complete". It is used only by update and run_executable. Install has no streaming. Prefix deps use pipes.

## 4. PID / cancel handling problems

- Children are spawned without `process_group(0)`. Cancel = `kill <pid>` on the leader only, so Wine children survive. Run-exec "stop" compensates with the /proc environ sweep.
- Proton commands are built with `build_direct_proton_command_with_wrappers_in_directory` (`launch/runtime_helpers/proton_command.rs:85`), which calls the **non-lifecycle** `host_command_with_env_and_directory` (no `--watch-bus`). There is no lifecycle variant of the proton builder.
- [INFERENCE] Under Flatpak the stored PID is the sandbox `flatpak-spawn` proxy, but `cancel_*` runs `kill` on the **host** via `host_std_command` (`flatpak-spawn --host kill <sandbox-pid>`). That signals an unrelated host PID namespace. `kill_processes_using_prefix` reads the sandbox `/proc`, so it cannot see host Wine processes.
- PID-reuse race: the state is cleared only by the poll loop (≤500ms late).
- Global single slot per command kind: a second `update_game`/`run_executable` overwrites the stored PID, so the first run becomes uncancellable.

## 5. YAN-1538

`runner::check_installed` (runner.rs:498) uses `build_prefix_dep_command` (:28) → the non-lifecycle `host_command_with_env_and_directory`. It has a 30s timeout and `kill_on_drop`, but under Flatpak a timeout kills only the proxy. Fix: build it through `host_command_with_env_and_directory_lifecycle` (as `build_prefix_dep_mutation_command` already does) and assert in a test that the Flatpak argv has `--watch-bus` right after `--host`. The `*_inner(..., flatpak: bool)` variants are `pub(super)`/`pub(crate)` in `platform/gateway.rs:154-178`. Existing argv tests: `platform/tests/gateway.rs:96,136`. The `HostCommandBuilder` fn-pointer seam (runner.rs:24) allows injecting a flatpak=true builder in a cfg(test) helper.

## 6. JobRegistry API (`crates/crosshook-app/src/jobs*`)

- `JobId` (types.rs:13): UUID v4; `JobId::from_string`, `as_str`.
- `JobKey` (types.rs:41): `new(impl Into<String>)`, `from_path(&Path)` (lossy).
- `JobSpec { kind: &'static str, key: Option<JobKey> }`: `JobSpec::new(kind).with_key(key)`.
- `Outcome { Succeeded, Failed(AppError), Cancelled }`, `From<Result<(),AppError>>` (Cancelled kind → Cancelled). **No exit-code field** ("a job that needs to report a non-zero exit maps it to Failed").
- `JobEventKind { Log(LogBatch), Progress(JobProgress{current,total,message}), Finished(Outcome) }`; `JobEvent { job_id, kind }`.
- `JobRegistry` (registry.rs:79): `new(JobRegistryConfig{event_capacity, retention=64})`, `for_app(handle,&shutdown)` (crate). `start(spec, FnOnce(JobContext)->Fut<Output: Into<Outcome>>) -> Result<JobHandle,AppError>` returns `Conflict("a {kind} job is already running for this target")` when the key is held. The key is released when the job finishes. The handle is subscribed **before** the task is spawned, so no event is lost. Other methods: `subscribe(&JobId) -> JobEvents` (running: live; finished-unreaped: yields `Finished` once); `cancel(&JobId)` (cooperative token cancel; NotFound for unknown); `outcome(&JobId)`; `reap(&JobId)`; `#[cfg(test)] job_counts() -> (running, finished)`.
- `JobContext`: `id()`, `cancel_token()`, `is_cancelled()`, `cancelled()`, `log(Vec<String>)`, `log_batch(LogBatch)`, `progress(JobProgress)`.
- `JobHandle`: `id()`, `cancel_token()`, `cancel()`, `recv()`, `into_events()`. `JobEvents::recv()` returns `Log(Dropped{n})` on lag; `Finished` is never lost.
- `App::jobs()` (app.rs:118): the registry token is a child of app shutdown, so shutdown cancels every job.
- Process helpers (`jobs/process.rs`):
  - `supervise_child(Child, &CancellationToken, grace) -> io::Result<ChildExit>`. `ChildExit { Natural, Terminated, Killed }(ExitStatus)` with `status()` and `was_cancelled()`. **Requires `process_group(0)`**, else `InvalidInput` (child left running). On cancel it sends SIGTERM to the group, waits `grace`, then SIGKILLs the group. Dropping the future SIGKILLs the group. `DEFAULT_KILL_GRACE = 5s`.
  - `create_log_path(prefix, slug)` (`/tmp/crosshook-logs`) and `create_log_path_in(dir, …)` (create_new, `-N` suffix).
  - `slugify_target`.
  - `kill_processes_using_prefix(&Path)` (environ boundary match, `host_std_command kill -KILL`).
- Log tail (`log_tail.rs`): `LogTail::new/poll/flush_partial/has_backlog`; `follow(path, TailConfig::default(), cancel, sink: FnMut(LogBatch)) -> io::Result<()>` (16–33ms coalescing, idle backoff ≤500ms, bounded final drain `FINAL_DRAIN_CHUNKS=64`, `LogBatch::{Lines,Dropped{n},Truncated{bytes}}`).
- `services::launch::stream_child_log(&mut Child, &Path, sink) -> io::Result<ExitStatus>` (`services/launch/streaming.rs:57`, pub) tees piped stdout/stderr into the log file (lossy, bounded) and runs `follow` concurrently. It **never kills** and does its own `child.wait()`, so it cannot be combined with `supervise_child` (both need the `Child`).
- `services::run_job(jobs, spec, blocking work)` (services/mod.rs:102, private to `services`) runs blocking work under a key, reaps, and returns `T`. Cancel returns `Cancelled`, but the blocking closure keeps running.

### 6.1 How prior services used it

- **protonup** (`services/protonup/install.rs:141-176`): `JobSpec::new(KIND).with_key(JobKey::new("kind:provider:version"))` → `jobs.start` → a detached reaper task drains events and `reap`s → returns `op_id` + a progress receiver via a oneshot `started` channel. The job body registers an owner ledger (`owners.rs`) so `cancel_install(owner, op_id)` (cancel.rs:18) is owner-checked and maps errors to NotFound. The Tauri side (`commands/protonup.rs:96-112`) forwards the progress receiver to `app.emit("protonup:install:progress")`. Offline: `flags().network_block()` → `AppErrorKind::Unavailable` (catalog.rs:26).
- **community** (`services/community/runner.rs:61-140`): `host_command_with_env_and_directory_lifecycle(...).stdin(null).stdout/stderr(piped).process_group(0).kill_on_drop(true)`. Pipes are drained concurrently with `supervise_child`; after a natural exit the drain gets a `DRAIN_GRACE=2s` window, and it is abandoned on cancel. Sync job: `community/sync.rs:22` (`jobs.start` + `subscribe`).
- **launch** relay pattern (`src-tauri/src/commands/launch/relay/mod.rs`): a pure `map_event` maps service events to wire emits, `LogBatch::Lines` becomes one emit per non-empty line, and `Dropped`/`Truncated` become visible notice lines. Reuse it for `update-log`/`run-executable-log`/`prefix-dep-log`.
- **storage scan** (`services/storage/scan.rs:63`): a fixed global `JobKey`.

## 7. Gaps: what JobRegistry offers vs what these flows need

1. **Proton commands cannot be supervised as-is.** There is no `process_group(0)` and no lifecycle (`--watch-bus`) builder for Proton. Need a lifecycle variant of `build_direct_proton_command_with_wrappers_in_directory` (or a builder-fn parameter) in crosshook-core, plus `process_group(0)` in install/update/run_executable/prefix_deps command construction. Core `install_game`/`update_game`/`run_executable` currently spawn internally. They need either "build command only" entry points (update and run_exec already expose `build_*_command`; install's `build_install_command` is private, service.rs:110) or core changes so the app service owns spawn.
2. **Install is a blocking `block_on` in core.** It must be split into prepare/spawn (app) → wait (supervised) → post-process (`discover_game_executable_candidates` is pub; `resolve_confirmed_game_executable_path` :336 and `build_reviewable_profile` :351 are private). Otherwise cancel cannot work.
3. **Two-phase cancel (SIGTERM "cancel" vs SIGKILL "stop").** `supervise_child` has a single cancel token with automatic escalation after `grace`. There is no API for "SIGTERM now, wait indefinitely" vs "SIGKILL now". Options: supervise with a long grace for cancel and an extra stop signal; extend `supervise_child` with a second "force" token; or an explicit `killpg(SIGKILL)` helper. `JobRegistry::cancel` exposes only one token.
4. **Supervise + stream together.** `stream_child_log` owns `child.wait()` and never kills; `supervise_child` consumes the `Child`. Need a combinator: the pipe tee/`follow` running concurrently with `supervise_child`, with bounded cancel-aware drains after leader exit (community pattern). Run-exec/update/install write directly to the log file via `attach_log_stdio`, so `follow` alone suffices there. Prefix deps use pipes, so the tee or per-line drains are needed, and they are bounded.
5. **Exit code in the outcome.** `Outcome` has no exit status, but `*-complete` payloads need `Option<i32>`, and prefix deps need the full `PrefixDependencyInstallOutcome`. Carry the result via a oneshot/typed service event (protonup `started`/progress pattern) or a service-specific event enum, not `Outcome`.
6. **Handle before start.** The AC says "handle is returned before the job starts". `start` already subscribes before spawn. The service must return `{ job_id, log_path, resolved_prefix_path }` synchronously after `start`, and the Tauri layer must relay events keyed by job id. The frontend then subscribes by id, which removes the `NO_EXIT_RECEIVED`/`completedBeforeInvoke` workarounds. Payloads should gain a `job_id` (like the launch `session_id`); the event names stay.
7. **Per-prefix exclusivity.** `JobKey` gives one running job per key. Use `JobKey` from the **canonical** prefix (`normalize_prefix_path`), shared by prefix-dep install and repair (and run-exec/update on the same prefix? decide). `PrefixDepsInstallLock` (global) then becomes obsolete, so delete it with no shim. Note the registry message is `Conflict`; today's error text is "installation already in progress for prefix: …". The key must be held across journal check → spawn → restore → persistence → completion. A `jobs.start` closure does this naturally if the whole flow runs inside the job.
8. **Cancel lookup without owner ledger.** Today's cancel commands take no args. With job ids, add a `job_id` arg (protonup style, owner/ledger optional) or keep a per-kind "current job" slot in the service. Decide the wire contract (`cancel_update`/`cancel_run_executable`/`stop_run_executable` currently have no args).
9. **Throwaway cleanup with retries** must run after `supervise_child` returns via any path (natural/cancel/stop) inside the job, before `Finished`, with the result logged. `kill_processes_using_prefix` is Flatpak-blind (sandbox /proc) [INFERENCE]. Keep the `is_throwaway_prefix_path` guard. The "rm -rf fallback" in the doc is not implemented.
10. **Journal type + recovery.** Need a `PrefixVersionJournal` (typed tool/state; wraps the `MetadataStore` fns; decisions Delete/MarkFailed/NoMutation; `recover(prefix)` = restore-if-pending, else keep failed) and a new `list_prefix_version_restores` in core metadata. Wire the recovery as a startup job (§2.6) and as a pre-install step inside the job (today: hard Err). Metadata-unavailable (`ReadOnlyDisabled`) must block installs that need a journal.
11. **Blocking vs async.** `run_job` is for blocking closures and can't cancel. Process flows must use `jobs.start` with async bodies (`check_installed` and `restore` are already async). `check_prefix_dependencies` is a short (30s) probe, so `run_job`-style without a key, or keyed by prefix to avoid racing an install (decide).
12. **Offline**: none of these flows download anything (winetricks verbs do download at runtime) [INFERENCE: winetricks fetches installers]. Decide whether `offline_mode` → `Unavailable` applies to prefix-dep install (protonup's `network_block` precedent).
13. **Event relays**: a new Tauri relay per kind (`update-log`/`update-complete`, `run-executable-log`/`run-executable-complete`, `prefix-dep-log`/`prefix-dep-complete`; install currently has none). The relay must emit complete **after** the final log drain (today's log_stream inverts that).
14. **Gateway**: the AC `./scripts/check-host-gateway.sh` scans src-tauri, crosshook-app and crosshook-core for denylisted `Command::new("winetricks"|"protontricks"|"proton"|"flatpak-spawn"…)`. New code must spawn only through `crosshook_core::platform` builders.
15. **Tests to add** (AC): journal recovery matrix (crash after version change before restore → pending → restore; crash after restore → pending row but already at target verb; failed restore → failed; delete decisions); fallback protontricks→winetricks and app id validation (the logic must move into the app service with an injectable detection seam; `detect_binary_with` is private in core); per-prefix second-install `Conflict` (use `job_counts()`); check_installed lifecycle argv.

## 8. Files to delete/retire after cutover

`src-tauri/src/commands/log_stream.rs`; `UpdateProcessState`, `RunExecutableProcessState`, `PrefixDepsInstallState` and their `lib.rs:71-73` registrations; `crosshook_core::prefix_deps::lock` (no other users); `runner::install_packages` compat wrapper (runner.rs:626, no callers outside tests); the journal/payload helpers in `commands/prefix_deps.rs` (move to the service with their tests).
