# Code Analysis: launcher-delete

## Executive Summary

The launcher-delete feature integrates across three layers: a new `launcher_store` module in `crosshook-core/src/export/`, thin Tauri IPC wrappers in `src-tauri/src/commands/export.rs`, and frontend extensions to `LauncherExport.tsx`. The existing codebase follows rigid conventions -- display-only error enums with manual `Display` + `Error` impls, `#[serde(default)]` on every IPC field, `tempfile::tempdir()` test isolation, and `map_err(|e| e.to_string())` in every Tauri command. This analysis extracts the exact patterns, function signatures, and integration points needed to implement the feature without architectural deviation.

## Existing Code Structure

### Related Components

- `crates/crosshook-core/src/export/launcher.rs`: Core export logic with the slug derivation chain (`resolve_display_name` -> `sanitize_launcher_slug` -> `combine_host_unix_path`) and file-writing helpers -- the functions the new `launcher_store` module must reuse.
- `crates/crosshook-core/src/export/mod.rs`: Module root (10 lines) -- needs `pub mod launcher_store;` and re-exports of new public types.
- `crates/crosshook-core/src/profile/toml_store.rs`: `ProfileStore` CRUD with `with_base_path()` for test isolation -- the `delete()` method is the cascade trigger point; needs a new `rename()` method.
- `crates/crosshook-core/src/profile/models.rs`: `GameProfile`, `LauncherSection` (with `display_name` and `icon_path`), `SteamSection`, `LaunchSection` -- these are the inputs for deriving launcher slugs from profile data.
- `crates/crosshook-core/src/settings/mod.rs`: `SettingsStore` with `AppSettingsData.last_used_profile` -- must be updated during profile rename.
- `src-tauri/src/commands/export.rs`: Two thin Tauri commands (16 lines total) -- add 4 new commands here.
- `src-tauri/src/commands/profile.rs`: Profile CRUD commands with `map_error` helper -- modify `profile_delete` for cascade, add `profile_rename`.
- `src-tauri/src/lib.rs`: Tauri app setup with `invoke_handler` macro -- register all new commands here.
- `src/components/LauncherExport.tsx`: Export UI panel -- add status indicators, delete button, existence checking.
- `src/hooks/useProfile.ts`: Profile state hook with `deleteProfile()` and `saveProfile()` -- cascade and rename detection points.
- `src/types/profile.ts`: TypeScript type definitions for `GameProfile` and `LaunchMethod`.

### File Organization Pattern

Modules in `crosshook-core` follow a flat-sibling pattern: `export/launcher.rs` (existing logic) sits next to `export/mod.rs` (module root). The new `export/launcher_store.rs` follows this exact pattern. Each module has its own error enum, request/result structs, and `#[cfg(test)] mod tests` block at the bottom. Tauri commands are organized as one file per domain in `src-tauri/src/commands/` and registered in `lib.rs`.

## Implementation Patterns

### Pattern: Display-Only Error Enum

**Description**: Custom error enums with manual `impl Display` and `impl Error`, plus `From<io::Error>` for `?` ergonomics. Never uses `anyhow`. The `Display` impl provides user-facing messages.

**Example**: See `crates/crosshook-core/src/export/launcher.rs` lines 76-108 (`SteamExternalLauncherExportError`), and `crates/crosshook-core/src/profile/toml_store.rs` lines 13-51 (`ProfileStoreError`).

**Apply to**: The new `LauncherStoreError` enum in `launcher_store.rs`. Follow the exact structure:

```rust
#[derive(Debug)]
pub enum LauncherStoreError {
    Io(io::Error),
    // ...other variants
}

impl fmt::Display for LauncherStoreError { /* match arms */ }
impl std::error::Error for LauncherStoreError {}
impl From<io::Error> for LauncherStoreError { /* Self::Io(value) */ }
```

### Pattern: Request/Result IPC Structs

**Description**: All data crossing the Tauri IPC boundary uses `#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]` with `#[serde(default)]` on every field. Field names use `snake_case` and match the frontend TypeScript interfaces exactly.

**Example**: See `crates/crosshook-core/src/export/launcher.rs` lines 10-29 (`SteamExternalLauncherExportRequest` / `Result`), and `crates/crosshook-core/src/install/models.rs` lines 11-45 (`InstallGameRequest` / `InstallGameResult`).

