# Fix Report: pr-231-review

**Source**: `docs/prps/reviews/pr-231-review.md`
**Applied**: 2026-04-14
**Mode**: Parallel (3 batches, max width 5)
**Severity threshold**: LOW

## Summary

- **Total findings in source**: 12
- **Already processed before this run**:
  - Fixed: 0
  - Failed: 0
- **Eligible this run**: 12
- **Applied this run**:
  - Fixed: 12
  - Failed: 0
- **Skipped this run**:
  - Below severity threshold: 0
  - No suggested fix: 0
  - Missing file: 0

## Fixes Applied

| ID   | Severity | File                                                    | Line    | Status | Notes                                                      |
| ---- | -------- | ------------------------------------------------------- | ------- | ------ | ---------------------------------------------------------- |
| F001 | HIGH     | `tasks/lessons.md`                                      | 8       | Fixed  | Replaced `effective_trainer_gamescope()` with `resolved_trainer_gamescope()` |
| F002 | MEDIUM   | `crates/crosshook-core/src/launch/script_runner.rs`     | 322     | Fixed  | Added doc-comment above `build_trainer_command` re: trainer-only env drop, links #229 |
| F007 | MEDIUM   | `crates/crosshook-core/src/launch/script_runner.rs`     | 242     | Fixed  | Resolved trainer gamescope once; plumbed `&GamescopeConfig` into `helper_arguments`/`trainer_arguments` |
| F003 | MEDIUM   | `crates/crosshook-core/src/launch/{mod.rs,request.rs}` + `profile/models.rs` | 119 / 404 | Fixed | Extracted `pub(crate) resolve_trainer_gamescope(...)` into `launch/mod.rs`; both impls delegate |
| F006 | MEDIUM   | `crates/crosshook-core/src/profile/models.rs`           | tests   | Fixed  | Added 3 parity tests: explicit-enabled, disabled-override auto-derive, disabled-game default |
| F005 | MEDIUM   | `crates/crosshook-core/src/launch/preview.rs`           | tests   | Fixed  | Added `preview_trainer_only_auto_derives_windowed_gamescope_when_trainer_gamescope_is_none` (None-branch coverage: `-W`/`-H` present, `-f` absent) |
| F008 | MEDIUM   | `docs/prps/reports/auto-trainer-gamescope-report.md`    | 12      | Fixed  | "Files Changed: 11 (8 production code + 3 docs/lessons)" |
| F004 | MEDIUM   | `src/components/ProfileSubTabs.tsx`                     | 50      | Fixed  | Added parity-required comment referencing `LaunchRequest::resolved_trainer_gamescope` / `LaunchSection::resolved_trainer_gamescope` |
| F009 | LOW      | `src/components/GamescopeConfigPanel.tsx` + `styles/theme.css` | 117-121 | Fixed | Introduced `crosshook-info-banner` style (using `--crosshook-color-info`) and applied to `role="note"` element |
| F010 | LOW      | `src/components/ProfileSubTabs.tsx`                     | 274-277 | Fixed  | Trimmed `derivedConfigNotice` to suggested copy |
| F011 | LOW      | `src/components/ProfileSubTabs.tsx`                     | 54-76   | Fixed  | Verified TS types are already optional in `types/profile.ts:141-142`; `?.enabled` is correct and retained (no code change needed — alignment already holds) |
| F012 | LOW      | `src/components/ProfileSubTabs.tsx`                     | 112     | Fixed  | Wrapped with `useMemo(() => resolveTrainerGamescopeForDisplay(profile), [profile])`; added `useMemo` to react import |

## Files Changed

- `tasks/lessons.md` (Fixed F001)
- `src/crosshook-native/crates/crosshook-core/src/launch/mod.rs` (Fixed F003 — added shared `resolve_trainer_gamescope`)
- `src/crosshook-native/crates/crosshook-core/src/launch/request.rs` (Fixed F003)
- `src/crosshook-native/crates/crosshook-core/src/profile/models.rs` (Fixed F003, F006)
- `src/crosshook-native/crates/crosshook-core/src/launch/script_runner.rs` (Fixed F002, F007)
- `src/crosshook-native/crates/crosshook-core/src/launch/preview.rs` (Fixed F005)
- `docs/prps/reports/auto-trainer-gamescope-report.md` (Fixed F008)
- `src/crosshook-native/src/components/ProfileSubTabs.tsx` (Fixed F004, F010, F011 [no-op], F012)
- `src/crosshook-native/src/components/GamescopeConfigPanel.tsx` (Fixed F009)
- `src/crosshook-native/src/styles/theme.css` (Fixed F009 — new `.crosshook-info-banner`)

## Failed Fixes

_(none)_

## Validation Results

| Check                  | Result |
| ---------------------- | ------ |
| Rust type check (`cargo check -p crosshook-core --tests`) | Pass |
| Rust tests (`cargo test -p crosshook-core`)   | Pass (851 + 1 + 3 = 855 tests green) |
| Frontend type check (`npx tsc --noEmit`)      | Pass (no output) |

## Execution Summary

- **Batch 1 (HIGH, sequential)**: F001 — 1 agent, 1 fix.
- **Batch 2 (MEDIUM, parallel, width 5)**: F002+F007 / F003+F006 / F005 / F008 / F004+F010+F011+F012 — 5 concurrent `ycc:review-fixer` agents, 10 fixes. Between-batch validation: cargo check + cargo test → Pass.
- **Batch 3 (LOW, sequential)**: F009 — 1 agent, 1 fix. Final validation: cargo test + tsc → Pass.

## Next Steps

- Re-run `/ycc:code-review 231` to verify all findings are resolved and no new issues were introduced
- Run `/ycc:git-workflow` to commit the changes when satisfied
