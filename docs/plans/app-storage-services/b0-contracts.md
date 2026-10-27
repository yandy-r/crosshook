# YAN-847 B0 frozen contracts

Output of the planning phase. B1 lands these as real code (struct, `new`, module tree, re-exports, harness); B2 implements methods in the files named below; B3 cuts the Tauri adapters over. **No task may edit a file owned by another task.** If a contract is wrong, message the integration owner; do not patch it from inside a slice. Paths are relative to `src/crosshook-native/`. All types named below already exist in `crosshook-core` unless the module path says otherwise; serde names are frozen.

## Wiring (D2)

```rust
// crates/crosshook-app/src/bootstrap/stores.rs
impl Services {
    pub fn new(stores: &Stores, events: EventBus, runtime: tokio::runtime::Handle, jobs: JobRegistry) -> Self;
}
pub struct Services {
    // …existing 9 fields unchanged…
    pub storage: StorageService,
    pub mods: ModsService,
    pub collections: CollectionsService,
    pub launcher_export: LauncherExportService,
    pub version: VersionService,
    pub steam: SteamService,
}
```

All `Services::new` callers (grep `Services::new` across the workspace, verified at plan time):

| Caller                                                                                                                           | Change                                                                                                                                                                  |
| -------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `crates/crosshook-app/src/bootstrap/app_bootstrap.rs:116` — `Services::new(&stores, app.events().clone(), app.runtime_handle())` | becomes `Services::new(&stores, app.events().clone(), app.runtime_handle(), app.jobs().clone())` (`app` is built at :94, before this line — `App::jobs()` is available) |
| `crates/crosshook-app/src/bootstrap/stores/tests.rs:127-131` — `services_new_builds_bundle_from_stores`                          | passes `JobRegistry::default()`; test extended to assert the six new fields build and the registry is clone-shared into `storage`                                       |

No other caller exists (`crosshook-cli` does not reference `Services`). `src-tauri/src/lib.rs` gains, after line 49: `.manage(services.storage.clone())`, `.manage(services.mods.clone())`, `.manage(services.collections.clone())`, `.manage(services.launcher_export.clone())`, `.manage(services.version.clone())`, `.manage(services.steam.clone())`. Raw store `.manage()` lines stay (other commands still use them).

`services/mod.rs` gains six `pub mod` lines and re-exports; `crosshook-app/src/lib.rs` re-exports the six services at crate root (house style: `pub use services::{CollectionsService, LauncherExportService, ModsService, SteamService, StorageService, VersionService};`).

## Scan as a job (D3)

```rust
// services/storage/mod.rs
pub const STORAGE_SCAN_JOB_KIND: &'static str = "prefix-storage-scan";  // JobSpec::kind
pub const STORAGE_SCAN_JOB_KEY: &str = "prefix-storage-scan";           // JobKey (exclusive)
// slot: Arc<Mutex<HashMap<JobId, PrefixStorageScanResult>>>  (take-on-read; entry removed when read)
```

- `start_scan()` → `self.jobs.start(JobSpec::new(STORAGE_SCAN_JOB_KIND).with_key(JobKey::new(STORAGE_SCAN_JOB_KEY)), move |ctx| async move { … })`. The closure runs the whole scan inside one `run_blocking` (collect references → `scan_prefix_storage_core` → fail-soft snapshot inserts), publishing `ctx.progress(JobProgress { current, total: Some(n), message })` once per scanned prefix, then `ctx.log(vec![format!("scanned {} prefixes", n)])`. On success it stores the result in the slot under `ctx.id()` **before** returning `Ok(Outcome::Succeeded)`; on error it returns `Outcome::Failed(error)`. A concurrent `start_scan` gets `Conflict` from the registry (key held). Cancellation: the closure checks `ctx.is_cancelled()` between the reference walk and the scan; cancelled → `Outcome::Cancelled` and no slot write.
- `scan()` = `start_scan()`, then `handle.into_events()` loop awaiting `JobEventKind::Finished` (with a `tokio::time::timeout` of 300 s so a hung scan surfaces as `AppErrorKind::Io`/`Cancelled` instead of blocking forever), then `scan_result(handle.id())`.
- `scan_result(&JobId)` takes (removes) the slot entry: `Ok(Some(result))`, `Ok(None)` when the job finished without a result (failed/cancelled/already read), `Err(NotFound)` when the registry knows nothing of the id.
- The scan result is **never authority** for cleanup: `cleanup` rescans itself (D4).

