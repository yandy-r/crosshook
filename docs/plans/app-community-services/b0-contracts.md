# YAN-835 B0 frozen contracts

Output of the planning phase. B1/B2 land these as real code; B3 cuts the Tauri adapters over. **No task may edit a file owned by another task in the same batch.** If a contract is wrong, message the integration owner; do not patch it from inside a slice. Paths are relative to `src/crosshook-native/`; `core/` = `crates/crosshook-core/src`, `app/` = `crates/crosshook-app/src`, `tauri/` = `src-tauri/src`. All types named below already exist unless marked **(new)**. `file:line` refs were verified at plan time. Binding decisions: [decisions.md](./decisions.md).

## 0. Dependency finding (`async-trait`)

`async-trait = "0.1"` is already a dependency of `crosshook-core` (`crates/crosshook-core/Cargo.toml:42`), but not of `crosshook-app`. Per integration-owner direction, use the **explicit boxed-future signature below**, not a macro, and add no dependencies anywhere. It remains object-safe (`&dyn TapGitRunner`); app/tests implement it with `Box::pin(async move { … })`.

## 1. Core: containment — `core/fs_util.rs` (task 1.1)

Existing imports at `fs_util.rs:3-5` (`fs`, `io`, `Component`, `Path`, `PathBuf`) are reused; add `std::fmt`.

```rust
/// Why a candidate path was refused. `Display` never contains a path.
#[derive(Debug)]
pub enum WorkspacePathError {
    InvalidRoot,                 // root is not an existing directory / cannot be canonicalized
    Traversal,                   // candidate has a Component::ParentDir (even if it ends up inside)
    OutsideWorkspace,            // absolute candidate not component-prefixed by root, or resolved outside
    NotFound,                    // target (or dangling final symlink) does not exist
    Io { operation: &'static str, source: std::io::Error }, // permission, ELOOP, ENOTDIR, ...
}
impl std::fmt::Display for WorkspacePathError { /* fixed strings; Io => "failed to {operation}: {kind}" */ }
impl std::error::Error for WorkspacePathError { /* source() for Io */ }

/// Resolves `candidate` against `root` and returns the canonical, contained path.
pub fn ensure_within_workspace(root: &Path, candidate: &Path) -> Result<PathBuf, WorkspacePathError>;
```

Semantics (D3, in this order): (1) `root.canonicalize()` once, must be a dir else `InvalidRoot`; (2) any `Component::ParentDir` in `candidate` → `Traversal`; empty candidate → `OutsideWorkspace`; (3) relative candidate is joined to the **canonical root**, never CWD; an absolute candidate must `strip_prefix` (component-wise) the supplied **or** canonical root spelling, else `OutsideWorkspace`; (4) `fs::canonicalize` the joined target (`ErrorKind::NotFound` → `NotFound`, other errors → `Io`), require `canonical.starts_with(&canonical_root)` (component-wise `OsStr` bytes; no `display()`, no lossy conversion) else `OutsideWorkspace`; (5) in-root intermediate/final symlinks accepted only when the resolved target stays inside; a dangling final symlink canonicalizes to `NotFound` and is rejected; (6) no case folding, no Unicode normalization. Candidate == root returns the canonical root (callers must still require a regular file). The helper does not handle missing candidates.

Walk each existing relative prefix using symlink_metadata/canonicalize as well: an intermediate link that resolves outside must fail at that prefix even if a later link would lead back in (research-security §2). Require intermediate resolved components to be directories. No absent-prefix allowance; any missing candidate/prefix fails. The final canonical starts_with check remains mandatory. This is a point-in-time check, not handle-pinned/race-free open (D11).

## 2. Core: indexer and Lutris scanner containment (task 1.1)

- `core/community/index.rs` `collect_manifests` (:106, `path.is_dir()` at :124-133): iterate with `entry.file_type()` (does not follow links); **skip every symlink entry**, file or directory (`tracing::warn!`, no path in new message beyond what the module already logs); recurse only into real directories; read only real files. Signatures of `index_taps` (:63), `index_tap` (:81) and `CommunityProfileIndexError` (:32) unchanged.
- `core/profile/lutris_import/paths.rs` (D4d): `list_game_configs(root: &Path) -> Vec<PathBuf>` (:33) and `read_pga_games(root: &Path) -> Result<Vec<PgaGame>, LutrisImportError>` (:89) keep their signatures. Validate `games/` and nested directories before `read_dir`, then each YAML candidate with `ensure_within_workspace(root, &candidate)`; return resolved candidates and skip failures with a diagnostic. For `pga.db`, validate before opening; map failure to `LutrisImportError::Pga { message: "Lutris database path could not be resolved inside the library root".into() }`. The preview already skips the database on error and appends its diagnostic (`import.rs:34-40`); missing-database behavior therefore remains fail-soft. Keep entry cap and YAML size limit unchanged; game executable/prefix paths are not contained to this library root.

