# Profile Health Dashboard (v2) Implementation Plan

The profile health dashboard adds batch filesystem-path validation of all saved CrossHook profiles with per-profile health status (healthy/stale/broken) and specific remediation suggestions. It uses a two-layer architecture: Layer 1 (`profile/health.rs` in crosshook-core) performs pure-filesystem validation with no MetadataStore dependency; Layer 2 (`commands/health.rs` in src-tauri) enriches results with launch failure trends, last-success timestamps, and launcher drift via existing MetadataStore APIs in a fail-soft composition pattern. Implementation follows five phases: S (security pre-ship), A (core filesystem MVP), B (metadata enrichment using existing queries — no new tables), C (startup background scan), and D (health snapshot persistence via migration v6). Zero new Rust dependencies are needed — approximately 80% of the validation logic already exists in `validate_all()`, `ValidationError::help()`, `ProfileStore::list()`/`load()`, and the `CompatibilityBadge` CSS pattern.

## Critically Relevant Files and Documentation

- docs/plans/profile-health-dashboard/feature-spec.md: Authoritative v2 spec — two-layer architecture, data models, business rules, phasing, security findings
- docs/plans/profile-health-dashboard/research-practices.md: 18 reusable code items with exact file:line references
- docs/plans/profile-health-dashboard/research-security.md: Security findings W-1 through N-4 — path sanitization, CSP, deleted_at filter
- src/crosshook-native/crates/crosshook-core/src/launch/request.rs: Path validation helpers to promote (`require_directory` ~line 700, `require_executable_file` ~line 721, `is_executable_file` ~line 742), `ValidationError` enum with `.issue()` and `.help()`, `ValidationSeverity`, `validate_all()`
- src/crosshook-native/crates/crosshook-core/src/profile/models.rs: `GameProfile` struct — all path fields, `resolve_launch_method()`
- src/crosshook-native/crates/crosshook-core/src/profile/toml_store.rs: `ProfileStore` — `list()`, `load()`, `with_base_path()` test pattern
- src/crosshook-native/crates/crosshook-core/src/metadata/mod.rs: `MetadataStore` API — `query_failure_trends()`, `query_last_success_per_profile()`, `lookup_profile_id()`, `with_conn()` fail-soft
- src/crosshook-native/crates/crosshook-core/src/metadata/models.rs: `DriftState`, `FailureTrendRow`, `MetadataStoreError`
- src/crosshook-native/crates/crosshook-core/src/metadata/migrations.rs: Schema v0-v5; Phase D adds v6
- src/crosshook-native/src-tauri/src/commands/profile.rs: Reference dual-store command pattern (ProfileStore + MetadataStore, fail-soft)
- src/crosshook-native/src-tauri/src/commands/shared.rs: `sanitize_display_path()` at line 20
- src/crosshook-native/src-tauri/src/lib.rs: `.manage()` lines 76-80, `invoke_handler!` line 85, startup async spawn lines 46-56
- src/crosshook-native/src/hooks/useLaunchState.ts: `useReducer` + event listener pattern — template for `useProfileHealth`
- src/crosshook-native/src/components/CompatibilityViewer.tsx: Badge CSS pattern — `crosshook-status-chip crosshook-compatibility-badge--{rating}`
- src/crosshook-native/src/components/pages/ProfilesPage.tsx: Primary integration point for health badges

## Implementation Plan

### Phase S: Security Pre-Ship

#### Task S.1: Enable CSP in Tauri config Depends on [none]

**READ THESE BEFORE TASK**

- src/crosshook-native/src-tauri/tauri.conf.json
- docs/plans/profile-health-dashboard/research-security.md (W-1 finding)

**Instructions**

Files to Modify

- src/crosshook-native/src-tauri/tauri.conf.json

Security finding W-1: `"csp": null` at line 23 disables Content Security Policy. Enable it before adding new IPC commands. Change to `"csp": "default-src 'self'; script-src 'self'"`. Test that the dev server (`./scripts/dev-native.sh`) still works — Vite dev mode may require `script-src 'self' 'unsafe-eval'` for HMR; if so, use that in development and strict CSP in the production `tauri.conf.json` or via build-time configuration.

