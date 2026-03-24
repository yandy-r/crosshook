# Launcher Lifecycle Management Implementation Plan

CrossHook's launcher export system writes `.sh` scripts and `.desktop` entries but has zero lifecycle management -- deleting or renaming a profile leaves orphaned files on disk. This plan introduces a `launcher_store` module in `crosshook-core/src/export/` that discovers, deletes, and renames launcher file pairs by reusing the existing deterministic `sanitize_launcher_slug()` derivation chain (requiring 5 private functions elevated to `pub(crate)`), plus new Tauri IPC commands cascaded from `profile_delete` and a new `profile_rename`, and frontend extensions to `LauncherExport.tsx` for status indicators and manual management. The implementation spans 19 tasks across 3 phases: Foundation + Delete (7 tasks), Rename + Manual Management (7 tasks), and Polish (5 tasks), with the critical path running through `launcher_store.rs` -> Tauri commands -> cascade wiring -> frontend UI.

## Critically Relevant Files and Documentation

- src/crosshook-native/crates/crosshook-core/src/export/launcher.rs: Core export logic with slug derivation chain --5 private functions need `pub(crate)` elevation
- src/crosshook-native/crates/crosshook-core/src/export/mod.rs: Export module root --needs `pub mod launcher_store;` and re-exports
- src/crosshook-native/crates/crosshook-core/src/profile/toml_store.rs: `ProfileStore` CRUD --needs `rename()` method
- src/crosshook-native/crates/crosshook-core/src/profile/models.rs: `GameProfile`, `LauncherSection` (display_name, icon_path) --slug derivation inputs
- src/crosshook-native/crates/crosshook-core/src/settings/mod.rs: `SettingsStore`, `AppSettingsData.last_used_profile` --update on rename
- src/crosshook-native/src-tauri/src/commands/export.rs: Thin Tauri export commands --add 4 new commands
- src/crosshook-native/src-tauri/src/commands/profile.rs: Profile CRUD commands --modify `profile_delete`, add `profile_rename`
- src/crosshook-native/src-tauri/src/lib.rs: Tauri setup --register all new commands in `invoke_handler`
- src/crosshook-native/src/components/LauncherExport.tsx: Export panel --add status indicator, delete/rename buttons
- src/crosshook-native/src/hooks/useProfile.ts: Profile state hook --cascade delete and rename detection
- src/crosshook-native/src/types/profile.ts: TypeScript types for `GameProfile`, `LaunchMethod`
- src/crosshook-native/src/types/index.ts: Type re-exports
- src/crosshook-native/crates/crosshook-core/src/install/models.rs: Reference for newest IPC struct pattern with serde derives
- src/crosshook-native/src-tauri/src/commands/settings.rs: Reference for command contract test pattern
- docs/plans/launcher-delete/feature-spec.md: Authoritative feature specification
- docs/plans/launcher-delete/research-technical.md: Architecture design and technical decisions
- docs/plans/launcher-delete/research-patterns.md: Codebase patterns and conventions
- docs/plans/launcher-delete/research-ux.md: UX patterns, confirmation dialogs, gamepad accessibility
- docs/plans/launcher-delete/research-external.md: Freedesktop .desktop spec, file operation patterns
- docs/features/steam-proton-trainer-launch.doc.md: Current launcher export system documentation

## Implementation Plan

### Phase 1: Foundation + Delete

#### Task 1.1: Elevate launcher.rs private functions to pub(crate)

Depends on [none]

**READ THESE BEFORE TASK**

- src/crosshook-native/crates/crosshook-core/src/export/launcher.rs
- docs/plans/launcher-delete/research-patterns.md (Rust Visibility Pattern section)

**Instructions**

Files to Modify

- src/crosshook-native/crates/crosshook-core/src/export/launcher.rs

Change the visibility of these 5 private functions from `fn` to `pub(crate) fn`:

