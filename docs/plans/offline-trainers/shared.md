# Offline-First Trainer Management

CrossHook's offline trainer feature adds trainer type classification, SHA-256 hash caching with stat-based invalidation, and a 0-100 readiness scoring system so users know whether their game+trainer setup will work without network access. The feature integrates into the existing profile/health/launch pipeline: a new `OfflineCapability` enum (compiled) plus a data-driven trainer type catalog (TOML, same architecture as `launch/catalog.rs`) classify trainers; a `trainer_hash_cache` SQLite table (migration 13) stores hashes with mtime-based revalidation; and `offline_readiness_snapshots` persist per-profile readiness scores that surface in the health dashboard and pre-flight launch checks. All trainer type metadata is portable (TOML profile), while activation state and readiness scores are machine-local (SQLite only), following the existing `storage_profile()`/`portable_profile()` boundary.

## Relevant Files

### Core Library

- `src/crosshook-native/crates/crosshook-core/src/profile/models.rs`: `GameProfile`, `TrainerSection` (has `kind: String` + `loading_mode`) -- extend with `trainer_type: Option<TrainerType>` field
- `src/crosshook-native/crates/crosshook-core/src/profile/toml_store.rs`: `ProfileStore` CRUD for TOML profiles -- no changes needed, but `save()` triggers downstream hash events
- `src/crosshook-native/crates/crosshook-core/src/profile/health.rs`: `check_profile_health()`, `ProfileHealthReport`, `HealthIssue` -- extend with offline readiness checks
- `src/crosshook-native/crates/crosshook-core/src/profile/legacy.rs`: Profile migration helpers (v1 format) -- pattern for handling `trainer_type` field addition to existing TOMLs
- `src/crosshook-native/crates/crosshook-core/src/launch/request.rs`: `LaunchRequest` with `trainer_path`/`trainer_host_path` -- add offline readiness pre-flight warning (non-fatal)
- `src/crosshook-native/crates/crosshook-core/src/launch/script_runner.rs`: Assembles shell commands for proton_run/steam_applaunch/native -- no changes, but depends on resolved trainer paths
- `src/crosshook-native/crates/crosshook-core/src/launch/catalog.rs`: Data-driven TOML optimization catalog with embedded default + user override -- **exact pattern** for trainer type catalog
- `src/crosshook-native/crates/crosshook-core/src/metadata/mod.rs`: `MetadataStore` facade with `try_new()`/`disabled()`/`with_conn()` -- new offline store methods go here
- `src/crosshook-native/crates/crosshook-core/src/metadata/migrations.rs`: Sequential `PRAGMA user_version` migrations (currently v12) -- add migration 13 for offline tables
- `src/crosshook-native/crates/crosshook-core/src/metadata/models.rs`: Shared enums (`TrainerLoadingMode`, `DriftState`, `LaunchOutcome`) with `as_str()` + serde -- pattern for `OfflineCapability`/`TrainerType`
- `src/crosshook-native/crates/crosshook-core/src/metadata/version_store.rs`: `hash_trainer_file()` (SHA-256), `compute_correlation_status()`, `upsert_version_snapshot()` -- reuse hash infra, `TrainerChanged` triggers `hash_stale`
- `src/crosshook-native/crates/crosshook-core/src/metadata/health_store.rs`: `health_snapshots` read/write -- pattern for `offline_readiness_snapshots` store methods
- `src/crosshook-native/crates/crosshook-core/src/metadata/cache_store.rs`: `external_cache_entries` CRUD with size-limiting -- pattern for bounded cache storage
- `src/crosshook-native/crates/crosshook-core/src/metadata/profile_sync.rs`: `sha256_hex()` utility -- reusable for content hashing
- `src/crosshook-native/crates/crosshook-core/src/settings/mod.rs`: `AppSettingsData` TOML struct -- add `offline_mode: bool` toggle
- `src/crosshook-native/crates/crosshook-core/src/community/taps.rs`: `CommunityTapStore` with git-clone/fetch, `sync_tap()` -- add `is_tap_available_offline()` method
- `src/crosshook-native/crates/crosshook-core/src/install/models.rs`: `InstallGameRequest` -- extension point for install-time trainer hash registration
- `src/crosshook-native/crates/crosshook-core/src/onboarding/readiness.rs`: `check_system_readiness()` / `evaluate_checks()` / `ReadinessCheckResult` -- closest existing pattern for offline pre-flight

### Tauri Command Layer

