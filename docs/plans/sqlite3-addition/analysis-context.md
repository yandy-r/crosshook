# Context Analysis: SQLite Metadata Layer Phase 3 — Catalog and Intelligence

Synthesized from: `shared.md`, `feature-spec.md` (lines 223–225, 524–534), `research-architecture.md`,
`research-patterns.md`, `research-integration.md`, `research-docs.md`.

---

## Executive Summary

Phases 1 and 2 are fully merged. Phase 3 adds five new SQLite tables via a single `migrate_3_to_4` migration, two new metadata submodule files (`community_index.rs`, `cache_store.rs`), collections/favorites CRUD commands, usage insights queries over existing `launch_operations`, and an external cache scaffold. Every new construct follows the exact `with_conn` fail-soft, free-function, warn-and-continue patterns already verified in Phases 1-2 — no new dependencies or structural changes to existing modules are needed.

---

## Architecture Context

### System Structure

```
metadata/mod.rs             — MetadataStore; add ~10 new public methods delegating to submodules
metadata/db.rs              — Unchanged (connection factory, new_id(), configure_connection())
metadata/migrations.rs      — Add migrate_3_to_4() + "if version < 4" guard
metadata/models.rs          — Add CacheEntryStatus enum, CommunityProfileRow, CollectionRow,
                              MAX_CACHE_PAYLOAD_BYTES = 512_000
metadata/profile_sync.rs    — Unchanged; lookup_profile_id() reused by collections/favorites
metadata/launcher_sync.rs   — Unchanged
metadata/launch_history.rs  — Unchanged; read-only for usage insights queries
metadata/community_index.rs [NEW] — free functions: index_community_tap_result, list_community_profiles
metadata/cache_store.rs     [NEW] — free functions: get_cache_entry, put_cache_entry, evict_expired_cache
src-tauri/src/commands/community.rs   — Add State<MetadataStore>, inline sync_tap_index after sync_many
src-tauri/src/commands/collections.rs [NEW] — collection_list, create, delete, add/remove profile
src-tauri/src/commands/profile.rs     — Add profile_set_favorite
src-tauri/src/lib.rs                  — Register new commands in invoke_handler!
```

### Data Flow

```
community_sync (Tauri command)
  → tap_store.sync_many(&taps) → Vec<CommunityTapSyncResult>
        each result: { workspace: { subscription.url, .branch }, head_commit, index.entries }
  → for each result (fail-soft):
      metadata_store.index_community_tap_result(result)
          └── community_index::index_community_tap_result(conn, result)
                → compare head_commit vs community_taps.last_head_commit
                → if unchanged: return Ok(()) (watermark skip)
                → if changed:
                    UPSERT community_taps (tap_id, url, branch, head_commit, profile_count)
                    Transaction::new + DELETE community_profiles WHERE tap_id = ?
                    INSERT each entry (validate A6 bounds first)
                    tx.commit()
  → Ok(results)  [metadata failure does not block community_sync return]

community_list_profiles (Tauri command, Phase 3 fast-path)
  → if MetadataStore available AND all taps indexed:
      metadata_store.list_community_profiles(tap_url) → Vec<CommunityProfileRow>
  → else fallback: tap_store.index_workspaces() full disk scan

profile_set_favorite (new Tauri command)  [PRIMARY OPERATION — error propagates to frontend]
  → metadata_store.set_profile_favorite(name, favorite).map_err(map_error)?
      └── UPDATE profiles SET is_favorite=?1 WHERE current_filename=?2
  → Ok(())  [NOT warn-and-continue; this command IS the metadata write]

collection_add_profile (new Tauri command)
  → metadata_store.add_profile_to_collection(collection_id, profile_name)
      └── lookup_profile_id(conn, name) → profile_id
          INSERT INTO collection_profiles (collection_id, profile_id, added_at)
```

### Integration Points

Phase 3 touches **three existing files** and adds **two new files** in commands/:

- `commands/community.rs` — add `State<'_, MetadataStore>` + fail-soft index hook
- `commands/profile.rs` — add `profile_set_favorite` command
- `src-tauri/src/lib.rs` — register ~8 new commands in `invoke_handler!`
- `commands/collections.rs` [NEW] — 6-7 collection CRUD commands
- `startup.rs` — optionally add community index orphan cleanup

