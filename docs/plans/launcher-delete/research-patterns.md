# Pattern Research: launcher-delete

## Overview

This document catalogs the architectural patterns, code conventions, error handling strategies, and testing approaches used in the CrossHook codebase that are directly relevant to implementing the launcher-delete feature. The feature adds a new `launcher_store` module in `crosshook-core/src/export/`, new Tauri IPC commands, modifications to `profile_delete`, a new `profile_rename` command, and frontend changes to `LauncherExport.tsx` and `useProfile.ts`. Every pattern documented below has a concrete code reference to follow.

## Relevant Files

- `/src/crosshook-native/crates/crosshook-core/src/export/launcher.rs`: Existing export logic -- `export_launchers`, `sanitize_launcher_slug`, `resolve_display_name`, `resolve_target_home_path`, `combine_host_unix_path`, `write_host_text_file`, `build_trainer_script_content`, `build_desktop_entry_content`. The new `launcher_store.rs` will import many of these as `pub(crate)`.
- `/src/crosshook-native/crates/crosshook-core/src/export/mod.rs`: Module root for export; re-exports the public API. New `pub mod launcher_store;` and re-exports will be added here.
- `/src/crosshook-native/crates/crosshook-core/src/profile/toml_store.rs`: `ProfileStore` with CRUD methods (`save`, `load`, `list`, `delete`) and `validate_name`. The new `rename` method will be added here.
- `/src/crosshook-native/crates/crosshook-core/src/profile/models.rs`: `GameProfile`, `LauncherSection`, and all nested section structs. Used to derive launcher display name from profile data.
- `/src/crosshook-native/crates/crosshook-core/src/install/models.rs`: Newest domain model pattern -- `InstallGameRequest`, `InstallGameResult`, `InstallGameError`, `InstallGameValidationError` with `message()` methods and serde derives. Reference for structuring `LauncherInfo`, `LauncherDeleteResult`, `LauncherRenameResult`.
- `/src/crosshook-native/crates/crosshook-core/src/install/service.rs`: Newest service pattern -- validation, provisioning, and business logic. Reference for how `launcher_store.rs` should structure its public functions.
- `/src/crosshook-native/src-tauri/src/commands/export.rs`: Tauri command wrappers for export. New `check_launcher_exists`, `delete_launcher`, `rename_launcher`, `list_launchers` commands will be added here.
- `/src/crosshook-native/src-tauri/src/commands/profile.rs`: Tauri commands for profile CRUD. `profile_delete` will be modified to cascade launcher deletion; `profile_rename` will be added here.
- `/src/crosshook-native/src-tauri/src/commands/install.rs`: Newest command pattern using `spawn_blocking` for filesystem-heavy operations. Reference for async command structure.
- `/src/crosshook-native/src-tauri/src/lib.rs`: Tauri app setup with command registration at lines 69-94. All new commands must be registered here.
- `/src/crosshook-native/src/components/LauncherExport.tsx`: Frontend launcher export panel. Will be extended with status indicator, delete/rename buttons.
- `/src/crosshook-native/src/hooks/useProfile.ts`: Profile state management hook. Rename detection logic will be added here.
- `/src/crosshook-native/src/types/profile.ts`: TypeScript types for `GameProfile`. New launcher types may go here or in a new `launcher.ts`.
- `/src/crosshook-native/src/types/install.ts`: Newest TypeScript type file -- reference for how to structure `LauncherInfo`, `LauncherDeleteResult`, etc.

## Architectural Patterns

- **Domain-oriented core modules**: Each feature lives in its own directory under `crosshook-core/src/` with a `mod.rs` that re-exports the public API. The `export/` module currently has `launcher.rs`; the new `launcher_store.rs` will be a sibling. `crosshook-core/src/lib.rs` lists each module as `pub mod export;`, `pub mod profile;`, etc.

- **mod.rs re-export pattern**: Every module directory has a `mod.rs` that declares sub-modules and selectively re-exports public types. See `export/mod.rs` (lines 1-9) which re-exports `export_launchers`, `validate`, and the error/request/result types from `launcher.rs`. The new `launcher_store` types must be re-exported here for Tauri commands to import them.