#### Task S.2: Verify sanitize_display_path availability Depends on [none]

**READ THESE BEFORE TASK**

- src/crosshook-native/src-tauri/src/commands/shared.rs

**Instructions**

Files to Modify

- (none expected — verification task)

Confirm that `sanitize_display_path()` is `pub` at `commands/shared.rs:20`. Verify it can be imported via `use super::shared::sanitize_display_path;` from a sibling module (e.g., `commands/health.rs`). If the function is not in `shared.rs`, move it there from `commands/launch.rs`. This is a prerequisite for all health IPC commands.

### Phase A: Core Health Check (MVP)

#### Task A.1: Promote path validation helpers to pub(crate) Depends on [none]

**READ THESE BEFORE TASK**

- src/crosshook-native/crates/crosshook-core/src/launch/request.rs (lines 700-756)

**Instructions**

Files to Modify

- src/crosshook-native/crates/crosshook-core/src/launch/request.rs

Change visibility on three private functions to `pub(crate)`:

- `fn require_directory(...)` at ~line 700 → `pub(crate) fn require_directory(...)`
- `fn require_executable_file(...)` at ~line 721 → `pub(crate) fn require_executable_file(...)`
- `fn is_executable_file(...)` at ~line 742 → `pub(crate) fn is_executable_file(...)`

These are used by `validate_all()` internally and will be reused by `check_profile_health()` in `profile/health.rs`. Three call sites (validate_steam, validate_proton, health check) justifies the promotion. Run `cargo test -p crosshook-core` to verify no breakage.

#### Task A.2: Create profile/health.rs — types and core logic Depends on [A.1]

**READ THESE BEFORE TASK**

- docs/plans/profile-health-dashboard/feature-spec.md (Data Models section)
- src/crosshook-native/crates/crosshook-core/src/profile/models.rs
- src/crosshook-native/crates/crosshook-core/src/launch/request.rs (ValidationError, require_directory, require_executable_file)

**Instructions**

Files to Create

- src/crosshook-native/crates/crosshook-core/src/profile/health.rs

Create the core health module with:

**Types** (all `#[derive(Debug, Clone, Serialize, Deserialize)]` with `#[serde(rename_all = "snake_case")]`):

- `HealthStatus { Healthy, Stale, Broken }` — profile-level roll-up
- `HealthIssueSeverity { Error, Warning, Info }` — NEW enum, do NOT reuse `ValidationSeverity` (it always returns Fatal)
- `HealthIssue { field: String, path: String, message: String, remediation: String, severity: HealthIssueSeverity }` — per-field issue
- `ProfileHealthReport { name: String, status: HealthStatus, launch_method: String, issues: Vec<HealthIssue>, checked_at: String }` — per-profile result
- `HealthCheckSummary { profiles: Vec<ProfileHealthReport>, healthy_count: usize, stale_count: usize, broken_count: usize, total_count: usize, validated_at: String }` — batch result

**Functions**:

- `pub fn check_profile_health(name: &str, profile: &GameProfile) -> ProfileHealthReport` — validates path fields using `std::fs::metadata()` + promoted `require_directory()` / `require_executable_file()` from `launch/request.rs`. Method-aware: call `resolve_launch_method(profile)` first, check only fields relevant to that method. Classify: `Missing` (ENOENT) → Stale, `Inaccessible` (EACCES) / `WrongType` / `NotConfigured` (required field) → Broken. Detect "all NotConfigured" as Unconfigured variant (badge-only, no banner).
- `pub fn batch_check_health(store: &ProfileStore) -> HealthCheckSummary` — iterate `store.list()`, load each profile, call `check_profile_health()`. **Critical**: catch `ProfileStoreError` per-profile and emit as Broken entry with load-error message. Never `?`-propagate from within the per-profile loop.

