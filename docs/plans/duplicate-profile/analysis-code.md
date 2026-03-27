# Duplicate Profile — Code Analysis

## Executive Summary

Profile duplication requires changes across all three layers: a new `duplicate()` method on `ProfileStore` (Rust core), a `profile_duplicate` Tauri command (IPC layer), and a `duplicateProfile` callback on `useProfile` (React hook). The codebase has strong, consistent patterns at every layer — the implementation is a composition exercise, not a design exercise. The primary risk is the silent-overwrite behavior of `ProfileStore::save()`, which requires a pre-save uniqueness check against `list()`.

## Existing Code Structure

### Backend Layer (`crosshook-core`)

| File                                              | Role                                                       | Lines |
| ------------------------------------------------- | ---------------------------------------------------------- | ----- |
| `crates/crosshook-core/src/profile/toml_store.rs` | `ProfileStore` struct — all filesystem CRUD                | ~478  |
| `crates/crosshook-core/src/profile/models.rs`     | `GameProfile` struct and section types                     | ~293  |
| `crates/crosshook-core/src/profile/mod.rs`        | Module re-exports                                          | ~23   |
| `crates/crosshook-core/src/profile/exchange.rs`   | Community profile import/export, `sanitize_profile_name()` | ~437  |

### IPC Layer (`src-tauri`)

| File                                | Role                                     | Lines |
| ----------------------------------- | ---------------------------------------- | ----- |
| `src-tauri/src/commands/profile.rs` | Tauri command handlers for profile ops   | ~297  |
| `src-tauri/src/lib.rs`              | Command registration in `invoke_handler` | ~112  |

### Frontend Layer (`src/`)

| File                                    | Role                                                        | Lines |
| --------------------------------------- | ----------------------------------------------------------- | ----- |
| `src/hooks/useProfile.ts`               | Profile state machine — all CRUD operations                 | ~710  |
| `src/context/ProfileContext.tsx`        | Context wrapper, spreads `UseProfileResult`                 | ~73   |
| `src/types/profile.ts`                  | `GameProfile` TypeScript interface                          | ~54   |
| `src/types/launcher.ts`                 | `LauncherDeleteResult`, `LauncherRenameResult` result types | ~30   |
| `src/types/index.ts`                    | Barrel re-exports                                           | ~8    |
| `src/components/ProfileActions.tsx`     | Save/Delete buttons component                               | ~48   |
| `src/components/pages/ProfilesPage.tsx` | Page wiring context to components                           | ~216  |

## Implementation Patterns (with code examples)

### Pattern 1: ProfileStore Method Composition

All `ProfileStore` methods are stateless — they take `&self` (which holds only `base_path: PathBuf`) and compose `profile_path()`, `load()`, `save()`, `list()`. New methods follow the same pattern.

```rust
// EXISTING — rename composes profile_path + fs::rename
pub fn rename(&self, old_name: &str, new_name: &str) -> Result<(), ProfileStoreError> {
    let old_name = old_name.trim();
    let new_name = new_name.trim();
    validate_name(old_name)?;
    validate_name(new_name)?;
    let old_path = self.profile_path(old_name)?;
    // ... fs operations ...
    Ok(())
}

// NEW duplicate() should compose: load() -> generate name via list() -> save()
```

**Key detail**: `save()` at line 93-98 does `fs::write()` unconditionally — no existence check. The `duplicate()` method MUST check `list()` before calling `save()` to prevent overwriting.

### Pattern 2: Error Enum — ProfileStoreError

All errors go through `ProfileStoreError` (toml_store.rs:14-22). Duplicate will need a new variant:

```rust
// EXISTING variants:
pub enum ProfileStoreError {
    InvalidName(String),
    NotFound(PathBuf),
    InvalidLaunchOptimizationId(String),
    Io(std::io::Error),
    TomlDe(toml::de::Error),
    TomlSer(toml::ser::Error),
}

// NEW variant needed:
// NameCollisionExhausted(String)  — when all candidate names "X (Copy)", "X (Copy 2)"..."X (Copy N)" are taken
```

The `Display` impl and `std::error::Error` impl must be extended.

