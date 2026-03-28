# Health Dashboard Page — Task Structure

This document breaks the feature into implementation tasks respecting the 3-phase delivery plan from `research-technical.md` and `feature-spec.md`. Each task is a self-contained unit of work that can be reviewed independently. Dependencies are explicit.

---

## Phase 1: Scaffold + Summary (MVP)

**Goal:** A routable, working page with summary cards, a flat profile list, Re-check All, and Fix navigation. All P1 success criteria from `feature-spec.md` must be met before merging.

### Task P1-A: Add HealthIcon and HealthDashboardArt

**Files:** `SidebarIcons.tsx`, `PageBanner.tsx`
**Depends on:** nothing
**Blocks:** P1-B

Add the two decorative SVG assets used by routing and the page banner.

- Append `HealthIcon` export to `src/components/icons/SidebarIcons.tsx`
  - Follow the exact `defaults` spread pattern (20×20 viewBox, stroke, no fill)
  - Suggested geometry: a pulse/heartbeat line or shield-check — must read at 20×20 stroke weight 1.5
- Append `HealthDashboardArt` export to `src/components/layout/PageBanner.tsx`
  - Follow `SVG_DEFAULTS` spread (200×120 viewBox, stroke weight 1)
  - Keep opacities in the 0.1–0.5 range, consistent with other art exports in that file

**Acceptance:** Both exports compile and TypeScript reports no errors. No behavior change on existing routes.

---

### Task P1-B: Wire the `health` route (atomic 5-file edit)

**Files:** `Sidebar.tsx`, `App.tsx`, `ContentArea.tsx`, `SidebarIcons.tsx` (already done in P1-A), `HealthDashboardPage.tsx` (stub)
**Depends on:** P1-A
**Blocks:** P1-C

All five changes must ship together — a partial apply breaks TypeScript compilation.

1. **`Sidebar.tsx` line 12** — extend `AppRoute` union: `| 'health'`
2. **`Sidebar.tsx` line 53–60** — add `health: 'Health'` to `ROUTE_LABELS`
3. **`Sidebar.tsx` line 32–51** — append a new section after the existing `Community` section:

   ```ts
   { label: 'Dashboards', items: [{ route: 'health', label: 'Health', icon: HealthIcon }] }
   ```

4. **`Sidebar.tsx` imports** — add `HealthIcon` import from `../icons/SidebarIcons`
5. **`App.tsx` line 14–21** — add `health: true` to `VALID_APP_ROUTES`
6. **`ContentArea.tsx` imports** — add `HealthDashboardPage` import from `../pages/HealthDashboardPage`
7. **`ContentArea.tsx` line 34–51** — add `case 'health': return <HealthDashboardPage onNavigate={onNavigate} />;` before `default`
8. **Create stub** `src/components/pages/HealthDashboardPage.tsx`:

   ```tsx
   import type { AppRoute } from '../layout/Sidebar';
   export function HealthDashboardPage({ onNavigate }: { onNavigate?: (route: AppRoute) => void }) {
     return <div>Health Dashboard (coming soon)</div>;
   }
   export default HealthDashboardPage;
   ```

**Acceptance:** `npm run dev` starts without TypeScript errors. "Health" appears in the sidebar under "Dashboards". Clicking it renders the stub. The `never` guard in `ContentArea.tsx:48` does not fire.

**Gotcha:** `VALID_APP_ROUTES` is a `Record<AppRoute, true>` — TypeScript does NOT enforce completeness here (unlike the switch). Manually verify it includes `health: true` after editing.

---

### Task P1-C: Implement HealthDashboardPage — banner, summary cards, loading/error/empty states

**Files:** `HealthDashboardPage.tsx`
**Depends on:** P1-B
**Blocks:** P1-D

Replace the stub with the page shell and summary section.

**Component structure:**

