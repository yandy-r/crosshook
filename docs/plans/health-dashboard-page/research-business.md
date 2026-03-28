# Health Dashboard Page — Business Research

## Executive Summary

The Health Dashboard Page is a dedicated read-only diagnostics surface for CrossHook that consolidates all profile health signals into a single top-level view. The feature is a pure frontend addition: all required backend data already exists through Phase A/B/D health work. The business value is to shift users from reactive (discovering broken profiles only at launch time) to proactive (reviewing aggregate health before a play session), with special emphasis on Steam Deck users who need a controller-navigable diagnostic view.

The feature is designed for phased delivery:

- **Phase 1 — Triage at a Glance**: Sidebar route, summary cards, flat profile list with status badges, Re-check All. Enough to answer "is anything broken?"
- **Phase 2 — Power-User Filtering**: Sortable/filterable table, issue count column, single-profile re-check, Fix navigation. Enough to work through a backlog of broken profiles efficiently.
- **Phase 3 — Polish and Pattern Identification**: Trend arrows, failure history, launcher drift, community import annotation, favorites prominence, cached snapshot "checking…" state.

---

## User Stories

### Phase 1 — MVP: Triage at a Glance

These stories are essential for the first working version. They answer the core question: "do I have any broken profiles right now, and where are they?"

**US-1.1** As any user, I want to open a Health tab and immediately see how many profiles are healthy, stale, and broken as color-coded summary counts, so I can assess overall fleet health in one glance.

**US-1.2** As any user, I want to see a flat list of all profiles with their current health status badge, so I know which profiles have problems without opening each one in the editor.

**US-1.3** As any user, I want a 'Re-check All' button that rescans all profiles and updates the view, so I can get a fresh result after making fixes.

**US-1.4** As a Steam Deck user, I want to reach the Health tab via L1/R1 sidebar cycling and navigate the profile list with D-pad, so I can triage health without a mouse.

**US-1.5** As any user when a profile is broken or stale, I want a 'Fix' action that takes me directly to that profile in the editor, so I don't have to find it manually in the dropdown.

**US-1.6** As any user, I want the page to show a meaningful empty state when there are no profiles and a positive confirmation state when all profiles are healthy, so the absence of problems is clearly communicated.

### Phase 2 — Power-User Features: Filtering and Pattern Identification

These stories are valuable for users with multiple profiles. They answer: "which specific profiles need attention and what's wrong with them?"

**US-2.1** As a user with many profiles, I want to sort the profile table by status (broken first), name, and issue count, so I can prioritize which profiles to fix.

**US-2.2** As a user with many profiles, I want to filter the table to show only broken or only stale profiles, so I can focus on a specific problem tier without scrolling past healthy entries.

**US-2.3** As a user investigating a broken profile, I want to expand a table row to see the specific issues (field, path, message, remediation), so I understand what to fix before navigating to the editor.

**US-2.4** As a user, I want to re-check a single profile inline without triggering a full re-scan, so I can verify a quick fix without waiting for all profiles to validate.

**US-2.5** As a user, I want an issue breakdown panel that categorizes problems by type (missing executables, missing Proton paths, inaccessible directories), so I can spot systemic issues affecting multiple profiles at once (e.g. a Proton version was deleted).

### Phase 3 — Polish: Trends, History, and Contextual Signals

These stories add diagnostic depth for users who want to understand patterns over time, not just current state.

**US-3.1** As a returning user, I want trend arrows (got worse / got better) on each profile row based on the last cached snapshot, so I can immediately see what changed since my last session.

**US-3.2** As a user who has been launching games, I want a 'Recent Failures' panel showing profiles with launch failures in the last 30 days, so I know which profiles have a history of problems even when currently passing the path check.

**US-3.3** As a user who has exported launchers, I want a launcher drift indicator per profile (missing / moved / stale), so I know which launchers need re-exporting after profile changes.

**US-3.4** As a user who imported community profiles, I want broken/stale community imports to be annotated with "paths may need adjustment for your system", so I understand why those profiles are likely broken.

**US-3.5** As a user who has favorited profiles, I want favorited profiles to be visually flagged in the table and sorted prominently, so my most-used profiles are easy to spot even in a long list.

**US-3.6** As a Steam Deck user, I want 'Re-check All' mapped to the Y button so I can trigger a rescan from any row without moving focus to the button.

