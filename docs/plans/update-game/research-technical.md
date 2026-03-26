# Technical Specifications: update-game

## Executive Summary

The update-game feature runs a Windows update/patch executable against an existing Proton prefix that was established by a prior install-game or profile setup. It reuses the same `proton run` command-building primitives from `crosshook-core::launch::runtime_helpers` and the same prefix-provisioning logic from `crosshook-core::install::service`, but skips profile generation, prefix creation, and executable discovery -- the prefix and profile already exist. The feature is exposed as a new `update` module in `crosshook-core`, a new Tauri IPC command group, a dedicated React hook, and a new `UpdateGamePanel` component rendered within the existing Install page alongside the Install Game panel.

## Architecture Design

### Component Diagram

```
Frontend (React)                                    Backend (Rust)
+-------------------+   invoke('update_game')    +---------------------------+
| UpdateGamePanel   |  ----------------------->  | commands::update           |
|  (component)      |                            |   update_game()            |
|                   |   invoke('validate_       |   validate_update_request()|
|  useUpdateGame    |   update_request')         |                           |
|  (hook)           |  ----------------------->  +---------------------------+
+-------------------+                                      |
       |                                                   v
       |                                      +---------------------------+
       |  profile_load() (existing)           | crosshook_core::update    |
       | -----------------------------------> |   models.rs               |
       |                                      |   service.rs              |
       |  list_proton_installs() (existing)   |   (reuses launch/         |
       | -----------------------------------> |    runtime_helpers,        |
       |                                      |    install/discovery)     |
       +--                                    +---------------------------+
                                                           |
                                              +---------------------------+
                                              | Proton prefix (existing)  |
                                              |  drive_c/...              |
                                              +---------------------------+
```

### Data Flow

1. User selects a saved profile (or types a profile name). The hook calls `profile_load` to resolve the existing prefix path, proton path, and game executable path from the profile.
2. User selects an update executable (e.g. `update.exe`, `patch_v1.2.exe`).
3. Frontend calls `validate_update_request` -- backend checks the update executable exists, the prefix path exists, and the proton path is executable.
4. Frontend calls `update_game` -- backend builds a Proton command using `new_direct_proton_command` from `runtime_helpers`, sets `STEAM_COMPAT_DATA_PATH`, `WINEPREFIX`, and `STEAM_COMPAT_CLIENT_INSTALL_PATH`, runs the update executable, and blocks until it exits.
5. Backend returns `UpdateGameResult` with success/failure status and the log path.
6. The UI shows results. No profile modification is needed -- the update was applied to the existing prefix.

### New Components

- **`crosshook_core::update` module**: Contains `UpdateGameRequest`, `UpdateGameResult`, `UpdateGameError`, `UpdateGameValidationError` models and the `update_game` / `validate_update_request` service functions. Mirrors the `install` module's structure but simpler -- no profile generation, no discovery, no prefix provisioning.
- **`commands::update` (Tauri)**: Thin IPC layer exposing `update_game` and `validate_update_request` as Tauri commands. Follows the pattern from `commands::install`.
- **`useUpdateGame` hook (React)**: State machine for the update flow (idle, preparing, running_updater, complete, failed). Mirrors `useInstallGame` but significantly simpler.
- **`UpdateGamePanel` component (React)**: Form for profile selection, updater executable browse, and execution. Rendered within the existing Install page below the Install Game panel, or as a collapsible section.

### Integration Points

- **`crosshook_core::launch::runtime_helpers`** -- Reused for `new_direct_proton_command`, `apply_host_environment`, `apply_runtime_proton_environment`, `apply_working_directory`, `attach_log_stdio`. No changes needed to this module.
- **`crosshook_core::install::service::provision_prefix`** -- NOT called. Update assumes the prefix already exists. Validation rejects missing prefixes.
- **`crosshook_core::profile::ProfileStore`** -- Used read-only via the existing `profile_load` Tauri command to populate the update form from a saved profile.
- **`commands::steam::list_proton_installs`** -- Reused by the frontend so the user can pick a different Proton version for the update if needed.
- **`src-tauri/src/lib.rs`** -- New command registrations added to `invoke_handler`.
- **`Sidebar.tsx` / `ContentArea.tsx`** -- No new routes needed. The Update Game panel is co-located on the existing Install page, under a separate section heading.

