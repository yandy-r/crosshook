# Pattern Research: SQLite Metadata Layer Phase 2 (Operational History)

## Overview

Phase 1 (`metadata/`) is fully implemented and live. The patterns below are extracted from
the real Phase 1 source, not speculation. Phase 2 must follow these exact shapes or it will
look inconsistent inside the same module.

---

## Relevant Files

- `crates/crosshook-core/src/metadata/mod.rs` — `MetadataStore` struct, `with_conn` helper, public API delegates, full test suite
- `crates/crosshook-core/src/metadata/db.rs` — `open_at_path`, `open_in_memory`, `new_id`, `configure_connection`
- `crates/crosshook-core/src/metadata/migrations.rs` — `run_migrations`, `migrate_0_to_1`, `migrate_1_to_2`
- `crates/crosshook-core/src/metadata/models.rs` — `MetadataStoreError`, `SyncSource`, `SyncReport`, `ProfileRow`
- `crates/crosshook-core/src/metadata/profile_sync.rs` — `observe_profile_write`, `lookup_profile_id`, `observe_profile_rename`, `observe_profile_delete`, `sync_profiles_from_store`
- `crates/crosshook-core/src/launch/request.rs` — `LaunchRequest`, `SteamLaunchConfig`, `RuntimeLaunchConfig`, method constants
- `crates/crosshook-core/src/launch/diagnostics/models.rs` — `DiagnosticReport`, `ExitCodeInfo`, `FailureMode`, `PatternMatch`
- `crates/crosshook-core/src/launch/diagnostics/mod.rs` — `analyze()`, `should_surface_report()`
- `crates/crosshook-core/src/export/launcher_store.rs` — `LauncherInfo`, `LauncherDeleteResult`, `LauncherRenameResult`, `sanitize_launcher_slug`, `derive_launcher_paths`
- `crates/crosshook-core/src/export/launcher.rs` — `sanitize_launcher_slug` implementation (line 265)
- `src-tauri/src/commands/launch.rs` — `launch_game`, `launch_trainer` async commands, `spawn_log_stream`
- `src-tauri/src/commands/profile.rs` — warn-and-continue pattern with `MetadataStore`
- `src-tauri/src/commands/install.rs` — `spawn_blocking` canonical usage
- `src-tauri/src/lib.rs` — `MetadataStore::try_new()` with fail-soft fallback to `MetadataStore::disabled()`
- `src-tauri/src/startup.rs` — `run_metadata_reconciliation` calling `sync_profiles_from_store`

---

## Phase 1 Metadata Patterns

### `with_conn` Helper Pattern

`mod.rs:56–73` — the central routing method for all MetadataStore operations.

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

Key rules:

- `T: Default` is required — disabled store returns `T::default()` silently (no error).
- `action` is always a quoted English phrase: `"observe a profile write"`, `"look up a profile id"`.
- The closure receives `&Connection`, not `&mut Connection` — SQLite WAL mode allows concurrent reads; writes use `Transaction` inside the closure when needed.
- `conn` field is `Option<Arc<Mutex<Connection>>>` and `available: bool` are two separate guards.

Every public method on `MetadataStore` delegates through `with_conn`:

```rust
// mod.rs:75–93 — exact delegation shape Phase 2 must replicate
pub fn observe_profile_write(
    &self,
    name: &str,
    profile: &GameProfile,
    path: &Path,
    source: SyncSource,
    source_profile_id: Option<&str>,
) -> Result<(), MetadataStoreError> {
    self.with_conn("observe a profile write", |conn| {
        profile_sync::observe_profile_write(conn, name, profile, path, source, source_profile_id)
    })
}
```

For `()` return type, `()` trivially satisfies `T: Default`, so disabled store returns `Ok(())`.

### `profile_sync` Function Pattern

All functions in `profile_sync.rs` take `conn: &Connection` as first arg and return `Result<T, MetadataStoreError>`.
They are free functions, not methods. The `mod.rs` method wraps them via `with_conn`.

