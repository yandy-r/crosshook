# Technical Specifications: Profile Duplicate/Clone

## Executive Summary

Profile duplication requires a new `duplicate` method on `ProfileStore` (Rust), a new `profile_duplicate` Tauri command, and a frontend "Duplicate" button integrated into the existing `ProfileActions` component. The implementation composes existing `load`/`save`/`list` operations with a new unique-name generation algorithm. All profile fields are copied verbatim -- the only mutation is the profile name (file name on disk). No new error variants are needed; existing `ProfileStoreError` covers all failure modes.

## Architecture Design

### Component Diagram

```
ProfilesPage.tsx
  -> ProfileActions.tsx          (new "Duplicate" button)
  -> useProfileContext()         (provides duplicateProfile callback)
       -> ProfileContext.tsx      (wraps useProfile)
            -> useProfile.ts     (new duplicateProfile function)
                 -> invoke('profile_duplicate', { sourceName, newName? })
                      -> src-tauri/src/commands/profile.rs::profile_duplicate()
                           -> ProfileStore::duplicate()
                                -> ProfileStore::load()
                                -> ProfileStore::list()     (for name conflict check)
                                -> ProfileStore::save()
                           -> returns GameProfile
```

### Data Flow

1. User clicks "Duplicate" in ProfileActions
2. React calls `invoke('profile_duplicate', { sourceName: selectedProfile })`
3. Tauri command `profile_duplicate` receives `source_name: String` and optional `new_name: Option<String>`
4. Command delegates to `ProfileStore::duplicate(source_name, new_name)`
5. `duplicate()` loads the source profile, generates a unique name (or validates provided name), saves with the new name
6. Returns `GameProfile` to frontend
7. Frontend calls `refreshProfiles()` then `selectProfile(newName)` to load the duplicate

## Data Models

### GameProfile Struct (field-by-field copy decisions)

All fields are copied verbatim. The profile name is the TOML filename, not a field inside the struct.

| Section                       | Field    | Copy Strategy                    | Rationale |
| ----------------------------- | -------- | -------------------------------- | --------- |
| `game.name`                   | Verbatim | Same game, user can rename later |
| `game.executable_path`        | Verbatim | Points to same game binary       |
| `trainer.path`                | Verbatim | Same trainer                     |
| `trainer.kind`                | Verbatim | Same trainer type                |
| `trainer.loading_mode`        | Verbatim | User preference                  |
| `injection.dll_paths`         | Verbatim | Same DLLs                        |
| `injection.inject_on_launch`  | Verbatim | Same injection config            |
| `steam.enabled`               | Verbatim | Same launch method               |
| `steam.app_id`                | Verbatim | Same Steam app                   |
| `steam.compatdata_path`       | Verbatim | Same prefix                      |
| `steam.proton_path`           | Verbatim | Same Proton version              |
| `steam.launcher.icon_path`    | Verbatim | Same icon                        |
| `steam.launcher.display_name` | Verbatim | Same display name                |
| `runtime.prefix_path`         | Verbatim | Same runtime config              |
| `runtime.proton_path`         | Verbatim | Same runtime config              |
| `runtime.working_directory`   | Verbatim | Same working directory           |
| `launch.method`               | Verbatim | Same launch method               |
| `launch.optimizations`        | Verbatim | Same optimizations               |

Key insight: `GameProfile` derives `Clone`, so the entire struct can be cloned with `.clone()`. No field-by-field copying is needed. The only "modification" is the file name on disk (the profile name argument to `save()`).

### Existing ProfileStore Error Variants (no new variants needed)

```rust
pub enum ProfileStoreError {
    InvalidName(String),       // covers invalid new_name
    NotFound(PathBuf),         // covers source profile not found
    InvalidLaunchOptimizationId(String),
    Io(std::io::Error),        // covers filesystem errors
    TomlDe(toml::de::Error),   // covers deserialization errors
    TomlSer(toml::ser::Error), // covers serialization errors
}
```

A new `AlreadyExists` variant is NOT recommended because:

- The `duplicate()` method with auto-generated names will never hit a conflict (it loops until unique)
- If a caller provides an explicit `new_name` that already exists, overwriting matches the existing `rename()` and `save()` behavior (which silently overwrite)
- Alternatively, if overwrite-prevention is desired for explicit names, a simple `profile_path.exists()` check before save can return an `Io` error with appropriate messaging

## API Design

### Tauri Command Signature

