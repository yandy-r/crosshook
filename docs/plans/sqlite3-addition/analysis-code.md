# Code Analysis: SQLite Metadata Layer Phase 3 — Catalog and Intelligence

## Executive Summary

Phases 1-2 established the `MetadataStore` with `Arc<Mutex<Connection>>`, `with_conn`/`with_conn_mut` fail-soft delegation, free-function submodules (`profile_sync.rs`, `launcher_sync.rs`, `launch_history.rs`), and sequential migration up to v3. Phase 3 adds five new tables (schema v4) with two new submodule files (`community_index.rs`, `cache_store.rs`), integrates HEAD-commit watermarking into existing `community_sync`, adds collections/favorites as new Tauri commands, and exposes usage insights via SQL aggregates over the existing `launch_operations` table. Every pattern in this document is extracted directly from live source with exact file paths and line numbers.

---

## Existing Code Structure

### Metadata Module Files (all at `src/crosshook-native/crates/crosshook-core/src/metadata/`)

| File                | Role                                                                                                       |
| ------------------- | ---------------------------------------------------------------------------------------------------------- |
| `mod.rs`            | `MetadataStore` struct, `with_conn`/`with_conn_mut`, all public methods                                    |
| `db.rs`             | `open_at_path`, `open_in_memory`, `new_id()` (UUID v4), `configure_connection`                             |
| `migrations.rs`     | Sequential migration runner, `migrate_0_to_1` through `migrate_2_to_3`                                     |
| `models.rs`         | Error types, enums (`SyncSource`, `LaunchOutcome`, `DriftState`), row structs, `MAX_DIAGNOSTIC_JSON_BYTES` |
| `profile_sync.rs`   | Profile CRUD sync, `lookup_profile_id` reusable bridge                                                     |
| `launcher_sync.rs`  | Launcher export/delete/rename with `with_conn_mut` and transaction                                         |
| `launch_history.rs` | Launch start/finish/sweep with size-bounded JSON storage                                                   |

### Community Module Files (all at `src/crosshook-native/crates/crosshook-core/src/community/`)

| File       | Role                                                                                    |
| ---------- | --------------------------------------------------------------------------------------- |
| `mod.rs`   | Re-exports from `profile/community_schema.rs` and submodules                            |
| `taps.rs`  | `CommunityTapStore`, `CommunityTapSyncResult` with `head_commit: String` at line 44     |
| `index.rs` | `CommunityProfileIndex`, `CommunityProfileIndexEntry`, `index_taps()` and `index_tap()` |

### Tauri Command Files (all at `src/crosshook-native/src-tauri/src/commands/`)

| File           | Role                                                                                  |
| -------------- | ------------------------------------------------------------------------------------- |
| `community.rs` | All community commands; `map_error` helper at line 8-10; `community_sync` at line 124 |
| `export.rs`    | Warn-and-continue metadata hook pattern; all metadata-integrated export commands      |
| `profile.rs`   | Profile CRUD commands; existing metadata hooks after save/rename/delete               |
| `lib.rs`       | `.manage()` registrations at lines 76-81; `invoke_handler!` list at lines 85-128      |
| `startup.rs`   | `run_metadata_reconciliation` — Phase 3 may add community index orphan sweep          |

---

## Implementation Patterns

### Pattern 1: `with_conn` Delegation Shape

**Source**: `src/crosshook-native/crates/crosshook-core/src/metadata/mod.rs:59-95`

The exact shape every Phase 3 `MetadataStore` method must replicate. `with_conn` is for `&Connection`; `with_conn_mut` is for `&mut Connection` (needed for `Transaction::new`).

```rust
// with_conn — read-only or single-statement writes
fn with_conn<F, T>(&self, action: &'static str, f: F) -> Result<T, MetadataStoreError>
where
    F: FnOnce(&Connection) -> Result<T, MetadataStoreError>,
    T: Default,
{
    if !self.available {
        return Ok(T::default());
    }
    let Some(conn) = &self.conn else {
        return Ok(T::default());
    };
    let guard = conn.lock().map_err(|_| {
        MetadataStoreError::Corrupt(format!("metadata store mutex poisoned while {action}"))
    })?;
    f(&guard)
}

// with_conn_mut — required when passing conn to Transaction::new
fn with_conn_mut<F, T>(&self, action: &'static str, f: F) -> Result<T, MetadataStoreError>
where
    F: FnOnce(&mut Connection) -> Result<T, MetadataStoreError>,
    T: Default,
{
    if !self.available {
        return Ok(T::default());
    }
    let Some(conn) = &self.conn else {
        return Ok(T::default());
    };
    let mut guard = conn.lock().map_err(|_| {
        MetadataStoreError::Corrupt(format!("metadata store mutex poisoned while {action}"))
    })?;
    f(&mut guard)
}
```

