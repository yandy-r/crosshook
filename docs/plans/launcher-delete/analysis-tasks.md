# Task Structure Analysis: launcher-delete

## Executive Summary

The launcher lifecycle feature decomposes into 19 discrete tasks across 3 phases, with the critical path running through the new `launcher_store` Rust module, Tauri IPC commands, cascade wiring in `profile_delete`, and frontend status indicators. The codebase's strict three-layer separation (core logic, thin Tauri commands, React hooks/components) creates natural task boundaries at 1-3 files each. Phase 1 achieves the highest user-visible value (automatic cleanup on profile delete + status indicator) with the fewest moving parts; Phase 2 introduces the most complexity (rename requires content regeneration + file moves + UI rename detection); Phase 3 is low-risk polish that can be deferred or dropped without impacting core functionality.

## Recommended Phase Structure

### Phase 1: Foundation + Delete

**Purpose**: Establish the `launcher_store` module, wire cascade delete into `profile_delete`, and surface launcher status in the UI. This delivers the single highest-value behavior (orphaned launchers cleaned up on profile delete) with the minimum viable surface area.

**Suggested Tasks**:

#### Task 1.1: Elevate launcher.rs private functions to pub(crate)

- **Files**: `crates/crosshook-core/src/export/launcher.rs`
- **Work**: Change visibility of `resolve_display_name`, `combine_host_unix_path`, `write_host_text_file`, `build_desktop_entry_content`, `build_trainer_script_content` from `fn` to `pub(crate) fn`. No logic changes.
- **Risk**: Low. Purely additive visibility change. Existing tests remain green.
- **Dependencies**: None (can start immediately)

#### Task 1.2: Create launcher_store module with types and delete logic

- **Files**: `crates/crosshook-core/src/export/launcher_store.rs` (new), `crates/crosshook-core/src/export/mod.rs`
- **Work**: Define `LauncherInfo`, `LauncherDeleteResult`, `LauncherStoreError` structs/enum. Implement `check_launcher_exists(launcher_slug, target_home_path)` and `delete_launcher_files(launcher_slug, target_home_path)`. Add `pub mod launcher_store;` and re-exports to `mod.rs`.
- **Risk**: Medium. Core business logic. Must reuse `combine_host_unix_path` from `launcher.rs` (depends on Task 1.1). Follow `SteamExternalLauncherExportError` pattern for the error enum with manual `Display` + `Error` impls + `From<io::Error>`.
- **Dependencies**: Task 1.1

#### Task 1.3: Write Rust unit tests for check and delete logic

- **Files**: `crates/crosshook-core/src/export/launcher_store.rs` (test module at bottom)
- **Work**: `#[cfg(test)] mod tests` with `tempfile::tempdir()` fixtures. Test cases: check when both files exist, check when neither exists, check when one exists, delete when both exist, delete when neither exists (NotFound is no-op), delete when one is missing. Assert `LauncherDeleteResult` fields.
- **Risk**: Low. Test-only.
- **Dependencies**: Task 1.2

#### Task 1.4: Add TypeScript launcher types

- **Files**: `src/types/launcher.ts` (new), `src/types/index.ts`
- **Work**: Define `LauncherInfo`, `LauncherDeleteResult`, `LauncherRenameResult` interfaces. Add `export * from './launcher'` to index.
- **Risk**: Low. Type definitions only.
- **Dependencies**: None (can start immediately, mirrors Rust types from feature spec)

#### Task 1.5: Add check_launcher_exists and delete_launcher Tauri commands

- **Files**: `src-tauri/src/commands/export.rs`, `src-tauri/src/lib.rs`
- **Work**: Add `check_launcher_exists` and `delete_launcher` commands following the thin-adapter pattern (delegate to `crosshook_core::export::launcher_store::*`, map errors via `.map_err(|e| e.to_string())`). Register both in `invoke_handler` array in `lib.rs`.
- **Risk**: Low. Follows established pattern exactly -- see existing `export_launchers` command (16 lines total in `export.rs`).
- **Dependencies**: Task 1.2

