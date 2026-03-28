# SQLite Metadata Layer Phase 3 - Catalog and Intelligence Implementation Plan

Phase 3 extends the existing `MetadataStore` (schema v3, `Arc<Mutex<Connection>>`, 7 metadata files) with five new SQLite tables via a single `migrate_3_to_4` migration, two new metadata submodule files (`community_index.rs` for tap catalog indexing with HEAD commit watermark skip, `cache_store.rs` for bounded external metadata cache), a `collections.rs` submodule for collections/favorites CRUD, usage insights as SQL aggregate projections over existing `launch_operations`, and Tauri command integration across three command files. Every new construct follows the exact `with_conn` fail-soft, free-function, warn-and-continue patterns verified in Phases 1-2 — no new Cargo dependencies, no structural changes to existing modules, and FTS5 explicitly deferred.

## Critically Relevant Files and Documentation

- docs/plans/sqlite3-addition/shared.md: Phase 3 shared context — locked design decisions, schema DDL, patterns, security constraints, integration points
- docs/plans/sqlite3-addition/feature-spec.md: Master spec — Phase 3 schema (line 223), task list (lines 524-534), business rules 7/13, security findings W3/W6/W8/A6
- docs/plans/sqlite3-addition/analysis-context.md: Condensed Phase 3 context — data flow diagrams, parallelization tracks, cross-cutting security concerns, new Tauri commands list
- docs/plans/sqlite3-addition/analysis-code.md: Implementation patterns with exact code shapes — `with_conn`/`with_conn_mut`, free function headers, migration shape, DELETE+INSERT transaction, warn-and-continue, test patterns
- docs/plans/sqlite3-addition/analysis-tasks.md: Task structure analysis — 11 tasks across 5 phases, dependency DAG, parallelization schedule, file-to-task mapping
- src/crosshook-native/crates/crosshook-core/src/metadata/mod.rs: MetadataStore struct, `with_conn`/`with_conn_mut` at lines 59-95, all Phase 1-2 public API — Phase 3 adds ~15 new delegate methods
- src/crosshook-native/crates/crosshook-core/src/metadata/models.rs: MetadataStoreError, existing enums (SyncSource, LaunchOutcome, DriftState) with `as_str()`, MAX_DIAGNOSTIC_JSON_BYTES — template for Phase 3 types
- src/crosshook-native/crates/crosshook-core/src/metadata/migrations.rs: Sequential migration runner (v0→v3) — Phase 3 adds `migrate_3_to_4()` for all five new tables
- src/crosshook-native/crates/crosshook-core/src/metadata/profile_sync.rs: `lookup_profile_id(conn, name)` at lines 72-86 — reusable bridge for collections/favorites FK resolution
- src/crosshook-native/crates/crosshook-core/src/metadata/launcher_sync.rs: `Transaction::new(conn, TransactionBehavior::Immediate)` pattern — template for DELETE+INSERT community re-index
- src/crosshook-native/crates/crosshook-core/src/metadata/launch_history.rs: Size-bounded JSON storage pattern at lines 66-82 — template for cache payload bounds
- src/crosshook-native/crates/crosshook-core/src/community/taps.rs: `CommunityTapSyncResult` with `head_commit: String` at line 44 — watermark source; `CommunityTapSubscription` at lines 19-25
- src/crosshook-native/crates/crosshook-core/src/community/index.rs: `CommunityProfileIndex`, `CommunityProfileIndexEntry` at lines 16-25, schema version check at line 145
- src/crosshook-native/crates/crosshook-core/src/profile/community_schema.rs: `CommunityProfileManifest`, `CommunityProfileMetadata` field list, `CompatibilityRating` enum, `COMMUNITY_PROFILE_SCHEMA_VERSION`
- src/crosshook-native/src-tauri/src/commands/community.rs: `community_sync` at line 124, `map_error` helper at lines 8-10 — Phase 3 adds `State<MetadataStore>` and sync_tap_index hook
- src/crosshook-native/src-tauri/src/commands/export.rs: Warn-and-continue pattern at lines 26-38 — exact template for Phase 3 metadata hooks
- src/crosshook-native/src-tauri/src/commands/profile.rs: Existing metadata hooks after profile CRUD — template for `profile_set_favorite`
- src/crosshook-native/src-tauri/src/lib.rs: `.manage()` at line 80, `invoke_handler!` command list at lines 85-128 — Phase 3 registers ~9 new commands
- CLAUDE.md: Project conventions — commit messages, build commands, Rust style, test commands

## Implementation Plan

### Phase 1: Foundation

#### Task 1.1: Add Phase 3 types and constants to `metadata/models.rs` Depends on [none]

**READ THESE BEFORE TASK**

- src/crosshook-native/crates/crosshook-core/src/metadata/models.rs
- docs/plans/sqlite3-addition/shared.md (Design Decisions table)
- docs/plans/sqlite3-addition/analysis-code.md (Enum with as_str() pattern)

**Instructions**

Files to Modify

- src/crosshook-native/crates/crosshook-core/src/metadata/models.rs

