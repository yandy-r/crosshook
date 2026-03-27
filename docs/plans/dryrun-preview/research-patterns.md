# Dry Run / Preview — Codebase Patterns Research

## Overview

This document catalogs concrete coding patterns, conventions, and architectural approaches found in the CrossHook codebase that the dry-run preview implementation must follow. Each section provides file paths, code snippets, and rationale so implementors can match existing conventions exactly.

## 1. Tauri Command Patterns

### Command Structure (`src-tauri/src/commands/launch.rs`)

All Tauri commands follow a consistent pattern:

1. **Function signature**: `#[tauri::command]` attribute, snake_case name, typed parameters, returns `Result<T, String>`
2. **Wrapping core functions**: Tauri commands are thin wrappers — they call a `crosshook-core` function and map errors to `String`
3. **Error mapping**: `.map_err(|error| error.to_string())` is the universal error conversion

```rust
// Sync command — simplest pattern (this is what preview_launch should follow)
#[tauri::command]
pub fn validate_launch(request: LaunchRequest) -> Result<(), LaunchValidationIssue> {
    validate(&request).map_err(|error| error.issue())
}

// Sync command with primitive input
#[tauri::command]
pub fn build_steam_launch_options_command(
    enabled_option_ids: Vec<String>,
) -> Result<String, String> {
    build_steam_launch_options_command_core(&enabled_option_ids).map_err(|error| error.to_string())
}

// Async command with AppHandle (preview does NOT need this pattern)
#[tauri::command]
pub async fn launch_game(app: AppHandle, request: LaunchRequest) -> Result<LaunchResult, String> {
    // ...
}
```

**Pattern for `preview_launch`**: Use the sync thin-wrapper pattern like `validate_launch` and `build_steam_launch_options_command`, NOT the async pattern used by `launch_game`/`launch_trainer`. Preview is a pure computation with no IO-heavy operations.

```rust
// Target pattern:
#[tauri::command]
pub fn preview_launch(request: LaunchRequest) -> Result<LaunchPreview, String> {
    build_launch_preview(&request).map_err(|error| error.to_string())
}
```

### State Access Pattern (`src-tauri/src/commands/profile.rs`)

Commands that need managed state use `State<'_, T>`:

```rust
#[tauri::command]
pub fn profile_load(name: String, store: State<'_, ProfileStore>) -> Result<GameProfile, String> {
    store.load(&name).map_err(map_error)
}
```

**`preview_launch` does NOT need managed state** — it takes only a `LaunchRequest` as input.

### Command Registration (`src-tauri/src/lib.rs:70-109`)

Commands are registered in `tauri::generate_handler![]` grouped by module. Launch commands are at lines 87-90:

```rust
commands::launch::launch_game,
commands::launch::launch_trainer,
commands::launch::validate_launch,
commands::launch::build_steam_launch_options_command,
```

**Add** `commands::launch::preview_launch` to this group.

### Import Pattern (`src-tauri/src/commands/launch.rs:4-12`)

Core types are imported from `crosshook_core::launch::*` with aliasing for name conflicts:

```rust
use crosshook_core::launch::{
    build_steam_launch_options_command as build_steam_launch_options_command_core,
    // ...
    validate, LaunchRequest, LaunchValidationIssue, METHOD_NATIVE, METHOD_PROTON_RUN,
    METHOD_STEAM_APPLAUNCH,
};
```

### Error Conversion Patterns

Two patterns exist:

| Pattern                                 | Used When                               | Example                                                          |
| --------------------------------------- | --------------------------------------- | ---------------------------------------------------------------- |
| `.map_err(\|error\| error.to_string())` | Generic errors → String                 | `build_steam_launch_options_command`, `launch_game`              |
| `map_error` helper function             | Repeated type-specific mapping          | `profile.rs`: `fn map_error(error: ProfileStoreError) -> String` |
| `.map_err(\|error\| error.issue())`     | ValidationError → LaunchValidationIssue | `validate_launch`                                                |

**For `preview_launch`**: Use `.map_err(|error| error.to_string())` since the command returns `Result<LaunchPreview, String>`.

## 2. Serde Serialization Patterns

### Struct Serialization

