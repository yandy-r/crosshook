# Trainer Version Correlation Implementation Plan

This feature adds a `version_snapshots` multi-row SQLite history table (migration 8→9) that tracks Steam game build IDs (from `appmanifest_*.acf` VDF files) against trainer binary SHA-256 hashes to detect when either component changes after a successful launch. The implementation follows the existing `MetadataStore` facade pattern — a new `version_store.rs` module with CRUD functions and a pure `compute_correlation_status()` comparator, four Tauri IPC commands in `commands/version.rs`, and hooks into three existing paths: post-launch success recording in `commands/launch.rs`, startup reconciliation scanning in `startup.rs`, and health dashboard enrichment via `BatchMetadataPrefetch` in `commands/health.rs`. Zero new Cargo dependencies are required; `rusqlite`, `sha2`, `chrono`, and `serde` are already in the workspace.

## Critically Relevant Files and Documentation

- docs/plans/trainer-version-correlation/feature-spec.md: Master spec — exact schema, API surface, phasing, resolved decisions; read before any task
- docs/plans/trainer-version-correlation/research-business.md: BR-1 through BR-20 business rules; domain model state machine; `steam.app_id` not in SQLite constraint
- docs/plans/trainer-version-correlation/research-security.md: W1 (A6 bounds), W2 (git injection), W3 (community trust boundary), A8 (DB must not block launch)
- docs/plans/trainer-version-correlation/research-practices.md: 14 reusable code locations with file:line references; KISS scope limits
- src/crosshook-native/crates/crosshook-core/src/metadata/health_store.rs: Template for version_store.rs — upsert/load/lookup triad with with_conn wrappers
- src/crosshook-native/crates/crosshook-core/src/metadata/migrations.rs: Sequential migration runner — currently at version 8; follow IF NOT EXISTS + pragma_update pattern
- src/crosshook-native/crates/crosshook-core/src/metadata/profile_sync.rs: SHA-256 hashing pattern (sha2::{Digest, Sha256})
- src/crosshook-native/crates/crosshook-core/src/metadata/models.rs: Shared row structs and error types — VersionSnapshotRow goes here
- src/crosshook-native/crates/crosshook-core/src/steam/manifest.rs: VDF manifest parsing — add parse_manifest_full() alongside existing parse_manifest()
- src/crosshook-native/crates/crosshook-core/src/steam/vdf.rs: VdfNode::get_child() already supports arbitrary key extraction
- src/crosshook-native/src-tauri/src/commands/health.rs: BatchMetadataPrefetch + ProfileHealthMetadata enrichment pipeline template
- src/crosshook-native/src-tauri/src/commands/launch.rs: Post-launch hook point — stream_log_lines() → record_launch_finished()
- src/crosshook-native/src-tauri/src/commands/community.rs: community_import_profile — version snapshot seed point; map_error() pattern
- src/crosshook-native/src-tauri/src/startup.rs: run_metadata_reconciliation() — startup scan hook
- src/crosshook-native/src-tauri/src/lib.rs: Tauri app setup — invoke_handler! registration; background spawn pattern (lines 73-101)
- src/crosshook-native/src/hooks/useProfileHealth.ts: Frontend hook pattern — useReducer + useCallback + listen<T> event subscription
- src/crosshook-native/src/types/health.ts: TypeScript type pattern for health metadata
- CLAUDE.md: Code conventions, workspace structure, commit message rules

## Implementation Plan

### Phase 1: Detection Foundation

#### Task 1.1: Steam Manifest Extension Depends on [none]

**READ THESE BEFORE TASK**

- src/crosshook-native/crates/crosshook-core/src/steam/manifest.rs
- src/crosshook-native/crates/crosshook-core/src/steam/vdf.rs
- docs/plans/trainer-version-correlation/research-external.md

**Instructions**

Files to Create

- (none)

Files to Modify

- src/crosshook-native/crates/crosshook-core/src/steam/manifest.rs

Add `pub struct ManifestData` and `pub fn parse_manifest_full(manifest_path: &Path) -> Result<ManifestData, String>` alongside the existing private `parse_manifest()`. **Do NOT modify `parse_manifest()` — its signature is frozen with existing callers.**

`ManifestData` fields: `build_id: String`, `install_dir: String`, `state_flags: Option<u32>`, `last_updated: Option<u64>`.

