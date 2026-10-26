# Plan: LaunchService — launch pipeline in `crosshook-app` with session-scoped event streams

Tracking: Linear YAN-798 / GitHub yandy-r/crosshook#534 (M1-05). Target release `crosshook v0.7.0-alpha.1`, trunk-only, base `main`, branch `refactor/yan-798-launch-service`. Paths are relative to the repo root. Rust and frontend code lives under `src/crosshook-native/`. In task bodies, `app/` = `src/crosshook-native/crates/crosshook-app`, `core/` = `src/crosshook-native/crates/crosshook-core`, `cli/` = `src/crosshook-native/crates/crosshook-cli`, `tauri/` = `src/crosshook-native/src-tauri`, `ui/` = `src/crosshook-native/src`, `svc/` = `app/src/services/launch`.

## Contents

- Summary · User Story · Problem → Solution · Metadata · Batches · Worktree Setup · UX Design · Mandatory Reading · External Documentation · Patterns to Mirror · B0 Contracts · Files to Change · NOT Building · Step-by-Step Tasks · Testing Strategy · Validation Commands · Acceptance Criteria · Completion Checklist · Risks · Notes

## Summary

Move the launch pipeline (about 1,880 lines in `tauri/src/commands/launch/`) into `app/src/services/launch/` as one `LaunchService`. `start_game` and `start_trainer` return a `LaunchSession { id, kind, warnings, log_path, events }`. `events` is a per-session bounded `broadcast<LaunchEvent>` (`Log`, `Injection`, `Diagnostic`, `Exited`), and every event carries `session_id`. One pure predicate, `gamescope_required`, replaces the four Tauri copies. A `ProcessSpawner` seam (one method, takes the already-built core `Command`) lets the finalizer and cascade be tested without Proton. Tauri keeps thin `#[tauri::command]` delegates plus a relay that maps events to the four existing event names, adding `session_id` to each payload. `crosshook-cli launch` drops `stream_helper_log`, `drain_log` and `safe_read_tail`. SQLite, TOML and `DiagnosticReport` formats do not change. Spec: [feature-spec.md](./feature-spec.md). Binding decisions: [decisions.md](./decisions.md) D1–D12. Acceptance criteria: [issue.md](./issue.md).

## User Story

As a CrossHook maintainer, I want the launch pipeline in `crosshook-app` behind one testable service with session-scoped events, so the Tauri shell, the CLI and the future native UI share one launch path, and a game's and its trainer's output can be told apart.

## Problem → Solution

Today `launch_game` and `launch_trainer` duplicate about 30 lines of preamble. The gamescope gate exists in four copies. The finalizer, the cascade and the 500 ms whole-file re-read can only run under a real `AppHandle`. A panic in the stream task skips teardown. The pipe reader stops on the first invalid UTF-8 byte. The `launch-log` payload has no session id, and the CLI duplicates streaming and tail reading. → One service whose shared preamble and finalizer are written once. A pure injected-env predicate. Per-session bounded streams. A drop/panic guard that always runs teardown. A lossy, capped pipe reader. A Tauri relay as the only place that knows event names.

## Metadata

- **Complexity**: XL
- **Source PRD**: `docs/plans/launch-service/issue.md` (+ binding `decisions.md` D1–D12, spec `feature-spec.md`)
- **PRD Phase**: M1-05 (v0.7.0-alpha.1)
- **Estimated Files**: ~75 source and doc files (≈45 create, ≈22 modify, 7 delete) plus 58 frozen fixture files (13 CLI cases × 4 files, 6 event JSON)

## Batches

Tasks in the same batch run concurrently and touch **disjoint files**. Batches run in order. Subagents skip builds mid-flight, so the main agent compiles and verifies once after each batch (B0 tasks run their own capture commands). Internal batches may be temporarily incomplete; the final branch is fully cut over in one PR, with no shims.

| Batch | Tasks              | Depends On | Parallel Width | What                                                                             |
| ----- | ------------------ | ---------- | -------------- | -------------------------------------------------------------------------------- |
| B0    | 0.1, 0.2           | —          | 2              | **Phase 0**: freeze CLI output and Tauri payloads from the **unmodified** code   |
| B1    | 1.1                | B0         | 1              | Frozen contracts: types, spawner, module tree, harness, `Services` wiring        |
| B2    | 2.1 – 2.6          | B1         | 6              | Leaves: predicate, warnings/diagnostics, streaming, snapshot, finalizer, queries |
| B3    | 3.1, 3.2, 3.3      | B2         | 3              | Orchestration (`prepare`/`run`), game/trainer builders + parity, CLI streaming   |
| B4    | 4.1, 4.2, 4.3, 4.4 | B3         | 4              | Cascade tests, Tauri relay, Tauri command cutover, TS types + mocks              |
| B5    | 5.1                | B4         | 1              | Docs, dead-code sweep                                                            |

- **Total tasks**: 17
- **Total batches**: 6
- **Max parallel width**: 6

Ownership rule: no two tasks in a batch touch the same file (audited in Files to Change). The cross-slice Rust signatures live in **B0 Contracts** below. B2–B4 slices code against them, and B1 turns them into real types plus an empty-but-declared module tree, so no later task edits `mod.rs`.

## Worktree Setup

- **Parent**: /home/yandy/Projects/github.com/yandy-r/crosshook-yan-798/ (branch: refactor/yan-798-launch-service)

## UX Design

Internal change — no user-facing UX transformation. The only visible difference is that `launch-log` rows are now `{line, session_id}` objects. The console renders them identically, because `normalizeLogMessage` already accepts `{line}` (`ui/utils/log.ts`).

| Touchpoint             | Before                          | After                                                                          |
| ---------------------- | ------------------------------- | ------------------------------------------------------------------------------ |
| Console / drawer badge | string per line                 | `{line, session_id}` per line, same rows, timestamps and counts                |
| Lagging UI             | silent loss or unbounded memory | one visible line `[crosshook] Console skipped N log batches; …`, `Exited` kept |
| Diagnostic / complete  | no session id                   | extra `session_id` key; React ignores unknown keys                             |

## Mandatory Reading

| Priority       | File                                                                                  | Lines                     | Why                                                |
| -------------- | ------------------------------------------------------------------------------------- | ------------------------- | -------------------------------------------------- |
| P0 (critical)  | `docs/plans/launch-service/issue.md`                                                  | all                       | Acceptance criteria                                |
| P0 (critical)  | `docs/plans/launch-service/decisions.md`                                              | all                       | Binding D1–D12                                     |
| P0 (critical)  | `docs/plans/launch-service/feature-spec.md`                                           | all                       | API, finalizer order, wire payloads, CLI scope     |
| P0 (critical)  | `docs/plans/launch-service/research-business.md`                                      | §1, §2                    | Invariants I-1..I-63 and the gamescope truth table |
| P0 (critical)  | `src/crosshook-native/src-tauri/src/commands/launch/execution.rs`                     | 43-641                    | Code being moved (game, trainer, cancel consumer)  |
| P0 (critical)  | `src/crosshook-native/src-tauri/src/commands/launch/streaming.rs`                     | 20-542                    | Streaming and finalizer being moved                |
| P0 (critical)  | `src/crosshook-native/src-tauri/src/commands/launch/shared.rs`                        | 12-208                    | Relay state, injection types, `LaunchResult`       |
| P0 (critical)  | `src/crosshook-native/crates/crosshook-app/src/log_tail.rs`                           | 15-345                    | `LogBatch`, `TailConfig`, `follow`                 |
| P0 (critical)  | `src/crosshook-native/crates/crosshook-app/src/jobs/registry.rs`                      | 134-200, 322-380, 504-532 | subscribe-before-start, `FinishGuard`, lag mapping |
| P0 (critical)  | `src/crosshook-native/crates/crosshook-core/src/launch/session/registry.rs`           | 27-240                    | Registry API the service calls                     |
| P1 (important) | `src/crosshook-native/crates/crosshook-cli/src/launch.rs`                             | all                       | CLI flow and the three helpers to delete           |
| P1 (important) | `src/crosshook-native/crates/crosshook-app/src/services/profiles/tests/helpers.rs`    | 35-56                     | Harness to copy                                    |
| P1 (important) | `src/crosshook-native/crates/crosshook-app/src/bootstrap/stores.rs`                   | 42-90                     | `Services` bundle                                  |
| P1 (important) | `src/crosshook-native/crates/crosshook-core/src/launch/request/tests/from_profile.rs` | 189-210, 260-310          | Existing Flatpak parity test and `shape` helper    |
| P2 (reference) | `docs/plans/launch-service/research-security.md`                                      | §1, §3                    | C-1/C-2, H-1..H-6, security invariants             |
| P2 (reference) | `docs/plans/launch-service/research-cli.md`                                           | golden design             | CLI golden harness and the case table              |
| P2 (reference) | `docs/plans/launch-service/research-ux.md`                                            | §2, §4                    | Event table, TS and mock changes                   |
| P2 (reference) | `docs/plans/profile-writer/profile-writer.plan.md`                                    | all                       | Format exemplar                                    |

## External Documentation

| Topic                         | Source                                                                 | Key Takeaway                                                                     |
| ----------------------------- | ---------------------------------------------------------------------- | -------------------------------------------------------------------------------- |
| tokio `sync::broadcast`       | https://docs.rs/tokio/1.51.0/tokio/sync/broadcast/index.html           | `send` never blocks; a slow receiver gets `Lagged(n)` and then the newest events |
| serde `flatten`               | https://serde.rs/attr-flatten.html                                     | Flatten a borrowed struct plus `session_id` for the diagnostic wrapper           |
| Tauri v2 events               | https://v2.tauri.app/develop/calling-frontend/                         | Payloads must be `Serialize + Clone`; JS gets parsed JSON                        |
| `std::process::ExitStatusExt` | https://doc.rust-lang.org/std/os/unix/process/trait.ExitStatusExt.html | `from_raw` fixtures in finalizer tests; `signal()` / `core_dumped()`             |

## Patterns to Mirror

### SERVICE_PATTERN

```rust
// SOURCE: src/crosshook-native/crates/crosshook-app/src/services/health/mod.rs:37-41
    pub async fn batch_summary(&self) -> Result<EnrichedHealthSummary, AppError> {
        let profiles = self.profiles.clone();
        let metadata = self.metadata.clone();
        run_blocking(move || Ok(build_enriched_health_summary(&profiles, &metadata))).await
    }
```

### FINISH_GUARD (drop-safe terminal event)

```rust
// SOURCE: src/crosshook-native/crates/crosshook-app/src/jobs/registry.rs:361-373
impl Drop for FinishGuard {
    fn drop(&mut self) {
        if self.publisher.is_none() { return; }
        let outcome = if std::thread::panicking() {
            // Never surface the panic payload; it may embed user paths.
```

### EVENT_RECV (lag mapping, terminal event never lost)

```rust
// SOURCE: src/crosshook-native/crates/crosshook-app/src/jobs/registry.rs:513-518
            Source::Live(rx) => match rx.recv().await {
                Ok(event) => Some(event),
                Err(RecvError::Lagged(n)) => Some(JobEvent {
                    job_id: self.id.clone(),
                    kind: JobEventKind::Log(LogBatch::Dropped { n }),
```

### INJECT_ENV (pure fn plus a public wrapper that reads the env)

```rust
// SOURCE: src/crosshook-native/crates/crosshook-core/src/launch/optimizations/gamemode.rs:34-36
pub fn should_register_gamemode_portal(request: &LaunchRequest, effective_method: &str) -> bool {
    should_register_gamemode_portal_with(request, platform::is_flatpak(), effective_method)
}
```

### THIN_COMMAND

```rust
// SOURCE: src/crosshook-native/src-tauri/src/commands/health/batch.rs:6-11
pub async fn batch_validate_profiles(service: State<'_, HealthService>) -> Result<EnrichedHealthSummary, String> {
    service.batch_summary().await.map_err(|error| error.to_string())
}
```

### IPC_NAME_TEST

```rust
// SOURCE: src/crosshook-native/src-tauri/src/commands/settings.rs:182-187
    fn command_names_match_expected_ipc_contract() {
        // Async commands cannot be cast to fn pointers; naming them pins the
        // exported symbol (and thus the IPC command name).
        let _ = settings_load;
```

### TEST_HARNESS

```rust
// SOURCE: src/crosshook-native/crates/crosshook-app/src/services/profiles/tests/helpers.rs:37-42
        let root = tempfile::tempdir().unwrap();
        let profiles = ProfileStore::with_base_path(root.path().join("profiles"));
        let settings = SettingsStore::with_base_path(root.path().join("config"));
        let metadata = match metadata { Metadata::Enabled => MetadataStore::open_in_memory().unwrap(),
            Metadata::Disabled => MetadataStore::disabled() };
```

### SH_CHILD (real throwaway process as the fake)

```rust
// SOURCE: src/crosshook-native/crates/crosshook-app/src/jobs/process/tests.rs:20-27
async fn spawn_ready(script: &str) -> Child {
    let mut child = Command::new("sh")
        .arg("-c")
        .arg(script)
        .stdout(Stdio::piped())
```

