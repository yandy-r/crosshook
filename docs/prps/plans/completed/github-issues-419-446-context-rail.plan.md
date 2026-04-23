# Plan: GitHub Issues 419 and 446 Context Rail

## Summary

Implement Phase 7 context rail for ultrawide layouts by adding a fourth shell pane in the Library route, populated from existing host-readiness, profile, and launch-history sources. The change keeps backend surfaces untouched, composes existing providers/hooks, and adds explicit viewport gating so the rail appears at `3440x1440` while remaining hidden at `2560x1440` per issue acceptance.

## User Story

As an ultrawide CrossHook user, I want a context rail beside the Library so that I can see host readiness and profile activity without losing focus on browsing games.

## Problem -> Solution

The current shell renders at most three panes (sidebar, content, inspector) and has no dedicated ultrawide context panel. Add a context-rail component and a shell-level visibility contract tied to route + library mode + viewport constraints, then validate visibility and content composition via targeted tests.

## Metadata

- **Complexity**: Medium
- **Source PRD**: `docs/prps/prds/unified-desktop-redesign.prd.md`
- **PRD Phase**: Phase 7 - Context rail (ultrawide only)
- **Estimated Files**: 10
- **GitHub Issues**: #446 tracking, #419 deliverable
- **Persistence Classification**: runtime-only UI composition; no new TOML settings; no new SQLite tables.

## Batches

Tasks grouped by dependency for parallel execution. Tasks within the same batch run concurrently; batches run in order.

| Batch | Tasks         | Depends On | Parallel Width |
| ----- | ------------- | ---------- | -------------- |
| B1    | 1.1, 1.2, 1.3 | -          | 3              |
| B2    | 2.1, 2.2      | B1         | 2              |
| B3    | 3.1, 3.2      | B2         | 2              |

- **Total tasks**: 7
- **Total batches**: 3
- **Max parallel width**: 3

---

## UX Design

### Before

```text
AppShell
├─ Sidebar
├─ Main Content (Library or Detail)
└─ Inspector (route-gated)
No fourth pane for operational context
```

### After

```text
AppShell (Library route only)
├─ Sidebar
├─ Main Content
├─ Inspector
└─ ContextRail (ultrawide + library mode + viewport gate)
   ├─ Host readiness summary
   ├─ Pinned profiles
   ├─ 7-day activity mini chart
   └─ Most-played list
```

### Interaction Changes

| Touchpoint             | Before                        | After                                                | Notes                                    |
| ---------------------- | ----------------------------- | ---------------------------------------------------- | ---------------------------------------- |
| Library at `3440x1440` | No context pane               | Context rail renders as fourth pane                  | Content is read-only and compositional   |
| Library at `2560x1440` | No context pane               | Still no context pane                                | Enforced by explicit visibility contract |
| Library detail mode    | No context pane               | Context rail remains hidden                          | Keeps focus on hero-detail mode          |
| Scroll behavior        | Existing shell selectors only | Context rail body participates in enhanced scrolling | Add `.crosshook-context-rail__body`      |

---

## Mandatory Reading

Files that MUST be read before implementing:

| Priority | File                                                                         | Lines          | Why                                                                    |
| -------- | ---------------------------------------------------------------------------- | -------------- | ---------------------------------------------------------------------- |
| P0       | `docs/prps/prds/unified-desktop-redesign.prd.md`                             | 259-264        | Phase 7 scope and success signal contract                              |
| P0       | `src/crosshook-native/src/components/layout/AppShell.tsx`                    | 58-74, 295-370 | Breakpoint-driven shell panel composition and inspector gating         |
| P0       | `src/crosshook-native/src/components/pages/LibraryPage.tsx`                  | 51-53, 279-351 | Library/detail mode source of truth needed for context-rail visibility |
| P0       | `src/crosshook-native/src/context/InspectorSelectionContext.tsx`             | 13-39          | Shared library-specific shell state contract location                  |
| P0       | `src/crosshook-native/src/hooks/useBreakpoint.ts`                            | 3-7, 28-39     | Current breakpoint contract (`uw >= 2200`) and related risk            |
| P1       | `src/crosshook-native/src/components/layout/inspectorVariants.ts`            | 3-12           | Existing `*ForBreakpoint` helper naming pattern                        |
| P1       | `src/crosshook-native/src/hooks/useScrollEnhance.ts`                         | 8-10, 78-127   | Selector-based scroll enhancement registry                             |
| P1       | `src/crosshook-native/src/context/HostReadinessContext.tsx`                  | 8-20           | Existing host-readiness provider and hook contract                     |
| P1       | `src/crosshook-native/src/components/host-readiness/HostToolMetricsHero.tsx` | 52-95          | Existing host-readiness summary section to mirror                      |
| P1       | `src/crosshook-native/src/components/PinnedProfilesStrip.tsx`                | 21-58          | Existing pinned profiles section style and semantics                   |
| P1       | `src/crosshook-native/src/components/layout/__tests__/AppShell.test.tsx`     | 28-34, 79-188  | Viewport-aware shell assertions and test harness patterns              |
| P1       | `src/crosshook-native/src/components/pages/__tests__/LibraryPage.test.tsx`   | 40-59, 89-188  | Existing library/detail transition test style                          |

