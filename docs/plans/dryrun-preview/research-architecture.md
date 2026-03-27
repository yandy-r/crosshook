# Architecture Research: Dry Run / Preview Launch Mode

## System Overview

The dryrun-preview feature adds a read-only launch preview mode that reuses CrossHook's existing pure computation functions to show users exactly what a launch will do before executing it. The architecture is a vertical slice through all three layers: `crosshook-core` (new `preview.rs` module with `build_launch_preview()` aggregate function), `src-tauri` (new `preview_launch` Tauri command), and React frontend (new `usePreviewState` hook + preview modal in `LaunchPanel.tsx`). All backend computation already exists and is side-effect-free; the primary new work is a `validate_all()` function that collects all validation issues instead of short-circuiting on the first, plus the aggregation and UI layers.

## Relevant Components

### Backend: crosshook-core (Rust)

#### Launch Module (`crates/crosshook-core/src/launch/`)

- **`mod.rs`** (line 1-23): Module root with `pub mod` declarations and re-exports. Currently exports from `env`, `optimizations`, `request`, `runtime_helpers`, `script_runner`. New `pub mod preview;` and re-exports go here.
- **`request.rs`** (line 1-955): Core `LaunchRequest` struct (line 16-37), `validate()` function (line 442-454), `ValidationError` enum (line 158-199), `LaunchValidationIssue` struct (line 151-156), `ValidationSeverity` enum (line 143-149). The `validate()` function is **fail-fast** (returns on first `Err`) — preview needs a new `validate_all()` that collects all issues.
  - Key methods on `LaunchRequest`: `resolved_method()` (line 74-83), `game_executable_name()` (line 85-102), `should_copy_trainer_to_prefix()` (line 138-140)
  - Internal validators: `validate_steam_applaunch()` (line 456-485), `validate_proton_run()` (line 487-508), `validate_native()` (line 510-524)
  - Helper functions: `require_game_path_if_needed()`, `require_trainer_paths_if_needed()`, `require_directory()`, `require_executable_file()` — all return `Result<_, ValidationError>` (fail-fast)
- **`optimizations.rs`** (line 1-490): `LaunchDirectives` struct (line 17-21) with `env: Vec<(String, String)>` and `wrappers: Vec<String>`. **Currently missing `Serialize, Deserialize` derives** — needs one-line addition. `resolve_launch_directives()` (line 267-283) and `resolve_launch_directives_for_method()` (line 188-265) are pure functions. `build_steam_launch_options_command()` (line 288-300) produces the `%command%` string. `LAUNCH_OPTIMIZATION_DEFINITIONS` (line 38-175) is the static registry of all optimization IDs, env vars, wrappers, conflicts, and binary dependencies.
- **`env.rs`** (line 1-120): Constants for environment variable lists — `WINE_ENV_VARS_TO_CLEAR` (31 vars, line 8-40), `REQUIRED_PROTON_VARS` (3 vars, line 42-46), `LAUNCH_OPTIMIZATION_ENV_VARS` (14 vars, line 48-63), `PASSTHROUGH_DISPLAY_VARS` (4 vars, line 65-70). Preview uses these to tag env var sources.
- **`runtime_helpers.rs`** (line 1-191): Pure path resolution functions that preview will call directly:
  - `resolve_wine_prefix_path(prefix_path: &Path) -> PathBuf` (line 94-105): Resolves `pfx` subdirectory heuristic
  - `resolve_steam_client_install_path(configured_path: &str) -> Option<String>` (line 157-182): Falls back through env var then filesystem candidates
  - `apply_host_environment(command: &mut Command)` (line 46-60): Sets HOME, USER, LOGNAME, SHELL, PATH, DISPLAY, WAYLAND_DISPLAY, XDG_RUNTIME_DIR, DBUS_SESSION_BUS_ADDRESS — preview needs to read these values without mutating a Command
  - `apply_runtime_proton_environment()` (line 62-92): Sets WINEPREFIX, STEAM_COMPAT_DATA_PATH, STEAM_COMPAT_CLIENT_INSTALL_PATH
  - `DEFAULT_HOST_PATH` constant (line 9): `/usr/bin:/bin`
- **`script_runner.rs`** (line 1-861): Command building functions that have **side effects** (spawn processes, copy files). Preview must NOT call these directly. Key constants for trainer staging path computation:
  - `STAGED_TRAINER_ROOT = "CrossHook/StagedTrainers"` (line 22)
  - `stage_trainer_into_prefix()` (line 227-266): The side-effecting function. Preview must replicate the **path computation only** (format string at line 261-265: `C:\CrossHook\StagedTrainers\{base_name}\{file_name}`)
