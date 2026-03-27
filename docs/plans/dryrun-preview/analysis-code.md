# Code Analysis: Dry Run / Preview Launch Mode

## Executive Summary

The dry run / preview feature integrates into a well-structured Rust backend + React frontend separated by Tauri IPC. The backend has clean pure-function boundaries in `crosshook-core` that preview can call directly — `validate()`, `resolve_launch_directives()`, `build_steam_launch_options_command()`. The primary implementation gap is that `validate()` short-circuits on the first error (returns `Result<(), ValidationError>`), requiring a new `validate_all()` collector. Environment assembly currently mutates `tokio::process::Command` objects, so preview needs pure equivalents that return `Vec<PreviewEnvVar>` instead. The frontend uses BEM + data-attribute styling, `CollapsibleSection` for accordions, and a portal-based modal pattern in `ProfileReviewModal.tsx` that preview should reuse.

---

## Existing Code Structure

### Backend Module Layout (`crosshook-core/src/launch/`)

```
launch/
├── mod.rs                  # Module declarations + pub use re-exports
├── env.rs                  # Constant arrays: WINE_ENV_VARS_TO_CLEAR (31), REQUIRED_PROTON_VARS (3), LAUNCH_OPTIMIZATION_ENV_VARS (14), PASSTHROUGH_DISPLAY_VARS (4)
├── optimizations.rs        # LaunchDirectives struct, resolve_launch_directives(), build_steam_launch_options_command(), LAUNCH_OPTIMIZATION_DEFINITIONS (private)
├── request.rs              # LaunchRequest, validate(), ValidationError (26 variants), ValidationSeverity, LaunchValidationIssue
├── runtime_helpers.rs      # Command-mutating helpers: apply_host_environment(), apply_runtime_proton_environment(), resolve_wine_prefix_path()
├── script_runner.rs        # Build commands + stage_trainer_into_prefix() (side-effecting file copy)
└── test_support/           # ScopedCommandSearchPath for test isolation
```

### Tauri Command Layer (`src-tauri/src/commands/launch.rs`)

```
commands/
├── mod.rs                  # Module declarations
├── launch.rs               # validate_launch, build_steam_launch_options_command, launch_game, launch_trainer
├── shared.rs               # create_log_path (uses SystemTime, no chrono)
└── ...
```

### Frontend Launch Stack

```
src/
├── types/launch.ts         # LaunchRequest, LaunchPhase (enum), LaunchValidationIssue, LaunchResult, LaunchFeedback
├── hooks/useLaunchState.ts # Reducer-based state machine (GameLaunching → WaitingForTrainer → etc.)
├── components/
│   ├── LaunchPanel.tsx     # Launch UI: status, feedback display, action buttons
│   ├── pages/LaunchPage.tsx # buildLaunchRequest(), profile selector, optimization panels
│   ├── ProfileReviewModal.tsx # Portal + focus trap + inert siblings + gamepad nav
│   └── ui/CollapsibleSection.tsx # Native <details>/<summary> accordion
```

---

## Implementation Patterns (with code examples)

### Pattern 1: Tauri Sync Thin Wrapper

The `validate_launch` command at `src-tauri/src/commands/launch.rs:25-28` is the exact pattern to follow for `preview_launch`:

```rust
#[tauri::command]
pub fn validate_launch(request: LaunchRequest) -> Result<(), LaunchValidationIssue> {
    validate(&request).map_err(|error| error.issue())
}
```

**For preview_launch:**

```rust
#[tauri::command]
pub fn preview_launch(request: LaunchRequest) -> Result<LaunchPreview, String> {
    build_launch_preview(&request).map_err(|error| error.to_string())
}
```

Key rules:

- Sync function (no `async`, no `AppHandle` parameter)
- Takes `LaunchRequest` directly (Tauri deserializes from JS)
- Returns `Result<T, String>` for errors that aren't structured, or `Result<T, LaunchValidationIssue>` for structured errors
- All logic lives in `crosshook-core`; the Tauri command is a one-liner

### Pattern 2: Command Registration

At `src-tauri/src/lib.rs:70-109`, commands are registered in `invoke_handler`. New command goes after line 90:

```rust
.invoke_handler(tauri::generate_handler![
    // ...existing commands...
    commands::launch::validate_launch,
    commands::launch::build_steam_launch_options_command,
    commands::launch::preview_launch,  // <-- ADD HERE
    // ...
])
```

### Pattern 3: Serde Enum Serialization

