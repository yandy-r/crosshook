# Update Game Implementation Plan

The update-game feature adds a new `update` module in `crosshook-core` (sibling to `install`) that runs a Windows update executable against an existing Proton prefix using the same `runtime_helpers` command-building primitives. A new Tauri command (`commands/update.rs`) uses the launch module's real-time log streaming pattern with a dedicated `update-log` event, while a new `UpdateGamePanel` React component integrates into the existing Install page with a profile selector filtered to `proton_run` profiles only. The implementation creates 7 new files and modifies ~10 existing files with no new dependencies — 90%+ of the infrastructure is reused from the install and launch modules.

## Critically Relevant Files and Documentation

- docs/plans/update-game/feature-spec.md: Authoritative specification with 5 resolved decisions, data models, API contracts, and success criteria
- docs/plans/update-game/analysis-context.md: Condensed architecture context with parallelization opportunities and cross-cutting concerns
- docs/plans/update-game/analysis-code.md: Concrete code patterns with line references for every template file
- docs/plans/update-game/analysis-tasks.md: Task structure analysis with dependency graph and file-to-task mapping
- src/crosshook-native/crates/crosshook-core/src/install/models.rs: Type pattern template — Request/Result/Error/ValidationError with serde, Display, Error, From impls
- src/crosshook-native/crates/crosshook-core/src/install/service.rs: Service pattern template — validation, command building, process execution, unit/integration tests
- src/crosshook-native/crates/crosshook-core/src/install/mod.rs: Three-file module layout template with selective pub use re-exports
- src/crosshook-native/crates/crosshook-core/src/launch/runtime_helpers.rs: Proton command-building primitives reused as-is (new_direct_proton_command, apply_host_environment, apply_runtime_proton_environment, apply_working_directory, attach_log_stdio)
- src/crosshook-native/src-tauri/src/commands/launch.rs: Streaming command pattern — spawn_log_stream (line 103), stream_log_lines (line 115), create_log_path (line 168)
- src/crosshook-native/src-tauri/src/commands/install.rs: Blocking command pattern (reference only) and duplicate create_log_path (line 40)
- src/crosshook-native/src-tauri/src/lib.rs: Command registration in generate_handler![] macro (lines 69-104)
- src/crosshook-native/src/types/install.ts: TypeScript type pattern — mirrored interfaces, validation error unions, message/field maps, stage type
- src/crosshook-native/src/hooks/useInstallGame.ts: Hook state machine pattern — useState-based, validation-to-field mapping, derived text functions
- src/crosshook-native/src/components/InstallGamePanel.tsx: UI component pattern — InstallField (line 64), ProtonPathField (line 113), section structure, status card
- src/crosshook-native/src/components/pages/InstallPage.tsx: Page orchestrator where UpdateGamePanel will be rendered
- src/crosshook-native/src/components/ConsoleView.tsx: Log display with launch-log subscription (line 48) — needs update-log addition
- src/crosshook-native/src/components/layout/ConsoleDrawer.tsx: Console wrapper with launch-log subscription (line 54) — needs update-log addition
- CLAUDE.md: Project conventions, build commands, commit message rules

## Implementation Plan

### Phase 1: Backend Core Module

#### Task 1.1: Create update data models Depends on [none]

**READ THESE BEFORE TASK**

- src/crosshook-native/crates/crosshook-core/src/install/models.rs
- docs/plans/update-game/feature-spec.md (Data Models section)
- docs/plans/update-game/analysis-code.md (Pattern: Serde-Driven Type Hierarchy)

**Instructions**

Files to Create

- src/crosshook-native/crates/crosshook-core/src/update/models.rs

Define `UpdateGameRequest`, `UpdateGameResult`, `UpdateGameError`, and `UpdateGameValidationError` following the `install/models.rs` pattern exactly:

