# Context Analysis: SQLite Metadata Layer Phase 2 (Operational History)

Synthesized from: `shared.md`, `feature-spec.md` (lines 189-308), `research-architecture.md`,
`research-patterns.md`, `research-integration.md`, `research-docs.md`.

---

## Executive Summary

Phase 1 is complete: `MetadataStore` with `Arc<Mutex<Connection>>`, schema v2 (`profiles` + `profile_name_history`), and profile sync hooks in Tauri commands. Phase 2 adds two new tables (`launchers`, `launch_operations`), three new `MetadataStore` methods, and integration hooks in the async launch commands and synchronous export commands. The only structural blocker outside the metadata module is `LaunchRequest` missing a `profile_name` field — this must land first before any launch history hook can link a row to a `profile_id`.

---

## Architecture Context

### System Structure

```
metadata/mod.rs          — MetadataStore struct, with_conn helper, public API delegates
metadata/db.rs           — Connection factory, new_id() UUID generation
metadata/migrations.rs   — Sequential user_version runner (currently v2 → Phase 2 adds v3)
metadata/models.rs       — Error types, enums, row structs (add LaunchOutcome, DriftState here)
metadata/profile_sync.rs — Phase 1 profile lifecycle; template for Phase 2 submodules
metadata/launcher_sync.rs  [NEW] — observe_launcher_exported, observe_launcher_deleted, scan
metadata/launch_history.rs [NEW] — record_launch_started, record_launch_finished, sweep
```

### Data Flow

```
launch_game / launch_trainer (async Tauri command)
  → validate request
  → [spawn_blocking] record_launch_started(profile_name, method) → operation_id: Option<String>
  → command.spawn() → child
  → spawn_log_stream(app, log_path, child, method, operation_id)
        └── [detached tokio task] stream_log_lines(...)
              → poll loop (500ms) → child exit
              → analyze(exit_status, log_tail, method) → DiagnosticReport
              → sanitize_diagnostic_report(report)
              → should_surface_report? → app.emit("launch-diagnostic")
              → [spawn_blocking] record_launch_finished(op_id, outcome, report)
              → app.emit("launch-complete")

export_launchers (synchronous Tauri command)
  → export_launchers_core(&request) → SteamExternalLauncherExportResult
  → observe_launcher_exported(profile_name, slug, script_path, desktop_path)
  → Ok(result)
```

### Integration Points

Phase 2 hooks into **three existing Tauri commands** only:

- `commands/launch.rs`: `launch_game` (pre-spawn) + `stream_log_lines` (post-analyze)
- `commands/export.rs`: `export_launchers`, `delete_launcher`/`delete_launcher_by_slug`, `rename_launcher`
- `src-tauri/src/startup.rs`: `run_metadata_reconciliation` (adds `sweep_abandoned_operations` call)

No new Tauri commands are added by Phase 2. `invoke_handler!` registration in `lib.rs` does not change.

---

## Critical Files Reference

| File                                                                                                                            | Why Critical                                                                                                                                   |
| ------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------- |
| `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/metadata/mod.rs`              | Add all three Phase 2 public methods here via `with_conn` delegation                                                                           |
| `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/metadata/migrations.rs`       | Add `migrate_2_to_3()` with DDL for `launchers` + `launch_operations`; add `if version < 3` guard                                              |
| `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/metadata/models.rs`           | Add `LaunchOutcome`, `DriftState` enums; `LauncherRow`, `LaunchOperationRow` structs                                                           |
| `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/launch/request.rs`            | **Phase 2 blocker**: add `pub profile_name: Option<String>` with `#[serde(default)]` at lines 16-37                                            |
| `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src-tauri/src/commands/launch.rs`                       | Wire `record_launch_started` (~line 72) and `record_launch_finished` (~line 211); adjust `spawn_log_stream` signature                          |
| `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src-tauri/src/commands/export.rs`                       | Add `State<'_, MetadataStore>` to `export_launchers` (line 20), `delete_launcher` (47), `delete_launcher_by_slug` (66), `rename_launcher` (81) |
| `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src-tauri/src/startup.rs`                               | Add `sweep_abandoned_operations` call after `sync_profiles_from_store` in `run_metadata_reconciliation`                                        |
| `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/launch/diagnostics/models.rs` | `DiagnosticReport` — source for `diagnostic_json`; `ExitCodeInfo.failure_mode` + `severity` are promoted columns                               |
| `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/export/launcher_store.rs`     | `LauncherInfo`, `derive_launcher_paths()` — Phase 2 `launchers` table maps to these types                                                      |
| `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src-tauri/src/commands/install.rs`                      | Canonical `tauri::async_runtime::spawn_blocking` pattern — copy for Phase 2 async bridge                                                       |
| `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/launch/script_runner.rs`      | Test fixtures (struct literals) need `profile_name: None` added after `LaunchRequest` change                                                   |

