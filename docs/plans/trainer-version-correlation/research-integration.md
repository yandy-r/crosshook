# Research: Integration — Trainer-Version Correlation

## Overview

Trainer-version correlation is a **fully local feature** — no network APIs, third-party services, or new runtime dependencies are required. All integration surfaces are the existing SQLite metadata DB (`metadata.db`), the local Steam ACF manifest filesystem, and Tauri IPC. The feature extends three existing integration layers: `steam/manifest.rs` (add `buildid`/`StateFlags` extraction), `metadata/migrations.rs` (new `version_snapshots` table via migration 8→9), and the Tauri command surface (new `commands/version.rs` module).

---

## API Endpoints (Tauri IPC)

CrossHook uses Tauri v2's `invoke()` IPC pattern — no HTTP routing. All backend entry points are `#[tauri::command]` functions registered in `src-tauri/src/lib.rs`.

### Existing Relevant Commands

| Command                                       | File                        | Description                                                                                                          |
| --------------------------------------------- | --------------------------- | -------------------------------------------------------------------------------------------------------------------- |
| `community_sync`                              | `commands/community.rs:216` | Syncs community taps; calls `metadata_store.index_community_tap_result()` — version integration seeds here on import |
| `community_import_profile`                    | `commands/community.rs:96`  | Imports a community profile; calls `metadata_store.observe_profile_write()` — version seed on import goes here       |
| `launch_game` / launch commands               | `commands/launch.rs`        | Post-`LaunchOutcome::Succeeded` is the primary snapshot-record trigger                                               |
| `run_health_check` / `run_batch_health_check` | `commands/health.rs`        | Batch health prefetch; must be extended to include version mismatch in `BatchMetadataPrefetch`                       |
| `steam_find_game_match`                       | `commands/steam.rs`         | Calls `find_game_match()` — exposes manifest path; related to `buildid` extraction path                              |

### New Commands to Add (Spec-Defined)

| Command                              | Input            | Output                        | Trigger                                  |
| ------------------------------------ | ---------------- | ----------------------------- | ---------------------------------------- |
| `check_version_status(name)`         | `String`         | `VersionCheckResult`          | On-demand from LaunchPanel; startup scan |
| `get_version_snapshot(name)`         | `String`         | `Option<VersionSnapshotInfo>` | Profile page, Launch page display        |
| `set_trainer_version(name, version)` | `String, String` | `()`                          | Manual trainer version hint field        |
| `acknowledge_version_change(name)`   | `String`         | `()`                          | "Mark as Verified" action                |

**Registration point:** `src-tauri/src/lib.rs` — add all four to the existing `invoke_handler!` macro list.

### Tauri Event

| Event                   | Payload                             | Direction          | When                                        |
| ----------------------- | ----------------------------------- | ------------------ | ------------------------------------------- |
| `version-scan-complete` | `{ scanned: u32, mismatches: u32 }` | Backend → Frontend | After startup reconciliation scan completes |

---

## Database Schema

**Storage path:** `~/.local/share/crosshook/metadata.db` (resolved via `directories::BaseDirs::data_local_dir()`)

**Migration framework:** Sequential `user_version` PRAGMA pattern in `metadata/migrations.rs`. Currently at version 8 (migration 7→8 removes `is_pinned`). New work adds migration 8→9.

### Existing Tables (Relevant to Feature)

#### `profiles`

```sql
CREATE TABLE profiles (
    profile_id      TEXT PRIMARY KEY,        -- UUID, FK anchor for all per-profile metadata
    current_filename TEXT NOT NULL UNIQUE,   -- Used for name-based lookups
    current_path    TEXT NOT NULL,
    game_name       TEXT,
    launch_method   TEXT,
    content_hash    TEXT,
    is_favorite     INTEGER NOT NULL DEFAULT 0,
    source_profile_id TEXT REFERENCES profiles(profile_id),
    deleted_at      TEXT,
    created_at      TEXT NOT NULL,
    updated_at      TEXT NOT NULL
);
```

**Key:** `profile_id` is the UUID anchor. Version snapshots must FK to this column with `ON DELETE CASCADE`.

#### `community_profiles`

```sql
CREATE TABLE community_profiles (
    id                  INTEGER PRIMARY KEY AUTOINCREMENT,
    tap_id              TEXT NOT NULL REFERENCES community_taps(tap_id) ON DELETE CASCADE,
    relative_path       TEXT NOT NULL,
    manifest_path       TEXT NOT NULL,
    game_name           TEXT,
    game_version        TEXT,           -- Existing: community-supplied game version string
    trainer_name        TEXT,
    trainer_version     TEXT,           -- Existing: community-supplied trainer version string
    proton_version      TEXT,
    compatibility_rating TEXT,
    author              TEXT,
    description         TEXT,
    platform_tags       TEXT,
    schema_version      INTEGER NOT NULL DEFAULT 1,
    created_at          TEXT NOT NULL
);
```

