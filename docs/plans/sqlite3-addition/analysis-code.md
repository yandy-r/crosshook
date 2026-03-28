# SQLite Metadata Layer — Phase 2 Code Analysis

## Executive Summary

Phase 1 established MetadataStore with `Arc<Mutex<Connection>>`, `with_conn` fail-soft helper, and five profile sync hooks. Phase 2 extends this with two new tables (`launchers`, `launch_operations`), three new `MetadataStore` methods, and integration hooks in async launch commands and synchronous export commands. The critical pre-requisite is adding `profile_name: Option<String>` to `LaunchRequest` — every Phase 2 launch hook depends on it. All patterns below are extracted from the existing Phase 1 source and are the direct templates for Phase 2 code.

---

## Pattern Reference (from Phase 1 source)

### 1. `with_conn` Delegation Pattern

**Source**: `crates/crosshook-core/src/metadata/mod.rs:56–73` and `75–127`

The exact shape every Phase 2 method must replicate:

```rust
fn with_conn<F, T>(&self, action: &'static str, f: F) -> Result<T, MetadataStoreError>
where
    F: FnOnce(&Connection) -> Result<T, MetadataStoreError>,
    T: Default,
{
    if !self.available {
        return Ok(T::default());
    }

    let Some(conn) = &self.conn else {
        return Ok(T::default());
    };

    let guard = conn.lock().map_err(|_| {
        MetadataStoreError::Corrupt(format!("metadata store mutex poisoned while {action}"))
    })?;
    f(&guard)
}
```

Every public method must call `self.with_conn("gerund phrase", |conn| { submodule::function(conn, ...) })`.

**Phase 2 method shapes** to add to `mod.rs`:

```rust
pub fn observe_launcher_exported(
    &self,
    profile_name: Option<&str>,
    result: &SteamExternalLauncherExportResult,
) -> Result<(), MetadataStoreError> {
    self.with_conn("observe a launcher export", |conn| {
        launcher_sync::observe_launcher_exported(conn, profile_name, result)
    })
}

pub fn record_launch_started(
    &self,
    profile_name: Option<&str>,
    method: &str,
    log_path: Option<&str>,
) -> Result<String, MetadataStoreError> {  // returns operation_id
    self.with_conn("record a launch start", |conn| {
        launch_history::record_launch_started(conn, profile_name, method, log_path)
    })
}

pub fn record_launch_finished(
    &self,
    operation_id: &str,
    exit_code: Option<i32>,
    signal: Option<i32>,
    report: &DiagnosticReport,
) -> Result<(), MetadataStoreError> {
    self.with_conn("record a launch finish", |conn| {
        launch_history::record_launch_finished(conn, operation_id, exit_code, signal, report)
    })
}

pub fn sweep_abandoned_operations(&self) -> Result<usize, MetadataStoreError> {
    self.with_conn("sweep abandoned operations", |conn| {
        launch_history::sweep_abandoned_operations(conn)
    })
}
```

Note: `record_launch_started` returns `Result<String, MetadataStoreError>` where `String` is the `operation_id`. `String::default()` is an empty string — callers must handle the case where the id is empty (store is disabled), by skipping the `record_launch_finished` call.

### 2. Free Function Signature Pattern

**Source**: `crates/crosshook-core/src/metadata/profile_sync.rs:11–18`, `72–86`, `88–104`

All free functions in metadata submodules take `conn: &Connection` as first arg and return `Result<T, MetadataStoreError>`:

```rust
// profile_sync.rs — canonical shape
pub fn observe_profile_write(
    conn: &Connection,
    name: &str,
    profile: &GameProfile,
    path: &Path,
    source: SyncSource,
    source_profile_id: Option<&str>,
) -> Result<(), MetadataStoreError> { ... }

pub fn lookup_profile_id(
    conn: &Connection,
    name: &str,
) -> Result<Option<String>, MetadataStoreError> { ... }
```

**Phase 2 free function shapes** (in new files `launcher_sync.rs` and `launch_history.rs`):

