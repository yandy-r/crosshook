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
- `cargo test -p crosshook-core`: passed after updating the stale launcher export assertion

## Critical Issues

- [tests] The branch did not initially satisfy the PR's claimed Rust test verification because `export::launcher::tests::export_writes_expected_paths_and_content` still asserted the removed unconditional `STEAM_COMPAT_DATA_PATH` export. The test has been updated to match the current `PREFIX_ROOT`/`WINEPREFIX` bootstrap. [`src/crosshook-native/crates/crosshook-core/src/export/launcher.rs:619`](../../src/crosshook-native/crates/crosshook-core/src/export/launcher.rs#L619)
  Status: Fixed

## Important Issues

- [code] The new install flow initially let users bypass executable confirmation and save an unusable generated profile. The handoff is now gated until the executable is explicitly confirmed, and the shared save path rejects profiles whose game executable is still empty. [`src/crosshook-native/src/components/InstallGamePanel.tsx:276`](../../src/crosshook-native/src/components/InstallGamePanel.tsx#L276) [`src/crosshook-native/src/components/InstallGamePanel.tsx:517`](../../src/crosshook-native/src/components/InstallGamePanel.tsx#L517) [`src/crosshook-native/src/hooks/useProfile.ts:295`](../../src/crosshook-native/src/hooks/useProfile.ts#L295)
  Status: Fixed

## Suggestions

- [tests] Add a regression test around the install review handoff for the "no discovered executable" path, so the UI cannot expose the save handoff until `installed_game_executable_path` is non-empty. [`src/crosshook-native/src/components/InstallGamePanel.tsx:276`](../../src/crosshook-native/src/components/InstallGamePanel.tsx#L276) [`src/crosshook-native/src/hooks/useInstallGame.ts:365`](../../src/crosshook-native/src/hooks/useInstallGame.ts#L365)
  Status: Open

## Strengths

- The install feature largely follows the existing architecture cleanly: Rust install-domain logic lives in `crosshook-core`, the Tauri surface stays thin, and the frontend hook/component split is consistent with the rest of the app.
- `cargo check -p crosshook-native` and `npm run build` both pass, so the current issues are concentrated in verification drift and install-flow behavior rather than broad compile instability.

## Residual Risks

- I did not run the installer flow end-to-end against a real Proton prefix, so the report is based on code inspection plus focused build/test verification rather than live install execution.
- I did not deeply review the newly added planning/research docs under `docs/plans/install-game/`; the review focused on changed product code, build scripts, and test behavior.