**Key:** `game_version` and `trainer_version` already stored from community manifests — but currently **display-only, not used for comparisons**. Per `feature-spec.md` BR-8, community version data must remain display-only and never drive behavioral outcomes.

**A6 bounds gap (Security W1):** `check_a6_bounds()` in `metadata/community_index.rs` validates `game_name`, `description`, `platform_tags`, `trainer_name`, `author` — but **does NOT validate `game_version` or `trainer_version`**. Must add `MAX_VERSION_BYTES = 256` check.

#### `launch_operations`

```sql
CREATE TABLE launch_operations (
    operation_id    TEXT PRIMARY KEY,
    profile_id      TEXT REFERENCES profiles(profile_id),
    profile_name    TEXT,
    launch_method   TEXT NOT NULL,
    status          TEXT NOT NULL DEFAULT 'started',   -- 'succeeded' is the trigger
    exit_code       INTEGER,
    signal          INTEGER,
    log_path        TEXT,
    diagnostic_json TEXT,
    severity        TEXT,
    failure_mode    TEXT,
    started_at      TEXT NOT NULL,
    finished_at     TEXT
);
```

**Key:** `status = 'succeeded'` is the event that triggers version snapshot recording per BR-1. Post-launch hook location: `commands/launch.rs` after `record_launch_finished()`.

#### `health_snapshots`

```sql
CREATE TABLE health_snapshots (
    profile_id  TEXT PRIMARY KEY REFERENCES profiles(profile_id) ON DELETE CASCADE,
    status      TEXT NOT NULL,
    issue_count INTEGER NOT NULL DEFAULT 0,
    checked_at  TEXT NOT NULL
);
```

**Key:** Single-row per profile (latest snapshot only). Version mismatch extends this via `BatchMetadataPrefetch` enrichment, not as a parallel table — health commands aggregate version data into `ProfileHealthMetadata`.

#### `external_cache_entries`

```sql
CREATE TABLE external_cache_entries (
    cache_id        TEXT PRIMARY KEY,
    source_url      TEXT NOT NULL,
    cache_key       TEXT NOT NULL UNIQUE,
    payload_json    TEXT,                   -- max 512 KiB enforced
    payload_size    INTEGER NOT NULL DEFAULT 0,
    fetched_at      TEXT NOT NULL,
    expires_at      TEXT,
    created_at      TEXT NOT NULL,
    updated_at      TEXT NOT NULL
);
```

**Future use:** Feature spec identifies this table for Phase 4 SteamDB/PCGamingWiki changelog caching. No version feature v1 dependency.

### New Table: `version_snapshots` (Migration 8→9)

```sql
CREATE TABLE IF NOT EXISTS version_snapshots (
    id                  INTEGER PRIMARY KEY AUTOINCREMENT,
    profile_id          TEXT NOT NULL REFERENCES profiles(profile_id) ON DELETE CASCADE,
    steam_app_id        TEXT,               -- NULL for non-Steam profiles
    steam_build_id      TEXT,               -- Validated numeric-only; NULL until first launch
    trainer_version     TEXT,               -- Opaque string, max 256 bytes
    trainer_file_hash   TEXT,               -- SHA-256 hex of trainer executable
    human_game_ver      TEXT,               -- Display-only from community metadata, max 256 bytes
    status              TEXT NOT NULL DEFAULT 'untracked',
    -- status values: 'untracked', 'matched', 'game_updated', 'trainer_changed', 'both_changed', 'unknown'
    checked_at          TEXT NOT NULL       -- RFC 3339
);
CREATE INDEX IF NOT EXISTS idx_version_snapshots_profile_checked
    ON version_snapshots(profile_id, checked_at DESC);
CREATE INDEX IF NOT EXISTS idx_version_snapshots_steam_app_id
    ON version_snapshots(steam_app_id);
```

**Multi-row design** (per feature-spec decision): unlike `health_snapshots` (single-row), `version_snapshots` is a history table. Mismatch detection queries `ORDER BY checked_at DESC LIMIT 1`. **Row retention pruning required** (Security A7) — prune to N most recent rows per `profile_id` on each insert to prevent unbounded growth.

---

## External Services

**None.** This feature has no external service dependencies. All data sources are local filesystem.

### Local "Services" (Filesystem Interfaces)

#### Steam ACF Manifest (`appmanifest_<appid>.acf`)

| Property             | Value                                                                         |
| -------------------- | ----------------------------------------------------------------------------- |
| Format               | VDF/KeyValues (Valve proprietary key-value format)                            |
| Location             | `<steam_library>/steamapps/appmanifest_<appid>.acf`                           |
| Parser               | `steam/vdf.rs` — `parse_vdf()` returns `VdfNode` tree                         |
| Current extraction   | `steam/manifest.rs:parse_manifest()` — extracts `appid`, `installdir` only    |
| **Needed extension** | Add `parse_manifest_full()` to extract `buildid`, `LastUpdated`, `StateFlags` |