Implementation: read file → `parse_vdf()` → get `AppState` node → extract `buildid`, `installdir`, `StateFlags`, `LastUpdated` using `get_child()`. Follow the exact same `app_state_node.get_child("appid")` pattern already in `parse_manifest()`.

Validate `build_id` as numeric-only before returning (Security A1 — prevents garbage from corrupted manifests). Use `.chars().all(|c| c.is_ascii_digit())`.

Add unit tests following the existing `#[cfg(test)] mod tests` block in `manifest.rs` (lines 226-346): create fixture ACF strings with `buildid`, `StateFlags`, `LastUpdated` values; test missing fields return defaults; test non-numeric `buildid` rejection.

#### Task 1.2: Schema and Models Depends on [none]

**READ THESE BEFORE TASK**

- src/crosshook-native/crates/crosshook-core/src/metadata/migrations.rs
- src/crosshook-native/crates/crosshook-core/src/metadata/models.rs
- src/crosshook-native/crates/crosshook-core/src/metadata/health_store.rs

**Instructions**

Files to Create

- (none)

Files to Modify

- src/crosshook-native/crates/crosshook-core/src/metadata/models.rs
- src/crosshook-native/crates/crosshook-core/src/metadata/migrations.rs

**In models.rs:** Add `VersionSnapshotRow` struct with fields: `id: i64`, `profile_id: String`, `steam_app_id: String`, `steam_build_id: Option<String>`, `trainer_version: Option<String>`, `trainer_file_hash: Option<String>`, `human_game_ver: Option<String>`, `status: String`, `checked_at: String`. Derive `Debug, Clone`. Add `#[derive(Debug, Clone, Serialize, Deserialize)]` and `#[serde(rename_all = "snake_case")]` for the `VersionCorrelationStatus` enum with variants: `Untracked`, `Matched`, `GameUpdated`, `TrainerChanged`, `BothChanged`, `Unknown`, `UpdateInProgress`. Add `impl VersionCorrelationStatus` with `pub fn as_str(&self) -> &'static str` method. Add a constant `MAX_VERSION_SNAPSHOTS_PER_PROFILE: usize = 20`.

**In migrations.rs:** Add `fn migrate_8_to_9(conn: &Connection) -> Result<(), MetadataStoreError>` after the existing `migrate_7_to_8()`. Create `version_snapshots` table with `id INTEGER PRIMARY KEY AUTOINCREMENT`, `profile_id TEXT NOT NULL REFERENCES profiles(profile_id) ON DELETE CASCADE`, `steam_app_id TEXT NOT NULL DEFAULT ''`, `steam_build_id TEXT`, `trainer_version TEXT`, `trainer_file_hash TEXT`, `human_game_ver TEXT`, `status TEXT NOT NULL DEFAULT 'untracked'`, `checked_at TEXT NOT NULL`. Create two indexes: `idx_version_snapshots_profile_checked ON version_snapshots(profile_id, checked_at DESC)` and `idx_version_snapshots_steam_app_id ON version_snapshots(steam_app_id)`. Add the `if version < 9` guard block in `run_migrations()` after the existing `if version < 8` block, following the exact same `pragma_update(None, "user_version", 9_u32)` pattern.

**Critical:** This is a multi-row table — no `UNIQUE` on `profile_id`, no `INSERT OR REPLACE`. This is different from `health_snapshots` (single-row).

Add a migration smoke test: `MetadataStore::open_in_memory()` → verify `version_snapshots` table exists with correct columns via a simple INSERT/SELECT roundtrip.

#### Task 1.3: Version Store Module Depends on [1.2]

**READ THESE BEFORE TASK**

- src/crosshook-native/crates/crosshook-core/src/metadata/health_store.rs
- src/crosshook-native/crates/crosshook-core/src/metadata/mod.rs
- src/crosshook-native/crates/crosshook-core/src/metadata/profile_sync.rs
- src/crosshook-native/crates/crosshook-core/src/metadata/models.rs

**Instructions**

Files to Create

- src/crosshook-native/crates/crosshook-core/src/metadata/version_store.rs

Files to Modify

- src/crosshook-native/crates/crosshook-core/src/metadata/mod.rs

**Create version_store.rs** following `health_store.rs` as the structural template. Import `rusqlite::{params, Connection, OptionalExtension}`, `sha2::{Digest, Sha256}`, `chrono::Utc`, and the models from `super::models`.

