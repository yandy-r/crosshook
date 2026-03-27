# Profile Health Dashboard — Technical Specification

## Executive Summary

The profile health dashboard adds batch validation of all saved CrossHook profiles, surfacing per-profile health status (healthy/stale/broken) with specific remediation suggestions for broken paths. The feature introduces a new `health` module in `crosshook-core` that validates persisted `GameProfile` path fields against the filesystem, a thin Tauri command layer, and a React dashboard component following existing badge and collapsible-section UI patterns. The design prioritizes synchronous simplicity (filesystem checks are fast enough for 50+ profiles) with an optional background startup scan emitted via Tauri events.

---

## Architecture Design

### Component Diagram

```
┌─────────────────────────────────────────────────────────┐
│  React Frontend                                         │
│  ┌──────────────────────┐  ┌─────────────────────────┐  │
│  │ ProfileHealthDashboard│  │ HealthBadge             │  │
│  │  (new component)      │  │  (reusable badge)       │  │
│  └──────────┬───────────┘  └─────────────────────────┘  │
│             │ invoke()                                   │
│  ┌──────────┴───────────┐                               │
│  │ useProfileHealth     │  listen("health-check-done")  │
│  │  (new hook)          │◄──────────────────────────────│
│  └──────────┬───────────┘                               │
└─────────────┼───────────────────────────────────────────┘
              │ Tauri IPC
┌─────────────┼───────────────────────────────────────────┐
│  src-tauri  │                                           │
│  ┌──────────┴───────────┐                               │
│  │ commands/health.rs   │  (new command module)         │
│  │  batch_validate_     │                               │
│  │  profiles()          │                               │
│  │  get_profile_health()│                               │
│  └──────────┬───────────┘                               │
└─────────────┼───────────────────────────────────────────┘
              │
┌─────────────┼───────────────────────────────────────────┐
│  crosshook-core                                         │
│  ┌──────────┴───────────┐  ┌─────────────────────────┐  │
│  │ health/              │  │ profile/                │  │
│  │  mod.rs              │  │  models.rs (GameProfile) │  │
│  │  models.rs           │──│  toml_store.rs (Store)   │  │
│  │  validate.rs         │  └─────────────────────────┘  │
│  └──────────────────────┘                               │
│       uses: std::path::Path::exists/is_file/is_dir      │
└─────────────────────────────────────────────────────────┘
```

### New Components

| Component                | Location                                       | Responsibility                                                                                                |
| ------------------------ | ---------------------------------------------- | ------------------------------------------------------------------------------------------------------------- |
| `health::models`         | `crates/crosshook-core/src/health/models.rs`   | Data types: `HealthStatus`, `ProfileHealthReport`, `HealthIssue`, `HealthIssueSeverity`, `HealthCheckSummary` |
| `health::validate`       | `crates/crosshook-core/src/health/validate.rs` | Core validation logic: `validate_profile_health()`, `batch_validate()`                                        |
| `health` module root     | `crates/crosshook-core/src/health/mod.rs`      | Re-exports                                                                                                    |
| `commands::health`       | `src-tauri/src/commands/health.rs`             | Tauri IPC commands                                                                                            |
| `useProfileHealth`       | `src/hooks/useProfileHealth.ts`                | React hook for health state management                                                                        |
| `ProfileHealthDashboard` | `src/components/ProfileHealthDashboard.tsx`    | Dashboard UI                                                                                                  |
| `HealthBadge`            | `src/components/HealthBadge.tsx`               | Reusable status badge (follows `CompatibilityBadge` pattern)                                                  |
| `health` types           | `src/types/health.ts`                          | TypeScript interfaces                                                                                         |

### Integration Points

1. **`crosshook-core/src/lib.rs`** — add `pub mod health;`
2. **`src-tauri/src/commands/mod.rs`** — add `pub mod health;`
3. **`src-tauri/src/lib.rs`** — register `commands::health::batch_validate_profiles` and `commands::health::get_profile_health` in `invoke_handler`
4. **`src-tauri/src/lib.rs` setup** — optionally spawn background health check after startup (like `auto-load-profile` pattern at line 46-56)
5. **`src/types/index.ts`** — add `export * from './health';`
6. **`src/App.tsx`** — integrate `ProfileHealthDashboard` (inline in profile list area or as sub-view)

---

## Data Models

### Rust Structs (`crates/crosshook-core/src/health/models.rs`)

```rust
use serde::{Deserialize, Serialize};

/// Overall health status for a profile.
///
/// - `Healthy`: all configured paths exist and pass basic checks.
/// - `Stale`: some non-critical paths are missing (e.g., icon path, optional DLL).
///   The profile may still launch but with degraded behavior.
/// - `Broken`: critical paths are missing or invalid. The profile cannot launch.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HealthStatus {
    Healthy,
    Stale,
    Broken,
}

/// Severity of an individual health issue.
///
/// Maps to the visual severity system used throughout the app
/// (`data-severity` attribute on feedback elements).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HealthIssueSeverity {
    /// Profile cannot launch. Corresponds to `HealthStatus::Broken`.
    Error,
    /// Profile may launch with degraded behavior. Corresponds to `HealthStatus::Stale`.
    Warning,
    /// Informational note (e.g., empty optional field). Does not affect status.
    Info,
}

/// A single health issue found during profile validation.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HealthIssue {
    /// The profile section and field that failed validation.
    /// Format: `"section.field"` (e.g., `"game.executable_path"`, `"steam.proton_path"`).
    pub field: String,

    /// The path value that was checked (as stored in the profile).
    /// Empty string if the field was blank.
    pub path: String,

    /// What went wrong.
    pub message: String,

    /// How to fix it.
    pub remediation: String,

    /// Severity of this issue.
    pub severity: HealthIssueSeverity,
}

/// Health report for a single profile.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProfileHealthReport {
    /// Profile name (matches the TOML filename stem).
    pub name: String,

    /// Derived overall status from the worst issue severity.
    pub status: HealthStatus,

    /// Resolved launch method for this profile (for context in the UI).
    pub launch_method: String,

    /// All issues found during validation. Empty if healthy.
    pub issues: Vec<HealthIssue>,

    /// ISO 8601 timestamp of when this check was performed.
    pub checked_at: String,
}

/// Aggregate summary of a batch health check.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HealthCheckSummary {
    /// Per-profile reports, sorted by name.
    pub profiles: Vec<ProfileHealthReport>,

    /// Count of profiles with `HealthStatus::Healthy`.
    pub healthy_count: usize,

    /// Count of profiles with `HealthStatus::Stale`.
    pub stale_count: usize,

    /// Count of profiles with `HealthStatus::Broken`.
    pub broken_count: usize,

    /// Total number of profiles checked.
    pub total_count: usize,

    /// ISO 8601 timestamp of when the batch check completed.
    pub validated_at: String,
}
```

### TypeScript Interfaces (`src/types/health.ts`)

```typescript
export type HealthStatus = 'healthy' | 'stale' | 'broken';
export type HealthIssueSeverity = 'error' | 'warning' | 'info';

export interface HealthIssue {
  field: string;
  path: string;
  message: string;
  remediation: string;
  severity: HealthIssueSeverity;
}

export interface ProfileHealthReport {
  name: string;
  status: HealthStatus;
  launch_method: string;
  issues: HealthIssue[];
  checked_at: string;
}

export interface HealthCheckSummary {
  profiles: ProfileHealthReport[];
  healthy_count: number;
  stale_count: number;
  broken_count: number;
  total_count: number;
  validated_at: string;
}
```

---

## API Design

### Tauri Commands (`src-tauri/src/commands/health.rs`)

#### `batch_validate_profiles`

Validates all saved profiles and returns an aggregate summary.

```rust
#[tauri::command]
pub fn batch_validate_profiles(
    store: State<'_, ProfileStore>,
) -> Result<HealthCheckSummary, String> {
    crosshook_core::health::batch_validate(&store).map_err(|e| e.to_string())
}
```

**Frontend invocation:**

```typescript
const summary = await invoke<HealthCheckSummary>('batch_validate_profiles');
```

**Response:** `HealthCheckSummary` (see data model above).

**Errors:** Returns stringified error if `ProfileStore::list()` fails (filesystem error). Individual profile load failures are captured as `HealthIssue` entries within the report (not command-level errors).

---

#### `get_profile_health`

Validates a single profile by name.

```rust
#[tauri::command]
pub fn get_profile_health(
    name: String,
    store: State<'_, ProfileStore>,
) -> Result<ProfileHealthReport, String> {
    let profile = store.load(&name).map_err(|e| e.to_string())?;
    Ok(crosshook_core::health::validate_profile_health(&name, &profile))
}
```

**Frontend invocation:**

```typescript
const report = await invoke<ProfileHealthReport>('get_profile_health', { name: 'MyGame' });
```

**Response:** `ProfileHealthReport` for the named profile.

**Errors:** Returns stringified `ProfileStoreError` if the profile does not exist or cannot be parsed.

---

### Core Validation Logic (`crates/crosshook-core/src/health/validate.rs`)

