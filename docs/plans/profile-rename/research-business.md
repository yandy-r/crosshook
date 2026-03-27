# Profile Rename — Business Logic & Requirements Research

## Executive Summary

Profile rename is a gap in the current UX: changing a profile's name in the text field and saving creates a **new** profile, leaving the old one behind. The backend already has `ProfileStore::rename()` and a registered `profile_rename` Tauri command — the missing piece is frontend integration and cascading side-effect handling (settings, launchers). Complexity is low-medium; the primary risk is silent data loss from filesystem-level overwrites when the target name collides with an existing profile.

## User Stories

### US-1: Rename an Existing Profile

**As a** CrossHook user, **I want to** rename a saved profile **so that** I can correct typos, update game versions, or reorganize my library without losing my launch configuration.

**Acceptance:** Renaming updates the filename, the profile list, the last-used-profile setting, and optionally cascades to exported launchers. The old profile file no longer exists.

### US-2: Rename Without Data Loss

**As a** CrossHook user, **I want to** be warned before overwriting another profile's name **so that** I don't accidentally lose a different profile's configuration.

**Acceptance:** If the new name matches an existing profile (other than the current one), the user is prompted for confirmation before proceeding.

### US-3: Rename Preserves Launch State

**As a** CrossHook user, **I want** my exported launchers and auto-load settings to stay valid after renaming **so that** I don't have to reconfigure desktop shortcuts or startup behavior.

**Acceptance:** `last_used_profile` is updated. Exported launcher files remain functional (launcher paths derive from display_name, not profile name, so this is inherently safe unless display_name changes simultaneously).

### US-4: Seamless In-Place Editing

**As a** CrossHook user, **I want** the rename to feel as lightweight as a normal save **so that** I don't need a separate rename dialog or multiple confirmation steps for the common case.

**Acceptance:** When the profile name text field changes and the user saves, the system detects rename intent and handles it automatically (rename old file, save new content).

## Business Rules

### Core Rules

1. **Profile name = filename**: The profile name is the TOML file stem in `~/.config/crosshook/profiles/`. Renaming a profile means `fs::rename(old.toml, new.toml)`. The name is NOT stored inside the TOML content.

