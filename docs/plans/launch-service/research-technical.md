# M1-05 launch service — technical research

Tracking: Linear YAN-798 / GitHub #534. Paths are relative to `src/crosshook-native/` unless absolute.
Read-only investigation. `[INFERENCE]` marks claims I did not observe directly.

## 1. Current surface: `src-tauri/src/commands/launch/` (~1,880 lines)

| File             | Lines | Role                                                                                                                                                                                                                                                                 | Tauri coupling                                                                                                                                                                                                  |
| ---------------- | ----- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------- | ---------------------------------- |
| `mod.rs`         | 52    | re-exports + `__cmd__*` macro re-exports for `generate_handler!`                                                                                                                                                                                                     | macros only                                                                                                                                                                                                     |
| `execution.rs`   | 641   | `launch_game` (:62), `launch_trainer` (:239), `launch_reset_sessions` (:503), `consume_cancel_channel` (:519), `record_launch_start` (:558), `spawn_gamescope_watchdog` (:592)                                                                                       | `AppHandle`, `State<ProfileStore/SettingsStore/Arc<LaunchSessionRegistry>>`, `app.state::<MetadataStore>()`, `app.state::<Arc<BackgroundGrantHolder>>()` (:612), `tauri::async_runtime::{spawn,spawn_blocking}` |
| `streaming.rs`   | 542   | `spawn_log_stream` (:26), file-poll `stream_log_lines` (:61), pipe `stream_log_pipes` (:142), `pipe_reader_task` (:211), `finalize_launch_stream` (:232), `emit_injection_stream_finalization` (:447), `finalize_launch_session` (:513)                              | `app.emit("launch-log"                                                                                                                                                                                          | "launch-diagnostic" | "launch-complete")`, async_runtime |
| `shared.rs`      | 208   | xdg_backend relay suppression (:23-63), `LaunchResult` (:66), `InjectionLog{Level,Source,SessionKind,Event}` (:76-125), `build_injection_log_event` (:128), `emit_injection_log_event` (:157), `LaunchStreamContext` (:189), `LaunchHookStreamContext` (:205)        | `Emitter` in `emit_injection_log_event` only                                                                                                                                                                    |
| `warnings.rs`    | ~180  | offline (:14), trainer-hash (:61), mod-coexistence (:90), `coexistence_records_from_warnings` (:126), low-disk (:145)                                                                                                                                                | only `tauri::async_runtime::spawn_blocking`                                                                                                                                                                     |
| `queries.rs`     | ~140  | `validate_launch` (:23), `preview_launch` (:28), `build_steam_launch_options_command` (:62), `check_gamescope_session` (:96), `launch_platform_status` (:102), `check_game_running` (:107), `list_running_profiles` (:116), `list_launch_history_for_profile` (:124) | `State<…>`                                                                                                                                                                                                      |
| `diagnostics.rs` | 76    | `safe_read_tail` (:8), `sanitize_diagnostic_report` (:43), `diagnostic_method_for_log` (:66)                                                                                                                                                                         | none (uses `commands::shared::sanitize_display_path`, which is `crosshook_app::presenter::display_path_tilde`, `commands/shared.rs:6`)                                                                          |
| `portal.rs`      | 38    | `try_register_gamemode_portal_for_launch` (:12)                                                                                                                                                                                                                      | none                                                                                                                                                                                                            |
| `tests/`         | —     | `coexistence.rs`, `diagnostics.rs`, `log_relay.rs` (pure unit tests; they move with their code)                                                                                                                                                                      | none                                                                                                                                                                                                            |

The only real Tauri dependencies are event emission, `State` injection, `AppHandle::state`, and `tauri::async_runtime`, which is the same Tokio handle that `crosshook-app` bootstraps with (`src-tauri/src/lib.rs`, `.with_runtime_handle(tauri::async_runtime::handle()…)`). Everything else is core calls, so the move does not need a domain redesign.

The registry is managed outside `Services`: `src-tauri/src/lib.rs:58-60` `.manage(Arc::new(LaunchSessionRegistry::new()))`.

## 2. `launch_game` vs `launch_trainer` step by step

