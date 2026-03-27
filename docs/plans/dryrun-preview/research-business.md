# Dryrun Preview — Business Logic & Codebase Research

## Executive Summary

The dryrun-preview feature exposes CrossHook's existing pure computation functions — validation, environment resolution, directive resolution, and command construction — through a read-only Tauri command and a new UI panel. All core computation already exists in `crosshook-core`; the feature requires one new Tauri command, one new IPC response type, and a frontend preview component. The primary backend challenge is that `validate()` is fail-fast (returns the first error), while a useful preview needs exhaustive validation. The primary frontend challenge is presenting method-specific data (native vs proton_run vs steam_applaunch) in a coherent, navigable layout that works with gamepad controls.

---

## User Stories

### US-1: Debugging Failed Launches

**As a** user whose game or trainer fails to start,
**I want to** see exactly what CrossHook would have done before launching,
**so that** I can identify misconfigured paths, missing binaries, or conflicting optimizations without trial-and-error restarts.

### US-2: Verifying Configuration Before Launch

**As a** user who has just finished setting up a profile,
**I want to** preview the resolved environment, wrapper chain, and effective command line,
**so that** I can confirm everything looks correct before committing to a launch that may take time to fail.

### US-3: Understanding What CrossHook Does

**As a** new user unfamiliar with Proton/WINE internals,
**I want to** see a human-readable breakdown of the launch steps (environment variables set, variables cleared, wrappers applied),
**so that** I can learn how CrossHook orchestrates the launch without reading source code.

### US-4: Sharing Configurations for Troubleshooting

**As a** user reporting a bug or asking for help in a community channel,
**I want to** copy the preview output as structured text,
**so that** I can share my exact launch configuration with others for diagnosis.

### US-5: Verifying Optimization Effects

**As a** user who has toggled launch optimizations (MangoHud, GameMode, HDR, etc.),
**I want to** see the concrete environment variables and wrapper commands those toggles produce,
**so that** I can verify the optimizations are applied as expected.

### US-6: Steam Deck Gamepad-Driven Preview

**As a** Steam Deck user navigating with a controller,
**I want to** access and read the preview without a keyboard,
**so that** the preview is usable in the primary target environment.

---

## Business Rules

### Core Rules

| #    | Rule                                                                                                       | Rationale                                                                                       |
| ---- | ---------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------- |
| BR-1 | Preview MUST be read-only — no filesystem mutations, no process spawning, no trainer staging               | Preview is a diagnostic tool; side effects belong to the launch path                            |
| BR-2 | Preview MUST show the resolved launch method, not just the configured one                                  | `resolved_method()` auto-detects when method is empty; users need to see what will actually run |
| BR-3 | Preview MUST show ALL validation issues, not just the first                                                | Current `validate()` is fail-fast; preview needs exhaustive reporting for diagnostic value      |
| BR-4 | Preview MUST show resolved environment variables with actual values                                        | Users need to see `WINEPREFIX=/home/user/.steam/...` not just `WINEPREFIX=<resolved>`           |
| BR-5 | Preview MUST show optimization directives (env vars + wrappers) when optimizations are enabled             | This is the primary "what will actually happen" insight for optimization users                  |
| BR-6 | Preview MUST be available when a LaunchRequest can be constructed (game path is non-empty)                 | Matches the existing `buildLaunchRequest()` guard in `LaunchPage.tsx:15`                        |
| BR-7 | Preview MUST be disabled during an active launch session (phase !== Idle)                                  | Prevents confusion between current session state and preview of next launch                     |
| BR-8 | For two-step methods (steam_applaunch, proton_run), preview SHOULD show both game and trainer perspectives | Different validation paths apply (`launch_game_only` vs `launch_trainer_only`)                  |
| BR-9 | Preview output SHOULD be copyable as structured text for sharing                                           | Supports US-4 troubleshooting workflow                                                          |

### Edge Case Rules

| #    | Rule                                                                                                                                                                    | Rationale                                                                                                                    |
| ---- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------- |
| EC-1 | When `trainer_loading_mode = "copy_to_prefix"`, preview MUST show the predicted staged path (`C:\CrossHook\StagedTrainers\{name}\{exe}`) WITHOUT actually copying files | `stage_trainer_into_prefix()` has side effects; preview must compute the path without executing the copy                     |
| EC-2 | When wrapper binaries are missing from PATH, preview SHOULD show the missing dependency as a validation issue while still displaying other preview data                 | Current `resolve_launch_directives()` fails on missing binary; preview should degrade gracefully                             |
| EC-3 | When method is auto-detected (empty `method` field), preview SHOULD explicitly show that auto-detection occurred and why                                                | `resolved_method()` has heuristics (app_id present → steam_applaunch, .exe → proton_run); making this visible aids debugging |
| EC-4 | For `native` method, preview MUST NOT show Proton/WINE environment sections                                                                                             | Native launches don't use Proton; showing empty Proton sections would confuse users                                          |
| EC-5 | For empty/new profiles, preview SHOULD show a clear "incomplete profile" state rather than a wall of validation errors                                                  | Better UX than listing 5+ "required field" errors                                                                            |