### COMMAND_SHAPE (parity comparison)

```rust
// SOURCE: src/crosshook-native/crates/crosshook-core/src/launch/request/tests/from_profile.rs:192-199
fn shape(command: &Command) -> CommandShape {
    let std = command.as_std();
    let mut env: BTreeMap<OsString, Option<OsString>> = std
        .get_envs()
        .map(|(key, value)| (key.to_owned(), value.map(ToOwned::to_owned)))
        .collect();
```

### GOLDEN_FIXTURE

```rust
// SOURCE: src/crosshook-native/crates/crosshook-core/tests/launch_request_golden.rs:39-40
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/launch_request");
    let mut paths: Vec<PathBuf> = fs::read_dir(&dir)
```

### PIPE_READER (the defect to replace; stops on the first invalid UTF-8 byte)

```rust
// SOURCE: src/crosshook-native/src-tauri/src/commands/launch/streaming.rs:211-218
async fn pipe_reader_task<R>(reader: R, tx: mpsc::UnboundedSender<String>)
where R: AsyncRead + Unpin + Send + 'static,
{
    let mut lines = BufReader::new(reader).lines();
    loop { match lines.next_line().await {
```

### WIRE_EMIT (today's finalizer emits)

```rust
// SOURCE: src/crosshook-native/src-tauri/src/commands/launch/streaming.rs:427-440
    if should_surface_report(&report) { app.emit("launch-diagnostic", &report) … }
    app.emit("launch-complete", serde_json::json!({ "code": exit_code, "signal": signal }))
```

### VITEST_HANDLER_CAPTURE

```ts
// SOURCE: src/crosshook-native/src/hooks/__tests__/useLaunchState.test.ts:21-23
vi.mock('@/lib/events', () => ({
  subscribeEvent: vi.fn((): Promise<() => void> => Promise.resolve(() => {})),
}));
```

## B0 Contracts

Frozen by Task 1.1 (real types, plus an empty module tree). B2–B4 tasks code against these exact signatures and **must not edit** `svc/mod.rs`, `svc/types.rs`, `svc/spawner.rs` or `svc/tests/{mod,helpers}.rs` after B1. If a contract is wrong, message the main agent. Items marked `pub` are crate API; `pub(super)` items are module-internal.

```rust
// svc/types.rs ----------------------------------------------------------------------------------
pub const LAUNCH_EVENT_CAPACITY: usize = 256;
#[derive(Debug, Clone, PartialEq)]
pub enum LaunchEvent {
    Log { session_id: SessionId, batch: LogBatch },                 // raw lines (incl. empty)
    Injection { session_id: SessionId, event: InjectionLogData },   // trainer sessions only
    Diagnostic { session_id: SessionId, report: Box<DiagnosticReport> },
    Exited { session_id: SessionId, code: Option<i32>, signal: Option<i32> },
}
impl LaunchEvent { pub fn session_id(&self) -> SessionId; }
#[derive(Debug, Clone, Copy, PartialEq, Eq)] pub enum InjectionLevel { Info, Warning, Error }
#[derive(Debug, Clone, Copy, PartialEq, Eq)] pub enum InjectionSource { Trainer, Injection, Runtime }
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InjectionLogData { pub at: SystemTime, pub profile_name: String, pub session_kind: SessionKind,
    pub level: InjectionLevel, pub source: InjectionSource, pub message: String, pub unsupported_runtime: bool }
impl InjectionLogData { pub fn new(profile_name: Option<&str>, kind: SessionKind, level: InjectionLevel,
    source: InjectionSource, message: impl Into<String>, unsupported_runtime: bool) -> Self; } // trim; blank/None -> "Unknown profile"; at = now
pub struct LaunchSession { pub id: SessionId, pub kind: SessionKind, pub warnings: Vec<LaunchValidationIssue>,
    pub log_path: PathBuf, pub events: LaunchEvents }
pub struct LaunchEvents { /* id, broadcast::Receiver<LaunchEvent>, done */ }
impl LaunchEvents {
    pub fn session_id(&self) -> SessionId;
    pub async fn recv(&mut self) -> Option<LaunchEvent>;      // Lagged(n) -> Log{Dropped{n}}; None after Exited or Closed
    pub fn drain_ready(&mut self) -> Vec<LaunchEvent>;        // non-blocking; pre-spawn events for LaunchStartError
}
#[derive(Debug)] pub struct LaunchStartError { pub error: AppError, pub events: Vec<LaunchEvent> }
// impl From<AppError>, Display (= error), std::error::Error
#[derive(Clone)] pub(super) struct EventPublisher { /* id, broadcast::Sender<LaunchEvent> */ }
impl EventPublisher {
    pub(super) fn channel(id: SessionId) -> (Self, LaunchEvents);     // receiver exists before spawn (subscribe-before-start)
    pub(super) fn session_id(&self) -> SessionId;
    pub(super) fn publish(&self, event: LaunchEvent);                  // ignores "no receivers"; never blocks
    pub(super) fn log(&self, batch: LogBatch); pub(super) fn injection(&self, data: InjectionLogData);
    pub(super) fn diagnostic(&self, report: DiagnosticReport);
    pub(super) fn exited(&self, code: Option<i32>, signal: Option<i32>);
}
pub(super) struct SessionContext {
    pub session_id: SessionId, pub kind: SessionKind, pub method: &'static str /* execution method */,
    pub profile_name: Option<String>, pub steam_app_id: String, pub steam_client_path: String,
    pub trainer_host_path: Option<String>, pub log_path: PathBuf, pub operation_id: Option<String>,
    pub watchdog_outcome: WatchdogOutcome, pub coexistence: Vec<CoexistenceAdvisoryRecord>,
    pub post_exit_hooks: Vec<LaunchHook>, pub hook_context: LaunchHookExecutionContext,
    pub portal_guard: Option<GameModeRegistration>, pub metadata: MetadataStore,
    pub registry: Arc<LaunchSessionRegistry>,
}

// svc/spawner.rs --------------------------------------------------------------------------------
pub trait ProcessSpawner: Send + Sync + 'static {
    fn spawn(&self, command: &mut tokio::process::Command) -> std::io::Result<tokio::process::Child>;
}
pub struct TokioSpawner;   // command.spawn()

// svc/mod.rs ------------------------------------------------------------------------------------
pub enum HelperSource { Detect, Roots(HelperRoots) }          // Clone
#[derive(Clone)] pub struct LaunchService { /* private fields: profiles, settings, metadata,
    registry: Arc<LaunchSessionRegistry>, background: Option<Arc<BackgroundGrantHolder>>,
    spawner: Arc<dyn ProcessSpawner>, helpers: HelperSource, env: Option<LaunchEnv> */ }
impl LaunchService {
    pub fn new(profiles: ProfileStore, settings: SettingsStore, metadata: MetadataStore,
               background: Option<Arc<BackgroundGrantHolder>>) -> Self;
    pub fn with_spawner(self, s: Arc<dyn ProcessSpawner>) -> Self;
    pub fn with_env(self, env: LaunchEnv) -> Self;
    pub fn with_helper_source(self, h: HelperSource) -> Self;
    pub(super) fn launch_env(&self) -> LaunchEnv;                                                  // fixed or LaunchEnv::detect()
    pub(super) fn resolve_helper(&self, helper: RuntimeHelper) -> Result<PathBuf, AppError>;       // Unavailable on failure
}
// The public async/sync methods are defined in the files named in each task:
//   start_game (game.rs), start_trainer (trainer.rs), validate/preview/steam_launch_options_command/
//   launch_history/reset_sessions/running_profiles (queries.rs). Signatures: feature-spec.md § API.

// LaunchEnv is defined in svc/types.rs (Task 1.1) and re-exported; svc/gamescope.rs (Task 2.1) uses it ---
// #[derive(Debug, Clone, Copy, PartialEq, Eq)] pub struct LaunchEnv { pub is_flatpak: bool, pub inside_gamescope_session: bool }
// impl LaunchEnv { pub fn detect() -> Self; }
pub fn execution_method(req: &LaunchRequest, kind: SessionKind, env: LaunchEnv) -> &'static str;
pub fn gamescope_required(req: &LaunchRequest, kind: SessionKind, env: LaunchEnv) -> bool;
pub fn pid_capture_required(req: &LaunchRequest, kind: SessionKind, env: LaunchEnv) -> bool;
pub(super) struct GamescopePlan { pub required: bool, pub pid_capture: Option<PathBuf> }
pub(super) fn gamescope_plan(req: &LaunchRequest, kind: SessionKind, env: LaunchEnv, log_path: &Path) -> GamescopePlan;
pub(super) enum CancelConsumer { Watchdog { pid: u32, exe_name: String, capture: Option<PathBuf> }, Drain }
pub(super) fn choose_cancel_consumer(plan: &GamescopePlan, req: &LaunchRequest, kind: SessionKind, child_pid: Option<u32>) -> CancelConsumer;
pub(super) fn spawn_cancel_consumer(consumer: CancelConsumer, session_id: SessionId, outcome: WatchdogOutcome,
    cancel_rx: broadcast::Receiver<TeardownReason>, background: Option<Arc<BackgroundGrantHolder>>);

// svc/warnings.rs  (Task 2.2) -------------------------------------------------------------------
pub(super) struct LaunchWarnings { pub issues: Vec<LaunchValidationIssue>, pub coexistence: Vec<CoexistenceAdvisoryRecord> }
pub(super) async fn collect_launch_warnings(req: &LaunchRequest, profiles: &ProfileStore, metadata: &MetadataStore) -> LaunchWarnings;
    // order: low-disk, offline, trainer-hash, mod-coexistence (I-11..15); fail-open
pub(super) async fn collect_mod_coexistence_warnings(req: &LaunchRequest, profiles: &ProfileStore, metadata: &MetadataStore) -> Vec<LaunchValidationIssue>;
pub(super) fn coexistence_records_from_warnings(warnings: &[LaunchValidationIssue]) -> Vec<CoexistenceAdvisoryRecord>;

// svc/portal.rs, svc/diagnostics.rs  (Task 2.2) -------------------------------------------------
pub(super) async fn try_register_gamemode_portal_for_launch(req: &LaunchRequest, effective_method: &str) -> Option<GameModeRegistration>;
pub async fn read_log_tail(path: &Path, max_bytes: u64) -> String;               // bounded (take), lossy, errors -> ""; re-exported for the CLI
pub(super) fn sanitize_diagnostic_report(report: DiagnosticReport) -> DiagnosticReport;      // display_path_tilde (~/)
pub(super) fn diagnostic_method_for_log(method: &'static str, log_tail: &str) -> &'static str;

// svc/streaming.rs  (Task 2.3) ------------------------------------------------------------------
pub async fn stream_child_log(child: &mut Child, log_path: &Path, sink: impl FnMut(LogBatch) + Send) -> io::Result<ExitStatus>;
    // piped stdio -> lossy byte tee into the log (single writer, 64 KiB partial cap, bounded channel, never stops on bad bytes);
    // then follow(log_path) -> sink; wait; cancel the tail so its bounded drain + partial flush run; returns after both.
pub(super) async fn stream_session(child: Child, log_path: &Path, publisher: &EventPublisher) -> Option<ExitStatus>;
    // stream_child_log with sink = publisher.log; Err -> None (I-40)

// svc/snapshot.rs  (Task 2.4) -------------------------------------------------------------------
pub(super) async fn record_success_effects(ctx: &SessionContext);   // version snapshot, then known-good tag (I-48, I-49); fail-open; blocking work via run_blocking

// svc/finalizer.rs  (Task 2.5) ------------------------------------------------------------------
pub(super) struct SessionGuard { /* ctx, publisher, per-step done flags */ }
impl SessionGuard {
    pub(super) fn new(ctx: SessionContext, publisher: EventPublisher) -> Self;
    pub(super) async fn finish(self, status: Option<ExitStatus>);    // feature-spec.md finalizer order, steps 1-13
}   // Drop: if finish did not complete, runs the remaining teardown steps (Exited, cascade, hooks, deregister, portal)

// svc/prepare.rs  (Task 3.1) --------------------------------------------------------------------
pub(super) struct Prepared { pub request: LaunchRequest, pub kind: SessionKind, pub execution_method: &'static str,
    pub env: LaunchEnv, pub warnings: Vec<LaunchValidationIssue>, pub coexistence: Vec<CoexistenceAdvisoryRecord>,
    pub log_path: PathBuf, pub gamescope: GamescopePlan }
impl LaunchService { pub(super) async fn prepare(&self, request: LaunchRequest, kind: SessionKind) -> Result<Prepared, AppError>; }
    // steps 1-6: flags, validate FIRST, UMU enrich, warnings, execution method, create_log_path, GamescopePlan

// svc/run.rs  (Task 3.1) ------------------------------------------------------------------------
impl LaunchService { pub(super) async fn run_session(&self, prepared: Prepared, command: Command) -> Result<LaunchSession, LaunchStartError>; }
    // steps 8-14: portal, hook ctx + pre-hooks (run_blocking), register (kind-specific), channel, trainer pre-spawn injections,
    // spawner.spawn, history start, session task (SessionGuard + stream_session), cancel consumer. Spawn errors: kind-specific message,
    // deregister, LaunchStartError with drain_ready() events.

// tauri/src/commands/launch/relay/mod.rs  (Task 4.2) --------------------------------------------
pub(crate) fn spawn(app: tauri::AppHandle, events: LaunchEvents);                       // one task, drains in order, emits the 4 events
pub(crate) fn replay_start_error(app: &tauri::AppHandle, events: Vec<LaunchEvent>);     // pre-spawn injection rows
pub(crate) struct LaunchLogPayload<'a> { line: &'a str, session_id: &'a str }           // Serialize
// pure mapping: fn map_event(state: &mut LaunchLogRelayState, event: LaunchEvent) -> Vec<RelayEmit>  (RelayEmit::{Log,Diagnostic,Complete,Injection})
```