Implement these functions:

1. `pub fn upsert_version_snapshot(conn: &Connection, profile_id: &str, steam_app_id: &str, steam_build_id: Option<&str>, trainer_version: Option<&str>, trainer_file_hash: Option<&str>, human_game_ver: Option<&str>, status: &str) -> Result<(), MetadataStoreError>` — INSERT a new row with `checked_at = Utc::now().to_rfc3339()`, then prune to `MAX_VERSION_SNAPSHOTS_PER_PROFILE` most recent rows per profile_id (DELETE WHERE id NOT IN (SELECT id ... ORDER BY checked_at DESC LIMIT N)). This is NOT `INSERT OR REPLACE` — it's multi-row history.

2. `pub fn lookup_latest_version_snapshot(conn: &Connection, profile_id: &str) -> Result<Option<VersionSnapshotRow>, MetadataStoreError>` — SELECT with `WHERE profile_id = ? ORDER BY checked_at DESC LIMIT 1`, using `.optional()` from `OptionalExtension`.

3. `pub fn load_version_snapshots_for_profiles(conn: &Connection) -> Result<Vec<VersionSnapshotRow>, MetadataStoreError>` — Bulk load the latest snapshot per profile for batch prefetch. Use a subquery: `WHERE id IN (SELECT MAX(id) FROM version_snapshots GROUP BY profile_id)`.

4. `pub fn acknowledge_version_change(conn: &Connection, profile_id: &str) -> Result<(), MetadataStoreError>` — UPDATE the latest row's status to `'matched'` (user confirmed the version combo works).

5. `pub fn compute_correlation_status(current_build_id: &str, snapshot_build_id: Option<&str>, current_trainer_hash: Option<&str>, snapshot_trainer_hash: Option<&str>, state_flags: Option<u32>) -> VersionCorrelationStatus` — Pure function, no I/O. If `state_flags != Some(4)` → `UpdateInProgress`. If no snapshot → `Untracked`. Compare `build_id`: different → `GameUpdated`. Compare trainer hash: different → `TrainerChanged`. Both different → `BothChanged`. All same → `Matched`.

6. `pub fn hash_trainer_file(path: &std::path::Path) -> Option<String>` — Read file bytes via `std::fs::read()`, SHA-256 hash, return lowercase hex. Return `None` on any I/O error.

**In mod.rs:** Add `mod version_store;` declaration. Add 4+ public wrapper methods on `MetadataStore`:

- `pub fn upsert_version_snapshot(...)` via `with_conn_mut` (needs transaction for insert+prune)
- `pub fn lookup_latest_version_snapshot(...)` via `with_conn`
- `pub fn load_version_snapshots_for_profiles(...)` via `with_conn`
- `pub fn acknowledge_version_change(...)` via `with_conn_mut`
- Re-export `compute_correlation_status` and `hash_trainer_file` as public functions on the module.

Add comprehensive `#[cfg(test)] mod tests` with in-memory DB: test upsert→lookup lifecycle, test row pruning at MAX limit, test acknowledge sets status to 'matched', test `compute_correlation_status` for all 6 status transitions, test `load_version_snapshots_for_profiles` returns latest per profile.

#### Task 1.4: Security Fixes (W1 + W2) Depends on [none]

**READ THESE BEFORE TASK**

- src/crosshook-native/crates/crosshook-core/src/metadata/community_index.rs
- src/crosshook-native/crates/crosshook-core/src/community/taps.rs
- docs/plans/trainer-version-correlation/research-security.md

**Instructions**

Files to Create

- (none)

Files to Modify

- src/crosshook-native/crates/crosshook-core/src/metadata/community_index.rs
- src/crosshook-native/crates/crosshook-core/src/community/taps.rs

**W1 — A6 Bounds Fix (community_index.rs):** Add `const MAX_VERSION_BYTES: usize = 256;` alongside the existing `MAX_GAME_NAME_BYTES` and similar constants. In `check_a6_bounds()`, add bounds checks for `game_version`, `trainer_version`, and `proton_version` fields following the exact same pattern used for `game_name` and `description`. These fields are currently unbounded — any community tap could inject arbitrarily long strings.

