# Profile Rename — Code Analysis

## Executive Summary

The profile-rename feature spans three implementation layers: Rust core (`ProfileStore::rename()`), Tauri IPC command (`profile_rename`), and React frontend (`useProfile` hook + UI components). The backend rename is ~90% complete — it performs `fs::rename` with validation but lacks an `AlreadyExists` overwrite guard and settings cascade. The Tauri command is a thin pass-through that needs a `SettingsStore` state parameter. The frontend requires a new `renameProfile()` callback (modeled on `duplicateProfile()`), a Rename button in `ProfileActions`, and a rename modal dialog in `ProfilesPage` (modeled on the delete overlay).

---

## Existing Code Structure

### Layer 1: Rust Core — `ProfileStore` (`toml_store.rs`)

**File**: `crates/crosshook-core/src/profile/toml_store.rs`

```
ProfileStore { base_path: PathBuf }
├── load(&self, name) -> Result<GameProfile, ProfileStoreError>
├── save(&self, name, profile) -> Result<(), ProfileStoreError>
├── list(&self) -> Result<Vec<String>, ProfileStoreError>
├── delete(&self, name) -> Result<(), ProfileStoreError>
├── rename(&self, old_name, new_name) -> Result<(), ProfileStoreError>  ← EXISTS
├── duplicate(&self, source_name) -> Result<DuplicateProfileResult, ProfileStoreError>
├── import_legacy(&self, legacy_path) -> Result<GameProfile, ProfileStoreError>
├── save_launch_optimizations(&self, name, ids) -> Result<(), ProfileStoreError>
└── profile_path(&self, name) -> Result<PathBuf, ProfileStoreError>  (private)
```

**`rename()` implementation (line 163-178)**:

```rust
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

**Gap**: No `AlreadyExists` check before `fs::rename`. The current test `test_rename_overwrites_existing_target_profile` (line 515) explicitly validates overwrite behavior — adding an `AlreadyExists` guard will require updating this test.

**Error enum (line 16-23)**:

```rust
pub enum ProfileStoreError {
    InvalidName(String),
    NotFound(PathBuf),
    InvalidLaunchOptimizationId(String),
    Io(std::io::Error),
    TomlDe(toml::de::Error),
    TomlSer(toml::ser::Error),
}
```

Missing: `AlreadyExists(String)` variant needed for the overwrite guard.

### Layer 2: Tauri Command — `profile_rename` (`commands/profile.rs`)

**File**: `src-tauri/src/commands/profile.rs`

**Current implementation (line 147-154)**:

```rust
#[tauri::command]
pub fn profile_rename(
    old_name: String,
    new_name: String,
    store: State<'_, ProfileStore>,
) -> Result<(), String> {
    store.rename(&old_name, &new_name).map_err(map_error)
}
```

**Gap**: Missing `State<'_, SettingsStore>` parameter and `last_used_profile` cascade logic.

**Reference pattern — `profile_delete` (line 112-123)** shows best-effort side effects:

```rust
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

**Command registration (lib.rs line 96)**: `commands::profile::profile_rename` is already registered in `generate_handler![]`. No changes needed.

**State management (lib.rs lines 62-65)**: Both `ProfileStore` and `SettingsStore` are already `.manage()`d:

```rust
.manage(profile_store)
.manage(settings_store)
```

### Layer 3: React Frontend

#### `useProfile` Hook (`hooks/useProfile.ts`)

**`duplicateProfile` pattern (line 569-588)** — exact template for `renameProfile`:

```typescript
const duplicateProfile = useCallback(
  async (sourceName: string): Promise<void> => {
    if (!sourceName.trim()) return;
    setDuplicating(true); // 1. Set loading flag
    setError(null); // 2. Clear error
    try {
      const result = await invoke<DuplicateProfileResult>('profile_duplicate', {
        name: sourceName,
      }); // 3. IPC call
      await refreshProfiles(); // 4. Refresh list
      await loadProfile(result.name); // 5. Auto-select
    } catch (err) {
      const message = err instanceof Error ? err.message : String(err);
      setError(message); // 6. Error display
    } finally {
      setDuplicating(false); // 7. Clear flag
    }
  },
  [loadProfile, refreshProfiles]
);
```

**State variables pattern**: Each operation has a boolean state flag:

- `duplicating` (line 329): `useState(false)` + returned in hook result
- `deleting` (line 328): same pattern

**`UseProfileResult` interface (line 20-48)**: New fields `renaming: boolean` and `renameProfile: (oldName: string, newName: string) => Promise<void>` must be added here.

**Autosave timer concern**: `launchOptimizationsAutosaveTimerRef` (line 335) runs a debounced save. If rename happens mid-autosave, the autosave would target the old profile name. Must clear timer before rename.

#### `ProfileActions` Component (`components/ProfileActions.tsx`)

**Props interface (line 8-24)**:

```typescript
export interface ProfileActionsProps {
  dirty: boolean;
  loading: boolean;
  saving: boolean;
  deleting: boolean;
  duplicating: boolean;
  error: string | null;
  canSave: boolean;
  canDelete: boolean;
  canDuplicate: boolean;
  onSave: () => void | Promise<void>;
  onDelete: () => void | Promise<void>;
  onDuplicate: () => void | Promise<void>;
}
```

**Button layout pattern (lines 42-61)**: Buttons are `<button>` elements in a flex container with `gap: 12`. Each button follows:

```tsx
<button
  type="button"
  className="crosshook-button crosshook-button--secondary"
  onClick={() => void onAction()}
  disabled={!canAction || actionInProgress}
>
  {actionInProgress ? 'Acting...' : 'Action'}
</button>
```

New props needed: `canRename: boolean`, `renaming: boolean`, `onRename: () => void | Promise<void>`.

#### `ProfilesPage` Component (`components/pages/ProfilesPage.tsx`)

**Capability guard pattern (lines 69-70)**:

```typescript
const canDelete = profileExists && !saving && !deleting && !loading && !duplicating;
const canDuplicate = profileExists && !saving && !deleting && !loading && !duplicating;
```

New: `const canRename = profileExists && !saving && !deleting && !loading && !duplicating && !renaming;`

**Delete overlay dialog pattern (lines 179-216)**:

```tsx
{
  pendingDelete ? (
    <div className="crosshook-profile-editor-delete-overlay" data-crosshook-focus-root="modal">
      <div className="crosshook-profile-editor-delete-dialog">
        <h3>Delete Profile</h3>
        <p>
          Delete profile <strong>{pendingDelete.name}</strong>?
        </p>
        {/* conditional warning content */}
        <div className="crosshook-profile-editor-delete-actions">
          <button onClick={cancelDelete} data-crosshook-modal-close>
            Cancel
          </button>
          <button onClick={() => void executeDelete()}>Delete Profile</button>
        </div>
      </div>
    </div>
  ) : null;
}
```

Rename modal will follow this structure but with a text input for the new name instead of warning content.

**Context auto-extension** (`context/ProfileContext.tsx` line 51-58):

```typescript
const value = useMemo<ProfileContextValue>(
  () => ({
    ...profileState, // ← spread means new hook fields flow automatically
    launchMethod,
    steamClientInstallPath,
    targetHomePath,
  }),
  [launchMethod, profileState, steamClientInstallPath, targetHomePath]
);
```

`ProfileContextValue extends UseProfileResult` so new fields (`renaming`, `renameProfile`) are auto-exposed. No changes needed to context files.

#### `ProfileFormSections` — Profile Name Input (line 323-329)

```tsx
<input
    id={profileNamesListId}
    className="crosshook-input"
    list={...}
    value={profileName}
    placeholder="Enter or choose a profile name"
    onChange={(event) => onProfileNameChange(event.target.value)}