```rust
use std::path::Path;
use chrono::Utc; // or use manual ISO 8601 formatting to avoid new deps

use crate::profile::{GameProfile, ProfileStore, resolve_launch_method};
use super::models::*;

/// Validates a single profile's filesystem paths and returns a health report.
pub fn validate_profile_health(name: &str, profile: &GameProfile) -> ProfileHealthReport {
    let mut issues = Vec::new();
    let method = resolve_launch_method(profile);

    // --- Game executable ---
    check_file_path(
        &profile.game.executable_path,
        "game.executable_path",
        "Game executable",
        HealthIssueSeverity::Error,
        "Re-browse to the game executable or verify game files are installed.",
        &mut issues,
    );

    // --- Trainer path (host-side) ---
    // trainer.path is a WINE-mapped path, not directly checkable on host.
    // Skip filesystem check for trainer.path; it is validated at launch time.

    // --- Steam paths (only if steam-based method) ---
    if method == "steam_applaunch" {
        check_directory_path(
            &profile.steam.compatdata_path,
            "steam.compatdata_path",
            "Steam compatdata directory",
            HealthIssueSeverity::Error,
            "Launch the game through Steam once to create the compatdata directory, or use Auto-Populate.",
            &mut issues,
        );
        check_executable_path(
            &profile.steam.proton_path,
            "steam.proton_path",
            "Steam Proton executable",
            HealthIssueSeverity::Error,
            "The configured Proton version may have been removed. Re-select via Auto-Populate or browse to an installed Proton.",
            &mut issues,
        );
    }

    // --- Runtime paths (only if proton_run method) ---
    if method == "proton_run" {
        check_directory_path(
            &profile.runtime.prefix_path,
            "runtime.prefix_path",
            "WINE/Proton prefix directory",
            HealthIssueSeverity::Error,
            "Re-select the prefix directory or launch the game once to recreate it.",
            &mut issues,
        );
        check_executable_path(
            &profile.runtime.proton_path,
            "runtime.proton_path",
            "Runtime Proton executable",
            HealthIssueSeverity::Error,
            "The configured Proton version may have been removed. Re-select an installed Proton.",
            &mut issues,
        );
    }

    // --- DLL injection paths (warning severity — optional) ---
    for (i, dll_path) in profile.injection.dll_paths.iter().enumerate() {
        if dll_path.trim().is_empty() {
            continue;
        }
        check_file_path(
            dll_path,
            &format!("injection.dll_paths[{i}]"),
            &format!("DLL injection path #{}", i + 1),
            HealthIssueSeverity::Warning,
            "Remove the DLL path or re-browse to the correct file.",
            &mut issues,
        );
    }

    // --- Launcher icon path (info severity — cosmetic) ---
    if !profile.steam.launcher.icon_path.trim().is_empty() {
        check_file_path(
            &profile.steam.launcher.icon_path,
            "steam.launcher.icon_path",
            "Launcher icon",
            HealthIssueSeverity::Info,
            "Remove the icon path or browse to a new icon image.",
            &mut issues,
        );
    }

    // --- Derive overall status ---
    let status = derive_status(&issues);

    ProfileHealthReport {
        name: name.to_string(),
        status,
        launch_method: method.to_string(),
        issues,
        checked_at: now_iso8601(),
    }
}

/// Validates all profiles in the store and returns a batch summary.
pub fn batch_validate(store: &ProfileStore) -> Result<HealthCheckSummary, crate::profile::ProfileStoreError> {
    let names = store.list()?;
    let mut profiles = Vec::with_capacity(names.len());

    for name in &names {
        match store.load(name) {
            Ok(profile) => {
                profiles.push(validate_profile_health(name, &profile));
            }
            Err(error) => {
                // Profile exists on disk but cannot be parsed — report as broken.
                profiles.push(ProfileHealthReport {
                    name: name.clone(),
                    status: HealthStatus::Broken,
                    launch_method: String::new(),
                    issues: vec![HealthIssue {
                        field: "profile".to_string(),
                        path: String::new(),
                        message: format!("Failed to load profile: {error}"),
                        remediation: "The profile TOML may be corrupted. Open it in a text editor or delete and recreate the profile.".to_string(),
                        severity: HealthIssueSeverity::Error,
                    }],
                    checked_at: now_iso8601(),
                });
            }
        }
    }

    let healthy_count = profiles.iter().filter(|p| p.status == HealthStatus::Healthy).count();
    let stale_count = profiles.iter().filter(|p| p.status == HealthStatus::Stale).count();
    let broken_count = profiles.iter().filter(|p| p.status == HealthStatus::Broken).count();

    Ok(HealthCheckSummary {
        total_count: profiles.len(),
        healthy_count,
        stale_count,
        broken_count,
        validated_at: now_iso8601(),
        profiles,
    })
}

// --- Internal helpers ---

fn derive_status(issues: &[HealthIssue]) -> HealthStatus {
    if issues.iter().any(|i| i.severity == HealthIssueSeverity::Error) {
        HealthStatus::Broken
    } else if issues.iter().any(|i| i.severity == HealthIssueSeverity::Warning) {
        HealthStatus::Stale
    } else {
        HealthStatus::Healthy
    }
}

fn check_file_path(
    value: &str,
    field: &str,
    label: &str,
    severity: HealthIssueSeverity,
    remediation: &str,
    issues: &mut Vec<HealthIssue>,
) {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        // Empty path for a critical field
        if severity == HealthIssueSeverity::Error {
            issues.push(HealthIssue {
                field: field.to_string(),
                path: String::new(),
                message: format!("{label} path is not configured."),
                remediation: remediation.to_string(),
                severity,
            });
        }
        return;
    }
    let path = Path::new(trimmed);
    if !path.exists() {
        issues.push(HealthIssue {
            field: field.to_string(),
            path: trimmed.to_string(),
            message: format!("{label} path does not exist."),
            remediation: remediation.to_string(),
            severity,
        });
    } else if !path.is_file() {
        issues.push(HealthIssue {
            field: field.to_string(),
            path: trimmed.to_string(),
            message: format!("{label} path exists but is not a file."),
            remediation: remediation.to_string(),
            severity,
        });
    }
}

fn check_directory_path(
    value: &str,
    field: &str,
    label: &str,
    severity: HealthIssueSeverity,
    remediation: &str,
    issues: &mut Vec<HealthIssue>,
) {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        if severity == HealthIssueSeverity::Error {
            issues.push(HealthIssue {
                field: field.to_string(),
                path: String::new(),
                message: format!("{label} path is not configured."),
                remediation: remediation.to_string(),
                severity,
            });
        }
        return;
    }
    let path = Path::new(trimmed);
    if !path.exists() {
        issues.push(HealthIssue {
            field: field.to_string(),
            path: trimmed.to_string(),
            message: format!("{label} does not exist."),
            remediation: remediation.to_string(),
            severity,
        });
    } else if !path.is_dir() {
        issues.push(HealthIssue {
            field: field.to_string(),
            path: trimmed.to_string(),
            message: format!("{label} exists but is not a directory."),
            remediation: remediation.to_string(),
            severity,
        });
    }
}

fn check_executable_path(
    value: &str,
    field: &str,
    label: &str,
    severity: HealthIssueSeverity,
    remediation: &str,
    issues: &mut Vec<HealthIssue>,
) {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        if severity == HealthIssueSeverity::Error {
            issues.push(HealthIssue {
                field: field.to_string(),
                path: String::new(),
                message: format!("{label} path is not configured."),
                remediation: remediation.to_string(),
                severity,
            });
        }
        return;
    }
    let path = Path::new(trimmed);
    if !path.exists() {
        issues.push(HealthIssue {
            field: field.to_string(),
            path: trimmed.to_string(),
            message: format!("{label} does not exist."),
            remediation: remediation.to_string(),
            severity,
        });
    } else if !is_executable_file(path) {
        issues.push(HealthIssue {
            field: field.to_string(),
            path: trimmed.to_string(),
            message: format!("{label} exists but is not executable."),
            remediation: remediation.to_string(),
            severity,
        });
    }
}

fn is_executable_file(path: &Path) -> bool {
    // Reuse pattern from launch/request.rs:740-757
    let metadata = match std::fs::metadata(path) {
        Ok(m) => m,
        Err(_) => return false,
    };
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        metadata.is_file() && metadata.permissions().mode() & 0o111 != 0
    }
    #[cfg(not(unix))]
    {
        metadata.is_file()
    }
}

fn now_iso8601() -> String {
    // Use std::time to avoid adding chrono dependency.
    // Format: "2026-03-27T14:30:00Z" (approximate — no subsecond precision needed).
    let duration = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default();
    // Delegate to JavaScript-compatible ISO string via serde or use a simple formatter.
    // For the initial implementation, return Unix timestamp as string;
    // the frontend can format it. Alternatively, use the `time` crate (already
    // in the dependency tree via tracing-subscriber) for proper formatting.
    format!("{}", duration.as_secs())
}
```

> **Note on `is_executable_file`**: This duplicates logic from `launch/request.rs:740-757`. A follow-up refactor should extract this into a shared utility (e.g., `crosshook_core::fs_util::is_executable_file`). For the initial implementation, duplication is acceptable to keep the health module self-contained.

> **Note on `now_iso8601`**: The `time` crate is already in the dependency tree (via `tracing-subscriber`). Use `time::OffsetDateTime::now_utc().format(&time::format_description::well_known::Rfc3339)` for proper ISO 8601 formatting without adding new dependencies.

---

## System Constraints

### Performance on Steam Deck

| Metric                    | Estimate                              | Acceptable?                      |
| ------------------------- | ------------------------------------- | -------------------------------- |
| Single profile validation | ~5-10ms (load TOML + check 4-8 paths) | Yes                              |
| 50 profiles batch         | ~250-500ms                            | Yes (synchronous)                |
| 100 profiles batch        | ~500ms-1s                             | Borderline — add progress events |
| 200+ profiles batch       | >1s                                   | Requires async with progress     |

**Rationale:** `Path::exists()` is a single `stat()` syscall (~0.5-1ms on NVMe, ~2-5ms on eMMC). Profile TOML parsing is CPU-bound but fast (~0.1ms per profile). The bottleneck is I/O, not CPU.

**Recommendation:** Ship synchronous for v1. The typical Steam Deck user has 10-30 profiles. If telemetry or user reports show >100 profiles, add async progress events in a follow-up.

### Startup Time Impact

The app currently delays auto-load-profile by 350ms after setup (`src-tauri/src/lib.rs:48`). Background health validation should be:

1. Spawned as an async task after startup (like auto-load-profile)
2. Delayed by ~1000ms to avoid competing with profile auto-load
3. Results emitted via `app.emit("health-check-done", summary)` event
4. Frontend listens and caches the result in the `useProfileHealth` hook

```rust
// In src-tauri/src/lib.rs setup closure, after auto-load-profile spawn:
{
    let profile_store = profile_store.clone();
    let app_handle = app.handle().clone();
    tauri::async_runtime::spawn(async move {
        sleep(Duration::from_millis(1000)).await;
        match crosshook_core::health::batch_validate(&profile_store) {
            Ok(summary) => {
                if let Err(error) = app_handle.emit("health-check-done", &summary) {
                    tracing::warn!(%error, "failed to emit health-check-done event");
                }
            }
            Err(error) => {
                tracing::warn!(%error, "startup health check failed");
            }
        }
    });
}
```

### Async Considerations

- `ProfileStore` methods are synchronous (`std::fs`). The Tauri command handler runs them on the async runtime's blocking thread pool.
- `batch_validate_profiles` is a synchronous Tauri command (no `async`). Tauri will automatically run it on a blocking thread.
- The startup background check wraps the synchronous call in `tauri::async_runtime::spawn` (which uses `tokio::task::spawn_blocking` internally for sync functions accessed via `State`).
- No `Mutex` or `RwLock` needed — `ProfileStore` is read-only for health checks and the `base_path` is immutable after construction.

---

## Codebase Changes

### Files to Create

| File                                           | Purpose                                                                                           |
| ---------------------------------------------- | ------------------------------------------------------------------------------------------------- | ----- | --------------------- |
| `crates/crosshook-core/src/health/mod.rs`      | Module root, re-exports                                                                           |
| `crates/crosshook-core/src/health/models.rs`   | `HealthStatus`, `ProfileHealthReport`, `HealthIssue`, `HealthIssueSeverity`, `HealthCheckSummary` |
| `crates/crosshook-core/src/health/validate.rs` | `validate_profile_health()`, `batch_validate()`, internal helpers                                 |
| `src-tauri/src/commands/health.rs`             | `batch_validate_profiles`, `get_profile_health` Tauri commands                                    |
| `src/types/health.ts`                          | TypeScript interfaces mirroring Rust structs                                                      |
| `src/hooks/useProfileHealth.ts`                | React hook: `invoke` + `listen` + `useReducer`                                                    |
| `src/components/ProfileHealthDashboard.tsx`    | Dashboard UI with summary bar + per-profile cards                                                 |
| `src/components/HealthBadge.tsx`               | `<HealthBadge status="healthy                                                                     | stale | broken" />` component |

### Files to Modify