- **`test_support.rs`** (line 1-35): `ScopedCommandSearchPath` for test isolation of binary-availability checks. Preview tests may need this.

#### Library Root (`crates/crosshook-core/src/lib.rs`)

- Line 1-9: Module declarations. `pub mod launch;` already exposes the launch module. No changes needed here — preview types are re-exported through `launch/mod.rs`.

#### Profile Module (`crates/crosshook-core/src/profile/models.rs`)

- `TrainerLoadingMode` enum (line 48-54): `SourceDirectory` (default) | `CopyToPrefix`. Used by preview to determine whether to show staged trainer path.
- `GameProfile` struct (line 32-46): The profile data model with `game`, `trainer`, `injection`, `steam`, `runtime`, `launch` sections.

### Tauri Command Layer (`src-tauri/`)

- **`src/commands/launch.rs`** (line 1-200): Existing launch commands. Key patterns:
  - `validate_launch` (line 25-28): Sync command, returns `Result<(), LaunchValidationIssue>` — the simplest command pattern
  - `build_steam_launch_options_command` (line 31-36): Sync command, `Result<String, String>` — takes simple params
  - `launch_game` (line 38-68): Async command with `AppHandle` — spawns processes
  - `launch_trainer` (line 70-102): Async command with `AppHandle`
  - `LaunchResult` struct (line 18-23): `#[derive(Debug, Clone, Serialize)]` — return type pattern
  - New `preview_launch` command should be **sync** (no `AppHandle` needed, pure computation)
- **`src/commands/mod.rs`** (line 1-9): Module declarations for all command groups. No change needed — `launch` module already declared.
- **`src/lib.rs`** (line 1-112): Tauri app setup with `invoke_handler` registration. Launch commands at lines 87-90:

  ```
  commands::launch::launch_game,
  commands::launch::launch_trainer,
  commands::launch::validate_launch,
  commands::launch::build_steam_launch_options_command,
  ```

  New `commands::launch::preview_launch` adds after line 90.

- **`src/lib.rs`** state management (line 62-66): Stores (`ProfileStore`, `SettingsStore`, `RecentFilesStore`, `CommunityTapStore`) are managed via `tauri::manage()`. Preview command does NOT need any managed state — it's pure computation on the `LaunchRequest`.

### Frontend (React/TypeScript)

#### Types (`src/types/`)

- **`launch.ts`** (line 1-66): `LaunchRequest` interface (line 12-32), `LaunchPhase` enum (line 4-10), `LaunchValidationIssue` (line 36-40), `LaunchFeedback` (line 42-44), `LaunchResult` (line 62-66), `isLaunchValidationIssue()` type guard (line 46-60). New `LaunchPreview` and supporting types go here.
- **`launch-optimizations.ts`** (line 1-307): `LaunchOptimizations` interface, `LAUNCH_OPTIMIZATION_OPTIONS` catalog with labels and descriptions. Potentially useful for enriching preview display.
- **`index.ts`** (line 1-8): Re-exports from all type modules. New launch.ts exports auto-propagate.

#### Hooks (`src/hooks/`)

- **`useLaunchState.ts`** (line 1-304): State machine hook using `useReducer`. Pattern:
  - Typed state: `{ phase, feedback, helperLogPath }`
  - Typed action union: `LaunchAction` with discriminated `type` field
  - `reducer()` function with exhaustive switch
  - Async functions `launchGame()` and `launchTrainer()` that dispatch actions
  - Returns computed values: `canLaunchGame`, `canLaunchTrainer`, `isBusy`, `statusText`, `hintText`, etc.
  - Preview hook (`usePreviewState.ts`) should be simpler: `{ loading, preview, error }` state

#### Components (`src/components/`)

- **`LaunchPanel.tsx`** (line 1-143): Receives props from `useLaunchState` hook. Structure:
  - Header section (line 47-63): eyebrow, title, status badge
  - Info section (line 65-99): status text, hint text, helper log path, feedback
  - Actions section (line 101-118): Primary action button + Reset button — **Preview button goes here** (new secondary/ghost button)
  - Indicator section (line 120-137): Runner status dot
  - BEM classes: `crosshook-launch-panel__*`
