# Offline Trainers — Integration Research

Comprehensive audit of APIs, database schema, file system patterns, and external service integrations relevant to the offline-first trainer management feature.

## Relevant Files

- `src/crosshook-native/src-tauri/src/lib.rs` — Tauri app setup; command registration via `invoke_handler!`, managed state injection
- `src/crosshook-native/src-tauri/src/commands/mod.rs` — Command module index (16 modules today)
- `src/crosshook-native/src-tauri/src/commands/launch.rs` — `launch_game`/`launch_trainer` commands; `hash_trainer_file` used at launch-complete for version snapshots
- `src/crosshook-native/src-tauri/src/commands/version.rs` — `check_version_status`, `get_version_snapshot`, `set_trainer_version`, `acknowledge_version_change`
- `src/crosshook-native/src-tauri/src/commands/health.rs` — `batch_validate_profiles`, `get_profile_health`, `get_cached_health_snapshots`; model for enriched health reports
- `src/crosshook-native/src-tauri/src/commands/settings.rs` — `settings_load`/`settings_save`; `AppSettingsData` TOML struct
- `src/crosshook-native/crates/crosshook-core/src/metadata/db.rs` — SQLite connection factory; WAL mode, FK enforcement, 0600/0700 permissions, symlink rejection
- `src/crosshook-native/crates/crosshook-core/src/metadata/migrations.rs` — Sequential `PRAGMA user_version` migrations 0→12; current schema with all tables
- `src/crosshook-native/crates/crosshook-core/src/metadata/version_store.rs` — `hash_trainer_file()`, `upsert_version_snapshot()`, `compute_correlation_status()`
- `src/crosshook-native/crates/crosshook-core/src/metadata/health_store.rs` — `health_snapshots` read/write; pattern for offline_readiness_snapshots
- `src/crosshook-native/crates/crosshook-core/src/metadata/cache_store.rs` — `external_cache_entries` CRUD; size-limiting pattern (`MAX_CACHE_PAYLOAD_BYTES`)
- `src/crosshook-native/crates/crosshook-core/src/metadata/mod.rs` — `MetadataStore` struct; `try_new()` + `disabled()` fail-soft pattern; re-exports
- `src/crosshook-native/crates/crosshook-core/src/profile/models.rs` — `GameProfile`, `TrainerSection` (has `kind` + `path` + `loading_mode`), `LocalOverrideSection`
- `src/crosshook-native/crates/crosshook-core/src/settings/mod.rs` — `AppSettingsData` (TOML-persisted settings struct)
- `src/crosshook-native/crates/crosshook-core/Cargo.toml` — Dependencies: `sha2 = "0.11.0"`, `rusqlite = "0.39.0"`, `uuid`, `chrono`, `directories`

## Tauri IPC Commands (Existing Patterns)

### Registration Pattern

All commands are declared with `#[tauri::command]` and registered in `lib.rs` via `tauri::generate_handler![...]`. The new `commands/offline.rs` module must be added to `commands/mod.rs` and all its commands listed in `invoke_handler`.

### Command Signature Conventions

```rust
// Sync command with managed state (most health/version commands)
#[tauri::command]
pub fn command_name(
    arg: ArgType,
    store: State<'_, ProfileStore>,
    metadata_store: State<'_, MetadataStore>,
) -> Result<ReturnType, String>

// Async command with AppHandle for event emission (launch commands)
#[tauri::command]
pub async fn command_name(
    app: AppHandle,
    request: RequestType,
) -> Result<ReturnType, String>

// Blocking async command (spawn_blocking for CPU-intensive work)
tauri::async_runtime::spawn_blocking(move || { ... }).await
```

### Existing Relevant Commands

