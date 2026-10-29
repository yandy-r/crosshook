# YAN-815 / YAN-1538 practices research and implementer checklist

Read-only research of the three most recent service moves (protonup YAN-822, community YAN-835, onboarding YAN-831), their landed code, and the current install/update/run-executable/prefix-deps adapters. Paths are relative to `src/crosshook-native/` unless noted. No builds or tests were run. The earlier `docs/plans/app-community-services/research-practices.md` is still accurate; this report covers what is new or specific to process services.

## 1. Service module shape (copy exactly)

- **Layout**: `crates/crosshook-app/src/services/<name>/mod.rs` with one file per operation group, private `mod` declarations, `pub use` of DTOs/seams, plus `#[cfg(test)] mod tests;` → `tests/{mod.rs,helpers.rs,<slice>.rs}`. Examples: `services/protonup/mod.rs:12-33,112-113` (11 files, all ≤281 lines) and `services/protonup/tests/mod.rs:1-10`.
- **Struct**: `#[derive(Clone)] pub struct XService { <cloned stores>, jobs: JobRegistry, <Arc<dyn Seam>>…, <runtime ledgers> }`. Close the module with `static_assertions::assert_impl_all!(XService: Send, Sync);` (`protonup/mod.rs:42-50,110`).
- **Constructors**: a production positional `new(stores…, jobs)` that installs core-backed seams (`protonup/mod.rs:55-74`), and a `#[cfg(test)] pub(crate) fn with_seams(...)` (or `with_runner_factory`, `community/mod.rs:64-78`, using `..Self::new(...)`). `HostToolsService`/`OnboardingService` instead expose a public `with_probe` (`host_tools/mod.rs:32-39`). Prefer the `#[cfg(test)] pub(crate)` form for new seams.
- **Test accessors**: `#[cfg(test)] pub(crate) fn jobs(&self) -> &JobRegistry` and counters such as `owner_count()` (`protonup/mod.rs:97-107`).
- **Job kind constants**: `pub const <NAME>_JOB_KIND: &str = "protonup-install";` in `mod.rs` (`protonup/mod.rs:35-36`, `community/mod.rs:22-23`). Key format `"<kind>:<identity>"` built by a single helper (`protonup/install.rs:266-272`). For prefix exclusivity, use `JobKey::from_path(&normalized_prefix)` (`jobs/types.rs:49-52`; its doc comment already cites "e.g. a Wine prefix path", `jobs/types.rs:38-39`).
- **Seams**: app-private `pub trait X: Send + Sync` returning `BoxFuture<'_, T>` (`services/mod.rs:76-78`; `protonup/seams.rs:22-52`), with `Core*` default impls (`seams.rs:54-120`). Use these for host processes (protontricks/winetricks/proton) so tests never spawn real host tools.
- **Docs**: `crosshook-app` has `#![deny(missing_docs)]` (`lib.rs:8`); every `pub` item needs a doc comment, and errors go in a `# Errors` section listing `AppErrorKind`s (`protonup/install.rs:118-122`).
- **Re-exports**: add `pub mod <name>;` and `pub use <name>::XService;` in `services/mod.rs:16-74`; add the service to the crate-root `pub use services::{…}` (`lib.rs:30-34`).
- **DTO files**: frozen IPC DTOs go in `<name>/types.rs` with a "serde shapes are frozen" module doc (`protonup/types.rs:1-2`). Start results carry `{ op_id, <receiver> }` plus `fn handle() -> <IpcHandle>` (`protonup/types.rs:52-68`).

## 2. Blocking work, jobs, and process supervision

