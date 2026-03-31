# Offline-First Trainer Management Implementation Plan

CrossHook's offline trainer feature adds trainer type classification via a data-driven TOML catalog (identical to `launch/catalog.rs`), SHA-256 hash caching with stat-based mtime invalidation, and a 0-100 weighted readiness scoring system that surfaces in the health dashboard and pre-flight launch checks. The implementation extends existing modules (`profile/`, `metadata/`, `launch/`, `community/`) and adds a new `offline/` core module with `trainer_type.rs`, `readiness.rs`, `hash.rs`, and `network.rs`. SQLite migration 13 introduces three new tables (`trainer_hash_cache`, `offline_readiness_snapshots`, `community_tap_offline_state`) with bootstrap seeding from existing `version_snapshots.trainer_file_hash` data. No new Rust crate dependencies are required.

## Critically Relevant Files and Documentation

- `docs/plans/offline-trainers/feature-spec.md`: Authoritative synthesized spec — supersedes all individual research files where they conflict
- `docs/plans/offline-trainers/research-business.md`: Business rules BR-1 through BR-8, scoring weights, state machine, score caps per trainer type
- `docs/plans/offline-trainers/research-technical.md`: Architecture diagram, component diagram, integration points, portable vs machine-local data split
- `docs/plans/offline-trainers/research-practices.md`: Reusable code inventory, existing patterns to follow, v1 minimal scope analysis
- `docs/plans/offline-trainers/research-security.md`: Security findings W-1 through W-5, A-6 (git hardening) — several are MUST-FIX
- `docs/plans/offline-trainers/shared.md`: Consolidated shared context with all file references, patterns, and tables
- `src/crosshook-native/crates/crosshook-core/src/launch/catalog.rs`: Exact template for trainer type catalog — `include_str!` + `OnceLock` + merge pattern
- `src/crosshook-native/crates/crosshook-core/src/metadata/health_store.rs`: Direct template for `offline_store.rs` — upsert/load CRUD pattern
- `src/crosshook-native/crates/crosshook-core/src/metadata/migrations.rs`: Sequential migration runner — add `migrate_12_to_13`
- `src/crosshook-native/crates/crosshook-core/src/metadata/models.rs`: Enum pattern template — `TrainerLoadingMode`, `DriftState`, `LaunchOutcome`
- `src/crosshook-native/crates/crosshook-core/src/metadata/version_store.rs`: `hash_trainer_file()`, `compute_correlation_status()` — reuse, don't duplicate
- `src/crosshook-native/crates/crosshook-core/src/onboarding/readiness.rs`: `ReadinessCheckResult` / `evaluate_checks()` — offline pre-flight pattern
- `src/crosshook-native/crates/crosshook-core/src/profile/models.rs`: `TrainerSection`, `effective_profile()`, `storage_profile()`/`portable_profile()` boundary
- `src/crosshook-native/crates/crosshook-core/src/profile/health.rs`: `check_profile_health()`, `HealthIssue`, `ProfileHealthReport` — extension point
- `src/crosshook-native/src-tauri/src/commands/health.rs`: IPC command pattern — `State<'_>`, `Result<T, String>`, `spawn_blocking`
- `src/crosshook-native/src-tauri/src/lib.rs`: Tauri state registration + `invoke_handler!` macro — register new commands here
- `src/crosshook-native/src/hooks/useProfileHealth.ts`: React hook template — `useReducer` + `HookStatus` + `invoke()` + `listen()`
- `src/crosshook-native/src/components/HealthBadge.tsx`: `crosshook-status-chip` CSS pattern for offline badge
- `CLAUDE.md`: Authoritative codebase architecture map, conventions, build commands

## Implementation Plan

### Phase 1: Foundation

#### Task 1.1: Offline module skeleton + trainer type catalog Depends on [none]

**READ THESE BEFORE TASK**

- `src/crosshook-native/crates/crosshook-core/src/launch/catalog.rs`
- `src/crosshook-native/crates/crosshook-core/src/metadata/models.rs`
- `docs/plans/offline-trainers/feature-spec.md`

**Instructions**

Files to Create

- `src/crosshook-native/crates/crosshook-core/src/offline/mod.rs`
- `src/crosshook-native/crates/crosshook-core/src/offline/trainer_type.rs`
- `src/crosshook-native/crates/crosshook-core/src/offline/network.rs`
- `src/crosshook-native/assets/default_trainer_type_catalog.toml`

Files to Modify