| #   | `launch_game` (execution.rs)                                                                                                                                                       | `launch_trainer` (execution.rs)                                                                                                                                                                                                                        |
| --- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| 1   | `launch_game_only=true, launch_trainer_only=false` (:70-71)                                                                                                                        | inverse (:247-248)                                                                                                                                                                                                                                     |
| 2   | `validate(&request)`                                                                                                                                                               | same                                                                                                                                                                                                                                                   |
| 3   | `enrich_umu_gameid_resolution` (:47): loads settings, `umu_database::resolve_umu_game_id(req, settings.umu_database_lookup, &metadata)` (core `umu_database/resolver.rs:265`)      | same                                                                                                                                                                                                                                                   |
| 4   | warnings: `collect_offline_launch_warnings` (+ low-disk), `collect_trainer_hash_launch_warnings_ipc`, `collect_mod_coexistence_warnings_ipc` → `coexistence_records_from_warnings` | **identical copy** (~30 lines duplicated)                                                                                                                                                                                                              |
| 5   | `method` = `resolved_method()` folded to `&'static str`                                                                                                                            | `resolved_method` + `execution_method = resolve_trainer_launch_method(resolved, is_flatpak)` (core `profile/models/resolve.rs:32`: Steam+Flatpak → `proton_run`)                                                                                       |
| 6   | `create_log_path("game", slug)` (`crosshook_app::jobs`, `jobs/process.rs:228`, `/tmp/crosshook-logs/<prefix>-<slug>-<ms>.log`)                                                     | `create_log_path("trainer", …)`                                                                                                                                                                                                                        |
| 7   | Flatpak pid-capture predicate (:109-112)                                                                                                                                           | trainer predicate (:293-302)                                                                                                                                                                                                                           |
| 8   | command: steam → `runtime_helpers::resolve(SteamLaunchHelper)` + `build_helper_command`; proton → `build_proton_game_command` + piped stdio; native → `build_native_game_command`  | steam+Flatpak → `build_flatpak_steam_trainer_command` + piped; steam host → `resolve(SteamLaunchTrainer)` + `build_trainer_command`; proton → `build_proton_trainer_command` + piped; native → `Err("native launch does not support trainer launch.")` |
| 9   | `try_register_gamemode_portal_for_launch(&req, method)`                                                                                                                            | `(&req, execution_method)`                                                                                                                                                                                                                             |
| 10  | `build_launch_hook_execution_context` + `run_pre_launch_hooks` (**blocking**: `hooks.rs` `wait_with_timeout`, called on the async executor)                                        | same                                                                                                                                                                                                                                                   |
| 11  | `registry.register(Game, key)` (:150)                                                                                                                                              | `register_and_link_to_parent_of_kind(Trainer, key, Game)` (:347), atomic, plus a `tracing` parent log                                                                                                                                                  |
| 12  | —                                                                                                                                                                                  | `injection-log` "Trainer launch requested." (Info/Trainer)                                                                                                                                                                                             |
| 13  | `command.spawn()`; on error `deregister` + `Err("failed to launch helper: …")`                                                                                                     | on error `injection-log` Error "…before the trainer process started.", `deregister`, `Err("failed to launch trainer (method=…): …")`                                                                                                                   |
| 14  | —                                                                                                                                                                                  | `injection-log` "Trainer process started." and Warning/Injection "DLL injection engine is not available…" `unsupported_runtime=true`                                                                                                                   |
| 15  | `record_launch_start(metadata, profile, method, sanitized_log_path)` → `operation_id` (spawn_blocking `record_launch_started`)                                                     | same with `execution_method`                                                                                                                                                                                                                           |
| 16  | build `LaunchStreamContext` (snapshot fields: `steam.app_id`, `trainer_host_path`, `profile_name`, `steam_client_install_path`, `WatchdogOutcome`, session, coexistence, hooks)    | same, `session_kind: Trainer`                                                                                                                                                                                                                          |
| 17  | `spawn_log_stream(app, log, child, method, ctx, portal_guard)`                                                                                                                     | same with `execution_method`                                                                                                                                                                                                                           |
| 18  | `consume_cancel_channel(gamescope_active, pid, …, game exe basename, …)`                                                                                                           | same with trainer basename                                                                                                                                                                                                                             |
| 19  | `LaunchResult { succeeded, "Game launch started.", helper_log_path, warnings }`                                                                                                    | "Trainer launch started."                                                                                                                                                                                                                              |

Steps 2-4, 9-10, 15-18 are shared code. Steps 5, 7-8, 11-14 are kind-specific. Put the shared part in a single `prepare`/`run_session` pipeline.

### 2.1 Gamescope gating — every copy

Tauri layer (the copies the issue means):

```rust
// execution.rs:109-112 (game, Flatpak PID capture)
request.resolved_method() == METHOD_PROTON_RUN && crosshook_core::platform::is_flatpak()
    && request.gamescope.enabled
    && (request.gamescope.allow_nested || !is_inside_gamescope_session())
// execution.rs:182-184 (game, watchdog gate)
method == METHOD_PROTON_RUN && request.gamescope.enabled
    && (request.gamescope.allow_nested || !is_inside_gamescope_session());
// execution.rs:293-296 (trainer, watchdog gate) + :297-298 (trainer, PID capture)
let trainer_gamescope = request.resolved_trainer_gamescope();
execution_method == METHOD_PROTON_RUN && trainer_gamescope.enabled
    && (trainer_gamescope.allow_nested || !is_inside_gamescope_session());
if trainer_gamescope_active && crosshook_core::platform::is_flatpak() { … }
```