- `UpdateGameRequest`: fields `profile_name`, `updater_path`, `proton_path`, `prefix_path`, `steam_client_install_path` (all `String`, all `#[serde(default)]`). Derive `Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default`.
- `UpdateGameResult`: fields `succeeded` (bool), `message` (String), `helper_log_path` (String). Same derives + Default.
- `UpdateGameValidationError`: 10 variants (`UpdaterPathRequired`, `UpdaterPathMissing`, `UpdaterPathNotFile`, `UpdaterPathNotWindowsExecutable`, `ProtonPathRequired`, `ProtonPathMissing`, `ProtonPathNotExecutable`, `PrefixPathRequired`, `PrefixPathMissing`, `PrefixPathNotDirectory`). Use `#[serde(rename_all = "snake_case")]`. Implement `message(&self) -> String`, `Display` (delegates to `message()`), and `std::error::Error`.
- `UpdateGameError`: 6 variants (`Validation(UpdateGameValidationError)`, `RuntimeUnavailable`, `LogAttachmentFailed { path: PathBuf, message: String }`, `UpdaterSpawnFailed { message: String }`, `UpdaterWaitFailed { message: String }`, `UpdaterExitedWithFailure { status: Option<i32> }`). Same trait impls. Add `impl From<UpdateGameValidationError> for UpdateGameError`.

#### Task 1.2: Create update service with validation and command building Depends on [1.1]

**READ THESE BEFORE TASK**

- src/crosshook-native/crates/crosshook-core/src/install/service.rs
- src/crosshook-native/crates/crosshook-core/src/launch/runtime_helpers.rs
- docs/plans/update-game/analysis-code.md (Pattern: Service Function with Proton Command Building)

**Instructions**

Files to Create

- src/crosshook-native/crates/crosshook-core/src/update/service.rs

Implement three public functions and private validation helpers:

1. `validate_update_request(request: &UpdateGameRequest) -> Result<(), UpdateGameValidationError>`: Validate updater path (required, exists, is file, ends in `.exe`), proton path (required, exists, is executable), prefix path (required, exists, is directory). Duplicate `is_windows_executable` and `is_executable_file` from `install/service.rs` as private helpers (or make them `pub(crate)` in install — choose whichever is simpler).

2. `build_update_command(request: &UpdateGameRequest, log_path: &Path) -> Result<Command, UpdateGameError>`: Call `new_direct_proton_command(request.proton_path.trim())`, add `request.updater_path.trim()` as arg, `apply_host_environment`, `apply_runtime_proton_environment(cmd, request.prefix_path.trim(), request.steam_client_install_path.trim())`, `apply_working_directory(cmd, "", Path::new(request.updater_path.trim()))`, `attach_log_stdio(cmd, log_path)`.

3. `update_game(request: &UpdateGameRequest, log_path: &Path) -> Result<(UpdateGameResult, tokio::process::Child), UpdateGameError>`: Validate, build command, spawn child. Return both the result (with `helper_log_path`) and the `Child` handle so the Tauri layer can stream logs. Do NOT block on `child.wait()` — the streaming pattern handles process monitoring.

Include `#[cfg(test)] mod tests` with validation tests (all 10 error variants using `tempfile::tempdir()`) and a command-building test verifying the correct environment variables are set.

#### Task 1.3: Create update module root and register Depends on [1.1, 1.2]

**READ THESE BEFORE TASK**

- src/crosshook-native/crates/crosshook-core/src/install/mod.rs
- src/crosshook-native/crates/crosshook-core/src/lib.rs

**Instructions**

Files to Create

- src/crosshook-native/crates/crosshook-core/src/update/mod.rs

Files to Modify

- src/crosshook-native/crates/crosshook-core/src/lib.rs

Create `update/mod.rs` following `install/mod.rs`: declare `mod models; mod service;` and selectively `pub use` the public API (`UpdateGameRequest`, `UpdateGameResult`, `UpdateGameError`, `UpdateGameValidationError`, `validate_update_request`, `update_game`).

Add `pub mod update;` to `lib.rs` (after `pub mod steam;`).

Run `cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core` to verify all tests pass.

### Phase 2: Tauri Commands + Frontend Foundation

#### Task 2.1: Create Tauri update commands with log streaming Depends on [1.3]

**READ THESE BEFORE TASK**

- src/crosshook-native/src-tauri/src/commands/launch.rs (lines 103-181 — spawn_log_stream, stream_log_lines, create_log_path)
- src/crosshook-native/src-tauri/src/commands/install.rs (lines 40-53 — duplicate create_log_path)
- src/crosshook-native/src-tauri/src/lib.rs (lines 69-104 — generate_handler)
- docs/plans/update-game/analysis-code.md (Pattern: Thin Tauri Command, Pattern: Real-Time Log Streaming)

**Instructions**

Files to Create

- src/crosshook-native/src-tauri/src/commands/update.rs

Files to Modify

- src/crosshook-native/src-tauri/src/commands/mod.rs
- src/crosshook-native/src-tauri/src/lib.rs