## Data Models

### New Structs (Rust -- `crates/crosshook-core/src/update/models.rs`)

```rust
use serde::{Deserialize, Serialize};
use std::fmt;
use std::path::PathBuf;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct UpdateGameRequest {
    /// The profile name to resolve prefix/proton settings from.
    /// Optional if prefix_path and proton_path are provided directly.
    #[serde(default)]
    pub profile_name: String,

    /// Path to the update/patch executable (.exe).
    #[serde(default)]
    pub updater_path: String,

    /// Proton executable path. Pre-filled from profile, editable by user.
    #[serde(default)]
    pub proton_path: String,

    /// Proton prefix path (compatdata or standalone prefix).
    /// Pre-filled from profile, editable by user.
    #[serde(default)]
    pub prefix_path: String,

    /// Optional Steam client install path for STEAM_COMPAT_CLIENT_INSTALL_PATH.
    #[serde(default)]
    pub steam_client_install_path: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct UpdateGameResult {
    #[serde(default)]
    pub succeeded: bool,

    #[serde(default)]
    pub message: String,

    #[serde(default)]
    pub helper_log_path: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UpdateGameError {
    Validation(UpdateGameValidationError),
    RuntimeUnavailable,
    LogAttachmentFailed { path: PathBuf, message: String },
    UpdaterSpawnFailed { message: String },
    UpdaterWaitFailed { message: String },
    UpdaterExitedWithFailure { status: Option<i32> },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum UpdateGameValidationError {
    UpdaterPathRequired,
    UpdaterPathMissing,
    UpdaterPathNotFile,
    UpdaterPathNotWindowsExecutable,
    ProtonPathRequired,
    ProtonPathMissing,
    ProtonPathNotExecutable,
    PrefixPathRequired,
    PrefixPathMissing,
    PrefixPathNotDirectory,
}
```

### New TypeScript Types (`src/types/update.ts`)

```typescript
export interface UpdateGameRequest {
  profile_name: string;
  updater_path: string;
  proton_path: string;
  prefix_path: string;
  steam_client_install_path: string;
}

export interface UpdateGameResult {
  succeeded: boolean;
  message: string;
  helper_log_path: string;
}

export type UpdateGameValidationError =
  | 'UpdaterPathRequired'
  | 'UpdaterPathMissing'
  | 'UpdaterPathNotFile'
  | 'UpdaterPathNotWindowsExecutable'
  | 'ProtonPathRequired'
  | 'ProtonPathMissing'
  | 'ProtonPathNotExecutable'
  | 'PrefixPathRequired'
  | 'PrefixPathMissing'
  | 'PrefixPathNotDirectory';

export const UPDATE_GAME_VALIDATION_MESSAGES: Record<UpdateGameValidationError, string> = {
  UpdaterPathRequired: 'An updater path is required.',
  UpdaterPathMissing: 'The updater path does not exist.',
  UpdaterPathNotFile: 'The updater path must be a file.',
  UpdaterPathNotWindowsExecutable: 'The updater path must point to a Windows .exe file.',
  ProtonPathRequired: 'A Proton path is required.',
  ProtonPathMissing: 'The Proton path does not exist.',
  ProtonPathNotExecutable: 'The Proton path must be executable.',
  PrefixPathRequired: 'A prefix path is required.',
  PrefixPathMissing: 'The prefix path does not exist.',
  PrefixPathNotDirectory: 'The prefix path must be a directory.',
};

export const UPDATE_GAME_VALIDATION_FIELD: Record<UpdateGameValidationError, keyof UpdateGameRequest | null> = {
  UpdaterPathRequired: 'updater_path',
  UpdaterPathMissing: 'updater_path',
  UpdaterPathNotFile: 'updater_path',
  UpdaterPathNotWindowsExecutable: 'updater_path',
  ProtonPathRequired: 'proton_path',
  ProtonPathMissing: 'proton_path',
  ProtonPathNotExecutable: 'proton_path',
  PrefixPathRequired: 'prefix_path',
  PrefixPathMissing: 'prefix_path',
  PrefixPathNotDirectory: 'prefix_path',
};

export type UpdateGameStage = 'idle' | 'preparing' | 'running_updater' | 'complete' | 'failed';

export interface UpdateGameValidationState {
  fieldErrors: Partial<Record<keyof UpdateGameRequest, string>>;
  generalError: string | null;
}
```

