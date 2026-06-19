# Fix Report: pr-20-review

**Source**: docs/prps/reviews/pr-20-review.md
**Applied**: 2026-06-18
**Mode**: Parallel sub-agents (2 batches, max width 7)
**Severity threshold**: LOW

## Summary

- **Total findings in source**: 18
- **Already processed before this run**:
  - Fixed: 4 (F001–F004)
  - Failed: 0
- **Eligible this run**: 14
- **Applied this run**:
  - Fixed: 14
  - Failed: 0
- **Skipped this run**:
  - Below severity threshold: 0
  - No suggested fix: 0
  - Missing file: 0

## Fixes Applied

| ID   | Severity | File                                 | Line | Status | Notes                                            |
| ---- | -------- | ------------------------------------ | ---- | ------ | ------------------------------------------------ |
| F005 | MEDIUM   | crates/crosshook-core/Cargo.toml     | 21   | Fixed  | Documented `serde_yaml_ng` fork choice           |
| F006 | MEDIUM   | lutris_import/import.rs              | 102  | Fixed  | `apply_lutris_import` now takes `&ProfileStore`  |
| F007 | MEDIUM   | lutris_import/import.rs              | 199  | Fixed  | `MAX_UNIQUE_NAME_ATTEMPTS` constant extracted    |
| F008 | MEDIUM   | lutris_import/map.rs                 | 73   | Fixed  | Removed duplicate `resolve_runner_path` call     |
| F009 | MEDIUM   | lutris_import/map.rs                 | 115  | Fixed  | Uses `METHOD_PROTON_RUN` constant                |
| F010 | MEDIUM   | lutris_import/map.rs                 | 263  | Fixed  | Added `parse_resolution` and gamescope tests     |
| F011 | MEDIUM   | lutris_import/map.rs                 | 334  | Fixed  | `shell-words` tokenizer for quoted args          |
| F012 | MEDIUM   | lutris_import/parse.rs               | 57   | Fixed  | 512 KiB file-size cap before YAML parse          |
| F013 | MEDIUM   | lutris_import/paths.rs               | 79   | Fixed  | `MAX_LUTRIS_LIBRARY_ENTRIES` SQL LIMIT + dir cap |
| F014 | MEDIUM   | lutris_import/tests/paths_tests.rs   | 37   | Fixed  | `HomeEnvGuard` RAII restores env vars            |
| F015 | MEDIUM   | src/hooks/useLutrisImport.ts         | 49   | Fixed  | Removed dead `error`/`clearLutrisError` API      |
| F016 | LOW      | lutris_import/import.rs              | 171  | Fixed  | `to_lowercase()` for Unicode slug consistency    |
| F017 | LOW      | lutris_import/import.rs              | 199  | Fixed  | `{base}-copy` collision guard added              |
| F018 | LOW      | src/components/LutrisImportModal.tsx | 136  | Fixed  | `MODAL_SURFACE_GRID_ROW` constant + comments     |

## Files Changed

- `src/crosshook-native/crates/crosshook-core/Cargo.toml` (F005, F011 — `shell-words` dep)
- `src/crosshook-native/crates/crosshook-core/src/profile/lutris_import/import.rs` (F006, F007, F016, F017)
- `src/crosshook-native/crates/crosshook-core/src/profile/lutris_import/map.rs` (F008, F009, F010, F011)
- `src/crosshook-native/crates/crosshook-core/src/profile/lutris_import/parse.rs` (F012)
- `src/crosshook-native/crates/crosshook-core/src/profile/lutris_import/paths.rs` (F013)
- `src/crosshook-native/crates/crosshook-core/src/profile/lutris_import/tests/paths_tests.rs` (F014)
- `src/crosshook-native/src-tauri/src/commands/lutris.rs` (F006 caller update)
- `src/crosshook-native/src/hooks/useLutrisImport.ts` (F015)
- `src/crosshook-native/src/components/LutrisImportModal.tsx` (F018)

## Failed Fixes

None.

## Validation Results

| Check      | Result                                                                 |
| ---------- | ---------------------------------------------------------------------- |
| Type check | Pass (`cargo check -p crosshook-core`; `npm run typecheck`)            |
| Tests      | Pass (`cargo test -p crosshook-core` — 1330 tests; 23 lutris-specific) |

## Next Steps

- Re-run `/code-review 20` to verify fixes resolved the remaining open findings
- Run `/git-workflow --commit` to commit the changes when satisfied
- Minor cleanup: unused `Path` import warning in `import.rs` (not in review scope)