**Phase 3 application**: `sync_community_tap` and `set_collection_profiles` use `with_conn_mut`; all other Phase 3 methods use `with_conn`.

**Example delegate method shape** (from `mod.rs:97-115`):

```rust
pub fn observe_profile_write(
    &self,
    name: &str,
    profile: &GameProfile,
    path: &Path,
    source: SyncSource,
    source_profile_id: Option<&str>,
) -> Result<(), MetadataStoreError> {
    self.with_conn("observe a profile write", |conn| {
        profile_sync::observe_profile_write(conn, name, profile, path, source, source_profile_id)
    })
}
```

The string literal passed to `with_conn` appears verbatim in the mutex-poison error message — use a descriptive verb phrase, e.g., `"sync a community tap index"`.

---

### Pattern 2: Free Function Module Shape

**Source**: `src/crosshook-native/crates/crosshook-core/src/metadata/launch_history.rs:1-7` (imports); `profile_sync.rs:1-9`

Every submodule begins with `super::` imports for shared types, then defines free functions with `conn: &Connection` (or `conn: &mut Connection`) as the first argument:

```rust
// community_index.rs — header shape
use super::{db, MetadataStoreError};
use super::profile_sync::lookup_profile_id;
use crate::community::taps::{CommunityTapSubscription, CommunityTapSyncResult};
use crate::community::index::CommunityProfileIndexEntry;
use chrono::Utc;
use rusqlite::{params, Connection, Transaction, TransactionBehavior};

pub fn sync_community_tap(
    conn: &mut Connection,
    subscription: &CommunityTapSubscription,
    result: &CommunityTapSyncResult,
) -> Result<(), MetadataStoreError> { ... }
```

```rust
// cache_store.rs — header shape
use super::{db, MetadataStoreError};
use super::models::MAX_CACHE_PAYLOAD_BYTES;
use chrono::Utc;
use rusqlite::{params, Connection};

pub fn upsert_cache_entry(
    conn: &Connection,
    source_url: &str,
    cache_key: &str,
    payload: Option<&str>,
    expires_at: Option<&str>,
) -> Result<(), MetadataStoreError> { ... }
```

**Declaring new modules**: Add `mod community_index;` and `mod cache_store;` to `mod.rs:1-6` alongside existing module declarations.

---

### Pattern 3: Migration Shape (adding `migrate_3_to_4`)

**Source**: `src/crosshook-native/crates/crosshook-core/src/metadata/migrations.rs:1-137`

The `run_migrations` function uses sequential `if version < N` guards. Add the `if version < 4` block after the existing `if version < 3` block at line 30-37:

```rust
// In run_migrations, after line 37:
if version < 4 {
    migrate_3_to_4(conn)?;
    conn.pragma_update(None, "user_version", 4_u32)
        .map_err(|source| MetadataStoreError::Database {
            action: "update metadata schema version",
            source,
        })?;
}
```

The `migrate_3_to_4` function uses `execute_batch` with literal-only DDL (no runtime-interpolated strings):