**Fields to extract from `AppState` node:**

| Field         | VDF Key         | Type                                      | Use                                                                           |
| ------------- | --------------- | ----------------------------------------- | ----------------------------------------------------------------------------- |
| `buildid`     | `"buildid"`     | Numeric string (opaque monotonic integer) | Game version anchor per BR-2                                                  |
| `StateFlags`  | `"StateFlags"`  | Integer bitmask                           | 4 = fully installed, 1026 = update in progress (BR: skip check during update) |
| `LastUpdated` | `"LastUpdated"` | Unix timestamp                            | Display in version history                                                    |

**Access pattern:** `app_state_node.get_child("buildid").and_then(|n| n.value.as_ref())` — consistent with existing `appid`/`installdir` extraction in `manifest.rs:110-133`.

**Manifest path discovery:** `find_game_match()` in `manifest.rs` already locates the manifest path (`SteamGameMatch.manifest_path`). The version check path reuses this: given `steam.app_id` from the profile, scan library paths to find `appmanifest_<appid>.acf`.

**Security A1:** Validate `buildid` as numeric-only before storing (prevents garbage from corrupted manifests).

#### Trainer Executable (Local File)

| Property       | Value                                                                                          |
| -------------- | ---------------------------------------------------------------------------------------------- |
| Access         | Direct `std::fs::read()` for SHA-256 hashing                                                   |
| Library        | `sha2` crate (already in `Cargo.toml`) — same lib used by `profile_sync.rs` for `content_hash` |
| Path source    | `profile.trainer.path` (after `effective_profile()` resolution)                                |
| Hash algorithm | SHA-256, stored as lowercase hex string                                                        |

**Change detection logic (BR-5):** If `CommunityProfileMetadata.trainer_version` is present, use that as the version string. Always compute SHA-256 hash regardless — hash is the automated silent-update detector.

---

## Internal Services (Rust Module Communication Patterns)

### MetadataStore (`metadata/mod.rs`)

The `MetadataStore` struct (`Arc<Mutex<Connection>>`) is the single SQLite connection wrapper. All version operations must follow the existing wrapper pattern:

```rust
// Read-only operations use with_conn()
pub fn get_version_snapshot(&self, profile_name: &str) -> Result<Option<VersionSnapshotInfo>, MetadataStoreError> {
    self.with_conn("get version snapshot", |conn| {
        version_store::get_latest_version_snapshot(conn, profile_name)
    })
}

// Write/transaction operations use with_conn_mut()
pub fn upsert_version_snapshot(&self, ...) -> Result<(), MetadataStoreError> {
    self.with_conn_mut("upsert version snapshot", |conn| {
        version_store::upsert_version_snapshot(conn, ...)
    })
}
```

**Availability guard (Security A8):** `with_conn()` / `with_conn_mut()` already silently return `T::default()` when `self.available = false`. All version DB calls are protected. DB failure must NEVER block launch.

**New module:** `metadata/version_store.rs` — CRUD functions + pure `compute_correlation_status()` function.

### Launch History Hook (`commands/launch.rs`)

**Integration point:** After `metadata_store.record_launch_finished(operation_id, exit_code, signal, &report)`, check if `status == LaunchOutcome::Succeeded` and then call `metadata_store.upsert_version_snapshot(...)`.

**Manifest read at launch-record time:** At post-launch success, read the Steam manifest to capture the `buildid` that the game actually ran on. This avoids a TOCTOU gap between "when launch was triggered" and "when snapshot is recorded".

### Startup Reconciliation (`startup.rs`)

**Current behavior:** Loads profiles, syncs metadata.

**New behavior (Phase 2):** After existing sync, spawn async task (2–3 second delay) to iterate profiles with `steam.app_id`, read current manifest `buildid`, compare against latest `version_snapshots` row, emit `version-scan-complete` Tauri event with `{ scanned, mismatches }` counts.

**StateFlags guard:** Skip profiles where manifest `StateFlags != 4` (update in progress) — return `update_in_progress: true`, do not generate mismatch warning.

### Health Pipeline Integration (`commands/health.rs`)

**Current `BatchMetadataPrefetch`:** Bulk-loads launcher drift states and health snapshots for all profiles in one query (avoids N+1). Version data must extend this structure.

**Extension:** Add version fields to `ProfileHealthMetadata`:

```rust
pub struct ProfileHealthMetadata {
    // existing fields...
    pub version_status: Option<String>,      // 'matched', 'game_updated', etc.
    pub snapshot_build_id: Option<String>,   // last known-good buildid
    pub current_build_id: Option<String>,    // current manifest buildid
    pub trainer_version: Option<String>,     // opaque version string
    pub trainer_file_hash: Option<String>,   // SHA-256 for change detection
}
```