```rust
#[tauri::command]
pub fn profile_duplicate(
    source_name: String,
    new_name: Option<String>,
    store: State<'_, ProfileStore>,
) -> Result<DuplicateProfileResult, String> {
    store.duplicate(&source_name, new_name.as_deref()).map_err(map_error)
}
```

### Request/Response Contract

**Invoke call (TypeScript):**

```typescript
const result = await invoke<DuplicateProfileResult>('profile_duplicate', {
  sourceName: 'elden-ring',
  // newName is optional; omit for auto-generated name
});
// result: { name: "elden-ring (Copy)", profile: GameProfile }
```

**Response type:**

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DuplicateProfileResult {
    pub name: String,
    pub profile: GameProfile,
}
```

The response includes `name` because the caller needs to know the auto-generated name to select it in the UI.

**TypeScript type:**

```typescript
export interface DuplicateProfileResult {
  name: string;
  profile: GameProfile;
}
```

### Error Cases

| Scenario                      | Error Message Pattern                                     |
| ----------------------------- | --------------------------------------------------------- |
| Source profile does not exist | `"profile file not found: /path/to/profiles/source.toml"` |
| Invalid source name           | `"invalid profile name: ..."`                             |
| Invalid new name (explicit)   | `"invalid profile name: ..."`                             |
| Filesystem write error        | `"Permission denied"` or similar IO error                 |

## Core Library Design

### ProfileStore::duplicate Method

Location: `crates/crosshook-core/src/profile/toml_store.rs`

```rust
pub fn duplicate(
    &self,
    source_name: &str,
    new_name: Option<&str>,
) -> Result<DuplicateProfileResult, ProfileStoreError> {
    let profile = self.load(source_name)?;

    let resolved_name = match new_name {
        Some(name) => {
            let trimmed = name.trim();
            validate_name(trimmed)?;
            trimmed.to_string()
        }
        None => self.generate_unique_copy_name(source_name)?,
    };

    self.save(&resolved_name, &profile)?;

    Ok(DuplicateProfileResult {
        name: resolved_name,
        profile,
    })
}
```

### DuplicateProfileResult Struct

Location: `crates/crosshook-core/src/profile/toml_store.rs` (alongside ProfileStore)

```rust
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DuplicateProfileResult {
    pub name: String,
    pub profile: GameProfile,
}
```

Export from `crates/crosshook-core/src/profile/mod.rs`:

```rust
pub use toml_store::{DuplicateProfileResult, ProfileStore, ProfileStoreError};
```

### Name Generation Algorithm

Location: private method on `ProfileStore`

```rust
fn generate_unique_copy_name(
    &self,
    source_name: &str,
) -> Result<String, ProfileStoreError> {
    let existing = self.list()?;
    let base = strip_copy_suffix(source_name);

    let candidate = format!("{base} (Copy)");
    if !existing.contains(&candidate) {
        return Ok(candidate);
    }

    for n in 2u32.. {
        let candidate = format!("{base} (Copy {n})");
        if !existing.contains(&candidate) {
            return Ok(candidate);
        }
    }

    // Unreachable in practice: u32 gives ~4 billion candidates
    unreachable!("exhausted copy name candidates")
}
```

**Strip existing copy suffix** (prevents "Profile (Copy) (Copy)"):

```rust
fn strip_copy_suffix(name: &str) -> &str {
    let trimmed = name.trim();

    // Match " (Copy N)" where N >= 2
    if let Some(prefix) = trimmed.strip_suffix(')') {
        if let Some(rest) = prefix.strip_suffix(" (Copy") {
            // Handles exact " (Copy)" suffix
            if rest.ends_with(|_: char| true) {
                // Check for " (Copy)" or " (Copy N)"
            }
        }
    }

    // Simpler approach using regex-free parsing:
    if let Some(base) = trimmed.strip_suffix(" (Copy)") {
        return base;
    }

    // Match " (Copy N)" where N is a number
    if let Some(paren_start) = trimmed.rfind(" (Copy ") {
        let after_copy = &trimmed[paren_start + 7..]; // skip " (Copy "
        if let Some(digits) = after_copy.strip_suffix(')') {
            if digits.chars().all(|c| c.is_ascii_digit()) && !digits.is_empty() {
                return &trimmed[..paren_start];
            }
        }
    }

    trimmed
}
```

**Name generation sequence:**

1. Source: "Elden Ring" -> "Elden Ring (Copy)", "Elden Ring (Copy 2)", "Elden Ring (Copy 3)", ...
2. Source: "Elden Ring (Copy)" -> "Elden Ring (Copy 2)" (strips existing suffix first)
3. Source: "Elden Ring (Copy 3)" -> "Elden Ring (Copy 4)" or first available

### Test Cases for ProfileStore::duplicate

```rust
#[test]
fn duplicate_creates_copy_with_generated_name() {
    let store = ProfileStore::with_base_path(tempdir().path().join("profiles"));
    store.save("elden-ring", &sample_profile()).unwrap();

    let result = store.duplicate("elden-ring", None).unwrap();
    assert_eq!(result.name, "elden-ring (Copy)");
    assert_eq!(result.profile, sample_profile());
    assert!(store.load("elden-ring (Copy)").is_ok());
    assert!(store.load("elden-ring").is_ok()); // original untouched
}