### Modified Structs

None. The existing `GameProfile`, `LaunchRequest`, and `InstallGameRequest` remain unchanged. The update feature reads profiles but does not modify them.

## API Design (Tauri IPC)

### New Commands

#### `validate_update_request`

**Purpose**: Synchronous field-level validation of an update request before running the updater.

**Request**: `UpdateGameRequest` (passed as `request` parameter)

**Response**: `Result<(), String>` -- Ok on success, Err with the validation error message.

**Errors**: Propagated from `UpdateGameValidationError::message()` as a `String`.

**Implementation pattern**: Identical to `validate_install_request` in `commands::install`.

```rust
#[tauri::command]
pub async fn validate_update_request(request: UpdateGameRequest) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        validate_update_request_core(&request).map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| error.to_string())?
}
```

#### `update_game`

**Purpose**: Run a Windows update/patch executable through Proton against an existing prefix. Blocks until the updater process exits.

**Request**: `UpdateGameRequest` (passed as `request` parameter)

**Response**: `Result<UpdateGameResult, String>`

**Errors**:

- Validation errors (updater path missing, proton not executable, prefix not a directory)
- `RuntimeUnavailable` -- no Tokio runtime
- `LogAttachmentFailed` -- cannot create log file
- `UpdaterSpawnFailed` -- Proton cannot start the updater
- `UpdaterWaitFailed` -- cannot monitor the updater process
- `UpdaterExitedWithFailure` -- updater exited with a non-zero status code

**Implementation pattern**: Mirrors `install_game` in `commands::install` but simpler.

```rust
#[tauri::command]
pub async fn update_game(request: UpdateGameRequest) -> Result<UpdateGameResult, String> {
    let log_path = create_log_path("update", &update_log_target_slug(&request.profile_name))?;
    tauri::async_runtime::spawn_blocking(move || {
        update_game_core(&request, &log_path).map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| error.to_string())?
}
```

### Modified Commands

None. Existing commands (`profile_load`, `list_proton_installs`, `default_steam_client_install_path`) are reused as-is from the frontend.

### Command Registration

Add to `src-tauri/src/lib.rs` `invoke_handler`:

```rust
commands::update::update_game,
commands::update::validate_update_request,
```

Add to `src-tauri/src/commands/mod.rs`:

```rust
pub mod update;
```

## Core Service Design (`crosshook_core::update::service`)

The service function follows the same pattern as `install::service::install_game` but without prefix provisioning, discovery, or profile generation.