### Pattern 3: Thin Tauri Command Delegation

Every Tauri command in `commands/profile.rs` follows this exact 1-3 line pattern:

```rust
#[tauri::command]
pub fn profile_list(store: State<'_, ProfileStore>) -> Result<Vec<String>, String> {
    store.list().map_err(map_error)
}

#[tauri::command]
pub fn profile_rename(
    old_name: String,
    new_name: String,
    store: State<'_, ProfileStore>,
) -> Result<(), String> {
    store.rename(&old_name, &new_name).map_err(map_error)
}
```

The new command MUST follow this pattern:

```rust
#[tauri::command]
pub fn profile_duplicate(
    name: String,
    store: State<'_, ProfileStore>,
) -> Result<DuplicateProfileResult, String> {
    store.duplicate(&name).map_err(map_error)
}
```

### Pattern 4: Result Structs for IPC

Rich operations return dedicated serde-derived result structs. Examples from `launcher.ts`:

```typescript
// TypeScript side
export interface LauncherDeleteResult {
  script_deleted: boolean;
  desktop_entry_deleted: boolean;
  // ...
}

export interface LauncherRenameResult {
  old_slug: string;
  new_slug: string;
  // ...
}
```

The new `DuplicateProfileResult` follows this pattern:

```rust
// Rust side — in toml_store.rs
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DuplicateProfileResult {
    pub name: String,
    pub profile: GameProfile,
}
```

```typescript
// TypeScript side — in types/profile.ts
export interface DuplicateProfileResult {
  name: string;
  profile: GameProfile;
}
```

### Pattern 5: useProfile Hook State Machine

All async operations in `useProfile.ts` follow this state transition pattern:

```typescript
// EXISTING — persistProfileDraft (lines 512-548)
const persistProfileDraft = useCallback(
  async (name: string, draftProfile: GameProfile): Promise<PersistProfileDraftResult> => {
    // 1. Validate inputs
    const trimmedName = name.trim();
    if (!trimmedName) { setError(...); return { ok: false, error: ... }; }

    // 2. Set loading state
    setSaving(true);
    setError(null);

    try {
      // 3. Invoke backend
      await invoke('profile_save', { name: trimmedName, data: normalizedProfile });

      // 4. Refresh list + select result
      await refreshProfiles();
      await loadProfile(trimmedName);
      return { ok: true };
    } catch (err) {
      // 5. Set error state
      setError(err instanceof Error ? err.message : String(err));
      return { ok: false, error: message };
    } finally {
      // 6. Clear loading state
      setSaving(false);
    }
  },
  [loadProfile, refreshProfiles, syncProfileMetadata]
);
```

The new `duplicateProfile` callback MUST follow the same sequence:

1. Guard: check `profileExists` and `!saving && !deleting && !loading`
2. Set state: `setSaving(true); setError(null);`
3. Invoke: `invoke<DuplicateProfileResult>('profile_duplicate', { name })`
4. Refresh: `await refreshProfiles();`
5. Select: `await loadProfile(result.name);`
6. Catch/finally: same error handling pattern

### Pattern 6: Command Registration

Commands are registered in `lib.rs` lines 70-108 in grouped blocks. Profile commands are at lines 91-97:

```rust
commands::profile::profile_delete,
commands::profile::profile_import_legacy,
commands::profile::profile_list,
commands::profile::profile_load,
commands::profile::profile_rename,
commands::profile::profile_save,
commands::profile::profile_save_launch_optimizations,
```

Add `commands::profile::profile_duplicate` to this block (alphabetical).

### Pattern 7: Context Auto-Propagation

`ProfileContext.tsx` spreads all of `UseProfileResult` into the context value:

```typescript
const value = useMemo<ProfileContextValue>(
  () => ({
    ...profileState, // <-- auto-includes any new fields from UseProfileResult
    launchMethod,
    steamClientInstallPath,
    targetHomePath,
  }),
  [launchMethod, profileState, steamClientInstallPath, targetHomePath]
);
```

Adding `duplicateProfile` to `UseProfileResult` (the return type of `useProfile`) automatically makes it available via `useProfileContext()`. No changes needed to `ProfileContext.tsx`.

