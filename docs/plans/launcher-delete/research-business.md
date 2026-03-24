# Business Logic Research: launcher-delete

## Executive Summary

CrossHook currently exports launcher files (a `.sh` script and a `.desktop` entry) from profiles but provides no lifecycle management -- deleting or renaming a profile leaves orphaned launcher files on disk, and there is no UI for managing existing launchers. This feature closes the gap by cascading profile lifecycle events to their associated launcher artifacts and adding manual launcher management controls to the Launcher Export panel.

## User Stories

### Primary User: Game Player / Steam Deck User

- As a player who deletes a profile, I want the associated launcher script and desktop entry to be cleaned up automatically so that my application menu and launcher directory do not accumulate stale shortcuts.
- As a player who renames a profile, I want the associated launcher files to be renamed and their internal display name updated so that the shortcuts remain accurate and findable.
- As a player managing multiple game profiles, I want to see which launchers exist for the current profile and be able to delete or rename them manually in case automatic cleanup missed something or I changed my mind.
- As a Steam Deck user, I want orphaned `.desktop` entries removed so that my Game Mode library stays clean and only shows launchers for games I actually have configured.

## Business Rules

### Core Rules

1. **Profile Deletion Cascades to Launcher Cleanup**
   - When a profile is deleted, the system must check whether launcher artifacts exist for that profile and delete them.
   - Validation: The system must derive the expected launcher file paths from the profile data before deletion and verify they exist on disk.
   - Exception: If the profile was never exported (no launcher exists), deletion proceeds without launcher cleanup and is not considered an error.

2. **Profile Rename Cascades to Launcher Rename**
   - When a profile is saved under a new name (effectively a rename), the system must rename both launcher files and update internal content (display name in the `.desktop` entry `Name=` field, script comment header) to match the new identity.
   - Validation: Old launcher files must exist before attempting rename; the new launcher slug must not collide with an existing launcher from a different profile.
   - Exception: If no launcher exists under the old name, the rename proceeds silently -- this is a profile-only rename.

3. **Launcher Path Derivation Must Be Deterministic and Consistent**
   - Launcher file paths are derived from the profile's display name via `sanitize_launcher_slug`. The same derivation logic must be used for both export and lifecycle operations so the system can always locate existing launchers.
   - The canonical paths are:
     - Script: `~/.local/share/crosshook/launchers/{slug}-trainer.sh`
     - Desktop entry: `~/.local/share/applications/crosshook-{slug}-trainer.desktop`
   - The derivation chain is: `launcher_name` -> `sanitize_launcher_slug()` -> file paths.

4. **Manual Launcher Management Is Always Available**
   - The Launcher Export panel must expose delete and rename actions for any launcher associated with the current profile, regardless of whether the profile was just exported or loaded from a previous session.
   - Validation: The system should check if launcher files exist on disk before showing management actions.

5. **Launcher Deletion Is Atomic**
   - Both the script and the desktop entry must be deleted together. If one fails (e.g., permission error), the operation should report the failure and not leave the system in a half-deleted state.
   - Best effort: If one file is already missing, delete the other and report partial success.

6. **No Confirmation Dialog for Automatic Cleanup**
   - When a profile delete triggers launcher cleanup, no additional confirmation is needed -- the user already confirmed the profile deletion.
   - Manual launcher deletion from the Launcher Export panel should show a confirmation prompt since it is a destructive action initiated directly by the user.

### Edge Cases

- **Profile deleted but launcher script was manually moved or renamed**: The system attempts deletion at the canonical paths. If the files are not found, the deletion is a no-op (not an error). The system cannot track manually relocated files.
- **Profile renamed but launcher was exported under the old name with a custom `launcher_name` override**: The launcher slug is derived from the `launcher_name` field (or fallback chain), not the profile name directly. If the user overrode the launcher name during export, the system needs to use the same derivation to find the old files. This means the profile must store or the system must re-derive the launcher slug from the same inputs.
- **Multiple profiles export launchers with the same slug**: This is possible if two profiles have the same game name. The system should warn but not prevent this. During profile deletion, only the launcher matching the deleted profile's derivation chain should be removed.
- **Launcher exported, then profile fields changed without re-export**: The launcher on disk reflects the old export state. The system must derive paths from the profile data at the time of the lifecycle event, which may not match what is on disk. This is a fundamental limitation -- the system should attempt cleanup using current profile data.
- **Target home path varies between exports**: The launcher paths depend on `resolve_target_home_path()`, which can resolve differently depending on environment. The system should use the same resolution logic during cleanup as during export.
- **Profile has `native` launch method**: Launcher export only supports `steam_applaunch` and `proton_run`. Profiles with `native` method never have launchers, so lifecycle events should skip launcher cleanup for native profiles.
- **Permission denied on launcher files**: The script is written with mode `0o755` and the desktop entry with `0o644`. Deletion should handle permission errors gracefully and report them to the user.

