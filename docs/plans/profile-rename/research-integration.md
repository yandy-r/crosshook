# Profile Rename — Integration Research

Research into the APIs, file system operations, and internal integrations required to implement profile rename with overwrite protection and settings cascade.

## Relevant Files

- `src-tauri/src/commands/profile.rs`: Tauri IPC command layer for profile operations — includes existing `profile_rename` command
- `src-tauri/src/lib.rs`: Tauri app setup, state management, and command registration
- `src-tauri/src/commands/settings.rs`: Tauri IPC commands for `SettingsStore` and `RecentFilesStore`
- `src-tauri/src/startup.rs`: Auto-load profile on startup via `resolve_auto_load_profile_name()`
- `crates/crosshook-core/src/profile/toml_store.rs`: `ProfileStore` — TOML-backed profile persistence with `rename()`, `validate_name()`, error types
- `crates/crosshook-core/src/settings/mod.rs`: `SettingsStore` — app settings persistence, `last_used_profile` field
- `crates/crosshook-core/src/export/launcher_store.rs`: Launcher file management — `rename_launcher_files()`, `delete_launcher_for_profile()`
- `src/hooks/useProfile.ts`: React hook managing profile CRUD state, IPC calls, and metadata sync
- `src/types/profile.ts`: TypeScript types for `GameProfile`, `DuplicateProfileResult`
- `src/types/launcher.ts`: TypeScript types for `LauncherInfo`, `LauncherRenameResult`
- `src/types/settings.ts`: TypeScript types for `AppSettingsData`, `RecentFilesData`

## API Endpoints

### Existing `profile_rename` Command

The command already exists and is registered in `lib.rs:96`:

```rust
// src-tauri/src/commands/profile.rs:148-154
#[tauri::command]
pub fn profile_rename(
    old_name: String,
    new_name: String,
    store: State<'_, ProfileStore>,
) -> Result<(), String> {
    store.rename(&old_name, &new_name).map_err(map_error)
}
```

**Current gaps:**

- Does NOT accept `State<'_, SettingsStore>` — cannot cascade `last_used_profile`
- Delegates directly to `ProfileStore::rename()` which currently allows silent overwrites

### Command Registration

All profile commands are registered in `lib.rs:70-98` via `tauri::generate_handler![]`. The `profile_rename` command is already included at line 96. No changes needed to command registration.

### State Injection Pattern

Tauri v2 state is managed via `.manage()` calls in `lib.rs:62-66`:

```rust
.manage(profile_store)       // State<'_, ProfileStore>
.manage(settings_store)      // State<'_, SettingsStore>
.manage(recent_files_store)  // State<'_, RecentFilesStore>
.manage(community_tap_store) // State<'_, CommunityTapStore>
```

Any Tauri command can request multiple `State<'_, T>` parameters. The `profile_rename` command needs to add `settings_store: State<'_, SettingsStore>` to access `last_used_profile`.

### Established Pattern: Profile Delete with Side Effects

`profile_delete` (lines 113-123) shows the established pattern for profile operations with cascading side effects:

```rust
#[tauri::command]
pub fn profile_delete(name: String, store: State<'_, ProfileStore>) -> Result<(), String> {
    // Best-effort launcher cleanup before profile deletion.
    if let Ok(profile) = store.load(&name) {
        if let Err(error) = cleanup_launchers_for_profile_delete(&name, &profile) {
            tracing::warn!("Launcher cleanup failed for profile {name}: {error}");
        }
    }
    store.delete(&name).map_err(map_error)
}
```

Key convention: side effects are **best-effort** — they log warnings on failure but don't prevent the primary operation from succeeding. The rename command should follow this same pattern for the settings cascade.

### IPC Argument Convention

Tauri v2 IPC uses `camelCase` argument names on the TypeScript side (Tauri auto-converts to `snake_case` for Rust). Frontend invocation:

```ts
await invoke('profile_rename', { oldName: 'Old Name', newName: 'New Name' });
```

Maps to Rust parameters `old_name: String, new_name: String`.

## 2. File System Operations

### `ProfileStore` Structure

```rust
// crates/crosshook-core/src/profile/toml_store.rs:10-13
pub struct ProfileStore {
    pub base_path: PathBuf,  // ~/.config/crosshook/profiles/
}
```