## External Documentation

| Topic         | Source | Key Takeaway                                                                             |
| ------------- | ------ | ---------------------------------------------------------------------------------------- |
| External docs | none   | No external API/library research required; implementation is fully internal composition. |

---

## Patterns to Mirror

Code patterns discovered in the codebase. Follow these exactly.

### BREAKPOINT_VARIANT_PATTERN

```ts
// SOURCE: src/crosshook-native/src/components/layout/inspectorVariants.ts:3-8
export const INSPECTOR_WIDTHS: Record<BreakpointSize, number> = {
  uw: 360,
  desk: 320,
  narrow: 280,
  deck: 0,
} as const;
```

Use a dedicated helper map/function pair for context-rail visibility/width decisions instead of inline condition chains in `AppShell`.

### SHELL_PANEL_GATING_PATTERN

```tsx
// SOURCE: src/crosshook-native/src/components/layout/AppShell.tsx:71-74
const inspectorWidthBase = inspectorWidthForBreakpoint(breakpoint.size);
const routeHasInspector = ROUTE_METADATA[route].inspectorComponent != null;
const inspectorWidth = routeHasInspector ? inspectorWidthBase : 0;
```

Gate panel rendering through derived shell state (`hasX`, `width`) before JSX rendering.

### ROUTE_COMPONENT_CONTRACT_PATTERN

```ts
// SOURCE: src/crosshook-native/src/components/layout/routeMetadata.ts:31-40
export interface RouteMetadataEntry {
  navLabel: string;
  inspectorComponent?: ComponentType<InspectorBodyProps>;
}
```

If context-rail content needs route awareness, keep it in typed contracts rather than route-string conditionals spread across components.

### ERROR_HANDLING_PATTERN

```ts
// SOURCE: src/crosshook-native/src/components/pages/LibraryPage.tsx:124-136
setLaunchingName(name);
try {
  await selectProfile(name, { collectionId: collectionIdForLoad });
} finally {
  setLaunchingName(undefined);
}
```

For async UI actions in context-rail callbacks, use `try/finally` around transient state.

### LOGGING_PATTERN

```ts
// SOURCE: src/crosshook-native/src/hooks/useLibrarySummaries.ts:49-52
} catch (err) {
  console.error('Failed to fetch profile summaries', err);
  setError(String(err));
}
```

Log via `console.error` and surface user-visible fallback state; avoid silent failures.

### TEST_STRUCTURE_PATTERN

```tsx
// SOURCE: src/crosshook-native/src/components/layout/__tests__/AppShell.test.tsx:28-34
setInnerWidth(1280);
setInnerHeight(800);
const rectSpy = mockAppShellRect(1280, 800);
```

Mirror viewport simulation helpers for all rail visibility assertions.

### SCROLL_SELECTOR_PATTERN

```ts
// SOURCE: src/crosshook-native/src/hooks/useScrollEnhance.ts:8-10
const SCROLLABLE = '.crosshook-route-card-scroll, ... , .crosshook-inspector__body, .crosshook-hero-detail__body';
```

Register every new scrollable container selector in `SCROLLABLE`.

---

## Files to Change

