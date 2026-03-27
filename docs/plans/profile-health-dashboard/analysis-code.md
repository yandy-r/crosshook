# Profile Health Dashboard — Code Analysis

## Executive Summary

The existing codebase provides all the primitive building blocks for profile health validation without changes to public APIs. Private path-checking helpers in `request.rs` need visibility promotion to `pub(crate)`; `ProfileStore::list()`/`load()` already support batch iteration; and the `crosshook-status-chip` CSS pattern in `CompatibilityViewer` is a drop-in for `HealthBadge`. New code consists of one Rust module (`profile/health.rs`), two Tauri commands in the existing `profile.rs`, one React hook, one badge component, and TypeScript types.

---

## Existing Code Structure

### Rust Backend

| File                                                 | Role                                                                                                                                                |
| ---------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------- |
| `crates/crosshook-core/src/launch/request.rs`        | `ValidationError` enum, `validate_all()`, `require_directory()`, `require_executable_file()`, `is_executable_file()` — all path-checking primitives |
| `crates/crosshook-core/src/profile/models.rs`        | `GameProfile` + all section structs; defines every path field to validate                                                                           |
| `crates/crosshook-core/src/profile/toml_store.rs`    | `ProfileStore::list()`, `load()`, `with_base_path()` — batch iteration + test harness                                                               |
| `crates/crosshook-core/src/profile/mod.rs`           | Module root — **add `pub mod health;` here**                                                                                                        |
| `crates/crosshook-core/src/lib.rs`                   | Crate root — health types exported through `profile::health`                                                                                        |
| `crates/crosshook-core/src/export/launcher_store.rs` | `LauncherInfo.is_stale` — analogous staleness flag pattern                                                                                          |
| `src-tauri/src/commands/profile.rs`                  | All profile CRUD Tauri commands — **add health commands here**                                                                                      |
| `src-tauri/src/commands/launch.rs`                   | `sanitize_display_path()` at line 301 — **must move to `shared.rs`** before health commands can use it                                              |
| `src-tauri/src/commands/shared.rs`                   | Already has `create_log_path`, `slugify_target` — **`sanitize_display_path()` migrates here**                                                       |
| `src-tauri/src/lib.rs`                               | `invoke_handler!` flat list at line 70 — **register new commands here**; async task spawn pattern at lines 46–56 is how health triggers too         |

### TypeScript Frontend

| File                                     | Role                                                                                             |
| ---------------------------------------- | ------------------------------------------------------------------------------------------------ |
| `src/hooks/useLaunchState.ts`            | `useReducer` + typed action union pattern — model for `useProfileHealth`                         |
| `src/types/launch.ts`                    | `LaunchValidationIssue`, `LaunchFeedback` discriminated union — pattern for health types         |
| `src/types/index.ts`                     | Barrel re-exports — **add `export * from './health'`**                                           |
| `src/components/CompatibilityViewer.tsx` | `crosshook-status-chip crosshook-compatibility-badge--{rating}` badge pattern                    |
| `src/components/pages/ProfilesPage.tsx`  | Profile list integration point; consumes `ProfileContext` + `ProfileFormSections`                |
| `src/styles/variables.css`               | Color tokens: `--crosshook-color-success` `--crosshook-color-warning` `--crosshook-color-danger` |

---

## Implementation Patterns

### 1. Path-Checking Primitives (Rust)

The three private helpers in `request.rs` must be promoted to `pub(crate)` for reuse in the new `profile/health.rs`:

```rust
// request.rs lines 698–756 (currently private — promote to pub(crate))
fn require_directory<'a>(
    value: &'a str,
    required_error: ValidationError,
    missing_error: ValidationError,
    not_directory_error: ValidationError,
) -> Result<&'a Path, ValidationError>

fn require_executable_file(
    value: &str,
    required_error: ValidationError,
    missing_error: ValidationError,
    not_executable_error: ValidationError,
) -> Result<(), ValidationError>

fn is_executable_file(path: &Path) -> bool  // uses mode & 0o111 on Unix
```

Health validation uses `std::fs::metadata()` directly via these helpers rather than constructing a `LaunchRequest`.

### 2. ValidationError → LaunchValidationIssue Pipeline