Profile names map directly to TOML filenames: `profile_path()` returns `base_path.join("{name}.toml")`.

### `ProfileStore::rename()` — Current Implementation

```rust
// toml_store.rs:163-178
pub fn rename(&self, old_name: &str, new_name: &str) -> Result<(), ProfileStoreError> {
    let old_name = old_name.trim();
    let new_name = new_name.trim();
    validate_name(old_name)?;
    validate_name(new_name)?;
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

**Current behavior:**

- Trims and validates both names via `validate_name()`
- Checks source file exists
- Same-name is a no-op
- Uses `std::fs::rename()` — atomic on same filesystem, **silently overwrites** if target exists

**Required change:** Add `new_path.exists()` check before `fs::rename()` to return `AlreadyExists` error.

### `validate_name()` — Name Validation

```rust
// toml_store.rs:273-298
pub fn validate_name(name: &str) -> Result<(), ProfileStoreError> {
    const WINDOWS_RESERVED_PATH_CHARACTERS: [char; 9] =
        ['<', '>', ':', '"', '/', '\\', '|', '?', '*'];

    let trimmed = name.trim();
    if trimmed.is_empty() || trimmed == "." || trimmed == ".." { /* error */ }
    if Path::new(trimmed).is_absolute() || trimmed.contains('/') || ... { /* error */ }
    if trimmed.chars().any(|c| WINDOWS_RESERVED_PATH_CHARACTERS.contains(&c)) { /* error */ }
    Ok(())
}
```

Prevents path traversal and filesystem-unsafe characters. Already called by `rename()` for both old and new names.

### `ProfileStoreError` Enum

```rust
// toml_store.rs:16-23
pub enum ProfileStoreError {
    InvalidName(String),
    NotFound(PathBuf),
    InvalidLaunchOptimizationId(String),
    Io(std::io::Error),
    TomlDe(toml::de::Error),
    TomlSer(toml::ser::Error),
}
```

**Required change:** Add `AlreadyExists(String)` variant with `Display` impl: `"a profile named '{name}' already exists"`.

### Existing `rename()` Test Coverage

Tests in `toml_store.rs` already cover:

- **`test_rename_success`** (line 460): Happy path — old file deleted, new file exists, content preserved
- **`test_rename_not_found`** (line 477): Source doesn't exist → error
- **`test_rename_same_name`** (line 487): No-op
- **`test_rename_preserves_content`** (line 499): Byte-for-byte file content equality
- **`test_rename_overwrites_existing_target_profile`** (line 515): Currently EXPECTS silent overwrite — **this test must be updated** to expect `AlreadyExists` error

### Other ProfileStore Methods Referenced

| Method        | Signature                                                                                     | Used by              |
| ------------- | --------------------------------------------------------------------------------------------- | -------------------- |
| `load()`      | `fn load(&self, name: &str) -> Result<GameProfile, ProfileStoreError>`                        | Many consumers       |
| `save()`      | `fn save(&self, name: &str, profile: &GameProfile) -> Result<(), ProfileStoreError>`          | save operations      |
| `list()`      | `fn list(&self) -> Result<Vec<String>, ProfileStoreError>`                                    | Profile list refresh |
| `delete()`    | `fn delete(&self, name: &str) -> Result<(), ProfileStoreError>`                               | Profile deletion     |
| `duplicate()` | `fn duplicate(&self, source_name: &str) -> Result<DuplicateProfileResult, ProfileStoreError>` | Profile duplication  |

## 3. Settings Store

### `SettingsStore` Structure

```rust
// crates/crosshook-core/src/settings/mod.rs:15-17
pub struct SettingsStore {
    pub base_path: PathBuf,  // ~/.config/crosshook/
}
```

Persists to `base_path.join("settings.toml")`.

### `AppSettingsData` — The Key Data Model

```rust
// settings/mod.rs:19-25
pub struct AppSettingsData {
    pub auto_load_last_profile: bool,
    pub last_used_profile: String,      // <-- must cascade on rename
    pub community_taps: Vec<CommunityTapSubscription>,
}
```

`last_used_profile` stores the profile name (TOML file stem). When a profile is renamed, this field must be updated if it matches the old name.

### SettingsStore API

```rust
pub fn load(&self) -> Result<AppSettingsData, SettingsStoreError>  // Returns default if file missing
pub fn save(&self, settings: &AppSettingsData) -> Result<(), SettingsStoreError>
```

Both methods create parent directories via `fs::create_dir_all()` before access.

### Settings Cascade Pattern — Frontend (Current)

The frontend currently manages `last_used_profile` in two places:

1. **On profile load** (`useProfile.ts:340-360`): `syncProfileMetadata()` — loads settings, sets `last_used_profile`, and updates recent files.
2. **On profile delete** (`useProfile.ts:433-461`): `finalizeProfileDeletion()` — clears `last_used_profile` if it matches the deleted name.

For rename, the cascade should happen **in the Tauri command** (server-side), matching the `profile_delete` pattern of keeping side effects in the backend. The frontend then calls `refreshProfiles()` + `loadProfile(newName)` to sync state.

### Startup Auto-Load Dependency

```rust
// startup.rs:34-64
pub fn resolve_auto_load_profile_name(
    settings_store: &SettingsStore,
    profile_store: &ProfileStore,
) -> Result<Option<String>, StartupError> {
    let settings = settings_store.load()?;
    if !settings.auto_load_last_profile { return Ok(None); }
    let last_used_profile = settings.last_used_profile.trim();
    if last_used_profile.is_empty() { return Ok(None); }
    let available_profiles = profile_store.list()?;
    if available_profiles.iter().any(|n| n == last_used_profile) {
        return Ok(Some(last_used_profile.to_string()));
    }
    Ok(None)
}
```

This function checks that `last_used_profile` exists in the profile list. If the rename cascade updates `last_used_profile` to the new name, auto-load works seamlessly. If the cascade fails (best-effort), the old name won't match any profile, and auto-load gracefully returns `None`. No changes needed to this function.

## 4. Frontend Hooks

### `useProfile.ts` — State Shape

Key state relevant to rename:

```ts
const [profiles, setProfiles] = useState<string[]>([]);         // profile name list
const [selectedProfile, setSelectedProfile] = useState('');      // currently selected name
const [profileName, setProfileName] = useState('');              // name in editor
const [profile, setProfile] = useState<GameProfile>(...);        // profile data
const [dirty, setDirty] = useState(false);                       // unsaved changes flag
```

### Existing Pattern: `duplicateProfile()`

The `duplicateProfile` function (lines 569-588) is the closest analog to what `renameProfile` needs:

```ts
const duplicateProfile = useCallback(
  async (sourceName: string): Promise<void> => {
    if (!sourceName.trim()) return;
    setDuplicating(true);
    setError(null);
    try {
      const result = await invoke<DuplicateProfileResult>('profile_duplicate', {
        name: sourceName,
      });
      await refreshProfiles();
      await loadProfile(result.name);
    } catch (err) {
      const message = err instanceof Error ? err.message : String(err);
      setError(message);
    } finally {
      setDuplicating(false);
    }
  },
  [loadProfile, refreshProfiles]
);
```

**Rename should follow the same pattern:**

1. Set `renaming = true`, clear error
2. `invoke('profile_rename', { oldName, newName })`
3. `refreshProfiles()` to update list
4. `loadProfile(newName)` to select the renamed profile
5. Catch errors and display
6. Set `renaming = false`

### Settings Cascade — Frontend vs. Backend

For `profile_delete`, the frontend handles the settings cascade in `finalizeProfileDeletion()` (lines 433-461):

```ts
const settings = await invoke<AppSettingsData>('settings_load');
if (settings.last_used_profile === name) {
  await invoke('settings_save', { data: { ...settings, last_used_profile: '' } });
}
```

However, per the feature spec, the rename settings cascade should be in the **Tauri command** (backend), not the frontend. This avoids an extra IPC round-trip and keeps the operation atomic.

### `UseProfileResult` Interface

The hook's return type (lines 20-48) exports the public API. New additions needed:

```ts
renameProfile: (oldName: string, newName: string) => Promise<void>;
renaming: boolean;
```

### Launch Optimizations Autosave Timer

The `useProfile` hook has a debounced autosave timer (350ms) for launch optimizations:

```ts
const launchOptimizationsAutosaveTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);
```

This timer writes to the profile TOML file by name. If a rename happens while the timer is pending, the autosave would write to the old (now nonexistent) filename. **The `renameProfile` function should cancel this timer before invoking the IPC call.**

## 5. Launcher Integration

### Key Principle: Launchers are Independent of Profile Name

Launcher file paths derive from `steam.launcher.display_name`, NOT the profile name. The derivation chain:

```
display_name → sanitize_launcher_slug() → slug
slug → {slug}-trainer.sh, crosshook-{slug}-trainer.desktop
```

Example: Profile named "my-elden-ring" with `display_name = "Elden Ring"`:

- Script: `~/.local/share/crosshook/launchers/elden-ring-trainer.sh`
- Desktop: `~/.local/share/applications/crosshook-elden-ring-trainer.desktop`

Renaming the profile from "my-elden-ring" to "er-souls" changes NOTHING about the launcher files.

### `rename_launcher_files()` — Exists but NOT Needed

A full launcher rename function exists (`launcher_store.rs:367`) for renaming the `display_name` (which changes the slug and file paths). This is used by the `rename_launcher` Tauri command (`export.rs:81-118`), but it operates on launcher display names, not profile names.

### `LauncherRenameResult` Type — Already Exists

```rust
pub struct LauncherRenameResult {
    pub old_slug: String,
    pub new_slug: String,
    pub new_script_path: String,
    pub new_desktop_entry_path: String,
    pub script_renamed: bool,
    pub desktop_entry_renamed: bool,
    pub old_script_cleanup_warning: Option<String>,
    pub old_desktop_entry_cleanup_warning: Option<String>,
}
```

TypeScript mirror exists in `src/types/launcher.ts`. Neither is needed for profile rename.

### Profile Delete Launcher Cascade — For Reference

`profile_delete` does cascade to launcher cleanup (`commands/profile.rs:40-62`), but this is because deleting a profile means the launcher is orphaned. Renaming a profile does NOT affect launcher paths since they derive from `display_name`.

## Architectural Patterns

- **Best-effort side effects**: Side effects (settings cascade, launcher cleanup) log warnings on failure but don't fail the primary operation. Pattern established by `profile_delete`.
- **State injection via `State<'_, T>`**: Multiple managed state types can be injected into a single Tauri command. All stores are registered in `lib.rs`.
- **Frontend state sync**: After any mutation, the pattern is `refreshProfiles()` → `loadProfile(targetName)` to ensure consistent state.
- **Error mapping**: All Tauri commands use `map_err(|e| e.to_string())` to convert domain errors to IPC-safe strings.
- **Autosave timer**: Launch optimizations use a 350ms debounced autosave that writes by profile name — must be cancelled before rename.