| Command                          | File                  | Description                                                |
| -------------------------------- | --------------------- | ---------------------------------------------------------- |
| `check_version_status`           | `commands/version.rs` | Reads manifest + trainer hash, computes correlation status |
| `get_version_snapshot`           | `commands/version.rs` | Returns latest `VersionSnapshotRow` for a profile          |
| `set_trainer_version`            | `commands/version.rs` | Manual trainer version upsert                              |
| `acknowledge_version_change`     | `commands/version.rs` | Resets snapshot status to `matched`                        |
| `batch_validate_profiles`        | `commands/health.rs`  | Full health scan + metadata enrichment                     |
| `get_profile_health`             | `commands/health.rs`  | Single-profile health check                                |
| `get_cached_health_snapshots`    | `commands/health.rs`  | Fast badge load from SQLite cache                          |
| `launch_game` / `launch_trainer` | `commands/launch.rs`  | Launch with post-exit version snapshot                     |
| `get_optimization_catalog`       | `commands/catalog.rs` | Returns loaded catalog entries                             |

### New Commands for Offline Feature (`commands/offline.rs`)

Per the existing technical spec (`research-technical.md`):

- `check_offline_readiness(name: String, ...)` — Compute readiness score for one profile
- `batch_offline_readiness(...)` — Batch version for health dashboard
- `verify_trainer_hash(name: String, ...)` — Re-hash trainer file and update cache
- `get_trainer_hash_cache(name: String, ...)` — Return cached hash entry
- `check_network_status()` — Probe connectivity via `std::net`
- `confirm_offline_activation(name: String, ...)` — Record activation in `offline_readiness_snapshots`

## Database Schema (SQLite — `~/.local/share/crosshook/metadata.db`)

### Connection Configuration (`db.rs`)

```
PRAGMA journal_mode = WAL
PRAGMA foreign_keys = ON
PRAGMA synchronous = NORMAL
PRAGMA busy_timeout = 5000
PRAGMA secure_delete = ON
application_id = 0x43484B00
File permissions: 0600 (db), 0700 (directory)
Symlink detection: refuses to open, returns MetadataStoreError::SymlinkDetected
```

### Current Tables (Schema v12)

| Table                            | Created | Key Columns                                                                                                                                                |
| -------------------------------- | ------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `profiles`                       | v1      | `profile_id TEXT PK`, `current_filename`, `game_name`, `launch_method`, `content_hash`, `is_favorite`, `source`, `deleted_at`                              |
| `profile_name_history`           | v1      | `profile_id FK`, `old_name`, `new_name`, `old_path`, `new_path`, `source`                                                                                  |
| `launchers`                      | v3      | `launcher_id PK`, `profile_id FK`, `launcher_slug UNIQUE`, `script_path`, `desktop_entry_path`, `drift_state`                                              |
| `launch_operations`              | v3      | `operation_id PK`, `profile_id FK`, `launch_method`, `status`, `exit_code`, `log_path`, `diagnostic_json`, `failure_mode`                                  |
| `community_taps`                 | v4      | `tap_id PK`, `tap_url`, `tap_branch`, `local_path`, `last_head_commit`, `profile_count`, `last_indexed_at`                                                 |
| `community_profiles`             | v4      | `tap_id FK`, `relative_path`, `game_name`, `trainer_name`, `trainer_version`, `proton_version`, `compatibility_rating`, `author`                           |
| `external_cache_entries`         | v4      | `cache_id PK`, `source_url`, `cache_key UNIQUE`, `payload_json`, `payload_size`, `fetched_at`, `expires_at`                                                |
| `collections`                    | v4      | `collection_id PK`, `name UNIQUE`, `description`                                                                                                           |
| `collection_profiles`            | v4      | `(collection_id, profile_id) PK` composite                                                                                                                 |
| `health_snapshots`               | v5/v6   | `profile_id PK FK`, `status`, `issue_count`, `checked_at`                                                                                                  |
| `version_snapshots`              | v9      | `id AUTOINCREMENT PK`, `profile_id FK`, `steam_app_id`, `steam_build_id`, `trainer_version`, `trainer_file_hash`, `human_game_ver`, `status`, `checked_at` |
| `bundled_optimization_presets`   | v10     | `preset_id PK`, `vendor`, `mode`, `option_ids_json`, `catalog_version`                                                                                     |
| `profile_launch_preset_metadata` | v10     | `(profile_id, preset_name) PK`, `origin`, `source_bundled_preset_id`                                                                                       |
| `config_revisions`               | v11     | `id AUTOINCREMENT PK`, `profile_id FK`, `content_hash`, `snapshot_toml`, `is_last_known_working` — TRIGGER validates lineage ownership                     |
| `optimization_catalog`           | v12     | `id PK`, `applies_to_method`, `env_json`, `wrappers_json`, `conflicts_with_json`, `category`, `advanced`, `community`                                      |