**W2 — Pinned Commit Validation (taps.rs):** In `checkout_pinned_commit()` (or wherever `pinned_commit` is passed to a git subprocess), validate the string is hex-only (`chars().all(|c| c.is_ascii_hexdigit())`) and 7–64 characters long before passing to `git checkout`. Reject invalid values with an error rather than passing unsanitized input to the subprocess.

Add tests: bounds rejection test for 257-byte version strings; hex validation rejection for `"'; rm -rf /"` and similar injection attempts; valid 40-char hex passes.

**This task can be shipped as a standalone PR before Phase 2 — the fixes have zero dependency on new version tracking code.**

### Phase 2: Launch Integration

#### Task 2.1: Tauri Version Commands Depends on [1.3, 1.1]

**READ THESE BEFORE TASK**

- src/crosshook-native/src-tauri/src/commands/health.rs
- src/crosshook-native/src-tauri/src/commands/community.rs
- src/crosshook-native/src-tauri/src/commands/mod.rs
- src/crosshook-native/src-tauri/src/lib.rs

**Instructions**

Files to Create

- src/crosshook-native/src-tauri/src/commands/version.rs

Files to Modify

- src/crosshook-native/src-tauri/src/commands/mod.rs
- src/crosshook-native/src-tauri/src/lib.rs

**Create commands/version.rs** with `fn map_error(e: impl ToString) -> String { e.to_string() }` at top (local error mapper pattern from `community.rs:12-14`).

Implement four `#[tauri::command]` functions:

1. `pub fn check_version_status(name: String, metadata_store: State<'_, MetadataStore>, profile_store: State<'_, ProfileStore>) -> Result<VersionCheckResult, String>` — Load profile from `profile_store` to get `steam.app_id`. Locate the manifest by iterating `SteamLibrary` paths (from `steam/libraries.rs`) to find `{steamapps_path}/appmanifest_{app_id}.acf` — follow the discovery pattern in `steam/auto_populate.rs`. Call `parse_manifest_full()` to get current `build_id` and `state_flags`. Load latest snapshot from `metadata_store`. Call `compute_correlation_status()`. Return assembled `VersionCheckResult`.

2. `pub fn get_version_snapshot(name: String, metadata_store: State<'_, MetadataStore>) -> Result<Option<VersionSnapshotInfo>, String>` — Simple lookup wrapper returning the latest snapshot row.

3. `pub fn set_trainer_version(name: String, version: String, metadata_store: State<'_, MetadataStore>) -> Result<(), String>` — Manual trainer version hint — upsert a snapshot row with the provided version string.

4. `pub fn acknowledge_version_change(name: String, metadata_store: State<'_, MetadataStore>) -> Result<(), String>` — Resolve profile_id, call `metadata_store.acknowledge_version_change()`.

**In commands/mod.rs:** Add `pub mod version;`.

**In lib.rs:** Register all four commands in the `invoke_handler!` macro list.

Add IPC contract type-cast tests following the pattern in `commands/community.rs:244-286`.

#### Task 2.2: Post-Launch Version Snapshot Hook Depends on [1.3, 1.1]

**READ THESE BEFORE TASK**

- src/crosshook-native/src-tauri/src/commands/launch.rs
- src/crosshook-native/crates/crosshook-core/src/metadata/launch_history.rs
- src/crosshook-native/crates/crosshook-core/src/metadata/models.rs

**Instructions**

Files to Create

- (none)

Files to Modify

- src/crosshook-native/src-tauri/src/commands/launch.rs

**CRITICAL GOTCHA**: The `LaunchRequest` (`request`) is moved into the `spawn_log_stream()` closure. You MUST extract `steam.app_id`, `trainer.path`, and `profile_name` into owned `String`/`Option<String>` variables BEFORE the `spawn_log_stream()` call. Attempting to access `request` after the spawn will fail to compile.

After the existing `record_launch_finished()` call in `stream_log_lines()`, add version snapshot recording gated on `LaunchOutcome::Succeeded` (check `failure_mode == FailureMode::CleanExit`):

1. Find the manifest path by iterating `SteamLibrary` paths (from `steam/libraries.rs`) to locate `{steamapps_path}/appmanifest_{app_id}.acf` using the extracted `steam.app_id` — follow the discovery pattern in `steam/auto_populate.rs`
2. Call `parse_manifest_full()` to get the current `build_id`
3. Call `hash_trainer_file()` on the extracted trainer path
4. Call `metadata_store.upsert_version_snapshot()` with the collected data and `status = compute_correlation_status(...).as_str()`

