# Engineering Practices Research: Profile Health Dashboard

## Executive Summary

The codebase already contains all the primitives needed for a profile health dashboard with minimal new code. `validate_all()` in `request.rs` performs multi-issue path and config checking against a `LaunchRequest`; `ProfileStore::list()` + `load()` provide the batch iteration backbone; and `LauncherInfo.is_stale` demonstrates the exact staleness pattern needed. The biggest architectural risk is over-engineering: the feature can be delivered as a thin wrapper around existing validation, a new Tauri command, and a status chip list in the UI—no new crate, no new abstraction layer.

---

## Existing Reusable Code

| Module / Utility                     | Location                                                | Purpose                                                                                                                                                     | How to Reuse                                                                                                                    |
| ------------------------------------ | ------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------- | ---------------- | ---------------------------------------------------------- | ------- | --------- |
| `validate_all()`                     | `crates/crosshook-core/src/launch/request.rs:442`       | Returns `Vec<LaunchValidationIssue>` for all path and config issues in a `LaunchRequest`                                                                    | Adapt or call directly to check profile paths; already handles require/missing/not-file/not-dir for all path fields             |
| `require_directory()`                | `crates/crosshook-core/src/launch/request.rs:698`       | Private helper: required → missing → not-directory three-case check                                                                                         | Promote to `pub(crate)` or duplicate pattern (3 call sites — not yet worth extracting)                                          |
| `require_executable_file()`          | `crates/crosshook-core/src/launch/request.rs:719`       | Private helper: required → missing → not-executable three-case check                                                                                        | Same as above                                                                                                                   |
| `is_executable_file()`               | `crates/crosshook-core/src/launch/request.rs:740`       | Checks `is_file()` + Unix exec permission bit                                                                                                               | Reuse as-is if promoted to `pub(crate)`; avoids re-implementing permission check                                                |
| `ValidationSeverity`                 | `crates/crosshook-core/src/launch/request.rs:143`       | `Fatal / Warning / Info` severity enum, Serde-serialized                                                                                                    | Reuse directly as health issue severity on new types — already crosses IPC boundary                                             |
| `LaunchValidationIssue`              | `crates/crosshook-core/src/launch/request.rs:151`       | `{ message, help, severity }` — the UI-ready issue struct                                                                                                   | Reuse as the element type of `ProfileHealthIssue`; avoids inventing a parallel type                                             |
| `ProfileStore::list()`               | `crates/crosshook-core/src/profile/toml_store.rs:136`   | Returns sorted `Vec<String>` of profile names                                                                                                               | Call this to enumerate all profiles for batch health check                                                                      |
| `ProfileStore::load()`               | `crates/crosshook-core/src/profile/toml_store.rs:100`   | Loads a single `GameProfile` from disk                                                                                                                      | Call per-profile during batch scan; already handles NotFound and TOML parse errors                                              |
| `GameProfile` path fields            | `crates/crosshook-core/src/profile/models.rs`           | All path strings live in `game.executable_path`, `trainer.path`, `steam.compatdata_path`, `steam.proton_path`, `runtime.prefix_path`, `runtime.proton_path` | Map each populated field to a health check; the set is well-defined and finite                                                  |
| `LauncherInfo.is_stale`              | `crates/crosshook-core/src/export/launcher_store.rs:42` | Boolean stale flag on the existing launcher info struct                                                                                                     | Exact pattern to adopt: add `is_healthy / health_issues` to a new `ProfileHealthInfo` struct                                    |
| `CompatibilityBadge`                 | `src/components/CompatibilityViewer.tsx:76`             | `crosshook-status-chip crosshook-compatibility-badge--{rating}` chip                                                                                        | Reuse the CSS class pattern; a `ProfileHealthBadge` just needs `healthy / stale / broken` variants added to the same stylesheet |
| `useLaunchState` reducer pattern     | `src/hooks/useLaunchState.ts:46`                        | `useReducer` + typed actions for async state machine                                                                                                        | Copy the `pending / loading / error / success` slice pattern for the new `useProfileHealth` hook                                |
| `LaunchFeedback` discriminated union | `src/types/launch.ts:43`                                | `kind: 'validation'                                                                                                                                         | 'diagnostic'                                                                                                                    | 'runtime'` union | Model `ProfileHealthStatus` the same way: `kind: 'healthy' | 'stale' | 'broken'` |
| `ProfileStore::with_base_path()`     | `crates/crosshook-core/src/profile/toml_store.rs:96`    | Constructs store with an arbitrary path                                                                                                                     | Critical for testing: create a `tempfile::tempdir()` store in unit tests, no mocking needed                                     |