- **Thin Tauri command adapters**: Tauri command files import domain functions from `crosshook_core` and wrap them in `#[tauri::command]` functions that return `Result<T, String>`. Business logic never lives in the command handlers. The thinnest example is `commands/export.rs` (16 lines total, 2 commands). `commands/profile.rs` is similarly thin with a local `map_error` helper. The `commands/install.rs` file demonstrates the `spawn_blocking` pattern for heavier operations.

- **Managed state via `tauri::State`**: Stores like `ProfileStore`, `SettingsStore`, `RecentFilesStore`, and `CommunityTapStore` are created in `lib.rs` `run()` (lines 15-30), then registered with `.manage()` (lines 62-65). Commands that need them accept `State<'_, ProfileStore>` parameters. Tauri automatically injects these. The launcher-delete feature should NOT need a new managed store -- the `launcher_store` module is stateless (derives paths from slugs). However, `profile_delete` and `profile_rename` commands already receive `State<'_, ProfileStore>`.

- **Request/Result structs for IPC boundaries**: All data crossing the Tauri IPC boundary uses `Serialize` + `Deserialize` derive macros. Request structs use `#[serde(default)]` on every field for defensive deserialization. Result structs use the same pattern. See `SteamExternalLauncherExportRequest` (launcher.rs:10-21), `SteamExternalLauncherExportResult` (launcher.rs:23-29), `InstallGameRequest` (install/models.rs:11-27), `InstallGameResult` (install/models.rs:29-45).

- **Validate-then-execute pattern**: Every domain operation follows `validate(request) -> Result<(), ValidationError>` as a separate step before execution. The validation function is exported independently and called both from the Tauri command (for pre-flight checks from the frontend) and from the main operation function (as a guard). See `export/launcher.rs` `validate()` (line 110) called by `export_launchers()` (line 173), and `install/service.rs` `validate_install_request()` called by `install_game()`.

- **Deterministic path derivation**: Launcher paths are derived from a display name through `resolve_display_name()` -> `sanitize_launcher_slug()` -> `combine_host_unix_path()`. This chain is fully deterministic and stateless. The launcher-delete feature reuses this exact chain. Key functions are currently private in `launcher.rs` and need to be elevated to `pub(crate)`.

- **Hook-owned frontend state**: React components delegate all state management and side effects to custom hooks. `useProfile.ts` manages the entire profile CRUD lifecycle including dirty tracking, error state, and metadata sync. The launcher-delete feature extends this pattern -- rename detection and launcher cascade logic will live in the hook, not the component.

- **Frontend error handling via string propagation**: All `invoke()` calls use `try/catch`, and caught errors are stored as a single `string | null` state variable. The pattern is `error instanceof Error ? error.message : String(error)`. See `useProfile.ts` `saveProfile` (line 318), `deleteProfile` (line 359), and `LauncherExport.tsx` `handleExport` (lines 274-278).

## Code Conventions

### Rust Naming

- `snake_case` for functions, variables, modules, and file names.
- Error enums use `PascalCase` variants: `ProfileStoreError::InvalidName`, `InstallGameValidationError::InstallerPathRequired`.
- Constants use `SCREAMING_SNAKE_CASE`: `METHOD_STEAM_APPLAUNCH`, `METHOD_PROTON_RUN`, `DEFAULT_PREFIX_ROOT_SEGMENT`.
- Tauri command function names are `snake_case` and match the IPC call name exactly: `profile_delete` in Rust is called as `invoke('profile_delete', ...)` in TypeScript.
- Variable names are descriptive and avoid abbreviations. The codebase spells out `character` instead of `ch`, `normalized_root_path` instead of `norm_root`, `last_character_was_separator` instead of `last_sep`.

### Rust Module Organization

