# Technical Specifications: launcher-delete

## Executive Summary

Launcher lifecycle management adds automatic cleanup and renaming of exported `.sh` scripts and `.desktop` entries when profiles are deleted or renamed, plus manual launcher management controls in the UI. The core approach is a new `LauncherStore` in `crosshook-core` that discovers, deletes, and renames launchers by deriving file paths from launcher slugs, with Tauri commands that cascade launcher operations during profile mutations and expose standalone management endpoints. No new persistence layer or tracking database is needed because launcher file paths are deterministically derived from the display name (via `sanitize_launcher_slug`).

## Architecture Design

### Component Diagram

```
ProfileEditor.tsx                  LauncherExport.tsx
  |                                    |
  | profile_delete / profile_rename    | export_launchers
  v                                    v
commands/profile.rs  <---------->  commands/export.rs
  |                                    |
  | cascades via                       | uses
  v                                    v
export::launcher_store  <------  export::launcher (existing)
  |
  | fs operations: list, delete, rename
  v
~/.local/share/crosshook/launchers/{slug}-trainer.sh
~/.local/share/applications/crosshook-{slug}-trainer.desktop
```

### New Components

- **`export::launcher_store` (Rust module)**: Pure-logic layer for discovering, deleting, and renaming launcher file pairs. Stateless -- derives paths from a home directory and launcher slug. Lives alongside the existing `export::launcher` module.
- **Cascading profile commands**: `profile_delete` and new `profile_rename` Tauri commands invoke `launcher_store` functions to clean up associated launcher files.
- **Launcher management UI section**: Extension to `LauncherExport.tsx` that shows existing launcher status and provides delete/rename controls.

### Integration Points

- **`ProfileStore::delete` -> `LauncherStore::delete_launchers_for_profile`**: When a profile is deleted, the Tauri `profile_delete` command resolves the profile's launcher slug and calls the launcher store to remove the script and desktop entry.
- **`profile_rename` (new) -> `LauncherStore::rename_launcher`**: When a profile is renamed, the old slug is derived, the new slug is computed, and both files are moved. The `.desktop` file content is rewritten with the new display name and updated script path reference.
- **`export_launchers` -> `LauncherStore::check_launcher_exists`**: After export, the UI can query whether launchers exist for the current profile.

## Data Models

### Launcher Tracking

Launchers are **not tracked in a database or registry**. They are discovered on the filesystem using deterministic path conventions. This is the correct approach because:

1. The `sanitize_launcher_slug` function in `launcher.rs` (line 243) is a pure, deterministic transformation from display name to slug.
2. The file paths follow a strict convention: `~/.local/share/crosshook/launchers/{slug}-trainer.sh` and `~/.local/share/applications/crosshook-{slug}-trainer.desktop`.
3. Checking for file existence is cheap and authoritative -- if the file exists, the launcher exists.

The relationship chain is: **Profile** -> **display_name** (from `steam.launcher.display_name`, or derived from game name / trainer path) -> **launcher_slug** (via `sanitize_launcher_slug`) -> **file paths**.

### File Path Convention

Given a launcher slug (e.g., `elden-ring-deluxe`) and a resolved home path:

| File          | Path Pattern                                                        |
| ------------- | ------------------------------------------------------------------- |
| Shell script  | `{home}/.local/share/crosshook/launchers/{slug}-trainer.sh`         |
| Desktop entry | `{home}/.local/share/applications/crosshook-{slug}-trainer.desktop` |

The slug derivation chain (from `launcher.rs` lines 220-271):

1. `resolve_display_name(launcher_name, steam_app_id, trainer_path)` picks the first non-empty value
2. `sanitize_launcher_slug(display_name)` lowercases, replaces non-alphanumeric runs with `-`, trims leading/trailing `-`

### Launcher Info Model (new)

```rust
/// Summary of an exported launcher discovered on disk.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LauncherInfo {
    pub display_name: String,
    pub launcher_slug: String,
    pub script_path: String,
    pub desktop_entry_path: String,
    pub script_exists: bool,
    pub desktop_entry_exists: bool,
}
```

This reuses the same shape as `SteamExternalLauncherExportResult` but adds existence booleans. On the TypeScript side:

```typescript
interface LauncherInfo {
  display_name: string;
  launcher_slug: string;
  script_path: string;
  desktop_entry_path: string;
  script_exists: boolean;
  desktop_entry_exists: boolean;
}
```