| File                                                                       | Action | Justification                                                                                                          |
| -------------------------------------------------------------------------- | ------ | ---------------------------------------------------------------------------------------------------------------------- |
| `src/crosshook-native/src/components/layout/ContextRail.tsx`               | CREATE | New rail component that composes host-readiness, pinned profiles, activity, and most-played sections.                  |
| `src/crosshook-native/src/components/layout/contextRailVariants.ts`        | CREATE | Centralized visibility/size contract for context rail, including the `3440` visible and `2560` hidden acceptance rule. |
| `src/crosshook-native/src/components/layout/AppShell.tsx`                  | UPDATE | Mount context rail panel with derived visibility state and width constraints.                                          |
| `src/crosshook-native/src/context/InspectorSelectionContext.tsx`           | UPDATE | Add library-mode state needed to hide rail during detail mode without coupling `AppShell` to `LibraryPage` internals.  |
| `src/crosshook-native/src/components/pages/LibraryPage.tsx`                | UPDATE | Publish `library` vs `detail` mode into shared shell context.                                                          |
| `src/crosshook-native/src/hooks/useScrollEnhance.ts`                       | UPDATE | Add `.crosshook-context-rail__body` to `SCROLLABLE`.                                                                   |
| `src/crosshook-native/src/styles/theme.css`                                | UPDATE | Add context-rail styles that match existing `crosshook-*` naming and token usage.                                      |
| `src/crosshook-native/src/components/layout/__tests__/AppShell.test.tsx`   | UPDATE | Add viewport + route/mode assertions for context-rail visibility and content presence.                                 |
| `src/crosshook-native/src/components/pages/__tests__/LibraryPage.test.tsx` | UPDATE | Verify detail-mode state propagation to shell context and no regressions in Library transitions.                       |
| `src/crosshook-native/src/hooks/__tests__/useBreakpoint.test.tsx`          | UPDATE | Guard current breakpoint contract while documenting rail-specific override behavior.                                   |

## NOT Building

- No changes to `BREAKPOINTS` thresholds in `useBreakpoint` (global contract stays unchanged).
- No new backend IPC commands, Rust handlers, or metadata schema changes.
- No new persisted settings for context-rail visibility; behavior is derived at runtime.
- No context rail rendering outside the Library route.
- No redesign of existing inspector content or command palette behavior.

---

## Step-by-Step Tasks

### Task 1.1: Define context-rail visibility contract helper — Depends on [none]

- **BATCH**: B1
- **ACTION**: Create `src/crosshook-native/src/components/layout/contextRailVariants.ts`.
- **IMPLEMENT**: Add a typed helper that derives `visible` and `width` for the context rail from route, library mode, breakpoint bucket, and raw viewport dimensions. Keep the global breakpoint contract unchanged, but add a rail-specific viewport gate that satisfies issue acceptance (`3440x1440` visible, `2560x1440` hidden) and document it inline as intentional product behavior.
- **MIRROR**: `BREAKPOINT_VARIANT_PATTERN`, `SHELL_PANEL_GATING_PATTERN`.
- **IMPORTS**: `BreakpointSize` from `@/hooks/useBreakpoint`, `AppRoute` from `@/components/layout/Sidebar`.
- **GOTCHA**: Do not modify `BREAKPOINTS` just to satisfy rail behavior; that would create cross-feature regressions in sidebar/inspector behavior.
- **VALIDATE**: `npm run typecheck` in `src/crosshook-native` passes and helper exports are consumed without `any` types.

### Task 1.2: Build `ContextRail` with compositional sections — Depends on [none]

- **BATCH**: B1
- **ACTION**: Create `src/crosshook-native/src/components/layout/ContextRail.tsx`.
- **IMPLEMENT**: Build a read-only rail body with four sections: host readiness summary, pinned profiles, 7-day activity mini chart, and most-played list. Reuse existing context/hooks and transformation utilities where possible; if aggregation helpers are missing, keep them local and pure in this file for v1.
- **MIRROR**: `ROUTE_COMPONENT_CONTRACT_PATTERN`, `ERROR_HANDLING_PATTERN`, `LOGGING_PATTERN`.
- **IMPORTS**: `useHostReadinessContext`, `useProfileContext`, existing activity/health hooks and types used in `GameInspector`.
- **GOTCHA**: Guard empty/loading states per section so the rail renders stable chrome even when one data source is unavailable.
- **VALIDATE**: Component renders in test harness with populated and empty states; no runtime throws when providers return empty arrays.

### Task 1.3: Add context-rail styling in theme system — Depends on [none]

- **BATCH**: B1
- **ACTION**: Update `src/crosshook-native/src/styles/theme.css`.
- **IMPLEMENT**: Add `.crosshook-context-rail*` classes using existing token palette and section idioms (`eyebrow`, `pill`, `kv-row` style language). Ensure body area uses `overflow-y: auto` and a dedicated `__body` class for scroll-enhancement registration.
- **MIRROR**: existing BEM class conventions in `GameInspector` and shell panel surfaces.
- **IMPORTS**: none.
- **GOTCHA**: Do not introduce literal color values; rely on existing CSS variables.
- **VALIDATE**: Visual snapshot in tests shows class hooks present and no CSS lint regressions from formatting.

