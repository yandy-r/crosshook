# Profile Health Dashboard — Business Analysis (Second Pass)

> **Second pass**: This document revises the original spec written before the SQLite3 metadata layer
> (PRs 89–91). Changes from the original are marked **[REVISED]** or **[NEW]**. Rules that remain
> unchanged are left unmarked.

---

## Executive Summary

CrossHook users silently accumulate broken profiles after game updates, Proton version changes, and
trainer file moves. The profile health dashboard surfaces per-profile health status (healthy / stale
/ broken) in batch by validating **persisted profile paths at rest** — distinct from launch-time
validation which checks runtime configuration compatibility. With the SQLite metadata layer now live,
health results can be **persisted per profile UUID**, failure trends from `launch_operations` can
enrich the health signal beyond filesystem-only checks, and launcher drift state from the `launchers`
table becomes a first-class health dimension. The feature uses `ProfileStore::list()` +
`ProfileStore::load()` for batch loading, a new `validate_profile_health()` function in
`crosshook-core/src/profile/health.rs` for path-existence checks, `MetadataStore` APIs for
failure trends and launcher drift, and presents results using the `crosshook-status-chip` badge
pattern. **[REVISED]** Health status can now be persisted to SQLite keyed by stable `profile_id`
UUID, replacing the original in-memory-only constraint.

---

## User Stories

### Primary Users

- **Steam Deck users** managing multiple game profiles — most affected because SteamOS auto-updates
  Proton versions, silently breaking saved `proton_path` fields
- **Linux gamers** who install/uninstall Proton versions from Steam or move game libraries

### Stories

| ID    | As a…                                  | I want…                                                                                | So that…                                                                              |
| ----- | -------------------------------------- | -------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------- |
| US-1  | Steam Deck user                        | to see a health badge on each profile at a glance                                      | I know which profiles need attention before game night                                |
| US-2  | Linux gamer                            | CrossHook to check all profiles in the background at startup                           | I get notified of broken configs after Proton auto-updates without any extra steps    |
| US-3  | User with many profiles                | to see a summary count ("3 of 12 profiles have issues") rather than a wall of warnings | I'm not overwhelmed and can triage what matters                                       |
| US-4  | User with a broken profile             | to see the specific path that failed and a fix suggestion                              | I know exactly what to do to fix it                                                   |
| US-5  | User who imported a community profile  | to see immediately if the profile references paths that don't exist on my system       | I can run Auto-Populate before wasting time on a launch attempt                       |
| US-6  | User who uninstalled a game            | to see the affected profiles marked stale rather than broken                           | I understand this is a normal lifecycle event, not a configuration error              |
| US-7  | Steam Deck user in portable mode       | to not see profiles as "broken" just because my SD card isn't mounted                  | I get accurate health status for the storage that is actually available               |
| US-8  | **[NEW]** User with recurring failures | to see which profiles have been failing recently even when paths look intact           | I can investigate configuration issues that don't manifest as missing files           |
| US-9  | **[NEW]** User with exported launchers | to see if any of my exported launchers have drifted from their source profiles         | I know if my desktop shortcuts or .sh scripts are stale and need to be re-exported    |
| US-10 | **[NEW]** Power user                   | to see a profile's last successful launch date alongside its health badge              | I can prioritize fixing profiles I actually use over ones I haven't touched in months |
| US-11 | **[NEW]** User browsing favorites      | to filter the health view by my favorite profiles or a collection                      | I can quickly assess just the profiles I care most about                              |

---

## Business Rules

### Scope Boundary: Health vs. Launch Validation

**Profile health validation** and **launch validation** are distinct concerns that must NOT be conflated:

| Concern                           | What it checks                                                                                                                         | When it runs                           |
| --------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------- |
| **Profile health** (this feature) | Do the filesystem paths stored in `GameProfile` still exist on disk with correct types and permissions?                                | Background at startup + manual trigger |
| **Launch validation** (existing)  | Is the runtime configuration compatible? (method constraints, optimization conflicts, `steam_client_install_path` derived value, etc.) | Immediately before each launch attempt |

Health checks validate paths at rest in `GameProfile`. They do **not** validate:

- Launch optimization conflicts or dependencies
- Launch method compatibility constraints
- `steam_client_install_path` — derived at launch time from `compatdata_path`
- Whether a WINE prefix has been fully initialized
- Whether Steam App IDs resolve to real games (no network calls)
- Whether Proton prefix directories are structurally valid (shallow only)

A profile can be **health-healthy** (all paths exist) and still fail launch validation (e.g.,
incompatible optimization combination). Health status is a prerequisite signal, not a launch
guarantee.

