# Plan: GitHub Issues 421 and 448 Route Rework Dashboards

## Summary

Implement Phase 9 of the Unified Desktop Redesign for GitHub issues #448 / #421 by re-skinning the dashboard routes `Health`, `Host Tools`, `Proton Manager`, and `Compatibility` so they read as one coherent part of the new shell. The implementation stays frontend-only, preserves existing IPC and route behavior, and focuses on shared panel/section idioms, cleaner scroll ownership, and broader smoke coverage for the currently under-tested dashboard routes.

## User Story

As a CrossHook user, I want the dashboard routes to match the unified desktop shell visually, so that health, host-tool readiness, Proton management, and compatibility data feel like part of the same application instead of four bespoke surfaces.

## Problem -> Solution

Today the target routes mix different layout contracts: `Compatibility` already uses the bounded card-shell pattern, `Host Tools` and `Proton Manager` use simpler body-scroll wrappers, and `Health` still renders as a custom free-flow dashboard with older chrome assumptions. Phase 9 adds a small reusable dashboard-shell foundation, then rewraps each route around the same banner/panel/section idioms while leaving the underlying hooks, DTOs, IPC calls, and route state untouched.

## Metadata

- **Complexity**: Large
- **Source PRD**: `docs/prps/prds/unified-desktop-redesign.prd.md`
- **PRD Phase**: Phase 9 - Route rework - Dashboards
- **Estimated Files**: 15
- **GitHub Issues**: #448 tracking, #421 deliverable
- **Persistence Classification**: runtime-only UI composition; no new TOML settings; no new SQLite tables or migrations.

## Persistence and Usability

1. **Storage boundary**
   - **TOML settings**: No new user-editable preferences. Route appearance remains code-driven.
   - **SQLite metadata DB**: No new cache/history tables. Existing route data continues to come from current hooks and IPC surfaces.
   - **Runtime-only state**: Layout-only state such as tab selection, filter input, loading banners, and panel visibility remains in React memory.

2. **Migration / backward compatibility**: No settings or schema migration is required. Older builds simply keep the prior route chrome.

3. **Offline behavior**: No new online requirement is introduced. Existing degraded states for cached health, host readiness, Proton catalogs, and compatibility data must remain visible and actionable.

4. **Failure fallback**: If a route data source fails, the restyled page must continue to surface the same inline error/empty banners instead of collapsing the shell.

5. **User visibility / editability**: Users can see the redesigned route chrome but cannot configure or persist route-specific layout preferences in this phase.

## Batches

Tasks grouped by dependency for parallel execution. Tasks within the same batch run concurrently; batches run in order.

| Batch | Tasks         | Depends On | Parallel Width |
| ----- | ------------- | ---------- | -------------- |
| B1    | 1.1, 1.2, 1.3 | -          | 3              |
| B2    | 2.1, 2.2, 2.3 | B1         | 3              |
| B3    | 3.1, 3.2      | B2         | 2              |

- **Total tasks**: 8
- **Total batches**: 3
- **Max parallel width**: 3

---

## UX Design

### Before

```text
Dashboard routes share the global shell, but each route presents different inner chrome:

Health         -> free-flow cards + table + panels
Host Tools     -> body-scroll dashboard stack
Proton Manager -> single large panel wrapper
Compatibility  -> bounded card shell with subtabs
```

### After

```text
All dashboard routes share the same visual language:

RouteBanner
  -> dashboard hero / summary strip
  -> bounded section panels with eyebrow/title/action rows
  -> consistent pill / status-chip / kv-row treatment
  -> stable scroll ownership inside the existing shell contract
```

### Interaction Changes

