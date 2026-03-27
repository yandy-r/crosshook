# Profile Duplication -- API Documentation

Issue: #56

This document describes the code-level API for the profile duplication feature across all three layers of the CrossHook stack: Rust core library, Tauri IPC commands, and React/TypeScript frontend.

---

## 1. Rust Core API (`crosshook-core`)

**File:** `crates/crosshook-core/src/profile/toml_store.rs`
**Re-exported from:** `crates/crosshook-core/src/profile/mod.rs` as `pub use toml_store::DuplicateProfileResult`

### `DuplicateProfileResult`

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DuplicateProfileResult {
    pub name: String,
    pub profile: GameProfile,
}
```

| Field     | Type          | Description                                                       |
|-----------|---------------|-------------------------------------------------------------------|
| `name`    | `String`      | Generated unique name for the copy (e.g. `"MyGame (Copy 2)"`).   |
| `profile` | `GameProfile` | Byte-for-byte clone of the source profile's data.                 |

Derives `Serialize`/`Deserialize` so it crosses the Tauri IPC boundary as JSON.

### `ProfileStore::duplicate`

```rust
pub fn duplicate(
    &self,
    source_name: &str,
) -> Result<DuplicateProfileResult, ProfileStoreError>
```

**Parameters:**

| Name          | Type   | Description                                       |
|---------------|--------|---------------------------------------------------|
| `source_name` | `&str` | Name of the existing profile to duplicate.        |

**Returns:** `Ok(DuplicateProfileResult)` containing the generated name and cloned profile data.

**Errors:**

| Variant                              | Condition                                            |
|--------------------------------------|------------------------------------------------------|
| `ProfileStoreError::InvalidName`     | `source_name` fails filesystem validation.           |
| `ProfileStoreError::NotFound`        | No `.toml` file exists for `source_name`.            |
| `ProfileStoreError::Io`             | Filesystem read/write failure.                        |
| `ProfileStoreError::TomlSer`        | TOML serialization failure.                           |
| `ProfileStoreError::InvalidName`     | All 1000 candidate copy names are exhausted.         |

**Safety constraints:**

- The generated copy name is always passed through `validate_name()` before being written, preventing path traversal or reserved characters.
- `save()` writes unconditionally to the computed path, but `generate_unique_copy_name` guarantees the name is not already present in the store's `list()`, so accidental overwrites cannot occur under normal single-threaded operation.
- The source profile is never modified -- `load()` returns a fresh deserialized copy.

### `ProfileStore::generate_unique_copy_name` (private)

```rust
fn generate_unique_copy_name(
    source_name: &str,
    existing_names: &[String],
) -> Result<String, ProfileStoreError>
```

**Algorithm:**

1. Strip any existing `(Copy)` or `(Copy N)` suffix from `source_name` via `strip_copy_suffix()` to recover the original base name.
2. If stripping produces an empty string (source is literally `"(Copy)"`), use the full source name as the base to prevent empty-name output.
3. Try `"{base} (Copy)"`.
4. If that collides with `existing_names`, iterate `"{base} (Copy 2)"` through `"{base} (Copy 1000)"`.
5. If all 1000 candidates collide, return `Err(ProfileStoreError::InvalidName(...))`.

This means duplicating `"MyGame (Copy)"` produces `"MyGame (Copy 2)"` rather than `"MyGame (Copy) (Copy)"`, keeping names clean across repeated duplications.

### `strip_copy_suffix` (free function, private)

```rust
fn strip_copy_suffix(name: &str) -> &str
```

Strips a trailing `(Copy)` or `(Copy N)` suffix, where `N` is a valid `u32`. Non-copy parenthesized suffixes (e.g. `"Game (Special Edition)"`) are left intact.

**Examples:**

| Input                     | Output                    |
|---------------------------|---------------------------|
| `"Name (Copy)"`           | `"Name"`                  |
| `"Name (Copy 3)"`         | `"Name"`                  |
| `"Name"`                  | `"Name"`                  |
| `"Game (Special Edition)"`| `"Game (Special Edition)"`|
| `"(Copy)"`                | `""`                      |

Returns the full trimmed input when no copy suffix is detected.

---

## 2. Tauri IPC Command

**File:** `src-tauri/src/commands/profile.rs`
**Registered in:** `src-tauri/src/lib.rs` via `commands::profile::profile_duplicate`

### `profile_duplicate`

```rust
#[tauri::command]
pub fn profile_duplicate(
    name: String,
    store: State<'_, ProfileStore>,
) -> Result<DuplicateProfileResult, String>
```

**Parameters:**

| Name    | Type                     | Description                                      |
|---------|--------------------------|--------------------------------------------------|
| `name`  | `String`                 | Name of the source profile to duplicate.         |
| `store` | `State<'_, ProfileStore>`| Tauri-managed singleton injected automatically.  |

**Returns:** `Result<DuplicateProfileResult, String>` -- the `Err` variant is the stringified `ProfileStoreError`.

