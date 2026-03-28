# SQLite Metadata Layer — Phase 2: Operational History

Phase 1 established the `MetadataStore` with `Arc<Mutex<Connection>>`, profile sync hooks, and the `with_conn` fail-soft wrapper across five files in `crates/crosshook-core/src/metadata/`. Phase 2 extends this foundation with two new tables (`launchers` and `launch_operations`), three new MetadataStore methods, and integration hooks in the async launch commands and synchronous export commands. The critical blocker is that `LaunchRequest` has no `profile_name` field — this must be added (as `Option<String>` with `#[serde(default)]`) before any Phase 2 hook can link a launch operation to a profile identity. All new methods follow the existing `with_conn` delegation pattern, new enums follow the `SyncSource` derive+`as_str()` pattern, and new SQL uses parameterized queries exclusively.

## Relevant Files

- src/crosshook-native/crates/crosshook-core/src/metadata/mod.rs: MetadataStore struct, `with_conn` fail-soft helper, public API delegates — all Phase 2 methods added here following the same delegation shape
- src/crosshook-native/crates/crosshook-core/src/metadata/db.rs: Connection factory, `new_id()` for UUID v4 generation — Phase 2 uses `new_id()` for `launcher_id` and `operation_id` PKs
- src/crosshook-native/crates/crosshook-core/src/metadata/migrations.rs: Sequential migration runner (currently v0→v1→v2) — Phase 2 adds `migrate_2_to_3()` for `launchers` and `launch_operations` DDL
- src/crosshook-native/crates/crosshook-core/src/metadata/models.rs: MetadataStoreError, SyncSource, SyncReport, ProfileRow — Phase 2 adds LaunchOutcome, DriftState enums and LauncherRow, LaunchOperationRow structs
- src/crosshook-native/crates/crosshook-core/src/metadata/profile_sync.rs: Profile lifecycle reconciliation — template for Phase 2 free functions (conn: &Connection first arg, structured error mapping)
- src/crosshook-native/crates/crosshook-core/src/launch/request.rs: LaunchRequest struct (lines 16-37) — must add `profile_name: Option<String>` with `#[serde(default)]` (Phase 2 blocker)
- src/crosshook-native/crates/crosshook-core/src/launch/diagnostics/models.rs: DiagnosticReport, ExitCodeInfo, FailureMode — serialized to `diagnostic_json` column (4KB max)
- src/crosshook-native/crates/crosshook-core/src/launch/diagnostics/mod.rs: `analyze()` entry point producing DiagnosticReport — called in `stream_log_lines` before Phase 2 hook
- src/crosshook-native/crates/crosshook-core/src/export/launcher_store.rs: LauncherInfo, LauncherDeleteResult, LauncherRenameResult, `derive_launcher_paths()` — Phase 2 `launchers` table maps to these types
- src/crosshook-native/crates/crosshook-core/src/export/launcher.rs: `sanitize_launcher_slug()` (line 265), `SteamExternalLauncherExportRequest` (lines 14-26, missing `profile_name`), `SteamExternalLauncherExportResult`
- src/crosshook-native/src-tauri/src/commands/launch.rs: Async `launch_game`/`launch_trainer` commands, `spawn_log_stream`/`stream_log_lines` — Phase 2 hooks `record_launch_started` before spawn and `record_launch_finished` after analyze
- src/crosshook-native/src-tauri/src/commands/export.rs: Synchronous export commands — Phase 2 adds `State<MetadataStore>` and `observe_launcher_exported` after successful export
- src/crosshook-native/src-tauri/src/commands/profile.rs: Existing warn-and-continue pattern for metadata sync — template for Phase 2 command hooks
- src/crosshook-native/src-tauri/src/commands/shared.rs: `sanitize_display_path()` — apply to `log_path` before storing in `launch_operations`
- src/crosshook-native/src-tauri/src/commands/install.rs: `spawn_blocking` canonical pattern (lines 10-18) — template for async metadata calls in launch commands
- src/crosshook-native/src-tauri/src/lib.rs: MetadataStore initialization with fail-soft fallback, `.manage()` registration, `.setup()` closure — calls `run_metadata_reconciliation` which includes the sweep
- src/crosshook-native/src-tauri/src/startup.rs: `run_metadata_reconciliation()`, StartupError — Phase 2 adds `sweep_abandoned_operations()` call
- src/crosshook-native/crates/crosshook-core/src/launch/script_runner.rs: Command builders — test fixtures (struct literals) need `profile_name: None` added