| Touchpoint           | Before                                                     | After                                                                   | Notes                                                      |
| -------------------- | ---------------------------------------------------------- | ----------------------------------------------------------------------- | ---------------------------------------------------------- |
| Health route         | Functional but visually older dashboard stack              | Same data/actions rendered in unified dashboard sections and hero cards | No change to validation, migration, or navigation behavior |
| Host Tools route     | Already closest to target, but still route-specific chrome | Keeps current behavior while adopting shared dashboard section framing  | Least-delta route in this phase                            |
| Proton Manager route | One large wrapper panel around a 500+ line component       | Same install/uninstall flows presented as split dashboard sections      | Behavior parity is mandatory                               |
| Compatibility route  | Mixed subtab shell + viewer chrome                         | Same tabs/viewer retained, but aligned to dashboard panel idioms        | Preserve `forceMount` tab behavior                         |
| Smoke coverage       | `compatibility` and `health` only in route sweep           | Adds `host-tools` and `proton-manager` to route sweep                   | Required by PRD Phase 12 direction and Phase 9 acceptance  |

---

## Mandatory Reading

Files that MUST be read before implementing:

| Priority | File                                                                          | Lines           | Why                                                                                                            |
| -------- | ----------------------------------------------------------------------------- | --------------- | -------------------------------------------------------------------------------------------------------------- |
| P0       | `docs/prps/prds/unified-desktop-redesign.prd.md`                              | 271-275         | Phase 9 goal, scope, and success signal define the contract for this plan.                                     |
| P0       | `src/crosshook-native/src/styles/layout.css`                                  | 140-219         | Shared route-shell layout contract for `--fill`, `__body--scroll`, `__body--fill`, and bounded card scrolling. |
| P0       | `src/crosshook-native/src/components/layout/routeMetadata.ts`                 | 31-123          | Route banner metadata and dashboard route identity live in one typed map.                                      |
| P0       | `src/crosshook-native/src/components/layout/RouteBanner.tsx`                  | 8-30            | Shared top-of-route identity banner contract all dashboard pages must keep.                                    |
| P0       | `src/crosshook-native/src/components/pages/HealthDashboardPage.tsx`           | 24-220, 280-452 | Existing health route composition, retry flows, and table/expanded-row behavior that must remain intact.       |
| P0       | `src/crosshook-native/src/components/pages/HostToolsPage.tsx`                 | 50-219          | Existing host-tools route wrapper, body-scroll ownership, and filter/empty/error state flows.                  |
| P0       | `src/crosshook-native/src/components/pages/ProtonManagerPage.tsx`             | 13-35           | Thin page-wrapper pattern for dashboard routes that delegate behavior into a focused panel component.          |
| P0       | `src/crosshook-native/src/components/pages/CompatibilityPage.tsx`             | 42-292          | Current compatibility tab contract, `forceMount` behavior, and shell/card composition.                         |
| P1       | `src/crosshook-native/src/components/proton-manager/ProtonManagerPanel.tsx`   | 27-259, 360-503 | Proton manager async state, error banners, and current >500-line split pressure.                               |
| P1       | `src/crosshook-native/src/components/CompatibilityViewer.tsx`                 | 95-220          | Viewer-side deferred filtering, panel headers, and inline error/empty handling.                                |
| P1       | `src/crosshook-native/src/components/host-readiness/HostToolMetricsHero.tsx`  | 16-112          | Existing summary-card pattern mirrored by dashboard hero stats.                                                |
| P1       | `src/crosshook-native/src/components/pages/health-dashboard/SummaryCards.tsx` | 19-60           | Health summary-card idiom to reuse instead of inventing a second metric pattern.                               |
| P1       | `src/crosshook-native/src/components/layout/__tests__/AppShell.test.tsx`      | 104-204         | Viewport-aware shell test harness and geometry mocking style.                                                  |
| P1       | `src/crosshook-native/tests/smoke.spec.ts`                                    | 7-85            | Current route smoke pattern and the missing dashboard routes that Phase 9 should add.                          |
| P2       | `src/crosshook-native/src/components/pages/CommunityPage.tsx`                 | 13-20           | Best nearby example of the unified bounded card-shell route wrapper.                                           |
| P2       | `src/crosshook-native/src/components/pages/SettingsPage.tsx`                  | 39-52           | Same route-card host/scroll pattern applied to another redesigned route.                                       |

## External Documentation

