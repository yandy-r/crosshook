# Launcher Lifecycle Management

CrossHook's launcher export system (`export/launcher.rs`) writes `.sh` scripts and `.desktop` entries to deterministic paths derived from a display name via `resolve_display_name()` → `sanitize_launcher_slug()` → `combine_host_unix_path()`, but provides zero lifecycle management — deleting or renaming a profile leaves orphaned launcher files. The new feature introduces a `launcher_store` module in `crosshook-core/src/export/` that discovers, deletes, and renames launcher file pairs by reusing the existing slug derivation chain (requiring several private functions elevated to `pub(crate)`), plus new Tauri IPC commands wired into both the existing `profile_delete` cascade and new `profile_rename` command, and frontend extensions to `LauncherExport.tsx` for status indicators and manual management controls. The architecture follows the established three-layer pattern: business logic in `crosshook-core`, thin IPC wrappers in `src-tauri/commands/`, and React components invoking via `@tauri-apps/api/core`.

## Relevant Files

- src/crosshook-native/crates/crosshook-core/src/export/launcher.rs: Core export logic — `export_launchers()`, `sanitize_launcher_slug()` (pub), `resolve_display_name()` (private, needs pub(crate)), `combine_host_unix_path()` (private, needs pub(crate)), `write_host_text_file()` (private, needs pub(crate)), `build_desktop_entry_content()` (private, needs pub(crate)), `build_trainer_script_content()` (private, needs pub(crate)), `resolve_target_home_path()` (pub)
- src/crosshook-native/crates/crosshook-core/src/export/mod.rs: Module root for export — add `pub mod launcher_store;` and re-export new types
- src/crosshook-native/crates/crosshook-core/src/profile/toml_store.rs: `ProfileStore` with CRUD methods — add `rename()` method using atomic `fs::rename`
- src/crosshook-native/crates/crosshook-core/src/profile/models.rs: `GameProfile`, `LauncherSection` (icon_path, display_name), `SteamSection`, `LaunchSection` (method) — inputs for slug derivation
- src/crosshook-native/crates/crosshook-core/src/profile/mod.rs: Profile module re-exports
- src/crosshook-native/crates/crosshook-core/src/settings/mod.rs: `SettingsStore`, `AppSettingsData` with `last_used_profile` — needs update on rename
- src/crosshook-native/crates/crosshook-core/src/lib.rs: Module root for all core modules
- src/crosshook-native/src-tauri/src/commands/export.rs: Tauri IPC for export — add `check_launcher_exists`, `delete_launcher`, `rename_launcher`, `list_launchers` commands
- src/crosshook-native/src-tauri/src/commands/profile.rs: Tauri IPC for profile CRUD — modify `profile_delete` to cascade, add `profile_rename`
- src/crosshook-native/src-tauri/src/commands/settings.rs: Settings/recent files commands — `last_used_profile` update on rename
- src/crosshook-native/src-tauri/src/lib.rs: Tauri app setup — register all new commands in `invoke_handler`
- src/crosshook-native/src/components/LauncherExport.tsx: Launcher export panel — add status indicator, delete/rename buttons, existence checking on mount
- src/crosshook-native/src/components/ProfileEditor.tsx: Profile editor — delete button triggers `deleteProfile()`, save triggers `saveProfile()`
- src/crosshook-native/src/hooks/useProfile.ts: Profile state hook — `deleteProfile()` (cascade point), `saveProfile()` (rename detection: `profileName !== selectedProfile`)
- src/crosshook-native/src/App.tsx: Main shell — derives `targetHomePath`, passes to `LauncherExport`
- src/crosshook-native/src/types/profile.ts: TypeScript `GameProfile` and `LaunchMethod` types
- src/crosshook-native/src/types/settings.ts: TypeScript `AppSettingsData` with `last_used_profile`
- src/crosshook-native/src/types/index.ts: Type re-exports — add new launcher types
- src/crosshook-native/crates/crosshook-core/src/install/models.rs: Reference for newest request/result struct pattern with serde derives
- src/crosshook-native/crates/crosshook-core/src/install/service.rs: Reference for validate-then-execute pattern and comprehensive test structure

