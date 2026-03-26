# Code Analysis: update-game

## Executive Summary

The update-game feature follows well-established patterns in the CrossHook codebase. The install module (`crates/crosshook-core/src/install/`) provides the exact structural template: a three-file module layout (`mod.rs`, `models.rs`, `service.rs`), serde-driven request/result/error types with `message()` and `Display` impls, and a service function that delegates to `launch/runtime_helpers.rs` for Proton command construction. The launch command module (`src-tauri/src/commands/launch.rs`) provides the streaming pattern: `spawn_log_stream` + `stream_log_lines` polling a log file every 500ms and emitting lines via `app.emit("update-log", ...)`. The frontend follows a hook state machine pattern (`useInstallGame.ts`) with typed stage unions driving conditional rendering, and validation error maps bridging Rust enum variants to TypeScript field names.

## Existing Code Structure

### Related Components

| Layer         | Install (blocking template) | Launch (streaming template) | Update (target)                  |
| ------------- | --------------------------- | --------------------------- | -------------------------------- |
| Core types    | `install/models.rs`         | `launch/` types inline      | `update/models.rs` (new)         |
| Core service  | `install/service.rs`        | `launch/script_runner.rs`   | `update/service.rs` (new)        |
| Core module   | `install/mod.rs`            | `launch/mod.rs`             | `update/mod.rs` (new)            |
| Tauri command | `commands/install.rs`       | `commands/launch.rs`        | `commands/update.rs` (new)       |
| TS types      | `types/install.ts`          | `types/launch.ts`           | `types/update.ts` (new)          |
| Hook          | `hooks/useInstallGame.ts`   | `hooks/useLaunchState.ts`   | `hooks/useUpdateGame.ts` (new)   |
| Component     | `InstallGamePanel.tsx`      | `LaunchPanel.tsx`           | `UpdateGamePanel.tsx` (new)      |
| Page          | `pages/InstallPage.tsx`     | Main tab                    | `pages/InstallPage.tsx` (modify) |
| Event         | n/a                         | `"launch-log"`              | `"update-log"` (new)             |

### File Organization Pattern

The codebase uses a strict three-file module layout for domain modules in `crosshook-core`:

```
crates/crosshook-core/src/{domain}/
  mod.rs        -- Module declarations + selective `pub use` re-exports
  models.rs     -- Request, Result, Error, ValidationError types with serde derives
  service.rs    -- Public business logic functions + private validation helpers + #[cfg(test)] tests
```

The install module also has `discovery.rs` (a fourth file for game executable discovery). The update module will be simpler: only `mod.rs`, `models.rs`, `service.rs`.

## Implementation Patterns

### Pattern: Serde-Driven Type Hierarchy

**Description**: All types crossing the IPC boundary use `#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]`. Request structs have `#[serde(default)]` on every field. Error enums use `#[serde(rename_all = "snake_case")]` and provide a `message(&self) -> String` method for user-facing text. A separate `ValidationError` enum wraps into the main `Error` via `From<>`.

**Example** (from `install/models.rs`):

```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct InstallGameRequest {
    #[serde(default)]
    pub profile_name: String,
    // ...
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum InstallGameError {
    Validation(InstallGameValidationError),
    RuntimeUnavailable,
    // ...
}

impl From<InstallGameValidationError> for InstallGameError {
    fn from(value: InstallGameValidationError) -> Self {
        Self::Validation(value)
    }
}
```

**Apply to**: `update/models.rs` -- create `UpdateGameRequest`, `UpdateGameResult`, `UpdateGameError`, `UpdateGameValidationError` following this exact pattern. The update request is simpler (profile name selector + update exe path + inherited proton/prefix from profile).

### Pattern: Service Function with Proton Command Building

**Description**: The service function validates the request, builds a Proton command using `runtime_helpers` functions, spawns it, and waits for completion. The install version blocks on `child.wait()` via `Handle::block_on`. The update version should instead return the `Child` process for streaming.

**Example** (from `install/service.rs` lines 102-119):

```rust
fn build_install_command(
    request: &InstallGameRequest,
    prefix_path: &Path,
    log_path: &Path,
) -> Result<Command, InstallGameError> {
    let mut command = new_direct_proton_command(request.proton_path.trim());
    command.arg(request.installer_path.trim());
    apply_host_environment(&mut command);
    let prefix_path_string = prefix_path.to_string_lossy().into_owned();
    apply_runtime_proton_environment(&mut command, &prefix_path_string, "");
    apply_working_directory(&mut command, "", Path::new(request.installer_path.trim()));
    attach_log_stdio(&mut command, log_path).map_err(|error| {
        InstallGameError::LogAttachmentFailed { /* ... */ }
    })?;
    Ok(command)
}
```

