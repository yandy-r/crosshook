# Technical Specification: Dry Run / Preview Launch Mode

## Executive Summary

The dry-run preview feature exposes CrossHook's existing pure computation functions through a single new Tauri command (`preview_launch`) that returns a structured `LaunchPreview` result. All computation already exists and is side-effect-free: `validate()`, `resolve_launch_directives()`, `build_steam_launch_options_command()`, and the env-building helpers. The primary technical work is: (1) a new `validate_all()` function that collects all issues instead of short-circuiting, (2) new pure environment-collection functions that mirror the `Command`-mutating helpers, (3) serializable preview data structures, and (4) a Tauri command + TypeScript types to wire them to the frontend.

---

## Architecture Design

### Data Flow

```
React UI (LaunchPanel)
  │
  ├─ User clicks "Preview Launch"
  │
  ▼
invoke("preview_launch", { request: LaunchRequest })
  │
  ▼
Tauri Command: preview_launch(request)
  │
  ├─ request.resolved_method()        → resolved method
  ├─ validate_all(&request)            → Vec<LaunchValidationIssue>
  ├─ resolve_launch_directives()       → LaunchDirectives (env + wrappers)
  ├─ collect_preview_environment()     → Vec<PreviewEnvVar>
  ├─ build_effective_command_string()  → String
  ├─ resolve_proton_setup()            → Option<ProtonSetup>
  │
  ▼
LaunchPreview (Serialize → JSON)
  │
  ▼
React UI displays structured preview
```

### New Components

| Layer | Component                 | Location                                      | Purpose                       |
| ----- | ------------------------- | --------------------------------------------- | ----------------------------- |
| Core  | `preview` module          | `crates/crosshook-core/src/launch/preview.rs` | Pure preview computation      |
| Core  | `validate_all()`          | `crates/crosshook-core/src/launch/request.rs` | Collect all validation issues |
| Tauri | `preview_launch` command  | `src-tauri/src/commands/launch.rs`            | IPC endpoint                  |
| TS    | `LaunchPreview` interface | `src/types/launch.ts`                         | Frontend type                 |
| TS    | `usePreviewState` hook    | `src/hooks/usePreviewState.ts`                | Preview state management      |
| React | Preview display           | `src/components/LaunchPanel.tsx`              | UI rendering                  |

### Integration Points

The preview command slots into the existing launch command module alongside `launch_game`, `launch_trainer`, `validate_launch`, and `build_steam_launch_options_command`. It follows the identical pattern: thin Tauri command wrapping pure `crosshook-core` logic.

Registration in `src-tauri/src/lib.rs:70`:

```rust
.invoke_handler(tauri::generate_handler![
    // ... existing commands ...
    commands::launch::preview_launch,  // new
])
```

---

## Data Models

### Rust Structs (crosshook-core)

