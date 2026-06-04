# Fix Report: pr-489-review

**Source**: docs/prps/reviews/pr-489-review.md
**Applied**: 2026-06-04T16:25:00-04:00
**Mode**: Parallel requested; local execution (4 severity batches, max width 4)
**Severity threshold**: LOW

## Summary

- **Total findings in source**: 8
- **Already processed before this run**:
  - Fixed: 0
  - Failed: 0
- **Eligible this run**: 8
- **Applied this run**:
  - Fixed: 8
  - Failed: 0
- **Skipped this run**:
  - Below severity threshold: 0
  - No suggested fix: 0
  - Missing file: 0

## Fixes Applied

| ID   | Severity | File                                                                               | Line | Status | Notes                                                                                                                                                                  |
| ---- | -------- | ---------------------------------------------------------------------------------- | ---- | ------ | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| F001 | HIGH     | src/crosshook-native/src/components/library/launch/HeroLaunchGate.tsx              | 134  | Fixed  | After selecting a mismatched displayed profile, the launch gate now aborts the current launch so dependency gating and launch callbacks wait for the refreshed render. |
| F002 | HIGH     | src/crosshook-native/src/components/library/launch/HeroLaunchSubTabsHost.tsx       | 94   | Fixed  | `LaunchSubTabs` now unmounts while `profileMismatch` is true, preventing live mutating handlers from writing to the context-selected profile.                          |
| F003 | HIGH     | src/crosshook-native/src/components/library/launch/HeroLaunchCommandSection.tsx    | 147  | Fixed  | Added a `canExportDesktop` guard and disables `.desktop` export when the displayed profile does not match the context-selected profile.                                |
| F004 | HIGH     | src/crosshook-native/src/components/library/profiles/HeroProfileActionsBar.tsx     | 217  | Fixed  | Delete now destructures and honors `canDelete` in both the button disabled state and handler early return.                                                             |
| F005 | MEDIUM   | src/crosshook-native/src/hooks/profile/useProfileActions.ts                        | 21   | Fixed  | Moved profile notification state/constants into shared `hooks/profile` modules and updated shared profile logic to import from there.                                  |
| F006 | MEDIUM   | src/crosshook-native/src/components/library/profiles/HeroProfileEditorSections.tsx | 49   | Fixed  | Extracted `resolveTrainerGamescopeForDisplay` to `utils/trainerGamescope.ts` and reused it from both profile editors.                                                  |
| F007 | MEDIUM   | src/crosshook-native/src/components/library/profiles/HeroProfileActionsBar.tsx     | 242  | Fixed  | Replaced duplicated Hero overlay markup with the shared `ProfilesOverlays` component.                                                                                  |
| F008 | LOW      | src/crosshook-native/src/components/library/launch/HeroLaunchGate.tsx              | 110  | Fixed  | Added a ref-tracked diagnostic copy reset timer, clears prior timers before scheduling, and clears the timer on unmount.                                               |

## Files Changed

- `src/crosshook-native/src/components/library/launch/HeroLaunchGate.tsx` (Fixed F001, F008)
- `src/crosshook-native/src/components/library/launch/HeroLaunchSubTabsHost.tsx` (Fixed F002)
- `src/crosshook-native/src/components/library/launch/HeroLaunchCommandSection.tsx` (Fixed F003)
- `src/crosshook-native/src/components/library/profiles/HeroProfileActionsBar.tsx` (Fixed F004, F007)
- `src/crosshook-native/src/hooks/profile/useProfileActions.ts` (Fixed F005)
- `src/crosshook-native/src/hooks/profile/useProfileNotifications.ts` (Fixed F005)
- `src/crosshook-native/src/hooks/profile/profileNotificationConstants.ts` (Fixed F005)
- `src/crosshook-native/src/components/pages/profiles/useProfilesPageNotifications.ts` (compatibility re-export for F005)
- `src/crosshook-native/src/components/pages/profiles/constants.ts` (compatibility re-export for F005)
- `src/crosshook-native/src/components/library/profiles/HeroProfileEditorSections.tsx` (Fixed F006)
- `src/crosshook-native/src/components/ProfileSubTabs.tsx` (Fixed F006)
- `src/crosshook-native/src/utils/trainerGamescope.ts` (Fixed F006)
- `src/crosshook-native/src/components/library/__tests__/HeroLaunchGate.test.tsx` (coverage for F001 and F008)
- `src/crosshook-native/src/components/library/__tests__/HeroLaunchSubTabsHost.test.tsx` (coverage for F002)
- `src/crosshook-native/src/components/library/__tests__/HeroLaunchCommandSection.test.tsx` (coverage for F003)
- `src/crosshook-native/src/components/library/__tests__/HeroProfileActionsBar.test.tsx` (coverage for F004)
- `docs/prps/reviews/pr-489-review.md` (marked F001-F008 Fixed)

## Failed Fixes

None.

## Validation Results

| Check      | Result                                                                               |
| ---------- | ------------------------------------------------------------------------------------ |
| Type check | Pass (`npm run typecheck` in `src/crosshook-native`)                                 |
| Tests      | Pass (`npm test` in `src/crosshook-native`: 49 files, 361 tests)                     |
| Lint       | Pass with pre-existing unrelated warnings (`npm run lint` in `src/crosshook-native`) |

## Next Steps

- Re-run `$code-review 489` to verify the fixes resolved the review findings.
- Run `$git-workflow --commit` to commit the fixes when satisfied.