Create `commands/update.rs` with two Tauri commands:

1. `validate_update_request`: Takes `UpdateGameRequest`, calls core `validate_update_request` via `spawn_blocking`, returns `Result<(), String>`. Follow `commands/install.rs` pattern with import aliasing (`validate_update_request as validate_update_request_core`).

2. `update_game`: Takes `app: AppHandle` and `request: UpdateGameRequest`. Create log path with `create_log_path("update", &slug)`. Call core `update_game` in `spawn_blocking` to get `(UpdateGameResult, Child)`. Then call `spawn_log_stream(app, log_path, child, "update-log")` and return `UpdateGameResult`.

For `create_log_path`: duplicate the function locally from `commands/launch.rs` (matching the existing codebase pattern of per-module duplication).

For `spawn_log_stream` and `stream_log_lines`: duplicate from `commands/launch.rs` but change the event name from `"launch-log"` to `"update-log"`. The key change: after `child.try_wait()` returns `Some(status)`, emit a final `"update-complete"` event with the exit code as payload so the frontend can detect completion.

Register: Add `pub mod update;` to `commands/mod.rs`. Add `commands::update::validate_update_request` and `commands::update::update_game` to `generate_handler![]` in `lib.rs`.

#### Task 2.2: Create TypeScript types for update Depends on [none]

**READ THESE BEFORE TASK**

- src/crosshook-native/src/types/install.ts
- docs/plans/update-game/feature-spec.md (Data Models section)

**Instructions**

Files to Create

- src/crosshook-native/src/types/update.ts

Files to Modify

- src/crosshook-native/src/types/index.ts

Create `types/update.ts` following `types/install.ts` pattern:

- `UpdateGameRequest` interface: `profile_name`, `updater_path`, `proton_path`, `prefix_path`, `steam_client_install_path` (all `string`)
- `UpdateGameResult` interface: `succeeded` (boolean), `message` (string), `helper_log_path` (string)
- `UpdateGameValidationError` string literal union: all 10 variants in PascalCase
- `UPDATE_GAME_VALIDATION_MESSAGES`: `Record<UpdateGameValidationError, string>` mapping each variant to a user-facing message (match Rust `message()` exactly)
- `UPDATE_GAME_VALIDATION_FIELD`: `Record<UpdateGameValidationError, keyof UpdateGameRequest | null>` mapping errors to form fields
- `UpdateGameStage`: `'idle' | 'preparing' | 'running_updater' | 'complete' | 'failed'`
- `UpdateGameValidationState` interface: `fieldErrors: Partial<Record<keyof UpdateGameRequest, string>>`, `generalError: string | null`

Add `export * from './update';` to `types/index.ts`.

#### Task 2.3: Add update-log subscription to console components Depends on [none]

**READ THESE BEFORE TASK**

- src/crosshook-native/src/components/ConsoleView.tsx (line 48 — listen call)
- src/crosshook-native/src/components/layout/ConsoleDrawer.tsx (line 54 — listen call)
- src/crosshook-native/src/utils/log.ts

**Instructions**

Files to Modify

- src/crosshook-native/src/components/ConsoleView.tsx
- src/crosshook-native/src/components/layout/ConsoleDrawer.tsx

In `ConsoleView.tsx`: Inside the existing `useEffect` that calls `listen('launch-log', ...)`, add a second `listen('update-log', ...)` call using the same handler function. Add its unlisten to the cleanup return.

In `ConsoleDrawer.tsx`: Similarly add a second `listen('update-log', ...)` call alongside the existing `listen('launch-log', ...)`. Same handler, same cleanup pattern.

Both listeners use `normalizeLogMessage` from `utils/log.ts` which already handles plain string payloads.

Note: The `update-complete` event subscription belongs in the `useUpdateGame` hook (Task 2.4), not in these console display components. Console components only need `update-log` for displaying streaming lines.

#### Task 2.4: Create useUpdateGame hook Depends on [2.1, 2.2]

**READ THESE BEFORE TASK**

- src/crosshook-native/src/hooks/useInstallGame.ts
- src/crosshook-native/src/types/update.ts (created in Task 2.2)
- docs/plans/update-game/analysis-code.md (Pattern: Hook State Machine with Typed Stages)

**Instructions**

Files to Create

- src/crosshook-native/src/hooks/useUpdateGame.ts

Create a `useState`-based state machine hook following `useInstallGame.ts` but significantly simpler (~200 lines vs 580):

