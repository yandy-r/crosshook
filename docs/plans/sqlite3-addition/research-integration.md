# SQLite3 Addition — Integration Research

## API Endpoints and Integration Overview

This document catalogues every Tauri IPC command, store public API, launch system hook, filesystem path, and community system type that the `metadata` module must integrate with. All signatures are verified against actual source code as of 2026-03-27. Metadata sync hooks live in the Tauri command layer only — `ProfileStore`, `LauncherStore`, and `CommunityTapStore` internals are not modified.

---

## 1. Tauri IPC Commands

All handlers registered in `src/crosshook-native/src-tauri/src/lib.rs:70-113`.

### 1.1 Profile Commands (`commands/profile.rs`)

| Command                             | Signature                                                                               | Metadata Sync Hook Needed                                                        |
| ----------------------------------- | --------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------- |
| `profile_list`                      | `fn(State<ProfileStore>) -> Result<Vec<String>, String>`                                | No — read-only                                                                   |
| `profile_load`                      | `fn(String, State<ProfileStore>) -> Result<GameProfile, String>`                        | No — read-only                                                                   |
| `profile_save`                      | `fn(String, GameProfile, State<ProfileStore>) -> Result<(), String>`                    | **Yes** — upsert profile identity + snapshot                                     |
| `profile_save_launch_optimizations` | `fn(String, LaunchOptimizationsPayload, State<ProfileStore>) -> Result<(), String>`     | **Yes** — upsert profile snapshot (content changed)                              |
| `profile_delete`                    | `fn(String, State<ProfileStore>) -> Result<(), String>`                                 | **Yes** — soft-delete identity row (tombstone)                                   |
| `profile_duplicate`                 | `fn(String, State<ProfileStore>) -> Result<DuplicateProfileResult, String>`             | **Yes** — create new identity + link `source_profile_id` + append rename history |
| `profile_rename`                    | `fn(String, String, State<ProfileStore>, State<SettingsStore>) -> Result<bool, String>` | **Yes** — append rename history + update `current_filename`/`current_path`       |
| `profile_import_legacy`             | `fn(String, State<ProfileStore>) -> Result<GameProfile, String>`                        | **Yes** — create profile identity from import                                    |
| `profile_export_toml`               | `fn(String, GameProfile) -> Result<String, String>`                                     | No — generates TOML string, no persistence                                       |

**Key internal helpers in `profile.rs`:**

- `cleanup_launchers_for_profile_delete(profile_name, profile)` — called inside `profile_delete` and `profile_rename` before store operations; metadata hook for launcher deletion should fire _after_ this.
- `derive_steam_client_install_path(profile)` / `derive_target_home_path(steam_client_install_path)` — path derivation used for launcher cleanup.

**`LaunchOptimizationsPayload`** (defined in `commands/profile.rs:76-83`):

```rust
pub struct LaunchOptimizationsPayload {
    pub enabled_option_ids: Vec<String>,
}
```

### 1.2 Export / Launcher Commands (`commands/export.rs`)

| Command                      | Signature                                                                                                                                    | Metadata Sync Hook Needed                         |
| ---------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------- |
| `validate_launcher_export`   | `fn(SteamExternalLauncherExportRequest) -> Result<(), String>`                                                                               | No — validation only                              |
| `export_launchers`           | `fn(SteamExternalLauncherExportRequest) -> Result<SteamExternalLauncherExportResult, String>`                                                | **Yes** — upsert launcher identity + observation  |
| `check_launcher_exists`      | `fn(SteamExternalLauncherExportRequest) -> Result<LauncherInfo, String>`                                                                     | **Yes** — upsert launcher observation             |
| `check_launcher_for_profile` | `fn(String, State<ProfileStore>) -> Result<LauncherInfo, String>`                                                                            | **Yes** — upsert launcher observation             |
| `delete_launcher`            | `fn(String, String, String, String, String) -> Result<LauncherDeleteResult, String>`                                                         | **Yes** — mark launcher deleted in observations   |
| `delete_launcher_by_slug`    | `fn(String, String, String) -> Result<LauncherDeleteResult, String>`                                                                         | **Yes** — mark launcher deleted by slug           |
| `rename_launcher`            | `fn(String, String, String, String, String, String, String, String, String, String, String, String) -> Result<LauncherRenameResult, String>` | **Yes** — update launcher slug/paths              |
| `list_launchers`             | `fn(String, String) -> Vec<LauncherInfo>`                                                                                                    | **Yes** — bulk observation sync                   |
| `find_orphaned_launchers`    | `fn(Vec<String>, String, String) -> Vec<LauncherInfo>`                                                                                       | **Yes** — mark orphaned launchers in observations |
| `preview_launcher_script`    | `fn(SteamExternalLauncherExportRequest) -> Result<String, String>`                                                                           | No — preview only                                 |
| `preview_launcher_desktop`   | `fn(SteamExternalLauncherExportRequest) -> Result<String, String>`                                                                           | No — preview only                                 |