## Workflows

### Primary Workflow: Profile Deletion with Launcher Cleanup

1. User clicks "Delete" in the Profile Editor panel.
2. System invokes `profile_delete` Tauri command with the profile name.
3. Backend loads the profile data from the TOML store (before deleting it) to derive launcher paths.
4. Backend derives the launcher slug using the same `resolve_display_name` -> `sanitize_launcher_slug` chain used during export.
5. Backend resolves the target home path using `resolve_target_home_path`.
6. Backend constructs the expected script path and desktop entry path.
7. Backend attempts to delete both files. Missing files are silently ignored.
8. Backend deletes the profile TOML file.
9. Backend clears `last_used_profile` in settings if it matches the deleted profile (existing behavior).
10. Frontend refreshes the profile list and selects the next available profile (existing behavior).

**Decision point**: Should profile deletion fail if launcher cleanup fails? Recommendation: No. Profile deletion should succeed even if launcher cleanup encounters errors. Launcher cleanup errors should be surfaced as warnings.

### Primary Workflow: Profile Rename with Launcher Update

1. User loads an existing profile, changes the profile name field, and clicks "Save".
2. System detects this is a rename (old profile name differs from new profile name).
3. Backend loads the old profile to derive the old launcher slug and paths.
4. Backend derives the new launcher slug from the new profile/display name.
5. If old launcher files exist on disk:
   a. Backend generates new launcher file content using the new display name.
   b. Backend writes the new launcher files at the new paths.
   c. Backend deletes the old launcher files.
6. Backend saves the profile under the new name.
7. Backend deletes the old profile TOML file.
8. Frontend refreshes the profile list.

**Decision point**: The current `useProfile` hook does not track "old name vs. new name" -- it only knows the current `profileName`. A rename is currently indistinguishable from "create new + manually delete old". The system may need to introduce explicit rename semantics or detect the rename at save time.

### Manual Launcher Management

1. User navigates to the Launcher Export panel for a non-native profile.
2. System checks if launcher files exist on disk for the current profile's derived slug.
3. If launchers exist, the panel shows "Delete Launcher" and "Rename Launcher" buttons alongside the export button.
4. User clicks "Delete Launcher".
5. System shows a confirmation prompt: "Delete the launcher script and desktop entry for {display_name}?"
6. On confirmation, system invokes a new `delete_launchers` Tauri command.
7. Backend deletes both files and returns success/failure.
8. Frontend updates the panel to reflect that no launcher exists.

### Error Recovery

- **Launcher file not found during cleanup**: Treat as success. The file was already gone (manually deleted, never exported, etc.).
- **Permission denied during cleanup**: Report error to the user. Suggest checking file ownership. Do not block profile deletion.
- **Launcher rename results in slug collision**: Report error. Suggest the user manually delete the conflicting launcher first.
- **Partial deletion (one file deleted, one failed)**: Report which file could not be deleted. The user can retry or manually remove it.

## Domain Model

### Key Entities

- **GameProfile**: The central configuration unit. Stored as a TOML file in `~/.config/crosshook/profiles/{name}.toml`. Contains all game, trainer, Steam, runtime, and launch configuration. The `steam.launcher.display_name` and `steam.launcher.icon_path` fields are the primary inputs for launcher export.

- **Launcher Artifact (Script)**: A bash script at `~/.local/share/crosshook/launchers/{slug}-trainer.sh` (mode `0o755`). Contains hardcoded paths from the profile at export time. The display name appears in the comment header.

- **Launcher Artifact (Desktop Entry)**: A `.desktop` file at `~/.local/share/applications/crosshook-{slug}-trainer.desktop` (mode `0o644`). Contains `Name=`, `Comment=`, `Exec=`, and `Icon=` fields derived from the profile. This is what desktop environments and Steam Deck Game Mode surface as a launchable application.

- **Launcher Slug**: A deterministic, URL-safe, lowercased identifier derived from the display name via `sanitize_launcher_slug()`. This is the link between a profile and its launcher artifacts.