All IPC structs derive `Serialize` (and usually `Deserialize`). Field naming uses `snake_case` by default — there is **no** `rename_all` on structs; Rust's native `snake_case` fields match TypeScript expectations directly.

```rust
// request.rs — input type (needs both Serialize and Deserialize)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct LaunchRequest {
    #[serde(default)]
    pub method: String,
    // ...
}

// launch.rs — output-only type (Serialize only)
#[derive(Debug, Clone, Serialize)]
pub struct LaunchResult {
    pub succeeded: bool,
    pub message: String,
    pub helper_log_path: String,
}
```

**Key pattern**: Input types (received from frontend) use `Serialize + Deserialize + Default` with `#[serde(default)]` on every field. Output-only types use `Serialize` only.

### Enum Serialization (`request.rs:143-148`)

Enums use `rename_all = "snake_case"`:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ValidationSeverity {
    Fatal,
    Warning,
    Info,
}
```

This maps to TypeScript string unions: `'fatal' | 'warning' | 'info'`.

**`EnvVarSource` must follow this exact pattern** — `#[serde(rename_all = "snake_case")]` on the enum.

### `LaunchDirectives` Current State (`optimizations.rs:17-27`)

`LaunchDirectives` currently lacks `Serialize`/`Deserialize`:

```rust
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct LaunchDirectives {
    pub env: Vec<(String, String)>,
    pub wrappers: Vec<String>,
}
```

**Feature spec confirms**: Add `#[derive(Serialize, Deserialize)]` — one-line change, unblocks preview usage.

### `LaunchValidationIssue` (request.rs:151-156)

Already has full serialization:

```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LaunchValidationIssue {
    pub message: String,
    pub help: String,
    pub severity: ValidationSeverity,
}
```

## 3. React Hook Patterns

### Reducer Pattern (`useLaunchState.ts`)

The launch hook uses `useReducer` for complex state machines:

```typescript
type LaunchState = {
  phase: LaunchPhase;
  feedback: LaunchFeedback | null;
  helperLogPath: string | null;
};

type LaunchAction =
  | { type: "reset" }
  | { type: "game-start" }
  | { type: "game-success"; helperLogPath: string; nextPhase: LaunchPhase }
  // ...

function reducer(state: LaunchState, action: LaunchAction): LaunchState {
  switch (action.type) {
    case "reset":
      return initialState;
    // ...
  }
}

export function useLaunchState({ ... }: UseLaunchStateArgs) {
  const [state, dispatch] = useReducer(reducer, initialState);
  // ...
}
```

**For `usePreviewState`**: The preview hook has simpler state (loading, result, error) — use `useState` like `useProfile.ts` rather than `useReducer`. The feature spec explicitly decouples preview from launch state.

### Simple Hook Pattern (`useProfile.ts`)

Uses individual `useState` hooks for each piece of state:

```typescript
export function useProfile(options: UseProfileOptions = {}): UseProfileResult {
  const [profiles, setProfiles] = useState<string[]>([]);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  // ...
}
```

### Invoke Pattern (Both hooks)

Tauri `invoke()` calls follow this pattern:

```typescript
import { invoke } from '@tauri-apps/api/core';

// Simple invoke with typed result
const result = await invoke<LaunchResult>('launch_game', { request: launchRequest });

// Invoke that may return a Tauri error (string or structured)
try {
  await invoke<void>('validate_launch', { request });
} catch (error) {
  if (isLaunchValidationIssue(error)) {
    return error;
  }
  throw error;
}
```

**For `usePreviewState`**: Follow the simple pattern:

```typescript
const preview = await invoke<LaunchPreview>('preview_launch', { request });
```

### Error Normalization Pattern

Both hooks normalize errors consistently:

```typescript
function normalizeRuntimeError(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

// In useProfile:
catch (err) {
  const message = err instanceof Error ? err.message : String(err);
  setError(message);
}
```

### Hook Return Shape

Hooks return flat objects (not arrays), with clear property names:

```typescript
return {
  actionLabel,
  canLaunchGame,
  isBusy,
  launchGame,
  phase,
  feedback: state.feedback,
};
```

## 4. Validation Patterns

### `validate()` — Fail-Fast Pattern (`request.rs:442-454`)

Current validation dispatches by method, returning `Err` on first failure:

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