```rust
fn migrate_3_to_4(conn: &Connection) -> Result<(), MetadataStoreError> {
    conn.execute_batch("
        CREATE TABLE IF NOT EXISTS community_taps (
            tap_id          TEXT PRIMARY KEY,
            tap_url         TEXT NOT NULL,
            tap_branch      TEXT NOT NULL DEFAULT '',
            local_path      TEXT NOT NULL,
            last_head_commit TEXT,
            profile_count   INTEGER NOT NULL DEFAULT 0,
            last_indexed_at TEXT,
            created_at      TEXT NOT NULL,
            updated_at      TEXT NOT NULL
        );
        CREATE UNIQUE INDEX IF NOT EXISTS idx_community_taps_url_branch
            ON community_taps(tap_url, tap_branch);

        CREATE TABLE IF NOT EXISTS community_profiles (
            id              INTEGER PRIMARY KEY AUTOINCREMENT,
            tap_id          TEXT NOT NULL REFERENCES community_taps(tap_id),
            relative_path   TEXT NOT NULL,
            manifest_path   TEXT NOT NULL,
            game_name       TEXT,
            game_version    TEXT,
            trainer_name    TEXT,
            trainer_version TEXT,
            proton_version  TEXT,
            compatibility_rating TEXT,
            author          TEXT,
            description     TEXT,
            platform_tags_json TEXT,
            schema_version  INTEGER NOT NULL DEFAULT 1,
            created_at      TEXT NOT NULL
        );
        CREATE UNIQUE INDEX IF NOT EXISTS idx_community_profiles_tap_path
            ON community_profiles(tap_id, relative_path);

        CREATE TABLE IF NOT EXISTS external_cache_entries (
            cache_id        TEXT PRIMARY KEY,
            source_url      TEXT NOT NULL,
            cache_key       TEXT NOT NULL UNIQUE,
            payload_json    TEXT,
            payload_size    INTEGER NOT NULL DEFAULT 0,
            fetched_at      TEXT NOT NULL,
            expires_at      TEXT,
            created_at      TEXT NOT NULL,
            updated_at      TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS collections (
            collection_id   TEXT PRIMARY KEY,
            name            TEXT NOT NULL UNIQUE,
            description     TEXT,
            created_at      TEXT NOT NULL,
            updated_at      TEXT NOT NULL
        );

        CREATE TABLE IF NOT EXISTS collection_profiles (
            collection_id   TEXT NOT NULL REFERENCES collections(collection_id) ON DELETE CASCADE,
            profile_id      TEXT NOT NULL REFERENCES profiles(profile_id),
            added_at        TEXT NOT NULL,
            PRIMARY KEY (collection_id, profile_id)
        );
    ")
    .map_err(|source| MetadataStoreError::Database {
        action: "run metadata migration 3 to 4",
        source,
    })?;

    Ok(())
}
```

**Critical**: `execute_batch` must only contain literal DDL. Never interpolate runtime values into migration SQL. All five Phase 3 tables go in a single `migrate_3_to_4` per the locked design decision.

---

### Pattern 4: DELETE+INSERT Transaction Shape (community profile re-index)

**Source**: `src/crosshook-native/crates/crosshook-core/src/metadata/launcher_sync.rs:68-141`

`observe_launcher_renamed` demonstrates `Transaction::new(conn, TransactionBehavior::Immediate)` — this is the exact template for community profile re-index. Note that `Transaction::new` consumes `&mut Connection`, which is why the delegate method uses `with_conn_mut`.

```rust
// In community_index.rs — sync_community_tap

pub fn sync_community_tap(
    conn: &mut Connection,
    subscription: &CommunityTapSubscription,
    result: &CommunityTapSyncResult,
) -> Result<(), MetadataStoreError> {
    let tap_url = &result.workspace.subscription.url;
    // tap_branch stored as empty string (not NULL) — see design decision
    let tap_branch = result.workspace.subscription.branch.as_deref().unwrap_or("");
    let now = Utc::now().to_rfc3339();

    // 1. Upsert community_taps watermark row
    conn.execute(
        "INSERT INTO community_taps (
            tap_id, tap_url, tap_branch, local_path,
            last_head_commit, profile_count, last_indexed_at,
            created_at, updated_at
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
        ON CONFLICT(tap_url, tap_branch) DO UPDATE SET
            local_path = excluded.local_path,
            last_head_commit = excluded.last_head_commit,
            profile_count = excluded.profile_count,
            last_indexed_at = excluded.last_indexed_at,
            updated_at = excluded.updated_at",
        params![
            db::new_id(),
            tap_url,
            tap_branch,
            result.workspace.local_path.to_string_lossy(),
            &result.head_commit,
            result.index.entries.len() as i64,
            &now,
            &now,
            &now,
        ],
    )
    .map_err(|source| MetadataStoreError::Database {
        action: "upsert a community tap watermark row",
        source,
    })?;

    // 2. Resolve tap_id for FK
    let tap_id: String = conn
        .query_row(
            "SELECT tap_id FROM community_taps WHERE tap_url = ?1 AND tap_branch = ?2",
            params![tap_url, tap_branch],
            |row| row.get(0),
        )
        .map_err(|source| MetadataStoreError::Database {
            action: "look up tap_id for community profile re-index",
            source,
        })?;

    // 3. Transactional DELETE+INSERT — eliminates stale ghost entries
    let tx = Transaction::new(conn, TransactionBehavior::Immediate)
        .map_err(|source| MetadataStoreError::Database {
            action: "start a community profile re-index transaction",
            source,
        })?;

    tx.execute(
        "DELETE FROM community_profiles WHERE tap_id = ?1",
        params![tap_id],
    )
    .map_err(|source| MetadataStoreError::Database {
        action: "delete stale community profile rows before re-index",
        source,
    })?;

    for entry in &result.index.entries {
        insert_community_profile(&tx, &tap_id, entry, &now)?;
    }

    tx.commit().map_err(|source| MetadataStoreError::Database {
        action: "commit the community profile re-index transaction",
        source,
    })?;

    Ok(())
}
```

