# SQLite Metadata Layer Phase 2 (Operational History) — Implementation Plan

Phase 2 extends the existing `MetadataStore` (Phase 1: profile sync with `Arc<Mutex<Connection>>`, schema v2) with two new tables (`launchers` for export tracking with drift detection, `launch_operations` for launch history with `DiagnosticReport` JSON storage), three new public methods, and integration hooks in async launch commands (via `spawn_blocking`) and synchronous export commands. The sole external blocker is `LaunchRequest` missing a `profile_name` field — once added as `Option<String>` with `#[serde(default)]`, all launch history hooks can link operations to profile identities. Schema version bumps from v2 to v3; no new Cargo dependencies are required.

## Critically Relevant Files and Documentation

- docs/plans/sqlite3-addition/shared.md: Phase 2 shared context — locked design decisions, schema DDL, patterns, security constraints
- docs/plans/sqlite3-addition/feature-spec.md: Master spec — Phase 2 schema (lines 189-222), API design (lines 246-249), business rules, edge cases
- docs/plans/sqlite3-addition/research-architecture.md: Current metadata module structure, launch flow, async bridge requirements, integration points
- docs/plans/sqlite3-addition/research-patterns.md: Phase 1 patterns extracted from source — with_conn, profile_sync, enum as_str, migration, test patterns
- docs/plans/sqlite3-addition/research-integration.md: All Tauri command signatures, LaunchRequest gap, DiagnosticReport size, hook locations
- docs/plans/sqlite3-addition/research-security.md: W3 (4KB diagnostic limit), W6 (re-validate stored paths), W2 (path sanitization), W7 (no format! in SQL)
- docs/plans/sqlite3-addition/research-docs.md: Phase 2 requirements, edge cases, documentation gaps, must-read priority list
- docs/plans/sqlite3-addition/analysis-context.md: Condensed Phase 2 context — async bridge pattern, security constraints, parallelization tracks
- docs/plans/sqlite3-addition/analysis-code.md: Implementation patterns with code examples — with_conn shapes, UPSERT, spawn_blocking, test patterns
- docs/plans/sqlite3-addition/analysis-tasks.md: Task structure analysis — 10 tasks across 5 phases, dependency DAG, parallelization schedule
- src/crosshook-native/crates/crosshook-core/src/metadata/mod.rs: MetadataStore struct, with_conn helper (lines 56-73), Phase 1 public API — all Phase 2 methods added here
- src/crosshook-native/crates/crosshook-core/src/metadata/models.rs: MetadataStoreError, SyncSource enum with as_str() — template for LaunchOutcome and DriftState
- src/crosshook-native/crates/crosshook-core/src/metadata/migrations.rs: Sequential migration runner (v0→v1→v2) — Phase 2 adds v2→v3
- src/crosshook-native/crates/crosshook-core/src/metadata/profile_sync.rs: Free function pattern (conn: &Connection first arg) — template for launcher_sync.rs and launch_history.rs
- src/crosshook-native/crates/crosshook-core/src/launch/request.rs: LaunchRequest struct (lines 16-37) — Phase 2 blocker: add profile_name
- src/crosshook-native/crates/crosshook-core/src/launch/diagnostics/models.rs: DiagnosticReport, ExitCodeInfo, FailureMode — serialized to diagnostic_json column
- src/crosshook-native/crates/crosshook-core/src/export/launcher.rs: SteamExternalLauncherExportRequest (lines 14-26), SteamExternalLauncherExportResult, sanitize_launcher_slug (line 265)
- src/crosshook-native/crates/crosshook-core/src/export/launcher_store.rs: LauncherInfo, derive_launcher_paths() — launchers table maps to these types
- src/crosshook-native/src-tauri/src/commands/launch.rs: Async launch_game/launch_trainer, spawn_log_stream, stream_log_lines — Phase 2 hook points
- src/crosshook-native/src-tauri/src/commands/export.rs: Synchronous export commands — Phase 2 adds State<MetadataStore> and observe hooks
- src/crosshook-native/src-tauri/src/commands/install.rs: Canonical spawn_blocking pattern (lines 10-18) — template for async metadata bridge
- src/crosshook-native/src-tauri/src/startup.rs: run_metadata_reconciliation — Phase 2 adds sweep_abandoned_operations call
- CLAUDE.md: Project conventions — commit messages, build commands, Rust style, test commands

