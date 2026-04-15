# Implementation Report: umu-launcher Migration — Phase 3: umu Opt-In (Non-Steam Only)

## Summary

Implemented the first real `umu-run` code path. Added `UmuPreference { Auto, Umu, Proton }` (default `Auto`, which still resolves to direct Proton in Phase 3) plus per-profile `runtime.umu_game_id`. When `UmuPreference::Umu` is set AND `umu-run` is on `PATH`, the non-Steam game and trainer builders now emit `umu-run <target>` with `PROTONPATH = dirname(proton_path)` and the existing `GAMEID` / `PROTON_VERB` / pressure-vessel allowlist active. Steam-context trainers explicitly opt out via a private `build_proton_trainer_command_with_umu_override(.., force_no_umu=true)` variant routed through `build_flatpak_steam_trainer_command`. Preview mirrors builder output exactly (command string + env + `ProtonSetup.umu_run_path`). Degraded fallback (`Umu` requested but `umu-run` missing) emits `tracing::warn!` and continues with direct Proton.

## Persistence and Usability

| Class                                | Data                                                                                                                                            | Migration / back-compat                                                                         | Offline                                                                      | Degraded fallback                                                                                                                                                                             | View / edit                                                                               |
| ------------------------------------ | ----------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------- |
| **User-editable preferences (TOML)** | `UmuPreference` in `AppSettingsData`; per-profile `runtime.umu_game_id`                                                                         | New fields default safely (`Auto`, empty `umu_game_id`); older configs deserialize without loss | Settings and profiles load from disk; no network required for umu preference | `Umu` requested but `umu-run` missing → `tracing::warn!` then direct Proton (`build_proton_trainer_command_with_umu_override` / `force_no_umu=true` on Steam keeps trainers on direct Proton) | **Settings → Umu preference** dropdown; **profile runtime** → `umu_game_id` where exposed |
| **Operational / metadata (SQLite)**  | Preview-only strings (`effective_command`, env list including `PROTONPATH`); `ProtonSetup.umu_run_path` in preview payload when umu is selected | Preview is derived; not a migration surface                                                     | Preview is local                                                             | Same as launch: missing `umu-run` reflected in preview reason and direct-Proton command                                                                                                       | Preview modal shows resolved command and umu decision; not stored as history              |
| **Ephemeral runtime**                | In-process choice to exec `umu-run` vs direct Proton; transient env (`PROTONPATH`, `GAMEID`, `PROTON_VERB`, etc.)                               | N/A                                                                                             | Launch uses the same resolution as online                                    | `Umu` + missing `umu-run` → warn + direct Proton                                                                                                                                              | Not persisted; visible only via logs                                                      |

**Steam opt-out:** `build_flatpak_steam_trainer_command` forces `force_no_umu=true`, so Steam-context trainers never use umu; non-Steam trainers use `build_proton_trainer_command_with_umu_override` with `force_no_umu=false` unless the Steam delegation path is taken.

## Assessment vs Reality

| Metric        | Predicted (Plan) | Actual                    |
| ------------- | ---------------- | ------------------------- |
| Complexity    | Large            | Large                     |
| Confidence    | High             | High                      |
| Files Changed | ~25              | 32 (Rust + TS + sh)       |
| Tests Added   | ~22              | 22 (unit + 1 integration) |

## Tasks Completed