- `src/crosshook-native/crates/crosshook-core/src/lib.rs`

Create the `offline/` module in `crosshook-core` with three files:

1. **`offline/mod.rs`**: Module root with `pub mod trainer_type; pub mod network;` and re-exports of key types (`OfflineCapability`, `TrainerTypeEntry`, `TrainerTypeCatalog`).

2. **`offline/trainer_type.rs`**: The core of this task. Follow `launch/catalog.rs` exactly:
   - Define `OfflineCapability` enum with variants: `Full`, `FullWithRuntime`, `ConditionalKey`, `ConditionalSession`, `OnlineOnly`, `Unknown` (default). Use `#[serde(rename_all = "snake_case")]`, `#[default]` on `Unknown`, and implement `as_str() -> &'static str`.
   - Define `TrainerTypeEntry` struct: `id: String`, `display_name: String`, `offline_capability: OfflineCapability`, `requires_network: bool`, `detection_hints: Vec<String>`, `score_cap: Option<u8>`, `info_modal: Option<String>`.
   - Define `TrainerTypeCatalog` wrapping `Vec<TrainerTypeEntry>` with `lookup(id) -> Option<&TrainerTypeEntry>` and `entries() -> &[TrainerTypeEntry]`.
   - Use `include_str!("../../../../assets/default_trainer_type_catalog.toml")` for the embedded default.
   - Implement `load_trainer_type_catalog(user_config_dir, tap_catalog_texts)` and `initialize_trainer_type_catalog()` with `OnceLock<TrainerTypeCatalog>` global — identical pattern to `launch/catalog.rs`.
   - Add `global_trainer_type_catalog() -> &'static TrainerTypeCatalog` accessor.

3. **`offline/network.rs`**: Simple network probe: `pub fn is_network_available() -> bool` using `std::net::TcpStream::connect_timeout("8.8.8.8:53", Duration::from_millis(300))`.

4. **`default_trainer_type_catalog.toml`**: Define minimum 6 entries: `standalone` (cap 100), `cheat_engine` (cap 100), `aurora` (cap 90, `info_modal = "aurora_offline_setup"`), `wemod` (cap 90, `info_modal = "wemod_offline_info"`), `plitch` (cap 80), `unknown` (cap 90). Each entry has `id`, `display_name`, `offline_capability`, `requires_network`, `detection_hints`, `score_cap`.

5. **`lib.rs`**: Add `pub mod offline;` to module declarations.

Write unit tests: `OfflineCapability` serde roundtrip, catalog parse with valid TOML, skip empty id, skip duplicate, parse invalid TOML gracefully, `global_trainer_type_catalog()` returns embedded default. Follow the 8 test patterns in `launch/catalog.rs`.

#### Task 1.2: Profile model extension + settings Depends on [none]

**READ THESE BEFORE TASK**

- `src/crosshook-native/crates/crosshook-core/src/profile/models.rs`
- `src/crosshook-native/crates/crosshook-core/src/settings/mod.rs`
- `src/crosshook-native/crates/crosshook-core/src/profile/legacy.rs`

**Instructions**

Files to Modify

- `src/crosshook-native/crates/crosshook-core/src/profile/models.rs`
- `src/crosshook-native/crates/crosshook-core/src/settings/mod.rs`

1. **`profile/models.rs`**: Add `trainer_type: String` to `TrainerSection` with `#[serde(default = "default_trainer_type")]` where `fn default_trainer_type() -> String { "unknown".to_string() }`. Keep the existing `kind: String` field (serialized as `type` via `#[serde(rename)]`) — it remains the display name. The new `trainer_type` field holds the catalog `id` reference (e.g., `"standalone"`, `"aurora"`). Add `#[serde(skip_serializing_if = "is_default_trainer_type")]` to omit the field from TOML when it's `"unknown"` (keeps existing profiles clean). **Note**: `shared.md` references `trainer_type: Option<TrainerType>` (typed enum) — this is superseded by the `feature-spec.md` decision to use a data-driven TOML catalog with string ID lookups. `String` is intentional here; type safety comes from catalog lookup at runtime, not a compiled enum.

2. **`settings/mod.rs`**: Add `#[serde(default)] pub offline_mode: bool` to `AppSettingsData`. The `#[serde(default)]` on the struct-level `Default` derive handles backward compatibility — existing `settings.toml` files without this field will deserialize with `offline_mode: false`.

