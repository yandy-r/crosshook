# Profile Health Dashboard — Technical Specification (v2)

> **Revision**: v2 — Revised to integrate with the SQLite MetadataStore (PRs #89-91). The original v1 spec was written before the metadata layer existed. This revision preserves the filesystem validation core unchanged, adds MetadataStore integration for health enrichment, introduces a `health_snapshots` table (migration v6), and revises the API contracts and file change list accordingly.

## Executive Summary

The profile health dashboard adds batch validation of all saved CrossHook profiles, surfacing per-profile health status (healthy/stale/broken) with specific remediation suggestions for broken filesystem paths. **What changed in v2**: health results are now enriched with metadata from `MetadataStore` — last successful launch timestamp, failure trends, launcher drift state, and community-import origin — when the SQLite layer is available. A new `health_snapshots` table (migration v6) caches the last-computed health status per profile for instant startup display. The core filesystem validation logic, data types, and UI patterns from v1 remain architecturally unchanged. The MetadataStore integration follows the established fail-soft pattern: all health features work without SQLite, and metadata enrichment is additive.

---

## Architecture Design

### Component Diagram

```
┌─────────────────────────────────────────────────────────────────────┐
│  React Frontend                                                     │
│  ┌──────────────────────┐  ┌─────────────────────────┐              │
│  │ ProfileHealthDashboard│  │ HealthBadge             │              │
│  │  (new component)      │  │  (reusable badge)       │              │
│  └──────────┬───────────┘  └─────────────────────────┘              │
│             │ invoke()                                               │
│  ┌──────────┴───────────┐                                           │
│  │ useProfileHealth     │  listen("profile-health-batch-complete")  │
│  │  (new hook)          │◄──────────────────────────────────────────│
│  └──────────┬───────────┘                                           │
└─────────────┼───────────────────────────────────────────────────────┘
              │ Tauri IPC
┌─────────────┼───────────────────────────────────────────────────────┐
│  src-tauri  │                                                       │
│  ┌──────────┴───────────┐                                           │
│  │ commands/health.rs   │  (new command module)                     │
│  │  batch_validate_     │  Accepts ProfileStore + MetadataStore     │
│  │  profiles()          │  Enriches core results with metadata      │
│  │  get_profile_health()│  Follows fail-soft pattern                │
│  └──────────┬───────────┘                                           │
└─────────────┼───────────────────────────────────────────────────────┘
              │
┌─────────────┼───────────────────────────────────────────────────────┐
│  crosshook-core                                                     │
│  ┌──────────┴───────────┐  ┌─────────────────────────┐              │
│  │ profile/health.rs    │  │ profile/                │              │
│  │  HealthStatus        │──│  models.rs (GameProfile) │              │
│  │  ProfileHealthReport │  │  toml_store.rs (Store)   │              │
│  │  HealthIssue         │  └─────────────────────────┘              │
│  │  check_profile_      │                                           │
│  │  health()            │  ┌─────────────────────────┐              │
│  │  batch_check_        │  │ metadata/               │              │
│  │  health()            │  │  mod.rs (MetadataStore)  │              │
│  └──────────────────────┘  │  health_store.rs (NEW)   │              │
│                            │  migrations.rs (v6)      │              │
│       uses:                └─────────────────────────┘              │
│       std::fs::metadata()                                           │
│       std::os::unix::fs::PermissionsExt                             │
└─────────────────────────────────────────────────────────────────────┘
```

### New Components

| Component               | Location                                          | Responsibility                                                                   |
| ----------------------- | ------------------------------------------------- | -------------------------------------------------------------------------------- |
| `profile/health.rs`     | `crates/crosshook-core/src/profile/health.rs`     | Core health types + filesystem validation logic (no MetadataStore dependency)    |
| `metadata/health_store` | `crates/crosshook-core/src/metadata/health_store.rs` | Health snapshot persistence (read/write `health_snapshots` table)             |
| `commands/health.rs`    | `src-tauri/src/commands/health.rs`                | Tauri IPC commands: orchestrates ProfileStore + MetadataStore, enriches results  |
| `useProfileHealth`      | `src/hooks/useProfileHealth.ts`                   | React hook for health state management (invoke + listen + useReducer)            |
| `HealthBadge`           | `src/components/HealthBadge.tsx`                  | Reusable status badge (follows `CompatibilityBadge` pattern)                     |
| `ProfileHealthDashboard`| `src/components/ProfileHealthDashboard.tsx`       | Dashboard UI with summary bar + per-profile cards                                |
| Health types            | `src/types/health.ts`                             | TypeScript interfaces                                                            |

### Integration Points

1. **`crates/crosshook-core/src/profile/mod.rs`** — add `pub mod health;`
2. **`crates/crosshook-core/src/metadata/mod.rs`** — add `mod health_store;` and public methods on `MetadataStore`
3. **`crates/crosshook-core/src/metadata/migrations.rs`** — add `migrate_5_to_6` for `health_snapshots` table
4. **`src-tauri/src/commands/mod.rs`** — add `pub mod health;`
5. **`src-tauri/src/lib.rs`** — register health commands in `invoke_handler`; spawn startup background health check
6. **`src/types/index.ts`** — add `export * from './health';`
7. **`src/App.tsx`** — integrate `ProfileHealthDashboard` inline in profile list area

### Key Architectural Decision: Two-Layer Health Architecture

**CHANGED from v1**: The original spec had a single layer — `crosshook-core` health module consumed directly by Tauri commands. v2 introduces a two-layer architecture:

**Layer 1 — Core Health (crosshook-core, pure filesystem)**
- `profile/health.rs` contains types and validation logic
- No MetadataStore dependency — testable with just `ProfileStore` + `tempdir`
- Input: `GameProfile` fields. Output: `ProfileHealthReport` with filesystem-only data

**Layer 2 — Enriched Health (src-tauri, orchestration)**
- `commands/health.rs` accepts both `State<'_, ProfileStore>` and `State<'_, MetadataStore>`
- Calls Layer 1 for filesystem validation
- Enriches results with metadata (last_success, failure_count, launcher_drift)
- Follows fail-soft: if MetadataStore unavailable, returns `metadata: null` on each report
- Persists health snapshots for fast startup display

**Rationale**: This mirrors the existing pattern where `crosshook-core` is store-agnostic (e.g., `profile/models.rs` has no MetadataStore import) and `src-tauri` orchestrates multiple stores (e.g., `commands/profile.rs` accepts both `ProfileStore` and `MetadataStore`).

---

## Data Models

### Rust Structs — Core Health Types (`crates/crosshook-core/src/profile/health.rs`)

> **UNCHANGED from v1** except module location changed from `health/models.rs` to `profile/health.rs`.

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HealthStatus {
    Healthy,
    Stale,
    Broken,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HealthIssueSeverity {
    Error,
    Warning,
    Info,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HealthIssue {
    pub field: String,
    pub path: String,
    pub message: String,
    pub remediation: String,
    pub severity: HealthIssueSeverity,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProfileHealthReport {
    pub name: String,
    pub status: HealthStatus,
    pub launch_method: String,
    pub issues: Vec<HealthIssue>,
    pub checked_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HealthCheckSummary {
    pub profiles: Vec<ProfileHealthReport>,
    pub healthy_count: usize,
    pub stale_count: usize,
    pub broken_count: usize,
    pub total_count: usize,
    pub validated_at: String,
}
```

### Rust Structs — Metadata-Enriched Types (`src-tauri/src/commands/health.rs`)

> **NEW in v2**. These types wrap core health results with optional MetadataStore data.

```rust
use serde::Serialize;
use crosshook_core::profile::health::{HealthCheckSummary, ProfileHealthReport};

/// Metadata enrichment for a single profile's health report.
/// All fields are optional — null when MetadataStore is unavailable.
#[derive(Debug, Clone, Serialize)]
pub struct ProfileHealthMetadata {
    pub profile_id: Option<String>,
    pub last_success: Option<String>,
    pub failure_count_30d: i64,
    pub total_launches: i64,
    pub launcher_drift_state: Option<String>,
    pub is_community_import: bool,
}

impl Default for ProfileHealthMetadata {
    fn default() -> Self {
        Self {
            profile_id: None,
            last_success: None,
            failure_count_30d: 0,
            total_launches: 0,
            launcher_drift_state: None,
            is_community_import: false,
        }
    }
}

/// Enriched health report for a single profile (core report + metadata).
#[derive(Debug, Clone, Serialize)]
pub struct EnrichedProfileHealthReport {
    #[serde(flatten)]
    pub core: ProfileHealthReport,
    pub metadata: Option<ProfileHealthMetadata>,
}

/// Enriched batch summary (core summary + per-profile metadata).
#[derive(Debug, Clone, Serialize)]
pub struct EnrichedHealthSummary {
    pub profiles: Vec<EnrichedProfileHealthReport>,
    pub healthy_count: usize,
    pub stale_count: usize,
    pub broken_count: usize,
    pub total_count: usize,
    pub validated_at: String,
}
```

### SQLite Schema — `health_snapshots` Table (Migration v6)

> **NEW in v2**. Advisory cache of last-computed health status per profile.

```sql
CREATE TABLE IF NOT EXISTS health_snapshots (
    profile_id      TEXT NOT NULL REFERENCES profiles(profile_id),
    status          TEXT NOT NULL,   -- 'healthy', 'stale', 'broken'
    issue_count     INTEGER NOT NULL DEFAULT 0,
    checked_at      TEXT NOT NULL,   -- ISO 8601 timestamp
    PRIMARY KEY (profile_id)
);
CREATE INDEX IF NOT EXISTS idx_health_snapshots_status ON health_snapshots(status);
```

**Design decisions:**
- **One row per profile** (upsert on re-check) — bounded storage, no unbounded growth
- **No issue detail stored** — only status and count. Full issue data is always recomputed from filesystem. This avoids storing stale path strings in SQLite.
- **Foreign key to `profiles(profile_id)`** — cascades on profile deletion
- **`checked_at` timestamp** — enables "stale snapshot" detection (e.g., "last checked 7 days ago")

**Migration function** (`crates/crosshook-core/src/metadata/migrations.rs`):

```rust
fn migrate_5_to_6(conn: &Connection) -> Result<(), MetadataStoreError> {
    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS health_snapshots (
            profile_id      TEXT NOT NULL REFERENCES profiles(profile_id),
            status          TEXT NOT NULL,
            issue_count     INTEGER NOT NULL DEFAULT 0,
            checked_at      TEXT NOT NULL,
            PRIMARY KEY (profile_id)
        );
        CREATE INDEX IF NOT EXISTS idx_health_snapshots_status ON health_snapshots(status);
        ",
    )
    .map_err(|source| MetadataStoreError::Database {
        action: "run metadata migration 5 to 6",
        source,
    })?;

    Ok(())
}
```

And the corresponding block in `run_migrations`:

```rust
if version < 6 {
    migrate_5_to_6(conn)?;
    conn.pragma_update(None, "user_version", 6_u32)
        .map_err(|source| MetadataStoreError::Database {
            action: "update metadata schema version",
            source,
        })?;
}
```

### Rust — Health Snapshot Store (`crates/crosshook-core/src/metadata/health_store.rs`)

> **NEW in v2**. Thin persistence layer for health snapshots.

```rust
use super::MetadataStoreError;
use rusqlite::{params, Connection, OptionalExtension};

pub fn upsert_health_snapshot(
    conn: &Connection,
    profile_id: &str,
    status: &str,
    issue_count: usize,
    checked_at: &str,
) -> Result<(), MetadataStoreError> {
    conn.execute(
        "INSERT INTO health_snapshots (profile_id, status, issue_count, checked_at)
         VALUES (?1, ?2, ?3, ?4)
         ON CONFLICT(profile_id) DO UPDATE SET
             status = excluded.status,
             issue_count = excluded.issue_count,
             checked_at = excluded.checked_at",
        params![profile_id, status, issue_count as i64, checked_at],
    )
    .map_err(|source| MetadataStoreError::Database {
        action: "upsert a health snapshot",
        source,
    })?;
    Ok(())
}

pub fn load_health_snapshots(
    conn: &Connection,
) -> Result<Vec<(String, String, i64, String)>, MetadataStoreError> {
    // Returns (profile_id, status, issue_count, checked_at) for all profiles
    let mut stmt = conn
        .prepare(
            "SELECT hs.profile_id, hs.status, hs.issue_count, hs.checked_at
             FROM health_snapshots hs
             INNER JOIN profiles p ON hs.profile_id = p.profile_id
             WHERE p.deleted_at IS NULL",
        )
        .map_err(|source| MetadataStoreError::Database {
            action: "prepare health snapshot query",
            source,
        })?;

    let rows = stmt
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, i64>(2)?,
                row.get::<_, String>(3)?,
            ))
        })
        .map_err(|source| MetadataStoreError::Database {
            action: "query health snapshots",
            source,
        })?;

    let mut result = Vec::new();
    for row in rows {
        result.push(row.map_err(|source| MetadataStoreError::Database {
            action: "read a health snapshot row",
            source,
        })?);
    }
    Ok(result)
}