The core builders carry the same rule in another form: `should_skip_gamescope(cfg) = !cfg.allow_nested && env GAMESCOPE_WAYLAND_DISPLAY` (`script_runner/common.rs:52`), duplicating `is_inside_gamescope_session` (`request/models.rs:13`). It is used at `proton_game.rs` (`gamescope_active = request.gamescope.enabled && !should_skip_gamescope(..)`), `proton_trainer.rs` (trainer config) and `steam_helpers.rs:82-84`. The builders decide argument wrapping and the Flatpak PID-capture script (`proton_game.rs` → `build_proton_command_with_gamescope_pid_capture_in_directory`, `runtime_helpers/proton_command.rs:234`). The Tauri copies decide watchdog spawn and the capture path. They must agree, or the watchdog waits 20 × 200 ms on a capture file that is never written (`watchdog/mod.rs:20-21`, `resolver.rs` `resolve_watchdog_target`).

What the rule does today:

- Game `steam_applaunch`/`native`: never active (the helper script owns gamescope, so there is no watchdog).
- Trainer `steam_applaunch` on host: `execution_method` stays steam, so inactive. In Flatpak it is rewritten to `proton_run`, so active whenever the trainer config is enabled.
- The trainer config is `resolved_trainer_gamescope()` (`models.rs:203`: explicit override → auto-windowed from game → disabled).

The single predicate should be pure for table tests:

```rust
// services/launch/gamescope.rs
#[derive(Clone, Copy)] pub struct LaunchEnv { pub is_flatpak: bool, pub inside_gamescope_session: bool }
impl LaunchEnv { pub fn detect() -> Self { /* platform::is_flatpak(), is_inside_gamescope_session() */ } }
pub struct GamescopePlan { pub active: bool, pub pid_capture: Option<PathBuf> }
pub fn gamescope_required(req: &LaunchRequest, kind: SessionKind, env: LaunchEnv) -> bool;
pub fn gamescope_plan(req: &LaunchRequest, kind: SessionKind, env: LaunchEnv, log: &Path) -> GamescopePlan;
```

`execution_method` comes from `resolve_trainer_launch_method(req.resolved_method(), env.is_flatpak)` for Trainer and `resolved_method()` for Game. The config comes from `req.gamescope` or `req.resolved_trainer_gamescope()`. The issue's "settings" argument maps to `LaunchEnv`: no persisted setting feeds this predicate today.

Table test axes (AC1): method {steam, proton_run, native} × flatpak {t,f} × config {disabled, enabled, enabled+allow_nested} × inside_session {t,f} × kind {game, trainer}. Optional follow-up `[INFERENCE]`: make core expose `pub fn gamescope_compositor_active(cfg, inside) -> bool` so `should_skip_gamescope` and the app predicate share one expression. The builders would still read the env themselves, so tests can't fully inject it there.

### 2.2 Session registry (core `launch/session/`)

- `LaunchSessionRegistry` (`registry.rs:27`, `Mutex<HashMap<SessionId, SessionEntry>>`). Methods:
  - `register(kind, key) -> (SessionId, broadcast::Receiver<TeardownReason>)` (:40)
  - `deregister(id)` (:54, idempotent)
  - `link_to_parent` (private, :63)
  - `sessions_for_profile` (:112, newest first)
  - `active_profile_keys(kind)` (:142)
  - `register_and_link_to_parent_of_kind(kind, key, parent_kind) -> (id, rx, Option<parent>)` (single lock; only Trainer→Game is legal)
  - `cancel_linked_children(parent, reason) -> usize` (:188, sends with the lock released)
  - `cancel_session(id, reason) -> bool`
  - `cancel_sessions_for_profile(key, reason)` (:233)
- Types (`types.rs`): `SessionId(Uuid)` (:21, `Display`, serde); `SessionKind {Game, Trainer}` (:38); `TeardownReason` (:50, snake_case, persisted in `diagnostic_json`); `SessionEntry` with a per-session `cancel_tx`; `WatchdogOutcome` (:160: `mark`, `record_reason`, `was_killed`, `reason`).
- Keys: `session_profile_key_for_name(Option<&str>)` (`session/keys.rs:19`).
- `ShutdownTarget { pid, host_namespace }` is **`pub(super)`** in `launch/watchdog/mod.rs:25`. It is built by `resolver.rs` from either the observed gamescope pid (sandbox) or the Flatpak capture file / host ancestor search (host namespace). Each watchdog task owns exactly one `ShutdownTarget` and `shutdown_gamescope_tree(target, &outcome, reason)` (`tasks.rs:143`) kills only that target's descendants. The service should not need `ShutdownTarget`. "Session owns its ShutdownTarget" holds because one watchdog is spawned per session with that session's own child pid and capture path.
- **Cascade semantics:** `cancel_linked_children` only signals. A trainer with gamescope tears down its own tree in the watchdog. A trainer **without** gamescope gets `drain_cancel_into_outcome` (`session/drain.rs:31`), which only `record_reason`s. The trainer process is **not killed** today. Cascade tests should assert the recorded `TeardownReason::LinkedSessionExit` plus the "sibling not signalled" property, not process death, unless the plan decides to add a kill (behaviour change).

