# Architecture Research: duplicate-profile

## System Overview

CrossHook uses a three-layer architecture for profile management: a Rust core library (`crosshook-core`) containing all business logic, a thin Tauri v2 IPC command layer (`src-tauri/commands/profile.rs`) that maps commands to core methods, and a React frontend where `useProfile.ts` hook manages all profile state and exposes CRUD operations via `ProfileContext`. Profiles are stored as TOML files in `~/.config/crosshook/profiles/`, with identity determined by filename (not an internal field). The `GameProfile` struct derives `Clone`, `Serialize`, and `Deserialize`, making duplication trivial at the data layer.

## Relevant Components

### Backend (Rust)

- `/src/crosshook-native/crates/crosshook-core/src/profile/mod.rs`: Module root; re-exports all public profile types and functions. Must export `DuplicateProfileResult` once added.
- `/src/crosshook-native/crates/crosshook-core/src/profile/toml_store.rs`: `ProfileStore` struct with `load()`, `save()`, `list()`, `delete()`, `rename()`, `import_legacy()`, and `validate_name()`. This is the primary integration point for `duplicate()`.
- `/src/crosshook-native/crates/crosshook-core/src/profile/models.rs`: `GameProfile` and all section structs. All derive `Clone`, `Serialize`, `Deserialize`, `Default`. Duplication uses `.clone()` with no field-level logic.
- `/src/crosshook-native/crates/crosshook-core/src/profile/exchange.rs`: Community profile import/export. Contains `sanitize_profile_name()` and `derive_import_name()` -- name generation patterns relevant to duplicate naming.
- `/src/crosshook-native/crates/crosshook-core/src/lib.rs`: Crate module root. No changes needed (profile module already exported).
- `/src/crosshook-native/src-tauri/src/commands/profile.rs`: Tauri IPC commands for profile operations. All commands take `State<'_, ProfileStore>` and return `Result<T, String>` via `map_error()`. New `profile_duplicate` command goes here.
- `/src/crosshook-native/src-tauri/src/lib.rs`: Tauri app setup; registers `ProfileStore` as managed state (line 62) and registers all commands in `invoke_handler` (lines 70-108). Must register `profile_duplicate`.

### Frontend (React/TypeScript)

- `/src/crosshook-native/src/hooks/useProfile.ts`: Central hook managing all profile state (load, save, delete, rename, refresh). Returns `UseProfileResult` interface. New `duplicateProfile` callback goes here.
- `/src/crosshook-native/src/context/ProfileContext.tsx`: Context provider wrapping `useProfile` with derived values (launch method, Steam paths). Extends `UseProfileResult` with `ProfileContextValue`. The new `duplicateProfile` function automatically propagates through context.
- `/src/crosshook-native/src/types/profile.ts`: TypeScript type definitions for `GameProfile` and sections. New `DuplicateProfileResult` interface goes here.
- `/src/crosshook-native/src/components/ProfileActions.tsx`: Action buttons (Save, Delete) with disabled states and loading indicators. New "Duplicate" button goes here.
- `/src/crosshook-native/src/components/pages/ProfilesPage.tsx`: Profile editor page consuming `useProfileContext()`. Wires profile state to `ProfileActions` and `ProfileFormSections`. Must pass `duplicateProfile` and enable conditions to `ProfileActions`.

## Data Flow

### Profile CRUD (existing pattern that duplicate follows)

```
User action (button click)
  -> ProfilesPage.tsx (event handler)
    -> useProfileContext() -> useProfile.ts (callback)
      -> invoke('profile_*', { ... })  [Tauri IPC]
        -> commands/profile.rs  [thin command handler]
          -> ProfileStore::method()  [crosshook-core business logic]
            -> TOML file read/write on disk
          -> returns Result<T, String>
        <- deserialized result
      -> updates React state (setProfile, setProfiles, etc.)
    <- UI re-renders
```

### Duplicate-specific flow

```
User clicks "Duplicate" button
  -> ProfilesPage.tsx: void duplicateProfile(profileName)
    -> useProfile.ts::duplicateProfile(sourceName)
      -> invoke<DuplicateProfileResult>('profile_duplicate', { sourceName })
        -> commands/profile.rs::profile_duplicate(source_name, store)
          -> ProfileStore::duplicate(source_name, None)
            -> load(source_name)       -- reads source TOML
            -> list()                  -- reads directory for conflict check
            -> generate_unique_copy_name(source_name) -- strips suffix, loops
            -> save(new_name, &profile) -- writes new TOML
          -> returns DuplicateProfileResult { name, profile }
      -> refreshProfiles()             -- re-fetches profile list
      -> loadProfile(result.name)      -- selects the new duplicate
    <- UI shows the new duplicate selected in the editor
```

