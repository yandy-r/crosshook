# Profile Health Dashboard — Code Analysis

## Executive Summary

The existing codebase provides all the primitive building blocks for profile health validation without changes to public APIs. Private path-checking helpers in `request.rs` need visibility promotion to `pub(crate)`; `ProfileStore::list()`/`load()` already support batch iteration; and the `crosshook-status-chip` CSS pattern in `CompatibilityViewer` is a drop-in for `HealthBadge`. New code consists of one Rust module (`profile/health.rs`), two Tauri commands added to the existing `profile.rs`, one React hook, one badge component, and TypeScript types. `sanitize_display_path()` is **already in `shared.rs`** — the move is complete, do not repeat it.

---

## Existing Code Structure

### Rust Backend

| File                                              | Role                                                                                                                                                               |
| ------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| `crates/crosshook-core/src/launch/request.rs`     | `ValidationError` enum, `validate_all()`, private `require_directory()`, `require_executable_file()`, `is_executable_file()` — path-checking primitives to promote |
| `crates/crosshook-core/src/profile/models.rs`     | `GameProfile` + section structs (`GameSection`, `TrainerSection`, `InjectionSection`, `SteamSection`, `RuntimeSection`) — all path fields                          |
| `crates/crosshook-core/src/profile/toml_store.rs` | `ProfileStore::list()`, `load()`, `with_base_path()` — batch iteration and test harness                                                                            |
| `crates/crosshook-core/src/profile/mod.rs`        | Module root — **add `pub mod health;` here**                                                                                                                       |
| `crates/crosshook-core/src/lib.rs`                | Crate root — health types exported through `profile::health`                                                                                                       |
| `src-tauri/src/commands/profile.rs`               | All profile CRUD Tauri commands — **add health commands here**                                                                                                     |
| `src-tauri/src/commands/shared.rs`                | `sanitize_display_path()` at line 20 — **already here**, import via `use super::shared::sanitize_display_path`                                                     |
| `src-tauri/src/lib.rs`                            | `invoke_handler!` flat list at line 85 — register new commands here; async spawn pattern at lines 61–71                                                            |

### TypeScript Frontend

| File                                     | Role                                                                                               |
| ---------------------------------------- | -------------------------------------------------------------------------------------------------- |
| `src/hooks/useLaunchState.ts`            | `useReducer` + typed action union pattern — exact model for `useProfileHealth`                     |
| `src/types/launch.ts`                    | `LaunchValidationIssue`, `LaunchFeedback` discriminated union — pattern for health types           |
| `src/types/index.ts`                     | Barrel re-exports — **add `export * from './health'`**                                             |
| `src/components/CompatibilityViewer.tsx` | `crosshook-status-chip crosshook-compatibility-badge--{rating}` badge pattern                      |
| `src/components/pages/ProfilesPage.tsx`  | Profile list integration point                                                                     |
| `src/styles/variables.css`               | Color tokens: `--crosshook-color-success`, `--crosshook-color-warning`, `--crosshook-color-danger` |

---

## Implementation Patterns

### 1. Path-Checking Primitives (Rust)

Three private helpers in `request.rs` (lines 700–756) must be promoted to `pub(crate)` for reuse in `profile/health.rs`:

```rust
// request.rs — change `fn` to `pub(crate) fn` for all three

pub(crate) fn require_directory<'a>(
    value: &'a str,
    required_error: ValidationError,
    missing_error: ValidationError,
    not_directory_error: ValidationError,
) -> Result<&'a Path, ValidationError>

pub(crate) fn require_executable_file(
    value: &str,
    required_error: ValidationError,
    missing_error: ValidationError,
    not_executable_error: ValidationError,
) -> Result<(), ValidationError>

pub(crate) fn is_executable_file(path: &Path) -> bool  // mode & 0o111 on Unix
```

The health module calls these helpers directly against `GameProfile` path fields via `std::fs::metadata()`. No `LaunchRequest` construction is needed (Option B from feature-spec).

### 2. ValidationError → LaunchValidationIssue Pipeline

