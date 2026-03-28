# Documentation Research: SQLite Metadata Layer Phase 2 (Operational History)

## Overview

Phase 1 is fully implemented: `MetadataStore` with `Arc<Mutex<Connection>>`, `db.rs` connection factory, `migrations.rs` (schema v2: `profiles` + `profile_name_history`), `models.rs` (errors, enums, row types), and `profile_sync.rs` (observe write/rename/delete, census sync). Phase 2 adds the `launchers` and `launch_operations` tables, three new API methods (`record_launch_started`, `record_launch_finished`, `observe_launcher_exported`), startup abandoned-operation sweep, and drift detection. The critical blocker is that `LaunchRequest` has no `profile_name` field (confirmed absent at `launch/request.rs:16-37`).

---

## Feature Spec Phase 2 Requirements

### Schema Requirements

**`launchers` table** (composite PK: `profile_id` + `launcher_slug`):

| Column               | Type | Constraints                    | Notes                                           |
| -------------------- | ---- | ------------------------------ | ----------------------------------------------- |
| `profile_id`         | TEXT | FK → `profiles.profile_id`     | Owning profile                                  |
| `launcher_slug`      | TEXT | NOT NULL                       | From `sanitize_launcher_slug()`                 |
| `display_name`       | TEXT | NOT NULL                       | Latest expected launcher title                  |
| `script_path`        | TEXT | NULL                           | Expected `.sh` path                             |
| `desktop_entry_path` | TEXT | NULL                           | Expected `.desktop` path                        |
| `drift_state`        | TEXT | NOT NULL DEFAULT `'unknown'`   | `aligned`, `missing`, `moved`, `stale`, `unknown` |
| `created_at`         | TEXT | NOT NULL                       | First export timestamp (RFC 3339)               |
| `updated_at`         | TEXT | NOT NULL                       | Last observation refresh                        |

On rename: old `(profile_id, slug)` row is tombstoned; new row created on next re-export. No in-place slug rename (Business Rule RF-2).

**`launch_operations` table** (PK: `id AUTOINCREMENT`):

| Column            | Type    | Constraints                 | Notes                                              |
| ----------------- | ------- | --------------------------- | -------------------------------------------------- |
| `id`              | INTEGER | PK AUTOINCREMENT            |                                                    |
| `profile_id`      | TEXT    | FK → `profiles.profile_id`  |                                                    |
| `method`          | TEXT    | NOT NULL                    | `steam_applaunch`, `proton_run`, `native`          |
| `game_path`       | TEXT    | NULL                        |                                                    |
| `trainer_path`    | TEXT    | NULL                        |                                                    |
| `started_at`      | TEXT    | NOT NULL                    | RFC 3339                                           |
| `ended_at`        | TEXT    | NULL                        |                                                    |
| `outcome`         | TEXT    | NOT NULL DEFAULT `'incomplete'` | `incomplete`, `succeeded`, `failed`, `abandoned` |
| `exit_code`       | INTEGER | NULL                        |                                                    |
| `signal`          | INTEGER | NULL                        |                                                    |
| `log_path`        | TEXT    | NULL                        | Sanitized display path (W2: never raw home dir)    |
| `diagnostic_json` | TEXT    | NULL                        | Serialized `DiagnosticReport` — **max 4 KB** (W3)  |
| `severity`        | TEXT    | NULL                        | Promoted from `DiagnosticReport.severity` for efficient query |
| `failure_mode`    | TEXT    | NULL                        | Promoted from `DiagnosticReport.exit_info.failure_mode` |

Migration target: schema version 3 (`PRAGMA user_version = 3`), following the existing pattern in `migrations.rs`.

### API Requirements

These three methods are added to `MetadataStore` in `mod.rs`, delegating to new files:

```rust
// Returns operation_id (stringified row id) for use in record_launch_finished
pub fn record_launch_started(
    &self,
    profile_name: &str,
    method: &str,
    game_path: Option<&str>,
    trainer_path: Option<&str>,
) -> Result<String, MetadataStoreError>

// outcome: LaunchOutcome enum (Incomplete, Succeeded, Failed, Abandoned)
pub fn record_launch_finished(
    &self,
    operation_id: &str,
    outcome: LaunchOutcome,
    exit_code: Option<i32>,
    signal: Option<i32>,
    report: Option<&DiagnosticReport>,
) -> Result<(), MetadataStoreError>

// Called from export.rs after export_launchers succeeds
pub fn observe_launcher_exported(
    &self,
    profile_name: &str,
    slug: &str,
    script_path: &str,
    desktop_path: &str,
) -> Result<(), MetadataStoreError>
```

**All methods use `self.with_conn(...)` — the same fail-soft wrapper already in `mod.rs`.**

### Business Rules

1. **Launcher PK is composite**: `(profile_id, launcher_slug)`. Slug change on rename tombstones the old row; new row is created on next explicit re-export.
2. **Launch operation lifecycle**: Row starts with `outcome = 'incomplete'`; updated to `succeeded`/`failed`/`abandoned` on terminal event.
3. **Abandoned sweep**: Startup scans for `outcome = 'incomplete'` rows where `started_at < now - 24h`; marks them `abandoned`. Runs after `sync_profiles_from_store` in `startup.rs`.
4. **4 KB diagnostic limit** (W3): `diagnostic_json` must be truncated or rejected if the serialized `DiagnosticReport` exceeds 4 096 bytes before INSERT.
5. **No raw CLI arguments**: `launch_operations` must never store raw command-line argument lists — only structured fields (`method`, `game_path`, `trainer_path`, `exit_code`, `signal`, `failure_mode`). Proton/Steam launch args may contain tokens.
6. **Path sanitization** (W2): `log_path` stored in `launch_operations` must use the display-sanitized form (`~`-normalized), not the absolute path. Same for `script_path` / `desktop_entry_path` in `launchers`.
7. **Re-validate stored paths** (W6): Before any filesystem operation using a path from `launchers` (e.g., drift check), re-apply path-safety validation. Never assume stored data is safe.
8. **Drift detection**: `drift_state` column tracks `aligned`/`missing`/`moved`/`stale`/`unknown`. Updated by `observe_launcher_scan` when `check_launcher_for_profile` runs. Warning-only for v1; no silent auto-repair.
9. **Watermark rule**: Do not record a launcher as "owned" if watermark verification would fail. Only `steam_applaunch` and `proton_run` profiles export launchers; `native` method is excluded.
10. **Launcher slug source**: Always derived via `sanitize_launcher_slug()` from `export/launcher.rs`. Never compute slug independently in the metadata layer.
11. **`LaunchRequest` gap must be resolved first**: `profile_name` must be threaded through before `record_launch_started` can link an operation to a profile ID. Either add the field to `LaunchRequest` (`launch/request.rs:16-37`) or pass it as a separate parameter from the Tauri command.

### Edge Cases

| Scenario | Expected Behavior |
| --- | --- |
| Force-kill during launch (Steam Deck power button) | `launch_operation` row left as `outcome = 'incomplete'`; startup sweep marks stale rows as `abandoned` after 24h |
| Launcher files renamed/moved outside CrossHook | `drift_state` updated to `moved`/`missing` on next observation; surface repair action in UI — never auto-repair |
| `record_launch_finished` called for unknown `operation_id` | Log warning, no-op — do not panic |
| `DiagnosticReport` serializes to > 4 KB | Truncate or omit `diagnostic_json`, still record outcome + exit_code + severity |
| `profile_name` not found in metadata | `record_launch_started` logs warning, returns `MetadataStoreError::Database`; launch itself is not affected |
| `observe_launcher_exported` for already-tracked slug | UPSERT — update `display_name`, `script_path`, `desktop_entry_path`, `drift_state = 'aligned'`, `updated_at` |
| Launch of `native` method profile | `method = 'native'` row in `launch_operations`; no `launchers` row (native has no launcher export) |

