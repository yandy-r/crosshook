# Architecture Research: SQLite Metadata Layer Phase 2 (Operational History)

## Current Metadata Module (Phase 1)

Phase 1 is fully implemented across five files under `src/crosshook-native/crates/crosshook-core/src/metadata/`.

### Module Structure

```
metadata/
  mod.rs          — MetadataStore struct, public API methods, with_conn helper
  db.rs           — Connection factory, PRAGMA setup, symlink check, chmod 0600
  migrations.rs   — user_version-based migration runner (0→1→2 currently at v2)
  models.rs       — MetadataStoreError, SyncReport, SyncSource, ProfileRow
  profile_sync.rs — Profile lifecycle reconciliation (observe_write, rename, delete, sync_from_store)
```

### MetadataStore Struct (`mod.rs:14-18`)

```rust
pub struct MetadataStore {
    conn: Option<Arc<Mutex<Connection>>>,
    available: bool,
}
```

- `Clone` is derived; `Arc<Mutex<Connection>>` allows cheap cloning across Tauri state.
- `disabled()` constructor sets `available = false`; used as soft-fail fallback in `lib.rs:32-35`.
- In `lib.rs`, `MetadataStore` is managed via `.manage(metadata_store)` at line 80.

### `with_conn` Helper (`mod.rs:56-73`)

```rust
fn with_conn<F, T>(&self, action: &'static str, f: F) -> Result<T, MetadataStoreError>
where
    F: FnOnce(&Connection) -> Result<T, MetadataStoreError>,
    T: Default,
```

- If `available == false` or `conn` is `None`, returns `Ok(T::default())` — complete no-op.
- Locks the mutex; maps poison errors to `MetadataStoreError::Corrupt`.
- All Phase 2 methods must follow this same pattern.
- The `T: Default` bound is load-bearing; Phase 2 return types (`()`, counters, `Option<String>`) all satisfy it.

### Migration System (`migrations.rs`)

- Current schema version: **2** (migration 1→2 added `source TEXT` column to `profiles`).
- Phase 2 requires a **3rd migration** (version 2→3) adding `launchers` and `launch_operations` tables.
- Migration function signature pattern: `fn migrate_N_to_M(conn: &Connection) -> Result<(), MetadataStoreError>`.
- `run_migrations` uses `if version < N` guards — idempotent if run on an already-migrated DB.
- New guard needed: `if version < 3 { migrate_2_to_3(conn)?; conn.pragma_update(None, "user_version", 3_u32)?; }`.

### Error Model (`models.rs:8-21`)

```rust
pub enum MetadataStoreError {
    HomeDirectoryUnavailable,
    Database { action: &'static str, source: SqlError },
    Io { action: &'static str, path: PathBuf, source: std::io::Error },
    Corrupt(String),
    SymlinkDetected(PathBuf),
}
```

- `action` strings are always `&'static str` — never runtime-constructed strings.
- Phase 2 should reuse `Database { action: "...", source }` for all SQL errors.
- `From<SqlError>` impl exists for ergonomic `?` on `conn.execute(...)` calls.

### Profile Sync Patterns (`profile_sync.rs`)

Key patterns usable in Phase 2:

- `validate_profile_name(name)` called at entry to every public function.
- Timestamps via `Utc::now().to_rfc3339()`.
- `db::new_id()` generates UUID v4 strings for primary keys.
- `Transaction::new_unchecked(conn, TransactionBehavior::Immediate)` used for multi-step operations (e.g., rename).
- `OptionalExtension` from rusqlite used for `query_row(...).optional()` to return `Option<T>`.
- ON CONFLICT upsert pattern used in `observe_profile_write` — preferred over separate INSERT/UPDATE logic.

---

## Launch System Architecture

### `LaunchRequest` Struct (`launch/request.rs:15-37`)

Current fields:

- `method: String`
- `game_path: String`
- `trainer_path: String`
- `trainer_host_path: String`
- `trainer_loading_mode: TrainerLoadingMode`
- `steam: SteamLaunchConfig`
- `runtime: RuntimeLaunchConfig`
- `optimizations: LaunchOptimizationsRequest`
- `launch_trainer_only: bool`
- `launch_game_only: bool`

**Phase 2 requires adding `profile_name: String`** with `#[serde(default)]`. This field is populated by the frontend before invoking `launch_game` or `launch_trainer`. It is used by the metadata layer to look up `profile_id` for the `launch_operations` FK.

The `log_target_slug()` method (`request.rs:104-136`) is the current way to derive a short identifier — Phase 2 should use `profile_name` directly instead.

### Launch Flow (`commands/launch.rs`)