## Core cleanup hardening (D5) — `crates/crosshook-core/src/storage/prefix_health/cleanup.rs`

Existing per-kind checks stay; these are added before any canonicalization in `cleanup_orphan_prefix` and `cleanup_stale_staged_trainer`:

```rust
/// Rejects the target unless it is a plain directory reachable without
/// following a symlink at the target or any fixed intermediate component
/// below the prefix root. Fails closed on metadata errors.
fn ensure_no_follow_directories(root: &Path, components: &[&str], target: &Path)
    -> Result<PathBuf, CleanupRejection>;

/// Rejects the staged target unless it is an immediate child of the staged
/// root and both are strict descendants of the resolved prefix (never equal).
fn ensure_staged_target_layout(resolved_prefix: &Path, staged_root: &Path, target: &Path)
    -> Result<(), CleanupRejection>;

#[derive(Debug)]
struct CleanupRejection { reason: String }   // fixed strings, no paths
```

- Orphan path: `symlink_metadata(target)` must be a directory (reject symlink/dangling/special → `"target is a symlink; refusing cleanup"` / `"target no longer exists"`); then `ensure_no_follow_directories(resolved_prefix, &["drive_c", "CrossHook"], target)`; canonicalized target must equal the canonicalized resolved prefix (existing check) and both must be strict descendants of the canonicalized trusted root derived from the **reference inventory** (parent of the referenced prefix; compatdata root when the prefix ends in `pfx`) — never from the request.
- Staged path: reject symlink/dangling on target (`symlink_metadata`); `ensure_no_follow_directories(resolved_prefix, &["drive_c", "CrossHook", "StagedTrainers"], target)`; `ensure_staged_target_layout` (immediate child, strict descendants, roots themselves rejected); then the existing canonical containment checks.
- Any `symlink_metadata`/`canonicalize` error → skipped entry, never a fallback to the raw string.
- Rejections become `PrefixCleanupSkipped { target, reason }` — the `PrefixCleanupResult` shape is unchanged.

## Metadata-validation error helper (D8) — `crates/crosshook-app/src/error.rs`

```rust
impl AppError {
    /// Maps `MetadataStoreError::Validation(msg)` from the collections and
    /// mods stores to `Validation { field }` while keeping `msg` (user-entered
    /// names only; no paths). `sanitize_message` still runs via `AppError::new`.
    pub(crate) fn from_metadata_validation(
        field: &'static str,
        error: MetadataStoreError,
    ) -> Self;
}
```

`Validation(msg)` → `AppError::new(AppErrorKind::Validation { field }, format!("metadata validation error: {msg}"))` — the string matches `MetadataStoreError::Validation`'s Display exactly, so the adapter's `error.message().to_string()` reproduces today's IPC text ("metadata validation error: a mod named "X" is already registered for this profile", "metadata validation error: mod not found", "collection not found: …", "profile not found when adding to collection: …"). Every other variant delegates to the existing `From<MetadataStoreError>`. Used by `services/mods` (field `"mod"`), `services/collections` (field `"collection"`) and `services/version` (field `"version"`).

---

## Service contracts

Notation: methods on the service struct; all blocking work runs inside one `run_blocking` closure per operation (cloned stores moved in). Errors are `AppError` unless noted. Tauri-facing serde DTOs keep their exact field names.

### 1. `services/storage/` — `StorageService` (3 commands)