Use `chrono::Utc::now().to_rfc3339()` for `checked_at` timestamps (chrono is already a dependency).

**Path field inventory** (all on `GameProfile`):

- `game.executable_path` → require file exists (always)
- `trainer.path` → require file exists if non-empty
- `injection.dll_paths` → iterate all entries, require file for each non-empty
- `steam.compatdata_path` → require directory if steam_applaunch
- `steam.proton_path` → require executable if steam_applaunch
- `steam.launcher.icon_path` → optional, Info severity if missing
- `runtime.prefix_path` → require directory if proton_run
- `runtime.proton_path` → require executable if proton_run
- `runtime.working_directory` → optional, Info severity if missing

Write inline `#[cfg(test)]` unit tests using `tempfile::tempdir()` + `ProfileStore::with_base_path()`. Test cases: healthy profile (all paths exist), missing path (Stale), wrong file type (Broken), EACCES (Broken), empty profile (Unconfigured/Broken), TOML parse error per-profile isolation.

#### Task A.3: Wire profile module Depends on [A.2]

**READ THESE BEFORE TASK**

- src/crosshook-native/crates/crosshook-core/src/profile/mod.rs

**Instructions**

Files to Modify

- src/crosshook-native/crates/crosshook-core/src/profile/mod.rs

Add `pub mod health;` to the module declarations. Add re-exports for public types: `pub use health::{HealthStatus, HealthIssueSeverity, HealthIssue, ProfileHealthReport, HealthCheckSummary};`. This is a 3-5 line change. Run `cargo test -p crosshook-core` to verify.

#### Task A.4: Create Tauri health commands Depends on [A.3, S.2]

**READ THESE BEFORE TASK**

- src/crosshook-native/src-tauri/src/commands/profile.rs (dual-store command pattern)
- src/crosshook-native/src-tauri/src/commands/shared.rs (sanitize_display_path import)
- docs/plans/profile-health-dashboard/feature-spec.md (API Design section)

**Instructions**

Files to Create

- src/crosshook-native/src-tauri/src/commands/health.rs

Files to Modify

- src/crosshook-native/src-tauri/src/commands/mod.rs
- src/crosshook-native/src-tauri/src/lib.rs

Create new `commands/health.rs` with two commands (sync `fn`, not `async fn`):

1. `batch_validate_profiles(store: State<'_, ProfileStore>) -> Result<HealthCheckSummary, String>` — calls `batch_check_health(&store)`. Apply `sanitize_display_path()` to every `HealthIssue.path` field before returning. Import via `use super::shared::sanitize_display_path;`.

2. `get_profile_health(name: String, store: State<'_, ProfileStore>) -> Result<ProfileHealthReport, String>` — single-profile check for save-triggered revalidation.

Note: Phase A commands accept only `ProfileStore`. `MetadataStore` enrichment is added in Phase B.

Register in `commands/mod.rs`: add `pub mod health;`. Register in `lib.rs`: append `commands::health::batch_validate_profiles` and `commands::health::get_profile_health` to the `invoke_handler!` macro (~line 85).

#### Task A.5: Create TypeScript health types Depends on [none]

**READ THESE BEFORE TASK**

- docs/plans/profile-health-dashboard/feature-spec.md (TypeScript Interfaces section)
- src/crosshook-native/src/types/launch.ts (type pattern reference)

**Instructions**

Files to Create

- src/crosshook-native/src/types/health.ts

Files to Modify

- src/crosshook-native/src/types/index.ts

Create TypeScript types matching the Rust structs:

- `HealthStatus = 'healthy' | 'stale' | 'broken'`
- `HealthIssueSeverity = 'error' | 'warning' | 'info'`
- `HealthIssue { field, path, message, remediation, severity }`
- `ProfileHealthReport { name, status, launch_method, issues, checked_at }`
- `HealthCheckSummary { profiles, healthy_count, stale_count, broken_count, total_count, validated_at }`

Also define Phase B stubs (all fields nullable/optional) so Phase B frontend tasks don't need a types update:

- `ProfileHealthMetadata { profile_id, last_success, failure_count_30d, total_launches, launcher_drift_state, is_community_import }`
- `EnrichedProfileHealthReport extends ProfileHealthReport { metadata: ProfileHealthMetadata | null }`

Add `export * from './health';` to `types/index.ts`.

#### Task A.6: Create HealthBadge component Depends on [A.5]

**READ THESE BEFORE TASK**

- src/crosshook-native/src/components/CompatibilityViewer.tsx (badge CSS pattern, ~line 76)
- src/crosshook-native/src/styles/variables.css (color tokens)

**Instructions**

Files to Create

- src/crosshook-native/src/components/HealthBadge.tsx

Create a presentational badge component using the existing `crosshook-status-chip` CSS pattern. Map `healthy→working`, `stale→partial`, `broken→broken` to the existing `crosshook-compatibility-badge--{rating}` modifier classes. Ensure minimum touch target of 48px (`--crosshook-touch-target-min`). Color + icon + text label on every badge (never rely on color alone per accessibility requirements).

#### Task A.7: Create useProfileHealth hook Depends on [A.4, A.5]

**READ THESE BEFORE TASK**

- src/crosshook-native/src/hooks/useLaunchState.ts (useReducer + event listener pattern)
- src/crosshook-native/src/hooks/useProfile.ts (invoke pattern)

**Instructions**

Files to Create

- src/crosshook-native/src/hooks/useProfileHealth.ts

Create a `useReducer`-based hook following the `useLaunchState` pattern:

**State**: `{ status: 'idle' | 'loading' | 'loaded' | 'error', summary: HealthCheckSummary | null, error: string | null }`

**Actions**: `batch-loading`, `batch-complete`, `single-loading`, `single-complete`, `error`, `reset`

**Exposed API**:

- `batchValidate()` — calls `invoke<HealthCheckSummary>('batch_validate_profiles')`
- `revalidateSingle(name: string)` — calls `invoke<ProfileHealthReport>('get_profile_health', { name })`
- `healthByName` — derived `Record<string, ProfileHealthReport>` for O(1) lookup
- `summary`, `loading`, `error` state accessors

Call `batchValidate()` on initial mount via `useEffect`. Include `active` flag + cleanup pattern for future event listener (Phase C).

**Merge behavior**: On `single-complete` action, replace the matching entry by `name` in `summary.profiles` array and recompute `healthy_count`/`stale_count`/`broken_count`. Do not push a duplicate — find and replace in-place.

#### Task A.8: Integrate health badges into ProfilesPage Depends on [A.6, A.7]

**READ THESE BEFORE TASK**

- src/crosshook-native/src/components/pages/ProfilesPage.tsx
- src/crosshook-native/src/components/CompatibilityViewer.tsx (CollapsibleSection pattern)

**Instructions**

Files to Modify

- src/crosshook-native/src/components/pages/ProfilesPage.tsx

Integrate health into the profile list page:

1. Import and call `useProfileHealth()` at page level
2. Render `HealthBadge` adjacent to each profile name in the sidebar list. Do NOT modify `ProfileFormSections.tsx` — badges sit in the `ProfilesPage` sidebar.
3. Add aggregate summary chip: "3 of 12 profiles have issues" at the top of the profile list
4. Add "Re-check All" button calling `batchValidate()`
5. Add per-issue detail via `CollapsibleSection` — when a broken/stale profile is selected, expand to show `HealthIssue` list with field labels, messages, and remediation help text
6. Wire save-triggered revalidation: after `save_profile` succeeds, call `revalidateSingle(name)` to update that profile's badge in-place
7. Ensure gamepad navigation: `useGamepadNav` two-zone model, health detail as content zone, "Y Re-check" / "A Open" controller hints

### Phase B: Metadata Enrichment

#### Task B.1: Add MetadataStore enrichment to health commands Depends on [A.4]

**READ THESE BEFORE TASK**