**Why DELETE+INSERT, not UPSERT**: When profiles are removed from a tap repo, UPSERT leaves stale rows for removed manifests. DELETE+INSERT is the canonical choice here.

**tap_branch empty-string invariant**: Always store `branch.as_deref().unwrap_or("")` — never NULL. The UNIQUE index is on `(tap_url, tap_branch)`. SQLite `NULL != NULL` in unique indexes would allow duplicate rows if branch were stored as NULL.

---

### Pattern 5: Size-Bounded JSON Storage

**Source**: `src/crosshook-native/crates/crosshook-core/src/metadata/launch_history.rs:65-82`

Payloads over the size limit are stored as NULL, not truncated:

```rust
// In launch_history.rs:65-68
let json = serde_json::to_string(report).ok();
let json = json.filter(|s| s.len() <= MAX_DIAGNOSTIC_JSON_BYTES);
```

Phase 3 adds `MAX_CACHE_PAYLOAD_BYTES = 512_000` in `models.rs` for external cache. The pattern in `cache_store.rs`:

```rust
// In cache_store.rs
use super::models::MAX_CACHE_PAYLOAD_BYTES;

pub fn upsert_cache_entry(
    conn: &Connection,
    source_url: &str,
    cache_key: &str,
    payload_json: Option<&str>,
    expires_at: Option<&str>,
) -> Result<(), MetadataStoreError> {
    let bounded_payload = payload_json.filter(|s| s.len() <= MAX_CACHE_PAYLOAD_BYTES);
    let payload_size = bounded_payload.map(|s| s.len() as i64).unwrap_or(0);
    let now = Utc::now().to_rfc3339();

    conn.execute(
        "INSERT INTO external_cache_entries (
            cache_id, source_url, cache_key,
            payload_json, payload_size, fetched_at, expires_at,
            created_at, updated_at
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
        ON CONFLICT(cache_key) DO UPDATE SET
            source_url = excluded.source_url,
            payload_json = excluded.payload_json,
            payload_size = excluded.payload_size,
            fetched_at = excluded.fetched_at,
            expires_at = excluded.expires_at,
            updated_at = excluded.updated_at",
        params![
            db::new_id(),
            source_url,
            cache_key,
            bounded_payload,
            payload_size,
            &now,
            expires_at,
            &now,
            &now,
        ],
    )
    .map_err(|source| MetadataStoreError::Database {
        action: "upsert an external cache entry",
        source,
    })?;

    Ok(())
}
```

---

### Pattern 6: Warn-and-Continue Hook Shape

**Source**: `src/crosshook-native/src-tauri/src/commands/export.rs:26-38` and `80-84` and `101-105`

Every metadata hook in Tauri commands follows this exact shape — metadata failures never block the primary operation:

```rust
// export.rs:26-38 — after primary operation succeeds
if let Err(e) = metadata_store.observe_launcher_exported(
    request.profile_name.as_deref(),
    &result.launcher_slug,
    &result.display_name,
    &result.script_path,
    &result.desktop_entry_path,
) {
    tracing::warn!(%e, launcher_slug = %result.launcher_slug, "metadata sync after export_launchers failed");
}

Ok(result)
```

Phase 3 applies this pattern in `community.rs` (after `sync_many`) and in a new `profile.rs` command (`profile_set_favorite`):

```rust
// In community_sync — Phase 3 addition
#[tauri::command]
pub fn community_sync(
    settings_store: State<'_, SettingsStore>,
    tap_store: State<'_, CommunityTapStore>,
    metadata_store: State<'_, MetadataStore>,
) -> Result<Vec<CommunityTapSyncResult>, String> {
    let taps = load_community_taps(&settings_store)?;
    let results = tap_store.sync_many(&taps).map_err(map_error)?;

    for result in &results {
        if let Err(e) = metadata_store.sync_community_tap(
            &result.workspace.subscription,
            result,
        ) {
            tracing::warn!(
                %e,
                tap_url = %result.workspace.subscription.url,
                "metadata sync after community_sync failed"
            );
        }
    }

    Ok(results)
}
```

