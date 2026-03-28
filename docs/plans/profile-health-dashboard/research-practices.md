# Engineering Practices Research: Profile Health Dashboard (Second Pass)

> **Second pass** — updated to account for the SQLite3 metadata layer added in PRs 89-91.
> Changes from the original are marked **[REVISED]** or **[NEW]**. Unchanged findings carry no marker.

## Executive Summary

The codebase already contains all the primitives needed for a profile health dashboard. `validate_all()` in `request.rs` covers path/config checking; `ProfileStore::list()` + `load()` provide batch iteration; `LauncherInfo.is_stale` demonstrates the staleness pattern; and — new since the first pass — `MetadataStore` adds launch-history data (`query_failure_trends`, `query_last_success_per_profile`) and drift-state semantics (`DriftState`) that enrich health output without new tables. The biggest risk remains over-engineering: the feature can be delivered as a thin wrapper around existing validation plus two targeted metadata queries, a single Tauri command, and a status chip list. No new tables or crates are needed.

---

## Existing Reusable Code **[REVISED]**

| Module / Utility                                 | Location                                                | Purpose                                                                                                                                                     | How to Reuse                                                                                                            |
| ------------------------------------------------ | ------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------- |
| `validate_all()`                                 | `crates/crosshook-core/src/launch/request.rs:444`       | Returns `Vec<LaunchValidationIssue>` for all path/config issues in a `LaunchRequest`                                                                        | Adapt or call directly for profile path checks; handles required/missing/not-file/not-dir for all path fields           |
| `require_directory()`                            | `crates/crosshook-core/src/launch/request.rs:700`       | Private helper: required → missing → not-directory three-case check                                                                                         | Promote to `pub(crate)` — three call sites justify it; `check_profile_health()` would be the third                      |
| `require_executable_file()`                      | `crates/crosshook-core/src/launch/request.rs:721`       | Private helper: required → missing → not-executable three-case check                                                                                        | Same promotion rationale                                                                                                |
| `is_executable_file()`                           | `crates/crosshook-core/src/launch/request.rs:742`       | Checks `is_file()` + Unix exec permission bit                                                                                                               | Promote to `pub(crate)` alongside the above                                                                             |
| `ValidationSeverity`                             | `crates/crosshook-core/src/launch/request.rs:143`       | `Fatal / Warning / Info` severity enum, Serde-serialized                                                                                                    | Reuse directly as health issue severity — already crosses IPC boundary                                                  |
| `LaunchValidationIssue`                          | `crates/crosshook-core/src/launch/request.rs:151`       | `{ message, help, severity }` — UI-ready issue struct                                                                                                       | Reuse as element type of `ProfileHealthInfo.issues`; avoids a parallel type                                             |
| `ProfileStore::list()`                           | `crates/crosshook-core/src/profile/toml_store.rs:136`   | Returns sorted `Vec<String>` of profile names                                                                                                               | Enumerate all profiles for batch health check                                                                           |
| `ProfileStore::load()`                           | `crates/crosshook-core/src/profile/toml_store.rs:100`   | Loads a single `GameProfile` from disk                                                                                                                      | Call per-profile; handles `NotFound` and TOML parse errors                                                              |
| `ProfileStore::with_base_path()`                 | `crates/crosshook-core/src/profile/toml_store.rs:96`    | Constructs store with an arbitrary path                                                                                                                     | Critical for testing: use with `tempfile::tempdir()` — no mocking needed                                                |
| `GameProfile` path fields                        | `crates/crosshook-core/src/profile/models.rs`           | All path strings live in `game.executable_path`, `trainer.path`, `steam.compatdata_path`, `steam.proton_path`, `runtime.prefix_path`, `runtime.proton_path` | Map each populated field to a health check                                                                              |
| `LauncherInfo.is_stale`                          | `crates/crosshook-core/src/export/launcher_store.rs:42` | Boolean stale flag on launcher info struct                                                                                                                  | Pattern to adopt: add `status / issues` fields to `ProfileHealthInfo`                                                   |
| `CompatibilityBadge` CSS                         | `src/components/CompatibilityViewer.tsx:76`             | `crosshook-status-chip crosshook-compatibility-badge--{rating}` chip pattern                                                                                | Reuse CSS class convention for `ProfileHealthBadge`                                                                     |
| `useLaunchState` reducer pattern                 | `src/hooks/useLaunchState.ts:46`                        | `useReducer` + typed actions for async state machine                                                                                                        | Copy `pending / loading / error / success` slice pattern for `useProfileHealth` hook                                    |
| `LaunchFeedback` discriminated union             | `src/types/launch.ts:43`                                | `kind: 'validation' \| 'diagnostic' \| 'runtime'` union                                                                                                     | Model `ProfileHealthStatus` the same way: `kind: 'healthy' \| 'stale' \| 'broken'`                                      |
| **`MetadataStore::open_in_memory()`** **[NEW]**  | `crates/crosshook-core/src/metadata/mod.rs:44`          | Opens an in-memory SQLite DB with migrations applied                                                                                                        | Use in health tests that exercise metadata queries — no temp file, fast, deterministic                                  |
| **`MetadataStore::with_path()`** **[NEW]**       | `crates/crosshook-core/src/metadata/mod.rs:40`          | Opens a MetadataStore at an explicit path                                                                                                                   | Test injection: pass a tmpdir-scoped path to isolate tests from production DB                                           |
| **`with_conn()` fail-soft wrapper** **[NEW]**    | `crates/crosshook-core/src/metadata/mod.rs:67`          | Returns `T::default()` if store is unavailable or disabled; lock-errors become `MetadataStoreError::Corrupt`                                                | Health commands should call MetadataStore the same way — surface metadata issues as `Warning` health items, never fatal |
| **`query_failure_trends(days)`** **[NEW]**       | `crates/crosshook-core/src/metadata/mod.rs:437`         | Returns `Vec<FailureTrendRow>` — per-profile failure counts + failure modes                                                                                 | Enrich `ProfileHealthInfo` with `recent_failures: Option<u32>` without new tables                                       |
| **`query_last_success_per_profile()`** **[NEW]** | `crates/crosshook-core/src/metadata/mod.rs:401`         | Returns `Vec<(profile_name, last_success_rfc3339)>`                                                                                                         | Use to flag profiles with no recent launches as `Warning` if relevant to UX                                             |
| **`DriftState`** **[NEW]**                       | `crates/crosshook-core/src/metadata/models.rs:122`      | `Unknown / Aligned / Missing / Moved / Stale` enum for launcher drift                                                                                       | Health check can query launchers table to include launcher drift in health output                                       |
| **`FailureTrendRow`** **[NEW]**                  | `crates/crosshook-core/src/metadata/models.rs:278`      | `{ profile_name, successes, failures, failure_modes }`                                                                                                      | Ready-made result type; map into `ProfileHealthInfo` as supplemental data                                               |
| **`MetadataStore::disabled()`** **[NEW]**        | `crates/crosshook-core/src/metadata/mod.rs:48`          | No-op store when SQLite is unavailable                                                                                                                      | Health command must tolerate this — metadata enrichment is optional, path checks are not                                |