## Implementation Plan

### Phase 1: Prerequisites

#### Task 1.1: Add `profile_name` to `LaunchRequest` and fix test fixtures Depends on [none]

**READ THESE BEFORE TASK**

- src/crosshook-native/crates/crosshook-core/src/launch/request.rs
- docs/plans/sqlite3-addition/research-integration.md (LaunchRequest gap analysis section)
- docs/plans/sqlite3-addition/shared.md (Design Decisions table)

**Instructions**

Files to Modify

- src/crosshook-native/crates/crosshook-core/src/launch/request.rs
- src/crosshook-native/crates/crosshook-core/src/launch/script_runner.rs (test fixtures only)

Add `pub profile_name: Option<String>` with `#[serde(default)]` to the `LaunchRequest` struct in `request.rs`. Place it after the last existing field (`launch_game_only`) to minimize diff noise. The `Option<String>` type (not bare `String`) avoids sentinel-value checking at every call site and is consistent with `profiles.source_profile_id` being a nullable FK in the Phase 1 schema.

After adding the field, grep for all struct literal constructions of `LaunchRequest` in the codebase. The test fixtures in `script_runner.rs` (approximately lines 353, 406, 498, and any others found) use exhaustive field lists — each needs `profile_name: None` appended. The `Default` derive on `LaunchRequest` already handles this for `..Default::default()` patterns.

Verify with `cargo check --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core`.

### Phase 2: Foundation

#### Task 2.1: Add Phase 2 types to `models.rs` Depends on [none]

**READ THESE BEFORE TASK**

- src/crosshook-native/crates/crosshook-core/src/metadata/models.rs
- docs/plans/sqlite3-addition/shared.md (schema section, Design Decisions table)
- docs/plans/sqlite3-addition/analysis-code.md (Enum with as_str() pattern)

**Instructions**

Files to Modify

- src/crosshook-native/crates/crosshook-core/src/metadata/models.rs

Add the following types after the existing `SyncSource` enum (after line ~93):

1. **`LaunchOutcome` enum** — derives `Debug, Clone, Copy, Serialize, Deserialize` with `#[serde(rename_all = "snake_case")]` and `as_str() -> &'static str`. Variants: `Started` ("started"), `Succeeded` ("succeeded"), `Failed` ("failed"), `Abandoned` ("abandoned"). Maps to the `launch_operations.status` TEXT column.

2. **`DriftState` enum** — same derives. Variants: `Unknown` ("unknown"), `Aligned` ("aligned"), `Missing` ("missing"), `Moved` ("moved"), `Stale` ("stale"). Maps to `launchers.drift_state` TEXT column.

3. **`MAX_DIAGNOSTIC_JSON_BYTES` constant** — `pub const MAX_DIAGNOSTIC_JSON_BYTES: usize = 4_096;` (W3 security requirement).

4. **`LauncherRow` struct** — `#[derive(Debug, Clone)]`, `#[allow(dead_code)]`, `pub(crate)` visibility. Fields: `launcher_id: String`, `profile_id: Option<String>`, `launcher_slug: String`, `display_name: String`, `script_path: String`, `desktop_entry_path: String`, `drift_state: String`, `created_at: String`, `updated_at: String`. All timestamps as `String` (RFC 3339), consistent with `ProfileRow`.

5. **`LaunchOperationRow` struct** — same pattern. Fields: `operation_id: String`, `profile_id: Option<String>`, `profile_name: Option<String>`, `launch_method: String`, `status: String`, `exit_code: Option<i32>`, `signal: Option<i32>`, `log_path: Option<String>`, `diagnostic_json: Option<String>`, `severity: Option<String>`, `failure_mode: Option<String>`, `started_at: String`, `finished_at: Option<String>`.

Update the `pub use models::` line in `mod.rs` to add `LaunchOutcome, DriftState, MAX_DIAGNOSTIC_JSON_BYTES`.

#### Task 2.2: Add `migrate_2_to_3()` to `migrations.rs` Depends on [none]

**READ THESE BEFORE TASK**

- src/crosshook-native/crates/crosshook-core/src/metadata/migrations.rs
- docs/plans/sqlite3-addition/shared.md (schema DDL in "Relevant Tables" section)
- docs/plans/sqlite3-addition/analysis-context.md (Phase 2 Schema section)

