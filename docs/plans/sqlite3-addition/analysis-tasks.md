# Task Structure Analysis: SQLite Metadata Layer Phase 3 — Catalog and Intelligence

## Executive Summary

Phase 3 decomposes into **11 atomic tasks** across **5 sequential phases**, with a maximum parallelism of **4 concurrent tasks**. No external blockers exist (Phases 1 and 2 are fully merged). The critical path runs: models/migrations (foundation) → `community_index.rs` → `mod.rs` API surface → `commands/community.rs` hook → tests. `cache_store.rs` and the collections module are fully independent of the community index path and can run in parallel with it after the foundation. Usage insights require no new schema and can begin as soon as `mod.rs` wrappers land. The `collections.rs` command file is the only new Tauri source file required.

---

## Cross-Cutting Rules (Every Task Must Enforce)

These rules apply across all Phase 3 tasks. An implementor working on any single task must not introduce a violation in their file, even if surrounding code does not yet enforce the rule.

1. **Best-effort cascade only**: All Tauri call sites for Phase 3 metadata methods use `if let Err(e) { tracing::warn!(...) }` — never `?` on a metadata call. Metadata failure must never block the primary operation (community sync, profile save, etc.).
2. **DELETE+INSERT for `community_profiles` re-index**: Never use UPSERT for the profile rows belonging to a tap. Always open a `Transaction::new(conn, TransactionBehavior::Immediate)`, DELETE all rows for the tap, INSERT the new entries, then commit. Stale ghost rows from removed profiles will appear if this is violated.
3. **UPSERT is correct for `community_taps` watermark rows**: The tap-level row (one per `(tap_url, tap_branch)`) uses UPSERT on the `UNIQUE(tap_url, COALESCE(tap_branch, ''))` index. The mismatch between tap-row and profile-rows is intentional.
4. **`tap_branch` as empty string, not NULL**: Store absent branch as `''` (`NOT NULL DEFAULT ''`). The `COALESCE` in the UNIQUE index expression handles legacy data; all new writes store `""` directly. Never bind `None` as NULL for this column.
5. **`platform_tags` as space-separated string**: Store `Vec<String>` as `"linux steam-deck"` (space-separated), not as JSON array. Better LIKE matching and FTS5 tokenization if FTS is added later.
6. **A6 string length bounds — reject, do not truncate**: `game_name` ≤ 512 bytes, `description` ≤ 4 KB, `platform_tags` ≤ 2 KB, `trainer_name`/`author` ≤ 512 bytes. Return `Err(MetadataStoreError::...)` on violation; do not silently truncate.
7. **`MAX_CACHE_PAYLOAD_BYTES = 512_000` (512 KB)**: For `external_cache_entries` payload. Distinct from Phase 2's `MAX_DIAGNOSTIC_JSON_BYTES = 4_096`. Store `NULL` when oversized; always write `payload_size` column even when payload is NULL.
8. **No `format!()` in SQL strings (W7)**: All SQL must be string literals. Runtime values go in `params![]` only — including DDL in migrations and DML in sync functions.
9. **`lookup_profile_id` reuse**: Collection membership and favorites writes resolve `profile_name → profile_id` via `profile_sync::lookup_profile_id(conn, name)`. Do not duplicate the lookup query.
10. **`map_error` helper in community commands**: The existing private `fn map_error(e: impl ToString) -> String` in `community.rs` must be used throughout Phase 3 community command additions.
11. **FTS5 deferred**: Do not create the `community_profiles_fts` virtual table in `migrate_3_to_4`. The migration DDL must not include it. FTS5 is deferred unless `LIKE` proves insufficient.

---

## Recommended Phase Structure

### Phase 1: Foundation (2 tasks — parallel)

No prerequisites. Both tasks can start immediately and execute in parallel. Every Phase 3 file depends on these two completing.

**Task P3-T1 — Add Phase 3 types and constants to `metadata/models.rs`**

Files touched: `metadata/models.rs` (1 file)

Add the following:

- `CacheEntryStatus` enum: `Valid`, `Stale`, `Oversized`, `Corrupt` — follows `DriftState` shape exactly (`Copy`, `as_str()`, `#[serde(rename_all = "snake_case")]`)
- `MAX_CACHE_PAYLOAD_BYTES: usize = 512_000` constant (distinct from `MAX_DIAGNOSTIC_JSON_BYTES`)
- `CommunityTapRow` struct (for query results from `community_taps` table)
- `CommunityProfileRow` struct (for query results from `community_profiles` table — metadata fields only, no full `GameProfile`)
- `CollectionRow` struct with `collection_id: String`, `name: String`, `description: Option<String>`, `created_at: String`, `updated_at: String`
- `CollectionSummary` struct (for IPC — same as `CollectionRow` plus `profile_count: usize`)
- `FailureTrendRow` struct (for usage insights queries)
- Update `pub use models::{...}` in `mod.rs` — defer this to the P3-T6 mod.rs task

