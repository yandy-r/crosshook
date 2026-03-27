# Research: Coding Patterns & Conventions for Profile Rename

Comprehensive analysis of codebase patterns relevant to implementing profile rename. All patterns documented with concrete file references to guide implementation.

## Relevant Files

- `crates/crosshook-core/src/profile/toml_store.rs`: ProfileStore, ProfileStoreError, rename/duplicate methods, validate_name, all tests
- `src-tauri/src/commands/profile.rs`: Tauri IPC commands (profile_rename, profile_duplicate, profile_delete), map_error helper
- `src-tauri/src/lib.rs`: Tauri state management, command registration via `generate_handler!`
- `src/hooks/useProfile.ts`: Profile CRUD hook — duplicateProfile pattern is the template for renameProfile
- `src/components/ProfileActions.tsx`: Action bar UI (Save, Duplicate, Delete buttons + props interface)
- `src/components/pages/ProfilesPage.tsx`: Page-level wiring — derives canDuplicate/canDelete, passes props to ProfileActions
- `src/context/ProfileContext.tsx`: Context provider that extends UseProfileResult with derived values
- `src/types/profile.ts`: TypeScript types for IPC boundary (GameProfile, DuplicateProfileResult)
- `crates/crosshook-core/src/settings/mod.rs`: SettingsStore, AppSettingsData (last_used_profile field), load/save pattern
- `src-tauri/src/commands/settings.rs`: Settings Tauri commands, map_settings_error pattern

## Architectural Patterns

### Tauri Command Structure

Every Tauri IPC command follows a consistent pattern in `src-tauri/src/commands/profile.rs`:

```rust
#[tauri::command]
pub fn command_name(
    param: Type,
    store: State<'_, StoreType>,
) -> Result<ReturnType, String> {
    store.method(&param).map_err(map_error)
}
```

Key observations:

- **Error mapping**: Each command module defines a local `fn map_error(error: DomainError) -> String` that calls `.to_string()`. Profile commands use `map_error`, settings commands use `map_settings_error`. Errors cross IPC as `String`.
- **State injection**: Stores are injected via `tauri::State<'_, StoreType>`. Multiple stores can be injected in the same command (see `profile_delete` loading both profile and launcher state).
- **Command registration**: All commands are registered in `src-tauri/src/lib.rs:70-109` via `tauri::generate_handler![]`. `profile_rename` is already registered at line 96.
- **Naming convention**: Rust function names are `snake_case` and match the frontend `invoke('profile_rename', ...)` call exactly.

### Adding a Second State Parameter to profile_rename

The current `profile_rename` command only takes `ProfileStore`. To cascade `last_used_profile`, it needs `SettingsStore` as a second parameter. This pattern already exists in the codebase:

```rust
// Current (needs enhancement):
pub fn profile_rename(
    old_name: String,
    new_name: String,
    store: State<'_, ProfileStore>,
) -> Result<(), String> {
    store.rename(&old_name, &new_name).map_err(map_error)
}

// Target pattern (follows profile_delete's cascade approach):
pub fn profile_rename(
    old_name: String,
    new_name: String,
    store: State<'_, ProfileStore>,
    settings_store: State<'_, SettingsStore>,  // NEW
) -> Result<(), String> {
    store.rename(&old_name, &new_name).map_err(map_error)?;
    // Best-effort settings cascade (see profile_delete for launcher cascade pattern)
    ...
    Ok(())
}
```

The `profile_delete` command (lines 113-123) demonstrates best-effort side effects: it attempts launcher cleanup but proceeds with deletion even if cleanup fails. The `profile_rename` settings cascade should follow this same pattern — log warning on failure, don't fail the rename.

### ProfileStore Method Pattern

All `ProfileStore` methods in `toml_store.rs` follow:

1. Accept `&self` + string name params
2. Call `validate_name()` for safety
3. Resolve path via `self.profile_path(name)?`
4. Check existence (`!path.exists()` → `NotFound`)
5. Perform filesystem operation
6. Return `Result<T, ProfileStoreError>`

The `rename()` method (lines 163-178) already follows steps 1-5 but is missing the `AlreadyExists` check before step 5.

### React Hook Pattern (useProfile.ts)

The `duplicateProfile` callback (lines 569-588) is the exact template for `renameProfile`:

```typescript
const duplicateProfile = useCallback(
  async (sourceName: string): Promise<void> => {
    if (!sourceName.trim()) return; // 1. Guard
    setDuplicating(true); // 2. Set loading flag
    setError(null); // 3. Clear error
    try {
      const result = await invoke<T>('profile_duplicate', { name: sourceName }); // 4. IPC call
      await refreshProfiles(); // 5. Refresh list
      await loadProfile(result.name); // 6. Select new name
    } catch (err) {
      // 7. Error handling
      const message = err instanceof Error ? err.message : String(err);
      setError(message);
    } finally {
      setDuplicating(false); // 8. Clear loading flag
    }
  },
  [loadProfile, refreshProfiles] // 9. Deps
);
```

The `renameProfile` function should follow this same 9-step pattern:

- `setRenaming(true)` / `setRenaming(false)` for loading state
- `invoke('profile_rename', { oldName, newName })` for IPC
- `refreshProfiles()` + `loadProfile(newName)` for post-rename state sync
- Same error handling pattern

### UI Component Pattern (ProfileActions.tsx)

The component follows a props interface pattern with:

- Boolean state flags: `saving`, `deleting`, `duplicating` (add `renaming`)
- Boolean capability flags: `canSave`, `canDelete`, `canDuplicate` (add `canRename`)
- Callback props: `onSave`, `onDelete`, `onDuplicate` (add `onRename`)
- Button rendering: `disabled={!canX || xing}` with label `{xing ? 'Xing...' : 'X'}`

### ProfilesPage Wiring Pattern

`ProfilesPage.tsx` derives capability flags from state:

```typescript
const canDelete = profileExists && !saving && !deleting && !loading && !duplicating;
const canDuplicate = profileExists && !saving && !deleting && !loading && !duplicating;
// canRename should follow same pattern:
// const canRename = profileExists && !saving && !deleting && !loading && !duplicating && !renaming;
```

### Context Pass-Through

`ProfileContext.tsx` extends `UseProfileResult` with derived values. The context interface automatically includes all `UseProfileResult` fields via `extends`, so adding `renaming`, `duplicating`, and `renameProfile` to `UseProfileResult` automatically surfaces them through context — no manual pass-through needed.

## Code Conventions

### Rust

- **Naming**: `snake_case` everywhere — functions, variables, modules, Tauri commands
- **Module organization**: Directories with `mod.rs` (e.g., `profile/mod.rs` re-exports from `toml_store.rs`, `models.rs`)
- **Error enums**: Named `{Module}StoreError` with variants like `InvalidName(String)`, `NotFound(PathBuf)`, `Io(std::io::Error)`
- **Display impl**: Match on each variant with `write!(f, "human-readable: {field}")`
- **From impls**: For each wrapped error type (e.g., `From<std::io::Error>` → `Self::Io`)
- **Serde**: `#[derive(Serialize, Deserialize)]` on all IPC types; `#[serde(rename = ...)]` for cross-boundary field names
- **Doc comments**: `///` on public methods with markdown, including `# Errors` and `# Safety constraints` sections
- **Imports**: Grouped by `use super::`, `use crate::`, `use external_crate::`, then `use std::`

### TypeScript/React