### Task 2.1: Wire shell mount and library-mode context state — Depends on [1.1, 1.2]

- **BATCH**: B2
- **ACTION**: Update `AppShell`, `InspectorSelectionContext`, and `LibraryPage`.
- **IMPLEMENT**: Extend `InspectorSelectionContext` with a minimal `libraryMode` state (`library`/`detail`) owned by `LibraryPage` and consumed by `AppShell`. In `AppShell`, derive context-rail visibility via `contextRailVariants` and conditionally mount a fourth panel with `ContextRail`.
- **MIRROR**: `SHELL_PANEL_GATING_PATTERN`, context provider shape from `InspectorSelectionContext`.
- **IMPORTS**: new helper/component imports in `AppShell`; context setter/getter in `LibraryPage`.
- **GOTCHA**: Reset `libraryMode` to `library` on `LibraryPage` unmount to avoid stale detail state when switching routes.
- **VALIDATE**: At least one integration test confirms rail unmounts when entering detail mode and remounts when returning to library mode.

### Task 2.2: Register scroll selector for context rail body — Depends on [1.2]

- **BATCH**: B2
- **ACTION**: Update `src/crosshook-native/src/hooks/useScrollEnhance.ts`.
- **IMPLEMENT**: Append `.crosshook-context-rail__body` exactly once to `SCROLLABLE`.
- **MIRROR**: `SCROLL_SELECTOR_PATTERN`.
- **IMPORTS**: none.
- **GOTCHA**: Keep selector list stable and comma-delimited; duplicate selectors complicate maintenance and diffs.
- **VALIDATE**: Unit/integration tests that exercise keyboard scroll do not regress and selector string contains the new class once.

### Task 3.1: Add shell-level viewport and route/mode visibility tests — Depends on [2.1, 2.2, 1.3]

- **BATCH**: B3
- **ACTION**: Update `src/crosshook-native/src/components/layout/__tests__/AppShell.test.tsx`.
- **IMPLEMENT**: Add assertions for: rail visible on library route at `3440x1440`; hidden at `2560x1440`; hidden on non-library routes; hidden in detail mode even on ultrawide. Use existing viewport helpers and route-navigation test harness patterns.
- **MIRROR**: `TEST_STRUCTURE_PATTERN`.
- **IMPORTS**: existing RTL/vitest imports only.
- **GOTCHA**: Use deterministic waits around layout-dependent rendering to avoid flaky width-based assertions.
- **VALIDATE**: `npm test -- src/components/layout/__tests__/AppShell.test.tsx`.

### Task 3.2: Add library/context and breakpoint contract regression tests — Depends on [2.1]

- **BATCH**: B3
- **ACTION**: Update `src/components/pages/__tests__/LibraryPage.test.tsx` and `src/hooks/__tests__/useBreakpoint.test.tsx`.
- **IMPLEMENT**: In `LibraryPage` tests, verify library/detail mode updates context as expected for shell consumers. In `useBreakpoint` tests, keep current threshold assertions intact so rail-specific gating remains isolated and does not mutate global breakpoint semantics.
- **MIRROR**: existing provider-heavy harness in `LibraryPage` tests; threshold tests in `useBreakpoint.test.tsx`.
- **IMPORTS**: existing test imports and any new context test utility exports.
- **GOTCHA**: Do not weaken existing `uw >= 2200` breakpoint assertions; this plan intentionally adds a rail-specific override rather than redefining breakpoint meaning.
- **VALIDATE**: `npm test -- src/components/pages/__tests__/LibraryPage.test.tsx src/hooks/__tests__/useBreakpoint.test.tsx`.

---

## Testing Strategy

### Unit Tests

| Test                         | Input                                                 | Expected Output                                       | Edge Case? |
| ---------------------------- | ----------------------------------------------------- | ----------------------------------------------------- | ---------- |
| Visibility gate              | Route `library`, mode `library`, viewport `3440x1440` | Context rail panel renders                            | No         |
| Visibility suppression       | Route `library`, mode `library`, viewport `2560x1440` | Context rail does not render                          | Yes        |
| Detail suppression           | Route `library`, mode `detail`, viewport `3440x1440`  | Context rail does not render                          | Yes        |
| Route suppression            | Route `settings`, viewport `3440x1440`                | Context rail does not render                          | No         |
| Scroll selector registration | Updated `SCROLLABLE` string                           | Contains `.crosshook-context-rail__body` exactly once | Yes        |
| Empty data fallback          | Host readiness/activity unavailable                   | Sections render fallback placeholders, no crash       | Yes        |

