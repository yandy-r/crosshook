# Technical Specification: Trainer-Version Correlation

## Executive Summary

This document specifies the architecture, data models, APIs, and integration strategy for game/trainer version correlation with mismatch detection in CrossHook. The feature tracks relationships between Steam game build IDs and trainer versions, detects mismatches after game updates, warns users when trainers may be incompatible, and records version history for successful launch combinations.

The design leverages the existing SQLite metadata database (schema v8) with a new migration to v9, follows the established `MetadataStore` facade pattern, extends `parse_manifest()` to extract `buildid` from Steam ACF files, and surfaces mismatch data through the existing health scoring pipeline rather than creating a parallel notification subsystem.

**Revision note (R2)**: Incorporates full cross-team consensus from all research dimensions — business analysis, external API research, UX patterns, security review, engineering practices, and architectural recommendations. Key changes across revisions: simplified single-table schema (mirroring `health_snapshots`), extending `parse_manifest()` instead of creating a separate module, health-integrated mismatch surfacing, pure-function comparison logic for testability, A6 security bounds for version strings, W3 community trust model constraint, UX loading-state requirements, and `StateFlags` awareness for update-in-progress detection. `game_buildid` confirmed as local-only data (not portable to community profiles).

---

## Architecture Design

### Component Diagram

```
                                   ┌──────────────────────┐
                                   │   React Frontend     │
                                   │                      │
                                   │  HealthDashboard     │
                                   │  (version enrichment)│
                                   │  LaunchPanel         │
                                   │  (mismatch banner)   │
                                   └──────────┬───────────┘
                                              │ invoke() / listen()
                                              ▼
                                   ┌──────────────────────┐
                                   │  Tauri IPC Layer      │
                                   │                       │
                                   │  commands/version.rs   │
                                   │  commands/health.rs    │
                                   │  commands/launch.rs    │
                                   └──────────┬────────────┘
                                              │
                     ┌────────────────────────┼────────────────────────┐
                     │                        │                        │
                     ▼                        ▼                        ▼
          ┌──────────────────┐   ┌──────────────────────┐   ┌──────────────────┐
          │ steam/manifest.rs│   │ metadata/             │   │  launch/         │
          │ (extended)       │   │ version_store.rs      │   │  launch_history  │
          │                  │   │ (new)                 │   │  (existing)      │
          │ parse_manifest() │   │                       │   │                  │
          │ + buildid return │   │ upsert_version_snap() │   │ record_launch_   │
          └────────┬─────────┘   │ lookup_version_snap() │   │ finished() hook  │
                   │             │ compute_correlation() │   └──────────────────┘
                   ▼             └──────────┬─────────────┘
          ┌──────────────────┐              │
          │  steam/vdf.rs    │              ▼
          │  (existing)      │   ┌──────────────────────┐
          │  VDF parser      │   │  metadata.db (SQLite) │
          └──────────────────┘   │  version_snapshots    │
                                 └──────────────────────┘
```

### New Components

| Component                   | Location                              | Responsibility                                                                     |
| --------------------------- | ------------------------------------- | ---------------------------------------------------------------------------------- |
| `metadata/version_store.rs` | `crates/crosshook-core/src/metadata/` | SQLite CRUD for version snapshots, mismatch detection via pure comparison function |
| `commands/version.rs`       | `src-tauri/src/commands/`             | Tauri IPC command handlers for version checking, history, and manual version input |
| `types/version.ts`          | `src/types/`                          | TypeScript types for version data IPC payloads                                     |

### Design Principles (Team Consensus)

1. **Single-table schema**: One `version_snapshots` table modeled after `health_snapshots` (INSERT OR REPLACE per profile_id), NOT a multi-table history model. Simplicity > granularity.
2. **Extend, don't duplicate**: Extend existing `parse_manifest()` to also return `buildid` rather than creating a separate `steam/version.rs` module. One manifest parse, all fields.
3. **Health-integrated display**: Version mismatch data flows through the existing `EnrichedProfileHealthReport.metadata` enrichment pattern, not a parallel notification subsystem.
4. **Pure comparison logic**: Extract `compute_correlation_status()` as a pure function — keeps I/O in command handlers, logic unit-testable.
5. **Fail-soft everywhere**: Manifest read failures, missing snapshots, and MetadataStore unavailability never block launches.

### Integration Points

1. **Post-launch success** (`commands/launch.rs`) — After `record_launch_finished()` with `LaunchOutcome::Succeeded`, call `upsert_version_snapshot()` with current build ID
2. **Health enrichment** (`commands/health.rs`) — Query `version_snapshots` in batch prefetch, compare against current manifest build ID, add mismatch flag to `ProfileHealthMetadata`
3. **On-demand check** (`commands/version.rs`) — Explicit IPC command for frontend to request version status for a profile
4. **Startup reconciliation** (`startup.rs`) — Optional background scan of Steam manifests to detect changes since last session
5. **Community import** (`commands/community.rs`) — Seed `version_snapshots.human_game_ver` from community metadata

---

## Data Models

### Migration 8 to 9: Version Snapshots Table