### Success Criteria (Phase 2)

- [ ] Launcher mappings persist separately from launcher slugs and can detect external drift.
- [ ] Launch operations, outcomes, timestamps, and diagnostic summaries are queryable locally.
- [ ] Force-killed operations are recovered via startup sweep (not left permanently as `incomplete`).
- [ ] Diagnostic JSON stored per operation, bounded at 4 KB.
- [ ] `observe_launcher_exported` wired into `commands/export.rs` after `export_launchers` succeeds.
- [ ] `record_launch_started` / `record_launch_finished` wired into `commands/launch.rs`.
- [ ] Phase 2 tests pass with `cargo test -p crosshook-core`.

---

## Existing Research Artifacts (Phase 2-Relevant Extracts)

### Security Findings for Phase 2

From `docs/plans/sqlite3-addition/research-security.md`:

- **W3** (must address): `launch_operations.diagnostic_json` max **4 KB**; `external_cache_entries.payload_json` max 512 KB. Enforce before INSERT — reject or truncate. Define as constants in `models.rs`: `pub const MAX_DIAGNOSTIC_JSON_BYTES: usize = 4_096;`
- **W6** (must address): Stored paths in `launchers.script_path` and `launchers.desktop_entry_path` are used in filesystem drift checks. Re-apply path-safety validation before any `fs::` call. Never assume stored values are safe — DB could be corrupted.
- **W2** (must address): New SQLite-backed Tauri commands must sanitize all paths before IPC boundary. The `sanitize_display_path()` promotion (already Phase 1 T0 work) must be complete before Phase 2 launch/export commands are modified. Confirmed present in `src-tauri/src/commands/shared.rs` via `use super::shared::sanitize_display_path` in `launch.rs:20`.
- **A5** (advisory): DB integrity check at startup — already implemented in `db.rs` via `PRAGMA quick_check`.

### Technical Specifications for Phase 2

From `docs/plans/sqlite3-addition/research-technical.md`:

**Type-to-table mapping for Phase 2:**

| Rust Type | Source File | Phase 2 Table |
| --- | --- | --- |
| `LauncherInfo` | `export/launcher_store.rs` | `launchers` (identity + drift state) |
| `LauncherDeleteResult` | `export/launcher_store.rs` | triggers `drift_state` update |
| `LauncherRenameResult` | `export/launcher_store.rs` | tombstones old slug row |
| `LaunchRequest` | `launch/request.rs` | `launch_operations` (method, game_path, trainer_path) |
| `LaunchResult` | `src-tauri/src/commands/launch.rs` | `launch_operations` (log_path, succeeded) |
| `DiagnosticReport` | `launch/diagnostics/models.rs` | `launch_operations.diagnostic_json` + promoted fields |
| `ExitCodeInfo` | `launch/diagnostics/models.rs` | `launch_operations.exit_code`, `signal`, `failure_mode` |
| `FailureMode` | `launch/diagnostics/models.rs` | `launch_operations.failure_mode` TEXT column |

**Integration points:**

- `launch_game` / `launch_trainer` → `record_launch_started()` before `command.spawn()`, `record_launch_finished()` at end of `stream_log_lines()` (after `analyze()` runs and the `launch-complete` event fires)
- `export_launchers` → `observe_launcher_exported()` after successful write
- `check_launcher_for_profile` → `observe_launcher_scan()` to update `drift_state`
- `delete_launcher` / `delete_launcher_for_profile` → mark launcher row `drift_state = 'missing'` or remove

**`spawn_blocking` pattern** (no existing example in codebase — new for Phase 2):