Use the fail-soft pattern: `if let Err(e) = metadata_store.upsert_version_snapshot(...) { tracing::warn!(%e, "version snapshot upsert failed"); }`. **DB failure must never block launch (A8).**

#### Task 2.3: Startup Version Scan Depends on [1.3, 1.1]

**READ THESE BEFORE TASK**

- src/crosshook-native/src-tauri/src/startup.rs
- src/crosshook-native/src-tauri/src/lib.rs (lines 73-101 — background spawn pattern)

**Instructions**

Files to Create

- (none)

Files to Modify

- src/crosshook-native/src-tauri/src/lib.rs
- src/crosshook-native/src-tauri/src/startup.rs

Add a new `tauri::async_runtime::spawn` block in `lib.rs` following the existing health scan pattern at lines 73-101 (do NOT extend `run_metadata_reconciliation()` directly — it is synchronous and adding a sleep would block startup). The version scan runs as an independent background task:

1. Iterate profiles that have `steam.app_id` (must load from `ProfileStore` TOML — `steam.app_id` is NOT in SQLite `profiles` table)
2. For each profile: find manifest, call `parse_manifest_full()`, check `StateFlags` (skip if != 4 — update in progress)
3. Compare current `build_id` against latest `version_snapshots` row via `lookup_latest_version_snapshot()`
4. Optionally hash trainer file and compare
5. Track `scanned` and `mismatches` counts
6. Emit `version-scan-complete` Tauri event with `{ scanned: u32, mismatches: u32 }` payload

Use a 2–3 second delay (slightly after the health scan) via `sleep(Duration::from_millis(2000))`. This must NOT block app startup — run in a `tauri::async_runtime::spawn` task.

**Emit the event even when zero mismatches** — frontend needs the signal to clear loading indicators.

#### Task 2.4: Health Dashboard Enrichment Depends on [1.3, 2.5]

**READ THESE BEFORE TASK**

- src/crosshook-native/src-tauri/src/commands/health.rs
- src/crosshook-native/src/types/health.ts

**Instructions**

Files to Create

- (none)

Files to Modify

- src/crosshook-native/src-tauri/src/commands/health.rs
- src/crosshook-native/src/types/health.ts

**In health.rs:** Extend `BatchMetadataPrefetch` struct with `version_snapshot_map: HashMap<String, VersionSnapshotRow>`. In `prefetch_batch_metadata()`, call `metadata_store.load_version_snapshots_for_profiles()` and collect into the HashMap keyed by `profile_id` (following `launcher_drift_map` pattern).

Extend `ProfileHealthMetadata` with: `version_status: Option<String>`, `snapshot_build_id: Option<String>`, `current_build_id: Option<String>`, `trainer_version: Option<String>`.

In the profile enrichment loop, look up the version snapshot from the prefetch map and populate the new fields. Version mismatch surfaces as `HealthIssueSeverity::Warning` (not Error — BR-6).

**In health.ts:** Extend the `ProfileHealthMetadata` TypeScript interface to match the new Rust fields: `version_status?: VersionCorrelationStatus` (import from `./version`), `snapshot_build_id?: string | null`, `current_build_id?: string | null`, `trainer_version?: string | null`. Use the typed enum from Task 2.5, not a bare `string`, for type safety.

**`version_untracked` must NOT generate a warning badge** — only `game_updated`, `trainer_changed`, and `both_changed` trigger warnings (BR-4).

#### Task 2.5: Frontend Version Types Depends on [none]

**READ THESE BEFORE TASK**

- src/crosshook-native/src/types/health.ts
- src/crosshook-native/src/types/index.ts

**Instructions**

Files to Create

- src/crosshook-native/src/types/version.ts

Files to Modify

- src/crosshook-native/src/types/index.ts

**Create types/version.ts** with TypeScript types mirroring the Rust IPC payloads:

```typescript
export type VersionCorrelationStatus =
  | 'matched'
  | 'game_updated'
  | 'trainer_changed'
  | 'both_changed'
  | 'untracked'
  | 'unknown'
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

export interface VersionScanComplete {
  scanned: number;
  mismatches: number;
}
```

