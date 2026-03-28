# SQLite3 Addition — Analysis Context

Synthesized from: `shared.md`, `feature-spec.md`, `research-technical.md`, `research-security.md`, `research-practices.md`, `research-integration.md`, `research-recommendations.md`.

---

## Executive Summary

CrossHook adds SQLite as a secondary metadata store inside `crosshook-core/src/metadata/`. TOML profiles and filesystem artifacts remain canonical forever. SQLite owns stable UUIDs, rename history, launch event log, and derived projections. Sync hooks live **exclusively** in Tauri command handlers — `ProfileStore` stays a pure TOML I/O layer. `MetadataStore` carries an internal `available` flag; all methods no-op gracefully on failure. Phase 1 is intentionally minimal: 3 tables. Phases 2 and 3 add launchers, launch history, and community indexing.

---

## Architecture Context

### Authority Boundary (hard rule)

| Source          | Authoritative For                                                                                             |
| --------------- | ------------------------------------------------------------------------------------------------------------- |
| TOML/filesystem | `GameProfile` content, launcher scripts, tap workspaces, settings files                                       |
| SQLite          | Stable local UUIDs, favorites/pins, rename history, launch events, launcher-profile mappings, cache freshness |

### Sync Hook Placement

```
Tauri command handler
  → TOML/filesystem op  (critical, propagates error with ?)
  → metadata sync call  (best-effort, tracing::warn on failure — never blocks)
```

This is the existing `profile_rename` cascade pattern (`commands/profile.rs:149-194`) extended with one more best-effort step.

### Connection Model

`Arc<Mutex<Connection>>` — matches `RotatingLogWriter` (`logging.rs:118-120`). Single `MetadataStore` registered via `.manage()` in `lib.rs`. For Tauri async commands (`launch_game`/`launch_trainer`), metadata writes go through `tokio::task::spawn_blocking`.

---

## Critical Files Reference

| File                                                 | Role                                                                                                                                                               |
| ---------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| `crates/crosshook-core/src/profile/toml_store.rs`    | Three-constructor pattern template; `validate_name()`; NOT modified                                                                                                |
| `crates/crosshook-core/src/logging.rs`               | `Arc<Mutex<>>` precedent for `MetadataStore` connection wrapper                                                                                                    |
| `crates/crosshook-core/src/community/taps.rs`        | Structured error enum pattern template (`MetadataError` mirrors this)                                                                                              |
| `crates/crosshook-core/Cargo.toml`                   | Add: `rusqlite = { version = "0.39", features = ["bundled"] }` + `uuid = { version = "1", features = ["v4", "serde"] }`                                            |
| `crates/crosshook-core/src/lib.rs`                   | Add `pub mod metadata;` alongside existing modules                                                                                                                 |
| `src-tauri/src/lib.rs`                               | Initialize `MetadataStore`, `.manage()`, startup reconciliation                                                                                                    |
| `src-tauri/src/startup.rs`                           | Add `sync_profiles_from_store()` call at startup                                                                                                                   |
| `src-tauri/src/commands/profile.rs`                  | Phase 1 sync hooks: `profile_save`, `profile_delete`, `profile_rename`, `profile_duplicate`, `profile_import_legacy`                                               |
| `src-tauri/src/commands/launch.rs`                   | Phase 2: `record_launch_started()` + `record_launch_finished()`; also contains private `sanitize_display_path()` that **must be promoted** to `commands/shared.rs` |
| `src-tauri/src/commands/export.rs`                   | Phase 2 launcher sync hooks                                                                                                                                        |
| `src-tauri/src/commands/community.rs`                | Phase 3: `sync_tap_index()` after `tap_store.sync_many()`                                                                                                          |
| `src-tauri/src/commands/shared.rs`                   | Destination for promoted `sanitize_display_path()`                                                                                                                 |
| `crates/crosshook-core/src/launch/request.rs`        | `LaunchRequest` is **missing `profile_name` field** — Phase 2 blocker                                                                                              |
| `crates/crosshook-core/src/export/launcher_store.rs` | `LauncherInfo`, `sanitize_launcher_slug()`, `derive_launcher_paths()` — Phase 2 table alignment                                                                    |