#[test]
fn duplicate_increments_copy_suffix_on_conflict() {
    let store = ProfileStore::with_base_path(tempdir().path().join("profiles"));
    store.save("elden-ring", &sample_profile()).unwrap();
    store.save("elden-ring (Copy)", &sample_profile()).unwrap();

    let result = store.duplicate("elden-ring", None).unwrap();
    assert_eq!(result.name, "elden-ring (Copy 2)");
}

#[test]
fn duplicate_with_explicit_name() {
    let store = ProfileStore::with_base_path(tempdir().path().join("profiles"));
    store.save("elden-ring", &sample_profile()).unwrap();

    let result = store.duplicate("elden-ring", Some("my-custom-copy")).unwrap();
    assert_eq!(result.name, "my-custom-copy");
}

#[test]
fn duplicate_source_not_found() {
    let store = ProfileStore::with_base_path(tempdir().path().join("profiles"));
    assert!(store.duplicate("nonexistent", None).is_err());
}

#[test]
fn duplicate_strips_existing_copy_suffix() {
    let store = ProfileStore::with_base_path(tempdir().path().join("profiles"));
    store.save("elden-ring (Copy)", &sample_profile()).unwrap();

    let result = store.duplicate("elden-ring (Copy)", None).unwrap();
    assert_eq!(result.name, "elden-ring (Copy 2)");
}
```

## Frontend Integration

### useProfile Hook Changes

Add a `duplicateProfile` function to `UseProfileResult`:

```typescript
export interface UseProfileResult {
  // ... existing fields ...
  duplicateProfile: (sourceName: string) => Promise<void>;
}
```

Implementation inside `useProfile()`:

```typescript
const duplicateProfile = useCallback(
  async (sourceName: string) => {
    const trimmed = sourceName.trim();
    if (!trimmed) {
      setError('Select a profile to duplicate.');
      return;
    }

    setSaving(true);
    setError(null);

    try {
      const result = await invoke<DuplicateProfileResult>('profile_duplicate', {
        sourceName: trimmed,
      });
      await refreshProfiles();
      await loadProfile(result.name);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setSaving(false);
    }
  },
  [loadProfile, refreshProfiles]
);
```

### ProfileContext Changes

`ProfileContextValue` already extends `UseProfileResult`, so `duplicateProfile` is automatically available to all consumers via `useProfileContext()` -- no changes needed in `ProfileContext.tsx`.

### ProfileActions Component Changes

Add `onDuplicate` prop and a "Duplicate" button:

```typescript
export interface ProfileActionsProps {
  // ... existing props ...
  canDuplicate: boolean;
  onDuplicate: () => void | Promise<void>;
}
```

Add button between Save and Delete:

```tsx
<button
  type="button"
  className="crosshook-button crosshook-button--secondary"
  onClick={() => void onDuplicate()}
  disabled={!canDuplicate}
>
  Duplicate
</button>
```

### ProfilesPage Integration

Pass the new props:

```tsx
const canDuplicate = profileExists && !saving && !deleting && !loading;

<ProfileActions
  // ... existing props ...
  canDuplicate={canDuplicate}
  onDuplicate={() => duplicateProfile(selectedProfile)}