```
HealthDashboardPage
  ├─ PageBanner (eyebrow="Dashboards", title="Profile Health", illustration=HealthDashboardArt)
  ├─ [error banner if error !== null]
  ├─ SummaryCards (local function component)
  │    ├─ Total card (.crosshook-card)
  │    ├─ Healthy card (green: --crosshook-color-success)
  │    ├─ Stale card (amber: --crosshook-color-warning)
  │    └─ Broken card (red: --crosshook-color-danger)
  └─ CollapsibleSection "Re-check" (.crosshook-panel)
       └─ Re-check All button + last validated timestamp
```

**Implementation rules:**

- Call `useProfileHealth()` at the top of `HealthDashboardPage`; do not call `invoke()` directly
- Call `useProfileContext()` for `selectProfile` — available at this level because `ProfileProvider` wraps `AppShell`
- Guard all `summary` accesses: `summary?.broken_count ?? 0`, `summary?.profiles ?? []`
- Loading state: when `loading && !summary`, render a "Checking profiles…" message (not an empty table)
- Cached-first display: `cachedSnapshots` populates before the live validate completes — render snapshot counts as a placeholder when `summary` is null but `cachedSnapshots` is non-empty
- Error state: `error !== null` → show `<p role="alert" className="crosshook-danger">`
- Empty state: `summary?.total_count === 0` → "No profiles yet" message
- All-healthy state: `summary?.broken_count === 0 && summary?.stale_count === 0 && summary?.total_count > 0` → positive "All profiles are healthy" message
- "Re-check All" button: `disabled={loading}`, label `{loading ? 'Checking…' : 'Re-check All'}`, wired to `() => void batchValidate()`
- Last checked: `summary?.validated_at` → `formatRelativeTime()` (inline a copy for P1; will be extracted in P2)

**CSS:** Summary cards use a CSS grid layout. Use `.crosshook-card` class. Color accent each card with inline `style={{ borderTopColor: 'var(--crosshook-color-danger)' }}` or similar — do not create new CSS classes unless unavoidable.

**Acceptance:** Summary cards show correct counts. Loading state shows while hook is fetching. Error/empty/all-healthy states render correctly. Re-check button triggers re-validate and shows "Checking…" while in-flight.

---

### Task P1-D: Implement profile list table with Fix navigation

**Files:** `HealthDashboardPage.tsx`
**Depends on:** P1-C
**Blocks:** P2-A

Add the profile list `<table>` inside a `CollapsibleSection` below the Re-check section.

**Table columns (P1):** Profile Name | Status | Issues
**Default sort:** broken first, then stale, then healthy; alphabetical within each group

**Implementation rules:**

- Derive sorted list via `useMemo`: sort `summary?.profiles ?? []` by status rank (broken=2, stale=1, healthy=0) descending then by `name` ascending
- Supplement with `cachedSnapshots` entries for profiles not yet in live `summary` — display their cached status with a visual "cached" indicator (e.g. dimmed badge or italic name)
- Use `<HealthBadge report={report} />` for the status cell — drop-in, no props massaging needed
- Issue count: `report.issues.length`
- Fix button per broken/stale row:

  ```tsx
  <button
    type="button"
    className="crosshook-button crosshook-button--secondary"
    onClick={() => {
      void selectProfile(report.name);
      onNavigate?.('profiles');
    }}
  >
    Fix
  </button>
  ```

- Rows need `tabIndex={0}` for gamepad D-pad traversal (already in `FOCUSABLE_SELECTOR`)
- Row `onKeyDown`: Enter key triggers Fix action for the focused row
- Healthy rows: show Fix button as disabled or omit it entirely (BR-01: dashboard is read-only; Fix is only meaningful for broken/stale)
- `aria-sort` not required in P1 (no sorting UI yet); add in P2

**Enriched metadata:** The Tauri command returns `EnrichedHealthSummary` but the hook types it as `HealthCheckSummary`. Cast where needed:

```tsx
const enriched = report as EnrichedProfileHealthReport;
const metadata = enriched.metadata ?? null;
```

For P1, metadata is not displayed — but null-guard the cast so P2 additions don't crash.