- src/crosshook-native/src-tauri/src/commands/health.rs (Phase A version)
- src/crosshook-native/crates/crosshook-core/src/metadata/mod.rs (query_failure_trends, query_last_success_per_profile, lookup_profile_id)
- src/crosshook-native/crates/crosshook-core/src/metadata/models.rs (FailureTrendRow, DriftState)

**Instructions**

Files to Modify

- src/crosshook-native/src-tauri/src/commands/health.rs

Add `State<'_, MetadataStore>` parameter to both health commands. Add enrichment types:

- `ProfileHealthMetadata { profile_id, last_success, failure_count_30d, total_launches, launcher_drift_state, is_community_import }`
- `EnrichedProfileHealthReport { #[serde(flatten)] core: ProfileHealthReport, metadata: Option<ProfileHealthMetadata> }`
- `EnrichedHealthSummary` (same structure as HealthCheckSummary but with enriched profiles)

Before the per-profile loop, batch-fetch:

- `metadata_store.query_failure_trends(30).unwrap_or_default()` — index by profile_name into HashMap
- `metadata_store.query_last_success_per_profile().unwrap_or_default()` — index by profile_name

Per-profile enrichment:

- `metadata_store.lookup_profile_id(name)` for stable UUID
- Map failure trends (Degraded if failures >= 2 and successes == 0)
- Map last-success timestamp
- Query launcher drift via `metadata_store.with_conn()` inline SQL: `SELECT drift_state FROM launchers WHERE profile_id = ?1 AND drift_state != 'missing' ORDER BY updated_at DESC LIMIT 1` (see `shared.md` > Relevant Tables > launchers for schema)
- Check community import from `profiles.source` field

If MetadataStore is unavailable, `with_conn()` returns defaults — enrichment fields are `None`/0. Apply `sanitize_display_path()` to any SQLite-sourced paths (security N-1, N-3).

**Known limitation**: `query_failure_trends()` groups by `profile_name` not `profile_id` — renamed profiles lose historical data. Acceptable for v1.

#### Task B.2: Add failure trend badge overlay Depends on [A.8]

**READ THESE BEFORE TASK**

- src/crosshook-native/src/components/HealthBadge.tsx

**Instructions**

Files to Modify

- src/crosshook-native/src/components/HealthBadge.tsx

Add a failure count indicator overlay when `metadata !== null` and `failure_count_30d >= 2`. Display as `↑Nx` badge annotation. Show only when metadata is available.

#### Task B.3: Add last-success label and launcher drift indicator Depends on [A.8]

**READ THESE BEFORE TASK**

- src/crosshook-native/src/components/pages/ProfilesPage.tsx (CollapsibleSection detail panel)

**Instructions**

Files to Modify

- src/crosshook-native/src/components/pages/ProfilesPage.tsx

In the health detail `CollapsibleSection`:

1. Add "Last worked: N days ago" relative timestamp when `metadata.last_success` is non-null
2. Add "Launched N times • M failures in last 30 days" line when `total_launches > 0`
3. Add launcher drift warning when `launcher_drift_state` is `missing`, `moved`, or `stale`
4. Add community import context note when `is_community_import` is true and status is broken/stale

All enrichment displays conditional on `metadata !== null`.

#### Task B.4: Add collection/favorites filter Depends on [A.8]

**READ THESE BEFORE TASK**

- src/crosshook-native/src/components/pages/ProfilesPage.tsx

**Instructions**

Files to Modify

- src/crosshook-native/src/components/pages/ProfilesPage.tsx

Add filter controls: "Show favorites only" toggle, collection dropdown. Filter `healthByName` client-side. Update summary count for filtered subset. Hide/disable filter controls when MetadataStore unavailable.

### Phase C: Startup Integration

#### Task C.1: Add background startup health scan Depends on [A.4]

**READ THESE BEFORE TASK**

- src/crosshook-native/src-tauri/src/lib.rs (lines 46-72 — existing async spawn + emit pattern)

**Instructions**

Files to Modify

