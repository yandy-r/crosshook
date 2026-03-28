# SQLite Metadata Layer — Implementation Plan

This plan implements Phase 1 (Identity Foundation) of the SQLite metadata layer for CrossHook. It adds `rusqlite` 0.39.0 with bundled SQLite 3.51.3 to `crosshook-core`, creates a new `metadata` module with stable UUID-based profile identity, rename history tracking, and favorite/pinned flags. Metadata sync hooks are injected into existing Tauri command handlers following the best-effort cascade pattern — `ProfileStore` remains a pure TOML I/O layer. The `MetadataStore` is always-present in Tauri state with an internal `available` flag for fail-soft degradation. Security hardening (file permissions, symlink checks, parameterized queries, PRAGMA verification) is built into the connection factory from day one.

## Critically Relevant Files and Documentation

- docs/plans/sqlite3-addition/feature-spec.md: Master spec — authority matrix, Phase 1 schema, business rules, security findings, adopted defaults
- docs/plans/sqlite3-addition/research-technical.md: Verified type-to-table mappings, API design, integration points with exact function signatures
- docs/plans/sqlite3-addition/research-security.md: W1-W8 security findings with required mitigations (file permissions, parameterized queries, path sanitization)
- docs/plans/sqlite3-addition/research-practices.md: Existing reusable code with file:line references, KISS assessment, testability patterns
- docs/plans/sqlite3-addition/research-patterns.md: Three-constructor pattern, error enum, cascade pattern, test patterns with code examples
- CLAUDE.md: Project conventions — commit messages, build commands, Rust style, test commands
- src/crosshook-native/crates/crosshook-core/src/profile/toml_store.rs: Primary store pattern template — `try_new()`, `with_base_path()`, `validate_name()`, error enum
- src/crosshook-native/crates/crosshook-core/src/logging.rs: `Arc<Mutex<RotatingLogState>>` precedent for MetadataStore connection wrapper
- src/crosshook-native/crates/crosshook-core/src/community/taps.rs: Structured error enum pattern — `Io { action, path, source }` variant
- src/crosshook-native/crates/crosshook-core/src/settings/recent.rs: `data_local_dir()` path pattern — confirms metadata.db location
- src/crosshook-native/src-tauri/src/commands/profile.rs: Best-effort cascade pattern at lines 149-194 — metadata sync hooks follow this
- src/crosshook-native/src-tauri/src/lib.rs: Store initialization and `.manage()` registration — MetadataStore goes here
- src/crosshook-native/src-tauri/src/startup.rs: Startup hooks — reconciliation scan added here

## Implementation Plan

### Phase 1: Prerequisites

#### Task 1.1: Promote `sanitize_display_path()` to shared utility Depends on [none]

**READ THESE BEFORE TASK**

- src/crosshook-native/src-tauri/src/commands/launch.rs
- src/crosshook-native/src-tauri/src/commands/shared.rs
- docs/plans/sqlite3-addition/research-security.md (W2 finding)

**Instructions**

Files to Modify

- src/crosshook-native/src-tauri/src/commands/shared.rs
- src/crosshook-native/src-tauri/src/commands/launch.rs

Move the private `fn sanitize_display_path(path: &str) -> String` from `launch.rs` (line ~301) to `shared.rs` as `pub fn sanitize_display_path`. In `launch.rs`, replace the local definition with `use super::shared::sanitize_display_path;`. The function replaces `$HOME` prefix with `~` in path strings before they cross the IPC boundary. This must land before any Tauri command modifications to prevent duplication across phases. Verify all 8 existing call sites in `launch.rs` still compile after the move.

### Phase 2: Foundation

#### Task 2.1: Add `rusqlite` and `uuid` dependencies Depends on [none]

**READ THESE BEFORE TASK**

- src/crosshook-native/crates/crosshook-core/Cargo.toml
- docs/plans/sqlite3-addition/research-external.md (rusqlite 0.39.0 details)

**Instructions**

Files to Modify

- src/crosshook-native/crates/crosshook-core/Cargo.toml