```rust
#[derive(Clone)]
pub struct StorageService {
    profiles: ProfileStore,
    metadata: MetadataStore,
    jobs: JobRegistry,
    scan_results: Arc<Mutex<HashMap<JobId, PrefixStorageScanResult>>>,
}
impl StorageService {
    pub fn new(profiles: ProfileStore, metadata: MetadataStore, jobs: JobRegistry) -> Self;
}
// scan.rs
impl StorageService {
    /// Awaitable scan: starts the exclusive job, awaits Finished, takes the result.
    pub async fn scan(&self) -> Result<PrefixStorageScanResult, AppError>;
    /// Starts the scan job (exclusive key; `Conflict` while one runs).
    pub fn start_scan(&self) -> Result<JobHandle, AppError>;
    /// Takes the stored result for a finished scan job.
    pub fn scan_result(&self, id: &JobId) -> Result<Option<PrefixStorageScanResult>, AppError>;
}
// cleanup.rs — the D4 orchestrator
impl StorageService {
    /// One blocking closure: reload references → fresh scan → exact-tuple
    /// allow-list (dedup) → server-produced targets → core cleanup (D5-hardened)
    /// → audit rows (D6). Never uses a stored scan result as authority.
    pub async fn cleanup(&self, targets: Vec<PrefixCleanupTarget>) -> Result<PrefixCleanupResult, AppError>;
}
// audit.rs
impl StorageService {
    pub async fn history(&self) -> Result<PrefixStorageHistoryResponse, AppError>;
}
// moved DTO (module path services::storage; serde names unchanged)
#[derive(Debug, Clone, Serialize)]
pub struct PrefixStorageHistoryResponse {
    pub available: bool,
    pub snapshots: Vec<PrefixStorageSnapshotRow>,   // crosshook_core::metadata
    pub audit: Vec<PrefixStorageCleanupAuditRow>,  // crosshook_core::metadata
}
```

Error mapping (adapter): scan/cleanup join failures keep the `prefix storage {scan|cleanup|history} task failed: {error}` shape via `AppError` messages of the same text built inside the closure; scanner `String` errors become `AppErrorKind::Internal` with the scanner's message. `cleanup` runs even when the DB is down; audit inserts stay fail-soft `tracing::warn!`.

### 2. `services/mods/` — `ModsService` (6 commands)

```rust
#[derive(Clone)]
pub struct ModsService { profiles: ProfileStore, metadata: MetadataStore }
impl ModsService { pub fn new(profiles: ProfileStore, metadata: MetadataStore) -> Self; }
// crud.rs
impl ModsService {
    pub async fn list(&self, profile_name: String) -> Result<ProfileModsResponse, AppError>;
    pub async fn add(&self, profile_name: String, input: ProfileModInput) -> Result<ProfileModRecord, AppError>;
    pub async fn update(&self, profile_name: String, mod_id: String, input: ProfileModInput) -> Result<ProfileModRecord, AppError>;
    pub async fn remove(&self, profile_name: String, mod_id: String) -> Result<(), AppError>;
}
// detect.rs — works with the DB down (best-effort enrichment only)
impl ModsService {
    pub async fn detect(&self, profile_name: String) -> Result<DetectionScanReport, AppError>;
}
// coexistence.rs
impl ModsService {
    pub async fn analyze(&self, profile_name: String) -> Result<Vec<LaunchValidationIssue>, AppError>;
}
// moved DTO (services::mods); ProfileModRecord/ProfileModInput/DetectionScanReport
// come from crosshook_core::mods, LaunchValidationIssue from crosshook_core::launch
#[derive(Serialize)]
pub struct ProfileModsResponse { pub available: bool, pub mods: Vec<ProfileModRecord> }
```

Semantics frozen: `list` on disabled DB → `Ok(ProfileModsResponse { available: false, mods: vec![] })`; `Ok(None)` identity → `{available:true, mods:[]}`. `add`/`update`/`remove`/`analyze` check `is_available()` first → `AppErrorKind::Unavailable`; adapter maps to the exact legacy string `metadata_unavailable: the metadata database could not be opened`. Profile resolution (moved from `commands/mods.rs:25-48`): load fails → `ProfileStoreError`'s text; `Ok(None)` → `AppErrorKind::NotFound` with `profile has no metadata record yet — save the profile once, then retry`; lookup error → `metadata_unavailable: {error}` text. `remove` returning `false` → `AppError::from_metadata_validation("mod", Validation("mod not found"))`. `detect`: missing/empty executable parent → `AppErrorKind::Validation { field: "game_path" }` with `no_game_path: set the game executable path to enable detection`; enrichment only when `is_available()`, all failures leave flags false. Store errors through `AppError::from_metadata_validation("mod", …)`.

