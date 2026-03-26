# Architecture Research: update-game

## System Overview

CrossHook is a Tauri v2 desktop application with a Rust workspace backend (`crosshook-core` library + `src-tauri` app shell) and a React 18 + TypeScript frontend. The backend exposes all operations as `#[tauri::command]` functions invoked from React via `@tauri-apps/api/core`. The update-game feature slots into this architecture as a new `update` module in `crosshook-core` (sibling to `install`), a new `commands/update.rs` in `src-tauri`, and a new `UpdateGamePanel` component co-located on the existing Install page.

## Relevant Components

### Backend (Rust)

- `crates/crosshook-core/src/lib.rs`: Module root -- add `pub mod update;` here (currently declares `community`, `export`, `install`, `launch`, `logging`, `profile`, `settings`, `steam`)
- `crates/crosshook-core/src/install/mod.rs`: Module pattern to follow -- declares sub-modules and re-exports public API
- `crates/crosshook-core/src/install/models.rs`: Data model pattern -- `InstallGameRequest`, `InstallGameResult`, `InstallGameError`, `InstallGameValidationError` with `message()` methods
- `crates/crosshook-core/src/install/service.rs`: Service pattern -- `install_game()`, `validate_install_request()`, `build_install_command()`, plus private validation/helper functions
- `crates/crosshook-core/src/launch/runtime_helpers.rs`: Shared primitives for Proton command building -- `new_direct_proton_command()`, `apply_host_environment()`, `apply_runtime_proton_environment()`, `apply_working_directory()`, `attach_log_stdio()`, `resolve_wine_prefix_path()`
- `src-tauri/src/commands/mod.rs`: Command module registry -- add `pub mod update;` here
- `src-tauri/src/commands/install.rs`: Tauri command pattern for blocking execution -- `install_game`, `validate_install_request`, `install_default_prefix_path`
- `src-tauri/src/commands/launch.rs`: Tauri command pattern for streaming execution -- `launch_game`, `launch_trainer`, `spawn_log_stream`, `stream_log_lines`
- `src-tauri/src/lib.rs`: Command registration in `invoke_handler` macro -- add new update commands here
- `src-tauri/src/commands/profile.rs`: `profile_list` and `profile_load` -- existing commands the frontend calls to populate profile selector

### Frontend (React/TypeScript)

- `src/types/install.ts`: Type definition pattern -- `InstallGameRequest`, `InstallGameResult`, `InstallGameStage`, validation error maps
- `src/types/launch.ts`: Launch types pattern -- `LaunchPhase` enum, `LaunchRequest`, `LaunchResult`, `LaunchFeedback`
- `src/types/index.ts`: Type re-export barrel -- add `export * from './update';`
- `src/hooks/useInstallGame.ts`: Hook pattern for form+execution state -- request state, validation state, stage machine, prefix resolution, `startInstall()`, `reset()`
- `src/hooks/useLaunchState.ts`: Hook pattern for streaming execution -- `useReducer` state machine with phases, dispatch actions, `launchGame()`/`launchTrainer()`
- `src/components/InstallGamePanel.tsx`: Panel component pattern -- form fields, Proton selector, status card, action buttons; uses `useInstallGame` hook
- `src/components/LaunchPanel.tsx`: Simpler panel consuming `useLaunchState` -- status display, action buttons, feedback rendering
- `src/components/ConsoleView.tsx`: Log display -- subscribes to `'launch-log'` Tauri event via `listen()`, renders timestamped lines with auto-scroll
- `src/components/layout/ConsoleDrawer.tsx`: Console wrapper -- subscribes to `'launch-log'` to auto-expand and track line count; wraps `ConsoleView`
- `src/components/pages/InstallPage.tsx`: Page orchestrator -- renders `InstallGamePanel` + `ProfileReviewModal`; add `UpdateGamePanel` below `InstallGamePanel`
- `src/components/layout/ContentArea.tsx`: Route renderer -- `InstallPage` renders at `case 'install'`
- `src/components/layout/Sidebar.tsx`: Navigation -- `AppRoute` type; Install page is under "Setup" section
- `src/components/ui/ThemedSelect.tsx`: Reusable themed dropdown (used in InstallGamePanel and LaunchPage)
- `src/utils/dialog.ts`: `chooseFile()` and `chooseDirectory()` wrappers around `@tauri-apps/plugin-dialog`
- `src/utils/log.ts`: `normalizeLogMessage()` -- handles multiple payload shapes from backend events