```sql
-- Migration: migrate_8_to_9
-- Purpose: Version tracking — single-row snapshot per profile (INSERT OR REPLACE pattern)
-- Follows health_snapshots pattern from migration 5→6

CREATE TABLE IF NOT EXISTS version_snapshots (
    profile_id          TEXT PRIMARY KEY
                        REFERENCES profiles(profile_id) ON DELETE CASCADE,
    steam_app_id        TEXT,
    steam_build_id      TEXT,
    trainer_version     TEXT,
    trainer_file_hash   TEXT,
    human_game_ver      TEXT,
    status              TEXT NOT NULL DEFAULT 'untracked',
    checked_at          TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_version_snapshots_checked_at
    ON version_snapshots(checked_at);
CREATE INDEX IF NOT EXISTS idx_version_snapshots_steam_app_id
    ON version_snapshots(steam_app_id);
```

**Column semantics:**

| Column              | Type          | Description                                                                                                          |
| ------------------- | ------------- | -------------------------------------------------------------------------------------------------------------------- |
| `profile_id`        | TEXT PK, FK   | Links to `profiles.profile_id`. CASCADE delete. Single row per profile.                                              |
| `steam_app_id`      | TEXT NULL     | Steam App ID from profile (`steam.app_id`). NULL for non-Steam profiles.                                             |
| `steam_build_id`    | TEXT NULL     | Steam manifest `buildid` at last successful launch. NULL until first verified launch.                                |
| `trainer_version`   | TEXT NULL     | User-provided or community-sourced trainer version string.                                                           |
| `trainer_file_hash` | TEXT NULL     | SHA-256 hash of trainer executable file. Change detection signal. Uses existing `sha2` crate.                        |
| `human_game_ver`    | TEXT NULL     | Human-readable game version from community metadata (e.g., "1.12.3"). Display-only.                                  |
| `status`            | TEXT NOT NULL | Correlation status: `'untracked'`, `'matched'`, `'game_updated'`, `'trainer_changed'`, `'both_changed'`, `'unknown'` |
| `checked_at`        | TEXT NOT NULL | RFC 3339 timestamp of last check/update.                                                                             |

**Key design notes:**

- `INSERT OR REPLACE` pattern (same as `health_snapshots`) — no unbounded history growth
- `steam_build_id` is NULL until first successful local launch (community imports start with NULL build ID, `human_game_ver` from community metadata)
- "No snapshot" (`status = 'untracked'`) is NOT a mismatch — it means version tracking has not been activated for this profile
- Only profiles with `steam.app_id` populated participate in automated build ID tracking; `native` method profiles are silently skipped

### Rust Implementation

