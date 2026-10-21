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

## Storage boundary

- **TOML settings:** `settings.toml` (`SettingsService`) and `recent.toml`
  (`RecentFilesService`). Formats are unchanged, unknown keys are preserved,
  and writes are atomic. No migration.
- **SQLite metadata:** none written; diagnostics reads metadata status only.
- **User-requested export:** the diagnostics `.tar.gz`, written to the chosen
  directory (system temp by default).
- **Runtime-only:** the Background portal grant and its protection state.

Offline behavior is unaffected. Save failures surface as `AppErrorKind::Io`.
A missing portal resolves to `Unavailable`/`NotApplicable` and launches
continue. Users view and edit settings and recent files through the settings
UI; the diagnostics archive is a regular file the user chose to create.

## Dependencies

`crosshook-core`, `tokio` (rt-multi-thread, sync, time, macros), `tokio-util`
(rt), `thiserror`, `serde` (derive), `tracing`, and `static_assertions`. Dev:
`tempfile`, `serde_json`, `toml`, `tar`, `flate2`.
`scripts/check-app-deps.sh` guards against UI-toolkit leakage.
