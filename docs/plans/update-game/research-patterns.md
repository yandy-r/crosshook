# Pattern Research: update-game

This report documents the concrete coding patterns, conventions, and architectural decisions in the CrossHook codebase that are relevant to implementing the update-game feature. Every pattern is traced to its source file with line references. The update-game feature closely mirrors the existing install module but is simpler (no prefix provisioning, no executable discovery, no profile generation) and adds real-time log streaming from the launch module.

## Relevant Files

### Rust (crosshook-core)

- `crates/crosshook-core/src/install/mod.rs`: Module root with selective re-exports -- pattern to follow for `update/mod.rs`
- `crates/crosshook-core/src/install/models.rs`: Request/Result/Error/ValidationError type definitions with serde, Display, Error impls
- `crates/crosshook-core/src/install/service.rs`: Service functions (`install_game`, `validate_install_request`), command building, validation helpers, tests
- `crates/crosshook-core/src/launch/runtime_helpers.rs`: `new_direct_proton_command`, `apply_host_environment`, `apply_runtime_proton_environment`, `apply_working_directory`, `attach_log_stdio` -- all reusable for update
- `crates/crosshook-core/src/launch/request.rs`: `LaunchRequest` struct and `ValidationError` enum with `message()`, `help()`, `severity()` methods
- `crates/crosshook-core/src/lib.rs`: Module registration -- add `pub mod update;`

### Tauri Commands

- `src-tauri/src/commands/install.rs`: Thin command wrappers with `map_err(|e| e.to_string())` and `spawn_blocking` pattern
- `src-tauri/src/commands/launch.rs`: `spawn_log_stream` and `stream_log_lines` for real-time log event emission -- the pattern to clone for `update-log` events
- `src-tauri/src/commands/mod.rs`: Command module registration -- add `pub mod update;`
- `src-tauri/src/lib.rs`: Command handler registration in `tauri::generate_handler![]` macro (line 69-104)

### Frontend Types

- `src/types/install.ts`: TypeScript interfaces mirroring Rust types, validation error union type, validation message/field maps, stage type
- `src/types/launch.ts`: `LaunchPhase` enum, `LaunchRequest` interface, `LaunchValidationIssue`, type guard function
- `src/types/index.ts`: Re-export barrel -- add `export * from './update';`

### Frontend Hooks

- `src/hooks/useInstallGame.ts`: Full state machine hook with request/validation/stage/result/error state, validation-to-field mapping, derived status/hint/action text
- `src/hooks/useLaunchState.ts`: `useReducer`-based state machine with action dispatch, validation integration, derived status/hint text

### Frontend Components

- `src/components/InstallGamePanel.tsx`: `InstallField` (lines 64-111), `ProtonPathField` (lines 113-179), `CandidateRow` component -- shared UI building blocks
- `src/components/pages/InstallPage.tsx`: Page shell with `PageBanner`, panel rendering, profile review modal orchestration
- `src/components/pages/LaunchPage.tsx`: Simpler page shell showing how profile selector + panel + feature sections compose
- `src/components/ConsoleView.tsx`: Event listener for `launch-log` (line 48) using `listen` from `@tauri-apps/api/event`
- `src/components/layout/ConsoleDrawer.tsx`: Badge counter and auto-expand on log event (line 54)
- `src/components/layout/ContentArea.tsx`: Route-to-page mapping with exhaustive switch (line 34-51)
- `src/components/layout/Sidebar.tsx`: `AppRoute` type and `SIDEBAR_SECTIONS` registry (lines 12, 32-51)
- `src/components/ui/ThemedSelect.tsx`: Radix-based themed dropdown used for profile and Proton selectors

### Styles

- `src/styles/theme.css` (line 728-1318): `.crosshook-install-*` class hierarchy for install shell, sections, cards, fields, candidates, status
- `src/styles/variables.css`: CSS custom properties (`--crosshook-color-*`, `--crosshook-radius-*`, `--crosshook-touch-target-min`)

### Utilities

- `src/utils/dialog.ts`: `chooseFile` and `chooseDirectory` wrappers around `@tauri-apps/plugin-dialog`
- `src/utils/log.ts`: `LogPayload` type union and `normalizeLogMessage` for event payload extraction

