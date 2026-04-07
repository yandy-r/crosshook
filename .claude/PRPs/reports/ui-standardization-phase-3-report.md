# Implementation Report: UI Standardization Phase 3

## Summary

Implemented Install Game flow parity with the profile wizard: five tabs (`Identity & Game`, `Runtime`, `Trainer`, `Media`, `Installer & Review`), canonical `profile-sections/*` composition, `draftProfile` + derived `InstallGameRequest`, extended Rust/TS `InstallGameRequest` with runner method, Steam App ID routing, art paths, working directory, and launcher icon on the Rust struct; `evaluateInstallRequiredFields` wraps the wizard checklist plus installer EXE; `InstallReviewSummary` bundles install status, final executable, candidates, log, and `WizardReviewSummary`; CSS row-gap for wrapped install subtabs and `--skip-trainer` placeholder class.

## Assessment vs Reality

| Metric        | Predicted (Plan) | Actual                  |
| ------------- | ---------------- | ----------------------- |
| Complexity    | Large            | Large                   |
| Confidence    | High             | High                    |
| Files Changed | 9–11             | 12+ (incl. new helpers) |

## Tasks Completed

| #   | Task                                  | Status   | Notes                                                                     |
| --- | ------------------------------------- | -------- | ------------------------------------------------------------------------- |
| 1   | Extend `InstallGameRequest` Rust + TS | Complete | Added `launcher_icon_path` on Rust for IPC parity with TS.                |
| 2   | Rust tests for `reviewable_profile`   | Complete | Two new tests in `install/models.rs`.                                     |
| 3   | Refactor `useInstallGame`             | Complete | `draftProfile`, `installerInputs`, derived `request`, merged `setResult`. |
| 4   | Refactor `InstallGamePanel`           | Complete | Five tabs, `ProfileProvider` presets, env section, validation gate.       |
| 5   | `installValidation.ts`                | Complete | Overrides wizard game-executable requirement for pre-install.             |
| 6   | `InstallReviewSummary.tsx`            | Complete |                                                                           |
| 7   | `theme.css`                           | Complete | `row-gap` + `--skip-trainer` placeholder.                                 |
| 8   | `InstallPage`                         | N/A      | No TS changes required.                                                   |

## Validation Results

| Level           | Status | Notes                            |
| --------------- | ------ | -------------------------------- |
| Static Analysis | Pass   | `npm run build` (tsc + vite)     |
| Unit Tests      | Pass   | `cargo test -p crosshook-core`   |
| Build           | Pass   | Vite production build            |
| Integration     | N/A    | Manual install route recommended |
| Edge Cases      | N/A    | Checklist in plan — manual       |

## Files Changed

| File                                                                   | Action  |
| ---------------------------------------------------------------------- | ------- |
| `src/crosshook-native/crates/crosshook-core/src/install/models.rs`     | Updated |
| `src/crosshook-native/crates/crosshook-core/src/install/service.rs`    | Updated |
| `src/crosshook-native/src/types/profile.ts`                            | Updated |
| `src/crosshook-native/src/types/install.ts`                            | Updated |
| `src/crosshook-native/src/hooks/useInstallGame.ts`                     | Updated |
| `src/crosshook-native/src/components/InstallGamePanel.tsx`             | Updated |
| `src/crosshook-native/src/components/install/installValidation.ts`     | Created |
| `src/crosshook-native/src/components/install/InstallReviewSummary.tsx` | Created |
| `src/crosshook-native/src/styles/theme.css`                            | Updated |

## Deviations from Plan

- **Rust `launcher_icon_path`**: Added to `InstallGameRequest` so backend `reviewable_profile` sets `steam.launcher.icon_path` and the modal no longer needs a manual launcher-icon patch (TS already sent the field; struct now round-trips).
- **`evaluateInstallRequiredFields`**: Wizard requires game executable path; install flow treats it as satisfied pre-install (only wizard + installer EXE gate Install), per plan testing strategy.
- **`WizardReviewSummary`**: Reused as-is; System Checks section still shows wizard copy when `readinessResult` is null (acceptable per plan).

## Issues Encountered

None blocking; full `crosshook-core` test suite passes.

## Tests Written

| Test file / area    | Tests | Coverage                                      |
| ------------------- | ----- | --------------------------------------------- |
| `install/models.rs` | +2    | Extended `reviewable_profile` routing         |
| Frontend            | —     | No unit framework; `tsc` + build verification |

## Next Steps

- [ ] Manual smoke: Install route, all tabs, native runner (trainer hidden), install + modal handoff.
- [ ] Code review and PR with `Closes #163` / `#162` as appropriate.
