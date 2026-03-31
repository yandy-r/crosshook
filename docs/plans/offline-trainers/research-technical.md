# Offline Trainers - Technical Architecture Specification

CrossHook needs offline-first trainer management for Steam Deck portable use. This document specifies the architecture, data models, API surface, and integration points for the feature.

## Executive Summary

The offline-trainers feature introduces a **trainer type classification system**, **offline readiness state machine**, **SHA-256 hash verification with caching**, **community tap offline operation**, and **network-aware launch workflow gates**. The design integrates with the existing profile/metadata/launch/community modules by extending profile TOML schemas, adding three new SQLite tables (migration 13), exposing new Tauri IPC commands, and injecting a pre-flight offline readiness check into the launch validation pipeline.

**Key cross-team alignment** (from business analysis): Activation state is machine-local and lives in SQLite, not profile TOML. The existing `VersionCorrelationStatus::TrainerChanged` signal drives hash staleness transitions. Community taps need no new caching -- the existing workspace-on-disk + `index_tap()` fallback is sufficient.

## Architecture Design

### Component Diagram

```
┌─────────────────────────────────────────────────────────────────────────────────┐
│ Frontend (React/TypeScript)                                                      │
│  ┌──────────────────┐  ┌───────────────────┐  ┌─────────────────────────┐       │
│  │ OfflineStatusBadge│  │ TrainerTypeSelect │  │ OfflineReadinessPanel   │       │
│  │ (per-profile)     │  │ (profile editor)  │  │ (pre-flight / health)  │       │
│  └────────┬─────────┘  └────────┬──────────┘  └───────────┬─────────────┘       │
│           │                     │                          │                     │
│           └─────────────────────┴──────────────────────────┘                     │
│                                     │ invoke()                                   │
├─────────────────────────────────────┼───────────────────────────────────────────┤
│ Tauri IPC Layer (src-tauri/src/commands/)                                        │
│  ┌───────────────────────────────────┐  ┌────────────────────────────────┐       │
│  │ commands/offline.rs (NEW)         │  │ commands/launch.rs (MODIFIED)  │       │
│  │ - check_offline_readiness         │  │ - launch_game (add pre-flight) │       │
│  │ - verify_trainer_hash             │  │ - launch_trainer (add gate)    │       │
│  │ - check_network_status            │  └────────────────────────────────┘       │
│  │ - get_trainer_hash_cache          │                                           │
│  │ - batch_offline_readiness         │                                           │
│  │ - confirm_offline_activation      │                                           │
│  └───────────────────────────────────┘                                           │
├─────────────────────────────────────────────────────────────────────────────────┤
│ crosshook-core Library                                                           │
│  ┌────────────────────────┐  ┌───────────────────────┐  ┌────────────────────┐  │
│  │ offline/ (NEW MODULE)  │  │ metadata/ (EXTENDED)   │  │ launch/ (MODIFIED) │  │
│  │ - trainer_type.rs      │  │ - offline_store.rs     │  │ - request.rs       │  │
│  │ - readiness.rs         │  │ - migrations.rs (+v13) │  │   (new validation  │  │
│  │ - network.rs           │  │ - hash_cache.rs        │  │    errors)         │  │
│  │ - hash.rs              │  └───────────────────────┘  │ - script_runner.rs  │  │
│  └────────────────────────┘                              │   (offline guards)  │  │
│                                                          └────────────────────┘  │
│  ┌────────────────────────┐  ┌───────────────────────┐                           │
│  │ profile/ (EXTENDED)    │  │ community/ (EXTENDED)  │                           │
│  │ - models.rs            │  │ - taps.rs              │                           │
│  │   (TrainerSection      │  │   (offline fallback)   │                           │
│  │    +trainer_type)      │  └───────────────────────┘                           │
│  │ - health.rs            │                                                      │
│  │   (offline readiness   │                                                      │
│  │    checks)             │                                                      │
│  └────────────────────────┘                                                      │
├─────────────────────────────────────────────────────────────────────────────────┤
│ SQLite (metadata.db)                                                             │
│  ┌─────────────────────────┐  ┌──────────────────────┐  ┌────────────────────┐  │
│  │ trainer_hash_cache      │  │ offline_readiness     │  │ community_tap_cache│  │
│  │ (SHA-256 hash cache)    │  │ (readiness snapshots) │  │ (offline state)    │  │
│  └─────────────────────────┘  └──────────────────────┘  └────────────────────┘  │
└─────────────────────────────────────────────────────────────────────────────────┘
```

### New Components

1. **`crosshook-core/src/offline/` module** (new): Core offline logic
   - `trainer_type.rs` - Trainer type enum and classification heuristics
   - `readiness.rs` - Per-profile offline readiness scoring and state machine
   - `network.rs` - Network connectivity detection
   - `hash.rs` - SHA-256 hash computation with streaming support for large files

