# PR Review #489 — feat(ui): hero detail launch/profile parity before route removal

**Reviewed**: 2026-06-04T16:07:10-04:00
**Mode**: PR
**Author**: yandy-r
**Branch**: feat/issue-486-hero-detail-launch-profile-parity → main
**Decision**: REQUEST CHANGES

## Summary

The parity work is well covered by tests and the local validation gates pass, but the new Hero Detail launch/profile surfaces still have mismatched-profile paths that can mutate or launch the wrong selected profile. Those correctness issues should be fixed before merge.

## Findings

### CRITICAL

### HIGH

- **[F001]** `src/crosshook-native/src/components/library/launch/HeroLaunchGate.tsx:134` — After selecting the displayed profile, the same click continues through `depGate.handleBeforeLaunch` and `launchGame`/`launchTrainer` from the pre-selection render, so the dependency gate and launch request can still target the previously selected profile.
  - **Status**: Fixed
  - **Category**: Correctness
  - **Suggested fix**: Treat profile selection as its own state transition: after `await selectProfile(...)`, abort the current launch and require the refreshed render before running the dependency gate and launch callbacks, or refactor launch to accept an explicit displayed-profile request.

- **[F002]** `src/crosshook-native/src/components/library/launch/HeroLaunchSubTabsHost.tsx:94` — `profileMismatch` renders an overlay and `aria-disabled`, but `LaunchSubTabs` remains mounted with live mutating handlers; `aria-disabled` does not block editing, so launch settings can still persist to the currently selected profile while the UI is displaying another profile.
  - **Status**: Fixed
  - **Category**: Correctness
  - **Suggested fix**: When `profileMismatch` is true, either do not mount `LaunchSubTabs` or provide a real read-only/disabled mode that gates every mutating handler until the displayed profile matches `ProfileContext`.

- **[F003]** `src/crosshook-native/src/components/library/launch/HeroLaunchCommandSection.tsx:147` — `.desktop` export is built from `useProfileContext().profile` while using `resolvedProfileName` from the displayed detail view, so mismatch/fallback views can export the previously selected profile under the displayed profile name.
  - **Status**: Fixed
  - **Category**: Correctness
  - **Suggested fix**: Disable the export action when the displayed profile does not match the context-selected profile, or build the export request from the same displayed-profile data used by the command block.

- **[F004]** `src/crosshook-native/src/components/library/profiles/HeroProfileActionsBar.tsx:217` — The delete button and handler ignore the shared `canDelete` guard, leaving delete available during states where legacy/profile action logic intentionally disables it, such as saving, loading, duplicating, renaming, or when the selected profile no longer exists.
  - **Status**: Fixed
  - **Category**: Correctness
  - **Suggested fix**: Include `canDelete` in `HeroProfileActionsBar`'s destructured action guards, disable the delete button with `!canDelete || deleting`, and have `handleDelete` return early unless `canDelete` is true.

### MEDIUM

- **[F005]** `src/crosshook-native/src/hooks/profile/useProfileActions.ts:21` — The reusable `useProfileActions` hook imports page-owned notification logic from `components/pages/profiles`, keeping shared profile action logic coupled to the legacy route layer.
  - **Status**: Fixed
  - **Category**: Pattern Compliance
  - **Suggested fix**: Move `useProfilesPageNotifications` and its constants into a shared profile module, then import that module from both `useProfileActions` and the legacy profiles page.

- **[F006]** `src/crosshook-native/src/components/library/profiles/HeroProfileEditorSections.tsx:49` — `resolveTrainerGamescopeForDisplay` is copied from `ProfileSubTabs`, creating a second implementation of logic that must stay aligned with the legacy tab and Rust launch resolution.
  - **Status**: Fixed
  - **Category**: Maintainability
  - **Suggested fix**: Extract the resolver to a shared utility or hook used by both `ProfileSubTabs` and `HeroProfileEditorSections`, with focused tests for the shared behavior.

- **[F007]** `src/crosshook-native/src/components/library/profiles/HeroProfileActionsBar.tsx:242` — Delete, rename, toast, preview, and history overlay markup is duplicated from the legacy profiles overlays, so route-removal work now has two modal implementations with separate IDs, inline styling, and close semantics to keep synchronized.
  - **Status**: Fixed
  - **Category**: Maintainability
  - **Suggested fix**: Extract a reusable profile overlay component or generalize `ProfilesOverlays` so both the legacy profiles route and Hero Detail actions bar render the same overlay implementation.