## 3. Core: async tap-sync driver — `core/community/taps/runner.rs` **(new)**, `git.rs`, `store.rs`, `types.rs` (task 1.2)

```rust
// runner.rs
use std::{collections::BTreeMap, ffi::OsString, future::Future, path::PathBuf, pin::Pin};

/// One git process to run. Core owns argv and env; the runner only executes it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitInvocation {
    pub action: &'static str,                 // "clone community tap", "fetch community tap", ...
    pub args: Vec<OsString>,                  // argv after the program name ("git")
    pub env: BTreeMap<String, String>,        // git_security_env_pairs() (git.rs:14)
    pub directory: Option<PathBuf>,           // working directory (None for clone)
}
/// Captured result of a finished git process. Non-zero exit is `success == false`, not an error.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitOutput { pub success: bool, pub stdout: String, pub stderr: String }
#[derive(Debug)]
pub enum GitRunError { Cancelled, Failed(std::io::Error) } // spawn/wait/I-O failure vs. cancellation

pub trait TapGitRunner: Send + Sync {
    fn run<'a>(&'a self, invocation: GitInvocation)
        -> Pin<Box<dyn Future<Output = Result<GitOutput, GitRunError>> + Send + 'a>>;
    /// Polled by the driver before every step and before indexing.
    fn is_cancelled(&self) -> bool { false }
}
```

```rust
// types.rs: CommunityTapError (:82-98) gains one variant; Display "community tap sync was cancelled".
pub enum CommunityTapError { /* existing 7 variants */ Cancelled }

// git.rs: replaces git_command (:24), clone_tap (:38), fetch_and_reset (:70), fetch_and_checkout_pinned (:85),
// checkout_pinned_commit (:98), rev_parse_head (:125), run_git (:148). git_security_env_pairs (:14) stays.
pub(super) fn clone_invocation(ws: &CommunityTapWorkspace) -> GitInvocation;  // clone --branch B --single-branch -- URL PATH
pub(super) fn fetch_invocation(ws: &CommunityTapWorkspace) -> GitInvocation;  // fetch --prune origin -- B, dir = local_path
pub(super) fn reset_invocation(ws: &CommunityTapWorkspace) -> GitInvocation;  // reset --hard FETCH_HEAD
pub(super) fn clean_invocation(ws: &CommunityTapWorkspace) -> GitInvocation;  // clean -fdx
pub(super) fn checkout_invocation(ws: &CommunityTapWorkspace) -> Result<GitInvocation, CommunityTapError>; // is_valid_git_sha (validation.rs:7) first
pub(super) fn rev_parse_invocation(path: &Path) -> GitInvocation;              // rev-parse HEAD, dir = path
pub(super) fn git_failure(invocation: &GitInvocation, output: &GitOutput) -> CommunityTapError; // CommunityTapError::Git

// store.rs: preserve sync_tap (:57), sync_many (:65), sync_workspace (:93) through B2 for the existing Tauri caller.
impl CommunityTapStore {
    pub async fn sync_many_with_runner(&self, subscriptions: &[CommunityTapSubscription], runner: std::sync::Arc<dyn TapGitRunner>)
        -> Result<Vec<CommunityTapSyncResult>, CommunityTapError>;   // first per-tap hard error aborts (D8)
    pub async fn sync_tap_with_runner(&self, subscription: &CommunityTapSubscription, runner: std::sync::Arc<dyn TapGitRunner>)
        -> Result<CommunityTapSyncResult, CommunityTapError>;        // resolve_workspace (:44) then sync_workspace_with_runner
    pub(crate) async fn sync_workspace_with_runner(&self, workspace: &CommunityTapWorkspace, runner: std::sync::Arc<dyn TapGitRunner>)
        -> Result<CommunityTapSyncResult, CommunityTapError>;        // owned clone can enter spawn_blocking for pre-index cancellation
    /// normalize_subscription (validation.rs:52) plus pinned-commit check via is_valid_git_sha; extras cleared.
    pub fn normalize_and_validate(subscription: &CommunityTapSubscription) -> Result<CommunityTapSubscription, CommunityTapError>;
}
```