**Task P3-T2 — Add `migrate_3_to_4()` to `metadata/migrations.rs`**

Files touched: `metadata/migrations.rs` (1 file)

Add DDL for all five Phase 3 tables in a single `migrate_3_to_4` function and add the `if version < 4` guard:

- `community_taps`: `tap_id TEXT PK`, `tap_url TEXT NOT NULL`, `tap_branch TEXT NOT NULL DEFAULT ''`, `local_path TEXT NOT NULL`, `last_head_commit TEXT`, `profile_count INTEGER NOT NULL DEFAULT 0`, `last_indexed_at TEXT`, `created_at TEXT NOT NULL`, `updated_at TEXT NOT NULL`; UNIQUE index `ON community_taps(tap_url, tap_branch)` (no COALESCE needed with NOT NULL DEFAULT '')
- `community_profiles`: `id INTEGER PK AUTOINCREMENT`, `tap_id TEXT NOT NULL REFERENCES community_taps(tap_id)`, `relative_path TEXT NOT NULL`, `manifest_path TEXT NOT NULL`, all metadata text columns, `schema_version INTEGER NOT NULL DEFAULT 1`, `created_at TEXT NOT NULL`; UNIQUE index on `(tap_id, relative_path)`; indexes on `game_name`, `compat_rating`
- `external_cache_entries`: `cache_id TEXT PK`, `source_url TEXT NOT NULL`, `cache_key TEXT NOT NULL`, `payload_json TEXT`, `payload_size INTEGER NOT NULL DEFAULT 0`, `fetched_at TEXT NOT NULL`, `expires_at TEXT`, `created_at TEXT NOT NULL`, `updated_at TEXT NOT NULL`; UNIQUE index on `(source_url, cache_key)`; index on `expires_at`
- `collections`: `collection_id TEXT PK`, `name TEXT NOT NULL UNIQUE`, `description TEXT`, `created_at TEXT NOT NULL`, `updated_at TEXT NOT NULL`
- `collection_profiles`: composite PK `(collection_id, profile_id)`, FK to `collections(collection_id) ON DELETE CASCADE`, FK to `profiles(profile_id)`, `added_at TEXT NOT NULL`; index on `profile_id`

Do NOT include the FTS5 virtual table DDL (deferred).

---

### Phase 2: Core Modules (3 tasks — parallel)

All three tasks are unblocked after Phase 1 completes. They have no inter-dependencies and can run simultaneously.

**Task P3-T3 — Create `metadata/community_index.rs`**

Files touched: `metadata/community_index.rs` (new, 1 file)

Implement the following free functions (all with `conn: &Connection` or `conn: &mut Connection` first arg):

- `pub fn index_community_tap_result(conn: &mut Connection, result: &CommunityTapSyncResult) -> Result<(), MetadataStoreError>`
  - Checks stored `last_head_commit` for this `(tap_url, tap_branch)` pair against `result.head_commit`
  - If equal: skip (return `Ok(())`) — this is the HEAD watermark optimization
  - If different or absent: open `Transaction::new(conn, TransactionBehavior::Immediate)`, UPSERT the `community_taps` row, DELETE all `community_profiles` rows for this `tap_id`, INSERT all entries from `result.index.entries`, commit
  - Apply A6 length bounds on all string fields before INSERT; skip oversized entries with a `tracing::warn!`
  - Convert `Vec<String>` platform_tags to space-separated string for storage
  - Update `profile_count` on the `community_taps` row after INSERT batch

- `pub fn list_community_tap_profiles(conn: &Connection, tap_url: Option<&str>) -> Result<Vec<CommunityProfileRow>, MetadataStoreError>`
  - When `tap_url` is `Some`, filter by `community_taps.tap_url = ?`; when `None`, return all rows
  - JOIN `community_profiles` with `community_taps` to resolve `tap_url`

- `pub fn get_tap_head_commit(conn: &Connection, tap_url: &str, tap_branch: &str) -> Result<Option<String>, MetadataStoreError>`
  - Returns `last_head_commit` from `community_taps` for the given `(tap_url, tap_branch)` pair

Pattern reference: `launcher_sync.rs` for the `Transaction::new` pattern; `profile_sync.rs` for the UPSERT pattern; `shared.md` for the exact tap branch empty-string convention.