- `run_blocking(work)` (`services/mod.rs:82-88`) puts sync store/fs work on the blocking pool. Use **one closure per multi-step operation**.
- `run_job(jobs, spec, work)` (`services/mod.rs:102-134`) runs a blocking closure **inside** a job and reaps it. Cancellation is cooperative: the closure is not interrupted (`mod.rs:95-99`). It returns no handle, so use it **only** for short validation/probe work (e.g. `HostToolsService::tool_details`, `host_tools/mod.rs:42-50`) and never for the long installers.
- For handle-returning, cancellable process work, follow the **protonup `start_install`** pattern (`protonup/install.rs:123-177`):
  1. Validate everything **before** `jobs.start` (nothing is allocated on rejection; tests assert `job_counts() == (0,0)`, `protonup/tests/helpers.rs:314-320`).
  2. `self.jobs.start(JobSpec::new(KIND).with_key(key), move |ctx| run_x(ctx, job, started_tx))?`. The `?` propagates `Conflict` on a duplicate key; that is the "second concurrent install on same prefix rejected" AC.
  3. Spawn a reaper task: `while events.recv().await.is_some() {}` then `jobs.reap(&id)` (`install.rs:159-168`).
  4. Wait on a `oneshot` "started" signal from inside the job body before returning, so the **handle (op_id) returns before any process output** (`install.rs:170-176`, `run_install` at `install.rs:193-207`). This is the backend half of removing the `run-executable-complete`-before-invoke workaround: the adapter subscribes or pumps before the child can finish.
  5. The job body returns `Outcome::{Succeeded, Failed(AppError), Cancelled}` (`jobs/types.rs:98-120`; `Result<(), AppError>` converts via `From`).
- **Child processes** (community git runner is the canonical model, `services/community/runner.rs:46-155`):
  - Build with `crosshook_core::platform::host_command_with_env_and_directory_lifecycle(program, &env, dir, &BTreeMap::new())`, which adds `--watch-bus` right after `--host` under Flatpak (`core/platform/gateway.rs:103-143`). Then `.process_group(0).kill_on_drop(true)` and set stdin/stdout/stderr (`runner.rs:66-79`). Do not use the plain `host_command*` builders for supervised work: they leave the host process alive on cancel (`jobs/process.rs:67-76`).
  - Supervise with `jobs::supervise_child(child, ctx.cancel_token(), DEFAULT_KILL_GRACE)`: `SIGTERM` to the group, then `SIGKILL` after the grace (`jobs/process.rs:51-88`). The child **must** lead its group, otherwise `InvalidInput` (`process.rs:79-81`).
  - Drain both pipes concurrently with supervision; once supervision returns, bound the drain by `DRAIN_GRACE` and cancellation (`runner.rs:95-155`). This rule comes from the YAN-835 learning that a descendant can hold the pipes forever.
- **"SIGTERM cancel vs SIGKILL stop"**: `supervise_child` only offers TERM→grace→KILL on the cancel token. For an immediate "stop", either (a) a second token/path passing `Duration::ZERO` as grace, or (b) keep the existing `kill_processes_using_prefix` sweep (`jobs/process.rs:317-399`, already in app, uses gateway `kill`) after the group kill for Wine processes that left the group. The current adapter does both PID kill and sweep (`src-tauri/src/commands/run_executable.rs:186,204-220`).
- **Log tailing**: create the log with `jobs::create_log_path_in(dir, prefix, slug)` (injectable dir; production uses `create_log_path` → `/tmp/crosshook-logs`, `process.rs:223-248`). Keep a `log_dir: PathBuf` field with a `#[cfg(test)]` override, like `LaunchService::with_log_dir` (`services/launch/mod.rs:80`; `launch/prepare.rs:61`). Tail with `log_tail::follow(path, TailConfig::default(), cancel, |batch| ctx.log_batch(batch))` (`log_tail.rs:448-477`; launch use at `launch/streaming.rs:57-95`). This replaces `src-tauri/src/commands/log_stream.rs` (500 ms whole-file poll, 143 lines), which becomes dead once update and run_executable move. **Delete it** and its `mod log_stream;` in `commands/mod.rs:13`.
- **Prefix exclusivity**: the core `PrefixDepsInstallLock` (`core/prefix_deps/lock.rs:5-47`) is **one global slot**, not per prefix. A `JobKey` per normalized prefix (`runner::normalize_prefix_path`, `core/prefix_deps/runner.rs:85`) replaces the `PrefixDepsInstallState` managed in `src-tauri/src/lib.rs:73`. Remove that state along with `UpdateProcessState`/`RunExecutableProcessState` (`lib.rs:71-72`). Keep core's lock type only if another caller remains (none found outside `commands/prefix_deps.rs:3,17,23`). Install, prefix-deps and run-executable keys on the **same** prefix should probably share one key namespace if they must exclude each other. Decide this explicitly.
- **Owner/cancel authority** (from protonup D6): if cancel and stop take an op id, use an owner ledger keyed by the webview label (`protonup/owners.rs`, `protonup/cancel.rs:18-23`). Unknown or foreign ids map to the same `NotFound`, and never call `JobRegistry::cancel` for ids outside the ledger. The current commands take **no id** (`cancel_update()`, `cancel_run_executable()`). To keep the IPC frozen, the service can keep an "active job id" slot per operation kind and cancel only that.

