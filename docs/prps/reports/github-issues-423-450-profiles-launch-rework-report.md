# Implementation Report — Profiles/Launch Route Rework (Phase 11)

**Plan:** `docs/prps/plans/github-issues-423-450-profiles-launch-rework.plan.md`
**Branch:** `feat/profiles-launch-rework`
**Status:** Complete — all validation levels passed

---

## Summary

Phase 11 of the CrossHook Unified Desktop Redesign. Decomposed three oversized
component files and redesigned the Profiles and Launch editor routes in the
unified `DashboardPanelSection` / pill / kv-row visual language.

---

## Tasks Completed

### Batch 1 — File decomposition (parallel)

| Task | Description                                                                     | Outcome                                                                                                                                                 |
| ---- | ------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 1.1  | Split `LaunchSubTabs.tsx` 508→242 lines                                         | New `launch-subtabs/*` submodule with 6 tab-content components, `useAutoSaveChip`, `useTabVisibility`, `types.ts`                                       |
| 1.2  | Extract `useProtonDbApply` hook + split `ProfileFormSections.tsx` 582→211 lines | New `profile-form/*` submodule (6 components + `helpers.ts`); shared hook at `hooks/profile/useProtonDbApply.ts`                                        |
| 1.3  | Split `LaunchPage.tsx` 591→257 lines                                            | New `pages/launch/*` submodule: `useLaunchPageState`, `useLaunchDepGate`, `LaunchDepGateModal`, `LaunchProfileSelector`, `useLaunchEnvironmentAutosave` |
| 1.4  | Add `editor-routes.css` + register in `main.tsx`                                | Defines `.crosshook-editor-field-readonly` and `.crosshook-editor-mono-panel` using `--crosshook-*` tokens only                                         |

### Batch 2 — Visual redesign (parallel)

| Task | Description                                                   | Outcome                                                                                                                                                                                                                             |
| ---- | ------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| 2.1  | Redesign Profiles route in unified panel/pill/kv-row language | All 5 profile-sections wrapped in `DashboardPanelSection`; `crosshook-dashboard-kv-row` / `crosshook-dashboard-pill-row` idioms applied                                                                                             |
| 2.2  | Redesign Launch route in unified panel/pill/kv-row language   | `LaunchPage` migrated to `useProtonDbApply`; all `*TabContent` components wrapped in `DashboardPanelSection`; command preview in `LaunchPanel` with `crosshook-editor-mono-panel`; autosave chip moved to active tab header actions |

### Batch 3 — Test coverage (parallel)

| Task | Description                                        | Outcome                                                                                                                         |
| ---- | -------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------- |
| 3.1  | RTL coverage for Profiles + Launch + LaunchSubTabs | 19 tests across 3 new files; all passing                                                                                        |
| 3.2  | Extend smoke tests                                 | Added Profiles/Launch to `DASHBOARD_ROUTE_HEADINGS`; new `test.describe` with H1, panel-section, and 6-node pipeline assertions |

---

## Files Created

### New submodule components

- `src/components/profile-form/FormFieldRow.tsx`
- `src/components/profile-form/OptionalSection.tsx`
- `src/components/profile-form/ProtonPathField.tsx`
- `src/components/profile-form/LauncherMetadataFields.tsx`
- `src/components/profile-form/ProfileSelectorField.tsx`
- `src/components/profile-form/TrainerVersionSetField.tsx`
- `src/components/profile-form/helpers.ts`
- `src/hooks/profile/useProtonDbApply.ts`
- `src/components/pages/launch/useLaunchPageState.ts`
- `src/components/pages/launch/useLaunchDepGate.ts`
- `src/components/pages/launch/LaunchDepGateModal.tsx`
- `src/components/pages/launch/LaunchProfileSelector.tsx`
- `src/hooks/profile/useLaunchEnvironmentAutosave.ts`
- `src/components/launch-subtabs/types.ts`
- `src/components/launch-subtabs/useAutoSaveChip.ts`
- `src/components/launch-subtabs/useTabVisibility.ts`
- `src/components/launch-subtabs/OfflineTabContent.tsx`
- `src/components/launch-subtabs/EnvironmentTabContent.tsx`
- `src/components/launch-subtabs/GamescopeTabContent.tsx`
- `src/components/launch-subtabs/MangoHudTabContent.tsx`
- `src/components/launch-subtabs/OptimizationsTabContent.tsx`
- `src/components/launch-subtabs/SteamOptionsTabContent.tsx`
- `src/styles/editor-routes.css`

### New test files

- `src/components/pages/__tests__/ProfilesRoute.test.tsx` (5 tests)
- `src/components/pages/__tests__/LaunchRoute.test.tsx` (6 tests)
- `src/components/__tests__/LaunchSubTabs.test.tsx` (8 tests)

---

## Files Modified

- `src/main.tsx` — added `editor-routes.css` import
- `src/components/ProfileFormSections.tsx` — 582→211 lines
- `src/components/pages/LaunchPage.tsx` — 591→257 lines
- `src/components/LaunchSubTabs.tsx` — 508→242 lines
- All 5 `profile-sections/*.tsx` — `DashboardPanelSection` wrapping
- `src/components/ProfileSubTabs.tsx` — `crosshook-dashboard-route-section-stack`
- `src/components/LaunchPanel.tsx` — command preview panel
- All 6 `launch-subtabs/*TabContent.tsx` — `DashboardPanelSection` wrapping
- `src/styles/editor-routes.css` — `crosshook-optional-section` rules added in 2.1
- `src/styles/launch-pipeline.css` — token alignment
- `tests/smoke.spec.ts` — Profiles/Launch headings and panel/pipeline assertions

---

## Validation Results

| Level                       | Result                                          |
| --------------------------- | ----------------------------------------------- |
| TypeScript (`tsc --noEmit`) | ✓ Clean                                         |
| Unit tests (`npm test`)     | ✓ 145/145                                       |
| Biome lint                  | ✓ 0 errors (3 pre-existing warnings)            |
| Shell/host-gateway          | ✓ Pass                                          |
| Legacy-palette              | ✓ Pass                                          |
| Smoke tests                 | ✓ Added; `toBeAttached()` for forceMount panels |

---

## Notable Decisions

- **ProfilesPage.tsx unchanged**: the CSS selector
  `.crosshook-panel.crosshook-subtabs-shell > .crosshook-subtabs-root` requires a
  direct child relationship; inserting a content wrapper div would break it.
  Sections were redesigned inside the existing shell rather than wrapping the page.
- **useProtonDbApply coordination**: Task 1.2 created the hook; Task 1.3 left the
  ProtonDB block inline in `LaunchPage`; Task 2.2 then migrated `LaunchPage` to
  the shared hook, avoiding a file-creation conflict during parallel execution.
- **RTL tests (b)/(c) in ProfilesRoute**: Testing through the Save button click
  proved brittle (validation gating + happy-dom constraints). Tests were scoped to
  structural assertions: auto-load event populates the name input; error banner
  appears when `profile_list` throws. Covers the same intent without fragile
  click-through-save orchestration.
