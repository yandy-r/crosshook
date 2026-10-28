# YAN-822 B0 frozen contracts

Exact Rust surface for the three new services, their seams, ownership, texts and test helpers. Binding decisions: [decisions.md](./decisions.md) D1–D13 (unchanged). Paths relative to `src/crosshook-native/`; `app/` = `crates/crosshook-app/src`, `core/` = `crates/crosshook-core/src`, `tauri/` = `src-tauri/src`. Every cited signature was read in this worktree. **No task edits a file owned by another task in the same batch**; ask the integration owner (Main) if a contract is wrong.

## 0. Verified facts that shape the contracts

- `JobRegistry::start<F,Fut,R>(&self, JobSpec, F: FnOnce(JobContext)->Fut) -> Result<JobHandle, AppError>` (`app/jobs/registry.rs:134`); duplicate `JobKey` → `Conflict`; `cancel(&JobId)`, `outcome`, `reap`, `subscribe` are `JobId`-addressed with no owner; `job_counts() -> (running, finished)` is `#[cfg(test)] pub(crate)` (:275). `JobId::from_string`, `JobId::as_str`, `JobKey::new`, `JobSpec::new(kind).with_key(key)` (`jobs/types.rs`). `JobContext::{id, cancel_token, is_cancelled, cancelled}`. `Outcome::{Succeeded, Failed(AppError), Cancelled}`; `AppErrorKind::{Validation{field}, NotFound, Conflict, Unavailable, Io, Cancelled, Internal}`; `AppError::new` is `pub(crate)`, `AppError::validation` is `pub`, message is sanitized only for `$HOME`/`/home/<user>` (`app/error.rs:37`). `crate::services::run_blocking` is private to `services` but visible to descendants.
- Core: `ProtonUpAvailableVersion`/`ProtonUpInstallRequest`/`ProtonUpInstallResult` have **no `PartialEq`**; `Phase` has **no `PartialEq`** (use `matches!`); `ProgressEmitter` is `Clone` and `emit(phase, bytes_done, bytes_total, message)`; `InstallError` is not `std::error::Error`, its `to_result` is `pub(super)`. `AppSettingsData` has `offline_mode` and `protonup_include_prereleases` but **no Steam client path field** — D7's "configured path from settings" therefore reduces to `SteamService::default_client_install_path()`.
- Core catalog has no public cache-only read (`load_catalog_rows`/`scoped_cache_key` are `pub(crate)`); D9 fallback applies: offline forces `force_refresh=false`, an expired cache may still attempt one live fetch before stale fallback. Filed with D13 follow-ups.
- Core `plan_uninstall`/`resolve_install_root_candidates` read `$HOME` (`pub(crate) resolve_candidates_with` is unreachable from app). Hermetic approach: a fourth app-private seam `ProtonRootSource` (below); tests inject temp roots and pass the trusted temp Steam root as core's `steam_client_install_path`, so core derives `<tmp>/Steam/compatibilitytools.d` regardless of `$HOME`. **No test mutates process env.**
- `sanitize_display_path` in `tauri/commands/shared.rs` is used only by `migration.rs`; it is deleted with the move (`crosshook_app::presenter::display_path_tilde` is the replacement). `uuid` and `tokio-util` are used in `src-tauri` only by `commands/protonup.rs` and are removed from `src-tauri/Cargo.toml`.

## 1. Module map and owners

```
app/services/protonup/
  mod.rs        B1 struct, new, consts, assert_impl_all, B1 `mod` lines | B2.1 appends the batch's `mod` lines (all B2 protonup files, incl. 2.2's)
  types.rs      B1 InstallOwner, ProtonInstallHandle, ProtonUninstall{,Plan}Response, ProtonInstallStart
  seams.rs      B1 ProtonCatalogSource, ProtonInstaller, ProtonRootSource + Core* defaults
  identity.rs   B1 verify_install_identity, IdentityError (pure)
  messages.rs   B1 fixed texts + pure mappers (identity text, uninstall text, InstallError→result, progress text)
  owners.rs     B1 OwnerLedger, OwnerGuard
  progress.rs   B1 next_progress, forward_progress (relay + terminal tracking)
  catalog.rs    B2.1 flags() loader, list_providers, list_available_versions
  roots.rs      B2.1 resolve_install_roots, get_suggestion
  install.rs    B2.1 install_version (legacy), start_install, job body
  cancel.rs     B2.1 cancel_install
  uninstall.rs  B2.2 plan_uninstall, uninstall, root authority
  tests/mod.rs (B1; B2.1 appends mod lines), helpers.rs, identity.rs, messages.rs, owners.rs, progress.rs   B1
  tests/security.rs + security/{identity,cancel}.rs, install.rs, catalog.rs   B2.1
  tests/security/uninstall.rs, tests/uninstall.rs   B2.2
app/services/proton_migration/   mod.rs B1 | check.rs apply.rs tests/{mod,check,apply}.rs B2.3 (mod lines in mod.rs also B2.3)
app/services/umu/                mod.rs seams.rs B1 | refresh.rs coverage.rs tests/{mod,refresh,coverage}.rs B2.4 (mod lines in mod.rs also B2.4)
```

