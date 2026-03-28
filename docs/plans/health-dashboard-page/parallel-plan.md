# Health Dashboard Page Implementation Plan

The Health Dashboard is a frontend-only read-only diagnostics page consuming existing Tauri IPC commands (`batch_validate_profiles`, `get_profile_health`, `get_cached_health_snapshots`) via the `useProfileHealth` hook. Implementation creates one new page component (`HealthDashboardPage.tsx`), modifies five existing files for routing under a new "Dashboards" sidebar section, and ships in three independently releasable phases with one pre-phase type fix. No Rust changes, no new npm dependencies — all rendering uses existing `HealthBadge`, `CollapsibleSection`, `PageBanner` components and `crosshook-*` CSS classes.

## Critically Relevant Files and Documentation

- src/crosshook-native/src/hooks/useProfileHealth.ts: Primary data hook — provides summary, loading, error, healthByName, trendByName, staleInfoByName, cachedSnapshots, batchValidate, revalidateSingle
- src/crosshook-native/src/types/health.ts: All TypeScript health types — EnrichedHealthSummary, EnrichedProfileHealthReport, ProfileHealthMetadata, HealthIssue, CachedHealthSnapshot
- src/crosshook-native/src/components/layout/Sidebar.tsx: AppRoute type union (line 12), SIDEBAR_SECTIONS (line 32), ROUTE_LABELS (line 53)
- src/crosshook-native/src/components/layout/ContentArea.tsx: Route-to-page switch (lines 34-51), onNavigate prop, exhaustive never check (line 48)
- src/crosshook-native/src/App.tsx: VALID_APP_ROUTES (line 14)
- src/crosshook-native/src/components/icons/SidebarIcons.tsx: SVG icon pattern (20x20 viewBox, stroke-based, defaults spread)
- src/crosshook-native/src/components/layout/PageBanner.tsx: Banner + illustration pattern (200x120 viewBox SVGs)
- src/crosshook-native/src/components/HealthBadge.tsx: Drop-in status badge with trend arrows, failure badges, ARIA labels
- src/crosshook-native/src/components/ui/CollapsibleSection.tsx: Collapsible details/summary panel — use for all secondary sections
- src/crosshook-native/src/components/pages/CommunityPage.tsx: Thin page wrapper pattern to follow
- src/crosshook-native/src/components/pages/ProfilesPage.tsx: Existing health badge display (lines 501-563), formatRelativeTime (line 22), enriched type cast pattern (line 507)
- src/crosshook-native/src/components/CompatibilityViewer.tsx: Filter/search with useDeferredValue (lines 110-140)
- src/crosshook-native/src/hooks/useGamepadNav.ts: Gamepad navigation — D-pad, A/B, L1/R1, zone model
- src/crosshook-native/src/hooks/useProfile.ts: useProfileContext() — provides selectProfile() for Fix navigation
- src/crosshook-native/src/utils/health.ts: countProfileStatuses() utility
- src/crosshook-native/src/styles/variables.css: CSS custom properties (--crosshook-color-success/warning/danger/accent)
- src/crosshook-native/src/styles/theme.css: crosshook-panel, crosshook-card, crosshook-status-chip, crosshook-compatibility-badge CSS classes
- docs/plans/health-dashboard-page/feature-spec.md: Resolved decisions, phased user stories, business rules BR-01 through BR-15, edge cases EC-01 through EC-07
- docs/plans/health-dashboard-page/research-technical.md: Architecture spec, data models, phase boundary contracts
- docs/plans/health-dashboard-page/research-ux.md: Dashboard layout, ARIA patterns, gamepad nav, loading states
- docs/plans/health-dashboard-page/research-security.md: XSS mitigation, CSP guidance, secure coding patterns

## Implementation Plan

### Phase 0: Pre-requisite Type Fix

#### Task 0.1: Update useProfileHealth invoke generics Depends on [none]

**READ THESE BEFORE TASK**

- src/crosshook-native/src/hooks/useProfileHealth.ts
- src/crosshook-native/src/types/health.ts

