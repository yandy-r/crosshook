# Integration Research: launcher-delete

## Overview

This document catalogs all existing APIs, IPC commands, data models, file system integration points, and frontend patterns relevant to implementing launcher lifecycle management (auto-delete/rename launchers when profiles change). The codebase follows a clean layered architecture: `crosshook-core` (pure Rust library) exposes business logic, `src-tauri/commands/` wraps it as IPC handlers, and React components call them via `invoke()`. Launcher export currently writes files but has no discovery, deletion, or rename capability. Profile deletion has no awareness of launcher artifacts.

## API Endpoints

The following sections detail the Tauri IPC commands (CrossHook's equivalent of API endpoints), data models, and integration surfaces.

## Tauri IPC Commands

### Existing Export Commands

Located in `src-tauri/src/commands/export.rs`:

- **`validate_launcher_export(request: SteamExternalLauncherExportRequest) -> Result<(), String>`**: Validates the request fields (method, trainer path, prefix, proton, steam app ID, icon). Delegates to `crosshook_core::export::validate()`. Does NOT accept Tauri `State` -- the export system is currently stateless.
- **`export_launchers(request: SteamExternalLauncherExportRequest) -> Result<SteamExternalLauncherExportResult, String>`**: Generates both the `.sh` script and `.desktop` entry. Delegates to `crosshook_core::export::export_launchers()`. Returns paths and metadata in `SteamExternalLauncherExportResult`.

Key observation: Neither command accepts `State<'_, ProfileStore>` or any managed state. They are pure request-in/result-out functions. This means new launcher lifecycle commands can follow the same pattern (stateless, path-derived) or could accept `State` if they need access to profile data.

### Existing Profile Commands

Located in `src-tauri/src/commands/profile.rs`:

- **`profile_list(store: State<'_, ProfileStore>) -> Result<Vec<String>, String>`**: Lists all `.toml` profile names in `~/.config/crosshook/profiles/`.
- **`profile_load(name: String, store: State<'_, ProfileStore>) -> Result<GameProfile, String>`**: Loads and deserializes a single profile by name.
- **`profile_save(name: String, data: GameProfile, store: State<'_, ProfileStore>) -> Result<(), String>`**: Saves a profile. The `name` parameter is the file stem (e.g., "elden-ring" becomes "elden-ring.toml").
- **`profile_delete(name: String, store: State<'_, ProfileStore>) -> Result<(), String>`**: Deletes the TOML file only. Does NOT touch launcher artifacts, settings, or recent files. This is the primary hook point for cascading launcher cleanup.
- **`profile_import_legacy(path: String, store: State<'_, ProfileStore>) -> Result<GameProfile, String>`**: Imports from old `.profile` format.

### Existing Settings Commands

Located in `src-tauri/src/commands/settings.rs`:

- **`settings_load(store: State<'_, SettingsStore>) -> Result<AppSettingsData, String>`**
- **`settings_save(data: AppSettingsData, store: State<'_, SettingsStore>) -> Result<(), String>`**
- **`recent_files_load(store: State<'_, RecentFilesStore>) -> Result<RecentFilesData, String>`**
- **`recent_files_save(data: RecentFilesData, store: State<'_, RecentFilesStore>) -> Result<(), String>`**

### Command Registration

All commands are registered in `src-tauri/src/lib.rs` via `tauri::generate_handler![]` (lines 69-94). New commands must be added to this macro invocation.

### Managed State Objects

Four state objects are initialized in `lib.rs` `run()` and registered with `.manage()`:

1. **`ProfileStore`** -- `base_path: PathBuf` pointing to `~/.config/crosshook/profiles/`
2. **`SettingsStore`** -- `base_path: PathBuf` pointing to `~/.config/crosshook/`
3. **`RecentFilesStore`** -- `path: PathBuf` pointing to `~/.local/share/crosshook/recent.toml`
4. **`CommunityTapStore`** -- community profile taps

No launcher-related state object exists yet. A new `LauncherStore` or similar would need to be created and registered here if the implementation uses managed state. Alternatively, new commands can remain stateless like the current export commands.

## Data Models

### GameProfile (profile/models.rs)

```rust
pub struct GameProfile {
    pub game: GameSection,       // name, executable_path
    pub trainer: TrainerSection,  // path, kind
    pub injection: InjectionSection, // dll_paths, inject_on_launch
    pub steam: SteamSection,     // enabled, app_id, compatdata_path, proton_path, launcher
    pub runtime: RuntimeSection, // prefix_path, proton_path, working_directory
    pub launch: LaunchSection,   // method
}
```

Key nested types for launcher-delete:

- **`SteamSection.launcher: LauncherSection`**: Contains `icon_path: String` and `display_name: String`. The `display_name` is the user-visible name used for launcher derivation.
- **`LaunchSection.method: String`**: One of `"steam_applaunch"`, `"proton_run"`, or `"native"`. Launchers are only relevant for the first two methods.

The `GameProfile` struct does NOT track whether launchers have been exported or their file paths. Launcher existence is determined by deriving paths from profile data and checking the filesystem.

### Launcher Export Types (export/launcher.rs)

**Request:**

```rust
pub struct SteamExternalLauncherExportRequest {
    pub method: String,                   // "steam_applaunch" or "proton_run"
    pub launcher_name: String,            // Preferred display name (may be empty)
    pub trainer_path: String,             // Path to trainer executable
    pub launcher_icon_path: String,       // Path to PNG/JPG icon (may be empty)
    pub prefix_path: String,              // Proton/Wine prefix path
    pub proton_path: String,              // Path to proton binary
    pub steam_app_id: String,             // Steam app ID (required for steam_applaunch)
    pub steam_client_install_path: String, // Steam installation root
    pub target_home_path: String,         // Target $HOME for output path construction
}
```

**Result:**

```rust
pub struct SteamExternalLauncherExportResult {
    pub display_name: String,       // Resolved display name
    pub launcher_slug: String,      // Sanitized slug (e.g., "elden-ring-deluxe")
    pub script_path: String,        // Full path to generated .sh file
    pub desktop_entry_path: String, // Full path to generated .desktop file
}
```

**Validation errors** (`SteamExternalLauncherExportValidationError`):

- `TrainerPathRequired`, `PrefixPathRequired`, `ProtonPathRequired`, `SteamAppIdRequired`, `TargetHomePathRequired`, `LauncherIconPathNotFound`, `LauncherIconPathInvalidExtension`, `UnsupportedMethod(String)`

### ProfileStore (profile/toml_store.rs)

```rust
pub struct ProfileStore {
    pub base_path: PathBuf, // ~/.config/crosshook/profiles/
}
```

Methods: `try_new()`, `new()`, `with_base_path(PathBuf)`, `load(&str)`, `save(&str, &GameProfile)`, `list()`, `delete(&str)`, `import_legacy(&Path)`, `profile_path(&str) -> Result<PathBuf>`.

The `delete()` method on `ProfileStore` only removes the TOML file. It does NOT cascade to launchers. The cascading logic currently lives entirely in the frontend (`useProfile.ts` `deleteProfile` callback).

### AppSettingsData (settings/mod.rs)

```rust
pub struct AppSettingsData {
    pub auto_load_last_profile: bool,
    pub last_used_profile: String,
    pub community_taps: Vec<CommunityTapSubscription>,
}
```

The `last_used_profile` field is cleared by the frontend when a profile is deleted (in `useProfile.ts` `deleteProfile`). Settings are stored at `~/.config/crosshook/settings.toml`.

### TypeScript Type Mirrors

Located in `src/types/profile.ts`:

```typescript
export type LaunchMethod = '' | 'steam_applaunch' | 'proton_run' | 'native';

export interface GameProfile {
  game: { name: string; executable_path: string };
  trainer: { path: string; type: string };
  injection: { dll_paths: string[]; inject_on_launch: boolean[] };
  steam: {
    enabled: boolean;
    app_id: string;
    compatdata_path: string;
    proton_path: string;
    launcher: { icon_path: string; display_name: string };
  };
  runtime: { prefix_path: string; proton_path: string; working_directory: string };
  launch: { method: LaunchMethod };
}
```

Located in `src/types/settings.ts`:

```typescript
export interface AppSettingsData {
  auto_load_last_profile: boolean;
  last_used_profile: string;
  community_taps: CommunityTapSubscription[];
}
```

## File System Integration

### Launcher File Layout

Script path pattern:

```
{target_home}/.local/share/crosshook/launchers/{slug}-trainer.sh
```

Desktop entry path pattern:

```
{target_home}/.local/share/applications/crosshook-{slug}-trainer.desktop
```

Where `{target_home}` is resolved from (in priority order):

1. `request.target_home_path` (if non-empty and not inside `/compatdata/`)
2. Steam client install path stripped of `/.local/share/Steam` or `/.steam/root` suffix
3. `$HOME` environment variable

File permissions: Scripts get `0o755`, desktop entries get `0o644`.

### Key Functions in export/launcher.rs

**`pub fn sanitize_launcher_slug(value: &str) -> String`** (line 243)

- Already `pub`, can be reused by new launcher_store module
- Lowercases input, replaces non-alphanumeric runs with single `-`, trims leading/trailing `-`
- Empty or all-separator input returns `"crosshook-trainer"` (fallback)
- Examples: `"Elden Ring Deluxe"` -> `"elden-ring-deluxe"`, `"  CrossHook: Trainer 2026!!  "` -> `"crosshook-trainer-2026"`

**`fn resolve_display_name(preferred_name: &str, steam_app_id: &str, trainer_path: &str) -> String`** (line 220, private)

- Priority: preferred name > trainer file stem > `"steam-{app_id}-trainer"` > `"crosshook-trainer"`
- This is the entry point for deriving the display name that feeds into `sanitize_launcher_slug()`

**`pub fn resolve_target_home_path(preferred_home_path: &str, steam_client_install_path: &str) -> String`** (line 465, already `pub`)

- Resolution chain documented above

**`fn write_host_text_file(host_path: &str, content: &str, mode: u32) -> Result<(), io::Error>`** (line 441, private)

- Creates parent directories via `fs::create_dir_all()`
- Normalizes line endings to LF
- Sets Unix permissions via `PermissionsExt::set_mode()`

**`fn combine_host_unix_path(root_path: &str, segment_one: &str, segment_two: &str) -> String`** (line 274, private)

- Joins three path segments with `/`, normalizing backslashes

**`fn build_trainer_script_content(request, display_name) -> String`** (line 296, private)

- The `.sh` script embeds the display name in a comment on line 3: `# {display_name} - Trainer launcher`
- Scripts embed `PREFIX_ROOT`, `PROTON`, `TRAINER_HOST_PATH` as shell variables
- Script content must be regenerated (not just renamed) if these values change

**`fn build_desktop_entry_content(display_name, script_path, icon_path) -> String`** (line 414, private)

- `.desktop` file embeds: `Name={display_name} - Trainer`, `Comment=Trainer launcher for {display_name}. Generated by CrossHook...`, `Exec=/bin/bash {script_path}`, `Icon={icon_path}`
- On rename, the `Name=`, `Comment=`, and `Exec=` fields all need updating

### Path Construction Details

Both paths are constructed in `export_launchers()` (lines 190-198):

```rust
let script_path = combine_host_unix_path(
    &target_home_path,
    ".local/share/crosshook/launchers",
    &format!("{launcher_slug}-trainer.sh"),
);
let desktop_entry_path = combine_host_unix_path(
    &target_home_path,
    ".local/share/applications",
    &format!("crosshook-{launcher_slug}-trainer.desktop"),
);
```

Note the asymmetry: script filenames are `{slug}-trainer.sh` while desktop filenames are `crosshook-{slug}-trainer.desktop` (prefixed with `crosshook-`).

## Frontend Integration

### LauncherExport.tsx

- Receives `profile: GameProfile`, `method`, `steamClientInstallPath`, `targetHomePath`, `context` as props from `App.tsx`
- Only rendered when `effectiveLaunchMethod` is `"steam_applaunch"` or `"proton_run"` (controlled by `shouldShowLauncherExport` in `App.tsx`, line 96-99)
- Has local state for: `launcherName`, `isExporting`, `errorMessage`, `statusMessage`, `result`
- `deriveLauncherName(profile)` derives the launcher name from profile data (display_name > game name > trainer stem > app ID > fallback)
- `buildExportRequest()` constructs the IPC request from profile + props
- `handleExport()` calls `invoke('validate_launcher_export', { request })` then `invoke('export_launchers', { request })`
- After export, displays the result (script_path, desktop_entry_path, slug)
- Has a `Reset` button that clears state but does nothing on disk
- Has an `install` context variant that shows a read-only review panel (no export button)

**Key gap**: No delete or rename UI exists. The result display shows paths but offers no management actions.

### useProfile.ts Hook

Manages all profile CRUD state. Key functions:

**`deleteProfile()` (line 325-363)**:

1. Calls `invoke('profile_delete', { name })`
2. Loads settings and clears `last_used_profile` if it matched the deleted profile
3. Refreshes the profile list via `invoke('profile_list')`
4. Auto-selects the first remaining profile or resets to empty state

**This is the primary integration point for launcher cascade on delete.** Currently, it only deletes the TOML file and updates settings. No launcher cleanup happens.

**`saveProfile()` (line 295-323)**:

1. Validates profile name and executable path
2. Normalizes profile data via `normalizeProfileForSave()`
3. Calls `invoke('profile_save', { name, data: normalizedProfile })`
4. Syncs metadata (settings + recent files)
5. Refreshes profiles and reloads

**This is the integration point for launcher cascade on rename.** When a profile is saved under a different name than `selectedProfile`, it effectively renames the profile. Currently, no launcher rename happens.

**`hydrateProfile()` (line 271-288)**: Used by Install Game flow to inject a generated profile into the editor without saving.

**`normalizeProfileForSave()` (line 108-128)**: Derives `game.name` and `steam.launcher.display_name` before saving -- these are the same fields that feed the launcher name derivation chain.

### ProfileEditor.tsx

- `ProfileEditorView` receives `state: UseProfileResult` as props
- Delete button (line 712): `onClick={() => void deleteProfile()}` with `disabled={!canDelete}`
- `canDelete = profileExists && !saving && !deleting && !loading`
- No confirmation dialog before delete -- it triggers immediately
- Save button (line 709): `onClick={() => void saveProfile()}`
- No rename detection (comparing old name vs. new name) in the component

### App.tsx (Main Shell)

- Creates `profileState = useProfile({ autoSelectFirstProfile: false })`
- Passes `profileState` to `ProfileEditorView`
- Passes `profile`, `effectiveLaunchMethod`, `steamClientInstallPath`, `targetHomePath` to `LauncherExport`
- LauncherExport only receives the profile object -- it does NOT receive `profileName` or `selectedProfile`, which means it cannot independently determine which profile's launchers to manage
- `shouldShowLauncherExport` computed at lines 96-99: `profileEditorTab === 'install' || method === 'steam_applaunch' || method === 'proton_run'`

## Configuration

### File System Paths

| Artifact         | Path                                                           |
| ---------------- | -------------------------------------------------------------- |
| Profiles         | `~/.config/crosshook/profiles/{name}.toml`                     |
| Settings         | `~/.config/crosshook/settings.toml`                            |
| Recent files     | `~/.local/share/crosshook/recent.toml`                         |
| Launcher scripts | `~/.local/share/crosshook/launchers/{slug}-trainer.sh`         |
| Desktop entries  | `~/.local/share/applications/crosshook-{slug}-trainer.desktop` |

### Environment Variables

- `$HOME` -- fallback for target home path resolution
- `$XDG_CONFIG_HOME` -- used by `directories::BaseDirs::config_dir()` (default: `~/.config`)
- `$XDG_DATA_HOME` -- used by `directories::BaseDirs::data_local_dir()` (default: `~/.local/share`)

Note: Current `export_launchers()` hardcodes `.local/share/` path segments rather than using `BaseDirs::data_dir()`. The feature spec flags this as a refactoring candidate.

### Tauri Capabilities

`src-tauri/capabilities/default.json` grants `core:default` and `dialog:default`. No `fs` plugin permissions are declared in capabilities (though `tauri_plugin_fs::init()` is registered). The export system uses `std::fs` directly on the Rust side rather than going through Tauri's FS plugin, so no capability changes should be needed for new launcher file operations.

## Relevant Files

- `/src/crosshook-native/crates/crosshook-core/src/export/launcher.rs`: Core export logic, `sanitize_launcher_slug()`, `resolve_display_name()`, `resolve_target_home_path()`, path construction, script/desktop content generation, `write_host_text_file()`
- `/src/crosshook-native/crates/crosshook-core/src/export/mod.rs`: Public re-exports for the export module
- `/src/crosshook-native/crates/crosshook-core/src/profile/models.rs`: `GameProfile`, `LauncherSection`, `SteamSection`, and all nested model types
- `/src/crosshook-native/crates/crosshook-core/src/profile/toml_store.rs`: `ProfileStore` with `load()`, `save()`, `delete()`, `list()` methods
- `/src/crosshook-native/crates/crosshook-core/src/profile/mod.rs`: Profile module public exports
- `/src/crosshook-native/crates/crosshook-core/src/settings/mod.rs`: `SettingsStore`, `AppSettingsData` (contains `last_used_profile`)
- `/src/crosshook-native/crates/crosshook-core/src/settings/recent.rs`: `RecentFilesStore`, `RecentFilesData`
- `/src/crosshook-native/crates/crosshook-core/src/lib.rs`: Module root (lists all crate modules)
- `/src/crosshook-native/src-tauri/src/commands/export.rs`: Tauri IPC wrappers for export (stateless)
- `/src/crosshook-native/src-tauri/src/commands/profile.rs`: Tauri IPC wrappers for profile CRUD (uses `State<ProfileStore>`)
- `/src/crosshook-native/src-tauri/src/commands/settings.rs`: Tauri IPC wrappers for settings and recent files
- `/src/crosshook-native/src-tauri/src/commands/mod.rs`: Command module declarations
- `/src/crosshook-native/src-tauri/src/lib.rs`: Tauri app builder, state registration, command handler registration
- `/src/crosshook-native/src-tauri/src/startup.rs`: Auto-load profile on app start
- `/src/crosshook-native/src/components/LauncherExport.tsx`: Launcher export UI panel
- `/src/crosshook-native/src/components/ProfileEditor.tsx`: Profile editor with save/delete buttons
- `/src/crosshook-native/src/hooks/useProfile.ts`: Profile CRUD state management hook (contains `deleteProfile` and `saveProfile`)
- `/src/crosshook-native/src/App.tsx`: Main app shell, composes all components, derives launch method and paths
- `/src/crosshook-native/src/types/profile.ts`: TypeScript `GameProfile` and `LaunchMethod` types
- `/src/crosshook-native/src/types/settings.ts`: TypeScript `AppSettingsData` type
- `/src/crosshook-native/src/types/launch.ts`: TypeScript `LaunchRequest` type
- `/src/crosshook-native/src-tauri/capabilities/default.json`: Tauri permissions configuration

## Architectural Patterns

- **Thin IPC layer**: Tauri commands in `src-tauri/src/commands/` are one-liners that delegate to `crosshook-core` and map errors to `String`. New commands should follow this pattern.
- **Stateless export commands**: Export commands do not use Tauri managed state; they receive all needed data in the request. This contrasts with profile/settings commands that use `State<'_, T>`.
- **Frontend-driven orchestration**: The `useProfile.ts` hook currently drives the delete workflow (delete profile TOML -> clear settings -> refresh list). Launcher cascade could be added here or pushed to the backend.
- **Deterministic path derivation**: Launcher file paths are derived from `display_name` -> `sanitize_launcher_slug()` -> path templates. There is no registry or manifest tracking which launchers exist -- existence is checked by probing the filesystem.
- **Error handling**: All IPC errors are `Result<T, String>` with `map_err(|e| e.to_string())`. Core logic uses typed error enums.
- **Profile naming**: Profile names are validated by `validate_name()` to reject path-unsafe characters. Profile names and launcher slugs are distinct concepts (profile name is the TOML file stem; slug is derived from the display name).

## Edgecases

- **Profile name != launcher slug**: The profile TOML filename (e.g., `elden-ring.toml`) is independent of the launcher slug (e.g., `elden-ring-deluxe`). The slug derives from `launcher.display_name` or `game.name`, not the profile file name. Deleting a profile requires loading its data first to derive the launcher slug.
- **`resolve_display_name` is private**: The function that determines the display name from request fields is not `pub`. A new launcher_store module would either need this made public, or reimplement the logic.
- **Multiple private helper functions**: `combine_host_unix_path`, `build_trainer_script_content`, `build_desktop_entry_content`, `write_host_text_file` are all private. Rename operations that need to regenerate file content will either need these made public or need to use the existing `export_launchers()` function to regenerate files from scratch.
- **Hardcoded `.local/share/`**: The current code hardcodes `".local/share/crosshook/launchers"` and `".local/share/applications"` as path segments rather than using XDG via `BaseDirs`. This works on most Linux systems but technically violates the XDG spec if `XDG_DATA_HOME` is set to a non-default location.
- **No launcher existence tracking**: The system has no record of which profiles have exported launchers. Discovery must scan the filesystem or re-derive paths from profile data.
- **Slug collision risk**: Two profiles with similar display names (e.g., "Elden Ring" and "elden-ring") would produce the same slug, leading to one overwriting the other's launcher. No collision detection exists.
- **`deleteProfile` in useProfile.ts clears settings but not launchers**: The frontend hook clears `last_used_profile` in settings when a profile is deleted, but does nothing about launcher files.
- **No rename detection**: `saveProfile()` always saves to `profileName`. If a user changes the name and saves, the old profile TOML remains (it becomes a copy). There is no explicit "rename" operation -- it is "save as new" + "delete old" as separate manual steps.
- **Desktop entry content on rename**: The `.desktop` file has `Exec=` pointing to the old `.sh` path. On rename, both files must be moved/recreated AND the `Exec=` path inside the `.desktop` file must be updated.
- **Asymmetric filename patterns**: Scripts: `{slug}-trainer.sh`, Desktop entries: `crosshook-{slug}-trainer.desktop`. The `crosshook-` prefix on desktop entries but not scripts is intentional (for namespace isolation in `~/.local/share/applications/`).

## Other Docs

- `/docs/plans/launcher-delete/feature-spec.md`: Full feature specification with business requirements, technical design, and UX considerations
- `/docs/plans/launcher-delete/research-business.md`: Business requirements research
- `/docs/plans/launcher-delete/research-external.md`: External dependencies and API research
- `/docs/plans/launcher-delete/research-technical.md`: Technical architecture research
- `/docs/plans/launcher-delete/research-ux.md`: UX/UI design research
- `/docs/plans/launcher-delete/research-recommendations.md`: Implementation recommendations
- [Freedesktop Desktop Entry Spec](https://specifications.freedesktop.org/desktop-entry/latest-single/): File format reference for `.desktop` entries
- [XDG Base Directory Spec](https://specifications.freedesktop.org/basedir/latest/): Standard paths for user data (`XDG_DATA_HOME`)
