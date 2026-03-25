# Architecture Research: Launcher Lifecycle Management

## System Overview

CrossHook is a Tauri v2 application with a Rust workspace backend (`crosshook-core` library + `src-tauri` shell) and a React 18 + TypeScript frontend. Business logic lives entirely in `crosshook-core`; the Tauri layer (`src-tauri/src/commands/`) provides thin IPC wrappers that delegate to core functions and map errors to strings. The frontend uses custom React hooks for state management and invokes Tauri commands directly via `@tauri-apps/api/core`. Launcher export currently writes `.sh` scripts and `.desktop` entries to deterministic paths but provides zero lifecycle management -- no delete, rename, existence checking, or stale detection.

## Relevant Components

### Rust Backend -- crosshook-core

- `/src/crosshook-native/crates/crosshook-core/src/export/launcher.rs`: Contains all launcher export logic. The `export_launchers()` function validates, derives display name/slug, builds file paths, writes script+desktop files. Key helpers: `sanitize_launcher_slug()` (pub), `resolve_display_name()` (private), `combine_host_unix_path()` (private), `write_host_text_file()` (private), `build_trainer_script_content()` (private), `build_desktop_entry_content()` (private), `resolve_target_home_path()` (pub). New launcher_store module will need several of these elevated to `pub(crate)`.
- `/src/crosshook-native/crates/crosshook-core/src/export/mod.rs`: Module root for export. Currently only declares `pub mod launcher` and re-exports its public types. Must add `pub mod launcher_store` and new re-exports.
- `/src/crosshook-native/crates/crosshook-core/src/profile/toml_store.rs`: `ProfileStore` with CRUD operations (`load`, `save`, `list`, `delete`). No rename method exists -- `rename` must be added. Uses `~/.config/crosshook/profiles/` base path via `directories::BaseDirs`.
- `/src/crosshook-native/crates/crosshook-core/src/profile/models.rs`: All profile data models (`GameProfile`, `SteamSection`, `LauncherSection`, etc.). `LauncherSection` contains `icon_path` and `display_name` -- these are the inputs to slug derivation.
- `/src/crosshook-native/crates/crosshook-core/src/profile/mod.rs`: Module root for profile. Re-exports all public types from sub-modules.
- `/src/crosshook-native/crates/crosshook-core/src/settings/mod.rs`: `SettingsStore` and `AppSettingsData`. Follows identical store pattern (base_path, TOML persistence, `try_new`/`with_base_path`). The `last_used_profile` field will need updating on profile rename.
- `/src/crosshook-native/crates/crosshook-core/src/lib.rs`: Module root for all core modules (community, export, install, launch, logging, profile, settings, steam).

### Rust Backend -- Tauri Commands

- `/src/crosshook-native/src-tauri/src/commands/export.rs`: Two thin commands: `validate_launcher_export` and `export_launchers`. New commands (`check_launcher_exists`, `delete_launcher`, `rename_launcher`, `list_launchers`) will follow this same pattern.
- `/src/crosshook-native/src-tauri/src/commands/profile.rs`: Five commands: `profile_list`, `profile_load`, `profile_save`, `profile_delete`, `profile_import_legacy`. All take `State<'_, ProfileStore>`. `profile_delete` currently only deletes the TOML file -- must be extended to cascade launcher cleanup. `profile_rename` does not exist and must be added.
- `/src/crosshook-native/src-tauri/src/commands/settings.rs`: Settings CRUD commands. `settings_save` will be relevant for updating `last_used_profile` during rename.
- `/src/crosshook-native/src-tauri/src/commands/mod.rs`: Declares all command sub-modules.
- `/src/crosshook-native/src-tauri/src/lib.rs`: Tauri app setup. Registers all commands in `invoke_handler` macro, manages state (ProfileStore, SettingsStore, RecentFilesStore, CommunityTapStore). All new commands must be registered here.

### React Frontend