**Instructions**

Files to Modify

- src/crosshook-native/crates/crosshook-core/src/metadata/migrations.rs

Add a new migration guard after the existing `if version < 2` block in `run_migrations()`:

```rust
if version < 3 {
    migrate_2_to_3(conn)?;
    conn.pragma_update(None, "user_version", 3_u32)
        .map_err(|source| MetadataStoreError::Database {
            action: "update metadata schema version",
            source,
        })?;
}
```

Add the private `migrate_2_to_3` function using `conn.execute_batch()` with literal-only DDL (W7). The DDL creates two tables:

**`launchers`**: `launcher_id TEXT PRIMARY KEY`, `profile_id TEXT REFERENCES profiles(profile_id)` (nullable), `launcher_slug TEXT NOT NULL UNIQUE`, `display_name TEXT NOT NULL`, `script_path TEXT NOT NULL`, `desktop_entry_path TEXT NOT NULL`, `drift_state TEXT NOT NULL DEFAULT 'unknown'`, `created_at TEXT NOT NULL`, `updated_at TEXT NOT NULL`. Indexes on `profile_id` and `launcher_slug`.

**`launch_operations`**: `operation_id TEXT PRIMARY KEY`, `profile_id TEXT REFERENCES profiles(profile_id)` (nullable), `profile_name TEXT`, `launch_method TEXT NOT NULL`, `status TEXT NOT NULL DEFAULT 'started'`, `exit_code INTEGER`, `signal INTEGER`, `log_path TEXT`, `diagnostic_json TEXT`, `severity TEXT`, `failure_mode TEXT`, `started_at TEXT NOT NULL`, `finished_at TEXT`. Indexes on `profile_id` and `started_at`.

The `launcher_slug` column has a `UNIQUE` constraint — this is the conflict target for the UPSERT in `observe_launcher_exported`. Follow the exact DDL pattern from `migrate_0_to_1` and `migrate_1_to_2`. Use `CREATE TABLE IF NOT EXISTS` and `CREATE INDEX IF NOT EXISTS` for idempotency.

### Phase 3: Core Modules

#### Task 3.1: Create `metadata/launcher_sync.rs` Depends on [2.1, 2.2]

**READ THESE BEFORE TASK**

- src/crosshook-native/crates/crosshook-core/src/metadata/profile_sync.rs (template for free function pattern)
- src/crosshook-native/crates/crosshook-core/src/export/launcher.rs (SteamExternalLauncherExportResult, sanitize_launcher_slug)
- docs/plans/sqlite3-addition/analysis-code.md (UPSERT pattern, structured error mapping)
- docs/plans/sqlite3-addition/research-security.md (W6: re-validate stored paths)

**Instructions**

Files to Create

- src/crosshook-native/crates/crosshook-core/src/metadata/launcher_sync.rs

Create the file following the `profile_sync.rs` free function pattern. All functions take `conn: &Connection` as first arg and return `Result<T, MetadataStoreError>`. Import `DriftState` from `super::models`, `db` from `super::db`, and `profile_sync::lookup_profile_id` from `super::profile_sync`.

Implement:

1. **`observe_launcher_exported(conn, profile_name: Option<&str>, slug: &str, display_name: &str, script_path: &str, desktop_entry_path: &str) -> Result<(), MetadataStoreError>`**
   - If `profile_name` is `Some`, call `lookup_profile_id(conn, name)` to resolve the FK. If not found, proceed with `profile_id = NULL`.
   - UPSERT into `launchers` with `ON CONFLICT(launcher_slug) DO UPDATE`. Set `drift_state = 'aligned'` on both INSERT and UPDATE paths. Use `COALESCE(excluded.profile_id, launchers.profile_id)` to preserve an existing `profile_id` if the new one is NULL.
   - Use `db::new_id()` for the `launcher_id` PK on INSERT. Timestamps via `Utc::now().to_rfc3339()`.
   - All SQL as literal strings with `params![]` — never `format!()`.

2. **`observe_launcher_deleted(conn, launcher_slug: &str) -> Result<(), MetadataStoreError>`**
   - UPDATE `launchers SET drift_state = 'missing', updated_at = ?1 WHERE launcher_slug = ?2`. Do not hard-delete — preserve for history.