- **Naming**: `PascalCase` components, `camelCase` functions/hooks/variables
- **Hook exports**: Named `useX` with `UseXResult` interface for return type, `UseXOptions` for params
- **Type exports**: `export interface` in `types/*.ts`, re-exported from `types/index.ts`
- **IPC calls**: `invoke<ReturnType>('command_name', { param: value })` from `@tauri-apps/api/core`
- **Error formatting**: `err instanceof Error ? err.message : String(err)` — used consistently across all catch blocks
- **State pattern**: `const [flag, setFlag] = useState<boolean>(false)` for boolean loading states
- **CSS classes**: `crosshook-*` prefix with BEM-like modifiers (`crosshook-button--secondary`)

### Import Order in TypeScript

```typescript
// 1. React/third-party
import { useCallback, useState } from 'react';
import { invoke } from '@tauri-apps/api/core';
// 2. Local types
import type { GameProfile, DuplicateProfileResult } from '../types';
// 3. Local utilities/hooks
import { resolveLaunchMethod } from '../utils/launch';
```

## Error Handling

### Rust Error Flow

```
ProfileStoreError (enum)
  → Display impl (human-readable string)
  → map_error() in commands/profile.rs (calls .to_string())
  → Result<T, String> across IPC boundary
  → catch block in React hook
  → setError(message) in component state
```

Error variants in `ProfileStoreError`:

- `InvalidName(String)` → `"invalid profile name: {name}"`
- `NotFound(PathBuf)` → `"profile file not found: {path}"`
- `InvalidLaunchOptimizationId(String)` → `"unknown launch optimization id: {id}"`
- `Io(std::io::Error)` → OS error message
- `TomlDe/TomlSer` → Parsing error messages
- **NEW**: `AlreadyExists(String)` → `"a profile named '{name}' already exists"`

### Best-Effort Side Effects Pattern

The `profile_delete` command demonstrates the pattern for non-critical side effects:

```rust
// Best-effort launcher cleanup before profile deletion.
// Profile deletion must succeed even if launcher cleanup fails.
if let Ok(profile) = store.load(&name) {
    if let Err(error) = cleanup_launchers_for_profile_delete(&name, &profile) {
        tracing::warn!("Launcher cleanup failed for profile {name}: {error}");
    }
}
store.delete(&name).map_err(map_error)
```

The `profile_rename` settings cascade should follow this exact pattern:

```rust
store.rename(&old_name, &new_name).map_err(map_error)?;
// Best-effort: update last_used_profile if it matches old name
if let Ok(mut settings) = settings_store.load() {
    if settings.last_used_profile.trim() == old_name.trim() {
        settings.last_used_profile = new_name.trim().to_string();
        if let Err(err) = settings_store.save(&settings) {
            tracing::warn!(%err, old_name, new_name, "settings update after profile rename failed");
        }
    }
}
Ok(())
```

### Frontend Error Handling

All hook callbacks use the same try/catch/finally pattern:

```typescript
try {
  await invoke('command', { params });
  // Success: refresh state
} catch (err) {
  const message = err instanceof Error ? err.message : String(err);
  setError(message);
} finally {
  setLoadingFlag(false);
}
```

Errors display in the `ProfileActions` component via the `error` prop rendered in a `crosshook-error-banner` div.

## Testing Approach

### Rust Test Patterns (toml_store.rs)

Tests in `toml_store.rs` follow a consistent pattern:

1. **Setup**: Create `tempdir()`, build `ProfileStore::with_base_path(temp.path().join("profiles"))`, create `sample_profile()`
2. **Act**: Call the store method
3. **Assert**: Verify filesystem state and/or loaded data

```rust
#[test]
fn test_rename_success() {
    let temp_dir = tempdir().unwrap();
    let store = ProfileStore::with_base_path(temp_dir.path().join("profiles"));
    let profile = sample_profile();

    store.save("old-name", &profile).unwrap();
    store.rename("old-name", "new-name").unwrap();

    assert!(!store.profile_path("old-name").unwrap().exists());
    assert!(store.profile_path("new-name").unwrap().exists());
    assert_eq!(store.load("new-name").unwrap(), profile);
}
```

Existing rename tests (lines 460-529):

