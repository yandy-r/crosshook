# Fix Report: pr-266-review

**Source**: `docs/prps/reviews/pr-266-review.md`
**Applied**: 2026-04-15
**Mode**: Parallel (1 batch, max width 1)
**Severity threshold**: HIGH

## Summary

- **Total findings in source**: 2
- **Already processed before this run**:
  - Fixed: 0
  - Failed: 0
- **Eligible this run**: 2
- **Applied this run**:
  - Fixed: 2
  - Failed: 0
- **Skipped this run**:
  - Below severity threshold: 0
  - No suggested fix: 0
  - Missing file: 0

## Fixes Applied

| ID   | Severity | File                                                                | Line | Status | Notes                                                                                                                                      |
| ---- | -------- | ------------------------------------------------------------------- | ---- | ------ | ------------------------------------------------------------------------------------------------------------------------------------------ |
| F001 | HIGH     | `src/crosshook-native/crates/crosshook-core/src/export/launcher.rs` | 537  | Fixed  | Export request now carries effective `umu_preference` through frontend, Tauri, and core launcher-store paths.                              |
| F002 | HIGH     | `src/crosshook-native/crates/crosshook-core/src/export/launcher.rs` | 557  | Fixed  | Exported `umu-run` branch now emits `GAMEID`, `PROTON_VERB=runinprefix`, and resolved `PROTONPATH`, while direct Proton clears those vars. |

## Files Changed

- `src/crosshook-native/crates/crosshook-core/src/export/launcher.rs`
- `src/crosshook-native/crates/crosshook-core/src/export/launcher_store.rs`
- `src/crosshook-native/src-tauri/src/commands/export.rs`
- `src/crosshook-native/src/components/LauncherExport.tsx`
- `src/crosshook-native/src/hooks/useLauncherExport.ts`

## Validation Results

| Check      | Result |
| ---------- | ------ |
| Type check | Pass   |
| Tests      | Pass   |

## Validation Notes

- `cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core`
- Added focused coverage in `export::launcher` for Proton opt-out bypass and exported umu env parity.

## Next Steps

- Re-run `$code-review 266` if you want a fresh artifact against the updated branch.
- Use `$git-workflow` when you want to commit the fix set.
