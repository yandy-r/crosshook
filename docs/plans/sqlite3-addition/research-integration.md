# Integration Research: SQLite Metadata Layer Phase 2 (Operational History)

All findings are verified against source code as of 2026-03-27 on branch `feat/sqlite3-addition`.

---

## Launch System APIs

### LaunchRequest (current fields, profile_name gap)

**File:** `src/crosshook-native/crates/crosshook-core/src/launch/request.rs:16-37`

```rust
pub struct LaunchRequest {
    pub method: String,              // "steam_applaunch" | "proton_run" | "native"
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
```

Sub-structs:

- `SteamLaunchConfig` (lines 42-51): `app_id`, `compatdata_path`, `proton_path`, `steam_client_install_path`
- `RuntimeLaunchConfig` (lines 53-61): `prefix_path`, `proton_path`, `working_directory`
- `LaunchOptimizationsRequest` (lines 63-71): `enabled_option_ids: Vec<String>`

**profile_name gap:** `LaunchRequest` has no `profile_name` field. The name of the profile that produced the request is never propagated into the struct. This is the primary blocker for linking `launch_operations` rows to a `profile_id` in the metadata DB.

The `log_target_slug()` method (lines 104-136) derives a filesystem-safe slug from `steam.app_id` or `game_path`, but this is not the profile name and cannot be used to look up `profile_id`.

### Script Runner (execution flow, outcomes)

**File:** `src/crosshook-native/crates/crosshook-core/src/launch/script_runner.rs`

Four public builder functions — none returns an outcome; they only build a `tokio::process::Command`:

- `build_helper_command(request, script_path, log_path) -> Command` — Steam applaunch path, delegates to a shell script
- `build_trainer_command(request, script_path, log_path) -> Command` — Steam trainer launch via shell script
- `build_proton_game_command(request, log_path) -> io::Result<Command>` — Direct Proton execution for game
- `build_proton_trainer_command(request, log_path) -> io::Result<Command>` — Direct Proton execution for trainer; may stage trainer into prefix
- `build_native_game_command(request, log_path) -> io::Result<Command>` — Native Linux game execution

Outcome data is only available in the Tauri layer after the child process exits — script_runner itself has no outcome/return-value concept.

### Launch Tauri Commands (signatures, async patterns, hook points)

**File:** `src/crosshook-native/src-tauri/src/commands/launch.rs`

**Public commands registered in `lib.rs`:**

| Command                              | Signature                                                            | Async |
| ------------------------------------ | -------------------------------------------------------------------- | ----- |
| `launch_game`                        | `async fn(AppHandle, LaunchRequest) -> Result<LaunchResult, String>` | Yes   |
| `launch_trainer`                     | `async fn(AppHandle, LaunchRequest) -> Result<LaunchResult, String>` | Yes   |
| `validate_launch`                    | `fn(LaunchRequest) -> Result<(), LaunchValidationIssue>`             | No    |
| `preview_launch`                     | `fn(LaunchRequest) -> Result<LaunchPreview, String>`                 | No    |
| `build_steam_launch_options_command` | `fn(Vec<String>) -> Result<String, String>`                          | No    |

**`LaunchResult` struct (lines 23-27):**

```rust
pub struct LaunchResult {
    pub succeeded: bool,
    pub message: String,
    pub helper_log_path: String,
}
```

**Async execution model in `launch_game` / `launch_trainer` (lines 48-123):**

1. Mutate `request` to set `launch_game_only` / `launch_trainer_only`.
2. Validate request.
3. Resolve `method` as a `&'static str`.
4. Create `log_path` via `create_log_path(prefix, slug)`.
5. Build `Command` via script_runner.
6. Spawn child process.
7. Call `spawn_log_stream(app, log_path, child, method)` — this detaches a background async task.
8. Return `LaunchResult` immediately (fire-and-forget pattern).

**`spawn_log_stream` / `stream_log_lines` (lines 125-230):**

The background task polls the log file and the child exit status every 500ms. When the child exits, it:

- Does a final log tail read.
- Calls `analyze(exit_status, &log_tail, method)` to produce a `DiagnosticReport`.
- Emits `"launch-diagnostic"` event if the report should be surfaced.
- Emits `"launch-complete"` event with `{ code, signal }`.

