# Profile Health Dashboard (v2)

The health dashboard uses a two-layer architecture: Layer 1 is `profile/health.rs` in `crosshook-core` — a pure-filesystem module that validates `GameProfile` path fields via `std::fs::metadata()` with no MetadataStore dependency, testable with `ProfileStore::with_base_path()` + `tempdir`. Layer 2 is `commands/health.rs` in `src-tauri` — an orchestration layer accepting both `State<'_, ProfileStore>` and `State<'_, MetadataStore>`, calling Layer 1 for filesystem checks then enriching results with failure trends, last-success timestamps, and launcher drift when MetadataStore is available. The fail-soft pattern (`with_conn()` returns `T::default()` when unavailable) means all metadata enrichment is additive and non-blocking. Implementation follows four phases: A (core filesystem, no MetadataStore code), B (metadata enrichment via existing queries), C (startup integration), D (health persistence via migration v6).

## Relevant Files

- src/crosshook-native/crates/crosshook-core/src/profile/mod.rs: Profile module root — add `pub mod health;` here
- src/crosshook-native/crates/crosshook-core/src/profile/models.rs: `GameProfile` struct with all path fields (GameSection, TrainerSection, SteamSection, RuntimeSection, InjectionSection, LaunchSection)
- src/crosshook-native/crates/crosshook-core/src/profile/toml_store.rs: `ProfileStore` — `list()`, `load()`, `with_base_path()` for test fixtures; `base_path` is pub
- src/crosshook-native/crates/crosshook-core/src/launch/request.rs: `validate_all()`, private helpers `require_directory()`, `require_executable_file()`, `is_executable_file()` to promote to `pub(crate)`, `ValidationError` enum with `help()` remediation text, `ValidationSeverity`, `LaunchValidationIssue`
- src/crosshook-native/crates/crosshook-core/src/metadata/mod.rs: `MetadataStore` API — `query_failure_trends(days)`, `query_last_success_per_profile()`, `lookup_profile_id(name)`, `with_conn()` fail-soft wrapper
- src/crosshook-native/crates/crosshook-core/src/metadata/models.rs: `DriftState`, `FailureTrendRow`, `LaunchOutcome`, `MetadataStoreError`, `SyncSource`
- src/crosshook-native/crates/crosshook-core/src/metadata/migrations.rs: Sequential migrations v0-v5; Phase D adds v6 `health_snapshots`
- src/crosshook-native/crates/crosshook-core/src/metadata/launcher_sync.rs: Launcher drift management — `observe_launcher_exported/deleted/renamed`, manages `drift_state` column
- src/crosshook-native/crates/crosshook-core/src/metadata/launch_history.rs: `record_launch_started/finished`, `LaunchOutcome` classification
- src/crosshook-native/crates/crosshook-core/src/lib.rs: Module root — declares `pub mod profile`, `pub mod metadata`
- src/crosshook-native/src-tauri/src/commands/profile.rs: **Reference implementation** for dual-store command pattern (ProfileStore + MetadataStore, fail-soft metadata calls)
- src/crosshook-native/src-tauri/src/commands/launch.rs: Async command pattern, AppHandle event emission, `sanitize_diagnostic_report()`
- src/crosshook-native/src-tauri/src/commands/shared.rs: `sanitize_display_path()` at line 20 — import via `use super::shared::sanitize_display_path;`
- src/crosshook-native/src-tauri/src/commands/mod.rs: Module registry — add `pub mod health;` here
- src/crosshook-native/src-tauri/src/lib.rs: Tauri setup — `.manage()` calls at lines 76-80, `invoke_handler!` at line 85, startup spawn at line 61
- src/crosshook-native/src-tauri/src/startup.rs: Startup hooks — `run_metadata_reconciliation()` + async event emit pattern
- src/crosshook-native/src/App.tsx: App shell with route tabs
- src/crosshook-native/src/components/pages/ProfilesPage.tsx: Profile list page — health badges integrate here
- src/crosshook-native/src/components/layout/ContentArea.tsx: Route-to-page mapping with exhaustive switch guard
- src/crosshook-native/src/components/layout/PageBanner.tsx: Existing banner component for startup notifications
- src/crosshook-native/src/components/CompatibilityViewer.tsx: Badge CSS pattern — `crosshook-status-chip crosshook-compatibility-badge--{rating}`
- src/crosshook-native/src/hooks/useProfile.ts: Profile CRUD hook pattern (useState + useCallback + useEffect)
- src/crosshook-native/src/hooks/useLaunchState.ts: Event listener + useReducer pattern — model for `useProfileHealth`
- src/crosshook-native/src/types/launch.ts: TypeScript type definitions — pattern for health types
- src/crosshook-native/src/styles/variables.css: CSS custom properties including `--crosshook-touch-target-min: 48px`

