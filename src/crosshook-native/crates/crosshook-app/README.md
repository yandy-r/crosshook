# crosshook-app

Application layer for CrossHook: an owner handle (`App`) that owns (or
borrows) a Tokio runtime, exposes a bounded event bus, spawns cancellable
background work, and hosts toolkit-independent services over `crosshook-core`
stores. It is the seam between `crosshook-core` and UI shells (Tauri today,
native UI later). It contains **no UI toolkit dependency**; shells enter
through the two-phase `bootstrap` contract below (`pre_runtime` →
`App::bootstrap`).

## Design

| Piece        | What it is                                                                                                                                                                                                                                        |
| ------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `App`        | Cloneable owner handle. Last-clone drop cancels app tasks and (owned) shuts the runtime down with `shutdown_background`.                                                                                                                          |
| `App::spawn` | Eagerly spawns on the app runtime; returns a `Send + 'static` future usable from any executor. Shutdown resolves it to `Cancelled`.                                                                                                               |
| `EventBus`   | Bounded broadcast bus; slow subscribers get `AppEvent::Resync { missed }` and the retained tail. `#[non_exhaustive]`: `Resync` and `StartupJobFinished { job: StartupJobKind }` (published after a startup job's result is stored in the report). |
| `AppError`   | Sanitized presenter error: stable `AppErrorKind` + message with home paths redacted. Panic payloads and store paths never leak.                                                                                                                   |
| `presenter`  | Public `sanitize_display_path` / `sanitize_display_path_with_home` for redacting `$HOME` in displayed paths.                                                                                                                                      |
| `services`   | Async services below. Blocking store I/O runs in `spawn_blocking`; panics and cancellations map to `AppError`.                                                                                                                                    |

Limits: non-abortable blocking work inside spawned tasks is unsupported (tasks
are dropped at shutdown); callers must not capture an `App` clone inside a
never-ending task (it retains the owner and prevents shutdown). Dropping the
future returned by `App::spawn` does not cancel the task: it detaches and runs
until it completes or the app shuts down.

## Bootstrap

Startup is a typed, two-phase ordering contract in `bootstrap`. Shells must
run Phase A first; Phase B consumes its token, so skipping Phase A is a
compile error.

1. **Phase A** — `pre_runtime(PreRuntimeOptions) -> Result<PreRuntime, BootstrapError>`:
   synchronous, before any Tokio runtime, GTK, or thread spawned by CrossHook. Fixed order: Flatpak gate (+ XDG override in host-XDG shared mode)
   → first-run Flatpak migration (isolated mode only) → legacy Tauri app-id
   migration → UI `EnvPolicy` hook. Environment mutation happens only here,
   checked with a `Threads:` read; a multi-threaded entry (for example threads
   from preloaded libraries such as the Steam overlay) is logged and recorded
   as `Degraded("multi-threaded at entry")` on the env step, but every step
   still runs. `/proc` unreadable proceeds with a warning. Migration failures are
   best-effort: recorded in the report, never fatal.
2. **Phase B** — `App::bootstrap(pre, BootstrapOptions) -> Result<(BootstrappedApp, StartupReport), BootstrapError>`:
   consumes the token and initializes in a fixed order (runtime → settings
   store → logging → remaining stores → catalogs → metadata reconciliation →
   nag sync), then schedules the startup jobs. Fatal failures are typed
   `BootstrapError`s (`StoreInit`, `Runtime`) — never
   `process::exit`. Degraded facilities (metadata DB, unreadable settings,
   logging) continue and are flagged in the report.

### Step log

Every step of both phases appends one `StepRecord { step, outcome }` to a
shared `StepLog`, including skipped steps, so one canonical order holds in
every mode. `PreRuntime::steps()` exposes Phase A's log and
`BootstrappedApp::bootstrap_steps()` the combined log; `verify_pre_runtime_order()` is
the shared Phase-A order checker. The full sequence (16 steps) is asserted in
`tests/bootstrap_report.rs`.

### Startup report

`StartupHandle::snapshot()` (also `BootstrappedApp::startup_report()`, served by the
`get_startup_report` command) returns a live, idempotent `StartupReport`:

- `auto_load_profile` and `onboarding` are **live** — recomputed from the
  settings/profile stores on every snapshot, so a webview reload after
  dismissing onboarding never re-shows it.
- `flatpak` and `degraded` are **frozen** at bootstrap.
- `jobs` accumulate: health batch, offline-readiness notice, version scan,
  umu refresh and background portal each hold a `JobState<T>`
  (`pending | running | complete{data} | failed{message} | skipped{reason}`).
  Results are stored **before** `AppEvent::StartupJobFinished` is published
  (state-then-publish), so a late subscriber pulling a fresh report never
  misses a completion. `StartupHandle::wait_finished(kind)` awaits a job's
  terminal `JobSnapshotKind` with no sleeps.

The JSON shape is the frontend contract (`src/types/startup.ts`): snake_case
keys, `JobState` internally tagged `"state"`; pinned byte-for-byte by the
serde tests in `bootstrap/report/tests.rs`.

### Test seams

The crate feature `test-seams` enables `crosshook-core/bootstrap-trace` and
exposes `#[doc(hidden)]` seams for tests: the `PreRuntimeHost` trait +
`pre_runtime_with_host` / `pre_runtime_misordered_with_host` (drive Phase A
against a fake host, no real environment mutation), and the core `trace` log
the `tests/bootstrap_ordering.rs` harness asserts ordering rules R1–R5
against (own `main`, re-exec per scenario). Production builds keep the host
seam crate-private and omit the misordered path. The `cargo tree` normal-edge
feature checks in CI and `scripts/lint.sh` reject `bootstrap-trace` in shipped builds;
`verify:no-mocks` separately guards frontend mock code.

## Services

| Service                 | API                                                                                                                                                                          | Delegating Tauri commands                                                                                                      |
| ----------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------ |
| `SettingsService`       | `load() -> SettingsSnapshot` (data + profiles-dir restart state), `update(closure)` (atomic, normalized), `save_request(SettingsSaveRequest)`, `save_steamgriddb_key`        | `settings_load`, `settings_save`, `settings_save_steamgriddb_key`                                                              |
| `RecentFilesService`    | `load()`, `save(data)` (unknown keys preserved), `push(RecentFileKind, path)` (move to front, dedupe, cap at `recent_files_limit`)                                           | `recent_files_load`, `recent_files_save`                                                                                       |
| `CatalogService`        | `optimization_catalog()`, `command_argument_catalog()`, `mangohud_presets()` (in-memory reads)                                                                               | `get_optimization_catalog`, `get_command_argument_catalog`, `get_mangohud_presets`                                             |
| `DiagnosticsService`    | `export(DiagnosticsRequest) -> DiagnosticsExport` (absolute + sanitized display path); `DiagnosticAttachment`s are archived under `app/`                                     | `export_diagnostics`                                                                                                           |
| `HealthService`         | `batch_summary()`, `profile_health(name)`, `cached_health_snapshots()`, `cached_offline_readiness_snapshots()` (blocking I/O on the pool; wire types keep their serde names) | `batch_validate_profiles`, `get_profile_health`, `get_cached_health_snapshots`, `get_cached_offline_readiness_snapshots`       |
| `BackgroundGrantHolder` | `request_and_store(&impl BackgroundPortal)`, `store_result`, `protection_state`, `wait_for_initialization`; `XdgBackgroundPortal` is the production portal                   | `get_background_protection_state`; the background-portal startup job runs `request_and_store`, the launch watchdog awaits init |

Attachment names must be a single relative file name (no `/`, `\`, `..`, NUL,
or empty); invalid names fail with `Validation { field: "attachments" }` before
anything is written. With `redact_paths`, UTF-8 attachment contents are
home-path redacted.

`RecentFilesService` clones share one in-process I/O lock, so `load`, `save`,
and `push` through any clone are serialized. Writers outside it (another
process, the CLI, or a separately constructed service) are not covered; the
file write itself stays atomic, so the worst case is a lost update.

## Jobs

`App::jobs()` returns the app's `JobRegistry` (also constructible standalone
with `JobRegistry::new(JobRegistryConfig { event_capacity, retention })` or
`Default`, which spawns on the ambient runtime). Job tokens are children of the
app shutdown token, so app shutdown cancels every job. The startup scans,
umu refresh and background portal requests run as jobs scheduled by
`App::bootstrap` (`startup-scans`, `startup-umu-refresh`,
`startup-background-portal`); remaining process-state migrations are tracked
as M1-05/M1-31/M1-32.

| API                                      | Behavior                                                                                                                                                                                     |
| ---------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `start(JobSpec, \|ctx\| async { .. })`   | Checks the optional `JobKey` (`Conflict`, nothing spawned), creates the bounded channel and the handle's receiver, records the job, then spawns. Returns `JobHandle { id, cancel, events }`. |
| `JobContext`                             | `id()`, `cancel_token()`, `cancelled()`, `log(lines)`, `log_batch`, `progress(JobProgress)`; never blocks; no-op once the job finished, so `Finished` stays last.                            |
| `JobEvents::recv()`                      | `JobEvent { job_id, kind }` with `Log(LogBatch)`, `Progress`, then one `Finished(Outcome)`; `None` afterwards. Lag yields `Log(LogBatch::Dropped { n })`.                                    |
| `subscribe(&JobId)`                      | Live receiver while running; a one-shot `Finished` replay once finished and unreaped; `NotFound` otherwise.                                                                                  |
| `cancel(&JobId)` / `reap` / `outcome`    | Scoped to one job; unknown, foreign, reaped, or evicted ids are `NotFound`. `reap` of a running job is `Conflict`.                                                                           |
| `supervise_child(child, &cancel, grace)` | Child must lead its group (`process_group(0)`, else `InvalidInput`); cancel: `killpg(SIGTERM)`, `SIGKILL` at `grace` if any member lives. Returns `ChildExit::{Natural,Terminated,Killed}`.  |

`Outcome` is `Succeeded`, `Failed(AppError)`, or `Cancelled`; job closures
may return `Outcome` or `Result<(), AppError>` (a `Cancelled` error maps to
`Outcome::Cancelled`). On finish the key is released and the outcome stored
before `Finished` is published, so a caller can restart the key immediately. A
panicking job finishes as `Failed` (`Internal`, payload never surfaced); a job
dropped by runtime shutdown finishes as `Cancelled`. Finished outcomes are
retained up to `retention` (default 64, oldest evicted) until reaped.

`supervise_child` checks group leadership on entry and `SIGKILL`s the group if
its future is dropped (task abort, shutdown) before a final result; a natural
exit leaves surviving members alone, and zombie-only groups count as gone.
Under Flatpak, a host command spawned via `flatpak-spawn --host` makes the
local child the proxy, so the signals reach only the proxy. The host process
dies with it only when spawned with `--watch-bus`, so supervised host commands
must be built with `crosshook_core::platform::host_command_lifecycle` or
`crosshook_core::platform::host_command_with_env_and_directory_lifecycle`. The
other public `host_command*` builders omit `--watch-bus`, so cancelling a job
that uses them leaves the host process running.

`jobs` also hosts the process helpers used by the Tauri shell:
`create_log_path` (`/tmp/crosshook-logs/<prefix>-<slug>-<millis>.log`),
`slugify_target`, and `kill_processes_using_prefix` (`/proc/*/environ`
prefix sweep via the host gateway `kill`).

## Storage boundary

- **TOML settings:** `settings.toml` (`SettingsService`) and `recent.toml`
  (`RecentFilesService`). Formats are unchanged, unknown keys are preserved,
  and writes are atomic. No migration.
- **SQLite metadata:** none written; diagnostics reads metadata status only.
- **User-requested export:** the diagnostics `.tar.gz`, written to the chosen
  directory (system temp by default).
- **Runtime-only:** the Background portal grant and its protection state;
  job state, handles, and retained job outcomes; the startup report, its
  step log and job results (served by `get_startup_report`, never
  persisted).

Offline behavior is unaffected. Save failures surface as `AppErrorKind::Io`.
A missing portal resolves to `Unavailable`/`NotApplicable` and launches
continue. Users view and edit settings and recent files through the settings
UI; the diagnostics archive is a regular file the user chose to create.

## Dependencies

`crosshook-core`, `tokio` (rt-multi-thread, sync, time, macros, process),
`tokio-util` (rt), `nix` (signal), `uuid` (v4), `thiserror`, `serde` (derive),
`tracing`, and `static_assertions`. Dev: `tempfile`, `serde_json`, `toml`,
`tar`, `flate2`, `tokio` (test-util, macros, rt, io-util).
`scripts/check-app-deps.sh` guards against UI-toolkit leakage.