```rust
// metadata/version_store.rs — mirrors health_store.rs pattern

use super::MetadataStoreError;
use rusqlite::{params, Connection, OptionalExtension};

/// Defensive storage cap for version strings (A6 security bound).
pub const MAX_VERSION_STRING_BYTES: usize = 256;

/// Correlation status between stored and current versions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VersionCorrelationStatus {
    Untracked,
    Matched,
    GameUpdated,
    TrainerChanged,
    BothChanged,
    Unknown,
}

impl VersionCorrelationStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Untracked => "untracked",
            Self::Matched => "matched",
            Self::GameUpdated => "game_updated",
            Self::TrainerChanged => "trainer_changed",
            Self::BothChanged => "both_changed",
            Self::Unknown => "unknown",
        }
    }
}

#[derive(Debug, Clone)]
pub struct VersionSnapshotRow {
    pub profile_id: String,
    pub profile_name: String,  // Populated via JOIN
    pub steam_app_id: Option<String>,
    pub steam_build_id: Option<String>,
    pub trainer_version: Option<String>,
    pub trainer_file_hash: Option<String>,
    pub human_game_ver: Option<String>,
    pub status: String,
    pub checked_at: String,
}

/// Pure function: compare stored snapshot vs. current state.
/// No I/O — suitable for unit testing.
pub fn compute_correlation_status(
    stored_build_id: Option<&str>,
    current_build_id: Option<&str>,
    stored_trainer_hash: Option<&str>,
    current_trainer_hash: Option<&str>,
) -> VersionCorrelationStatus {
    let build_changed = match (stored_build_id, current_build_id) {
        (Some(stored), Some(current)) => stored != current,
        (None, _) | (_, None) => false,  // Can't detect change without both values
    };

    let trainer_changed = match (stored_trainer_hash, current_trainer_hash) {
        (Some(stored), Some(current)) => stored != current,
        (None, _) | (_, None) => false,
    };

    match (build_changed, trainer_changed) {
        (true, true) => VersionCorrelationStatus::BothChanged,
        (true, false) => VersionCorrelationStatus::GameUpdated,
        (false, true) => VersionCorrelationStatus::TrainerChanged,
        (false, false) => VersionCorrelationStatus::Matched,
    }
}

pub fn upsert_version_snapshot(
    conn: &Connection,
    profile_id: &str,
    steam_app_id: Option<&str>,
    steam_build_id: Option<&str>,
    trainer_version: Option<&str>,
    trainer_file_hash: Option<&str>,
    human_game_ver: Option<&str>,
    status: &str,
    checked_at: &str,
) -> Result<(), MetadataStoreError> {
    // A6 bounds validation
    if let Some(v) = trainer_version {
        if v.len() > MAX_VERSION_STRING_BYTES {
            return Err(MetadataStoreError::Validation(format!(
                "trainer_version exceeds {MAX_VERSION_STRING_BYTES} bytes"
            )));
        }
    }
    if let Some(v) = human_game_ver {
        if v.len() > MAX_VERSION_STRING_BYTES {
            return Err(MetadataStoreError::Validation(format!(
                "human_game_ver exceeds {MAX_VERSION_STRING_BYTES} bytes"
            )));
        }
    }
    // Validate steam_build_id is numeric-only (security: prevent injection via crafted ACF)
    if let Some(bid) = steam_build_id {
        if !bid.chars().all(|c| c.is_ascii_digit()) {
            return Err(MetadataStoreError::Validation(format!(
                "steam_build_id contains non-numeric characters: '{bid}'"
            )));
        }
    }

    conn.execute(
        "INSERT OR REPLACE INTO version_snapshots
            (profile_id, steam_app_id, steam_build_id, trainer_version,
             trainer_file_hash, human_game_ver, status, checked_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        params![
            profile_id, steam_app_id, steam_build_id, trainer_version,
            trainer_file_hash, human_game_ver, status, checked_at,
        ],
    )
    .map_err(|source| MetadataStoreError::Database {
        action: "upsert a version snapshot row",
        source,
    })?;

    Ok(())
}

pub fn lookup_version_snapshot(
    conn: &Connection,
    profile_id: &str,
) -> Result<Option<VersionSnapshotRow>, MetadataStoreError> {
    conn.query_row(
        "SELECT vs.profile_id, p.current_filename, vs.steam_app_id,
                vs.steam_build_id, vs.trainer_version, vs.trainer_file_hash,
                vs.human_game_ver, vs.status, vs.checked_at
         FROM version_snapshots vs
         INNER JOIN profiles p ON vs.profile_id = p.profile_id
         WHERE vs.profile_id = ?1 AND p.deleted_at IS NULL",
        params![profile_id],
        |row| {
            Ok(VersionSnapshotRow {
                profile_id: row.get(0)?,
                profile_name: row.get(1)?,
                steam_app_id: row.get(2)?,
                steam_build_id: row.get(3)?,
                trainer_version: row.get(4)?,
                trainer_file_hash: row.get(5)?,
                human_game_ver: row.get(6)?,
                status: row.get(7)?,
                checked_at: row.get(8)?,
            })
        },
    )
    .optional()
    .map_err(|source| MetadataStoreError::Database {
        action: "look up version snapshot by profile_id",
        source,
    })
}

pub fn load_version_snapshots(
    conn: &Connection,
) -> Result<Vec<VersionSnapshotRow>, MetadataStoreError> {
    let mut stmt = conn
        .prepare(
            "SELECT vs.profile_id, p.current_filename, vs.steam_app_id,
                    vs.steam_build_id, vs.trainer_version, vs.trainer_file_hash,
                    vs.human_game_ver, vs.status, vs.checked_at
             FROM version_snapshots vs
             INNER JOIN profiles p ON vs.profile_id = p.profile_id
             WHERE p.deleted_at IS NULL",
        )
        .map_err(|source| MetadataStoreError::Database {
            action: "prepare load version snapshots query",
            source,
        })?;

    let rows = stmt
        .query_map([], |row| {
            Ok(VersionSnapshotRow {
                profile_id: row.get(0)?,
                profile_name: row.get(1)?,
                steam_app_id: row.get(2)?,
                steam_build_id: row.get(3)?,
                trainer_version: row.get(4)?,
                trainer_file_hash: row.get(5)?,
                human_game_ver: row.get(6)?,
                status: row.get(7)?,
                checked_at: row.get(8)?,
            })
        })
        .map_err(|source| MetadataStoreError::Database {
            action: "query version snapshots",
            source,
        })?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|source| MetadataStoreError::Database {
            action: "collect version snapshot rows",
            source,
        })?;

    Ok(rows)
}
```

### Extended Steam Manifest Parsing

Extend `parse_manifest()` in `steam/manifest.rs` to also return `buildid`, `LastUpdated`, and `StateFlags`:

```rust
// steam/manifest.rs — MODIFIED (not new file)

/// Extended manifest data including version info.
pub struct ManifestData {
    pub app_id: String,
    pub install_dir: String,
    pub build_id: String,             // NEW — monotonically increasing integer (opaque string)
    pub last_updated: Option<String>, // NEW — Unix timestamp (u64 as string)
    pub state_flags: Option<u32>,     // NEW — 4 = fully installed, 1026 = update in progress
}

fn parse_manifest(manifest_path: &Path) -> Result<ManifestData, String> {
    let content = fs::read_to_string(manifest_path)
        .map_err(|error| format!("unable to read manifest: {error}"))?;
    let manifest_root = parse_vdf(&content).map_err(|error| error.to_string())?;
    let app_state_node = manifest_root
        .get_child("AppState")
        .unwrap_or(&manifest_root);

    let steam_app_id = app_state_node
        .get_child("appid")
        .and_then(|node| node.value.as_ref())
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .or_else(|| extract_app_id_from_manifest_path(manifest_path))
        .unwrap_or_default();

    let install_dir_name = app_state_node
        .get_child("installdir")
        .and_then(|node| node.value.as_ref())
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_default();

    // NEW: extract buildid
    let build_id = app_state_node
        .get_child("buildid")
        .and_then(|node| node.value.as_ref())
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty())
        .unwrap_or_default();

    // NEW: extract LastUpdated
    let last_updated = app_state_node
        .get_child("LastUpdated")
        .and_then(|node| node.value.as_ref())
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());

    // NEW: extract StateFlags (4 = fully installed, 1026 = update in progress)
    let state_flags = app_state_node
        .get_child("StateFlags")
        .and_then(|node| node.value.as_ref())
        .and_then(|value| value.trim().parse::<u32>().ok());

    Ok(ManifestData {
        app_id: steam_app_id,
        install_dir: install_dir_name,
        build_id,
        last_updated,
        state_flags,
    })
}
```