| Topic         | Source | Key Takeaway                                                                                                             |
| ------------- | ------ | ------------------------------------------------------------------------------------------------------------------------ |
| External docs | none   | No external API/library research is needed; the Phase 9 work is fully constrained by the PRD and existing repo patterns. |

---

## Patterns to Mirror

Code patterns discovered in the codebase. Follow these exactly.

### ROUTE_BANNER_METADATA_PATTERN

```ts
// SOURCE: src/crosshook-native/src/components/layout/routeMetadata.ts:102-123
health: {
  sectionEyebrow: 'Dashboards',
  bannerTitle: 'Health',
  Art: HealthDashboardArt,
}
```

Keep route identity in `ROUTE_METADATA`; do not hardcode new banner copy or icon decisions inside page components.

### ROUTE_FILL_CARD_PATTERN

```tsx
// SOURCE: src/crosshook-native/src/components/pages/CommunityPage.tsx:13-19
<div className="crosshook-page-scroll-shell crosshook-page-scroll-shell--fill ...">
  <div className="crosshook-route-stack">
    <div className="crosshook-route-card-host">
      <div className="crosshook-route-card-scroll">
```

When a route needs a bounded primary panel with its own scroll, use the `route-card-host` / `route-card-scroll` contract instead of hand-rolled nested overflow wrappers.

### BODY_SCROLL_CONTRACT_PATTERN

```css
/* SOURCE: src/crosshook-native/src/styles/layout.css:179-218 */
.crosshook-route-stack__body--scroll {
  overflow-y: auto;
  overscroll-behavior: contain;
}
```

If a route remains page-scroll-owned rather than card-scroll-owned, keep the existing shell selector contract so `useScrollEnhance` and WebKitGTK behavior stay correct.

### FILTER_MEMO_PATTERN

```tsx
// SOURCE: src/crosshook-native/src/components/pages/HostToolsPage.tsx:63-71
const filteredTools = useMemo(
  () =>
    toolChecks.filter((tool) => matchesCategory(tool, categoryFilter) && matchesAvailability(tool, availabilityFilter)),
  [availabilityFilter, categoryFilter, normalizedSearchQuery, toolChecks]
);
```

Dashboard routes derive filtered render lists with `useMemo` over typed predicates instead of mutating source arrays or combining filter logic inline in JSX.

### DEFERRED_FILTER_PATTERN

```tsx
// SOURCE: src/crosshook-native/src/components/CompatibilityViewer.tsx:109-139
const deferredGameFilter = useDeferredValue(gameFilter);
const deferredTrainerFilter = useDeferredValue(trainerFilter);
const deferredPlatformFilter = useDeferredValue(platformFilter);
```

For text-filter-heavy surfaces, preserve the deferred-input pattern instead of synchronously recomputing large filtered lists on every keystroke.

### SUMMARY_CARD_PATTERN

```tsx
// SOURCE: src/crosshook-native/src/components/pages/health-dashboard/SummaryCards.tsx:35-46
<div className="crosshook-card crosshook-health-dashboard-card" style={{ borderLeftColor: accentColor }}>
  <div className="crosshook-health-dashboard-card__count">{displayCount}</div>
  <div className="crosshook-health-dashboard-card__label crosshook-muted">{label}</div>
</div>
```

Mirror the existing metric-card shape for dashboard hero summaries so Health and Host Tools continue to feel related and Proton/Compatibility can align without inventing a conflicting stat-card idiom.

### TRY_FINALLY_ASYNC_ACTION_PATTERN

```tsx
// SOURCE: src/crosshook-native/src/components/pages/HostToolsPage.tsx:100-106
setProbingToolId(toolId);
try {
  await probeTool(toolId);
} finally {
  setProbingToolId((current) => (current === toolId ? null : current));
}
```

Async button actions that expose transient UI state should use `try/finally` to avoid stuck loading affordances.

### INLINE_ALERT_PATTERN

