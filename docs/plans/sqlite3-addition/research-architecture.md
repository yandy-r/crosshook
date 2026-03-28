# Architecture Research: SQLite Metadata Layer

## System Overview

CrossHook's codebase is a Tauri v2 application with a Rust workspace backend (`crosshook-core` library + `crosshook-cli` binary + `src-tauri` shell) and a React 18 TypeScript frontend. The new `MetadataStore` (SQLite) will live entirely inside `crosshook-core/src/metadata/` and be wired into the Tauri command layer at `src-tauri/src/commands/` — **never** inside the existing pure-TOML stores. The stores (`ProfileStore`, `SettingsStore`, `RecentFilesStore`, `CommunityTapStore`) are each initialized in `src-tauri/src/lib.rs::run()` with `try_new()` and passed to Tauri via `.manage()`.

## Relevant Files

### Rust Workspace

- `src/crosshook-native/crates/crosshook-core/src/lib.rs` — Module root; `pub mod` declarations for every domain (community, export, install, launch, logging, profile, settings, steam, update). New `pub mod metadata;` goes here.
- `src/crosshook-native/crates/crosshook-core/Cargo.toml` — Only current runtime deps: `chrono`, `directories`, `serde`, `serde_json`, `toml`, `tokio`, `tracing`, `tracing-subscriber`. Must add `rusqlite = { version = "0.39", features = ["bundled"] }` and `uuid = { version = "1", features = ["v4", "serde"] }`.
- `src/crosshook-native/src-tauri/src/lib.rs` — Canonical Tauri setup: initializes all four stores, calls `.manage()` for each, registers all Tauri commands. `MetadataStore` initialization and `.manage()` belong here, plus startup census in the `setup` closure.
- `src/crosshook-native/src-tauri/src/startup.rs` — Handles `resolve_auto_load_profile_name`; the startup reconciliation scan should be added here or as a parallel `startup::run_metadata_census()` function.
- `src/crosshook-native/src-tauri/src/paths.rs` — Script path resolution helpers; not touched by SQLite.
- `src/crosshook-native/src-tauri/src/commands/profile.rs` — All profile lifecycle Tauri commands; metadata sync hooks go here after `store.save()`, `store.rename()`, `store.delete()`, `store.duplicate()`, and `store.import_legacy()`.
- `src/crosshook-native/src-tauri/src/commands/launch.rs` — `launch_game` / `launch_trainer` async commands; Phase 2 hooks for `record_launch_started` / `record_launch_finished` go here, including `spawn_blocking` wrapping.
- `src/crosshook-native/src-tauri/src/commands/export.rs` — Launcher export/delete/rename commands; Phase 2 metadata sync for `observe_launcher_exported` goes here.
- `src/crosshook-native/src-tauri/src/commands/community.rs` — `community_sync` command; Phase 3 `sync_tap_index()` goes after `tap_store.sync_many()`.

### crosshook-core Domains

