# Implementation Report: umu-launcher Migration — Phase 2: Sandbox Allowlist Plumbing

## Summary

Implemented the inert-under-Proton pressure-vessel allowlist plumbing for Phase 2. `collect_pressure_vessel_paths(&LaunchRequest)` now derives a deduplicated ordered path list, both Proton builders export that list under `STEAM_COMPAT_LIBRARY_PATHS` and `PRESSURE_VESSEL_FILESYSTEMS_RW`, Launch Preview renders the same values as `ProtonRuntime` env entries, and the Steam trainer helper clears the paired keys alongside the rest of the Proton/Wine host bleed list.

## Assessment vs Reality

| Metric        | Predicted (Plan) | Actual                    |
| ------------- | ---------------- | ------------------------- |
| Complexity    | Medium           | Medium                    |
| Confidence    | High             | High                      |
| Files Changed | 5                | 8 plus archived plan move |

## Tasks Completed

| #   | Task                                                    | Status          | Notes                                                                                                           |
| --- | ------------------------------------------------------- | --------------- | --------------------------------------------------------------------------------------------------------------- |
| 1.1 | Add `collect_pressure_vessel_paths` helper + unit tests | [done] Complete | Added seven helper tests, including the explicit root-directory edge case                                       |
| 1.2 | Add pressure-vessel keys to `WINE_ENV_VARS_TO_CLEAR`    | [done] Complete | Length assertion updated from 32 to 34                                                                          |
| 1.3 | Shell-helper parity                                     | [done] Complete | Added the two `unset` lines and a targeted ShellCheck suppression for an intentional quoted child-shell command |
| 2.1 | Wire env inserts into Proton builders + tests           | [done] Complete | `build_flatpak_steam_trainer_command` still inherits via delegation only                                        |
| 2.2 | Preview parity + tests                                  | [done] Complete | Preview now surfaces both keys with `EnvVarSource::ProtonRuntime`                                               |
| 3.1 | Full validation gate                                    | [done] Complete | Full core test suite, fmt, clippy, lint, and code review all green                                              |

## Validation Results

| Level           | Status      | Notes                                                                                                                                                                         |
| --------------- | ----------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Static Analysis | [done] Pass | `cargo fmt --manifest-path src/crosshook-native/Cargo.toml --all -- --check`, `cargo clippy --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core -- -D warnings` |
| Unit Tests      | [done] Pass | `cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core`                                                                                                |
| Build           | [done] Pass | `./scripts/lint.sh` runs the repo build/check pipeline successfully                                                                                                           |
| Integration     | [done] Pass | In-repo Rust integration tests included in the full `cargo test` pass                                                                                                         |
| Edge Cases      | [done] Pass | Empty request, SourceDirectory vs CopyToPrefix, `/run/host` normalization, root `/`, and dedup cases covered                                                                  |

## Files Changed

| File                                                                       | Action  | Lines                    |
| -------------------------------------------------------------------------- | ------- | ------------------------ |
| `src/crosshook-native/crates/crosshook-core/src/launch/runtime_helpers.rs` | UPDATED | +169 / -2                |
| `src/crosshook-native/crates/crosshook-core/src/launch/script_runner.rs`   | UPDATED | +165 / -1                |
| `src/crosshook-native/crates/crosshook-core/src/launch/preview.rs`         | UPDATED | +128 / -2                |
| `src/crosshook-native/crates/crosshook-core/src/launch/env.rs`             | UPDATED | +5 / -1                  |
| `src/crosshook-native/runtime-helpers/steam-host-trainer-runner.sh`        | UPDATED | +3 / -0                  |
| `docs/prps/prds/umu-launcher-migration.prd.md`                             | UPDATED | +1 / -1                  |
| `tasks/todo.md`                                                            | UPDATED | tracking surface updated |
| `docs/prps/reports/umu-migration-phase-2-sandbox-allowlist-report.md`      | CREATED | +this file               |

## Deviations from Plan

- Added an explicit root-directory collector test (`/game.exe` -> `["/"]`) because the plan's edge-case checklist called it out and the helper already supported it.
- Added a targeted `# shellcheck disable=SC2016` comment for an intentional single-quoted `bash -c` string so the repo lint gate can pass cleanly.

## Issues Encountered

- `cargo fmt --check` initially failed on newly added test formatting. Resolved by running `cargo fmt`.
- `./scripts/lint.sh` initially failed at ShellCheck because of a pre-existing intentional `SC2016` warning in `steam-host-trainer-runner.sh`. Resolved with an inline suppression comment at the exact command site.

## Tests Written

| Test File                                                                  | Tests   | Coverage                                                                 |
| -------------------------------------------------------------------------- | ------- | ------------------------------------------------------------------------ |
| `src/crosshook-native/crates/crosshook-core/src/launch/runtime_helpers.rs` | 7 tests | collector ordering, dedup, empty paths, flatpak normalization, root path |
| `src/crosshook-native/crates/crosshook-core/src/launch/script_runner.rs`   | 3 tests | builder env insertion and flatpak delegation inheritance                 |
| `src/crosshook-native/crates/crosshook-core/src/launch/preview.rs`         | 2 tests | preview env rendering for SourceDirectory and CopyToPrefix               |

## Next Steps

- [ ] Code review via `$code-review` if a second review artifact is desired
- [ ] Create PR via `$prp-pr`
