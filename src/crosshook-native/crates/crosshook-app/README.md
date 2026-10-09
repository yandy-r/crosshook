# crosshook-app

Application scaffold for CrossHook: an owner handle (`App`) that owns (or
borrows) a Tokio runtime, exposes a bounded event bus, and spawns cancellable
background work. It is the future seam between `crosshook-core` and the Tauri
IPC layer — but contains **no Tauri, CLI, or UI wiring yet**.

## Design

| Piece        | What it is                                                                                                                                         |
| ------------ | -------------------------------------------------------------------------------------------------------------------------------------------------- |
| `App`        | Cloneable owner handle. Last-clone drop cancels app tasks and (owned) shuts the runtime down with `shutdown_background`.                           |
| `App::spawn` | Eagerly spawns on the app runtime; returns a `Send + 'static` future usable from any executor. Shutdown resolves it to `Cancelled`.                |
| `EventBus`   | Bounded broadcast bus; slow subscribers get `AppEvent::Resync { missed }` and the retained tail. Only `Resync` exists today (`#[non_exhaustive]`). |
| `AppError`   | Sanitized presenter error: stable `AppErrorKind` + message with home paths redacted. Panic payloads and store paths never leak.                    |
| `presenter`  | Public `sanitize_display_path` / `sanitize_display_path_with_home` for redacting `$HOME` in displayed paths.                                       |

Limits: non-abortable blocking work inside spawned tasks is unsupported (tasks
are dropped at shutdown); callers must not capture an `App` clone inside a
never-ending task (it retains the owner and prevents shutdown). Dropping the
future returned by `App::spawn` does not cancel the task: it detaches and runs
until it completes or the app shuts down.

## Storage boundary

**Runtime-only.** This crate persists nothing — no TOML settings, no SQLite
metadata, no filesystem access. Offline behavior is unaffected, there is no
migration or backward-compatibility surface, and nothing is user-visible or
editable. Tauri commands: none (wiring lands in later phases).

## Dependencies

Only `crosshook-core`, `tokio` (rt-multi-thread, sync, time, macros),
`tokio-util` (rt), and `thiserror`. `time` backs timers on the owned runtime
for `App::spawn` users. `scripts/check-app-deps.sh` guards against UI-toolkit leakage.