Error-kind policy (S10): build / spawn / hook-context / log-path failures → `AppErrorKind::Io`; helper-resolution failure → `Unavailable`; validation → `From<ValidationError>`. Message texts are kept verbatim (`failed to build Steam helper launch: …`, `failed to launch helper: …`, `failed to launch trainer (method={execution_method}): …`, `native launch does not support trainer launch.`). Never format with `{:?}`.

## Files to Change

CREATE rows are skipped by path validation (files do not exist yet); every UPDATE and DELETE row exists in the worktree today. Task 1.1 pre-creates every leaf file under `svc/` and `svc/tests/` as an empty doc-only placeholder, so the module tree is declared once. The owning task then fills its file. The Owner column names that task.

| File                                                                                 | Action | Owner    | Justification                                                                                                                              |
| ------------------------------------------------------------------------------------ | ------ | -------- | ------------------------------------------------------------------------------------------------------------------------------------------ |
| `.github/workflows/lint.yml`                                                         | UPDATE | 0.1      | add `-p crosshook-cli` to the `cargo test` step, so the golden gates merges (research-cli)                                                 |
| `src/crosshook-native/crates/crosshook-cli/Cargo.toml`                               | UPDATE | 0.1, 3.3 | `tempfile` dev-dep (0.1); `crosshook-app` path dep (3.3; different batches)                                                                |
| `src/crosshook-native/crates/crosshook-cli/tests/launch_golden.rs`                   | CREATE | 0.1      | frozen CLI dry-run and launch golden, run through `CARGO_BIN_EXE_crosshook`                                                                |
| `src/crosshook-native/crates/crosshook-cli/tests/fixtures/launch/*`                  | CREATE | 0.1      | 13 cases × (`.toml`, `.stdout`, `.stderr`, `.code`; the dry-run D4 case also keeps its masked JSON), captured from the unmodified CLI      |
| `src/crosshook-native/src-tauri/src/commands/launch/fixtures/events/*.json`          | CREATE | 0.2      | frozen current payloads: `launch_diagnostic`, `launch_complete` (code and signal variants), `launch_log`, `injection_log`, `launch_result` |
| `src/crosshook-native/crates/crosshook-app/Cargo.toml`                               | UPDATE | 1.1      | tokio features `io-util`, `fs` (pipe tee)                                                                                                  |
| `src/crosshook-native/crates/crosshook-app/src/lib.rs`                               | UPDATE | 1.1      | re-export `LaunchService` and the launch API                                                                                               |
| `src/crosshook-native/crates/crosshook-app/src/services/mod.rs`                      | UPDATE | 1.1      | `pub mod launch; pub use launch::LaunchService;`                                                                                           |
| `src/crosshook-native/crates/crosshook-app/src/bootstrap/stores.rs`                  | UPDATE | 1.1      | `Services.launch`; build `background` first and clone it into the service                                                                  |
| `src/crosshook-native/crates/crosshook-app/src/services/launch/mod.rs`               | CREATE | 1.1      | `LaunchService`, `new`, `with_*`, `HelperSource`, module tree, re-exports                                                                  |
| `src/crosshook-native/crates/crosshook-app/src/services/launch/types.rs`             | CREATE | 1.1      | events, session, errors, `EventPublisher`, `SessionContext`, `LaunchEnv`                                                                   |
| `src/crosshook-native/crates/crosshook-app/src/services/launch/spawner.rs`           | CREATE | 1.1      | `ProcessSpawner`, `TokioSpawner`                                                                                                           |
| `src/crosshook-native/crates/crosshook-app/src/services/launch/tests/mod.rs`         | CREATE | 1.1      | declares all test modules                                                                                                                  |
| `src/crosshook-native/crates/crosshook-app/src/services/launch/tests/helpers.rs`     | CREATE | 1.1      | `Harness`, `FakeSpawner`, request fixtures                                                                                                 |
| `src/crosshook-native/crates/crosshook-app/src/services/launch/gamescope.rs`         | CREATE | 2.1      | predicate, plan, cancel-consumer choice and spawn                                                                                          |
| `src/crosshook-native/crates/crosshook-app/src/services/launch/tests/gamescope.rs`   | CREATE | 2.1      | AC1 predicate table                                                                                                                        |
| `src/crosshook-native/crates/crosshook-app/src/services/launch/warnings.rs`          | CREATE | 2.2      | moved collectors                                                                                                                           |
| `src/crosshook-native/crates/crosshook-app/src/services/launch/portal.rs`            | CREATE | 2.2      | moved verbatim                                                                                                                             |
| `src/crosshook-native/crates/crosshook-app/src/services/launch/diagnostics.rs`       | CREATE | 2.2      | `read_log_tail`, sanitize, method rewrite                                                                                                  |
| `src/crosshook-native/crates/crosshook-app/src/services/launch/tests/warnings.rs`    | CREATE | 2.2      | moved coexistence test                                                                                                                     |
| `src/crosshook-native/crates/crosshook-app/src/services/launch/tests/diagnostics.rs` | CREATE | 2.2      | moved diagnostic-method tests, bounded tail                                                                                                |
| `src/crosshook-native/crates/crosshook-app/src/services/launch/streaming.rs`         | CREATE | 2.3      | pipe tee, `follow` driver                                                                                                                  |
| `src/crosshook-native/crates/crosshook-app/src/services/launch/tests/streaming.rs`   | CREATE | 2.3      | H-2 fixture, lag, ordering                                                                                                                 |
| `src/crosshook-native/crates/crosshook-app/src/services/launch/snapshot.rs`          | CREATE | 2.4      | version snapshot + known-good, moved                                                                                                       |
| `src/crosshook-native/crates/crosshook-app/src/services/launch/tests/snapshot.rs`    | CREATE | 2.4      | I-48/I-49 branches                                                                                                                         |
| `src/crosshook-native/crates/crosshook-app/src/services/launch/finalizer.rs`         | CREATE | 2.5      | `SessionGuard`, finalizer order                                                                                                            |
| `src/crosshook-native/crates/crosshook-app/src/services/launch/tests/finalizer.rs`   | CREATE | 2.5      | AC3 finalizer side effects, guard, disabled metadata                                                                                       |
| `src/crosshook-native/crates/crosshook-app/src/services/launch/queries.rs`           | CREATE | 2.6      | read-only commands, `enrich_umu_gameid`, reset/running                                                                                     |
| `src/crosshook-native/crates/crosshook-app/src/services/launch/tests/queries.rs`     | CREATE | 2.6      | history guards, key consistency                                                                                                            |
| `src/crosshook-native/crates/crosshook-app/src/services/launch/prepare.rs`           | CREATE | 3.1      | shared pre-spawn steps                                                                                                                     |
| `src/crosshook-native/crates/crosshook-app/src/services/launch/run.rs`               | CREATE | 3.1      | register, spawn, stream task, cancel consumer                                                                                              |
| `src/crosshook-native/crates/crosshook-app/src/services/launch/game.rs`              | CREATE | 3.2      | `start_game` builder dispatch                                                                                                              |
| `src/crosshook-native/crates/crosshook-app/src/services/launch/trainer.rs`           | CREATE | 3.2      | `start_trainer` builder dispatch                                                                                                           |
| `src/crosshook-native/crates/crosshook-app/src/services/launch/tests/start.rs`       | CREATE | 3.2      | validate-first, spawn error, per-session ids                                                                                               |
| `src/crosshook-native/crates/crosshook-app/src/services/launch/tests/parity.rs`      | CREATE | 3.2      | AC2 trainer parity                                                                                                                         |
| `src/crosshook-native/crates/crosshook-cli/src/launch.rs`                            | UPDATE | 3.3      | use `stream_child_log`, `read_log_tail`; delete the three helpers                                                                          |
| `src/crosshook-native/crates/crosshook-app/src/services/launch/tests/cascade.rs`     | CREATE | 4.1      | AC3 cascade, sibling survives                                                                                                              |
| `src/crosshook-native/src-tauri/src/commands/launch/relay/mod.rs`                    | CREATE | 4.2      | per-session relay, `map_event`                                                                                                             |
| `src/crosshook-native/src-tauri/src/commands/launch/relay/payloads.rs`               | CREATE | 4.2      | `LaunchLogPayload`, diagnostic and complete wrappers, `InjectionLogEvent`, `LaunchLogRelayState`                                           |
| `src/crosshook-native/src-tauri/src/commands/launch/relay/tests.rs`                  | CREATE | 4.2      | fixture comparison, suppression, `session_id` on every payload                                                                             |
| `src/crosshook-native/src-tauri/src/commands/bench.rs`                               | UPDATE | 4.2      | replay emits `{line, session_id: "bench"}`                                                                                                 |
| `src/crosshook-native/src-tauri/src/commands/launch/mod.rs`                          | UPDATE | 4.3      | module list and re-exports                                                                                                                 |
| `src/crosshook-native/src-tauri/src/commands/launch/execution.rs`                    | UPDATE | 4.3      | three thin commands                                                                                                                        |
| `src/crosshook-native/src-tauri/src/commands/launch/queries.rs`                      | UPDATE | 4.3      | delegates; three stateless commands stay                                                                                                   |
| `src/crosshook-native/src-tauri/src/commands/launch/shared.rs`                       | UPDATE | 4.3      | keep `LaunchResult` only                                                                                                                   |
| `src/crosshook-native/src-tauri/src/commands/launch/streaming.rs`                    | DELETE | 4.3      | moved to the service                                                                                                                       |
| `src/crosshook-native/src-tauri/src/commands/launch/warnings.rs`                     | DELETE | 4.3      | moved                                                                                                                                      |
| `src/crosshook-native/src-tauri/src/commands/launch/portal.rs`                       | DELETE | 4.3      | moved                                                                                                                                      |
| `src/crosshook-native/src-tauri/src/commands/launch/diagnostics.rs`                  | DELETE | 4.3      | moved (`safe_read_tail` deleted)                                                                                                           |
| `src/crosshook-native/src-tauri/src/commands/launch/tests/mod.rs`                    | UPDATE | 4.3      | now declares `contract` only                                                                                                               |
| `src/crosshook-native/src-tauri/src/commands/launch/tests/contract.rs`               | CREATE | 4.3      | IPC name contract                                                                                                                          |
| `src/crosshook-native/src-tauri/src/commands/launch/tests/coexistence.rs`            | DELETE | 4.3      | moved to `svc/tests/warnings.rs`                                                                                                           |
| `src/crosshook-native/src-tauri/src/commands/launch/tests/diagnostics.rs`            | DELETE | 4.3      | moved to `svc/tests/diagnostics.rs`                                                                                                        |
| `src/crosshook-native/src-tauri/src/commands/launch/tests/log_relay.rs`              | DELETE | 4.3      | moved to `relay/tests.rs`                                                                                                                  |
| `src/crosshook-native/src-tauri/src/lib.rs`                                          | UPDATE | 4.3      | `.manage(services.launch.clone())`; drop the separate registry                                                                             |
| `src/crosshook-native/src/utils/log.ts`                                              | UPDATE | 4.4      | `LogPayload` gains `{ line; session_id? }`                                                                                                 |
| `src/crosshook-native/src/types/diagnostics.ts`                                      | UPDATE | 4.4      | `LaunchDiagnosticEvent`                                                                                                                    |
| `src/crosshook-native/src/types/launch.ts`                                           | UPDATE | 4.4      | `LaunchCompletePayload`                                                                                                                    |
| `src/crosshook-native/src/hooks/useLaunchState.ts`                                   | UPDATE | 4.4      | event generic only                                                                                                                         |
| `src/crosshook-native/src/lib/mocks/handlers/launch.ts`                              | UPDATE | 4.4      | emit the new shapes, one shared id per session                                                                                             |
| `src/crosshook-native/src/components/layout/__tests__/ConsoleDrawer.test.tsx`        | UPDATE | 4.4      | object-payload case                                                                                                                        |
| `src/crosshook-native/src/components/__tests__/ConsoleView.test.tsx`                 | CREATE | 4.4      | object payload renders as text                                                                                                             |
| `src/crosshook-native/src/hooks/__tests__/useLaunchState.events.test.ts`             | CREATE | 4.4      | diagnostic with extra key still matches by path                                                                                            |
| `src/crosshook-native/src/lib/mocks/__tests__/launch.session.test.ts`                | CREATE | 4.4      | one session id across the mock events                                                                                                      |
| `src/crosshook-native/crates/crosshook-app/README.md`                                | UPDATE | 5.1      | `LaunchService` section                                                                                                                    |
| `docs/architecture/adr-0002-flatpak-portal-contracts.md`                             | UPDATE | 5.1      | watchdog moved from `execution.rs` to `svc/gamescope.rs`                                                                                   |

