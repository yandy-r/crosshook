# Context Analysis: update-game

## Executive Summary

The update-game feature runs a Windows update/patch `.exe` against an existing Proton prefix, reusing CrossHook's established `launch/runtime_helpers.rs` command-building primitives. It adds a new `update` module in `crosshook-core` (sibling to `install`), a new `commands/update.rs` Tauri command file with real-time log streaming via a dedicated `update-log` event, and a new `UpdateGamePanel` component co-located on the existing Install page. The feature is significantly simpler than install-game: no prefix provisioning, no executable discovery, no profile generation. The core value is eliminating the manual shell command workflow for running update executables inside the correct WINEPREFIX.

## Architecture Context

- **System Structure**: Tauri v2 app with a Rust workspace backend (`crosshook-core` library + `src-tauri` app shell) and React 18 + TypeScript frontend. All backend operations are Tauri IPC commands invoked via `invoke()`. The new `update` module slots in as a sibling to `install` in `crosshook-core`, following the three-file module layout (`mod.rs`, `models.rs`, `service.rs`).
- **Data Flow**: Frontend loads profile via existing `profile_load` command -> auto-fills prefix/Proton paths -> user selects update `.exe` -> frontend calls `validate_update_request` -> frontend calls `update_game` -> backend spawns Proton process and starts `spawn_log_stream` -> returns `UpdateGameResult` immediately -> background task polls log file and emits `update-log` events -> `ConsoleDrawer`/`ConsoleView` display live output.
- **Integration Points**: 7 new files to create, 5 existing files to modify (one-liner additions), 6+ files reused as-is. No new dependencies (Rust or npm). No new routes, no sidebar changes, no capability changes.

## Critical Files Reference

### Files to Create

- `crates/crosshook-core/src/update/mod.rs`: Module root with selective `pub use` re-exports (follow `install/mod.rs`)
- `crates/crosshook-core/src/update/models.rs`: `UpdateGameRequest`, `UpdateGameResult`, `UpdateGameError`, `UpdateGameValidationError` with serde + Display + Error + From impls
- `crates/crosshook-core/src/update/service.rs`: `validate_update_request`, `update_game`, `build_update_command` -- the core business logic
- `src-tauri/src/commands/update.rs`: Thin Tauri IPC wrappers with `spawn_log_stream` for streaming and `create_log_path` for log file creation
- `src/types/update.ts`: TypeScript interfaces mirroring Rust types, validation error union, message/field maps, stage type
- `src/hooks/useUpdateGame.ts`: `useState`-based state machine hook (simplified version of `useInstallGame`)
- `src/components/UpdateGamePanel.tsx`: React UI component with profile selector, update exe browser, status card, action buttons

### Files to Modify (One-Liner Changes)

- `crates/crosshook-core/src/lib.rs`: Add `pub mod update;` (currently 8 modules, alphabetically between `steam` and end)
- `src-tauri/src/commands/mod.rs`: Add `pub mod update;` (currently 7 modules)
- `src-tauri/src/lib.rs`: Add `commands::update::update_game` and `commands::update::validate_update_request` to `generate_handler![]` macro (lines 69-104, currently 33 commands)
- `src/types/index.ts`: Add `export * from './update';` (currently 6 re-exports)
- `src/components/pages/InstallPage.tsx`: Import and render `UpdateGamePanel` below `InstallGamePanel`

### Files to Modify (Subscription Changes)

- `src/components/ConsoleView.tsx`: Add second `listen('update-log')` subscription alongside existing `listen('launch-log')` (line 48)
- `src/components/layout/ConsoleDrawer.tsx`: Add second `listen('update-log')` subscription alongside existing `listen('launch-log')` (line 54)

### Primary Template Files (Reused As-Is)

- `crates/crosshook-core/src/install/service.rs`: `build_install_command` (line 102-119) is the direct template for `build_update_command`
- `crates/crosshook-core/src/install/models.rs`: Type definition pattern with `message()`, `Display`, `Error`, `From<>` impls
- `crates/crosshook-core/src/launch/runtime_helpers.rs`: All 6 command-building primitives reused directly: `new_direct_proton_command`, `apply_host_environment`, `apply_runtime_proton_environment`, `apply_working_directory`, `attach_log_stdio`, `resolve_wine_prefix_path`
- `src-tauri/src/commands/launch.rs`: `spawn_log_stream` (line 103-113) and `stream_log_lines` (line 115-166) for real-time log streaming -- must be cloned or extracted for `update-log` event
- `src/hooks/useInstallGame.ts`: Hook state machine pattern (update version is much simpler -- ~200 lines vs 580)
- `src/types/install.ts`: TypeScript type convention: mirrored interfaces, validation error unions, message/field maps

## Patterns to Follow