### Proposed Migration 13 (from `research-technical.md`)

Three new tables for offline-trainer state:

```sql
-- Stat-based hash cache; seeded from version_snapshots.trainer_file_hash
CREATE TABLE trainer_hash_cache (
    cache_id         TEXT PRIMARY KEY,
    profile_id       TEXT NOT NULL REFERENCES profiles(profile_id) ON DELETE CASCADE,
    file_path        TEXT NOT NULL,
    file_size        INTEGER NOT NULL,
    file_modified_at TEXT NOT NULL,    -- ISO 8601; stat-only cache validity check
    sha256_hash      TEXT NOT NULL,    -- lowercase hex, 64 chars
    verified_at      TEXT NOT NULL,
    created_at       TEXT NOT NULL,
    updated_at       TEXT NOT NULL
);
CREATE UNIQUE INDEX idx_trainer_hash_cache_profile_path ON trainer_hash_cache(profile_id, file_path);

-- Machine-local readiness snapshots (not portable)
CREATE TABLE offline_readiness_snapshots (
    profile_id           TEXT PRIMARY KEY REFERENCES profiles(profile_id) ON DELETE CASCADE,
    readiness_state      TEXT NOT NULL DEFAULT 'unconfigured',
    readiness_score      INTEGER NOT NULL,  -- 0-100
    trainer_type         TEXT NOT NULL DEFAULT 'unknown',
    trainer_present      INTEGER NOT NULL DEFAULT 0,
    trainer_hash_valid   INTEGER NOT NULL DEFAULT 0,
    trainer_activated    INTEGER NOT NULL DEFAULT 0,  -- Aurora/WeMod activation
    proton_available     INTEGER NOT NULL DEFAULT 0,
    community_tap_cached INTEGER NOT NULL DEFAULT 0,
    network_required     INTEGER NOT NULL DEFAULT 0,
    blocking_reasons     TEXT,  -- JSON array
    checked_at           TEXT NOT NULL
);

-- Community tap offline state
CREATE TABLE community_tap_offline_state (
    tap_id           TEXT PRIMARY KEY REFERENCES community_taps(tap_id) ON DELETE CASCADE,
    has_local_clone  INTEGER NOT NULL DEFAULT 0,
    last_sync_at     TEXT,
    clone_size_bytes INTEGER
);
```

**Bootstrap note**: When migration 13 runs, `version_snapshots.trainer_file_hash` already has hashes for previously-launched profiles. The migration can `INSERT INTO trainer_hash_cache ... SELECT` from `version_snapshots` to seed initial hash values without a full re-hash pass.

## File System Integration (TOML Stores, Config Paths)

### Storage Layout

```
~/.config/crosshook/
  settings.toml                  # AppSettingsData (auto_load_last_profile, community_taps, etc.)
  recent.toml                    # RecentFilesStore
  *.toml                         # Individual GameProfile files (one per profile)
  optimization_catalog.toml      # Optional user override for optimization catalog

~/.local/share/crosshook/
  metadata.db                    # SQLite database (WAL, 0600)
  community/taps/                # Git-cloned community tap repositories

~/.local/share/crosshook/logs/   # Launch and helper logs
```

### Profile TOML Schema (Relevant to Offline Feature)

The `TrainerSection` in `profile/models.rs` already has a `kind` field (serialized as `[trainer] type`):

