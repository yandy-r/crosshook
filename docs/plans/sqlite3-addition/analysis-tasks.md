# Task Structure Analysis: SQLite Metadata Layer Phase 2

## Executive Summary

Phase 2 (Operational History) decomposes into **9 atomic tasks** across **5 sequential phases**. The critical path runs: `LaunchRequest` field addition → models/migrations → core sync modules (parallelizable) → `mod.rs` delegation wrappers → Tauri command hooks (parallelizable) → startup sweep → tests. The single external blocker (`LaunchRequest.profile_name`) must be resolved before any launch history work; launcher sync is independent and can start immediately after the models/migrations foundation. The `spawn_blocking` async bridge pattern is new to this codebase and affects two tasks.

---

## Cross-Cutting Rules (Every Task Must Enforce)

These rules apply across all Phase 2 tasks. An implementor working on any single task must not introduce a violation in their file, even if the surrounding code does not yet enforce the rule.

1. **Best-effort cascade only**: All Tauri call sites for metadata methods use `if let Err(e) { tracing::warn!(...) }` — never `?` on a metadata call. Metadata failure must never block the primary operation.
2. **`tauri::async_runtime::spawn_blocking` for async Tauri commands only**: Export commands (`export.rs`) are synchronous — use `State<MetadataStore>` directly. Launch commands (`launch.rs`) are async — use `spawn_blocking` for every metadata write. Do not mix the patterns.
3. **`Option<String>` operation_id sentinel**: `record_launch_started` returns `Option<String>`. Pass `None` through unchanged rather than converting to empty string. `record_launch_finished` with `None` operation_id is a silent no-op — not a warning, not an error.
4. **4 KB diagnostic_json cap (W3)**: Enforce in `launch_history.rs`, not in the command layer. When the report exceeds 4 096 bytes, still write the promoted scalar columns (`severity`, `failure_mode`, `exit_code`, `signal`); only `diagnostic_json` is `NULL`.
5. **No `format!()` in any SQL string (W7)**: All SQL must be string literals. Runtime values go in `params![]` only. This applies equally to DDL in migrations and DML in sync functions.

---

## Recommended Phase Structure

### Phase 1: Prerequisites (1 task)

Resolve the one external blocker. This is the only task that touches files outside the `metadata/` module before the foundation is laid.

**Task P2-T1 — Add `profile_name: Option<String>` to `LaunchRequest`**

This unlocks all launch history work. Launcher sync (`launcher_sync.rs`) is independent of this blocker and can begin once models + migrations are done.

---

### Phase 2: Foundation (2 tasks — parallel)

Once Phase 1 completes, these two tasks can execute in parallel. Each touches exactly one file.

**Task P2-T2 — Add Phase 2 types to `models.rs`**

Add `LaunchOutcome` enum, `DriftState` enum, `LauncherRow` struct, `LaunchOperationRow` struct, and the `MAX_DIAGNOSTIC_JSON_BYTES` constant. Every Phase 3 file depends on these types.

**Task P2-T3 — Add `migrate_2_to_3()` to `migrations.rs`**

Add DDL for `launchers` and `launch_operations` tables. Update `run_migrations()` runner with `if version < 3` guard. Depends on `MetadataStoreError` from models, but `models.rs` modifications in P2-T2 are purely additive — the error type already exists.

---

### Phase 3: Core Modules (2 tasks — parallel)

Both new module files depend on Phase 2 completion and are fully independent of each other. They can be implemented simultaneously.

**Task P2-T4 — Create `metadata/launcher_sync.rs`**

Implement:
- `observe_launcher_exported(conn, profile_name, slug, script_path, desktop_path)` — call `profile_sync::lookup_profile_id(conn, profile_name)` to resolve `profile_id` (nullable FK); UPSERT on `launcher_slug` conflict with `drift_state = 'aligned'`
- `observe_launcher_deleted(conn, profile_id, slug)` — tombstone row by setting `drift_state = 'missing'`; do not hard-delete (consistent with profile tombstone rule)
- `observe_launcher_scan(conn, profile_id, slug, current_state)` — update `drift_state` column; re-validate stored paths before any `fs::` call (W6)