- **Three-File Module Layout**: `mod.rs` (re-exports), `models.rs` (types with serde + Display + Error), `service.rs` (business logic). See `crates/crosshook-core/src/install/mod.rs`.
- **Thin Tauri Command**: Commands in `src-tauri/src/commands/` are thin async wrappers delegating to `crosshook-core`. Import aliasing (`update_game as update_game_core`) avoids collisions. Errors convert to `String` via `map_err(|e| e.to_string())`.
- **Streaming Execution (Launch Pattern)**: `spawn_log_stream` takes `AppHandle`, log path, and child process. Background tokio task polls log file every 500ms and emits lines via `app.emit("update-log", line)`. Command returns immediately. This is the mandated pattern (feature-spec Decision #2), NOT the blocking install pattern.
- **Hook State Machine**: `useState`-based with typed stage union (`'idle' | 'preparing' | 'running_updater' | 'complete' | 'failed'`). Derived text functions (`deriveStatusText`, `deriveHintText`) compute display strings. `mapValidationErrorToField` maps errors to fields.
- **Validation Error Maps**: Three parallel TypeScript maps -- `UPDATE_GAME_VALIDATION_MESSAGES` (variant to user text), `UPDATE_GAME_VALIDATION_FIELD` (variant to request field), and a stage type union. Rust uses `#[serde(rename_all = "snake_case")]` on error enums with `message()` methods.
- **BEM-Like CSS**: All classes use `crosshook-update-*` prefix. Shell: `crosshook-update-shell`. Sections: `crosshook-update-section`. Cards: `crosshook-update-card`. Follow the class hierarchy in `theme.css` (line 728+).
- **Profile Filtering**: `profile_list` returns names only. Frontend must call `profile_load` for each and filter to `launch.method === 'proton_run'`. Only `proton_run` profiles are eligible (feature-spec Decision #5).
- **Serde Field Defaults**: All `UpdateGameRequest` fields get `#[serde(default)]`. Error enums get `#[serde(rename_all = "snake_case")]`. Request and Result derive `Default`.

## Cross-Cutting Concerns

- **Log Streaming Event Isolation**: The `update-log` event is distinct from `launch-log` to prevent cross-contamination. Both `ConsoleView.tsx` and `ConsoleDrawer.tsx` must subscribe to both events. This is a modification that touches components shared by multiple features.
- **`create_log_path` Duplication**: This private helper is duplicated in `commands/install.rs` and `commands/launch.rs`. The update command needs a third copy or (preferably) extraction to a shared utility. Similarly, `spawn_log_stream` / `stream_log_lines` are private to `commands/launch.rs` and need extraction or duplication with the event name changed.
- **`is_windows_executable` and `is_executable_file` Accessibility**: These validation helpers are private in `install/service.rs`. Options: duplicate in `update/service.rs` (simplest for MVP), extract to shared utility, or make `pub(crate)`. Same applies to slug generation functions.
- **Exit Code Reporting with Streaming**: The current `stream_log_lines` discards the child exit status. For update-game, the exit code is needed for success/failure reporting. The streaming task should emit a final completion event with exit status, or the hook should track completion via a separate mechanism.
- **Profile Context Independence**: `UpdateGamePanel` must NOT use the global `ProfileContext` for its profile selection. The update panel needs its own independent profile loader since the selected-for-update profile is independent of the "active" profile used for launch. Use direct `invoke('profile_list')` and `invoke('profile_load')` calls within the hook.
- **Gamepad Navigation**: The update panel must be within a `data-crosshook-focus-zone` container. All interactive elements need minimum 48px touch targets (matching `--crosshook-touch-target-min`). The existing `useGamepadNav` hook handles focus automatically if CSS conventions are followed.
- **No Frontend Tests**: There is no frontend test framework. Only Rust tests exist (`cargo test -p crosshook-core`). Test coverage for the update module should include validation (all 10 error variants), command building, and integration (mock Proton script with exit 0 and exit non-zero).

## Parallelization Opportunities

### Independent Work Streams

- **Rust models (`update/models.rs`)**: Can be written immediately with zero dependencies on other new code
- **Rust service (`update/service.rs`)**: Depends only on models and existing `runtime_helpers` -- no dependency on Tauri commands
- **TypeScript types (`types/update.ts`)**: Can be written in parallel with Rust work -- just mirrors the model definitions
- **Shared utility extraction**: `create_log_path` and `spawn_log_stream` extraction from `commands/launch.rs` to a shared location can happen independently
- **ConsoleView/ConsoleDrawer event subscription**: Adding `update-log` listener can be done independently of all other work

### Coordination Required

- **Tauri commands (`commands/update.rs`)**: Depends on Rust models + service being complete
- **React hook (`useUpdateGame.ts`)**: Depends on TypeScript types and Tauri commands being registered
- **React component (`UpdateGamePanel.tsx`)**: Depends on hook being complete
- **InstallPage integration**: Depends on component being complete
- **Shared component extraction** (`InstallField`, `ProtonPathField` from `InstallGamePanel.tsx`): If done, must be coordinated with both InstallGamePanel (refactor) and UpdateGamePanel (new consumer). Can be deferred -- duplication is acceptable for MVP.

### Suggested Parallel Tracks

1. **Track A (Backend)**: models.rs -> service.rs -> service tests -> commands/update.rs -> lib.rs registration
2. **Track B (Frontend Types + Infra)**: types/update.ts + ConsoleView/ConsoleDrawer event subscription + shared utility extraction
3. **Track C (Frontend UI)**: Starts after Track A and B merge -- useUpdateGame.ts -> UpdateGamePanel.tsx -> InstallPage.tsx integration

## Implementation Constraints

### Technical Constraints

- **`proton_run` profiles only**: Feature-spec Decision #5 excludes both `native` and `steam_applaunch` profiles. Steam games update through Steam itself.
- **Real-time streaming from Phase 1**: Feature-spec Decision #2 mandates the `spawn_log_stream` pattern (launch module), NOT the `spawn_blocking` + `block_on` pattern (install module).
- **Dedicated `update-log` event**: Feature-spec Decision #3 requires a new event name to avoid mixing with `launch-log`.
- **Install page co-location**: Feature-spec Decision #1 places UpdateGamePanel on the existing Install page. No new sidebar route, no new `AppRoute` variant.
- **Working directory = updater's parent directory**: Feature-spec Decision #4 uses the updater exe's parent dir, not the game's install dir.
- **Profile is read-only during update**: The update process must NOT modify the saved profile TOML. No `profile_save` call is permitted.
- **No new dependencies**: All Rust crates and npm packages are already available.

### Business Constraints

- **Single update at a time**: Only one update operation can run concurrently (disable button while running).
- **Prefix must exist**: Validation rejects missing prefix directories -- updates do not create prefixes.
- **Update executable must be `.exe`**: Same `is_windows_executable` check as install flow.
- **Confirmation before execution**: Modal dialog with task-specific language, default focus on "Cancel".

## Key Recommendations

### For Task Breakdown

- **Split backend into 3 subtasks**: (1) models with Display/Error impls + unit tests, (2) service with validation + command building + integration tests, (3) Tauri commands with log streaming + registration. Each is independently testable.
- **Split frontend into 4 subtasks**: (1) TypeScript types, (2) hook state machine, (3) UI component, (4) InstallPage integration + ConsoleDrawer subscription changes. Types can start immediately; hook requires Tauri commands; component requires hook.
- **Defer shared component extraction**: `InstallField` and `ProtonPathField` extraction from `InstallGamePanel.tsx` is nice-to-have. For MVP, duplicating the patterns in `UpdateGamePanel.tsx` is faster and avoids a risky refactor of the existing Install panel.
- **Extract `create_log_path` and `spawn_log_stream`**: These should be shared utilities, not copy-pasted a third time. Create `src-tauri/src/log_utils.rs` or similar with parameterized event name.

### Phase Organization

- **Phase 1 (MVP)**: Full backend + frontend for core update execution with streaming. All 7 new files + 7 modifications. This is the minimum viable feature.
- **Phase 2 (Safety)**: Pre-update prefix backup, update history log, elapsed time display.
- **Phase 3 (Polish)**: Batch updates, community tap integration, working directory override.

### Dependency Management

- Backend work has zero frontend dependencies -- start immediately.
- TypeScript types have zero backend dependencies -- start immediately.
- ConsoleView/ConsoleDrawer changes have zero dependencies on new code -- start immediately.
- The hook depends on Tauri commands being registered (you can stub with `invoke` calls that will fail until backend is ready).
- The component depends on the hook being functional.
- InstallPage integration is the final step.

## Resolved Decisions (Authoritative)

The feature-spec contains 5 resolved decisions that **supersede** conflicting recommendations in the research docs:

1. **UI Placement** -> Install page section (NOT dedicated page, despite research-recommendations suggesting Option B)
2. **Log Streaming** -> Real-time via `spawn_log_stream` from Phase 1 (NOT blocking `spawn_blocking`, despite research-technical Decision 4)
3. **Event Channel** -> New `update-log` event (NOT reusing `launch-log`)
4. **Working Directory** -> Updater's parent directory (NOT game install directory)
5. **Profile Scope** -> `proton_run` only (NOT `steam_applaunch`, despite research-business initially considering both)

## Data Models (Quick Reference)

### UpdateGameRequest

Fields: `profile_name`, `updater_path`, `proton_path`, `prefix_path`, `steam_client_install_path` (all `String`, all `#[serde(default)]`)

### UpdateGameResult

Fields: `succeeded` (bool), `message` (String), `helper_log_path` (String)

### UpdateGameValidationError (10 variants)

`UpdaterPathRequired`, `UpdaterPathMissing`, `UpdaterPathNotFile`, `UpdaterPathNotWindowsExecutable`, `ProtonPathRequired`, `ProtonPathMissing`, `ProtonPathNotExecutable`, `PrefixPathRequired`, `PrefixPathMissing`, `PrefixPathNotDirectory`

### UpdateGameError (6 variants)

`Validation(UpdateGameValidationError)`, `RuntimeUnavailable`, `LogAttachmentFailed`, `UpdaterSpawnFailed`, `UpdaterWaitFailed`, `UpdaterExitedWithFailure`

### UpdateGameStage (frontend)

`'idle' | 'preparing' | 'running_updater' | 'complete' | 'failed'`