---

## Workflows

### Primary Workflow: Preview Before Launch

```
1. User navigates to Launch page (LaunchPage.tsx)
2. User selects a profile from the Active Profile dropdown
3. System builds LaunchRequest via buildLaunchRequest() → non-null if game path exists
4. User clicks "Preview Launch" button (visible when LaunchRequest is non-null and phase === Idle)
5. Frontend invokes new Tauri command: preview_launch(request: LaunchRequest)
6. Backend executes (all read-only):
   a. request.resolved_method() → effective method
   b. validate() or validate_all() → Vec<LaunchValidationIssue>
   c. resolve_launch_directives() → LaunchDirectives { env, wrappers }
   d. build_steam_launch_options_command() → %command% string (if steam_applaunch)
   e. Compute resolved Proton environment (WINEPREFIX, STEAM_COMPAT_DATA_PATH, etc.)
   f. Assemble host environment snapshot (HOME, PATH, DISPLAY, etc.)
7. Backend returns DryRunPreview struct to frontend
8. Frontend displays preview in a structured panel (collapsible sections)
9. User reviews preview:
   → If validation passes and preview looks correct → clicks "Launch Game"
   → If validation fails → reads help text → navigates to Profiles page to fix
   → If preview looks unexpected → adjusts optimizations/settings on Launch page
```

### Error Recovery Workflow

```
1. User clicks "Preview Launch"
2. Backend encounters error in resolve_launch_directives() (e.g. missing mangohud)
3. Backend continues collecting other preview data (validation, env, etc.)
4. Backend returns partial DryRunPreview with:
   - validation_issues populated (including the optimization dependency error)
   - directives: empty or partial
   - All other fields still populated
5. Frontend renders preview with warning banner: "Some data could not be fully resolved"
6. User sees the specific missing dependency and help text
7. User installs missing binary or disables the optimization
8. User re-runs preview to verify fix
```

### Copy-for-Sharing Workflow

```
1. User has preview open showing full launch details
2. User clicks "Copy Preview" button
3. System serializes preview to structured text (markdown or plain text):
   - Method, paths, validation status
   - Environment variables (grouped)
   - Wrapper chain
   - Effective command
4. Text is copied to clipboard
5. User pastes into Discord/GitHub issue/forum for troubleshooting
```

---

## Domain Model

### Entity Map

```
GameProfile (TOML on disk)
  └─ buildLaunchRequest() ──→ LaunchRequest
                                  │
                    ┌─────────────┼─────────────────┐
                    ▼             ▼                  ▼
              validate()   resolve_launch     build_steam_launch
                    │      _directives()      _options_command()
                    ▼             ▼                  ▼
          Vec<Validation   LaunchDirectives    String (%command%)
              Issue>       { env, wrappers }
                    │             │                  │
                    └─────────────┼──────────────────┘
                                  ▼
                          DryRunPreview (new aggregate)
                                  │
                                  ▼
                          Frontend Panel
```

### Key Entities

**LaunchRequest** — The central input. Built from a `GameProfile` + `steamClientInstallPath` on the frontend (`LaunchPage.tsx:10-43`). Contains all configuration needed to compute every aspect of the launch.

**LaunchDirectives** — Output of optimization resolution. `{ env: Vec<(String, String)>, wrappers: Vec<String> }`. Represents the concrete environment variables and command wrappers that optimizations produce. Only meaningful for `proton_run` method.

**LaunchValidationIssue** — `{ message: String, help: String, severity: ValidationSeverity }`. Provides both the problem description and actionable remediation guidance. Severity infrastructure exists for Fatal/Warning/Info but currently all variants return Fatal.

**ValidationError** — Rust enum with 20+ variants covering every validation check. Each variant maps to a `LaunchValidationIssue` via `.issue()`. Categories: path existence, file type, method compatibility, optimization conflicts, dependency availability.

**DryRunPreview** (new) — The aggregate response type. Combines validation results, resolved directives, environment snapshot, command reconstruction, and metadata into a single IPC payload.

### Data Flow