```rust
pub fn update_game(
    request: &UpdateGameRequest,
    log_path: &Path,
) -> Result<UpdateGameResult, UpdateGameError> {
    validate_update_request(request)?;

    let prefix_path = PathBuf::from(request.prefix_path.trim());
    // Prefix must already exist -- no provisioning
    if !prefix_path.is_dir() {
        return Err(UpdateGameError::Validation(
            UpdateGameValidationError::PrefixPathMissing,
        ));
    }

    let runtime_handle = Handle::try_current()
        .map_err(|_| UpdateGameError::RuntimeUnavailable)?;

    let mut command = build_update_command(request, &prefix_path, log_path)?;
    let mut child = command.spawn().map_err(|error| {
        UpdateGameError::UpdaterSpawnFailed { message: error.to_string() }
    })?;

    let status = runtime_handle.block_on(child.wait()).map_err(|error| {
        UpdateGameError::UpdaterWaitFailed { message: error.to_string() }
    })?;

    if !status.success() {
        return Err(UpdateGameError::UpdaterExitedWithFailure {
            status: status.code(),
        });
    }

    Ok(UpdateGameResult {
        succeeded: true,
        message: "Update completed successfully.".to_string(),
        helper_log_path: log_path.to_string_lossy().into_owned(),
    })
}

fn build_update_command(
    request: &UpdateGameRequest,
    prefix_path: &Path,
    log_path: &Path,
) -> Result<Command, UpdateGameError> {
    // Reuses the same runtime_helpers as install
    let mut command = new_direct_proton_command(request.proton_path.trim());
    command.arg(request.updater_path.trim());
    apply_host_environment(&mut command);
    apply_runtime_proton_environment(
        &mut command,
        &prefix_path.to_string_lossy(),
        request.steam_client_install_path.trim(),
    );
    apply_working_directory(&mut command, "", Path::new(request.updater_path.trim()));
    attach_log_stdio(&mut command, log_path).map_err(|error| {
        UpdateGameError::LogAttachmentFailed {
            path: log_path.to_path_buf(),
            message: error.to_string(),
        }
    })?;
    Ok(command)
}
```

### Validation (`crosshook_core::update::service::validate_update_request`)

Validates only three things (the minimum viable set):

1. **Updater path**: Required, must exist, must be a file, must end in `.exe`
2. **Proton path**: Required, must exist, must be executable
3. **Prefix path**: Required, must exist, must be a directory

Profile name is optional -- used only for log file naming. If the user provides prefix/proton paths directly (e.g. browsing to them), no profile is needed.

## System Constraints

### Proton Prefix Management

The update feature NEVER creates a prefix. It only validates that the prefix directory exists. The prefix must have been established by a prior install-game flow, a Steam launch, or manual setup.

The `resolve_wine_prefix_path` function from `runtime_helpers` handles the `pfx` subdirectory detection automatically -- if the user points to a compatdata root that contains a `pfx/` child, `WINEPREFIX` is set to the `pfx/` path while `STEAM_COMPAT_DATA_PATH` stays at the parent. This is critical for Steam-managed prefixes where the prefix structure is `compatdata/<appid>/pfx/`.

### Environment Variables

The update command sets the same environment as `build_install_command` in `install::service`:

| Variable                                                                    | Source                                                                |
| --------------------------------------------------------------------------- | --------------------------------------------------------------------- |
| `WINEPREFIX`                                                                | Derived from `prefix_path` via `resolve_wine_prefix_path`             |
| `STEAM_COMPAT_DATA_PATH`                                                    | Derived from `prefix_path` via `resolve_compat_data_path`             |
| `STEAM_COMPAT_CLIENT_INSTALL_PATH`                                          | From request or auto-detected via `resolve_steam_client_install_path` |
| `HOME`, `USER`, `LOGNAME`, `SHELL`, `PATH`                                  | From host via `apply_host_environment`                                |
| `DISPLAY`, `WAYLAND_DISPLAY`, `XDG_RUNTIME_DIR`, `DBUS_SESSION_BUS_ADDRESS` | From host via `apply_host_environment`                                |

All Wine/Proton session variables are cleared by `env_clear()` on the command before applying the above, preventing host-session bleed.

### Steam Deck Controller Navigation

The `UpdateGamePanel` component uses the same CSS class conventions (`crosshook-field`, `crosshook-input`, `crosshook-button`, `crosshook-button--secondary`) that the existing `useGamepadNav` hook's focus-zone system recognizes. No changes to gamepad navigation are needed. The panel will be within the same `data-crosshook-focus-zone="content"` zone as the Install Game panel.

### Working Directory

The updater's working directory is set to the parent directory of the updater executable, matching the behavior of `install::service::build_install_command`. This is important because some updaters expect to find sibling files (patches, data) relative to their own location.

## Frontend Design

### Hook: `useUpdateGame`

