# Coding Patterns and Conventions: Profile Health Dashboard

This document catalogs the precise patterns from the existing codebase that health dashboard
implementation must follow. Every pattern described here has a verified source location.

---

## Overview

The health dashboard sits at the intersection of four existing subsystems: `ProfileStore` (TOML
I/O), `MetadataStore` (SQLite enrichment), Tauri IPC commands, and React hooks. Each subsystem
has a well-established pattern. The health feature must replicate these patterns exactly — not
invent new ones — to remain consistent with the rest of the codebase.

---

## Relevant Files

- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/metadata/mod.rs` — MetadataStore: `with_conn`, `open_in_memory`, `disabled`, `query_failure_trends`, `query_last_success_per_profile`
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/launch/request.rs` — `ValidationSeverity`, `LaunchValidationIssue`, `ValidationError`, `validate_all`, `require_directory`, `require_executable_file`, `is_executable_file`
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src-tauri/src/commands/profile.rs` — Tauri command pattern: dual `State<'_, ProfileStore>` + `State<'_, MetadataStore>`, `map_error`, best-effort metadata sync
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/hooks/useLaunchState.ts` — React `useReducer` hook pattern with typed action discriminated union
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/components/CompatibilityViewer.tsx` — Badge/chip CSS class pattern
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/types/launch.ts` — `LaunchFeedback` discriminated union, `LaunchValidationIssue` TypeScript mirror
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src-tauri/src/lib.rs` — Tauri state management (`app.manage()`), command registration in `invoke_handler!`
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/metadata/models.rs` — `DriftState`, `FailureTrendRow`, `SyncReport` Serde patterns

---

## Architectural Patterns

### MetadataStore Fail-Soft Pattern

`with_conn()` is the canonical pattern for all metadata reads. It returns `T::default()` silently
when the store is `disabled()` (SQLite unavailable) or when `conn` is `None`. Lock poisoning
becomes a `MetadataStoreError::Corrupt`. This ensures health commands degrade to path-checks-only
mode without panicking.

```rust
// metadata/mod.rs:67
fn with_conn<F, T>(&self, action: &'static str, f: F) -> Result<T, MetadataStoreError>
where
    F: FnOnce(&Connection) -> Result<T, MetadataStoreError>,
    T: Default,
{
    if !self.available { return Ok(T::default()); }
    let Some(conn) = &self.conn else { return Ok(T::default()); };
    let guard = conn.lock().map_err(|_| MetadataStoreError::Corrupt(...))?;
    f(&guard)
}
```

**Health implication**: `query_failure_trends()` and `query_last_success_per_profile()` both go
through `with_conn`. Calling them when `MetadataStore::disabled()` is in use returns empty `Vec`s,
not errors. Health code receives `Option<&MetadataStore>` and calls these methods; the fail-soft
behaviour is transparent.

### MetadataStore Constructor Pattern

Four constructors exist, each for a distinct use case:

| Constructor        | Use case                                                                           |
| ------------------ | ---------------------------------------------------------------------------------- |
| `try_new()`        | Production: opens `~/.local/share/crosshook/metadata.db`, returns `Err` on failure |
| `with_path(path)`  | Testing: opens at explicit path, returns `Result`                                  |
| `open_in_memory()` | Tests: in-memory SQLite with all migrations applied automatically                  |
| `disabled()`       | Fallback: no-op store, all reads return `T::default()`                             |

`lib.rs:32` shows the production fallback:

```rust
let metadata_store = MetadataStore::try_new().unwrap_or_else(|error| {
    tracing::warn!(%error, "metadata store unavailable — SQLite features disabled");
    MetadataStore::disabled()
});
```

### ProfileStore TOML I/O Pattern

`ProfileStore` has a parallel constructor set. `with_base_path()` (line 96 of `toml_store.rs`) is
the test fixture constructor — pass `tempdir().path().join("profiles")` to get an isolated store
with no production data pollution.