| Batch | #   | Task                                                                                                          | Status   |
| ----- | --- | ------------------------------------------------------------------------------------------------------------- | -------- |
| B1    | 1.1 | `UmuPreference` enum + `umu_preference` field in `AppSettingsData`                                            | Complete |
| B1    | 1.2 | Add `umu_game_id: String` to `RuntimeSection` (+ collateral struct-literal fixes across 9 files)              | Complete |
| B1    | 1.3 | Add `umu_game_id` to `RuntimeLaunchConfig` + `umu_preference` to `LaunchRequest`                              | Complete |
| B1    | 1.4 | Add `PROTONPATH` to `WINE_ENV_VARS_TO_CLEAR` (34 → 35) + shell parity                                         | Complete |
| B2    | 2.1 | Tauri `SettingsSaveRequest` + `merge_settings_from_request`                                                   | Complete |
| B2    | 2.2 | TS `UmuPreference` type + `AppSettingsData` / `SettingsSaveRequest` / defaults                                | Complete |
| B2    | 2.3 | TS `LaunchRequest.runtime` + profile runtime extension                                                        | Complete |
| B2    | 2.4 | `buildProfileLaunchRequest` + caller updates                                                                  | Complete |
| B2    | 2.5 | CLI `launch_request_from_profile` + caller wiring                                                             | Complete |
| B2    | 2.6 | `SettingsPanel.tsx` umu-preference dropdown                                                                   | Complete |
| B3    | 3.1 | Precedence (`runtime.umu_game_id → steam.app_id → runtime.steam_app_id → "umu-0"`) + 4 assertion updates      | Complete |
| B4    | 4.1 | `should_use_umu` helper + game builder umu branch + `use_umu` parameter on 3 helpers (4 `.arg("run")` guards) | Complete |
| B5    | 5.1 | Trainer builder umu branch + private `_with_umu_override` variant + flatpak-Steam Steam opt-out               | Complete |
| B6    | 6.1 | Preview parity (command string branch, `PROTONPATH` env push, `ProtonSetup.umu_run_path` tightened)           | Complete |
| B7    | 7.1 | `resolve_umu_run_path` unit tests (3)                                                                         | Complete |
| B7    | 7.2 | `tests/umu_concurrent_pids.rs` integration test (PR #148 non-regression)                                      | Complete |
| B8    | 8.1 | Validation gate + PRD already current (Phase 3 row + #243 footnote landed in plan-creation commit)            | Complete |

## Validation Results

| Level           | Status | Notes                                                                                                                                                                                                         |
| --------------- | ------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Static Analysis | Pass   | `cargo fmt --all -- --check`, `cargo clippy --workspace --all-targets -- -D warnings` (after fixing 6 lint issues introduced during implementation: 2 uninlined format args + 4 non-binding `let _ = future`) |
| Unit Tests      | Pass   | `cargo test --workspace` — 933 tests pass (891 lib + 30 preview-style integration + 7 cli + others)                                                                                                           |
| Build           | Pass   | `./scripts/lint.sh` exits 0 (rustfmt + clippy + biome + tsc + shellcheck)                                                                                                                                     |
| Integration     | Pass   | `cargo test -p crosshook-core --test umu_concurrent_pids` — 1 test, both stub PIDs alive at t+500ms, ~0.5s wall-time                                                                                          |
| Edge Cases      | Pass   | Empty PATH, non-executable umu-run, Steam-context delegation, `Auto`/`Proton` preference, missing umu-run + warn fallback, precedence over Steam app_id                                                       |

## Files Changed

### Rust core (`crates/crosshook-core/`)

| File                                                                                                                                                                                                                                                | Action | Notes                                                                                                                                                                                                                                                                                                                                                           |
| --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ------ | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `src/settings/mod.rs`                                                                                                                                                                                                                               | UPDATE | Add `UmuPreference` enum + `umu_preference` field to `AppSettingsData` (Default + manual Debug + 3 tests)                                                                                                                                                                                                                                                       |
| `src/profile/models.rs`                                                                                                                                                                                                                             | UPDATE | Add `umu_game_id: String` to `RuntimeSection` + `is_empty()` + 2 tests                                                                                                                                                                                                                                                                                          |
| `src/launch/request.rs`                                                                                                                                                                                                                             | UPDATE | Add `umu_game_id` to `RuntimeLaunchConfig` + `umu_preference: UmuPreference` to `LaunchRequest` + 2 tests                                                                                                                                                                                                                                                       |
| `src/launch/env.rs`                                                                                                                                                                                                                                 | UPDATE | Add `"PROTONPATH"` to `WINE_ENV_VARS_TO_CLEAR`; assertion 34 → 35                                                                                                                                                                                                                                                                                               |
| `src/launch/script_runner.rs`                                                                                                                                                                                                                       | UPDATE | `should_use_umu`, `proton_path_dirname`, `warn_on_umu_fallback` helpers; game builder umu branch; `build_proton_trainer_command_with_umu_override` private variant; flatpak-Steam delegation forces `force_no_umu=true`; precedence change in `resolve_steam_app_id_for_umu`; `"umu-0"` fallback; +10 new tests; 4 GAMEID assertion updates (`"0"` → `"umu-0"`) |
| `src/launch/preview.rs`                                                                                                                                                                                                                             | UPDATE | `build_effective_command_string` branches on `should_use_umu`; `collect_runtime_proton_environment` pushes `PROTONPATH`; `build_proton_setup` tightens `umu_run_path` semantics; +4 new tests                                                                                                                                                                   |
| `src/launch/runtime_helpers.rs`                                                                                                                                                                                                                     | UPDATE | Game/trainer helpers gain `use_umu: bool` (4 `.arg("run")` guards); +3 unit tests for `resolve_umu_run_path`                                                                                                                                                                                                                                                    |
| `src/launch/optimizations.rs`                                                                                                                                                                                                                       | UPDATE | `#[cfg(test)] resolve_umu_run_path_for_test()` hook for `ScopedCommandSearchPath`-driven tests                                                                                                                                                                                                                                                                  |
| `src/install/models.rs`, `src/install/service.rs`, `src/update/service.rs`, `src/run_executable/service.rs`, `src/metadata/mod.rs`, `src/profile/exchange.rs`, `src/profile/health.rs`, `src/profile/toml_store.rs`, `src/export/launcher_store.rs` | UPDATE | Collateral: extend `RuntimeSection`/`RuntimeLaunchConfig` struct literals with `umu_game_id: String::new()`; pass `false` for new `use_umu` parameter at 3 call sites                                                                                                                                                                                           |
| `tests/umu_concurrent_pids.rs`                                                                                                                                                                                                                      | CREATE | PR #148 non-regression integration smoke (stub `umu-run` + concurrent game/trainer spawn)                                                                                                                                                                                                                                                                       |

### Rust src-tauri / CLI

| File                                 | Action | Notes                                                                                                                                                  |
| ------------------------------------ | ------ | ------------------------------------------------------------------------------------------------------------------------------------------------------ |
| `src-tauri/src/commands/settings.rs` | UPDATE | `umu_preference: Option<UmuPreference>` on `SettingsSaveRequest` + merge through `merge_settings_from_request`                                         |
| `crates/crosshook-cli/src/main.rs`   | UPDATE | `launch_request_from_profile` signature gains `umu_preference: UmuPreference`; loads `AppSettingsData` at the call site; threads `runtime.umu_game_id` |

### Frontend (`src/crosshook-native/src/`)

| File                              | Action | Notes                                                                                                                                                                                                          |
| --------------------------------- | ------ | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `types/settings.ts`               | UPDATE | `export type UmuPreference = 'auto' \| 'umu' \| 'proton'`; `umu_preference` on `SettingsSaveRequest` + `AppSettingsData`; default `'auto'` in `DEFAULT_APP_SETTINGS`; threaded through `toSettingsSaveRequest` |
| `types/launch.ts`                 | UPDATE | `runtime.steam_app_id?` (latent bug fix), `runtime.umu_game_id?`, top-level `umu_preference?: UmuPreference`                                                                                                   |
| `types/profile.ts`                | UPDATE | `runtime.umu_game_id?: string` + default in `DEFAULT_RUNTIME_SECTION`                                                                                                                                          |
| `utils/launch.ts`                 | UPDATE | `buildProfileLaunchRequest` accepts `umuPreference: UmuPreference` arg; threads `steam_app_id` / `umu_game_id` / top-level `umu_preference`                                                                    |
| `context/LaunchStateContext.tsx`  | UPDATE | Pass `settings.umu_preference` to `buildProfileLaunchRequest`                                                                                                                                                  |
| `components/pages/LaunchPage.tsx` | UPDATE | Pass `settings.umu_preference` to `buildProfileLaunchRequest`                                                                                                                                                  |
| `components/SettingsPanel.tsx`    | UPDATE | Add `<select>` dropdown for `umu_preference` (Auto / Umu / Proton) adjacent to `default_launch_method`                                                                                                         |

### Shell

| File                                           | Action | Notes                                                                           |
| ---------------------------------------------- | ------ | ------------------------------------------------------------------------------- |
| `runtime-helpers/steam-host-trainer-runner.sh` | UPDATE | Add `unset PROTONPATH` to parity unset block (mirrors `WINE_ENV_VARS_TO_CLEAR`) |

## Tests Written

| Area                           | New Tests | Coverage                                                                                                                                                                                                                              |
| ------------------------------ | --------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `settings/mod.rs`              | 3         | UmuPreference backward-compat, roundtrip, FromStr error path                                                                                                                                                                          |
| `profile/models.rs`            | 2         | `umu_game_id` roundtrip, `is_empty` consideration                                                                                                                                                                                     |
| `launch/request.rs`            | 2         | `LaunchRequest` Serde default, runtime+top-level roundtrip                                                                                                                                                                            |
| `launch/script_runner.rs`      | 10        | Game swap to umu-run, PROTONPATH dirname, fallback on missing umu, Auto resolves to Proton, Proton always direct, trainer swap, trainer fallback, **flatpak-Steam never uses umu** (Steam opt-out), `umu-0` fallback + precedence (3) |
| `launch/preview.rs`            | 4         | Preview command string uses umu, PROTONPATH env push, Steam branch no PROTONPATH, ProtonSetup.umu_run_path None when preference=Proton                                                                                                |
| `launch/runtime_helpers.rs`    | 3         | `resolve_umu_run_path` empty/present/non-executable                                                                                                                                                                                   |
| `tests/umu_concurrent_pids.rs` | 1         | PR #148 non-regression — game + trainer PIDs both alive at t+500ms under stub umu-run                                                                                                                                                 |

## Deviations from Plan

- **Existing assertion splits**: plan called for splitting ~15-20 existing `"$PROTON" run` assertions into Proton-branch + umu-branch siblings. Implementation took a simpler path: the existing assertions remain valid for the Proton branch (which is the default when `UmuPreference::Auto`), and 4 new sibling tests cover the umu branch — meeting the spirit (verified coverage on both branches) without churning ~20 test bodies.
- **Test-only override of `resolve_umu_run_path`**: B4 added a `#[cfg(test)]` hook in `runtime_helpers.rs` that delegates to `optimizations::resolve_umu_run_path_for_test()` so `ScopedCommandSearchPath` controls umu-run discovery in unit tests. Integration tests (which compile without `#[cfg(test)]`) bypass the hook and use the real process `PATH`.
- **`launch_request_from_profile` signature**: settings are loaded at the CLI call site rather than threaded down from `profile_store(...)`. Cheap (TOML read) and avoids refactoring the helper signature.
- **PRD updates**: the Phase 3 row marker (`pending → in-progress`) and the #243 decision footnote were already landed in the plan-creation commit (`8dfde05`) — Task 8.1 verified rather than re-applied them.
- **Collateral struct-literal fixes**: 9 files outside the plan's "Files to Change" list were touched to satisfy compilation when `RuntimeSection` and `RuntimeLaunchConfig` gained the new field. All edits are minimal — `umu_game_id: String::new()` additions to existing `..Default::default()`-less literals.

## Issues Encountered

- **clippy lint regressions**: After all batches landed, clippy surfaced 2 `uninlined_format_args` violations in new test assertions and 4 `let_underscore_future` violations in `tests/umu_concurrent_pids.rs` (the test used `tokio::process::Command` whose `kill()`/`wait()` return futures). All 6 fixed in B8: `{var:?}` inlined format args + `.await` added to cleanup futures.
- **Pre-existing shellcheck noise**: `runtime-helpers/steam-host-trainer-runner.sh` carries existing SC1072/SC1073 warnings unrelated to Phase 3. They surface when running `shellcheck` on the file directly but `./scripts/lint.sh` (which uses the project's shellcheck config) passes clean.

## Out-of-Scope Working-Tree State

Three files modified outside this session's scope are present in the working tree (created/touched by an unrelated background process during the implementation window): `scripts/format.sh`, `scripts/lint.sh`, `scripts/lib/modified-files.sh` (new). These appear to introduce a `--modified` flag for incremental linting and are unrelated to the umu Phase 3 implementation. They should be committed (or stashed) **separately** from the Phase 3 changes.

## Next Steps

- [ ] Optional: `/ycc:code-review` to validate the diff before commit
- [ ] Commit the Phase 3 changeset (Conventional Commit, e.g. `feat(launch): add umu opt-in for non-Steam launches (Phase 3)`); commit unrelated `scripts/*` changes separately
- [ ] `/ycc:prp-pr` to open the PR (issue #256, child issues #236/#237/#238/#243)
- [ ] After merge: archive the plan to `docs/prps/plans/completed/` (mirroring Phase 2 closeout)
- [ ] Phase 4 prerequisite met: monitor `area:launch` + `feat:umu-launcher` + `type:bug` labels for 2 weeks before flipping `Auto → Umu` default