State machine stages: `idle` -> `preparing` -> `running_updater` -> `complete` | `failed`

The hook is structurally simpler than `useInstallGame` because:

- No prefix path resolution (prefix is provided from the profile or manual entry)
- No post-execution review (no profile generation, no candidate discovery)
- No executable confirmation step

The hook provides:

- `request` / `updateRequest` / `reset` for form state
- `validation` for field-level errors
- `stage` for UI state
- `result` for the update outcome
- `error` for general error display
- `startUpdate` async action
- Derived booleans: `isRunningUpdater`, `isComplete`, `hasFailed`
- `populateFromProfile(profileName)` -- calls `profile_load` and fills prefix_path, proton_path, steam section paths

### Component: `UpdateGamePanel`

Layout mirrors `InstallGamePanel` but with fewer fields:

1. **Profile selector** -- Dropdown of existing profile names (from `profile_list`), or manual entry. Selecting a profile auto-fills Proton path and Prefix path.
2. **Updater EXE** -- File browser with `.exe` filter.
3. **Proton Path** -- Pre-filled from profile, editable. Proton install dropdown (same `ProtonPathField` pattern from `InstallGamePanel`).
4. **Prefix Path** -- Pre-filled from profile, editable. Directory browser.
5. **Status card** -- Stage label, status text, hint text, error display, log path.
6. **Actions** -- "Run Update" button, "Reset Form" button.

### Page Integration

The `UpdateGamePanel` is added to `InstallPage.tsx` below the existing `InstallGamePanel`, separated by a section divider. This avoids adding a new route/tab but keeps the related setup operations co-located.

Alternatively, the Install page could be renamed to "Setup" with two collapsible sections. The `Sidebar.tsx` entry already groups Install under the "Setup" section label, so the naming is consistent.

The `PageBanner` component at the top of the Install page may need a subtitle update to mention both install and update capabilities.

## Codebase Changes

### Files to Create

| Path                                          | Purpose                                                                                 |
| --------------------------------------------- | --------------------------------------------------------------------------------------- |
| `crates/crosshook-core/src/update/mod.rs`     | Module root, re-exports                                                                 |
| `crates/crosshook-core/src/update/models.rs`  | `UpdateGameRequest`, `UpdateGameResult`, `UpdateGameError`, `UpdateGameValidationError` |
| `crates/crosshook-core/src/update/service.rs` | `update_game`, `validate_update_request`, `build_update_command`                        |
| `src-tauri/src/commands/update.rs`            | Tauri IPC commands: `update_game`, `validate_update_request`                            |
| `src/types/update.ts`                         | TypeScript types and validation maps                                                    |
| `src/hooks/useUpdateGame.ts`                  | React hook for update flow state management                                             |
| `src/components/UpdateGamePanel.tsx`          | React UI component for the update form                                                  |

### Files to Modify

| Path                                   | Changes                                                                                                      |
| -------------------------------------- | ------------------------------------------------------------------------------------------------------------ |
| `crates/crosshook-core/src/lib.rs`     | Add `pub mod update;`                                                                                        |
| `src-tauri/src/commands/mod.rs`        | Add `pub mod update;`                                                                                        |
| `src-tauri/src/lib.rs`                 | Register `commands::update::update_game` and `commands::update::validate_update_request` in `invoke_handler` |
| `src/types/index.ts`                   | Add `export * from './update';`                                                                              |
| `src/components/pages/InstallPage.tsx` | Import and render `UpdateGamePanel` below `InstallGamePanel`                                                 |

### Files Unchanged (Reused As-Is)

| Path                                                  | Reuse                                                                                                                                                                |
| ----------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `crates/crosshook-core/src/launch/runtime_helpers.rs` | `new_direct_proton_command`, `apply_host_environment`, `apply_runtime_proton_environment`, `apply_working_directory`, `attach_log_stdio`, `resolve_wine_prefix_path` |
| `src-tauri/src/commands/profile.rs`                   | `profile_load` and `profile_list` called from frontend                                                                                                               |
| `src-tauri/src/commands/steam.rs`                     | `list_proton_installs` called from frontend                                                                                                                          |
| `src/utils/dialog.ts`                                 | `chooseFile`, `chooseDirectory` reused for browse buttons                                                                                                            |
| `src/components/InstallGamePanel.tsx`                 | `InstallField` and `ProtonPathField` -- either reused or extracted to shared components                                                                              |