- `src/crosshook-native/src-tauri/src/commands/launch.rs`: `launch_game`/`launch_trainer` handlers, calls `hash_trainer_file` post-launch -- add pre-flight offline warning
- `src/crosshook-native/src-tauri/src/commands/health.rs`: `batch_validate_profiles`, `get_profile_health` -- integrate offline readiness into health reports
- `src/crosshook-native/src-tauri/src/commands/version.rs`: `check_version_status`, `set_trainer_version` -- existing version/hash commands; offline hash cache extends this
- `src/crosshook-native/src-tauri/src/commands/settings.rs`: `settings_load`/`settings_save` -- surfaces new `offline_mode` toggle
- `src/crosshook-native/src-tauri/src/commands/community.rs`: `sync_tap` -- must fail gracefully when offline
- `src/crosshook-native/src-tauri/src/commands/mod.rs`: Command module index -- register new `offline` module
- `src/crosshook-native/src-tauri/src/lib.rs`: Tauri state registration, `invoke_handler` macro -- register new offline commands

### Frontend

- `src/crosshook-native/src/components/pages/LaunchPage.tsx`: Launch UI with version-correlation display -- add pre-flight offline readiness section
- `src/crosshook-native/src/components/pages/HealthDashboardPage.tsx`: Health dashboard -- surface offline readiness scores per profile
- `src/crosshook-native/src/components/pages/ProfilesPage.tsx`: Profile management -- add trainer type badge/indicator on profile rows
- `src/crosshook-native/src/components/LaunchPanel.tsx`: Launch controls -- add `CollapsibleSection` for pre-flight validation
- `src/crosshook-native/src/components/HealthBadge.tsx`: Health score badge (`crosshook-status-chip` CSS) -- pattern for `OfflineReadinessBadge`
- `src/crosshook-native/src/hooks/useProfileHealth.ts`: `useReducer` + `invoke()` pattern -- follow for `useOfflineReadiness` hook
- `src/crosshook-native/src/hooks/useLaunchState.ts`: Launch state management -- integrate offline pre-flight check
- `src/crosshook-native/src/types/health.ts`: `HealthIssue`, `ProfileHealthReport` TypeScript types -- extend with offline readiness types
- `src/crosshook-native/src/types/profile.ts`: Profile TypeScript types -- add `TrainerType` union type
- `src/crosshook-native/src/types/index.ts`: Type re-exports -- add new offline/trainer types
- `src/crosshook-native/src/styles/variables.css`: CSS custom properties -- add offline status colors

### Assets (to create)

- `src/crosshook-native/src-tauri/assets/default_trainer_type_catalog.toml`: Embedded trainer type catalog (FLiNG, Aurora, WeMod, PLITCH, CheatEngine, etc.)

## Relevant Tables

### Existing (schema v12)

- `profiles`: `profile_id TEXT PK`, `current_filename`, `content_hash`, `deleted_at` -- FK target for offline tables; filter `deleted_at IS NOT NULL` in queries
- `version_snapshots`: `profile_id FK`, `trainer_file_hash`, `trainer_version`, `steam_build_id`, `status` -- existing hash data seeds `trainer_hash_cache` at migration time
- `health_snapshots`: `profile_id PK FK`, `status`, `issue_count`, `checked_at` -- pattern for `offline_readiness_snapshots`
- `community_taps`: `tap_id PK`, `tap_url`, `local_path`, `last_indexed_at` -- FK target for tap offline state
- `external_cache_entries`: `cache_id PK`, `cache_key UNIQUE`, `payload_json` -- alternative to dedicated tap offline table

### New (migration 13)

- `trainer_hash_cache`: `cache_id TEXT PK`, `profile_id FK`, `file_path`, `file_size INTEGER`, `file_modified_at TEXT`, `sha256_hash TEXT`, `verified_at`, `created_at`, `updated_at` -- UNIQUE INDEX on `(profile_id, file_path)`; bootstrap from `version_snapshots.trainer_file_hash`
- `offline_readiness_snapshots`: `profile_id TEXT PK FK`, `readiness_state TEXT`, `readiness_score INTEGER (0-100)`, `trainer_type TEXT`, `trainer_present INTEGER`, `trainer_hash_valid INTEGER`, `trainer_activated INTEGER`, `proton_available INTEGER`, `community_tap_cached INTEGER`, `network_required INTEGER`, `blocking_reasons TEXT (JSON)`, `checked_at TEXT`
- `community_tap_offline_state`: `tap_id TEXT PK FK`, `has_local_clone INTEGER`, `last_sync_at TEXT`, `clone_size_bytes INTEGER`

## Relevant Patterns

**Store Pattern (MetadataStore)**: All SQLite operations go through `MetadataStore::with_conn(action_label, |conn| {...})` which handles lock acquisition and availability checking. New offline store methods must follow this pattern. See `src/crosshook-native/crates/crosshook-core/src/metadata/mod.rs`.