This is a thin delegation to `ProfileStore::duplicate()` with `map_err(map_error)`. No launcher cleanup or settings side-effects occur during duplication (unlike `profile_delete`).

**Frontend invocation:**

```typescript
const result = await invoke<DuplicateProfileResult>('profile_duplicate', { name });
```

---

## 3. TypeScript/React API

### `DuplicateProfileResult` interface

**File:** `src/types/profile.ts`

```typescript
export interface DuplicateProfileResult {
  name: string;       // Generated unique name
  profile: GameProfile; // Full clone of source profile data
}
```

Mirrors the Rust `DuplicateProfileResult` struct. Field names use `snake_case` to match serde JSON serialization from the backend.

### `useProfile` hook additions

**File:** `src/hooks/useProfile.ts`

#### `UseProfileResult` new members

| Member             | Type                                      | Description                                                                                       |
|--------------------|-------------------------------------------|---------------------------------------------------------------------------------------------------|
| `duplicateProfile` | `(sourceName: string) => Promise<void>`   | Calls `profile_duplicate` IPC, refreshes profile list, and auto-selects the new copy.             |
| `duplicating`      | `boolean`                                 | True while the duplication IPC call is in-flight. Drives button disabled state and spinner text.   |

#### `duplicateProfile` callback behavior

1. Guards against empty `sourceName`.
2. Sets `duplicating = true` and clears any prior error.
3. Invokes `profile_duplicate` via Tauri IPC.
4. On success: calls `refreshProfiles()` to re-fetch the full name list, then `loadProfile(result.name)` to select and display the duplicate.
5. On failure: sets `error` with the backend message.
6. Always resets `duplicating = false` in the `finally` block.

### `ProfileActions` component

**File:** `src/components/ProfileActions.tsx`

#### `ProfileActionsProps` duplication-related props

| Prop            | Type                               | Description                                                                     |
|-----------------|-------------------------------------|---------------------------------------------------------------------------------|
| `canDuplicate`  | `boolean`                          | True when a saved profile is selected and eligible for duplication.              |
| `duplicating`   | `boolean`                          | True while the backend IPC call is in-flight. Disables the Duplicate button.    |
| `onDuplicate`   | `() => void \| Promise<void>`     | Callback to initiate duplication. Wired to `duplicateProfile(selectedProfile)`. |

The Duplicate button renders between Save and Delete, with the `crosshook-button--secondary` class. Its label toggles between `"Duplicate"` and `"Duplicating..."` based on the `duplicating` prop.

---

## 4. Cross-Layer Contract

The duplication feature has a single IPC boundary with a shared data shape:

```
Rust (DuplicateProfileResult)  --serde JSON-->  TypeScript (DuplicateProfileResult)
   name: String                                     name: string
   profile: GameProfile                              profile: GameProfile
```

- Rust field names are `snake_case` and serde serializes them as-is.
- TypeScript interface field names match the `snake_case` JSON keys exactly.
- Both `GameProfile` types are already established and used across the full profile CRUD surface -- duplication reuses them without modification.
- The Tauri command name `profile_duplicate` is registered in `lib.rs` via `tauri::generate_handler![]` and invoked from TypeScript as `invoke('profile_duplicate', { name })`.

---

## 5. Test Coverage

**File:** `crates/crosshook-core/src/profile/toml_store.rs` (within `#[cfg(test)] mod tests`)

All tests use `tempfile::tempdir()` for isolated filesystem state.

| Test Name                                           | What It Verifies                                                                                              |
|-----------------------------------------------------|---------------------------------------------------------------------------------------------------------------|
| `test_strip_copy_suffix`                            | Suffix stripping for `(Copy)`, `(Copy N)`, non-copy parenthesized names, and edge cases.                     |
| `test_duplicate_basic`                              | Duplicating `"MyGame"` produces `"MyGame (Copy)"` with identical profile data and a persisted TOML file.      |
| `test_duplicate_increments_on_conflict`             | When `"MyGame (Copy)"` already exists, duplication produces `"MyGame (Copy 2)"`.                              |
| `test_duplicate_of_copy`                            | Duplicating `"MyGame (Copy)"` strips the suffix first, yielding `"MyGame (Copy 2)"` instead of nested suffixes.|
| `test_duplicate_copy_suffix_only_name_keeps_non_empty_base` | Duplicating a profile literally named `"(Copy)"` produces `"(Copy) (Copy)"` with a non-empty, loadable name. |
| `test_duplicate_preserves_all_fields`               | Round-trip verification: every field in the loaded source equals the loaded copy.                              |
| `test_duplicate_source_not_found`                   | Duplicating a nonexistent profile returns `ProfileStoreError::NotFound`.                                      |

**Coverage summary:** The test suite covers the happy path, name collision incrementing, recursive copy-of-copy naming, degenerate edge cases (suffix-only names), full-field preservation, and the primary error path (missing source).