**Task P3-T4 — Create `metadata/cache_store.rs`**

Files touched: `metadata/cache_store.rs` (new, 1 file)

Implement:

- `pub fn get_cache_entry(conn: &Connection, source_url: &str, cache_key: &str) -> Result<Option<String>, MetadataStoreError>`
  - Returns `payload_json` if entry exists and not expired (`expires_at IS NULL OR expires_at > datetime('now')`); returns `None` otherwise
- `pub fn put_cache_entry(conn: &Connection, source_url: &str, cache_key: &str, payload: &str, expires_at: Option<&str>) -> Result<(), MetadataStoreError>`
  - Validate payload is parseable JSON (`serde_json::from_str::<serde_json::Value>(payload).is_ok()`)
  - If `payload.len() > MAX_CACHE_PAYLOAD_BYTES`: store NULL + `payload_size` + warn; do not error
  - UPSERT on `(source_url, cache_key)` conflict — cache writes are always idempotent
- `pub fn evict_expired_cache_entries(conn: &Connection) -> Result<usize, MetadataStoreError>`
  - `DELETE FROM external_cache_entries WHERE expires_at IS NOT NULL AND expires_at < datetime('now')`
  - Returns rows deleted count

Pattern reference: `launch_history.rs` lines 66–82 for the size-bounded JSON storage pattern; `models.rs` for `MAX_DIAGNOSTIC_JSON_BYTES` placement analogue.

**Task P3-T5 — Create `metadata/collections.rs`**

Files touched: `metadata/collections.rs` (new, 1 file)

Implement:

- `pub fn list_collections(conn: &Connection) -> Result<Vec<CollectionRow>, MetadataStoreError>`
  - SELECT with optional subquery for `profile_count` from `collection_profiles`
- `pub fn create_collection(conn: &Connection, name: &str) -> Result<String, MetadataStoreError>`
  - Validate name is non-empty; INSERT `collections` row; return `collection_id` (UUID from `db::new_id()`)
- `pub fn delete_collection(conn: &Connection, collection_id: &str) -> Result<(), MetadataStoreError>`
  - DELETE from `collections`; `collection_profiles` cascade handles membership rows
- `pub fn add_profile_to_collection(conn: &Connection, collection_id: &str, profile_name: &str) -> Result<(), MetadataStoreError>`
  - Call `profile_sync::lookup_profile_id(conn, profile_name)` to resolve FK
  - If `profile_id` not found: return `Err(MetadataStoreError::NotFound {...})`
  - INSERT into `collection_profiles` with `added_at = datetime('now')`
- `pub fn remove_profile_from_collection(conn: &Connection, collection_id: &str, profile_name: &str) -> Result<(), MetadataStoreError>`
  - Resolve `profile_id` via `lookup_profile_id`; DELETE from `collection_profiles`
- `pub fn list_profiles_in_collection(conn: &Connection, collection_id: &str) -> Result<Vec<String>, MetadataStoreError>`
  - JOIN `collection_profiles` → `profiles` to return `current_filename` list
- `pub fn set_profile_favorite(conn: &Connection, profile_name: &str, favorite: bool) -> Result<(), MetadataStoreError>`
  - UPDATE `profiles SET is_favorite = ?1 WHERE current_filename = ?2`
  - No migration needed — column already exists from Phase 1 schema
- `pub fn list_favorite_profiles(conn: &Connection) -> Result<Vec<String>, MetadataStoreError>`
  - SELECT `current_filename` WHERE `is_favorite = 1 AND deleted_at IS NULL`

Pattern reference: `profile_sync.rs:72–86` for `lookup_profile_id` call shape; `launcher_sync.rs` for FK resolution pattern.

---

### Phase 3: MetadataStore API Surface (1 task)

Must wait for all three Phase 2 core module tasks to complete. One task modifies one file.

**Task P3-T6 — Add Phase 3 method wrappers to `metadata/mod.rs`**

Files touched: `metadata/mod.rs` (1 file)

Add submodule declarations:

```rust
mod cache_store;
mod collections;
mod community_index;
```

Update `pub use models::{...}` to export new Phase 3 public types: `CacheEntryStatus`, `MAX_CACHE_PAYLOAD_BYTES`, `CommunityProfileRow`, `CommunityTapRow`, `CollectionRow`, `CollectionSummary`, `FailureTrendRow`.

Add public `with_conn` / `with_conn_mut` delegate methods for all Phase 3 core functions:

```rust
// Community index (requires &mut Connection for transaction)
pub fn index_community_tap_result(&self, result: &CommunityTapSyncResult) -> Result<(), MetadataStoreError>
pub fn list_community_tap_profiles(&self, tap_url: Option<&str>) -> Result<Vec<CommunityProfileRow>, MetadataStoreError>

// Collections
pub fn list_collections(&self) -> Result<Vec<CollectionRow>, MetadataStoreError>
pub fn create_collection(&self, name: &str) -> Result<String, MetadataStoreError>
pub fn delete_collection(&self, collection_id: &str) -> Result<(), MetadataStoreError>
pub fn add_profile_to_collection(&self, collection_id: &str, profile_name: &str) -> Result<(), MetadataStoreError>
pub fn remove_profile_from_collection(&self, collection_id: &str, profile_name: &str) -> Result<(), MetadataStoreError>
pub fn list_profiles_in_collection(&self, collection_id: &str) -> Result<Vec<String>, MetadataStoreError>

// Favorites (writes to existing Phase 1 profiles.is_favorite column)
pub fn set_profile_favorite(&self, profile_name: &str, favorite: bool) -> Result<(), MetadataStoreError>
pub fn list_favorite_profiles(&self) -> Result<Vec<String>, MetadataStoreError>

// Cache
pub fn get_cache_entry(&self, source_url: &str, cache_key: &str) -> Result<Option<String>, MetadataStoreError>
pub fn put_cache_entry(&self, source_url: &str, cache_key: &str, payload: &str, expires_at: Option<&str>) -> Result<(), MetadataStoreError>
pub fn evict_expired_cache_entries(&self) -> Result<usize, MetadataStoreError>

// Usage insights (read-only SQL projections over Phase 2 launch_operations)
pub fn query_most_launched(&self, limit: usize) -> Result<Vec<(String, u64)>, MetadataStoreError>
pub fn query_last_success_per_profile(&self) -> Result<Vec<(String, String)>, MetadataStoreError>
pub fn query_failure_trends(&self, days: u32) -> Result<Vec<FailureTrendRow>, MetadataStoreError>
```

Note: usage insights methods delegate directly via `with_conn` to inline SQL (no separate module file needed — aggregate queries are simple enough to live as inline closures in `mod.rs`).

`index_community_tap_result` requires `with_conn_mut` (needs `&mut Connection` for `Transaction::new`). All others use `with_conn`.

---

### Phase 4: Tauri Integration (4 tasks — parallel)

All four tasks are unblocked simultaneously once Phase 3 (`mod.rs` wrappers, P3-T6) lands. They have no inter-dependencies.

**Task P3-T7 — Wire community tap index hook into `commands/community.rs`**

Files touched: `src-tauri/src/commands/community.rs` (1 file)

- Add `metadata_store: State<'_, MetadataStore>` parameter to `community_sync`
- After `tap_store.sync_many(&taps)` returns `results`, call `metadata_store.index_community_tap_result(result)` for each result in a warn-and-continue block
- Add `community_list_indexed_profiles` command: calls `metadata_store.list_community_tap_profiles(None)`, returns `Vec<CommunityProfileRow>`

Pattern reference: Option A from `research-integration.md` — inline, fail-soft. Mirrors `profile_save`/`profile_delete`/`profile_rename` metadata hook pattern.

**Task P3-T8 — Create `commands/collections.rs` with full collections CRUD**

Files touched: `src-tauri/src/commands/collections.rs` (new, 1 file)

Implement all collection commands following existing command signature shape (`State<'_, MetadataStore>`, `Result<T, String>`, `map_error`-equivalent):

- `collection_list` → `metadata_store.list_collections()`
- `collection_create(name: String)` → `metadata_store.create_collection(&name)`
- `collection_delete(collection_id: String)` → `metadata_store.delete_collection(&collection_id)`
- `collection_add_profile(collection_id: String, profile_name: String)`
- `collection_remove_profile(collection_id: String, profile_name: String)`
- `collection_list_profiles(collection_id: String)` → returns `Vec<String>` (profile names)

Note: this file needs its own `fn map_error` helper (copy from `community.rs:8-10`) or a shared helper from `commands/mod.rs` — check `commands/mod.rs` for whether a shared helper exists before duplicating.

**Task P3-T9 — Add `profile_set_favorite` command to `commands/profile.rs`**

Files touched: `src-tauri/src/commands/profile.rs` (1 file)

- Add `profile_set_favorite(name: String, favorite: bool, metadata_store: State<'_, MetadataStore>) -> Result<(), String>` command
- Add `profile_list_favorites(metadata_store: State<'_, MetadataStore>) -> Result<Vec<String>, String>` command
- These write to the existing `profiles.is_favorite` column (no schema change needed)

Pattern reference: existing metadata hooks in `commands/profile.rs` for the `State<'_, MetadataStore>` usage pattern.