**Apply to**: New `LauncherExistsResult`, `LauncherDeleteResult`, `LauncherRenameRequest`, `LauncherListEntry` structs. Every field must have `#[serde(default)]`.

### Pattern: Thin Tauri Command Adapter

**Description**: Every `#[tauri::command]` is 1-3 lines: delegate to a `crosshook_core` function, map errors via `.map_err(|e| e.to_string())` or a local `map_error` helper. Zero business logic in command handlers. Commands use `State<'_, T>` for managed stores.

**Example**: See `src-tauri/src/commands/export.rs` (entire file, 17 lines):

```rust
#[tauri::command]
pub fn export_launchers(
    request: SteamExternalLauncherExportRequest,
) -> Result<SteamExternalLauncherExportResult, String> {
    export_launchers_core(&request).map_err(|error| error.to_string())
}
```

And `src-tauri/src/commands/profile.rs` lines 4-6 for the `map_error` helper pattern:

```rust
fn map_error(error: ProfileStoreError) -> String {
    error.to_string()
}
```

**Apply to**: New `check_launcher_exists`, `delete_launcher`, `rename_launcher`, `list_launchers` commands. Note: export commands do NOT use `State<'_>` -- they receive all data via request structs. Profile commands use `State<'_, ProfileStore>`. New profile commands (`profile_rename`) should follow the profile pattern with `State`.

### Pattern: Validate-Then-Execute

**Description**: Validation is a separate exported function called from both the frontend (pre-flight) and the main operation (guard). The main function calls `validate()` first, then proceeds.

**Example**: See `crates/crosshook-core/src/export/launcher.rs` line 173 -- `export_launchers()` calls `validate(request)` as its first line.

**Apply to**: New `check_launcher_exists` function should reuse the slug derivation chain to compute paths, then check filesystem existence. This serves as the "validation" step for delete/rename operations.

### Pattern: Testable Store Constructor

**Description**: Stores use `try_new()` for production (BaseDirs resolution) and `with_base_path(PathBuf)` for test isolation. Functions that derive filesystem paths accept path parameters rather than hardcoding locations.

**Example**: See `crates/crosshook-core/src/profile/toml_store.rs` lines 60-75 (`ProfileStore::try_new()` / `with_base_path()`).

**Apply to**: New `launcher_store` functions should accept `target_home_path: &str` as a parameter (matching the existing export pattern) rather than using a store object, since launcher paths are derived from profile data + home path, not from a fixed base directory.

### Pattern: Inline Test Organization

**Description**: Tests live in `#[cfg(test)] mod tests { ... }` at the bottom of each file. They use `tempfile::tempdir()` for filesystem isolation, `fs::read_to_string` + `.contains()` for content assertions, and descriptive snake_case test names.

**Example**: See `crates/crosshook-core/src/export/launcher.rs` lines 546-694 (7 tests) and `crates/crosshook-core/src/profile/toml_store.rs` lines 174-304 (5 tests).

