# crosshook-app

Application layer for CrossHook: an owner handle (`App`) that owns (or
borrows) a Tokio runtime, exposes a bounded event bus, spawns cancellable
background work, and hosts toolkit-independent services over `crosshook-core`
stores. It is the seam between `crosshook-core` and UI shells (Tauri today,
native UI later). It contains **no UI toolkit dependency**; `App` is not yet
wired into any shell (bootstrap lands in M1-03).

## Design

| Piece        | What it is                                                                                                                                         |
| ------------ | -------------------------------------------------------------------------------------------------------------------------------------------------- |
| `App`        | Cloneable owner handle. Last-clone drop cancels app tasks and (owned) shuts the runtime down with `shutdown_background`.                           |
| `App::spawn` | Eagerly spawns on the app runtime; returns a `Send + 'static` future usable from any executor. Shutdown resolves it to `Cancelled`.                |
| `EventBus`   | Bounded broadcast bus; slow subscribers get `AppEvent::Resync { missed }` and the retained tail. Only `Resync` exists today (`#[non_exhaustive]`). |
| `AppError`   | Sanitized presenter error: stable `AppErrorKind` + message with home paths redacted. Panic payloads and store paths never leak.                    |
| `presenter`  | Public `sanitize_display_path` / `sanitize_display_path_with_home` for redacting `$HOME` in displayed paths.                                       |
| `services`   | Async services below. Blocking store I/O runs in `spawn_blocking`; panics and cancellations map to `AppError`.                                     |

Limits: non-abortable blocking work inside spawned tasks is unsupported (tasks
are dropped at shutdown); callers must not capture an `App` clone inside a
never-ending task (it retains the owner and prevents shutdown). Dropping the
future returned by `App::spawn` does not cancel the task: it detaches and runs
until it completes or the app shuts down.

## Services

| Service                 | API                                                                                                                                                                   | Delegating Tauri commands                                                                                  |
| ----------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------- |
| `SettingsService`       | `load() -> SettingsSnapshot` (data + profiles-dir restart state), `update(closure)` (atomic, normalized), `save_request(SettingsSaveRequest)`, `save_steamgriddb_key` | `settings_load`, `settings_save`, `settings_save_steamgriddb_key`                                          |
| `RecentFilesService`    | `load()`, `save(data)` (unknown keys preserved), `push(RecentFileKind, path)` (move to front, dedupe, cap at `recent_files_limit`)                                    | `recent_files_load`, `recent_files_save`                                                                   |
| `CatalogService`        | `optimization_catalog()`, `command_argument_catalog()`, `mangohud_presets()` (in-memory reads)                                                                        | `get_optimization_catalog`, `get_command_argument_catalog`, `get_mangohud_presets`                         |
| `DiagnosticsService`    | `export(DiagnosticsRequest) -> DiagnosticsExport` (absolute + sanitized display path); `DiagnosticAttachment`s are archived under `app/`                              | `export_diagnostics`                                                                                       |
| `BackgroundGrantHolder` | `request_and_store(&impl BackgroundPortal)`, `store_result`, `protection_state`, `wait_for_initialization`; `XdgBackgroundPortal` is the production portal            | `get_background_protection_state`; Tauri setup spawns `request_and_store`, the launch watchdog awaits init |

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
app shutdown token, so app shutdown cancels every job. Not yet wired into any
command (M1-05, M1-31, M1-32 migrate the existing process-state types).

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
dies with it only when spawned with `--watch-bus`, which today only the
crate-private `crosshook_core::platform::host_command_with_env_and_directory_lifecycle_inner`
adds; the public `host_command*` builders do not, so cancelling such a job can
leave the host process running until a lifecycle-bound builder is exported.

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
  job state, handles, and retained job outcomes.

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
