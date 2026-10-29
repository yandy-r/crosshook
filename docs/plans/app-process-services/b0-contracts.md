# YAN-815 / YAN-1538 B0 contracts

Signatures only. Paths are relative to `src/crosshook-native/`:

- `core/` = `crates/crosshook-core/src`
- `app/` = `crates/crosshook-app/src`
- `tauri/` = `src-tauri/src`

Decisions: [decisions.md](./decisions.md) D1–D15. Every "existing" signature below was read in this worktree.

**FROZEN** means the wire shape, name or signature must not change after B1. Anything not marked frozen may be adjusted only by its owning task, and the change must be reported to Main.

## 0. Existing anchors (verified, unchanged unless noted)

```rust
// app/jobs (registry.rs, types.rs, process.rs)
JobRegistry::start<F,Fut,R>(&self, JobSpec, F) -> Result<JobHandle, AppError>   // R: Into<Outcome>; key held → Conflict "a {kind} job is already running for this target"
JobRegistry::{subscribe, cancel, outcome, reap}(&self, &JobId); #[cfg(test)] pub(crate) job_counts() -> (usize, usize)
JobContext::{id, cancel_token, is_cancelled, cancelled, log(Vec<String>), log_batch(LogBatch), progress}
JobHandle::{id, cancel_token, cancel, recv, into_events}; JobEvents::recv() -> Option<JobEvent>
JobKey::new(impl Into<String>); JobSpec::new(&'static str).with_key(JobKey); Outcome::{Succeeded, Failed(AppError), Cancelled}
pub async fn supervise_child(child: Child, cancel: &CancellationToken, grace: Duration) -> io::Result<ChildExit>
pub enum ChildExit { Natural(ExitStatus), Terminated(ExitStatus), Killed(ExitStatus) }  // status(), was_cancelled()
pub const DEFAULT_KILL_GRACE: Duration = 5s
pub fn create_log_path(prefix: &str, target_slug: &str) -> Result<PathBuf, String>
pub fn create_log_path_in(dir: PathBuf, prefix: &str, target_slug: &str) -> Result<PathBuf, String>
pub fn slugify_target(name: &str, fallback: &str) -> String
pub fn kill_processes_using_prefix(prefix_path: &Path)
// app/log_tail.rs
pub async fn follow(path: impl Into<PathBuf>, config: TailConfig, cancel: CancellationToken, sink: impl FnMut(LogBatch)) -> io::Result<()>
pub enum LogBatch { Lines(Vec<String>), Dropped { n: u64 }, Truncated { bytes: u64 } }; pub const PARTIAL_CAP: usize
// app/services/mod.rs
pub type BoxFuture<'a, T>; async fn run_blocking<T,F>(F) -> Result<T, AppError>   // private to services (pub(super) re-export pattern as launch)
// app/error.rs
AppError::new(kind, msg) (pub(crate), sanitizes $HOME); AppError::validation(field, msg); kind(); message()
// core/platform
pub fn host_command_with_env_and_directory_lifecycle(program, &BTreeMap env, Option<&str> dir, &BTreeMap custom) -> Command
pub(super) fn host_command_with_env_and_directory_lifecycle_inner(program, envs, directory, flatpak: bool, custom) -> Command   // → pub(crate) (B1)
// core/prefix_deps
pub fn runner::normalize_prefix_path(&str) -> Result<String, PrefixDepsError>     // canonical pfx
pub async fn runner::restore_prefix_windows_version(&PrefixVersionRestorePlan) -> PrefixVersionRestoreOutcome
pub fn runner::sanitize_output_for_ui(&str) -> String
pub fn detection::{detect_binary(&str) -> BinaryDetectionResult, resolve_winetricks_path() -> Option<String>}
pub fn validation::validate_protontricks_verbs(&[String]) -> Result<(), PrefixDepsError>
// core/metadata MetadataStore
{upsert,load,mark_…_failed,delete}_prefix_version_restore; upsert_prefix_dep_state; load_prefix_dep_states; lookup_profile_id; is_available(); open_in_memory(); disabled()
```

## 1. Core changes (B1)