Note: `community_sync` signature gains `metadata_store: State<'_, MetadataStore>` — the Tauri state injection order does not matter for compilation but the test in `commands/community.rs:138-161` must be updated to match the new signature.

---

### Pattern 7: Enum with `as_str()` Shape

**Source**: `src/crosshook-native/crates/crosshook-core/src/metadata/models.rs:69-137`

If Phase 3 adds a `CacheEntryStatus` enum:

```rust
/// Maps to the `external_cache_entries` status if a status column is added (optional).
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CacheEntryStatus {
    Fresh,
    Expired,
    Oversized,
}

impl CacheEntryStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Fresh => "fresh",
            Self::Expired => "expired",
            Self::Oversized => "oversized",
        }
    }
}
```

The current schema design does not include a status column on `external_cache_entries`; this enum is only needed if that decision changes. The existing `CompatibilityRating` enum in `community_schema.rs:11-20` stores as serialized JSON strings via serde — for SQL storage, use `as_str()` pattern from `models.rs`, not serde.

---

### Pattern 8: `lookup_profile_id` Reuse

**Source**: `src/crosshook-native/crates/crosshook-core/src/metadata/profile_sync.rs:72-86`

The function signature is `pub fn lookup_profile_id(conn: &Connection, name: &str) -> Result<Option<String>, MetadataStoreError>`. It is already `pub` and re-exported to other submodules via `use super::profile_sync::lookup_profile_id` (see `launcher_sync.rs:1` and `launch_history.rs:3`).

For collection membership and `profile_set_favorite`, import the same way:

```rust
// In community_index.rs (if favorite flag cross-reference is needed)
use super::profile_sync::lookup_profile_id;

// Usage
let profile_id = lookup_profile_id(conn, profile_name)?;
```

Do NOT duplicate the `SELECT profile_id FROM profiles WHERE current_filename = ?1 AND deleted_at IS NULL` query.

---

### Pattern 9: Test Shape

**Source**: `src/crosshook-native/crates/crosshook-core/src/metadata/mod.rs:229-290`

All metadata tests use `MetadataStore::open_in_memory()` and a `connection(&store)` helper to get raw SQL access for assertions:

```rust
fn connection(store: &MetadataStore) -> std::sync::MutexGuard<'_, Connection> {
    store
        .conn
        .as_ref()
        .expect("metadata store should expose a connection in tests")
        .lock()
        .expect("metadata store mutex should not be poisoned")
}
```

Test pattern for Phase 3:

```rust
#[test]
fn test_sync_community_tap_replaces_stale_entries() {
    let store = MetadataStore::open_in_memory().unwrap();

    // First sync — 2 entries
    let subscription = CommunityTapSubscription { url: "https://example.invalid/tap.git".to_string(), branch: None };
    let result1 = make_tap_sync_result(&subscription, "abc123", 2);
    store.sync_community_tap(&subscription, &result1).unwrap();

    let conn = connection(&store);
    let count1: i64 = conn.query_row(
        "SELECT COUNT(*) FROM community_profiles",
        [],
        |row| row.get(0),
    ).unwrap();
    assert_eq!(count1, 2);
    drop(conn);

    // Second sync — 1 entry (one was removed from tap)
    let result2 = make_tap_sync_result(&subscription, "def456", 1);
    store.sync_community_tap(&subscription, &result2).unwrap();

    let conn = connection(&store);
    let count2: i64 = conn.query_row(
        "SELECT COUNT(*) FROM community_profiles",
        [],
        |row| row.get(0),
    ).unwrap();
    assert_eq!(count2, 1, "stale entry must be removed by DELETE+INSERT re-index");
}
```

The `drop(conn)` before re-borrowing is necessary because `MutexGuard` holds the lock — two `connection()` calls in the same scope will deadlock.

---

### Pattern 10: `map_error` in Community Commands

**Source**: `src/crosshook-native/src-tauri/src/commands/community.rs:8-10`

```rust
fn map_error(error: impl ToString) -> String {
    error.to_string()
}
```

This private helper converts any `Display` error to `String`. All Phase 3 community command errors use `.map_err(map_error)`. New collection commands in `collections.rs` must define their own identical `map_error` (not re-exported from `community.rs`).

---

### Pattern 11: `profile_sync.rs` UPSERT Shape (for favorites)

**Source**: `src/crosshook-native/crates/crosshook-core/src/metadata/profile_sync.rs:28-63`

The `profiles` table already has `is_favorite INTEGER NOT NULL DEFAULT 0` and `is_pinned INTEGER NOT NULL DEFAULT 0` (from `migrations.rs:52-53`). A new `set_profile_favorite` free function in `profile_sync.rs` follows the minimal-UPDATE shape:

```rust
pub fn set_profile_favorite(
    conn: &Connection,
    name: &str,
    is_favorite: bool,
    is_pinned: bool,
) -> Result<(), MetadataStoreError> {
    let rows_affected = conn
        .execute(
            "UPDATE profiles SET is_favorite = ?1, is_pinned = ?2, updated_at = ?3
             WHERE current_filename = ?4 AND deleted_at IS NULL",
            params![
                is_favorite as i64,
                is_pinned as i64,
                Utc::now().to_rfc3339(),
                name,
            ],
        )
        .map_err(|source| MetadataStoreError::Database {
            action: "set profile favorite flags",
            source,
        })?;

    if rows_affected == 0 {
        tracing::warn!(
            profile_name = %name,
            "set_profile_favorite found no matching active profile row — skipping update"
        );
    }

    Ok(())
}
```

The zero-rows-affected warn pattern comes from `launch_history.rs:111-116`.

---

## Integration Points

### Files to Create

| File                                                                         | What it contains                                                                                                                                                                                                             |
| ---------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `src/crosshook-native/crates/crosshook-core/src/metadata/community_index.rs` | `sync_community_tap`, `lookup_community_tap_head`, helper `insert_community_profile`, `query_community_profiles`                                                                                                             |
| `src/crosshook-native/crates/crosshook-core/src/metadata/cache_store.rs`     | `upsert_cache_entry`, `lookup_cache_entry`, `evict_expired_entries`                                                                                                                                                          |
| `src/crosshook-native/src-tauri/src/commands/collections.rs`                 | Collection CRUD commands: `collection_create`, `collection_delete`, `collection_add_profile`, `collection_remove_profile`, `collection_list`, `collection_list_profiles`; also `usage_top_profiles`, `usage_recent_launches` |

### Files to Modify

| File                                                                      | What changes                                                                                                                                                                      |
| ------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `src/crosshook-native/crates/crosshook-core/src/metadata/mod.rs`          | Add `mod community_index; mod cache_store;` declarations; add Phase 3 public delegate methods; add new re-exports to `pub use models::...`                                        |
| `src/crosshook-native/crates/crosshook-core/src/metadata/migrations.rs`   | Add `if version < 4` block calling `migrate_3_to_4`                                                                                                                               |
| `src/crosshook-native/crates/crosshook-core/src/metadata/models.rs`       | Add `MAX_CACHE_PAYLOAD_BYTES = 512_000`; add new row structs (`CommunityTapRow`, `CommunityProfileRow`, `CacheEntryRow`, `CollectionRow`); optionally add `CacheEntryStatus` enum |
| `src/crosshook-native/crates/crosshook-core/src/metadata/profile_sync.rs` | Add `set_profile_favorite` function                                                                                                                                               |
| `src/crosshook-native/src-tauri/src/commands/community.rs`                | Add `metadata_store: State<'_, MetadataStore>` to `community_sync`; add warn-and-continue hook loop after `sync_many`; update test type-alias at line 138-161                     |
| `src/crosshook-native/src-tauri/src/commands/profile.rs`                  | Add `profile_set_favorite` command with warn-and-continue metadata hook                                                                                                           |
| `src/crosshook-native/src-tauri/src/lib.rs`                               | Add `mod commands::collections;` (if separate file); register new commands in `invoke_handler!`                                                                                   |
| `src/crosshook-native/src-tauri/src/startup.rs`                           | Optionally add community tap orphan sweep to `run_metadata_reconciliation`                                                                                                        |

---

## Code Conventions

### Naming

- Metadata module free functions: verb phrase matching the `with_conn` action string — `sync_community_tap`, `upsert_cache_entry`, `create_collection`, `set_profile_favorite`
- `MetadataStore` public methods: same verb phrase — `sync_community_tap`, `upsert_cache_entry`
- Tauri command names: domain-prefixed snake_case — `community_sync` (existing), `collection_create`, `collection_list`, `usage_top_profiles`, `profile_set_favorite`
- Row struct names: `CommunityTapRow`, `CommunityProfileRow`, `CacheEntryRow`, `CollectionRow` — `pub(crate)` visibility, `#[allow(dead_code)]` (pattern from `models.rs:152-197`)

### Imports