- `/src/crosshook-native/src/components/LauncherExport.tsx`: Renders the Launcher Export panel. Currently: name input, icon display, metadata rows, "Export Launcher" button, result display. No existence checking, no delete/rename UI. Receives `profile`, `method`, `steamClientInstallPath`, `targetHomePath` as props. Has both `default` and `install` context modes. Uses local state only (no hook extraction).
- `/src/crosshook-native/src/components/ProfileEditor.tsx`: Profile editing UI. Contains the "Delete" button that calls `deleteProfile()` from the useProfile hook. Also contains "Save" button. The component destructures the full `UseProfileResult` from props. This is where rename detection and cascade confirmation UI would live.
- `/src/crosshook-native/src/hooks/useProfile.ts`: Central profile state management hook. `saveProfile()` normalizes and saves via `profile_save` IPC. `deleteProfile()` calls `profile_delete` IPC then clears `last_used_profile` in settings. Neither method has any launcher awareness. `selectedProfile` vs `profileName` mismatch is how rename is detected (profileName changed while selectedProfile still holds the old value).
- `/src/crosshook-native/src/App.tsx`: Top-level orchestrator. Owns `profileState` via `useProfile()`, derives `steamClientInstallPath` and `targetHomePath`, passes them to both `ProfileEditorView` and `LauncherExport`. The `LauncherExport` component is conditionally rendered based on `shouldShowLauncherExport`. This is where derived launcher context (home path, method) is computed and threaded down.
- `/src/crosshook-native/src/types/profile.ts`: TypeScript interfaces for `GameProfile`, `LaunchMethod`. New launcher types (`LauncherInfo`, `LauncherDeleteResult`, `LauncherRenameResult`) should go in a new `types/launcher.ts` or be added here.
- `/src/crosshook-native/src/types/settings.ts`: `AppSettingsData` interface -- `last_used_profile` field relevant for rename cascade.
- `/src/crosshook-native/src/types/index.ts`: Re-exports all type modules.

## Data Flow

### Current Export Flow

```
User clicks "Export Launcher" in LauncherExport.tsx
  -> buildExportRequest() assembles SteamExternalLauncherExportRequest from profile + derived paths
  -> invoke('validate_launcher_export', { request })
     -> commands/export.rs::validate_launcher_export
        -> crosshook_core::export::validate(&request)
  -> invoke('export_launchers', { request })
     -> commands/export.rs::export_launchers
        -> crosshook_core::export::export_launchers(&request)
           -> validate(request)
           -> resolve_display_name(launcher_name, steam_app_id, trainer_path)
           -> sanitize_launcher_slug(display_name)
           -> resolve_target_home_path(target_home_path, steam_client_install_path)
           -> combine_host_unix_path(home, ".local/share/crosshook/launchers", "{slug}-trainer.sh")
           -> combine_host_unix_path(home, ".local/share/applications", "crosshook-{slug}-trainer.desktop")
           -> write_host_text_file(script_path, script_content, 0o755)
           -> write_host_text_file(desktop_path, desktop_content, 0o644)
           -> returns SteamExternalLauncherExportResult { display_name, launcher_slug, script_path, desktop_entry_path }
  -> setResult(exported) -- shows paths in UI
```

### Current Profile Delete Flow

```
User clicks "Delete" in ProfileEditor.tsx
  -> useProfile.deleteProfile()
     -> invoke('profile_delete', { name })
        -> commands/profile.rs::profile_delete
           -> ProfileStore::delete(&name)  -- only deletes the .toml file
     -> clears last_used_profile if it matches
     -> refreshes profile list
     -> selects next profile
  (NO launcher cleanup happens)
```

### Current Profile Save Flow (Rename Detection Point)

```
User changes profileName and clicks "Save" in ProfileEditor.tsx
  -> useProfile.saveProfile()
     -> invoke('profile_save', { name: profileName, data: normalizedProfile })
        -> commands/profile.rs::profile_save
           -> ProfileStore::save(&name, &profile)  -- writes new .toml
     -> syncProfileMetadata(name, normalizedProfile)  -- updates last_used_profile, recent files
     -> refreshProfiles()
     -> loadProfile(name)
  (Old profile .toml still exists if name changed; NO launcher rename happens)
```

### Proposed New Flows

**Profile Delete with Cascade**: `profile_delete` command loads profile data first, derives launcher slug, attempts best-effort launcher file deletion, then deletes profile TOML. Launcher cleanup failure does not block profile deletion.

**Profile Rename with Cascade**: New `profile_rename` command or enhanced save flow. Frontend detects rename when `profileName !== selectedProfile`. Backend: rename profile TOML via `fs::rename`, derive old/new slugs, write new launcher files, delete old launcher files.

**Manual Launcher Management**: `check_launcher_exists` called on mount/profile-change to show status badge. `delete_launcher` and `rename_launcher` callable from LauncherExport UI.

## Integration Points

### New Rust Module: `export/launcher_store.rs`

Must introduce all new types (`LauncherInfo`, `LauncherDeleteResult`, `LauncherRenameResult`) and functions (`check_launcher_exists`, `delete_launcher_files`, `rename_launcher_files`, `list_launchers`). This module reuses slug derivation and path construction from `launcher.rs`, so several functions there must be elevated from private to `pub(crate)`:

- `resolve_display_name`
- `combine_host_unix_path`
- `build_desktop_entry_content`
- `build_trainer_script_content`
- `write_host_text_file`

