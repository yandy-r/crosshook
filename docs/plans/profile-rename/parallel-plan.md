# Profile Rename Implementation Plan

Profile rename adds overwrite protection and settings cascade to the existing backend `ProfileStore::rename()` and `profile_rename` Tauri command, then builds a frontend Rename button with modal dialog following established patterns (`duplicateProfile` hook pattern, `pendingDelete` overlay pattern). The backend is ~90% complete — the remaining work is ~75 lines across 6 files with no new dependencies. Launcher cascade is NOT needed because launcher paths derive from `display_name`, not profile name.

## Critically Relevant Files and Documentation

- src/crosshook-native/crates/crosshook-core/src/profile/toml_store.rs: ProfileStore with rename() at L163, ProfileStoreError enum at L16, validate_name() at L273, overwrite test at L515
- src/crosshook-native/src-tauri/src/commands/profile.rs: Tauri commands — profile_rename at L148, profile_delete best-effort pattern at L113, map_error helper
- src/crosshook-native/src-tauri/src/lib.rs: Command registration — profile_rename already at L96, state management at L62-65
- src/crosshook-native/crates/crosshook-core/src/settings/mod.rs: SettingsStore + AppSettingsData with last_used_profile at L23
- src/crosshook-native/src/hooks/useProfile.ts: duplicateProfile template at L569, autosave timer at L335, UseProfileResult interface at L20
- src/crosshook-native/src/components/ProfileActions.tsx: Action button props interface at L8, button layout pattern
- src/crosshook-native/src/components/pages/ProfilesPage.tsx: canDuplicate guard at L70, pendingDelete overlay at L179
- src/crosshook-native/src/components/ProfileFormSections.tsx: Profile name input at L323
- src/crosshook-native/src/context/ProfileContext.tsx: Auto-extends via ...profileState spread at L53
- docs/plans/profile-rename/feature-spec.md: Complete feature spec with architecture, API contracts, decisions
- docs/plans/profile-rename/research-ux.md: Competitive analysis, accessibility, Steam Deck dialog sizing
- docs/plans/profile-rename/research-patterns.md: Implementation checklist with patterns and line numbers

## Implementation Plan

### Phase 1: Backend Hardening

#### Task 1.1: Add AlreadyExists Error and Overwrite Guard Depends on [none]

**READ THESE BEFORE TASK**

- src/crosshook-native/crates/crosshook-core/src/profile/toml_store.rs
- docs/plans/profile-rename/feature-spec.md

**Instructions**

Files to Modify

- src/crosshook-native/crates/crosshook-core/src/profile/toml_store.rs

Add `AlreadyExists(String)` variant to `ProfileStoreError` enum (after `NotFound` at line 17). Add Display match arm: `Self::AlreadyExists(name) => write!(f, "a profile named '{name}' already exists")`.

In `rename()` method (line 163-178), insert `new_path.exists()` check after the `old_path.exists()` check and before `fs::rename()`:

```rust
if new_path.exists() {
    return Err(ProfileStoreError::AlreadyExists(new_name.to_string()));
}
```

Update `test_rename_overwrites_existing_target_profile` (line 515) to expect `AlreadyExists` error instead of success. Rename test to `test_rename_rejects_existing_target_profile`. Assert both source and target files still exist after the blocked rename.

Verify: `cargo test -p crosshook-core`

#### Task 1.2: Add Settings Cascade to Tauri Command Depends on [none]

**READ THESE BEFORE TASK**

- src/crosshook-native/src-tauri/src/commands/profile.rs
- src/crosshook-native/crates/crosshook-core/src/settings/mod.rs

**Instructions**

Files to Modify

- src/crosshook-native/src-tauri/src/commands/profile.rs

Add `use crosshook_core::settings::SettingsStore;` import. Add `settings_store: State<'_, SettingsStore>` parameter to `profile_rename` function (line 148).

After `store.rename(&old_name, &new_name).map_err(map_error)?;`, add best-effort settings cascade following the `profile_delete` pattern (line 113):