```rust
// launcher_sync.rs
pub fn observe_launcher_exported(
    conn: &Connection,
    profile_name: Option<&str>,
    result: &SteamExternalLauncherExportResult,
) -> Result<(), MetadataStoreError> { ... }

// launch_history.rs
pub fn record_launch_started(
    conn: &Connection,
    profile_name: Option<&str>,
    method: &str,
    log_path: Option<&str>,
) -> Result<String, MetadataStoreError> { ... }

pub fn record_launch_finished(
    conn: &Connection,
    operation_id: &str,
    exit_code: Option<i32>,
    signal: Option<i32>,
    report: &DiagnosticReport,
) -> Result<(), MetadataStoreError> { ... }

pub fn sweep_abandoned_operations(conn: &Connection) -> Result<usize, MetadataStoreError> { ... }
```

### 3. Structured Error Mapping Pattern

**Source**: `crates/crosshook-core/src/metadata/profile_sync.rs:28–30`, `64–67`, `82–85`

The action string completes "failed to \_\_\_". Use a lowercase gerund phrase. Never use `format!()` for action strings — they must be `&'static str`.

```rust
// From profile_sync.rs — the exact map_err shape:
conn.execute("...", params![...])
    .map_err(|source| MetadataStoreError::Database {
        action: "upsert a profile metadata row",   // gerund, &'static str
        source,
    })?;

conn.query_row("...", params![name], |row| row.get::<_, String>(0))
    .optional()
    .map_err(|source| MetadataStoreError::Database {
        action: "look up a profile id by name",
        source,
    })

Transaction::new_unchecked(conn, TransactionBehavior::Immediate)
    .map_err(|source| MetadataStoreError::Database {
        action: "start a profile rename transaction",
        source,
    })?;

tx.commit().map_err(|source| MetadataStoreError::Database {
    action: "commit the profile rename transaction",
    source,
})?;
```

**Phase 2 action strings** (for exact error messages in new code):
- `"upsert a launcher metadata row"`
- `"insert a launch operation row"`
- `"update the finished launch operation"`
- `"sweep abandoned launch operations"`
- `"look up a profile id for launcher sync"`

### 4. Enum with `as_str()` Pattern

**Source**: `crates/crosshook-core/src/metadata/models.rs:69–93`

```rust
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SyncSource {
    AppWrite,
    AppRename,
    AppDuplicate,
    AppDelete,
    FilesystemScan,
    Import,
    InitialCensus,
}

impl SyncSource {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::AppWrite => "app_write",
            Self::AppRename => "app_rename",
            // ...
        }
    }
}
```

**Phase 2 enums** to add to `models.rs` — exact same derive set:

```rust
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LaunchOutcome {
    Started,
    Success,
    NonZeroExit,
    Signal,
    Abandoned,
}

impl LaunchOutcome {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Started => "started",
            Self::Success => "success",
            Self::NonZeroExit => "non_zero_exit",
            Self::Signal => "signal",
            Self::Abandoned => "abandoned",
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DriftState {
    Unknown,
    Clean,
    Stale,
    Orphaned,
}

impl DriftState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Unknown => "unknown",
            Self::Clean => "clean",
            Self::Stale => "stale",
            Self::Orphaned => "orphaned",
        }
    }
}
```

Note: `LaunchOutcome` maps to the `status` column in `launch_operations`. The initial INSERT uses `LaunchOutcome::Started.as_str()`. The UPDATE uses the resolved outcome after analyzing the DiagnosticReport.

### 5. UPSERT Pattern

**Source**: `crates/crosshook-core/src/metadata/profile_sync.rs:28–67`

The full UPSERT shape for idempotent sync:

```rust
conn.execute(
    "INSERT INTO profiles (
        profile_id,
        current_filename,
        ...
        created_at,
        updated_at
    ) VALUES (?1, ?2, ..., ?9, ?10)
    ON CONFLICT(current_filename) DO UPDATE SET
        current_path = excluded.current_path,
        game_name = excluded.game_name,
        ...,
        updated_at = excluded.updated_at",
    params![
        db::new_id(),   // ← UUID generated at call site
        name,
        ...,
        created_at,
        now,
    ],
)
.map_err(|source| MetadataStoreError::Database {
    action: "upsert a profile metadata row",
    source,
})?;
```