#### Task 1.6: Modify profile_delete to cascade launcher deletion

- **Files**: `src-tauri/src/commands/profile.rs`
- **Work**: In `profile_delete`, before calling `store.delete(&name)`: load the profile data via `store.load(&name)`, derive the launcher slug using `sanitize_launcher_slug(resolve_display_name(...))`, resolve target home path, call `delete_launcher_files` as best-effort (catch and log errors, do not block profile deletion). The profile delete must succeed even if launcher cleanup fails.
- **Risk**: Medium. This is the most critical integration point. Must handle the case where the profile has `native` launch method (skip cleanup). Must handle the case where the profile was never exported (delete returns no-op). Must not change the `profile_delete` return type or error behavior.
- **Dependencies**: Task 1.2, Task 1.5

#### Task 1.7: Add launcher status indicator to LauncherExport.tsx

- **Files**: `src/components/LauncherExport.tsx`
- **Work**: On component mount (and after export), call `check_launcher_exists` via `invoke()` using the current profile's derived launcher slug. Display a colored status badge: "Exported" (green) when both files exist, "Partial" (amber) when one file exists, "Not Exported" (gray) when neither exists. Use `useState` + `useEffect` for status, not a new hook.
- **Risk**: Low-medium. Must correctly derive `launcher_slug` on the frontend side -- the `deriveLauncherName()` helper already exists in `LauncherExport.tsx` but differs slightly from the Rust `resolve_display_name()`. The slug itself uses `sanitize_launcher_slug()` which only exists in Rust. Solution: call `check_launcher_exists` with the `launcher_slug` field from the last export result, or derive it frontend-side by porting the slug logic.
- **Dependencies**: Task 1.4, Task 1.5

**Parallelization**:

- Tasks 1.1 and 1.4 can start immediately in parallel (no dependencies)
- Task 1.2 depends on 1.1 only
- Task 1.3 depends on 1.2
- Task 1.5 depends on 1.2
- Task 1.6 depends on 1.2 and 1.5
- Task 1.7 depends on 1.4 and 1.5
- Maximum parallel lanes: 2 (Rust core track: 1.1 -> 1.2 -> 1.3 | Frontend track: 1.4, then waits)

### Phase 2: Rename + Manual Management

**Purpose**: Add profile rename with launcher cascade, manual launcher delete/rename buttons in the Launcher Export panel, and rename detection in the frontend save flow.

**Dependencies from Phase 1**: `launcher_store` module must exist with `check_launcher_exists` and `delete_launcher_files`. Tauri commands `check_launcher_exists` and `delete_launcher` must be registered. TypeScript types must be defined.

**Suggested Tasks**:

#### Task 2.1: Add ProfileStore::rename method

- **Files**: `crates/crosshook-core/src/profile/toml_store.rs`
- **Work**: Add `pub fn rename(&self, old_name: &str, new_name: &str) -> Result<(), ProfileStoreError>`. Validate both names via existing `validate_name()`. Verify old file exists. Verify new file does not exist (add `AlreadyExists` variant to `ProfileStoreError` if needed). Use atomic `fs::rename` on the `.toml` file.
- **Risk**: Low-medium. The `ProfileStoreError` enum may need a new variant. Must handle the edge case where old_name == new_name (no-op or error).
- **Dependencies**: None within Phase 2 (only Phase 1 completion)

#### Task 2.2: Add rename_launcher_files and list_launchers to launcher_store

- **Files**: `crates/crosshook-core/src/export/launcher_store.rs`
- **Work**: Implement `rename_launcher_files(old_slug, new_display_name, new_icon_path, target_home_path)` using write-then-delete strategy (not `fs::rename`, because `.sh` and `.desktop` embed display names as plaintext content). Reuse `build_trainer_script_content` and `build_desktop_entry_content` from `launcher.rs` via `pub(crate)` visibility. Implement `list_launchers(target_home_path)` to scan `~/.local/share/crosshook/launchers/` for `*-trainer.sh` files and derive `LauncherInfo` for each. Add `LauncherRenameResult` type.
- **Risk**: High. Rename is the most complex operation -- must read old script content to extract the `SteamExternalLauncherExportRequest` fields needed to regenerate the new script. Alternative: accept a full export request as input to rename (simpler, profile data is available at the call site). The feature spec recommends the latter approach with `new_display_name` + `new_launcher_icon_path` parameters.
- **Dependencies**: Task 1.2 (launcher_store module exists)

