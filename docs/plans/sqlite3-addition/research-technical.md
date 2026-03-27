# SQLite3 Addition - Technical Analysis

## Executive Summary

The best technical fit is a hybrid model: keep `GameProfile` TOML files canonical for editable profile content and runtime artifacts, but add a SQLite metadata store inside `crosshook-core` for stable IDs, relationships, history, derived projections, and caches. This matches current code structure because `ProfileStore`, `LauncherStore` functions, community indexing, recent files, and launch diagnostics are all local and file-centric already, but today they have no durable relational join layer. The main technical challenge is not SQLite itself; it is defining clean sync boundaries so CrossHook can reconcile profile files, launcher files, tap workspaces, and launch events into SQLite without ever making the database silently more authoritative than the filesystem.

### Architecture Approach

- Add a new `metadata` module in `crosshook-core` (peer to existing `community`, `export`, `install`, `launch`, `logging`, `profile`, `settings`, `steam`, `update` modules) that owns SQLite connection setup, migrations, and reconciliation APIs.
- Store the database under the CrossHook app data directory (e.g. `~/.local/share/crosshook/metadata.db`), separate from TOML config at `~/.config/crosshook/profiles/`, paralleling how `CommunityTapStore` uses `BaseDirs::data_local_dir()` for tap workspaces.
- Keep `ProfileStore` as the only writer for canonical profile content, then emit metadata sync actions after create/save/rename/delete/import/duplicate operations.
- Let Tauri commands and CLI flows record launch operations and launcher sync events through shared core APIs so desktop and CLI behavior do not diverge.
- Use scans plus events together:
  - events for immediate writes after known operations
  - scans for recovery when users modify TOML or launcher files outside CrossHook

### Codebase Module Inventory (Verified)

The current `crosshook-core/src/lib.rs` exports these modules:

```rust
pub mod community;   // taps.rs, index.rs — git-based community profile sharing
pub mod export;      // launcher.rs, launcher_store.rs — shell script + .desktop export
pub mod install;     // discovery.rs, models.rs, service.rs — game installation
pub mod launch;      // request.rs, script_runner.rs, diagnostics/, optimizations.rs, preview.rs, env.rs, runtime_helpers.rs
pub mod logging;     // structured logging via tracing
pub mod profile;     // models.rs, toml_store.rs, community_schema.rs, exchange.rs, legacy.rs
pub mod settings;    // mod.rs (SettingsStore), recent.rs (RecentFilesStore)
pub mod steam;       // discovery.rs, libraries.rs, manifest.rs, proton.rs, vdf.rs, auto_populate.rs, diagnostics.rs, models.rs
pub mod update;      // models.rs, service.rs — game update management
```

The new `metadata` module will be added as a peer at the same level.

### Data Model Implications

#### Core Tables