**Instructions**

Files to Modify

- src/crosshook-native/src/hooks/useProfileHealth.ts

Update the `invoke<>` generics in `useProfileHealth` to use the enriched types that match what the Rust backend actually returns:

1. Change `invoke<HealthCheckSummary>('batch_validate_profiles')` to `invoke<EnrichedHealthSummary>('batch_validate_profiles')`
2. Change `invoke<ProfileHealthReport>('get_profile_health', ...)` to `invoke<EnrichedProfileHealthReport>('get_profile_health', ...)`
3. Update `ProfileHealthState` interface — change `summary: HealthCheckSummary | null` to `summary: EnrichedHealthSummary | null`
4. Update the `ProfileHealthAction` union — change the `batch-complete` action's `summary` field from `HealthCheckSummary` to `EnrichedHealthSummary`, and the `single-complete` action's report field from `ProfileHealthReport` to `EnrichedProfileHealthReport`
5. Update the `reducer` function — its `state` parameter and action types must use the enriched types. The `batch-complete` case assigns `action.summary` to state; the `single-complete` case updates `healthByName` entries — both must now use enriched types
6. Update the hook's return type — `summary` should be `EnrichedHealthSummary | null`, `healthByName` should be `Record<string, EnrichedProfileHealthReport>`
7. Import the enriched types from `../../types/health`

This is a type-only change with no runtime impact — the data already flows as `EnrichedHealthSummary` from the backend. Verify by logging `summary.profiles[0]?.metadata` at runtime after the change — it should be an object (not undefined).

The existing `ProfilesPage.tsx` consumer currently casts at line 507 (`report as EnrichedProfileHealthReport`). After this fix, that cast becomes unnecessary but harmless — remove it in Phase 2 task 2.0 when touching ProfilesPage.

### Phase 1: MVP Dashboard

#### Task 1.1: Route wiring, sidebar icon, and page banner art Depends on [0.1]

**READ THESE BEFORE TASK**

- src/crosshook-native/src/components/layout/Sidebar.tsx
- src/crosshook-native/src/App.tsx
- src/crosshook-native/src/components/layout/ContentArea.tsx
- src/crosshook-native/src/components/icons/SidebarIcons.tsx
- src/crosshook-native/src/components/layout/PageBanner.tsx

**Instructions**

Files to Modify

- src/crosshook-native/src/components/layout/Sidebar.tsx
- src/crosshook-native/src/App.tsx
- src/crosshook-native/src/components/layout/ContentArea.tsx
- src/crosshook-native/src/components/icons/SidebarIcons.tsx
- src/crosshook-native/src/components/layout/PageBanner.tsx

All five edits must land atomically — `ContentArea.tsx:48` has a TypeScript exhaustive `never` check that causes a compile error if `AppRoute` grows without a matching switch case.

1. **Sidebar.tsx line 12**: Add `| 'health'` to the `AppRoute` union type
2. **Sidebar.tsx line 32-51**: Add a new "Dashboards" section to `SIDEBAR_SECTIONS` after the existing sections (before Community). Structure: `{ label: 'Dashboards', items: [{ route: 'health', label: 'Health', icon: HealthIcon }] }`
3. **Sidebar.tsx line 53-60**: Add `health: 'Health'` to `ROUTE_LABELS`
4. **App.tsx line 14-21**: Add `health: true` to `VALID_APP_ROUTES`
5. **ContentArea.tsx**: Import `HealthDashboardPage` from `../pages/HealthDashboardPage`, add `case 'health': return <HealthDashboardPage onNavigate={onNavigate} />;` before the `default` case
6. **SidebarIcons.tsx**: Add `HealthIcon` export — 20x20 viewBox, stroke-based SVG matching existing icon style (use a heartbeat/pulse or shield-check motif). Spread `defaults` like other icons
7. **PageBanner.tsx**: Add `HealthDashboardArt` export — 200x120 viewBox decorative SVG. Follow the style of existing illustration components (low-opacity geometric shapes)

