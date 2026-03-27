# Integration Research: Dry Run / Preview Launch Mode

Research into the Tauri IPC layer, launch request flow, validation integration, optimization integration, and frontend state management for implementing the `preview_launch` command.

## API Endpoints

### Tauri IPC Layer

### Existing Launch Commands

All launch commands live in `src-tauri/src/commands/launch.rs` and are registered in `src-tauri/src/lib.rs:70-90` via `tauri::generate_handler![]`.

| Command                              | Signature                                                                          | Async | Notes                                                                                        |
| ------------------------------------ | ---------------------------------------------------------------------------------- | ----- | -------------------------------------------------------------------------------------------- |
| `validate_launch`                    | `fn(request: LaunchRequest) -> Result<(), LaunchValidationIssue>`                  | No    | Fail-fast: returns first error as `Err`, or `Ok(())`                                         |
| `launch_game`                        | `async fn(app: AppHandle, request: LaunchRequest) -> Result<LaunchResult, String>` | Yes   | Mutates `request` (sets `launch_game_only=true`), validates, spawns process, streams logs    |
| `launch_trainer`                     | `async fn(app: AppHandle, request: LaunchRequest) -> Result<LaunchResult, String>` | Yes   | Mutates `request` (sets `launch_trainer_only=true`), validates, spawns process, streams logs |
| `build_steam_launch_options_command` | `fn(enabled_option_ids: Vec<String>) -> Result<String, String>`                    | No    | Pure function — builds `KEY=val wrappers %command%` string                                   |

**`LaunchResult` struct** (`launch.rs:18-23`):

```rust
pub struct LaunchResult {
    pub succeeded: bool,
    pub message: String,
    pub helper_log_path: String,
}
```

### Command Registration Pattern

Commands are registered as a flat list in the `invoke_handler` macro call in `src-tauri/src/lib.rs:70`:

```rust
.invoke_handler(tauri::generate_handler![
    commands::launch::launch_game,
    commands::launch::launch_trainer,
    commands::launch::validate_launch,
    commands::launch::build_steam_launch_options_command,
    // ... other commands
])
```

**To add `preview_launch`**: Add `commands::launch::preview_launch` to this list. No other registration needed — Tauri's `generate_handler!` macro handles serde and routing.

### IPC Serialization

All command arguments use serde `Deserialize` (Tauri deserializes JSON from frontend), and return types use `Serialize`. The `LaunchRequest` struct already derives both. Return types must be `Result<T, String>` or `Result<T, impl Serialize>` for Tauri IPC.

### Frontend Invocation Pattern

The frontend invokes backend commands via `invoke()` from `@tauri-apps/api/core`:

```typescript
// Validation call (useLaunchState.ts:104)
await invoke<void>('validate_launch', { request });

// Launch call (useLaunchState.ts:150)
const result = await invoke<LaunchResult>('launch_game', { request: launchRequest });
```

**Convention**: The argument name matches the Rust parameter name exactly. For `preview_launch(request: LaunchRequest)`, the frontend call will be:

```typescript
await invoke<LaunchPreview>('preview_launch', { request });
```

## Launch Request Flow

### Where `LaunchRequest` Is Constructed

The `LaunchRequest` is built in **`LaunchPage.tsx:10-43`** by the `buildLaunchRequest()` function:

```typescript
function buildLaunchRequest(
  profile: GameProfile,
  launchMethod: Exclude<LaunchMethod, ''>,
  steamClientInstallPath: string
): LaunchRequest | null;
```

**Key behaviors**:

- Returns `null` if `profile.game.executable_path` is empty (the guard that disables Launch/Preview buttons)
- Maps profile fields to the flat `LaunchRequest` structure
- Only includes `enabled_option_ids` when `launchMethod === 'proton_run'` (clears them for other methods)
- Sets both `launch_trainer_only` and `launch_game_only` to `false` — these are set later per-step by `useLaunchState.ts:79-94`

**Request passed to `LaunchPanel`** (`LaunchPage.tsx:48-52`):