```
launch_game (async, line 48) / launch_trainer (async, line 86)
  ├── validate(&request)
  ├── create_log_path(kind, slug) -> log_path
  ├── command.spawn() -> child
  └── spawn_log_stream(app, log_path, child, method)   [line 76/116]
        └── [detached tokio task] stream_log_lines(app, log_path, child, method)  [line 142]
              ├── polling loop: read log, emit "launch-log" events
              ├── child.try_wait() -> exits loop on completion
              ├── final log read
              ├── safe_read_tail() -> log_tail string
              ├── analyze(exit_status, &log_tail, method) -> DiagnosticReport  [line 211]
              ├── sanitize_diagnostic_report(report)
              ├── should_surface_report(&report) -> app.emit("launch-diagnostic", &report)  [line 215]
              └── app.emit("launch-complete", {code, signal})  [line 221]
```

**Phase 2 insertion points:**

1. **Before `command.spawn()`** in `launch_game`/`launch_trainer` (lines 74/114): call `record_launch_started(profile_name, method)` → returns `operation_id`.
2. **After `analyze()` call** inside `stream_log_lines` (after line 211): call `record_launch_finished(operation_id, exit_status, &report)`.
3. Both calls cross the async/sync boundary and require `spawn_blocking` (see Async Bridge section).

The `operation_id` must be threaded from `launch_game`/`launch_trainer` into the spawned log stream task via closure capture.

### `DiagnosticReport` Fields (`launch/diagnostics/models.rs:10-18`)

```rust
pub struct DiagnosticReport {
    pub severity: ValidationSeverity,       // Fatal | Warning | Info
    pub summary: String,
    pub exit_info: ExitCodeInfo,            // contains failure_mode, code, signal, description
    pub pattern_matches: Vec<PatternMatch>,
    pub suggestions: Vec<ActionableSuggestion>,
    pub launch_method: String,
    pub log_tail_path: Option<String>,
    pub analyzed_at: String,               // RFC3339 timestamp
}
```

`DiagnosticReport` derives `Serialize + Deserialize` — safe to store as `serde_json::to_string(&report)` in the `diagnostic_json TEXT` column of `launch_operations`. `MAX_LOG_TAIL_BYTES` (2 MB) is the tail buffer for `analyze()`; the serialized `DiagnosticReport` is much smaller (typically < 4 KB).

---

## Export/Launcher System

### `SteamExternalLauncherExportRequest` (`export/launcher.rs:14-26`)

```rust
pub struct SteamExternalLauncherExportRequest {
    pub method: String,
    pub launcher_name: String,
    pub trainer_path: String,
    pub trainer_loading_mode: TrainerLoadingMode,
    pub launcher_icon_path: String,
    pub prefix_path: String,
    pub proton_path: String,
    pub steam_app_id: String,
    pub steam_client_install_path: String,
    pub target_home_path: String,
}
```

**Notably absent: `profile_name`**. Phase 2 must either add `profile_name: String` to this struct (with `#[serde(default)]`) or look up profile_name from slug at insert time.

### Slug Generation (`export/launcher.rs:265-290`)

`sanitize_launcher_slug(value: &str) -> String` — lowercases, replaces non-alphanumeric with `-`, collapses runs, trims edges. Fallback: `"crosshook-trainer"`. This slug is the filesystem identity for exported launchers.

`resolve_display_name(preferred_name, steam_app_id, trainer_path) -> String` — prefers `launcher_name`, falls back to trainer file stem, then `steam-{app_id}-trainer`.

Path structure:

- Script: `~/.local/share/crosshook/launchers/{slug}-trainer.sh`
- Desktop: `~/.local/share/applications/crosshook-{slug}-trainer.desktop`

### `LauncherInfo` Struct (`export/launcher_store.rs:27-43`)

```rust
pub struct LauncherInfo {
    pub display_name: String,
    pub launcher_slug: String,
    pub script_path: String,
    pub desktop_entry_path: String,
    pub script_exists: bool,
    pub desktop_entry_exists: bool,
    pub is_stale: bool,         // populated only when profile context is available
}
```

Phase 2 `launchers` table tracks: `profile_id FK`, `launcher_slug`, `script_path`, `desktop_entry_path`, `drift_state`. The `is_stale` field from `LauncherInfo` maps to drift state.

### Export Command Signatures (`commands/export.rs`)

All export commands are **synchronous** (not `async`). They take `State<'_, ProfileStore>` only where needed. No `MetadataStore` injection currently. Phase 2 adds `State<'_, MetadataStore>` to:

- `export_launchers` (line 19) — to call `observe_launcher_exported` after `export_launchers_core`
- `delete_launcher` (line 47) / `delete_launcher_by_slug` (line 66) — to call `observe_launcher_deleted`
- `rename_launcher` (line 81) — to call `observe_launcher_renamed`