**Acceptance:** All profiles appear in the table. Broken/stale profiles are at the top. Clicking Fix selects the profile and navigates to Profiles tab with that profile active. D-pad traverses rows. All P1 success criteria from `feature-spec.md` are met.

---

## Phase 2: Table + Patterns

**Goal:** The profile list becomes a fully interactive health table. Issue patterns become discoverable via breakdown and search.

### Task P2-A: Extract `formatRelativeTime` to shared util

**Files:** `src/utils/format.ts` (new), `ProfilesPage.tsx`, `HealthDashboardPage.tsx`
**Depends on:** P1-D
**Blocks:** P2-B, P2-C

- Create `src/utils/format.ts` with the `formatRelativeTime` function (source: `ProfilesPage.tsx:22–36`)
- Update `ProfilesPage.tsx` to import from `../utils/format` instead of its local definition — remove the local copy
- Update `HealthDashboardPage.tsx` to import from `../../utils/format` instead of its inline copy

**Acceptance:** Both pages compile and display relative timestamps identically to before.

---

### Task P2-B: Add sort and filter controls to the table

**Files:** `HealthDashboardPage.tsx`
**Depends on:** P2-A
**Blocks:** P2-C

Add page-level sort/filter state and a toolbar row above the table.

**New local types (define at top of file):**

```ts
type SortField = 'name' | 'status' | 'issues' | 'last_success' | 'launch_method' | 'failures' | 'favorite';
type SortDirection = 'asc' | 'desc';
interface TableSort {
  field: SortField;
  direction: SortDirection;
}
type StatusFilter = 'all' | HealthStatus;
```

**State:**

```ts
const [sort, setSort] = useState<TableSort>({ field: 'status', direction: 'desc' });
const [statusFilter, setStatusFilter] = useState<StatusFilter>('all');
const [nameQuery, setNameQuery] = useState('');
const deferredNameQuery = useDeferredValue(nameQuery);
```

**Toolbar:** Search input (`.crosshook-input`) + status filter `<select>` (`.crosshook-select`). Follow `CompatibilityViewer.tsx:153–203` for the filter layout pattern inside a `CollapsibleSection`.

**Table `<thead>`:** Replace static column headers with sortable `<button>` elements. Each column header button:

- Shows current sort direction indicator when active (`↑` / `↓`)
- Sets `aria-sort="ascending"` / `aria-sort="descending"` / `aria-sort="none"` on `<th>`
- Clicking the active column toggles direction; clicking a new column sets it as active ascending

**Sorted+filtered list via `useMemo`:**

- Filter by `statusFilter` first, then by `deferredNameQuery`, then sort
- Sort comparators for each `SortField` — status rank for `status`, string comparison for `name`, numeric for `issues` and `failures`, ISO date string comparison for `last_success`

**New columns for P2:** Last Success (from metadata), Launch Method, Failures/30d
Use the `EnrichedProfileHealthReport` cast pattern; show "N/A" when metadata is null (BR-03).

**Acceptance:** Sort toggles work on all columns. Status filter hides non-matching rows. Name search updates as the user types (with `useDeferredValue` deferral). `aria-sort` attributes are correct.

---

### Task P2-C: Add row expansion and single-profile re-check

**Files:** `HealthDashboardPage.tsx`
**Depends on:** P2-B
**Blocks:** P2-D

Add per-row expansion showing issue details and a single re-check action.

**State:** `const [expandedProfile, setExpandedProfile] = useState<string | null>(null);`

**Row expansion pattern:** When a row is expanded, insert an additional `<tr>` immediately below it containing a `<td colSpan={columnCount}>` with:

- Issue list matching `ProfilesPage.tsx:548–559` style (field, path, message, remediation)
- Metadata context: last success, total launches, failure count (if metadata available)
- Single re-check button: `onClick={() => void revalidateSingle(report.name)}` — use `revalidateSingle` from the hook

**Toggle:** Row click or Enter key toggles expansion. Already-expanded row collapses. The expanded state resets when `summary` refreshes (wipe `expandedProfile` state on `batchValidate` call).

