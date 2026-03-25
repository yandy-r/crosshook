# TODO

## Changelog Hygiene

- [x] Tighten `.git-cliff.toml` so new release notes include only intentional, conventional, user-facing commits.
- [x] Add a release-notes validation script that fails on noisy sections before publish.
- [x] Run release-note validation during local release prep and in `.github/workflows/release.yml`.
- [x] Update `CLAUDE.md` with commit-message rules that preserve clean changelogs.
- [x] Update `tasks/lessons.md` with the changelog/commit-discipline lesson.
- [x] Verify the `v0.2.1` release-body extraction still passes after the new rules.

## Release Notes And Changelog Publishing

- [x] Confirm why `.github/workflows/release.yml` stopped publishing changelog/release body content after the native migration.
- [x] Add a deterministic release-body generation path that publishes only the matching `CHANGELOG.md` section for a tag.
- [x] Update the release workflow to publish the generated notes alongside the AppImage asset.
- [x] Remove the separate `release_notes.md` path so `CHANGELOG.md` is the only release-notes source.
- [x] Verify the generated release body locally for `v0.2.1`.
- [x] Update the live GitHub release `v0.2.1` with the corrected notes body.

## Restore Native Workspace Manifest

- [x] Reproduce the native container-build failure and confirm the root cause.
- [x] Restore the missing Rust workspace manifest at `src/crosshook-native/Cargo.toml`.
- [x] Verify `cargo metadata` succeeds against the restored workspace manifest.
- [x] Verify the container build path no longer fails at manifest parsing.
- [x] Trace the regression back to `scripts/prepare-release.sh`.
- [x] Patch the release-prep manifest update logic so it cannot truncate the workspace manifest again.
- [x] Verify the release-prep manifest update logic on temp copies without touching tracked manifests.

## Review

- Restored the missing Rust workspace manifest in `src/crosshook-native/Cargo.toml`, which had been committed as an empty file in `ee40b65` (`chore(release): prepare v0.2.1`).
- Verified the restored manifest with `cargo metadata --manifest-path src/crosshook-native/Cargo.toml --no-deps --format-version 1`.
- Verified the original failing path now succeeds with `scripts/build-native-container.sh`, which completed and produced `dist/CrossHook_0.2.0_amd64.AppImage`.
- Traced the regression to `scripts/prepare-release.sh`: the `perl -0pi -e '... exit(...)'` pattern can truncate the first file when used with in-place editing.
- Patched the release script to remove the unsafe `exit(...)` from the in-place Perl edit and added explicit sanity checks that fail if the workspace headers or expected versions are missing after the update.
- Reproduced the release-edit logic on temp manifest copies and confirmed the workspace manifest remains non-empty and updates to the requested version.

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

- Tightened `.git-cliff.toml` to filter unconventional commits and skip internal/non-user-facing forms such as `chore(...)`, `docs(internal):`, `docs(research):`, `docs(plan):`, and `style`.
- Added `scripts/validate-release-notes.sh` to reject noisy tagged release sections before publish. It fails on disallowed section headings and low-signal patterns like generic README churn, release-notes maintenance text, and task-plan residue.
- Wired release-note validation into both `scripts/prepare-release.sh` and `.github/workflows/release.yml`.
- Updated `CLAUDE.md` so contributors know commit messages are effectively release-note copy and that internal maintenance should use skipped forms.
- Updated `tasks/lessons.md` with the same changelog-discipline rule.
- Verification passed for the curated notes path with `./scripts/validate-release-notes.sh v0.2.1`.
- Verification also proved the gate catches bad generated output: `git-cliff --config .git-cliff.toml --tag v0.2.1 > /tmp/crosshook-generated-changelog.md && ./scripts/validate-release-notes.sh --changelog /tmp/crosshook-generated-changelog.md v0.2.1` failed on `Update README.md`.

- Confirmed the native release workflow regression: unlike the old .NET workflow, `.github/workflows/release.yml` never set `generate_release_notes` or any release body, so tagged releases published assets but no notes.
- Added `scripts/render-release-notes.sh` and wired the release workflow to publish the matching tagged `CHANGELOG.md` section via `body_path`.
- Removed `release_notes.md` so `CHANGELOG.md` is the only release-notes source in the repo.
- Cleaned the `v0.2.1` `CHANGELOG.md` section so the published notes emphasize the install-game flow, launcher lifecycle management, launch-panel follow fix, and release-manifest hardening.
- Verified locally with `./scripts/render-release-notes.sh v0.2.1`.
- Updated and verified the live GitHub release body with `gh release edit v0.2.1 --repo yandy-r/crosshook --notes-file /tmp/v0.2.1-release-body.md` and `gh release view v0.2.1 --repo yandy-r/crosshook --json body,url`.

- Replaced the console's `scrollIntoView`-based follow behavior with scroll-container-local bottom tracking so incoming `launch-log` events no longer move the page viewport away from the launch panel.
- Preserved live log following when the console is already pinned near the bottom, and preserved the user's position when they intentionally scroll up inside the console.
- Verification passed:
- `npm run build`