**Architecture constraint:** The new `validate_profile_health()` function lives in
`crosshook-core/src/profile/health.rs` — not `launch/`. It takes `&GameProfile` directly, never
constructs a `LaunchRequest`.

---

### Core Health Classification

**Profile-level status** — three states (plus a presentation variant):

| Status           | Condition                                                                                                             | Visual (existing CSS)                            | Notification behavior                               |
| ---------------- | --------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------ | --------------------------------------------------- |
| **Healthy**      | All required fields configured; all configured paths accessible with correct type                                     | Green — `crosshook-compatibility-badge--working` | No notification                                     |
| **Stale**        | Required fields configured (non-empty); one or more configured paths missing from disk                                | Amber — `crosshook-compatibility-badge--partial` | Badge only — no banner                              |
| **Broken**       | Required field empty/unconfigured, OR path exists but wrong type, OR path exists but inaccessible (permission denied) | Red — `crosshook-compatibility-badge--broken`    | Banner notification on startup                      |
| **Unconfigured** | UI presentation variant of Broken for profiles with ALL path fields empty (never set up)                              | Same red badge — softer notification tone        | Badge only (less alarming; normal for new profiles) |

**Severity precedence:** If a profile has both Stale and Broken issues, the overall status is
**Broken** (more severe wins).

**Stale vs. Broken distinction:**

- _Stale_ = the path was configured; external state changed (Proton updated, game uninstalled,
  trainer moved, SD card not mounted). This is a **normal lifecycle event**.
- _Broken_ = the profile data itself is incomplete or self-contradictory. User must edit the profile
  or fix permissions.

---

### **[NEW]** Composite Health Signal — Filesystem + Launch History

The tri-state filesystem classification is enriched by launch history from `launch_operations`.
A profile can be **path-healthy** (all paths exist) but **launch-failing** (every recent launch
recorded `status = 'failed'`). These composite signals require a clear combination rule.

**Composite health matrix:**

| Filesystem state | Launch history (last N days)                    | Composite status | Display                                              |
| ---------------- | ----------------------------------------------- | ---------------- | ---------------------------------------------------- |
| Healthy          | Never launched (no rows in launch_operations)   | Healthy          | Green badge; detail panel shows "Never launched"   |
| Healthy          | Launched but no activity in lookback window     | Healthy          | Green badge                                          |
| Healthy          | Mixed (some successes in lookback window)       | Healthy          | Green badge                                          |
| Healthy          | All failures (0 successes, ≥2 failures)         | Degraded         | Amber badge + launch-fail indicator                  |
| Healthy          | Exclusively `failure_mode = clean_exit`         | Healthy          | Green badge (user quit the game)                     |
| Stale            | Any history                                     | Stale            | Amber badge (filesystem wins over history)           |
| Broken           | Any history                                     | Broken           | Red badge (filesystem wins)                          |

**"Degraded" is a new composite sub-state of Healthy**, not a new top-level enum variant. It means
"paths exist, but launch history suggests the configuration is not working." It maps to the Amber
badge and does not trigger a startup banner (same as Stale).

**Launch failure threshold rule (BR-NEW-1):** A profile is marked "launch-failing" if, within the
configurable lookback window (default: 30 days), it has at least 2 recorded failures AND 0
successes. A single failure does not trigger the indicator — one-off crashes are normal.

"Never launched" (no rows in `launch_operations` at all) and "launched but nothing in the lookback
window" are both treated as neutral — no Degraded indicator. These are distinct from "launched
multiple times and always failed": the latter is an active signal; the former is absence of data.
The `never_launched: bool` flag on `ProfileHealthResult` captures the never-launched case so the UI
can show a contextual "Never launched" note in the detail panel without implying a problem.

When MetadataStore is unavailable, `never_launched` defaults to false (unknown, not asserted).

**`clean_exit` exclusion (BR-NEW-2):** `FailureMode::CleanExit` maps to `LaunchOutcome::Succeeded`
in `record_launch_finished()`. Do not count `clean_exit` records as failures in health scoring.
This is already handled by the existing `record_launch_finished` implementation.

---

### **[NEW]** Launcher Drift as a Health Dimension

The `launchers` table tracks `drift_state` (aligned / missing / moved / stale / unknown) for each
exported launcher. Launcher drift is **a separate health dimension from profile health** — a profile
can be path-Healthy but have a drifted launcher, or be path-Broken with an aligned launcher.

**Launcher drift business rules (BR-NEW-3):**

- Launcher drift is surfaced as a secondary indicator on the profile health entry, not as a
  modifier of the primary health badge. The primary badge reflects filesystem + launch history.
- `DriftState::Aligned` → no launcher drift indicator shown.
- `DriftState::Missing` or `DriftState::Moved` → amber drift warning: "Exported launcher may be
  out of sync — re-export to update."