**State**: `request` (UpdateGameRequest), `validation` (UpdateGameValidationState), `stage` (UpdateGameStage), `result` (UpdateGameResult | null), `error` (string | null), `profiles` (string[]), `isLoadingProfiles` (boolean).

**Key functions**:
- `loadProfiles()`: Call `invoke('profile_list')`, then `invoke('profile_load', { name })` for each to filter to `launch.method === 'proton_run'` profiles. Store filtered names in `profiles` state.
- `populateFromProfile(name: string)`: Call `invoke('profile_load', { name })`, extract `runtime.proton_path` and `runtime.prefix_path`, update request fields.
- `startUpdate()`: Set stage to `'preparing'`, call `invoke('validate_update_request', { request })`, then `invoke('update_game', { request })`. On success, set stage to `'running_updater'`. Subscribe to `'update-complete'` event to transition to `'complete'` or `'failed'` based on exit code.
- `reset()`: Reset all state to initial values.

**Derived values**: `statusText`, `hintText`, `actionLabel` computed from `stage` + `result` + `error`. Use `mapValidationErrorToField` with update-specific maps.

**Profile context**: Do NOT use the global `ProfileContext`. Load profiles independently via direct `invoke()` calls.

**Return type**: Define `UseUpdateGameResult` interface with all state, setters, derived values, and actions.

### Phase 3: UI Component and Integration

#### Task 3.1: Extract shared form field components Depends on [none]

**READ THESE BEFORE TASK**

- src/crosshook-native/src/components/InstallGamePanel.tsx (lines 64-179 — InstallField and ProtonPathField)

**Instructions**

Files to Create

- src/crosshook-native/src/components/ui/InstallField.tsx
- src/crosshook-native/src/components/ui/ProtonPathField.tsx

Files to Modify

- src/crosshook-native/src/components/InstallGamePanel.tsx

Move the `InstallField` component (lines 64-111 of InstallGamePanel.tsx) to `src/components/ui/InstallField.tsx` as a named + default export. Move the `ProtonPathField` component (lines 113-179) to `src/components/ui/ProtonPathField.tsx`. Update `InstallGamePanel.tsx` to import from the new locations.

Keep the component interfaces and props unchanged — this is a pure move refactor. Run the dev server to verify the Install page still renders correctly.

#### Task 3.2: Create UpdateGamePanel component Depends on [2.4, 3.1]

**READ THESE BEFORE TASK**

- src/crosshook-native/src/components/InstallGamePanel.tsx
- src/crosshook-native/src/hooks/useUpdateGame.ts (created in Task 2.4)
- src/crosshook-native/src/components/ui/ThemedSelect.tsx
- src/crosshook-native/src/utils/dialog.ts
- docs/plans/update-game/feature-spec.md (UX Considerations section)

**Instructions**

Files to Create

- src/crosshook-native/src/components/UpdateGamePanel.tsx

Create the `UpdateGamePanel` component following `InstallGamePanel.tsx` structure but simpler:

**Sections**:
1. **Section header**: Eyebrow "Update Game" with description text.
2. **Profile selector**: `ThemedSelect` dropdown populated from `useUpdateGame.profiles`. On selection, call `populateFromProfile(name)`. Show "Loading profiles..." while loading. Show "No proton_run profiles found" if empty.
3. **Update executable**: `InstallField` with "Browse" button using `chooseFile()` with `.exe` filter. Display file name prominently.
4. **Runtime info**: Read-only display of prefix path and Proton path (auto-filled from profile). Optionally editable Proton path with `ProtonPathField`.
5. **Status card**: Shows stage indicator, `statusText`, `hintText`, log path when available. Use `crosshook-install-card` pattern.
6. **Confirmation dialog**: Before executing, show modal: "Apply update to [profile]? This will run [exe] inside the Proton prefix. This action cannot be automatically undone." Default focus on "Cancel".
7. **Action buttons**: "Apply Update" (disabled while running or invalid), "Reset" button.

**Confirmation dialog**: Use a simple modal overlay (follow the `ProfileReviewModal` pattern in `InstallGamePanel.tsx` or create a lightweight confirmation component). The dialog must have: title "Apply update to [profile]?", body describing the action, "Cancel" button (default focus) and "Apply Update" button (accent color). On Steam Deck, B-button should trigger Cancel.

