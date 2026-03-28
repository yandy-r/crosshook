# SQLite Metadata Layer — Task Breakdown Analysis

## Executive Summary

Phase 1 decomposes into **10 atomic tasks** across 4 sequential batches. The critical path runs through dependency addition → connection factory → schema models → core sync logic → Tauri integration → startup reconciliation → tests. The `sanitize_display_path()` promotion is a Phase 1 prerequisite that must land before any Tauri command modifications to avoid duplicating the function. Security hardening (W1-W8) integrates into the connection factory task, not as separate tasks. All metadata module files can be created in a single parallel batch once Cargo.toml is updated.

---

## Recommended Phase Structure

### Phase 1: Identity Foundation (10 tasks, 4 batches)

**Batch 0 — Prerequisites (1 task, sequential)**
Must complete before anything else: `sanitize_display_path()` exists in `launch.rs` and must be promoted to `shared.rs` before any command files are touched.

**Batch 1 — Foundation (2 tasks, parallel)**
Cargo.toml dependency addition and metadata module skeleton can proceed in parallel. No interdependencies.

**Batch 2 — Core Module (4 tasks, parallel)**
Once the module skeleton and dependencies exist, all internal metadata files (`db.rs`, `migrations.rs`, `models.rs`, `profile_sync.rs`) can be written in parallel. Each touches exactly one new file.

**Batch 3 — Tauri Integration (2 tasks, sequential-then-parallel)**
`lib.rs` registration must happen before command modifications so the `MetadataStore` type is available in `State<>`. Command modifications to `profile.rs` and `startup.rs` can then run in parallel.

**Batch 4 — Tests (1 task)**
Integration and unit tests for the metadata module; run after all implementation is complete.

---

## Task Granularity Recommendations

### T0 — Promote `sanitize_display_path()` to shared utility

**Files to modify**: `src-tauri/src/commands/shared.rs`, `src-tauri/src/commands/launch.rs`

**What to do**:

1. Move `fn sanitize_display_path(path: &str) -> String` from `launch.rs:301` to `shared.rs`
2. Make it `pub fn`
3. In `launch.rs`, replace the local definition with `use super::shared::sanitize_display_path;`

**Dependencies**: none
**Scope**: 2 files, ~10 lines moved
**Why prerequisite**: W2 security finding requires shared path sanitization across all new IPC commands. If any command task lands first with a local copy, deduplication becomes a merge conflict.

---

### T1 — Add `rusqlite` + `uuid` to `Cargo.toml`

**File to modify**: `crates/crosshook-core/Cargo.toml`

**What to add**:

```toml
rusqlite = { version = "0.39", features = ["bundled"] }
uuid     = { version = "1",    features = ["v4", "serde"] }
```

**Dependencies**: none (parallel with T2)
**Scope**: 1 file, 2 lines
**Note**: `bundled` is mandatory — SteamOS may ship affected SQLite versions. Verify with a `cargo check` after adding.

---

### T2 — Create `metadata/mod.rs` module skeleton

**File to create**: `crates/crosshook-core/src/metadata/mod.rs`

**What to do**:

1. Declare submodules: `mod db; mod migrations; mod models; mod profile_sync;`
2. Define `pub struct MetadataStore { conn: Arc<Mutex<Connection>>, available: bool }`
3. Implement stub `try_new()`, `with_path()`, `open_in_memory()`, and `disabled()` constructors (matching three-constructor pattern from `toml_store.rs:83-98`). `disabled()` and `try_new()` must be in the same task — `lib.rs` T7 needs both to compile
4. Forward stub declarations for all Phase 1 public methods

**Also modify**: `crates/crosshook-core/src/lib.rs` — add `pub mod metadata;`

**Dependencies**: T1 must be complete (needs `rusqlite` in scope)
**Scope**: 1 new file + 1 line in lib.rs
**Note**: `MetadataStore` must be `Clone` via `Arc<Mutex<Connection>>` — derive or impl `Clone` manually.