## Relevant Tables

- profiles: Stable UUID identity (Phase 1) — `profile_id` is the FK target for both Phase 2 tables; `lookup_profile_id()` resolves name→id
- profile_name_history: Append-only rename events (Phase 1) — unmodified by Phase 2
- launchers (Phase 2 NEW): `launcher_id TEXT PK`, `profile_id TEXT FK NULLABLE`, `launcher_slug TEXT NOT NULL`, `display_name TEXT NOT NULL`, `script_path TEXT NOT NULL`, `desktop_entry_path TEXT NOT NULL`, `drift_state TEXT NOT NULL DEFAULT 'unknown'`, `created_at TEXT NOT NULL`, `updated_at TEXT NOT NULL`; indexes on `profile_id` and `launcher_slug`
- launch_operations (Phase 2 NEW): `operation_id TEXT PK`, `profile_id TEXT FK NULLABLE`, `profile_name TEXT`, `launch_method TEXT NOT NULL`, `status TEXT NOT NULL DEFAULT 'started'`, `exit_code INTEGER`, `signal INTEGER`, `log_path TEXT`, `diagnostic_json TEXT` (max 4KB), `severity TEXT`, `failure_mode TEXT`, `started_at TEXT NOT NULL`, `finished_at TEXT`; indexes on `profile_id` and `started_at`

## Relevant Patterns

**`with_conn` Fail-Soft Delegation**: Every public MetadataStore method delegates through `with_conn(action, |conn| ...)` which no-ops when disabled (`T::default()`) and locks the mutex when available. Phase 2 methods replicate this exact shape. See [src/crosshook-native/crates/crosshook-core/src/metadata/mod.rs](src/crosshook-native/crates/crosshook-core/src/metadata/mod.rs) lines 56-73.

**Free Function + Module Delegation**: Sync functions in submodules take `conn: &Connection` as first arg and return `Result<T, MetadataStoreError>`. The `mod.rs` method wraps them via `with_conn`. Phase 2 adds `launcher_sync.rs` and `launch_history.rs` following this pattern. See [src/crosshook-native/crates/crosshook-core/src/metadata/profile_sync.rs](src/crosshook-native/crates/crosshook-core/src/metadata/profile_sync.rs).

**Structured Error Mapping**: SQL errors are mapped with `MetadataStoreError::Database { action: "lowercase gerund phrase", source }` where `action` finishes the sentence "failed to \_\_\_". Never use `format!()` for action strings. See [src/crosshook-native/crates/crosshook-core/src/metadata/profile_sync.rs](src/crosshook-native/crates/crosshook-core/src/metadata/profile_sync.rs) lines 25-30.

**Enum with `as_str()`**: Metadata enums derive `Debug + Clone + Copy + Serialize + Deserialize` with `#[serde(rename_all = "snake_case")]` and expose `as_str() -> &'static str` for SQL storage. Phase 2 `LaunchOutcome` and `DriftState` follow this. See [src/crosshook-native/crates/crosshook-core/src/metadata/models.rs](src/crosshook-native/crates/crosshook-core/src/metadata/models.rs) lines 69-93.

**Warn-and-Continue**: Tauri commands call metadata hooks in `if let Err(e) { tracing::warn!(...) }` blocks — metadata failures never block the primary operation. See [src/crosshook-native/src-tauri/src/commands/profile.rs](src/crosshook-native/src-tauri/src/commands/profile.rs) lines 106-113.

