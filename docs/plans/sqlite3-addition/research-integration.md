# Integration Research: SQLite Metadata Layer Phase 3 — Catalog and Intelligence

All findings are verified against source code as of 2026-03-27 on branch `main` (Phases 1 and 2 merged).

---

## Community Tap Sync Flow

### How `community_sync()` Works

**File:** `src/crosshook-native/src-tauri/src/commands/community.rs:124-131`

```rust
pub fn community_sync(
    settings_store: State<'_, SettingsStore>,
    tap_store: State<'_, CommunityTapStore>,
) -> Result<Vec<CommunityTapSyncResult>, String>
```

Execution path:

1. `load_community_taps()` — reads `AppSettingsData.community_taps: Vec<CommunityTapSubscription>` from TOML settings
2. `tap_store.sync_many(&taps)` — iterates subscriptions, calls `sync_tap()` for each
3. Each `sync_tap()` → `sync_workspace()` → either `clone_tap()` (first time) or `fetch_and_reset()` (subsequent)
4. After git operation: `rev_parse_head()` returns the 40-char commit SHA
5. `index::index_tap(workspace)` — recursive filesystem scan for `community-profile.json` files

**Git operations** (in `taps.rs`):

- Clone: `git clone --branch <branch> --single-branch <url> <local_path>`
- Update: `git fetch --prune origin <branch>` → `git reset --hard FETCH_HEAD` → `git clean -fdx`
- HEAD: `git -C <path> rev-parse HEAD` → returns 40-char SHA string

**Timeout protection:** `GIT_HTTP_LOW_SPEED_LIMIT=1000` and `GIT_HTTP_LOW_SPEED_TIME=30` env vars on all git commands.

**Local workspace path:** `~/.local/share/crosshook/community/taps/<url-slug>[-branch-slug]`

- Slug derived by `slugify()`: lowercases alphanumerics, replaces non-alphanumeric runs with `-`, trims dashes

### Data Produced by a Sync

`CommunityTapSyncResult` (taps.rs:40-46):

```rust
pub struct CommunityTapSyncResult {
    pub workspace: CommunityTapWorkspace,   // subscription + local_path
    pub status: CommunityTapSyncStatus,     // Cloned | Updated
    pub head_commit: String,                // 40-char SHA from git rev-parse HEAD
    pub index: CommunityProfileIndex,       // all parsed community-profile.json entries
}
```

`CommunityProfileIndex` (index.rs:11-15):

```rust
pub struct CommunityProfileIndex {
    pub entries: Vec<CommunityProfileIndexEntry>,
    pub diagnostics: Vec<String>,    // non-fatal issues (parse errors, schema mismatches)
}
```

`CommunityProfileIndexEntry` (index.rs:16-25):

```rust
pub struct CommunityProfileIndexEntry {
    pub tap_url: String,
    pub tap_branch: Option<String>,
    pub tap_path: PathBuf,
    pub manifest_path: PathBuf,         // absolute path to community-profile.json
    pub relative_path: PathBuf,         // path relative to tap root
    pub manifest: CommunityProfileManifest,
}
```

### HEAD Commit as Watermark

The `head_commit` from `CommunityTapSyncResult` is the natural watermark for skip-if-unchanged logic. Phase 3 adds a `community_taps` table to store the last-indexed HEAD commit per tap. On `sync_tap_index()`, if `head_commit == stored_head_commit`, the expensive recursive manifest scan and database upsert can be skipped entirely.

---

## Community Data Structures

### Core Types in crosshook-core

**`CommunityTapSubscription`** (`taps.rs:19-25`) — persisted in settings TOML:

```rust
pub struct CommunityTapSubscription {
    pub url: String,
    pub branch: Option<String>,
}
```

**`CommunityTapWorkspace`** (`taps.rs:27-31`) — transient, not persisted:

```rust
pub struct CommunityTapWorkspace {
    pub subscription: CommunityTapSubscription,
    pub local_path: PathBuf,
}
```

**`CommunityTapStore`** (`taps.rs:93-95`) — stateless wrapper around a filesystem base path:

```rust
pub struct CommunityTapStore {
    base_path: PathBuf,   // ~/.local/share/crosshook/community/taps/
}
```

There is no in-memory cache of sync results. Every call to `index_workspaces()` re-scans the filesystem. Phase 3 SQLite indexing replaces this repeated scan with a DB read for already-indexed taps.