Add the following types after the existing `LaunchOperationRow` struct:

1. **`MAX_CACHE_PAYLOAD_BYTES` constant** — `pub const MAX_CACHE_PAYLOAD_BYTES: usize = 512_000;` Place alongside `MAX_DIAGNOSTIC_JSON_BYTES`.

2. **`CacheEntryStatus` enum** — derives `Debug, Clone, Copy, Serialize, Deserialize` with `#[serde(rename_all = "snake_case")]` and `as_str() -> &'static str`. Variants: `Valid` ("valid"), `Stale` ("stale"), `Oversized` ("oversized"), `Corrupt` ("corrupt"). Follows the exact `DriftState` shape.

3. **`CommunityTapRow` struct** — `#[derive(Debug, Clone, Serialize)]`, `pub(crate)` visibility. Fields: `tap_id: String`, `tap_url: String`, `tap_branch: String`, `local_path: String`, `last_head_commit: Option<String>`, `profile_count: i64`, `last_indexed_at: Option<String>`, `created_at: String`, `updated_at: String`.

4. **`CommunityProfileRow` struct** — `#[derive(Debug, Clone, Serialize)]`, `pub(crate)` visibility. Fields: `id: i64`, `tap_id: String`, `tap_url: String`, `relative_path: String`, `manifest_path: String`, `game_name: Option<String>`, `game_version: Option<String>`, `trainer_name: Option<String>`, `trainer_version: Option<String>`, `proton_version: Option<String>`, `compatibility_rating: Option<String>`, `author: Option<String>`, `description: Option<String>`, `platform_tags: Option<String>`, `schema_version: i64`, `created_at: String`. Note: `tap_url` is denormalized for IPC convenience — populated via JOIN in queries.

5. **`CollectionRow` struct** — `#[derive(Debug, Clone, Serialize)]`, `pub(crate)` visibility. Fields: `collection_id: String`, `name: String`, `description: Option<String>`, `profile_count: i64`, `created_at: String`, `updated_at: String`.

6. **`FailureTrendRow` struct** — `#[derive(Debug, Clone, Serialize)]`, `pub(crate)` visibility. Fields: `profile_name: String`, `successes: i64`, `failures: i64`, `failure_modes: Option<String>`.

7. **`MetadataStoreError::Validation` variant** — Add a new variant `Validation(String)` to the `MetadataStoreError` enum. Include it in the `Display` impl following the existing pattern (e.g., `Validation(msg) => write!(f, "metadata validation error: {msg}")`). This variant is used by `collections.rs` for empty-name rejection and can be reused for future input validation in the metadata layer.

Do NOT update `pub use models::{...}` in `mod.rs` yet — defer to Task 3.1.

Verify with `cargo check --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core`.

#### Task 1.2: Add `migrate_3_to_4()` to `metadata/migrations.rs` Depends on [none]

**READ THESE BEFORE TASK**

- src/crosshook-native/crates/crosshook-core/src/metadata/migrations.rs
- docs/plans/sqlite3-addition/shared.md (Relevant Tables section for full DDL)
- docs/plans/sqlite3-addition/analysis-code.md (Migration Shape section)

**Instructions**

Files to Modify

- src/crosshook-native/crates/crosshook-core/src/metadata/migrations.rs

Add a new migration guard after the existing `if version < 3` block in `run_migrations()`:

```rust
if version < 4 {
    migrate_3_to_4(conn)?;
    conn.pragma_update(None, "user_version", 4_u32)
        .map_err(|source| MetadataStoreError::Database {
            action: "update metadata schema version",
            source,
        })?;
}
```

Add the private `migrate_3_to_4` function using `conn.execute_batch()` with literal-only DDL (W7). The DDL creates five tables:

**`community_taps`**: `tap_id TEXT PRIMARY KEY`, `tap_url TEXT NOT NULL`, `tap_branch TEXT NOT NULL DEFAULT ''` (empty string for absent branch — avoids NULL!=NULL UNIQUE index issue), `local_path TEXT NOT NULL`, `last_head_commit TEXT`, `profile_count INTEGER NOT NULL DEFAULT 0`, `last_indexed_at TEXT`, `created_at TEXT NOT NULL`, `updated_at TEXT NOT NULL`. UNIQUE index on `(tap_url, tap_branch)`.

**`community_profiles`**: `id INTEGER PRIMARY KEY AUTOINCREMENT`, `tap_id TEXT NOT NULL REFERENCES community_taps(tap_id)`, `relative_path TEXT NOT NULL`, `manifest_path TEXT NOT NULL`, `game_name TEXT`, `game_version TEXT`, `trainer_name TEXT`, `trainer_version TEXT`, `proton_version TEXT`, `compatibility_rating TEXT`, `author TEXT`, `description TEXT`, `platform_tags TEXT` (space-separated string), `schema_version INTEGER NOT NULL DEFAULT 1`, `created_at TEXT NOT NULL`. UNIQUE index on `(tap_id, relative_path)`.