/>;
```

### TypeScript Type Addition

Add to `src/types/profile.ts`:

```typescript
export interface DuplicateProfileResult {
  name: string;
  profile: GameProfile;
}
```

## Files to Create/Modify

### Files to Modify

| File                                              | Change                                                                                                                                                  |
| ------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `crates/crosshook-core/src/profile/toml_store.rs` | Add `DuplicateProfileResult` struct, `duplicate()` method, `generate_unique_copy_name()` private method, `strip_copy_suffix()` free function, and tests |
| `crates/crosshook-core/src/profile/mod.rs`        | Export `DuplicateProfileResult`                                                                                                                         |
| `src-tauri/src/commands/profile.rs`               | Add `profile_duplicate` Tauri command                                                                                                                   |
| `src-tauri/src/lib.rs`                            | Register `commands::profile::profile_duplicate` in `invoke_handler`                                                                                     |
| `src/types/profile.ts`                            | Add `DuplicateProfileResult` interface                                                                                                                  |
| `src/hooks/useProfile.ts`                         | Add `duplicateProfile` to `UseProfileResult` and implement                                                                                              |
| `src/components/ProfileActions.tsx`               | Add `canDuplicate` and `onDuplicate` props, render Duplicate button                                                                                     |
| `src/components/pages/ProfilesPage.tsx`           | Wire `canDuplicate` and `onDuplicate` to ProfileActions                                                                                                 |

### No New Files Required

The feature integrates entirely into existing files and modules.

## Technical Decisions

### Decision 1: New `duplicate()` Method vs Composing `load()`/`save()` at Command Layer

**Option A (recommended):** Add `ProfileStore::duplicate()` method

- Encapsulates name generation logic in the core library
- Keeps the Tauri command thin (consistent with all other profile commands)
- Name generation logic is testable without Tauri
- Follows the pattern of `rename()` and `import_legacy()` which are also composite operations

**Option B:** Compose `load()` + `save()` in the Tauri command

- Simpler initially, but leaks business logic (name generation) into the command layer
- Breaks the pattern where all profile business logic lives in `crosshook-core`

### Decision 2: Auto-Generated Name Only vs Optional Explicit Name

**Recommendation:** Accept `Option<&str>` for the new name

- Auto-generated names are the primary UX flow ("Duplicate" button click)
- Explicit names support a future "Duplicate as..." dialog without API changes
- Minimal extra code (just a `match` on the option)

### Decision 3: Overwrite Behavior for Explicit Names

**Recommendation:** Allow overwrite (match existing `save()` and `rename()` behavior)

- `ProfileStore::save()` already overwrites without checking existence
- `ProfileStore::rename()` already overwrites the target (confirmed by `test_rename_overwrites_existing_target_profile`)
- Introducing overwrite-prevention only for `duplicate()` would be inconsistent
- The auto-generated name path already guarantees uniqueness via `list()` check

### Decision 4: Where to Place the UI Button

**Recommendation:** In `ProfileActions` next to Save and Delete

- Consistent with the existing action bar pattern
- Only enabled when a saved profile is selected (same guard as Delete)
- Keeps the profile selector area clean (no context menu needed for v1)

### Decision 5: Return Type -- `GameProfile` vs `DuplicateProfileResult`

**Recommendation:** Return `DuplicateProfileResult { name, profile }`

- The frontend needs both the generated name (for `selectProfile`) and the profile data
- Without `name` in the response, the frontend would need a second round-trip to discover the generated name
- This pattern is used by `import_legacy` which returns `GameProfile` (but there the caller already knows the name from the file path)

## Open Questions

1. **Should duplicate trigger launcher export cleanup?** -- No. The duplicated profile does not have an exported launcher. Launcher files are tied to the profile name, and the duplicate gets a new name. No launcher files need to be created or cleaned up during duplication.

2. **Should the UI auto-navigate to the duplicated profile?** -- Yes, recommended. After a successful duplicate, call `refreshProfiles()` then `selectProfile(result.name)`. This matches the UX pattern after save (which also reloads the profile list and selects the saved profile).

3. **Race condition: what if another profile is created between `list()` and `save()`?** -- Extremely unlikely in a single-user desktop app. The `save()` call would silently overwrite the conflicting profile. This is acceptable for the same reason `rename()` does not check for target existence. If this ever becomes a concern, an atomic `O_EXCL` file creation could be used instead of `fs::write`.

4. **Should the CLI (`crosshook-cli`) also support duplication?** -- Out of scope for this issue (#56), which specifically targets the Tauri command and UI. The CLI can be added later using the same `ProfileStore::duplicate()` method.

5. **Maximum copy number limit?** -- Not needed. The `u32` loop gives 4 billion candidates before theoretical exhaustion. In practice, a user would never have more than a handful of copies.