## Data Flow

### Install Game Flow (blocking pattern -- for reference, update diverges from this)

```
1. User fills form in InstallGamePanel
2. useInstallGame.startInstall() is called
3. Hook invokes validate_install_request (Tauri IPC) -> crosshook-core install::validate_install_request
4. If valid, hook invokes install_game (Tauri IPC)
5. commands/install.rs::install_game spawns a blocking task via tauri::async_runtime::spawn_blocking
6. Inside spawn_blocking: install_game_core calls build_install_command (which uses runtime_helpers),
   spawns the process, and blocks on child.wait() via Handle::block_on()
7. Process completes -> InstallGameResult returned to frontend
8. Hook updates stage/result state; InstallGamePanel re-renders
```

Key detail: the install flow runs `spawn_blocking` + `block_on(child.wait())`, meaning the IPC call blocks until the installer process exits. Logs are written to a file and only visible after completion.

### Launch Game Flow (streaming pattern -- update should follow this)

```
1. User clicks Launch Game in LaunchPanel
2. useLaunchState.launchGame() dispatches "game-start" action
3. Hook invokes validate_launch (synchronous Tauri IPC)
4. If valid, hook invokes launch_game (async Tauri IPC)
5. commands/launch.rs::launch_game:
   a. Builds command via build_proton_game_command (which calls runtime_helpers)
   b. Spawns the child process
   c. Calls spawn_log_stream(app, log_path, child) -- returns immediately
   d. Returns LaunchResult with helper_log_path to frontend
6. spawn_log_stream runs a background tokio task:
   a. Polls log file every 500ms via tokio::fs::read_to_string
   b. Emits new lines via app.emit("launch-log", line) as Tauri events
   c. Checks child.try_wait() each iteration
   d. On process exit, performs final log read
7. Frontend ConsoleView listens to 'launch-log' events via listen() from @tauri-apps/api/event
8. ConsoleDrawer also listens to auto-expand on new lines
```

The update-game feature spec mandates following this streaming pattern with a dedicated `update-log` event.

### Profile Loading Flow (relevant for update's profile selector)

```
1. Frontend calls invoke('profile_list') -> commands/profile.rs::profile_list -> ProfileStore::list()
2. Returns Vec<String> of profile names
3. Frontend calls invoke('profile_load', { name }) -> commands/profile.rs::profile_load -> ProfileStore::load()
4. Returns GameProfile with all sections (game, trainer, injection, steam, runtime, launch)
5. For update: frontend filters profiles where launch.method === 'proton_run'
6. Selected profile provides: runtime.prefix_path, runtime.proton_path (auto-fill update form)
```

### Proton Command Building Flow (core of update execution)

```
install/service.rs::build_install_command (existing reference):
  1. new_direct_proton_command(proton_path)  -- creates Command with "run" arg, env_clear()
  2. command.arg(installer_path)             -- the executable to run
  3. apply_host_environment(&command)        -- HOME, PATH, DISPLAY, etc.
  4. apply_runtime_proton_environment(&command, prefix_path, steam_client_install_path)
     - Sets WINEPREFIX via resolve_wine_prefix_path (handles pfx/ subdirectory)
     - Sets STEAM_COMPAT_DATA_PATH
     - Sets STEAM_COMPAT_CLIENT_INSTALL_PATH
  5. apply_working_directory(&command, "", exe_path)  -- defaults to exe parent dir
  6. attach_log_stdio(&command, log_path)    -- redirects stdout/stderr to log file

update/service.rs should call the same sequence with updater_path instead of installer_path.
```

## Integration Points

### Backend: Where New Code Connects

1. **New module directory**: `crates/crosshook-core/src/update/` with `mod.rs`, `models.rs`, `service.rs`
2. **Module registration**: Add `pub mod update;` to `crates/crosshook-core/src/lib.rs` (line 3-8, between `install` and `launch` alphabetically, or after `steam`)
3. **New Tauri command file**: `src-tauri/src/commands/update.rs`
4. **Command module registration**: Add `pub mod update;` to `src-tauri/src/commands/mod.rs`
5. **Command handler registration**: Add to `invoke_handler` in `src-tauri/src/lib.rs` (around lines 83-85, after install commands):

   ```
   commands::update::update_game,
   commands::update::validate_update_request,
   ```