### 1.3 Launch Commands (`commands/launch.rs`)

| Command                              | Signature                                                            | Metadata Sync Hook Needed                                                                                                  |
| ------------------------------------ | -------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------- |
| `validate_launch`                    | `fn(LaunchRequest) -> Result<(), LaunchValidationIssue>`             | No — validation only                                                                                                       |
| `preview_launch`                     | `fn(LaunchRequest) -> Result<LaunchPreview, String>`                 | No — preview only                                                                                                          |
| `build_steam_launch_options_command` | `fn(Vec<String>) -> Result<String, String>`                          | No — utility                                                                                                               |
| `launch_game`                        | `async fn(AppHandle, LaunchRequest) -> Result<LaunchResult, String>` | **Yes** — `record_launch_started()` before `spawn_log_stream`; `record_launch_finished()` in `stream_log_lines` completion |
| `launch_trainer`                     | `async fn(AppHandle, LaunchRequest) -> Result<LaunchResult, String>` | **Yes** — same as `launch_game`                                                                                            |

**Internal `stream_log_lines`** (not a Tauri command; called by `spawn_log_stream`):

- Emits `"launch-log"` events per line
- Emits `"launch-diagnostic"` event with `DiagnosticReport` (conditionally, when `should_surface_report` is true)
- Emits `"launch-complete"` event with `{ code: Option<i32>, signal: Option<i32> }`
- **Metadata hook insertion point**: after exit status is known (line 204-231 in `launch.rs`), before or after emitting the events

**`LaunchResult`** (defined in `commands/launch.rs:24-28`):

```rust
pub struct LaunchResult {
    pub succeeded: bool,
    pub message: String,
    pub helper_log_path: String,
}
```

### 1.4 Community Commands (`commands/community.rs`)

| Command                    | Signature                                                                                                                  | Metadata Sync Hook Needed                                          |
| -------------------------- | -------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------ |
| `community_add_tap`        | `fn(CommunityTapSubscription, State<SettingsStore>) -> Result<Vec<CommunityTapSubscription>, String>`                      | **Yes** — upsert tap subscription in `community_taps` table        |
| `community_list_profiles`  | `fn(State<SettingsStore>, State<CommunityTapStore>) -> Result<CommunityProfileIndex, String>`                              | Optional — could serve from cache; Phase 2+                        |
| `community_import_profile` | `fn(String, State<ProfileStore>, State<SettingsStore>, State<CommunityTapStore>) -> Result<CommunityImportResult, String>` | **Yes** — create profile identity post-import                      |
| `community_sync`           | `fn(State<SettingsStore>, State<CommunityTapStore>) -> Result<Vec<CommunityTapSyncResult>, String>`                        | **Yes** — `sync_tap_index()` after `tap_store.sync_many()` returns |

### 1.5 Settings Commands (`commands/settings.rs`)

Settings commands (`settings_load`, `settings_save`, `recent_files_load`, `recent_files_save`) do not require direct metadata hooks for Phase 1. Community tap subscriptions are managed via `community_add_tap`.

---

## 2. Store APIs

### 2.1 `ProfileStore` (`crates/crosshook-core/src/profile/toml_store.rs`)

**Construction:**

```rust
ProfileStore::try_new() -> Result<ProfileStore, String>
ProfileStore::new() -> ProfileStore
ProfileStore::with_base_path(base_path: PathBuf) -> ProfileStore
```

**Public API:**

```rust
fn load(&self, name: &str) -> Result<GameProfile, ProfileStoreError>
fn save(&self, name: &str, profile: &GameProfile) -> Result<(), ProfileStoreError>
fn save_launch_optimizations(&self, name: &str, enabled_option_ids: Vec<String>) -> Result<(), ProfileStoreError>
fn list(&self) -> Result<Vec<String>, ProfileStoreError>
fn delete(&self, name: &str) -> Result<(), ProfileStoreError>
fn rename(&self, old_name: &str, new_name: &str) -> Result<(), ProfileStoreError>
fn import_legacy(&self, legacy_path: &Path) -> Result<GameProfile, ProfileStoreError>
fn duplicate(&self, source_name: &str) -> Result<DuplicateProfileResult, ProfileStoreError>
```

**Path derivation:**

- `profile_path(name)` → `{base_path}/{name}.toml` (private, used internally)
- `base_path` is public; metadata sync uses `store.list()` to discover all profile filenames

**`ProfileStoreError` variants:**

