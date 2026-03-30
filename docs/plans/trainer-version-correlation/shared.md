# Trainer Version Correlation

CrossHook's trainer-version-correlation feature tracks the relationship between Steam game build IDs (extracted from `appmanifest_*.acf` VDF files via `steam/manifest.rs`) and trainer binary versions (SHA-256 hash of trainer executable + optional community-sourced version string) to detect when either component has changed since the last successful launch. The feature adds a new `version_snapshots` multi-row history table (migration 8→9) in the existing SQLite metadata layer, a `metadata/version_store.rs` module following the `health_store.rs` facade pattern, four new Tauri IPC commands in `commands/version.rs`, and integrates into three existing paths: post-launch success recording, startup reconciliation scanning, and health dashboard enrichment. All data sources are local filesystem — no new Cargo dependencies, no external APIs, no network calls.

## Relevant Files

### New Files to Create

- src/crosshook-native/crates/crosshook-core/src/metadata/version_store.rs: Core CRUD functions (`upsert_version_snapshot`, `lookup_latest_version_snapshot`, `acknowledge_version_change`) plus pure `compute_correlation_status()` for testability
- src/crosshook-native/src-tauri/src/commands/version.rs: Four Tauri IPC handlers (`check_version_status`, `get_version_snapshot`, `set_trainer_version`, `acknowledge_version_change`)
- src/crosshook-native/src/types/version.ts: TypeScript types (`VersionCheckResult`, `VersionSnapshotInfo`, `VersionCorrelationStatus`)

### Files to Modify

- src/crosshook-native/crates/crosshook-core/src/metadata/migrations.rs: Add `migrate_8_to_9()` creating `version_snapshots` table with indexes (schema currently at version 8)
- src/crosshook-native/crates/crosshook-core/src/metadata/mod.rs: Add `mod version_store;` and four public wrapper methods delegating via `with_conn`/`with_conn_mut`
- src/crosshook-native/crates/crosshook-core/src/metadata/models.rs: Add `VersionSnapshotRow` struct and any shared version types
- src/crosshook-native/crates/crosshook-core/src/steam/manifest.rs: Add `pub fn parse_manifest_full()` returning `ManifestData { build_id, state_flags, last_updated }` alongside existing `parse_manifest()`
- src/crosshook-native/crates/crosshook-core/src/metadata/community_index.rs: Add `MAX_VERSION_BYTES = 256` bounds check for `game_version`/`trainer_version` in `check_a6_bounds()` (Security W1)
- src/crosshook-native/src-tauri/src/commands/launch.rs: After `record_launch_finished()`, call `upsert_version_snapshot()` on `LaunchOutcome::Succeeded` — `LaunchRequest` already carries full profile with `steam.app_id`
- src/crosshook-native/src-tauri/src/commands/health.rs: Extend `BatchMetadataPrefetch` and `ProfileHealthMetadata` with version fields (`version_status`, `snapshot_build_id`, `current_build_id`)
- src/crosshook-native/src-tauri/src/commands/community.rs: On `community_import_profile`, seed initial `version_snapshot` row with `status = 'untracked'` from `CommunityProfileMetadata`
- src/crosshook-native/src-tauri/src/startup.rs: Extend `run_metadata_reconciliation()` with background version scan emitting `version-scan-complete` Tauri event
- src/crosshook-native/src-tauri/src/lib.rs: Register new version commands in `invoke_handler!` macro
- src/crosshook-native/src-tauri/src/commands/mod.rs: Add `pub mod version;` declaration

### Reference Files (Patterns to Follow)

- src/crosshook-native/crates/crosshook-core/src/metadata/health_store.rs: Template for `version_store.rs` — single upsert, load_all, lookup_by_id pattern
- src/crosshook-native/crates/crosshook-core/src/metadata/profile_sync.rs: SHA-256 hashing pattern (`sha2::{Digest, Sha256}`) already used for `content_hash`
- src/crosshook-native/crates/crosshook-core/src/steam/vdf.rs: VDF parser — `VdfNode::get_child("buildid")` for extracting build IDs
- src/crosshook-native/crates/crosshook-core/src/metadata/launch_history.rs: `record_launch_finished()` hook point and `LaunchOutcome` enum usage
- src/crosshook-native/crates/crosshook-core/src/profile/community_schema.rs: `CommunityProfileMetadata` with existing `game_version`/`trainer_version` fields
- src/crosshook-native/src/hooks/useProfileHealth.ts: Frontend hook pattern (`useReducer` + `useCallback` + status state machine)
- src/crosshook-native/src/types/health.ts: TypeScript type pattern for health metadata types

## Relevant Tables