Use `snake_case` fields to match Rust serde output. Use `| null` for optional fields (not `undefined`). Follow the existing `health.ts` pattern exactly.

**In index.ts:** Add re-export: `export * from './version';`.

### Phase 3: User Experience

#### Task 3.1: Community Import Version Seeding Depends on [1.3]

**READ THESE BEFORE TASK**

- src/crosshook-native/src-tauri/src/commands/community.rs
- src/crosshook-native/crates/crosshook-core/src/profile/community_schema.rs

**Instructions**

Files to Create

- (none)

Files to Modify

- src/crosshook-native/src-tauri/src/commands/community.rs

In `community_import_profile()`, after the existing `observe_profile_write()` block, seed an initial `version_snapshot` row:

1. Check if `result.community_metadata` contains `game_version` or `trainer_version`
2. Call `metadata_store.upsert_version_snapshot()` with `status = 'untracked'`, `human_game_ver` from community `game_version`, `trainer_version` from community `trainer_version`
3. Set `steam_build_id = None` — community data does NOT provide a build ID baseline
4. Use the fail-soft pattern: `if let Err(e) = ... { tracing::warn!(...); }`

**CRITICAL (BR-8/W3):** Community version data is **display-only**. The seeded row uses `status = 'untracked'` — it establishes no baseline for mismatch comparison. Community `game_version` goes into `human_game_ver` (display label) only, NOT into `steam_build_id`.

#### Task 3.2: Launch Page Warning Banner Depends on [2.4, 2.5]

**READ THESE BEFORE TASK**

- src/crosshook-native/src/components/LaunchPanel.tsx
- src/crosshook-native/src/components/HealthBadge.tsx
- src/crosshook-native/src/hooks/useProfileHealth.ts
- docs/plans/trainer-version-correlation/research-ux.md

**Instructions**

Files to Create

- (none)

Files to Modify

- src/crosshook-native/src/components/LaunchPanel.tsx
- src/crosshook-native/src/components/HealthBadge.tsx

In `LaunchPanel.tsx`: Read `version_status` from the health metadata already loaded by `useProfileHealth` context. When status is `game_updated`, `trainer_changed`, or `both_changed`, render a persistent warning strip below the launch button with a message like "Game version has changed since last successful launch" and a "Mark as Verified" action button.

When `update_in_progress` is true, show an info note: "Steam update in progress — version check skipped".

`version_untracked` shows nothing — no badge, no banner (BR-4).

In `HealthBadge.tsx`: Extend the badge logic to show a version mismatch indicator when `version_status` indicates a mismatch. Use `Warning` severity coloring (amber), not `Error` (red) — per BR-6.

Ensure all interactive elements support gamepad navigation via the existing `useGamepadNav` hook patterns.

#### Task 3.3: Profile Page Version Display Depends on [2.5, 2.1]

**READ THESE BEFORE TASK**

- src/crosshook-native/src/components/pages/ProfilesPage.tsx
- src/crosshook-native/src/components/ProfileFormSections.tsx

**Instructions**

Files to Create

- (none)

Files to Modify

- src/crosshook-native/src/components/pages/ProfilesPage.tsx
- src/crosshook-native/src/components/ProfileFormSections.tsx

In `ProfilesPage.tsx`: Add a version status badge to the profile card/header area, consuming `version_status` from the health context (via `useProfileHealth` — same data source as the rest of the page, not a per-profile IPC call). This ensures consistency with the batch-prefetched health data already loaded.

In `ProfileFormSections.tsx`: Add a read-only "Trainer Version" display field showing the current `trainer_version` value from the version snapshot. Add an optional manual "Set Trainer Version" input field that calls the `set_trainer_version` Tauri command for cases where the version can't be auto-detected.

#### Task 3.4: Health Dashboard Version Column Depends on [2.4]

**READ THESE BEFORE TASK**

- src/crosshook-native/src/components/pages/HealthDashboardPage.tsx

**Instructions**

Files to Create

- (none)

Files to Modify

- src/crosshook-native/src/components/pages/HealthDashboardPage.tsx

Add a "Version Status" column to the sortable health dashboard table. Display the `version_status` value from `ProfileHealthMetadata` with appropriate color coding: green for `matched`, amber for `game_updated`/`trainer_changed`/`both_changed`, grey for `untracked`/`unknown`.