Add to `[dependencies]`:

```toml
rusqlite = { version = "0.39", features = ["bundled"] }  # bundled required: system SQLite on SteamOS may be pre-3.51.3
uuid     = { version = "1",    features = ["v4", "serde"] }
```

Run `cargo check --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core` to verify. The `bundled` feature compiles SQLite from C source, adding ~30s to clean builds. Incremental builds are unaffected. No other dependencies are needed for Phase 1 — `chrono`, `serde_json`, `directories`, `tracing`, and `tempfile` (dev) are already present.

#### Task 2.2: Create metadata module skeleton and register in lib.rs Depends on [2.1]

**READ THESE BEFORE TASK**

- src/crosshook-native/crates/crosshook-core/src/lib.rs
- src/crosshook-native/crates/crosshook-core/src/profile/toml_store.rs (three-constructor pattern at lines 82-98)
- src/crosshook-native/crates/crosshook-core/src/logging.rs (Arc<Mutex> at lines 118-120)
- docs/plans/sqlite3-addition/research-practices.md (interface design section)

**Instructions**

Files to Create

- src/crosshook-native/crates/crosshook-core/src/metadata/mod.rs

Files to Modify

- src/crosshook-native/crates/crosshook-core/src/lib.rs

Create `metadata/mod.rs` with:

1. Submodule declarations: `mod db; mod migrations; mod models; pub mod profile_sync;`
2. Re-exports: `pub use models::{MetadataStoreError, SyncReport, SyncSource};`
3. `MetadataStore` struct: `pub struct MetadataStore { conn: Option<Arc<Mutex<Connection>>>, available: bool }` with `#[derive(Clone)]`. Use `Option<Arc<Mutex<Connection>>>` so `disabled()` can store `None` without opening a real connection.
4. Four constructors following `toml_store.rs:82-98`:
   - `pub fn try_new() -> Result<Self, String>` — resolves `BaseDirs::data_local_dir().join("crosshook/metadata.db")`, calls `Self::open()`, maps error to String. Use exact error string: `"home directory not found — CrossHook requires a user home directory"`
   - `pub fn with_path(path: &Path) -> Result<Self, MetadataStoreError>` — test injection
   - `pub fn open_in_memory() -> Result<Self, MetadataStoreError>` — unit tests
   - `pub fn disabled() -> Self` — returns store with `conn: None, available: false`. No connection opened. Used when `try_new()` fails.
5. Internal `with_conn<F, T>(&self, action: &'static str, f: F)` helper that returns `Ok(T::default())` when `self.available == false` or `self.conn.is_none()`, otherwise unwraps the `Option` and acquires the mutex lock
6. Stub public API methods for Phase 1: `observe_profile_write`, `observe_profile_rename`, `observe_profile_delete`, `sync_profiles_from_store`

In `lib.rs`, add `pub mod metadata;` after `pub mod logging;` (alphabetical ordering with existing modules).

### Phase 3: Core Module

#### Task 3.1: Create `models.rs` — error types and row structs Depends on [2.2]

**READ THESE BEFORE TASK**

- src/crosshook-native/crates/crosshook-core/src/community/taps.rs (error enum at lines 48-91)
- src/crosshook-native/crates/crosshook-core/src/logging.rs (error enum at lines 20-58)
- src/crosshook-native/crates/crosshook-core/src/export/launcher_store.rs (IPC result structs)
- docs/plans/sqlite3-addition/research-patterns.md (error enum section)

**Instructions**

Files to Create

- src/crosshook-native/crates/crosshook-core/src/metadata/models.rs

Define the following types:

1. `MetadataStoreError` enum following `CommunityTapError` pattern:
   - `HomeDirectoryUnavailable` — `try_new()` failure
   - `Database { action: &'static str, source: rusqlite::Error }` — SQL operation failures
   - `Io { action: &'static str, path: PathBuf, source: std::io::Error }` — file permission/symlink failures
   - `Corrupt(String)` — `PRAGMA quick_check` failures
   - `SymlinkDetected(PathBuf)` — W5 security check failure
   - Implement `Display`, `Error`, `From<rusqlite::Error>`. Do NOT implement `From<std::io::Error>` — the `Io` variant requires `action` and `path` context that a blanket `From` cannot provide. Use explicit `.map_err(|e| MetadataStoreError::Io { action: "...", path: p.to_path_buf(), source: e })` at each call site instead.