Intermediate-build rule: B1 keeps synchronous APIs for the existing caller, refactoring them onto the same git invocation builders (a private std runner through the existing host gateway); it adds the async APIs alongside them. The old `git.rs` helpers become thin builder consumers, not a second argv policy. B3 task 3.1 deletes those synchronous APIs and helpers after converting their sole production caller (`tauri/commands/community.rs:226`) and migrating core tests; **no legacy wrapper survives the final cutover**. This deliberate sequential ownership also keeps B1's workspace clippy gate compilable.

Driver semantics (a faithful port of `sync_workspace`): `tokio::fs::create_dir_all(base_path)`; existing workspace (`tokio::fs::try_exists`) → pinned? `fetch` → `checkout` → `clean`, else `fetch` → `reset` → `clean`; any non-success output or `GitRunError::Failed` → `CachedFallback`/`from_cache = true` when the workspace still exists, else `Err`; absent workspace → `clone` (`--` before URL) then pinned `checkout`; hard errors on clone/rev-parse. **`GitRunError::Cancelled` and `runner.is_cancelled()` → `Err(CommunityTapError::Cancelled)` immediately, never `CachedFallback`.** After `rev-parse` the file index (`index::index_tap`, blocking) runs in `tokio::task::spawn_blocking`; `last_sync_at` stays `None`. Re-export `GitInvocation, GitOutput, GitRunError, TapGitRunner` from `taps/mod.rs` and `core/community/mod.rs:9-12`.

`normalize_and_validate` is a public associated function so app can reuse core normalization without exposing the private `validation` module. Move pin validation into `normalize_subscription` as well, so `resolve_workspace` rejects invalid persisted pins before git. Driver takes an owned `Arc<dyn TapGitRunner>` (not a borrowed trait object) so a clone can enter the `'static` blocking index closure: check `runner.is_cancelled()` inside that closure before `index_tap`, not only before spawning it. Map a join failure to a fixed `CommunityTapError::Io` source (never its panic payload).

> **Final shape diverges (pre-PR review):** pin validation was reverted out of `normalize_subscription`. Otherwise one stored bad pin breaks listing, import and the catalog for every tap. The app's `add_tap` validates the incoming pin with the re-exported `crosshook_core::community::is_valid_git_sha`, and checkout-time validation in `git.rs` stays.

## 4. App: errors — `app/error.rs` (task 3.1; one owner)

Added after `From<MetadataStoreError>` (:226); `AppError::new` (:36) still sanitizes.

```rust
impl From<crosshook_core::fs_util::WorkspacePathError> for AppError {
    fn from(error: crosshook_core::fs_util::WorkspacePathError) -> Self;
}
// Traversal | OutsideWorkspace | InvalidRoot -> Validation { field: "path" }, "path is not inside the allowed workspace"
// NotFound -> NotFound "path was not found"; Io -> Io "failed to resolve path"
impl From<crosshook_core::community::CommunityTapError> for AppError {
    fn from(error: crosshook_core::community::CommunityTapError) -> Self;
}
// EmptyTapUrl | InvalidTapUrl | InvalidBranch | InvalidPinnedCommit -> Validation { field: "tap" }, error.to_string() (verbatim, user input only)
// Io | Git -> Io "failed to sync community tap"; Index -> Io "failed to index community taps"; Cancelled -> Cancelled
impl From<crosshook_core::profile::LutrisImportError> for AppError {
    fn from(error: crosshook_core::profile::LutrisImportError) -> Self;
}
// Io -> Io "failed to read Lutris library"; Yaml -> Validation { field: "lutris" } "failed to parse Lutris config";
// Pga -> Io "failed to read Lutris pga.db"; ProfileStore { .. } -> Io "failed to access profiles"
```

Community-specific text ("community profile path is not inside a known tap workspace") is built in `services/community/containment.rs`, not in the generic mapper (D9).

## 5. App services (all `async -> Result<T, AppError>`; blocking work in one `run_blocking`, `app/services/mod.rs:61`)