```rust
// crates/crosshook-core/src/launch/preview.rs

use serde::{Deserialize, Serialize};
use super::request::{LaunchRequest, LaunchValidationIssue};

/// Source category for a preview environment variable.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EnvVarSource {
    /// Core Proton runtime vars (STEAM_COMPAT_DATA_PATH, WINEPREFIX, etc.)
    ProtonRuntime,
    /// Vars from launch optimization toggles (PROTON_NO_STEAMINPUT, etc.)
    LaunchOptimization,
    /// Passthrough from host (HOME, DISPLAY, PATH, etc.)
    Host,
    /// Steam-specific Proton vars for steam_applaunch
    SteamProton,
}

/// A single environment variable that will be set during launch.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PreviewEnvVar {
    pub key: String,
    pub value: String,
    pub source: EnvVarSource,
}

/// Proton runtime setup details (non-native methods only).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProtonSetup {
    pub wine_prefix_path: String,
    pub compat_data_path: String,
    pub steam_client_install_path: String,
    pub proton_executable: String,
}

/// Trainer configuration details for the preview.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PreviewTrainerInfo {
    pub path: String,
    pub host_path: String,
    pub loading_mode: String,
    /// The Windows-side path when copy_to_prefix mode is used.
    pub staged_path: Option<String>,
}

/// Validation summary for the preview.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PreviewValidation {
    pub passed: bool,
    pub issues: Vec<LaunchValidationIssue>,
}

/// Complete dry-run preview result returned to the frontend.
///
/// Sections that depend on independent computations use `Option<T>` so
/// the preview can return partial results when one section fails (e.g.,
/// directive resolution fails but validation and game info are still useful).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LaunchPreview {
    /// The effective launch method after inference.
    pub resolved_method: String,

    /// All validation results collected (not short-circuited).
    pub validation: PreviewValidation,

    /// Environment variables that will be set.
    /// None when environment collection fails (e.g., directive resolution error).
    pub environment: Option<Vec<PreviewEnvVar>>,

    /// WINE/Proton env vars that will be cleared before launch.
    pub cleared_variables: Vec<String>,

    /// Wrapper command chain (e.g. ["mangohud", "gamemoderun"]).
    /// None when directive resolution fails.
    pub wrappers: Option<Vec<String>>,

    /// Human-readable effective command string.
    /// None when directive resolution fails.
    pub effective_command: Option<String>,

    /// Error message when directive resolution or command building fails.
    /// Allows the frontend to show what went wrong alongside partial results.
    pub directives_error: Option<String>,

    /// Steam Launch Options %command% string (steam_applaunch only).
    pub steam_launch_options: Option<String>,

    /// Proton environment setup details.
    pub proton_setup: Option<ProtonSetup>,

    /// Resolved working directory.
    pub working_directory: String,

    /// Full game executable path.
    pub game_executable: String,

    /// Just the file name portion.
    pub game_executable_name: String,

    /// Trainer details (None for game-only or native launches).
    pub trainer: Option<PreviewTrainerInfo>,

    /// ISO 8601 timestamp when the preview was generated.
    /// Filesystem state (path existence, executable permissions) and PATH
    /// lookups are point-in-time checks that can go stale between preview
    /// and actual launch. The frontend can use this to show staleness.
    pub generated_at: String,
}

impl LaunchPreview {
    /// Renders a human-readable plain-text summary for clipboard copy or CLI output.
    pub fn to_display_text(&self) -> String {
        let mut lines = Vec::new();
        lines.push(format!("CrossHook Launch Preview ({})", self.generated_at));
        lines.push(format!("Method: {}", self.resolved_method));
        lines.push(format!("Game:   {}", self.game_executable));
        if let Some(ref setup) = self.proton_setup {
            lines.push(format!("Proton: {}", setup.proton_executable));
            lines.push(format!("Prefix: {}", setup.wine_prefix_path));
        }
        lines.push(String::new());

        // Validation
        let pass_count = self.validation.issues.iter()
            .filter(|i| i.severity != "fatal").count();
        let fail_count = self.validation.issues.iter()
            .filter(|i| i.severity == "fatal").count();
        lines.push(format!("Validation: {} passed, {} failed", pass_count, fail_count));
        for issue in &self.validation.issues {
            lines.push(format!("  [{}] {}", issue.severity, issue.message));
        }
        lines.push(String::new());

        // Command
        if let Some(ref cmd) = self.effective_command {
            lines.push(format!("Command: {cmd}"));
        }
        if let Some(ref err) = self.directives_error {
            lines.push(format!("Command resolution error: {err}"));
        }
        lines.push(String::new());

        // Environment
        if let Some(ref env) = self.environment {
            lines.push(format!("Environment ({} vars):", env.len()));
            for var in env {
                lines.push(format!("  {}={}", var.key, var.value));
            }
        }

        lines.join("\n")
    }
}
```

### Existing Types That Need Modification

```rust
// crates/crosshook-core/src/launch/optimizations.rs
// Add Serialize/Deserialize to LaunchDirectives:

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct LaunchDirectives {
    pub env: Vec<(String, String)>,
    pub wrappers: Vec<String>,
}
```

### New Function: `validate_all()`

```rust
// crates/crosshook-core/src/launch/request.rs

/// Collects ALL validation issues instead of stopping at the first error.
/// Returns an empty Vec when the request is fully valid.
pub fn validate_all(request: &LaunchRequest) -> Vec<LaunchValidationIssue> {
    let mut issues = Vec::new();

    // Method validation
    match request.method.trim() {
        "" | METHOD_STEAM_APPLAUNCH | METHOD_PROTON_RUN | METHOD_NATIVE => {}
        value => {
            issues.push(
                ValidationError::UnsupportedMethod(value.to_string()).issue(),
            );
            return issues; // Cannot proceed with unknown method
        }
    }

    match request.resolved_method() {
        METHOD_STEAM_APPLAUNCH => collect_steam_issues(request, &mut issues),
        METHOD_PROTON_RUN => collect_proton_issues(request, &mut issues),
        METHOD_NATIVE => collect_native_issues(request, &mut issues),
        other => {
            issues.push(
                ValidationError::UnsupportedMethod(other.to_string()).issue(),
            );
        }
    }

    issues
}
```