#### Task 1.2: Page shell with summary cards and loading states Depends on [1.1]

**READ THESE BEFORE TASK**

- src/crosshook-native/src/components/pages/CommunityPage.tsx
- src/crosshook-native/src/hooks/useProfileHealth.ts
- src/crosshook-native/src/styles/theme.css
- docs/plans/health-dashboard-page/research-ux.md

**Instructions**

Files to Create

- src/crosshook-native/src/components/pages/HealthDashboardPage.tsx

Files to Modify

- src/crosshook-native/src/styles/theme.css

Create `HealthDashboardPage.tsx` following the `CommunityPage.tsx` thin wrapper pattern:

1. **Component signature**: `export function HealthDashboardPage({ onNavigate }: { onNavigate?: (route: AppRoute) => void })` — wire `onNavigate` from day one for Phase 1 Fix nav
2. **Hook**: Call `useProfileHealth()` at top level — destructure `summary`, `loading`, `error`, `cachedSnapshots`, `batchValidate`
3. **PageBanner**: `eyebrow="Dashboards"`, `title="Profile Health"`, `copy="Aggregate status across all profiles."`, `illustration={<HealthDashboardArt />}`
4. **SummaryCards**: Local `SummaryCard` function component rendered in a 4-column CSS grid. Cards: Total (accent), Healthy (success), Stale (warning), Broken (danger). Each card: left-border accent stripe (4px) in status color, large count number, label below. Use `crosshook-card` class. Grid CSS: `grid-template-columns: repeat(4, 1fr)` with `gap: var(--crosshook-grid-gap)`
5. **Loading state**: When `loading && !summary`, show "Checking profiles..." with disabled summary cards showing `—` placeholders. Never show "0 broken" while validating
6. **Error state**: When `error`, show `role="alert"` banner with generic "Health scan failed. Check app logs for details." + Retry button calling `batchValidate()`. Log full error to console. Do not surface raw error strings in UI
7. **Empty state**: When `summary?.total_count === 0`, show "No profiles configured yet" message + link to Profiles page via `onNavigate?.('profiles')`
8. **All-healthy state**: When `summary?.broken_count === 0 && summary?.stale_count === 0`, show summary cards + positive "All profiles are healthy" message below the table placeholder
9. **Table placeholder**: Render `<table role="grid" aria-label="Profile health status">` with 3-column `<thead>` (Name, Status, Issues) and empty `<tbody>` — populated in Task 1.3. Use `<table>` from day one so Phase 2 adds sort headers without restructuring
10. **CSS**: Add `.crosshook-health-dashboard-cards` grid class and `.crosshook-health-dashboard-card` with left-border accent to `theme.css`

#### Task 1.3: Basic profile list with status badges Depends on [1.2]

**READ THESE BEFORE TASK**

- src/crosshook-native/src/components/HealthBadge.tsx
- src/crosshook-native/src/types/health.ts
- docs/plans/health-dashboard-page/research-business.md

**Instructions**

Files to Modify

- src/crosshook-native/src/components/pages/HealthDashboardPage.tsx

Populate the table `<tbody>` with profile health rows:

1. **Derive sorted profiles**: `useMemo` over `summary?.profiles ?? []` — default sort: broken first (status rank: broken=2 > stale=1 > healthy=0), alphabetical within each group
2. **Render rows**: Each row is `<tr tabIndex={0} role="row" aria-label="[Name] — [Status], [N] issues">` with 3 cells: Name (`{report.name}`), Status (drop-in `<HealthBadge report={report} />`), Issue count (`{report.issues.length}`)
3. **Sentinel detection**: If any `report.name === '<unknown>'`, render a system-level error banner above the table instead of a normal row (edge case EC-05)
4. **Null metadata**: Phase 1 only uses `name`, `status`, `issues`, `launch_method` — no metadata fields. But guard `summary?.profiles` access throughout
5. **Table attributes**: `aria-rowcount={profiles.length}` on `<table>`
6. **Cached display**: If `summary` is null but `cachedSnapshots` has entries, render a minimal list from cached snapshot data with a "Cached — checking..." label. Note: `CachedHealthSnapshot` has different fields than live data — use `snap.profile_name` (not `name`), `snap.status` for badge rendering, and `snap.issue_count` (number, not an issues array). Iterate `Object.values(cachedSnapshots)` to build the list

