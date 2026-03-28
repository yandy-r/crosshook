# Context Analysis: profile-health-dashboard (v2)

**Revision**: v2.2 — exact signatures added from code-analyzer second pass (analysis-code.md).
**Based on**: feature-spec.md (v2), research-practices.md (second pass), research-security.md (second pass), research-recommendations.md (v2), shared.md, analysis-code.md.

---

## Executive Summary

Profile Health Dashboard (GitHub #38) adds batch filesystem-path validation surfacing per-profile health status (healthy/stale/broken) inline on the profile list. ~80% of logic already exists. Two-layer architecture: **Layer 1** (`profile/health.rs`) is pure-filesystem, no MetadataStore dependency; **Layer 2** (`commands/health.rs`) enriches results with launch failure trends, last-success timestamps, and launcher drift via existing `MetadataStore` APIs in a fail-soft composition pattern. Four delivery phases: A (core filesystem, zero MetadataStore code), B (metadata enrichment, no new SQL/tables), C (startup background scan), D (health snapshot persistence, migration v6). Zero new Rust dependencies. Both stores are already managed in `lib.rs` — no new `.manage()` calls needed.

---

## Architecture Context

- **Two-layer split**: `crosshook-core/src/profile/health.rs` (Layer 1, pure-filesystem) never imports `MetadataStore`. `src-tauri/src/commands/health.rs` (Layer 2) accepts `State<ProfileStore>` + `State<MetadataStore>`, calls Layer 1, then enriches via `with_conn()` fail-soft pattern.
- **Fail-soft pattern**: `with_conn<F, T>(&self, action: &'static str, f: F) -> Result<T, MetadataStoreError> where T: Default` — returns `T::default()` when unavailable or mutex poisoned. `recent_failures: Option<u32>` and `last_launch_at: Option<String>` are `None` when metadata absent — not zero, not error.
- **Data flow**: `ProfileStore::list()` → per-profile `ProfileStore::load()` → `check_profile_health(name, &profile, metadata)` → `ProfileHealthReport` → [Phase B: batch-enrich via `query_failure_trends(30)` + `query_last_success_per_profile()` HashMaps + drift query] → Tauri IPC (paths sanitized via `sanitize_display_path()`) → `useProfileHealth` reducer → `HealthBadge` render.
- **Profile list is TOML-authoritative**: Health commands discover profiles via `ProfileStore::list()` only. SQLite used exclusively for enrichment keyed to those names — health must never independently discover profiles from SQLite (guards N-4: ghost deleted profiles).
- **Startup scan (Phase C)**: Mirrors exact `lib.rs` lines 61–71 pattern: `tauri::async_runtime::spawn(async move { sleep(Duration::from_millis(500)).await; app_handle.emit("profile-health-batch-complete", &payload) })`. Use 500ms (not 350ms) so UI renders before health scan starts. Frontend also calls `invoke('batch_validate_profiles')` on mount to handle event-before-listener race.
- **Phase D option**: If `health_snapshots` (migration v6) ships before Phase C, startup can show cached badges instantly then refresh async. Otherwise, badges are empty until live scan completes.
- **`query_failure_trends` behavior**: SQL uses `HAVING failures > 0` — only returns rows for profiles with at least one failure. Profiles absent from the result have zero failures, not unknown status.

---

## Critical Files Reference

**Rust backend (crosshook-core):**

- `crates/crosshook-core/src/profile/mod.rs` — add `pub mod health;` (one line)
- `crates/crosshook-core/src/profile/models.rs` — `GameProfile` with all path fields: `game.executable_path`, `trainer.path`, `steam.proton_path`, `steam.compatdata_path`, `runtime.prefix_path`, `runtime.proton_path`, `injection.dll_paths`, launcher `icon_path`
- `crates/crosshook-core/src/profile/toml_store.rs` — `list()`, `load()`, `with_base_path()` (test harness); `base_path` is pub
- `crates/crosshook-core/src/launch/request.rs` — three helpers at lines 700–756, currently bare `fn`, add `pub(crate)` prefix only (no logic changes):
  ```rust
  pub(crate) fn require_directory<'a>(
      value: &'a str,
      required_error: ValidationError,
      missing_error: ValidationError,
      not_directory_error: ValidationError,
  ) -> Result<&'a Path, ValidationError>

  pub(crate) fn require_executable_file(
      value: &str,
      required_error: ValidationError,
      missing_error: ValidationError,
      not_executable_error: ValidationError,
  ) -> Result<(), ValidationError>

  pub(crate) fn is_executable_file(path: &Path) -> bool
  // impl: metadata.is_file() && mode & 0o111 != 0
  ```
  Also: `ValidationError::severity()` at line ~430 — unconditional `ValidationSeverity::Fatal` return for all variants.
- `crates/crosshook-core/src/metadata/mod.rs` — key signatures (lines confirmed):
  ```rust
  pub fn open_in_memory() -> Result<Self, MetadataStoreError>  // line 44, runs all migrations

  fn with_conn<F, T>(&self, action: &'static str, f: F) -> Result<T, MetadataStoreError>
  where F: FnOnce(&Connection) -> Result<T, MetadataStoreError>, T: Default
  // lines 67–84; returns T::default() when available==false or connection poisoned

  pub fn query_last_success_per_profile(&self)
      -> Result<Vec<(String, String)>, MetadataStoreError>
  // line 401; (profile_name, finished_at ISO string) pairs

  pub fn query_failure_trends(&self, days: u32)
      -> Result<Vec<FailureTrendRow>, MetadataStoreError>
  // line 437; HAVING failures > 0 — absent profiles have zero failures
  ```
- `crates/crosshook-core/src/metadata/models.rs` — `DriftState` enum (Unknown/Aligned/Missing/Moved/Stale); exact `FailureTrendRow`:
  ```rust
  pub struct FailureTrendRow {
      pub profile_name: String,
      pub successes: i64,
      pub failures: i64,
      pub failure_modes: Option<String>,  // GROUP_CONCAT, comma-separated or NULL
  }
  ```
- `crates/crosshook-core/src/metadata/migrations.rs` — Phase D adds migration v6 `health_snapshots` table here

**Tauri layer (src-tauri):**

- `src-tauri/src/commands/profile.rs` — dual-store command reference; `map_error` helper at line 13: `fn map_error(error: ProfileStoreError) -> String { error.to_string() }`; metadata failures use `tracing::warn!(%e, ...)`, never propagated; `derive_steam_client_install_path()` at line 17 is launcher-specific — **not relevant to health commands**
- `src-tauri/src/commands/shared.rs` — exact `sanitize_display_path()` at line 20:
  ```rust
  pub fn sanitize_display_path(path: &str) -> String {
      match env::var("HOME") {
          Ok(home) => match Path::new(path).strip_prefix(Path::new(&home)) {
              Ok(suffix) if suffix.as_os_str().is_empty() => "~/".to_string(),
              Ok(suffix) => format!("~/{}", suffix.display()),
              Err(_) => Path::new(path).display().to_string(),
          },
          _ => path.to_string(),
      }
  }
  ```
  Uses `Path::strip_prefix` (not string slicing) — handles trailing slash for home dir itself correctly.
- `src-tauri/src/commands/launch.rs` — `sanitize_diagnostic_report()` pattern for free-text fields from `diagnostic_json`
- `src-tauri/src/commands/mod.rs` — add `pub mod health;`
- `src-tauri/src/lib.rs` — `.manage()` calls at lines 76–80 (all five stores including `metadata_store`); `invoke_handler!` at line 85; append new health commands after "Phase 3: Catalog and Intelligence" comment block (~line 128); startup async task pattern at lines 61–71
- `src-tauri/src/startup.rs` — **do not modify**; health check must NOT enter the synchronous startup path

**React frontend:**

- `src/hooks/useLaunchState.ts` — exact `listen()` cleanup pattern:
  ```typescript
  let active = true;
  const unlistenFoo = listen<T>("event-name", (event) => {
      if (!active) return;
      dispatch({ type: "...", payload: event.payload });
  });
  return () => {
      active = false;
      void unlistenFoo.then((unlisten) => unlisten());  // Promise<UnlistenFn>
  };
  ```
- `src/components/CompatibilityViewer.tsx` — **three-class** badge at line 76:
  ```tsx
  <span className={`crosshook-status-chip crosshook-compatibility-badge crosshook-compatibility-badge--${rating}`}>
  ```
  `HealthBadge` follows the same three-class pattern: `crosshook-status-chip crosshook-health-badge crosshook-health-badge--{status}`. Map: `healthy→working`, `stale→partial`, `broken→broken`.
- `src/components/pages/ProfilesPage.tsx` — primary integration point; `HealthBadge` sits inline adjacent to profile names in the **sidebar**, not through `ProfileFormSections.tsx`
- `src/components/ProfileFormSections.tsx` — **do not modify** (25k component; health badges are outside this boundary)
- `src/types/launch.ts` — `LaunchValidationSeverity`, `LaunchFeedback` discriminated union pattern for TypeScript type design
- `src/styles/variables.css` — `--crosshook-color-success/warning/danger`, `--crosshook-touch-target-min: 48px`; do not add new color tokens

---

## Patterns to Follow

- **Dual-store Tauri command**: `State<'_, ProfileStore>` + `State<'_, MetadataStore>` — canonical op via ProfileStore; metadata failures use `tracing::warn!(%e, ...)` and return `Ok(())`, never propagated. Use `map_error` helper from `profile.rs:13` for ProfileStore errors. No new `.manage()` calls needed.
- **Fail-soft MetadataStore**: Call enrichment queries via `.unwrap_or_default()` — `with_conn()` returns `T::default()` when unavailable. No explicit availability check needed.
- **Batch enrichment as HashMap**: Call `query_failure_trends(30)` and `query_last_success_per_profile()` once each before the per-profile loop; build `HashMap<String, _>` from results. Look up per-profile; absent key = zero failures / no last-success. Never call these queries inside the loop.
- **Batch error isolation**: Catch `ProfileStoreError` per-profile in batch loop; emit as `Broken` entry. Never `?`-propagate from within the per-profile iteration.
- **Path helper promotion**: Add `pub(crate)` prefix to `require_directory`, `require_executable_file`, `is_executable_file` at lines 700–742 in `request.rs`. Zero logic changes.
- **Use `std::fs::metadata()` not `path.exists()`**: Distinguishes `NotFound` (→ Stale) from `PermissionDenied` (→ Broken). Required by business rule + security advisory A-1.
- **Real-FS testing**: `tempfile::tempdir()` + `ProfileStore::with_base_path()`. For metadata tests: `MetadataStore::open_in_memory()`. Seed via `record_launch_started`/`record_launch_finished` — never raw SQL.
- **CSS badge — three classes**: `crosshook-status-chip crosshook-health-badge crosshook-health-badge--{status}`. No utility function at 1–2 call sites.
- **Async hook `listen()` cleanup**: `active` flag + `void promise.then((unlisten) => unlisten())` — copy exact pattern from `useLaunchState.ts`.
- **New command registration**: Append to `invoke_handler!` at `lib.rs:85` after the "Phase 3" comment block (~line 128).

---

## Data Models (Locked)

**Layer 1 — `profile/health.rs`:**

```rust
pub enum HealthStatus { Healthy, Stale, Broken }

// NEW enum — do NOT reuse ValidationSeverity.
// ValidationError::severity() unconditionally returns Fatal (request.rs:430).
// Derive HealthIssueSeverity from HealthIssueKind:
//   required field empty → Error; optional path missing → Warning; skipped optional → Info
pub enum HealthIssueSeverity { Error, Warning, Info }

pub struct HealthIssue {
    pub field: String,        // e.g. "game.executable_path"
    pub path: String,         // sanitized via sanitize_display_path()
    pub message: String,
    pub remediation: String,
    pub severity: HealthIssueSeverity,
}
pub struct ProfileHealthReport { name, status, launch_method, issues: Vec<HealthIssue>, checked_at: String }
pub struct HealthCheckSummary { profiles, healthy_count, stale_count, broken_count, total_count, validated_at }

pub fn check_profile_health(name: &str, profile: &GameProfile, metadata: Option<&MetadataStore>) -> ProfileHealthReport
pub fn batch_check_health(store: &ProfileStore, metadata: Option<&MetadataStore>) -> Vec<ProfileHealthReport>
```

**Layer 2 — `commands/health.rs` (metadata enrichment):**

```rust
pub struct ProfileHealthMetadata {
    pub profile_id: Option<String>,
    pub last_success: Option<String>,          // ISO 8601 from query_last_success_per_profile
    pub failure_count_30d: i64,                // maps to FailureTrendRow.failures
    pub total_launches: i64,                   // FailureTrendRow.successes + .failures
    pub launcher_drift_state: Option<String>,  // DriftState serialized
    pub is_community_import: bool,
}
pub struct EnrichedProfileHealthReport { #[serde(flatten)] core: ProfileHealthReport, metadata: Option<ProfileHealthMetadata> }
pub struct EnrichedHealthSummary { profiles, healthy_count, stale_count, broken_count, total_count, validated_at }
```

**TypeScript** — new file `src/types/health.ts`, re-exported from `src/types/index.ts`:

```typescript
type HealthStatus = 'healthy' | 'stale' | 'broken';
type HealthIssueSeverity = 'error' | 'warning' | 'info';

interface HealthIssue {
  field: string; path: string; message: string; remediation: string; severity: HealthIssueSeverity;
}
interface ProfileHealthReport {
  name: string; status: HealthStatus; launch_method: string; issues: HealthIssue[]; checked_at: string;
}
interface ProfileHealthMetadata {
  profile_id: string | null; last_success: string | null;
  failure_count_30d: number; total_launches: number;
  launcher_drift_state: string | null; is_community_import: boolean;
}
interface EnrichedProfileHealthReport extends ProfileHealthReport { metadata: ProfileHealthMetadata | null }
interface EnrichedHealthSummary {
  profiles: EnrichedProfileHealthReport[];
  healthy_count: number; stale_count: number; broken_count: number; total_count: number; validated_at: string;
}
```

**Phase D — `health_snapshots` table (migration v6, `migrations.rs`):**

```sql
CREATE TABLE IF NOT EXISTS health_snapshots (
    profile_id   TEXT NOT NULL REFERENCES profiles(profile_id),
    status       TEXT NOT NULL,
    issue_count  INTEGER NOT NULL DEFAULT 0,
    checked_at   TEXT NOT NULL,
    PRIMARY KEY (profile_id)
);
CREATE INDEX IF NOT EXISTS idx_health_snapshots_status ON health_snapshots(status);
```

One row per profile (UPSERT), no path strings stored, FK cascade on deletion.

---

## Cross-Cutting Concerns

- **Security pre-work blocks Phase A IPC wiring**: CSP (W-1: `"csp": null` → `"default-src 'self'"` in `tauri.conf.json`) must land before `commands/health.rs` is registered. `sanitize_display_path()` is already at `shared.rs:20` — no migration needed.
- **`sanitize_display_path()` on ALL path strings before IPC**: Both TOML-sourced and SQLite-sourced paths. Uses `Path::strip_prefix` — handles edge cases correctly. Apply at struct-assembly time to cover both IPC and Phase D persistence in one call (N-3).
- **Re-validate SQLite-sourced paths before `metadata()` calls** (N-1): Any code path reading a path from SQLite then calling `std::fs::metadata()` must first validate non-empty and absolute. Defense-in-depth.
- **Filter `deleted_at IS NULL` in MetadataStore joins** (N-4): Profile list from `ProfileStore::list()` is TOML-authoritative. Any `MetadataStore` method joining `profiles` must include `WHERE p.deleted_at IS NULL`. Reference: `collections.rs:143`, `profile_sync.rs:77`.
- **Method-aware validation**: Resolve launch method first; check only relevant fields. `steam.proton_path` for `steam_applaunch` only; `runtime.prefix_path` for `proton_run` only; empty optional fields → no issue.
- **Severity precedence**: Broken > Stale > Healthy. `Missing` (ENOENT) → Stale; `Inaccessible` (EACCES) / `WrongType` / `NotConfigured` (required field empty) → Broken.
- **`ValidationError::severity()` unconditionally returns `Fatal`** — `HealthIssueSeverity` is a new enum, derived from `HealthIssueKind`, not from `ValidationSeverity`.
- **`query_failure_trends` absence = healthy**: `HAVING failures > 0` in SQL. Build `HashMap` once before the loop; absent key = zero failures.
- **`failure_modes` is `Option<String>`**: `GROUP_CONCAT` result — comma-separated string or NULL. Parse carefully if surfacing individual modes in Phase B UI.
- **Composite health display (Phase B)**: Filesystem badge, failure-trend overlay, and launcher-drift indicator as **separate visual indicators** — do not merge into a single score until user feedback validates.
- **Touch targets**: All interactive health elements need `min-height: var(--crosshook-touch-target-min)` (48px). Controller hints: "Y Re-check" / "A Open" when broken profile focused.
- **Prefer promoted columns over `diagnostic_json` blob** (N-2): Read `severity`/`failure_mode` enum columns only. If free-text needed, apply `sanitize_diagnostic_report()` before IPC.

---

## Parallelization Opportunities

**Security pre-work:**
- S1 (CSP: one-line `tauri.conf.json` change) is independent of all other work.
- S2 (`sanitize_display_path()` already at `shared.rs:20`) — verification only, zero code change.

**Phase A (two parallel tracks):**
- **Track 1 — Rust backend**: Add `pub(crate)` to three helpers in `request.rs` (prefix-only) → implement `profile/health.rs` → unit tests with `tempdir` + `open_in_memory()`.
- **Track 2 — TypeScript layer**: Create `src/types/health.ts` → implement `useProfileHealth` hook (copy `useLaunchState` pattern) → implement `HealthBadge` (three-class pattern).
- **Track merge point**: `commands/health.rs` Tauri commands + `ProfilesPage.tsx` integration depend on both tracks completing.

**Phase B and Phase C are independent of each other** — both depend only on Phase A being complete.

**Phase D is independent** of Phase C — `health_snapshots` migration adds no coupling to startup integration.

---

## Implementation Constraints

- **Zero new Rust dependencies**: `std::fs`, `std::os::unix::fs::PermissionsExt`, `tokio`, `serde`, `rusqlite`, `tempfile` — all already present.
- **No new `.manage()` calls**: `ProfileStore` (line 76) and `MetadataStore` (line 80) already managed in `lib.rs`.
- **No `LaunchRequest` conversion path**: Validate `GameProfile` fields directly. `derive_steam_client_install_path()` in `profile.rs:17` is launcher-specific — not available or needed for health commands. Derive `steam_client_install_path` from `AppSettings` state in the command layer if needed for steam_applaunch validation.
- **Do not add new health-specific SQLite tables in Phase A/B**: `health_snapshots` is Phase D only.
- **Batch validation is synchronous**: `spawn_blocking` if needed; 50 profiles × 8 paths ≈ 400ms. Do not add `rayon`.
- **No file watching**: Reject `notify` crate.
- **Do not modify `startup.rs`**: Spawn health check from `lib.rs` async task pattern (lines 61–71) with 500ms delay.
- **Do not reuse `isStale()` from `LaunchPanel.tsx`**: Different concept (60-second preview staleness).
- **`src/utils/` does not exist**: Creating it is a side-effect of any utility extraction — mark optional.
- **No new Tauri capabilities required**: `std::fs::metadata()` needs no `fs:read` plugin.

---

## Key Recommendations

1. **CSP change is the only blocking pre-work code change**: One line in `tauri.conf.json`. `sanitize_display_path()` is already ready at `shared.rs:20`.
2. **Phase A Rust and TypeScript tracks are parallelizable**: Backend and frontend scaffolding can proceed simultaneously. `commands/health.rs` + `ProfilesPage.tsx` integration is the merge gate.
3. **`check_profile_health()` takes `Option<&MetadataStore>`**: Metadata is a parameter, not a module dependency. `batch_check_health(&store, None)` must pass tests — write explicit degraded-mode test.
4. **Build enrichment HashMaps before the per-profile loop**: One call to `query_failure_trends(30)` and one to `query_last_success_per_profile()`, both indexed by `profile_name`. Absent key = zero failures / no last-success.
5. **Enrich on `profile_id` (stable UUID), not `profile_name`**: Use `lookup_profile_id(name)` for drift and collection queries. Ensures enrichment survives renames.
6. **Community-import context note is Phase A scope**: Surface via `is_community_import` flag from `profiles.source` field. Do not defer to Phase B.
7. **Phase C startup mirrors `lib.rs:61–71` exactly**: 500ms delay, `app_handle.emit("profile-health-batch-complete", &payload)`, `tracing::warn!` on emit failure.
8. **Critical path**: CSP change (S1) → `commands/health.rs` registered → `useProfileHealth` hook → `ProfilesPage` integration. Rust domain logic and TypeScript types are off the critical path.

---

## Verified Codebase State

| Claim | Status |
| --- | --- |
| `sanitize_display_path()` in `shared.rs:20` | Confirmed — pub, uses `Path::strip_prefix`, handles home-dir trailing slash |
| Both stores already managed in `lib.rs` | Confirmed — `ProfileStore` line 76, `MetadataStore` line 80 |
| `invoke_handler!` registration point | Confirmed at line 85; append after "Phase 3" comment ~line 128 |
| `query_failure_trends` SQL uses `HAVING failures > 0` | Confirmed — absent profiles have zero failures |
| `query_failure_trends(days: u32)` signature | Confirmed — returns `Result<Vec<FailureTrendRow>, MetadataStoreError>` |
| `FailureTrendRow` fields | Confirmed — `profile_name: String, successes: i64, failures: i64, failure_modes: Option<String>` |
| `query_last_success_per_profile` return type | Confirmed — `Result<Vec<(String, String)>, MetadataStoreError>` (profile_name, ISO timestamp) |
| `with_conn` signature | Confirmed — `fn with_conn<F,T>(&self, action: &'static str, f: F)` where `T: Default`, lines 67–84 |
| `ValidationError::severity()` unconditionally returns `Fatal` | Confirmed at `request.rs:430` — `HealthIssueSeverity` must be a new enum |
| Three path helpers are bare `fn` at lines 700–756 | Confirmed — add `pub(crate)` prefix only, no other changes |
| Exact signatures of all three helpers | Confirmed — see Critical Files Reference above |
| CSS badge is three classes | Confirmed — `crosshook-status-chip crosshook-compatibility-badge crosshook-compatibility-badge--{rating}` |
| `derive_steam_client_install_path()` in `profile.rs:17` | Confirmed — launcher-specific, not relevant to health commands |
| `lib.rs` startup async task at lines 61–71 | Confirmed — exact pattern for Phase C, use 500ms delay |
| `profile/mod.rs` has no health module | Confirmed — exports: community_schema, exchange, legacy, models, toml_store only |
| `ProfileFormSections.tsx` — badges go outside it | Confirmed — health badges in `ProfilesPage.tsx` sidebar |
| `src/utils/` directory | Does NOT exist — `severityIcon()` extraction would create it |
| `MetadataStore::open_in_memory()` at line 44 | Confirmed — runs all migrations automatically |