2. `SyncSource` enum: `AppWrite`, `AppRename`, `AppDuplicate`, `AppDelete`, `FilesystemScan`, `Import`, `InitialCensus`

3. `SyncReport` struct with `#[derive(Debug, Clone, Default, Serialize, Deserialize)]`:
   - `profiles_seen: usize`, `created: usize`, `updated: usize`, `deleted: usize`, `errors: Vec<String>`

4. `ProfileRow` struct (internal, not IPC): `profile_id: String`, `current_filename: String`, `current_path: String`, `game_name: Option<String>`, `launch_method: Option<String>`, `is_favorite: bool`, `is_pinned: bool`, `created_at: String`, `updated_at: String`

This file must compile first — every other metadata file depends on `MetadataStoreError`.

#### Task 3.2: Create `db.rs` — connection factory with security hardening Depends on [3.1]

**READ THESE BEFORE TASK**

- docs/plans/sqlite3-addition/research-security.md (W1, W5, W7 findings)
- docs/plans/sqlite3-addition/research-external.md (PRAGMA reference, connection setup)
- src/crosshook-native/crates/crosshook-core/src/logging.rs (Arc<Mutex> pattern)

**Instructions**

Files to Create

- src/crosshook-native/crates/crosshook-core/src/metadata/db.rs

Implement:

1. `pub fn open_at_path(path: &Path) -> Result<Connection, MetadataStoreError>` — the single connection factory:
   - **W5 symlink check**: if path exists, call `std::fs::symlink_metadata(path)` — reject if `is_symlink()` with `MetadataStoreError::SymlinkDetected`
   - **W1 parent dir**: `create_dir_all` parent with `0o700` permissions
   - **W1 file permissions**: after `Connection::open(path)`, call `set_permissions(path, 0o600)`
   - **PRAGMAs** via `execute_batch()` with hard-coded string literals only (W7):

     ```sql
     PRAGMA journal_mode=WAL;
     PRAGMA foreign_keys=ON;
     PRAGMA synchronous=NORMAL;
     PRAGMA busy_timeout=5000;
     PRAGMA secure_delete=ON;
     ```

   - **Verify PRAGMAs**: re-read `journal_mode` and `foreign_keys` after setting — silent PRAGMA failure is a real gotcha
   - **Application ID**: `conn.pragma_update(None, "application_id", &0x43484B00_i32)` (CrossHook magic number)
   - **A5 quick_check**: `conn.query_row("PRAGMA quick_check", [], |row| row.get::<_, String>(0))` — return `MetadataStoreError::Corrupt` if result is not `"ok"`

2. `pub fn open_in_memory() -> Result<Connection, MetadataStoreError>` — for unit tests, same PRAGMAs minus file permissions/symlink check

3. `pub fn new_id() -> String` — wraps `uuid::Uuid::new_v4().to_string()`

All SQL in this file must be string literals. Never use `format!()` in any SQL string.

#### Task 3.3: Create `migrations.rs` — schema DDL Depends on [3.1]

**READ THESE BEFORE TASK**

- docs/plans/sqlite3-addition/feature-spec.md (Phase 1 schema tables)
- docs/plans/sqlite3-addition/research-external.md (UPSERT, foreign keys, user_version)

**Instructions**

Files to Create

- src/crosshook-native/crates/crosshook-core/src/metadata/migrations.rs

Implement:

1. `pub fn run_migrations(conn: &Connection) -> Result<(), MetadataStoreError>` — hand-rolled runner using `PRAGMA user_version`:
   - Read current version: `conn.pragma_query_value(None, "user_version", |row| row.get::<_, u32>(0))`
   - If version < 1, run migration 0→1
   - After migration, set version: `conn.pragma_update(None, "user_version", &1_u32)`

