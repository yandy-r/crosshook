# PR #23 Review: feat(native): implement install game workflow

**PR**: #23 (`feat/install-game` -> `main`)
**Date**: 2026-03-24
**Scope**: +5,686 / -1,193 across 51 files
**Commits Reviewed**: `49a6583..1b8cdef`

## Verification

- `cargo check --manifest-path src/crosshook-native/Cargo.toml -p crosshook-native`
- `cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core`
- `npm run build` in `src/crosshook-native`

Results:

- `cargo check -p crosshook-native`: passed
- `npm run build`: passed
- `cargo test -p crosshook-core`: failed with 1 failing test

## Critical Issues

- [tests] The branch does not currently satisfy the PR's claimed Rust test verification because `cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core` fails in `export::launcher::tests::export_writes_expected_paths_and_content`. The implementation changed `build_trainer_script_content()` to derive `STEAM_COMPAT_DATA_PATH` through a shell bootstrap branch, but the test still asserts the old unconditional export string, so the suite is red on the PR branch. [`src/crosshook-native/crates/crosshook-core/src/export/launcher.rs:619`](../../src/crosshook-native/crates/crosshook-core/src/export/launcher.rs#L619)
  Status: Open

## Important Issues

- [code] The new install flow lets users bypass executable confirmation and save an unusable generated profile. `canReviewGeneratedProfile` is enabled for any successful install result, even when no executable has been confirmed, and `saveProfile()` only checks that the profile has a name. If discovery returns no candidates, or the user never picks one, `Review in Profile` still hydrates a profile with an empty `game.executable_path` and the normal save path accepts it. [`src/crosshook-native/src/components/InstallGamePanel.tsx:276`](../../src/crosshook-native/src/components/InstallGamePanel.tsx#L276) [`src/crosshook-native/src/components/InstallGamePanel.tsx:513`](../../src/crosshook-native/src/components/InstallGamePanel.tsx#L513) [`src/crosshook-native/src/hooks/useProfile.ts:287`](../../src/crosshook-native/src/hooks/useProfile.ts#L287)
  Status: Open

## Suggestions

- [tests] Add a regression test around the install review handoff for the "no discovered executable" path, so the UI cannot expose the save handoff until `installed_game_executable_path` is non-empty. [`src/crosshook-native/src/components/InstallGamePanel.tsx:276`](../../src/crosshook-native/src/components/InstallGamePanel.tsx#L276) [`src/crosshook-native/src/hooks/useInstallGame.ts:365`](../../src/crosshook-native/src/hooks/useInstallGame.ts#L365)
  Status: Open

## Strengths

- The install feature largely follows the existing architecture cleanly: Rust install-domain logic lives in `crosshook-core`, the Tauri surface stays thin, and the frontend hook/component split is consistent with the rest of the app.
- `cargo check -p crosshook-native` and `npm run build` both pass, so the current issues are concentrated in verification drift and install-flow behavior rather than broad compile instability.

## Residual Risks

- I did not run the installer flow end-to-end against a real Proton prefix, so the report is based on code inspection plus focused build/test verification rather than live install execution.
- I did not deeply review the newly added planning/research docs under `docs/plans/install-game/`; the review focused on changed product code, build scripts, and test behavior.