```
Frontend                          Backend (crosshook-core)
─────────                         ─────────────────────────
GameProfile ──┐
              ├─→ LaunchRequest ──→ preview_launch() ──┐
steamClient ──┘                                        │
                                    ┌──────────────────┘
                                    ├─→ resolved_method()
                                    ├─→ validate() / validate_all()
                                    ├─→ resolve_launch_directives()
                                    ├─→ build_steam_launch_options_command()
                                    ├─→ resolve_wine_prefix_path()
                                    ├─→ resolve_steam_client_install_path()
                                    └─→ DryRunPreview ──→ Frontend
```

### State Transitions

Preview does not introduce new launch phases. It operates entirely within the `Idle` phase:

```
LaunchPhase.Idle ──[click Preview]──→ (compute, no phase change) ──→ display preview
                 ──[click Launch]──→ LaunchPhase.GameLaunching (existing flow)
```

The preview panel can be open or closed independent of the launch phase, but the "Preview Launch" button is only enabled during `Idle`.

---

## Existing Codebase Analysis

### Function Signatures & Return Types

#### `validate()` — `crates/crosshook-core/src/launch/request.rs:442`

```rust
pub fn validate(request: &LaunchRequest) -> Result<(), ValidationError>
```

- **Behavior**: Fail-fast; returns `Ok(())` or `Err(first_error)`
- **Side effects**: Filesystem reads (existence checks, permission checks via `fs::metadata`)
- **Dispatches to**: `validate_steam_applaunch()`, `validate_proton_run()`, `validate_native()` based on `resolved_method()`
- **Preview impact**: Needs a companion `validate_all()` that collects all errors

#### `resolve_launch_directives()` — `crates/crosshook-core/src/launch/optimizations.rs:267`

```rust
pub fn resolve_launch_directives(
    request: &LaunchRequest
) -> Result<LaunchDirectives, ValidationError>
```

Returns:

```rust
pub struct LaunchDirectives {
    pub env: Vec<(String, String)>,  // e.g. [("PROTON_NO_STEAMINPUT", "1")]
    pub wrappers: Vec<String>,       // e.g. ["mangohud", "gamemoderun"]
}
```

- **Behavior**: Only processes `proton_run` method; errors on others if optimizations present
- **Side effects**: PATH scanning for binary availability (`is_command_available()`)
- **Preview impact**: Can be called directly; errors should be captured as validation issues

#### `resolve_launch_directives_for_method()` — `optimizations.rs:188`

```rust
pub fn resolve_launch_directives_for_method(
    enabled_option_ids: &[String],
    resolved_method: &str,
) -> Result<LaunchDirectives, ValidationError>
```

- Generic version not tied to a full `LaunchRequest`; used internally and by `build_steam_launch_options_command()`

#### `build_steam_launch_options_command()` — `optimizations.rs:288`

```rust
pub fn build_steam_launch_options_command(
    enabled_option_ids: &[String]
) -> Result<String, ValidationError>
```

- **Returns**: `"PROTON_NO_STEAMINPUT=1 PROTON_ENABLE_HDR=1 mangohud %command%"` or just `"%command%"`
- **Behavior**: Calls `resolve_launch_directives_for_method()` internally
- **Preview impact**: Can be called directly for steam_applaunch previews

#### `LaunchRequest.resolved_method()` — `request.rs:74`

```rust
pub fn resolved_method(&self) -> &str
```

- Auto-detection: empty method → checks app_id (steam_applaunch), checks .exe extension (proton_run), else native

#### `resolve_wine_prefix_path()` — `runtime_helpers.rs:94`

```rust
pub fn resolve_wine_prefix_path(prefix_path: &Path) -> PathBuf
```

- If path ends with "pfx" → return as-is; if path/pfx is a directory → return path/pfx; else return path

#### `resolve_steam_client_install_path()` — `runtime_helpers.rs:157`

```rust
pub fn resolve_steam_client_install_path(configured_path: &str) -> Option<String>
```

- Cascade: configured → `$STEAM_COMPAT_CLIENT_INSTALL_PATH` env → `~/.local/share/Steam` → `~/.steam/root` → Flatpak path

### Current Launch Flow (`src-tauri/src/commands/launch.rs`)

```rust
// Existing Tauri commands:
#[tauri::command]
pub fn validate_launch(request: LaunchRequest) -> Result<(), LaunchValidationIssue>

#[tauri::command]
pub fn build_steam_launch_options_command(enabled_option_ids: Vec<String>) -> Result<String, String>

#[tauri::command]
pub async fn launch_game(app: AppHandle, request: LaunchRequest) -> Result<LaunchResult, String>

#[tauri::command]
pub async fn launch_trainer(app: AppHandle, request: LaunchRequest) -> Result<LaunchResult, String>
```

The `launch_game` flow:

1. Mutates request: `launch_game_only = true`
2. `validate(&request)` — fail-fast
3. `create_log_path()` — filesystem mutation
4. Matches on `resolved_method()` to build the right `Command`
5. `command.spawn()` — process creation
6. `spawn_log_stream()` — async log tailing

Preview reuses steps 1-2 and the method matching logic from step 4, but stops before any filesystem mutation or process creation.

### Frontend Launch Flow (`LaunchPage.tsx` + `useLaunchState.ts`)

1. `LaunchPage` builds `LaunchRequest` via `buildLaunchRequest(profile, method, steamClientInstallPath)`
2. Returns `null` if `game.executable_path` is empty
3. Passes `LaunchRequest` to `LaunchPanel` component
4. `useLaunchState` hook manages the reducer:
   - `Idle` → user clicks Launch Game → `GameLaunching`
   - Calls `validateLaunchRequest()` via IPC → if fails, dispatches `failure` with validation feedback
   - Calls `invoke("launch_game")` → if succeeds, moves to `WaitingForTrainer` (two-step) or `SessionActive` (native)
5. For two-step: user clicks Launch Trainer → `TrainerLaunching` → `SessionActive`

### Environment Variable Architecture (`env.rs`)

| Constant                       | Count | Purpose                                                              |
| ------------------------------ | ----- | -------------------------------------------------------------------- |
| `WINE_ENV_VARS_TO_CLEAR`       | 31    | Cleared before trainer launch to prevent host-session bleed          |
| `REQUIRED_PROTON_VARS`         | 3     | STEAM_COMPAT_DATA_PATH, STEAM_COMPAT_CLIENT_INSTALL_PATH, WINEPREFIX |
| `LAUNCH_OPTIMIZATION_ENV_VARS` | 14    | Vars that optimization toggles can set (PROTON_NO_STEAMINPUT, etc.)  |
| `PASSTHROUGH_DISPLAY_VARS`     | 4     | DISPLAY, WAYLAND_DISPLAY, XDG_RUNTIME_DIR, DBUS_SESSION_BUS_ADDRESS  |

### Optimization Definitions (`optimizations.rs`)

17 optimization definitions in `LAUNCH_OPTIMIZATION_DEFINITIONS`, each with:

- `id: &str` — matches frontend `LAUNCH_OPTIMIZATION_IDS`
- `applies_to_method: &str` — all currently `proton_run`
- `env: &[(&str, &str)]` — environment variable pairs to set
- `wrappers: &[&str]` — command wrappers to prepend (mangohud, gamemoderun, game-performance)
- `conflicts_with: &[&str]` — mutually exclusive optimization IDs
- `required_binary: Option<&str>` — binary that must exist on PATH

---

## Success Criteria

1. **Completeness**: Preview shows all data that influences the launch outcome — method, paths, environment, wrappers, validation status
2. **Accuracy**: Preview exactly matches what the actual launch would produce (same functions, same resolution logic)
3. **Read-only safety**: Preview execution produces zero filesystem mutations and zero process spawns
4. **Exhaustive validation**: Preview shows ALL validation issues, not just the first
5. **Method-appropriate content**: Each launch method (native, proton_run, steam_applaunch) shows only relevant sections
6. **Gamepad navigable**: Preview works with controller navigation on Steam Deck
7. **Copyable output**: Preview can be exported as structured text for troubleshooting

---

## Open Questions

1. **Exhaustive validation strategy**: Should we add a new `validate_all()` function that collects all errors, or modify the existing `validate()` to accept a collection mode parameter? The former preserves backwards compatibility; the latter avoids code duplication.

2. **Environment snapshot depth**: Should preview show only CrossHook-managed environment variables, or also include the host passthrough values (HOME, PATH, DISPLAY)? Including them is more complete but potentially noisy.

3. **CopyToPrefix path prediction**: For `trainer_loading_mode = "copy_to_prefix"`, should preview compute the staged path (pure string manipulation) or note that staging will occur without showing the path? The computation is trivial (`C:\CrossHook\StagedTrainers\{stem}\{filename}`) but the actual staging involves file copies.

4. **Preview staleness**: Should the preview auto-refresh when profile data changes, or require manual re-trigger? Auto-refresh avoids stale data but may cause performance issues on Steam Deck. Manual re-trigger is simpler but risks users acting on outdated previews.

5. **Scope of "collect all validation"**: The validate functions are nested (e.g., `validate_proton_run` calls `resolve_launch_directives` which itself validates). Should the exhaustive collector recurse into these sub-validators, or only collect top-level errors?

6. **Preview for both launch steps**: Should preview show a single unified view (both game and trainer), or tabbed/toggled views for each step? A unified view is more informative but could be overwhelming.
