# Profile Rename — Analysis Context

## Executive Summary

Profile rename is ~90% implemented on the backend. The core `ProfileStore::rename()`, `profile_rename` Tauri command, and command registration all exist. The remaining work spans 3 layers across ~6 files and ~75 lines: add `AlreadyExists` overwrite protection, cascade `last_used_profile` in the Tauri command, and build a frontend Rename button + modal dialog. No new dependencies. Launcher cascade is NOT needed (launcher paths derive from `display_name`, not profile name).

## Architecture Context

### Three-Layer Pattern (Core → Command → Hook → UI)

Every profile operation follows this vertical slice, and rename must follow it identically:

1. **crosshook-core** (`ProfileStore::rename()`) — validates names, checks existence, performs `fs::rename`. Atomic on same filesystem (POSIX guarantee). Currently **silently overwrites** if target exists — must add `AlreadyExists` guard.
2. **Tauri command** (`profile_rename`) — wraps core with `State<'_>` injection, handles best-effort side effects (settings cascade). Pattern established by `profile_delete` (best-effort launcher cleanup before deletion).
3. **React hook** (`useProfile.renameProfile()`) — sets loading flag, invokes IPC, refreshes profiles, loads target profile, catches errors. Exact template: `duplicateProfile()` at `useProfile.ts:569`.
4. **UI components** — Rename button in `ProfileActions`, rename modal in `ProfilesPage` (follows `pendingDelete` overlay pattern), read-only name field in `ProfileFormSections`.

### Profile Identity Model

- Profile name = TOML filename stem (`~/.config/crosshook/profiles/{name}.toml`)
- Name is NOT stored inside the TOML content
- `game.name` inside TOML is the game's display name — conceptually independent, do not sync
- Rename operation: `fs::rename(old.toml, new.toml)` — file contents unchanged

### What Does NOT Need Changing

- `lib.rs` — `profile_rename` already registered at line 96
- `models.rs` — GameProfile struct unchanged
- `launcher_store.rs` — launcher paths derive from `display_name`, not profile name
- `startup.rs` — auto-load works automatically after settings cascade
- `types/profile.ts` — no new TS types needed (rename returns void)
- `recent.toml` — stores file paths, not profile names

## Critical Files Reference

| File                                              | Change Required                                                                                                                                    | Key Lines                                                                                     |
| ------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------- |
| `crates/crosshook-core/src/profile/toml_store.rs` | Add `AlreadyExists(String)` variant to `ProfileStoreError`, add `new_path.exists()` guard before `fs::rename()` in `rename()`, update Display impl | Error enum: L16, rename(): L163-178, Display: near L26, test to update: L515                  |
| `src-tauri/src/commands/profile.rs`               | Add `settings_store: State<'_, SettingsStore>` param, add best-effort `last_used_profile` cascade                                                  | `profile_rename`: L148, `profile_delete` pattern: L113                                        |
| `src/hooks/useProfile.ts`                         | Add `renameProfile()` callback + `renaming` boolean, cancel autosave timer before rename                                                           | `duplicateProfile` template: L569, autosave timer: L335, `UseProfileResult` interface: L20-48 |
| `src/components/ProfileActions.tsx`               | Add Rename button between Duplicate and Delete, add `canRename`/`renaming`/`onRename` props                                                        | Props interface: L8                                                                           |
| `src/components/ProfileFormSections.tsx`          | Make profile name input read-only for existing profiles                                                                                            | Name input: L323                                                                              |
| `src/components/pages/ProfilesPage.tsx`           | Add rename modal (follow `pendingDelete` overlay pattern), wire `canRename` guard, add `pendingRename` state                                       | Delete overlay: L179, `canDuplicate` guard: L70                                               |

## Patterns to Follow

### 1. Best-Effort Side Effects (Tauri Command)

```
Primary operation → succeeds or fails (hard error)
Side effects → best-effort with warning log on failure
```

Source: `profile_delete` at `commands/profile.rs:113-123`. Settings cascade must follow this pattern — log warning, don't fail the rename.

### 2. React Hook IPC (9-Step Pattern)

Guard → set loading flag → clear error → invoke IPC → refresh profiles → load target → catch/display errors → clear flag → deps array. Source: `duplicateProfile()` at `useProfile.ts:569-588`.

### 3. UI Action Button Props

Interface pattern: `{canX: boolean, xing: boolean, onX: callback}`. All action buttons disable when ANY async operation is in-flight. Source: `ProfileActions.tsx:8`.

### 4. Delete Overlay Dialog → Rename Modal Template

State-driven overlay with `pendingDelete: {name} | null` pattern. Focus trapping, ESC to cancel, Enter to confirm. Source: `ProfilesPage.tsx:179`.

### 5. ProfileContext Auto-Extension

`ProfileContextValue extends UseProfileResult`. New hook fields (`renaming`, `renameProfile`) flow through automatically via `...profileState` spread at `ProfileContext.tsx:53`. No explicit wiring needed.