1. `resolve_display_name` (line ~220) --display name fallback chain used for slug derivation
2. `combine_host_unix_path` (line ~274) --path segment joiner for launcher file paths
3. `build_trainer_script_content` (line ~296) --generates `.sh` script content
4. `build_desktop_entry_content` (line ~414) --generates `.desktop` entry content
5. `write_host_text_file` (line ~441) --writes text file with Unix permissions

This is a purely additive visibility change --no logic modification. All existing tests must continue to pass. These functions are needed by the new `launcher_store` module to reuse the slug derivation chain and file generation without duplication.

Do NOT make them `pub` --they should remain crate-internal via `pub(crate)`.

#### Task 1.2: Create launcher_store module with types and delete logic

Depends on [1.1]

**READ THESE BEFORE TASK**

- src/crosshook-native/crates/crosshook-core/src/export/launcher.rs (slug derivation chain, error enum pattern)
- src/crosshook-native/crates/crosshook-core/src/install/models.rs (newest IPC struct pattern with serde derives)
- docs/plans/launcher-delete/feature-spec.md (Data Models section)
- docs/plans/launcher-delete/research-external.md (Integration Patterns section)

**Instructions**

Files to Create

- src/crosshook-native/crates/crosshook-core/src/export/launcher_store.rs

Files to Modify

- src/crosshook-native/crates/crosshook-core/src/export/mod.rs

Create the new `launcher_store` module as a sibling to `launcher.rs`. Import shared functions via `use super::launcher::{sanitize_launcher_slug, resolve_display_name, combine_host_unix_path, resolve_target_home_path}`.

Define types following the IPC struct pattern (`#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]` with `#[serde(default)]` on every field):

- `LauncherInfo` --`display_name`, `launcher_slug`, `script_path`, `desktop_entry_path`, `script_exists: bool`, `desktop_entry_exists: bool`
- `LauncherDeleteResult` --`script_deleted: bool`, `desktop_entry_deleted: bool`, `script_path`, `desktop_entry_path`
- `LauncherStoreError` --follow Display-only error enum pattern from `SteamExternalLauncherExportError` (launcher.rs:76-108). Variants: `Io(io::Error)`, `HomePathResolutionFailed`. Include `From<io::Error>` impl.

Implement core functions. **IMPORTANT cross-crate visibility**: Since `resolve_display_name` is `pub(crate)` (visible within `crosshook-core` but NOT from `src-tauri`), the `launcher_store` must expose `pub` facade functions that accept profile-level inputs and derive the slug internally. The Tauri command layer calls these `pub` facades, never the `pub(crate)` helpers directly.

1. `check_launcher_exists(display_name: &str, steam_app_id: &str, trainer_path: &str, target_home_path: &str, steam_client_install_path: &str) -> LauncherInfo` -- internally calls `resolve_display_name` to derive the display name, then `sanitize_launcher_slug` for the slug, then `resolve_target_home_path` + `combine_host_unix_path` to construct both file paths. Checks existence with `Path::exists()`, returns populated `LauncherInfo` including the derived `launcher_slug`. This solves the frontend slug derivation problem -- the frontend passes profile-level field inputs and the backend derives the slug.

2. `delete_launcher_files(display_name: &str, steam_app_id: &str, trainer_path: &str, target_home_path: &str, steam_client_install_path: &str) -> Result<LauncherDeleteResult, LauncherStoreError>` -- derives slug same as above, resolves paths, attempts `fs::remove_file` on each file treating `ErrorKind::NotFound` as success (idempotent). Deletes `.desktop` first (user-visible artifact), then `.sh` script. Before deletion, verify target is a regular file and check for the `# Generated by CrossHook` watermark.

3. `delete_launcher_for_profile(profile: &GameProfile, target_home_path: &str, steam_client_install_path: &str) -> Result<LauncherDeleteResult, LauncherStoreError>` -- convenience `pub` facade that extracts `display_name`, `steam_app_id`, `trainer_path` from the `GameProfile` struct and delegates to `delete_launcher_files`. This is what the `profile_delete` cascade in `src-tauri` calls.