## Architectural Patterns

### Module Structure (Rust)

- **Three-file module layout**: Each domain module uses `mod.rs` (re-exports), `models.rs` (types), `service.rs` (logic). The install module adds `discovery.rs` for post-install scanning. The update module needs only `mod.rs`, `models.rs`, `service.rs`.
- **Selective pub re-exports**: `mod.rs` re-exports only the public API surface. Private helpers stay internal to `service.rs`. See `crates/crosshook-core/src/install/mod.rs` lines 1-14.
- **lib.rs one-liner registration**: Each module is added to `crates/crosshook-core/src/lib.rs` as `pub mod update;`.

### Tauri Command Wrapping (Thin Command Pattern)

- **Pattern**: Tauri commands are thin async wrappers that call into `crosshook_core` functions. They do not contain business logic.
- **`spawn_blocking` for CPU-bound work**: Install commands use `tauri::async_runtime::spawn_blocking` because the core functions call `Handle::try_current().block_on()` internally. See `src-tauri/src/commands/install.rs` lines 12-18.
- **String error conversion**: All error types are mapped to `String` via `map_err(|error| error.to_string())`. Tauri IPC requires serializable error types. See `src-tauri/src/commands/install.rs` line 24.
- **Import aliasing**: Core functions are aliased on import to avoid name collisions: `install_game as install_game_core`. See `src-tauri/src/commands/install.rs` lines 4-8.
- **Log path creation**: Both install and launch commands create timestamped log paths under `/tmp/crosshook-logs/`. Both use the same `create_log_path` helper (duplicated in each module). The update module should follow suit.

### Real-Time Log Streaming (Launch Pattern)

- **`spawn_log_stream` function**: Takes `AppHandle`, `PathBuf` (log file), `tokio::process::Child`. Spawns an async task that polls the log file every 500ms and emits lines via `app.emit("launch-log", line)`. See `src-tauri/src/commands/launch.rs` lines 103-166.
- **Polling loop**: Reads the full file, tracks `last_len`, emits only new content. Handles file truncation (resets `last_len` if content shrinks). Continues until `child.try_wait()` returns `Some`.
- **Final read**: After the process exits, performs one last read to capture trailing lines. This is critical -- without it, the last few lines can be lost.
- **Dedicated event channel**: The feature spec requires a new `update-log` event name to avoid cross-contamination with `launch-log`. ConsoleView and ConsoleDrawer must subscribe to both.
- **Non-blocking return**: `launch_game` returns `LaunchResult` immediately after spawning; the log stream runs independently. For update-game, the spec calls for the same pattern -- return immediately, stream logs, let the frontend track completion via stage transitions.

### React Hook State Machine

- **Install hook pattern** (`useInstallGame.ts`): Uses multiple `useState` calls for request, validation, stage, result, error, and derived values. The stage type (`InstallGameStage`) drives conditional rendering. `useCallback` wraps all setters. Derived values (`statusText`, `hintText`, `actionLabel`) are computed from stage + state inside the hook body.
- **Launch hook pattern** (`useLaunchState.ts`): Uses `useReducer` with typed actions for a simpler state machine. Better for linear phase transitions (idle -> launching -> active). Returns derived `statusText`, `hintText`, `actionLabel`, boolean flags.
- **For update-game**: The `useState`-based pattern from install is more appropriate since update has field-level validation state, but the stage transitions are simpler (idle -> preparing -> running -> complete/failed). The hook should be a simplified version of `useInstallGame`.

### Frontend Type Convention

- **Mirrored types**: TypeScript interfaces exactly mirror Rust struct field names using `snake_case`. See `InstallGameRequest` in `src/types/install.ts` (lines 5-13) matching `install/models.rs` (lines 13-28).
- **Validation error union type**: A string literal union lists all validation error variant names in PascalCase matching the Rust enum variant names. See `InstallGameValidationError` in `src/types/install.ts` (lines 40-56).
- **Validation message map**: A `Record<ErrorType, string>` maps each variant to its user-facing message, kept in sync with the Rust `message()` method. See `INSTALL_GAME_VALIDATION_MESSAGES` (lines 59-76).
- **Validation field map**: A `Record<ErrorType, keyof Request | null>` maps each validation error to the request field it belongs to, enabling field-level error display. See `INSTALL_GAME_VALIDATION_FIELD` (lines 78-95).
- **Stage type**: A string literal union defines the state machine stages. See `InstallGameStage` (lines 97-103).
- **Validation state interface**: Combines field-level errors (`Partial<Record<keyof Request, string>>`) and a general error (`string | null`). See `InstallGameValidationState` (lines 107-110).

