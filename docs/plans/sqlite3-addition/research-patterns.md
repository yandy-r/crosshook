# Pattern Research: SQLite Metadata Layer Phase 3 — Catalog and Intelligence

## Overview

Phases 1 and 2 are fully implemented. All patterns below are extracted from live source code,
not speculation. Phase 3 (`community_index.rs`, `cache_store.rs`) must follow these exact shapes
to stay consistent inside `metadata/`. The `with_conn` fail-soft delegation, free function pattern,
`as_str()` enums, UPSERT reconciliation, and sequential migration runner are the load-bearing
conventions every new file must match.

---

## Relevant Files

- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/metadata/mod.rs` — `MetadataStore`, `with_conn`/`with_conn_mut`, public API, full test suite
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/metadata/db.rs` — `open_at_path`, `open_in_memory`, `new_id`, `configure_connection`
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/metadata/migrations.rs` — sequential migration runner, all three existing migration functions
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/metadata/models.rs` — `MetadataStoreError`, `SyncSource`, `LaunchOutcome`, `DriftState` with `as_str()`, `MAX_DIAGNOSTIC_JSON_BYTES`
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/metadata/profile_sync.rs` — UPSERT pattern, transaction usage, `lookup_profile_id` helper
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/metadata/launcher_sync.rs` — `with_conn_mut` usage, atomic rename transaction
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/metadata/launch_history.rs` — size-bounded JSON storage, `tracing::warn!` for rows_affected==0
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/community/taps.rs` — `CommunityTapStore`, `CommunityTapSubscription`, `head_commit` from `rev_parse_head`
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/community/index.rs` — `CommunityProfileIndex`, `CommunityProfileIndexEntry`, schema version check
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/community/mod.rs` — re-exports of `CommunityProfileManifest`, `CommunityProfileMetadata`, `CompatibilityRating`
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src-tauri/src/commands/community.rs` — all four community commands, `State<'_>` usage, `map_error` helper
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src-tauri/src/commands/export.rs` — warn-and-continue pattern (`if let Err(e) = ... { tracing::warn!(...) }`)
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src-tauri/src/commands/launch.rs` — async metadata hook via `spawn_blocking`, `record_launch_start` helper
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src-tauri/src/lib.rs` — `MetadataStore::try_new()` with `tracing::warn!` fallback to `MetadataStore::disabled()`
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/settings/mod.rs` — `SettingsStore`, `AppSettingsData`, TOML persistence pattern
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/settings/recent.rs` — `RecentFilesStore`, bounded list, load/save round-trip
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/Cargo.toml` — `rusqlite = { version = "0.38", features = ["bundled"] }` — FTS5 requires adding `"bundled-full"` here
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/components/CommunityBrowser.tsx` — component structure, `CollapsibleSection`, `ThemedSelect`, `crosshook-*` BEM classes
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/hooks/useCommunityProfiles.ts` — hook shape, `invoke`, `useEffect`/`useCallback` state management pattern

---

## Metadata Module Patterns (Phases 1-2)

### `with_conn` Fail-Soft Delegation

File: `metadata/mod.rs:59–95`

Every `MetadataStore` public method routes through `with_conn` (read) or `with_conn_mut` (write
needing `&mut Connection` for transactions). When the store is disabled or unavailable it returns
`Ok(T::default())` silently, never propagates errors to callers.

```rust
fn with_conn<F, T>(&self, action: &'static str, f: F) -> Result<T, MetadataStoreError>
where
    F: FnOnce(&Connection) -> Result<T, MetadataStoreError>,
    T: Default,
{
    if !self.available { return Ok(T::default()); }
    let Some(conn) = &self.conn else { return Ok(T::default()); };
    let guard = conn.lock().map_err(|_| {
        MetadataStoreError::Corrupt(format!("metadata store mutex poisoned while {action}"))
    })?;
    f(&guard)
}
```

Phase 3 public methods on `MetadataStore` follow this exact shape. Methods that need
`&mut Connection` (for `Transaction::new`) use `with_conn_mut` — see `observe_launcher_renamed`
at `mod.rs:186–196`.

### Free Function Pattern

All module-level logic lives in free functions with `conn: &Connection` (or `conn: &mut Connection`)
as the first argument. The `MetadataStore` methods are thin delegates:

```rust
// mod.rs — delegate
pub fn observe_profile_write(...) -> Result<(), MetadataStoreError> {
    self.with_conn("observe a profile write", |conn| {
        profile_sync::observe_profile_write(conn, name, profile, path, source, source_profile_id)
    })
}

// profile_sync.rs — implementation
pub fn observe_profile_write(
    conn: &Connection,
    name: &str,
    profile: &GameProfile,
    ...
) -> Result<(), MetadataStoreError> { ... }
```

Phase 3 files (`community_index.rs`, `cache_store.rs`) must expose public free functions with
`conn` first, delegated from `MetadataStore::with_conn` methods.

### Enum with `as_str()`

File: `metadata/models.rs:69–137`

All enums that map to SQLite TEXT columns implement `as_str()` returning `&'static str`. They
also derive `Serialize`/`Deserialize` with `#[serde(rename_all = "snake_case")]`.

```rust
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DriftState {
    Unknown,
    Aligned,
    Missing,
    Moved,
    Stale,
}

impl DriftState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Unknown => "unknown",
            Self::Aligned => "aligned",
            Self::Missing => "missing",
            Self::Moved => "moved",
            Self::Stale => "stale",
        }
    }
}
```

Phase 3 will need at least one new enum for the cache entry's validation state (e.g.
`CacheEntryStatus { Valid, Stale, Oversized, Corrupt }`). Follow this exact shape. Use
`SomeEnum::Variant.as_str()` when binding to SQLite params.

### UPSERT Reconciliation Pattern

File: `metadata/profile_sync.rs:28–68`, `metadata/launcher_sync.rs:20–50`

Insert-or-update uses SQLite `ON CONFLICT(...) DO UPDATE SET`. Idempotency is guaranteed by
`UNIQUE` constraints on the natural key. The primary key (`*_id`) is always `db::new_id()`
(UUID v4) and never reused — the conflict target is always the natural key column.

```sql
INSERT INTO profiles (profile_id, current_filename, ...)
VALUES (?1, ?2, ...)
ON CONFLICT(current_filename) DO UPDATE SET
    current_path = excluded.current_path,
    ...
    updated_at = excluded.updated_at
```

For `community_index.rs`, the natural key for a tap entry is `(tap_url, tap_branch, relative_path)`.
For `cache_store.rs`, the natural key is the external URL or a deterministic slug.

### Sequential Migration Runner

File: `metadata/migrations.rs:4–40`

Each migration guard is `if version < N { migrate_N_minus_1_to_N(conn)?; conn.pragma_update(..., N)?; }`.
All guards are evaluated sequentially — a fresh DB runs all of them in order, an existing
DB at version 3 runs only the new ones.

```rust
pub fn run_migrations(conn: &Connection) -> Result<(), MetadataStoreError> {
    let version = conn.pragma_query_value(None, "user_version", |row| row.get::<_, u32>(0))?;

    if version < 1 { migrate_0_to_1(conn)?; conn.pragma_update(None, "user_version", 1_u32)?; }
    if version < 2 { migrate_1_to_2(conn)?; conn.pragma_update(None, "user_version", 2_u32)?; }
    if version < 3 { migrate_2_to_3(conn)?; conn.pragma_update(None, "user_version", 3_u32)?; }
    // Phase 3:
    if version < 4 { migrate_3_to_4(conn)?; conn.pragma_update(None, "user_version", 4_u32)?; }

    Ok(())
}
```

Phase 3 adds a `migrate_3_to_4` function at the bottom of `migrations.rs` and adds the
`if version < 4` guard. All Phase 3 tables (`community_tap_index`, `collections`,
`collection_entries`, `external_cache`) go into this single migration function.

### Structured Error Mapping

File: `metadata/models.rs:7–67`

Errors carry both a human-readable `action: &'static str` and the wrapped source. Every
`conn.execute(...)` call is followed by `.map_err(|source| MetadataStoreError::Database { action: "...", source })`.

```rust
conn.execute("...", params![...])
    .map_err(|source| MetadataStoreError::Database {
        action: "upsert a community tap index row",
        source,
    })?;
```

The action string is always in lowercase, imperative, describes what the operation was doing
(e.g. `"upsert a community tap index row"`, `"insert an external cache row"`).

### Test Patterns

File: `metadata/mod.rs:229–893`

All tests:

- Open with `MetadataStore::open_in_memory()` — never a real filesystem path
- Use the private `connection()` helper (`mod.rs:282–289`) to get a raw `MutexGuard<Connection>` and run direct SQL assertions
- Test idempotency (call the same write twice, assert `COUNT(*) = 1`)
- Test the disabled store returns `Ok` (noop assertions)
- Use `tempdir()` only when testing filesystem-side concerns (permissions, symlink rejection)