### Anti-Users (Not Served by Any Phase)

- The dashboard never modifies profile data. 'Fix' actions are navigation shortcuts only — no inline repair.
- No bulk operations (bulk delete, bulk fix). Each 'Fix' is single-profile.

---

## Business Rules

Rules are tagged with the phase in which they become relevant: **(P1)**, **(P2)**, or **(P3)**.

### Core Rules

**BR-01: Read-Only Surface** (P1)
The Health Dashboard never modifies profile data, launch metadata, or any stored state. 'Fix' actions are navigation shortcuts that open the Profile Editor pre-selected on the affected profile — they do not perform any repair inline.

**BR-02: Data is Already Available** (P1)
All health data required for the dashboard exists in the current backend:

- `batch_validate_profiles` → `EnrichedHealthSummary` (aggregate counts + per-profile reports with metadata; single unpaginated call — all profiles returned at once)
- `get_cached_health_snapshots` → instant badge status before live scan completes
- `get_profile_health` → single-profile re-check
- `useProfileHealth` hook already manages fetch lifecycle, caching, trends, and stale detection

No new Tauri commands or backend work is required for any phase.

**BR-03: Status Hierarchy** (P1)
The three statuses are ranked: `Broken` (2) > `Stale` (1) > `Healthy` (0). This ranking drives:

- Color coding: Broken = red/danger, Stale = orange/warning, Healthy = green/success
- Default sort order: Broken first (P1 list order, P2 sortable column)
- Summary card visual prominence

**BR-04: Issue Severity Classification** (P2 — needed when issues are expanded)
Issues are classified at three severities:

- `Error` (broken): required field missing or misconfigured — prevents launch
- `Warning` (stale): path was valid but is now missing from disk — launch will fail
- `Info`: optional field path is missing — launch may succeed but is incomplete

**BR-05: Failure Trend Window** (P3)
Failure trend data covers a rolling 30-day window (`FAILURE_TREND_WINDOW_DAYS = 30`). The `HealthBadge` component shows a failure trend indicator when `failure_count_30d >= 2`. The Recent Failures panel should use `failure_count_30d > 0` as its inclusion threshold.

**BR-06: Trend Direction Computation** (P3)
`TrendDirection` is derived by comparing the current live `HealthStatus` against the cached snapshot status from `CachedHealthSnapshot.status`. The direction is `got_worse`, `got_better`, or `unchanged`. A `null` trend means no cached baseline exists. Only `got_worse` and `got_better` render visible arrows — `unchanged` and `null` are silent.

**BR-07: Snapshot Staleness** (P3)
Cached snapshots older than 7 days are considered stale (`STALE_THRESHOLD_DAYS = 7`). A stale snapshot should be visually demoted (muted note) and treated as advisory for trend display, not authoritative.

**BR-08: Launch Method Scoping** (P2 — shown in table column, P2 issue breakdown)
Health checks are method-aware. Required fields differ by launch method:

- `steam_applaunch`: requires `steam.compatdata_path` (dir) and `steam.proton_path` (executable)
- `proton_run`: requires `runtime.prefix_path` (dir) and `runtime.proton_path` (executable)
- `native`: no method-specific required paths beyond `game.executable_path`

**BR-09: Community Import Annotation** (P3)
When `ProfileHealthMetadata.is_community_import` is `true` and the profile is broken or stale, the UI adds a contextual note: "This profile was imported from a community tap — paths may need adjustment for your system." This is already implemented in `ProfilesPage` and should be replicated in the Health Dashboard per-profile detail expansion.

**BR-10: Launcher Drift States** (P3)
`launcher_drift_state` serializes from the Rust `DriftState` enum (`snake_case`). Full value set and UI treatment:

- `missing`: exported launcher file not found — show re-export warning
- `moved`: launcher file has moved — show re-export warning
- `stale`: launcher exists but may be outdated — show re-export warning
- `aligned`: launcher exists and matches the profile — no action needed, do not surface to user
- `unknown`: drift state could not be determined — treat as no indicator (silent)
- `null` / absent in metadata: no launcher has been exported for this profile — no indicator

**BR-11: Favorites Visibility** (P3)
`ProfileHealthMetadata.is_favorite` is `true` for profiles the user marked as favorites. Favorited profiles should be visually flagged (star indicator) and may be sorted prominently. This is purely cosmetic — favorites are not exempt from health rules.

