# Code Analysis: Trainer Version Correlation

**Scope**: Actionable code patterns, integration points, and implementation guidance extracted from the existing codebase for implementing `trainer-version-correlation`.

---

## Executive Summary

The feature fits cleanly into the existing `MetadataStore` facade architecture. The dominant implementation pattern — a private store module delegating through `with_conn`/`with_conn_mut`, a sequential migration block, and Tauri command enrichment via batch prefetch — is repeated consistently across health, launch history, community index, and launcher sync. `version_store.rs` follows `health_store.rs` line for line in structure. The one structural departure is that `version_snapshots` is a **multi-row** history table (queried `ORDER BY checked_at DESC LIMIT 1`), unlike the single-row-per-profile `health_snapshots`. SHA-256 hashing and chrono are already in the workspace; no new Cargo dependencies are needed.

---

## Existing Code Structure

### Metadata Layer (`crosshook-core/src/metadata/`)

```
mod.rs              # MetadataStore public facade — all pub methods delegate via with_conn / with_conn_mut
models.rs           # Error types, shared enums (LaunchOutcome, SyncSource, DriftState), row structs, constants
migrations.rs       # run_migrations() — sequential if version < N blocks, IF NOT EXISTS DDL
health_store.rs     # TEMPLATE: upsert_health_snapshot / load_health_snapshots / lookup_health_snapshot
launch_history.rs   # record_launch_started / record_launch_finished / sweep_abandoned_operations
profile_sync.rs     # observe_profile_write — SHA-256 hashing pattern, INSERT OR REPLACE upsert
community_index.rs  # A6 bounds check constants; transactional DELETE+INSERT pattern
```

### Command Layer (`src-tauri/src/commands/`)

```
health.rs           # BatchMetadataPrefetch / enrich_profile / build_enriched_health_summary — TEMPLATE
launch.rs           # spawn_log_stream / stream_log_lines — version snapshot INSERT point
community.rs        # community_import_profile — initial 'untracked' snapshot seed point
mod.rs              # pub mod declarations (add `pub mod version;`)
```

### Frontend

```
src/types/health.ts          # TypeScript type pattern (discriminated unions, optional fields)
src/hooks/useProfileHealth.ts # useReducer + useCallback + listen<T> event subscription pattern
```

---

## Implementation Patterns

### 1. MetadataStore Facade Method

Every public operation follows this exact template:

```rust
// Read-only:
pub fn lookup_latest_version_snapshot(
    &self,
    profile_id: &str,
) -> Result<Option<VersionSnapshotRow>, MetadataStoreError> {
    self.with_conn("look up the latest version snapshot", |conn| {
        version_store::lookup_latest_version_snapshot(conn, profile_id)
    })
}

// Mutating (or transaction-required):
pub fn upsert_version_snapshot(
    &self,
    profile_id: &str,
    // ... args
) -> Result<(), MetadataStoreError> {
    self.with_conn_mut("upsert a version snapshot row", |conn| {
        version_store::upsert_version_snapshot(conn, profile_id, /* ... */)
    })
}
```

`with_conn` / `with_conn_mut` both return `Ok(T::default())` when `!self.available` — **fail-soft is automatic**. Never call `unwrap()` on the lock result; the wrapper handles poisoned mutex too.

The `action` string is `&'static str` — required by the `MetadataStoreError::Database` variant.

### 2. Store Module Function Signatures

From `health_store.rs` — the direct template:

```rust
// Takes bare &Connection (not MetadataStore) — module-private from external crates
pub fn upsert_health_snapshot(
    conn: &Connection,
    profile_id: &str,
    status: &str,
    issue_count: usize,
    checked_at: &str,
) -> Result<(), MetadataStoreError> {
    // validation then:
    conn.execute(
        "INSERT OR REPLACE INTO health_snapshots ...",
        params![...],
    )
    .map_err(|source| MetadataStoreError::Database {
        action: "upsert a health snapshot row",
        source,
    })?;
    Ok(())
}

pub fn load_health_snapshots(conn: &Connection) -> Result<Vec<HealthSnapshotRow>, MetadataStoreError> {
    let mut stmt = conn.prepare("SELECT ...").map_err(|source| MetadataStoreError::Database {
        action: "prepare load health snapshots query",
        source,
    })?;
    let rows = stmt.query_map([], |row| Ok(HealthSnapshotRow { ... }))
        .map_err(|source| MetadataStoreError::Database { action: "query ...", source })?
        .collect::<Result<Vec<_>, _>>()
        .map_err(|source| MetadataStoreError::Database { action: "collect ...", source })?;
    Ok(rows)
}

pub fn lookup_health_snapshot(
    conn: &Connection,
    profile_id: &str,
) -> Result<Option<HealthSnapshotRow>, MetadataStoreError> {
    // ...
    stmt.query_row(params![profile_id], |row| Ok(HealthSnapshotRow { ... }))
        .optional()                    // <-- rusqlite::OptionalExtension
        .map_err(|source| MetadataStoreError::Database { action: "lookup ...", source })
}
```

`version_store.rs` **differs from `health_store.rs` in one key way**: `version_snapshots` is multi-row, so `lookup_latest_version_snapshot` uses `ORDER BY checked_at DESC LIMIT 1` rather than a simple unique lookup.

### 3. Migration Block (migrations.rs)

Add at the bottom of `run_migrations()` after the existing `if version < 8` block:

```rust
if version < 9 {
    migrate_8_to_9(conn)?;
    conn.pragma_update(None, "user_version", 9_u32)
        .map_err(|source| MetadataStoreError::Database {
            action: "update metadata schema version",
            source,
        })?;
}
```

Then define:

```rust
fn migrate_8_to_9(conn: &Connection) -> Result<(), MetadataStoreError> {
    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS version_snapshots (
            id              INTEGER PRIMARY KEY AUTOINCREMENT,
            profile_id      TEXT NOT NULL REFERENCES profiles(profile_id) ON DELETE CASCADE,
            steam_app_id    TEXT NOT NULL DEFAULT '',
            steam_build_id  TEXT,
            trainer_version TEXT,
            trainer_file_hash TEXT,
            human_game_ver  TEXT,
            status          TEXT NOT NULL DEFAULT 'untracked',
            checked_at      TEXT NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_version_snapshots_profile_id ON version_snapshots(profile_id);
        CREATE INDEX IF NOT EXISTS idx_version_snapshots_checked_at ON version_snapshots(checked_at);
        ",
    )
    .map_err(|source| MetadataStoreError::Database {
        action: "run metadata migration 8 to 9",
        source,
    })?;
    Ok(())
}
```

Note: multi-row table — no `UNIQUE` on `profile_id`, no `PRIMARY KEY` on `profile_id`. Latest row retrieved by `ORDER BY checked_at DESC LIMIT 1`. Row pruning to N most recent is done in the upsert function (A7).

### 4. Row Pruning Pattern (A7)

After INSERT in `upsert_version_snapshot`, prune old rows:

```rust
// Prune: keep only the N most recent rows per profile
conn.execute(
    "DELETE FROM version_snapshots
     WHERE profile_id = ?1
       AND id NOT IN (
         SELECT id FROM version_snapshots
         WHERE profile_id = ?1
         ORDER BY checked_at DESC
         LIMIT ?2
       )",
    params![profile_id, MAX_VERSION_SNAPSHOTS_PER_PROFILE as i64],
)
.map_err(|source| MetadataStoreError::Database {
    action: "prune version snapshot rows",
    source,
})?;
```

### 5. SHA-256 Hashing Pattern (from profile_sync.rs)

`sha2` is already a workspace dependency. Trainer file hashing:

```rust
use sha2::{Digest, Sha256};
// sha2 is already imported in profile_sync.rs — same workspace dep

fn hash_trainer_file(path: &Path) -> Option<String> {
    let bytes = std::fs::read(path).ok()?;
    let mut hasher = Sha256::new();
    hasher.update(&bytes);
    Some(format!("{:x}", hasher.finalize()))
}
```

`compute_content_hash` in `profile_sync.rs` serializes to TOML then hashes — trainer hashing hashes the raw bytes of the file directly.

### 6. Tauri Command Pattern (from health.rs and community.rs)

```rust
// Local error mapper — define at top of commands/version.rs
fn map_error(e: impl ToString) -> String {
    e.to_string()
}

// Fail-soft metadata operation — warn and continue
if let Err(e) = metadata_store.upsert_version_snapshot(...) {
    tracing::warn!(%e, profile_id, "failed to upsert version snapshot");
}

// Fatal operation — propagate error
let snapshot = metadata_store
    .lookup_latest_version_snapshot(&profile_id)
    .map_err(map_error)?;

// Command signature pattern
#[tauri::command]
pub fn check_version_status(
    profile_id: String,
    metadata_store: State<'_, MetadataStore>,
) -> Result<VersionCheckResult, String> {
    // ...
}
```

### 7. BatchMetadataPrefetch Extension (health.rs)

The existing `BatchMetadataPrefetch` struct gets version fields added. Follow the exact pattern of `launcher_drift_map`:

```rust
// In BatchMetadataPrefetch struct:
version_status_map: HashMap<String, String>,        // profile_id → status
snapshot_build_id_map: HashMap<String, Option<String>>,  // profile_id → build_id

// In prefetch_batch_metadata():
let version_status_map: HashMap<String, String> = metadata_store
    .load_version_statuses_for_profile_ids(&profile_ids)
    .unwrap_or_default()
    .into_iter()
    .collect();

// In enrich_profile():
let version_status = prefetch.version_status_map.get(profile_id.as_deref().unwrap_or("")).cloned();
```

### 8. Startup Version Scan (lib.rs pattern)

The health scan in `lib.rs` (lines 73–101) is the exact template for the version scan:

```rust
// After health scan block, in the setup closure:
{
    let app_handle = app.handle().clone();
    tauri::async_runtime::spawn(async move {
        sleep(Duration::from_millis(800)).await;  // slightly after health scan
        let metadata_store = app_handle.state::<MetadataStore>();
        let profile_store = app_handle.state::<ProfileStore>();
        // ... run version reconciliation ...
        match app_handle.emit("version-scan-complete", &result) {
            Ok(()) => tracing::info!("startup version scan complete"),
            Err(e) => tracing::warn!(%e, "failed to emit version-scan-complete"),
        }
    });
}
```

The version scan must NOT block startup. It runs in the background and emits `"version-scan-complete"` when done.

### 9. parse_manifest_full() (steam/manifest.rs)

The existing `parse_manifest()` (line 110) is private and returns `(String, String)`. Add alongside it:

```rust
pub struct ManifestData {
    pub build_id: String,
    pub install_dir: String,
    pub state_flags: Option<u32>,
    pub last_updated: Option<u64>,
}

pub fn parse_manifest_full(manifest_path: &Path) -> Result<ManifestData, String> {
    let content = fs::read_to_string(manifest_path)
        .map_err(|e| format!("unable to read manifest: {e}"))?;
    let manifest_root = parse_vdf(&content).map_err(|e| e.to_string())?;
    let app_state = manifest_root.get_child("AppState").unwrap_or(&manifest_root);

    let build_id = app_state
        .get_child("buildid")
        .and_then(|n| n.value.as_ref())
        .map(|v| v.trim().to_string())
        .unwrap_or_default();

    let install_dir = app_state
        .get_child("installdir")
        .and_then(|n| n.value.as_ref())
        .map(|v| v.trim().to_string())
        .unwrap_or_default();

    let state_flags = app_state
        .get_child("StateFlags")
        .and_then(|n| n.value.as_ref())
        .and_then(|v| v.trim().parse::<u32>().ok());

    let last_updated = app_state
        .get_child("LastUpdated")
        .and_then(|n| n.value.as_ref())
        .and_then(|v| v.trim().parse::<u64>().ok());

    Ok(ManifestData { build_id, install_dir, state_flags, last_updated })
}
```

