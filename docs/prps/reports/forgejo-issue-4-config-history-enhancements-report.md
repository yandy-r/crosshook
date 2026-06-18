# Implementation Report: Config history enhancements (Forgejo #4)

## Summary

Implemented all four vertical slices from the plan: migrated unified diffing to the `similar` crate in `crosshook-core`, added semantic TOML diff mode with IPC + UI toggle, exposed a user-configurable per-profile revision retention cap in Settings, and shipped collapse-unchanged-hunks UX for unified diffs. No SQLite migration was required; retention is driven by `[config_history] max_revisions` in `settings.toml`.

## Assessment vs Reality

| Metric        | Predicted (Plan) | Actual                 |
| ------------- | ---------------- | ---------------------- |
| Complexity    | Medium           | Medium                 |
| Confidence    | High             | High                   |
| Files Changed | ~15–20           | 36 (4 new, 32 updated) |

## Tasks Completed

| #   | Task                       | Status      | Notes                                      |
| --- | -------------------------- | ----------- | ------------------------------------------ |
| 1   | `similar` crate migration  | ✅ Complete | `config_diff.rs` in core; tauri thin layer |
| 2   | Semantic TOML diff         | ✅ Complete | `config_semantic_diff.rs` + UI toggle      |
| 3   | Retention configuration UI | ✅ Complete | Settings + prune param plumbed             |
| 4   | Collapse unchanged hunks   | ✅ Complete | `helpers.ts` + RevisionDetail toggle       |

## Validation Results

| Level           | Status  | Notes                                              |
| --------------- | ------- | -------------------------------------------------- |
| Static Analysis | ✅ Pass | `./scripts/lint.sh` (rustfmt, clippy, biome, tsc)  |
| Unit Tests      | ✅ Pass | `cargo test -p crosshook-core`; Vitest helpers (2) |
| Build           | ✅ Pass | `cargo check -p crosshook-native`                  |
| Integration     | ✅ Pass | Existing config history integration tests          |
| Edge Cases      | ✅ Pass | Semantic reorder test; custom prune limit test     |

## Files Changed

| File                                                        | Action  | Notes                               |
| ----------------------------------------------------------- | ------- | ----------------------------------- |
| `crates/crosshook-core/src/profile/config_diff.rs`          | CREATED | `similar`-based unified diff        |
| `crates/crosshook-core/src/profile/config_semantic_diff.rs` | CREATED | TOML tree semantic diff             |
| `src/components/config-history/SemanticDiffView.tsx`        | CREATED | Semantic diff UI                    |
| `src/components/config-history/__tests__/helpers.test.ts`   | CREATED | Collapse helper tests               |
| ~32 other paths                                             | UPDATED | IPC, settings, mocks, capture paths |

## Deviations from Plan

None — all four slices implemented as specified. Semantic parse failures fall back to unified diff with an inline notice (per plan degraded fallback).

## Issues Encountered

- Clippy `wildcard_in_or_patterns` in semantic diff match — resolved by simplifying the non-table arm.
- Integration test sed accidentally added `max_revisions` to `observe_profile_write` — corrected.

## Tests Written

| Test File                        | Tests | Coverage                                        |
| -------------------------------- | ----- | ----------------------------------------------- |
| `config_diff.rs` (unit)          | 4     | Empty diff, line counts, truncation, byte cap   |
| `config_semantic_diff.rs` (unit) | 4     | Reorder, scalar change, section add, parse fail |
| `pruning.rs`                     | +1    | Custom `max_revisions` limit                    |
| `settings/tests.rs`              | +1    | Default + clamp                                 |
| `helpers.test.ts`                | 2     | Collapse toggle behavior                        |

## Next Steps

- [x] Code review via `/code-review`
- [x] PR [#17](https://git.home.rfamily.dev/yandy/crosshook/pull/17) merged (`Part of #4`)
- [x] Close-out: ROADMAP + Forgejo #3 tracker reconciled; Forgejo #4 closed; HMAC deferred to follow-up issue