### CSS Naming and Structure

- **BEM-like prefix**: All classes use `crosshook-` prefix. Component-scoped names use `crosshook-{component}-{element}` (e.g., `crosshook-install-shell`, `crosshook-install-card`, `crosshook-install-stage`).
- **Shell container**: Top-level component uses `crosshook-{feature}-shell` with a consistent visual treatment: `border-radius: 20px`, `border: 1px solid rgba(120, 160, 255, 0.22)`, `background: rgba(14, 20, 40, 0.82)`, `backdrop-filter: blur(18px)`. See theme.css line 728.
- **Section grouping**: Interior sections use `crosshook-{feature}-section` with softer styling: `border-radius: 16px`, `border: 1px solid rgba(255, 255, 255, 0.06)`. See theme.css line 1193.
- **Grid layout**: Two-column grid for related fields (`crosshook-install-grid`), single-column stack for runtime fields (`crosshook-install-runtime-stack`). See theme.css lines 1210-1228.
- **For update-game**: Use `crosshook-update-*` prefix. Can share structural classes (`crosshook-install-field-control`, `crosshook-install-card`) or duplicate them as `crosshook-update-*` variants. The feature spec recommends the latter for independence.

### Shared UI Components

- **`InstallField`**: A reusable labeled input with optional browse button, help text, and error display. Defined inline in `InstallGamePanel.tsx` (lines 64-111). The feature spec calls for extracting this to a shared location so UpdateGamePanel can import it.
- **`ProtonPathField`**: Combines a `ThemedSelect` dropdown (for detected Proton installs) with a manual path input and browse button. Defined inline in `InstallGamePanel.tsx` (lines 113-179). Also needs extraction.
- **`ThemedSelect`**: Radix Select wrapper at `src/components/ui/ThemedSelect.tsx`. Handles empty-string-to-sentinel mapping for Radix compatibility.
- **`PageBanner`**: Page header with eyebrow, title, copy, and illustration SVG. Used by all pages.

## Code Conventions

### Rust Naming and Style

- **`snake_case`** for all functions, variables, modules, file names.
- **Struct derives**: Always `#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]`. Add `Default` when the struct has meaningful defaults. See `InstallGameRequest` (models.rs line 12).
- **`#[serde(default)]`** on every field that crosses IPC -- ensures deserialization never fails for missing fields. See `InstallGameRequest` fields (models.rs lines 14-27).
- **`#[serde(rename_all = "snake_case")]`** on error enums -- ensures variant names serialize as `snake_case` for frontend consumption. See `InstallGameError` (models.rs line 49) and `InstallGameValidationError` (models.rs line 63).
- **Explicit `message()` methods**: Error types implement `fn message(&self) -> String` with match arms providing user-facing text. Display trait delegates to `message()`. See models.rs lines 124-212.
- **`impl From<ValidationError> for Error`**: Validation errors auto-convert to the parent error type. See models.rs lines 214-218.
- **Path validation helpers**: Functions like `validate_proton_path`, `validate_prefix_path` return the specific validation error variant. They trim the input, check emptiness, check existence, check type. See service.rs lines 192-239.
- **`is_windows_executable`** and **`is_executable_file`**: Reusable path checks. Both exist in `install/service.rs` (lines 292-315). The update module should import or re-implement these.
- **Slugification**: Profile/target names are converted to lowercase ASCII slugs for log file names using the same pattern: replace non-alphanumeric with `-`, trim `-` from ends. See service.rs lines 272-290 and launch/request.rs lines 104-136.

### TypeScript Naming and Style

