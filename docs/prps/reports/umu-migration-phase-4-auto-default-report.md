# Implementation Report: umu-launcher Migration — Phase 4 (Auto default + exported-script parity)

## Summary

Phase 4 of the umu-launcher migration: `UmuPreference::Auto` now prefers `umu-run` when available (previously Auto behaved identically to `Proton`), and `build_exec_line` in `export/launcher.rs` emits a runtime `command -v umu-run` probe so exported trainer scripts pick up umu on hosts that have it while remaining shareable with hosts that do not. Steam-applaunch and Flatpak-Steam-trainer paths remain force-opted-out via the existing `force_no_umu_for_launch_request` predicate; no new dispatch method, no schema change, no settings migration.

## Assessment vs Reality

| Metric        | Predicted (Plan) | Actual                                                                                                                                       |
| ------------- | ---------------- | -------------------------------------------------------------------------------------------------------------------------------------------- |
| Complexity    | Medium (6 files) | Medium (6 production files + 1 script_runner.rs legacy-test sweep pinning 16 tests to `UmuPreference::Proton` to maintain PATH-independence) |
| Confidence    | 9/10             | 9/10 — no architectural surprises; the only deviation was mechanical legacy-test pinning to prevent PATH-dependent flakes                    |
| Files Changed | 6                | 8 tracked paths: 6 production sources + PRD row update + Cargo.lock sync (see **Files Changed** table below)                                 |

## Tasks Completed

| #             | Task                                                                  | Status     | Notes                                                                                                      |
| ------------- | --------------------------------------------------------------------- | ---------- | ---------------------------------------------------------------------------------------------------------- |
| 1.1           | Flip `should_use_umu` Auto semantics                                  | ✓ Complete | Match on `UmuPreference` — `Proton` → direct Proton; `Umu \| Auto` → resolve umu path                      |
| 1.2           | Rewrite Auto reason-string arm in `build_umu_decision_preview`        | ✓ Complete | `(Auto, _, false)` arm now explains the umu-missing fallback; `(_, _, true)` arm folds Auto naturally      |
| 1.3           | Refresh `LaunchRequest.umu_preference` doc-comment                    | ✓ Complete | One-line rustdoc update                                                                                    |
| 1.4           | Introduce umu runtime probe in `build_exec_line`                      | ✓ Complete | `_UMU_AVAILABLE` probe + single if/else dual-exec block; pre-computed prefix string across 4 branches      |
| 1.5           | Update Settings UI `Auto` label                                       | ✓ Complete | "Auto (Phase 3 → Proton)" → "Auto (umu when available, else Proton)"                                       |
| 1.6           | Update TS type doc-comment for `UmuPreference`                        | ✓ Complete | `settings.ts` updated; `profile.ts` / `launch.ts` had no stale Phase-3-specific copy                       |
| 2.1           | Update Rust tests in `script_runner.rs` for Auto semantics            | ✓ Complete | Deleted `auto_preference_resolves_to_proton_in_phase_3`; added 4 new Auto tests (2 game + 2 trainer)       |
| 2.2           | Update preview tests for new Auto reason strings                      | ✓ Complete | 2 new Auto preview tests added; no stale "Phase 3" test found to delete                                    |
| 2.3           | Update exported-script tests for dual-branch probe                    | ✓ Complete | 6 sibling umu-branch assertions added; `network_isolation_enabled_*` extended; standalone probe test added |
| 3.1           | Full-repo verification                                                | ✓ Complete | lint.sh green (after single rustfmt auto-fix); full workspace `cargo test` + `npm run build` green         |
| **Deviation** | **Pin 16 legacy `script_runner.rs` tests to `UmuPreference::Proton`** | ✓ Complete | Not in original plan — see **Deviations from Plan** below.                                                 |

## Validation Results

| Level                 | Status | Notes                                                                                                                                                                                                                             |
| --------------------- | ------ | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Level 1 — Static      | ✓ Pass | `cargo fmt` (auto-fixed 1 line), `cargo clippy -D warnings`, Biome, `tsc --noEmit`, `shellcheck`                                                                                                                                  |
| Level 2 — Unit tests  | ✓ Pass | crosshook-core: all green (**+7** new `#[test]` entries: 4 in `script_runner`, 2 in `preview`, 1 in `export/launcher`; amended sibling assertions in `export/launcher` are not additional tests); full workspace test suite green |
| Level 3 — Build       | ✓ Pass | `npm run build` (frontend) succeeded; `cargo check` (backend) succeeded                                                                                                                                                           |
| Level 4 — Integration | N/A    | No integration harness for this change; covered by plan's "manual validation" checklist (deferred)                                                                                                                                |
| Level 5 — Edge cases  | ✓ Pass | All 11 edge-case scenarios from plan's Testing Strategy exercised by new tests                                                                                                                                                    |

