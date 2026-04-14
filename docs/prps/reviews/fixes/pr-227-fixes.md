# Fix Report: pr-227-review

**Source**: `docs/prps/reviews/pr-227-review.md`
**Applied**: 2026-04-13
**Mode**: Parallel (3 batches, max width 2)
**Severity threshold**: LOW

## Summary

- **Total findings in source**: 9
- **Already processed before this run**:
  - Fixed: 0
  - Failed: 0
- **Eligible this run**: 9
- **Applied this run**:
  - Fixed: 8
  - Failed: 1
- **Skipped this run**:
  - Below severity threshold: 0
  - No suggested fix: 0
  - Missing file: 0

## Fixes Applied

| ID   | Severity | File                                                                     | Line | Status | Notes                                                                                                                                                               |
| ---- | -------- | ------------------------------------------------------------------------ | ---- | ------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| F001 | HIGH     | `src/crosshook-native/runtime-helpers/steam-host-trainer-runner.sh`      | 110  | Fixed  | Preserved custom env now goes through a protected env file instead of `flatpak-spawn --env=...`.                                                                    |
| F002 | HIGH     | `src/crosshook-native/runtime-helpers/steam-host-trainer-runner.sh`      | 96   | Fixed  | Removed wildcard sandbox-env replay; only explicit session keys plus curated built-in keys cross the Flatpak host boundary.                                         |
| F003 | HIGH     | `src/crosshook-native/crates/crosshook-core/src/launch/script_runner.rs` | 603  | Fixed  | `helper_arguments()` now forwards trainer-gamescope flags, and the Steam helper delegates the trainer leg to the shared host runner.                                |
| F004 | HIGH     | `src/crosshook-native/src/components/InstallGamePanel.tsx`               | 33   | Fixed  | Replaced `Object.hasOwn(...)` with `Object.prototype.hasOwnProperty.call(...)`.                                                                                     |
| F005 | MEDIUM   | `src/crosshook-native/src-tauri/src/commands/launch.rs`                  | 292  | Fixed  | Added log-based diagnostic method selection so trainer-runner failures are analyzed with `proton_run` semantics while the outer Steam helper launch remains intact. |
| F006 | MEDIUM   | `tasks/todo.md`                                                          | 1    | Failed | The broader “split unrelated churn out of PR #227” fix requires branch-scope cleanup/history surgery outside a safe targeted fix pass.                              |
| F007 | MEDIUM   | `.github/workflows/lint-autofix.yml`                                     | 43   | Fixed  | Autofix staging now excludes `CHANGELOG.md`, archived PRP reports, and review outputs instead of using `git add -A`.                                                |
| F008 | MEDIUM   | `src/crosshook-native/crates/crosshook-core/src/launch/script_runner.rs` | 75   | Fixed  | The helper path now reuses the shared trainer runner instead of maintaining a second shell-level trainer env/gamescope contract.                                    |
| F009 | LOW      | `.claude/PRPs/reports/ui-standardization-phase-4-report.md`              | 93   | Fixed  | Repaired the malformed archived Markdown cited by the review.                                                                                                       |

## Files Changed

- `src/crosshook-native/crates/crosshook-core/src/launch/script_runner.rs` (F001, F002, F003, F008)
- `src/crosshook-native/runtime-helpers/steam-host-trainer-runner.sh` (F001, F002, F008)
- `src/crosshook-native/runtime-helpers/steam-launch-helper.sh` (F001, F002, F003, F008)
- `src/crosshook-native/src-tauri/src/commands/launch.rs` (F005)
- `src/crosshook-native/src/components/InstallGamePanel.tsx` (F004)
- `.github/workflows/lint-autofix.yml` (F007)
- `.claude/PRPs/reports/ui-standardization-phase-4-report.md` (F009)

## Failed Fixes

### F006 — `tasks/todo.md:1`

**Severity**: MEDIUM
**Category**: Pattern Compliance
**Description**: This PR still bundles unrelated hook/autofix/docs churn, which keeps the launch review scope wider than it should be.
**Suggested fix (from review)**: Restore `tasks/todo.md` in this branch and split the non-launch hook/docs/autofix work into separate `chore(...)` or `docs(internal): ...` changes outside PR #227.
**Blocker**: Splitting unrelated work out of the existing PR requires branch-scope cleanup or history rewriting beyond a safe targeted review-fix pass.
**Recommendation**: Follow up with a separate PR-scope cleanup: move the unrelated autofix/docs churn out of PR #227, or explicitly coordinate a branch rewrite if that is still desired.

## Validation Results

| Check      | Result |
| ---------- | ------ |
| Type check | Pass   |
| Tests      | Pass   |

## Next Steps

- Re-run `$code-review 227` to confirm the remaining branch-scope finding is the only unresolved item.
- Decide whether to do a PR-scope cleanup for F006 or accept that scope issue separately from the trainer-parity fixes.
- Use `$git-workflow` when the branch is ready to commit/push.
