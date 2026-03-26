# Business Logic Research: Update Game

## Executive Summary

CrossHook already provides a complete "Install Game" feature that runs a Windows installer through Proton into a managed prefix, discovers the game executable, and produces a reviewable profile. Adding an "Update Game" capability extends this mechanism so users can apply game patches and updates to an **existing** profile's Proton prefix without rebuilding the profile from scratch. The core business value is eliminating the manual script-based workflow that currently requires users to construct and execute custom shell commands to run an update executable against the correct WINEPREFIX.

## User Stories

### Primary User: Linux Gamer / Steam Deck User

- As a Linux gamer with a Proton-based game managed by CrossHook, I want to apply a game update or patch through CrossHook so that I do not need to manually construct a Proton command line pointing at the correct prefix.
- As a Steam Deck user, I want to apply game updates from the same Install page through a controller-friendly UI so that the update workflow mirrors the install workflow I already know.
- As a user who maintains multiple game profiles, I want to select an existing profile and have CrossHook automatically resolve the prefix path and Proton version for the update so that I never target the wrong prefix by mistake.
- As a user applying sequential patches (e.g., v1.0 -> v1.1 -> v1.2), I want a predictable workflow that preserves my profile settings between updates so that I do not need to reconfigure my trainer, injection DLLs, or launch method after each patch.

### Secondary User: Power User / Modder

- As a power user, I want the option to override the Proton version or prefix path at update time so that I can test compatibility without permanently changing my profile.
- As a modder, I want to see the update log output in the console drawer so that I can diagnose update failures.

## Business Rules

### Core Rules

1. **Profile Selection Required**: An update must target an existing saved profile. The profile provides the prefix path and Proton path. A profile that has never been saved (only exists as a draft) cannot be updated.
   - Validation: The selected profile name must exist in `ProfileStore.list()`.
   - Exception: None. Unlike "Install Game" which creates a new profile, "Update Game" always operates on an existing one.

2. **Prefix Must Exist**: The profile's prefix path (from `runtime.prefix_path` or derived from `steam.compatdata_path`) must already exist on disk. Updates do not create prefixes; the install step did that.
   - Validation: `Path::is_dir()` on the resolved prefix path.
   - Exception: If the prefix was deleted externally, the user must reinstall or manually restore it first.

3. **Update Executable Must Be a Valid Windows .exe**: The update/patch executable must pass the same `is_windows_executable()` check used by the install flow.
   - Validation: File exists, is a file, has `.exe` extension.
   - Exception: None.

4. **Profile Settings Are Read-Only During Update**: The update process must NOT modify the saved profile. The update executable runs against the prefix, but the profile TOML on disk stays unchanged. If the update changes the game executable location (e.g., moves it to a different directory), the user must manually update the profile afterward.
   - Validation: No `profile_save` call is made as part of the update flow.
   - Exception: A future enhancement could detect executable relocation and prompt the user, but this is out of scope for the initial feature.

5. **Proton Path Resolution**: The Proton executable used for the update should default to the profile's configured Proton path (`runtime.proton_path` for `proton_run`; `steam.proton_path` for `steam_applaunch`), but may be overridden by the user.
   - Validation: Same `is_executable_file()` check as the install flow.
   - Exception: If the profile uses `native` launch method, the Update Game feature is not applicable and should be hidden or disabled.

6. **Single Update at a Time**: Only one update operation can run at a time, matching the install flow's pattern where the installer is awaited synchronously via `block_on(child.wait())`.
   - Validation: UI disables the "Apply Update" button while an update is running.
   - Exception: None.

7. **Update Applies to `proton_run` and `steam_applaunch` Profiles Only**: Profiles with `launch.method == "native"` do not have a Proton prefix and cannot be updated through this mechanism.
   - Validation: Filter or disable the profile selector to exclude native profiles.
   - Exception: None.

### Edge Cases

