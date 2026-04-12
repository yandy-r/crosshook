# Fix Report: pr-214-review

**Source**: docs/prps/reviews/pr-214-review.md
**Applied**: 2026-04-12
**Mode**: Parallel (1 batch, max width 1)
**Severity threshold**: LOW

## Summary

- **Total findings in source**: 4
- **Already processed before this run**:
  - Fixed: 3
  - Failed: 0
- **Eligible this run**: 1
- **Applied this run**:
  - Fixed: 1
  - Failed: 0
- **Skipped this run**:
  - Below severity threshold: 0
  - No suggested fix: 0
  - Missing file: 0

## Fixes Applied

| ID   | Severity | File             | Line | Status | Notes |
| ---- | -------- | ---------------- | ---- | ------ | ----- |
| F004 | MEDIUM   | LaunchPage.tsx (via profile command + hooks) | 89   | Fixed  | `profile_list_summaries` now takes optional `collection_id` and applies `apply_collection_defaults` before `effective_profile()`; LaunchPage, LibraryPage, and ProfilesPage thread active collection id. |

## Files Changed

- `src/crosshook-native/src-tauri/src/commands/profile.rs` (Fixed F004)
- `src/crosshook-native/src/components/pages/LaunchPage.tsx`
- `src/crosshook-native/src/components/pages/LibraryPage.tsx`
- `src/crosshook-native/src/components/pages/ProfilesPage.tsx`
- `src/crosshook-native/src/hooks/useLibrarySummaries.ts`
- `src/crosshook-native/src/hooks/useProfileSummaries.ts`
- `src/crosshook-native/src/lib/mocks/handlers/profile.ts`

## Failed Fixes

None.

## Validation Results

| Check      | Result |
| ---------- | ------ |
| Type check | Pass (`npx tsc --noEmit` in src/crosshook-native; `cargo check -p crosshook-native`) |
| Tests      | Pass (`cargo test -p crosshook-core`) |

## Next Steps

- Re-run `/code-review` on PR 214 to confirm no regressions.
- Commit when satisfied (`/git-workflow`).
