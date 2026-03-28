# Integration Research: Profile Health Dashboard

Second-pass integration research. All findings are verified against actual source files as of 2026-03-28.

---

## Tauri IPC Commands

### Command Registration Pattern

All commands are registered in `src-tauri/src/lib.rs:85` via `tauri::generate_handler![]`. New health commands must be added to this macro and exposed via `pub mod health` in `src-tauri/src/commands/mod.rs`.

### State Injection Pattern

Commands receive shared Tauri state via `State<'_, T>` parameters. Existing commands use these managed types:

| State Type      | How it's managed                        |
| --------------- | --------------------------------------- |
| `ProfileStore`  | `.manage(profile_store)` — `lib.rs:76`  |
| `MetadataStore` | `.manage(metadata_store)` — `lib.rs:80` |
| `SettingsStore` | `.manage(settings_store)` — `lib.rs:77` |

Health commands need `State<'_, ProfileStore>` and `State<'_, MetadataStore>` — both are already managed. Example from `commands/profile.rs:99-106`:

```rust
#[tauri::command]
pub fn profile_save(
    name: String,
    data: GameProfile,
    store: State<'_, ProfileStore>,
    metadata_store: State<'_, MetadataStore>,
) -> Result<(), String>
```

### Error Handling Pattern

All commands return `Result<T, String>`. The convention is `map_err(|e| e.to_string())`. MetadataStore failures are logged as warnings but never propagated as command errors — this is the existing fail-soft pattern used throughout `commands/profile.rs`.

### Async Commands vs Sync Commands

- **Sync** (`fn`): `profile_list`, `profile_load`, `profile_save` — blocking I/O is acceptable because the operations are fast. Health batch scan should also be `fn` wrapped in `spawn_blocking` per feature spec.
- **Async** (`async fn`): `launch_game`, `launch_trainer` — only needed when spawning long-running processes. Health commands do NOT need async.

### AppHandle for Event Emission

The startup health event push needs `AppHandle`. Pattern from `commands/launch.rs:49`:

```rust
#[tauri::command]
pub async fn launch_game(app: AppHandle, request: LaunchRequest) -> Result<LaunchResult, String>
```

For the `profile-health-batch-complete` startup event, the health check must be spawned from `lib.rs` setup closure using `tauri::async_runtime::spawn`, similar to the `auto-load-profile` pattern at `lib.rs:61-70`.

---

## MetadataStore API

### Connection Access Pattern (`metadata/mod.rs`)

MetadataStore wraps `Option<Arc<Mutex<Connection>>>`. All public methods go through `with_conn` or `with_conn_mut` — these automatically return `T::default()` when `available == false`, implementing the fail-soft pattern at no extra cost to callers. Health commands get this for free.

```rust
fn with_conn<F, T>(&self, action: &'static str, f: F) -> Result<T, MetadataStoreError>
where T: Default  // If unavailable, returns T::default()
```

### Health-Critical Methods

All methods below are on `MetadataStore` in `metadata/mod.rs`. All return `Result<T, MetadataStoreError>`.

#### `query_failure_trends(days: u32) -> Result<Vec<FailureTrendRow>, MetadataStoreError>`

**Location**: `metadata/mod.rs:437`

SQL: Counts `succeeded` and `failed` operations per `profile_name` in the last N days. Only returns rows where `failures > 0`.

```rust
pub struct FailureTrendRow {
    pub profile_name: String,
    pub successes: i64,
    pub failures: i64,
    pub failure_modes: Option<String>,  // comma-separated GROUP_CONCAT
}
```

**Degraded rule**: Profile is Degraded when `failures >= 2 && successes == 0`. `clean_exit` outcomes are stored as `LaunchOutcome::Succeeded`, not `Failed`, so they do not count toward failures.

#### `query_last_success_per_profile() -> Result<Vec<(String, String)>, MetadataStoreError>`

**Location**: `metadata/mod.rs:401`

Returns `(profile_name, MAX(finished_at))` for all profiles with at least one `succeeded` operation. Result is `(String, String)` — profile name + ISO 8601 timestamp.

**Usage note**: Returns only profiles that have at least one success. Profiles with zero launches return nothing — handle the `None` case in the enrichment layer.

#### `lookup_profile_id(name: &str) -> Result<Option<String>, MetadataStoreError>`

**Location**: `metadata/mod.rs:125`, implemented in `profile_sync.rs:72`

Looks up the stable UUID `profile_id` for a profile by current filename. Returns `None` if the profile hasn't been synced to SQLite yet. Required to query `launchers` table by profile.

**Critical**: `profile_id` is rename-stable — survives `observe_profile_rename()`. Use this UUID for all health enrichment lookups, not the profile name.

#### No existing `drift_state` query by `profile_id`