```rust
// All ValidationError variants produce Fatal severity today
pub fn severity(&self) -> ValidationSeverity {
    ValidationSeverity::Fatal  // line 430 — health module needs Warning/Info variants
}

// Conversion pattern
let issue: LaunchValidationIssue = some_validation_error.issue();
// issue = { message: String, help: String, severity: ValidationSeverity }
```

The health module will introduce a new `HealthIssueKind` enum for machine-readable classification alongside the existing `message`/`help`/`severity` structure. `ValidationSeverity` is already `Serialize/Deserialize` with `serde(rename_all = "snake_case")`.

### 3. ProfileStore Batch Iteration (Option B: Direct GameProfile Validation)

The feature-spec chose **Option B** — validate `GameProfile` path fields directly via `std::fs::metadata()`, without constructing a `LaunchRequest`. Do not analyze the `to_launch_request()` conversion path.

```rust
// toml_store.rs — batch pattern used for health validation
let names = store.list()?;        // Vec<String>, sorted alphabetically
for name in &names {
    let profile = store.load(name)?;
    // validate profile.game.executable_path, profile.trainer.path, etc.
    // using std::fs::metadata() directly — NOT via LaunchRequest conversion
}
```

`ProfileStore::with_base_path()` is the test harness entry point — all health module tests follow the `tempfile::tempdir()` + `with_base_path(tmp.path().join("profiles"))` convention.

### 4. Tauri Command Pattern

```rust
// commands/profile.rs — exact pattern for new health commands
#[tauri::command]
pub fn profile_list(store: State<'_, ProfileStore>) -> Result<Vec<String>, String> {
    store.list().map_err(map_error)
}

fn map_error(error: ProfileStoreError) -> String {
    error.to_string()
}
```

New health commands follow identical structure: `store: State<'_, ProfileStore>`, return `Result<T, String>`, errors stringified via `.to_string()`. No new `State` is needed — health validation reads through the existing `ProfileStore`.

### 5. sanitize_display_path() — Must Migrate to shared.rs First

```rust
// commands/launch.rs line 301 — currently private to launch.rs, must move to shared.rs
fn sanitize_display_path(path: &str) -> String {
    match env::var("HOME") {
        Ok(home) if path.starts_with(&home) => format!("~{}", &path[home.len()..]),
        _ => path.to_string(),
    }
}
```

`commands/shared.rs` already exists with `create_log_path` and `slugify_target`. **`sanitize_display_path()` must be moved (not duplicated) to `shared.rs`** and re-imported in `launch.rs`. Health commands in `profile.rs` then import it from `super::shared`. Every path string in health IPC responses must pass through it before serialization.

### 5b. Per-Profile Error Isolation in Batch Loop

```rust
// CORRECT — per-profile isolation: one broken profile does not abort the batch
let mut results = HashMap::new();
for name in store.list()? {
    match store.load(&name) {
        Ok(profile) => {
            results.insert(name, validate_profile_health(&name, &profile, method));
        }
        Err(e) => {
            results.insert(name, ProfileHealthSummary::load_error(e.to_string()));
        }
    }
}

// WRONG — propagates with ? and aborts on first load failure
for name in store.list()? {
    let profile = store.load(&name)?;  // ← kills the entire batch on any error
    ...
}
```

`ProfileStoreError` must be caught per-profile using `match` or `.unwrap_or_else`. The batch command returns a result for every profile regardless of individual load failures.

### 6. Command Registration Pattern

```rust
// src-tauri/src/lib.rs line 70 — flat list, append new commands here
.invoke_handler(tauri::generate_handler![
    // ... existing commands ...
    commands::profile::batch_validate_profiles,  // NEW
    commands::profile::get_profile_health,       // NEW
])
```

The new commands are registered in the same flat `invoke_handler!` macro — no sub-routing or module nesting.

### 7. LauncherInfo.is_stale — Analogous Pattern

```rust
// export/launcher_store.rs line 42
pub struct LauncherInfo {
    pub is_stale: bool,  // set from profile context; list() always sets false
    // ... other fields
}
```

`ProfileHealthSummary.status: HealthStatus` follows the same pattern — derived with profile context, serialized with `#[serde(default)]`.

### 8. useReducer Async State Hook (TypeScript)

