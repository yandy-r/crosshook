# Context Analysis: launcher-delete

## Executive Summary

CrossHook exports `.sh` scripts and `.desktop` entries from profiles but provides zero lifecycle management -- deleting or renaming a profile leaves orphaned launcher files on disk. This feature adds a new `launcher_store` module in `crosshook-core/src/export/` that discovers, deletes, and renames launcher file pairs by reusing the existing deterministic slug derivation chain, plus Tauri IPC commands wired into profile lifecycle events, and frontend extensions to `LauncherExport.tsx` for status indicators and manual management. The core architectural approach is **stateless path derivation** (Phase 1-2) with no tracking database -- launcher paths are computed from `display_name` via `sanitize_launcher_slug()` and verified against the filesystem.

## Architecture Context

- **System Structure**: Three-layer architecture. Business logic in `crosshook-core` (Rust library). Thin IPC wrappers in `src-tauri/commands/`. React frontend invokes via `@tauri-apps/api/core invoke()`. The new `launcher_store.rs` is a sibling to `launcher.rs` inside `crosshook-core/src/export/`.
- **Data Flow**: Profile data provides display name inputs. `resolve_display_name()` picks the first non-empty value from `launcher_name > trainer_path stem > steam_app_id > fallback`. `sanitize_launcher_slug()` lowercases and normalizes to hyphen-separated ASCII. Paths are: `{home}/.local/share/crosshook/launchers/{slug}-trainer.sh` and `{home}/.local/share/applications/crosshook-{slug}-trainer.desktop`. Note the asymmetric `crosshook-` prefix on desktop entries only.
- **Integration Points**: (1) `profile_delete` Tauri command -- load profile before delete, derive slug, best-effort delete launcher files. (2) New `profile_rename` Tauri command -- atomic `fs::rename` of TOML, then write-new-then-delete-old for launcher files. (3) `LauncherExport.tsx` -- check launcher existence on mount, add status badge and management buttons. (4) `useProfile.ts` -- detect rename when `profileName !== selectedProfile`.

## Critical Files Reference

### Files to Create

- `src/crosshook-native/crates/crosshook-core/src/export/launcher_store.rs`: New module with all lifecycle types (`LauncherInfo`, `LauncherDeleteResult`, `LauncherRenameResult`) and functions (`check_launcher_exists`, `delete_launcher_files`, `rename_launcher_files`, `list_launchers`)

### Files to Modify -- Rust Backend

- `src/crosshook-native/crates/crosshook-core/src/export/launcher.rs`: Elevate 6 private functions to `pub(crate)` -- `resolve_display_name`, `combine_host_unix_path`, `build_desktop_entry_content`, `build_trainer_script_content`, `write_host_text_file`, `resolve_desktop_icon_value`
- `src/crosshook-native/crates/crosshook-core/src/export/mod.rs`: Add `pub mod launcher_store;` and re-export new public types
- `src/crosshook-native/crates/crosshook-core/src/profile/toml_store.rs`: Add `ProfileStore::rename(old_name, new_name)` using atomic `fs::rename`
- `src/crosshook-native/src-tauri/src/commands/export.rs`: Add 4 new Tauri commands -- `check_launcher_exists`, `delete_launcher`, `rename_launcher`, `list_launchers`
- `src/crosshook-native/src-tauri/src/commands/profile.rs`: Add `profile_rename` command; modify `profile_delete` to cascade launcher deletion
- `src/crosshook-native/src-tauri/src/lib.rs`: Register all 5 new commands in `invoke_handler`

### Files to Modify -- Frontend

- `src/crosshook-native/src/components/LauncherExport.tsx`: Add launcher status indicator (green/gray/amber badge), delete/rename buttons with inline confirmation, existence check on mount via `useEffect`
- `src/crosshook-native/src/hooks/useProfile.ts`: Add rename detection logic (`profileName !== selectedProfile && selectedProfile !== ''`), invoke `profile_rename` instead of save+delete
- `src/crosshook-native/src/types/` (new `launcher.ts` or additions to `profile.ts`): Add `LauncherInfo`, `LauncherDeleteResult`, `LauncherRenameResult` TypeScript interfaces
- `src/crosshook-native/src/types/index.ts`: Re-export new types

### Reference Files (patterns to follow, do not modify)

- `src/crosshook-native/crates/crosshook-core/src/install/models.rs`: Newest request/result struct pattern with serde derives
- `src/crosshook-native/crates/crosshook-core/src/install/service.rs`: Validate-then-execute pattern and comprehensive test structure
- `src/crosshook-native/src-tauri/src/commands/install.rs`: Async command pattern with `spawn_blocking` (reference only -- launcher ops are sync)
- `src/crosshook-native/crates/crosshook-core/src/profile/models.rs`: `GameProfile`, `LauncherSection` (contains `icon_path`, `display_name`) -- inputs for slug derivation
- `src/crosshook-native/crates/crosshook-core/src/settings/mod.rs`: `AppSettingsData.last_used_profile` -- must be updated on rename