2. **`metadata/offline_store.rs`** (new): SQLite persistence for offline state
   - Hash cache CRUD
   - Readiness snapshot persistence (including activation state)
   - Community tap cache state tracking

3. **`src-tauri/src/commands/offline.rs`** (new): Tauri IPC commands

4. **Frontend components** (new):
   - `OfflineStatusBadge` - Inline badge showing offline readiness
   - `OfflineReadinessPanel` - Detailed readiness breakdown
   - `TrainerTypeSelect` - Dropdown in profile editor for trainer type

### Integration Points

| Existing Module             | Integration                                         | Nature                   |
| --------------------------- | --------------------------------------------------- | ------------------------ |
| `profile/models.rs`         | Add `trainer_type` field to `TrainerSection`        | Schema extension         |
| `profile/health.rs`         | Inject offline readiness issues into health reports | Health check extension   |
| `launch/request.rs`         | Add `OfflineReadinessInsufficient` validation error | Validation extension     |
| `launch/script_runner.rs`   | Pre-launch offline readiness gate                   | Launch flow modification |
| `community/taps.rs`         | Offline-aware sync with cached fallback             | Sync flow modification   |
| `metadata/migrations.rs`    | Migration v12 -> v13 for new tables                 | Schema migration         |
| `metadata/version_store.rs` | `TrainerChanged` status drives hash staleness       | State machine trigger    |
| `settings/mod.rs`           | Add `offline_mode` preference to `AppSettingsData`  | Settings extension       |
| `startup.rs`                | Compute initial offline readiness on app start      | Startup hook             |

## Data Models

### Offline Readiness State Machine

The offline readiness feature is governed by a state machine that tracks each profile's progression toward full offline capability:

```
Unconfigured ──[save profile + hash trainer]──► Hash Recorded
                                                    │
                              ┌──────────────────────┴───────────────────────┐
                              │                                              │
                    trainer_type = fling/standalone               trainer_type = aurora/wemod
                              │                                              │
                              ▼                                              ▼
                        Offline Ready                              Awaiting Activation
                              │                                              │
                              │                        [user confirms offline activation]
                              │                                              │
                              │                                              ▼
                              │                                        Offline Ready
                              │                                              │
                              └──────────────┬───────────────────────────────┘
                                             │
                              [TrainerChanged from version_store]
                                             │
                                             ▼
                                        Hash Stale
                                             │
                                    [re-hash trainer]
                                             │
                                             ▼
                                      Hash Recorded (restart)
```

Valid `readiness_state` values: `unconfigured`, `hash_recorded`, `awaiting_activation`, `offline_ready`, `hash_stale`

**Integration with existing `VersionCorrelationStatus`**: When `version_store::compute_correlation_status()` returns `TrainerChanged`, the offline readiness state transitions to `hash_stale`. This reuses the existing version tracking infrastructure rather than duplicating change detection.

### Portable vs. Machine-Local Data Split

A critical design decision (aligned with business analysis):

| Data                                               | Storage         | Rationale                                                                |
| -------------------------------------------------- | --------------- | ------------------------------------------------------------------------ |
| `trainer_type` (fling/aurora/wemod/etc.)           | Profile TOML    | Portable -- travels with community exports, describes the trainer itself |
| `trainer_activated` (offline activation confirmed) | SQLite metadata | Machine-local -- Aurora activates per-device, not per-profile-file       |
| `readiness_state` (state machine position)         | SQLite metadata | Machine-local -- depends on local file presence and activation state     |
| `trainer_hash` (SHA-256 of trainer .exe)           | SQLite metadata | Machine-local -- file content is machine-specific                        |

### SQLite Tables (Migration 13)

#### `trainer_hash_cache`

Caches SHA-256 hashes of trainer executables to avoid recomputation. The existing `hash_trainer_file()` in `version_store.rs` reads the entire file into memory -- this table caches results so the hash is only recomputed when file metadata changes.

**Bootstrap from existing data**: When the migration runs, the `version_snapshots.trainer_file_hash` column already contains hashes for profiles that have been launched. The migration or first startup scan can seed `trainer_hash_cache` from these existing hashes, avoiding a full re-hash pass for active profiles.

```sql
CREATE TABLE IF NOT EXISTS trainer_hash_cache (
    cache_id            TEXT PRIMARY KEY,
    profile_id          TEXT NOT NULL REFERENCES profiles(profile_id) ON DELETE CASCADE,
    file_path           TEXT NOT NULL,
    file_size           INTEGER NOT NULL,
    file_modified_at    TEXT NOT NULL,    -- ISO 8601 from fs::metadata().modified()
    sha256_hash         TEXT NOT NULL,    -- lowercase hex, 64 chars
    verified_at         TEXT NOT NULL,    -- last time the hash was confirmed valid
    created_at          TEXT NOT NULL,
    updated_at          TEXT NOT NULL
);
CREATE UNIQUE INDEX IF NOT EXISTS idx_trainer_hash_cache_profile_path
    ON trainer_hash_cache(profile_id, file_path);
CREATE INDEX IF NOT EXISTS idx_trainer_hash_cache_verified_at
    ON trainer_hash_cache(verified_at);
```

