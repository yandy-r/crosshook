# Practices Research: sqlite3-addition

## Executive Summary

CrossHook-core has a consistent, disciplined codebase. Every module follows the same Store pattern, custom error enum, TOML serialization, and synchronous filesystem operations. These patterns are strong guides for the metadata module design. The main KISS risk in the proposed design is table over-scope for v1 — six of the proposed nine tables belong in Phase 2/3. The build-vs-depend decision is straightforward: bundled rusqlite + uuid crate + hand-rolled migrations match the codebase's existing DIY preference. The metadata module should be a first-class store that fits the established patterns, not a new abstraction layer.

---

## Existing Reusable Code

| Module/Utility | Location | Purpose | How to Reuse |
|---|---|---|---|
| `Store::try_new() / with_base_path()` | `profile/toml_store.rs:83-98`, `settings/mod.rs:71-85`, `settings/recent.rs:67-79` | Constructor pattern for testable stores | Follow the same pattern: `MetadataStore::try_new() -> Result<Self, String>` + `with_path(path) -> Result<Self, MetadataError>` |
| `BaseDirs::new()` + `data_local_dir()` | `settings/recent.rs:69-73`, `logging.rs:108-110` | XDG-compliant path resolution to `~/.local/share/` | Use `data_local_dir().join("crosshook").join("metadata.db")` — same convention the log writer and recent-files store use |
| `BaseDirs::new()` error message convention | `profile/toml_store.rs:85-86`, all stores | Uniform `"home directory not found — CrossHook requires a user home directory"` message | Copy the exact message pattern; it already sets user expectation |
| `validate_name()` | `profile/toml_store.rs:300-325` | Path-safe name validation (blocks traversal, Windows reserved chars) | Do not re-implement. Call `validate_name()` before using any profile name as a SQLite parameter |
| `tracing::info!` / `warn!` / `error!` with key=value fields | `src-tauri/src/commands/profile.rs:33-38`, `logging.rs:70-77` | Structured logging conventions | Use the same `tracing::warn!(profile_name, "skipping ...")` syntax in metadata sync operations |
| `RotatingLogWriter` / `init_logging()` | `logging.rs` | Log file path is `data_local_dir()/crosshook/logs/crosshook.log` | No reuse needed, but confirms `data_local_dir()` is the correct base for `metadata.db` |
| Custom error enum with `Io { action, path, source }` variant | `community/taps.rs:49-62`, `logging.rs:22-29`, `community/index.rs:28-37` | Structured IO error pattern carrying context | Mirror this in `MetadataError::Io { action: &'static str, path: PathBuf, source: rusqlite::Error }` |
| `From<X> for XError` trait impls | Every error enum in the codebase | Error coercion with `?` operator | Add `impl From<rusqlite::Error> for MetadataError` |
| `DuplicateProfileResult` / `LauncherRenameResult` | `profile/toml_store.rs:68-74`, `export/launcher_store.rs:61-79` | Result structs that cross the IPC boundary with `#[derive(Serialize, Deserialize)]` | Mirror for `SyncReport` — it will cross IPC into Tauri commands |
| `sanitize_launcher_slug()` | `export/launcher.rs` (via `launcher_store.rs:123`) | Slug generation from display name | Reuse directly — the `launchers` table uses `current_slug`, which must match what `launcher_store.rs` generates |
| `derive_launcher_paths()` internal function | `export/launcher_store.rs:113-138` | Computes expected script/desktop paths from profile context | This function's output is exactly what the `launchers` metadata table stores. Read it for field-level alignment |
| `tempfile::tempdir()` in tests | All test modules | Isolated temp directory for test stores | Already a dev-dependency (`Cargo.toml:17`). Use in metadata integration tests |
| `#[serde(default)]` on data structs | `settings/mod.rs:20`, `community/taps.rs:20` | TOML/JSON tolerates missing fields | Use on `MetadataConfig` struct for forward-compat with new settings keys |
| `CommunityTapSyncResult` | `community/taps.rs:41-46` | Result of a tap sync with workspace + index | `sync_tap_index()` should accept a slice of these directly |
| `sanitize_display_path()` | `src-tauri/src/commands/launch.rs:301` | Replaces `$HOME` prefix with `~` before sending paths over IPC — currently a private function used in 8 places in that file | **Must be promoted to a shared utility** before the metadata module adds new IPC commands that return stored path strings (`current_toml_path`, `expected_script_path`, `expected_desktop_path`). If left private, every new metadata command must remember to apply it manually. Candidate location: `src-tauri/src/commands/shared.rs` (already hosts `create_log_path`). |
| `conn.pragma_update()` pattern | rusqlite API (no existing codebase call site) | `execute_batch()` cannot accept parameters — any PRAGMA requiring a runtime value (e.g. `PRAGMA user_version = ?`) must use `conn.pragma_update(None, "user_version", &version)` | Document as the canonical PRAGMA pattern in `metadata/db.rs` comments. Mixing `execute_batch()` and `pragma_update()` ad-hoc invites subtle bugs (silent no-op on parameterised `execute_batch` PRAGMA calls). |
| `validate_stored_path()` | To be created in `metadata/db.rs` or `metadata/models.rs` | Safety check when reading a stored path from SQLite and using it in a filesystem operation: must be absolute, no `..` components, resolves within an expected directory prefix | Complements `validate_name()` (which checks name strings) for the full-path case. Apply before any `fs::` call using a path retrieved from SQLite. Reuses the same invariant logic as `validate_name()` but for `Path` inputs. |