## Files Changed

| File                                                                     | Action | Lines                                                                              |
| ------------------------------------------------------------------------ | ------ | ---------------------------------------------------------------------------------- |
| `src/crosshook-native/crates/crosshook-core/src/launch/script_runner.rs` | UPDATE | +~160 / -15                                                                        |
| `src/crosshook-native/crates/crosshook-core/src/launch/preview.rs`       | UPDATE | +~45 / -1                                                                          |
| `src/crosshook-native/crates/crosshook-core/src/launch/request.rs`       | UPDATE | +1 / -1                                                                            |
| `src/crosshook-native/crates/crosshook-core/src/export/launcher.rs`      | UPDATE | +~75 / -3                                                                          |
| `src/crosshook-native/src/components/SettingsPanel.tsx`                  | UPDATE | +1 / -1                                                                            |
| `src/crosshook-native/src/types/settings.ts`                             | UPDATE | +1 / -1                                                                            |
| `docs/prps/prds/umu-launcher-migration.prd.md`                           | UPDATE | Phase matrix: 3 / 3b marked `complete` + report links (Depends align with 4)       |
| `src/crosshook-native/Cargo.lock`                                        | UPDATE | +3 / -3 (unrelated 0.2.10-flatpak → 0.2.11 version sync from prior release commit) |

Total: **8** tracked paths in the table above — **6** production source files (matches plan) plus **2** ancillary (`docs/.../prd.md`, `Cargo.lock`). Cargo.lock churn is a stale-lockfile fix incidental to this work.

## Deviations from Plan

### Deviation 1: PATH-dependence sweep in `script_runner.rs` legacy tests

**What**: 16 pre-existing tests in `script_runner.rs` that construct `LaunchRequest` via `..Default::default()` (thus inheriting `UmuPreference::Auto`) were pinned to `UmuPreference::Proton` with a short comment explaining the Phase 4 context. Affected tests:

- `proton_game_command_sets_proton_verb_to_waitforexitandrun` (misnamed; asserts `PROTON_VERB == None`)
- `proton_trainer_command_sets_proton_verb_to_runinprefix` (similar name/assert mismatch)
- `proton_trainer_command_stages_support_files_into_prefix`
- `proton_trainer_command_uses_source_directory_without_staging`
- `proton_trainer_command_prefers_enabled_trainer_gamescope`
- `proton_game_command_applies_optimization_wrappers_and_env`
- `proton_game_custom_env_overrides_duplicate_optimization_key`
- `proton_game_command_sets_pressure_vessel_paths_from_request`
- `proton_trainer_command_ignores_game_optimization_wrappers_and_env`
- `proton_trainer_command_sets_pressure_vessel_paths_skipping_copy_to_prefix_trainer_dir`
- `flatpak_steam_trainer_command_inherits_proton_verb_runinprefix`
- `proton_game_command_sets_compat_data_path_for_standalone_prefixes`
- `proton_trainer_command_uses_pfx_child_when_prefix_path_is_compatdata_root`
- `proton_trainer_command_prepends_unshare_net_when_isolation_enabled`
- `proton_trainer_command_skips_unshare_when_isolation_disabled`
- `proton_game_command_does_not_include_unshare_net`

**Why**: These tests were written under the Phase 3 assumption that `Auto` behaves like `Proton`, making them implicitly PATH-independent. Phase 4 flips Auto semantics so `Auto` + host with `umu-run` on PATH → umu branch (different `program`, different env). On a dev/CI host that has `umu-run` installed, these tests would fail in opaque ways (program becomes `umu-run`, `PROTON_VERB` is injected, argv loses `run` subcommand). Pinning them to `UmuPreference::Proton` preserves their original invariants and makes them PATH-independent — the tests now explicitly verify the direct-Proton path, which matches their actual intent.