- **`PascalCase`** for components, types, interfaces, enums.
- **`camelCase`** for hooks (`useInstallGame`), functions, variables.
- **`snake_case`** for properties that cross the IPC boundary (matching Rust struct fields).
- **Hook return type interface**: Explicitly defined as `UseInstallGameResult` with all fields, setters, derived values, and action functions typed. See `useInstallGame.ts` lines 19-58.
- **`createEmpty*` factory functions**: Used to initialize state with typed defaults. See `createEmptyInstallGameRequest` (line 60) and `createEmptyValidationState` (line 72).
- **`normalizeErrorMessage` helper**: Converts `unknown` errors to strings: `error instanceof Error ? error.message : String(error)`. See `useInstallGame.ts` line 90.
- **`mapValidationErrorToField` function**: Maps error messages back to request fields using the validation maps, with a fallback keyword search. See `useInstallGame.ts` lines 94-137.
- **Derived text functions**: Pure functions like `deriveStatusText`, `deriveHintText` compute display strings from stage + state. Defined outside the hook body as standalone functions. See `useInstallGame.ts` lines 186-242.

### Component Patterns

- **Destructured hook result**: Components destructure the hook return at the top. See `InstallGamePanel.tsx` lines 212-232.
- **Section structure**: Components render sections with `crosshook-install-section` containers, each with a `crosshook-install-section-title` eyebrow. See `InstallGamePanel.tsx` lines 330-412.
- **Conditional rendering**: Uses ternary `{condition ? <Element /> : null}` rather than `&&`. See throughout `InstallGamePanel.tsx`.
- **Button states**: Primary action button uses `disabled={isRunningInstaller || isResolvingDefaultPrefixPath}`. Label changes based on stage.
- **`default export` at bottom**: Components use named export inline and `export default ComponentName;` at the end. See `InstallGamePanel.tsx` line 545.

## Error Handling

### Rust Error Pattern

1. **Nested error enums**: `UpdateGameError` wraps `UpdateGameValidationError` as `Validation(UpdateGameValidationError)` variant. This matches `InstallGameError::Validation(InstallGameValidationError)`.
2. **`message()` method**: Each error variant returns a user-facing message string. The `Display` trait delegates to `message()`:

   ```rust
   impl fmt::Display for InstallGameError {
       fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
           f.write_str(&self.message())
       }
   }
   ```

3. **`Error` trait**: Both error types implement `std::error::Error` with empty body.
4. **`From` conversion**: `impl From<InstallGameValidationError> for InstallGameError` enables `?` operator to auto-wrap validation errors.
5. **Structured error data**: Some variants carry context (`path: PathBuf`, `message: String`). See `InstallGameError::PrefixCreationFailed` (models.rs line 55).

### Tauri Command Error Handling

1. **All commands return `Result<T, String>`**: Tauri IPC requires errors to be serializable. The pattern is always `map_err(|error| error.to_string())`.
2. **Double unwrap for spawn_blocking**: Commands using `spawn_blocking` have two `map_err` layers -- one for the join error, one for the business logic error:

   ```rust
   tauri::async_runtime::spawn_blocking(move || { ... })
       .await
       .map_err(|error| error.to_string())?  // JoinError
   ```

3. **For streaming commands** (launch pattern): The command returns `Ok(LaunchResult)` immediately. Errors during streaming are logged via `tracing::warn!` rather than propagated.

### Frontend Error Handling

1. **Validation-to-field mapping**: When `invoke` throws, the error message is matched against the validation messages map to determine which field to annotate. If no field matches, it becomes a general error:

   ```typescript
   const validationField = mapValidationErrorToField(message);
   if (validationField === null) {
     setGeneralError(message);
   } else {
     setFieldError(validationField, message);
   }
   ```

2. **Error display**: Field errors render as `<p className="crosshook-danger">{error}</p>` below the input. General errors render in the status card.
3. **Stage transitions on error**: On failure, stage moves to `'failed'` and error message is stored. The `hasFailed` boolean flag enables conditional rendering.

## Testing Approach

### Existing Test Patterns

Tests live alongside their source in `#[cfg(test)] mod tests` blocks (not in a separate `tests/` directory). Run with `cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core`.

#### Model Tests (`install/models.rs` lines 220-257)

- Test `reviewable_profile` method with a constructed request and temp directory prefix path.
- Assert field-by-field on the generated profile.

#### Validation Tests (`install/service.rs` lines 361-427)