## Patterns to Follow

- **Thin Tauri Command Adapters**: Every `#[tauri::command]` delegates immediately to `crosshook_core` and maps errors via `.map_err(|e| e.to_string())`. Zero business logic in command handlers. Reference: `src-tauri/src/commands/export.rs` (16 lines, 2 commands).
- **Display-Only Error Enums**: Custom error enum with manual `Display` + `Error` impls and `From<io::Error>`. NOT `anyhow`. New `LauncherStoreError` should follow `SteamExternalLauncherExportError` pattern in `launcher.rs:76-108`.
- **Request/Result IPC Structs**: `#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]` with `#[serde(default)]` on every field for all types crossing the IPC boundary.
- **Validate-Then-Execute**: Validation as a separate exported function called both from frontend (pre-flight) and from the main operation (guard). See `launcher.rs:110`.
- **Store Pattern with Testable Constructors**: `try_new()` / `with_base_path()` for production/test paths. New `launcher_store` functions should accept path parameters for `tempdir()` test isolation.
- **Inline Tests**: `#[cfg(test)] mod tests { ... }` at the bottom of each file. `tempfile::tempdir()` for isolation. Content assertions via `fs::read_to_string` + `contains()`. Permission assertions with `#[cfg(unix)]` and `PermissionsExt::mode()`.
- **Command Contract Tests**: Cast each command function to its expected type signature in a test to prevent signature drift. See `commands/settings.rs:39-52`.
- **Best-Effort Cascade**: Profile operations never blocked by launcher cleanup failures. Primary operation first, secondary attempt, catch and log. Profile deletion succeeds even if launcher cleanup fails.
- **Frontend Error Display**: Styled `<div>` with red background `rgba(185, 28, 28, 0.16)`, border `rgba(248, 113, 113, 0.28)`, text color `#fee2e2`. Success uses green scheme `rgba(16, 185, 129, 0.12)`.
- **Destructive Button Click-Again Pattern**: For low-severity actions (delete launcher only), button label changes to "Click again to delete" with 3-second timeout, reverts on blur. For medium-severity (profile + launcher), use modal dialog with "Cancel" as default focus.

## Cross-Cutting Concerns

- **Slug Derivation Consistency**: The exact same `resolve_display_name() -> sanitize_launcher_slug()` chain MUST be used by export, delete, rename, and existence checking. Duplication would cause path mismatches. This is why the private functions in `launcher.rs` must be elevated to `pub(crate)`.
- **Hardcoded Home Path vs. XDG**: Current `export_launchers()` hardcodes `~/.local/share/` instead of using `BaseDirs::data_dir()`. New code must match the hardcoded paths to find launchers exported by the current code. Either keep hardcoded in new code too, or refactor both simultaneously. Do not mix approaches.
- **Home Path Resolution for Backend Cascades**: `profile_delete` cascade happens in backend without frontend interaction. Backend must resolve `$HOME` independently via `resolve_target_home_path("", "")` which falls back to `std::env::var("HOME")`. Manual operations from UI continue to receive `targetHomePath` from the frontend.
- **Native Launch Method Skip**: Profiles with `launch.method === "native"` never have launchers. All lifecycle operations must skip these profiles silently.
- **Gamepad Accessibility**: A=confirm, B=cancel. Focus trap in dialogs. Default focus on "Cancel" (safe action). 44px minimum touch targets. No hover-dependent interactions.
- **Desktop Entry Content on Rename**: `.desktop` files embed `Name=`, `Exec=` (pointing to `.sh` path), and `Comment=` as plaintext. A file rename alone leaves stale content. Strategy: regenerate both files from scratch using `build_desktop_entry_content()` and `build_trainer_script_content()`, then delete old files. This is the "write-then-delete" approach.
- **Symlink Safety**: Before deleting any file, verify it is a regular file (not a symlink) using `fs::symlink_metadata`. Prevents symlink-following attacks.
- **Watermark Verification**: Check for `# Generated by CrossHook` comment before deleting any file to avoid removing user-created files with similar naming.
- **No Frontend Test Framework**: CLAUDE.md confirms no frontend test framework exists. UI behavior must be verified manually or through Rust-side contract tests.

## Parallelization Opportunities

### Phase 1 -- Can Run in Parallel