```typescript
// hooks/useLaunchState.ts — model for useProfileHealth
type LaunchState = { phase: LaunchPhase; feedback: LaunchFeedback | null; ... }
type LaunchAction = { type: "reset" } | { type: "game-start" } | { type: "failure"; ... }

const [state, dispatch] = useReducer(reducer, initialState);

// Async invoke pattern
const result = await invoke<T>("command_name", { args });
dispatch({ type: "success", result });
```

`useProfileHealth` mirrors this exactly: `idle → loading → loaded | error` states, typed action union, `invoke<ProfileHealthMap>("batch_validate_profiles", {})`.

### 9. CompatibilityBadge — Drop-in Pattern for HealthBadge

```tsx
// CompatibilityViewer.tsx lines 76–82
function CompatibilityBadge({ rating }: { rating: CompatibilityRating }) {
  return (
    <span className={`crosshook-status-chip crosshook-compatibility-badge crosshook-compatibility-badge--${rating}`}>
      {getCompatibilityRatingLabel(rating)}
    </span>
  );
}
```

`HealthBadge` uses the same `crosshook-status-chip` base class with `crosshook-health-badge--healthy`, `crosshook-health-badge--stale`, `crosshook-health-badge--broken` modifiers and the same three CSS color tokens.

### 10. LaunchValidationIssue TypeScript Type

```typescript
// types/launch.ts lines 37–41
export interface LaunchValidationIssue {
  message: string;
  help: string;
  severity: LaunchValidationSeverity; // 'fatal' | 'warning' | 'info'
}
```

`HealthIssue` in `src/types/health.ts` follows this shape but adds `kind: HealthIssueKind` for machine-readable classification and optional `path?: string` for display.

---

## Integration Points

### Files to Create

| File                                          | Purpose                                                                                                                 |
| --------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------- |
| `crates/crosshook-core/src/profile/health.rs` | New Rust module — `validate_profile_health()`, `ProfileHealthSummary`, `HealthStatus`, `HealthIssue`, `HealthIssueKind` |
| `src/types/health.ts`                         | TypeScript types — `ProfileHealthSummary`, `HealthStatus`, `HealthIssue`, `HealthIssueKind`                             |
| `src/hooks/useProfileHealth.ts`               | React hook — `useReducer` state machine calling `batch_validate_profiles` / `get_profile_health`                        |
| `src/components/HealthBadge.tsx`              | Inline badge component using `crosshook-status-chip` pattern                                                            |

### Files to Modify

| File                                          | Change                                                                                                           |
| --------------------------------------------- | ---------------------------------------------------------------------------------------------------------------- |
| `crates/crosshook-core/src/launch/request.rs` | Promote `require_directory`, `require_executable_file`, `is_executable_file` to `pub(crate)`                     |
| `crates/crosshook-core/src/profile/mod.rs`    | Add `pub mod health;` and re-export health types                                                                 |
| `src-tauri/src/commands/profile.rs`           | Add `batch_validate_profiles` and `get_profile_health` Tauri commands                                            |
| `src-tauri/src/lib.rs`                        | Register the two new commands in `invoke_handler!`                                                               |
| `src/types/index.ts`                          | Add `export * from './health'`                                                                                   |
| `src/components/pages/ProfilesPage.tsx`       | Import `useProfileHealth`, render `HealthBadge` inline in profile list via `ProfileFormSections.profileSelector` |

---

## Code Conventions

### Rust

- All new types that cross the IPC boundary need `#[derive(Debug, Clone, Serialize, Deserialize)]`
- Serde enums use `#[serde(rename_all = "snake_case")]`
- Optional fields use `#[serde(default)]`
- Errors convert to `String` via `.to_string()` at the Tauri boundary using the `map_error` helper pattern
- Tests use `tempfile::tempdir()` + `ProfileStore::with_base_path(tmp.path())` — never mock the filesystem

### TypeScript

- Discriminated unions for state (`HealthStatus`) and feedback (`HealthIssue`)
- `invoke<T>('command_name', { camelCaseArgs })` — args serialize to `snake_case` Rust params via Tauri
- Type guard functions (`isHealthSummary(value): value is ProfileHealthSummary`) follow `isLaunchValidationIssue` pattern from `types/launch.ts:48`
- CSS classes follow BEM-like `crosshook-{component}__{element}--{modifier}` pattern

---

## Dependencies and Services