```rust
InvalidName(String)
NotFound(PathBuf)
AlreadyExists(String)
InvalidLaunchOptimizationId(String)
Io(std::io::Error)
TomlDe(toml::de::Error)
TomlSer(toml::ser::Error)
```

**`DuplicateProfileResult`:**

```rust
pub struct DuplicateProfileResult {
    pub name: String,        // generated copy name, e.g. "MyGame (Copy)"
    pub profile: GameProfile, // byte-for-byte clone of source
}
```

**`validate_name(name: &str) -> Result<(), ProfileStoreError>`** — public function used for all name validation. Rules: non-empty, not `"."` or `".."`, no absolute path, no `/` `\` `:` `<` `>` `"` `|` `?` `*`.

**Metadata sync is NOT inside `ProfileStore`** — the store is a pure TOML I/O layer. Sync hooks fire in Tauri commands after successful store operations.

### 2.2 Launcher Functions (`crates/crosshook-core/src/export/launcher_store.rs`)

These are free functions (not a struct), re-exported via `crosshook_core::export::*`:

```rust
// Check existence and staleness
pub fn check_launcher_exists(
    display_name: &str,
    steam_app_id: &str,
    trainer_path: &str,
    target_home_path: &str,
    steam_client_install_path: &str,
) -> Result<LauncherInfo, LauncherStoreError>

pub fn check_launcher_exists_for_request(
    display_name: &str,
    request: &SteamExternalLauncherExportRequest,
) -> Result<LauncherInfo, LauncherStoreError>

pub fn check_launcher_for_profile(
    profile: &GameProfile,
    target_home_path: &str,
    steam_client_install_path: &str,
) -> Result<LauncherInfo, LauncherStoreError>

// Delete operations
pub fn delete_launcher_files(
    display_name: &str,
    steam_app_id: &str,
    trainer_path: &str,
    target_home_path: &str,
    steam_client_install_path: &str,
) -> Result<LauncherDeleteResult, LauncherStoreError>

pub fn delete_launcher_by_slug(
    launcher_slug: &str,
    target_home_path: &str,
    steam_client_install_path: &str,
) -> Result<LauncherDeleteResult, LauncherStoreError>

pub fn delete_launcher_for_profile(
    profile: &GameProfile,
    target_home_path: &str,
    steam_client_install_path: &str,
) -> Result<LauncherDeleteResult, LauncherStoreError>

// Rename
pub fn rename_launcher_files(
    old_launcher_slug: &str,
    new_display_name: &str,
    new_launcher_icon_path: &str,
    target_home_path: &str,
    steam_client_install_path: &str,
    request: &SteamExternalLauncherExportRequest,
) -> Result<LauncherRenameResult, LauncherStoreError>

// List / discovery
pub fn list_launchers(
    target_home_path: &str,
    steam_client_install_path: &str,
) -> Vec<LauncherInfo>

pub fn find_orphaned_launchers(
    known_profile_slugs: &[String],
    target_home_path: &str,
    steam_client_install_path: &str,
) -> Vec<LauncherInfo>
```

**`LauncherInfo`:**

```rust
pub struct LauncherInfo {
    pub display_name: String,
    pub launcher_slug: String,
    pub script_path: String,
    pub desktop_entry_path: String,
    pub script_exists: bool,
    pub desktop_entry_exists: bool,
    pub is_stale: bool,        // only meaningful from check_launcher_*; list_launchers reports false
}
```

**`LauncherDeleteResult`:**

```rust
pub struct LauncherDeleteResult {
    pub script_deleted: bool,
    pub desktop_entry_deleted: bool,
    pub script_path: String,
    pub desktop_entry_path: String,
    pub script_skipped_reason: Option<String>,
    pub desktop_entry_skipped_reason: Option<String>,
}
```

**`LauncherRenameResult`:**

```rust
pub struct LauncherRenameResult {
    pub old_slug: String,
    pub new_slug: String,
    pub new_script_path: String,
    pub new_desktop_entry_path: String,
    pub script_renamed: bool,
    pub desktop_entry_renamed: bool,
    pub old_script_cleanup_warning: Option<String>,
    pub old_desktop_entry_cleanup_warning: Option<String>,
}
```

**`LauncherStoreError` variants:**

```rust
Io(io::Error)
HomePathResolutionFailed
```

**Watermarks** (important for metadata sync — never record an artifact as owned if watermark verification would fail):

- Script: `"# Generated by CrossHook"` (constant `SCRIPT_WATERMARK`)
- Desktop entry: `"Generated by CrossHook"` (constant `DESKTOP_ENTRY_WATERMARK`)

**`SteamExternalLauncherExportRequest`** (`export/launcher.rs:14-26`):