2. Migration 0→1 DDL (use `execute_batch()` with literal SQL — this is the one permitted use of `execute_batch` for DDL):

   ```sql
   CREATE TABLE IF NOT EXISTS profiles (
       profile_id TEXT PRIMARY KEY,
       current_filename TEXT NOT NULL UNIQUE,
       current_path TEXT NOT NULL,
       game_name TEXT,
       launch_method TEXT,
       content_hash TEXT,
       is_favorite INTEGER NOT NULL DEFAULT 0,
       is_pinned INTEGER NOT NULL DEFAULT 0,
       source_profile_id TEXT REFERENCES profiles(profile_id),
       deleted_at TEXT,
       created_at TEXT NOT NULL,
       updated_at TEXT NOT NULL
   );
   CREATE INDEX IF NOT EXISTS idx_profiles_current_filename ON profiles(current_filename);

   CREATE TABLE IF NOT EXISTS profile_name_history (
       id INTEGER PRIMARY KEY AUTOINCREMENT,
       profile_id TEXT NOT NULL REFERENCES profiles(profile_id),
       old_name TEXT,
       new_name TEXT NOT NULL,
       old_path TEXT,
       new_path TEXT NOT NULL,
       source TEXT NOT NULL,
       created_at TEXT NOT NULL
   );
   CREATE INDEX IF NOT EXISTS idx_profile_name_history_profile_id ON profile_name_history(profile_id);
   ```

Keep migration functions small and sequential. Future migrations add to this file without touching existing DDL blocks.

#### Task 3.4: Create `profile_sync.rs` — profile lifecycle reconciliation Depends on [3.1, 3.2, 3.3]

**READ THESE BEFORE TASK**

- src/crosshook-native/crates/crosshook-core/src/profile/toml_store.rs (ProfileStore API, validate_name)
- src/crosshook-native/crates/crosshook-core/src/profile/models.rs (GameProfile struct)
- docs/plans/sqlite3-addition/research-business.md (workflows: create, rename, delete, duplicate)
- docs/plans/sqlite3-addition/feature-spec.md (UPSERT reconciliation pattern)

**Instructions**

Files to Create

- src/crosshook-native/crates/crosshook-core/src/metadata/profile_sync.rs

Implement four functions that take `&Connection` (called from `MetadataStore` methods which handle the mutex):

1. `pub fn observe_profile_write(conn: &Connection, name: &str, profile: &GameProfile, path: &Path, source: SyncSource) -> Result<(), MetadataStoreError>`
   - Call `validate_name(name)` before any SQL — reject invalid names at the API boundary
   - UPSERT: `INSERT INTO profiles (...) VALUES (?, ?, ...) ON CONFLICT(current_filename) DO UPDATE SET game_name=excluded.game_name, launch_method=excluded.launch_method, content_hash=excluded.content_hash, updated_at=excluded.updated_at`
   - On INSERT (new profile): generate UUID via `db::new_id()`, set `created_at` to `chrono::Utc::now().to_rfc3339()`
   - Extract `game_name` from `profile.game.name`, `launch_method` from `profile.launch.method.to_string()`
   - All parameters via `params![]` — never `format!()`

2. `pub fn observe_profile_rename(conn: &Connection, old_name: &str, new_name: &str, old_path: &Path, new_path: &Path) -> Result<(), MetadataStoreError>`
   - Validate both names
   - UPDATE `profiles` SET `current_filename = ?, current_path = ?, updated_at = ?` WHERE `current_filename = ?`
   - INSERT into `profile_name_history` with `source = 'app_rename'`
   - Use `TransactionBehavior::Immediate` for the combined update+insert

3. `pub fn observe_profile_delete(conn: &Connection, name: &str) -> Result<(), MetadataStoreError>`
   - Soft-delete: `UPDATE profiles SET deleted_at = ? WHERE current_filename = ?` — never hard DELETE (tombstone rule)