---

### T3 — Create `metadata/db.rs` (connection factory + security hardening)

**File to create**: `crates/crosshook-core/src/metadata/db.rs`

**What to implement**:

- `pub fn open_connection(path: &Path) -> Result<Connection, MetadataError>` — the single factory all opens go through
- Symlink check before `Connection::open()` (W5): `if path.symlink_metadata().is_ok() { return Err(...) }`
- File permission enforcement `0o600` immediately after creation (W1)
- Parent directory permission `0o700` (W1)
- All PRAGMAs via `conn.pragma_update()` — never `execute_batch()` (W7):
  - `foreign_keys = ON`
  - `journal_mode = WAL`
  - `synchronous = NORMAL`
  - `busy_timeout = 5000`
  - `secure_delete = ON`
  - `application_id = <crosshook-specific u32>`
- `PRAGMA quick_check` at startup; return `MetadataError::Corrupt` on failure (A5)

**Dependencies**: T2 (needs `MetadataError` type from models.rs — see ordering note below)
**Scope**: 1 new file, ~80 lines
**Ordering note**: `db.rs` depends on `MetadataError` which lives in `models.rs` (T4). In practice, write a forward declaration or write T4 and T3 together in the same batch. If running strictly in parallel, stub `MetadataError` in `db.rs` and reconcile after T4 lands.

---

### T4 — Create `metadata/models.rs`

**File to create**: `crates/crosshook-core/src/metadata/models.rs`

**What to define**:

- `MetadataError` enum — structured error type following `LoggingError` pattern (`logging.rs:21-58`):
  - `HomeDirectoryUnavailable`
  - `Io { action: &'static str, path: PathBuf, source: io::Error }`
  - `Corrupt(String)` — for `quick_check` failures
  - `Sql(rusqlite::Error)` — internal; mapped to opaque variants at IPC boundary (A3)
  - `UnavailableStore` — emitted by methods when `available = false`
- `ProfileRow` — SQLite row struct for `profiles` table (with `Serialize`/`Deserialize`)
- `SyncSource` enum: `AppWrite`, `AppRename`, `AppDuplicate`, `FilesystemScan`, `Import`, `InitialCensus`
- `SyncReport` struct: counts of inserted/updated/skipped/error rows
- `LaunchOutcome` enum (Phase 2 stub): `Incomplete`, `Succeeded`, `Failed`, `Abandoned`

**Dependencies**: T2 (module scope)
**Scope**: 1 new file, ~120 lines

---

### T5 — Create `metadata/migrations.rs`

**File to create**: `crates/crosshook-core/src/metadata/migrations.rs`

**What to implement**:

- `pub fn run_migrations(conn: &Connection) -> Result<(), MetadataError>`
- Hand-rolled `PRAGMA user_version`-based runner (~20 lines per spec)
- Migration 0 → 1: DDL for `profiles` + `profile_name_history` tables
  - `profiles`: all Phase 1 columns including `is_favorite`, `is_pinned`, `source_profile_id`, `deleted_at`
  - `idx_profiles_current_filename` UNIQUE index
  - `profile_name_history` with FK to `profiles.profile_id`
- All DDL as hard-coded string literals in `execute_batch()` only (W7 rule: never dynamic SQL)

**Dependencies**: T4 (needs `MetadataError`)
**Scope**: 1 new file, ~80 lines
**Security**: `execute_batch()` is acceptable only for DDL literals — migrations is the only permitted use.

---

### T6 — Create `metadata/profile_sync.rs`

**File to create**: `crates/crosshook-core/src/metadata/profile_sync.rs`

**What to implement**:

- `pub fn observe_profile_write(conn: &MutexGuard<Connection>, name: &str, profile: &GameProfile, path: &Path, source: SyncSource) -> Result<(), MetadataError>`
  - UPSERT into `profiles` with parameterized query: `INSERT ... ON CONFLICT(current_filename) DO UPDATE`
  - Insert into `profile_name_history` on name/path changes
  - Extract `game_name` from `profile.game.name`, `launch_method` from `profile.launch.method`