Write tests: verify an existing TOML profile string without `trainer_type` deserializes successfully with `trainer_type == "unknown"`. Verify round-trip serialization of a profile with `trainer_type = "aurora"` preserves the field. Verify `settings.toml` without `offline_mode` deserializes correctly.

#### Task 1.3: SQLite migration 13 + offline store Depends on [none]

**READ THESE BEFORE TASK**

- `src/crosshook-native/crates/crosshook-core/src/metadata/migrations.rs`
- `src/crosshook-native/crates/crosshook-core/src/metadata/health_store.rs`
- `src/crosshook-native/crates/crosshook-core/src/metadata/mod.rs`
- `src/crosshook-native/crates/crosshook-core/src/metadata/version_store.rs`

**Instructions**

Files to Create

- `src/crosshook-native/crates/crosshook-core/src/metadata/offline_store.rs`

Files to Modify

- `src/crosshook-native/crates/crosshook-core/src/metadata/migrations.rs`
- `src/crosshook-native/crates/crosshook-core/src/metadata/mod.rs`

1. **`migrations.rs`**: Add `migrate_12_to_13(conn)` following the exact naming pattern. Create three tables:
   - `trainer_hash_cache`: `cache_id TEXT PK`, `profile_id TEXT NOT NULL REFERENCES profiles(profile_id) ON DELETE CASCADE`, `file_path TEXT NOT NULL`, `file_size INTEGER`, `file_modified_at TEXT`, `sha256_hash TEXT NOT NULL`, `verified_at TEXT NOT NULL`, `created_at TEXT NOT NULL`, `updated_at TEXT NOT NULL`. Add `CREATE UNIQUE INDEX idx_trainer_hash_cache_profile_path ON trainer_hash_cache(profile_id, file_path)`.
   - `offline_readiness_snapshots`: `profile_id TEXT PRIMARY KEY REFERENCES profiles(profile_id) ON DELETE CASCADE`, `readiness_state TEXT NOT NULL DEFAULT 'unconfigured'`, `readiness_score INTEGER NOT NULL`, `trainer_type TEXT NOT NULL DEFAULT 'unknown'`, `trainer_present INTEGER NOT NULL DEFAULT 0`, `trainer_hash_valid INTEGER NOT NULL DEFAULT 0`, `trainer_activated INTEGER NOT NULL DEFAULT 0`, `proton_available INTEGER NOT NULL DEFAULT 0`, `community_tap_cached INTEGER NOT NULL DEFAULT 0`, `network_required INTEGER NOT NULL DEFAULT 0`, `blocking_reasons TEXT`, `checked_at TEXT NOT NULL`.
   - `community_tap_offline_state`: `tap_id TEXT PRIMARY KEY REFERENCES community_taps(tap_id) ON DELETE CASCADE`, `has_local_clone INTEGER NOT NULL DEFAULT 0`, `last_sync_at TEXT`, `clone_size_bytes INTEGER`.
   - Bootstrap: `INSERT OR IGNORE INTO trainer_hash_cache (cache_id, profile_id, file_path, sha256_hash, verified_at, created_at, updated_at) SELECT lower(hex(randomblob(16))), profile_id, '', trainer_file_hash, checked_at, datetime('now'), datetime('now') FROM version_snapshots WHERE trainer_file_hash IS NOT NULL AND id IN (SELECT MAX(id) FROM version_snapshots GROUP BY profile_id)`.
   - Add the `if version < 13` check and `PRAGMA user_version 13` update in `run_migrations()`.

2. **`offline_store.rs`**: Follow `health_store.rs` pattern exactly. Create free functions taking `&Connection`:
   - `upsert_trainer_hash_cache(conn, cache_id, profile_id, file_path, file_size, file_modified_at, sha256_hash, verified_at)` — use `ON CONFLICT(profile_id, file_path) DO UPDATE SET` to preserve `created_at`.
   - `lookup_trainer_hash_cache(conn, profile_id, file_path) -> Option<TrainerHashCacheRow>`.
   - `upsert_offline_readiness_snapshot(conn, profile_id, readiness_state, readiness_score, trainer_type, ...)` — use `INSERT OR REPLACE`.
   - `load_offline_readiness_snapshots(conn) -> Vec<OfflineReadinessRow>` — JOIN with `profiles` table, filter `deleted_at IS NULL`.
   - `upsert_community_tap_offline_state(conn, tap_id, has_local_clone, last_sync_at, clone_size_bytes)`.
   - `lookup_community_tap_offline_state(conn, tap_id) -> Option<CommunityTapOfflineRow>`.
   - Define row structs (`TrainerHashCacheRow`, `OfflineReadinessRow`, `CommunityTapOfflineRow`) in this file.