#### Task 2.3: Write Rust unit tests for rename and list logic

- **Files**: `crates/crosshook-core/src/export/launcher_store.rs` (append to test module)
- **Work**: Test rename when old files exist, rename when old files are missing, rename when new slug collides, rename when slug is unchanged (content-only rewrite). Test list with 0, 1, and multiple launcher scripts.
- **Risk**: Low. Test-only.
- **Dependencies**: Task 2.2

#### Task 2.4: Add profile_rename, rename_launcher, list_launchers Tauri commands

- **Files**: `src-tauri/src/commands/export.rs`, `src-tauri/src/commands/profile.rs`, `src-tauri/src/lib.rs`
- **Work**: Add `rename_launcher` and `list_launchers` to `export.rs`. Add `profile_rename` to `profile.rs` -- this command orchestrates: (1) rename profile TOML via `ProfileStore::rename`, (2) best-effort rename launcher files via `rename_launcher_files`, (3) update `last_used_profile` in settings if it matches the old name. Register all three in `invoke_handler`.
- **Risk**: Medium. `profile_rename` is an orchestrating command that touches 3 stores (ProfileStore, launcher_store, SettingsStore). Must accept `SettingsStore` as a Tauri `State` parameter alongside `ProfileStore`.
- **Dependencies**: Task 2.1, Task 2.2

#### Task 2.5: Add rename detection to useProfile.ts

- **Files**: `src/hooks/useProfile.ts`
- **Work**: In `saveProfile()`, detect rename scenario when `profileName.trim() !== selectedProfile` and `selectedProfile` is non-empty and `profiles.includes(selectedProfile)`. When detected, call `profile_rename` instead of `profile_save` + manual old-profile cleanup. Update `UseProfileResult` interface if new methods are needed.
- **Risk**: Medium. The rename detection logic is subtle -- must distinguish between "rename existing profile" and "save as new profile" (where `selectedProfile` is empty or not in the list). The current `saveProfile` already handles the save path; rename is a new branch.
- **Dependencies**: Task 2.4

#### Task 2.6: Add manual delete button to LauncherExport.tsx

- **Files**: `src/components/LauncherExport.tsx`
- **Work**: Add a "Delete Launcher" button (red destructive style) visible when launcher status is "Exported" or "Partial". Implement inline click-again confirmation pattern (first click changes label to "Click again to confirm", 3-second timeout reverts). On confirm, call `delete_launcher` via `invoke()`, then re-check status.
- **Risk**: Low. Purely frontend. The click-again pattern is simple state machine.
- **Dependencies**: Task 1.5 (delete_launcher command), Task 1.7 (status indicator exists)

#### Task 2.7: Add manual rename button and inline rename notification

- **Files**: `src/components/LauncherExport.tsx`
- **Work**: Add "Rename Launcher" button visible when launcher is exported. When profile name changes and launcher exists, show inline notification panel between name field and Save: "Renaming will update launcher files. Old: [paths] -> New: [paths]". Buttons: "Save and Update Launcher" | "Save Without Updating" | "Cancel".
- **Risk**: Medium. Requires coordination with `useProfile` rename detection (Task 2.5). The inline notification must be aware of both the old slug and new slug. May need a `derive_launcher_slug` helper ported to TypeScript, or a new Tauri command that returns the slug for a given display name.
- **Dependencies**: Task 2.4, Task 2.5, Task 2.6

**Parallelization**:

- Tasks 2.1 and 2.2 can proceed in parallel (independent core modules)
- Task 2.3 depends on 2.2
- Task 2.4 depends on 2.1 and 2.2
- Task 2.5 depends on 2.4
- Task 2.6 depends on Phase 1 only (1.5 and 1.7) -- can start at the beginning of Phase 2
- Task 2.7 depends on 2.4, 2.5, and 2.6
- Maximum parallel lanes: 3 (Profile rename track: 2.1 | Launcher rename track: 2.2 -> 2.3 | Manual delete UI track: 2.6)

### Phase 3: Integration + Polish

**Purpose**: Add stale detection, orphan management, file safety checks, and metadata enrichment. These are read-only or additive enhancements that reduce risk and improve UX but are not required for core functionality.

**Dependencies from Phase 2**: `rename_launcher_files`, `list_launchers`, and `profile_rename` commands must exist. Manual management UI must be in place.

**Suggested Tasks**:

#### Task 3.1: Add X-CrossHook-Profile metadata to exported .desktop files

- **Files**: `crates/crosshook-core/src/export/launcher.rs`
- **Work**: In `build_desktop_entry_content`, append `X-CrossHook-Profile={profile_name}\n` and `X-CrossHook-Slug={slug}\n` lines. This is a non-breaking addition per the Freedesktop spec (implementations must not remove unknown fields). Update existing export test to assert the new lines.
- **Risk**: Low. Additive change to generated content. Existing `.desktop` files without the field continue to work.
- **Dependencies**: None within Phase 3

#### Task 3.2: Add file watermark verification before delete operations

- **Files**: `crates/crosshook-core/src/export/launcher_store.rs`
- **Work**: Before `fs::remove_file`, read the first few lines and verify the `# Generated by CrossHook` comment exists in `.sh` files and `Comment=...Generated by CrossHook` exists in `.desktop` files. If not present, skip deletion and return a warning. Add symlink safety check: verify target is a regular file via `fs::metadata().is_file()`.
- **Risk**: Low. Defensive guard. May need a new `LauncherDeleteResult` field like `skipped_reason: Option<String>` for files that were not deleted due to safety checks.
- **Dependencies**: Task 1.2 (delete logic exists)

#### Task 3.3: Add stale launcher detection

- **Files**: `crates/crosshook-core/src/export/launcher_store.rs`, `src/components/LauncherExport.tsx`
- **Work**: Compare the `Name=` line in an existing `.desktop` file with the current profile's derived display name. If they differ, the launcher is "stale". Expose this as a new field in `LauncherInfo` (`is_stale: bool` or a `LauncherStatus` enum). Update the frontend status badge to show "Stale" (amber) when detected, with an "Update Launcher" button that re-exports.
- **Risk**: Medium. Requires reading and parsing `.desktop` file content. Edge case: if the `.desktop` file has been manually edited, parsing may fail.
- **Dependencies**: Task 1.2, Task 1.7

#### Task 3.4: Add orphan scanner and cleanup UI

- **Files**: `crates/crosshook-core/src/export/launcher_store.rs`, `src/components/SettingsPanel.tsx`
- **Work**: Use `list_launchers` to find all CrossHook launcher scripts. Cross-reference against all profile slugs (via `ProfileStore::list` + loading each profile to derive its slug). Launchers without a matching profile are orphans. Add an "Orphaned Launchers" section to SettingsPanel with a list and bulk delete button.
- **Risk**: Medium-high. Loading every profile to derive slugs is O(n) disk reads. For small profile counts (expected <50) this is fine. The UI in SettingsPanel requires a new section with its own state management.
- **Dependencies**: Task 2.2 (list_launchers exists), Task 2.4 (list_launchers command exists)

#### Task 3.5: Add confirmation modal for profile-delete-with-launcher cascade