**Apply to**: `update/service.rs` -- create `build_update_command` following the same sequence: `new_direct_proton_command` -> add exe arg -> `apply_host_environment` -> `apply_runtime_proton_environment` -> `apply_working_directory` -> `attach_log_stdio`. Key difference: the update service returns `(Command, Child)` or just the spawned `Child` so the Tauri layer can stream its logs, rather than blocking.

### Pattern: Thin Tauri Command with Import Aliasing

**Description**: Tauri commands are thin `async fn` wrappers. Core functions are imported with aliases to avoid name collisions (e.g., `install_game as install_game_core`). Errors always convert via `.map_err(|e| e.to_string())`. Blocking operations use `tauri::async_runtime::spawn_blocking`.

**Example** (from `commands/install.rs` lines 30-38):

```rust
#[tauri::command]
pub async fn install_game(request: InstallGameRequest) -> Result<InstallGameResult, String> {
    let log_path = create_log_path("install", &install_log_target_slug(&request.profile_name))?;
    tauri::async_runtime::spawn_blocking(move || {
        install_game_core(&request, &log_path).map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| error.to_string())?
}
```

**Apply to**: `commands/update.rs` -- the update command takes an `AppHandle` (like `launch_game`), creates a log path, calls core `update_game` in `spawn_blocking`, then calls `spawn_log_stream` to stream logs via the `"update-log"` event. Return shape mirrors `LaunchResult`.

### Pattern: Real-Time Log Streaming via File Polling

**Description**: `spawn_log_stream` in `commands/launch.rs` takes `AppHandle`, a log path, and a `Child` process. It spawns a tokio task that reads the log file every 500ms, emitting new lines via `app.emit("launch-log", line)`. After the child exits, a final read captures remaining lines.

**Example** (from `commands/launch.rs` lines 103-166):

```rust
fn spawn_log_stream(app: AppHandle, log_path: PathBuf, child: tokio::process::Child) {
    let handle = tauri::async_runtime::spawn(async move {
        stream_log_lines(app, log_path, child).await;
    });
    tauri::async_runtime::spawn(async move {
        if let Err(error) = handle.await {
            tracing::error!(%error, "launch log stream task failed");
        }
    });
}
```

**Apply to**: `commands/update.rs` -- duplicate `spawn_log_stream` and `stream_log_lines` locally (or extract to a shared utility), changing the event name from `"launch-log"` to `"update-log"`. The shared.md notes this duplication as a known gotcha.

### Pattern: create_log_path Duplication

**Description**: Both `commands/install.rs` and `commands/launch.rs` contain independent `create_log_path` functions with identical logic -- creating timestamped log files under `/tmp/crosshook-logs`. This is a known code duplication.

**Example** (identical in both files):

```rust
fn create_log_path(prefix: &str, target_slug: &str) -> Result<PathBuf, String> {
    let log_dir = PathBuf::from("/tmp/crosshook-logs");
    std::fs::create_dir_all(&log_dir).map_err(|error| error.to_string())?;
    let timestamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| error.to_string())?
        .as_millis();
    let file_name = format!("{prefix}-{target_slug}-{timestamp}.log");
    let log_path = log_dir.join(file_name);
    std::fs::File::create(&log_path).map_err(|error| error.to_string())?;
    Ok(log_path)
}
```

**Apply to**: `commands/update.rs` -- duplicate this function again (following the existing pattern), using `"update"` as the prefix. Consider noting this as tech debt for future extraction.

### Pattern: Hook State Machine with Typed Stages

**Description**: Frontend hooks use `useState` with a union type for stages driving conditional rendering. The hook exposes boolean stage checks (`isIdle`, `isPreparing`, etc.), derived text functions, validation state with field-level errors, and action callbacks.

**Example** (from `useInstallGame.ts`):

```typescript
export type InstallGameStage =
  | 'idle'
  | 'preparing'
  | 'running_installer'
  | 'review_required'
  | 'ready_to_save'
  | 'failed';

// Hook returns derived booleans:
isIdle: stage === 'idle',
isPreparing: stage === 'preparing',
isRunningInstaller: stage === 'running_installer',
```