3. **`metadata/mod.rs`**: Add `mod offline_store;` and expose public methods wrapping each `offline_store` function through `with_conn`/`with_conn_mut`.

Write tests: `open_in_memory()` + `run_migrations()` then upsert/load round-trips for each table. Verify migration 13 creates all 3 tables. Verify bootstrap INSERT populates `trainer_hash_cache` from `version_snapshots` fixture data. Verify `ON DELETE CASCADE` from profiles.

#### Task 1.4: Readiness scoring + hash caching + offline commands Depends on [1.1, 1.2, 1.3]

**READ THESE BEFORE TASK**

- `src/crosshook-native/crates/crosshook-core/src/onboarding/readiness.rs`
- `src/crosshook-native/crates/crosshook-core/src/metadata/version_store.rs`
- `src/crosshook-native/src-tauri/src/commands/health.rs`
- `docs/plans/offline-trainers/research-business.md`

**Instructions**

Files to Create

- `src/crosshook-native/crates/crosshook-core/src/offline/readiness.rs`
- `src/crosshook-native/crates/crosshook-core/src/offline/hash.rs`
- `src/crosshook-native/src-tauri/src/commands/offline.rs`

Files to Modify

- `src/crosshook-native/crates/crosshook-core/src/offline/mod.rs`
- `src/crosshook-native/src-tauri/src/commands/mod.rs`
- `src/crosshook-native/src-tauri/src/lib.rs`

1. **`offline/readiness.rs`**: Create a **pure** scoring function (no I/O inside — follows `evaluate_checks()` pattern):

   ```
   pub fn compute_offline_readiness(
       profile_name: &str,
       trainer_type: &str,
       trainer_present: bool,
       trainer_hash_valid: bool,
       game_present: bool,
       proton_available: bool,
       prefix_exists: bool,
       network_required: bool,
       score_cap: Option<u8>,
   ) -> OfflineReadinessReport
   ```

   Scoring weights: `trainer_present=30`, `hash_valid=15`, `game_present=20`, `proton_available=15`, `prefix_exists=10`, `network_not_required=10`. Apply `score_cap` from the trainer type catalog entry. Return `OfflineReadinessReport { profile_name: String, score: u8, readiness_state: String, trainer_type: String, checks: Vec<HealthIssue>, blocking_reasons: Vec<String>, checked_at: String }`. The pure function accepts `profile_name` and `trainer_type` as pass-through parameters from the caller; `checked_at` is set to `Utc::now().to_rfc3339()` at call time. All 7 fields are required for IPC serialization — the TypeScript mirror in Task 3.3 expects every field.

   Also create `check_offline_preflight(profile, conn)` that does the I/O (file existence checks, hash lookup, catalog lookup) and delegates to the pure scoring function. This follows the split in `onboarding/readiness.rs` between `check_system_readiness` (I/O) and `evaluate_checks` (pure).

2. **`offline/hash.rs`**: Create `verify_and_cache_trainer_hash(conn, profile_id, trainer_path) -> Option<HashVerifyResult>`:
   - `stat()` the file for `mtime` and `size`.
   - Call `lookup_trainer_hash_cache(conn, profile_id, file_path)`.
   - If cached entry exists AND `file_size` matches AND `file_modified_at` matches → return cached `sha256_hash` (fast path).
   - Otherwise → call `hash_trainer_file(path)` from `version_store.rs` → `upsert_trainer_hash_cache(conn, ...)` → return new hash.
   - Return `HashVerifyResult { hash: String, from_cache: bool, file_size: u64 }`.

3. **`commands/offline.rs`**: Follow `commands/health.rs` pattern. Create IPC commands:
   - `check_offline_readiness(name: String, store: State<ProfileStore>, metadata_store: State<MetadataStore>) -> Result<OfflineReadinessReport, String>` — loads profile via `store`, resolves `effective_profile()`, does I/O checks, calls `compute_offline_readiness`.
   - `batch_offline_readiness(store: State<ProfileStore>, metadata_store: State<MetadataStore>) -> Result<Vec<OfflineReadinessReport>, String>` — batch version for dashboard.
   - `verify_trainer_hash(name: String, store: State<ProfileStore>, metadata_store: State<MetadataStore>) -> Result<HashVerifyResult, String>` — wraps `verify_and_cache_trainer_hash` in `spawn_blocking`.
   - `check_network_status() -> Result<bool, String>` — wraps `is_network_available()` in `spawn_blocking`.
   - `get_trainer_type_catalog() -> Result<Vec<TrainerTypeEntry>, String>` — returns serialized snapshot of loaded catalog for frontend dropdown.

