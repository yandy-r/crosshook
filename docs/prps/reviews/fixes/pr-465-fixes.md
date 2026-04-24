# Fix Report: pr-465-review

**Source**: `docs/prps/reviews/pr-465-review.md`
**Applied**: 2026-04-23T22:32:00Z
**Mode**: Sequential (Path A)
**Severity threshold**: MEDIUM

## Summary

- **Total findings in source**: 11
- **Already processed before this run**: 1 Fixed (F002 — smoke heading mismatch, fixed before review-fix invocation), 0 Failed
- **Eligible this run**: 5 (F001, F003 HIGH; F004, F005, F006 MEDIUM)
- **Applied this run**: Fixed 5, Failed 0
- **Skipped this run**:
  - Below severity threshold: 5 (F007–F011 LOW)
  - No suggested fix: 0
  - Missing file: 0

## Fixes Applied

| ID   | Severity | File(s)                                                                       | Line | Status | Notes                                                                      |
| ---- | -------- | ----------------------------------------------------------------------------- | ---- | ------ | -------------------------------------------------------------------------- |
| F001 | HIGH     | `src/hooks/gamepad-nav/dom.ts` + `src/hooks/__tests__/useGamepadNav.test.tsx` | 69   | Fixed  | Inspector zone now reachable; new unit test covers back-from-inspector     |
| F003 | HIGH     | `src/styles/library.css` + `tests/smoke.spec.ts`                              | 299  | Fixed  | Card-root hover-lift suppressed under `prefers-reduced-motion`             |
| F004 | MEDIUM   | `src/__tests__/a11y/modals.a11y.test.tsx` (new)                               | —    | Fixed  | 3 modals: `ProfileReviewModal`, `LauncherPreviewModal`, `OnboardingWizard` |
| F005 | MEDIUM   | `src/__tests__/a11y/routes.a11y.test.tsx`                                     | 144+ | Fixed  | 3 populated-fixture tests: Library, Launch, HealthDashboard (substitute)   |
| F006 | MEDIUM   | `src/components/ui/ThemedSelectField.tsx` (new) + 4 migrations                | —    | Fixed  | Migrated 4 of ~8 call-sites; 4 skipped as scope-unsafe (see agent notes)   |

## Files Changed

- `src/crosshook-native/src/hooks/gamepad-nav/dom.ts` (F001: accept `'inspector'` in explicit-zone check)
- `src/crosshook-native/src/hooks/__tests__/useGamepadNav.test.tsx` (F001: new inspector back-nav test)
- `src/crosshook-native/src/styles/library.css` (F003: card-root reduced-motion guard)
- `src/crosshook-native/tests/smoke.spec.ts` (F003: assert card-root `transitionDuration === '0s'`)
- `src/crosshook-native/src/__tests__/a11y/modals.a11y.test.tsx` (F004: new file, 3 modal axe tests)
- `src/crosshook-native/src/__tests__/a11y/routes.a11y.test.tsx` (F005: populated-fixture describe block)
- `src/crosshook-native/src/components/ui/ThemedSelectField.tsx` (F006: new shared wrapper)
- `src/crosshook-native/src/components/profile-sections/RunnerMethodSection.tsx` (F006: migration)
- `src/crosshook-native/src/components/community/CommunityProfilesSection.tsx` (F006: migration)
- `src/crosshook-native/src/components/host-readiness/HostToolFilterBar.tsx` (F006: migrated 2 selects)
- `src/crosshook-native/src/components/settings/RunnerSection.tsx` (F006: migration)

## Failed Fixes

None this run. F006 had 4 call-sites skipped (not failed) — each preserved scope-discipline rather than force-fitting the new wrapper onto call-sites with non-trivial surrounding markup:

- `RuntimeSection.tsx` — select wrapped in extra control-class div
- `ProfilesHero.tsx` — label has inline style + nested wrapper div
- `LaunchProfileSelector.tsx` — label lives in a sibling component (cross-component labelling)
- `WizardPresetPicker.tsx` — uses help-text paragraph, not `<label>`

These remain as follow-up opportunities; not regressions.

## Validation Results

| Check      | Result                                                                                                                         |
| ---------- | ------------------------------------------------------------------------------------------------------------------------------ |
| Type check | Pass                                                                                                                           |
| Tests      | 194/196 (2 pre-existing `AppShell.test.tsx` palette-autofocus failures; documented in PR #465 report as reproducing on `main`) |
| Lint       | Pass (Biome + tsc + shellcheck + host-gateway + legacy-palette; only pre-existing warnings remain)                             |

## Next Steps

- Open a follow-up issue for the 4 skipped `ThemedSelectField` call-sites noted above (F006 residuals).
- Open a follow-up issue for `@axe-core/playwright` against `?fixture=populated` to restore `color-contrast` auditing (tied to F005's partial scope).
- F007–F011 (LOW) remain `Status: Open` in the review artifact — address in a future polish pass or close once follow-ups are tracked.
- Re-running `/ycc:code-review 465` after these changes land on the branch should report zero open HIGH findings.