```rust
// core/platform/gateway.rs + mod.rs
pub(crate) fn host_command_with_env_and_directory_lifecycle_inner(/* unchanged params */) -> Command;   // visibility only
// mod.rs: pub(crate) use gateway::host_command_with_env_and_directory_lifecycle_inner;

// core/launch/runtime_helpers/proton_command.rs   FROZEN
/// `<proton> run` via host_command_with_env_and_directory_lifecycle (no wrappers, no umu).
pub fn build_direct_proton_command_lifecycle(proton_path: &str, env: &BTreeMap<String, String>, working_directory: Option<&str>) -> Command;
pub(crate) fn build_direct_proton_command_lifecycle_with(proton_path: &str, env: &BTreeMap<String, String>, working_directory: Option<&str>, flatpak: bool) -> Command;
// runtime_helpers/mod.rs re-exports build_direct_proton_command_lifecycle

// core/install (service.rs; mod.rs re-exports)   FROZEN
pub struct PreparedInstall { pub command: tokio::process::Command, pub prefix_path: PathBuf }
/// validate + normalize + provision prefix + lifecycle command (stdio NOT attached).
pub fn prepare_install(request: &InstallGameRequest) -> Result<PreparedInstall, InstallGameError>;
/// discovery + confirmed exe + reviewable profile; message "Installer completed. Review the generated profile."
pub fn finish_install(request: &InstallGameRequest, prefix_path: &Path, log_path: &Path) -> InstallGameResult;
// DELETED: install_game; InstallGameError::{RuntimeUnavailable, LogAttachmentFailed}

// core/update (service.rs)   FROZEN
pub fn build_update_command(request: &UpdateGameRequest) -> Result<Command, UpdateGameError>;   // lifecycle, no stdio; was (request, log_path)
// DELETED: update_game; UpdateGameError::{RuntimeUnavailable, LogAttachmentFailed}

// core/run_executable (service/*; mod.rs re-exports)   FROZEN
pub struct PreparedRunExecutable { pub command: Command, pub prefix_path: PathBuf, pub is_throwaway: bool }
pub fn adhoc_prefix_root() -> Result<PathBuf, RunExecutableError>;                    // flatpak host root/_run-adhoc or <data_local>/crosshook/prefixes/_run-adhoc
pub fn resolve_adhoc_prefix_path(adhoc_root: &Path, executable_path: &Path, run_suffix: &str) -> PathBuf;   // <root>/<slug>-<run_suffix>
pub fn is_throwaway_prefix_path(prefix_path: &Path) -> bool;                         // = adhoc_prefix_root().is_ok_and(|r| is_throwaway_prefix_path_in(p, &r))
pub fn is_throwaway_prefix_path_in(prefix_path: &Path, adhoc_root: &Path) -> bool;   // parent == root, non-empty name, no `..`
/// validate; empty prefix → resolve_adhoc_prefix_path (is_throwaway=true); provision; lifecycle command (no stdio).
pub fn prepare_run_executable(request: &RunExecutableRequest, adhoc_root: &Path, run_suffix: &str) -> Result<PreparedRunExecutable, RunExecutableError>;
pub fn build_run_executable_command(request: &RunExecutableRequest, prefix_path: &Path) -> Result<Command, RunExecutableError>;   // was (…, log_path)
// DELETED: run_executable, resolve_default_adhoc_prefix_path(+_with,+_from_data_local_dir); RunExecutableError::{RuntimeUnavailable, LogAttachmentFailed}

// core/prefix_deps: runner.rs → runner/{mod,command,snapshot,check,install,restore,sanitize}.rs + runner/tests/*.rs (behaviour-preserving split)
pub(crate) fn build_check_command_with(binary_path: &str, prefix_path: &str, tool_type: PrefixDepsTool, steam_app_id: Option<&str>, flatpak: bool) -> Result<Command, PrefixDepsError>;   // lifecycle; YAN-1538 seam
pub async fn check_installed(binary_path: &str, prefix_path: &str, tool_type: PrefixDepsTool, steam_app_id: Option<&str>) -> Result<Vec<String>, PrefixDepsError>;   // unchanged signature; now lifecycle builder
impl PreparedPrefixDependencyInstall {
    pub fn restore_plan(&self) -> Option<&PrefixVersionRestorePlan>;                  // unchanged
    pub fn into_parts(self) -> (Command, Option<PrefixVersionRestorePlan>);           // command has process_group(0), kill_on_drop, piped stdout/stderr, null stdin
}
pub fn is_windows_version_verb(verb: &str) -> bool;                                 // was private
pub(crate) async fn restore_prefix_windows_version_with_timeout(plan: &PrefixVersionRestorePlan, timeout: Duration) -> PrefixVersionRestoreOutcome;   // unchanged, now pub(crate)
// DELETED: build_prefix_dep_command, HostCommandBuilder, PreparedPrefixDependencyInstall::{spawn, spawn_with_restore_timeout}, PrefixDependencyInstall (+take_stdout/take_stderr/wait_and_restore), install_packages, prefix_deps::lock (module)
// core/prefix_deps/validation.rs   FROZEN
pub fn parse_nonzero_steam_app_id(raw: &str) -> Option<String>;                     // moved verbatim from tauri/commands/prefix_deps.rs:178

// core/metadata   FROZEN
pub fn prefix_version_restore_store::list_restores(conn: &Connection) -> Result<Vec<PrefixVersionRestoreJournalRow>, MetadataStoreError>;   // SELECT … ORDER BY prefix_path
impl MetadataStore { pub fn list_prefix_version_restores(&self) -> Result<Vec<PrefixVersionRestoreJournalRow>, MetadataStoreError>; }   // with_sqlite_conn; read-only
```