6. **Event name**: Use `"update-log"` (distinct from `"launch-log"`) to avoid cross-contamination

### Frontend: Where New Code Connects

1. **New type file**: `src/types/update.ts`
2. **Type barrel export**: Add `export * from './update';` to `src/types/index.ts`
3. **New hook file**: `src/hooks/useUpdateGame.ts`
4. **New component file**: `src/components/UpdateGamePanel.tsx`
5. **Page integration**: Import and render `UpdateGamePanel` in `src/components/pages/InstallPage.tsx` below `InstallGamePanel` (after line 372)
6. **ConsoleDrawer subscription**: `src/components/layout/ConsoleDrawer.tsx` must subscribe to `'update-log'` in addition to `'launch-log'` (line 54)
7. **ConsoleView subscription**: `src/components/ConsoleView.tsx` must also subscribe to `'update-log'` (line 48)

### Shared Component Extraction (prerequisite for clean reuse)

The feature spec calls for extracting these from `InstallGamePanel.tsx` into shared files:

- `InstallField` component (lines 64-111) -- generic labeled input with browse button
- `ProtonPathField` component (lines 113-179) -- Proton selector with detected installs dropdown

## Key Dependencies

### Rust Crate Dependencies (all existing, no new dependencies needed)

| Dependency    | Version | Used For                                   | Location                  |
| ------------- | ------- | ------------------------------------------ | ------------------------- |
| `tokio`       | 1.x     | `Command`, `process`, async runtime        | crosshook-core, src-tauri |
| `serde`       | 1.x     | `Serialize`/`Deserialize` on IPC types     | crosshook-core, src-tauri |
| `tracing`     | 0.1     | Structured logging in command handlers     | crosshook-core, src-tauri |
| `directories` | 5       | Not needed for update (no prefix creation) | crosshook-core            |
| `tempfile`    | 3       | Dev-only: test fixtures                    | crosshook-core (dev)      |

### Frontend Dependencies (all existing)

| Package                     | Used For                                   |
| --------------------------- | ------------------------------------------ |
| `@tauri-apps/api/core`      | `invoke()` for Tauri IPC calls             |
| `@tauri-apps/api/event`     | `listen()` for Tauri event subscription    |
| `@tauri-apps/plugin-dialog` | File/directory browse dialogs via `open()` |

### Internal Module Dependencies

The new `update` module in `crosshook-core` will import:

- `crate::launch::runtime_helpers::{new_direct_proton_command, apply_host_environment, apply_runtime_proton_environment, apply_working_directory, attach_log_stdio}` -- command building
- `crate::launch::runtime_helpers::resolve_wine_prefix_path` -- used indirectly via `apply_runtime_proton_environment`

The new `commands/update.rs` in `src-tauri` will import:

- `crosshook_core::update::*` -- service functions and types
- `tauri::{AppHandle, Emitter}` -- for event emission (streaming pattern)
- `tokio::process::Child` -- for process handle in streaming

The frontend `useUpdateGame` hook will call:

- `invoke('profile_list')` -- existing command for profile dropdown
- `invoke('profile_load', { name })` -- existing command to load profile data
- `invoke('list_proton_installs')` -- existing command for Proton selector
- `invoke('validate_update_request', { request })` -- new command
- `invoke('update_game', { request })` -- new command

## Architectural Patterns