## Relevant Tables

- profiles: Stable UUID identity (`profile_id`), `current_filename`, `game_name`, `launch_method`, `is_favorite`, `source`, `deleted_at` (soft-delete tombstone)
- launch_operations: Launch history — `status` (started/succeeded/failed/abandoned), `failure_mode`, `severity`, `diagnostic_json` (4KB cap), indexed on `profile_id` and `started_at`
- launchers: Exported launcher tracking — `profile_id` FK, `drift_state` (unknown/aligned/missing/moved/stale), tombstoned rows not deleted
- profile_name_history: Rename audit trail — `old_name`, `new_name`, `source`
- collections / collection_profiles: User-defined profile groups — available for filtered health views
- [Phase D] health_snapshots: Advisory cache of last-computed health status per profile, one row per profile (UPSERT), FK cascade on deletion

## Relevant Patterns

**MetadataStore Fail-Soft**: All metadata queries go through `with_conn()` which returns `T::default()` when `available == false` or connection is poisoned. Health commands use `unwrap_or_default()` on enrichment results — no explicit availability checks needed. See [src/crosshook-native/crates/crosshook-core/src/metadata/mod.rs](src/crosshook-native/crates/crosshook-core/src/metadata/mod.rs) for the pattern.

**Dual-Store Tauri Command**: Commands accept `State<'_, ProfileStore>` + `State<'_, MetadataStore>`. Canonical operation succeeds via ProfileStore; metadata sync is best-effort with `tracing::warn!` on failure. See [src/crosshook-native/src-tauri/src/commands/profile.rs](src/crosshook-native/src-tauri/src/commands/profile.rs) for `profile_save`/`profile_delete`/`profile_rename`.

**Batch Error Isolation**: When iterating profiles, catch per-item errors and include a sentinel Broken entry rather than aborting the batch. Already implemented in `launch/request.rs::validate_all()`.

**Path Validation Helpers**: `require_directory()`, `require_executable_file()`, `is_executable_file()` are private in `request.rs` — promote to `pub(crate)` for reuse by `health.rs`. Three call sites justify the promotion.

**ValidationError → LaunchValidationIssue**: Each `ValidationError` variant has `.issue()` returning a `LaunchValidationIssue`. Health checks construct the same `ValidationError` variants and call `.issue()` for remediation text — no new message strings.

**React useReducer + Event Listener**: `useLaunchState` uses `useReducer` for atomic state transitions plus `listen()` from `@tauri-apps/api/event`. The `active` flag + `unlisten()` cleanup pattern is at [src/crosshook-native/src/hooks/useLaunchState.ts](src/crosshook-native/src/hooks/useLaunchState.ts).

**CSS Badge/Chip**: `crosshook-status-chip crosshook-compatibility-badge--{rating}` maps health status. Map `healthy→working`, `stale→partial`, `broken→broken`. See [src/crosshook-native/src/components/CompatibilityViewer.tsx](src/crosshook-native/src/components/CompatibilityViewer.tsx).

**Test Fixture**: `tempfile::tempdir()` + `ProfileStore::with_base_path()` for filesystem tests. `MetadataStore::open_in_memory()` for SQLite tests — runs all migrations automatically. Seed via `record_launch_started`/`record_launch_finished`.

## Relevant Docs

**docs/plans/profile-health-dashboard/feature-spec.md**: You _must_ read this — the v2 feature spec with two-layer architecture, data models, business rules, phasing (A/B/C/D), security findings, and all decisions.

**docs/plans/profile-health-dashboard/research-practices.md**: You _must_ read this when implementing — 18 reusable code items with exact file:line references, KISS assessment, testability patterns.

**docs/plans/profile-health-dashboard/research-security.md**: You _must_ read this before writing IPC commands — 3 warnings (CSP, path sanitization, diagnostic bundle) + 4 new SQLite findings (N-1 through N-4).

**docs/plans/profile-health-dashboard/research-technical.md**: You _must_ read this for data models — complete Rust struct and TypeScript interface definitions, health_snapshots migration v6 schema.

**docs/plans/sqlite3-addition/feature-spec.md**: Reference for MetadataStore security model (W1-W8 findings, data sensitivity classifications).

**CLAUDE.md**: Project conventions — commit messages, code quality standards, Rust/TypeScript naming conventions.