`parse_manifest()` signature is **frozen** — do not modify it.

### 10. Pure Function Pattern (compute_correlation_status)

From the spec: separate pure logic from I/O. Follow `resolve_launch_method()` in `profile/models.rs`:

```rust
// In version_store.rs or a submodule — pure, no I/O, fully unit-testable
pub fn compute_correlation_status(
    current_build_id: &str,
    snapshot_build_id: Option<&str>,
    current_trainer_hash: Option<&str>,
    snapshot_trainer_hash: Option<&str>,
    state_flags: Option<u32>,
) -> VersionCorrelationStatus {
    // StateFlags == 4 means fully installed; != 4 means update in progress
    if state_flags.map_or(false, |f| f != 4) {
        return VersionCorrelationStatus::UpdateInProgress;
    }
    match snapshot_build_id {
        None => VersionCorrelationStatus::Untracked,
        Some(snap_id) if snap_id != current_build_id => VersionCorrelationStatus::GameUpdated,
        Some(_) => {
            match (current_trainer_hash, snapshot_trainer_hash) {
                (Some(curr), Some(snap)) if curr != snap => VersionCorrelationStatus::TrainerUpdated,
                _ => VersionCorrelationStatus::Matched,
            }
        }
    }
}
```

### 11. Version Snapshot Insert Point (launch.rs)

In `stream_log_lines` at line 282, after `record_launch_finished`:

```rust
// After the record_launch_finished block (lines 282-297):
if matches!(report.exit_info.failure_mode, FailureMode::CleanExit) {
    // app_id and trainer_path must be captured in the spawn closure BEFORE request is consumed
    let ms2 = metadata_store.clone();
    let app_id2 = request_app_id.clone();   // extracted before spawn_log_stream
    let trainer_path2 = request_trainer_path.clone();
    let profile_name2 = request_profile_name.clone();
    let result = tauri::async_runtime::spawn_blocking(move || {
        ms2.upsert_version_snapshot_from_launch(&profile_name2, &app_id2, &trainer_path2)
    }).await;
    if let Err(e) = result {
        tracing::warn!(%e, "version snapshot upsert join failed");
    }
}
```

**Key constraint**: `request` is consumed into the `spawn_log_stream` closure. You must extract `request.steam.app_id`, `request.trainer_path`, and `request.profile_name` **before** calling `spawn_log_stream`.

### 12. Community Import Seed (community.rs)

After `observe_profile_write` in `community_import_profile`:

```rust
// After existing metadata_store.observe_profile_write block:
if let Some(ref metadata) = result.community_metadata {
    // Only seed if we have version metadata from the community profile
    if let Err(e) = metadata_store.seed_version_snapshot_untracked(
        &result.profile_name,
        metadata.game_version.as_deref(),
        metadata.trainer_version.as_deref(),
    ) {
        tracing::warn!(
            %e,
            profile_name = %result.profile_name,
            "version snapshot seed after community import failed"
        );
    }
}
```

This seeds `status = 'untracked'` — never drives behavioral outcomes (BR-8/W3).

### 13. TypeScript Types Pattern (from health.ts)