```toml
[trainer]
path = "/local/path/to/trainer.exe"   # local_override only — not in portable export
type = ""                              # trainer_type goes here: "standalone", "wemod", etc.
loading_mode = "source_directory"
```

**Key design constraint** (`research-technical.md`): `trainer_type` is portable (goes in TOML), while `trainer_activated` and `readiness_state` are machine-local (SQLite only). This matches the pattern already used by `local_override` fields in `GameProfile`.

### TOML Serde Conventions

- `#[serde(default)]` on all fields — unknown TOML keys survive round-trips without errors
- `#[serde(skip_serializing_if = "...::is_empty")]` for optional sections
- `#[serde(rename = "type")]` already used on `TrainerSection.kind` — adding `trainer_type` as a new enum-backed field needs `#[serde(rename = "type")]` on the new enum field or rename of `kind`

### AppSettingsData Extension

`settings/mod.rs` stores `AppSettingsData` in `settings.toml`. The offline feature needs:

```rust
pub struct AppSettingsData {
    pub auto_load_last_profile: bool,
    pub last_used_profile: String,
    pub community_taps: Vec<CommunityTapSubscription>,
    pub onboarding_completed: bool,
    // NEW:
    #[serde(default)]
    pub offline_mode: bool,  // global offline mode toggle
}
```

Because `#[serde(default)]` is used, adding this field is backwards-compatible with existing `settings.toml` files.

## Steam Integration (Discovery and Manifest Patterns)

### Discovery API

Used in `commands/launch.rs`, `commands/health.rs`, and `commands/version.rs`:

```rust
// Steam root candidates from steam_client_install_path + $HOME fallbacks
let roots = discover_steam_root_candidates(&steam_client_path, &mut diagnostics);
// Enumerate Steam library folders (libraryfolders.vdf)
let libraries = discover_steam_libraries(&roots, &mut diagnostics);
// Parse an individual app manifest for build_id + state_flags
let data = parse_manifest_full(&lib.steamapps_path.join("appmanifest_{app_id}.acf"))?;
```

The `steam_client_install_path` is derived from the profile's `compatdata_path` by splitting on `/steamapps/compatdata/`.

### Manifest Fields Used

- `data.build_id: String` — game build ID, compared against snapshot for change detection
- `data.state_flags: Option<u32>` — `4` = fully installed; `!= 4` = update in progress (→ `UpdateInProgress` status)

### Offline Relevance

For pre-flight offline readiness: the `discover_steam_*` functions are filesystem-only (no network calls). They can be called offline to validate Proton availability. The offline readiness check should call these to set the `proton_available` flag in `offline_readiness_snapshots`.

## External Service Patterns

### SHA-256 Hashing (`version_store.rs`)

The current `hash_trainer_file()` reads the **entire file** into memory via `std::fs::read`:

```rust
pub fn hash_trainer_file(path: &Path) -> Option<String> {
    let bytes = std::fs::read(path).ok()?;
    let digest = Sha256::digest(&bytes);
    // ... hex encode
}
```

**Problem for offline feature**: Trainer `.exe` files are often 2–20 MB. Re-reading on every check is expensive. The `trainer_hash_cache` table adds stat-based invalidation: compare `file_size` and `file_modified_at` against stored values — skip re-hash if both match.

**New hash function needed** in `offline/hash.rs`:

```rust
pub fn hash_with_stat_cache(path: &Path, cache: &HashCache) -> Option<HashResult>
```