4. **`commands/mod.rs`**: Add `pub mod offline;`.

5. **`lib.rs`**: Add all 5 offline commands to `tauri::generate_handler![...]`. Call `initialize_trainer_type_catalog()` in the Tauri setup hook alongside existing catalog initialization.

6. **`offline/mod.rs`**: Add `pub mod readiness; pub mod hash;` and re-export key types.

Write tests: `compute_offline_readiness` with all checks passing → score 100 (capped by trainer type). Same with `trainer_present=false` → score 70 (missing 30 weight). `verify_and_cache_trainer_hash` with `tempfile` trainer binary — verify cache hit on second call with same mtime. Verify `OfflineReadinessReport` serde roundtrip.

### Phase 2: Launch + Community Integration

#### Task 2.1: Launch validation errors Depends on [1.4]

**READ THESE BEFORE TASK**

- `src/crosshook-native/crates/crosshook-core/src/launch/request.rs`
- `src/crosshook-native/src-tauri/src/commands/launch.rs`

**Instructions**

Files to Modify

- `src/crosshook-native/crates/crosshook-core/src/launch/request.rs`
- `src/crosshook-native/src-tauri/src/commands/launch.rs`

1. **`launch/request.rs`**: Add `OfflineReadinessInsufficient { score: u8, reasons: Vec<String> }` variant to `ValidationError` (or the existing validation enum). Set severity to `Warning` (not `Fatal`), following the `GamescopeNestedSession` pattern already in this file. The variant surfaces a non-blocking warning — launch proceeds regardless.

2. **`commands/launch.rs`**: In the `launch_game`/`launch_trainer` handlers, after existing validation and before subprocess spawn:
   - Skip offline check entirely if no trainer path is configured (game-only profiles).
   - Call `check_offline_readiness` for the profile.
   - If score < 60, add `OfflineReadinessInsufficient` to the validation warnings (not errors).
   - The warning is surfaced to the UI but does NOT block the launch.

Write tests: verify `OfflineReadinessInsufficient` has Warning severity. Verify profiles without trainer path skip the check.

#### Task 2.2: Community tap offline wiring Depends on [1.3]

**READ THESE BEFORE TASK**

- `src/crosshook-native/crates/crosshook-core/src/community/taps.rs`
- `docs/plans/offline-trainers/research-security.md`

**Instructions**

Files to Modify

- `src/crosshook-native/crates/crosshook-core/src/community/taps.rs`

1. **Git command hardening (A-6)**: In the existing `git_command()` helper function, add three environment variables to the `Command` builder:
   - `.env("GIT_CONFIG_NOSYSTEM", "1")`
   - `.env("GIT_CONFIG_GLOBAL", "/dev/null")`
   - `.env("GIT_TERMINAL_PROMPT", "0")`

2. **`is_tap_available_offline(&self, subscription: &CommunityTapSubscription) -> bool`**: Add method that checks if the local clone directory exists on disk. Simple `workspace_path.exists()` check — no git subprocess needed.