```tsx
// SOURCE: src/crosshook-native/src/components/CompatibilityViewer.tsx:212-219
{
  error ? (
    <div className="crosshook-panel crosshook-compatibility-viewer__message">
      <div className="crosshook-danger">{error}</div>
    </div>
  ) : null;
}
```

Route failures stay inline within the route shell. Do not replace the whole dashboard page with a blank or modal error state.

### THIN_PAGE_WRAPPER_PATTERN

```tsx
// SOURCE: src/crosshook-native/src/components/pages/ProtonManagerPage.tsx:22-31
<RouteBanner route="proton-manager" />
<div className="crosshook-panel">
  <ProtonManagerPanel steamClientInstallPath={effectiveSteamPath.length > 0 ? effectiveSteamPath : undefined} />
</div>
```

Keep page components thin when most behavior already lives in a focused child panel or route-specific component tree.

### VIEWPORT_SMOKE_PATTERN

```tsx
// SOURCE: src/crosshook-native/src/components/layout/__tests__/AppShell.test.tsx:120-127
setInnerWidth(1920);
setInnerHeight(1080);
const rectSpy = mockAppShellRect(1920, 1080);
```

Use the existing viewport/geometry test harness for route-shell assertions instead of inventing a second layout test setup.

---

## Files to Change

| File                                                                              | Action | Justification                                                                                                                                       |
| --------------------------------------------------------------------------------- | ------ | --------------------------------------------------------------------------------------------------------------------------------------------------- |
| `src/crosshook-native/src/components/layout/DashboardPanelSection.tsx`            | CREATE | Shared eyebrow/title/action shell for Phase 9 dashboard sections so the four routes converge on one chrome contract.                                |
| `src/crosshook-native/src/styles/dashboard-routes.css`                            | CREATE | Shared route-level dashboard spacing, hero, section-header, and kv-row/pill utility styles kept out of `theme.css` to avoid broad collateral edits. |
| `src/crosshook-native/src/main.tsx`                                               | UPDATE | Register the new shared dashboard stylesheet once at the app entrypoint.                                                                            |
| `src/crosshook-native/src/components/pages/HostToolsPage.tsx`                     | UPDATE | Rewrap Host Tools around the shared dashboard section primitives without changing readiness behavior.                                               |
| `src/crosshook-native/src/styles/host-tool-dashboard.css`                         | UPDATE | Align existing route-specific host-tools classes with the new dashboard-shell chrome.                                                               |
| `src/crosshook-native/src/components/pages/HealthDashboardPage.tsx`               | UPDATE | Recompose Health around shared dashboard sections while preserving the current hooks, alerts, table, and navigation flows.                          |
| `src/crosshook-native/src/components/proton-manager/InstalledVersionsSection.tsx` | CREATE | Split installed-runtime rendering out of the 503-line Proton manager panel to keep the redesign modular.                                            |
| `src/crosshook-native/src/components/proton-manager/AvailableVersionsSection.tsx` | CREATE | Split catalog/install-list rendering into a focused section component with the same existing async behavior.                                        |
| `src/crosshook-native/src/components/proton-manager/ProtonManagerPanel.tsx`       | UPDATE | Shrink the over-cap panel and recompose it around the new split sections plus existing alert/progress logic.                                        |
| `src/crosshook-native/src/components/pages/ProtonManagerPage.tsx`                 | UPDATE | Upgrade the page wrapper from a generic single panel to the new dashboard route treatment.                                                          |
| `src/crosshook-native/src/styles/proton-manager.css`                              | UPDATE | Apply the unified dashboard visual language to Proton Manager without altering install/uninstall semantics.                                         |
| `src/crosshook-native/src/components/compatibility/ProtonVersionsPanel.tsx`       | CREATE | Extract the inline Proton tab content from `CompatibilityPage.tsx` so the redesign stays modular and easier to test.                                |
| `src/crosshook-native/src/components/pages/CompatibilityPage.tsx`                 | UPDATE | Rewrap the compatibility route and keep the `forceMount` subtab behavior while aligning outer chrome to the dashboard shell.                        |
| `src/crosshook-native/src/components/CompatibilityViewer.tsx`                     | UPDATE | Tighten viewer chrome and result/panel structure to match the shared dashboard idioms while preserving deferred filtering.                          |
| `src/crosshook-native/src/components/pages/__tests__/DashboardRoutes.test.tsx`    | CREATE | Add a focused RTL harness for the four dashboard routes and their persistent banner/alert/section contracts.                                        |
| `src/crosshook-native/tests/smoke.spec.ts`                                        | UPDATE | Add `host-tools` and `proton-manager` to the route sweep and capture dashboard-regression smoke coverage.                                           |