- Every metadata submodule imports from `super::` for shared types: `use super::{db, MetadataStoreError};`
- `lookup_profile_id` re-imported as `use super::profile_sync::lookup_profile_id;`
- `chrono::Utc` for timestamps: `use chrono::Utc;`
- `rusqlite::params` always imported alongside `Connection`: `use rusqlite::{params, Connection};`
- Transaction pattern adds: `use rusqlite::{params, Connection, Transaction, TransactionBehavior};`
- `OptionalExtension` for `.optional()` on single-row queries: `use rusqlite::OptionalExtension;`

### Error Mapping

All SQL errors mapped with the `MetadataStoreError::Database { action: "...", source }` pattern — the `action` string is a static human-readable description that appears in error messages. Never use the `From<SqlError>` blanket impl directly in submodule code; always use the structured variant.

### Timestamp Format

All timestamps are RFC 3339 strings: `Utc::now().to_rfc3339()`. Never store Unix timestamps as integers in this schema.

### `pub use` re-exports in `mod.rs`

New public types from `models.rs` that cross the `MetadataStore` API boundary are re-exported at `mod.rs:8`:

```rust
// Current: pub use models::{DriftState, LaunchOutcome, MAX_DIAGNOSTIC_JSON_BYTES, MetadataStoreError, SyncReport, SyncSource};
// Phase 3 adds (as needed by callers):
pub use models::{..., MAX_CACHE_PAYLOAD_BYTES};
```

---

## Gotchas and Warnings

1. **`tap_branch` must never be NULL**. The UNIQUE index is on `(tap_url, tap_branch)`. SQLite treats `NULL != NULL` in unique constraint checks, so two rows with `NULL` branch would not conflict. Always store `branch.as_deref().unwrap_or("")`. This is a locked design decision.

2. **`with_conn_mut` vs `with_conn`**. `Transaction::new(conn, ...)` takes `&mut Connection` — the borrow checker will reject `&Connection`. Any Phase 3 method that starts a transaction must use `with_conn_mut`. Methods that only call `conn.execute()` or `conn.query_row()` use `with_conn`.

3. **`Transaction::new` vs `Transaction::new_unchecked`**. `launcher_sync.rs` uses `Transaction::new` (for `&mut Connection`). `profile_sync.rs` uses `Transaction::new_unchecked` (for `&Connection` via interior mutability). Community re-index goes through `with_conn_mut` so must use `Transaction::new`.

4. **`connection()` test helper holds the mutex**. If you call `connection(&store)` in a test, the `MutexGuard` holds the lock. Calling any `store.*` method while the guard is still alive will deadlock. Always `drop(conn)` or let it go out of scope before calling another store method. This is documented by the existing `test_diagnostic_json_truncated_at_4kb` test pattern (`mod.rs:697-707`).

5. **`community_sync` test must be updated**. The test at `community.rs:138-161` type-aliases the function signature. Adding `metadata_store: State<'_, MetadataStore>` to `community_sync` will break this compile-time contract check. Update the test type-alias immediately after modifying the command signature.

6. **A6 string length bounds**. Security review mandates rejecting oversized strings at the `MetadataStore` API boundary, not silently truncating: `game_name`/`trainer_name`/`author` ≤ 512 bytes, `description` ≤ 4096 bytes, `platform_tags` ≤ 2048 bytes. Return a `MetadataStoreError::Corrupt(...)` with a diagnostic message (not silent truncation). The free functions in `community_index.rs` must validate before INSERT.

7. **`platform_tags` storage format**. Store as space-separated string (e.g., `"linux steam-deck"`), not as a JSON array. Better FTS5 tokenization if FTS is added later; simpler `LIKE` queries. The `CommunityProfileMetadata.platform_tags` field is `Vec<String>` — join with `" "` before storing, split on `" "` when reading back.

8. **`community_profiles` uses `INTEGER PRIMARY KEY AUTOINCREMENT`**. Unlike other Phase 3 tables that use `TEXT PRIMARY KEY` with `db::new_id()`, `community_profiles.id` is an SQLite autoincrement integer. Do not generate a UUID for this column.

9. **FTS5 is deferred**. Do not add FTS5 virtual tables in Phase 3 migration. The schema is designed to add FTS5 later without migration complexity. Use `LIKE '%query%'` for community search until FTS5 is proven necessary.

10. **External cache HTTP fetch happens in Tauri command layer, not in `crosshook-core`**. There is no HTTP client in `crosshook-core`. The `cache_store.rs` module only stores/retrieves; the command layer in `collections.rs` (or future commands) is responsible for fetching and passing `payload_json` to the store.