---

## Modularity Design

### Recommended Module Boundaries

```
src/metadata/
  mod.rs            — public API: MetadataStore struct, SyncReport, re-exports
  db.rs             — connection factory: open_at_path(), open_in_memory(), setup_pragmas()
  migrations.rs     — hand-rolled migration runner keyed on PRAGMA user_version
  models.rs         — SQLite-facing structs and enums (ProfileRow, LauncherRow, etc.)
  profile_sync.rs   — profile lifecycle reconciliation (observe, rename, delete)
  launcher_sync.rs  — launcher mapping and drift observation
  launch_history.rs — append-only launch operation recording
  community_index.rs— tap manifest indexing (Phase 3)
  cache_store.rs    — external metadata cache (Phase 3)
```

This mirrors the codebase's existing directory-per-concern pattern (`community/`, `export/`, `launch/`, `profile/`, `settings/`). Each subfile stays focused.

**What belongs in `mod.rs`**: `MetadataStore` struct definition, public API methods that delegate to submodules, and `SyncReport`. Keep `mod.rs` thin — it is a routing surface, not implementation.

**What belongs in `db.rs`**: only connection lifecycle: opening, PRAGMA setup, and the migration bootstrap call. Never SQL queries.

**What belongs in `migrations.rs`**: all DDL, version tracking via `PRAGMA user_version`, and migration functions indexed by version number.

### Shared vs. Feature-Specific

| Item | Shared | Feature-Specific |
|---|---|---|
| `MetadataStore` struct + `conn: Arc<Mutex<Connection>>` | Global — injected as Tauri state | — |
| `db.rs` PRAGMA setup | Shared — called for every connection | — |
| `migrations.rs` schema DDL | Shared — single migration runner | — |
| `models.rs` row types | Shared — used by all submodules | — |
| `profile_sync.rs` reconciliation | Profile module (called after TOML write/delete/rename) | — |
| `launcher_sync.rs` drift observation | Launcher export module | — |
| `launch_history.rs` event recording | Tauri launch commands + CLI | — |
| `SyncReport` / `SyncSource` enums | Shared — returned from all sync entry points | — |

### How Sync Hooks Connect to Existing Code

Do **not** put sync calls inside `ProfileStore` or `LauncherStore` directly. Those stores are file-only and should remain that way. Instead, call the metadata sync from the Tauri command layer after a successful TOML or filesystem operation:

- `commands/profile.rs`: after `store.save()`, `store.rename()`, `store.delete()` → call `metadata.observe_profile_write()`
- `commands/launch.rs`: before `command.spawn()` → call `metadata.record_launch_started()`, after join → call `metadata.record_launch_finished()`
- `commands/export.rs`: after `export_launchers()` → call `metadata.observe_launcher_exported()`

This keeps the core stores free of SQLite dependencies and preserves the fail-soft requirement: if metadata is unavailable, the Tauri command still proceeds with the TOML operation.

---

## KISS Assessment