- **Module structure**: Each domain (`install`, `launch`, `export`, etc.) is a directory under `crosshook-core/src/` with `mod.rs` (re-exports), `models.rs` (types + error types with `message()` methods), and `service.rs` (business logic). The `update` module follows this exactly.
- **Tauri command pattern**: Commands in `src-tauri/src/commands/*.rs` are thin wrappers that delegate to `crosshook-core` functions. Blocking operations use `tauri::async_runtime::spawn_blocking`. Streaming operations spawn a child process, call `spawn_log_stream`, and return immediately.
- **Error handling**: Core errors are enums with `message()` methods and `Display`/`Error` impls. Tauri commands convert errors to `String` via `.map_err(|error| error.to_string())`. The frontend maps error strings back to field names using validation message maps.
- **Log streaming**: Backend writes process stdout/stderr to a log file via `attach_log_stdio`. A background tokio task (`stream_log_lines`) polls the file every 500ms and emits each new line as a Tauri event. Frontend subscribes via `listen()`.
- **Frontend state machines**: Install uses `useState` with a stage union type (`'idle' | 'preparing' | 'running_installer' | ...`). Launch uses `useReducer` with a `LaunchPhase` enum and typed actions. The update hook should follow the launch `useReducer` pattern since it is a streaming operation.
- **Profile context**: `ProfileContext` wraps `useProfile` hook and provides global profile state. However, `UpdateGamePanel` should NOT use the global profile context for its profile selection -- it needs its own independent profile loader (the selected profile for update is independent of the "active" profile used for launch). Use direct `invoke('profile_list')` and `invoke('profile_load')` calls within the hook.

## Gotchas and Edge Cases

- **Streaming vs. blocking divergence**: The install flow uses `spawn_blocking` + `block_on(child.wait())` which blocks the IPC call until completion. The update flow must NOT follow this pattern -- it must follow the launch pattern with `spawn_log_stream` so the Tauri command returns immediately and logs stream in real-time. The feature spec explicitly mandates this at "Decision 2: Log Streaming".
- **Event channel isolation**: The feature spec mandates a dedicated `update-log` event (not reusing `launch-log`) to avoid mixing output when both are active. This means `ConsoleDrawer.tsx` and `ConsoleView.tsx` each need a second `listen()` subscription.
- **`resolve_wine_prefix_path` handles both prefix layouts**: If the prefix path ends in `pfx/` or contains a `pfx/` subdirectory, it uses that; otherwise it uses the path directly. This is critical for Steam compatdata prefixes vs. standalone CrossHook prefixes.
- **`new_direct_proton_command` adds "run" verb automatically**: The function creates `Command::new(proton_path)` and adds `.arg("run")`. The feature spec mentions `waitforexitandrun` as preferred for blocking updates, but since the streaming pattern is chosen, `run` is correct (the tokio task monitors process exit separately). If `waitforexitandrun` is desired, a new helper or parameter would be needed.
- **`env_clear()` in command builder**: `new_direct_proton_command` calls `command.env_clear()` -- the child process starts with a clean environment, then `apply_host_environment` and `apply_runtime_proton_environment` add back only the required variables. The update command gets the same clean-environment treatment.
- **`attach_log_stdio` redirects both stdout and stderr to the same file**: The log file captures everything the Proton process writes. This is the file that `stream_log_lines` polls.
- **Profile filtering for `proton_run` only**: The frontend must filter the profile list to exclude `native` and `steam_applaunch` profiles. The profile's `launch.method` field determines this. Profiles loaded via `profile_load` include the full `GameProfile` struct.
- **`create_log_path` is duplicated**: Both `commands/install.rs` and `commands/launch.rs` have their own `create_log_path` function with identical logic. The new `commands/update.rs` will need a third copy unless these are extracted to a shared module. Consider extracting to a `commands` utility or using the launch version directly.
- **Tauri capabilities**: The `capabilities/default.json` file only lists `core:default` and `dialog:default`. The `emit()` API for backend-to-frontend events is covered by `core:default`, so no capability changes are needed for the new `update-log` event.
- **No new sidebar route needed**: The `AppRoute` union type in `Sidebar.tsx` does not need to change. The update panel lives on the existing `install` route.
- **Working directory default**: `apply_working_directory` with an empty `configured_directory` falls back to the parent directory of the primary path (the updater exe). This is the correct default for update executables.

## Other Docs

- `docs/plans/update-game/feature-spec.md`: Complete feature specification with data models, API contracts, UX workflows, task breakdown, and resolved decisions
- `docs/plans/update-game/research-technical.md`: Technical deep-dive on architecture design and system constraints
- `docs/plans/update-game/research-ux.md`: UX workflows, competitive analysis, gamepad navigation patterns
- `docs/plans/update-game/research-business.md`: User stories, business rules, codebase integration analysis
- `docs/plans/update-game/research-external.md`: Proton CLI reference, environment variables, prefix management
- `docs/plans/update-game/research-recommendations.md`: Implementation approach, risk assessment, task breakdown
