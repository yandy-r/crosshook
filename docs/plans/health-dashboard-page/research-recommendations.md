# Health Dashboard Page — Implementation Recommendations

## Executive Summary

The Health Dashboard is a frontend-only read-only diagnostics page that aggregates all profile health data into a single top-level view. All required backend infrastructure exists (Phase A+B+D). The recommended approach is a single new page component (`HealthDashboardPage.tsx`) consuming the existing `useProfileHealth` hook, reusing `HealthBadge`, `CollapsibleSection`, and `PageBanner` components, with a hand-rolled sortable table (no new dependencies). The feature touches 5 existing files for routing integration and adds 1 new page file plus CSS additions to `theme.css`.

**This is a large feature. The central recommendation is a strict phased approach where each phase is independently shippable and delivers standalone user value.** The phases are designed so that a team could ship Phase 1 and defer the rest indefinitely without leaving the feature incomplete — each subsequent phase refines and enriches rather than completing something half-built.

Key recommendations:

- **MVP-first phasing** — 4 phases, each independently shippable (see Phased Implementation Strategy below)
- **No new dependencies** — hand-roll the table with `useMemo` sort/filter, consistent with the existing `CompatibilityViewer` pattern
- **No HealthContext provider** — the startup event deduplication in `useProfileHealth` makes dual-instantiation safe in practice
- **Cross-page "Fix" navigation** via sequential `selectProfile()` + `onNavigate('profiles')` — test first, add `pendingNavProfile` only if timing fails
- **Y button re-check** via a page-local `useEffect` (Phase 3 scope) — avoids modifying the shared `useGamepadNav` hook
- **Single-file component** — inline sub-components inside `HealthDashboardPage.tsx`, following the `ProfilesPage.tsx` pattern

---

## Phased Implementation Strategy

This is the most important section of this document. The feature is structured into 4 phases, each independently shippable. **Each phase boundary is a natural stopping point** — the feature is useful and complete at the end of every phase.

### Phasing Rationale

The phases are ordered by the ratio of user value delivered per unit of implementation effort:

| Phase                 | What Ships                                                          | User Value                                                      | Effort     | Value/Effort           |
| --------------------- | ------------------------------------------------------------------- | --------------------------------------------------------------- | ---------- | ---------------------- |
| 1 — Minimal Dashboard | Route + summary cards + static profile list + re-check              | Users see aggregate health at a glance + can re-check           | Low-medium | Highest                |
| 2 — Interactive Table | Sort, filter, search, fix navigation                                | Users can triage and act on specific profiles                   | Medium     | High                   |
| 3 — Gamepad + Trends  | Y button re-check, trend arrows, skeleton loading                   | Steam Deck users get full controller experience + trend context | Low-medium | Medium                 |
| 4 — Diagnostic Panels | Issue breakdown, recent failures, launcher drift, community imports | Power users get deep diagnostic views                           | Low        | Lower (niche audience) |

**Why this ordering:**

- Phase 1 answers the primary user question: "Are my profiles healthy?" A summary card row plus a basic list is enough to answer it. No sorting or filtering needed — just show the data.
- Phase 2 answers the follow-up: "Which profiles need attention and what can I do about it?" Sort/filter/fix-navigation are the interaction layer on top of the data.
- Phase 3 serves the Steam Deck audience (controller input) and returning users (trend context). Both are important but neither blocks the core diagnostic value.
- Phase 4 serves power users with many profiles who need categorical breakdowns. These are filtered views of data that already displays in the table — they add convenience, not new capability.

**Phase boundaries as ship points:**

- After Phase 1: The Health tab exists, shows summary cards and a profile list. Users can re-check. Ship it.
- After Phase 2: The table is sortable and filterable, "Fix" navigates to the editor. This is the "complete" dashboard for most users. Ship it.
- After Phase 3: Steam Deck users have full controller support, trend arrows add temporal context. Ship it.
- After Phase 4: Power users get specialized diagnostic panels. Full feature complete. Ship it.

---

### Phase 1: Minimal Viable Dashboard

**Goal**: Get the Health tab into the sidebar and show aggregate health data. Users can see at a glance how many profiles are healthy, stale, or broken, and trigger a re-check. This is the minimum useful dashboard.

**What ships**: A new top-level "Health" route with summary cards + a read-only profile list (no sort/filter yet) + "Re-check All" button.