**Apply to**: `useUpdateGame.ts` -- create `UpdateGameStage = 'idle' | 'validating' | 'running_update' | 'succeeded' | 'failed'`. Simpler than install (no review/save stage). The hook loads profiles via `invoke('profile_list')`, lets user select one, auto-fills proton/prefix, and drives the update action.

### Pattern: Validation Error Maps (TypeScript)

**Description**: TypeScript types maintain three parallel structures: a union type of validation error variants, a `Record<Variant, string>` message map mirroring the Rust `message()` method, and a `Record<Variant, keyof Request>` field map for routing errors to specific form fields.

**Example** (from `types/install.ts`):

```typescript
export const INSTALL_GAME_VALIDATION_MESSAGES: Record<InstallGameValidationError, string> = {
  ProfileNameRequired: 'An install profile name is required.',
  // ...mirrors Rust InstallGameValidationError::message()
};

export const INSTALL_GAME_VALIDATION_FIELD: Record<InstallGameValidationError, keyof InstallGameRequest | null> = {
  ProfileNameRequired: 'profile_name',
  // ...maps to request fields
};
```

**Apply to**: `types/update.ts` -- create parallel maps for `UpdateGameValidationError`. Fewer variants than install (update exe path required, proton path required/missing/not-executable, prefix path required/missing/not-directory).

### Pattern: BEM-Like CSS Class Hierarchy

**Description**: All CSS classes follow `crosshook-{feature}-{element}` naming. The install feature uses: `crosshook-install-shell` (container), `crosshook-install-section` (grouped fields), `crosshook-install-section-title`, `crosshook-install-grid` (2-col field layout), `crosshook-install-card` (status/review area), `crosshook-install-field-control` (input + browse button row).

**Apply to**: Use `crosshook-update-{element}` for all update-specific classes. Reuse generic classes (`crosshook-field`, `crosshook-label`, `crosshook-input`, `crosshook-button`, `crosshook-help-text`, `crosshook-danger`) directly.

### Pattern: Profile Selector via ThemedSelect

**Description**: `ThemedSelect` wraps Radix `@radix-ui/react-select` with the project's dark theme. It maps empty strings to a `__empty__` sentinel (Radix disallows empty-string values). Options are `{value: string, label: string, disabled?: boolean}`.

**Example** (from `InstallGamePanel.tsx` ProtonPathField):

```tsx
<ThemedSelect
  id="install-detected-proton"
  value={selectedPath}
  onValueChange={(val) => {
    if (val.trim().length > 0) props.onChange(val);
  }}
  placeholder="Detected Proton install"
  options={installs.map((install) => ({
    value: install.path,
    label: formatProtonInstallLabel(install, duplicateNameCounts),
  }))}
/>
```

**Apply to**: `UpdateGamePanel.tsx` -- use `ThemedSelect` for the profile selector. Load profiles via `invoke('profile_list')`, display as options with profile name as both value and label. On selection, call `invoke('profile_load', { name })` to retrieve `GameProfile` and auto-fill proton/prefix fields.

### Pattern: ConsoleView/ConsoleDrawer Event Subscription

**Description**: Both `ConsoleView.tsx` and `ConsoleDrawer.tsx` independently subscribe to the `"launch-log"` event via `listen()`. ConsoleView renders the log lines; ConsoleDrawer tracks line count and auto-expands. Both use the `normalizeLogMessage` utility to handle the `LogPayload` union type.

**Example** (from `ConsoleView.tsx` line 48):

```tsx
const unlistenPromise = listen<LogPayload>('launch-log', (event) => {
  const text = normalizeLogMessage(event.payload).trimEnd();
  // ...
});
```

**Apply to**: Both files must add a second `listen()` call for `"update-log"`. The `normalizeLogMessage` function works as-is for the update event since the backend emits plain strings via `app.emit()`.

## Integration Points

### Files to Create

| File                                          | Purpose                                                                                 |
| --------------------------------------------- | --------------------------------------------------------------------------------------- |
| `crates/crosshook-core/src/update/mod.rs`     | Module root with `pub use` re-exports                                                   |
| `crates/crosshook-core/src/update/models.rs`  | `UpdateGameRequest`, `UpdateGameResult`, `UpdateGameError`, `UpdateGameValidationError` |
| `crates/crosshook-core/src/update/service.rs` | `validate_update_request`, `build_update_command`, `update_game` + tests                |
| `src-tauri/src/commands/update.rs`            | `validate_update_request`, `update_game` Tauri commands with `spawn_log_stream`         |
| `src/types/update.ts`                         | TypeScript interfaces mirroring Rust types + validation maps                            |
| `src/hooks/useUpdateGame.ts`                  | Hook state machine for update workflow                                                  |
| `src/components/UpdateGamePanel.tsx`          | UI component with profile selector, update exe picker, status card                      |