**`external_cache_entries`**: `cache_id TEXT PRIMARY KEY`, `source_url TEXT NOT NULL`, `cache_key TEXT NOT NULL UNIQUE`, `payload_json TEXT`, `payload_size INTEGER NOT NULL DEFAULT 0`, `fetched_at TEXT NOT NULL`, `expires_at TEXT`, `created_at TEXT NOT NULL`, `updated_at TEXT NOT NULL`.

**`collections`**: `collection_id TEXT PRIMARY KEY`, `name TEXT NOT NULL UNIQUE`, `description TEXT`, `created_at TEXT NOT NULL`, `updated_at TEXT NOT NULL`.

**`collection_profiles`**: `collection_id TEXT NOT NULL REFERENCES collections(collection_id) ON DELETE CASCADE`, `profile_id TEXT NOT NULL REFERENCES profiles(profile_id)`, `added_at TEXT NOT NULL`, `PRIMARY KEY (collection_id, profile_id)`. Index on `profile_id`.

Do NOT include FTS5 virtual table DDL (deferred).

Use `CREATE TABLE IF NOT EXISTS` and `CREATE INDEX IF NOT EXISTS` for idempotency. Follow the exact DDL pattern from `migrate_0_to_1`, `migrate_1_to_2`, and `migrate_2_to_3`.

### Phase 2: Core Modules

#### Task 2.1: Create `metadata/community_index.rs` Depends on [1.1, 1.2]

**READ THESE BEFORE TASK**

- src/crosshook-native/crates/crosshook-core/src/metadata/launcher_sync.rs (Transaction pattern for DELETE+INSERT)
- src/crosshook-native/crates/crosshook-core/src/metadata/profile_sync.rs (UPSERT pattern, lookup_profile_id)
- src/crosshook-native/crates/crosshook-core/src/community/taps.rs (CommunityTapSyncResult struct)
- src/crosshook-native/crates/crosshook-core/src/community/index.rs (CommunityProfileIndexEntry struct)
- docs/plans/sqlite3-addition/shared.md (Design Decisions — DELETE+INSERT, tap_branch handling, A6 bounds)
- docs/plans/sqlite3-addition/analysis-code.md (DELETE+INSERT Transaction Shape, A6 enforcement)

**Instructions**

Files to Create

- src/crosshook-native/crates/crosshook-core/src/metadata/community_index.rs

Create the file following the `profile_sync.rs` free function pattern. Import `super::{db, MetadataStoreError}`, `CommunityTapSyncResult` from `crate::community::taps`, `CommunityProfileIndexEntry` from `crate::community::index`, `CommunityProfileMetadata` from `crate::profile`, `chrono::Utc`, `rusqlite::{params, Connection, Transaction, TransactionBehavior}`.

Implement:

1. **`pub fn index_community_tap_result(conn: &mut Connection, result: &CommunityTapSyncResult) -> Result<(), MetadataStoreError>`**
   - Extract `tap_url = &result.workspace.subscription.url`, `tap_branch = result.workspace.subscription.branch.as_deref().unwrap_or("")`.
   - Call `get_tap_head_commit(conn, tap_url, tap_branch)`. If returned head commit equals `result.head_commit`, return `Ok(())` immediately (watermark skip).
   - UPSERT the `community_taps` row: `INSERT INTO community_taps (tap_id, tap_url, tap_branch, local_path, last_head_commit, profile_count, last_indexed_at, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?) ON CONFLICT(tap_url, tap_branch) DO UPDATE SET local_path=excluded.local_path, last_head_commit=excluded.last_head_commit, profile_count=excluded.profile_count, last_indexed_at=excluded.last_indexed_at, updated_at=excluded.updated_at`. Use `db::new_id()` for `tap_id` on INSERT. Get `local_path` from `result.workspace.local_path.to_string_lossy()`.
   - Retrieve the `tap_id` for this `(tap_url, tap_branch)`: `SELECT tap_id FROM community_taps WHERE tap_url = ?1 AND tap_branch = ?2`.
   - Open `Transaction::new(conn, TransactionBehavior::Immediate)`.
   - `DELETE FROM community_profiles WHERE tap_id = ?1`.
   - For each entry in `result.index.entries`: access metadata fields via `entry.manifest.metadata` (e.g., `entry.manifest.metadata.game_name`, `entry.manifest.metadata.description`). Validate A6 string length bounds (`game_name` <= 512B, `description` <= 4096B, `platform_tags` <= 2048B, `trainer_name`/`author` <= 512B). If any field exceeds bounds, log `tracing::warn!` with the field name and entry's `relative_path`, then skip this entry (continue to next). Convert `entry.manifest.metadata.platform_tags: Vec<String>` to space-separated string via `.join(" ")`. Get `relative_path` from `entry.relative_path.to_string_lossy()` and `manifest_path` from `entry.manifest_path.to_string_lossy()`. INSERT into `community_profiles`.
   - `tx.commit()`.
   - UPDATE `community_taps SET profile_count = (SELECT COUNT(*) FROM community_profiles WHERE tap_id = ?1) WHERE tap_id = ?1`.