- `test_rename_success` — basic rename
- `test_rename_not_found` — source doesn't exist
- `test_rename_same_name` — no-op
- `test_rename_preserves_content` — byte-level content equality
- `test_rename_overwrites_existing_target_profile` — **this test must be updated** to expect `AlreadyExists` error

### Test for New AlreadyExists Error

The existing `test_rename_overwrites_existing_target_profile` (line 515) currently asserts that rename silently overwrites. This test must be changed to:

```rust
#[test]
fn test_rename_rejects_existing_target_profile() {
    let temp_dir = tempdir().unwrap();
    let store = ProfileStore::with_base_path(temp_dir.path().join("profiles"));
    store.save("source", &sample_profile()).unwrap();
    store.save("target", &sample_profile()).unwrap();

    let result = store.rename("source", "target");
    assert!(matches!(result, Err(ProfileStoreError::AlreadyExists(name)) if name == "target"));
    // Both files still exist (rename was blocked)
    assert!(store.profile_path("source").unwrap().exists());
    assert!(store.profile_path("target").unwrap().exists());
}
```

### Tauri Command Tests

`commands/profile.rs` tests (lines 166-319) test helper functions directly, not via the Tauri IPC mechanism. The `commands/settings.rs` tests (lines 38-52) verify command function signatures match the IPC contract. Follow the same approach for any new command tests.

### Test Infrastructure

- **Crate**: `tempfile` (already a dev dependency)
- **Pattern**: `tempdir()` for isolated filesystem
- **Helper**: `sample_profile()` factory returns a fully populated `GameProfile`
- **Assertions**: `matches!()` macro for enum variant matching, `assert_eq!` for data equality

## Patterns to Follow

### Implementation Checklist (derived from patterns)

**Rust Backend:**

1. Add `AlreadyExists(String)` variant to `ProfileStoreError` enum (after `NotFound`)
2. Add `Display` match arm: `"a profile named '{name}' already exists"`
3. Add `new_path.exists()` guard in `ProfileStore::rename()` before `fs::rename()`
4. Add `SettingsStore` state param to `profile_rename` Tauri command
5. Add best-effort `last_used_profile` cascade (follow `profile_delete` pattern)
6. Update `test_rename_overwrites_existing_target_profile` to expect `AlreadyExists`
7. Add test for settings cascade on rename

**TypeScript Frontend:**

1. Add `renaming: boolean` state to `useProfile`
2. Add `renameProfile(oldName: string, newName: string): Promise<void>` callback following `duplicateProfile` pattern
3. Export `renaming` and `renameProfile` from `UseProfileResult` interface
4. Add `canRename`, `renaming`, `onRename` props to `ProfileActionsProps`
5. Add Rename button between Duplicate and Delete in `ProfileActions`
6. Derive `canRename` in `ProfilesPage` (same pattern as `canDuplicate`)
7. Wire `renameProfile` through `ProfilesPage` → `ProfileActions`

### Key Convention: ProfileContext Auto-Extension

Since `ProfileContextValue extends UseProfileResult`, any new fields added to `UseProfileResult` (like `renaming` and `renameProfile`) are automatically available through `useProfileContext()` without modifying `ProfileContext.tsx`. The spread `...profileState` in the `useMemo` handles this.

### Key Convention: Void Return for Rename IPC

`profile_rename` returns `Result<(), String>` — no result data crosses IPC. The frontend handles state refresh via `refreshProfiles()` + `loadProfile(newName)`, same as the `duplicateProfile` post-save pattern but without needing the IPC response to know the new name (since the user provides it).

### Key Convention: Button Disable Guard

All action buttons disable when **any** async operation is in-flight. Adding `renaming` means updating the disable conditions for Save, Duplicate, and Delete buttons too:

```typescript
const canSave = ... && !renaming;
const canDelete = ... && !renaming;
const canDuplicate = ... && !renaming;
```