**BR-12: Metadata May Be Absent** (P1 — must handle from day one)
`ProfileHealthMetadata` is `null` when `MetadataStore` is unavailable (e.g. SQLite not initialized). Phase 1 renders only core health data. Phases 2 and 3 columns/panels that depend on metadata fields must degrade to "N/A" or hide when metadata is null. The page must never crash on null metadata.

**BR-13: Re-Check All** (P1)
The 'Re-check All' button triggers `batchValidate()` from `useProfileHealth`. While validating, the button is disabled and shows loading state. The hook manages the batch loading state via `state.status === 'loading'`.

**BR-14: Single Profile Re-Check** (P2)
Each profile row can trigger `revalidateSingle(name)` which updates only that profile's entry in the summary state via the `single-complete` reducer action. The overall counts are recalculated via `countProfileStatuses`.

**BR-15: Issue Category Aggregation is Client-Side** (P2)
The backend provides no pre-aggregated issue counts by category. The P2 issue breakdown panel must be computed client-side by iterating `summary.profiles` and grouping `issues` by their `field` prefix:

- `game.executable_path` → "Missing executable"
- `trainer.path` → "Missing trainer"
- `injection.dll_paths[N]` → "Missing DLL"
- `steam.proton_path` / `runtime.proton_path` → "Missing/invalid Proton path"
- `steam.compatdata_path` / `runtime.prefix_path` → "Inaccessible directory"
- `steam.launcher.icon_path` / `runtime.working_directory` → "Missing optional path"

This grouping logic belongs in a utility function (e.g. `src/utils/health.ts`) alongside the existing `countProfileStatuses` helper, not inline in the component.

### Edge Cases

**EC-01: Zero Profiles** (P1)
When `HealthCheckSummary.total_count === 0`, the dashboard shows an empty-state message instead of summary cards and list/table.

**EC-02: All Profiles Healthy** (P1)
When `broken_count === 0` and `stale_count === 0`, the page renders but issues breakdown and recent failures panels are empty-state. The green summary card is the only prominent signal.

**EC-03: MetadataStore Unavailable** (P1 — must handle from day one)
When `metadata` is `null` on all profiles, all metadata-dependent columns and panels are absent or show "N/A". The page remains functional with core health data.

**EC-04: Profile Load Failure (Malformed TOML)** (P1)
A profile that cannot be parsed appears as `Broken` with `launch_method = ""` and a single issue: `"Profile could not be loaded: ..."`. Renders with broken status; fix action navigates to the editor.

**EC-05: Profile Enumeration Failure** (P1)
If `ProfileStore.list()` fails, `batch_check_health` returns a single sentinel entry with name `"<unknown>"`. Detect this (empty `field` + empty `path` on the single issue of an `<unknown>` named entry) and render a system-level error state rather than a normal broken profile row.

**EC-06: Stale Snapshot With No Live Data** (P1)
If `useProfileHealth` is still loading and only cached snapshots are available, display cached data with a "cached / checking…" visual indicator, not a blank screen.

**EC-07: Trend With No Prior Snapshot** (P3)
`TrendDirection` is `null` when there is no cached snapshot baseline. Trend arrows must only render for `got_worse` or `got_better`.

---

## Workflows

### Primary Workflow: Opening the Health Dashboard

1. User navigates to the Health tab (new sidebar item)
2. Page mounts — `useProfileHealth` hook runs:
   - Immediately loads `CachedHealthSnapshot[]` from `get_cached_health_snapshots`
   - Listens for `profile-health-batch-complete` Tauri event (emitted by background startup scan)
   - Falls back to `batchValidate()` after 700ms if no startup event received
3. **P1**: Cached snapshots render instantly as badge-only status in summary cards and list
4. Live scan completes — summary cards and list/table update with enriched data
5. **P3**: Trend arrows and failure history populate from metadata

### Secondary Workflow: Re-Check All

1. User presses 'Re-check All' button (P1: button only; P3: also Y gamepad button)
2. Button disables, shows "Checking…"
3. `batchValidate()` is called
4. On completion, all counts, badges, and (P3) trend arrows update
5. Button re-enables

### Tertiary Workflow: Fix a Profile

1. User identifies a broken/stale profile in the list/table **(P1)** or recent failures panel **(P3)**
2. User activates the 'Fix' action on that row
3. Navigation fires `onNavigate('profiles')` with that profile pre-selected
4. Profile Editor opens with that profile loaded