---

## Modularity Design **[REVISED]**

### Recommended module boundaries — Rust backend

```
crates/crosshook-core/src/profile/
    health.rs          ← NEW: ProfileHealthInfo, check_profile_health(), batch_check_health()
```

`health.rs` belongs in the `profile` module (same rationale as the first pass — it operates on `GameProfile`). It is a **client** of `MetadataStore`, not a method on it.

**[REVISED] Do NOT add health methods to MetadataStore.** The `launch_history.rs` / `launcher_sync.rs` pattern adds methods to `MetadataStore` because those modules own database-writing operations. Health checks are read-only enrichment queries; the filesystem path checks are the primary result. Adding health methods to `MetadataStore` would import profile-domain logic into the metadata module and invert the existing dependency direction. The correct pattern is the same one used by `profile_sync.rs` — `health.rs` receives a `&MetadataStore` (or `Option<&MetadataStore>`) as a parameter and calls its existing public query methods.

```rust
// profile/health.rs — signature (metadata is optional enrichment)
pub fn check_profile_health(
    name: &str,
    profile: &GameProfile,
    metadata: Option<&MetadataStore>,
) -> ProfileHealthInfo

pub fn batch_check_health(
    store: &ProfileStore,
    metadata: Option<&MetadataStore>,
) -> Vec<ProfileHealthInfo>
```

