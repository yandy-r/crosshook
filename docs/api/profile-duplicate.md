# Profile Duplication API Reference

**Tauri IPC Command:** `profile_duplicate`
**Issue:** #56
**Added in:** `feat/duplicate-profile` branch

---

## Overview

Duplicates an existing game profile by creating a deep copy under an automatically generated unique name. The duplicated profile is persisted to disk as a new TOML file alongside the source profile.

This command follows the same Tauri IPC pattern as all other profile commands: it accepts a managed `ProfileStore` state, delegates to `crosshook-core`, and returns `Result<T, String>` with errors mapped via `map_error()`.

---

## Command Signature

### Rust (Tauri Command)

```rust
// src-tauri/src/commands/profile.rs

#[tauri::command]
pub fn profile_duplicate(
    name: String,
    store: State<'_, ProfileStore>,
) -> Result<DuplicateProfileResult, String>
```

### TypeScript Invocation

```typescript
import { invoke } from '@tauri-apps/api/core';
import type { DuplicateProfileResult } from '../types';

const result = await invoke<DuplicateProfileResult>('profile_duplicate', {
  name: sourceName,
});
```

---

## Parameters

| Parameter | Type     | Required | Description                                   |
|-----------|----------|----------|-----------------------------------------------|
| `name`    | `String` | Yes      | The name of the existing profile to duplicate. |

The `name` parameter refers to the profile's logical name (the TOML filename stem), not a filesystem path.

### Name Validation Rules

The source name is validated by `validate_name()` before any work begins. A name is **rejected** if it:

- Is empty or whitespace-only after trimming
- Equals `.` or `..`
- Contains any of: `< > : " / \ | ? *`
- Represents an absolute path

---

## Response

### Success

Returns a `DuplicateProfileResult` containing the generated name and the full profile data of the new copy.

**Rust type:**

```rust
// crates/crosshook-core/src/profile/toml_store.rs

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DuplicateProfileResult {
    pub name: String,
    pub profile: GameProfile,
}
```

**TypeScript type:**

```typescript
// src/types/profile.ts

export interface DuplicateProfileResult {
  name: string;
  profile: GameProfile;
}
```

**Example response (JSON over IPC):**

```json
{
  "name": "Elden Ring (Copy)",
  "profile": {
    "game": {
      "name": "Elden Ring",
      "executable_path": "/games/elden-ring/eldenring.exe"
    },
    "trainer": {
      "path": "/trainers/elden-ring.exe",
      "type": "fling",
      "loading_mode": "source_directory"
    },
    "injection": {
      "dll_paths": ["/dlls/a.dll", "/dlls/b.dll"],
      "inject_on_launch": [true, false]
    },
    "steam": {
      "enabled": true,
      "app_id": "1245620",
      "compatdata_path": "/steam/compatdata/1245620",
      "proton_path": "/steam/proton/proton",
      "launcher": {
        "icon_path": "/icons/elden-ring.png",
        "display_name": "Elden Ring"
      }
    },
    "runtime": {
      "prefix_path": "",
      "proton_path": "",
      "working_directory": ""
    },
    "launch": {
      "method": "steam_applaunch",
      "optimizations": {
        "enabled_option_ids": []
      }
    }
  }
}
```

### Error

All errors are returned as `Err(String)` over the IPC boundary (the `ProfileStoreError` variants are converted via their `Display` implementation).

| Error Condition                              | Error Message Pattern                                                    |
|----------------------------------------------|--------------------------------------------------------------------------|
| Source name fails validation                 | `invalid profile name: <name>`                                           |
| Source profile TOML file does not exist       | `profile file not found: <path>`                                         |
| Source profile TOML cannot be deserialized    | TOML parse error details from `toml::de::Error`                          |
| Generated copy name fails validation         | `invalid profile name: <generated_name>`                                 |
| Name generation exhausted (1000 copies exist) | `invalid profile name: cannot generate unique copy name for '<name>'`   |
| Filesystem I/O failure (read or write)       | OS-level I/O error message                                               |
| TOML serialization failure                   | TOML serialization error details from `toml::ser::Error`                 |