## 2. App jobs (B1)

```rust
// app/jobs/process.rs (+ jobs.rs re-exports)   FROZEN
/// supervise_child + `kill`: kill fired → immediate killpg(SIGKILL) (also during the TERM grace) → ChildExit::Killed.
pub async fn supervise_child_with_kill(child: Child, cancel: &CancellationToken, kill: &CancellationToken, grace: Duration) -> io::Result<ChildExit>;
// supervise_child(child, cancel, grace) = supervise_child_with_kill(child, cancel, &CancellationToken::new(), grace)
/// create_new + mode 0o600; returns the open handle (D11, M3 cheap part). create_log_path_in delegates to it.
pub fn create_log_file_in(dir: PathBuf, prefix: &str, target_slug: &str) -> Result<(PathBuf, std::fs::File), String>;
/// /proc environ boundary scan without killing; kill_processes_using_prefix uses it.
pub fn processes_using_prefix(prefix_path: &Path) -> Vec<u32>;

// app/jobs/registry.rs
pub fn is_key_held(&self, key: &JobKey) -> bool;
```

## 3. Shared process plumbing: `app/services/process/` (B1, pub(crate) unless noted)

```rust
// process/mod.rs
pub(crate) const PREFIX_KEY_PREFIX: &str = "prefix:";
pub(crate) const MSG_PREFIX_BUSY: &str = "another operation is already running for this prefix";
/// normalize_prefix_path(raw) → (canonical, JobKey("prefix:<canonical>")).
pub(crate) fn prefix_job_key(raw_prefix: &str) -> Result<(String, JobKey), AppError>;
pub(crate) fn prefix_job_key_for_path(canonical: &Path) -> JobKey;
/// Conflict from jobs.start → AppError(Conflict, text); other kinds unchanged.
pub(crate) fn conflict_as(error: AppError, text: &str) -> AppError;

// process/owner.rs   FROZEN (pub, re-exported from services::process)
#[derive(Debug, Clone, PartialEq, Eq, Hash)] pub struct JobOwner(String);
impl JobOwner { pub fn new(label: impl Into<String>) -> Self; }
pub(crate) struct ActiveJob { pub(crate) id: JobId, pub(crate) kill: CancellationToken }
#[derive(Clone, Default)] pub(crate) struct ActiveJobs(Arc<Mutex<HashMap<JobOwner, ActiveJob>>>);
impl ActiveJobs {
    pub(crate) fn register(&self, owner: JobOwner, job: ActiveJob) -> ActiveGuard;   // replaces owner's entry
    pub(crate) fn get(&self, owner: &JobOwner) -> Option<(JobId, CancellationToken)>;
    #[cfg(test)] pub(crate) fn len(&self) -> usize;
}
pub(crate) struct ActiveGuard;  // Drop removes entry only if id matches; poison-tolerant

// process/run.rs   FROZEN (pub types)
pub struct ProcessRun<C> { /* run_id: JobId, events: JobEvents, completion: Option<oneshot::Receiver<C>>, jobs: JobRegistry */ }
pub enum RunEvent<C> { Log(LogBatch), Finished(Option<C>) }
impl<C: Send + 'static> ProcessRun<C> {
    pub fn run_id(&self) -> &str;
    /// Log → Log; Progress skipped; Finished → reap, await completion → Finished(c); then None.
    pub async fn next(&mut self) -> Option<RunEvent<C>>;
}
pub(crate) fn start_run<C, F, Fut>(jobs: &JobRegistry, spec: JobSpec, body: F) -> Result<StartingRun<C>, AppError>
    where F: FnOnce(JobContext, oneshot::Sender<C>, oneshot::Sender<()>) -> Fut + Send + 'static, Fut: Future<Output = Outcome> + Send + 'static;
pub(crate) struct StartingRun<C> { /* run, started_rx */ }
impl<C> StartingRun<C> { pub(crate) async fn started(self) -> Result<ProcessRun<C>, AppError>; }   // waits body's `started` signal (handle before output)

// process/child.rs
pub(crate) enum ChildOutput { LogFile, Piped }
pub(crate) struct LogFile { pub(crate) path: PathBuf, pub(crate) file: std::fs::File }
/// stdin null; LogFile: stdout/stderr = file.try_clone(); Piped: tee pipes → log file (process/tee.rs);
/// process_group(0) + kill_on_drop(true); spawn; supervise_child_with_kill(ctx.cancel_token(), kill, DEFAULT_KILL_GRACE)
/// concurrently with log_tail::follow → sink; after exit: tee drain bounded by DRAIN_GRACE (2 s) and cancel,
/// then tail final drain; returns only after the final drain.
pub(crate) async fn run_child(ctx: &JobContext, command: Command, log: LogFile, output: ChildOutput,
    kill: &CancellationToken, sink: impl FnMut(LogBatch) + Send) -> io::Result<ChildExit>;
pub(crate) const DRAIN_GRACE: Duration = Duration::from_secs(2);
pub(crate) fn exit_code_and_signal(exit: &ChildExit) -> (Option<i32>, Option<i32>);

// process/tee.rs (moved from services/launch/streaming.rs; streaming.rs imports these, public API unchanged)
pub(crate) struct PipeTee;
impl PipeTee { pub(crate) async fn finish(self); pub(crate) async fn finish_within(self, grace: Duration, cancel: &CancellationToken) -> bool; }
pub(crate) async fn spawn_pipe_tee(child: &mut Child, log_path: &Path) -> PipeTee;
```