```rust
fn connection(store: &MetadataStore) -> std::sync::MutexGuard<'_, Connection> {
    store.conn.as_ref().expect("...").lock().expect("...")
}

#[test]
fn test_phase3_index_tap_inserts_row() {
    let store = MetadataStore::open_in_memory().unwrap();
    store.index_community_tap("https://example.invalid/taps.git", None, "abc123", 3).unwrap();
    let conn = connection(&store);
    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM community_tap_index WHERE tap_url = ?1",
        params!["https://example.invalid/taps.git"],
        |row| row.get(0),
    ).unwrap();
    assert_eq!(count, 1);
}
```

---

## Community Module Patterns

File: `community/taps.rs`, `community/index.rs`, `community/mod.rs`

### Tap Data Structures

```
CommunityTapSubscription { url: String, branch: Option<String> }   — persisted in AppSettings
CommunityTapWorkspace    { subscription, local_path: PathBuf }     — resolved at runtime
CommunityTapSyncResult   { workspace, status, head_commit: String, index: CommunityProfileIndex }
CommunityProfileIndex    { entries: Vec<CommunityProfileIndexEntry>, diagnostics: Vec<String> }
CommunityProfileIndexEntry { tap_url, tap_branch, tap_path, manifest_path, relative_path, manifest }
```

The `head_commit` comes from `git rev-parse HEAD` on the local workspace (`taps.rs:237–258`).
This is the watermark for Phase 3's skip-if-unchanged optimization: store the last-indexed
`head_commit` per tap in `community_tap_index` and skip `collect_manifests` when HEAD matches.

### `CommunityTapStore` Operations

- `sync_tap(subscription)` — clones if missing, fetch+reset+clean if present, returns `head_commit` + parsed index
- `sync_many(subscriptions)` — iterates `sync_tap` in sequence (not parallel)
- `index_workspaces(workspaces)` — calls `index::index_taps`, aggregates entries, sorts alphabetically by `game_name`
- `resolve_workspace(subscription)` — constructs `CommunityTapWorkspace` with `base_path/slug` path, no I/O

### Schema Version Check

`index.rs:145–150` — manifests with wrong schema version are skipped with a diagnostic message
rather than treated as an error. Phase 3's `community_index.rs` should follow the same pattern
when re-indexing: skip stale manifests gracefully, record diagnostics.

---

## Tauri Command Patterns

File: `src-tauri/src/commands/community.rs`, `export.rs`, `launch.rs`

### Command Signature Shape

All synchronous commands:

```rust
#[tauri::command]
pub fn community_add_tap(
    tap: CommunityTapSubscription,
    settings_store: State<'_, SettingsStore>,
) -> Result<Vec<CommunityTapSubscription>, String> { ... }
```

Multiple state args in any order — Tauri resolves them by type. Return type is always
`Result<T, String>` where the error is the display string of the underlying error.

Async commands use `app: AppHandle` + `app.state::<T>().inner().clone()` to extract managed state:

```rust
let metadata_store = app.state::<MetadataStore>().inner().clone();
```

### `map_error` Helper

`community.rs:8–10` — a single private helper converts any `Display` impl to `String`:

```rust
fn map_error(error: impl ToString) -> String {
    error.to_string()
}
```

Used as `.map_err(map_error)` throughout the file. Phase 3 community commands should use the
same helper (already present in `community.rs`).

### Warn-and-Continue (Metadata Hooks)

File: `export.rs:26–38`, `export.rs:79–84`, `export.rs:102–106`, `export.rs:151–159`

The canonical pattern for non-critical metadata side-effects:

```rust
if let Err(e) = metadata_store.observe_launcher_exported(
    request.profile_name.as_deref(),
    &result.launcher_slug,
    ...
) {
    tracing::warn!(%e, launcher_slug = %result.launcher_slug,
        "metadata sync after export_launchers failed");
}
```

The primary operation result is returned regardless. Phase 3 Tauri commands that call
`metadata_store.index_community_tap(...)` or `metadata_store.cache_external_metadata(...)`
must wrap these calls in the same pattern — the community sync must not fail because metadata
recording failed.

### Async Metadata via `spawn_blocking`

File: `launch.rs:165–193`

SQLite is synchronous; async commands offload it to `spawn_blocking`:

```rust
let operation_id = tauri::async_runtime::spawn_blocking(move || {
    ms_clone.record_launch_started(pn.as_deref(), method, Some(&lp))
})
.await
.unwrap_or_else(|e| { tracing::warn!("metadata spawn_blocking join failed: {e}"); Ok(String::new()) })
.unwrap_or_else(|e| { tracing::warn!(%e, "record_launch_started failed"); String::new() });
```

Phase 3 commands that are `async fn` and call metadata must use `spawn_blocking` with this
double `unwrap_or_else` (join error + inner Result error).

### State Registration

`lib.rs:76–81` — all managed stores are registered with `.manage(store)` before
`.invoke_handler(...)`. MetadataStore is already registered. New Phase 3 commands that need
`State<'_, MetadataStore>` require no new registration — just add them to `invoke_handler`.

---

## Frontend Patterns

File: `src/components/CommunityBrowser.tsx`, `src/hooks/useCommunityProfiles.ts`

### Hook Shape

All feature hooks return a result interface with a flat state bag plus async action functions:

```typescript
export interface UseCommunityProfilesResult {
  taps: CommunityTapSubscription[];
  index: CommunityProfileIndex;
  loading: boolean;
  syncing: boolean;
  importing: boolean;
  error: string | null;
  refreshProfiles: () => Promise<void>;
  syncTaps: () => Promise<void>;
  addTap: (tap) => Promise<CommunityTapSubscription[]>;
  ...
}
```

A new `useCollections` hook for Phase 3 would follow this shape:

```typescript
export interface UseCollectionsResult {
  collections: Collection[];
  loading: boolean;
  error: string | null;
  createCollection: (name: string) => Promise<void>;
  addToCollection: (collectionId: string, profileId: string) => Promise<void>;
  removeFromCollection: (collectionId: string, profileId: string) => Promise<void>;
  deleteCollection: (collectionId: string) => Promise<void>;
}
```

### `invoke` Call Pattern

All backend calls go through `invoke<ReturnType>('command_name', { arg1, arg2 })`.
State loading at mount uses `useEffect` with an `active` boolean guard to prevent
state updates after unmount:

```typescript
useEffect(() => {
  let active = true;
  async function load() {
    try {
      const result = await invoke<T>('my_command');
      if (!active) return;
      setState(result);
    } catch (e) {
      if (active) setError(String(e));
    } finally {
      if (active) setLoading(false);
    }
  }
  void load();
  return () => {
    active = false;
  };
}, []);
```

### Component Structure

`CommunityBrowser.tsx` shows the standard layout pattern:

- `<section className="crosshook-card crosshook-community-browser">` as root
- `<header>` with eyebrow + h2 + copy paragraph
- `<CollapsibleSection>` panels for logical groups
- `className="crosshook-button"` / `crosshook-button--secondary` for all buttons
- `className="crosshook-input"` for inputs, `crosshook-label` for labels
- `className="crosshook-muted"` for secondary text
- Error state: `<p className="crosshook-community-browser__error">{error}</p>`
- Loading state: `<p className="crosshook-muted ...">Loading...</p>`
- Empty state: `<p className="crosshook-community-browser__empty">No results...</p>`

A collections/favorites panel would live inside a new `<CollapsibleSection title="Collections">`
inside `CommunityBrowser`, or as a separate component following the same shell.

### TypeScript Types

Types are declared locally in the hook file and exported from `src/types/index.ts` re-export.
All snake_case field names mirror the Rust serde output exactly:

```typescript
export interface CommunityProfileIndexEntry {
  tap_url: string; // matches Rust field name via serde default
  tap_branch?: string;
  relative_path: string;
  manifest_path: string;
  manifest: CommunityProfileManifest;
}
```

Phase 3 types (Collection, CollectionEntry, CacheEntry, UsageStats) follow this convention.

---

## Cache/Storage Patterns

### `RecentFilesStore` Pattern

File: `settings/recent.rs`

The simplest store pattern — a struct wrapping a `PathBuf`, `load()` returns `Default` when
file is absent, `save()` creates parent directories then writes TOML. All validation
happens inside `load()` (not `save()`).

### `AppSettingsData` Persistence

File: `settings/mod.rs:19–25`

```rust
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, Default)]
#[serde(default)]
pub struct AppSettingsData {
    pub auto_load_last_profile: bool,
    pub last_used_profile: String,
    pub community_taps: Vec<CommunityTapSubscription>,
}
```

Key conventions:

- `#[serde(default)]` on the struct — missing TOML fields deserialize to `Default::default()`
- `Default` derive — `load()` returns `AppSettingsData::default()` when file is absent
- Round-trip is `toml::from_str` / `toml::to_string_pretty`

### JSON Serialization for Cached Payloads

File: `metadata/launch_history.rs:66–82`

The established pattern for storing JSON payloads with size bounds:

```rust
// Serialize; nullify if over 4KB
let json = serde_json::to_string(report).ok();
let json = json.filter(|s| s.len() <= MAX_DIAGNOSTIC_JSON_BYTES);
```

For Phase 3 `cache_store.rs`, the external metadata payload should follow the same pattern:

- Serialize with `serde_json::to_string`
- Apply a `MAX_CACHE_PAYLOAD_BYTES` constant (analogous to `MAX_DIAGNOSTIC_JSON_BYTES = 4_096`)
- Store `NULL` if oversized; record `oversized` status in a separate column rather than storing partial JSON
- The size constant should live in `models.rs` alongside `MAX_DIAGNOSTIC_JSON_BYTES`

---

## FTS5 Patterns

### rusqlite Feature Flag

Current `Cargo.toml:15`:

```toml
rusqlite = { version = "0.38", features = ["bundled"] }
```

FTS5 is available with the bundled build of SQLite (already enabled). However, `rusqlite`
itself exposes FTS5 helper APIs only with the `"bundled-full"` feature, which enables the
`LoadableExtension` and `fts5` tokenizer hooks. For basic FTS5 via SQL `CREATE VIRTUAL TABLE`
and `MATCH` queries, the existing `"bundled"` feature is sufficient — FTS5 is compiled into
SQLite when `bundled` is used.

To add FTS5 support: no Cargo.toml change is required for basic `CREATE VIRTUAL TABLE ... USING fts5(...)`.
Only change to `"bundled-full"` if Rust-side FTS5 tokenizer registration is needed.

### FTS5 Migration Shape

```rust
fn migrate_3_to_4(conn: &Connection) -> Result<(), MetadataStoreError> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS community_tap_index ( ... );
         CREATE TABLE IF NOT EXISTS collections ( ... );
         CREATE TABLE IF NOT EXISTS collection_entries ( ... );
         CREATE TABLE IF NOT EXISTS external_cache ( ... );
         -- Optional FTS5 virtual table:
         CREATE VIRTUAL TABLE IF NOT EXISTS community_fts
             USING fts5(game_name, trainer_name, author, description,
                        content='community_tap_index', content_rowid='rowid');",
    )
    .map_err(|source| MetadataStoreError::Database {
        action: "run metadata migration 3 to 4",
        source,
    })?;
    Ok(())
}
```

FTS5 `MATCH` queries are standard SQL: `SELECT * FROM community_fts WHERE community_fts MATCH ?1`.
Results can be ranked with `rank` column. The `content=` and `content_rowid=` options create a
content table FTS index backed by the real table — requires explicit sync via `INSERT INTO
community_fts(community_fts, rowid, ...) VALUES ('delete', ...)` on deletes/updates.

---

## Patterns to Follow for Phase 3

1. **`community_index.rs`** — free functions `index_community_tap(conn, url, branch, head_commit, entry_count)`,
   `list_indexed_taps(conn)`, `get_tap_head_commit(conn, url, branch)`. All follow the free
   function pattern. UPSERT on `(tap_url, tap_branch)` natural key. `MetadataStore` delegates
   via `with_conn`.

2. **`cache_store.rs`** — free functions `upsert_cache_entry(conn, url, payload_json, ttl_secs)`,
   `get_cache_entry(conn, url)`, `sweep_expired_cache(conn)`. JSON payload size-bounded by a
   constant in `models.rs`. UPSERT on `url` natural key.

3. **New enums** — `CacheEntryStatus { Valid, Stale, Oversized, Corrupt }` follows `DriftState`
   shape exactly: `Copy`, `as_str()`, `#[serde(rename_all = "snake_case")]`.

4. **Migration** — single `migrate_3_to_4` function added at bottom of `migrations.rs`, with
   corresponding `if version < 4 { ... }` guard in `run_migrations`. All Phase 3 tables in
   one migration, not separate ones.

5. **Tauri commands** — add to `commands/community.rs` using existing `map_error` helper,
   `State<'_, MetadataStore>` parameter, warn-and-continue for metadata side-effects.
   Register new command names in `lib.rs` `invoke_handler!` list.

