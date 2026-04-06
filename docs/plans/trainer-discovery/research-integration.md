# Trainer Discovery: Integration Research

## Overview

This document captures the actual integration points in the CrossHook codebase relevant to implementing
trainer-discovery. SQLite **`PRAGMA user_version`** is advanced by `metadata::run_migrations()` in `migrations.rs` and **ends at 17 in-tree today** (successive `if version < N` blocks through `N = 17`). Trainer-discovery adds **`migrate_17_to_18`** for **`trainer_sources`** per `feature-spec.md` (Option B). _Note:_ `AGENTS.md` “Current schema version: 13” describes the documented table-inventory snapshot, not `user_version` — **use `migrations.rs` for migration numbering.** Trainer SHA-256 verification (#156) and ProtonDB suggestions (#155) remain reference patterns.

---

## Relevant Files

### Tauri IPC Layer

- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src-tauri/src/lib.rs` — App setup, all `State<>` managed stores registered here, `invoke_handler!` macro registers all IPC commands
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src-tauri/src/commands/mod.rs` — Module declarations for all command files
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src-tauri/src/commands/community.rs` — Community tap commands: `community_sync`, `community_list_indexed_profiles`, `community_import_profile`, etc.
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src-tauri/src/commands/protondb.rs` — ProtonDB commands: `protondb_lookup`, `protondb_get_suggestions`, `protondb_accept_suggestion`, `protondb_dismiss_suggestion` (template for async external API commands)

### Database Schema and Migrations

- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/metadata/migrations.rs` — `run_migrations()`; **`user_version` 17** after the last in-tree guard; trainer-discovery extends with **v18** (`trainer_sources`)
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/metadata/mod.rs` — `MetadataStore` struct, `with_sqlite_conn()` helper, all public store methods
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/metadata/models.rs` — All model types: `CommunityProfileRow`, `VersionSnapshotRow`, `MetadataStoreError`, `MAX_CACHE_PAYLOAD_BYTES` (512 KiB)
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/metadata/community_index.rs` — `index_community_tap_result()`, `list_community_tap_profiles()`; A6 field length validation constants
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/metadata/cache_store.rs` — `get_cache_entry()`, `put_cache_entry()`, `evict_expired_cache_entries()` — the cache pattern
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/metadata/version_store.rs` — `upsert_version_snapshot()`, `lookup_latest_version_snapshot()`, `compute_correlation_status()`

### External Service Integration (ProtonDB Pattern)

- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/protondb/client.rs` — Complete reference for: `OnceLock` HTTP client singleton, cache → live → stale-fallback pattern, `load_cached_lookup_row()`, `persist_lookup_result()`
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/protondb/mod.rs` — Re-exports for the protondb module
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/steam_metadata/client.rs` — `lookup_steam_metadata()` with same pattern as ProtonDB

### File System Operations (Steam/Manifest)

- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/steam/manifest.rs` — `parse_manifest_full()`, `find_game_match()`, `compatdata_path_for_match()`
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/steam/libraries.rs` — `find_steam_libraries()` — locates all Steam library paths
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/steam/vdf.rs` — `parse_vdf()` — hand-rolled VDF/KeyValue parser

### Security / Hash Verification

- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/offline/hash.rs` — `verify_and_cache_trainer_hash()`, `normalize_sha256_hex()` (must be 64 hex chars), `trainer_hash_launch_check()`, `TrainerHashLaunchOutcome`

### Community Profile Schema

- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/profile/community_schema.rs` — `CommunityProfileManifest`, `CommunityProfileMetadata`, `CompatibilityRating`, `COMMUNITY_PROFILE_SCHEMA_VERSION = 1`

### Frontend Hooks (Templates)

- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/hooks/useProtonDbSuggestions.ts` — Request-ID cancellation pattern (`requestIdRef.current`) for stale request protection
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/hooks/useCommunityProfiles.ts` — Community profile data hook
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/types/` — Directory of all TypeScript type definitions; discovery types file goes here

---

## Architectural Patterns

### Tauri IPC Command Registration

All IPC commands follow this exact structure:

1. Define a `#[tauri::command]` function in `src-tauri/src/commands/<module>.rs`
2. Declare the module in `src-tauri/src/commands/mod.rs`
3. Register the command in the `invoke_handler!` macro in `src-tauri/src/lib.rs`
4. Access managed state via `State<'_, T>` parameters (stores are registered via `.manage()` in `run()`)

**Sync vs async**: Most commands are `pub fn` (synchronous). Commands that perform network I/O use `pub async fn` and call `.inner().clone()` on the `State<>` to move it across the await. See `protondb_lookup` in `commands/protondb.rs:50-57` for the exact pattern.

**Error convention**: All commands return `Result<T, String>`. Errors are converted with a local `map_error` helper or `.map_err(|e| e.to_string())`. Never use `unwrap()`.

**Managed stores in scope for discovery**:

- `MetadataStore` — SQLite database access (required for all discovery DB operations)
- `ProfileStore` — TOML profile file access (required for version matching)
- `SettingsStore` — App settings including community tap subscriptions
- `CommunityTapStore` — Git-based tap workspace management

### Database Schema (`user_version` 17 today) — Relevant Tables

The database lives at `~/.local/share/crosshook/metadata.db` (resolved via `directories::BaseDirs::data_local_dir()`).

#### `community_profiles` (migration v4, rebuilt v5)

Indexed community profile rows from `community-profile.json`. Under **Option B**, this is **not** the primary table for discovery **search** (see **`trainer_sources`** below). Columns are mostly `TEXT` with `NULL` meaning absent/unknown.

| Column                 | Type       | Notes                                                 |
| ---------------------- | ---------- | ----------------------------------------------------- |
| `id`                   | INTEGER PK | Auto-increment                                        |
| `tap_id`               | TEXT FK    | References `community_taps(tap_id)` ON DELETE CASCADE |
| `relative_path`        | TEXT       | Path within tap workspace                             |
| `manifest_path`        | TEXT       | Absolute path on disk                                 |
| `game_name`            | TEXT       | Searchable; max 512 bytes (A6 bound)                  |
| `game_version`         | TEXT       | For version matching; max 256 bytes                   |
| `trainer_name`         | TEXT       | Searchable; max 512 bytes                             |
| `trainer_version`      | TEXT       | For version matching; max 256 bytes                   |
| `proton_version`       | TEXT       | Filter criterion; max 256 bytes                       |
| `compatibility_rating` | TEXT       | `unknown\|broken\|partial\|working\|platinum`         |
| `author`               | TEXT       | Searchable; max 512 bytes                             |
| `description`          | TEXT       | Searchable; max 4096 bytes                            |
| `platform_tags`        | TEXT       | Space-joined list; max 2048 bytes                     |
| `schema_version`       | INTEGER    | Must equal 1                                          |
| `created_at`           | TEXT       | RFC3339                                               |

UNIQUE index on `(tap_id, relative_path)`.

**Trainer-discovery (v17→v18)**: Do **not** add `source_url` / `source_name` here. Add new table **`trainer_sources`** (`CREATE TABLE` + indexes) as specified in `feature-spec.md`.

#### `trainer_sources` (planned — `migrate_17_to_18`)

New relational table populated from **`trainer-sources.json`** during tap sync. Phase A **LIKE** search targets this table (joined to `community_taps`). See `feature-spec.md` for full DDL and indexing strategy. Phase C may add an **FTS5 virtual table** (`migrate_18_to_19`) referencing these rows — not a second copy of business data when using SQLite content-sync FTS.

#### `community_taps` (migration v4)

| Column             | Type            | Notes                                 |
| ------------------ | --------------- | ------------------------------------- |
| `tap_id`           | TEXT PK         | UUID                                  |
| `tap_url`          | TEXT            | Git remote URL                        |
| `tap_branch`       | TEXT DEFAULT '' | Branch name                           |
| `local_path`       | TEXT            | On-disk workspace path                |
| `last_head_commit` | TEXT            | Watermark: skip re-index if unchanged |
| `profile_count`    | INTEGER         | Cached count                          |
| `last_indexed_at`  | TEXT            | RFC3339                               |

UNIQUE on `(tap_url, tap_branch)`.

#### `external_cache_entries` (migration v4)

Used for all external API responses (ProtonDB, Steam metadata, and future discovery sources).

| Column         | Type        | Notes                                  |
| -------------- | ----------- | -------------------------------------- |
| `cache_id`     | TEXT PK     | UUID                                   |
| `source_url`   | TEXT        | Origin URL                             |
| `cache_key`    | TEXT UNIQUE | Namespace-prefixed key                 |
| `payload_json` | TEXT        | JSON payload, NULL if > 512 KiB        |
| `payload_size` | INTEGER     | Byte count                             |
| `fetched_at`   | TEXT        | RFC3339                                |
| `expires_at`   | TEXT        | TTL boundary; NULL means never expires |

`put_cache_entry()` uses UPSERT on `cache_key`. The 512 KiB cap (`MAX_CACHE_PAYLOAD_BYTES = 524_288`) is enforced in `cache_store.rs` — payloads over the limit are stored as NULL.

#### `version_snapshots` (migration v8→v9)

Stores game version history per profile for version correlation.

| Column              | Type       | Notes                                                                                          |
| ------------------- | ---------- | ---------------------------------------------------------------------------------------------- |
| `id`                | INTEGER PK | Auto-increment                                                                                 |
| `profile_id`        | TEXT FK    | References `profiles(profile_id)`                                                              |
| `steam_app_id`      | TEXT       | Steam App ID                                                                                   |
| `steam_build_id`    | TEXT       | From `.acf` manifest (numeric string)                                                          |
| `trainer_version`   | TEXT       | From community profile or manual set                                                           |
| `trainer_file_hash` | TEXT       | SHA-256 of trainer executable                                                                  |
| `human_game_ver`    | TEXT       | Display label (e.g., "1.12.3")                                                                 |
| `status`            | TEXT       | `untracked\|matched\|game_updated\|trainer_changed\|both_changed\|unknown\|update_in_progress` |
| `checked_at`        | TEXT       | RFC3339                                                                                        |

Max 50 rows per profile (`MAX_VERSION_SNAPSHOTS_PER_PROFILE`). `compute_correlation_status()` in `version_store.rs:185` is the pure function for version comparison — adapt it, do not duplicate.

#### `trainer_hash_cache` (migration v12→v13)

SHA-256 cache keyed by `(profile_id, file_path)`:

| Column                     | Key Fields           |
| -------------------------- | -------------------- |
| `profile_id` + `file_path` | UNIQUE index         |
| `sha256_hash`              | Hex string, 64 chars |
| `file_size`                | i64                  |
| `file_modified_at`         | RFC3339 mtime        |

Used by `verify_and_cache_trainer_hash()` in `offline/hash.rs` — stat + compare → rehash only when stale.

### External Service Integration (ProtonDB Pattern)

Every external API call in crosshook-core follows a 5-step pattern (see `protondb/client.rs`):

1. **Normalize input** — validate/normalize the lookup key (e.g., `normalize_app_id()`)
2. **Fresh cache check** — `load_cached_lookup_row(allow_expired=false)` → return immediately if valid
3. **Live fetch** — `fetch_live_lookup()` using the `OnceLock<reqwest::Client>` singleton
4. **Persist** — `persist_lookup_result()` via `metadata_store.put_cache_entry()`
5. **Stale fallback** — on network failure: `load_cached_lookup_row(allow_expired=true)` → return stale result with `is_stale=true`

**HTTP client singleton** (copy this exactly for new modules):

```rust
const REQUEST_TIMEOUT_SECS: u64 = 6;
static TRAINER_DISCOVERY_HTTP_CLIENT: OnceLock<reqwest::Client> = OnceLock::new();

fn trainer_discovery_http_client() -> Result<&'static reqwest::Client, TrainerDiscoveryError> {
    if let Some(client) = TRAINER_DISCOVERY_HTTP_CLIENT.get() {
        return Ok(client);
    }
    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(REQUEST_TIMEOUT_SECS))
        .user_agent(format!("CrossHook/{}", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(TrainerDiscoveryError::Network)?;
    let _ = TRAINER_DISCOVERY_HTTP_CLIENT.set(client);
    Ok(TRAINER_DISCOVERY_HTTP_CLIENT.get().expect("initialized"))
}
```

**Cache key convention**: Namespace-prefixed, colon-delimited. ProtonDB uses `protondb::{app_id}`. Discovery should use:

- `trainer_discovery:game:{steam_app_id}` for per-game source lookups
- `trainer_discovery:fling_index` for the FLiNG RSS feed index (1h TTL)

**CACHE_TTL**: ProtonDB uses 6 hours. Steam metadata uses 24 hours. Discovery: 1h for RSS feed index, 6h for individual trainer source data.

### Community Profile Indexing Pipeline

The full pipeline when a tap is synced via `community_sync`:

1. `CommunityTapStore::sync_many()` → fetches/pulls git repos
2. `metadata_store.index_community_tap_result(result)` → calls `community_index::index_community_tap_result()`
3. Inside: watermark check (skip if HEAD SHA unchanged), transactional DELETE+INSERT for all `community_profiles` rows
4. A6 field length validation (`check_a6_bounds()`) skips oversized entries with `tracing::warn!`
5. `profile_count` updated to actual inserted count

**Adding `source_url`/`source_name` to Phase 1**: Three touch points:

1. `CommunityProfileMetadata` struct in `profile/community_schema.rs` (add optional fields)
2. `index_community_tap_result()` in `metadata/community_index.rs` (persist new fields)
3. `CommunityProfileRow` struct in `metadata/models.rs` (include in row mapping)
4. Migration v17→v18 SQL: `ALTER TABLE community_profiles ADD COLUMN source_url TEXT; ALTER TABLE community_profiles ADD COLUMN source_name TEXT;`

### File System Operations

**Steam `.acf` manifest parsing** — fully implemented, no new code needed:

- `steam/libraries.rs::find_steam_libraries()` — enumerates all Steam library paths
- `steam/manifest.rs::find_game_match(game_path, libraries, &mut diagnostics)` — scans all `.acf` manifests to find which one contains the game exe path; returns `SteamGameMatchSelection` with `.matched.app_id` and `.matched.manifest_path`
- `steam/manifest.rs::parse_manifest_full(manifest_path)` — returns `ManifestData { build_id, install_dir, state_flags, last_updated }`; rejects non-numeric `build_id`

**Gotcha**: `build_id` in `.acf` files is validated to be numeric-only (`build_id.chars().all(|c| c.is_ascii_digit())`). Non-numeric build IDs return `Err`. Empty build ID is allowed and returns `""`.

**Community tap workspace paths** — resolved via `CommunityTapStore::resolve_workspace()`. Each tap workspace is a local git clone whose path is stored in `community_taps.local_path`. The workspace path is the root for all `relative_path` entries in `community_profiles`.

### Process Management (Launch)

Trainer launch is handled in `src-tauri/src/commands/launch.rs`. The trainer-discovery feature does not need to modify the launch pipeline — it provides discovery metadata that links to the existing import (`community_import_profile`) and launch commands.

**Trainer hash verification at launch** (`launch/trainer_hash.rs`):

- `trainer_hash_launch_check(conn, profile_id, trainer_path, community_trainer_sha256)` runs on every launch
- Cross-checks on-disk hash vs `trainer_hash_cache` (baseline check) and vs community manifest `trainer_sha256` (advisory check)
- Returns `TrainerHashLaunchOutcome { baseline, community_advisory }`

The `community_trainer_sha256` parameter is read from `profile.trainer.sha256` in the TOML profile — which community manifests can populate via `CommunityProfileMetadata.trainer_sha256`. Discovery should propagate this SHA-256 from tap manifests through to imported profiles.

### Frontend Integration Pattern

All frontend IPC calls use `invoke()` from `@tauri-apps/api/core`:

```typescript
import { invoke } from '@tauri-apps/api/core';

const result = await invoke<TrainerSearchResponse>('discovery_search_trainers', { query, installedAppId });
```

**Stale request cancellation** (`useProtonDbSuggestions.ts:27`):

```typescript
const requestIdRef = useRef(0);
const id = ++requestIdRef.current;
// ... after await:
if (requestIdRef.current !== id) return; // discard stale response
```

Copy this pattern verbatim into `useTrainerDiscovery.ts`.

**Hook return shape** — follows the same `{ data, loading, error, refresh }` pattern across all existing hooks.

**Type file location**: `src/crosshook-native/src/types/` — create `discovery.ts` here; re-export from `src/types/index.ts`.

---

## Database Schema — Complete Current State (v17)

18 tables exist at v17. Relevant to discovery:

| Table                    | Migration | Purpose                                                                                   |
| ------------------------ | --------- | ----------------------------------------------------------------------------------------- |
| `community_taps`         | v4        | Git tap subscriptions                                                                     |
| `community_profiles`     | v4/v5     | Indexed community profiles (trainer metadata)                                             |
| `external_cache_entries` | v4        | External API response cache (ProtonDB, Steam, future trainer sources)                     |
| `version_snapshots`      | v9        | Game/trainer version history per profile                                                  |
| `trainer_hash_cache`     | v13       | SHA-256 hash cache for trainer files                                                      |
| `suggestion_dismissals`  | v17       | Per-profile suggestion dismissal tracking (ProtonDB pattern; same approach for discovery) |

---

## External Service Integration

### What Already Exists (No New Code)

| Service                       | Implementation                                                             |
| ----------------------------- | -------------------------------------------------------------------------- |
| Steam `.acf` manifest parsing | `steam/manifest.rs`                                                        |
| Steam appdetails API          | `steam_metadata/client.rs::lookup_steam_metadata()`                        |
| ProtonDB summary + reports    | `protondb/client.rs::lookup_protondb()`                                    |
| HTTP client (reqwest)         | `reqwest` with `json` + `rustls-tls` features, no additional crates needed |
| SQLite cache                  | `metadata/cache_store.rs`                                                  |

### What Needs to Be Built (Phase 1 → Phase 2)

| Capability                                       | Phase | New Code                                     |
| ------------------------------------------------ | ----- | -------------------------------------------- |
| `source_url`/`source_name` on community profiles | 1     | Small schema + struct additions              |
| `discovery_search_trainers` IPC (LIKE-based)     | 1     | New command module + search query            |
| FTS5 virtual table + triggers                    | 2     | New migration + index module                 |
| External trainer source HTTP client              | 2     | New `discovery/client.rs` mirroring protondb |
| FLiNG RSS fetch + XML parse                      | 2     | New `discovery/client.rs` + parse function   |
| Version matching algorithm                       | 2     | New `discovery/version_match.rs`             |

**No new crate dependencies required for Phase 1**. Phase 2 may need `scraper` for HTML parsing if FLiNG RSS is unavailable; all other infrastructure is already present.

---

## Configuration

**MetadataStore path**: `~/.local/share/crosshook/metadata.db`

**Community tap workspaces**: stored in `~/.local/share/crosshook/community/` (resolved via `CommunityTapStore`)

**Settings file** (TOML): `community_taps` list in `AppSettingsData` — this is the source-of-truth for which taps are subscribed. Modified by `community_add_tap` command.

**MAX_CACHE_PAYLOAD_BYTES**: `524_288` (512 KiB) — enforced in `cache_store.rs::put_cache_entry()`. Payloads exceeding this are stored as NULL. FLiNG RSS feed and individual trainer page payloads are expected to be well under this limit.

---

## Gotchas and Edge Cases

- **`with_sqlite_conn` is the only safe DB access path**: All SQLite operations go through `metadata_store.with_sqlite_conn(action_label, |conn| {...})`. Direct `Connection` access is only available in unit tests via `db::open_in_memory()`. Never hold the mutex lock across an await point.

- **Async commands need `.inner().clone()`**: The `MetadataStore` wraps a `Arc<Mutex<Connection>>`. Async Tauri commands must call `metadata_store.inner().clone()` to move the store across the await boundary (see `commands/protondb.rs:55`).

- **Watermark skip is opaque**: If `last_head_commit` in `community_taps` matches the current HEAD, `index_community_tap_result()` returns `Ok(())` silently without updating any profiles. Phase 2 FTS trigger-based sync is only safe if the source table is updated — the watermark skip means FTS is also implicitly skipped correctly.

- **`CommunityProfileRow` is the query result type, not `CommunityProfileMetadata`**: The metadata struct is for manifest deserialization. The row struct is for DB query results. They have different shapes (row has `tap_url`, `id`, `tap_id`; metadata does not).

- **Community profile imports must be in-workspace**: `validate_import_path_in_workspace()` in `commands/community.rs:230` enforces that `community_import_profile` only accepts files from known tap workspaces. Discovery search results can link to `manifest_path` which IS a workspace path, so they will pass this validation.

- **`COMMUNITY_PROFILE_SCHEMA_VERSION` is 1**: Adding optional fields (`source_url`, `source_name`) to `CommunityProfileMetadata` is backward-compatible; old manifests without these fields continue to deserialize with `None`. The version should stay at 1 unless breaking changes require enforcement.

- **`normalize_sha256_hex` is strict**: Strips `0x`/`0X` prefix, then validates 64 hex characters. Returns `None` for any deviation. Always call this before comparing community-supplied SHA-256 values.

- **TrainerHashBaselineResult::Mismatch does NOT update cache**: On content change vs baseline, the hash cache is NOT updated. The user must confirm via the `verify_trainer_hash` IPC command. Discovery should document this behavior when surfacing SHA-256 mismatch warnings.

- **`suggestion_dismissals` TTL pattern**: The ProtonDB suggestion dismissal mechanism uses a 30-day retention TTL stored in `suggestion_dismissals`. If discovery wants per-profile dismissal of trainer suggestions, the same table/pattern can be reused (it's generic by `suggestion_key`).

---

## Findings for Teammate Agents

### For architecture-researcher

- `MetadataStore` is passed as managed Tauri state. It is `Clone` (wraps `Arc<Mutex<Connection>>`). The DB connection is single-writer; all writes serialize through the `Mutex`.
- The `community_profiles` table is owned by the community tap sync pipeline. Discovery search reads from it without modification in Phase 1. Phase 2 FTS table is a content-sync virtual table that shadows `community_profiles` — no duplication of base data.
- All IPC commands are registered in a single flat `invoke_handler!` macro. There is no dynamic registration.

### For patterns-researcher

- The 3-stage cache pattern (valid cache → live → stale) is used identically in `protondb/client.rs` and `steam_metadata/client.rs`. Any new external source client must implement all three stages.
- Community tap indexing uses watermark-based skip (HEAD commit SHA), transactional DELETE+INSERT, and A6 field length validation. These are all co-located in `community_index.rs`.
- Frontend hooks use `requestIdRef.current` increment for stale request cancellation — this is the project's established pattern for async IPC hooks.

### For docs-researcher

- Tauri IPC command naming convention: `snake_case`, matching the Rust function name exactly. Frontend `invoke('discovery_search_trainers', ...)` maps to `#[tauri::command] pub fn discovery_search_trainers(...)`.
- The Rust `#[serde(rename_all = "camelCase")]` attribute on structs means frontend receives camelCase fields while backend uses snake_case. This is the established convention for all Serde types crossing the IPC boundary.