/>
```

This should be made read-only for existing (saved) profiles to prevent accidental inline renaming. Add `readOnly={profileExists}` or similar.

### Settings Store (`settings/mod.rs`)

**`AppSettingsData` (line 19-25)**:

```rust
pub struct AppSettingsData {
    pub auto_load_last_profile: bool,
    pub last_used_profile: String,
    pub community_taps: Vec<CommunityTapSubscription>,
}
```

**Cascade logic needed**: When renaming from `old_name` to `new_name`, if `settings.last_used_profile == old_name`, update it to `new_name`.

**Access pattern from Tauri commands**: `State<'_, SettingsStore>` injected via Tauri DI (see `commands/settings.rs` line 16-22).

---

## Implementation Patterns

### Pattern: ProfileStore Method

All `ProfileStore` methods follow this sequence:

1. Accept `&self` + string name parameter(s)
2. Call `validate_name()` for each name
3. Resolve path via `self.profile_path(name)`
4. Check existence (`path.exists()`)
5. Perform filesystem operation
6. Return `Result<T, ProfileStoreError>`

**Example** — `delete()` (line 153-161):

```rust
pub fn delete(&self, name: &str) -> Result<(), ProfileStoreError> {
    let path = self.profile_path(name)?;       // validates + resolves
    if !path.exists() {
        return Err(ProfileStoreError::NotFound(path));
    }
    fs::remove_file(path)?;
    Ok(())
}
```

### Pattern: Tauri Command with Side Effects

Profile operations that have side effects use a best-effort approach: the primary operation succeeds or fails, and side effects are logged warnings on failure.

**Example** — `profile_delete` (line 112-123):

```rust
// Side effect: best-effort launcher cleanup
if let Ok(profile) = store.load(&name) {
    if let Err(error) = cleanup_launchers_for_profile_delete(&name, &profile) {
        tracing::warn!("Launcher cleanup failed for profile {name}: {error}");
    }
}
// Primary operation: must succeed or return error
store.delete(&name).map_err(map_error)
```

For `profile_rename`, the settings cascade is the side effect:

```rust
// Primary: rename the file
store.rename(&old_name, &new_name).map_err(map_error)?;
// Side effect: cascade last_used_profile
if let Ok(settings) = settings_store.load() {
    if settings.last_used_profile == old_name {
        let mut updated = settings;
        updated.last_used_profile = new_name.clone();
        if let Err(error) = settings_store.save(&updated) {
            tracing::warn!("Settings cascade failed after rename: {error}");
        }
    }
}
Ok(())
```

### Pattern: Error Enum Extension

`ProfileStoreError` uses named tuple variants with `Display` impl and `From` impls for wrapped types.

**To add `AlreadyExists`**:

```rust
// In the enum:
AlreadyExists(String),

// In Display impl:
Self::AlreadyExists(name) => write!(f, "a profile named '{name}' already exists"),
```