```rust
pub struct SteamExternalLauncherExportRequest {
    pub method: String,
    pub launcher_name: String,
    pub trainer_path: String,
    pub trainer_loading_mode: TrainerLoadingMode,
    pub launcher_icon_path: String,
    pub prefix_path: String,
    pub proton_path: String,
    pub steam_app_id: String,
    pub steam_client_install_path: String,
    pub target_home_path: String,
}
```

**`SteamExternalLauncherExportResult`** (`export/launcher.rs:28-33`):

```rust
pub struct SteamExternalLauncherExportResult {
    pub display_name: String,
    pub launcher_slug: String,
    pub script_path: String,
    pub desktop_entry_path: String,
}
```

**Launcher path derivation** (`derive_launcher_paths` in `launcher_store.rs`):

- Script: `{home}/.local/share/crosshook/launchers/{slug}-trainer.sh`
- Desktop entry: `{home}/.local/share/applications/crosshook-{slug}-trainer.desktop`
- Slug: `sanitize_launcher_slug(resolved_name)` from `export/launcher.rs`

### 2.3 `CommunityTapStore` (`crates/crosshook-core/src/community/taps.rs`)

**Construction:**

```rust
CommunityTapStore::try_new() -> Result<CommunityTapStore, String>
CommunityTapStore::new() -> CommunityTapStore
CommunityTapStore::with_base_path(base_path: PathBuf) -> CommunityTapStore
```

**Public API:**

```rust
pub fn resolve_workspace(
    &self,
    subscription: &CommunityTapSubscription,
) -> Result<CommunityTapWorkspace, CommunityTapError>

pub fn sync_tap(
    &self,
    subscription: &CommunityTapSubscription,
) -> Result<CommunityTapSyncResult, CommunityTapError>

pub fn sync_many(
    &self,
    subscriptions: &[CommunityTapSubscription],
) -> Result<Vec<CommunityTapSyncResult>, CommunityTapError>

pub fn index_workspaces(
    &self,
    workspaces: &[CommunityTapWorkspace],
) -> Result<CommunityProfileIndex, CommunityTapError>
```

**Key types:**

```rust
pub struct CommunityTapSubscription {
    pub url: String,
    pub branch: Option<String>,   // defaults to "main" when None
}

pub struct CommunityTapWorkspace {
    pub subscription: CommunityTapSubscription,
    pub local_path: PathBuf,
}

pub struct CommunityTapSyncResult {
    pub workspace: CommunityTapWorkspace,
    pub status: CommunityTapSyncStatus,  // Cloned | Updated
    pub head_commit: String,             // output of `git rev-parse HEAD` — KEY for idempotent re-index
    pub index: CommunityProfileIndex,
}

pub enum CommunityTapSyncStatus {
    Cloned,
    Updated,
}
```

**Metadata integration point**: After `community_sync` calls `tap_store.sync_many()`, each `CommunityTapSyncResult.head_commit` is compared against `community_taps.last_synced_commit` in SQLite. Re-indexing `community_profiles` only happens when HEAD changed.

### 2.4 `SettingsStore` (`crates/crosshook-core/src/settings/mod.rs`)

```rust
pub struct SettingsStore { pub base_path: PathBuf }

pub struct AppSettingsData {
    pub auto_load_last_profile: bool,
    pub last_used_profile: String,
    pub community_taps: Vec<CommunityTapSubscription>,
}

// API
pub fn load(&self) -> Result<AppSettingsData, SettingsStoreError>
pub fn save(&self, settings: &AppSettingsData) -> Result<(), SettingsStoreError>
pub fn settings_path(&self) -> PathBuf  // → {base_path}/settings.toml
```

**Community taps list lives in `AppSettingsData.community_taps`**. SQLite only mirrors tap _sync state_ and catalog cache — it does not own the authoritative taps list (TOML is authoritative).

### 2.5 `RecentFilesStore` (`crates/crosshook-core/src/settings/recent.rs`)

```rust
pub struct RecentFilesStore { pub path: PathBuf }

pub struct RecentFilesData {
    pub game_paths: Vec<String>,     // max 10, existing files only on load
    pub trainer_paths: Vec<String>,  // max 10
    pub dll_paths: Vec<String>,      // max 10
}

// API
pub fn load(&self) -> Result<RecentFilesData, RecentFilesStoreError>
pub fn save(&self, recent_files: &RecentFilesData) -> Result<(), RecentFilesStoreError>
```

**Phase 1**: No SQLite integration. Phase 2+ may replace with `recent_file_entry` table with timestamps.

---

## 3. Launch System

### 3.1 `LaunchRequest` (`crates/crosshook-core/src/launch/request.rs`)