`list()` returns sorted `Vec<String>` of profile names. `load(name)` returns `Result<GameProfile,
ProfileStoreError>`. Batch iteration is a simple loop: `list()` then `load()` per name.

### Tauri State Management Pattern

All stores are registered via `.manage()` in `lib.rs:76-81`. Commands receive stores via
`State<'_, T>` parameters.

**Dual-store command signature** (from `profile_save`, `profile_delete`, `profile_duplicate`):

```rust
#[tauri::command]
pub fn profile_save(
    name: String,
    data: GameProfile,
    store: State<'_, ProfileStore>,
    metadata_store: State<'_, MetadataStore>,
) -> Result<(), String>
```

The new `check_profiles_health` command must follow this exact signature shape. Both stores are
always passed even when the command primarily uses one — this keeps the pattern consistent and
avoids the metadata store being unavailable at the command layer.

### Metadata Sync Error Handling Pattern

Metadata operations after successful store operations use best-effort logging, not failure
propagation. The canonical form (from `profile_save`):

```rust
store.save(&name, &data).map_err(map_error)?;  // primary operation: fail hard
if let Err(e) = metadata_store.observe_profile_write(...) {  // metadata: best-effort
    tracing::warn!(%e, profile_name = %name, "metadata sync after profile_save failed");
}
Ok(())
```

Health commands are **read-only**, so they call `query_failure_trends()` and
`query_last_success_per_profile()`. These return `Result<Vec<...>, MetadataStoreError>`. If they
fail, surface the data as `None` in the health info struct — do not propagate the error to the
caller.

---

## Code Conventions

### Rust Naming and Module Organization

- Functions: `snake_case` — Tauri commands mirror their frontend `invoke()` call names exactly
- New module: `crates/crosshook-core/src/profile/health.rs` — placed in the `profile` crate
  module since it operates on `GameProfile`, not in `launch/` or `metadata/`
- New Tauri command file: `src-tauri/src/commands/health.rs` — mirroring the
  `commands/profile.rs` pattern. This keeps the health IPC separate from profile CRUD.
- Command registration: add to `invoke_handler!` list in `lib.rs`

### Serde Derive Pattern

All types that cross the IPC boundary derive `Serialize, Deserialize`. Enums use
`#[serde(rename_all = "snake_case")]`. Example from `ValidationSeverity`:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ValidationSeverity { Fatal, Warning, Info }
```

`ProfileHealthStatus` must follow the same pattern:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProfileHealthStatus { Healthy, Stale, Broken }
```

### Error Mapping in Tauri Commands

All Tauri commands use a local `map_error` function that stringifies domain errors:

```rust
fn map_error(error: ProfileStoreError) -> String { error.to_string() }
```

Health commands should follow the same: `Result<Vec<ProfileHealthInfo>, String>` return type.
`String` errors are already the contract at the IPC boundary.

### `ValidationError` → `LaunchValidationIssue` Conversion

`ValidationError::issue()` produces a `LaunchValidationIssue` from any variant. This is the
correct way to construct issues — not by constructing `LaunchValidationIssue` directly with string
literals. Health code should construct `ValidationError` variants and call `.issue()`.

Private helpers (`require_directory`, `require_executable_file`, `is_executable_file` at
`request.rs:700-758`) need to be promoted to `pub(crate)` to be callable from `profile/health.rs`
without duplicating the three-case check logic.

### Path Display Sanitization

`sanitize_display_path()` is defined at `src-tauri/src/commands/shared.rs:20`. It replaces `$HOME`
with `~` in path strings for display. Apply it to all path strings in `LaunchValidationIssue.message`
before returning over IPC — both TOML-sourced paths and any SQLite-sourced paths. Do not apply it
inside `crosshook-core`; it is a display concern owned by the Tauri command layer.

---

## Error Handling

### Result<T, String> at IPC Boundary