Add `pub mod launcher_store;` to `mod.rs` and re-export all new public types and functions.

Include inline tests (`#[cfg(test)] mod tests`) using `tempfile::tempdir()`: test check when both files exist, when neither exists, when one exists; test delete when both exist, when neither exists (no-op), when one is missing. Use `fs::write` + `fs::create_dir_all` to create test fixtures.

#### Task 1.3: Add TypeScript launcher types

Depends on [none]

**READ THESE BEFORE TASK**

- src/crosshook-native/src/types/profile.ts (existing type pattern)
- src/crosshook-native/src/types/index.ts (re-export pattern)
- docs/plans/launcher-delete/feature-spec.md (Data Models section)

**Instructions**

Files to Create

- src/crosshook-native/src/types/launcher.ts

Files to Modify

- src/crosshook-native/src/types/index.ts

Create TypeScript interfaces mirroring the Rust structs with `snake_case` property names (matching serde serialization):

```typescript
export interface LauncherInfo {
  display_name: string;
  launcher_slug: string;
  script_path: string;
  desktop_entry_path: string;
  script_exists: boolean;
  desktop_entry_exists: boolean;
}

export interface LauncherDeleteResult {
  script_deleted: boolean;
  desktop_entry_deleted: boolean;
  script_path: string;
  desktop_entry_path: string;
}

export interface LauncherRenameResult {
  old_slug: string;
  new_slug: string;
  new_script_path: string;
  new_desktop_entry_path: string;
  script_renamed: boolean;
  desktop_entry_renamed: boolean;
}
```

Add `export * from './launcher'` to `index.ts`.

#### Task 1.4: Add check_launcher_exists and delete_launcher Tauri commands

Depends on [1.2]

**READ THESE BEFORE TASK**

- src/crosshook-native/src-tauri/src/commands/export.rs (thin adapter pattern)
- src/crosshook-native/src-tauri/src/lib.rs (command registration)
- src/crosshook-native/src-tauri/src/commands/settings.rs (command contract test pattern, lines 38-52)

**Instructions**

Files to Modify

- src/crosshook-native/src-tauri/src/commands/export.rs
- src/crosshook-native/src-tauri/src/lib.rs

Add two new Tauri commands to `commands/export.rs` following the existing thin-adapter pattern (receive data via parameters, delegate to `crosshook_core`, map errors):

```rust
#[tauri::command]
pub fn check_launcher_exists(
    display_name: String,
    steam_app_id: String,
    trainer_path: String,
    target_home_path: String,
    steam_client_install_path: String,
) -> LauncherInfo {
    crosshook_core::export::check_launcher_exists(
        &display_name, &steam_app_id, &trainer_path,
        &target_home_path, &steam_client_install_path,
    )
}

#[tauri::command]
pub fn delete_launcher(
    display_name: String,
    steam_app_id: String,
    trainer_path: String,
    target_home_path: String,
    steam_client_install_path: String,
) -> Result<LauncherDeleteResult, String> {
    crosshook_core::export::delete_launcher_files(
        &display_name, &steam_app_id, &trainer_path,
        &target_home_path, &steam_client_install_path,
    ).map_err(|error| error.to_string())
}
```

Note: The commands accept profile-level field inputs (display_name, steam_app_id, trainer_path) and derive the slug internally via the `pub(crate)` functions in `launcher_store`. The frontend already has these values from the profile object.

Register both in `lib.rs` `invoke_handler` macro after the existing export commands (after `commands::export::export_launchers`):

- `commands::export::check_launcher_exists`
- `commands::export::delete_launcher`

Add a command contract test verifying the function signatures.

#### Task 1.5: Modify profile_delete to cascade launcher deletion

Depends on [1.2, 1.4]

**READ THESE BEFORE TASK**

- src/crosshook-native/src-tauri/src/commands/profile.rs (existing profile_delete)
- src/crosshook-native/crates/crosshook-core/src/export/launcher.rs (resolve_display_name, sanitize_launcher_slug, resolve_target_home_path)
- docs/plans/launcher-delete/analysis-context.md (Cross-Cutting Concerns section)