- One feature directory per domain under `crosshook-core/src/`: `export/`, `profile/`, `settings/`, `launch/`, `install/`, `community/`, `steam/`.
- Each directory has a `mod.rs` that declares sub-modules and re-exports public API.
- Private implementation details live in separate files within the module directory. For example, `profile/toml_store.rs` and `profile/models.rs` are private modules re-exported through `profile/mod.rs`.
- Tests are inline (`#[cfg(test)] mod tests { ... }`) at the bottom of each implementation file.

### Rust Visibility Pattern

- Functions in `launcher.rs` are currently private (`fn`). The launcher-delete feature needs some elevated to `pub(crate)`: `resolve_display_name`, `combine_host_unix_path`, `build_desktop_entry_content`, `write_host_text_file`, `build_trainer_script_content`, `resolve_desktop_icon_value`. The `sanitize_launcher_slug` is already `pub`.
- The `pub(crate)` visibility is the correct choice for functions shared between sibling modules within `crosshook-core` but not exposed to external consumers.

### Tauri Command File Organization

- One command file per domain: `commands/export.rs`, `commands/profile.rs`, `commands/settings.rs`, `commands/launch.rs`, `commands/install.rs`, `commands/community.rs`, `commands/steam.rs`.
- `commands/mod.rs` simply declares `pub mod` for each file.
- New commands are registered in `lib.rs` inside `tauri::generate_handler![...]` (lines 69-94). Each command uses the full path `commands::export::export_launchers`.

### Tauri Command Registration

- Every new command function must be listed in the `invoke_handler` macro in `lib.rs` (line 69). Forgetting to register a command results in a runtime "command not found" error from the frontend.
- Commands are grouped by module and listed alphabetically within each group. New launcher commands should be added in the `commands::export::` group. The `profile_rename` command should be added in the `commands::profile::` group.

### TypeScript Naming

- `PascalCase` for component names and type/interface names: `GameProfile`, `LauncherExport`, `InstallGameResult`.
- `camelCase` for functions, hooks, and variables: `handleExport`, `deriveLauncherName`, `useProfile`.
- Hooks follow the `use*` prefix convention: `useProfile`, `useLaunchState`, `useCommunityProfiles`, `useGamepadNav`.
- Type files live in `src/types/` with one file per domain and an `index.ts` that re-exports all: `export * from './profile'`, etc.

### TypeScript Interface Conventions

- Interfaces mirror Rust structs field-for-field using `snake_case` property names (matching serde serialization): `{ display_name: string; launcher_slug: string; }`.
- Tauri `invoke()` calls use `camelCase` parameter names: `invoke('profile_load', { name: trimmed })`. Tauri automatically maps camelCase to snake_case for Rust function parameters.
- Union types are used for finite sets of values: `type LaunchMethod = '' | 'steam_applaunch' | 'proton_run' | 'native'`.

### Frontend Component Conventions

- Inline `CSSProperties` objects for component-specific styles. Each component defines `panelStyle`, `inputStyle`, `buttonStyle`, `labelStyle`, `helperStyle`, etc. as `const` at module level.
- Components are exported as both named and default exports: `export function LauncherExport(...) {}` and `export default LauncherExport;`.
- Destructive buttons use a distinct red color scheme: `background: 'rgba(185, 28, 28, 0.16)'`, `border: '1px solid rgba(248, 113, 113, 0.28)'`, `color: '#fee2e2'`. See the error display pattern in `LauncherExport.tsx` (lines 389-400).
- Success/status messages use a green color scheme: `background: 'rgba(16, 185, 129, 0.12)'`, `border: '1px solid rgba(16, 185, 129, 0.28)'`, `color: '#d1fae5'`. See `LauncherExport.tsx` (lines 375-386).

## Error Handling

### Rust Error Enum Pattern (crosshook-core)

The codebase uses custom error enums -- not `anyhow` -- for all domain-level errors. Two patterns exist:

**Pattern A -- Display-only errors (most modules)**: Error enum with manual `Display` implementation and `Error` trait. `From<io::Error>` and `From<toml::*::Error>` impls for `?` operator ergonomics. See `ProfileStoreError` (`toml_store.rs:13-51`), `SettingsStoreError` (`settings/mod.rs:27-62`), `SteamExternalLauncherExportError` (`launcher.rs:76-108`).

**Pattern B -- Serializable errors with `message()` (install module)**: Error enum with `#[derive(Serialize, Deserialize)]` and `#[serde(rename_all = "snake_case")]`, plus a `message()` method returning a human-readable string. See `InstallGameError` (`install/models.rs:47-59`) and `InstallGameValidationError` (`install/models.rs:61-80`). This pattern is newer and better for errors that may need to be returned directly to the frontend as structured data.

**Recommendation for launcher-delete**: Follow Pattern A (Display-only) for `LauncherStoreError` since the Tauri command layer converts all errors to strings via `.to_string()` / `.map_err(|error| error.to_string())` anyway. The error types will be: `LauncherStoreError::Io(io::Error)`, `LauncherStoreError::NotFound(PathBuf)`, etc.

### Validation Error Pattern

Validation errors are separate from operational errors. They provide user-facing messages via a `message()` method. See `SteamExternalLauncherExportValidationError` (`launcher.rs:32-74`) and `InstallGameValidationError` (`install/models.rs:61-193`). Each variant maps to a single actionable sentence.

### Tauri Command Error Propagation

All Tauri commands return `Result<T, String>`. Error conversion happens at the command boundary:

- **Inline map_err**: `export_launchers_core(&request).map_err(|error| error.to_string())` -- see `commands/export.rs:15`.
- **Local helper function**: `fn map_error(error: ProfileStoreError) -> String { error.to_string() }` -- see `commands/profile.rs:4-6`.
- **Generic helper**: `fn map_error(error: impl ToString) -> String { error.to_string() }` -- see `commands/community.rs:8-10`.
- **Double map_err for spawn_blocking**: `spawn_blocking(move || { ... .map_err(|error| error.to_string()) }).await.map_err(|error| error.to_string())?` -- see `commands/install.rs:12-19`.

### Frontend Error Display

Errors are displayed as styled div elements with a red background, not as alerts or toasts. The pattern is consistent across components:

```tsx
{
  errorMessage ? (
    <div
      style={{
        borderRadius: 12,
        padding: 12,
        background: 'rgba(185, 28, 28, 0.16)',
        border: '1px solid rgba(248, 113, 113, 0.28)',
        color: '#fee2e2',
      }}
    >
      {errorMessage}
    </div>
  ) : null;
}
```

See `LauncherExport.tsx` (lines 389-400) and `ProfileEditor.tsx` (lines 720-733).

### Best-Effort Pattern for Cascaded Operations

The feature spec requires that profile operations never be blocked by launcher cleanup failures. The pattern to follow is: perform the primary operation first, then attempt the secondary operation, catching and logging failures. The startup module shows a similar pattern -- `resolve_auto_load_profile_name` returns `None` if anything goes wrong rather than failing the app startup. For `profile_delete`, this means: delete the profile file first, then attempt launcher cleanup, swallowing errors.

## Testing Approach

### Inline Test Organization

All Rust tests are colocated with the implementation in `#[cfg(test)] mod tests { ... }` blocks at the bottom of each file. There are no separate test files or integration test directories for `crosshook-core`.

### tempdir Fixture Pattern

File-backed tests use `tempfile::tempdir()` to create isolated temporary directories. Store constructors accept a custom base path for testing:

- `ProfileStore::with_base_path(temp_dir.path().join("profiles"))` -- `toml_store.rs:73`
- `SettingsStore::with_base_path(temp_dir.path().join("config").join("crosshook"))` -- `settings/mod.rs:83`
- `RecentFilesStore::with_path(temp_dir.path().join("recent.toml"))` -- `recent.rs:82`

The new `launcher_store` functions should accept path parameters (home path, launchers dir) rather than using hardcoded paths, enabling test isolation with `tempdir()`.

### Round-Trip Test Pattern