Signature shape:

```rust
pub fn observe_profile_write(
    conn: &Connection,
    name: &str,
    profile: &GameProfile,
    path: &Path,
    source: SyncSource,
    source_profile_id: Option<&str>,
) -> Result<(), MetadataStoreError>
```

SQL operations always map errors with the structured variant:

```rust
conn.execute("...", params![...])
    .map_err(|source| MetadataStoreError::Database {
        action: "upsert a profile metadata row",
        source,
    })?;
```

`action` strings are always lowercase English gerund phrases that finish the sentence
"failed to \_\_\_": `"upsert a profile metadata row"`, `"look up a profile id by name"`,
`"soft-delete a profile metadata row"`.

For multi-step operations that must be atomic, use an explicit transaction:

```rust
// profile_sync.rs:98–103
let tx = Transaction::new_unchecked(conn, TransactionBehavior::Immediate)
    .map_err(|source| MetadataStoreError::Database {
        action: "start a profile rename transaction",
        source,
    })?;
// ... tx.execute(), tx.execute() ...
tx.commit().map_err(|source| MetadataStoreError::Database {
    action: "commit the profile rename transaction",
    source,
})?;
```

`query_row` returns use `.optional()` for nullable results:

```rust
conn.query_row("SELECT profile_id FROM profiles WHERE ...", params![name], |row| row.get::<_, String>(0))
    .optional()
    .map_err(|source| MetadataStoreError::Database {
        action: "look up a profile id by name",
        source,
    })
```

### Models Pattern (derives, field types)

`models.rs` — all structs and enums in the metadata layer.

**Error enum** (`models.rs:8–21`):

```rust
#[derive(Debug)]
pub enum MetadataStoreError {
    HomeDirectoryUnavailable,
    Database { action: &'static str, source: SqlError },
    Io { action: &'static str, path: PathBuf, source: std::io::Error },
    Corrupt(String),
    SymlinkDetected(PathBuf),
}
```

- `action: &'static str` is a string literal, never allocated.
- `From<SqlError>` is implemented for ergonomic `?` propagation with a generic fallback action.

**Enum with string mapping** (`models.rs:69–93`):

```rust
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SyncSource {
    AppWrite,
    // ...
}

impl SyncSource {
    pub fn as_str(self) -> &'static str {
        match self { Self::AppWrite => "app_write", ... }
    }
}
```

Phase 2 enums (`LaunchOutcome`, `DriftState`) must follow this exact pattern: derive
`Debug + Clone + Copy + Serialize + Deserialize` with `#[serde(rename_all = "snake_case")]`
and an `as_str() -> &'static str` method for SQL storage.

**Row struct** (internal, not IPC-bound, `models.rs:104–117`):

```rust
#[derive(Debug, Clone)]
#[allow(dead_code)]
pub(crate) struct ProfileRow {
    pub profile_id: String,
    pub current_filename: String,
    // ...
    pub created_at: String,
    pub updated_at: String,
}
```

- `pub(crate)` visibility — never exposed at the crate boundary.
- `#[allow(dead_code)]` — row structs may not be fully consumed yet; suppress warnings.
- Timestamps are stored as `String` (RFC 3339), not `chrono::DateTime`.
- No `#[derive(Default)]` — row structs represent live DB rows, not constructed values.

**Report struct** (IPC-bound, `models.rs:95–102`):

```rust
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SyncReport {
    pub profiles_seen: usize,
    pub created: usize,
    pub updated: usize,
    pub deleted: usize,
    pub errors: Vec<String>,
}
```

IPC-bound structs derive `Default` and use primitive field types (`usize`, `String`, `Vec<String>`).

### Migration Pattern (DDL, user_version bump)