3. **`observe_launcher_renamed(conn, old_slug: &str, new_slug: &str, new_display_name: &str, new_script_path: &str, new_desktop_entry_path: &str) -> Result<(), MetadataStoreError>`**
   - Tombstone old row: UPDATE `drift_state = 'missing'` WHERE `launcher_slug = old_slug`.
   - UPSERT new row with new slug (same as `observe_launcher_exported` but uses the new slug). Use `TransactionBehavior::Immediate` for atomicity.

#### Task 3.2: Create `metadata/launch_history.rs` Depends on [2.1, 2.2]

**READ THESE BEFORE TASK**

- src/crosshook-native/crates/crosshook-core/src/metadata/profile_sync.rs (free function pattern)
- src/crosshook-native/crates/crosshook-core/src/launch/diagnostics/models.rs (DiagnosticReport struct)
- docs/plans/sqlite3-addition/analysis-code.md (spawn_blocking pattern, truncation logic)
- docs/plans/sqlite3-addition/research-security.md (W3: 4KB diagnostic limit)

**Instructions**

Files to Create

- src/crosshook-native/crates/crosshook-core/src/metadata/launch_history.rs

Create the file following the `profile_sync.rs` pattern. Import `DiagnosticReport` from `crate::launch::diagnostics::models`, `LaunchOutcome` and `MAX_DIAGNOSTIC_JSON_BYTES` from `super::models`, and `db` from `super::db`.

Implement:

1. **`record_launch_started(conn, profile_name: Option<&str>, method: &str, log_path: Option<&str>) -> Result<String, MetadataStoreError>`**
   - Generate `operation_id` via `db::new_id()`.
   - If `profile_name` is `Some`, call `lookup_profile_id(conn, name)` to resolve FK. Proceed with `NULL` if not found.
   - INSERT into `launch_operations` with `status = 'started'`, `started_at = Utc::now().to_rfc3339()`. Leave `finished_at`, `exit_code`, `signal`, `diagnostic_json`, `severity`, `failure_mode` as NULL.
   - Store `log_path` as-is (caller is responsible for sanitization via `sanitize_display_path`).
   - Return the `operation_id` string.

2. **`record_launch_finished(conn, operation_id: &str, exit_code: Option<i32>, signal: Option<i32>, report: &DiagnosticReport) -> Result<(), MetadataStoreError>`**
   - Serialize report: `let json = serde_json::to_string(report).ok()`. Apply 4KB truncation: if `json.as_ref().map_or(0, |s| s.len()) > MAX_DIAGNOSTIC_JSON_BYTES`, set `json = None`. Still extract promoted columns regardless.
   - Determine outcome: if `report.exit_info.failure_mode` is `CleanExit`, use `LaunchOutcome::Succeeded`; otherwise `LaunchOutcome::Failed`.
   - Extract promoted columns: `severity` from `report.severity` (type `ValidationSeverity` from `crate::launch::request` — serialize via `serde_json::to_value(&report.severity)` and extract the string, or add an `as_str()` method), `failure_mode` from `report.exit_info.failure_mode` (type `FailureMode` — already has `#[serde(rename_all = "snake_case")]`, serialize the same way).
   - UPDATE `launch_operations SET status = ?1, exit_code = ?2, signal = ?3, diagnostic_json = ?4, severity = ?5, failure_mode = ?6, finished_at = ?7 WHERE operation_id = ?8`.
   - If the UPDATE affects 0 rows (unknown `operation_id`), log `tracing::warn!` and return `Ok(())` — do not panic.

3. **`sweep_abandoned_operations(conn) -> Result<usize, MetadataStoreError>`**
   - `UPDATE launch_operations SET status = 'abandoned', finished_at = ?1 WHERE status = 'started' AND finished_at IS NULL`.
   - Use `Utc::now().to_rfc3339()` for `finished_at`. The 24-hour threshold from the feature spec is intentionally NOT enforced in SQL — sweep ALL incomplete operations at startup, since any incomplete row at startup is definitively abandoned (no async tasks survive app restart).
   - Return the number of rows affected via `conn.execute(...)` return value.

### Phase 4: Integration

#### Task 4.1: Add Phase 2 method wrappers to `metadata/mod.rs` Depends on [3.1, 3.2]