| Table | Purpose | Columns (Key) | Notes |
| --- | --- | --- | --- |
| `profiles` | stable local profile identity | `profile_id` TEXT PK (ULID), `current_filename` TEXT UNIQUE, `current_path` TEXT, `name_hash` TEXT, `game_name` TEXT, `launch_method` TEXT, `created_at` TEXT, `updated_at` TEXT | filename maps to `ProfileStore::profile_path()` stem; `game_name` from `GameSection.name`; `launch_method` from `LaunchSection.method` |
| `profile_file_snapshots` | observed file-state projection | `profile_id` TEXT FK, `mtime` TEXT, `size` INTEGER, `content_hash` TEXT, `parse_ok` BOOLEAN, `observed_at` TEXT | one latest row per profile; enables drift detection |
| `profile_name_history` | append-only rename history | `id` INTEGER PK, `profile_id` TEXT FK, `old_name` TEXT, `new_name` TEXT, `old_path` TEXT, `new_path` TEXT, `source` TEXT, `created_at` TEXT | `source` enum: `app_rename`, `app_duplicate`, `filesystem_scan`, `import` |
| `launchers` | logical launcher identity | `launcher_id` TEXT PK (ULID), `profile_id` TEXT FK, `launcher_slug` TEXT, `display_name` TEXT, `script_path` TEXT, `desktop_entry_path` TEXT, `created_at` TEXT, `updated_at` TEXT | maps to `LauncherInfo` fields from `launcher_store.rs` |
| `launcher_observations` | filesystem observations | `id` INTEGER PK, `launcher_id` TEXT FK, `script_exists` BOOLEAN, `desktop_entry_exists` BOOLEAN, `is_stale` BOOLEAN, `observed_at` TEXT | mirrors `LauncherInfo.script_exists`, `.desktop_entry_exists`, `.is_stale` |
| `launch_operations` | append-only launch attempts | `id` INTEGER PK, `profile_id` TEXT FK, `method` TEXT, `target` TEXT, `game_path` TEXT, `trainer_path` TEXT, `started_at` TEXT, `ended_at` TEXT, `exit_code` INTEGER, `signal` INTEGER, `log_path` TEXT | captures data from `LaunchRequest` + `LaunchResult`; `method` is one of `steam_applaunch`, `proton_run`, `native` |
| `launch_diagnostics` | structured diagnostic payloads | `id` INTEGER PK, `operation_id` INTEGER FK, `severity` TEXT, `failure_mode` TEXT, `summary` TEXT, `pattern_matches_json` TEXT, `suggestions_json` TEXT, `analyzed_at` TEXT | maps `DiagnosticReport`, `ExitCodeInfo`, `PatternMatch`, `ActionableSuggestion` from `launch/diagnostics/models.rs` |
| `collections` | user curation groups | `collection_id` TEXT PK (ULID), `name` TEXT UNIQUE, `created_at` TEXT | independent from filenames |
| `collection_profiles` | M:N join | `collection_id` TEXT FK, `profile_id` TEXT FK, `added_at` TEXT | composite PK |
| `profile_preferences` | favorite/pinned/local flags | `profile_id` TEXT PK FK, `is_favorite` BOOLEAN DEFAULT 0, `is_pinned` BOOLEAN DEFAULT 0, `usage_count` INTEGER DEFAULT 0, `last_launched_at` TEXT | one-to-one with profile identity |
| `community_taps` | subscribed taps | `tap_id` TEXT PK (ULID), `url` TEXT, `branch` TEXT, `local_path` TEXT, `last_synced_commit` TEXT, `last_synced_at` TEXT | maps `CommunityTapSubscription` + `CommunityTapSyncResult.head_commit` |
| `community_profiles` | indexed manifest rows | `id` INTEGER PK, `tap_id` TEXT FK, `manifest_path` TEXT, `relative_path` TEXT, `game_name` TEXT, `trainer_name` TEXT, `compatibility_rating` TEXT, `author` TEXT, `platform_tags_json` TEXT | maps `CommunityProfileIndexEntry` + `CommunityProfileMetadata` fields |
| `external_cache_entries` | cached ProtonDB/art/other | `id` INTEGER PK, `cache_bucket` TEXT, `cache_key` TEXT, `payload_json` TEXT, `fetched_at` TEXT, `expires_at` TEXT | typed cache with freshness policy |
| `sync_runs` | reconciliation audit trail | `id` INTEGER PK, `sync_type` TEXT, `started_at` TEXT, `ended_at` TEXT, `profiles_seen` INTEGER, `profiles_added` INTEGER, `profiles_updated` INTEGER, `issues_count` INTEGER | |
| `sync_issues` | per-item sync problems | `id` INTEGER PK, `sync_run_id` INTEGER FK, `item_type` TEXT, `item_path` TEXT, `issue` TEXT, `created_at` TEXT | |

#### Type Mapping from Codebase to Tables