```rust
// ValidationError produces a LaunchValidationIssue via .issue()
impl ValidationError {
    pub fn issue(&self) -> LaunchValidationIssue {
        LaunchValidationIssue {
            message: self.message(),
            help: self.help(),
            severity: self.severity(),  // currently always Fatal
        }
    }
}
```

All `ValidationError` variants today return `ValidationSeverity::Fatal` (line 431). Health validation needs `Warning` and `Info` severities for optional fields. The new `HealthIssueKind` enum provides machine-readable classification alongside the existing `message`/`help`/`severity` structure.

`ValidationSeverity` is `#[derive(Serialize, Deserialize)]` with `#[serde(rename_all = "snake_case")]` — it crosses the IPC boundary already.

### 3. GameProfile Path Fields — Complete Inventory

```
profile.game.executable_path          → String  (require file exists)
profile.trainer.path                  → String  (require file exists if non-empty)
profile.injection.dll_paths           → Vec<String>  (iterate all, require file for each non-empty)
profile.steam.compatdata_path         → String  (require directory if steam.enabled)
profile.steam.proton_path             → String  (require executable if steam.enabled)
profile.steam.launcher.icon_path      → String  (optional, warn if non-empty and missing)
profile.runtime.prefix_path           → String  (require directory if non-empty)
profile.runtime.proton_path           → String  (require executable if non-empty)
profile.runtime.working_directory     → String  (optional, warn if non-empty and missing)
```

Key: `injection.dll_paths` is a `Vec<String>` — must iterate all entries, not just index 0.

### 4. ProfileStore Batch Iteration

```rust
// Per-profile error isolation — the CORRECT pattern
let names = store.list()?;  // Vec<String>
let mut results = HashMap::new();
for name in &names {
    match store.load(name) {
        Ok(profile) => {
            results.insert(name.clone(), validate_profile_health(name, &profile));
        }
        Err(e) => {
            results.insert(name.clone(), ProfileHealthSummary::load_error(e.to_string()));
        }
    }
}

// WRONG — propagates with ? and kills the batch on one bad TOML
for name in &names {
    let profile = store.load(name)?;  // aborts entire batch
}
```

`ProfileStore::with_base_path(tmp.path().join("profiles"))` is the test harness entry point — tests always follow `tempfile::tempdir()` + this constructor.

### 5. Dual-Store Tauri Command Pattern (from profile.rs)

```rust
// The canonical dual-store pattern for health commands
#[tauri::command]
pub fn batch_validate_profiles(
    store: State<'_, ProfileStore>,
    metadata_store: State<'_, MetadataStore>,
) -> Result<HashMap<String, ProfileHealthSummary>, String> {
    // 1. Filesystem validation via ProfileStore (always runs)
    let names = store.list().map_err(map_error)?;
    let mut results = HashMap::new();
    for name in &names {
        match store.load(name) {
            Ok(profile) => results.insert(name.clone(), health::validate_profile_health(name, &profile)),
            Err(e) => results.insert(name.clone(), ProfileHealthSummary::load_error(e.to_string())),
        };
    }

    // 2. Metadata enrichment (fail-soft, non-blocking)
    let failure_trends = metadata_store.query_failure_trends(30).unwrap_or_default();
    let last_successes = metadata_store.query_last_success_per_profile().unwrap_or_default();
    // enrich results...

    Ok(results)
}

fn map_error(error: ProfileStoreError) -> String {
    error.to_string()
}
```

The `map_error` helper (line 13 of `profile.rs`) is already present in the file — reuse it, do not redefine.

### 6. MetadataStore Fail-Soft Pattern

```rust
// with_conn() internals — returns T::default() when unavailable or poisoned
fn with_conn<F, T>(&self, action: &'static str, f: F) -> Result<T, MetadataStoreError>
where
    F: FnOnce(&Connection) -> Result<T, MetadataStoreError>,
    T: Default,  // <-- required bound; Vec<_>, HashMap<_>, Option<_> all satisfy it
{
    if !self.available {
        return Ok(T::default());
    }
    // ...
}
```

Health commands use `unwrap_or_default()` on enrichment results — no explicit `available` checks needed:

```rust
let trends = metadata_store.query_failure_trends(30).unwrap_or_default();
let successes = metadata_store.query_last_success_per_profile().unwrap_or_default();
```

**Key signatures** (from metadata/mod.rs lines 401–483):

- `query_last_success_per_profile() -> Result<Vec<(String, String)>, MetadataStoreError>` — `(profile_name, finished_at_iso)` pairs
- `query_failure_trends(days: u32) -> Result<Vec<FailureTrendRow>, MetadataStoreError>` — only profiles with `failures > 0`

`FailureTrendRow` fields (from `metadata/models.rs`): `profile_name: String`, `successes: i64`, `failures: i64`, `failure_modes: Option<String>`.

### 7. sanitize_display_path() — Already in shared.rs

```rust
// src-tauri/src/commands/shared.rs line 20 — ALREADY HERE
pub fn sanitize_display_path(path: &str) -> String {
    match env::var("HOME") {
        Ok(home) => {
            let path = Path::new(path);
            let home = Path::new(&home);
            match path.strip_prefix(home) {
                Ok(suffix) if suffix.as_os_str().is_empty() => "~/".to_string(),
                Ok(suffix) => format!("~/{}", suffix.display()),
                Err(_) => path.display().to_string(),
            }
        }
        _ => path.to_string(),
    }
}
```

Import in `profile.rs` via `use super::shared::sanitize_display_path;`. Apply to every path string in health IPC responses before serialization.

### 8. Command Registration Pattern

```rust
// src-tauri/src/lib.rs line 85 — flat list, append new commands
.invoke_handler(tauri::generate_handler![
    // ... existing commands ...
    commands::profile::batch_validate_profiles,  // NEW
    commands::profile::get_profile_health,       // NEW
])
```

No sub-routing, no new `mod`, no new `.manage()` call needed — `ProfileStore` and `MetadataStore` are already managed at lines 76 and 80.

### 9. CompatibilityBadge — Drop-in CSS Pattern for HealthBadge

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

`HealthBadge` uses the same `crosshook-status-chip` base class with modifiers:

- `crosshook-health-badge--healthy` (maps to `working` token)
- `crosshook-health-badge--stale` (maps to `partial` token)
- `crosshook-health-badge--broken` (maps to `broken` token)

No new CSS architecture needed — add three modifier rules to an existing stylesheet.

### 10. useReducer + Event Listener Pattern (TypeScript)

```typescript
// hooks/useLaunchState.ts — exact model for useProfileHealth

// State shape
type LaunchState = { phase: LaunchPhase; feedback: LaunchFeedback | null; ... }

// Action union with discriminants
type LaunchAction =
  | { type: "reset" }
  | { type: "game-start" }
  | { type: "failure"; fallbackPhase: LaunchPhase; feedback: LaunchFeedback }

// Hook body
const [state, dispatch] = useReducer(reducer, initialState);

// Cleanup pattern for event listeners
useEffect(() => {
  let active = true;
  const unlisten = listen<T>("event-name", (event) => {
    if (!active) return;
    dispatch({ type: "received", payload: event.payload });
  });
  return () => {
    active = false;
    void unlisten.then((fn) => fn());
  };
}, []);
```

`useProfileHealth` follows this exactly: `idle → loading → loaded | error` states, typed action union, `invoke<ProfileHealthMap>("batch_validate_profiles", {})`.

### 11. TypeScript Type Guard Pattern

```typescript
// types/launch.ts — existing type guard to model health guards after
export function isLaunchValidationIssue(value: unknown): value is LaunchValidationIssue {
  return typeof value === 'object' && value !== null && 'message' in value && 'help' in value && 'severity' in value;
}
```

Add matching guards `isProfileHealthSummary`, `isHealthIssue` in `src/types/health.ts`.

---

## Integration Points

### Files to Create

| File                                          | Purpose                                                                                               |
| --------------------------------------------- | ----------------------------------------------------------------------------------------------------- |
| `crates/crosshook-core/src/profile/health.rs` | `validate_profile_health()`, `ProfileHealthSummary`, `HealthStatus`, `HealthIssue`, `HealthIssueKind` |
| `src/types/health.ts`                         | TypeScript types + type guards                                                                        |
| `src/hooks/useProfileHealth.ts`               | `useReducer` state machine, `invoke` calls                                                            |
| `src/components/HealthBadge.tsx`              | Inline badge using `crosshook-status-chip`                                                            |