## Edge Cases

- **Autosave race**: If the launch optimizations autosave timer fires between the rename IPC call and the frontend state update, it would attempt to write to the old profile name (which no longer exists). The `renameProfile()` function in `useProfile.ts` must clear `launchOptimizationsAutosaveTimerRef` before the IPC call.
- **Silent overwrite**: `std::fs::rename()` overwrites target on POSIX. The `AlreadyExists` check in `ProfileStore::rename()` prevents this, but there's a theoretical TOCTOU race (acceptable for single-user desktop app).
- **Settings cascade failure**: If `SettingsStore::load()` or `save()` fails after a successful rename, the profile file is already renamed. The old `last_used_profile` value won't match any profile, so auto-load gracefully falls back to `None`. Not a data loss scenario.
- **Concurrent operations**: No locking exists across profile operations. A concurrent `save()` to the old name during rename could create a new file at the old path. Acceptable for single-user desktop app.

## Other Docs

- [feature-spec.md](./feature-spec.md): Complete feature specification with phased implementation plan
- [research-business.md](./research-business.md): Business logic analysis and domain model
- [research-technical.md](./research-technical.md): Architecture design and API contracts
- [research-external.md](./research-external.md): External library evaluation (`std::fs::rename` atomicity)
- [Tauri v2 State Management](https://v2.tauri.app/develop/state-management/): Official docs for `State<'_, T>` injection pattern
- [rename(2) man page](https://man7.org/linux/man-pages/man2/rename.2.html): POSIX atomicity guarantees
