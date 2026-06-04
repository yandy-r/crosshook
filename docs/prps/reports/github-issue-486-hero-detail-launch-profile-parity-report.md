# Implementation Report: Hero Detail Launch/Profile Parity Before Route Removal (Phase 5b)

**GitHub issue**: [#486](https://github.com/yandy-r/crosshook/issues/486) · Part of #478 (Hero Detail Consolidation tracker)
**Branch**: `feat/issue-486-hero-detail-launch-profile-parity`
**Execution mode**: Parallel sub-agents (`/ycc:prp-implement --parallel --no-worktree`) — 5 batches, 12 tasks, max width 4

## Summary

Brought the Hero Detail `launch-options` and `profiles` tabs to functional parity with the legacy `/launch` and `/profiles` routes, gating Phases 8/9/10 (#473/#474/#475). Delivered:

- **Shared extraction layer** (composition-only; legacy routes unchanged): `useLaunchSubTabsProps` (LaunchSubTabs 40-prop assembly, consumed by both `LaunchPage` and Hero Detail) and `useProfileActions` (duplicate/rename/delete/preview/export/history/mark-verified, consumed by both `useProfilesPageState` and Hero Detail).
- **Hero Launch tab**: embedded legacy `LaunchSubTabs` (Environment/Gamescope/MangoHud/Optimizations/Steam Options/Offline + ProtonDB lookup/overwrite/suggestions + merged autosave chip + offline auto-switch) via `HeroLaunchSubTabsHost`; in-place launch via `HeroLaunchGate` (LaunchStateContext `launchGame`/`launchTrainer`, `useLaunchDepGate` + `LaunchDepGateModal`, selectProfile-first gating, `LaunchPanelFeedback`, `LaunchPipeline`, helper log path, guidance text, trainer launch affordance).
- **Hero Profiles tab**: full editor parity (`RunnerMethodSection`, `TrainerSection` + version set, trainer-gamescope `GamescopeConfigPanel` with derived-from-game notice, `GameMetadataBar`, `PrefixDepsPanel` in `CollapsibleSection`, ProtonUp runtime suggestion banner, health issues list + badge scroll + stale note + trainer/version/isolation chips) plus `HeroProfileActionsBar` (all 7 lifecycle actions with busy/error states, rename modal + undo toast + F2, collection-aware delete confirm, TOML preview, config history/rollback) and the full `LauncherExport` panel with `pendingReExport`.
- **Both parity inventories** finalized in the plan against the shipped UI: all 41 **Port** rows flipped to **Ported**; no reclassification needed.
- **New Playwright responsive coverage**: 10 no-horizontal-overflow assertions (5 viewports × 2 tabs) — all green, no overflow bugs found.

## Assessment vs Reality

| Metric        | Predicted (Plan) | Actual                       |
| ------------- | ---------------- | ---------------------------- |
| Complexity    | High             | High                         |
| Confidence    | 8/10             | 9/10 — no blocking surprises |
| Files Changed | ~30              | 27 (15 created, 12 modified) |

## Tasks Completed

| #   | Task                                                 | Status | Notes                                                                                       |
| --- | ---------------------------------------------------- | ------ | ------------------------------------------------------------------------------------------- |
| 1.1 | Extract `useLaunchSubTabsProps` bridge hook          | Done   | LaunchPage 225→117 lines; behavior-neutral                                                  |
| 1.2 | Extract `useProfileActions` shared hook              | Done   | Deviation: rename toast/undo/F2 delegated to `useProfilesPageNotifications` (see below)     |
| 1.3 | Profiles editor section renderer + tab decomposition | Done   | HeroDetailProfilesTab 289→135 lines (pre-2.2); test file unchanged                          |
| 1.4 | CSS groundwork + scroll registration                 | Done   | Subtabs-shell flattening, actions-bar wrap, min-width:0, 720px rules; no new scrollers      |
| 2.1 | Hero Launch tab: embed LaunchSubTabs + ProtonDB      | Done   | Env section deduplicated into LaunchSubTabs; profileMismatch overlay (disabled-not-removed) |
| 2.2 | Hero Profiles tab: full editor parity sections       | Done   | Trainer-gamescope routed through `updateProfile` draft (single full-draft writer)           |
| 3.1 | In-place launch + dependency gate + feedback         | Done   | `HeroLaunchGate`; selectProfile-first mirrors `LibraryPage.tsx`; legacy `onLaunch` ignored  |
| 3.2 | Profile lifecycle actions UI + full LauncherExport   | Done   | Rename-pause via name-divergence guard in `useHeroProfilesAutosave`                         |
| 4.1 | GameDetail/HeroDetailPanels bridge + a11y harness    | Done   | No panelProps changes needed (verified); axe coverage added to both hero tab cases          |
| 4.2 | Launch parity focused tests + shared fixtures        | Done   | +56 tests; `makeLaunchRequest`/`makeLaunchPreview` canonicalized in `@/test/fixtures`       |
| 4.3 | Profiles parity focused tests                        | Done   | +61 tests across 3 files                                                                    |
| 5.1 | Responsive no-horizontal-overflow Playwright checks  | Done   | +10 smoke tests; 85/85 smoke suite green; zero overflow bugs                                |
| 5.2 | Finalize parity inventories + full validation        | Done   | All 41 Port rows → Ported; all 8 ACs checked                                                |

## Validation Results

| Level               | Status | Notes                                                                                                                                                                                      |
| ------------------- | ------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| Static Analysis     | Pass   | `npm run typecheck` zero errors; `./scripts/lint.sh --modified` exit 0; Biome warnings at pre-existing baseline (5, all in untouched files)                                                |
| Unit Tests          | Pass   | 358/358 (44→49 files; +110 tests vs main's 248 baseline)                                                                                                                                   |
| Build               | Pass   | `tsc && vite build` succeed (pre-existing chunk-size warning only)                                                                                                                         |
| Integration (smoke) | Pass   | `npm run test:smoke` 85/85 incl. 10 new overflow assertions                                                                                                                                |
| Edge Cases          | Pass   | Disabled-not-removed states, sparse-profile guards, selectProfile-first abort, rename-pause autosave, flush-before-switch ordering, dep-gate confirm/cancel — all covered by focused tests |
| Dependency guard    | Pass   | `git diff main -- package*.json` empty — zero new dependencies                                                                                                                             |
| Legacy routes       | Pass   | `LaunchRoute.test.tsx` (6) + `ProfilesRoute.test.tsx` (5) + `LaunchSubTabs.test.tsx` (8) unchanged and green                                                                               |

## Files Changed (27 — 15 created, 12 modified)

| File                                                                  | Action  | Notes                                                                  |
| --------------------------------------------------------------------- | ------- | ---------------------------------------------------------------------- |
| `src/hooks/launch/useLaunchSubTabsProps.ts`                           | CREATED | +223                                                                   |
| `src/hooks/profile/useProfileActions.ts`                              | CREATED | +304                                                                   |
| `src/components/library/launch/HeroLaunchGate.tsx`                    | CREATED | +220                                                                   |
| `src/components/library/launch/HeroLaunchCommandSection.tsx`          | CREATED | +309                                                                   |
| `src/components/library/launch/HeroLaunchSubTabsHost.tsx`             | CREATED | +99                                                                    |
| `src/components/library/profiles/HeroProfileEditorSections.tsx`       | CREATED | +277                                                                   |
| `src/components/library/profiles/HeroProfileEditorExtras.tsx`         | CREATED | +286                                                                   |
| `src/components/library/profiles/HeroProfileActionsBar.tsx`           | CREATED | +397                                                                   |
| `src/components/library/profiles/HeroProfileCardList.tsx`             | CREATED | +112                                                                   |
| `src/components/library/profiles/useHeroProfilesAutosave.ts`          | CREATED | +134                                                                   |
| `src/components/library/__tests__/HeroLaunchGate.test.tsx`            | CREATED | +566 (20 tests)                                                        |
| `src/components/library/__tests__/HeroLaunchCommandSection.test.tsx`  | CREATED | +359 (26 tests)                                                        |
| `src/components/library/__tests__/HeroLaunchSubTabsHost.test.tsx`     | CREATED | +194 (10 tests)                                                        |
| `src/components/library/__tests__/HeroProfileActionsBar.test.tsx`     | CREATED | +542 (32 tests)                                                        |
| `src/components/library/__tests__/HeroProfileEditorSections.test.tsx` | CREATED | +531 (22 tests)                                                        |
| `src/components/library/HeroDetailLaunchTab.tsx`                      | UPDATED | 294→106 (thin shell)                                                   |
| `src/components/library/HeroDetailProfilesTab.tsx`                    | UPDATED | 289→~220 (shell + actions wiring)                                      |
| `src/components/pages/LaunchPage.tsx`                                 | UPDATED | 225→117 (consumes bridge hook)                                         |
| `src/components/pages/profiles/useProfilesPageState.ts`               | UPDATED | 326→171 (consumes shared hook)                                         |
| `src/styles/hero-detail.css`                                          | UPDATED | +76 (shell flattening, wrap, min-width:0, 720px)                       |
| `src/test/fixtures.ts`                                                | UPDATED | +95 (`makeLaunchRequest`, `makeLaunchPreview`)                         |
| `tests/smoke.spec.ts`                                                 | UPDATED | +130 (10 overflow tests)                                               |
| `src/components/library/__tests__/HeroDetailLaunchTab.test.tsx`       | UPDATED | gate-mock shell tests + shared fixtures                                |
| `src/components/library/__tests__/HeroDetailProfilesTab.test.tsx`     | UPDATED | +7 tests, new section stubs                                            |
| `src/components/library/__tests__/HeroDetailPanels.test.tsx`          | UPDATED | provider stack + shared fixtures                                       |
| `src/__tests__/a11y/components.a11y.test.tsx`                         | UPDATED | `HeroDetailTabsProviders` full stack + axe coverage for both hero tabs |
| `docs/prps/plans/...486....plan.md`                                   | UPDATED | Inventories finalized, ACs checked                                     |

## Deviations from Plan

1. **Task 1.2 — rename notifications not fully extracted**: The rename confirm → toast → undo flow in `useProfilesPageNotifications` shares internal timer/state refs; splitting it would have duplicated state management. `useProfileActions` delegates to and re-exports that hook instead — same composition goal, no invariant breakage.
2. **Task 2.2 — trainer-gamescope persistence**: Legacy uses a granular `profile_save_trainer_gamescope_config` autosave; in Hero Detail the panel's changes route through `updateProfile` into the 350ms full-draft autosave instead, preserving the single-full-draft-writer rule (the plan's autosave-race mitigation).
3. **Task 3.1 — legacy `onLaunch` prop retained but ignored**: `HeroLaunchGate` owns in-place launch; the `onLaunch` (navigate-to-`/launch`) plumbing from `GameDetail` is left in place untouched for Phases 9/10 to remove with the nav rewire.
4. **Task 4.1 — HeroDetailPanels profiles-tab branch kept rendering the real tab**: mocking it would have required rewriting passing assertions; lower-churn option chosen (providers added instead), consistent with "child tabs stay mocked" for the launch branch.
5. **Orchestrator fixture dedup**: `HeroDetailPanels.test.tsx`'s local builders were converted to the shared fixtures by the orchestrator after Batch 4 (file was 4.1-owned during the batch; 4.2 owned the fixtures module) — avoids same-file parallel edits.

## Issues Encountered

- **Anticipated a11y/panels harness failures after Batch 2**: `usePreferencesContext`/`useProfileHealthContext` missing-provider errors in `components.a11y.test.tsx` and `HeroDetailPanels.test.tsx` — exactly the plan's risk-table prediction. Fixed between batches (provider stack additions) rather than left to accumulate; Task 4.1 then extended the harness with axe coverage.
- **Intermittent cold-start flake in `ProfilesRoute.test.tsx`** ("Save" button `waitFor` timeout): appears only on the first full-suite run under heavy transform load; passes consistently in isolation and on every re-run. Pre-dates Batch 2; worth watching but not introduced as a deterministic failure by this work.
- **Lefthook lint warnings after Batches 1/2** (unused destructured bindings, unused test imports, a trigger-only hook dep): fixed in follow-up commits; final Biome state matches the pre-existing baseline exactly.

## Tests Written

| Test File                                   | Tests | Coverage                                                                                                                                      |
| ------------------------------------------- | ----- | --------------------------------------------------------------------------------------------------------------------------------------------- |
| `HeroLaunchGate.test.tsx`                   | 20    | selectProfile-first gate, dep-gate modal, feedback/pipeline render, launch flow, hint/disabled states                                         |
| `HeroLaunchCommandSection.test.tsx`         | 26    | dry-run/copy/legacy/in-place launch buttons, onBeforeLaunch abort, trainer flow                                                               |
| `HeroLaunchSubTabsHost.test.tsx`            | 10    | hook input forwarding, profileMismatch overlay (disabled-not-removed)                                                                         |
| `HeroProfileActionsBar.test.tsx`            | 32    | all 7 lifecycle actions, busy labels, `role="alert"` errors, rename modal/toast/undo                                                          |
| `HeroProfileEditorSections.test.tsx`        | 22    | runner method, trainer + version set, trainer-gamescope notice + draft routing, prefix deps, suggestion banner, health list/scroll/stale note |
| `HeroDetailProfilesTab.test.tsx` (extended) | +7    | runner-method autosave, LauncherExport slot, flush-before-switch                                                                              |
| `components.a11y.test.tsx` (extended)       | 2→axe | both hero tabs now axe-asserted under the full provider stack                                                                                 |
| `tests/smoke.spec.ts` (extended)            | +10   | no-horizontal-overflow, 5 viewports × launch/profiles tabs                                                                                    |

Full suite: 248 → 358 tests (44 → 49 files).

## Next Steps

- [ ] Code review via `/ycc:code-review`
- [ ] Create PR via `/ycc:prp-pr` — title `feat(ui): hero detail launch/profile parity before route removal`, body `Part of #478`, `Closes #486`
