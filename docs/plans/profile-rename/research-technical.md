# Profile Rename - Technical Specification

## Executive Summary

Profile rename is partially implemented: `ProfileStore::rename()`, the `profile_rename` Tauri command, and `rename_launcher_files()` all exist and are registered. The gap is a missing frontend `renameProfile` hook function, missing overwrite protection in `ProfileStore::rename()`, and missing cascade to `last_used_profile` settings. The current bug -- "renaming creates a new profile" -- is a frontend-only issue: `saveProfile` writes to `profileName` without checking whether it differs from `selectedProfile`.

## Architecture Design

### Component Diagram

```
┌─────────────────────────────────────────────────────────────────┐
│  Frontend (React)                                               │
│                                                                 │
│  ProfileFormSections.tsx   useProfile.ts       ProfileActions.tsx│
│  ┌──────────────────┐    ┌──────────────────┐  ┌──────────────┐ │
│  │ Profile Name     │───>│ setProfileName() │  │ Save button  │ │
│  │ <input> field    │    │ profileName state │  │ onClick      │ │
│  └──────────────────┘    │ selectedProfile   │  └──────┬───────┘ │
│                          │                   │         │         │
│                          │ NEW: renameProfile│<────────┘         │
│                          │ (detects name     │  (when name       │
│                          │  change on save)  │   differs from    │
│                          └────────┬──────────┘   selectedProfile)│
│                                   │ invoke()                     │
└───────────────────────────────────┼──────────────────────────────┘
                                    │ IPC
┌───────────────────────────────────┼──────────────────────────────┐
│  Tauri Command Layer              │                              │
│                                   ▼                              │
│  commands/profile.rs::profile_rename(old_name, new_name)         │
│  ┌──────────────────────────────────────────────────────────┐    │
│  │ 1. store.rename(old_name, new_name)   [file rename]     │    │
│  │ 2. Update settings.last_used_profile  [if it matched]   │    │
│  │ 3. Best-effort launcher cascade       [optional]        │    │
│  └──────────────────────────────────────────────────────────┘    │
│                    │                                             │
└────────────────────┼─────────────────────────────────────────────┘
                     │
┌────────────────────┼─────────────────────────────────────────────┐
│  crosshook-core    │                                             │
│                    ▼                                             │
│  ProfileStore::rename()           SettingsStore                  │
│  ┌────────────────────────┐       ┌──────────────────────┐      │
│  │ validate_name(old)     │       │ load() → update      │      │
│  │ validate_name(new)     │       │ last_used_profile     │      │
│  │ check old exists       │       │ → save()              │      │
│  │ NEW: check new !exists │       └──────────────────────┘      │
│  │ fs::rename(old, new)   │                                     │
│  └────────────────────────┘                                     │
│                                                                 │
│  launcher_store.rs::rename_launcher_files()  [already exists]   │
└─────────────────────────────────────────────────────────────────┘
```

### New Components

No new files needed. All changes fit into existing modules:

1. **`useProfile.ts`** -- Add `renameProfile()` function + `renaming` state
2. **`commands/profile.rs`** -- Enhance `profile_rename` to cascade settings + launchers
3. **`toml_store.rs`** -- Add overwrite protection to `ProfileStore::rename()`

### Integration Points

| Component                 | Role                                   | File                                                     |
| ------------------------- | -------------------------------------- | -------------------------------------------------------- |
| `ProfileStore::rename()`  | Core file rename with validation       | `crates/crosshook-core/src/profile/toml_store.rs:163`    |
| `profile_rename` command  | Tauri IPC handler (already registered) | `src-tauri/src/commands/profile.rs:148`                  |
| `useProfile` hook         | Frontend state management              | `src/hooks/useProfile.ts`                                |
| `ProfileFormSections`     | Profile name input field               | `src/components/ProfileFormSections.tsx:320-329`         |
| `ProfileActions`          | Action buttons (Save/Duplicate/Delete) | `src/components/ProfileActions.tsx`                      |
| `ProfileContext`          | Context provider wrapping useProfile   | `src/context/ProfileContext.tsx`                         |
| `AppSettingsData`         | `last_used_profile` field              | `crates/crosshook-core/src/settings/mod.rs:21`           |
| `rename_launcher_files()` | Launcher cascade                       | `crates/crosshook-core/src/export/launcher_store.rs:367` |
| `startup.rs`              | Auto-load uses `last_used_profile`     | `src-tauri/src/startup.rs:34`                            |