### Modified: `profile/toml_store.rs`

Add `ProfileStore::rename(old_name: &str, new_name: &str)` method. Atomic `fs::rename` from old `.toml` path to new `.toml` path. Must validate both names and ensure old file exists.

### Modified: `commands/profile.rs`

- `profile_delete`: Load profile before delete, derive launcher slug, call launcher store delete, then delete profile. Requires `ProfileStore` state already available.
- New `profile_rename`: Takes old_name, new_name. Loads old profile, renames TOML, cascades launcher rename, updates settings.

**Critical consideration**: `profile_delete` currently does not have access to launcher context (home path). The home path derivation depends on profile data (steam compatdata path) and the default steam client install path. The cascade logic either needs the home path passed from the frontend, or the backend must derive it independently using `resolve_target_home_path`.

### Modified: `commands/export.rs`

Add four new Tauri commands: `check_launcher_exists`, `delete_launcher`, `rename_launcher`, `list_launchers`. All follow the thin-wrapper pattern already used by `export_launchers`.

### Modified: `src-tauri/src/lib.rs`

Register all new commands in the `invoke_handler` macro. No new managed state required -- launcher operations are stateless file-system operations.

### Modified: `LauncherExport.tsx`

- Add launcher existence checking (call `check_launcher_exists` on mount/profile-change)
- Add status badge: "Exported" (green) / "Not Exported" (gray) / "Stale" (amber)
- Add "Delete Launcher" button with inline confirmation
- Add "Update Launcher" button when status is stale
- Optionally add "Rename Launcher" for manual rename

### Modified: `useProfile.ts`

- `deleteProfile()`: Before calling `profile_delete`, derive launcher slug and home path, pass to backend for cascade cleanup, or let the backend handle it if the command is enhanced.
- `saveProfile()`: Detect rename scenario when `profileName.trim() !== selectedProfile` and `selectedProfile !== ''`. Either call a new `profile_rename` command or handle as save+delete+launcher-rename sequence.

### Modified: `App.tsx`

May need to pass additional props to `LauncherExport` if launcher management actions need access to `profileState` methods (e.g., refreshing after launcher operations).

### New: TypeScript types

New file `types/launcher.ts` (or additions to `types/profile.ts`) for:

- `LauncherInfo`
- `LauncherDeleteResult`
- `LauncherRenameResult`

Must be re-exported from `types/index.ts`.

## Key Dependencies

### Internal

| Module                                    | Role                                             | Used By                              |
| ----------------------------------------- | ------------------------------------------------ | ------------------------------------ |
| `crosshook-core::export::launcher`        | Slug derivation, path construction, file writing | New `launcher_store` module          |
| `crosshook-core::profile::toml_store`     | Profile CRUD                                     | Commands for cascade                 |
| `crosshook-core::settings::SettingsStore` | `last_used_profile` management                   | Rename cascade                       |
| `directories::BaseDirs`                   | XDG path resolution                              | All stores, launcher path derivation |

### External Crates (existing, no additions needed)

| Crate            | Version | Relevance                                                                                      |
| ---------------- | ------- | ---------------------------------------------------------------------------------------------- |
| `directories`    | 5       | `BaseDirs::data_dir()` for XDG_DATA_HOME resolution (currently hardcoded to `~/.local/share/`) |
| `serde` + `toml` | 1 / 0.8 | Serialization for new types crossing IPC boundary                                              |
| `tempfile`       | 3 (dev) | Test fixtures for new launcher store tests                                                     |

### Frontend (existing, no additions needed)

| Package                     | Relevance                                                             |
| --------------------------- | --------------------------------------------------------------------- |
| `@tauri-apps/api/core`      | `invoke()` for all new IPC commands                                   |
| `@tauri-apps/plugin-dialog` | Potential confirmation dialogs (currently only used for file picking) |
| React 18                    | State management, effects for existence checking                      |

## Architectural Patterns