**Alternative approach (preserves backward compatibility):** If changing the return type of `parse_manifest()` introduces too many caller changes, add a parallel `parse_manifest_full()` that returns `ManifestData` while keeping the original `(String, String)` return for `find_game_match()`. The practices researcher recommends extending in-place — evaluate during implementation.

**API research confirmation:** `buildid` is a monotonically-increasing opaque integer stored as string. It is a **local machine artifact** with no portable meaning across users — it belongs in the local `version_snapshots` table, NOT in `CommunityProfileMetadata`. Integer comparison (`!=`) is sufficient for mismatch detection; no semver library is needed. `LastUpdated` is a Unix timestamp (u64). `StateFlags=4` means fully installed; `StateFlags=1026` means update in progress — the scan should skip non-4 states to avoid false mismatch alerts during Steam auto-updates.

### IPC Serialization Models

```rust
// Used in Tauri commands — Serialize for frontend

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VersionCheckResult {
    pub profile_name: String,
    pub steam_app_id: Option<String>,
    pub current_build_id: Option<String>,
    pub snapshot_build_id: Option<String>,
    pub trainer_version: Option<String>,
    pub human_game_ver: Option<String>,
    pub status: String,            // VersionCorrelationStatus as string
    pub has_mismatch: bool,
    pub checked_at: String,
    pub update_in_progress: bool,  // true when StateFlags != 4
}
```

**Frontend display mapping** (from UX research):

- `has_mismatch: true` → warning banner with delta: `"Build {snapshot_build_id} → {current_build_id}"`
- `update_in_progress: true` → info banner: `"Steam update in progress — version check deferred"`
- `status: "untracked"` → no badge / neutral state
- `checked_at` → tooltip via existing `formatRelativeTime`: `"Last checked: 5 min ago"`

### TypeScript Types

```typescript
// types/version.ts

export type VersionCorrelationStatus =
  | 'untracked'
  | 'matched'
  | 'game_updated'
  | 'trainer_changed'
  | 'both_changed'
  | 'unknown';

export interface VersionCheckResult {
  profile_name: string;
  steam_app_id: string | null;
  current_build_id: string | null;
  snapshot_build_id: string | null;
  trainer_version: string | null;
  human_game_ver: string | null;
  status: VersionCorrelationStatus;
  has_mismatch: boolean;
  checked_at: string;
  update_in_progress: boolean;
}

export interface VersionSnapshotInfo {
  profile_name: string;
  steam_build_id: string | null;
  trainer_version: string | null;
  human_game_ver: string | null;
  status: VersionCorrelationStatus;
  checked_at: string;
}
```

---

## API Design

### Tauri IPC Commands

#### 1. `check_version_status`

On-demand version check for a single profile. Reads current manifest build ID, compares against stored snapshot.

```rust
#[tauri::command]
pub fn check_version_status(
    name: String,
    store: State<'_, ProfileStore>,
    metadata_store: State<'_, MetadataStore>,
) -> Result<VersionCheckResult, String>
```

- **Flow**: Load profile -> get `steam.app_id` -> locate manifest via `find_game_match()` -> `parse_manifest()` for `build_id` + `state_flags` -> skip if `state_flags != 4` (update in progress) -> `lookup_version_snapshot()` -> `compute_correlation_status()` -> return result
- **Fail-soft**: If manifest read fails or MetadataStore unavailable, return `status: "unknown"` with `has_mismatch: false`
- **Non-Steam profiles**: Return `status: "untracked"` immediately (no manifest to read)
- **Update in progress**: If `state_flags != Some(4)`, return `status: "unknown"` with a note that Steam update is in progress — do not flag as mismatch

#### 2. `set_trainer_version`

User manually sets the trainer version string for a profile.

```rust
#[tauri::command]
pub fn set_trainer_version(
    name: String,
    trainer_version: String,
    metadata_store: State<'_, MetadataStore>,
) -> Result<(), String>
```

- **Flow**: Look up `profile_id` -> `upsert_version_snapshot()` updating only `trainer_version` field
- **Validation**: A6 bounds check (MAX_VERSION_STRING_BYTES = 256)

#### 3. `get_version_snapshot`

Retrieve the current version snapshot for a profile.

```rust
#[tauri::command]
pub fn get_version_snapshot(
    name: String,
    metadata_store: State<'_, MetadataStore>,
) -> Result<Option<VersionSnapshotInfo>, String>
```

#### 4. `acknowledge_version_change`

User acknowledges a detected version change, resetting status to `matched`.

```rust
#[tauri::command]
pub fn acknowledge_version_change(
    name: String,
    metadata_store: State<'_, MetadataStore>,
) -> Result<(), String>
```

- **Flow**: Update `version_snapshots.status` to `'matched'` for this profile. This accepts the current state as the new baseline.

### Tauri Events

| Event Name              | Payload                             | Trigger                            |
| ----------------------- | ----------------------------------- | ---------------------------------- |
| `version-scan-complete` | `{ scanned: u32, mismatches: u32 }` | When startup version scan finishes |

Note: No `version-mismatch-detected` event needed — mismatches are surfaced through the health enrichment pipeline on the next `batch_validate_profiles` call or `get_profile_health` call.