**CSS**: Use `crosshook-install-shell` and `crosshook-install-section` class patterns. Only add `crosshook-update-*` classes where update-specific styling is needed.

**Gamepad**: Wrap in `data-crosshook-focus-zone` container. All interactive elements need minimum 48px touch targets.

**Props**: Accept `protonInstalls` from parent (InstallPage already loads these).

**`steam_client_install_path`**: Auto-populate from the existing `default_steam_client_install_path` Tauri command (already used by InstallPage) or pass empty string `""` — the runtime helper's `resolve_steam_client_install_path` falls back through env vars and well-known paths.

#### Task 3.3: Integrate UpdateGamePanel into InstallPage Depends on [3.2]

**READ THESE BEFORE TASK**

- src/crosshook-native/src/components/pages/InstallPage.tsx

**Instructions**

Files to Modify

- src/crosshook-native/src/components/pages/InstallPage.tsx

Import `UpdateGamePanel` and render it below `InstallGamePanel` with a visual separator. Pass `protonInstalls` and `protonInstallsError` as props (these are already loaded in the page state).

The update section should be always visible (not collapsed), separated by a section divider or spacing. No tab switching needed — both panels are visible on the same page.

#### Task 3.4: CSS styling and end-to-end validation Depends on [3.3]

**READ THESE BEFORE TASK**

- src/crosshook-native/src/styles/theme.css (line 728 — crosshook-install-* classes)

**Instructions**

Files to Modify

- src/crosshook-native/src/styles/theme.css (if needed)

Add any update-specific CSS classes not covered by reusing `crosshook-install-*` classes. Follow the same visual treatment: `border-radius: 20px`, subtle border, backdrop blur, dark theme.

**End-to-end validation checklist**:
- [ ] Select a `proton_run` profile → prefix and Proton path auto-fill
- [ ] Browse for a `.exe` update executable
- [ ] Validation errors show inline on correct fields
- [ ] "Apply Update" shows confirmation dialog with default focus on "Cancel"
- [ ] Update runs and console shows live streaming output via `update-log` events
- [ ] Completion shows success/failure status with log path
- [ ] "Reset" clears all state
- [ ] Gamepad navigation works (D-pad through all interactive elements)
- [ ] Native and steam_applaunch profiles excluded from selector
- [ ] Profile TOML is NOT modified after update

Run `cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core` to verify backend tests.

## Advice

- **The streaming pattern diverges from install at the Tauri command layer, not the core service layer.** The core `update_game` function returns a `(Result, Child)` tuple. The Tauri command takes the `Child` and passes it to `spawn_log_stream`. Do not try to make the core service async or event-aware — keep it synchronous like install.

- **The `update-complete` event is critical for the hook state machine.** Without it, the frontend has no way to know when the update process exits. The `stream_log_lines` function in `commands/update.rs` must emit this event with the exit code after `child.try_wait()` returns `Some(status)`. The hook subscribes to this event to transition from `'running_updater'` to `'complete'` or `'failed'`.

- **Profile filtering causes N+1 `invoke` calls.** The `profile_list` command returns only names, so the hook must call `profile_load` for each to check `launch.method`. For typical users with <20 profiles this is fine. If it becomes a bottleneck, add a backend `profile_list_with_method` command later.

- **The `new_direct_proton_command` function already adds `"run"` as the first argument.** Do not add it again in `build_update_command`. Just add the updater path as the next `.arg()`.

- **`apply_runtime_proton_environment` handles `pfx/` subdirectory detection internally** via `resolve_wine_prefix_path`. You do not need to detect or handle the `pfx` subdirectory in the update service — pass the profile's `prefix_path` directly and the helper resolves it.

- **The empty string for `steam_client_install_path` is valid.** Both install and launch pass `""` when no explicit Steam client path is configured. The helper function `resolve_steam_client_install_path` falls back through environment variables and well-known paths.

- **ConsoleView and ConsoleDrawer subscribe independently.** Both must get the `update-log` listener. Missing either causes partial behavior: logs display but the drawer badge doesn't update, or vice versa.

- **Task 2.2 (TypeScript types) can start immediately in parallel with Phase 1 backend work.** The types are derived entirely from the feature spec — no backend compilation needed. This front-loads frontend development.

- **Task 2.3 (console subscription) and Task 3.1 (component extraction) also have no dependencies.** These three tasks (2.2, 2.3, 3.1) can all start during Phase 1, maximizing parallelism across the full plan.