**Accessibility:** Expanded row's trigger `<tr>` gets `aria-expanded={isExpanded}`.

**Acceptance:** Clicking a row expands/collapses its issue details. Single re-check updates only that row's data. Column count matches between `<thead>` and `colSpan`.

---

### Task P2-D: Add Issue Breakdown panel

**Files:** `HealthDashboardPage.tsx`
**Depends on:** P2-A
**Blocks:** P3-A
**Note:** Can be developed in parallel with P2-B/P2-C after P2-A

Add a `CollapsibleSection "Issue Breakdown"` above the profiles table, showing aggregated issue counts by category.

**New types:**

```ts
type IssueCategory =
  | 'missing_executable'
  | 'missing_trainer'
  | 'missing_dll'
  | 'missing_proton'
  | 'missing_prefix'
  | 'missing_compatdata'
  | 'inaccessible_path'
  | 'optional_path'
  | 'other';

interface IssueCategoryCount {
  category: IssueCategory;
  label: string;
  count: number;
  severity: HealthIssueSeverity;
}
```

**`categorizeIssue(issue: HealthIssue): IssueCategory`** — map `issue.field` prefix:

- `game.executable_path` → `missing_executable`
- `trainer.path` or `trainer.dll` → `missing_trainer` / `missing_dll`
- `steam.proton_path` / `runtime.proton_path` → `missing_proton`
- `steam.prefix_path` → `missing_prefix`
- `steam.compatdata_path` → `missing_compatdata`
- Contains `path` and message includes `permission`/`access` → `inaccessible_path`
- Severity `info` → `optional_path`
- Everything else → `other`

**Aggregation via `useMemo`:** Iterate `summary?.profiles.flatMap(p => p.issues)`, categorize each, count by category.

**Render:** A horizontal chip/badge list or simple grid. Each category: label + count badge using `.crosshook-status-chip`. Only show categories with count > 0.

**Acceptance:** Counts match the raw issue data. Panel is collapsible (defaultOpen) and shows "No issues" when count is zero.

---

## Phase 3: Trends + Polish

**Goal:** Add historical context, per-profile signals, and gamepad Y-button. All P3 user stories from `feature-spec.md` must be met.

### Task P3-A: Add trend arrows to summary cards and table rows

**Files:** `HealthDashboardPage.tsx`
**Depends on:** P2-D
**Blocks:** P3-B

**Summary card trends:** Compare current `summary.broken_count` and `summary.stale_count` to the aggregate computed from `cachedSnapshots`. Show a `↑` (worse) or `↓` (better) arrow next to each count using `--crosshook-color-warning` / `--crosshook-color-success`. Only render when there is a meaningful delta.

**Table row trends:** The hook already provides `trendByName`. Wire it to `HealthBadge`:

```tsx
<HealthBadge report={report} trend={trendByName[report.name] ?? null} />
```

`HealthBadge` already handles `trend` rendering — no changes to that component needed.

**Acceptance:** Profiles that worsened since last snapshot show a downward trend in the table. The summary card for broken count shows an up arrow when broken_count increased vs. cached aggregate.

---

### Task P3-B: Add Recent Failures, Launcher Drift, and Community Import Health panels

**Files:** `HealthDashboardPage.tsx`
**Depends on:** P3-A
**Blocks:** P3-C

Add three `CollapsibleSection` panels below the profiles table, all `defaultOpen={false}`.

**Recent Failures** (BR-06: `failure_count_30d > 0`):

- Filter enriched profiles where `metadata?.failure_count_30d > 0`
- Sort descending by `failure_count_30d`
- Show profile name, failure count, last success (via `formatRelativeTime`)
- Cast: `report as EnrichedProfileHealthReport`, guard `metadata ?? null`

**Launcher Drift Summary** (BR-08: `launcher_drift_state` in `['missing', 'moved', 'stale']`):

- Filter enriched profiles where `metadata?.launcher_drift_state` is one of those values
- Show profile name, drift state label, and a "Re-export" hint (not an action — BR-01, read-only page)

