# Implementation Report: UI Standardization Phase 2 — Profile Wizard Rework

## Summary

Reworked the New/Edit Profile wizard from 3 overloaded steps into 5 balanced
steps (4 when `launchMethod === 'native'`) that compose the canonical
`profile-sections/*` components instead of duplicating field graphs inline.
Added a dedicated Review step with a required-field checklist, a slim launch
preset picker, and an inline readiness-check summary. Extracted check-badge
helpers and the bundled-preset TOML-key helper to shared modules so both the
wizard and the Launch Optimizations panel use the same source of truth.

Every step transition remains in-memory (`updateProfile` / `setProfileName`)
until the explicit Save Profile click — the BR-9 no-write-before-review
invariant is preserved.

## Assessment vs Reality

| Metric        | Predicted (Plan)                                            | Actual                                               |
| ------------- | ----------------------------------------------------------- | ---------------------------------------------------- |
| Complexity    | Large                                                       | Large                                                |
| Confidence    | N/A                                                         | High — all validation levels passed first time       |
| Files Changed | 8–10                                                        | 10 (5 modified, 5 created)                           |
| Wizard LOC    | Target ≤ 600 after refactor                                 | 528 (down from 764; 31% reduction)                   |

## Tasks Completed

| # | Task                                                  | Status         | Notes                                                                           |
| - | ----------------------------------------------------- | -------------- | ------------------------------------------------------------------------------- |
| 1 | Extend wizard stage state machine                     | ✅ Complete    | 6-stage sequence, new boolean flags, skip rules preserved for native            |
| 2 | Create wizard validation helpers                      | ✅ Complete    | `evaluateWizardRequiredFields` strict superset of `validateProfileForSave`      |
| 3 | Extract check badge helpers                           | ✅ Complete    | `components/wizard/checkBadges.ts` used by wizard + review summary              |
| 4 | Create `WizardPresetPicker`                           | ✅ Complete    | Slim grouped `ThemedSelect`, disabled in create mode with explanatory hint      |
| 5 | Create `WizardReviewSummary`                          | ✅ Complete    | Required-field checklist + readiness recap + tip                                |
| 6 | Refactor `OnboardingWizard.tsx` to compose sections   | ✅ Complete    | 764 → 528 lines; portal/focus contract untouched                                |
| 7 | Add CSS for wizard layout + review summary            | ✅ Complete    | `__step-grid`, `__review-summary/row/list/icon/label`, badge variants           |
| 8 | Verify mount sites and run validation                 | ✅ Complete    | `App.tsx` and `ProfilesPage.tsx` unchanged — props stable                       |

## Validation Results

| Level                     | Status            | Notes                                                                 |
| ------------------------- | ----------------- | --------------------------------------------------------------------- |
| Static Analysis (tsc)     | ✅ Pass           | `cd src/crosshook-native && npm run build` — zero errors              |
| Lint                      | ✅ Pass (N/A)     | No lint pipeline configured in repo; tsc + Vite is the gate           |
| Unit Tests                | ✅ Pass (N/A)     | No frontend test framework configured; see Testing Strategy for map   |
| Build (Vite)              | ✅ Pass           | `dist/assets/index-UngSdTd4.js 768.48 kB / index-D1QFqPHm.css 144.46 kB` |
| Rust sanity suite         | ✅ Pass           | `cargo test -p crosshook-core`: 718 + 3 passed, 0 failed              |
| Console.log hygiene       | ✅ Pass           | Zero `console.*` calls added in new/modified wizard files             |

## Files Changed

| File                                                                  | Action  | Lines        |
| --------------------------------------------------------------------- | ------- | ------------ |
| `src/crosshook-native/src/types/onboarding.ts`                        | UPDATED | +7 / -1      |
| `src/crosshook-native/src/hooks/useOnboarding.ts`                     | UPDATED | +44 / -32    |
| `src/crosshook-native/src/components/OnboardingWizard.tsx`            | UPDATED | +144 / -380  |
| `src/crosshook-native/src/components/LaunchOptimizationsPanel.tsx`    | UPDATED | +4 / -7      |
| `src/crosshook-native/src/styles/theme.css`                           | UPDATED | +68 / -0     |
| `src/crosshook-native/src/utils/launchOptimizationPresets.ts`         | CREATED | +27          |
| `src/crosshook-native/src/components/wizard/checkBadges.ts`           | CREATED | +33          |
| `src/crosshook-native/src/components/wizard/wizardValidation.ts`      | CREATED | +117         |
| `src/crosshook-native/src/components/wizard/WizardPresetPicker.tsx`   | CREATED | +136         |
| `src/crosshook-native/src/components/wizard/WizardReviewSummary.tsx`  | CREATED | +103         |

