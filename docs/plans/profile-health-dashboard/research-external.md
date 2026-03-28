# External API & Library Research: Profile Health Dashboard

**Feature**: Profile health dashboard with staleness detection (GitHub issue #38)
**Research Date**: 2026-03-28 (second pass — revised for SQLite3 metadata layer)
**Researcher**: research-specialist

---

## Changelog Since First Pass (2026-03-27)

| Section                       | Change                                                                                                 |
| ----------------------------- | ------------------------------------------------------------------------------------------------------ |
| Executive Summary             | Updated: "no persistence" assumption removed; health data now persistable via existing `MetadataStore` |
| §1.3 notify crate             | Unchanged — still not recommended                                                                      |
| §1.4 Tauri State              | Still valid, still recommended for in-memory cache layer                                               |
| §1.5 Tauri Events             | Unchanged                                                                                              |
| §2.1–2.3 Integration Patterns | Unchanged                                                                                              |
| §3.x FS Considerations        | Unchanged                                                                                              |
| §4.x Constraints              | Updated §4.1 (converter still needed); added §4.5 (fail-soft MetadataStore pattern)                    |
| **NEW §6**                    | SQLite health persistence: schema, rusqlite query patterns, index strategy, JSON extraction — **Phase D forward spec only** |
| **NEW §7**                    | Existing MetadataStore APIs inventory — what Phase A calls directly (no new MetadataStore code)        |
| §8 Alternatives               | Added SQLite persistence row for health snapshots table                                                |
| §9 Open Questions             | Updated for persistence layer questions                                                                |

---

## Phase Boundary Summary (added 2026-03-28 after practices/recommendations review)

| Phase | MetadataStore changes | What's used |
|---|---|---|
| **A & B** | **None** — zero new tables, zero new methods, zero new migrations | Call existing `query_failure_trends(days)` and `query_last_success_per_profile()` from `batch_check_health()` in `profile/health.rs` via `Option<&MetadataStore>` parameter |
| **D** | Migration 6 (`profile_health_snapshots`), new `health_snapshots.rs` submodule, 3-4 new `impl MetadataStore` methods | Full persistence + `query_health_with_launch_context()` |

**Phase A MetadataStore surface**: `Option<&MetadataStore>` parameter in `crosshook-core/src/profile/health.rs::batch_check_health()`. That is the only new surface. It lives in the health module, not in the metadata module.

The §6 forward spec (migration 6 schema, rusqlite INSERT patterns, index strategy) is correct and ready for Phase D — it does not belong in Phase A implementation.

---

## Executive Summary

The profile health dashboard can be implemented with **zero new Rust crate dependencies**. The existing `crosshook-core` already contains:

- A complete `ValidationError` enum and `LaunchValidationIssue` struct with Serde
- Synchronous `validate_all()`, `require_directory()`, `require_executable_file()`, and `is_executable_file()` functions in `launch/request.rs`
- Tokio (with `fs`, `rt`, `sync` features) already in `Cargo.toml`
- `ProfileStore::list()` + `ProfileStore::load()` in `profile/toml_store.rs`
- **[NEW]** `MetadataStore` with `rusqlite 0.38.0` (bundled SQLite 3.51.1) providing launch history, failure trends, and profile identity — health persistence is essentially "free" infrastructure

**Second-pass key finding**: The original Business Rule 8 ("No Persistence") is now invalidated. Health results can be persisted to the existing `metadata.db` via a new `profile_health_snapshots` table added as migration 6. The `MetadataStore` pattern (fail-soft via `available` flag, `with_conn` wrapper, UUID-keyed rows) is the established extension point.

**Confidence**: High — based on direct codebase inspection of `metadata/mod.rs`, `metadata/models.rs`, `metadata/migrations.rs`, `metadata/launch_history.rs`, and authoritative rusqlite/SQLite documentation.

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

### 4.5 [NEW] MetadataStore Fail-Soft Pattern Must Be Respected

`MetadataStore` uses a fail-soft pattern: when `available = false`, every `with_conn` call returns `Ok(T::default())` silently. Any health persistence code added to `MetadataStore` must follow this pattern — health snapshot writes that fail must not surface as errors to the user or abort health checks. The validation result itself is the primary output; persistence is supplementary.

This is critical because the SQLite metadata database may be unavailable on first run, after a corrupt migration, or on systems where `BaseDirs` fails.

**Confidence**: High — derived directly from `with_conn` implementation in `metadata/mod.rs:67-83`.

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
    // [NEW] optional — populated if MetadataStore available
    pub last_launch_succeeded_at: Option<String>,
    pub recent_failure_count: Option<i64>,
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
        Self { name, status, issues, last_launch_succeeded_at: None, recent_failure_count: None }
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

## 6. [NEW] SQLite Health Persistence via MetadataStore

### 6.1 Dependency Status — Exact Confirmed Versions

From `Cargo.lock` inspection (not the task brief, which cited 0.39):

| Crate            | Version in Cargo.toml | Resolved in Cargo.lock | Bundled SQLite |
| ---------------- | --------------------- | ---------------------- | -------------- |
| `rusqlite`       | `^0.38`               | `0.38.0`               | **3.51.1**     |
| `libsqlite3-sys` | (transitive)          | `0.36.0`               | —              |
| `uuid`           | `^1`                  | confirmed              | v4 + serde     |
| `tokio`          | `^1`                  | confirmed              | —              |
| `chrono`         | `^0.4`                | confirmed              | —              |

**Correction vs. task brief**: The brief cited rusqlite 0.39.0 bundling SQLite 3.51.3. The actual lock file shows `rusqlite 0.38.0` bundling SQLite **3.51.1**. No upgrade is needed or implied — 0.38.0 is sufficient for all health persistence requirements.

**Confidence**: High — verified via `Cargo.lock` grep and rusqlite 0.38.0 release notes.

### 6.2 Proposed Health Snapshot Table (Migration 6)

The existing migration pattern (sequential version bumps, `execute_batch` SQL strings, `pragma user_version`) is the established convention. A new `profile_health_snapshots` table fits cleanly as migration 6:

```sql
-- Migration 6: profile health snapshots
CREATE TABLE IF NOT EXISTS profile_health_snapshots (
    snapshot_id     TEXT PRIMARY KEY,
    profile_id      TEXT REFERENCES profiles(profile_id),
    profile_name    TEXT NOT NULL,
    health_status   TEXT NOT NULL,          -- 'healthy' | 'stale' | 'broken'
    issue_count     INTEGER NOT NULL DEFAULT 0,
    issues_json     TEXT,                   -- JSON array of LaunchValidationIssue, capped at 4KB
    checked_at      TEXT NOT NULL           -- RFC3339
);
CREATE INDEX IF NOT EXISTS idx_health_snapshots_profile_name ON profile_health_snapshots(profile_name);
CREATE INDEX IF NOT EXISTS idx_health_snapshots_checked_at   ON profile_health_snapshots(checked_at);
```

**Design rationale**:

- `profile_id` is nullable (same pattern as `launch_operations.profile_id`) — handles profiles not yet in the profiles table
- `issues_json` capped at 4KB matches `MAX_DIAGNOSTIC_JSON_BYTES` — consistent defensive bound
- Two indexes cover the two query patterns: by-name (load latest snapshot) and by-time (trend/history queries)
- No TTL column needed at this tier — eviction can be by row count or age in a sweep function
- Keep snapshots append-only; read latest via `MAX(checked_at)` per profile_name — consistent with `query_last_success_per_profile()` pattern

**Confidence**: High — derived from existing schema conventions in `migrations.rs`.

### 6.3 rusqlite Query Patterns for Health Data

**Pattern 1: Record a health snapshot (INSERT)**

```rust
// In metadata/health_snapshots.rs (new file, follows launch_history.rs pattern)
use super::{db, MetadataStoreError};
use super::profile_sync::lookup_profile_id;
use chrono::Utc;
use rusqlite::{params, Connection};

pub fn record_health_snapshot(
    conn: &Connection,
    profile_name: &str,
    health_status: &str,
    issue_count: usize,
    issues_json: Option<&str>,
) -> Result<(), MetadataStoreError> {
    let snapshot_id = db::new_id();
    let profile_id = lookup_profile_id(conn, profile_name)?;
    let now = Utc::now().to_rfc3339();
    // Cap issues_json at 4KB — same pattern as diagnostic_json in launch_history
    let issues_json = issues_json.filter(|s| s.len() <= super::models::MAX_DIAGNOSTIC_JSON_BYTES);

    conn.execute(
        "INSERT INTO profile_health_snapshots
         (snapshot_id, profile_id, profile_name, health_status, issue_count, issues_json, checked_at)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![snapshot_id, profile_id, profile_name, health_status, issue_count as i64, issues_json, now],
    )
    .map_err(|source| MetadataStoreError::Database {
        action: "insert a health snapshot row",
        source,
    })?;
    Ok(())
}
```

**Pattern 2: Query latest snapshot per profile**

```rust
pub fn query_latest_health_per_profile(
    conn: &Connection,
) -> Result<Vec<(String, String, String)>, MetadataStoreError> {
    // Returns (profile_name, health_status, checked_at) for the most recent snapshot
    let mut stmt = conn.prepare(
        "SELECT profile_name, health_status, MAX(checked_at) as checked_at
         FROM profile_health_snapshots
         GROUP BY profile_name
         ORDER BY profile_name"
    ).map_err(|source| MetadataStoreError::Database {
        action: "prepare query_latest_health_per_profile",
        source,
    })?;

    let rows = stmt.query_map([], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?, row.get::<_, String>(2)?))
    }).map_err(|source| MetadataStoreError::Database {
        action: "execute query_latest_health_per_profile",
        source,
    })?;

    let mut result = Vec::new();
    for row in rows {
        result.push(row.map_err(|source| MetadataStoreError::Database {
            action: "read a query_latest_health_per_profile row",
            source,
        })?);
    }
    Ok(result)
}
```

**Pattern 3: Health + launch history enrichment (JOIN across tables)**

This query enriches health results with launch failure trends — the key value of integrating with the existing `launch_operations` table:

```rust
// Correlated query: health status + recent failure count per profile
// Mirrors the existing query_failure_trends pattern using FILTER (WHERE ...)
// SQLite 3.30+ required for FILTER clause — confirmed present in SQLite 3.51.1
pub fn query_health_with_launch_context(
    conn: &Connection,
    days: u32,
) -> Result<Vec<HealthWithLaunchContext>, MetadataStoreError> {
    let interval = format!("-{days} days");
    let mut stmt = conn.prepare(
        "SELECT
             h.profile_name,
             h.health_status,
             h.checked_at,
             COUNT(*) FILTER (WHERE lo.status = 'failed')   AS recent_failures,
             COUNT(*) FILTER (WHERE lo.status = 'succeeded') AS recent_successes,
             MAX(CASE WHEN lo.status = 'succeeded' THEN lo.finished_at END) AS last_success_at
         FROM (
             SELECT profile_name, health_status, MAX(checked_at) AS checked_at
             FROM profile_health_snapshots
             GROUP BY profile_name
         ) h
         LEFT JOIN launch_operations lo
             ON lo.profile_name = h.profile_name
             AND lo.started_at >= datetime('now', ?1)
         GROUP BY h.profile_name, h.health_status, h.checked_at
         ORDER BY h.profile_name"
    ).map_err(|source| MetadataStoreError::Database {
        action: "prepare query_health_with_launch_context",
        source,
    })?;
    // ... query_map rows into HealthWithLaunchContext struct
}
```

**FILTER clause support note**: SQLite 3.30.0 (released 2019-10-04) introduced `FILTER (WHERE expr)` for aggregate functions. The bundled SQLite 3.51.1 fully supports this. The existing `query_failure_trends()` already uses `FILTER (WHERE status = '...')` — confirming it works in this codebase.

**Confidence**: High — FILTER clause confirmed working in existing code at `metadata/mod.rs:444-453`.

### 6.4 JSON Extraction from `diagnostic_json` Column

The `launch_operations.diagnostic_json` column stores serialized `DiagnosticReport` (capped 4KB). For health dashboard purposes, failure mode extraction is possible with SQLite's `json_extract()`:

```sql
-- Extract failure_mode from diagnostic_json (SQLite 3.38+ ->> operator, or json_extract)
SELECT
    profile_name,
    json_extract(diagnostic_json, '$.exit_info.failure_mode') AS failure_mode,
    COUNT(*) AS occurrences
FROM launch_operations
WHERE status = 'failed'
  AND diagnostic_json IS NOT NULL
  AND profile_name = ?1
GROUP BY failure_mode
ORDER BY occurrences DESC
LIMIT 5;
```

**Note**: This uses `json_extract()` which is available in SQLite 3.9+. The `->>` shorthand requires SQLite 3.38+ — available in 3.51.1. However, since `failure_mode` is also stored as a promoted column (`launch_operations.failure_mode TEXT`), direct column access is simpler and more efficient than JSON extraction.

**Recommendation**: Use promoted `failure_mode` column directly. The `diagnostic_json` is useful only for surfacing deeper diagnostic text (e.g., `$.suggestions[0].description`) in the health detail panel.

**Confidence**: High — from SQLite JSON docs and codebase inspection of `launch_history.rs:76-82`.

### 6.5 Index Strategy for Health Queries

Existing indexes in `metadata.db`:

| Table               | Index                           | Covers                                         |
| ------------------- | ------------------------------- | ---------------------------------------------- |
| `launch_operations` | `idx_launch_ops_profile_id`     | JOIN on `profile_id`                           |
| `launch_operations` | `idx_launch_ops_started_at`     | Time-range `WHERE started_at >= datetime(...)` |
| `profiles`          | `idx_profiles_current_filename` | Lookup by name                                 |

Proposed additional index for health snapshots:

- `idx_health_snapshots_profile_name` — covers `GROUP BY profile_name`, `WHERE profile_name = ?`
- `idx_health_snapshots_checked_at` — covers `MAX(checked_at)` aggregate, sweep queries

**Composite index consideration**: For the enrichment query (Pattern 3), a composite index on `launch_operations(profile_name, started_at)` would accelerate the correlated subquery. However, the existing `idx_launch_ops_started_at` + SQLite's B-tree optimization for append-only tables (naturally ordered by insertion/time) is likely sufficient for CrossHook's scale (< 1000 launch records per profile). Defer composite index until profiling indicates need.

**WAL mode**: `metadata.db` does not appear to set WAL mode explicitly. For health snapshot writes during startup (concurrent with read queries from the UI), WAL mode (`PRAGMA journal_mode=WAL`) would reduce write contention. This is an optimization, not a blocker.

**Confidence**: Medium — index strategy analysis based on SQLite optimizer documentation and CrossHook's data scale; no profiling data available.

---

## 7. [NEW] Existing MetadataStore APIs Relevant to Health Dashboard

These APIs already exist — no new code needed to query them:

| MetadataStore Method               | Signature                  | Health Relevance                                                                    |
| ---------------------------------- | -------------------------- | ----------------------------------------------------------------------------------- |
| `query_failure_trends(days)`       | `-> Vec<FailureTrendRow>`  | Provides `(profile_name, successes, failures, failure_modes)` for health enrichment |
| `query_last_success_per_profile()` | `-> Vec<(String, String)>` | Provides `(profile_name, last_success_at)` — direct health enrichment field         |
| `query_most_launched(limit)`       | `-> Vec<(String, i64)>`    | Usage frequency context for health prioritization                                   |
| `lookup_profile_id(name)`          | `-> Option<String>`        | Needed when inserting health snapshots to populate `profile_id` FK                  |
| `sweep_abandoned_operations()`     | `-> usize`                 | Cleanup pattern — health sweep function should follow the same pattern              |

**`FailureTrendRow` fields** (from `models.rs`):

- `profile_name: String`
- `successes: i64`
- `failures: i64`
- `failure_modes: Option<String>` — comma-delimited from `GROUP_CONCAT(DISTINCT failure_mode)`

**Business rule implication**: The health dashboard's "Stale" status tier can be enriched with `query_failure_trends(30)` data — a profile may pass path validation (no broken files) yet have a high failure rate from runtime errors. This creates a richer 4-tier classification:

1. `Healthy` — valid paths + no recent failures
2. `Stale` — valid paths + warning/info issues only
3. `Unreliable` — valid paths + high failure rate in `launch_operations`
4. `Broken` — any fatal path validation issue

The `Unreliable` tier is only possible with the MetadataStore integration and was not in the original spec.

**Confidence**: High — APIs verified by reading `metadata/mod.rs:362-483`.

---

## 8. Alternatives Evaluated

| Option                                                 | Pros                                                              | Cons                                                            | Verdict                                              |
| ------------------------------------------------------ | ----------------------------------------------------------------- | --------------------------------------------------------------- | ---------------------------------------------------- |
| `notify` crate (file watching)                         | Real-time updates, event-driven                                   | Wrong trigger model, complexity, new dependency                 | Reject for Phase 2                                   |
| `rayon` parallel iterator                              | Simple CPU parallelism                                            | Sync only, adds dependency, no IPC integration                  | Reject — JoinSet is better fit                       |
| `tokio::fs::try_exists` per path                       | Proper async, TOCTOU safe                                         | Each call is a spawn_blocking internally; more syscalls         | Use in new code but not required to replace existing |
| Reuse existing sync `validate_all` in `spawn_blocking` | Zero new dependencies, existing logic, existing error types       | Profile→Request conversion needed                               | **Recommended**                                      |
| Polling timer (`tokio::time::interval`)                | Auto-refresh without user action                                  | Background overhead, complexity, unreliable when app is idle    | Optional Phase 3                                     |
| **[NEW]** Health snapshot table in `metadata.db`       | Free persistence via existing `MetadataStore`; enables trend view | Additional migration, issues_json serialization                 | **Recommended for Phase 2**                          |
| **[NEW]** Health results in frontend-only state        | Simple, no migration needed                                       | Lost on restart, cannot show history — original Business Rule 8 | Downgrade to "acceptable" (no longer required)       |

---

## 9. Open Questions (Updated)

1. **`LaunchRequest::from_profile()`**: This converter needs to be written. Should it live in `profile/health.rs` or `profile/`? It mirrors the reverse of profile→launch flow. **New consideration**: if health enrichment queries `MetadataStore`, the `profile/health.rs` module needs to accept `Option<&MetadataStore>` as a parameter.

2. **Health snapshot retention policy**: How many snapshots to retain per profile? Options: (a) keep only the latest (UPDATE instead of INSERT), (b) keep last N per profile (sweep on insert), (c) keep all with periodic vacuum. The append-only pattern (`launch_operations` style) plus a `sweep_old_health_snapshots(days)` function is most consistent.

3. **`Unreliable` tier adoption**: Should the health dashboard expose a 4-tier status (`Healthy / Stale / Unreliable / Broken`) leveraging `query_failure_trends()`, or keep the 3-tier model from the original spec for simplicity? Business decision — requires input from business-analyzer.

4. **IPC boundary serialization**: `LaunchValidationIssue` already derives `Serialize`/`Deserialize`. `ProfileHealthStatus` and `ProfileHealthResult` will need the same. Verify no `anyhow::Error` leaks across the IPC boundary (it doesn't implement Serialize).

5. **MetadataStore availability at health check time**: The Tauri command handler must accept `State<'_, MetadataStore>` (injected via `app.manage()`). The fail-soft pattern means health checks proceed even if SQLite is unavailable — results just won't be persisted and enrichment fields will be `None`.

---

## 10. Sources

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
- [rusqlite 0.38.0 release notes](https://github.com/rusqlite/rusqlite/releases/tag/v0.38.0) — bundles SQLite 3.51.1, breaking changes, minimal SQLite 3.34.1
- [rusqlite docs.rs](https://docs.rs/rusqlite/0.38.0/rusqlite/) — prepare, query_map, Statement API
- [SQLite FILTER clause](https://til.simonwillison.net/sqlite/sqlite-aggregate-filter-clauses) — available since SQLite 3.30.0 (2019-10-04)
- [SQLite FILTER syntax](https://sqlite.org/lang_aggfunc.html) — official docs confirming support
- [SQLite json_extract](https://www.sqlitetutorial.net/sqlite-json-functions/sqlite-json_extract-function/) — JSON column query patterns
- [SQLite JSON operators](https://sqlite.org/json1.html) — `->>` (3.38+), `json_extract()` (3.9+)
- [SQLite index strategy for time range queries](https://blog.sqlite.ai/choosing-the-right-index-in-sqlite) — multi-column index guidance
- [SQLite query optimizer](https://sqlite.org/optoverview.html) — range query index usage
- [SQLite best practices (Android)](https://developer.android.com/topic/performance/sqlite-performance-best-practices) — WAL mode, transaction batching
- [crosshook-core/src/metadata/mod.rs](../../src/crosshook-native/crates/crosshook-core/src/metadata/mod.rs) — MetadataStore API inventory (source inspection)
- [crosshook-core/src/metadata/migrations.rs](../../src/crosshook-native/crates/crosshook-core/src/metadata/migrations.rs) — existing migration pattern (source inspection)
- [crosshook-core/src/metadata/launch_history.rs](../../src/crosshook-native/crates/crosshook-core/src/metadata/launch_history.rs) — rusqlite INSERT/UPDATE patterns (source inspection)
- [crosshook-core/src/metadata/models.rs](../../src/crosshook-native/crates/crosshook-core/src/metadata/models.rs) — FailureTrendRow, MAX_DIAGNOSTIC_JSON_BYTES (source inspection)
- [crosshook-core/Cargo.toml](../../src/crosshook-native/crates/crosshook-core/Cargo.toml) — dependency versions (source inspection)
- [crosshook-native/Cargo.lock](../../src/crosshook-native/Cargo.lock) — resolved rusqlite 0.38.0, libsqlite3-sys 0.36.0 (source inspection)

---

## 11. Search Queries Executed

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
15. `rusqlite 0.38 changelog API docs.rs SQLite bundled version`
16. `SQLite "FILTER (WHERE" aggregate syntax version support window functions health trend query`
17. `rusqlite "libsqlite3-sys" 0.36 bundled SQLite version 3.50 OR 3.48 OR 3.47`
18. `SQLite health snapshot table design "check_results" OR "health_snapshots" persist application health status schema 2024`
19. `SQLite JSON extraction "json_extract" aggregate query diagnostic column rusqlite health trend failure mode 2024`
20. `SQLite index strategy "started_at" time range queries performance health monitoring append-only table 2024`