`ValidationSeverity` at `request.rs:143-149` shows the exact pattern for `EnvVarSource`:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ValidationSeverity {
    Fatal,
    Warning,
    Info,
}
```

TypeScript receives this as `"fatal" | "warning" | "info"`. `EnvVarSource` must follow:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EnvVarSource {
    HostPassthrough,
    ProtonRuntime,
    LaunchOptimization,
    WineConfiguration,
    SystemDefault,
}
```

### Pattern 4: Serde Struct Serialization (Output-Only Types)

`LaunchResult` at `commands/launch.rs:18-23` shows output-only pattern — `Serialize` only, no `Deserialize`:

```rust
#[derive(Debug, Clone, Serialize)]
pub struct LaunchResult {
    pub succeeded: bool,
    pub message: String,
    pub helper_log_path: String,
}
```

`LaunchPreview` follows this pattern (never deserialized from JS):

```rust
#[derive(Debug, Clone, Serialize)]
pub struct LaunchPreview {
    pub method: String,
    pub method_label: String,
    pub validation: PreviewValidation,
    pub environment: Vec<PreviewEnvVar>,
    // ... etc
}
```

### Pattern 5: Validation Dispatch (fail-fast)

`validate()` at `request.rs:442-454` dispatches by method and short-circuits:

```rust
pub fn validate(request: &LaunchRequest) -> Result<(), ValidationError> {
    match request.method.trim() {
        "" | METHOD_STEAM_APPLAUNCH | METHOD_PROTON_RUN | METHOD_NATIVE => {}
        value => return Err(ValidationError::UnsupportedMethod(value.to_string())),
    }

    match request.resolved_method() {
        METHOD_STEAM_APPLAUNCH => validate_steam_applaunch(request),
        METHOD_PROTON_RUN => validate_proton_run(request),
        METHOD_NATIVE => validate_native(request),
        other => Err(ValidationError::UnsupportedMethod(other.to_string())),
    }
}
```

Each method-specific validator chains calls with `?`:

```rust
fn validate_steam_applaunch(request: &LaunchRequest) -> Result<(), ValidationError> {
    require_game_path_if_needed(request, false)?;
    require_trainer_paths_if_needed(request)?;
    // ...steam-specific checks with early return via ?...
    Ok(())
}
```

**For validate_all:** Same dispatch structure, but each helper writes to `Vec<LaunchValidationIssue>` instead of returning `Result`:

```rust
pub fn validate_all(request: &LaunchRequest) -> Vec<LaunchValidationIssue> {
    let mut issues = Vec::new();

    match request.method.trim() {
        "" | METHOD_STEAM_APPLAUNCH | METHOD_PROTON_RUN | METHOD_NATIVE => {}
        value => {
            issues.push(ValidationError::UnsupportedMethod(value.to_string()).issue());
            return issues;
        }
    }

    match request.resolved_method() {
        METHOD_STEAM_APPLAUNCH => collect_steam_issues(request, &mut issues),
        METHOD_PROTON_RUN => collect_proton_issues(request, &mut issues),
        METHOD_NATIVE => collect_native_issues(request, &mut issues),
        other => issues.push(ValidationError::UnsupportedMethod(other.to_string()).issue()),
    }

    issues
}
```

### Pattern 6: LaunchDirectives and Serialization Gap

`LaunchDirectives` at `optimizations.rs:17-21` currently derives only `Debug, Clone, PartialEq, Eq, Default`:

```rust
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct LaunchDirectives {
    pub env: Vec<(String, String)>,
    pub wrappers: Vec<String>,
}
```

Preview needs to serialize this across IPC. **One-line change required**: add `Serialize, Deserialize` to the derive:

```rust
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct LaunchDirectives {
```

This also requires adding `use serde::{Deserialize, Serialize};` to `optimizations.rs` (or just `Serialize` if Deserialize is unnecessary).

### Pattern 7: Environment Assembly (Command-Mutating → Pure)

`apply_host_environment()` at `runtime_helpers.rs:46-60` mutates a `Command`:

```rust
pub fn apply_host_environment(command: &mut Command) {
    set_env(command, "HOME", env_value("HOME", ""));
    set_env(command, "USER", env_value("USER", ""));
    set_env(command, "LOGNAME", env_value("LOGNAME", ""));
    set_env(command, "SHELL", env_value("SHELL", DEFAULT_SHELL));
    set_env(command, "PATH", env_value("PATH", DEFAULT_HOST_PATH));
    set_env(command, "DISPLAY", env_value("DISPLAY", ""));
    set_env(command, "WAYLAND_DISPLAY", env_value("WAYLAND_DISPLAY", ""));
    set_env(command, "XDG_RUNTIME_DIR", env_value("XDG_RUNTIME_DIR", ""));
    set_env(command, "DBUS_SESSION_BUS_ADDRESS", env_value("DBUS_SESSION_BUS_ADDRESS", ""));
}
```