| Rust Type | Source File | SQLite Table(s) |
| --- | --- | --- |
| `GameProfile` | `profile/models.rs` | `profiles` (identity only; content stays in TOML) |
| `GameSection` | `profile/models.rs` | `profiles.game_name` extracted |
| `LaunchSection` | `profile/models.rs` | `profiles.launch_method` extracted |
| `DuplicateProfileResult` | `profile/toml_store.rs` | triggers `profile_name_history` insert |
| `ProfileStore` | `profile/toml_store.rs` | sync source for `profiles` table |
| `LauncherInfo` | `export/launcher_store.rs` | `launchers` + `launcher_observations` |
| `LauncherDeleteResult` | `export/launcher_store.rs` | triggers `launcher_observations` update |
| `LauncherRenameResult` | `export/launcher_store.rs` | triggers `launchers` slug/path update |
| `LauncherStoreError` | `export/launcher_store.rs` | recorded in `sync_issues` on failure |
| `LaunchRequest` | `launch/request.rs` | `launch_operations` (method, game_path, trainer_path, steam config) |
| `LaunchResult` | `src-tauri/src/commands/launch.rs` | `launch_operations` (log_path, succeeded) |
| `DiagnosticReport` | `launch/diagnostics/models.rs` | `launch_diagnostics` |
| `ExitCodeInfo` | `launch/diagnostics/models.rs` | `launch_diagnostics.failure_mode`, exit/signal |
| `PatternMatch` | `launch/diagnostics/models.rs` | `launch_diagnostics.pattern_matches_json` |
| `ActionableSuggestion` | `launch/diagnostics/models.rs` | `launch_diagnostics.suggestions_json` |
| `FailureMode` | `launch/diagnostics/models.rs` | `launch_diagnostics.failure_mode` |
| `CommunityTapSubscription` | `community/taps.rs` | `community_taps` |
| `CommunityTapSyncResult` | `community/taps.rs` | `community_taps.last_synced_commit` |
| `CommunityTapWorkspace` | `community/taps.rs` | `community_taps.local_path` |
| `CommunityProfileIndexEntry` | `community/index.rs` | `community_profiles` |
| `CommunityProfileMetadata` | `profile/community_schema.rs` | `community_profiles` fields |
| `CompatibilityRating` | `profile/community_schema.rs` | `community_profiles.compatibility_rating` |
| `AppSettingsData` | `settings/mod.rs` | `community_taps` (taps list extracted) |
| `RecentFilesData` | `settings/recent.rs` | could optionally index, but low-priority |

#### Recommended Stable ID Strategy

- Generate a local `profile_id` (ULID) on first observation and keep it forever.
- Primary match order during sync:
  1. explicit embedded ID in TOML if CrossHook chooses to add one later
  2. existing exact current filename match against `profiles.current_filename`
  3. high-confidence rename match using recent `profile_name_history` + file content hash from `profile_file_snapshots`
  4. otherwise create a new profile identity
- Do not key durable relationships on filename, `game.name`, launcher display name, or slug.
- Use a separate stable `launcher_id`; launcher slug is only the current export projection, derived from `sanitize_launcher_slug()` in `export/launcher.rs`.

#### Authority Boundaries

- **Filesystem/TOML authoritative**:
  - `GameProfile` content (all sections: `game`, `trainer`, `injection`, `steam`, `runtime`, `launch`)
  - exported launcher script and `.desktop` artifact contents (watermark-verified in `launcher_store.rs`)
  - tap git workspaces and `community-profile.json` manifest files
  - logs, prefixes, runtime helper outputs
  - `settings.toml` and `recent.toml` config files
- **SQLite authoritative**:
  - local stable IDs (`profile_id`, `launcher_id`, `tap_id`, `collection_id`)
  - favorites, collections, usage counters, pinned state
  - relationship history (rename history, launcher-to-profile mapping)
  - launch event history and structured diagnostics index
  - cache freshness and normalized metadata joins
  - derived search indexes and projections
  - sync audit trail

### API Design Considerations

#### Core Reconciliation Surfaces