### `community_schema.rs` — The Manifest Format

**File:** `src/crosshook-native/crates/crosshook-core/src/profile/community_schema.rs`

Schema version: `COMMUNITY_PROFILE_SCHEMA_VERSION = 1` (u32)

`CommunityProfileManifest`:

```rust
pub struct CommunityProfileManifest {
    pub schema_version: u32,               // default 1; skip-serialized if == 1
    pub metadata: CommunityProfileMetadata,
    pub profile: GameProfile,
}
```

`CommunityProfileMetadata`:

- `game_name: String`
- `game_version: String`
- `trainer_name: String`
- `trainer_version: String`
- `proton_version: String`
- `platform_tags: Vec<String>`
- `compatibility_rating: CompatibilityRating` — enum: Unknown/Broken/Partial/Working/Platinum
- `author: String`
- `description: String`

All fields default to empty string / empty vec. `CompatibilityRating` defaults to `Unknown`.

**File format:** JSON, filename must be exactly `community-profile.json`. Schema version mismatch produces a diagnostic string rather than an error.

### Re-exports Chain

`community/mod.rs` re-exports from `profile` module:

```rust
pub use crate::profile::{
    CommunityProfileManifest, CommunityProfileMetadata, CompatibilityRating,
    COMMUNITY_PROFILE_SCHEMA_VERSION,
};
```

The manifest types live in `crosshook-core/src/profile/community_schema.rs` and are surfaced through both the `community` and `profile` module paths.

---

## Frontend IPC — Community

### Tauri Commands Invoked

| Frontend call                                  | Tauri command              | Return type                     |
| ---------------------------------------------- | -------------------------- | ------------------------------- |
| `invoke('community_list_profiles')`            | `community_list_profiles`  | `CommunityProfileIndex`         |
| `invoke('community_sync')`                     | `community_sync`           | `Vec<CommunityTapSyncResult>`   |
| `invoke('community_add_tap', { tap })`         | `community_add_tap`        | `Vec<CommunityTapSubscription>` |
| `invoke('community_import_profile', { path })` | `community_import_profile` | `CommunityImportResult`         |

`removeTap` in `useCommunityProfiles.ts` calls `settings_load` / `settings_save` directly rather than a dedicated remove command — no `community_remove_tap` command exists.

### Data Shapes Across IPC

**`CommunityTapSyncResult`** (TypeScript mirror in `useCommunityProfiles.ts:76-82`):

```ts
interface CommunityTapSyncResult {
  workspace: CommunityTapWorkspace;
  status: 'cloned' | 'updated';
  head_commit: string; // 40-char SHA
  index: CommunityProfileIndex;
}
```

**`CommunityProfileIndex`** (TS):

```ts
interface CommunityProfileIndex {
  entries: CommunityProfileIndexEntry[];
  diagnostics: string[];
}
```

**`CommunityProfileIndexEntry`** (TS):

```ts
interface CommunityProfileIndexEntry {
  tap_url: string;
  tap_branch?: string;
  tap_path: string;
  manifest_path: string;
  relative_path: string;
  manifest: CommunityProfileManifest; // full parsed manifest including profile GameProfile
}
```

**Payload size concern:** Each entry carries the full `GameProfile` embedded in the manifest. For large taps (hundreds of profiles), this IPC payload can grow substantially. The existing frontend `community_list_profiles` call re-scans on every invocation; Phase 3 SQLite indexing should store only metadata columns, not the full `GameProfile`, to keep payload sizes predictable.

### Frontend Search (Current — In-Memory)

`CommunityBrowser.tsx` performs client-side search via `matchesQuery()`:

- Concatenates all metadata fields + tap_url + relative_path into a single lowercased string
- Substring match only — no tokenization
- `useMemo` with `useDeferredValue` patterns for deferred filtering (also in `CompatibilityViewer.tsx`)

Phase 3 FTS5 would be a backend-only optimization; the frontend already has a working filter UX.

---

## Profile Favorites/Collections IPC

### Existing Profile IPC

All profile IPC commands (from `lib.rs:109-117` handler registration):

