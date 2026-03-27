# Profile Rename — Recommendations & Risk Assessment

## Executive Summary

Profile rename is a well-scoped feature with substantial backend plumbing already in place. The `ProfileStore::rename()` method exists with tests, the `profile_rename` Tauri command is registered, and the `launcher_store.rs` already has `rename_launcher_files()` for cascading launcher updates. The primary work is: (1) adding cascading side effects to the Tauri command (settings, launchers), (2) building the frontend rename flow in `useProfile.ts` and `ProfileActions.tsx`, and (3) adding conflict detection (prevent overwriting existing profiles). This is a low-risk, high-impact feature that can be delivered in 2-3 focused phases.

---

## Implementation Recommendations

### Approach: Atomic Rename + Cascading Side Effects

The recommended approach uses the existing `fs::rename`-based `ProfileStore::rename()` as the core operation, wrapped in an orchestrating Tauri command that handles all cascading effects in a single IPC call.

**Why this approach over alternatives:**

- `fs::rename` is atomic on the same filesystem (profiles always live in `~/.config/crosshook/profiles/`)
- The backend primitive already exists and is tested (`toml_store.rs:163-178`)
- Copy-then-delete introduces partial failure risk the atomic approach avoids
- The "save with old name context" approach would require threading rename semantics through the existing save flow, adding complexity

### Technology Choices

No new dependencies required. All necessary primitives exist:

- **Backend**: `ProfileStore::rename()`, `rename_launcher_files()`, `SettingsStore` for `last_used_profile` updates
- **Frontend**: `invoke()` from `@tauri-apps/api/core`, existing `useProfile` hook patterns
- **Testing**: `tempfile` crate (already a dev dependency), existing test patterns in `toml_store.rs`

**Library evaluation summary** (full details in `research-external.md`):

- `std::fs::rename` — already in use, atomic on Linux (POSIX guarantee), zero dependencies. **Recommended.**
- `renamore` crate — provides `rename_exclusive()` via `renameat2(RENAME_NOREPLACE)` to eliminate TOCTTOU race on overwrite detection. Rejected: overkill for single-user desktop app; requires glibc 2.28+; marginal benefit over a simple `path.exists()` guard.
- `atomic-write-file` / `atomicwrites` — solve atomic writes, not renames. Wrong tool.
- `tempfile::NamedTempFile::persist_noclobber()` — already a dev-dependency but designed for persisting new files, not renaming existing ones.

### Phasing

#### Phase 1: Backend Orchestration (Core)

Enhance the `profile_rename` Tauri command in `src-tauri/src/commands/profile.rs` to:

1. **Add conflict detection** — check if `new_name` already exists before renaming; return an error (or a typed result indicating conflict)
2. **Cascade to settings** — if `last_used_profile == old_name`, update it to `new_name`
3. **Return a result struct** — `RenameProfileResult { new_name, settings_updated }` for the frontend to react to

**Note on launchers**: Launcher cascade is NOT required for profile rename. Launcher paths derive from `steam.launcher.display_name` (via `resolve_display_name()` in `launcher.rs:230`), not the profile filename. Renaming a profile file does not affect exported launchers.

#### Phase 2: Frontend Integration

1. Add `renameProfile` function to `useProfile.ts` following the `duplicateProfile` pattern
2. Add a `renaming` loading state and expose it via `UseProfileResult`
3. Add a "Rename" button to `ProfileActions.tsx` (separate from Save, following UX best practice of not overloading Save with rename semantics)
4. Implement a rename modal dialog with:
   - Pre-filled input with text fully selected (universal rename pattern)
   - Inline validation: empty name, invalid characters (300ms debounce), collision check on blur
   - Error display below input with `role="alert"` for accessibility
   - Gamepad-friendly focus trapping (modal is better than inline edit for controller navigation)
5. After successful rename: refresh profile list, select the renamed profile, update `selectedProfile` and `profileName` state
6. Consider making the profile name field read-only for existing profiles — this eliminates the core UX bug (edit name + save = accidental new profile). The Duplicate button already covers "create copy" and the new Rename button covers name changes.

#### Phase 3: Polish & Edge Cases