### Controller Navigation Workflow (Steam Deck)

- **P1**: L1/R1 to reach Health tab; D-pad up/down to navigate profile list; A to activate Fix
- **P3**: Y button (index 3) to trigger Re-check All from any position

### Error Recovery Workflow

1. If `batchValidate()` returns an error, a visible error message renders
2. A 'Retry' button is available
3. Cached snapshot data (if any) remains displayed during error state

---

## Domain Model

### Key Entities

**ProfileHealthReport** (from `crosshook-core/src/profile/health.rs`)

- `name: String` — profile identifier (filename stem)
- `status: HealthStatus` — `Healthy | Stale | Broken`
- `launch_method: String` — resolved method string
- `issues: Vec<HealthIssue>` — per-field issues found
- `checked_at: String` — ISO 8601 UTC timestamp

**HealthIssue** (per-field problem)

- `field: String` — dot-path to field (e.g. `game.executable_path`)
- `path: String` — actual filesystem path (sanitized: `~` replaces home dir)
- `message: String` — human-readable description
- `remediation: String` — suggested fix step
- `severity: HealthIssueSeverity` — `Error | Warning | Info`

**ProfileHealthMetadata** (enrichment from MetadataStore)

- `profile_id: Option<String>` — UUID assigned at profile creation
- `last_success: Option<String>` — ISO 8601 of last successful launch
- `failure_count_30d: i64` — failure count in rolling 30-day window
- `total_launches: i64` — lifetime launch count
- `launcher_drift_state: Option<DriftState>` — `unknown | aligned | missing | moved | stale | null` (UI action only for `missing`, `moved`, `stale`)
- `is_community_import: bool`
- `is_favorite: bool`

**EnrichedProfileHealthReport** (IPC output of `batch_validate_profiles`)

- Flattened `ProfileHealthReport` fields
- `metadata: Option<ProfileHealthMetadata>`

**EnrichedHealthSummary** (root IPC response)

- `profiles: Vec<EnrichedProfileHealthReport>`
- `healthy_count / stale_count / broken_count / total_count: usize`
- `validated_at: String`

**CachedHealthSnapshot** (from `get_cached_health_snapshots`)

- `profile_id: String`
- `profile_name: String`
- `status: HealthStatus` (as string)
- `issue_count: i64`
- `checked_at: String`

### State Transitions

```
Profile Health Status State Machine:
  (unconfigured) → Broken       [required field empty]
  (missing path) → Stale        [path was set but file no longer exists]
  (wrong type)   → Broken       [path exists but is not file/dir/executable as required]
  (permission denied) → Broken  [path exists but inaccessible]
  (all fields ok) → Healthy

Status Ranking (for sort and UI prominence):
  Broken (2) > Stale (1) > Healthy (0)

Snapshot Status Transitions (TrendDirection) — Phase 3 only:
  cached:Healthy → live:Stale    → got_worse
  cached:Healthy → live:Broken   → got_worse
  cached:Stale   → live:Broken   → got_worse
  cached:Broken  → live:Stale    → got_better
  cached:Broken  → live:Healthy  → got_better
  cached:Stale   → live:Healthy  → got_better
  same           → same          → unchanged (no arrow)
  no snapshot    → any           → null (no arrow)
```

### Relationships

- One `AppRoute` ('health') → One `HealthDashboardPage` component
- `HealthDashboardPage` consumes one `useProfileHealth` hook instance
- One `useProfileHealth` hook manages one `HealthCheckSummary` (batch) + per-profile updates
- Each `EnrichedProfileHealthReport` optionally carries one `ProfileHealthMetadata`
- Each profile row 'Fix' action triggers navigation to `ProfilesPage` with pre-selection

---

## Existing Codebase Integration

### Reusable Hook — `useProfileHealth`

`/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/hooks/useProfileHealth.ts`

Primary data source for all three phases. Key exports used:

- P1: `summary`, `loading`, `error`, `cachedSnapshots`, `batchValidate`
- P2: `healthByName`, `revalidateSingle`
- P3: `trendByName`, `staleInfoByName`

### Reusable Component — `HealthBadge`

`/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/components/HealthBadge.tsx`

Used from P1. Renders status chip + (P3) failure trend indicator + (P3) trend arrow. The `metadata` and `trend` props can be omitted in P1.