### Pattern 8: Filesystem Test Isolation

All Rust tests use `tempfile::tempdir()` + `ProfileStore::with_base_path()`:

```rust
#[test]
fn save_load_list_and_delete_round_trip() {
    let temp_dir = tempdir().unwrap();
    let store = ProfileStore::with_base_path(temp_dir.path().join("profiles"));
    let profile = sample_profile();

    store.save("elden-ring", &profile).unwrap();
    assert_eq!(store.list().unwrap(), vec!["elden-ring".to_string()]);
    // ...
}
```

Duplicate tests should follow this pattern. Key test cases:

- Basic duplicate produces `"X (Copy)"`
- Duplicate of `"X (Copy)"` produces `"X (Copy 2)"`
- Duplicate when `"X (Copy)"` exists already skips to `"X (Copy 2)"`
- Duplicate preserves all profile fields (deep equality check)
- Duplicate of nonexistent profile returns `NotFound`

## Integration Points

### Files to MODIFY

1. **`toml_store.rs`** — Add `duplicate()`, `generate_unique_copy_name()`, `strip_copy_suffix()`, `DuplicateProfileResult` struct, new `ProfileStoreError::NameCollisionExhausted` variant, and tests
2. **`mod.rs`** (profile module) — Add `DuplicateProfileResult` to `pub use toml_store::` line
3. **`commands/profile.rs`** — Add `profile_duplicate` Tauri command
4. **`lib.rs`** — Register `profile_duplicate` in `invoke_handler` macro
5. **`useProfile.ts`** — Add `duplicateProfile` callback and `duplicating` state; add to `UseProfileResult` interface
6. **`types/profile.ts`** — Add `DuplicateProfileResult` interface
7. **`ProfileActions.tsx`** — Add Duplicate button with `canDuplicate` and `onDuplicate` props
8. **`ProfilesPage.tsx`** — Wire `duplicateProfile` and `canDuplicate` from context to `ProfileActions`

### Files to CREATE

None. All changes fit within existing files.

## Code Conventions

### Rust

- `snake_case` everywhere (functions, variables, modules)
- `Result<T, ProfileStoreError>` return type for all ProfileStore methods
- `validate_name()` called at entry points, not deep in helpers
- `#[derive(Debug, Clone, Serialize, Deserialize)]` on all IPC-crossing types
- Tests in `#[cfg(test)] mod tests` at bottom of file, using `super::*`

### TypeScript

- `camelCase` for functions/hooks, `PascalCase` for components/interfaces
- `invoke<ReturnType>('command_name', { param })` — generic type parameter on invoke
- Error handling: `err instanceof Error ? err.message : String(err)`
- All callbacks wrapped in `useCallback` with explicit dependency arrays

### Naming

- Rust: `profile_duplicate` (Tauri command), `duplicate()` (store method), `DuplicateProfileResult`
- TypeScript: `duplicateProfile` (hook callback), `DuplicateProfileResult` (interface), `onDuplicate` (prop), `canDuplicate` (prop)

## Dependencies and Services

### Rust Dependencies (already present)

- `serde` — Serialize/Deserialize for `DuplicateProfileResult`
- `std::fs` — filesystem ops (already used in all ProfileStore methods)
- `tempfile` — test isolation (already a dev dependency)

### No new dependencies needed

The implementation composes existing primitives. No new crates or npm packages required.

## Gotchas and Warnings

### Critical: `save()` Silently Overwrites

`ProfileStore::save()` (toml_store.rs:93-98) calls `fs::write()` with no existence check. The `duplicate()` method MUST call `list()` to get existing names and verify uniqueness BEFORE calling `save()`. This is the #1 safety constraint.

### Name Validation Allows Parentheses and Digits

`validate_name()` (toml_store.rs:189-214) rejects `< > : " / \ | ? *` but allows parentheses, spaces, and digits. Generated names like `"My Profile (Copy 2)"` pass validation — confirmed by inspection.

### `rename()` Overwrites Existing Target