## Data Models

### Profile TOML File Structure

Profile name is NOT stored inside the TOML file. The profile name IS the filename:

```
~/.config/crosshook/profiles/
  ├── Elden Ring.toml          ← profile name: "Elden Ring"
  ├── Cyberpunk 2077.toml      ← profile name: "Cyberpunk 2077"
  └── Dark Souls III (Copy).toml
```

TOML contents (no name field at top level -- `game.name` is the game's display name, not the profile name):

```toml
[game]
name = "Elden Ring"
executable_path = "/games/elden-ring/eldenring.exe"

[trainer]
path = "/trainers/elden-ring.exe"
type = "fling"
loading_mode = "source_directory"

[injection]
dll_paths = ["/dlls/a.dll"]
inject_on_launch = [true]

[steam]
enabled = true
app_id = "1245620"
compatdata_path = "/steam/compatdata/1245620"
proton_path = "/steam/proton/proton"

[steam.launcher]
icon_path = "/icons/elden-ring.png"
display_name = "Elden Ring"

[launch]
method = "steam_applaunch"
```

### Rename Mechanics

- **Input**: `old_name: &str`, `new_name: &str`
- **Operation**: `fs::rename("~/.config/crosshook/profiles/{old_name}.toml", "~/.config/crosshook/profiles/{new_name}.toml")`
- **File contents**: Unchanged (byte-for-byte identical after rename)
- **Validation**: Both names pass `validate_name()` which rejects `<>:"/\|?*`, empty, `.`, `..`, absolute paths

### Settings Data Model Impact

```toml
# ~/.config/crosshook/settings.toml
auto_load_last_profile = true
last_used_profile = "Elden Ring"    # ← must update on rename
```

When `last_used_profile == old_name`, the rename command must update it to `new_name`.

### Recent Files Impact

`recent.toml` stores file system paths (`game_paths`, `trainer_paths`, `dll_paths`), NOT profile names. **No impact from profile rename.**

## API Design

### Enhanced Tauri IPC Command: `profile_rename`

The command already exists but needs enhancement. New signature and behavior:

````rust
// commands/profile.rs

/// Renames a profile and cascades to settings and (best-effort) launchers.
///
/// # Frontend invocation
/// ```ts
/// await invoke('profile_rename', { oldName: 'Old Name', newName: 'New Name' });
/// ```
///
/// # Cascade behavior
/// 1. Renames the TOML file via ProfileStore::rename()
/// 2. Updates settings.last_used_profile if it matched old_name
/// 3. Best-effort: no launcher cascade (launcher names derive from
///    steam.launcher.display_name, not profile filename)
///
/// # Errors
/// - Invalid name (path traversal, reserved chars)
/// - Source profile not found
/// - Target name already exists (new error)
/// - IO error during rename
#[tauri::command]
pub fn profile_rename(
    old_name: String,
    new_name: String,
    store: State<'_, ProfileStore>,
    settings_store: State<'_, SettingsStore>,
) -> Result<(), String> {
    // 1. Rename the profile file (with overwrite protection)
    store.rename(&old_name, &new_name).map_err(map_error)?;

    // 2. Update last_used_profile if it references the old name
    if let Ok(mut settings) = settings_store.load() {
        if settings.last_used_profile.trim() == old_name.trim() {
            settings.last_used_profile = new_name.trim().to_string();
            if let Err(err) = settings_store.save(&settings) {
                tracing::warn!(
                    %err,
                    old_name,
                    new_name,
                    "settings update after profile rename failed"
                );
            }
        }
    }

    Ok(())
}
````

**Request format** (TypeScript):

```ts
await invoke('profile_rename', {
  oldName: string, // camelCase for Tauri serde
  newName: string,
});
```

**Response**: `void` on success, `string` error on failure.

**Error cases**:

| Error                 | Cause                                     | User message                              |
| --------------------- | ----------------------------------------- | ----------------------------------------- |
| `InvalidName`         | Name contains forbidden chars or is empty | "invalid profile name: {name}"            |
| `NotFound`            | Old profile doesn't exist on disk         | "profile file not found: {path}"          |
| `AlreadyExists` (NEW) | Target name already taken                 | "a profile named '{name}' already exists" |
| `Io`                  | File system error during rename           | OS error message                          |

### New Error Variant

```rust
// toml_store.rs - ProfileStoreError
pub enum ProfileStoreError {
    InvalidName(String),
    NotFound(PathBuf),
    AlreadyExists(String),  // NEW
    InvalidLaunchOptimizationId(String),
    Io(std::io::Error),
    TomlDe(toml::de::Error),
    TomlSer(toml::ser::Error),
}

// Display impl addition:
Self::AlreadyExists(name) => write!(f, "a profile named '{name}' already exists"),
```

### Enhanced ProfileStore::rename()

```rust
pub fn rename(&self, old_name: &str, new_name: &str) -> Result<(), ProfileStoreError> {
    let old_name = old_name.trim();
    let new_name = new_name.trim();
    validate_name(old_name)?;
    validate_name(new_name)?;

    if old_name == new_name {
        return Ok(()); // no-op
    }

    let old_path = self.profile_path(old_name)?;
    let new_path = self.profile_path(new_name)?;

    if !old_path.exists() {
        return Err(ProfileStoreError::NotFound(old_path));
    }

    // NEW: Overwrite protection
    if new_path.exists() {
        return Err(ProfileStoreError::AlreadyExists(new_name.to_string()));
    }

    fs::rename(&old_path, &new_path)?;
    Ok(())
}
```

### Frontend Hook Addition: `renameProfile`

```ts
// useProfile.ts - new function in UseProfileResult

export interface UseProfileResult {
  // ... existing fields ...
  renameProfile: (oldName: string, newName: string) => Promise<void>;
  renaming: boolean;
}
```

Implementation pattern (follows `duplicateProfile` pattern):

```ts
const renameProfile = useCallback(
  async (oldName: string, newName: string): Promise<void> => {
    const trimmedOld = oldName.trim();
    const trimmedNew = newName.trim();
    if (!trimmedOld || !trimmedNew || trimmedOld === trimmedNew) return;

    setRenaming(true);
    setError(null);
    try {
      await invoke('profile_rename', { oldName: trimmedOld, newName: trimmedNew });
      await refreshProfiles();
      await loadProfile(trimmedNew);
    } catch (err) {
      const message = err instanceof Error ? err.message : String(err);
      setError(message);
    } finally {
      setRenaming(false);
    }
  },
  [loadProfile, refreshProfiles]
);
```

### Save Flow Enhancement

The critical integration: modify `saveProfile` / `persistProfileDraft` to detect name change and call rename first:

```ts
// In persistProfileDraft or saveProfile:
const isRename = selectedProfile && trimmedName !== selectedProfile && profiles.includes(selectedProfile);

if (isRename) {
  // Rename the file first, then save content changes
  await invoke('profile_rename', { oldName: selectedProfile, newName: trimmedName });
}

// Then save profile content to the (possibly new) name
await invoke('profile_save', { name: trimmedName, data: normalizedProfile });
```

## System Constraints

### File System Atomicity

- `fs::rename()` is atomic on the same filesystem (POSIX guarantee). Since source and target are in the same directory (`~/.config/crosshook/profiles/`), this is always same-filesystem.
- Settings update after rename is NOT atomic with the rename. If settings save fails, the profile is renamed but `last_used_profile` is stale. This is acceptable (matches the existing best-effort pattern in `profile_delete`).

### Case Sensitivity

- Linux filesystems (ext4, btrfs) are case-sensitive: "Elden Ring" and "elden ring" are distinct profiles.
- macOS (APFS default) is case-insensitive: renaming "Elden Ring" to "elden ring" may behave differently.
- The current implementation uses `old_name == new_name` string comparison (case-sensitive), which is correct for Linux. macOS would need `fs::rename` to handle it (which it does -- rename to different case works on case-insensitive FS via a temp rename).

### Concurrent Access

- `ProfileStore` is not thread-safe for concurrent writes. Multiple `rename` or `save` calls for the same profile could race. In practice, the Tauri command handler runs on a single thread for `State<'_, ProfileStore>`, so this is safe within the desktop app.
- No locking mechanism is needed for the current architecture.

### Cross-Platform (macOS Support)

- `fs::rename` works identically on macOS. `validate_name` already rejects Windows-reserved characters for forward compatibility.
- AppSettings path differs on macOS (`~/Library/Application Support/crosshook/` vs `~/.config/crosshook/`), but `directories::BaseDirs` handles this.

## Codebase Changes

### Files to Modify

| File                                              | Change                                                                                                                                  | Scope     |
| ------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------- | --------- |
| `crates/crosshook-core/src/profile/toml_store.rs` | Add `AlreadyExists` error variant, add overwrite protection to `rename()`, update `test_rename_overwrites_existing_target_profile` test | ~20 lines |
| `src-tauri/src/commands/profile.rs`               | Enhance `profile_rename` to accept `SettingsStore` state and update `last_used_profile`                                                 | ~15 lines |
| `src/hooks/useProfile.ts`                         | Add `renameProfile()` function, `renaming` state, integrate into save flow                                                              | ~30 lines |
| `src/components/ProfileActions.tsx`               | Add `renaming` prop to disable buttons during rename                                                                                    | ~5 lines  |
| `src/context/ProfileContext.tsx`                  | Pass through `renaming` from hook                                                                                                       | ~2 lines  |
| `src/components/pages/ProfilesPage.tsx`           | Wire `renaming` state                                                                                                                   | ~3 lines  |

### Files to Read but NOT Modify

| File                                                 | Reason                                                                                                     |
| ---------------------------------------------------- | ---------------------------------------------------------------------------------------------------------- |
| `crates/crosshook-core/src/profile/models.rs`        | Profile data model unchanged                                                                               |
| `crates/crosshook-core/src/settings/mod.rs`          | Settings model unchanged, just used                                                                        |
| `crates/crosshook-core/src/settings/recent.rs`       | No impact (stores paths, not names)                                                                        |
| `crates/crosshook-core/src/export/launcher_store.rs` | Launcher cascade is NOT needed for profile rename (launchers derive from `display_name`, not profile name) |
| `src-tauri/src/lib.rs`                               | `profile_rename` already registered in `invoke_handler`                                                    |
| `src-tauri/src/startup.rs`                           | Will automatically work once `last_used_profile` is updated                                                |
| `src/types/profile.ts`                               | No new types needed for rename                                                                             |

### Dependencies

- No new crate dependencies.
- No new npm dependencies.
- `SettingsStore` is already managed as Tauri state (lib.rs:62).

## Technical Decisions

### Decision 1: Overwrite Protection

| Option                         | Description                                     | Recommendation                    |
| ------------------------------ | ----------------------------------------------- | --------------------------------- |
| A: Add `AlreadyExists` error   | Check `new_path.exists()` before `fs::rename()` | **Recommended**                   |
| B: Add `force: bool` parameter | Allow caller to opt into overwrite              | Over-engineered for current needs |
| C: Keep silent overwrite       | Current behavior                                | Dangerous for user-facing rename  |

**Rationale**: Option A is the simplest safe approach. The existing `test_rename_overwrites_existing_target_profile` test should be updated to expect `AlreadyExists` error instead, or moved to a separate `rename_force` method if force-overwrite is ever needed.

### Decision 2: Settings Cascade Location

| Option                       | Description                               | Recommendation                                                    |
| ---------------------------- | ----------------------------------------- | ----------------------------------------------------------------- |
| A: In Tauri command          | Single IPC call handles rename + settings | **Recommended**                                                   |
| B: In frontend hook          | Multiple IPC calls from frontend          | More round-trips, partial failure                                 |
| C: In ProfileStore::rename() | Core library handles settings             | Violates separation (profile store shouldn't know about settings) |

**Rationale**: Option A matches the existing `profile_delete` pattern which does best-effort launcher cleanup in the Tauri command layer.

### Decision 3: Launcher Cascade

| Option                         | Description                               | Recommendation         |
| ------------------------------ | ----------------------------------------- | ---------------------- |
| A: No launcher cascade         | Profile rename only renames the TOML file | **Recommended**        |
| B: Best-effort launcher rename | Also rename exported launchers            | Unnecessary complexity |

**Rationale**: Exported launchers derive their filenames from `steam.launcher.display_name`, NOT from the profile filename. Renaming "Elden Ring" profile to "ER" does not affect the launcher named "Elden Ring - Trainer". If the user also changes the display name in the profile editor, that's a separate save operation that can trigger launcher rename independently via the existing `rename_launcher` command.

### Decision 4: Frontend Integration

| Option                    | Description                                              | Recommendation                           |
| ------------------------- | -------------------------------------------------------- | ---------------------------------------- |
| A: Detect in save flow    | `saveProfile` checks if name changed, calls rename first | **Recommended**                          |
| B: Separate Rename button | Add explicit rename UI                                   | Over-complicated for inline name editing |
| C: Both                   | Inline detection + explicit action                       | Confusing UX                             |

**Rationale**: Option A is the most natural UX. Users expect to change the name field and hit Save. The hook detects `profileName !== selectedProfile` and handles the rename transparently. No new UI elements needed.

## Cross-Team Synthesis

### UX Researcher Recommendations vs Technical Constraints

The UX researcher recommended a **modal dialog** for rename instead of inline save-flow detection. After reviewing the codebase:

1. **Modal pattern exists**: `ProfileReviewModal.tsx` provides focus trapping, portal rendering, and keyboard/gamepad navigation. The delete confirmation uses a lighter inline overlay (`crosshook-profile-editor-delete-overlay`). A rename dialog could follow either pattern.

2. **Modal vs inline trade-off**: The UX researcher argues that the profile name field serving dual purpose (create + edit) is confusing. Their recommendation: make the name field **read-only for existing profiles** and add a Rename action (button or context menu) that opens a modal. This is a cleaner separation of concerns than detecting name divergence in the save flow.

3. **Revised technical recommendation**: Adopt the modal approach. It aligns better with:
   - The existing `PendingDelete` confirmation pattern (state-driven overlay)
   - Gamepad/controller navigation (focus trapping in modals works with `useGamepadNav`)
   - Clear transaction boundaries (rename is a discrete action, not a side-effect of save)

4. **No rollback needed**: The UX researcher suggested rollback if launcher cascade fails. However, since launcher names derive from `display_name` (not profile name), there IS no launcher cascade on profile rename. The rename is a single `fs::rename` + settings update, both of which succeed or fail independently. Best-effort settings update (matching the delete pattern) is sufficient.

5. **Undo via reverse rename**: The UX researcher suggested an undo command. Technically, `profile_rename(newName, oldName)` is a valid undo operation. The frontend could store the previous name in a toast's callback closure and call rename with swapped args. No dedicated undo API needed.

### Business Analyzer Confirmation

The business analyzer confirmed all technical findings regarding:

- Profile identity = filename (no internal name field)
- `last_used_profile` cascade requirement
- `fs::rename` atomicity guarantees
- Overwrite protection need (current code silently overwrites)

### API Researcher Confirmation

The API researcher confirmed no new dependencies are needed. `std::fs::rename` with a pre-existence check is sufficient. The `AlreadyExists` error variant approach was independently recommended.

## Open Questions

1. **Modal vs inline rename UX**: Should rename use a dedicated modal dialog (UX researcher recommendation) or detect name change in the save flow (original technical recommendation)? The modal approach is cleaner but adds a new UI component. **Revised recommendation: modal.**

2. **Read-only name field for existing profiles**: The UX researcher recommends making the profile name field read-only when editing an existing profile. This eliminates the "save creates new profile" confusion entirely. The name field stays editable only when creating a new profile (no `selectedProfile`). **This is the simplest fix for the original bug.**

3. **Should renaming to an existing profile name show a confirmation dialog or just an error?** Recommendation: error with `AlreadyExists`. The overwrite path can be achieved by deleting the target first.

4. **Case-insensitive rename on macOS**: On case-insensitive filesystems, renaming "Game" to "game" would be flagged as `AlreadyExists` since `new_path.exists()` returns true for the existing file. This needs special handling (check if old_path and new_path resolve to the same inode). This is a macOS-only concern and can be deferred if macOS support is not imminent.