11. **`collections.rs` needs its own `map_error`**. The `map_error` in `community.rs` is `fn map_error(error: impl ToString) -> String` — it is private. New commands in `collections.rs` must define the same function locally. Do not move it to a shared module (would violate the "one-domain-per-file" convention and introduce unnecessary coupling).

12. **`profile_set_favorite` goes in `commands/profile.rs`**, not `collections.rs`. The locked design decision places it alongside existing profile commands. The underlying store method delegates to `profile_sync::set_profile_favorite`.

---

## Task-Specific Guidance

### Task: migrate_3_to_4 (migration)

- Modify only `migrations.rs`
- Add the `if version < 4` guard immediately after line 37 (after the `if version < 3` block)
- The five tables must all be in one `execute_batch` call in `migrate_3_to_4`
- Test: in-memory store opens at v4; verify `PRAGMA user_version = 4`; verify all five tables exist via `SELECT name FROM sqlite_master WHERE type='table'`

### Task: community_index.rs (new submodule)

- Key functions: `sync_community_tap(conn: &mut Connection, ...) -> Result<(), MetadataStoreError>`, `lookup_community_tap_head(conn: &Connection, tap_url: &str, tap_branch: &str) -> Result<Option<String>, MetadataStoreError>`
- `sync_community_tap` is the DELETE+INSERT pattern (Pattern 4 above)
- `lookup_community_tap_head` returns the stored `last_head_commit` so callers can skip re-index when HEAD is unchanged
- In `mod.rs`: add `mod community_index;` and delegate method `pub fn sync_community_tap(&self, subscription, result) -> Result<(), MetadataStoreError>` using `with_conn_mut`

### Task: cache_store.rs (new submodule)

- Key functions: `upsert_cache_entry`, `lookup_cache_entry`, `evict_expired_entries`
- Apply `MAX_CACHE_PAYLOAD_BYTES = 512_000` from `models.rs` — add this constant before writing the submodule
- `evict_expired_entries`: `DELETE FROM external_cache_entries WHERE expires_at IS NOT NULL AND expires_at < ?1` with `now` as argument

### Task: community_sync watermark hook

- Modify `src-tauri/src/commands/community.rs` only
- Add `metadata_store: State<'_, MetadataStore>` to `community_sync` signature
- After `tap_store.sync_many(&taps)` succeeds, iterate results and call `metadata_store.sync_community_tap` inside a warn-and-continue block
- Update the command type-alias in the `command_names_match_expected_ipc_contract` test
- **Do not** skip re-index based on HEAD watermark at this layer — the watermark check belongs in a separate optimization pass if needed

### Task: collections commands (collections.rs)

- New file at `src-tauri/src/commands/collections.rs`
- Commands: `collection_create(name: String, description: Option<String>, metadata_store: State<'_, MetadataStore>) -> Result<String, String>` (returns `collection_id`)
- `collection_list`, `collection_delete`, `collection_add_profile`, `collection_remove_profile`, `collection_list_profiles`
- All take `metadata_store: State<'_, MetadataStore>` as final state arg
- Register all in `lib.rs` `invoke_handler!`
- Usage insights commands (`usage_top_profiles`, `usage_recent_launches`) can live in this file or a new `insights.rs` — they only need `State<'_, MetadataStore>` and run aggregate SQL over `launch_operations`

### Task: profile_set_favorite command

- Modify `commands/profile.rs` to add `profile_set_favorite(name: String, is_favorite: bool, is_pinned: bool, metadata_store: State<'_, MetadataStore>) -> Result<(), String>`
- Uses warn-and-continue: the primary operation IS the metadata write (unlike other commands where metadata is a side-effect), so this command returns the metadata error wrapped as `String`
- Add `set_profile_favorite` to `profile_sync.rs` following the `observe_profile_delete` shape
- Add `set_profile_favorite` delegate method to `MetadataStore` using `with_conn`
- Register in `lib.rs` `invoke_handler!`

### Task: lib.rs registration

- Add `mod commands::collections;` (or `mod collections;` inside `commands/mod.rs` if that file exists)
- Extend `invoke_handler!` with all new commands from `collections.rs` and `profile_set_favorite`
- Verify `metadata_store` is already `.manage()`d at line 80 — no new `.manage()` call needed for Phase 3

### Task: startup.rs community orphan sweep (optional)

- `run_metadata_reconciliation` currently calls `sync_profiles_from_store` and `sweep_abandoned_operations`
- Phase 3 may add: sweep community_taps rows whose `local_path` no longer exists on disk
- Only add this if the feature spec mandates it; otherwise defer