---

## Modularity Design

### Recommended module boundaries

**Rust backend**

```
crates/crosshook-core/src/profile/
    health.rs          ← NEW: ProfileHealthInfo, check_profile_health(), batch_check_health()
```

Do **not** create a top-level `health/` module. Health checking is profile data + path stat calls—it belongs in the `profile` module alongside `toml_store.rs`. It depends on `profile::GameProfile`, `launch::LaunchValidationIssue`, and `launch::ValidationSeverity`; all are already re-exported from `crosshook_core::launch`.

**Tauri command layer**

Add one command to `src-tauri/src/commands/profile.rs`:

```rust
#[tauri::command]
pub fn check_profiles_health(state: State<ProfileStore>) -> Result<Vec<ProfileHealthInfo>, String>
```

No new command file needed—this sits naturally next to the other profile commands (`list_profiles`, `load_profile`, etc.).

**Frontend**

```
src/hooks/useProfileHealth.ts    ← NEW: batch-fetch hook, mirrors useLaunchState structure
src/components/ProfileHealthBadge.tsx  ← optional, only if badge is used in 2+ places
```

The health table can live directly in a new section of `App.tsx` or as a dedicated panel component. Do not create a whole new tab unless the feature warrants it.

### Shared vs. feature-specific

- **Shared (leave alone):** `LaunchValidationIssue`, `ValidationSeverity`, `ProfileStore`, CSS chip classes
- **Feature-specific (new):** `ProfileHealthInfo` struct, `check_profile_health()` function, `batch_check_health()` function, `useProfileHealth` hook, `ProfileHealthBadge` component

---

## KISS Assessment

### Current proposal vs. simpler alternative

| Approach                                                                        | Complexity | Value delivered                       | Verdict                      |
| ------------------------------------------------------------------------------- | ---------- | ------------------------------------- | ---------------------------- |
| Full dashboard tab: table, per-profile drill-down, remediation wizard           | High       | High                                  | Over-engineered for v1       |
| Inline health chip on profile list + expandable issue list                      | Medium     | High                                  | **Recommended**              |
| Extend the existing launch-time validation message to show at profile list time | Low        | Medium                                | Viable MVP, no new component |
| New `health/` top-level crate module                                            | High       | Zero beyond a single `health.rs` file | Over-engineered              |

### Over-engineering risks

1. **New severity enum:** `ValidationSeverity` is already `Fatal/Warning/Info` and already crosses the IPC boundary. Inventing a parallel `HealthStatus` enum adds mapping code for no gain—just reuse the existing type for individual issues, and derive the top-level `status: healthy | stale | broken` from whether any Fatal or Warning issues exist.

2. **Remediation wizard:** Actionable help text is already present in every `LaunchValidationIssue.help` string (e.g., "Re-browse to the current executable"). Surface these strings directly—no need to build a separate remediation UI.

3. **Streaming/background health check:** For typical profile counts (< 50), a synchronous `invoke` that checks all profiles in one shot is simpler and sufficient. A streaming approach adds complexity only if profiling shows >200ms latency.

---

## Abstraction vs. Repetition

### What to extract

- **`require_directory()` and `require_executable_file()`**: Already private helpers in `request.rs` with a clear signature. If `health.rs` needs the same logic, the cleanest move is to promote them to `pub(crate)` in a sibling `crates/crosshook-core/src/launch/path_checks.rs` or directly in `request.rs`. This hits the rule-of-three: `validate_steam_applaunch`, `validate_proton_run`, and the new health checker would all call them—three call sites justifies making them pub(crate).

- **`is_executable_file()`**: Same justification. Promote to `pub(crate)`.

### What to repeat (three lines > abstraction)

- The per-profile dispatch logic (game path → trainer path → steam paths → runtime paths): this is naturally a flat sequence of `if !path.is_empty()` checks on `GameProfile`. Do **not** abstract this into a trait or macro. The logic is read once and never needs to be polymorphic.

- CSS badge class construction: just inline `crosshook-status-chip crosshook-health-badge--{status}` in the JSX. No utility function needed for 1–2 call sites.

---

## Interface Design

### Rust public API (`crates/crosshook-core/src/profile/health.rs`)

```rust
// Mirroring LauncherInfo.is_stale pattern
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfileHealthInfo {
    pub name: String,
    pub status: ProfileHealthStatus,     // "healthy" | "stale" | "broken"
    pub issues: Vec<LaunchValidationIssue>,  // reuse existing type
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProfileHealthStatus {
    Healthy,
    Stale,   // path issues (files missing/moved)
    Broken,  // parse failures or required field missing
}

pub fn check_profile_health(name: &str, profile: &GameProfile) -> ProfileHealthInfo { ... }
pub fn batch_check_health(store: &ProfileStore) -> Vec<ProfileHealthInfo> { ... }
```