```rust
// In async Tauri command (launch_game / launch_trainer):
let metadata = state.metadata_store.clone();
let profile_name_owned = profile_name.to_string();
let method_owned = method.to_string();
let operation_id = tokio::task::spawn_blocking(move || {
    metadata.record_launch_started(&profile_name_owned, &method_owned, ...)
})
.await
.map_err(|e| format!("metadata spawn_blocking failed: {e}"))??;
```

### Business Workflows for Phase 2

From `docs/plans/sqlite3-addition/research-business.md`:

**Launcher Export workflow:**
1. User clicks "Export Launcher" in UI.
2. `export_launchers` writes `.sh` and `.desktop` files.
3. SQLite: upsert `launchers` row with `profile_id`, slug, display_name, paths, `drift_state = 'aligned'`, timestamp.

**Drift Detection workflow:**
1. CrossHook checks launcher via `check_launcher_for_profile`.
2. Staleness detected by comparing `Name=` in `.desktop` vs expected display name, or full `.sh` content vs rebuilt.
3. SQLite: compare current disk state vs stored expected paths → emit `drift_state` update if mismatch → surface repair action.

**Launch Execution workflow:**
1. `launch_game` / `launch_trainer` spawns child, streams logs.
2. On process exit, `analyze()` produces `DiagnosticReport`.
3. SQLite: create `launch_operation` at start → update with exit_code, signal, `DiagnosticReport` on completion.

**State transitions:**

- `launch_operation`: `incomplete` → `succeeded` | `failed` | `abandoned`
- `launcher`: `unknown` → `aligned` (on export) → `missing`/`moved`/`stale` (on drift observation) → `aligned` (on re-export)

### Reusable Code for Phase 2

From `docs/plans/sqlite3-addition/research-practices.md`:

| Pattern | Location | How to Reuse |
| --- | --- | --- |
| `with_conn()` method | `metadata/mod.rs:56-73` | All Phase 2 methods delegate through this — fail-soft, mutex-safe |
| `db::new_id()` | `metadata/db.rs:64-66` | Not needed for `launch_operations` (autoincrement PK) — used for launcher UUID if added |
| `SyncSource` enum | `metadata/models.rs:71-93` | Add `LaunchRuntime` variant for launch event tagging |
| `params![]` pattern | `metadata/profile_sync.rs` | All Phase 2 SQL must use parameterized queries — never `format!()` |
| `TransactionBehavior::Immediate` | `profile_sync.rs:4` | Use for all Phase 2 write transactions (prevents SQLITE_BUSY upgrade races) |
| `sanitize_display_path()` | `src-tauri/src/commands/shared.rs` (promoted in Phase 1) | Apply to `log_path` before storing in `launch_operations` |
| `serde_json::to_string()` | existing dependency | Serialize `DiagnosticReport` to `diagnostic_json` column |
| `chrono::Utc::now().to_rfc3339()` | existing dependency | All RFC 3339 timestamps in Phase 2 rows |
| `validate_name()` | `profile/toml_store.rs:300-325` | Use before any profile name is used as SQL param in Phase 2 |

**New utility needed for Phase 2:**
- `validate_stored_path(path: &Path) -> Result<(), MetadataStoreError>` — must be absolute, no `..` components, within expected directory prefix. Apply before any `fs::` call using a path from `launchers` table (W6).

**Anti-patterns to avoid:**
- Do not create a `LaunchHistoryStore` type — all Phase 2 logic lives in `launch_history.rs` called from `mod.rs::with_conn()`
- Do not share connection state between `launcher_sync.rs` and `launch_history.rs` — both receive `&Connection` from `with_conn()`
- Do not store raw CLI arguments in any `launch_operations` column (security requirement)

### Recommendations for Phase 2

From `docs/plans/sqlite3-addition/research-recommendations.md`:

**Phase 2 task sequence:**