- **SteamExternalLauncherExportResult**: The return value of a successful export. Contains `display_name`, `launcher_slug`, `script_path`, and `desktop_entry_path`. This structure is a natural fit for the "does a launcher exist?" query.

### Relationships

```
GameProfile 1 --- 0..1 Launcher (Script + Desktop Entry)
    |                       |
    |-- display_name ------>|-- slug (derived)
    |-- launcher fields --->|-- file content
    |-- profile name        |-- file paths (derived from slug + home path)
```

A profile may have zero or one launcher pair. The relationship is implicit -- derived from the profile's fields at export time, not stored as a foreign key.

### State Transitions

- **No Launcher -> Launcher Exists**: User exports a launcher from the Launcher Export panel. (`export_launchers` writes both files.)
- **Launcher Exists -> No Launcher**: User deletes launcher manually from the panel, or the system deletes it during profile deletion.
- **Launcher Exists -> Launcher Updated**: User re-exports the launcher (current behavior: overwrites in place). Or the system renames launcher files during a profile rename.
- **Launcher Exists -> Orphaned Launcher**: User deletes the profile but launcher cleanup fails or is not implemented (current state).

### Lifecycle Events

| Profile Event                              | Launcher Action                                            |
| ------------------------------------------ | ---------------------------------------------------------- |
| Profile created                            | No launcher action                                         |
| Profile saved (same name)                  | No automatic launcher action (user may re-export manually) |
| Profile saved (new name, old name existed) | Rename launcher files + update content                     |
| Profile deleted                            | Delete launcher files                                      |
| Launcher exported                          | Creates/overwrites launcher files                          |
| Launcher manually deleted                  | Removes launcher files                                     |
| Launcher manually renamed                  | Renames launcher files + updates content                   |

## Existing Codebase Integration

### Related Features

- `/src/crosshook-native/crates/crosshook-core/src/export/launcher.rs`: The existing launcher export implementation. Contains `export_launchers()`, `validate()`, `sanitize_launcher_slug()`, `resolve_display_name()`, `resolve_target_home_path()`, and file-writing helpers. New delete/rename functions should live here.
- `/src/crosshook-native/crates/crosshook-core/src/export/mod.rs`: Re-exports from `launcher.rs`. New public functions must be added here.
- `/src/crosshook-native/crates/crosshook-core/src/profile/toml_store.rs`: `ProfileStore` with `save()`, `load()`, `delete()`, `list()`. Profile deletion is a plain `fs::remove_file` with no lifecycle hooks. This is where launcher cleanup should be triggered or coordinated.
- `/src/crosshook-native/crates/crosshook-core/src/profile/models.rs`: `GameProfile`, `LauncherSection` (contains `icon_path` and `display_name`), `SteamSection`, `LaunchSection`. These models provide the inputs for launcher path derivation.
- `/src/crosshook-native/src-tauri/src/commands/profile.rs`: Tauri commands `profile_delete`, `profile_save`. The `profile_delete` command is a thin wrapper around `ProfileStore::delete()` -- it currently does not load the profile first or perform any cleanup. This is the integration point for automatic launcher cleanup.
- `/src/crosshook-native/src-tauri/src/commands/export.rs`: Tauri commands `export_launchers`, `validate_launcher_export`. New commands for delete/rename should be added here.
- `/src/crosshook-native/src/components/LauncherExport.tsx`: The React component for launcher export. Currently only supports export and reset. Must be extended with delete/rename controls and launcher existence detection.
- `/src/crosshook-native/src/components/ProfileEditor.tsx`: Contains the delete button that triggers `deleteProfile()` from the `useProfile` hook. The hook's `deleteProfile` function calls `profile_delete` and `settings_save`.
- `/src/crosshook-native/src/hooks/useProfile.ts`: The `deleteProfile()` callback invokes `profile_delete`, clears `last_used_profile`, refreshes the profile list, and selects the next profile. Launcher cleanup should be integrated into this flow.
- `/src/crosshook-native/src-tauri/src/lib.rs`: Tauri command registration. New commands must be added to the `invoke_handler` array.

### Patterns to Follow