```rust
if let Ok(mut settings) = settings_store.load() {
    if settings.last_used_profile.trim() == old_name.trim() {
        settings.last_used_profile = new_name.trim().to_string();
        if let Err(err) = settings_store.save(&settings) {
            tracing::warn!(%err, %old_name, %new_name, "settings update after profile rename failed");
        }
    }
}
```

No changes to `lib.rs` — `profile_rename` is already registered at line 96 and `SettingsStore` is already managed at line 63.

Verify: `cargo test -p crosshook-core`

### Phase 2: Frontend Integration

#### Task 2.1: Add renameProfile to useProfile Hook Depends on [1.1, 1.2]

**READ THESE BEFORE TASK**

- src/crosshook-native/src/hooks/useProfile.ts
- docs/plans/profile-rename/research-patterns.md

**Instructions**

Files to Modify

- src/crosshook-native/src/hooks/useProfile.ts

Add `renaming` and `renameProfile` to the `UseProfileResult` interface (line 20-48):

```typescript
renameProfile: (oldName: string, newName: string) => Promise<void>;
renaming: boolean;
```

Add state: `const [renaming, setRenaming] = useState(false);` near the other operation flags (line 328-329).

Add `renameProfile` callback after `duplicateProfile` (line 588), following the exact same 9-step pattern:

1. Guard: `if (!oldName.trim() || !newName.trim() || oldName.trim() === newName.trim()) return;`
2. Cancel autosave timer: `if (launchOptimizationsAutosaveTimerRef.current !== null) { clearTimeout(launchOptimizationsAutosaveTimerRef.current); launchOptimizationsAutosaveTimerRef.current = null; }`
3. `setRenaming(true); setError(null);`
4. `await invoke('profile_rename', { oldName: oldName.trim(), newName: newName.trim() });`
5. `await refreshProfiles(); await loadProfile(newName.trim());`
6. Catch: `setError(err instanceof Error ? err.message : String(err));`
7. Finally: `setRenaming(false);`

Add `renaming` and `renameProfile` to the hook return value.

Context auto-extension: `ProfileContext.tsx` uses `...profileState` spread so `renaming` and `renameProfile` flow through automatically — no context changes needed.

#### Task 2.2: Add Rename Button, Modal, and Read-Only Name Field Depends on [2.1]

**READ THESE BEFORE TASK**

- src/crosshook-native/src/components/ProfileActions.tsx
- src/crosshook-native/src/components/pages/ProfilesPage.tsx
- src/crosshook-native/src/components/ProfileFormSections.tsx
- docs/plans/profile-rename/research-ux.md

**Instructions**

Files to Modify

- src/crosshook-native/src/components/ProfileActions.tsx
- src/crosshook-native/src/components/pages/ProfilesPage.tsx
- src/crosshook-native/src/components/ProfileFormSections.tsx

**ProfileActions.tsx**: Add `canRename: boolean`, `renaming: boolean`, `onRename: () => void | Promise<void>` to `ProfileActionsProps` interface. Add Rename button between Duplicate and Delete buttons following the existing button pattern:

```tsx
<button
  type="button"
  className="crosshook-button crosshook-button--secondary"
  onClick={() => void onRename()}
  disabled={!canRename || renaming}
>
  {renaming ? 'Renaming...' : 'Rename'}
</button>
```

**ProfilesPage.tsx**: Destructure `renaming` and `renameProfile` from `useProfileContext()`. Add `canRename` guard following the `canDuplicate` pattern (line 70):

```typescript
const canRename = profileExists && !saving && !deleting && !loading && !duplicating && !renaming;
```

CRITICAL: Add `&& !renaming` to existing `canDelete` and `canDuplicate` guards (which already include `!duplicating`). Note: `canSave` does NOT currently include `!duplicating` — do NOT add `!renaming` to `canSave` either, to maintain consistency with the existing guard pattern. Only modify guards that already follow the mutual-exclusion pattern. Grep for `!duplicating` to find the correct locations.