| File                               | Change                                                                                |
| ---------------------------------- | ------------------------------------------------------------------------------------- |
| `crates/crosshook-core/src/lib.rs` | Add `pub mod health;`                                                                 |
| `src-tauri/src/commands/mod.rs`    | Add `pub mod health;`                                                                 |
| `src-tauri/src/lib.rs`             | Register health commands in `invoke_handler`, add startup background check            |
| `src/types/index.ts`               | Add `export * from './health';`                                                       |
| `src/App.tsx`                      | Integrate `ProfileHealthDashboard`                                                    |
| `src/styles/variables.css`         | Add health badge color variables (if not already covered by existing severity colors) |

### Dependencies

No new crate dependencies required. The `time` crate (already present via `tracing-subscriber`) can be used for ISO 8601 timestamp formatting. All path checks use `std::path::Path` and `std::fs`.

---

## Technical Decisions

### Decision 1: New `health` Module vs. Extending `launch/request.rs`

| Option                                   | Pros                                                                                                                 | Cons                                                                            |
| ---------------------------------------- | -------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------- |
| **A: New `health` module** (recommended) | Clean separation of concerns; health checks profile-at-rest, launch checks runtime request; independent test surface | Minor code duplication (`is_executable_file`)                                   |
| B: Extend `launch/request.rs`            | No new module; reuses existing helpers                                                                               | Conflates two different validation lifecycles; `request.rs` already ~1100 lines |

**Recommendation:** Option A. The health module imports from `profile` but does not modify it. A follow-up refactor can extract shared helpers (e.g., `is_executable_file`) into a `crosshook_core::fs_util` module.

### Decision 2: Synchronous vs. Async Batch Validation

| Option                                  | Pros                                                         | Cons                                                                           |
| --------------------------------------- | ------------------------------------------------------------ | ------------------------------------------------------------------------------ |
| **A: Synchronous** (recommended for v1) | Simple; no event coordination; fast enough for <100 profiles | Blocks Tauri command thread during validation                                  |
| B: Async with progress events           | Responsive UI for large profile counts                       | More complex; requires event listener coordination; overkill for typical usage |

**Recommendation:** Option A for the initial implementation. Add progress events as a follow-up if needed.

### Decision 3: Health Status Granularity

| Option                                                | Description                                                                         |
| ----------------------------------------------------- | ----------------------------------------------------------------------------------- |
| A: Binary (healthy/broken)                            | Too coarse — missing icon ≠ missing game exe                                        |
| **B: Tri-state (healthy/stale/broken)** (recommended) | Matches issue #38 spec; "stale" = degraded but launchable; "broken" = cannot launch |
| C: Four-level (healthy/info/warning/error)            | Overcomplicates the summary view                                                    |

**Recommendation:** Option B. Individual issues carry their own severity (error/warning/info), but the profile-level status is the simpler tri-state.

### Decision 4: Trainer Path Validation

The `trainer.path` field stores a WINE-mapped path (e.g., `Z:\trainers\fling.exe` or `/trainers/fling.exe`), not a direct host filesystem path. The `trainer_host_path` field (which IS a host path) only exists on `LaunchRequest`, not on `GameProfile`.

**Recommendation:** Skip filesystem validation of `trainer.path` in health checks. It cannot be reliably resolved to a host path without the full launch context. Document this as a known limitation. The trainer path is validated at launch time via `require_trainer_paths_if_needed()`.

### Decision 5: ISO 8601 Timestamps

| Option                            | Pros                                              | Cons                                                     |
| --------------------------------- | ------------------------------------------------- | -------------------------------------------------------- |
| A: `chrono` crate                 | Full-featured datetime                            | New dependency                                           |
| **B: `time` crate** (recommended) | Already in dependency tree via tracing-subscriber | Slightly more verbose API                                |
| C: Unix timestamp as string       | Zero dependencies                                 | Frontend must format; loses human readability in reports |

**Recommendation:** Option B. Use `time::OffsetDateTime::now_utc()` with RFC 3339 formatting.

---

## Open Questions

1. **Dashboard placement:** Should the health dashboard be a new tab alongside Main/Settings/Community, a sub-section within the Main tab, or a modal triggered from the profile list? (Defer to UX research.)

2. **Profile list integration:** Should the profile dropdown/list in `ProfileEditor.tsx` show inline health badges next to each profile name? This would require `get_profile_health` calls for each listed profile, adding latency to the profile list rendering.

3. **Auto-refresh behavior:** Should health results auto-refresh when the user modifies a profile (e.g., changes a path in ProfileEditor)? This would require the `useProfileHealth` hook to listen for profile save events.

4. **Remediation actions:** Should the dashboard provide one-click remediation (e.g., "Re-run Auto-Populate" button for missing Steam paths)? This adds scope but significantly improves UX. (Defer to business/UX research.)

5. **CLI support:** Should `crosshook-cli` expose a `health` subcommand? The core logic is in `crosshook-core` and would be trivially callable from the CLI. (Low priority, potential follow-up.)

6. **`is_executable_file` deduplication:** The health module duplicates this helper from `launch/request.rs`. Should we extract it to a shared module now, or defer? (Recommend: defer to a follow-up refactor to keep the health PR focused.)

---

## Revision: Cross-Team Feedback Integration

The following revisions incorporate findings from the business-analyzer, practices-researcher, security-researcher, ux-researcher, and recommendations-agent. Where feedback conflicts with the original spec, the resolution is documented with rationale.

### R-1: Path Sanitization in IPC Responses (Security — W-2)

**Finding:** `sanitize_display_path()` in `src-tauri/src/commands/launch.rs:301-306` replaces `$HOME` with `~` before paths cross the IPC boundary. All health report path strings must be sanitized the same way.

**Resolution:** The Tauri command layer (`commands/health.rs`) must apply `sanitize_display_path()` to every `HealthIssue.path` field before returning the `HealthCheckSummary` or `ProfileHealthReport` to the frontend. This mirrors the pattern in `sanitize_diagnostic_report()` at `launch.rs:308-329`.

```rust
// In src-tauri/src/commands/health.rs:
fn sanitize_health_report(mut report: ProfileHealthReport) -> ProfileHealthReport {
    for issue in &mut report.issues {
        issue.path = sanitize_display_path(&issue.path);
    }
    report
}

fn sanitize_health_summary(mut summary: HealthCheckSummary) -> HealthCheckSummary {
    for report in &mut summary.profiles {
        for issue in &mut report.issues {
            issue.path = sanitize_display_path(&issue.path);
        }
    }
    summary
}
```

**Action:** Extract `sanitize_display_path()` to `src-tauri/src/commands/shared.rs` (or a new shared utilities module) so it can be reused by both `launch.rs` and `health.rs` without duplication.

### R-2: PermissionDenied vs NotFound Distinction (Security — W-2)

**Finding:** `std::fs::metadata()` can return `PermissionDenied` in addition to `NotFound`. These require different remediation (fix permissions vs. reinstall/re-browse).

**Resolution:** Update `check_file_path`, `check_directory_path`, and `check_executable_path` to catch `metadata()` errors explicitly and distinguish `PermissionDenied` from `NotFound`:

```rust
fn check_file_path(/* ... */) {
    // ... (empty check as before)
    let path = Path::new(trimmed);
    match std::fs::metadata(path) {
        Ok(metadata) => {
            if !metadata.is_file() {
                issues.push(/* "exists but is not a file" */);
            }
        }
        Err(err) if err.kind() == std::io::ErrorKind::PermissionDenied => {
            issues.push(HealthIssue {
                field: field.to_string(),
                path: trimmed.to_string(),
                message: format!("{label} exists but is not accessible (permission denied)."),
                remediation: format!("Check file permissions on the path or run CrossHook with appropriate access."),
                severity,
            });
        }
        Err(_) => {
            issues.push(HealthIssue {
                field: field.to_string(),
                path: trimmed.to_string(),
                message: format!("{label} path does not exist."),
                remediation: remediation.to_string(),
                severity,
            });
        }
    }
}
```

This gives us four effective path states without changing the `HealthIssueSeverity` enum: **not_configured** (empty, skipped), **missing** (NotFound), **inaccessible** (PermissionDenied), **invalid** (wrong type — file vs dir). The issue `message` field carries the distinction.

### R-3: Module Placement — `health/` Module vs. `profile/health.rs` (Practices)

**Finding:** Practices-researcher recommends `crates/crosshook-core/src/profile/health.rs` (single file, not a new top-level module).

**Trade-off Analysis:**

| Approach                             | Pros                                                | Cons                                                                                         |
| ------------------------------------ | --------------------------------------------------- | -------------------------------------------------------------------------------------------- |
| `health/` top-level module (3 files) | Independent test file, clear boundary, room to grow | One more directory                                                                           |
| `profile/health.rs` (1 file)         | Simpler, fewer files                                | Couples health to profile module; test file mixes with profile tests; harder to add features |

**Resolution:** Keep `health/` as a top-level module with `mod.rs`, `models.rs`, `validate.rs`. Rationale:

- Health validation will grow (DLL path checks, trainer host path resolution, launcher staleness checks are all potential additions).
- Tests for health validation are substantial (50+ profiles × multiple path states × 3 launch methods = many test cases).
- A top-level module with its own `mod.rs` follows the pattern of every other domain module (`export/`, `launch/`, `profile/`, `steam/`, `community/`).

### R-4: Reuse `LaunchValidationIssue` vs. New `HealthIssue` Type (Practices vs. UX/Security)

**Finding:** Practices-researcher says reuse `LaunchValidationIssue { message, help, severity }` directly. UX-researcher wants remediation actions (`RemediationAction[]`). Security-researcher wants the `field` discriminant for structured path identification.

**Resolution:** Keep the new `HealthIssue` type. Justification:

1. `LaunchValidationIssue` lacks a `field` discriminant — the UI cannot know _which_ profile field failed without parsing the message string.
2. `LaunchValidationIssue` lacks `path` — the UI cannot display the broken path.
3. Adding `field` and `path` to `LaunchValidationIssue` would change the existing launch validation IPC contract for no benefit.
4. UX-researcher's `RemediationAction[]` is deferred to a follow-up — the `remediation: String` field provides free-text guidance for v1, and structured actions can be added later without breaking the type.

The `help` text from `ValidationError::help()` can still be used as remediation text content — the existing text is excellent.

### R-5: `GameProfile → LaunchRequest` Conversion Gap (Business)

**Finding:** No Rust-side `GameProfile → LaunchRequest` conversion exists. The frontend builds `LaunchRequest` from UI state. Batch server-side validation via `validate_all()` would need this conversion.

**Resolution:** The health module validates `GameProfile` fields directly (as in the original spec) rather than constructing a `LaunchRequest` and calling `validate_all()`. Rationale:

1. `LaunchRequest` requires `trainer_host_path` and `steam_client_install_path` which are not stored in `GameProfile` — they are derived at launch time from filesystem state.
2. Building a `GameProfile → LaunchRequest` converter would require `derive_steam_client_install_path()` (currently in `src-tauri/src/commands/profile.rs:13`) to be moved to `crosshook-core`, adding scope.
3. Health validation has different semantics than launch validation: it checks _path existence_ not _launch readiness_. A profile can be healthy (all configured paths exist) but still fail launch validation (e.g., incompatible optimization flags).
4. The direct `GameProfile` field checking approach is simpler, self-contained, and avoids coupling to the launch request lifecycle.