- src/crosshook-native/src-tauri/src/lib.rs

In the `setup` closure, after the existing `auto-load-profile` spawn, add a second `tauri::async_runtime::spawn` with 500ms delay that calls `batch_check_health()` and emits `"profile-health-batch-complete"` via `app_handle.emit()`. Do NOT modify `startup.rs`.

#### Task C.2: Listen for startup event in hook Depends on [A.7]

**READ THESE BEFORE TASK**

- src/crosshook-native/src/hooks/useLaunchState.ts (lines 157-186 — event listener pattern)

**Instructions**

Files to Modify

- src/crosshook-native/src/hooks/useProfileHealth.ts

Add `listen("profile-health-batch-complete")` in the hook's `useEffect`. Follow the `useLaunchState` pattern: `active` flag + `unlisten()` cleanup. On event receipt, dispatch `batch-complete` action.

#### Task C.3: Add startup summary banner Depends on [C.2, A.8]

**READ THESE BEFORE TASK**

- src/crosshook-native/src/components/pages/ProfilesPage.tsx (lines ~461-484 — existing `crosshook-rename-toast` dismissible notification pattern)

**Instructions**

Files to Modify

- src/crosshook-native/src/components/pages/ProfilesPage.tsx

Add dismissible non-modal banner when `broken_count > 0` after batch results arrive. Replicate the existing `crosshook-rename-toast` pattern already in this file (lines ~461-484) — NOT `PageBanner.tsx` which is a static decorative header, not a notification. Use `role="status"` + `aria-live="polite"`. Per-session dismiss only (useState boolean, reset on next app launch). Stale/Degraded/Unconfigured profiles: badge only, no banner.

### Phase D: Persistence + Trends

#### Task D.1: Add health_snapshots migration v6 Depends on [none]

**READ THESE BEFORE TASK**

- src/crosshook-native/crates/crosshook-core/src/metadata/migrations.rs (existing v0-v5 pattern)

**Instructions**

Files to Modify

- src/crosshook-native/crates/crosshook-core/src/metadata/migrations.rs

Add `migrate_5_to_6()`: `CREATE TABLE IF NOT EXISTS health_snapshots (profile_id TEXT PK REFERENCES profiles, status TEXT NOT NULL, issue_count INTEGER NOT NULL DEFAULT 0, checked_at TEXT NOT NULL)` + index. Add version check block in `run_migrations()`.

#### Task D.2: Create metadata/health_store.rs Depends on [D.1]

**READ THESE BEFORE TASK**

- src/crosshook-native/crates/crosshook-core/src/metadata/launch_history.rs (module pattern)

**Instructions**

Files to Create

- src/crosshook-native/crates/crosshook-core/src/metadata/health_store.rs

Implement `upsert_health_snapshot()`, `load_health_snapshots()` (JOIN profiles WHERE deleted_at IS NULL), `lookup_health_snapshot()`. All use `params![]`, return `Result<T, MetadataStoreError>`.

#### Task D.3: Wire MetadataStore public API Depends on [D.2]

**READ THESE BEFORE TASK**

- src/crosshook-native/crates/crosshook-core/src/metadata/mod.rs (delegation pattern)

**Instructions**

Files to Modify

- src/crosshook-native/crates/crosshook-core/src/metadata/mod.rs

Add `mod health_store;` + public delegation methods wrapping via `self.with_conn()`.

#### Task D.4: Persist results after batch validation Depends on [D.3, B.1]

**READ THESE BEFORE TASK**

- src/crosshook-native/src-tauri/src/commands/health.rs

**Instructions**

Files to Modify

- src/crosshook-native/src-tauri/src/commands/health.rs

After batch validation and enrichment, call `upsert_health_snapshot()` for each profile. Fail-soft — persistence failure logged as warning, does not affect result.

#### Task D.5: Load cached snapshots at startup Depends on [D.4]

**READ THESE BEFORE TASK**

- src/crosshook-native/src-tauri/src/commands/health.rs

**Instructions**