- `pub fn observe_profile_rename(conn: &MutexGuard<Connection>, old_name: &str, new_name: &str, old_path: &Path, new_path: &Path) -> Result<(), MetadataError>`
  - Update `profiles.current_filename` + `current_path`
  - Append history row with `source = AppRename`
- `pub fn observe_profile_delete(conn: &MutexGuard<Connection>, name: &str) -> Result<(), MetadataError>`
  - Soft-delete: `UPDATE profiles SET deleted_at = ... WHERE current_filename = ?` (tombstone rule, never hard delete)
- `pub fn sync_profiles_from_store(conn: &MutexGuard<Connection>, store: &ProfileStore) -> Result<SyncReport, MetadataError>`
  - Full scan of TOML profiles; UPSERT each via `observe_profile_write` with `SyncSource::InitialCensus`
  - Detect SQLite rows with no matching TOML file → mark as potentially deleted

**All SQL**: parameterized `params![]` exclusively — no `format!()` in SQL strings (W4)

**Dependencies**: T4 (models), T5 (migrations must have run to create tables)
**Scope**: 1 new file, ~150 lines

---

### T7 — Register `MetadataStore` in Tauri `lib.rs`

**File to modify**: `src-tauri/src/lib.rs`

**What to do**:

1. Add `use crosshook_core::metadata::MetadataStore;`
2. Initialize store: `let metadata_store = MetadataStore::try_new().unwrap_or_else(|err| { tracing::warn!(%err, "SQLite metadata unavailable"); MetadataStore::disabled() });`
   - `MetadataStore::try_new()` returns `Result<Self, String>` (IPC error convention)
   - On failure: construct with `available = false` via `MetadataStore::disabled()`
3. Add `.manage(metadata_store.clone())` after existing store registrations
4. In `setup()` closure: run `sync_profiles_from_store()` as best-effort after logging init

**Dependencies**: T2, T3, T4, T5, T6 all complete (MetadataStore must compile)
**Scope**: 1 file, ~15 lines added
**Note**: This is the gate for all command modifications — must land before T8/T9.

---

### T8 — Add metadata sync hooks to `profile.rs` commands

**File to modify**: `src-tauri/src/commands/profile.rs`

**What to add** (best-effort cascade pattern from `profile_rename:149-194`):

After `profile_save` (`store.save(&name, &data).map_err(map_error)?`):

```rust
if let Err(error) = state.metadata_store.observe_profile_write(&name, &data, &path, SyncSource::AppWrite) {
    tracing::warn!(%error, profile_name = %name, "metadata sync failed after profile save");
}
```

Similarly after: `profile_rename`, `profile_delete` (use `observe_profile_delete` for tombstone), `profile_duplicate` (use `observe_profile_write` with `source_profile_id`), `profile_import_legacy` (use `SyncSource::Import`)

**Dependencies**: T7 (MetadataStore in Tauri state), T0 (sanitize_display_path promoted)
**Scope**: 1 file, ~25 lines added across 5 command functions
**Critical**: `profile_rename` hook must fire AFTER the TOML rename succeeds (preserve `?` propagation order).

---

### T9 — Add startup reconciliation scan to `startup.rs`

**File to modify**: `src-tauri/src/startup.rs`

**What to add**:

- New function `run_startup_reconciliation(metadata_store: &MetadataStore, profile_store: &ProfileStore) -> Result<SyncReport, StartupError>`
- Calls `metadata_store.sync_profiles_from_store(profile_store)`
- Extend `StartupError` with `Metadata(MetadataError)` variant + `From<MetadataError>`
- Called from `lib.rs` `setup()` closure as best-effort (log warning on failure, never panic)

**Dependencies**: T7 (MetadataStore API), T6 (sync_profiles_from_store)
**Scope**: 1 file, ~35 lines added
**Note**: Must not block startup — wrap in best-effort pattern, not `?`-propagation.