**Future consideration:** If `GameProfile → LaunchRequest` conversion is needed for other features, it should be built as a separate utility in `crosshook-core`, and health validation can optionally use it to layer launch-readiness checks on top of path-existence checks.

### R-6: Promote `require_directory()` and `require_executable_file()` (Practices)

**Finding:** Three call sites justify promoting these private helpers from `request.rs` to `pub(crate)`.

**Resolution:** Defer to a follow-up refactor. The health module implements its own versions with a different signature (they push to `&mut Vec<HealthIssue>` instead of returning `Result<_, ValidationError>`). Extracting a shared abstraction that serves both calling conventions adds complexity for minimal benefit in v1. Track as a cleanup task post-merge.

### R-7: Progressive Streaming Validation (UX)

**Finding:** UX-researcher wants per-profile Tauri events emitted as each profile validates, so the UI can update badges incrementally.

**Resolution:** Defer to v2. The synchronous batch command returns all results at once (<500ms for 50 profiles). For the startup background scan, the entire `HealthCheckSummary` is emitted as a single `"health-check-done"` event.

If progressive streaming is needed later, the implementation is straightforward:

```rust
// Future: per-profile event emission
for name in &names {
    let report = /* validate */;
    app_handle.emit("health-check-progress", &report)?;
}
app_handle.emit("health-check-done", &summary)?;
```

The `useProfileHealth` hook should be designed to accept both patterns: a single `HealthCheckSummary` from the synchronous command or incremental `ProfileHealthReport` events from the startup scan.

### R-8: CSP Configuration (Security — W-1)

**Finding:** `tauri.conf.json` has `"csp": null`. Adding health-check commands without CSP means any XSS can probe filesystem paths.

**Resolution:** This is a project-wide security improvement, not specific to the health dashboard. Recommend filing a separate issue to enable CSP:

```json
"csp": "default-src 'self'; script-src 'self'; style-src 'self' 'unsafe-inline'"
```

With `'unsafe-eval'` added only for development builds. This should be addressed before or alongside the health dashboard PR but tracked as a separate work item.

### R-9: Stale vs. Broken Classification Criteria (Business)

**Finding:** Business-analyzer provides a precise classification:

- **Stale** = `*Missing` errors only (paths were configured but deleted from disk — external breakage)
- **Broken** = `*Required`, `*NotFile`, `*NotDirectory`, `*NotExecutable` errors (misconfigured or fundamentally incomplete)

**Resolution:** Align the `derive_status()` function with this classification. Update the logic:

```rust
fn derive_status(issues: &[HealthIssue]) -> HealthStatus {
    if issues.iter().any(|i| i.severity == HealthIssueSeverity::Error) {
        HealthStatus::Broken
    } else if issues.iter().any(|i| i.severity == HealthIssueSeverity::Warning) {
        HealthStatus::Stale
    } else {
        HealthStatus::Healthy
    }
}
```

This is already correct in the original spec. The key implementation detail is assigning the right severity to each check:

- Empty required field (`not_configured`) → `Error` → Broken
- Path does not exist (`missing`) → `Error` for critical paths (game exe, Proton, prefix), `Warning` for optional paths (DLLs, icon)
- Path exists but wrong type or permission denied → `Error` → Broken
- Optional path missing → `Warning` → Stale

### R-10: DLL Path Validation (Business)

**Finding:** DLL paths are NOT currently validated by `validate_all()`. Open question whether to add validation in health checks.

**Resolution:** Include DLL path validation in health checks at `Warning` severity. DLL paths are optional (many profiles have empty DLL arrays), but if configured and missing, the user should be informed. This aligns with the "stale" status — the profile can still launch but with degraded injection behavior.

### R-11: Read-Only Module Documentation (Security)

**Finding:** Mark the health module with a module-level doc comment indicating all operations are read-only.

**Resolution:** Add to `health/mod.rs`:

```rust
//! Profile health validation.
//!
//! All operations in this module are **read-only metadata checks**. No write I/O
//! (`fs::write`, `fs::remove_file`, `fs::rename`) is performed. The module uses
//! only `std::fs::metadata()`, `Path::exists()`, `Path::is_file()`, `Path::is_dir()`,
//! and `PermissionsExt::mode()` for filesystem inspection.
```

---

## Resolved Open Questions

Based on team feedback, the following open questions from the original spec are now resolved:

| #   | Question                            | Resolution                                                                                                   |
| --- | ----------------------------------- | ------------------------------------------------------------------------------------------------------------ |
| 4   | Remediation actions (one-click CTA) | Deferred to v2. UX-researcher confirmed `RemediationAction[]` shape; v1 uses free-text `remediation` string. |
| 6   | `is_executable_file` deduplication  | Deferred to follow-up refactor (confirmed by practices-researcher).                                          |

Remaining open: #1 (dashboard placement), #2 (profile list inline badges), #3 (auto-refresh), #5 (CLI support).

---

## Revision 2: Final Reconciliation (Round 2 Feedback)

This revision addresses the second round of teammate feedback, primarily from practices-researcher, api-researcher, and ux-researcher. Several structural decisions from the original spec and R-1 through R-11 are revised based on stronger codebase evidence.

### R-12: Module Placement — Concede to `profile/health.rs` (Practices)

**Previous position (R-3):** Keep `health/` as a top-level module with 3 files (`mod.rs`, `models.rs`, `validate.rs`).

**New evidence:**

- `crates/crosshook-core/src/profile/` has 6 flat sibling files: `community_schema.rs`, `exchange.rs`, `legacy.rs`, `mod.rs`, `models.rs`, `toml_store.rs`. No subdirectory nesting.
- `commands/` has one file per domain (community.rs, export.rs, launch.rs, profile.rs, etc.) — not per-command.
- api-researcher confirmed: "pub(crate) promotion is the only structural change needed, no new files in crosshook-core."
- The models (`HealthStatus`, `HealthIssue`, `ProfileHealthReport`, `HealthCheckSummary`, `HealthIssueSeverity`) and validation logic fit comfortably in a single ~300-line file.

**Revised decision:** Place all health validation in `crates/crosshook-core/src/profile/health.rs` (single file). If the module grows beyond ~500 lines, split into a `profile/health/` subdirectory in a follow-up.

**Impact on R-3:** R-3 is superseded by this revision.

**Updated integration points:**

- ~~`crates/crosshook-core/src/lib.rs` — add `pub mod health;`~~ → No change needed to `lib.rs`
- `crates/crosshook-core/src/profile/mod.rs` — add `pub mod health;`
- Re-export from `crate::profile::health::*` for external consumers

### R-13: Command Placement — Concede to `commands/profile.rs` (Practices)

**Previous position:** New `src-tauri/src/commands/health.rs` file with dedicated health commands.

**New evidence:**

- `commands/profile.rs` already has 10+ commands (`profile_list`, `profile_load`, `profile_save`, `profile_delete`, `profile_rename`, `profile_duplicate`, etc.).
- Adding 2 health commands (`check_profiles_health`, `check_profile_health`) to the same file follows the existing one-file-per-domain pattern.
- `commands/mod.rs` has 9 modules — adding a 10th for just 2 functions is unnecessary.

**Revised decision:** Add health commands to `src-tauri/src/commands/profile.rs`. The `sanitize_health_report()` and `sanitize_health_summary()` helpers from R-1 also live in `profile.rs` (alongside existing commands).

**Impact on R-1:** `sanitize_display_path()` still needs to be accessible from both `launch.rs` and `profile.rs`. It is already importable via `super::shared::create_log_path` pattern — promote `sanitize_display_path` from `launch.rs` to `shared.rs` (the `commands/shared.rs` module already exists for cross-command utilities).

**Updated command signatures:**

```rust
// In src-tauri/src/commands/profile.rs:

#[tauri::command]
pub fn check_profiles_health(
    store: State<'_, ProfileStore>,
) -> Result<HealthCheckSummary, String> {
    let mut summary = crosshook_core::profile::health::batch_validate(&store)
        .map_err(|e| e.to_string())?;
    sanitize_health_summary(&mut summary);
    Ok(summary)
}

#[tauri::command]
pub fn check_profile_health(
    name: String,
    store: State<'_, ProfileStore>,
) -> Result<ProfileHealthReport, String> {
    let profile = store.load(&name).map_err(|e| e.to_string())?;
    let mut report = crosshook_core::profile::health::validate_profile_health(&name, &profile);
    sanitize_health_report(&mut report);
    Ok(report)
}
```

### R-14: Add `state` Discriminant to `HealthIssue` (UX)

**Finding:** UX-researcher requests an issue-level `state` field that tells the UI _what kind_ of path problem occurred, distinct from `severity` which tells _how bad_ it is. The roll-up `HealthStatus` (healthy/stale/broken) remains unchanged.

**Rationale:**

- `state` enables the UI to show different icons/colors per issue (e.g., 🚫 for missing, 🔒 for inaccessible, ⚙️ for not configured).
- `severity` alone cannot distinguish "path missing from disk" from "path exists but permission denied" — both might be `Error` severity.
- This aligns with R-2's `PermissionDenied` vs `NotFound` distinction but lifts it to a first-class field.

**Revised `HealthIssue` (Rust):**

```rust
/// What happened to the path being checked.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HealthIssueState {
    /// The field is empty or blank — nothing was configured.
    NotConfigured,
    /// The configured path does not exist on disk.
    Missing,
    /// The path exists but is not accessible (e.g., PermissionDenied).
    Inaccessible,
    /// The path exists but has the wrong type (file vs directory) or lacks execute bit.
    Invalid,
    /// The profile TOML file itself could not be loaded.
    Corrupt,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HealthIssue {
    pub field: String,
    pub path: String,
    pub message: String,
    pub remediation: String,
    pub severity: HealthIssueSeverity,
    /// What kind of path problem was detected.
    pub state: HealthIssueState,
}
```

**Revised `HealthIssue` (TypeScript):**

```typescript
export type HealthIssueState = 'not_configured' | 'missing' | 'inaccessible' | 'invalid' | 'corrupt';

export interface HealthIssue {
  field: string;
  path: string;
  message: string;
  remediation: string;
  severity: HealthIssueSeverity;
  state: HealthIssueState;
}
```

**Impact on validation helpers:** `check_file_path`, `check_directory_path`, and `check_executable_path` now set `state` based on the `metadata()` result:

| Condition                                    | `state`         |
| -------------------------------------------- | --------------- |
| Empty/blank field                            | `NotConfigured` |
| `metadata()` returns `NotFound`              | `Missing`       |
| `metadata()` returns `PermissionDenied`      | `Inaccessible`  |
| Path exists but wrong type or not executable | `Invalid`       |
| Profile TOML parse failure                   | `Corrupt`       |

### R-15: Direct `GameProfile` Validation — Hold Firm (Practices vs. API)