**Why this is independently shippable**: The primary user story is "I want to see at a glance which profiles are broken before I try to launch a game." Summary cards answer this directly. A static list of profiles with status badges gives enough detail. No sorting or filtering is needed when the list is 5–30 items — users can scan visually.

#### Task Group 1.1: Routing Integration

- Add `'health'` to `AppRoute` type in `Sidebar.tsx`
- Add `HealthIcon` to `SidebarIcons.tsx`
- Add health route to `SIDEBAR_SECTIONS` under "Game" section (after "Launch")
- Add `'health'` to `VALID_APP_ROUTES` in `App.tsx`
- Add `'health'` case to `ContentArea.tsx` switch
- Add `HealthDashboardArt` illustration SVG to `PageBanner.tsx`

_Estimated complexity: Low — 6 small edits to existing files, all following established patterns_

#### Task Group 1.2: Page Shell + Summary Cards

- Create `HealthDashboardPage.tsx` in `components/pages/`
- Implement `PageBanner` header with health illustration
- Wire `useProfileHealth` hook as the sole data source
- Implement 4 summary cards in a CSS grid row:
  - **Total** (`--crosshook-color-accent`): `summary.total_count`
  - **Healthy** (`--crosshook-color-success`): `summary.healthy_count`
  - **Stale** (`--crosshook-color-warning`): `summary.stale_count`
  - **Broken** (`--crosshook-color-danger`): `summary.broken_count`
- Implement loading state (simple "Checking profiles..." text)
- Implement error state (error message + "Retry" button calling `batchValidate()`)
- Implement empty state when `total_count === 0` ("No profiles configured yet")
- Add CSS classes to `theme.css` (`crosshook-health-dashboard*`, summary card grid)

_Estimated complexity: Medium — new page component with layout and 3 state variants_

#### Task Group 1.3: Basic Profile List

- Render `summary.profiles` as a simple read-only list or table
- Columns for Phase 1: Name, Status (via `HealthBadge`), Issue Count
- Default order: Broken first, then Stale, then Healthy (hardcoded, no interactive sort)
- Row `tabIndex={0}` for keyboard accessibility
- `role="grid"` and `aria-label` on table for screen readers

_Estimated complexity: Low — static rendering of existing data_

#### Task Group 1.4: Re-check All Button

- "Re-check All" button in toolbar area above the profile list
- Wire `batchValidate()` with loading state — disable button and show "Checking..." during validation
- Add `aria-live="polite"` status region for completion announcement
- Show "Last validated: X min ago" using `summary.validated_at`

_Estimated complexity: Low — reuses existing hook method_

#### Phase 1 Exit Criteria

- Health tab visible in sidebar under "Game" section
- Summary cards show correct counts, color-coded
- Profile list shows all profiles with status badges
- Re-check All button works and shows loading state
- Empty/error/loading states all render correctly
- Keyboard accessible (Tab to navigate, Enter on re-check button)

---

### Phase 2: Interactive Table + Fix Navigation

**Goal**: Transform the static profile list into an interactive, sortable, filterable table with "Fix" actions that navigate to the profile editor. This is the "complete" dashboard for the majority of users.

**What ships**: Column sorting, status filter pills, text search, per-row "Fix" button, and additional metadata columns (last success, launch method, favorites).

**Why this is independently shippable**: Phase 1 shows the data; Phase 2 makes it actionable. After this phase, users can triage broken profiles by sorting/filtering, then jump directly to the profile editor to fix them. This is the complete diagnostic-then-act workflow.

**Prerequisite**: Phase 1 complete (routing, page shell, basic list exist).

#### Task Group 2.1: Sortable Table

- Replace static list with sortable `<table>` using `useMemo` sort logic
- Click column header to cycle sort direction: ascending, descending, none
- `aria-sort="ascending|descending|none"` on `<th>` elements
- Sort indicator arrows in header cells
- Default sort: Status (Broken first), with favorites pinned to top
- Expand columns to full set:

  | Column        | Source                          | Sortable | Phase 2 addition? |
  | ------------- | ------------------------------- | -------- | ----------------- |
  | Name          | `report.name`                   | Yes      | No (Phase 1)      |
  | Status        | `report.status`                 | Yes      | Sort is new       |
  | Issues        | `report.issues.length`          | Yes      | Sort is new       |
  | Last Success  | `metadata?.last_success`        | Yes      | New column        |
  | Launch Method | `report.launch_method`          | Yes      | New column        |
  | Favorite      | `metadata?.is_favorite`         | Yes      | New column        |
  | Source        | `metadata?.is_community_import` | No       | New column        |
  | Actions       | —                               | No       | New column        |