#### Task 1.4: Fix navigation to profile editor Depends on [1.2]

**READ THESE BEFORE TASK**

- src/crosshook-native/src/hooks/useProfile.ts
- src/crosshook-native/src/context/ProfileContext.tsx

**Instructions**

Files to Modify

- src/crosshook-native/src/components/pages/HealthDashboardPage.tsx

Add "Fix" navigation from dashboard rows to the profile editor:

1. Import `useProfileContext()` from hooks — destructure `selectProfile`
2. Add a "Fix" button or clickable row action for broken/stale profiles (hide for healthy)
3. Handler: `void selectProfile(profileName); onNavigate?.('profiles');` — fire-and-forget pattern. Note: `selectProfile` is async (returns `Promise<void>` — it calls `invoke` internally). The navigation fires immediately while the profile is still loading. `ProfileContext` wraps the entire app, so the async load continues even after route change
4. Row click handler: `onClick={() => { if (report.status !== 'healthy') { void selectProfile(report.name); onNavigate?.('profiles'); } }}`
5. Keyboard: `onKeyDown={(e) => { if (e.key === 'Enter') { ... } }}` for gamepad A-button and keyboard Enter
6. **Race condition risk**: `ProfilesPage` may mount before the async profile load completes. Test this during implementation. If the selected profile is not available when ProfilesPage renders, add a `pendingNavProfile: string | null` field to `ProfileContext` — set it before navigating, have `ProfilesPage` watch for it on mount, call `selectProfile()` from there, and clear the field after loading. This is the recommended robust approach if the fire-and-forget pattern proves unreliable

#### Task 1.5: Re-check All button with ARIA status region Depends on [1.2]

**READ THESE BEFORE TASK**

- src/crosshook-native/src/components/ui/CollapsibleSection.tsx
- docs/plans/health-dashboard-page/research-ux.md

**Instructions**

Files to Modify

- src/crosshook-native/src/components/pages/HealthDashboardPage.tsx

Add the Re-check All button and accessible status announcements:

1. Wrap the re-check area in `<CollapsibleSection title="Re-check" defaultOpen>` above the profile table
2. "Re-check All" button: wire to `batchValidate()`, disable during `loading`, change label to "Checking..." while loading
3. "Last validated" timestamp: show `summary?.validated_at` formatted with an inline `formatRelativeTime` (copy the function from ProfilesPage.tsx:22 — extract to shared util in Phase 2)
4. ARIA live region: `<div role="status" aria-live="polite" aria-atomic="true" className="sr-only">` — set content to "" when idle, "Checking all profiles..." during validation, "Validation complete. N broken, N stale, N healthy." on completion
5. Error announcement: use `role="alert"` (not `aria-live`) for validation failures

### Phase 2: Interactive Table + Diagnostic Panels

#### Task 2.0: Extract formatRelativeTime to shared utility Depends on [none]

**READ THESE BEFORE TASK**

- src/crosshook-native/src/components/pages/ProfilesPage.tsx

**Instructions**

Files to Create

- src/crosshook-native/src/utils/format.ts

Files to Modify

- src/crosshook-native/src/components/pages/ProfilesPage.tsx

1. Create `src/crosshook-native/src/utils/format.ts` — move `formatRelativeTime` from ProfilesPage.tsx:22 to this file as a named export
2. Update `ProfilesPage.tsx` to import from `../../utils/format`
3. Remove the redundant inline copy from `HealthDashboardPage.tsx` (added in 1.5) and import from `../../utils/format` instead

#### Task 2.1: Sortable, filterable table with row expansion Depends on [1.3, 1.4, 1.5]