2. **`pub fn list_community_tap_profiles(conn: &Connection, tap_url: Option<&str>) -> Result<Vec<CommunityProfileRow>, MetadataStoreError>`**
   - JOIN `community_profiles cp` with `community_taps ct ON cp.tap_id = ct.tap_id`. When `tap_url` is `Some`, add `WHERE ct.tap_url = ?1`.
   - Map rows to `CommunityProfileRow` with `tap_url` populated from the JOIN.

3. **`fn get_tap_head_commit(conn: &Connection, tap_url: &str, tap_branch: &str) -> Result<Option<String>, MetadataStoreError>`**
   - `SELECT last_head_commit FROM community_taps WHERE tap_url = ?1 AND tap_branch = ?2`.
   - Return `Ok(None)` if no row found.

#### Task 2.2: Create `metadata/cache_store.rs` Depends on [1.1, 1.2]

**READ THESE BEFORE TASK**

- src/crosshook-native/crates/crosshook-core/src/metadata/launch_history.rs (size-bounded JSON storage pattern)
- src/crosshook-native/crates/crosshook-core/src/metadata/profile_sync.rs (free function pattern)
- docs/plans/sqlite3-addition/shared.md (Design Decisions — cache payload bound)
- docs/plans/sqlite3-addition/analysis-code.md (Size-Bounded JSON Payload pattern)

**Instructions**

Files to Create

- src/crosshook-native/crates/crosshook-core/src/metadata/cache_store.rs

Create the file following the `profile_sync.rs` free function pattern. Import `super::{db, MetadataStoreError}`, `super::models::MAX_CACHE_PAYLOAD_BYTES`, `chrono::Utc`, `rusqlite::{params, Connection}`.

Implement:

1. **`pub fn get_cache_entry(conn: &Connection, source_url: &str, cache_key: &str) -> Result<Option<String>, MetadataStoreError>`**
   - `SELECT payload_json FROM external_cache_entries WHERE cache_key = ?1 AND (expires_at IS NULL OR expires_at > ?2)` with `?2 = Utc::now().to_rfc3339()`.
   - Return `Ok(None)` if no row or expired.

2. **`pub fn put_cache_entry(conn: &Connection, source_url: &str, cache_key: &str, payload: &str, expires_at: Option<&str>) -> Result<(), MetadataStoreError>`**
   - Compute `payload_size = payload.len()`.
   - If `payload_size > MAX_CACHE_PAYLOAD_BYTES`: store `payload_json = NULL`, still write `payload_size`. Log `tracing::warn!` with `cache_key` and size.
   - Otherwise store `payload_json = Some(payload)`.
   - UPSERT: `INSERT INTO external_cache_entries (cache_id, source_url, cache_key, payload_json, payload_size, fetched_at, expires_at, created_at, updated_at) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?) ON CONFLICT(cache_key) DO UPDATE SET source_url=excluded.source_url, payload_json=excluded.payload_json, payload_size=excluded.payload_size, fetched_at=excluded.fetched_at, expires_at=excluded.expires_at, updated_at=excluded.updated_at`.
   - Use `db::new_id()` for `cache_id`.

3. **`pub fn evict_expired_cache_entries(conn: &Connection) -> Result<usize, MetadataStoreError>`**
   - `DELETE FROM external_cache_entries WHERE expires_at IS NOT NULL AND expires_at < ?1` with `Utc::now().to_rfc3339()`.
   - Return rows deleted via `conn.execute(...)` return value.

#### Task 2.3: Create `metadata/collections.rs` Depends on [1.1, 1.2]

**READ THESE BEFORE TASK**

- src/crosshook-native/crates/crosshook-core/src/metadata/profile_sync.rs (lookup_profile_id at lines 72-86)
- src/crosshook-native/crates/crosshook-core/src/metadata/mod.rs (with_conn delegation pattern)
- docs/plans/sqlite3-addition/shared.md (Design Decisions — lookup_profile_id reuse, favorites columns)

**Instructions**

Files to Create

- src/crosshook-native/crates/crosshook-core/src/metadata/collections.rs

Create the file following the free function pattern. Import `super::{db, MetadataStoreError}`, `super::profile_sync::lookup_profile_id`, `super::models::CollectionRow`, `chrono::Utc`, `rusqlite::{params, Connection}`.

Implement:

1. **`pub fn list_collections(conn: &Connection) -> Result<Vec<CollectionRow>, MetadataStoreError>`**
   - `SELECT c.collection_id, c.name, c.description, c.created_at, c.updated_at, (SELECT COUNT(*) FROM collection_profiles cp WHERE cp.collection_id = c.collection_id) as profile_count FROM collections c ORDER BY c.name`.

2. **`pub fn create_collection(conn: &Connection, name: &str) -> Result<String, MetadataStoreError>`**
   - Validate name is non-empty (return `MetadataStoreError::Validation` or similar if empty).
   - `INSERT INTO collections (collection_id, name, description, created_at, updated_at) VALUES (?, ?, NULL, ?, ?)`.
   - Use `db::new_id()` for `collection_id`. Return the generated ID.

