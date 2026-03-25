# TODO

## Launcher Naming Normalization

- [x] Normalize the launcher display-name label between the profile and launcher export panels.
- [x] Add UI guidance that exported launchers automatically append ` - Trainer` to the visible launcher title.
- [x] Prevent duplicate ` - Trainer` suffixes when the entered launcher name already includes it.
- [x] Run targeted verification for the touched frontend and Rust export paths.

## Review

- Normalized the shared launcher field label to `Launcher Name` in both the profile editor and launcher export panels.
- Added helper copy in the profile editor and a styled info callout in launcher export to explain that CrossHook appends ` - Trainer` to the exported launcher title.
- Normalized launcher names on the frontend edit/save path and in the Rust export resolver so an entered or derived name that already ends with ` - Trainer` is reduced to its base name before export metadata is generated.
- Verification passed:
- `npm exec --yes tsc -- --noEmit`
- `cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core trainer_suffix`
- `git -c core.whitespace=trailing-space diff --check`

## PR #25 Critical Validation + Fix

- [x] Validate C1 against current `useProfile.ts` and Tauri `check_launcher_exists` command contract.
- [x] Validate C2 against current `SettingsPanel.tsx` and Tauri `delete_launcher` command contract.
- [x] Validate C3 against current `profile_rename` flow and launcher rename cascade coverage.
- [x] Validate C4 against current `launcher_store.rs` doc comment placement/content.
- [x] Implement minimal fixes for confirmed real issues (C1, C2, C4).
- [x] Run targeted tests for the touched Rust/Tauri/frontend paths.
- [x] Update `docs/pr-reviews/pr-25-review.md` with validation and fix status.
- [x] Commit verified progress.

## Review

- Confirmed C1: `useProfile.ts` invoked `check_launcher_exists` with `{ profileName }`, which cannot satisfy the command's five-argument IPC contract.
- Confirmed C2: `SettingsPanel.tsx` invoked `delete_launcher` with `{ launcherSlug }`, which cannot satisfy the command's five-argument IPC contract.
- C3 was not reproducible as written: launcher file paths are derived from `steam.launcher.display_name` / `steam.app_id` / `trainer.path`, not the profile TOML filename, so renaming a profile file does not by itself imply launcher orphaning.
- Confirmed C4: the doc comment above `find_orphaned_launchers` contained text describing `extract_display_name_from_desktop`.
- Verification run:
- `cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core check_launcher_for_profile_delegates_correctly`
- `cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core delete_launcher_by_slug_deletes_matching_files`
- `npm exec --yes tsc -- --noEmit`
- `cargo check --manifest-path src/crosshook-native/Cargo.toml -p crosshook-native --lib`
- Follow-up resolved the earlier `crosshook-native` test blocker by adding the missing `tempfile` dev-dependency and updating stale `AppSettingsData` test initializers in `startup.rs`.

## PR #25 Remaining Findings

- [x] Validate remaining frontend/type findings (I1, I2, I3, I10).
- [x] Validate remaining Rust/Tauri/doc/test findings (I4-I12, T1-T6, S2-S5).
- [x] Implement minimal fixes only for confirmed remaining issues.
- [x] Run targeted checks for touched areas.
- [x] Update `docs/pr-reviews/pr-25-review.md` with second-pass validation and fix status.
- [x] Commit verified follow-up progress.

## Second-Pass Review

- Verified `I1`, `I2`, `I3`, and `I10` against the current workspace. `I1`, `I2`, `I3`, and `I10` are fixed in the working tree state.
- Fixed `I4` and `I5` by returning rename cleanup warnings and enforcing watermark verification before old-file removal.
- Fixed `I6`, `T3`, and `S3` by extracting profile-delete launcher cleanup into a testable helper that derives Steam/home paths and logs native skips.
- Fixed `I8`, `I9`, `S2`, and `S5` by propagating launcher inspection errors via `Result`, logging directory/entry read failures, and treating unreadable desktop entries as stale.
- Fixed `I12` with Tauri command doc comments.
- Closed `T1`, `T2`, `T4`, `T5`, and `T6` with focused regression tests.
- Verification now passes for `npm exec --yes tsc -- --noEmit`, `cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core`, and `cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-native`.

## Launch Panel Console Follow Behavior

- [x] Confirm the current launch/console implementation and identify the source of viewport movement.
- [x] Replace console viewport-follow behavior with container-local auto-follow logic.
- [x] Run a focused frontend verification check.

## Review

- Replaced the console's `scrollIntoView`-based follow behavior with scroll-container-local bottom tracking so incoming `launch-log` events no longer move the page viewport away from the launch panel.
- Preserved live log following when the console is already pinned near the bottom, and preserved the user's position when they intentionally scroll up inside the console.
- Verification passed:
- `npm run build`