- **Files**: `src/components/ProfileEditor.tsx`, `src/hooks/useProfile.ts`
- **Work**: Before calling `profile_delete`, check launcher existence via `check_launcher_exists`. If launchers exist, show a custom React modal: "Delete profile [Name] and its launcher files: [slug]-trainer.sh, crosshook-[slug]-trainer.desktop?" with "Delete Profile and Launcher" (red) and "Cancel" (default focus) buttons. If no launchers, use the existing simple confirmation. Requires adding a modal component (or inline expandable confirmation).
- **Risk**: Medium. Modal component does not currently exist in the codebase -- this is new UI infrastructure. Must support gamepad navigation (focus trap, A=confirm, B=cancel via `useGamepadNav`).
- **Dependencies**: Task 1.5 (check command exists), Task 1.6 (cascade exists)

**Parallelization**:

- Tasks 3.1, 3.2, 3.3, and 3.5 can all proceed in parallel
- Task 3.4 depends on 2.2 and 2.4
- Maximum parallel lanes: 4

## Task Granularity Recommendations

### Appropriate Task Sizes

These tasks are correctly scoped at 1-3 files each and represent a single logical unit of work:

- **Task 1.1** (visibility changes): 1 file, ~5 lines changed, mechanical
- **Task 1.4** (TypeScript types): 2 files, ~30 lines, definition-only
- **Task 1.5** (Tauri commands): 2 files, ~30 lines, follows existing template
- **Task 2.1** (ProfileStore::rename): 1 file, ~30 lines, self-contained method
- **Task 3.1** (metadata in .desktop): 1 file, ~5 lines, additive

### Tasks to Split

- **Task 1.6 (cascade in profile_delete)** could be split if the home path resolution for the cascade is complex. Currently `profile_delete` takes only a `name` and a `State<ProfileStore>`. To cascade, it needs to derive the launcher slug, which requires loading the profile data _and_ resolving the target home path. The target home path is currently frontend-derived (passed to `export_launchers`). Options: (A) accept `target_home_path` as a new parameter to `profile_delete`, (B) resolve it backend-side using `$HOME`. Recommend (B) to avoid breaking the existing command signature, but this design decision should be resolved before implementation starts.

- **Task 2.2 (rename + list)** is dense. Consider splitting into:
  - **Task 2.2a**: `list_launchers` function (read-only scan, simpler)
  - **Task 2.2b**: `rename_launcher_files` function (write operation, more complex)

- **Task 2.7 (rename UI)** touches multiple UI concerns. Consider splitting into:
  - **Task 2.7a**: Inline rename notification panel (awareness-only)
  - **Task 2.7b**: Manual rename button with rename execution

### Tasks to Combine

- **Tasks 1.2 and 1.3** (module creation + tests) could be combined into a single task since writing tests alongside the implementation is natural TDD workflow and touches the same file.
- **Tasks 2.2 and 2.3** (rename/list logic + tests) -- same reasoning.

## Dependency Analysis

### Independent Tasks (Can Run in Parallel)

**Phase 1 parallel groups:**

| Group            | Tasks             | Rationale                        |
| ---------------- | ----------------- | -------------------------------- |
| Rust foundation  | 1.1 -> 1.2 -> 1.3 | Sequential chain, core logic     |
| TypeScript types | 1.4               | No Rust dependency, mirrors spec |

**Phase 2 parallel groups:**

| Group                | Tasks      | Rationale                     |
| -------------------- | ---------- | ----------------------------- |
| Profile rename core  | 2.1        | Independent of launcher_store |
| Launcher rename core | 2.2 -> 2.3 | Independent of ProfileStore   |
| Manual delete UI     | 2.6        | Depends only on Phase 1       |

**Phase 3 parallel groups:**

| Group     | Tasks    | Rationale                |
| --------- | -------- | ------------------------ |
| Safety    | 3.1, 3.2 | Both are additive guards |
| Detection | 3.3      | Read-only check logic    |
| UX        | 3.5      | Modal component          |

### Sequential Dependencies