**Phase 2 UPSERT** for `launchers` table — conflict on `launcher_slug`:

```rust
conn.execute(
    "INSERT INTO launchers (
        launcher_id, profile_id, launcher_slug, display_name,
        script_path, desktop_entry_path, drift_state, created_at, updated_at
    ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
    ON CONFLICT(launcher_slug) DO UPDATE SET
        profile_id = COALESCE(excluded.profile_id, launchers.profile_id),
        display_name = excluded.display_name,
        script_path = excluded.script_path,
        desktop_entry_path = excluded.desktop_entry_path,
        drift_state = excluded.drift_state,
        updated_at = excluded.updated_at",
    params![
        db::new_id(),
        profile_id,          // Option<String> from lookup_profile_id()
        result.launcher_slug,
        result.display_name,
        result.script_path,
        result.desktop_entry_path,
        DriftState::Clean.as_str(),
        now,
        now,
    ],
)
.map_err(|source| MetadataStoreError::Database {
    action: "upsert a launcher metadata row",
    source,
})?;
```

`launch_operations` rows are INSERT-only (no UPSERT) — each operation gets a fresh UUID PK.

### 6. Transaction Pattern

**Source**: `crates/crosshook-core/src/metadata/profile_sync.rs:98–167`

Used for multi-step operations that must be atomic:

```rust
let tx = Transaction::new_unchecked(conn, TransactionBehavior::Immediate)
    .map_err(|source| MetadataStoreError::Database {
        action: "start a profile rename transaction",
        source,
    })?;

// ... multiple tx.execute() calls ...

tx.commit().map_err(|source| MetadataStoreError::Database {
    action: "commit the profile rename transaction",
    source,
})?;
```

`record_launch_finished` should use a transaction if it needs to both UPDATE the operation row and do a secondary write (e.g., updating launcher drift state). If it is a single UPDATE, no transaction is needed.

### 7. `spawn_blocking` Pattern for Async Commands

**Source**: `src-tauri/src/commands/install.rs:10–18`

```rust
#[tauri::command]
pub async fn install_default_prefix_path(profile_name: String) -> Result<String, String> {
    tauri::async_runtime::spawn_blocking(move || {
        install_default_prefix_path_core(&profile_name)
            .map(|path| path.to_string_lossy().into_owned())
            .map_err(|error| error.to_string())
    })
    .await
    .map_err(|error| error.to_string())?
}
```

`rusqlite::Connection` is `!Send`, so it cannot be held across `.await` points. In the async `launch_game` and `launch_trainer` commands, all metadata writes must be in `spawn_blocking`. The `MetadataStore` itself is `Clone` (cheap `Arc` clone), so clone it into the closure:

```rust
// In launch_game or launch_trainer (before spawn_log_stream):
let metadata_store = app.state::<MetadataStore>().inner().clone();
let op_id = tauri::async_runtime::spawn_blocking(move || {
    metadata_store.record_launch_started(
        request.profile_name.as_deref(),
        resolved_method,
        Some(&log_path.to_string_lossy()),
    )
})
.await
.map_err(|e| format!("spawn_blocking join failed: {e}"))??;  // double ? — join error then Result
```

Note the double `?`: one for `JoinError` from `spawn_blocking`, one for the `Result<T, E>` inside.

### 8. Warn-and-Continue Pattern

**Source**: `src-tauri/src/commands/profile.rs:106–113` and `128–142`

```rust
// From profile_save:
store.save(&name, &data).map_err(map_error)?;   // critical op — propagates error

let profile_path = store.base_path.join(format!("{name}.toml"));
if let Err(e) =
    metadata_store.observe_profile_write(&name, &data, &profile_path, SyncSource::AppWrite, None)
{
    tracing::warn!(%e, profile_name = %name, "metadata sync after profile_save failed");
}

Ok(())  // metadata failure does not block primary success
```