Preview must create **pure equivalents** in `preview.rs` that return tagged data:

```rust
pub fn collect_host_environment() -> Vec<PreviewEnvVar> {
    vec![
        PreviewEnvVar { key: "HOME".into(), value: env_value("HOME", ""), source: EnvVarSource::HostPassthrough },
        PreviewEnvVar { key: "USER".into(), value: env_value("USER", ""), source: EnvVarSource::HostPassthrough },
        // ...
    ]
}
```

The `env_value()` helper at `runtime_helpers.rs:184-186` is private but trivial to replicate:

```rust
fn env_value(key: &str, default: &str) -> String {
    env::var(key).unwrap_or_else(|_| default.to_string())
}
```

### Pattern 8: Wine Prefix Resolution

`resolve_wine_prefix_path()` at `runtime_helpers.rs:94-105` is public and pure (filesystem read, no mutations):

```rust
pub fn resolve_wine_prefix_path(prefix_path: &Path) -> PathBuf {
    if prefix_path.file_name().and_then(|value| value.to_str()) == Some("pfx") {
        return prefix_path.to_path_buf();
    }
    let pfx_path = prefix_path.join("pfx");
    if pfx_path.is_dir() { pfx_path } else { prefix_path.to_path_buf() }
}
```

Preview can call this directly. The distinction between Steam and Proton paths matters:

- **Steam method**: uses `compatdata_path + "/pfx"` (hardcoded in `apply_steam_proton_environment` in script_runner.rs)
- **Proton method**: uses `resolve_wine_prefix_path()` heuristic

### Pattern 9: Trainer Path Computation (Pure, No Side Effects)

`stage_trainer_into_prefix()` at `script_runner.rs:227-266` copies files — preview must only compute the path:

```rust
// The actual function does fs::copy — preview replaces this with string manipulation:
fn preview_staged_trainer_path(trainer_host_path: &str) -> Option<String> {
    let path = Path::new(trainer_host_path.trim());
    let file_name = path.file_name()?.to_string_lossy();
    let base_name = path.file_stem()?.to_string_lossy();
    Some(format!("C:\\CrossHook\\StagedTrainers\\{base_name}\\{file_name}"))
}
```

Uses the constant `STAGED_TRAINER_ROOT = "CrossHook/StagedTrainers"` from `script_runner.rs:22`.

### Pattern 10: React Hook (Simple useState)