## 3. Errors (`AppError`)

- Kinds: `Validation { field }`, `NotFound`, `Conflict`, `Unavailable`, `Io`, `Cancelled`, `Internal` (`error.rs:7-25`). `AppError::new` is `pub(crate)` and **sanitizes** home paths in the message (`error.rs:37-42`, `presenter/mod.rs:28-40`). `AppError::validation(field, msg)` is public. `From<JoinError>` and `From<io::Error>` drop payloads and paths (`error.rs:88-103`).
- No `From` exists yet for `InstallGameError`, `UpdateGameError`, `RunExecutableError` or `PrefixDepsError` (none found in `crosshook-app/src`). Add `From` impls in `error.rs` (or a local `fn map_*`) with explicit kinds: `PrefixDepsError::AlreadyInstalling` → `Conflict`, `BinaryNotFound` → `Unavailable`, `ValidationError` → `Validation{..}`, `Timeout`/`ProcessFailed` → `Io`, `Database` → `Unavailable` (`core/prefix_deps/models.rs:83-105`). Check legacy strings first: decisions in every prior move (onboarding D8, community D9, protonup D8) kept legacy user-visible text via `AppError::new(kind, "<legacy text>")`. Note that `error.rs` is already 324 lines; put large mappers in `error/` submodules if they would push it past ~500.
- Offline: `settings.offline_mode` → `AppErrorKind::Unavailable, "offline mode is enabled"` **before** any job (`protonup/install.rs:129-132`; `community/sync.rs:98-103`). Per the issue, only downloads (install/update downloads, protontricks/winetricks verb fetches) gate on this.
- **Special IPC error envelope**: `run_executable` commands return `Result<_, RunCommandError>` with a `{ kind: "validation"|"runtime", variant, field, message }` serde shape (`src-tauri/src/commands/run_executable.rs:50-93`; TS mirror `src/types/run-executable.ts:27-78`). Keep that adapter type and map `AppErrorKind::Validation{field}` back to it. The core `RunExecutableValidationError` variant must survive, so either keep the core error in the service's error type (as with `LaunchStartError`, `services/launch/types.rs:232-248`) or return `Result<_, RunExecutableError>` from the validate path. Do not flatten it to a string.

## 4. Tauri adapter pattern (`src-tauri`)