```
Phase 1 critical path:
  1.1 (visibility) -> 1.2 (core module) -> 1.5 (Tauri commands) -> 1.6 (cascade) -> 1.7 (status UI)

Phase 2 critical path:
  [2.1 (rename method) + 2.2 (rename logic)] -> 2.4 (commands) -> 2.5 (hook detection) -> 2.7 (rename UI)

Phase 3 critical path:
  3.4 (orphan scanner) depends on 2.2 + 2.4
```

### Potential Bottlenecks

1. **Task 1.2 (launcher_store module)**: Every subsequent task depends on this. It is the single most critical task and should be started first. If it takes longer than expected, the entire feature timeline shifts.

2. **Task 2.4 (orchestrating Tauri commands)**: The `profile_rename` command touches 3 Tauri managed states (`ProfileStore`, `SettingsStore`, and launcher_store functions). This is the most complex integration point in Phase 2.

3. **Target home path resolution for cascade delete (Task 1.6)**: The existing `profile_delete` command has no access to `target_home_path`. This design decision blocks cascade implementation. Recommended resolution: use `env::var("HOME")` or `BaseDirs::new()` in the backend, matching how `resolve_target_home_path` falls back in `launcher.rs:481-486`.

4. **Slug derivation on the frontend (Task 1.7, 2.7)**: The frontend needs to know the launcher slug to call `check_launcher_exists`. Currently `sanitize_launcher_slug` only exists in Rust. Options: (A) port it to TypeScript, (B) add a Tauri command `derive_launcher_slug(display_name) -> string`, (C) return the slug from `check_launcher_exists` given profile fields. Recommend (B) for consistency -- a single source of truth for slug derivation.

## File-to-Task Mapping

### Files to Create

| File                                                 | Task | Phase | Dependencies |
| ---------------------------------------------------- | ---- | ----- | ------------ |
| `crates/crosshook-core/src/export/launcher_store.rs` | 1.2  | 1     | Task 1.1     |
| `src/types/launcher.ts`                              | 1.4  | 1     | None         |

### Files to Modify

| File                                              | Task          | Phase | Changes                                                       |
| ------------------------------------------------- | ------------- | ----- | ------------------------------------------------------------- |
| `crates/crosshook-core/src/export/launcher.rs`    | 1.1           | 1     | 5 functions: `fn` -> `pub(crate) fn`                          |
| `crates/crosshook-core/src/export/launcher.rs`    | 3.1           | 3     | Add `X-CrossHook-*` lines to `build_desktop_entry_content`    |
| `crates/crosshook-core/src/export/mod.rs`         | 1.2           | 1     | Add `pub mod launcher_store;` + re-exports                    |
| `crates/crosshook-core/src/profile/toml_store.rs` | 2.1           | 2     | Add `rename()` method, possibly `AlreadyExists` error variant |
| `src-tauri/src/commands/export.rs`                | 1.5, 2.4      | 1, 2  | Add 4 Tauri commands total                                    |
| `src-tauri/src/commands/profile.rs`               | 1.6, 2.4      | 1, 2  | Modify `profile_delete` cascade; add `profile_rename`         |
| `src-tauri/src/lib.rs`                            | 1.5, 2.4      | 1, 2  | Register new commands in `invoke_handler` array               |
| `src/types/index.ts`                              | 1.4           | 1     | Add `export * from './launcher'`                              |
| `src/components/LauncherExport.tsx`               | 1.7, 2.6, 2.7 | 1, 2  | Status badge, delete button, rename button/notification       |
| `src/hooks/useProfile.ts`                         | 2.5           | 2     | Rename detection in `saveProfile()`                           |
| `src/components/ProfileEditor.tsx`                | 3.5           | 3     | Confirmation modal before delete                              |
| `src/components/SettingsPanel.tsx`                | 3.4           | 3     | Orphaned launchers section                                    |

## Optimization Opportunities

### Maximize Parallelism

1. **Start Task 1.4 (TypeScript types) immediately** alongside Task 1.1. These have zero dependencies on each other and can be reviewed/merged independently.

2. **Start Task 2.6 (manual delete button) at the beginning of Phase 2**, since it only depends on Phase 1 outputs (the `delete_launcher` command already exists). Do not wait for Phase 2's rename tasks.