Add a bulk "Check All Versions" action button. On click, iterate all displayed profiles and call `invoke('check_version_status', { name })` for each — there is no bulk scan command, so this fans out N individual IPC calls. Show a progress indicator during the scan and refresh the dashboard data when all calls complete.

#### Task 3.5: Mark as Verified Action Depends on [2.1]

**READ THESE BEFORE TASK**

- src/crosshook-native/src/components/ProfileActions.tsx

**Instructions**

Files to Create

- (none)

Files to Modify

- src/crosshook-native/src/components/ProfileActions.tsx

Add a "Mark as Verified" button to the profile action bar, visible only when `version_status` is `game_updated`, `trainer_changed`, or `both_changed`. On click, call `invoke('acknowledge_version_change', { name: profileName })` and refresh the health data. This tells the system the user has verified the trainer still works with the new game version.

Button should be styled as a secondary/subtle action (not primary), positioned near existing health-related actions.

## Advice

- **The #1 implementation trap is in Task 2.2**: `LaunchRequest` is moved into the `spawn_log_stream()` closure. Extract `steam.app_id`, `trainer.path`, and `profile_name` as owned clones BEFORE the spawn call — accessing `request` after it is consumed will not compile. Read `commands/launch.rs` carefully before starting this task.

- **`version_snapshots` is multi-row, not single-row**: Do NOT copy the `INSERT OR REPLACE` pattern from `health_store.rs`. Use plain `INSERT` followed by a pruning `DELETE` in the same `with_conn_mut` transaction. This is the most likely copy-paste error when using `health_store.rs` as a template.

- **`steam.app_id` is nowhere in SQLite**: The `profiles` table stores `profile_id`, `current_filename`, `game_name`, `launch_method`, `content_hash` — but NOT `steam.app_id`. Every code path that needs the app ID must either (a) get it from `LaunchRequest` (Task 2.2), (b) load the full `GameProfile` from `ProfileStore` TOML (Tasks 2.1, 2.3), or (c) receive it from community metadata (Task 3.1). Never attempt a JOIN to get `steam.app_id` from the database.

- **Ship Task 1.4 (security fixes) as a standalone PR first**: W1 (A6 bounds for version strings) and W2 (pinned_commit hex validation) have zero dependencies on new version tracking code. Merging them early eliminates the security gap before any new code reads from those paths. This also reduces the blast radius of the main feature PR.

- **`version_untracked` is NOT an error state**: This is codified in BR-4 and reinforced by a prior lesson in `tasks/lessons.md` ("do not map 'no fresh scan result yet' to an error-like state such as NotFound"). Profiles with no baseline show `status = 'untracked'` — no warning badge, no UI alert, no health issue. Only `game_updated`, `trainer_changed`, and `both_changed` trigger the warning system.

- **Community data is display-only forever (BR-8/W3)**: `community_profiles.game_version` and `trainer_version` are never used as a mismatch comparison baseline. They populate `human_game_ver` (a display label) in the seeded snapshot, but `steam_build_id` stays NULL until a real local launch records the actual build ID. This is an architectural hard constraint.

- **Batch prefetch is mandatory for health enrichment**: The `BatchMetadataPrefetch` pattern in `commands/health.rs` exists to prevent N+1 queries. Task 1.3 must include `load_version_snapshots_for_profiles()` that returns a bulk `HashMap` — per-profile lookups in the enrichment loop would regress the batch prefetch optimization.

- **Version checking never blocks the launch path**: All version I/O (manifest reads, trainer hashing) happens post-launch (Task 2.2), at startup (Task 2.3), or on-demand (Task 2.1). Never add version checking to `validate_launch` or any pre-launch synchronous path — SD card latency on Steam Deck can cause multi-second delays.

- **`StateFlags != 4` means Steam is updating the game**: When `parse_manifest_full()` returns `state_flags != Some(4)`, `compute_correlation_status()` must return `UpdateInProgress` — not a mismatch. Do not surface this as a warning to the user; show an info note instead ("Steam update in progress").

- **Emit `version-scan-complete` even on zero mismatches**: The startup scan event (Task 2.3) must fire regardless of results. The frontend needs this signal to clear loading indicators and transition from "scanning" to "idle" state. Omitting the event on zero mismatches creates a stuck loading state.