- **Prefix Deleted Externally**: If the user deleted the prefix directory outside CrossHook, the update validation should catch this early and display a clear error: "The prefix path does not exist. Reinstall the game or browse to a valid prefix."
- **Game Is Running**: CrossHook does not currently track running processes. If the game is running when an update is applied, the update may fail or corrupt game files. This should be documented as a warning in the UI, but CrossHook cannot reliably prevent it on Linux.
- **Update Requires Specific DLLs or Runtime Libraries**: Some updates require Visual C++ redistributables or .NET runtimes already present in the prefix. CrossHook cannot detect this; the update will simply fail. The user should be directed to the update log for diagnosis.
- **Update Executable Has a GUI Installer**: Many game updates ship as interactive Windows installers (e.g., InnoSetup, NSIS) that pop up a GUI. Proton handles rendering these, so CrossHook should run the update the same way it runs the initial installer (via `new_direct_proton_command`). The update completes when the process exits.
- **Update Executable Is a Silent Patcher**: Some updates are command-line patchers that require the working directory to be the game install directory. CrossHook should set the working directory to the update executable's parent directory (matching `apply_working_directory` behavior), or the game's known working directory from the profile.
- **Steam-Managed Games**: For `steam_applaunch` profiles, the prefix is under Steam's compatdata. Updates applied here persist across Steam sessions since the prefix is the same one Steam uses. However, Steam may overwrite changes on game verification. This is documented behavior, not something CrossHook should prevent.
- **Prefix Path Ambiguity (pfx child)**: The existing `resolve_wine_prefix_path()` function handles the case where the prefix path points to a compatdata root that contains a `pfx/` subdirectory. The update flow must use the same resolution.
- **Multiple Updates in Sequence**: If a user needs to apply patches in order (e.g., v1.0 -> v1.1 -> v1.2), each update should be a separate "Apply Update" action. There is no batch update mechanism.
- **Profile With Empty Prefix Path**: A `proton_run` profile might have an empty `runtime.prefix_path` if the user never completed the install flow. The update form should validate that the prefix path is populated and existing.

## Workflows

### Primary Workflow: Apply Game Update

1. User navigates to the **Install** page in the sidebar (the existing "Install Game" route).
2. User scrolls or tabs to a new **Update Game** section on the same page, below or alongside the Install Game panel.
3. User selects an existing profile from a dropdown populated by `ProfileStore.list()`, filtered to exclude `native` profiles.
4. CrossHook auto-populates:
   - **Prefix Path**: from the profile's `runtime.prefix_path` or `steam.compatdata_path`
   - **Proton Path**: from the profile's `runtime.proton_path` or `steam.proton_path`
   - These fields are pre-filled but editable (overrideable).
5. User browses to the update executable (`.exe` file).
6. User clicks **Apply Update**.
7. Backend validates:
   - Profile exists
   - Prefix path exists and is a directory
   - Update executable exists and is a `.exe` file
   - Proton path exists and is executable
8. Backend runs the update executable through Proton against the profile's prefix (reusing `new_direct_proton_command`, `apply_host_environment`, `apply_runtime_proton_environment`, `apply_working_directory`, `attach_log_stdio`).
9. UI shows "Running update..." with the stage indicator.
10. Update process completes (success or failure).
11. UI shows the result message and the log file path.
12. Profile is NOT modified. User can launch the game immediately.

### Alternative Workflow: Update With Proton Override

1. Steps 1-4 same as primary workflow.
2. User changes the Proton Path field to a different Proton version (e.g., to test GE-Proton compatibility).
3. Steps 5-12 same as primary workflow.
4. The override is ephemeral; the profile on disk still points to the original Proton version.

### Error Recovery

- **Update executable does not exist**: Validation catches this before execution. User re-browses to the correct file.
- **Prefix does not exist**: Validation catches this. User is told to reinstall or restore the prefix.
- **Proton path invalid**: Validation catches this. User selects a valid Proton installation.
- **Update process fails (non-zero exit code)**: Result is displayed with the log file path. User reviews the log. User can retry after fixing the issue (e.g., installing a prerequisite into the prefix).
- **Update process hangs**: Since `block_on(child.wait())` is used (same as install), the UI shows "Running update..." indefinitely. The user must kill the process externally. A future enhancement could add a timeout or cancel button.
- **Update corrupts the prefix**: CrossHook does not provide rollback. The user must reinstall. This is the same behavior as the manual script workflow today.