**Task P3-T10 — Register all Phase 3 commands in `lib.rs`**

Files touched: `src-tauri/src/lib.rs` (1 file), `src-tauri/src/commands/mod.rs` (1 file)

- Add `pub mod collections;` to `commands/mod.rs`
- Add all new Phase 3 commands to the `invoke_handler!` macro in `lib.rs`:

  ```rust
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

- No new `.manage()` call needed — `MetadataStore` is already registered at `lib.rs:80`

---

### Phase 5: Testing (1 task)

Must wait for all Phase 4 tasks to complete (tests cover the full integrated stack).

**Task P3-T11 — Add Phase 3 unit and integration tests**

Files touched: `metadata/mod.rs` (test module), optionally inline per-module tests (1–4 files)

All tests use `MetadataStore::open_in_memory()` and the private `connection()` helper. Test patterns follow Phase 2 precedents exactly.

Required test cases:

**Community index tests:**

1. `test_index_tap_result_inserts_tap_and_profile_rows` — single tap result with 2 entries creates 1 `community_taps` row and 2 `community_profiles` rows
2. `test_index_tap_result_skips_on_unchanged_head` — second call with same `head_commit` does not modify rows (watermark skip)
3. `test_index_tap_result_replaces_stale_profiles` — second call with new `head_commit` and 1 entry replaces the 2-entry set (DELETE+INSERT verified by COUNT)
4. `test_index_tap_result_disabled_store_noop` — disabled store returns `Ok(())` without panic

**Cache store tests:** 5. `test_put_cache_entry_inserts_row` — basic put + get round-trip returns same payload 6. `test_put_cache_entry_idempotent` — two puts with same key do not duplicate rows 7. `test_cache_payload_oversized_stored_as_null` — payload over 512 KB stored as NULL; `payload_size` column stores original size 8. `test_evict_expired_entries_removes_expired` — entry with past `expires_at` is deleted; non-expired entry is retained 9. `test_cache_entry_disabled_store_noop` — disabled store returns `Ok(None)` from get

**Collections tests:** 10. `test_create_collection_returns_id` — `create_collection` returns a non-empty UUID 11. `test_add_profile_to_collection_succeeds` — requires a profile row; verifies `collection_profiles` COUNT = 1 12. `test_collection_delete_cascades_memberships` — deleting collection removes `collection_profiles` rows 13. `test_set_profile_favorite_toggles_column` — verify `is_favorite = 1` set, then verify `is_favorite = 0` unset 14. `test_list_favorite_profiles_excludes_deleted` — tombstoned profile not returned in favorites

**Usage insights tests:** 15. `test_query_most_launched_returns_top_profiles` — insert 3 `launch_operations` rows; verify ordering 16. `test_query_failure_trends_filters_by_days` — verify 30-day filter excludes old records

---

## Task Granularity Summary

| Task   | Files Touched                                                  | File Count |
| ------ | -------------------------------------------------------------- | ---------- |
| P3-T1  | `metadata/models.rs`                                           | 1          |
| P3-T2  | `metadata/migrations.rs`                                       | 1          |
| P3-T3  | `metadata/community_index.rs` (new)                            | 1          |
| P3-T4  | `metadata/cache_store.rs` (new)                                | 1          |
| P3-T5  | `metadata/collections.rs` (new)                                | 1          |
| P3-T6  | `metadata/mod.rs`                                              | 1          |
| P3-T7  | `src-tauri/src/commands/community.rs`                          | 1          |
| P3-T8  | `src-tauri/src/commands/collections.rs` (new)                  | 1          |
| P3-T9  | `src-tauri/src/commands/profile.rs`                            | 1          |
| P3-T10 | `src-tauri/src/lib.rs`, `src-tauri/src/commands/mod.rs`        | 2          |
| P3-T11 | `metadata/mod.rs` (test module) + optionally inline per-module | 1–4        |

All tasks respect the 1–3 files guideline. No task combines schema work with business logic work.

---

## Dependency Analysis

### Full DAG

```
P3-T1 (models.rs types + constants)
    └─→ P3-T3 (community_index.rs)  ─┐
    └─→ P3-T4 (cache_store.rs)      ─┤ all → P3-T6 (mod.rs wrappers)
    └─→ P3-T5 (collections.rs)      ─┘
                                            └─→ P3-T7 (community.rs hook)   ─┐
                                            └─→ P3-T8 (collections.rs cmd)  ─┤ all → P3-T10 (lib.rs) → P3-T11 (tests)
                                            └─→ P3-T9 (profile.rs cmd)      ─┘