1. Resolve `LaunchRequest.profile_name` gap — this is the only blocking prerequisite outside the metadata module.
2. Add `launchers` + `launch_operations` DDL in a new migration (schema version 3) in `migrations.rs`.
3. Create `launcher_sync.rs` — `observe_launcher_exported()`, `observe_launcher_scan()`, `observe_launcher_delete()`.
4. Create `launch_history.rs` — `record_launch_started()`, `record_launch_finished()`, `sweep_abandoned_operations()`.
5. Expose Phase 2 methods on `MetadataStore` via `mod.rs::with_conn()`.
6. Wire into Tauri: `commands/export.rs` for launcher hooks, `commands/launch.rs` for operation recording.
7. Add `sweep_abandoned_operations()` call to startup reconciliation in `startup.rs`.
8. Add Phase 2 unit tests using `open_in_memory()`.

**Key decision confirmed** (RF-2): Slug change on rename tombstones old row — no in-place rename. Old `(profile_id, slug)` → `drift_state = 'missing'`; new row created on next explicit export.

**First-run**: Existing launchers are NOT retroactively mapped. They become tracked only after explicit re-export.

---

## Project Conventions (CLAUDE.md)

### Rust Conventions

- `snake_case` for functions, variables, modules
- `Result<T, E>` with custom error enums — never `unwrap()` in non-test code
- All IPC-crossing types: `#[derive(Serialize, Deserialize)]`
- Error handling: throw early; no silent fallbacks; `MetadataStoreError` must not expose raw `rusqlite::Error` across IPC
- `spawn_blocking` for all synchronous `rusqlite` calls in async Tauri commands

### Commit Messages

- Conventional commits: `feat(metadata): ...`, `fix(metadata): ...`
- `docs(internal): ...` for planning/research (skipped in changelog)
- Titles appear in CHANGELOG via `git-cliff` — write as release notes

### Testing

- Run: `cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core`
- All unit tests: `MetadataStore::open_in_memory()` — no filesystem, fast
- Integration tests: `MetadataStore::with_path(tempdir)` — file permissions, symlink rejection
- No test framework for frontend; all Phase 2 tests are Rust

### Build Commands

- `./scripts/dev-native.sh` — development with hot reload
- `./scripts/build-native.sh --binary-only` — quick binary check (use in CI-like verification)
- `./scripts/build-native.sh` — full AppImage build

---

## Inline Code Documentation

### Launch System Docs

**`/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/launch/request.rs`**

- `LaunchRequest` struct at lines 16-37: `method`, `game_path`, `trainer_path`, `trainer_host_path`, `trainer_loading_mode`, `steam`, `runtime`, `optimizations`, `launch_trainer_only`, `launch_game_only`
- **`profile_name` is absent** — must be added as `pub profile_name: String` with `#[serde(default)]` before Phase 2 launch history can link operations to profiles
- `resolved_method()` method computes the effective method string from request fields
- `METHOD_STEAM_APPLAUNCH`, `METHOD_PROTON_RUN`, `METHOD_NATIVE` constants at lines 11-13

**`/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src-tauri/src/commands/launch.rs`**

- `LaunchResult` struct at lines 23-27: `succeeded: bool`, `message: String`, `helper_log_path: String`
- `launch_game` at line 48 — async, spawns child, calls `spawn_log_stream`
- `launch_trainer` at line 86 — async, same pattern
- `stream_log_lines` at line 142 — where `analyze()` runs and `launch-complete` / `launch-diagnostic` events are emitted (lines 203-229). **Phase 2 `record_launch_finished()` call goes here**, after `exit_code`/`signal` are captured and `report` is built.
- `sanitize_display_path` already imported from `shared.rs` at line 20 (Phase 1 T0 complete)

**`/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/launch/diagnostics/models.rs`**