**Critical gap**: `delete_launcher_by_slug` (line 66) has no `profile_name` — it only receives a `launcher_slug`. Phase 2 must do a reverse lookup from `launchers` table by slug to find the `profile_id`, or accept that FK is nullable for slug-only deletes.

---

## Diagnostics System

### Analysis Pipeline (`launch/diagnostics/mod.rs:17-37`)

```rust
pub fn analyze(exit_status: Option<ExitStatus>, log_tail: &str, method: &str) -> DiagnosticReport
```

Called in `stream_log_lines` after the child exits. Returns a fully-populated `DiagnosticReport`.

`should_surface_report(&report) -> bool` — determines whether to emit `launch-diagnostic`. This is separate from whether to record: Phase 2 should **always** record the DiagnosticReport in `launch_operations`, regardless of surface worthiness.

### Serialization

`DiagnosticReport` derives `serde::Serialize + Deserialize`. `serde_json` is already in `crosshook-core/Cargo.toml` at line 10. Use `serde_json::to_string(&report)` for the `diagnostic_json` column.

---

## Async Bridge Requirements

### The Core Problem

`rusqlite::Connection` is `!Send` — it cannot be moved across thread boundaries. The `MetadataStore` wraps it in `Arc<Mutex<Connection>>`, but accessing it from an `async` context still requires special handling.

### Current Pattern in Commands

All profile commands (`commands/profile.rs`) are **synchronous** (`fn`, not `async fn`). They can call `metadata_store.observe_*()` directly — no `spawn_blocking` needed. This is why profile sync works today without any special bridge.

### `launch_game` / `launch_trainer` Are Async

Both are `async fn` Tauri commands (lines 48, 86 in `commands/launch.rs`). The entire launch flow runs on the Tauri async runtime (Tokio).

`stream_log_lines` is an `async fn` spawned via `tauri::async_runtime::spawn` (line 131). Inside it, after the child exits, `analyze()` is called.

### Required Pattern for Phase 2 Metadata Writes

For the `record_launch_started` call in `launch_game`/`launch_trainer`:

```rust
let metadata = metadata_store.clone();  // cheap Arc clone
let op_id = tokio::task::spawn_blocking(move || {
    metadata.record_launch_started(&profile_name, method)
}).await
  .map_err(|join_err| format!("spawn_blocking panicked: {join_err}"))??;
```

For `record_launch_finished` inside `stream_log_lines` (which is also async):

```rust
let metadata = metadata_store.clone();
let report_clone = report.clone();
tokio::task::spawn_blocking(move || {
    metadata.record_launch_finished(op_id, exit_status_code, &report_clone)
}).await
  .map_err(|join_err| tracing::warn!("metadata record_launch_finished panicked: {join_err}"))
  .ok();
```

The `operation_id` (`String`) must be captured in the `spawn_log_stream` closure, then passed into `stream_log_lines`. This requires adjusting `stream_log_lines`'s signature from `(app, log_path, child, method)` to add `operation_id: Option<String>` (optional so it degrades gracefully if metadata is unavailable).

### Why `Option<String>` for `operation_id`

`record_launch_started` can fail (store disabled, lock contention). If it returns `Err`, `operation_id` is `None` and `record_launch_finished` is skipped — no orphaned half-record.

---

## Tauri Command Integration Points

### `commands/launch.rs` Integration Points

**Point 1 — `launch_game` (line 48)**:

```rust
pub async fn launch_game(app: AppHandle, request: LaunchRequest) -> Result<LaunchResult, String> {
    // ... existing validate, log_path setup ...

    // PHASE 2: record start — before spawn()
    let metadata_store = app.state::<MetadataStore>().inner().clone();
    let profile_name = request.profile_name.clone();
    let op_id = tokio::task::spawn_blocking(move || {
        metadata_store.record_launch_started(&profile_name, method)
    }).await.ok().and_then(|r| r.ok());

    let child = command.spawn()...;
    spawn_log_stream(app, log_path.clone(), child, method, op_id);  // pass op_id

    Ok(LaunchResult { ... })
}
```

**Point 2 — `stream_log_lines` (after line 211)**:

After `let report = sanitize_diagnostic_report(report);`:

```rust
if let Some(op_id) = operation_id {
    let metadata_store = app.state::<MetadataStore>().inner().clone();
    let report_clone = report.clone();
    let exit_code = exit_status.and_then(|s| s.code());
    tokio::task::spawn_blocking(move || {
        let _ = metadata_store.record_launch_finished(&op_id, exit_code, &report_clone);
    }).await.ok();
}
```

`app: AppHandle` is already available in `stream_log_lines` for `app.emit()` calls. Accessing `app.state::<MetadataStore>()` requires the `Manager` trait, already imported at line 17.

### `commands/export.rs` Integration Points