Existing type imports used in the service signatures (not new DTOs):

```rust
use std::{collections::HashMap, path::{Path, PathBuf}, sync::Arc};
use crosshook_core::community::{CommunityProfileIndex, CommunityTapStore, CommunityTapSubscription,
    CommunityTapSyncResult, CommunityTapWorkspace, GitInvocation, GitOutput, GitRunError, TapGitRunner};
use crosshook_core::metadata::{CommunityProfileRow, MetadataStore};
use crosshook_core::profile::{CommunityExportResult, CommunityImportPreview, CommunityImportResult,
    LutrisImportEntry, LutrisImportPreview, LutrisImportResult, ProfileStore};
use crosshook_core::settings::SettingsStore;
use crosshook_core::discovery::{CatalogPage, CatalogQuery, ExternalTrainerSearchQuery,
    ExternalTrainerSearchResponse, ExternalTrainerSourceSubscription, VersionMatchResult};
use crosshook_core::protondb::{ProtonDbLookupResult, ProtonDbSuggestionSet};
use tokio_util::sync::CancellationToken;
use crate::{AppError, services::ProfileWriter, jobs::{JobContext, JobHandle, JobId, JobRegistry}};
```

Verified export sites: `core/community/mod.rs:7-13`, `core/profile/mod.rs:38-45`, `core/metadata/mod.rs:140`, `core/discovery/mod.rs:8-14`, `core/protondb/mod.rs:8-18`, `app/jobs.rs:17-24`; runner types are the explicitly new re-exports above. Give every public item rustdoc (`crosshook-app` denies missing docs); all four service structs assert `Send + Sync`.

```rust
// services/community/mod.rs — task 2.1 owns the complete service; task 3.1 registers its module.
pub const COMMUNITY_SYNC_JOB_KIND: &str = "community-sync"; // JobSpec::new(kind) with NO key => non-exclusive
pub(crate) type RunnerFactory = Arc<dyn Fn(CancellationToken) -> Arc<dyn TapGitRunner> + Send + Sync>; // tokio_util::sync
#[derive(Clone)]
pub struct CommunityService {
    settings: SettingsStore, taps: CommunityTapStore, profiles: ProfileStore, metadata: MetadataStore,
    writer: ProfileWriter, jobs: JobRegistry,
    sync_lock: Arc<tokio::sync::Mutex<()>>,                                   // overlapping syncs queue (D6)
    sync_results: Arc<std::sync::Mutex<HashMap<JobId, Vec<CommunityTapSyncResult>>>>, // slot keyed by JobId (storage pattern)
    runner_factory: RunnerFactory,
}
impl CommunityService {
    pub fn new(settings: SettingsStore, taps: CommunityTapStore, profiles: ProfileStore, metadata: MetadataStore,
               writer: ProfileWriter, jobs: JobRegistry) -> Self;           // initializes fields with default_runner_factory()
    #[cfg(test)] pub(crate) fn with_runner_factory(settings: SettingsStore, taps: CommunityTapStore,
        profiles: ProfileStore, metadata: MetadataStore, writer: ProfileWriter, jobs: JobRegistry,
        runner_factory: RunnerFactory) -> Self;
    #[cfg(test)] pub(crate) fn jobs(&self) -> &JobRegistry;
}
// taps.rs (2.1), methods in impl CommunityService
impl CommunityService {
pub async fn add_tap(&self, tap: CommunityTapSubscription) -> Result<Vec<CommunityTapSubscription>, AppError>;
pub async fn remove_tap(&self, tap: CommunityTapSubscription) -> Result<Vec<CommunityTapSubscription>, AppError>; // idempotent, service-only
pub async fn list_profiles(&self) -> Result<CommunityProfileIndex, AppError>;
pub async fn list_indexed_profiles(&self) -> Result<Vec<CommunityProfileRow>, AppError>; // metadata.list_community_tap_profiles(None) (community_ops.rs:72)
// import.rs (2.1), methods in impl CommunityService
pub async fn export_profile(&self, profile_name: String, output_path: String) -> Result<CommunityExportResult, AppError>;
pub async fn prepare_import(&self, path: PathBuf) -> Result<CommunityImportPreview, AppError>;
pub async fn import_profile(&self, path: PathBuf) -> Result<CommunityImportResult, AppError>;
// sync.rs (2.1), methods in impl CommunityService; lifecycle getters are deliberately synchronous
pub fn start_sync(&self) -> Result<JobHandle, AppError>;
pub async fn sync(&self) -> Result<Vec<CommunityTapSyncResult>, AppError>;   // start, await Finished, take slot, reap
pub fn sync_result(&self, id: &JobId) -> Result<Option<Vec<CommunityTapSyncResult>>, AppError>; // take-once
}
```