---

### T10 — Metadata module unit + integration tests

**Files to create/modify**: `crates/crosshook-core/src/metadata/mod.rs` (test module), or separate `tests/` files

**What to test**:

Unit tests (in-memory DB via `MetadataStore::open_in_memory()`):

- `observe_profile_write` → row exists in `profiles`
- `observe_profile_rename` → `profile_name_history` row appended, `profiles.current_filename` updated
- `observe_profile_delete` → `deleted_at` set, row not hard-deleted
- `observe_profile_write` idempotency (UPSERT, not duplicate insert)
- `sync_profiles_from_store` with populated `ProfileStore::with_base_path()`

Integration tests:

- Connection factory creates file at correct path with `0o600` permissions
- Symlink check blocks open when target is a symlink
- `sync_profiles_from_store` with 0, 1, and many profiles

**Existing test pattern**: `startup.rs:66-149` shows `tempdir()` + `with_base_path()` pattern. Replicate for `MetadataStore::open_in_memory()` and `MetadataStore::with_path(tempdir)`.

**Dependencies**: T9 complete (all implementation done)
**Scope**: new test modules, ~200 lines

---

## Dependency Analysis (DAG)

```
T0 (sanitize_display_path promotion)
    └─→ T8 (profile.rs command hooks)

T1 (Cargo.toml deps)
    └─→ T2 (mod.rs skeleton + lib.rs mod declaration)
            ├─→ T3 (db.rs) ─┐
            ├─→ T4 (models.rs) ─┤
            └─→ T5 (migrations.rs) ─┤
                                    ├─→ T6 (profile_sync.rs)
                                            └─→ T7 (lib.rs Tauri registration)
                                                    ├─→ T8 (profile.rs hooks) ─┐
                                                    └─→ T9 (startup.rs) ────────┤
                                                                                 └─→ T10 (tests)
```

**Critical path**: T1 → T2 → T4 → T6 → T7 → T8 → T10 (8 sequential steps minimum)
**T0** is independent of the metadata module and can proceed any time before T8.

---

## File-to-Task Mapping

| File                                                 | Task | Action                                                    |
| ---------------------------------------------------- | ---- | --------------------------------------------------------- |
| `crates/crosshook-core/Cargo.toml`                   | T1   | Modify: add rusqlite + uuid                               |
| `crates/crosshook-core/src/lib.rs`                   | T2   | Modify: add `pub mod metadata;`                           |
| `crates/crosshook-core/src/metadata/mod.rs`          | T2   | Create: MetadataStore struct + API stubs                  |
| `crates/crosshook-core/src/metadata/db.rs`           | T3   | Create: connection factory + security hardening           |
| `crates/crosshook-core/src/metadata/models.rs`       | T4   | Create: MetadataError, ProfileRow, SyncSource, SyncReport |
| `crates/crosshook-core/src/metadata/migrations.rs`   | T5   | Create: DDL + user_version runner                         |
| `crates/crosshook-core/src/metadata/profile_sync.rs` | T6   | Create: observe\_\* + sync_profiles_from_store            |
| `src-tauri/src/lib.rs`                               | T7   | Modify: initialize + .manage() MetadataStore              |
| `src-tauri/src/commands/shared.rs`                   | T0   | Modify: add pub sanitize_display_path                     |
| `src-tauri/src/commands/launch.rs`                   | T0   | Modify: use shared::sanitize_display_path                 |
| `src-tauri/src/commands/profile.rs`                  | T8   | Modify: add 5 metadata sync hooks                         |
| `src-tauri/src/startup.rs`                           | T9   | Modify: add reconciliation fn + MetadataError variant     |

**Not touched in Phase 1** (confirmed): `profile/toml_store.rs`, `commands/launch.rs` (beyond T0), `commands/export.rs`, `commands/community.rs`, `launch/request.rs`, `commands/mod.rs` (Phase 2 adds `pub mod metadata;`), `commands/metadata.rs` (Phase 2 new file — no frontend-facing metadata commands exist in Phase 1)