1. Undo toast — "Renamed to 'X'" with [Undo] button, 5-8s window (NNGroup recommendation for reversible actions; rename is trivially undoable via reverse rename)
2. F2 keyboard shortcut for rename (matches Linux file manager convention)
3. Save flow disambiguation — detect `profileName !== selectedProfile` and prompt user to choose rename vs. save-as-new
4. Gamepad text input accommodation — Steam Deck users must manually invoke virtual keyboard (Steam+X); dialog layout should accommodate keyboard overlay

### Quick Wins

- **Backend is ~90% done** — the `profile_rename` command is already registered in `lib.rs:96` and calls `store.rename()`. Only the cascading settings update is missing.
- **The `rename_launcher_files()` function** in `launcher_store.rs:367-471` is already fully implemented with write-then-delete strategy, watermark verification, and slug change handling. However, since launcher paths derive from `display_name` (not profile name), this is NOT needed for profile rename — it's only relevant if display name also changes.
- **`LauncherRenameResult` TypeScript type** already exists at `types/launcher.ts:20-29`, matching the Rust struct.
- **Frontend type** — `RenameProfileResult` TypeScript interface is a ~10-line addition to `types/profile.ts`.

### Existing Code Inventory

| Component                      | Status                                  | Location                                   |
| ------------------------------ | --------------------------------------- | ------------------------------------------ |
| `ProfileStore::rename()`       | Working, tested                         | `toml_store.rs:163-178`                    |
| `profile_rename` Tauri command | Registered, working                     | `commands/profile.rs:148-154`, `lib.rs:96` |
| `rename_launcher_files()`      | Working (not needed for profile rename) | `launcher_store.rs:367-471`                |
| `LauncherRenameResult` TS type | Exists                                  | `types/launcher.ts:20-29`                  |
| `validate_name()`              | Working, rejects path traversal         | `toml_store.rs:273-298`                    |

### What's Missing

1. Overwrite protection in `ProfileStore::rename()` (currently silently overwrites)
2. Settings (`last_used_profile`) cascade in the `profile_rename` Tauri command
3. `RenameProfileResult` Rust struct with serde derive
4. Frontend `renameProfile()` function in `useProfile.ts`
5. Save flow detection of `profileName !== selectedProfile` to trigger rename
6. `RenameProfileResult` TypeScript interface in `types/profile.ts`

---

## Competitive Landscape

No Linux game launcher does profile rename well. This is an opportunity for CrossHook to set the standard.

| Launcher    | Rename Support                                         | Lesson for CrossHook                                                                                                                |
| ----------- | ------------------------------------------------------ | ----------------------------------------------------------------------------------------------------------------------------------- |
| **Steam**   | "Set Custom Name" — display-only, no filesystem rename | Simple but limited; doesn't update shortcuts                                                                                        |
| **Bottles** | Settings page text field — **buggy**                   | Issues #2304 (silent failure) and #1392 (desktop entries break after rename). CrossHook must avoid this — cascade must be reliable. |
| **Lutris**  | Configure dialog name field                            | Basic but functional; no cascade handling                                                                                           |
| **Heroic**  | No rename feature                                      | Gap in the market                                                                                                                   |
| **VS Code** | Profiles editor with full CRUD                         | Gold standard for profile management UX                                                                                             |

---

## Improvement Ideas

### Related Features

1. **Smart save detection** — When the user changes `profileName` and hits Save while a profile with the original name exists, detect this as a rename intent rather than creating a duplicate. This eliminates the current pain point described in the feature request.
2. **Batch rename** — If rename works, batch operations (rename multiple profiles matching a pattern) become possible, useful for users who import many community profiles.
3. **Undo support** — Since rename is atomic, an undo operation is trivially another rename in the reverse direction. Consider exposing this.

### Future Enhancements

1. **Stable profile IDs** — Currently profile name = filename = identity. Adding a UUID inside the TOML would decouple identity from display name, making rename a metadata-only change. This is a larger refactor but would simplify all name-dependent operations.
2. **Profile rename history** — Track renames in a lightweight log for debugging launcher staleness issues.
3. **Auto-rename launchers toggle** — Some users may want to rename the profile but keep the old launcher slug. A setting could control whether launcher cascade is automatic.