### Files to Modify

| File                                          | Change                                                                                                               |
| --------------------------------------------- | -------------------------------------------------------------------------------------------------------------------- |
| `crates/crosshook-core/src/launch/request.rs` | Promote `require_directory`, `require_executable_file`, `is_executable_file` to `pub(crate)`                         |
| `crates/crosshook-core/src/profile/mod.rs`    | Add `pub mod health;` and re-export health types                                                                     |
| `src-tauri/src/commands/profile.rs`           | Add `batch_validate_profiles` and `get_profile_health` commands; import `sanitize_display_path` from `super::shared` |
| `src-tauri/src/lib.rs`                        | Register two new commands in `invoke_handler!` at line 85                                                            |
| `src/types/index.ts`                          | Add `export * from './health'`                                                                                       |
| `src/components/pages/ProfilesPage.tsx`       | Import `useProfileHealth`, render `HealthBadge` in profile list                                                      |

---

## Code Conventions

### Rust

- All new types that cross the IPC boundary: `#[derive(Debug, Clone, Serialize, Deserialize)]`
- Serde enums: `#[serde(rename_all = "snake_case")]`
- Optional fields: `#[serde(default)]`
- Error conversion at Tauri boundary: `.to_string()` via the existing `map_error` helper in `profile.rs` (line 13)
- Tests: `tempfile::tempdir()` + `ProfileStore::with_base_path(tmp.path().join("profiles"))` — never mock the filesystem
- `MetadataStore::open_in_memory()` for SQLite tests — runs all migrations automatically

### TypeScript

- Discriminated unions for state (`HealthStatus`) and issue types (`HealthIssue`)
- `invoke<T>('command_name', { camelCaseArgs })` — args serialize to `snake_case` Rust params
- Type guard functions follow the `isLaunchValidationIssue` pattern
- CSS classes follow BEM-like `crosshook-{component}__{element}--{modifier}` pattern

---

## Dependencies and Services

- **No new Tauri plugins needed** — health commands use `ProfileStore` and `MetadataStore` already managed in `lib.rs` at lines 76 and 80
- **No new Rust crates** — `std::fs::metadata()` is sufficient; `tempfile` (dev-dependency) already used in tests
- **`resolve_launch_method()`** from `profile/models.rs` is already `pub` — use it for method-aware validation dispatch in health checks
- **`MetadataStore::disabled()`** is the fallback when SQLite is unavailable — `with_conn()` silently returns defaults; health enrichment degrades gracefully

---

## Gotchas and Warnings

### Path-Checking Helpers are Private

`require_directory`, `require_executable_file`, and `is_executable_file` are `fn` (private) in `request.rs` at lines 700–756. Change them to `pub(crate) fn`. Do not duplicate the logic.

### All ValidationErrors are Fatal — Health Needs New Severities

`ValidationError::severity()` returns `ValidationSeverity::Fatal` unconditionally (line 431). Health validation needs `Warning` (path missing but optional field) and `Info` (empty/skipped). The new `HealthIssueKind` enum provides machine-readable classification separate from severity.

### sanitize_display_path Is Already in shared.rs (Not launch.rs)

Previous analysis drafts described this function as being in `launch.rs` and needing migration. It is **already at `src-tauri/src/commands/shared.rs` line 20** and is `pub`. Import via `use super::shared::sanitize_display_path;` in `profile.rs`. No migration needed.

### injection.dll_paths Is a Vec — Must Iterate All Entries

`profile.injection.dll_paths: Vec<String>` — validation must iterate every element with a non-empty value, not just index 0.

### Batch Command Must Isolate Per-Profile Errors

Use `match store.load(name) { Ok(p) => ..., Err(e) => ProfileHealthSummary::load_error(...) }` per iteration. Never propagate with `?` inside the batch loop — a single corrupt TOML must not abort the entire response.

### Health Validation Must NOT Be Added to startup.rs