There is **no existing public MetadataStore method** that returns launcher drift state for a given profile. The `launchers` table has `profile_id` FK and `drift_state` column (schema in `migrations.rs:246`), but drift is only queried by `launcher_slug` in tests. The health command layer must either:

1. Add a new `query_launcher_drift_for_profile(profile_id: &str) -> Result<Option<String>, MetadataStoreError>` method to MetadataStore, or
2. Execute the inline SQL directly in `commands/health.rs` via `with_conn`.

Option 1 is cleaner and follows the existing pattern. The SQL is:

```sql
SELECT drift_state FROM launchers
WHERE profile_id = ?1 AND drift_state != 'missing'
ORDER BY updated_at DESC LIMIT 1
```

#### Metadata Availability Check

`MetadataStore.available` is a private field. The `with_conn` pattern silently returns `T::default()` when unavailable. For the health enrichment layer, the convention is to attempt the query and treat an empty/default result as "unavailable" — no explicit `available` flag check is needed. This matches how `profile_save` handles MetadataStore failures.

---

## Database Schema

All schema is in `metadata/migrations.rs`. Current schema version is **5**.

### `profiles` Table (migration v1, v2)

```sql
profiles (
    profile_id   TEXT PRIMARY KEY,        -- stable UUID, rename-safe
    current_filename TEXT NOT NULL UNIQUE, -- indexed: idx_profiles_current_filename
    current_path TEXT NOT NULL,
    game_name TEXT,
    launch_method TEXT,
    content_hash TEXT,
    is_favorite INTEGER DEFAULT 0,
    source TEXT,                           -- added migration v2
    deleted_at TEXT,
    created_at TEXT NOT NULL,
    updated_at TEXT NOT NULL
)
```

**Index**: `idx_profiles_current_filename ON profiles(current_filename)` — used by `lookup_profile_id`.

### `launch_operations` Table (migration v3)

```sql
launch_operations (
    operation_id TEXT PRIMARY KEY,
    profile_id   TEXT REFERENCES profiles(profile_id),
    profile_name TEXT,                     -- denormalized, NULL for anonymous launches
    launch_method TEXT NOT NULL,
    status       TEXT NOT NULL DEFAULT 'started',  -- started/succeeded/failed/abandoned
    exit_code    INTEGER,
    signal       INTEGER,
    log_path     TEXT,
    diagnostic_json TEXT,                  -- capped at 4096 bytes
    severity     TEXT,
    failure_mode TEXT,
    started_at   TEXT NOT NULL,
    finished_at  TEXT
)
```

**Indexes**: `idx_launch_ops_profile_id`, `idx_launch_ops_started_at`

**Key distinction**: `LaunchOutcome::Succeeded` maps `FailureMode::CleanExit`. Everything else maps to `LaunchOutcome::Failed`. See `launch_history.rs:70-73`.

### `launchers` Table (migration v3)

```sql
launchers (
    launcher_id         TEXT PRIMARY KEY,
    profile_id          TEXT REFERENCES profiles(profile_id),
    launcher_slug       TEXT NOT NULL UNIQUE,
    display_name        TEXT NOT NULL,
    script_path         TEXT NOT NULL,
    desktop_entry_path  TEXT NOT NULL,
    drift_state         TEXT NOT NULL DEFAULT 'unknown',  -- aligned/missing/moved/stale/unknown
    created_at          TEXT NOT NULL,
    updated_at          TEXT NOT NULL
)
```

**Indexes**: `idx_launchers_profile_id`, `idx_launchers_launcher_slug`

**DriftState enum** (`models.rs:124`): `Unknown`, `Aligned`, `Missing`, `Moved`, `Stale`.

- Exported → `Aligned`
- Deleted → `Missing` (tombstone, row NOT removed)
- Renamed → old slug set to `Missing`, new slug inserted as `Aligned`

**Gotcha**: When a profile is deleted, `observe_launcher_deleted` sets `drift_state = 'missing'` but does NOT delete the row. Health queries must filter or handle tombstoned launchers.

### Phase D: `health_snapshots` Table (migration v6, not yet implemented)

```sql
health_snapshots (
    profile_id   TEXT NOT NULL REFERENCES profiles(profile_id),
    status       TEXT NOT NULL,
    issue_count  INTEGER NOT NULL DEFAULT 0,
    checked_at   TEXT NOT NULL,
    PRIMARY KEY (profile_id)
)
```

One row per profile (UPSERT). FK cascade on profile deletion. Do NOT implement until Phase D.

---

## Frontend Integration

### Hook Pattern (`useProfile.ts`, `useLaunchState.ts`)

**`useProfile`** (`src/hooks/useProfile.ts`): Uses `useState` + `useCallback` + `useEffect` for all profile CRUD state. IPC calls go directly through `invoke()` from `@tauri-apps/api/core`. The hook exposes both loading state flags and data. This is the model for `useProfileHealth`.

