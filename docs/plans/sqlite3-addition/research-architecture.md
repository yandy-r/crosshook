# Architecture Research: SQLite Metadata Layer Phase 3 — Catalog and Intelligence

## System Overview

CrossHook's metadata layer is a fail-soft SQLite database (`~/.local/share/crosshook/metadata.db`) that provides stable identity, relationships, and history without replacing TOML profiles as canonical. Phases 1 and 2 are complete: `metadata/` has six files, schema is at v3 (three migrations), and all profile/launcher/launch hooks are wired into Tauri command handlers following a strict warn-and-continue pattern. Phase 3 adds community catalog indexing, collections/favorites UX, usage insights queries, and an external metadata cache — all as new files and a migration from v3 to v4+ that the existing migration runner will pick up automatically.

## Current Metadata Module Structure

All files are under `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/metadata/`.

### Files (post-Phase 2)

- `mod.rs` — `MetadataStore` struct (`conn: Option<Arc<Mutex<Connection>>>`, `available: bool`), `with_conn` and `with_conn_mut` fail-soft helpers, all public API methods delegating to submodule free functions
- `db.rs` — `open_at_path()`, `open_in_memory()`, `new_id()` (UUID v4), `configure_connection()` with all PRAGMAs and quick_check validation
- `migrations.rs` — sequential runner: `if version < N { migrate_N_to_M(conn)?; pragma_update(N)? }`. Currently v0→v1→v2→v3. Phase 3 adds `migrate_3_to_4()`.
- `models.rs` — `MetadataStoreError`, `SyncSource`, `LaunchOutcome`, `DriftState`, `MAX_DIAGNOSTIC_JSON_BYTES` (4096), `SyncReport`, `ProfileRow`, `LauncherRow`, `LaunchOperationRow`
- `profile_sync.rs` — free functions: `observe_profile_write`, `observe_profile_rename`, `observe_profile_delete`, `sync_profiles_from_store`, `lookup_profile_id`
- `launcher_sync.rs` — free functions: `observe_launcher_exported`, `observe_launcher_deleted`, `observe_launcher_renamed`
- `launch_history.rs` — free functions: `record_launch_started`, `record_launch_finished`, `sweep_abandoned_operations`

### Public API Pattern

Every public method on `MetadataStore` has this shape:

```rust
pub fn observe_something(&self, ...) -> Result<T, MetadataStoreError> {
    self.with_conn("action gerund", |conn| {
        submodule::free_function(conn, ...)
    })
}
```

`with_conn` no-ops (returns `T::default()`) when `available = false`. `with_conn_mut` is used only when a mutable `Connection` ref is needed (e.g., for `Transaction::new`).

### Current Schema (v3)

- `profiles` (v1): `profile_id TEXT PK`, `current_filename TEXT NOT NULL UNIQUE`, `current_path`, `game_name`, `launch_method`, `content_hash`, `is_favorite INTEGER DEFAULT 0`, `is_pinned INTEGER DEFAULT 0`, `source_profile_id TEXT FK`, `deleted_at`, `created_at`, `updated_at`
- `profile_name_history` (v1): `id AUTOINCREMENT`, `profile_id FK`, `old_name`, `new_name`, `old_path`, `new_path`, `source`, `created_at`
- `launchers` (v3): `launcher_id TEXT PK`, `profile_id FK NULLABLE`, `launcher_slug NOT NULL UNIQUE`, `display_name`, `script_path`, `desktop_entry_path`, `drift_state NOT NULL DEFAULT 'unknown'`, `created_at`, `updated_at`
- `launch_operations` (v3): `operation_id TEXT PK`, `profile_id FK NULLABLE`, `profile_name`, `launch_method`, `status DEFAULT 'started'`, `exit_code`, `signal`, `log_path`, `diagnostic_json` (max 4KB), `severity`, `failure_mode`, `started_at`, `finished_at`

### Dependency Versions

From `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/Cargo.toml`:

- `rusqlite = { version = "0.38", features = ["bundled"] }` — note: spec recommends 0.39.0; current is 0.38
- `uuid = { version = "1", features = ["v4", "serde"] }`
- `chrono = "0.4"` — used for `Utc::now().to_rfc3339()` timestamps
- `sha2 = "0.10"` — for `compute_content_hash()` in profile_sync

## Community Tap Module