`CLAUDE.md` and `AGENTS.md` are intentionally not modified: no agent rule changes (no new label, schema bump or startup contract).

## NOT Building

- `crosshook-cli launch` calling `start_game`, writing history, snapshots or hooks, or changing `proton_run` output, log paths, the `--scripts-dir` check, or exit codes. All of that is YAN-837 (D10). The CLI bugs B1–B4 are not fixed (D12).
- The three kept divergences (D8): Flatpak trainers never write the gamescope PID capture file; the watchdog drops the cancel channel when the exe name is empty; the Flatpak Steam trainer hook environment is built as Steam. Follow-up issues are filed instead.
- `create_new` or symlink hardening for `/tmp` log and PID files (A-1); `crosshook-cli/src` in `check-host-gateway.sh` (D12).
- `JobRegistry` jobs for launches, `AppEvent` variants, process groups, `supervise_child`, `killpg`, `kill_processes_using_prefix`, a kill method on the spawner or service, or killing a non-gamescope trainer on cascade (D1, D3).
- Any SQLite schema change, TOML change, or `DiagnosticReport` change. `session_id` lives on the events and wire wrappers only.
- `EventSink`, store or filesystem trait seams; a strategy trait per launch method; an `AppError` replacement.
- `session_id` on `LaunchResult`, a `session_id`-based diagnostic filter in React, console virtualization, or console labels per session.
- Deduplicating the four Steam-manifest lookups, or sharing `should_skip_gamescope` with the app predicate.
- Moving `check_gamescope_session`, `launch_platform_status` and `check_game_running` (D9).
- New Rust or npm dependencies. `tempfile` is already in the lockfile.

## Step-by-Step Tasks

### Task 0.1: Freeze CLI output from the unmodified CLI (Phase 0) — Depends on [none]

- **BATCH**: B0
- **ACTION**: Capture `crosshook launch` and `launch --dry-run` stdout, stderr and exit code for 13 fixture cases from the **unmodified** CLI into committed frozen fixtures, and land the golden test that asserts them. Add `-p crosshook-cli` to the CI test step so it gates merges.
- **IMPLEMENT**: Create `cli/tests/launch_golden.rs` as a libtest integration test. It spawns `env!("CARGO_BIN_EXE_crosshook")` with `env_clear()` and an isolated `HOME`, `XDG_CONFIG_HOME`, `XDG_DATA_HOME`, `XDG_RUNTIME_DIR`, `PATH=@ROOT@/bin:/usr/bin:/bin`, `DISPLAY=:99`, `WAYLAND_DISPLAY=wayland-test`, and no `STEAM_COMPAT_CLIENT_INSTALL_PATH`, `GAMESCOPE_WAYLAND_DISPLAY` or `FLATPAK_ID`. It skips the suite when `/.flatpak-info` exists. It passes `--profile-dir @ROOT@/profiles`, pins `runtime.umu_preference = "proton"` in `proton_run` cases, and materializes the stub executables the profiles point at (stub proton, `game.sh`, `steam/steam.sh` plus `steamapps/compatdata/<appid>/pfx`, a stub `steam-launch-helper.sh` that parses `--log-file`). Cases (from research-cli): D1 native dry-run, D2 `proton_run` dry-run with custom env, D3 `steam_applaunch` dry-run, D4 `--json` dry-run (compare as `serde_json::Value` with `generated_at` and the `generated_at = "…"` line in `display_text` masked), E1 missing profile, E2 missing prefix, E3 no `--profile`, S1 steam helper prints `[steam-helper] Steam is not running` and exits 0, S2 helper exits 3, S3 `--scripts-dir` with a non-regular helper, N1/N2 native, P1 `proton_run`. Each case has `<case>.toml`, `.stdout`, `.stderr`, `.code`, compared after substituting `@ROOT@`. After every launch case it opens `@ROOT@/home/.local/share/crosshook/metadata.db` through `MetadataStore::with_path` and asserts `query_launch_history_for_profile` is empty (scope guard against YAN-837 creep). Capture procedure: add a temporary `CROSSHOOK_GOLDEN_CAPTURE=1` branch that writes the expected files, run it once on the unmodified tree, review each file by hand against the research-cli expectations (S1 stderr, E1 path text, P1 empty stdout), then **delete the capture branch** so the fixtures are frozen. Record the base commit SHA (`git rev-parse --short HEAD`) in the file's doc comment. Edit `.github/workflows/lint.yml` line 52 to `-p crosshook-app -p crosshook-native -p crosshook-cli`. Add `tempfile = "3"` under `[dev-dependencies]` in `cli/Cargo.toml`.
- **MIRROR**: GOLDEN_FIXTURE (frozen-fixture convention: "a mismatch is a regression, never a reason to regenerate"); `crates/crosshook-app/tests/profile_writer_golden.rs` for the doc-comment convention.
- **IMPORTS**: `std::process::Command`, `tempfile`, `crosshook_core::metadata::MetadataStore`, `serde_json`.
- **GOTCHA**: Do not bake in the known bugs: no second-run replay case, no non-UTF-8, CRLF or unterminated-line case (the PR body lists that delta). Log paths must not appear in expected output. N1/N2 stdout is an inference (research-cli step 13), so freeze whatever the baseline prints. The test never mutates its own environment.
- **VALIDATE**: `cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-cli --test launch_golden` passes twice with no diff in `git status` for `tests/fixtures/launch/`. `git diff --stat` shows no change under `cli/src/`.

### Task 0.2: Pin the current Tauri event payloads (Phase 0) — Depends on [none]

- **BATCH**: B0
- **ACTION**: Freeze today's JSON for `launch-diagnostic`, `launch-complete`, `launch-log`, `injection-log` and `LaunchResult`, generated by the unmodified Tauri code, so the relay can later prove that old fields are byte-identical.
- **IMPLEMENT**: Add a throwaway test module under `tauri/src/commands/launch/tests/` (temporary `mod pin;` line in `tests/mod.rs`). It builds: a `DiagnosticReport` from `analyze(Some(ExitStatus::from_raw(1 << 8)), "Permission denied\n", "proton_run")` with `analyzed_at` set to `2026-01-01T00:00:00+00:00`, `log_tail_path = Some("/tmp/crosshook-logs/game-x.log")`, then `sanitize_diagnostic_report`; the `launch-complete` shapes `json!({"code":0,"signal":null})` and `{"code":null,"signal":9}`; the plain-string `launch-log` payload; an `InjectionLogEvent` through `build_injection_log_event` with a fixed timestamp, profile `"  Synthetic Quest "`, a real id from `LaunchSessionRegistry::register` (replaced by the literal `@SESSION@` before writing), and one `unsupported_runtime = true` row; and a `LaunchResult` with empty warnings. It writes each via `serde_json::to_string_pretty` into `tauri/src/commands/launch/fixtures/events/*.json`. Run it, review the files, then **remove the throwaway module and the `mod pin;` line**. Only the JSON fixtures stay.
- **MIRROR**: WIRE_EMIT (the emit sites being pinned); `tests/log_relay.rs:36-90` for how the injection event is built.
- **IMPORTS**: `crosshook_core::launch::{analyze, LaunchSessionRegistry, SessionKind}`, `serde_json`, `std::os::unix::process::ExitStatusExt`.
- **GOTCHA**: These must come from the current code paths (`sanitize_diagnostic_report`, `build_injection_log_event`), not from hand-written JSON. Never regenerate them from the relay later.
- **VALIDATE**: `cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-native commands::launch` passes before and after removing the throwaway. The fixtures dir has 6 files, and `git diff` shows no change to non-test sources.

### Task 1.1: Freeze contracts — types, spawner, module tree, harness, `Services` wiring — Depends on [0.1, 0.2]

- **BATCH**: B1
- **ACTION**: Land every cross-slice contract in B0 Contracts as real code, and pre-declare the whole module tree so B2–B4 tasks never edit shared files. B1 is not independently compilable: `mod.rs` re-exports leaf items that B2 defines. The main agent's first compile gate is after B2.
- **IMPLEMENT**: Create `svc/mod.rs` (struct, `new`, `with_spawner`, `with_env`, `with_helper_source`, `launch_env`, `resolve_helper`, `pub(super) fn registry(&self)`, the `mod` list, `pub use` lines for `LaunchEnv`, `read_log_tail`, `stream_child_log`, the public types and `ProcessSpawner`, `assert_impl_all!(LaunchService: Send, Sync)`, doc comments on every `pub` item). Create `svc/types.rs` with every type in B0 Contracts: `EventPublisher::channel` builds the broadcast channel (capacity `LAUNCH_EVENT_CAPACITY`) and the first receiver; `LaunchEvents::recv` maps `Lagged(n)` to `Log{Dropped{n}}` and ends after `Exited`; `LaunchEvents::drain_ready` uses `try_recv`; `LaunchEnv` lives here (`detect` reads `platform::is_flatpak()` and `is_inside_gamescope_session()`). Create `svc/spawner.rs`. Create every other leaf (`gamescope`, `warnings`, `portal`, `diagnostics`, `streaming`, `snapshot`, `finalizer`, `queries`, `prepare`, `run`, `game`, `trainer`, and `tests/{gamescope,warnings,diagnostics,streaming,snapshot,finalizer,queries,start,parity,cascade}.rs`) as an empty file holding one `//!` line. Create `svc/tests/mod.rs` declaring all test modules and `svc/tests/helpers.rs`: `Harness { root, profiles, settings, metadata, spawner: Arc<FakeSpawner>, service }`; `Harness::new(Metadata::{Enabled, Disabled})` builds the service with `with_spawner`, `with_env(LaunchEnv{false,false})` and `HelperSource::Roots(HelperRoots { flatpak_resources: Some(dir), exe_dir: None, dev_dir: None })`, where `dir` holds stub `steam-launch-helper.sh` and `steam-launch-trainer.sh` (mode 0755; the flatpak-resources candidate is exempt from the trust check, so no uid games); `Harness::proton_request(name)`, `steam_request(name)` and `trainer_of(req)` build valid, hermetic `LaunchRequest`s (temp proton stub, prefix with `pfx`, game and trainer files, `UmuPreference::Proton`); `Harness::collect(session)` drains events until `Exited` (10 s timeout). `FakeSpawner` records a `CapturedCommand { program, args, envs, current_dir }` per call, then spawns `sh -c <script>` from a queue of `Script { body, mode: Piped | LogFile }` (`Piped` sets piped stdout and stderr; `LogFile` takes the path after `--log-file` in the captured args, as the real helper does, and appends to it), with `fail_next(io::Error)` support. Wire `Services`: add `pub launch: LaunchService`, building `background` first and passing `Some(background.clone())`. Add `pub mod launch` and the re-export in `services/mod.rs` and `lib.rs`, and the two tokio features in `Cargo.toml`.
- **MIRROR**: SERVICE_PATTERN, FINISH_GUARD and EVENT_RECV (for `EventPublisher` and `LaunchEvents`), TEST_HARNESS, SH_CHILD; `services/profiles/mod.rs` for the module-tree layout; `bootstrap/stores.rs:64-90`.
- **IMPORTS**: `crosshook_core::launch::{LaunchSessionRegistry, SessionId, SessionKind, WatchdogOutcome, LaunchHook, LaunchHookExecutionContext, CoexistenceAdvisoryRecord, DiagnosticReport}`, `crosshook_core::runtime_helpers::{HelperRoots, RuntimeHelper}`, `crosshook_core::platform::portals::gamemode::GameModeRegistration`, `crate::log_tail::LogBatch`, `static_assertions`.
- **GOTCHA**: `run_blocking` is a private fn of `services`, so descendants name it as `crate::services::run_blocking` (re-exported once in `svc/mod.rs` as `pub(super) use crate::services::run_blocking;` so leaf files write `super::run_blocking`). Do not capture an `App` or `BootstrappedApp` in the service. `EventPublisher::publish` must ignore `SendError` (no receivers). `LaunchEnv` is defined in `types.rs` (needed by `mod.rs`); `gamescope.rs` imports it, and `mod.rs` re-exports it. Keep every file under 500 lines. The fake must never call `Command::new` with a denylisted host tool (`sh` is fine, and the `tests/` path is exempt from `check-host-gateway.sh`).
- **VALIDATE**: `cargo fmt --manifest-path src/crosshook-native/Cargo.toml --all -- --check` and `./scripts/check-app-deps.sh`. No compile gate yet (see ACTION).

### Task 2.1: Gamescope predicate, plan and cancel-consumer choice — Depends on [1.1]