**Apply to**: New `launcher_store.rs` tests. Use `tempdir()` for home path isolation, write fake launcher files, then verify exists/delete/rename/list operations. Also see `src-tauri/src/commands/settings.rs` lines 38-52 for command contract tests (verify function signatures don't drift):

```rust
#[test]
fn command_names_match_expected_ipc_contract() {
    let _ = settings_load as fn(State<'_, SettingsStore>) -> Result<AppSettingsData, String>;
}
```

### Pattern: Frontend Invoke with try/catch

**Description**: All Tauri IPC calls use `invoke<T>('command_name', { params })` wrapped in try/catch. Errors are extracted via `error instanceof Error ? error.message : String(error)`. State is managed with `useState` for local component state and custom hooks for shared state.

**Example**: See `src/components/LauncherExport.tsx` lines 263-279 (`handleExport`):

```typescript
try {
  await invoke<void>('validate_launcher_export', { request });
  const exported = await invoke<SteamExternalLauncherExportResult>('export_launchers', { request });
  setResult(exported);
  setStatusMessage('Launcher export completed.');
} catch (error) {
  setErrorMessage(error instanceof Error ? error.message : String(error));
}
```

And `src/hooks/useProfile.ts` lines 325-363 (`deleteProfile`):

```typescript
try {
  await invoke('profile_delete', { name });
  // ... cascade: clear settings.last_used_profile, refresh list, load next
} catch (err) {
  setError(err instanceof Error ? err.message : String(err));
}
```

**Apply to**: New launcher existence checks on mount (`useEffect` calling `check_launcher_exists`), delete handler, and rename trigger in `saveProfile()`.

## Integration Points

### Files to Create

- `crates/crosshook-core/src/export/launcher_store.rs`: New module containing `check_launcher_exists()`, `delete_launcher_files()`, `rename_launcher_files()`, `list_launcher_files()` functions plus `LauncherStoreError` enum and result structs.

### Files to Modify

- `crates/crosshook-core/src/export/launcher.rs`: Change visibility of 5 private functions to `pub(crate)`:
  - `resolve_display_name` (line 220) -- needed to derive slug from profile data
  - `combine_host_unix_path` (line 274) -- needed to construct expected file paths
  - `write_host_text_file` (line 441) -- needed for rename (rewrite) operations
  - `build_desktop_entry_content` (line 414) -- needed to regenerate .desktop after rename
  - `build_trainer_script_content` (line 296) -- needed to regenerate .sh after rename

- `crates/crosshook-core/src/export/mod.rs`: Add `pub mod launcher_store;` and re-export new public types.

- `crates/crosshook-core/src/profile/toml_store.rs`: Add `rename()` method to `ProfileStore`:

  ```rust
  pub fn rename(&self, old_name: &str, new_name: &str) -> Result<(), ProfileStoreError> {
      let old_path = self.profile_path(old_name)?;
      let new_path = self.profile_path(new_name)?;
      // validate old exists, new does not
      fs::rename(old_path, new_path)?;
      Ok(())
  }
  ```

- `src-tauri/src/commands/export.rs`: Add 4 new Tauri commands:
  - `check_launcher_exists` -- delegates to `launcher_store::check_launcher_exists()`
  - `delete_launcher` -- delegates to `launcher_store::delete_launcher_files()`
  - `rename_launcher` -- delegates to `launcher_store::rename_launcher_files()`
  - `list_launchers` -- delegates to `launcher_store::list_launcher_files()`

- `src-tauri/src/commands/profile.rs`: Modify `profile_delete` to cascade launcher cleanup (best-effort, log failures), add `profile_rename` command.

- `src-tauri/src/lib.rs`: Register all new commands in the `invoke_handler` macro (lines 69-94). Add each new command name in the appropriate section (export section and profile section).

- `src/components/LauncherExport.tsx`: Add launcher status indicator, delete button, existence check on mount/profile change via `useEffect`.

- `src/hooks/useProfile.ts`: Enhance `deleteProfile()` (lines 325-363) to invoke launcher cascade. Enhance `saveProfile()` (lines 295-323) to detect rename scenario (`profileName !== selectedProfile && selectedProfile !== ''`).

- `src/types/profile.ts` or `src/types/index.ts`: Add new TypeScript interfaces for launcher store results.

## Code Conventions

### Naming

- **Rust functions**: `snake_case` -- e.g., `check_launcher_exists`, `delete_launcher_files`
- **Rust types**: `PascalCase` -- e.g., `LauncherStoreError`, `LauncherExistsResult`
- **Rust modules**: `snake_case` -- e.g., `launcher_store`
- **Tauri commands**: `snake_case` matching frontend `invoke()` call names -- e.g., `check_launcher_exists`
- **TypeScript interfaces**: `PascalCase` -- e.g., `LauncherExistsResult`
- **React hooks**: `camelCase` with `use` prefix
- **CSS class names**: `crosshook-*` BEM-like pattern

### Error Handling

- **Rust core**: Custom error enums with `Display` + `Error` + `From<io::Error>`. Never `anyhow`.
- **Tauri commands**: `.map_err(|error| error.to_string())` -- errors are always stringified for IPC.
- **Frontend**: `try {} catch (error) { error instanceof Error ? error.message : String(error) }`.
- **Cascade operations**: Best-effort -- perform primary operation, then attempt secondary, catch and log failures. Primary operation must never be blocked by cascade failure.

### Testing

- **Filesystem isolation**: `tempfile::tempdir()` for every test that touches the filesystem.
- **Content assertions**: `fs::read_to_string(&path).expect("file").contains("expected")`.
- **Permission assertions**: `fs::metadata(&path).permissions().mode() & 0o777` (Unix only, behind `#[cfg(unix)]`).
- **Round-trip tests**: Save, load, verify equality; or create, delete, verify absence.
- **Command contract tests**: Verify function pointer types match expected Tauri command signatures.
- **No test framework on frontend**: Only Rust tests exist (`cargo test -p crosshook-core`).

## Dependencies and Services

### Rust Dependencies (already available)

- `std::fs` -- all file operations (read_to_string, write, remove_file, create_dir_all, rename, metadata)
- `std::path::{Path, PathBuf}` -- path manipulation
- `serde::{Serialize, Deserialize}` -- IPC struct derives
- `tempfile::tempdir` -- test isolation (dev-dependency, already in `Cargo.toml`)
- `directories::BaseDirs` -- home directory resolution (not needed for launcher_store since it receives paths)

### Tauri Dependencies (already available)

- `tauri::State<'_, T>` -- managed state injection for store-based commands
- `crosshook_core::export::*` -- import from core crate

### Frontend Dependencies (already available)

- `@tauri-apps/api/core` -- `invoke<T>()`
- React hooks: `useState`, `useEffect`, `useMemo`, `useCallback`

## Gotchas and Warnings

### Visibility Elevation Risk

The 5 private functions in `launcher.rs` that need `pub(crate)` are currently only tested indirectly through `export_launchers()`. After elevating visibility, they become part of the crate's internal API surface. Any future changes to `resolve_display_name`, `sanitize_launcher_slug`, or `combine_host_unix_path` will affect both export AND lifecycle operations. The slug derivation chain MUST remain identical between export and lifecycle to ensure the same paths are computed.

### Rename is Not Atomic at the Launcher Level

Profile rename via `fs::rename` is atomic at the TOML file level (single filesystem rename). However, launcher "rename" involves: (1) deleting old `.sh` and `.desktop` files, (2) re-exporting with the new name. If the process crashes between (1) and (2), launchers are lost. This is acceptable per the best-effort cascade design, but tests should verify both halves independently.

### Empty Display Name Fallback Chain

`resolve_display_name()` (launcher.rs:220) falls back through: preferred_name -> trainer file stem -> steam app id -> "crosshook-trainer". The launcher_store must use the SAME fallback chain to derive the slug for checking/deleting. If the profile's `steam.launcher.display_name` is empty, the slug comes from the trainer path stem -- meaning changing the trainer path changes the expected launcher file paths.

### Home Path Derivation Complexity

`resolve_target_home_path()` (launcher.rs:465) tries: preferred path -> steam client install path suffix stripping -> `$HOME` env var. The launcher_store must call this same function to ensure it looks for files in the same location the export wrote them. Callers from the frontend must pass both `target_home_path` and `steam_client_install_path` for correct resolution.

### Profile Name vs. Launcher Slug Distinction

Profile names are filesystem-safe identifiers validated by `validate_name()` (e.g., "elden-ring"). Launcher slugs are derived from the display name via `sanitize_launcher_slug()` (e.g., "Elden Ring Deluxe" -> "elden-ring-deluxe"). These are independent namespaces. A profile named "elden-ring" might have launcher slug "god-of-war-ragnarok" if the display name was overridden. The launcher_store must work with slugs derived from profile data, not profile file names.

### Frontend Rename Detection

In `useProfile.ts`, rename is detected by comparing `profileName !== selectedProfile` where both are non-empty and `selectedProfile` is in the `profiles` list. See `saveProfile()` (line 295): the hook saves to the new name but does NOT delete the old profile file. The rename cascade (delete old profile + rename launchers) must be added here. Currently `profileName` is freely editable (line 331 in ProfileEditor.tsx) -- the user types a new name in the input, which sets `profileName` while `selectedProfile` retains the old value.

### Launcher File Path Format

Script path: `{home}/.local/share/crosshook/launchers/{slug}-trainer.sh`
Desktop entry path: `{home}/.local/share/applications/crosshook-{slug}-trainer.desktop`

Note the asymmetry: script files are under `crosshook/launchers/` while desktop entries are under the standard XDG applications directory. The `crosshook-` prefix on the desktop entry filename is intentional for namespace isolation.

### Tauri Command Registration is Order-Sensitive for Readability

The `invoke_handler` macro in `lib.rs` (lines 69-94) groups commands by domain. New export commands should be added after line 71 (after `validate_launcher_export`), and new profile commands after line 86 (after `profile_save`). The macro is not order-sensitive for functionality, but grouping by domain matches the existing convention.

### No Frontend Test Framework

There are no frontend tests. All new IPC contracts should be verified through Rust-side command contract tests (the `fn command_names_match_expected_ipc_contract` pattern in `settings.rs` lines 38-52).

## Task-Specific Guidance

### For Rust Core Tasks (launcher_store.rs)

- Create `crates/crosshook-core/src/export/launcher_store.rs` as a sibling to `launcher.rs`.
- Import from sibling: `use super::launcher::{sanitize_launcher_slug, resolve_display_name, combine_host_unix_path, resolve_target_home_path}` (after they are elevated to `pub(crate)`).
- Functions should accept `display_name: &str`, `target_home_path: &str`, and `steam_client_install_path: &str` as parameters (not a store struct) since there is no persistent state -- paths are computed from profile data.
- The `check_launcher_exists` function computes expected paths and returns a result struct with `script_exists`, `desktop_entry_exists`, `script_path`, `desktop_entry_path` fields.
- The `delete_launcher_files` function removes both files, returning which were actually deleted.
- The `list_launcher_files` function scans `{home}/.local/share/crosshook/launchers/` for `*-trainer.sh` files and `{home}/.local/share/applications/crosshook-*-trainer.desktop` files.
- Error enum pattern: `LauncherStoreError { Io(io::Error), HomePathResolutionFailed }`.
- Tests should create temp directories, write fake launcher files, and verify operations. Use `fs::write` to create test files (no need for actual shell script content).

### For Tauri Command Tasks (commands/export.rs, commands/profile.rs)

- New export commands follow the existing pattern: receive all data via request struct, no `State<'_>`, delegate to core, `map_err(|e| e.to_string())`.
- New profile commands (`profile_rename`) use `State<'_, ProfileStore>` and `State<'_, SettingsStore>`, performing: (1) rename profile TOML, (2) update `last_used_profile` in settings if it matches old name, (3) best-effort launcher rename.
- The cascade in `profile_delete` should: (1) delete profile TOML (existing), (2) attempt launcher cleanup (new -- catch errors, log with `tracing::warn!`).
- Register all new commands in `lib.rs` `invoke_handler` macro. Export commands after line 71, profile commands after line 86.

### For Frontend Tasks (LauncherExport.tsx, useProfile.ts)

- Add a `useEffect` in `LauncherExport.tsx` that calls `check_launcher_exists` on mount and whenever `profile` or `targetHomePath` changes. Store result in local `useState`.
- Add a "Delete Launcher" button using the destructive button style: `background: 'rgba(185, 28, 28, 0.16)'`, `border: '1px solid rgba(248, 113, 113, 0.28)'`, `color: '#fee2e2'` (matching the existing error display on lines 388-400).
- In `useProfile.ts` `deleteProfile()` (lines 325-363): after `invoke('profile_delete', { name })`, the backend cascade handles launcher cleanup automatically. No frontend launcher delete call needed if cascade is in the backend.
- In `useProfile.ts` `saveProfile()` (lines 295-323): detect rename when `profileName.trim() !== selectedProfile && selectedProfile !== ''`. In this case, call `invoke('profile_rename', { oldName: selectedProfile, newName: profileName.trim() })` instead of the normal save flow.
- New TypeScript interfaces should go in `src/types/profile.ts` or a new `src/types/launcher.ts` file, re-exported from `src/types/index.ts`.
- The `LauncherExportProps` interface (line 5-11) already receives `profile`, `method`, `steamClientInstallPath`, and `targetHomePath` -- these are sufficient for computing the existence check request. No new props needed from App.tsx.