### `ValidationError` — Rich Enum Pattern (`request.rs:158-199`)

Each variant provides three methods: `message()`, `help()`, `severity()`. All current variants return `ValidationSeverity::Fatal`:

```rust
impl ValidationError {
    pub fn issue(&self) -> LaunchValidationIssue {
        LaunchValidationIssue {
            message: self.message(),
            help: self.help(),
            severity: self.severity(),
        }
    }

    pub fn severity(&self) -> ValidationSeverity {
        ValidationSeverity::Fatal  // all current variants are Fatal
    }
}
```

### `validate_all()` — New Collector Pattern

The feature spec requires a `validate_all()` that collects **all** issues instead of short-circuiting. It should follow the existing dispatch structure but use `Vec<LaunchValidationIssue>` as a collector:

```rust
// Proposed pattern (mirrors existing validate() dispatch):
pub fn validate_all(request: &LaunchRequest) -> Vec<LaunchValidationIssue> {
    let mut issues = Vec::new();

    // Method validation
    match request.method.trim() {
        "" | METHOD_STEAM_APPLAUNCH | METHOD_PROTON_RUN | METHOD_NATIVE => {}
        value => issues.push(ValidationError::UnsupportedMethod(value.to_string()).issue()),
    }

    // Dispatch to method-specific collectors
    match request.resolved_method() {
        METHOD_STEAM_APPLAUNCH => collect_steam_issues(request, &mut issues),
        METHOD_PROTON_RUN => collect_proton_issues(request, &mut issues),
        METHOD_NATIVE => collect_native_issues(request, &mut issues),
        _ => {}
    }

    issues
}
```

### Helper Validation Patterns

Individual checks follow a pattern of testing → returning specific error:

```rust
fn require_game_path_if_needed(request: &LaunchRequest, must_exist: bool) -> Result<(), ValidationError> {
    if request.launch_trainer_only { return Ok(()); }
    let game_path = request.game_path.trim();
    if game_path.is_empty() { return Err(ValidationError::GamePathRequired); }
    if must_exist {
        let path = Path::new(game_path);
        if !path.exists() { return Err(ValidationError::GamePathMissing); }
        if !path.is_file() { return Err(ValidationError::GamePathNotFile); }
    }
    Ok(())
}
```

For `validate_all()`, these helpers should be adapted to push to a `Vec` instead of returning `Err`.

## 5. UI Component Patterns

### `CollapsibleSection` (`src/components/ui/CollapsibleSection.tsx`)

Native `<details>`/`<summary>` with optional controlled state:

```typescript
export interface CollapsibleSectionProps {
  title: string;
  defaultOpen?: boolean; // uncontrolled default
  open?: boolean; // controlled mode (overrides defaultOpen)
  onToggle?: (nextOpen: boolean) => void;
  meta?: ReactNode; // right-aligned metadata slot
  className?: string;
  children: ReactNode;
}
```

**CSS classes**:

- Root: `crosshook-collapsible` (a `<details>` element)
- Summary: `crosshook-collapsible__summary`
- Chevron: `crosshook-collapsible__chevron` (CSS triangle rotated 90° on open)
- Title: `crosshook-collapsible__title` (uppercase, small, accent color)
- Meta: `crosshook-collapsible__meta` (muted, right-aligned)
- Body: `crosshook-collapsible__body`

**Usage pattern**: Widely used for section grouping (LaunchPage, InstallPage, ProfilesPage, SettingsPanel, CommunityBrowser, LaunchOptimizationsPanel).

### `ProfileReviewModal` (`src/components/ProfileReviewModal.tsx`)

Full modal dialog with portal rendering, focus trap, and gamepad support:

```typescript
export interface ProfileReviewModalProps {
  open: boolean;
  title: string;
  statusLabel: string;
  profileName: string;
  executablePath: string;
  prefixPath: string;
  helperLogPath: string;
  children: ReactNode;
  footer?: ReactNode;
  description?: string;
  onClose: () => void;
  allowBackdropDismiss?: boolean;
  closeLabel?: string;
  initialFocusRef?: RefObject<HTMLElement | null>;
  statusTone?: ProfileReviewModalStatusTone;
  className?: string;
  confirmation?: ProfileReviewModalConfirmation | null;
}
```