| Command                             | Purpose                                                                        |
| ----------------------------------- | ------------------------------------------------------------------------------ |
| `profile_list`                      | List all profile name stems                                                    |
| `profile_load`                      | Load a `GameProfile` by name                                                   |
| `profile_save`                      | Save a `GameProfile` (calls `observe_profile_write`)                           |
| `profile_delete`                    | Delete profile + cascade launcher cleanup (calls `observe_profile_delete`)     |
| `profile_duplicate`                 | Duplicate with unique copy name (calls `observe_profile_write` with source ID) |
| `profile_rename`                    | Rename + cascade (calls `observe_profile_rename`)                              |
| `profile_import_legacy`             | Import old format (calls `observe_profile_write`)                              |
| `profile_export_toml`               | Serialize to shareable TOML string                                             |
| `profile_save_launch_optimizations` | Targeted update of launch.optimizations only                                   |

The `profiles` table already has `is_favorite INTEGER NOT NULL DEFAULT 0` and `is_pinned INTEGER NOT NULL DEFAULT 0` columns (from Phase 1 migration). They are never written to in Phases 1 or 2 — Phase 3 is when these columns become active.

### New IPC Needed for Collections

Phase 3 needs commands to expose favorites/collections to the frontend. Proposed pattern following existing conventions:

```rust
// Favorites (single toggle, returns updated state)
#[tauri::command]
pub fn profile_set_favorite(name: String, favorite: bool, metadata_store: State<'_, MetadataStore>) -> Result<(), String>

// Collections CRUD
#[tauri::command]
pub fn collection_list(metadata_store: State<'_, MetadataStore>) -> Result<Vec<CollectionSummary>, String>

#[tauri::command]
pub fn collection_create(name: String, metadata_store: State<'_, MetadataStore>) -> Result<CollectionSummary, String>

#[tauri::command]
pub fn collection_delete(collection_id: String, metadata_store: State<'_, MetadataStore>) -> Result<(), String>

#[tauri::command]
pub fn collection_add_profile(collection_id: String, profile_name: String, metadata_store: State<'_, MetadataStore>) -> Result<(), String>

#[tauri::command]
pub fn collection_remove_profile(collection_id: String, profile_name: String, metadata_store: State<'_, MetadataStore>) -> Result<(), String>

#[tauri::command]
pub fn collection_list_profiles(collection_id: String, metadata_store: State<'_, MetadataStore>) -> Result<Vec<String>, String>
```

### Data Shapes for Collections IPC

```rust
// New serde type needed in metadata module or commands
#[derive(Serialize, Deserialize)]
pub struct CollectionSummary {
    pub collection_id: String,   // UUID
    pub name: String,
    pub profile_count: usize,
    pub created_at: String,
    pub updated_at: String,
}
```

The resolution path for `profile_name → profile_id` already exists as `lookup_profile_id()` in `profile_sync.rs:72-86`. New collection commands use this to convert names to stable IDs before writing to `collection_profiles`.

---

## External Metadata Sources

### `CompatibilityViewer.tsx` — Current Data Source

**File:** `src/crosshook-native/src/components/CompatibilityViewer.tsx`

The component is **purely presentational** — it accepts entries as a prop (`entries: CompatibilityDatabaseEntry[]`) and has no Tauri `invoke()` calls of its own. All data is passed in from a parent.

`CompatibilityDatabaseEntry` (TS type in the component):

```ts
interface CompatibilityDatabaseEntry {
  id: string;
  tap_url: string;
  tap_branch?: string | null;
  manifest_path: string;
  relative_path?: string;
  metadata: CompatibilityProfile; // game_name, trainer_name, rating, platform_tags, etc.
}
```

This component is currently fed from community tap index data — there is no separate external API call for compatibility metadata in the existing codebase.

### External API Calls — Current State

**No external HTTP/API calls exist anywhere in the current codebase.** The app is fully offline-first:

- Community profiles come from git repos (user-configured tap URLs)
- Steam discovery reads local filesystem VDF files
- Compatibility data is derived from community manifest metadata
- No ProtonDB API, no external rating services, no CDN asset fetches

### What Phase 3 Could Cache

The `external_cache_entries` table in the feature spec is designed for future external metadata — ProtonDB ratings, Steam store artwork, game metadata enrichment. In Phase 3 as currently scoped, the most natural use is caching the resolved community index data (parsed manifest metadata without full GameProfiles) so `community_list_profiles` reads from SQLite instead of doing a filesystem scan.

### Payload Size Bounds