4. `pub fn sync_profiles_from_store(conn: &Connection, store: &ProfileStore) -> Result<SyncReport, MetadataStoreError>`
   - Call `store.list()` to get all TOML filenames
   - For each filename: call `observe_profile_write` with `SyncSource::InitialCensus`
   - For first-run (INSERT path of UPSERT only): use file `mtime` as `created_at` timestamp (via `std::fs::metadata(path).modified()`). Do NOT overwrite `created_at` on the UPDATE path — it must be preserved across subsequent reconciliation runs.
   - Detect SQLite rows with `deleted_at IS NULL` but no matching TOML file — mark as potentially deleted
   - Return `SyncReport` with counts
   - `content_hash`: compute as SHA256 of raw TOML file bytes (`std::fs::read(path)` then `sha256::digest()`). Leave NULL if the file cannot be read. The `sha2` crate is NOT a dependency — use `format!("{:x}", md5)` or simply leave `content_hash` as NULL for Phase 1 and defer hashing to Phase 2 when content-drift detection is needed. The column exists in schema for forward compatibility.

### Phase 4: Tauri Integration

#### Task 4.1: Register `MetadataStore` in Tauri app Depends on [3.2, 3.3, 3.4]

**READ THESE BEFORE TASK**

- src/crosshook-native/src-tauri/src/lib.rs (store init at lines 15-31, .manage() at lines 62-66)
- docs/plans/sqlite3-addition/analysis-code.md (integration points section)

**Instructions**

Files to Modify

- src/crosshook-native/src-tauri/src/lib.rs

1. Add `use crosshook_core::metadata::MetadataStore;`
2. After community_tap_store initialization (~line 30), add MetadataStore init with fail-soft pattern:

   ```rust
   let metadata_store = MetadataStore::try_new().unwrap_or_else(|error| {
       tracing::warn!(%error, "metadata store unavailable — SQLite features disabled");
       MetadataStore::disabled()
   });
   ```

   Unlike other stores, do NOT call `process::exit(1)` on failure.

3. **CRITICAL: Clone before `.manage()` takes ownership.** `.manage()` consumes the value, but the `setup()` closure also needs a reference:

   ```rust
   let metadata_for_startup = metadata_store.clone();  // clone BEFORE .manage()
   // ... later in builder chain:
   .manage(metadata_store)  // ownership moved here
   ```

4. Add `.manage(metadata_store)` after `.manage(community_tap_store)` (~line 65)
5. In the `setup()` closure, after `startup::resolve_auto_load_profile_name()`, call the reconciliation wrapper from `startup.rs` (created in Task 4.3):

   ```rust
   if let Err(error) = startup::run_metadata_reconciliation(&metadata_for_startup, &profile_store) {
       tracing::warn!(%error, "startup metadata reconciliation failed");
   }
   ```

   Use the cloned `metadata_for_startup` reference, not the moved `metadata_store`.

#### Task 4.2: Add metadata sync hooks to profile commands Depends on [1.1, 4.1]

**READ THESE BEFORE TASK**