P3-T2 (migrations.rs v3→v4)
    └─→ P3-T3 (community_index.rs — schema dep)
    └─→ P3-T4 (cache_store.rs — schema dep)
    └─→ P3-T5 (collections.rs — schema dep)
```

### Critical Path

```
P3-T1 → P3-T2 → P3-T3 → P3-T6 → P3-T7 → P3-T10 → P3-T11
```

Critical path depth: **7 sequential tasks** minimum. This path goes through `community_index.rs` because that module uses `with_conn_mut` (transaction-based re-index) and has the most complex implementation.

P3-T1 and P3-T2 can run in parallel (they are fully independent of each other), collapsing the first two steps to 1 batch wall-clock time.

### Parallelization Schedule

```
Batch 0:  P3-T1 ∥ P3-T2
          (models.rs and migrations.rs are fully independent)

Batch 1:  P3-T3 ∥ P3-T4 ∥ P3-T5
          (all three unblocked after Batch 0; no inter-dependencies)

Batch 2:  P3-T6
          (mod.rs — single task, waits for Batch 1 completion)

Batch 3:  P3-T7 ∥ P3-T8 ∥ P3-T9 ∥ P3-T10
          (P3-T10 can begin lib.rs/mod.rs registration as soon as P3-T6 lands;
          P3-T7/T8/T9 are independent of each other and of P3-T10)

Batch 4:  P3-T11
          (tests require all integration to be present)
```

Minimum wall-clock depth with full parallelism: **5 batches**.

---

## File-to-Task Mapping

### New Files to Create

| File                                                    | Task  | Description                                                         |
| ------------------------------------------------------- | ----- | ------------------------------------------------------------------- |
| `crates/crosshook-core/src/metadata/community_index.rs` | P3-T3 | Community tap indexing, HEAD watermark skip, DELETE+INSERT re-index |
| `crates/crosshook-core/src/metadata/cache_store.rs`     | P3-T4 | External metadata cache: get/put/evict with size bounds             |
| `crates/crosshook-core/src/metadata/collections.rs`     | P3-T5 | Collections CRUD + favorites write path                             |
| `src-tauri/src/commands/collections.rs`                 | P3-T8 | Tauri command handlers for all collection operations                |

### Files to Modify

| File                                               | Task   | Change                                                                                                 |
| -------------------------------------------------- | ------ | ------------------------------------------------------------------------------------------------------ |
| `crates/crosshook-core/src/metadata/models.rs`     | P3-T1  | Add `CacheEntryStatus`, `MAX_CACHE_PAYLOAD_BYTES`, row structs, `CollectionSummary`, `FailureTrendRow` |
| `crates/crosshook-core/src/metadata/migrations.rs` | P3-T2  | Add `migrate_3_to_4()` + `if version < 4` guard                                                        |
| `crates/crosshook-core/src/metadata/mod.rs`        | P3-T6  | Add submodule decls, `pub use` exports, all `with_conn` delegate methods                               |
| `src-tauri/src/commands/community.rs`              | P3-T7  | Add `State<MetadataStore>` to `community_sync`, add index hook, add `community_list_indexed_profiles`  |
| `src-tauri/src/commands/profile.rs`                | P3-T9  | Add `profile_set_favorite` and `profile_list_favorites` commands                                       |
| `src-tauri/src/commands/mod.rs`                    | P3-T10 | Add `pub mod collections;`                                                                             |
| `src-tauri/src/lib.rs`                             | P3-T10 | Register 9 new commands in `invoke_handler!`                                                           |
| `crates/crosshook-core/src/metadata/mod.rs`        | P3-T11 | Add `#[cfg(test)] mod tests` Phase 3 cases                                                             |

**Not touched in Phase 3:** `metadata/db.rs`, `metadata/profile_sync.rs`, `metadata/launcher_sync.rs`, `metadata/launch_history.rs`, `commands/export.rs`, `commands/launch.rs`, `startup.rs`, any frontend `.tsx`/`.ts` files (frontend integration is out of Phase 3 scope per task brief).

---

## Optimization Opportunities

### Parallel Group A: Foundation Tasks (Batch 0)

P3-T1 and P3-T2 write to entirely different files and have no compile-time dependency between them. A developer working on type additions in `models.rs` does not block another working on DDL in `migrations.rs`. In a two-agent workflow, assign T1 and T2 simultaneously as the first batch.

### Parallel Group B: Core Modules (Batch 1)