`migrations.rs` — hand-rolled sequential migration runner, no migration framework.

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
        conn.pragma_update(None, "user_version", 2_u32)
            .map_err(...)?;
    }

    Ok(())
}
```

Rules:

- Each migration is `if version < N` not `if version == N-1` — allows re-running from any version.
- `user_version` is bumped **after** the migration function succeeds, inside the same `if` block.
- Each migration is a separate private function named `migrate_{old}_to_{new}`.
- DDL goes inside `conn.execute_batch("...")` — one string per migration step.
- Phase 2 adds `migrate_2_to_3` creating `launchers` and `launch_operations` tables.

### `mod.rs` Delegation to Submodules

`mod.rs` pattern: declare submodules at top, `pub use` only types that belong to the public
API, private struct fields, and public methods that delegate via `with_conn`:

```rust
// mod.rs:1–6
mod db;
mod migrations;
mod models;
pub mod profile_sync;  // pub only because sync is called from startup.rs directly

pub use models::{MetadataStoreError, SyncReport, SyncSource};
```

Phase 2 adds `mod launcher_sync;` and `mod launch_history;` — both private (no `pub mod`).
New public-API types like `LauncherRow`, `LaunchOperationRow`, `LaunchOutcome`, `DriftState`
are added to `models.rs` and re-exported via `pub use models::...` in `mod.rs`.

---

## Launch System Patterns

### `LaunchRequest` Construction

`launch/request.rs:16–37` — fully serde-derived with `#[serde(default)]` on all fields.

```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct LaunchRequest {
    #[serde(default)]
    pub method: String,
    #[serde(default)]
    pub game_path: String,
    // ...
    #[serde(default)]
    pub launch_trainer_only: bool,
    #[serde(default)]
    pub launch_game_only: bool,
}
```

The request flows from frontend JSON → Tauri IPC deserialization → command handler.
It is never constructed from core library code; only from commands or tests.

Method constants (`request.rs:11–13`):

```rust
pub const METHOD_STEAM_APPLAUNCH: &str = "steam_applaunch";
pub const METHOD_PROTON_RUN: &str = "proton_run";
pub const METHOD_NATIVE: &str = "native";
```

`request.resolved_method()` handles fallback resolution from `app_id` and game path heuristics.
Phase 2 stores the resolved method string (not the raw `request.method`) via `request.resolved_method()`.

### Async Command Pattern

`commands/launch.rs:48–83` — the canonical async Tauri command shape.

```rust
#[tauri::command]
pub async fn launch_game(app: AppHandle, request: LaunchRequest) -> Result<LaunchResult, String> {
    // 1. Mutate local copy of request
    let mut request = request;
    request.launch_game_only = true;

    // 2. Validate synchronously before any async work
    validate(&request).map_err(|error| error.to_string())?;

    // 3. Build command (sync, cheap)
    let log_path = create_log_path("game", &request.log_target_slug())?;
    let mut command = match method { ... };

    // 4. Spawn child process
    let child = command.spawn().map_err(|error| format!("failed to launch helper: {error}"))?;

    // 5. Spawn background task for log streaming + outcome reporting
    spawn_log_stream(app, log_path.clone(), child, method);

    // 6. Return immediately — outcome reported via events
    Ok(LaunchResult { succeeded: true, message: "...", helper_log_path: ... })
}
```

The command returns immediately after spawning. The `spawn_log_stream` inner function spawns
a `tauri::async_runtime::spawn` task that polls the child and streams log lines via `app.emit`.

`AppHandle` is the first parameter for all async commands that emit events. `State<'_>` params
come after `AppHandle`.

### `spawn_blocking` Pattern

`commands/install.rs:10–18` — the canonical pattern for blocking core-library calls in async commands.

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

The double `.map_err` handles two error levels: the core function error and the `JoinError`
from `spawn_blocking` panicking. Phase 2 async commands that call `MetadataStore` methods
(which hold a `Mutex<Connection>`) must use this pattern to avoid blocking the async executor.

### Launch Outcome Reporting

`commands/launch.rs:203–229` — outcomes are emitted as Tauri events, not returned from the command.