The most common test shape is: create data, persist it, read it back, assert equality. See `save_load_list_and_delete_round_trip` in `toml_store.rs:215-227`, `save_and_load_round_trip` in `settings/mod.rs:127-142`, and `save_and_load_round_trip_preserves_lists` in `recent.rs:134-153`.

For launcher-delete, the equivalent tests would be: export a launcher, verify files exist, delete the launcher, verify files are gone.

### Validation Test Pattern

Validation tests check specific error variants using `matches!()`:

```rust
assert!(matches!(
    validate_install_request(&request),
    Err(InstallGameValidationError::ProfileNameRequired)
));
```

See `install/service.rs:380-418` for the most comprehensive validation test example.

### Content Assertion Pattern

Tests that generate files assert on file content using `fs::read_to_string` and `contains()` or equality checks. The export test in `launcher.rs:577-654` is the definitive example -- it creates a full request, calls `export_launchers`, then asserts on both file paths and content (script body, desktop entry fields, file permissions).

### Permission Assertion Pattern (Unix)

File permission tests use `#[cfg(unix)]` blocks with `PermissionsExt::mode()`:

```rust
#[cfg(unix)]
{
    use std::os::unix::fs::PermissionsExt;
    let mode = fs::metadata(&path).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode, 0o755);
}
```

See `launcher.rs:637-654`. The launcher-delete tests should verify that renamed files preserve the correct permissions (0o755 for `.sh`, 0o644 for `.desktop`).

### Test Helper Functions

Tests extract reusable setup into helper functions within the `mod tests` block:

- `sample_profile()` -- `toml_store.rs:179-212`
- `valid_request(temp_dir: &Path)` -- `install/service.rs:337-359`
- `write_executable_script(path: &Path, body: &str)` -- `install/service.rs:324-335`
- `create_file(path: &Path)` -- `recent.rs:129-131`
- `store_pair()` -- `startup.rs:72-78`

### Command Contract Tests

Tauri command files include a `command_names_match_expected_ipc_contract` test that casts each command function to its expected type signature. This ensures function signatures don't accidentally change:

```rust
#[test]
fn command_names_match_expected_ipc_contract() {
    let _ = settings_load as fn(State<'_, SettingsStore>) -> Result<AppSettingsData, String>;
    // ...
}
```

See `commands/settings.rs:39-52` and `commands/community.rs:134-161`. New commands added to `commands/export.rs` and `commands/profile.rs` should have their signatures verified in similar contract tests.

## Patterns to Follow

- **Add `launcher_store.rs` as a sibling to `launcher.rs` inside `export/`**: Do not create a new top-level module. The launcher lifecycle management is part of the export domain. Follow the `mod.rs` + sibling-file pattern already used by `launcher.rs`.

- **Elevate private functions to `pub(crate)` in `launcher.rs`**: The functions `resolve_display_name`, `combine_host_unix_path`, `write_host_text_file`, `build_desktop_entry_content`, `build_trainer_script_content`, and `resolve_desktop_icon_value` are currently private. Change them to `pub(crate)` so `launcher_store.rs` can import them. Do NOT make them `pub` -- they should not be part of the external API.

- **Keep Tauri commands in `commands/export.rs` and `commands/profile.rs`**: New launcher commands (`check_launcher_exists`, `delete_launcher`, `rename_launcher`, `list_launchers`) belong in `commands/export.rs`. The `profile_rename` command belongs in `commands/profile.rs`. Do not create a new `commands/launcher.rs` -- the command grouping follows the Rust module structure.

- **Use Display-only error enums for `launcher_store`**: Follow the `SteamExternalLauncherExportError` pattern (launcher.rs:76-108) with variants like `Io(io::Error)`, `NotFound(PathBuf)`, and `From<io::Error>` impl. The Tauri command layer will call `.to_string()` on all errors.

- **Result structs use `#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]`**: All IPC result types follow this standard derive set. `Default` is also common. See `SteamExternalLauncherExportResult` and `InstallGameResult` for examples.