`containment.rs` (2.1, private): `fn contain_in_taps(workspaces: &[CommunityTapWorkspace], path: &Path) -> Result<PathBuf, AppError>` — first successful `ensure_within_workspace(&ws.local_path, path)` wins; reject traversal as `Validation { field: "path" }`, missing candidates as `NotFound`, other unmatched paths as `Validation { field: "path" }` with "community profile path is not inside a known tap workspace". Load settings and resolve workspaces with `CommunityTapStore::resolve_workspace` (:44) in the blocking closure. `fn require_regular_json(path: &Path) -> Result<(), AppError>` checks `is_file()` and case-insensitive `json`, using fixed path-free validation messages. `import_profile`: contain → regular-json → `writer.import_community(resolved)` (`profiles/imports.rs:47`) → `ProfileCommit::into_output` (`profiles/types.rs:113`). `prepare_import` (D4b): canonicalize → regular-json → classify the original candidate against resolved tap roots for debug logging only (no containment rejection) → `preview_community_profile_import(&canonical)` (`core/profile/exchange/import.rs:38`). Do not choose candidate.parent() as an import authority.

`sync.rs` `async fn run_sync(&self, ctx: &JobContext) -> Result<Vec<CommunityTapSyncResult>, AppError>`: acquire `sync_lock` cancellation-aware; load settings through `run_blocking`; `offline_mode` returns `Unavailable` "offline mode is enabled" before constructing/running git; call `taps.sync_many_with_runner` with `(self.runner_factory)(ctx.cancel_token().clone())`. After all taps succeed, call `persist.rs` `fn finalize_sync_results(metadata: &MetadataStore, results: Vec<CommunityTapSyncResult>, ctx: &JobContext) -> Result<Vec<CommunityTapSyncResult>, AppError>` through `run_blocking`. Check `ctx.is_cancelled()` before every index, offline upsert and read-back phase; cancellation prevents subsequent phases. Preserve the two-loop order from `tauri/commands/community.rs:228-256`: each index → offline upsert (warn-only), then timestamp lookups and extra clearing. `start_sync` uses `jobs.start(JobSpec::new(COMMUNITY_SYNC_JOB_KIND), …)` with no key; success stores the result under `ctx.id()` before `Outcome::Succeeded`, failure returns `Outcome::Failed`, cancellation returns `Outcome::Cancelled` without a slot. `sync_result` takes only a finished slot, returns `None` for running/failed/cancelled/already-read and `NotFound` for unknown ids. Prune evicted slots on start; `sync()` consumes Finished, takes result and reaps on every outcome. Copy storage's abandonment/result guard (`storage/scan.rs:21-44`) without its arbitrary timeout; a dropped waiter must not leak its slot. Cancellation is cooperative at blocking phase boundaries, not rollback of earlier completed writes.

`runner.rs` (2.1): `pub(super) struct SupervisedGitRunner { cancel: CancellationToken }`; `pub(super) fn default_runner_factory() -> RunnerFactory`. `run` prechecks cancellation, rejects a non-UTF-8 `directory` rather than dropping it, then uses `platform::host_command_with_env_and_directory_lifecycle("git", &inv.env, directory, &BTreeMap::new())` (`core/platform/gateway.rs:110`, returns `tokio::process::Command`) → `.args(&inv.args).stdin(null).stdout(piped).stderr(piped).process_group(0).kill_on_drop(true)` → spawn. Drain both pipes concurrently to EOF, retaining at most 64 KiB each (do not stop draining at the cap), and wait through `jobs::supervise_child(child, &self.cancel, DEFAULT_KILL_GRACE)` (`jobs/process.rs:84,17`). `ChildExit::was_cancelled()` ⇒ `GitRunError::Cancelled`; otherwise `GitOutput { success: status.success(), .. }`. Spawn/wait/read failures ⇒ `Failed(std::io::Error)`. Share the execution helper with a test-only program seam for the controlled sleep child; production program is always git. No new deps (`tokio` process/io-util and tokio-util are already in app).