2. **Name validation**: Both old and new names must pass `validate_name()`:
   - Non-empty, not `.` or `..`
   - No path separators (`/`, `\`)
   - No Windows-reserved characters: `< > : " | ? *`
   - Not an absolute path

3. **Same-name no-op**: If old name equals new name after trimming, rename is a no-op (returns `Ok(())`).

4. **Atomic rename**: `fs::rename()` is atomic on the same filesystem. Since all profiles live in the same directory, rename cannot produce partial states.

5. **Settings cascade**: If `last_used_profile` in `settings.toml` matches the old name, it must be updated to the new name. Failure to update causes the auto-load-on-startup feature to silently skip (not crash — it returns `None`).

6. **Launcher independence**: Launcher file paths derive from `steam.launcher.display_name`, not from the profile name. Renaming a profile does NOT inherently require renaming launchers. However, if a rename is accompanied by a display_name change, launcher rename should be offered or cascaded.

7. **Best-effort side effects**: Following the pattern established by `profile_delete`, cascading updates (settings, launchers) should be best-effort. If they fail, the profile rename itself should still succeed.

### Edge Cases

1. **Collision with existing profile**: `fs::rename()` silently overwrites on Linux. The frontend MUST check `profiles.includes(newName)` before calling rename and prompt for confirmation if a collision would occur.

2. **Rename during autosave**: The launch optimizations autosave runs on a 350ms debounce using the current `profileName` from state. If rename fires while an autosave is pending, the autosave could target the old filename. The rename flow should cancel any pending autosave timer.

3. **Rename + content changes**: When the user changes both the name AND the profile data, the operation should: (a) rename the file, (b) then save the updated content to the new filename. This ensures the old file is removed and the new file has current data.

4. **Empty profile name after edit**: Already handled by `persistProfileDraft` validation, but the rename path should also validate early.

5. **Concurrent rename of same profile**: Not a realistic scenario in a single-user desktop app but `fs::rename()` atomicity handles it safely.

6. **Profile loaded from community tap then renamed**: Community profiles are imported by content, not by name reference. No community tap tracks local profile names. Rename is fully safe.

7. **Rename to name with different casing**: Linux filesystems are case-sensitive. "Elden Ring" and "elden ring" are different filenames. This works correctly with no special handling.

## Workflows

### Primary Workflow: Rename via Name Field Edit

```
1. User loads profile "Old Name" → selectedProfile = "Old Name", profileName = "Old Name"
2. User edits name field to "New Name" → profileName = "New Name", dirty = true
3. User clicks Save
4. Frontend detects rename intent: selectedProfile ("Old Name") !== profileName ("New Name") AND selectedProfile is non-empty
5. Frontend checks collision: profiles.includes("New Name")
   a. If collision → show confirmation: "A profile named 'New Name' already exists. Replace it?"
   b. If no collision → proceed
6. Frontend calls profile_rename(old_name: "Old Name", new_name: "New Name")
7. Frontend calls profile_save(name: "New Name", data: normalizedProfile) — saves updated content
8. Frontend updates last_used_profile setting to "New Name"
9. Frontend refreshes profile list
10. Frontend auto-selects "New Name"
```

### Error Recovery

- **Rename fails (validation error)**: Display error banner, do not modify state. User corrects name and retries.
- **Rename fails (IO error)**: Display error, profile remains at old name. No partial state possible due to atomicity.
- **Save after rename fails**: Profile has been renamed but content not saved. This is acceptable — the old content under the new name is still valid. Error banner tells user to retry save.
- **Settings update fails**: Profile rename succeeds but auto-load reference is stale. Non-critical — log warning, user can still select profile manually.

### Alternative Workflow: New Profile Creation (unchanged behavior)

When `selectedProfile` is empty (no profile loaded from backend), saving with a new name should continue to create a new profile — NOT attempt a rename. This preserves the existing "create from scratch" flow.

## Domain Model

### Entities

| Entity           | Identity          | Storage                                                | References                                                 |
| ---------------- | ----------------- | ------------------------------------------------------ | ---------------------------------------------------------- |
| GameProfile      | filename stem     | `~/.config/crosshook/profiles/{name}.toml`             | settings.last_used_profile, in-memory state                |
| AppSettings      | singleton         | `~/.config/crosshook/settings.toml`                    | GameProfile by name (last_used_profile)                    |
| ExportedLauncher | display_name slug | `~/.local/share/crosshook/launchers/{slug}-trainer.sh` | GameProfile by content (display_name), NOT by profile name |
| RecentFiles      | singleton         | `~/.local/share/crosshook/recent.toml`                 | File paths only, no profile name references                |
| CommunityProfile | tap URL + path    | Git repositories                                       | Standalone; no local profile name tracking                 |

### State Transitions

```
[Saved Profile "A"]
    → user edits name to "B"
    → [Dirty Profile, rename intent detected]
    → save triggered
    → [if "B" exists: Pending Overwrite Confirmation]
    → rename "A.toml" → "B.toml"
    → save content to "B.toml"
    → update settings.last_used_profile if needed
    → [Saved Profile "B"]
```

## Existing Codebase Integration

### Backend (already implemented)

| Component                      | Status | Notes                                                                                   |
| ------------------------------ | ------ | --------------------------------------------------------------------------------------- |
| `ProfileStore::rename()`       | Done   | `toml_store.rs:163-178` — validates names, handles same-name no-op, does `fs::rename()` |
| `profile_rename` Tauri command | Done   | `commands/profile.rs:148-154` — thin wrapper, registered in `lib.rs:96`                 |
| `validate_name()`              | Done   | `toml_store.rs:273-298` — shared validation for all profile operations                  |

### Frontend (needs implementation)

| Component                  | Status                    | What's Needed                                                                                    |
| -------------------------- | ------------------------- | ------------------------------------------------------------------------------------------------ |
| `useProfile` hook          | Exists, needs rename flow | Add `renameProfile` method or modify `saveProfile`/`persistProfileDraft` to detect rename intent |
| `ProfileActions` component | Exists                    | May need "Rename" affordance or save button should handle rename transparently                   |
| `ProfileFormSections`      | Exists                    | Name field already supports editing; no changes needed                                           |
| `ProfileContext`           | Exists                    | Will propagate new hook methods automatically                                                    |

### Cascade Infrastructure (exists, needs wiring)

| Component                             | Status       | Notes                                                                                                                              |
| ------------------------------------- | ------------ | ---------------------------------------------------------------------------------------------------------------------------------- |
| Settings update (`last_used_profile`) | Needs wiring | `syncProfileMetadata` in `useProfile.ts` already updates `last_used_profile` on save — rename needs same                           |
| Launcher rename                       | Exists       | `rename_launcher_files()` in `launcher_store.rs` and `rename_launcher` Tauri command exist but are not triggered by profile rename |
| Launch optimization autosave          | Needs guard  | Timer should be cancelled before rename to prevent writing to old filename                                                         |

## Success Criteria

1. Renaming a profile removes the old TOML file and creates the new one with updated content
2. Profile list refreshes and auto-selects the renamed profile
3. `last_used_profile` setting is updated if applicable
4. Overwriting an existing profile requires explicit user confirmation
5. The operation feels as lightweight as a normal save (no extra dialogs for the common case)
6. Existing tests pass; new tests cover rename + save, rename + collision, rename + settings cascade

## Open Questions

1. **Should rename + content change be a single user action?** Current analysis assumes yes (edit name, edit fields, click Save = rename + save). Alternative: separate Rename button. Recommendation: single action to match user mental model.

2. **Should the backend `profile_rename` command handle settings cascade?** Currently it's a thin wrapper. The `profile_delete` command handles launcher cleanup server-side. Should `profile_rename` do the same for settings? Recommendation: yes, for consistency — accept optional `SettingsStore` state and update `last_used_profile` within the command.

3. **Should overwriting be blocked at the backend level?** Currently `fs::rename()` overwrites silently (by design — test confirms). Should we add a `force: bool` parameter or a separate `profile_exists` check command? Recommendation: keep backend behavior, add frontend guard — matches the pattern used by `duplicate` which checks existing names client-side.

4. **Should the Rename button be a separate UI action or integrated into Save?** The duplicate and delete flows have dedicated buttons. Rename could either be a new button or be detected automatically when the name diverges during save. Recommendation: automatic detection during save — it's the most natural UX and avoids cluttering the action bar.