```typescript
// src/types/version.ts

export type VersionCorrelationStatus =
  | 'matched'
  | 'game_updated'
  | 'trainer_updated'
  | 'both_updated'
  | 'untracked'
  | 'update_in_progress';

export interface VersionSnapshotInfo {
  profile_id: string;
  steam_app_id: string;
  steam_build_id: string | null;
  trainer_version: string | null;
  trainer_file_hash: string | null;
  human_game_ver: string | null;
  status: VersionCorrelationStatus;
  checked_at: string;
}

export interface VersionCheckResult {
  profile_id: string;
  current_build_id: string | null;
  snapshot: VersionSnapshotInfo | null;
  status: VersionCorrelationStatus;
  update_in_progress: boolean;
}
```

### 14. TypeScript Hook Pattern (from useProfileHealth.ts)

```typescript
// Minimal hook shape for version correlation:
type VersionState = {
  status: 'idle' | 'loading' | 'loaded' | 'error';
  results: Record<string, VersionCheckResult>;
  error: string | null;
};

// Event listener pattern:
const unlistenVersionScan = listen<VersionScanComplete>('version-scan-complete', (event) =>
  dispatch({ type: 'scan-complete', payload: event.payload })
);

// invoke pattern:
const result = await invoke<VersionCheckResult>('check_version_status', { profileId });
```

---

## Integration Points

### Files to Create

| File                                                  | Purpose                                                                                                                                                                          |
| ----------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `crates/crosshook-core/src/metadata/version_store.rs` | Core CRUD: `upsert_version_snapshot`, `lookup_latest_version_snapshot`, `load_version_snapshots_for_profiles`, `acknowledge_version_change`, `compute_correlation_status` (pure) |
| `src-tauri/src/commands/version.rs`                   | Four Tauri commands: `check_version_status`, `get_version_snapshot`, `set_trainer_version`, `acknowledge_version_change`                                                         |
| `src/types/version.ts`                                | TypeScript types: `VersionCheckResult`, `VersionSnapshotInfo`, `VersionCorrelationStatus`                                                                                        |

### Files to Modify

| File                          | What Changes                                                                                                                                         | Key Detail                                                                          |
| ----------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------- |
| `metadata/migrations.rs`      | Add `migrate_8_to_9()` + `if version < 9` block                                                                                                      | Append after `if version < 8` block; multi-row table, no UNIQUE on profile_id       |
| `metadata/mod.rs`             | Add `mod version_store;`, `pub use version_store::VersionSnapshotRow`, 4+ public wrapper methods                                                     | Use `with_conn_mut` for upsert (needs transaction for prune); `with_conn` for reads |
| `metadata/models.rs`          | Add `VersionSnapshotRow` struct, `MAX_VERSION_BYTES = 256`                                                                                           | Follow `HealthSnapshotRow` shape                                                    |
| `steam/manifest.rs`           | Add `pub fn parse_manifest_full()` + `pub struct ManifestData`                                                                                       | Do NOT touch `parse_manifest()`                                                     |
| `metadata/community_index.rs` | Add `MAX_VERSION_BYTES: usize = 256` to A6 bounds constants, call in `check_a6_bounds()`                                                             | Parallel to existing `MAX_GAME_NAME_BYTES`                                          |
| `commands/launch.rs`          | Extract `app_id`, `trainer_path`, `profile_name` before `spawn_log_stream`; add version snapshot write after `record_launch_finished` on `CleanExit` | These are consumed by the closure — extract BEFORE `spawn_log_stream`               |
| `commands/health.rs`          | Extend `BatchMetadataPrefetch` and `ProfileHealthMetadata` with version fields                                                                       | Follow `launcher_drift_map` pattern for HashMap prefetch                            |
| `commands/community.rs`       | In `community_import_profile`: seed initial `'untracked'` snapshot                                                                                   | After existing `observe_profile_write` block, same fail-soft pattern                |
| `startup.rs`                  | Extend `run_metadata_reconciliation` with version scan call                                                                                          | Or add separate background spawn in `lib.rs` like health scan                       |
| `lib.rs`                      | Register new version commands in `invoke_handler!`                                                                                                   | Add `commands::version::*` entries                                                  |
| `commands/mod.rs`             | Add `pub mod version;`                                                                                                                               | Follows existing `pub mod health;` pattern                                          |