**READ THESE BEFORE TASK**

- src/crosshook-native/crates/crosshook-core/src/metadata/mod.rs
- docs/plans/sqlite3-addition/analysis-code.md (with_conn delegation pattern shapes)

**Instructions**

Files to Modify

- src/crosshook-native/crates/crosshook-core/src/metadata/mod.rs

1. Add submodule declarations after `pub mod profile_sync;`:

   ```rust
   mod launcher_sync;
   mod launch_history;
   ```

   Both are private (`mod`, not `pub mod`) — they are accessed only through `MetadataStore` methods.

2. Add imports for `DiagnosticReport` and `SteamExternalLauncherExportResult` at the top.

3. Add these public methods to `impl MetadataStore`, after the existing `sync_profiles_from_store` method. Each delegates through `with_conn` following the exact same shape as `observe_profile_write`:
   - `observe_launcher_exported(&self, profile_name: Option<&str>, slug: &str, display_name: &str, script_path: &str, desktop_entry_path: &str) -> Result<(), MetadataStoreError>` — action: `"observe a launcher export"`
   - `observe_launcher_deleted(&self, launcher_slug: &str) -> Result<(), MetadataStoreError>` — action: `"observe a launcher deletion"`
   - `observe_launcher_renamed(&self, old_slug: &str, new_slug: &str, new_display_name: &str, new_script_path: &str, new_desktop_entry_path: &str) -> Result<(), MetadataStoreError>` — action: `"observe a launcher rename"`
   - `record_launch_started(&self, profile_name: Option<&str>, method: &str, log_path: Option<&str>) -> Result<String, MetadataStoreError>` — action: `"record a launch start"`
   - `record_launch_finished(&self, operation_id: &str, exit_code: Option<i32>, signal: Option<i32>, report: &DiagnosticReport) -> Result<(), MetadataStoreError>` — action: `"record a launch finish"`
   - `sweep_abandoned_operations(&self) -> Result<usize, MetadataStoreError>` — action: `"sweep abandoned operations"`

Note: `record_launch_started` returns `Result<String, ...>` where `String::default()` is `""`. Callers must filter empty strings to `None` before passing to `record_launch_finished`.

#### Task 4.2: Wire launcher sync hooks into `commands/export.rs` Depends on [1.1, 4.1]

**READ THESE BEFORE TASK**

- src/crosshook-native/src-tauri/src/commands/export.rs
- src/crosshook-native/crates/crosshook-core/src/export/launcher.rs (SteamExternalLauncherExportRequest struct)
- docs/plans/sqlite3-addition/research-integration.md (export hook points)
- docs/plans/sqlite3-addition/analysis-code.md (warn-and-continue pattern)

**Instructions**

Files to Modify

- src/crosshook-native/crates/crosshook-core/src/export/launcher.rs
- src/crosshook-native/src-tauri/src/commands/export.rs

First, add `pub profile_name: Option<String>` with `#[serde(default)]` to `SteamExternalLauncherExportRequest` in `export/launcher.rs`. This mirrors the `LaunchRequest` change. Fix any struct literal constructions (check `rename_launcher` in `export.rs` around lines 95-108 — add `profile_name: None`).

Then modify `commands/export.rs`:

1. Add `use crosshook_core::metadata::MetadataStore;` and `use tauri::State;` imports.

2. **`export_launchers`** (line ~20): Add `metadata_store: State<'_, MetadataStore>` parameter. After successful `export_launchers_core(&request)`, add:

   ```rust
   if let Err(e) = metadata_store.observe_launcher_exported(
       request.profile_name.as_deref(),
       &result.launcher_slug,
       &result.display_name,
       &result.script_path,
       &result.desktop_entry_path,
   ) {
       tracing::warn!(%e, launcher_slug = %result.launcher_slug, "metadata sync after export_launchers failed");
   }
   ```

3. **`delete_launcher`** (line ~47): Add `metadata_store: State<'_, MetadataStore>` parameter. This command takes individual string args (`display_name`, `steam_app_id`, `trainer_path`, `target_home_path`, `steam_client_install_path`) — it does NOT receive a `launcher_slug` directly. Derive the slug by calling `sanitize_launcher_slug(&display_name)` (import from `crosshook_core::export::launcher`), then call `metadata_store.observe_launcher_deleted(&slug)` with warn-and-continue.