---

## System Integration

### Post-Launch Success Hook (Primary Integration)

The primary version capture happens AFTER a successful launch, not before. This avoids adding latency to the launch path and ensures we only record versions that actually worked.

**In `commands/launch.rs::stream_log_lines()`**, after `record_launch_finished()`:

```rust
// After the existing record_launch_finished call with Succeeded outcome:
if outcome == LaunchOutcome::Succeeded {
    if let Some(ref profile_name) = request.profile_name {
        // Capture version snapshot for successful launch
        let ms = metadata_store.clone();
        let pn = profile_name.clone();
        let steam_app_id = request.steam.app_id.clone();
        tauri::async_runtime::spawn_blocking(move || {
            if let Err(e) = record_successful_launch_version(&ms, &pn, &steam_app_id) {
                tracing::warn!(%e, profile_name = %pn, "version snapshot after launch failed");
            }
        });
    }
}
```

The `record_successful_launch_version()` helper:

1. Look up `profile_id` from profile name
2. If `steam_app_id` is non-empty, locate and parse the manifest to get current `build_id`
3. Compute trainer file hash via `sha2` (already in Cargo.toml) on the trainer path
4. Call `upsert_version_snapshot()` with status `'matched'`

### Health Scoring Integration

Extend `commands/health.rs::ProfileHealthMetadata`:

```rust
pub struct ProfileHealthMetadata {
    // ... existing fields ...
    pub version_status: Option<String>,    // VersionCorrelationStatus as string, or None if untracked
    pub version_build_id: Option<String>,  // Current snapshot build ID for display
    pub version_checked_at: Option<String>,
}
```

Extend `commands/health.rs::BatchMetadataPrefetch`:

```rust
struct BatchMetadataPrefetch {
    // ... existing fields ...
    version_snapshot_map: HashMap<String, VersionSnapshotRow>,
}
```

In `prefetch_batch_metadata()`:

```rust
let version_snapshot_map: HashMap<String, VersionSnapshotRow> = metadata_store
    .load_version_snapshots()
    .unwrap_or_default()
    .into_iter()
    .map(|row| (row.profile_id.clone(), row))
    .collect();
```

In `enrich_profile()`:

```rust
let version_status = profile_id
    .as_deref()
    .and_then(|pid| prefetch.version_snapshot_map.get(pid))
    .map(|snap| snap.status.clone());
// ... populate metadata.version_status, version_build_id, version_checked_at
```

### Startup Reconciliation (Background Scan)

In `startup.rs::run_metadata_reconciliation()`, after existing sweep:

```rust
// Optional: scan Steam manifests for build ID changes since last session
if let Err(error) = scan_version_updates(&metadata_store, &profile_store) {
    tracing::warn!(%error, "startup version scan failed");
}
```

The scan:

1. Load all `version_snapshots` that have a `steam_app_id`
2. For each, locate the current appmanifest and call `parse_manifest()` for `buildid` and `state_flags`
3. **Skip** manifests where `state_flags != Some(4)` (update in progress — `buildid` may be transitional)
4. If `buildid` differs from `steam_build_id` in snapshot, update status to `'game_updated'`
5. Emit `version-scan-complete` event with counts

**Important**: This scan is fail-soft and non-blocking. Any individual manifest read failure is logged and skipped. StateFlags-based skipping prevents false mismatch alerts during Steam auto-updates.

### Community Import Integration

When importing a community profile that has `game_version` and `trainer_version` in its `CommunityProfileMetadata`:

In `commands/community.rs::community_import_profile()`:

```rust
// After profile is saved and metadata synced:
if let Some(profile_id) = metadata_store.lookup_profile_id(&name).ok().flatten() {
    let _ = metadata_store.upsert_version_snapshot(
        &profile_id,
        None,  // steam_app_id — not yet populated from local manifest
        None,  // steam_build_id — NULL until first local launch
        nullable(&metadata.trainer_version),
        None,  // trainer_file_hash
        nullable(&metadata.game_version),  // human_game_ver from community
        "untracked",  // status — not yet verified locally
        &Utc::now().to_rfc3339(),
    );
}
```

---

## Security Requirements

### Input Validation (from security review)

1. **A6 string bounds** for all version fields:
   - `trainer_version`: max 256 bytes (`MAX_VERSION_STRING_BYTES`)
   - `human_game_ver`: max 256 bytes
   - `steam_build_id`: numeric-only validation (reject non-digit characters)
   - These follow the established pattern in `metadata/community_index.rs::check_a6_bounds()`

2. **Extend `check_a6_bounds()`** in `community_index.rs`:
   - Add `game_version` and `trainer_version` to the existing bounds check
   - Currently these fields flow through unvalidated; with version correlation making them active in comparisons, bounds must be enforced

3. **SQL parameterization**: All version data reaches SQLite exclusively via `params![]` macro (existing convention). No string interpolation in queries.

4. **Community data isolation**: Version strings from community profiles are informational metadata only. They must NEVER flow into subprocess arguments, shell script content, or filesystem paths.

5. **Existing DB security** (inherited, no changes needed): Symlink detection, 0o700/0o600 permissions, WAL mode, foreign keys ON, quick_check, secure_delete ON.

### Trust Model Constraint: W3 — Community Data Is Display-Only