### 2.3 Watchdog selection and background-grant wait

`consume_cancel_channel` (execution.rs:519):

- `gamescope_active && child.id().is_some()` → `spawn_gamescope_watchdog`.
- Otherwise it `warn!`s when gamescope was active but the pid is missing, then `spawn(drain_cancel_into_outcome(..))`. Every `cancel_rx` gets a consumer.

`spawn_gamescope_watchdog` (:592):

- An empty exe basename means warn and return. The rx is dropped, so a cascade is then lost (`[INFERENCE]`, minor).
- If `platform::portals::background::background_supported()`, it spawns a task that runs `holder.wait_for_initialization(500ms)` (`services/background.rs:294`) and logs the protection state.
- The watchdog task awaits that handle, then runs core `gamescope_watchdog(pid, exe, outcome, capture, rx)` (`watchdog/tasks.rs:31`).

The holder already lives in `Services.background: Arc<BackgroundGrantHolder>` (`bootstrap/stores.rs:58`), so the service can take it directly with no `AppHandle`.

### 2.4 Streaming and finalizer side effects

Streaming (streaming.rs). Mode is chosen by `child.stdout.is_some() || child.stderr.is_some()`:

- **Pipe mode** (proton game, proton trainer, Flatpak Steam trainer): two `pipe_reader_task`s feed an unbounded mpsc. Each line is appended to the log file **and** emitted (`transform_launch_log_line_for_ui`).
- **File mode** (steam helper/trainer scripts write the log themselves; native uses `attach_log_stdio`, `runtime_helpers/path_resolution.rs:84`): `loop { tokio::fs::read_to_string(&log) … sleep(500ms) }` (:81-119). This is the **whole-file re-read every 500 ms**, with truncation reset and a final re-read (:122).
- Both modes emit `suppression_summary_line` at the end.

`finalize_launch_stream` (:232), in order:

1. `exit_code`, `signal` (`ExitStatusExt`).
2. `safe_read_tail(log, MAX_LOG_TAIL_BYTES=2 MiB)` (`diagnostics/models.rs:6`). This is a **second full read**.
3. `diagnostic_method_for_log` → `analyze(exit, tail, method)` → `log_tail_path = sanitize` → `sanitize_diagnostic_report`.
4. If `watchdog_outcome.was_killed()`, rewrite to CleanExit/Info with "Game|Trainer exited; gamescope compositor cleaned up." and clear suggestions.
5. `teardown_reason = outcome.reason().or(NaturalExit)`; `coexistence_advisories`.
6. If there is an `operation_id`: spawn_blocking `record_launch_finished(op, code, signal, &report)`. This is the **history row**.
7. If `failure_mode ∈ {CleanExit, Indeterminate}` and `profile_name`:
   - **Version snapshot** (:286-385): `lookup_profile_id` → Steam roots/libraries → `appmanifest_<id>.acf` `parse_manifest_full` → `hash_trainer_file(trainer_host_path)` → `lookup_latest_version_snapshot` → `compute_correlation_status`. Skip on `UpdateInProgress`, otherwise `upsert_version_snapshot`.
   - **Known-good** (:387-424): `list_config_revisions(id, Some(1))` → `set_known_good_revision(id, latest.id)`.
   - Both run for **game and trainer** sessions.
8. If `should_surface_report`: emit `launch-diagnostic` with the `DiagnosticReport`.
9. Emit `launch-complete` `{code, signal}`, for **every** session kind.
10. `emit_injection_stream_finalization` (Trainer only). If teardown is not NaturalExit, emit Warning/Runtime "Trainer session teardown was recorded." Then:
    - natural clean exit → Info "Trainer launch completed.";
    - Indeterminate → Warning/Runtime "…indeterminate.";
    - otherwise → Error "Trainer launch ended with diagnostics."
11. `finalize_launch_session` (:513):
    - Game → `cancel_linked_children(id, LinkedSessionExit)`.
    - `run_post_exit_hooks` (**blocking**, called on the async executor).
    - `deregister(id)`.
12. `spawn_log_stream` (:46-52): `GameModeRegistration::unregister().await` (`platform/portals/gamemode.rs:250`).

All metadata APIs are on `MetadataStore`: `record_launch_started/finished` (`metadata/launcher_ops.rs:51/62`), `lookup_profile_id` (`profile_ops.rs:31`), `upsert_version_snapshot`/`lookup_latest_version_snapshot` (`version_ops.rs:6/30`), `list_config_revisions`/`set_known_good_revision` (`config_history_ops.rs:72/96`), `query_launch_history_for_profile` (`launch_queries.rs:567`). Tests can use `MetadataStore::open_in_memory()` (`metadata/store.rs:57`), as the ProfileWriter harness does (`services/profiles/tests/helpers.rs:41`).

## 3. M1-04 building blocks (`crosshook-app`)

