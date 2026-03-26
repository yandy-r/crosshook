# Update Game

The update-game feature runs a Windows update/patch `.exe` against an existing Proton prefix, reusing CrossHook's established `launch/runtime_helpers.rs` command-building primitives. It adds a new `update` module in `crosshook-core` (sibling to `install`), a new `commands/update.rs` Tauri command file with real-time log streaming via a dedicated `update-log` event, and a new `UpdateGamePanel` component co-located on the existing Install page. The feature targets `proton_run` profiles only — Steam games update through Steam itself — and the profile selector auto-fills prefix/Proton paths from the selected profile.

## Relevant Files

- src/crosshook-native/crates/crosshook-core/src/install/service.rs: Primary template — `build_install_command`, `validate_install_request`, `install_game`, private validation helpers (`is_windows_executable`, `is_executable_file`), comprehensive tests
- src/crosshook-native/crates/crosshook-core/src/install/models.rs: Type pattern — `InstallGameRequest`/`Result`/`Error`/`ValidationError` with serde derives, `message()`, `Display`, `Error`, `From<>` impls
- src/crosshook-native/crates/crosshook-core/src/install/mod.rs: Three-file module layout with selective `pub use` re-exports
- src/crosshook-native/crates/crosshook-core/src/launch/runtime_helpers.rs: Reused as-is — `new_direct_proton_command`, `apply_host_environment`, `apply_runtime_proton_environment`, `apply_working_directory`, `attach_log_stdio`, `resolve_wine_prefix_path`
- src/crosshook-native/crates/crosshook-core/src/launch/env.rs: Wine/Proton environment variable constants (`WINE_ENV_VARS_TO_CLEAR`, `PASSTHROUGH_DISPLAY_VARS`)
- src/crosshook-native/crates/crosshook-core/src/profile/models.rs: `GameProfile` struct with `RuntimeSection` and `LaunchSection` — provides `prefix_path` and `proton_path`
- src/crosshook-native/crates/crosshook-core/src/profile/toml_store.rs: `ProfileStore::list()` and `ProfileStore::load()` for profile retrieval
- src/crosshook-native/crates/crosshook-core/src/lib.rs: Module root — add `pub mod update;` here
- src/crosshook-native/src-tauri/src/commands/install.rs: Tauri blocking command pattern — `spawn_blocking`, `create_log_path`, import aliasing (`install_game as install_game_core`)
- src/crosshook-native/src-tauri/src/commands/launch.rs: Streaming command pattern — `spawn_log_stream` (line 103-113), `stream_log_lines` (line 115-166), `app.emit("launch-log", line)`, `create_log_path`
- src/crosshook-native/src-tauri/src/commands/profile.rs: `profile_list` and `profile_load` — existing commands called from update frontend
- src/crosshook-native/src-tauri/src/commands/mod.rs: Command module registry — add `pub mod update;`
- src/crosshook-native/src-tauri/src/lib.rs: Command registration in `invoke_handler` / `generate_handler![]` macro — add update commands
- src/crosshook-native/src/types/install.ts: TypeScript pattern — interfaces mirroring Rust, validation error union, message/field maps, stage type, validation state interface
- src/crosshook-native/src/hooks/useInstallGame.ts: Hook pattern — `useState`-based state machine, request/validation/stage/result/error state, derived `statusText`/`hintText`/`actionLabel`, `mapValidationErrorToField`
- src/crosshook-native/src/components/InstallGamePanel.tsx: UI pattern — `InstallField` (line 64-111), `ProtonPathField` (line 113-179), section structure, status card, action buttons
- src/crosshook-native/src/components/pages/InstallPage.tsx: Page orchestrator — render `UpdateGamePanel` below `InstallGamePanel`
- src/crosshook-native/src/components/ConsoleView.tsx: Log display — `listen('launch-log')` at line 48; must add `update-log` subscription
- src/crosshook-native/src/components/layout/ConsoleDrawer.tsx: Console wrapper — `listen('launch-log')` at line 54; must add `update-log` subscription
- src/crosshook-native/src/components/ui/ThemedSelect.tsx: Radix-based themed dropdown for profile selector
- src/crosshook-native/src/utils/dialog.ts: `chooseFile()` and `chooseDirectory()` wrappers
- src/crosshook-native/src/utils/log.ts: `normalizeLogMessage()` for event payload extraction
- src/crosshook-native/src/types/index.ts: Type barrel — add `export * from './update';`
- src/crosshook-native/src/styles/theme.css: `.crosshook-install-*` class hierarchy (line 728-1318) — pattern for update CSS classes