### Files to Create

- `crates/crosshook-core/src/metadata/launcher_sync.rs` — free functions: `observe_launcher_exported`, `observe_launcher_deleted`, `observe_launcher_renamed`, `observe_launcher_deleted_by_slug`
- `crates/crosshook-core/src/metadata/launch_history.rs` — free functions: `record_launch_started`, `record_launch_finished`, `sweep_abandoned_operations`

---

## Design Decisions (Locked)

| Decision                                          | Choice                                                                                                         | Rationale                                                                            |
| ------------------------------------------------- | -------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------------------------------ |
| `launch_operations` PK                            | UUID TEXT via `db::new_id()`                                                                                   | Consistent with Phase 1 `profiles` PK; avoids being the only AUTOINCREMENT in schema |
| `launchers` PK                                    | UUID TEXT (`launcher_id`) + index on `launcher_slug`                                                           | Nullable `profile_id` in composite PK creates SQLite ambiguity                       |
| `profile_name` in `LaunchRequest`                 | `Option<String>` with `#[serde(default)]`                                                                      | Backwards compatible; avoids sentinel-value checking                                 |
| `SteamExternalLauncherExportRequest.profile_name` | `Option<String>` with `#[serde(default)]`                                                                      | Same reasoning; frontend callers unaffected                                          |
| Startup sweep threshold                           | Rows with `status = 'started'` and `finished_at IS NULL` after 24h                                             | Run in `.setup()` after reconciliation; non-fatal warn-only                          |
| DiagnosticReport truncation                       | Truncate `diagnostic_json` before INSERT when > 4 096 bytes                                                    | Still record outcome, exit_code, severity, failure_mode in promoted columns          |
| Slug rename rule (RF-2)                           | Old `(profile_id, slug)` row tombstoned; new row created on next re-export                                     | No in-place rename — slug is filesystem identity                                     |
| `operation_id` sentinel                           | `record_launch_started` returns `""` when store disabled; caller filters with `.filter(\|id\| !id.is_empty())` | Prevents orphaned half-records when store is unavailable                             |
| `tauri::async_runtime::spawn_blocking`            | Use Tauri alias, not `tokio::task::spawn_blocking` directly                                                    | Matches codebase convention per `commands/install.rs`                                |

---

## Phase 2 Schema (locked)

```sql
-- migrate_2_to_3 DDL
CREATE TABLE IF NOT EXISTS launchers (
    launcher_id         TEXT PRIMARY KEY,
    profile_id          TEXT REFERENCES profiles(profile_id),
    launcher_slug       TEXT NOT NULL,
    display_name        TEXT NOT NULL,
    script_path         TEXT NOT NULL,
    desktop_entry_path  TEXT NOT NULL,
    drift_state         TEXT NOT NULL DEFAULT 'unknown',
    created_at          TEXT NOT NULL,
    updated_at          TEXT NOT NULL
);
CREATE INDEX IF NOT EXISTS idx_launchers_profile_id    ON launchers(profile_id);
CREATE INDEX IF NOT EXISTS idx_launchers_launcher_slug ON launchers(launcher_slug);

CREATE TABLE IF NOT EXISTS launch_operations (
    operation_id    TEXT PRIMARY KEY,
    profile_id      TEXT REFERENCES profiles(profile_id),
    profile_name    TEXT,
    launch_method   TEXT NOT NULL,
    status          TEXT NOT NULL DEFAULT 'started',
    exit_code       INTEGER,
    signal          INTEGER,
    log_path        TEXT,
    diagnostic_json TEXT,               -- max 4 096 bytes (W3)
    severity        TEXT,               -- promoted from DiagnosticReport.severity
    failure_mode    TEXT,               -- promoted from DiagnosticReport.exit_info.failure_mode
    started_at      TEXT NOT NULL,
    finished_at     TEXT
);
CREATE INDEX IF NOT EXISTS idx_launch_ops_profile_id ON launch_operations(profile_id);
CREATE INDEX IF NOT EXISTS idx_launch_ops_started_at ON launch_operations(started_at);
```

`drift_state` values: `unknown`, `aligned`, `missing`, `moved`, `stale`
`status` values: `started`, `succeeded`, `failed`, `abandoned`