**Phase 2 hook points:**

- `record_launch_started` should be called in `launch_game` / `launch_trainer` after validation passes and before `command.spawn()` — line ~72 in `launch_game`.
- `record_launch_finished` should be called inside `stream_log_lines` after the child exits (around line 203), after `exit_code` and `signal` are resolved and before/after `analyze()`.
- The `operation_id` returned by `record_launch_started` must be threaded from the command function into `spawn_log_stream` and then into `stream_log_lines`. Currently `spawn_log_stream` takes `(app, log_path, child, method)` — a fifth `operation_id: String` parameter would be needed.

**Important constraint:** `launch_game` and `launch_trainer` do not take `State<MetadataStore>` — it is managed via `app.state::<MetadataStore>()` pattern. The `AppHandle` is already available in both commands; `app.state::<MetadataStore>()` can be called without changing signatures.

---

## Export/Launcher APIs

### LauncherStore (full API surface)

**File:** `src/crosshook-native/crates/crosshook-core/src/export/launcher_store.rs`

Public free functions (no struct — all are module-level):

| Function                            | Signature                                                                                                                                                                                    | Returns                                                      |
| ----------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------ |
| `check_launcher_exists`             | `(display_name, steam_app_id, trainer_path, target_home_path, steam_client_install_path) -> Result<LauncherInfo, LauncherStoreError>`                                                        | Checks by deriving slug from fields                          |
| `check_launcher_exists_for_request` | `(display_name: &str, request: &SteamExternalLauncherExportRequest) -> Result<LauncherInfo, LauncherStoreError>`                                                                             | Full staleness check (script content + Name= comparison)     |
| `check_launcher_for_profile`        | `(profile: &GameProfile, target_home_path, steam_client_install_path) -> Result<LauncherInfo, LauncherStoreError>`                                                                           | Profile-aware staleness; returns default for `native` method |
| `delete_launcher_files`             | `(display_name, steam_app_id, trainer_path, target_home_path, steam_client_install_path) -> Result<LauncherDeleteResult, LauncherStoreError>`                                                | Deletes with watermark guard                                 |
| `delete_launcher_by_slug`           | `(launcher_slug, target_home_path, steam_client_install_path) -> Result<LauncherDeleteResult, LauncherStoreError>`                                                                           | Slug-based delete                                            |
| `delete_launcher_for_profile`       | `(profile: &GameProfile, target_home_path, steam_client_install_path) -> Result<LauncherDeleteResult, LauncherStoreError>`                                                                   | Profile-aware delete                                         |
| `rename_launcher_files`             | `(old_slug, new_display_name, new_icon_path, target_home_path, steam_client_install_path, request: &SteamExternalLauncherExportRequest) -> Result<LauncherRenameResult, LauncherStoreError>` | Write-then-delete rename                                     |
| `list_launchers`                    | `(target_home_path, steam_client_install_path) -> Vec<LauncherInfo>`                                                                                                                         | Directory scan, `is_stale=false`                             |

**`LauncherInfo` struct (lines 28-43):**

```rust
pub struct LauncherInfo {
    pub display_name: String,
    pub launcher_slug: String,
    pub script_path: String,
    pub desktop_entry_path: String,
    pub script_exists: bool,
    pub desktop_entry_exists: bool,
    pub is_stale: bool,
}
```

**`SteamExternalLauncherExportResult` struct (launcher.rs lines 29-34):**

```rust
pub struct SteamExternalLauncherExportResult {
    pub display_name: String,
    pub launcher_slug: String,
    pub script_path: String,
    pub desktop_entry_path: String,
}
```

### Launcher Generation (slug, paths)

**File:** `src/crosshook-native/crates/crosshook-core/src/export/launcher.rs`

Slug derivation: `sanitize_launcher_slug(value: &str) -> String` (line 265).

- Maps characters to lowercase ASCII alphanumeric, collapses non-alnum runs to `-`, trims leading/trailing `-`.
- Falls back to `"crosshook-trainer"` for blank input.

Path pattern:

- Script: `{home}/.local/share/crosshook/launchers/{slug}-trainer.sh`
- Desktop: `{home}/.local/share/applications/crosshook-{slug}-trainer.desktop`

