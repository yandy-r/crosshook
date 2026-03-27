# Duplicate Profile Implementation Plan

Profile duplication (#56) adds a `ProfileStore::duplicate()` method in `crosshook-core` that composes existing `load()`/`list()`/`save()` primitives to clone a `GameProfile` under a unique auto-generated name, exposed via a thin `profile_duplicate` Tauri IPC command and surfaced as a "Duplicate" button in the profile editor. The implementation modifies 8 existing files across 3 architectural layers (Rust core, Tauri IPC, React frontend) with zero new files or dependencies. The critical safety constraint is that `ProfileStore::save()` silently overwrites via `fs::write()`, so the `duplicate()` method must check `list()` before saving to prevent data destruction.

## Critically Relevant Files and Documentation

- src/crosshook-native/crates/crosshook-core/src/profile/toml_store.rs: ProfileStore with load/save/list/delete/rename — receives duplicate(), generate_unique_copy_name(), strip_copy_suffix(), DuplicateProfileResult
- src/crosshook-native/crates/crosshook-core/src/profile/models.rs: GameProfile struct (derives Clone) — .clone() enables full deep copy
- src/crosshook-native/crates/crosshook-core/src/profile/mod.rs: Module re-exports — must add DuplicateProfileResult
- src/crosshook-native/crates/crosshook-core/src/profile/exchange.rs: sanitize_profile_name() — name generation precedent
- src/crosshook-native/src-tauri/src/commands/profile.rs: Existing Tauri profile commands — pattern for profile_duplicate
- src/crosshook-native/src-tauri/src/lib.rs: Command registration in invoke_handler macro (profile block at lines 91-97)
- src/crosshook-native/src/hooks/useProfile.ts: Profile state hook (~700 lines) — add duplicateProfile callback
- src/crosshook-native/src/types/profile.ts: TypeScript types — add DuplicateProfileResult interface
- src/crosshook-native/src/components/ProfileActions.tsx: Save/Delete buttons — add Duplicate button
- src/crosshook-native/src/components/pages/ProfilesPage.tsx: Profile page — wire onDuplicate from context
- docs/plans/duplicate-profile/feature-spec.md: Authoritative feature blueprint with API design and name generation algorithm
- docs/plans/duplicate-profile/research-technical.md: Detailed technical specs, test cases, and technical decisions
- docs/plans/duplicate-profile/research-ux.md: UI placement, terminology, loading states, gamepad behavior
- docs/plans/duplicate-profile/research-business.md: 12 business rules and the critical no-overwrite constraint
- tasks/lessons.md: Prior implementation lessons — gamepad handler exclusions, Tauri permissions, UI audit requirements

## Implementation Plan

### Phase 1: Backend Core Logic and Tests

#### Task 1.1: Implement ProfileStore::duplicate() with unique name generation and tests Depends on [none]

**READ THESE BEFORE TASK**

- src/crosshook-native/crates/crosshook-core/src/profile/toml_store.rs
- src/crosshook-native/crates/crosshook-core/src/profile/models.rs
- src/crosshook-native/crates/crosshook-core/src/profile/mod.rs
- docs/plans/duplicate-profile/feature-spec.md
- docs/plans/duplicate-profile/research-technical.md
- docs/plans/duplicate-profile/research-business.md

**Instructions**

Files to Modify

- src/crosshook-native/crates/crosshook-core/src/profile/toml_store.rs
- src/crosshook-native/crates/crosshook-core/src/profile/mod.rs

Add the following to `toml_store.rs`:

1. **`DuplicateProfileResult` struct** (after line ~57, after `ProfileStoreError` enum and its impl blocks, before `impl Default for ProfileStore`): `#[derive(Debug, Clone, Serialize, Deserialize)] pub struct DuplicateProfileResult { pub name: String, pub profile: GameProfile }`. This crosses the IPC boundary so it needs serde derives. **Note**: `serde::{Serialize, Deserialize}` is not currently imported in `toml_store.rs` — add `use serde::{Serialize, Deserialize};` to the imports at the top of the file.

2. **`strip_copy_suffix()` free function** (after `validate_name()` at line ~214): Takes `&str`, returns `&str`. Strips `(Copy)` or `(Copy N)` suffix from a profile name to find the base name. Uses string matching — if the name ends with `(Copy)`, strip it; if it ends with `(Copy N)` where N is a digit sequence, strip that. Edge cases: `"Name"` returns unchanged, `"Copy"` returns unchanged, `"Game (Special Edition)"` returns unchanged (don't strip non-copy parentheticals).

3. **`generate_unique_copy_name()` method** on `ProfileStore`: Takes `source_name: &str` and `existing_names: &[String]`. First calls `strip_copy_suffix(source_name)` to get the base name. Then tries `"{base} (Copy)"`, if taken tries `"{base} (Copy 2)"`, `"{base} (Copy 3)"`, etc. up to a reasonable limit (e.g., 1000). Returns `Result<String, ProfileStoreError>`. If all candidates are taken, return an error. All generated names must pass `validate_name()`.

4. **`duplicate()` method** on `ProfileStore`: Public method `pub fn duplicate(&self, source_name: &str) -> Result<DuplicateProfileResult, ProfileStoreError>`. Implementation: `validate_name(source_name)?` -> `self.load(source_name)?` to get the profile (this also validates source exists) -> `self.list()?` to get existing names -> `generate_unique_copy_name(source_name, &existing_names)?` -> `self.save(&new_name, &profile)?` -> return `DuplicateProfileResult { name: new_name, profile }`.

   **CRITICAL**: The `save()` call must come AFTER checking `list()` for uniqueness. `save()` silently overwrites via `fs::write()`. This is the primary safety constraint.

5. **Re-export in `mod.rs`**: Add `DuplicateProfileResult` to the `pub use toml_store::{...}` line (line ~23).

6. **Tests** (in the existing `#[cfg(test)] mod tests` block at the bottom of `toml_store.rs`): Use the existing `sample_profile()` helper and `tempfile::tempdir()` + `ProfileStore::with_base_path()` pattern.

   Required test cases:
   - `test_duplicate_basic`: Save "MyGame", duplicate it, verify new file "MyGame (Copy)" exists with identical profile data (deep equality via `PartialEq`)
   - `test_duplicate_increments_on_conflict`: Save "MyGame" and "MyGame (Copy)", duplicate "MyGame", verify new name is "MyGame (Copy 2)"
   - `test_duplicate_of_copy`: Duplicate "MyGame (Copy)", verify it strips suffix and generates "MyGame (Copy 2)" (not "MyGame (Copy) (Copy)")
   - `test_duplicate_preserves_all_fields`: Save a fully populated profile, duplicate it, load both, assert `source_profile == duplicated_profile` (GameProfile derives PartialEq)
   - `test_duplicate_source_not_found`: Attempt to duplicate a nonexistent profile, verify `Err(ProfileStoreError::NotFound(_))`
   - `test_strip_copy_suffix`: Unit test the suffix stripping: `"Name (Copy)"` -> `"Name"`, `"Name (Copy 3)"` -> `"Name"`, `"Name"` -> `"Name"`, `"Game (Special Edition)"` -> `"Game (Special Edition)"`

   **Verification**: Run `cargo test -p crosshook-core` — all new and existing tests must pass.

### Phase 2: IPC Wiring

#### Task 2.1: Add profile_duplicate Tauri IPC command Depends on [1.1]

**READ THESE BEFORE TASK**

- src/crosshook-native/src-tauri/src/commands/profile.rs
- src/crosshook-native/src-tauri/src/lib.rs

**Instructions**

Files to Modify

- src/crosshook-native/src-tauri/src/commands/profile.rs
- src/crosshook-native/src-tauri/src/lib.rs

1. **Add `profile_duplicate` command** in `commands/profile.rs` (after `profile_delete`, before `profile_import_legacy`). Follow the exact pattern of existing commands:

   ```rust
   #[tauri::command]
   pub fn profile_duplicate(
       name: String,
       store: State<'_, ProfileStore>,
   ) -> Result<DuplicateProfileResult, String> {
       store.duplicate(&name).map_err(map_error)
   }
   ```

   Add `DuplicateProfileResult` to the existing import line `use crosshook_core::profile::{...}` at line 1 of the file.

2. **Register the command** in `lib.rs`: Add `commands::profile::profile_duplicate,` to the `tauri::generate_handler![]` macro invocation in the profile command block (lines 91-97). Insert alphabetically after `profile_delete`.

   **Verification**: `cargo build --manifest-path src/crosshook-native/Cargo.toml` succeeds.

#### Task 2.2: Add DuplicateProfileResult type and duplicateProfile hook callback Depends on [1.1]

**READ THESE BEFORE TASK**

- src/crosshook-native/src/types/profile.ts
- src/crosshook-native/src/hooks/useProfile.ts
- src/crosshook-native/src/context/ProfileContext.tsx
- docs/plans/duplicate-profile/research-ux.md

**Instructions**

Files to Modify

- src/crosshook-native/src/types/profile.ts
- src/crosshook-native/src/hooks/useProfile.ts

1. **Add `DuplicateProfileResult` interface** in `types/profile.ts` (after the `GameProfile` interface):

   ```typescript
   export interface DuplicateProfileResult {
     name: string;
     profile: GameProfile;
   }
   ```

2. **Add `duplicating` state** in `useProfile.ts`: `const [duplicating, setDuplicating] = useState(false);`

3. **Add `duplicateProfile` callback** in `useProfile.ts` (after `persistProfileDraft`). Follow the same async state machine pattern:

   ```typescript
   const duplicateProfile = useCallback(
     async (sourceName: string): Promise<void> => {
       if (!sourceName.trim()) return;
       setDuplicating(true);
       setError(null);
       try {
         const result = await invoke<DuplicateProfileResult>('profile_duplicate', {
           name: sourceName,
         });
         await refreshProfiles();
         await loadProfile(result.name);
       } catch (err) {
         const message = err instanceof Error ? err.message : String(err);
         setError(message);
       } finally {
         setDuplicating(false);
       }
     },
     [loadProfile, refreshProfiles]
   );
   ```

   Import `DuplicateProfileResult` from the types module.

4. **Extend `UseProfileResult` interface**: Add `duplicateProfile: (sourceName: string) => Promise<void>;` and `duplicating: boolean;` to the interface definition.

5. **Add to return object**: Include `duplicateProfile` and `duplicating` in the hook's return value.

   Note: `ProfileContext.tsx` uses `...profileState` spread, so `duplicateProfile` and `duplicating` auto-propagate to all context consumers. No changes needed in `ProfileContext.tsx`.

   **Verification**: `npm run build` (from `src/crosshook-native/`) succeeds with no TypeScript errors.

### Phase 3: UI Integration

#### Task 3.1: Add Duplicate button to ProfileActions and wire in ProfilesPage Depends on [2.1, 2.2]

**READ THESE BEFORE TASK**

- src/crosshook-native/src/components/ProfileActions.tsx
- src/crosshook-native/src/components/pages/ProfilesPage.tsx
- src/crosshook-native/src/hooks/useProfile.ts
- docs/plans/duplicate-profile/research-ux.md
- tasks/lessons.md

**Instructions**

Files to Modify

- src/crosshook-native/src/components/ProfileActions.tsx
- src/crosshook-native/src/components/pages/ProfilesPage.tsx

1. **Update `ProfileActions.tsx` props interface**: Add `canDuplicate: boolean`, `onDuplicate: () => void | Promise<void>`, and `duplicating: boolean` to the component's props.

2. **Add Duplicate button**: Place between the Save and Delete buttons. Use `crosshook-button crosshook-button--secondary` CSS classes. The button should be disabled when `!canDuplicate || duplicating`. Show "Duplicating..." text when `duplicating` is true, "Duplicate" otherwise. Use terminology "Duplicate" (not "Copy" or "Clone") per UX research.

   ```tsx
   <button
     type="button"
     className="crosshook-button crosshook-button--secondary"
     onClick={() => void onDuplicate()}
     disabled={!canDuplicate || duplicating}
   >
     {duplicating ? 'Duplicating...' : 'Duplicate'}
   </button>
   ```

3. **Wire in `ProfilesPage.tsx`**: Destructure `duplicateProfile`, `duplicating`, `selectedProfile`, `saving`, `deleting`, `loading` from `useProfileContext()`. Compute the guard:

   ```typescript
   const canDuplicate = profileExists && !saving && !deleting && !loading && !duplicating;
   ```

   Pass to `<ProfileActions>`:

   ```tsx
   <ProfileActions
     // ... existing props
     canDuplicate={canDuplicate}
     onDuplicate={() => duplicateProfile(selectedProfile)}
     duplicating={duplicating}
   />
   ```

   Note: Per `tasks/lessons.md`, gamepad handlers must skip editable controls. The Duplicate button is a standard button (not editable), so it is naturally D-pad navigable alongside Save/Delete — no special gamepad handling needed.

   **Verification**: `npm run build` succeeds. Manual verification: launch dev server (`./scripts/dev-native.sh`), load a profile, click Duplicate, verify new `"Name (Copy)"` profile appears and is auto-selected.

## Advice

- **The `save()` overwrite risk is the single most important correctness concern.** `ProfileStore::save()` does `fs::write()` unconditionally. The `duplicate()` method MUST call `list()` to get existing names and verify uniqueness BEFORE calling `save()`. Every code reviewer should verify this check is present.
- **`strip_copy_suffix()` is the most nuanced logic.** Test edge cases thoroughly: names ending in `(Copy)` vs `(Copy N)` vs names that happen to contain parentheses for other reasons (e.g., `"Game (Special Edition)"`). Use a regex or careful string parsing — don't accidentally strip non-copy parentheticals.
- **The feature spec chose Option A (ProfileStore::duplicate in crosshook-core)**, overriding `research-recommendations.md` which favored composing in the Tauri command layer. Follow the feature spec — all business logic lives in `crosshook-core`, and the Tauri command is a thin delegation.
- **Task 2.1 and Task 2.2 are fully parallelizable.** They modify non-overlapping files across the IPC boundary (Rust vs TypeScript). Both write against the agreed API contract (`DuplicateProfileResult` struct/interface and `profile_duplicate` command name). No coordination needed.
- **`ProfileContext.tsx` requires zero changes.** It spreads `UseProfileResult` via `...profileState`, so adding `duplicateProfile` and `duplicating` to the hook's return automatically propagates them to all context consumers. Do not modify this file.
- **Commit each phase separately using conventional commits** for changelog visibility: `feat(profile): add ProfileStore::duplicate() with unique name generation` (Phase 1), `feat(profile): add profile_duplicate Tauri IPC command` (Task 2.1), `feat(profile): add duplicateProfile hook and DuplicateProfileResult type` (Task 2.2), `feat(ui): add Duplicate button to profile actions` (Phase 3). The PR must reference `Closes #56`.
- **No new error variant may be needed** if you cap the name generation loop at a reasonable limit (e.g., 1000 iterations) and return an existing `InvalidName` error for the exhaustion case. However, adding a `NameCollisionExhausted` variant to `ProfileStoreError` is cleaner and more descriptive — either approach works.
- **Post-duplicate UX**: The duplicate flow auto-selects the new profile via `loadProfile(result.name)`. This matches macOS Finder, VS Code, and JetBrains patterns where the duplicate is immediately active for editing/renaming.