3. **`pub fn delete_collection(conn: &Connection, collection_id: &str) -> Result<(), MetadataStoreError>`**
   - `DELETE FROM collections WHERE collection_id = ?1`. The `ON DELETE CASCADE` on `collection_profiles` handles membership rows.

4. **`pub fn add_profile_to_collection(conn: &Connection, collection_id: &str, profile_name: &str) -> Result<(), MetadataStoreError>`**
   - Call `lookup_profile_id(conn, profile_name)` to resolve `profile_id`. If `None`, log `tracing::warn!` and return `Ok(())` — profile may not be indexed yet.
   - `INSERT OR IGNORE INTO collection_profiles (collection_id, profile_id, added_at) VALUES (?, ?, ?)`.

5. **`pub fn remove_profile_from_collection(conn: &Connection, collection_id: &str, profile_name: &str) -> Result<(), MetadataStoreError>`**
   - Resolve `profile_id` via `lookup_profile_id`. If `None`, return `Ok(())`.
   - `DELETE FROM collection_profiles WHERE collection_id = ?1 AND profile_id = ?2`.

6. **`pub fn list_profiles_in_collection(conn: &Connection, collection_id: &str) -> Result<Vec<String>, MetadataStoreError>`**
   - `SELECT p.current_filename FROM collection_profiles cp JOIN profiles p ON cp.profile_id = p.profile_id WHERE cp.collection_id = ?1 AND p.deleted_at IS NULL ORDER BY p.current_filename`.

7. **`pub fn set_profile_favorite(conn: &Connection, profile_name: &str, favorite: bool) -> Result<(), MetadataStoreError>`**
   - `UPDATE profiles SET is_favorite = ?1, updated_at = ?2 WHERE current_filename = ?3 AND deleted_at IS NULL`.
   - No migration needed — `is_favorite` column exists from Phase 1 schema.

8. **`pub fn list_favorite_profiles(conn: &Connection) -> Result<Vec<String>, MetadataStoreError>`**
   - `SELECT current_filename FROM profiles WHERE is_favorite = 1 AND deleted_at IS NULL ORDER BY current_filename`.

### Phase 3: MetadataStore API Surface

#### Task 3.1: Add Phase 3 method wrappers to `metadata/mod.rs` Depends on [2.1, 2.2, 2.3]

**READ THESE BEFORE TASK**

- src/crosshook-native/crates/crosshook-core/src/metadata/mod.rs
- docs/plans/sqlite3-addition/analysis-code.md (with_conn Delegation Shape, usage insights inline SQL)
- docs/plans/sqlite3-addition/analysis-tasks.md (P3-T6 task description)

**Instructions**

Files to Modify

- src/crosshook-native/crates/crosshook-core/src/metadata/mod.rs

1. Add submodule declarations after existing `mod launch_history;`:

   ```rust
   mod cache_store;
   mod collections;
   mod community_index;
   ```

   All three are private (`mod`, not `pub mod`) — accessed only through `MetadataStore` methods.

2. Update `pub use models::{...}` to add: `CacheEntryStatus, MAX_CACHE_PAYLOAD_BYTES, CommunityProfileRow, CommunityTapRow, CollectionRow, FailureTrendRow`.

3. Add imports at the top: `use crate::community::taps::CommunityTapSyncResult;`

4. Add public methods to `impl MetadataStore`. Each delegates through `with_conn` or `with_conn_mut`:

   **Community index** (`with_conn_mut` — needs `&mut Connection` for transaction):
   - `index_community_tap_result(&self, result: &CommunityTapSyncResult) -> Result<(), MetadataStoreError>` — action: `"index a community tap"`
   - `list_community_tap_profiles(&self, tap_url: Option<&str>) -> Result<Vec<CommunityProfileRow>, MetadataStoreError>` — action: `"list community tap profiles"` (uses `with_conn`)

   **Collections** (all `with_conn`):
   - `list_collections(&self) -> Result<Vec<CollectionRow>, MetadataStoreError>` — action: `"list collections"`
   - `create_collection(&self, name: &str) -> Result<String, MetadataStoreError>` — action: `"create a collection"`
   - `delete_collection(&self, collection_id: &str) -> Result<(), MetadataStoreError>` — action: `"delete a collection"`
   - `add_profile_to_collection(&self, collection_id: &str, profile_name: &str) -> Result<(), MetadataStoreError>` — action: `"add a profile to a collection"`
   - `remove_profile_from_collection(&self, collection_id: &str, profile_name: &str) -> Result<(), MetadataStoreError>` — action: `"remove a profile from a collection"`
   - `list_profiles_in_collection(&self, collection_id: &str) -> Result<Vec<String>, MetadataStoreError>` — action: `"list profiles in a collection"`

   **Favorites** (all `with_conn`):
   - `set_profile_favorite(&self, profile_name: &str, favorite: bool) -> Result<(), MetadataStoreError>` — action: `"set a profile favorite"`
   - `list_favorite_profiles(&self) -> Result<Vec<String>, MetadataStoreError>` — action: `"list favorite profiles"`

   **Cache** (all `with_conn`):
   - `get_cache_entry(&self, source_url: &str, cache_key: &str) -> Result<Option<String>, MetadataStoreError>` — action: `"get a cache entry"`
   - `put_cache_entry(&self, source_url: &str, cache_key: &str, payload: &str, expires_at: Option<&str>) -> Result<(), MetadataStoreError>` — action: `"put a cache entry"`
   - `evict_expired_cache_entries(&self) -> Result<usize, MetadataStoreError>` — action: `"evict expired cache entries"`

   **Usage insights** (inline SQL via `with_conn` — no separate module file):
   - `query_most_launched(&self, limit: usize) -> Result<Vec<(String, i64)>, MetadataStoreError>` — `SELECT profile_name, COUNT(*) as launch_count FROM launch_operations WHERE status IN ('succeeded', 'failed') GROUP BY profile_name ORDER BY launch_count DESC LIMIT ?1`
   - `query_last_success_per_profile(&self) -> Result<Vec<(String, String)>, MetadataStoreError>` — `SELECT profile_name, MAX(finished_at) as last_success FROM launch_operations WHERE status = 'succeeded' GROUP BY profile_name`
   - `query_failure_trends(&self, days: u32) -> Result<Vec<FailureTrendRow>, MetadataStoreError>` — `SELECT profile_name, COUNT(*) FILTER (WHERE status = 'succeeded') as successes, COUNT(*) FILTER (WHERE status = 'failed') as failures, GROUP_CONCAT(DISTINCT failure_mode) as failure_modes FROM launch_operations WHERE started_at >= datetime('now', '-' || ?1 || ' days') GROUP BY profile_name HAVING failures > 0 ORDER BY failures DESC`