```rust
// Primary sync entry points in metadata module
pub fn sync_profiles_from_store(db: &Connection, store: &ProfileStore) -> Result<SyncReport, MetadataError>
pub fn sync_launcher_observations(db: &Connection, launchers: &[LauncherInfo]) -> Result<SyncReport, MetadataError>
pub fn record_launch_started(db: &Connection, profile_id: &str, request: &LaunchRequest) -> Result<i64, MetadataError>
pub fn record_launch_finished(db: &Connection, operation_id: i64, exit_code: Option<i32>, signal: Option<i32>, report: &DiagnosticReport) -> Result<(), MetadataError>
pub fn sync_tap_index(db: &Connection, results: &[CommunityTapSyncResult]) -> Result<SyncReport, MetadataError>
pub fn get_profile_catalog(db: &Connection, profile_id: &str) -> Result<ProfileCatalogEntry, MetadataError>
```

#### Integration Points (Verified Against Actual Code)

**ProfileStore operations** (all in `profile/toml_store.rs`):
- `save()` → upsert profile identity, update snapshot
- `delete()` → soft-mark profile deleted, preserve history
- `rename()` → append to `profile_name_history`, update `current_filename`/`current_path`
- `duplicate()` → create new profile identity, link rename history to source
- `import_legacy()` → create new profile identity from legacy import
- `list()` → used by sync to discover all profile filenames

**Launcher operations** (all in `export/launcher_store.rs`):
- `check_launcher_exists()` / `check_launcher_for_profile()` → upsert launcher, update observations
- `delete_launcher_files()` / `delete_launcher_for_profile()` / `delete_launcher_by_slug()` → mark launcher deleted
- `rename_launcher_files()` → update launcher slug/paths
- `export_launchers()` (in `export/launcher.rs`) → create/upsert launcher identity
- `list_launchers()` → bulk observation sync
- `find_orphaned_launchers()` → mark orphaned launchers in observations

**Launch command integration** (in `src-tauri/src/commands/launch.rs`):
- `launch_game()` / `launch_trainer()` → `record_launch_started()` with `LaunchRequest` data
- `stream_log_lines()` completion → `record_launch_finished()` with exit code, signal, and `DiagnosticReport`
- Note: launch commands use Tokio async (`async fn`), but the metadata write can be a blocking `rusqlite` call wrapped in `spawn_blocking` or called at the sync boundary

**Community sync** (in `src-tauri/src/commands/community.rs` → `crosshook-core/community/taps.rs`):
- `community_sync()` → `sync_tap_index()` after `CommunityTapStore::sync_many()` returns `Vec<CommunityTapSyncResult>`
- `community_add_tap()` → insert tap subscription

**Profile lifecycle in Tauri** (in `src-tauri/src/commands/profile.rs`):
- `profile_delete()` calls `cleanup_launchers_for_profile_delete()` then `store.delete()` — both should trigger metadata updates
- `profile_rename()` loads profile, renames via store, cleans up old launchers, updates `display_name` and `settings.last_used_profile` — all should flow to metadata

#### Sync Semantics

- All sync entry points should be idempotent and transaction-backed using `rusqlite::Transaction`.
- Reconciliation should use explicit source tags: `app_write`, `app_rename`, `app_duplicate`, `app_delete`, `filesystem_scan`, `tap_sync`, `launch_runtime`, `cache_refresh`, `import`.
- Append-only event tables (`profile_name_history`, `launch_operations`, `launch_diagnostics`, `sync_runs`, `sync_issues`) feed mutable projection tables (`profiles.current_*`, `launchers.current_*`, `profile_preferences.usage_count`, `profile_preferences.last_launched_at`) for fast UI reads.

### Cross-Team Refinements

The following corrections and enhancements were integrated from teammate research (security, business, practices, API):

#### Security Requirements (from security-researcher)