No `From` impl needed (it's constructed directly, not converted from another error type).

### Pattern: React Hook IPC Callback

The `duplicateProfile` pattern (6 steps) is the exact template:

1. Guard: `if (!sourceName.trim()) return;`
2. Set loading flag: `setRenaming(true);`
3. Clear error: `setError(null);`
4. IPC call: `await invoke('profile_rename', { oldName, newName });`
5. Refresh + reload: `await refreshProfiles(); await loadProfile(newName);`
6. Error catch: `setError(message);`
7. Clear flag: `setRenaming(false);`

**Critical addition**: Before IPC, cancel autosave timer:

```typescript
if (launchOptimizationsAutosaveTimerRef.current !== null) {
  clearTimeout(launchOptimizationsAutosaveTimerRef.current);
  launchOptimizationsAutosaveTimerRef.current = null;
}
```

### Pattern: UI Action Button

**Props triple**: `canX: boolean` (guard), `xing: boolean` (loading), `onX: () => void | Promise<void>` (callback).

**JSX**:

```tsx
<button
  type="button"
  className="crosshook-button crosshook-button--secondary"
  onClick={() => void onRename()}
  disabled={!canRename || renaming}
>
  {renaming ? 'Renaming...' : 'Rename'}
</button>
```

### Pattern: Modal Overlay Dialog

**State**: `const [pendingRename, setPendingRename] = useState<string | null>(null);` (holds the profile name being renamed, or null when closed).

**Structure**: Overlay `div` with `data-crosshook-focus-root="modal"` → dialog `div` → heading + content + action buttons. Cancel button has `data-crosshook-modal-close` attribute for keyboard/gamepad handling.

---

## Integration Points

### Files to Modify

| File                                              | Changes                                                                                                      |
| ------------------------------------------------- | ------------------------------------------------------------------------------------------------------------ |
| `crates/crosshook-core/src/profile/toml_store.rs` | Add `AlreadyExists` variant to `ProfileStoreError`; add existence check in `rename()`; update overwrite test |
| `src-tauri/src/commands/profile.rs`               | Add `State<'_, SettingsStore>` param to `profile_rename`; add settings cascade logic                         |
| `src/hooks/useProfile.ts`                         | Add `renaming` state; add `renameProfile()` callback; cancel autosave timer; export in `UseProfileResult`    |
| `src/components/ProfileActions.tsx`               | Add `canRename`, `renaming`, `onRename` props; add Rename button between Duplicate and Delete                |
| `src/components/pages/ProfilesPage.tsx`           | Add `canRename` guard; add rename modal state + overlay JSX; wire `onRename` to open modal                   |
| `src/components/ProfileFormSections.tsx`          | Make profile name input read-only when `profileExists` is true                                               |

### Files NOT to Modify

| File                                                 | Reason                                                                                                  |
| ---------------------------------------------------- | ------------------------------------------------------------------------------------------------------- |
| `src-tauri/src/lib.rs`                               | `profile_rename` already registered in `generate_handler![]` at line 96                                 |
| `src-tauri/src/startup.rs`                           | `resolve_auto_load_profile_name` reads `last_used_profile` — works automatically after settings cascade |
| `src/context/ProfileContext.tsx`                     | Uses `...profileState` spread — new hook fields flow through automatically                              |
| `src/types/profile.ts`                               | `profile_rename` returns `void` (no new IPC result type needed)                                         |
| `crates/crosshook-core/src/profile/models.rs`        | Profile name is filename stem, not stored inside TOML                                                   |
| `crates/crosshook-core/src/export/launcher_store.rs` | Launcher paths derive from `display_name`, not profile name                                             |

---

## Code Conventions

### Rust

- `snake_case` for all names
- Error handling: `Result<T, ProfileStoreError>` with `map_error()` at IPC boundary (converts to `String`)
- Tests: `tempdir()` + `ProfileStore::with_base_path()` for isolated filesystem tests
- Side effects: best-effort with `tracing::warn!` on failure
- Validation: `validate_name()` called at start of every public method that takes a name

### TypeScript / React

- `camelCase` for hooks/functions, `PascalCase` for components
- `invoke<ReturnType>('command_name', { param })` for Tauri IPC
- `useState` boolean flags per async operation
- `useCallback` with explicit dependency arrays for all callbacks
- Error formatting: `err instanceof Error ? err.message : String(err)`

---

## Dependencies and Services

### Tauri State Dependencies for `profile_rename`

Currently:

```rust
store: State<'_, ProfileStore>
```

Needs:

```rust
store: State<'_, ProfileStore>,
settings_store: State<'_, SettingsStore>,
```

Both are already registered via `.manage()` in `lib.rs` (lines 62-63).

### Frontend Dependencies

- `@tauri-apps/api/core` → `invoke` (already imported in `useProfile.ts`)
- No new npm packages needed
- No new TypeScript types needed (rename returns `void`)

---

## Gotchas and Warnings

1. **Overwrite test must change**: `test_rename_overwrites_existing_target_profile` (toml_store.rs line 515) currently asserts that rename silently overwrites. Adding `AlreadyExists` guard means this test must flip to assert an error is returned instead.

2. **Autosave timer race**: The launch optimizations autosave timer (`launchOptimizationsAutosaveTimerRef`) fires on a 350ms delay. If rename completes while a timer is pending, the delayed write targets the old file path (which no longer exists). Must `clearTimeout` before invoking `profile_rename`.

3. **Tauri IPC param casing**: Tauri's serde deserialization uses `camelCase` by default for command parameters. The `profile_rename` command accepts `old_name` / `new_name` in Rust, but the frontend `invoke` call must use `oldName` / `newName` (camelCase). This matches the existing pattern where `profile_save` accepts `data` and `name`.

4. **Profile name is filename, not field**: Profile identity is the TOML filename stem, NOT a field inside the TOML data. `rename()` does `fs::rename` on the `.toml` file without touching file contents. The `GameProfile` struct has no `name` field for this — `game.name` is the display name of the game, not the profile identifier.

5. **`last_used_profile` cascade is async-safe**: The settings cascade in the Tauri command runs synchronously on the Tauri main thread. There's no risk of concurrent access from the frontend because Tauri serializes command execution per-window by default.

6. **No launcher cascade needed**: Launcher file paths are derived from `steam.launcher.display_name` and `steam.app_id` (see `export/launcher_store.rs`), not from the profile name. Renaming a profile does NOT affect exported launcher scripts.

7. **`readOnly` on profile name input**: Making the input read-only for existing profiles prevents accidental inline editing. Users must use the explicit Rename button/modal for controlled renaming. The `datalist` suggestions still work for new profile creation.

8. **Context auto-extension**: `ProfileContextValue extends UseProfileResult` with spread `...profileState`. Adding `renaming` and `renameProfile` to `UseProfileResult` makes them available through context without modifying `ProfileContext.tsx`.

---

## Task-Specific Guidance

### Task: Add `AlreadyExists` guard to `ProfileStore::rename()`

**Where**: `toml_store.rs` line 163-178
**What**: After `validate_name(new_name)?` and before `fs::rename`, check if `new_path.exists()` and return `Err(ProfileStoreError::AlreadyExists(new_name.to_string()))`.
**Also**: Add `AlreadyExists(String)` variant to the error enum (line 16-23), add Display match arm, update `test_rename_overwrites_existing_target_profile` to assert error.
**Test pattern**: Follow `test_rename_not_found` (line 476) — create two profiles, attempt rename from one to the other, assert `AlreadyExists` error.

### Task: Add settings cascade to `profile_rename` command

**Where**: `commands/profile.rs` line 147-154
**What**: Add `settings_store: State<'_, SettingsStore>` parameter. After successful `store.rename()`, load settings, check `last_used_profile`, update if matching.
**Pattern**: Follow `profile_delete`'s best-effort side effect pattern with `tracing::warn!` on failure.
**Import**: Add `use crosshook_core::settings::SettingsStore;` at top of file (currently only `ProfileStore` types are imported).

### Task: Add `renameProfile()` to `useProfile` hook

**Where**: `hooks/useProfile.ts`
**What**:

1. Add `const [renaming, setRenaming] = useState(false);` (near line 329)
2. Add `renameProfile` callback following `duplicateProfile` pattern (after line 588)
3. Cancel autosave timer at start of callback
4. Invoke `'profile_rename'` with `{ oldName, newName }`
5. Refresh + load new profile name
6. Add `renaming` and `renameProfile` to `UseProfileResult` interface and return object

### Task: Add Rename button to `ProfileActions`

**Where**: `components/ProfileActions.tsx`
**What**: Add `canRename`, `renaming`, `onRename` to props interface. Add button between Duplicate and Delete buttons. Follow existing button pattern.

### Task: Add rename modal to `ProfilesPage`

**Where**: `components/pages/ProfilesPage.tsx`
**What**:

1. Add `pendingRename` state (string | null)
2. Add `canRename` guard (line ~70)
3. Wire `onRename` to set `pendingRename`
4. Add rename overlay dialog after delete overlay (line ~216), with text input for new name
5. On confirm, call `renameProfile(pendingRename, newNameValue)` then clear `pendingRename`

### Task: Make profile name read-only for existing profiles

**Where**: `components/ProfileFormSections.tsx` line 323
**What**: Add `readOnly` prop to the `<input>` element when the profile is an existing saved profile. The `profileExists` boolean or equivalent must be passed as a prop.