Phase 1 established the precedent: `MAX_DIAGNOSTIC_JSON_BYTES = 4096` (`models.rs:141`). The same principle applies to external cache payloads. For community metadata, each `CommunityProfileMetadata` struct serializes to roughly 200-500 bytes as JSON. For a tap with 100 profiles, that is ~50KB total — well within SQLite's comfort zone.

For future external metadata (ProtonDB, Steam artwork URLs), individual JSON payloads should be bounded at 16KB per entry to prevent unbounded growth. The `external_cache_entries` table should include a `payload_size` column and a `MAX_CACHE_PAYLOAD_BYTES` constant mirroring the `MAX_DIAGNOSTIC_JSON_BYTES` pattern.

---

## Usage Insights Data Sources

### Existing `launch_operations` Table (from Phase 2, migration v3)

Columns available for analytics queries:

- `operation_id TEXT PRIMARY KEY` — UUID
- `profile_id TEXT` — FK to profiles (nullable — launch without named profile)
- `profile_name TEXT` — denormalized name at launch time (survives renames)
- `launch_method TEXT NOT NULL` — `steam_applaunch` | `proton_run` | `native`
- `status TEXT NOT NULL` — `started` | `succeeded` | `failed` | `abandoned`
- `exit_code INTEGER` — nullable
- `signal INTEGER` — nullable
- `log_path TEXT` — nullable
- `diagnostic_json TEXT` — nullable, max 4KB (nullified if over)
- `severity TEXT` — promoted from DiagnosticReport (`info`/`warning`/`error`)
- `failure_mode TEXT` — promoted from DiagnosticReport
- `started_at TEXT NOT NULL` — RFC3339
- `finished_at TEXT` — nullable (NULL = abandoned/in-progress)

Indexes: `idx_launch_ops_profile_id ON launch_operations(profile_id)`, `idx_launch_ops_started_at ON launch_operations(started_at)`

### Existing `launchers` Table (from Phase 2, migration v3)

Columns:

- `launcher_id TEXT PRIMARY KEY` — UUID
- `profile_id TEXT` — FK to profiles
- `launcher_slug TEXT NOT NULL UNIQUE`
- `display_name TEXT NOT NULL`
- `script_path TEXT NOT NULL`
- `desktop_entry_path TEXT NOT NULL`
- `drift_state TEXT NOT NULL DEFAULT 'unknown'` — `aligned`/`missing`/`moved`/`stale`/`unknown`
- `created_at TEXT NOT NULL`
- `updated_at TEXT NOT NULL`

### Proposed Usage Insights Queries

**Most launched profiles:**

```sql
SELECT profile_name, COUNT(*) as launch_count
FROM launch_operations
WHERE status IN ('succeeded', 'failed')
GROUP BY profile_name
ORDER BY launch_count DESC
LIMIT 10;
```

**Last successful launch per profile:**

```sql
SELECT profile_name, MAX(finished_at) as last_success
FROM launch_operations
WHERE status = 'succeeded'
GROUP BY profile_name;
```

**Failure trends (last 30 days):**

```sql
SELECT
  profile_name,
  COUNT(*) FILTER (WHERE status = 'succeeded') as successes,
  COUNT(*) FILTER (WHERE status = 'failed') as failures,
  GROUP_CONCAT(DISTINCT failure_mode) as failure_modes
FROM launch_operations
WHERE started_at >= datetime('now', '-30 days')
GROUP BY profile_name
HAVING failures > 0
ORDER BY failures DESC;
```

**Most common failure modes:**

```sql
SELECT failure_mode, COUNT(*) as count
FROM launch_operations
WHERE status = 'failed' AND failure_mode IS NOT NULL
GROUP BY failure_mode
ORDER BY count DESC;
```

**Launcher drift summary:**

```sql
SELECT drift_state, COUNT(*) as count
FROM launchers
GROUP BY drift_state;
```

---

## Database Schema (Current v3)

Schema state after migration `2_to_3` in `migrations.rs`. All timestamps are RFC3339 strings. All IDs are UUID v4 strings.

### Table: `profiles`