```rust
pub struct LaunchRequest {
    pub method: String,                              // "steam_applaunch" | "proton_run" | "native"
    pub game_path: String,
    pub trainer_path: String,
    pub trainer_host_path: String,
    pub trainer_loading_mode: TrainerLoadingMode,
    pub steam: SteamLaunchConfig,
    pub runtime: RuntimeLaunchConfig,
    pub optimizations: LaunchOptimizationsRequest,
    pub launch_trainer_only: bool,
    pub launch_game_only: bool,
}

pub struct SteamLaunchConfig {
    pub app_id: String,
    pub compatdata_path: String,
    pub proton_path: String,
    pub steam_client_install_path: String,
}

pub struct RuntimeLaunchConfig {
    pub prefix_path: String,
    pub proton_path: String,
    pub working_directory: String,
}

pub struct LaunchOptimizationsRequest {
    pub enabled_option_ids: Vec<String>,
}
```

**Key methods:**

```rust
fn resolved_method(&self) -> &str  // resolves ambiguous method to one of the 3 constants
fn log_target_slug(&self) -> String // slug for log file naming
fn should_copy_trainer_to_prefix(&self) -> bool
```

**Constants:**

```rust
pub const METHOD_STEAM_APPLAUNCH: &str = "steam_applaunch";
pub const METHOD_PROTON_RUN: &str = "proton_run";
pub const METHOD_NATIVE: &str = "native";
```

**What to store in `launch_operations` vs. what NOT to store:**

- ✅ Store: `method`, `game_path`, `trainer_path`, `steam.app_id`, `launch_game_only`, `launch_trainer_only`, started/ended timestamps, exit code, signal
- ❌ Never store: full CLI argument lists, `steam.compatdata_path`, `steam.proton_path`, `runtime.*`, `optimizations.enabled_option_ids`, environment variables (potential credential leakage)

### 3.2 Launch Flow and Event Emission

**`launch_game` / `launch_trainer`** (both `async fn` in `commands/launch.rs`):

1. Set `launch_game_only` / `launch_trainer_only` on request
2. Call `validate()` — synchronous
3. Resolve method → build command
4. Spawn child process
5. Call `spawn_log_stream(app, log_path, child, method)` — spawns background task

**`stream_log_lines`** (background async fn — metadata hook insertion point):

```
loop:
  → read log file → emit "launch-log" events
  → check child.try_wait()
  → sleep 500ms

on process exit:
  → final log read + emit remaining "launch-log" events
  → extract exit_code (status.code()) and signal (status.signal())
  → read log tail via safe_read_tail()
  → call analyze(exit_status, &log_tail, method) → DiagnosticReport
  → sanitize_diagnostic_report(report)
  → if should_surface_report(&report): emit "launch-diagnostic" event
  → emit "launch-complete" { code, signal }
```

**Metadata hook for launch must use `tokio::task::spawn_blocking`** because `rusqlite` is synchronous and `stream_log_lines` runs in an async context.

**Proposed insertion in `stream_log_lines`**:

```
// After line 204 (exit_status extraction), before emit:
// 1. record_launch_started() called in launch_game/launch_trainer at start of launch
// 2. record_launch_finished(operation_id, exit_code, signal, &report) called here
```

**`DiagnosticReport`** (`crates/crosshook-core/src/launch/diagnostics/models.rs`):

```rust
pub struct DiagnosticReport {
    pub severity: ValidationSeverity,     // Fatal | Warning | Info
    pub summary: String,
    pub exit_info: ExitCodeInfo,
    pub pattern_matches: Vec<PatternMatch>,
    pub suggestions: Vec<ActionableSuggestion>,
    pub launch_method: String,
    pub log_tail_path: Option<String>,
    pub analyzed_at: String,
}

pub struct ExitCodeInfo {
    pub code: Option<i32>,
    pub signal: Option<i32>,
    pub signal_name: Option<String>,
    pub core_dumped: bool,
    pub failure_mode: FailureMode,
    pub description: String,
    pub severity: ValidationSeverity,
}

pub enum FailureMode {
    CleanExit, NonZeroExit, Segfault, Abort, Kill, BusError,
    IllegalInstruction, FloatingPointException, BrokenPipe,
    Terminated, CommandNotFound, PermissionDenied, UnknownSignal,
    Indeterminate, Unknown,
}

pub struct PatternMatch {
    pub pattern_id: String,
    pub summary: String,
    pub severity: ValidationSeverity,
    pub matched_line: Option<String>,
    pub suggestion: String,
}

pub struct ActionableSuggestion {
    pub title: String,
    pub description: String,
    pub severity: ValidationSeverity,
}
```

**Constants in diagnostics:**