- **`pages/LaunchPage.tsx`** (line 1-106): Page layout hosting `LaunchPanel`. Constructs `LaunchRequest` via `buildLaunchRequest()` (line 10-43) from profile state. Also hosts `LaunchOptimizationsPanel` and `SteamLaunchOptionsPanel` in `CollapsibleSection` wrappers.
- **`ui/CollapsibleSection.tsx`** (line 1-64): Reusable accordion using native `<details>/<summary>`. Props: `title`, `defaultOpen`, `open` (controlled), `onToggle`, `meta` (ReactNode), `className`, `children`. Supports both controlled and uncontrolled modes. Preview modal will use this for section expand/collapse.
- **`ProfileReviewModal.tsx`** (line 1-456): Full-featured modal dialog. Key infrastructure to reuse or reference:
  - Portal rendering via `createPortal` (line 310)
  - Focus trapping with `handleKeyDown` Tab cycling (line 235-278)
  - Inert sibling management for `aria-modal` (line 185-196)
  - Backdrop dismiss handling (line 280-295)
  - Focus restore on close (line 226-232)
  - Header with status chip, close button
  - Summary section with key-value items
  - Body slot for children
  - Footer with actions
  - Nested confirmation dialog support
  - Props include `statusTone` for color coding, `footer` ReactNode slot
  - **Note**: Preview modal needs a simpler variant — the ProfileReviewModal is profile-specific. Preview should create a new `PreviewModal` component reusing the modal infrastructure patterns (portal, focus trap, inert management) but with preview-specific content layout.

## Data Flow

### Complete Launch Flow (existing)

```
LaunchPage.tsx
  |-- buildLaunchRequest(profile, method, steamClientInstallPath) --> LaunchRequest | null
  |
  v
LaunchPanel.tsx (receives request as prop)
  |-- useLaunchState({ profileId, method, request })
  |     |-- useReducer(reducer, initialState)  --> state machine
  |     |
  |     |-- launchGame():
  |     |     1. buildLaunchRequest(request, GameLaunching)  --> sets launch_game_only=true
  |     |     2. invoke("validate_launch", { request })      --> Tauri IPC
  |     |     3. invoke("launch_game", { request })          --> Tauri IPC
  |     |
  |     |-- launchTrainer():
  |           1. buildLaunchRequest(request, TrainerLaunching) --> sets launch_trainer_only=true
  |           2. invoke("validate_launch", { request })        --> Tauri IPC
  |           3. invoke("launch_trainer", { request })         --> Tauri IPC
  |
  v
Tauri Command Layer (src-tauri/src/commands/launch.rs)
  |-- validate_launch(request) --> validate(&request).map_err(...)
  |-- launch_game(app, request):
  |     1. request.resolved_method()     --> determine launch path
  |     2. build_*_command(&request)     --> Command with env, args, working dir
  |     3. command.spawn()               --> child process
  |     4. spawn_log_stream(app, ...)    --> stream log lines via Tauri events
  |
  v
crosshook-core/src/launch/ (business logic)
  |-- validate(&request)                 --> fail-fast validation
  |-- request.resolved_method()          --> auto-detect method
  |-- resolve_launch_directives(&request) --> env + wrappers from optimizations
  |-- build_proton_game_command()        --> Command with full env setup
  |-- apply_host_environment()           --> set HOME, PATH, DISPLAY, etc.
  |-- apply_runtime_proton_environment() --> set WINEPREFIX, COMPAT_DATA_PATH
```

### Preview Flow (new)

```
LaunchPage.tsx (same buildLaunchRequest)
  |
  v
LaunchPanel.tsx
  |-- usePreviewState({ request })  [NEW]
  |     |-- useState<{ loading, preview, error }>
  |     |
  |     |-- requestPreview():
  |           1. invoke("preview_launch", { request })  --> Tauri IPC
  |           2. setState({ preview: result })
  |
  v
Tauri Command Layer
  |-- preview_launch(request) [NEW]:
        build_launch_preview(&request).map_err(|e| e.to_string())
  |
  v
crosshook-core/src/launch/preview.rs [NEW]
  |-- build_launch_preview(&request):
        1. request.resolved_method()                  --> resolved method string
        2. validate_all(&request)                     --> Vec<LaunchValidationIssue> (ALL issues)
        3. resolve_launch_directives(&request)        --> Ok(directives) | Err(error)
        4. collect_preview_environment(...)            --> Vec<PreviewEnvVar> with source tags
        5. build_effective_command_string(...)         --> human-readable command chain
        6. resolve_proton_setup(...)                   --> ProtonSetup (paths only, no mutation)
        7. build_trainer_info(...)                     --> PreviewTrainerInfo with predicted staged path
        8. Assemble LaunchPreview struct
```