Files under `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/community/`:

- `mod.rs` — re-exports from `index`, `taps`, and from `crate::profile` (`CommunityProfileManifest`, `CommunityProfileMetadata`, `CompatibilityRating`, `COMMUNITY_PROFILE_SCHEMA_VERSION`)
- `taps.rs` — `CommunityTapStore`, `CommunityTapSubscription` (`url: String`, `branch: Option<String>`), `CommunityTapWorkspace`, `CommunityTapSyncResult` (includes `head_commit: String` and `index: CommunityProfileIndex`)
- `index.rs` — `CommunityProfileIndex` (`entries: Vec<CommunityProfileIndexEntry>`, `diagnostics: Vec<String>`), `CommunityProfileIndexEntry` (`tap_url`, `tap_branch`, `tap_path`, `manifest_path`, `relative_path`, `manifest: CommunityProfileManifest`)

`CommunityProfileManifest` is defined in `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/profile/community_schema.rs`:

- `schema_version: u32` — `COMMUNITY_PROFILE_SCHEMA_VERSION = 1`
- `metadata: CommunityProfileMetadata` — `game_name`, `game_version`, `trainer_name`, `trainer_version`, `proton_version`, `platform_tags: Vec<String>`, `compatibility_rating: CompatibilityRating`, `author`, `description`
- `profile: GameProfile`

### Tap Data Flow

1. `CommunityTapStore::sync_tap()` clones or fetches the tap git repo under `~/.local/share/crosshook/community/taps/`
2. `rev_parse_head()` returns the HEAD commit SHA string — this is the **watermark** for HEAD commit skip
3. `index::index_tap()` recursively walks the local path looking for `community-profile.json` files
4. Each found manifest produces a `CommunityProfileIndexEntry` with `tap_url`, `tap_path`, `manifest_path`, `relative_path`
5. `CommunityTapSyncResult.head_commit` carries the SHA — Phase 3 must persist this to `community_taps.last_head_commit` to enable skip-if-unchanged

### Key Gotcha: No HEAD Tracking Yet

`CommunityTapSyncResult.head_commit` is returned but never persisted anywhere. The watermark skip requires Phase 3 to store this in SQLite and compare before re-indexing. The field is already populated by `taps.rs:179`.

### Key Gotcha: index.rs Does Not Validate Size

`collect_manifests()` reads every `community-profile.json` without size bounds. Security finding A6 from the spec requires length validation before inserting manifest fields into SQLite (game_name <= 512B, description <= 4KB).

## Community Tauri Commands

File: `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src-tauri/src/commands/community.rs`

| Command                    | Signature                                                                                                                | Notes                                                                        |
| -------------------------- | ------------------------------------------------------------------------------------------------------------------------ | ---------------------------------------------------------------------------- |
| `community_add_tap`        | `(tap: CommunityTapSubscription, settings_store: State<SettingsStore>) -> Result<Vec<CommunityTapSubscription>, String>` | Persists tap list to settings.toml; no metadata hook                         |
| `community_list_profiles`  | `(settings_store, tap_store) -> Result<CommunityProfileIndex, String>`                                                   | Calls `index_workspaces()` on disk; no SQLite read yet                       |
| `community_import_profile` | `(path: String, profile_store, settings_store, tap_store) -> Result<CommunityImportResult, String>`                      | Validates path is inside a tap workspace; no metadata hook                   |
| `community_sync`           | `(settings_store, tap_store) -> Result<Vec<CommunityTapSyncResult>, String>`                                             | Returns `head_commit` per tap; **Phase 3 hook point** for `sync_tap_index()` |

The spec (files-to-modify list) identifies `community_sync` as the Phase 3 hook point: after `sync_many()`, call a new `sync_tap_index(results, metadata_store)` that persists the index to `community_taps` and `community_profiles` tables with the HEAD watermark.

`community_list_profiles` is a second Phase 3 integration point with a two-tier fast path: (1) if `MetadataStore` is available AND all subscribed taps have a `last_head_commit` row in `community_taps`, serve `community_profiles` rows from SQLite; (2) otherwise fall back to `index_workspaces()` full disk scan. This keeps first-run and degraded-mode behavior identical to current. Both `community_sync` and `community_list_profiles` need `State<'_, MetadataStore>` added.

### State Injection Gap