4. **`delete_launcher_by_slug`** (line ~66): Add `metadata_store: State<'_, MetadataStore>` parameter. The slug is already a parameter — call `metadata_store.observe_launcher_deleted(&launcher_slug)` directly.

5. **`rename_launcher`** (line ~81): Add `metadata_store: State<'_, MetadataStore>` parameter. After successful `rename_launcher_files(...)`, the result is a `LauncherRenameResult` with fields `old_slug`, `new_slug`, `new_script_path`, `new_desktop_entry_path`. Call `metadata_store.observe_launcher_renamed(&result.old_slug, &result.new_slug, &new_display_name, &result.new_script_path, &result.new_desktop_entry_path)` with warn-and-continue.

6. **Update `command_names_match_expected_ipc_contract` test** (lines ~163-188 in `export.rs`): This compile-time test casts each command function to its expected type signature. After adding `State<'_, MetadataStore>` to `export_launchers`, `delete_launcher`, `delete_launcher_by_slug`, and `rename_launcher`, update the corresponding type-cast assertions in this test to include the new parameter. **This test will fail to compile if skipped.**

All export commands are synchronous — no `spawn_blocking` needed.

#### Task 4.3: Wire launch history hooks into `commands/launch.rs` Depends on [4.1]

**READ THESE BEFORE TASK**

- src/crosshook-native/src-tauri/src/commands/launch.rs (full file — understand spawn_log_stream and stream_log_lines flow)
- src/crosshook-native/src-tauri/src/commands/install.rs (spawn_blocking pattern, lines 10-18)
- docs/plans/sqlite3-addition/analysis-code.md (spawn_blocking pattern, data flow for Phase 2)
- docs/plans/sqlite3-addition/analysis-context.md (async bridge section)

**Instructions**

Files to Modify

- src/crosshook-native/src-tauri/src/commands/launch.rs

This is the most complex Phase 2 task. `launch_game` and `launch_trainer` are async commands; `rusqlite::Connection` is `!Send`; all metadata writes must use `tauri::async_runtime::spawn_blocking`.

1. **Add import**: `use crosshook_core::metadata::MetadataStore;`

2. **Modify `spawn_log_stream` signature** to accept two new parameters:

   ```rust
   fn spawn_log_stream(
       app: AppHandle,
       log_path: PathBuf,
       child: tokio::process::Child,
       method: &'static str,
       metadata_store: MetadataStore,    // Arc clone
       operation_id: Option<String>,     // None when store disabled or start failed
   )
   ```

   Thread both into the `stream_log_lines` call inside the spawned task.

3. **Modify `stream_log_lines` signature** to accept `metadata_store: MetadataStore` and `operation_id: Option<String>`.

4. **In `launch_game` and `launch_trainer`**, before `spawn_log_stream`:

   ```rust
   let metadata_store = app.state::<MetadataStore>().inner().clone();
   let pn = request.profile_name.clone();
   let lp = sanitize_display_path(&log_path.to_string_lossy());
   let ms_clone = metadata_store.clone();
   let operation_id: Option<String> = tauri::async_runtime::spawn_blocking(move || {
       ms_clone.record_launch_started(pn.as_deref(), method, Some(&lp))
   })
   .await
   .unwrap_or_else(|e| {
       tracing::warn!("metadata spawn_blocking join failed: {e}");
       Ok(String::new())
   })
   .unwrap_or_else(|e| {
       tracing::warn!(%e, "record_launch_started failed");
       String::new()
   });
   let operation_id = if operation_id.is_empty() { None } else { Some(operation_id) };
   ```

   Pass `metadata_store` and `operation_id` to `spawn_log_stream`.

5. **In `stream_log_lines`**, after `let report = sanitize_diagnostic_report(report);` (around line 211), before the `should_surface_report` check:

   ```rust
   if let Some(ref op_id) = operation_id {
       let ms = metadata_store.clone();
       let op = op_id.clone();
       let ec = exit_code;
       let sig = signal;
       let rpt = report.clone();
       let _ = tauri::async_runtime::spawn_blocking(move || {
           if let Err(e) = ms.record_launch_finished(&op, ec, sig, &rpt) {
               tracing::warn!(%e, operation_id = %op, "record_launch_finished failed");
           }
       }).await;
   }
   ```