The largest parallelism opportunity in the phase. P3-T3, P3-T4, and P3-T5 are three completely independent new files. Each can be authored without reading or modifying the others. In a three-agent workflow this entire batch completes in the time of one module file. Key note: P3-T5 (`collections.rs`) calls into `profile_sync::lookup_profile_id` — this function already exists and is public; the dependency is on a Phase 1 artifact, not anything written in Phase 3.

### Parallel Group C: Tauri Integration (Batch 3)

P3-T7, P3-T8, P3-T9, and P3-T10 have no inter-dependencies. P3-T10 (`lib.rs` registration) can be partially drafted while P3-T7/T8/T9 are in progress since command names are known from the API design. Finalize P3-T10 after the three command files exist. In a four-agent workflow all of Batch 3 completes in parallel.

### Merge Order Recommendation

Merge in batch order to keep the branch always buildable:

1. Merge P3-T1 + P3-T2 together (both are additive, no existing code changes)
2. Merge P3-T3, P3-T4, P3-T5 together (three new files, no conflicts expected)
3. Merge P3-T6 (`mod.rs` wires it all together)
4. Merge P3-T7, P3-T8, P3-T9, P3-T10 together (Tauri layer additions)
5. Merge P3-T11 (tests validate the complete stack)

---

## Implementation Strategy Recommendations

### 1. No External Blockers — Start on Foundation Immediately

Unlike Phase 2 which had a `LaunchRequest.profile_name` external blocker, Phase 3 has no prerequisites outside the `metadata/` module. P3-T1 and P3-T2 can begin immediately after Phases 1 and 2 are confirmed merged to `main`.

### 2. `with_conn_mut` for `index_community_tap_result` — Match `observe_launcher_renamed`

The community re-index transaction requires `&mut Connection` for `Transaction::new`. Use `with_conn_mut` in the `mod.rs` wrapper for this method only. The pattern is already present at `mod.rs:186–196` in `observe_launcher_renamed`. All other Phase 3 methods use `with_conn`.

### 3. HEAD Watermark Check Must Compare String Equality — No SHA Validation

The `last_head_commit` field is a 40-char hex string from `git rev-parse HEAD`. The skip condition is simple string equality: `stored_head == incoming_head`. Do not validate SHA format or length — `CommunityTapSyncResult.head_commit` is trusted as-is (produced by `taps.rs` which already calls `rev_parse_head()`).

### 4. `community_profiles` vs. `community_taps` Re-index Atomicity

The `index_community_tap_result` transaction scope must cover: UPSERT `community_taps` row + DELETE + INSERT `community_profiles` batch. If the `community_taps` UPSERT succeeds but the `community_profiles` DELETE+INSERT fails, the watermark would advance past the stale profile rows. Keep all three operations inside a single `Transaction` — commit or rollback together.

### 5. Usage Insights — Inline SQL in `mod.rs`, No Separate Module

The three usage insights query methods (`query_most_launched`, `query_last_success_per_profile`, `query_failure_trends`) are pure SQL projections over the existing `launch_operations` table. They require no new schema and no new module file. Implement them directly as inline `with_conn` closures in `mod.rs`. The queries are too simple to warrant a `launch_insights.rs` module.

### 6. Collections CRUD Must Use `lookup_profile_id` — Not a Direct JOIN

The `add_profile_to_collection` and `remove_profile_from_collection` functions receive a `profile_name: &str` string from the Tauri layer. They must call `profile_sync::lookup_profile_id(conn, profile_name)` to resolve to `profile_id` before the `collection_profiles` write. Duplicating the lookup query would create a divergence risk if the `profiles` table query ever changes.

### 7. `collections.rs` Command File — `map_error` Helper Pattern

The new `src-tauri/src/commands/collections.rs` file needs a `map_error` converter. Check whether `commands/shared.rs` or `commands/mod.rs` already exposes a shared helper before adding a third copy. If not shared, add a local private `fn map_error(e: impl ToString) -> String { e.to_string() }` following `community.rs:8-10`. Do not add a public shared utility speculatively — only if two files need it.

### 8. A6 Length Bounds — Enforce in Core Module, Not Command Layer

String length validation for `game_name ≤ 512B`, `description ≤ 4KB`, `platform_tags ≤ 2KB`, `trainer_name/author ≤ 512B` belongs in `community_index.rs`, not in the `community_sync` command. The command layer passes `CommunityTapSyncResult` through; the metadata layer enforces bounds before INSERT. Follow the same principle as W3 diagnostic truncation in Phase 2 (`launch_history.rs` enforces 4KB, not `commands/launch.rs`).

### 9. Tests for Disabled Store — Use the Existing Phase 2 Test as Template