All export commands are synchronous, so no `spawn_blocking` is needed. Add `State<'_, MetadataStore>` parameter:

**`export_launchers` (line 19)**:

```rust
pub fn export_launchers(
    request: SteamExternalLauncherExportRequest,
    metadata_store: State<'_, MetadataStore>,
) -> Result<SteamExternalLauncherExportResult, String> {
    let result = export_launchers_core(&request).map_err(|e| e.to_string())?;
    if let Err(e) = metadata_store.observe_launcher_exported(&request.profile_name, &result) {
        tracing::warn!(%e, "metadata sync after export_launchers failed");
    }
    Ok(result)
}
```

**`delete_launcher_by_slug` (line 66)**: Needs `State<'_, MetadataStore>`. No `profile_name` available — call `metadata_store.observe_launcher_deleted_by_slug(&launcher_slug)` which does a reverse lookup from the `launchers` table.

**`rename_launcher` (line 81)**: Needs `State<'_, MetadataStore>`. Call `metadata_store.observe_launcher_renamed(&old_launcher_slug, &result.new_slug)`.

### New Handler Registration in `lib.rs`

No new Tauri commands are added by Phase 2 — only existing commands gain `State<'_, MetadataStore>` parameters. The `invoke_handler!` macro registration at line 85-128 does not change.

---

## Key Dependencies

Phase 2 has no new Cargo dependencies. All required crates are already present:

| Crate        | Version         | Usage in Phase 2                                                |
| ------------ | --------------- | --------------------------------------------------------------- |
| `rusqlite`   | `0.38, bundled` | `launchers` and `launch_operations` tables                      |
| `serde_json` | `1`             | `DiagnosticReport` serialization for `diagnostic_json` column   |
| `chrono`     | `0.4`           | RFC3339 timestamps for `started_at`, `finished_at`              |
| `uuid`       | `1, v4`         | `operation_id` UUID for `launch_operations` PK                  |
| `tokio`      | `1`             | `spawn_blocking` for metadata writes from async launch commands |

Phase 2 new files to create:

- `src/crosshook-native/crates/crosshook-core/src/metadata/launcher_sync.rs` — `observe_launcher_exported`, `observe_launcher_deleted`, `observe_launcher_renamed`, `observe_launcher_deleted_by_slug`
- `src/crosshook-native/crates/crosshook-core/src/metadata/launch_history.rs` — `record_launch_started`, `record_launch_finished`, `sweep_abandoned_operations`

Phase 2 files to modify:

- `metadata/mod.rs` — add public method wrappers for all Phase 2 functions
- `metadata/migrations.rs` — add `migrate_2_to_3()` + guard in `run_migrations()`
- `launch/request.rs` — add `profile_name: String` field with `#[serde(default)]`
- `commands/launch.rs` — add `spawn_blocking` hooks pre-spawn and post-analyze
- `commands/export.rs` — add `State<'_, MetadataStore>` to export/delete/rename commands

Also must update `lib.rs` invoke_handler registration if any command signature changes require it (signatures with new State params do not require re-listing, but are a compile-time check).

---

## Schema for Phase 2 Migration

```sql
CREATE TABLE IF NOT EXISTS launchers (
    launcher_id   TEXT PRIMARY KEY,
    profile_id    TEXT REFERENCES profiles(profile_id),
    launcher_slug TEXT NOT NULL,
    script_path   TEXT NOT NULL,
    desktop_entry_path TEXT NOT NULL,
    drift_state   TEXT NOT NULL DEFAULT 'unknown',
    created_at    TEXT NOT NULL,
    updated_at    TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_launchers_profile_id ON launchers(profile_id);
CREATE INDEX IF NOT EXISTS idx_launchers_launcher_slug ON launchers(launcher_slug);

CREATE TABLE IF NOT EXISTS launch_operations (
    operation_id  TEXT PRIMARY KEY,
    profile_id    TEXT REFERENCES profiles(profile_id),
    profile_name  TEXT,
    launch_method TEXT NOT NULL,
    status        TEXT NOT NULL DEFAULT 'started',
    exit_code     INTEGER,
    diagnostic_json TEXT,
    started_at    TEXT NOT NULL,
    finished_at   TEXT
);
CREATE INDEX IF NOT EXISTS idx_launch_ops_profile_id ON launch_operations(profile_id);
CREATE INDEX IF NOT EXISTS idx_launch_ops_started_at ON launch_operations(started_at);
```

- `drift_state` values: `'current'`, `'stale'`, `'missing'`, `'unknown'`
- `status` values: `'started'`, `'succeeded'`, `'failed'`, `'abandoned'`
- Startup sweep: `UPDATE launch_operations SET status='abandoned' WHERE status='started' AND started_at < datetime('now', '-24 hours')`
