# External Research: Duplicate Profile Feature

## Executive Summary

The duplicate-profile feature is primarily an internal operation requiring no external APIs or services. The implementation leverages well-established patterns from Tauri v2 IPC command design, Rust filesystem safety primitives, and OS-standard naming conventions. The core operation is straightforward: load an existing TOML profile, generate a unique new name, and save the clone. The research below covers the five key dimensions: Tauri v2 IPC patterns, TOML file operations, filesystem safety, name generation algorithms, and precedent from similar desktop applications.

**Confidence**: High -- all patterns are well-documented, the codebase already uses consistent conventions, and no external dependencies are needed.

---

## Tauri v2 IPC Patterns

### Command Design for CRUD Operations

Tauri v2 commands follow a consistent pattern that the CrossHook codebase already implements well:

1. **Command definition**: Functions annotated with `#[tauri::command]` accepting typed arguments
2. **State access**: `State<'_, ProfileStore>` for shared backend state
3. **Error handling**: `Result<T, String>` with `map_err(map_error)` converting domain errors
4. **Naming convention**: `profile_*` prefix for all profile commands (e.g., `profile_load`, `profile_save`, `profile_delete`, `profile_rename`)

The existing CrossHook pattern for a new command like `profile_duplicate` would follow the established convention:

```rust
#[tauri::command]
pub fn profile_duplicate(
    source_name: String,
    new_name: Option<String>,  // None triggers auto-name generation
    store: State<'_, ProfileStore>,
) -> Result<String, String> {
    store.duplicate(&source_name, new_name.as_deref()).map_err(map_error)
}
```

**Key constraints from Tauri v2 docs:**

- All arguments and return types must implement `serde::Serialize` / `serde::Deserialize`
- JavaScript calls use camelCase argument names (e.g., `sourceName`, `newName`)
- Command names must be globally unique across the application
- Synchronous commands block the main thread; async is preferred for I/O but the existing profile commands are synchronous (and I/O is minimal for a single TOML file)

**Confidence**: High -- based on official Tauri v2 documentation at <https://v2.tauri.app/develop/calling-rust/> and existing codebase patterns.

### Error Handling Pattern

The CrossHook codebase uses a simple `map_err(|e| e.to_string())` approach. For the duplicate feature, a new `AlreadyExists` error variant in `ProfileStoreError` would be appropriate for explicit conflict detection:

```rust
#[derive(Debug)]
pub enum ProfileStoreError {
    // existing variants...
    AlreadyExists(String),  // new variant for name conflicts
}
```

Reference: Tauri error handling best practices at <https://tbt.qkation.com/posts/tauri-error-handling/> recommend custom error types over generic strings, though the existing CrossHook approach of mapping to String at the IPC boundary is pragmatic and consistent.

**Confidence**: High

---

## TOML File Operations

### Read-Modify-Save Pattern

The CrossHook codebase uses the `toml` crate with `serde` for all profile I/O:

- **Load**: `fs::read_to_string` -> `toml::from_str::<GameProfile>`
- **Save**: `toml::to_string_pretty` -> `fs::write`

For duplication, the pattern is simply load + save with a new name -- no TOML-level manipulation is needed because the profile struct is fully deserialized and re-serialized:

```rust
pub fn duplicate(&self, source_name: &str, new_name: Option<&str>) -> Result<String, ProfileStoreError> {
    let profile = self.load(source_name)?;
    let target_name = match new_name {
        Some(name) => name.to_string(),
        None => self.generate_unique_name(source_name)?,
    };
    // Conflict check before writing
    let target_path = self.profile_path(&target_name)?;
    if target_path.exists() {
        return Err(ProfileStoreError::AlreadyExists(target_name));
    }
    self.save(&target_name, &profile)?;
    Ok(target_name)
}
```

### Format Preservation

The `toml::to_string_pretty` serializer produces clean, formatted TOML output. Since profiles are fully deserialized into `GameProfile` structs and re-serialized, any comments in the original TOML file would be lost. This is acceptable since CrossHook profiles are machine-generated and do not contain user-edited comments.