## Integration Points

### 1. `validate_all()` — New Function in `request.rs`

The existing `validate()` (line 442) calls method-specific validators that return `Result<(), ValidationError>` — each uses `?` to short-circuit. `validate_all()` needs to call the same individual checks but **collect** all errors into a `Vec<LaunchValidationIssue>`.

**Approach**: Extract individual check functions that return `Option<ValidationError>` instead of `Result`. The method-specific collectors (`collect_steam_issues`, `collect_proton_issues`, `collect_native_issues`) call each check and push issues into a Vec.

**Shared helpers to extract** (currently embedded in `require_*` functions):

- Game path validation (empty, exists, is_file)
- Trainer paths validation (empty, exists, is_file)
- Directory validation (empty, exists, is_dir)
- Executable file validation (empty, exists, is_executable)
- Launch optimization validation (unknown, duplicate, conflict, dependency)

### 2. Environment Collection — New Pure Functions in `preview.rs`

The existing `runtime_helpers.rs` functions mutate a `Command` object. Preview needs the **same values** without a Command. New pure functions:

- `collect_host_environment() -> Vec<PreviewEnvVar>`: Reads HOME, USER, PATH, DISPLAY, etc. Tags each as `EnvVarSource::Host`
- `collect_proton_environment(prefix_path, steam_client_install_path) -> Vec<PreviewEnvVar>`: Computes WINEPREFIX, STEAM_COMPAT_DATA_PATH, STEAM_COMPAT_CLIENT_INSTALL_PATH. Tags as `EnvVarSource::ProtonRuntime`
- `collect_optimization_environment(directives) -> Vec<PreviewEnvVar>`: Maps directive env pairs. Tags as `EnvVarSource::LaunchOptimization`
- `collect_steam_proton_environment(request) -> Vec<PreviewEnvVar>`: For steam_applaunch method. Tags as `EnvVarSource::SteamProton`

### 3. Effective Command String

The preview needs a human-readable command string showing the full wrapper chain. For `proton_run`:

```
mangohud gamemoderun /path/to/proton run /path/to/game.exe
```

This is assembled from `directives.wrappers` + proton path + "run" + game path.

For `steam_applaunch`: The `build_steam_launch_options_command()` output is directly usable.

For `native`: Simply the game path.

### 4. Trainer Staging Path Prediction

For `copy_to_prefix` mode, preview computes the predicted staged path **without copying files**. The formula from `script_runner.rs` line 261-265:

```
C:\CrossHook\StagedTrainers\{trainer_base_name}\{trainer_file_name}
```

Where `trainer_base_name` is `file_stem()` of `trainer_host_path` and `trainer_file_name` is `file_name()`.

### 5. Tauri Command Registration

Add to `invoke_handler` in `src-tauri/src/lib.rs` line 87-90:

```rust
commands::launch::preview_launch,
```

### 6. Frontend Type Additions

New types in `src/types/launch.ts`:

- `EnvVarSource` — string union type
- `PreviewEnvVar` — key/value/source
- `ProtonSetup` — wine_prefix_path, compat_data_path, steam_client_install_path, proton_executable
- `PreviewTrainerInfo` — path, host_path, loading_mode, staged_path
- `PreviewValidation` — passed boolean + issues array
- `LaunchPreview` — the complete preview struct

## Key Dependencies

### Rust Dependencies

| Dependency | Status                | Purpose                                                                                                                              |
| ---------- | --------------------- | ------------------------------------------------------------------------------------------------------------------------------------ |
| `serde`    | Already in Cargo.toml | Serialize/Deserialize for all preview types                                                                                          |
| `chrono`   | **NOT in Cargo.toml** | `generated_at` timestamp. Alternative: use `std::time::SystemTime` + manual RFC 3339 formatting, or add `chrono` as a new dependency |
| `tokio`    | Already in Cargo.toml | Not needed for preview (sync computation)                                                                                            |
| `tracing`  | Already in Cargo.toml | Optional debug logging in preview                                                                                                    |

### Required Code Changes (Prerequisite)

1. **`optimizations.rs` line 17**: Add `Serialize, Deserialize` to `LaunchDirectives` derive — currently only `Debug, Clone, PartialEq, Eq, Default`. One-line change that unblocks serialization of directives for preview.