This deviation was not anticipated in the plan's Testing Strategy. The plan's "GOTCHA (a)" on Task 2.1 — "Do NOT touch the 4 existing UmuPreference::Umu tests" — covered the umu-specific tests but didn't call out the implicit Auto-default dependency in unrelated Phase-1/Phase-2 tests. Future phase plans should add an explicit "audit `..Default::default()` call sites for PATH-dependence" step whenever Auto semantics change.

### Deviation 2: Test naming quirk left in place

Two pre-existing tests have misleading names that assert the opposite of what the name suggests:

- `proton_game_command_sets_proton_verb_to_waitforexitandrun` asserts `PROTON_VERB == None` on the direct-Proton path.
- `proton_trainer_command_sets_proton_verb_to_runinprefix` asserts `PROTON_VERB == None` on the direct-Proton path.

Both were renamed-by-intent as "ensure direct-Proton doesn't leak a PROTON_VERB" but kept the umu-era names. I did **not** rename them — renaming touches test identifiers in CI logs and is out of scope for Phase 4. Filed mentally as a follow-up hygiene task.

## Issues Encountered

1. **Stale Cargo.lock**: `cargo check` auto-bumped `0.2.10-flatpak → 0.2.11` entries in `Cargo.lock` (the repo's `Cargo.toml` is at 0.2.11 from commit `b26b842 chore(release): prepare v0.2.11`, but Cargo.lock was not re-generated in that release commit). The Cargo.lock diff is a drive-by fix unrelated to Phase 4 — left in place.
2. **rustfmt drift** on one `assert!` line in `export/launcher.rs` (line 1197) caused by Task 2.3's sibling-assertion addition. Auto-fixed by `cargo fmt --all`; no clippy or semantic impact.
3. **One teammate failure mode not seen** (two-researcher failure): all 9 implementor teammates (6 in B1, 3 in B2) completed successfully on first dispatch; no mid-batch retries needed.

## Tests Written

| Test File                                                                               | Tests Added                                                                                                                                                                                                                       | Coverage                                                                       |
| --------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------ |
| `src/crosshook-native/crates/crosshook-core/src/launch/script_runner.rs` (tests module) | 4 new: `auto_preference_uses_umu_when_umu_run_present`, `auto_preference_falls_back_to_proton_when_umu_run_missing`, `auto_preference_uses_umu_trainer_when_present`, `auto_preference_trainer_falls_back_to_proton_when_missing` | Phase 4 Auto-flip game + trainer matrix                                        |
| `src/crosshook-native/crates/crosshook-core/src/launch/preview.rs` (tests module)       | 2 new: `auto_preference_preview_reports_using_umu_when_umu_run_present`, `auto_preference_preview_explains_fallback_when_umu_missing`                                                                                             | Preview `umu_decision.reason` for both Auto paths                              |
| `src/crosshook-native/crates/crosshook-core/src/export/launcher.rs` (tests module)      | 1 new: `build_exec_line_emits_umu_probe_and_dual_exec`; 7 sibling umu-branch assertions added to existing tests (6 call sites + 1 network_isolation test extension)                                                               | Exported-script `command -v umu-run` probe + dual-branch exec; 4 matrix combos |

Total new tests: 7; total amended tests: 7 (6 sibling assertions + 1 extension). All passing.

## Next Steps

- [ ] `/ycc:code-review` to review changes before committing
- [ ] `/ycc:prp-commit` with a `feat(launch): enable umu-launcher by default for non-Steam launches (Phase 4)` title — this phrasing surfaces in `CHANGELOG.md`'s `### Features` section via git-cliff per `.git-cliff.toml:51`
- [ ] `/ycc:prp-pr` to open a pull request; link to tracker issue #257 and implementation issue #239
- [ ] Per PRD prerequisite: confirm 2-week observation window after #263 / #247 landed before merging (qualitative, not gated by this report)
- [ ] Optional manual validation: export a trainer script on a host with `umu-run` installed; run the exported script on a host WITHOUT `umu-run` → confirm `"$PROTON" run` fallback still works (cross-host shareability check from plan's Manual Validation section)
- [ ] Follow-up hygiene: consider renaming the two misleading test functions (`proton_game_command_sets_proton_verb_to_waitforexitandrun`, `proton_trainer_command_sets_proton_verb_to_runinprefix`) to reflect that they assert the opposite — not part of Phase 4 scope.