---

## Code Conventions

### Rust

- `action: &'static str` in `MetadataStoreError::Database` — all action strings must be string literals
- `with_conn` for reads, `with_conn_mut` when a transaction is needed
- `tracing::warn!(%error, field = val, "description")` — structured format, percent-sign for Display impls
- `use chrono::Utc; let now = Utc::now().to_rfc3339();` — timestamps always RFC3339
- Row structs: `#[derive(Debug, Clone)]` + `#[allow(dead_code)]` when module-internal
- IPC-bound types: `#[derive(Debug, Clone, Serialize, Deserialize)]` + `#[serde(rename_all = "snake_case")]` for enums
- `rusqlite::OptionalExtension` trait must be imported for `.optional()`
- `use rusqlite::{params, Connection, OptionalExtension};` — standard import line for store modules
- `db::new_id()` for UUID generation (already used in `health_store` is NOT the case — health_store has no id; version_snapshots has AUTOINCREMENT id, no need to generate)

### TypeScript

- Type union strings for status enums (not enums)
- `| null` for optional metadata fields (not `undefined`)
- `useReducer` + `useCallback` pattern for async state machines
- `listen<T>()` for Tauri events; `invoke<T>()` for commands
- `normalizeError(e: unknown): string` helper pattern

---

## Dependencies and Services

### Already Available — No New Deps

| Dep        | Module                                 | Used For                                   |
| ---------- | -------------------------------------- | ------------------------------------------ |
| `rusqlite` | `metadata/*`                           | All SQLite operations                      |
| `sha2`     | `profile_sync.rs`                      | SHA-256 hashing — same crate, same pattern |
| `chrono`   | `launch_history.rs`, `profile_sync.rs` | RFC3339 timestamps                         |
| `serde`    | All IPC types                          | Serialization                              |
| `tracing`  | All command modules                    | Structured logging                         |

### Service State (Tauri Managed)

- `MetadataStore` — already `manage()`d in `lib.rs`; available via `State<'_, MetadataStore>` in any command
- `ProfileStore` — already managed; needed in startup scan to iterate profiles and find manifest paths
- No new managed state required

---

## Gotchas and Warnings

### G1: `steam.app_id` Not in SQLite `profiles` Table

The `profiles` table has NO `steam_app_id` column. App ID lives in the TOML profile's `steam.app_id` field and on `LaunchRequest.steam.app_id`. Must be passed explicitly into every version snapshot write. **Never JOIN from `profiles` to get app_id** — it doesn't exist there.

### G2: `parse_manifest()` Signature Is Frozen

The existing `fn parse_manifest(manifest_path: &Path) -> Result<(String, String), String>` is used by `find_game_match`. Do NOT rename or modify it. Add `pub fn parse_manifest_full()` as a new sibling function.

### G3: `request` Is Consumed Into the Spawn Closure

In `stream_log_lines`, `request` is moved. If you need `app_id`, `trainer_path`, and `profile_name` inside the async log-stream task to write the version snapshot, extract them **before** calling `spawn_log_stream`. The function currently only receives `operation_id` — extend its signature or pass the extracted fields separately.

### G4: Multi-Row vs Single-Row Table

`health_snapshots` uses `profile_id TEXT PRIMARY KEY` (one row per profile, `INSERT OR REPLACE`). `version_snapshots` has `id INTEGER PRIMARY KEY AUTOINCREMENT` and multiple rows per profile. The latest is fetched with `ORDER BY checked_at DESC LIMIT 1`. Row pruning (A7) runs on every upsert.

### G5: `StateFlags != 4` Is Not an Error

`StateFlags = 4` means "fully installed" in Steam. Any other value means an update is in progress. `compute_correlation_status` must return `UpdateInProgress` (not a mismatch) when `state_flags != Some(4)`. Don't surface this as a warning badge.