Note: `lookup_profile_id` is already public on `profile_sync` (confirmed at `metadata/mod.rs:95-99` where `mod.rs` wraps it). Call the free function directly with the `conn` reference, not through `MetadataStore`.

**Task P2-T5 — Create `metadata/launch_history.rs`**

Implement:
- `record_launch_started(conn, profile_name, method, game_path, trainer_path)` → `Result<Option<String>, MetadataStoreError>` (operation_id; the `mod.rs` wrapper must return `Result<Option<String>, _>` so that `with_conn`'s `T::default()` path yields `None` when the store is disabled — not an empty string)
- `record_launch_finished(conn, operation_id, outcome, exit_code, signal, report)` — serialize `DiagnosticReport` with 4 KB truncation (W3); promote `severity` and `failure_mode` columns. `ValidationSeverity` derives `Serialize` with `#[serde(rename_all = "snake_case")]` — use `serde_json::to_string(&report.severity)` (then strip quotes) or `report.severity.to_string()` to get the column string value
- `sweep_abandoned_operations(conn)` — mark `outcome = 'incomplete'` rows with `started_at < now - 24h` as `'abandoned'`

---

### Phase 4: Integration (3 tasks — partially parallel)

Delegate from `mod.rs` first (P2-T6), then all three Tauri integration tasks are unblocked. Export and launch command hooks are independent of each other and of the startup sweep — all three can run concurrently.

**Task P2-T6 — Add Phase 2 method wrappers to `metadata/mod.rs`**

Add submodule declarations and public `with_conn` wrappers for all Phase 3 methods: `record_launch_started`, `record_launch_finished`, `observe_launcher_exported`, `observe_launcher_deleted`, `observe_launcher_scan`. This is the `mod.rs` routing-surface task; no business logic lives here.

**Task P2-T7 — Wire launcher sync hooks into `commands/export.rs`**

- Add `State<MetadataStore>` parameter to `export_launchers` and `delete_launcher*` commands
- After successful `export_launchers_core()`, call `metadata_store.observe_launcher_exported(...)` — best-effort, warn-only
- After successful `delete_launcher_*()`, call `metadata_store.observe_launcher_deleted(...)` — best-effort
- Optionally: wire `check_launcher_for_profile` → `observe_launcher_scan` for drift detection
- Also requires: `SteamExternalLauncherExportRequest` needs `profile_name: Option<String>` with `#[serde(default)]` added to `export/launcher.rs`

**Task P2-T8 — Wire launch history hooks into `commands/launch.rs`**

- Access `MetadataStore` via `app.state::<MetadataStore>()` (no signature change needed — `AppHandle` already present)
- Call `record_launch_started` via `spawn_blocking` before `command.spawn()` in both `launch_game` and `launch_trainer`
- Thread returned `operation_id: String` through `spawn_log_stream` → `stream_log_lines` as a new parameter
- Call `record_launch_finished` via `spawn_blocking` inside `stream_log_lines` after `analyze()` runs and exit status is resolved
- Apply `sanitize_display_path()` to `log_path` before storing (W2)
- Enforce 4 KB truncation on `diagnostic_json` before insert (W3)

**Task P2-T9 — Add `sweep_abandoned_operations` to `startup.rs`**

- Add `pub fn sweep_abandoned_operations(metadata_store: &MetadataStore) -> Result<(), StartupError>` that delegates to `metadata_store.sweep_abandoned_operations()`
- Call it from `lib.rs` setup closure after existing `run_metadata_reconciliation()`, wrapped in best-effort `if let Err(e)` pattern
- Non-fatal; must not block app startup

---

### Phase 5: Testing (1 task)

**Task P2-T10 — Add Phase 2 unit and integration tests**

Add to `metadata/mod.rs` `#[cfg(test)] mod tests` (or per-module inline tests). All use `MetadataStore::open_in_memory()`.

Required test cases:
1. `test_observe_launcher_exported_creates_row` — UPSERT creates row with `drift_state = 'aligned'`
2. `test_observe_launcher_exported_idempotent` — re-export same slug does not duplicate
3. `test_observe_launcher_deleted_tombstones` — `drift_state = 'missing'` set, row not hard-deleted
4. `test_record_launch_started_returns_operation_id` — non-empty string returned
5. `test_record_launch_finished_updates_row` — outcome, exit_code, diagnostic_json written
6. `test_diagnostic_json_truncated_at_4kb` — oversized report capped before insert
7. `test_sweep_abandoned_marks_old_operations` — operations older than 24h swept to `abandoned`
8. `test_record_launch_finished_unknown_operation_id_is_noop` — log warning, no panic, `Ok(())`
9. `test_phase2_disabled_store_noop` — all Phase 2 methods return `Ok(())` on disabled store

---

## Task Granularity Recommendations

Each task stays within the 1–3 file guideline:

| Task  | Files Touched                                                                           | File Count |
| ----- | --------------------------------------------------------------------------------------- | ---------- |
| P2-T1 | `launch/request.rs`, `launch/script_runner.rs` (test fixtures only)                     | 2          |
| P2-T2 | `metadata/models.rs`                                                                    | 1          |
| P2-T3 | `metadata/migrations.rs`                                                                | 1          |
| P2-T4 | `metadata/launcher_sync.rs` (new)                                                       | 1          |
| P2-T5 | `metadata/launch_history.rs` (new)                                                      | 1          |
| P2-T6 | `metadata/mod.rs`                                                                       | 1          |
| P2-T7 | `commands/export.rs`, `export/launcher.rs` (add `profile_name` field)                   | 2          |
| P2-T8 | `commands/launch.rs`                                                                    | 1          |
| P2-T9 | `startup.rs`, `lib.rs` (2-line sweep call added)                                        | 2          |
| P2-T10| `metadata/mod.rs` (test module), optionally inline per-module tests                    | 1–4        |

---

## Dependency Analysis

### Full DAG

```
P2-T1 (LaunchRequest.profile_name)
    └─→ P2-T5 (launch_history.rs)
            └─→ P2-T6 (mod.rs wrappers)
                    └─→ P2-T8 (commands/launch.rs)
                                └─→ P2-T10 (tests)

P2-T2 (models.rs Phase 2 types)
    ├─→ P2-T4 (launcher_sync.rs)  ┐
    ├─→ P2-T5 (launch_history.rs) ┤ parallel
    └─→ P2-T3 (migrations.rs)  ───┘
                ↓
           P2-T4, P2-T5 (implicit schema dep)

P2-T4 (launcher_sync.rs)
    └─→ P2-T6 (mod.rs wrappers)
            └─→ P2-T7 (commands/export.rs)
                        └─→ P2-T10 (tests)

P2-T6 (mod.rs wrappers)
    ├─→ P2-T7 (export hooks)   ┐
    ├─→ P2-T8 (launch hooks)   ┤ parallel
    └─→ P2-T9 (startup sweep)  ┘
                → P2-T10 (tests, waits for all three)
```

### Critical Path

```
P2-T1 → P2-T2 → P2-T5 → P2-T6 → P2-T8 → P2-T10
```

Where P2-T2 and P2-T3 can run in parallel (P2-T3 only adds DDL, `MetadataStoreError` already exists), and P2-T1 is required before P2-T5 but not before P2-T4.

**Critical path depth:** 6 sequential tasks minimum.

---

## File-to-Task Mapping

### New Files to Create

| File                                                       | Task  | Description                                      |
| ---------------------------------------------------------- | ----- | ------------------------------------------------ |
| `crates/crosshook-core/src/metadata/launcher_sync.rs`     | P2-T4 | Launcher observation + drift state functions     |
| `crates/crosshook-core/src/metadata/launch_history.rs`    | P2-T5 | Launch operation lifecycle functions + sweep     |

### Files to Modify

| File                                                                | Task         | Change                                                              |
| ------------------------------------------------------------------- | ------------ | ------------------------------------------------------------------- |
| `crates/crosshook-core/src/launch/request.rs`                       | P2-T1        | Add `pub profile_name: Option<String>` with `#[serde(default)]`    |
| `crates/crosshook-core/src/launch/script_runner.rs`                 | P2-T1        | Add `profile_name: None` to test fixture struct literals            |
| `src-tauri/src/commands/export.rs`                                  | P2-T1 (minor) | Add `profile_name: None` to `rename_launcher` builder at lines 95–108 (struct literal update only) |
| `crates/crosshook-core/src/metadata/models.rs`                      | P2-T2        | Add `LaunchOutcome`, `DriftState`, `LauncherRow`, `LaunchOperationRow`, `MAX_DIAGNOSTIC_JSON_BYTES` |
| `crates/crosshook-core/src/metadata/migrations.rs`                  | P2-T3        | Add `migrate_2_to_3()` + `if version < 3` guard in runner          |
| `crates/crosshook-core/src/metadata/mod.rs`                         | P2-T6        | Add submodule declarations + `with_conn` public wrappers           |
| `crates/crosshook-core/src/export/launcher.rs`                      | P2-T7        | Add `profile_name: Option<String>` to `SteamExternalLauncherExportRequest` |
| `src-tauri/src/commands/export.rs`                                  | P2-T7        | Add `State<MetadataStore>` + `observe_launcher_exported` hooks     |
| `src-tauri/src/commands/launch.rs`                                  | P2-T8        | Add `spawn_blocking` hooks for `record_launch_started/finished`    |
| `src-tauri/src/startup.rs`                                          | P2-T9        | Add `sweep_abandoned_operations()` function                        |
| `src-tauri/src/lib.rs`                                              | P2-T9        | Call `startup::sweep_abandoned_operations()` in `.setup()` closure |
| `crates/crosshook-core/src/metadata/mod.rs`                         | P2-T10       | Add `#[cfg(test)] mod tests` for Phase 2 unit tests                |

**Not touched in Phase 2:** `profile/toml_store.rs`, `metadata/db.rs`, `metadata/profile_sync.rs`, `commands/profile.rs`, `metadata/models.rs`'s existing Phase 1 types (additive only).

---

## Parallelization Opportunities

### Parallel Group A: Phase 2 Foundation (after none)

P2-T2 (`models.rs` additions) and P2-T3 (`migrations.rs`) can run in parallel. P2-T3 only adds DDL strings; `MetadataStoreError` already exists from Phase 1. No cross-file dependencies between them.

```
START → P2-T2 ∥ P2-T3
```

### Parallel Group B: Core Modules (after P2-T2 + P2-T3 + P2-T1)

P2-T4 (`launcher_sync.rs`) and P2-T5 (`launch_history.rs`) are fully independent of each other. Both need P2-T2 models for `LaunchOutcome`/`DriftState` types, and P2-T5 additionally needs P2-T1 for `profile_name` threading.

- P2-T4 can start as soon as P2-T2 + P2-T3 complete (no P2-T1 dependency)
- P2-T5 can start as soon as P2-T1 + P2-T2 + P2-T3 complete

```
(P2-T2 + P2-T3 complete) → P2-T4 ─┐
(P2-T1 + P2-T2 + P2-T3 complete) → P2-T5 ─┤ parallel
                                            └─→ P2-T6
```

### Parallel Group C: Tauri Integration (after P2-T6)

P2-T7, P2-T8, and P2-T9 are all unblocked simultaneously once P2-T6 lands. None depends on another within this group.

```
P2-T6 → P2-T7 ∥ P2-T8 ∥ P2-T9 → P2-T10
```

### Optimized Parallel Schedule

```
Batch 0:   P2-T1 (prerequisite — can start immediately, in parallel with Batch 1)
Batch 1:   P2-T2 ∥ P2-T3
Batch 2:   P2-T4 ∥ P2-T5   (P2-T4 unblocked by Batch 1; P2-T5 also needs Batch 0)
Batch 3:   P2-T6
Batch 4:   P2-T7 ∥ P2-T8 ∥ P2-T9
Batch 5:   P2-T10
```

Minimum wall-clock depth (fully parallel): 6 batches, but Batch 0 can overlap with Batch 1 since P2-T1 is in a different module tree from P2-T2/P2-T3.

---

## Implementation Strategy Recommendations

### 1. Start P2-T1 in Parallel with P2-T2/P2-T3

`LaunchRequest` and `models.rs`/`migrations.rs` touch completely different module trees. There is no compile-time dependency between them at the write stage. A developer can work on P2-T1 simultaneously with another working on P2-T2 + P2-T3.

### 2. P2-T4 Is Independently Startable After Foundation

`launcher_sync.rs` has no dependency on the `LaunchRequest` gap. If P2-T1 is slow (frontend changes required to populate `profile_name`), P2-T4 can complete while P2-T1 is still in review.

### 3. `spawn_blocking` Is New to This Codebase — Plan Carefully for P2-T8

Phase 1 used `State<T>` with synchronous commands only. P2-T8 introduces the first `tauri::async_runtime::spawn_blocking` bridge for `rusqlite` in async Tauri commands. Key gotchas:

- Use `tauri::async_runtime::spawn_blocking` (not `tokio::task::spawn_blocking` directly) — Tauri's runtime wrapper is the correct call inside Tauri commands
- `MetadataStore` is `Clone` (cheap `Arc` clone) — clone before the `spawn_blocking` closure; do not move the state reference
- **`stream_log_lines` signature change required**: add `operation_id: Option<String>` as a fifth parameter. Thread it from `launch_game`/`launch_trainer` → `spawn_log_stream` → `stream_log_lines` so the detached background task can call `record_launch_finished`
- `record_launch_started` is called before `command.spawn()` — await it, but on failure the launch still proceeds (best-effort); use `None` as the fallback `operation_id`
- `record_launch_finished` also needs `spawn_blocking` inside `stream_log_lines`; this background task is already detached from the command, so there is no return path to thread errors to the UI

**`operation_id` sentinel: use `Option<String>`, never empty string.**

`record_launch_started` on `mod.rs` must be declared as `pub fn record_launch_started(...) -> Result<Option<String>, MetadataStoreError>`. This is required because `with_conn`'s disabled-path returns `Ok(T::default())` — for `T = Option<String>`, `default()` is `None`, which is the correct sentinel. For `T = String`, `default()` would be `""` (empty), requiring a guard like `if !op_id.is_empty()` at every call site. Use `lookup_profile_id` in `mod.rs` (line 95) as the precedent — it already returns `Result<Option<String>, _>` through `with_conn`.

`record_launch_finished` with `operation_id = None` must be a silent no-op — not a warning, not an error. This is the normal degraded-mode path when `MetadataStore::disabled()` is in use.

**Recommended pattern for `record_launch_started`:**

```rust
// In launch_game / launch_trainer (async command):
let meta = app.state::<MetadataStore>().inner().clone();
let profile_name_owned = request.profile_name.clone().unwrap_or_default();
let method_owned = method.to_string();
let operation_id: Option<String> = tauri::async_runtime::spawn_blocking(move || {
    meta.record_launch_started(&profile_name_owned, &method_owned, None, None)
})
.await
.unwrap_or_else(|e| {
    tracing::warn!("metadata spawn_blocking join failed: {e}");
    Ok(None)
})
.unwrap_or_else(|e| {
    tracing::warn!(%e, "record_launch_started failed");
    None
});
// Thread `operation_id` into spawn_log_stream as Option<String>
```

### 4. DiagnosticReport Truncation Must Live in P2-T5, Not P2-T8

The 4 KB enforcement belongs in `launch_history.rs::record_launch_finished`, not in the Tauri command layer. The command layer just passes `Option<&DiagnosticReport>`; the metadata layer truncates. This keeps the security rule in one place (W3) and avoids leaking it into command handler code.

Recommended approach: serialize to `serde_json::to_string()`, then check `bytes.len() > MAX_DIAGNOSTIC_JSON_BYTES`. If over limit, still insert `None` for `diagnostic_json` but retain the promoted scalar columns (`severity`, `failure_mode`, `exit_code`, `signal`).

### 5. Launcher Schema Discrepancy — Verify PK Design Before P2-T3

`shared.md` and `research-docs.md` disagree on the `launchers` PK:

- `shared.md` specifies `launcher_id TEXT PK` (UUID) + index on `launcher_slug` + nullable `profile_id`
- `research-docs.md` specifies composite PK `(profile_id, launcher_slug)`

The `shared.md` decision table (section "Design Decisions (Locked)") is authoritative: UUID PK `launcher_id` wins because nullable `profile_id` in a composite PK creates SQLite ambiguity. P2-T3 DDL must use `launcher_id TEXT PRIMARY KEY` with a separate index on `(profile_id, launcher_slug)`.

This also means P2-T5's `record_launch_started` should use `db::new_id()` for `operation_id` rather than `AUTOINCREMENT` — `shared.md` locks this as UUID TEXT PK for `launch_operations` too. Verify against `feature-spec.md` lines 189-222 before writing DDL.

### 6. Path Validation Before Filesystem Ops in P2-T4

`observe_launcher_scan` will retrieve `script_path` and `desktop_entry_path` from the DB before checking them on disk. W6 requires re-validating these before any `fs::` call. Implement a private `validate_stored_path(path: &str) -> Result<PathBuf, MetadataStoreError>` in `launcher_sync.rs` that:
- Confirms the path is absolute
- Confirms no `..` components
- Optionally confirms prefix is within expected home directory subtree

This is not a shared utility yet — if it grows, promote it to `db.rs` or a new `paths.rs` file in a later phase.

### 7. SteamExternalLauncherExportRequest Needs `profile_name` Added in P2-T7

The export command gap mirrors the `LaunchRequest` gap. Adding `profile_name: Option<String>` with `#[serde(default)]` to `SteamExternalLauncherExportRequest` in `export/launcher.rs` is part of P2-T7 scope, not P2-T1. The two gaps are in different files and can be resolved independently.

### 8. Startup Sweep (P2-T9) Should Log Swept Count at INFO Level

Following the pattern of `run_metadata_reconciliation()` in `startup.rs` (which logs `created` and `updated` counts at INFO when non-zero), the sweep should log at `tracing::info!` when it marks any rows as abandoned. Use `tracing::warn!` only if the sweep itself fails — consistent with the existing startup reconciliation log discipline.

---

## Dependency Matrix (Phase 2 Tasks)

| Task  | Depends On                          | Blocks                      |
| ----- | ----------------------------------- | --------------------------- |
| P2-T1 | (none — Phase 1 complete)           | P2-T5                       |
| P2-T2 | (none — Phase 1 complete)           | P2-T3, P2-T4, P2-T5        |
| P2-T3 | P2-T2 (MetadataStoreError exists)   | P2-T4, P2-T5 (schema dep)  |
| P2-T4 | P2-T2, P2-T3                        | P2-T6                       |
| P2-T5 | P2-T1, P2-T2, P2-T3                 | P2-T6                       |
| P2-T6 | P2-T4, P2-T5                        | P2-T7, P2-T8, P2-T9        |
| P2-T7 | P2-T6                               | P2-T10                      |
| P2-T8 | P2-T6                               | P2-T10                      |
| P2-T9 | P2-T6                               | P2-T10                      |
| P2-T10| P2-T7, P2-T8, P2-T9                | (final)                     |

---

## Must-Read Documents (Per Task)

| Task  | Must Read Before Starting                                                                              |
| ----- | ------------------------------------------------------------------------------------------------------ |
| P2-T1 | `research-integration.md` (LaunchRequest gap analysis), `shared.md`                                   |
| P2-T2 | `shared.md` (models section), `research-docs.md` (Phase 2 API requirements), `feature-spec.md` L189-222 |
| P2-T3 | `migrations.rs` (current migration runner), `shared.md` (table DDL), `feature-spec.md` L189-222       |
| P2-T4 | `research-integration.md` (export hooks), `shared.md` (patterns), `research-security.md` W6           |
| P2-T5 | `research-integration.md` (launch hooks, DiagnosticReport), `shared.md`, `research-security.md` W3    |
| P2-T6 | `metadata/mod.rs` (with_conn pattern), `shared.md`                                                     |
| P2-T7 | `commands/export.rs`, `research-integration.md` (export hook points), `shared.md`                     |
| P2-T8 | `commands/launch.rs`, `research-integration.md` (spawn_blocking pattern), `research-docs.md`          |
| P2-T9 | `startup.rs`, `research-integration.md` (startup sweep placement), `shared.md`                        |
| P2-T10| All implementation files, `research-docs.md` (success criteria), `research-security.md` W3/W6         |