```rust
pub const MAX_LOG_TAIL_BYTES: u64 = 2 * 1024 * 1024;  // 2 MB
pub const MAX_DIAGNOSTIC_ENTRIES: usize = 50;
pub const MAX_LINE_DISPLAY_CHARS: usize = 500;
```

**Sanitization** (`sanitize_display_path` in `commands/launch.rs:301-306`):

- Replaces `$HOME` prefix with `~` in string paths
- Applied to `report.summary`, `report.exit_info.description`, `report.launch_method`, `report.log_tail_path`, pattern match strings, and suggestion strings
- Must be applied before storing to SQLite (store sanitized versions)

### 3.3 Log Path Creation

**`create_log_path(kind, slug)`** (from `commands/shared.rs`):

- Creates timestamped log files in the app data log directory
- Path stored in `LaunchResult.helper_log_path` — this is what `launch_operations.log_path` stores

---

## 4. Filesystem Layout

All paths use XDG conventions via `directories::BaseDirs`.

### 4.1 Config Directory (`~/.config/crosshook/`)

| Path                                  | Purpose                                                               | Authority                     |
| ------------------------------------- | --------------------------------------------------------------------- | ----------------------------- |
| `~/.config/crosshook/profiles/*.toml` | GameProfile TOML files (one per profile, filename = profile name)     | **Filesystem/TOML canonical** |
| `~/.config/crosshook/settings.toml`   | `AppSettingsData` (auto_load, last_used_profile, community_taps list) | Filesystem canonical          |

**ProfileStore base**: `BaseDirs::config_dir().join("crosshook/profiles")`
**SettingsStore base**: `BaseDirs::config_dir().join("crosshook")`

### 4.2 Data Directory (`~/.local/share/crosshook/`)

| Path                                       | Purpose                                                              | Authority            |
| ------------------------------------------ | -------------------------------------------------------------------- | -------------------- |
| `~/.local/share/crosshook/metadata.db`     | **SQLite database** (target location for new feature)                | SQLite canonical     |
| `~/.local/share/crosshook/metadata.db-wal` | WAL sidecar — created automatically in WAL mode                      | SQLite managed       |
| `~/.local/share/crosshook/metadata.db-shm` | WAL shared memory — created automatically                            | SQLite managed       |
| `~/.local/share/crosshook/community/taps/` | CommunityTapStore base (one subdirectory per tap, named by URL slug) | Filesystem canonical |
| `~/.local/share/crosshook/launchers/`      | Exported trainer launcher `.sh` scripts                              | Filesystem canonical |
| `~/.local/share/crosshook/recent.toml`     | RecentFilesData (game/trainer/dll paths)                             | Filesystem canonical |

**CommunityTapStore base**: `BaseDirs::data_local_dir().join("crosshook/community/taps")`
**RecentFilesStore path**: `BaseDirs::data_local_dir().join("crosshook/recent.toml")`

### 4.3 Desktop Entries (`~/.local/share/applications/`)

| Path Pattern                                                   | Purpose                        | Authority            |
| -------------------------------------------------------------- | ------------------------------ | -------------------- |
| `~/.local/share/applications/crosshook-{slug}-trainer.desktop` | XDG desktop entry for launcher | Filesystem canonical |

### 4.4 Log Files

Log files are written during launch. Path stored in `LaunchResult.helper_log_path`. Exact directory determined by `commands/shared.rs:create_log_path`. Full path format: `{log_dir}/{kind}-{slug}-{timestamp}.log`.

### 4.5 Security Constraints for DB File

Per the feature spec security requirements:

- `metadata.db` must be created with permissions `0600` (via `set_permissions()` after `Connection::open()`)
- Parent directory `~/.local/share/crosshook/` must be `0700`
- Before opening: verify path is a regular file (not a symlink) via `symlink_metadata()`
- WAL (`.db-wal`) and SHM (`.db-shm`) sidecars inherit directory permissions

---

## 5. Community System

### 5.1 `CommunityTapSubscription` (authoritative source: `AppSettingsData.community_taps`)

```rust
pub struct CommunityTapSubscription {
    pub url: String,           // git repository URL
    pub branch: Option<String>, // None → uses "main"
}
```

**SQLite mirror** (`community_taps` table): Keyed on `(tap_url, tap_branch)` composite — matches the subscription identity. SQLite mirrors sync state and catalog cache only; it never adds or removes subscriptions (TOML is authoritative).

**Deduplication**: `community_add_tap` in `commands/community.rs` deduplicates by `(url, branch)` before persisting.

### 5.2 `CommunityTapSyncResult` (produced by `sync_many`)