- `src/crosshook-native/crates/crosshook-core/src/profile/toml_store.rs` — `ProfileStore`: `try_new()`, `with_base_path()`, `load()`, `save()`, `list()`, `delete()`, `rename()`, `duplicate()`, `import_legacy()`, `save_launch_optimizations()`. **NOT modified** — stays pure TOML I/O.
- `src/crosshook-native/crates/crosshook-core/src/profile/models.rs` — `GameProfile` struct with `GameSection`, `TrainerSection`, `InjectionSection`, `SteamSection`, `RuntimeSection`, `LaunchSection`. `metadata::observe_profile_write` accepts `&GameProfile` to extract `game_name` and `launch_method` fields.
- `src/crosshook-native/crates/crosshook-core/src/profile/mod.rs` — Re-exports `validate_name` from `legacy`; the `validate_name()` rule also applies to SQLite identity rows.
- `src/crosshook-native/crates/crosshook-core/src/export/launcher_store.rs` — `LauncherInfo`, `LauncherDeleteResult`, `LauncherRenameResult` structs. Phase 2: after export/rename/delete these results inform `observe_launcher_exported`.
- `src/crosshook-native/crates/crosshook-core/src/launch/diagnostics/models.rs` — `DiagnosticReport`, `FailureMode`, `ExitCodeInfo`. Phase 2: `diagnostic_json` column in `launch_operations` stores a serialized (≤4 KB) `DiagnosticReport`.
- `src/crosshook-native/crates/crosshook-core/src/launch/diagnostics/mod.rs` — `analyze()` builds `DiagnosticReport` from exit status and log tail. `should_surface_report()` decides frontend emission. Both are called inside `stream_log_lines()` in `commands/launch.rs`.
- `src/crosshook-native/crates/crosshook-core/src/settings/mod.rs` — `SettingsStore`: `try_new()`, `with_base_path()`, `load()`, `save()`. `AppSettingsData` includes `community_taps: Vec<CommunityTapSubscription>`.
- `src/crosshook-native/crates/crosshook-core/src/community/taps.rs` — `CommunityTapStore`, `CommunityTapSubscription`, `CommunityTapSyncResult` (includes `head_commit: String`). Phase 3: `head_commit` is the value tracked in the `community_taps` SQLite table for idempotent re-indexing.

### New Files to Create

- `src/crosshook-native/crates/crosshook-core/src/metadata/mod.rs` — `MetadataStore` struct + public API
- `src/crosshook-native/crates/crosshook-core/src/metadata/db.rs` — Connection factory, PRAGMA setup, permission enforcement, symlink check
- `src/crosshook-native/crates/crosshook-core/src/metadata/migrations.rs` — Schema DDL, `user_version`-based migration runner
- `src/crosshook-native/crates/crosshook-core/src/metadata/models.rs` — `ProfileRow`, `SyncReport`, `SyncSource`, `LaunchOutcome`, `MetadataError`
- `src/crosshook-native/crates/crosshook-core/src/metadata/profile_sync.rs` — Profile lifecycle reconciliation (Phase 1)
- `src/crosshook-native/crates/crosshook-core/src/metadata/launcher_sync.rs` — Launcher mapping and drift (Phase 2)
- `src/crosshook-native/crates/crosshook-core/src/metadata/launch_history.rs` — Launch operation recording (Phase 2)
- `src/crosshook-native/crates/crosshook-core/src/metadata/community_index.rs` — Tap/catalog indexing (Phase 3)
- `src/crosshook-native/crates/crosshook-core/src/metadata/cache_store.rs` — External metadata cache (Phase 3)

## Data Flow

### Profile Lifecycle (Phase 1)

```
[React] invoke('profile_save', { name, data })
  -> commands/profile.rs::profile_save()
     -> store.save(name, &data)            // ProfileStore writes TOML
     -> metadata_store.observe_profile_write(name, &data, &path, SyncSource::AppWrite)  // NEW
        -> metadata/profile_sync.rs: UPSERT into `profiles` table; append to `profile_name_history`

[React] invoke('profile_rename', { old_name, new_name })
  -> commands/profile.rs::profile_rename()
     -> store.load(old_name)               // load BEFORE rename for launcher cleanup context
     -> store.rename(old_name, new_name)   // TOML file rename
     -> cleanup_launchers_for_profile_delete(old_name, &profile)  // best-effort launcher files
     -> store.load(new_name) + store.save(new_name, updated_display_name)  // display_name update
     -> settings_store.load() + settings_store.save(last_used_profile update)
     -> metadata_store.observe_profile_rename(old_name, new_name, ...)  // NEW — appends history

[React] invoke('profile_delete', { name })
  -> commands/profile.rs::profile_delete()
     -> store.load(name) + cleanup_launchers_for_profile_delete(...)  // best-effort
     -> store.delete(name)
     -> metadata_store.observe_profile_delete(name)  // NEW — soft-delete tombstone

[React] invoke('profile_duplicate', { name })
  -> commands/profile.rs::profile_duplicate()
     -> store.duplicate(name) -> DuplicateProfileResult { name: copy_name, profile }
     -> metadata_store.observe_profile_write(copy_name, &profile, ..., SyncSource::AppDuplicate)  // NEW
```