**Phase 2 export hook** — add `State<'_, MetadataStore>` to `export_launchers` and call after success:

```rust
#[tauri::command]
pub fn export_launchers(
    request: SteamExternalLauncherExportRequest,
    metadata_store: State<'_, MetadataStore>,
) -> Result<SteamExternalLauncherExportResult, String> {
    let result = export_launchers_core(&request).map_err(|error| error.to_string())?;

    if let Err(e) = metadata_store.observe_launcher_exported(
        request.profile_name.as_deref(),
        &result,
    ) {
        tracing::warn!(%e, launcher_slug = %result.launcher_slug, "metadata sync after export_launchers failed");
    }

    Ok(result)
}
```

### 9. Migration Runner Pattern

**Source**: `crates/crosshook-core/src/metadata/migrations.rs:4–84`

```rust
pub fn run_migrations(conn: &Connection) -> Result<(), MetadataStoreError> {
    let version = conn
        .pragma_query_value(None, "user_version", |row| row.get::<_, u32>(0))
        .map_err(|source| MetadataStoreError::Database {
            action: "read metadata schema version",
            source,
        })?;

    if version < 1 {
        migrate_0_to_1(conn)?;
        conn.pragma_update(None, "user_version", 1_u32)
            .map_err(|source| MetadataStoreError::Database {
                action: "update metadata schema version",
                source,
            })?;
    }

    if version < 2 {
        migrate_1_to_2(conn)?;
        conn.pragma_update(None, "user_version", 2_u32) ...
    }

    Ok(())
}

fn migrate_0_to_1(conn: &Connection) -> Result<(), MetadataStoreError> {
    conn.execute_batch("CREATE TABLE IF NOT EXISTS profiles (...); CREATE INDEX ...; ...")
        .map_err(|source| MetadataStoreError::Database {
            action: "run metadata migration 0 to 1",
            source,
        })?;
    Ok(())
}
```

**Phase 2 migration** adds `migrate_2_to_3` to the existing chain:

```rust
// Append to run_migrations after the `version < 2` block:
if version < 3 {
    migrate_2_to_3(conn)?;
    conn.pragma_update(None, "user_version", 3_u32)
        .map_err(|source| MetadataStoreError::Database {
            action: "update metadata schema version",
            source,
        })?;
}

fn migrate_2_to_3(conn: &Connection) -> Result<(), MetadataStoreError> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS launchers (
            launcher_id TEXT PRIMARY KEY,
            profile_id TEXT REFERENCES profiles(profile_id),
            launcher_slug TEXT NOT NULL UNIQUE,
            display_name TEXT NOT NULL,
            script_path TEXT NOT NULL,
            desktop_entry_path TEXT NOT NULL,
            drift_state TEXT NOT NULL DEFAULT 'unknown',
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        );
        CREATE INDEX IF NOT EXISTS idx_launchers_profile_id ON launchers(profile_id);
        CREATE INDEX IF NOT EXISTS idx_launchers_launcher_slug ON launchers(launcher_slug);

        CREATE TABLE IF NOT EXISTS launch_operations (
            operation_id TEXT PRIMARY KEY,
            profile_id TEXT REFERENCES profiles(profile_id),
            profile_name TEXT,
            launch_method TEXT NOT NULL,
            status TEXT NOT NULL DEFAULT 'started',
            exit_code INTEGER,
            signal INTEGER,
            log_path TEXT,
            diagnostic_json TEXT,
            severity TEXT,
            failure_mode TEXT,
            started_at TEXT NOT NULL,
            finished_at TEXT
        );
        CREATE INDEX IF NOT EXISTS idx_launch_ops_profile_id ON launch_operations(profile_id);
        CREATE INDEX IF NOT EXISTS idx_launch_ops_started_at ON launch_operations(started_at);
        ",
    )
    .map_err(|source| MetadataStoreError::Database {
        action: "run metadata migration 2 to 3",
        source,
    })?;
    Ok(())
}
```

