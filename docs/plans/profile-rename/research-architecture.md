# Architecture Research: Profile Rename

The profile-rename feature touches a clean vertical slice through the stack: Rust store layer, Tauri command layer, React hook layer, and UI components. The existing `rename()` method and `profile_rename` command provide 80% of the backend; the remaining work is adding overwrite protection, settings cascade, and frontend integration.

## Relevant Components

### Backend (crosshook-core)

- `crates/crosshook-core/src/profile/toml_store.rs`: **ProfileStore** — central profile CRUD. `rename()` at line 163 does `validate_name` + `fs::rename` but lacks `AlreadyExists` guard (line 176 silently overwrites). `ProfileStoreError` enum at line 16 needs an `AlreadyExists(String)` variant. The `duplicate()` method at line 213 is the closest pattern reference.
- `crates/crosshook-core/src/profile/models.rs`: **GameProfile** struct — the profile data model. Profile name is NOT stored inside the TOML; it's the filename stem. No changes needed.
- `crates/crosshook-core/src/settings/mod.rs`: **SettingsStore** + **AppSettingsData** — `last_used_profile: String` at line 23 must be updated when a profile is renamed. `SettingsStore::load()` and `save()` are simple TOML read/write.
- `crates/crosshook-core/src/export/launcher_store.rs`: Launcher paths derive from `steam.launcher.display_name`, NOT profile name. **No cascade needed** for rename.

### Tauri Command Layer (src-tauri)

- `src-tauri/src/commands/profile.rs`: **profile_rename** command at line 148 — currently takes `ProfileStore` state only. Needs `SettingsStore` state param added for `last_used_profile` cascade. Pattern: follow `profile_delete` (line 113) which does best-effort launcher cleanup before deletion.
- `src-tauri/src/commands/settings.rs`: **settings_load/settings_save** commands — frontend can also cascade settings via IPC, but the feature spec recommends doing it in the Tauri command for atomicity.
- `src-tauri/src/lib.rs`: **Command registration** — `profile_rename` already registered at line 96. No changes needed.
- `src-tauri/src/startup.rs`: **Auto-load** — reads `last_used_profile` from settings and emits `auto-load-profile` event. Works automatically once settings are updated on rename. No changes needed.

### Frontend Hooks

- `src/hooks/useProfile.ts`: **useProfile hook** — the state management center. Key state: `profiles`, `selectedProfile`, `profileName`, `saving`, `deleting`, `duplicating`, `profileExists`. The `duplicateProfile()` at line 569 is the exact pattern to follow for `renameProfile()`: set in-flight flag, invoke IPC, refreshProfiles, loadProfile(newName). The `launchOptimizationsAutosaveTimerRef` at line 335 has a 350ms debounce timer that should be cancelled before rename to avoid race conditions.
- `src/context/ProfileContext.tsx`: **ProfileContext** — thin passthrough of `UseProfileResult` plus derived values (`launchMethod`, `steamClientInstallPath`). New `renameProfile` and `renaming` will flow through automatically via the spread (`...profileState`) at line 53.

### Frontend Components

- `src/components/ProfileActions.tsx`: **Action bar** — Save, Duplicate, Delete buttons. Rename button goes between Duplicate and Delete. Props interface at line 8 needs `canRename`, `renaming`, `onRename`.
- `src/components/ProfileFormSections.tsx`: **Profile name input** — line 323, currently a plain `<input>` that's always editable. For existing profiles this should become read-only to prevent the "edit name + save = new profile" bug.
- `src/components/pages/ProfilesPage.tsx`: **Page orchestrator** — wires ProfileFormSections, ProfileActions, and the delete overlay dialog. The delete overlay pattern (line 179) is the template for the rename modal dialog. `canDuplicate` guard at line 70 shows the pattern for `canRename`.

### TypeScript Types

- `src/types/profile.ts`: **GameProfile**, **DuplicateProfileResult** — no new types needed (rename returns void).
- `src/types/launcher.ts`: **LauncherRenameResult** — exists but NOT needed for profile rename (launchers are independent).
- `src/types/settings.ts`: **AppSettingsData** — `last_used_profile: string` mirrors the Rust struct.

## Data Flow

### Current Profile Load/Save Flow

```
User selects profile in UI
  → ProfilesPage.selectProfile(name)
    → useProfile.loadProfile(name)
      → invoke('profile_load', { name })
        → ProfileStore::load() reads ~/.config/crosshook/profiles/{name}.toml
      → syncProfileMetadata(name, profile)
        → invoke('settings_save', { last_used_profile: name })
      → setSelectedProfile(name), setProfileName(name), setProfile(data)
```