**Key infrastructure to reuse/adapt for preview modal**:

1. **Portal rendering**: `createPortal()` into a dynamically created `div.crosshook-modal-portal`
2. **Focus trap**: Manual `Tab` key interception with `getFocusableElements()` cycling
3. **Escape key**: Closes modal (or cancels confirmation overlay)
4. **Backdrop dismiss**: `onMouseDown` on `crosshook-modal__backdrop` with `allowBackdropDismiss` toggle
5. **Accessibility**: `role="dialog"`, `aria-modal="true"`, `aria-labelledby`, `aria-describedby`
6. **Inert sibling nodes**: All other body children set to `inert` + `aria-hidden="true"` while modal is open
7. **Focus restoration**: Returns focus to previously focused element on close
8. **Body scroll lock**: `body.style.overflow = 'hidden'` + `body.classList.add('crosshook-modal-open')`

**Modal CSS classes**:

- `crosshook-modal` (wrapper)
- `crosshook-modal__backdrop`
- `crosshook-modal__surface` + `crosshook-panel` + `crosshook-focus-scope`
- `crosshook-modal__header`, `crosshook-modal__title`, `crosshook-modal__description`
- `crosshook-modal__status-chip` with tone modifiers (`--success`, `--warning`, `--danger`, `--neutral`)
- `crosshook-modal__body`, `crosshook-modal__footer`

### `useGamepadNav` Integration (`src/hooks/useGamepadNav.ts`)

Modal detection is automatic via `data-crosshook-focus-root="modal"`:

```typescript
const MODAL_FOCUS_ROOT_SELECTOR = '[data-crosshook-focus-root="modal"]';

function getNavigationRoot(rootRef) {
  const modalRoots = document.querySelectorAll(MODAL_FOCUS_ROOT_SELECTOR);
  return modalRoots.item(modalRoots.length - 1) ?? getRootElement(rootRef);
}
```

**For preview modal**: Set `data-crosshook-focus-root="modal"` on the modal surface element (already done via `ProfileReviewModal` convention).

### Button Styling (`src/styles/theme.css`)

```css
/* Primary (default) — blue gradient accent */
.crosshook-button { ... }

/* Secondary — subtle transparent background */
.crosshook-button--secondary { ... }

/* Ghost — fully transparent, muted text */
.crosshook-button--ghost { ... }

/* Warning — amber tint */
.crosshook-button--warning { ... }

/* Danger — red tint */
.crosshook-button--danger { ... }
```

All buttons use `min-height: var(--crosshook-touch-target-min)` (48px for Steam Deck).

**For preview**: "Preview Launch" button → `crosshook-button--ghost` or `crosshook-button--secondary`. "Launch Now" → `crosshook-button` (primary). "Close" → `crosshook-button--ghost`.

### `LaunchPanel` Component Pattern (`src/components/LaunchPanel.tsx`)

The launch panel follows a prop-driven pattern with hook consumption:

```typescript
interface LaunchPanelProps {
  profileId: string;
  method: Exclude<LaunchMethod, ''>;
  request: LaunchRequest | null;
}

export function LaunchPanel({ profileId, method, request }: LaunchPanelProps) {
  const { ... } = useLaunchState({ profileId, method, request });
  // ...
}
```

**BEM-like CSS naming**: `crosshook-launch-panel__header`, `crosshook-launch-panel__actions`, etc.

**The preview button should be added to** `crosshook-launch-panel__actions` alongside the existing Launch and Reset buttons.

## 6. Module & Re-export Patterns

### Core Module Organization (`crates/crosshook-core/src/launch/mod.rs`)

New modules are declared and selectively re-exported:

```rust
pub mod env;
pub mod optimizations;
pub mod request;
pub mod runtime_helpers;
pub mod script_runner;
#[cfg(test)]
pub(crate) mod test_support;

pub use optimizations::{
    build_steam_launch_options_command, is_known_launch_optimization_id,
    resolve_launch_directives, resolve_launch_directives_for_method, LaunchDirectives,
};
pub use request::{
    validate, LaunchRequest, LaunchValidationIssue, /* ... */
};
```

**Add for preview**:

```rust
pub mod preview;

pub use preview::{build_launch_preview, LaunchPreview};
```

