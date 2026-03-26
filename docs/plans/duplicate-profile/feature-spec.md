# Feature Spec: Profile Duplicate / Clone

## Executive Summary

Profile duplication (#56) adds a single-action "Duplicate" command that clones an existing profile under a unique auto-generated name, enabling users to create variant configurations (different trainers, optimization presets, Proton versions) without rebuilding profiles from scratch. The implementation is a genuine quick win: a new `ProfileStore::duplicate()` method composing existing `load`/`save`/`list` primitives, a thin `profile_duplicate` Tauri command, and a "Duplicate" button in the existing `ProfileActions` component. No external APIs, no new dependencies, and no new files are required -- the feature integrates entirely into existing modules. The primary risk is silent overwrite since `ProfileStore::save()` has no existence guard, which the duplicate method must address with an explicit conflict check.

## External Dependencies

### APIs and Services

None. This is a purely internal feature using existing `ProfileStore` CRUD operations.

### Libraries and SDKs

No new dependencies required. The implementation uses:

| Library                | Already Used | Purpose                                          |
| ---------------------- | :----------: | ------------------------------------------------ |
| `toml` + `serde`       |     Yes      | Profile serialization/deserialization (existing) |
| `std::fs`              |     Yes      | Filesystem operations (existing)                 |
| `@tauri-apps/api/core` |     Yes      | Frontend `invoke()` calls (existing)             |

### External Documentation

- [Tauri v2 IPC Documentation](https://v2.tauri.app/develop/calling-rust/): Command pattern reference
- [Rust std::fs::OpenOptions](https://doc.rust-lang.org/std/fs/struct.OpenOptions.html): Atomic `create_new` if stricter safety is ever needed

## Business Requirements

### User Stories

**Primary User: Game/trainer configuration enthusiast**

- **US-1**: As a user tuning Proton launch settings, I want to duplicate a working profile so I can experiment with different launch optimizations without risking my known-good configuration.
- **US-2**: As a user with multiple trainer executables for the same game, I want to clone my profile and swap the trainer path so I can quickly switch between FLiNG and WeMod setups.
- **US-3**: As a user who installed a community profile, I want to duplicate it before making local modifications so I can preserve the original community configuration for reference or re-sync.
- **US-4**: As a user about to make significant changes (switching launch method, changing Proton version), I want to clone my profile as a safety net before experimenting.

### Business Rules

1. **Source must exist on disk**: The profile being duplicated must pass `ProfileStore::load()`. Unsaved/dirty profiles cannot be duplicated.
2. **New name must be unique**: The generated name must not collide with any existing profile in `ProfileStore::list()`. The duplicate operation must **never silently overwrite** an existing profile (unlike `rename()` and `save()` which do).
3. **New name must pass validation**: Must satisfy `validate_name()` rules -- not empty, no `.`/`..`, no absolute paths, no reserved characters (`< > : " / \ | ? *`), trimmed.
4. **Full deep copy**: All `GameProfile` fields are copied verbatim via `.clone()`. The struct derives `Clone`, so no field-by-field logic is needed. The only difference is the filename on disk.
5. **No launcher inheritance**: Duplicated profiles do NOT inherit exported launchers (`.sh` scripts, `.desktop` entries). Launchers are derived from profile content at export time; the duplicate starts with zero launcher files.
6. **No runtime state mutation**: The `runtime` section (prefix_path, proton_path, working_directory) copies as-is. These are user-specified config, not ephemeral state.
7. **No community provenance tracking**: Community profiles are stored as plain TOML with no metadata. Duplication is identical to any other profile.

### Name Generation Rules

8. **Pattern**: `"{original_name} (Copy)"` for first duplicate. `"{original_name} (Copy 2)"`, `"(Copy 3)"`, etc., for conflicts.
9. **Suffix stripping**: When duplicating a profile that already has a `(Copy)` or `(Copy N)` suffix, strip it before appending to avoid stacking (e.g., `"Game (Copy)"` becomes `"Game (Copy 2)"`, not `"Game (Copy) (Copy)"`).
10. **Collision detection**: Check against `ProfileStore::list()` using in-memory HashSet lookup.

### Edge Cases

| Scenario                          | Expected Behavior                                                         | Notes                                                          |
| --------------------------------- | ------------------------------------------------------------------------- | -------------------------------------------------------------- |
| Duplicate of a duplicate          | `"Game (Copy) (Copy)"` → strips suffix → `"Game (Copy 2)"`                | Suffix stripping prevents stacking                             |
| Profile with empty/default fields | Valid to duplicate                                                        | `ProfileStore::save` has no field-level validation beyond name |
| Very long profile name            | `(Copy N)` adds ~12 chars max; 255-byte filesystem limit is practical cap | Extremely unlikely edge case                                   |
| All `(Copy N)` names taken        | Error after exhausting candidates (u32 range)                             | Unreachable in practice                                        |
| Explicit name that already exists | Allow overwrite (matches `save()`/`rename()` behavior)                    | Only applies if caller provides explicit `new_name`            |

### Success Criteria

- [ ] "Duplicate" action creates a copy of the selected profile with a unique `(Copy)` name
- [ ] Duplicated profile is immediately selected and editable in the profile editor
- [ ] Name conflicts are handled gracefully via auto-incrementing suffix
- [ ] Existing tests pass; new tests cover duplicate + collision logic
- [ ] Error states surface in the UI using existing error banner pattern

## Technical Specifications

### Architecture Overview

```
ProfilesPage.tsx
  → ProfileActions.tsx             [new "Duplicate" button]
  → useProfileContext()            [provides duplicateProfile callback]
       → ProfileContext.tsx         [wraps useProfile]
            → useProfile.ts        [new duplicateProfile function]
                 → invoke('profile_duplicate', { sourceName, newName? })
                      → profile.rs::profile_duplicate()
                           → ProfileStore::duplicate()
                                → load() + list() + save()
                           → returns DuplicateProfileResult { name, profile }
```

### Data Models

#### GameProfile (copied verbatim)

All fields copied via `.clone()`. Profile identity is the TOML filename, not a field in the struct.

| Section     | Key Fields                                                          | Copy Strategy |
| ----------- | ------------------------------------------------------------------- | ------------- |
| `game`      | `name`, `executable_path`                                           | Verbatim      |
| `trainer`   | `path`, `kind`, `loading_mode`                                      | Verbatim      |
| `injection` | `dll_paths[]`, `inject_on_launch[]`                                 | Verbatim      |
| `steam`     | `enabled`, `app_id`, `compatdata_path`, `proton_path`, `launcher.*` | Verbatim      |
| `runtime`   | `prefix_path`, `proton_path`, `working_directory`                   | Verbatim      |
| `launch`    | `method`, `optimizations.enabled_option_ids[]`                      | Verbatim      |

#### DuplicateProfileResult (new struct)

```rust
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct DuplicateProfileResult {
    pub name: String,
    pub profile: GameProfile,
}
```

#### Existing Error Variants (sufficient)

```rust
pub enum ProfileStoreError {
    InvalidName(String),       // invalid new_name
    NotFound(PathBuf),         // source not found
    Io(std::io::Error),        // filesystem errors
    TomlDe(toml::de::Error),   // deserialization
    TomlSer(toml::ser::Error), // serialization
    // ...existing variants
}
```

No new error variant needed for auto-generated names (the loop guarantees uniqueness). For explicit names, conflict detection returns `InvalidName` with a descriptive message.

### API Design

#### `profile_duplicate` Tauri Command

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

**TypeScript invocation:**

```typescript
const result = await invoke<DuplicateProfileResult>('profile_duplicate', {
  sourceName: 'elden-ring',
  // newName: optional, omit for auto-generated name
});
// result: { name: "elden-ring (Copy)", profile: GameProfile }
```

**Errors:**

| Scenario               | Error Pattern                                             |
| ---------------------- | --------------------------------------------------------- |
| Source not found       | `"profile file not found: /path/to/profiles/source.toml"` |
| Invalid source name    | `"invalid profile name: ..."`                             |
| Invalid explicit name  | `"invalid profile name: ..."`                             |
| Filesystem write error | IO error message                                          |

### Core Library Design

#### ProfileStore::duplicate() Method

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

#### Name Generation Algorithm

```rust
fn generate_unique_copy_name(&self, source_name: &str) -> Result<String, ProfileStoreError> {
    let existing: HashSet<String> = self.list()?.into_iter().collect();
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
    unreachable!()
}
```

#### strip_copy_suffix() Helper

```rust
fn strip_copy_suffix(name: &str) -> &str {
    let trimmed = name.trim();
    if let Some(base) = trimmed.strip_suffix(" (Copy)") {
        return base;
    }
    if let Some(paren_start) = trimmed.rfind(" (Copy ") {
        let after = &trimmed[paren_start + 7..];
        if let Some(digits) = after.strip_suffix(')') {
            if !digits.is_empty() && digits.chars().all(|c| c.is_ascii_digit()) {
                return &trimmed[..paren_start];
            }
        }
    }
    trimmed
}
```

### Frontend Integration

#### useProfile Hook Addition

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

#### ProfileActions Button

```tsx
<button
  type="button"
  className="crosshook-button crosshook-button--secondary"
  onClick={() => void onDuplicate()}
  disabled={!canDuplicate}
>
  {duplicating ? 'Duplicating...' : 'Duplicate'}
</button>
```

**Button placement**: Between Save and Delete (constructive actions left, destructive right).

**Enable condition**: `profileExists && !saving && !deleting && !loading`

### System Integration

#### Files to Modify

| File                                              | Change                                                                                                   |
| ------------------------------------------------- | -------------------------------------------------------------------------------------------------------- |
| `crates/crosshook-core/src/profile/toml_store.rs` | Add `DuplicateProfileResult`, `duplicate()`, `generate_unique_copy_name()`, `strip_copy_suffix()`, tests |
| `crates/crosshook-core/src/profile/mod.rs`        | Export `DuplicateProfileResult`                                                                          |
| `src-tauri/src/commands/profile.rs`               | Add `profile_duplicate` command                                                                          |
| `src-tauri/src/lib.rs`                            | Register `profile_duplicate` in invoke handler                                                           |
| `src/types/profile.ts`                            | Add `DuplicateProfileResult` interface                                                                   |
| `src/hooks/useProfile.ts`                         | Add `duplicateProfile` to hook + `UseProfileResult`                                                      |
| `src/components/ProfileActions.tsx`               | Add `canDuplicate`/`onDuplicate` props, render button                                                    |
| `src/components/pages/ProfilesPage.tsx`           | Wire duplicate props to ProfileActions                                                                   |

#### No New Files Required

The feature integrates entirely into existing files and modules.

## UX Considerations

### User Workflows

#### Primary Workflow: Mouse/Keyboard

1. **Trigger**: User has a saved profile loaded, clicks "Duplicate" in ProfileActions (or presses Ctrl+D)
2. **Execution**: System creates `"Profile Name (Copy)"` on disk
3. **Feedback**: Profile selector updates, new profile auto-selected, name field focused with text selected
4. **Editing**: User renames and modifies the duplicate as desired
5. **Completion**: User saves normally

#### Gamepad Workflow: Steam Deck

1. **Trigger**: D-pad to "Duplicate" button, press A
2. **Execution**: Same as above
3. **Feedback**: Profile selector updates, name input gains focus (triggers Steam Input on-screen keyboard)
4. **Editing**: User renames via on-screen keyboard or dismisses with B

#### Error Recovery

| Error               | User Sees                         | Recovery                     |
| ------------------- | --------------------------------- | ---------------------------- |
| Source not found    | Error banner: "Profile not found" | Select a valid profile       |
| Disk write failure  | Error banner: IO error message    | Check disk space/permissions |
| No profile selected | Button is disabled                | Select a profile first       |
| Unsaved changes     | Button is disabled                | Save first                   |

### UI Patterns

| Component        | Pattern                       | Notes                                  |
| ---------------- | ----------------------------- | -------------------------------------- |
| Duplicate button | `crosshook-button--secondary` | Matches Delete styling                 |
| Loading state    | "Duplicating..." button text  | Matches "Saving..."/"Deleting..."      |
| Error display    | `crosshook-error-banner`      | Existing error pattern                 |
| Success feedback | Auto-select + name focus      | No toast needed (visible state change) |

### Terminology

Use **"Duplicate"** (not "Copy" or "Clone"):

- "Copy" implies clipboard semantics
- "Clone" has Git/VM technical connotations
- "Duplicate" clearly communicates "make another one right here" -- used by macOS Finder, Figma, Lutris, Postman

### Accessibility Requirements

- Button must be D-pad navigable (existing `useGamepadNav` handles this)
- Touch target minimum 44x44px (existing button styles meet this)
- Disabled state must be visually distinct and include tooltip explanation

## Recommendations

### Implementation Approach

**Recommended Strategy**: Encapsulate `duplicate()` in `ProfileStore` (crosshook-core), keeping Tauri command thin.

**Rationale**: The CLAUDE.md architecture states "crosshook-core contains all business logic; crosshook-cli and src-tauri are thin consumers." Name generation requires profile listing and validation -- this is business logic, not presentation. Encapsulation also makes the method unit-testable without Tauri and reusable by CLI.

**Phasing:**

1. **Phase 1 - Backend**: `ProfileStore::duplicate()` + name generation + tests (~30 lines of Rust)
2. **Phase 2 - Wiring**: Tauri command + frontend hook + TypeScript type
3. **Phase 3 - UI**: Duplicate button in ProfileActions + page integration
4. **Phase 4 - Verification**: Manual testing (gamepad, conflict resolution, edge cases)

### Technology Decisions

| Decision                     | Recommendation                                | Rationale                                                                |
| ---------------------------- | --------------------------------------------- | ------------------------------------------------------------------------ |
| Where to put business logic  | `ProfileStore::duplicate()` in crosshook-core | Matches workspace separation pattern; testable without Tauri             |
| Name parameter               | `Option<&str>`                                | Auto-generate by default; explicit name supports future "Save As"        |
| Overwrite for explicit names | Allow (match `save()`/`rename()`)             | Consistency with existing API                                            |
| Conflict for auto names      | Never overwrite (loop until unique)           | Auto-generated names must never destroy data                             |
| Return type                  | `DuplicateProfileResult { name, profile }`    | Frontend needs both for select + hydrate                                 |
| New error variant            | Not needed                                    | Loop guarantees uniqueness; explicit names follow existing save behavior |

### Quick Wins to Ship Alongside

- **Auto-select new profile**: `refreshProfiles()` + `loadProfile(result.name)` after duplicate succeeds
- **Focused name field**: Focus profile name input with text selected for immediate inline rename

### Future Enhancements

- **Ctrl+D keyboard shortcut**: Standard duplicate shortcut; verify no conflicts
- **"Duplicate as..."** dialog: Explicit name input before duplicating (uses the existing `new_name` parameter)
- **Template profiles**: Read-only base profiles that can only be duplicated (#42 override layers)
- **Connection to #50**: Optimization presets workflow is: duplicate profile → change only launch optimizations

## Risk Assessment

### Technical Risks

| Risk                                      | Likelihood | Impact | Mitigation                                    |
| ----------------------------------------- | ---------- | ------ | --------------------------------------------- |
| Silent overwrite (save has no guard)      | Medium     | High   | `duplicate()` checks `list()` before `save()` |
| Launcher slug collision                   | Medium     | Medium | Document; do not block duplication            |
| TOCTOU race on conflict check             | Very Low   | Low    | Single-user desktop app; acceptable           |
| Name validation failure on generated name | Very Low   | Low    | `(Copy)` suffix uses only safe characters     |
| Large profile round-trip normalization    | Very Low   | Low    | Machine-generated TOML; no comments to lose   |

### Integration Challenges

- **Launcher associations**: Two profiles with identical game/trainer data produce the same launcher slug. Exporting from both would overwrite. Acceptable for MVP; document in launcher export UI.
- **Autosave timer interaction**: The `useProfile.ts` autosave targets the selected profile name. Auto-selecting the duplicate after creation means autosave correctly targets the new profile.

### Security Considerations

- No new attack surface (internal file copy between user-owned directories)
- Profile names are validated against path traversal via `validate_name()`

## Task Breakdown Preview

### Phase 1: Backend (Rust)

**Focus**: Core duplication logic in crosshook-core
**Tasks**:

- Add `DuplicateProfileResult` struct to `toml_store.rs`
- Add `duplicate()` method to `ProfileStore`
- Add `generate_unique_copy_name()` private method
- Add `strip_copy_suffix()` helper function
- Export `DuplicateProfileResult` from `profile/mod.rs`
- Add unit tests (duplicate basic, conflict increment, suffix stripping, source not found, explicit name)
  **Parallelization**: All tasks are sequential within this phase

### Phase 2: Tauri + Frontend Wiring

**Focus**: Connect backend to frontend
**Dependencies**: Phase 1 complete
**Tasks**:

- Add `profile_duplicate` Tauri command in `commands/profile.rs`
- Register command in `lib.rs` invoke handler
- Add `DuplicateProfileResult` TypeScript interface in `types/profile.ts`
- Add `duplicateProfile` function to `useProfile.ts` hook
  **Parallelization**: Rust command and TypeScript type can be done in parallel

### Phase 3: UI Integration

**Focus**: User-facing button and interactions
**Dependencies**: Phase 2 complete
**Tasks**:

- Add `canDuplicate`/`onDuplicate` props to `ProfileActions.tsx`
- Render "Duplicate" button with disabled state and loading text
- Wire props in `ProfilesPage.tsx`
- Verify gamepad navigation reaches the new button
  **Parallelization**: Component changes and page wiring can be done in parallel

### Phase 4: Verification

**Focus**: End-to-end testing
**Dependencies**: Phase 3 complete
**Tasks**:

- Run `cargo test -p crosshook-core` (verify new + existing tests pass)
- Manual test: duplicate a profile, verify copy loads correctly
- Manual test: duplicate when `(Copy)` exists, verify `(Copy 2)` generated
- Manual test: duplicate a community-imported profile
- Manual test: gamepad navigation to Duplicate button on Steam Deck

## Decisions Needed

Before proceeding to implementation planning, clarify:

1. **Name generation location**
   - Options: (A) `ProfileStore::duplicate()` in crosshook-core, (B) Helper in Tauri command layer
   - Impact: A matches workspace separation ("core has all business logic"); B is leaner but leaks logic
   - Recommendation: **Option A** -- name generation requires `list()` and `validate_name()`, making it business logic

2. **Post-duplicate focus behavior**
   - Options: (A) Auto-focus name field for rename, (B) Focus stays on Duplicate button
   - Impact: A enables immediate rename (especially useful on Steam Deck with on-screen keyboard); B is simpler
   - Recommendation: **Option A** for MVP -- matches macOS Finder, VS Code, JetBrains patterns

3. **Dirty profile handling**
   - Options: (A) Disable Duplicate when unsaved changes exist, (B) Duplicate the saved-on-disk state ignoring dirty state
   - Impact: A is clearer about what is being duplicated; B is more permissive but could confuse users
   - Recommendation: **Option A** -- avoids ambiguity about "which version am I duplicating?"

## Research References

For detailed findings, see:

- [research-external.md](./research-external.md): Tauri IPC patterns, TOML operations, filesystem safety, name generation algorithms
- [research-business.md](./research-business.md): User stories, 12 business rules, workflows, domain model, codebase integration
- [research-technical.md](./research-technical.md): Architecture, data models, API design, frontend integration, test cases
- [research-ux.md](./research-ux.md): Competitive analysis (Lutris, VS Code, JetBrains, Figma), gamepad UX, terminology
- [research-recommendations.md](./research-recommendations.md): Implementation approach comparison, risk assessment, task breakdown
