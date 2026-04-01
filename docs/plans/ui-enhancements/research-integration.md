# Integration Research: UI Enhancements — APIs, Database, and External Services

## Overview

This document covers the concrete integration surface for the Profiles page restructuring and game metadata/cover art feature (GitHub #52). The ProtonDB lookup is the canonical model for every new external integration: cache-first via `external_cache_entries`, stale fallback on network failure, `MetadataStore` as the single SQLite access point. The new Steam metadata and image caching integrations mirror this pattern exactly, with image binaries going to the filesystem instead of the 512 KiB–capped JSON cache. All Tauri IPC commands follow the `snake_case` naming convention and accept `State<'_, MetadataStore>` for database access.

## Relevant Files

### Backend (Rust)

- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/protondb/client.rs`: Canonical cache-first external API client — the exact model for `steam_metadata/client.rs`
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/protondb/models.rs`: ProtonDB result/state/cache types — mirror structure for Steam metadata types
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/metadata/mod.rs`: `MetadataStore` — `Arc<Mutex<Connection>>` wrapper; exposes `put_cache_entry`, `get_cache_entry`, `with_sqlite_conn`
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/metadata/cache_store.rs`: `put_cache_entry` / `get_cache_entry` / `evict_expired_cache_entries` — upsert pattern with 512 KiB payload cap
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/metadata/migrations.rs`: Schema migration runner — `run_migrations` with sequential `if version < N` guards; current version: **13**
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/metadata/models.rs`: `MetadataStoreError`, `MAX_CACHE_PAYLOAD_BYTES` (524288), shared row types
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/settings/mod.rs`: `AppSettingsData` with `#[serde(default)]` — new `steamgriddb_api_key` field slots in cleanly
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/Cargo.toml`: `reqwest` (0.12, rustls-tls), `rusqlite` (0.39, bundled) — no new HTTP/DB dependencies needed; `infer` (~0.16) is the one new crate
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src-tauri/src/commands/protondb.rs`: Minimal Tauri command — clone `MetadataStore`, delegate to core, return result directly
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src-tauri/src/commands/steam.rs`: Existing Steam commands (proton discovery, auto-populate) — new `steam_metadata` commands live here or in a separate `commands/game_metadata.rs`
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src-tauri/src/commands/mod.rs`: Command module re-exports — new module must be added here and registered in `lib.rs`
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src-tauri/src/lib.rs`: Tauri builder — `manage(metadata_store)` already present; new commands added to `invoke_handler!` list
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src-tauri/src/commands/settings.rs`: Settings load/save commands — thin wrappers, `settings_load` / `settings_save` pattern for `AppSettingsData`

### Tauri Configuration

- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src-tauri/tauri.conf.json`: Current CSP: `default-src 'self'; script-src 'self'` — must add `img-src 'self' asset: http://asset.localhost`
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src-tauri/capabilities/default.json`: Current permissions: `core:default`, `dialog:default`, `shell:allow-open` (protondb.com) — must add `fs:allow-read-file` scoped to cache images and asset protocol scope

### Frontend (TypeScript/React)

- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/hooks/useProtonDbLookup.ts`: The canonical frontend hook pattern — request deduplication via `requestIdRef`, `loading` state, `invoke()` call, `unavailable` on error
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/types/protondb.ts`: ProtonDB TypeScript types — mirrors Rust serde output; `snake_case` field names from Serde's `rename_all`
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/types/settings.ts`: Frontend `AppSettingsData` interface — must add `steamgriddb_api_key?: string | null`

## Architectural Patterns

### Tauri IPC Command Registration

Commands are defined with `#[tauri::command]` in `src-tauri/src/commands/`, declared `pub` in `commands/mod.rs`, and registered in `lib.rs`'s `invoke_handler!(tauri::generate_handler![...])` macro. State is injected via `tauri::State<'_, T>` parameters. Async commands use `tauri::async_runtime::spawn_blocking` when the underlying function is synchronous, or `async fn` directly for async operations.

**Pattern for new command (async, MetadataStore)**:

```rust
// src-tauri/src/commands/game_metadata.rs
#[tauri::command]
pub async fn fetch_game_metadata(
    app_id: String,
    metadata_store: State<'_, MetadataStore>,
) -> Result<SteamMetadataLookupResult, String> {
    let metadata_store = metadata_store.inner().clone();
    Ok(lookup_steam_metadata(&metadata_store, &app_id).await)
}
```

The `protondb_lookup` command at `commands/protondb.rs:6-13` is the exact template.

### MetadataStore Access Pattern

`MetadataStore` wraps `Arc<Mutex<Connection>>` (see `metadata/mod.rs:39-42`). External callers use:

- `metadata_store.put_cache_entry(source_url, cache_key, payload, expires_at)` — upsert with 512 KiB cap
- `metadata_store.get_cache_entry(cache_key)` — returns `Option<String>` (valid/non-expired only)
- `metadata_store.with_sqlite_conn(action, |conn| { ... })` — for custom queries not in the pre-built methods (used by ProtonDB's `load_cached_lookup_row` for the stale fallback)

The `with_conn` / `with_conn_mut` internal methods silently return `T::default()` when the store is unavailable — the store is designed to degrade gracefully.

### Cache-First External API Pattern (ProtonDB as Model)

The complete cache-first flow from `protondb/client.rs`:

1. **Normalize input**: `normalize_app_id(app_id)` — trim and reject empty strings
2. **Valid cache hit**: Query `external_cache_entries WHERE cache_key = ?1 AND expires_at > now` → deserialize and return with `from_cache: true, is_stale: false`
3. **Live fetch**: HTTP request via `OnceLock<reqwest::Client>` with `timeout(6s)` and `User-Agent: CrossHook/{version}`
4. **Persist**: `metadata_store.put_cache_entry(source_url, cache_key, payload_json, expires_at)` — upsert on `cache_key` conflict
5. **Network failure fallback**: Query `external_cache_entries WHERE cache_key = ?1` (no expiry check) → return with `is_stale: true`
6. **Total failure**: Return `state: Unavailable`

**Cache key convention**: `protondb:{app_id}` (namespace:id). Steam metadata should use `steam:appdetails:v1:{app_id}` (feature spec, consistent with this pattern).

**TTL**: ProtonDB uses 6-hour TTL (`CACHE_TTL_HOURS = 6`). Steam metadata spec calls for 24-hour TTL.

### HTTP Client Initialization

`OnceLock<reqwest::Client>` singleton per module (see `protondb/client.rs:26`). Build with:

- `.timeout(Duration::from_secs(REQUEST_TIMEOUT_SECS))`
- `.user_agent(format!("CrossHook/{}", env!("CARGO_PKG_VERSION")))`
- `.default-features = false, features = ["json", "rustls-tls"]` (no native-tls)

New clients for Steam metadata and image download should each use their own `OnceLock` in their respective modules.

### Schema Migration Pattern

`metadata/migrations.rs:run_migrations` uses sequential `if version < N` guards — not `else if`. The `user_version` PRAGMA is updated after each migration block. To add migration v14:

```rust
// In run_migrations(), after the version < 13 block:
if version < 14 {
    migrate_13_to_14(conn)?;
    conn.pragma_update(None, "user_version", 14_u32)
        .map_err(|source| MetadataStoreError::Database {
            action: "set user_version to 14",
            source,
        })?;
}
```

All migrations use `conn.execute_batch(sql)` returning `MetadataStoreError::Database { action: "run metadata migration N to M", source }`. The current schema version is 13 (added `trainer_hash_cache`, `offline_readiness_snapshots`, `community_tap_offline_state` in `migrate_12_to_13`).

### Frontend Hook Pattern

`useProtonDbLookup.ts` is the canonical model for `useGameMetadata` and `useGameCoverArt`:

- **Request deduplication**: `useRef(0)` counter incremented per call; stale responses discarded by comparing `requestId !== requestIdRef.current`
- **Loading state**: `useState(false)` set before `invoke()`, cleared in `finally`
- **Error handling**: `console.error` + return `unavailable` state (never throw to caller)
- **Re-fetch on ID change**: `useEffect([normalizedAppId, runLookup])` triggers automatically
- **Refresh**: Exposed as `() => Promise<void>` callback (`forceRefresh: true`)

`invoke<T>('command_name', { camelCaseParam })` — Tauri serializes camelCase JS args to snake_case Rust args automatically.

### AppSettingsData Extension Pattern

`AppSettingsData` uses `#[serde(default)]` at struct level (`settings/mod.rs:22`). Adding a new optional field:

```rust
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, Default)]
#[serde(default)]
pub struct AppSettingsData {
    // existing fields...
    pub steamgriddb_api_key: Option<String>,
}
```

No migration required — `serde(default)` means existing `settings.toml` files without the field deserialize to `None`. The frontend `AppSettingsData` interface in `types/settings.ts` must be updated in parallel.

## Database Schema

### Current Schema (v13) — Relevant Tables

#### `external_cache_entries` — Steam Metadata JSON Storage

```sql
CREATE TABLE external_cache_entries (
    cache_id        TEXT PRIMARY KEY,
    source_url      TEXT NOT NULL,
    cache_key       TEXT NOT NULL UNIQUE,  -- "steam:appdetails:v1:{app_id}"
    payload_json    TEXT,                  -- NULL if > 512 KiB
    payload_size    INTEGER NOT NULL DEFAULT 0,
    fetched_at      TEXT NOT NULL,
    expires_at      TEXT,                  -- RFC3339; NULL = never expires
    created_at      TEXT NOT NULL,
    updated_at      TEXT NOT NULL
);
```

**Upsert behavior**: `ON CONFLICT(cache_key) DO UPDATE SET ...` — updating an existing entry refreshes all fields. The 512 KiB (`MAX_CACHE_PAYLOAD_BYTES = 524_288`) cap stores `NULL` in `payload_json` with a warning log; this prevents oversized remote payloads from bloating the database. Steam metadata JSON is 3–15 KiB, well within the cap.

#### `profiles` — Profile Registry (v1, extended through v13)

```sql
CREATE TABLE profiles (
    profile_id          TEXT PRIMARY KEY,
    current_filename    TEXT NOT NULL UNIQUE,
    current_path        TEXT NOT NULL,
    game_name           TEXT,
    launch_method       TEXT,
    content_hash        TEXT,
    is_favorite         INTEGER NOT NULL DEFAULT 0,
    source_profile_id   TEXT REFERENCES profiles(profile_id),
    deleted_at          TEXT,
    source              TEXT,   -- added in v2
    created_at          TEXT NOT NULL,
    updated_at          TEXT NOT NULL
);
```

`steam_app_id` is not stored in the `profiles` table — it lives in the TOML profile file. The `game_image_cache` table joins to `steam_app_id` from the TOML, not a FK to `profiles`.

#### `version_snapshots` — Contains `steam_app_id` Reference (v9)

```sql
CREATE TABLE version_snapshots (
    id                  INTEGER PRIMARY KEY AUTOINCREMENT,
    profile_id          TEXT NOT NULL REFERENCES profiles(profile_id) ON DELETE CASCADE,
    steam_app_id        TEXT NOT NULL DEFAULT '',
    -- ...
);
CREATE INDEX idx_version_snapshots_steam_app_id ON version_snapshots(steam_app_id);
```

This is the existing pattern for `steam_app_id` in the metadata DB — the new `game_image_cache` table follows the same approach (store the raw string, no FK to an app-id registry table).

### New Schema (v14) — `game_image_cache` Table

```sql
CREATE TABLE IF NOT EXISTS game_image_cache (
    cache_id         TEXT PRIMARY KEY,         -- uuid::Uuid::new_v4().to_string()
    steam_app_id     TEXT NOT NULL,            -- numeric string, validated before insert
    image_type       TEXT NOT NULL DEFAULT 'cover',     -- 'cover' | 'hero' | 'capsule'
    source           TEXT NOT NULL DEFAULT 'steam_cdn', -- 'steam_cdn' | 'steamgriddb'
    file_path        TEXT NOT NULL,            -- absolute path to cached image file
    file_size        INTEGER NOT NULL DEFAULT 0,
    content_hash     TEXT NOT NULL DEFAULT '',
    mime_type        TEXT NOT NULL DEFAULT 'image/jpeg',
    width            INTEGER,
    height           INTEGER,
    source_url       TEXT NOT NULL DEFAULT '', -- original download URL
    preferred_source TEXT NOT NULL DEFAULT 'auto',
    expires_at       TEXT,                     -- RFC3339 TTL (24h)
    fetched_at       TEXT NOT NULL,
    created_at       TEXT NOT NULL,
    updated_at       TEXT NOT NULL
);
CREATE UNIQUE INDEX IF NOT EXISTS idx_game_image_cache_app_type_source
    ON game_image_cache(steam_app_id, image_type, source);
CREATE INDEX IF NOT EXISTS idx_game_image_cache_expires
    ON game_image_cache(expires_at);
```

**Relationship to `external_cache_entries`**: `external_cache_entries` stores Steam metadata JSON (cache key `steam:appdetails:v1:{app_id}`). `game_image_cache` stores filesystem paths to image binaries. Both are keyed by `steam_app_id` but have no FK relationship — independent caches with independent TTLs.

**Filesystem path convention**: `~/.local/share/crosshook/cache/images/{steam_app_id}/cover_steam_cdn.jpg`

**ID generation**: Same as all other primary keys — `db::new_id()` which calls `uuid::Uuid::new_v4().to_string()`.

### Connection Configuration (Applied to All Connections)

From `metadata/db.rs:68-105`, all connections configure:

- `PRAGMA journal_mode=WAL` (WAL for file connections, MEMORY for in-memory)
- `PRAGMA foreign_keys=ON`
- `PRAGMA synchronous=NORMAL`
- `PRAGMA busy_timeout=5000`
- `PRAGMA secure_delete=ON`
- `PRAGMA application_id=0x43484B00` (CrossHook identifier)
- `PRAGMA quick_check` (validated on open — returns error if not "ok")
- Symlink detection before open (refuses to open symlinked database files)
- Directory created with `0o700`, file set to `0o600` permissions

## External Services

### Steam Store API

- **Endpoint**: `GET https://store.steampowered.com/api/appdetails?appids={steam_app_id}`
- **Authentication**: None required
- **Rate limits**: Undocumented; community-reported ~200 req/min
- **Response shape** (relevant fields):

  ```json
  {
    "{app_id}": {
      "success": true,
      "data": {
        "steam_appid": 1245620,
        "name": "Elden Ring",
        "short_description": "...",
        "header_image": "https://cdn.akamai.steamstatic.com/steam/apps/1245620/header.jpg",
        "genres": [{ "id": "1", "description": "Action" }]
      }
    }
  }
  ```

- **Phase 2 image target**: `header_image` field (460x215 JPEG, landscape)
- **Steam CDN direct URLs** (no API call needed):
  - `https://cdn.cloudflare.steamstatic.com/steam/apps/{id}/library_600x900.jpg` (portrait)
  - `https://cdn.cloudflare.steamstatic.com/steam/apps/{id}/library_hero.jpg` (hero)

### ProtonDB API (Existing — Canonical Model)

The ProtonDB client (`protondb/client.rs`) demonstrates the full cache-first external API integration. Key implementation details not obvious from types:

- **`OnceLock` HTTP client**: Built once on first request, reused for all subsequent requests — see `protondb_http_client()` at line 175
- **Stale fallback**: `load_cached_lookup_row(allow_expired=true)` — a separate SQL query without the `expires_at > now` constraint
- **Degraded recommendations**: Network failure for the report feed does not fail the whole lookup — tier summary is returned with a degraded `recommendation_groups` containing a user-facing message
- **Cache persistence**: Only called after a successful live fetch; stale results are never re-persisted

### Image Download and Validation (New — Phase 2)

The `game_images/client.rs` module must:

1. Accept a URL from the validated allowlist (Steam CDN or SteamGridDB CDN)
2. Download bytes with `reqwest` (streaming to avoid full memory load for large images)
3. Validate magic bytes using `infer` crate — reject SVG (no magic bytes), allow JPEG/PNG/WEBP only
4. Enforce 5 MB file size cap before writing to disk (advisory A1)
5. Write to `~/.local/share/crosshook/cache/images/{steam_app_id}/{image_type}_{source}.{ext}`
6. Return absolute path on success

**Path traversal protection**: `steam_app_id` must be validated as numeric-only before constructing any filesystem path (use `steam_app_id.chars().all(|c| c.is_ascii_digit())`). After construction, use `canonicalize` + prefix assertion to verify the result path is under the cache directory.

## Image Caching Infrastructure

### Tauri Asset Protocol (Required for Rendering)

To render local files in the Tauri webview, `convertFileSrc` from `@tauri-apps/api/core` converts an absolute path to an `asset://localhost/...` URL. This requires:

**`tauri.conf.json` CSP change**:

```json
"csp": "default-src 'self'; script-src 'self'; img-src 'self' asset: http://asset.localhost"
```

**`capabilities/default.json` additions**:

```json
{
  "identifier": "fs:allow-read-file",
  "allow": [{ "path": "$LOCALDATA/crosshook/cache/images/**" }]
}
```

Plus the asset protocol scope permission. The feature spec references `$LOCALDATA/cache/images/**` — the actual `MetadataStore` path uses `BaseDirs::new().data_local_dir().join("crosshook/...")` which resolves to `~/.local/share/crosshook/` on Linux (`XDG_DATA_HOME`).

### Cache Directory Initialization

`MetadataStore::try_new()` uses `BaseDirs::new().data_local_dir().join("crosshook/metadata.db")`, placing the DB at `~/.local/share/crosshook/metadata.db`. The image cache should use the same base: `~/.local/share/crosshook/cache/images/`. This directory must be created on first use (like `db.rs` creates the DB directory with `create_dir_all`).

### Frontend Image Rendering Pattern

```typescript
import { convertFileSrc } from '@tauri-apps/api/core';

// path comes from fetch_game_cover_art IPC result
const imageUrl = path ? convertFileSrc(path) : null;
```

The `convertFileSrc` call is the only required frontend change for image rendering beyond receiving the path from the IPC command.

## IPC Commands — Existing and New

### Existing Commands (Registered in `lib.rs:invoke_handler!`)

| Command               | Module               | Signature                                                                                                                             |
| --------------------- | -------------------- | ------------------------------------------------------------------------------------------------------------------------------------- |
| `protondb_lookup`     | `commands::protondb` | `async fn(app_id: String, force_refresh: Option<bool>, metadata_store: State<MetadataStore>) -> Result<ProtonDbLookupResult, String>` |
| `settings_load`       | `commands::settings` | `fn(store: State<SettingsStore>) -> Result<AppSettingsData, String>`                                                                  |
| `settings_save`       | `commands::settings` | `fn(data: AppSettingsData, store: State<SettingsStore>) -> Result<(), String>`                                                        |
| `auto_populate_steam` | `commands::steam`    | `async fn(request: SteamAutoPopulateRequest) -> Result<SteamAutoPopulateResult, String>`                                              |

### New Commands (Phase 2)

| Command                | Module                          | Signature                                                                                                              |
| ---------------------- | ------------------------------- | ---------------------------------------------------------------------------------------------------------------------- |
| `fetch_game_metadata`  | `commands::game_metadata` (new) | `async fn(app_id: String, metadata_store: State<MetadataStore>) -> Result<SteamMetadataLookupResult, String>`          |
| `fetch_game_cover_art` | `commands::game_metadata` (new) | `async fn(app_id: String, image_type: String, metadata_store: State<MetadataStore>) -> Result<Option<String>, String>` |

`fetch_game_cover_art` returns `Option<String>` — `Some(absolute_path)` on success, `None` if no image is available. The frontend converts to `asset://` URL via `convertFileSrc`.

## Configuration and Environment

### Data Directory Resolution

`MetadataStore` and image cache both resolve paths via the `directories` crate (`BaseDirs`):

- **Linux**: `~/.local/share/crosshook/` (`XDG_DATA_HOME/crosshook/`)
- **Settings**: `~/.config/crosshook/settings.toml` (`XDG_CONFIG_HOME/crosshook/`)

### Cargo.toml Changes Required

**`crosshook-core/Cargo.toml`** — one new dependency:

```toml
infer = "~0.16"
```

All other dependencies (`reqwest`, `rusqlite`, `chrono`, `uuid`, `serde`, `tracing`, `directories`) are already present and adequate for the new modules.

**`src-tauri/Cargo.toml`** — no changes needed (delegates to crosshook-core for all new logic).

### Shell:allow-open Extension (Phase 3 — SteamGridDB)

The capabilities file currently permits `shell:allow-open` only for `https://www.protondb.com/**`. If SteamGridDB links are surfaced in UI, `https://www.steamgriddb.com/**` must be added to the allow list.

## Gotchas and Edge Cases

- **`MetadataStore` mutex is a `Mutex`, not `RwLock`**: All reads and writes serialize on the same lock. The cache-first pattern's two SQL queries (valid check + stale fallback) each acquire the lock separately — no nested locking possible. New `GameImageStore` methods must follow the same `with_sqlite_conn` pattern, never holding the lock across async awaits.

- **`with_conn` silently returns `T::default()` when unavailable**: If `MetadataStore` is in degraded (`disabled()`) state, cache reads return `None` and writes are silently dropped. The feature code must handle `None` cache results as a normal code path (not an error), matching ProtonDB's behavior of falling through to a live fetch.

- **`external_cache_entries` upsert uses `ON CONFLICT(cache_key)`**: The `cache_id` UUID is generated fresh on each call but the conflict target is `cache_key`. A second upsert for the same `cache_key` updates the row in-place — `cache_id` stays from the original insert. This is intentional; do not treat `cache_id` as a freshness indicator.

- **Migration `migrate_7_to_8` uses defensive column-existence check**: Some migrations check `PRAGMA table_info` before applying `ALTER TABLE`. This is the pattern for idempotent migrations when a column might already exist (from a partial previous run). New migrations for additive tables can use `CREATE TABLE IF NOT EXISTS` directly and do not need this pattern.

- **`migrations.rs` has no `migrate_2_to_3` guard** (line jumps from `1_to_2` to `3_to_4`): The `run_migrations` function has `if version < 2` but no `if version < 3`. Migration `migrate_2_to_3` is defined but called inside the `version < 3` block (line is present in `migrate_2_to_3`). This means the function exists at the Rust level. This is not a bug — just an ordering artifact in the file.

- **`spawn_blocking` for sync core functions**: `commands/steam.rs:54` shows the `spawn_blocking` pattern when the core function (`attempt_auto_populate`) is synchronous. If `lookup_steam_metadata` uses `async fn` throughout (as ProtonDB does), use `async fn` in the command handler directly — no `spawn_blocking` needed.

- **`convertFileSrc` is required** — rendering a raw filesystem path as an `<img src>` in the Tauri webview does not work without it. The CSP `img-src` and capability scope must both be in place before `asset://` URLs render correctly.

- **`AppSettingsData` must be updated on both Rust and TypeScript sides**: The settings are round-tripped through IPC. Adding `steamgriddb_api_key` to `AppSettingsData` in Rust without updating `types/settings.ts` in TypeScript will cause the frontend to silently drop the field on `settings_save`.

## Other Docs

- `docs/plans/ui-enhancements/feature-spec.md`: Full feature specification with phasing, data models, and security findings
- `docs/plans/ui-enhancements/research-security.md`: Security findings including SVG rejection (I1), path traversal (I2), asset protocol CSP (A3)
- `docs/plans/ui-enhancements/research-technical.md`: Component hierarchy, Rust module structure, frontend component breakdown
- [Tauri v2 Asset Protocol](https://v2.tauri.app/security/csp/): `convertFileSrc` usage and CSP requirements
- [Steam Store API (community wiki)](https://wiki.teamfortress.com/wiki/User:RJackson/StorefrontAPI): `appdetails` endpoint reference