### Dependencies

No new crate or npm dependencies are needed. The feature reuses:

- `serde` (already in crosshook-core)
- `tokio` (already in crosshook-core)
- `directories` (already in crosshook-core, only if profile-name-to-prefix resolution is added -- but not needed since update uses existing prefix)
- `@tauri-apps/api/core` (already in frontend)
- `@tauri-apps/plugin-dialog` (already in frontend)

## Technical Decisions

### Decision 1: Separate `update` module vs. extending `install`

- **Options**:
  - A. Add update functions to the existing `install` module
  - B. Create a new `update` module in `crosshook-core`
- **Recommendation**: B -- new `update` module
- **Rationale**: The install module has significant complexity around prefix provisioning, executable discovery, and profile generation that update does not need. A separate module keeps each concern focused and follows the existing crate-separation pattern (each domain gets its own module directory). The update models (`UpdateGameRequest`) are intentionally different from `InstallGameRequest` -- no `profile_name` requirement, no `display_name`, no `installed_game_executable_path`, no `trainer_path`.

### Decision 2: UI placement -- new route vs. co-located on Install page

- **Options**:
  - A. Add a new `update` route in the sidebar
  - B. Co-locate as a section within the existing Install page
  - C. Add as a tab within the Install page
- **Recommendation**: B -- co-located section on the Install page
- **Rationale**: The sidebar already groups Install under a "Setup" section. Adding another top-level route increases navigation surface for a feature that is used infrequently (only when a game update arrives). Co-locating keeps related prefix operations together and avoids changes to `AppRoute`, `Sidebar`, `ContentArea`, and the exhaustive switch in `ContentArea.renderPage`. The section is visually separated by a heading and optional divider.

### Decision 3: Profile-driven vs. fully manual

- **Options**:
  - A. Require a profile name, always load from profile
  - B. Allow fully manual entry (prefix path + proton path), optionally populate from profile
  - C. Require profile name but allow overrides
- **Recommendation**: B -- optional profile population with manual fallback
- **Rationale**: Some users may have a prefix that is not backed by a saved CrossHook profile (e.g. a manually created prefix or a Steam-managed prefix). Requiring a profile would block those users. The UI provides a profile dropdown that auto-fills fields, but all fields remain editable and the profile name is optional. This matches the flexibility pattern of the existing `proton_run` launch method where prefix and proton paths are independently configurable.

### Decision 4: Log streaming vs. blocking

- **Options**:
  - A. Stream updater logs to the frontend via Tauri events (like `launch_game`)
  - B. Block and return the log path after completion (like `install_game`)
- **Recommendation**: B -- blocking with log path return
- **Rationale**: The install command already blocks (`spawn_blocking` + `block_on(child.wait())`) and returns the log path. Updates are typically short-lived (seconds to minutes), unlike game launches which run indefinitely. Blocking simplifies the implementation and matches user expectations -- "click Run Update, wait for it to finish, see the result." If future feedback indicates users want real-time log streaming, it can be added as an enhancement using the same `spawn_log_stream` pattern from `commands::launch`.

### Decision 5: Shared field components

- **Options**:
  - A. Duplicate `InstallField` and `ProtonPathField` into `UpdateGamePanel`
  - B. Extract to shared components reusable by both panels
- **Recommendation**: B -- extract to shared components
- **Rationale**: Both `InstallField` and `ProtonPathField` are already well-defined in `InstallGamePanel.tsx`. Extracting them to a shared location (e.g. `src/components/ui/InstallField.tsx` or `src/components/shared/RuntimeFields.tsx`) removes duplication and ensures consistent styling. The extraction is mechanical -- move the components and update imports.