### Tauri IPC surface

One new command: `check_profiles_health() -> Result<Vec<ProfileHealthInfo>, String>`

This keeps the contract minimal: the frontend fetches the whole batch at once, no pagination needed for typical profile counts.

### TypeScript mirror

```typescript
// src/types/profile.ts additions
export type ProfileHealthStatus = 'healthy' | 'stale' | 'broken';

export interface ProfileHealthIssue {
  message: string;
  help: string;
  severity: LaunchValidationSeverity; // reuse existing type
}

export interface ProfileHealthInfo {
  name: string;
  status: ProfileHealthStatus;
  issues: ProfileHealthIssue[];
}
```

### Extension points

- `batch_check_health` can accumulate `ProfileStoreError` cases (e.g., unparseable TOML) as a `broken` entry rather than propagating the error—this is the right pattern for a dashboard that shows all profiles.
- The health status can be cached on the frontend and refreshed on demand (button) or on profile list focus, without any Rust-side caching needed.

---

## Testability Patterns

### Recommended patterns (from existing test evidence)

**`tempfile::tempdir()` + `ProfileStore::with_base_path()`**: The most important test tool in the codebase. Every `toml_store.rs` test uses this pattern. Health check tests should do the same—create a real temp directory, write real `.toml` files with controlled content, and call `batch_check_health(&store)`. No mocking required.

```rust
#[test]
fn healthy_profile_reports_no_issues() {
    let temp_dir = tempdir().unwrap();
    let store = ProfileStore::with_base_path(temp_dir.path().join("profiles"));
    store.save("game", &complete_profile_fixture()).unwrap();
    // Write actual fixture files to temp_dir so path checks pass
    let results = batch_check_health(&store);
    assert_eq!(results[0].status, ProfileHealthStatus::Healthy);
}
```

**Test fixture helpers**: `toml_store.rs` defines a `sample_profile()` helper function inline. Follow the same pattern—one `fn healthy_profile_fixture() -> GameProfile` and one `fn profile_with_missing_paths() -> GameProfile` for health tests.

**Filesystem fixture approach for "file exists" cases**: Write actual empty files at the paths referenced by the profile fixture to the temp dir. This is what the existing import tests do (e.g., `std::fs::write(&legacy_path, ...)`). Do not mock `std::fs`.

### Anti-patterns to avoid

- **Mocking `std::fs` or `std::path::Path`**: Not used anywhere in the codebase. The `with_base_path()` constructor exists specifically to avoid needing it. Don't introduce `mockall` or similar for this.
- **Testing via the Tauri command layer**: Test `check_profile_health()` and `batch_check_health()` as plain Rust functions, not through Tauri's invoke machinery.
- **Asserting on `help` text strings**: The help text in `LaunchValidationIssue` is long prose. Assert on `severity` and `status` enum values; use `contains()` on message strings only for the most critical assertions.

---

## Build vs. Depend

| Capability                                 | Build custom                                   | Use library                           | Recommendation                                                                                         |
| ------------------------------------------ | ---------------------------------------------- | ------------------------------------- | ------------------------------------------------------------------------------------------------------ |
| File existence / is_file / is_dir checks   | `std::fs`, `std::path::Path`                   | —                                     | **Build** (stdlib, already used throughout)                                                            |
| Execute permission bit                     | `std::os::unix::fs::PermissionsExt`            | —                                     | **Build** (already done in `is_executable_file()`)                                                     |
| Temporary directories in tests             | —                                              | `tempfile` (already a dev-dependency) | **Use existing** — `tempfile` is already in `Cargo.toml`                                               |
| Parallel batch health checks               | `rayon`                                        | —                                     | **Defer** — not needed unless profiling shows latency. Sequential iteration is simpler and sufficient. |
| Path canonicalization / symlink resolution | `std::fs::canonicalize`                        | —                                     | **Build if needed** — do not add a dependency for this                                                 |
| TOML parsing errors as health issues       | Already handled by `ProfileStoreError::TomlDe` | —                                     | **Reuse existing** error types                                                                         |

**Verdict:** No new crates needed. `std::fs`, `std::path`, existing `tempfile` dev-dep, and existing `crosshook-core` types cover everything.

---

## Additional Utility Consolidation

