# Implementation Report: Unified Desktop Phase 4 (Library + Inspector Rail)

## Summary

Implemented the Phase 4 library surface and persistent inspector rail: `routeMetadata` inspector contract, `Inspector` shell with error boundary, breakpoint-derived widths, `GameInspector` for the library route, `InspectorSelectionContext` + `AppShell` third `Panel`, redesigned `LibraryCard` (hover reveal + favorite heart), toolbar sort/filter chips and ⌘K placeholder, scroll hook + CSS for sidebar nav and inspector body, tests (unit + integration + Playwright), and a GitHub follow-up issue for launch-history IPC.

## Assessment vs Reality

| Metric        | Predicted (Plan) | Actual                          |
| ------------- | ---------------- | ------------------------------- |
| Complexity    | Large            | Large                           |
| Confidence    | (n/a in plan)    | High after full test/build pass |
| Files Changed | ~16 / ~8 new     | See git diff                    |

## Tasks Completed

| #   | Task                               | Status   | Notes                                                                   |
| --- | ---------------------------------- | -------- | ----------------------------------------------------------------------- |
| 1.1 | routeMetadata contract             | Complete | `InspectorBodyProps`, `SelectedGame`, optional `inspectorComponent`     |
| 1.2 | useScrollEnhance selectors         | Complete |                                                                         |
| 1.3 | breakpoint test helper             | Complete | `breakpointResult` + `BREAKPOINT_PX` + unit test                        |
| 2.1 | Inspector + variants + CSS + tests | Complete |                                                                         |
| 2.2 | LibraryCard chrome                 | Complete | Single-click selects when `onSelect` set; double-click opens details    |
| 2.3 | LibraryToolbar chips               | Complete | Fieldsets for a11y; palette trigger                                     |
| 3.1 | GameInspector                      | Complete | Follow-up issue [#456](https://github.com/yandy-r/crosshook/issues/456) |
| 3.2 | makeProfileHealthReport            | Complete |                                                                         |
| 3.3 | routeMetadata GameInspector        | Complete |                                                                         |
| 4.1 | AppShell + Sidebar scroll          | Complete | `InspectorSelectionProvider` wired in `App.tsx`                         |
| 4.2 | Library selection bridge           | Complete | Stabilized `setInspectorSelection` vs `summaries` reference churn       |
| 4.3 | Tests + smoke                      | Complete | Deck width tests use shell width `<1100` (matches `useBreakpoint`)      |

## Validation Results

| Level           | Status | Notes                                    |
| --------------- | ------ | ---------------------------------------- |
| Static Analysis | Pass   | `npm run typecheck`, `npm run lint`      |
| Unit Tests      | Pass   | `npm test` (82 tests)                    |
| Build           | Pass   | `npm run build`                          |
| Integration     | Pass   | AppShell / LibraryPage tests             |
| Edge Cases      | Pass   | Playwright `library inspector` smoke (2) |

## Files Changed

High-touch paths (non-exhaustive): `App.tsx`, `routeMetadata.ts`, `AppShell.tsx`, `Sidebar.tsx`, `Inspector.tsx`, `inspectorVariants.ts`, `InspectorSelectionContext.tsx`, `LibraryPage.tsx`, `LibraryGrid.tsx`, `LibraryCard.tsx`, `LibraryToolbar.tsx`, `GameInspector.tsx`, `useScrollEnhance.ts`, `layout.css`, `library.css`, `sidebar.css`, `fixtures.ts`, `breakpoint.ts`, multiple `__tests__` files, `tests/smoke.spec.ts`, `useAccessibilityEnhancements.ts`, `runtime.test.ts`.

## Deviations from Plan

1. **`InspectorSelectionProvider` placement**: Wrapped in `App.tsx` (around `AppShell`) instead of only inside `AppShell`/`ContentArea`, so `useInspectorSelection()` works without splitting `AppShell`.
2. **Card click vs modal**: When `onSelect` is set, single-click selects for the inspector; double-click on the hitbox still opens details (preserves modal path without blocking single-click selection).
3. **Deck breakpoint in tests**: Plan text mentioned 1280px “deck”; code uses `BreakpointSize` deck at `<1100px`. Tests and smoke use **1024×800** for “inspector absent”.
4. **`recentlyLaunched` filter**: Returns an empty list in v1 (no IPC); chips still exercise UI.
5. **Lint hygiene**: Fixed `forEach` callback return in `useAccessibilityEnhancements.ts` and removed unused `MockWindow` in `runtime.test.ts` so `npm run lint` stays clean.

## Issues Encountered

- **Infinite render loop** when syncing `inspectorSelection` to `summaries` — fixed with guarded `setInspectorSelection` functional updates (reference + key field equality).
- **Playwright console**: “Maximum update depth” during first smoke run — resolved by the guard above.
- **TypeScript**: `setInspectorSelection` was typed as a plain value setter; `LibraryPage` uses a functional updater — fixed by typing it as `Dispatch<SetStateAction<SelectedGame | undefined>>` in `InspectorSelectionContext.tsx`.

## Tests Written

| Test file                 | Role                                     |
| ------------------------- | ---------------------------------------- |
| `Inspector.test.tsx`      | Empty route, error boundary              |
| `LibraryToolbar.test.tsx` | Chips, palette, tab order                |
| `GameInspector.test.tsx`  | Empty state, actions, health heading     |
| `LibraryPage.test.tsx`    | Inspector sync, sort chip, palette debug |
| `LibraryCard.test.tsx`    | Heart + hover-reveal                     |
| `LibraryGrid.test.tsx`    | `onSelect`                               |
| `AppShell.test.tsx`       | `sidebar` + `inspector` testids by width |
| `breakpoint.test.ts`      | Helper sanity                            |
| `smoke.spec.ts`           | Library inspector E2E                    |

## Next Steps

- [ ] `/code-review` on the branch
- [ ] Open PR: `feat(native): add persistent inspector rail + Library redesign (phase 4)` — link issues #443, #416, and follow-up [#456](https://github.com/yandy-r/crosshook/issues/456)
- [ ] Attach desk + deck screenshots to the PR body (manual)

## Worktree Summary

`--no-worktree` was used; no auxiliary worktrees were created.