- `log_tail::LogTail` (`log_tail.rs`): offset-tracking sync reader. API: `new`, `poll() -> io::Result<Vec<String>>` (≤ `READ_CHUNK`=256 KiB), `has_backlog`, `flush_partial` (:178), `bytes_read`. It handles a missing file, truncation and rotation.
- `log_tail::follow(path, TailConfig, CancellationToken, sink: FnMut(LogBatch)) -> io::Result<()>` (:332):
  - polls immediately, then on a 16-33 ms window while data flows, with idle backoff to 500 ms;
  - reads on the blocking pool;
  - on cancel, drains ≤ `FINAL_DRAIN_CHUNKS`=16 polls and flushes the partial line;
  - `sink` must be non-blocking.
- `LogBatch::{Lines(Vec<String>), Dropped{n}}`.
- `jobs::{JobRegistry, JobContext::log_batch, JobEvent{job_id, kind: Log|Progress|Finished(Outcome)}, supervise_child, ChildExit, create_log_path, slugify_target}` (`jobs.rs`). `App::jobs()` is the app-owned registry (`app.rs:112`). Its tokens are children of the app shutdown token.

### How they replace the 500 ms re-read

**Recommendation: one streaming path.** In pipe mode the pipe readers only _append to the log file_. Both modes then run `follow(log_path, TailConfig::default(), tail_cancel, sink)`. The sink:

1. applies `LaunchLogRelayState` suppression;
2. pushes raw lines into a bounded byte ring (`TailBuffer`, capped at `MAX_LOG_TAIL_BYTES`), which replaces `safe_read_tail` in the finalizer (no second read);
3. `try_send`s `LaunchEvent::Log { session_id, batch }`.

After `child.wait()` (and, in pipe mode, after the pipe writers flush), cancel `tail_cancel` and await `follow`. Its final drain and flush delivers the last lines. The CLI consumes the same events.

There is one small behaviour difference: the ring starts on a line boundary, while `safe_read_tail` may start mid-line. `analyze` only scans patterns, so `[INFERENCE]` this is harmless.

**Do not run launches as `JobRegistry` jobs.** `JobEventKind` has no Diagnostic/Injection/Exited-code variants, and `Outcome` drops exit codes (`jobs/types.rs:99`). Launch cancel semantics are registry-linked, not token-only. `supervise_child` requires a process-group leader (`jobs/process.rs:84`), but the launch builders never set `process_group(0)` (none found in `crosshook-core/src/launch`), and the gamescope tree lives in the host namespace. Reuse `log_tail` and `create_log_path`. Keep `LaunchSessionRegistry` for lifecycle. Also, game sessions should **not** be cancelled on app shutdown (no such behaviour today), so the service should not derive from the app shutdown token.

## 4. M1-01 and M1-29

- `LaunchRequest::from_profile(&GameProfile, method, steam_client_path, profile_name, umu_default) -> Result<Self, LaunchRequestError>` (`launch/request/from_profile.rs:41`).
  - Flag-neutral: the caller sets `launch_game_only`/`launch_trainer_only`.
  - Errors: `MissingGamePath`, `UnsupportedMethod`.
  - Golden tests: `crosshook-core/tests/launch_request_golden.rs:72`.
  - Callers today: the CLI only (`crosshook-cli/src/launch.rs:45`). Tauri receives a frontend-built request (`src/utils/launch.ts`).
  - The service API should take a `LaunchRequest`, which Tauri passes through and the CLI builds via `from_profile`.
- `runtime_helpers::resolve(RuntimeHelper) -> Result<PathBuf, RuntimeHelperError>` (`crosshook-core/src/runtime_helpers/mod.rs:210`).
  - `resolve_with(&HelperRoots, helper)` (:181) is the injectable form. `HelperRoots { flatpak_resources, exe_dir, dev_dir }`, `HelperRoots::detect()`.
  - Helpers: `SteamLaunchHelper`, `SteamLaunchTrainer`, `SteamHostTrainerRunner`.
  - Lookup applies a trust check (owner/permission) to the helper and its siblings.
  - The CLI `--scripts-dir` (`args.rs:74`) bypasses resolve with `dir.join(file_name)` + `validate_explicit_helper` (owner-uid check, `launch.rs`).
  - **Service input needed:** `HelperSource { Detect | Roots(HelperRoots) | ExplicitDir(PathBuf) }` (or `Option<PathBuf>` override) so the CLI keeps `--scripts-dir` and tests use temp roots.

## 5. Proposed `crates/crosshook-app/src/services/launch/`

