# Implementation Report: Hero Detail panel contract expansion (Phase 1)

## Summary

Extended `HeroDetailPanelsProps` with three new optional fields (`updateProfile`, `profileList`, `onSetActiveTab`) and threaded them through `GameDetail` + `HeroDetailTabs`, without touching any panel body. Added `data-testid="hero-detail-profiles-tab"` and `data-testid="hero-detail-launch-tab"` to the Radix `<Tabs.Content>` roots. Added a centralized testid map (`HERO_DETAIL_TAB_TESTIDS`) and `heroDetailTabTestId` helper in `hero-detail-model.ts`. Extended test factories in `HeroDetailPanels.test.tsx` and `components.a11y.test.tsx`, added a no-op-default render smoke, two test-id presence assertions, and a panelProps-forwarding smoke in `GameDetail.test.tsx`. No user-visible change; all new fields remain optional.

## Assessment vs Reality

| Metric        | Predicted (Plan)                          | Actual                                                   |
| ------------- | ----------------------------------------- | -------------------------------------------------------- |
| Complexity    | Small (~40 LOC functional + test updates) | Small (160 insertions, 14 deletions across 7 files)      |
| Confidence    | High (self-contained, typed, reversible)  | High — plan matched reality exactly                      |
| Files Changed | 6 source + 3 tests = 9 (estimated)        | 4 source + 3 tests = 7 (GameInspector didn't need edits) |

## Tasks Completed

| #   | Task                                                          | Status   | Notes                                                                  |
| --- | ------------------------------------------------------------- | -------- | ---------------------------------------------------------------------- |
| 1.1 | Add `heroDetailTabTestId` helper                              | Complete | `b1be27d`                                                              |
| 1.2 | Extend `HeroDetailPanelsProps` shape                          | Complete | `ce0b107` — 3 biome `noUnusedFunctionParameters` warns (intentional)   |
| 2.1 | Inject `data-testid` on `Tabs.Content`                        | Complete | `c730bac`                                                              |
| 2.2 | Thread new props through `GameDetail.panelProps`              | Complete | `3c39a6e` — biome did NOT flag `useExhaustiveDependencies` (good)      |
| 3.1 | Update `HeroDetailPanels.test.tsx` + a11y factory             | Complete | `602dc51` — wrapped tests in `ProfileProvider` (see Deviations)        |
| 3.2 | Update `GameDetail.test.tsx` + `GameInspector.test.tsx` smoke | Complete | `de6efbb` — `GameInspector.test.tsx` did not need edits (typecheck OK) |

## Validation Results

| Level                | Status | Notes                                                                                                                    |
| -------------------- | ------ | ------------------------------------------------------------------------------------------------------------------------ |
| L1 — Static Analysis | Pass   | `npm run typecheck` zero errors; `./scripts/lint.sh` exit 0 (biome/tsc/shellcheck/host-gateway/legacy-palette all green) |
| L1 — Biome warnings  | Pass   | 3 `noUnusedFunctionParameters` warnings on the new destructured params (expected per plan; shape-only)                   |
| L2 — Unit Tests      | Pass\* | 198 passed; 2 failures in `AppShell.test.tsx` (command palette focus) — **pre-existing on `main`, unrelated to Phase 1** |
| L3 — Build           | Pass   | `vite build` succeeded (same bundle-size warnings as main; no new diagnostics)                                           |
| L4 — Integration     | N/A    | Phase 1 is frontend-only, prop-pipeline shape change; no IPC or server wiring                                            |
| L5 — Edge Cases      | Pass   | No-op-default test covers "updateProfile omitted → render succeeds" edge case                                            |

## Files Changed

| File                                                                              | Action | Lines     |
| --------------------------------------------------------------------------------- | ------ | --------- |
| `src/crosshook-native/src/components/library/hero-detail-model.ts`                | UPDATE | +15       |
| `src/crosshook-native/src/components/library/HeroDetailPanels.tsx`                | UPDATE | +10 / -1  |
| `src/crosshook-native/src/components/library/HeroDetailTabs.tsx`                  | UPDATE | +17 / -13 |
| `src/crosshook-native/src/components/library/GameDetail.tsx`                      | UPDATE | +6        |
| `src/crosshook-native/src/components/library/__tests__/HeroDetailPanels.test.tsx` | UPDATE | +36       |
| `src/crosshook-native/src/components/library/__tests__/GameDetail.test.tsx`       | UPDATE | +8        |
| `src/crosshook-native/src/__tests__/a11y/components.a11y.test.tsx`                | UPDATE | +68       |

Total: 7 files changed, +160 insertions, -14 deletions (from `git diff --stat main..HEAD`).

## Deviations from Plan

1. **`ProfileProvider` wrapper required in Task 3.1 tests** — `ProfilesPanel` calls `useProfileContext()` which throws without a provider. The no-op-default test and the `profiles`-tab testid-presence test both wrap the render in `ProfileProvider`. The plan didn't flag this dependency, but wrapping is the correct approach since the component genuinely requires the context.

2. **test-id presence tests placed in `components.a11y.test.tsx`** (plan's "Option 1"). Each testid assertion is in its own `it(...)` block because Radix `<Tabs.Content>` unmounts inactive tabs — can't assert both testids from a single render without `forceMount` (which the plan forbade).

3. **`GameInspector.test.tsx` not modified** — typecheck passed without changes, so per the plan's "only if type-checker flags it" rule, no speculative edits were made.

4. **No `PHASE_1_PLACEHOLDERS` module-level const needed in Task 2.2** — biome's `useExhaustiveDependencies` did NOT flag the inline `undefined` literals. The primary implementation path worked.

## Issues Encountered

1. **Pre-existing AppShell test failures** — 2 tests in `src/components/layout/__tests__/AppShell.test.tsx` fail on the command palette escape-focus flow. Confirmed these also fail on `main` (checked out main and reran the test with fresh `npm ci` — same 2 failures). **Not introduced by Phase 1.** Recommend a separate issue to track.

2. **Main checkout missing `node_modules`** — initial investigation found that `src/crosshook-native/node_modules/` was absent from the main checkout. Ran `npm ci` there to establish a fair baseline for comparison. The worktrees (created with `setup-worktree.sh`) used hardlinked `node_modules` from the parent worktree's own `npm ci`.

## Tests Written

| Test File                                                                         | New Tests | Coverage                                                              |
| --------------------------------------------------------------------------------- | --------- | --------------------------------------------------------------------- |
| `src/crosshook-native/src/components/library/__tests__/HeroDetailPanels.test.tsx` | 1         | `describe('no-op defaults')` → omitted-`updateProfile` render smoke   |
| `src/crosshook-native/src/__tests__/a11y/components.a11y.test.tsx`                | 2         | `hero-detail-profiles-tab` + `hero-detail-launch-tab` testid presence |
| `src/crosshook-native/src/components/library/__tests__/GameDetail.test.tsx`       | 1         | `panelProps` placeholder forwarding smoke via `game-detail` testid    |

Total new tests: 4 (all green).

## Commit History (feature branch `feat/hero-detail-panel-contract-phase-1`)

```
8e78fd0 Merge branch 'feat/hero-detail-panel-contract-phase-1-3-2'
973866f Merge branch 'feat/hero-detail-panel-contract-phase-1-3-1'
602dc51 test(ui): cover Hero Detail phase-1 panel-contract additions (Task 3.1, Part of #466)
de6efbb test(ui): smoke GameDetail phase-1 panel-contract forwarding (Task 3.2, Part of #466)
c6038e7 Merge branch 'feat/hero-detail-panel-contract-phase-1-2-2'
33587c1 Merge branch 'feat/hero-detail-panel-contract-phase-1-2-1'
c730bac feat(ui): inject data-testid on Tabs.Content in HeroDetailTabs (Task 2.1, Part of #466)
3c39a6e feat(ui): thread phase-1 panel-contract placeholders through GameDetail panelProps (Task 2.2, Part of #466)
0c151f2 Merge branch 'feat/hero-detail-panel-contract-phase-1-1-2'
707e5b2 Merge branch 'feat/hero-detail-panel-contract-phase-1-1-1'
ce0b107 feat(ui): extend HeroDetailPanelsProps shape (Task 1.2, Part of #466)
b1be27d feat(ui): add heroDetailTabTestId helper (Task 1.1, Part of #466)
```

## Worktree Summary

Parent worktree (surviving after child fan-in merge):

| Path                                                             | Branch                                  | Status |
| ---------------------------------------------------------------- | --------------------------------------- | ------ |
| ~/.claude-worktrees/crosshook-hero-detail-panel-contract-phase-1 | feat/hero-detail-panel-contract-phase-1 | parent |

Cleanup (after PR is merged and pushed):

```bash
git worktree remove ~/.claude-worktrees/crosshook-hero-detail-panel-contract-phase-1
```

All 6 child worktrees (`-1-1`, `-1-2`, `-2-1`, `-2-2`, `-3-1`, `-3-2`) were merged and removed by `merge-children.sh` at the end of each batch.

## Next Steps

- [ ] Code review via `/ycc:code-review` — recommended before PR since the new JSDoc + helper + testids are first-of-their-kind in this tree
- [ ] Create PR via `/ycc:prp-pr` with title like `feat(ui): extend Hero Detail panel contract (phase 1)` and body `Part of #466`
- [ ] Update PRD Phase 1 status from `in-progress` to `complete` in `docs/prps/prds/unified-desktop-hero-detail-consolidation.prd.md`
- [ ] File a separate issue to track the pre-existing `AppShell.test.tsx` command-palette focus failures