`community.rs` currently takes `State<SettingsStore>` and `State<CommunityTapStore>` but **not** `State<MetadataStore>`. Phase 3 must add `State<MetadataStore>` to `community_sync` (and optionally `community_list_profiles`). The `MetadataStore` is already registered via `.manage(metadata_store)` in `lib.rs:80`.

## Frontend Community Integration

### useCommunityProfiles.ts

File: `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/hooks/useCommunityProfiles.ts`

- Maintains `taps: CommunityTapSubscription[]` and `index: CommunityProfileIndex` in local React state
- On mount: loads `settings_load` to populate `taps`, then calls `refreshProfiles()` which invokes `community_list_profiles`
- `syncTaps()` calls `community_sync` then `refreshProfiles()` — the Phase 3 hook in `community_sync` on the backend will be transparent to this hook
- **No SQLite-facing types** exist in the frontend yet; community data is always fresh from the backend IPC response

### CommunityBrowser.tsx

File: `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/components/CommunityBrowser.tsx`

- In-browser search is done via `matchesQuery()` at line 30: a concatenated-haystack `includes()` check. This is the current "search" — no backend FTS, no SQLite.
- Filter by `ratingFilter` and `query` is entirely client-side on `index.entries`
- Sort is client-side in `sortProfiles()` by compatibility rating then game name
- The "Import" button calls `community_import_profile` with `entry.manifest_path`

**Phase 3 FTS impact**: if FTS5 is added, `community_list_profiles` would accept a query parameter and return filtered results from SQLite, replacing the current client-side `matchesQuery()`. The component would need a debounced search that invokes a new IPC command instead of filtering locally.

**Collections/Favorites UI impact**: the profile grid cards have no favorite/collection actions yet. Adding them requires new IPC commands (e.g., `community_toggle_favorite`, `collections_add`) and new state in `useCommunityProfiles`.

## Favorites/Collections Integration Points

### Existing Profile Favorites Infrastructure

The `profiles` table already has `is_favorite INTEGER NOT NULL DEFAULT 0` and `is_pinned INTEGER NOT NULL DEFAULT 0` columns (migration v1, `mod.rs` tests verify these columns exist). However:

- No `MetadataStore` API method exposes `set_favorite` or `set_pinned`
- No Tauri command exposes these to the frontend
- No frontend component renders or toggles them

Phase 3 collections are a new concept (`collections` + `collection_profiles` tables) but the `is_favorite`/`is_pinned` flags are already schema-ready for Phase 3 to wire up.

### Profile Listing in Frontend

`useProfile.ts` lists profiles via `invoke<string[]>('profile_list')` which delegates to `ProfileStore::list()` — a pure TOML directory scan. The list returns filename stems only (no metadata). Phase 3 favorites/pins would need either:

1. A new `profile_list_with_metadata` command that joins TOML names with SQLite `is_favorite`/`is_pinned`
2. Or a separate `metadata_get_profile_flags` command invoked after `profile_list`

### Collections Schema (Phase 3)

From the feature spec, Phase 3 adds:

- `collections` table: `collection_id TEXT PK`, `name TEXT NOT NULL`, `description TEXT`, `created_at`, `updated_at`
- `collection_profiles` table: `collection_id FK`, `profile_id FK`, `added_at TEXT NOT NULL` (many-to-many)

Collections are local-only in Phase 3 but designed with future export semantics. They are profile-centric (keyed by `profile_id` from the `profiles` table) not community-centric.

## External Cache Architecture

### What Would Be Cached

Phase 3's `external_cache_entries` table is designed to cache external metadata lookups — the spec does not define a specific external API to call, but the typical use case is:

- ProtonDB compatibility reports
- SteamGrid artwork/banners
- Any future community-sourced metadata beyond what tap manifests contain

The spec's security finding W3 defines bounds: 512 KB max per cache entry, 4 KB per diagnostic summary.

### Current Caching Patterns

There is **no existing external metadata fetch** in the codebase. Community data comes entirely from local git clones. The cache_store.rs file does not exist yet. There is no HTTP client dependency in crosshook-core currently.

### cache_store.rs Integration Points

The new `cache_store.rs` module would:

1. Be invoked from Tauri commands (likely new commands: `cache_fetch_metadata`, or embedded in existing commands like `community_list_profiles`)
2. Use `with_conn` pattern for all SQLite operations
3. Require a `MetadataStore` API method like `get_cached_entry(url, max_age)` and `set_cached_entry(url, payload, expires_at)`
4. Payload validation: check byte length before INSERT (`payload.len() <= 512 * 1024`)
5. No HTTP client exists in crosshook-core — **external fetch belongs in the Tauri command layer, not in `cache_store.rs`**. The MetadataStore never initiates I/O; it only receives data and writes to SQLite. This matches the existing pattern: `commands/launch.rs` coordinates process execution and passes the `DiagnosticReport` down to `record_launch_finished()`. A future external metadata fetch follows the same boundary: Tauri command fetches and validates, then calls `metadata_store.put_cache_entry(source_url, key, payload, expires_at)`. No new Cargo dependency needed in crosshook-core.

## Usage Insights Architecture

### Available Data Sources

From the current `launch_operations` table (Phase 2):

- `profile_name TEXT` — which profile was launched (denormalized for resilience to deletes)
- `profile_id TEXT FK NULLABLE` — stable identity linkage
- `launch_method TEXT` — `steam_applaunch`, `proton_run`, `native`
- `status TEXT` — `started`, `succeeded`, `failed`, `abandoned`
- `exit_code INTEGER`, `signal INTEGER`
- `severity TEXT`, `failure_mode TEXT` — promoted from DiagnosticReport
- `started_at TEXT`, `finished_at TEXT`

### SQL Aggregate Query Patterns

The spec's "Projection Rule" says insights are derived via SQL aggregates, not materialized tables. Example queries:

```sql
-- Most launched profiles (last 30 days)
SELECT profile_name, COUNT(*) as launch_count
FROM launch_operations
WHERE started_at >= datetime('now', '-30 days')
  AND status IN ('succeeded', 'failed')
GROUP BY profile_name
ORDER BY launch_count DESC
LIMIT 10;

-- Success rate per profile
SELECT profile_name,
  COUNT(*) FILTER (WHERE status = 'succeeded') as successes,
  COUNT(*) FILTER (WHERE status = 'failed') as failures
FROM launch_operations
WHERE status != 'abandoned'
GROUP BY profile_name;

-- Last launch outcome per profile
SELECT profile_name, status, failure_mode, started_at
FROM launch_operations
WHERE (profile_name, started_at) IN (
  SELECT profile_name, MAX(started_at) FROM launch_operations GROUP BY profile_name
);
```

### UI Surface for Insights

No insights UI exists yet. The feature spec lists "usage insights queries" as a Phase 3 task without prescribing the UI. Integration points:

1. New Tauri commands (e.g., `metadata_usage_summary`) returning structured data
2. New frontend hook (e.g., `useUsageInsights`) calling those commands
3. Display in `ProfileEditor.tsx` (per-profile stats panel) or a new tab/section in `App.tsx`

The `ConsoleView.tsx` component already renders launch log output — a natural sibling for a "Launch History" expandable panel.

## FTS5 Integration Points

### Current Search

`CommunityBrowser.tsx:30-51` implements client-side search: concatenates all text fields into a haystack string and uses `String.includes()`. This works for small catalogs but degrades at scale.

### FTS5 Availability

The `bundled` feature of `rusqlite` includes FTS5 (SQLite `SQLITE_ENABLE_FTS5` is set in the bundled build). No additional Cargo dependency needed.

### Text Data That Would Benefit

From `CommunityProfileMetadata`:

- `game_name`, `game_version`, `trainer_name`, `trainer_version`, `proton_version`, `author`, `description`
- `platform_tags` (stored as a JSON array or space-separated string in the FTS5 virtual table)

From `community_profiles` (Phase 3 table):

- All metadata fields above, indexed in an FTS5 virtual table

### FTS5 Schema Pattern

```sql
CREATE VIRTUAL TABLE IF NOT EXISTS community_profiles_fts USING fts5(
    game_name,
    trainer_name,
    author,
    description,
    platform_tags,
    content='community_profiles',
    content_rowid='rowid'
);
```

The content table approach keeps the FTS index synchronized with `community_profiles`. Triggers on INSERT/UPDATE/DELETE maintain the FTS index.

### FTS5 Integration Decision

