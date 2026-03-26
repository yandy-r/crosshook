# Context Analysis: duplicate-profile

## Executive Summary

Profile duplication (#56) adds a `ProfileStore::duplicate()` method in `crosshook-core` that composes existing `load()`/`list()`/`save()` primitives to clone a `GameProfile` under a unique auto-generated name (`"Name (Copy)"`, `"Name (Copy 2)"`, etc.), exposed via a thin `profile_duplicate` Tauri IPC command, and surfaced as a "Duplicate" button in `ProfileActions`. No new files, dependencies, or error variants are needed -- the feature integrates entirely into existing modules across 8 files.

## Architecture Context

- **System Structure**: Three-layer architecture -- `ProfileStore` (Rust, `crosshook-core`) owns all business logic, Tauri commands (`src-tauri/commands/profile.rs`) are 1-3 line delegations with `State<'_, ProfileStore>` + `map_err(map_error)`, and `useProfile.ts` hook manages all frontend profile state via `ProfileContext`.
- **Data Flow**: User clicks "Duplicate" -> `ProfileActions.tsx` -> `useProfileContext().duplicateProfile(name)` -> `invoke('profile_duplicate', { sourceName })` -> `ProfileStore::duplicate()` -> `load()` + `list()` + `generate_unique_copy_name()` + `save()` -> returns `DuplicateProfileResult { name, profile }` -> frontend calls `refreshProfiles()` + `loadProfile(result.name)` -> UI shows new profile selected.
- **Integration Points**: New code plugs into 4 layers: (1) `ProfileStore` impl block in `toml_store.rs`, (2) `commands/profile.rs` + `lib.rs` handler registration, (3) `useProfile.ts` hook + `UseProfileResult` interface, (4) `ProfileActions.tsx` button + `ProfilesPage.tsx` wiring. `ProfileContext.tsx` auto-propagates via spread -- no changes needed there.

## Critical Files Reference

- `crates/crosshook-core/src/profile/toml_store.rs`: Receives `duplicate()`, `generate_unique_copy_name()`, `strip_copy_suffix()`, `DuplicateProfileResult`. **Critical safety concern**: `save()` at line ~93 does `fs::write()` with no existence guard -- `duplicate()` MUST check `list()` before saving auto-generated names.
- `crates/crosshook-core/src/profile/models.rs`: `GameProfile` derives `Clone` -- `.clone()` gives full deep copy, no field-level logic needed.
- `crates/crosshook-core/src/profile/mod.rs`: Must add `DuplicateProfileResult` to `pub use toml_store::{...}` re-exports.
- `src-tauri/src/commands/profile.rs`: Add `profile_duplicate` command following `State<'_, ProfileStore>` + `map_err(map_error)` pattern. Existing commands at ~line 8-10 show `map_error` helper.
- `src-tauri/src/lib.rs`: Register `profile_duplicate` in `invoke_handler` macro at lines 91-97 (profile command block). `ProfileStore` managed as state at line 62.
- `src/hooks/useProfile.ts`: ~700 line hook. Add `duplicateProfile` callback following `persistProfileDraft`/`selectProfile` async pattern: set `saving` -> invoke -> `refreshProfiles()` -> `loadProfile(result.name)` -> clear state.
- `src/types/profile.ts`: Add `DuplicateProfileResult` interface `{ name: string; profile: GameProfile }`.
- `src/components/ProfileActions.tsx`: Add `canDuplicate`/`onDuplicate` props, render button between Save and Delete with `crosshook-button--secondary` styling.
- `src/components/pages/ProfilesPage.tsx`: Wire `canDuplicate = profileExists && !saving && !deleting && !loading` and `onDuplicate = () => duplicateProfile(selectedProfile)`.

## Patterns to Follow

- **Store Pattern**: `ProfileStore` is stateless (`base_path: PathBuf`), all methods return `Result<T, ProfileStoreError>`, new methods compose existing primitives. See `toml_store.rs`.
- **Thin Command Layer**: Tauri commands are 1-3 lines: receive `State<'_, ProfileStore>`, delegate to store method, `.map_err(map_error)`. See `commands/profile.rs`.
- **Result Structs for IPC**: Rich return types use serde-derived structs (e.g., `LauncherDeleteResult`). Duplicate returns `DuplicateProfileResult { name: String, profile: GameProfile }`. See `types/launcher.ts` for TS examples.
- **Hook State Machine**: Async operations in `useProfile.ts` follow: set loading/saving state -> invoke Tauri command -> `refreshProfiles()` -> `loadProfile(result)` -> clear state. See `persistProfileDraft`.
- **Filesystem Test Isolation**: Tests use `tempfile::tempdir()` + `ProfileStore::with_base_path()`. Round-trip save/load/assert pattern. Inline `#[cfg(test)] mod tests`.
- **Name Validation**: `validate_name()` rejects empty, `.`, `..`, absolute paths, `< > : " / \ | ? *`. Parentheses, spaces, digits are allowed -- `"Name (Copy 2)"` is safe.
- **Error Mapping**: `ProfileStoreError` has `Display`/`Error` impls + `From<io::Error>`, `From<toml::de::Error>`, `From<toml::ser::Error>`. No new variants needed -- existing variants cover all duplicate failure modes.

## Cross-Cutting Concerns

- **No-overwrite safety**: `save()` silently overwrites via `fs::write()`. The `duplicate()` method must guard auto-generated names against `list()`. For explicit names, overwrite is allowed (matches `save()`/`rename()` behavior).
- **Launcher associations**: Duplicated profiles do NOT inherit exported launchers. Two profiles with identical game/trainer data produce the same launcher slug -- exporting from both would overwrite. Acceptable for MVP; document in launcher export UI.
- **Gamepad accessibility**: "Duplicate" button in `ProfileActions` is naturally D-pad navigable alongside Save/Delete. Post-duplicate focus on name field triggers Steam Input on-screen keyboard. Per `tasks/lessons.md`, gamepad handlers must skip editable controls.
- **No frontend test framework**: All UI verification is manual. Rust tests cover backend logic via `cargo test -p crosshook-core`.
- **Autosave interaction**: `useProfile.ts` autosave targets `selectedProfile` name. Auto-selecting the duplicate after creation means autosave correctly targets the new profile.
- **TOML normalization**: Source profile is already saved/normalized. Clone is written verbatim to new filename. Frontend normalizes on load -- no re-normalization needed at backend.
- **Commit conventions**: Must use `feat(profile): ...` for changelog visibility. PR must reference `Closes #56` and follow `.github/pull_request_template.md`.

## Parallelization Opportunities

- **Phase 1 (Backend)**: All Rust changes are sequential within `toml_store.rs` + `mod.rs`. Cannot parallelize.
- **Phase 2 (Wiring)**: Rust command (`commands/profile.rs` + `lib.rs`) and TypeScript type (`types/profile.ts`) can be done in parallel. The `useProfile.ts` hook change depends on the TS type.
- **Phase 3 (UI)**: `ProfileActions.tsx` button and `ProfilesPage.tsx` wiring can be done in parallel if the hook is done.
- **Cross-phase**: Backend (Phase 1) and TypeScript type definition (from Phase 2) are independent and could run in parallel as separate implementation tasks.

## Implementation Constraints

- **Feature spec is authoritative**: `feature-spec.md` is the agreed blueprint. It chose Option A (`ProfileStore::duplicate()` in crosshook-core), overriding `research-recommendations.md` which favored Option B (compose in Tauri command layer). Follow the feature spec.
- **No new files**: All changes go into existing files. No new modules, components, or test files.
- **No new dependencies**: Uses existing `toml`, `serde`, `std::fs`, `@tauri-apps/api/core`.
- **No new error variants**: The loop in `generate_unique_copy_name()` guarantees uniqueness. Existing `ProfileStoreError` variants cover all failure modes.
- **Workspace separation**: All business logic (name generation, conflict detection, suffix stripping) lives in `crosshook-core`, not in Tauri command layer.
- **Terminology**: Use "Duplicate" (not "Copy"/"Clone") per UX research.
- **Button placement**: Between Save and Delete. `crosshook-button--secondary` styling. Disabled when `!profileExists || saving || deleting || loading`.
- **Return type**: `DuplicateProfileResult { name: String, profile: GameProfile }` -- frontend needs both for select + hydrate.
- **Name generation**: `"{base} (Copy)"` -> `"{base} (Copy 2)"` -> ... with suffix stripping to prevent `"Name (Copy) (Copy)"`.

## Key Recommendations

- **Phase the implementation**: Backend first (testable independently), then wiring, then UI. Each phase has a natural verification checkpoint.
- **Test suffix stripping thoroughly**: `strip_copy_suffix()` is the most nuanced logic. Test: `"Name (Copy)"` -> `"Name"`, `"Name (Copy 3)"` -> `"Name"`, `"Name (Copy) extra"` -> unchanged, `"Name"` -> `"Name"`.
- **Check `tasks/lessons.md`** before UI work: prior lessons on gamepad handler exclusions, Tauri plugin permissions, and UI refactoring audit requirements apply.
- **The `DuplicateProfileResult` struct** should be placed near `ProfileStore` in `toml_store.rs` (alongside the store) and exported from `mod.rs`, following the pattern of other result types.
- **Post-duplicate UX**: Auto-select new profile + focus name field with text selected for immediate rename. This matches macOS Finder, VS Code, JetBrains patterns.