Version mismatch surfaces as a `HealthIssue` with `HealthIssueSeverity::Warning` (not Error — BR-6).

### Community Import (`commands/community.rs`)

**On `community_import_profile`:** After `observe_profile_write()`, seed an initial `version_snapshot` with `trainer_version` and `human_game_ver` from `CommunityProfileMetadata` — but `steam_build_id = NULL`, `status = 'untracked'`. This populates version metadata for display without creating a false baseline (BR-4).

**Source data available:** `CommunityProfileManifest.metadata.trainer_version` and `game_version` are already parsed at import time via `CommunityProfileMetadata` struct.

---

## Configuration

### Settings (`settings/mod.rs`, `~/.config/crosshook/settings.toml`)

`AppSettingsData` currently stores: `auto_load_last_profile`, `last_used_profile`, `community_taps`.

No new settings fields are required for v1. Version data is stored exclusively in SQLite, not TOML, keeping profiles portable (per feature-spec decision).

### Profile TOML (`~/.config/crosshook/*.toml`)

`GameProfile.trainer.path` — the trainer executable path used for SHA-256 hashing. No new fields needed in TOML. Version metadata is intentionally SQLite-only and excluded from portable/community exports.

---

## Key Constraints and Gotchas

- **No new crate dependencies**: All required libraries (`rusqlite`, `sha2`, `chrono`, `uuid`, `serde`) are already in `Cargo.toml`.
- **Migration sequencing**: The current schema is at version 8. New migration must be `migrate_8_to_9()` — DO NOT skip or reorder migration steps.
- **`parse_manifest()` is public with callers**: Do not change its signature. Add `parse_manifest_full()` alongside it.
- **Multi-row vs. single-row**: `health_snapshots` is single-row per profile; `version_snapshots` is multi-row history — they follow different update patterns. Do not conflate them.
- **Community data is display-only (BR-8/W3)**: `community_profiles.trainer_version` and `game_version` must NEVER be used as the stored baseline for mismatch comparisons — only data from actual local launches establishes a baseline.
- **`pinned_commit` git injection risk (Security W2)**: Before passing `pinned_commit` to git subprocess, validate it is hex-only (7–64 chars). This is a pre-existing security gap touched by community tap work.
- **A6 bounds missing for version fields (Security W1)**: `check_a6_bounds()` in `community_index.rs` does not currently validate `game_version` or `trainer_version` string lengths — add `MAX_VERSION_BYTES = 256`.
- **`external_cache_entries` TTL**: The cache store already handles expiry and size caps (`MAX_CACHE_PAYLOAD_BYTES = 512 KiB`). Future SteamDB caching can reuse this without schema changes.
- **`MetadataStore::disabled()` path**: Some test/offline configurations create a `MetadataStore` with `available = false`. All version methods must degrade gracefully via the `with_conn*` guard.

---

## Relevant Files

| File                                                    | Role                                                                                 |
| ------------------------------------------------------- | ------------------------------------------------------------------------------------ |
| `crates/crosshook-core/src/metadata/migrations.rs`      | Sequential migration runner — add `migrate_8_to_9()`                                 |
| `crates/crosshook-core/src/metadata/mod.rs`             | `MetadataStore` wrapper — add version module + wrapper methods                       |
| `crates/crosshook-core/src/metadata/community_index.rs` | `check_a6_bounds()` — add version field length validation (Security W1)              |
| `crates/crosshook-core/src/metadata/models.rs`          | Shared row structs and error types — add `VersionSnapshotRow`                        |
| `crates/crosshook-core/src/steam/manifest.rs`           | `parse_manifest()` — add `parse_manifest_full()` with `buildid`/`StateFlags`         |
| `crates/crosshook-core/src/steam/vdf.rs`                | `VdfNode::get_child()` — already supports `buildid` key lookup                       |
| `crates/crosshook-core/src/profile/community_schema.rs` | `CommunityProfileMetadata` — `trainer_version`/`game_version` fields already defined |
| `crates/crosshook-core/src/profile/models.rs`           | `GameProfile` + `TrainerSection` — `trainer.path` is the hash source                 |
| `src-tauri/src/commands/community.rs`                   | `community_import_profile` — version snapshot seeding on import                      |
| `src-tauri/src/commands/launch.rs`                      | Post-success version snapshot recording hook                                         |
| `src-tauri/src/commands/health.rs`                      | `BatchMetadataPrefetch` extension for version fields                                 |
| `src-tauri/src/startup.rs`                              | Background startup version reconciliation scan                                       |
| `src-tauri/src/lib.rs`                                  | `invoke_handler!` — register new version commands                                    |