```
mod.rs        LaunchService (Clone), new(), pub use; queries (validate, preview, build_steam_launch_options_command,
              list_launch_history_for_profile, list_running_profiles, reset_sessions, platform_status, is_game_running)
types.rs      LaunchSession, LaunchEvent, LaunchEvents, InjectionLog{Level,Source,Event}, LaunchKind alias
spawner.rs    ProcessSpawner trait + TokioSpawner
prepare.rs    shared pre-spawn pipeline: flags, validate, umu enrich, warnings, portal, pre-hooks (run_blocking)
game.rs       start_game: method→command, register(Game)
trainer.rs    start_trainer: execution_method, TrainerPlan, register_and_link, injection lifecycle events
gamescope.rs  LaunchEnv, gamescope_required/plan, spawn_cancel_consumer (watchdog | drain), grant wait
streaming.rs  pipe→file writers, follow() sink, LaunchLogRelayState (moved from shared.rs), TailBuffer
finalizer.rs  report build/sanitize, record_launch_finished, version snapshot, known-good, injection finalization,
              cascade + post-exit hooks (run_blocking) + deregister, portal unregister
warnings.rs   moved collectors (tauri spawn_blocking → super::run_blocking / tokio::task::spawn_blocking)
tests/        predicate.rs, parity.rs, finalizer.rs, cascade.rs, streaming.rs, + moved log_relay/coexistence/diagnostics
```

Each file stays under ~500 lines. finalizer.rs at about 300 is the largest; split `version_snapshot.rs` out if it grows.

### Types

```rust
pub struct LaunchService {
    profiles: ProfileStore, settings: SettingsStore, metadata: MetadataStore,
    registry: Arc<LaunchSessionRegistry>, background: Option<Arc<BackgroundGrantHolder>>,
    spawner: Arc<dyn ProcessSpawner>, helpers: HelperSource, env: LaunchEnvSource,
}
impl LaunchService {
    pub async fn start_game(&self, req: LaunchRequest) -> Result<LaunchSession, AppError>;
    pub async fn start_trainer(&self, req: LaunchRequest) -> Result<LaunchSession, AppError>;
    pub fn reset_sessions(&self, profile_name: &str) -> usize;     // cancel_sessions_for_profile(UserRequest)
    pub fn running_profiles(&self) -> Vec<String>;                  // active_profile_keys(Some(Game))
    pub fn validate(&self, req: &LaunchRequest) -> Result<(), LaunchValidationIssue>;
    pub async fn preview(&self, req: LaunchRequest) -> Result<LaunchPreview, AppError>;
    pub fn steam_launch_options_command(/* same 5 args as queries.rs:62 */) -> Result<String, AppError>;
    pub async fn launch_history(&self, profile: String, limit: Option<u32>) -> Result<Vec<LaunchHistoryEntry>, AppError>;
}

pub struct LaunchSession {
    pub id: SessionId, pub kind: SessionKind,
    pub warnings: Vec<LaunchValidationIssue>,
    pub log_path: PathBuf,              // needed for LaunchResult.helper_log_path + diagnostic matching
    pub events: LaunchEvents,           // mpsc receiver; recv() -> Option<LaunchEvent>
}

#[derive(Debug, Clone)]
pub enum LaunchEvent {
    Log { session_id: SessionId, batch: LogBatch },            // UI-transformed lines
    Injection { session_id: SessionId, event: InjectionLogEvent },
    Diagnostic { session_id: SessionId, report: DiagnosticReport }, // only when should_surface_report
    Exited { session_id: SessionId, code: Option<i32>, signal: Option<i32> }, // always last
}

pub trait ProcessSpawner: Send + Sync + 'static {
    fn spawn(&self, kind: SessionKind, command: &mut tokio::process::Command) -> io::Result<tokio::process::Child>;
}
pub struct TokioSpawner; // command.spawn()
```

Notes on these types:

- Every event carries `session_id` (AC6). The issue writes `Exited { code, signal }`; adding `session_id` makes the AC hold uniformly.
- Channel: a bounded `mpsc` (e.g. 256). The `follow` sink uses `try_send` and folds dropped batches into `LogBatch::Dropped{n}`. `Exited` is sent with `send().await`, so it is never lost.
- Pre-spawn injection events are buffered in the channel and delivered after `start_trainer` returns. Order is preserved; React doesn't depend on timing.
- `ProcessSpawner` test fakes swap the program (e.g. `Command::new("sh").args(["-c", script])`, writing to the log or exiting with a code). The finalizer and cascade then run without Proton. A fake can also refuse to spawn, which covers the deregister-on-error path.
- Watchdog selection stays real. Tests use gamescope-disabled requests, so the drain path is exercised. `LaunchEnv` is injected (`LaunchEnvSource::Fixed(env)`) so the predicate doesn't read `FLATPAK_ID` / `GAMESCOPE_WAYLAND_DISPLAY`.
- `[INFERENCE]` Core builders still call `platform::is_flatpak()` internally (`steam_helpers.rs`, `proton_command.rs:252`). Flatpak-branch parity tests must therefore use the core builder fns directly or the existing env-guard pattern (`platform/tests/common.rs:8` uses a test-only key, which won't affect `is_flatpak()`). The parity test should compare `build_flatpak_steam_trainer_command(steam_req)` with `build_proton_trainer_command(equivalent proton_run req)`, extending `script_runner/tests/proton_trainer_staging.rs:10`. Also compare `TrainerPlan` (execution method, `resolved_trainer_gamescope()`, working directory via `command.as_std().get_current_dir()`, env via `support.rs:47 command_env_value`).
- **Decision needed:** on the host (non-Flatpak), Steam trainers go through `steam-launch-trainer.sh` (`build_trainer_command`), not the proton_run builder. AC2 equality only holds literally for the Flatpak branch.

Errors: use `AppError`. The Tauri adapter maps `error.to_string()` (ProfileWriter decision D11). Keep today's message texts ("failed to launch helper: …", "native launch does not support trainer launch.") as the `AppError` messages. AppError redacts home paths (`AppError` sanitization), which may slightly change surfaced texts. That is acceptable but should be called out.

### `Services` wiring

`bootstrap/stores.rs:46` `Services` gains `pub launch: LaunchService`. `Services::new(stores, events)` (:64) builds it from `stores.profiles/settings/metadata`, a new `Arc<LaunchSessionRegistry>`, the **same** `background` Arc, `Arc::new(TokioSpawner)`, `HelperSource::Detect` and `LaunchEnvSource::Detect`. Construct `background` first and clone it into both fields.

Tauri `lib.rs`: replace `.manage(Arc::new(LaunchSessionRegistry::new()))` (:58-60) with `.manage(services.launch.clone())`. All eight query commands plus `launch_reset_sessions` take `State<'_, LaunchService>`. `check_gamescope_session`, `launch_platform_status` and `check_game_running` may stay as thin core calls.

`crosshook-app/Cargo.toml` already has tokio `process`, `tokio-util`, `uuid`, `serde_json`. It lacks `chrono` (used by `emit_injection_log_event` for the rfc3339 timestamp). Add `chrono = "0.4"`, which core already uses (`crosshook-core/Cargo.toml:16`). `crosshook-cli/Cargo.toml` has **no** `crosshook-app` dependency, so add it.

### Tauri adapter (`src-tauri/src/commands/launch/`)

`launch_game(app, request, service: State<LaunchService>)` → `service.start_game(request)` → `relay::spawn(app, session.events)` → `Ok(LaunchResult{ succeeded: true, message, helper_log_path: session.log_path, warnings })`.

The relay maps:

- `Log` → one `launch-log` emit **per line** (keeps ConsoleView's one-entry-per-event behaviour);
- `Dropped{n}` → an optional notice line;
- `Injection` → `injection-log`;
- `Diagnostic` → `launch-diagnostic`;
- `Exited` → `launch-complete`.

The remaining shell files are about 150 lines: `mod.rs`, `execution.rs` (3 commands), `queries.rs`, `relay.rs`, `shared.rs` (`LaunchResult` only). The `portal`, `warnings`, `diagnostics` and `streaming` files and the `tests/` directory move into the service.

## 6. Frontend listeners and payload shapes (`src/`)

| Event               | Rust payload today                                                                                                                                                                                                                                    | Listeners                                                                                                                                                                                                                                                                         | Shape-compatible `session_id` addition                                                                                                                                                                                                                                                                 |
| ------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| `launch-log`        | plain `String`, one line per emit (`streaming.rs:93,126,134,183,204`)                                                                                                                                                                                 | `components/ConsoleView.tsx:65` (`normalizeLogMessage(payload).trimEnd()`, one row per event); `components/layout/ConsoleDrawer.tsx:148` (`countLogLines`, :9); mocks `lib/mocks/handlers/launch.ts:302`; tests `AppShell.test.tsx:256`, `ConsoleDrawer.test.tsx:55` emit strings | `{ line, session_id }`. `normalizeLogMessage` (`utils/log.ts`) already accepts `{ line: string }`, so React stays unchanged; widen `LogPayload` to `{ line: string; session_id?: string }`. Emitting an object for every line is the single shape. `update-log` shares the handler and stays a string. |
| `launch-diagnostic` | `DiagnosticReport` (core `launch/diagnostics/models.rs`)                                                                                                                                                                                              | `hooks/useLaunchState.ts:266`, validated by `isDiagnosticReport` (`types/diagnostics.ts:127`, ignores extra keys); accepted only if `log_tail_path === activeHelperLogPathRef.current`                                                                                            | `#[serde(flatten)]` the report in a wrapper with `session_id`; add optional `session_id?: string` to the TS `DiagnosticReport`                                                                                                                                                                         |
| `launch-complete`   | `json!({code, signal})` (`streaming.rs:433`), for game **and** trainer                                                                                                                                                                                | `useLaunchState.ts:282` (reducer `case 'launch-complete': return state;`, :82, a no-op); `hooks/useProfileHealth.ts:292` (`batchValidate`); `hooks/useRunningProfiles.ts:47` (refresh); mocks emit `{code:0, signal:null}` (`launch.ts:320`)                                      | `{code, signal, session_id}`; no listener reads fields                                                                                                                                                                                                                                                 |
| `injection-log`     | `InjectionLogEvent` (`shared.rs:111`): `timestamp, profile_name, session_id, session_kind: game\|trainer, level: info\|warning\|error, source: trainer\|injection\|runtime, message, hook_id?, hook_name?, unsupported_runtime?` (skipped when false) | `components/library/trainer/InjectionLogTail.tsx:74` (`isInjectionLogEvent`, `types/injection.ts:27`; scoped by profile, session, kind); used by `HeroDetailTrainerTab.tsx:138`                                                                                                   | already carries `session_id`; unchanged                                                                                                                                                                                                                                                                |

Today `LaunchResult` (`types/launch.ts:144`) has no session id. Adding `session_id` there is the natural correlation key `[INFERENCE]`, optional and additive.

## 7. CLI (`crates/crosshook-cli/src/launch.rs`, 354 lines)

Today the CLI does the following:

- `from_profile` → umu enrich → `validate`.
- `--dry-run`: `build_launch_preview`, printed as JSON or text. Keep this byte-identical; it doesn't touch the service launch path.
- Launch:
  - log path `$XDG_RUNTIME_DIR|/tmp/crosshook-logs/<safe>.log` (`launch_log_path`), which differs from `create_log_path`;
  - spawns with **stdout/stderr nulled** for all methods;
  - `stream_helper_log` (:278: `try_wait` + `drain_log` byte copy every 500 ms);
  - `safe_read_tail` (:321);
  - `analyze(status, tail, request.method)` → prints `summary` + `[sev] summary: suggestion`, then non-zero → `LaunchFailure`.
- No warnings, hooks, sessions, history, watchdog or portal.

Moving the CLI to `LaunchService::start_game` changes behaviour in these ways (each is a plan decision; the AC says output is "unchanged for fixture profiles"):

1. proton_run stdout is currently `Stdio::null()`, and `build_proton_game_command` attaches no log stdio (only `native.rs:28` calls `attach_log_stdio`). So the CLI prints **nothing** for proton_run today. The service pipes output, so the CLI would start printing Proton output.
2. The log path moves to `/tmp/crosshook-logs/game-…`.
3. Launch history, version snapshot and known-good rows would now be written from the CLI.
4. Pre-launch warnings and hooks would run, and warnings could be printed to stderr.
5. Diagnostics: the service uses `diagnostic_method_for_log` plus sanitization, and only surfaces `should_surface_report`. The CLI printing matches today because it also gates on `should_surface_report`.
6. Bytes vs lines: the CLI would print `line + "\n"` per event rather than raw bytes. That is identical except for a missing trailing newline at EOF.

The CLI builds a standalone `LaunchService` (stores from `crate::store::profile_store`, `SettingsStore::try_new`, `MetadataStore::try_new().unwrap_or(disabled)`, `background: None`, `HelperSource::ExplicitDir` when `--scripts-dir` is given, after the existing `validate_explicit_helper`). It drains `session.events`: `Log` → stdout, `Diagnostic` → stderr in the current format, `Exited` → exit status. Delete `stream_helper_log`, `drain_log` and `safe_read_tail` from both the CLI and `src-tauri/.../diagnostics.rs:8`.

## 8. Risks and decisions for the plan

1. **Blocking hooks on the async executor.** `run_pre_launch_hooks` (execution.rs:144/≈331) and `run_post_exit_hooks` (streaming.rs:529) use a sync `wait_with_timeout`. Wrap them in `run_blocking`. Ordering is preserved.
2. **Non-gamescope trainer cascade** only records a reason (no kill). Keep that, or add a kill and treat it as a behaviour change.
3. **Watchdog with an empty exe basename** drops `cancel_rx`. Have the service fall back to drain so the cascade attribution is still recorded.
4. **Steam trainer parity on the host** goes through the shell script. Narrow AC2 to Flatpak, or add a script-env parity assertion.
5. **CLI output deltas** (§7.1-7.6) need an explicit decision. To keep CLI proton_run output identical, add a `quiet_pipes`/`echo_to_stdout` option.
6. `launch-log` payload shape: string → `{line, session_id}`. React is compatible via `normalizeLogMessage`; update mocks and tests to the object form.
7. Registry ownership moves from Tauri `.manage` to `Services`. `list_running_profiles` and `launch_reset_sessions` must use the same instance.
8. `ShutdownTarget` stays `pub(super)` in core. The per-session watchdog keeps tree isolation, and the service never touches PIDs of other sessions.

## 9. Verification hooks for the main agent

- `cargo test -p crosshook-app` (new `services/launch/tests/*`), `-p crosshook-core` (parity extension), `-p crosshook-cli`.
- `./scripts/check-host-gateway.sh`: the service must not introduce `Command::new("<denylisted>")`. Test fakes use `sh`, which is not on the denylist (`scripts/check-host-gateway.sh:7-10`).
- `npm test` (Console/Drawer/useLaunchState/InjectionLogTail tests) after the `launch-log` shape change, plus smoke.