pub fn lookup_health_snapshot(
    conn: &Connection,
    profile_id: &str,
) -> Result<Option<(String, i64, String)>, MetadataStoreError> {
    conn.query_row(
        "SELECT status, issue_count, checked_at FROM health_snapshots WHERE profile_id = ?1",
        params![profile_id],
        |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
    )
    .optional()
    .map_err(|source| MetadataStoreError::Database {
        action: "look up a health snapshot",
        source,
    })
}
```

### MetadataStore Public API Additions

> **NEW in v2**. Methods added to `MetadataStore` in `mod.rs`.

```rust
// In impl MetadataStore:

pub fn upsert_health_snapshot(
    &self,
    profile_id: &str,
    status: &str,
    issue_count: usize,
    checked_at: &str,
) -> Result<(), MetadataStoreError> {
    self.with_conn("upsert a health snapshot", |conn| {
        health_store::upsert_health_snapshot(conn, profile_id, status, issue_count, checked_at)
    })
}

pub fn load_health_snapshots(
    &self,
) -> Result<Vec<(String, String, i64, String)>, MetadataStoreError> {
    self.with_conn("load health snapshots", |conn| {
        health_store::load_health_snapshots(conn)
    })
}

pub fn lookup_health_snapshot(
    &self,
    profile_id: &str,
) -> Result<Option<(String, i64, String)>, MetadataStoreError> {
    self.with_conn("look up a health snapshot", |conn| {
        health_store::lookup_health_snapshot(conn, profile_id)
    })
}
```

### TypeScript Interfaces (`src/types/health.ts`)

> **UPDATED from v1**. Added `ProfileHealthMetadata` and `EnrichedProfileHealthReport`.

```typescript
export type HealthStatus = 'healthy' | 'stale' | 'broken';
export type HealthIssueSeverity = 'error' | 'warning' | 'info';