```rust
// services/lutris/mod.rs (task 1.4)
#[derive(Clone)] pub struct LutrisService { profiles: ProfileStore, writer: ProfileWriter }
impl LutrisService {
    pub fn new(profiles: ProfileStore, writer: ProfileWriter) -> Self;
    pub async fn prepare_import(&self, directory: Option<PathBuf>) -> Result<LutrisImportPreview, AppError>; // preview_lutris_import(directory, Some(&profiles)) (import.rs:15)
    pub async fn import_profiles(&self, entries: Vec<LutrisImportEntry>) -> Result<LutrisImportResult, AppError>; // writer.import_lutris(..).into_output()
}
// services/discovery/mod.rs (task 1.3)
#[derive(Clone)] pub struct DiscoveryService { settings: SettingsStore, metadata: MetadataStore, taps: CommunityTapStore }
impl DiscoveryService {
    pub fn new(settings: SettingsStore, metadata: MetadataStore, taps: CommunityTapStore) -> Self;
    pub async fn catalog(&self, query: CatalogQuery) -> Result<CatalogPage, AppError>; // query_community_catalog (catalog.rs:18); ANY Err -> degraded_catalog_from_taps (catalog.rs:218)
    pub async fn search_external(&self, query: ExternalTrainerSearchQuery) -> Result<ExternalTrainerSearchResponse, AppError>;
    pub async fn check_version_compatibility(&self, profile_name: String, trainer_game_version: Option<String>) -> Result<VersionMatchResult, AppError>;
    pub async fn list_external_sources(&self) -> Result<Vec<ExternalTrainerSourceSubscription>, AppError>;
    pub async fn add_external_source(&self, source: ExternalTrainerSourceSubscription) -> Result<Vec<ExternalTrainerSourceSubscription>, AppError>;
    pub async fn remove_external_source(&self, source_id: String) -> Result<Vec<ExternalTrainerSourceSubscription>, AppError>;
}
// services/protondb/mod.rs (task 1.3)
#[derive(Clone)] pub struct ProtonDbService { metadata: MetadataStore, profiles: ProfileStore }
impl ProtonDbService {
    pub fn new(metadata: MetadataStore, profiles: ProfileStore) -> Self;
    pub async fn lookup(&self, app_id: String, force_refresh: bool) -> Result<ProtonDbLookupResult, AppError>;     // lookup_protondb (client.rs:85)
    pub async fn suggestions(&self, app_id: String, profile_name: String, force_refresh: bool) -> Result<ProtonDbSuggestionSet, AppError>;
    pub async fn dismiss_suggestion(&self, profile_name: String, app_id: String, suggestion_key: String) -> Result<(), AppError>;
}
```

Verbatim texts (D9): `unknown profile: {name}` (`Validation { field: "profile_name" }`); `source with id {id:?} already exists` (`Conflict`); `no source with id {id:?} found` (`NotFound`); `validate_external_source` (`core/discovery/models.rs:155`) strings via `Validation { field: "external_source" }`; `profile_name is required` (`Validation { field: "profile_name" }`); `profile '{name}' has no metadata record — cannot dismiss suggestion` (`NotFound`). `SUGGESTION_DISMISSAL_RETENTION_DAYS: u32 = 30` moves from `tauri/commands/protondb.rs:12-13` into the service. Settings writes use `SettingsStore::update` (`store.rs:136`; returns `Result<Result<T, E>, SettingsStoreError>`), returned lists have `extra` cleared.

**Offline external search (D7)** — core additive API (task 1.3, `core/discovery/client/{search.rs,mod.rs}`, `core/discovery/mod.rs:13`): `pub enum ExternalSearchMode { Online, CacheOnly }` and `pub async fn search_external_trainers_with_mode(metadata_store: &MetadataStore, sources: &[ExternalTrainerSourceSubscription], query: &ExternalTrainerSearchQuery, mode: ExternalSearchMode) -> ExternalTrainerSearchResponse`; existing `search_external_trainers` (`search.rs:19`) remains the Online entry point. CacheOnly never enters stage 2 (`fetch_source`, :149): fresh cache → stale cache → offline response; forceRefresh cannot bypass cache-only policy. Preserve empty query/no enabled sources as empty success. For active searches, app returns `Unavailable` "offline mode is enabled" only when CacheOnly has no decodable cache (`!response.cached`), including disabled metadata. Cached empty results count as cached. Execute the core async search/ProtonDB lookup on a Tokio blocking-pool worker using `Handle::current().block_on(...)` inside `run_blocking`: these core async clients synchronously access SQLite before/after network awaits (`search.rs:159,189`, `protondb/client.rs:85-130`); merely awaiting them on the caller runtime would not satisfy D10. This retains their TTL/key/concurrency/size and stale fallback policies without redesigning either client.