## Relevant Patterns

**Thin Tauri Command Adapters**: Every `#[tauri::command]` delegates immediately to a `crosshook_core` function and maps errors via `.map_err(|e| e.to_string())` or a local `map_error` helper. Business logic never lives in command handlers. See [src/crosshook-native/src-tauri/src/commands/export.rs](src/crosshook-native/src-tauri/src/commands/export.rs) (16 lines, 2 commands).

**Request/Result IPC Structs**: All data crossing the Tauri IPC boundary uses `#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]` with `#[serde(default)]` on every field. See `SteamExternalLauncherExportRequest` in launcher.rs:10-21 and `InstallGameRequest` in install/models.rs:11-27.

**Display-Only Error Enums**: Custom error enums with manual `Display` + `Error` impls and `From<io::Error>` for `?` ergonomics. NOT `anyhow`. See `SteamExternalLauncherExportError` (launcher.rs:76-108) and `ProfileStoreError` (toml_store.rs:13-51).

**Validate-Then-Execute**: Validation is a separate exported function called both from the frontend (pre-flight) and from the main operation (guard). See `validate()` in launcher.rs:110 called by `export_launchers()`.

**Deterministic Path Derivation**: `resolve_display_name()` → `sanitize_launcher_slug()` → `combine_host_unix_path()` chain derives launcher file paths from profile data. This chain MUST be shared between export and lifecycle operations.

**Store Pattern with Testable Constructors**: Stores use `try_new()` / `with_base_path()` for production/test paths. See `ProfileStore::with_base_path()` in toml_store.rs:73. The new `launcher_store` functions should accept path parameters for test isolation.

**Inline Test Organization**: Tests live in `#[cfg(test)] mod tests { ... }` at the bottom of each file with `tempfile::tempdir()` for isolation. Content assertions use `fs::read_to_string` + `contains()`. Command contract tests verify function signatures don't drift.

**Best-Effort Cascade**: Profile operations must never be blocked by launcher cleanup failures. The cascade pattern: perform primary operation first, then attempt secondary, catch and log failures. See startup module for similar error-swallowing pattern.

**Frontend State via Hooks**: React components delegate state to custom hooks. `useProfile.ts` manages all profile CRUD. Launcher status should live as local state in `LauncherExport.tsx` via `useState` + `useEffect` calling `check_launcher_exists` on mount.

**Destructive Button Styling**: Red color scheme: `background: 'rgba(185, 28, 28, 0.16)'`, `border: '1px solid rgba(248, 113, 113, 0.28)'`, `color: '#fee2e2'`. See LauncherExport.tsx error display and ProfileEditor.tsx.

## Relevant Docs

**docs/plans/launcher-delete/feature-spec.md**: You _must_ read this when implementing any launcher lifecycle task — contains all data models, API contracts, UX workflows, phased breakdown, and design decisions.

**CLAUDE.md**: You _must_ read this when adding new modules, Tauri commands, or React components — contains project conventions, architecture overview, and code patterns.

**docs/plans/launcher-delete/research-technical.md**: You _must_ read this when designing the `launcher_store` module or modifying Tauri commands — contains architecture design, system constraints, and 5 technical decisions with rationale.

**docs/plans/launcher-delete/research-patterns.md**: You _must_ read this when writing new Rust code or tests — contains all coding patterns, error handling conventions, and testing approaches with file path references.

**docs/plans/launcher-delete/research-integration.md**: You _must_ read this when connecting frontend to backend — contains complete Tauri IPC command inventory, data model definitions, and frontend component integration points.

**docs/plans/launcher-delete/research-ux.md**: You _must_ read this when implementing frontend UI — contains confirmation dialog patterns, status indicators, gamepad accessibility requirements, and competitive analysis.

**docs/features/steam-proton-trainer-launch.doc.md**: You _must_ read this when working with launcher file content — documents the current launcher output locations, naming patterns, and script structure.

**docs/plans/launcher-delete/research-external.md**: You _must_ read this when implementing file operations — contains Freedesktop .desktop spec requirements, XDG path conventions, and 8 gotchas.
