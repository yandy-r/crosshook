# Fix Report: pr-457-review

**Source**: docs/prps/reviews/pr-457-review.md
**Applied**: 2026-04-22
**Mode**: Parallel sub-agents (orchestrated in single session; `--parallel --no-worktree`)
**Severity threshold**: MEDIUM (CRITICAL + HIGH + MEDIUM)

## Summary

- **Total findings in source**: 6
- **Already processed before this run**:
  - Fixed: 0
  - Failed: 0
- **Eligible this run**: 6
- **Applied this run**:
  - Fixed: 6
  - Failed: 0
- **Skipped this run**:
  - Below severity threshold: 0
  - No suggested fix: 0
  - Missing file: 0

## Fixes Applied

| ID   | Severity | File                                                 | Line | Status | Notes                                                                                                  |
| ---- | -------- | ---------------------------------------------------- | ---- | ------ | ------------------------------------------------------------------------------------------------------ |
| F001 | HIGH     | launch_queries.rs                                    | —    | Fixed  | Query by `profile_id` with `profile_id IS NULL AND profile_name` legacy branch; rename regression test |
| F002 | HIGH     | LibraryPage.tsx, LibraryList.tsx, LibraryListRow.tsx | —    | Fixed  | List view uses `inspectorPickName` + `onSelect`; details via icon when selecting                       |
| F003 | MEDIUM   | library.ts, LibraryPage.tsx                          | —    | Fixed  | `libraryCardDataEqual` covers all inspector-relevant card fields                                       |
| F004 | MEDIUM   | AppShell.tsx                                         | —    | Fixed  | Inspector width zero when `ROUTE_METADATA[route].inspectorComponent` is absent                         |
| F005 | MEDIUM   | LibraryToolbar.tsx, LibraryPage.tsx                  | —    | Fixed  | Toolbar shows only implemented sort/filter chips; removed placeholder filter branch                    |
| F006 | MEDIUM   | GameInspector.tsx, useLaunchHistoryForProfile.ts     | —    | Fixed  | Launch history IPC moved to `useLaunchHistoryForProfile`                                               |

## Files Changed

- `src/crosshook-native/crates/crosshook-core/src/metadata/launch_queries.rs`
- `src/crosshook-native/crates/crosshook-core/src/metadata/launch_queries_tests.rs`
- `src/crosshook-native/src/types/library.ts`
- `src/crosshook-native/src/hooks/useLaunchHistoryForProfile.ts`
- `src/crosshook-native/src/components/library/GameInspector.tsx`
- `src/crosshook-native/src/components/library/LibraryToolbar.tsx`
- `src/crosshook-native/src/components/library/LibraryList.tsx`
- `src/crosshook-native/src/components/library/LibraryListRow.tsx`
- `src/crosshook-native/src/components/pages/LibraryPage.tsx`
- `src/crosshook-native/src/components/layout/AppShell.tsx`
- `src/crosshook-native/src/components/library/__tests__/LibraryToolbar.test.tsx`
- `src/crosshook-native/src/components/pages/__tests__/LibraryPage.test.tsx`
- `docs/prps/reviews/pr-457-review.md`

## Failed Fixes

None.

## Validation Results

| Check      | Result                                                                                                                                          |
| ---------- | ----------------------------------------------------------------------------------------------------------------------------------------------- |
| Type check | Pass (`npm run typecheck` in `src/crosshook-native`)                                                                                            |
| Lint       | Pass (`npm run lint` in `src/crosshook-native`)                                                                                                 |
| Tests      | Pass (targeted Vitest: LibraryToolbar, LibraryPage, GameInspector, AppShell; `cargo test` filter `test_query_launch_history` on crosshook-core) |

## Next Steps

- Re-run `/code-review 457` to confirm no regressions and that the React max-depth warning is gone or acceptable.
- Commit fixes and the updated review artifact when satisfied (`--no-worktree` mode does not auto-commit this report).
