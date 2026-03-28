# Health Dashboard Page — Task Structure Analysis

## Executive Summary

The health-dashboard-page feature is a frontend-only addition consuming existing Tauri IPC infrastructure. The planning documents define a clear 3-phase delivery structure with one pre-phase type fix. The recommended task breakdown optimizes for: (1) strict phase boundaries as ship points, (2) maximum parallelization within each phase, and (3) 1–3 file scope per task to keep PRs reviewable. The critical path runs through Pre-P1 → 1.1 (routing) → 1.2 (page shell) → 1.3/1.4/1.5 in parallel → Phase 2 entry.

The planning docs note a discrepancy between `research-recommendations.md` (4-phase model) and the team-lead-requested 3-phase model. This analysis follows the **3-phase structure** as instructed: diagnostic panels (formerly Phase 4) are folded into Phase 2, and Phase 3 is the polish/trend/gamepad phase.

---

## Recommended Phase Structure

### Pre-Phase 1: Type Fix (Unblocked, 1 task)

**P0 — Fix `useProfileHealth` invoke generics**

- File: `src/crosshook-native/src/hooks/useProfileHealth.ts`
- Change: Update `invoke<HealthCheckSummary>` → `invoke<EnrichedHealthSummary>` in batch validate and cached snapshots calls
- Risk: Identified as "High likelihood" type mismatch in all planning docs — must resolve before any Phase 1 work consumes metadata fields
- No runtime impact; type-only change; unblocks Phase 1

---

### Phase 1: MVP (Core Dashboard)

**Goal**: Route wiring, page shell, summary cards, basic profile list with Fix navigation, Re-check All, loading/error/empty states, D-pad gamepad nav.

Five tasks, with tasks 1.3–1.5 parallelizable after 1.2 completes.

---

**Task 1.1 — Routing Integration** _(blocks all Phase 1)_

Files touched (all small edits to existing files):

- `src/crosshook-native/src/components/layout/Sidebar.tsx` — add `'health'` to `AppRoute` union (line 12), add entry in `SIDEBAR_SECTIONS` under a new "Dashboards" section (line 32), add `ROUTE_LABELS` entry
- `src/crosshook-native/src/App.tsx` — add `health: true` to `VALID_APP_ROUTES`
- `src/crosshook-native/src/components/layout/ContentArea.tsx` — add `'health'` case to `renderPage()` switch (lines 34–51); import `HealthDashboardPage`
- `src/crosshook-native/src/components/icons/SidebarIcons.tsx` — add `HealthIcon` SVG (20x20 viewBox, stroke-based, matching existing icon style)
- `src/crosshook-native/src/components/layout/PageBanner.tsx` — add `HealthDashboardArt` illustration SVG (200x120 viewBox)

Implementation note: `ContentArea.tsx:47` uses a TypeScript exhaustive check (`const _exhaustive: never = route`). Adding `'health'` to `AppRoute` in `Sidebar.tsx` will cause a compile error in `ContentArea.tsx` until the switch case is added. All 5 file edits must land atomically in one PR to keep the build green.

---

**Task 1.2 — Page Shell + Summary Cards** _(depends on 1.1, blocks 1.3/1.4/1.5)_

Files touched:

- `src/crosshook-native/src/components/pages/HealthDashboardPage.tsx` — **create new file**
- `src/crosshook-native/src/styles/theme.css` — add `crosshook-health-dashboard*` CSS classes, 4-column summary card grid

Implementation scope:

- Create `HealthDashboardPage` component following `CommunityPage.tsx` thin wrapper pattern
- Wire `useProfileHealth()` as sole data source
- Implement `PageBanner` header (`eyebrow="Diagnostics"`, `HealthDashboardArt`)
- Implement 4 inline `SummaryCard` sub-components in a CSS grid row:
  - Total (`--crosshook-color-accent`), Healthy (`--crosshook-color-success`), Stale (`--crosshook-color-warning`), Broken (`--crosshook-color-danger`)
  - Left-border accent stripe (4px) in status color — not full background fills