Note: `query_most_launched` and `query_last_success_per_profile` return tuples; `query_failure_trends` returns `FailureTrendRow`. **Critical binding note for `query_failure_trends`**: the SQL `datetime('now', '-N days')` format requires the interval as a string. Construct it as `let interval = format!("-{days} days");` and bind `&interval` as a SQL parameter (e.g., `params![&interval]`). Do NOT interpolate `format!()` inside the SQL string itself — that would violate W7. The full SQL is: `WHERE started_at >= datetime('now', ?1)` with `?1 = "-30 days"` (for 30 days).

### Phase 4: Tauri Integration

#### Task 4.1: Wire community tap index hook into `commands/community.rs` Depends on [3.1]

**READ THESE BEFORE TASK**

- src/crosshook-native/src-tauri/src/commands/community.rs
- src/crosshook-native/src-tauri/src/commands/export.rs (warn-and-continue pattern at lines 26-38)
- docs/plans/sqlite3-addition/analysis-context.md (Data Flow section)

**Instructions**

Files to Modify

- src/crosshook-native/src-tauri/src/commands/community.rs

1. Add import: `use crosshook_core::metadata::MetadataStore;` and `use tauri::State;` (if not already imported).

2. **Modify `community_sync`**: Add `metadata_store: State<'_, MetadataStore>` parameter. After `tap_store.sync_many(&taps)` returns `results`, add a fail-soft indexing loop:

   ```rust
   for result in &results {
       if let Err(e) = metadata_store.index_community_tap_result(result) {
           tracing::warn!(%e, tap_url = %result.workspace.subscription.url,
               "community tap index sync failed");
       }
   }
   ```

3. **Add `community_list_indexed_profiles` command**: A new command that reads from the SQLite index:

   ```rust
   #[tauri::command]
   pub fn community_list_indexed_profiles(
       metadata_store: State<'_, MetadataStore>,
   ) -> Result<Vec<crosshook_core::metadata::CommunityProfileRow>, String> {
       metadata_store.list_community_tap_profiles(None).map_err(map_error)
   }
   ```

The primary `community_sync` return value is unaffected — metadata failure never blocks.

#### Task 4.2: Create `commands/collections.rs` with collection CRUD Depends on [3.1]

**READ THESE BEFORE TASK**

- src/crosshook-native/src-tauri/src/commands/community.rs (command structure, map_error helper)
- src/crosshook-native/src-tauri/src/commands/profile.rs (State<MetadataStore> usage)
- docs/plans/sqlite3-addition/analysis-tasks.md (P3-T8 task description)

**Instructions**

Files to Create

- src/crosshook-native/src-tauri/src/commands/collections.rs

Create the file with all collection CRUD commands following existing command signature shape. Add a private `fn map_error(e: impl ToString) -> String { e.to_string() }` helper (same as `community.rs:8-10`). Check `commands/mod.rs` first — if a shared helper exists, use it instead.

Implement all commands with `State<'_, MetadataStore>` and `Result<T, String>` return:

1. `collection_list(metadata_store) -> Result<Vec<CollectionRow>, String>` — delegates to `metadata_store.list_collections()`
2. `collection_create(name: String, metadata_store) -> Result<String, String>` — returns the new collection_id
3. `collection_delete(collection_id: String, metadata_store) -> Result<(), String>`
4. `collection_add_profile(collection_id: String, profile_name: String, metadata_store) -> Result<(), String>`
5. `collection_remove_profile(collection_id: String, profile_name: String, metadata_store) -> Result<(), String>`
6. `collection_list_profiles(collection_id: String, metadata_store) -> Result<Vec<String>, String>`