**`useLaunchState`** (`src/hooks/useLaunchState.ts`): Uses `useReducer` for multi-step state machine + `listen()` from `@tauri-apps/api/event` for backend-pushed events. This is the precise pattern for the startup `profile-health-batch-complete` event listener.

### Event Listening Pattern

```typescript
import { listen } from '@tauri-apps/api/event';

useEffect(() => {
  let active = true;
  const unlisten = listen<EnrichedHealthSummary>('profile-health-batch-complete', (event) => {
    if (!active) return;
    dispatch({ type: 'batch-complete', summary: event.payload });
  });
  return () => {
    active = false;
    void unlisten.then((fn) => fn());
  };
}, []);
```

**Critical pattern**: Always capture `active` flag and call `unlisten()` on cleanup. This is the exact pattern in `useLaunchState.ts:157-186`.

### IPC Invocation Pattern

```typescript
import { invoke } from '@tauri-apps/api/core';

const summary = await invoke<EnrichedHealthSummary>('batch_validate_profiles');
const report = await invoke<EnrichedProfileHealthReport>('get_profile_health', { name });
```

Tauri serializes `Result<T, String>` as a resolved/rejected Promise. Command errors surface as thrown strings, not structured errors.

### Hook Architecture for `useProfileHealth`

The new hook should follow `useLaunchState`'s `useReducer` pattern (not `useState`) because health state has multiple concurrent updates (batch scan + startup event + single-profile revalidate). The reducer pattern handles atomic state transitions cleanly.

Expected actions: `batch-loading`, `batch-complete`, `single-loading`, `single-complete`, `reset`.

---

## `sanitize_display_path()`

**Location**: `src-tauri/src/commands/shared.rs:20`

```rust
pub fn sanitize_display_path(path: &str) -> String
```

Replaces `$HOME/...` prefix with `~/...`. Falls back to the original path if `HOME` is unset. This function is in `commands/shared`, which is a private module (`mod shared`). To use it in `commands/health.rs`, import it as:

```rust
use super::shared::sanitize_display_path;
```

This matches how `commands/launch.rs:21` imports it.

**Required usage**: Every `HealthIssue.path` field that contains an absolute filesystem path MUST pass through `sanitize_display_path()` before returning to the frontend. This prevents home directory leakage in the IPC response.

---

## Configuration

### No New Tauri Capabilities Needed

`std::fs::metadata()` does not require any Tauri plugin capabilities. The existing `tauri_plugin_fs` is not used for health checks — raw stdlib I/O is sufficient and already used by `ProfileStore`.

### MetadataStore State Access in `health.rs`

`MetadataStore` is already in Tauri state at `lib.rs:80`. Health commands receive it as `State<'_, MetadataStore>`. No changes to `lib.rs` state management are required.

### lib.rs Changes Required

Two changes are needed:

1. Add `pub mod health;` to `src-tauri/src/commands/mod.rs` (currently has: `export`, `launch`, `profile`, `settings`, `steam`, etc.)
2. Register `commands::health::batch_validate_profiles` and `commands::health::get_profile_health` in the `invoke_handler![]` macro at `lib.rs:85`
3. Add the startup background health scan spawn in the `setup` closure at `lib.rs:44`, after the existing `auto-load-profile` spawn

### Startup Event Timing

The existing startup `auto-load-profile` event is emitted after a 350ms delay (`lib.rs:62`). The health batch scan should start after UI renders — a similar or slightly longer delay (e.g., 500ms) is appropriate to avoid contention with the profile load. The feature spec states it must never block the synchronous `startup.rs` path.

---

## Relevant Files

- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src-tauri/src/commands/profile.rs` — Command pattern, State injection, MetadataStore fail-soft
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src-tauri/src/commands/launch.rs` — Async command pattern, AppHandle event emission, spawn_blocking for metadata
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src-tauri/src/commands/shared.rs` — `sanitize_display_path()` and `create_log_path()`
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src-tauri/src/commands/mod.rs` — Module registration (add `pub mod health;` here)
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src-tauri/src/lib.rs` — Tauri state management, invoke_handler registration, startup spawn pattern
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src-tauri/src/startup.rs` — Startup reconciliation pattern (sync, non-blocking)
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/metadata/mod.rs` — Full MetadataStore API, `query_failure_trends()`, `query_last_success_per_profile()`, `lookup_profile_id()`
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/metadata/migrations.rs` — SQLite schema for `profiles`, `launch_operations`, `launchers` tables
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/metadata/launch_history.rs` — `record_launch_started/finished`, outcome classification
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/metadata/launcher_sync.rs` — Launcher drift state management
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/metadata/models.rs` — `DriftState`, `LaunchOutcome`, `FailureTrendRow`, `MetadataStoreError`
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/hooks/useProfile.ts` — React hook pattern (useState + useCallback + useEffect)
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/hooks/useLaunchState.ts` — Event listening pattern (useReducer + listen())