```rust
// After process exits:
let report = analyze(exit_status, &log_tail, method);  // -> DiagnosticReport
report.log_tail_path = Some(sanitize_display_path(&log_path.to_string_lossy()));

if should_surface_report(&report) {
    app.emit("launch-diagnostic", &report)?;
}

app.emit("launch-complete", serde_json::json!({
    "code": exit_code,
    "signal": signal,
}))?;
```

Events: `"launch-log"` (per line, String), `"launch-diagnostic"` (DiagnosticReport JSON),
`"launch-complete"` (JSON with `code` and `signal` fields).

---

## Export/Launcher Patterns

### `LauncherStore` API

`export/launcher_store.rs` — free functions, not a struct. No `LauncherStore` type exists;
the "store" is the filesystem. All public functions take `&str` params and return `Result<T, LauncherStoreError>`.

Key public functions:

- `check_launcher_exists(display_name, steam_app_id, trainer_path, target_home_path, steam_client_install_path) -> Result<LauncherInfo, LauncherStoreError>`
- `check_launcher_for_profile(profile, target_home_path, steam_client_install_path) -> Result<LauncherInfo, LauncherStoreError>`
- `delete_launcher_files(display_name, steam_app_id, trainer_path, target_home_path, steam_client_install_path) -> Result<LauncherDeleteResult, LauncherStoreError>`
- `delete_launcher_by_slug(launcher_slug, target_home_path, steam_client_install_path) -> Result<LauncherDeleteResult, LauncherStoreError>`
- `delete_launcher_for_profile(profile, target_home_path, steam_client_install_path) -> Result<LauncherDeleteResult, LauncherStoreError>`

### Slug Generation and Path Derivation

`export/launcher.rs:265–289` — `sanitize_launcher_slug` converts a display name to a
lowercase hyphen-separated ASCII slug, falling back to `"crosshook-trainer"` for empty input.

`export/launcher_store.rs:115–138` — `derive_launcher_paths` is the shared inner function
that all public check/delete functions call to get consistent paths:

```rust
fn derive_launcher_paths(
    display_name: &str,
    steam_app_id: &str,
    trainer_path: &str,
    target_home_path: &str,
    steam_client_install_path: &str,
) -> (String, String, String, String) {  // (resolved_name, slug, script_path, desktop_entry_path)
```

Files land at:

- Script: `~/.local/share/crosshook/launchers/{slug}-trainer.sh`
- Desktop entry: `~/.local/share/applications/crosshook-{slug}-trainer.desktop`

### Launcher Lifecycle Cascade

`commands/profile.rs:147–265` — profile delete and rename trigger best-effort launcher cleanup.
Profile delete calls `cleanup_launchers_for_profile_delete` which calls `delete_launcher_for_profile`.
Profile rename deletes old launcher files so the frontend can re-export with the new name.

**For `observe_launcher_exported` in Phase 2**: the slug and paths needed for the DB row are
already computed by `derive_launcher_paths` at export time. The slug is the stable key for
drift detection.

IPC result structs (all follow the same derive pattern):

```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct LauncherInfo {
    #[serde(default)]
    pub display_name: String,
    #[serde(default)]
    pub launcher_slug: String,
    #[serde(default)]
    pub script_path: String,
    #[serde(default)]
    pub desktop_entry_path: String,
    #[serde(default)]
    pub script_exists: bool,
    #[serde(default)]
    pub desktop_entry_exists: bool,
    #[serde(default)]
    pub is_stale: bool,
}
```

---

## Diagnostic Report Pattern

### `DiagnosticReport` Structure

`launch/diagnostics/models.rs:10–18`:

```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DiagnosticReport {
    pub severity: ValidationSeverity,
    pub summary: String,
    pub exit_info: ExitCodeInfo,
    pub pattern_matches: Vec<PatternMatch>,
    pub suggestions: Vec<ActionableSuggestion>,
    pub launch_method: String,
    pub log_tail_path: Option<String>,
    pub analyzed_at: String,  // RFC 3339
}
```