### Optimization Opportunities

- The current `saveProfile` flow in `useProfile.ts` does `save → syncProfileMetadata → refreshProfiles → loadProfile`. A rename-aware save could skip the full round-trip by updating local state directly after the backend confirms success.

---

## Risk Assessment

### Technical Risks

| Risk                                        | Likelihood | Impact | Mitigation                                                                                                                                                                                                                                                                                     |
| ------------------------------------------- | ---------- | ------ | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **Overwrite existing profile**              | Medium     | High   | Add existence check in Tauri command; return typed error. The current `fs::rename` silently overwrites (confirmed by test at `toml_store.rs:515-529`). This is the highest-severity risk — permanent data loss with no undo.                                                                   |
| **Launcher cascade failure**                | Low        | Medium | Use best-effort pattern from `cleanup_launchers_for_profile_delete`; log warnings but don't fail the rename.                                                                                                                                                                                   |
| **Settings desync**                         | Low        | Medium | Update `last_used_profile` in the same Tauri command, before returning to frontend. If settings save fails, rename still succeeded — log the warning. Without this fix, auto-load-on-startup silently fails (returns `None` from `resolve_auto_load_profile_name`) with no user-visible error. |
| **Frontend state inconsistency**            | Medium     | Medium | After rename, refresh profile list and re-select by new name. Match the `duplicateProfile` pattern which already handles this.                                                                                                                                                                 |
| **Launch optimizations autosave race**      | Low        | Low    | The `useProfile` hook has a debounced autosave (350ms delay) for launch optimizations. If rename fires while a debounce is pending, the autosave could write to the old filename. Mitigate by cancelling the debounce timer before invoking rename.                                            |
| **Race condition on concurrent operations** | Low        | Low    | `ProfileStore` is not synchronized (noted in doc comment at `toml_store.rs:113-115`). Single-user desktop app makes this unlikely.                                                                                                                                                             |

### Integration Challenges

1. **`useProfile.ts` state management** — The hook tracks `selectedProfile` (the last loaded/saved name), `profileName` (the UI field value), and `profileExists` (derived). Rename must update all three atomically. The `loadProfile` callback already handles this correctly if called after rename.
2. **Profile name field as rename trigger** — Currently, changing `profileName` when `profileExists` is true creates ambiguity: is the user renaming or creating a new profile? The UI must disambiguate, possibly with a separate Rename action or by detecting the "name changed but profile exists" state.
3. **Launcher paths are decoupled from profile name** — Launcher file paths are derived from `steam.launcher.display_name` (via `resolve_display_name()` in `launcher.rs:230`), NOT from the profile filename. This means renaming the profile file alone does not break or orphan existing launchers. Launcher cascade is only needed if the user also changes `display_name` — which is a separate concern from profile rename. This significantly reduces the complexity of the launcher cascade: the rename command only needs to update settings, not launchers.
4. **Launcher export panel** — `LauncherExport.tsx` derives launcher paths from the current profile state. After rename, the component re-renders with updated profile data naturally. Since launcher slugs derive from `display_name` (not profile name), no launcher staleness is introduced by profile rename alone.

### Performance Considerations

- `fs::rename` is O(1) on the same filesystem — no performance concerns for the core operation
- Launcher file rewriting involves reading, regenerating, and writing 2 files — negligible cost
- Settings load/save is a single TOML read/write — negligible cost
- Profile list refresh is an `fs::read_dir` — negligible cost

### Security Considerations

