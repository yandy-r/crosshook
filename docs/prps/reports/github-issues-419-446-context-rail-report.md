# Implementation Report: GitHub Issues 419 / 446 — Context Rail

## Summary

Implemented the Library ultrawide context rail: visibility contract in `contextRailVariants.ts`, compositional `ContextRail` pane (host readiness, pinned profiles, 7-day launch buckets, recent successful sessions), shell integration in `AppShell`, `libraryShellMode` in `InspectorSelectionContext`, explicit mode updates from `LibraryPage` (no `useEffect` sync loop), route-leave reset from `AppShell`, scroll-enhance selector, and theme styles. Added/extended tests for shell visibility, library mode propagation, breakpoint vs rail width gate, and scroll selector registration.

## Assessment vs Reality

| Metric        | Predicted (Plan) | Actual       |
| ------------- | ---------------- | ------------ |
| Complexity    | Medium           | Medium       |
| Confidence    | (plan)           | High (logic) |
| Files Changed | ~10              | 12           |

## Tasks Completed

| #   | Task                          | Status   | Notes                                            |
| --- | ----------------------------- | -------- | ------------------------------------------------ |
| 1.1 | `contextRailVariants.ts`      | Complete | Width gate 3400px; re-exports `LibraryShellMode` |
| 1.2 | `ContextRail.tsx`             | Complete | Host hero, pins, chart, sessions                 |
| 1.3 | `theme.css`                   | Complete | `.crosshook-context-rail*`                       |
| 2.1 | Shell + context + LibraryPage | Complete | Mode via handlers + `AppShell` ref reset         |
| 2.2 | `useScrollEnhance`            | Complete | `SCROLL_ENHANCE_SELECTORS` export                |
| 3.1 | `AppShell.test.tsx`           | Complete | 3440 / 2560 / route / detail                     |
| 3.2 | Library + breakpoint tests    | Complete | `useScrollEnhance.test.ts` added                 |

## Validation Results

| Level           | Status    | Notes                                                                                                                                                            |
| --------------- | --------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Static Analysis | Pass      | `npm run typecheck`, `./scripts/lint.sh` (Biome warnings remain in `LibraryToolbar.tsx` — pre-existing)                                                          |
| Unit Tests      | Pass      | `npm test` — 115 tests (full `crosshook-native` suite)                                                                                                           |
| Build           | Pass      | `npm run build` (crosshook-native)                                                                                                                               |
| Integration     | See below | `npm run test:smoke` fails on **main** as well (library route `Maximum update depth` console.error); treat as pre-existing / env — not introduced by this branch |
| Edge Cases      | Partial   | Manual checklist in plan                                                                                                                                         |

## Files Changed

| File                                                                       | Action   |
| -------------------------------------------------------------------------- | -------- |
| `src/crosshook-native/src/components/layout/contextRailVariants.ts`        | CREATED  |
| `src/crosshook-native/src/components/layout/ContextRail.tsx`               | CREATED  |
| `src/crosshook-native/src/components/layout/AppShell.tsx`                  | UPDATED  |
| `src/crosshook-native/src/context/InspectorSelectionContext.tsx`           | UPDATED  |
| `src/crosshook-native/src/components/pages/LibraryPage.tsx`                | UPDATED  |
| `src/crosshook-native/src/hooks/useScrollEnhance.ts`                       | UPDATED  |
| `src/crosshook-native/src/styles/theme.css`                                | UPDATED  |
| `src/crosshook-native/src/components/layout/__tests__/AppShell.test.tsx`   | UPDATED  |
| `src/crosshook-native/src/components/pages/__tests__/LibraryPage.test.tsx` | UPDATED  |
| `src/crosshook-native/src/hooks/__tests__/useBreakpoint.test.tsx`          | UPDATED  |
| `src/crosshook-native/src/hooks/__tests__/useScrollEnhance.test.ts`        | CREATED  |
| `docs/prps/plans/completed/github-issues-419-446-context-rail.plan.md`     | ARCHIVED |

## Deviations from Plan

- **Library shell mode**: Sync is done in `handleOpenGameDetail` / `handleBackFromDetail` plus `AppShell` route reset via a ref-backed effect, instead of a `pageMode` `useEffect` (avoided dependency churn and matched investigation of smoke `Maximum update depth` warnings).
- **`handleExecuteCommand` deps**: Biome fix removed `setRoute` from the `useCallback` dependency list (stable `useState` setter).
- **Most-played**: Implemented as top recent **successful** sessions for the focus profile in the last 7 days (per-profile IPC only), not cross-library aggregation.

## Issues Encountered

- Playwright `test:smoke` reports `Maximum update depth exceeded` originating from `LibraryPage` on the library route; **the same failure reproduces on `main`** in this environment, so it was not treated as a regression from this feature.
- The plan file was not tracked by git; it was moved to `docs/prps/plans/completed/` with a filesystem `mv` (equivalent archive outcome to `git mv`).

## Tests Written / Updated

| Test file                  | Coverage                                                               |
| -------------------------- | ---------------------------------------------------------------------- |
| `AppShell.test.tsx`        | Context rail visibility (3440 vs 2560), non-library route, detail mode |
| `LibraryPage.test.tsx`     | `libraryShellMode` via harness probe                                   |
| `useBreakpoint.test.tsx`   | Rail width gate vs `uw` bucket                                         |
| `useScrollEnhance.test.ts` | Single `.crosshook-context-rail__body` registration                    |

## Next Steps

- [ ] Run `/code-review` on the branch
- [ ] Investigate smoke `Maximum update depth` on `main` (out of scope for this plan but blocks trusting smoke)
- [ ] Create PR via `/prp-pr` when ready

## Worktree Summary

`--no-worktree` was used; no Claude worktrees.