---

## Name Generation Behavior

The command generates a unique name for the copy using the following algorithm:

1. **Strip existing copy suffix.** If the source name already ends with `(Copy)` or `(Copy N)` where N is a positive integer, that suffix is removed to find the base name. Examples:
   - `"Elden Ring"` -> base `"Elden Ring"`
   - `"Elden Ring (Copy)"` -> base `"Elden Ring"`
   - `"Elden Ring (Copy 3)"` -> base `"Elden Ring"`
   - `"Game (Special Edition)"` -> base `"Game (Special Edition)"` (not a copy suffix)

2. **Edge case: name is entirely a copy suffix.** If stripping the suffix would produce an empty string (e.g., source name is `"(Copy)"`), the original name is used as the base instead.

3. **Try `<base> (Copy)` first.** If no profile with that name exists, use it.

4. **Increment: `<base> (Copy 2)`, `(Copy 3)`, ... up to `(Copy 1000)`.** The first unused name wins.

5. **Exhaustion.** If all 1000 candidates are taken, the command returns an error.

### Name Generation Examples

| Source Name          | Existing Profiles                                   | Generated Name         |
|----------------------|-----------------------------------------------------|------------------------|
| `MyGame`             | `MyGame`                                            | `MyGame (Copy)`        |
| `MyGame`             | `MyGame`, `MyGame (Copy)`                           | `MyGame (Copy 2)`      |
| `MyGame (Copy)`      | `MyGame`, `MyGame (Copy)`                           | `MyGame (Copy 2)`      |
| `MyGame (Copy 2)`    | `MyGame`, `MyGame (Copy)`, `MyGame (Copy 2)`        | `MyGame (Copy 3)`      |
| `(Copy)`             | `(Copy)`                                            | `(Copy) (Copy)`        |

---

## Side Effects

- **Creates a new TOML file** at `~/.config/crosshook/profiles/<generated_name>.toml` containing a serialized copy of the source profile.
- The source profile file is **not modified**.
- No launcher export files (`.sh` scripts or `.desktop` entries) are created for the duplicate -- the copy is profile-only.

---

## Frontend Integration

The `useProfile` hook in `src/hooks/useProfile.ts` exposes the duplication workflow through:

```typescript
duplicateProfile: (sourceName: string) => Promise<void>;
duplicating: boolean;
```

### Hook Behavior

1. Sets `duplicating` to `true` and clears any previous error.
2. Invokes `profile_duplicate` with the source name.
3. On success: refreshes the profile list, then loads the newly created copy as the active profile.
4. On failure: sets the `error` state with the error message.
5. Sets `duplicating` back to `false`.

The hook does **not** return the `DuplicateProfileResult` to callers -- it uses `result.name` internally to auto-select the new profile after duplication.

---

## Related Commands

| Command                            | Description                                        |
|------------------------------------|----------------------------------------------------|
| `profile_list`                     | List all profile names (sorted alphabetically)     |
| `profile_load`                     | Load a single profile by name                      |
| `profile_save`                     | Create or overwrite a profile                      |
| `profile_delete`                   | Delete a profile (with best-effort launcher cleanup) |
| `profile_rename`                   | Rename a profile (filesystem move)                 |
| `profile_import_legacy`            | Import a legacy `.profile` file                    |
| `profile_save_launch_optimizations`| Partial update of the launch optimizations section |

All profile commands are registered in `src-tauri/src/lib.rs` via `tauri::generate_handler![]` and share the same managed `ProfileStore` state instance.

---

## Registration

The command is registered in the Tauri invoke handler alongside all other profile commands:

```rust
// src-tauri/src/lib.rs

.invoke_handler(tauri::generate_handler![
    // ...
    commands::profile::profile_duplicate,
    // ...
])
```

The `ProfileStore` is initialized at app startup via `ProfileStore::try_new()` and managed as Tauri state:

```rust
.manage(profile_store)
```

Profiles are stored at `~/.config/crosshook/profiles/` with one `<name>.toml` file per profile.
