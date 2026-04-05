# Implementation Report: Network Isolation for Trainers via `unshare --net`

## Summary

Added per-profile `network_isolation` toggle (default: `true`) to `[launch]` section of profile TOML. When enabled, `unshare --net` is prepended to the trainer launch command, creating an isolated network namespace. Degrades gracefully with a warning when `unshare --net` is unavailable.

## Assessment vs Reality

| Metric        | Predicted (Plan) | Actual |
| ------------- | ---------------- | ------ |
| Complexity    | Medium           | Medium |
| Confidence    | High             | High   |
| Files Changed | 10-12            | 7      |

## Tasks Completed

| #   | Task                                                   | Status   | Notes                                                 |
| --- | ------------------------------------------------------ | -------- | ----------------------------------------------------- |
| 1   | Add `network_isolation` field to `LaunchSection`       | Complete | Manual `Default` impl for `true` default              |
| 2   | Add `network_isolation` field to `LaunchRequest`       | Complete |                                                       |
| 3   | Add `is_unshare_net_available()` capability check      | Complete | Probes via `unshare --net true`                       |
| 4   | Add `UnshareNetUnavailable` validation warning         | Complete | Warning severity in both steam and proton paths       |
| 5   | Prepend `unshare --net` to trainer wrapper chain       | Complete | Both `proton_run` and `steam_applaunch` trainer paths |
| 6   | Include network isolation in launch preview            | Complete | Preview mirrors runtime behavior for trainer-only     |
| 7   | Update frontend TypeScript types                       | Complete | Added field + default `true`                          |
| 8   | Add unit tests for profile TOML backward compatibility | Complete | 4 tests                                               |
| 9   | Add unit tests for trainer command wrapper chain       | Complete | 4 tests (3 script_runner + 1 validation)              |

## Validation Results

| Level           | Status | Notes                                             |
| --------------- | ------ | ------------------------------------------------- |
| Static Analysis | Pass   | `cargo check` — zero errors across full workspace |
| Unit Tests      | Pass   | 587 tests (584 core + 3 integration), 8 new       |
| Build           | Pass   | Full workspace compiles clean                     |
| Integration     | N/A    | No integration test framework                     |
| Edge Cases      | Pass   | Covered via tests                                 |

## Files Changed

| File                                                  | Action  | Lines                                                                  |
| ----------------------------------------------------- | ------- | ---------------------------------------------------------------------- |
| `crates/crosshook-core/src/profile/models.rs`         | UPDATED | +60 (field, Default impl, serde helpers, 4 tests)                      |
| `crates/crosshook-core/src/launch/request.rs`         | UPDATED | +30 (field, variant, message/help/severity, validation checks, 1 test) |
| `crates/crosshook-core/src/launch/runtime_helpers.rs` | UPDATED | +14 (`is_unshare_net_available()`)                                     |
| `crates/crosshook-core/src/launch/script_runner.rs`   | UPDATED | +120 (wrapper prepend logic, 3 tests)                                  |
| `crates/crosshook-core/src/launch/preview.rs`         | UPDATED | +12 (effective_wrappers for trainer preview)                           |
| `crates/crosshook-cli/src/main.rs`                    | UPDATED | +1 (`network_isolation` field in request)                              |
| `src/types/profile.ts`                                | UPDATED | +3 (field + default)                                                   |

## Deviations from Plan

- Plan listed `launch/optimizations.rs` and `launch/mod.rs` as potential changes — not needed. The `unshare --net` prepend is handled entirely within `script_runner.rs` for `proton_run` and within `build_trainer_command` for `steam_applaunch`.
- Plan estimated 10-12 files; actual was 7 files (simpler than anticipated).

## Issues Encountered

- `crosshook-cli` crate explicitly constructs `LaunchRequest` and did not compile after adding the new field. Fixed by adding `network_isolation: profile.launch.network_isolation` to the constructor.

## Tests Written

| Test File                 | Tests   | Coverage                                                   |
| ------------------------- | ------- | ---------------------------------------------------------- |
| `profile/models.rs`       | 4 tests | TOML backward compat, roundtrip, Default impl              |
| `launch/script_runner.rs` | 3 tests | unshare prepend (enabled/disabled), game command exclusion |
| `launch/request.rs`       | 1 test  | UnshareNetUnavailable warning severity                     |

## Next Steps

- [ ] Code review via `/code-review`
- [ ] Create PR via `/prp-pr`