### Files to Modify

| File                                      | Change                                                                                                                        |
| ----------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------- |
| `crates/crosshook-core/src/lib.rs`        | Add `pub mod update;` (line 8, after `pub mod steam;`)                                                                        |
| `src-tauri/src/commands/mod.rs`           | Add `pub mod update;` (line 8, after `pub mod steam;`)                                                                        |
| `src-tauri/src/lib.rs`                    | Add `commands::update::validate_update_request` and `commands::update::update_game` to `generate_handler![]` (around line 86) |
| `src/types/index.ts`                      | Add `export * from './update';` (after the install export, line 6)                                                            |
| `src/components/ConsoleView.tsx`          | Add second `listen('update-log', ...)` handler in the `useEffect` (line 48 area)                                              |
| `src/components/layout/ConsoleDrawer.tsx` | Add second `listen('update-log', ...)` handler in the `useEffect` (line 54 area)                                              |
| `src/components/pages/InstallPage.tsx`    | Import and render `UpdateGamePanel` below `InstallGamePanel`                                                                  |
| `src/styles/theme.css`                    | Add `crosshook-update-*` class rules (after the install block, around line 1318)                                              |

## Code Conventions

### Naming

- **Rust modules**: `snake_case` -- `update`, `models`, `service`
- **Rust types**: `PascalCase` with domain prefix -- `UpdateGameRequest`, `UpdateGameError`
- **Rust functions**: `snake_case` with domain verb -- `validate_update_request`, `update_game`, `build_update_command`
- **Rust test functions**: descriptive `snake_case` -- `validate_update_request_returns_specific_field_errors`
- **Tauri commands**: `snake_case` matching frontend `invoke()` names -- `validate_update_request`, `update_game`
- **Import aliasing**: Core functions aliased when names collide -- `update_game as update_game_core`
- **TypeScript types**: `PascalCase` with domain prefix -- `UpdateGameRequest`, `UpdateGameStage`
- **TypeScript hooks**: `camelCase` with `use` prefix -- `useUpdateGame`
- **React components**: `PascalCase` -- `UpdateGamePanel`
- **CSS classes**: `crosshook-{feature}-{element}` BEM-like -- `crosshook-update-shell`, `crosshook-update-section`
- **Event names**: `kebab-case` -- `"update-log"`

### Error Handling

- **Rust core**: Functions return `Result<T, UpdateGameError>`. Validation returns `Result<(), UpdateGameValidationError>`. The `?` operator propagates via `From<ValidationError> for Error`.
- **Rust Tauri commands**: All errors convert to `String` via `.map_err(|e| e.to_string())`. Double unwrap pattern: `spawn_blocking(...).await.map_err(...)?` handles both join error and business error.
- **TypeScript hooks**: `try/catch` around `invoke()` calls. The `mapValidationErrorToField` function matches the stringified Rust error message against the `VALIDATION_MESSAGES` map to route errors to specific form fields. Unmatched errors go to `generalError`.
- **Error display chain**: Rust `Display` impl calls `message()` method -> Tauri `map_err(|e| e.to_string())` -> TypeScript receives the user-facing string -> hook maps it to field or general error.

### Testing

- **Rust tests**: Inline `#[cfg(test)] mod tests` within `service.rs` and `models.rs`. Use `tempfile::tempdir()` for filesystem isolation. Create a `valid_request(temp_dir)` factory function that writes fixture files. Test both happy path and specific validation error variants with `assert!(matches!(...))`.
- **Test runtime pattern**: For functions needing tokio, tests build a `tokio::runtime::Builder::new_current_thread()` runtime and use `block_on(async { spawn_blocking(...) })`.
- **No frontend tests**: The project has no frontend test framework configured. TypeScript types are verified at compile time only.

## Dependencies and Services

### Available Utilities (reuse directly)

