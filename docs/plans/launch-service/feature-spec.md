# Feature Spec: LaunchService — launch pipeline in `crosshook-app` with session-scoped event streams

Tracking: Linear YAN-798 / GitHub #534 (M1-05). Target `crosshook v0.7.0-alpha.1`, trunk-only off `main`, branch `refactor/yan-798-launch-service`.
Paths are relative to `src/crosshook-native/` unless prefixed `docs/`. Acceptance criteria: [issue.md](./issue.md). Binding decisions D1–D12: [decisions.md](./decisions.md). Invariant ids `I-n`: [research-business.md](./research-business.md). Security ids `C-/H-/M-`: [research-security.md](./research-security.md).

## Executive Summary

About 1,880 lines in `src-tauri/src/commands/launch/` hold the launch pipeline: UMU enrichment, warnings, method resolution, a gamescope predicate copied four times, portal registration, hooks, session registration, watchdog selection, log streaming and a 12-step finalizer. All of it is tied to `AppHandle`. This feature moves it into `crates/crosshook-app/src/services/launch/` as one `LaunchService` that returns a `LaunchSession` with a session-scoped, bounded `broadcast<LaunchEvent>`. `src-tauri` keeps thin `#[tauri::command]` delegates plus a relay that maps events to the existing `launch-log`, `launch-diagnostic`, `launch-complete` and `injection-log` names. Every payload gains `session_id`. The 500 ms whole-file re-read is replaced by `log_tail::follow`. `crosshook-cli launch` drops its copies of `stream_helper_log`, `drain_log` and `safe_read_tail` and uses the service's tail and bounded tail reader. SQLite, TOML and `DiagnosticReport` formats do not change.

## Decisions

D1–D12 in [decisions.md](./decisions.md) are binding. They override the research files, which disagree in places (see Discrepancies).

| Id  | Resolution made by this spec (consistent with D1–D12)                                                                                                                                                                                                                                                                                                                                                                                                          |
| --- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| S1  | `start_*` returns `Result<LaunchSession, LaunchStartError>`. `LaunchStartError { error: AppError, events: Vec<LaunchEvent> }` carries pre-spawn `Injection` events (trainer "requested" and "failed before the process started"), which today are emitted before the `Err` return. Tauri relays them, then returns `error.to_string()`.                                                                                                                        |
| S2  | The service's `Log` batches carry **raw** lines, including empty ones. The gamescope `xdg_backend` suppression, the blank-line filter and the suppression summary are UI concerns and live in the Tauri relay (`LaunchLogRelayState` moves to `src-tauri`). This keeps CLI bytes unchanged (research-cli).                                                                                                                                                     |
| S3  | D2 says `Exited` is last; D5 says `launch-complete` precedes the trainer `injection-log` wrap-up. Resolution: publish order is `Log*` → `Diagnostic?` → trainer wrap-up `Injection*` → `Exited`; then, with no further events, cascade → post-exit hooks → deregister → portal unregister. The only wire difference is that wrap-up `injection-log` rows now arrive before `launch-complete`. No listener depends on that order (checked in research-ux §2.2). |
| S4  | `InjectionLogData` carries `at: SystemTime`, not a string. The relay formats RFC 3339 with `chrono` (already in `src-tauri`). `crosshook-app` gets no new dependency.                                                                                                                                                                                                                                                                                          |
| S5  | The finalizer's diagnostic tail is `read_log_tail(path, MAX_LOG_TAIL_BYTES)`: a seek-and-`take`-bounded, lossy read (D3). There is no ring buffer. It is the single replacement for both `safe_read_tail` copies and is `pub` for the CLI.                                                                                                                                                                                                                     |
| S6  | Test seams are `with_spawner`, `with_env`, `with_helper_source` (consuming builders on `LaunchService`). `new` stays positional (house style).                                                                                                                                                                                                                                                                                                                 |
| S7  | `validate`, `steam_launch_options_command`, `reset_sessions` and `running_profiles` are sync by design (pure or in-memory, no store I/O). Today their Tauri commands are sync too. Everything that touches stores or the filesystem is `async` via `run_blocking`.                                                                                                                                                                                             |
| S8  | The service uses `jobs::create_log_path` (`/tmp/crosshook-logs/<kind>-<slug>-<ms>.log`) exactly as Tauri does today. The CLI keeps its own `launch_log_path` (D10: log paths unchanged).                                                                                                                                                                                                                                                                       |
| S9  | `LaunchResult` is unchanged (no `session_id`). It stays a Tauri DTO.                                                                                                                                                                                                                                                                                                                                                                                           |
| S10 | Command errors become `AppError` (sanitized, `$HOME` instead of `~`). Helper-candidate paths in `runtime_helpers` errors are therefore redacted. Event payloads keep `display_path_tilde` (`~/`) byte-for-byte (H-5). Messages are otherwise kept verbatim.                                                                                                                                                                                                    |