### 6. Error Enum Convention

Named variants with Display impl. `AlreadyExists(String)` → `"a profile named '{name}' already exists"`. From impls for wrapped types. Source: `toml_store.rs:16`.

### 7. Button Disable Guard

```ts
const canRename = profileExists && !saving && !deleting && !loading && !duplicating && !renaming;
```

All capability flags must include `!renaming`. Source: `ProfilesPage.tsx:70` (canDuplicate pattern).

## Cross-Cutting Concerns

1. **Autosave race condition**: `launchOptimizationsAutosaveTimerRef` has a 350ms debounce timer. Must be cleared in `renameProfile()` before invoking IPC to prevent writing to old filename.

2. **Test update required**: `test_rename_overwrites_existing_target_profile` (toml_store.rs:515) currently expects silent overwrite — must be changed to expect `AlreadyExists` error. Both files must still exist after blocked rename.

3. **All button disable states must include `renaming`**: Adding a new async operation means existing `canSave`, `canDelete`, `canDuplicate` guards must also exclude `renaming`.

4. **Settings cascade timing**: Must happen in Tauri command (single IPC call), not frontend. Avoids extra round-trip and partial failure risk.

5. **Same-name no-op**: `rename("A", "A")` returns `Ok(())` silently — both backend and frontend should handle this early return.

6. **Validation runs twice**: `validate_name()` in backend + inline validation in modal. Backend validation is the safety net; frontend validation provides UX feedback.

## Parallelization Opportunities

### Phase 1 (Backend) — All tasks can be developed together

- `AlreadyExists` error variant + Display impl + test update → single file (`toml_store.rs`)
- Settings cascade in Tauri command → single file (`commands/profile.rs`)
- New test for settings cascade → can run after both changes

### Phase 2 (Frontend) — Two independent streams

- **Stream A**: Hook changes (`useProfile.ts` — `renameProfile()` + `renaming` state)
- **Stream B**: UI components (rename modal in `ProfilesPage.tsx`, button in `ProfileActions.tsx`, read-only field in `ProfileFormSections.tsx`)
- Stream B has no code dependency on Stream A until final wiring
- Context auto-extension means no `ProfileContext.tsx` changes needed

### Phase 3 (Polish) — Fully independent tasks

- Undo toast, F2 shortcut, gamepad optimization, save flow disambiguation — each independent

## Implementation Constraints

1. **No new dependencies**: All primitives exist in codebase (serde, toml, tracing, tempfile for tests)
2. **No new files**: All changes fit in existing modules
3. **Backend must complete before frontend**: Frontend `renameProfile()` depends on enhanced `profile_rename` IPC command
4. **`AlreadyExists` must precede `fs::rename`**: Silent overwrite is the highest-severity risk (permanent data loss)
5. **Read-only name field eliminates root cause bug**: The "edit name + save = new profile" confusion goes away when existing profiles can only be renamed via dedicated button
6. **Void return for rename IPC**: No result data crosses IPC — frontend knows the new name because the user provided it. `refreshProfiles()` + `loadProfile(newName)` handles state sync.
7. **macOS case-sensitivity deferred**: Case-only rename (e.g., "Game" → "game") works on Linux but has edge cases on macOS (APFS). Not a concern for current Linux-only target.

## Key Decisions (Resolved)

| Decision                  | Choice                             | Rationale                                                     |
| ------------------------- | ---------------------------------- | ------------------------------------------------------------- |
| Overwrite policy          | Block with `AlreadyExists` error   | Simplest, safest. User can delete target first if needed      |
| Rename trigger UX         | Dedicated Rename button + modal    | Unambiguous intent; gamepad-friendly; avoids overloading Save |
| Profile name field        | Read-only for existing profiles    | Eliminates root cause "edit name + save = new profile" bug    |
| `game.name` sync          | Leave independent                  | `game.name` is game display name, not profile identity        |
| Launcher cascade          | NOT needed                         | Launcher paths derive from `display_name`, not profile name   |
| Settings cascade location | In Tauri command (single IPC call) | Matches `profile_delete` pattern; avoids partial failure      |
| Return type               | `void` (not a result struct)       | Frontend provides newName; no IPC result data needed          |

## Key Recommendations

1. **Start with backend**: Add `AlreadyExists` + settings cascade first — small, testable, unblocks frontend
2. **Follow `duplicateProfile` exactly**: The 9-step hook pattern is battle-tested; don't deviate
3. **Reuse `pendingDelete` overlay pattern**: Minimize new UI patterns; rename modal should be structurally identical
4. **Cancel autosave timer**: One line (`clearTimeout`) prevents a subtle race condition
5. **Update ALL disable guards**: When adding `renaming`, grep for existing `!duplicating` guards to find all places that need `!renaming`
6. **Test the overwrite guard**: The existing test `test_rename_overwrites_existing_target_profile` is the safety-critical test — update it to expect `AlreadyExists` and verify both files survive