- **BATCH**: B2
- **ACTION**: Implement `svc/gamescope.rs` and the AC1 predicate table test.
- **IMPLEMENT**: Write `execution_method` (game → `resolved_method()` folded to the three constants; trainer → `resolve_trainer_launch_method(resolved, env.is_flatpak)`), `gamescope_required` (`exec == METHOD_PROTON_RUN && cfg.enabled && (cfg.allow_nested || !env.inside_gamescope_session)` with `cfg = req.gamescope` for a game and `req.resolved_trainer_gamescope()` for a trainer), `pid_capture_required` (`required && env.is_flatpak`), `gamescope_plan` (capture path via core `gamescope_pid_capture_path(log)`), `choose_cancel_consumer` (`Watchdog` iff `plan.required && child_pid.is_some()`; exe name is the basename of `game_path` for a game and of `trainer_host_path` for a trainer), and `spawn_cancel_consumer`. The consumer waits up to 500 ms on `BackgroundGrantHolder::wait_for_initialization` only if `background_supported()` and a holder exists (logging only), then runs core `gamescope_watchdog`. `Drain` runs `drain_cancel_into_outcome`. An empty exe basename keeps today's behaviour: `warn!` and return, dropping `cancel_rx` (D8 divergence 2, I-32). Tests (`tests/gamescope.rs`): a table over method {steam, proton_run, native} × flatpak {t,f} × config {disabled, enabled, enabled+allow_nested} × inside_session {t,f} × kind {game, trainer}, with expected `execution_method`, `gamescope_required` and `pid_capture_required` derived row by row from the research-business §2 truth tables (write the expected values out as literals, never recomputed with the same expression). It also covers the trainer config rules (explicit override enabled, game-enabled auto-windowed, neither), the `choose_cancel_consumer` choice (Drain when the predicate is false or the pid is `None`), and the exe-name rule.
- **MIRROR**: INJECT_ENV; `execution.rs:109-116,182-189,293-302,519-556,592-641` (code being unified); `core/launch/session/drain.rs:31`.
- **IMPORTS**: `crosshook_core::launch::{gamescope_pid_capture_path (via script_runner), gamescope_watchdog, drain_cancel_into_outcome, LaunchRequest, SessionKind, SessionId, TeardownReason, WatchdogOutcome, METHOD_*}`, `crosshook_core::profile::resolve_trainer_launch_method`, `crosshook_core::platform::portals::background`, `crate::services::BackgroundGrantHolder`.
- **GOTCHA**: `should_skip_gamescope` in core stays untouched (it decides builder arguments, not watchdog gating). The predicate takes no settings argument (D8). Never call `kill`, `killpg` or `kill_processes_using_prefix`. Mark the PID-capture-for-Flatpak-trainer row in a comment as the kept divergence (the builder writes no capture file).
- **VALIDATE**: `cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-app services::launch::tests::gamescope` (main agent, after B2).

### Task 2.2: Warnings, portal and diagnostics helpers — Depends on [1.1]

- **BATCH**: B2
- **ACTION**: Move the warning collectors, the GameMode portal registration and the diagnostic helpers into the service, and replace both `safe_read_tail` copies with one bounded `read_log_tail`.
- **IMPLEMENT**: Move `collect_offline_launch_warnings`, `collect_trainer_hash_launch_warnings_ipc`, `collect_mod_coexistence_warnings_ipc`, low-disk and `coexistence_records_from_warnings` from `tauri/.../warnings.rs` into `svc/warnings.rs`, replacing `tauri::async_runtime::spawn_blocking` with `tokio::task::spawn_blocking`. Add `collect_launch_warnings`, which runs them in the order low-disk, offline, trainer-hash, mod-coexistence and returns `LaunchWarnings { issues, coexistence }` (the same fail-open behaviour and DB side effects, I-11..15). Move `try_register_gamemode_portal_for_launch` verbatim to `svc/portal.rs`. Create `svc/diagnostics.rs`: `read_log_tail(path, max_bytes)` opens the file, reads size from the opened handle, seeks to `End(-max)` when larger, wraps the reader in `.take(max_bytes)`, decodes with `String::from_utf8_lossy`, and returns `""` on any error (a `tracing::warn!` without the path in the event); `sanitize_diagnostic_report` calls `presenter::display_path_tilde` over summary, description, method, path, patterns and suggestions, exactly as the old function; `diagnostic_method_for_log` moved verbatim. Move the tests: coexistence prefix-and-cap-16 → `tests/warnings.rs`; the two `diagnostic_method_*` tests → `tests/diagnostics.rs`, plus a `read_log_tail` test with a log larger than the cap (tail ≤ cap, starts after a seek, lossy on a split character) and a missing file returning `""`.
- **MIRROR**: `tauri/.../warnings.rs:14-182`, `portal.rs:12-39`, `diagnostics.rs:8-75`, `tests/coexistence.rs`, `tests/diagnostics.rs`.
- **IMPORTS**: `crosshook_core::{launch, metadata, storage, offline, profile}` (unchanged from the moved code), `crate::presenter::display_path_tilde`, `tokio::io::AsyncReadExt` or `std::io::Read` inside `run_blocking`.
- **GOTCHA**: Keep the `sanitize_diagnostic_report` output byte-for-byte (`~/`, not `$HOME`, H-5), because `log_tail_path` must still equal the raw `helper_log_path` for React's path match. `collect_launch_warnings` runs the collectors for game **and** trainer launches (I-13: offline and trainer-hash also for games). Preview uses `collect_mod_coexistence_warnings` only (I-16).
- **VALIDATE**: `cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-app services::launch::tests::warnings services::launch::tests::diagnostics` (main agent, after B2).

### Task 2.3: Streaming — lossy pipe tee and `follow` driver — Depends on [1.1]

- **BATCH**: B2
- **ACTION**: Implement `svc/streaming.rs`: one streaming path for pipe and file children built on `log_tail::follow`.
- **IMPLEMENT**: `stream_child_log(child, log_path, sink)`: if `child.stdout` or `child.stderr` is piped, take them and spawn one reader per pipe. Each reader reads bytes with `read_until(b'\n')` into a buffer capped at `PARTIAL_CAP` (64 KiB; an over-long line is flushed as a chunk), decodes lossily, and keeps reading until EOF whatever the decode result (D7). Readers send raw byte chunks over a bounded `mpsc` (capacity 256) to **one** writer task that appends to the log file in arrival order, so stdout and stderr interleave as today and a pipe is never left undrained. A write failure stops file writes but not draining. In every mode, run `follow(log_path, TailConfig::default(), tail_cancel, sink)` concurrently. After the readers hit EOF and `child.wait()` returns, flush the file, cancel the tail token and await `follow`, so its bounded final drain and partial-line flush deliver the last lines before the function returns the `ExitStatus`. `stream_session` wraps it with `publisher.log` as the sink and maps an I/O error to `None` (I-40).
- **MIRROR**: `log_tail.rs:332-357` (`follow`), PIPE_READER (defect to replace), `tauri/.../streaming.rs:142-229` (the tee), `jobs/process.rs` for `create`-then-append handling.
- **IMPORTS**: `crate::log_tail::{follow, LogBatch, TailConfig, PARTIAL_CAP}`, `tokio_util::sync::CancellationToken`, `tokio::{io::{AsyncBufReadExt, AsyncWriteExt}, sync::mpsc, fs::OpenOptions}`.
- **GOTCHA**: The file must exist before `follow` starts (create it append-mode first); `create_log_path` already creates it. The sink passed to `follow` must not block (`broadcast::send` is non-blocking). The function must not `kill` the child. The lagged `Dropped{n}` path is produced by `LaunchEvents::recv` (Task 1.1), so the streaming test asserts it through a real receiver. Tests (`tests/streaming.rs`, `#[tokio::test(flavor = "multi_thread", worker_threads = 2)]`): (1) a child printing `\xff\xfe\n`, then 1 MiB with no newline, then exiting 0 yields `Some(status.success())`, every byte is in the log, and lines are lossy and bounded (H-2); (2) stdout and stderr lines both arrive, and the last lines are delivered before return (final drain); (3) file-mode child (writes the log itself) streams without a tee; (4) a receiver that does not read while ≥ 300 batches are published sees `Log{Dropped{n}}` and still gets `Exited` last.
- **VALIDATE**: `cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-app services::launch::tests::streaming` (main agent, after B2).

### Task 2.4: Version snapshot and known-good tagging — Depends on [1.1]

- **BATCH**: B2
- **ACTION**: Move the success-path metadata side effects out of the stream finalizer into `svc/snapshot.rs`, behaviour unchanged.
- **IMPLEMENT**: Move `streaming.rs:286-424` into `record_success_effects(ctx)`: `lookup_profile_id` → Steam roots and libraries from `ctx.steam_client_path` → first existing `appmanifest_<id>.acf` through `parse_manifest_full` → `hash_trainer_file(trainer_host_path.trim())` → `lookup_latest_version_snapshot` (an error is treated as `None`) → `compute_correlation_status` → skip the snapshot on `UpdateInProgress`, otherwise `upsert_version_snapshot`; then the independent known-good block (`list_config_revisions(id, Some(1))` → `set_known_good_revision(id, latest.id)`), which still runs when the snapshot was skipped. All metadata and filesystem work runs inside one `run_blocking`/`spawn_blocking`; errors are traced and swallowed. Tests (`tests/snapshot.rs`, `MetadataStore::open_in_memory`): first success writes a snapshot with status `untracked` (even with an empty app id); a second success with a changed trainer-file hash records `trainer_changed`; `state_flags != 4` in a temp `appmanifest` skips the snapshot but still sets the known-good flag on the latest revision; no profile id skips both; `MetadataStore::disabled()` is a silent no-op.
- **MIRROR**: `tauri/.../streaming.rs:286-424` (code being moved), `core/metadata/version_store.rs:182-207`, `services/profiles/tests/helpers.rs` for the metadata setup.
- **IMPORTS**: `crosshook_core::metadata::{compute_correlation_status, hash_trainer_file, MetadataStore}`, `crosshook_core::steam::*` as used by the old code, `super::types::SessionContext`, `super::super::run_blocking`.
- **GOTCHA**: Move, do not deduplicate, the Steam manifest lookup (spec: out of scope). The success gate (`CleanExit | Indeterminate` and `profile_name.is_some()`) is evaluated by the finalizer (Task 2.5), not here. Do not log paths in warnings.
- **VALIDATE**: `cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-app services::launch::tests::snapshot` (main agent, after B2).

### Task 2.5: Finalizer and `SessionGuard` — Depends on [1.1]