- **No new Tauri plugins needed** — health commands use existing `ProfileStore` state already managed in `lib.rs`
- **No new Rust crates** — `std::fs::metadata()` is sufficient; `tempfile` (dev) already used in `toml_store.rs` tests
- **`crosshook-core`** is a library crate; the new `profile/health.rs` module lives there and is consumed by `src-tauri`
- **`resolve_launch_method()`** from `profile/models.rs` is needed for method-aware validation dispatch in health checks — already `pub` and re-exported

---

## Gotchas and Warnings

### Path-Checking Helpers are Private

`require_directory`, `require_executable_file`, and `is_executable_file` are private functions in `request.rs`. They must be promoted to `pub(crate)` before the health module can use them. Do not duplicate the logic.

### All ValidationErrors are Fatal — Health Needs New Severities

`ValidationError::severity()` returns `ValidationSeverity::Fatal` unconditionally (line 430). Health validation needs `Warning` (path missing but optional field) and `Info` (empty/skipped field) severities. The new `HealthIssueKind` enum provides machine-readable classification separate from severity.

### isStale in LaunchPanel Must NOT Be Reused

`LaunchPanel.tsx` has a local `isStale()` check based on a 60-second preview age threshold. This is unrelated to profile path health and must not be confused with `HealthStatus.stale`.

### Health Check Must NOT Be Added to startup.rs

`src-tauri/src/startup.rs` is a hard constraint — synchronous, do not modify it. Health validation is async I/O triggered only from the frontend via `invoke()`. If a startup health emit is ever needed, it mirrors the `tauri::async_runtime::spawn` pattern in `lib.rs:47-56` (the existing `auto-load-profile` emit) — but the current spec defers to frontend-only triggers.

### ProfilesPage Uses ProfileFormSections for Profile List Rendering

The profile selector list is rendered inside `ProfileFormSections` via the `profileSelector` prop (not directly in `ProfilesPage`). Health badges in the profile list require passing health data through this prop boundary or via a separate React context.

### Injection DLL Paths are a Vec

`injection.dll_paths: Vec<String>` — health validation must iterate all entries, not just check the first. Each non-empty DLL path should be validated as an existing file.

### Path Display Must Be Sanitized Before IPC

Any path returned in a health issue must pass through `sanitize_display_path()` (replaces `$HOME` with `~`) before serialization. This applies to the optional `path` field in `HealthIssue`. Failure to sanitize is a medium-severity security finding.

### launcher_store.rs list() Always Reports is_stale = false

The analogous `LauncherInfo.is_stale` is documented as only meaningful when derived with profile context (see launcher_store.rs line 22 comment). The health module avoids this pitfall by always computing status inline during validation.

---

## Task-Specific Guidance

### Phase 1: Rust Core (`profile/health.rs`)

1. Promote `require_directory`, `require_executable_file`, `is_executable_file` in `request.rs` to `pub(crate)`
2. Create `profile/health.rs` with `HealthStatus` enum, `HealthIssue` struct, `HealthIssueKind` enum, `ProfileHealthSummary` struct, `validate_profile_health(name, profile, method) -> ProfileHealthSummary`
3. Add `pub mod health;` to `profile/mod.rs` and re-export from `profile::health`
4. Write tests using `tempfile` + real file creation (not mocks)

### Phase 2: Tauri Commands (`commands/profile.rs`)

1. Add `batch_validate_profiles(store) -> Result<HashMap<String, ProfileHealthSummary>, String>` — iterates `store.list()` + `store.load()` per name
2. Add `get_profile_health(name, store) -> Result<ProfileHealthSummary, String>` — single profile check
3. Apply `sanitize_display_path()` to all path strings in returned summaries
4. Register both in `lib.rs` `invoke_handler!`

### Phase 3: TypeScript Types and Hook

1. Create `src/types/health.ts` with `HealthStatus`, `HealthIssue`, `HealthIssueKind`, `ProfileHealthSummary` interfaces and type guard
2. Add `export * from './health'` to `src/types/index.ts`
3. Create `src/hooks/useProfileHealth.ts` using `useReducer` pattern from `useLaunchState.ts`

### Phase 4: UI Components and Integration

1. Create `src/components/HealthBadge.tsx` using `crosshook-status-chip` pattern
2. Integrate into `ProfilesPage.tsx` / `ProfileFormSections` profile selector
3. Add CSS for `crosshook-health-badge--{healthy,stale,broken}` modifiers using existing color tokens