**Key design decisions:**

- `file_size` + `file_modified_at` form a fast-path cache validity check (stat-only, no read)
- `UNIQUE(profile_id, file_path)` prevents duplicate entries per profile/path combo
- `ON DELETE CASCADE` from `profiles` automatically cleans up when profiles are deleted
- `verified_at` enables staleness queries ("hashes not reverified in N days")

#### `offline_readiness_snapshots`

Caches the computed offline readiness score and state machine position per profile, similar to `health_snapshots`. Includes activation state because it is **machine-local** (Aurora activates per-device).

```sql
CREATE TABLE IF NOT EXISTS offline_readiness_snapshots (
    profile_id              TEXT PRIMARY KEY REFERENCES profiles(profile_id) ON DELETE CASCADE,
    readiness_state         TEXT NOT NULL DEFAULT 'unconfigured',
    readiness_score         INTEGER NOT NULL,  -- 0-100
    trainer_type            TEXT NOT NULL DEFAULT 'unknown',
    trainer_present         INTEGER NOT NULL DEFAULT 0,
    trainer_hash_valid      INTEGER NOT NULL DEFAULT 0,
    trainer_activated       INTEGER NOT NULL DEFAULT 0,
    proton_available        INTEGER NOT NULL DEFAULT 0,
    community_tap_cached    INTEGER NOT NULL DEFAULT 0,
    network_required        INTEGER NOT NULL DEFAULT 0,
    blocking_reasons        TEXT,              -- JSON array of strings
    checked_at              TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_offline_readiness_checked_at
    ON offline_readiness_snapshots(checked_at);
```

**Key design decisions:**

- Single row per profile (PRIMARY KEY on profile_id), upserted on each check
- `readiness_state` tracks the state machine position (see above)
- `readiness_score` is a 0-100 integer for easy sorting and badge display
- `trainer_activated` is a boolean for Aurora/WeMod offline activation confirmation -- stored here (not TOML) because activation is per-device
- Individual boolean columns for fast filtering without JSON parsing
- `blocking_reasons` is a JSON array for the UI detail panel
- `ON DELETE CASCADE` for automatic cleanup

#### `community_tap_offline_state`

Companion table for offline cache state tracking. Uses the companion table pattern (not ALTER TABLE) following the precedent from migration 4->5 and 6->7 which required full table rebuilds.

```sql
CREATE TABLE IF NOT EXISTS community_tap_offline_state (
    tap_id              TEXT PRIMARY KEY REFERENCES community_taps(tap_id) ON DELETE CASCADE,
    cache_status        TEXT NOT NULL DEFAULT 'unknown',  -- 'cached', 'stale', 'missing', 'unknown'
    cached_at           TEXT,
    cached_profile_count INTEGER NOT NULL DEFAULT 0,
    checked_at          TEXT NOT NULL
);
```

**Note from business analysis**: No new caching logic is strictly needed -- tap workspaces already exist on disk after sync and `index_tap()` already handles missing workspaces gracefully. This table primarily tracks **freshness metadata** for the UI ("last synced: 3 days ago") rather than implementing new cache mechanics.

### Profile TOML Schema Extension