- Extract `formatRelativeTime` from `ProfilesPage.tsx:22` to `utils/time.ts` (second consumer justifies extraction)
- Handle `metadata === null` gracefully — show "N/A" for metadata-derived columns

_Estimated complexity: Medium — sort state management + column expansion_

#### Task Group 2.2: Filtering + Search

- Status filter pills: All / Healthy / Stale / Broken (toggle filter)
- Text search input with `useDeferredValue` for debouncing (matches on profile name)
- Show filtered count: "Showing X of Y profiles"
- Follow the `CompatibilityViewer` filter pattern for consistency

_Estimated complexity: Low-medium — follow existing pattern_

#### Task Group 2.3: Fix Navigation

- "Fix" button per table row for broken/stale profiles
- Wire `selectProfile(name)` + `onNavigate('profiles')` sequentially
- Test timing reliability — if `ProfilesPage` mounts before `selectProfile` resolves, add `pendingNavProfile` to `ProfileContext`
- For healthy profiles, the Fix button is hidden or disabled (nothing to fix)

_Estimated complexity: Low-medium — depends on timing behavior_

#### Phase 2 Exit Criteria

- Table columns are sortable (click header to toggle)
- Status filter pills filter the table
- Text search filters by profile name
- "Fix" navigates to Profile Editor with the correct profile selected
- Metadata columns render correctly or show "N/A" when metadata is null
- `formatRelativeTime` extracted to shared utility

---

### Phase 3: Gamepad Support + Trend Analysis

**Goal**: Full Steam Deck controller support for the dashboard, plus trend arrows that give returning users temporal context on health changes. Also addresses loading UX with skeleton states.

**What ships**: Y button triggers re-check, D-pad navigates table rows (already works via `tabIndex`), trend arrows on summary cards and table rows, skeleton loading placeholders.

**Why this is independently shippable**: Phases 1+2 work fully with keyboard and mouse. Phase 3 adds the controller polish that Steam Deck users need, plus visual trend context that returning users want. Both are refinements on an already-functional dashboard.

**Prerequisite**: Phase 2 complete (interactive table exists).

#### Task Group 3.1: Gamepad Y Button Re-check

- Page-local `useEffect` polling `navigator.getGamepads()` via `requestAnimationFrame`
- Detect edge press of button index 3 (Y on Xbox layout)
- Trigger `batchValidate()` on edge press when not already loading
- Requires `controllerMode` — either thread through `ContentArea` props or detect locally via gamepad API
- Add `ControllerPrompts` hint for Y button on the health page

```tsx
useEffect(() => {
  if (!controllerMode) return;
  let prevY = false;
  let rafId: number;
  function poll() {
    const gp = navigator.getGamepads()[0];
    const yPressed = gp?.buttons[3]?.pressed ?? false;
    if (yPressed && !prevY && !loading) {
      void batchValidate();
    }
    prevY = yPressed;
    rafId = requestAnimationFrame(poll);
  }
  rafId = requestAnimationFrame(poll);
  return () => cancelAnimationFrame(rafId);
}, [controllerMode, loading, batchValidate]);
```

_Estimated complexity: Low — ~20 lines of code + prop threading decision_

#### Task Group 3.2: Trend Arrows

- Per-profile trend arrow in table status column using `trendByName[name]`
- Render only for `got_worse` (red down arrow) and `got_better` (green up arrow)
- `null` and `unchanged` render nothing (matching existing `HealthBadge` behavior)
- Aggregate trend arrows on summary cards: compare current counts to cached snapshot counts
- Use same color convention as existing `HealthBadge` trend rendering

_Estimated complexity: Low — data already computed by `useProfileHealth`_

#### Task Group 3.3: Skeleton Loading States

- Skeleton placeholders for summary cards during initial loading (pulsing gray rectangles)
- Skeleton rows for table during batch validation
- Transition from cached snapshot data to live data without layout shift
- If cached snapshots are available, show them with a "Cached — checking..." label instead of skeletons

_Estimated complexity: Low-medium — CSS animation + conditional rendering_

#### Task Group 3.4: Responsive Card Layout