Community version data (from `CommunityProfileMetadata.game_version`, `trainer_version`) must NEVER suppress or trigger behavioral outcomes such as mismatch warnings, launch blocks, or version-change notifications. A malicious tap could silence a valid warning or cause false positives if community data drives behavioral outcomes.

**Rule**: Only local + Steam-derived data (manifest `buildid`, trainer file hash) drives version correlation logic. Community-sourced `human_game_ver` and `trainer_version` are informational display strings only. The `compute_correlation_status()` pure function compares stored vs. current local data — community strings are not inputs to this function.

### Fail-Soft Requirement: A8 — Version DB Errors Never Block Launches

All new version correlation DB calls must follow the existing `is_available()` guard + log-and-continue pattern. A version check failure must NEVER surface as a launch error. This is enforced by:

- `MetadataStore.with_conn()` returning `Err` when unavailable
- All callers using `.unwrap_or_default()` or `.ok()` for version queries
- Launch path (`launch_game`, `launch_trainer`) never depends on version snapshot success

### Security Finding: W2 — Validate `pinned_commit`

Flagged by security review (independent of version tracking but discovered during analysis): `community/taps.rs::checkout_pinned_commit()` passes commit hash to `git checkout --detach <hash>` without format validation. Validate as hex-only, 7-64 characters, before passing to subprocess. This should be fixed independently.

---

## Codebase Changes

### Files to Create

| File                                                  | Purpose                                                                                                                 |
| ----------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------- |
| `crates/crosshook-core/src/metadata/version_store.rs` | Version snapshot CRUD + pure `compute_correlation_status()` function                                                    |
| `src-tauri/src/commands/version.rs`                   | Tauri IPC commands: `check_version_status`, `set_trainer_version`, `get_version_snapshot`, `acknowledge_version_change` |
| `src/types/version.ts`                                | TypeScript types for version IPC payloads                                                                               |

### Files to Modify

| File                                                    | Change                                                                       |
| ------------------------------------------------------- | ---------------------------------------------------------------------------- |
| `crates/crosshook-core/src/steam/manifest.rs`           | Extend `parse_manifest()` to extract `buildid` and `LastUpdated` from VDF    |
| `crates/crosshook-core/src/metadata/mod.rs`             | Add `mod version_store;` + public MetadataStore wrapper methods              |
| `crates/crosshook-core/src/metadata/migrations.rs`      | Add `migrate_8_to_9()` for `version_snapshots` table                         |
| `crates/crosshook-core/src/metadata/community_index.rs` | Extend `check_a6_bounds()` with `game_version`/`trainer_version` validation  |
| `src-tauri/src/commands/mod.rs`                         | Add `pub mod version;`                                                       |
| `src-tauri/src/lib.rs`                                  | Register version commands in `invoke_handler`                                |
| `src-tauri/src/startup.rs`                              | Add optional version scan to reconciliation                                  |
| `src-tauri/src/commands/launch.rs`                      | Hook `upsert_version_snapshot()` after successful launch                     |
| `src-tauri/src/commands/health.rs`                      | Extend `ProfileHealthMetadata` + `BatchMetadataPrefetch` with version fields |
| `src-tauri/src/commands/community.rs`                   | Seed version snapshot on community import                                    |
| `src/types/index.ts`                                    | Re-export version types                                                      |
| `src/types/health.ts`                                   | Extend `ProfileHealthMetadata` with version fields                           |

### Dependencies

No new crate dependencies required. The implementation uses:

- `rusqlite` (existing) — SQLite operations
- `chrono` (existing) — Timestamps
- `uuid` (existing via `db::new_id()`) — Record IDs
- `serde` (existing) — Serialization
- `sha2` (existing, used in `profile_sync.rs`) — Trainer file hashing
- VDF parser (existing `steam/vdf.rs`) — Manifest parsing

---

## Technical Decisions

### Decision 1: Version Storage — Single-Row Snapshot vs. History Table

| Option                                                                            | Pros                                                     | Cons                                                         | Recommendation |
| --------------------------------------------------------------------------------- | -------------------------------------------------------- | ------------------------------------------------------------ | -------------- |
| **A: History table** (version_records with many rows per profile)                 | Full audit trail; compatibility matrix potential         | Unbounded growth; retention complexity; more complex queries | No (v1)        |
| **B: Single-row snapshot** (INSERT OR REPLACE per profile, like health_snapshots) | Bounded by profile count; simple queries; proven pattern | No history; single reference point                           | **Yes**        |

**Rationale** (team consensus): The `health_snapshots` table already proves this pattern works well in CrossHook. A single-row snapshot per profile answers the core question ("has the game been updated since last successful launch?") without the complexity of history management, retention policies, or unbounded growth. History can be added in a future version if needed — the snapshot captures the current baseline.

### Decision 2: Manifest Parsing Strategy

| Option                                       | Pros                    | Cons                                          | Recommendation |
| -------------------------------------------- | ----------------------- | --------------------------------------------- | -------------- |
| **A: Separate `steam/version.rs` module**    | No existing API changes | Duplicate manifest reading; separate file I/O | No             |
| **B: Extend `parse_manifest()` return type** | Single parse; DRY       | Changes `find_game_match()` caller chain      | **Yes**        |

**Rationale** (practices researcher): The VDF parser already exists. Adding `get_child("buildid")` is one line. A separate module would re-read the same file. If the return type change cascades too far, add `parse_manifest_full()` alongside the existing function and migrate callers incrementally.

### Decision 3: Mismatch Surfacing — Dedicated Subsystem vs. Health Integration

