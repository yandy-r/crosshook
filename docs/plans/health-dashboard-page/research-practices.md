# Engineering Practices Research: Health Dashboard Page

## Executive Summary

Comprehensive analysis of reusable code, modularity opportunities, and KISS risks for implementing `HealthDashboardPage`. All findings are grounded in the current codebase.

---

## Existing Reusable Code

| Asset                                                                                                             | Location                                  | Reuse Opportunity                                                                                                                                                                 |
| ----------------------------------------------------------------------------------------------------------------- | ----------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `useProfileHealth()`                                                                                              | `hooks/useProfileHealth.ts:116`           | Primary data source — provides `summary`, `healthByName`, `trendByName`, `staleInfoByName`, `batchValidate`, `revalidateSingle`. Must be accessed via context (see Gotcha below). |
| `HealthBadge`                                                                                                     | `components/HealthBadge.tsx:32`           | Per-row status chip with trend arrow and failure count. Drop-in for table rows and summary cards.                                                                                 |
| `CollapsibleSection`                                                                                              | `components/ui/CollapsibleSection.tsx:13` | Controlled/uncontrolled collapsible panel. Use for "Recent Failures", "Issue Breakdown", "Launcher Drift" sections.                                                               |
| `PageBanner`                                                                                                      | `components/layout/PageBanner.tsx:10`     | Standard page header (eyebrow + title + copy + SVG art). Every existing page uses it.                                                                                             |
| `countProfileStatuses()`                                                                                          | `utils/health.ts:3`                       | Already aggregates healthy/stale/broken/total from a profile list. Use directly for summary card values.                                                                          |
| `computeTrend()`                                                                                                  | `hooks/useProfileHealth.ts:10`            | Status rank comparison returning `TrendDirection`. Already exported — call from the page if additional trend logic is needed.                                                     |
| CSS: `.crosshook-status-chip`                                                                                     | `styles/theme.css:442`                    | Pill chip for count badges and category chips on summary cards.                                                                                                                   |
| CSS: `.crosshook-compatibility-badge--{rating}`                                                                   | `styles/theme.css:658`                    | Color-coded `broken/partial/working` badges. `HealthBadge` already maps health statuses to these via `STATUS_TO_RATING`.                                                          |
| CSS: `.crosshook-panel` / `.crosshook-card`                                                                       | `styles/theme.css:137`                    | Standard surface containers with blur, border, and shadow. Use for each summary card and panel.                                                                                   |
| CSS: `.crosshook-heading-*`                                                                                       | `styles/theme.css:154`                    | `.crosshook-heading-eyebrow`, `.crosshook-heading-title`, `.crosshook-heading-copy` for card headers.                                                                             |
| CSS: `.crosshook-muted`, `.crosshook-danger`                                                                      | `styles/theme.css:2040`                   | Muted text for secondary values; danger text for error states.                                                                                                                    |
| CSS: `.crosshook-help-text`                                                                                       | `styles/theme.css:176`                    | Small muted copy already used for stale-note text in `ProfilesPage`.                                                                                                              |
| CSS: design tokens                                                                                                | `styles/variables.css`                    | All colors, spacing, radius, and transitions via `--crosshook-color-*` and `--crosshook-*` variables.                                                                             |
| `AppRoute` type                                                                                                   | `components/layout/Sidebar.tsx:12`        | Union type for routing. Must be extended with `'health'` to add the new tab.                                                                                                      |
| `SIDEBAR_SECTIONS` constant                                                                                       | `components/layout/Sidebar.tsx:32`        | Data-driven sidebar sections — add a new `{ route: 'health', label: 'Health', icon: ... }` entry.                                                                                 |
| `ContentArea` switch                                                                                              | `components/layout/ContentArea.tsx:33`    | `renderPage()` exhaustive switch — add `'health'` case here.                                                                                                                      |
| `formatRelativeTime()`                                                                                            | `components/pages/ProfilesPage.tsx:22`    | ISO timestamp to human-readable string. Copy or extract — see Abstraction section.                                                                                                |
| `HealthCheckSummary`, `ProfileHealthReport`, `HealthIssue`, `EnrichedProfileHealthReport`, `CachedHealthSnapshot` | `types/health.ts`                         | All types already defined and exported via `types/index.ts`.                                                                                                                      |