## 4. Install service: `app/services/install/` (B2)

```rust
pub const INSTALL_GAME_JOB_KIND: &str = "install-game";
#[derive(Clone)] pub struct InstallService { jobs: JobRegistry, log_dir: PathBuf }
impl InstallService {
    pub fn new(jobs: JobRegistry) -> Self;                                   // log_dir = jobs::LOG_DIR
    #[cfg(test)] pub(crate) fn with_log_dir(jobs: JobRegistry, log_dir: PathBuf) -> Self;
    pub async fn default_prefix_path(&self, profile_name: String) -> Result<String, AppError>;
    pub async fn validate(&self, request: InstallGameRequest) -> Result<(), AppError>;
    /// prepare_install (sync errors) → prefix key → job "install-game"; completion = Result<InstallGameResult, AppError>
    /// (non-zero exit → InstallGameError::InstallerExitedWithFailure text; cancel → Outcome::Cancelled).
    pub async fn start(&self, request: InstallGameRequest) -> Result<ProcessRun<Result<InstallGameResult, AppError>>, AppError>;
}
assert_impl_all!(InstallService: Send, Sync);
```

## 5. Update service: `app/services/update/` (B2)

```rust
pub const UPDATE_GAME_JOB_KIND: &str = "update-game";
// types.rs   FROZEN wire
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateGameResponse { #[serde(flatten)] pub result: UpdateGameResult, pub run_id: String }   // additive run_id
pub struct UpdateCompletion { pub code: Option<i32>, pub signal: Option<i32> }
#[derive(Clone)] pub struct UpdateService { jobs: JobRegistry, active: ActiveJobs, log_dir: PathBuf }
impl UpdateService {
    pub fn new(jobs: JobRegistry) -> Self;
    #[cfg(test)] pub(crate) fn with_log_dir(jobs: JobRegistry, log_dir: PathBuf) -> Self;
    pub async fn validate(&self, request: UpdateGameRequest) -> Result<(), AppError>;
    pub async fn start(&self, owner: JobOwner, request: UpdateGameRequest) -> Result<(UpdateGameResponse, ProcessRun<UpdateCompletion>), AppError>;
    /// SIGTERM→grace→SIGKILL on own active job; false when none/foreign.
    pub fn cancel(&self, owner: &JobOwner) -> bool;
}
```