## Domain Model

### Key Entities

- **GameProfile**: The persisted profile containing game, trainer, injection, steam, runtime, and launch sections. The update flow reads the profile for prefix and proton paths but does not write to it.
- **UpdateGameRequest**: A new request type containing the profile name (to look up prefix/proton), the update executable path, and optional overrides for prefix and proton paths.
- **UpdateGameResult**: Similar to `InstallGameResult` but simpler -- no executable discovery, no profile generation. Contains succeeded/failed status, message, and log path.
- **Proton Prefix**: The WINE/Proton compatibility environment directory. Contains `drive_c/`, registry files, and the game installation. The update executable writes into this environment.
- **Update Executable**: A Windows `.exe` file (patch, update, hotfix) that modifies game files within the prefix.

### State Transitions

- **idle** -> **preparing**: User clicks "Apply Update"
- **preparing** -> **running_update**: Validation passes, update process spawned
- **running_update** -> **completed**: Update process exits with code 0
- **running_update** -> **failed**: Update process exits with non-zero code or spawn fails
- **completed** -> **idle**: User clicks "Reset" or starts another update
- **failed** -> **idle**: User clicks "Reset"
- **failed** -> **preparing**: User clicks "Retry"

## Existing Codebase Integration

### Related Features

- `/src/crosshook-native/crates/crosshook-core/src/install/service.rs`: The `install_game()` function is the closest analog. The update flow reuses its command-building pattern (`new_direct_proton_command`, `apply_host_environment`, `apply_runtime_proton_environment`, `apply_working_directory`, `attach_log_stdio`) but skips prefix creation and executable discovery.
- `/src/crosshook-native/crates/crosshook-core/src/install/models.rs`: `InstallGameRequest` and `InstallGameResult` provide the pattern for `UpdateGameRequest` and `UpdateGameResult`. The update versions are simpler (no `display_name`, no `trainer_path`, no `installed_game_executable_path`, no `discovered_game_executable_candidates`, no `needs_executable_confirmation`, no `profile`).
- `/src/crosshook-native/crates/crosshook-core/src/install/discovery.rs`: Executable discovery is NOT needed for updates. The game executable is already known from the profile.
- `/src/crosshook-native/crates/crosshook-core/src/launch/runtime_helpers.rs`: `new_direct_proton_command()`, `apply_host_environment()`, `apply_runtime_proton_environment()`, `apply_working_directory()`, `attach_log_stdio()`, and `resolve_wine_prefix_path()` are all directly reusable for building the update command.
- `/src/crosshook-native/crates/crosshook-core/src/profile/toml_store.rs`: `ProfileStore::load()` and `ProfileStore::list()` are used to look up the target profile and populate the profile selector.
- `/src/crosshook-native/src-tauri/src/commands/install.rs`: The Tauri command pattern (`#[tauri::command] pub async fn install_game(...)`) with `spawn_blocking`, `create_log_path`, and `install_log_target_slug` provides the template for the new `update_game` command.
- `/src/crosshook-native/src/components/InstallGamePanel.tsx`: The UI structure (form fields, stage indicators, status text, action buttons) provides the visual pattern for the update panel.
- `/src/crosshook-native/src/hooks/useInstallGame.ts`: The hook structure (request state, validation state, stage state, result state, derived UI text) provides the pattern for a `useUpdateGame` hook.
- `/src/crosshook-native/src/types/install.ts`: The type definitions pattern for install types can be followed for update types.
- `/src/crosshook-native/src/components/pages/InstallPage.tsx`: The page-level orchestration (panel + modal) shows where the update panel should be integrated.
- `/src/crosshook-native/src/components/layout/Sidebar.tsx`: The "Install Game" route already exists under the "Setup" section. The update feature would live on the same page, not a new route.