### Current Save Flow (creates duplicate bug)

```
User edits profileName in UI input → setProfileName("New Name")
User clicks Save
  → useProfile.saveProfile()
    → persistProfileDraft(profileName, profile)
      → invoke('profile_save', { name: "New Name", data: profile })
        → ProfileStore::save() writes ~/.config/crosshook/profiles/New Name.toml
      → Old file still exists → DUPLICATE created
```

### Proposed Rename Flow

```
User clicks Rename button
  → Rename modal opens with current name pre-filled
  → User edits name, clicks Confirm
    → useProfile.renameProfile(oldName, newName)
      → invoke('profile_rename', { oldName, newName })
        → ProfileStore::rename() — validate, check !exists, fs::rename
        → SettingsStore cascade — update last_used_profile if matched
      → refreshProfiles()
      → loadProfile(newName)
```

## Integration Points

### 1. ProfileStoreError::AlreadyExists (NEW)

Add variant to `ProfileStoreError` enum in `toml_store.rs:16`. Add `Display` impl line. Add `new_path.exists()` check before `fs::rename()` in `rename()` method at line 176.

### 2. Tauri Command Enhancement

In `commands/profile.rs:148`, add `settings_store: State<'_, SettingsStore>` parameter. After successful `store.rename()`, do best-effort `last_used_profile` update (load → compare → save). Pattern matches `profile_delete` line 113 (best-effort launcher cleanup).

### 3. useProfile Hook Extension

Add to `useProfile.ts`:

- `renaming: boolean` state (like `duplicating` at line 329)
- `renameProfile(oldName: string, newName: string): Promise<void>` callback (follow `duplicateProfile` pattern at line 569)
- Cancel `launchOptimizationsAutosaveTimerRef` before rename to prevent race

### 4. ProfileActions Button

In `ProfileActions.tsx`, add Rename button between Duplicate and Delete. New props: `canRename`, `renaming`, `onRename`.

### 5. Rename Modal Dialog

In `ProfilesPage.tsx`, add rename modal following the `pendingDelete` overlay pattern (line 179). State: `pendingRename: { oldName: string } | null`. Input pre-filled with current name, inline validation, Enter/Escape keyboard handling.

### 6. Read-Only Name Field

In `ProfileFormSections.tsx:323`, add `readOnly` attribute when editing an existing profile. This eliminates the root cause bug.

## Key Dependencies

| Component                      | Depends On                                             | Notes                                       |
| ------------------------------ | ------------------------------------------------------ | ------------------------------------------- |
| `ProfileStore::rename()`       | `validate_name()`, `fs::rename`                        | Already exists; needs `AlreadyExists` guard |
| `profile_rename` command       | `ProfileStore`, `SettingsStore` (NEW)                  | Needs settings cascade                      |
| `useProfile.renameProfile`     | `profile_rename` IPC, `refreshProfiles`, `loadProfile` | Follow `duplicateProfile` pattern           |
| `ProfileActions` rename button | `useProfile.renaming`, `profileExists`                 | Disable during any in-flight operation      |
| Rename modal                   | `pendingDelete` overlay pattern in ProfilesPage        | Focus trap, ESC cancel, inline validation   |
| Auto-load (startup.rs)         | `last_used_profile` in settings                        | Works automatically after settings cascade  |

## Edgecases

- `rename()` at line 176 uses `fs::rename()` which silently overwrites — the `AlreadyExists` guard must precede it
- The existing test `test_rename_overwrites_existing_target_profile` (line 515) expects successful overwrite — must be updated to expect `AlreadyExists` error
- `launchOptimizationsAutosaveTimerRef` (350ms debounce) could fire during rename — must be cleared
- Profile name field is currently always editable (`ProfileFormSections.tsx:323`) — making it read-only for existing profiles eliminates the duplicate-on-save bug
- `ProfileContext` uses spread (`...profileState`) so new hook fields (`renaming`, `renameProfile`) flow through without explicit wiring
- Settings cascade is best-effort: if `settings_store.save()` fails after successful rename, the profile is renamed but `last_used_profile` is stale — acceptable for single-user desktop app

## Other Docs

- Feature spec: `docs/plans/profile-rename/feature-spec.md`
- Profile module: `crates/crosshook-core/src/profile/` (models.rs, toml_store.rs, legacy.rs)
- Settings module: `crates/crosshook-core/src/settings/mod.rs`
- Tauri v2 State Management: <https://v2.tauri.app/develop/state-management/>
- `rename(2)` man page: <https://man7.org/linux/man-pages/man2/rename.2.html>