For Phase 2 `launch_operations` table: store as `diagnostic_json TEXT` column using
`serde_json::to_string(&report)`. The full `DiagnosticReport` serializes to JSON cleanly
because all its types derive `Serialize`. Retrieve with `serde_json::from_str::<DiagnosticReport>`.

### `FailureMode` Enum

`models.rs:34–50` — 14 variants covering all exit code categories:

```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum FailureMode {
    CleanExit, NonZeroExit, Segfault, Abort, Kill, BusError,
    IllegalInstruction, FloatingPointException, BrokenPipe,
    Terminated, CommandNotFound, PermissionDenied, UnknownSignal,
    Indeterminate, Unknown,
}
```

For `LaunchOutcome` enum in Phase 2, model after `FailureMode`:
`#[serde(rename_all = "snake_case")]` + `as_str()` method for SQL column storage.

### Serialization Pattern

`DiagnosticReport` is already `Serialize + Deserialize`. Store the complete JSON blob in SQLite:

```rust
// In launch_history.rs (Phase 2):
let diagnostic_json = serde_json::to_string(&report)
    .map_err(|e| MetadataStoreError::Corrupt(format!("failed to serialize DiagnosticReport: {e}")))?;
conn.execute(
    "UPDATE launch_operations SET diagnostic_json = ?1 WHERE operation_id = ?2",
    params![diagnostic_json, operation_id],
)?;
```

---

## Error Handling Patterns

### `MetadataStoreError` Usage

`models.rs:8–57` — five variants:

1. `HomeDirectoryUnavailable` — standalone, no fields
2. `Database { action: &'static str, source: SqlError }` — all SQLite errors
3. `Io { action: &'static str, path: PathBuf, source: std::io::Error }` — all filesystem errors
4. `Corrupt(String)` — invariant violations (missing rows, mutex poisoned, unexpected states)
5. `SymlinkDetected(PathBuf)` — security-specific

`From<SqlError>` impl provides ergonomic `?` with generic action `"run a database operation"`.
For new SQL operations in Phase 2, prefer the explicit structured form over the `From` impl
so action strings remain specific.

### Warn-and-Continue in Tauri Commands

`commands/profile.rs` — the dominant pattern for all metadata sync hooks:

```rust
// profile_save (profile.rs:106–113)
if let Err(e) = metadata_store.observe_profile_write(&name, &data, &profile_path, SyncSource::AppWrite, None) {
    tracing::warn!(%e, profile_name = %name, "metadata sync after profile_save failed");
}

// profile_delete (profile.rs:162–165)
if let Err(e) = metadata_store.observe_profile_delete(&name) {
    tracing::warn!(%e, profile_name = %name, "metadata sync after profile_delete failed");
}

// profile_rename (profile.rs:226–229)
if let Err(e) = metadata_store.observe_profile_rename(&old_name, &new_name, &old_path, &new_path) {
    tracing::warn!(%e, %old_name, %new_name, "metadata sync after profile_rename failed");
}
```

Rules:

- Message format: `"metadata sync after {command_name} failed"`.
- Field format: `%e` (Display), named fields with `%` sigil for Display types.
- Never return the metadata error — the critical operation already succeeded.
- `profile_duplicate` (profile.rs:190–193) shows that lookup errors are `.ok().flatten()` silently:

  ```rust
  let source_profile_id = metadata_store.lookup_profile_id(&name).ok().flatten();
  ```

### `MetadataStore` Fail-Soft Initialization

`lib.rs:32–35` — the key difference from other stores (which `process::exit(1)` on failure):

```rust
let metadata_store = MetadataStore::try_new().unwrap_or_else(|error| {
    tracing::warn!(%error, "metadata store unavailable — SQLite features disabled");
    MetadataStore::disabled()
});
```

`MetadataStore::disabled()` returns a store with `conn: None, available: false` that silently
no-ops on all `with_conn` calls. Phase 2 methods automatically inherit this behavior.

---

## Testing Patterns

### In-Memory Store Tests