- src/crosshook-native/src-tauri/src/commands/profile.rs (full file — understand each command's flow)
- docs/plans/sqlite3-addition/analysis-code.md (5 hook injection points with exact line numbers)
- docs/plans/sqlite3-addition/research-patterns.md (best-effort cascade pattern)

**Instructions**

Files to Modify

- src/crosshook-native/src-tauri/src/commands/profile.rs

1. Add `use crosshook_core::metadata::{MetadataStore, SyncSource};` at the top
2. Add `metadata_store: State<'_, MetadataStore>` parameter to these 6 commands: `profile_save`, `profile_save_launch_optimizations`, `profile_delete`, `profile_rename`, `profile_duplicate`, `profile_import_legacy`
3. After each critical TOML operation (the line with `?`), add the best-effort metadata sync.

**GOTCHA: `profile_path()` is private.** `ProfileStore::profile_path()` is a private method (verified at `toml_store.rs:274-277`). You cannot call `store.profile_path(&name)` from command handlers. Instead, reconstruct the path: `store.base_path.join(format!("{name}.toml"))`. If `base_path` is also private, pass the profile directory path from `try_new()` context or use `BaseDirs::new().config_dir().join("crosshook/profiles/{name}.toml")`.

**profile_save** (after `store.save(&name, &data).map_err(map_error)?`):

```rust
let profile_path = store.base_path.join(format!("{name}.toml"));
if let Err(e) = metadata_store.observe_profile_write(&name, &data, &profile_path, SyncSource::AppWrite) {
    tracing::warn!(%e, profile_name = %name, "metadata sync after profile_save failed");
}
```

**profile_save_launch_optimizations** (after `store.save_launch_optimizations(&name, payload.enabled_option_ids).map_err(map_error)?`):

```rust
// Content changed — re-sync the profile snapshot. Load the updated profile to extract fields.
if let Ok(updated) = store.load(&name) {
    let profile_path = store.base_path.join(format!("{name}.toml"));
    if let Err(e) = metadata_store.observe_profile_write(&name, &updated, &profile_path, SyncSource::AppWrite) {
        tracing::warn!(%e, profile_name = %name, "metadata sync after save_launch_optimizations failed");
    }
}
```

**profile_delete** (after `store.delete(&name).map_err(map_error)?`):

```rust
if let Err(e) = metadata_store.observe_profile_delete(&name) {
    tracing::warn!(%e, profile_name = %name, "metadata sync after profile_delete failed");
}
```

**profile_rename** (after `store.rename(&old_name, &new_name).map_err(map_error)?`, before existing launcher cleanup):

```rust
let old_path = store.base_path.join(format!("{old_name}.toml"));
let new_path = store.base_path.join(format!("{new_name}.toml"));
if let Err(e) = metadata_store.observe_profile_rename(&old_name, &new_name, &old_path, &new_path) {
    tracing::warn!(%e, %old_name, %new_name, "metadata sync after profile_rename failed");
}
```

**profile_duplicate** (after `let result = store.duplicate(&name).map_err(map_error)?`):

```rust
let copy_path = store.base_path.join(format!("{}.toml", result.name));
if let Err(e) = metadata_store.observe_profile_write(&result.name, &result.profile, &copy_path, SyncSource::AppDuplicate) {
    tracing::warn!(%e, name = %result.name, "metadata sync after profile_duplicate failed");
}
```

**profile_import_legacy** (after `let profile = store.import_legacy(...).map_err(map_error)?`):

```rust
let stem = Path::new(&path).file_stem().and_then(|s| s.to_str()).unwrap_or("imported");
let import_path = store.base_path.join(format!("{stem}.toml"));
if let Err(e) = metadata_store.observe_profile_write(stem, &profile, &import_path, SyncSource::Import) {
    tracing::warn!(%e, profile_name = %stem, "metadata sync after import_legacy failed");
}
```

Also add `profile_save_launch_optimizations` hook — content changed, so UPSERT the profile snapshot.

#### Task 4.3: Add startup reconciliation scan Depends on [4.1]

**READ THESE BEFORE TASK**

- src/crosshook-native/src-tauri/src/startup.rs (existing StartupError, resolve_auto_load_profile_name)
- docs/plans/sqlite3-addition/analysis-tasks.md (T9 details)

**Instructions**

Files to Modify

- src/crosshook-native/src-tauri/src/startup.rs

1. Add `use crosshook_core::metadata::{MetadataStore, MetadataStoreError};`
2. Extend `StartupError` enum with `Metadata(MetadataStoreError)` variant, add `From<MetadataStoreError>` impl and `Display` arm
3. Add function:

   ```rust
   pub fn run_metadata_reconciliation(
       metadata_store: &MetadataStore,
       profile_store: &ProfileStore,
   ) -> Result<(), StartupError> {
       let report = metadata_store.sync_profiles_from_store(profile_store)?;
       if report.created > 0 || report.updated > 0 {
           tracing::info!(
               created = report.created,
               updated = report.updated,
               "startup metadata reconciliation complete"
           );
       }
       Ok(())
   }
   ```

4. This function is called from `lib.rs` setup closure as best-effort (already added in Task 4.1)

### Phase 5: Testing

#### Task 5.1: Add metadata module unit and integration tests Depends on [4.2, 4.3]

**READ THESE BEFORE TASK**

- src/crosshook-native/crates/crosshook-core/src/profile/toml_store.rs (test module at end of file)
- src/crosshook-native/src-tauri/src/startup.rs (store_pair test helper at lines 72-78)
- docs/plans/sqlite3-addition/research-practices.md (testability patterns section)

**Instructions**

Files to Modify

- src/crosshook-native/crates/crosshook-core/src/metadata/mod.rs (add `#[cfg(test)] mod tests`)

Add unit tests using `MetadataStore::open_in_memory()`:

1. **`test_observe_profile_write_creates_row`** — write a profile, query `profiles` table, verify row exists with correct UUID, filename, game_name, launch_method
2. **`test_observe_profile_write_idempotent`** — write same profile twice, verify single row (UPSERT not duplicate)
3. **`test_observe_profile_rename_creates_history`** — rename, verify `profile_name_history` has one row with correct old/new names
4. **`test_observe_profile_delete_tombstones`** — delete, verify `deleted_at` is set, row not hard-deleted
5. **`test_sync_profiles_from_store`** — create `ProfileStore::with_base_path(tempdir)`, save 3 profiles, run sync, verify 3 rows in SQLite
6. **`test_unavailable_store_noop`** — `MetadataStore::disabled()`, call all methods, verify no panics, all return Ok
7. **`test_file_permissions`** — `MetadataStore::with_path(tempdir)`, verify `metadata.db` has `0o600` permissions (use `std::fs::metadata(path).permissions().mode() & 0o777`)
8. **`test_symlink_rejected`** — create symlink at DB path, attempt `MetadataStore::with_path()`, verify `SymlinkDetected` error

Run with: `cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core`

## Advice

- **Write `models.rs` first within Phase 3** — `MetadataStoreError` is referenced by every other metadata file. If implementing tasks in parallel, stub the error enum inline in `db.rs`/`migrations.rs` and reconcile after `models.rs` lands, or simply implement T3.1 before T3.2/T3.3.
- **The `profile_path()` method on `ProfileStore` is not public** — if it is private, the Tauri command hooks in Task 4.2 need to reconstruct the path from `store.base_path.join(format!("{name}.toml"))` or the path needs to be derived from the name. Check visibility before implementing.
- **`validate_name()` is re-exported from `profile/mod.rs`** but lives in `profile/legacy.rs`. Import it as `crosshook_core::profile::validate_name` in the metadata module.
- **WAL mode is persistent** — once set on a database, it survives connection close/reopen. The PRAGMA in `db.rs` is idiomatic for first-open but not strictly required on subsequent opens. Still set it for defense-in-depth.
- **`execute_batch()` for PRAGMA setup is acceptable** because the strings are hard-coded literals. The W7 security rule only prohibits runtime-derived values in `execute_batch()`. For `user_version` (which takes a runtime value), use `conn.pragma_update()` instead.
- **Startup reconciliation must not block the app** — if `sync_profiles_from_store` panics or hangs, the Tauri setup closure would block indefinitely. Consider wrapping in a timeout or running in a detached `tauri::async_runtime::spawn` task.
- **`Arc<Mutex<Connection>>` clone semantics** — all clones of `MetadataStore` share the same connection and mutex. This is intentional for Tauri state. But it means all metadata operations are serialized through the mutex, which is fine for a single-user desktop app with low write volume.
- **The `disabled()` constructor must not open a real connection** — it should store `None` internally (use `Option<Arc<Mutex<Connection>>>`) or open a `:memory:` connection that's never queried. The `with_conn` helper skips the lock entirely when `available == false`.
- **Phase 2 blocker**: `LaunchRequest` in `launch/request.rs` has no `profile_name` field. This must be added before any Phase 2 launch history work begins. It is intentionally out of scope for this plan.