### Launch Lifecycle (Phase 2)

```
[React] invoke('launch_game', { request })
  -> commands/launch.rs::launch_game()         // async Tauri command
     -> validate(&request)
     -> create_log_path(...)
     -> command.spawn() -> child
     -> spawn_log_stream(app, log_path, child, method)
        -> stream_log_lines(app, ...)          // tokio task; polls child + emits events
           -> analyze(exit_status, &log_tail, method)  -> DiagnosticReport
           -> app.emit("launch-diagnostic", &report)
           -> app.emit("launch-complete", { code, signal })
     // Phase 2: record_launch_started BEFORE spawn; record_launch_finished inside stream_log_lines
     // needs spawn_blocking because rusqlite Connection is !Send
```

### Community Tap Lifecycle (Phase 3)

```
[React] invoke('community_sync')
  -> commands/community.rs::community_sync()
     -> tap_store.sync_many(&taps)  -> Vec<CommunityTapSyncResult> (each has head_commit)
     // Phase 3: sync_tap_index(metadata_store, &sync_results)
     //   -> for each result: if head_commit unchanged, skip; else index profiles
```

### Launcher Export (Phase 2)

```
[React] invoke('export_launchers', { request })
  -> commands/export.rs::export_launchers()
     -> export_launchers_core(&request)  -> SteamExternalLauncherExportResult
     // Phase 2: observe_launcher_exported(profile_name, slug, script_path, desktop_path)
```

### Startup Reconciliation (Phase 1)

```
src-tauri/src/lib.rs::run()
  -> setup closure:
     -> logging::init_logging(false)
     -> paths::ensure_development_scripts_executable()
     -> startup::resolve_auto_load_profile_name(...)   // existing
     // NEW: metadata_store.sync_profiles_from_store(&profile_store)  // background census
     //      runs in tauri::async_runtime::spawn + spawn_blocking
```

## Integration Points

### Store Initialization Pattern (`src-tauri/src/lib.rs`)

Every store follows the same pattern: `try_new()` returns `Result<Self, String>`; on error, print to stderr and `process::exit(1)`. `MetadataStore` deviates for fail-soft: `try_new()` returns `Result<Self, String>` but the store carries an internal `available: bool` flag. A failure sets `available = false` and all methods no-op internally.

```rust
// Existing pattern (hard-fail):
let profile_store = ProfileStore::try_new().unwrap_or_else(|error| {
    eprintln!("CrossHook: failed to initialize profile store: {error}");
    std::process::exit(1);
});

// New MetadataStore (soft-fail):
let metadata_store = MetadataStore::try_new().unwrap_or_else(|error| {
    tracing::warn!(%error, "metadata store unavailable; SQLite features disabled");
    MetadataStore::unavailable()
});
// ...
.manage(metadata_store)
```

### Tauri Command State Injection

All stores are `Clone` (they contain only `PathBuf` for TOML stores; `MetadataStore` must be `Clone` via `Arc<Mutex<Connection>>`). State is accessed via `State<'_, StoreType>` in command function signatures.

### Fail-Soft Pattern in Commands

```rust
// After every canonical write in commands/profile.rs:
store.save(name, &data).map_err(map_error)?;
if let Err(error) = state.metadata_store.observe_profile_write(name, &data, &path, SyncSource::AppWrite) {
    tracing::warn!(%error, profile_name = name, "metadata sync failed after profile save");
}
// Metadata failure NEVER returns Err to the frontend
```

### `spawn_blocking` Requirement for Async Commands

`launch_game` and `launch_trainer` are `async` Tauri commands. `rusqlite::Connection` is `!Send`, so SQLite operations inside async tasks must use `tokio::task::spawn_blocking`. The pattern:

```rust
let metadata = metadata_store.clone();
tokio::task::spawn_blocking(move || {
    metadata.record_launch_started(profile_name, method)
}).await.map_err(|e| e.to_string())??
```