If comment preservation were needed, the `toml_edit` crate (<https://github.com/toml-rs/toml>) provides format-preserving round-trip editing, but this is unnecessary for this feature.

**Confidence**: High -- based on existing codebase patterns in `toml_store.rs` and official `toml` crate documentation at <https://docs.rs/toml>.

---

## Filesystem Safety Patterns

### Conflict Detection Approaches

Three approaches exist for preventing name collisions:

| Approach                                   | Mechanism                     | Race-Safe   | Complexity |
| ------------------------------------------ | ----------------------------- | ----------- | ---------- |
| `path.exists()` check                      | Pre-check before write        | No (TOCTOU) | Low        |
| `OpenOptions::create_new(true)`            | Atomic kernel-level check     | Yes         | Medium     |
| `fs::rename` from temp file + `create_new` | Atomic write + conflict check | Yes         | Higher     |

**Recommendation for CrossHook**: Use `path.exists()` pre-check. The TOCTOU race condition is theoretical for this use case because:

- CrossHook is a single-user desktop application
- Profile operations are not concurrent (UI triggers one operation at a time)
- The existing `rename` and `save` methods already use `fs::rename` / `fs::write` without atomic safeguards
- Consistency with the existing codebase pattern is more important than theoretical safety

If stricter safety is desired, the `OpenOptions::create_new(true)` pattern from Rust's standard library provides kernel-level atomic conflict detection:

```rust
use std::fs::OpenOptions;
use std::io::Write;

let content = toml::to_string_pretty(&profile)?;
let mut file = OpenOptions::new()
    .write(true)
    .create_new(true)  // Fails atomically if file exists
    .open(&target_path)?;
file.write_all(content.as_bytes())?;
```

Reference: <https://doc.rust-lang.org/std/fs/struct.OpenOptions.html>

### Atomic Write Crates (Not Recommended)

Several Rust crates provide atomic file writes:

- `atomicwrites` -- write-to-temp-then-rename pattern with `AllowOverwrite` / `DisallowOverwrite` modes (<https://github.com/untitaker/rust-atomicwrites>)
- `atomic-write-file` -- atomic overwrite without intermediate states (<https://crates.io/crates/atomic-write-file>)

These are **not recommended** for this feature because:

- The existing codebase uses simple `fs::write` throughout
- Adding a new dependency for a single file copy operation is over-engineering
- The operation is not crash-critical (a partial write would produce an invalid TOML that fails to load, which is a safe failure mode)

**Confidence**: High -- based on Rust stdlib docs and analysis of existing codebase patterns.

---

## Name Generation Algorithms

### OS Convention Survey

| Platform         | Duplicate Naming Pattern             | Example                                |
| ---------------- | ------------------------------------ | -------------------------------------- |
| Windows Explorer | `{name} - Copy`, `{name} - Copy (2)` | `Profile - Copy`, `Profile - Copy (2)` |
| macOS Finder     | `{name} copy`, `{name} copy 2`       | `Profile copy`, `Profile copy 2`       |
| GNOME Files      | `{name} (copy)`, `{name} (copy 2)`   | `Profile (copy)`, `Profile (copy 2)`   |
| VS Code (ext)    | `{name}-copy`, `{name}-copy-2`       | `Profile-copy`, `Profile-copy-2`       |
| Firefox profiles | `{name} (copy)`                      | Not user-visible                       |

### Recommended Algorithm for CrossHook

The pattern `{Name} (Copy)`, `{Name} (Copy 2)`, `{Name} (Copy 3)` is the most intuitive for a desktop application. Here is the algorithm:

```rust
fn generate_unique_name(&self, source_name: &str) -> Result<String, ProfileStoreError> {
    let existing = self.list()?;
    let existing_set: std::collections::HashSet<_> = existing.iter().map(|s| s.as_str()).collect();

    let base = source_name
        .trim_end_matches(|c: char| c.is_ascii_digit() || c == ' ' || c == ')')
        .trim_end_matches("(Copy")
        .trim_end_matches(" (Copy")
        .trim();
    let base = if base.is_empty() { source_name } else { base };

    let candidate = format!("{base} (Copy)");
    if !existing_set.contains(candidate.as_str()) {
        return Ok(candidate);
    }

    for n in 2..=1000 {
        let candidate = format!("{base} (Copy {n})");
        if !existing_set.contains(candidate.as_str()) {
            return Ok(candidate);
        }
    }

    Err(ProfileStoreError::InvalidName(
        format!("could not generate unique name for {source_name}")
    ))
}
```

**Key design decisions:**

- Use `(Copy)` suffix, not `- Copy`, to avoid conflicts with game names that use hyphens
- Strip existing `(Copy N)` suffixes before appending to avoid `Name (Copy) (Copy)`
- Load the full profile list once and check in-memory (O(1) lookup with HashSet) rather than checking filesystem per candidate
- Cap iterations at 1000 as a safety bound (no user will have 1000 copies)

**Confidence**: High -- based on established OS conventions and straightforward algorithmic pattern.

---

## Similar Implementations

### VS Code Duplicate Extension

The `vscode-duplicate` extension (<https://github.com/mrmlnc/vscode-duplicate>) implements file duplication with:

- Right-click context menu action "Duplicate file"
- Opens a dialog pre-filled with `{filename}-copy.{ext}`
- User can edit the name before confirming
- Validates the new name before creating

This maps closely to the CrossHook use case: context menu action -> pre-filled name -> optional user edit -> save.

### General Desktop App Patterns

From the NN/g UX research on context menus (<https://www.nngroup.com/articles/contextual-menus/>):

- "Duplicate" is a standard action grouped with Edit/Delete operations
- Power users expect `Ctrl+D` / `Cmd+D` as a keyboard shortcut
- The action label should be verb-first: "Duplicate", not "Make a copy"
- In data grid contexts, context menus typically offer: Edit, **Duplicate**, Archive, Export

### Electron Profile Manager

The `electron-profile` library (<https://github.com/electron-utils/electron-profile>) provides a pattern for profile storage and manipulation in Electron apps, using JSON files with CRUD operations, similar to CrossHook's TOML-based approach.

**Confidence**: Medium -- based on publicly available extension source code and UX research articles.

---

## Code Examples

### Complete Rust Implementation Sketch

```rust
// In ProfileStore (toml_store.rs)
pub fn duplicate(
    &self,
    source_name: &str,
    new_name: Option<&str>,
) -> Result<String, ProfileStoreError> {
    let source_name = source_name.trim();
    let profile = self.load(source_name)?;

    let target_name = match new_name {
        Some(name) => {
            let name = name.trim();
            validate_name(name)?;
            name.to_string()
        }
        None => self.generate_unique_name(source_name)?,
    };

    let target_path = self.profile_path(&target_name)?;
    if target_path.exists() {
        return Err(ProfileStoreError::AlreadyExists(target_name));
    }

    self.save(&target_name, &profile)?;
    Ok(target_name)
}
```

### Tauri Command

```rust
#[tauri::command]
pub fn profile_duplicate(
    source_name: String,
    new_name: Option<String>,
    store: State<'_, ProfileStore>,
) -> Result<String, String> {
    store
        .duplicate(&source_name, new_name.as_deref())
        .map_err(map_error)
}
```

### Frontend Invocation

```typescript
import { invoke } from '@tauri-apps/api/core';

async function duplicateProfile(sourceName: string, newName?: string): Promise<string> {
  return invoke<string>('profile_duplicate', {
    sourceName,
    newName: newName ?? null,
  });
}
```

---

## Open Questions

1. **Should duplication be purely backend-driven or frontend-driven?** The backend can auto-generate names, but the frontend could also pre-compute names by calling `profile_list` first. Backend-driven is simpler and avoids race conditions.

2. **Should the duplicated profile's `game.name` or `steam.launcher.display_name` be modified?** If "Elden Ring" is duplicated, should the clone's game name become "Elden Ring (Copy)" or remain "Elden Ring"? The profile _file_ name will differ, but internal fields are a UX decision.

3. **Should launcher exports be duplicated?** The existing profile has exported `.sh` scripts and `.desktop` entries. The duplicate should NOT inherit these -- they reference the original profile name and paths.

4. **Keyboard shortcut**: Should `Ctrl+D` be assigned for quick duplication? This conflicts with some browser/system shortcuts but is the de facto standard for "Duplicate" in desktop apps.

5. **Undo support**: Should profile duplication be undoable? The simplest approach is to just delete the clone, but a formal undo stack is a larger architectural decision.

---

## Sources

- [Tauri v2 IPC Documentation](https://v2.tauri.app/concept/inter-process-communication/)
- [Tauri v2 Calling Rust from Frontend](https://v2.tauri.app/develop/calling-rust/)
- [Tauri Error Handling Recipes](https://tbt.qkation.com/posts/tauri-error-handling/)
- [Tauri v2 File System Plugin](https://v2.tauri.app/plugin/file-system/)
- [Rust std::fs::copy Documentation](https://doc.rust-lang.org/std/fs/fn.copy.html)
- [Rust OpenOptions::create_new](https://doc.rust-lang.org/std/fs/struct.OpenOptions.html)
- [rust-atomicwrites Crate](https://github.com/untitaker/rust-atomicwrites)
- [atomic-write-file Crate](https://crates.io/crates/atomic-write-file)
- [toml-rs Crate Documentation](https://docs.rs/toml)
- [toml_edit for Format-Preserving Edits](https://github.com/toml-rs/toml)
- [Rust Atomic File Write Forum Discussion](https://users.rust-lang.org/t/how-to-write-replace-files-atomically/42821)
- [Windows Copy Name Template](https://winaero.com/change-the-copied-file-name-template-in-windows-10/)
- [macOS Duplicate File Naming](https://discussions.apple.com/thread/255186790)
- [VS Code Duplicate Extension](https://github.com/mrmlnc/vscode-duplicate)
- [VS Code Duplicate Feature Request](https://github.com/microsoft/vscode/issues/127256)
- [NN/g Contextual Menus Guidelines](https://www.nngroup.com/articles/contextual-menus/)
- [NN/g Designing Effective Contextual Menus](https://www.nngroup.com/articles/contextual-menus-guidelines/)
- [electron-profile Library](https://github.com/electron-utils/electron-profile)
- [Tauri v2 Error Handling Discussion](https://github.com/tauri-apps/tauri/discussions/8805)
- [Handling Errors in Tauri Tutorial](https://tauritutorials.com/blog/handling-errors-in-tauri)

---

## Search Queries Executed

1. `Tauri v2 IPC command patterns file operations CRUD best practices 2025`
2. `Rust safe file copy atomic write conflict detection pattern filesystem`
3. `auto generate unique copy name algorithm "Copy" "Copy 2" duplicate naming pattern`
4. `Tauri desktop app duplicate clone profile configuration implementation`
5. `Windows macOS file copy naming convention "Copy" "(2)" duplicate file naming algorithm`
6. `Rust TOML file read modify write serde toml crate best practices`
7. `Tauri v2 command error handling Result serde serialize pattern`
8. `Electron desktop app duplicate profile clone configuration pattern UX`
9. `Rust std::fs::copy vs read write file duplicate safe pattern`
10. `unique name generation algorithm "Name (Copy)" increment suffix existing files programming`
11. `VS Code duplicate file keyboard shortcut implementation similar desktop apps clone action`
12. `desktop app UX "duplicate" "clone" context menu action button profile Firefox Chrome browser profile duplicate`
13. `Rust check file exists before creating race condition safe pattern create_new OpenOptions`

---

## Uncertainties & Gaps

- **No Tauri v2 apps with profile duplication found**: Could not find an open-source Tauri v2 application that implements profile/configuration cloning as a reference implementation. The patterns are assembled from Tauri docs and general desktop app conventions.
- **Concurrent access not a practical concern**: While TOCTOU race conditions exist theoretically, CrossHook is single-user and UI-driven, making this a non-issue in practice.
- **`toml_edit` not evaluated in depth**: If format-preserving duplication (keeping comments, ordering) is ever needed, the `toml_edit` crate would need deeper evaluation. Currently unnecessary since CrossHook profiles are machine-generated.
- **No benchmarking data**: For very large profile directories (100+ profiles), the `list()` + HashSet approach for name generation has not been benchmarked, but should be well within acceptable performance for any realistic profile count.