- **Path traversal** — `validate_name()` in `toml_store.rs:273-298` already rejects `/`, `\`, `:`, `..`, and other dangerous characters. Both old and new names are validated in `rename()`.
- **Symlink attacks** — `fs::rename` follows symlinks. Since profiles are in a user-owned directory (`~/.config/crosshook/profiles/`), this is acceptable.
- **No privilege escalation** — all operations are user-space file operations.

---

## Alternative Approaches

### Option A: Atomic `fs::rename` + Cascading (Recommended)

**How it works**: `fs::rename` the TOML file, then cascade to settings and launchers in the same Tauri command.

| Dimension  | Assessment                                                                                   |
| ---------- | -------------------------------------------------------------------------------------------- |
| **Pros**   | Atomic core operation; backend primitives exist; matches existing patterns; minimal new code |
| **Cons**   | Cascading failures are possible (mitigated by best-effort pattern)                           |
| **Effort** | Small — ~2-3 hours backend, ~4-6 hours frontend                                              |
| **Risk**   | Low                                                                                          |

### Option B: Copy-then-Delete

**How it works**: Load profile under old name, save under new name, delete old file. Similar to how duplicate works but with source deletion.

| Dimension  | Assessment                                                                                                                    |
| ---------- | ----------------------------------------------------------------------------------------------------------------------------- |
| **Pros**   | Works across filesystems (irrelevant here); familiar pattern from `duplicate()`                                               |
| **Cons**   | Non-atomic — partial failure leaves both or neither; TOML serialization round-trip may reformat file; more code than Option A |
| **Effort** | Medium — ~3-4 hours backend, same frontend effort                                                                             |
| **Risk**   | Medium — partial failure states                                                                                               |

### Option C: Save-Aware Rename (Implicit)

**How it works**: Modify the existing `saveProfile` flow to detect when `profileName` differs from `selectedProfile` and treat it as a rename rather than a new profile creation.

| Dimension  | Assessment                                                                                                                                                      |
| ---------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **Pros**   | No new UI element needed; solves the exact user pain point described in the issue                                                                               |
| **Cons**   | Ambiguous — hard to distinguish "rename" from "save as copy" intent; breaks the current "change name + save = new profile" mental model; complicates save logic |
| **Effort** | Medium — ~4-5 hours total, higher complexity                                                                                                                    |
| **Risk**   | Medium-High — UX ambiguity, regression risk in save flow                                                                                                        |

### Recommendation

**Option A (Atomic rename + cascading)** is the clear winner. It's the simplest, lowest-risk approach that leverages existing backend code. The frontend gets an explicit `renameProfile` action that's unambiguous in intent. Option C could be layered on top later as a UX enhancement (Phase 3) once the core rename machinery is proven.

---

## Task Breakdown Preview

### Phase 1: Backend — Cascading Rename Command

**Estimated complexity: Small**

| Task Group             | Description                                                                                                                          |
| ---------------------- | ------------------------------------------------------------------------------------------------------------------------------------ |
| 1.1 Conflict detection | Add `exists()` or `name_taken()` check to `ProfileStore`; return typed error from `profile_rename` when target name is already taken |
| 1.2 Settings cascade   | In `profile_rename` command, load settings, update `last_used_profile` if it matches old name                                        |
| 1.3 Result struct      | Define `RenameProfileResult` in Rust (serde), return from command with `new_name` and `settings_updated` fields                      |
| 1.4 Backend tests      | Add tests for conflict detection and settings cascade in `commands/profile.rs` tests module                                          |

**Note**: Launcher cascade is NOT needed for profile rename. Launcher paths derive from `steam.launcher.display_name`, not the profile filename. Renaming the profile file does not break or orphan any exported launchers.

**Dependencies**: None — can start immediately.

### Phase 2: Frontend — Rename Integration

**Estimated complexity: Medium**

| Task Group               | Description                                                                                                                                                                                                        |
| ------------------------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| 2.1 TypeScript types     | Add `RenameProfileResult` interface to `types/profile.ts`                                                                                                                                                          |
| 2.2 Hook integration     | Add `renameProfile()` and `renaming` state to `useProfile.ts`; follow `duplicateProfile` pattern; cancel any pending launch optimizations autosave timer before invoking rename                                    |
| 2.3 Rename modal         | New `ProfileRenameDialog` component: pre-filled input (text fully selected), inline validation (300ms debounce for local checks, blur for conflict), `role="alert"` error display, gamepad-friendly focus trapping |
| 2.4 UI action            | Add Rename button to `ProfileActions.tsx` with enable/disable logic (enabled when `profileExists && !saving && !deleting && !duplicating`)                                                                         |
| 2.5 Context wiring       | Expose `renameProfile`, `renaming`, and rename dialog state through `ProfileContext.tsx` and `ProfilesPage.tsx`                                                                                                    |
| 2.6 Read-only name field | Consider making profile name field read-only for existing profiles to prevent the "edit name + save = new profile" bug. Rename and Duplicate buttons cover the two legitimate name-change use cases.               |

**Dependencies**: Phase 1 must be complete.

### Phase 3: UX Polish (P1-P2 priority)

**Estimated complexity: Small-Medium**

| Task Group                   | Description                                                                                                |
| ---------------------------- | ---------------------------------------------------------------------------------------------------------- |
| 3.1 Undo toast               | "Renamed to 'X'" toast with [Undo] button, 5-8s window. Rename is trivially reversible via reverse rename. |
| 3.2 F2 keyboard shortcut     | Trigger rename dialog via F2 when a profile is selected (Linux file manager convention)                    |
| 3.3 Save flow disambiguation | Detect `profileName !== selectedProfile` in save handler; prompt rename vs. save-as-new choice             |
| 3.4 Gamepad optimization     | Accommodate Steam Deck virtual keyboard overlay in rename dialog layout                                    |

**Dependencies**: Phase 2 must be complete.

---

## Key Decisions Needed

1. **Overwrite policy** — Should renaming to an existing profile name be blocked (recommended), require confirmation, or silently overwrite (current behavior)? Options: (a) Add `AlreadyExists` error variant to `ProfileStoreError` and check in `rename()` before `fs::rename`; (b) Add a `force` parameter to `rename()` that preserves the overwrite behavior for internal use while defaulting to safe for user-facing operations. Note: a `path.exists()` check before `fs::rename` has a theoretical TOCTTOU race, but this is acceptable for a single-user desktop app. The `renamore` crate's `renameat2(RENAME_NOREPLACE)` would eliminate this race but is not justified here (see `research-external.md`).
2. **Orchestration layer** — Should cascading effects (settings update) live in the Tauri command (recommended, single IPC call) or be orchestrated by the frontend hook (multiple IPC calls, partial failure risk)?
3. **Rename trigger UX** — The save flow currently calls `persistProfileDraft(profileName, profile)` (line 556 of `useProfile.ts`) which always creates/overwrites the profile at `profileName`, never checking if `profileName !== selectedProfile`. The recommended fix: add a dedicated `renameProfile()` function to the hook, and have the save handler detect the name-changed-on-existing-profile case and call rename before save.
4. **Launcher cascade scope** — Launcher paths derive from `display_name`, not profile name. Profile rename alone does NOT require launcher cascade. Decision: should the rename command optionally cascade to launchers if the display name also changed, or keep this as a separate concern?

---

## Open Questions

1. **Should the profile's internal `game.name` field be updated during rename?** Currently `game.name` is independent of the profile filename/identity. The rename only changes the filename. If users expect the game name to track the profile name, this needs additional logic.
2. **What happens if the user renames a profile while a launch is in progress?** The launch system reads profile data at launch time, so an in-flight launch shouldn't be affected, but the console output might reference the old name.
3. **Should rename be available from the CLI (`crosshook-cli`)?** The CLI currently only has args defined in `args.rs` — a `rename` subcommand would be a natural addition if the core logic lives in `crosshook-core`.
4. **Should the ProfileReviewModal (recently added) be rename-aware?** The `ProfileReviewModal.tsx` component exists but its interaction with rename is unclear without seeing its full implementation.
5. **Should the profile name field become read-only for existing profiles?** This eliminates the root cause of the "rename creates duplicate" bug by making rename an explicit action (Rename button) rather than an implicit side effect of editing the name and saving. The Duplicate button already covers the "save as copy" use case. Trade-off: users lose the ability to "save as new name" directly from the editor.

---

## Related Research

- `docs/plans/profile-rename/research-external.md` — Library evaluation (std::fs::rename vs. renamore, atomicwrites, tempfile)
- `docs/plans/profile-rename/research-ux.md` — UX competitive analysis, accessibility patterns, gamepad considerations
- `docs/plans/profile-rename/research-business.md` — Business logic analysis, domain complexity assessment
- `docs/plans/profile-rename/research-technical.md` — Technical architecture specs, data flow diagrams