### `validate_name()` Constraint

`profile/legacy.rs::validate_name()` enforces profile name rules. SQLite identity rows must accept only names that pass this function. The metadata layer calls `validate_name()` on the `profile_name` argument before inserting rows.

### Database Path

- Uses `directories::BaseDirs::data_local_dir()` → `~/.local/share/crosshook/metadata.db`
- Parent directory at `~/.local/share/crosshook/` created with `0o700`
- DB file `chmod 0600` immediately after creation
- Symlink check via `symlink_metadata()` before `Connection::open()`

## Key Dependencies

| Dependency             | Current Version  | Role                                                               |
| ---------------------- | ---------------- | ------------------------------------------------------------------ |
| `directories`          | `5`              | `BaseDirs::data_local_dir()` for DB path resolution                |
| `serde` / `serde_json` | `1`              | `DiagnosticReport` JSON serialization for `diagnostic_json` column |
| `chrono`               | `0.4`            | `Utc::now().to_rfc3339()` for timestamps in history tables         |
| `tracing`              | `0.1`            | `tracing::warn!` for fail-soft metadata error logging              |
| `tokio`                | `1`              | `spawn_blocking` for SQLite in async commands                      |
| `rusqlite`             | **to add: 0.39** | Bundled SQLite 3.51.3; `Arc<Mutex<Connection>>` for Clone          |
| `uuid`                 | **to add: 1.x**  | UUID v4 for stable profile identity                                |

## Architectural Patterns

- **Try-new constructor**: every store exposes `try_new() -> Result<Self, String>` and `with_base_path(PathBuf) -> Self` for test injection. `MetadataStore` adds `open_in_memory() -> Result<Self, MetadataError>` for unit tests.
- **Best-effort cascade**: profile rename/delete in `commands/profile.rs` already uses best-effort steps (launcher cleanup → display_name update → settings update) with `tracing::warn!` on each failure. Metadata sync fits this exact pattern as the final step.
- **Single-method-per-command hooks**: each Tauri command gets exactly one or two `observe_*` / `record_*` calls at the end; no cross-command coordination required for Phase 1.
- **Strict TOML authority**: `ProfileStore` is never modified. Metadata reads profile content fields from `&GameProfile` passed into the Tauri command, not from a second TOML read.
- **IPC boundary types**: all types crossing the Tauri IPC boundary derive `Serialize + Deserialize`. `MetadataError` is mapped to `String` at the command layer (same `map_error` pattern); raw SQL errors never reach the frontend.
- **Async event emission**: `launch-log`, `launch-diagnostic`, `launch-complete` events emitted from a detached `tokio::spawn` task via `app.emit()`. Phase 2 launch history recording must fit inside `stream_log_lines()` after `analyze()` completes.

## Edge Cases

- `profile_rename` loads the old profile **before** calling `store.rename()` — metadata sync must receive the old path/name from this pre-rename load, not from a post-rename read.
- `profile_duplicate` returns a `DuplicateProfileResult { name, profile }` — metadata must record both the source UUID (`source_profile_id`) and the new UUID for lineage tracking.
- Community tap `head_commit` is available only on `CommunityTapSyncResult`, not on the `CommunityTapSubscription` stored in settings. Phase 3 indexing must consume the full sync result, not re-read settings.
- `launch_game` and `launch_trainer` are decoupled async commands — each spawns its own log stream task. Phase 2 must handle partial records if CrossHook is killed before `stream_log_lines` completes (startup sweep marks stale rows as `abandoned`).
- The `check_launcher_for_profile` command passes empty strings for `target_home_path` and `steam_client_install_path`, relying on fallback logic in `check_launcher_for_profile_core`. Phase 2 metadata sync should only record launcher paths when they are non-empty.
- `export.rs::delete_launcher_by_slug` does not read a profile — it has no `profile_name` context. Phase 2 metadata hooks for this path need the profile name threaded through or a reverse-lookup from slug.