**Instructions**

Files to Modify

- src/crosshook-native/src-tauri/src/commands/profile.rs

Modify the `profile_delete` command to cascade launcher cleanup. The cascade is **best-effort** --profile deletion MUST succeed even if launcher cleanup fails. Do NOT change the command signature or return type.

Before calling `store.delete(&name)`:

1. Attempt to load the profile via `store.load(&name).ok()` (best-effort --profile may be corrupt)
2. If loaded successfully and `profile.launch.method` is NOT `"native"`:
   - Call `delete_launcher_for_profile(&profile, "", "")` from `crosshook_core::export::launcher_store` --this `pub` facade internally derives the slug and resolves `$HOME` for the target path. Wrap in `let _ =` or match to swallow errors.
3. Then proceed with existing `store.delete(&name)` logic

Use the `delete_launcher_for_profile` facade (NOT `resolve_display_name` or `sanitize_launcher_slug` directly --those are `pub(crate)` and invisible from `src-tauri`). Use `tracing::warn!` to log any launcher cleanup failures for debugging.

#### Task 1.6: Add launcher status indicator to LauncherExport.tsx

Depends on [1.3, 1.4]

**READ THESE BEFORE TASK**

- src/crosshook-native/src/components/LauncherExport.tsx (existing component structure, deriveLauncherName helper)
- src/crosshook-native/src/types/launcher.ts (LauncherInfo type)
- docs/plans/launcher-delete/research-ux.md (Status Indicators section)

**Instructions**

Files to Modify

- src/crosshook-native/src/components/LauncherExport.tsx

Add launcher existence checking and status display. Only for `context === 'default'` (not the install flow).

1. Add state: `const [launcherStatus, setLauncherStatus] = useState<LauncherInfo | null>(null)`

2. Add a `useEffect` that calls `check_launcher_exists` on mount and whenever `profile`, `targetHomePath`, or `steamClientInstallPath` changes. Use the `deriveLauncherName(profile)` helper already in the component to get the display name, then pass it through the backend `check_launcher_exists` command to get the authoritative slug and existence status.

3. After the existing export result display area, add a status badge:
   - Both exist: green dot + "Exported" label (`background: 'rgba(16, 185, 129, 0.12)'`, `color: '#d1fae5'`)
   - Neither exists: gray dot + "Not Exported" label
   - One exists: amber dot + "Partial" label (warning state)
   - Use a small colored circle (8-10px) with text label. Do not use color alone --include text for accessibility.

4. After a successful export (`handleExport`), re-run the existence check to update the status badge.

The `check_launcher_exists` command accepts profile-level field inputs (`display_name`, `steam_app_id`, `trainer_path`) and derives the slug internally in Rust. The frontend has all these values available from the `profile` prop: `profile.steam.launcher.display_name`, `profile.steam.app_id`, `profile.trainer.path`. Pass these alongside `targetHomePath` and `steamClientInstallPath` (already available as props). The returned `LauncherInfo` includes the derived `launcher_slug` for display purposes.

### Phase 2: Rename + Manual Management

#### Task 2.1: Add ProfileStore::rename method

Depends on [none]

**READ THESE BEFORE TASK**

- src/crosshook-native/crates/crosshook-core/src/profile/toml_store.rs (existing store pattern, validate_name, profile_path helper)

**Instructions**

Files to Modify

- src/crosshook-native/crates/crosshook-core/src/profile/toml_store.rs

Add a `rename` method to `ProfileStore`:

```rust
pub fn rename(&self, old_name: &str, new_name: &str) -> Result<(), ProfileStoreError> {
    let old_name = old_name.trim();
    let new_name = new_name.trim();
    Self::validate_name(old_name)?;
    Self::validate_name(new_name)?;
    let old_path = self.profile_path(old_name)?;
    let new_path = self.profile_path(new_name)?;
    if !old_path.exists() {
        return Err(ProfileStoreError::NotFound(old_path));
    }
    if old_name == new_name {
        return Ok(()); // no-op
    }
    fs::rename(&old_path, &new_path)?;
    Ok(())
}
```