---

## Critical Files Reference

| File                                                                                                                           | Why Critical                                                                                                               |
| ------------------------------------------------------------------------------------------------------------------------------ | -------------------------------------------------------------------------------------------------------------------------- |
| `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/metadata/mod.rs`             | Add ~10 new public methods via `with_conn`/`with_conn_mut`; verify `with_conn` and `with_conn_mut` shape at lines 59-95    |
| `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/metadata/migrations.rs`      | Add `migrate_3_to_4()` DDL for all 5 new tables; add `if version < 4` guard                                                |
| `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/metadata/models.rs`          | Add `CacheEntryStatus`, row structs, `MAX_CACHE_PAYLOAD_BYTES = 512_000`; `MAX_DIAGNOSTIC_JSON_BYTES` pattern at line 141  |
| `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/metadata/profile_sync.rs`    | `lookup_profile_id(conn, name)` at lines 72-86 — reuse for favorites and collection membership; do not duplicate           |
| `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/metadata/launcher_sync.rs`   | Template for `Transaction::new(conn, TransactionBehavior::Immediate)` — copy for DELETE+INSERT community_profiles re-index |
| `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/metadata/launch_history.rs`  | Size-bounded JSON pattern at lines 66-82 — copy for `MAX_CACHE_PAYLOAD_BYTES` in cache_store.rs                            |
| `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/community/taps.rs`           | `CommunityTapSyncResult` struct at line 40-46 — watermark source; `head_commit: String` at line 44                         |
| `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/community/index.rs`          | `CommunityProfileIndex`, `CommunityProfileIndexEntry` shapes; schema version skip at line 145                              |
| `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src-tauri/src/commands/community.rs`                   | `community_sync` signature (add `State<MetadataStore>`); `map_error` helper at lines 8-10                                  |
| `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src-tauri/src/commands/export.rs`                      | Warn-and-continue pattern at lines 26-38 — exact template for Phase 3 metadata hooks                                       |
| `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src-tauri/src/lib.rs`                                  | `invoke_handler!` list at line ~109; `.manage()` at line 80; add new command registrations                                 |
| `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/profile/community_schema.rs` | `CommunityProfileManifest`, `CommunityProfileMetadata` field list — drives community_profiles DDL columns                  |

### Files to Create

- `crates/crosshook-core/src/metadata/community_index.rs`
- `crates/crosshook-core/src/metadata/cache_store.rs`
- `src-tauri/src/commands/collections.rs`

---

## Phase 3 Schema (migrate_3_to_4)

```sql
-- All Phase 3 tables in one migration function

CREATE TABLE IF NOT EXISTS community_taps (
    tap_id           TEXT PRIMARY KEY,
    tap_url          TEXT NOT NULL,
    tap_branch       TEXT NOT NULL DEFAULT '',   -- empty string, NOT NULL; avoids SQLite NULL!=NULL in UNIQUE
    local_path       TEXT NOT NULL,
    last_head_commit TEXT,                        -- 40-char SHA; NULL = never indexed
    profile_count    INTEGER NOT NULL DEFAULT 0,
    last_indexed_at  TEXT,
    created_at       TEXT NOT NULL,
    updated_at       TEXT NOT NULL
);
CREATE UNIQUE INDEX IF NOT EXISTS idx_community_taps_url_branch
    ON community_taps(tap_url, tap_branch);       -- safe because tap_branch is NOT NULL

CREATE TABLE IF NOT EXISTS community_profiles (
    id               INTEGER PRIMARY KEY AUTOINCREMENT,
    tap_id           TEXT NOT NULL REFERENCES community_taps(tap_id),
    relative_path    TEXT NOT NULL,
    manifest_path    TEXT NOT NULL,
    game_name        TEXT,          -- A6: <= 512 bytes
    game_version     TEXT,
    trainer_name     TEXT,          -- A6: <= 512 bytes
    trainer_version  TEXT,
    proton_version   TEXT,
    compatibility_rating TEXT,
    author           TEXT,          -- A6: <= 512 bytes
    description      TEXT,          -- A6: <= 4096 bytes
    platform_tags TEXT,             -- space-separated: "linux steam-deck"; A6: <= 2048 bytes
    schema_version   INTEGER NOT NULL DEFAULT 1,
    created_at       TEXT NOT NULL
);
CREATE UNIQUE INDEX IF NOT EXISTS idx_community_profiles_tap_relative
    ON community_profiles(tap_id, relative_path);

CREATE TABLE IF NOT EXISTS external_cache_entries (
    cache_id      TEXT PRIMARY KEY,
    source_url    TEXT NOT NULL,
    cache_key     TEXT NOT NULL UNIQUE,
    payload_json  TEXT,             -- NULL if > 512_000 bytes or invalid JSON
    payload_size  INTEGER NOT NULL DEFAULT 0,
    fetched_at    TEXT NOT NULL,
    expires_at    TEXT,
    created_at    TEXT NOT NULL,
    updated_at    TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS collections (
    collection_id TEXT PRIMARY KEY,
    name          TEXT NOT NULL UNIQUE,
    description   TEXT,
    created_at    TEXT NOT NULL,
    updated_at    TEXT NOT NULL
);

CREATE TABLE IF NOT EXISTS collection_profiles (
    collection_id TEXT NOT NULL REFERENCES collections(collection_id) ON DELETE CASCADE,
    profile_id    TEXT NOT NULL REFERENCES profiles(profile_id),
    added_at      TEXT NOT NULL,
    PRIMARY KEY (collection_id, profile_id)
);
CREATE INDEX IF NOT EXISTS idx_collection_profiles_profile_id
    ON collection_profiles(profile_id);
```