```typescript
const launchRequest = buildLaunchRequest(profile, profileState.launchMethod, profileState.steamClientInstallPath);
// ...
<LaunchPanel profileId={profileId} method={profileState.launchMethod} request={launchRequest} />
```

### `LaunchRequest` Fields (Rust) — `request.rs:15-37`

```rust
pub struct LaunchRequest {
    pub method: String,
    pub game_path: String,
    pub trainer_path: String,
    pub trainer_host_path: String,
    pub trainer_loading_mode: TrainerLoadingMode,
    pub steam: SteamLaunchConfig,
    pub runtime: RuntimeLaunchConfig,
    pub optimizations: LaunchOptimizationsRequest,
    pub launch_trainer_only: bool,
    pub launch_game_only: bool,
}
```

**Preview note**: For preview, `launch_trainer_only` and `launch_game_only` should both remain `false` (the default from `buildLaunchRequest`) so preview shows the complete picture for both steps. The preview function should NOT mutate these flags.

### `resolved_method()` — `request.rs:74-83`

Auto-detection logic when `method` is empty or doesn't match known values:

1. If `steam.app_id` is non-empty → `steam_applaunch`
2. If `game_path` ends with `.exe` → `proton_run`
3. Otherwise → `native`

**Preview must call `resolved_method()`** to show the actual computed method, not just the configured string.

## Validation Integration

### Current `validate()` — `request.rs:442-454`

```rust
pub fn validate(request: &LaunchRequest) -> Result<(), ValidationError> {
    // Checks method string validity first
    // Then dispatches to method-specific validator
    match request.resolved_method() {
        METHOD_STEAM_APPLAUNCH => validate_steam_applaunch(request),
        METHOD_PROTON_RUN => validate_proton_run(request),
        METHOD_NATIVE => validate_native(request),
        other => Err(ValidationError::UnsupportedMethod(other.to_string())),
    }
}
```

**Fail-fast behavior**: Every internal validation function uses `?` to return on first error. This is the core gap — preview needs `validate_all()` that collects ALL issues.

### How Validation Feedback Flows to Frontend

1. `useLaunchState.ts:100-113`: `validateLaunchRequest()` calls `invoke("validate_launch", { request })`
2. On Tauri side: `validate_launch` returns `Result<(), LaunchValidationIssue>` — the `Err` variant is a structured `LaunchValidationIssue`
3. The frontend catches the error. If it matches `isLaunchValidationIssue()` shape, it creates validation feedback
4. Dispatch to reducer: `{ type: "failure", feedback: { kind: "validation", issue }, fallbackPhase }`
5. `LaunchPanel.tsx:35-36`: Extracts `validationFeedback` from state and renders message + help + severity badge

### `LaunchValidationIssue` — `request.rs:151-156`

```rust
pub struct LaunchValidationIssue {
    pub message: String,
    pub help: String,
    pub severity: ValidationSeverity,
}
```

**Current limitation**: `severity()` always returns `ValidationSeverity::Fatal` (`request.rs:429-431`). Preview's `validate_all()` should introduce `Warning` and `Info` severity levels for non-blocking issues (e.g., missing optional wrapper binary).

### `ValidationError` Variants — `request.rs:158-198`

There are **24 distinct error variants**, covering:

- Game path issues (3 variants)
- Trainer path issues (4 variants)
- Steam configuration issues (7 variants)
- Runtime Proton configuration issues (5 variants)
- Launch optimization issues (5 variants: unknown, duplicate, unsupported method, incompatible, missing dependency)
- Method issues (3 variants: native windows exe, native trainer, unsupported method)

Each variant has a `message()`, `help()`, and `severity()` method. The `issue()` method packages all three into a `LaunchValidationIssue`.

### Internal Validation Functions