- `DriftState::Stale` → amber drift warning: "Exported launcher was generated from an older profile
  version — re-export to update."
- `DriftState::Unknown` → no indicator (insufficient data).
- A profile with no launcher row in the `launchers` table has no launcher drift indicator.

**Rationale:** Launcher drift is a separate concern from profile validity. Merging it into the
primary badge would create a confusing "Amber because launcher is stale but the profile itself is
fine" scenario. Separate indicator keeps the primary health signal clean.

---

### **[REVISED]** Business Rule 8: Persistence

> **Original Rule 8 (superseded):** "Health results are held in frontend state (in-memory) only —
> never written to disk."

**Revised Rule 8:** Health results **CAN** be persisted to SQLite via the `MetadataStore`, keyed
by the profile's stable `profile_id` UUID. Persistence is optional and additive — the filesystem
scan remains the authoritative source of truth; SQLite stores a cache of the most recent result.

**Persistence constraints:**

- Health results are stored under the profile's `profile_id` (from `profiles.profile_id`), not
  the filename. This means health history survives profile renames (UUIDs are rename-stable).
- A health result row is invalidated (deleted or marked stale) whenever `observe_profile_write()`,
  `observe_profile_rename()`, or `observe_profile_delete()` is called for the profile. The
  Tauri command layer that calls those hooks must also invalidate the cached health row.
- When `MetadataStore.available` is false, persistence is silently skipped — the feature
  degrades gracefully to the original in-memory behavior. This is the **fail-soft pattern** all
  new metadata features follow.
- Persisted health results are used to populate the UI on subsequent app launches **before** the
  background scan completes, so users see the last-known health state immediately rather than
  waiting for the scan. Once the scan finishes, the fresh result replaces the cached one.
- The SQLite row must record `checked_at` timestamp. Results older than a configurable threshold
  (default: 7 days) are treated as unknown/stale even if persisted, to avoid showing stale health
  data after a long gap in usage.
- **What is NOT persisted:** `ProfileHealthIssue` detail records (field labels, message strings,
  path-containing help text) are never written to SQLite. Only the summary badge row
  (`status`, `is_degraded`, `failure_count`, `launcher_drift`, `checked_at`) is cached. Issue
  detail is always recomputed from the live filesystem scan.

**Security-researcher concern acknowledged:** The security-researcher recommends against
persisting health results at all, citing stale-data risk and `launch_operations` as a sufficient
proxy for "last known good." This concern is valid for the issue-detail level but does not apply
to the status-only cache row defined here:
- `launch_operations` gives last-success timestamp but does NOT give the filesystem health
  classification (Healthy/Stale/Broken/is_degraded). These are distinct signals.
- The status-only cache row contains no path strings, so stale-data risk is limited to showing
  a stale badge color (e.g., Green when the profile has since gone Stale) — not misleading path
  detail. The 7-day threshold and mandatory `checked_at` timestamp bound this risk.
- Issue-detail records (which DO contain path strings) are explicitly excluded from persistence,
  directly addressing the security-researcher's path-leakage concern.
- If implementation complexity of the cache proves higher than expected, falling back to the
  original in-memory-only behavior (original Rule 8) is a valid scope reduction — the cache is
  additive and the feature works correctly without it.

---

### **[NEW]** Fail-Soft Degradation Rules

The `MetadataStore` uses an `available: bool` flag. All health features that depend on SQLite
must degrade to the original filesystem-only behavior when `available` is false.

**Degradation boundaries (BR-NEW-4):**

| Health feature                      | MetadataStore available                  | MetadataStore unavailable                   |
| ----------------------------------- | ---------------------------------------- | ------------------------------------------- |
| Filesystem path checks              | Full                                     | Full (no DB dependency)                     |
| Persisted result cache (last state) | Yes                                      | Falls back to "not checked yet"             |
| Launch failure trend enrichment     | Yes (`query_failure_trends()`)           | Omitted; badge shows filesystem-only health |
| Last-success timestamp              | Yes (`query_last_success_per_profile()`) | Omitted from UI                             |
| Launcher drift indicator            | Yes (launchers table)                    | Omitted; no drift indicator shown           |
| Collection/favorites filter         | Yes                                      | Filter controls hidden or disabled          |

**No silent failures:** If `MetadataStore` is available but a specific query fails, log the error
and degrade that dimension only — do not abort the full health scan. This mirrors the per-profile
error isolation rule: one failure must not abort the batch.

---

### **[REVISED]** Notification Rules

**Base rules** (unchanged):

- Broken profiles → startup banner ("N profiles have broken paths — click to review")
- Stale profiles → badge only; no startup banner
- Unconfigured profiles → badge only