**Practices-researcher and api-researcher recommend:** Build `profile_to_launch_request()` or `LaunchRequest::from_profile()` converter, then reuse `validate_all()`.

**We hold firm on direct `GameProfile` field validation.** The reasons from R-5 remain valid, and Round 2 feedback adds one more:

1. **`steam_client_install_path` gap.** Api-researcher says it "comes from AppSettings" — but `AppSettings` is a Tauri-managed state, not available in `crosshook-core`. Passing it through adds a parameter to every health check function.
2. **`trainer_host_path` gap.** Not stored on `GameProfile` at all. Requires filesystem resolution logic that doesn't exist in core.
3. **DLL paths are not validated by `validate_all()`.** Health checks validate them (at Warning severity per R-10). A converter wouldn't help here.
4. **Different error model.** `validate_all()` returns `Vec<LaunchValidationIssue>` — no `field`, no `state`, no `path`. Mapping would lose structural information.

**Concession:** If a `GameProfile → LaunchRequest` converter is built for other features in the future, the health module can layer launch-readiness validation on top of its path-existence checks. This is noted as a future enhancement, not a v1 requirement.

### R-16: Promote `pub(crate)` Helpers (API)

**Api-researcher confirms:** The only structural change needed in `crosshook-core` is promoting `require_directory()`, `require_executable_file()`, and `is_executable_file()` from private to `pub(crate)`.

**Revised position:** We still defer the cross-module extraction (per R-6), but acknowledge that the `pub(crate)` promotion is low-risk and should be done in the health PR if the implementor finds value in it. The health module's own helpers have a different signature (`&mut Vec<HealthIssue>` vs `Result<_, ValidationError>`), so promotion alone doesn't eliminate duplication.

### R-17: Zero New Dependencies — Confirmed (API)

**Api-researcher confirms:** No new crate dependencies needed. `time` crate (via `tracing-subscriber`) for ISO 8601 formatting. `JoinSet` from tokio for parallel validation if needed later (already in dependency tree).

No change to the spec — this confirms Decision 5 (Option B: `time` crate).

---

## Updated File Plan (Post–Revision 2)

### Files to Create

| File                                          | Purpose                                                                                                                                                                                  |
| --------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ----- | --------------------- |
| `crates/crosshook-core/src/profile/health.rs` | `HealthStatus`, `HealthIssueSeverity`, `HealthIssueState`, `HealthIssue`, `ProfileHealthReport`, `HealthCheckSummary`, `validate_profile_health()`, `batch_validate()`, internal helpers |
| `src/types/health.ts`                         | TypeScript interfaces mirroring Rust structs                                                                                                                                             |
| `src/hooks/useProfileHealth.ts`               | React hook: `invoke` + `listen` + `useReducer`                                                                                                                                           |
| `src/components/ProfileHealthDashboard.tsx`   | Dashboard UI with summary bar + per-profile cards                                                                                                                                        |
| `src/components/HealthBadge.tsx`              | `<HealthBadge status="healthy                                                                                                                                                            | stale | broken" />` component |

### Files to Modify

| File                                       | Change                                                                                                               |
| ------------------------------------------ | -------------------------------------------------------------------------------------------------------------------- |
| `crates/crosshook-core/src/profile/mod.rs` | Add `pub mod health;`                                                                                                |
| `src-tauri/src/commands/profile.rs`        | Add `check_profiles_health` and `check_profile_health` commands, sanitization helpers                                |
| `src-tauri/src/commands/shared.rs`         | Move `sanitize_display_path()` from `launch.rs` to shared                                                            |
| `src-tauri/src/commands/launch.rs`         | Import `sanitize_display_path` from `shared` instead of local definition                                             |
| `src-tauri/src/lib.rs`                     | Register `check_profiles_health` and `check_profile_health` in `invoke_handler`; add startup background health check |
| `src/types/index.ts`                       | Add `export * from './health';`                                                                                      |
| `src/App.tsx`                              | Integrate `ProfileHealthDashboard`                                                                                   |
| `src/styles/variables.css`                 | Add health badge color variables (if not already covered)                                                            |

### Files Removed from Plan (vs. Original Spec)

| File                                               | Reason                                   |
| -------------------------------------------------- | ---------------------------------------- |
| ~~`crates/crosshook-core/src/health/mod.rs`~~      | Replaced by `profile/health.rs` (R-12)   |
| ~~`crates/crosshook-core/src/health/models.rs`~~   | Merged into `profile/health.rs` (R-12)   |
| ~~`crates/crosshook-core/src/health/validate.rs`~~ | Merged into `profile/health.rs` (R-12)   |
| ~~`src-tauri/src/commands/health.rs`~~             | Merged into `commands/profile.rs` (R-13) |

### Updated Architecture Diagram

```
┌─────────────────────────────────────────────────────────┐
│  React Frontend                                         │
│  ┌──────────────────────┐  ┌─────────────────────────┐  │
│  │ ProfileHealthDashboard│  │ HealthBadge             │  │
│  │  (new component)      │  │  (reusable badge)       │  │
│  └──────────┬───────────┘  └─────────────────────────┘  │
│             │ invoke()                                   │
│  ┌──────────┴───────────┐                               │
│  │ useProfileHealth     │  listen("health-check-done")  │
│  │  (new hook)          │◄──────────────────────────────│
│  └──────────┬───────────┘                               │
└─────────────┼───────────────────────────────────────────┘
              │ Tauri IPC
┌─────────────┼───────────────────────────────────────────┐
│  src-tauri  │                                           │
│  ┌──────────┴───────────┐                               │
│  │ commands/profile.rs  │  (extended — 2 new commands)  │
│  │  check_profiles_     │                               │
│  │  health()            │                               │
│  │  check_profile_      │                               │
│  │  health()            │                               │
│  └──────────┬───────────┘                               │
└─────────────┼───────────────────────────────────────────┘
              │
┌─────────────┼───────────────────────────────────────────┐
│  crosshook-core                                         │
│  ┌──────────┴───────────┐  ┌─────────────────────────┐  │
│  │ profile/             │  │ profile/                │  │
│  │  health.rs (new)     │  │  models.rs (GameProfile) │  │
│  │  validate + models   │──│  toml_store.rs (Store)   │  │
│  └──────────────────────┘  └─────────────────────────┘  │
│       uses: std::path, std::fs::metadata, time crate    │
└─────────────────────────────────────────────────────────┘
```

---

## Summary of Concessions and Holds

| #    | Topic                  | Decision                                        | Rationale                                                                               |
| ---- | ---------------------- | ----------------------------------------------- | --------------------------------------------------------------------------------------- |
| R-12 | Module placement       | **Concede** → `profile/health.rs`               | Codebase has flat file structure; 6 sibling files in profile/                           |
| R-13 | Command placement      | **Concede** → `commands/profile.rs`             | One file per domain; 2 commands don't warrant a new file                                |
| R-14 | `state` discriminant   | **Accept** → add `HealthIssueState`             | Enables UI distinction between missing/inaccessible/invalid/not_configured              |
| R-15 | Direct validation      | **Hold firm** → validate `GameProfile` directly | Converter has 3 field gaps; different error model; DLLs not covered by `validate_all()` |
| R-16 | `pub(crate)` promotion | **Acknowledge** → optional in health PR         | Different signatures; low-risk but not blocking                                         |
| R-17 | Zero new deps          | **Confirmed** → no changes                      | `time` crate already available                                                          |

---

## Revision 3: Type Simplification and Classification Rules (Round 3 Feedback)

This revision addresses the third round of feedback from business-analyzer, practices-researcher, ux-researcher, and api-researcher. The major change: **drop custom health types in favor of reusing `LaunchValidationIssue`**, and formalize the Stale vs Broken classification.

### R-18: Drop `HealthIssue` — Reuse `LaunchValidationIssue` (UX + Practices)

**Previous position (R-4, R-14):** Custom `HealthIssue` type with `field`, `path`, `state`, `remediation`, `severity` fields. Custom `HealthIssueState` enum.

**Round 3 convergence:**

- UX-researcher reverses Round 2 position: "ship with the existing type as-is. No extension needed for v1."
- Practices-researcher: "use `ValidationError` variants + `.issue()` for producing the `LaunchValidationIssue` output — not new message strings."
- Business-analyzer: confirms the output shape is secondary to the validation semantics.

**Revised decision:** Drop `HealthIssue`, `HealthIssueSeverity`, and `HealthIssueState` entirely. Reuse `LaunchValidationIssue` (`message`, `help`, `severity: ValidationSeverity`) as the issue type for health reports.

**Trade-off:**

- `LaunchValidationIssue` has no `field` discriminant — the UI cannot programmatically identify which profile section failed. For v1 this is acceptable: remediation is "Open Profile" button + prose help text.
- `LaunchValidationIssue` has no `path` field — the broken path is embedded in the `message` string. Acceptable for v1.
- Phase 2 can add `code: Option<String>` to `LaunchValidationIssue` (per UX-researcher) if per-field "Browse…" CTAs are needed. This is a non-breaking, additive change.

**Impact:** R-4 and R-14 are superseded. `HealthIssueSeverity`, `HealthIssue`, and `HealthIssueState` are removed from the spec.

### R-19: Stale vs Broken Classification (Business — SD Card Rule)

**Finding:** Business-analyzer formalizes the classification:

- **Stale** = path was configured (non-empty) but `Path::exists()` returns false. Cause is external: game uninstalled, SD card unmounted, Proton version removed.
- **Broken** = field is empty (never configured), or path exists but has wrong type (file vs directory) or wrong permissions.
- **Unconfigured** = all fields empty (UI-layer rendering distinction only, same `Broken` enum variant).

**SD card rule:** A missing-but-configured path is ALWAYS `Stale`, regardless of whether the field is "critical" (game exe) or "optional" (DLL). The system cannot distinguish a deleted file from an unmounted drive, and "stale" is more accurate than "broken" for external-state changes.

**Impact on severity assignment:**

| Condition                                    | Severity  | Status  |
| -------------------------------------------- | --------- | ------- |
| Required field empty (not configured)        | `Fatal`   | Broken  |
| Path configured but does not exist (missing) | `Warning` | Stale   |
| Path exists but wrong type (file↔dir)        | `Fatal`   | Broken  |
| Path exists but not executable               | `Fatal`   | Broken  |
| Optional path configured but missing         | `Warning` | Stale   |
| Optional path empty                          | Skipped   | —       |
| Info-level cosmetic issue                    | `Info`    | Healthy |

**This changes R-9's severity mapping.** Previously, a missing critical path (game exe) was `Fatal` → Broken. Now it is `Warning` → Stale. The distinction: "Broken" means the user must reconfigure; "Stale" means something external changed and a re-browse or remount may fix it.

### R-20: Converter Debate Resolved — No Converter (Business + Practices)

**Round 3 status:**

- business-analyzer: "Health validation must NOT route through `LaunchRequest` / `validate_all()`."
- practices-researcher: "Direct GameProfile operation is the right call — no LaunchRequest conversion."
- api-researcher: Still recommends `profile_to_launch_request()` + `validate_all()`.