### Routing — `AppRoute` and `ContentArea`

`/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/components/layout/Sidebar.tsx`
`/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/components/layout/ContentArea.tsx`

Must be updated in P1: add `'health'` to `AppRoute`, `VALID_APP_ROUTES`, `SIDEBAR_SECTIONS`, and the `ContentArea` route switch.

### Page Structure Pattern

`/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/components/pages/CommunityPage.tsx`

Follow the same pattern: `PageBanner` header + content component. New illustration SVG needed in `PageBanner.tsx`.

### Gamepad Navigation — `useGamepadNav`

`/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/hooks/useGamepadNav.ts`

P1 D-pad navigation is automatic — the content zone handles it. P3 Y button requires a page-level gamepad effect (Y = button index 3, not currently handled by the hook).

### Existing Issue Display Pattern — `ProfilesPage`

`/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/components/pages/ProfilesPage.tsx` (lines 501–563)

The per-profile issue list, drift warning, community import note, last-success, and failure count display patterns already exist here. Reference directly for P2 row expansion and P3 metadata annotations.

### Type Definitions

`/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/types/health.ts`

---

## Success Criteria by Phase

### Phase 1 — Done When

1. Health tab is reachable from the sidebar
2. Summary cards show correct color-coded counts (total / healthy / stale / broken)
3. All profiles are listed with their current status badge, defaulting to broken-first order
4. 'Re-check All' triggers `batchValidate()`, disables during scan, re-enables on completion
5. 'Fix' on a row navigates to Profile Editor with that profile pre-selected
6. Empty state renders when zero profiles exist
7. Positive confirmation state renders when all profiles are healthy
8. Page renders without crashing when `metadata` is `null`
9. Cached snapshot data displays immediately on load (no blank-screen wait)
10. D-pad navigates the profile list; A activates Fix

### Phase 2 — Done When

11. Table is sortable by status, name, and issue count
12. Table can be filtered to show only broken / only stale / all profiles
13. Expanding a row shows the specific issues with field, path, message, and remediation
14. Single-profile re-check button updates only that row without full re-scan
15. Issue breakdown panel categorizes problems by type across all profiles
16. Launch method is shown as a column in the table

### Phase 3 — Done When

17. Trend arrows appear on rows where status changed since the last cached snapshot
18. Recent Failures panel lists profiles with `failure_count_30d > 0`
19. Launcher drift indicator appears per row when drift state is non-null
20. Community import annotation appears in row detail for broken/stale imported profiles
21. Favorites are visually flagged with a star indicator
22. Y gamepad button triggers Re-check All

---

## Open Questions

1. **Sidebar section placement**: Should 'Health' be in the 'Game' section (alongside Profiles and Launch) or a standalone 'Diagnostics' section? Recommend 'Game' for P1 — it covers game profiles directly.

2. **Fix navigation pre-selection** (P1 blocker): `ProfileContext` has no external pre-selection mechanism. Options: (a) extend `ProfileContext` with a pending-selection signal, (b) lift selection to `AppShell` state and pass it down, (c) use a simple module-level ref as a one-shot signal. Option (c) is the lowest-risk change for P1.

3. **Y button (re-check) mapping** (P3): Y is button index 3. Current `useGamepadNav` has no handler for it. A page-level `useEffect` polling `navigator.getGamepads()` is the least invasive approach and avoids changing the shared hook.

4. **Table row focusability** (P2): Full-row tabIndex + `role="row"` is more Steam Deck friendly. Action buttons within the row remain clickable. ARIA semantics need care (`role="grid"` or `role="table"` with `role="row"` + `role="gridcell"`).

5. **Recent failures threshold** (P3): Feature spec says "last 30 days." Recommend `failure_count_30d > 0` for panel inclusion (show all profiles with any failure), matching the data already available. The `>= 2` threshold in `HealthBadge` is for the inline failure count indicator only.

6. **Launcher drift as column vs. panel** (P3): `launcher_drift_state` is per-profile metadata — a table column is more coherent than a dedicated panel. A column with a drift icon + tooltip satisfies the spec without adding a separate scroll section.

7. **Community import as filter vs. panel** (P3): `is_community_import` is a per-profile boolean — a table filter chip ("Show: Community imports only") is cleaner than a separate panel and avoids duplicating the table.