```rust
pub struct CommunityTapSyncResult {
    pub workspace: CommunityTapWorkspace,
    pub status: CommunityTapSyncStatus,  // Cloned | Updated
    pub head_commit: String,             // SHA from `git rev-parse HEAD`
    pub index: CommunityProfileIndex,    // full parsed index for this tap
}
```

**Key integration decision**: `head_commit` enables idempotent re-indexing. When `community_sync` runs:

1. Compare each result's `head_commit` against `community_taps.last_synced_commit` in SQLite
2. Only update `community_profiles` rows for taps where HEAD changed
3. Update `community_taps.last_synced_commit` and `last_synced_at` for all synced taps

### 5.3 `CommunityProfileIndex` and `CommunityProfileIndexEntry`

```rust
pub struct CommunityProfileIndex {
    pub entries: Vec<CommunityProfileIndexEntry>,
    pub diagnostics: Vec<String>,         // non-fatal parse warnings
}

pub struct CommunityProfileIndexEntry {
    pub tap_url: String,
    pub tap_branch: Option<String>,
    pub tap_path: PathBuf,                // absolute local path to tap workspace
    pub manifest_path: PathBuf,           // absolute path to community-profile.json
    pub relative_path: PathBuf,           // relative to tap root, e.g. "profiles/elden-ring/community-profile.json"
    pub manifest: CommunityProfileManifest,
}
```

**Manifest scanning**: `index_tap` in `community/index.rs` walks the tap workspace recursively, collecting all `community-profile.json` files. Skips hidden directories, skips manifests with `schema_version != 1`.

### 5.4 `CommunityProfileManifest` and `CommunityProfileMetadata`

```rust
pub struct CommunityProfileManifest {
    pub schema_version: u32,              // currently 1
    pub metadata: CommunityProfileMetadata,
    pub profile: GameProfile,             // embedded profile — NOT stored in SQLite
}

pub struct CommunityProfileMetadata {
    pub game_name: String,
    pub game_version: String,
    pub trainer_name: String,
    pub trainer_version: String,
    pub proton_version: String,
    pub platform_tags: Vec<String>,       // stored as JSON array in community_profiles
    pub compatibility_rating: CompatibilityRating,
    pub author: String,
    pub description: String,
}

pub enum CompatibilityRating {
    Unknown, Broken, Partial, Working, Platinum,
}
```

**Serialized as snake_case** (`#[serde(rename_all = "snake_case")]` on `CompatibilityRating`).

**Community profile files are JSON** (`community-profile.json`), not TOML. `CommunityProfileManifest` is serialized with `serde_json`.

**SQLite storage** (`community_profiles` table): Only metadata fields are stored — never the embedded `GameProfile` (which is stored on disk in the tap workspace). `platform_tags` stored as JSON array string.

### 5.5 Community Import Flow

`community_import_profile` in `commands/community.rs`:

1. Validates the import path is inside a known tap workspace
2. Calls `import_community_profile(import_path, &profile_store.base_path)` from `crosshook_core::profile`
3. Returns `CommunityImportResult`

**Metadata hook**: After successful `community_import_profile`, create a new profile identity with source tagged as `"import"` in `profile_name_history.source`.

---

## 6. Tauri State Setup (`src-tauri/src/lib.rs`)

Current managed state (lines 62-66):

```rust
.manage(profile_store)          // ProfileStore
.manage(settings_store)         // SettingsStore
.manage(recent_files_store)     // RecentFilesStore
.manage(community_tap_store)    // CommunityTapStore
.manage(commands::update::UpdateProcessState::new())
```

**MetadataStore addition**:

```rust
.manage(metadata_store)         // Option<MetadataStore> — fail-soft; None if DB unavailable
```

MetadataStore must implement `Clone + Send + Sync`. Internal connection uses `Arc<Mutex<Connection>>` pattern.

**Startup sequence** (`startup.rs`): Currently `resolve_auto_load_profile_name` → emits `"auto-load-profile"` event. MetadataStore initialization should happen in `run()` before `.manage()`. A full startup sync scan (`sync_profiles_from_store`) should be scheduled as a background task after the app setup completes, consistent with the existing pattern of spawning async tasks in the setup closure.

---

## 7. Edge Cases and Gotchas

- **`list_launchers` sets `is_stale = false`**: The function does not have profile context, so staleness cannot be computed. Only `check_launcher_exists` / `check_launcher_exists_for_request` set `is_stale` accurately. Metadata bulk sync from `list_launchers` should record `is_stale = false` without overwriting values computed via `check_launcher_exists`.

- **Launcher paths are home-relative**: `derive_launcher_paths` calls `resolve_target_home_path` which falls back to `$HOME` when `target_home_path` and `steam_client_install_path` are both empty. SQLite must store absolute paths after resolution, not the raw inputs.