export interface HealthIssue {
  field: string;
  path: string;
  message: string;
  remediation: string;
  severity: HealthIssueSeverity;
}

export interface ProfileHealthMetadata {
  profile_id: string | null;
  last_success: string | null;
  failure_count_30d: number;
  total_launches: number;
  launcher_drift_state: string | null;
  is_community_import: boolean;
}

export interface EnrichedProfileHealthReport {
  name: string;
  status: HealthStatus;
  launch_method: string;
  issues: HealthIssue[];
  checked_at: string;
  metadata: ProfileHealthMetadata | null;
}

export interface EnrichedHealthSummary {
  profiles: EnrichedProfileHealthReport[];
  healthy_count: number;
  stale_count: number;
  broken_count: number;
  total_count: number;
  validated_at: string;
}
```

---

## API Design

### Tauri Commands (`src-tauri/src/commands/health.rs`)

#### `batch_validate_profiles`

> **CHANGED from v1**: Now accepts `MetadataStore` and returns `EnrichedHealthSummary`.

Validates all saved profiles against the filesystem, enriches with metadata when available, and persists health snapshots.

```rust
#[tauri::command]
pub fn batch_validate_profiles(
    store: State<'_, ProfileStore>,
    metadata_store: State<'_, MetadataStore>,
) -> Result<EnrichedHealthSummary, String> {
    // Layer 1: Core filesystem validation
    let base_summary = crosshook_core::profile::health::batch_check_health(&store)
        .map_err(|e| e.to_string())?;

    // Layer 2: Metadata enrichment (fail-soft)
    let metadata_map = gather_metadata_for_profiles(&metadata_store);

    // Build enriched summary
    let enriched_profiles: Vec<EnrichedProfileHealthReport> = base_summary
        .profiles
        .into_iter()
        .map(|mut report| {
            // Sanitize paths before IPC
            for issue in &mut report.issues {
                issue.path = shared::sanitize_display_path(&issue.path);
            }

            let metadata = metadata_map.as_ref().map(|m| {
                m.get(&report.name).cloned().unwrap_or_default()
            });

            // Persist health snapshot (best-effort)
            if let Some(ref meta) = metadata {
                if let Some(ref pid) = meta.profile_id {
                    let status_str = match report.status {
                        HealthStatus::Healthy => "healthy",
                        HealthStatus::Stale => "stale",
                        HealthStatus::Broken => "broken",
                    };
                    if let Err(e) = metadata_store.upsert_health_snapshot(
                        pid, status_str, report.issues.len(), &report.checked_at,
                    ) {
                        tracing::warn!(%e, name = %report.name, "health snapshot upsert failed");
                    }
                }
            }

            EnrichedProfileHealthReport { core: report, metadata }
        })
        .collect();

    Ok(EnrichedHealthSummary {
        healthy_count: enriched_profiles.iter().filter(|p| p.core.status == HealthStatus::Healthy).count(),
        stale_count: enriched_profiles.iter().filter(|p| p.core.status == HealthStatus::Stale).count(),
        broken_count: enriched_profiles.iter().filter(|p| p.core.status == HealthStatus::Broken).count(),
        total_count: enriched_profiles.len(),
        validated_at: base_summary.validated_at,
        profiles: enriched_profiles,
    })
}
```

**Frontend invocation:**

```typescript
const summary = await invoke<EnrichedHealthSummary>('batch_validate_profiles');
```

---

#### `get_profile_health`

> **CHANGED from v1**: Now accepts `MetadataStore` and returns `EnrichedProfileHealthReport`.

```rust
#[tauri::command]
pub fn get_profile_health(
    name: String,
    store: State<'_, ProfileStore>,
    metadata_store: State<'_, MetadataStore>,
) -> Result<EnrichedProfileHealthReport, String> {
    let profile = store.load(&name).map_err(|e| e.to_string())?;
    let mut report = crosshook_core::profile::health::check_profile_health(&name, &profile);

    // Sanitize paths
    for issue in &mut report.issues {
        issue.path = shared::sanitize_display_path(&issue.path);
    }

    // Metadata enrichment (fail-soft)
    let metadata = gather_metadata_for_single_profile(&name, &metadata_store);

    // Persist snapshot (best-effort)
    if let Some(ref meta) = metadata {
        if let Some(ref pid) = meta.profile_id {
            let status_str = match report.status {
                HealthStatus::Healthy => "healthy",
                HealthStatus::Stale => "stale",
                HealthStatus::Broken => "broken",
            };
            let _ = metadata_store.upsert_health_snapshot(
                pid,
                status_str,
                report.issues.len(),
                &report.checked_at,
            );
        }
    }

    Ok(EnrichedProfileHealthReport { core: report, metadata })
}
```

**Frontend invocation:**

```typescript
const report = await invoke<EnrichedProfileHealthReport>('get_profile_health', { name: 'MyGame' });
```

---

#### Metadata Gathering Helper (private, in `commands/health.rs`)

```rust
use std::collections::HashMap;