1. **DB file permissions**: SQLite creates files with umask-filtered permissions (typically `0644` world-readable). `metadata.db`, WAL, and SHM files must be created/enforced at `0600` via `set_permissions()` immediately after `Connection::open()`. Parent directory should be `0700`.
2. **Single connection factory**: All connection opens must go through a centralized `open_connection(db_path)` function that unconditionally applies required PRAGMAs. No raw `Connection::open()` calls elsewhere.
3. **Symlink attack prevention**: Before opening the database, verify the path (if it exists) is a regular file (not a symlink) using `symlink_metadata()`.
4. **Payload size limits**: `external_cache_entries.payload_json` ≤ 512 KB. `launch_diagnostics` summary ≤ 4 KB. Enforce before write.
5. **Path sanitization**: New SQLite-backed Tauri commands must sanitize all stored paths before crossing the IPC boundary. Recommend a `DisplayPath` newtype with sanitization in its `Serialize` impl, or application at the Tauri command layer.
6. **Error opacity**: Define `MetadataError` enum with opaque user-facing variants. Log full `rusqlite::Error` via `tracing::error!` but never propagate raw error strings to the frontend.
7. **Re-validate paths from SQLite before filesystem use**: When a stored path string (e.g. `current_toml_path` from `profiles`, `expected_script_path` from `launchers`) is used in a filesystem operation (file open, delete, rename), re-apply `validate_name()` or path-safety checks. Do not assume stored values are safe by virtue of having been stored — the database could be corrupted or tampered with.
8. **`execute_batch()` only for hard-coded strings**: `execute_batch()` does not support parameterized values. The connection bootstrap PRAGMA strings must be string literals in source code. For any PRAGMA needing a runtime value (like `user_version`), use `conn.pragma_update()` instead.
9. **Frontend XSS prevention for community/metadata fields**: React WebView rendering of community tap manifest fields (`game_name`, `trainer_name`, `author`, `description`) and TOML-derived filenames in rename disambiguation prompts must use standard JSX interpolation (`{value}`) — never `dangerouslySetInnerHTML`. Targeted audit requirement for `CommunityBrowser` and rename UI components.
10. **No raw CLI arguments in `launch_operations`**: `launch_operations` and `launch_diagnostics` rows must never store raw CLI argument lists. Proton/Steam launch arguments may contain tokens or path fragments with credentials that would persist in history rows even after profile deletion. Store only structured, pre-parsed fields (`method`, `game_path`, `trainer_path`, `exit_code`, `signal`, `failure_mode`) — never the full command line or environment variables.

#### Connection Setup (from api-researcher)

Every connection must run these PRAGMAs (verified for rusqlite 0.39.0 / SQLite 3.51.3):

```sql
PRAGMA journal_mode=WAL;      -- persistent; truly needed on first open
PRAGMA foreign_keys=ON;        -- must set per connection; no-op inside transaction
PRAGMA synchronous=NORMAL;     -- safe+fast with WAL
PRAGMA busy_timeout=5000;      -- ms; avoids immediate SQLITE_BUSY errors
PRAGMA secure_delete=ON;       -- zero-fill deleted data (security)
```

After executing, re-read each PRAGMA to verify — silent failures are real.

- Write transactions: always use `TransactionBehavior::Immediate` (`BEGIN IMMEDIATE`). Deferred transactions that upgrade to write can return `SQLITE_BUSY`.
- Set `PRAGMA application_id` to a CrossHook-specific 32-bit magic number on DB creation so `file(1)` can identify the format.
- Use `rusqlite_migration` 2.5.0 for schema migrations (uses `PRAGMA user_version` internally — do not write `user_version` elsewhere).

#### Business Rule Corrections (from business-analyzer)

1. **Launcher PK**: `(profile_id, launcher_slug)` composite key, not a separate ULID. Slug change on rename = old row tombstoned + new row on re-export. No attempt to keep identity across a slug change.
2. **Duplicate lineage**: `profiles` table needs a `source_profile_id` nullable FK to track duplication lineage.
3. **Community tap PK**: `(tap_url, tap_branch)` matching `CommunityTapSubscription`. SQLite never adds or removes subscriptions — only mirrors sync state and catalog cache for subscriptions that already exist in TOML.
4. **Launch operation lifecycle**: Row created at launch start with `status = 'incomplete'`, updated on terminal event from both `launch-complete` (exit code + signal) and `launch-diagnostic` (DiagnosticReport).
5. **RecentFilesStore migration**: Current TOML-based store (`settings/recent.rs`) with 3 lists (game_paths, trainer_paths, dll_paths, max 10 each) can be replaced by a `recent_file_entry` table with timestamps in a later phase.