| Utility                            | Location                                               | Purpose                                                                   |
| ---------------------------------- | ------------------------------------------------------ | ------------------------------------------------------------------------- |
| `new_direct_proton_command`        | `launch/runtime_helpers.rs:12`                         | Creates a `Command` for `proton run <exe>` with env cleared               |
| `apply_host_environment`           | `launch/runtime_helpers.rs:46`                         | Sets HOME, USER, PATH, DISPLAY, etc. on command                           |
| `apply_runtime_proton_environment` | `launch/runtime_helpers.rs:62`                         | Sets WINEPREFIX, STEAM_COMPAT_DATA_PATH, STEAM_COMPAT_CLIENT_INSTALL_PATH |
| `apply_working_directory`          | `launch/runtime_helpers.rs:122`                        | Sets cwd from configured dir or exe parent                                |
| `attach_log_stdio`                 | `launch/runtime_helpers.rs:139`                        | Redirects stdout/stderr to log file                                       |
| `resolve_wine_prefix_path`         | `launch/runtime_helpers.rs:94`                         | Resolves `pfx` subdirectory within prefix                                 |
| `validate_name`                    | `profile/legacy.rs` (re-exported via `profile/mod.rs`) | Validates profile name characters                                         |
| `ProfileStore::list`               | `profile/toml_store.rs:119`                            | Returns `Vec<String>` of profile names                                    |
| `ProfileStore::load`               | `profile/toml_store.rs:83`                             | Returns `GameProfile` for a given name                                    |
| `normalizeLogMessage`              | `utils/log.ts`                                         | Extracts display text from log event payload                              |
| `chooseFile` / `chooseDirectory`   | `utils/dialog.ts`                                      | Tauri file/directory picker wrappers                                      |
| `ThemedSelect`                     | `components/ui/ThemedSelect.tsx`                       | Radix-based themed dropdown                                               |

### Required Dependencies (already in Cargo.toml)

- `tokio` (process, fs, rt, sync) -- for Command, spawn_blocking, async file read
- `serde` with derive -- for all IPC types
- `directories` -- for BaseDirs (if needed for prefix resolution)
- `tracing` -- for structured logging in error paths
- `tempfile` (dev) -- for tests

### Frontend Dependencies (already in package.json)

- `@tauri-apps/api` -- `invoke()` for IPC, `listen()` for events
- `@radix-ui/react-select` -- via `ThemedSelect` component
- `@tauri-apps/plugin-dialog` -- via `chooseFile()`/`chooseDirectory()`

## Gotchas and Warnings

- **`create_log_path` duplication**: Both `commands/install.rs` and `commands/launch.rs` define their own identical `create_log_path`. The update command must duplicate it again (or factor out). The existing codebase has not extracted this to a shared module.

- **`spawn_log_stream` / `stream_log_lines` duplication**: These functions live in `commands/launch.rs` and are private (`fn`, not `pub fn`). The update command must duplicate them or they must be refactored to a shared location. The event name is hardcoded (`"launch-log"`) so a copy is necessary to emit `"update-log"`.

- **ConsoleView/ConsoleDrawer double-subscribe**: Both components independently subscribe to events. Adding `"update-log"` requires changes in both files. Missing either one will cause partial log display (ConsoleView shows lines but drawer badge does not update, or vice versa).

- **Radix Select empty-string sentinel**: `ThemedSelect` maps `""` to `"__empty__"` internally. The profile selector must ensure the initial unselected state uses `""` as the value, and the `onValueChange` handler checks for empty before loading a profile.

- **`install_game` blocks via `Handle::block_on(child.wait())`**: The install pattern blocks the calling thread until the installer exits. The update command should NOT follow this pattern -- it should follow the launch pattern: spawn the child, pass it to `spawn_log_stream`, and return immediately. The install blocking pattern exists because install needs the process result to discover executables; update does not need post-process discovery.

- **Proton `run` argument**: `new_direct_proton_command` already appends `"run"` as the first arg. The update exe path is added as a subsequent arg via `command.arg()`. Do not add `"run"` manually.

- **`apply_runtime_proton_environment` takes string refs, not `Path`**: The function signature takes `&str` for both `prefix_path` and `steam_client_install_path`. The second arg can be `""` for update (install also passes `""`).

- **Profile `launch.method` filtering**: Update targets `proton_run` profiles only. The profile list from `ProfileStore::list()` returns all profiles. The frontend must filter or the backend must accept a profile name and validate that its `launch.method == "proton_run"`.

- **Event payload type**: The launch backend emits `app.emit("launch-log", line.to_string())` where `line` is a plain `String`. The `LogPayload` type on the frontend handles this via the `typeof payload === 'string'` branch in `normalizeLogMessage`. The update event can follow the same approach.