## API Design (Tauri IPC)

### New Commands

#### `check_launcher_exists`

**Purpose**: Check whether launcher files exist for a given profile's launcher slug.

```rust
#[tauri::command]
pub fn check_launcher_exists(
    launcher_slug: String,
    target_home_path: String,
) -> Result<LauncherInfo, String>
```

**Request**: `{ launcher_slug: string, target_home_path: string }`
**Response**: `LauncherInfo` object with `script_exists` and `desktop_entry_exists` booleans.
**Frontend call**: `invoke<LauncherInfo>('check_launcher_exists', { launcherSlug, targetHomePath })`

#### `delete_launcher`

**Purpose**: Delete the `.sh` script and `.desktop` entry for a given launcher slug.

```rust
#[tauri::command]
pub fn delete_launcher(
    launcher_slug: String,
    target_home_path: String,
) -> Result<LauncherDeleteResult, String>
```

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LauncherDeleteResult {
    pub script_deleted: bool,
    pub desktop_entry_deleted: bool,
    pub script_path: String,
    pub desktop_entry_path: String,
}
```

**Frontend call**: `invoke<LauncherDeleteResult>('delete_launcher', { launcherSlug, targetHomePath })`

#### `rename_launcher`

**Purpose**: Rename launcher files from one slug to another, updating internal content (display name in `.desktop`, comment line in `.sh`).

```rust
#[tauri::command]
pub fn rename_launcher(
    old_launcher_slug: String,
    new_display_name: String,
    new_launcher_icon_path: String,
    target_home_path: String,
) -> Result<LauncherRenameResult, String>
```

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LauncherRenameResult {
    pub old_slug: String,
    pub new_slug: String,
    pub new_script_path: String,
    pub new_desktop_entry_path: String,
    pub script_renamed: bool,
    pub desktop_entry_renamed: bool,
}
```

The rename operation:

1. Derives `new_slug` from `new_display_name` via `sanitize_launcher_slug`.
2. If `old_slug == new_slug`, rewrites content in place (display name may differ even if slug is the same).
3. If slugs differ, writes new files with updated content and deletes old files.
4. The `.sh` script header comment line (`# {display_name} - Trainer launcher`) is updated.
5. The `.desktop` entry `Name=`, `Comment=`, and `Exec=` fields are rewritten.

**Frontend call**: `invoke<LauncherRenameResult>('rename_launcher', { oldLauncherSlug, newDisplayName, newLauncherIconPath, targetHomePath })`

#### `list_launchers`

**Purpose**: Scan the `~/.local/share/crosshook/launchers/` directory for all CrossHook-generated launcher scripts and return their metadata.

```rust
#[tauri::command]
pub fn list_launchers(
    target_home_path: String,
) -> Result<Vec<LauncherInfo>, String>
```

Discovery logic:

1. Read all `*-trainer.sh` files in `{home}/.local/share/crosshook/launchers/`.
2. For each script, derive the slug by stripping the `-trainer.sh` suffix.
3. Check if a matching `crosshook-{slug}-trainer.desktop` exists in `{home}/.local/share/applications/`.
4. Extract display name from the script's comment header line (`# {name} - Trainer launcher`).

### Modified Commands

#### `profile_delete` (cascade launcher deletion)

The existing `profile_delete` Tauri command at `commands/profile.rs` line 28 should be extended:

```rust
#[tauri::command]
pub fn profile_delete(
    name: String,
    store: State<'_, ProfileStore>,
) -> Result<(), String> {
    // 1. Load the profile to get the launcher display name before deleting
    let profile = store.load(&name).ok(); // best-effort; profile may be corrupt

    // 2. Delete the profile file (existing behavior)
    store.delete(&name).map_err(map_error)?;

    // 3. Best-effort launcher cleanup (don't fail the delete if this fails)
    if let Some(profile) = profile {
        let display_name = resolve_launcher_display_name(&profile, &name);
        let slug = sanitize_launcher_slug(&display_name);
        let home = resolve_target_home_path("", "");
        if !home.is_empty() {
            let _ = delete_launcher_files(&slug, &home);
        }
    }

    Ok(())
}
```