**Resolution:** The converter approach is rejected by 3 of 4 stakeholders. api-researcher's sample code uses `profile_to_launch_request(&profile)` + `validate_all(&request)`, but this has the same gaps identified in R-5 and R-15 (`steam_client_install_path` not on `GameProfile`, `trainer_host_path` not stored, DLLs not covered by `validate_all()`). R-15 stands. R-5 stands.

### R-21: Helper Reuse — Promote `is_executable_file()` Only (Practices)

**Practices-researcher requests:** Promote `require_directory()`, `require_executable_file()`, and `is_executable_file()` to `pub(crate)` and reuse them.

**Analysis of actual signatures:**

```rust
// launch/request.rs:698
fn require_directory<'a>(
    value: &'a str,
    required_error: ValidationError,   // ← launch-specific error variant
    missing_error: ValidationError,    // ← always Fatal severity via .issue()
    not_directory_error: ValidationError,
) -> Result<&'a Path, ValidationError>

// launch/request.rs:719
fn require_executable_file(
    value: &str,
    required_error: ValidationError,
    missing_error: ValidationError,
    not_executable_error: ValidationError,
) -> Result<(), ValidationError>
```

The helpers take `ValidationError` variants as parameters. Each variant produces a `LaunchValidationIssue` via `.issue()` with a fixed severity. For health checking, **missing paths must be `Warning` (→ Stale)**, but `ValidationError::*Missing` variants produce `Fatal` severity. The severity is baked into the error variant's `.issue()` method.

**Options:**

1. Use the helpers and override severity after `.issue()` → hacky, fragile
2. Add a severity parameter to the helpers → changes the launch code for no benefit
3. Promote only `is_executable_file()` and write health-specific path checkers → clean separation