3. **Overlap Phase 2 and Phase 3**: Tasks 3.1 (metadata) and 3.2 (watermark verification) depend only on Phase 1 outputs. They could start during Phase 2 on a separate track.

4. **Pair Rust core + tests**: Implement and test each function in the same task/PR rather than separating them. This reduces context switching and ensures tests are never forgotten.

### Minimize Risk

1. **Resolve the target_home_path design question first** (see Bottleneck #3). This is a blocking architectural decision for Task 1.6. Recommend: add a `resolve_launcher_home_path()` utility in `launcher_store.rs` that calls `BaseDirs::new()` or `env::var("HOME")`, matching the existing fallback in `launcher.rs`.

2. **Implement the `# Generated by CrossHook` watermark check early** (Task 3.2). Even though it is Phase 3 in the spec, moving it to Phase 1 as part of Task 1.2 is low-effort and prevents the worst-case scenario (deleting a user-created `.desktop` file).

3. **Add a `derive_launcher_slug` Tauri command** during Phase 1 (as part of Task 1.5). This avoids duplicating slug logic in TypeScript and prevents slug derivation drift between frontend and backend.

4. **Keep profile_delete backward-compatible**: The cascade in Task 1.6 must not change the command signature, return type, or error behavior. Launcher cleanup failures are logged warnings, never propagated errors.

5. **Feature-flag the cascade** during development: consider a settings flag `auto_delete_launchers: bool` (default true) so the cascade can be disabled if bugs are discovered post-release without requiring a code change.

## Implementation Strategy Recommendations

### Recommended Implementation Order

```
Week 1: Phase 1 (Foundation + Delete)
  Day 1: Task 1.1 (visibility) + Task 1.4 (TS types) -- parallel
  Day 1-2: Task 1.2 + 1.3 (launcher_store module + tests)
  Day 2: Task 1.5 (Tauri commands)
  Day 3: Task 1.6 (cascade in profile_delete)
  Day 3: Task 1.7 (status indicator in LauncherExport.tsx)
  -- PR review and merge --

Week 2: Phase 2 (Rename + Manual Management)
  Day 1: Task 2.1 (ProfileStore::rename) + Task 2.2 (rename/list logic) + Task 2.6 (manual delete UI) -- parallel
  Day 2: Task 2.3 (rename tests) + Task 2.4 (Tauri commands)
  Day 3: Task 2.5 (rename detection in useProfile)
  Day 3-4: Task 2.7 (rename UI in LauncherExport)
  -- PR review and merge --

Week 3: Phase 3 (Polish)
  Tasks 3.1-3.5 in any order, parallel where possible
  -- PR review and merge --
```

### PR Strategy

- **Phase 1**: Single PR. All 7 tasks form a cohesive unit. The PR is reviewable because the new module is self-contained and the cascade is best-effort.
- **Phase 2**: Consider splitting into 2 PRs: (A) Rust core + commands (Tasks 2.1-2.4), (B) Frontend (Tasks 2.5-2.7). This allows backend review before frontend integration.
- **Phase 3**: One PR per task or group of related tasks (3.1+3.2 together as "safety", 3.3 alone, 3.4 alone, 3.5 alone).

### Key Design Decisions to Lock Before Starting

1. **How does `profile_delete` cascade resolve `target_home_path`?** Recommendation: backend-resolved via `BaseDirs::data_dir()` or `env::var("HOME")`.
2. **How does the frontend derive launcher slugs?** Recommendation: new `derive_launcher_slug` Tauri command rather than porting slug logic to TypeScript.
3. **Should `rename_launcher_files` accept a full export request or just display name + icon?** Recommendation: accept `SteamExternalLauncherExportRequest` so it can regenerate full script content without parsing the old script. The profile data is available at the Tauri command call site.
4. **Should watermark verification be Phase 1 or Phase 3?** Recommendation: Phase 1, as part of Task 1.2. The cost is ~10 lines; the safety benefit is significant.
