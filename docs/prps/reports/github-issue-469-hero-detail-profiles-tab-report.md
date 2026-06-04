# Implementation Report: Hero Detail Profiles Tab

## Summary

Implemented the Phase 4 Hero Detail Profiles tab for issue #469. The tab now renders a two-pane per-game profile editor with profile cards on the left, flattened identity/runtime/game/media sections on the right, card-click profile switching through `ProfileContext`, and 350ms autosave through `persistProfileDraft`.

## Assessment vs Reality

| Metric        | Predicted (Plan) | Actual |
| ------------- | ---------------- | ------ |
| Complexity    | Medium           | Medium |
| Confidence    | High             | High   |
| Files Changed | 9                | 9      |

## Tasks Completed

| #   | Task                              | Status | Notes                                                                                         |
| --- | --------------------------------- | ------ | --------------------------------------------------------------------------------------------- |
| 1.1 | Create `useProfileCardMeta`       | done   | Normalizes `profile_load` results and degrades per-card failures.                             |
| 1.2 | CSS classes + scroll registration | done   | Added two-pane/card/editor styles and registered the editor scroll container.                 |
| 1.3 | GameDetail wiring                 | done   | Hero pills and preview now follow the selected singleton profile for the current game.        |
| 2.1 | Create `HeroDetailProfilesTab`    | done   | Implemented cards, flattened editor, wizard CTA, flush-before-switch, and debounced autosave. |
| 3.1 | Swap panels switch                | done   | Replaced `ProfilesPanel` and updated existing panel tests.                                    |
| 3.2 | Dedicated tests                   | done   | Added autosave, switch, alignment, keyboard, rename-pause, and flush tests.                   |
| 4.1 | Full validation                   | done   | Typecheck, lint, focused tests, full tests, and binary build passed.                          |

## Validation Results

| Level           | Status | Notes                                                                                                     |
| --------------- | ------ | --------------------------------------------------------------------------------------------------------- |
| Static Analysis | PASS   | `cd src/crosshook-native && npm run typecheck`                                                            |
| Lint            | PASS   | `./scripts/lint.sh --ts` exited 0; pre-existing warnings remain outside touched files.                    |
| Unit Tests      | PASS   | Focused tests passed: `HeroDetailProfilesTab`, `HeroDetailPanels`, `GameDetail`.                          |
| Full Test Suite | PASS   | `cd src/crosshook-native && npm test` passed: 41 files, 235 tests.                                        |
| Build           | PASS   | `npm run build:binary` built and copied `crosshook-native`.                                               |
| Integration     | PASS   | `src/__tests__/a11y/components.a11y.test.tsx` passed after closed wizard mount was deferred.              |
| Browser Smoke   | PASS   | Existing `http://127.0.0.1:5173` dev server returned 200 and Playwright loaded the `CrossHook` app shell. |
| Edge Cases      | PASS   | Covered by tests: rename pause, dirty flush, mount alignment, keyboard selection.                         |

## Files Changed

| File                                                                                   | Action  | Lines     |
| -------------------------------------------------------------------------------------- | ------- | --------- |
| `src/crosshook-native/src/hooks/useProfileCardMeta.ts`                                 | CREATED | 85        |
| `src/crosshook-native/src/components/library/HeroDetailProfilesTab.tsx`                | CREATED | 284       |
| `src/crosshook-native/src/components/library/__tests__/HeroDetailProfilesTab.test.tsx` | CREATED | 268       |
| `src/crosshook-native/src/components/library/GameDetail.tsx`                           | UPDATED | +61 / -5  |
| `src/crosshook-native/src/components/library/HeroDetailPanels.tsx`                     | UPDATED | +10 / -51 |
| `src/crosshook-native/src/components/library/__tests__/GameDetail.test.tsx`            | UPDATED | +3 / -3   |
| `src/crosshook-native/src/components/library/__tests__/HeroDetailPanels.test.tsx`      | UPDATED | +23 / -25 |
| `src/crosshook-native/src/hooks/useScrollEnhance.ts`                                   | UPDATED | +1 / -1   |
| `src/crosshook-native/src/styles/hero-detail.css`                                      | UPDATED | +99 / -0  |

## Deviations from Plan

- Used native `<button>` profile cards inside list items instead of focusable `<li>` cards so Biome a11y rules pass while preserving click and Enter activation.
- Mounted `OnboardingWizard` only when the `+ New` CTA is open. This avoids requiring `PreferencesProvider` for closed tab renders and fixed the existing a11y test path.
- Used the profile context `steamClientInstallPath` directly for `useProtonInstalls`; no extra preferences dependency is needed in the tab.

## Issues Encountered

- Full Vitest initially failed because `OnboardingWizard` was mounted closed and called `usePreferencesContext` in a test harness without `PreferencesProvider`. Resolved by lazy-mounting the wizard only when open.
- `npm run lint` / `./scripts/lint.sh --ts` print existing warnings in unrelated files (`InstallGamePanel`, `Breadcrumb`, launch page files), but the scripts exit 0 and touched files pass targeted Biome checks.
- `./scripts/dev-native.sh --browser` could not start a second Vite server because port 5173 was already in use. Verified the existing server instead with `curl` and Playwright. Playwright reported only `/favicon.ico` 404.

## Tests Written

| Test File                                                                              | Tests   | Coverage                                                                                                                   |
| -------------------------------------------------------------------------------------- | ------- | -------------------------------------------------------------------------------------------------------------------------- |
| `src/crosshook-native/src/components/library/__tests__/HeroDetailProfilesTab.test.tsx` | 6       | Autosave debounce, no premature save, card switch, mount alignment, keyboard selection, rename pause, flush-before-switch. |
| `src/crosshook-native/src/components/library/__tests__/HeroDetailPanels.test.tsx`      | updated | Profiles switch integration and empty state.                                                                               |
| `src/crosshook-native/src/components/library/__tests__/GameDetail.test.tsx`            | updated | Live `updateProfile` and `profileList` panel channels.                                                                     |

## Next Steps

- [ ] Code review via `$code-review`
- [ ] Create PR via `$prp-pr`