Key design decision: Launcher cleanup on profile delete is **best-effort**. If the launcher files cannot be found or deleted (permissions, moved, etc.), the profile deletion still succeeds. This avoids blocking a user from deleting a profile because of a filesystem issue in an unrelated directory.

#### `profile_save` (handle renames)

The current `profile_save` does not distinguish between "create" and "rename" because profiles are identified by name (the filename stem). A rename in the current UI is actually: user changes the profile name text field while an existing profile is loaded, then saves.

The frontend `useProfile` hook should detect when `profileName !== selectedProfile && selectedProfile !== ''` (name changed from a loaded profile) and call `profile_rename` before or instead of `profile_save`.

#### `profile_rename` (new Tauri command)

```rust
#[tauri::command]
pub fn profile_rename(
    old_name: String,
    new_name: String,
    store: State<'_, ProfileStore>,
) -> Result<(), String> {
    // 1. Load the old profile
    let profile = store.load(&old_name).map_err(map_error)?;

    // 2. Save under new name
    store.save(&new_name, &profile).map_err(map_error)?;

    // 3. Delete old profile file
    store.delete(&old_name).map_err(map_error)?;

    // 4. Best-effort launcher rename
    let old_display_name = resolve_launcher_display_name(&profile, &old_name);
    let old_slug = sanitize_launcher_slug(&old_display_name);
    let new_display_name = resolve_launcher_display_name(&profile, &new_name);
    let home = resolve_target_home_path("", "");
    if !home.is_empty() {
        let _ = rename_launcher_files(&old_slug, &new_display_name, "", &home);
    }

    Ok(())
}
```

## System Constraints

### File System

- **Atomic rename**: Use `std::fs::rename` for same-filesystem moves (guaranteed on Linux for same mount point). Both `~/.local/share/crosshook/launchers/` and `~/.local/share/applications/` are under the same `~/.local/share/` tree, so `rename(2)` will be atomic.
- **Safe delete**: Use `std::fs::remove_file`. Check existence before deleting; treat `NotFound` as success (idempotent). Log other errors.
- **Content rewrite for rename**: The `.desktop` entry and `.sh` script header contain the display name as plaintext. On rename, these must be rewritten, not just moved. Strategy: write new files with updated content, then delete old files. This is safer than in-place editing.
- **Directory creation**: `delete_launcher` should not create directories. `rename_launcher` should create the target directories if they do not exist (same as `export_launchers` does today).

### Permissions

- **`~/.local/share/applications/`**: Standard XDG directory. User-writable by convention. Desktop environments read `.desktop` files here. No special permissions needed beyond standard user ownership.
- **`~/.local/share/crosshook/launchers/`**: CrossHook-owned directory. Already created by `export_launchers` with `mkdir -p` semantics.
- **Script permissions**: `.sh` files are written with `0o755` mode. On rename/rewrite, the new file must preserve this mode.
- **Desktop entry permissions**: `.desktop` files are written with `0o644` mode. Same preservation requirement.

### Desktop Environment Interaction