## 6. Run-executable service: `app/services/run_executable/` (B2)

```rust
pub const RUN_EXECUTABLE_JOB_KIND: &str = "run-executable";
// types.rs   FROZEN wire
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RunExecutableResponse { #[serde(flatten)] pub result: RunExecutableResult, pub run_id: String }
pub struct RunCompletion { pub code: Option<i32>, pub signal: Option<i32> }
#[derive(Debug)] pub enum RunExecutableStartError { Validation(RunExecutableValidationError), Failed(AppError) }
#[derive(Clone)] pub struct RunExecutableService { jobs: JobRegistry, active: ActiveJobs, log_dir: PathBuf, adhoc_root: Option<PathBuf> }
impl RunExecutableService {
    pub fn new(jobs: JobRegistry) -> Self;
    #[cfg(test)] pub(crate) fn with_dirs(jobs: JobRegistry, log_dir: PathBuf, adhoc_root: PathBuf) -> Self;
    pub async fn validate(&self, request: RunExecutableRequest) -> Result<(), RunExecutableStartError>;
    /// prepare_run_executable(root, uuid-12 suffix) → prefix key (throwaway too) → job; body: run_child;
    /// stop → kill_processes_using_prefix(resolved); throwaway → cleanup (guard, remove_dir_all, sweep, bounded retries) before completion.
    pub async fn start(&self, owner: JobOwner, request: RunExecutableRequest) -> Result<(RunExecutableResponse, ProcessRun<RunCompletion>), RunExecutableStartError>;
    pub fn cancel(&self, owner: &JobOwner) -> bool;   // TERM→grace→KILL
    pub fn stop(&self, owner: &JobOwner) -> bool;     // immediate group SIGKILL (+ sweep in job)
}
// cleanup.rs
pub(crate) fn cleanup_throwaway_prefix(prefix_path: &Path, adhoc_root: &Path) -> CleanupOutcome;
pub(crate) enum CleanupOutcome { Removed, NotThrowaway, Missing, Failed }
```

## 7. Prefix-deps service: `app/services/prefix_deps/` (B2)

