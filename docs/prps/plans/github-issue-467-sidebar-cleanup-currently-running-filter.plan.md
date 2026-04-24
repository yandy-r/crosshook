# Plan: GitHub Issue 467 sidebar cleanup + currently running filter

## Summary

Implement PRD Phase 2 by simplifying the sidebar Game section to `Library` only, then promoting `Favorites` and `Currently Playing` into first-class sidebar entries that open Library with an explicit filter state. The change adds a new `currentlyRunning` library filter key, threads navigation options through shell routing, and introduces a dedicated running-profile hook so filtering is data-driven instead of hardcoded.

## User Story

As a CrossHook user, I want Favorites and Currently Playing to be directly selectable from the sidebar, so I can jump to focused Library views without extra chip clicks or route detours.

## Problem -> Solution

Current state: sidebar still exposes duplicate top-level `Profiles` and `Launch` entries in the Game section, while Favorites only exists as an in-page chip and there is no currently-running library filter.

Desired state: sidebar Game section contains only `Library`; Collections includes `Favorites` and `Currently Playing` as library-filter entries; AppShell forwards route options; Library page can apply `favorites` and `currentlyRunning` filters immediately.

## Metadata

- **Complexity**: Medium
- **Source PRD**: `docs/prps/prds/unified-desktop-hero-detail-consolidation.prd.md`
- **PRD Phase**: Phase 2 - Sidebar cleanup + Favorites + Currently Playing
- **GitHub Issue**: [#467](https://github.com/yandy-r/crosshook/issues/467)
- **Estimated Files**: 10

---

## Batches

Tasks grouped by dependency for parallel execution. Tasks within the same batch run concurrently; batches run in order.

| Batch | Tasks    | Depends On | Parallel Width |
| ----- | -------- | ---------- | -------------- |
| B1    | 1.1, 1.2 | -          | 2              |
| B2    | 2.1, 2.2 | B1         | 2              |
| B3    | 3.1, 3.2 | B2         | 2              |

- **Total tasks**: 6
- **Total batches**: 3
- **Max parallel width**: 2

Same-file collision check: no two tasks in the same batch modify the same file.

---

## UX Design

### Before

- Sidebar Game section shows `Library`, `Profiles`, and `Launch`.
- Favorites is reachable only through the Library toolbar chip.
- No dedicated `Currently Playing` entry or running-status filter.

### After

- Sidebar Game section shows only `Library`.
- Sidebar Collections includes `Favorites` and `Currently Playing`.
- Clicking either entry routes to Library and preselects its matching filter chip.

### Interaction Changes

| Touchpoint                  | Before                          | After                                  | Notes                                            |
| --------------------------- | ------------------------------- | -------------------------------------- | ------------------------------------------------ |
| Sidebar Game section        | `Library`, `Profiles`, `Launch` | `Library` only                         | Aligns with consolidation direction in PRD       |
| Sidebar Collections section | Collection list only            | Collection list + fixed filter entries | Adds quick filters without replacing collections |
| Favorites access            | Toolbar chip only               | Sidebar entry + toolbar chip           | Both resolve to same `LibraryFilterKey`          |
| Running games view          | Not available as a filter       | `currentlyRunning` filter + chip       | Driven by a hook-backed `Set<string>`            |

---

## Mandatory Reading

Files that MUST be read before implementing:

| Priority       | File                                                                            | Lines           | Why                                                          |
| -------------- | ------------------------------------------------------------------------------- | --------------- | ------------------------------------------------------------ |
| P0 (critical)  | `src/crosshook-native/src/components/layout/Sidebar.tsx`                        | 1-263           | Sidebar section model, route typing, and trigger rendering   |
| P0 (critical)  | `src/crosshook-native/src/components/layout/AppShell.tsx`                       | 65-242, 326-462 | Central route state, sidebar wiring, and command routing     |
| P0 (critical)  | `src/crosshook-native/src/components/layout/ContentArea.tsx`                    | 16-67           | `onNavigate` contract passed into pages                      |
| P0 (critical)  | `src/crosshook-native/src/components/pages/LibraryPage.tsx`                     | 33-158, 281-340 | Filter state lifecycle and page-level routing handlers       |
| P1 (important) | `src/crosshook-native/src/components/library/LibraryToolbar.tsx`                | 1-87            | Filter chip options and aria-pressed behavior                |
| P1 (important) | `src/crosshook-native/src/types/library.ts`                                     | 1-32            | Library filter type contract used across page + toolbar      |
| P1 (important) | `src/crosshook-native/src/components/layout/__tests__/Sidebar.test.tsx`         | 25-54           | Sidebar structure and section-order invariants               |
| P1 (important) | `src/crosshook-native/src/components/library/__tests__/LibraryToolbar.test.tsx` | 18-85           | Filter event and keyboard/tab-order assertions               |
| P1 (important) | `src/crosshook-native/src/components/pages/__tests__/LibraryPage.test.tsx`      | 41-213          | Library harness pattern and filter/shell behavior test style |
| P2 (reference) | `src/crosshook-native/src/hooks/useLaunchState.ts`                              | 230-270         | Existing `check_game_running` command usage pattern          |
| P2 (reference) | `src/crosshook-native/package.json`                                             | 9-27            | Canonical typecheck/lint/test/smoke commands                 |

## External Documentation

| Topic | Source | Key Takeaway                                              |
| ----- | ------ | --------------------------------------------------------- |
| N/A   | N/A    | Internal codebase patterns are sufficient for this issue. |

---

## Patterns to Mirror

Code patterns discovered in the codebase. Follow these exactly.

### NAMING_CONVENTION

```ts
// SOURCE: src/crosshook-native/src/components/pages/LibraryPage.tsx
const [filterKey, setFilterKey] = useState<LibraryFilterKey>('all');
```

Use `*Key` naming for filter state and `set*` setter naming for local page state.

### TYPE_DEFINITION

```ts
// SOURCE: src/crosshook-native/src/types/library.ts
export type LibraryFilterKey = 'all' | 'favorites' | 'installed' | 'recentlyLaunched';
```

Extend narrow unions in one source-of-truth type file, then consume that type from toolbar/page props.

### SIDEBAR_SECTION_PATTERN

```ts
// SOURCE: src/crosshook-native/src/components/layout/Sidebar.tsx
type SidebarSection = SidebarRouteSection | SidebarCollectionsSection;
const SIDEBAR_SECTIONS: SidebarSection[] = [
  /* ... */
];
```

Sidebar behavior is declarative via typed section variants plus a single render switch.

### ROUTE_CONTRACT_PATTERN

```tsx
// SOURCE: src/crosshook-native/src/components/layout/AppShell.tsx
<Sidebar activeRoute={route} onNavigate={setRoute} ... />
<ContentArea route={route} onNavigate={setRoute} ... />
```

Route updates originate in AppShell and are forwarded consistently into Sidebar + ContentArea.

### ERROR_HANDLING

```ts
// SOURCE: src/crosshook-native/src/components/pages/LibraryPage.tsx
void toggleFavorite(name, !current).catch(() => {
  setSummaries((prev) => prev.map((s) => (s.name === name ? { ...s, isFavorite: current } : s)));
});
```

Use optimistic update + catch-and-revert for UI-state mutations that depend on async command results.

### LOGGING_PATTERN

```ts
// SOURCE: src/crosshook-native/src/hooks/useLibrarySummaries.ts
console.error('Failed to fetch profile summaries', err);
setError(String(err));
```

Log at hook boundary, then expose recoverable state to UI (avoid throwing from React render paths).

### TEST_STRUCTURE

```tsx
// SOURCE: src/crosshook-native/src/components/library/__tests__/LibraryToolbar.test.tsx
await user.click(screen.getByRole('button', { name: 'Favorites' }));
expect(onFilterChange).toHaveBeenCalledWith('favorites');
```

Test filter behavior through accessible button labels and callback payload assertions.

### CONFIGURATION

```json
// SOURCE: src/crosshook-native/package.json
"typecheck": "tsc --noEmit && tsc -p tsconfig.test.json --noEmit",
"test": "vitest run",
"test:smoke": "playwright test"
```

Use package-defined scripts for verification; avoid ad-hoc command variants.

---

## Files to Change

| File                                                                            | Action | Justification                                                                                       |
| ------------------------------------------------------------------------------- | ------ | --------------------------------------------------------------------------------------------------- |
| `src/crosshook-native/src/components/layout/Sidebar.tsx`                        | UPDATE | Add `library-filter` section-item variant and remove Game-section `profiles`/`launch` items         |
| `src/crosshook-native/src/components/layout/AppShell.tsx`                       | UPDATE | Expand route navigation contract to accept optional route intent payload                            |
| `src/crosshook-native/src/components/layout/ContentArea.tsx`                    | UPDATE | Forward richer `onNavigate` signature to Library and other page callers                             |
| `src/crosshook-native/src/components/pages/LibraryPage.tsx`                     | UPDATE | Accept initial filter intent, apply `currentlyRunning` filtering, preserve existing detail behavior |
| `src/crosshook-native/src/components/library/LibraryToolbar.tsx`                | UPDATE | Add `Running` chip mapped to `currentlyRunning`                                                     |
| `src/crosshook-native/src/types/library.ts`                                     | UPDATE | Extend `LibraryFilterKey` with `currentlyRunning`                                                   |
| `src/crosshook-native/src/hooks/useRunningProfiles.ts`                          | CREATE | New reusable hook that returns running profile names as `Set<string>`                               |
| `src/crosshook-native/src/components/layout/__tests__/Sidebar.test.tsx`         | UPDATE | Assert Profiles/Launch removal and presence of new fixed filter entries                             |
| `src/crosshook-native/src/components/library/__tests__/LibraryToolbar.test.tsx` | UPDATE | Assert `Running` filter chip emits `currentlyRunning`                                               |
| `src/crosshook-native/src/components/pages/__tests__/LibraryPage.test.tsx`      | UPDATE | Assert initial-filter navigation behavior and running-filter path                                   |

## NOT Building

- AppRoute union shrink (`'profiles' | 'launch'` removal) - explicitly deferred to Phase 8.
- Deleting `ProfilesPage` / `LaunchPage` routes - handled by later PRD phases.
- New backend IPC surface for running sessions - use existing frontend command patterns.
- Full Playwright route-suite rewrite - keep existing smoke green and add minimal coverage only if needed.
- Any hero-detail tabs, hook schema, or route-deletion work from Phases 3+.

---

## Step-by-Step Tasks

### Task 1.1: Sidebar section model + fixed library-filter entries - Depends on [none]

- **BATCH**: B1
- **ACTION**: Refactor sidebar section typing so Collections can render both dynamic collections and fixed `library-filter` entries, then remove Game `profiles`/`launch`.
- **IMPLEMENT**: In `Sidebar.tsx`, add a `SidebarLibraryFilterItem` variant with `{ type: 'library-filter'; filterKey: LibraryFilterKey; label; icon; }`, keep route items as `{ type: 'route'; route; ... }`, and update render logic to call `onNavigate('library', { libraryFilter: filterKey })` for filter items. Game section keeps only `library`; Collections section includes `Favorites` and `Currently Playing`.
- **MIRROR**: `SIDEBAR_SECTIONS` declarative shape + `SidebarSectionBlock` switch in `Sidebar.tsx`.
- **IMPORTS**: `LibraryFilterKey` type from `@/types/library`; `HeartIcon`/`PlayIcon` from sidebar icon module if available.
- **GOTCHA**: Do not remove `AppRoute` union values in this phase; Phase 8 owns route shrink.
- **VALIDATE**: `cd src/crosshook-native && npm test -- --run src/components/layout/__tests__/Sidebar.test.tsx`

### Task 1.2: Library filter contract update (`currentlyRunning`) - Depends on [none]

- **BATCH**: B1
- **ACTION**: Extend library filter typing and toolbar chip options to expose running-state filtering.
- **IMPLEMENT**: Update `types/library.ts` to append `'currentlyRunning'` to `LibraryFilterKey`; update `LibraryToolbar.tsx` `FILTER_OPTIONS` with `{ key: 'currentlyRunning', label: 'Running' }`; ensure aria-pressed behavior remains unchanged.
- **MIRROR**: Existing literal-union + `FILTER_OPTIONS as const` style in `types/library.ts` and `LibraryToolbar.tsx`.
- **IMPORTS**: No new imports expected.
- **GOTCHA**: Keep existing `'recentlyLaunched'` key even if not surfaced in toolbar; do not remove unrelated union members.
- **VALIDATE**: `cd src/crosshook-native && npm test -- --run src/components/library/__tests__/LibraryToolbar.test.tsx`

### Task 2.1: Add `useRunningProfiles` hook - Depends on [1.2]

- **BATCH**: B2
- **ACTION**: Introduce a hook that derives currently running profile names for filter use.
- **IMPLEMENT**: Create `src/hooks/useRunningProfiles.ts` returning `{ runningProfiles: Set<string>; refresh: () => Promise<void>; }` or equivalent minimal API. Start from current profile summaries and `check_game_running` command pattern; fail-open to empty set on errors; avoid throwing from hook render paths.
- **MIRROR**: Async hook patterns in `useLibrarySummaries.ts` and command calls in `useLaunchState.ts`.
- **IMPORTS**: `useEffect`, `useMemo`, `useState`, `callCommand`, and profile summary sources already used by library-facing hooks.
- **GOTCHA**: Keep polling/subscription strategy simple and testable; no global singleton registry dependency should block this phase.
- **VALIDATE**: `cd src/crosshook-native && npm run typecheck`

### Task 2.2: Route intent plumbing + LibraryPage running-filter behavior - Depends on [1.1, 1.2]

- **BATCH**: B2
- **ACTION**: Thread optional navigation intent from sidebar through shell/content/page and apply filter intent in LibraryPage.
- **IMPLEMENT**: Expand `onNavigate` signature in `Sidebar.tsx`, `AppShell.tsx`, `ContentArea.tsx`, and `LibraryPage.tsx` to allow `{ libraryFilter?: LibraryFilterKey; heroDetailTab?: ...; profileName?: ... }` without breaking existing callers. In `LibraryPage.tsx`, add support for initializing/updating `filterKey` from navigation intent and add `currentlyRunning` case in filter switch using `useRunningProfiles`.
- **MIRROR**: Existing callback threading between `AppShell -> ContentArea -> LibraryPage`; filter switch style in `LibraryPage.tsx`.
- **IMPORTS**: `LibraryFilterKey` type and `useRunningProfiles` hook in `LibraryPage.tsx`.
- **GOTCHA**: Preserve current launch/edit flows and page mode transitions while adding intent payload support.
- **VALIDATE**: `cd src/crosshook-native && npm test -- --run src/components/pages/__tests__/LibraryPage.test.tsx`

### Task 3.1: Update focused RTL coverage for sidebar + toolbar + library filter flow - Depends on [2.2]

- **BATCH**: B3
- **ACTION**: Expand existing test suites to lock the new behavior and prevent navigation regression.
- **IMPLEMENT**: In `Sidebar.test.tsx`, assert Game section no longer includes Profiles/Launch tabs and Collections exposes Favorites/Currently Playing controls. In `LibraryToolbar.test.tsx`, assert clicking `Running` emits `currentlyRunning` and tab order still passes. In `LibraryPage.test.tsx`, add a case for intent-driven filter selection and running-filter behavior with mocked command responses.
- **MIRROR**: Existing `renderWithMocks` harness and role-based assertions used in these same files.
- **IMPORTS**: `userEvent`, `waitFor`, and existing test helper imports only.
- **GOTCHA**: Keep assertions role/label-driven; avoid brittle className selectors unless no accessible role exists.
- **VALIDATE**: `cd src/crosshook-native && npm test -- --run src/components/layout/__tests__/Sidebar.test.tsx src/components/library/__tests__/LibraryToolbar.test.tsx src/components/pages/__tests__/LibraryPage.test.tsx`

### Task 3.2: End-to-end regression check + command verification - Depends on [2.1, 2.2]

- **BATCH**: B3
- **ACTION**: Confirm this phase does not regress baseline quality gates and route smoke.
- **IMPLEMENT**: Run project standard checks (`typecheck`, targeted tests, lint, smoke). If smoke assertions fail solely due to explicit sidebar label changes, update only the affected assertions while preserving route coverage intent.
- **MIRROR**: Existing package scripts in `src/crosshook-native/package.json`.
- **IMPORTS**: N/A (verification task).
- **GOTCHA**: Do not preemptively rewrite smoke scenarios that belong to later PRD phases; keep this scoped to phase-2 behavior.
- **VALIDATE**: `cd src/crosshook-native && npm run typecheck && npm test && npm run test:smoke`

---

## Testing Strategy

### Unit Tests

| Test                             | Input                                   | Expected Output                                                                       | Edge Case? |
| -------------------------------- | --------------------------------------- | ------------------------------------------------------------------------------------- | ---------- |
| Sidebar fixed entries render     | Render `Sidebar` in full variant        | `Favorites` + `Currently Playing` present; `Profiles`/`Launch` absent in Game section | Yes        |
| Toolbar running chip             | Click `Running` chip                    | `onFilterChange('currentlyRunning')`                                                  | No         |
| Library currentlyRunning filter  | Mock running set with one profile       | Only running profile cards remain visible                                             | Yes        |
| Navigation intent filter handoff | Trigger sidebar library-filter navigate | Library route active + matching chip `aria-pressed="true"`                            | Yes        |
| Existing favorite behavior       | Toggle Favorites entry/chip             | Favorites flow still works unchanged                                                  | Regression |

### Edge Cases Checklist

- [x] Empty running set shows an empty/filtered library state without crash
- [x] Running filter with no favorites still renders toolbar and page controls
- [x] Invalid command/error in running-status fetch falls back to empty set
- [x] Existing route-only sidebar entries still navigate as before
- [ ] High-frequency running-state churn (can be covered with lightweight polling debounce assertions)

---

## Validation Commands

### Static Analysis

```bash
cd /home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native
npm run typecheck
```

EXPECT: Zero type errors in app and test tsconfigs.

### Unit / Integration Tests

```bash
cd /home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native
npm test -- --run src/components/layout/__tests__/Sidebar.test.tsx \
  src/components/library/__tests__/LibraryToolbar.test.tsx \
  src/components/pages/__tests__/LibraryPage.test.tsx
```

EXPECT: New + existing behavior tests pass.

### Full Test Suite

```bash
cd /home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native
npm test
```

EXPECT: No regressions outside phase-2 scope.

### Lint

```bash
cd /home/yandy/Projects/github.com/yandy-r/crosshook
./scripts/lint.sh
```

EXPECT: Biome/type checks pass for touched frontend files.

### Browser Validation

```bash
cd /home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native
npm run test:smoke
```

EXPECT: Existing smoke remains green; only explicitly affected assertions need updates.

---

## Acceptance Criteria

- [ ] Sidebar Game section renders with `Library` only.
- [ ] Sidebar includes `Favorites` and `Currently Playing` quick-filter entries.
- [ ] Clicking `Favorites` opens Library with Favorites chip active.
- [ ] Clicking `Currently Playing` opens Library with Running chip active.
- [ ] `LibraryFilterKey` includes `currentlyRunning`.
- [ ] Library toolbar exposes a `Running` filter chip.
- [ ] Library page correctly filters cards by running profile names.
- [ ] Existing typecheck, lint, unit tests, and smoke checks pass.

## Completion Checklist

- [ ] All 6 tasks completed with batch dependency order preserved.
- [ ] No AppRoute deletion/scope creep beyond phase-2 requirements.
- [ ] New hook and navigation contract are documented in code comments where non-obvious.
- [ ] Tests cover both positive path and fallback/error path for running filter data.
- [ ] No new dependency introduced.
- [ ] Plan remains self-contained for `/prp-implement` execution.

## Risks

| Risk                                                             | Likelihood | Impact | Mitigation                                                                    |
| ---------------------------------------------------------------- | ---------- | ------ | ----------------------------------------------------------------------------- |
| Running-status source is inconsistent in webdev/mock mode        | Medium     | Medium | Fail-open to empty set; assert deterministic mocks in tests                   |
| Navigation intent payload breaks existing `onNavigate` callers   | Medium     | High   | Keep payload optional and backwards compatible; typecheck all route callsites |
| Sidebar item model refactor introduces accessibility regressions | Low        | Medium | Preserve `Tabs.Trigger` role/aria patterns and verify with existing tests     |
| Smoke assertions tied to old labels/routes become flaky          | Medium     | Low    | Update only directly impacted assertions and keep scope narrow                |

## Notes

- Research dispatch used parallel `prp-researcher` passes across patterns/quality/infra categories.
- External research is not required for this issue because all contracts are internal.
- This plan intentionally aligns to PRD Phase 2 and defers route deletion + AppRoute shrink to later phases.