---

## Modularity Design

The page should be a single file with inline sub-components. Do **not** pre-split into multiple files — the rule of three applies.

### Proposed Structure

```
context/ProfileHealthContext.tsx             ← new file (shared context)
components/pages/HealthDashboardPage.tsx     ← new file (the page)
```

`ProfileHealthContext` is required (not optional) — see the Gotcha below. It is the only new non-page file.

### Module Boundaries

| Layer                         | What it owns                                                                   | What it does NOT own                   |
| ----------------------------- | ------------------------------------------------------------------------------ | -------------------------------------- |
| `ProfileHealthContext`        | Single instance of `useProfileHealth`, exposes result via React context        | Rendering, routing                     |
| `HealthDashboardPage`         | Layout, local UI state (sort key, filter string, active panel)                 | Health data fetching                   |
| `ProfilesPage` (existing)     | Consumes health data via `useProfileHealthContext()` — **must be updated**     | Health data fetching                   |
| `useProfileHealth` (existing) | IPC calls, Tauri event listener, cached snapshots, trend computation           | Rendering                              |
| Inline sub-components         | `SummaryCard`, `IssueBreakdownRow`, `ProfileHealthRow` — all small, page-local | Not exported, not reused elsewhere yet |

### Shared vs. Feature-Specific

| Code                                                                  | Classification             | Reason                                                                                                                     |
| --------------------------------------------------------------------- | -------------------------- | -------------------------------------------------------------------------------------------------------------------------- |
| `useProfileHealth`, `HealthBadge`, `CollapsibleSection`, `PageBanner` | Shared — already shared    | Used by `ProfilesPage` today                                                                                               |
| `ProfileHealthContext`                                                | Shared infrastructure      | Required to avoid dual-instance event listener conflict                                                                    |
| `countProfileStatuses`                                                | Shared utility             | Already in `utils/health.ts`, used inside `useProfileHealth` reducer                                                       |
| Sort/filter logic inside the page                                     | Feature-specific           | First (and only) use of this UI pattern for health tables                                                                  |
| `SummaryCard` inline component                                        | Feature-specific (for now) | Only one consumer                                                                                                          |
| `formatRelativeTime`                                                  | Borderline                 | Defined in `ProfilesPage.tsx:22`, second use would be HealthDashboardPage — wait for a third before extracting to `utils/` |

---

## KISS Assessment

| Proposed Approach                                              | Risk                                                       | Simpler Alternative                                                                                                          |
| -------------------------------------------------------------- | ---------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------- |
| Separate hook `useHealthDashboard` wrapping `useProfileHealth` | Over-engineering — adds indirection for no gain            | Call `useProfileHealthContext()` directly in the page component                                                              |
| Separate `HealthSummaryCards.tsx` component file               | Premature extraction — only one use                        | Define `SummaryCard` as a local function inside `HealthDashboardPage.tsx`                                                    |
| Generic `DataTable<T>` abstraction                             | Heavy abstraction for a single table                       | Render `<table>` or a `<div role="table">` grid directly with inline sort state                                              |
| Redux/Zustand store for dashboard state                        | No cross-component state sharing needed                    | Plain `useState` for sort key and filter string                                                                              |
| Virtualized list (react-window)                                | Unnecessary — profile lists are small (< 200 rows typical) | Standard DOM list                                                                                                            |
| Custom sort comparator registry                                | Abstraction before third caller                            | Inline sort comparators per column using `Array.prototype.sort`                                                              |
| Separate CSS file for dashboard                                | Unnecessary new file                                       | Add scoped classes to existing `theme.css` using the `crosshook-health-dashboard*` namespace, following the existing pattern |