**[REVISED] Extended notification rules with launch history:**

- **Degraded profiles** (path-healthy but launch-failing) → badge only; no startup banner.
  The amber "Degraded" indicator appears in the health detail panel, not in startup banners.
- Notification deduplication: if a profile was already shown in a startup banner for the same
  issues since last app launch, do not show it again within the same session.
- **[NEW]** Stale persisted result (checked_at older than 7 days): show a subtle "Last checked N
  days ago" note in the health panel alongside the cached badge, prompting manual re-check.

---

### Retained Rules (Unchanged from Original)

**Rule 1 — Health vs. Launch Validation Boundary**: (see Scope Boundary section above)

**Rule 2 — Tri-State Classification**: Healthy / Stale / Broken (with Degraded as composite
sub-state of Healthy per BR-NEW-1 above).

**Rule 3 — Severity Precedence**: Broken > Stale > Degraded > Healthy.

**Rule 4 — Method-Aware Validation**: Health checks only validate fields required by the profile's
resolved launch method.

| Profile field              | Required for method                                       | Optional for method         | Always optional |
| -------------------------- | --------------------------------------------------------- | --------------------------- | --------------- |
| `game.executable_path`     | `proton_run`, `native`; conditional for `steam_applaunch` | —                           | —               |
| `trainer.path` (host path) | Any (when trainer workflow configured)                    | Any (when game-only)        | —               |
| `steam.proton_path`        | `steam_applaunch`                                         | `proton_run`, `native`      | —               |
| `steam.compatdata_path`    | `steam_applaunch`                                         | `proton_run`, `native`      | —               |
| `steam.app_id`             | `steam_applaunch`                                         | `proton_run`, `native`      | —               |
| `runtime.proton_path`      | `proton_run`                                              | `steam_applaunch`, `native` | —               |
| `runtime.prefix_path`      | `proton_run`                                              | `steam_applaunch`, `native` | —               |
| `injection.dll_paths[]`    | —                                                         | —                           | Always optional |
| `steam.launcher.icon_path` | —                                                         | —                           | Always optional |

**Rule 5 — Removable Media Rule**: `Path::exists()` returning false → always Missing → Stale.

**Rule 6 — Permission Denied Is Distinct**: Inaccessible (EACCES) → Broken; not conflated
with Missing.

**Rule 7 — No Auto-Repair**: Health dashboard is strictly read-only diagnostic.

**Rule 9 — Non-Blocking Startup**: Startup health scan is a background async task after UI
renders. A single profile failing to load must not abort the batch scan.

---

### Path Issue Classification (Field Level)

For each path field in `GameProfile`, the health check produces one of four per-field states:

| Field state       | Filesystem condition                                                           | Profile impact                                                |
| ----------------- | ------------------------------------------------------------------------------ | ------------------------------------------------------------- |
| **NotConfigured** | Field is empty string                                                          | Required field → Broken; Optional field → no issue (expected) |
| **Missing**       | Path non-empty, `Path::exists()` → false (ENOENT or unmounted media)           | → **Stale**                                                   |
| **Inaccessible**  | Path exists, but permission denied on stat/access (EACCES)                     | → **Broken** (different remediation: "fix permissions")       |
| **WrongType**     | Path exists and accessible, but is file where directory expected or vice versa | → **Broken**                                                  |
| **Healthy**       | Path exists, accessible, correct type                                          | → no issue                                                    |

---

### **[REVISED]** Edge Cases

| Scenario                                                     | Expected Behavior                                                                                | Notes                                                               |
| ------------------------------------------------------------ | ------------------------------------------------------------------------------------------------ | ------------------------------------------------------------------- |
| Empty/unconfigured profile (all paths empty)                 | Classified as Broken but presented with softer "Unconfigured" UI tone                            | Badge only, no startup banner                                       |
| Community-imported profile with many missing paths           | Show contextual note: "This profile was imported — paths may need to be updated for your system" | Must-have to avoid blaming CrossHook                                |
| Profile TOML parse error                                     | Classified as Broken with "Profile data could not be read" message                               | Must not abort batch scan                                           |
| SD card unmounted (Steam Deck)                               | All affected profiles show Stale, not Broken                                                     | Conservative classification                                         |
| Proton auto-updated by Steam (e.g., 9.0-1 → 9.0-2)           | Profile shows Stale (old path missing)                                                           | Future: detect pattern for targeted "Proton updated" message        |
| Symlink to deleted target                                    | Reports as Missing (Stale) — `metadata()` follows symlinks                                       | Correct behavior                                                    |
| File exists but not executable (Proton binary)               | Reports as Broken (WrongType)                                                                    | `PermissionsExt::mode() & 0o111 == 0`                               |
| **[NEW]** Profile renamed — health result in SQLite          | `profile_id` UUID is rename-stable; persisted result still linked after rename                   | `observe_profile_rename()` invalidates the result (stale, re-check) |
| **[NEW]** MetadataStore unavailable                          | Health scan runs filesystem-only; no failure trend enrichment; no drift indicator                | Silent degradation — no error banner for missing metadata           |
| **[NEW]** Profile has 30 failures, 0 successes (Degraded)    | Shows Amber badge with "Launch failures detected" sub-indicator                                  | Does not trigger startup banner; appears in detail panel only       |
| **[NEW]** Persisted health result > 7 days old               | Show cached badge with "Last checked N days ago" note; prompt manual re-check                    | Avoids showing actively wrong health data after long gaps           |
| **[NEW]** Launcher drift state = Missing for aligned profile | Amber drift indicator shown alongside Green health badge                                         | Separate dimension — does not change primary badge color            |
| **[NEW]** Profile deleted; launch_operations rows remain     | Orphaned `launch_operations` rows are expected; `query_failure_trends()` filters by profile_name | Soft-delete pattern in `profiles` table; FK is nullable             |