- **What happens if a `.desktop` file is in use**: Desktop environments (GNOME, KDE, Steam Deck's Gamescope) re-scan `~/.local/share/applications/` periodically or on file change. Deleting or renaming a `.desktop` file while the DE has it cached may show a stale entry until the next scan. This is standard Linux desktop behavior and not something CrossHook can control. The operation is safe -- there is no file locking.
- **`update-desktop-database`**: Some DEs require running `update-desktop-database ~/.local/share/applications/` after modifying `.desktop` files to update the MIME cache. This is optional and can be offered as a post-operation step. For Steam Deck (Gamescope), this is not needed.

### Handling Exported Scripts in Custom Locations

The current implementation always exports to the deterministic paths under `~/.local/share/`. There is no mechanism for exporting to custom locations. If this is added in the future, launcher tracking would need to be persisted (e.g., in a TOML registry). For now, the deterministic path convention is sufficient and should be documented as the contract.

### Edge Case: Slug Collision

If two profiles produce the same launcher slug (e.g., "Elden Ring!" and "Elden Ring?" both become `elden-ring`), deleting one profile will remove the launcher for both. This is an existing limitation of the slug-based approach. The `LauncherDeleteResult` and `LauncherRenameResult` return the actual paths affected, so the UI can warn the user. A future enhancement could add a profile-to-slug mapping registry, but this is out of scope for the initial implementation.

## Codebase Changes

### Files to Create

- **`src/crosshook-native/crates/crosshook-core/src/export/launcher_store.rs`**: New module containing `check_launcher_exists`, `delete_launcher_files`, `rename_launcher_files`, `list_launchers`, and the `LauncherInfo`, `LauncherDeleteResult`, `LauncherRenameResult` types. This module imports `sanitize_launcher_slug` and path helpers from the sibling `launcher` module.

### Files to Modify

- **`src/crosshook-native/crates/crosshook-core/src/export/mod.rs`**: Add `pub mod launcher_store;` and re-export the new public types.
- **`src/crosshook-native/crates/crosshook-core/src/export/launcher.rs`**: Make `resolve_display_name`, `combine_host_unix_path`, `build_desktop_entry_content`, `write_host_text_file`, and `resolve_desktop_icon_value` `pub(crate)` so `launcher_store` can reuse them. Currently they are private (`fn`). Also expose `build_trainer_script_content` as `pub(crate)` for rewriting scripts during rename.
- **`src/crosshook-native/src-tauri/src/commands/export.rs`**: Add `check_launcher_exists`, `delete_launcher`, `rename_launcher`, and `list_launchers` Tauri command handlers.
- **`src/crosshook-native/src-tauri/src/commands/profile.rs`**: Add `profile_rename` Tauri command. Modify `profile_delete` to cascade launcher deletion (best-effort).
- **`src/crosshook-native/src-tauri/src/commands/mod.rs`**: No change needed (commands are already grouped by `export` and `profile` modules).
- **`src/crosshook-native/src-tauri/src/lib.rs`**: Register new Tauri commands in `invoke_handler`: `check_launcher_exists`, `delete_launcher`, `rename_launcher`, `list_launchers`, `profile_rename`.
- **`src/crosshook-native/src/components/LauncherExport.tsx`**: Add launcher status section showing whether launchers exist, with "Delete Launcher" and "Rename Launcher" buttons. Add state management for these operations.
- **`src/crosshook-native/src/hooks/useProfile.ts`**: Detect rename scenario (profile name changed from loaded profile), invoke `profile_rename` instead of save+delete.
- **`src/crosshook-native/src/types/index.ts`**: No change needed (re-exports wildcard).

### Dependencies

No new dependencies required. All filesystem operations use `std::fs`. The `directories` crate (already a dependency) provides the home path fallback.

## Technical Decisions

### Decision 1: Stateless vs. Tracked Launcher Registry

- **Options**: (A) Derive launcher paths from slug on every operation (stateless). (B) Maintain a TOML registry mapping profile names to exported launcher paths.
- **Recommendation**: (A) Stateless derivation.
- **Rationale**: The path convention is already deterministic and stable. Adding a registry creates a synchronization problem (registry vs. filesystem drift). The stateless approach is simpler, has no migration concern, and works identically for the CLI. A registry can be added later if custom export paths are supported.

### Decision 2: Cascade Behavior on Profile Delete

- **Options**: (A) Fail-fast -- block profile deletion if launcher cleanup fails. (B) Best-effort -- delete profile, log launcher cleanup failures. (C) Two-phase -- check launcher existence first, confirm with user, then delete both.
- **Recommendation**: (B) Best-effort with UI notification.
- **Rationale**: Profile deletion is the primary intent. Launcher files are a derivative artifact. Users should never be blocked from managing profiles by launcher filesystem issues. The Tauri command should return a structured result indicating whether launcher cleanup succeeded, and the frontend can display a warning if it did not.

### Decision 3: Rename Strategy (Move vs. Rewrite)

- **Options**: (A) `fs::rename` the files and patch content in-place. (B) Write new files with correct content, then delete old files.
- **Recommendation**: (B) Write-then-delete.
- **Rationale**: Both the `.sh` script and `.desktop` entry embed the display name and paths as plaintext. Simple file renaming would leave stale content. Writing new files ensures content correctness and avoids partial-update risks. The operation is idempotent: if the new file already exists (from a prior export), it gets overwritten with correct content.

### Decision 4: Where to Resolve Home Path for Cascade Operations

- **Options**: (A) Pass `target_home_path` from the frontend for every cascade. (B) Resolve `$HOME` in the backend using `std::env::var("HOME")` or `directories::BaseDirs`.
- **Recommendation**: (B) Backend resolution for cascade operations, (A) Frontend-provided for manual operations.
- **Rationale**: The cascade on profile delete/rename happens in the backend without UI interaction. The backend already has `resolve_target_home_path` which falls back to `$HOME`. For manual delete/rename from the launcher panel, the frontend already computes `targetHomePath` and should continue to pass it explicitly. This is consistent with the existing `export_launchers` pattern.

### Decision 5: Profile Rename Approach

- **Options**: (A) Add a `rename` method to `ProfileStore` using `std::fs::rename`. (B) Save-as-new + delete-old at the Tauri command level.
- **Recommendation**: (A) Add a `rename` method to `ProfileStore`.
- **Rationale**: A single `rename` call is atomic on the same filesystem. The save+delete approach has a window where two copies exist, or where the delete could fail leaving a duplicate. The `ProfileStore::rename` method encapsulates the validation of both old and new names, the existence check for the old profile, and the conflict check for the new name.

```rust
// New method on ProfileStore
pub fn rename(&self, old_name: &str, new_name: &str) -> Result<(), ProfileStoreError> {
    let old_path = self.profile_path(old_name)?;
    let new_path = self.profile_path(new_name)?;

    if !old_path.exists() {
        return Err(ProfileStoreError::NotFound(old_path));
    }

    // fs::rename is atomic on Linux for same-mount paths
    fs::rename(&old_path, &new_path)?;
    Ok(())
}
```

Note: This renames the file but does not update internal profile content. If the profile's `game.name` or `steam.launcher.display_name` should also change, the caller (Tauri command) should load, mutate, and re-save after the rename.

## Open Questions

- **Should the launcher panel show launchers for all profiles, or only the currently loaded profile?** The `list_launchers` command returns all launchers in `~/.local/share/crosshook/launchers/`. The UI could show all with profile-name annotations, or filter to only the current profile's slug. Showing all provides better management capability.
- **Should rename cascade update the script's runtime paths (PREFIX_ROOT, PROTON, etc.) or only the display name?** The script content is derived from profile data at export time. A rename only changes the name, not the runtime configuration. Rewriting just the display name and file paths is correct. If the user wants to update runtime paths, they should re-export.
- **Should the CLI (`crosshook-cli`) gain launcher management subcommands?** The `launcher_store` module lives in `crosshook-core`, making it available to the CLI. Adding `crosshook launcher list`, `crosshook launcher delete`, `crosshook launcher rename` subcommands would be a natural extension but is separate scope.

## Relevant Files

| File                                                                   | Role                                                                                                                      |
| ---------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------- |
| `src/crosshook-native/crates/crosshook-core/src/export/launcher.rs`    | Existing export logic: `export_launchers`, `sanitize_launcher_slug`, path construction, script/desktop content generation |
| `src/crosshook-native/crates/crosshook-core/src/export/mod.rs`         | Module root: re-exports public API from `launcher`                                                                        |
| `src/crosshook-native/crates/crosshook-core/src/profile/toml_store.rs` | `ProfileStore` with `save`, `load`, `list`, `delete` methods                                                              |
| `src/crosshook-native/crates/crosshook-core/src/profile/models.rs`     | `GameProfile` and nested section structs including `LauncherSection`                                                      |
| `src/crosshook-native/src-tauri/src/commands/export.rs`                | Tauri commands: `export_launchers`, `validate_launcher_export`                                                            |
| `src/crosshook-native/src-tauri/src/commands/profile.rs`               | Tauri commands: `profile_save`, `profile_load`, `profile_list`, `profile_delete`                                          |
| `src/crosshook-native/src-tauri/src/lib.rs`                            | Tauri app setup, command registration, managed state                                                                      |
| `src/crosshook-native/src/components/LauncherExport.tsx`               | Frontend launcher export panel with export form and result display                                                        |
| `src/crosshook-native/src/components/ProfileEditor.tsx`                | Frontend profile editor with save/delete controls                                                                         |
| `src/crosshook-native/src/hooks/useProfile.ts`                         | Profile state management hook: `saveProfile`, `deleteProfile`, `refreshProfiles`                                          |
| `src/crosshook-native/src/types/profile.ts`                            | TypeScript types: `GameProfile`, `LaunchMethod`                                                                           |