## NOT Building

- No new Tauri commands, IPC payloads, or `crosshook-core` changes.
- No data-model or persistence changes in TOML or SQLite.
- No route-order or navigation-model changes; `AppShell` remains the single source of route state.
- No Phase 10/11 scope creep into `Install`, `Settings`, `Community`, `Discover`, `Profiles`, or `Launch`.
- No new external UI dependency such as a dashboard component library.
- No redesign of the underlying host-readiness, ProtonUp, community sync, or health-validation business logic.

---

## Step-by-Step Tasks

### Task 1.1: Add shared dashboard route chrome primitives — Depends on [none]

- **BATCH**: B1
- **ACTION**: Create `DashboardPanelSection.tsx`, create `dashboard-routes.css`, and register the stylesheet in `main.tsx`.
- **IMPLEMENT**: Add a minimal shared route-section primitive that standardizes eyebrow/title/action rows and shared dashboard spacing classes. Keep the component presentational-only and move reusable Phase 9 chrome into the new stylesheet so later route tasks can reuse it without editing the same CSS file concurrently.
- **MIRROR**: `ROUTE_BANNER_METADATA_PATTERN`, `ROUTE_FILL_CARD_PATTERN`, `BODY_SCROLL_CONTRACT_PATTERN`.
- **IMPORTS**: React node/props typing, existing global style import pattern from `main.tsx`.
- **GOTCHA**: Do not put route-specific host-tools or Proton manager selectors into the shared file unless they are genuinely reusable across at least two Phase 9 routes.
- **VALIDATE**: `npm run typecheck` passes and the new section primitive renders in at least one route without changing behavior.

### Task 1.2: Re-skin Host Tools on the new dashboard chrome — Depends on [none]

- **BATCH**: B1
- **ACTION**: Update `HostToolsPage.tsx` and `host-tool-dashboard.css`.
- **IMPLEMENT**: Keep the current body-scroll route contract and readiness actions, but reorganize the page into clearer dashboard hero/section groupings using the shared section primitive. Preserve existing alert, empty, and filtered states; this is the least-delta route and should become the visual baseline for the rest of the dashboard set.
- **MIRROR**: `FILTER_MEMO_PATTERN`, `TRY_FINALLY_ASYNC_ACTION_PATTERN`, `INLINE_ALERT_PATTERN`.
- **IMPORTS**: `DashboardPanelSection`, existing host-readiness child components.
- **GOTCHA**: Keep `refresh`, `probeTool`, and `dismiss_readiness_nag` flows exactly as-is; the route redesign must not change readiness semantics or fallback order.
- **VALIDATE**: Host Tools still renders loading, empty, filtered-empty, partial-error, and full-inventory states without changing the underlying action paths.

### Task 1.3: Split Proton Manager into route-friendly section components — Depends on [none]

- **BATCH**: B1
- **ACTION**: Create `InstalledVersionsSection.tsx` and `AvailableVersionsSection.tsx`, then update `ProtonManagerPanel.tsx` to use them.
- **IMPLEMENT**: Extract the installed-runtime list and available-version catalog into focused components so `ProtonManagerPanel.tsx` drops below the soft cap and becomes easier to restyle in B2. Preserve the existing async state ownership in the parent panel: active op ids, pending install keys, uninstall confirmation, and error banners should stay centralized unless a child only needs display props.
- **MIRROR**: `THIN_PAGE_WRAPPER_PATTERN`, `TRY_FINALLY_ASYNC_ACTION_PATTERN`, subcomponent prop-contract style from `HostToolInventory` / `CompatibilityViewer`.
- **IMPORTS**: Existing ProtonUp DTO types, `VersionRow`, `InstallProgressBar`, `InstallRootBadge`, `ProviderPicker`.
- **GOTCHA**: Do not let extracted children own install/uninstall side effects directly if that duplicates the parent’s async guardrails.
- **VALIDATE**: `ProtonManagerPanel.tsx` is under the soft cap, type-checks cleanly, and still exposes the same banners/progress rows for current operations.