- `DiagnosticReport` struct: `severity`, `summary`, `exit_info: ExitCodeInfo`, `pattern_matches: Vec<PatternMatch>`, `suggestions`, `launch_method`, `log_tail_path`, `analyzed_at`
- `ExitCodeInfo`: `code: Option<i32>`, `signal: Option<i32>`, `signal_name`, `core_dumped`, `failure_mode: FailureMode`, `description`, `severity`
- `FailureMode` enum: `CleanExit`, `NonZeroExit`, `Segfault`, `Abort`, `Kill`, `BusError`, `IllegalInstruction`, `FloatingPointException`, `BrokenPipe`, `Terminated`, `CommandNotFound`, `PermissionDenied`, `UnknownSignal`, `Indeterminate`, `Unknown`
- `MAX_LOG_TAIL_BYTES: u64 = 2 * 1024 * 1024` (2 MB log tail cap)
- All types derive `Serialize` / `Deserialize` — use `serde_json::to_string(&report)` for `diagnostic_json` column

### Export System Docs

**`/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/export/launcher_store.rs`**

- `LauncherInfo` struct at lines 27-43: `display_name`, `launcher_slug`, `script_path`, `desktop_entry_path`, `script_exists`, `desktop_entry_exists`, `is_stale`
  - Note: `is_stale` is only meaningful when derived via `check_launcher_exists`/`check_launcher_for_profile` — not from `list_launchers()`
- `LauncherDeleteResult` at lines 45-59: `script_deleted`, `desktop_entry_deleted`, `script_path`, `desktop_entry_path`, `script_skipped_reason`, `desktop_entry_skipped_reason`
- `LauncherRenameResult` at lines 61-79: `old_slug`, `new_slug`, `new_script_path`, `new_desktop_entry_path`, `script_renamed`, `desktop_entry_renamed`, `old_*/new_*_cleanup_warning`
- `derive_launcher_paths()` at lines 115-138: computes `(resolved_name, slug, script_path, desktop_entry_path)` from display_name, steam_app_id, trainer_path, target_home_path, steam_client_install_path
- `SCRIPT_WATERMARK = "# Generated by CrossHook"` and `DESKTOP_ENTRY_WATERMARK = "Generated by CrossHook"` at lines 17-20 — used for ownership verification

**`/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src-tauri/src/commands/export.rs`**

- `export_launchers` at line 20: no `MetadataStore` wiring yet — Phase 2 hook goes here
- `check_launcher_for_profile` at line 36: no `MetadataStore` wiring yet — Phase 2 `observe_launcher_scan` goes here
- `delete_launcher` at line 47: no `MetadataStore` wiring yet — Phase 2 drift state update goes here


### Startup Docs

**`/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src-tauri/src/startup.rs`**

- `StartupError` enum at lines 7-11: already has `Metadata(MetadataStoreError)` variant + `From<MetadataStoreError>` impl — no changes needed for Phase 2 error plumbing
- `run_metadata_reconciliation(metadata_store, profile_store)` at lines 43-56: calls `sync_profiles_from_store` and logs created/updated counts — **Phase 2 adds `sweep_abandoned_operations()` as a second call inside this function**, after the existing census sync
- `resolve_auto_load_profile_name()` at line 58: unrelated to metadata Phase 2 — untouched

### Metadata Module Docs

**`/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/metadata/mod.rs`**

- `MetadataStore` struct: `conn: Option<Arc<Mutex<Connection>>>`, `available: bool`
- Constructors: `try_new()`, `with_path()`, `open_in_memory()`, `disabled()`
- `with_conn()` at lines 56-73: the fail-soft wrapper — **all Phase 2 methods must use this**
- Phase 1 public API: `observe_profile_write`, `lookup_profile_id`, `observe_profile_rename`, `observe_profile_delete`, `sync_profiles_from_store`
- `observe_profile_write` signature includes `source_profile_id: Option<&str>` for duplication lineage

**`/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/metadata/models.rs`**

