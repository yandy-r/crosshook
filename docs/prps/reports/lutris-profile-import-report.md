# Implementation Report: Lutris Profile Import for Migration

## Summary

Implemented a one-way Lutris → CrossHook profile importer: core module (`profile/lutris_import/`) discovers Lutris library roots, reads `pga.db` and per-game YAML, maps Wine/Proton configs to `GameProfile` TOML, exposes preview/apply IPC commands, and adds a Community-route UI with review modal and folder-picker fallback.

## Assessment vs Reality

| Metric        | Predicted (Plan)  | Actual                       |
| ------------- | ----------------- | ---------------------------- |
| Complexity    | Large (10+ files) | Large — 7 batches, ~20 files |
| Confidence    | 9/10              | 9/10 — matched plan closely  |
| Files Changed | ~16               | ~20 (13 new module files)    |

## Tasks Completed

| #   | Task                     | Status   | Notes                           |
| --- | ------------------------ | -------- | ------------------------------- |
| 1.1 | YAML dependency          | Complete | `serde_yaml_ng = "0.10"`        |
| 1.2 | Module skeleton          | Complete | Full `mod.rs` re-exports        |
| 1.3 | Error + DTO types        | Complete |                                 |
| 1.4 | SyncSource::LutrisImport | Complete | No DB migration                 |
| 2.1 | YAML parse structs       | Complete | 2 unit tests                    |
| 2.2 | Paths + pga.db           | Complete | 7 unit tests                    |
| 2.3 | Runner classification    | Complete | 2 unit tests                    |
| 3.1 | Map → GameProfile        | Complete | 4 unit tests in `map.rs`        |
| 4.1 | Import orchestration     | Complete | Preview read-only + apply batch |
| 5.1 | Tauri commands           | Complete | SAVE_THEN_SYNC metadata         |
| 5.2 | Register + contract test | Complete |                                 |
| 6.1 | Frontend hook            | Complete | `useLutrisImport.ts`            |
| 6.2 | Import modal             | Complete | Checkbox review + result phase  |
| 6.3 | Community entry point    | Complete | Auto-detect + folder picker     |
| 7.1 | Tests, lint, validation  | Complete | 18 lutris_import tests          |

## Validation Results

| Level           | Status | Notes                                     |
| --------------- | ------ | ----------------------------------------- |
| Static Analysis | Pass   | clippy -D warnings, npm typecheck         |
| Unit Tests      | Pass   | 18 lutris_import + 1307 core tests        |
| Build           | Pass   | crosshook-core + crosshook-native compile |
| Integration     | N/A    | Manual Lutris host validation deferred    |
| Edge Cases      | Pass   | Covered in unit tests per plan checklist  |

## Files Changed

| File                                                    | Action             |
| ------------------------------------------------------- | ------------------ |
| `crates/crosshook-core/src/profile/lutris_import/*`     | CREATED (13 files) |
| `crates/crosshook-core/Cargo.toml`                      | UPDATED            |
| `crates/crosshook-core/src/metadata/models.rs`          | UPDATED            |
| `crates/crosshook-core/src/profile/mod.rs`              | UPDATED            |
| `src-tauri/src/commands/lutris.rs`                      | CREATED            |
| `src-tauri/src/commands/mod.rs`, `lib.rs`, `export.rs`  | UPDATED            |
| `src/hooks/useLutrisImport.ts`                          | CREATED            |
| `src/components/LutrisImportModal.tsx`                  | CREATED            |
| `src/components/CommunityBrowser.tsx`                   | UPDATED            |
| `src/components/community/CommunityProfilesSection.tsx` | UPDATED            |
| `src/lib/mocks/handlers/lutris.ts`                      | CREATED            |

## Deviations from Plan

- **`preview_lutris_import` signature**: Added `profile_store: Option<&ProfileStore>` parameter (plan implied via IPC) for name-collision checks during preview.
- **`emit_profiles_changed` visibility**: Used `pub(crate) use` re-export from `commands/profile/mod.rs` instead of making `shared.rs` helper fully public.
- **Map tests location**: Inline `#[cfg(test)]` in `map.rs` plus dedicated test modules under `tests/` (plan allowed either).

## Issues Encountered

- `LaunchCommandArgumentsSection` and `models::gamescope` are not public re-exports — resolved by using `GamescopeConfig`/`GamescopeFilter` from `crate::profile` and mutating `launch.command_arguments.custom_args` on a default `LaunchSection`.
- Parallel B5 agent initially blocked on map.rs compile errors — fixed before final validation.

## Tests Written

| Test File / Module      | Tests | Coverage                     |
| ----------------------- | ----- | ---------------------------- |
| `tests/parse_tests.rs`  | 2     | YAML full + minimal          |
| `tests/paths_tests.rs`  | 7     | Discovery, glob, pga.db      |
| `tests/runner_tests.rs` | 2     | Proton classify, missing dir |
| `map.rs` (inline)       | 4     | esync, env, missing exe      |
| `import.rs` (inline)    | 2     | preview read-only, apply     |
| `tests/sync_source.rs`  | 1     | LutrisImport as_str          |

## Next Steps

- [ ] Code review via `/code-review`
- [ ] Create PR via `/prp-pr` with `Closes #2`
- [ ] Manual validation on a host with Lutris installed