Display name resolution: `resolve_display_name(preferred_name, steam_app_id, trainer_path)` (line 230) — uses `preferred_name` first, then trainer file stem, then `app-{steam_app_id}`.

Export entry point: `export_launchers(request: &SteamExternalLauncherExportRequest) -> Result<SteamExternalLauncherExportResult, SteamExternalLauncherExportError>` (line 175).

### Export Tauri Commands (signatures, hook points)

**File:** `src/crosshook-native/src-tauri/src/commands/export.rs`

| Command                      | Signature                                                                                     |
| ---------------------------- | --------------------------------------------------------------------------------------------- |
| `export_launchers`           | `fn(SteamExternalLauncherExportRequest) -> Result<SteamExternalLauncherExportResult, String>` |
| `validate_launcher_export`   | `fn(SteamExternalLauncherExportRequest) -> Result<(), String>`                                |
| `check_launcher_exists`      | `fn(SteamExternalLauncherExportRequest) -> Result<LauncherInfo, String>`                      |
| `check_launcher_for_profile` | `fn(String, State<ProfileStore>) -> Result<LauncherInfo, String>`                             |
| `delete_launcher`            | `fn(String, String, String, String, String) -> Result<LauncherDeleteResult, String>`          |
| `delete_launcher_by_slug`    | `fn(String, String, String) -> Result<LauncherDeleteResult, String>`                          |
| `rename_launcher`            | `fn(String × 10) -> Result<LauncherRenameResult, String>`                                     |
| `list_launchers`             | `fn(String, String) -> Vec<LauncherInfo>`                                                     |
| `find_orphaned_launchers`    | `fn(Vec<String>, String, String) -> Vec<LauncherInfo>`                                        |
| `preview_launcher_script`    | `fn(SteamExternalLauncherExportRequest) -> Result<String, String>`                            |
| `preview_launcher_desktop`   | `fn(SteamExternalLauncherExportRequest) -> Result<String, String>`                            |

**Phase 2 hook points for `observe_launcher_exported`:**

- `export_launchers` command (line 20): After `export_launchers_core(&request)` returns `Ok(result)`, call `observe_launcher_exported(profile_name, &result.launcher_slug, &result.script_path, &result.desktop_entry_path)`.
- **Gap:** The `export_launchers` command takes a `SteamExternalLauncherExportRequest` — it has `launcher_name` but no `profile_name`. The profile name must be added to the request struct or passed as a separate parameter to know which `profile_id` to associate.
- `export_launchers` is currently a synchronous free function with no `State` parameters — adding `State<MetadataStore>` requires also adding it to the command signature and ensuring it is managed (it already is in `lib.rs:80`).

---

## Diagnostics APIs

### DiagnosticReport (struct, fields, serialization)

**File:** `src/crosshook-native/crates/crosshook-core/src/launch/diagnostics/models.rs`

```rust
pub struct DiagnosticReport {
    pub severity: ValidationSeverity,       // Fatal | Warning | Info
    pub summary: String,
    pub exit_info: ExitCodeInfo,
    pub pattern_matches: Vec<PatternMatch>,
    pub suggestions: Vec<ActionableSuggestion>,
    pub launch_method: String,
    pub log_tail_path: Option<String>,
    pub analyzed_at: String,               // RFC 3339 timestamp
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

Serde: All structs derive `Serialize, Deserialize`. `FailureMode` uses `#[serde(rename_all = "snake_case")]`.

### FailureMode Enum

14 variants (line 34-50): `CleanExit`, `NonZeroExit`, `Segfault`, `Abort`, `Kill`, `BusError`, `IllegalInstruction`, `FloatingPointException`, `BrokenPipe`, `Terminated`, `CommandNotFound`, `PermissionDenied`, `UnknownSignal`, `Indeterminate`, `Unknown`.

### Size Estimation

Constants (models.rs lines 5-7):

- `MAX_LOG_TAIL_BYTES = 2 * 1024 * 1024` (2MB) — the log tail passed to `analyze()`
- `MAX_DIAGNOSTIC_ENTRIES = 50` — max `pattern_matches` / `suggestions` entries
- `MAX_LINE_DISPLAY_CHARS = 500`

**Estimated serialized size of `DiagnosticReport`:**
A typical report has:

- `summary`: ~100 chars
- `exit_info`: ~200 chars (description, code, signal fields)
- `pattern_matches`: 0–3 common matches × ~300 chars each = ~900 chars
- `suggestions`: same as pattern_matches × ~300 chars = ~900 chars
- `analyzed_at`: ~30 chars
- Structural JSON overhead: ~300 chars

Total typical: **~2,500 bytes (well under 4KB)**. A worst-case maximum of 50 pattern_matches × 500 chars for `matched_line` + overhead approaches ~30KB. The feature spec's 4KB diagnostic_json cap therefore requires truncation or pre-serialization size checking before inserting. The sanitized report (with paths stripped) will be smaller.

---

## Current Metadata Module API

### MetadataStore Public Methods

**File:** `src/crosshook-native/crates/crosshook-core/src/metadata/mod.rs`

```rust
impl MetadataStore {
    pub fn try_new() -> Result<Self, String>
    pub fn with_path(path: &Path) -> Result<Self, MetadataStoreError>
    pub fn open_in_memory() -> Result<Self, MetadataStoreError>
    pub fn disabled() -> Self

    pub fn observe_profile_write(
        &self,
        name: &str,
        profile: &GameProfile,
        path: &Path,
        source: SyncSource,
        source_profile_id: Option<&str>,
    ) -> Result<(), MetadataStoreError>

    pub fn lookup_profile_id(
        &self,
        name: &str,
    ) -> Result<Option<String>, MetadataStoreError>

    pub fn observe_profile_rename(
        &self,
        old_name: &str,
        new_name: &str,
        old_path: &Path,
        new_path: &Path,
    ) -> Result<(), MetadataStoreError>

    pub fn observe_profile_delete(
        &self,
        name: &str,
    ) -> Result<(), MetadataStoreError>

    pub fn sync_profiles_from_store(
        &self,
        store: &ProfileStore,
    ) -> Result<SyncReport, MetadataStoreError>
}
```

Internal dispatch via `with_conn(&self, action: &'static str, f: F)` (lines 56-73):

- No-ops when `available = false` (disabled store), returning `T::default()`.
- Locks `Arc<Mutex<Connection>>` before executing.

### Profile ID Resolution Pattern

`lookup_profile_id` (mod.rs line 95) queries `profiles WHERE current_filename = ?1 AND deleted_at IS NULL`. Returns `Option<String>`.

In `profile_sync.rs` (line 72-86), the underlying SQL is:

```sql
SELECT profile_id FROM profiles WHERE current_filename = ?1 AND deleted_at IS NULL
```

For Phase 2: to insert a `launch_operations` row, the implementation must call `lookup_profile_id(profile_name)` to get the `profile_id`, then use that FK. If the profile is not yet in the metadata DB (possible on first launch before a profile_save), `lookup_profile_id` returns `None` and the operation should either be recorded with `profile_id = NULL` or skipped.

### SyncSource Variants

**File:** `src/crosshook-native/crates/crosshook-core/src/metadata/models.rs:70-79`

```rust
pub enum SyncSource {
    AppWrite,
    AppRename,
    AppDuplicate,
    AppDelete,
    FilesystemScan,
    Import,
    InitialCensus,
}
```

Phase 2 does not require new `SyncSource` variants — launcher and launch operation events are distinct tables, not profile sync events.

---

## LaunchRequest.profile_name Gap Analysis

### Current State

`LaunchRequest` is constructed entirely from profile data in the frontend (`ProfileEditor.tsx` / `LaunchPanel.tsx`) before being sent via IPC. The Tauri commands `launch_game` and `launch_trainer` receive the deserialized struct with no profile name context.

The test fixtures in `script_runner.rs` (lines 353-371) confirm the struct's fields — there is no `profile_name` anywhere.

### Required Changes

Two options:

**Option A — Add `profile_name: Option<String>` to `LaunchRequest`:**

- Add `#[serde(default)]` field to `LaunchRequest`.
- The frontend must populate it from the active profile name before calling `invoke("launch_game", ...)`.
- In `launch_game` / `launch_trainer`, read `request.profile_name.as_deref()` to call `record_launch_started`.
- Downstream: no existing callers break because `#[serde(default)]` means the field is optional in serialization.

**Option B — Pass profile_name as a separate IPC parameter:**