### Files to Create (metadata module)

```
crates/crosshook-core/src/metadata/
  mod.rs             — MetadataStore struct, public API, SyncReport re-export
  db.rs              — open_at_path(), open_in_memory(), setup_pragmas(), new_id()
  migrations.rs      — DDL, PRAGMA user_version-based migration runner
  models.rs          — ProfileRow, LauncherRow, LaunchOperation, SyncReport, enums
  profile_sync.rs    — observe_profile_write/rename/delete, sync_profiles_from_store
  launcher_sync.rs   — observe_launcher_exported, observe_launcher_scan  (Phase 2)
  launch_history.rs  — record_launch_started/finished                    (Phase 2)
  community_index.rs — sync_tap_index                                    (Phase 3)
  cache_store.rs     — external_cache_entries with TTL                   (Phase 3)
src-tauri/src/commands/metadata.rs  — new Tauri commands for catalog queries, collections
src/types/metadata.ts               — TypeScript interfaces for IPC responses
src/hooks/useMetadata.ts            — React hook for metadata queries
```

---

## Patterns to Follow

### Three-Constructor Store Pattern

Every store: `try_new() -> Result<Self, String>` (Tauri startup), `with_path(path) -> Result<Self, MetadataError>` (test injection), `open_in_memory() -> Result<Self, MetadataError>` (unit tests). See `toml_store.rs:83-98`.

### Best-Effort Cascade

```rust
store.save(name, &profile).map_err(map_error)?;
if let Err(e) = metadata.observe_profile_write(name, &profile, &path, SyncSource::AppWrite) {
    tracing::warn!(%e, profile_name = name, "metadata sync failed after profile save");
    // no-op when metadata.available == false; always safe to call
}
```

`MetadataStore` is always present in Tauri state (never `Option`). If `try_new()` fails at startup, log and call `MetadataStore::disabled()` — returns a store where `available = false`. All methods are no-ops when `!available`. Do **not** call `process::exit(1)` on SQLite init failure.

### IPC Error Boundary

All Tauri commands return `Result<T, String>`. `MetadataError` variants are opaque at the IPC boundary. Never propagate raw `rusqlite::Error` to the frontend. Log full detail with `tracing::error!`.

### Structured Error Enum

Mirror `community/taps.rs:48-91`: `MetadataError::Io { action: &'static str, path: PathBuf, source }`, `Display` impl, `From<rusqlite::Error>`.

### UPSERT Idempotency

All sync entry points use `INSERT ... ON CONFLICT DO UPDATE` (SQLite ≥ 3.24.0). Reconciliation methods are transaction-backed and tagged with `SyncSource` enum.

### Module Structure

`mod.rs` is a routing surface only. `db.rs` owns only connection lifecycle (no SQL queries). `migrations.rs` owns all DDL. `models.rs` owns all row types.

---

## Cross-Cutting Concerns

### Security Requirements (W1-W8)

| #   | Concern              | Requirement                                                                                                                                |
| --- | -------------------- | ------------------------------------------------------------------------------------------------------------------------------------------ |
| W1  | File permissions     | `chmod 0600` on `metadata.db`, WAL, SHM immediately after `Connection::open()`; parent dir `0700`                                          |
| W2  | Path sanitization    | Promote `sanitize_display_path()` to `commands/shared.rs`; apply to **all** new SQLite-backed IPC paths before crossing the IPC boundary   |
| W3  | Payload bounds       | `external_cache_entries.payload_json` ≤ 512 KB; `launch_diagnostics.summary` ≤ 4 KB                                                        |
| W4  | SQL injection        | All SQL strings must be **string literals** — no `format!()` inside SQL. Always use `rusqlite::params![]`. Add as code review requirement. |
| W5  | Symlink attack       | Before opening DB: check `symlink_metadata()` — reject symlinks with actionable error                                                      |
| W6  | Path re-validation   | When reading stored paths for filesystem ops, re-apply `validate_name()` / path-safety check                                               |
| W7  | execute_batch safety | `execute_batch()` receives only hard-coded string literals. PRAGMAs with runtime values use `conn.pragma_update()`                         |
| W8  | Frontend XSS         | Community manifest fields rendered via JSX `{value}` interpolation only — never `dangerouslySetInnerHTML`                                  |