### 3. `services/collections/` — `CollectionsService` (13 commands + D9)

```rust
#[derive(Clone)]
pub struct CollectionsService { metadata: MetadataStore, profiles: ProfileStore }
impl CollectionsService { pub fn new(metadata: MetadataStore, profiles: ProfileStore) -> Self; }
// crud.rs — each guards `is_available()` first (D7) except list/collections_for_profile
impl CollectionsService {
    pub async fn list(&self) -> Result<Vec<CollectionRow>, AppError>;
    pub async fn create(&self, name: String) -> Result<String, AppError>;
    pub async fn delete(&self, collection_id: String) -> Result<(), AppError>;
    pub async fn add_profile(&self, collection_id: String, profile_name: String) -> Result<(), AppError>;
    pub async fn remove_profile(&self, collection_id: String, profile_name: String) -> Result<(), AppError>;
    pub async fn list_profiles(&self, collection_id: String) -> Result<Vec<String>, AppError>;
    pub async fn rename(&self, collection_id: String, new_name: String) -> Result<(), AppError>;
    pub async fn update_description(&self, collection_id: String, description: Option<String>) -> Result<(), AppError>;
    pub async fn collections_for_profile(&self, profile_name: String) -> Result<Vec<CollectionRow>, AppError>;
}
// defaults.rs
impl CollectionsService {
    pub async fn get_defaults(&self, collection_id: String) -> Result<Option<CollectionDefaultsSection>, AppError>;
    pub async fn set_defaults(&self, collection_id: String, defaults: Option<CollectionDefaultsSection>) -> Result<(), AppError>;
    /// D9: the merge moved from commands/profile/shared.rs:35-48.
    /// Policy unchanged: blank id = identity; Corrupt bubbles; other errors fail open.
    pub async fn apply_defaults(&self, profile: GameProfile, collection_id: Option<&str>) -> Result<GameProfile, AppError>;
    /// Same resolution policy, sync, for ProfileWriter::list_summaries' resolver
    /// closure (runs at most once, only after the first profile loads).
    pub fn resolve_defaults_sync(&self, collection_id: Option<&str>) -> Result<Option<CollectionDefaultsSection>, AppError>;
    /// Pure, store-free merge core for unit tests (effective_profile_with wrapper).
    pub fn resolve(profile: GameProfile, defaults: Option<&CollectionDefaultsSection>) -> GameProfile;
}
// transfer.rs — D16: import stays a non-mutating preview
impl CollectionsService {
    pub async fn export_to_toml(&self, collection_id: String, output_path: String) -> Result<CollectionExportResult, AppError>;
    pub async fn import_from_toml(&self, path: String) -> Result<CollectionImportPreview, AppError>;
}
```

Frozen validation strings (adapter-identical): `collection_id must not be empty`, `output_path must not be empty`, `path must not be empty` (trim id, not output path). `get_defaults` clears extras before returning; `set_defaults` clears incoming extras then `preserve_extra_from(existing)` (the read uses `?` — a corrupt existing row fails the set; preserved deliberately). Writes (create/delete/add/remove/rename/set_defaults) and reads-that-must-be-honest return `Unavailable` before touching the store when disabled; `list`/`collections_for_profile` on a disabled DB keep today's `Ok(vec![])`. Store errors map through `AppError::from_metadata_validation("collection", …)` (D8), keeping "collection not found: …" etc.

### 4. `services/launcher_export/` — `LauncherExportService` (12 commands + D14)