### 10. Test Patterns

**Source**: `crates/crosshook-core/src/metadata/mod.rs:129–389`

The `open_in_memory` constructor is the test entry point. The private `connection` helper gives raw SQL access for assertions:

```rust
// In tests module (mod.rs):
fn connection(store: &MetadataStore) -> std::sync::MutexGuard<'_, Connection> {
    store
        .conn
        .as_ref()
        .expect("metadata store should expose a connection in tests")
        .lock()
        .expect("metadata store mutex should not be poisoned")
}

#[test]
fn test_observe_launcher_exported_creates_row() {
    let store = MetadataStore::open_in_memory().unwrap();
    let result = SteamExternalLauncherExportResult {
        display_name: "Elden Ring - Trainer".to_string(),
        launcher_slug: "elden-ring".to_string(),
        script_path: "/home/user/.local/share/crosshook/launchers/elden-ring-trainer.sh".to_string(),
        desktop_entry_path: "/home/user/.local/share/applications/crosshook-elden-ring-trainer.desktop".to_string(),
    };

    store.observe_launcher_exported(None, &result).unwrap();

    let conn = connection(&store);
    let row_count: i64 = conn
        .query_row("SELECT COUNT(*) FROM launchers WHERE launcher_slug = ?1", params!["elden-ring"], |row| row.get(0))
        .unwrap();
    assert_eq!(row_count, 1);
}
```

Test for disabled store — all Phase 2 methods must no-op:

```rust
#[test]
fn test_disabled_store_phase2_noop() {
    let store = MetadataStore::disabled();
    // SteamExternalLauncherExportResult::default() is all empty strings
    assert!(store.observe_launcher_exported(None, &SteamExternalLauncherExportResult::default()).is_ok());
    assert!(store.record_launch_started(None, "native", None).is_ok());
    // returned op_id is empty string when disabled — callers must handle this
    let op_id = store.record_launch_started(None, "native", None).unwrap();
    assert!(op_id.is_empty());
}
```

---

## Integration Points

### Files to Create

| File                                                                                      | Purpose                                                        |
| ----------------------------------------------------------------------------------------- | -------------------------------------------------------------- |
| `crates/crosshook-core/src/metadata/launcher_sync.rs`                                    | Free functions for `launchers` table UPSERT and drift updates  |
| `crates/crosshook-core/src/metadata/launch_history.rs`                                   | Free functions for `launch_operations` INSERT/UPDATE/sweep     |

### Files to Modify — Exact Insertion Points

#### `crates/crosshook-core/src/metadata/mod.rs`

- Add `mod launcher_sync;` and `mod launch_history;` at the top (after `pub mod profile_sync;`)
- Add imports: `use crate::export::launcher::SteamExternalLauncherExportResult;` and `use crate::launch::diagnostics::models::DiagnosticReport;`
- Add four new public methods after `sync_profiles_from_store` (lines 119–127): `observe_launcher_exported`, `record_launch_started`, `record_launch_finished`, `sweep_abandoned_operations`

#### `crates/crosshook-core/src/metadata/models.rs`

- Add `LaunchOutcome` enum after `SyncSource` (after line 93)
- Add `DriftState` enum after `LaunchOutcome`
- Add `LauncherRow` and `LaunchOperationRow` structs (mark `#[allow(dead_code)]` matching `ProfileRow` at line 105)

#### `crates/crosshook-core/src/metadata/migrations.rs`

- Add `if version < 3 { migrate_2_to_3(conn)?; pragma_update(3)?; }` after the `version < 2` block (after line 28)
- Add private `migrate_2_to_3` function with full DDL for both tables

#### `crates/crosshook-core/src/launch/request.rs`

**Phase 2 blocker**: Add `profile_name: Option<String>` field to `LaunchRequest` struct (after line 36):

```rust
// LaunchRequest struct — add before closing brace:
#[serde(default)]
pub profile_name: Option<String>,
```