### Frontend Dependencies

No new npm packages needed. All UI infrastructure exists:

- `@tauri-apps/api/core` — `invoke()` for IPC
- `react` / `react-dom` — hooks, portal
- Existing CSS custom properties in `variables.css`

### Cross-Layer Type Alignment

The Rust `LaunchPreview` struct fields use `serde(rename_all = "snake_case")` convention, which produces JSON keys matching TypeScript snake_case interfaces. This is consistent with existing `LaunchRequest` / `LaunchResult` / `LaunchValidationIssue` patterns.

## Gotchas and Edge Cases

### 1. `chrono` Dependency Gap

The feature spec assumes `chrono::Utc::now().to_rfc3339()` for `generated_at`, but `chrono` is not in the workspace. Options:

- Add `chrono = "0.4"` to `crosshook-core/Cargo.toml` (recommended — small, well-maintained)
- Use `std::time::SystemTime::now()` with manual formatting (avoids new dep but more code)

### 2. `validate()` vs `validate_all()` Divergence Risk

The existing per-method validators (`validate_steam_applaunch`, `validate_proton_run`, `validate_native`) use `?` operator throughout, making them inherently fail-fast. `validate_all()` cannot simply wrap these — it needs parallel collector functions that accumulate issues. Risk: logic duplication. Mitigation: extract individual checks as small functions returning `Option<ValidationError>` that both `validate()` and `validate_all()` can call.

### 3. `resolve_launch_directives()` Side Effects

`resolve_launch_directives()` itself is pure (no I/O). However, it calls `is_command_available()` which checks `PATH` for binaries. This is acceptable for preview since it's a read-only filesystem stat, not a mutation. The function also returns `Err(ValidationError)` on failure — preview should capture this error in `directives_error` and still return partial preview data.

### 4. `apply_host_environment()` Reads Host State

`runtime_helpers.rs:apply_host_environment()` reads `env::var()` at call time. Preview's env collection must do the same, meaning preview output reflects point-in-time host state. The `generated_at` timestamp addresses this.

### 5. Steam Proton Environment Differs from Proton Run

For `steam_applaunch`, the environment is set differently in `script_runner.rs:apply_steam_proton_environment()` (line 144-163):

- `WINEPREFIX` = `{compatdata_path}/pfx` (hardcoded join)
- `STEAM_COMPAT_DATA_PATH` = `{compatdata_path}`
- `STEAM_COMPAT_CLIENT_INSTALL_PATH` = `{steam_client_install_path}`

For `proton_run`, `runtime_helpers.rs:apply_runtime_proton_environment()` (line 62-92):

- `WINEPREFIX` = resolved via `resolve_wine_prefix_path()` heuristic
- `STEAM_COMPAT_DATA_PATH` = resolved via `resolve_compat_data_path()` heuristic
- `STEAM_COMPAT_CLIENT_INSTALL_PATH` = resolved via `resolve_steam_client_install_path()` with fallbacks

Preview must handle both methods correctly with their distinct resolution logic.

### 6. Working Directory Resolution

`runtime_helpers.rs:apply_working_directory()` (line 122-137): Uses configured directory if non-empty, otherwise falls back to parent of the primary path. Preview needs to replicate this logic for the `working_directory` field.

### 7. Native Method Has No Proton/WINE Sections

For `native` method, preview must set `proton_setup = None`, `steam_launch_options = None`, and exclude WINE-related environment variables. The `cleared_variables` field should be empty for native.

### 8. `LaunchRequest` `launch_game_only` / `launch_trainer_only` Flags

The existing launch flow sets these flags before validation and launch (see `useLaunchState.ts:buildLaunchRequest()` line 79-94). Preview should show a unified view with both flags set to `false` (showing the complete picture), matching how the `LaunchRequest` arrives from `buildLaunchRequest()` in `LaunchPage.tsx`.

## Other Docs

- Feature spec: `docs/plans/dryrun-preview/feature-spec.md`
- UX research: `docs/plans/dryrun-preview/research-ux.md`
- Technical research: `docs/plans/dryrun-preview/research-technical.md`
- Business research: `docs/plans/dryrun-preview/research-business.md`
- External research: `docs/plans/dryrun-preview/research-external.md`
- Recommendations: `docs/plans/dryrun-preview/research-recommendations.md`
- Tauri v2 IPC docs: <https://v2.tauri.app/develop/calling-rust/>