- **BATCH**: B2
- **ACTION**: Implement the finalizer in the strict D5 order under a drop/panic guard, and the AC3 finalizer tests.
- **IMPLEMENT**: `SessionGuard::finish(status)` follows feature-spec § finalizer order, steps 1–13. It computes code and signal; reads `read_log_tail` (2 MiB) and runs `diagnostic_method_for_log`, `analyze`, sets `log_tail_path = display_path_tilde(log)` and sanitizes; applies the watchdog-killed rewrite ("Game|Trainer exited; gamescope compositor cleaned up.", CleanExit, Info, suggestions cleared); sets `teardown_reason = outcome.reason().or(NaturalExit)` and the coexistence advisories; runs `record_launch_finished` through `run_blocking` when `operation_id` is set; runs `record_success_effects` only when the failure mode is CleanExit or Indeterminate and a profile name exists; publishes `Diagnostic` iff `should_surface_report`; for trainers publishes the wrap-up injections (the teardown warning when the reason is not NaturalExit, then exactly one of completed, indeterminate, ended-with-diagnostics, all with today's messages, levels and sources); publishes `Exited` exactly once; for a game session calls `cancel_linked_children(LinkedSessionExit)`; runs `run_post_exit_hooks` through `run_blocking`; `deregister`; then `portal_guard.unregister().await`. Each step sets a done flag. `Drop` runs the not-yet-done teardown steps (publish `Exited{None,None}` if not done, cascade, hooks via `Handle::try_current().spawn_blocking`, deregister, portal unregister via a spawned task), using a registry call that does not panic when poisoned. Tests (`tests/finalizer.rs`, in-memory metadata, contexts built directly with `ExitStatus::from_raw`): (a) exit 0 on `proton_run` → history row `succeeded` + `exit_code 0` + version snapshot + known-good flag on the latest seeded revision (AC3); (b) exit 1 → `failed`, no snapshot, no tag; (c) signal 9 → `failed` with `signal=9`; (d) watchdog-killed (`outcome.mark(WatchdogNaturalExit)`) with exit 143 → row `succeeded`, snapshot and tag written, `Diagnostic` absent, trainer wrap-up shows the teardown warning plus the Error message (I-52 consequence); (e) steam method exit 0 → Indeterminate; (f) `MetadataStore::disabled()` → `Exited` still published exactly once; (g) a task that panics while holding the guard still yields exactly one `Exited`, deregisters the session, and delivers `LinkedSessionExit` to a linked trainer receiver registered through the registry; (h) the event order is `Diagnostic?` → trainer `Injection*` → `Exited`, with nothing after `Exited`.
- **MIRROR**: FINISH_GUARD, `tauri/.../streaming.rs:236-445, 447-512, 513-542` (moved code), `core/launch/diagnostics/exit_codes.rs` for classification, `core/metadata/launch_history.rs:56-121`.
- **IMPORTS**: `crosshook_core::launch::{analyze, should_surface_report, run_post_exit_hooks, TeardownReason, SessionKind}`, `crosshook_core::launch::diagnostics::{FailureMode, MAX_LOG_TAIL_BYTES}`, `std::os::unix::process::ExitStatusExt`, `super::{diagnostics::*, snapshot::record_success_effects, types::*}`, `super::super::run_blocking`.
- **GOTCHA**: `exit_code` and `signal` in the history row come from the raw exit status, not the watchdog rewrite (I-45). The trainer wrap-up is published **before** `Exited` here (S3); the relay turns `Exited` into `launch-complete`, so no listener-visible contract breaks. `Drop` must never panic or block the runtime thread for the full hook timeout. Tests must not sleep for timing; use `collect` with a timeout. Never call `Handle::current` in `Drop` (it can panic outside a runtime); use `try_current`.
- **VALIDATE**: `cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-app services::launch::tests::finalizer` (main agent, after B2).

### Task 2.6: Read-only commands, UMU enrichment, reset and running — Depends on [1.1]

- **BATCH**: B2
- **ACTION**: Implement `svc/queries.rs` (everything that is not start/stream) and the shared `enrich_umu_gameid`.
- **IMPLEMENT**: Add `pub(super) async fn enrich_umu_gameid(settings: &SettingsStore, metadata: &MetadataStore, req: &mut LaunchRequest)` (load settings, defaulting on error, then `umu_database::resolve_umu_game_id`). Add `impl LaunchService`: `validate` (sync; `core::launch::validate(req).map_err(|e| e.issue())`); `preview` (enrich → `build_launch_preview` → append only the mod-coexistence warnings, I-16; the error is mapped to `AppError` with the same text); `steam_launch_options_command` (the old body; `escape_steam_token`-joined resolved tokens; error text kept as `{error:?}` because `CommandArgumentResolveError` has no `Display`, a flagged follow-up); `launch_history` (trimmed empty name → `AppError::validation("profile_name", "profile_name is required")`; unavailable store → `Ok([])`; `limit` default 20, cap `MAX_HISTORY_LIST_LIMIT`; blocking through `run_blocking`); `reset_sessions` (`cancel_sessions_for_profile(session_profile_key_for_name(Some(name)), UserRequest)`); `running_profiles` (`active_profile_keys(Some(Game))`). Tests (`tests/queries.rs`): blank-name error, unavailable store `[]`, limit capped at 50 and defaulting to 20 with seeded rows; `reset_sessions` and `running_profiles` use the same key derivation (a blank or `None` profile name maps to the anonymous key); `preview` adds mod-coexistence issues but not low-disk, offline or trainer-hash ones.
- **MIRROR**: `tauri/.../queries.rs:22-140`, `execution.rs:43-59,503-509`, SERVICE_PATTERN.
- **IMPORTS**: `crosshook_core::launch::{validate, build_launch_preview, build_steam_launch_options_command, escape_steam_token, resolve_command_arguments_for_method, session_profile_key_for_name, TeardownReason, SessionKind}`, `crosshook_core::metadata::{MAX_HISTORY_LIST_LIMIT, LaunchHistoryEntry}`, `crosshook_core::umu_database`.
- **GOTCHA**: `validate`, `reset_sessions`, `running_profiles` and `steam_launch_options_command` are sync (S7). `launch_history` rejects on the **trimmed** name but queries with the trimmed value. The `launch_history` error text is `profile_name is required`, which `AppError` keeps unchanged (no path inside).
- **VALIDATE**: `cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-app services::launch::tests::queries` (main agent, after B2).

### Task 3.1: Orchestration — `prepare` and `run_session` — Depends on [2.1, 2.2, 2.3, 2.4, 2.5, 2.6]

- **BATCH**: B3
- **ACTION**: Implement the shared pre-spawn pipeline and the shared spawn/stream/teardown pipeline that `start_game` and `start_trainer` call.
- **IMPLEMENT**: `prepare.rs`: force the kind's flags (I-1); `validate` **first**, before any side effect (H-4); `enrich_umu_gameid`; `collect_launch_warnings`; `execution_method`; `create_log_path(kind, slug)` mapped to `AppError` (Io); `gamescope_plan`. `run.rs::run_session(prepared, command)`: `try_register_gamemode_portal_for_launch(&req, exec_method)`; `build_launch_hook_execution_context` (error → `AppError` Io `failed to prepare launch hook context: …`); `run_pre_launch_hooks` through `run_blocking`, appending warnings; register (game: `register(Game, key)`; trainer: `register_and_link_to_parent_of_kind(Trainer, key, Game)` with the same tracing); `EventPublisher::channel(session_id)` **before** spawn; trainer: publish "Trainer launch requested."; `spawner.spawn(&mut command)`; on error, trainer: publish the "failed before the trainer process started." error row, then `deregister`, then `Err(LaunchStartError { error, events: events.drain_ready() })` (game: `failed to launch helper: …`; trainer: `failed to launch trainer (method={exec}): …`); on success, trainer: "Trainer process started." and the unsupported-runtime warning (`unsupported_runtime = true`); `record_launch_started` via `run_blocking` (sanitized log path, `Some(op)` only for a non-empty id; I-28, I-46); build `SessionContext` (snapshot fields as today); `tokio::spawn` the session task (builds `SessionGuard`, `stream_session`, `guard.finish(status)`); then `choose_cancel_consumer` + `spawn_cancel_consumer`; return `LaunchSession { id, kind, warnings, log_path, events }`.
- **MIRROR**: `execution.rs:62-236, 239-490` (both paths), FINISH_GUARD, `jobs/registry.rs:134-200` (subscribe-before-start).
- **IMPORTS**: `super::{gamescope::*, queries::enrich_umu_gameid, warnings::*, portal::*, streaming::stream_session, finalizer::SessionGuard, types::*}`, `crate::jobs::create_log_path`, `crosshook_core::launch::{build_launch_hook_execution_context, run_pre_launch_hooks, session_profile_key_for_name}`.
- **GOTCHA**: Register **before** spawn and deregister on spawn failure (I-26). No history row on spawn failure (I-28). The portal guard is dropped, not unregistered, on early errors (I-22 as today). Spawn the stream task **before** the cancel consumer (I-29). Do not use `process_group`, `kill_on_drop`, `supervise_child` or any kill. The trainer log path prefix is `"trainer"`, the game's is `"game"`. The `Prepared.kind` branches live here only; `game.rs`/`trainer.rs` just build the command. Files stay under 500 lines (`run.rs` target ~280, split `run/trainer_events.rs` only if needed).
- **VALIDATE**: `cargo check --manifest-path src/crosshook-native/Cargo.toml -p crosshook-app` (main agent, after B3); behaviour is covered by 3.2's `start` tests.

### Task 3.2: `start_game`, `start_trainer`, start and parity tests — Depends on [2.1, 2.2, 2.3, 2.4, 2.5, 2.6] (codes against the `prepare` / `run_session` contract in B0 Contracts; the main agent compiles it together with 3.1)

- **BATCH**: B3
- **ACTION**: Implement the two builder-dispatch entry points and the AC2 and AC6 tests.
- **IMPLEMENT**: `game.rs::start_game(req)`: `prepare(req, Game)`, then `match prepared.execution_method`: `steam_applaunch` → `resolve_helper(SteamLaunchHelper)` + `build_helper_command`; `proton_run` → `build_proton_game_command` with piped stdout and stderr; `native` → `build_native_game_command`; anything else → `unsupported launch method: {other}`. Wrap build errors as `AppError` Io with today's texts (`failed to build Steam helper launch: …`, `failed to build Proton game launch: …`, `failed to build native game launch: …`), then `run_session`. `trainer.rs::start_trainer(req)`: `prepare(req, Trainer)`, then match on the **request's resolved method** with `execution_method` driving the branch: steam + Flatpak → `build_flatpak_steam_trainer_command` (piped); steam host → `resolve_helper(SteamLaunchTrainer)` + `build_trainer_command`; `proton_run` → `build_proton_trainer_command` (piped); native → `Err("native launch does not support trainer launch.")` kept as a defensive arm (I-3). Tests: `tests/start.rs` — (1) an invalid request fails with no side effect (registry empty, no captured spawn, no history row, no log file, no events); (2) spawn failure (`FakeSpawner::fail_next`) deregisters the session, writes no history row, and for a trainer returns `LaunchStartError.events` holding exactly "requested" then "failed before…" `Injection`s with the right message text; (3) **every event on a game session and on a trainer session carries that session's own id, and the trainer's lines never appear on the game's receiver (AC6)**; (4) `Exited` is the last event and is delivered once with a disabled metadata store; (5) `LaunchSession.log_path` and `warnings` (pre-hook warning appended) are set. `tests/parity.rs` — **AC2**: with `LaunchEnv{is_flatpak: true}`, `start_trainer` for a Steam-profile request and for its `proton_run` twin (same compatdata as prefix, same proton, same working directory, custom env `DXVK_HUD=fps`, `trainer_gamescope` override, `UmuPreference::Proton`) produces captured commands with identical env (minus host passthrough keys, as in the existing core `shape` helper), `current_dir` and arguments, and equal `gamescope_required` and `pid_capture_required`; with `LaunchEnv{is_flatpak: false}`, the Steam trainer goes through the script, so assert effective inputs instead: the script's `--working-directory` argument equals the `proton_run` command's `current_dir`, custom env is present in both commands, and `resolved_trainer_gamescope()` is equal for both requests (D11).
- **MIRROR**: `execution.rs:118-135, 303-330`, COMMAND_SHAPE (`from_profile.rs:189-210, 260-310`), `core/launch/script_runner/tests/proton_trainer_staging.rs:10`.
- **IMPORTS**: `crosshook_core::launch::script_runner::{build_helper_command, build_native_game_command, build_proton_game_command, build_proton_trainer_command, build_flatpak_steam_trainer_command, build_trainer_command}`, `crosshook_core::runtime_helpers::RuntimeHelper`, `super::{prepare::Prepared, types::*}`.
- **GOTCHA**: Never format errors with `{:?}`. The Flatpak variant of the parity test must use the injected `LaunchEnv`; the core builders still read `platform::is_flatpak()` internally, which is false on the host and harmless for the Flatpak steam-trainer builder (`normalize_flatpak_host_path` is the identity on host). Skip the suite when `/.flatpak-info` exists. Do not edit `types.rs` or `mod.rs` (frozen).
- **VALIDATE**: `cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-app services::launch::tests::start services::launch::tests::parity` (main agent, after B3).

### Task 3.3: CLI uses the service's streaming and bounded tail — Depends on [2.2, 2.3]

- **BATCH**: B3
- **ACTION**: Replace the CLI's copies of streaming and tail reading, and nothing else (D10).
- **IMPLEMENT**: Add `crosshook-app = { path = "../crosshook-app" }` to `cli/Cargo.toml`. In `cli/src/launch.rs`, replace `stream_helper_log(&mut child, &log_path)` with `crosshook_app::services::launch::stream_child_log(&mut child, &log_path, |batch| { if let LogBatch::Lines(lines) = batch { for line in lines { println!("{line}") } flush } })`, and `safe_read_tail(...)` with `crosshook_app::services::launch::read_log_tail(&log_path, MAX_LOG_TAIL_BYTES)`. The spawn paths still pass stdio null; the dry-run path, `launch_log_path`, `validate_explicit_helper`, `--scripts-dir`, the diagnostic printing and the exit codes stay as they are. Delete `stream_helper_log`, `drain_log`, `safe_read_tail` and the imports they leave unused (`AsyncReadExt`, `AsyncSeekExt`, `AsyncWriteExt`, `OpenOptions`, `sleep`, `Duration` if unused).
- **MIRROR**: `cli/src/launch.rs:149-176` (call sites), `crosshook-app` re-exports from Task 1.1.
- **IMPORTS**: `crosshook_app::log_tail::LogBatch`, `crosshook_app::services::launch::{stream_child_log, read_log_tail}`, `crosshook_core::launch::diagnostics::MAX_LOG_TAIL_BYTES`.
- **GOTCHA**: Take the stdout lock once per batch and flush so output order is unchanged. Do not call `start_game`, construct a `LaunchService` or touch metadata (D10). Do not edit `args.rs`, `cli_error.rs` or `store.rs`. The only intended output delta (unterminated last line gains `\n`; CRLF and invalid UTF-8 normalised) is documented for the PR body, and the goldens do not cover it. `crosshook-app` must not gain a UI dependency (`check-app-deps.sh` guards it).
- **VALIDATE**: `cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-cli` (the frozen goldens from 0.1 must pass **without any fixture change**: `git diff --stat src/crosshook-native/crates/crosshook-cli/tests/fixtures` is empty).

### Task 4.1: Cascade tests — Depends on [3.1, 3.2]

- **BATCH**: B4
- **ACTION**: Write the AC3 cascade test (linked child cascade and sibling survival) against real fake children.
- **IMPLEMENT**: `tests/cascade.rs` (`#[tokio::test(flavor = "multi_thread", worker_threads = 2)]`). Register through the service with scripted `FakeSpawner` children that block on a per-session release file or FIFO: T0 (profile A trainer, started **before** any game, so unlinked), G1 (profile A game), T1 (profile A trainer, linked to G1), G2 (profile B game), T2 (profile B trainer, linked to G2). Release G1 and wait for its `Exited`, then poll `running_profiles()` until G1 is deregistered and give the drain task one yield window. Assert: G1 is deregistered; T0, T1, T2 and G2 are still registered and their processes still alive (matching today's non-gamescope cascade, which records the reason but does not kill); none of G2, T2, T0 has received any event yet. Then release T1: its event stream contains the Warning/Runtime "Trainer session teardown was recorded." (proving `LinkedSessionExit` was recorded) and exactly one `Exited`. Release T0, T2 and G2: none of their streams contains the teardown warning, and each has exactly one `Exited`. Add a `reset_sessions` case: `reset_sessions("A")` returns the count of live profile-A sessions once each, signals no profile-B session, and does not cascade.
- **MIRROR**: SH_CHILD (`jobs/process/tests.rs:19-27`), `core/launch/session/registry_tests.rs` for the isolation matrix, research-business §4 cascade fixture.
- **IMPORTS**: `super::helpers::*`, `crosshook_core::launch::SessionKind`, `tokio::time::timeout`.
- **GOTCHA**: Observe the cascade through the trainer wrap-up injection (`teardown_reason != NaturalExit`), because `WatchdogOutcome` is private to the session. Do not assert process death for non-gamescope trainers. The cascade runs after `Exited` (step 10), so poll the registry instead of sleeping for a fixed time. Use release files under the Harness temp dir; no real Proton.
- **VALIDATE**: `cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-app services::launch::tests::cascade` (main agent, after B4).

### Task 4.2: Tauri relay and payload DTOs — Depends on [3.1]

- **BATCH**: B4
- **ACTION**: Implement the only place that knows Tauri event names: one task per session maps `LaunchEvent` to `launch-log`, `launch-diagnostic`, `launch-complete` and `injection-log`, adding `session_id`.
- **IMPLEMENT**: `relay/payloads.rs`: move `LaunchLogRelayState`, `transform_launch_log_line_for_ui`, `suppression_summary_line`, the xdg_backend constants and the `InjectionLogEvent` plus level, source and kind enums out of `shared.rs` verbatim; add `LaunchLogPayload<'a> { line, session_id }`, `LaunchDiagnosticPayload<'a> { #[serde(flatten)] report, session_id }` and `LaunchCompletePayload { code: Option<i32>, signal: Option<i32>, session_id }` (no `skip_serializing_if`). `relay/mod.rs`: a pure `map_event(state, event) -> Vec<RelayEmit>` (`Log::Lines` → one `Log` per non-empty line after the relay transform; `Log::Dropped{n}` → one `[crosshook] Console skipped N log batches; the raw launch log is complete.` line; `Diagnostic` → `Diagnostic`; `Injection` → `InjectionLogEvent` with the RFC 3339 `at`; `Exited` → the suppression summary line (if any) **then** `Complete`), `spawn(app, events)` (one `tauri::async_runtime::spawn` loop over `events.recv()`, emit failures only warn) and `replay_start_error(app, events)`. Update `bench.rs` replay to emit `LaunchLogPayload { line, session_id: "bench" }`. Tests (`relay/tests.rs`): (a) each payload equals its frozen Task 0.2 fixture once `session_id` is removed (`launch-diagnostic`, `launch-complete`, `injection-log` after substituting `@SESSION@`, `LaunchResult`); `launch-log` is `{line, session_id}` with `line` equal to the old string; (b) **every emitted payload has the session's `session_id` as the same string** (AC6); (c) the xdg_backend suppression: the first line verbatim, the second replaced by the notice, repeats dropped, and the summary line precedes `Complete`; (d) blank lines are not emitted; (e) `Dropped{n}` becomes one visible line; (f) `launch-complete` serializes `code`/`signal` as JSON `null` when absent.
- **MIRROR**: `tauri/.../shared.rs:12-208`, `tests/log_relay.rs:7-86`, `startup_bridge.rs` (event relay precedent), `bench.rs:269-301`.
- **IMPORTS**: `crosshook_app::services::launch::{LaunchEvent, LaunchEvents, InjectionLogData}`, `tauri::{AppHandle, Emitter}`, `chrono`, `serde`.
- **GOTCHA**: `injection-log` keeps its exact shape and `hook_id`/`hook_name` omission (I-54). The `Diagnostic` wrapper must borrow the report so the persisted struct is untouched. One relay task per session keeps order (`Log*` → `Diagnostic` → wrap-up `injection-log` → `launch-complete`). Apply `LaunchLogRelayState` per session, never globally. Do not edit `shared.rs`, `execution.rs` or `mod.rs` (Task 4.3).
- **VALIDATE**: `cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-native commands::launch::relay` (main agent, after B4).

### Task 4.3: Tauri command cutover — Depends on [3.1, 3.2] (codes against the `relay::{spawn, replay_start_error}` contract in B0 Contracts; the main agent compiles it together with 4.2)

- **BATCH**: B4
- **ACTION**: Make `launch_game`, `launch_trainer` and the query commands thin async delegates, remove the Tauri-owned pipeline, and switch the registry to `Services`.
- **IMPLEMENT**: `execution.rs`: `launch_game(app, request, service: State<'_, LaunchService>) -> Result<LaunchResult, String>` calls `service.start_game(request)`, on `Ok(session)` spawns `relay::spawn(app, session.events)` and returns `LaunchResult { succeeded: true, message: "Game launch started.", helper_log_path: session.log_path (lossy string), warnings: session.warnings }`; on `Err(e)` calls `relay::replay_start_error(&app, e.events)` and returns `Err(e.to_string())`. `launch_trainer` the same, with "Trainer launch started.". `launch_reset_sessions`, `list_running_profiles`, `validate_launch`, `preview_launch`, `build_steam_launch_options_command` and `list_launch_history_for_profile` delegate to the service with unchanged names, argument names and return types. `check_gamescope_session`, `launch_platform_status` and `check_game_running` stay as they are. Trim `shared.rs` to `LaunchResult`. Rewrite `mod.rs` (`mod execution; mod queries; mod relay; mod shared; #[cfg(test)] mod tests;` and the `__cmd__*` re-exports). Delete `streaming.rs`, `warnings.rs`, `portal.rs`, `diagnostics.rs`, `tests/{coexistence,diagnostics,log_relay}.rs`. Rewrite `tests/mod.rs` to `mod contract;` and add `tests/contract.rs` with `command_names_match_expected_ipc_contract` naming all 11 launch commands. In `tauri/src/lib.rs`, replace `.manage(Arc::new(LaunchSessionRegistry::new()))` with `.manage(services.launch.clone())`, and remove the now-unused import.
- **MIRROR**: THIN_COMMAND, IPC_NAME_TEST (`commands/settings.rs:182`), `lib.rs:41-58`.
- **IMPORTS**: `crosshook_app::services::launch::LaunchService`, `tauri::State`, `super::relay`.
- **GOTCHA**: JS argument names are camelCase and must not change (`request`, `profileName`, `limit`, `enabledOptionIds`, …); `AppHandle` and `State` are injected, not JS arguments. `scripts/check-mock-coverage.sh` scans `src-tauri` for command names, so every command stays declared there with the same name. The `validate_launch` error must remain a bare `LaunchValidationIssue` (React allow-lists its keys). Map service errors with `error.to_string()` only. Do not leave `Arc<LaunchSessionRegistry>` managed anywhere (two registries would split the session world, D9).
- **VALIDATE**: `cargo check --manifest-path src/crosshook-native/Cargo.toml -p crosshook-native --all-targets`, `./scripts/check-mock-coverage.sh`, and `cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-native commands::launch` (main agent, after B4).

### Task 4.4: TypeScript types, browser mocks and tests — Depends on [3.1]

- **BATCH**: B4
- **ACTION**: Add the optional `session_id` types, make the browser-dev mocks emit the real wire shapes, and cover the new shapes in Vitest. The same PR as the Rust cutover (D4).
- **IMPLEMENT**: `utils/log.ts`: add `LaunchLogPayload { line: string; session_id?: string }` to the `LogPayload` union (`normalizeLogMessage` needs no change; `update-log` strings stay valid). `types/diagnostics.ts`: `export type LaunchDiagnosticEvent = DiagnosticReport & { session_id?: string }`; do **not** touch `DiagnosticReport` or `isDiagnosticReport`. `types/launch.ts`: `export interface LaunchCompletePayload { code: number | null; signal: number | null; session_id?: string }`. `hooks/useLaunchState.ts`: use `subscribeEvent<LaunchDiagnosticEvent>` for `launch-diagnostic` (type only). `lib/mocks/handlers/launch.ts`: `scheduleLaunchLogSequence` gains a `sessionId` parameter and emits `{ line, session_id }`, `{ ...report, session_id }` and `{ code: 0, signal: null, session_id }`; `launch_game` uses `mock-game-session-${steamAppId}`; `launch_trainer` hoists the id `scheduleTrainerInjectionLogSequence` already builds (`mock-trainer-session-${steamAppId}`) so a trainer's four events share one id. Tests: `ConsoleDrawer.test.tsx` gets a case emitting `{ line: 'a', session_id: 's1' }` expecting "1 line"; new `ConsoleView.test.tsx` emits `{ line, session_id }` and asserts the line text renders and no `{"line"` appears; new `useLaunchState.events.test.ts` captures `subscribeEvent` handlers (mock `@/lib/events` so the handler map is exposed) and asserts a `launch-diagnostic` with an extra `session_id` key still matches by `log_tail_path`, and a non-matching path is still ignored; new `lib/mocks/__tests__/launch.session.test.ts` runs the `launch_trainer` mock under fake timers and asserts the emitted `launch-log`, `launch-diagnostic`, `launch-complete` and `injection-log` events share one `session_id`.
- **MIRROR**: VITEST_HANDLER_CAPTURE, `components/layout/__tests__/ConsoleDrawer.test.tsx:38-62`, `lib/mocks/eventBus.ts`, research-ux §4.2–4.4.
- **IMPORTS**: `@/lib/events` (`emitMockEvent`), `@/test/render` (`renderWithMocks`), `vitest`.
- **GOTCHA**: `LaunchResult` and `LaunchValidationIssue` do not change. `session_id` is `?:` everywhere (no `exactOptionalPropertyTypes`). If `types/index.ts` enumerates exports instead of `export *`, add the new type names there, and say so in the task report. Do not change React behaviour or add console labels. The existing `useLaunchState.test.ts` mocks `subscribeEvent` to a no-op, so leave it alone and put the new cases in the new file.
- **VALIDATE**: `cd src/crosshook-native && npm run typecheck && npm test && npm run lint` (main agent, after B4); `npm run test:smoke` exercises `tests/pipeline.spec.ts` against the mocks.

### Task 5.1: Docs and dead-code sweep — Depends on [4.1, 4.2, 4.3, 4.4]

- **BATCH**: B5
- **ACTION**: Document the service, fix stale references, and verify nothing of the old pipeline is left.
- **IMPLEMENT**: Add a `LaunchService` section to `app/README.md` (API, event contract and ordering, `ProcessSpawner`, the registry owned by `Services`, the "no kill path" rule, the storage boundary). Update `docs/architecture/adr-0002-flatpak-portal-contracts.md` lines 191 and 199 so `spawn_gamescope_watchdog` in `execution.rs` becomes `spawn_cancel_consumer` in `crates/crosshook-app/src/services/launch/gamescope.rs`. Sweep: no remaining references to `safe_read_tail`, `stream_helper_log`, `drain_log`, `spawn_log_stream`, `Arc<LaunchSessionRegistry>` as managed state, or `commands/launch/{streaming,warnings,portal,diagnostics}`; every file under `svc/` is under 500 lines (`wc -l`); no `unwrap`/`expect` added in service code paths other than tests. Run `./scripts/format.sh --modified --docs` so the lint-autofix bot does not push a format commit.
- **MIRROR**: `app/README.md` § ProfileWriter (section shape).
- **IMPORTS**: none.
- **GOTCHA**: Do not edit `CLAUDE.md` or `AGENTS.md` unless a rule actually changes (none does). Leave `graphify-out/` alone.
- **VALIDATE**: `./scripts/format.sh --modified --docs` leaves no diff; a repo grep for the removed names returns only `docs/plans/` and `docs/research/` history.

## Testing Strategy

Only critical tests. There are no wiring or tautology tests, and the single IPC-name test is the repo's existing contract convention (`commands/settings.rs:182`), which the cutover touches 11 command names for.

### Acceptance-criterion → test map

| Acceptance criterion (issue.md)                                                                                              | Test (task)                                                                                                                                                                                                       | What it proves                                                                                                                                                                                                                                                                                                                                  |
| ---------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **AC1** Predicate table tests cover method × Flatpak × gamescope setting × game/trainer                                      | `crosshook-app` `services::launch::tests::gamescope` (2.1)                                                                                                                                                        | the full cross product of method {steam, proton_run, native} × flatpak × config {disabled, enabled, allow_nested} × inside_session × kind, with expected `execution_method`, `gamescope_required` and `pid_capture_required` written as literals from the research-business truth tables; trainer config rules; `choose_cancel_consumer` choice |
| **AC2** Steam-profile trainer request equals a `proton_run` trainer request in env, `working_directory` and gamescope config | `services::launch::tests::parity` (3.2)                                                                                                                                                                           | on Flatpak the captured commands are identical in env, `current_dir`, args, and the predicate; on host the effective inputs agree (the Steam trainer goes through the script, D11)                                                                                                                                                              |
| **AC3a** Finalizer side effects: history row, version snapshot, known-good tag                                               | `services::launch::tests::finalizer` (a)–(f) (2.5) and `tests::snapshot` (2.4)                                                                                                                                    | success writes `succeeded` + snapshot + tag; failure and signal write `failed` and neither; a watchdog kill writes `succeeded` and both; `UpdateInProgress` skips only the snapshot; disabled metadata still publishes `Exited` once                                                                                                            |
| **AC3b** Linked-child cascade, and a sibling session survives                                                                | `services::launch::tests::cascade` (4.1); `tests::finalizer` (g) (2.5)                                                                                                                                            | G1's exit records `LinkedSessionExit` for T1 only; G2, T2 and the unlinked T0 are untouched and still registered; `reset_sessions` is per profile and does not cascade; a panicking stream task still cascades, deregisters and publishes one `Exited`                                                                                          |
| **AC4** Tauri `launch_game` and `launch_trainer` delegate with unchanged behaviour; React smoke passes                       | `commands::launch::relay::tests` (4.2) against the Phase-0 fixtures (0.2); `commands::launch::tests::contract` (4.3); `npm test` (4.4); `npm run test:smoke` (`tests/pipeline.spec.ts`)                           | old payload fields are byte-identical; the 11 command names are pinned; the console and `useLaunchState` accept the new shapes; the mocks emit them with a single id per session                                                                                                                                                                |
| **AC5** CLI dry-run and launch output unchanged for fixture profiles                                                         | `crosshook-cli` `tests/launch_golden.rs` (0.1, frozen; run after 3.3 with no fixture diff)                                                                                                                        | stdout, stderr and exit code for 13 cases are unchanged; `launch_operations` stays empty (no YAN-837 creep)                                                                                                                                                                                                                                     |
| **AC6** Every event carries `session_id`                                                                                     | `services::launch::tests::start` (3.2: type-level plus a disjoint-stream check across a game and a trainer session) and `relay::tests` (4.2: every emitted payload carries the same id string as `injection-log`) | all four service variants and all four wire payloads                                                                                                                                                                                                                                                                                            |

### Other critical tests (each tied to a security or data-integrity invariant)

| Test                                                                                                                                    | Task | Invariant                |
| --------------------------------------------------------------------------------------------------------------------------------------- | ---- | ------------------------ |
| Invalid UTF-8 byte plus a 1 MiB newline-free line plus exit 0 → complete log, `Some(success)`, no stall                                 | 2.3  | D7, H-2                  |
| ≥ 300 unread batches → `Log{Dropped{n}}` then `Exited` last                                                                             | 2.3  | D2, M-3                  |
| `read_log_tail` bounded to the cap, lossy, missing file → `""`                                                                          | 2.2  | H-3, issue "tail reader" |
| Invalid request has no side effect (no session, no spawn, no log file, no history)                                                      | 3.2  | H-4, I-1..3              |
| Spawn failure deregisters, writes no history row, and returns the pre-spawn trainer injections                                          | 3.2  | I-26, I-28, I-55         |
| Frozen payload fixtures (`launch-diagnostic`, `launch-complete`, `injection-log`, `LaunchResult`) equal relay output minus `session_id` | 4.2  | D4, storage boundary     |
| xdg_backend suppression per session; summary line before `launch-complete`; blank lines dropped                                         | 4.2  | I-38                     |
| `launch_history` guards (blank name, unavailable store, limit cap)                                                                      | 2.6  | I-61                     |
| `preview` adds only mod-coexistence warnings                                                                                            | 2.6  | I-16                     |

### Edge cases checklist

- [x] Blank, `None` and anonymous profile keys (2.6)
- [x] Maximum sizes (2 MiB tail cap, 64 KiB partial cap, 256-event channel, history limit 50)
- [x] Invalid input (bad bytes, invalid request, native trainer)
- [x] Concurrent sessions and sibling isolation (4.1)
- [x] Panic or abort in the session task (2.5 (g))
- [x] Metadata disabled, and an unavailable store (2.5 (f), 2.6)
- [x] Lagging receiver (2.3)
- [x] Network failure: not applicable (UMU lookup stays fail-soft and is unchanged)

## Validation Commands

Run from the repo root unless noted. The main agent runs these once per batch gate and in full at the end. Subagents do not run them.

### Static analysis

```bash
cargo fmt --manifest-path src/crosshook-native/Cargo.toml --all -- --check
cargo clippy --manifest-path src/crosshook-native/Cargo.toml --workspace --all-targets -- -D warnings
./scripts/lint.sh
./scripts/check-host-gateway.sh
./scripts/check-host-gateway.sh --self-test
./scripts/check-app-deps.sh
./scripts/check-startup-path.sh
./scripts/check-schema-version.sh
./scripts/check-mock-coverage.sh
```

EXPECT: zero warnings; no host-gateway violations (the service only runs core-built commands; fakes live under `tests/`); `crosshook-app` has no UI-toolkit dependency; schema stays unchanged; every `#[tauri::command]` still has a mock.

### Rust tests

```bash
cargo test --manifest-path src/crosshook-native/Cargo.toml --workspace
cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-app services::launch
cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-native commands::launch
cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-cli --test launch_golden
```

EXPECT: all pass. The CLI golden passes with `git diff --stat src/crosshook-native/crates/crosshook-cli/tests/fixtures` empty, and the event fixtures under `src-tauri/.../fixtures/events` are unchanged from B0.

### Frontend

```bash
cd src/crosshook-native && npm run typecheck && npm test && npm run lint && npm run dev:browser:check && npm run test:smoke
```

EXPECT: both tsconfigs clean; Vitest green (Console, Drawer, `useLaunchState` events, mock session ids); Biome clean; Playwright smoke green (`pipeline.spec.ts` launches in browser-dev mode).

### Formatting

```bash
./scripts/format.sh --modified --docs
```

EXPECT: no diff afterwards (avoids a `chore: apply format (lint-autofix workflow)` bot commit on the plan docs).

### Manual validation (issue test plan)

- [ ] `./scripts/dev-native.sh`, a `proton_run` game plus its trainer: launch both, exit the game, and confirm the trainer receives the cascade (the "Trainer session teardown was recorded." row appears in the trainer's injection log) and the Console shows both sessions' lines.
- [ ] A `steam_applaunch` game plus its trainer inside Flatpak: confirm the trainer runs with `proton_run` semantics, the gamescope watchdog gating matches the pre-change behaviour, and `launch-complete` arrives for both sessions.
- [ ] Check the Console badge and the diagnostic banner on a failing launch.

## Acceptance Criteria

- [ ] All 17 tasks complete across B0 → B5 (ownership table respected).
- [ ] **AC1**: the predicate table test passes (Task 2.1).
- [ ] **AC2**: the trainer parity test passes in both the Flatpak and host variants (Task 3.2).
- [ ] **AC3**: the finalizer side-effect tests and the cascade test, including sibling survival, pass (Tasks 2.4, 2.5, 4.1).
- [ ] **AC4**: the Tauri adapter matches the frozen Phase-0 payloads, and Vitest plus `npm run test:smoke` are green (Tasks 0.2, 4.2–4.4).
- [ ] **AC5**: the frozen CLI golden passes unmodified, and `stream_helper_log`, `drain_log` and `safe_read_tail` are gone (Tasks 0.1, 3.3).
- [ ] **AC6**: every service event and wire payload carries `session_id` (Tasks 3.2, 4.2).
- [ ] No remaining reference to the deleted Tauri launch modules; every file under `svc/` is under 500 lines.
- [ ] All validation commands pass.

## Completion Checklist

- [x] Follows discovered patterns (service, finish guard, event receiver, injected env, thin command, IPC-name test, harness)
- [x] Error handling matches the codebase (`AppError` with verbatim messages, no `{:?}` Debug)
- [x] Logging follows conventions (`tracing::warn!` fail-soft, no paths in events)
- [x] Tests are critical-only, deterministic (no sleeps for timing) and isolated (tempdir stores, in-memory metadata)
- [x] No hardcoded limits (core constants: `MAX_LOG_TAIL_BYTES`, `PARTIAL_CAP`, `MAX_HISTORY_LIST_LIMIT`)
- [x] Documentation updated (app README, ADR-0002 references); `CLAUDE.md` and `AGENTS.md` intentionally unchanged
- [x] No scope additions (NOT Building enforced)
- [x] Self-contained: B0 Contracts plus Mandatory Reading let each slice be built without questions

## Risks

| Risk                                                                              | Likelihood | Impact | Mitigation                                                                                                                                                          |
| --------------------------------------------------------------------------------- | ---------- | ------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Wire drift breaks React (shape change, flatten collision, mock divergence)        | Medium     | High   | Phase-0 fixtures (0.2); per-line `launch-log`; the diagnostic wrapper borrows the report; mocks and types change in the same PR; Vitest plus smoke                  |
| Trainer parity regression (the CLAUDE.md parity clause)                           | Medium     | High   | one `execution_method` plus one predicate fed by `resolved_trainer_gamescope()`; the parity test in both variants                                                   |
| Cross-session kill (a new kill path, process groups)                              | Low        | High   | D1: the spawner has one method and no process-group, kill or wait API; the cascade test asserts siblings are untouched and processes alive; `check-host-gateway.sh` |
| Finalizer effect loss or duplication during the move                              | Medium     | High   | `snapshot.rs` and `finalizer.rs` move code verbatim; ordered tests; exactly-once `Exited`; the guard test with a panic                                              |
| Pipe backpressure or data loss under a slow UI                                    | Medium     | Medium | one dedicated writer task keeps the pipes drained; the log file stays the source of truth; `Dropped{n}` is visible; a lagging receiver still gets `Exited`          |
| Event-order change (wrap-up before `launch-complete`, S3)                         | Low        | Low    | no listener depends on the order (research-ux §2.2); recorded in the spec and the PR body                                                                           |
| CLI output delta for unterminated, CRLF or non-UTF-8 logs                         | Low        | Low    | outside the golden by design; named in the PR body (a deliberate, approved delta)                                                                                   |
| Shared-file collisions across parallel slices                                     | Medium     | Medium | B1 pre-creates the whole module tree and the harness; exclusive ownership per file (audited); the Cargo.toml edits are in different batches                         |
| B1 and B2 are not independently compilable                                        | Medium     | Low    | the main agent's first compile gate is after B2, and the contracts are frozen in this plan; the B2 leaves only depend on B1 types                                   |
| `Drop` guard blocks or double-panics                                              | Low        | High   | `Handle::try_current`, `spawn_blocking` for the hooks, a poison-safe registry call, done-flags; the guard test                                                      |
| Error text drift from `AppError` sanitization (`$HOME` in helper-candidate paths) | Medium     | Low    | message texts kept verbatim; only home paths are redacted (S10); no UI parses them                                                                                  |
| Lint workflow does not run while the PR has merge conflicts                       | Medium     | Medium | rebase onto `origin/main` before pushing, and run `./scripts/format.sh --modified --docs`                                                                           |

## Notes

- **Decisions D1–D12 are binding** ([decisions.md](./decisions.md)); [feature-spec.md](./feature-spec.md) resolves their two conflicts (S3: `Exited` versus wrap-up order) and records the discrepancies found in the research.
- **Storage boundary**: TOML untouched. SQLite is written by the same calls under the same conditions. No schema bump. Sessions, channels and relay state are runtime-only.
- **Batch model**: B0 captures frozen goldens from the unmodified code → B1 freezes contracts → B2 six disjoint leaves → B3 orchestration, builders and the CLI → B4 cascade tests, relay, Tauri cutover, TS → B5 docs. The main agent compiles once per batch gate (B2, B3, B4) and runs the full validation at the end. Subagents skip builds.
- **Trunk-only**: one PR off `main`, squash merge, title `refactor(launch): move the launch pipeline into crosshook-app with session-scoped event streams`. The body links `Closes #534`, lists the three kept divergences with their follow-up issues, the CLI delta, and the S3 ordering change.
- **Follow-ups to file** (D8, D12): Flatpak trainer capture file; empty-exe watchdog drops the cancel channel; Flatpak Steam trainer hook environment; `/tmp` log and PID `create_new` hardening (A-1); `crosshook-cli/src` in the host-gateway scan; CLI bugs B1–B4; `session_id` on `LaunchResult` plus an id-based diagnostic filter; Steam-manifest lookup dedup; `should_skip_gamescope` sharing; whether to kill non-gamescope trainers on cascade.