### TypeScript Type Re-exports (`src/types/index.ts`)

All type modules are barrel-exported:

```typescript
export * from './profile';
export * from './profile-review';
export * from './launch';
export * from './launcher';
export * from './install';
export * from './update';
export * from './settings';
```

**New preview types go in `src/types/launch.ts`** alongside existing launch types — no new file needed for types.

## 7. Testing Patterns

### Test Module Structure (`request.rs:655-955`)

Tests live in a `#[cfg(test)] mod tests` at the bottom of the same file:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    // Test fixture struct
    struct RequestFixture {
        _temp_dir: tempfile::TempDir,
        game_path: String,
        // ...
    }

    // Factory functions for fixtures
    fn fixture() -> RequestFixture { ... }
    fn steam_request() -> (tempfile::TempDir, LaunchRequest) { ... }
    fn proton_request() -> (tempfile::TempDir, LaunchRequest) { ... }
    fn native_request() -> (tempfile::TempDir, LaunchRequest) { ... }
}
```

### Key Testing Patterns

1. **TempDir ownership**: `_temp_dir` returned alongside the request to keep temp files alive

2. **Executable file creation**: Helper function with platform-aware chmod:

   ```rust
   fn write_executable_file(path: &Path) {
       fs::write(path, b"test").expect("write file");
       #[cfg(unix)]
       {
           use std::os::unix::fs::PermissionsExt;
           let mut permissions = fs::metadata(path).expect("metadata").permissions();
           permissions.set_mode(0o755);
           fs::set_permissions(path, permissions).expect("chmod");
       }
   }
   ```

3. **Assertions**: `assert_eq!` for exact match, typically comparing `Result` values:

   ```rust
   assert_eq!(validate(&request), Ok(()));
   assert_eq!(validate(&request), Err(ValidationError::GamePathRequired));
   ```

4. **Test naming**: `snake_case` descriptive names: `validates_steam_applaunch_request`, `proton_run_rejects_unknown_launch_optimization`

5. **Scoped command search path** (`test_support.rs`): `ScopedCommandSearchPath` RAII guard overrides `is_command_available()` for tests that check binary availability:

   ```rust
   let temp_dir = tempfile::tempdir().expect("temp dir");
   let _command_search_path =
       crate::launch::test_support::ScopedCommandSearchPath::new(temp_dir.path());
   ```

### Test Patterns for `build_launch_preview()` and `validate_all()`

Follow the existing fixture pattern:

```rust
#[test]
fn preview_shows_resolved_method_for_steam_applaunch() {
    let (_temp_dir, request) = steam_request();
    let preview = build_launch_preview(&request).expect("preview");
    assert_eq!(preview.resolved_method, "steam_applaunch");
}

#[test]
fn validate_all_collects_multiple_issues() {
    let (_temp_dir, mut request) = steam_request();
    request.steam.app_id.clear();
    request.steam.compatdata_path.clear();
    let issues = validate_all(&request);
    assert!(issues.len() >= 2, "expected multiple issues, got {}", issues.len());
}
```

## 8. Environment & Runtime Patterns

### Environment Variable Constants (`launch/env.rs`)

All env var lists are declared as `pub const` slices:

```rust
pub const WINE_ENV_VARS_TO_CLEAR: &[&str] = &["WINESERVER", "WINELOADER", ...]; // 31 vars
pub const REQUIRED_PROTON_VARS: &[&str] = &["STEAM_COMPAT_DATA_PATH", ...];     // 3 vars
pub const LAUNCH_OPTIMIZATION_ENV_VARS: &[&str] = &["PROTON_NO_STEAMINPUT", ...]; // 14 vars
pub const PASSTHROUGH_DISPLAY_VARS: &[&str] = &["DISPLAY", ...];                  // 4 vars
```

**Preview needs to reference these** for collecting env vars with source tags. The `EnvVarSource` enum maps to these groups:

- `ProtonRuntime` → `REQUIRED_PROTON_VARS`
- `LaunchOptimization` → from `LaunchDirectives.env`
- `Host` → from `apply_host_environment()` pattern in `runtime_helpers.rs`
- `SteamProton` → Steam-specific vars set by helper scripts

### Runtime Helpers (`launch/runtime_helpers.rs`)

Environment setup uses `Command`-mutating functions. Preview **must not** call these directly (they modify `Command` objects). Instead, preview should compute the same values using pure functions:

```rust
// runtime_helpers.rs uses:
pub fn apply_host_environment(command: &mut Command) { ... }
pub fn apply_runtime_proton_environment(command: &mut Command, prefix_path: &str, ...) { ... }