**Additional**: Never store raw CLI argument lists in `launch_operations` — store only structured fields (`method`, `game_path`, `trainer_path`, `exit_code`, `signal`, `failure_mode`).

### Fail-Soft Pattern

`MetadataStore` is always present in Tauri state (never `Option`). It carries an internal `available: bool` flag. `MetadataStore::disabled()` returns a no-op store used when init fails. All methods return early (`Ok(())`) when `!self.available`. Tauri state registration always succeeds; callers never check `Option`. Do not call `process::exit(1)` on SQLite init failure.

### Path Sanitization Promotion

`sanitize_display_path()` is currently private in `commands/launch.rs:301`. It must be promoted to `commands/shared.rs` before any Phase 2/3 metadata commands are added. This is a Phase 1 prerequisite, not optional.

### Connection Bootstrap (required PRAGMAs)

```sql
PRAGMA journal_mode=WAL;
PRAGMA foreign_keys=ON;
PRAGMA synchronous=NORMAL;
PRAGMA busy_timeout=5000;
PRAGMA secure_delete=ON;
```

Re-read each PRAGMA after setting to verify. All connections must go through `db::open_at_path()` — no raw `Connection::open()` elsewhere. Use `TransactionBehavior::Immediate` (`BEGIN IMMEDIATE`) for all write transactions.

### Startup Reconciliation

`sync_profiles_from_store()` must run at app startup (in `src-tauri/src/startup.rs`) to bootstrap first-run UUIDs and repair SQLite/TOML mismatches. First-run: create UUID per TOML file using `mtime` as `created_at`.

---

## Parallelization Opportunities

### Phase 1 batch ordering (confirmed by task-structurer)

| Batch                       | Tasks                                                                     | Notes                                                                                                                        |
| --------------------------- | ------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------- |
| T0 (prerequisite)           | Promote `sanitize_display_path()` to `commands/shared.rs`                 | Standalone; must land before any Tauri command modifications                                                                 |
| Batch 1                     | `Cargo.toml` deps + `lib.rs` `pub mod metadata;`                          | Unblocks all metadata files                                                                                                  |
| Batch 2 (sequential within) | **`models.rs` first** → then `db.rs` + `migrations.rs` in parallel        | `MetadataError` (in models.rs) is referenced by every other metadata file; must exist before db.rs and migrations.rs compile |
| Batch 3                     | `profile_sync.rs` — depends on models + db                                |                                                                                                                              |
| Batch 4                     | Tauri command hooks (`commands/profile.rs`) + `startup.rs` reconciliation |                                                                                                                              |

### Must be sequential (hard deps)

1. T0 (`sanitize_display_path` promotion) → before any Tauri command modifications
2. `Cargo.toml` + `lib.rs` → before any metadata module files
3. `models.rs` → before `db.rs`, `migrations.rs`, all sync submodules
4. `db.rs` + `migrations.rs` → before `profile_sync.rs`
5. `LaunchRequest.profile_name` field addition → before `launch_history.rs` Phase 2 integration
6. Phase 1 complete → before Phase 2 work starts

---

## Implementation Constraints

### Phase 1 Table Set (minimal — do this first)

- `profiles`: `profile_id` TEXT PK (UUID v4), `current_filename` TEXT UNIQUE, `current_path` TEXT, `game_name` TEXT, `launch_method` TEXT, `is_favorite` INTEGER DEFAULT 0, `is_pinned` INTEGER DEFAULT 0, `content_hash` TEXT, `source_profile_id` TEXT FK, `deleted_at` TEXT, `created_at` TEXT, `updated_at` TEXT
- `profile_name_history`: `id` INTEGER PK, `profile_id` TEXT FK, `old_name` TEXT, `new_name` TEXT, `old_path` TEXT, `new_path` TEXT, `source` TEXT, `created_at` TEXT

Cut from Phase 1: `sync_runs`/`sync_issues` (use `tracing::warn!`), `external_cache_entries` (Phase 3 only), derived projection tables (compute on read via SQL aggregates), `profile_file_snapshots` (inline `content_hash` on `profiles` instead), `collections`/`profile_preferences` as separate tables.