## API (Rust, `crosshook_app::services::launch`)

Every `pub` item needs a doc comment (`#![deny(missing_docs)]`).

```rust
#[derive(Clone)]
pub struct LaunchService { /* profiles, settings, metadata, Arc<LaunchSessionRegistry>,
                              Option<Arc<BackgroundGrantHolder>>, Arc<dyn ProcessSpawner>,
                              HelperSource, fixed LaunchEnv (None = detect per launch) */ }
impl LaunchService {
    pub fn new(profiles: ProfileStore, settings: SettingsStore, metadata: MetadataStore,
               background: Option<Arc<BackgroundGrantHolder>>) -> Self;  // builds its own registry, TokioSpawner, Detect sources
    pub fn with_spawner(self, s: Arc<dyn ProcessSpawner>) -> Self;
    pub fn with_env(self, env: LaunchEnv) -> Self;                       // fixes the env instead of detecting it
    pub fn with_helper_source(self, h: HelperSource) -> Self;
    pub async fn start_game(&self, req: LaunchRequest) -> Result<LaunchSession, LaunchStartError>;
    pub async fn start_trainer(&self, req: LaunchRequest) -> Result<LaunchSession, LaunchStartError>;
    pub fn reset_sessions(&self, profile_name: &str) -> usize;           // cancel_sessions_for_profile(UserRequest)
    pub fn running_profiles(&self) -> Vec<String>;                       // active_profile_keys(Some(Game))
    pub fn validate(&self, req: &LaunchRequest) -> Result<(), LaunchValidationIssue>;
    pub async fn preview(&self, req: LaunchRequest) -> Result<LaunchPreview, AppError>;
    pub fn steam_launch_options_command(&self, enabled_option_ids: &[String], custom_env_vars: &BTreeMap<String, String>,
        gamescope: Option<&GamescopeConfig>, enabled_argument_ids: &[String], custom_command_args: &[String]) -> Result<String, AppError>;
    pub async fn launch_history(&self, profile_name: String, limit: Option<u32>) -> Result<Vec<LaunchHistoryEntry>, AppError>;
}
pub struct LaunchSession { pub id: SessionId, pub kind: SessionKind, pub warnings: Vec<LaunchValidationIssue>,
                           pub log_path: PathBuf, pub events: LaunchEvents }   // log_path: needed for LaunchResult.helper_log_path
pub struct LaunchEvents { /* broadcast::Receiver<LaunchEvent> + session id + done flag */ }
impl LaunchEvents { pub async fn recv(&mut self) -> Option<LaunchEvent>; }     // Lagged(n) -> Log{Dropped{n}}; None after Exited or Closed
pub struct LaunchStartError { pub error: AppError, pub events: Vec<LaunchEvent> }   // Display = error; From<AppError>

#[derive(Debug, Clone, PartialEq)]
pub enum LaunchEvent {
    Log        { session_id: SessionId, batch: LogBatch },                 // log_tail::LogBatch, raw lines
    Injection  { session_id: SessionId, event: InjectionLogData },         // trainer sessions only
    Diagnostic { session_id: SessionId, report: Box<DiagnosticReport> },   // sanitized (~/); only when should_surface_report
    Exited     { session_id: SessionId, code: Option<i32>, signal: Option<i32> },  // exactly once, after all of the above
}
impl LaunchEvent { pub fn session_id(&self) -> SessionId; }
pub struct InjectionLogData { pub at: SystemTime, pub profile_name: String /* trimmed or "Unknown profile" */, pub session_kind: SessionKind,
                              pub level: InjectionLevel, pub source: InjectionSource, pub message: String, pub unsupported_runtime: bool }
pub const LAUNCH_EVENT_CAPACITY: usize = 256;

pub trait ProcessSpawner: Send + Sync + 'static {
    /// Spawns a command already built by a core builder. Production: `command.spawn()`.
    fn spawn(&self, command: &mut tokio::process::Command) -> std::io::Result<tokio::process::Child>;
}
pub struct TokioSpawner;
pub enum HelperSource { Detect, Roots(HelperRoots) }                       // Roots: tests

// gamescope.rs (LaunchEnv is defined in types.rs): pure, no Tauri, no env reads
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LaunchEnv { pub is_flatpak: bool, pub inside_gamescope_session: bool }
impl LaunchEnv { pub fn detect() -> Self }                                 // platform::is_flatpak(), is_inside_gamescope_session()
pub fn execution_method(req: &LaunchRequest, kind: SessionKind, env: LaunchEnv) -> &'static str;
pub fn gamescope_required(req: &LaunchRequest, kind: SessionKind, env: LaunchEnv) -> bool;
pub fn pid_capture_required(req: &LaunchRequest, kind: SessionKind, env: LaunchEnv) -> bool;   // gamescope_required && env.is_flatpak

// Shared by CLI and service (streaming.rs / diagnostics.rs)
pub async fn stream_child_log(child: &mut Child, log_path: &Path, sink: impl FnMut(LogBatch) + Send) -> io::Result<ExitStatus>;
pub async fn read_log_tail(path: &Path, max_bytes: u64) -> String;        // lossy, bounded, errors -> ""
```