// Preview should compute equivalent values without a Command:
// e.g., resolve_wine_prefix_path(prefix), resolve_compat_data_path(prefix, wine_prefix)
```

The existing `resolve_wine_prefix_path()` and `resolve_compat_data_path()` functions in `runtime_helpers.rs` are already `pub` and can be called from `preview.rs`.

## 9. CSS & Design System Conventions

### CSS Custom Properties (`src/styles/variables.css`)

| Variable                          | Value           | Usage                 |
| --------------------------------- | --------------- | --------------------- |
| `--crosshook-color-bg`            | `#1a1a2e`       | Page background       |
| `--crosshook-color-surface`       | `#12172a`       | Panel surfaces        |
| `--crosshook-color-accent`        | `#0078d4`       | Primary action        |
| `--crosshook-color-accent-strong` | `#2da3ff`       | Active/focused accent |
| `--crosshook-color-success`       | `#28c76f`       | Pass indicators       |
| `--crosshook-color-warning`       | `#f5c542`       | Warning indicators    |
| `--crosshook-color-danger`        | `#ff758f`       | Error indicators      |
| `--crosshook-font-mono`           | SFMono/Consolas | Code/path display     |
| `--crosshook-touch-target-min`    | `48px`          | Minimum touch target  |
| `--crosshook-radius-md`           | `14px`          | Button/panel corners  |

### BEM-like Naming Convention

All classes follow `crosshook-{component}__{element}--{modifier}`:

```
crosshook-launch-panel           (block)
crosshook-launch-panel__header   (element)
crosshook-launch-panel__action   (element)
crosshook-launch-panel__action--secondary  (modifier)
```

### Data Attribute State Pattern

Components use `data-*` attributes for state-based styling:

```html
<div className="crosshook-launch-panel__status" data-phase={phase}>
<div className="crosshook-launch-panel__feedback" data-kind={feedback.kind} data-severity={feedbackSeverity}>
<div className="crosshook-launch-panel__indicator" data-state={isSessionActive ? 'active' : ...}>
```

**Preview validation items should use** `data-severity="fatal"`, `data-severity="warning"`, `data-severity="info"` for CSS-driven styling.

## Patterns to Follow

| Pattern              | File                                    | Convention                                                                                   |
| -------------------- | --------------------------------------- | -------------------------------------------------------------------------------------------- |
| Tauri command        | `commands/launch.rs`                    | Sync, thin wrapper, `Result<T, String>`, `.map_err(\|e\| e.to_string())`                     |
| Core function        | `launch/preview.rs`                     | New module, pure function `build_launch_preview(&LaunchRequest) -> Result<LaunchPreview, E>` |
| Struct derives       | `launch/preview.rs`                     | `#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]`                             |
| Enum serialization   | `launch/preview.rs`                     | `#[serde(rename_all = "snake_case")]`                                                        |
| Module re-export     | `launch/mod.rs`                         | `pub mod preview;` + `pub use preview::{...}`                                                |
| Command registration | `lib.rs`                                | Add to `invoke_handler` in launch group                                                      |
| TypeScript types     | `types/launch.ts`                       | Mirror Rust structs with `snake_case` fields, `Option<T>` → `T \| null`                      |
| React hook           | `hooks/usePreviewState.ts`              | `useState`-based, wrap `invoke()`, return flat object                                        |
| UI component         | `components/LaunchPanel.tsx`            | Add button + modal, use `CollapsibleSection`, `crosshook-button--ghost`                      |
| Modal pattern        | Use `ProfileReviewModal` infrastructure | Portal, focus trap, `data-crosshook-focus-root="modal"`                                      |
| Tests                | `launch/preview.rs`                     | `#[cfg(test)] mod tests`, `tempfile` fixtures, `assert_eq!`                                  |
| CSS                  | `styles/theme.css`                      | BEM naming, `data-*` state attributes, CSS custom properties                                 |