### TypeScript Interfaces

```typescript
// src/types/launch.ts

export type EnvVarSource = 'proton_runtime' | 'launch_optimization' | 'host' | 'steam_proton';

export interface PreviewEnvVar {
  key: string;
  value: string;
  source: EnvVarSource;
}

export interface ProtonSetup {
  wine_prefix_path: string;
  compat_data_path: string;
  steam_client_install_path: string;
  proton_executable: string;
}

export interface PreviewTrainerInfo {
  path: string;
  host_path: string;
  loading_mode: string;
  staged_path: string | null;
}

export interface PreviewValidation {
  passed: boolean;
  issues: LaunchValidationIssue[];
}

export interface LaunchPreview {
  resolved_method: 'steam_applaunch' | 'proton_run' | 'native';
  validation: PreviewValidation;
  /** null when directive resolution fails — check directives_error for details. */
  environment: PreviewEnvVar[] | null;
  cleared_variables: string[];
  /** null when directive resolution fails. */
  wrappers: string[] | null;
  /** null when directive resolution or command building fails. */
  effective_command: string | null;
  /** Error message when directives/command resolution fails, shown alongside partial results. */
  directives_error: string | null;
  steam_launch_options: string | null;
  proton_setup: ProtonSetup | null;
  working_directory: string;
  game_executable: string;
  game_executable_name: string;
  trainer: PreviewTrainerInfo | null;
  /** ISO 8601 timestamp — filesystem checks can go stale between preview and launch. */
  generated_at: string;
}
```

---

## API Design

### Tauri Command

```rust
// src-tauri/src/commands/launch.rs

#[tauri::command]
pub fn preview_launch(request: LaunchRequest) -> Result<LaunchPreview, String> {
    build_launch_preview(&request).map_err(|error| error.to_string())
}
```

**Input**: `LaunchRequest` (existing type, already serializable, same struct used by `launch_game` and `launch_trainer`).

**Output**: `Result<LaunchPreview, String>` — the preview never fails fatally (validation errors are part of the result), but it can fail if the request itself is malformed beyond recovery.

**Error handling**: Errors are runtime errors (e.g., the request has a completely unrecognizable method string). Validation failures are NOT errors; they are data inside `LaunchPreview.validation`.

### Frontend Invocation

```typescript
// In usePreviewState.ts or directly in LaunchPanel:

const preview = await invoke<LaunchPreview>('preview_launch', { request });
```

---

## Core Preview Function

```rust
// crates/crosshook-core/src/launch/preview.rs

use super::env::WINE_ENV_VARS_TO_CLEAR;
use super::optimizations::{resolve_launch_directives, build_steam_launch_options_command};
use super::request::{validate_all, LaunchRequest, METHOD_STEAM_APPLAUNCH, METHOD_PROTON_RUN, METHOD_NATIVE};
use super::runtime_helpers::{resolve_wine_prefix_path, resolve_steam_client_install_path};

pub fn build_launch_preview(request: &LaunchRequest) -> Result<LaunchPreview, String> {
    let resolved_method = request.resolved_method().to_string();
    let validation_issues = validate_all(request);
    let validation_passed = validation_issues.is_empty();

    // Resolve launch directives (wrappers + optimization env).
    // This can fail independently of validation (e.g., missing wrapper binary).
    // On failure, capture the error and continue with partial results.
    let (directives, directives_error) = match resolve_launch_directives(request) {
        Ok(d) => (Some(d), None),
        Err(e) => (None, Some(e.to_string())),
    };

    // Environment and command depend on successful directive resolution.
    let (environment, wrappers, effective_command) = match &directives {
        Some(directives) => {
            let mut env = Vec::new();
            collect_host_environment(&mut env);
            match resolved_method.as_str() {
                METHOD_STEAM_APPLAUNCH => {
                    collect_steam_proton_environment(request, &mut env);
                }
                METHOD_PROTON_RUN => {
                    collect_runtime_proton_environment(request, &mut env);
                    collect_optimization_environment(directives, &mut env);
                }
                _ => {}
            }
            let cmd = build_effective_command_string(request, &resolved_method, directives);
            (Some(env), Some(directives.wrappers.clone()), Some(cmd))
        }
        None => (None, None, None),
    };

    // Steam launch options (for copy/paste)
    let steam_launch_options = if resolved_method == METHOD_STEAM_APPLAUNCH {
        build_steam_launch_options_command(
            &request.optimizations.enabled_option_ids
        ).ok()
    } else {
        None
    };

    // These sections are independent of directive resolution.
    let proton_setup = build_proton_setup(request, &resolved_method);
    let trainer = build_trainer_info(request, &resolved_method);
    let cleared_variables = if resolved_method != METHOD_NATIVE {
        WINE_ENV_VARS_TO_CLEAR.iter().map(|s| s.to_string()).collect()
    } else {
        Vec::new()
    };
    let working_directory = resolve_working_directory(request);
    let generated_at = chrono::Utc::now().to_rfc3339();

    Ok(LaunchPreview {
        resolved_method,
        validation: PreviewValidation {
            passed: validation_passed,
            issues: validation_issues,
        },
        environment,
        cleared_variables,
        wrappers,
        effective_command,
        directives_error,
        steam_launch_options,
        proton_setup,
        working_directory,
        game_executable: request.game_path.trim().to_string(),
        game_executable_name: request.game_executable_name(),
        trainer,
        generated_at,
    })
}
```