`metadata/mod.rs` already has `test_phase2_disabled_store_noop`. Add `test_phase3_disabled_store_noop` as a single test that calls every Phase 3 method on a `MetadataStore::disabled()` instance and asserts `Ok(...)` with appropriate defaults. This is the fastest safety net for the fail-soft delegation paths.

### 10. FTS5 — Leave Infrastructure Notes but Do Not Implement

If there is temptation to add FTS5 during P3-T2 (migration) "while we're in there," resist it. The FTS5 content table sync requirement (explicit sync after every DELETE+INSERT to `community_profiles`) significantly increases implementation complexity of P3-T3. Deferring FTS5 keeps the DELETE+INSERT transaction in `community_index.rs` clean and avoids the content-table rebuild surface. Add a `// TODO(fts5): community_profiles_fts virtual table — deferred` comment in `migrate_3_to_4` as a marker.

---

## Dependency Matrix (Phase 3 Tasks)

| Task   | Depends On                                    | Blocks                      |
| ------ | --------------------------------------------- | --------------------------- |
| P3-T1  | (none — Phase 2 merged)                       | P3-T3, P3-T4, P3-T5         |
| P3-T2  | (none — Phase 2 merged)                       | P3-T3, P3-T4, P3-T5         |
| P3-T3  | P3-T1, P3-T2                                  | P3-T6                       |
| P3-T4  | P3-T1, P3-T2                                  | P3-T6                       |
| P3-T5  | P3-T1, P3-T2                                  | P3-T6                       |
| P3-T6  | P3-T3, P3-T4, P3-T5                           | P3-T7, P3-T8, P3-T9, P3-T10 |
| P3-T7  | P3-T6                                         | P3-T11                      |
| P3-T8  | P3-T6                                         | P3-T10, P3-T11              |
| P3-T9  | P3-T6                                         | P3-T10, P3-T11              |
| P3-T10 | P3-T6 (+ P3-T8, P3-T9 for final registration) | P3-T11                      |
| P3-T11 | P3-T7, P3-T8, P3-T9, P3-T10                   | (final)                     |

---

## Must-Read Documents (Per Task)

| Task   | Must Read Before Starting                                                                                                                                                                                                                                                                                                        |
| ------ | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| P3-T1  | `shared.md` (models section, design decisions table), `research-integration.md` (CollectionSummary, FailureTrendRow shapes), `research-patterns.md` (enum with `as_str()` pattern)                                                                                                                                               |
| P3-T2  | `shared.md` (Phase 3 relevant tables section — locked DDL), `migrations.rs` (sequential runner pattern), `research-integration.md` (Phase 3 migration DDL section)                                                                                                                                                               |
| P3-T3  | `shared.md` (design decisions: DELETE+INSERT, tap_branch NOT NULL DEFAULT '', watermark source), `research-patterns.md` (UPSERT pattern, DELETE+INSERT gotcha, NULL uniqueness gotcha), `research-integration.md` (HEAD watermark skip logic, `index_community_tap_result` pseudocode), `launcher_sync.rs` (Transaction pattern) |
| P3-T4  | `shared.md` (cache payload bound, external cache table), `research-patterns.md` (size-bounded JSON storage pattern), `research-integration.md` (external cache API section), `launch_history.rs` (existing size-bound implementation)                                                                                            |
| P3-T5  | `shared.md` (collections scope, `lookup_profile_id` reuse), `research-integration.md` (collections IPC shapes), `research-patterns.md` (`lookup_profile_id` reuse section), `profile_sync.rs:72–86`                                                                                                                              |
| P3-T6  | `metadata/mod.rs` (with_conn pattern, existing delegates), `shared.md` (API method signatures), `research-integration.md` (MetadataStore API extensions section)                                                                                                                                                                 |
| P3-T7  | `commands/community.rs` (current command signatures, `map_error`), `research-integration.md` (community sync integration point, Option A recommendation), `shared.md` (warn-and-continue pattern)                                                                                                                                |
| P3-T8  | `commands/community.rs` (map_error pattern), `research-integration.md` (collections IPC command shapes), `research-patterns.md` (Tauri command patterns section)                                                                                                                                                                 |
| P3-T9  | `commands/profile.rs` (existing metadata hook pattern), `research-integration.md` (profile_set_favorite command shape)                                                                                                                                                                                                           |
| P3-T10 | `src-tauri/src/lib.rs` (invoke_handler! macro), `src-tauri/src/commands/mod.rs`, `research-integration.md` (new commands to register section)                                                                                                                                                                                    |
| P3-T11 | All Phase 3 implementation files, `research-docs.md` (success criteria, A6 security findings), `research-patterns.md` (test patterns section), Phase 2 test cases in `mod.rs` as structural template                                                                                                                             |