`gamescope_required` is `execution_method == METHOD_PROTON_RUN && cfg.enabled && (cfg.allow_nested || !env.inside_gamescope_session)`. `cfg` is `req.gamescope` for a game and `req.resolved_trainer_gamescope()` for a trainer. `execution_method` is `resolved_method()` for a game and `resolve_trainer_launch_method(resolved_method(), env.is_flatpak)` for a trainer. The issue's second parameter `settings` is dropped (D8): no setting feeds the predicate today. Core's `should_skip_gamescope` stays and keeps deciding builder arguments.

`ProcessSpawner` has one method and receives the built command (D1). The trait has no kill, wait or process-group method. `child.wait()` stays plain, and the only kill paths are the core watchdog on a session's own `ShutdownTarget` and registry cancel (C-1). `ShutdownTarget` stays `pub(super)` in core.

## Module layout (`crates/crosshook-app/src/services/launch/`, each file < 500 lines)

| File                     | Owns                                                                                                                                                                             | ≈ lines   |
| ------------------------ | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | --------- |
| `mod.rs`                 | `LaunchService`, `new`, `with_*`, module tree, re-exports, `assert_impl_all!(Send, Sync)`                                                                                        | 150       |
| `types.rs`               | `LaunchSession`, `LaunchEvent`, `LaunchEvents`, `LaunchStartError`, `EventPublisher`, `InjectionLogData`, `LaunchEnv`, crate-private `SessionContext`                            | 260       |
| `spawner.rs`             | `ProcessSpawner`, `TokioSpawner`                                                                                                                                                 | 40        |
| `gamescope.rs`           | `execution_method`, `gamescope_required`, `pid_capture_required`, `GamescopePlan`, `choose_cancel_consumer`, `spawn_cancel_consumer` (watchdog or drain, background-grant wait)  | 200       |
| `warnings.rs`            | moved low-disk, offline, trainer-hash and mod-coexistence collectors, `coexistence_records_from_warnings`; `tokio::task::spawn_blocking` instead of `tauri::async_runtime`       | 190       |
| `portal.rs`              | `try_register_gamemode_portal_for_launch`, moved verbatim                                                                                                                        | 40        |
| `diagnostics.rs`         | `read_log_tail`, `sanitize_diagnostic_report`, `diagnostic_method_for_log`                                                                                                       | 100       |
| `streaming.rs`           | pipe tee (lossy bytes, 64 KiB cap, bounded channel, single file writer), `follow` driver, `stream_child_log`                                                                     | 260       |
| `finalizer.rs`           | `finalize_session`, `SessionGuard` (drop/panic safety), report rewrite, history row, injection wrap-up, teardown                                                                 | 330       |
| `snapshot.rs`            | version snapshot and known-good tagging, moved verbatim (the Steam manifest lookup is moved, not deduplicated)                                                                   | 170       |
| `prepare.rs`             | `Prepared` and the shared pre-spawn steps 1–6 (flags, validate, UMU enrich, warnings, execution method, log path, `GamescopePlan`)                                               | 150       |
| `run.rs`                 | steps 9–19: portal, hooks via `run_blocking`, register, channel, spawn, history start, stream task, cancel consumer                                                              | 280       |
| `game.rs` / `trainer.rs` | `impl LaunchService { start_game / start_trainer }`: method → core builder dispatch                                                                                              | 120 / 170 |
| `queries.rs`             | `impl LaunchService` queries, `reset_sessions`, `running_profiles`, `enrich_umu_gameid`                                                                                          | 150       |
| `tests/`                 | `mod.rs`, `helpers.rs` (Harness, FakeSpawner), `gamescope.rs`, `warnings.rs`, `streaming.rs`, `finalizer.rs`, `snapshot.rs`, `cascade.rs`, `start.rs`, `parity.rs`, `queries.rs` | n/a       |