### Patterns to Follow

- **Tauri IPC Pattern**: Thin `#[tauri::command]` wrappers in `src-tauri/src/commands/` that delegate to `crosshook-core` functions. All business logic lives in the core crate.
- **Request/Result Pattern**: Strongly typed request and result structs with serde derives, used for both Rust and TypeScript via IPC.
- **Validation-Then-Execute Pattern**: `validate_update_request()` is called before `update_game()`, matching `validate_install_request()` before `install_game()`.
- **Log Path Pattern**: `create_log_path("update", &target_slug)` follows the existing `create_log_path("install", ...)` and `create_log_path("game", ...)` patterns, writing to `/tmp/crosshook-logs/`.
- **Hook-Driven UI Pattern**: A `useUpdateGame` hook manages all state, exposing derived values (`statusText`, `hintText`, `actionLabel`, `isRunningUpdate`) to the component.
- **ProfileStore Read Pattern**: The update flow calls `ProfileStore::load()` to read the profile for prefix/proton resolution. It never calls `ProfileStore::save()`.
- **Prefix Path Resolution Pattern**: Uses `resolve_wine_prefix_path()` from `runtime_helpers.rs` to handle both standalone prefixes and compatdata-with-pfx prefixes.

### Components to Leverage

- **`InstallGamePanel.tsx` layout**: The update panel can reuse the same section structure (`crosshook-install-shell`, `crosshook-install-section`, `crosshook-install-card`) and field components (`InstallField`, `ProtonPathField`).
- **`ProfileReviewModal`**: NOT needed for updates. The update does not generate a new profile.
- **`ThemedSelect`**: Used for the profile selector dropdown in the update panel.
- **`useInstallGame` hook structure**: The `useUpdateGame` hook follows the same pattern but with fewer fields and simpler state transitions (no review step, no candidate discovery).
- **`PageBanner`**: The Install page already has a banner. The update section sits below it or uses a sub-heading.

### Key Rust Functions to Reuse (Not Duplicate)

The update command should call the same runtime helper functions the install flow uses. These are already public:

- `new_direct_proton_command(proton_path)` -- creates the base Proton command
- `apply_host_environment(command)` -- injects HOME, PATH, DISPLAY, etc.
- `apply_runtime_proton_environment(command, prefix_path, steam_client_install_path)` -- sets WINEPREFIX and STEAM_COMPAT_DATA_PATH
- `apply_working_directory(command, configured_directory, primary_path)` -- sets cwd
- `attach_log_stdio(command, log_path)` -- redirects stdout/stderr to log file
- `resolve_wine_prefix_path(prefix_path)` -- handles pfx/ subdirectory resolution

### Difference From Install Flow

| Aspect                      | Install Game                                | Update Game                          |
| --------------------------- | ------------------------------------------- | ------------------------------------ |
| Creates prefix              | Yes (`provision_prefix`)                    | No (prefix must exist)               |
| Discovers executables       | Yes (`discover_game_executable_candidates`) | No                                   |
| Generates profile           | Yes (`build_reviewable_profile`)            | No                                   |
| Opens review modal          | Yes                                         | No                                   |
| Saves profile               | Yes (after review)                          | No                                   |
| Requires profile name input | Yes (new name)                              | Yes (existing name from dropdown)    |
| Requires installer path     | Yes                                         | Yes (but called "update executable") |
| Requires trainer path       | Yes (optional)                              | No                                   |
| Requires display name       | Yes (optional)                              | No                                   |
| Prefix path editable        | Yes (defaults to auto)                      | Yes (defaults to profile value)      |
| Proton path editable        | Yes                                         | Yes (defaults to profile value)      |

## Success Criteria