### LOW

- **[F008]** `src/crosshook-native/src/components/library/launch/HeroLaunchGate.tsx:110` — Diagnostic-copy feedback schedules untracked `setTimeout` calls on every click and never clears them on repeated clicks or unmount, allowing stale timers to update state after navigation.
  - **Status**: Fixed
  - **Category**: Performance
  - **Suggested fix**: Store the diagnostic-copy timeout in a ref, clear it before scheduling a new timeout, and clear it in an unmount cleanup effect.

## Validation Results

| Check      | Result                                                                                                                                                  |
| ---------- | ------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Type check | Pass (`npm run typecheck` in `src/crosshook-native`; root `npm run typecheck` is not configured)                                                        |
| Lint       | Pass (`npm run lint`; includes rustfmt, clippy, Biome, TypeScript, shellcheck, host-gateway, legacy-palette; Biome reported pre-existing warnings only) |
| Tests      | Pass (`npm test` in `src/crosshook-native`: 49 files, 358 tests)                                                                                        |
| Build      | Pass (`npm run build` in `src/crosshook-native`; Vite chunk-size warning only)                                                                          |

## Files Reviewed

- `docs/prps/plans/completed/github-issue-486-hero-detail-launch-profile-parity.plan.md` (Modified)
- `docs/prps/reports/github-issue-486-hero-detail-launch-profile-parity-report.md` (Added)
- `src/crosshook-native/src/__tests__/a11y/components.a11y.test.tsx` (Modified)
- `src/crosshook-native/src/components/library/HeroDetailLaunchTab.tsx` (Modified)
- `src/crosshook-native/src/components/library/HeroDetailProfilesTab.tsx` (Modified)
- `src/crosshook-native/src/components/library/__tests__/HeroDetailLaunchTab.test.tsx` (Modified)
- `src/crosshook-native/src/components/library/__tests__/HeroDetailPanels.test.tsx` (Modified)
- `src/crosshook-native/src/components/library/__tests__/HeroDetailProfilesTab.test.tsx` (Modified)
- `src/crosshook-native/src/components/library/__tests__/HeroLaunchCommandSection.test.tsx` (Added)
- `src/crosshook-native/src/components/library/__tests__/HeroLaunchGate.test.tsx` (Added)
- `src/crosshook-native/src/components/library/__tests__/HeroLaunchSubTabsHost.test.tsx` (Added)
- `src/crosshook-native/src/components/library/__tests__/HeroProfileActionsBar.test.tsx` (Added)
- `src/crosshook-native/src/components/library/__tests__/HeroProfileEditorSections.test.tsx` (Added)
- `src/crosshook-native/src/components/library/launch/HeroLaunchCommandSection.tsx` (Added)
- `src/crosshook-native/src/components/library/launch/HeroLaunchGate.tsx` (Added)
- `src/crosshook-native/src/components/library/launch/HeroLaunchSubTabsHost.tsx` (Added)
- `src/crosshook-native/src/components/library/profiles/HeroProfileActionsBar.tsx` (Added)
- `src/crosshook-native/src/components/library/profiles/HeroProfileCardList.tsx` (Added)
- `src/crosshook-native/src/components/library/profiles/HeroProfileEditorExtras.tsx` (Added)
- `src/crosshook-native/src/components/library/profiles/HeroProfileEditorSections.tsx` (Added)
- `src/crosshook-native/src/components/library/profiles/useHeroProfilesAutosave.ts` (Added)
- `src/crosshook-native/src/components/pages/LaunchPage.tsx` (Modified)
- `src/crosshook-native/src/components/pages/profiles/useProfilesPageState.ts` (Modified)
- `src/crosshook-native/src/hooks/launch/useLaunchSubTabsProps.ts` (Added)
- `src/crosshook-native/src/hooks/profile/useProfileActions.ts` (Added)
- `src/crosshook-native/src/styles/hero-detail.css` (Modified)
- `src/crosshook-native/src/test/fixtures.ts` (Modified)
- `src/crosshook-native/tests/smoke.spec.ts` (Modified)