`startup.rs` is synchronous. Do not add health validation there. The spec triggers health checks from the frontend via `invoke()`. Any startup notification follows the `tauri::async_runtime::spawn` + `app_handle.emit()` pattern already used for `auto-load-profile` in `lib.rs` lines 61–71.

### ProfilesPage Badge Integration Path

Profile list rendering uses `ProfileFormSections` with a `profileSelector` prop. Health badges in the list require either threading health data through that prop boundary or a React context. Confirm with the UI design before choosing the injection point.

### query_failure_trends Only Returns Profiles With Failures

`query_failure_trends(days)` uses `HAVING failures > 0` — it returns nothing for healthy profiles. To find the absence of failures, cross-reference the full profile name list from `store.list()` against the trends result set.

### launcher_store.rs list() Always Reports is_stale = false

`LauncherInfo.is_stale` is only meaningful when set with profile context; `list()` sets it to `false`. The health module avoids this pattern by computing status inline during validation.

---

## Task-Specific Guidance

### Phase A: Rust Core (`profile/health.rs`)

1. In `request.rs`: change `fn require_directory`, `fn require_executable_file`, `fn is_executable_file` to `pub(crate) fn`
2. Create `crates/crosshook-core/src/profile/health.rs`:
   - `pub enum HealthStatus { Healthy, Stale, Broken }`
   - `pub enum HealthIssueKind { PathMissing, PathNotFile, PathNotDirectory, PathNotExecutable, LoadError }`
   - `pub struct HealthIssue { pub kind: HealthIssueKind, pub message: String, pub help: String, pub severity: ValidationSeverity, pub path: Option<String> }`
   - `pub struct ProfileHealthSummary { pub status: HealthStatus, pub issues: Vec<HealthIssue> }`
   - `pub fn validate_profile_health(name: &str, profile: &GameProfile, method: &str) -> ProfileHealthSummary`
3. Add `pub mod health;` to `profile/mod.rs` and re-export types
4. Tests: `tempfile::tempdir()` + real file creation — test each field type (missing file, missing directory, missing exe)

### Phase B: Tauri Commands (`commands/profile.rs`)

1. Add `use super::shared::sanitize_display_path;` import
2. Add `batch_validate_profiles(store, metadata_store) -> Result<HashMap<String, ProfileHealthSummary>, String>`:
   - Iterate `store.list()`, load each profile, call `health::validate_profile_health()`
   - Enrich with `metadata_store.query_failure_trends(30).unwrap_or_default()` and `query_last_success_per_profile().unwrap_or_default()`
   - Apply `sanitize_display_path()` to all path strings in `HealthIssue.path`
3. Add `get_profile_health(name, store, metadata_store) -> Result<ProfileHealthSummary, String>` for single-profile refresh
4. Register both in `lib.rs` `invoke_handler!` at the comment block around line 128

### Phase C: TypeScript Types and Hook

1. Create `src/types/health.ts`: `HealthStatus`, `HealthIssue`, `HealthIssueKind`, `ProfileHealthSummary`, type guards
2. Add `export * from './health'` to `src/types/index.ts`
3. Create `src/hooks/useProfileHealth.ts` using `useReducer` pattern from `useLaunchState.ts`: `idle → loading → loaded | error` states

### Phase D: UI Components and Integration (if Phase D from spec)

1. Create `src/components/HealthBadge.tsx` using `crosshook-status-chip` pattern from `CompatibilityViewer.tsx:76–82`
2. CSS modifiers: `crosshook-health-badge--healthy`, `crosshook-health-badge--stale`, `crosshook-health-badge--broken`
3. Integrate into `ProfilesPage.tsx` profile list

### Phase D: Metadata Persistence (health_snapshots migration)

1. Add migration v6 in `metadata/migrations.rs` — `health_snapshots` table (one row per profile_id, UPSERT, FK cascade)
2. Add `upsert_health_snapshot(profile_id, status, checked_at)` and `get_health_snapshot(profile_id)` to `metadata/mod.rs`
3. Call `upsert_health_snapshot` from `batch_validate_profiles` and `get_profile_health` after computing results