```rust
pub const PREFIX_DEPS_INSTALL_JOB_KIND: &str = "prefix-deps-install";
pub const PREFIX_REPAIR_JOB_KIND: &str = "prefix-version-repair";
pub const PREFIX_RECOVERY_JOB_KIND: &str = "prefix-version-recovery";
pub const MSG_OFFLINE: &str = "offline mode is enabled";
pub const MSG_NO_TOOL_CHECK: &str = "No winetricks or protontricks binary found. Install winetricks or configure the path in Settings.";
pub const MSG_NO_TOOL_INSTALL: &str = "No winetricks or protontricks binary found.";
pub const MSG_APP_ID_REQUIRED: &str = "protontricks requires a valid nonzero Steam App ID; configure winetricks or set steam.app_id";
pub const MSG_REPAIR_REQUIRED: &str = "This prefix requires Windows version repair before installing dependencies.";
pub fn msg_already_installing(prefix: &str) -> String;   // "installation already in progress for prefix: {prefix}"

// types.rs   FROZEN wire (moved from tauri/commands/prefix_deps.rs; additive fields only)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PrefixVersionRepairStatus { pub required: bool, pub state: Option<PrefixVersionRestoreState>, pub last_error: Option<String>,
    pub in_progress: bool, pub last_recovery: Option<LastRecovery> }
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)] pub struct LastRecovery { pub outcome: RecoveryOutcome, pub message: Option<String> }
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)] #[serde(rename_all = "snake_case")] pub enum RecoveryOutcome { Restored, Failed }
#[derive(Debug, Clone, Serialize, Deserialize)] pub struct DepLogPayload { pub profile_name: String, pub prefix_path: String, pub line: String }
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DepCompletePayload { pub profile_name: String, pub prefix_path: String, pub succeeded: bool, pub exit_code: Option<i32>,
    pub install_succeeded: bool, pub install_exit_code: Option<i32>, pub restore_state: PrefixVersionRestoreState, pub restore_error: Option<String> }
pub fn completion_payload(profile_name: impl Into<String>, raw_prefix_path: impl Into<String>, outcome: PrefixDependencyInstallOutcome) -> DepCompletePayload;

// tools.rs   FROZEN
pub struct ResolvedTool { pub binary_path: String, pub tool: PrefixDepsTool, pub steam_app_id: Option<String> }   // app id None for winetricks
#[derive(Debug, Clone, Copy, PartialEq, Eq)] pub enum ToolSelectionError { NotFound, ProtontricksNeedsAppId }
pub fn resolve_tool(detection: &BinaryDetectionResult, steam_app_id: Option<&str>, winetricks_on_path: impl FnOnce() -> Option<String>) -> Result<ResolvedTool, ToolSelectionError>;
pub fn profile_steam_app_id(effective: &GameProfile) -> Option<String>;   // steam.app_id → runtime.steam_app_id via parse_nonzero_steam_app_id

// seams.rs (pub(crate) traits; Core* defaults)
pub(crate) trait ToolProbe: Send + Sync + 'static { fn detect(&self, settings_path: &str) -> BinaryDetectionResult; fn winetricks_on_path(&self) -> Option<String>; }
pub(crate) trait JournalStore: Send + Sync + 'static {
    fn is_available(&self) -> bool;
    fn load(&self, prefix: &str) -> Result<Option<PrefixVersionRestoreJournalRow>, MetadataStoreError>;
    fn list(&self) -> Result<Vec<PrefixVersionRestoreJournalRow>, MetadataStoreError>;
    fn upsert_pending(&self, row: &PrefixVersionRestoreJournalRow) -> Result<(), MetadataStoreError>;
    fn mark_failed(&self, prefix: &str, error: &str) -> Result<(), MetadataStoreError>;
    fn delete(&self, prefix: &str) -> Result<(), MetadataStoreError>;
}
pub(crate) trait PrefixVersionRestorer: Send + Sync + 'static { fn restore(&self, plan: PrefixVersionRestorePlan) -> BoxFuture<'static, PrefixVersionRestoreOutcome>; }
pub(crate) trait PrefixInUse: Send + Sync + 'static { fn in_use(&self, canonical_prefix: &Path) -> bool; }   // core: !processes_using_prefix().is_empty()
pub(crate) struct CoreToolProbe; pub(crate) struct MetadataJournalStore(MetadataStore); pub(crate) struct CoreRestorer; pub(crate) struct ProcPrefixInUse;

// journal.rs   FROZEN
#[derive(Debug, Clone, PartialEq, Eq)] pub(crate) enum JournalDecision { Delete, MarkFailed(String), NoMutation }
pub(crate) fn decide(state: PrefixVersionRestoreState, error: Option<&str>) -> JournalDecision;   // Failed→MarkFailed(err or "Windows version restoration failed")
pub(crate) enum PendingOrFailed { Pending, Failed }
pub(crate) struct JournalEntry { pub(crate) plan: PrefixVersionRestorePlan, pub(crate) state: PendingOrFailed, pub(crate) last_error: Option<String> }
/// tool, state, verb (validate_protontricks_verbs + is_windows_version_verb), protontricks app id,
/// binary_path: bare "winetricks"/"protontricks" or absolute + normalized_path_is_executable_file_on_host (M4).
impl TryFrom<PrefixVersionRestoreJournalRow> for JournalEntry { type Error = String; }
#[must_use = "resolve the ticket after the install child exits"] pub(crate) struct JournalTicket { prefix: String }
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum RecoveryResult { Clean, Restored, RestoreFailed { error: String }, CleanupFailed { error: String }, Corrupt { error: String }, SkippedFailed, SkippedInUse, SkippedBusy, StoreUnavailable }
#[derive(Clone)] pub(crate) struct PrefixVersionJournal { store: Arc<dyn JournalStore>, restorer: Arc<dyn PrefixVersionRestorer>, in_use: Arc<dyn PrefixInUse> }
impl PrefixVersionJournal {
    pub(crate) fn new(store: Arc<dyn JournalStore>, restorer: Arc<dyn PrefixVersionRestorer>, in_use: Arc<dyn PrefixInUse>) -> Self;
    pub(crate) fn status(&self, canonical: &str) -> Result<PrefixVersionRepairStatus, AppError>;      // in_progress/last_recovery filled by service
    pub(crate) fn record(&self, plan: &PrefixVersionRestorePlan, profile_id: &str) -> Result<JournalTicket, AppError>;   // I1
    pub(crate) fn resolve(&self, ticket: JournalTicket, outcome: PrefixVersionRestoreOutcome) -> PrefixVersionRestoreOutcome;   // I3: delete fail → mark_failed("…cleanup failed: …") → Failed
    /// CALLER HOLDS the prefix key. include_failed=false skips `failed`; invalid entry → mark_failed, never executes;
    /// startup (check_in_use=true) + live process → SkippedInUse (row stays pending).
    pub(crate) async fn recover(&self, canonical: &str, include_failed: bool, check_in_use: bool) -> RecoveryResult;
    pub(crate) async fn recover_before_install(&self, canonical: &str) -> Result<RecoveryResult, AppError>;   // Clean|Restored → Ok; else Err(Conflict, MSG_REPAIR_REQUIRED)
}

// service struct + API
#[derive(Clone)]
pub struct PrefixDepsService { settings: SettingsStore, profiles: ProfileStore, metadata: MetadataStore, jobs: JobRegistry,
    journal: PrefixVersionJournal, tools: Arc<dyn ToolProbe>, recoveries: RecoveryLedger, log_dir: PathBuf }
#[derive(Clone, Default)] pub(crate) struct RecoveryLedger(Arc<Mutex<HashMap<String, LastRecovery>>>);
pub struct DepInstallCompletion { pub outcome: PrefixDependencyInstallOutcome }
impl PrefixDepsService {
    pub fn new(settings: SettingsStore, profiles: ProfileStore, metadata: MetadataStore, jobs: JobRegistry) -> Self;
    #[cfg(test)] pub(crate) fn with_seams(settings: SettingsStore, profiles: ProfileStore, metadata: MetadataStore, jobs: JobRegistry,
        store: Arc<dyn JournalStore>, restorer: Arc<dyn PrefixVersionRestorer>, in_use: Arc<dyn PrefixInUse>, tools: Arc<dyn ToolProbe>, log_dir: PathBuf) -> Self;
    pub async fn detect_binary(&self) -> Result<BinaryDetectionResult, AppError>;
    pub async fn check(&self, profile_name: String, prefix_path: String, packages: Vec<String>) -> Result<Vec<PrefixDependencyStatus>, AppError>;   // no key
    /// offline → Unavailable MSG_OFFLINE (no job); tool/verbs/normalize sync; job keyed prefix:<canonical>:
    /// recover_before_install → lookup_profile_id → prepare → record → spawn(run_child Piped, sanitized sink)
    /// → restore on its own token (not job token; I8) → dep states → resolve → completion.
    pub async fn start_install(&self, profile_name: String, prefix_path: String, packages: Vec<String>) -> Result<ProcessRun<DepInstallCompletion>, AppError>;
    pub async fn repair_status(&self, prefix_path: String) -> Result<PrefixVersionRepairStatus, AppError>;
    pub async fn repair(&self, prefix_path: String) -> Result<PrefixVersionRepairStatus, AppError>;   // keyed job; recover(include_failed=true)
    pub async fn dependency_status(&self, profile_name: String, prefix_path: String) -> Result<Vec<PrefixDependencyStatus>, AppError>;
    /// store unavailable → []; pending rows only; each in a keyed recovery job (Conflict → SkippedBusy); records RecoveryLedger.
    pub async fn recover_on_startup(&self) -> Vec<(String, RecoveryResult)>;
}
```