**`sanitize_display_path()` in `src-tauri/src/commands/launch.rs:301`** replaces `$HOME` with `~` for display. The health dashboard Tauri command should call this on path strings in `LaunchValidationIssue.message` before returning over IPC — same as `sanitize_diagnostic_report()` does for launch results. Move this function to `src-tauri/src/commands/shared.rs` (not `crosshook-core` — it's a display concern, not business logic).

**`severityIcon()` in `LaunchPanel.tsx`** maps `LaunchValidationSeverity` to a single Unicode character. If the health dashboard component also needs this mapping, extract it to `src/utils/severity.ts`. At 2 call sites it crosses the justified-deduplication threshold for a 5-line lookup function.

**`isStale()` in `LaunchPanel.tsx:119` — do NOT reuse for profile health.** Verified: this is a 60-second threshold for preview staleness (`generatedAt` timestamp on `LaunchPreview`). The threshold is wrong for profile health (days/weeks, not seconds), and the concept is unrelated. The `is_stale: bool` on `LauncherInfo` is a separate backend-computed field, also unrelated. The health dashboard hook should define its own constant if staleness timing is needed: `const STALE_THRESHOLD_MS = 7 * 24 * 60 * 60 * 1000`.

**`DiagnosticCollector` pattern does NOT exist in the codebase.** `launch/diagnostics/mod.rs` uses a plain `analyze()` function returning a struct directly — no collector object. Health checking should follow the same plain-function style.

---

## Health Validation Architecture (Revised — No `LaunchRequest` Conversion)

**Scope update from tech-designer:** The health checker operates directly on `GameProfile` — no `GameProfile → LaunchRequest` conversion. This keeps health checking as a pure profile-domain concern and eliminates the conversion complexity.

```rust
// profile/health.rs
pub fn check_profile_health(name: &str, profile: &GameProfile) -> ProfileHealthInfo
pub fn batch_check_health(store: &ProfileStore) -> Vec<ProfileHealthInfo>
```

**Duplication risk — the critical engineering constraint:**

`check_profile_health()` will perform the same path-existence checks as `validate_all()` in `request.rs`, but operating on `GameProfile` fields directly. Without care, this becomes a near-duplicate that diverges over time.

**How to avoid it:** The path-checking logic is already factored into private helpers — `require_directory()`, `require_executable_file()`, `is_executable_file()`. Promoting these to `pub(crate)` lets both `validate_all()` and `check_profile_health()` call the same functions. The `ValidationError` variants and their `.issue()` / `message()` / `help()` text are equally reusable — `check_profile_health()` should construct `ValidationError` variants and call `.issue()` to get `LaunchValidationIssue`s, not invent new message strings.

**Method-dispatch still applies:** Call `resolve_launch_method(profile)` first and check only the paths relevant to the resolved method — same logic as `validate_all()`'s dispatch. Don't check Proton paths on a native profile.

**Module dependency note (verified):** `launch/request.rs` already imports `use crate::profile::TrainerLoadingMode` at line 9 — so a `launch → profile` type dependency already exists. The `profile/health.rs → launch::validate_all()` call would formalize the existing `profile → launch` direction, not create a new cross-module edge. Do not widen the dependency further by adding `GameProfile` as a parameter to launch functions.

> **Not async:** `batch_check_health()` should be synchronous. For <50 profiles the stat calls complete in single-digit milliseconds. Use `JoinSet`/`spawn_blocking` only if profiling reveals actual latency.

---

## Open Questions

1. **Should `batch_check_health` skip or include profiles that fail to parse (TOML error)?** Recommendation: include them as `broken` entries with a synthetic `LaunchValidationIssue` describing the parse error—this makes the dashboard useful for diagnosing corrupt profiles.

2. **Which paths in `GameProfile` should be health-checked?** Not all paths are always required (e.g., `steam.proton_path` is only needed for `steam_applaunch`). The health checker should respect the same method-conditional logic as `validate_all()`—derive the method from `resolve_launch_method(profile)` first, then check only the paths relevant to that method.

3. **How does health refresh interact with profile save?** The simplest answer: frontend calls `check_profiles_health` on mount and on explicit refresh. No event-driven invalidation needed for v1.

4. **Should the health status be shown inline in the existing profile list or as a separate view?** Inline chips (one per profile row) with a collapsible detail panel are the lowest-friction approach and reuse the existing `CollapsibleSection` component pattern already used in `CompatibilityViewer.tsx` and `LaunchPanel.tsx`.

5. **`LaunchValidationIssue` vs new `HealthValidationIssue`?** Do not create a new type. The `help` field already contains remediation guidance; `ValidationError` variant names are the implicit field discriminant. If a `field: Option<String>` tag is truly needed by the UI, add it to `LaunchValidationIssue` rather than duplicating the type hierarchy.