- **Prefix path `pfx` resolution**: `resolve_wine_prefix_path` auto-detects whether the prefix has a `pfx` subdirectory and adjusts. The update service should use the profile's `runtime.prefix_path` (already resolved by the profile) and let `apply_runtime_proton_environment` handle the `pfx` resolution internally.

## Task-Specific Guidance

- **For Rust backend tasks (update/models.rs, update/service.rs, update/mod.rs)**:
  - Mirror the install module's three-file layout exactly. The `mod.rs` should declare `mod models; mod service;` and re-export public items.
  - `UpdateGameRequest` needs: `profile_name: String`, `update_executable_path: String`, `proton_path: String`, `prefix_path: String`. All fields `#[serde(default)]`.
  - `UpdateGameResult` needs: `succeeded: bool`, `message: String`, `helper_log_path: String`. Simpler than install (no profile generation or discovery).
  - `UpdateGameError` enum variants: `Validation(UpdateGameValidationError)`, `RuntimeUnavailable`, `LogAttachmentFailed`, `UpdateSpawnFailed`, `UpdateWaitFailed`, `UpdateExitedWithFailure`. Each with `message()` and `Display`.
  - `UpdateGameValidationError` variants: `ProfileNameRequired`, `UpdateExecutablePathRequired`, `UpdateExecutablePathMissing`, `UpdateExecutablePathNotFile`, `UpdateExecutablePathNotWindowsExecutable`, `ProtonPathRequired`, `ProtonPathMissing`, `ProtonPathNotExecutable`, `PrefixPathRequired`, `PrefixPathMissing`, `PrefixPathNotDirectory`.
  - The `update_game` function should validate, build the command, spawn it, and return the `Child` + log path (for streaming), NOT block on completion. This is the key difference from install.
  - Tests: create `valid_request(temp_dir)` factory, test validation error routing, test command construction (verify env vars set correctly). Use `tempfile::tempdir()`.

- **For Tauri command tasks (commands/update.rs)**:
  - Import with alias: `use crosshook_core::update::{update_game as update_game_core, validate_update_request as validate_update_request_core, UpdateGameRequest};`
  - The `update_game` command takes `app: AppHandle` and `request: UpdateGameRequest`. Create log path with `create_log_path("update", &slug)`. Call core in `spawn_blocking` to get the `Child`. Then `spawn_log_stream(app, log_path, child)` to stream.
  - Duplicate `create_log_path`, `spawn_log_stream`, `stream_log_lines` locally, changing event name to `"update-log"`.
  - Register both `commands::update::validate_update_request` and `commands::update::update_game` in `generate_handler![]` in `src-tauri/src/lib.rs`.

- **For frontend tasks (types/update.ts, useUpdateGame.ts, UpdateGamePanel.tsx)**:
  - `types/update.ts`: Mirror `types/install.ts` structure. Simpler request/result interfaces. Create `UPDATE_GAME_VALIDATION_MESSAGES` and `UPDATE_GAME_VALIDATION_FIELD` maps.
  - `useUpdateGame.ts`: Simpler state machine than `useInstallGame`. States: `idle -> validating -> running_update -> succeeded | failed`. No review/save stages. Load profile list on mount. On profile select, invoke `profile_load` to get `GameProfile` and auto-fill `proton_path` = `profile.runtime.proton_path`, `prefix_path` = `profile.runtime.prefix_path`. Filter to `proton_run` profiles only.
  - `UpdateGamePanel.tsx`: Render inside `crosshook-update-shell` container. Profile selector (`ThemedSelect`), update exe path (`InstallField` or similar), auto-filled proton/prefix (read-only or editable), status card, action button. Co-locate with `InstallGamePanel` on the Install page.
  - `ConsoleView.tsx` and `ConsoleDrawer.tsx`: Add a second `listen('update-log', ...)` call inside the existing `useEffect`. Both listeners share the same `setLines` / `setLineCount` state so update and launch logs interleave in the same console.
  - `InstallPage.tsx`: Import `UpdateGamePanel` and render it below `InstallGamePanel` within the page layout. No props coupling needed between the two panels.
  - `types/index.ts`: Add `export * from './update';` barrel export.
  - `theme.css`: Add `crosshook-update-shell`, `crosshook-update-section`, `crosshook-update-section-title` etc. following the `crosshook-install-*` class pattern and visual style.
