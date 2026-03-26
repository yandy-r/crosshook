# Integration Research: update-game

This report documents the APIs, IPC commands, data persistence patterns, internal services, and frontend integration points relevant to implementing the update-game feature. The update-game feature runs a Windows update/patch executable against an existing Proton prefix, reusing established runtime helper primitives from the launch and install modules. The implementation requires new Tauri IPC commands (`update_game`, `validate_update_request`), a new `update-log` Tauri event for real-time streaming, and a new frontend hook and component integrated into the existing Install page.

## API Endpoints

### Tauri IPC Commands

### Existing Related Commands

| Command                             | File                                   | Description                                                                                                                                                                                                     |
| ----------------------------------- | -------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `profile_load`                      | `src-tauri/src/commands/profile.rs:90` | Loads a `GameProfile` by name from `ProfileStore`. Synchronous, takes `State<ProfileStore>`. Returns the full profile struct including `runtime.prefix_path`, `runtime.proton_path`, and `launch.method`.       |
| `profile_list`                      | `src-tauri/src/commands/profile.rs:85` | Lists all profile names (alphabetically sorted). Returns `Vec<String>`. Used by frontend to populate profile selectors.                                                                                         |
| `list_proton_installs`              | `src-tauri/src/commands/steam.rs:35`   | Discovers Proton/Wine compat tools from Steam library directories. Returns `Vec<ProtonInstall>` with `name`, `path`, `is_official` fields. Accepts optional `steam_client_install_path` override.               |
| `install_game`                      | `src-tauri/src/commands/install.rs:31` | Runs a Windows installer through Proton. Most analogous to `update_game` -- same pattern of creating a log path, spawning a process via `spawn_blocking`, and waiting for exit. Blocks until process completes. |
| `validate_install_request`          | `src-tauri/src/commands/install.rs:22` | Validates an `InstallGameRequest` before execution. Uses `spawn_blocking` for filesystem checks.                                                                                                                |
| `launch_game`                       | `src-tauri/src/commands/launch.rs:38`  | Launches a game through Proton with real-time log streaming via `spawn_log_stream`. Most relevant pattern for the async streaming approach.                                                                     |
| `default_steam_client_install_path` | `src-tauri/src/commands/steam.rs:9`    | Resolves the Steam client install path from environment or known filesystem paths.                                                                                                                              |
| `install_default_prefix_path`       | `src-tauri/src/commands/install.rs:11` | Resolves a default prefix path for a profile name. Not needed for update (prefix comes from the profile).                                                                                                       |

### Command Registration Pattern

Commands are registered in `src-tauri/src/lib.rs:69-104` via `tauri::generate_handler![]`. The macro takes a comma-separated list of fully-qualified paths to command functions. New commands follow the pattern:

```
commands::update::update_game,
commands::update::validate_update_request,
```

The command module must be declared in `src-tauri/src/commands/mod.rs` (currently 7 modules: `community`, `export`, `install`, `launch`, `profile`, `settings`, `steam`). Add `pub mod update;`.

### Tauri State Management

Four stores are managed as Tauri state (created once in `lib.rs:15-30`, registered via `.manage()`):

- `ProfileStore` -- needed by `update_game` to verify the profile exists (or the frontend can pass paths directly)
- `SettingsStore` -- not needed for update
- `RecentFilesStore` -- not needed for update
- `CommunityTapStore` -- not needed for update

The update command can either:

1. Accept `State<ProfileStore>` and load the profile server-side to extract paths (safer)
2. Accept all paths directly from the frontend (simpler, matches the feature spec's `UpdateGameRequest` which has explicit `prefix_path` and `proton_path` fields)

The feature spec uses approach (2): the frontend loads the profile via `profile_load`, populates the form, and sends explicit paths. This allows the user to override Proton path before applying.

### Install Command Pattern (Blocking)

`commands/install.rs` uses `spawn_blocking` + `block_on` for synchronous execution:

```rust
pub async fn install_game(request: InstallGameRequest) -> Result<InstallGameResult, String> {
    let log_path = create_log_path("install", &install_log_target_slug(&request.profile_name))?;
    tauri::async_runtime::spawn_blocking(move || {
        install_game_core(&request, &log_path).map_err(|error| error.to_string())
    }).await.map_err(|error| error.to_string())?
}
```

This blocks the Tauri async runtime thread until the installer process exits. No real-time streaming occurs -- the log file is written to disk and the path is returned in the result.

### Launch Command Pattern (Streaming)

`commands/launch.rs` uses async spawning + `spawn_log_stream` for real-time output:

```rust
pub async fn launch_game(app: AppHandle, request: LaunchRequest) -> Result<LaunchResult, String> {
    // ... validate, build command ...
    let child = command.spawn()?;
    spawn_log_stream(app, log_path.clone(), child);
    Ok(LaunchResult { succeeded: true, ... })  // Returns immediately
}
```

The feature spec requires real-time streaming (Phase 1, decision #2), so the update command should follow the **launch pattern** (not the install pattern). This means the command returns immediately after spawning, and log lines stream via events.

**Critical difference**: The install pattern `block_on(child.wait())` is synchronous and returns the exit code in the result. The launch pattern returns immediately and the exit status is only captured in the log stream task. For update-game, the feature spec wants both streaming AND exit code reporting. This can be achieved by having the `stream_log_lines` function emit a final "update-complete" event with the exit status, or by having the hook poll a completion state.

## Data Persistence

### Profile TOML Structure

Profiles are stored as TOML files in `~/.config/crosshook/profiles/<name>.toml`. The full `GameProfile` struct has these sections:

| Section     | Key Fields for Update                                             | Source                                                                                                    |
| ----------- | ----------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------- |
| `game`      | `name`, `executable_path`                                         | Display only -- not modified by update                                                                    |
| `trainer`   | `path`, `type`, `loading_mode`                                    | Not relevant to update                                                                                    |
| `injection` | `dll_paths`, `inject_on_launch`                                   | Not relevant to update                                                                                    |
| `steam`     | `enabled`, `app_id`, `compatdata_path`, `proton_path`, `launcher` | `compatdata_path` used for `steam_applaunch` prefix resolution; `proton_path` is the Steam-managed Proton |
| `runtime`   | `prefix_path`, `proton_path`, `working_directory`                 | **Primary source for update**: prefix_path and proton_path for `proton_run` profiles                      |
| `launch`    | `method`, `optimizations`                                         | `method` used to filter eligible profiles (`proton_run` only)                                             |

**Key distinction for prefix resolution**: `proton_run` profiles store prefix in `runtime.prefix_path` and Proton in `runtime.proton_path`. `steam_applaunch` profiles store prefix implicitly via `steam.compatdata_path` and Proton in `steam.proton_path`. The update feature targets only `proton_run` profiles, so `runtime.prefix_path` and `runtime.proton_path` are the relevant fields.

### ProfileStore API

Defined in `crates/crosshook-core/src/profile/toml_store.rs`:

- `ProfileStore::try_new()` -- Creates store pointing to `~/.config/crosshook/profiles/`
- `ProfileStore::load(name)` -> `Result<GameProfile, ProfileStoreError>` -- Reads and deserializes TOML
- `ProfileStore::list()` -> `Result<Vec<String>, ProfileStoreError>` -- Lists `.toml` file stems, sorted
- `ProfileStore::save(name, profile)` -- Not used by update (read-only)
- `ProfileStore::with_base_path(path)` -- Constructor for tests

### Log Files

Log path creation pattern (identical in `commands/launch.rs:168-181` and `commands/install.rs:40-53`):

- **Directory**: `/tmp/crosshook-logs/` (created with `create_dir_all`)
- **Naming**: `{prefix}-{target_slug}-{timestamp}.log`
  - `prefix`: operation type (`"game"`, `"trainer"`, `"install"` -- update should use `"update"`)
  - `target_slug`: slugified profile name (alphanumeric lowercase, hyphens for non-alnum)
  - `timestamp`: Unix epoch milliseconds
- **Creation**: Empty file created immediately via `fs::File::create`
- **Writing**: stdout/stderr redirected to the file via `attach_log_stdio`

Both `commands/launch.rs` and `commands/install.rs` have their own `create_log_path` functions (duplicated). The update command needs its own copy or should extract a shared utility. The slug function is also duplicated between the two files (`install_log_target_slug` in install vs `log_target_slug` method on `LaunchRequest`).

## Internal Services

### runtime_helpers.rs Functions

File: `crates/crosshook-core/src/launch/runtime_helpers.rs`

These are the 6 reusable primitives that `build_install_command` already reuses and `build_update_command` will also reuse:

| Function                                                                            | Lines   | Purpose                                                                                                                                                         | Used by Install  | Used by Update |
| ----------------------------------------------------------------------------------- | ------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------- | ---------------- | -------------- |
| `new_direct_proton_command(proton_path)`                                            | 12-14   | Creates `Command::new(proton_path).arg("run").env_clear()`. Always uses the `run` verb.                                                                         | Yes              | Yes            |
| `new_direct_proton_command_with_wrappers(proton_path, wrappers)`                    | 16-35   | Same but prepends wrapper commands (e.g., `gamemoderun`). Wrapper list empty = same as above.                                                                   | No (launch only) | No             |
| `apply_host_environment(command)`                                                   | 46-60   | Sets HOME, USER, PATH, DISPLAY, WAYLAND_DISPLAY, XDG_RUNTIME_DIR, DBUS_SESSION_BUS_ADDRESS from current environment.                                            | Yes              | Yes            |
| `apply_runtime_proton_environment(command, prefix_path, steam_client_install_path)` | 62-92   | Sets WINEPREFIX (via `resolve_wine_prefix_path`), STEAM_COMPAT_DATA_PATH, STEAM_COMPAT_CLIENT_INSTALL_PATH.                                                     | Yes              | Yes            |
| `apply_working_directory(command, configured_dir, primary_path)`                    | 122-137 | Sets `current_dir` to configured directory or parent of primary path.                                                                                           | Yes              | Yes            |
| `attach_log_stdio(command, log_path)`                                               | 139-155 | Redirects stdout + stderr to the log file (append mode). Creates parent directory.                                                                              | Yes              | Yes            |
| `resolve_wine_prefix_path(prefix_path)`                                             | 94-105  | If path ends in `pfx`, use as-is; if `prefix/pfx` exists, use that; otherwise use the prefix path directly. Handles both standalone and Steam-managed prefixes. | Indirectly       | Indirectly     |

**Install's `build_install_command` pattern** (in `install/service.rs:102-120`) is the template for `build_update_command`:

```rust
fn build_install_command(request, prefix_path, log_path) -> Result<Command, Error> {
    let mut command = new_direct_proton_command(request.proton_path.trim());
    command.arg(request.installer_path.trim());   // update: request.updater_path
    apply_host_environment(&mut command);
    apply_runtime_proton_environment(&mut command, &prefix_path_string, "");
    apply_working_directory(&mut command, "", Path::new(request.installer_path.trim()));
    attach_log_stdio(&mut command, log_path)?;
    Ok(command)
}
```

The update version would pass `steam_client_install_path` instead of empty string for the third argument to `apply_runtime_proton_environment`, since the feature spec includes that field.

**Proton verb note**: The feature spec mentions `proton waitforexitandrun` as preferred for blocking updates. The current codebase exclusively uses `proton run` (hardcoded in `new_direct_proton_command`). To use `waitforexitandrun`, the update module would need either: (a) a new helper function `new_direct_proton_waitforexitandrun_command`, or (b) a parameter to `new_direct_proton_command` to choose the verb. The `run` verb works fine -- `waitforexitandrun` explicitly waits for wineserver shutdown, which is safer for sequential patches but may add unnecessary delay.

### Install Service (service.rs) Execution Pattern

`install/service.rs:45-99` shows the blocking execution model:

1. Validate the request
2. Provision the prefix (create directory if needed) -- **not needed for update** (prefix must already exist)
3. Get a `Handle::try_current()` for the Tokio runtime
4. `build_install_command()` -- creates the `Command`
5. `command.spawn()` -- starts the process
6. `runtime_handle.block_on(child.wait())` -- blocks until exit
7. Check exit status
8. Discover game executables (post-install) -- **not needed for update**
9. Build reviewable profile -- **not needed for update**

For update-game with streaming, steps 5-6 change to the launch pattern: 5. `command.spawn()` -- starts the process 6. `spawn_log_stream(app, log_path, child)` -- streams lines in background 7. Return immediately with result

### Log Streaming (spawn_log_stream)

File: `src-tauri/src/commands/launch.rs:103-166`

`spawn_log_stream` is a private function in the launch commands module. It spawns two async tasks:

**Task 1** (`stream_log_lines`): Polls the log file every 500ms, emits new lines as `launch-log` events:

- Reads the entire file each iteration (not seeking)
- Tracks `last_len` to emit only new content
- Handles file truncation (resets `last_len` if content shrinks)
- Emits each non-empty line individually via `app.emit("launch-log", line)`
- Checks `child.try_wait()` each iteration to detect process exit
- Performs one final read after process exit to capture trailing lines
- Event payload is a plain `String`

**Task 2**: Outer wrapper that logs errors from Task 1.

**For update-game**: This function needs to be either:

1. Extracted to a shared location (e.g., a new `src-tauri/src/log_stream.rs` utility)
2. Duplicated in `commands/update.rs` with the event name changed to `"update-log"`
3. Parameterized to accept the event name

Option (1) is cleanest. The function signature would become:

```rust
fn spawn_log_stream(app: AppHandle, log_path: PathBuf, child: Child, event_name: &'static str)
```

**Exit code reporting**: The current `stream_log_lines` discards the exit status. For update-game, the feature spec needs to report success/failure. The streaming task should emit a final event (e.g., `"update-complete"`) with the exit status after the process exits.

## Frontend Integration

### Hook -> Command Invocation

All frontend-to-backend communication uses `invoke()` from `@tauri-apps/api/core`. The pattern is consistent across all hooks:

```typescript
import { invoke } from '@tauri-apps/api/core';

// Validate first
await invoke<void>('validate_update_request', { request: updateRequest });

// Then execute
const result = await invoke<UpdateGameResult>('update_game', { request: updateRequest });
```

**Key convention**: Tauri `invoke()` converts `camelCase` JavaScript parameter names to `snake_case` Rust parameter names automatically. For example, `invoke('profile_load', { name: 'foo' })` maps to `fn profile_load(name: String)`.

When a Tauri command takes `AppHandle`, it is injected automatically and not passed from the frontend.

### useInstallGame Hook Pattern

`src/hooks/useInstallGame.ts` is the closest analogue for `useUpdateGame`. Key patterns:

- State managed via multiple `useState` hooks (not `useReducer` like `useLaunchState`)
- Exposes: `request`, `validation`, `stage`, `result`, `error`, action methods
- Stage lifecycle: `'idle' -> 'preparing' -> 'running_installer' -> 'review_required' | 'ready_to_save' | 'failed'`
- For update: `'idle' -> 'preparing' -> 'running_updater' -> 'complete' | 'failed'` (simpler -- no review stage)
- Validation uses `try/catch` on `invoke`, maps error messages to field names via lookup table
- Returns derived values: `statusText`, `hintText`, `actionLabel`, boolean flags (`isIdle`, `isPreparing`, etc.)
- Prefix path resolution is automatic from profile name (not needed for update -- prefix comes from profile)
- `reset()` clears all state back to initial

### Console Event Subscription

Two components subscribe to log events:

**ConsoleView** (`src/components/ConsoleView.tsx:45-69`):

```typescript
const unlistenPromise = listen<LogPayload>('launch-log', (event) => {
  const text = normalizeLogMessage(event.payload).trimEnd();
  setLines((current) => [...current, entry]);
});
```

**ConsoleDrawer** (`src/components/layout/ConsoleDrawer.tsx:50-74`):

```typescript
const unlistenPromise = listen<LogPayload>('launch-log', (event) => {
    // Counts lines for badge, auto-expands drawer on first event
    setCollapsed((current) => { if (current) panelRef.current?.expand(); ... });
    setLineCount((current) => current + nextLineCount);
});
```

Both use `listen()` from `@tauri-apps/api/event`. Both subscribe to `'launch-log'` event name.

For the `update-log` event, both components need to subscribe to both `'launch-log'` AND `'update-log'`. Options:

1. Subscribe to both events in each component (two `listen()` calls)
2. Use a single unified event name (simpler but mixes log sources)
3. Parameterize the event name via props/context

The feature spec chose option (1): dedicated `update-log` event, with ConsoleDrawer subscribing to both. This is the cleanest separation.

The `normalizeLogMessage()` utility in `src/utils/log.ts` handles multiple payload shapes (string, `{line}`, `{message}`, `{text}`). Since the backend emits plain strings, it works as-is for update logs.

### File/Directory Dialog

`src/utils/dialog.ts` provides two functions:

- `chooseFile(title, filters?)` -- opens a file picker, returns path or null
- `chooseDirectory(title)` -- opens a directory picker, returns path or null

The update panel needs `chooseFile` with a filter for `.exe` files:

```typescript
const path = await chooseFile('Select update executable', [{ name: 'Windows Executable', extensions: ['exe'] }]);
```

Both functions use `@tauri-apps/plugin-dialog` which is already configured in the Tauri capabilities (`"dialog:default"` in `src-tauri/capabilities/default.json`).

### App Layout and Routing

The sidebar (`src/components/layout/Sidebar.tsx`) defines `AppRoute`:

```typescript
type AppRoute = 'profiles' | 'launch' | 'install' | 'community' | 'compatibility' | 'settings';
```

The feature spec places the update panel on the existing `'install'` route. `ContentArea.tsx` renders `InstallPage` for the `'install'` route. The `InstallPage` will import and render `UpdateGamePanel` below `InstallGamePanel`.

No new route or sidebar entry is needed.

### Profile Context

`ProfileContext` (`src/context/ProfileContext.tsx`) provides profile state to all components. However, the Install page uses its own separate `protonInstalls` state (loaded in `InstallPage.tsx:293-329`). The update panel can reuse the same Proton installs list loaded by the Install page by receiving it as a prop.

For loading a profile by name (when the user selects from the update dropdown), the frontend calls `invoke<GameProfile>('profile_load', { name })` directly -- not through ProfileContext (which manages the globally selected profile, not ad-hoc loads).

## Configuration

### Environment Variables

The update command sets these via `apply_host_environment` and `apply_runtime_proton_environment`:

| Variable                           | Set By                             | Value Source                               |
| ---------------------------------- | ---------------------------------- | ------------------------------------------ |
| `HOME`                             | `apply_host_environment`           | Current process `$HOME`                    |
| `USER`                             | `apply_host_environment`           | Current process `$USER`                    |
| `PATH`                             | `apply_host_environment`           | Current process `$PATH` or `/usr/bin:/bin` |
| `DISPLAY`                          | `apply_host_environment`           | Current process `$DISPLAY`                 |
| `WAYLAND_DISPLAY`                  | `apply_host_environment`           | Current process `$WAYLAND_DISPLAY`         |
| `XDG_RUNTIME_DIR`                  | `apply_host_environment`           | Current process `$XDG_RUNTIME_DIR`         |
| `DBUS_SESSION_BUS_ADDRESS`         | `apply_host_environment`           | Current process                            |
| `WINEPREFIX`                       | `apply_runtime_proton_environment` | `resolve_wine_prefix_path(prefix_path)`    |
| `STEAM_COMPAT_DATA_PATH`           | `apply_runtime_proton_environment` | Computed from prefix path                  |
| `STEAM_COMPAT_CLIENT_INSTALL_PATH` | `apply_runtime_proton_environment` | From request or auto-discovered            |

Commands are created with `env_clear()` first, then these variables are explicitly set. No host WINE/Proton environment variables leak through.

### Tauri Capabilities

`src-tauri/capabilities/default.json` currently allows `"core:default"` and `"dialog:default"`. No additional permissions are needed for the update feature -- `invoke()` calls to custom commands are covered by `core:default`, and file dialogs by `dialog:default`.

### Log Directory

`/tmp/crosshook-logs/` -- created on demand. Not configurable. Survives reboots on most Linux distributions but is not guaranteed persistent.

## Relevant Files

### Backend (Rust)

- `src-tauri/src/lib.rs`: Tauri app initialization, command registration, state management
- `src-tauri/src/commands/mod.rs`: Command module declarations (add `pub mod update;`)
- `src-tauri/src/commands/install.rs`: Closest command pattern for blocking execution, log path creation
- `src-tauri/src/commands/launch.rs`: `spawn_log_stream` and `stream_log_lines` -- real-time streaming pattern
- `src-tauri/src/commands/profile.rs`: `profile_load` and `profile_list` commands
- `src-tauri/src/commands/steam.rs`: `list_proton_installs` command
- `crates/crosshook-core/src/lib.rs`: Core module declarations (add `pub mod update;`)
- `crates/crosshook-core/src/install/mod.rs`: Install module public API pattern
- `crates/crosshook-core/src/install/models.rs`: Request/Result/Error type patterns
- `crates/crosshook-core/src/install/service.rs`: `build_install_command` -- direct template for `build_update_command`
- `crates/crosshook-core/src/launch/runtime_helpers.rs`: All 6 reusable command-building primitives
- `crates/crosshook-core/src/launch/env.rs`: Environment variable constants
- `crates/crosshook-core/src/profile/models.rs`: `GameProfile`, `RuntimeSection`, `LaunchSection` structs
- `crates/crosshook-core/src/profile/toml_store.rs`: `ProfileStore` API

### Frontend (TypeScript/React)

- `src/App.tsx`: App shell with ConsoleDrawer integration
- `src/components/layout/ConsoleDrawer.tsx`: Log event subscription pattern (must add `update-log`)
- `src/components/layout/ContentArea.tsx`: Route-to-page mapping
- `src/components/layout/Sidebar.tsx`: `AppRoute` type definition
- `src/components/ConsoleView.tsx`: Log line rendering and `launch-log` subscription
- `src/components/InstallGamePanel.tsx`: `InstallField` component (extractable for shared use)
- `src/components/pages/InstallPage.tsx`: Integration point for `UpdateGamePanel`
- `src/hooks/useInstallGame.ts`: Closest hook pattern for update state machine
- `src/hooks/useLaunchState.ts`: Alternative hook pattern using `useReducer`
- `src/types/install.ts`: Type, validation error, and stage patterns
- `src/types/profile.ts`: `GameProfile` TypeScript interface
- `src/types/index.ts`: Type re-export barrel (add `update` export)
- `src/utils/dialog.ts`: `chooseFile` and `chooseDirectory` utilities
- `src/utils/log.ts`: `normalizeLogMessage` and `LogPayload` type
- `src/context/ProfileContext.tsx`: Global profile state provider

## Architectural Patterns

- **Tauri command → core service delegation**: Commands in `src-tauri/src/commands/` are thin wrappers that delegate to `crosshook_core` functions. Error mapping is done via `.map_err(|e| e.to_string())`.
- **Validation-then-execute**: Frontend calls `validate_*` first, then `execute_*`. Both are separate IPC commands. This allows field-level error reporting before starting the operation.
- **Log file as IPC bridge**: Proton processes write to a log file; the Tauri async task polls the file and emits events. This avoids piping stdout/stderr through Tauri directly (which would require managing the child process lifetime more carefully).
- **Frontend state machines via hooks**: Complex multi-stage operations use custom hooks with stage enums (`'idle' | 'preparing' | 'running' | 'complete' | 'failed'`). Derived values (`statusText`, `hintText`, boolean flags) are computed from the stage.
- **Profile-driven configuration**: Instead of free-text path entry, operations resolve paths from saved profiles. This prevents wrong-prefix targeting.
- **Serde derive for IPC types**: All types crossing the Tauri IPC boundary derive `Serialize` and `Deserialize`. Enum error types use `#[serde(rename_all = "snake_case")]`.
- **`env_clear()` + explicit set**: All Proton commands start with a clean environment and explicitly set only the required variables. This prevents host WINE/Proton variable bleed.

## Gotchas and Edge Cases

- **Duplicate `create_log_path`**: Both `commands/launch.rs:168-181` and `commands/install.rs:40-53` define their own `create_log_path` function with identical logic. The update command will need a third copy unless extracted to a shared utility.
- **`spawn_log_stream` is private to launch**: The streaming function is defined in `commands/launch.rs:103-113` and not exported. To use it from `commands/update.rs`, it must be either extracted to a shared module or duplicated.
- **Exit code not propagated via streaming**: The `stream_log_lines` function discards the child exit status. For update-game, the exit code matters (success/failure reporting). The streaming task needs to emit a completion event.
- **`proton run` vs `waitforexitandrun`**: The codebase uses `run` everywhere. The feature spec mentions `waitforexitandrun` as preferred for blocking operations. Using `waitforexitandrun` would change the verb in `new_direct_proton_command`, which affects all callers. A separate `new_direct_proton_waitforexitandrun_command` helper would be safer, or the update module can build its own command without using `new_direct_proton_command`.
- **ConsoleView subscribes only to `launch-log`**: Both `ConsoleView.tsx` and `ConsoleDrawer.tsx` hardcode the `'launch-log'` event name. Adding `'update-log'` requires modifying both components to subscribe to both events.
- **No frontend test framework**: There are no frontend tests. The Rust `crosshook-core` crate has tests (`cargo test -p crosshook-core`), but frontend behavior is only validated manually.
- **`State<ProfileStore>` injection**: If the update command needs to load a profile server-side, it must accept `State<'_, ProfileStore>` as a parameter. This is already done by `profile_load` and `profile_save`. However, the feature spec passes paths explicitly in the request, so the command may not need the store directly.
- **Profile filtering on frontend**: The profile selector for update must filter to `proton_run` profiles only. This requires loading each profile to check `launch.method`, or loading all profiles and filtering. The `profile_list` command returns only names, not methods -- the frontend must call `profile_load` for each to determine the method, or a new command could return profiles with their methods.
- **`InstallField` component is not exported**: The `InstallField` component is defined locally inside `InstallGamePanel.tsx` (line 64). To reuse it in `UpdateGamePanel`, it must be extracted to a shared component file.

## Other Docs

- `docs/plans/update-game/feature-spec.md`: Complete feature specification including data models, API contracts, UX workflows, task breakdown, and risk assessment
- `docs/plans/update-game/research-technical.md`: Architecture design, data models, system constraints
- `docs/plans/update-game/research-ux.md`: User workflows, competitive analysis, gamepad navigation
- `docs/plans/update-game/research-business.md`: User stories, business rules, codebase integration analysis
- [Proton GitHub: Wine Prefix Management](https://github.com/ValveSoftware/Proton): Proton CLI and environment variable reference
- [Tauri v2 Commands Documentation](https://v2.tauri.app/develop/calling-rust/): Tauri IPC command patterns