---

## Workflows

### Workflow 1: Startup Health Check (Background, Non-Blocking)

1. App starts; Tauri UI renders with profile list (badges show last-known persisted state from
   SQLite if available, or "Not checked yet" if MetadataStore unavailable or no cached result)
2. Immediately after UI is ready, a background async task begins
3. `ProfileStore::list()` enumerates all profile names
4. **[NEW]** If MetadataStore available: fetch `query_failure_trends(30)` and
   `query_last_success_per_profile()` results in a single batch before the per-profile loop
5. **[NEW]** If MetadataStore available: fetch all launcher drift states in a single batch query
6. For each profile:
   - a. `ProfileStore::load()` → if error → classify as Broken (load-error issue)
   - b. Call `validate_profile_health(&profile)` → collect `Vec<ProfileHealthIssue>`
   - c. Classify filesystem health status: Healthy / Stale / Broken / Unconfigured
   - d. **[NEW]** If MetadataStore available: enrich with failure trend signal → composite Degraded
     if applicable
   - e. **[NEW]** If MetadataStore available: attach launcher drift indicator if drift != Aligned
   - f. **[NEW]** Persist result to SQLite (keyed by `profile_id`); skip if MetadataStore
     unavailable
7. Background task completes → emits Tauri event `"profile-health-results"` with
   `Vec<ProfileHealthResult>`
8. Frontend receives event, updates health badges, displays summary count
9. If any profiles are Broken → show startup banner notification (non-modal, dismissable)
10. If zero profiles → health panel hidden or shows "No profiles"

### Workflow 2: Manual Health Check ("Re-check All")

1. User clicks "Re-check All" in the health panel
2. Panel shows loading state ("Checking profiles...")
3. Frontend invokes `batch_validate_profiles` Tauri command
4. Same logic as startup scan, returns `Vec<ProfileHealthResult>` synchronously (or via callback)
5. **[NEW]** Persists fresh results to SQLite; overwrites cached results
6. Frontend replaces all results; badges and summary count update
7. If issues persist → banner re-shown (or summary refreshed)

### Workflow 3: Viewing Remediation Detail

1. User clicks a broken/stale profile's badge or expand arrow
2. `CollapsibleSection` (existing component) expands showing per-issue list
3. Issues sorted: Broken issues first, then Stale
4. Each issue shows: human-readable field label, issue message, remediation help text
5. **[NEW]** If Degraded: shows "Recent launch failures" sub-section with failure count and
   failure_modes from `query_failure_trends()` result
6. **[NEW]** If launcher drift: shows "Launcher drift" sub-section with drift state and
   "Re-export launcher" CTA
7. **[NEW]** Shows last-success timestamp if available: "Last successful launch: 3 days ago"
8. "Open Profile" CTA navigates to `ProfileEditor` for that profile

### Workflow 4: Auto-Revalidate on Save

1. User edits a profile in `ProfileEditor` and saves (`invoke('save_profile')` resolves)
2. Frontend immediately calls single-profile health check: `invoke('validate_profile', { name })`
3. **[NEW]** Tauri command layer invalidates the SQLite health cache row for this `profile_id`
   before running the check
4. Result updates only that profile's badge in-place — no full batch re-scan
5. **[NEW]** Fresh result persisted to SQLite after check completes

### Workflow 5: Profile Load Failure

1. `ProfileStore::load()` returns error
2. Profile classified **Broken**: single load-error issue: "Profile file could not be read"
3. Remediation: "Try deleting and re-creating this profile."
4. Must not abort the batch scan