All error mapping uses `map_error`. No warn-and-continue needed here — collections are the primary operation, not a side-effect.

#### Task 4.3: Add `profile_set_favorite` and `profile_list_favorites` to `commands/profile.rs` Depends on [3.1]

**READ THESE BEFORE TASK**

- src/crosshook-native/src-tauri/src/commands/profile.rs
- docs/plans/sqlite3-addition/shared.md (Design Decisions — favorites columns reuse)

**Instructions**

Files to Modify

- src/crosshook-native/src-tauri/src/commands/profile.rs

Add two new commands:

1. **`profile_set_favorite`**:

   ```rust
   #[tauri::command]
   pub fn profile_set_favorite(
       name: String,
       favorite: bool,
       metadata_store: State<'_, MetadataStore>,
   ) -> Result<(), String> {
       metadata_store.set_profile_favorite(&name, favorite).map_err(|e| e.to_string())
   }
   ```

2. **`profile_list_favorites`**:

   ```rust
   #[tauri::command]
   pub fn profile_list_favorites(
       metadata_store: State<'_, MetadataStore>,
   ) -> Result<Vec<String>, String> {
       metadata_store.list_favorite_profiles().map_err(|e| e.to_string())
   }
   ```

These write to the existing `profiles.is_favorite` column from Phase 1. No schema change needed.

#### Task 4.4: Register all Phase 3 commands in `lib.rs` Depends on [4.1, 4.2, 4.3]

**READ THESE BEFORE TASK**

- src/crosshook-native/src-tauri/src/lib.rs
- src/crosshook-native/src-tauri/src/commands/mod.rs (if exists)

**Instructions**

Files to Modify

- src/crosshook-native/src-tauri/src/commands/mod.rs
- src/crosshook-native/src-tauri/src/lib.rs

1. Add `pub mod collections;` to `commands/mod.rs` alongside existing module declarations.

2. Add all new Phase 3 commands to the `invoke_handler!` macro in `lib.rs`:

   ```rust
   // Phase 3: Catalog and Intelligence
   commands::community::community_list_indexed_profiles,
   commands::collections::collection_list,
   commands::collections::collection_create,
   commands::collections::collection_delete,
   commands::collections::collection_add_profile,
   commands::collections::collection_remove_profile,
   commands::collections::collection_list_profiles,
   commands::profile::profile_set_favorite,
   commands::profile::profile_list_favorites,
   ```

No new `.manage()` call needed — `MetadataStore` is already registered.

Verify with `cargo check --manifest-path src/crosshook-native/Cargo.toml`.

### Phase 5: Testing

#### Task 5.1: Add Phase 3 unit and integration tests Depends on [4.4]

**READ THESE BEFORE TASK**

- src/crosshook-native/crates/crosshook-core/src/metadata/mod.rs (existing test module, connection() helper)
- docs/plans/sqlite3-addition/analysis-code.md (Test Shape section)
- docs/plans/sqlite3-addition/analysis-tasks.md (P3-T11 test cases)

**Instructions**

Files to Modify

- src/crosshook-native/crates/crosshook-core/src/metadata/mod.rs (add tests in existing `#[cfg(test)] mod tests`)

Add the following tests using `MetadataStore::open_in_memory()` and the existing `connection()` helper. To create test `CommunityTapSyncResult` data, construct mock instances with the necessary fields.

**Community index tests:**

1. **`test_index_tap_result_inserts_tap_and_profile_rows`** — Create a mock `CommunityTapSyncResult` with 2 entries. Call `index_community_tap_result`. Query `community_taps` — verify 1 row with correct `tap_url`, `last_head_commit`, `profile_count = 2`. Query `community_profiles` — verify 2 rows.

2. **`test_index_tap_result_skips_on_unchanged_head`** — Index once, then call again with the same `head_commit`. Verify `community_profiles` rows are unchanged (no DELETE+INSERT churn). Check `updated_at` on `community_taps` row is NOT updated on the second call.

3. **`test_index_tap_result_replaces_stale_profiles`** — Index with 3 entries, then index with 1 entry (different `head_commit`). Verify `community_profiles` COUNT = 1 (the 2 removed entries are gone).

4. **`test_index_tap_result_disabled_store_noop`** — Call on `MetadataStore::disabled()`. Verify `Ok(())` returned.

**Cache store tests:**

5. **`test_put_get_cache_entry_round_trip`** — Put an entry, get it back. Verify same payload returned.

6. **`test_put_cache_entry_idempotent`** — Put twice with same `cache_key`. Verify single row (UPSERT, not duplicate).

7. **`test_cache_payload_oversized_stored_as_null`** — Create a payload string > 512,000 bytes. Put it. Query directly — verify `payload_json IS NULL` but `payload_size` equals the original size.