### State management flow

The `useProfile` hook maintains these relevant state atoms:

- `profiles: string[]` -- profile name list from `ProfileStore::list()`
- `selectedProfile: string` -- currently loaded profile name
- `profileName: string` -- name in the editor input field
- `profile: GameProfile` -- current profile data
- `dirty: boolean` -- whether unsaved changes exist
- `saving: boolean` -- used for button disabled states and loading text

After duplicate completes, the hook calls `refreshProfiles()` (re-fetches list) then `loadProfile(result.name)` (loads the new profile, clears dirty, syncs metadata).

## Integration Points

### Where duplicate() logic lives

`ProfileStore::duplicate()` in `crates/crosshook-core/src/profile/toml_store.rs`. This follows the crate separation pattern ("crosshook-core contains all business logic"). The method composes existing primitives:

- `self.load()` to read the source profile
- `self.list()` to check for name collisions
- `validate_name()` for the generated name
- `self.save()` to write the new TOML file

### Tauri command registration

In `src-tauri/src/lib.rs`, the `profile_duplicate` command must be added to the `tauri::generate_handler![]` macro invocation at lines 91-97 (the profile command block).

### Frontend hook integration

`useProfile.ts` already exposes the pattern for async profile operations (see `saveProfile`, `confirmDelete`/`executeDelete`). The `duplicateProfile` callback follows the same pattern:

1. Set `saving` state
2. Invoke Tauri command
3. Refresh profile list
4. Load the new profile
5. Clear error/saving state

### ProfileActions component

Currently has `onSave` and `onDelete` props. Needs `onDuplicate` and `canDuplicate` props. The "Duplicate" button sits between Save and Delete (constructive actions grouped left).

### ProfileContext propagation

Since `ProfileContext` extends `UseProfileResult` via spread (`{ ...profileState }`), adding `duplicateProfile` to the hook's return type automatically makes it available to all consumers via `useProfileContext()`.

## Key Dependencies

### Internal

| Dependency                             | Used By                | Role                                                |
| -------------------------------------- | ---------------------- | --------------------------------------------------- |
| `ProfileStore` (crosshook-core)        | Tauri commands, CLI    | All profile CRUD operations                         |
| `validate_name()` (toml_store.rs)      | `ProfileStore` methods | Profile name validation (path traversal prevention) |
| `toml` + `serde` crates                | `ProfileStore`         | TOML serialization/deserialization                  |
| `GameProfile` derives `Clone`          | `duplicate()`          | Enables `.clone()` for full deep copy               |
| `useProfile` hook                      | `ProfileContext`       | All frontend profile state management               |
| `invoke()` from `@tauri-apps/api/core` | All frontend hooks     | Tauri IPC bridge                                    |

### External (no new dependencies)

| Library                  | Version        | Already Used                     |
| ------------------------ | -------------- | -------------------------------- |
| `toml`                   | workspace      | Yes (profile serialization)      |
| `serde` / `serde_derive` | workspace      | Yes (IPC boundary types)         |
| `directories`            | workspace      | Yes (`BaseDirs` for config path) |
| `tempfile`               | dev-dependency | Yes (existing tests)             |

### Filesystem

- Profile storage: `~/.config/crosshook/profiles/*.toml`
- Each profile is one TOML file named `{profile_name}.toml`
- `ProfileStore::list()` reads the directory to enumerate profiles
- `ProfileStore::save()` creates the directory if needed and writes atomically (overwrite semantics)

## Architectural Patterns Observed

- **Thin command layer**: Tauri commands are 1-3 line delegations to `ProfileStore` methods with `map_error()`. The `profile_duplicate` command should follow this pattern.
- **State-as-managed-resource**: `ProfileStore` is registered via `tauri::Builder::manage()` and injected into commands via `State<'_>`. No global/static state.
- **Hook-as-state-machine**: `useProfile` is a substantial hook (~700 lines) managing profile lifecycle. It owns all state transitions and exposes a clean interface via `UseProfileResult`.
- **No existence guard on save**: `ProfileStore::save()` silently overwrites. The `duplicate()` method must explicitly check `list()` before saving auto-generated names. This is the primary safety concern.
- **Error as String**: All Tauri commands return `Result<T, String>`. Frontend error handling is uniform via `setError(msg)` and the `crosshook-error-banner` CSS class.
- **Auto-select after mutation**: Both delete (`finalizeProfileDeletion`) and save (`persistProfileDraft`) refresh the profile list and select a profile afterward. Duplicate should follow this same pattern.
