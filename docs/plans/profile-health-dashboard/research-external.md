# External API & Library Research: Profile Health Dashboard

**Feature**: Profile health dashboard with staleness detection (GitHub issue #38)
**Research Date**: 2026-03-27
**Researcher**: research-specialist

---

## Executive Summary

The profile health dashboard can be implemented with **zero new Rust crate dependencies**. The existing `crosshook-core` already contains:

- A complete `ValidationError` enum and `LaunchValidationIssue` struct with Serde
- Synchronous `validate_all()`, `require_directory()`, `require_executable_file()`, and `is_executable_file()` functions in `launch/request.rs`
- Tokio (with `fs`, `rt`, `sync` features) already in `Cargo.toml`
- `ProfileStore::list()` + `ProfileStore::load()` in `profile/toml_store.rs`

The recommended architecture wraps existing sync validation in `tokio::task::spawn_blocking()` per profile, batches them with `tokio::task::JoinSet`, stores aggregate results in `app.manage(Mutex<HealthState>)`, and pushes updates to the frontend via `app.emit()`. File system watching (notify crate) is NOT recommended for this use case — on-demand + startup batch validation is simpler and sufficient.

**Confidence**: High — based on direct codebase inspection and authoritative Tokio/Tauri v2 documentation.

---

## Primary APIs & Libraries

### 1.1 Tokio (Already in Cargo.toml)

**Version**: `tokio = { version = "1", features = ["fs", "process", "rt", "sync"] }`
**Docs**: <https://docs.rs/tokio/latest/tokio/>
**Maintenance**: Actively maintained (tokio-rs org), de facto async runtime for Rust.

**Relevant sub-APIs for health dashboard:**

| API                           | Purpose                                                | Notes                                                    |
| ----------------------------- | ------------------------------------------------------ | -------------------------------------------------------- |
| `tokio::task::JoinSet`        | Batch concurrent tasks, collect results as they finish | Requires `rt` feature (already enabled)                  |
| `tokio::task::spawn_blocking` | Run sync `std::fs` calls off the async executor        | Used internally by `tokio::fs` anyway                    |
| `tokio::fs::try_exists`       | Async path existence check                             | Returns `Result<bool>` — preferred over `Path::exists()` |
| `tokio::fs::metadata`         | Full async metadata (type, permissions)                | Handles symlinks, executable bits                        |
| `tokio::sync::Mutex`          | Async-safe mutex for holding locks across `.await`     | Only needed if health state update involves async I/O    |
| `std::sync::Mutex`            | Sync mutex — Tauri recommends this for most state      | No `await` needed in state update path                   |

**Key performance note from Tokio docs**: "Most operating systems do not provide asynchronous file system APIs." Tokio uses `spawn_blocking` internally for all `tokio::fs` calls. For batch validation of N profiles, spawning one `spawn_blocking` task per profile (each running the full sync `validate_all()`) is more efficient than calling `tokio::fs` functions per-path, because it minimizes task-switching overhead.

**Confidence**: High — directly from <https://docs.rs/tokio/latest/tokio/fs/index.html>

### 1.2 Existing crosshook-core Validation Infrastructure (No New Dependency)

**Location**: `src/crosshook-native/crates/crosshook-core/src/launch/request.rs`

The existing code already provides everything needed for path health checks:

```rust
// Already exists — these are the building blocks for health dashboard
pub fn validate_all(request: &LaunchRequest) -> Vec<LaunchValidationIssue>;
pub fn validate(request: &LaunchRequest) -> Result<(), ValidationError>;

// Internal helpers (private, but can be extracted to pub(crate) if needed)
fn require_directory(value, required_err, missing_err, not_dir_err) -> Result<_, ValidationError>;
fn require_executable_file(value, required_err, missing_err, not_exec_err) -> Result<(), ValidationError>;
fn is_executable_file(path: &Path) -> bool;  // Uses unix PermissionsExt::mode() & 0o111
```

**The `ValidationError` enum** already models all broken-path cases:

- `GamePathMissing`, `GamePathNotFile`
- `TrainerHostPathMissing`, `TrainerHostPathNotFile`
- `SteamCompatDataPathMissing`, `SteamCompatDataPathNotDirectory`
- `SteamProtonPathMissing`, `SteamProtonPathNotExecutable`
- `RuntimePrefixPathMissing`, `RuntimePrefixPathNotDirectory`
- `RuntimeProtonPathMissing`, `RuntimeProtonPathNotExecutable`

**The `ValidationSeverity` enum** (Fatal/Warning/Info) with `#[derive(Serialize, Deserialize)]` maps directly to health status tiers.

**Confidence**: High — verified by reading source code.

### 1.3 notify Crate (Evaluated — NOT Recommended for This Feature)

**Version**: `8.2.0` (released 2025-08-03)
**Docs**: <https://docs.rs/notify/latest/notify/>
**Repo**: <https://github.com/notify-rs/notify>
**Maintenance**: Active (notify-rs org), used by rust-analyzer, cargo-watch, Deno.

**What it does**: Cross-platform filesystem event watcher. On Linux uses inotify (`INotifyWatcher`). Companion crates: `notify-debouncer-mini` (0.6.0), `notify-debouncer-full`.

**Linux/SteamOS inotify limits** (confirmed from SteamOS sysctl dump):

- `fs.inotify.max_user_watches = 524288` — very permissive, not a constraint
- `fs.inotify.max_user_instances = 1024`
- `user.max_inotify_watches = 524288`

**Why NOT recommended** for profile health dashboard:

1. **Wrong trigger model**: Health dashboard checks existing profiles on demand and at startup — it doesn't need to watch for external file changes in real time.
2. **Complexity cost**: notify requires a background thread, channels, and event handling code. For a "check health when user asks / on startup" use case, this is significant overhead.
3. **False reliability**: A watcher would detect file moves/deletes, but not when a Proton installation becomes corrupted, or when a new game install path supersedes the old one. The validation logic is richer than "does the path still exist."
4. **Dependency weight**: Would add notify + crossbeam-channel + inotify crates to the build.

**When notify WOULD be appropriate**: Only if CrossHook needs real-time file change notifications (e.g., live badge update when a game executable is deleted while the app is open). This could be Phase 3.

**Confidence**: High.

### 1.4 Tauri v2 State Management (Already Available)

**Docs**: <https://v2.tauri.app/develop/state-management/>
**Pattern**: `app.manage(Mutex::new(HealthState::default()))` in setup hook.

```rust
// In src-tauri/src/lib.rs setup:
#[derive(Default)]
struct ProfileHealthCache {
    results: HashMap<String, ProfileHealthStatus>,
    last_checked: Option<chrono::DateTime<chrono::Utc>>,
}

app.manage(Mutex::new(ProfileHealthCache::default()));
```

**Access from commands**:

```rust
#[tauri::command]
fn get_profile_health(
    state: State<'_, Mutex<ProfileHealthCache>>,
) -> HashMap<String, ProfileHealthStatus> {
    state.lock().unwrap().results.clone()
}
```

**Access from background tasks** (via AppHandle):

```rust
let state = app_handle.state::<Mutex<ProfileHealthCache>>();
let mut cache = state.lock().unwrap();
cache.results = new_results;
```

**Important**: Tauri wraps state in `Arc` internally — do NOT add `Arc<Mutex<T>>`, use `Mutex<T>` directly. Use `std::sync::Mutex` (not `tokio::sync::Mutex`) unless the critical section spans an `.await` point.

**Confidence**: High — from <https://v2.tauri.app/develop/state-management/>

### 1.5 Tauri v2 Event Emission (Already Available)

**Docs**: <https://v2.tauri.app/develop/calling-frontend/>

Push health status updates to the frontend from background tasks:

```rust
use tauri::Emitter;

#[derive(Clone, serde::Serialize)]
struct ProfileHealthUpdate {
    profile_name: String,
    status: ProfileHealthStatus,  // "healthy" | "stale" | "broken"
    issues: Vec<LaunchValidationIssue>,
}

// In background task:
app_handle.emit("profile-health-update", &update).unwrap();
// Or all at once after batch validation:
app_handle.emit("profile-health-batch-complete", &all_results).unwrap();
```

**Frontend listener** (React/TypeScript):

```typescript
import { listen } from '@tauri-apps/api/event';

const unlisten = await listen<ProfileHealthBatchResult>('profile-health-batch-complete', (event) => {
  setHealthResults(event.payload);
});
```

**Confidence**: High — from <https://v2.tauri.app/develop/calling-frontend/> and <https://sneakycrow.dev/blog/2024-05-12-running-async-tasks-in-tauri-v2>

---

## Integration Patterns

### 2.1 Recommended: On-Demand Sequential Validation (v1)

For <50 profiles, stat-only validation completes in single-digit milliseconds sequentially. The Tauri command is already async; the inner loop can be synchronous. KISS — reach for `JoinSet` only if profiling reveals actual latency.

```rust
#[tauri::command]
async fn check_profiles_health() -> Result<Vec<ProfileHealthResult>, String> {
    let store = ProfileStore::new();
    let names = store.list().map_err(|e| e.to_string())?;
    let mut results = Vec::new();
    for name in names {
        match store.load(&name) {
            Ok(profile) => {
                // Converter lives in profile/health.rs (not launch/request.rs)
                let request = profile_to_launch_request(&profile);
                let issues = validate_all(&request);
                results.push(ProfileHealthResult::from_issues(name, issues));
            }
            Err(e) => results.push(ProfileHealthResult::from_load_error(name, e)),
        }
    }
    Ok(results)
}
```

**Module placement note**: `profile_to_launch_request()` belongs in `profile/health.rs` as a free function — not in `launch/request.rs`. This keeps the dependency direction profile→launch (already established: `launch/request.rs` imports `TrainerLoadingMode` from `profile`). Widening it with a full `GameProfile` parameter in `launch/` would deepen coupling unnecessarily.

**Command naming**: `check_profiles_health` preferred over `validate_all_profiles` — consistent with returning health status data (not pass/fail), and aligns with the existing `profile_*` CRUD / `validate_launch` naming convention. Confirm against `src-tauri/src/commands/` inventory.

**Confidence**: High.

### 2.2 Startup Background Validation with Event Push

```rust
// In src-tauri/src/startup.rs or lib.rs setup:
fn run_startup_health_check(app: &AppHandle) {
    let handle = app.clone();
    tauri::async_runtime::spawn(async move {
        // Delay slightly so frontend is ready to receive events
        // (frontend emits a "ready" event; listen for it before emitting)
        let store = ProfileStore::new();
        let names = match store.list() {
            Ok(n) => n,
            Err(_) => return,
        };

        let mut join_set: JoinSet<ProfileHealthResult> = JoinSet::new();
        for name in names {
            let s = store.clone();
            let n = name.clone();
            join_set.spawn_blocking(move || {
                let profile = s.load(&n).ok();
                let issues = profile
                    .map(|p| validate_all(&launch_request_from_profile(&p)))
                    .unwrap_or_default();
                ProfileHealthResult { name: n, issues }
            });
        }

        let mut all_results = Vec::new();
        while let Some(Ok(result)) = join_set.join_next().await {
            all_results.push(result);
        }

        // Persist to shared state
        if let Some(state) = handle.try_state::<Mutex<ProfileHealthCache>>() {
            let mut cache = state.lock().unwrap();
            for r in &all_results {
                cache.results.insert(r.name.clone(), r.to_status());
            }
        }

        // Push to frontend
        let _ = handle.emit("profile-health-batch-complete", &all_results);
    });
}
```

**Confidence**: High — pattern validated against Tauri v2 docs and sneakycrow blog.

### 2.3 Health Status Derivation from Existing ValidationIssue

The existing `ValidationSeverity` (Fatal/Warning/Info) maps cleanly to a 3-tier health status:

```rust
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ProfileHealthStatus {
    Healthy,    // zero issues
    Stale,      // only Warning/Info issues (e.g. optional paths missing)
    Broken,     // any Fatal issue (game path missing, Proton not found)
}

impl ProfileHealthStatus {
    pub fn from_issues(issues: &[LaunchValidationIssue]) -> Self {
        if issues.iter().any(|i| i.severity == ValidationSeverity::Fatal) {
            Self::Broken
        } else if !issues.is_empty() {
            Self::Stale
        } else {
            Self::Healthy
        }
    }
}
```

**Confidence**: High — derived from existing `ValidationSeverity` and `LaunchValidationIssue` in `request.rs`.

---

## 3. File System Considerations

### 3.1 Steam Deck Constraints

From SteamOS sysctl configuration (verified via GitHub gist):

- **inotify watches**: `fs.inotify.max_user_watches = 524288` — extremely permissive
- **file max**: `fs.file-max = 9223372036854775807` — effectively unlimited
- **inotify instances**: `fs.inotify.max_user_instances = 1024`

**Conclusion**: No inotify-related constraints affect this feature on Steam Deck. Even if notify were used, the limits are generous.

**Confidence**: High — from <https://gist.github.com/t-jonesy/2f6d2cc93c33bc6a538b4f4901493fa6>

### 3.2 Path Validation Gotchas

**`Path::exists()` vs `tokio::fs::try_exists()`**:

- `Path::exists()` follows symlinks and returns `false` on error (TOCTOU risk)
- `tokio::fs::try_exists()` returns `Result<bool>` — correctly surfaces permission errors vs. non-existence
- The existing code uses `path.exists()` synchronously — acceptable for health dashboard since it runs in `spawn_blocking`

**`std::fs::canonicalize()` limitations**:

- Fails on non-existing paths (returns `Err`)
- Do NOT use for validation of potentially broken paths — `path.exists()` + `path.is_file()`/`path.is_dir()` is correct as already implemented

**Executable bit checking** (already implemented correctly):

```rust
fn is_executable_file(path: &Path) -> bool {
    let metadata = match fs::metadata(path) {
        Ok(m) => m,
        Err(_) => return false,
    };
    // Linux/Unix:
    metadata.is_file() && metadata.permissions().mode() & 0o111 != 0
}
```

This is correct for detecting broken Proton installations on Linux.

**Confidence**: High.

### 3.3 Performance Estimate for Batch Validation

For typical CrossHook usage (5–30 profiles), each profile has 3–6 path checks (game exe, trainer, Proton, compatdata, prefix):

- Each `std::fs::metadata()` call: ~0.05–0.5ms on local SSD
- Per profile: ~0.3–3ms total (all sync, sequential within profile)
- Batch with JoinSet(N profiles): bounded by slowest profile, typically < 50ms for 30 profiles
- Startup overhead: negligible

No need for Rayon or complex parallelism. JoinSet with spawn_blocking is sufficient.

**Confidence**: Medium (no benchmark data specific to CrossHook, but consistent with general Rust file I/O guidance).

---

## 4. Constraints and Gotchas

### 4.1 No `LaunchRequest` Builder from `GameProfile` Yet

The health dashboard needs to construct a `LaunchRequest` from a `GameProfile`. Currently this conversion doesn't exist as a public function. The `validate_all()` function takes `&LaunchRequest`, not `&GameProfile`. A `from_profile()` function needs to be added to `crosshook-core`.

This is the **primary new code** required — everything else reuses existing infrastructure.

### 4.2 Tauri Frontend Readiness Race Condition

Emitting events from the Rust setup hook before the frontend has registered its listener means events are lost. The documented workaround: have the frontend emit a "ready" event after `useEffect` fires, then the Rust side triggers the health check.

Alternatively: expose `validate_all_profiles` as an explicit Tauri command so the frontend can call it when ready.

**Recommendation**: Use the explicit command approach (frontend calls `invoke('validate_all_profiles')` after mount). Simpler, no race condition.

### 4.3 Mutex Poisoning in Long-Running Health State

If `state.lock().unwrap()` is called while a panic occurs in another thread holding the lock, the Mutex becomes "poisoned." Prefer `state.lock().unwrap_or_else(|p| p.into_inner())` or use `RwLock` for a read-heavy health status cache (many reads, infrequent writes).

### 4.4 Health Check Granularity: Profile-Level vs. Path-Level

`validate_all()` validates a `LaunchRequest` against the resolved launch method. Some paths are only required for certain methods (e.g., `compatdata_path` only for `steam_applaunch`). The health dashboard should respect this — a profile with `steam_applaunch` method won't flag missing `runtime.prefix_path` as an issue.

This is already handled correctly by the existing `validate_all()` dispatch logic.

---

## 5. Code Examples

### 5.1 Minimum New Types Required

```rust
// In crosshook-core/src/profile/ (new file: health.rs)
use serde::{Deserialize, Serialize};
use crate::launch::{validate_all, LaunchValidationIssue, ValidationSeverity};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ProfileHealthStatus {
    Healthy,
    Stale,
    Broken,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProfileHealthResult {
    pub name: String,
    pub status: ProfileHealthStatus,
    pub issues: Vec<LaunchValidationIssue>,
}

impl ProfileHealthResult {
    pub fn from_issues(name: String, issues: Vec<LaunchValidationIssue>) -> Self {
        let status = if issues.iter().any(|i| i.severity == ValidationSeverity::Fatal) {
            ProfileHealthStatus::Broken
        } else if !issues.is_empty() {
            ProfileHealthStatus::Stale
        } else {
            ProfileHealthStatus::Healthy
        };
        Self { name, status, issues }
    }
}
```

### 5.2 New Tauri Command Skeleton

```rust
// In src-tauri/src/commands/profile.rs (add to existing file)
#[tauri::command]
pub async fn validate_all_profiles() -> Result<Vec<ProfileHealthResult>, String> {
    use tokio::task::JoinSet;
    let store = ProfileStore::new();
    let names = store.list().map_err(|e| e.to_string())?;

    let mut join_set: JoinSet<ProfileHealthResult> = JoinSet::new();
    for name in names {
        let s = store.clone();
        let n = name.clone();
        join_set.spawn_blocking(move || {
            match s.load(&n) {
                Ok(profile) => {
                    let request = LaunchRequest::from_profile(&profile);
                    let issues = validate_all(&request);
                    ProfileHealthResult::from_issues(n, issues)
                }
                Err(e) => ProfileHealthResult::from_load_error(n, e),
            }
        });
    }

    let mut results = Vec::new();
    while let Some(Ok(r)) = join_set.join_next().await {
        results.push(r);
    }
    Ok(results)
}
```

---

## 6. Alternatives Evaluated

| Option                                                 | Pros                                                        | Cons                                                         | Verdict                                              |
| ------------------------------------------------------ | ----------------------------------------------------------- | ------------------------------------------------------------ | ---------------------------------------------------- |
| `notify` crate (file watching)                         | Real-time updates, event-driven                             | Wrong trigger model, complexity, new dependency              | Reject for Phase 2                                   |
| `rayon` parallel iterator                              | Simple CPU parallelism                                      | Sync only, adds dependency, no IPC integration               | Reject — JoinSet is better fit                       |
| `tokio::fs::try_exists` per path                       | Proper async, TOCTOU safe                                   | Each call is a spawn_blocking internally; more syscalls      | Use in new code but not required to replace existing |
| Reuse existing sync `validate_all` in `spawn_blocking` | Zero new dependencies, existing logic, existing error types | Profile→Request conversion needed                            | **Recommended**                                      |
| Polling timer (`tokio::time::interval`)                | Auto-refresh without user action                            | Background overhead, complexity, unreliable when app is idle | Optional Phase 3                                     |

---

## 7. Open Questions

1. **`LaunchRequest::from_profile()`**: This converter needs to be written. Should it live in `profile/` or `launch/`? It mirrors the reverse of profile→launch flow.

2. **`steam_client_install_path` gap — RESOLVED (not applicable)**: `AppSettingsData` does not store `steam_client_install_path`. It is a runtime-derived value: `commands/profile.rs:derive_steam_client_install_path()` splits `steam.compatdata_path` on `/steamapps/compatdata/` to produce it. It is a `LaunchRequest` concern only. If the health check validates `GameProfile` fields directly (rather than routing through `LaunchRequest` + `validate_all()`), this field is never checked and the false-positive risk does not exist. `AppSettings` injection is not needed.

3. **Caching duration**: How stale is "stale"? Should the cache have a TTL (e.g., re-validate on next request if older than 5 minutes), or is it purely on-demand?

4. **IPC boundary serialization**: `LaunchValidationIssue` already derives `Serialize`/`Deserialize`. `ProfileHealthStatus` and `ProfileHealthResult` will need the same. Verify no `anyhow::Error` leaks across the IPC boundary (it doesn't implement Serialize).

---

## 8. Sources

- [notify crate docs](https://docs.rs/notify/latest/notify/) — v8.2.0 API, watcher types, Linux inotify notes
- [notify-debouncer-mini docs](https://docs.rs/notify-debouncer-mini/latest/notify_debouncer_mini/) — debouncing API
- [notify GitHub](https://github.com/notify-rs/notify) — maintenance status, changelog
- [tokio::fs module](https://docs.rs/tokio/latest/tokio/fs/index.html) — async file ops, spawn_blocking notes
- [tokio::fs::try_exists](https://docs.rs/tokio/latest/tokio/fs/fn.try_exists.html) — async existence check
- [Tauri v2 Calling Frontend](https://v2.tauri.app/develop/calling-frontend/) — AppHandle::emit, event patterns
- [Tauri v2 State Management](https://v2.tauri.app/develop/state-management/) — Mutex<T> pattern, AppHandle::state
- [Tauri v2 IPC overview](https://v2.tauri.app/concept/inter-process-communication/) — async commands, threading
- [Long-running async tasks in Tauri v2](https://sneakycrow.dev/blog/2024-05-12-running-async-tasks-in-tauri-v2) — AppHandle background task pattern
- [SteamOS sysctl configuration](https://gist.github.com/t-jonesy/2f6d2cc93c33bc6a538b4f4901493fa6) — confirmed inotify limits
- [tokio::task::JoinSet vs join_all](https://github.com/tokio-rs/tokio/discussions/6921) — batch task comparison
- [inotify watch limits](https://watchexec.github.io/docs/inotify-limits.html) — default values, Linux behavior
- [Rust path::canonicalize limitations](https://doc.rust-lang.org/std/fs/fn.canonicalize.html) — non-existing path behavior
- [crosshook-core/src/launch/request.rs](../../src/crosshook-native/crates/crosshook-core/src/launch/request.rs) — existing validation infrastructure (source inspection)
- [crosshook-core/src/profile/models.rs](../../src/crosshook-native/crates/crosshook-core/src/profile/models.rs) — GameProfile model (source inspection)
- [crosshook-core/Cargo.toml](../../src/crosshook-native/crates/crosshook-core/Cargo.toml) — existing dependencies (source inspection)

---

## 9. Search Queries Executed

1. `Rust notify crate file system watching inotify crates.io 2024 2025`
2. `Rust async path validation file existence checking tokio 2024`
3. `Tauri v2 IPC health check background task batch validation pattern`
4. `notify-debouncer-mini crate Rust filesystem watcher debounce 2024 2025`
5. `Tauri v2 emit event frontend background thread AppHandle async command startup validation`
6. `Rust rayon parallel iterator file validation batch processing performance benchmark`
7. `SteamOS Steam Deck inotify max_user_watches default value sysctl 2024`
8. `Rust "path::canonicalize" symlink resolution broken path validation error handling`
9. `Tauri v2 manage state AppState health check status shared state Mutex Arc startup`
10. `which crate rust executable finder PATH resolution crates.io docs`
11. `Rust anyhow thiserror error types validation result profile health check pattern 2024`
12. `Rust futures::future::join_all vs tokio::task::JoinSet batch concurrent tasks 2024 comparison`
13. `Steam Deck inotify watch limits Linux file descriptor constraints embedded hardware`
14. `tokio::fs batch file existence validation concurrent futures join_all performance`