Key decision: `tap_branch NOT NULL DEFAULT ''` avoids SQLite `NULL != NULL` UNIQUE constraint issue that would allow duplicate tap rows. Empty string represents "no explicit branch".

---

## Patterns to Follow

### with_conn Fail-Soft Delegation (`metadata/mod.rs:59-95`)

Every `MetadataStore` Phase 3 public method must follow this shape exactly:

```rust
pub fn index_community_tap_result(&self, result: &CommunityTapSyncResult) -> Result<(), MetadataStoreError> {
    self.with_conn_mut("index a community tap", |conn| {
        community_index::index_community_tap_result(conn, result)
    })
}
```

Use `with_conn_mut` when the free function needs `Transaction::new` (DELETE+INSERT); use `with_conn` for read-only queries.

### Free Function Pattern (`metadata/profile_sync.rs`)

```rust
// community_index.rs
pub fn index_community_tap_result(conn: &mut Connection, result: &CommunityTapSyncResult) -> Result<(), MetadataStoreError> {
    let url = &result.workspace.subscription.url;
    let branch = result.workspace.subscription.branch.as_deref().unwrap_or("");
    let head = &result.head_commit;
    // 1. check watermark
    // 2. UPSERT community_taps
    // 3. Transaction DELETE + INSERT community_profiles
}
```

### Transactional DELETE+INSERT for community_profiles Re-index

UPSERT does NOT work for re-index because removed profiles become stale ghost rows. Copy the `Transaction::new(conn, TransactionBehavior::Immediate)` pattern from `launcher_sync.rs`:

```rust
let tx = Transaction::new(conn, TransactionBehavior::Immediate)?;
tx.execute("DELETE FROM community_profiles WHERE tap_id = ?1", params![tap_id])?;
for entry in &result.index.entries {
    // validate A6 bounds before each INSERT
    tx.execute("INSERT INTO community_profiles (...) VALUES (...)", params![...])?;
}
tx.commit()?;
```

### Enum Pattern (`metadata/models.rs:69-137`)

New `CacheEntryStatus` enum must match existing shape exactly:

```rust
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CacheEntryStatus { Valid, Stale, Oversized, Corrupt }

impl CacheEntryStatus {
    pub fn as_str(self) -> &'static str { match self { ... } }
}
```

### Size-Bounded JSON Payload (`metadata/launch_history.rs:66-82`)

```rust
// In cache_store.rs
let json = serde_json::to_string(&payload).ok();
let json = json.filter(|s| s.len() <= MAX_CACHE_PAYLOAD_BYTES);  // None if oversized
// Store json (may be NULL), always store payload_size
```

`MAX_CACHE_PAYLOAD_BYTES = 512_000` lives in `models.rs` alongside `MAX_DIAGNOSTIC_JSON_BYTES = 4_096`.