/// Gathers metadata enrichment data for all profiles.
/// Returns None if MetadataStore is unavailable (fail-soft).
fn gather_metadata_for_profiles(
    metadata_store: &MetadataStore,
) -> Option<HashMap<String, ProfileHealthMetadata>> {
    // Query all data sources in sequence (all are fail-soft via with_conn)
    let last_successes: HashMap<String, String> = metadata_store
        .query_last_success_per_profile()
        .unwrap_or_default()
        .into_iter()
        .collect();

    let failure_trends: HashMap<String, (i64, i64)> = metadata_store
        .query_failure_trends(30)
        .unwrap_or_default()
        .into_iter()
        .map(|row| (row.profile_name.clone(), (row.failures, row.successes + row.failures)))
        .collect();

    let launch_counts: HashMap<String, i64> = metadata_store
        .query_most_launched(1000)  // effectively unlimited
        .unwrap_or_default()
        .into_iter()
        .collect();

    // Build per-profile metadata map
    // Note: profile_id lookup and source/drift state require per-profile queries
    // which are done inline during enrichment. The batch queries above cover
    // the high-value aggregate data.
    Some(HashMap::new()) // Populated during enrichment loop with per-profile queries
}

fn gather_metadata_for_single_profile(
    name: &str,
    metadata_store: &MetadataStore,
) -> Option<ProfileHealthMetadata> {
    let profile_id = metadata_store.lookup_profile_id(name).ok().flatten();

    let last_success = metadata_store
        .query_last_success_per_profile()
        .unwrap_or_default()
        .into_iter()
        .find(|(n, _)| n == name)
        .map(|(_, ts)| ts);

    let failure_trend = metadata_store
        .query_failure_trends(30)
        .unwrap_or_default()
        .into_iter()
        .find(|row| row.profile_name == name);

    let total_launches = metadata_store
        .query_most_launched(1000)
        .unwrap_or_default()
        .into_iter()
        .find(|(n, _)| n == name)
        .map(|(_, count)| count)
        .unwrap_or(0);

    Some(ProfileHealthMetadata {
        profile_id,
        last_success,
        failure_count_30d: failure_trend.as_ref().map(|r| r.failures).unwrap_or(0),
        total_launches,
        launcher_drift_state: None, // TODO: query from launchers table
        is_community_import: false,  // TODO: query source from profiles table
    })
}
```

---

#### `profile-health-batch-complete` — Tauri Event (startup)

> **UNCHANGED from v1** except payload type is now `EnrichedHealthSummary`.

**Purpose**: Push startup health results to frontend after background scan.
**Payload**: `EnrichedHealthSummary`.
**Timing**: Emitted ~1000ms after UI renders via async task (after auto-load-profile at 350ms).

```rust
// In src-tauri/src/lib.rs setup closure:
{
    let profile_store = profile_store.clone();
    let metadata_store = metadata_for_startup.clone();
    let app_handle = app.handle().clone();
    tauri::async_runtime::spawn(async move {
        sleep(Duration::from_millis(1000)).await;
        match crosshook_core::profile::health::batch_check_health(&profile_store) {
            Ok(base_summary) => {
                // Enrich with metadata (inline, same logic as command)
                let enriched = enrich_summary_for_startup(base_summary, &metadata_store);
                if let Err(error) = app_handle.emit("profile-health-batch-complete", &enriched) {
                    tracing::warn!(%error, "failed to emit startup health check event");
                }
            }
            Err(error) => {
                tracing::warn!(%error, "startup health check failed");
            }
        }
    });
}
```

---

### Core Validation Logic (`crates/crosshook-core/src/profile/health.rs`)

> **UNCHANGED from v1** in logic. Moved from `health/validate.rs` to `profile/health.rs`.

```rust
//! Profile health validation.
//!
//! All operations in this module are **read-only metadata checks**. No write I/O
//! is performed. Uses only `std::fs::metadata()`, `Path::is_file()`, `Path::is_dir()`,
//! and `PermissionsExt::mode()` for filesystem inspection.