- Create a `valid_request` fixture using `tempfile::tempdir()` with real files on disk.
- Write executable scripts using a `write_executable_script` helper that sets `0o111` permissions on Unix.
- Assert `validate_install_request(&base_request).is_ok()` for the happy path.
- Clone base request, modify one field to an invalid value, assert the specific error variant with `matches!()`.
- Cover multiple validation paths in a single test by cloning and modifying sequentially.

#### Integration Tests (`install/service.rs` lines 429-481)

- Build a tokio runtime in the test: `tokio::runtime::Builder::new_current_thread().enable_all().build()`.
- Use `block_on(async { spawn_blocking(|| install_game(&request, &log_path)).await })` to match the real execution path.
- Use a mock Proton script that creates expected files in the prefix (line 347-348).
- Assert on result fields: `succeeded`, `profile_name`, `needs_executable_confirmation`, discovered candidates, profile paths.

#### Discovery Tests (`install/discovery.rs` lines 352-447)

- Create realistic directory structures under `tempdir()`: `drive_c/Games/...`, `drive_c/Program Files/...`.
- Write `.exe` files as test fixtures.
- Assert candidate ordering and presence/absence.

### Patterns for update-game Tests

1. **Validation tests**: Clone a valid request, break one field, assert specific `UpdateGameValidationError` variant. Cover: empty updater path, missing file, non-.exe, empty proton path, missing proton, non-executable proton, empty prefix, missing prefix, non-directory prefix.
2. **Command building test**: Verify `build_update_command` produces a `Command` with the correct arguments and environment. Use a mock proton script.
3. **Integration test**: Create a temp prefix with `drive_c`, write a mock proton that runs and exits 0, verify `update_game` returns `succeeded: true`.
4. **Failure test**: Use a mock proton that exits with non-zero, verify `UpdaterExitedWithFailure` error.
5. **No frontend tests**: The project has no frontend test framework configured.

## Patterns to Follow

### Rust Module (`crates/crosshook-core/src/update/`)

1. **`mod.rs`**: Follow `install/mod.rs` exactly. Three declarations (`mod models; mod service;`), selective `pub use` for public API.
2. **`models.rs`**: Define `UpdateGameRequest`, `UpdateGameResult`, `UpdateGameError`, `UpdateGameValidationError`. Use the same derive chain (`Debug, Clone, PartialEq, Eq, Serialize, Deserialize`). Add `Default` to Request and Result. Use `#[serde(default)]` on all request fields. Use `#[serde(rename_all = "snake_case")]` on error enums. Implement `message()`, `Display`, `Error`, `From<ValidationError>`.
3. **`service.rs`**: Define `validate_update_request` and `update_game` as the public API. Reuse `new_direct_proton_command`, `apply_host_environment`, `apply_runtime_proton_environment`, `apply_working_directory`, `attach_log_stdio` from `launch::runtime_helpers`. Use `Handle::try_current().block_on(child.wait())` for process management.

### Tauri Commands (`src-tauri/src/commands/update.rs`)

1. Follow the **launch command** pattern for log streaming -- use `AppHandle` parameter, `spawn_log_stream`, emit `"update-log"` events.
2. `validate_update_request`: `spawn_blocking` + `map_err(|e| e.to_string())`.
3. `update_game`: Create log path, spawn the update process, call `spawn_log_stream`, return `UpdateGameResult` immediately.
4. Add to `generate_handler!` in `src-tauri/src/lib.rs`.

### TypeScript Types (`src/types/update.ts`)

1. Define `UpdateGameRequest`, `UpdateGameResult` interfaces matching Rust structs (snake_case fields).
2. Define `UpdateGameValidationError` string literal union matching Rust enum variants (PascalCase).
3. Define `UPDATE_GAME_VALIDATION_MESSAGES` and `UPDATE_GAME_VALIDATION_FIELD` maps.
4. Define `UpdateGameStage` type: `'idle' | 'preparing' | 'running_updater' | 'complete' | 'failed'`.
5. Define `UpdateGameValidationState` interface.

### React Hook (`src/hooks/useUpdateGame.ts`)