---

## Abstraction vs. Repetition

Apply "rule of three" before extracting:

| Pattern                               | Current Count                                                                       | Action                                                                                  |
| ------------------------------------- | ----------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------- |
| `formatRelativeTime` logic            | 1 (ProfilesPage) + 1 (HealthDashboardPage) = **2**                                  | **Repeat** — copy into page; extract to `utils/format.ts` only when a third use appears |
| `crosshook-status-chip` chip elements | N (across multiple components)                                                      | Already a CSS class — **reuse directly**                                                |
| Health issue list rendering           | 1 (ProfilesPage health issues section) + 1 (HealthDashboard failures panel) = **2** | **Repeat** — keep separate; extract only if a third location appears                    |
| `SummaryCard` stat card               | 4 cards on this page, no other use                                                  | **Keep local** — inline component inside HealthDashboardPage                            |
| "batch validate" re-check button      | 2 (ProfilesPage + HealthDashboardPage)                                              | **Repeat** — both just call `batchValidate()` from the hook, no shared component needed |

---

## Interface Design

### Tab / Route Extension

Four touch points (exhaustive, verified at compile time by TypeScript):

1. `AppRoute` union in `components/layout/Sidebar.tsx:12` — add `'health'`
2. `VALID_APP_ROUTES` record in `App.tsx:14` — add `health: true`
3. `SIDEBAR_SECTIONS` array in `Sidebar.tsx:32` — add item with icon
4. `ContentArea` switch in `ContentArea.tsx:34` — add `'health'` case (TypeScript will error on missing case due to `never` exhaustive check at line 47)

The existing routing is purely string-based with no registry — this is appropriate for the scale and should not be replaced with a more complex system.

### Provider Nesting (App.tsx)

`ProfileHealthProvider` must wrap `AppShell` alongside `ProfileProvider` and `PreferencesProvider`. The existing nesting in `App.tsx:46` is the right place:

```tsx
<ProfileProvider>
  <ProfileHealthProvider>   {/* ← add */}
    <AppShell controllerMode={...} />
  </ProfileHealthProvider>
</ProfileProvider>
```

### Page Component Props

The page receives no props — it sources all data via `useProfileHealthContext()` as every other page sources data from context. Zero-prop page components is the established pattern throughout this codebase.

```ts
// HealthDashboardPage.tsx
export function HealthDashboardPage() {
  const { summary, loading, error, healthByName, trendByName, staleInfoByName, batchValidate } =
    useProfileHealthContext();
  // local UI state: sort key, filter string
}
```

### Filtering / Sorting State

Use `useState` + `useMemo` with `useDeferredValue` for the filter input — this is exactly the pattern in `CompatibilityViewer.tsx:110-140`. The deferred value prevents blocking on each keystroke.

---

## Gotchas

- **`useProfileHealth` dual-instance conflict** — `useProfileHealth` is currently called directly in `ProfilesPage` (line 120), not in any shared context. If `HealthDashboardPage` also calls `useProfileHealth()`, there will be two competing Tauri event listeners for `profile-health-batch-complete` (registered at `useProfileHealth.ts:155`), two separate `invoke('batch_validate_profiles')` fallback timers, and two separate `invoke('get_cached_health_snapshots')` calls on mount. State will diverge. **Fix:** create `context/ProfileHealthContext.tsx` wrapping a single `useProfileHealth()` instance, then update `ProfilesPage` to call `useProfileHealthContext()` instead. This is a required prerequisite, not optional.

---

## Testability Patterns

The Rust codebase has tests (`cargo test -p crosshook-core`). There is no frontend test framework configured. Therefore:

| Concern                      | Approach                                                                                                                                                                                     |
| ---------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Unit-testable pure functions | `countProfileStatuses` (already in `utils/health.ts`) and any new sort comparators should be pure functions with no side effects — naturally unit-testable if a test framework is ever added |
| Stateful UI                  | Not unit-tested currently; follow the existing convention (no tests)                                                                                                                         |
| IPC boundary                 | `useProfileHealth` already centralizes all `invoke` calls — the page never calls `invoke` directly, which preserves testability of business logic separately from the view                   |
| Type safety as test coverage | All IPC payloads cross the boundary as typed interfaces (`HealthCheckSummary`, `ProfileHealthReport`) — TypeScript strict mode catches shape mismatches at compile time                      |

---

## Build vs. Depend

| Need                        | Recommendation                                                                 | Reason                                                                                                            |
| --------------------------- | ------------------------------------------------------------------------------ | ----------------------------------------------------------------------------------------------------------------- |
| Sortable table              | **Build** (inline `useMemo` + `sort`)                                          | The table has 4-5 columns; a library like `@tanstack/table` is heavier than the problem requires                  |
| Trend arrows / color coding | **Build** — already done in `HealthBadge`                                      | `HealthBadge` exports `TrendDirection` arrows; no charting library needed for "up/down" indicators                |
| Summary stat cards          | **Build** inline                                                               | Simple `div` + CSS custom properties — no UI library card component needed                                        |
| Filter inputs               | **Build** — follow `CompatibilityViewer` pattern                               | `<input>` + `datalist` + `useDeferredValue` is already proven in the codebase                                     |
| Date formatting             | **Build** — copy `formatRelativeTime`                                          | One function, no `date-fns` or similar needed                                                                     |
| Accessible markup for table | **Build** — `<table>` element with `<thead>`/`<tbody>`, or `role="table"` grid | Native HTML is sufficient; no accessibility library needed given the existing focus management in `useGamepadNav` |

No new npm packages are recommended for this feature.

---

## Relevant Files

- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/hooks/useProfileHealth.ts` — primary data hook (must be wrapped in context)
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/context/ProfileContext.tsx` — pattern to follow for new ProfileHealthContext
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/components/HealthBadge.tsx` — reusable status badge
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/components/ui/CollapsibleSection.tsx` — panel wrapper
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/components/layout/PageBanner.tsx` — page header pattern
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/components/layout/ContentArea.tsx` — routing switch to extend
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/components/layout/Sidebar.tsx` — route union + nav sections to extend
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/App.tsx` — `VALID_APP_ROUTES` + provider nesting to extend
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/utils/health.ts` — `countProfileStatuses` utility
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/types/health.ts` — all health types
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/components/CompatibilityViewer.tsx` — filter + deferred-value pattern to follow
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/components/pages/ProfilesPage.tsx` — must be updated to use `useProfileHealthContext()`; also source of `formatRelativeTime` pattern
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/styles/theme.css` — `.crosshook-status-chip`, `.crosshook-compatibility-badge--*`, `.crosshook-panel`, `.crosshook-card`, tokens

---

## Open Questions

1. **`formatRelativeTime` extraction** — should it be moved to `utils/format.ts` now (anticipating two callers) or after a third? The conservative answer is to copy it for now.
2. **Health icon for sidebar** — `SidebarIcons.tsx` defines SVG icons for each route. A new `HealthIcon` SVG component will need to be added alongside `ProfilesIcon`, `LaunchIcon`, etc.
3. **`EnrichedProfileHealthReport` vs. `ProfileHealthReport`** — `useProfileHealth` returns `ProfileHealthReport[]` (not enriched). If the dashboard needs `ProfileHealthMetadata` (failure count, last success, drift state), it must call a separate enrichment command or the hook must be extended. This should be clarified before implementation.
4. **Launcher drift and community import data** — these fields live on `ProfileHealthMetadata` which requires the enrichment path. The basic dashboard can render without them; they are progressive enhancement.
5. **CSS file scope** — the existing pattern adds styles directly to `theme.css`. For a page this size, consider whether a new `health-dashboard.css` file is warranted, or whether inline styles (already used extensively in `ProfilesPage`) are acceptable for the initial implementation.