**READ THESE BEFORE TASK**

- src/crosshook-native/src/components/CompatibilityViewer.tsx
- src/crosshook-native/src/types/health.ts
- docs/plans/health-dashboard-page/feature-spec.md

**Instructions**

Files to Modify

- src/crosshook-native/src/components/pages/HealthDashboardPage.tsx

Upgrade the basic profile list to a full interactive health table:

1. **Page-local types**: Add `SortField = 'name' | 'status' | 'issues' | 'last_success' | 'launch_method' | 'failures' | 'favorite'`, `SortDirection = 'asc' | 'desc'`, `StatusFilter = 'all' | HealthStatus`
2. **State**: `useState` for `sortField`, `sortDirection`, `statusFilter`, `searchQuery`, `expandedProfile: string | null`
3. **Deferred search**: `useDeferredValue(searchQuery)` following CompatibilityViewer.tsx:110-140 pattern
4. **Sort/filter memo**: `useMemo` keyed on `[summary?.profiles, sortField, sortDirection, statusFilter, deferredSearch]` — filter by status, then by search term (`name.toLowerCase().includes(term)` — never RegExp), then sort by field. Status sort uses rank: broken=2 > stale=1 > healthy=0. Favorites pinned to top
5. **TableToolbar**: Local component above table — status filter pills (All/Healthy/Stale/Broken), text search input (`maxLength={200}`, `placeholder="Filter profiles..."`), result count "Showing X of Y"
6. **Column headers**: Expand `<thead>` to 8 sortable columns + 1 action column: Status (40px), Name (flex), Issues (80px), Last Success (120px), Launch Method (100px), Failures (80px), Favorite (40px), Source (80px), Actions (60px). Each `<th>` has `aria-sort`, `onClick` to toggle sort, sort arrow indicator
7. **Row expansion**: Click row body (not action button) toggles `expandedProfile`. Expanded row: `<tr><td colSpan={9}>` showing issues list (field, path in `<code>`, message, remediation) + single-profile re-check button wired to `revalidateSingle(name)`
8. **Metadata columns**: Access `report.metadata` (now typed correctly after Task 0.1) — show "N/A" when null. `last_success` via `formatRelativeTime()`, `failure_count_30d`, `is_favorite` (star icon), `is_community_import` ("Community" badge). For sort: profiles with `null` metadata sort as `is_favorite = false` and `failure_count_30d = 0`
9. **Search input**: `maxLength={200}`, use `String.includes()` for filtering — never `new RegExp()`
10. **Minimum 48px row height** for Steam Deck touchscreen targets

#### Task 2.2: Issue breakdown panel Depends on [2.1]

**READ THESE BEFORE TASK**

- docs/plans/health-dashboard-page/research-business.md

**Instructions**

Files to Modify

- src/crosshook-native/src/components/pages/HealthDashboardPage.tsx

Add aggregate issue breakdown by category:

1. **Types**: Add `IssueCategory` union and `IssueCategoryCount` interface (page-local)
2. **Categorize function**: `categorizeIssue(field: string): IssueCategory` — map `game.executable_path` → `missing_executable`, `trainer.path` → `missing_trainer`, `injection.dll_paths` → `missing_dll`, `steam.proton_path`/`runtime.proton_path` → `missing_proton`, `steam.compatdata_path` → `missing_compatdata`, `runtime.prefix_path` → `missing_prefix`, severity-based `inaccessible_path`, everything else → `other`
3. **Aggregation**: `useMemo` over `summary?.profiles` grouping issues by category → `IssueCategoryCount[]`
4. **Render**: `<CollapsibleSection title="Issue Breakdown" defaultOpen>` placed above the table section. Each category row: label, count badge (`crosshook-status-chip`), CSS width bar (width as % of max count)
5. **Empty state**: If no issues exist, show "No issues found across all profiles"

#### Task 2.3: Recent failures panel Depends on [2.1]

**READ THESE BEFORE TASK**

- docs/plans/health-dashboard-page/research-business.md