### Tauri command layer

Add one command to `src-tauri/src/commands/profile.rs`:

```rust
#[tauri::command]
pub fn check_profiles_health(
    store: State<'_, ProfileStore>,
    metadata_store: State<'_, MetadataStore>,
) -> Result<Vec<ProfileHealthInfo>, String>
```

This follows the exact pattern of `profile_save`, `profile_delete`, `profile_duplicate` — all take both `State<'_, ProfileStore>` and `State<'_, MetadataStore>`. Pass `Some(metadata_store.inner())` to `batch_check_health`. If MetadataStore is in `disabled()` state its queries silently return empty via `with_conn()`'s `T::default()` path — health works degraded but does not fail.

### Frontend

```
src/hooks/useProfileHealth.ts    ← NEW: batch-fetch hook, mirrors useLaunchState structure
src/components/ProfileHealthBadge.tsx  ← optional, only if badge is used in 2+ places
```

### Shared vs. feature-specific

- **Shared (leave alone):** `LaunchValidationIssue`, `ValidationSeverity`, `ProfileStore`, CSS chip classes, all existing `MetadataStore` query methods
- **Feature-specific (new):** `ProfileHealthInfo` struct, `check_profile_health()`, `batch_check_health()`, `useProfileHealth` hook, `ProfileHealthBadge` component

---

## KISS Assessment **[REVISED]**

| Approach                                                              | Complexity | Value delivered                       | Verdict                                                                                     |
| --------------------------------------------------------------------- | ---------- | ------------------------------------- | ------------------------------------------------------------------------------------------- |
| Full dashboard tab: table, per-profile drill-down, remediation wizard | High       | High                                  | Over-engineered for v1                                                                      |
| Inline health chip on profile list + expandable issue list            | Medium     | High                                  | **Recommended**                                                                             |
| Extend launch-time validation to show at profile list time            | Low        | Medium                                | Viable MVP, no new component                                                                |
| New `health/` top-level crate module                                  | High       | Zero beyond a single `health.rs` file | Over-engineered                                                                             |
| **[NEW] Add new health-specific tables to SQLite**                    | Medium     | Low                                   | **Do not do this**                                                                          |
| **[NEW] "Just query existing tables"**                                | Low        | Medium                                | **Correct** — `launch_operations` + `launchers` + `profiles` already have everything needed |

**[NEW] SQLite complexity verdict:** The metadata layer does NOT over-complicate health checks. The existing `query_failure_trends()` and `query_last_success_per_profile()` methods require zero new SQL. Call them as-is and merge the results into `ProfileHealthInfo`. Adding new health-specific tables would be the wrong move — the data already exists and the queries are already written.

### Existing over-engineering risks (unchanged)

1. **New severity enum:** `ValidationSeverity` is already `Fatal/Warning/Info` and crosses the IPC boundary. Do not invent a parallel `HealthStatus` — reuse the existing type.
2. **Remediation wizard:** Actionable help text is already in every `LaunchValidationIssue.help` string. Surface these strings directly.
3. **Streaming/background health check:** Synchronous `invoke` is sufficient for < 50 profiles. Add streaming only if profiling shows > 200 ms latency.

---