| Column              | Type    | Constraints                          | Notes                                         |
| ------------------- | ------- | ------------------------------------ | --------------------------------------------- |
| `profile_id`        | TEXT    | PRIMARY KEY                          | UUID v4                                       |
| `current_filename`  | TEXT    | NOT NULL UNIQUE                      | Profile stem name (no `.toml`)                |
| `current_path`      | TEXT    | NOT NULL                             | Absolute TOML file path                       |
| `game_name`         | TEXT    | NULL                                 | From `GameSection.name`                       |
| `launch_method`     | TEXT    | NULL                                 | From `LaunchSection.method`                   |
| `content_hash`      | TEXT    | NULL                                 | SHA256 of TOML-serialized profile             |
| `is_favorite`       | INTEGER | NOT NULL DEFAULT 0                   | Unused in Phases 1-2                          |
| `is_pinned`         | INTEGER | NOT NULL DEFAULT 0                   | Unused in Phases 1-2                          |
| `source_profile_id` | TEXT    | NULL REFERENCES profiles(profile_id) | Duplication lineage                           |
| `deleted_at`        | TEXT    | NULL                                 | Soft-delete tombstone                         |
| `created_at`        | TEXT    | NOT NULL                             | File mtime on initial census; now() on write  |
| `updated_at`        | TEXT    | NOT NULL                             | Last upsert timestamp                         |
| `source`            | TEXT    | NULL (added in v2)                   | `app_write`, `initial_census`, `import`, etc. |

Indexes:

- `idx_profiles_current_filename ON profiles(current_filename)` UNIQUE

### Table: `profile_name_history`

| Column       | Type    | Constraints                              |
| ------------ | ------- | ---------------------------------------- |
| `id`         | INTEGER | PRIMARY KEY AUTOINCREMENT                |
| `profile_id` | TEXT    | NOT NULL REFERENCES profiles(profile_id) |
| `old_name`   | TEXT    | NULL                                     |
| `new_name`   | TEXT    | NOT NULL                                 |
| `old_path`   | TEXT    | NULL                                     |
| `new_path`   | TEXT    | NOT NULL                                 |
| `source`     | TEXT    | NOT NULL                                 |
| `created_at` | TEXT    | NOT NULL                                 |

Indexes:

- `idx_profile_name_history_profile_id ON profile_name_history(profile_id)`

### Table: `launchers`

| Column               | Type | Constraints                     |
| -------------------- | ---- | ------------------------------- |
| `launcher_id`        | TEXT | PRIMARY KEY                     |
| `profile_id`         | TEXT | REFERENCES profiles(profile_id) |
| `launcher_slug`      | TEXT | NOT NULL UNIQUE                 |
| `display_name`       | TEXT | NOT NULL                        |
| `script_path`        | TEXT | NOT NULL                        |
| `desktop_entry_path` | TEXT | NOT NULL                        |
| `drift_state`        | TEXT | NOT NULL DEFAULT 'unknown'      |
| `created_at`         | TEXT | NOT NULL                        |
| `updated_at`         | TEXT | NOT NULL                        |

Indexes:

- `idx_launchers_profile_id ON launchers(profile_id)`
- `idx_launchers_launcher_slug ON launchers(launcher_slug)`

### Table: `launch_operations`

| Column            | Type    | Constraints                     |
| ----------------- | ------- | ------------------------------- |
| `operation_id`    | TEXT    | PRIMARY KEY                     |
| `profile_id`      | TEXT    | REFERENCES profiles(profile_id) |
| `profile_name`    | TEXT    | NULL (denormalized)             |
| `launch_method`   | TEXT    | NOT NULL                        |
| `status`          | TEXT    | NOT NULL DEFAULT 'started'      |
| `exit_code`       | INTEGER | NULL                            |
| `signal`          | INTEGER | NULL                            |
| `log_path`        | TEXT    | NULL                            |
| `diagnostic_json` | TEXT    | NULL (max 4096 bytes or NULL)   |
| `severity`        | TEXT    | NULL                            |
| `failure_mode`    | TEXT    | NULL                            |
| `started_at`      | TEXT    | NOT NULL                        |
| `finished_at`     | TEXT    | NULL                            |

Indexes:

- `idx_launch_ops_profile_id ON launch_operations(profile_id)`
- `idx_launch_ops_started_at ON launch_operations(started_at)`

### SQLite Connection Configuration

From `db.rs`:

- `PRAGMA journal_mode=WAL` (persistent, file-level)
- `PRAGMA foreign_keys=ON`
- `PRAGMA synchronous=NORMAL`
- `PRAGMA busy_timeout=5000`
- `PRAGMA secure_delete=ON`
- `PRAGMA application_id=0x43484B00` (CrossHook file fingerprint)
- File permissions: `0o600` on DB file, `0o700` on parent directory
- Symlink detection: `fs::symlink_metadata()` check before open
- In-memory connections skip WAL and `journal_mode` check uses `"memory"` instead of `"wal"`

