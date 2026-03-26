# Pattern Research: duplicate-profile

## Overview

Profile management in CrossHook follows a consistent layered architecture: `ProfileStore` (Rust struct with filesystem-backed TOML persistence) -> Tauri IPC commands (thin wrappers with error mapping) -> React hooks (`useProfile`) -> React context (`ProfileContext`) -> page components. The duplicate feature should compose existing `load`/`save`/`list` primitives following the same pattern, adding a single new method to `ProfileStore`, one Tauri command, and minimal frontend wiring.

## Relevant Files

- `crates/crosshook-core/src/profile/toml_store.rs`: ProfileStore struct with all CRUD operations and `ProfileStoreError` enum
- `crates/crosshook-core/src/profile/models.rs`: `GameProfile` struct (derives Clone, Serialize, Deserialize, Default, PartialEq, Eq)
- `crates/crosshook-core/src/profile/mod.rs`: Module re-exports -- all public types and functions
- `crates/crosshook-core/src/profile/exchange.rs`: Community profile import/export with `sanitize_profile_name()` helper
- `src-tauri/src/commands/profile.rs`: Tauri IPC command handlers for profile operations
- `src-tauri/src/commands/mod.rs`: Command module declarations
- `src-tauri/src/lib.rs`: Tauri command registration in `invoke_handler`
- `src/hooks/useProfile.ts`: Profile CRUD state management hook with `persistProfileDraft`, `hydrateProfile`, `selectProfile`
- `src/context/ProfileContext.tsx`: Wraps `useProfile` into React context; provides `ProfileContextValue`
- `src/components/ProfileActions.tsx`: Save/Delete action buttons (will need Duplicate button)
- `src/components/pages/ProfilesPage.tsx`: Profiles page using `ProfileActions`, `ProfileFormSections`
- `src/types/profile.ts`: TypeScript `GameProfile` interface
- `src/types/launcher.ts`: `LauncherInfo`, `LauncherDeleteResult`, `LauncherRenameResult` types

## Architectural Patterns

- **Store Pattern**: `ProfileStore` is a plain struct holding a `base_path: PathBuf`. It has no interior mutability or locking. Operations are stateless read/write against the filesystem. Instantiated once via `ProfileStore::try_new()` in `lib.rs:15` and managed as Tauri state via `.manage(profile_store)` at `lib.rs:62`.

- **Error Mapping at IPC Boundary**: Every Tauri command maps `ProfileStoreError` to `String` via a local `map_error` function (`commands/profile.rs:8-10`). The pattern is `store.method(...).map_err(map_error)`. New commands must follow this exact pattern.

- **Custom Error Type with From Impls**: `ProfileStoreError` is an enum with `Display` and `Error` impls, plus `From<io::Error>`, `From<toml::de::Error>`, `From<toml::ser::Error>`. This enables `?` propagation within store methods. No `anyhow` is used in the profile module.

- **Name Validation Guard**: Every operation that takes a profile name calls `validate_name()` (toml_store.rs:189-214). The validation rejects empty strings, `.`, `..`, absolute paths, and Windows reserved characters. The `(Copy)` suffix uses parentheses and spaces which are NOT in the reserved set, so auto-generated names are safe.

- **Result Structs for IPC**: Operations that return rich data use dedicated result structs (e.g., `CommunityImportResult`, `CommunityExportResult`, `LauncherDeleteResult`, `LauncherRenameResult`). These derive `Debug, Clone, Serialize, Deserialize`. The duplicate operation should return a `DuplicateProfileResult { name: String, profile: GameProfile }`.

- **Tauri Command Registration**: Commands are declared in `commands/profile.rs` with `#[tauri::command]` and registered in `lib.rs:70-108` via `tauri::generate_handler![]`. New commands must be added to both locations.

- **Frontend Hook Pattern**: `useProfile` manages all profile state. New operations are added as: (1) new `useCallback` function in the hook, (2) added to the return object, (3) typed in `UseProfileResult` interface, (4) consumed via `useProfileContext()` in page components. The `hydrateProfile` callback is the most relevant precedent -- it loads a profile into the editor with a specific name without going through the standard `selectProfile` path.

- **Frontend State After Mutation**: After save/delete, the hook calls `refreshProfiles()` then `loadProfile(name)` to sync state. For duplicate, the pattern should be: invoke backend -> `refreshProfiles()` -> `selectProfile(newName)`.

## Code Conventions