---

## Optimization Opportunities (Parallel Execution)

### Batch 1 (fully parallel after T0)

- **T1** (Cargo.toml) ∥ **T2** (mod.rs skeleton)
  - T1 and T2 have no dependency on each other. However T2 needs T1 to compile. For a parallel plan, a human/CI can write T2 content while T1 is being reviewed, but final verification requires T1 present.

### Batch 2 (parallel after T1+T2)

- **T3** (db.rs) ∥ **T4** (models.rs) ∥ **T5** (migrations.rs)
  - T3 and T4 have a mild cross-dependency: `db.rs` references `MetadataError` from `models.rs`. Resolve by writing `MetadataError` as the first thing in T4, or having T3 use a placeholder error type initially. In practice, an implementor writing T3 should write the error stub inline or coordinate with T4 to define the error type first.
  - T5 (migrations) depends only on `MetadataError` from T4, not on `db.rs`. T4 can unblock T5.
  - **Recommendation**: T4 first (fast, ~120 lines), then T3 ∥ T5 in parallel.

### Batch 3 (parallel after batch 2)

- **T6** (profile_sync.rs) — depends on T4+T5

### Batch 4 (parallel after T6)

- **T7** (lib.rs registration) then **T8** (profile.rs hooks) ∥ **T9** (startup.rs) in parallel

### Summary parallel schedule

```
Serial:   T0
Parallel: T1 ∥ (T2 after T1)
Parallel: T4 → (T3 ∥ T5)
Serial:   T6
Serial:   T7
Parallel: T8 ∥ T9
Serial:   T10
```

---

## Teammate Input Synthesis

Inputs received from `context-synthesizer` and `code-analyzer` after initial analysis. Reconciliation notes:

### Conflict: `Option<MetadataStore>` vs always-present with `available` flag

`context-synthesizer` suggested `Option<MetadataStore>` in Tauri state. This conflicts with the feature spec and my analysis. The spec is explicit (feature-spec.md §Decisions Resolved, §Fail-Soft Rule):

> "always-present `MetadataStore` with internal `available: bool` flag. This avoids `Option<MetadataStore>` checks at every call site. Methods return early with a logged warning when `available` is false."

**Resolution**: Always-present pattern with `available` flag wins. `lib.rs` constructs an `disabled()` instance on failure, not `None`. The `Option<MetadataStore>` suggestion from context-synthesizer was shorthand and does not reflect the spec.

### Clarification: `sanitize_display_path()` timing

`context-synthesizer` scoped this as "before Phase 2/3 Tauri metadata commands." My analysis treats it as a Phase 1 prerequisite (T0 before T8). The difference:

- T8 adds metadata sync to existing profile commands — not new metadata commands
- The profile commands themselves return paths (via `DiagnosticReport`, launcher paths) that need sanitization
- W2 applies to every IPC response path, including existing profile commands that get new code paths

**Resolution**: Keep T0 as Phase 1 prerequisite. Promoting it in Phase 1 before touching any command files prevents the function from being duplicated across phases. The cost is negligible (2 files, ~3 line change).

### Addition: `commands/metadata.rs` is Phase 2+, not Phase 1

`code-analyzer` listed `src-tauri/src/commands/metadata.rs` and the matching `commands/mod.rs` addition as Phase 1 tasks. Phase 1 has no new frontend-facing metadata Tauri commands — only sync hooks injected into existing commands. A metadata commands file would be empty in Phase 1.

**Resolution**: `commands/metadata.rs` and its `mod.rs` declaration are Phase 2 prerequisites (favorites toggle, history query commands). Not included in Phase 1 task list. The `commands/mod.rs` modification to add `pub mod metadata;` moves to Phase 2.

### Confirmed: code-analyzer `mod.rs`-only structure vs multi-file structure