| Area | Current Proposal | Simpler Alternative | Trade-off |
|---|---|---|---|
| `profile_file_snapshots` table | Separate table for observed file state per sync | Add `content_hash`, `mtime_unix` columns directly to `profiles` projection | Three columns vs. a join table. The full snapshots table only pays off if you need multi-generational history. For v1 identity matching, inline columns suffice. |
| `sync_runs` + `sync_issues` audit trail | Separate append-only tables recording every sync operation | Use `tracing::warn!` + existing log file for diagnostics | Audit tables add ~150 lines of schema/DDL and a reconciliation contract. The log file already captures this context. Add formal audit tables only after a user-surfaced debugging gap justifies it. |
| `external_cache_entries` table | Cache for ProtonDB, cover art, Steam catalog | Skip entirely in Phase 1 and Phase 2 | No current feature in the UI drives this table. It belongs exclusively in Phase 3 when the external fetch features are implemented. |
| `community_profiles` + `community_taps` tables | Index all tap manifests into SQLite | Keep the current in-memory index scan; only add SQLite index when search latency is observed | `CommunityProfileIndex` already scans and returns structured data. SQLite adds value only when the tap size grows large enough to feel slow. Belongs in Phase 3. |
| Derived projection tables (health/staleness) | Separate materialised projection rows | Compute on read with simple SQL queries (`MAX(started_at)`, `COUNT(*)`) | With <1000 launch operations per user, derived columns via inline queries are fast and eliminate a sync surface. Add materialized projections only when query profiling reveals a bottleneck. |
| `collections` / `collection_profiles` | Phase 1–2 curating tables | Defer to Phase 3 — no UI component exists yet | Building the schema before the UI is over-engineering. The stable `profile_id` is the only prerequisite; collections hang off it whenever needed. |
| `profile_preferences` | Separate one-to-one table | JSON column in `profiles` OR simple boolean columns (`is_favorite`, `is_pinned`) | For v1, a `is_favorite INTEGER NOT NULL DEFAULT 0` column on the `profiles` table is sufficient. Promote to separate table only if preference fields multiply. |

**Phase 1 table set (recommended)**: `profiles`, `profile_name_history`, two preference columns on `profiles`. That is it.

**Phase 2 table set**: `launchers`, `launch_operations`.

**Phase 3 table set**: `community_profiles`, `community_taps`, `external_cache_entries`, formal `sync_runs`/`sync_issues`.

---

## Abstraction vs. Repetition

| Decision | Recommendation |
|---|---|
| `SyncSource` enum | Extract — used in `profile_name_history.source` and `launch_operations` source tagging. Define once in `models.rs`. |
| Timestamp generation | Repeat — use `chrono::Utc::now().to_rfc3339()` inline. Three call sites do not justify a helper. |
| PRAGMA setup | Extract into `db::setup_pragmas(conn: &Connection)` — called once at open time, and in tests. |
| Path-to-string conversion for SQLite TEXT | Repeat — use `.to_string_lossy().to_string()` inline. No helper warranted. |
| Null/Option TEXT binding | Repeat — rusqlite handles `Option<String>` natively via `ToSql`. |
| `MetadataError` `map_err` in Tauri commands | Repeat as `map_err(\|e\| e.to_string())` — matches every other Tauri command in the codebase. |
| ID generation | Extract into `db::new_id() -> String` which wraps `uuid::Uuid::new_v4().to_string()` — called in profile_sync and launcher_sync at creation time. |

---

## Interface Design

### Public API Surface (`mod.rs`)

```rust
pub struct MetadataStore { /* conn: Arc<Mutex<Connection>> */ }

impl MetadataStore {
    pub fn try_new() -> Result<Self, String>         // Tauri startup, matches store convention
    pub fn with_path(path: &Path) -> Result<Self, MetadataError>  // test injection
    pub fn open_in_memory() -> Result<Self, MetadataError>         // unit tests

    // Phase 1
    pub fn observe_profile_write(&self, name: &str, profile: &GameProfile, path: &Path, source: SyncSource) -> Result<(), MetadataError>
    pub fn observe_profile_rename(&self, old_name: &str, new_name: &str, old_path: &Path, new_path: &Path) -> Result<(), MetadataError>
    pub fn observe_profile_delete(&self, name: &str) -> Result<(), MetadataError>
    pub fn sync_profiles_from_store(&self, store: &ProfileStore) -> Result<SyncReport, MetadataError>  // rebuild only

    // Phase 2
    pub fn record_launch_started(&self, profile_name: &str, method: &str) -> Result<String, MetadataError>  // returns operation_id
    pub fn record_launch_finished(&self, operation_id: &str, outcome: LaunchOutcome, exit_code: Option<i32>) -> Result<(), MetadataError>
    pub fn observe_launcher_exported(&self, profile_name: &str, slug: &str, script_path: &str, desktop_path: &str) -> Result<(), MetadataError>
    pub fn observe_launcher_scan(&self, profile_name: &str, info: &LauncherInfo) -> Result<(), MetadataError>
}
```