- **Thin Tauri command layer**: Every `#[tauri::command]` in `commands/*.rs` delegates immediately to a `crosshook_core` function and maps errors via `.map_err(|e| e.to_string())`. New commands must follow this pattern exactly.
- **Store pattern**: `ProfileStore`, `SettingsStore`, `RecentFilesStore` all follow identical structure: `struct Store { base_path: PathBuf }` with `try_new()`, `new()`, `with_base_path()` constructors, and TOML serialization. Launcher operations are stateless (no store struct needed) since paths are derived from inputs, not stored.
- **State injection**: Stores are registered via `tauri::Builder::manage()` and injected into commands via `State<'_, Store>`. Launcher commands are purely functional (no managed state needed -- they take slug + home_path as arguments).
- **Error handling**: Custom error enums per domain (`ProfileStoreError`, `SteamExternalLauncherExportError`, etc.) with `Display` impl. Tauri commands convert all errors to `String`. New launcher store should define `LauncherStoreError` following this pattern.
- **Slug derivation chain**: `resolve_display_name(launcher_name, steam_app_id, trainer_path)` -> `sanitize_launcher_slug(display_name)` -> path templates. This chain MUST be shared between export and lifecycle operations.
- **Frontend state hooks**: Each domain has a dedicated hook (`useProfile`, `useLaunchState`, `useCommunityProfiles`). Launcher management may warrant a new `useLauncherStatus` hook or can be embedded in `LauncherExport.tsx` as local state (simpler, since it's only used by one component).
- **Async pattern in commands**: Most commands are synchronous. The `install.rs` commands use `tauri::async_runtime::spawn_blocking` for long operations. Launcher file operations are fast enough to remain synchronous.

## Gotchas and Edge Cases

- **Hardcoded home path**: `export_launchers()` in `launcher.rs` uses `combine_host_unix_path(target_home_path, ".local/share/crosshook/launchers", ...)` with hardcoded paths instead of `BaseDirs::data_dir()`. The feature spec flags this as a refactoring candidate, but changing it would break path consistency between export and delete for existing users. Either keep hardcoded or migrate both simultaneously.
- **Profile rename is not atomic**: The current `saveProfile()` in `useProfile.ts` calls `profile_save` (creates/overwrites new name) but never deletes the old profile file. This means renaming today actually creates a copy. The new `profile_rename` command or enhanced save flow must handle the old file deletion.
- **Slug collision**: Two profiles with names like "Elden Ring" and "Elden Ring!" produce the same slug `elden-ring`. Deleting one profile's launcher removes the other's. The feature spec acknowledges this as an inherent limitation of lossy slug derivation.
- **Custom launcher_name override**: Users can type a custom `launcherName` in `LauncherExport.tsx` that differs from the profile's display name. This means the exported slug may not match what would be derived from the profile alone. Lifecycle operations must use the same fallback chain as `resolve_display_name()`.
- **Home path derivation**: The `target_home_path` is derived client-side in `App.tsx` from `steamClientInstallPath` via `deriveTargetHomePath()`. For backend cascade (profile_delete triggering launcher cleanup), the backend must independently derive this path or receive it from the frontend. `resolve_target_home_path()` in `launcher.rs` already has this logic.
- **Private helper functions**: Several critical functions in `launcher.rs` are private (`resolve_display_name`, `combine_host_unix_path`, `build_desktop_entry_content`, `build_trainer_script_content`, `write_host_text_file`). They must be elevated to `pub(crate)` for the new `launcher_store` module to reuse them.
- **Desktop entry content rewriting**: Rename requires updating `Name=`, `Exec=`, and `Comment=` fields inside `.desktop` files. The current `build_desktop_entry_content()` function generates a complete file from scratch, which is simpler than field-level parsing. The write-new-then-delete-old strategy avoids in-place editing complexities.
- **No filesystem watcher**: CrossHook does not watch the filesystem for launcher file changes. If a user manually deletes launcher files, the UI will show stale status until the next existence check. This is acceptable per the feature spec.
- **`install` context mode**: `LauncherExport` has a `context === 'install'` path that renders a completely different UI (install review). Launcher management UI must only appear in the `default` context.

## Other Docs

- `/home/yandy/Projects/github.com/yandy-r/crosshook/docs/plans/launcher-delete/feature-spec.md`: Complete feature specification with data models, API design, UX workflows, and implementation sequence
- `/home/yandy/Projects/github.com/yandy-r/crosshook/docs/plans/launcher-delete/research-business.md`: Business requirements research
- `/home/yandy/Projects/github.com/yandy-r/crosshook/docs/plans/launcher-delete/research-technical.md`: Technical research
- `/home/yandy/Projects/github.com/yandy-r/crosshook/docs/plans/launcher-delete/research-ux.md`: UX research
- `/home/yandy/Projects/github.com/yandy-r/crosshook/docs/plans/launcher-delete/research-external.md`: External dependency research
- `/home/yandy/Projects/github.com/yandy-r/crosshook/docs/plans/launcher-delete/research-recommendations.md`: Implementation recommendations
- [Freedesktop Desktop Entry Spec](https://specifications.freedesktop.org/desktop-entry/latest-single/): File format for `.desktop` entries
- [XDG Base Directory Spec](https://specifications.freedesktop.org/basedir/latest/): Standard paths for user data (`XDG_DATA_HOME`)