**Instructions**

Files to Modify

- src/crosshook-native/src/components/pages/HealthDashboardPage.tsx

Add panel showing profiles with recent launch failures:

1. **Filter**: `useMemo` over `summary?.profiles` where `metadata?.failure_count_30d > 0`, sorted by failure count descending
2. **Render**: `<CollapsibleSection title="Recent Failures" defaultOpen={false}>` below the table. Rows: profile name, failure count (30d), last success date via `formatRelativeTime`
3. **Empty state**: "No profiles with recent launch failures"
4. **Threshold**: `> 0` for panel inclusion (broader than HealthBadge's `>= 2` badge threshold)

#### Task 2.4: Launcher drift summary panel Depends on [2.1]

**READ THESE BEFORE TASK**

- docs/plans/health-dashboard-page/research-business.md

**Instructions**

Files to Modify

- src/crosshook-native/src/components/pages/HealthDashboardPage.tsx

Add panel showing profiles with launcher drift:

1. **Filter**: `useMemo` over `summary?.profiles` where `metadata?.launcher_drift_state` is not null, undefined, `'aligned'`, or `'unknown'`
2. **Render**: `<CollapsibleSection title="Launcher Drift" defaultOpen={false}>`. Per row: profile name, drift state message. Message map: `missing` → "Exported launcher not found", `moved` → "Launcher has moved", `stale` → "Launcher may be outdated"
3. **Empty state**: "All exported launchers are current"

#### Task 2.5: Community import health panel Depends on [2.1]

**READ THESE BEFORE TASK**

- docs/plans/health-dashboard-page/research-business.md

**Instructions**

Files to Modify

- src/crosshook-native/src/components/pages/HealthDashboardPage.tsx

Add panel for broken/stale community-imported profiles:

1. **Filter**: `useMemo` over `summary?.profiles` where `metadata?.is_community_import === true` and `report.status !== 'healthy'`
2. **Render**: `<CollapsibleSection title="Community Import Health" defaultOpen={false}>`. Contextual note: "Imported profiles often need path adjustments for your system." Per row: profile name, status badge, issue count
3. **Empty state**: "All community-imported profiles are healthy"

### Phase 3: Polish

#### Task 3.1: Skeleton loading states Depends on [2.1]

**READ THESE BEFORE TASK**

- docs/plans/health-dashboard-page/research-ux.md

**Instructions**

Files to Modify

- src/crosshook-native/src/components/pages/HealthDashboardPage.tsx
- src/crosshook-native/src/styles/theme.css

Replace basic loading states with skeleton placeholders:

1. **Skeleton cards**: 4 placeholder cards matching summary card dimensions with pulsing gray animation
2. **Skeleton rows**: 5-8 placeholder table rows with pulsing cells
3. **CSS**: Add `@keyframes crosshook-skeleton-pulse` with opacity animation (1.5s ease infinite). Add `.crosshook-health-dashboard-skeleton` class
4. **Cached-to-live transition**: If `cachedSnapshots` exist on mount, show cached data with "Cached — checking..." label; swap to live data without layout shift when scan completes
5. **Reduced motion**: `@media (prefers-reduced-motion: reduce) { .crosshook-health-dashboard-skeleton { animation: none; } }`

#### Task 3.2: Gamepad Y button for Re-check All Depends on [2.1]

**READ THESE BEFORE TASK**

- src/crosshook-native/src/hooks/useGamepadNav.ts

**Instructions**

Files to Modify

- src/crosshook-native/src/components/pages/HealthDashboardPage.tsx

Add page-local Y button handler for gamepad re-check:

1. Add `useEffect` that polls `navigator.getGamepads()` via `requestAnimationFrame`
2. Detect edge press of button index 3 (Y on Xbox / Triangle on PlayStation)
3. Trigger `batchValidate()` on edge press when `!loading`
4. Track previous button state via `useRef` for edge detection
5. Cleanup: `cancelAnimationFrame(rafId)` on unmount
6. Only activate if `navigator.getGamepads()[0]` exists (controller connected)
7. Do NOT modify `useGamepadNav.ts` — this is page-local per resolved decision

#### Task 3.3: Trend arrows on summary cards and table rows Depends on [2.1]

**READ THESE BEFORE TASK**

- src/crosshook-native/src/components/HealthBadge.tsx

**Instructions**

Files to Modify

- src/crosshook-native/src/components/pages/HealthDashboardPage.tsx

Add trend indicators based on cached snapshot comparison:

1. **Per-row trends**: Pass `trend={trendByName[report.name]}` to `HealthBadge` in table rows — the badge already renders trend arrows via its built-in `trend` prop
2. **Summary card trends**: Derive cached aggregate counts by iterating `Object.values(cachedSnapshots)` and counting by status: `const cachedHealthy = Object.values(cachedSnapshots).filter(s => s.status === 'healthy').length` (same for stale, broken). Compare against `summary.{healthy,stale,broken}_count`. Show up/down arrow per card. For Healthy card: count going up = green up arrow (improving). For Broken/Stale cards: count going up = red up arrow (worsening). Use same color variables as HealthBadge
3. **Null trend**: Render nothing when `trendByName[name]` is undefined or `'unchanged'`
4. **Stale snapshot indicator**: If `staleInfoByName[name]?.isStale`, show muted "(cached N days ago)" next to the trend arrow

#### Task 3.4: Responsive summary card layout Depends on [none]

**READ THESE BEFORE TASK**

- src/crosshook-native/src/styles/variables.css

**Instructions**

Files to Modify

- src/crosshook-native/src/styles/theme.css

Add responsive breakpoints for the summary card grid:

1. Default: `grid-template-columns: repeat(4, 1fr)` (already set in Phase 1)
2. `@media (max-width: 1100px)`: `grid-template-columns: repeat(2, 1fr)` — 2x2 grid
3. `@media (max-width: 640px)`: `grid-template-columns: 1fr` — stacked
4. Test at 1280x800 (Steam Deck native resolution) — all 4 cards must fit in one row
5. Ensure minimum 48px height on all interactive elements for touchscreen targets

## Advice

- All five routing edits in Task 1.1 must land in a single atomic commit — the TypeScript exhaustive `never` check at ContentArea.tsx:48 will break the build if `AppRoute` grows without a matching switch case. Run `npm run dev` after the edit to verify compilation before committing.
- The `useProfileHealth` hook types `summary` as `HealthCheckSummary` but the Rust backend returns `EnrichedHealthSummary` with metadata. Task 0.1 fixes this at the type level. ProfilesPage.tsx:507 already casts to the enriched type — after 0.1, that cast becomes redundant but harmless.
- `CollapsibleSection` uses native `<details>/<summary>` — it must NOT be placed inside a `<table>` element (invalid HTML). Use it only for panels surrounding the table, never inside `<tbody>`.
- The `formatRelativeTime` function is duplicated between ProfilesPage and HealthDashboardPage in Phase 1 (inline copy). Task 2.0 extracts it to a shared utility. Do not extract prematurely — the second consumer (dashboard) justifies extraction at Phase 2 time.
- For text filtering, always use `String.prototype.includes()` — never `new RegExp(userInput)` which creates ReDoS risk. This is a security requirement from research-security.md.
- The "Dashboards" sidebar section was chosen over "Diagnostics" to be future-proof — it allows grouping additional dashboards (Launch History, Compatibility) under the same section later.
- The feature-spec resolved that `useProfileHealth` uses separate instances per page (no context lift). ContentArea renders only one page at a time via its switch, so dual instances never coexist. If profiling ever reveals duplicate batch calls, lift to a `ProfileHealthContext` at that point — but not preemptively.
- When rendering profile names or paths, always use JSX interpolation (`{profile.name}`) — never `dangerouslySetInnerHTML`. Paths arrive pre-sanitized from the backend (`~` replaces home dir). Error messages may contain unsanitized paths — display a generic message and log the full error.