---

## Patterns to Follow

- **`with_conn` fail-soft delegation** (`metadata/mod.rs:56-73`): every Phase 2 public method goes through this; returns `Ok(T::default())` when store is disabled. The `T: Default` bound is load-bearing — `()`, `String`, `Option<String>` all satisfy it.
- **Free function + module delegation** (`metadata/profile_sync.rs`): `launcher_sync.rs` and `launch_history.rs` are private submodules with free functions taking `conn: &Connection` as first arg. `mod.rs` wraps them via `with_conn`.
- **Structured error mapping**: `MetadataStoreError::Database { action: "lowercase gerund phrase", source }` for all SQL errors. `action` is always `&'static str` — never `format!()`.
- **Enum pattern** (`metadata/models.rs:69-93`): `LaunchOutcome` and `DriftState` derive `Debug + Clone + Copy + Serialize + Deserialize` with `#[serde(rename_all = "snake_case")]` and expose `as_str() -> &'static str` for SQL column storage.
- **Warn-and-continue** (`commands/profile.rs:106-113`): `if let Err(e) { tracing::warn!(%e, profile_name = %name, "metadata sync after {cmd} failed"); }` — metadata failures never propagate.
- **UPSERT reconciliation**: `INSERT ... ON CONFLICT DO UPDATE` for `observe_launcher_exported` (same as `observe_profile_write`).
- **Sequential migration** (`migrations.rs`): `if version < N { migrate_N_to_M(conn)?; conn.pragma_update(None, "user_version", N_u32)?; }` — idempotent.
- **Row structs**: `pub(crate)` with `#[allow(dead_code)]`, timestamps as `String` (RFC 3339), no `#[derive(Default)]`.

---

## Cross-Cutting Concerns

### Async Bridge (spawn_blocking)

`rusqlite::Connection` is `!Send`. `launch_game` and `launch_trainer` are `async fn`. All metadata writes from these commands must use `tauri::async_runtime::spawn_blocking`:

```rust
// In launch_game / launch_trainer, before command.spawn():
let metadata = app.state::<MetadataStore>().inner().clone();
let profile_name = request.profile_name.clone().unwrap_or_default();
let operation_id: Option<String> = tauri::async_runtime::spawn_blocking(move || {
    metadata.record_launch_started(&profile_name, method)
})
.await
.ok()
.and_then(|r| r.ok())
.filter(|id| !id.is_empty());

// In stream_log_lines, after analyze():
if let Some(op_id) = operation_id {
    let metadata = app.state::<MetadataStore>().inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        if let Err(e) = metadata.record_launch_finished(&op_id, outcome, diagnostic_json) {
            tracing::warn!(%e, operation_id = %op_id, "metadata record_launch_finished failed");
        }
    }).await.ok();
}
```

Export commands are synchronous — no `spawn_blocking` needed there.

### Security Constraints

| Ref | Rule                                           | Implementation                                                                                                                                                  |
| --- | ---------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| W3  | `diagnostic_json` max 4 096 bytes              | `pub const MAX_DIAGNOSTIC_JSON_BYTES: usize = 4_096;` in `models.rs`; truncate or omit before INSERT; still record promoted `severity` + `failure_mode` columns |
| W6  | Re-validate stored paths before filesystem ops | `validate_stored_path(path)` utility; applies before any `fs::` call using `launchers.script_path` or `launchers.desktop_entry_path`                            |
| W2  | Path sanitization at IPC boundary              | `sanitize_display_path()` (already in `commands/shared.rs` per Phase 1) applied to `log_path` before storing in `launch_operations`                             |
| W7  | No `format!()` in SQL                          | All SQL strings are string literals; `execute_batch()` receives only hard-coded DDL                                                                             |

### Fail-Soft at All Levels

1. `MetadataStore` is always present in Tauri state — no `Option<MetadataStore>`.
2. All Phase 2 methods route through `with_conn` — auto-no-op when `available = false`.
3. All Tauri command call sites use `if let Err(e) { tracing::warn! }` — never `?`.
4. `operation_id: Option<String>` — empty string filtered to `None` so `record_launch_finished` is skipped rather than writing an orphaned row.

### Startup Sweep

Add inside `run_metadata_reconciliation` in `startup.rs`, after `sync_profiles_from_store`:

```rust
if let Err(error) = metadata_store.sweep_abandoned_operations() {
    tracing::warn!(%error, "startup abandoned operation sweep failed");
}
```