| Option                                       | Pros                                                               | Cons                                                                | Recommendation |
| -------------------------------------------- | ------------------------------------------------------------------ | ------------------------------------------------------------------- | -------------- |
| **A: Separate mismatch table + events + UI** | Rich dedicated UX; separate acknowledgment flow                    | Parallel notification system; more components; maintenance overhead | No (v1)        |
| **B: Health pipeline integration**           | Reuses batch prefetch; enrichment pattern proven; fewer components | Less granular mismatch tracking                                     | **Yes**        |

**Rationale** (recommendations agent + practices researcher): The existing health enrichment pipeline (`BatchMetadataPrefetch` -> `enrich_profile()` -> `EnrichedProfileHealthReport`) is the natural extension point. Version mismatch becomes a new dimension of "profile health" rather than a separate subsystem. The `HealthDashboardPage` already handles sortable metadata display and `HealthBadge` already communicates profile status.

### Decision 4: Version Comparison Semantics

| Aspect                       | Approach                                                                                                   |
| ---------------------------- | ---------------------------------------------------------------------------------------------------------- |
| **Game version**             | Steam `buildid` — opaque integer string. Simple `!=` equality. No semver.                                  |
| **Trainer version**          | Free-text string. Simple string equality.                                                                  |
| **Trainer change detection** | SHA-256 file hash (using existing `sha2` crate). Change = hash differs from snapshot.                      |
| **"No data"**                | NULL fields are NOT mismatches. `compute_correlation_status()` returns `Matched` when either side is NULL. |

**Rationale** (business analysis + practices researcher): Steam build IDs are opaque integers, not semantic versions. Trainer versions are inconsistent across providers. The simplest correct approach is raw equality comparison via a pure function, making the logic trivially testable.

### Decision 5: Capture Timing

| Trigger               | Action                                                                | Justification                       |
| --------------------- | --------------------------------------------------------------------- | ----------------------------------- |
| **Successful launch** | `upsert_version_snapshot()` with status `'matched'`                   | Primary — records the working combo |
| **Startup scan**      | Check manifests, update status to `'game_updated'` if buildid changed | Catches offline updates             |
| **On-demand check**   | `check_version_status` IPC command                                    | Frontend can request fresh check    |
| **Community import**  | Seed `human_game_ver` and `trainer_version` from metadata             | Pre-populate display data           |

**NOT on pre-launch** (practices researcher recommendation): Avoid adding latency to the launch path. The health dashboard provides the persistent monitoring surface; the launch panel can query on-demand before the user clicks Launch.

---

## UX Requirements (from UX Research)

### Loading States

- **Version check (local manifest scan)**: <10ms — no loading state needed
- **Community index sync**: Show "Syncing…" badge only on Compatibility page, NOT on Launch page
- **Launch button**: Must NEVER be blocked by a version check in-flight
- **Warning banner**: Appears/updates in-place without full page re-render

### Data Display Requirements

The frontend needs these data points from the version correlation system:

1. **Stored baseline**: `last_verified_game_version` (build ID or human version string)
2. **Current game version**: Read from Steam manifest `buildid`
3. **Version delta display**: `"buildid 12345 → 12348"` in warning banner
4. **Last scan timestamp**: `"Last checked: 5 min ago"` as tooltip on badge (use existing `formatRelativeTime` utility)
5. **Launch count since mismatch**: Future metric tracked via launch history

### Component Integration Points

| Component                 | Integration                                                                                            |
| ------------------------- | ------------------------------------------------------------------------------------------------------ |
| `LaunchPanel.tsx`         | Warning banner when `version_status` is `game_updated` / `trainer_changed` / `both_changed`            |
| `HealthDashboardPage.tsx` | Add `version_changed` to issue categories; version status column in table                              |
| `HealthBadge.tsx`         | Reflect version mismatch in badge (use existing `crosshook-compatibility-badge--{rating}` CSS classes) |
| `ProfilesPage.tsx`        | Card badge showing version status for pinned/visible profiles                                          |

### Frontend Hook Strategy

If the feature surfaces only in health dashboard + launch panel, reuse `useProfileHealth` (version data flows through health enrichment). A dedicated `useVersionTracking` hook is only justified if version correlation gets its own standalone UI page.

---

## Open Questions

1. **Trainer version auto-detection depth**: `pelite` crate can read PE VERSIONINFO resources from Windows executables on Linux (zero-unsafe, cross-platform). This would auto-detect trainer versions from `.exe` files. However, `pelite` is NOT in `Cargo.toml` — it's a new dependency. **Recommendation**: SHA-256 hash + user-entered string for v1. PE extraction via `pelite` is a v2 enhancement. Alternative intermediate: filename regex `v\d+[\.\d]*` as heuristic.

2. **Non-Steam game support**: For `proton_run` and `native` launch methods without a Steam app ID, should the `version_snapshots` row be created with `steam_build_id = NULL` and only track trainer file hash changes? Or skip these profiles entirely? (Business analysis says: skip silently for v1, `status = 'untracked'`.)

3. **Community compatibility sharing**: Should successful launch combos (`status = 'matched'`) be exportable back to community taps? This would enhance `CommunityProfileMetadata` with verified version data. (Defer to v2 — requires community schema version bump.)

4. **Manifest path caching**: Should we cache the resolved manifest path per `steam_app_id` to avoid re-scanning all Steam libraries on every check? (Optimization concern — profile on real hardware first.)