`ProfileStore::rename()` uses `fs::rename()` which overwrites the target file if it exists (see test at line 405-419). This is a known behavior in the codebase but reinforces why `duplicate()` must check before saving.

### Strip Copy Suffix Edge Cases

The `strip_copy_suffix()` helper must handle:

- `"My Profile (Copy)"` -> `"My Profile"`
- `"My Profile (Copy 2)"` -> `"My Profile"`
- `"My Profile (Copy 99)"` -> `"My Profile"`
- `"My Profile"` -> `"My Profile"` (no suffix to strip)
- `"Copy"` -> `"Copy"` (the word "Copy" alone is not a suffix pattern)
- `"Game (Special Edition)"` -> `"Game (Special Edition)"` (don't strip non-copy parens)

### Frontend State: `duplicating` vs `saving`

The hook already has `saving` state. Adding a separate `duplicating` boolean avoids conflicts if someone tries to duplicate while a save is in progress (both should be guarded). The `canDuplicate` guard should check: `profileExists && !saving && !deleting && !loading && !duplicating`.

### Context Spread Propagation

`ProfileContext.tsx` uses `...profileState` spread, so adding `duplicateProfile` and `duplicating` to `UseProfileResult` auto-propagates to consumers. However, `ProfileContextValue` interface extends `UseProfileResult`, so TypeScript will catch any consumer that destructures the wrong name.

### Command Registration is Order-Sensitive

The `invoke_handler!` macro in `lib.rs` takes a comma-separated list. Add `commands::profile::profile_duplicate` in alphabetical position within the profile block (after `profile_delete`, before `profile_import_legacy`).

### Test Pattern: `sample_profile()` is Duplicated

Both `toml_store.rs` and `exchange.rs` define their own `sample_profile()` test helpers — they are NOT shared. New tests in `toml_store.rs` should use the existing `sample_profile()` defined there (line 221-256).

## Task-Specific Guidance

### Task: Implement `duplicate()` on ProfileStore

- Location: `toml_store.rs`, after `rename()` method (line 165)
- Compose: `load()` -> `generate_unique_copy_name()` (using `list()`) -> `save()` -> return `DuplicateProfileResult`
- Add `DuplicateProfileResult` struct above the `impl ProfileStore` block
- Add `NameCollisionExhausted(String)` to `ProfileStoreError` enum + `Display` match arm
- Helper `generate_unique_copy_name(base: &str, existing: &[String]) -> Result<String, ProfileStoreError>` — pure function, easily testable
- Helper `strip_copy_suffix(name: &str) -> &str` — strips `(Copy)` or `(Copy N)` suffix

### Task: Add Tauri Command

- Location: `commands/profile.rs`, after `profile_delete` command (line 123)
- Single function: `profile_duplicate(name: String, store: State<'_, ProfileStore>) -> Result<DuplicateProfileResult, String>`
- Registration: `lib.rs` line ~91, add `commands::profile::profile_duplicate,`

### Task: Add Frontend Hook Callback

- Location: `useProfile.ts`, after `persistProfileDraft` (line 548)
- New state: `const [duplicating, setDuplicating] = useState(false);`
- New callback: `duplicateProfile` — follows `persistProfileDraft` pattern
- Add to `UseProfileResult` interface: `duplicateProfile: (name: string) => Promise<void>; duplicating: boolean;`
- Add to return object

### Task: Add TypeScript Types

- Location: `types/profile.ts`, after `GameProfile` interface (line 54)
- Add: `export interface DuplicateProfileResult { name: string; profile: GameProfile; }`

### Task: Add Duplicate Button to UI

- Location: `ProfileActions.tsx` — add `canDuplicate: boolean; onDuplicate: () => void | Promise<void>; duplicating: boolean;` to props interface
- Add button between Save and Delete, using `crosshook-button crosshook-button--secondary` class
- Location: `ProfilesPage.tsx` — compute `canDuplicate` guard, pass props from context

### Task: Module Re-exports

- Location: `mod.rs` line 23 — change `pub use toml_store::{ProfileStore, ProfileStoreError};` to `pub use toml_store::{DuplicateProfileResult, ProfileStore, ProfileStoreError};`