**Key design constraints derived from existing code:**

- `observe_profile_write` takes `profile_name: &str`, not a `profile_id` — the metadata layer resolves its own stable ID from the name.
- All methods return `Result<_, MetadataError>` not `String`. The Tauri command layer does `.map_err(|e| e.to_string())` as the boundary.
- `try_new()` returns `Result<Self, String>` to match every other store in the codebase.
- `MetadataStore` must be `Clone` to be placed in Tauri `.manage()`. Use `Arc<Mutex<Connection>>` internally.

### Fail-Soft Integration Pattern

In Tauri command layer:

```rust
// Profile save — proceed even if metadata fails
store.save(name, &profile).map_err(map_error)?;
if let Some(ref metadata) = state.metadata_store {
    if let Err(error) = metadata.observe_profile_write(name, &profile, &path, SyncSource::AppWrite) {
        tracing::warn!(%error, profile_name = name, "metadata sync failed after profile save");
    }
}
```

Using `Option<MetadataStore>` in the Tauri state avoids the overhead of defining a disabled wrapper. If `try_new()` fails at startup, log the error and proceed with `None`.

### Extension Points

- `SyncSource` enum (in `models.rs`): `AppWrite`, `FilesystemScan`, `TapSync`, `LaunchRuntime`, `CacheRefresh`, `Repair`
- `LaunchOutcome` enum: `Started`, `Succeeded`, `Failed`, `Canceled`
- `DriftState` enum (for launchers): `Aligned`, `Missing`, `Moved`, `Ambiguous`, `Stale`

---

## Testability Patterns

### Recommended Patterns