6. Both `launch_game` and `launch_trainer` follow the same pattern. Extract the `record_launch_started` block into a private helper if the duplication is excessive, but the two commands have slightly different `method` resolution — keep them separate if simpler.

#### Task 4.4: Add `sweep_abandoned_operations` to startup Depends on [4.1]

**READ THESE BEFORE TASK**

- src/crosshook-native/src-tauri/src/startup.rs
- src/crosshook-native/src-tauri/src/lib.rs (setup closure around line 50)
- docs/plans/sqlite3-addition/analysis-context.md (startup sweep section)

**Instructions**

Files to Modify

- src/crosshook-native/src-tauri/src/startup.rs

Add the sweep call inside `run_metadata_reconciliation`, after the existing `sync_profiles_from_store` call and its log statement:

```rust
match metadata_store.sweep_abandoned_operations() {
    Ok(count) if count > 0 => {
        tracing::info!(swept = count, "startup abandoned operation sweep complete");
    }
    Err(error) => {
        tracing::warn!(%error, "startup abandoned operation sweep failed");
    }
    _ => {}
}
```

This follows the existing log discipline in `run_metadata_reconciliation`: INFO for non-zero results, WARN for failures, silent for zero-work. The sweep runs before the Tauri event loop starts, so no async launch tasks can be in-flight. Non-fatal — the app starts regardless.

### Phase 5: Testing

#### Task 5.1: Add Phase 2 unit and integration tests Depends on [4.2, 4.3, 4.4]

**READ THESE BEFORE TASK**

- src/crosshook-native/crates/crosshook-core/src/metadata/mod.rs (existing test module, sample_profile helper, connection helper)
- docs/plans/sqlite3-addition/analysis-code.md (test patterns section)
- docs/plans/sqlite3-addition/analysis-tasks.md (required test cases)

**Instructions**

Files to Modify

- src/crosshook-native/crates/crosshook-core/src/metadata/mod.rs (add tests in existing `#[cfg(test)] mod tests`)

Add the following tests using `MetadataStore::open_in_memory()` and the existing `connection()` helper:

1. **`test_observe_launcher_exported_creates_row`** — Call `observe_launcher_exported(None, "test-slug", "Test Name", "/path/script.sh", "/path/desktop.desktop")`. Query `launchers` table directly. Verify row exists with correct slug, `drift_state = 'aligned'`, non-empty `launcher_id`.

2. **`test_observe_launcher_exported_idempotent`** — Call twice with same slug. Verify single row (UPSERT not duplicate). Second call should update `updated_at`.

3. **`test_observe_launcher_deleted_tombstones`** — Export then delete by slug. Verify `drift_state = 'missing'`, row not hard-deleted.

4. **`test_record_launch_started_returns_operation_id`** — Call `record_launch_started(Some("test-profile"), "native", None)`. Verify non-empty string returned. Query `launch_operations` — verify row with `status = 'started'`, non-null `started_at`.

5. **`test_record_launch_finished_updates_row`** — Start then finish. Verify `status`, `exit_code`, `diagnostic_json` (or `None` if over 4KB), `severity`, `failure_mode`, non-null `finished_at`.

6. **`test_diagnostic_json_truncated_at_4kb`** — Test BOTH boundary cases: (a) Create a `DiagnosticReport` that serializes to exactly 4096 bytes — verify `diagnostic_json` IS stored (not truncated). (b) Create one that exceeds 4096 bytes — verify `diagnostic_json IS NULL` but `severity` and `failure_mode` are still populated. The nullify approach is correct — do NOT truncate to a partial JSON string (malformed JSON is worse than NULL).

7. **`test_sweep_abandoned_marks_old_operations`** — Insert a `launch_operations` row with `status = 'started'` and `finished_at IS NULL` by calling `record_launch_started`. Then call `sweep_abandoned_operations`. Verify the row's `status` is now `'abandoned'` and `finished_at` is non-null.

8. **`test_record_launch_finished_unknown_op_id_noop`** — Call `record_launch_finished("nonexistent-id", ...)`. Verify `Ok(())` returned, no panic.