8. **`test_evict_expired_entries`** — Insert one expired entry and one non-expired. Call `evict_expired_cache_entries`. Verify only expired row removed.

9. **`test_cache_entry_disabled_store_noop`** — Call get on disabled store. Verify `Ok(None)`.

**Collections tests:**

10. **`test_create_collection_returns_id`** — Create a collection, verify non-empty string returned. Query `collections` — verify 1 row.

11. **`test_add_profile_to_collection`** — First create a profile row via `observe_profile_write`. Then create a collection, add the profile. Query `collection_profiles` — verify 1 row.

12. **`test_collection_delete_cascades`** — Create a collection with a profile. Delete the collection. Verify `collection_profiles` rows are gone (CASCADE).

13. **`test_set_profile_favorite_toggles`** — Create a profile via `observe_profile_write`. Call `set_profile_favorite(name, true)`. Query `profiles.is_favorite` — verify 1. Call again with `false` — verify 0.

14. **`test_list_favorite_profiles_excludes_deleted`** — Create 2 profiles, favorite both, then soft-delete one via `observe_profile_delete`. Call `list_favorite_profiles` — verify only the non-deleted one is returned.

**Usage insights tests:**

15. **`test_query_most_launched`** — Create 3 `launch_operations` rows (via `record_launch_started` + `record_launch_finished`). Call `query_most_launched(10)`. Verify ordered by count.

16. **`test_query_failure_trends`** — Create launch operations with various statuses. Call `query_failure_trends(30)`. Verify only profiles with failures appear.

Run all tests with: `cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core`

## Advice

- **`index_community_tap_result` requires `with_conn_mut`, not `with_conn`** — The DELETE+INSERT re-index requires a `Transaction::new(conn, ...)` which needs `&mut Connection`. If you accidentally use `with_conn`, you'll get a compiler error about mutability. Only `index_community_tap_result` needs `with_conn_mut` in Phase 3; all other Phase 3 methods use `with_conn`.

- **The UPSERT for `community_taps` and DELETE+INSERT for `community_profiles` are intentionally different strategies** — The tap-level row is stable (one per subscription) and uses UPSERT on the `(tap_url, tap_branch)` UNIQUE constraint. The profile-level rows must be fully replaced per re-index because UPSERT cannot detect removed profiles. This asymmetry is correct and intentional — do not "simplify" both to the same strategy.

- **`tap_branch` empty string convention is critical** — `result.workspace.subscription.branch` is `Option<String>`. Use `.as_deref().unwrap_or("")` to convert `None` to `""` before storing in SQLite. The `tap_branch TEXT NOT NULL DEFAULT ''` DDL ensures the UNIQUE index on `(tap_url, tap_branch)` works correctly (SQLite `NULL != NULL` would allow duplicates).

- **Usage insights `FILTER (WHERE ...)` syntax requires SQLite >= 3.30.0** — The bundled SQLite in rusqlite 0.38 is >= 3.45.0, so this is safe. However, if the `FILTER` clause causes issues, the equivalent `SUM(CASE WHEN status = 'succeeded' THEN 1 ELSE 0 END)` works on all SQLite versions.

- **`query_failure_trends` day parameter binding** — The SQL `datetime('now', '-N days')` format requires the interval as a string like `"-30 days"`. Construct this as `let interval = format!("-{days} days");` and bind `&interval` as a parameter. Do NOT put `format!()` inside the SQL string itself.

- **The `community_list_indexed_profiles` command returns `CommunityProfileRow` (metadata only)** — This is intentionally smaller than the full `CommunityProfileIndexEntry` which includes the entire `GameProfile`. The SQLite index stores only metadata fields, not the full profile content. The frontend can use this for fast browse/search and fall back to `community_list_profiles` (disk scan) when it needs the full manifest for import.

- **Collections `add_profile_to_collection` silently succeeds when profile not found** — This is a deliberate design choice matching the fail-soft pattern. If a profile hasn't been indexed in SQLite yet (e.g., first run before reconciliation), the collection membership silently does nothing rather than blocking the user. Once the profile is indexed, the next add attempt will succeed. Do not return an error for "profile not found" in the metadata layer.

- **`map_error` helper duplication** — `community.rs` and `collections.rs` each have their own `fn map_error`. Check `commands/mod.rs` for a shared version first. If none exists, the duplication is acceptable (3 lines each, same as Phase 2 approach in `export.rs`). Do not create a shared utility just for this.

- **`external_cache_entries` is infrastructure-only in Phase 3** — No Tauri command currently calls `put_cache_entry` or `get_cache_entry`. The table, functions, and MetadataStore methods exist so future features (ProtonDB integration, artwork caching) can use them without a new migration. The only Phase 3 test that exercises it is the unit test in Task 5.1.

- **FTS5 is explicitly deferred** — Do not add `CREATE VIRTUAL TABLE community_profiles_fts USING fts5(...)` to the migration. Do not add FTS5-related functions to `community_index.rs`. If needed later, FTS5 can be added in a `migrate_4_to_5` without affecting any Phase 3 code.