`useProfile.ts` shows the simpler `useState` pattern (vs `useLaunchState`'s reducer). Preview hook follows this:

```typescript
export function usePreviewState() {
  const [status, setStatus] = useState<'idle' | 'loading' | 'ready' | 'error'>('idle');
  const [preview, setPreview] = useState<LaunchPreview | null>(null);
  const [error, setError] = useState<string | null>(null);

  async function requestPreview(request: LaunchRequest) {
    setStatus('loading');
    setError(null);
    try {
      const result = await invoke<LaunchPreview>('preview_launch', { request });
      setPreview(result);
      setStatus('ready');
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
      setStatus('error');
    }
  }

  function dismiss() {
    setStatus('idle');
    setPreview(null);
    setError(null);
  }

  return { status, preview, error, requestPreview, dismiss };
}
```

### Pattern 11: Modal Infrastructure

`ProfileReviewModal.tsx` provides the reusable modal pattern (portal + focus trap + inert siblings + gamepad):

Key infrastructure to reuse or parallel:

- **Portal**: `createPortal()` to a `document.createElement('div')` appended to `document.body`
- **Focus trap**: Tab cycling via `getFocusableElements()` + `handleKeyDown()` — wraps first/last
- **Inert siblings**: `body.children` get `inert=true` + `aria-hidden="true"` while modal is open
- **Scroll lock**: `body.style.overflow = 'hidden'` + class `crosshook-modal-open`
- **Gamepad nav**: `data-crosshook-focus-root="modal"` attribute
- **Escape dismiss**: `event.key === 'Escape'` calls `onClose()`

Preview modal can be a simpler variant (no confirmation sub-dialog needed, no profile-specific summary fields).

### Pattern 12: CollapsibleSection Accordion

`CollapsibleSection.tsx` wraps native `<details>/<summary>` with controlled/uncontrolled modes:

```tsx
<CollapsibleSection title="Environment Variables" defaultOpen meta={<span>{envVars.length}</span>}>
  {/* table of env vars */}
</CollapsibleSection>
```

Props: `title`, `defaultOpen` (default `true`), `open` (controlled), `onToggle`, `meta` (badge slot), `className`, `children`.

BEM classes: `crosshook-collapsible`, `crosshook-collapsible__summary`, `crosshook-collapsible__chevron`, `crosshook-collapsible__title`, `crosshook-collapsible__meta`, `crosshook-collapsible__body`.

### Pattern 13: buildLaunchRequest (Frontend Assembly)

`LaunchPage.tsx:10-43` shows how the frontend builds a `LaunchRequest` from a profile:

```typescript
function buildLaunchRequest(
  profile: GameProfile,
  launchMethod: Exclude<LaunchMethod, ''>,
  steamClientInstallPath: string
): LaunchRequest | null {
  if (!profile.game.executable_path.trim()) return null;
  return {
    method: launchMethod,
    game_path: profile.game.executable_path,
    trainer_path: profile.trainer.path,
    trainer_host_path: profile.trainer.path,
    // ...
  };
}
```

Preview uses this same function — the preview button just invokes `preview_launch` with the same request.

### Pattern 14: Module Re-export

`launch/mod.rs` declares modules and re-exports key types:

```rust
pub mod env;
pub mod optimizations;
pub mod request;
pub mod runtime_helpers;
pub mod script_runner;

pub use env::{LAUNCH_OPTIMIZATION_ENV_VARS, PASSTHROUGH_DISPLAY_VARS, ...};
pub use optimizations::{build_steam_launch_options_command, resolve_launch_directives, LaunchDirectives};
pub use request::{validate, LaunchRequest, LaunchValidationIssue, ...};
```

Preview adds:

```rust
pub mod preview;
pub use preview::{build_launch_preview, LaunchPreview};
```

### Pattern 15: Test Fixtures

Tests use `tempfile::TempDir` for filesystem fixtures and factory functions:

```rust
fn steam_request() -> (tempfile::TempDir, LaunchRequest) { ... }
fn proton_request() -> (tempfile::TempDir, LaunchRequest) { ... }
fn native_request() -> (tempfile::TempDir, LaunchRequest) { ... }
```

Preview tests should follow the same pattern. `ScopedCommandSearchPath` (in `test_support/`) isolates PATH for binary-availability checks.

---

## Integration Points

### Files to Create

| File                                          | Purpose             | Key Content                                                                                                                                                  |
| --------------------------------------------- | ------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| `crates/crosshook-core/src/launch/preview.rs` | Core preview module | `LaunchPreview`, `PreviewEnvVar`, `EnvVarSource`, `ProtonSetup`, `PreviewTrainerInfo`, `PreviewValidation`, `build_launch_preview()`, env collection helpers |
| `src/hooks/usePreviewState.ts`                | React hook          | `useState` for `{status, preview, error}`, `requestPreview()`, `dismiss()`                                                                                   |

### Files to Modify

| File                                                | Change                                                                           | Lines Affected                                        |
| --------------------------------------------------- | -------------------------------------------------------------------------------- | ----------------------------------------------------- |
| `crates/crosshook-core/src/launch/mod.rs`           | Add `pub mod preview;` + re-export                                               | After line 7, add to re-exports                       |
| `crates/crosshook-core/src/launch/request.rs`       | Add `validate_all()` + method-specific collectors                                | After `validate()` at line 454                        |
| `crates/crosshook-core/src/launch/optimizations.rs` | Add `Serialize` to `LaunchDirectives` derive                                     | Line 17: add `Serialize` to derive + add serde import |
| `src-tauri/src/commands/launch.rs`                  | Add `preview_launch` Tauri command                                               | After line 36                                         |
| `src-tauri/src/lib.rs`                              | Register `commands::launch::preview_launch`                                      | After line 90                                         |
| `src/types/launch.ts`                               | Add `LaunchPreview`, `PreviewEnvVar`, `EnvVarSource`, etc. TypeScript interfaces | After line 66                                         |
| `src/components/LaunchPanel.tsx`                    | Add "Preview Launch" ghost button in `__actions` div                             | After line 117, before Reset button                   |
| `src/components/pages/LaunchPage.tsx`               | Wire preview modal trigger + pass `buildLaunchRequest` to preview                | Wrap with preview modal state                         |

---

## Code Conventions

### Rust

- **Imports**: group std → external crates → `super::`/`crate::` with blank lines between
- **Error types**: `ValidationError` is the error enum, `.issue()` converts to `LaunchValidationIssue` for IPC
- **Constants**: `SCREAMING_SNAKE_CASE`, module-level, `pub const` for cross-module access
- **Functions**: `snake_case`, `pub fn` for API surface, plain `fn` for internal helpers
- **Structs crossing IPC**: `#[derive(Debug, Clone, Serialize)]` — no `rename_all` (snake_case matches TypeScript naturally)
- **Enums crossing IPC**: `#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]` + `#[serde(rename_all = "snake_case")]`

### TypeScript

- **Interfaces**: match Rust snake_case field names exactly (Tauri preserves them)
- **Enums**: only `LaunchPhase` uses TS `enum`; others use string literal union types
- **Hooks**: named `use{Feature}`, return object with state + actions
- **Components**: `PascalCase` names, default export at bottom, BEM classes prefixed `crosshook-`
- **invoke() calls**: `invoke<ReturnType>("command_name", { paramName: value })`

### CSS

- **BEM**: `crosshook-{component}__{element}--{modifier}`
- **Data attributes for state**: `data-phase`, `data-severity`, `data-state`, `data-kind`
- **Touch targets**: minimum `--crosshook-touch-target-min: 48px`
- **Semantic colors**: `--crosshook-color-success` (#28c76f), `--crosshook-color-warning` (#f5c542), `--crosshook-color-danger` (#ff758f)
- **Font**: mono paths use `--crosshook-font-mono`

---

## Dependencies

### Existing (no changes needed)

- `serde` (with `derive` feature) — already in `crosshook-core/Cargo.toml`
- `std::env` — for reading host environment variables
- `std::path::Path` / `PathBuf` — for prefix resolution
- `tokio` — NOT needed (preview is sync)
- `@tauri-apps/api/core` — `invoke()` for IPC

### Required Additions

- **`Serialize` derive on `LaunchDirectives`**: needs `use serde::Serialize;` in `optimizations.rs` (serde is already a dependency)
- **No new crate dependencies**: `chrono` mentioned in gotchas but `std::time::SystemTime` suffices (see `commands/shared.rs` for precedent)

---

## Gotchas

### 1. `validate()` is fail-fast — preview needs exhaustive collection

`validate()` returns `Result<(), ValidationError>` — stops at first error. `validate_all()` must call the same helper validators but collect into `Vec<LaunchValidationIssue>` instead of using `?`. Each `require_*` helper returns `Result` — preview should match-and-push instead of propagating with `?`.

### 2. `validate_proton_run()` internally calls `resolve_launch_directives()` (line 505)

Proton validation calls `resolve_launch_directives(request)?` at `request.rs:505`, which validates optimizations AND resolves them. For `validate_all()`, if this call fails, the error must be collected, not propagated. For the preview path, `resolve_launch_directives()` is called separately to get the actual `LaunchDirectives` — must handle gracefully if validation already found issues.

### 3. Steam vs Proton prefix resolution differs

- **Steam**: `script_runner.rs` `apply_steam_proton_environment()` hardcodes `compatdata_path + "/pfx"` as WINEPREFIX
- **Proton**: `runtime_helpers.rs` `apply_runtime_proton_environment()` uses `resolve_wine_prefix_path()` heuristic (checks if path already ends in "pfx", then tries `path/pfx`)
- Preview must use the correct resolution per method

### 4. `LAUNCH_OPTIMIZATION_DEFINITIONS` is private

The `const` array at `optimizations.rs:38` is module-private (`const`, no `pub`). Preview cannot iterate it. But `resolve_launch_directives()` is public and returns the resolved env/wrappers. Preview should call `resolve_launch_directives()` rather than iterating definitions.

### 5. `stage_trainer_into_prefix()` has side effects

At `script_runner.rs:227-266`, this function copies files to the prefix. Preview must only compute the Windows path string: `C:\CrossHook\StagedTrainers\{stem}\{filename}`. The `STAGED_TRAINER_ROOT` constant is also private, so preview hardcodes the path format.

### 6. `chrono` is not a dependency

`crosshook-core/Cargo.toml` does not include `chrono`. If the feature spec calls for timestamps, use `std::time::SystemTime` (precedent: `commands/shared.rs` `create_log_path()`).

### 7. All `ValidationError::severity()` returns `Fatal`

At `request.rs:429-431`, every variant returns `ValidationSeverity::Fatal`. If preview wants to distinguish warnings from errors (e.g., missing optional wrapper), a new variant or severity override is needed. Consider making this change carefully — it affects existing `validate()` behavior.

### 8. `env_value()` helper is private in `runtime_helpers.rs`

At line 184, this tiny helper reads env vars with defaults. Preview needs the same logic — either make it `pub(crate)` or duplicate the one-liner in `preview.rs`.

### 9. `resolve_steam_client_install_path()` does filesystem probing

At `runtime_helpers.rs:157-182`, this function checks multiple candidate paths. For preview, call it directly to show what Steam client path would be resolved.

### 10. `LaunchRequest` fields use `String` not `Option<String>`

Empty string = not set. All string fields default to `""` via `#[serde(default)]`. Preview checks and helpers should use `.trim().is_empty()` consistently.

---

## Task-Specific Guidance

### Task: Create `preview.rs` with core types and `build_launch_preview()`

**Dependencies**: None — can start immediately.

**Key decisions**:

- All new types go in this single file
- `build_launch_preview()` takes `&LaunchRequest` and returns `Result<LaunchPreview, String>`
- Internally calls: `validate_all()` (from request.rs), `resolve_launch_directives()` (from optimizations.rs), env collection helpers (new in this file), trainer path computation (new in this file)
- `resolve_wine_prefix_path()` is imported from `runtime_helpers`
- `env_value()` must be duplicated or made `pub(crate)` in `runtime_helpers.rs`

### Task: Add `validate_all()` to `request.rs`

**Dependencies**: None — can start immediately.

**Pattern**: Mirror `validate()`'s dispatch but use mutable `Vec<LaunchValidationIssue>`. Create `collect_steam_issues()`, `collect_proton_issues()`, `collect_native_issues()` that wrap existing `require_*` helpers:

```rust
fn collect_steam_issues(request: &LaunchRequest, issues: &mut Vec<LaunchValidationIssue>) {
    if let Err(e) = require_game_path_if_needed(request, false) { issues.push(e.issue()); }
    if let Err(e) = require_trainer_paths_if_needed(request) { issues.push(e.issue()); }
    // ...continue collecting instead of returning early...
}
```

### Task: Add `Serialize` to `LaunchDirectives`

**Dependencies**: None — one-line change at `optimizations.rs:17`.

Add `use serde::Serialize;` at top of file (serde is already imported in the file's dependency chain but not directly imported yet in optimizations.rs) and add `Serialize` to the derive macro.

### Task: Create `preview_launch` Tauri command

**Dependencies**: `preview.rs` must exist first.

**Pattern**: Copy `validate_launch` pattern — sync, thin wrapper:

```rust
#[tauri::command]
pub fn preview_launch(request: LaunchRequest) -> Result<LaunchPreview, String> {
    build_launch_preview(&request).map_err(|e| e.to_string())
}
```

Register in `lib.rs` at line 90 area.

### Task: Add TypeScript types to `launch.ts`

**Dependencies**: Rust types must be finalized first (field names must match).

Add after existing types (line 66+):

```typescript
export type EnvVarSource =
  | 'host_passthrough'
  | 'proton_runtime'
  | 'launch_optimization'
  | 'wine_configuration'
  | 'system_default';

export interface PreviewEnvVar {
  key: string;
  value: string;
  source: EnvVarSource;
}

export interface LaunchPreview {
  method: string;
  method_label: string;
  validation: PreviewValidation;
  environment: Vec<PreviewEnvVar>;
  // ... match Rust struct fields
}
```

### Task: Create `usePreviewState.ts` hook

**Dependencies**: TypeScript types must exist.

Follow `useProfile.ts` `useState` pattern, NOT `useLaunchState.ts` reducer pattern.

### Task: Add Preview button and modal to `LaunchPanel.tsx`

**Dependencies**: Hook + types must exist.

Add a ghost button in the `__actions` div:

```tsx
<button
  type="button"
  className="crosshook-button crosshook-button--ghost crosshook-launch-panel__action"
  onClick={() => requestPreview(request)}
  disabled={!request || isBusy}
>
  Preview Launch
</button>
```

Modal uses portal pattern from `ProfileReviewModal.tsx` with `CollapsibleSection` accordions for each preview section.