- A command is only: extract `State<'_, XService>`, `.await`, `.map_err(map_app_error)`, where `fn map_app_error(e: AppError) -> String { e.message().to_string() }` (`commands/community.rs:21-31`; `commands/protonup.rs:17-19`). Async commands taking `State` must return `Result`.
- **Event pump** for long jobs: the service returns the start struct, and the adapter `drop(tauri::async_runtime::spawn(async move { while let Some(x) = next(&mut rx).await { let _ = app.emit("<event>", &x); } }))` then returns the handle (`commands/protonup.rs:96-116`). For per-session events with mapping logic, use a `relay/` module with a pure `map_event(state, event) -> Vec<Emit>` tested without Tauri, plus `spawn(app, events)` (`commands/launch/relay/mod.rs:79-200`, tests in `relay/tests.rs`). Apply that to `run-executable-log`/`run-executable-complete`, `update-log`/`update-complete`, `prefix-dep-log`/`prefix-dep-complete`, and keep event names and payload shapes byte-identical (`commands/prefix_deps.rs:28-45,412-494`).
- **Order matters**: the relay task must start (or have subscribed) before the command returns, and the job must not emit completion before the handle is returned (§2 step 4). Then the frontend workaround in `useRunExecutable.ts` can be removed (frontend peer's scope).
- **IPC pin tests**: each touched module gets `#[cfg(test)] mod tests { #[test] fn command_names_match_expected_ipc_contract() { let _ = cmd_a; … } #[test] fn app_error_maps_to_its_plain_message() {…} }` (`commands/community.rs:88-110`; `commands/protonup.rs:154-203`; `commands/launch/tests/contract.rs:7-22`). Convert the existing `stringify!` pin (`commands/prefix_deps.rs:776-787`) to this form. Total commands: install 3, update 3, run_executable 4, prefix_deps 6 (`src-tauri/src/lib.rs:101-103,158-164,219-224`).
- Register `.manage(services.<name>.clone())` in `src-tauri/src/lib.rs:42-65` and delete the three ad hoc `.manage(...State::new())` lines (`lib.rs:71-73`). Keep the raw store `.manage` calls (`lib.rs:37-41`); other commands still use them.
- The adapter's pure helpers (`journal_decision`, `completion_payload`, `outcome_after_journal_persistence`, `repair_status_from_row`, `restore_plan_from_row`, `parse_nonzero_steam_app_id`, `resolve_tool_type`; `commands/prefix_deps.rs:64-213`) and their tests (`:636-775`) **move into the service** (journal peer owns the `PrefixVersionJournal` design). Only the Tauri payload structs stay.

## 5. Services wiring and startup hook (journal recovery)

- Add fields to `bootstrap::Services` (`bootstrap/stores.rs:52-101`, one doc line each) and construct them in `Services::new(&Stores, EventBus, Handle, JobRegistry)` with `jobs.clone()` (`stores.rs:108-190`). **Never** use `JobRegistry::default()` in production, or shutdown cancellation won't reach the jobs. Extend `services_new_builds_bundle_from_stores` (`bootstrap/stores/tests.rs:119`).
- Startup hooks: `App::bootstrap` builds `Services`, then calls `scans::schedule(app, startup, stores, services, umu)` (`bootstrap/app_bootstrap.rs:116-155`). Each hook is a `schedule_*` fn that runs `app.jobs().start(JobSpec::new("startup-…"), …)` and records state in `StartupHandle` with `mark_running`/`finish`/`finish_skipped_cancelled`/`finish_start_failed` (`bootstrap/scans.rs:31-188`; helpers in `services/startup_scans.rs:28-57`). Rules: state-then-publish, no sleeps, no `App` clone captured (`scans.rs:5-16`).
- Choose one of two recovery-on-startup options:
  - (a) **Lightweight**: add a `schedule_prefix_journal_recovery(app, services)` job that is _not_ a `StartupJobKind`. It just calls `services.prefix_deps.recover_pending_journals().await`, logs, and lets the Prefix deps panel read the status through the existing `get_prefix_version_repair_status` command. It needs no IPC/TS changes.
  - (b) **Reported**: add `StartupJobKind::PrefixJournalRecovery`. That requires changing `events.rs:16-27`, `StartupJobs` (`bootstrap/report.rs:183-197`), `state.rs` terminal mapping (`:149-190`), `startup_bridge.rs` `BRIDGED_JOBS` and `finished_payload` (`src-tauri/src/startup_bridge.rs:8-75`), every `StartupJobs { … }` literal in tests (`startup_scans/tests.rs:46`, `bootstrap/report/tests.rs:29`, `bootstrap/state/tests.rs:4`, `src-tauri/src/startup_bridge/tests.rs:11`), TS `src/types/startup.ts:57-75`, and the mock `src/lib/mocks/handlers/startup.ts:47-51`.

  Option (a) matches "resolved on next start … and shown in Prefix deps panel" with the smaller blast radius. Recovery must skip when `stores.metadata.status() != Ok` (`app_bootstrap.rs:256-268` pattern) and must hold the per-prefix `JobKey` while restoring, so it cannot race a user install.

- "Before each install" recovery runs inside the install job, after acquiring the prefix key and before mutation. Today the adapter holds the lock across journal check, mutation and restore (`commands/prefix_deps.rs:334`).

## 6. Test layout and fakes

- Harness per service in `tests/helpers.rs`: a retained `tempfile::TempDir`, `SettingsStore::with_base_path(root/config)`, `ProfileStore::with_base_path(root/profiles)`, `MetadataStore::open_in_memory()` (or `disabled()` for degraded paths), `JobRegistry::default()` **cloned into the test**, fakes held as `Arc<Fake…>` with `AtomicUsize` call counters, and builders like `with_offline()` (`protonup/tests/helpers.rs:233-321`).
- `finished(&jobs)` yields until `job_counts() == (0,0)` with a 10 s timeout and no sleeps (`helpers.rs:364-375`); `job_counts` is `#[cfg(test)] pub(crate)` (`jobs/registry.rs:274-278`), so these tests live inside `crosshook-app`.
- Gate with `Notify`/oneshot plus the job's cancel token, never timing sleeps (`protonup/tests/helpers.rs:1-3`). Non-blocking proof: current-thread runtime with `max_blocking_threads(1)` (`host_tools/tests.rs:55-96`).
- **Real-process tests** are allowed for supervision: `run_test_program(&cancel, "sleep", …)` routes a harmless binary through the production spawn/supervise path (`community/runner.rs:178-213`; tests `community/tests/runner.rs:58-230`, including the descendant-holds-pipe case). For protontricks→winetricks fallback and Steam App ID validation, use fake shell scripts the way core does (`make_fake_binary`, `core/prefix_deps/runner.rs:657-674`) or a `PrefixDepsRunner` seam with a fake that records `(tool, args)`.
- `FakeHostProbe` (`host_tools/fake.rs`) is only for host readiness. Do not bend it to process work.
- Exclusivity test: start install A on prefix P (gated), then assert install B on P is `Err(kind == Conflict)` and the installer call count is unchanged; a different prefix Q succeeds.

## 7. Host gateway (YAN-1538) and core boundaries

- `check_installed` currently uses the **non-lifecycle** `build_prefix_dep_command` → `host_command_with_env_and_directory` (`core/prefix_deps/runner.rs:27-37,498-504`), while mutations already use `build_prefix_dep_mutation_command` (`:39-50,577`). YAN-1538 fix: switch `check_installed` to the lifecycle builder (or delete `build_prefix_dep_command` if it becomes unused) and add a test asserting that the Flatpak argv has `--watch-bus` right after `--host` (pattern: `mutation_command_targets_binary_with_resolved_wineprefix_natively`, `runner.rs:1113-1118`). Note that `core/prefix_deps/runner.rs` is **1180 lines**, well over the ~500 soft cap. Any edits there should split it (e.g. `runner/{command,check,install,restore,sanitize}.rs`).
- install/update/run_executable core spawn through `build_direct_proton_command_with_wrappers_in_directory` → `host_command_with_env_and_directory` (non-lifecycle; `core/launch/runtime_helpers/proton_command.rs:85-112`; `core/run_executable/service/command_builder.rs:31-38`). For real Flatpak cancel, the services need a lifecycle builder variant (core change), and `.process_group(0)` must be applied by the app before spawn. That means core should return the `Command` (as `update::build_update_command`, `core/update/service.rs:29`, already does) instead of spawning inside `install_game`/`run_executable` (`core/install/service.rs:53-70`; `core/run_executable/service/runner.rs:12-35`). `install_game` even `block_on`s the child on a runtime handle, so it must be split into build and spawn steps for supervision.
- `scripts/check-host-gateway.sh` scans `crosshook-core/src`, `src-tauri/src` and `crosshook-app/src` (`scripts/check-host-gateway.sh:11-18`). Denylist: `proton umu-run gamescope mangohud winetricks protontricks gamemoderun xdg-open gio kde-open gnome-open flatpak-spawn` (`:7-10`). Never write `Command::new("protontricks"|"winetricks"|…)` in app or tests; test scripts must use other names (e.g. `sh`, fake script paths). `kill` via `host_std_command("kill")` is allowed.

## 8. crosshook-cli compatibility

- `crosshook-cli` uses only `crosshook_app::log_tail::LogBatch` and `services::launch::{read_log_tail, stream_child_log}` (`crates/crosshook-cli/src/launch.rs:9-10,151-174`). Its `crosshook_core` imports are `launch`, `metadata`, `profile`, `runtime_helpers`, `settings`, `steam`, `umu_database`, `export` — **no** `install`, `update`, `run_executable` or `prefix_deps` usage. The only other callers of `jobs::{create_log_path, kill_processes_using_prefix, slugify_target}` are the four src-tauri modules being moved. Constraints: keep `stream_child_log`/`read_log_tail`/`LogBatch` signatures unchanged; keep `jobs::create_log_path*`, `slugify_target`, `kill_processes_using_prefix` public. `cargo test -p crosshook-cli` must still pass.

## 9. File-size and doc rules (CLAUDE.md / AGENTS.md)

- ~500-line soft cap; files that "drift meaningfully past" it must be split (`CLAUDE.md:52`). Current offenders touched here: `commands/prefix_deps.rs` 788, `core/prefix_deps/runner.rs` 1180, `commands/run_executable.rs` 301. Target: service files ≤ ~300 lines like protonup, and a separate `tests/<slice>.rs` per AC group (journal recovery, fallback, app-id validation, exclusivity, cancel/stop, throwaway cleanup, log streaming).
- **Persistence classification is mandatory** in plan artifacts (`CLAUDE.md:50,76`; AGENTS.md "Persistence planning"). Here: `prefix_dependency_state` and `prefix_version_restore_journal` (v27, `AGENTS.md:245-249`) are SQLite metadata, unchanged schema, no migration. Registries, owner/active-job slots and log tails are runtime-only. Log files stay in `/tmp/crosshook-logs` (ephemeral). Settings `protontricks_binary_path`/`auto_install_prefix_deps` are TOML (read-only here). There is no schema bump, so `check-schema-version.sh` stays green.
- Docs updated by every prior move, which must be updated again: `CLAUDE.md:31` (Architecture service list + "commands are thin delegates" list) plus a new per-area bullet like `CLAUDE.md:33-34`; `AGENTS.md` MUST bullets (`:39-64`), Stack Overview rows (`:150-163`), Directory Map services line (`:183`); `crates/crosshook-app/README.md` Services table (`:93+`), Jobs section (`:363-408`, which says "Proton installs also run through this registry"; add the new kinds and remove "process helpers used by the Tauri shell" wording if no longer true), and Storage boundary (`:410-447`). ADR-0001 (`docs/architecture/adr-0001-platform-host-gateway.md`) is unchanged unless the gateway API changes. If a lifecycle proton builder is added to core, add it to the gateway function list there and in the `CLAUDE.md:30` mention ("and friends" may suffice).
- Commit or PR title: Conventional Commits (`refactor(app): …` or `feat(app): …`); plan docs under `docs/plans/` use `docs(internal): …`.

## 10. Verification gates (run once, by main agent, after integration)

From the repo root unless noted:

- `cargo fmt --manifest-path src/crosshook-native/Cargo.toml --all -- --check`
- `cargo clippy --manifest-path src/crosshook-native/Cargo.toml --workspace --all-targets -- -D warnings` (CI: `.github/workflows/lint.yml:49`)
- `cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-app -p crosshook-native -p crosshook-cli` (CI `lint.yml:52`). **Also** run `-p crosshook-core`: CI lint does not run core tests (only `release.yml:104-108` does), and YAN-1538 plus the spawn split touch core.
- `./scripts/lint.sh` (runs clippy/rustfmt/biome/tsc/shellcheck and the guardrails: host-gateway `lint.sh:216`, startup-path `:240`, legacy-palette, schema-version)
- `./scripts/check-host-gateway.sh` and `./scripts/check-host-gateway.sh --self-test` (`lint.yml:110-113`)
- `./scripts/check-app-deps.sh` (no tauri/gtk/… in crosshook-app's tree)
- `./scripts/check-startup-path.sh` (+ `--selftest`), required if bootstrap is touched for recovery
- `./scripts/check-schema-version.sh` (should be a no-op; no schema change)
- `./scripts/check-mock-coverage.sh` (= `npm run dev:browser:check`). It compares `#[tauri::command]` names against `map.set('…')` in `src/lib/mocks/handlers` **by name only**. Current handlers: `install.ts:112-139`, `update.ts:32-73`, `system.ts:580-655`. The run-executable mock must now emit `run-executable-log` lines (frontend AC), and `update.ts:53-60` shows the `emitMockEvent` pattern.
- From `src/crosshook-native`: `npm run typecheck && npm test && npm run lint && npm run test:smoke` (`package.json:16-26`). Fresh worktree: `npm ci` first and the root setup per `CLAUDE.md:22-26`; `npm run test:smoke:install` once.
- Finish with `./scripts/format.sh --modified --docs`.

## 11. Implementer checklist

### Structure

- [ ] Four modules `services/{install,update,run_executable,prefix_deps}/` (or a shared `services/process/` helper module plus four thin services), each `#[derive(Clone)]`, `Send + Sync` asserted, documented, files ≤ ~300–500 lines, `tests/` subdir.
- [ ] Job kind constants plus a single key helper. Per-prefix `JobKey::from_path(normalized_prefix)`; `Conflict` on the second concurrent job for the same prefix, with no process spawned and no journal write.
- [ ] One shared child runner: lifecycle gateway builder, `process_group(0)`, `kill_on_drop(true)`, `supervise_child(…, DEFAULT_KILL_GRACE)` for cancel, an immediate-kill path for stop plus `kill_processes_using_prefix` sweep, bounded cancel-aware pipe drains, and `log_tail::follow` into `ctx.log_batch` with log files from `create_log_path_in(log_dir, …)`.
- [ ] `start_*` validates, then starts the job, waits for a "started" oneshot, and returns `{ op_id, events }` **before** any completion can be emitted. A reaper task reaps the job.
- [ ] Throwaway-prefix cleanup (`core::run_executable::is_throwaway_prefix_path`) runs in the job's finish path, with bounded retries and a sweep between attempts (moved from `commands/run_executable.rs:242-301`). No sleeps in tests.
- [ ] `PrefixVersionJournal` type (journal peer) with the moved pure helpers and their tests. Recovery runs (a) as a bootstrap job wired in `bootstrap/scans.rs` and (b) inside each install job under the prefix key.
- [ ] `AppError` mappings for the four core error types; legacy texts preserved; offline → `Unavailable` only for download paths.
- [ ] YAN-1538: `check_installed` uses `host_command_with_env_and_directory_lifecycle`, with a `--host --watch-bus` argv test; split `core/prefix_deps/runner.rs` if edited.
- [ ] Core install and run_executable expose command builders (no internal spawn or `block_on`) so the app owns spawn and supervision.

### Wiring and adapters

- [ ] `Services` fields, `Services::new` with the shared `jobs.clone()`, bootstrap wiring test extended; `services/mod.rs` and `lib.rs` re-exports.
- [ ] `src-tauri/src/lib.rs`: `.manage(services.x.clone())`; delete `UpdateProcessState`, `RunExecutableProcessState`, `PrefixDepsInstallState`; delete `commands/log_stream.rs` and its `mod` line.
- [ ] Thin adapters with `map_app_error`; `RunCommandError` envelope preserved for run_executable; event relays keep names and payloads byte-identical; name-only IPC pin tests in all 4 modules (16 commands).
- [ ] Mocks: names unchanged; run-executable mock emits `run-executable-log`.

### Tests (inside crosshook-app)

- [ ] Journal recovery: crash after the version change but before restore; crash after restore (journal still present); failed restore; delete/mark-failed decisions; metadata-disabled skip.
- [ ] protontricks→winetricks fallback; Steam App ID validation (empty, zero, non-decimal, and positive values).
- [ ] Per-prefix exclusivity (`Conflict` on the same prefix, OK on a different one); cancel = TERM, stop = KILL (real `sh`/`sleep` child via the production runner path); `job_counts()` returns to `(0,0)`.
- [ ] Log streaming delivers lines incrementally (`JobEventKind::Log`), and the handle is returned before `Finished`.

### Docs and gates

- [ ] Update `CLAUDE.md`, `AGENTS.md` (MUST bullet, Stack Overview, Directory Map) and `crosshook-app/README.md` (Services, Jobs, Storage boundary). No schema/ADR change unless the gateway API grows.
- [ ] Run every gate in §10, including `-p crosshook-core`.
