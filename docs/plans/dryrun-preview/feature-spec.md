# Feature Spec: Dry Run / Preview Launch Mode

## Executive Summary

The dry-run preview feature (#40) exposes CrossHook's existing pure computation functions -- `validate()`, `resolve_launch_directives()`, `build_steam_launch_options_command()` -- through a single new `preview_launch` Tauri command and a modal UI, letting users see exactly what CrossHook will do before clicking Launch. All backend computation already exists and is side-effect-free; the work is a new aggregate function in `crosshook-core`, a Tauri IPC wrapper, TypeScript types, and a React modal with collapsible sections. The primary technical gap is that `validate()` short-circuits on the first error, requiring a new `validate_all()` that collects all issues (the Terraform-style "show everything" value proposition). The `LaunchPreview` struct is designed for forward-compatibility with #36 (post-launch diagnostics) and #49 (diagnostic bundle export). No external APIs are involved; no new dependencies are needed.

## External Dependencies

### APIs and Services

None. This feature is entirely self-contained -- all computation is performed by existing pure functions in `crosshook-core`. No external API calls, no network access, no new library dependencies.

### Libraries and SDKs

No new libraries required. The implementation uses:

| Component            | Approach                                | Rationale                                           |
| -------------------- | --------------------------------------- | --------------------------------------------------- |
| Collapsible sections | Existing `CollapsibleSection` component | Already in codebase (commit `79cba3c`)              |
| Modal dialog         | Existing `ProfileReviewModal` pattern   | Reuse focus trapping, gamepad nav, portal rendering |
| Syntax display       | Custom CSS on `<pre>` blocks            | Zero-dependency; matches existing dark theme        |
| Copy to clipboard    | `navigator.clipboard.writeText()`       | Works in Tauri WebView (secure context)             |
| Timestamp            | `chrono::Utc::now().to_rfc3339()`       | `chrono` already in workspace dependencies          |

### External Documentation

- [Tauri v2: Calling Rust from Frontend](https://v2.tauri.app/develop/calling-rust/): IPC command pattern
- [Terraform plan command reference](https://developer.hashicorp.com/terraform/cli/commands/plan): UX inspiration for summary line and structured output

## Business Requirements

### User Stories

**Primary User: Game/Trainer Launcher**

- As a user who has just configured a profile, I want to preview the resolved environment, wrapper chain, and command line, so that I can confirm everything looks correct before launching
- As a user whose game or trainer fails to start, I want to see what CrossHook would have done, so I can identify misconfigurations without trial-and-error restarts
- As a user toggling launch optimizations (MangoHud, GameMode, HDR), I want to see the concrete environment variables and wrapper commands those toggles produce, so I can verify they work as expected

**Secondary User: Community Troubleshooter**

- As a user reporting a bug or asking for help, I want to copy the preview output as structured text, so I can share my exact launch configuration for diagnosis

**Tertiary User: Steam Deck Controller User**

- As a Steam Deck user navigating with a controller, I want to access and read the preview without a keyboard, so the preview is usable in the primary target environment

### Business Rules

| #    | Rule                                                                                                                      | Rationale                                                                                          |
| ---- | ------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------- |
| BR-1 | Preview MUST be read-only -- no filesystem mutations, no process spawning, no trainer staging                             | Preview is a diagnostic tool; side effects belong to the launch path                               |
| BR-2 | Preview MUST show the resolved launch method, not just the configured one                                                 | `resolved_method()` auto-detects when method is empty; users need to see what actually runs        |
| BR-3 | Preview MUST show ALL validation issues, not just the first                                                               | Current `validate()` is fail-fast; preview needs exhaustive reporting (Terraform-style)            |
| BR-4 | Preview MUST show resolved environment variables with actual values and source tags                                       | Users need `WINEPREFIX=/home/user/.steam/...` not `WINEPREFIX=<resolved>`                          |
| BR-5 | Preview MUST be available when a LaunchRequest can be constructed (game path non-empty)                                   | Matches the existing `buildLaunchRequest()` guard in `LaunchPage.tsx`                              |
| BR-6 | Preview MUST be disabled during an active launch session (phase !== Idle)                                                 | Prevents confusion between current session state and preview of next launch                        |
| BR-7 | For `trainer_loading_mode = "copy_to_prefix"`, preview MUST show the predicted staged path WITHOUT actually copying files | `stage_trainer_into_prefix()` has side effects; preview computes the path only                     |
| BR-8 | For `native` method, preview MUST NOT show Proton/WINE sections                                                           | Native launches don't use Proton; showing empty sections would confuse users                       |
| BR-9 | Preview output SHOULD be copyable as structured TOML matching profile data format                                         | Users can share valid snippets or paste into profiles; supports troubleshooting via Discord/GitHub |

### Edge Cases

| Scenario                                            | Expected Behavior                                                | Notes                                             |
| --------------------------------------------------- | ---------------------------------------------------------------- | ------------------------------------------------- |
| Missing wrapper binary (e.g., mangohud not in PATH) | Show validation warning; still display other preview data        | `Option<T>` fields enable partial results         |
| Auto-detected method (empty `method` field)         | Show "auto-detected" label with explanation of heuristic         | `resolved_method()` checks app_id, .exe extension |
| Empty/new profile (no game path)                    | Preview button disabled; same guard as Launch button             | `buildLaunchRequest()` returns null               |
| Very long file paths (Steam Deck display)           | Horizontal scroll or `word-break: break-all` in monospace blocks | 1280x800 screen constraint                        |
| Profile changed after preview generated             | Show staleness indicator via `generated_at` timestamp            | Manual re-preview; no auto-refresh for MVP        |

### Success Criteria

- [ ] A "Preview" button exists alongside the Launch button, disabled when profile is incomplete or launch is active
- [ ] Preview shows resolved method, all validation issues, environment variables (with source), wrapper chain, and effective command
- [ ] Preview runs exhaustive validation (`validate_all`) and displays all results with severity indicators
- [ ] No actual processes are launched or files mutated in preview mode
- [ ] Preview is navigable via gamepad on Steam Deck (expand/collapse sections, copy, close)
- [ ] Preview can be copied to clipboard as structured TOML matching profile data format
- [ ] For two-step methods (steam_applaunch, proton_run), preview shows both game and trainer launch details in a unified view

## Technical Specifications

### Architecture Overview

```
React UI (LaunchPanel.tsx)
  |
  +-- User clicks "Preview Launch"
  |
  v
invoke("preview_launch", { request: LaunchRequest })
  |
  v
Tauri Command: preview_launch(request)     [src-tauri/src/commands/launch.rs]
  |
  v
build_launch_preview(&request)             [crosshook-core/src/launch/preview.rs]
  |
  +-- request.resolved_method()            --> resolved method string
  +-- validate_all(&request)               --> Vec<LaunchValidationIssue>
  +-- resolve_launch_directives(&request)  --> Ok(LaunchDirectives) | Err(...)
  +-- collect_preview_environment(...)     --> Vec<PreviewEnvVar>
  +-- build_effective_command_string(...)  --> String
  +-- resolve_proton_setup(...)           --> Option<ProtonSetup>
  +-- build_trainer_info(...)             --> Option<PreviewTrainerInfo>
  |
  v
LaunchPreview (Serialize --> JSON --> TypeScript)
  |
  v
React modal with collapsible sections
```

### Data Models

#### Rust: `LaunchPreview` (crosshook-core)

```rust
// crates/crosshook-core/src/launch/preview.rs

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EnvVarSource {
    ProtonRuntime,       // STEAM_COMPAT_DATA_PATH, WINEPREFIX, etc.
    LaunchOptimization,  // PROTON_NO_STEAMINPUT, MANGOHUD, etc.
    Host,                // HOME, DISPLAY, PATH, etc.
    SteamProton,         // Steam-specific Proton vars for steam_applaunch
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PreviewEnvVar {
    pub key: String,
    pub value: String,
    pub source: EnvVarSource,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProtonSetup {
    pub wine_prefix_path: String,
    pub compat_data_path: String,
    pub steam_client_install_path: String,
    pub proton_executable: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PreviewTrainerInfo {
    pub path: String,
    pub host_path: String,
    pub loading_mode: String,
    pub staged_path: Option<String>,  // predicted path for copy_to_prefix mode
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PreviewValidation {
    pub passed: bool,
    pub issues: Vec<LaunchValidationIssue>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LaunchPreview {
    pub resolved_method: String,
    pub validation: PreviewValidation,
    pub environment: Option<Vec<PreviewEnvVar>>,     // None on directive failure
    pub cleared_variables: Vec<String>,               // WINE vars cleared before launch
    pub wrappers: Option<Vec<String>>,                // None on directive failure
    pub effective_command: Option<String>,             // None on directive/command failure
    pub directives_error: Option<String>,             // Error when directives fail
    pub steam_launch_options: Option<String>,          // steam_applaunch only
    pub proton_setup: Option<ProtonSetup>,            // None for native
    pub working_directory: String,
    pub game_executable: String,
    pub game_executable_name: String,
    pub trainer: Option<PreviewTrainerInfo>,           // None if no trainer configured
    pub generated_at: String,                         // ISO 8601 timestamp
}
```

**Key design decisions:**

- `Option<T>` for `environment`, `wrappers`, `effective_command` enables partial results when directive resolution fails
- `directives_error` carries the failure reason alongside partial data
- `EnvVarSource` tags let the UI group variables by origin
- `generated_at` enables staleness detection in the UI

#### TypeScript: `LaunchPreview` Interface

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
  environment: PreviewEnvVar[] | null;
  cleared_variables: string[];
  wrappers: string[] | null;
  effective_command: string | null;
  directives_error: string | null;
  steam_launch_options: string | null;
  proton_setup: ProtonSetup | null;
  working_directory: string;
  game_executable: string;
  game_executable_name: string;
  trainer: PreviewTrainerInfo | null;
  generated_at: string;
}
```

### API Design

#### `preview_launch` Tauri Command

**Purpose**: Compute a complete launch preview without side effects.

**Input**: `LaunchRequest` (existing serializable type -- same struct used by `launch_game` and `launch_trainer`)

**Output**: `Result<LaunchPreview, String>`

```rust
// src-tauri/src/commands/launch.rs

#[tauri::command]
pub fn preview_launch(request: LaunchRequest) -> Result<LaunchPreview, String> {
    build_launch_preview(&request).map_err(|error| error.to_string())
}
```

**Error semantics**: The command only errors on malformed requests (unrecognizable method). Validation failures are data inside `LaunchPreview.validation`, not command errors.

**Frontend invocation**:

```typescript
const preview = await invoke<LaunchPreview>('preview_launch', { request });
```

**Errors:**

| Condition                          | Behavior                                                                                             |
| ---------------------------------- | ---------------------------------------------------------------------------------------------------- |
| Malformed request (unknown method) | Returns `Err(String)` -- caught by frontend error handler                                            |
| Validation failures                | Returns `Ok(LaunchPreview)` with `validation.passed = false` and `validation.issues` populated       |
| Directive resolution failure       | Returns `Ok(LaunchPreview)` with `environment = null`, `wrappers = null`, `directives_error = "..."` |

### System Integration

#### Files to Create

| File                                          | Purpose                                                                                                                                     |
| --------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------- |
| `crates/crosshook-core/src/launch/preview.rs` | Core preview logic: `build_launch_preview()`, env collection helpers, `LaunchPreview` struct + supporting types, `to_display_toml()` method |
| `src/hooks/usePreviewState.ts`                | React hook for preview invocation and state (loading, result, error)                                                                        |

#### Files to Modify

| File                                                | Change                                                                                                                          |
| --------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------- |
| `crates/crosshook-core/src/launch/mod.rs`           | Add `pub mod preview;` and re-export `LaunchPreview`, `build_launch_preview`                                                    |
| `crates/crosshook-core/src/launch/request.rs`       | Add `validate_all()` function with collector helpers (`collect_steam_issues`, `collect_proton_issues`, `collect_native_issues`) |
| `crates/crosshook-core/src/launch/optimizations.rs` | Add `#[derive(Serialize, Deserialize)]` to `LaunchDirectives` (one-line change)                                                 |
| `src-tauri/src/commands/launch.rs`                  | Add `preview_launch` Tauri command                                                                                              |
| `src-tauri/src/lib.rs`                              | Register `commands::launch::preview_launch` in `invoke_handler`                                                                 |
| `src/types/launch.ts`                               | Add `LaunchPreview`, `PreviewEnvVar`, `ProtonSetup`, `PreviewTrainerInfo`, `PreviewValidation`, `EnvVarSource` types            |
| `src/components/LaunchPanel.tsx`                    | Add "Preview" button + preview modal with collapsible sections                                                                  |

#### Files NOT Modified

- `src/hooks/useLaunchState.ts` -- preview state is separate from launch state; no coupling
- `crates/crosshook-core/src/launch/script_runner.rs` -- preview does not build actual `Command` objects
- `crates/crosshook-core/src/launch/runtime_helpers.rs` -- preview has its own pure env collection functions

## UX Considerations

### User Workflows

#### Primary Workflow: Preview Before Launch

1. **Configure** -- User selects profile, adjusts settings
2. **Preview** -- User clicks "Preview Launch" button (secondary/ghost style, next to Launch Game)
3. **System computes** -- All pure functions called, LaunchPreview assembled (<1ms)
4. **Modal opens** -- Structured preview with summary banner, collapsible sections
5. **User reviews** -- Expands sections of interest, checks validation results
6. **Decision**:
   - If valid and correct -> clicks "Launch Now" in modal footer -> launch begins
   - If issues found -> clicks "Close" -> adjusts profile -> re-previews
   - If needs help -> clicks "Copy Preview" -> pastes into Discord/GitHub

#### Error Recovery Workflow

1. **Preview** with missing wrapper (e.g., mangohud not in PATH)
2. **Modal shows** partial results: validation issues populated (warning about missing binary), environment/command sections show "Could not fully resolve" with error message
3. **User identifies** the missing dependency from the validation section
4. **User installs** the binary or disables the optimization
5. **User re-previews** to verify the fix

### UI Patterns

| Component      | Pattern                                           | Notes                                                          |
| -------------- | ------------------------------------------------- | -------------------------------------------------------------- |
| Container      | Modal dialog (full-viewport on Steam Deck)        | Reuse `ProfileReviewModal` infrastructure                      |
| Sections       | Collapsible accordion via `CollapsibleSection`    | Progressive disclosure; independent expand/collapse            |
| Summary        | Always-visible banner with Terraform-style counts | "Preview: 12 env vars, 3 wrappers, 5 checks passed, 1 warning" |
| Validation     | Severity-grouped list with icons                  | Errors first, then warnings, then passes; icons + color + text |
| Command chain  | Monospace code block with recessed background     | Line-continuation markers for multi-wrapper chains             |
| Env vars       | Two-column key-value table                        | Monospace, muted keys, standard values; grouped by source      |
| Footer actions | Copy Preview / Launch Now / Close                 | "Launch Now" disabled if validation errors exist               |

**Section default states:**

1. **Summary** -- always visible (not collapsible)
2. **Validation Results** -- expanded by default
3. **Command Chain** -- expanded by default
4. **Environment Variables** -- collapsed by default
5. **Proton / Runtime Setup** -- collapsed by default (hidden for `native` method)

### Accessibility Requirements

- Color-coded severity MUST be paired with icons and text labels (never color alone)
- All interactive elements meet 48px minimum touch target for Steam Deck
- Focus trap within modal; Tab order flows through sections to footer actions
- Gamepad controller prompts: A=expand, B=close, X=copy, Y=launch

### Performance UX

- **Computation**: <1ms (pure functions, minor filesystem stat calls)
- **Loading state**: Brief skeleton/shimmer (100-200ms minimum display) to signal freshness
- **No async required**: Tauri command can be synchronous
- **Payload size**: ~2-5KB JSON -- negligible IPC overhead

## Recommendations

### Implementation Approach

**Recommended Strategy**: Single `preview_launch` Tauri command in `crosshook-core` with modal UI. MVP in one PR, polish in a follow-up.

**Phasing:**

1. **Phase A -- MVP (this PR)**: `preview.rs` with `LaunchPreview` struct and `build_launch_preview()`, `validate_all()` in request.rs, Tauri command, TypeScript types, "Preview" button in LaunchPanel, modal with basic section rendering
2. **Phase B -- Polish (follow-up PR)**: Copy-to-clipboard button, collapsible sections with proper expand/collapse, CLI `crosshook preview` command, Steam Launch Options copy button, toast confirmation on copy
3. **Phase C -- Cross-Feature (separate PRs)**: Reuse `LaunchPreview` as "before" snapshot in #36 (post-launch diagnostics), include preview JSON in #49 (diagnostic bundle)

### Technology Decisions

| Decision                 | Recommendation                                                 | Rationale                                                              |
| ------------------------ | -------------------------------------------------------------- | ---------------------------------------------------------------------- |
| Tauri command structure  | Single `preview_launch` returning unified struct               | Atomic snapshot, no TOCTOU risk, single IPC round-trip                 |
| Validation strategy      | New `validate_all()` function                                  | Core value proposition -- show everything, not just first error        |
| Core logic placement     | `crosshook-core/src/launch/preview.rs`                         | Unit-testable, reusable by CLI, follows workspace separation pattern   |
| Preview state management | Separate `usePreviewState.ts` hook                             | Decoupled from launch lifecycle; simpler state machine                 |
| Env collection           | New pure functions in `preview.rs`                             | Avoids risky refactor of working `Command`-mutating launch code        |
| UI container             | Modal dialog                                                   | Focused attention, existing infrastructure, Steam Deck gamepad support |
| Collapsible sections     | Existing `CollapsibleSection` component                        | No new dependency, already accessible, gamepad-compatible              |
| Clipboard                | `navigator.clipboard.writeText()` + custom `to_display_toml()` | Zero-dependency, works in Tauri WebView; TOML format matches profiles  |

### Quick Wins

- Add `#[derive(Serialize, Deserialize)]` to `LaunchDirectives` (one-line change, unblocks preview)
- The `build_steam_launch_options_command()` result is directly useful as a copyable string for Steam users
- `to_display_toml()` on `LaunchPreview` serves UI clipboard, CLI output, and produces valid TOML snippets users can share or paste into profiles

### Future Enhancements

- **Diff view**: Show what changed since last preview (useful when tweaking optimizations)
- **Preview history**: Keep last N previews in session memory for comparison
- **CLI preview**: `crosshook preview --profile elden-ring [--json]` for headless/scripted usage
- **Auto-preview on profile change**: Re-run preview when fields change (consider performance)
- **Quick-fix actions**: From a validation error, navigate directly to the relevant profile field

## Risk Assessment

### Technical Risks

| Risk                                                         | Likelihood | Impact | Mitigation                                                                                  |
| ------------------------------------------------------------ | ---------- | ------ | ------------------------------------------------------------------------------------------- |
| Preview/launch divergence from filesystem changes            | Medium     | Medium | `generated_at` timestamp + staleness indicator; actual launch re-validates independently    |
| `stage_trainer_into_prefix()` side effects leak into preview | Low        | High   | Preview computes staged path via string manipulation only; never calls the staging function |
| PATH-dependent wrapper availability changes                  | Low        | Low    | Preview reflects point-in-time state; cheap to re-generate                                  |
| Trainer staging description diverges from actual staging     | Low        | High   | Keep staging path logic close to `stage_trainer_into_prefix()`; test both paths             |

### Integration Challenges

- **`validate_all()` duplication**: The collector functions mirror existing `validate_*()` logic. Mitigated by extracting individual checks as shared helpers where possible.
- **Frontend type drift**: TypeScript `LaunchPreview` could diverge from Rust struct. Standard Tauri IPC risk; mitigate with integration tests.

### Security Considerations

- Preview displays full file paths including user home directory. The "Copy Preview" feature should include a note that the output may contain identifying paths.
- No network access, no external data, no user input beyond the existing profile data.

## Task Breakdown Preview

### Phase A: MVP

**Focus**: Backend preview function + frontend button + modal display

**Tasks**:

- Define `LaunchPreview` and supporting types in `crosshook-core/src/launch/preview.rs`
- Implement `build_launch_preview()` assembling validation, directives, env, command
- Implement `validate_all()` in `request.rs` with method-specific collectors
- Add `Serialize`/`Deserialize` to `LaunchDirectives`
- Add `preview_launch` Tauri command + register in handler
- Add TypeScript types in `src/types/launch.ts`
- Add `usePreviewState` hook for preview invocation/state
- Add "Preview" button and modal display in `LaunchPanel.tsx`
- Unit tests for `build_launch_preview()` and `validate_all()`

**Parallelization**: Backend types/function (tasks 1-4) can run in parallel with frontend types (task 6). Tauri command (task 5) depends on backend. UI (tasks 7-8) depends on frontend types. Tests (task 9) depend on backend function.

### Phase B: Polish

**Focus**: Clipboard, collapsible sections, CLI command

**Dependencies**: Phase A complete
**Tasks**:

- Copy-to-clipboard button using `to_display_toml()` (structured TOML output)
- Collapsible sections with proper expand/collapse defaults
- CLI `Command::Preview` in `crosshook-cli`
- Steam Launch Options dedicated copy button
- Toast confirmation on clipboard copy

### Phase C: Cross-Feature Integration

**Focus**: Reuse `LaunchPreview` for diagnostics features

**Tasks**:

- Capture preview at launch time for #36 (post-launch diagnostics)
- Include preview JSON in #49 (diagnostic bundle)
- Diff view comparing two `LaunchPreview` instances

## Decisions (Resolved)

1. **Preview for both game AND trainer?** -- **Yes, unified view showing both steps.** The preview shows both game and trainer launch details in a single view, matching the "show everything" philosophy.

2. **Environment snapshot depth** -- **Include host passthrough vars** (HOME, DISPLAY, PATH). Collapsed by default with source tags for filtering.

3. **Preview auto-refresh** -- **Manual re-trigger only** for MVP. Matches the Terraform `plan` metaphor. No auto-refresh on profile change.

4. **Clipboard format** -- **Structured TOML matching profile data format.** Users can copy/paste valid TOML snippets directly into profiles or share them in community channels. The `to_display_toml()` method on `LaunchPreview` renders the preview as TOML-compatible structured output, making it both human-readable and machine-parseable by CrossHook's existing TOML infrastructure.

## Research References

For detailed findings, see:

- [research-external.md](./research-external.md): Comparable tool implementations (Terraform, Docker, Ansible, game launchers), UI rendering libraries, integration patterns
- [research-business.md](./research-business.md): User stories, business rules, existing function signatures, domain model, data flow
- [research-technical.md](./research-technical.md): Architecture design, complete Rust/TypeScript data models, Tauri command specification, system constraints
- [research-ux.md](./research-ux.md): Modal vs panel analysis, competitive analysis, Steam Deck considerations, accessibility, performance UX
- [research-recommendations.md](./research-recommendations.md): Implementation approach, alternative evaluation, risk assessment, task breakdown
