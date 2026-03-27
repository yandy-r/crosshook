# Profile Rename

Profile rename spans three layers: `ProfileStore::rename()` in `crates/crosshook-core/src/profile/toml_store.rs` handles the atomic `fs::rename` with validation, `profile_rename` in `src-tauri/src/commands/profile.rs` wraps it as a Tauri IPC command (needs `SettingsStore` param added for `last_used_profile` cascade), and `useProfile.ts` manages frontend state (needs a `renameProfile()` callback following the existing `duplicateProfile` pattern). The backend is ~90% implemented — the primary gaps are an `AlreadyExists` overwrite guard in `ProfileStore::rename()`, settings cascade in the Tauri command, and a new Rename button + modal dialog in the frontend. Launcher cascade is NOT needed because launcher paths derive from `display_name`, not profile name.

## Relevant Files

- src/crosshook-native/crates/crosshook-core/src/profile/toml_store.rs: ProfileStore with rename(), validate_name(), ProfileStoreError enum — needs AlreadyExists variant and overwrite guard
- src/crosshook-native/crates/crosshook-core/src/profile/models.rs: GameProfile struct — profile name is filename stem, NOT stored inside TOML. No changes needed
- src/crosshook-native/crates/crosshook-core/src/settings/mod.rs: SettingsStore + AppSettingsData with last_used_profile field — must cascade on rename
- src/crosshook-native/src-tauri/src/commands/profile.rs: Tauri IPC commands — profile_rename at line 148 needs SettingsStore state param and cascade logic
- src/crosshook-native/src-tauri/src/lib.rs: Command registration — profile_rename already registered at line 96. No changes needed
- src/crosshook-native/src-tauri/src/startup.rs: Auto-load reads last_used_profile — works automatically after settings cascade. No changes needed
- src/crosshook-native/src/hooks/useProfile.ts: Profile CRUD hook — add renameProfile() following duplicateProfile() pattern at line 569, cancel autosave timer before rename
- src/crosshook-native/src/components/ProfileActions.tsx: Action bar buttons — add Rename between Duplicate and Delete with canRename/renaming/onRename props
- src/crosshook-native/src/components/ProfileFormSections.tsx: Profile name input at line 323 — make read-only for existing profiles
- src/crosshook-native/src/components/pages/ProfilesPage.tsx: Page orchestrator — wire canRename guard (same as canDuplicate), add rename modal following pendingDelete overlay pattern at line 179
- src/crosshook-native/src/context/ProfileContext.tsx: Context provider — uses spread (...profileState) so new hook fields flow through automatically
- src/crosshook-native/src/types/profile.ts: TypeScript types — no new types needed (rename returns void)
- src/crosshook-native/crates/crosshook-core/src/export/launcher_store.rs: Launcher paths derive from display_name, NOT profile name. No cascade needed

## Relevant Patterns

**Tauri Command with Side Effects**: Profile operations follow a pattern where the primary operation succeeds or fails, and side effects are best-effort with warning logs. See [src-tauri/src/commands/profile.rs](src/crosshook-native/src-tauri/src/commands/profile.rs) `profile_delete` (line 113) which does best-effort launcher cleanup before deletion.

**ProfileStore Method Pattern**: All methods accept `&self` + string name params, call `validate_name()`, resolve path via `profile_path()`, check existence, perform fs operation, return `Result<T, ProfileStoreError>`. See [toml_store.rs](src/crosshook-native/crates/crosshook-core/src/profile/toml_store.rs) `duplicate()` at line 213.

**React Hook IPC Pattern**: Hook callbacks set a loading flag, clear error, invoke IPC, refresh profiles, load target profile, catch/display errors, clear flag. See [useProfile.ts](src/crosshook-native/src/hooks/useProfile.ts) `duplicateProfile()` at line 569 — the exact template for `renameProfile()`.

**UI Action Button Pattern**: ProfileActions uses props interface with boolean flags (saving, deleting, duplicating), capability flags (canSave, canDelete, canDuplicate), and callbacks (onSave, onDelete, onDuplicate). See [ProfileActions.tsx](src/crosshook-native/src/components/ProfileActions.tsx).

**Delete Overlay Dialog Pattern**: ProfilesPage manages a `pendingDelete` state for confirmation overlays with focus trapping and keyboard handling. See [ProfilesPage.tsx](src/crosshook-native/src/components/pages/ProfilesPage.tsx) line 179 — the template for the rename modal.

**Error Enum Convention**: ProfileStoreError uses named variants with Display impl and From impls for wrapped types. See [toml_store.rs](src/crosshook-native/crates/crosshook-core/src/profile/toml_store.rs) line 16.

**ProfileContext Auto-Extension**: ProfileContextValue extends UseProfileResult, so new fields (renaming, renameProfile) surface through context automatically via spread. See [ProfileContext.tsx](src/crosshook-native/src/context/ProfileContext.tsx) line 53.

## Relevant Docs

**docs/plans/profile-rename/feature-spec.md**: You _must_ read this when implementing any part of profile rename — contains architecture overview, API contracts, phased task breakdown, risk assessment, and 4 decisions needing resolution.

**CLAUDE.md**: You _must_ read this when writing code — defines workspace separation, code conventions (Rust snake_case, React PascalCase), commit message requirements, and PR template usage.

**docs/plans/profile-rename/research-technical.md**: You _must_ read this when modifying backend code — contains enhanced API contracts with exact Rust/TypeScript signatures, cross-team synthesis of UX vs technical recommendations.

**docs/plans/profile-rename/research-patterns.md**: You _must_ read this when adding frontend hook/component code — contains concrete implementation checklist derived from codebase patterns with file paths and line numbers.

**docs/plans/profile-rename/research-ux.md**: You _must_ read this when building the rename modal — contains competitive analysis, accessibility requirements (ARIA, keyboard, gamepad), Steam Deck dialog sizing, validation timing patterns.

**docs/plans/duplicate-profile/shared.md**: Reference for the three-layer implementation pattern (core -> command -> hook) used by the most recent comparable feature.

**docs/api/profile-duplicate.md**: Template for documenting the profile_rename API post-implementation.