`mod.rs:129–389` — all unit tests use `MetadataStore::open_in_memory()`:

```rust
#[test]
fn test_observe_profile_write_creates_row() {
    let store = MetadataStore::open_in_memory().unwrap();
    let profile = sample_profile();
    let path = std::path::Path::new("/profiles/elden-ring.toml");

    store.observe_profile_write("elden-ring", &profile, path, SyncSource::AppWrite, None).unwrap();

    // Verify directly in the connection
    let conn = connection(&store);
    let (profile_id, current_filename, ...) = conn.query_row(
        "SELECT profile_id, current_filename FROM profiles WHERE current_filename = ?1",
        params!["elden-ring"],
        |row| Ok((row.get(0)?, row.get(1)?)),
    ).unwrap();
    assert!(!profile_id.trim().is_empty());
}
```

The `connection()` helper extracts `MutexGuard<Connection>` for SQL assertions:

```rust
fn connection(store: &MetadataStore) -> std::sync::MutexGuard<'_, Connection> {
    store.conn.as_ref()
        .expect("metadata store should expose a connection in tests")
        .lock()
        .expect("metadata store mutex should not be poisoned")
}
```

This helper lives in `mod.rs` `#[cfg(test)]` block. Phase 2 tests in the same module can
reuse it. Tests added in new submodule files would need to either `use super::connection` or
duplicate it.

### TempDir Integration Tests

`mod.rs:304–327` — tests requiring `ProfileStore` use `tempfile::tempdir()`:

```rust
#[test]
fn test_sync_profiles_from_store() {
    let temp_dir = tempdir().unwrap();
    let store = MetadataStore::open_in_memory().unwrap();
    let profile_store = ProfileStore::with_base_path(temp_dir.path().join("profiles"));
    // ...
    // temp_dir must stay alive for the duration of the test
}
```

File permission / symlink tests use `MetadataStore::with_path(&db_path)`:

```rust
#[test]
fn test_file_permissions() {
    let temp_dir = tempdir().unwrap();
    let db_path = temp_dir.path().join("metadata.db");
    let _store = MetadataStore::with_path(&db_path).unwrap();
    let mode = fs::metadata(&db_path).unwrap().permissions().mode() & 0o777;
    assert_eq!(mode, 0o600);
}
```

### Sample Data Constructors

`mod.rs:141–176` — `sample_profile()` returns a fully-populated `GameProfile`. Phase 2 tests
should add any additional sample constructors they need (`sample_launch_request()`) alongside
`sample_profile()` in the same `#[cfg(test)]` block in `mod.rs`.

### Test for Disabled Store

`mod.rs:329–361` — every new public `MetadataStore` method must be covered by the
`test_unavailable_store_noop` test verifying that `MetadataStore::disabled()` returns `Ok(...)`:

```rust
#[test]
fn test_unavailable_store_noop() {
    let store = MetadataStore::disabled();
    assert!(store.observe_profile_write(...).is_ok());
    // add Phase 2 methods here
}
```

---

## Patterns to Follow for Phase 2

### New `launcher_sync.rs`

1. Free functions with `conn: &Connection` first arg, same as `profile_sync.rs`.
2. `observe_launcher_exported(conn, profile_name, launcher_slug, script_path, desktop_entry_path) -> Result<(), MetadataStoreError>` — UPSERT on `launchers(profile_id, launcher_slug)`.
3. `sweep_abandoned_operations(conn) -> Result<usize, MetadataStoreError>` — UPDATE with `WHERE finished_at IS NULL AND started_at < datetime('now', '-2 hours')`.
4. Use `db::new_id()` for any new UUID primary keys.
5. Use `Utc::now().to_rfc3339()` for all timestamp columns.

### New `launch_history.rs`