Add `pendingRename` state: `const [pendingRename, setPendingRename] = useState<string | null>(null);` for the rename modal.

Add rename modal overlay after the delete overlay (line 216), following the `pendingDelete` pattern: overlay div with `data-crosshook-focus-root="modal"` → dialog div → heading "Rename Profile" → text input pre-filled with current name (fully selected on mount) → inline validation for empty/conflict → Confirm/Cancel buttons. Wire `onRename` to `setPendingRename(selectedProfile)` and confirm to `renameProfile(pendingRename, newNameValue)`.

Pass rename props to `ProfileActions`: `canRename`, `renaming`, `onRename`.

**ProfileFormSections.tsx**: Add `readOnly={profileExists}` to the profile name `<input>` at line 323. This makes the name field read-only for existing profiles, eliminating the root cause "edit name + save = new profile" bug. `ProfileFormSections` does not currently receive `profileExists` — add it as a prop to the component's props interface and pass it from `ProfilesPage.tsx` where `profileExists` is already available from `useProfileContext()`.

### Phase 3: UX Polish (Optional)

#### Task 3.1: Accessibility and Keyboard Shortcuts Depends on [2.2]

**READ THESE BEFORE TASK**

- src/crosshook-native/src/components/pages/ProfilesPage.tsx
- docs/plans/profile-rename/research-ux.md

**Instructions**

Files to Modify

- src/crosshook-native/src/components/pages/ProfilesPage.tsx

Add ARIA attributes to rename modal: `role="dialog"`, `aria-modal="true"`, `aria-labelledby` pointing to dialog heading. Add `role="alert"` to inline validation error text.

Add F2 keyboard shortcut: when a profile is selected and no modal is open, F2 opens the rename dialog. Use existing keyboard event handling patterns.

Add success toast with Undo: after successful rename, show "Renamed to 'New Name'" with Undo button (5-8s window). Undo calls `renameProfile(newName, oldName)` with swapped args. NOTE: Check if a toast/notification component already exists in the codebase. If not, creating one expands this task's scope — consider deferring the toast to a separate task or using a simple CSS-animated banner instead.

Gamepad optimization: dialog positioned in top half of screen to accommodate Steam Deck virtual keyboard overlay. Minimum 44px touch targets. B button cancels, A button confirms.

## Advice

- The existing test `test_rename_overwrites_existing_target_profile` at toml_store.rs:515 is the safety-critical test. It currently expects silent overwrite. Task 1.1 must update this test first (test-driven approach) — if this test still passes after your changes, the overwrite guard is broken.
- Cancel the autosave timer (`launchOptimizationsAutosaveTimerRef`) in `renameProfile()` BEFORE the `invoke()` call, not after. A 350ms debounced save targeting the old filename would create a ghost file if it fires between rename and state refresh.
- When adding `renaming` to disable guards in ProfilesPage, grep for `!duplicating` to find guards that need `&& !renaming`. Only add to guards that already follow the mutual-exclusion pattern (`canDelete`, `canDuplicate`). Note that `canSave` does NOT include `!duplicating` — do not add `!renaming` to `canSave` to maintain consistency.
- `ProfileContext.tsx` does NOT need changes — it uses `...profileState` spread, so `renaming` and `renameProfile` flow through automatically. Don't add explicit wiring.
- The Tauri command `profile_rename` is already registered in `lib.rs:96`. Do NOT re-register it. Only the function signature in `commands/profile.rs` changes (adding `SettingsStore` param).
- Profile name is the filename stem, NOT a field inside the TOML. `game.name` inside the TOML is the game's display name — do not sync them on rename. The rename operation does `fs::rename` without modifying file contents.
- The rename modal should pre-fill and fully select the current name text. This is the universal rename UX convention (Windows F2, macOS Enter, Linux file managers). Use `inputRef.current?.select()` in a `useEffect` on mount.