### Phase 2 Table Set

- `launchers`: composite PK `(profile_id, launcher_slug)`, `display_name`, `script_path`, `desktop_entry_path`, `drift_state`, `created_at`, `updated_at`
- `launch_operations`: `id` INTEGER PK, `profile_id` FK, `method`, `game_path`, `trainer_path`, `started_at`, `ended_at`, `exit_code`, `signal`, `outcome` (incomplete/succeeded/failed/abandoned), `diagnostic_json` (max 4 KB)

Blocker: `LaunchRequest.profile_name` field must be added before Phase 2 launch hooks work.

### Phase 3 Table Set

- `community_taps`: PK `(tap_url, tap_branch)`, `head_commit`, `last_synced_at`
- `community_profiles`: `tap_id` FK, `game_name`, `trainer_name`, `compatibility_rating`, `platform_tags_json`
- `external_cache_entries`: `cache_bucket`, `cache_key`, `payload_json` (max 512 KB), `fetched_at`, `expires_at`

### Async Bridge (Phase 2 new pattern)

`rusqlite::Connection` is `!Send`. Async Tauri commands must use `tokio::task::spawn_blocking` for all metadata writes. No existing codebase example — this is a new pattern.

### DB Location

`BaseDirs::data_local_dir().join("crosshook/metadata.db")` → `~/.local/share/crosshook/metadata.db`. Matches `CommunityTapStore` base path convention.

### Tauri Commands with Sync Hooks (Phase 1)

`profile_save`, `profile_save_launch_optimizations`, `profile_delete`, `profile_duplicate`, `profile_rename`, `profile_import_legacy`

### lib.rs MetadataStore init pattern

Unlike other stores (which call `process::exit(1)` on failure), `MetadataStore` must use fail-soft init:

```rust
let metadata_store = MetadataStore::try_new()
    .unwrap_or_else(|e| {
        tracing::warn!(%e, "metadata store unavailable — metadata features disabled");
        MetadataStore::disabled()
    });
```

`MetadataStore::disabled()` returns an always-no-op instance with `available = false`. Register via `.manage(metadata_store)` as usual — call sites need no guard.

### startup.rs reconciliation constraint

`StartupError` currently has 2 variants. Adding `Metadata(MetadataError)` is acceptable for the enum, but the reconciliation call in `run_startup()` must be **best-effort** — use `if let Err(e) { tracing::warn! }`, not `?`. A metadata sync failure must never block app startup.

### Open Decisions

- `LaunchRequest.profile_name`: add `Option<String>` field or pass as separate param to `record_launch_started()` — must decide before Phase 2
- CLI metadata sync: deferred to v2 (Tauri-only for now)
- Launcher drift repair: warning-only for v1

---

## Key Recommendations

1. **Start with `models.rs`, then `db.rs` + `migrations.rs`** — `MetadataError` (defined in models.rs) is referenced by every other metadata file; it must compile first. `db.rs` and `migrations.rs` can then be written in parallel.
2. **Promote `sanitize_display_path()` to `shared.rs` in Phase 1** — prevents security debt accumulating before Phase 2 commands are written.
3. **Use `MetadataStore::disabled()` for fail-soft init** — always register a `MetadataStore` in Tauri state; use an internal `available: bool` flag. Never use `Option<MetadataStore>` — confirmed by code-analyzer reviewing existing store patterns.
4. **Run migrations inside `try_new()`** — matches how `ProfileStore` auto-creates directories in `save()`. Explicit migration call adds ceremony without benefit.
5. **Use in-memory SQLite for unit tests** — `MetadataStore::open_in_memory()`. Do not mock the store. Do not share instances across test functions.
6. **Add `rusqlite_migration = "2.5"` only if migration count grows past 5-6 entries** — hand-rolled is simpler for Phase 1's 2-table schema.
7. **Never store raw CLI args in `launch_operations`** — this is both a security requirement and a privacy requirement.
8. **Startup reconciliation is a hard requirement**, not optional — existing users will have profiles with no SQLite identity on first install.