`code-analyzer` suggested collapsing all logic into `metadata/mod.rs`. The feature spec explicitly enumerates separate files (`db.rs`, `migrations.rs`, `models.rs`, `profile_sync.rs`). The multi-file structure is intentional for:

- Parallel implementation by separate implementors
- Clear authority boundaries (db.rs owns security, migrations.rs owns DDL, etc.)
- Phase 2/3 additions (`launcher_sync.rs`, `launch_history.rs`) drop in without touching existing files

**Resolution**: Multi-file structure from the spec is retained.

### Confirmed alignments

Both teammates confirmed:

- Sequential constraints match my DAG (Cargo.toml → module files → Tauri integration)
- `LaunchRequest.profile_name` is the Phase 2 blocker
- `profile/toml_store.rs` is not modified
- Best-effort cascade pattern applies to all metadata sync hooks

---

## Implementation Strategy Recommendations

### 1. Implement T4 (`models.rs`) as the First Concrete File

`MetadataError` is referenced by every other metadata module. Writing it first eliminates the forward-declaration coordination problem between T3 and T4. It is also the smallest task in batch 2 and sets the error enum pattern for the entire module.

### 2. Use `open_in_memory()` as the Testing Backbone

The in-memory constructor (`MetadataStore::open_in_memory()`) bypasses all filesystem concerns and makes every sync method independently testable without tempdir setup. All unit tests should use it; only permission and symlink tests need real paths.

### 3. Security Hardening Lives in T3, Not Separate Tasks

W1 (permissions), W4 (parameterized queries), W5 (symlink check), W7 (execute_batch literals) are all enforced at the connection factory level in `db.rs`. They are not separate tasks — they are the implementation of `open_connection()`. W2 (path sanitization) is T0. W3 (payload bounds) is Phase 2 (cache entries don't exist in Phase 1). W6 (re-validate stored paths before fs ops) applies to Phase 2 launcher sync.

### 4. `MetadataStore` Constructors Must Match the Three-Constructor Pattern

From `toml_store.rs:82-98`, the pattern is:

- `try_new() -> Result<Self, String>` — production (called from `lib.rs`)
- `new() -> Self` — panic wrapper (for tests that can't fail)
- `with_path(path: &Path) -> Result<Self, MetadataError>` — test injection

Additionally add `open_in_memory() -> Result<Self, MetadataError>` for unit tests and `disabled() -> Self` for the fail-soft construction path in `lib.rs`. Both must be implemented in T2 alongside `try_new()` — they are not separate tasks.

### 5. Gate Phase 2 on `LaunchRequest.profile_name` Addition

Phase 2 tasks (`record_launch_started`, `observe_launcher_exported`) require `profile_name: &str`. Verify `launch/request.rs:LaunchRequest` has this field before starting any Phase 2 work. Currently confirmed missing (line 16-37 of request.rs shows no `profile_name` field). This is the only blocking prerequisite for Phase 2 that lives outside the metadata module.

### 6. `StartupError` Extension in T9

`startup.rs` currently defines `StartupError` with only `Settings` and `Profiles` variants. T9 adds a `Metadata(MetadataError)` variant. This is a clean additive change — existing code using `?` on `SettingsStoreError` and `ProfileStoreError` is unaffected. The reconciliation call must be wrapped in best-effort (`if let Err(e) = ... { tracing::warn! }`) not propagated with `?` to ensure startup never blocks on SQLite failure.

### 7. Test Coverage Requirements for Phase 1 Completion

Per feature-spec success criteria, Phase 1 is complete when:

- Profile write/rename/delete operations create corresponding SQLite rows
- `sync_profiles_from_store` correctly reconciles TOML state to SQLite
- Startup reconciliation runs without panicking (including when SQLite is unavailable)
- File permissions on `metadata.db` are verified as `0o600`
- All SQL paths use parameterized queries (code review gate, no format! in SQL)

Run tests with: `cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core`