Other changes:

- `services/mod.rs` gains `pub mod launch; pub use launch::LaunchService;`.
- `Services` gains `launch: LaunchService`. `Services::new` builds the `background` Arc first and clones it into the service (D9).
- `crosshook-app/Cargo.toml` adds the tokio features `io-util` and `fs` (pipe tee).
- `crosshook-cli/Cargo.toml` gains the `crosshook-app` path dependency.

## Wire payloads (Tauri relay; the service never emits Tauri events)

Every `session_id` is `SessionId::to_string()` (hyphenated UUID). It is the same value that `injection-log` already carries today.

| Event               | Before                         | After                                                                                                                                                                                                                             |
| ------------------- | ------------------------------ | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `launch-log`        | bare JSON string, one per line | `{"line": "<text>", "session_id": "<id>"}`, one per non-empty UI line. `normalizeLogMessage` already accepts `{line}`. `Dropped{n}` becomes one line `[crosshook] Console skipped N log batches; the raw launch log is complete.` |
| `launch-diagnostic` | `DiagnosticReport`             | `{...DiagnosticReport, "session_id"}` via a `#[serde(flatten)]` wrapper built at emit time. `DiagnosticReport` and the persisted `diagnostic_json` do not change.                                                                 |
| `launch-complete`   | `{"code","signal"}`            | `{"code": i32\|null, "signal": i32\|null, "session_id"}`. Nulls stay (no `skip_serializing_if`).                                                                                                                                  |
| `injection-log`     | `InjectionLogEvent`            | unchanged shape; the relay builds it from `InjectionLogData`.                                                                                                                                                                     |

The relay is one task per session that drains `LaunchEvents` in order. It emits `Log` lines (S2 transforms) and, before any `Diagnostic` or `Exited`, one suppression-summary line if any lines were suppressed. It emits `launch-diagnostic` for `Diagnostic`, `injection-log` for `Injection`, and `launch-complete` for `Exited`. `bench.rs` replay emits the same `{line, session_id: "bench"}` DTO so G8 measures the real payload. TypeScript: widen `LogPayload`, add `LaunchDiagnosticEvent` and `LaunchCompletePayload` (all `session_id?: string`), and update browser mocks to emit the new shapes with one shared id per session.

## Session lifecycle and finalizer order

`start_game` and `start_trainer` do the following, in order (I-n in brackets):

1. Force `launch_game_only` or `launch_trainer_only` [I-1], then `validate` before any side effect [H-4].
2. Enrich the UMU GAMEID [I-8..10].
3. Collect warnings in the order low-disk, offline, trainer-hash, mod-coexistence [I-11..16].
4. Resolve the execution method [I-6].
5. `create_log_path`.
6. Compute the `GamescopePlan`.
7. Build the command with a core builder. For a trainer, a native request is an error [I-17,18]. Missing helpers are resolved through `HelperSource`.
8. Register the GameMode portal [I-21].
9. Build the hook context [I-23] and run pre-launch hooks through `run_blocking` [D6]. Their warnings are appended to the session warnings.
10. Register the session before spawn (`register`, or the atomic `register_and_link_to_parent_of_kind(Trainer, key, Game)`) [I-26,27]. Create the broadcast channel and its first receiver. Publish the trainer "requested" `Injection`.
11. `spawner.spawn`. On error: trainer "failed before…" `Injection`, `deregister`, and `Err(LaunchStartError)` with the drained events. No history row [I-28,55].
12. Trainer "started" and "unsupported" `Injection`s, then `record_launch_started` [I-46].
13. Spawn the session task. Start the cancel consumer: `CancelConsumer::Watchdog` if `gamescope_required` and the child pid exists, otherwise `drain_cancel_into_outcome` [I-31]. The watchdog is a core task over this session's own `cancel_rx`; the service never holds a kill target (M-2). An empty exe basename still makes the watchdog warn and return without draining; this divergence is kept (D8 item 2, I-32).
14. Return the `LaunchSession`.

Session task, which owns a `SessionGuard` from its first line:

- Pipe mode (`child.stdout/stderr` present): tee raw bytes to the log file. File mode: the child writes the log itself. Both then run `follow(log_path, …)` into `Log` events.
- Await the pipe drain and `child.wait()`. Cancel the tail, so its bounded final drain and partial-line flush deliver the last lines.

`finalize_session` order (D5, S3):

1. Exit code and signal.
2. `read_log_tail`; `diagnostic_method_for_log`; `analyze`; set `log_tail_path`; sanitize [I-41].
3. Watchdog-killed rewrite [I-43].
4. `teardown_reason` and coexistence advisories [I-44].
5. `record_launch_finished` [I-45].
6. If CleanExit or Indeterminate and a profile name exists: version snapshot, then known-good tag [I-47..49].
7. `Diagnostic` event if `should_surface_report` [I-50].
8. Trainer wrap-up `Injection`s [I-52].
9. `Exited` [I-51].
10. Game only: `cancel_linked_children(LinkedSessionExit)` [I-56].
11. Post-exit hooks through `run_blocking` [I-25].
12. `deregister`.
13. Portal `unregister().await` [I-22,53].

`SessionGuard::drop` runs whichever of steps 9–13 had not yet run if the task panicked or was aborted: `Exited { code: None, signal: None }`, then cascade, post-exit hooks, deregister and portal unregister. Steps 1–8 are not replayed. Each step has a done-flag, so `Exited` is published exactly once. The guard uses `Handle::try_current` and `spawn_blocking` for the hooks and a registry call that cannot panic on poison, so unwinding never double-panics (H-1, H-6).

## Invariants checklist (each maps to a test or to "moved verbatim"; full map in the plan)

- **Request**: validate first [I-1..3,H-4]; method resolution split [I-4..7].
- **Warnings**: fail-open and in order, with their DB side effects (offline snapshot row, trainer-hash baseline) kept [I-11..16].
- **Gamescope**: the single predicate reproduces every row of the research-business §2 truth table [I-31..36,D8].
  - Three divergences are kept and filed [I-20,32 and the trainer capture file].
- **Portal and hooks**: [I-21..25]. Hooks move off the executor; their order is unchanged.
- **Ordering**: register before spawn, deregister on spawn error, history start only after a successful spawn [I-26..29].
- **Streaming**: the xdg_backend relay is per session; its transforms live in the relay; the raw log keeps every line [I-37,38,S2]. Bytes are decoded lossily; partial lines are capped at 64 KiB; a bad byte never stops the reader [D7,H-2].
- **Finalizer**: [I-40..53] in the order above, with the guard.
- **Injection-log**: the sequence, messages and levels are unchanged [I-54,55].
- **Cascade**: only a game cascades; reset is per profile key; a trainer without gamescope only records `LinkedSessionExit` and keeps running; a sibling is never signalled [I-56..60,C-1].
  - No `kill_processes_using_prefix`, `killpg`, `supervise_child`, `process_group` or `kill_on_drop` on launch children [D1].
- **Read-only commands**: [I-61,62]; `preview` adds only the mod-coexistence warnings [I-16].
- **Host gateway**: the service only runs commands built by core. The fake spawner lives under `tests/`, which the scanner exempts.
- **Storage**: no schema, TOML or `DiagnosticReport` change; sessions are runtime-only.

## Tauri adapter scope

- `launch_game` and `launch_trainer` stay `#[tauri::command] async fn` with unchanged names, arguments and return types. They call the service, spawn the relay, and return `LaunchResult`. On `Err(LaunchStartError)`, they replay `error.events`, then return `Err(error.to_string())`.
- `validate_launch`, `preview_launch`, `build_steam_launch_options_command`, `list_launch_history_for_profile`, `list_running_profiles` and `launch_reset_sessions` become delegates to `State<'_, LaunchService>`. `check_gamescope_session`, `launch_platform_status` and `check_game_running` stay as core passthroughs (D9).
- `lib.rs`: `.manage(services.launch.clone())` replaces `.manage(Arc::new(LaunchSessionRegistry::new()))`.
- Deleted from `src-tauri`: `streaming.rs`, `warnings.rs`, `portal.rs`, `diagnostics.rs`, the `LaunchStreamContext` and `LaunchHookStreamContext` types, and the `tests/{coexistence,diagnostics,log_relay}.rs` files (they move with their code).
- New: `relay.rs` (event mapping and DTOs) and the adapter tests.

## CLI scope (D10)