use std::path::Path;
use serde::{Deserialize, Serialize};
use crate::profile::{GameProfile, ProfileStore, ProfileStoreError, resolve_launch_method};

// --- Types (HealthStatus, HealthIssueSeverity, HealthIssue,
//            ProfileHealthReport, HealthCheckSummary) as shown above ---

/// Validates a single profile's filesystem paths and returns a health report.
pub fn check_profile_health(name: &str, profile: &GameProfile) -> ProfileHealthReport {
    let mut issues = Vec::new();
    let method = resolve_launch_method(profile);

    // Game executable (Error severity — required for all methods)
    check_file_path(
        &profile.game.executable_path,
        "game.executable_path",
        "Game executable",
        HealthIssueSeverity::Error,
        "Re-browse to the game executable or verify game files are installed.",
        &mut issues,
    );

    // Steam paths (only for steam_applaunch)
    if method == "steam_applaunch" {
        check_directory_path(
            &profile.steam.compatdata_path,
            "steam.compatdata_path",
            "Steam compatdata directory",
            HealthIssueSeverity::Error,
            "Launch the game through Steam once to create the compatdata directory, or use Auto-Populate.",
            &mut issues,
        );
        check_executable_path(
            &profile.steam.proton_path,
            "steam.proton_path",
            "Steam Proton executable",
            HealthIssueSeverity::Error,
            "The configured Proton version may have been removed. Re-select via Auto-Populate.",
            &mut issues,
        );
    }

    // Runtime paths (only for proton_run)
    if method == "proton_run" {
        check_directory_path(
            &profile.runtime.prefix_path,
            "runtime.prefix_path",
            "WINE/Proton prefix directory",
            HealthIssueSeverity::Error,
            "Re-select the prefix directory or launch the game once to recreate it.",
            &mut issues,
        );
        check_executable_path(
            &profile.runtime.proton_path,
            "runtime.proton_path",
            "Runtime Proton executable",
            HealthIssueSeverity::Error,
            "The configured Proton version may have been removed. Re-select an installed Proton.",
            &mut issues,
        );
    }

    // DLL injection paths (Warning severity — optional)
    for (i, dll_path) in profile.injection.dll_paths.iter().enumerate() {
        if dll_path.trim().is_empty() {
            continue;
        }
        check_file_path(
            dll_path,
            &format!("injection.dll_paths[{i}]"),
            &format!("DLL injection path #{}", i + 1),
            HealthIssueSeverity::Warning,
            "Remove the DLL path or re-browse to the correct file.",
            &mut issues,
        );
    }

    // Launcher icon path (Info severity — cosmetic)
    if !profile.steam.launcher.icon_path.trim().is_empty() {
        check_file_path(
            &profile.steam.launcher.icon_path,
            "steam.launcher.icon_path",
            "Launcher icon",
            HealthIssueSeverity::Info,
            "Remove the icon path or browse to a new icon image.",
            &mut issues,
        );
    }

    let status = derive_status(&issues);

    ProfileHealthReport {
        name: name.to_string(),
        status,
        launch_method: method.to_string(),
        issues,
        checked_at: now_iso8601(),
    }
}

/// Validates all profiles in the store and returns a batch summary.
pub fn batch_check_health(store: &ProfileStore) -> Result<HealthCheckSummary, ProfileStoreError> {
    let names = store.list()?;
    let mut profiles = Vec::with_capacity(names.len());

    for name in &names {
        match store.load(name) {
            Ok(profile) => {
                profiles.push(check_profile_health(name, &profile));
            }
            Err(error) => {
                profiles.push(ProfileHealthReport {
                    name: name.clone(),
                    status: HealthStatus::Broken,
                    launch_method: String::new(),
                    issues: vec![HealthIssue {
                        field: "profile".to_string(),
                        path: String::new(),
                        message: format!("Failed to load profile: {error}"),
                        remediation: "The profile TOML may be corrupted. Open it in a text editor or delete and recreate the profile.".to_string(),
                        severity: HealthIssueSeverity::Error,
                    }],
                    checked_at: now_iso8601(),
                });
            }
        }
    }

    let healthy_count = profiles.iter().filter(|p| p.status == HealthStatus::Healthy).count();
    let stale_count = profiles.iter().filter(|p| p.status == HealthStatus::Stale).count();
    let broken_count = profiles.iter().filter(|p| p.status == HealthStatus::Broken).count();

    Ok(HealthCheckSummary {
        total_count: profiles.len(),
        healthy_count,
        stale_count,
        broken_count,
        validated_at: now_iso8601(),
        profiles,
    })
}

// --- Internal helpers ---