### Edge Cases Checklist

- [ ] Ultrawide width with short height does not cause layout overflow or clipped panel content
- [ ] Rail body remains independently scrollable without moving underlying content
- [ ] Switching Library -> Detail -> Library toggles rail predictably without stale state
- [ ] Route changes away from Library always unmount the rail
- [ ] Host readiness fetch errors surface fallback copy instead of blank panel
- [ ] Most-played and activity sections handle empty launch history

---

## Validation Commands

### Static Analysis

```bash
cd src/crosshook-native && npm run typecheck
```

EXPECT: Zero TypeScript errors in new layout helper/component, context changes, and tests.

### Unit Tests

```bash
cd src/crosshook-native && npm test -- src/components/layout/__tests__/AppShell.test.tsx src/components/pages/__tests__/LibraryPage.test.tsx src/hooks/__tests__/useBreakpoint.test.tsx
```

EXPECT: All targeted shell/library/breakpoint tests pass with new context-rail assertions.

### Full Test Suite

```bash
./scripts/lint.sh
```

EXPECT: Repo lint checks pass with no formatting or type regressions.

### Browser Validation

```bash
cd src/crosshook-native && npm run test:smoke
```

EXPECT: Smoke run remains green; no new console errors introduced by rail rendering logic.

### Manual Validation

- [ ] Run browser dev mode and confirm context rail appears in Library at `3440x1440`.
- [ ] Confirm context rail is absent in Library at `2560x1440`.
- [ ] Open a game detail view at `3440x1440` and verify rail hides while detail is active.
- [ ] Switch to `settings` and `host-tools` at `3440x1440` and verify rail remains hidden.
- [ ] Validate scroll wheel and Arrow keys scroll rail body when hovered/focused.

---

## Acceptance Criteria

- [ ] A dedicated `ContextRail` pane is mounted by `AppShell` only for Library-mode ultrawide scenarios.
- [ ] Context rail renders host readiness, pinned profiles, 7-day activity, and most-played sections using existing data sources.
- [ ] `3440x1440` shows the rail and `2560x1440` does not.
- [ ] Library detail mode suppresses the rail even when viewport is ultrawide.
- [ ] `.crosshook-context-rail__body` is registered in `useScrollEnhance`.
- [ ] No changes are made to core `useBreakpoint` thresholds or backend APIs.
- [ ] Typecheck, targeted tests, lint, and smoke checks pass.

## Completion Checklist

- [ ] Code follows shell variant and panel-gating conventions already used for inspector/sidebar.
- [ ] Visibility logic is centralized in a dedicated helper, not spread across JSX conditionals.
- [ ] Context state propagation between `LibraryPage` and `AppShell` is minimal and explicit.
- [ ] Error/fallback handling is present for each data section in the rail.
- [ ] Tests cover viewport, route, and mode gates plus selector registration.
- [ ] No out-of-scope backend/storage changes were introduced.
- [ ] Plan remains self-contained for single-pass implementation.

## Risks

| Risk                                                                                             | Likelihood | Impact | Mitigation                                                                                                       |
| ------------------------------------------------------------------------------------------------ | ---------- | ------ | ---------------------------------------------------------------------------------------------------------------- |
| Acceptance criteria conflicts with existing breakpoint semantics (`2560` currently maps to `uw`) | High       | High   | Keep `useBreakpoint` unchanged and add explicit rail-specific visibility helper with tests and inline rationale. |
| Library/detail mode coupling between page and shell introduces stale state                       | Medium     | Medium | Store only minimal mode enum in shared context and reset on unmount.                                             |
| Data composition for 7-day activity/most-played is sparse for new users                          | Medium     | Medium | Ship robust empty states and avoid assuming non-empty history arrays.                                            |
| New panel adds layout pressure on ultrawide but narrow-height windows                            | Medium     | Medium | Constrain rail width and add body scroll/fallback truncation styles.                                             |

## Notes

- Research dispatch mode: Parallel sub-agents (`patterns-research`, `quality-research`, `infra-research`).
- No worktree annotations were added because this plan was explicitly requested with `--no-worktree`.
- If product wants to redefine `uw` semantics globally (instead of rail-specific gating), that should be a separate scoped issue because it affects inspector/sidebar behavior and existing tests.