Files to Modify

- src/crosshook-native/src-tauri/src/commands/health.rs
- src/crosshook-native/src-tauri/src/lib.rs

Add new Tauri command `get_cached_health_snapshots` calling `metadata_store.load_health_snapshots()`. Register in `invoke_handler!`. Frontend calls on mount for instant badge display before live scan.

#### Task D.6: Add trend arrows in UI Depends on [D.5]

**READ THESE BEFORE TASK**

- src/crosshook-native/src/components/HealthBadge.tsx

**Instructions**

Files to Modify

- src/crosshook-native/src/components/HealthBadge.tsx
- src/crosshook-native/src/hooks/useProfileHealth.ts

Compare current status to cached snapshot: `got_worse` (↓), `got_better` (↑), `unchanged` (no arrow). Show only when both current and cached exist.

#### Task D.7: Add stale-snapshot detection Depends on [D.5]

**READ THESE BEFORE TASK**

- src/crosshook-native/src/hooks/useProfileHealth.ts

**Instructions**

Files to Modify

- src/crosshook-native/src/hooks/useProfileHealth.ts

If cached `checked_at` >7 days old, show "Last checked N days ago" note in detail panel, prompting re-check.

## Advice

- **`ValidationError::severity()` always returns `Fatal`** (confirmed at request.rs:430). Do NOT reuse `ValidationSeverity` for health issue severity. Create `HealthIssueSeverity { Error, Warning, Info }` derived from `HealthIssueKind` directly.
- **`query_failure_trends()` only returns rows where failures > 0**. Profiles absent from the result have zero failures — treat absence as healthy, not as "unknown." Build a `HashMap<String, FailureTrendRow>` from the batch result for O(1) lookup per profile.
- **`query_failure_trends()` groups by `profile_name`, not `profile_id`**. After a profile rename, old launch history rows carry the old name and won't appear in enrichment. Acceptable for v1 — a `profile_id`-based query can be added later.
- **CSP may need `'unsafe-eval'` for Vite dev mode HMR**. Test the dev server after enabling CSP. Use strict CSP in production AppImage.
- **`injection.dll_paths` is a `Vec<String>`**. Must iterate all entries — do not check only index 0.
- **Do NOT modify `ProfileFormSections.tsx`** (25k+ component). Health badges go in `ProfilesPage.tsx` sidebar adjacent to profile names, outside the form boundary.
- **Do NOT modify `startup.rs`**. Health checks must never enter the synchronous startup path. Phase C uses `tauri::async_runtime::spawn` from `lib.rs`.
- **Phase A health commands accept only `ProfileStore`**. `MetadataStore` parameter is added in Phase B Task B.1. This keeps Phase A zero-MetadataStore as designed.
- **All path strings in IPC responses must pass through `sanitize_display_path()`**. This covers both TOML-sourced and SQLite-sourced paths. Apply at struct-assembly time to cover both IPC and Phase D persistence (security N-3).
- **Health queries joining `profiles` table must filter `deleted_at IS NULL`** (security N-4). Build profile list from `ProfileStore::list()` (TOML-authoritative); use SQLite only for enrichment keyed to those names.
- **The `CompatibilityBadge` CSS reuse works directly** — map `healthy→working`, `stale→partial`, `broken→broken` to existing modifier classes. No new CSS architecture needed.
- **Test both metadata-present and metadata-absent paths**. `batch_check_health(&store)` without metadata must succeed. Write explicit tests for degraded mode.
- **Batch MetadataStore queries before the per-profile loop** (Phase B). Call `query_failure_trends(30)` and `query_last_success_per_profile()` once each, index into HashMaps. O(1) queries regardless of profile count.
- **Phase A critical path**: S.2 → A.4 → A.7 → A.8. All other Phase A tasks parallelize around this chain.
- **Phases B and C are independent of each other** — both depend only on Phase A. Can run in parallel.
- **Phase D depends on Phase B** (metadata queries must work before persistence is meaningful). Phase D is independent of Phase C.