fn derive_status(issues: &[HealthIssue]) -> HealthStatus {
    if issues.iter().any(|i| i.severity == HealthIssueSeverity::Error) {
        HealthStatus::Broken
    } else if issues.iter().any(|i| i.severity == HealthIssueSeverity::Warning) {
        HealthStatus::Stale
    } else {
        HealthStatus::Healthy
    }
}

fn check_file_path(
    value: &str, field: &str, label: &str,
    severity: HealthIssueSeverity, remediation: &str,
    issues: &mut Vec<HealthIssue>,
) {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        if severity == HealthIssueSeverity::Error {
            issues.push(HealthIssue {
                field: field.to_string(),
                path: String::new(),
                message: format!("{label} path is not configured."),
                remediation: remediation.to_string(),
                severity,
            });
        }
        return;
    }
    let path = Path::new(trimmed);
    match std::fs::metadata(path) {
        Ok(metadata) => {
            if !metadata.is_file() {
                issues.push(HealthIssue {
                    field: field.to_string(),
                    path: trimmed.to_string(),
                    message: format!("{label} path exists but is not a file."),
                    remediation: remediation.to_string(),
                    severity,
                });
            }
        }
        Err(err) if err.kind() == std::io::ErrorKind::PermissionDenied => {
            issues.push(HealthIssue {
                field: field.to_string(),
                path: trimmed.to_string(),
                message: format!("{label} exists but is not accessible (permission denied)."),
                remediation: "Check file permissions on the path or run CrossHook with appropriate access.".to_string(),
                severity,
            });
        }
        Err(_) => {
            issues.push(HealthIssue {
                field: field.to_string(),
                path: trimmed.to_string(),
                message: format!("{label} path does not exist."),
                remediation: remediation.to_string(),
                severity,
            });
        }
    }
}

fn check_directory_path(
    value: &str, field: &str, label: &str,
    severity: HealthIssueSeverity, remediation: &str,
    issues: &mut Vec<HealthIssue>,
) {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        if severity == HealthIssueSeverity::Error {
            issues.push(HealthIssue {
                field: field.to_string(),
                path: String::new(),
                message: format!("{label} path is not configured."),
                remediation: remediation.to_string(),
                severity,
            });
        }
        return;
    }
    let path = Path::new(trimmed);
    match std::fs::metadata(path) {
        Ok(metadata) => {
            if !metadata.is_dir() {
                issues.push(HealthIssue {
                    field: field.to_string(),
                    path: trimmed.to_string(),
                    message: format!("{label} exists but is not a directory."),
                    remediation: remediation.to_string(),
                    severity,
                });
            }
        }
        Err(err) if err.kind() == std::io::ErrorKind::PermissionDenied => {
            issues.push(HealthIssue {
                field: field.to_string(),
                path: trimmed.to_string(),
                message: format!("{label} exists but is not accessible (permission denied)."),
                remediation: "Check directory permissions or run CrossHook with appropriate access.".to_string(),
                severity,
            });
        }
        Err(_) => {
            issues.push(HealthIssue {
                field: field.to_string(),
                path: trimmed.to_string(),
                message: format!("{label} does not exist."),
                remediation: remediation.to_string(),
                severity,
            });
        }
    }
}

fn check_executable_path(
    value: &str, field: &str, label: &str,
    severity: HealthIssueSeverity, remediation: &str,
    issues: &mut Vec<HealthIssue>,
) {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        if severity == HealthIssueSeverity::Error {
            issues.push(HealthIssue {
                field: field.to_string(),
                path: String::new(),
                message: format!("{label} path is not configured."),
                remediation: remediation.to_string(),
                severity,
            });
        }
        return;
    }
    let path = Path::new(trimmed);
    match std::fs::metadata(path) {
        Ok(metadata) => {
            if !is_executable_file_from_metadata(&metadata) {
                issues.push(HealthIssue {
                    field: field.to_string(),
                    path: trimmed.to_string(),
                    message: format!("{label} exists but is not executable."),
                    remediation: remediation.to_string(),
                    severity,
                });
            }
        }
        Err(err) if err.kind() == std::io::ErrorKind::PermissionDenied => {
            issues.push(HealthIssue {
                field: field.to_string(),
                path: trimmed.to_string(),
                message: format!("{label} exists but is not accessible (permission denied)."),
                remediation: "Check file permissions on the path.".to_string(),
                severity,
            });
        }
        Err(_) => {
            issues.push(HealthIssue {
                field: field.to_string(),
                path: trimmed.to_string(),
                message: format!("{label} does not exist."),
                remediation: remediation.to_string(),
                severity,
            });
        }
    }
}

fn is_executable_file_from_metadata(metadata: &std::fs::Metadata) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        metadata.is_file() && metadata.permissions().mode() & 0o111 != 0
    }
    #[cfg(not(unix))]
    {
        metadata.is_file()
    }
}