3. **Offline-aware sync**: In `sync_tap()`, catch git fetch failures and write to `community_tap_offline_state` via `MetadataStore`. **Wiring**: The Tauri command layer (`commands/community.rs`) already has `State<'_, MetadataStore>` — pass `&MetadataStore` as a new parameter to `sync_tap()` and `sync_many()`. Do NOT add `MetadataStore` as a field on `CommunityTapStore`; keep it as a call-site dependency injected from the command layer.
   - On successful sync: `upsert_community_tap_offline_state(tap_id, has_local_clone=1, last_sync_at=now, clone_size_bytes)`.
   - On fetch failure with existing local clone: return cached profiles with a "from_cache" indicator instead of propagating error.
   - On fetch failure with no local clone: propagate the error (can't do anything offline).

Write tests: verify `is_tap_available_offline` returns false for non-existent path. Verify git env vars are set in the command builder.

#### Task 2.3: Frontend launch pre-flight wiring Depends on [1.4, 2.1]

**READ THESE BEFORE TASK**

- `src/crosshook-native/src/hooks/useLaunchState.ts`
- `src/crosshook-native/src/components/LaunchPanel.tsx`
- `src/crosshook-native/src/components/pages/LaunchPage.tsx`

**Instructions**

Files to Modify

- `src/crosshook-native/src/hooks/useLaunchState.ts`
- `src/crosshook-native/src/components/LaunchPanel.tsx`
- `src/crosshook-native/src/components/pages/LaunchPage.tsx`

1. **`useLaunchState.ts`**: Before the `launchTrainer()` call, invoke `check_offline_readiness` for the current profile. Store the result in hook state. If the score is below 60, set an `offlineWarning` state field. The warning does NOT prevent launch — the button stays enabled.

2. **`LaunchPanel.tsx`**: Add a `CollapsibleSection` (reuse existing component) for pre-flight validation results. Collapsed by default when all checks pass; expanded automatically when any warning is present. Display each `HealthIssue` from the offline readiness check with severity-appropriate styling.

3. **`LaunchPage.tsx`**: Thread the offline readiness state from `useLaunchState` to `LaunchPanel`. Pass the `offlineWarning` and readiness `checks` array as props.

No new TypeScript types needed yet — use inline types for now. Task 3.3 creates the formal types in `types/offline.ts`, and Task 4.1 (which depends on 3.3) must refactor `LaunchPanel.tsx` and `LaunchPage.tsx` to import from `types/offline.ts` instead of using inline definitions.

### Phase 3: Health + Community UI Integration

#### Task 3.1: Health system integration Depends on [1.4]

**READ THESE BEFORE TASK**

- `src/crosshook-native/crates/crosshook-core/src/profile/health.rs`
- `src/crosshook-native/src-tauri/src/commands/health.rs`

**Instructions**

Files to Modify

- `src/crosshook-native/crates/crosshook-core/src/profile/health.rs`
- `src/crosshook-native/src-tauri/src/commands/health.rs`

1. **`profile/health.rs`**: At the end of `check_profile_health()`, after all existing checks, call `check_offline_preflight()` from `offline/readiness.rs` and append the resulting `HealthIssue` entries (with `field: "offline_readiness"`, `severity: Warning`) to the `issues` vec. This is additive — do NOT modify the existing `HealthStatus` enum (Healthy/Stale/Broken). Offline readiness is a separate dimension surfaced as additional warning issues.

   Only run the offline check for profiles that have a trainer path configured. Skip for game-only profiles.

2. **`commands/health.rs`**: Extend `build_enriched_health_summary` to include offline readiness data. After computing health reports, call `load_offline_readiness_snapshots()` from `MetadataStore` and merge the offline scores into the enriched summary. Persist updated `offline_readiness_snapshots` after each batch health scan.

   Add startup offline scan: in the existing 500ms delayed health scan, also compute and persist offline readiness for all profiles. Emit a `"offline-readiness-scan-complete"` Tauri event.

#### Task 3.2: Community browser cache status Depends on [2.2]

**READ THESE BEFORE TASK**

- `src/crosshook-native/src/components/CommunityBrowser.tsx`
- `src/crosshook-native/src-tauri/src/commands/community.rs`

**Instructions**

Files to Modify

- `src/crosshook-native/src/components/CommunityBrowser.tsx`
- `src/crosshook-native/src-tauri/src/commands/community.rs`

1. **`commands/community.rs`**: Modify `sync_tap` to handle network failure gracefully — when the git fetch fails but a local clone exists, return the cached profiles with a `from_cache: true` flag in the response. Display "last synced" timestamps from `community_tap_offline_state.last_sync_at`.

2. **`CommunityBrowser.tsx`**: When `from_cache: true`, display a banner: "Showing cached profiles (last synced: [date])" with the staleness timestamp. Style using existing `crosshook-status-chip` pattern with a muted/info variant. Community profile **install** (not browse) should show "Network required" error when offline — the browse from cache is allowed, install is not.

#### Task 3.3: TypeScript types + offline readiness hook Depends on [1.1]

**READ THESE BEFORE TASK**

- `src/crosshook-native/src/types/health.ts`
- `src/crosshook-native/src/types/index.ts`
- `src/crosshook-native/src/hooks/useProfileHealth.ts`

**Instructions**

Files to Create

- `src/crosshook-native/src/types/offline.ts`
- `src/crosshook-native/src/hooks/useOfflineReadiness.ts`

Files to Modify

- `src/crosshook-native/src/types/profile.ts`
- `src/crosshook-native/src/types/index.ts`

1. **`types/offline.ts`**: Define TypeScript types mirroring Rust structs:
   - `OfflineCapability = 'full' | 'full_with_runtime' | 'conditional_key' | 'conditional_session' | 'online_only' | 'unknown'`
   - `TrainerTypeEntry = { id: string; display_name: string; offline_capability: OfflineCapability; requires_network: boolean; detection_hints: string[]; score_cap: number | null; info_modal: string | null }`
   - `OfflineReadinessReport = { profile_name: string; score: number; readiness_state: string; trainer_type: string; checks: HealthIssue[]; blocking_reasons: string[]; checked_at: string }`
   - `HashVerifyResult = { hash: string; from_cache: boolean; file_size: number }`

2. **`types/profile.ts`**: Add `trainer_type?: string` to the `TrainerSection` interface.

3. **`types/index.ts`**: Add `export * from './offline'`.

4. **`useOfflineReadiness.ts`**: Follow `useProfileHealth.ts` exactly:
   - `useReducer` with `HookStatus` (idle/loading/loaded/error).
   - Actions: `batch-loading`, `batch-complete`, `single-loading`, `single-complete`, `error`.
   - `batchCheck()`: calls `invoke<OfflineReadinessReport[]>("batch_offline_readiness")`.
   - `checkSingle(name)`: calls `invoke<OfflineReadinessReport>("check_offline_readiness", { name })`.
   - `listen("offline-readiness-scan-complete", ...)` for startup scan event.
   - Load cached snapshots on mount for instant display (same cached-then-live pattern as health hook).

### Phase 4: UI Components

#### Task 4.1: Offline status badge + readiness panel Depends on [3.3]

**READ THESE BEFORE TASK**

- `src/crosshook-native/src/components/HealthBadge.tsx`
- `src/crosshook-native/src/styles/variables.css`

**Instructions**

Files to Create

- `src/crosshook-native/src/components/OfflineStatusBadge.tsx`
- `src/crosshook-native/src/components/OfflineReadinessPanel.tsx`

Files to Modify

- `src/crosshook-native/src/styles/variables.css`

1. **`variables.css`**: Add CSS custom properties: `--offline-ready: #4caf50`, `--offline-partial: #ff9800`, `--offline-not-ready: #f44336`, `--offline-unknown: #9e9e9e`.

2. **`OfflineStatusBadge.tsx`**: Reuse the `crosshook-status-chip` CSS pattern from `HealthBadge.tsx`. States: green (score >= 80, "Offline Ready"), amber (50-79, "Partial"), red (< 50, "Not Ready"), grey (unknown/unconfigured, "Unknown"), spinner (computing). Include `aria-label` with full status context for accessibility. Support gamepad focus via `data-crosshook-focus-root`.

3. **`OfflineReadinessPanel.tsx`**: Expandable detail panel showing per-check results from `OfflineReadinessReport.checks`. Each check displays as a row: icon (pass/fail) + field name + message + remediation text. Show `blocking_reasons` as a highlighted list when present. Styled with existing section patterns.

4. **Type Consolidation (Refactor)**: After creating the UI components above, refactor `LaunchPanel.tsx` and `LaunchPage.tsx` (from Task 2.3) to import formal types from `types/offline.ts` instead of using inline type definitions. Replace any inline type declarations for `OfflineReadinessReport`, `HealthIssue`, or related structures with imports from the centralized types module. This consolidates type definitions to a single source of truth (Task 3.3's `types/offline.ts`).

#### Task 4.2: Trainer type form + profile UI Depends on [3.3, 1.2]

**READ THESE BEFORE TASK**

- `src/crosshook-native/src/components/ProfileFormSections.tsx`
- `src/crosshook-native/src/components/pages/ProfilesPage.tsx`
- `src/crosshook-native/src/components/ui/ThemedSelect.tsx`

**Instructions**

Files to Create

- `src/crosshook-native/src/components/OfflineTrainerInfoModal.tsx`

Files to Modify

- `src/crosshook-native/src/components/ProfileFormSections.tsx`
- `src/crosshook-native/src/components/pages/ProfilesPage.tsx`

1. **`ProfileFormSections.tsx`**: In the trainer section of the profile form, add a `ThemedSelect` dropdown for trainer type. Populate from `get_trainer_type_catalog` command (invoke on mount). Options: each `TrainerTypeEntry` from the catalog, displaying `display_name` with `id` as value. Default to `"unknown"`. Handle loading/error states — on catalog load failure, show fallback list with just "Unknown".

2. **`ProfilesPage.tsx`**: Integrate `OfflineStatusBadge` on profile list rows (next to existing health badge). Show `trainer_type` display name in the profile header area.

3. **`OfflineTrainerInfoModal.tsx`**: Create an instructional modal triggered by the `info_modal` field on `TrainerTypeEntry`. Only Aurora and WeMod have `info_modal` set. Content is platform-aware: use `isSteamDeck` from `useGamepadNav` — Steam Deck shows "ONLINE ONLY" notice (Aurora offline keys require Windows HWID); desktop Linux shows step-by-step offline key setup guide. Modal must include `data-crosshook-focus-root="modal"` for gamepad focus interception.

#### Task 4.3: Health dashboard offline section Depends on [3.1, 3.3]

**READ THESE BEFORE TASK**

- `src/crosshook-native/src/components/pages/HealthDashboardPage.tsx`
- `src/crosshook-native/src/hooks/useOfflineReadiness.ts`

**Instructions**

Files to Modify

- `src/crosshook-native/src/components/pages/HealthDashboardPage.tsx`

1. **`HealthDashboardPage.tsx`**: Add an "Offline Readiness" column to the sortable health table. Display `OfflineStatusBadge` for each profile row. Use data from `useOfflineReadiness` hook. Add a sortable column header for offline score. On row click/expand, show `OfflineReadinessPanel` with per-check detail.

   The offline readiness section should load from cached snapshots first (instant display) and refresh after the batch scan completes — same cached-then-live pattern as the existing health data.

## Advice

- **`feature-spec.md` is authoritative**: Where it conflicts with individual research files (e.g., module placement, hash table vs reuse), `feature-spec.md` wins. Specifically: use a new `offline/` module (not extend existing), use a new `trainer_hash_cache` table (not reuse `version_snapshots`), and the trainer type system is a TOML catalog (not a simple enum).

- **The `trainer.kind` field in `TrainerSection` uses `#[serde(rename = "type")]`**: In TOML it appears as `[trainer] type = "fling"`. The new `trainer_type` field is a SEPARATE key — both coexist. Do not attempt to replace `kind` with the new field. `kind` is the display name; `trainer_type` is the catalog ID.

- **`effective_profile()` is mandatory for all path checks**: Never read `trainer.path` directly from the raw profile — it will be empty on machines where `local_override` provides the path. Always call `profile.effective_profile()` first, then read paths from the effective version.

- **`MetadataStore::disabled()` returns `Ok(T::default())`**: All new `with_conn` methods automatically handle this. But Tauri commands that call `check_offline_preflight()` must handle the case where the conn-dependent I/O returns defaults — the scoring function will receive `trainer_hash_valid=false`, `trainer_present=false` etc., producing a score of 0 with `readiness_state = "unconfigured"`. This is correct behavior.

- **Hash computation MUST use `spawn_blocking`**: `hash_trainer_file()` reads the entire binary into memory. Trainer `.exe` files can be 2-20MB. Always wrap hash operations in `tauri::async_runtime::spawn_blocking` in the Tauri command layer. The stat-based cache fast path avoids re-reads.

- **Migration 13 bootstrap rows have `file_path = ''`**: Bootstrapped rows from `version_snapshots` don't know the file path (it's not stored in that table). The `offline_store::lookup_trainer_hash_cache` must handle empty `file_path` as "needs stat-check on next access" — fall through to the full hash path.

- **Offline readiness scoring is informational only — never blocks launch**: `OfflineReadinessInsufficient` is Warning severity. The launch button stays enabled regardless of score. Missing trainer file IS blocking (handled by existing validation), but a low offline score is advisory.

- **Aurora offline keys do NOT work on Steam Deck**: This is a hard platform constraint. `OfflineTrainerInfoModal` must detect `isSteamDeck` and show "ONLINE ONLY" — do not show the offline key setup guide on Steam Deck.

- **`sanitize_display_path()` for all IPC path strings**: All `HealthIssue` entries with file paths in the `path` field must be sanitized before returning over IPC. Call `sanitize_display_path(&issue.path)` in the Tauri command layer. This replaces the home directory with `~` for display.

- **Verify migration 13 is unclaimed**: Before implementing Task 1.3, run `git grep "user_version.*13" -- "*.rs"` and `git log --all --oneline --grep="migration 13"` to confirm no other branch claims this migration number.