**In-memory SQLite for unit tests** (no filesystem, no tempdir):

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn observe_profile_write_creates_stable_id() {
        let store = MetadataStore::open_in_memory().unwrap();
        // ...
    }
}
```

This is the primary pattern for `profile_sync.rs`, `launcher_sync.rs`, `launch_history.rs` tests. All existing sync and history logic is deterministic and does not need the filesystem.

**Temp directory for integration tests** (matches existing `ProfileStore` test pattern):

```rust
#[test]
fn sync_profiles_round_trip() {
    let temp_dir = tempdir().unwrap();
    let metadata = MetadataStore::with_path(&temp_dir.path().join("metadata.db")).unwrap();
    let profiles = ProfileStore::with_base_path(temp_dir.path().join("profiles"));
    // ...
}
```

`tempfile` is already a dev-dependency in `Cargo.toml:17`.

**Existing tests to mirror**: `startup.rs` tests use a `store_pair()` helper that creates two isolated stores from a shared `tempdir`. Use the same approach for integration tests requiring both `ProfileStore` and `MetadataStore`.

### Anti-patterns to Avoid

- Do **not** create a separate `TestMetadataStore` type or mock. Use `open_in_memory()` instead.
- Do **not** share a single `MetadataStore` across test functions — each test should create its own `open_in_memory()` instance.
- Do **not** test through the Tauri command layer in unit tests — test `MetadataStore` methods directly.
- Do **not** assert on exact UUID values in tests — assert on behavior (row counts, field values, FK relationships).

---

## Build vs. Depend

| Need | Build Custom | Use Library | Recommendation | Rationale |
|---|---|---|---|---|
| SQLite access | `libsqlite3-sys` C bindings by hand | `rusqlite` | **Use rusqlite** | Idiomatic, well-maintained, matches synchronous codebase model |
| SQLite bundling for AppImage | — | `rusqlite` `bundled` feature (`libsqlite3-sys` compiles SQLite from source) | **`bundled` feature** | Avoids host SQLite version mismatches across distros. AppImage packaging benefits from determinism. The AppImage currently links everything; this is consistent. |
| Schema migrations | Hand-rolled with `PRAGMA user_version` | `rusqlite_migration 2.5.0` — lightweight, no external deps, uses `PRAGMA user_version`, no rollback support | **Lean hand-rolled for Phase 1** (zero-framework codebase, ~20 lines); accept `rusqlite_migration` if migration count grows past 5–6 entries | See note below table |
| Stable ID generation | Hand-rolled (sequential int, hash, etc.) | `uuid` crate (v4 random) | **`uuid` crate with `v4` feature** | `uuid::Uuid::new_v4().to_string()` is one line. The alternative is sequential integers (simpler but fragile for portable identity) or content-hash-based IDs (complex). Random UUIDs are collision-safe and do not require coordination. |
| UUID serialization | — | `uuid` crate `serde` feature | **Enable `uuid/serde`** | `profiles.profile_id` and `launchers.launcher_id` will flow through Tauri IPC. Serde support avoids manual `.to_string()` at the boundary. |
| ULID (sortable ID) | — | `ulid` crate | **Skip — use UUID v4** | ULIDs are sortable by creation time, but SQLite `created_at TEXT` (RFC 3339) already provides that ordering. A second ID dependency is not justified. |
| Timestamp formatting | — | `chrono` crate (already a dependency) | **Reuse `chrono`** | `chrono::Utc::now().to_rfc3339()` is already used across the codebase. Store all timestamps as ISO 8601 TEXT in SQLite. |
| JSON payload storage | — | `serde_json` crate (already a dependency) | **Reuse `serde_json`** | `external_cache_entries.payload_json` and any JSON blobs use `serde_json::to_string()` / `from_str()` inline. No rusqlite JSON feature needed. |
| Full-text search | Hand-rolled LIKE queries | SQLite FTS5 | **Defer — use LIKE for v1** | FTS5 availability depends on bundled SQLite build flags. Community manifest search is not user-blocking. Use `LIKE '%query%'` on `game_name` and `trainer_name` until query performance is an observed problem. |
| Connection pooling | — | `r2d2` + `r2d2-sqlite` | **Skip — use Mutex<Connection>** | The app is single-user, single-process. A single `Arc<Mutex<Connection>>` is correct for the synchronous, low-concurrency usage model. |
| Async SQLite | — | `sqlx`, `tokio-rusqlite` | **Skip — stay synchronous** | The technical research confirms current codebase is synchronous/file-based in core. `rusqlite` is the right fit. Tauri async commands can call synchronous core functions via `tauri::async_runtime::spawn_blocking` if needed. |

**Final `Cargo.toml` addition:**

```toml
[dependencies]
rusqlite = { version = "0.39", features = ["bundled"] }
uuid     = { version = "1",    features = ["v4", "serde"] }
```

No other new dependencies are required for Phase 1.

> **Note on `rusqlite_migration`**: The api-researcher recommends `rusqlite_migration 2.5.0` as a lightweight option that also uses `PRAGMA user_version` with no external dependencies. The trade-off: it adds a dependency for ~20 lines of logic the codebase could own directly, and it does not support down-migrations (rollback). Both approaches are valid; the choice is whether adding a crate for this is acceptable given the project's current zero-framework preference. If rollback is never needed and the migration count stays small, hand-rolled is simpler. If migrations grow complex, `rusqlite_migration` is worth the dependency cost.

---

## Open Questions

1. **`LaunchRequest` has no `profile_name` field** (`launch/request.rs:16-37`). The `record_launch_started()` API requires knowing which profile is being launched. Either add `profile_name: Option<String>` to `LaunchRequest`, or pass it as a separate parameter alongside the request in the Tauri command. Decide before hooking `commands/launch.rs`.

2. **`MetadataStore` in Tauri state: `Option<MetadataStore>` or always-present?** Using `Option<MetadataStore>` correctly models fail-soft but requires every Tauri command to handle `Option`. A dedicated `MetadataStore` with an internal `available: bool` flag is less correct but less ergonomically disruptive. Pick one convention before any integration work starts.

3. **Profile ID bootstrapping on first install**: when a user has 20 existing profiles and installs the version with SQLite for the first time, `sync_profiles_from_store()` must be called at startup. Where does this happen — in `src-tauri/src/lib.rs` `setup()` callback, or lazily on first metadata access? The startup callback is already used for auto-load-profile; bootstrap fits naturally there.

4. **`validate_name()` and SQLite TEXT**: profile names stored as TEXT in SQLite go through `validate_name()` before being used as filenames. Confirm this validation is applied at the metadata API boundary so SQL injection via profile name (e.g., a name containing `'` or `--`) is not possible. Use parametrized queries (`?` placeholders via rusqlite) — never string interpolation.

5. **`migrate at startup vs. open`: should `MetadataStore::try_new()` run migrations automatically, or should the caller explicitly call `migrate()`?** Running migrations in `try_new()` matches how `ProfileStore` auto-creates directories in `save()`. Explicit migration adds ceremony without benefit for a single-user app.