- `MetadataStoreError` enum: `HomeDirectoryUnavailable`, `Database { action, source }`, `Io { action, path, source }`, `Corrupt(String)`, `SymlinkDetected(PathBuf)`
- `SyncSource` enum variants: `AppWrite`, `AppRename`, `AppDuplicate`, `AppDelete`, `FilesystemScan`, `Import`, `InitialCensus` — **add `LaunchRuntime` for Phase 2**
- `SyncReport` struct: `profiles_seen`, `created`, `updated`, `deleted`, `errors: Vec<String>`
- `ProfileRow` struct at lines 104-117: internal row representation (not public)
- **Phase 2 additions needed**: `LaunchOutcome` enum (`Incomplete`, `Succeeded`, `Failed`, `Abandoned`), `LauncherRow` struct, `LaunchOperationRow` struct

**`/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/metadata/db.rs`**

- `open_at_path()`: symlink check, parent dir `0o700`, DB file `0o600`, PRAGMA setup, `quick_check`
- `open_in_memory()`: PRAGMA setup without WAL (uses `memory` journal mode)
- `configure_connection()` at line 68: all PRAGMAs via `execute_batch()` for literals; `pragma_update()` for `application_id`; verifies journal_mode and foreign_keys after setting
- `new_id() -> String` at line 64: `uuid::Uuid::new_v4().to_string()` — reuse for any new identity rows in Phase 2

**`/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/metadata/migrations.rs`**

- Current: v0→v1 (profiles + profile_name_history tables), v1→v2 (adds `source` column to profiles)
- **Phase 2**: add `migrate_2_to_3()` function with DDL for `launchers` and `launch_operations` tables
- Pattern: each migration is a standalone `fn migrate_N_to_N1(conn: &Connection) -> Result<(), MetadataStoreError>` using `conn.execute_batch("...")` with hard-coded DDL literals only (W7 rule)
- After adding migration: update `run_migrations()` with `if version < 3 { migrate_2_to_3(conn)?; conn.pragma_update(None, "user_version", 3_u32)?; }`

### Diagnostics Docs

`DiagnosticReport` is defined at `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/launch/diagnostics/models.rs` — all fields derive `Serialize`/`Deserialize`.

Key fields for Phase 2 column promotion:
- `report.severity` → `launch_operations.severity` (promoted for efficient query — avoids JSON parse on filter)
- `report.exit_info.failure_mode` → `launch_operations.failure_mode` (promoted for efficient query)
- Full report → `launch_operations.diagnostic_json` (serialized via `serde_json::to_string()`, bounded to 4 KB)

`should_surface_report()` is already called in `stream_log_lines` at line 215 — Phase 2 writes happen regardless of whether the report is surfaced to the UI.

---

## Post-Launch Diagnostics Feature Docs

No dedicated feature documentation file exists for the post-launch diagnostics feature (introduced in commit `82e3187`). The implementation lives entirely in:

- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/launch/diagnostics/models.rs` — type definitions
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/launch/diagnostics/mod.rs` — `analyze()` entry point
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/launch/diagnostics/patterns.rs` — pattern matching rules
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/launch/diagnostics/exit_codes.rs` — exit code interpretation

**Phase 2 relevance**: `DiagnosticReport` from `analyze()` is the payload stored in `launch_operations.diagnostic_json`. The report is already constructed in `stream_log_lines()` before the `launch-diagnostic` event fires — Phase 2 just needs to serialize it with a size check and INSERT.

**`MAX_LOG_TAIL_BYTES`** (2 MB) controls how much of the log is analyzed. The 4 KB limit on `diagnostic_json` is separate and applies after serialization of the already-analyzed report (not the raw log).

---

## Must-Read Documents

Priority order for Phase 2 implementers:

1. **`docs/plans/sqlite3-addition/feature-spec.md`** — REQUIRED. Phase 2 schema tables (lines 189-221), Phase 2 API design (lines 245-249), Phase 2 task breakdown (lines 513-523), edge case table (lines 98-110), business rules 10-13 (launcher watermark, rename cascade, delete cascade, native method).

2. **`docs/plans/sqlite3-addition/shared.md`** — REQUIRED. Compressed context with relevant files, patterns, Phase 2 table definitions, and the `spawn_blocking` pattern note. Read before touching any file.

