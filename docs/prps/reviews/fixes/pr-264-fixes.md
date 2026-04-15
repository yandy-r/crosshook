# Fix Report: pr-264-review

**Source**: `docs/prps/reviews/pr-264-review.md`  
**Applied**: 2026-04-15T10:57:00-04:00  
**Mode**: Parallel (1 batch, max width 5)  
**Severity threshold**: MEDIUM

## Summary

- **Total findings in source**: 25
- **Already processed before this run**:
  - Fixed: 5
  - Failed: 1
- **Eligible this run**: 10
- **Applied this run**:
  - Fixed: 10
  - Failed: 0
- **Skipped this run**:
  - Below severity threshold: 9
  - No suggested fix: 0
  - Missing file: 0

## Fixes Applied

| ID   | Severity | File                                                                      | Line    | Status | Notes                                                                                           |
| ---- | -------- | ------------------------------------------------------------------------- | ------- | ------ | ----------------------------------------------------------------------------------------------- | --- | ----------------- |
| F007 | MEDIUM   | `src/crosshook-native/crates/crosshook-core/src/umu_database/client.rs`   | 19-21   | Fixed  | Added rationale comments for cache/timeout constants and extracted shared CSV subpath constant. |
| F008 | MEDIUM   | `src/crosshook-native/crates/crosshook-core/src/umu_database/client.rs`   | 88-109  | Fixed  | Removed duplicate `OnceLock` test override and kept a single env-based override path.           |
| F009 | MEDIUM   | `src/crosshook-native/crates/crosshook-core/src/umu_database/client.rs`   | 271-273 | Fixed  | Replaced predictable temp path write with `tempfile::NamedTempFile::new_in` + `persist`.        |
| F010 | MEDIUM   | `src/crosshook-native/crates/crosshook-core/src/umu_database/client.rs`   | 271-273 | Fixed  | Tempfile flow now keeps secure temporary-file permissions before atomic persist.                |
| F011 | MEDIUM   | `src/crosshook-native/crates/crosshook-core/src/umu_database/coverage.rs` | 15      | Fixed  | Removed blanket `#[allow(dead_code)]`; retained schema-intent comment for serde-only fields.    |
| F012 | MEDIUM   | `src/crosshook-native/crates/crosshook-core/src/umu_database/coverage.rs` | 50-55   | Fixed  | Moved mtime read inside cache mutex critical section to remove race window.                     |
| F013 | MEDIUM   | `src/crosshook-native/crates/crosshook-core/src/umu_database/coverage.rs` | 56      | Fixed  | Replaced panic-on-poison with `unwrap_or_else(                                                  | e   | e.into_inner())`. |
| F014 | MEDIUM   | `src/crosshook-native/crates/crosshook-core/src/umu_database/paths.rs`    | 20-26   | Fixed  | Added guards to skip non-absolute and parent-directory `XDG_DATA_DIRS` entries.                 |
| F015 | MEDIUM   | `src/crosshook-native/src/components/profile-sections/RuntimeSection.tsx` | 64-83   | Fixed  | Extracted IPC/effect logic into reusable `useUmuCoverage` hook.                                 |
| F016 | MEDIUM   | `src/crosshook-native/src/types/launch.ts`                                | 152     | Fixed  | Reused shared `UmuPreference` type for `requested_preference`.                                  |

## Files Changed

- `docs/prps/reviews/pr-264-review.md` (status updates for F007-F016)
- `src/crosshook-native/crates/crosshook-core/Cargo.toml` (promoted `tempfile` dependency for
  runtime use)
- `src/crosshook-native/crates/crosshook-core/src/umu_database/client.rs` (F007-F010)
- `src/crosshook-native/crates/crosshook-core/src/umu_database/coverage.rs` (F011-F013)
- `src/crosshook-native/crates/crosshook-core/src/umu_database/mod.rs` (shared CSV subpath constant
  for F007)
- `src/crosshook-native/crates/crosshook-core/src/umu_database/paths.rs` (F007, F014)
- `src/crosshook-native/src/components/profile-sections/RuntimeSection.tsx` (F015)
- `src/crosshook-native/src/hooks/useUmuCoverage.ts` (F015)
- `src/crosshook-native/src/types/launch.ts` (F016)

## Failed Fixes

None in this run.

## Validation Results

| Check      | Result                                                                                                                                                           |
| ---------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Type check | Pass (`cargo check --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core`) with expected dead-code warnings in `CsvRow` after removing blanket allow |
| Tests      | Pass (`cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core`)                                                                            |

## Next Steps

- Re-run `/code-review 264` to refresh findings for any remaining LOW items.
- If desired, run `/review-fix --parallel --severity low 264` to continue.
- Use `/git-workflow` when you want to commit.