**Community Import Health:**

- Filter enriched profiles where `metadata?.is_community_import === true` and `status !== 'healthy'`
- Annotate each with: "Paths may need adjustment for your system" (per spec US-3.4)
- Show profile name, status badge, issue count

**Acceptance:** Panels appear/hide based on data. Empty panels show a "None" message. Metadata-null profiles never cause crashes (BR-03).

---

### Task P3-C: Favorites flag and gamepad Y-button

**Files:** `HealthDashboardPage.tsx`
**Depends on:** P3-B
**Blocks:** nothing (final task)

**Favorites star (US-3.5):**

- In the table row, check `(report as EnrichedProfileHealthReport).metadata?.is_favorite === true`
- Render a `★` character (or inline SVG star) in a dedicated column when true; empty cell otherwise
- `aria-label="Favorited"` on the star element

**Y-button gamepad binding (US-3.6):**
The Y button is gamepad button index 3 (standard layout). Add a `useEffect` that registers a gamepad polling handler:

```ts
useEffect(() => {
  if (!controllerMode) return;
  // ...poll gamepad button 3 (Y), on press: void batchValidate()
}, [controllerMode, batchValidate]);
```

The page does not have `controllerMode` directly — get it from `useGamepadNav` called at page level OR pass it down from `App.tsx`. The simpler path: check `document.documentElement.hasAttribute('data-crosshook-controller-mode')` within a gamepad poll loop scoped to this page. Keep the polling minimal — one `requestAnimationFrame` loop that only fires on Y-press edge detection.

**Acceptance:** Favorited profiles show a star in the table. Y-button triggers Re-check All when controller mode is active (verify on Steam Deck or with a controller connected).

---

## Cross-Cutting Constraints

### Must hold across all phases

- **No `dangerouslySetInnerHTML`** — profile names and paths are user data; JSX interpolation only
- **All metadata access null-guarded** — `metadata?.field ?? fallback` everywhere (BR-03)
- **`useProfileHealth()` called once at the page root** — not inside child components or loops
- **`useProfileContext()` consumed for `selectProfile`** — never call the Tauri `load_profile` command directly
- **No new npm dependencies** — the feature spec explicitly bans new libraries for v1
- **`formatRelativeTime` lives in `src/utils/format.ts` from P2 onward** — do not duplicate it

### TypeScript strictness

- `EnrichedProfileHealthReport` is a type-cast pattern, not a type-safe generic — always null-guard after the cast
- `AppRoute` union change in `Sidebar.tsx` cascades to `VALID_APP_ROUTES` (App.tsx) which TypeScript does NOT enforce — verify manually
- The `never` guard at `ContentArea.tsx:48` DOES enforce the switch — trust it

### File creation summary

| File                                           | Phase | Purpose                                  |
| ---------------------------------------------- | ----- | ---------------------------------------- |
| `src/components/pages/HealthDashboardPage.tsx` | P1-B  | New page (stub), grows across all phases |
| `src/utils/format.ts`                          | P2-A  | Shared `formatRelativeTime`              |

### File modification summary

| File                                    | Phase          | Changes                                                  |
| --------------------------------------- | -------------- | -------------------------------------------------------- |
| `src/components/icons/SidebarIcons.tsx` | P1-A           | Add `HealthIcon`                                         |
| `src/components/layout/PageBanner.tsx`  | P1-A           | Add `HealthDashboardArt`                                 |
| `src/components/layout/Sidebar.tsx`     | P1-B           | `AppRoute`, `ROUTE_LABELS`, `SIDEBAR_SECTIONS`, import   |
| `src/App.tsx`                           | P1-B           | `VALID_APP_ROUTES`                                       |
| `src/components/layout/ContentArea.tsx` | P1-B           | Import + switch case                                     |
| `src/components/pages/ProfilesPage.tsx` | P2-A           | Import `formatRelativeTime` from utils                   |
| `src/styles/theme.css`                  | P3 (if needed) | Health table styles (only if inline styles insufficient) |