Any struct literal in tests that omit `profile_name` will need `profile_name: None` added. The `#[serde(default)]` annotation ensures existing frontend callers that omit this field continue to work.

#### `crates/crosshook-core/src/export/launcher.rs`

**Phase 2 blocker**: Add `profile_name: Option<String>` to `SteamExternalLauncherExportRequest` (after line 25):

```rust
// SteamExternalLauncherExportRequest struct — add:
#[serde(default)]
pub profile_name: Option<String>,
```

The `export_launchers` command in `commands/export.rs` constructs `SteamExternalLauncherExportRequest` from flat params in `rename_launcher` (lines 95–108) — that builder will need `profile_name: None` added.

#### `src-tauri/src/commands/launch.rs`

Phase 2 hooks in `launch_game` (lines 48–83) and `launch_trainer` (lines 86–123):

1. **Before `spawn_log_stream`**: call `record_launch_started` via `spawn_blocking`, capture `op_id`
2. **Pass `op_id` and `MetadataStore` into `spawn_log_stream`**: the function signature must gain `metadata_store: MetadataStore` and `operation_id: String` params
3. **In `stream_log_lines` after `analyze()`** (line 211): call `record_launch_finished` via `spawn_blocking` with the `DiagnosticReport`

Current `spawn_log_stream` signature (line 125):
```rust
fn spawn_log_stream(app: AppHandle, log_path: PathBuf, child: tokio::process::Child, method: &'static str)
```

Phase 2 signature:
```rust
fn spawn_log_stream(
    app: AppHandle,
    log_path: PathBuf,
    child: tokio::process::Child,
    method: &'static str,
    metadata_store: MetadataStore,  // Arc clone, cheap
    operation_id: String,
)
```

Both `launch_game` and `launch_trainer` must add `metadata_store: State<'_, MetadataStore>` parameter and clone it before the `spawn_log_stream` call.

#### `src-tauri/src/commands/export.rs`

- `export_launchers` (line 20): add `metadata_store: State<'_, MetadataStore>` parameter, add warn-and-continue hook after `export_launchers_core` succeeds
- Add `use crosshook_core::metadata::MetadataStore;` import at top

#### `src-tauri/src/startup.rs`

- `run_metadata_reconciliation` (line 43): add `sweep_abandoned_operations` call after `sync_profiles_from_store`:

```rust
pub fn run_metadata_reconciliation(
    metadata_store: &MetadataStore,
    profile_store: &ProfileStore,
) -> Result<(), StartupError> {
    let report = metadata_store.sync_profiles_from_store(profile_store)?;
    if report.created > 0 || report.updated > 0 {
        tracing::info!(created = report.created, updated = report.updated, "startup metadata reconciliation complete");
    }
    // Phase 2 addition:
    if let Err(error) = metadata_store.sweep_abandoned_operations() {
        tracing::warn!(%error, "startup sweep of abandoned launch operations failed");
    }
    Ok(())
}
```

---

## Data Flow for Phase 2

### Launcher Export Flow

```
export_launchers (commands/export.rs)
  └─ export_launchers_core() → SteamExternalLauncherExportResult
  └─ metadata_store.observe_launcher_exported(request.profile_name, &result)
       └─ with_conn("observe a launcher export", |conn| launcher_sync::observe_launcher_exported(conn, ...))
            └─ lookup_profile_id(conn, name) → Option<String>    [resolves profile_name → profile_id]
            └─ INSERT INTO launchers ... ON CONFLICT(launcher_slug) DO UPDATE ...
```

### Launch Operation Flow

```
launch_game / launch_trainer (commands/launch.rs)
  └─ spawn_blocking → metadata_store.record_launch_started(profile_name, method, log_path)
       └─ INSERT INTO launch_operations (operation_id=new_id(), status='started', ...)
  └─ spawn_log_stream(app, log_path, child, method, metadata_store, op_id)
       └─ [poll loop — existing behavior]
       └─ analyze(exit_status, &log_tail, method) → DiagnosticReport  [line 211]
       └─ spawn_blocking → metadata_store.record_launch_finished(op_id, exit_code, signal, &report)
            └─ serialize DiagnosticReport → JSON, truncate to 4096 bytes if > 4KB
            └─ UPDATE launch_operations SET status=outcome, exit_code=..., finished_at=now WHERE operation_id=?
```