Shared files, B1 task 1.3 only: `app/services/mod.rs`, `app/lib.rs`, `app/bootstrap/stores.rs`, `app/bootstrap/stores/tests.rs`. `app/bootstrap/scans.rs` → B2.4 only; `crates/crosshook-app/tests/profile_writer_golden.rs` + fixtures → Phase 0 (task 0.1) then B2.3. `app/error.rs` is **not touched** (private mappers per service). Tauri files → B3 only.

## 2. Shared aliases

```rust
// app/services/mod.rs (B1 1.3) — one definition for all seams
pub type BoxFuture<'a, T> = std::pin::Pin<Box<dyn std::future::Future<Output = T> + Send + 'a>>;
```

Re-exports added to `app/services/mod.rs`: `pub mod protonup; pub mod proton_migration; pub mod umu;` and `pub use protonup::ProtonUpService; pub use proton_migration::ProtonMigrationService; pub use umu::UmuService;`. `app/lib.rs` `pub use services::{…}` gains the three names.

## 3. protonup

```rust
// types.rs (serde shapes frozen; moved verbatim from tauri/commands/protonup.rs)
#[derive(Debug, Clone, PartialEq, Eq, Hash)] pub struct InstallOwner(String);
impl InstallOwner { pub fn new(label: impl Into<String>) -> Self; }          // adapter passes webview label
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)] pub struct ProtonInstallHandle { pub op_id: String }
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ProtonUninstallResponse { pub success: bool, pub conflicting_app_ids: Vec<String>, pub error_message: Option<String> }
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ProtonUninstallPlanResponse { pub success: bool, pub conflicting_app_ids: Vec<String>, pub error_message: Option<String> }
pub struct ProtonInstallStart { pub op_id: String, pub progress: tokio::sync::broadcast::Receiver<ProtonInstallProgress> }
impl ProtonInstallStart { pub fn handle(&self) -> ProtonInstallHandle; }

// progress.rs
pub async fn next_progress(rx: &mut broadcast::Receiver<ProtonInstallProgress>) -> Option<ProtonInstallProgress>;
//   RecvError::Lagged(_) → continue; Closed → None.  Adapter pump = `while let Some(p) = next_progress(&mut rx).await { emit }`.
pub(super) async fn forward_progress(mut core_rx: broadcast::Receiver<ProtonInstallProgress>, out: ProgressEmitter) -> bool;
//   Relays every core event to `out` with `messages::progress_message` applied; continues on Lagged; returns
//   true when a Done|Failed|Cancelled phase was relayed. Ends when the core sender drops.

// identity.rs (D3, pure; no I/O, no allocation of jobs)
#[derive(Debug, Clone, Copy, PartialEq, Eq)] pub enum IdentityError { Missing, Mismatch, MetadataMismatch }
pub fn verify_install_identity<'a>(request: &ProtonUpInstallRequest, supplied: Option<&ProtonUpAvailableVersion>,
    catalog: &'a [ProtonUpAvailableVersion]) -> Result<&'a ProtonUpAvailableVersion, IdentityError>;
//   entry = first v with v.version==request.version && v.provider==request.provider else Missing.
//   Some(s): s.provider!=entry.provider || s.version!=entry.version → Mismatch;
//            s.download_url/checksum_url/checksum_kind (Option<String> ==, no normalization) differ → MetadataMismatch.
//   release_url, asset_size, published_at are never compared.

// owners.rs
#[derive(Clone, Default)] pub(super) struct OwnerLedger(Arc<Mutex<HashMap<JobId, InstallOwner>>>);
impl OwnerLedger {
    pub(super) fn register(&self, id: JobId, owner: InstallOwner) -> OwnerGuard;            // inserted by the job body before it signals start_install
    pub(super) fn authorize(&self, owner: &InstallOwner, op_id: &str) -> Result<JobId, AppError>; // NotFound MSG_OP_NOT_FOUND for unknown AND foreign
    #[cfg(test)] pub(super) fn len(&self) -> usize;
}
pub(super) struct OwnerGuard { /* Drop removes the entry, panic-safe (PoisonError::into_inner) */ }
```