### Task 2.1: Recompose Health into unified dashboard sections — Depends on [1.1]

- **BATCH**: B2
- **ACTION**: Update `HealthDashboardPage.tsx` to use the shared dashboard chrome around existing summary cards, toolbar, table, and side panels.
- **IMPLEMENT**: Keep the current hooks and table logic intact, but reorganize the route into clearer hero, primary status, and supporting-section groupings so it visually matches the shell. Reuse existing `SummaryCard`, `TableToolbar`, and health-dashboard subpanels instead of inventing replacement widgets.
- **MIRROR**: `SUMMARY_CARD_PATTERN`, `INLINE_ALERT_PATTERN`, `VIEWPORT_SMOKE_PATTERN`.
- **IMPORTS**: `DashboardPanelSection`, existing `health-dashboard/*` subcomponents.
- **GOTCHA**: Do not introduce a second scroll owner inside the Health table region; the page should continue to use its current shell/body scroll contract unless a new selector is intentionally added to `useScrollEnhance`.
- **VALIDATE**: Health still supports retry, recheck, version-scan progress, row expansion, and navigation to `profiles` without regressions.

### Task 2.2: Rework Proton Manager route shell around the split sections — Depends on [1.1, 1.3]

- **BATCH**: B2
- **ACTION**: Update `ProtonManagerPage.tsx` and `proton-manager.css`.
- **IMPLEMENT**: Upgrade Proton Manager from a generic single-panel wrapper into a dashboard route with clearer hero/section segmentation, while composing the split installed/available sections from B1. The page should keep the same effective Steam path resolution and present error/stale/progress states with stronger but still inline dashboard chrome.
- **MIRROR**: `THIN_PAGE_WRAPPER_PATTERN`, `INLINE_ALERT_PATTERN`, token-driven CSS pattern in `proton-manager.css`.
- **IMPORTS**: `DashboardPanelSection`, split Proton manager section components, existing contexts/hooks already used by the route.
- **GOTCHA**: Preserve the current `steamClientInstallPath` resolution order (`prop -> preferences -> profile context`) exactly; visual rework must not change which install root data the page sees.
- **VALIDATE**: Proton Manager still shows roots/providers/catalog/install-progress/uninstall-warning flows, and no async action becomes double-triggerable.

### Task 2.3: Align Compatibility with the dashboard route language — Depends on [1.1]

- **BATCH**: B2
- **ACTION**: Create `components/compatibility/ProtonVersionsPanel.tsx`, update `CompatibilityPage.tsx`, and refine `CompatibilityViewer.tsx`.
- **IMPLEMENT**: Extract the inline Proton tab content, keep Radix `Tabs` + `forceMount`, and restyle the outer route and viewer sections so the trainer/proton experience feels like a Phase 9 dashboard rather than a one-off subtab surface. Preserve deferred filtering, inline errors, and current empty-state behavior.
- **MIRROR**: `DEFERRED_FILTER_PATTERN`, `INLINE_ALERT_PATTERN`, `ROUTE_FILL_CARD_PATTERN`.
- **IMPORTS**: `DashboardPanelSection`, `CompatibilityViewer`, existing Proton/community hooks, Radix Tabs.
- **GOTCHA**: Keep tab panels mounted when hidden; switching between Trainer and Proton must not refetch or reset route-local state unnecessarily.
- **VALIDATE**: Compatibility still preserves tab state, renders inline empty/error messages, and installs Proton versions into the default compat-tools directory with unchanged behavior.

