# TODO

## PR #25 Critical Validation + Fix

- [x] Validate C1 against current `useProfile.ts` and Tauri `check_launcher_exists` command contract.
- [x] Validate C2 against current `SettingsPanel.tsx` and Tauri `delete_launcher` command contract.
- [x] Validate C3 against current `profile_rename` flow and launcher rename cascade coverage.
- [x] Validate C4 against current `launcher_store.rs` doc comment placement/content.
- [x] Implement minimal fixes for confirmed real issues (C1, C2, C4).
- [x] Run targeted tests for the touched Rust/Tauri/frontend paths.
- [x] Update `docs/pr-reviews/pr-25-review.md` with validation and fix status.
- [ ] Commit verified progress.

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
- Note: `cargo test -p crosshook-native ...` is currently blocked by pre-existing unrelated test compile failures in `src-tauri/src/startup.rs` (`tempfile` missing and stale `AppSettingsData` initializers).