## Abstraction vs. Repetition **[REVISED]**

### What to extract

- **`require_directory()`, `require_executable_file()`, `is_executable_file()`**: Three call sites after `health.rs` is added (`validate_steam_applaunch`, `validate_proton_run`, `check_profile_health`) — promote to `pub(crate)` in `request.rs` or a new `crates/crosshook-core/src/launch/path_checks.rs`. This is the rule-of-three threshold; promote them, do not duplicate.

### What to repeat

- Per-profile dispatch logic: flat `if !path.is_empty()` sequence on `GameProfile`. Do not abstract into a trait or macro — read once, never polymorphic.
- CSS badge class construction: inline `crosshook-status-chip crosshook-health-badge--{status}` in JSX. No utility function at 1-2 call sites.

### **[NEW] Metadata query repetition non-issue**

`query_failure_trends()` and `query_last_success_per_profile()` are already defined on `MetadataStore`. Health code simply calls them — no new query strings need writing. There is no abstraction decision here.

---

## Interface Design

### Rust public API (`crates/crosshook-core/src/profile/health.rs`)

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfileHealthInfo {
    pub name: String,
    pub status: ProfileHealthStatus,
    pub issues: Vec<LaunchValidationIssue>,  // reuse existing type
    pub recent_failures: Option<u32>,         // NEW: from query_failure_trends()
    pub last_launch_at: Option<String>,       // NEW: from query_last_success_per_profile(), RFC3339
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProfileHealthStatus {
    Healthy,
    Stale,   // path issues (files missing/moved)
    Broken,  // parse failures or required field missing
}

pub fn check_profile_health(
    name: &str,
    profile: &GameProfile,
    metadata: Option<&MetadataStore>,
) -> ProfileHealthInfo

pub fn batch_check_health(
    store: &ProfileStore,
    metadata: Option<&MetadataStore>,
) -> Vec<ProfileHealthInfo>
```

### Tauri IPC surface

One new command: `check_profiles_health() -> Result<Vec<ProfileHealthInfo>, String>`

Keeps the contract minimal; the frontend fetches the whole batch at once.

### TypeScript mirror

```typescript
// src/types/profile.ts additions
export type ProfileHealthStatus = 'healthy' | 'stale' | 'broken';

export interface ProfileHealthInfo {
  name: string;
  status: ProfileHealthStatus;
  issues: ProfileHealthIssue[];
  recent_failures?: number;
  last_launch_at?: string;
}

export interface ProfileHealthIssue {
  message: string;
  help: string;
  severity: LaunchValidationSeverity; // reuse existing type
}
```

---

## Testability Patterns **[REVISED]**

### Filesystem tests — unchanged pattern

**`tempfile::tempdir()` + `ProfileStore::with_base_path()`**: Every `toml_store.rs` test uses this pattern. Health check tests should do the same.

```rust
#[test]
fn healthy_profile_reports_no_issues() {
    let temp_dir = tempdir().unwrap();
    let store = ProfileStore::with_base_path(temp_dir.path().join("profiles"));
    store.save("game", &complete_profile_fixture()).unwrap();
    // Write actual fixture files to temp_dir so path checks pass
    let results = batch_check_health(&store, None);
    assert_eq!(results[0].status, ProfileHealthStatus::Healthy);
}
```

### **[NEW] Combined filesystem + metadata test fixture**

For tests that exercise metadata enrichment alongside filesystem path checks:

```rust
#[test]
fn profile_with_failures_shows_recent_failure_count() {
    let temp_dir = tempdir().unwrap();
    let store = ProfileStore::with_base_path(temp_dir.path().join("profiles"));
    store.save("game", &complete_profile_fixture()).unwrap();

    // MetadataStore::open_in_memory() runs all migrations automatically
    let metadata = MetadataStore::open_in_memory().unwrap();
    // Seed failure data using existing API
    // (record_launch_started / record_launch_finished from launch_history.rs)

    let results = batch_check_health(&store, Some(&metadata));
    assert_eq!(results[0].recent_failures, Some(3));
}
```

**Key points:**

- `MetadataStore::open_in_memory()` runs `run_migrations()` automatically — no schema setup needed in tests.
- `with_conn()` fail-soft means `batch_check_health(&store, None)` (metadata absent) must still work — test both paths.
- Do not share a `MetadataStore` instance across test cases; each test gets its own `open_in_memory()` call.

### Anti-patterns to avoid (unchanged)

- Do not mock `std::fs` or `std::path::Path`.
- Do not test via the Tauri command layer.
- Do not assert on `help` text strings — assert on `severity` and `status` enum values.
- **[NEW]** Do not seed test data via raw SQL strings in health tests — use the existing `record_launch_started` / `record_launch_finished` API so tests stay in sync with schema migrations.


### **[NEW] Phase D testability note — JOIN query seeding**

When `profile_health_snapshots` is added (Phase D, migration 6), integration tests for the
`query_health_with_launch_context` JOIN query will need to seed both tables:

```rust
// Step 1 — populate launch_operations (existing API, unchanged)
let op_id = metadata.record_launch_started(Some("game"), "steam_applaunch", None).unwrap();
metadata.record_launch_finished(&op_id, Some(0), None, &clean_report()).unwrap();

// Step 2 — populate profile_health_snapshots (new method added in Phase D)
metadata.record_health_snapshot("game", &health_info).unwrap();
```

Both steps use the typed API — no raw SQL. `open_in_memory()` remains the correct test fixture
because migration 6 will run automatically alongside the existing migrations.

---

## Build vs. Depend

| Capability                                 | Build custom                                   | Use library                                                       | Recommendation                                               |
| ------------------------------------------ | ---------------------------------------------- | ----------------------------------------------------------------- | ------------------------------------------------------------ |
| File existence / is_file / is_dir checks   | `std::fs`, `std::path::Path`                   | —                                                                 | **Build** (stdlib, already used throughout)                  |
| Execute permission bit                     | `std::os::unix::fs::PermissionsExt`            | —                                                                 | **Build** (already done in `is_executable_file()`)           |
| Temporary directories in tests             | —                                              | `tempfile` (already a dev-dependency)                             | **Use existing**                                             |
| Parallel batch health checks               | `rayon`                                        | —                                                                 | **Defer** — not needed for < 50 profiles                     |
| Path canonicalization / symlink resolution | `std::fs::canonicalize`                        | —                                                                 | **Build if needed** — no new dependency                      |
| TOML parsing errors as health issues       | Already handled by `ProfileStoreError::TomlDe` | —                                                                 | **Reuse existing** error types                               |
| **[NEW] Launch failure counts**            | —                                              | `MetadataStore::query_failure_trends()` (already built)           | **Reuse existing** — zero new SQL                            |
| **[NEW] Last launch timestamp**            | —                                              | `MetadataStore::query_last_success_per_profile()` (already built) | **Reuse existing** — zero new SQL                            |
| **[NEW] In-memory test DB**                | —                                              | `MetadataStore::open_in_memory()` (already built)                 | **Reuse existing** — no temp file management needed in tests |

**Verdict (updated):** No new crates needed. `std::fs`, `std::path`, existing `tempfile` dev-dep, and existing `crosshook-core` types cover everything. The metadata layer provides additional data at zero dependency cost.

---

## Additional Utility Consolidation

**`sanitize_display_path()` in `src-tauri/src/commands/launch.rs:301`** replaces `$HOME` with `~` for display. The health dashboard Tauri command should call this on path strings in `LaunchValidationIssue.message` before returning over IPC. Move this function to `src-tauri/src/commands/shared.rs` (display concern, not business logic).

**`severityIcon()` in `LaunchPanel.tsx`** maps `LaunchValidationSeverity` to a Unicode character. Extract to `src/utils/severity.ts` if the health dashboard component also needs this mapping (2 call sites crosses the deduplication threshold).

**`isStale()` in `LaunchPanel.tsx:119` — do NOT reuse for profile health.** This is a 60-second threshold for launch preview staleness (`generatedAt` timestamp on `LaunchPreview`). The concept is unrelated to profile path health. Define a separate constant in `useProfileHealth.ts` if staleness timing is needed.

**`DiagnosticCollector` pattern does NOT exist in the codebase.** `launch/diagnostics/mod.rs` uses a plain `analyze()` function returning a struct directly. Health checking should follow the same plain-function style.

---

## Health Validation Architecture (Revised — No `LaunchRequest` Conversion)

`check_profile_health()` operates directly on `GameProfile` — no `GameProfile → LaunchRequest` conversion. This keeps health checking as a pure profile-domain concern.

**Duplication risk — critical engineering constraint:**

`check_profile_health()` will perform the same path-existence checks as `validate_all()` in `request.rs`. Without care, this becomes a near-duplicate that diverges over time.

**How to avoid it:** The path-checking logic is already factored into private helpers — `require_directory()`, `require_executable_file()`, `is_executable_file()`. Promoting these to `pub(crate)` lets both `validate_all()` and `check_profile_health()` call the same functions. The `ValidationError` variants and their `.issue()` / `message()` / `help()` text are equally reusable — `check_profile_health()` should construct `ValidationError` variants and call `.issue()` to get `LaunchValidationIssue`s, not invent new message strings.

**Method-dispatch still applies:** Call `resolve_launch_method(profile)` first and check only the paths relevant to the resolved method. Do not check Proton paths on a native profile.

**Module dependency note (verified):** `launch/request.rs` already imports `use crate::profile::TrainerLoadingMode` at line 9 — a `launch → profile` type dependency already exists. The `profile/health.rs → launch::path_checks` dependency formalizes the existing direction, not a new cross-module edge.

> **Not async:** `batch_check_health()` should be synchronous. For < 50 profiles the stat calls complete in single-digit milliseconds.

---

## Open Questions

1. **Should `batch_check_health` skip or include profiles that fail to parse (TOML error)?** Recommendation: include them as `broken` entries with a synthetic `LaunchValidationIssue` describing the parse error — makes the dashboard useful for diagnosing corrupt profiles.

2. **Which paths in `GameProfile` should be health-checked?** Not all paths are always required. The health checker should derive the method from `resolve_launch_method(profile)` first, then check only the paths relevant to that method.

3. **How does health refresh interact with profile save?** The simplest answer: frontend calls `check_profiles_health` on mount and on explicit refresh. No event-driven invalidation needed for v1.

4. **Should health status be shown inline in the existing profile list or as a separate view?** Inline chips (one per profile row) with a collapsible detail panel are the lowest-friction approach and reuse the existing `CollapsibleSection` pattern from `CompatibilityViewer.tsx` and `LaunchPanel.tsx`.

5. **`LaunchValidationIssue` vs new `HealthValidationIssue`?** Do not create a new type. The `help` field already contains remediation guidance. If a `field: Option<String>` tag is truly needed by the UI, add it to `LaunchValidationIssue` rather than duplicating the type hierarchy.

6. **[NEW] Should `recent_failures` and `last_launch_at` be null when MetadataStore is disabled?** Yes. Both fields are `Option<T>` on `ProfileHealthInfo`. The fail-soft `with_conn()` pattern ensures `query_failure_trends()` returns an empty `Vec` when metadata is unavailable — health degrades gracefully to path-checks-only mode. This is explicit in the type: `None` means "no metadata available", not "zero failures".

7. **[NEW] Should health queries use `query_failure_trends(30)` (last 30 days)?** 30 days is a reasonable default. The caller (Tauri command) can make `days` a parameter if the frontend needs a configurable window, but for v1 a hardcoded 30-day window in `batch_check_health` is simpler.
