# Implementation Report: GitHub Issues 421 and 448 Route Rework Dashboards

## Summary

Implemented the Phase 9 dashboard-route rework for `Health`, `Host Tools`, `Proton Manager`, and `Compatibility`, plus the supporting shared section chrome and focused automated coverage. The work stayed frontend-only, preserved existing IPC and route behavior, and aligned the four routes to a common banner/panel/section language.

## Assessment vs Reality

| Metric        | Predicted (Plan) | Actual                                        |
| ------------- | ---------------- | --------------------------------------------- |
| Complexity    | Large            | Large                                         |
| Confidence    | High             | Medium-High                                   |
| Files Changed | 15               | 16 implementation files, plus PRD/report docs |

## Tasks Completed

| #   | Task                                                        | Status          | Notes                                                                                            |
| --- | ----------------------------------------------------------- | --------------- | ------------------------------------------------------------------------------------------------ |
| 1.1 | Add shared dashboard route chrome primitives                | [done] Complete | Added `DashboardPanelSection` and shared dashboard route stylesheet.                             |
| 1.2 | Re-skin Host Tools on the new dashboard chrome              | [done] Complete | Replaced the temporary local section shim with the shared primitive before batch closeout.       |
| 1.3 | Split Proton Manager into route-friendly section components | [done] Complete | Installed and available lists were extracted while keeping async state in the parent panel.      |
| 2.1 | Recompose Health into unified dashboard sections            | [done] Complete | Preserved the existing table, retry/recheck flows, and page-level scroll ownership.              |
| 2.2 | Rework Proton Manager route shell around the split sections | [done] Complete | Added dashboard hero/section framing without changing Steam path resolution order.               |
| 2.3 | Align Compatibility with the dashboard route language       | [done] Complete | Restored the legacy subtabs shell contract after review to preserve bounded tab layout behavior. |
| 3.1 | Add focused dashboard route RTL coverage                    | [done] Complete | Added a 4-test provider-backed dashboard route suite.                                            |
| 3.2 | Expand browser smoke to cover all Phase 9 dashboard routes  | [done] Complete | Added `host-tools` and `proton-manager` to the route sweep and dashboard-specific assertions.    |

## Validation Results

| Level           | Status         | Notes                                                                                                                                                                                                                                                                           |
| --------------- | -------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Static Analysis | [done] Pass    | `npm --prefix src/crosshook-native run typecheck` passed after implementation and after review fixes.                                                                                                                                                                           |
| Unit Tests      | [done] Pass    | `npm --prefix src/crosshook-native test` passed: 26 files, 126 tests.                                                                                                                                                                                                           |
| Build           | [done] Pass    | `npm --prefix src/crosshook-native run build` passed.                                                                                                                                                                                                                           |
| Integration     | [warn] Partial | `npm --prefix src/crosshook-native run test:smoke` still fails on pre-existing browser-dev `LibraryPage` console-error loops affecting library and one pipeline test; dashboard-only smoke subset for `health`, `host-tools`, `proton-manager`, and `compatibility` passed 4/4. |
| Edge Cases      | [warn] Partial | Automated fallback-state coverage added; manual dashboard inspection and interactive checklist were not run in this session.                                                                                                                                                    |

## Files Changed

| File                                                                              | Action  | Lines       |
| --------------------------------------------------------------------------------- | ------- | ----------- |
| `src/crosshook-native/src/components/layout/DashboardPanelSection.tsx`            | CREATED | 90          |
| `src/crosshook-native/src/styles/dashboard-routes.css`                            | CREATED | 141         |
| `src/crosshook-native/src/main.tsx`                                               | UPDATED | +1 / -0     |
| `src/crosshook-native/src/components/pages/HostToolsPage.tsx`                     | UPDATED | +114 / -88  |
| `src/crosshook-native/src/styles/host-tool-dashboard.css`                         | UPDATED | +114 / -6   |
| `src/crosshook-native/src/components/pages/HealthDashboardPage.tsx`               | UPDATED | +337 / -282 |
| `src/crosshook-native/src/components/proton-manager/InstalledVersionsSection.tsx` | CREATED | 45          |
| `src/crosshook-native/src/components/proton-manager/AvailableVersionsSection.tsx` | CREATED | 73          |
| `src/crosshook-native/src/components/proton-manager/ProtonManagerPanel.tsx`       | UPDATED | +26 / -69   |
| `src/crosshook-native/src/components/pages/ProtonManagerPage.tsx`                 | UPDATED | +67 / -4    |
| `src/crosshook-native/src/styles/proton-manager.css`                              | UPDATED | +144 / -30  |
| `src/crosshook-native/src/components/compatibility/ProtonVersionsPanel.tsx`       | CREATED | 224         |
| `src/crosshook-native/src/components/pages/CompatibilityPage.tsx`                 | UPDATED | +75 / -234  |
| `src/crosshook-native/src/components/CompatibilityViewer.tsx`                     | UPDATED | +15 / -6    |
| `src/crosshook-native/src/components/pages/__tests__/DashboardRoutes.test.tsx`    | CREATED | 191         |
| `src/crosshook-native/tests/smoke.spec.ts`                                        | UPDATED | +17 / -2    |

## Deviations from Plan

- Added a lightweight `vi.mock('@/lib/ipc', ...)` bridge in `DashboardRoutes.test.tsx` so `renderWithMocks` handler overrides drive the route components consistently in Vitest.
- Full Playwright smoke remains blocked by a pre-existing browser-dev `LibraryPage` maximum-update-depth console error outside the files changed by this plan. Dashboard-only smoke was run separately to validate the new route work.

## Issues Encountered

- `./scripts/lint.sh` initially failed on import-order and formatting issues in newly edited files. Those were fixed with targeted Biome writes and the lint pass now succeeds with only pre-existing warnings in `LibraryToolbar.tsx`.
- Code review identified a compatibility-shell regression risk and a Proton “Available” empty-state gap. Both were fixed before final validation.
- Playwright generated `test-results/` artifacts while reproducing the unrelated smoke failures. They are left untracked in the working tree.

## Tests Written

| Test File                                                                      | Tests                  | Coverage                                                                                                    |
| ------------------------------------------------------------------------------ | ---------------------- | ----------------------------------------------------------------------------------------------------------- |
| `src/crosshook-native/src/components/pages/__tests__/DashboardRoutes.test.tsx` | 4 tests                | Route banners, primary dashboard headings, shell classes, and fallback states for the four dashboard routes |
| `src/crosshook-native/tests/smoke.spec.ts`                                     | Updated existing sweep | Added `host-tools` and `proton-manager` route smoke plus dashboard-heading/body assertions                  |

## Next Steps

- [x] Code review via `code-reviewer`
- [ ] Investigate the pre-existing `LibraryPage` browser-dev `Maximum update depth exceeded` console error blocking full Playwright smoke
- [ ] Create PR via `$prp-pr`