```rust
#[derive(Clone)]
pub struct LauncherExportService { profiles: ProfileStore, settings: SettingsStore, metadata: MetadataStore }
impl LauncherExportService { pub fn new(profiles: ProfileStore, settings: SettingsStore, metadata: MetadataStore) -> Self; }
// moved DTO (services::launcher_export) — the IPC command keeps 15 flat args;
// the adapter packs them into this struct.
pub struct LauncherRenameRequest {
    pub old_launcher_slug: String,
    pub new_display_name: String,
    pub new_launcher_icon_path: String,
    pub target_home_path: String,
    pub steam_client_install_path: String,
    pub method: String,
    pub trainer_path: String,
    pub trainer_loading_mode: String,
    pub prefix_path: String,
    pub proton_path: String,
    pub steam_app_id: String,
    pub launcher_name: String,
    pub network_isolation: bool,
    pub gamescope: GamescopeConfig,
    pub custom_env_vars: BTreeMap<String, String>,
}
// export.rs
impl LauncherExportService {
    pub async fn validate(&self, request: SteamExternalLauncherExportRequest) -> Result<(), AppError>;
    /// Resolves only after files are written AND observe finishes (D11).
    pub async fn export(&self, request: SteamExternalLauncherExportRequest) -> Result<SteamExternalLauncherExportResult, AppError>;
    pub async fn check_exists(&self, request: SteamExternalLauncherExportRequest) -> Result<LauncherInfo, AppError>;
    pub async fn check_for_profile(&self, name: String) -> Result<LauncherInfo, AppError>;
    pub async fn delete(&self, display_name: String, steam_app_id: String, trainer_path: String,
                        target_home_path: String, steam_client_install_path: String) -> Result<LauncherDeleteResult, AppError>;
    pub async fn delete_by_slug(&self, launcher_slug: String, target_home_path: String,
                                steam_client_install_path: String) -> Result<LauncherDeleteResult, AppError>;
    /// D14: validates/sanitizes the OLD slug like delete_by_slug, and requires
    /// old and new paths to stay inside the resolved launcher directories.
    pub async fn rename(&self, request: LauncherRenameRequest) -> Result<LauncherRenameResult, AppError>;
}
// list.rs
impl LauncherExportService {
    pub async fn list(&self, target_home_path: String, steam_client_install_path: String) -> Vec<LauncherInfo>;
    pub async fn find_orphaned(&self, known_profile_slugs: Vec<String>, target_home_path: String,
                               steam_client_install_path: String) -> Vec<LauncherInfo>;
    pub async fn reexport_by_slug(&self, launcher_slug: String, target_home_path: String,
                                  steam_client_install_path: String) -> Result<SteamExternalLauncherExportResult, AppError>;
}
// preview.rs
impl LauncherExportService {
    pub async fn preview_script(&self, request: SteamExternalLauncherExportRequest) -> Result<String, AppError>;
    pub async fn preview_desktop(&self, request: SteamExternalLauncherExportRequest) -> Result<String, AppError>;
}
```