6. **Tests** — `open_in_memory()` + `connection()` helper for all unit tests. Test idempotency,
   disabled-store noop, and size-bound behavior (analogous to `test_diagnostic_json_truncated_at_4kb`).

7. **Frontend** — new hook `useCollections` following `useCommunityProfiles` shape: `loading`,
   `error`, async action functions with `try/catch/finally`. Component uses `crosshook-card`,
   `CollapsibleSection`, BEM-style `crosshook-collections__*` class names.

---

## Corrections and Gotchas (from Integration Research)

The following corrections and non-obvious gotchas were identified during integration research
and must be applied when implementing Phase 3.

### UPSERT vs DELETE+INSERT for Community Profiles Re-index

**Gotcha:** The UPSERT pattern works for `community_taps` (keyed on `tap_url + tap_branch`) but
does NOT work correctly for `community_profiles` when re-indexing.

**Why:** On re-index, profiles that existed in a previous sync but are no longer present in the
tap will not be touched by UPSERT — they become stale ghost rows. The correct approach is a
transactional DELETE + INSERT:

```rust
// Inside a Transaction::new(conn, TransactionBehavior::Immediate):
// 1. DELETE all rows for this tap_id
// 2. INSERT each entry from the new index
tx.execute("DELETE FROM community_profiles WHERE tap_id = ?1", params![tap_id])?;
for entry in &entries {
    tx.execute("INSERT INTO community_profiles (...) VALUES (...)", params![...])?;
}
tx.commit()?;
```

This mirrors `sync_profiles_from_store` in `profile_sync.rs:186–258` which handles the
"profile disappeared from filesystem" case via explicit delete logic.

### NULL Uniqueness Gotcha for `community_taps`

**Gotcha:** SQLite treats `NULL != NULL` in UNIQUE indexes, so `UNIQUE(tap_url, tap_branch)` with
`tap_branch = NULL` allows duplicate rows for the same tap URL with no branch.

**Fix:** Use `COALESCE` in the unique index definition:

```sql
CREATE UNIQUE INDEX IF NOT EXISTS idx_community_taps_url_branch
    ON community_taps(tap_url, COALESCE(tap_branch, ''));
```

Or store branch as `''` (empty string) instead of `NULL` when branch is absent, and use a
standard `UNIQUE(tap_url, tap_branch)` constraint with `NOT NULL DEFAULT ''` on the column.

### `platform_tags` Storage

**Gotcha:** SQLite has no native array column type. `Vec<String>` platform tags must be
serialized to JSON for storage:

```rust
let tags_json = serde_json::to_string(&entry.manifest.metadata.platform_tags)
    .unwrap_or_else(|_| "[]".to_string());
// store tags_json as TEXT
```

FTS5 indexes this as raw text (`'["linux","steam-deck"]'`), which is functional but
not token-clean. For precise FTS5 matching on individual tags, store as a space-separated
string (`"linux steam-deck"`) instead.

### FTS5 Content Table Sync Requirement

**Gotcha:** Using `content='community_profiles'` in the FTS5 virtual table definition means
changes to the base table do NOT automatically propagate to the FTS index. Every insert,
update, and delete on `community_profiles` requires an explicit FTS5 sync:

```sql
-- After INSERT into community_profiles:
INSERT INTO community_fts(rowid, game_name, trainer_name, author, description)
    VALUES (last_insert_rowid(), ?1, ?2, ?3, ?4);

-- After DELETE from community_profiles:
INSERT INTO community_fts(community_fts, rowid) VALUES ('delete', ?1);
```

Because Phase 3 re-indexes via DELETE + INSERT, the FTS table must be rebuilt in the same
transaction. Alternatively, use an independent (non-content-backed) FTS5 table that is
fully rebuilt on each tap re-index — simpler to reason about and consistent with the
DELETE+INSERT re-index approach.

### Collections Commands Location

Integration research confirms the one-domain-per-file convention. New collection commands go
in a new file `src-tauri/src/commands/collections.rs`, not appended to `community.rs`.
`profile_set_favorite` goes in the existing `commands/profile.rs` since it modifies a profile
record. Both follow the same `State<'_, MetadataStore>` + warn-and-continue pattern.

### `lookup_profile_id` Reuse

`profile_sync.rs:72–86` exposes `pub fn lookup_profile_id(conn: &Connection, name: &str)`.
This is the bridge from a profile name string to its stable UUID. Collection membership
operations that link profiles to collections must call this helper to resolve the `profile_id`
foreign key — do not duplicate the lookup query.