```rust
// seams.rs — app-private (pub inside pub mod, not re-exported from services/lib); D4 + root seam for HOME hermeticity
pub trait ProtonCatalogSource: Send + Sync {
    fn versions<'a>(&'a self, provider_id: &'a str, force_refresh: bool, include_prereleases: bool)
        -> BoxFuture<'a, ProtonUpCatalogResponse>;                       // default CoreCatalogSource{metadata}: core list_available_versions_by_id
}
pub trait ProtonInstaller: Send + Sync {
    fn install<'a>(&'a self, request: &'a ProtonUpInstallRequest, version: &'a ProtonUpAvailableVersion,
        emitter: Option<ProgressEmitter>, cancel: Option<CancellationToken>)
        -> BoxFuture<'a, Result<ProtonUpInstallResult, InstallError>>;  // default CoreInstaller: core install_version_with_progress
}
pub trait ProtonRootSource: Send + Sync {
    fn steam_roots(&self) -> BoxFuture<'_, Vec<PathBuf>>;               // backend-trusted Steam roots (default: default_client_install_path → discover_steam_root_candidates in run_blocking)
    fn install_roots(&self, steam_root: Option<&Path>) -> Vec<InstallRootCandidate>; // blocking; default: resolve_install_root_candidates
}
pub(super) struct CoreCatalogSource { pub(super) metadata: MetadataStore }
pub(super) struct CoreInstaller;
pub(super) struct CoreProtonRoots { pub(super) steam: SteamService }
```

```rust
// mod.rs
pub const PROTONUP_INSTALL_JOB_KIND: &str = "protonup-install";
pub const DEFAULT_PROVIDER_ID: &str = "ge-proton";
#[derive(Clone)]
pub struct ProtonUpService {
    metadata: MetadataStore, settings: SettingsStore, steam: SteamService, jobs: JobRegistry,
    catalog: Arc<dyn ProtonCatalogSource>, installer: Arc<dyn ProtonInstaller>, roots: Arc<dyn ProtonRootSource>,
    owners: OwnerLedger,
}
impl ProtonUpService {
    pub fn new(metadata: MetadataStore, settings: SettingsStore, steam: SteamService, jobs: JobRegistry) -> Self;
    #[cfg(test)] pub(crate) fn with_seams(metadata: MetadataStore, settings: SettingsStore, steam: SteamService, jobs: JobRegistry,
        catalog: Arc<dyn ProtonCatalogSource>, installer: Arc<dyn ProtonInstaller>, roots: Arc<dyn ProtonRootSource>) -> Self;
    #[cfg(test)] pub(crate) fn owner_count(&self) -> usize;
    #[cfg(test)] pub(crate) fn jobs(&self) -> &JobRegistry;
}
static_assertions::assert_impl_all!(ProtonUpService: Send, Sync);

// catalog.rs (B2.1) — also defines private `async fn flags(&self) -> ProtonFlags {offline_mode, include_prereleases}`: run_blocking(settings.load); Err → both false + tracing::warn
pub fn list_providers(&self) -> Vec<ProtonUpProviderDescriptor>;                       // describe_providers()
pub async fn list_available_versions(&self, provider: Option<String>, force_refresh: bool) -> Result<ProtonUpCatalogResponse, AppError>;
//   provider None → DEFAULT_PROVIDER_ID; offline_mode ⇒ force_refresh=false. Ok always except join failure.
// roots.rs (B2.1)
pub async fn resolve_install_roots(&self, steam_client_install_path: Option<String>) -> Result<Vec<InstallRootCandidate>, AppError>; // run_blocking(roots.install_roots) — today's behaviour
pub async fn get_suggestion(&self, community_version: String, steam_client_install_path: Option<String>) -> Result<ProtonUpSuggestion, AppError>; // supplied path else steam.default_client_install_path(); run_blocking(discover_steam_root_candidates → discover_compat_tools → match_community_version)
// install.rs (B2.1)
pub async fn install_version(&self, request: ProtonUpInstallRequest) -> Result<ProtonUpInstallResult, AppError>;
//   offline → Ok(success:false, NetworkError, MSG_OFFLINE); catalog fetch (force_refresh=false, include_prereleases); identity(None);
//   Missing → Ok(success:false, DependencyMissing, "version {v} not found in catalog"); install catalog entry via installer(emitter None, cancel None);
//   Err(InstallError) → Ok(messages::install_error_result(&e)).
pub async fn start_install(&self, owner: InstallOwner, request: ProtonUpInstallRequest, version: ProtonUpAvailableVersion)
    -> Result<ProtonInstallStart, AppError>;
// cancel.rs (B2.1)
pub fn cancel_install(&self, owner: &InstallOwner, op_id: &str) -> Result<(), AppError>;   // authorize → jobs.cancel; any failure = NotFound MSG_OP_NOT_FOUND
// uninstall.rs (B2.2)
pub async fn plan_uninstall(&self, tool_path: String, steam_client_install_path: Option<String>) -> Result<ProtonUninstallPlanResponse, AppError>;
pub async fn uninstall(&self, tool_path: String, steam_client_install_path: Option<String>) -> Result<ProtonUninstallResponse, AppError>;
```