The helper functions (`collect_host_environment`, `collect_steam_proton_environment`, `collect_runtime_proton_environment`, `collect_optimization_environment`) mirror the existing `apply_*` functions in `runtime_helpers.rs` but return `PreviewEnvVar` items instead of mutating a `Command`.

---

## System Constraints

### Performance

- All computation is CPU-bound with minor filesystem `stat()` calls (path existence checks in validation).
- Total preview computation: < 1ms on any hardware, including Steam Deck.
- JSON serialization: ~1-2KB payload, negligible overhead.
- No async required; the Tauri command can be synchronous (no `async fn`).

### Serialization

- `LaunchPreview` uses only `String`, `Vec`, `Option`, and simple enums — all trivially serializable.
- `LaunchDirectives` needs `Serialize`/`Deserialize` added (currently missing, one-line change).
- All field names use `snake_case` to match Tauri's default serde convention and the existing TypeScript types.

### Staleness

- `validate()` checks filesystem state (`Path::exists()`, `Path::is_file()`, executable permissions). `resolve_launch_directives()` scans `PATH` for wrapper binaries. These are point-in-time checks — results can diverge between preview and actual launch.
- The `generated_at` timestamp in `LaunchPreview` lets the frontend show staleness (e.g., "Preview generated 5 minutes ago") and encourage re-preview before launch.

### Steam Deck Compatibility

- Preview computation adds zero runtime overhead to the actual launch path.
- Preview UI must be navigable via gamepad (existing `useGamepadNav` hook handles this if standard buttons and focusable elements are used).
- No additional system dependencies required.

### CLI Integration

- Because core preview logic lives in `crosshook-core` (not `src-tauri`), `crosshook-cli` can trivially add a `Command::Preview` subcommand that calls `build_launch_preview()` and prints the result as formatted text or JSON. This aligns with the existing workspace crate separation pattern.

---

## Codebase Changes

### Files to Create

| File                                          | Purpose                                                                                                         |
| --------------------------------------------- | --------------------------------------------------------------------------------------------------------------- |
| `crates/crosshook-core/src/launch/preview.rs` | Core preview logic: `build_launch_preview()`, env collection helpers, `LaunchPreview` struct + supporting types |
| `src/hooks/usePreviewState.ts`                | React hook for preview invocation and state (loading, result, error)                                            |

### Files to Modify

| File                                                | Change                                                                                                                       |
| --------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------- |
| `crates/crosshook-core/src/launch/mod.rs`           | Add `pub mod preview;` and re-export `LaunchPreview`, `build_launch_preview`                                                 |
| `crates/crosshook-core/src/launch/request.rs`       | Add `validate_all()` function + helper collectors (`collect_steam_issues`, `collect_proton_issues`, `collect_native_issues`) |
| `crates/crosshook-core/src/launch/optimizations.rs` | Add `#[derive(Serialize, Deserialize)]` to `LaunchDirectives`                                                                |
| `src-tauri/src/commands/launch.rs`                  | Add `preview_launch` Tauri command                                                                                           |
| `src-tauri/src/lib.rs`                              | Register `commands::launch::preview_launch` in `invoke_handler`                                                              |
| `src/types/launch.ts`                               | Add `LaunchPreview`, `PreviewEnvVar`, `ProtonSetup`, `PreviewTrainerInfo`, `PreviewValidation` interfaces                    |
| `src/components/LaunchPanel.tsx`                    | Add "Preview" button + preview result display                                                                                |