### **[NEW]** Workflow 6: Filtering by Collection / Favorites

1. User selects "Show favorites only" or picks a collection from the filter dropdown in the health
   panel
2. Frontend calls `MetadataStore::list_favorite_profiles()` or
   `MetadataStore::list_profiles_in_collection(collection_id)` via a Tauri query command
3. Health view filters to only show matching profiles
4. Summary count updates to reflect filtered subset ("2 of 5 favorites have issues")
5. If MetadataStore unavailable: filter controls are hidden or disabled

---

## Domain Model

### Key Entities

**`GameProfile`** (existing — `crates/crosshook-core/src/profile/models.rs`)
Source of truth for health checks — validated directly, no conversion to `LaunchRequest`.

**`ProfileHealthStatus`** (existing shape, with Degraded added as a display concept)

```
enum ProfileHealthStatus { Healthy, Stale, Broken }
// Degraded is a composite property, not a 4th enum variant.
// A ProfileHealthResult.is_degraded: bool flag carries the launch-history signal.
```

**`ProfileHealthIssue`** (new — unchanged from original spec)

```
struct ProfileHealthIssue {
    field: String,         // human-readable field label ("Game Executable", "Proton Path")
    message: String,       // what's wrong
    help: String,          // remediation suggestion
    kind: HealthIssueKind, // NotConfigured | Missing | Inaccessible | WrongType
}

enum HealthIssueKind { NotConfigured, Missing, Inaccessible, WrongType }
```

**`ProfileHealthResult`** (new — expanded from original spec)

```
struct ProfileHealthResult {
    name: String,
    profile_id: Option<String>,        // [NEW] stable UUID from MetadataStore; None if unavailable
    status: ProfileHealthStatus,
    issues: Vec<ProfileHealthIssue>,
    is_degraded: bool,                 // [NEW] true if path-Healthy but launch-failing (>= 2 failures, 0 successes)
    failure_count: u32,                // [NEW] failure count from query_failure_trends (0 if unavailable)
    failure_modes: Vec<String>,        // [NEW] failure mode strings (e.g., ["crash", "timeout"])
    never_launched: bool,              // [NEW] true if no launch_operations rows exist for this profile (absence of data, not a problem signal)
    last_success_at: Option<String>,   // [NEW] RFC3339 timestamp of last successful launch; None if never or unavailable
    launcher_drift: Option<DriftState>,// [NEW] drift state from launchers table; None if no launcher exported
    checked_at: Option<String>,        // [NEW] RFC3339 timestamp when this result was computed
}
```

**`HealthCacheRow`** (new — SQLite persistence for health results)

Persisted to a new `profile_health_cache` table, keyed by `profile_id`. Columns: `profile_id`,
`status`, `is_degraded`, `failure_count`, `launcher_drift`, `checked_at`. Not a full copy of
issues — issues are recomputed on next scan; the cache only stores the summarized badge state for
fast UI hydration on startup.

### State Transitions

```
Initial (UI rendered, scan not started)
    │  MetadataStore available → show persisted badge from health_cache
    │  MetadataStore unavailable → show "Not checked yet"
    ▼
Scanning...
    │ scan completes → emits "profile-health-results"
    ▼
Healthy | Stale | Broken | Degraded (per profile, in frontend state + SQLite cache)
    │
    ├── profile saved/renamed/deleted → SQLite cache row invalidated → badge shows "Checking..."
    │     → single-profile re-check automatically triggered
    │
    ├── "Re-check All" clicked → Scanning... (full re-check)
    │
    ├── persisted result > 7 days old → badge shown with "Last checked N days ago" note
    │
    └── notification dismissed → dismissed until next app launch
         └── next app launch → show persisted badge immediately → Scanning... (re-checks)
```

---

## Existing Codebase Integration

### Directly Reusable