- Implement 3 state variants: loading ("Checking profiles…"), error (message + Retry → `batchValidate()`), empty (`total_count === 0`)
- Props interface: `{ onNavigate?: (route: AppRoute) => void }` — wire from day one even if only used for Fix nav
- Profile list placeholder `<table>` shell with 3-column `<thead>` (Name, Status, Issues) — populated in 1.3

Design constraint: The profile list MUST be `<table>` from Phase 1 (not `<ul>`) so Phase 2 can add sort headers to `<thead>` without restructuring.

---

**Task 1.3 — Basic Profile List** _(depends on 1.2, parallelizable with 1.4 and 1.5)_

Files touched:

- `src/crosshook-native/src/components/pages/HealthDashboardPage.tsx`

Implementation scope:

- Populate `<tbody>` with `summary.profiles` rendered as rows
- Default sort: broken-first (status hierarchy: broken=2 > stale=1 > healthy=0) — hardcoded, no sort state yet
- Columns: Name, Status (via `HealthBadge` drop-in), Issue Count (`report.issues.length`)
- Row attributes: `tabIndex={0}`, `role="row"`, `aria-label="[Name] — [Status], [N] issues"`
- Table attributes: `role="grid"`, `aria-label`, `aria-rowcount`
- All-healthy empty state: summary cards show correct counts + "All profiles are healthy" message below table
- Null metadata handling: page renders without crashing when `report.metadata === null` (metadata columns are not present yet in Phase 1)
- Sentinel `<unknown>` detection: if any `report.name === '<unknown>'`, render system-level error banner above table

---

**Task 1.4 — Fix Navigation** _(depends on 1.2, parallelizable with 1.3 and 1.5)_

Files touched:

- `src/crosshook-native/src/components/pages/HealthDashboardPage.tsx`

Implementation scope:

- Import `useProfileContext()` from `src/crosshook-native/src/hooks/useProfile.ts` to get `selectProfile()`
- Add "Fix" button column to the profile list table rows (broken/stale profiles only; hide for healthy)
- Handler: `selectProfile(profileName)` then `onNavigate('profiles')` sequentially
- Verify timing: `ProfileContext` wraps the entire app so `selectProfile` updates global state before `ProfilesPage` mounts — per docs, no additional mechanism needed
- If timing proves unreliable (to test during implementation): add `pendingNavProfile: string | null` to `ProfileContext` as fallback

---

**Task 1.5 — Re-check All + Status Region** _(depends on 1.2, parallelizable with 1.3 and 1.4)_

Files touched:

- `src/crosshook-native/src/components/pages/HealthDashboardPage.tsx`

Implementation scope:

- Add `CollapsibleSection` wrapper around Re-check area (label: "Re-check", `defaultOpen={true}`)
- "Re-check All" button wired to `batchValidate()`; disabled during scan; label changes to "Checking…" during loading
- "Last validated: X min ago" using `summary.validated_at` (use inline `formatRelativeTime` — extraction to shared util is Phase 2)
- `role="status"` + `aria-live="polite"` region for completion announcement
- D-pad: table rows already have `tabIndex={0}` from 1.3; add `data-crosshook-focus-zone="content"` on the table's wrapping `div` for D-pad traversal via `useGamepadNav`

---

### Phase 2: Interactive Table + Secondary Panels

**Goal**: Sortable/filterable table, issue breakdown panel, recent failures, launcher drift, community import health.

Seven tasks. Task 2.1 blocks 2.2 and 2.3. Tasks 2.4, 2.5, 2.6, 2.7 are all independent of each other but all depend on 2.1. Task 2.0 (utility extraction) is independent and can run in parallel with 2.1.

---

**Task 2.0 — Extract `formatRelativeTime` utility** _(independent, parallelizable with 2.1)_

Files touched:

- `src/crosshook-native/src/utils/format.ts` — **create new file**; extract `formatRelativeTime` from `ProfilesPage.tsx:22`
- `src/crosshook-native/src/components/pages/ProfilesPage.tsx` — update import to use `utils/format.ts`

