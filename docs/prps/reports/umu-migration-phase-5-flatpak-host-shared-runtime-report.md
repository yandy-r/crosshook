# Implementation Report: umu-launcher Migration — Phase 5 (Flatpak host-shared runtime + install guidance)

## Summary

Implemented Phase 5 end-to-end with parallel batch execution. The Flatpak manifest now grants host-shared UMU runtime access, onboarding readiness emits structured install guidance for Flatpak + missing `umu-run`, onboarding exposes a dedicated dismiss command, and frontend review UI renders actionable guidance controls (copy/open docs/dismiss) while preserving Proton fallback behavior.

## Assessment vs Reality

| Metric        | Predicted (Plan) | Actual |
| ------------- | ---------------- | ------ |
| Complexity    | Medium           | Medium |
| Confidence    | Medium-High      | High   |
| Files Changed | 10               | 13     |

## Tasks Completed

| #   | Task                                                           | Status          | Notes                                                                        |
| --- | -------------------------------------------------------------- | --------------- | ---------------------------------------------------------------------------- |
| 1.1 | Add Flatpak host-shared umu filesystem permission              | [done] Complete |                                                                              |
| 1.2 | Add persisted install-nag dismissal setting field              | [done] Complete |                                                                              |
| 1.3 | Upgrade readiness umu check to actionable guidance payload     | [done] Complete |                                                                              |
| 2.1 | Surface new settings field across settings IPC boundary        | [done] Complete |                                                                              |
| 2.2 | Add onboarding IPC for install-guidance dismissal              | [done] Complete |                                                                              |
| 2.3 | Extend onboarding TS contracts and hook state                  | [done] Complete |                                                                              |
| 3.1 | Render actionable UMU install guidance in onboarding review UI | [done] Complete |                                                                              |
| 3.2 | Ensure preferences/settings persistence flow handles timestamp | [done] Complete |                                                                              |
| 3.3 | Add/adjust tests across readiness/settings/onboarding commands | [done] Complete |                                                                              |
| 4.1 | Run full validation + browser smoke startup checks             | [done] Complete | Manual Flatpak runtime scenarios remain to be run on a Flatpak host session. |

## Validation Results

| Level           | Status         | Notes                                                                                             |
| --------------- | -------------- | ------------------------------------------------------------------------------------------------- |
| Static Analysis | [done] Pass    | `./scripts/lint.sh` passes after all edits                                                        |
| Unit Tests      | [done] Pass    | `cargo test -p crosshook-core` passes including new readiness/settings tests                      |
| Build           | [done] Pass    | `cargo check -p crosshook-native` (from task validation) and command test builds pass             |
| Integration     | [done] Pass    | `cargo test -p crosshook-native onboarding` passes                                                |
| Edge Cases      | [done] Partial | Browser dev mode startup validated; host-specific Flatpak manual checklist still required locally |

## Files Changed

| File                                                                     | Action  | Lines     |
| ------------------------------------------------------------------------ | ------- | --------- |
| `packaging/flatpak/dev.crosshook.CrossHook.yml`                          | UPDATED | +4 / -0   |
| `src/crosshook-native/crates/crosshook-core/src/onboarding/mod.rs`       | UPDATED | +17 / -0  |
| `src/crosshook-native/crates/crosshook-core/src/onboarding/readiness.rs` | UPDATED | +125 / -2 |
| `src/crosshook-native/crates/crosshook-core/src/settings/mod.rs`         | UPDATED | +40 / -0  |
| `src/crosshook-native/src-tauri/src/commands/onboarding.rs`              | UPDATED | +39 / -0  |
| `src/crosshook-native/src-tauri/src/commands/settings.rs`                | UPDATED | +149 / -0 |
| `src/crosshook-native/src-tauri/src/lib.rs`                              | UPDATED | +1 / -0   |
| `src/crosshook-native/src/components/OnboardingWizard.tsx`               | UPDATED | +9 / -1   |
| `src/crosshook-native/src/components/wizard/WizardReviewSummary.tsx`     | UPDATED | +80 / -3  |
| `src/crosshook-native/src/hooks/useOnboarding.ts`                        | UPDATED | +15 / -1  |
| `src/crosshook-native/src/lib/mocks/handlers/onboarding.ts`              | UPDATED | +7 / -0   |
| `src/crosshook-native/src/types/onboarding.ts`                           | UPDATED | +14 / -0  |
| `src/crosshook-native/src/types/settings.ts`                             | UPDATED | +8 / -0   |

## Deviations from Plan

- Added `src/crosshook-native/src/lib/mocks/handlers/onboarding.ts` updates to keep browser-mode mock IPC contract aligned with the new onboarding readiness payload and dismiss command.
- Used `Option<String>` RFC3339 timestamp representation for `install_nag_dismissed_at` to match existing settings serialization conventions instead of introducing a typed datetime wrapper in settings structs.

## Issues Encountered

- A transient compile failure occurred after Task 1.2 because `AppSettingsData` initializer usage in Tauri settings merge did not yet include the new field. Resolved in Task 2.1 by wiring the field across IPC DTO/request/merge behavior with explicit preserve/set/clear tests.
- A temporary branch context switch happened mid-run during repository migration work; execution resumed on the feature branch and completed without reverting any user changes.

## Tests Written

| Test File                                                                | Tests                                                                       | Coverage                                                          |
| ------------------------------------------------------------------------ | --------------------------------------------------------------------------- | ----------------------------------------------------------------- |
| `src/crosshook-native/crates/crosshook-core/src/settings/mod.rs`         | Added backward-compat + roundtrip assertions for `install_nag_dismissed_at` | Missing-field default and persisted timestamp roundtrip           |
| `src/crosshook-native/crates/crosshook-core/src/onboarding/readiness.rs` | Added Flatpak/native missing-umu guidance behavior tests                    | Guidance payload gating and `all_passed` semantics                |
| `src/crosshook-native/src-tauri/src/commands/settings.rs`                | Added request merge + serialization tests for new field                     | Preserve/set/clear IPC merge semantics                            |
| `src/crosshook-native/src-tauri/src/commands/onboarding.rs`              | Added dismissal behavior/signature tests                                    | New `dismiss_umu_install_nag` command contract and state mutation |

## Next Steps

- [ ] Run `/code-review` before commit/PR.
- [ ] Execute manual Flatpak checklist scenarios from the plan on a Flatpak host runtime session.
- [ ] Create PR via `/prp-pr` once manual checks are complete.