| Asset                                                       | Location                           | How it's used                                                                           |
| ----------------------------------------------------------- | ---------------------------------- | --------------------------------------------------------------------------------------- |
| `ValidationError::help()`                                   | `request.rs:297`                   | Remediation text for all path errors — no new copy needed                               |
| `ProfileStore::list()`                                      | `toml_store.rs:136`                | Enumerates all profiles for batch check                                                 |
| `ProfileStore::load()`                                      | `toml_store.rs:100`                | Loads each profile                                                                      |
| `CompatibilityBadge` CSS                                    | `CompatibilityViewer.tsx:76`       | `crosshook-status-chip crosshook-compatibility-badge--{rating}` maps to health statuses |
| `CollapsibleSection`                                        | `components/ui/CollapsibleSection` | Used for expandable issue detail panels                                                 |
| `is_stale` concept                                          | `launcher_store.rs:42`             | Analogous pattern: path-existence check on stored data                                  |
| `validate_launch` IPC pattern                               | `commands/launch.rs:31`            | Tauri command pattern to follow                                                         |
| **[NEW]** `MetadataStore::query_failure_trends(days)`       | `metadata/mod.rs:437`              | Batch fetch failure trends for all profiles in one query                                |
| **[NEW]** `MetadataStore::query_last_success_per_profile()` | `metadata/mod.rs:401`              | Batch fetch last-success timestamps for all profiles                                    |
| **[NEW]** `MetadataStore::lookup_profile_id()`              | `metadata/mod.rs:125`              | Look up stable UUID by profile filename for cache key                                   |
| **[NEW]** `MetadataStore.available` flag                    | `metadata/mod.rs:29`               | Fail-soft gate: all metadata enrichment is conditional on this flag                     |
| **[NEW]** `DriftState` enum                                 | `metadata/models.rs:122`           | Maps launcher drift states to health indicator display logic                            |
| **[NEW]** `MetadataStore::list_favorite_profiles()`         | `metadata/mod.rs:323`              | Filter health view by favorites                                                         |
| **[NEW]** `MetadataStore::query_most_launched(limit)`       | `metadata/mod.rs:362`              | Prioritize health issue ordering for frequently-used profiles                           |
| **[NEW]** `profiles.source` column                         | `metadata/profile_sync.rs:26`      | Detect community-imported profiles (`source = 'import'`) for contextual import note    |
| **[NEW]** `MetadataStore::list_profiles_in_collection()`    | `metadata/mod.rs:300`              | Filter health view by collection                                                        |

### What NOT to Reuse

- **`validate_all(request: &LaunchRequest)`**: Includes launch-configuration checks out of scope
  for health checks. Health validation must NOT route through `LaunchRequest`.
- **`validate_name()` / `validate_path_traversal()`**: Not needed in health checks for profiles
  already loaded via `ProfileStore::list()`.

### Gaps Requiring New Code

1. **`validate_profile_health(profile: &GameProfile, method: &str) -> Vec<ProfileHealthIssue>`** —
   New function in `crosshook-core/src/profile/health.rs`. Path-existence + type + permission
   checks directly on `GameProfile` fields, method-aware. Reuses `ValidationError::help()` for
   remediation text.

2. **`batch_validate_profiles` Tauri command** — In `src-tauri/src/commands/`, manages
   `State<ProfileStore>` and `State<MetadataStore>`, iterates profiles, calls
   `validate_profile_health()`, enriches with metadata signals, classifies status, persists cache,
   returns `Vec<ProfileHealthResult>`.

3. **`validate_profile(name: String)` Tauri command** — Single-profile variant. Called after
   `save_profile` resolves to update badge in-place. Invalidates SQLite cache row before re-check.
   Returns `ProfileHealthResult` for one profile.

4. **`ProfileHealthStatus`, `ProfileHealthResult`, `ProfileHealthIssue`, `HealthIssueKind` types**
   — New Serde types in `crosshook-core/src/profile/health.rs`. `ProfileHealthResult` gains new
   fields per the revised model above.

5. **`profile_health_cache` SQLite table** — New table (schema migration) to persist summarized
   health results keyed by `profile_id`. Used to populate UI before the scan completes on startup.

6. **Startup async task** — Non-blocking background task spawned from `lib.rs` Tauri setup, emits
   `"profile-health-results"` Tauri event on completion.

7. **Frontend health panel** — Summary count + per-profile badge list + `CollapsibleSection` detail
   panels. Extended with: last-success timestamp, failure count sub-section, launcher drift
   indicator, collection/favorites filter.

---

## Security Constraints

- Health check is **strictly read-only** — checks existence/metadata only, never reads binary
  content of game/trainer executables.
- Community-tap profiles may reference any absolute paths. The health check confirms whether those
  paths exist on disk. Acceptable within the local single-user threat model.
- Empty-string paths are treated as `NotConfigured`, NOT as `Missing` — prevents false-positive
  "broken" classification for optional fields.
- **[NEW]** Persisted health results (in `profile_health_cache`) contain path existence booleans
  and failure counts — no path strings are stored in the cache row itself. Actual path values
  remain in TOML only. This limits information leakage from the SQLite file.
- **[NEW]** The `diagnostic_json` field in `launch_operations` is already capped at 4 KiB
  (`MAX_DIAGNOSTIC_JSON_BYTES`). When reading failure modes for health enrichment, only the
  `failure_modes` string column from `FailureTrendRow` is consumed — not the raw JSON blob.