- **Cascade launcher cleanup at the Tauri command level, not in `ProfileStore`**: The `profile_delete` command handler should load the profile, delete it, then attempt launcher cleanup. The `ProfileStore` itself should remain unaware of launchers. This follows the existing separation where `commands/profile.rs` orchestrates operations while `ProfileStore` handles only TOML file I/O.

- **Accept path parameters for testability**: `launcher_store` functions should accept a `home_path: &str` parameter rather than resolving `$HOME` internally. This matches the existing `resolve_target_home_path` pattern and enables isolated testing with `tempdir()`.

- **Register new commands in `lib.rs` `invoke_handler`**: Add all new commands to the `tauri::generate_handler![...]` list in `lib.rs`. Group them with their module siblings:
  - `commands::export::check_launcher_exists`
  - `commands::export::delete_launcher`
  - `commands::export::rename_launcher`
  - `commands::export::list_launchers`
  - `commands::profile::profile_rename`

- **Add TypeScript types in `src/types/`**: Create a new `launcher.ts` file or add types to `profile.ts`. Re-export from `index.ts`. Mirror the Rust struct fields exactly with `snake_case` property names.

- **Frontend state for launcher existence should live in `LauncherExport.tsx`**: Query `check_launcher_exists` on component mount (in a `useEffect`) and expose status to the user. Follow the same `useState` + `invoke` pattern used by `LauncherExport.tsx` `handleExport` for the delete/rename operations.

- **Rename detection in `useProfile.ts`**: Compare `profileName` with `selectedProfile` to detect rename. When `selectedProfile` is non-empty and differs from `profileName`, the user is renaming. Invoke `profile_rename` instead of the current save+no-delete flow. The existing `deleteProfile` callback (lines 325-363) shows the pattern for cascading metadata updates after a destructive operation.

## Edge Cases

- Slug collision: two profiles producing the same slug (e.g., "Elden Ring!" and "Elden Ring?" both become `elden-ring`) means deleting one removes the launcher for both. This is an inherent limitation of the lossy `sanitize_launcher_slug` function and is documented in the feature spec as accepted behavior.
- Stale launchers: if a profile's fields change after export without re-exporting, the launcher file content is stale. The derived slug may also differ from what is on disk. The status indicator should compare expected paths to actual file existence, not launcher content.
- `target_home_path` resolution: The existing `resolve_target_home_path` function (launcher.rs:465-489) tries the explicitly provided path, then derives from the Steam client install path, then falls back to `$HOME`. Backend cascade operations should use the `$HOME` fallback since there is no frontend interaction.
- Profile with no launcher: deleting a profile that was never exported should succeed silently. The delete function should treat `NotFound` on the launcher files as a no-op, not an error.
- Symlink safety: before deleting any file, verify it is a regular file (not a symlink) to prevent symlink-following attacks. Use `fs::symlink_metadata` to check without following symlinks.
- Permission errors: launcher files are in `~/.local/share/` which is normally user-writable. However, if permissions are wrong, the cascade should log the error and continue without blocking the profile operation.

## Other Docs

- `docs/plans/launcher-delete/feature-spec.md`: Full feature specification with business rules, data models, API design, UX workflows, phased task breakdown, and risk assessment.
- `docs/plans/launcher-delete/research-technical.md`: Architecture design, API contracts, codebase change list, system constraints, and technical decisions.
- `docs/plans/launcher-delete/research-business.md`: User stories, business rules, domain model, and workflow analysis.
- `docs/plans/launcher-delete/research-external.md`: Freedesktop .desktop spec, XDG paths, Rust crate evaluation, file operation patterns.
- `docs/plans/launcher-delete/research-ux.md`: UX patterns, competitive analysis, gamepad accessibility, confirmation dialogs.
- `docs/plans/launcher-delete/research-recommendations.md`: Implementation strategy, risk assessment, alternative approaches, phased plan.
- `docs/plans/install-game/research-patterns.md`: Previous pattern research for the install-game feature -- same structural template used here.