#### Phase 1 Simplification (from practices-researcher)

The full 14-table schema is the complete vision. Phase 1 needs only:
- `profiles` table (with inline `content_hash` column instead of separate `profile_file_snapshots` table)
- `profile_name_history` table
- 2-3 preference columns on `profiles` (`is_favorite`, `is_pinned`, `usage_count`)
- `launchers` table

Cut from v1:
- `sync_runs` / `sync_issues` audit tables → use `tracing::warn!` instead
- `external_cache_entries` → Phase 3 only, no current UI feature drives it
- Derived projection tables → compute on read via simple SQL aggregates at v1 scale
- `profile_file_snapshots` separate table → inline `content_hash` on `profiles` suffices

### System Constraints

#### Sync vs Async Architecture

- `crosshook-core` business logic is primarily **synchronous**: `ProfileStore`, `LauncherStore` functions, `CommunityTapStore`, `SettingsStore`, `RecentFilesStore` all use `std::fs` blocking I/O.
- **Exception**: `crosshook-core` already depends on `tokio` (Cargo.toml: `tokio = { version = "1", features = ["fs", "process", "rt", "sync"] }`), but this is for the Tauri async runtime in launch commands (`launch_game`/`launch_trainer` are `async fn`).
- `rusqlite` is the correct choice because it matches the synchronous core pattern. The existing stores (`ProfileStore`, `SettingsStore`, `CommunityTapStore`) are all `Clone + Send + Sync` structs with `PathBuf` state — the metadata store should use `Arc<Mutex<Connection>>` internally for thread-safe access while remaining `Clone` for Tauri `.manage()`.
- For Tauri async command handlers that need metadata writes, use `tokio::task::spawn_blocking` to call synchronous `rusqlite` operations, consistent with how the existing codebase already bridges sync core logic into async Tauri commands.

#### AppImage Packaging

- The `rusqlite` crate with `bundled` feature compiles SQLite from source, avoiding host SQLite version discrepancies. This is critical for AppImage distribution where the host environment varies.
- `Cargo.toml` dependency should be: `rusqlite = { version = "0.32", features = ["bundled"] }` (or latest stable).
- JSON1 and FTS5 extensions can be enabled via additional features (`bundled-full` or selectively `bundled` + `trace` + `json`) but should be optional for v1.

#### Path Handling

- Profile paths use `BaseDirs::config_dir().join("crosshook/profiles")` → `~/.config/crosshook/profiles/` (see `ProfileStore::try_new()`).
- Settings use `BaseDirs::config_dir().join("crosshook")` → `~/.config/crosshook/` (see `SettingsStore::try_new()`).
- Community taps use `BaseDirs::data_local_dir().join("crosshook/community/taps")` → `~/.local/share/crosshook/community/taps/` (see `CommunityTapStore::try_new()`).
- Launcher scripts export to `~/.local/share/crosshook/launchers/` and desktop entries to `~/.local/share/applications/` (see `derive_launcher_paths()` in `launcher_store.rs`).
- **Database location should be**: `BaseDirs::data_local_dir().join("crosshook/metadata.db")` → `~/.local/share/crosshook/metadata.db`, co-located with taps and launcher scripts, separate from config.
- Path sanitization for UI display uses `sanitize_display_path()` in `src-tauri/src/commands/launch.rs` (replaces `$HOME` with `~`); the same pattern should apply to any paths stored in SQLite that are displayed to users.

#### Fail-Soft Behavior

- Schema should tolerate partial failure: if SQLite is unavailable, launching and TOML editing should still work, with metadata features degraded.
- The `MetadataStore` should expose a `try_new()` pattern matching existing stores (`ProfileStore::try_new()`, `SettingsStore::try_new()`, etc.) but with a `Result` that the Tauri setup can handle gracefully (log warning, set `Option<MetadataStore>` managed state).
- Each Tauri command that writes to metadata should check `Option<MetadataStore>` and skip gracefully if unavailable, logging a warning.

#### Tauri State Management