### Files NOT Modified

- `src/hooks/useLaunchState.ts` — the preview state is separate from launch state; no reason to couple them.
- `crates/crosshook-core/src/launch/script_runner.rs` — preview does not build actual Commands.
- `crates/crosshook-core/src/launch/runtime_helpers.rs` — the env-collection logic for preview lives in the new `preview.rs` module, mirroring these helpers but as pure data functions. The existing helpers remain untouched.

---

## Technical Decisions

### 1. Single Tauri Command (Recommended)

| Option                         | Pros                                        | Cons                                |
| ------------------------------ | ------------------------------------------- | ----------------------------------- |
| **A: Single `preview_launch`** | One invoke, atomic result, simpler frontend | Slightly larger response            |
| B: Multiple commands           | Incremental loading                         | Multiple round-trips, complex state |

**Recommendation**: Option A. All computation is < 1ms. No benefit to splitting.

### 2. New `validate_all()` Function (Recommended)

| Option                      | Pros                                       | Cons                                                     |
| --------------------------- | ------------------------------------------ | -------------------------------------------------------- |
| **A: New `validate_all()`** | Shows all issues at once (Terraform-style) | ~100 lines of new validation code mirroring `validate()` |
| B: Reuse `validate()`       | Zero backend changes                       | Only shows first error — defeats the purpose             |

**Recommendation**: Option A. Core value proposition of the preview feature. The `collect_*_issues` helpers can be extracted from the existing `validate_*` functions with minimal duplication.

### 3. Core Logic in crosshook-core (Recommended)

| Option                                    | Pros                           | Cons                               |
| ----------------------------------------- | ------------------------------ | ---------------------------------- |
| **A: `crosshook-core/launch/preview.rs`** | Unit-testable, reusable by CLI | New module                         |
| B: Inline in Tauri command                | Fewer files                    | Not testable without Tauri runtime |

**Recommendation**: Option A. Follows the existing workspace crate separation pattern where `crosshook-core` contains all business logic and `src-tauri` is a thin consumer.

### 4. Separate `usePreviewState` Hook (Recommended)

| Option                          | Pros                           | Cons                                |
| ------------------------------- | ------------------------------ | ----------------------------------- |
| **A: New `usePreviewState.ts`** | Clean separation, simple state | New file                            |
| B: Extend `useLaunchState.ts`   | Fewer files                    | Couples preview to launch lifecycle |

**Recommendation**: Option A. Preview and launch are independent operations with different state machines. Coupling them would add complexity to the already non-trivial `useLaunchState` reducer.

### 5. Environment Collection Strategy (Recommended)

| Option                                         | Pros                        | Cons                                  |
| ---------------------------------------------- | --------------------------- | ------------------------------------- |
| **A: New pure functions in `preview.rs`**      | No changes to existing code | Some logic duplication                |
| B: Refactor `runtime_helpers.rs` to be generic | DRY                         | Risky refactor of working launch code |

**Recommendation**: Option A. The existing `apply_*` functions in `runtime_helpers.rs` work on `tokio::process::Command` objects. Refactoring them to be generic over both `Command` mutation and data collection would add complexity to critical launch-path code for the sake of DRY. The preview helpers are ~30 lines total and change infrequently.

---

## Open Questions

1. **Preview for both game AND trainer?** The current design previews the request as-is (respecting `launch_game_only` / `launch_trainer_only`). Should the preview show both the game launch AND trainer launch steps together, or preview one step at a time matching the existing two-step flow?

2. **Copy-to-clipboard for Steam Launch Options?** The preview could include a "Copy to clipboard" action for the `steam_launch_options` string. This requires the `tauri-plugin-clipboard-manager` plugin (not currently installed) or a browser-API approach via `navigator.clipboard`.

3. **Preview auto-refresh?** Should the preview auto-update when the user changes profile fields, or only when explicitly requested via button click? Auto-refresh adds reactivity but may be distracting; explicit invocation matches the Terraform `plan` metaphor.

4. **Should `validate_all()` also surface informational/warning-level issues?** Currently all `ValidationError` variants return `ValidationSeverity::Fatal`. If the feature wants to surface warnings (e.g., "Proton path exists but is an older version"), new warning variants would need to be added to `ValidationError`. This could be a follow-up enhancement.