If a `NotFound` variant doesn't exist on `ProfileStoreError`, add one: `NotFound(PathBuf)` with appropriate `Display` arm.

Add inline tests: rename succeeds, rename when old doesn't exist fails, rename to same name is no-op, rename preserves profile content.

#### Task 2.2: Add rename_launcher_files and list_launchers to launcher_store

Depends on [1.2]

**READ THESE BEFORE TASK**

- src/crosshook-native/crates/crosshook-core/src/export/launcher_store.rs (existing module)
- src/crosshook-native/crates/crosshook-core/src/export/launcher.rs (build_desktop_entry_content, build_trainer_script_content)
- docs/plans/launcher-delete/feature-spec.md (API Design section --rename_launcher)
- docs/plans/launcher-delete/research-external.md (Operation: Rename Launcher section)

**Instructions**

Files to Modify

- src/crosshook-native/crates/crosshook-core/src/export/launcher_store.rs

Add `LauncherRenameResult` type (same serde pattern as other types): `old_slug`, `new_slug`, `new_script_path`, `new_desktop_entry_path`, `script_renamed: bool`, `desktop_entry_renamed: bool`.

Implement `rename_launcher_files(old_launcher_slug, new_display_name, new_launcher_icon_path, target_home_path, steam_client_install_path, request: &SteamExternalLauncherExportRequest) -> Result<LauncherRenameResult, LauncherStoreError>`:

The rename strategy is **write-then-delete** (not in-place `fs::rename`) because both `.sh` and `.desktop` files embed display names and paths as plaintext:

1. Derive `new_slug` from `new_display_name` via `sanitize_launcher_slug`
2. Resolve home path via `resolve_target_home_path`
3. Construct old file paths from `old_launcher_slug` and new file paths from `new_slug`
4. If old files don't exist, return early with `renamed: false`
5. Generate new script content via `build_trainer_script_content` and new desktop content via `build_desktop_entry_content` using the new display name and new paths
6. Write new files via `write_host_text_file` (script: `0o755`, desktop: `0o644`)
7. Delete old files if paths differ (skip if slug unchanged --content was rewritten in place)
8. Return `LauncherRenameResult` with all details

Implement `list_launchers(target_home_path, steam_client_install_path) -> Vec<LauncherInfo>`:

1. Resolve home path, construct launchers directory path (`{home}/.local/share/crosshook/launchers/`)
2. Read directory, filter for `*-trainer.sh` files
3. For each: derive slug by stripping `-trainer.sh` suffix, check for matching `.desktop` entry, extract display name from `Name=` line in `.desktop` file
4. Return sorted `Vec<LauncherInfo>`

Add inline tests for: rename when old exists, rename when old doesn't exist, rename when slug is unchanged (content rewrite only), list with 0/1/multiple launchers.

#### Task 2.3: Add profile_rename, rename_launcher, list_launchers Tauri commands

Depends on [2.1, 2.2]

**READ THESE BEFORE TASK**

- src/crosshook-native/src-tauri/src/commands/export.rs (existing command pattern)
- src/crosshook-native/src-tauri/src/commands/profile.rs (profile commands with State)
- src/crosshook-native/src-tauri/src/lib.rs (command registration)

**Instructions**

Files to Modify

- src/crosshook-native/src-tauri/src/commands/export.rs
- src/crosshook-native/src-tauri/src/commands/profile.rs
- src/crosshook-native/src-tauri/src/lib.rs

Add to `commands/export.rs` (thin-adapter pattern, no `State`):

- `rename_launcher` --delegates to `launcher_store::rename_launcher_files`, maps errors
- `list_launchers` --delegates to `launcher_store::list_launchers`

Add to `commands/profile.rs` (uses `State<'_, ProfileStore>` and `State<'_, SettingsStore>`):