**`spawn_blocking` Async Bridge**: `rusqlite::Connection` is `!Send`; async Tauri commands must use `tauri::async_runtime::spawn_blocking` for metadata writes, cloning the `MetadataStore` (cheap `Arc` clone) into the closure. See [src/crosshook-native/src-tauri/src/commands/install.rs](src/crosshook-native/src-tauri/src/commands/install.rs) lines 10-18.

**UPSERT Reconciliation**: `INSERT ... ON CONFLICT DO UPDATE` for idempotent sync. Used in `observe_profile_write` for profile census; Phase 2 uses same pattern for `observe_launcher_exported`. See [src/crosshook-native/crates/crosshook-core/src/metadata/profile_sync.rs](src/crosshook-native/crates/crosshook-core/src/metadata/profile_sync.rs) lines 17-50.

**Sequential Migration Runner**: `if version < N { migrate(conn)?; pragma_update(N)?; }` guards with `migrate_N_to_M()` private functions using `conn.execute_batch()` for literal-only DDL. See [src/crosshook-native/crates/crosshook-core/src/metadata/migrations.rs](src/crosshook-native/crates/crosshook-core/src/metadata/migrations.rs).

## Relevant Docs

**docs/plans/sqlite3-addition/feature-spec.md**: You _must_ read this when working on any Phase 2 task. Phase 2 schema (lines 189-222), API design (lines 246-249), business rules 10-13, edge cases, success criteria. Authority matrix and security findings apply to Phase 2.

**docs/plans/sqlite3-addition/research-architecture.md**: You _must_ read this when wiring launch or export commands. Current metadata module structure, launch system flow, async bridge requirements, Tauri command integration points with exact line numbers.

**docs/plans/sqlite3-addition/research-patterns.md**: You _must_ read this when creating new metadata module files. Phase 1 patterns extracted from source: `with_conn`, `profile_sync` function shape, models pattern, migration pattern, testing patterns — all with code examples.

**docs/plans/sqlite3-addition/research-integration.md**: You _must_ read this when modifying launch or export commands. All Tauri command signatures, LaunchRequest gap analysis, DiagnosticReport size estimation, hook point locations, startup sweep placement.

**docs/plans/sqlite3-addition/research-docs.md**: You _must_ read this for Phase 2 business rules, edge cases, security findings (W2/W3/W6), documentation gaps, and must-read document priority list.

**docs/plans/sqlite3-addition/research-security.md**: You _must_ read this when implementing connection setup, path handling, or diagnostic storage. W3 (4KB payload bound), W6 (re-validate stored paths), W2 (path sanitization), W7 (no format! in SQL).

**CLAUDE.md**: You _must_ read this for project conventions — commit messages, build commands, Rust style, test commands, label taxonomy.

## Design Decisions (Locked)

| Decision                                          | Choice                                               | Rationale                                                                                    |
| ------------------------------------------------- | ---------------------------------------------------- | -------------------------------------------------------------------------------------------- |
| `launch_operations` PK                            | UUID TEXT via `db::new_id()`                         | Consistent with Phase 1 `profiles` PK pattern; avoids being the only AUTOINCREMENT in schema |
| `launchers` PK                                    | UUID TEXT (`launcher_id`) + index on `launcher_slug` | Nullable `profile_id` in composite PK creates SQLite ambiguity; UUID PK is clean             |
| `profile_name` type                               | `Option<String>` with `#[serde(default)]`            | Avoids sentinel-value checking; consistent with nullable FK pattern in Phase 1               |
| `SteamExternalLauncherExportRequest.profile_name` | `Option<String>` with `#[serde(default)]`            | Same reasoning; backwards compatible with existing frontend callers                          |
| Startup sweep threshold                           | Rows with `status = 'started'` and no `finished_at`  | Run in `.setup()` closure after reconciliation; non-fatal warn-only                          |
| DiagnosticReport truncation                       | Truncate `diagnostic_json` before INSERT when > 4KB  | Still record outcome, exit_code, severity, failure_mode in promoted columns                  |
