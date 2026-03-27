# Feature Spec: Profile Rename

## Executive Summary

Profile rename closes a UX gap where editing a profile's name and saving creates a duplicate instead of renaming. The backend is ~90% complete — `ProfileStore::rename()`, `profile_rename` Tauri command, and `rename_launcher_files()` all exist. The remaining work is adding overwrite protection (`AlreadyExists` error), cascading `last_used_profile` in the Tauri command, and building a frontend rename flow with a dedicated Rename button and modal dialog. No new dependencies; ~75 lines across 6 files.

## External Dependencies

### APIs and Services

#### Rust `std::fs::rename`

- **Documentation**: [std::fs::rename](https://doc.rust-lang.org/std/fs/fn.rename.html)
- **Behavior**: Maps to POSIX `rename(2)` — atomic on the same filesystem
- **Already used**: `toml_store.rs:176`
- **Key constraint**: Silently overwrites target if it exists (requires frontend/backend guard)

#### Tauri v2 IPC

- **Documentation**: [Tauri v2 State Management](https://v2.tauri.app/develop/state-management/)
- **Pattern**: Backend operations via `#[tauri::command]` invoked from React via `invoke()`
- **Already configured**: `profile_rename` registered in `lib.rs:96`

### Libraries and SDKs

No new dependencies required. All primitives exist in the codebase:

| Library    | Version | Purpose                         | Status                   |
| ---------- | ------- | ------------------------------- | ------------------------ |
| `serde`    | 1.x     | Serialize/Deserialize IPC types | Already in use           |
| `toml`     | 0.8     | Profile TOML parsing            | Already in use           |
| `tracing`  | 0.1     | Structured logging              | Already in use           |
| `tempfile` | 3.x     | Test infrastructure             | Already a dev dependency |

**Evaluated and rejected**: `renamore` (atomic no-clobber rename via `renameat2`) — overkill for a single-user desktop app where a simple `path.exists()` check is sufficient.

### External Documentation

- [rename(2) man page](https://man7.org/linux/man-pages/man2/rename.2.html): POSIX atomicity guarantees
- [Tauri v2 Calling Frontend](https://v2.tauri.app/develop/calling-frontend/): Event system (not needed for initial implementation)

## Business Requirements

### User Stories

**Primary User: CrossHook user managing game profiles**

- **US-1**: As a user, I want to rename a saved profile so that I can correct typos, update game versions, or reorganize my library without losing my launch configuration.
- **US-2**: As a user, I want to be warned before overwriting another profile's name so that I don't accidentally lose a different profile's configuration.
- **US-3**: As a user, I want my auto-load settings to stay valid after renaming so that I don't have to reconfigure startup behavior.
- **US-4**: As a user, I want rename to be a clear, explicit action so that I don't accidentally create duplicate profiles when I just want to change a name.

### Business Rules

1. **Profile name = filename**: The profile name is the TOML file stem in `~/.config/crosshook/profiles/`. The name is NOT stored inside the TOML content.
   - Validation: Both old and new names must pass `validate_name()` — non-empty, no path separators, no reserved characters (`< > : " | ? *`)

2. **Same-name no-op**: If old name equals new name after trimming, rename returns `Ok(())` silently.

3. **Atomic rename**: `fs::rename()` is atomic on the same filesystem. All profiles live in the same directory, so rename cannot produce partial states.

4. **Settings cascade**: If `last_used_profile` matches the old name, it must be updated to the new name. Best-effort — failure is logged but does not fail the rename.

5. **Launcher independence**: Launcher file paths derive from `steam.launcher.display_name`, NOT the profile name. Profile rename does NOT cascade to launchers.

6. **No overwrite without error**: Renaming to an existing profile name must return `AlreadyExists` error. The current implementation silently overwrites — this must be fixed.

### Edge Cases

| Scenario                                       | Expected Behavior                                            | Notes                                                                |
| ---------------------------------------------- | ------------------------------------------------------------ | -------------------------------------------------------------------- |
| Name collision with existing profile           | Return `AlreadyExists` error                                 | Frontend checks profile list; backend adds `new_path.exists()` guard |
| Rename + content changes simultaneously        | Rename file first, then save updated content to new filename | Two sequential IPC calls                                             |
| Empty name after edit                          | Blocked by inline validation in modal                        | `validate_name()` catches server-side                                |
| Case-only change (e.g., "Game" to "game")      | Works correctly on Linux (case-sensitive)                    | macOS edge case deferred                                             |
| Profile loaded from community tap then renamed | Safe — community taps don't track local profile names        | No special handling                                                  |
| Autosave race during rename                    | Cancel pending autosave timer before rename                  | Launch optimizations autosave uses 350ms debounce                    |

### Success Criteria

- [ ] Renaming a profile removes the old TOML file and the new file has current content
- [ ] Profile list refreshes and auto-selects the renamed profile
- [ ] `last_used_profile` setting is updated when applicable
- [ ] Renaming to an existing name shows an error (not silent overwrite)
- [ ] The profile name field is read-only for existing profiles (rename via dedicated button only)
- [ ] Existing tests pass; new tests cover rename + collision, rename + settings cascade

## Technical Specifications

### Architecture Overview

```text
Frontend (React)
  ProfileActions.tsx ─── [Rename] button ───> Rename Modal Dialog
                                                    │
                                              useProfile.ts
                                              renameProfile()
                                                    │ invoke()
                                                    ▼
Tauri Command Layer
  commands/profile.rs::profile_rename(old_name, new_name)
    1. store.rename()           ← file rename + overwrite protection
    2. settings.last_used_profile update  ← best-effort cascade
                    │
crosshook-core
  ProfileStore::rename()        SettingsStore
    validate_name(old)            load() → update
    validate_name(new)            last_used_profile
    check old exists              → save()
    check new !exists (NEW)
    fs::rename(old, new)
```

### Data Models

#### Profile TOML File

Profile name is the filename, NOT stored inside the TOML:

```text
~/.config/crosshook/profiles/
  ├── Elden Ring.toml          ← profile name: "Elden Ring"
  ├── Cyberpunk 2077.toml      ← profile name: "Cyberpunk 2077"
```

Rename operation: `fs::rename("Elden Ring.toml", "New Name.toml")` — file contents unchanged.

#### Settings Impact

```toml
# ~/.config/crosshook/settings.toml
auto_load_last_profile = true
last_used_profile = "Elden Ring"    # must update on rename
```

#### No Impact

- `recent.toml`: Stores file paths (game, trainer, dll), not profile names
- Exported launchers: Derive paths from `display_name`, not profile name
- Community taps: Reference source repos, not local profile names

### API Design

#### Enhanced `profile_rename` Tauri IPC Command

**Purpose**: Rename a profile and cascade to settings

**Request (TypeScript)**:

```ts
await invoke('profile_rename', { oldName: 'Old Name', newName: 'New Name' });
```

**Response**: `void` on success

**Errors**:

| Error                 | Condition                                 | User Message                              |
| --------------------- | ----------------------------------------- | ----------------------------------------- |
| `InvalidName`         | Name contains forbidden chars or is empty | "invalid profile name: {name}"            |
| `NotFound`            | Old profile doesn't exist on disk         | "profile file not found: {path}"          |
| `AlreadyExists` (NEW) | Target name already taken                 | "a profile named '{name}' already exists" |
| `Io`                  | File system error during rename           | OS error message                          |

#### New Error Variant

```rust
pub enum ProfileStoreError {
    // ... existing variants ...
    AlreadyExists(String),  // NEW
}

// Display: "a profile named '{name}' already exists"
```

#### Enhanced `ProfileStore::rename()`

```rust
pub fn rename(&self, old_name: &str, new_name: &str) -> Result<(), ProfileStoreError> {
    let old_name = old_name.trim();
    let new_name = new_name.trim();
    validate_name(old_name)?;
    validate_name(new_name)?;
    if old_name == new_name { return Ok(()); }
    let old_path = self.profile_path(old_name)?;
    let new_path = self.profile_path(new_name)?;
    if !old_path.exists() { return Err(ProfileStoreError::NotFound(old_path)); }
    if new_path.exists() { return Err(ProfileStoreError::AlreadyExists(new_name.to_string())); }
    fs::rename(&old_path, &new_path)?;
    Ok(())
}
```

#### Enhanced Tauri Command

```rust
#[tauri::command]
pub fn profile_rename(
    old_name: String,
    new_name: String,
    store: State<'_, ProfileStore>,
    settings_store: State<'_, SettingsStore>,
) -> Result<(), String> {
    store.rename(&old_name, &new_name).map_err(map_error)?;
    // Best-effort settings cascade
    if let Ok(mut settings) = settings_store.load() {
        if settings.last_used_profile.trim() == old_name.trim() {
            settings.last_used_profile = new_name.trim().to_string();
            if let Err(err) = settings_store.save(&settings) {
                tracing::warn!(%err, old_name, new_name, "settings update after profile rename failed");
            }
        }
    }
    Ok(())
}
```

### System Integration

#### Files to Modify

| File                                              | Change                                                                       | Scope     |
| ------------------------------------------------- | ---------------------------------------------------------------------------- | --------- |
| `crates/crosshook-core/src/profile/toml_store.rs` | Add `AlreadyExists` variant, overwrite protection in `rename()`, update test | ~20 lines |
| `src-tauri/src/commands/profile.rs`               | Add `SettingsStore` state param, cascade `last_used_profile`                 | ~15 lines |
| `src/hooks/useProfile.ts`                         | Add `renameProfile()`, `renaming` state, rename modal state                  | ~30 lines |
| `src/components/ProfileActions.tsx`               | Add Rename button, `renaming` disable state                                  | ~5 lines  |
| `src/context/ProfileContext.tsx`                  | Pass through `renaming` and `renameProfile`                                  | ~2 lines  |
| `src/components/pages/ProfilesPage.tsx`           | Wire `renaming` state                                                        | ~3 lines  |

#### Files NOT Modified

| File                                                 | Reason                                    |
| ---------------------------------------------------- | ----------------------------------------- |
| `src-tauri/src/lib.rs`                               | `profile_rename` already registered       |
| `crates/crosshook-core/src/profile/models.rs`        | Profile data model unchanged              |
| `crates/crosshook-core/src/export/launcher_store.rs` | Launcher cascade NOT needed               |
| `src-tauri/src/startup.rs`                           | Works automatically once settings updated |
| `src/types/profile.ts`                               | No new types needed (rename returns void) |

## UX Considerations

### User Workflows

#### Primary Workflow: Rename via Dedicated Button

1. **Initiate**: User selects an existing profile and clicks "Rename" button in ProfileActions (or presses F2)
2. **Modal opens**: Compact dialog with current name pre-filled and fully selected
3. **Edit**: User types new name; inline validation runs on debounced input (300ms)
4. **Confirm**: Enter key, "Rename" button click, or gamepad A button
5. **Backend**: `profile_rename` IPC call — rename file + cascade settings
6. **Success**: Dialog closes, profile list refreshes, new name selected, success toast with Undo

#### Error Recovery

- **Name conflict**: Inline error "A profile named 'X' already exists" — user must choose different name
- **Invalid characters**: Inline error blocks submission — user corrects name
- **Backend failure**: Dialog stays open with error, user can retry or cancel
- **Settings cascade failure**: Non-critical — logged, profile still renamed successfully

### UI Patterns

| Component          | Pattern                                                     | Notes                                                   |
| ------------------ | ----------------------------------------------------------- | ------------------------------------------------------- |
| Rename trigger     | Dedicated button in ProfileActions                          | Between Duplicate and Delete                            |
| Rename dialog      | Lightweight modal (follows `PendingDelete` overlay pattern) | Focus trapping, ESC to cancel                           |
| Name input         | Pre-filled + fully selected text                            | Universal rename convention                             |
| Validation         | Debounced 300ms for local checks, blur for conflict         | Inline error below input                                |
| Success feedback   | Toast with Undo button (5-8s window)                        | NNGroup recommendation                                  |
| Profile name field | Read-only for existing profiles                             | Eliminates root cause of "rename creates duplicate" bug |

### Accessibility Requirements

- **Keyboard**: Enter to confirm, Escape to cancel, F2 to open rename dialog, Tab between controls
- **ARIA**: `role="dialog"`, `aria-modal="true"`, `aria-labelledby`, `role="alert"` for errors
- **Gamepad**: A to confirm, B to cancel, D-pad navigation; dialog positioned above virtual keyboard overlay
- **Touch targets**: Minimum 44px for Steam Deck (7" 1280x800 display)

### Performance UX

- **Loading States**: Near-instant operation (single `fs::rename` syscall) — no spinner needed, brief button disable to prevent double-click
- **Optimistic Updates**: Not needed — operation completes before any spinner would render
- **Error Feedback**: Inline validation immediate; backend errors displayed in-dialog

## Recommendations

### Implementation Approach

**Recommended Strategy**: Atomic `fs::rename` + cascading side effects in a single Tauri command, with a dedicated frontend Rename button and modal dialog.

**Phasing**:

1. **Phase 1 - Backend Orchestration**: Add `AlreadyExists` error, overwrite protection, settings cascade in Tauri command. Update tests.
2. **Phase 2 - Frontend Integration**: Add `renameProfile()` to `useProfile.ts`, Rename button in `ProfileActions.tsx`, rename modal dialog, make name field read-only for existing profiles.
3. **Phase 3 - UX Polish**: Undo toast, F2 keyboard shortcut, gamepad optimizations, save flow disambiguation fallback.

### Technology Decisions

| Decision                  | Recommendation                                    | Rationale                                                                    |
| ------------------------- | ------------------------------------------------- | ---------------------------------------------------------------------------- |
| Overwrite protection      | `AlreadyExists` error in `ProfileStore::rename()` | Prevents silent data loss; simple `path.exists()` guard                      |
| Settings cascade location | In Tauri command (single IPC call)                | Matches `profile_delete` pattern; avoids partial failure from multiple calls |
| Launcher cascade          | Not needed                                        | Launchers derive from `display_name`, not profile name                       |
| Frontend integration      | Dedicated Rename button + modal                   | Unambiguous intent; gamepad-friendly; avoids overloading Save                |
| Name field behavior       | Read-only for existing profiles                   | Eliminates root cause of "edit name + save = new profile" bug                |

### Quick Wins

- Backend `profile_rename` command already registered — only needs `SettingsStore` param and cascade logic
- `ProfileStore::rename()` exists and is tested — only needs `AlreadyExists` guard
- `duplicateProfile` pattern in `useProfile.ts` can be directly followed for `renameProfile`
- `LauncherRenameResult` TypeScript type already exists (though not needed for this feature)

### Future Enhancements

- **Stable profile IDs**: Add UUID inside TOML to decouple identity from filename (larger refactor)
- **CLI rename**: Add `rename` subcommand to `crosshook-cli` (core logic already in `crosshook-core`)
- **Batch rename**: Pattern-based rename for users with many profiles
- **Save flow disambiguation**: Detect `profileName !== selectedProfile` on save and offer rename vs. save-as-new choice

## Risk Assessment

### Technical Risks

| Risk                                      | Likelihood | Impact | Mitigation                                                                      |
| ----------------------------------------- | ---------- | ------ | ------------------------------------------------------------------------------- |
| Silent overwrite of existing profile      | Medium     | High   | Add `AlreadyExists` error + `path.exists()` check before `fs::rename`           |
| Settings desync (`last_used_profile`)     | Low        | Medium | Update in same Tauri command; best-effort with warning log                      |
| Frontend state inconsistency after rename | Medium     | Medium | Follow `duplicateProfile` pattern: `refreshProfiles()` + `loadProfile(newName)` |
| Autosave race condition                   | Low        | Low    | Cancel pending debounce timer before invoking rename                            |
| Concurrent rename of same profile         | Low        | Low    | Single-user desktop app; `fs::rename` atomicity handles it                      |

### Integration Challenges

- **`useProfile.ts` state management**: Must update `selectedProfile`, `profileName`, and `profileExists` atomically after rename. The existing `loadProfile` callback handles this correctly.
- **Profile name field dual purpose**: Currently used for both create and edit. Making it read-only for existing profiles eliminates ambiguity — Rename button and Duplicate button cover the two legitimate name-change use cases.

### Security Considerations

- **Path traversal**: `validate_name()` already rejects `/`, `\`, `..`, and reserved characters for both old and new names
- **Symlink attacks**: Acceptable risk — profiles in user-owned directory
- **No privilege escalation**: All operations are user-space file operations

## Task Breakdown Preview

### Phase 1: Backend Orchestration

**Focus**: Safe rename with cascading side effects
**Tasks**:

- Add `AlreadyExists` variant to `ProfileStoreError` with `Display` impl
- Add `new_path.exists()` check to `ProfileStore::rename()`
- Enhance `profile_rename` Tauri command with `SettingsStore` param and `last_used_profile` cascade
- Update `test_rename_overwrites_existing_target_profile` to expect `AlreadyExists` error
- Add test for settings cascade on rename

**Parallelization**: All backend changes can be developed together; tests run after code changes.

### Phase 2: Frontend Integration

**Focus**: Rename UI and hook integration
**Dependencies**: Phase 1 must be complete
**Tasks**:

- Add `renameProfile()` function and `renaming` state to `useProfile.ts`
- Add Rename button to `ProfileActions.tsx` (enabled when `profileExists && !saving && !deleting && !duplicating`)
- Create rename modal dialog component (pre-filled input, inline validation, focus trapping)
- Make profile name field read-only for existing profiles in `ProfileFormSections.tsx`
- Wire through `ProfileContext.tsx` and `ProfilesPage.tsx`

**Parallelization**: Modal component can be built independently from hook changes.

### Phase 3: UX Polish

**Focus**: Enhanced user experience
**Dependencies**: Phase 2 must be complete
**Tasks**:

- Success toast with Undo button (5-8s window, reverse rename on undo)
- F2 keyboard shortcut to open rename dialog
- Gamepad-optimized dialog layout (accommodate virtual keyboard overlay)
- Save flow disambiguation fallback (optional)

## Decisions Needed

1. **Overwrite policy**
   - Options: (A) Block with `AlreadyExists` error, (B) Add `force` parameter, (C) Keep silent overwrite
   - Impact: Option A is simplest and safest; Option B adds complexity for no current use case
   - Recommendation: **Option A** — block overwrites, user can delete target first if needed

2. **Rename trigger UX**
   - Options: (A) Dedicated Rename button + modal, (B) Detect name change on save, (C) Both
   - Impact: Option A is unambiguous; Option B may confuse users about rename vs. save-as-new
   - Recommendation: **Option A** for Phase 1-2; Option B as optional Phase 3 enhancement

3. **Profile name field editability**
   - Options: (A) Read-only for existing profiles, (B) Editable with disambiguation on save
   - Impact: Option A eliminates the root cause bug; Option B preserves "save as new" workflow (which Duplicate already covers)
   - Recommendation: **Option A** — read-only, with Rename and Duplicate buttons covering name changes

4. **`game.name` sync on rename**
   - Options: (A) Leave independent (current behavior), (B) Update `game.name` to match new profile name
   - Impact: `game.name` is the game's display name, not the profile identity. They're conceptually different.
   - Recommendation: **Option A** — keep them independent

## Research References

For detailed findings, see:

- [research-external.md](./research-external.md): Library evaluation (`std::fs::rename` vs `renamore`, atomicity guarantees)
- [research-business.md](./research-business.md): Business logic analysis, domain model, existing codebase integration
- [research-technical.md](./research-technical.md): Architecture design, API contracts, cross-team synthesis
- [research-ux.md](./research-ux.md): Competitive analysis, accessibility patterns, gamepad/Steam Deck considerations
- [research-recommendations.md](./research-recommendations.md): Implementation phasing, risk assessment, alternative approaches
