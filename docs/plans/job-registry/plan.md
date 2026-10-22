# Job registry and incremental log tailing — plan

Tracking: Linear YAN-789 (M1-04) / GitHub yandy-r/crosshook#521. Target release: `crosshook v0.7.0-alpha.1`.
Paths are relative to `src/crosshook-native/`.

## Goal

One toolkit-independent supervisor for long-running work in `crosshook-app`, replacing (in later issues M1-05, M1-31,
M1-32) the four ad hoc process-state types and the 500 ms whole-file log re-read. This PR adds the registry, the tailer
and the moved helpers; it does not migrate existing commands (unwired, so `main` stays releasable).

## Decisions

| Topic                  | Decision                                                                                                                                                                                                                                                                                                                                                                                  |
| ---------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Placement              | `crates/crosshook-app/src/jobs/` (registry, handle, events, process supervision) and `crates/crosshook-app/src/log_tail.rs` (tailer + coalescer). `App::jobs()` exposes one registry whose tasks are children of the app shutdown token.                                                                                                                                                  |
| Subscribe-before-start | `JobRegistry::start` creates the per-job `tokio::sync::broadcast` channel and its first receiver, records the job, and only then spawns the task. The receiver is moved into the returned `JobHandle`, so an instantly finishing job cannot outrun its caller.                                                                                                                            |
| Late subscribers       | `subscribe(&JobId)` on a running job returns a live receiver; on a finished, unreaped job it returns a receiver that yields the retained `Finished(Outcome)` once. Finished jobs are retained until `reap(&JobId)` or until the bounded retention cap (oldest reaped first), so memory is bounded.                                                                                        |
| Ordering on finish     | Release the `JobKey`, store the outcome, then publish `Finished`. A caller reacting to `Finished` can restart the same key immediately.                                                                                                                                                                                                                                                   |
| Backpressure           | Per-job bounded broadcast channel (configurable capacity). A lagging receiver gets `JobEvent::Log(LogBatch::Dropped { n })` with the lag count, then the retained tail. `Finished` is the newest event, so it is never lost to lag. No unbounded channels anywhere.                                                                                                                       |
| Ownership check        | `JobId` is an unguessable UUID v4 (same as the ProtonUp `op_id`). Every event carries its `JobId`; each handle receives only its own job's events. `cancel` with an unknown, foreign or already-reaped id is `NotFound` and touches no other job. This preserves the ProtonUp `op_id` contract.                                                                                           |
| `JobKey`               | Optional uniqueness key (e.g. the prefix path). Starting a second job with an active key fails with `Conflict`; no job is spawned.                                                                                                                                                                                                                                                        |
| Cancellation           | `CancellationToken` per job. `supervise_child` spawns nothing itself; for a child started in its own process group (`process_group(0)`), cancel sends `SIGTERM` to the group via `nix::sys::signal::killpg`, waits a configurable grace period, then `SIGKILL`s the group.                                                                                                                |
| Log tailing            | Offset-based reader: reads only new bytes, at most 256 KiB per poll (larger backlogs drain over following polls), keeps a partial-line buffer capped at 64 KiB (overflow is emitted as a line cut at a UTF-8 char boundary), decodes complete lines with lossy UTF-8, resets on truncation (size < offset) and reopens on rotation (inode change). Tracks total bytes read.               |
| Watcher                | No `notify` dependency. Adaptive polling (once per coalescing window while data flows, backing off to 500 ms when idle) keeps the dependency tree unchanged and works on tmpfs/Flatpak paths. Lines are coalesced into one `LogBatch` per window (default 33 ms, clamped to 16–33 ms). On cancel, the backlog is drained for at most 16 chunks (4 MiB), then the partial line is flushed. |
| Moved helpers          | `create_log_path`, `slugify_target`, `kill_processes_using_prefix` (and its `/proc/*/environ` boundary matcher + tests) move from `src-tauri/src/commands/shared.rs` to `crosshook-app` (`jobs::process`). src-tauri callers import from `crosshook_app`; the old copies are deleted.                                                                                                     |
| Host tools             | The prefix sweep keeps `crosshook_core::platform::host_std_command("kill")`. `killpg` is a syscall, not a host tool. `./scripts/check-host-gateway.sh` must pass.                                                                                                                                                                                                                         |

## Storage boundary

Runtime-only. Job state, handles, retained outcomes and tail offsets live in memory. Log files keep their location in
`/tmp/crosshook-logs`. No TOML or SQLite change.

## Persistence & usability

No migration and no user-visible change in this PR (the registry is not wired into commands yet). Works offline. If a log
file disappears, the tailer reports nothing new and resumes on recreation. Users keep reading the same log files.

## Tasks

1. Registry slice: `jobs/` module, `App::jobs()`, `Cargo.toml` (`uuid`, `nix` `signal`, tokio `process`), helper move,
   src-tauri import updates, README.
2. Tailer slice: `log_tail.rs` (`LogTail` reader + `follow` coalescing loop) with tests.

## Verification

- `cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-app` (race ×1000, tail bytes = file size,
  truncation, rotation, partial lines, invalid UTF-8, cancel graceful/forced, `JobKey` exclusivity, coalescing windows
  with `tokio::time::pause`, dropped counter, ownership).
- `cargo test -p crosshook-native` (moved helper callers), `cargo clippy --all-targets -D warnings` on touched crates.
- `./scripts/check-host-gateway.sh`, `./scripts/check-app-deps.sh`, `./scripts/lint.sh`.