All Tauri commands return `Result<T, String>`. The `String` error is what the frontend receives as
a rejected `invoke()` promise. Never return domain-specific error types across the IPC boundary —
always call `.map_err(|e| e.to_string())`.

### MetadataStoreError Variants

`MetadataStoreError` variants are: `Database { action, source }`, `Corrupt(String)`,
`ProfileNotFound`, `AlreadyExists`. The `Corrupt` variant is returned when the mutex is poisoned
(lock error inside `with_conn`). Health code treating metadata as optional enrichment never needs
to match on these variants — returning `None` on any error is the correct policy.

### `std::fs::metadata()` Over `path.exists()` for Tri-State Classification

The health tri-state (`healthy` / `stale` / `broken`) requires distinguishing `NotFound` from
`PermissionDenied`. `path.exists()` swallows both as `false`. Use `std::fs::metadata(path)` and
inspect the `ErrorKind`:

- `ErrorKind::NotFound` → **Stale** (file missing from disk — normal lifecycle, e.g. game uninstalled)
- `ErrorKind::PermissionDenied` → **Broken** (file exists but inaccessible — requires user action)
- `Ok(metadata)` → check type (`.is_dir()`, `.is_file()`) and executable bit

This is the mechanism that makes the removable-media rule and permission-denied distinction in the
feature spec work. The existing `is_executable_file()` helper already uses `fs::metadata()` — the
same pattern applies to existence checks.

### Per-Item Error Isolation in Batch Operations

`batch_check_health()` must not abort when a single profile fails to load. The pattern from
`startup::run_metadata_reconciliation` and `LauncherInfo.is_stale` is to capture per-item errors
as part of the result rather than propagating them up. A profile that fails TOML parse should
produce a `broken` `ProfileHealthInfo` entry with a synthetic issue describing the parse error —
not a top-level `Err`.

---

## Testing Approach

### Filesystem Test Fixture

Every `toml_store.rs` test uses:

```rust
let temp = tempdir().expect("temp dir");
let store = ProfileStore::with_base_path(temp.path().join("profiles"));
```

Health tests must use the same pattern. `tempfile` is already a dev-dependency.

### In-Memory MetadataStore Fixture

`MetadataStore::open_in_memory()` runs all migrations automatically. Use one instance per test
case — never share across tests.

```rust
let metadata = MetadataStore::open_in_memory().unwrap();
```

When testing the metadata-absent path, pass `None` for `metadata: Option<&MetadataStore>` to
`check_profile_health()` — this exercises the fail-soft branch explicitly.

### No Mocking

The codebase has no mock infrastructure. Tests use real filesystem (tempdir) and real in-memory
SQLite. Do not introduce mocks. `ProfileStore::with_base_path()` and
`MetadataStore::open_in_memory()` are the provided test doubles.

### Seed Metadata via Typed API Only

Do not seed test data via raw SQL strings. Use `record_launch_started()` /
`record_launch_finished()` (from `launch_history.rs`) to seed `launch_operations` rows. This keeps
tests in sync with schema migrations automatically.

### Assertion Conventions

Assert on enum values (`status`, `severity`) and field presence — not on `help` or `message`
string literals. String content is UI copy and subject to change; enum variants are structural
contracts.

---

## React Hook Pattern

### useReducer State Machine (from useLaunchState)

`useLaunchState` (`src/hooks/useLaunchState.ts:46`) defines the pattern `useProfileHealth` must
follow:

```typescript
// Typed state shape
type HealthState = {
  phase: HealthPhase; // idle | loading | loaded | error
  results: ProfileHealthInfo[] | null;
  error: string | null;
};

// Typed discriminated action union
type HealthAction =
  | { type: 'reset' }
  | { type: 'fetch-start' }
  | { type: 'fetch-success'; results: ProfileHealthInfo[] }
  | { type: 'fetch-error'; error: string };

// useReducer — not useState
const [state, dispatch] = useReducer(reducer, initialState);
```