The current Tauri setup (in `src-tauri/src/lib.rs`) manages state via:
```rust
.manage(profile_store)         // ProfileStore
.manage(settings_store)        // SettingsStore
.manage(recent_files_store)    // RecentFilesStore
.manage(community_tap_store)   // CommunityTapStore
.manage(commands::update::UpdateProcessState::new())
```

The metadata store should be added as:
```rust
.manage(metadata_store)        // Option<MetadataStore> or MetadataStore with internal error state
```

This follows the established pattern where each store is initialized in `run()` and passed to `.manage()`.

### File-Level Impact Preview

#### Files to Create

| File | Purpose |
| --- | --- |
| `src/crosshook-native/crates/crosshook-core/src/metadata/mod.rs` | Module root: exports, `MetadataStore` struct definition |
| `src/crosshook-native/crates/crosshook-core/src/metadata/db.rs` | Connection management, PRAGMA setup, open/close helpers |
| `src/crosshook-native/crates/crosshook-core/src/metadata/migrations.rs` | Schema creation, version-based migration runner |
| `src/crosshook-native/crates/crosshook-core/src/metadata/models.rs` | Rust types for metadata rows (`ProfileIdentity`, `LauncherIdentity`, `LaunchOperation`, `SyncReport`, etc.) |
| `src/crosshook-native/crates/crosshook-core/src/metadata/profile_sync.rs` | `sync_profiles_from_store()`, profile identity resolution, rename history |
| `src/crosshook-native/crates/crosshook-core/src/metadata/launcher_sync.rs` | `sync_launcher_observations()`, launcher identity management |
| `src/crosshook-native/crates/crosshook-core/src/metadata/launch_history.rs` | `record_launch_started()`, `record_launch_finished()`, query APIs |
| `src/crosshook-native/crates/crosshook-core/src/metadata/community_index.rs` | `sync_tap_index()`, community profile catalog |
| `src/crosshook-native/crates/crosshook-core/src/metadata/collections.rs` | User collections CRUD, favorites, profile preferences |
| `src/crosshook-native/crates/crosshook-core/src/metadata/cache.rs` | External cache entry management with TTL |

#### Files to Modify

| File | Change | Verified |
| --- | --- | --- |
| `src/crosshook-native/crates/crosshook-core/Cargo.toml` | Add `rusqlite = { version = "0.39", features = ["bundled"] }` + `uuid = { version = "1", features = ["v4", "serde"] }` + `rusqlite_migration = "2.5"` | Current deps: chrono, directories, serde, serde_json, toml, tokio, tracing, tracing-subscriber |
| `src/crosshook-native/crates/crosshook-core/src/lib.rs` | Add `pub mod metadata;` to module exports | Currently exports: community, export, install, launch, logging, profile, settings, steam, update |
| `src/crosshook-native/crates/crosshook-core/src/profile/toml_store.rs` | No direct modification needed for v1 — sync is triggered from Tauri commands, not from `ProfileStore` internals | `ProfileStore` methods: `save()`, `load()`, `list()`, `delete()`, `rename()`, `duplicate()`, `import_legacy()`, `save_launch_optimizations()` |
| `src/crosshook-native/src-tauri/src/lib.rs` | Initialize `MetadataStore`, add to `.manage()` state, pass to `setup()` for startup sync | Current state: `ProfileStore`, `SettingsStore`, `RecentFilesStore`, `CommunityTapStore`, `UpdateProcessState` |
| `src/crosshook-native/src-tauri/src/commands/profile.rs` | Add metadata sync calls after `profile_save`, `profile_delete`, `profile_rename`, `profile_duplicate`, `profile_import_legacy` | Current: `profile_delete` calls `cleanup_launchers_for_profile_delete` then `store.delete()`; `profile_rename` does launcher cleanup, display_name update, settings update |
| `src/crosshook-native/src-tauri/src/commands/launch.rs` | Add `record_launch_started()` in `launch_game()`/`launch_trainer()` and `record_launch_finished()` in `stream_log_lines()` completion | Current: emits `launch-complete` and `launch-diagnostic` events but does not persist |
| `src/crosshook-native/src-tauri/src/commands/export.rs` | Add metadata sync after `export_launchers`, `delete_launcher`, `rename_launcher`, `check_launcher_exists`, `check_launcher_for_profile` | Current: delegates to `crosshook_core::export::*` functions |
| `src/crosshook-native/src-tauri/src/commands/community.rs` | Add `sync_tap_index()` call after `community_sync()` completes | Current: calls `tap_store.sync_many()` and returns results directly |
| `src/crosshook-native/src-tauri/src/commands/mod.rs` | Add `pub mod metadata;` for new metadata Tauri commands (catalog queries, collection CRUD, launch history queries) | Current modules: community, export, install, launch, profile, settings, shared, steam, update |
| `src/crosshook-native/src-tauri/src/startup.rs` | Add initial full sync on startup after profile auto-load | Current: resolves `auto_load_profile_name` from settings |