### Task 3.1: Add focused dashboard route RTL coverage — Depends on [2.1, 2.2, 2.3]

- **BATCH**: B3
- **ACTION**: Create `src/crosshook-native/src/components/pages/__tests__/DashboardRoutes.test.tsx`.
- **IMPLEMENT**: Add a provider-backed route harness that renders the four dashboard pages and asserts their persistent `RouteBanner`, primary section headings, and key fallback states. Reuse the existing `renderWithMocks` + real-provider style from `LibraryPage.test.tsx` and the viewport geometry helpers from `AppShell.test.tsx` where shell-size assertions matter.
- **MIRROR**: `VIEWPORT_SMOKE_PATTERN`, real-provider test harness style from `LibraryPage.test.tsx`.
- **IMPORTS**: `renderWithMocks`, relevant providers, the four route components.
- **GOTCHA**: Keep tests scoped to shell/chrome regression and fallback-state parity; do not turn them into duplicate end-to-end tests for every hook action.
- **VALIDATE**: `npm test` passes with a new dashboard route test suite and no added `console.error` noise.

### Task 3.2: Expand browser smoke to cover all Phase 9 dashboard routes — Depends on [2.1, 2.2, 2.3]

- **BATCH**: B3
- **ACTION**: Update `src/crosshook-native/tests/smoke.spec.ts`.
- **IMPLEMENT**: Add `host-tools` and `proton-manager` to `ROUTE_ORDER`, keep the tab-driven route sweep, and capture screenshots for the full dashboard set. Add at least one dashboard-focused smoke assertion that the redesigned route bodies render without page errors or `console.error`.
- **MIRROR**: existing route-loop structure and `attachConsoleCapture` contract in `smoke.spec.ts`.
- **IMPORTS**: `ROUTE_NAV_LABEL`, existing smoke helpers only.
- **GOTCHA**: If mock fixtures are missing data for the newly added routes, fix the mocks before loosening the assertions; do not weaken the no-error contract to make smoke pass.
- **VALIDATE**: `npm run test:smoke` produces screenshots for `health`, `host-tools`, `proton-manager`, and `compatibility` with zero uncaught page or console errors.

---

## Testing Strategy

### Unit / Integration Tests

| Test                               | Input                                                                 | Expected Output                                                                                             | Edge Case? |
| ---------------------------------- | --------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------- | ---------- |
| Dashboard route shell render       | Render each of the four target pages in the existing provider harness | Shared `RouteBanner`, primary dashboard section, and route-specific fallback chrome render without crashing | No         |
| Health route fallback parity       | Health route with cached/error/loading states from mocks              | Retry banner, summary cards, table heading, and health panels still render with the redesigned layout       | Yes        |
| Host Tools filter + empty states   | Host readiness data present vs filtered away vs unavailable           | Hero, toolbar, inventory, and empty-state cards still appear in the correct combinations                    | Yes        |
| Proton Manager async banner parity | Mock install/uninstall/cancel errors and active operations            | Inline warning/error/progress banners remain visible after the panel split                                  | Yes        |
| Compatibility tab persistence      | Toggle between Trainer and Proton subtabs                             | `forceMount`-backed state stays mounted and no subtab silently resets                                       | Yes        |
| Route smoke regression             | Browser dev mode route sweep for all dashboard routes                 | Screenshots capture each route and `attachConsoleCapture` sees no page errors or `console.error` calls      | Yes        |

### Edge Cases Checklist

- [ ] Health route still behaves correctly with only cached snapshots and no fresh summary.
- [ ] Host Tools still renders a useful page when `snapshot` is missing but `error` is present.
- [ ] Proton Manager keeps install/uninstall progress rows stable during async transitions.
- [ ] Compatibility trainer tab still handles zero entries and inline error text cleanly.
- [ ] Dashboard restyles do not create a second unregistered scroll owner.
- [ ] `host-tools` and `proton-manager` browser smoke fixtures contain enough mock data to exercise the redesigned route bodies.