---

## Phase 3 Migration (v3→v4)

Proposed DDL additions for migration `3_to_4`. All tables follow the UUID PK, RFC3339 timestamps, soft-delete pattern established in Phases 1 and 2.

```sql
-- Community tap catalog: tracks subscribed taps and their last indexed commit
CREATE TABLE IF NOT EXISTS community_taps (
    tap_id          TEXT PRIMARY KEY,
    url             TEXT NOT NULL,
    branch          TEXT,
    local_path      TEXT NOT NULL,
    last_head_commit TEXT,           -- last `git rev-parse HEAD` result (40 chars)
    last_synced_at  TEXT,
    profile_count   INTEGER NOT NULL DEFAULT 0,
    created_at      TEXT NOT NULL,
    updated_at      TEXT NOT NULL
);
CREATE UNIQUE INDEX IF NOT EXISTS idx_community_taps_url_branch
    ON community_taps(url, COALESCE(branch, ''));

-- Community profiles: indexed manifest metadata (no full GameProfile)
CREATE TABLE IF NOT EXISTS community_profiles (
    profile_id      TEXT PRIMARY KEY,
    tap_id          TEXT NOT NULL REFERENCES community_taps(tap_id),
    relative_path   TEXT NOT NULL,
    manifest_path   TEXT NOT NULL,
    game_name       TEXT,
    game_version    TEXT,
    trainer_name    TEXT,
    trainer_version TEXT,
    proton_version  TEXT,
    platform_tags   TEXT,            -- JSON array: '["linux","steam-deck"]'
    compat_rating   TEXT,            -- 'unknown'|'broken'|'partial'|'working'|'platinum'
    author          TEXT,
    description     TEXT,
    schema_version  INTEGER NOT NULL DEFAULT 1,
    created_at      TEXT NOT NULL,
    updated_at      TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_community_profiles_tap_id
    ON community_profiles(tap_id);
CREATE INDEX IF NOT EXISTS idx_community_profiles_game_name
    ON community_profiles(game_name);
CREATE INDEX IF NOT EXISTS idx_community_profiles_compat_rating
    ON community_profiles(compat_rating);

-- Optional: FTS5 virtual table for full-text search over manifest metadata
-- Only create if SQLITE_ENABLE_FTS5 is available (bundled rusqlite includes it)
CREATE VIRTUAL TABLE IF NOT EXISTS community_profiles_fts USING fts5(
    game_name,
    trainer_name,
    author,
    description,
    platform_tags,
    content='community_profiles',
    content_rowid='rowid'
);

-- External metadata cache: generic key-value store for future external enrichment
CREATE TABLE IF NOT EXISTS external_cache_entries (
    cache_id        TEXT PRIMARY KEY,
    source          TEXT NOT NULL,   -- 'protondb'|'steam_store'|'custom'
    key             TEXT NOT NULL,   -- e.g., steam app_id or game_name
    payload         TEXT,            -- JSON, NULL if over size limit
    payload_size    INTEGER,         -- byte count of original payload
    fetched_at      TEXT NOT NULL,
    expires_at      TEXT,            -- NULL = no expiry
    created_at      TEXT NOT NULL,
    updated_at      TEXT NOT NULL
);
CREATE UNIQUE INDEX IF NOT EXISTS idx_external_cache_source_key
    ON external_cache_entries(source, key);
CREATE INDEX IF NOT EXISTS idx_external_cache_expires_at
    ON external_cache_entries(expires_at);

-- Collections: user-defined profile groupings
CREATE TABLE IF NOT EXISTS collections (
    collection_id   TEXT PRIMARY KEY,
    name            TEXT NOT NULL UNIQUE,
    description     TEXT,
    created_at      TEXT NOT NULL,
    updated_at      TEXT NOT NULL
);

-- Collection membership (join table)
CREATE TABLE IF NOT EXISTS collection_profiles (
    collection_id   TEXT NOT NULL REFERENCES collections(collection_id) ON DELETE CASCADE,
    profile_id      TEXT NOT NULL REFERENCES profiles(profile_id),
    position        INTEGER NOT NULL DEFAULT 0,  -- user-defined sort order
    added_at        TEXT NOT NULL,
    PRIMARY KEY (collection_id, profile_id)
);
CREATE INDEX IF NOT EXISTS idx_collection_profiles_profile_id
    ON collection_profiles(profile_id);
```