## Relevant Patterns

**Three-File Module Layout**: Each domain module uses `mod.rs` (re-exports), `models.rs` (types with serde + Display + Error), `service.rs` (business logic). See [crates/crosshook-core/src/install/mod.rs](src/crosshook-native/crates/crosshook-core/src/install/mod.rs) for the template.

**Thin Tauri Command Pattern**: Commands in `src-tauri/src/commands/` are thin async wrappers delegating to `crosshook-core`. Import aliasing avoids name collisions (`install_game as install_game_core`). Errors convert to `String` via `map_err(|e| e.to_string())`. See [src-tauri/src/commands/install.rs](src/crosshook-native/src-tauri/src/commands/install.rs).

**Real-Time Log Streaming**: `spawn_log_stream` takes `AppHandle`, log path, and child process. A background tokio task polls the log file every 500ms and emits new lines via `app.emit("event-name", line)`. Returns immediately; frontend subscribes via `listen()`. See [src-tauri/src/commands/launch.rs](src/crosshook-native/src-tauri/src/commands/launch.rs) lines 103-166.

**Hook State Machine**: `useState`-based state machine with typed stage union driving conditional rendering. Validation errors map to specific request fields via `mapValidationErrorToField`. Derived text functions (`deriveStatusText`, `deriveHintText`) compute display strings from stage + state. See [src/hooks/useInstallGame.ts](src/crosshook-native/src/hooks/useInstallGame.ts).

**Validation Error Maps**: TypeScript uses three parallel maps — `VALIDATION_MESSAGES` (variant → user text), `VALIDATION_FIELD` (variant → request field), and a stage type union. Rust uses `#[serde(rename_all = "snake_case")]` on error enums with `message()` methods. See [src/types/install.ts](src/crosshook-native/src/types/install.ts).

**BEM-Like CSS**: All classes use `crosshook-{feature}-{element}` prefix. Shell container: `crosshook-install-shell`. Sections: `crosshook-install-section`. Cards: `crosshook-install-card`. See [src/styles/theme.css](src/crosshook-native/src/styles/theme.css) line 728+.

## Relevant Docs

**docs/plans/update-game/feature-spec.md**: You _must_ read this when working on any update-game task. Contains the definitive specification: resolved decisions (streaming, proton_run only, Install page co-location), data models, API contracts, UX workflows, and success criteria.

**docs/plans/update-game/research-technical.md**: You _must_ read this when working on Rust models, service functions, or Tauri commands. Contains detailed struct definitions, service pseudocode, and frontend type definitions.

**docs/plans/update-game/research-patterns.md**: You _must_ read this when implementing any component. Contains line-by-line patterns for the module layout, error handling, hook state machine, and CSS conventions.

**docs/plans/update-game/research-architecture.md**: You _must_ read this when understanding integration points. Contains data flow diagrams for install (blocking) and launch (streaming) patterns, and the exact files to create/modify.

**docs/plans/update-game/research-integration.md**: You _must_ read this when implementing Tauri commands or log streaming. Contains `spawn_log_stream` mechanics, ConsoleDrawer subscription details, and `create_log_path` duplication gotcha.

**CLAUDE.md**: You _must_ read this when making any code changes. Contains project conventions, build commands, commit message rules, and architecture overview.
