# Profile Rename — Task Structure Analysis

## Executive Summary

Profile rename is a well-scoped feature (~75 lines across 6 files) with the backend ~90% complete. The remaining work spans three layers — Rust core, Tauri command, and React frontend — with clear dependency boundaries. The optimal strategy is 3 phases with 5 implementation tasks, where Phase 1 tasks can run in parallel and Phase 2 has limited parallelism due to data flow dependencies.

## Recommended Phase Structure

### Phase 1: Backend Hardening (2 tasks, parallelizable)

**Rationale**: The backend is functional but lacks overwrite protection and settings cascade. These two concerns are in separate crates and can be developed in parallel.

#### Task 1A: AlreadyExists Error + Overwrite Guard

- **Files**: `crates/crosshook-core/src/profile/toml_store.rs` (1 file)
- **Scope**: ~15 lines
- **Changes**:
  1. Add `AlreadyExists(String)` variant to `ProfileStoreError` enum (line 16)
  2. Add `Display` impl arm: `"a profile named '{name}' already exists"` (line 27)
  3. Insert `if new_path.exists() { return Err(ProfileStoreError::AlreadyExists(...)) }` in `rename()` before `fs::rename` (between lines 175-176)
  4. Update `test_rename_overwrites_existing_target_profile` (line 515) to expect `AlreadyExists` error instead of success
  5. Add test: rename to non-existent name succeeds (already covered by `test_rename_success`)
- **Verification**: `cargo test -p crosshook-core`
- **Parallelism**: Independent of Task 1B. No shared file modifications.

#### Task 1B: Settings Cascade in Tauri Command

- **Files**: `src-tauri/src/commands/profile.rs` (1 file)
- **Scope**: ~12 lines
- **Changes**:
  1. Add `settings_store: State<'_, SettingsStore>` parameter to `profile_rename` (line 148)
  2. Import `SettingsStore` from `crosshook_core::settings`
  3. Add best-effort `last_used_profile` cascade after `store.rename()` succeeds (following `profile_delete` pattern at line 113)
  4. Add test for settings cascade (load settings, set `last_used_profile`, rename, verify updated)
  5. Add test: settings cascade failure doesn't fail the rename
- **Verification**: `cargo test -p crosshook-cli` (Tauri command tests run in the binary crate context)
- **Parallelism**: Independent of Task 1A. Different file. Depends on `SettingsStore` API which is stable (no changes needed to `settings/mod.rs`).
- **Note**: `SettingsStore` is already registered as Tauri state in `lib.rs` — no registration changes needed.

### Phase 2: Frontend Integration (2 tasks, partially parallelizable)

**Dependencies**: Phase 1 must be complete (IPC contract changes must be deployed before frontend can invoke).

#### Task 2A: useProfile Hook — renameProfile()

- **Files**: `src/hooks/useProfile.ts` (1 file)
- **Scope**: ~30 lines
- **Changes**:
  1. Add `renaming` state: `const [renaming, setRenaming] = useState(false)`
  2. Add `renameProfile` callback following `duplicateProfile` pattern (line 569):
     - Accept `(oldName: string, newName: string)`
     - Cancel pending autosave timer (launch optimizations use 350ms debounce — see `launchOptimizationsAutosaveDelayMs`)
     - Set `renaming(true)`, clear error
     - `await invoke('profile_rename', { oldName, newName })`
     - `await refreshProfiles()` + `await loadProfile(newName)`
     - Error handling in catch, `setRenaming(false)` in finally
  3. Add `renaming` and `renameProfile` to `UseProfileResult` interface (line 20)
  4. Include in hook return value
- **Verification**: Manual test via dev server; TypeScript compilation check
- **Parallelism**: Must complete before Task 2B can wire the UI. However, Task 2B's modal UI can be scaffolded in parallel if it uses placeholder props.

#### Task 2B: UI Components — Rename Button + Modal + Read-Only Name

- **Files**: `src/components/ProfileActions.tsx`, `src/components/pages/ProfilesPage.tsx`, `src/components/ProfileFormSections.tsx` (3 files)
- **Scope**: ~20 lines total
- **Changes in ProfileActions.tsx**:
  1. Add to `ProfileActionsProps`: `renaming: boolean`, `canRename: boolean`, `onRename: () => void | Promise<void>`
  2. Add Rename button between Duplicate and Delete buttons (follows existing button pattern)
  3. Disable when `!canRename || renaming`
- **Changes in ProfilesPage.tsx**:
  1. Destructure `renaming` and `renameProfile` from `useProfileContext()`
  2. Add `canRename` guard: `profileExists && !saving && !deleting && !loading && !duplicating && !renaming`
  3. Add `pendingRename` state: `useState<string | null>(null)` for modal
  4. Add rename modal overlay following `pendingDelete` pattern (line 179): input pre-filled with current name, confirm/cancel buttons, inline validation
  5. Pass rename props to `ProfileActions`
- **Changes in ProfileFormSections.tsx**:
  1. Add `readOnly` or `disabled` attribute to profile name input (line 323) when `profileExists` is true
  2. Accept `profileExists` prop (or derive from context)