- **Rust naming**: `snake_case` for functions and modules. Tauri commands match frontend `invoke()` call names exactly (e.g., `profile_save` -> `invoke('profile_save', ...)`).
- **TypeScript naming**: `camelCase` for hooks/functions, `PascalCase` for components. Invoke parameter names use `camelCase` which Tauri auto-maps to `snake_case`.
- **Module structure**: Profile module uses `mod.rs` pattern with submodules re-exported. Public API surface controlled via explicit `pub use` statements in `mod.rs`.
- **Test helpers**: Each test file has a `sample_profile()` function that constructs a fully populated `GameProfile`. Tests use `tempfile::tempdir()` for isolated filesystem state.
- **Button styling**: Uses `crosshook-button` and `crosshook-button--secondary` CSS classes. Primary actions are unstyled buttons; secondary uses `--secondary` modifier.

## Error Handling

- **Store layer**: Methods return `Result<T, ProfileStoreError>`. Uses `?` operator for propagation. Each variant in `ProfileStoreError` has a descriptive `Display` impl.
- **Tauri command layer**: Maps errors to `String` via `error.to_string()`. Commands return `Result<T, String>`.
- **Frontend hook layer**: Catches errors in try/catch blocks, calls `setError(message)` to set error state. Error banners render conditionally via `{error ? <div className="crosshook-error-banner ...">...` pattern.
- **No panic/unwrap in store operations**: All fallible operations use `Result`. Only `ProfileStore::new()` (the non-`try` variant) calls `.expect()`, and it's only used as a convenience constructor.

## Testing Approach

- **Test location**: Inline `#[cfg(test)] mod tests` at the bottom of each file. No separate test files.
- **Filesystem isolation**: `tempfile::tempdir()` creates isolated directories. `ProfileStore::with_base_path()` constructor enables pointing at temp dirs.
- **Round-trip pattern**: Tests typically save -> load -> assert equality, or save -> mutate -> save -> load -> assert.
- **Error case testing**: Uses `assert!(result.is_err())` or `assert!(matches!(result, Err(ProfileStoreError::Variant(_))))`.
- **No mocking**: Tests use real filesystem operations against temp directories. No mock frameworks.
- **Sample data**: Each test module defines its own `sample_profile()` function. These are self-contained, not shared across modules.
- **Test naming**: Descriptive snake_case names like `save_load_list_and_delete_round_trip`, `test_rename_preserves_content`.
- **Tauri command tests**: `commands/profile.rs` has its own test module testing private helper functions directly (e.g., `cleanup_launchers_for_profile_delete`, `save_launch_optimizations_for_profile`). Command functions that take `State<'_, T>` are not tested directly; helpers are extracted and tested.

## Patterns to Follow

### Backend Implementation Pattern (in `toml_store.rs`)

1. Add `DuplicateProfileResult` struct with `Serialize`/`Deserialize` derives near the top.
2. Add `duplicate(&self, source_name: &str, new_name: Option<&str>) -> Result<DuplicateProfileResult, ProfileStoreError>` method to `ProfileStore` impl.
3. Compose existing `load()`, `list()`, `save()` methods internally.
4. Add a private `generate_unique_copy_name(&self, source_name: &str) -> Result<String, ProfileStoreError>` helper.
5. Add a free function `strip_copy_suffix(name: &str) -> &str` for removing existing `(Copy)` or `(Copy N)` suffixes.
6. Add comprehensive tests following existing patterns: round-trip, collision resolution, suffix stripping, error cases.

### Tauri Command Pattern (in `commands/profile.rs`)

1. Add `profile_duplicate` function with `#[tauri::command]` attribute.
2. Accept `source_name: String, new_name: Option<String>, store: State<'_, ProfileStore>`.
3. Call `store.duplicate(...)` with `.map_err(map_error)`.
4. Register in `lib.rs` `generate_handler![]` list.

### Frontend Pattern

1. Add `duplicateProfile: (sourceName: string) => Promise<void>` to `UseProfileResult` interface.
2. Implement as `useCallback` in `useProfile.ts` that invokes `profile_duplicate`, then calls `refreshProfiles()` and `selectProfile(result.name)`.
3. Add a "Duplicate" button in `ProfileActions.tsx` alongside Save and Delete.
4. Wire up in `ProfilesPage.tsx` from `useProfileContext()`.

### Key Gotcha: `save()` Overwrites Silently

`ProfileStore::save()` at `toml_store.rs:93-98` does `fs::write(path, ...)` which silently overwrites. The `duplicate()` method MUST check against `list()` before saving to prevent accidental overwrites. This is the critical difference from `save()`/`rename()` behavior and is explicitly called out in the feature spec (line 42).