3. **`src/crosshook-native/crates/crosshook-core/src/metadata/mod.rs`** — REQUIRED. Understand `with_conn()` before writing any Phase 2 method. All new public methods follow the same delegate pattern.

4. **`src/crosshook-native/crates/crosshook-core/src/metadata/migrations.rs`** — REQUIRED. Understand the version runner before adding the Phase 2 migration. Pattern is `if version < N { migrate(conn)?; pragma_update(N)?; }`.

5. **`src/crosshook-native/src-tauri/src/commands/launch.rs`** — REQUIRED. Understand `stream_log_lines()` flow before wiring `record_launch_started/finished`. The `spawn_blocking` bridge pattern goes here.

6. **`src/crosshook-native/src-tauri/src/commands/export.rs`** — REQUIRED. Understand the four export commands before adding metadata sync hooks.

7. **`docs/plans/sqlite3-addition/research-security.md`** — REQUIRED. W3 (4 KB diagnostic limit), W6 (re-validate stored paths), W2 (path sanitization) are all Phase 2 concerns.

8. **`docs/plans/sqlite3-addition/research-recommendations.md`** — RECOMMENDED. Phase 2 task ordering, `spawn_blocking` rationale, RF-2 (slug tombstone rule).

9. **`docs/plans/sqlite3-addition/research-technical.md`** — RECOMMENDED. Verified type-to-table mapping for Phase 2 types. Integration points with exact function references.

10. **`src/crosshook-native/crates/crosshook-core/src/launch/diagnostics/models.rs`** — RECOMMENDED. `DiagnosticReport` field layout before writing serialization code.

11. **`src/crosshook-native/crates/crosshook-core/src/export/launcher_store.rs`** — RECOMMENDED. `LauncherInfo` field layout and `derive_launcher_paths()` before writing `observe_launcher_exported`.

12. **`CLAUDE.md`** (repo root) — RECOMMENDED. Commit message format, test commands, build commands.

---

## Documentation Gaps

| Gap | Impact | Notes |
| --- | --- | --- |
| `LaunchRequest` missing `profile_name` field | **Phase 2 blocker** | Confirmed absent at `launch/request.rs:16-37`. Must be added (`pub profile_name: String` with `#[serde(default)]`) before any Phase 2 launch history work. Decide: add to `LaunchRequest` struct, or pass separately from the Tauri command layer. |
| No `spawn_blocking` example in codebase | High | Phase 2 is the first use of `tokio::task::spawn_blocking` for metadata writes in async Tauri commands. No existing pattern to copy — implementors must write it fresh. See research-technical.md for the pattern. |
| No `commands/metadata.rs` file yet | Medium | Phase 2 needs new Tauri commands for querying launch history and launcher drift state. File must be created and registered in `lib.rs::invoke_handler` and `commands/mod.rs`. |
| No TypeScript types for Phase 2 responses | Medium | `src/types/metadata.ts` does not yet exist. Frontend queries for launch history and launcher drift will need new interfaces. |
| `research-docs.md` Phase 1 content not updated for Phase 2 | Low | This file (the one you are reading) supersedes the prior Phase 1-focused `research-docs.md`. The "Must-Read" and "Documentation Gaps" sections in the old version referred to Phase 1 gaps; those are now resolved. |
| No retention policy implementation | Low | Feature spec §Decisions Resolved item 5: 90-day default, stored in `AppSettingsData.launch_history_retention_days`. Not yet implemented. Phase 2 should at minimum define the column; pruning can be Phase 3. |
| No documented `LaunchOutcome` serialization convention | Low | `launch_operations.outcome` is a TEXT column. The `LaunchOutcome` enum's `as_str()` method needs to match the spec values: `incomplete`, `succeeded`, `failed`, `abandoned`. Add to `models.rs` alongside `SyncSource::as_str()`. |