| Function                   | File:Line            | Checks                                                                                                   | Returns                       |
| -------------------------- | -------------------- | -------------------------------------------------------------------------------------------------------- | ----------------------------- |
| `validate_steam_applaunch` | `request.rs:456-485` | game_path, trainer_paths, app_id, compatdata_path, proton_path, steam_client_path, rejects optimizations | `Result<(), ValidationError>` |
| `validate_proton_run`      | `request.rs:487-508` | game_path (must exist), trainer_paths, prefix_path, proton_path, resolve_launch_directives               | `Result<(), ValidationError>` |
| `validate_native`          | `request.rs:510-524` | rejects trainer_only, game_path (must exist), rejects .exe, rejects optimizations                        | `Result<(), ValidationError>` |

**Critical for `validate_all()`**: `validate_proton_run` calls `resolve_launch_directives()` at line 505, which does its own validation (duplicates, unknowns, conflicts, missing binaries). This means directive validation errors are mixed into the validation path. For `validate_all()`, directive validation errors should be collected alongside path validation errors.

### Helper validation functions

| Function                                 | Purpose                                                    | Used by                             |
| ---------------------------------------- | ---------------------------------------------------------- | ----------------------------------- |
| `require_game_path_if_needed`            | Checks game path is non-empty, optionally exists + is file | steam, proton, native               |
| `require_trainer_paths_if_needed`        | Checks trainer_path + trainer_host_path                    | steam, proton                       |
| `require_directory`                      | Checks value non-empty, path exists, is directory          | steam (compatdata), proton (prefix) |
| `require_executable_file`                | Checks value non-empty, path exists, is executable         | steam (proton), proton (proton)     |
| `reject_launch_optimizations_for_method` | Rejects non-empty optimizations for non-proton methods     | steam, native                       |

These are the building blocks for `validate_all()` collector functions. Each can be called independently and its error converted to a `LaunchValidationIssue` rather than short-circuiting.

## Optimization & Environment Integration

### `LAUNCH_OPTIMIZATION_DEFINITIONS` — `optimizations.rs:38-175`

A `const` array of 17 `LaunchOptimizationDefinition` structs:

```rust
struct LaunchOptimizationDefinition {
    id: &'static str,              // e.g. "disable_steam_input"
    applies_to_method: &'static str, // currently always METHOD_PROTON_RUN
    env: &'static [(&'static str, &'static str)],  // e.g. [("PROTON_NO_STEAMINPUT", "1")]
    wrappers: &'static [&'static str],              // e.g. ["mangohud"]
    conflicts_with: &'static [&'static str],        // e.g. ["use_game_performance"]
    required_binary: Option<&'static str>,           // e.g. Some("mangohud")
}
```

**This struct is private** (`optimizations.rs:29`). Preview cannot directly iterate it, but can call the existing public functions to get the resolved output.

### `resolve_launch_directives()` — `optimizations.rs:267-283`

```rust
pub fn resolve_launch_directives(request: &LaunchRequest) -> Result<LaunchDirectives, ValidationError>
```

Returns `LaunchDirectives` with `env: Vec<(String, String)>` and `wrappers: Vec<String>`.

**`LaunchDirectives` struct** (`optimizations.rs:17-21`) — **currently NOT Serialize/Deserialize**:

```rust
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct LaunchDirectives {
    pub env: Vec<(String, String)>,
    pub wrappers: Vec<String>,
}
```

**Action required**: Add `#[derive(Serialize, Deserialize)]` to `LaunchDirectives`. This is a one-line change noted in the feature spec. Without it, the preview function cannot easily serialize directive data.

### Environment Variable Sources

The preview needs to tag each env var with its source. The source categories map to these constant arrays in `env.rs`:

| Constant                       | Source Tag           | Count | Purpose                                                                     |
| ------------------------------ | -------------------- | ----- | --------------------------------------------------------------------------- |
| `WINE_ENV_VARS_TO_CLEAR`       | (cleared)            | 31    | WINE vars cleared before launch to prevent session bleed                    |
| `REQUIRED_PROTON_VARS`         | `ProtonRuntime`      | 3     | `STEAM_COMPAT_DATA_PATH`, `STEAM_COMPAT_CLIENT_INSTALL_PATH`, `WINEPREFIX`  |
| `LAUNCH_OPTIMIZATION_ENV_VARS` | `LaunchOptimization` | 14    | `PROTON_NO_STEAMINPUT`, `PROTON_ENABLE_HDR`, etc.                           |
| `PASSTHROUGH_DISPLAY_VARS`     | `Host`               | 4     | `DISPLAY`, `WAYLAND_DISPLAY`, `XDG_RUNTIME_DIR`, `DBUS_SESSION_BUS_ADDRESS` |

**Host environment** set via `apply_host_environment()` (`runtime_helpers.rs:46-60`): `HOME`, `USER`, `LOGNAME`, `SHELL`, `PATH`, plus the 4 `PASSTHROUGH_DISPLAY_VARS`.

**Proton runtime env** set via `apply_runtime_proton_environment()` (`runtime_helpers.rs:62-92`): Resolves `WINEPREFIX` (with pfx/ auto-detection), `STEAM_COMPAT_DATA_PATH`, `STEAM_COMPAT_CLIENT_INSTALL_PATH`.

**Steam Proton env** set via `apply_steam_proton_environment()` (`script_runner.rs:144-163`): Sets `STEAM_COMPAT_DATA_PATH`, `STEAM_COMPAT_CLIENT_INSTALL_PATH`, `WINEPREFIX` using steam config fields directly.

### Frontend Optimization UI ↔ Backend Mapping

Frontend (`launch-optimizations.ts:24-42`): Defines `LAUNCH_OPTIMIZATION_IDS` as a const tuple of 17 string literals. The `LaunchOptimizations` interface holds `enabled_option_ids: LaunchOptimizationId[]`.

`LaunchPage.tsx:37-39`: Only passes optimization IDs for `proton_run`:

```typescript
optimizations: {
  enabled_option_ids: launchMethod === 'proton_run' ? profile.launch.optimizations.enabled_option_ids : [],
},
```

Backend `resolve_launch_directives_for_method()` (`optimizations.rs:188-265`) iterates `LAUNCH_OPTIMIZATION_DEFINITIONS` in catalog order, checking each enabled ID against the definition's `applies_to_method`, `conflicts_with`, and `required_binary`. Returns `LaunchDirectives` with env pairs and wrapper binaries.

### `build_steam_launch_options_command()` — `optimizations.rs:288-300`

Delegates to `resolve_launch_directives_for_method()` with `METHOD_PROTON_RUN`, then formats: `KEY=val ... wrappers %command%`. This output is directly useful as the `steam_launch_options` field in `LaunchPreview`.

## Frontend State Integration

### `useLaunchState` Hook — `hooks/useLaunchState.ts`

Uses `useReducer` with a state machine pattern:

**State shape** (`useLaunchState.ts:14-18`):

```typescript
type LaunchState = {
  phase: LaunchPhase;
  feedback: LaunchFeedback | null;
  helperLogPath: string | null;
};
```

**Phases** (`launch.ts:4-10`):

```
Idle → GameLaunching → WaitingForTrainer → TrainerLaunching → SessionActive
```

**Action types** (`useLaunchState.ts:20-26`):

- `reset` → back to Idle
- `game-start` → GameLaunching
- `game-success` → WaitingForTrainer or SessionActive
- `trainer-start` → TrainerLaunching
- `trainer-success` → SessionActive
- `failure` → stays at fallback phase with feedback

**Key exports**: `phase`, `feedback`, `isBusy`, `canLaunchGame`, `canLaunchTrainer`, `launchGame()`, `launchTrainer()`, `reset()`, `statusText`, `hintText`, `actionLabel`.

### Preview State: How `usePreviewState` Should Integrate

The preview state machine is simpler — only three states: `idle`, `loading`, `ready`/`error`. It should be a **separate hook** (as the feature spec recommends) because:

1. Preview is orthogonal to launch lifecycle — you can preview while Idle, but NOT while launching
2. Preview result should persist while the user reads it (not reset when launch phase changes)
3. Preview has no multi-step progression (single IPC call, single result)

**Recommended `usePreviewState` shape**:

```typescript
type PreviewState = {
  status: 'idle' | 'loading' | 'ready' | 'error';
  preview: LaunchPreview | null;
  error: string | null;
};
```

**Integration points with `useLaunchState`**:

- Preview button should be disabled when `phase !== LaunchPhase.Idle` (BR-6 from feature spec)
- Preview should use the same `request` prop passed to `LaunchPanel`
- No overlap with launch dispatch — completely independent state

### `LaunchPanel` Component Structure — `components/LaunchPanel.tsx`

Currently renders:

1. Header (method label, title, description)
2. Status info (statusText, hintText, helperLogPath, validation feedback)
3. Actions (Launch Game / Launch Trainer + Reset buttons)
4. Indicator (runner type + request status)

**Preview button placement** (per feature spec): Add a secondary/ghost-style "Preview Launch" button in the `__actions` div, alongside the existing Launch and Reset buttons. Disabled when `!canLaunchGame` (no request) or `phase !== Idle`.

### `LaunchPage` Component — `components/pages/LaunchPage.tsx`

Owns the `buildLaunchRequest()` function and passes `request` to `LaunchPanel`. The preview can reuse this same `request` object — no need to construct a different one.

### Existing UI Infrastructure for Preview Modal

**`CollapsibleSection`** (`components/ui/CollapsibleSection.tsx`): Wraps `<details>/<summary>`, supports controlled/uncontrolled modes, `defaultOpen` prop, `onToggle` callback, `meta` slot for right-side content. Matches the preview section pattern exactly.

**`ProfileReviewModal`** (`components/ProfileReviewModal.tsx`): Full modal infrastructure with:

- Portal rendering via `createPortal`
- Focus trapping (Tab cycling, initial focus)
- Backdrop dismiss (configurable)
- Status tone (neutral/success/warning/danger)
- Confirmation sub-dialog pattern
- Footer slot for action buttons

The preview modal should follow this same pattern — either reuse `ProfileReviewModal` directly or extract the focus-trap/portal logic into a shared base.

## Gotchas & Edge Cases

### 1. `chrono` Is NOT a Workspace Dependency

The feature spec claims "`chrono` already in workspace dependencies" for timestamps. **This is incorrect** — grep confirms no `chrono` in any `Cargo.toml`. The preview function should use `std::time::SystemTime` + manual RFC 3339 formatting, or add `chrono` as a new dependency to `crosshook-core`.

### 2. `LaunchDirectives` Needs Serde Derives

`LaunchDirectives` (`optimizations.rs:17`) currently only derives `Debug, Clone, PartialEq, Eq, Default`. Adding `Serialize, Deserialize` is a prerequisite for preview.

### 3. `validate_proton_run()` Calls `resolve_launch_directives()`

At `request.rs:505`, proton validation calls directive resolution, which does its own validation (unknown IDs, duplicates, conflicts, missing binaries). For `validate_all()`, this means directive validation errors should be collected alongside path validation errors, not treated as a separate pass.

### 4. `stage_trainer_into_prefix()` Has Side Effects

`script_runner.rs:227-266` performs actual file I/O (mkdir, copy). Preview must compute the staged path via string manipulation only:

```
C:\CrossHook\StagedTrainers\{trainer_base_name}\{trainer_file_name}
```

The path computation logic is: `resolve_wine_prefix_path(prefix_path).join("drive_c").join("CrossHook/StagedTrainers").join(trainer_base_name).join(trainer_file_name)`.

### 5. `LAUNCH_OPTIMIZATION_DEFINITIONS` Is Private

The const array is not `pub`, so preview code in `preview.rs` cannot directly iterate it. However, `resolve_launch_directives()` and `resolve_launch_directives_for_method()` are public and return the resolved `LaunchDirectives` — this is sufficient for preview.

### 6. Validation Severity Is Always Fatal