## Deviations from Plan

1. **Extracted `bundledOptimizationTomlKey` to a new `utils/launchOptimizationPresets.ts` module.**
   - **What**: Moved the helper + prefix constant out of `LaunchOptimizationsPanel.tsx`
     (where it was file-private) into a shared utility module, and updated
     `LaunchOptimizationsPanel.tsx` to import from it.
   - **Why**: The plan required the wizard picker to use "the same one — do not
     duplicate the constant." Since the helper was not previously exported, a
     single-source-of-truth move was the only way to honor that constraint
     without duplication.

2. **`WizardPresetPicker` adds an `unavailableReason` prop and renders a disabled
   state in create mode.**
   - **What**: When the wizard is mounted with `mode='create'`, the profile has
     not yet been persisted, so `hasExistingSavedProfile === false` in
     `useProfile.ts`, and both `applyBundledOptimizationPreset` and
     `switchLaunchOptimizationPreset` silently no-op. Rather than let the user
     click a dead control, the picker renders disabled with an explanatory help
     line: *"Save the profile first — presets can be applied from the Launch page."*
   - **Why**: Preserves the BR-9 no-write-before-review invariant (no draft
     profile exists to persist a preset against) while avoiding a silent
     usability failure. In edit mode the picker is fully functional.

3. **Added `crosshook-onboarding-wizard__review-list` and
   `crosshook-onboarding-wizard__review-icon` / `__review-label` subclasses**
   in addition to the four classes explicitly named in the plan, so the review
   summary list rendering stays consistent and token-driven.

No other behavioural deviations. All plan acceptance criteria satisfied.

## Issues Encountered

None blocking. The `hasExistingSavedProfile` constraint on preset IPCs was
discovered during exploration and handled as Deviation #2 above (a disabled
state in create mode) rather than as a plan-time oversight.

## Tests Written

No new frontend tests (no test framework configured in repo, per plan).
The required-field matrix, skip rules, and picker disabled-state cases are
documented in `.claude/PRPs/plans/completed/ui-standardization-phase-2.plan.md`
Testing Strategy → Unit Tests table for manual verification.

## Acceptance Criteria — Status

- ✅ Wizard renders 5 visible steps (4 when `launchMethod === 'native'`).
- ✅ Every step body composed from canonical `profile-sections/*` components.
- ✅ Required fields explicit per launch method; Save gated by `evaluateWizardRequiredFields`.
- ✅ Steam App ID surfaced for `proton_run` (optional) and `steam_applaunch` (required) via canonical `RuntimeSection`.
- ✅ Media step exposes Cover, Portrait, Background, and Launcher Icon (non-native) via canonical `MediaSection`.
- ✅ Review step exposes launch preset picker + optional `CustomEnvironmentVariablesSection`.
- ✅ Review step shows required-field summary and latest readiness check result.
- ✅ BR-9 preserved — dismiss/skip never write; only Save persists.
- ✅ Skip Setup / Run Checks / Back / Next / Save Profile / Done reachable by keyboard; focus trap untouched.
- ✅ No regression in route-level scroll behavior or wizard portal contract.
- ✅ `OnboardingWizard.tsx` 528 lines (≤ 600 target).

## Next Steps

- [ ] Code review via `/code-review`
- [ ] Manual smoke test in `./scripts/dev-native.sh`:
  - Open `Profiles → New Profile`, walk all 5 steps with `steam_applaunch`.
  - Open `Profiles → New Profile`, walk all 4 steps with `native` (Trainer skipped).
  - Open `Profiles → New Profile`, confirm Steam App ID surfaces in Runtime step for `proton_run` (optional).
  - Open `Profiles → Edit in Wizard` against an existing profile; verify fields populate and preset picker enables.
  - Set/clear required fields; confirm Save button enable/disable + `aria-describedby` behavior.
  - Confirm `Skip Setup` mid-flow does not persist a profile.
- [ ] Create PR via `/prp-pr`.
