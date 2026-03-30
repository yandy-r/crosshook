# Architecture Research: trainer-version-correlation

## System Overview

CrossHook is a Tauri v2 desktop application with a Rust backend (`crosshook-core` library crate) and React 18/TypeScript frontend. All persistent state lives in SQLite (`metadata.db`, via `rusqlite`) and TOML profile files; the two are linked via stable UUID profile identifiers. The trainer-version-correlation feature extends the existing SQLite metadata pipeline by adding a new `version_snapshots` table (migration 8→9) and hooking into the post-launch success path and startup reconciliation — all without new Cargo dependencies.

## Relevant Components

- `crates/crosshook-core/src/steam/manifest.rs`: Parses `appmanifest_*.acf` VDF files. Current `parse_manifest()` (private) returns only `(appid, installdir)`; needs a new public `parse_manifest_full()` returning `buildid`, `LastUpdated`, `StateFlags`.
- `crates/crosshook-core/src/steam/vdf.rs`: Generic VDF/KeyValues parser (`VdfNode` with `get_child()` / `find_descendant()`). Already supports arbitrary key extraction — `buildid` access is one call: `app_state_node.get_child("buildid")`.
- `crates/crosshook-core/src/metadata/mod.rs`: `MetadataStore` façade. All DB operations are `pub` wrapper methods delegating to inner sub-modules via `with_conn()` / `with_conn_mut()`. This is where all new `version_store` methods will be exposed.
- `crates/crosshook-core/src/metadata/migrations.rs`: Linear version-gated migration runner (`user_version` pragma). Currently at migration 8. New `migrate_8_to_9()` follows the established `IF NOT EXISTS` + `pragma_update` pattern.
- `crates/crosshook-core/src/metadata/health_store.rs`: Reference implementation for a simple insert/load/lookup store module. Patterns here (single `upsert`, `load_all`, `lookup_by_id`) are the template for `version_store.rs`.
- `crates/crosshook-core/src/metadata/community_index.rs`: Contains `check_a6_bounds()` — the string-length guard for community-sourced strings. **`game_version` and `trainer_version` are NOT currently bounded here** — security fix W1 targets this file.
- `crates/crosshook-core/src/metadata/profile_sync.rs`: Profile UUID management and `observe_profile_write()`. The `sha2` crate (`Sha256`) is already imported here — the same pattern applies to trainer file hashing in `version_store.rs`.
- `crates/crosshook-core/src/profile/models.rs`: `GameProfile` struct — `trainer.path` (the trainer binary path) and `steam.app_id` are the two fields consumed by version tracking.
- `crates/crosshook-core/src/profile/community_schema.rs`: `CommunityProfileMetadata` — already has `game_version: String` and `trainer_version: String` fields that feed community-sourced display data.
- `src-tauri/src/commands/launch.rs`: Orchestrates game/trainer launch. `stream_log_lines()` calls `record_launch_finished()` after process exit — **this is the primary hook point for `upsert_version_snapshot()` on successful launch (BR-1)**.
- `src-tauri/src/commands/health.rs`: `BatchMetadataPrefetch` + `ProfileHealthMetadata` + `EnrichedProfileHealthReport` — the enrichment pipeline that feeds the Health Dashboard. Version fields slot in here.
- `src-tauri/src/commands/community.rs`: Community profile import — the seed point for initial version snapshot from community metadata.
- `src-tauri/src/startup.rs`: `run_metadata_reconciliation()` — the startup hook point for the background version scan after app open.
- `src-tauri/src/lib.rs`: Tauri app setup. `MetadataStore` is registered as Tauri state via `.manage()` and resolved in commands via `app.state::<MetadataStore>()`. New commands registered in `invoke_handler!` macro here.

## Data Flow

```
Steam manifest (appmanifest_<appid>.acf)
  └─ steam/vdf.rs::parse_vdf()
  └─ steam/manifest.rs::parse_manifest_full()  [NEW]
       → ManifestData { build_id, state_flags, last_updated }

Launch success path:
  commands/launch.rs::stream_log_lines()
    → record_launch_finished()  [existing]
    → upsert_version_snapshot() [NEW — on LaunchOutcome::Succeeded only]
         ↳ reads profile.steam.app_id → find manifest path → read build_id
         ↳ reads profile.trainer.path → sha2 hash → trainer_file_hash
         ↳ metadata/version_store.rs::upsert()
               → version_snapshots table (INSERT + prune oldest rows)

Startup scan path:
  startup.rs::run_metadata_reconciliation()  [extended]
    → iterate profiles with steam.app_id
    → read current build_id from manifest
    → compare against latest version_snapshots row
    → emit "version-scan-complete" event { scanned, mismatches }

Health enrichment path:
  commands/health.rs::prefetch_batch_metadata()
    → BatchMetadataPrefetch [extended with version fields]
    → ProfileHealthMetadata [extended with version_status, snapshot_build_id]
    → EnrichedProfileHealthReport.metadata [surfaces to frontend]
```

## Critical Gotcha: `steam.app_id` Is Not In SQLite

The SQLite `profiles` table only stores `profile_id`, `current_filename`, `game_name`, `launch_method`, and `content_hash`. **`steam.app_id` is not promoted to SQLite** — it lives only in the TOML profile file. This has concrete implications for each integration point:

- **Launch hook**: `LaunchRequest` (already in scope in `stream_log_lines()`) carries the full resolved profile including `steam.app_id` — no extra TOML load needed here.
- **Startup scan**: Must iterate profile names, then load each `GameProfile` from `ProfileStore` (TOML) to read `steam.app_id` before looking up its manifest.
- **On-demand `check_version_status(name)`**: Must load the profile from `ProfileStore` inside the Tauri command handler to obtain `steam.app_id`.
- **Health enrichment `BatchMetadataPrefetch`**: Must load profiles from TOML (or accept `app_id` as a pre-resolved input from the caller) — cannot derive `steam.app_id` from the `profiles` table alone.

## Integration Points

1. **`steam/manifest.rs`** — Add `pub fn parse_manifest_full(path) → Result<ManifestData, _>` alongside existing private `parse_manifest()`. Keep existing callers unchanged. `ManifestData` needs `build_id: Option<String>`, `state_flags: Option<u32>`, `last_updated: Option<u64>`.

2. **`metadata/migrations.rs`** — Add `migrate_8_to_9()` function and a `version < 9` gate in `run_migrations()`. Create `version_snapshots` table with multi-row history + two indexes.

3. **`metadata/mod.rs`** — Add `mod version_store;` and four public wrapper methods: `upsert_version_snapshot()`, `load_version_snapshot()`, `lookup_latest_version_snapshot()`, `acknowledge_version_change()`.

4. **`commands/launch.rs`** — In `stream_log_lines()`, after `record_launch_finished()`, call the new version upsert if `report.outcome == LaunchOutcome::Succeeded` and the profile has a `steam.app_id`. The `LaunchRequest` already carries the full profile — `steam.app_id` is directly accessible without an extra TOML load.

5. **`startup.rs`** — Extend `run_metadata_reconciliation()` (or add a separate `run_version_scan()` called from a delayed Tauri task) to scan manifests and emit `version-scan-complete`.

6. **`commands/health.rs`** — Extend `BatchMetadataPrefetch` with a `version_snapshot_map: HashMap<String, VersionSnapshotRow>` and add `version_status`, `snapshot_build_id`, `current_build_id` to `ProfileHealthMetadata`.

7. **`commands/community.rs`** — On import, seed a `version_snapshots` row with `trainer_version` / `game_version` from `CommunityProfileMetadata` and `status = 'untracked'`.

8. **`metadata/community_index.rs`** — Add `MAX_VERSION_BYTES: usize = 256` constant and bounds check calls for `game_version` and `trainer_version` inside `check_a6_bounds()` (security fix W1).

9. **`src-tauri/src/lib.rs`** — Register new Tauri commands from `commands/version.rs` in `invoke_handler!`.

10. **New file: `metadata/version_store.rs`** — Three core functions: `upsert_version_snapshot()`, `lookup_latest_version_snapshot()`, `acknowledge_version_change()`, plus pure `compute_correlation_status()`.

11. **New file: `src-tauri/src/commands/version.rs`** — Four IPC handlers: `check_version_status`, `set_trainer_version`, `get_version_snapshot`, `acknowledge_version_change`.

12. **New file: `src/types/version.ts`** — TypeScript types for `VersionCheckResult`, `VersionSnapshotInfo`, `VersionCorrelationStatus`.

## Key Dependencies

| Dependency                 | Already in workspace?    | Usage in this feature                                    |
| -------------------------- | ------------------------ | -------------------------------------------------------- |
| `rusqlite` (0.38, bundled) | Yes                      | `version_snapshots` table CRUD                           |
| `sha2` (0.10)              | Yes — `profile_sync.rs`  | `Sha256` hash of trainer binary                          |
| `chrono` (0.4)             | Yes                      | RFC 3339 `checked_at` timestamps                         |
| `uuid` (1.x)               | Yes — via `db::new_id()` | Not needed for version_snapshots (uses AUTOINCREMENT PK) |
| `serde` / `serde_json`     | Yes                      | Tauri IPC serialization for version types                |
| `tauri` (Emitter)          | Yes                      | Emit `version-scan-complete` event                       |

**No new Cargo.toml dependencies required.**

### Internal Module Dependencies

```
version_store.rs (new)
  ← metadata/db.rs            (new_id, open helpers)
  ← metadata/MetadataStoreError (shared error type)
  ← sha2                       (Sha256 hashing)
  ← chrono                     (Utc::now().to_rfc3339())

commands/version.rs (new)
  ← crosshook_core::metadata::MetadataStore (version wrapper methods)
  ← crosshook_core::steam::manifest::parse_manifest_full (NEW)
  ← tauri::{State, AppHandle}

commands/launch.rs (modified)
  ← crosshook_core::metadata::MetadataStore::upsert_version_snapshot (NEW)
  ← steam/manifest::parse_manifest_full (NEW)
```

### Schema Relationships

```
profiles (existing, profile_id PK)
  └─ version_snapshots.profile_id  FK ON DELETE CASCADE  [NEW table]
  └─ health_snapshots.profile_id   FK ON DELETE CASCADE  [existing — single-row]
  └─ launch_operations.profile_id  FK                    [existing]
```

Note: `version_snapshots` is multi-row per profile (unlike single-row `health_snapshots`). Mismatch detection always queries `ORDER BY checked_at DESC LIMIT 1`.