Key points from `useLaunchState`:

- `initialState` is a named constant, not an inline object
- The reducer is a pure top-level function (not defined inside the hook)
- Async operations dispatch start/success/failure actions — they do not set state directly
- The hook returns a flat object of derived values and action functions

### LaunchFeedback Discriminated Union Pattern

`LaunchFeedback` (`src/types/launch.ts:43`) uses `kind` as the discriminant:

```typescript
export type LaunchFeedback =
  | { kind: 'validation'; issue: LaunchValidationIssue }
  | { kind: 'diagnostic'; report: DiagnosticReport }
  | { kind: 'runtime'; message: string };
```

`ProfileHealthStatus` follows the same pattern: `kind: 'healthy' | 'stale' | 'broken'` on the
Rust-derived type.

---

## CSS Badge/Chip Pattern

`CompatibilityBadge` (`src/components/CompatibilityViewer.tsx:76`) defines the chip convention:

```tsx
<span className={`crosshook-status-chip crosshook-compatibility-badge crosshook-compatibility-badge--${rating}`}>
  {getCompatibilityRatingLabel(rating)}
</span>
```

Two CSS classes are always present: the base `crosshook-status-chip` and the component-specific
modifier. `ProfileHealthBadge` should follow:

```tsx
<span className={`crosshook-status-chip crosshook-health-badge crosshook-health-badge--${status}`}>
  {getHealthStatusLabel(status)}
</span>
```

CSS modifier values must be lowercase `snake_case` matching the serialized enum value from Rust:
`healthy`, `stale`, `broken`.

---

## Patterns to Follow (Summary)

| Pattern                             | Source                        | Rule                                                                                                  |
| ----------------------------------- | ----------------------------- | ----------------------------------------------------------------------------------------------------- |
| Fail-soft metadata                  | `metadata/mod.rs:67`          | `with_conn` returns `T::default()` when unavailable — health commands must tolerate `disabled()`      |
| Dual-store Tauri command            | `commands/profile.rs:100`     | Accept `State<'_, ProfileStore>` + `State<'_, MetadataStore>` even if one is not the primary consumer |
| Best-effort metadata sync           | `commands/profile.rs:109`     | Primary operations fail hard; metadata operations log warn and continue                               |
| `ValidationError.issue()`           | `request.rs:204`              | Always construct `LaunchValidationIssue` via `ValidationError::issue()`, not with string literals     |
| Per-item batch isolation            | `startup.rs` / `LauncherInfo` | A single failing profile must not abort `batch_check_health`; return `broken` entries                 |
| `tempdir` + `with_base_path`        | `toml_store.rs` tests         | All health filesystem tests use real tempdir — no mocking                                             |
| `open_in_memory` per test           | `research-practices.md:30`    | Each test gets a fresh in-memory MetadataStore; never share across test cases                         |
| `useReducer` hook structure         | `useLaunchState.ts:46`        | Named `initialState`, top-level reducer, dispatch-only async handlers                                 |
| Chip CSS convention                 | `CompatibilityViewer.tsx:76`  | Base class `crosshook-status-chip` + modifier `crosshook-health-badge--{status}`                      |
| `serde(rename_all = "snake_case")`  | `models.rs:123`               | All IPC-crossing enums use this attribute                                                             |
| `Result<T, String>` at IPC boundary | All commands                  | Domain errors are stringified before crossing; never leak `MetadataStoreError` to frontend            |
| Command registration                | `lib.rs:85`                   | Every new `#[tauri::command]` must be added to `invoke_handler!`                                      |
| `sanitize_display_path`             | `commands/shared.rs:20`       | Apply to all path strings in IPC responses before returning; display concern, not business logic      |
| `metadata()` not `exists()`         | `request.rs:742`              | Use `std::fs::metadata()` to distinguish `NotFound` (stale) from `PermissionDenied` (broken)          |