## Relevant Files

| Path                                                  | Description                                                     |
| ----------------------------------------------------- | --------------------------------------------------------------- |
| `crates/crosshook-core/src/install/service.rs`        | Primary pattern to follow for update service implementation     |
| `crates/crosshook-core/src/install/models.rs`         | Primary pattern for update data models                          |
| `crates/crosshook-core/src/launch/runtime_helpers.rs` | Proton command building, env setup, prefix resolution           |
| `crates/crosshook-core/src/launch/env.rs`             | Wine/Proton environment variable constants                      |
| `crates/crosshook-core/src/launch/request.rs`         | Validation patterns for path/file/executable checks             |
| `src-tauri/src/commands/install.rs`                   | Tauri command pattern for blocking install execution            |
| `src-tauri/src/commands/launch.rs`                    | Alternative pattern with log streaming (not recommended for v1) |
| `src-tauri/src/lib.rs`                                | Command registration site                                       |
| `src/hooks/useInstallGame.ts`                         | Hook pattern to follow for `useUpdateGame`                      |
| `src/components/InstallGamePanel.tsx`                 | UI component pattern with `InstallField`, `ProtonPathField`     |
| `src/components/pages/InstallPage.tsx`                | Page where UpdateGamePanel will be rendered                     |
| `src/types/install.ts`                                | TypeScript type pattern for validation maps and stage types     |
| `src/utils/dialog.ts`                                 | File/directory browser utilities                                |

## Edge Cases

- **Prefix path points to compatdata root (has `pfx/` child)**: `resolve_wine_prefix_path` handles this automatically, setting `WINEPREFIX` to the `pfx/` subdirectory. No special handling needed.
- **Prefix path points directly to a wine prefix (no `pfx/` child)**: Also handled by `resolve_wine_prefix_path` -- returns the path as-is.
- **Updater expects sibling data files**: Working directory is set to the updater's parent directory via `apply_working_directory`, so relative paths in the updater resolve correctly.
- **Updater requires user interaction (GUI windows)**: Proton renders the Windows GUI natively. The updater process blocks until the user closes it, same as install. Display env vars (`DISPLAY`, `WAYLAND_DISPLAY`) are forwarded.
- **User picks a profile with `steam_applaunch` method**: The profile's `steam.compatdata_path` should be used as the prefix path, and `steam.proton_path` as the proton path. The `populateFromProfile` hook function handles this by checking `launch.method` and resolving the correct fields.
- **User picks a profile with `native` method**: Update is only meaningful for Proton-based games. The frontend should disable/warn when a native profile is selected.
- **Updater exits with non-zero status**: Reported as `UpdaterExitedWithFailure` with the exit code. The UI shows the error and the log path for diagnosis. The prefix is left as-is (partial update state).
- **Multiple sequential updates**: Each invocation creates a new log file with a timestamp. No cleanup of old logs is performed (matches install behavior).
- **Profile prefix path was changed since profile creation**: The form auto-fills from the profile but all fields are editable. The user can correct the prefix path before running.

## Open Questions

- **Should the update feature support `steam_applaunch` profiles by extracting `compatdata_path` as the prefix?** The technical answer is yes (the data is available in the profile's `steam` section), and the `populateFromProfile` logic in the hook should handle both `proton_run` (uses `runtime.prefix_path`) and `steam_applaunch` (uses `steam.compatdata_path`). This is a UX question about whether to surface that mapping transparently or require the user to browse to the compatdata path manually.
- **Should the console drawer show real-time updater output?** The initial implementation blocks and returns the log path. Adding real-time streaming (via `launch-log` events) would require switching from `spawn_blocking + block_on` to `spawn` with `spawn_log_stream`, which is a larger change. Recommend deferring to a follow-up enhancement if users request it.
- **Should the "Setup" sidebar section label be updated?** Currently it says "Setup" with one child "Install Game". Adding the update feature either means co-locating it under Install Game (making the label slightly misleading) or renaming the page to "Setup" with two sections. This is a UX/naming decision.