Per the spec: "defer unless proven necessary; `LIKE` queries sufficient for v1." The current client-side search is equivalent to `LIKE` query coverage. Phase 3 should implement FTS5 as optional — build the `community_profiles` table first, then add FTS5 virtual table and triggers only if query performance proves insufficient with `LIKE '%query%'` on a real-world catalog.

## Key Dependencies

### Existing (No New Cargo Dependencies Needed for Core Phase 3)

- `rusqlite 0.38` with `bundled` — provides SQLite + FTS5 already
- `serde_json` — for manifest payload serialization in cache
- `chrono` — for `expires_at` timestamps in cache entries
- `sha2` — could be reused for cache key hashing (URL → hash → lookup)
- `uuid` — for `collection_id`, `cache_entry_id` PKs

### Potentially New

- HTTP client (e.g., `ureq`) if `cache_store.rs` makes outbound requests inside crosshook-core. Currently no HTTP client exists in crosshook-core. If external fetch stays in the Tauri command layer (recommended), this can be deferred.

### Internal Module Dependencies for Phase 3

- `community_index.rs` depends on: `db::new_id()`, `MetadataStoreError`, `CommunityTapSyncResult` (from `crosshook_core::community`), `CommunityProfileManifest`
- `cache_store.rs` depends on: `db::new_id()`, `MetadataStoreError`, `chrono`, `serde_json`
- Both new files follow the same free-function pattern as `profile_sync.rs`, `launcher_sync.rs`, `launch_history.rs`

### Tauri State Already Available

`lib.rs` already manages: `ProfileStore`, `SettingsStore`, `RecentFilesStore`, `CommunityTapStore`, `MetadataStore`. Phase 3 does not need new Tauri-managed state — `MetadataStore` is already available as `State<MetadataStore>` in any command that declares it.

## Phase 3 New Files

Per the feature spec:

- `src/crosshook-native/crates/crosshook-core/src/metadata/community_index.rs` — tap/catalog indexing, HEAD watermark check, `community_taps` + `community_profiles` table writes
- `src/crosshook-native/crates/crosshook-core/src/metadata/cache_store.rs` — external metadata cache, payload size validation, TTL-based expiry

## Phase 3 Files to Modify

- `metadata/mod.rs` — add new public API methods: `index_community_tap()`, `list_community_profiles()` (with optional query), `get_collection()`, `create_collection()`, etc.
- `metadata/migrations.rs` — add `migrate_3_to_4()` for `community_taps`, `community_profiles`, `external_cache_entries`, `collections`, `collection_profiles`
- `src-tauri/src/commands/community.rs` — add `State<MetadataStore>` to `community_sync`; call `sync_tap_index()` after `sync_many()`; optionally add SQLite read path in `community_list_profiles`
- `metadata/models.rs` — add row structs for new tables if needed

## Relevant Docs

- `/home/yandy/Projects/github.com/yandy-r/crosshook/docs/plans/sqlite3-addition/feature-spec.md` — Phase 3 task list (lines 524-534), Phase 3 schema additions, security findings A6/W3/W6
- `/home/yandy/Projects/github.com/yandy-r/crosshook/docs/plans/sqlite3-addition/shared.md` — Phase 2 patterns reference; all patterns apply to Phase 3 unchanged
- `/home/yandy/Projects/github.com/yandy-r/crosshook/docs/plans/sqlite3-addition/research-patterns.md` — with_conn pattern, free function shape, migration pattern, test patterns
- `/home/yandy/Projects/github.com/yandy-r/crosshook/docs/plans/sqlite3-addition/research-security.md` — W3 (512KB payload bound), A6 (validate string lengths before INSERT), W6 (re-validate stored paths)
- `/home/yandy/Projects/github.com/yandy-r/crosshook/docs/plans/sqlite3-addition/research-technical.md` — type-to-table mapping for Phase 3 (lines 48–84): `CommunityTapSubscription` → `community_taps`, `CommunityProfileIndexEntry` → `community_profiles`; authority boundary matrix
- `/home/yandy/Projects/github.com/yandy-r/crosshook/docs/plans/sqlite3-addition/analysis-context.md` — Phase 2 data flow and integration points; Phase 3 follows the same hook-in-command pattern
- <https://www.sqlite.org/fts5.html> — FTS5 virtual table reference
- <https://www.sqlite.org/json1.html> — JSON storage for `platform_tags` array and cache payloads
