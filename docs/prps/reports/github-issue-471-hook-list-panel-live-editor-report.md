# Implementation Report: GitHub Issue 471 Hook List Panel Live Editor

## Summary

Implemented the issue #471 live editor for launch hook declarations in the Hero Detail Launch tab. The Launch tab now renders editable pre-launch and post-exit hook lists, persists profile hook changes through the existing profile draft path, and clearly links runtime execution follow-up work to issue #482.

## Assessment vs Reality

| Metric        | Predicted (Plan) | Actual |
| ------------- | ---------------- | ------ |
| Complexity    | Medium           | Medium |
| Confidence    | 8/10             | 8/10   |
| Files Changed | 7                | 8      |

## Tasks Completed

| #   | Task                                | Status          | Notes                                                                  |
| --- | ----------------------------------- | --------------- | ---------------------------------------------------------------------- |
| 1.1 | Create `HookListPanel`              | [done] Complete | Add/edit/remove/toggle UI with stage-aligned hook declarations.        |
| 1.2 | Create hook autosave helper         | [done] Complete | Debounced draft persistence reuses the launch optimization delay.      |
| 1.3 | Add Launch tab hook styles          | [done] Complete | Responsive panel, row, popover, invalid-row, and banner styles added.  |
| 2.1 | Wire Launch tab to profile context  | [done] Complete | Pre/post arrays update in memory and schedule profile draft saves.     |
| 2.2 | Preserve runtime execution deferral | [done] Complete | Banner links to issue #482 and no runtime launch path was changed.     |
| 3.1 | Add focused hook list tests         | [done] Complete | Covers attach, toggle, edit, remove, and invalid-row removal behavior. |
| 3.2 | Update Launch tab and panel tests   | [done] Complete | Covers autosave, profile mismatch guard, and updated Launch tab copy.  |
| 4.1 | Validate implementation             | [done] Complete | Typecheck, focused tests, full tests, Rust hook tests, gateway, build. |

## Validation Results

| Level           | Status      | Notes                                                                                                                         |
| --------------- | ----------- | ----------------------------------------------------------------------------------------------------------------------------- |
| Static Analysis | [done] Pass | `npm --prefix src/crosshook-native run typecheck`; targeted Biome check passed for all touched files.                         |
| Unit Tests      | [done] Pass | Focused Vitest suite: 14 tests passed; full Vitest suite: 433 tests passed.                                                   |
| Rust Tests      | [done] Pass | `cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core profile::models::tests::hooks`: 7 tests passed. |
| Build           | [done] Pass | `npm --prefix src/crosshook-native run build` completed; Vite emitted existing large-chunk/dynamic-import warnings.           |
| Integration     | [done] Pass | `HeroDetailLaunchTab` tests cover profile-context updates, debounce persistence, and selected-profile mismatch behavior.      |
| Host Gateway    | [done] Pass | `./scripts/check-host-gateway.sh` passed.                                                                                     |

## Lint Note

`npm --prefix src/crosshook-native run lint` still fails on unrelated pre-existing Biome findings in files not touched by this implementation, including `InstallGamePanel.tsx`, `CommunityBrowser.tsx`, `Breadcrumb.tsx`, `LibraryList.test.tsx`, and `HeroDetailProfilesTab.test.tsx`. The touched files pass targeted Biome checking after formatting.

## Files Changed

| File                                                                                 | Action  | Notes                                                   |
| ------------------------------------------------------------------------------------ | ------- | ------------------------------------------------------- |
| `src/crosshook-native/src/components/library/HookListPanel.tsx`                      | CREATED | Hook list editor component.                             |
| `src/crosshook-native/src/components/library/launch/useHeroLaunchHooksAutosave.ts`   | CREATED | Debounced hook profile draft autosave helper.           |
| `src/crosshook-native/src/components/library/__tests__/HookListPanel.test.tsx`       | CREATED | Focused component tests.                                |
| `src/crosshook-native/src/components/library/HeroDetailLaunchTab.tsx`                | UPDATED | Replaces placeholder with live hook editors and banner. |
| `src/crosshook-native/src/components/library/__tests__/HeroDetailLaunchTab.test.tsx` | UPDATED | Adds autosave and mismatch coverage.                    |
| `src/crosshook-native/src/components/library/__tests__/HeroDetailPanels.test.tsx`    | UPDATED | Updates Launch tab copy expectations.                   |
| `src/crosshook-native/src/styles/hero-detail.css`                                    | UPDATED | Hook editor responsive styles.                          |
| `docs/prps/plans/github-issue-471-hook-list-panel-live-editor.plan.md`               | MOVED   | Archived under `docs/prps/plans/completed/`.            |

## Deviations from Plan

- Blank hook names are allowed while editing so users can clear and retype the field without the row being converted into an invalid-row fallback mid-edit.
- Package-wide lint is not green because of unrelated existing Biome diagnostics; targeted lint for this implementation's files is green.

## Issues Encountered

- The first test pass exposed stale popover queries after controlled rerenders; tests now reacquire the popover after each state update.
- Fake timers plus `userEvent` caused autosave tests to hang; those tests use `fireEvent` for the simple click interactions and then advance timers deterministically.

## Tests Written

| Test File                                                                            | Tests   | Coverage                                                             |
| ------------------------------------------------------------------------------------ | ------- | -------------------------------------------------------------------- |
| `src/crosshook-native/src/components/library/__tests__/HookListPanel.test.tsx`       | 3       | Attach, toggle, edit, remove, invalid-row removal.                   |
| `src/crosshook-native/src/components/library/__tests__/HeroDetailLaunchTab.test.tsx` | Updated | Launch hook rendering, debounce persistence, empty arrays, mismatch. |
| `src/crosshook-native/src/components/library/__tests__/HeroDetailPanels.test.tsx`    | Updated | Updated Launch tab contract text.                                    |

## Next Steps

- [ ] Code review via `$code-review`
- [ ] Create PR via `$prp-pr closes issue 471`