5. **parse_manifest() return type migration**: Should we change the existing return type in-place (breaking callers) or add `parse_manifest_full()` alongside? (Implementation decision — evaluate cascade during development.)

6. **Filesystem watching (v2)**: `notify v8.2.0` with `notify-debouncer-full v0.7.0` could watch `steamapps/` directories for `appmanifest_*.acf` changes in real-time using `NonRecursive` inotify. The debouncer stitches rename events (Steam writes temp then renames). **Recommendation**: v1 uses poll-on-open (startup scan + on-demand check). Filesystem watching is a v2 optimization for long-running sessions.

7. **StateFlags-aware scanning**: When `StateFlags != 4` (e.g., `1026` = update in progress), the manifest `buildid` may be stale or transitional. Should the startup scan skip manifests with non-4 StateFlags? **Recommendation**: Yes — skip and log a debug note; re-check on next scan or on-demand.

---

## Future Enhancements (v2 Scope)

These capabilities were identified during research but are explicitly deferred from v1:

| Enhancement                     | Dependency                                       | Notes                                                                                           |
| ------------------------------- | ------------------------------------------------ | ----------------------------------------------------------------------------------------------- |
| **Filesystem watching**         | `notify v8.2.0` + `notify-debouncer-full v0.7.0` | Real-time ACF change detection via inotify; `NonRecursive` on `steamapps/` dirs                 |
| **PE version extraction**       | `pelite` crate (new dependency)                  | Auto-read VERSIONINFO from trainer `.exe` files on Linux; replaces user-entered trainer version |
| **Version comparison libs**     | `semver v1.0.27` or `versions` crate             | Semantic version comparison if needed; v1 uses opaque string equality                           |
| **Community version sharing**   | Community schema version bump                    | Export verified `status = 'matched'` combos back to community taps                              |
| **Launch-count-since-mismatch** | Launch history query extension                   | Track how many times user launched despite mismatch warning                                     |
| **Pre-launch block option**     | Settings page toggle                             | Hard pre-launch block for version mismatches as opt-in settings option                          |

---

## Relevant Files Reference

| File                                                    | Purpose                                                                                                                            |
| ------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------- |
| `crates/crosshook-core/src/steam/manifest.rs`           | Existing manifest parsing — extend to extract `buildid` (one `get_child()` call)                                                   |
| `crates/crosshook-core/src/steam/vdf.rs`                | VDF parser — already supports extracting any field by key name                                                                     |
| `crates/crosshook-core/src/metadata/db.rs`              | Database open/configure (WAL, FK, symlink check, permissions)                                                                      |
| `crates/crosshook-core/src/metadata/migrations.rs`      | Migration runner — current schema v8, add v9                                                                                       |
| `crates/crosshook-core/src/metadata/models.rs`          | Error types, row structs, enums — add `VersionCorrelationStatus`                                                                   |
| `crates/crosshook-core/src/metadata/mod.rs`             | MetadataStore facade (Arc<Mutex<Connection>>, `with_conn` pattern)                                                                 |
| `crates/crosshook-core/src/metadata/health_store.rs`    | **Primary template** — `upsert_health_snapshot()`, `load_health_snapshots()`, `lookup_health_snapshot()` pattern to mirror exactly |
| `crates/crosshook-core/src/metadata/launch_history.rs`  | Launch operation recording — hook point for post-success version capture                                                           |
| `crates/crosshook-core/src/metadata/community_index.rs` | A6 bounds validation — extend for `game_version`/`trainer_version`                                                                 |
| `crates/crosshook-core/src/metadata/profile_sync.rs`    | `compute_content_hash()` uses `sha2` — reuse for trainer file hashing                                                              |
| `crates/crosshook-core/src/profile/models.rs`           | GameProfile struct — NO version fields (intentional, confirmed)                                                                    |
| `crates/crosshook-core/src/profile/community_schema.rs` | CommunityProfileMetadata — has `game_version`, `trainer_version` (display-only, seed into snapshots)                               |
| `crates/crosshook-core/src/launch/request.rs`           | LaunchRequest struct — `profile_name` field used for version linkage                                                               |
| `src-tauri/src/lib.rs`                                  | Tauri app setup — command registration in `invoke_handler`                                                                         |
| `src-tauri/src/startup.rs`                              | Startup reconciliation — add version scan after existing profile sync                                                              |
| `src-tauri/src/commands/launch.rs`                      | Launch commands — hook version snapshot after `record_launch_finished()`                                                           |
| `src-tauri/src/commands/health.rs`                      | Health enrichment — extend `BatchMetadataPrefetch` and `ProfileHealthMetadata`                                                     |
| `src-tauri/src/commands/community.rs`                   | Community import — seed version snapshot from community metadata                                                                   |
| `src/types/health.ts`                                   | Health types — extend `ProfileHealthMetadata` with version fields                                                                  |
| `src/types/profile.ts`                                  | Profile types — GameProfile (no changes needed)                                                                                    |
| `src/hooks/useProfileHealth.ts`                         | Health state hook — version data flows through existing health enrichment                                                          |
| `src/components/HealthBadge.tsx`                        | Health badge — reflect version mismatch status                                                                                     |
| `src/components/LaunchPanel.tsx`                        | Launch controls — on-demand mismatch check before launch                                                                           |
| `src/components/CompatibilityViewer.tsx`                | Compatibility info — potential version display                                                                                     |
