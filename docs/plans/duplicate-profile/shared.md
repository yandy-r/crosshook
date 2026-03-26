# Duplicate Profile

Profile duplication follows CrossHook's three-layer architecture: a `ProfileStore::duplicate()` method in `crosshook-core` composes existing `load()`/`list()`/`save()` primitives to clone a `GameProfile` (which derives `Clone`) under a unique auto-generated name like `"My Profile (Copy)"` or `"My Profile (Copy 2)"`. A thin `profile_duplicate` Tauri IPC command delegates to the core method, and the React `useProfile` hook exposes a `duplicateProfile` callback that invokes the backend, refreshes the profile list, and selects the new duplicate. The critical safety constraint is that `ProfileStore::save()` silently overwrites via `fs::write()` -- the `duplicate()` method must check `list()` before saving to prevent accidental data destruction.

## Relevant Files

- src/crosshook-native/crates/crosshook-core/src/profile/toml_store.rs: ProfileStore with load/save/list/delete/rename; receives duplicate(), generate_unique_copy_name(), strip_copy_suffix(), DuplicateProfileResult
- src/crosshook-native/crates/crosshook-core/src/profile/models.rs: GameProfile struct (Clone, Serialize, Deserialize, Default) -- .clone() enables full deep copy
- src/crosshook-native/crates/crosshook-core/src/profile/mod.rs: Module re-exports; must export DuplicateProfileResult
- src/crosshook-native/crates/crosshook-core/src/profile/exchange.rs: sanitize_profile_name() and derive_import_name() -- name generation precedent
- src/crosshook-native/src-tauri/src/commands/profile.rs: Existing Tauri profile commands; add profile_duplicate following State<ProfileStore> + map_error pattern
- src/crosshook-native/src-tauri/src/lib.rs: Command registration in invoke_handler macro (lines 91-97 profile block)
- src/crosshook-native/src/hooks/useProfile.ts: Profile state hook (~700 lines); add duplicateProfile callback following persistProfileDraft/selectProfile pattern
- src/crosshook-native/src/context/ProfileContext.tsx: Context provider wrapping useProfile; duplicateProfile auto-propagates via spread
- src/crosshook-native/src/types/profile.ts: TypeScript types; add DuplicateProfileResult interface
- src/crosshook-native/src/components/ProfileActions.tsx: Save/Delete buttons; add Duplicate button with canDuplicate guard
- src/crosshook-native/src/components/pages/ProfilesPage.tsx: Profile page wiring context to actions; pass onDuplicate and canDuplicate

## Relevant Patterns

**Store Pattern**: `ProfileStore` is a stateless struct doing filesystem TOML I/O via `base_path: PathBuf`. All methods return `Result<T, ProfileStoreError>`. New methods compose existing primitives. See [toml_store.rs](src/crosshook-native/crates/crosshook-core/src/profile/toml_store.rs) for all CRUD operations.

**Thin Command Layer**: Tauri commands are 1-3 line delegations to ProfileStore methods with `State<'_, ProfileStore>` injection and `.map_err(map_error)` error conversion. See [commands/profile.rs](src/crosshook-native/src-tauri/src/commands/profile.rs) for the exact pattern.

**Result Structs for IPC**: Operations returning rich data use dedicated serde-derived structs (e.g., `LauncherDeleteResult`, `LauncherRenameResult`). The duplicate operation returns `DuplicateProfileResult { name: String, profile: GameProfile }`. See [launcher.ts](src/crosshook-native/src/types/launcher.ts) for TypeScript result type examples.

**Hook State Machine**: `useProfile` manages all profile state transitions. New async operations follow: set loading/saving state -> invoke Tauri command -> refreshProfiles() -> selectProfile(result) -> clear state. See [useProfile.ts](src/crosshook-native/src/hooks/useProfile.ts) `persistProfileDraft` and `selectProfile` methods.

**Filesystem Test Isolation**: Tests use `tempfile::tempdir()` + `ProfileStore::with_base_path()` for isolated filesystem state. Round-trip save/load/assert pattern with inline `#[cfg(test)] mod tests`. See existing tests in [toml_store.rs](src/crosshook-native/crates/crosshook-core/src/profile/toml_store.rs).

**Name Validation**: `validate_name()` rejects empty, `.`, `..`, absolute paths, and `< > : " / \ | ? *`. Parentheses, spaces, and digits are allowed, so generated names like `"My Profile (Copy 2)"` pass validation. See [toml_store.rs:189-214](src/crosshook-native/crates/crosshook-core/src/profile/toml_store.rs).

## Relevant Docs

**docs/plans/duplicate-profile/feature-spec.md**: You _must_ read this when implementing any part of the duplicate feature -- it is the agreed implementation blueprint with architecture, API design, name generation algorithm, phased implementation, and risk assessment.

**CLAUDE.md**: You _must_ read this when making any code changes -- defines workspace separation ("crosshook-core contains all business logic"), code conventions, commit message requirements, and label taxonomy.

**docs/plans/duplicate-profile/research-technical.md**: You _must_ read this when implementing backend logic -- detailed technical specifications including test cases, all 5 technical decisions with rationale, and the 8 files to modify.

**docs/plans/duplicate-profile/research-business.md**: You _must_ read this when implementing name generation or error handling -- 12 business rules, the critical no-overwrite constraint, and edge case table.

**docs/plans/duplicate-profile/research-ux.md**: You _must_ read this when implementing the frontend UI -- button placement, styling, loading states, gamepad behavior, terminology ("Duplicate" not "Copy"/"Clone"), and competitive analysis.

**tasks/lessons.md**: You _must_ read this before implementing UI changes -- prior lessons on gamepad handler exclusions, Tauri plugin permissions, and UI refactoring audit requirements.