Important search detail: `search.rs:98` currently runs `fetch_source` inside `JoinSet::spawn`, which moves its synchronous cache calls back onto a runtime worker. Task 1.3 must bridge **inside each source task**: capture the runtime handle, invoke `spawn_blocking(move || runtime.block_on(fetch_source(...)))`, and await it under the existing four-source concurrency cap. Keep JoinSet output/aggregation behavior unchanged and handle join failure as the existing failed-source path. The outer app bridge alone is not sufficient. This is limited scheduling extraction, not network/cache policy redesign.

## 6. Wiring — task 3.1 only (`app/bootstrap/stores.rs`, `stores/tests.rs`, `app/services/mod.rs`, `app/lib.rs`, `tauri/lib.rs`)

```rust
// bootstrap/stores.rs: Services (:51) gains four fields; Services::new (:93) keeps its signature.
pub struct Services {
    // All 17 existing fields at bootstrap/stores.rs:51-86 remain unchanged.
    pub community: CommunityService,
    pub lutris: LutrisService,
    pub discovery: DiscoveryService,
    pub protondb: ProtonDbService,
}
impl Services {
    pub fn new(stores: &Stores, events: EventBus, runtime: tokio::runtime::Handle, jobs: JobRegistry) -> Self;
}
```

Constructor additions (within the existing implementation; unchanged fields omitted here):

```rust
// Services::new builds ONE writer first (ProfileWriter::new, currently at :122) and clones it:
let writer = ProfileWriter::new(stores.profiles.clone(), stores.metadata.clone(), stores.settings.clone(), events);
profiles: writer.clone(),
community: CommunityService::new(stores.settings.clone(), stores.community_taps.clone(), stores.profiles.clone(),
                                 stores.metadata.clone(), writer.clone(), jobs.clone()),   // before storage consumes `jobs` (:134)
lutris: LutrisService::new(stores.profiles.clone(), writer.clone()),
discovery: DiscoveryService::new(stores.settings.clone(), stores.metadata.clone(), stores.community_taps.clone()),
protondb: ProtonDbService::new(stores.metadata.clone(), stores.profiles.clone()),
```

`services/mod.rs`: `pub mod community; pub mod discovery; pub mod lutris; pub mod protondb;` plus `pub use community::CommunityService; pub use discovery::DiscoveryService; pub use lutris::LutrisService; pub use protondb::ProtonDbService;`; `lib.rs:30-33` re-exports the four. `tauri/lib.rs` after line 58: `.manage(services.community.clone()) .manage(services.lutris.clone()) .manage(services.discovery.clone()) .manage(services.protondb.clone())` (raw store `.manage()` lines at :37-41 stay). `services_new_builds_bundle_from_stores` (`stores/tests.rs`) additionally asserts the registry is shared: start a job via `services.community.jobs()` and check `jobs.job_counts() == (1, 0)` (`registry.rs:275`).

## 7. Tauri adapters — task 3.1 (names, arg keys and return DTOs frozen, D2)

Every command becomes `pub async fn name(<same args>, service: State<'_, XService>) -> Result<T, String>` returning `service.method(..).await.map_err(|e| e.message().to_string())`; `Option<bool>` is applied as `.unwrap_or(false)` in the adapter; `path`/`directory` `String` → `PathBuf::from`. `protondb_accept_suggestion` keeps `State<'_, ProfileWriter>` (`protondb.rs`). Pins: replace sync fn-pointer casts with name-only `let _ = command_name;` (model `commands/storage.rs:39-50`); add the missing ProtonDB pin for all four commands; rewrite the lutris pins at `commands/export.rs:216-221`; the dedupe test at `commands/community.rs:269-339` moves to `services/community/tests/taps.rs` on the **normalized** triple.