- **Track A (Rust Core)**: Create `launcher_store.rs` with types + delete functions + unit tests. Elevate `launcher.rs` private functions to `pub(crate)`.
- **Track B (TypeScript Types)**: Create `LauncherInfo`, `LauncherDeleteResult` interfaces in `types/launcher.ts`.
- **Sequential dependency**: Tauri commands depend on both tracks completing. `profile_delete` cascade depends on Tauri commands. Frontend UI depends on types + commands.

### Phase 2 -- Can Run in Parallel

- **Track A (Profile Rename Core)**: `ProfileStore::rename` method + `profile_rename` Tauri command + `useProfile.ts` rename detection.
- **Track B (Manual Management UI)**: Delete/rename buttons in `LauncherExport.tsx` + inline confirmation UX.
- **Sequential dependency**: Launcher rename function depends on `launcher_store` from Phase 1. Full rename cascade wires both tracks together.

### Shared Files Needing Coordination

- `src-tauri/src/lib.rs` -- all new commands registered here (merge conflict risk)
- `export/mod.rs` -- module declaration and re-exports
- `useProfile.ts` -- both rename detection and delete cascade touch this file

## Implementation Constraints

### Technical Constraints

- **No new dependencies**: All operations use `std::fs` and the existing `directories` crate (v5). No new Cargo.toml entries needed.
- **Synchronous operations**: Launcher file operations are <1ms. No `spawn_blocking` needed. Commands remain synchronous like existing export commands.
- **Atomic rename on same filesystem**: `std::fs::rename` is atomic on Linux when source and destination are on the same mount point. Both launcher paths are under `~/.local/share/`, so this is guaranteed.
- **Idempotent deletion**: All delete operations must treat `ErrorKind::NotFound` as success (file already gone).
- **Permissions preservation**: New files written during rename must maintain `0o755` for `.sh` and `0o644` for `.desktop`.
- **`install` context mode**: `LauncherExport.tsx` has a `context === 'install'` path that renders completely different UI. Launcher management UI must only appear in `context === 'default'`.

### Business/Design Constraints

- **Best-effort cascade**: Profile deletion always succeeds regardless of launcher cleanup outcome. Launcher errors are warnings, not blockers.
- **Rename is opt-in**: When profile rename is detected and launchers exist, show inline notification with "Save and Update Launcher" / "Save Without Updating" / "Cancel" -- do not auto-rename silently.
- **Confirmation tiering**: Modal dialog for profile+launcher delete (medium severity). Inline click-again for launcher-only delete (low severity).
- **Stateless Phase 1-2, optional manifest Phase 3**: No tracking registry needed initially. The deterministic slug derivation is sufficient. Manifest for orphan detection is a future enhancement.

### Unresolved Decisions (from feature-spec)

The feature spec lists 4 decisions with recommendations but no final sign-off:

1. **Manifest vs. Stateless** -- Recommended: (C) Phased, stateless now. _Proceed with stateless._
2. **Explicit Rename vs. Implicit Detection** -- Recommended: (A) Explicit `profile_rename` command. _Proceed with explicit._
3. **Confirmation UX for Automatic Cascade** -- Recommended: (B) Enhanced dialog when launchers exist. _Proceed with enhanced._
4. **Rename Cascade Behavior** -- Recommended: (B) Opt-in with inline notification. _Proceed with opt-in._

## Key Recommendations

- **Start with Phase 1 (Foundation)**: `launcher_store.rs` + `pub(crate)` visibility elevation + `check_launcher_exists`/`delete_launcher` commands + `profile_delete` cascade + launcher status indicator. This provides immediate user-visible value.
- **Critical path**: `launcher_store` module -> Tauri commands -> cascade integration -> frontend UI. The Rust module is the foundation; nothing else can proceed without it.
- **Test strategy**: Follow the `tempdir()` + round-trip pattern from `toml_store.rs`. Write fixtures that create launcher files, then verify delete/rename operations. Add command contract tests for all new Tauri commands.
- **Keep `ProfileStore` unaware of launchers**: Cascade logic lives at the Tauri command level, not inside `ProfileStore`. This preserves the existing separation of concerns.
- **Rename strategy is write-then-delete**: Do not use in-place `fs::rename` for launcher files because their content embeds display names and paths. Regenerate both files with correct content at new paths, then delete old files.
- **Frontend launcher state should be local to `LauncherExport.tsx`**: Use `useState` + `useEffect` calling `check_launcher_exists` on mount and profile change. No need for a dedicated hook since only one component consumes this state.
- **Phase tasks into ~18-22 discrete items across 3 phases**: Phase 1 (Foundation, ~7 tasks), Phase 2 (Rename + Manual Management, ~8 tasks), Phase 3 (Polish, ~5 tasks).