### Size Bounds for Cache Payloads

Following the `MAX_DIAGNOSTIC_JSON_BYTES = 4096` pattern from Phase 2:

```rust
// In models.rs additions:
pub const MAX_CACHE_PAYLOAD_BYTES: usize = 16_384;  // 16 KiB per external cache entry
```

---

## Tauri Command Integration Points

### `commands/community.rs` — Where to Hook `sync_tap_index()`

The feature spec says to add `sync_tap_index()` after `community_sync()`. Based on the code:

**Proposed insertion point** — `community_sync()` already returns `Vec<CommunityTapSyncResult>`, each containing `head_commit` and the indexed entries. `sync_tap_index()` should be called by the existing `community_sync` command or as a separate post-sync command:

Option A — inline in `community_sync`:

```rust
#[tauri::command]
pub fn community_sync(
    settings_store: State<'_, SettingsStore>,
    tap_store: State<'_, CommunityTapStore>,
    metadata_store: State<'_, MetadataStore>,     // add this
) -> Result<Vec<CommunityTapSyncResult>, String> {
    let taps = load_community_taps(&settings_store)?;
    let results = tap_store.sync_many(&taps).map_err(map_error)?;

    // Phase 3: index each sync result into SQLite (fail-soft)
    for result in &results {
        if let Err(e) = metadata_store.index_community_tap_result(result) {
            tracing::warn!(%e, tap_url = %result.workspace.subscription.url,
                "community tap index sync failed");
        }
    }

    Ok(results)
}
```

Option B — new standalone command `sync_tap_index()` called explicitly from the frontend after `community_sync`. Less desirable: forces frontend to make two IPC calls and adds a failure surface where the index becomes stale if the second call is never made.

**Recommendation:** Option A, inline, fail-soft. Mirrors the pattern used in `profile_save`, `profile_delete`, `profile_rename`, and `launch_game` where metadata sync is a best-effort step that never blocks the primary operation.

### HEAD Commit Watermark Skip Logic

```rust
// In metadata/community_index.rs
pub fn index_community_tap_result(
    conn: &Connection,
    result: &CommunityTapSyncResult,
) -> Result<(), MetadataStoreError> {
    let url = &result.workspace.subscription.url;
    let branch = result.workspace.subscription.branch.as_deref();
    let head_commit = &result.head_commit;

    // Check stored watermark
    let stored: Option<String> = conn.query_row(
        "SELECT last_head_commit FROM community_taps
         WHERE url = ?1 AND COALESCE(branch, '') = COALESCE(?2, '')",
        params![url, branch],
        |row| row.get(0),
    ).optional()?;

    if stored.as_deref() == Some(head_commit.as_str()) {
        // HEAD unchanged — skip re-indexing
        return Ok(());
    }

    // Full re-index: upsert tap row, replace profile rows for this tap
    // ... (upsert community_taps, DELETE + INSERT community_profiles for tap_id)
    Ok(())
}
```

### New Commands to Register in `lib.rs`

Phase 3 additions to the `invoke_handler!` macro:

```rust
// Community index
commands::community::sync_tap_index,          // or fold into community_sync

// Collections
commands::collections::collection_list,
commands::collections::collection_create,
commands::collections::collection_delete,
commands::collections::collection_rename,
commands::collections::collection_add_profile,
commands::collections::collection_remove_profile,
commands::collections::collection_list_profiles,

// Favorites
commands::profile::profile_set_favorite,

// Usage insights
commands::insights::query_launch_history,
commands::insights::query_most_launched,
```

Collections commands will likely live in a new `src-tauri/src/commands/collections.rs` file following the existing one-file-per-domain pattern.

### `MetadataStore` API Extensions for Phase 3

New public methods needed on `MetadataStore`:

```rust
// community_index.rs additions
pub fn index_community_tap_result(&self, result: &CommunityTapSyncResult) -> Result<(), MetadataStoreError>
pub fn list_community_profiles(&self, tap_url: Option<&str>) -> Result<Vec<CommunityProfileRow>, MetadataStoreError>

// collections
pub fn list_collections(&self) -> Result<Vec<CollectionRow>, MetadataStoreError>
pub fn create_collection(&self, name: &str) -> Result<String, MetadataStoreError>
pub fn delete_collection(&self, collection_id: &str) -> Result<(), MetadataStoreError>
pub fn add_profile_to_collection(&self, collection_id: &str, profile_name: &str) -> Result<(), MetadataStoreError>
pub fn remove_profile_from_collection(&self, collection_id: &str, profile_name: &str) -> Result<(), MetadataStoreError>
pub fn list_profiles_in_collection(&self, collection_id: &str) -> Result<Vec<String>, MetadataStoreError>

// favorites (writes to existing profiles.is_favorite column)
pub fn set_profile_favorite(&self, profile_name: &str, favorite: bool) -> Result<(), MetadataStoreError>
pub fn list_favorite_profiles(&self) -> Result<Vec<String>, MetadataStoreError>

// insights
pub fn query_most_launched(&self, limit: usize) -> Result<Vec<(String, u64)>, MetadataStoreError>
pub fn query_last_success_per_profile(&self) -> Result<Vec<(String, String)>, MetadataStoreError>
pub fn query_failure_trends(&self, days: u32) -> Result<Vec<FailureTrendRow>, MetadataStoreError>
```

### External Cache API

```rust
// cache_store.rs (new file for Phase 3)
pub fn get_cache_entry(&self, source: &str, key: &str) -> Result<Option<String>, MetadataStoreError>
pub fn put_cache_entry(&self, source: &str, key: &str, payload: &str, expires_at: Option<&str>) -> Result<(), MetadataStoreError>
pub fn evict_expired_cache_entries(&self) -> Result<usize, MetadataStoreError>
```

Payload validation (mirrors Phase 2 diagnostic JSON pattern):

- Validate JSON is parseable before insert
- Reject if `payload.len() > MAX_CACHE_PAYLOAD_BYTES` (store NULL + log warning)
- Always store `payload_size` even when payload is NULL

---

## Key Gotchas and Edge Cases

- **`community_sync` return value already has everything needed.** The `CommunityTapSyncResult` already contains `head_commit`, the workspace (url + branch + local_path), and the full index. No additional git calls are needed for Phase 3 indexing; just consume the sync result.

- **`community_list_profiles` does a fresh filesystem scan every call** (via `index_workspaces` → `index_tap` → `collect_manifests`). Phase 3 should add a SQLite-backed fast path for this command when the tap HEAD hasn't changed, falling back to the filesystem scan if SQLite is unavailable.

- **`CompatibilityViewer.tsx` has no IPC calls** — it is currently fed from community browser state. Any "external metadata" integration goes through `CommunityBrowser` or a new top-level component, not `CompatibilityViewer` directly.

- **No external API calls exist** — Phase 3 external cache infrastructure is forward-looking; there are no current external HTTP calls to integrate against.

- **`profiles.is_favorite` and `profiles.is_pinned` already exist in Phase 1 schema** but are never written. Phase 3 activates these columns; no migration DDL change is needed for favorites, only new write paths in `profile_sync.rs` and new Tauri commands.

- **`platform_tags` must be stored as JSON in SQLite.** `Vec<String>` has no native SQLite column type; serialize as `'["linux","steam-deck"]'` and deserialize on read. FTS5 can index this as text.

- **FTS5 content table vs. standalone table.** Using `content='community_profiles'` in FTS5 means FTS5 is a read-only shadow of the base table — updates to `community_profiles` do NOT automatically propagate to the FTS5 index. An explicit `INSERT INTO community_profiles_fts(community_profiles_fts, rowid, ...)` or `REPLACE` must be called after each base table write. Alternatively, use an external content table and manage FTS5 manually. This is a significant implementation gotcha.

- **Unique index on `community_taps(url, COALESCE(branch, ''))`.** SQLite NULL handling: `NULL != NULL` in unique indexes, so two rows with `branch = NULL` would both be allowed without the `COALESCE`. The `COALESCE(branch, '')` expression index avoids duplicate tap rows for taps with no explicit branch.

- **DELETE + INSERT vs UPSERT for community profiles on re-index.** Since a re-sync may remove profiles from a tap (user deletes files from their tap repo), a simple upsert is not sufficient. The correct pattern is: within a transaction, delete all `community_profiles` rows for the tap, then insert the new full index. This ensures removed profiles don't persist as stale rows.