**Fail-Soft Degradation**: `MetadataStore::disabled()` mode returns defaults when SQLite is unavailable. All offline readiness queries must check `is_available()` and return `readiness_state = "unconfigured"` when false. Pattern: `unwrap_or_default()` for non-critical metadata. See `src/crosshook-native/crates/crosshook-core/src/metadata/mod.rs`.

**Enum with as_str + serde**: All enums crossing IPC use `#[serde(rename_all = "snake_case")]`, implement `as_str() -> &'static str`, and have a `#[default]` variant. See `TrainerLoadingMode`, `DriftState`, `LaunchOutcome` in `src/crosshook-native/crates/crosshook-core/src/metadata/models.rs`.

**Data-Driven TOML Catalog**: The optimization catalog loads from embedded TOML asset with community tap overrides and user file overrides. Trainer type catalog follows identical pattern. See `src/crosshook-native/crates/crosshook-core/src/launch/catalog.rs`.

**Sequential Migration**: Each migration is a `migrate_N_to_N+1(conn)` function; the runner calls them sequentially. New tables go in migration 13. See `src/crosshook-native/crates/crosshook-core/src/metadata/migrations.rs`.

**Domain Error Enum**: Each domain has a custom error type with `Database { action: &'static str, source: SqlError }` and `Io { action, path, source }` variants. Action strings use imperative present tense. See `src/crosshook-native/crates/crosshook-core/src/metadata/models.rs`.

**IPC Command Convention**: `#[tauri::command]` functions take `State<'_, Store>` parameters, return `Result<T, String>`, and convert errors with `.map_err(|e| e.to_string())`. CPU-intensive work uses `spawn_blocking`. See `src/crosshook-native/src-tauri/src/commands/health.rs`.

**React Hook Pattern**: `useReducer` with typed `HookStatus` (idle/loading/loaded/error), `invoke()` for Tauri commands, `listen()` for push events. See `src/crosshook-native/src/hooks/useProfileHealth.ts`.

**Readiness Check Pattern**: `check_system_readiness()` returns `ReadinessCheckResult { checks: Vec<HealthIssue>, all_passed: bool }`. Offline pre-flight check should follow this structure. See `src/crosshook-native/crates/crosshook-core/src/onboarding/readiness.rs`.

**Portable vs Machine-Local Split**: `portable_profile()` strips machine-specific paths; `storage_profile()` includes them via `LocalOverrideSection`. `trainer_type` is portable (TOML); activation state and readiness scores are machine-local (SQLite). See `src/crosshook-native/crates/crosshook-core/src/profile/models.rs`.

## Relevant Docs

**docs/plans/offline-trainers/feature-spec.md**: You _must_ read this -- synthesized spec with final decisions on data model, catalog architecture, SQLite tables, API surface, and scoring weights. Supersedes individual research files where they conflict.

**docs/plans/offline-trainers/research-business.md**: You _must_ read this -- business rules (BR-1 through BR-8), offline readiness scoring weights (trainer_present=30, hash_valid=15, game_present=20, proton_available=15, prefix_exists=10, network_not_required=10), score caps per trainer type, and state machine.

**docs/plans/offline-trainers/research-technical.md**: You _must_ read this -- architecture diagram, component diagram, new modules, integration points table, state machine, portable vs machine-local data split.

**docs/plans/offline-trainers/research-practices.md**: You _must_ read this when deciding module placement -- identifies all reusable code (`hash_trainer_file`, `sha256_hex`, `TrainerLoadingMode` pattern, `check_system_readiness`), v1 minimal scope (~180 LOC), and final decisions on extending existing modules vs new `offline/` module.

**docs/plans/offline-trainers/research-security.md**: You _must_ read this -- W-1 (key storage via keyring crate), W-2 (DB permissions 0600), W-4 (untrusted trainer binaries), W-5 (activation not in TOML), A-6 (git config hardening).

**docs/plans/offline-trainers/research-recommendations.md**: Read this for implementation phasing (Phase 1: FLiNG+hash, Phase 2: launch integration, Phase 3: community+Aurora, Phase 4: UI polish), parallelization map, and risk assessment.

**docs/plans/offline-trainers/research-ux.md**: Read this for UI component designs -- `OfflineReadinessBadge`, platform-aware Aurora modal, pre-flight `CollapsibleSection`, inline hash verification UX, gamepad accessibility requirements.

**CLAUDE.md**: You _must_ read this -- authoritative codebase architecture map, module descriptions, build commands, code conventions.