### Startup Sweep Flow

```
run_metadata_reconciliation (startup.rs)
  └─ sync_profiles_from_store()                [existing — Phase 1]
  └─ sweep_abandoned_operations()              [Phase 2 addition]
       └─ UPDATE launch_operations SET status='abandoned', finished_at=now
          WHERE status='started' AND finished_at IS NULL
```

---

## Gotchas and Edge Cases

1. **`record_launch_started` returns empty string when store is disabled** — Callers must guard before calling `record_launch_finished`:
   ```rust
   if !operation_id.is_empty() {
       // call record_launch_finished
   }
   ```

2. **`DiagnosticReport` serialization size** — `serde_json::to_string(&report)` can exceed 4KB. **Nullify** (do NOT truncate to partial JSON — malformed JSON is worse than NULL):
   ```rust
   let json = serde_json::to_string(&report).ok();
   let json = json.filter(|s| s.len() <= MAX_DIAGNOSTIC_JSON_BYTES);
   ```
   The promoted columns (`severity`, `failure_mode`, `exit_code`) must still be populated even when `diagnostic_json` is `None`.

3. **`profile_name` in `LaunchRequest` is `Option<String>`** — `lookup_profile_id` returns `Option<String>`. When `profile_name` is `None`, store `NULL` in both `profile_id` and `profile_name` columns. Never store sentinel strings like `""` or `"unknown"`.

4. **Struct literal breakage from `profile_name` addition** — Adding `profile_name` to `LaunchRequest` and `SteamExternalLauncherExportRequest` will break all struct literals that use `..Default::default()` or exhaustive field lists. Grep for struct literals in:
   - `crates/crosshook-core/src/launch/script_runner.rs` (test fixtures)
   - `src-tauri/src/commands/export.rs:95–108` (`rename_launcher` builder)
   - Any test files that construct `LaunchRequest { ... }` directly

5. **`rename_launcher` command has inline struct construction** (`export.rs:95–108`) — this builder does not accept `profile_name` from the frontend today. Add `profile_name: None` to make it compile; a separate task can wire the profile name through later.

6. **`spawn_blocking` double-unwrap** — The `.await` on `spawn_blocking` returns `Result<Result<T, E>, JoinError>`. The double `?` is intentional:
   ```rust
   .await
   .map_err(|e| e.to_string())?   // JoinError → String
   ?                               // inner Result<T, E>
   ```

7. **`log_path` sanitization before storage** — Use `sanitize_display_path` from `commands/shared.rs` on the `log_path` before passing to `record_launch_started`. This is the same function called at line 212 of `launch.rs`.

8. **`launchers.launcher_slug` must be UNIQUE** — The UPSERT conflicts on `launcher_slug`, not `launcher_id`. If a slug changes (launcher renamed), it will create a new row rather than updating the old one. The old row becomes orphaned. Launcher rename cleanup is a separate concern (outside Phase 2 scope per shared.md).

9. **`from_str` for `FailureMode` needed for SQL read-back** — `DiagnosticReport.exit_info.failure_mode` is a `FailureMode` enum stored as snake_case text. When reading back from `launch_operations`, you need `FailureMode` to implement `FromStr` or use `serde_json::from_str`. Since Phase 2 only writes (never reads back `DiagnosticReport`), this is deferred.

10. **`ValidationSeverity` for `severity` column** — `DiagnosticReport.severity` is `ValidationSeverity` from `launch/request.rs`. Its `as_str()` or serde representation must be used for the `severity` column. Check `ValidationSeverity` derives `Serialize` — if so, use `serde_json::to_value(&report.severity)` and extract the string.