#### Frontend Files to Create/Modify (for new metadata-driven features)

| File | Change |
| --- | --- |
| `src/crosshook-native/src/types/metadata.ts` | New TypeScript interfaces for catalog entries, collections, launch history, preferences |
| `src/crosshook-native/src/hooks/useMetadata.ts` | New hook for querying metadata catalog, collections, launch history |
| Various `src/crosshook-native/src/components/*.tsx` | Consume metadata-driven data for favorites, usage stats, launch history display |

### Open Decisions

Decisions marked **RESOLVED** reflect cross-team consensus from security, business, practices, and API research.

1. **RESOLVED — UUID v4 for stable IDs**: Practices research recommends `uuid = { version = "1", features = ["v4", "serde"] }` — the `uuid` crate is more widely used in the Rust ecosystem than `ulid`, and time-ordering is not critical since `created_at` timestamps provide ordering. UUID v4 is random, which avoids leaking timing information.

2. **RESOLVED — `Arc<Mutex<Connection>>` with WAL**: Single `Connection` wrapped in `Arc<Mutex<>>`, matching the pattern already used in the `logging` module. WAL mode for concurrent reads. No connection pool needed for a single-user desktop app. Security requirement: all opens go through a single `open_connection()` factory that applies required PRAGMAs and sets file permissions to `0600`.

3. **RESOLVED — Sync in Tauri command handlers**: Confirmed by all teams. `ProfileStore` remains a pure TOML I/O layer. Metadata sync is best-effort in Tauri commands, matching the existing multi-step orchestration pattern. A failed metadata write after a successful TOML operation does NOT return an error to the frontend — it's logged and deferred to startup reconciliation.

4. **RESOLVED — `rusqlite_migration` for schema migrations**: Use `rusqlite_migration` 2.5.0 which manages `PRAGMA user_version` internally. Do not hand-roll migration tracking or write `user_version` elsewhere.

5. **RESOLVED — `ProfileStore` not modified**: Confirmed as the correct architectural choice. Sync hooks live in Tauri commands, preserving `ProfileStore` testability (existing tests use `tempfile` and don't need SQLite).

6. **RESOLVED — Tokio async bridge**: Launch commands use `tokio::task::spawn_blocking` for metadata writes in `stream_log_lines()` completion. Launch operation rows are created at launch start (`status = 'incomplete'`) and updated on terminal events.

7. **OPEN — CLI integration**: The `crosshook-cli` crate (`crates/crosshook-cli/`) would also need metadata sync if it performs profile/launch operations. For v1, CLI metadata sync is deferred since CLI is secondary to the Tauri desktop app. V2 should introduce an orchestrator layer shared by both Tauri and CLI.

8. **OPEN — Launcher drift auto-repair**: Should drift detection be conservative (warning-only surface, user must manually re-export) or permit high-confidence auto-relink based on slug history and path similarity? Business recommendation: warning-only for v1.

9. **OPEN — `RecentFilesStore` migration timing**: Current TOML-based `RecentFilesStore` can eventually be replaced by SQLite `recent_file_entry` table. This is Phase 2+ — not needed for v1 since it's a working feature today.