- **Uses the service's shared pieces.** `launch_profile` keeps its flow: `from_profile`, UMU enrich, validate, dry-run, the stdio-null spawn per method, its own `launch_log_path`, `--scripts-dir` + `validate_explicit_helper`, and its exit codes. After spawn, `stream_child_log(&mut child, &log_path, print_lines)` replaces `stream_helper_log`/`drain_log`, and `read_log_tail` replaces `safe_read_tail`.
- **Deleted from the CLI.** `stream_helper_log`, `drain_log`, `safe_read_tail`, and the imports they leave unused.
- **Not done.** The CLI does not call `start_game` and writes no history or snapshots. It runs no hooks and does not change `proton_run` output (bugs B2/B3). All of that is YAN-837.
- **Known delta.** An unterminated last log line is printed with a trailing `\n`; CRLF and invalid UTF-8 are normalised. The goldens use LF-terminated ASCII, so they are unaffected. Call this out in the PR body.
- **Pinned by a golden test.** `crates/crosshook-cli/tests/launch_golden.rs` is captured from the **unmodified** CLI in Phase 0, and its fixtures are frozen afterwards. It asserts that `launch_operations` stays empty.

## Persistence and usability

- **TOML**: untouched.
- **SQLite**: `launch_operations` start and finish rows, `version_snapshots` and the known-good flag on `config_revisions` are written by the same calls under the same conditions. There is no schema bump; `check-schema-version.sh` stays green.
- **Runtime-only**: `LaunchSession`, event channels, relay state, `WatchdogOutcome`, the registry and the log files.
- **Offline**: unchanged. Warnings already degrade to empty when metadata is unavailable.
- **Degraded metadata**: with `MetadataStore::disabled()` every finalizer effect is a no-op, and `Exited` is still published exactly once.
- **User-visible**: nothing changes except `session_id` on events and the unified `{line, session_id}` console payload.

## Discrepancies found while spot-checking the research

- **Business I-34** says `BackgroundGrantHolder` lives in `src-tauri`. It is `crosshook_app::services::BackgroundGrantHolder` (`bootstrap/stores.rs`), so no probe seam is needed.
- **Recommendations D1/D4** propose `GameLaunchRequest`/`TrainerLaunchRequest`, a `SpawnedChild` trait, `supervise_child` and `process_group(0)`. D1 supersedes them. This spec uses `LaunchRequest` and a one-method spawner.
- **Technical research** adds `chrono` to `crosshook-app` and passes `kind` to the spawner. This spec uses S4 and D1 instead.
- **D2 and D5 conflict** on `Exited` versus wrap-up ordering; S3 resolves it.
- **Line numbers** in research-recommendations point to a stale base (for example `execution.rs:114-122`). The code agrees with research-business and research-technical (`execution.rs:108-115`, `182-184`, `293-302`).
- **Confirmed against code**: `ShutdownTarget` is `pub(super)`. The trainer builder passes `pid_capture_path: None` (`proton_trainer.rs:139-149`), so the trainer capture-file divergence is real. No `process_group` exists under `launch/`. CI never runs `-p crosshook-cli` (`lint.yml:52`). `pipe_reader_task` stops on the first invalid UTF-8 byte (`streaming.rs:211-230`).

## Out of scope and follow-ups (D8, D12)

- **Kept as-is and filed as issues**: Flatpak trainers never write the gamescope capture file; the watchdog drops the cancel channel when the exe name is empty; the Flatpak Steam trainer hook environment is built as Steam.
- **Other follow-ups**: `create_new`/symlink hardening for `/tmp` log and PID files (A-1); `crosshook-cli/src` in `check-host-gateway.sh` (YAN-837); CLI bugs B1–B4; a `session_id` on `LaunchResult` and a `session_id`-based diagnostic filter in React; deduplicating the four Steam-manifest lookups; sharing `should_skip_gamescope` with the app predicate; whether a non-gamescope trainer should be killed on cascade (a behaviour change).
- **Not built**: launches as `JobRegistry` jobs; launch variants on `AppEvent`; `EventSink`, store or filesystem traits; per-method strategy traits; a `session_id` change to `DiagnosticReport`; a native/iced consumer.

## Research references

[issue.md](./issue.md) · [decisions.md](./decisions.md) · [research-business.md](./research-business.md) (I-1..63, truth tables) · [research-technical.md](./research-technical.md) · [research-practices.md](./research-practices.md) · [research-security.md](./research-security.md) · [research-ux.md](./research-ux.md) · [research-cli.md](./research-cli.md) (golden design) · [research-recommendations.md](./research-recommendations.md)