### Warn-and-Continue (Tauri Commands, `commands/export.rs:26-38`)

```rust
// In community_sync after sync_many():
for result in &results {
    if let Err(e) = metadata_store.index_community_tap_result(result) {
        tracing::warn!(%e, tap_url = %result.workspace.subscription.url,
            "community tap index sync failed");
    }
}
```

The primary `Ok(results)` is returned regardless.

### Structured Error Mapping

All SQL errors must use:

```rust
.map_err(|source| MetadataStoreError::Database { action: "insert a community profile row", source })?
```

`action` is always a `&'static str` lowercase gerund phrase. Never `format!()`.

### Test Pattern (`metadata/mod.rs:229-893`)

```rust
#[test]
fn test_index_community_tap_inserts_row() {
    let store = MetadataStore::open_in_memory().unwrap();
    // call store method
    let conn = connection(&store);  // private helper at mod.rs:282-289
    let count: i64 = conn.query_row("SELECT COUNT(*) FROM community_taps ...", ...).unwrap();
    assert_eq!(count, 1);
}
```

Every new submodule needs: idempotency test, disabled-store noop test, size-bound test.

---

## Cross-Cutting Concerns

### Security (W3 / W6 / W8 / A6)

| Ref | Rule                                                                             | Where to Enforce                                                                                      |
| --- | -------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------- |
| W3  | `external_cache_entries.payload_json` max 512 KB                                 | `cache_store.rs` before INSERT; `MAX_CACHE_PAYLOAD_BYTES` in `models.rs`                              |
| W6  | Re-validate stored `manifest_path` before any `fs::` call                        | Anywhere `community_profiles.manifest_path` is used in filesystem ops                                 |
| W8  | Audit no `dangerouslySetInnerHTML` for manifest fields in `CommunityBrowser.tsx` | JSX review — current code is safe but must stay safe                                                  |
| A6  | `game_name` ≤ 512B, `description` ≤ 4096B, `platform_tags` ≤ 2048B               | `community_index.rs` before each `community_profiles` INSERT; reject with diagnostic, do not truncate |

A6 rejection means: log a `tracing::warn!`, add the entry to `result.index.diagnostics`, skip the INSERT. Continue with remaining entries.

### Fail-Soft at All Levels

1. `MetadataStore` always present — no `Option<MetadataStore>` in Tauri state.
2. All methods route through `with_conn` — auto no-op when `available = false`.
3. All Tauri command call sites use `if let Err(e) { tracing::warn! }` — never `?` on metadata. **Exception**: `profile_set_favorite` IS the metadata write (primary operation), so its error propagates via `.map_err(map_error)?` to the frontend, not swallowed.
4. `community_list_profiles` has a disk-scan fallback if SQLite is unavailable.

### No New Cargo Dependencies

`rusqlite 0.38 bundled`, `serde_json`, `chrono`, `uuid`, `sha2` all already present. FTS5 is available in the bundled build via `CREATE VIRTUAL TABLE USING fts5` without changing `Cargo.toml`. HTTP client stays deferred — external cache fetch belongs in the Tauri command layer, not `crosshook-core`.

### `lookup_profile_id` Reuse

`profile_sync.rs:72-86` — `pub fn lookup_profile_id(conn: &Connection, name: &str) -> Result<Option<String>>`. Both `profile_set_favorite` (UPDATE profiles) and `collection_add_profile` must use this to resolve `profile_name → profile_id`. Do not duplicate the query.

---

## Parallelization Opportunities

| Track                  | Work                                                                                           | Dependency                                             |
| ---------------------- | ---------------------------------------------------------------------------------------------- | ------------------------------------------------------ |
| A — Migration + Models | `models.rs` additions (enums, consts, row structs) → `migrations.rs` v4 DDL                    | Sequential within track; gates all others              |
| B — Community Index    | `community_index.rs` free functions + `mod.rs` delegates + `community.rs` hook                 | Depends on Track A tables existing                     |
| C — Cache Store        | `cache_store.rs` free functions + `mod.rs` delegates                                           | Depends on Track A; independent of Track B             |
| D — Collections        | `collections.rs` commands + free functions in new `collections_store.rs` or inline in `mod.rs` | Depends on Track A                                     |
| E — Favorites          | `profile_set_favorite` command + `set_profile_favorite` MetadataStore method                   | Depends on Track A; profiles table already has columns |
| F — Usage Insights     | SQL aggregate queries via new `metadata_usage_summary` or similar command                      | Depends on Track A (schema); no new tables needed      |
| G — Frontend           | New hooks + UI components for collections/favorites, insight display                           | Depends on Tracks D, E, F commands being registered    |