SQL: `UPDATE launch_operations SET status='abandoned', finished_at=?1 WHERE status='started' AND started_at < datetime('now', '-24 hours')`

Safe to run before the Tauri event loop because no async launch tasks can be in-flight during `setup()`.

---

## Parallelization Opportunities

Once `LaunchRequest.profile_name` is added (the sole external blocker), Phase 2 core and integration work can proceed with these parallel tracks:

| Track                  | Tasks                                                                                                                | Dependency                                                  |
| ---------------------- | -------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------- |
| A — Metadata core      | `models.rs` additions → `migrations.rs` v3 DDL → `launcher_sync.rs` → `launch_history.rs` → `mod.rs` method wrappers | Sequential within track                                     |
| B — Launch integration | Wire `record_launch_started`/`finished` into `commands/launch.rs`                                                    | Depends on Track A: `launch_history.rs` + `mod.rs` wrappers |
| C — Export integration | Wire `observe_launcher_exported`/deleted/renamed into `commands/export.rs`                                           | Depends on Track A: `launcher_sync.rs` + `mod.rs` wrappers  |
| D — Startup            | Add `sweep_abandoned_operations` call to `startup.rs`                                                                | Depends on Track A: `launch_history.rs`                     |
| E — Tests              | Phase 2 unit tests (`open_in_memory`, `disabled` store no-op, `test_unavailable_store_noop`)                         | Runs after each Track A submodule lands                     |

Tracks B, C, D, E can all run in parallel once Track A completes.

---

## Implementation Constraints

1. **No new Cargo dependencies** — `rusqlite`, `serde_json`, `chrono`, `uuid`, `tokio` all already present.
2. **`profile_name` must resolve to `profile_id`** via `lookup_profile_id(conn, name)` inside `launch_history.rs`; if not found (profile not yet in metadata), store `profile_id = NULL` and `profile_name = TEXT` — do not fail the launch.
3. **`delete_launcher_by_slug`** has no `profile_name` — reverse lookup from `launchers` table by slug to find `profile_id`; accept `NULL` FK for slug-only deletes.
4. **`sanitize_launcher_slug()`** from `export/launcher.rs:265` is the sole source for slug computation — never re-derive in the metadata layer.
5. **`request.resolved_method()`** (not raw `request.method`) is the launch method string to store in `launch_operations.launch_method`.
6. **Test fixtures** in `launch/script_runner.rs` (struct literals at lines 353, 406, 498) need `profile_name: None` after `LaunchRequest` is changed — compile-time check, not a logic change.
7. **`test_unavailable_store_noop`** in `metadata/mod.rs` must be extended to cover all three Phase 2 methods.
8. **RF-2 (slug rename tombstone)**: on `rename_launcher`, update old row `drift_state = 'missing'`, `updated_at`; do not delete the row — history is preserved.
9. **No retroactive mapping**: existing launcher files are not tracked until the user explicitly re-exports through CrossHook after Phase 2 ships.
10. **Watermark rule**: do not record a `launchers` row for a launcher whose watermark check would fail; `native` method profiles have no launcher export and must not produce `launchers` rows.

---

## Key Recommendations

1. **Resolve `LaunchRequest.profile_name` first** — one-line change to `launch/request.rs`, but it gates all launch history work. Add `Option<String>` with `#[serde(default)]`; update test fixtures in `script_runner.rs` with `profile_name: None`.
2. **Add models before submodules** — `LaunchOutcome` and `DriftState` enums must be in `models.rs` before `launcher_sync.rs` and `launch_history.rs` compile.
3. **Write DDL before free functions** — `migrate_2_to_3` must create the tables before any sync function's SQL compiles into a tested path.
4. **Use `tauri::async_runtime::spawn_blocking`, not `tokio::task`** — codebase convention from `commands/install.rs`; both use the same Tokio executor but naming must match.
5. **`operation_id` must thread through `spawn_log_stream`** — add `operation_id: Option<String>` as a fifth parameter to the private `spawn_log_stream` + `stream_log_lines` functions; capture in the detached task closure.
6. **Never `?` on metadata calls in Tauri commands** — always `if let Err(e) { warn! }` regardless of how obviously the operation should succeed.
7. **`DiagnosticReport` always recorded regardless of `should_surface_report`** — the surface decision controls the UI event, not the DB write.
8. **Serialization size check before INSERT** — `let json = serde_json::to_string(&report)?; let bounded = if json.len() > MAX_DIAGNOSTIC_JSON_BYTES { None } else { Some(json) };`; still record promoted columns even when `diagnostic_json` is `None`.