1. `record_launch_started(conn, profile_name, method, log_path) -> Result<String, MetadataStoreError>` — INSERT returning the new `operation_id`.
2. `record_launch_finished(conn, operation_id, outcome, diagnostic_json) -> Result<(), MetadataStoreError>` — UPDATE by `operation_id`.
3. `diagnostic_json` is `Option<String>` — `None` for clean exits without a surfaced report.
4. Serialize `DiagnosticReport` with `serde_json::to_string`, map error to `MetadataStoreError::Corrupt(...)`.

### New Migration (`migrate_2_to_3`)

```rust
// migrations.rs — add after existing if-blocks
if version < 3 {
    migrate_2_to_3(conn)?;
    conn.pragma_update(None, "user_version", 3_u32)
        .map_err(|source| MetadataStoreError::Database {
            action: "update metadata schema version",
            source,
        })?;
}
```

DDL follows the exact structure of `migrate_0_to_1`:

- `CREATE TABLE IF NOT EXISTS` with explicit column types and constraints.
- `CREATE INDEX IF NOT EXISTS` for every foreign key and high-cardinality lookup column.
- FOREIGN KEY references use `REFERENCES profiles(profile_id)` consistent with existing schema.

### New `MetadataStore` Public Methods (in `mod.rs`)

Follow the delegation shape exactly:

```rust
pub fn observe_launcher_exported(
    &self,
    profile_name: &str,
    launcher_slug: &str,
    script_path: &str,
    desktop_entry_path: &str,
) -> Result<(), MetadataStoreError> {
    self.with_conn("observe a launcher export", |conn| {
        launcher_sync::observe_launcher_exported(conn, profile_name, launcher_slug, script_path, desktop_entry_path)
    })
}
```

### Async Bridge in Tauri Commands

`rusqlite::Connection` is `!Send`, so `MetadataStore` calls cannot cross `.await` points.
Use **`tauri::async_runtime::spawn_blocking`** (not `tokio::task::spawn_blocking` — both use
the same executor, but the Tauri alias is the codebase convention per `commands/install.rs`).

`MetadataStore` is `Clone` (wraps `Arc<Mutex<Connection>>`), so clone before moving into
the closure:

```rust
// Inside stream_log_lines, after process exits and report is ready:
let metadata = metadata_store.clone();
tauri::async_runtime::spawn_blocking(move || {
    if let Err(e) = metadata.record_launch_finished(&operation_id, outcome, diagnostic_json) {
        tracing::warn!(%e, %operation_id, "metadata record_launch_finished failed");
    }
}).await.ok();  // .ok() — ignore JoinError, never propagate to frontend
```

### `operation_id` Sentinel Pattern

`record_launch_started` returns `Result<String, MetadataStoreError>`. When the store is
disabled, `with_conn` returns `Ok(String::default())` = `Ok("")`. Callers must treat an
empty string as "store unavailable — skip finish recording":

```rust
// After child is spawned, before spawning the log-stream task:
let operation_id: Option<String> = metadata_store
    .record_launch_started(profile_name, method, &log_path.to_string_lossy())
    .ok()
    .filter(|id| !id.is_empty());

// Later in stream_log_lines, after process exits:
if let Some(op_id) = operation_id {
    let metadata = metadata_store.clone();
    tauri::async_runtime::spawn_blocking(move || {
        if let Err(e) = metadata.record_launch_finished(&op_id, outcome, diagnostic_json) {
            tracing::warn!(%e, operation_id = %op_id, "metadata record_launch_finished failed");
        }
    }).await.ok();
}
```

This prevents orphaned `launch_operations` rows with no `finished_at` in two cases:
1. Store is disabled — `record_launch_started` returns `""`, `filter` drops it, finish is skipped.
2. `record_launch_started` fails with a DB error — `.ok()` converts to `None`, finish is skipped.

### New Model Types (`models.rs`)

Add `LauncherRow`, `LaunchOperationRow` as `pub(crate)` structs with `#[allow(dead_code)]`.
Add `LaunchOutcome` and `DriftState` enums with `#[serde(rename_all = "snake_case")]` and
`as_str()` methods. Re-export public types from `mod.rs` via `pub use models::...`.