Tracks B, C, D, E, F can all start in parallel once Track A is complete. Track G depends on B-F backends.

---

## Implementation Constraints

1. **`platform_tags` storage**: store as space-separated string (`"linux steam-deck"`) not JSON array — better FTS5 tokenization; simpler `LIKE` queries.
2. **`tap_branch NOT NULL DEFAULT ''`**: use empty string, not NULL. The `UNIQUE(tap_url, tap_branch)` index works correctly because both columns are NOT NULL. Never use `COALESCE` workaround.
3. **DELETE+INSERT for `community_profiles`**: never UPSERT for re-index. UPSERT leaves ghost rows when profiles are removed from a tap.
4. **Schema version is v4 (single migration)**: all five Phase 3 tables go into one `migrate_3_to_4` function, not separate migrations.
5. **`profile_set_favorite` goes in `commands/profile.rs`** (modifies a profile record); collection commands go in new `commands/collections.rs` (one-domain-per-file).
6. **FTS5 deferred**: build `community_profiles` table first. Add FTS5 virtual table only if `LIKE` proves insufficient. If added later, FTS5 content table sync requires explicit INSERT-into-FTS on every `community_profiles` INSERT/DELETE — this is the reason to defer.
7. **External cache is infrastructure-only in Phase 3**: no HTTP client, no external calls. Only the store/retrieve/evict API. Future Tauri commands fetch externally, then call `put_cache_entry`.
8. **Usage insights are SQL aggregates over existing `launch_operations`**: no materialized tables, no schema changes needed — just new MetadataStore query methods.
9. **`collection_profiles.position` column**: include for user-defined sort order, even if not exposed in v1 UI.
10. **`community_sync` compile-time contract test** (`community.rs:138-161`): adding `State<'_, MetadataStore>` changes the command signature — update the test's invocation to include the new state parameter or it will fail to compile.
11. **Community index watermark skip**: check `community_taps.last_head_commit` before running DELETE+INSERT. If `stored_head == result.head_commit`, return `Ok(())` immediately. This avoids unnecessary transaction churn on unchanged taps.

---

## New Tauri Commands (register in lib.rs invoke_handler!)

```rust
// In commands/community.rs (modified)
// community_sync — signature change only (add State<MetadataStore>)
// community_list_profiles — optionally add SQLite fast-path

// In commands/collections.rs (new file)
collection_list,
collection_create,
collection_delete,
collection_rename,
collection_add_profile,
collection_remove_profile,
collection_list_profiles,

// In commands/profile.rs (new command)
profile_set_favorite,

// Usage insights (new file or appended to existing)
metadata_query_most_launched,
metadata_query_failure_trends,
```

---

## Key Recommendations for Task Breakdown

1. **Track A first**: `models.rs` → `migrations.rs` → compile check. All other tracks depend on this. Keeping it as one sequential task (not parallelized internally) prevents conflicting DDL edits.
2. **`community_index.rs` is the highest-value deliverable**: watermark skip and catalog indexing enable `community_list_profiles` to serve from SQLite, eliminating the per-call filesystem scan. Prioritize Track B over C, D.
3. **Favorites before collections**: `profile_set_favorite` touches only an existing column in an existing table — minimal surface. Validates the `lookup_profile_id` + `set_profile_favorite` pattern before building the more complex collection membership logic.
4. **One test file per new submodule**: tests in `mod.rs` using `open_in_memory()` + `connection()` helper. Write tests immediately after each submodule compiles — do not batch at end.
5. **Register commands in lib.rs as a separate step**: avoids compile errors while commands are in-flight.
6. **FTS5**: explicitly out of scope for task breakdown. Leave a `// TODO: FTS5 virtual table` comment in `migrate_3_to_4` if needed, but do not create the virtual table.
7. **Frontend work is the natural final track**: all backend APIs must be stable before building `useCollections`, `useUsageInsights`, and the collections panel in `CommunityBrowser.tsx`.