- **Watermark verification before marking owned**: `delete_launcher_at_paths` verifies both watermarks before deleting. Metadata should never record an artifact as CrossHook-managed if the watermark check would fail. The `script_skipped_reason` / `desktop_entry_skipped_reason` fields on `LauncherDeleteResult` indicate when watermark verification failed.

- **`profile_rename` has a `had_launcher` return value**: The `bool` returned indicates whether a launcher existed before the rename. The Tauri command does launcher cleanup internally; the metadata hook runs after the rename and can use this information to decide whether to update launcher observations.

- **`stream_log_lines` completion is async**: Metadata writes for `record_launch_finished` must use `tokio::task::spawn_blocking` to avoid blocking the async runtime. The `operation_id` (from `record_launch_started`) must be threaded through the async context.

- **`community_sync` returns `Vec<CommunityTapSyncResult>`**: The full index is embedded in each result. Metadata indexing iterates this result set — no second pass is needed.

- **`community_list_profiles` does NOT call `sync_many`**: It only calls `index_workspaces` on already-resolved workspaces. It will not trigger metadata sync. Only `community_sync` triggers tap sync and should trigger metadata updates.

- **Schema version check in `index_tap`**: Manifests with `schema_version != 1` are skipped with a diagnostic message. Metadata indexing should replicate this behavior — do not insert `community_profiles` rows for unsupported schema versions.

- **`CommunityTapStore` base path**: Uses `data_local_dir()` (not `config_dir()`). The per-tap workspace directory name is derived by `CommunityTapSubscription::directory_name()` (URL + branch slugified). The actual directory name is not stable if the URL changes — SQLite should key on `(tap_url, tap_branch)`, not on the directory name.

- **`validate_name` is a public function**: Both `ProfileStore` and the new metadata module should call `validate_name` when resolving stored name values for filesystem operations. Never assume a name stored in SQLite has already been validated.

---

## Relevant Files

| File                                                                          | Description                                                                               |
| ----------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------- |
| `src/crosshook-native/src-tauri/src/commands/profile.rs`                      | Profile Tauri commands — all sync hooks fire here                                         |
| `src/crosshook-native/src-tauri/src/commands/export.rs`                       | Launcher Tauri commands — launcher sync hooks fire here                                   |
| `src/crosshook-native/src-tauri/src/commands/launch.rs`                       | Launch commands + `stream_log_lines` — launch history hooks fire here                     |
| `src/crosshook-native/src-tauri/src/commands/community.rs`                    | Community tap commands — tap index sync fires here                                        |
| `src/crosshook-native/src-tauri/src/lib.rs`                                   | Tauri app setup — MetadataStore added to `.manage()` here                                 |
| `src/crosshook-native/src-tauri/src/startup.rs`                               | Startup profile resolution — startup reconciliation scan added here                       |
| `src/crosshook-native/crates/crosshook-core/src/profile/toml_store.rs`        | ProfileStore API + `validate_name` + `DuplicateProfileResult`                             |
| `src/crosshook-native/crates/crosshook-core/src/export/launcher_store.rs`     | Launcher free functions + `LauncherInfo`, `LauncherDeleteResult`, `LauncherRenameResult`  |
| `src/crosshook-native/crates/crosshook-core/src/export/launcher.rs`           | `SteamExternalLauncherExportRequest`, `sanitize_launcher_slug`, path derivation           |
| `src/crosshook-native/crates/crosshook-core/src/launch/request.rs`            | `LaunchRequest`, `SteamLaunchConfig`, method constants                                    |
| `src/crosshook-native/crates/crosshook-core/src/launch/diagnostics/models.rs` | `DiagnosticReport`, `ExitCodeInfo`, `FailureMode`, `PatternMatch`, `ActionableSuggestion` |
| `src/crosshook-native/crates/crosshook-core/src/community/taps.rs`            | `CommunityTapStore`, `CommunityTapSubscription`, `CommunityTapSyncResult`                 |
| `src/crosshook-native/crates/crosshook-core/src/community/index.rs`           | `CommunityProfileIndex`, `CommunityProfileIndexEntry`, `index_tap`, `index_taps`          |
| `src/crosshook-native/crates/crosshook-core/src/profile/community_schema.rs`  | `CommunityProfileManifest`, `CommunityProfileMetadata`, `CompatibilityRating`             |
| `src/crosshook-native/crates/crosshook-core/src/settings/mod.rs`              | `SettingsStore`, `AppSettingsData`                                                        |
| `src/crosshook-native/crates/crosshook-core/src/settings/recent.rs`           | `RecentFilesStore`, `RecentFilesData`                                                     |
| `src/crosshook-native/crates/crosshook-core/src/lib.rs`                       | Module root — `pub mod metadata;` added here                                              |