- `profile_rename(old_name: String, new_name: String, store: State<'_, ProfileStore>, settings_store: State<'_, SettingsStore>)` --orchestrates: (1) load old profile for launcher slug derivation, (2) `store.rename(&old_name, &new_name)`, (3) best-effort launcher rename using derived slugs, (4) update `last_used_profile` in settings if it matches old_name

Register all 3 new commands in `lib.rs` `invoke_handler`:

- `commands::export::rename_launcher`
- `commands::export::list_launchers`
- `commands::profile::profile_rename`

Add command contract tests for all new commands.

#### Task 2.4: Add rename detection to useProfile.ts

Depends on [2.3]

**READ THESE BEFORE TASK**

- src/crosshook-native/src/hooks/useProfile.ts (saveProfile and deleteProfile functions)
- docs/plans/launcher-delete/analysis-code.md (Frontend Rename Detection section)

**Instructions**

Files to Modify

- src/crosshook-native/src/hooks/useProfile.ts

In `saveProfile()` (line ~295), add rename detection before the existing save logic:

```typescript
const isRename = profileName.trim() !== selectedProfile && selectedProfile !== '' && profiles.includes(selectedProfile);

if (isRename) {
  await invoke('profile_rename', {
    oldName: selectedProfile,
    newName: profileName.trim(),
  });
  // Update local state to reflect the rename
  await refreshProfiles();
  await loadProfile(profileName.trim());
  return;
}
```

The `profile_rename` backend command handles the full cascade: TOML rename, launcher rename, settings update. The frontend just needs to detect the condition and invoke the command.

Handle errors using the existing `setError` pattern.

#### Task 2.5: Add manual delete button to LauncherExport.tsx

Depends on [1.4, 1.6]

**READ THESE BEFORE TASK**

- src/crosshook-native/src/components/LauncherExport.tsx (existing button styles, error display pattern)
- docs/plans/launcher-delete/research-ux.md (Destructive Action Patterns section, Manual Launcher Management section)

**Instructions**

Files to Modify

- src/crosshook-native/src/components/LauncherExport.tsx

Add a "Delete Launcher" button visible when `launcherStatus?.script_exists || launcherStatus?.desktop_entry_exists` and `context === 'default'`.

Implement inline click-again confirmation pattern:

1. State: `const [deleteConfirming, setDeleteConfirming] = useState(false)`
2. First click: set `deleteConfirming = true`, change label to "Click again to confirm"
3. Set a 3-second timeout to revert `deleteConfirming = false`
4. On blur (focus lost), immediately revert
5. Second click (while `deleteConfirming`): invoke `delete_launcher`, then re-check status

Use destructive button styling: `background: 'rgba(185, 28, 28, 0.16)'`, `border: '1px solid rgba(248, 113, 113, 0.28)'`, `color: '#fee2e2'`.

After successful delete, update the launcher status state and show a brief status message.

#### Task 2.6: Add inline rename notification to LauncherExport.tsx

Depends on [2.3, 2.4, 2.5]

**READ THESE BEFORE TASK**

- src/crosshook-native/src/components/LauncherExport.tsx (current component structure)
- docs/plans/launcher-delete/research-ux.md (Rename Patterns section, Rename cascade notification)

**Instructions**

Files to Modify

- src/crosshook-native/src/components/LauncherExport.tsx

When a profile has been renamed (detected via `launcherStatus?.launcher_slug` differing from the slug that would be derived from the current display name) and launchers exist, show an inline notification panel:

- Display old and new file paths for transparency
- Offer "Update Launcher" button that calls `rename_launcher` via invoke
- Offer "Re-export Launcher" button as an alternative (calls existing `export_launchers`)

This is a "should-have" enhancement --the core rename cascade happens automatically via `profile_rename`. This UI is for awareness and manual intervention when the user wants finer control.

### Phase 3: Polish

#### Task 3.1: Add X-CrossHook-Profile metadata to .desktop files