- Add `profile_name: Option<String>` as an extra argument to `launch_game` and `launch_trainer`.
- Tauri IPC supports multiple parameters: `invoke("launch_game", { request: {...}, profileName: "foo" })`.
- Cleaner separation: LaunchRequest stays a pure launch-config struct.

Option B is architecturally cleaner (avoids polluting a data transfer struct with identity concerns) but requires frontend changes to `invoke()` calls. Option A requires fewer changes and is fully backwards compatible.

### Downstream Effects of Adding the Field

If Option A (add to struct):

- `request.rs`: Add `pub profile_name: Option<String>` with `#[serde(default)]`.
- `LaunchRequest::Default` still works (field defaults to `None`).
- Test fixtures in `script_runner.rs` (struct-literal construction at lines 353, 406, 498, etc.) all need `profile_name: None` added — these are in test code only, not public API.
- No existing `script_runner.rs` command-building functions use `profile_name`, so no functional changes cascade.
- Frontend `useLaunchState.ts` / `LaunchPanel.tsx` must populate the field.

---

## Startup Integration

### Current Flow

**File:** `src/crosshook-native/src-tauri/src/startup.rs`

`run_metadata_reconciliation(metadata_store, profile_store)` (line 43-56):

- Calls `metadata_store.sync_profiles_from_store(profile_store)`.
- Logs `created` and `updated` counts.
- Returns `Result<(), StartupError>`.

Called from `lib.rs:53-57` inside the `.setup()` closure (synchronous, before the Tauri event loop).

`resolve_auto_load_profile_name(settings_store, profile_store)` (line 58-88):

- Reads settings, checks for a valid `last_used_profile`, verifies it exists in the profile list.
- Returns `Option<String>`.

`StartupError` (lines 7-41) wraps `MetadataStoreError`, `SettingsStoreError`, `ProfileStoreError`.

### Abandoned Operation Sweep Placement

The abandoned operation sweep (marking `launch_operations` rows with `outcome = 'abandoned'` where `ended_at IS NULL`) should run in the startup sequence **after** `run_metadata_reconciliation` and **before** the Tauri event loop starts accepting IPC calls.

Concretely, add a `sweep_abandoned_operations(metadata_store: &MetadataStore) -> Result<(), MetadataStoreError>` function to `startup.rs`. Call it from `lib.rs` in the `.setup()` closure, after line 57, following the pattern:

```rust
if let Err(error) = startup::sweep_abandoned_operations(&metadata_for_startup) {
    tracing::warn!(%error, "startup abandoned operation sweep failed");
}
```

Non-fatal: consistent with the existing pattern — metadata failures warn but do not abort startup.

The sweep SQL would be:

```sql
UPDATE launch_operations
SET outcome = 'abandoned', ended_at = ?1
WHERE ended_at IS NULL AND outcome IS NULL
```

This is safe to run before the event loop because no async launch tasks can be in-flight yet at that point in `setup()`.

---

## Key Findings Summary

1. **`LaunchRequest` has no `profile_name` field** — this is the primary structural gap. Adding `Option<String>` with `#[serde(default)]` is backwards compatible and requires minimal upstream changes.

2. **`record_launch_started` / `record_launch_finished` hook points** are in `src-tauri/src/commands/launch.rs`: started just before `command.spawn()` (around line 72), finished inside `stream_log_lines` after child exit (around line 203). `operation_id` must be threaded from the command into the background task.

3. **`observe_launcher_exported` hook point** is in `src-tauri/src/commands/export.rs` inside `export_launchers` (line 20), after a successful `export_launchers_core` call. The export command needs a `profile_name` parameter added (same gap as launch).

4. **`MetadataStore` is already managed Tauri state** (lib.rs:80) — can be accessed via `app.state::<MetadataStore>()` from `AppHandle` in both async launch commands without signature changes.

5. **`DiagnosticReport` typically serializes to ~2.5KB** — under the 4KB limit for typical runs; worst-case with 50 pattern_matches may exceed 4KB and needs truncation logic.

6. **`db::new_id()`** (db.rs:64) generates UUID v4 strings — Phase 2 `operation_id` should use the same helper for `launch_operations.id`.

7. **Startup sweep placement** is straightforward — add after `run_metadata_reconciliation` in the `.setup()` closure; non-fatal, consistent with existing error-handling pattern.