Should use streaming SHA-256 (via sha2's `Update` trait) to avoid full-memory reads for large files.

### Network Connectivity Detection

No existing network probe in the codebase. The feature spec calls for a lightweight probe using `std::net::TcpStream::connect_timeout`. This requires no new crate dependencies.

```rust
// Proposed pattern in offline/network.rs
pub fn is_network_available() -> bool {
    std::net::TcpStream::connect_timeout(
        &"8.8.8.8:53".parse().unwrap(),
        std::time::Duration::from_millis(300),
    ).is_ok()
}
```

### Community Taps (Offline Fallback)

Community taps are Git repos cloned to `local_path` (recorded in `community_taps.local_path`). The existing `CommunityTapStore` already has the clone on disk — `index_tap()` can work offline if the repo is already cloned. No new caching layer is needed; the `community_tap_offline_state` table tracks whether a clone exists and its last sync timestamp.

### MetadataStore Fail-Soft Pattern

The `MetadataStore` has two private helpers that all public methods must use — **never access `conn` directly** from outside the metadata module:

```rust
// For read-only operations:
fn with_conn<F, T>(&self, action: &'static str, f: F) -> Result<T, MetadataStoreError>
where F: FnOnce(&Connection) -> Result<T, MetadataStoreError>, T: Default
// Returns Ok(T::default()) when disabled or conn is None — fail-soft built in.

// For write/transaction operations:
fn with_conn_mut<F, T>(&self, action: &'static str, f: F) -> Result<T, MetadataStoreError>
where F: FnOnce(&mut Connection) -> Result<T, MetadataStoreError>, T: Default
```

Both helpers check `self.available` first — if the store is in `disabled()` mode (SQLite unavailable), they return `Ok(T::default())` without any lock attempt. The fail-soft behavior is implicit: new store methods only need the `T: Default` bound and the right action label — no explicit `is_available()` guard needed.

Every new `MetadataStore` method for offline readiness must follow this pattern:

```rust
pub fn upsert_offline_readiness_snapshot(&self, ...) -> Result<(), MetadataStoreError> {
    self.with_conn_mut("upsert offline readiness snapshot", |conn| {
        offline_store::upsert_snapshot(conn, ...)
    })
}
```

## Edgecases

- `TrainerSection.kind` is the existing field name for trainer type (`#[serde(rename = "type")]`) — the offline feature's `trainer_type` enum should serialize into this same field, not add a new one, to avoid duplicating data in TOML
- `hash_trainer_file()` is called at launch-complete time in `commands/launch.rs` (not at profile-save time) — the hash cache bootstrap from `version_snapshots` only covers profiles that have been launched at least once; never-launched profiles need an explicit hash-on-demand flow
- `VersionCorrelationStatus::TrainerChanged` drives the `hash_stale` state transition — but this status is only set on `launch_game`/`launch_trainer` exit, so hash staleness detection requires a prior successful launch
- `profiles.deleted_at IS NOT NULL` soft-deletes are excluded in `load_health_snapshots` via JOIN — offline readiness queries must apply the same filter or use `lookup_profile_id` which returns `None` for deleted profiles
- The `community_tap_cache` table design should use `tap_id FK` on `community_taps(tap_id) ON DELETE CASCADE` — the `community_taps` table already exists at v4; migration 13 only adds the offline state table
- `AppSettingsData` uses `#[serde(default)]` on the struct but NOT on individual fields currently — adding `offline_mode: bool` requires `#[serde(default)]` on that field to avoid breaking existing settings.toml files that lack it
- `sqlite::PRAGMA busy_timeout=5000` (5 second timeout) — offline readiness batch scans that call `is_network_available()` (300ms timeout × N profiles) must run on a background thread via `spawn_blocking` to avoid blocking the Tauri async runtime

## Other Docs

- `/home/yandy/Projects/github.com/yandy-r/crosshook/docs/plans/offline-trainers/research-technical.md` — Full technical architecture spec with proposed data models, state machine, component diagram
- `/home/yandy/Projects/github.com/yandy-r/crosshook/docs/plans/offline-trainers/feature-spec.md` — Business requirements, trainer ecosystem classification, external service constraints
- `/home/yandy/Projects/github.com/yandy-r/crosshook/docs/plans/offline-trainers/research-external.md` — External API research (FLiNG, WeMod, Aurora, PLITCH, Cheat Engine)
- [sha2 crate docs](https://docs.rs/sha2) — Streaming SHA-256 via `sha2::Sha256` + `digest::Update` trait
- [rusqlite docs](https://docs.rs/rusqlite) — `Transaction::new_unchecked`, `OptionalExtension` used throughout metadata module