9. **`test_observe_launcher_renamed_atomic`** — Export a launcher, then rename. Verify old row has `drift_state = 'missing'`, new row exists with new slug and `drift_state = 'aligned'`. Verify the transaction is atomic (both changes or neither).

10. **`test_phase2_disabled_store_noop`** — Add to the existing `test_unavailable_store_noop` test. Call all Phase 2 methods on `MetadataStore::disabled()`. Verify all return `Ok(...)` — `record_launch_started` returns empty string.

Run all tests with: `cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core`

## Advice

- **`record_launch_started` returns empty string when disabled, not `None`** — The `with_conn` helper returns `Ok(T::default())` when the store is disabled. `String::default()` is `""`. Callers in `commands/launch.rs` must filter: `let operation_id = if operation_id.is_empty() { None } else { Some(operation_id) };`. This is not obvious from reading the `with_conn` signature alone.
- **`launcher_slug` UNIQUE constraint means the UPSERT conflicts on slug, not PK** — Even though `launcher_id` is the PK, the `ON CONFLICT(launcher_slug)` clause is what makes re-export idempotent. If you accidentally write `ON CONFLICT(launcher_id)`, every re-export will create a duplicate row.
- **`DiagnosticReport` truncation must happen in `launch_history.rs`, not the command layer** — This keeps W3 enforcement in one place. The command passes the full report; the metadata layer decides whether to store or truncate. Promoted columns (`severity`, `failure_mode`) are extracted before truncation and always stored.
- **`tauri::async_runtime::spawn_blocking`, not `tokio::task::spawn_blocking`** — Both work (Tauri wraps Tokio), but the codebase convention from `commands/install.rs` uses the Tauri alias. Be consistent.
- **`stream_log_lines` already has `app: AppHandle`** — Access `MetadataStore` via `app.state::<MetadataStore>()` if preferred over threading as a parameter. However, cloning once in the command and passing through is cheaper than calling `.state()` twice (once for start, once for finish). The analysis team chose the clone-and-pass approach.
- **The sweep SQL should NOT use the 24-hour threshold from the feature spec** — At startup, any incomplete operation is definitively abandoned because no child process survives app restart. The 24-hour rule was for potential mid-session cleanup which is not implemented in Phase 2. Sweep all `status = 'started'` rows unconditionally.
- **`profile_name` field is stored as TEXT in `launch_operations` even when `profile_id` is also stored** — This is intentional redundancy. If a profile is later deleted (soft-deleted in Phase 1), the `profile_name` column preserves the display name for history queries without needing to join through a tombstoned `profiles` row.
- **`SteamExternalLauncherExportRequest` also needs `profile_name`** — This is a separate struct from `LaunchRequest` in a different file (`export/launcher.rs`). Task 4.2 handles this, not Task 1.1. The two gaps are in different module trees and can be resolved independently.
- **P2-T4 (`launcher_sync.rs`) has no dependency on P2-T1** — It doesn't need `LaunchRequest.profile_name`. It can start as soon as models and migrations are done, even if the `LaunchRequest` change is still in review.
- **`validate_name()` is NOT called in Phase 2 functions** — Phase 1 validates profile names in `profile_sync.rs`. Phase 2 functions receive `profile_name` as an opaque `Option<&str>` and pass it to `lookup_profile_id`, which already validates. Do not add redundant validation.
- **Nullify oversized diagnostic JSON, do NOT truncate to partial JSON** — `analysis-code.md` shows a truncation example (`&s[..4096]`) that produces structurally broken JSON. The correct approach (used in `launch_history.rs`) is: if serialized size > `MAX_DIAGNOSTIC_JSON_BYTES`, store `diagnostic_json = NULL` and rely on the promoted scalar columns (`severity`, `failure_mode`, `exit_code`, `signal`). Malformed partial JSON is worse than NULL for any consumer.
- **`delete_launcher` command has no `launcher_slug` parameter** — Unlike `delete_launcher_by_slug`, the `delete_launcher` command takes individual args. Derive the slug via `sanitize_launcher_slug(&display_name)` imported from `crosshook_core::export::launcher`. This is the same derivation path the export system uses internally.
- **`command_names_match_expected_ipc_contract` test in `export.rs` will fail** — After adding `State<'_, MetadataStore>` to export commands, the type-cast assertions in this compile-time test (lines ~163-188) must be updated. This is easy to miss and will block compilation.