### G6: `version_untracked` Is Not an Error (BR-4)

Profiles with no baseline show `status = 'untracked'`. No warning badge, no UI alert. Only `game_updated`, `trainer_updated`, and `both_updated` trigger the three-level warning system.

### G7: Community Data Is Display-Only (W3/BR-8)

`community_profiles.game_version` and `community_profiles.trainer_version` are NEVER used as a mismatch baseline. They are display metadata only. The seeded `'untracked'` snapshot from a community import uses `status = 'untracked'` — it does not pre-populate comparison values.

### G8: DB Failure Must Not Block Launch (A8)

All version snapshot writes in the launch path use `if let Err(e)` fail-soft pattern, not `?`. See health snapshot write in `health.rs:239–252` for the exact idiom.

### G9: Version Check Not in Synchronous Launch Path

Version checking (reading manifest, hashing trainer) must NOT happen in the `validate_launch` or pre-launch path. SD card latency can cause multi-second delays. All version reads happen in the startup scan or on-demand via the health dashboard.

### G10: `nullable_text()` Helper for Option<String> Columns

`community_index.rs` defines a module-private helper used for all optional text DB columns:

```rust
fn nullable_text(value: &str) -> Option<String> {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed.to_string())
    }
}
```

`version_store.rs` must replicate this helper for inserting `trainer_version`, `human_game_ver`, `steam_build_id`, and `trainer_file_hash` — all of which are optional columns that must store `NULL` rather than empty string. Do not pass `""` to `params![]` for these fields.

### G11: `with_conn_mut` Needed for Prune+Insert Transaction

The upsert + prune sequence needs a transaction to be atomic. Use `with_conn_mut` for the facade wrapper (matches `community_index.rs` pattern which also uses `with_conn_mut` for its transactional DELETE+INSERT).

---

## Task-Specific Guidance

### Phase 1: Schema + Core (version_store.rs, migrations.rs, models.rs)

Start with the migration — it's self-contained and tests can be written immediately using `MetadataStore::open_in_memory()`. Then write `version_store.rs` functions one at a time, each with a corresponding `#[test]` that opens an in-memory store. The `compute_correlation_status` pure function needs only unit tests with no DB.

### Phase 2: Steam Integration (manifest.rs)

`parse_manifest_full` only needs the VDF parser — test by writing a temp `.acf` file (see existing `#[test]` in `manifest.rs` lines 226–346 for the exact pattern). The `StateFlags` field is a decimal integer in VDF format.

### Phase 3: Launch Hook (commands/launch.rs)

The trickiest change. Extract `app_id = request.steam.app_id.clone()`, `trainer_path = request.trainer_path.clone()`, `profile_name = request.profile_name.clone()` before `spawn_log_stream`. Inside `stream_log_lines`, after `record_launch_finished` succeeds and `failure_mode == CleanExit`, call `upsert_version_snapshot`.

### Phase 4: Health Enrichment (commands/health.rs)

Add `version_status_map: HashMap<String, String>` and `snapshot_build_id_map: HashMap<String, Option<String>>` to `BatchMetadataPrefetch`. Call `metadata_store.load_version_statuses_for_profile_ids(...)` in `prefetch_batch_metadata`. Extend `ProfileHealthMetadata` with `version_status: Option<String>`.

### Phase 5: IPC Commands (commands/version.rs)

Four commands: `check_version_status` (read + live manifest check), `get_version_snapshot` (read cached row), `set_trainer_version` (manual override), `acknowledge_version_change` (clear mismatch). Register all four in `lib.rs` `invoke_handler!` and add `pub mod version;` to `commands/mod.rs`.

### Phase 6: Frontend (types/version.ts + hook)

Follow `health.ts` for types. The new hook listens for `"version-scan-complete"` the same way `useProfileHealth` listens for `"profile-health-batch-complete"`. Cached snapshots advisory pattern (ignore errors, display stale data).