fn now_iso8601() -> String {
    // chrono is already a dependency (used in metadata/launch_history.rs, profile_sync.rs)
    chrono::Utc::now().to_rfc3339()
}
```

---

## System Constraints

### Performance on Steam Deck

| Metric                    | Estimate (v1)     | v2 Overhead                         | Total         |
| ------------------------- | ----------------- | ----------------------------------- | ------------- |
| Single profile validation | ~5-10ms           | +2-5ms metadata queries             | ~7-15ms       |
| 50 profiles batch         | ~250-500ms        | +10-20ms (batch metadata queries)   | ~260-520ms    |
| 100 profiles batch        | ~500ms-1s         | +10-20ms                            | ~510ms-1.02s  |
| Health snapshot upsert    | N/A               | ~1ms per profile (SQLite write)     | ~50ms total   |

**v2 note**: Metadata enrichment adds a fixed overhead (~10-20ms) regardless of profile count, because the aggregate queries (`query_last_success_per_profile`, `query_failure_trends`, `query_most_launched`) each scan the `launch_operations` table once. Per-profile `lookup_profile_id` calls add ~1ms each. The health snapshot upsert is done in a single transaction (future optimization) but even individual upserts at ~1ms each are acceptable.

### Startup Time Impact

> **CHANGED from v1**: With health snapshots, the startup flow can show cached health badges immediately, then refresh in background.

1. **App starts** — Profile list renders immediately
2. **Cached health** (NEW) — If MetadataStore available, load `health_snapshots` table (~5ms) and show cached badges immediately with "last checked at" timestamp
3. **Background scan** — Async task (1000ms delay) revalidates all profiles against filesystem
4. **Results arrive** — `profile-health-batch-complete` event fires; badges update atomically; snapshots persisted

This two-phase approach means users see health status instantly on startup (from cache) and get fresh results within ~1.5s. The cache is advisory — if stale, the live check corrects it.

### Fail-Soft Behavior

The MetadataStore integration follows the established fail-soft pattern:

| MetadataStore State | Health Behavior                                      |
| ------------------- | ---------------------------------------------------- |
| Available           | Full enrichment: metadata fields populated, snapshots persisted, cached startup display |
| Unavailable         | Core health only: `metadata: null` on all reports, no snapshots, no cached startup display, filesystem checks still run |
| Corrupted/Error     | Same as unavailable — `with_conn()` returns `T::default()` |

---

## Codebase Changes

### Files to Create

| File                                                          | Purpose                                                                              |
| ------------------------------------------------------------- | ------------------------------------------------------------------------------------ |
| `crates/crosshook-core/src/profile/health.rs`                | Core health types + filesystem validation logic                                      |
| `crates/crosshook-core/src/metadata/health_store.rs`         | Health snapshot SQLite persistence (upsert, load, lookup)                            |
| `src-tauri/src/commands/health.rs`                            | Tauri IPC commands with metadata enrichment                                          |
| `src/types/health.ts`                                         | TypeScript interfaces (enriched types)                                               |
| `src/hooks/useProfileHealth.ts`                               | React hook: invoke + listen + useReducer                                             |
| `src/components/ProfileHealthDashboard.tsx`                   | Dashboard UI with summary bar + per-profile cards                                    |
| `src/components/HealthBadge.tsx`                              | `<HealthBadge status="healthy|stale|broken" />` component                            |

### Files to Modify

| File                                                         | Change                                                                                          |
| ------------------------------------------------------------ | ----------------------------------------------------------------------------------------------- |
| `crates/crosshook-core/src/profile/mod.rs`                   | Add `pub mod health;`                                                                           |
| `crates/crosshook-core/src/metadata/mod.rs`                  | Add `mod health_store;` and public methods (`upsert_health_snapshot`, `load_health_snapshots`, `lookup_health_snapshot`) |
| `crates/crosshook-core/src/metadata/migrations.rs`           | Add `migrate_5_to_6` for `health_snapshots` table; add version 6 check to `run_migrations`     |
| `src-tauri/src/commands/mod.rs`                               | Add `pub mod health;`                                                                           |
| `src-tauri/src/lib.rs`                                        | Register `batch_validate_profiles` and `get_profile_health` in `invoke_handler`; add startup background health check |
| `src/types/index.ts`                                          | Add `export * from './health';`                                                                 |
| `src/App.tsx`                                                 | Integrate `ProfileHealthDashboard` in profile list area                                         |
| `src/styles/variables.css`                                    | Add health badge color variables if not covered by existing severity palette                     |

### Dependencies

**No new crate dependencies.** The `chrono` crate is already in the dependency tree (used in `metadata/launch_history.rs` and `metadata/profile_sync.rs`). All path checks use `std::path::Path` and `std::fs`. The `time` crate alternative from v1 is no longer needed since `chrono` is already available.

---

## Technical Decisions

### Decision 1: Module Location — `profile/health.rs` (Single File)

> **CHANGED from v1**: v1 proposed a top-level `health/` module (3 files). v2 uses `profile/health.rs` (single file).

| Option                             | Pros                                                | Cons                                                     |
| ---------------------------------- | --------------------------------------------------- | -------------------------------------------------------- |
| `health/` top-level (3 files, v1)  | Room to grow; independent test file                 | Premature — MVP is ~300 lines                            |
| **`profile/health.rs` (1 file)**   | Simpler; health is a profile-domain concern; matches feature-spec.md recommendation | Must split if module grows beyond ~500 lines |

**Recommendation**: `profile/health.rs`. Follows the pattern of `profile/exchange.rs` and `profile/legacy.rs`. If health validation grows significantly (e.g., DLL path resolution, trainer host path checking), it can be promoted to a `profile/health/` directory later.

### Decision 2: Health Snapshot Persistence (NEW)

| Option                              | Pros                                              | Cons                                                 |
| ----------------------------------- | ------------------------------------------------- | ---------------------------------------------------- |
| A: No persistence (v1 approach)     | Simplest; no schema change                        | No cached startup display; no health history         |
| **B: `health_snapshots` table**     | Instant startup badges; trend tracking; bounded storage | One more migration (~10 lines DDL)              |
| C: Store full issue detail in SQLite| Complete offline view of last check                | Stale path strings in DB; unbounded issue growth     |

**Recommendation**: Option B. The table stores only status + issue count + timestamp per profile (not full issue detail). Full issues are always recomputed from filesystem. This gives us the startup fast-path without stale data risk.

**Phasing note (per recommendations-agent feedback)**: The original feature-spec.md Business Rule #8 stated health results are "in-memory only, never written to disk." The `health_snapshots` table revises this rule, but introduces a migration coupling risk: adding migration v6 could delay Phase A if schema design is contested. **Mitigation**: The two-layer architecture explicitly decouples snapshot persistence from core health validation. Phase A can ship with metadata *enrichment* only (read from existing tables — `query_last_success_per_profile()`, `query_failure_trends()`, `lookup_profile_id()`) and no new migration. The `health_snapshots` table and migration v6 can be deferred to Phase B or C without affecting the core health check feature, the enriched API contract, or the UI. The `EnrichedProfileHealthReport.metadata` field already handles this gracefully — it is `null` when MetadataStore is unavailable, and the snapshot-dependent fields (cached startup display) are a progressive enhancement on top of the live filesystem check.

### Decision 3: MetadataStore Integration Layer

| Option                                     | Pros                                                | Cons                                            |
| ------------------------------------------ | --------------------------------------------------- | ----------------------------------------------- |
| A: MetadataStore in crosshook-core health  | Single layer; simpler                               | Couples core library to SQLite; harder to test  |
| **B: Two-layer (core + Tauri enrichment)** | Core is store-agnostic and testable; matches codebase pattern | Enrichment logic in Tauri layer, not reusable by CLI |

**Recommendation**: Option B. This exactly matches how `commands/profile.rs` already works — it accepts both `ProfileStore` and `MetadataStore`, does store-agnostic operations first, then enriches with metadata. The CLI can use the core layer directly without MetadataStore.

### Decision 4: Timestamp Generation

> **CHANGED from v1**: v1 proposed the `time` crate or Unix timestamp fallback.

**Resolution**: Use `chrono::Utc::now().to_rfc3339()` directly. The `chrono` crate is already a dependency used in `metadata/launch_history.rs` (line 2) and `metadata/profile_sync.rs` (line 3). No need for `time` crate or manual formatting.

### Decision 5: `is_executable_file` Duplication (UNCHANGED)

The health module duplicates `is_executable_file` logic from `launch/request.rs:742-759`. **Defer extraction** to a shared `fs_util` module in a follow-up. The health module uses `std::fs::Metadata` directly (to distinguish PermissionDenied from NotFound), while `request.rs` calls `fs::metadata(path)` internally. Different calling conventions make a shared abstraction nontrivial for v1.

### Decision 6: Trainer Path Validation (UNCHANGED)

`trainer.path` stores a WINE-mapped path (not a host filesystem path). `trainer_host_path` exists only on `LaunchRequest` (built by the frontend at launch time). **Skip filesystem validation** of `trainer.path` in health checks. Document as a known limitation.

---

## Cross-Team Feedback Integration

### From v1 Revisions (Preserved)

- **R-1 (Path Sanitization)**: All `HealthIssue.path` fields sanitized via `sanitize_display_path()` from `commands/shared.rs` before IPC. Integrated into the Tauri command layer.
- **R-2 (PermissionDenied vs NotFound)**: `std::fs::metadata()` error kinds distinguished. `PermissionDenied` gets separate message and remediation text.
- **R-5 (GameProfile direct validation)**: Health validates `GameProfile` fields directly, not via `LaunchRequest` conversion.
- **R-8 (CSP)**: Remains a project-wide issue. Recommend separate issue.
- **R-9 (Stale vs Broken classification)**: Unchanged — `Error` severity = Broken, `Warning` = Stale.
- **R-10 (DLL path validation)**: Included at `Warning` severity.
- **R-11 (Read-only module doc)**: Module-level doc comment added to `profile/health.rs`.

### New v2 Considerations

- **MetadataStore query performance**: Aggregate queries (`query_last_success_per_profile`, `query_failure_trends`, `query_most_launched`) are O(n) on `launch_operations` table. For users with thousands of launches, these queries should remain fast (<50ms) due to indexed columns. If performance becomes an issue, the `health_snapshots` table provides a fast alternative for badge rendering.
- **Snapshot staleness**: The `checked_at` field in `health_snapshots` lets the UI detect stale snapshots (e.g., ">7 days old") and prompt a re-check. This is a future UX enhancement, not required for v1.
- **Profile deletion cascade**: When a profile is deleted, the `health_snapshots` row is automatically cleaned up via foreign key (`REFERENCES profiles(profile_id)`). No additional cleanup code needed.
- **`is_community_import` detection**: The `profiles.source` column stores the sync source. When `source = 'import'`, the profile was community-imported. This enables the "This profile was imported — paths may need to be updated for your system" contextual note.

---

## Open Questions

1. **Health snapshot on every profile save?** — Should `profile_save` Tauri command trigger a single-profile health recheck and snapshot update? This would keep snapshots fresh without explicit re-check. Cost: ~15ms per save. Recommendation: yes, add to `profile_save` command flow.

2. **Batch metadata query optimization** — The current design queries `query_last_success_per_profile()`, `query_failure_trends(30)`, and `query_most_launched(1000)` separately. These could be combined into a single CTE query for batch validation. Defer optimization until profiling shows it matters.

3. **Launcher drift as health signal** — Should a profile with a `missing` or `stale` drift state on its launcher contribute to the health status? Currently launcher drift is informational-only (part of `ProfileHealthMetadata`). A future enhancement could promote certain drift states to health issues.

4. **CLI health command** — `crosshook health` in `crosshook-cli` can directly call `crosshook_core::profile::health::batch_check_health()` without MetadataStore enrichment. Trivial to implement. Defer to Phase 5 (#43).

5. **Dashboard placement** — Defer to UX research. Options: inline in profile list (badges on each profile card), dedicated sub-tab, or modal triggered from toolbar.

6. **Collection-scoped health checks** — (per recommendations-agent) `list_profiles_in_collection()` returns profile names. A "health check this collection" feature is a simple composition: filter `batch_check_health()` results by collection membership. No new infrastructure needed. Consider as a Phase B enhancement alongside collection UI work.