- **No changes needed**:
  - `ProfileContext.tsx`: Uses `...profileState` spread (line 53), so `renaming` and `renameProfile` flow through automatically
  - `src/types/profile.ts`: Rename returns void, no new types
- **Verification**: Dev server visual test, keyboard navigation, gamepad test
- **Parallelism**: Depends on Task 2A for `renameProfile` and `renaming` to exist in hook. The modal scaffold can start in parallel.

### Phase 3: UX Polish (1 task, optional/deferrable)

**Dependencies**: Phase 2 must be complete.

#### Task 3: Polish and Accessibility

- **Files**: `src/components/pages/ProfilesPage.tsx`, potentially `src/styles/` CSS files
- **Scope**: Variable
- **Changes** (each independently shippable):
  1. F2 keyboard shortcut to open rename dialog
  2. ARIA attributes on rename modal: `role="dialog"`, `aria-modal="true"`, `aria-labelledby`
  3. Gamepad support: A to confirm, B to cancel (follows existing `data-crosshook-focus-root="modal"` pattern)
  4. Success toast with Undo (5-8s window) — may require new toast component
  5. Debounced inline validation (300ms) for name conflict checking
- **Verification**: Keyboard navigation test, screen reader test, gamepad test
- **Parallelism**: All sub-items are independent and can be tackled incrementally.

## Dependency Analysis

```
Task 1A (AlreadyExists guard) ──┐
                                 ├──> Task 2A (useProfile hook) ──> Task 2B (UI) ──> Task 3 (Polish)
Task 1B (Settings cascade) ─────┘
```

- **Tasks 1A and 1B are fully parallel**: Different files, different crates, no shared state.
- **Task 2A depends on both 1A and 1B**: The hook invokes the updated IPC command.
- **Task 2B depends on 2A**: Needs `renaming` state and `renameProfile` callback to exist.
- **Task 2B's modal scaffold** can start in parallel with 2A if it uses stub props.
- **Task 3 depends on 2B**: Polish builds on the working UI.

## File-to-Task Mapping

| File                                              | Task | Change Type                               |
| ------------------------------------------------- | ---- | ----------------------------------------- |
| `crates/crosshook-core/src/profile/toml_store.rs` | 1A   | Error variant, guard, test update         |
| `src-tauri/src/commands/profile.rs`               | 1B   | SettingsStore param, cascade logic, tests |
| `src/hooks/useProfile.ts`                         | 2A   | `renameProfile()`, `renaming` state       |
| `src/components/ProfileActions.tsx`               | 2B   | Rename button + props                     |
| `src/components/pages/ProfilesPage.tsx`           | 2B   | Modal dialog, wiring, `canRename`         |
| `src/components/ProfileFormSections.tsx`          | 2B   | Read-only name field                      |
| `src/context/ProfileContext.tsx`                  | —    | No changes (auto-extends via spread)      |
| `src/types/profile.ts`                            | —    | No changes (rename returns void)          |
| `src-tauri/src/lib.rs`                            | —    | No changes (command already registered)   |
| `crates/crosshook-core/src/settings/mod.rs`       | —    | No changes (API already sufficient)       |

## Optimization Opportunities

1. **Batch Phase 1 into a single task** if only one implementor is available — the two files are small enough (~25 lines combined) that context switching overhead may exceed parallelism gains.

2. **Task 2B can be split** if more parallelism is desired:
   - 2B-i: `ProfileActions.tsx` (Rename button) — purely additive, follows existing pattern
   - 2B-ii: `ProfilesPage.tsx` (rename modal + wiring) — the main UI work
   - 2B-iii: `ProfileFormSections.tsx` (read-only name field) — one-line change
     However, these are so small that splitting adds overhead without meaningful benefit.

3. **Phase 3 items are independently shippable** — each polish item can be a separate commit/PR if the team prefers incremental delivery.

4. **Test-first approach for Task 1A**: The existing test `test_rename_overwrites_existing_target_profile` documents the current (broken) behavior. Updating it first to expect `AlreadyExists` creates a failing test that drives the implementation.

## Implementation Strategy Recommendations

1. **Start with Task 1A** (test-first): Update the overwrite test to expect `AlreadyExists`, then implement the guard. This is the highest-risk change (prevents data loss) and should be verified first.

2. **Run Task 1B in parallel** if a second implementor is available, otherwise sequence after 1A.

3. **Task 2A is the critical path**: The hook is the bridge between backend and UI. It should be implemented carefully following the `duplicateProfile` template exactly, including autosave timer cancellation.

4. **Task 2B is the largest task** but low-risk: All patterns are established (button pattern, modal pattern, read-only input). The rename modal follows the `pendingDelete` overlay almost identically.

5. **Phase 3 is optional for MVP**: The core rename functionality is complete after Phase 2. Polish items improve UX but don't affect correctness.

6. **Commit strategy**: One conventional commit per task (`feat(profile): add AlreadyExists overwrite guard`, `feat(profile): cascade last_used_profile on rename`, `feat(ui): add rename button and modal dialog`, etc.). Phase 3 items get individual commits.