- [ ] User can select an existing proton_run or steam_applaunch profile from a dropdown
- [ ] Selecting a profile auto-fills prefix path and Proton path from the profile
- [ ] User can browse to a Windows .exe update/patch file
- [ ] Validation catches missing prefix, invalid executable, and invalid Proton path with clear error messages
- [ ] Update executable runs through Proton against the correct prefix
- [ ] Update process exit code is captured and reported
- [ ] Log file is created and path is displayed to the user
- [ ] Profile on disk is NOT modified by the update process
- [ ] Native profiles are excluded from the profile selector
- [ ] UI matches the visual style and interaction patterns of the Install Game panel
- [ ] Gamepad/controller navigation works for the update form
- [ ] Rust tests exist for the update service validation and command building
- [ ] Update logs stream to the console drawer (if log streaming is wired up)

## Open Questions

- **Page Location**: Should the Update Game section appear as a second panel on the Install page (below InstallGamePanel), or as a tab/toggle within the same panel? The Install page already has a PageBanner and a panel; adding a second panel below is the simplest approach and matches the sidebar's single "Install Game" route.
- **Profile Selector Scope**: Should the profile selector show ALL non-native profiles, or only profiles that have a non-empty prefix path? Profiles without a prefix path cannot be updated, so filtering them out improves UX at the cost of potentially confusing users who expect to see all their profiles.
- **Working Directory for Updates**: Should the update executable's working directory default to the profile's `runtime.working_directory` (which points to the game directory), or to the update executable's parent directory? The install flow uses the installer's parent directory. For updates, the game directory may be more appropriate since some patches expect to be run from the game root.
- **Log Streaming**: The install flow blocks on `child.wait()` via `block_on`, meaning no live log streaming occurs during install. The launch flow uses `spawn_log_stream` for real-time streaming. Should the update flow adopt the launch flow's streaming pattern, or stay consistent with the install flow's blocking pattern? Streaming is better UX but changes the execution model.
- **Cancel / Timeout**: Should the update flow support cancellation? The install flow does not. Adding a cancel button requires process killing, which introduces complexity.
- **Sidebar Label Change**: Should the sidebar item change from "Install Game" to "Install / Update" or stay as "Install Game" with the update being a sub-section of the same page?

## Relevant Files

- `/src/crosshook-native/crates/crosshook-core/src/install/service.rs`: Install service (command building, validation, prefix provisioning)
- `/src/crosshook-native/crates/crosshook-core/src/install/models.rs`: Install request/result/error types
- `/src/crosshook-native/crates/crosshook-core/src/install/discovery.rs`: Executable discovery (not needed for updates)
- `/src/crosshook-native/crates/crosshook-core/src/install/mod.rs`: Install module public API
- `/src/crosshook-native/crates/crosshook-core/src/launch/runtime_helpers.rs`: Proton command building, env setup, prefix resolution
- `/src/crosshook-native/crates/crosshook-core/src/launch/request.rs`: Launch request validation patterns
- `/src/crosshook-native/crates/crosshook-core/src/profile/models.rs`: GameProfile and section structs
- `/src/crosshook-native/crates/crosshook-core/src/profile/toml_store.rs`: ProfileStore for loading/listing profiles
- `/src/crosshook-native/crates/crosshook-core/src/lib.rs`: Module root (where `update` module would be added)
- `/src/crosshook-native/src-tauri/src/commands/install.rs`: Tauri install commands (pattern for update commands)
- `/src/crosshook-native/src-tauri/src/commands/mod.rs`: Command module registration
- `/src/crosshook-native/src-tauri/src/lib.rs`: Tauri command handler registration
- `/src/crosshook-native/src/components/InstallGamePanel.tsx`: Install UI panel (visual pattern for update panel)
- `/src/crosshook-native/src/components/pages/InstallPage.tsx`: Install page orchestration (where update panel gets added)
- `/src/crosshook-native/src/hooks/useInstallGame.ts`: Install state management hook (pattern for update hook)
- `/src/crosshook-native/src/types/install.ts`: Install type definitions (pattern for update types)
- `/src/crosshook-native/src/components/layout/Sidebar.tsx`: Sidebar navigation (route definition)
- `/src/crosshook-native/src/components/layout/ContentArea.tsx`: Content routing