Extend `TrainerSection` in `profile/models.rs`:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum TrainerType {
    #[default]
    Unknown,
    /// FLiNG trainers: standalone .exe, fully offline-capable
    Fling,
    /// Aurora/WeMod trainers: require network activation or offline key
    Aurora,
    /// WeMod app-based trainers: require WeMod client (network-dependent)
    Wemod,
    /// Generic standalone trainers: offline-capable but unclassified
    Standalone,
    /// Custom/other trainers with unknown network requirements
    Custom,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct TrainerSection {
    #[serde(default)]
    pub path: String,
    #[serde(rename = "type", default)]
    pub kind: String,
    #[serde(rename = "loading_mode", default)]
    pub loading_mode: TrainerLoadingMode,
    /// Trainer type classification for offline readiness scoring.
    /// Defaults to `unknown` when not set. Backward-compatible with existing profiles.
    #[serde(rename = "trainer_type", default, skip_serializing_if = "TrainerType::is_unknown")]
    pub trainer_type: TrainerType,
}
```

**Backward compatibility**: `#[serde(default)]` ensures existing TOML profiles without `trainer_type` deserialize to `Unknown`. `skip_serializing_if` keeps existing profile files unchanged when the field is unset.

**Relationship to existing `kind` field**: The `kind` field (serde-renamed from `type`) is a free-form display string. The new `trainer_type` is a typed enum for programmatic classification. They coexist. When `trainer_type` is `Unknown` but `kind` contains "fling" (case-insensitive), the readiness logic can use `kind` as a heuristic hint to suggest classification.

**Not in TOML**: `offline_activated` does NOT go in the profile TOML. Per business analysis, activation is machine-local (Aurora activates per-device). It lives in `offline_readiness_snapshots.trainer_activated` in SQLite.

### Rust Model: `OfflineReadinessReport`

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum OfflineReadinessState {
    Unconfigured,
    HashRecorded,
    AwaitingActivation,
    OfflineReady,
    HashStale,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OfflineReadinessReport {
    pub profile_name: String,
    pub readiness_state: OfflineReadinessState,
    pub readiness_score: u8,           // 0-100
    pub trainer_type: TrainerType,
    pub checks: OfflineReadinessChecks,
    pub blocking_reasons: Vec<String>,
    pub checked_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OfflineReadinessChecks {
    pub trainer_present: bool,
    pub trainer_hash_valid: bool,
    pub trainer_activated: bool,       // Aurora/WeMod offline activation confirmed
    pub proton_available: bool,
    pub game_files_present: bool,
    pub prefix_exists: bool,
    pub community_tap_cached: bool,
    pub network_required: bool,        // true if trainer_type requires network
}
```

### TypeScript Types (Frontend)

```typescript
// types/offline.ts
export type TrainerType = 'unknown' | 'fling' | 'aurora' | 'wemod' | 'standalone' | 'custom';

export type OfflineReadinessState =
  | 'unconfigured'
  | 'hash_recorded'
  | 'awaiting_activation'
  | 'offline_ready'
  | 'hash_stale';

export interface OfflineReadinessReport {
  profile_name: string;
  readiness_state: OfflineReadinessState;
  readiness_score: number;
  trainer_type: TrainerType;
  checks: OfflineReadinessChecks;
  blocking_reasons: string[];
  checked_at: string;
}

export interface OfflineReadinessChecks {
  trainer_present: boolean;
  trainer_hash_valid: boolean;
  trainer_activated: boolean;
  proton_available: boolean;
  game_files_present: boolean;
  prefix_exists: boolean;
  community_tap_cached: boolean;
  network_required: boolean;
}
```

### Scoring Algorithm

The offline readiness score is a weighted composite:

| Check                  | Weight | Condition                                                                   |
| ---------------------- | ------ | --------------------------------------------------------------------------- |
| `trainer_present`      | 30     | Trainer .exe exists on disk                                                 |
| `trainer_hash_valid`   | 15     | SHA-256 matches cached hash                                                 |
| `game_files_present`   | 20     | Game executable exists                                                      |
| `proton_available`     | 15     | Proton path exists (proton_run/steam_applaunch)                             |
| `prefix_exists`        | 10     | WINEPREFIX directory exists                                                 |
| `network_not_required` | 10     | Trainer type is offline-capable (FLiNG, Standalone) OR activation confirmed |

**Score = sum of weights for passing checks. 100 = fully offline-ready.**

Trainer types that require network (Aurora, WeMod) can still reach 100 if `trainer_activated` is true (user confirmed offline activation works). Without activation, the `network_not_required` weight (10 points) is not earned, capping the score at 90.

### Validation Rules (extending existing `ValidationError` enum)

Per business analysis, the following validation rules apply at launch time:

- **Hash not recorded** -> Warning: "Launch the game once while online to record trainer hash for offline verification"
- **Aurora/WeMod, activation not confirmed** -> Warning: "This trainer type requires offline activation. Confirm offline activation in the profile settings."
- **Paths missing/broken** -> Reuse existing `check_profile_health()` results (already checks file/directory existence)
- **`VersionCorrelationStatus::TrainerChanged`** -> Warning: "Trainer file has changed since last hash -- offline verification may be stale"

## API Design

### New Tauri IPC Commands

#### `check_offline_readiness`

Computes offline readiness for a single profile.

```
Command: check_offline_readiness
Parameters: { name: string }
Returns: Result<OfflineReadinessReport, String>
Errors:
  - "profile not found: {name}"
  - "failed to compute offline readiness: {detail}"
```

#### `batch_offline_readiness`

Computes offline readiness for all profiles (batch, for health dashboard).

```
Command: batch_offline_readiness
Parameters: (none)
Returns: Result<Vec<OfflineReadinessReport>, String>
Errors:
  - "failed to compute batch offline readiness: {detail}"
```

#### `verify_trainer_hash`

Computes and caches the SHA-256 hash for a trainer executable.

```
Command: verify_trainer_hash
Parameters: { profile_name: string, trainer_path: string }
Returns: Result<TrainerHashResult, String>
  TrainerHashResult: {
    sha256: string,         // 64-char hex
    file_size: number,
    cached: boolean,        // true if hash was served from cache
    verified_at: string,    // ISO 8601
  }
Errors:
  - "trainer file not found: {path}"
  - "failed to hash trainer file: {detail}"
  - "trainer file too large for inline hashing (>{limit} bytes)"
```

#### `check_network_status`

Probes network connectivity.

```
Command: check_network_status
Parameters: (none)
Returns: Result<NetworkStatus, String>
  NetworkStatus: {
    connected: boolean,
    method: string,         // "dns_probe" | "http_probe" | "unknown"
    checked_at: string,
  }
Errors:
  - "network check failed: {detail}"
```

#### `get_trainer_hash_cache`

Returns cached hash info for a profile's trainer without recomputing.

```
Command: get_trainer_hash_cache
Parameters: { profile_name: string }
Returns: Result<Option<TrainerHashCacheEntry>, String>
  TrainerHashCacheEntry: {
    sha256: string,
    file_size: number,
    file_modified_at: string,
    verified_at: string,
  }
Errors:
  - "profile not found in metadata: {name}"
```

#### `confirm_offline_activation`

Records that the user has confirmed offline activation works for an Aurora/WeMod trainer. Transitions the readiness state from `awaiting_activation` to `offline_ready`.

```
Command: confirm_offline_activation
Parameters: { profile_name: string }
Returns: Result<OfflineReadinessReport, String>
Errors:
  - "profile not found: {name}"
  - "trainer type does not require activation"
```

### Modified Existing Commands

#### `validate_launch` (extended)

Add new `ValidationError` variant:

```rust
/// Profile is not offline-ready and network is unavailable.
OfflineReadinessInsufficient {
    score: u8,
    readiness_state: OfflineReadinessState,
    blocking_reasons: Vec<String>,
},
```

Severity: **Warning** (not Fatal). The launch proceeds but the user sees the issues. This follows the existing pattern where `GamescopeNestedSession` is a Warning that surfaces information but does not block.

#### `launch_game` / `launch_trainer` (extended)

Before launching, if network is unavailable AND trainer_type requires network (Aurora, WeMod):

- Emit a `launch-offline-warning` Tauri event with the blocking reasons
- Log the offline state to the launch operation diagnostics
- Proceed with launch (fail-soft) -- the trainer itself will fail if it truly needs network

This follows the existing pattern in `script_runner.rs` where the command is built and executed, with failures reported post-hoc via the `DiagnosticReport`.

## System Constraints

### Performance

1. **SHA-256 hashing**: The existing `hash_trainer_file()` reads the entire file into memory (`std::fs::read`). For large trainers (some FLiNG trainers are 10-50 MB), this is fine. For very large trainer bundles (100+ MB), switch to streaming:

```rust
pub fn hash_trainer_file_streaming(path: &Path) -> std::io::Result<String> {
    use sha2::{Digest, Sha256};
    use std::io::Read;
    let mut file = std::fs::File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 8192];
    loop {
        let bytes_read = file.read(&mut buffer)?;
        if bytes_read == 0 { break; }
        hasher.update(&buffer[..bytes_read]);
    }
    let digest = hasher.finalize();
    let mut hex = String::with_capacity(64);
    for byte in digest { let _ = std::fmt::Write::write_fmt(&mut hex, format_args!("{byte:02x}")); }
    Ok(hex)
}
```

**Decision**: Use the streaming approach for the new `hash.rs` module. Keep `version_store::hash_trainer_file()` unchanged (it returns `Option<String>` and is used in many places). The new module provides a `Result`-based API.

2. **Cache invalidation**: Use `file_size` + `file_modified_at` from `fs::metadata()` as a fast-path check (single stat syscall). Only recompute the full SHA-256 if the stat data differs from the cached values. This makes batch offline readiness checks near-instant for unchanged files.

3. **Batch operations**: `batch_offline_readiness` should prefetch all metadata in one pass (like `prefetch_batch_metadata` in `commands/health.rs`), then run checks profile-by-profile. This avoids N+1 queries.

4. **Bootstrap from `version_snapshots`**: On first run after migration, seed `trainer_hash_cache` from existing `version_snapshots.trainer_file_hash` values where available. This avoids a full re-hash pass for profiles that have already been launched.

### Steam Deck Storage

- Steam Deck has 64 GB / 256 GB / 512 GB internal storage plus SD card
- Community tap git clones should be shallow (`--depth 1`) for initial clone -- the existing `clone_tap()` does not use `--depth`, which wastes space for taps with long history
- Hash cache entries are small (~200 bytes each); even 1000 profiles produce negligible DB growth
- Offline readiness snapshots follow the same compact pattern as `health_snapshots`

**Recommendation**: Add `--depth 1` to the git clone in `taps.rs` for new clones. For existing clones, do not force-shallow (could break pinned commit checkout).

### Network Detection

Linux network detection options (ordered by reliability):

1. **`/sys/class/net/` interface check**: Fast, no external calls, but unreliable (interface can be up without connectivity)
2. **DNS probe**: `std::net::ToSocketAddrs::to_socket_addrs("dns.google:53")` -- fast, minimal overhead, works through most proxies
3. **HTTP probe**: `GET http://connectivity-check.ubuntu.com/` or equivalent -- most reliable, but slowest and requires an HTTP client dependency

**Recommendation**: Use DNS probe as primary (no new dependencies, fast). Fall back to interface check if DNS probe times out. Do NOT add an HTTP client dependency for this alone. The probe should have a 2-second timeout to avoid blocking the UI.

Implementation note: Use `std::net::TcpStream::connect_timeout()` to `1.1.1.1:53` or `8.8.8.8:53` with a 2-second timeout. This does not require DNS resolution itself and works even when DNS is broken.

### Concurrency

- Hash computation is CPU-bound. For batch operations, use `tokio::task::spawn_blocking` to avoid blocking the Tauri async runtime.
- Network probes must be async with timeout to avoid UI freezes.
- SQLite writes are serialized through the existing `Arc<Mutex<Connection>>` in `MetadataStore` -- no new concurrency concerns.

## Codebase Changes

### Files to Create

| Path                                                  | Purpose                                           |
| ----------------------------------------------------- | ------------------------------------------------- |
| `crates/crosshook-core/src/offline/mod.rs`            | Module root, re-exports                           |
| `crates/crosshook-core/src/offline/trainer_type.rs`   | `TrainerType` enum, classification heuristics     |
| `crates/crosshook-core/src/offline/readiness.rs`      | Offline readiness scoring logic and state machine |
| `crates/crosshook-core/src/offline/network.rs`        | Network connectivity probe                        |
| `crates/crosshook-core/src/offline/hash.rs`           | Streaming SHA-256 with cache integration          |
| `crates/crosshook-core/src/metadata/offline_store.rs` | SQLite CRUD for offline tables                    |
| `src-tauri/src/commands/offline.rs`                   | Tauri IPC command handlers                        |
| `src/types/offline.ts`                                | TypeScript type definitions                       |
| `src/components/OfflineStatusBadge.tsx`               | Inline readiness badge                            |
| `src/components/OfflineReadinessPanel.tsx`            | Detail panel in health dashboard                  |

### Files to Modify

| Path                                               | Change                                                                     |
| -------------------------------------------------- | -------------------------------------------------------------------------- |
| `crates/crosshook-core/src/lib.rs`                 | Add `pub mod offline;`                                                     |
| `crates/crosshook-core/src/profile/models.rs`      | Add `TrainerType` enum, extend `TrainerSection`                            |
| `crates/crosshook-core/src/profile/health.rs`      | Inject offline readiness into health checks                                |
| `crates/crosshook-core/src/metadata/migrations.rs` | Add `migrate_12_to_13()`                                                   |
| `crates/crosshook-core/src/metadata/mod.rs`        | Register `offline_store`, expose new functions                             |
| `crates/crosshook-core/src/launch/request.rs`      | Add `OfflineReadinessInsufficient` variant                                 |
| `crates/crosshook-core/src/community/taps.rs`      | Add offline-aware sync fallback                                            |
| `crates/crosshook-core/src/settings/mod.rs`        | Add `offline_mode` to `AppSettingsData`                                    |
| `src-tauri/src/commands/mod.rs`                    | Register `offline` command module                                          |
| `src-tauri/src/commands/launch.rs`                 | Add pre-flight offline check                                               |
| `src-tauri/src/lib.rs`                             | Register new IPC commands                                                  |
| `src-tauri/src/startup.rs`                         | Run initial offline readiness scan, seed hash cache from version_snapshots |
| `src/types/index.ts`                               | Re-export offline types                                                    |
| `src/types/profile.ts`                             | Add `trainer_type` to `GameProfile.trainer`                                |
| `src/components/pages/LaunchPage.tsx`              | Show offline readiness pre-flight                                          |
| `src/components/pages/ProfilesPage.tsx`            | Add trainer type selector                                                  |
| `src/components/pages/HealthDashboardPage.tsx`     | Show offline readiness column                                              |

### Dependencies

No new crate dependencies required:

- `sha2` already in `Cargo.toml` (used by `version_store.rs`)
- `chrono` already available
- `serde` / `serde_json` already available
- Network probe uses only `std::net` (no async HTTP client needed)

## Technical Decisions

### 1. TrainerType Storage Location

**Options:**

- (A) Profile TOML field (`trainer.trainer_type`)
- (B) SQLite-only metadata (separate from profile)
- (C) Auto-detected at runtime from trainer executable analysis

**Recommendation: (A) Profile TOML field.**

**Rationale:** TrainerType is a user-facing classification that should travel with the profile (community sharing, portability). Auto-detection (C) is unreliable since FLiNG and Aurora trainers are both Windows .exe files with no guaranteed distinguishing markers. SQLite-only (B) would be lost on metadata DB reset. TOML persistence with `#[serde(default)]` ensures backward compatibility.

### 2. Activation State Storage Location

**Options:**

- (A) Profile TOML field (`trainer.offline_activated`)
- (B) SQLite metadata (`offline_readiness_snapshots.trainer_activated`)

**Recommendation: (B) SQLite metadata.**

**Rationale (from business analysis):** Activation is **machine-local**. Aurora activates per-device, not per-profile-file. If activation were stored in TOML, exporting a community profile would falsely claim the trainer is activated on the recipient's device. SQLite keeps activation state tied to the specific machine where it was confirmed.

### 3. Hash Cache vs. Inline Computation

**Options:**

- (A) Always compute SHA-256 on demand (current `hash_trainer_file` behavior)
- (B) Cache in SQLite with stat-based invalidation
- (C) Cache in a sidecar `.sha256` file next to the trainer

**Recommendation: (B) SQLite cache with stat-based invalidation.**

**Rationale:** Stat check (size + mtime) is a single syscall vs. reading the entire file. The existing `version_snapshots.trainer_file_hash` already stores hashes but only for version correlation, not readiness scoring. A dedicated cache table allows independent lifecycle management and avoids coupling offline readiness to version tracking. Sidecar files (C) would litter user directories. The existing version snapshot hashes can bootstrap the cache on first run.

### 4. Network Detection Strategy

**Options:**

- (A) DNS probe to well-known resolver (1.1.1.1:53)
- (B) HTTP probe to connectivity check endpoint
- (C) NetworkManager D-Bus integration
- (D) `/sys/class/net/` interface scanning

**Recommendation: (A) DNS probe with (D) fallback.**

**Rationale:** DNS probe is fast (2s timeout), requires no new dependencies, works through NATs and proxies. NetworkManager D-Bus (C) is desktop-specific and may not be present on all Steam Deck configurations. HTTP probe (B) requires an HTTP client dependency. Interface scanning (D) is unreliable alone but useful as a fast negative check ("no interfaces up" = definitely offline).

### 5. Offline Launch Behavior

**Options:**

- (A) Block launch entirely when offline readiness < threshold
- (B) Warn but allow launch (fail-soft)
- (C) Degrade gracefully: launch game without trainer if trainer needs network

**Recommendation: (B) Warn but allow launch.**

**Rationale:** CrossHook's validation philosophy (see `ValidationSeverity::Warning` pattern) is to inform, not block. Users on Steam Deck may know their trainer works offline even if the type classification says otherwise. The launch log (`DiagnosticReport`) captures the offline state for post-hoc analysis.

### 6. Community Tap Offline Strategy

**Options:**

- (A) Full offline mirror with periodic background sync
- (B) Serve from existing git clone on disk (already offline-capable)
- (C) Export tap index to SQLite for offline browsing

**Recommendation: (B) + existing SQLite index.**

**Rationale:** The existing `CommunityTapStore` already clones taps to `~/.local/share/crosshook/community/taps/`. These clones ARE the offline cache -- they persist on disk and the index can be rebuilt from local files without network. The `community_profiles` SQLite table already indexes tap contents. The only change needed is: when `community_sync` fails due to network, fall back to the existing local clone and existing SQLite index instead of returning an error. Track cache freshness in `community_tap_offline_state` so the UI can show "last synced: 3 days ago". Per business analysis, `index_tap()` already handles missing workspaces gracefully -- no new caching logic required.

### 7. Migration Strategy

**Options:**

- (A) Single migration (v12 -> v13) with all new tables
- (B) Multiple migrations (v13, v14, v15) for incremental rollout

**Recommendation: (A) Single migration v12 -> v13.**

**Rationale:** All three tables ship together as one feature. The migration system uses sequential version numbers and each migration runs at most once. Splitting into multiple migrations adds no benefit since the feature is atomic.

## Edge Cases and Gotchas

1. **`hash_trainer_file()` reads entire file into memory**: The existing function in `version_store.rs` does `std::fs::read(path)`. For the offline hash cache, use streaming to handle large trainer bundles without memory pressure on Steam Deck (4-16 GB RAM).

2. **Trainer path resolution with `local_override`**: The effective trainer path comes from `effective_profile()` which merges `local_override.trainer.path` over `trainer.path`. All offline checks must use the effective profile, not the raw stored profile. The pattern is already established in `health.rs` and `commands/health.rs`.

3. **Community taps use `git reset --hard` and `git clean -fdx`**: The `fetch_and_reset()` method destroys any local modifications. This is intentional for tap integrity but means offline modifications to cached taps are not possible. The offline strategy should never modify tap working trees.

4. **`MetadataStore` may be disabled**: The `MetadataStore::disabled()` path (no SQLite) is a supported configuration. All offline readiness checks must degrade gracefully when `is_available()` returns false, similar to how `commands/health.rs` handles `!metadata_store.is_available()`.

5. **Steam Deck sleep/wake cycle**: Network state changes frequently when the Deck sleeps and wakes. The network probe result should have a short TTL (e.g., 30 seconds) and be rechecked before each launch, not cached for the session.

6. **Proton path may not exist until first launch**: For `steam_applaunch` profiles, the Proton path may point to a Steam-managed directory that only appears after the game is launched once through Steam. The offline readiness check should treat this as a known "warning" state, not a blocking error.

7. **`TrainerSection.kind` vs. `trainer_type`**: The existing `kind` field (renamed from `type` in serde) is a free-form string used for display purposes. The new `trainer_type` is a typed enum for programmatic classification. They serve different purposes and should coexist. When `trainer_type` is `Unknown` but `kind` contains "fling" (case-insensitive), the readiness logic can use `kind` as a heuristic hint.

8. **File modification time resolution**: `fs::metadata().modified()` resolution varies by filesystem. ext4 has 1-second resolution, which means rapid trainer updates within the same second could produce stale cache hits. This is an acceptable trade-off -- the worst case is a one-second window of stale hash data.

9. **State machine transition on `TrainerChanged`**: The `VersionCorrelationStatus::TrainerChanged` signal from `version_store::compute_correlation_status()` should trigger a transition to `hash_stale`. This is computed at startup in `startup.rs` and on launch completion in `commands/launch.rs`. The offline readiness check should read the latest version correlation status and derive the state machine position accordingly.

10. **Activation persistence across profile rename/duplicate**: Since activation lives in `offline_readiness_snapshots` keyed by `profile_id`, it survives profile renames (which change filename but not ID). Profile duplication creates a new `profile_id`, so the duplicate starts in `unconfigured` state -- this is correct behavior since activation should be re-confirmed per-device for the new profile.

## Relevant Files

- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/profile/models.rs`: Profile data model, `TrainerSection` to extend
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/profile/health.rs`: Health check system to integrate offline readiness
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/launch/request.rs`: Launch validation pipeline to extend
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/launch/script_runner.rs`: Launch execution to add offline guards
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/metadata/migrations.rs`: Migration system (currently at v12)
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/metadata/version_store.rs`: Existing `hash_trainer_file()` and version tracking -- bootstrap source for hash cache
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/metadata/health_store.rs`: Health snapshot pattern to follow
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/metadata/mod.rs`: MetadataStore facade
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/community/taps.rs`: Community tap sync to make offline-aware
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/settings/mod.rs`: Settings to extend with offline preference
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src-tauri/src/commands/health.rs`: Health command pattern to follow for batch operations
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src-tauri/src/commands/community.rs`: Community commands to extend
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src-tauri/src/startup.rs`: Startup reconciliation to extend
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/types/profile.ts`: Frontend profile types to extend
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/types/health.ts`: Frontend health types pattern to follow

## Open Questions

1. **Trainer type auto-detection feasibility**: Can FLiNG trainers be reliably distinguished from Aurora trainers by file analysis (PE headers, embedded strings, file size patterns)? If so, auto-classification could supplement manual selection. Needs investigation of actual trainer binaries.

2. **Hash verification on SD card**: If trainers are stored on a Steam Deck SD card, should the hash cache account for card removal/replacement? Card-mounted paths could appear valid (path exists) but point to a different card.

3. **Background offline readiness refresh**: Should the app periodically recompute offline readiness in the background (like a watchdog), or only on user action (profile load, launch attempt, health dashboard visit)? Background refresh consumes battery on Steam Deck.

4. **Community tap sync retry strategy**: When offline, how many times should `community_sync` retry before falling back to cache? Currently the `git clone` has HTTP timeout settings (`GIT_HTTP_LOW_SPEED_LIMIT`) but no retry logic.

5. ~~**Offline activation key storage**: Aurora/WeMod trainers may support offline keys.~~ **Resolved**: Per business analysis, activation state is a simple boolean confirmation stored in SQLite (`trainer_activated`), not key material. The user confirms "I have activated this trainer for offline use" and the app records that fact. No cryptographic key storage needed for v1. If actual key management is needed later, it can be a follow-up feature.