---

## Validation Commands

### Static Analysis

```bash
npm run typecheck
```

EXPECT: Zero TypeScript errors across the redesigned route components and any new shared dashboard primitives.

### Unit / Integration Tests

```bash
npm test
```

EXPECT: Vitest stays green, including the new dashboard route test coverage.

### Full Lint Pass

```bash
./scripts/lint.sh
```

EXPECT: Biome and repo lint checks pass after the CSS/component churn from the restyle.

### Browser Smoke

```bash
npm run test:smoke
```

EXPECT: Browser-dev smoke captures `health`, `host-tools`, `proton-manager`, and `compatibility` without page errors or `console.error`.

### Manual Validation

- [ ] Run `./scripts/dev-native.sh --browser` and visually inspect the four dashboard routes at `1920x1080`.
- [ ] Repeat manual inspection at `3440x1440` to confirm the redesigned dashboard routes still feel coherent inside the ultrawide shell.
- [ ] Verify Health retry/recheck/version-scan actions still respond and the table can expand/collapse rows.
- [ ] Verify Host Tools filter, search, probe, and dismiss-nag interactions still work.
- [ ] Verify Proton Manager provider switching, install, cancel, dismiss, and uninstall confirmation still work.
- [ ] Verify Compatibility tab switching preserves state and Proton installs still target the default compat-tools directory.

---

## Acceptance Criteria

- [ ] All eight tasks are completed.
- [ ] The four Phase 9 routes share a coherent unified-dashboard visual language.
- [ ] All existing Health, Host Tools, Proton Manager, and Compatibility behavior is preserved.
- [ ] `host-tools` and `proton-manager` are added to the browser smoke route sweep.
- [ ] `npm run typecheck`, `npm test`, `./scripts/lint.sh`, and `npm run test:smoke` pass.
- [ ] No new persistence, IPC, or backend changes are introduced.

## Completion Checklist

- [ ] Shared dashboard route chrome is reusable and not route-specific overfitting.
- [ ] Error/empty/loading states still render inline within each route shell.
- [ ] Scroll ownership stays compatible with `useScrollEnhance`.
- [ ] Proton Manager split keeps the parent async state contract intact.
- [ ] Tests follow the existing provider-backed shell/page patterns.
- [ ] Documentation artifact is self-contained enough for single-pass implementation.
- [ ] No unnecessary scope was added outside Phase 9 dashboard routes.

## Risks

| Risk                                                                                 | Likelihood | Impact | Mitigation                                                                                                                                                |
| ------------------------------------------------------------------------------------ | ---------- | ------ | --------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Shared dashboard chrome drifts into Phase 10 route-specific assumptions              | Medium     | Medium | Keep the new shared primitive limited to generic section framing and shared spacing; push route-unique selectors back into route-local CSS.               |
| Proton Manager split changes async install/uninstall behavior                        | Medium     | High   | Keep action state in `ProtonManagerPanel`, make children display-focused, and add focused tests around progress/error/warning banners.                    |
| Health route rewrap introduces nested scroll or broken row expansion                 | Medium     | High   | Preserve the current table/panel components and existing body-scroll owner; avoid adding a second overflow container without updating `useScrollEnhance`. |
| Smoke additions expose incomplete mock coverage for `host-tools` or `proton-manager` | High       | Medium | Fix fixture data or mock handlers first; do not relax the no-error smoke assertions.                                                                      |
| Route-by-route CSS adjustments fragment into one-off styles again                    | Medium     | Medium | Put genuinely reusable chrome in `dashboard-routes.css` and limit route-local CSS to data-specific layout differences.                                    |

## Notes

- Research dispatch used the requested **parallel sub-agent** mode. Findings were merged with local repo inspection for the final artifact.
- Worktree annotations are intentionally omitted because the user requested `--no-worktree`.
- This plan treats Host Tools as the least-delta dashboard, Health/Compatibility as composition-heavy rewraps, and Proton Manager as the only route that should also absorb a structural split for maintainability.