- CSS media query: 4 cards at >1100px, 2x2 grid at 900px breakpoint
- Test at 1280x800 (Steam Deck default resolution)
- Ensure table columns remain readable without horizontal scroll at 1280x800

_Estimated complexity: Low — CSS only_

#### Phase 3 Exit Criteria

- Y button triggers re-check on Steam Deck / controller
- Trend arrows appear on profiles whose status changed since last cached snapshot
- Summary cards show aggregate trend arrows
- Skeleton states render during loading without layout shift
- 1280x800 layout works without horizontal scroll

---

### Phase 4: Diagnostic Panels

**Goal**: Add specialized diagnostic panels for power users who manage many profiles and need categorical views. These panels are collapsible sections below the main table that surface specific diagnostic categories.

**What ships**: Issue breakdown by category, recent failures panel, launcher drift summary, community import health section.

**Why this is independently shippable**: These panels add convenience for power users but do not provide new information — they are filtered, pre-categorized views of data already visible in the Phase 2 table. They are worth building for discoverability (e.g., a user might not know to filter by `is_community_import`) but are strictly additive.

**Prerequisite**: Phase 2 complete (table with metadata columns exists). Phase 3 is NOT required — these panels work without controller support or trend arrows.

#### Task Group 4.1: Issue Breakdown by Category

- Aggregate `HealthIssue` items across all profiles by `field` prefix category:
  - `game.executable_path` → "Missing Executables"
  - `steam.proton_path` / `runtime.proton_path` → "Missing Proton Paths"
  - `steam.compatdata_path` / `runtime.prefix_path` → "Inaccessible Directories"
  - `trainer.path` / `injection.dll_paths[*]` → "Trainer/DLL Issues"
  - Other → "Other Issues"
- Render as `CollapsibleSection` with category rows showing count + CSS width bar chart
- Clicking a category filters the main table to show only profiles with that issue type
- `useMemo` keyed on `summary.profiles` for the aggregation

_Estimated complexity: Low — derived data, simple rendering_

#### Task Group 4.2: Recent Failures Panel