Depends on [none]

**READ THESE BEFORE TASK**

- src/crosshook-native/crates/crosshook-core/src/export/launcher.rs (build_desktop_entry_content function)
- docs/plans/launcher-delete/research-external.md (Desktop Entry File Naming Rules)

**Instructions**

Files to Modify

- src/crosshook-native/crates/crosshook-core/src/export/launcher.rs

In `build_desktop_entry_content`, append two `X-` prefixed lines before the final newline:

```
X-CrossHook-Profile={profile_name}
X-CrossHook-Slug={slug}
```

Per the Freedesktop spec, implementations must not remove unknown fields. These custom fields enable future orphan detection (scanning `.desktop` files for their owning profile).

Update existing export tests to assert the new lines are present in generated content.

#### Task 3.2: Add watermark verification before delete operations

Depends on [1.2]

**READ THESE BEFORE TASK**

- src/crosshook-native/crates/crosshook-core/src/export/launcher_store.rs (delete_launcher_files function)
- docs/plans/launcher-delete/research-recommendations.md (Security Considerations section)

**Instructions**

Files to Modify

- src/crosshook-native/crates/crosshook-core/src/export/launcher_store.rs

Enhance `delete_launcher_files` with safety checks before each `fs::remove_file`:

1. Verify target is a regular file (not a symlink) via `fs::symlink_metadata(path)?.file_type().is_file()`
2. Read the first few lines and verify the `# Generated by CrossHook` comment exists in `.sh` files
3. Verify `Comment=...Generated by CrossHook` exists in `.desktop` files
4. If either check fails, skip deletion for that file and add a `skipped_reason` to the result

Add a `script_skipped_reason: Option<String>` and `desktop_entry_skipped_reason: Option<String>` field to `LauncherDeleteResult`.

#### Task 3.3: Add stale launcher detection

Depends on [1.2, 1.6]

**READ THESE BEFORE TASK**

- src/crosshook-native/crates/crosshook-core/src/export/launcher_store.rs (check_launcher_exists)
- src/crosshook-native/src/components/LauncherExport.tsx (status indicator from Task 1.6)
- docs/plans/launcher-delete/research-ux.md (Status Indicators section)

**Instructions**

Files to Modify

- src/crosshook-native/crates/crosshook-core/src/export/launcher_store.rs
- src/crosshook-native/src/components/LauncherExport.tsx

Enhance `check_launcher_exists` (or add a separate `check_launcher_stale` function) to:

1. Read the `Name=` line from the existing `.desktop` file
2. Compare against the expected display name (derived from current profile data)
3. If they differ, the launcher is "stale"
4. Add `is_stale: bool` field to `LauncherInfo`

On the frontend, update the status badge to show "Stale" (amber, `rgba(245, 158, 11, 0.12)`) when `is_stale === true`. Add an "Update Launcher" button that re-exports.

#### Task 3.4: Add orphan scanner and cleanup UI

Depends on [2.2, 2.3]

**READ THESE BEFORE TASK**

- src/crosshook-native/crates/crosshook-core/src/export/launcher_store.rs (list_launchers)
- src/crosshook-native/src/components/SettingsPanel.tsx (existing settings UI structure)

**Instructions**

Files to Modify

- src/crosshook-native/crates/crosshook-core/src/export/launcher_store.rs
- src/crosshook-native/src/components/SettingsPanel.tsx

Add a `find_orphaned_launchers` function to `launcher_store` that:

1. Calls `list_launchers` to get all CrossHook launcher files
2. Accepts a list of known profile slugs (derived by the caller from `ProfileStore::list` + loading each profile)
3. Returns launchers that don't match any known profile slug

On the frontend, add an "Orphaned Launchers" section to `SettingsPanel.tsx`:

- Show count of orphaned launchers
- Expandable list with file paths
- "Clean Up" button to delete all orphans (with confirmation)
- Only visible when orphans are detected

#### Task 3.5: Add confirmation modal for profile-delete-with-launcher cascade