**Revised decision:** Promote `is_executable_file()` to `pub(crate)` (it's a pure utility with no semantic baggage). Write health-specific `check_required_file()`, `check_required_directory()`, and `check_executable_path()` functions in `profile/health.rs` that construct `LaunchValidationIssue` directly with health-appropriate severities.

**Rationale:** The helpers' signatures are tightly coupled to `ValidationError`'s launch-specific error model. Reusing them would either require overriding severity (fragile) or changing the launch code (unnecessary scope). The path-checking logic itself is trivial (`Path::exists()`, `Path::is_file()`, `Path::is_dir()`); the value of the helpers is in their error construction, which differs between health and launch contexts.

**Follow-up:** If both sites converge on a shared error model in the future, extract a generic `check_path()` utility. For v1, the duplication is 3 small functions totaling ~60 lines.

### R-22: Async Startup — Hardened Requirement (Business)

**Finding:** Business-analyzer hardens: "Health validation must NOT be in the synchronous startup path. Must be a spawned async task that emits a Tauri event when complete. UI shows immediately; badges populate seconds later."

**Resolution:** Already specified in the original spec (startup background check section). This revision formalizes it as a hard requirement, not optional:

1. The `check_profiles_health` Tauri command is for on-demand validation (button press).
2. The startup scan is a separate `tauri::async_runtime::spawn` task with ~1000ms delay.
3. UI renders immediately with no health badges. Badges populate when `"health-check-done"` event arrives.
4. The `useProfileHealth` hook initializes with `null` summary and listens for the event.

No code change from the original spec — this revision just upgrades "optionally spawn" to "must spawn."

### R-23: Sequential Batch for v1 (API)

**Finding:** Api-researcher concedes: "The async batch pattern I suggested adds real complexity for negligible gain. Stat-only validation on <50 profiles runs in single-digit milliseconds sequentially."

**Resolution:** Confirms Decision 2 (synchronous for v1). No JoinSet, no spawn_blocking, no parallel validation. Sequential loop.

### R-24: Phase 1 Remediation UX (UX)

**Finding:** UX-researcher defines Phase 1 remediation: "Prose help text + one 'Open Profile' button per broken profile. No per-issue action buttons. Gamepad-accessible (one confirm press)."

**Resolution:** The `ProfileHealthDashboard` component renders:

- Summary bar with counts (healthy/stale/broken)
- Per-profile cards with status badge, issue list (message + help text), and single "Open Profile" button
- "Open Profile" navigates to `ProfileEditor` with the profile name pre-selected

No structured `RemediationAction[]` for v1. The `help` field on `LaunchValidationIssue` provides the prose guidance.

---

## Updated Data Models (Post–Revision 3)

### Rust Types (`crates/crosshook-core/src/profile/health.rs`)

```rust
use serde::{Deserialize, Serialize};
use crate::launch::{LaunchValidationIssue, ValidationSeverity};
use crate::profile::{GameProfile, resolve_launch_method};

/// Overall health status for a profile.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HealthStatus {
    Healthy,
    Stale,
    Broken,
}

/// Health report for a single profile.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProfileHealthResult {
    pub name: String,
    pub status: HealthStatus,
    pub launch_method: String,
    pub issues: Vec<LaunchValidationIssue>,
    pub checked_at: String,
}

/// Aggregate summary of a batch health check.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HealthCheckSummary {
    pub profiles: Vec<ProfileHealthResult>,
    pub healthy_count: usize,
    pub stale_count: usize,
    pub broken_count: usize,
    pub total_count: usize,
    pub validated_at: String,
}
```

**Removed:** `HealthIssue`, `HealthIssueSeverity`, `HealthIssueState` (superseded by R-18).

### TypeScript Types (`src/types/health.ts`)

```typescript
import type { LaunchValidationIssue } from './launch';

export type HealthStatus = 'healthy' | 'stale' | 'broken';

export interface ProfileHealthResult {
  name: string;
  status: HealthStatus;
  launch_method: string;
  issues: LaunchValidationIssue[];
  checked_at: string;
}

export interface HealthCheckSummary {
  profiles: ProfileHealthResult[];
  healthy_count: number;
  stale_count: number;
  broken_count: number;
  total_count: number;
  validated_at: string;
}
```

### Updated Validation Logic (`profile/health.rs`)

```rust
use std::path::Path;
use crate::launch::is_executable_file; // promoted to pub(crate)

pub fn validate_profile_health(name: &str, profile: &GameProfile) -> ProfileHealthResult {
    let mut issues = Vec::new();
    let method = resolve_launch_method(profile);

    // --- Game executable (all methods) ---
    check_required_file(
        &profile.game.executable_path,
        "Game executable",
        "Re-browse to the game executable or verify game files are installed.",
        &mut issues,
    );

    // --- Steam paths (steam_applaunch only) ---
    if method == "steam_applaunch" {
        check_required_directory(
            &profile.steam.compatdata_path,
            "Steam compatdata directory",
            "Launch the game through Steam once to create the compatdata directory, or use Auto-Populate.",
            &mut issues,
        );
        check_required_executable(
            &profile.steam.proton_path,
            "Steam Proton executable",
            "The configured Proton version may have been removed. Re-select via Auto-Populate.",
            &mut issues,
        );
    }

    // --- Runtime paths (proton_run only) ---
    if method == "proton_run" {
        check_required_directory(
            &profile.runtime.prefix_path,
            "WINE/Proton prefix directory",
            "Re-select the prefix directory or launch the game once to recreate it.",
            &mut issues,
        );
        check_required_executable(
            &profile.runtime.proton_path,
            "Runtime Proton executable",
            "The configured Proton version may have been removed. Re-select an installed Proton.",
            &mut issues,
        );
    }

    // --- DLL injection paths (optional, all non-native methods) ---
    for (i, dll_path) in profile.injection.dll_paths.iter().enumerate() {
        if dll_path.trim().is_empty() {
            continue;
        }
        check_optional_file(
            dll_path,
            &format!("DLL injection path #{}", i + 1),
            "Remove the DLL path or re-browse to the correct file.",
            &mut issues,
        );
    }

    // --- Launcher icon path (cosmetic) ---
    if !profile.steam.launcher.icon_path.trim().is_empty() {
        check_optional_file(
            &profile.steam.launcher.icon_path,
            "Launcher icon",
            "Remove the icon path or browse to a new icon image.",
            &mut issues,
        );
    }

    let status = derive_status(&issues);

    ProfileHealthResult {
        name: name.to_string(),
        status,
        launch_method: method.to_string(),
        issues,
        checked_at: now_iso8601(),
    }
}

// --- Internal helpers ---

fn derive_status(issues: &[LaunchValidationIssue]) -> HealthStatus {
    if issues.iter().any(|i| i.severity == ValidationSeverity::Fatal) {
        HealthStatus::Broken
    } else if issues.iter().any(|i| i.severity == ValidationSeverity::Warning) {
        HealthStatus::Stale
    } else {
        HealthStatus::Healthy
    }
}

/// Required field: empty → Fatal (Broken), missing → Warning (Stale), wrong type → Fatal (Broken).
fn check_required_file(
    value: &str,
    label: &str,
    help: &str,
    issues: &mut Vec<LaunchValidationIssue>,
) {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        issues.push(LaunchValidationIssue {
            message: format!("{label} path is not configured."),
            help: help.to_string(),
            severity: ValidationSeverity::Fatal,
        });
        return;
    }
    let path = Path::new(trimmed);
    if !path.exists() {
        issues.push(LaunchValidationIssue {
            message: format!("{label} does not exist: {trimmed}"),
            help: help.to_string(),
            severity: ValidationSeverity::Warning, // Stale — external change
        });
    } else if !path.is_file() {
        issues.push(LaunchValidationIssue {
            message: format!("{label} exists but is not a file: {trimmed}"),
            help: help.to_string(),
            severity: ValidationSeverity::Fatal,
        });
    }
}

/// Required directory: empty → Fatal, missing → Warning, wrong type → Fatal.
fn check_required_directory(
    value: &str,
    label: &str,
    help: &str,
    issues: &mut Vec<LaunchValidationIssue>,
) {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        issues.push(LaunchValidationIssue {
            message: format!("{label} path is not configured."),
            help: help.to_string(),
            severity: ValidationSeverity::Fatal,
        });
        return;
    }
    let path = Path::new(trimmed);
    if !path.exists() {
        issues.push(LaunchValidationIssue {
            message: format!("{label} does not exist: {trimmed}"),
            help: help.to_string(),
            severity: ValidationSeverity::Warning,
        });
    } else if !path.is_dir() {
        issues.push(LaunchValidationIssue {
            message: format!("{label} exists but is not a directory: {trimmed}"),
            help: help.to_string(),
            severity: ValidationSeverity::Fatal,
        });
    }
}

/// Required executable: empty → Fatal, missing → Warning, not executable → Fatal.
fn check_required_executable(
    value: &str,
    label: &str,
    help: &str,
    issues: &mut Vec<LaunchValidationIssue>,
) {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        issues.push(LaunchValidationIssue {
            message: format!("{label} path is not configured."),
            help: help.to_string(),
            severity: ValidationSeverity::Fatal,
        });
        return;
    }
    let path = Path::new(trimmed);
    if !path.exists() {
        issues.push(LaunchValidationIssue {
            message: format!("{label} does not exist: {trimmed}"),
            help: help.to_string(),
            severity: ValidationSeverity::Warning,
        });
    } else if !is_executable_file(path) {
        issues.push(LaunchValidationIssue {
            message: format!("{label} exists but is not executable: {trimmed}"),
            help: help.to_string(),
            severity: ValidationSeverity::Fatal,
        });
    }
}

/// Optional field: empty → skip, missing → Warning, wrong type → Info.
fn check_optional_file(
    value: &str,
    label: &str,
    help: &str,
    issues: &mut Vec<LaunchValidationIssue>,
) {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return; // Optional — not configured is fine
    }
    let path = Path::new(trimmed);
    if !path.exists() {
        issues.push(LaunchValidationIssue {
            message: format!("{label} does not exist: {trimmed}"),
            help: help.to_string(),
            severity: ValidationSeverity::Warning,
        });
    } else if !path.is_file() {
        issues.push(LaunchValidationIssue {
            message: format!("{label} exists but is not a file: {trimmed}"),
            help: help.to_string(),
            severity: ValidationSeverity::Info,
        });
    }
}
```

> **Note on `is_executable_file`**: Promoted from `launch/request.rs:740` to `pub(crate)`. The health module imports it as `crate::launch::is_executable_file`. The only structural change needed in `launch/request.rs` is changing `fn is_executable_file` to `pub(crate) fn is_executable_file`.

> **Note on `require_directory` / `require_executable_file`**: NOT reused from `launch/request.rs` because their signatures take `ValidationError` variants with hardcoded `Fatal` severity. Health checking needs `Warning` severity for missing-but-configured paths (→ Stale). Writing 4 small functions (~60 lines total) is cleaner than overriding severity after the fact. A follow-up refactor can extract a shared `check_path` abstraction if both calling conventions converge.

---

## Updated File Plan (Post–Revision 3)

### Files to Create

| File                                          | Purpose                                                                                                                                      |
| --------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------- | ----- | --------------------- |
| `crates/crosshook-core/src/profile/health.rs` | `HealthStatus`, `ProfileHealthResult`, `HealthCheckSummary`, `validate_profile_health()`, `batch_validate()`, internal path-checking helpers |
| `src/types/health.ts`                         | TypeScript interfaces (`HealthStatus`, `ProfileHealthResult`, `HealthCheckSummary`) — reuses `LaunchValidationIssue` from `types/launch.ts`  |
| `src/hooks/useProfileHealth.ts`               | React hook: `invoke` + `listen("health-check-done")` + `useReducer`                                                                          |
| `src/components/ProfileHealthDashboard.tsx`   | Dashboard UI: summary bar + per-profile cards with "Open Profile" button                                                                     |
| `src/components/HealthBadge.tsx`              | `<HealthBadge status="healthy                                                                                                                | stale | broken" />` component |

### Files to Modify

| File                                          | Change                                                                           |
| --------------------------------------------- | -------------------------------------------------------------------------------- |
| `crates/crosshook-core/src/profile/mod.rs`    | Add `pub mod health;`                                                            |
| `crates/crosshook-core/src/launch/request.rs` | Promote `is_executable_file` from `fn` to `pub(crate) fn`                        |
| `src-tauri/src/commands/profile.rs`           | Add `check_profiles_health` and `check_profile_health` commands                  |
| `src-tauri/src/commands/shared.rs`            | Move `sanitize_display_path()` from `launch.rs`                                  |
| `src-tauri/src/commands/launch.rs`            | Import `sanitize_display_path` from `shared`                                     |
| `src-tauri/src/lib.rs`                        | Register health commands in `invoke_handler`; add startup background health scan |
| `src/types/index.ts`                          | Add `export * from './health';`                                                  |
| `src/App.tsx`                                 | Integrate `ProfileHealthDashboard`                                               |
| `src/styles/variables.css`                    | Add health badge color variables (if not covered)                                |

### Summary of Revision 3 Concessions and Holds

| #    | Topic                | Decision                                          | Rationale                                                                                       |
| ---- | -------------------- | ------------------------------------------------- | ----------------------------------------------------------------------------------------------- |
| R-18 | Drop `HealthIssue`   | **Concede** → reuse `LaunchValidationIssue`       | UX + practices agree: v1 needs no new issue type; `message` + `help` + `severity` is sufficient |
| R-19 | Stale classification | **Accept** → missing path = always Stale          | SD card rule: missing-but-configured is external change, not misconfiguration                   |
| R-20 | No converter         | **Hold firm** → validate `GameProfile` directly   | 3 of 4 stakeholders agree; converter has 3 field gaps                                           |
| R-21 | Helper reuse         | **Partial** → promote `is_executable_file()` only | `require_*` helpers have launch-specific severity baked in; health needs different severity     |
| R-22 | Async startup        | **Accept** → hardened to must-have                | Business-analyzer requirement; already in original spec as optional                             |
| R-23 | Sequential batch     | **Confirmed** → no JoinSet for v1                 | Api-researcher concedes; <50 profiles in single-digit ms                                        |
| R-24 | Phase 1 remediation  | **Accept** → prose help + "Open Profile" button   | Gamepad-accessible; no per-issue action buttons for v1                                          |

---

## Revision 4: EACCES Detection, Auto-Revalidate, Notification Rules (Round 4 Feedback)

This revision addresses the fourth round from business-analyzer, security-researcher, api-researcher, and ux-researcher. The major additions: internal `HealthIssueKind` enum for EACCES/ENOENT distinction, single-profile revalidation command, and notification rules.

### R-25: Internal `HealthIssueKind` Enum + `metadata()` Error Detection (Security + Business)

**Finding:** Security-researcher and business-analyzer define a 4-state classification:

- `NotConfigured` — field is empty
- `Missing` — path non-empty, `ENOENT` → Stale
- `Inaccessible` — path exists but `EACCES` (or other OS error) → Broken
- `WrongType` — path exists, wrong fs type (file↔dir) or not executable → Broken

**Reconciliation with R-18:** `HealthIssueKind` is an **internal Rust enum** used during validation to determine:

1. The correct `ValidationSeverity` for the output `LaunchValidationIssue`
2. Accurate message text ("does not exist" vs "not accessible — permission denied")
3. Accurate help text ("re-browse to file" vs "check file permissions")

It does NOT cross the IPC boundary in v1. The output remains `LaunchValidationIssue` with `message`, `help`, `severity` — preserving R-18.

**Phase 2 path:** Add `code: Option<String>` to `LaunchValidationIssue` with values `"not_configured"`, `"missing"`, `"inaccessible"`, `"wrong_type"` matching the enum names (per ux-researcher). This is additive and non-breaking.

**Internal enum definition:**

```rust
/// Internal classification of a path check result.
/// Used to determine severity and message content.
/// Does not cross the IPC boundary in v1.
enum PathCheckResult {
    /// Path is valid and matches expected type.
    Ok,
    /// Field is empty — not configured.
    NotConfigured,
    /// Path non-empty, ENOENT (or dangling symlink target gone).
    Missing,
    /// Path non-empty, EACCES or other OS error — cannot confirm absent.
    Inaccessible,
    /// Path exists but wrong fs type or not executable.
    WrongType,
}
```

**Detection pattern (from security-researcher):**

```rust
fn check_path(path: &Path) -> PathCheckResult {
    match std::fs::metadata(path) {
        Ok(meta) => PathCheckResult::Ok, // caller checks is_file/is_dir/mode
        Err(e) => match e.kind() {
            std::io::ErrorKind::NotFound => PathCheckResult::Missing,
            std::io::ErrorKind::PermissionDenied => PathCheckResult::Inaccessible,
            _ => PathCheckResult::Inaccessible, // can't confirm absent — treat as inaccessible
        },
    }
}
```

**Nuances documented:**

1. **Dangling symlinks → `NotFound`**: `metadata()` follows symlinks. Dangling target → `ENOENT` → `Missing` → Stale. Correct behavior.
2. **Catch-all `_` → `Inaccessible`**: Any error other than `NotFound` maps to `Inaccessible` — we cannot confirm the path is absent.
3. **Linux-only**: No cross-platform ambiguity. `ENOENT` = `NotFound`, `EACCES` = `PermissionDenied`.

**Impact on existing helpers:** The existing `require_directory()` and `require_executable_file()` in `launch/request.rs:698-737` use `Path::exists()` which does NOT distinguish `EACCES` from `ENOENT`. The health module must use `std::fs::metadata()` directly. This confirms R-21's decision: the existing helpers cannot be reused for health checking. Only `is_executable_file()` (promoted to `pub(crate)`) is shared.

**Severity mapping with `PathCheckResult`:**

| `PathCheckResult` | Required field severity | Optional field severity |
| ----------------- | ----------------------- | ----------------------- |
| `NotConfigured`   | `Fatal` → Broken        | Skip (no issue)         |
| `Missing`         | `Warning` → Stale       | `Warning` → Stale       |
| `Inaccessible`    | `Fatal` → Broken        | `Warning` → Stale       |
| `WrongType`       | `Fatal` → Broken        | `Info` → Healthy        |

### R-26: Helper Reuse Decision — Option C with Option B as Follow-Up (Business)

**Finding:** Business-analyzer presents three options:

- **A**: Promote `require_*` to `pub(crate)`, call from health module — but they use `Path::exists()` (no EACCES), return `ValidationError` with wrong severities
- **B**: Extract shared `check_path_status()` primitive — clean but adds scope
- **C**: Re-implement in health module — simplest, ~10 lines per check, minor duplication

**Decision: Option C for v1, Option B as follow-up.**

Rationale:

1. The existing helpers use `Path::exists()` — they cannot detect `EACCES` (R-25 requirement).
2. Health helpers need `metadata()` error kind matching — fundamentally different from the existing helpers.
3. The health-specific helpers total ~80 lines. The duplication is in the check pattern, not in complex business logic.
4. Option B (shared `PathStatus` primitive) is a good future refactor that would benefit both `validate_all()` and `validate_profile_health()`, but it changes `launch/request.rs` for no v1 benefit.

**Promotion of `is_executable_file()` still applies:** The mode-bit checking logic (line 740-757) is a pure utility with no semantic baggage. Promote to `pub(crate)` and re-export from `launch/mod.rs`.

### R-27: Single-Profile `check_profile_health` Command + Auto-Revalidate (Business + UX)

**Finding:** Business-analyzer and ux-researcher confirm: auto-revalidate the affected profile immediately after `save_profile` resolves. This requires a single-profile command.

**This command was already specified in R-13** as `check_profile_health(name: String) -> ProfileHealthResult`. Round 4 adds the auto-revalidate trigger:

**Frontend auto-revalidate pattern:**

```typescript
// In useProfile hook or ProfileEditor component:
async function handleSave(profile: GameProfile) {
  await invoke('profile_save', { name, profile });
  // Auto-revalidate after save
  const healthResult = await invoke<ProfileHealthResult>('check_profile_health', { name });
  // Update badge in-place via useProfileHealth dispatch
  dispatch({ type: 'profile-updated', report: healthResult });
}
```

**Edge case (business-analyzer):** If the user navigates away before the async health result arrives, the badge holds its last-known state and updates silently on return. No spinner revert. This is a frontend state management concern — the hook initializes with `null` and updates when results arrive.

**Final command surface:**

| Command                 | Signature                               | Trigger                                                |
| ----------------------- | --------------------------------------- | ------------------------------------------------------ |
| `check_profiles_health` | `() -> HealthCheckSummary`              | Startup background scan + manual "Re-check All" button |
| `check_profile_health`  | `(name: String) -> ProfileHealthResult` | Auto-revalidate after `profile_save`                   |

Both commands share `validate_profile_health()` core logic. `check_profiles_health` calls `batch_validate()` which iterates and calls `validate_profile_health()` per profile.

### R-28: No Persistence — In-Memory Only (Security + Business)

**Finding:** Health results are never persisted to disk. `checked_at` on `ProfileHealthResult` is display-only cosmetic feedback.

**Implications:**

1. No TOML/JSON health cache files in `~/.config/crosshook/`
2. On app restart, health state is unknown until the startup background scan completes
3. The `useProfileHealth` hook initializes with `null` summary
4. No time-based staleness threshold (would require persistence, rejected)

This is already consistent with the original spec but now formalized as a hard constraint.

### R-29: Notification Rules (UX + Business)

**Finding:** UX-researcher and business-analyzer define notification behavior:

| Status                 | Notification   | Behavior                                                                 |
| ---------------------- | -------------- | ------------------------------------------------------------------------ |
| Broken (any profile)   | Startup banner | Dismissable, per-session. Re-appears on next app launch if still broken. |
| Stale (any profile)    | Badge only     | No banner. Badge visible in profile list / dashboard.                    |
| Healthy (all profiles) | None           | No banner, green badges.                                                 |

**Frontend implementation notes:**

- The startup banner is rendered by `ProfileHealthDashboard` (or a dedicated `HealthBanner` sub-component) based on the `HealthCheckSummary` received from the `"health-check-done"` event.
- Banner dismissed state is held in React state (not persisted — per R-28).
- The banner shows: count of broken profiles + "View Details" link to the dashboard.

### R-30: No Auto-Repair — Diagnostic Only (Business)

**Confirmed:** The health dashboard performs NO profile data mutation. All repair flows are user-initiated:

- "Open Profile" → navigates to `ProfileEditor` with profile pre-selected
- "Re-run Auto-Populate" → user manually triggers from within ProfileEditor
- Profile field changes → user edits in ProfileEditor → `profile_save` → auto-revalidate (R-27)

The health module is read-only (confirmed in R-11). No `fs::write`, `fs::remove_file`, or `ProfileStore::save()` calls.

### R-31: No `AppSettings` Injection (API + Business — Confirmed)

**Finding:** `AppSettingsData` does NOT store `steam_client_install_path`. It is derived at launch time by splitting `steam.compatdata_path` on `/steamapps/compatdata/` (in `commands/profile.rs:13-19`).

**Impact:** The `check_profiles_health` and `check_profile_health` commands need only `State<'_, ProfileStore>`. No `State<'_, SettingsStore>` parameter. This confirms the original spec and simplifies the command signatures.

Api-researcher has updated their research to mark this gap as resolved and confirmed the direct `GameProfile` approach.

---

## Updated Validation Helpers (Post–Revision 4)

The following replaces the validation logic from Revision 3, incorporating `metadata()` error kind matching:

```rust
use std::path::Path;
use std::io;
use crate::launch::{LaunchValidationIssue, ValidationSeverity};

/// Internal path check result — does not cross IPC.
enum PathCheckResult {
    Ok(std::fs::Metadata),
    NotConfigured,
    Missing,
    Inaccessible,
    WrongType,
}

fn check_path_metadata(value: &str) -> PathCheckResult {
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return PathCheckResult::NotConfigured;
    }
    match std::fs::metadata(trimmed) {
        Ok(meta) => PathCheckResult::Ok(meta),
        Err(e) => match e.kind() {
            io::ErrorKind::NotFound => PathCheckResult::Missing,
            _ => PathCheckResult::Inaccessible,
        },
    }
}

/// Required file: empty → Fatal (Broken), missing → Warning (Stale),
/// inaccessible → Fatal (Broken), not a file → Fatal (Broken).
fn check_required_file(
    value: &str,
    label: &str,
    help: &str,
    issues: &mut Vec<LaunchValidationIssue>,
) {
    match check_path_metadata(value) {
        PathCheckResult::Ok(meta) => {
            if !meta.is_file() {
                issues.push(LaunchValidationIssue {
                    message: format!("{label} exists but is not a file: {}", value.trim()),
                    help: help.to_string(),
                    severity: ValidationSeverity::Fatal,
                });
            }
        }
        PathCheckResult::NotConfigured => {
            issues.push(LaunchValidationIssue {
                message: format!("{label} path is not configured."),
                help: help.to_string(),
                severity: ValidationSeverity::Fatal,
            });
        }
        PathCheckResult::Missing => {
            issues.push(LaunchValidationIssue {
                message: format!("{label} does not exist: {}", value.trim()),
                help: help.to_string(),
                severity: ValidationSeverity::Warning, // Stale — external change
            });
        }
        PathCheckResult::Inaccessible => {
            issues.push(LaunchValidationIssue {
                message: format!("{label} is not accessible (permission denied): {}", value.trim()),
                help: "Check file permissions on the path or run CrossHook with appropriate access.".to_string(),
                severity: ValidationSeverity::Fatal,
            });
        }
        PathCheckResult::WrongType => unreachable!(), // handled in Ok branch
    }
}

/// Required directory: same severity pattern as check_required_file but checks is_dir().
fn check_required_directory(
    value: &str,
    label: &str,
    help: &str,
    issues: &mut Vec<LaunchValidationIssue>,
) {
    match check_path_metadata(value) {
        PathCheckResult::Ok(meta) => {
            if !meta.is_dir() {
                issues.push(LaunchValidationIssue {
                    message: format!("{label} exists but is not a directory: {}", value.trim()),
                    help: help.to_string(),
                    severity: ValidationSeverity::Fatal,
                });
            }
        }
        PathCheckResult::NotConfigured => {
            issues.push(LaunchValidationIssue {
                message: format!("{label} path is not configured."),
                help: help.to_string(),
                severity: ValidationSeverity::Fatal,
            });
        }
        PathCheckResult::Missing => {
            issues.push(LaunchValidationIssue {
                message: format!("{label} does not exist: {}", value.trim()),
                help: help.to_string(),
                severity: ValidationSeverity::Warning,
            });
        }
        PathCheckResult::Inaccessible => {
            issues.push(LaunchValidationIssue {
                message: format!("{label} is not accessible (permission denied): {}", value.trim()),
                help: "Check directory permissions or run CrossHook with appropriate access.".to_string(),
                severity: ValidationSeverity::Fatal,
            });
        }
        PathCheckResult::WrongType => unreachable!(),
    }
}

/// Required executable: checks file existence + execute permission bits.
fn check_required_executable(
    value: &str,
    label: &str,
    help: &str,
    issues: &mut Vec<LaunchValidationIssue>,
) {
    match check_path_metadata(value) {
        PathCheckResult::Ok(meta) => {
            let path = Path::new(value.trim());
            if !crate::launch::request::is_executable_file(path) {
                issues.push(LaunchValidationIssue {
                    message: format!("{label} exists but is not executable: {}", value.trim()),
                    help: help.to_string(),
                    severity: ValidationSeverity::Fatal,
                });
            }
        }
        PathCheckResult::NotConfigured => {
            issues.push(LaunchValidationIssue {
                message: format!("{label} path is not configured."),
                help: help.to_string(),
                severity: ValidationSeverity::Fatal,
            });
        }
        PathCheckResult::Missing => {
            issues.push(LaunchValidationIssue {
                message: format!("{label} does not exist: {}", value.trim()),
                help: help.to_string(),
                severity: ValidationSeverity::Warning,
            });
        }
        PathCheckResult::Inaccessible => {
            issues.push(LaunchValidationIssue {
                message: format!("{label} is not accessible (permission denied): {}", value.trim()),
                help: "Check file permissions or run CrossHook with appropriate access.".to_string(),
                severity: ValidationSeverity::Fatal,
            });
        }
        PathCheckResult::WrongType => unreachable!(),
    }
}

/// Optional file: empty → skip, missing → Warning, inaccessible → Warning, wrong type → Info.
fn check_optional_file(
    value: &str,
    label: &str,
    help: &str,
    issues: &mut Vec<LaunchValidationIssue>,
) {
    match check_path_metadata(value) {
        PathCheckResult::Ok(meta) => {
            if !meta.is_file() {
                issues.push(LaunchValidationIssue {
                    message: format!("{label} exists but is not a file: {}", value.trim()),
                    help: help.to_string(),
                    severity: ValidationSeverity::Info,
                });
            }
        }
        PathCheckResult::NotConfigured => {} // Optional — not configured is fine
        PathCheckResult::Missing => {
            issues.push(LaunchValidationIssue {
                message: format!("{label} does not exist: {}", value.trim()),
                help: help.to_string(),
                severity: ValidationSeverity::Warning,
            });
        }
        PathCheckResult::Inaccessible => {
            issues.push(LaunchValidationIssue {
                message: format!("{label} is not accessible (permission denied): {}", value.trim()),
                help: "Check file permissions or run CrossHook with appropriate access.".to_string(),
                severity: ValidationSeverity::Warning,
            });
        }
        PathCheckResult::WrongType => unreachable!(),
    }
}
```

> **Note on `PathCheckResult::WrongType`**: This variant exists in the `HealthIssueKind` business rule but is not produced by `check_path_metadata()`. The wrong-type detection happens in the `Ok(meta)` branch of each caller (checking `meta.is_file()` or `meta.is_dir()`). The enum is kept for conceptual completeness and Phase 2 `code` field mapping.

> **Note on `is_executable_file` import path**: After promotion, the function is available at `crate::launch::request::is_executable_file` (or re-exported via `crate::launch::is_executable_file` if added to `launch/mod.rs` re-exports).

---

## Summary of Revision 4 Concessions and Holds

| #    | Topic                  | Decision                                                | Rationale                                                                                |
| ---- | ---------------------- | ------------------------------------------------------- | ---------------------------------------------------------------------------------------- |
| R-25 | `HealthIssueKind`      | **Accept as internal enum**                             | Drives severity + message selection; does NOT cross IPC in v1; Phase 2 adds `code` field |
| R-26 | Helper reuse           | **Option C** → re-implement with `metadata()` matching  | Existing helpers use `Path::exists()` — no EACCES detection; Option B as follow-up       |
| R-27 | Single-profile command | **Accept** → `check_profile_health` for auto-revalidate | Already in R-13; trigger on `profile_save` confirmed                                     |
| R-28 | No persistence         | **Confirmed**                                           | In-memory only; `checked_at` is display-only                                             |
| R-29 | Notification rules     | **Accept**                                              | Broken → startup banner; Stale → badge only                                              |
| R-30 | No auto-repair         | **Confirmed**                                           | Diagnostic only; all repair via ProfileEditor                                            |
| R-31 | No AppSettings         | **Confirmed**                                           | `steam_client_install_path` derived at launch time, not stored                           |