- Filter `summary.profiles` where `metadata?.failure_count_30d > 0`
- Render as `CollapsibleSection` (`defaultOpen={false}`)
- Show: profile name, failure count, last success date
- Empty state when no profiles have recent failures
- Use `failure_count_30d > 0` threshold (broader than `HealthBadge`'s `>= 2` for badge display)

_Estimated complexity: Low — filtered view of existing data_

#### Task Group 4.3: Launcher Drift Summary

- Filter `summary.profiles` where `metadata?.launcher_drift_state !== null`
- Render drift state using the existing message map:
  - `missing` → "Exported launcher not found — re-export recommended"
  - `moved` → "Exported launcher has moved — re-export recommended"
  - `stale` → "Exported launcher may be outdated — re-export recommended"
- `CollapsibleSection` (`defaultOpen={false}`)

_Estimated complexity: Low — filtered view of existing data_

#### Task Group 4.4: Community Import Health

- Filter `summary.profiles` where `metadata?.is_community_import === true` and status is not healthy
- Add contextual note: "Imported profiles often need path adjustments for your system."
- `CollapsibleSection` (`defaultOpen={false}`)

_Estimated complexity: Low — filtered view of existing data_

#### Phase 4 Exit Criteria

- Issue breakdown shows categories with counts and bar charts
- Clicking a category filters the main table
- Recent failures panel surfaces profiles with launch failures
- Launcher drift summary identifies profiles needing re-export
- Community import section contextualizes imported profiles
- All panels collapse gracefully and show empty states when no data matches

---

### Phase Dependency Graph

```
Phase 1 (Minimal Dashboard)
  ├── 1.1 (Routing) → blocks everything else
  ├── 1.2 (Shell + Cards) → depends on 1.1
  ├── 1.3 (Basic List) → depends on 1.2
  └── 1.4 (Re-check) → depends on 1.2
      1.3 and 1.4 are independent of each other

Phase 2 (Interactive Table) — depends on Phase 1 complete
  ├── 2.1 (Sortable Table) → depends on 1.3
  ├── 2.2 (Filtering) → depends on 2.1
  └── 2.3 (Fix Nav) → depends on 2.1
      2.2 and 2.3 are independent of each other

Phase 3 (Gamepad + Trends) — depends on Phase 2 complete
  ├── 3.1 (Y Button) → depends on 1.4
  ├── 3.2 (Trend Arrows) → depends on 2.1
  ├── 3.3 (Skeletons) → depends on 1.2 + 1.3
  └── 3.4 (Responsive) → depends on 1.2
      3.1–3.4 are all independent of each other

Phase 4 (Diagnostic Panels) — depends on Phase 2 complete (NOT Phase 3)
  ├── 4.1 (Issue Breakdown) → depends on 2.1
  ├── 4.2 (Recent Failures) → depends on 2.1
  ├── 4.3 (Launcher Drift) → depends on 2.1
  └── 4.4 (Community Imports) → depends on 2.1
      4.1–4.4 are all independent of each other

Note: Phases 3 and 4 are independent of each other and can be done in either order or in parallel.
```

---

## Implementation Recommendations

### 1. Technical Approach

#### Data Layer: Use `useProfileHealth` directly

The existing hook provides everything the dashboard needs:

- `summary` (aggregate counts + per-profile enriched reports)
- `healthByName`, `trendByName`, `staleInfoByName` (lookup maps)
- `cachedSnapshots` (instant badge status before live scan)
- `batchValidate()` (re-check trigger)
- `loading`, `error` (state indicators)

Call `useProfileHealth()` directly in `HealthDashboardPage`. No wrapper hook, no context lift. The hook's startup event listener (`profile-health-batch-complete`) already deduplicates: if `ProfilesPage` and `HealthDashboardPage` both mount, both receive the same Tauri event independently without triggering duplicate IPC calls.

**File**: `src/crosshook-native/src/hooks/useProfileHealth.ts`

#### Routing: Extend `AppRoute` union

Add `'health'` to the `AppRoute` type and wire it through:

1. `Sidebar.tsx:12` — add to `AppRoute` union type
2. `Sidebar.tsx:32` — add to `SIDEBAR_SECTIONS` under the "Game" section (after "Launch")
3. `App.tsx:14` — add to `VALID_APP_ROUTES`
4. `ContentArea.tsx:34` — add case to `renderPage()` switch
5. `SidebarIcons.tsx` — add `HealthIcon` SVG component
6. `PageBanner.tsx` — add `HealthDashboardArt` illustration SVG

**Sidebar placement**: Under "Game" section, after "Launch" — the health dashboard is a per-session triage step that naturally precedes or follows launching games.

#### Table: Hand-rolled with `useMemo` + `useDeferredValue`

Follow the `CompatibilityViewer` pattern: native `<table>` element with `useMemo` for sort/filter logic and `useDeferredValue` for search input debouncing. This approach:

- Adds zero bundle weight (vs. ~15 kB for TanStack Table)
- Is consistent with existing codebase patterns
- Is sufficient for the expected 5–30 profile rows
- Supports single-column sort toggle (click header to cycle `asc`/`desc`/`none`)

Full column spec (Phase 2):

| Column        | Source                          | Sortable                    | Notes                                                |
| ------------- | ------------------------------- | --------------------------- | ---------------------------------------------------- |
| Name          | `report.name`                   | Yes                         | Primary identifier                                   |
| Status        | `report.status`                 | Yes (default: Broken first) | Render via `HealthBadge`                             |
| Issues        | `report.issues.length`          | Yes                         | Numeric count                                        |
| Last Success  | `metadata?.last_success`        | Yes                         | `formatRelativeTime()` — extract from `ProfilesPage` |
| Launch Method | `report.launch_method`          | Yes                         | `steam_applaunch` / `proton_run` / `native`          |
| Trend         | `trendByName[name]`             | No                          | Arrow icon, Phase 3 addition                         |
| Favorite      | `metadata?.is_favorite`         | Yes                         | Star indicator, favorites first when sorted          |
| Source        | `metadata?.is_community_import` | No                          | "Community" badge if true                            |
| Actions       | —                               | No                          | "Fix" button navigates to ProfilesPage               |

**Phase 1 shows only**: Name, Status, Issue Count (3 columns, no interactivity).
**Phase 2 adds**: All columns + sort/filter/search + Fix button.
**Phase 3 adds**: Trend column with arrows.

#### Fix Navigation: Sequential calls, then `pendingNavProfile` if needed

Call `selectProfile(name)` and `onNavigate('profiles')` sequentially. `selectProfile` already triggers a profile load via the `useProfile` hook, and switching routes will mount `ProfilesPage` which reads the selected profile from context.

```tsx
const { selectProfile } = useProfileContext();
function handleFix(profileName: string) {
  selectProfile(profileName);
  onNavigate('profiles');
}
```

If timing is unreliable (test during Phase 2.3), add a `pendingNavProfile: string | null` field to `ProfileContext` as a fallback.

#### Gamepad Y Button: Page-local `useEffect` (Phase 3)

Add a `useEffect` in `HealthDashboardPage` that polls `navigator.getGamepads()` via `requestAnimationFrame` and detects edge presses of button index 3 (Y). This avoids modifying the shared `useGamepadNav` hook for a single-page need.

**Future improvement**: If more pages need custom button mappings, refactor into `useGamepadNav` with an extensible callback map. Not needed for the dashboard alone.

### 2. Technology Choices

| Choice           | Decision                         | Rationale                                                                      |
| ---------------- | -------------------------------- | ------------------------------------------------------------------------------ |
| Table library    | None (hand-rolled)               | Profile lists are small, existing pattern works, zero bundle cost              |
| Chart library    | None (CSS bars + Unicode arrows) | Trend arrows exist in `HealthBadge`; issue breakdown uses CSS width bars       |
| Gamepad library  | None (existing `useGamepadNav`)  | Already covers D-pad, A/B, L1/R1; Y button handled locally                     |
| State management | `useState` + `useProfileHealth`  | No cross-component state beyond what `ProfileContext` provides                 |
| Accessibility    | Manual WAI-ARIA attributes       | `role="grid"`, `aria-sort`, `aria-live="polite"` on status regions             |
| New dependency   | **None**                         | TanStack Table is a reasonable future addition if sort/filter complexity grows |

### 3. Quick Wins

1. **Extract `formatRelativeTime`** from `ProfilesPage.tsx:22` to `utils/time.ts` — second consumer (dashboard) justifies extraction. Do this in Phase 2.1.
2. **Reuse `HealthBadge` directly** in table status column — zero new rendering code. Available from Phase 1.
3. **Derive all secondary panels from `summary.profiles`** — filter by `metadata?.failure_count_30d > 0` for recent failures, `metadata?.launcher_drift_state !== null` for drift, `metadata?.is_community_import === true` for community imports. No new data fetching. Phase 4 scope.
4. **Use existing CSS variables** for all color coding — `--crosshook-color-success`, `--crosshook-color-warning`, `--crosshook-color-danger`, `--crosshook-color-accent`. From Phase 1.

---

## Improvement Ideas

### Related Features (post-Phase 4)

1. **Profile health notification badge on sidebar**: Show a small red dot on the Health sidebar item when `broken_count > 0`. Low effort, high visibility — users see health issues without navigating to the page.

2. **Health-aware launch gating**: On the Launch page, show a warning when the selected profile has broken health status. "This profile has issues that may prevent launching — view details on Health page." Already have the data via `useProfileHealth`.

3. **Export health report**: Add a "Copy report" button that serializes the health summary to a text/markdown format for pasting into support channels or GitHub issues. Useful for users reporting bugs.

4. **Scheduled re-check**: Auto-trigger `batchValidate()` when the app regains focus (e.g., after a system update) if the last check is older than the staleness threshold (7 days). Uses the existing `staleInfoByName` data.

### Future Enhancements

5. **Inline issue remediation hints**: For common issues (missing Proton path after update), show a "Quick fix: select new Proton version" button that opens the specific field in the profile editor. Extends beyond read-only but high value.

6. **Health history sparkline**: Store historical health snapshots (already persisted in `health_snapshots` SQLite table) and render a 7-day sparkline per profile showing status over time. Would require a new backend query but the data is already being collected.

7. **Batch fix for common issues**: When multiple profiles share the same broken Proton path (e.g., after a Proton update), offer "Update Proton path for all affected profiles" as a bulk action. Significant scope expansion but addresses a real pain point.

8. **Keyboard shortcut to Health page**: Bind `Ctrl+H` or `F5` to navigate to the Health page and trigger a re-check. Power user productivity feature.

### Optimization Opportunities

9. **Memoize issue categorization**: The issue breakdown panel groups issues by field category. Wrap this in `useMemo` keyed on `summary.profiles` to avoid re-computation on every render. Phase 4 scope.

10. **Defer secondary panels**: Use conditional rendering for Phase 4 panels. They default to collapsed (`defaultOpen={false}`) — defer their data derivation until the user opens them.

---

## Risk Assessment

### Technical Risks

| Risk                                                                        | Likelihood | Impact | Mitigation                                                                                                                                                                                                                                     | Affects Phase                                            |
| --------------------------------------------------------------------------- | ---------- | ------ | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------- |
| **`EnrichedHealthSummary` type mismatch on frontend**                       | High       | Medium | The frontend `HealthCheckSummary` type in `useProfileHealth` may not include `metadata` fields. Verify by logging `summary.profiles[0]` at runtime. If metadata is present but untyped, update the hook's type to use `EnrichedHealthSummary`. | Phase 1 (must verify before metadata columns in Phase 2) |
| **Fix navigation timing race**                                              | Medium     | Low    | If `selectProfile` and `onNavigate` execute in the same tick, `ProfilesPage` may mount before profile load completes. Test in Phase 2.3. If unreliable, add `pendingNavProfile` to `ProfileContext`.                                           | Phase 2                                                  |
| **Dual `useProfileHealth` instantiation causes redundant batch validation** | Medium     | Low    | Startup event deduplication prevents duplicate calls on mount. The 700ms fallback timer only fires if the event was missed. Monitor with `tracing::info` on the backend.                                                                       | Phase 1                                                  |
| **Gamepad Y button polling conflicts with main hook**                       | Low        | Low    | Page-local `useEffect` reads `navigator.getGamepads()` independently from `useGamepadNav`. Both use `requestAnimationFrame` but don't share state. No conflict — gamepad state is read-only.                                                   | Phase 3                                                  |
| **Table performance with 100+ profiles**                                    | Low        | Medium | Unlikely scenario (typical: 5–30). If it occurs, add `useDeferredValue` for filter input and consider `@tanstack/react-virtual`.                                                                                                               | Phase 2                                                  |

### Integration Challenges

1. **Sidebar section placement**: Adding a 7th route increases visual density. The "Game" section grows to 3 items (Profiles, Launch, Health). Acceptable, but monitor. Affects Phase 1.

2. **TypeScript exhaustive switch**: `ContentArea.tsx` uses `const _exhaustive: never = route`. Adding `'health'` to `AppRoute` causes a compile error until the switch case is added. By design — all routing changes must happen atomically. Affects Phase 1.

3. **CSS namespace**: New CSS classes should follow the `crosshook-health-dashboard*` namespace. Add to `theme.css` rather than creating a new CSS file. Affects Phase 1.

4. **`controllerMode` prop threading**: `HealthDashboardPage` needs `controllerMode` for Y button (Phase 3). Either thread through `ContentArea` props or detect locally via gamepad API. Can defer this decision until Phase 3 since Phase 1+2 don't need it.

### Performance Considerations

- **Batch validation I/O**: ~150 filesystem stat calls for 30 profiles. Completes in <100ms on SSD.
- **MetadataStore queries**: 7 SQL queries against local SQLite. All indexed. <20ms for 30 profiles.
- **React rendering**: Summary cards + 30-row table is trivial. No virtualization needed.
- **Memory**: `EnrichedHealthSummary` for 30 profiles is <50 KB.

### Security Considerations

(From security-researcher findings)

- **No critical findings**: Dashboard is read-only, no new IPC surface, no filesystem mutations.
- **WARNING: XSS via profile name**: All rendering must use JSX interpolation (React auto-escapes). Never use `dangerouslySetInnerHTML` for profile names or paths.
- **ADVISORY: Path sanitization already applied**: `sanitize_display_path` replaces `$HOME` with `~` before IPC.
- **ADVISORY: Error messages may contain filesystem paths**: Consider displaying in monospace code block without interpreting HTML.

---

## Alternative Approaches

### Option A: Hand-Rolled Table (Recommended)

**Approach**: Native `<table>` with `useMemo` sort/filter, `useDeferredValue` for search debouncing. Follow the `CompatibilityViewer` pattern.

**Pros**:

- Zero new dependencies
- Consistent with existing codebase patterns
- Full control over ARIA attributes and gamepad focus behavior
- Smaller bundle size

**Cons**:

- Manual sort direction state management
- Manual multi-column sort requires more code
- No built-in column resize or row virtualization

**Effort**: Low-medium. Sort/filter for a single-column-at-a-time approach is ~50 lines of code.

### Option B: TanStack Table v8

**Approach**: Add `@tanstack/react-table` (~15 kB gzipped), use `useReactTable` with `getSortedRowModel` and `getFilteredRowModel`.

**Pros**:

- Multi-column sort out of the box
- Pagination model available if needed
- Type-safe column definitions
- Industry-standard headless table

**Cons**:

- New dependency (+15 kB gzipped)
- First third-party table library in the project — sets a precedent
- Still requires manual ARIA attributes (headless, no built-in a11y)
- Rendering is still hand-written JSX — the library only manages state

**Effort**: Low-medium. ~40 lines of column definitions + table hook setup.

### Option C: HealthContext Provider

**Approach**: Lift `useProfileHealth` state into a React context, wrap in `App.tsx`. All consumers share one hook instance.

**Pros**:

- Eliminates any possibility of duplicate batch validation calls
- Single source of truth for health state across pages
- Simpler consumer components (just `useHealthContext()`)

**Cons**:

- Adds provider nesting in `App.tsx` (already has `ProfileProvider` and `PreferencesProvider`)
- All pages re-render when health state changes (mitigated by memoization)
- Increases coupling between pages and health lifecycle
- The startup event already prevents duplicate calls in practice

**Effort**: Medium. Requires refactoring `useProfileHealth` into a provider pattern, updating `ProfilesPage` to consume from context instead of hook.

### Recommendation

**Option A (hand-rolled table).** It aligns with existing patterns, adds no dependencies, and is sufficient for the expected data volume. If multi-column sort or pagination becomes a real need, upgrade to Option B — the migration is straightforward since the column definitions are similar. Defer Option C unless performance profiling reveals actual duplicate calls.

---

## Key Decisions Needed

1. **Table library**: Hand-rolled (recommended) vs. TanStack Table. Decide before Phase 2.1.

2. **Sidebar section placement**: "Health" under "Game" section (3 items: Profiles, Launch, Health) or as a new standalone "Diagnostics" section? Recommendation: under "Game" for discoverability. Decide before Phase 1.1.

3. **Fix navigation mechanism**: Simple sequential `selectProfile()` + `onNavigate()` (test first) or explicit `pendingNavProfile` state in `ProfileContext`? Test during Phase 2.3 and decide based on timing reliability.

4. **`controllerMode` prop threading**: Pass through `ContentArea` props, or detect locally via gamepad API? Recommendation: thread through `ContentArea` for consistency. Defer decision until Phase 3.

5. **Secondary panels as separate sections vs. table filters**: Should launcher drift, community imports, and recent failures be standalone collapsible panels or integrated as table filter presets? Recommendation: both — panels as summaries with "Show in table" action. Decide before Phase 4.

6. **Phase 3 vs. Phase 4 ordering**: Phases 3 and 4 are independent. If Steam Deck support is the higher priority, do Phase 3 first. If power user diagnostics are more urgent, do Phase 4 first. Can also be done in parallel by different contributors.

---

## Open Questions

1. **EnrichedHealthSummary vs. HealthCheckSummary on the frontend**: The `useProfileHealth` hook types its summary as `HealthCheckSummary` but the backend command `batch_validate_profiles` returns `EnrichedHealthSummary` with `metadata` fields. Need to verify at runtime whether metadata is accessible through the current hook. **Must resolve before Phase 2** (metadata columns depend on it). Can investigate during Phase 1.

2. **Sentinel profile `"<unknown>"` handling**: When `ProfileStore.list()` fails, the batch check returns a sentinel entry with name `"<unknown>"`. Should the dashboard detect this and render a system-level error banner? Recommendation: yes. Implement in Phase 1.2 (empty/error states).

3. **Health tab visibility when zero profiles exist**: Should the Health tab always appear in the sidebar? Recommendation: yes, with an empty state. The tab's presence reminds users the feature exists.

4. **Re-check All scope**: Does `batchValidate` also refresh cached snapshots? Yes — `batch_validate_profiles` persists new snapshots via `upsert_health_snapshot`. Cached data is implicitly updated. Confirmed from code analysis.

5. **Failure count threshold for "Recent Failures" panel (Phase 4)**: Use `failure_count_30d > 0` (any failure) or `failure_count_30d >= 2` (matching `HealthBadge`)? Recommendation: `> 0` for the panel (show all), `>= 2` for badge indicators.

6. **Table default sort**: Sort by status (Broken first) or by name? Recommendation: status by default, favorites pinned to top regardless of sort. Implement in Phase 2.1.