- **Tauri command pattern**: Thin command wrappers in `src-tauri/src/commands/` that delegate to `crosshook-core` functions and map errors to `String` via `.map_err(|e| e.to_string())`. See `commands/export.rs` and `commands/profile.rs`.
- **Error type pattern**: Domain-specific error enums with `Display` and `Error` implementations, plus `From<io::Error>` conversions. See `SteamExternalLauncherExportError` and `ProfileStoreError`.
- **Request/Result pattern**: Dedicated request structs (`SteamExternalLauncherExportRequest`) and result structs (`SteamExternalLauncherExportResult`) with serde derives for IPC. New delete/rename operations should follow this.
- **Slug derivation pattern**: `resolve_display_name()` -> `sanitize_launcher_slug()` -> path construction. This chain must be reused, not duplicated.
- **File writing pattern**: `write_host_text_file()` with explicit Unix permissions. Deletion should use a symmetric `delete_host_file()` helper.
- **Frontend invoke pattern**: `invoke<ResultType>('command_name', { params })` wrapped in async handlers with try/catch and state management. See `LauncherExport.tsx` `handleExport()`.
- **State management pattern**: The `useProfile` hook manages all profile state. Launcher state (exists/not) could be managed in `LauncherExport.tsx` component-locally or lifted to a new hook.

### Components to Leverage

- **`sanitize_launcher_slug()`** (`export/launcher.rs`): Already public. Reuse for deriving paths during delete/rename.
- **`resolve_display_name()`** (`export/launcher.rs`): Currently private. Should be made public (or a new `derive_launcher_paths()` function should encapsulate the full derivation).
- **`resolve_target_home_path()`** (`export/launcher.rs`): Already public. Needed to reconstruct launcher file paths.
- **`write_host_text_file()`** (`export/launcher.rs`): Currently private. A symmetric `delete_host_file()` is needed.
- **`build_desktop_entry_content()`** and **`build_trainer_script_content()`** (`export/launcher.rs`): Currently private. Needed for rename (regenerate content with new display name). Should be made accessible or a higher-level rename function should encapsulate this.
- **`ProfileStore::load()`** (`profile/toml_store.rs`): Must be called before `ProfileStore::delete()` to capture the profile data needed for launcher path derivation.
- **`SteamExternalLauncherExportResult`** (`export/launcher.rs`): Its shape (`display_name`, `launcher_slug`, `script_path`, `desktop_entry_path`) is the natural return type for a "resolve launcher paths" query.

### Data Model Gaps

- There is no stored mapping between a profile and its exported launcher paths. The relationship is implicit and derived. This means:
  - The system must re-derive launcher paths from the profile data to find them.
  - If the profile data has changed since the last export, the derived paths may not match the actual files on disk.
  - A future enhancement could store the last-exported launcher paths in the profile or in a separate metadata file, but this is not strictly necessary for the initial implementation.

## Success Criteria

- [ ] Deleting a profile automatically removes associated launcher files (`.sh` script and `.desktop` entry) when they exist
- [ ] Deleting a profile succeeds even when no launcher was ever exported
- [ ] Renaming a profile updates associated launcher files (paths and internal content) when they exist
- [ ] The Launcher Export panel shows delete/rename actions when a launcher exists for the current profile
- [ ] Manual launcher deletion requires user confirmation
- [ ] Launcher operations report clear error messages on failure (permission denied, etc.)
- [ ] Native-method profiles skip launcher lifecycle operations entirely
- [ ] Existing Rust tests pass and new tests cover the delete/rename logic in `crosshook-core`
- [ ] The launcher slug derivation is consistent between export, delete, and rename operations

## Open Questions

1. **Should rename be explicit or implicit?** The current `useProfile` hook does not distinguish "save under new name" from "create new profile." Should the system introduce an explicit `profile_rename` operation, or detect the rename from the frontend by comparing old and new names?

2. **Should the profile store its last-exported launcher slug?** Storing the slug (or full paths) in the profile TOML would make lookup reliable even if profile fields change. However, it adds a new field to the data model and requires migration for existing profiles. The alternative is always re-deriving from current fields, accepting that changed fields may cause a missed cleanup.

3. **Should launcher cleanup be synchronous or async?** The current export is synchronous (blocking Rust). Cleanup is fast (two `fs::remove_file` calls), so synchronous is likely fine, but the Tauri command layer uses `tokio` -- should cleanup be spawned as a background task to avoid blocking the UI?

4. **Should the system scan for orphaned launchers?** Beyond per-profile cleanup, should there be a "clean up all orphaned launchers" utility that scans `~/.local/share/crosshook/launchers/` and `~/.local/share/applications/crosshook-*` for files that no longer have a matching profile? This could be a separate feature or a settings-panel action.

5. **Confirmation UX for automatic cleanup**: The recommendation is no extra confirmation for profile-delete-triggered cleanup. Should the user at least see a message like "Profile and associated launcher deleted" to know cleanup happened?