Moved helpers (private): `build_export_request_for_profile`, `stale_flags_by_profile_slug`, `apply_stale_flags`, `find_profile_name_for_launcher_slug` — behaviour identical (settings failure → Auto fallback in stale/slug matching; **hard error** in `check_for_profile` and in re-export's final settings reload; both inconsistencies preserved, D15). Metadata observations stay fail-soft after the file operation. `rename` parses the loading mode (`invalid trainer loading mode` on failure) and preserves the umu/runtime reset behaviour (D15). Core error Display texts are kept verbatim through `AppError::new(kind, error.to_string())` with kind `Validation` for request validation and `Io` otherwise.

### 5. `services/version/` — `VersionService` (4 commands)

```rust
#[derive(Clone)]
pub struct VersionService { profiles: ProfileStore, metadata: MetadataStore }
impl VersionService { pub fn new(profiles: ProfileStore, metadata: MetadataStore) -> Self; }
// moved DTOs (services::version; serde names frozen from commands/version.rs:21-42)
#[derive(Debug, Clone, Serialize)]
pub struct VersionSnapshotInfo { /* profile_id, steam_app_id, steam_build_id, trainer_version,
                                     trainer_file_hash, human_game_ver, status, checked_at */ }
#[derive(Debug, Clone, Serialize)]
pub struct VersionCheckResult  { /* profile_id, current_build_id, snapshot, status, update_in_progress */ }
// status.rs
impl VersionService {
    pub async fn check_status(&self, name: String) -> Result<VersionCheckResult, AppError>;
    pub async fn snapshot(&self, name: String) -> Result<Option<VersionSnapshotInfo>, AppError>;
}
// ops.rs — D7: both guard is_available() first → Unavailable
impl VersionService {
    pub async fn set_trainer_version(&self, name: String, version: String) -> Result<(), AppError>;
    pub async fn acknowledge_change(&self, name: String) -> Result<(), AppError>;
}
```

Moved private helpers: `status_from_str` (unknown → `Unknown`), `row_to_snapshot_info`, `locate_manifest_for_app` (the 5th copy, moved not deduplicated — D15). Missing identity on `set`/`acknowledge` keeps `profile '{name}' is not registered in the metadata store` (now via `AppErrorKind::NotFound`); disabled DB returns `Unavailable` **before** that check (D7 fixes the misleading message). Reads degrade exactly as today (empty profile id → no snapshot).

### 6. `services/steam/` — `SteamService` (3 + 4 commands)

```rust
#[derive(Clone)]
pub struct SteamService { metadata: MetadataStore, settings: SettingsStore }
impl SteamService { pub fn new(metadata: MetadataStore, settings: SettingsStore) -> Self; }
// client.rs
impl SteamService {
    pub async fn default_client_install_path(&self) -> String;   // env + stat, run_blocking
    pub async fn auto_populate(&self, request: SteamAutoPopulateRequest) -> Result<SteamAutoPopulateResult, AppError>;
}
// proton.rs — D11: NOT cached; fresh scan every call
impl SteamService {
    pub async fn list_proton_installs(&self, steam_client_install_path: Option<String>) -> Result<Vec<ProtonInstall>, AppError>;
}
// metadata.rs
impl SteamService {
    pub async fn fetch_game_metadata(&self, app_id: String, force_refresh: Option<bool>) -> Result<SteamMetadataLookupResult, AppError>;
    /// D12: service returns Unavailable on network failure (cached art already
    /// returned first by core); adapter maps Unavailable -> Ok(None).
    pub async fn fetch_cover_art(&self, app_id: String, image_type: Option<String>) -> Result<Option<String>, AppError>;
    pub async fn import_custom_cover_art(&self, source_path: String) -> Result<String, AppError>;
    pub async fn import_custom_art(&self, source_path: String, art_type: Option<String>) -> Result<String, AppError>;
}
```

Frozen strings: `Unknown image type: {other}` / `Unknown art type: {other}`. The API key is read fail-soft inside `run_blocking` and every span uses `skip`/`skip_all` — it is never in a message. `default_client_install_path` order preserved: `$STEAM_COMPAT_CLIENT_INSTALL_PATH` (trimmed, verbatim) → `$HOME` unset → `""` → first of `.local/share/Steam`, `.steam/root`, `.var/app/com.valvesoftware.Steam/data/Steam` whose `steamapps` is a dir → `""`. `list_proton_installs(None)` defaults to the same resolver; `Some("")` passes through (unchanged). Two in-shell Tauri callers (`protonup.rs:168`, `migration.rs:14,35`) switch to `State<'_, SteamService>`.

---

## Command → service method map (all 45)

| #   | Tauri command (unchanged name)      | Source today                   | Service method                                |
| --- | ----------------------------------- | ------------------------------ | --------------------------------------------- |
| 1   | `scan_prefix_storage`               | `commands/storage.rs:18`       | `StorageService::scan`                        |
| 2   | `cleanup_prefix_storage`            | `commands/storage.rs:57`       | `StorageService::cleanup`                     |
| 3   | `get_prefix_storage_history`        | `commands/storage.rs:155`      | `StorageService::history`                     |
| 4   | `list_profile_mods`                 | `commands/mods.rs:57`          | `ModsService::list`                           |
| 5   | `add_profile_mod`                   | `commands/mods.rs:104`         | `ModsService::add`                            |
| 6   | `update_profile_mod`                | `commands/mods.rs:130`         | `ModsService::update`                         |
| 7   | `remove_profile_mod`                | `commands/mods.rs:157`         | `ModsService::remove`                         |
| 8   | `detect_profile_mods`               | `commands/mods.rs:187`         | `ModsService::detect`                         |
| 9   | `analyze_mod_coexistence`           | `commands/mods.rs:237`         | `ModsService::analyze`                        |
| 10  | `collection_list`                   | `commands/collections.rs:15`   | `CollectionsService::list`                    |
| 11  | `collection_create`                 | `commands/collections.rs:22`   | `CollectionsService::create`                  |
| 12  | `collection_delete`                 | `commands/collections.rs:30`   | `CollectionsService::delete`                  |
| 13  | `collection_add_profile`            | `commands/collections.rs:40`   | `CollectionsService::add_profile`             |
| 14  | `collection_remove_profile`         | `commands/collections.rs:51`   | `CollectionsService::remove_profile`          |
| 15  | `collection_list_profiles`          | `commands/collections.rs:62`   | `CollectionsService::list_profiles`           |
| 16  | `collection_rename`                 | `commands/collections.rs:72`   | `CollectionsService::rename`                  |
| 17  | `collection_update_description`     | `commands/collections.rs:83`   | `CollectionsService::update_description`      |
| 18  | `collections_for_profile`           | `commands/collections.rs:94`   | `CollectionsService::collections_for_profile` |
| 19  | `collection_get_defaults`           | `commands/collections.rs:104`  | `CollectionsService::get_defaults`            |
| 20  | `collection_set_defaults`           | `commands/collections.rs:118`  | `CollectionsService::set_defaults`            |
| 21  | `collection_export_to_toml`         | `commands/collections.rs:138`  | `CollectionsService::export_to_toml`          |
| 22  | `collection_import_from_toml`       | `commands/collections.rs:157`  | `CollectionsService::import_from_toml`        |
| 23  | `validate_launcher_export`          | `commands/export.rs:189`       | `LauncherExportService::validate`             |
| 24  | `export_launchers`                  | `commands/export.rs:195`       | `LauncherExportService::export`               |
| 25  | `check_launcher_exists`             | `commands/export.rs:216`       | `LauncherExportService::check_exists`         |
| 26  | `check_launcher_for_profile`        | `commands/export.rs:225`       | `LauncherExportService::check_for_profile`    |
| 27  | `delete_launcher`                   | `commands/export.rs:241`       | `LauncherExportService::delete`               |
| 28  | `delete_launcher_by_slug`           | `commands/export.rs:268`       | `LauncherExportService::delete_by_slug`       |
| 29  | `rename_launcher`                   | `commands/export.rs:290`       | `LauncherExportService::rename`               |
| 30  | `list_launchers`                    | `commands/export.rs:354`       | `LauncherExportService::list`                 |
| 31  | `find_orphaned_launchers`           | `commands/export.rs:374`       | `LauncherExportService::find_orphaned`        |
| 32  | `reexport_launcher_by_slug`         | `commands/export.rs:388`       | `LauncherExportService::reexport_by_slug`     |
| 33  | `preview_launcher_script`           | `commands/export.rs:437`       | `LauncherExportService::preview_script`       |
| 34  | `preview_launcher_desktop`          | `commands/export.rs:447`       | `LauncherExportService::preview_desktop`      |
| 35  | `check_version_status`              | `commands/version.rs:97`       | `VersionService::check_status`                |
| 36  | `get_version_snapshot`              | `commands/version.rs:167`      | `VersionService::snapshot`                    |
| 37  | `set_trainer_version`               | `commands/version.rs:193`      | `VersionService::set_trainer_version`         |
| 38  | `acknowledge_version_change`        | `commands/version.rs:228`      | `VersionService::acknowledge_change`          |
| 39  | `default_steam_client_install_path` | `commands/steam.rs:9`          | `SteamService::default_client_install_path`   |
| 40  | `list_proton_installs`              | `commands/steam.rs:35`         | `SteamService::list_proton_installs`          |
| 41  | `auto_populate_steam`               | `commands/steam.rs:52`         | `SteamService::auto_populate`                 |
| 42  | `fetch_game_metadata`               | `commands/game_metadata.rs:11` | `SteamService::fetch_game_metadata`           |
| 43  | `fetch_game_cover_art`              | `commands/game_metadata.rs:21` | `SteamService::fetch_cover_art`               |
| 44  | `import_custom_cover_art`           | `commands/game_metadata.rs:52` | `SteamService::import_custom_cover_art`       |
| 45  | `import_custom_art`                 | `commands/game_metadata.rs:57` | `SteamService::import_custom_art`             |

Plus the D9 cutover (not a command change): `profile_load` (`commands/profile/lifecycle.rs:17-40`) and `profile_list_summaries` (`:43-56`) delegate to `CollectionsService::apply_defaults` / `resolve_defaults_sync`; `commands/profile/shared.rs` and the five tests in `commands/profile/tests/optimizations.rs:1-128` are deleted after the move.

## Adapter error-string contract (frozen)

| Service error                                            | Tauri string                                                                                                                                                                                                                                                                                                                                          |
| -------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| mods `Unavailable` (add/update/remove/analyze)           | `metadata_unavailable: the metadata database could not be opened`                                                                                                                                                                                                                                                                                     |
| mods identity lookup error                               | `metadata_unavailable: {error}`                                                                                                                                                                                                                                                                                                                       |
| mods missing identity                                    | `profile has no metadata record yet — save the profile once, then retry`                                                                                                                                                                                                                                                                              |
| mods `remove` false                                      | `metadata validation error: mod not found`                                                                                                                                                                                                                                                                                                            |
| detect missing game path                                 | `no_game_path: set the game executable path to enable detection`                                                                                                                                                                                                                                                                                      |
| join failures (scan/cleanup/history/mods detect/analyze) | `prefix storage {scan\|cleanup\|history} task failed: {error}` / `mod {registry\|detection\|analysis} task failed: {error}`                                                                                                                                                                                                                           |
| collections/version validation strings                   | `collection_id must not be empty`, `output_path must not be empty`, `path must not be empty`, `profile '{name}' is not registered in the metadata store`, `invalid trainer loading mode`, `Unknown image type: {x}`, `Unknown art type: {x}`, `No profile found for launcher slug '{slug}'`, `Profile launch method does not support launcher export` |
| everything else                                          | `error.message().to_string()` of the `AppError` (Display text equals today's `.to_string()` of the underlying core error for moved code)                                                                                                                                                                                                              |

## File ownership at a glance

| Task        | Owns (creates or fills)                                                                                                                                                                                                                                                                        |
| ----------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| B0.1        | `crates/crosshook-app/tests/fixtures/launcher_export/*`, `crates/crosshook-app/tests/launcher_export_golden.rs`, `crates/crosshook-app/Cargo.toml` (`[[test]]` block), `docs/plans/app-storage-services/ipc-inventory.md`                                                                      |
| B1.1        | `services/mod.rs`, six `services/<x>/mod.rs` + `tests/{mod,helpers}.rs` skeletons, `bootstrap/stores.rs`, `bootstrap/stores/tests.rs`, `bootstrap/app_bootstrap.rs`, `error.rs` (D8 helper), `crates/crosshook-app/src/lib.rs`, `src-tauri/src/lib.rs` (manage lines only)                     |
| B2.1–B2.6   | one service directory each: implementation files + `tests/<slice>.rs` (B2.4 also owns the golden test flip)                                                                                                                                                                                    |
| B2.0 (core) | `crates/crosshook-core/src/storage/prefix_health/cleanup.rs` (D5) + its new tests                                                                                                                                                                                                              |
| B3.1–B3.4   | `commands/{storage,mods}.rs`; `commands/collections.rs` + `commands/profile/{shared,lifecycle}.rs` + `commands/profile/tests/optimizations.rs`; `commands/export.rs`; `commands/{version,steam,game_metadata}.rs` + `protonup.rs:168` + `migration.rs:14,35` — each owns its IPC-name pin test |
| B4.1        | `services/storage/tests/cleanup_security.rs`                                                                                                                                                                                                                                                   |
| B5.1        | `crates/crosshook-app/README.md`, dead-code sweep (removes `#![expect(dead_code)]` from the six `mod.rs`), follow-up issues                                                                                                                                                                    |

B1 skeleton rule: struct + `new` + module tree + re-exports + `assert_impl_all!(Send, Sync)` + `#![expect(dead_code)]` headers compile now; **no `todo!()` stubs** — B2 fills the declared method files, so the crate compiles at every batch gate (methods simply do not exist yet outside their owner's files).