## 8. Wiring

```rust
// app/services/mod.rs: pub mod install; pub mod prefix_deps; pub(crate) mod process (pub use process::{JobOwner, ProcessRun, RunEvent});
//   pub mod run_executable; pub mod update; pub use {InstallService, PrefixDepsService, RunExecutableService, UpdateService}
// app/lib.rs pub use services::{…the four…}
// app/bootstrap/stores.rs Services { pub install: InstallService, pub update: UpdateService, pub run_executable: RunExecutableService, pub prefix_deps: PrefixDepsService }
// app/bootstrap/options.rs
#[derive(Debug, Clone, Copy)] pub enum PrefixJournalRecovery { Enabled, Skip { reason: &'static str } }
pub struct BootstrapOptions { …, pub prefix_journal_recovery: PrefixJournalRecovery }   // default Enabled
impl BootstrapOptions { pub fn with_prefix_journal_recovery(mut self, r: PrefixJournalRecovery) -> Self; }
// app/bootstrap/scans.rs
fn schedule_prefix_journal_recovery(app: &App, stores: &Stores, services: &Services, policy: PrefixJournalRecovery);   // job "prefix-journal-recovery"; skip if !metadata.is_available() or Skip
```

## 9. Tauri adapters (B3); command names and arg keys FROZEN