- profiles: UUID `profile_id` is the FK anchor for `version_snapshots` — ON DELETE CASCADE; note `steam.app_id` is NOT in this table (lives only in TOML)
- version_snapshots (NEW): Multi-row history table per profile — `profile_id`, `steam_app_id`, `steam_build_id`, `trainer_version`, `trainer_file_hash`, `human_game_ver`, `status`, `checked_at`; latest row queried via `ORDER BY checked_at DESC LIMIT 1`
- community_profiles: Already has `game_version`, `trainer_version`, `proton_version` — display-only, NEVER used as mismatch baseline (BR-8/W3)
- launch_operations: `status = 'succeeded'` triggers version snapshot recording (BR-1)
- health_snapshots: Single-row per profile (contrast: `version_snapshots` is multi-row); version data enriches the health pipeline via `BatchMetadataPrefetch`

## Relevant Patterns

**MetadataStore Facade with Module Delegation**: All DB operations go through `MetadataStore` wrapper methods using `with_conn()`/`with_conn_mut()` — fail-soft by design (returns `Ok(T::default())` when DB unavailable). See [metadata/mod.rs](src/crosshook-native/crates/crosshook-core/src/metadata/mod.rs) for the facade, [health_store.rs](src/crosshook-native/crates/crosshook-core/src/metadata/health_store.rs) for the store module template.

**Batch Prefetch Pattern**: Before iterating profiles, bulk-load all metadata into `HashMap`s in a single pass (O(1) lookups per profile, no N+1 queries). See [commands/health.rs:81-156](src/crosshook-native/src-tauri/src/commands/health.rs) for `BatchMetadataPrefetch`.

**Enrichment Layer in Tauri Commands**: Core provides focused query functions; command layer assembles composite response structs. Version enrichment follows `EnrichedProfileHealthReport` pattern in [commands/health.rs:26-41](src/crosshook-native/src-tauri/src/commands/health.rs).

**Pure Function + I/O Separation**: Mismatch logic extracted to `compute_correlation_status()` (pure, no I/O) for testability. Pattern: `resolve_launch_method()` in `profile/models.rs`.

**SHA-256 Hashing**: Already used in `profile_sync.rs` via `sha2::{Digest, Sha256}`. Trainer file hashing follows the same pattern — no new crate needed.

**Sequential Migration Runner**: `user_version` PRAGMA pattern — currently at version 8. New `migrate_8_to_9()` follows `IF NOT EXISTS` + PRAGMA update pattern in [migrations.rs](src/crosshook-native/crates/crosshook-core/src/metadata/migrations.rs).

**Tauri IPC Error Handling**: Commands return `Result<T, String>` with `fn map_error(e: impl ToString) -> String { e.to_string() }` defined locally per command module. Non-fatal metadata failures logged with `tracing::warn!` and continued.

## Relevant Docs

**docs/plans/trainer-version-correlation/feature-spec.md**: You _must_ read this when working on any version correlation task — master spec with exact files, schema, API surface, phasing, and resolved decisions.

**docs/plans/trainer-version-correlation/research-business.md**: You _must_ read this when implementing business rules — BR-1 through BR-20, domain model state machine, and 6 key integration constraints (especially: `steam.app_id` is NOT in SQLite `profiles` table).

**docs/plans/trainer-version-correlation/research-security.md**: You _must_ read this before shipping — W1 (A6 bounds gap), W2 (git injection), W3 (community data trust boundary), A8 (DB failure must not block launch).

**docs/plans/trainer-version-correlation/research-practices.md**: You _must_ read this when choosing implementation patterns — 14 reusable code locations with exact file:line references, KISS scope limits, module boundaries.

**docs/plans/trainer-version-correlation/research-external.md**: Read this when implementing Steam manifest parsing — VDF key semantics, `buildid`/`StateFlags`/`LastUpdated` extraction, code examples.

**docs/plans/trainer-version-correlation/research-ux.md**: Read this when implementing frontend — three-layer warning system, version states, button labels, Steam Deck gamepad requirements.

**CLAUDE.md**: You _must_ read this for code conventions, workspace structure, commit message rules, and Tauri IPC patterns.

## Critical Constraints

- **`steam.app_id` not in SQLite**: Must be passed from TOML profile or `LaunchRequest` — cannot JOIN from `profiles` table
- **Community data is display-only (BR-8/W3)**: Community `game_version`/`trainer_version` NEVER drive behavioral outcomes or mismatch comparisons
- **`version_untracked` is NOT an error**: Profiles with no baseline show `status = 'untracked'` — no warning badge (BR-4)
- **No new Cargo dependencies**: All required crates (`rusqlite`, `sha2`, `chrono`, `serde`) already in workspace
- **DB failure must not block launch (A8)**: All version operations go through fail-soft `with_conn*` wrappers
- **Version check NOT in synchronous launch path**: On-demand via startup scan and health dashboard — SD card latency concern
- **Row pruning required (A7)**: Prune to N most recent `version_snapshots` rows per profile on each insert
- **`parse_manifest()` signature frozen**: Add `parse_manifest_full()` alongside — do not modify existing function
- **`StateFlags != 4` → skip check**: Steam update in progress — return `update_in_progress: true`, not a mismatch warning