Note: `research-technical.md` names the target file `utils/format.ts`; `research-recommendations.md` calls it `utils/time.ts`. Recommend `utils/format.ts` (more general, matches technical spec).

---

**Task 2.1 — Sortable Table + Filtering + Search** _(depends on Phase 1 complete, blocks 2.2/2.3/2.4/2.5/2.6/2.7)_

Files touched:

- `src/crosshook-native/src/components/pages/HealthDashboardPage.tsx`

Implementation scope:

- Add page-local types: `SortField`, `SortDirection`, `TableSort`, `StatusFilter`
- Add `useState` for: `tableSort: TableSort`, `statusFilter: StatusFilter`, `searchQuery: string`
- Add `useDeferredValue(searchQuery)` for debounced text search (follow `CompatibilityViewer.tsx` lines 110–140)
- Replace Phase 1 hardcoded sort with `useMemo` sort/filter chain keyed on `[summary.profiles, tableSort, statusFilter, deferredSearch]`
- Add `<TableToolbar>` local component above table: status filter pills (All/Healthy/Stale/Broken) + text search input (`maxLength={200}`) + result count "Showing X of Y profiles"
- Expand `<thead>` to 8 sortable columns: Name, Status, Issues, Last Success, Launch Method, Favorite, Source, Actions
- Click column header cycles sort direction: `asc → desc → none`; `aria-sort` attribute on `<th>`
- Sort indicator arrows in header cells
- Default sort: Status (broken first), favorites pinned to top regardless of sort field
- Expand `<tbody>` rows to include all columns; show "N/A" for null metadata fields
- Row expansion: clicking a row inserts a detail `<tr>` below with `<td colSpan={8}>` showing issues list + single re-check button
  - Issue fields: field, path, message, remediation — follow `ProfilesPage.tsx:548–559` pattern
  - Single-profile re-check: `revalidateSingle(name)` from `useProfileHealth`
  - `expandedProfile: string | null` state at page level
- Import `formatRelativeTime` from `utils/format.ts` (Task 2.0)

---

**Task 2.2 — Issue Breakdown Panel** _(depends on 2.1)_

Files touched:

- `src/crosshook-native/src/components/pages/HealthDashboardPage.tsx`

Implementation scope:

- Add local type: `IssueCategory`, `IssueCategoryCount`
- Add `categorizeIssue(field: string): IssueCategory` function using field prefix matching:
  - `game.executable_path` → `missing_executable`
  - `steam.proton_path` / `runtime.proton_path` → `missing_proton`
  - `steam.compatdata_path` / `runtime.prefix_path` → `missing_compatdata` / `missing_prefix`
  - `trainer.path` → `missing_trainer`
  - `injection.dll_paths` → `missing_dll`
  - `inaccessible_path` prefix → `inaccessible_path`
  - Everything else → `other`
- `useMemo` aggregation over `summary.profiles` → `IssueCategoryCount[]`
- Render as `CollapsibleSection` (label: "Issue Breakdown", `defaultOpen={true}`)
- Category rows: label, count badge, CSS width bar chart (width as `%` of max count)
- Clicking a category filters the main table to show only profiles with that issue type (adds `issueFilter: IssueCategory | null` state feeding into the 2.1 `useMemo` chain)

---

**Task 2.3 — Recent Failures Panel** _(depends on 2.1, independent of 2.2/2.4/2.5/2.6/2.7)_

Files touched:

- `src/crosshook-native/src/components/pages/HealthDashboardPage.tsx`

Implementation scope:

- Filter `summary.profiles` where `metadata?.failure_count_30d > 0`
- Render as `CollapsibleSection` (label: "Recent Failures", `defaultOpen={false}`)
- Columns: profile name, failure count (30d), last success date (`formatRelativeTime`)
- Empty state: "No profiles with recent launch failures"
- Threshold: `> 0` for panel inclusion (broader than `HealthBadge`'s `>= 2` badge threshold — show all, flag heavy hitters)

---

**Task 2.4 — Launcher Drift Summary Panel** _(depends on 2.1, independent of 2.2/2.3/2.5/2.6/2.7)_

Files touched:

- `src/crosshook-native/src/components/pages/HealthDashboardPage.tsx`

Implementation scope:

- Filter `summary.profiles` where `metadata?.launcher_drift_state` is not `null`, `undefined`, or `'aligned'`
- Render as `CollapsibleSection` (label: "Launcher Drift", `defaultOpen={false}`)
- Message map per drift state:
  - `missing` → "Exported launcher not found — re-export recommended"
  - `moved` → "Exported launcher has moved — re-export recommended"
  - `stale` → "Exported launcher may be outdated — re-export recommended"
  - `unknown` → "Launcher state could not be determined"
- Empty state: "All exported launchers are current"

---

**Task 2.5 — Community Import Health Panel** _(depends on 2.1, independent of 2.2/2.3/2.4/2.6/2.7)_

Files touched:

- `src/crosshook-native/src/components/pages/HealthDashboardPage.tsx`

Implementation scope:

- Filter `summary.profiles` where `metadata?.is_community_import === true` and `report.status !== 'healthy'`
- Render as `CollapsibleSection` (label: "Community Import Health", `defaultOpen={false}`)
- Contextual annotation: "Imported profiles often need path adjustments for your system."
- Show profile name, status badge, and issue count per row
- Empty state: "All community-imported profiles are healthy"

---

### Phase 3: Polish

**Goal**: Skeleton loading, Y-button gamepad, trend arrows on cards and rows, responsive layout, `formatRelativeTime` extraction (if not already done in Phase 2).

Five tasks — all independent of each other. All can be parallelized.

---

**Task 3.1 — Skeleton Loading States** _(depends on Phase 2 complete, independent of 3.2/3.3/3.4/3.5)_

Files touched:

- `src/crosshook-native/src/components/pages/HealthDashboardPage.tsx`
- `src/crosshook-native/src/styles/theme.css` — add skeleton pulse animation CSS

Implementation scope:

- If `cachedSnapshots` are available on mount: show cached data immediately with a "Cached — checking…" inline label; replace with live data when scan completes
- If no cached snapshots: show 4 skeleton summary cards + 5–8 skeleton table rows (pulsing gray rectangles)
- CSS: `@keyframes crosshook-skeleton-pulse` with `opacity` animation
- Transition from cached → live: no layout shift (same table structure, data swaps in place)
- Respect `prefers-reduced-motion`: disable pulse animation if media query matches

---

**Task 3.2 — Gamepad Y Button Re-check** _(depends on Phase 2 complete, independent of 3.1/3.3/3.4/3.5)_

Files touched:

- `src/crosshook-native/src/components/pages/HealthDashboardPage.tsx`

Implementation scope:

- Add page-local `useEffect` polling `navigator.getGamepads()` via `requestAnimationFrame`
- Detect edge press of button index 3 (Y on Xbox layout / cross on PlayStation layout)
- Trigger `batchValidate()` on edge press when `!loading`
- `controllerMode` detection: poll `navigator.getGamepads()[0]` locally (if connected gamepad exists) rather than threading `controllerMode` prop through `ContentArea` — matches resolved decision in `feature-spec.md`
- Add `ControllerPrompts` Y-button hint visible when controller detected
- Cleanup: `cancelAnimationFrame(rafId)` on unmount

Implementation note: Avoid modifying `useGamepadNav.ts` — this is a page-local need per the resolved decision.

---

**Task 3.3 — Trend Arrows** _(depends on Phase 2 complete, independent of 3.1/3.2/3.4/3.5)_

Files touched:

- `src/crosshook-native/src/components/pages/HealthDashboardPage.tsx`

Implementation scope:

- Per-profile trend arrows in table Trend column: use `trendByName[report.name]` from `useProfileHealth`
- Render: `got_worse` → red down arrow, `got_better` → green up arrow; `null` and `unchanged` render nothing
- Aggregate trend arrows on summary cards: compare current `summary.{healthy,stale,broken}_count` vs. aggregate counts derived from `cachedSnapshots`
- Use same color convention as existing `HealthBadge` trend rendering
- Phase 2 `SortField` already includes `'favorite'` and `'failures'` — no new sort state needed for trends (trend column is not sortable per the column spec)

---

**Task 3.4 — Responsive Card Layout** _(depends on Phase 2 complete, independent of 3.1/3.2/3.3/3.5)_

Files touched:

- `src/crosshook-native/src/styles/theme.css`

Implementation scope:

- CSS `@media` breakpoints for summary card grid:
  - `>1100px`: 4 cards in a row (`grid-template-columns: repeat(4, 1fr)`)
  - `900px–1100px`: 2×2 grid (`grid-template-columns: repeat(2, 1fr)`)
  - `<640px`: stacked (`grid-template-columns: 1fr`)
- Test at 1280×800 (Steam Deck default resolution) — table columns must remain readable without horizontal scroll
- Ensure `min-height: 48px` on table rows for touchscreen targets

---

**Task 3.5 — Extract `formatRelativeTime` (if not done in Phase 2)** _(independent)_

This task is conditional on Task 2.0. If Task 2.0 was completed in Phase 2, mark 3.5 as done. If Phase 2 was shipped without 2.0, complete the extraction here before Phase 3 closes.

Files: same as Task 2.0.

---

## Task Granularity Recommendations

| Task | Files | Scope                            | Parallelizable?         |
| ---- | ----- | -------------------------------- | ----------------------- |
| P0   | 1     | Type-only fix                    | Yes (unblocked)         |
| 1.1  | 5     | All routing wiring, atomic PR    | No (blocks 1.2)         |
| 1.2  | 2     | New file + CSS                   | No (blocks 1.3/1.4/1.5) |
| 1.3  | 1     | Table rows, badges               | Yes (with 1.4, 1.5)     |
| 1.4  | 1     | Fix navigation handler           | Yes (with 1.3, 1.5)     |
| 1.5  | 1     | Re-check button + ARIA           | Yes (with 1.3, 1.4)     |
| 2.0  | 2     | Utility extraction               | Yes (with 2.1)          |
| 2.1  | 1     | Sort/filter/search/row expansion | No (blocks 2.2–2.5)     |
| 2.2  | 1     | Issue breakdown panel            | Yes (with 2.3/2.4/2.5)  |
| 2.3  | 1     | Recent failures panel            | Yes (with 2.2/2.4/2.5)  |
| 2.4  | 1     | Launcher drift panel             | Yes (with 2.2/2.3/2.5)  |
| 2.5  | 1     | Community import panel           | Yes (with 2.2/2.3/2.4)  |
| 3.1  | 2     | Skeleton loading                 | Yes                     |
| 3.2  | 1     | Y-button gamepad                 | Yes                     |
| 3.3  | 1     | Trend arrows                     | Yes                     |
| 3.4  | 1     | Responsive CSS                   | Yes                     |
| 3.5  | 2     | formatRelativeTime (conditional) | Yes                     |

Task 1.1 is the only task that must touch 5 files simultaneously — this is unavoidable because of the TypeScript exhaustive check in `ContentArea.tsx` that enforces atomicity.

---

## Dependency Analysis

### Within-Phase Dependencies

```
Pre-Phase 1:
  P0 (type fix) ─── unblocked ──→ can ship first or alongside 1.1

Phase 1:
  1.1 (routing) ──→ 1.2 (page shell) ──┬──→ 1.3 (profile list)
                                        ├──→ 1.4 (fix nav)
                                        └──→ 1.5 (re-check)

Phase 2:
  2.0 (format.ts extraction) ─── independent of 2.1, can run in parallel
  2.1 (sortable table) ──→ 2.2 (issue breakdown)
                       ──→ 2.3 (recent failures)
                       ──→ 2.4 (launcher drift)
                       ──→ 2.5 (community imports)

Phase 3:
  3.1 ─── independent
  3.2 ─── independent
  3.3 ─── independent
  3.4 ─── independent
  3.5 ─── independent (conditional on 2.0 not done)
```

### Across-Phase Dependencies

```
P0 ──→ (must complete before Phase 2 metadata columns)
Phase 1 complete ──→ Phase 2 entry
Phase 2 complete ──→ Phase 3 entry
```

P0 is labeled "Pre-Phase 1" but it is not strictly blocking Phase 1 itself — Phase 1 does not use metadata fields. However, it MUST complete before Phase 2's metadata columns (2.1) use `EnrichedHealthSummary.profiles[].metadata`. The safest approach: ship P0 with Phase 1 routing (1.1) in the same PR or immediately before.

---

## File-to-Task Mapping

| File                                           | Task(s)                                                             | Notes                                                  |
| ---------------------------------------------- | ------------------------------------------------------------------- | ------------------------------------------------------ |
| `src/components/layout/Sidebar.tsx`            | 1.1                                                                 | `AppRoute` union + `SIDEBAR_SECTIONS` + `ROUTE_LABELS` |
| `src/App.tsx`                                  | 1.1                                                                 | `VALID_APP_ROUTES`                                     |
| `src/components/layout/ContentArea.tsx`        | 1.1                                                                 | Switch case + import                                   |
| `src/components/icons/SidebarIcons.tsx`        | 1.1                                                                 | `HealthIcon` SVG                                       |
| `src/components/layout/PageBanner.tsx`         | 1.1                                                                 | `HealthDashboardArt` SVG                               |
| `src/hooks/useProfileHealth.ts`                | P0                                                                  | `invoke<>` generics only                               |
| `src/components/pages/HealthDashboardPage.tsx` | 1.2 (create), 1.3, 1.4, 1.5, 2.1, 2.2, 2.3, 2.4, 2.5, 3.1, 3.2, 3.3 | Single file, grows across phases                       |
| `src/styles/theme.css`                         | 1.2, 3.1, 3.4                                                       | CSS additions only                                     |
| `src/utils/format.ts`                          | 2.0 (create)                                                        | `formatRelativeTime` extraction                        |
| `src/components/pages/ProfilesPage.tsx`        | 2.0                                                                 | Update import only                                     |

Note: `src/crosshook-native/src/types/health.ts` and `src/crosshook-native/src/utils/health.ts` are consumed but not modified in any phase.

---

## Parallelization Opportunities

**Within Phase 1** (after 1.1 and 1.2 land):

- 1.3, 1.4, and 1.5 all touch only `HealthDashboardPage.tsx` — they should be developed sequentially by one contributor or split into non-overlapping sections to avoid merge conflicts. If parallelizing across contributors: 1.3 owns `<tbody>` rendering, 1.4 owns the Fix handler + context import, 1.5 owns the CollapsibleSection wrapper and button component. Conflicts are unlikely since they touch different JSX regions.

**Within Phase 2** (after 2.1 lands):

- 2.2, 2.3, 2.4, 2.5 all add `CollapsibleSection` blocks to `HealthDashboardPage.tsx` below the main table — parallel development is feasible if contributors work on clearly separated JSX sections and merge sequentially. Alternatively, ship them as one PR since they are all small additions.
- 2.0 (`format.ts`) has zero overlap with 2.1 — fully parallelizable.

**Within Phase 3** (all independent):

- 3.1 (skeleton CSS + conditional render) and 3.4 (responsive CSS) both touch `theme.css` — coordinate on CSS to avoid conflicts. All other Phase 3 tasks touch only `HealthDashboardPage.tsx` and are non-overlapping (different JSX regions).
- All five Phase 3 tasks can ship as a single polishing PR.

---

## Implementation Strategy

### Recommended PR Sequence

1. **PR 0 (Pre-P1)**: `fix(health): update useProfileHealth invoke generics to EnrichedHealthSummary` — single file, type-only
2. **PR 1 (Phase 1, Task 1.1)**: `feat(health): wire health route, icon, and page banner art` — 5 existing files, no new file
3. **PR 2 (Phase 1, Tasks 1.2–1.5)**: `feat(health): MVP dashboard page with summary cards, profile list, and re-check` — new `HealthDashboardPage.tsx` + CSS additions
4. **PR 3 (Phase 2, Task 2.0 + 2.1)**: `feat(health): sortable/filterable profile health table with row expansion` — ships together since `formatRelativeTime` is needed by 2.1
5. **PR 4 (Phase 2, Tasks 2.2–2.5)**: `feat(health): diagnostic panels (issue breakdown, failures, drift, community)` — four CollapsibleSection panels
6. **PR 5 (Phase 3, Tasks 3.1–3.4)**: `feat(health): polish — skeleton loading, Y-button gamepad, trend arrows, responsive layout`

### Atomic Constraint for Task 1.1

Task 1.1 touches 5 files but they MUST land in one atomic commit/PR. The TypeScript exhaustive check at `ContentArea.tsx:47` (`const _exhaustive: never = route`) means the repo will fail to compile if `'health'` is added to `AppRoute` without the corresponding `ContentArea` switch case — and vice versa, if `ContentArea` references `HealthDashboardPage` before the route is registered in `VALID_APP_ROUTES`. All 5 edits ship together.

### Single-File Growth Plan for `HealthDashboardPage.tsx`

| After Phase | Estimated Lines | Action                    |
| ----------- | --------------- | ------------------------- |
| Phase 1     | ~250            | No extraction needed      |
| Phase 2     | ~500            | No extraction needed      |
| Phase 3     | ~700–800        | Evaluate; extract if >800 |

The `ProfilesPage.tsx` reference file is 715 lines. Staying under 800 lines is feasible if inline sub-components are kept small. If Phase 3 pushes past 800 lines, candidates for extraction: `IssueCategoryBreakdown`, `RecentFailuresPanel`, `LauncherDriftSummary`.

### Risk Mitigation Checkpoints

| Phase | Checkpoint                                             | Action if Failed                                                                           |
| ----- | ------------------------------------------------------ | ------------------------------------------------------------------------------------------ |
| P0    | Log `summary.profiles[0]?.metadata` at runtime         | Verify `EnrichedHealthSummary` fields arrive from IPC                                      |
| 1.4   | Test Fix navigation timing in dev                      | If `ProfilesPage` mounts before profile loads, add `pendingNavProfile` to `ProfileContext` |
| 2.1   | Verify `revalidateSingle` exists on `useProfileHealth` | Confirm hook exports this method before implementing single-row re-check                   |
| 3.2   | Test Y button on controller/Steam Deck                 | Verify button index 3 is correct for connected device                                      |

### Key Design Invariants (Do Not Break)

- Profile list is always `<table>` (never `<ul>` or card grid) — Phase 2 adds sort headers to `<thead>`
- `onNavigate?: (route: AppRoute) => void` prop wired from Phase 1 — used for Fix nav from Phase 1 onward
- All rendering via JSX interpolation only — never `dangerouslySetInnerHTML` on profile names or paths (XSS mitigation)
- `String.includes()` for text search — never `RegExp` (ReDoS protection)
- `maxLength={200}` on all filter inputs
- New CSS classes follow `crosshook-health-dashboard*` namespace in `theme.css`
- No new npm dependencies in any phase

---

## References

- `docs/plans/health-dashboard-page/feature-spec.md` — resolved decisions, business rules BR-01–BR-08, edge cases EC-01–EC-07
- `docs/plans/health-dashboard-page/research-technical.md` — component hierarchy, data models, phase boundary contracts, per-file change lists
- `docs/plans/health-dashboard-page/research-recommendations.md` — phasing rationale, risk assessment, alternative approaches
- `docs/plans/health-dashboard-page/shared.md` — relevant files and architectural patterns inventory