```rust
// tauri/commands/process_relay.rs (new)
pub(crate) fn batch_lines(batch: LogBatch) -> Vec<String>;   // Lines → non-empty lines; Dropped/Truncated → one "[crosshook] …" notice line
// install.rs
install_default_prefix_path(profile_name: String, service) -> Result<String, String>
validate_install_request(request: InstallGameRequest, service) -> Result<(), String>
install_game(request: InstallGameRequest, service) -> Result<InstallGameResult, String>   // drains ProcessRun to Finished
// update.rs
validate_update_request(request, service) -> Result<(), String>
update_game(app: AppHandle, webview: Webview, request, service) -> Result<UpdateGameResponse, String>
cancel_update(webview: Webview, service) -> Result<(), String>
// run_executable.rs (RunCommandError kept verbatim: tag="kind", Validation{variant, field, message} | Runtime{message})
validate_run_executable_request(request, service) -> Result<(), RunCommandError>
run_executable(app, webview, request, service) -> Result<RunExecutableResponse, RunCommandError>
cancel_run_executable(webview, service) -> Result<(), RunCommandError>
stop_run_executable(webview, service) -> Result<(), RunCommandError>
impl From<RunExecutableStartError> for RunCommandError
// prefix_deps.rs
detect_protontricks_binary(service) -> Result<BinaryDetectionResult, String>   // now async
check_prefix_dependencies(profile_name, prefix_path, packages, service) -> Result<Vec<PrefixDependencyStatus>, String>
install_prefix_dependency(app, profile_name, prefix_path, packages, service) -> Result<(), String>
get_prefix_version_repair_status(profile_name, prefix_path, service) -> Result<PrefixVersionRepairStatus, String>   // now async
repair_prefix_windows_version(profile_name, prefix_path, service) -> Result<PrefixVersionRepairStatus, String>
get_dependency_status(profile_name, prefix_path, service) -> Result<Vec<PrefixDependencyStatus>, String>              // now async
```

## 10. Event payloads (wire)

| Event                     | Payload                                                                   | Status                  |
| ------------------------- | ------------------------------------------------------------------------- | ----------------------- |
| `update-log`              | `string`                                                                  | FROZEN, unchanged       |
| `update-complete`         | `number \| null` (exit code; `null` = signal)                             | FROZEN, unchanged       |
| `run-executable-log`      | `{ run_id: string, line: string }`                                        | FROZEN, new shape (D10) |
| `run-executable-complete` | `{ run_id: string, code: number \| null, signal: number \| null }`        | FROZEN, new shape (D10) |
| `prefix-dep-log`          | `DepLogPayload` (raw `profile_name`, raw `prefix_path`, sanitized `line`) | FROZEN, unchanged       |
| `prefix-dep-complete`     | `DepCompletePayload`                                                      | FROZEN, unchanged       |
| `install-*`               | none (install has no events)                                              | unchanged               |

## 11. TypeScript (B3)

```ts
// src/types/run-executable.ts
export interface RunExecutableResult { succeeded; message; helper_log_path; resolved_prefix_path; run_id: string }
export interface RunExecutableLogPayload { run_id: string; line: string }
export interface RunExecutableCompletePayload { run_id: string; code: number | null; signal: number | null }
// src/types/update.ts
export interface UpdateGameResult { succeeded; message; helper_log_path; run_id: string }
// src/types/prefix-deps.ts
export type PrefixVersionRecoveryOutcome = 'restored' | 'failed';
export interface PrefixVersionLastRecovery { outcome: PrefixVersionRecoveryOutcome; message: string | null }
export interface PrefixVersionRepairStatus { required; state; last_error; in_progress: boolean; last_recovery: PrefixVersionLastRecovery | null }
// src/hooks/useRunExecutable.ts
export interface UseRunExecutableResult { …existing; logLines: string[] }   // cap RUN_LOG_LINE_CAP = 500
```