Depends on [1.4, 1.5]

**READ THESE BEFORE TASK**

- src/crosshook-native/src/components/ProfileEditor.tsx (existing delete button)
- src/crosshook-native/src/hooks/useProfile.ts (deleteProfile callback)
- docs/plans/launcher-delete/research-ux.md (Destructive Action Patterns, Gamepad Considerations)

**Instructions**

Files to Modify

- src/crosshook-native/src/components/ProfileEditor.tsx
- src/crosshook-native/src/hooks/useProfile.ts

Before calling `profile_delete`, check launcher existence:

1. In `deleteProfile()` (useProfile.ts), call `check_launcher_exists` first
2. If launchers exist, set a state flag to show a confirmation modal
3. The modal should display: "Delete profile **[Name]** and its launcher files?" with the file paths listed
4. Buttons: "Delete Profile and Launcher" (red, destructive style) | "Cancel" (default focus)
5. If no launchers exist, use a simpler inline confirmation

The modal must support gamepad navigation: A=confirm, B=cancel. Use the `<dialog>` HTML element for built-in focus trapping, or implement focus trap manually. Default focus should be on "Cancel" (safe action). All interactive elements must be >= 44px height.

## Advice

- The `launcher_store` module (Task 1.2) is the critical path bottleneck --every subsequent task depends on it. Prioritize getting this right, including tests, before moving on.
- The slug derivation chain (`resolve_display_name → sanitize_launcher_slug → combine_host_unix_path`) MUST be shared between export and lifecycle operations. Never duplicate this logic. After Task 1.1 elevates visibility, import from `super::launcher::*` in the new module.
- Current `export_launchers()` hardcodes `~/.local/share/` rather than using `BaseDirs::data_dir()`. The new code must match this hardcoded convention to find launchers exported by the current code. Do not mix approaches --either keep hardcoded everywhere or refactor both export and lifecycle simultaneously.
- Profile `delete` cascade resolves `target_home_path` backend-side via `resolve_target_home_path("", "")` which falls back to `$HOME`. Manual operations from the UI continue to receive `targetHomePath` from the frontend as they do today for export.
- The rename strategy is write-then-delete because `.sh` and `.desktop` files embed display names and paths as plaintext content. A simple `fs::rename` on the file would leave stale content inside. Regenerate from scratch using `build_*_content()` functions, then delete old files.
- Profile names and launcher slugs are independent namespaces. A profile named `elden-ring` might have slug `god-of-war-ragnarok` if the display name was overridden. Always derive slugs from profile data, never from the profile file name.
- Profiles with `launch.method === "native"` never have launchers. All lifecycle operations must check and skip these profiles silently.
- The `LauncherExport.tsx` component has an `install` context mode that renders completely different UI. All launcher management UI (status badge, delete/rename buttons) must only appear in `context === 'default'`.
- File `src-tauri/src/lib.rs` is modified by multiple tasks (1.4 and 2.3 for command registration). Coordinate to avoid merge conflicts --add all Phase 1 commands in Task 1.4, all Phase 2 commands in Task 2.3.
- The frontend needs launcher slugs but `sanitize_launcher_slug` only exists in Rust. All `launcher_store` public functions accept profile-level field inputs (display_name, steam_app_id, trainer_path) and derive the slug internally. The returned result structs include the derived `launcher_slug`. Never port slug logic to TypeScript -- keep derivation in Rust as the single source of truth.
- **Cross-crate visibility**: Functions elevated to `pub(crate)` in `launcher.rs` are visible within `crosshook-core` but NOT from `src-tauri`. The `launcher_store` module exposes `pub` facade functions (like `delete_launcher_for_profile`) that Tauri commands call. Never import `pub(crate)` functions directly in Tauri command handlers.
- Slug collision is an inherent limitation: two profiles producing the same slug (e.g., "Elden Ring!" and "Elden Ring?") share launcher files. Deleting one removes the other's launcher. This is documented and accepted per the feature spec.