### 3.1 `start_install` algorithm (order is binding)

1. `flags()`; `offline_mode` → `Err(Unavailable, MSG_OFFLINE)` — before catalog, before any job.
2. `catalog.versions(request.provider, false, include_prereleases)`; `verify_install_identity(&request, Some(&version), &catalog.versions)`; map `IdentityError` via `messages::identity_message` (`Validation{field:"version"}`, texts verbatim from today's command: Missing → `requested version '{v}' for provider '{p}' was not found in the server catalog`; Mismatch → `install request/version mismatch with server catalog identity`; MetadataMismatch → `install metadata did not match server-resolved catalog entry`). Nothing is allocated on any `Err`.
3. `entry = catalog_entry.clone()`; `jobs.start(JobSpec::new(PROTONUP_INSTALL_JOB_KIND).with_key(JobKey::new(format!("protonup-install:{provider}:{version}"))), body)` (`Conflict` on duplicate).
4. **Job body** (owns `entry`, `request`, `owner`, `installer`, `owners`, a `oneshot::Sender<Receiver<ProtonInstallProgress>>`): `let id = ctx.id().clone(); let _guard = owners.register(id.clone(), owner);` create `(core_emitter, core_rx) = ProgressEmitter::new(id.as_str())` and `(out_emitter, out_rx) = ProgressEmitter::new(id.as_str())`; send `out_rx` through the oneshot (so the owner entry exists before `start_install` returns); if `ctx.is_cancelled()` → emit `Cancelled` on `out_emitter`, return `Outcome::Cancelled`; else `tokio::join!(installer.install(&request,&entry,Some(core_emitter),Some(ctx.cancel_token().clone())), forward_progress(core_rx, out_emitter.clone()))`; if no terminal phase relayed and the result is `Err`/`success:false` emit `Phase::Failed` with `MSG_INSTALL_FAILED`; log underlying detail with `tracing` only; outcome: `Ok(r) if r.success` → `Succeeded`; `Err(InstallError::Cancelled)` or `error_kind==Cancelled` → `Cancelled`; else `Failed(AppError::new(Io|Internal, MSG_INSTALL_FAILED))`.
5. `start_install` awaits the oneshot (dropped sender → `Unavailable "install could not be started"`), spawns a detached `tokio::spawn` reaper (`into_events` until `Finished`, then `jobs.reap`), returns `ProtonInstallStart { op_id: id.to_string(), progress: out_rx }`.

### 3.2 Uninstall algorithm (D7; shared by plan and execute; execute = same steps + `execute_uninstall_for_path`)

1. `tool_path` empty or any `Component::ParentDir` → `Ok(success:false, UNINSTALL_OUTSIDE)`.
2. `trusted = roots.steam_roots().await`. Caller path: `fs::canonicalize` equal to a canonicalized trusted root → use that trusted spelling; else ignored (`tracing::debug`). No caller path → `trusted.first()`.
3. `candidates = roots.install_roots(steam.as_deref())`, dropping any with `reason == Some("invalid-path")`; accept the first candidate `c` where `fs_util::ensure_within_workspace(&c.path, Path::new(&tool_path))` is `Ok(canonical)`; none → `UNINSTALL_OUTSIDE` (`WorkspacePathError::NotFound` → `UNINSTALL_NOT_FOUND`).
4. Core containment stays authoritative: `plan_uninstall(&canonical, steam.as_deref())` (plan) or `execute_uninstall_for_path(&canonical, steam.as_deref())` (uninstall; re-plans immediately before `remove_dir_all`). `UninstallError` → `messages::uninstall_error_text`. Path-free responses; `conflicting_app_ids` only on plan success.

## 4. proton_migration

```rust
// mod.rs (B1)
#[derive(Clone)] pub struct ProtonMigrationService { profiles: ProfileStore, writer: ProfileWriter, steam: SteamService }
impl ProtonMigrationService { pub fn new(profiles: ProfileStore, writer: ProfileWriter, steam: SteamService) -> Self; }
static_assertions::assert_impl_all!(ProtonMigrationService: Send, Sync);
// check.rs (B2.3) — body moved verbatim from tauri/commands/migration.rs:30-65
pub async fn check(&self, steam_client_install_path: Option<String>) -> Result<MigrationScanResult, AppError>;
//   A-5: supplied.trim() non-empty && join("steamapps").is_dir() else steam.default_client_install_path().await;
//   run_blocking(discover_steam_root_candidates → scan_proton_migrations(&profiles, ..)); A-4: old_path/new_path/stale_path/installed.path via crate::presenter::display_path_tilde
// apply.rs (B2.3)
pub async fn apply(&self, request: ApplyMigrationRequest) -> Result<MigrationApplyResult, AppError>;           // writer.apply_proton_migration(request).await.map(ProfileCommit::into_output)
pub async fn apply_batch(&self, request: BatchMigrationRequest) -> Result<BatchMigrationResult, AppError>;     // writer.apply_batch_migration(request).await.map(ProfileCommit::into_output)
```

`ProfileCommit` is `crate::services::profiles::ProfileCommit`. Error texts are the writer's (unchanged); the adapter returns `error.message().to_string()`.

## 5. umu

```rust
// seams.rs (B1)
pub trait UmuRefresher: Send + Sync { fn refresh(&self) -> BoxFuture<'_, Result<UmuDatabaseRefreshStatus, umu_database::Error>>; } // default CoreUmuRefresher → umu_database::refresh_umu_database()
// mod.rs (B1)
#[derive(Clone)] pub struct UmuService { metadata: MetadataStore, refresher: Arc<dyn UmuRefresher> }
impl UmuService { pub fn new(metadata: MetadataStore) -> Self; #[cfg(test)] pub(crate) fn with_refresher(metadata: MetadataStore, refresher: Arc<dyn UmuRefresher>) -> Self; }
static_assertions::assert_impl_all!(UmuService: Send, Sync);
// refresh.rs / coverage.rs (B2.4)
pub async fn refresh(&self) -> Result<UmuDatabaseRefreshStatus, AppError>;
//   Err mapping, detail only via tracing::warn: Network|Metadata → Unavailable, Io|Base → Io; message always UMU_REFRESH_FAILED.
pub async fn coverage(&self, app_id: Option<String>) -> Result<CsvCoverage, AppError>;   // None/blank → Unknown; else run_blocking(check_coverage(trimmed, Some("steam")))
pub async fn clear_gameid_cache(&self) -> Result<usize, AppError>;                       // run_blocking(metadata.clear_umu_gameid_cache()); MetadataStoreError → AppError::from
```

`bootstrap/scans.rs` (B2.4): `schedule_umu(app, startup, services, umu, events)`; in the job closure `let umu_service = services.umu.clone();` replaces `umu_database::refresh_umu_database()`; `Ok(status)` → `UmuRefreshSummary { refreshed, cached_at, reason }` (unchanged), `Err(e)` → `JobUpdate::umu_refresh_failed(e.message().to_string())`; remove the `crosshook_core::umu_database` import; `UmuRefresh::Skip` branch untouched.

## 6. Services wiring (B1 task 1.3)

`Services` gains `pub protonup: ProtonUpService`, `pub proton_migration: ProtonMigrationService`, `pub umu: UmuService` (doc comments in the existing style). In `Services::new`: bind `let steam = SteamService::new(stores.metadata.clone(), stores.settings.clone());` before the literal; `steam: steam.clone()`; `protonup: ProtonUpService::new(stores.metadata.clone(), stores.settings.clone(), steam.clone(), jobs.clone())` placed **before** `storage` (which moves `jobs`); `proton_migration: ProtonMigrationService::new(stores.profiles.clone(), writer.clone(), steam)` placed **before** `lutris` (which moves `writer`); `umu: UmuService::new(stores.metadata.clone())`. `stores/tests.rs` extends `services_new_builds_bundle_from_stores`: a key `protonup-install:x:y` held through the app registry conflicts when `services.protonup` starts the same key — needs the `jobs()` accessor declared on `ProtonUpService` (B1 1.1).

## 7. Tauri adapters (B3)

Local `fn map_app_error(e: AppError) -> String { e.message().to_string() }` per file (community precedent). All 15 names, camelCase param names and return shapes unchanged:

| Command                                                                                                         | Delegate                                                                                                                                                                                                                                              |
| --------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `protonup_list_available_versions(provider, force_refresh, service)`                                            | `service.list_available_versions(provider, force_refresh.unwrap_or(false))`                                                                                                                                                                           |
| `protonup_install_version(request, service)`                                                                    | `service.install_version(request)`                                                                                                                                                                                                                    |
| `protonup_get_suggestion(community_version, steam_client_install_path, service)`                                | `service.get_suggestion(..)`                                                                                                                                                                                                                          |
| `protonup_list_providers(service)` (sync)                                                                       | `Ok(service.list_providers())`                                                                                                                                                                                                                        |
| `protonup_resolve_install_roots(steam_client_install_path, service)`                                            | `service.resolve_install_roots(..)`                                                                                                                                                                                                                   |
| `protonup_install_version_async(app, webview: tauri::Webview, request, version, service)`                       | `owner = InstallOwner::new(webview.label())`; `start_install`; `tauri::async_runtime::spawn` pump `while let Some(p) = next_progress(&mut start.progress).await { let _ = app.emit("protonup:install:progress", &p); }`; return `ProtonInstallHandle` |
| `protonup_cancel_install(op_id, webview, service)`                                                              | `Ok(()) → Ok(true)`; `Err(NotFound) → Ok(false)`; other `Err` → `Err(msg)`                                                                                                                                                                            |
| `protonup_plan_uninstall_version` / `protonup_uninstall_version(tool_path, steam_client_install_path, service)` | `service.plan_uninstall` / `service.uninstall`                                                                                                                                                                                                        |
| `check_proton_migrations(steam_client_install_path, service)`                                                   | `service.check(..)`                                                                                                                                                                                                                                   |
| `apply_proton_migration(request, service)` / `apply_batch_migration(request, service)`                          | `service.apply` / `service.apply_batch`                                                                                                                                                                                                               |
| `refresh_umu_database(service)`                                                                                 | `service.refresh()`                                                                                                                                                                                                                                   |
| `check_umu_coverage(app_id, service)` → `Result<CsvCoverage,String>`                                            | `service.coverage(app_id)` (Ok payload IPC-identical)                                                                                                                                                                                                 |
| `clear_umu_gameid_lookup_cache(service)`                                                                        | `service.clear_gameid_cache()`                                                                                                                                                                                                                        |

Each file ends with `mod tests { fn command_names_match_expected_ipc_contract() { let _ = <every command>; } }`; `protonup.rs` also tests `install_owner` derivation (label → `InstallOwner`) and `map_app_error`. `ProtonInstallRegistry` type, its `.manage` in `tauri/lib.rs:71-73`, `load_include_prereleases`, local DTOs, `DEFAULT_PROVIDER_ID` and the `uuid`/`tokio-util` deps are deleted. `lib.rs` gains `.manage(services.protonup.clone())`, `.manage(services.proton_migration.clone())`, `.manage(services.umu.clone())`. Raw `MetadataStore`/`SettingsStore`/`ProfileStore` manages stay (other commands use them).

## 8. Fixed texts (`messages.rs`; policy per D8, all asserted path-free)

| Const                   | Text                                             |
| ----------------------- | ------------------------------------------------ |
| `MSG_OFFLINE`           | `offline mode is enabled`                        |
| `MSG_OP_NOT_FOUND`      | `install operation was not found`                |
| `MSG_INSTALL_FAILED`    | `Proton install failed`                          |
| `MSG_INSTALL_CANCELLED` | `Proton install cancelled`                       |
| `UNINSTALL_SYSTEM`      | `refusing to delete a system path`               |
| `UNINSTALL_OUTSIDE`     | `path is not inside a known Proton install root` |
| `UNINSTALL_NOT_FOUND`   | `Proton tool was not found`                      |
| `UNINSTALL_FAILED`      | `failed to remove the Proton tool`               |
| `UMU_REFRESH_FAILED`    | `failed to refresh the umu database`             |

`uninstall_error_text`: `SystemPathRefused`→SYSTEM, `PathOutsideKnownRoots`→OUTSIDE, `NotFound`→NOT_FOUND, `Io`→FAILED. `install_error_result` (legacy path; kinds as core's `to_result`, texts fixed): InvalidPath→`install path is invalid`; PermissionDenied→`permission denied while installing`; NetworkError|UntrustedUrl→`network error while installing`; ChecksumMissing|ChecksumFailed→`checksum verification failed`; AlreadyInstalled{path}→`Proton version is already installed` (+ structured `installed_path`); DependencyMissing→`install dependency is missing`; NoWritableInstallRoot→core's existing path-free text; Cancelled→`install cancelled`; Unknown→`install failed`. `progress_message(phase, original)`: Failed→MSG_INSTALL_FAILED, Cancelled→MSG_INSTALL_CANCELLED, other phases keep `original` only if it contains no `/` (else `None`).

## 9. Test helpers (B1 `tests/helpers.rs`; used by B2.1/B2.2)

```rust
pub(super) struct FakeCatalog { response: Mutex<ProtonUpCatalogResponse>, pub calls: AtomicUsize, pub last_args: Mutex<Option<(String,bool,bool)>> }  // impl ProtonCatalogSource
pub(super) struct FakeInstaller { pub calls: AtomicUsize, pub seen: Mutex<Vec<ProtonUpAvailableVersion>>, mode: Mode }                                // impl ProtonInstaller
pub(super) enum Mode { Succeed /*emits Resolving, Done*/, FailNoTerminal /*Err(Unknown("<tmp path>")) without Failed phase*/, FailWithPathMessage /*emits Failed with a tmp path message, Err*/,
                       Gated(Arc<tokio::sync::Notify>) /*emits Resolving, waits notify or cancel token → emits Cancelled, Err(Cancelled)*/ }
pub(super) struct FakeRoots { steam: Vec<PathBuf>, installs: Vec<InstallRootCandidate> }                                                           // impl ProtonRootSource
pub(super) struct Harness { pub root: TempDir, pub service: ProtonUpService, pub jobs: JobRegistry, pub settings: SettingsStore, pub catalog: Arc<FakeCatalog>,
                            pub installer: Arc<FakeInstaller>, pub metadata: MetadataStore /* open_in_memory */ }
impl Harness { pub fn new(mode: Mode) -> Self; pub fn with_offline(self) -> Self /* settings.save offline_mode=true */; pub fn steam_root(&self) -> PathBuf /* <root>/Steam with steamapps/ */;
               pub fn make_tool(&self, name: &str) -> PathBuf /* <root>/Steam/compatibilitytools.d/<name>/proton */; pub fn assert_rejected(&self) /* installer.calls==0, jobs.job_counts()==(0,0), service.owner_count()==0 */ }
pub(super) fn ge_catalog() -> ProtonUpCatalogResponse;   // one GE-Proton9-21 entry, github.com URLs, sha512 sidecar
pub(super) fn request(version: &str) -> ProtonUpInstallRequest; pub(super) fn owner(label: &str) -> InstallOwner;
pub(super) async fn finished(jobs: &JobRegistry);        // polls job_counts()==(0,0) under tokio::time::timeout(10s), no sleeps for sync
```

Gating uses `Notify`/channels and the job's cancel token — never timing sleeps. Symlink fixtures use `std::os::unix::fs::symlink` under `#[cfg(unix)]`.