1. Follow `useInstallGame` pattern with `useState` for each state slice.
2. Simpler than install -- no prefix resolution, no candidate discovery, no review profile.
3. Need: request state, validation state, stage, result, error, profile list loading.
4. Derive `statusText`, `hintText`, `actionLabel` from stage.
5. Expose `startUpdate`, `reset`, field setters, validation accessors.
6. Use `mapValidationErrorToField` with the update-specific maps.

### Component (`src/components/UpdateGamePanel.tsx`)

1. Reuse `InstallField` and `ProtonPathField` (extract to shared if not already done).
2. Use `ThemedSelect` for profile selection (filtered to `proton_run` profiles).
3. Structure: intro section -> profile selector -> update exe field -> runtime section (read-only display of prefix/proton from profile, editable Proton override) -> status card -> action buttons.
4. Use `crosshook-update-shell`, `crosshook-update-section`, `crosshook-update-card` CSS classes (or reuse `crosshook-install-*` classes).

### Log Streaming Integration

1. **New event name**: `"update-log"` to avoid cross-contamination with `"launch-log"`.
2. **ConsoleView**: Must subscribe to both `"launch-log"` and `"update-log"` events. Add a second `listen` call in the `useEffect` (or generalize to accept an array of event names).
3. **ConsoleDrawer**: Similarly needs to count lines from both event channels for the badge.
4. **Backend**: Clone `spawn_log_stream` from `commands/launch.rs` into `commands/update.rs`, changing the event name to `"update-log"`.

### Registration Checklist

1. `crates/crosshook-core/src/lib.rs`: Add `pub mod update;`
2. `src-tauri/src/commands/mod.rs`: Add `pub mod update;`
3. `src-tauri/src/lib.rs`: Add `commands::update::update_game` and `commands::update::validate_update_request` to `generate_handler![]`
4. `src/types/index.ts`: Add `export * from './update';`
5. `src/components/pages/InstallPage.tsx`: Import and render `UpdateGamePanel` below `InstallGamePanel`
6. No new sidebar route needed -- update lives on the existing Install page.

## Edge Cases

- **ConsoleDrawer currently only listens to `launch-log`**: Adding `update-log` requires modifying both `ConsoleView.tsx` (line 48) and `ConsoleDrawer.tsx` (line 54) to listen to the new event. Failure to do this means update logs will not appear in the console.
- **`create_log_path` is duplicated** between `commands/install.rs` and `commands/launch.rs`: The update module will need its own copy or a shared utility. Currently there is no shared helper.
- **`Handle::try_current()` requires a Tokio runtime**: The install service uses `Handle::try_current().map_err(|_| InstallGameError::RuntimeUnavailable)` (service.rs line 53). Update must do the same. This works because Tauri commands run inside the async runtime.
- **`is_windows_executable` and `is_executable_file` are private** in `install/service.rs`: The update module needs the same checks. Options: (a) duplicate them in `update/service.rs`, (b) extract to a shared utility module, or (c) make them `pub(crate)` in the install module. Option (a) is simplest for the initial implementation.
- **Profile filtering**: The update profile selector must filter to `proton_run` profiles only. The `profile_list` command returns only names; you need `profile_load` to get each profile's launch method. The hook should load profiles and filter client-side, or add a filtered list command.
- **`InstallField` and `ProtonPathField` are not shared**: They are defined inline in `InstallGamePanel.tsx`. Extracting them is listed as a prerequisite in the feature spec. If not extracted first, duplicating them in `UpdateGamePanel.tsx` is acceptable for the MVP.
- **Prefix path from `compatdata` roots**: `resolve_wine_prefix_path` handles the `pfx/` subdirectory transparently. Update should use the same function to ensure consistency.

## Other Docs

- `/home/yandy/Projects/github.com/yandy-r/crosshook/docs/plans/update-game/feature-spec.md`: Full feature specification with data models, API design, UX workflows, and task breakdown
- `/home/yandy/Projects/github.com/yandy-r/crosshook/docs/plans/update-game/research-technical.md`: Architecture design, system constraints, integration analysis
- `/home/yandy/Projects/github.com/yandy-r/crosshook/docs/plans/update-game/research-ux.md`: User workflows, competitive analysis, gamepad navigation
- `/home/yandy/Projects/github.com/yandy-r/crosshook/CLAUDE.md`: Project conventions, build commands, architecture overview