- **[NEW] Path sanitization before IPC:** All path strings surfaced in `ProfileHealthIssue.message`
  and `ProfileHealthIssue.help` fields must be passed through `sanitize_display_path()`
  (`src-tauri/src/commands/shared.rs:20`) before the `ProfileHealthResult` is serialized across
  the Tauri IPC boundary. This replaces absolute paths with `~/`-prefixed display paths,
  consistent with the existing pattern in `commands/launch.rs`. Applies to both
  `batch_validate_profiles` and `validate_profile` Tauri commands.
- **[NEW] `diagnostic_json` must not be surfaced via health IPC:** The health enrichment layer
  reads only the promoted `severity` and `failure_mode` columns from `launch_operations` (via
  `FailureTrendRow`). The raw `diagnostic_json` blob must never be read or forwarded by health
  commands — it contains free-text summaries and path references that require the full launch
  diagnostic sanitization pipeline.

---

## Success Criteria (Revised from GitHub Issue #38)

| Criterion                                                            | Implementation path                                                                                     |
| -------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------- |
| All saved profiles can be validated in batch                         | `ProfileStore::list()` + `validate_profile_health()` loop                                               |
| Each profile shows health status indicator (healthy/stale/broken)    | `ProfileHealthStatus` enum + `crosshook-compatibility-badge--{rating}` CSS                              |
| Broken paths identified with specific remediation suggestions        | `ProfileHealthIssue.help` — reused from `ValidationError::help()` + new Inaccessible text               |
| Health check runs on app startup                                     | Non-blocking async task post-UI-render, emits `"profile-health-results"`                                |
| Health check can be triggered manually                               | `batch_validate_profiles` Tauri command + "Re-check All" button                                         |
| **[NEW]** Profiles with persistent launch failures are flagged       | `query_failure_trends(30)` enrichment; Degraded composite signal; amber badge with failure count detail |
| **[NEW]** Last successful launch date shown per profile              | `query_last_success_per_profile()` result attached to `ProfileHealthResult`                             |
| **[NEW]** Exported launcher drift surfaced in health view            | `launchers.drift_state` fetched per profile; shown as secondary indicator                               |
| **[NEW]** Health results survive app restart (last-known state)      | `profile_health_cache` table; populated on scan; hydrates UI before scan completes                      |
| **[NEW]** Feature degrades gracefully when MetadataStore unavailable | All metadata enrichment gated on `MetadataStore.available`; filesystem-only scan always proceeds        |

---

## Open Questions

1. **`profile_health_cache` schema placement**: Should health cache live in `metadata.db`
   alongside other MetadataStore tables, or be a separate concern? Recommendation: same DB, new
   table — consistent with the existing pattern of all SQLite state in `metadata.db`.

2. **Failure threshold tuning**: The 2-failures-in-30-days threshold for "Degraded" is an initial
   default. Should it be user-configurable via Settings, or fixed? Recommendation: fixed for now,
   revisit in a later phase.

3. **`Unconfigured` profiles notification**: Brand-new all-empty profiles show as Broken (Required
   errors). Should the startup banner explicitly exclude profiles with ALL-empty paths? Recommendation:
   yes — detect "all NotConfigured" as `Unconfigured` variant and use badge-only, no banner.

4. **DLL injection path validation**: `InjectionSection.dll_paths` are silently skipped in
   this phase. Recommend including in the next phase.

5. **Opt-in startup health check**: Should background health scanning be opt-in via settings (for
   users on very slow storage)? Since it's async and non-blocking, always-on may be acceptable.
   Open for UX/recommendations input.

6. **`Warning` severity for optional paths**: This phase uses only Broken/Stale for issue severity.
   Should missing-but-optional trainer path produce a Warning state? Defer to next phase.

7. **Launcher drift as optional schema migration**: Adding `profile_health_cache` requires a schema
   migration in `metadata/migrations.rs`. The migration should be additive and non-destructive.
   Confirm migration numbering with tech-designer.

8. **`failure_modes` string format**: `query_failure_trends()` returns `GROUP_CONCAT(DISTINCT failure_mode)`
   which is a comma-separated string. The health layer must split and parse this, or the SQL query
   should be adjusted to return a JSON array. Recommend normalizing to `Vec<String>` in the Rust
   layer before IPC serialization.

9. **[NEW] Startup banner scoping — all profiles vs. favorites/pinned only**: The current rule
   is that the startup banner fires when any Broken profile is found across ALL profiles. With
   collections and favorites now available, should the startup banner scope to favorites/pinned
   profiles only (reducing noise for users with many low-priority profiles), or always scan all?
   The health panel filter (Workflow 6) is a UI concern; the banner is a separate notification
   policy. Recommend: banner always fires for all Broken profiles regardless of collection
   membership — collections are a viewing filter, not a priority gate. Open for UX input.