`ValidationError::severity()` at `request.rs:429-431` returns `ValidationSeverity::Fatal` for all variants. For `validate_all()`, some checks should be downgraded to `Warning` (e.g., missing optional wrapper binary) or `Info`.

### 7. Steam Proton Env Uses Different Resolution Than Runtime Proton Env

`apply_steam_proton_environment()` in `script_runner.rs:144-163` hardcodes `compatdata_path + "/pfx"` as WINEPREFIX, while `apply_runtime_proton_environment()` in `runtime_helpers.rs:62-92` uses `resolve_wine_prefix_path()` which checks if the path already ends in "pfx". Preview must use the correct resolution for each method.

### 8. `buildLaunchRequest()` Clears Optimizations for Non-`proton_run`

In `LaunchPage.tsx:37-39`, optimization IDs are only passed for `proton_run`. For `steam_applaunch`, optimizations are cleared on the frontend side even though the backend `build_steam_launch_options_command()` can process them separately. Preview should reflect this frontend-side filtering.

### 9. Preview Command Can Be Synchronous

Unlike `launch_game` and `launch_trainer` which are `async fn` (they spawn processes), `preview_launch` does no I/O beyond filesystem stat calls. It can be a regular `fn` (synchronous Tauri command), which simplifies error handling — no `AppHandle` parameter needed.

## Relevant Files Summary

### Backend (Rust)

| File                                                  | Purpose                                                                                                                               |
| ----------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------- |
| `crates/crosshook-core/src/launch/mod.rs`             | Module root — add `pub mod preview;` here                                                                                             |
| `crates/crosshook-core/src/launch/request.rs`         | `LaunchRequest`, `validate()`, all validation errors — add `validate_all()`                                                           |
| `crates/crosshook-core/src/launch/optimizations.rs`   | `LaunchDirectives`, `resolve_launch_directives()`, optimization definitions — add Serde derives                                       |
| `crates/crosshook-core/src/launch/env.rs`             | `WINE_ENV_VARS_TO_CLEAR`, `REQUIRED_PROTON_VARS`, `LAUNCH_OPTIMIZATION_ENV_VARS`, `PASSTHROUGH_DISPLAY_VARS`                          |
| `crates/crosshook-core/src/launch/runtime_helpers.rs` | `resolve_wine_prefix_path()`, `apply_host_environment()`, `apply_runtime_proton_environment()`, `resolve_steam_client_install_path()` |
| `crates/crosshook-core/src/launch/script_runner.rs`   | `stage_trainer_into_prefix()`, `build_proton_game_command()`, `build_proton_trainer_command()`                                        |
| `src-tauri/src/commands/launch.rs`                    | Tauri command handlers — add `preview_launch`                                                                                         |
| `src-tauri/src/lib.rs`                                | `invoke_handler` registration — add preview command                                                                                   |

### Frontend (TypeScript/React)

| File                                       | Purpose                                                                             |
| ------------------------------------------ | ----------------------------------------------------------------------------------- |
| `src/types/launch.ts`                      | `LaunchRequest`, `LaunchValidationIssue`, `LaunchPhase` — add `LaunchPreview` types |
| `src/types/launch-optimizations.ts`        | `LAUNCH_OPTIMIZATION_IDS`, `LaunchOptimizations`                                    |
| `src/types/profile.ts`                     | `GameProfile`, `LaunchMethod`, `TrainerLoadingMode`                                 |
| `src/hooks/useLaunchState.ts`              | Launch state machine — reference pattern for `usePreviewState`                      |
| `src/components/LaunchPanel.tsx`           | Launch UI — add Preview button + modal trigger                                      |
| `src/components/pages/LaunchPage.tsx`      | `buildLaunchRequest()` — preview uses same request                                  |
| `src/components/ui/CollapsibleSection.tsx` | Reusable accordion — use in preview modal                                           |
| `src/components/ProfileReviewModal.tsx`    | Modal infrastructure — reuse portal/focus-trap pattern                              |
