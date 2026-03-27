# SQLite3 Addition - Recommendations

## Executive Summary

SQLite is worth adding to CrossHook, but only as a secondary metadata and intelligence layer with strict authority boundaries. The highest-confidence design is to make SQLite authoritative for stable IDs, relationships, history, projections, and caches while leaving TOML/filesystem artifacts authoritative for editable profile content and runtime state. The rollout should start with identity, lifecycle history, launcher mapping, and launch operations first; richer catalogs, usage insights, and external metadata caches should build on that foundation.

Codebase analysis confirms the existing architecture is well-suited for this addition: the store-based pattern (`ProfileStore`, `SettingsStore`, `RecentFilesStore`, `CommunityTapStore`) managed as Tauri state provides a natural integration point. The new `MetadataStore` should follow identical patterns -- concrete struct, eager initialization, `tauri::State` management -- but with fail-soft degradation when SQLite is unavailable.

**Critical architectural decision**: Metadata sync hooks belong in the **Tauri command layer**, not inside `ProfileStore` or `LauncherStore`. The existing Tauri commands already orchestrate multi-step flows (e.g., `profile_rename` does four steps: validate, rename TOML, rename launcher, update settings). Adding metadata sync as another step in this orchestration preserves `ProfileStore` as a clean TOML I/O layer with a single responsibility.

### Recommended Implementation Strategy

- Adopt SQLite for **identity + relationships + history**, not for replacing profile storage.
- Introduce stable `profile_id` and `launcher_id` immediately; almost every requested capability depends on decoupling identity from names/slugs/paths.
- Use append-only event tables plus mutable projection tables so the app can answer UI queries quickly while preserving auditability.
- Make reconciliation explicit and source-tagged. The system should always know whether a row came from a TOML write, a filesystem scan, a tap sync, or a launch event.
- Keep metadata features fail-soft: if SQLite is unavailable or needs repair, profile editing and launching still operate from files.
- Follow the existing error-handling pattern: custom `MetadataStoreError` enum with `Display`, `Error`, and `From` impls, consistent with `ProfileStoreError`, `LauncherStoreError`, `SettingsStoreError`, etc.
- **Do not modify `ProfileStore` internals** -- keep `toml_store.rs` as a pure TOML I/O layer. All metadata sync coordination happens at the Tauri command level.

### Phased Rollout Suggestion

1. **Phase 1 - Foundation**: database bootstrap, migrations, stable IDs, profile lifecycle sync, rename history, favorite/pinned flags, startup reconciliation scan. Include security hardening (file permissions, PRAGMA verification, parameterized queries, path sanitization, shared utility promotion) from the start. Minimal schema: `profiles` table + `profile_name_history` table + `is_favorite`/`is_pinned` columns on `profiles`. `RecentFilesStore` migration is a nice-to-have in Phase 1 but guaranteed in Phase 2 -- it is the lowest-risk migration but also the lowest-value item while Phase 1 already carries identity bootstrapping + rename cascade integration.
2. **Phase 2 - Operations History**: launcher mapping tables, launch operation/event logging, structured diagnostic index, derived health/state projections, launcher drift observations, abandoned launch-operation cleanup. Reuse existing `DiagnosticReport` and `FailureMode` types as serialized JSON payloads. **Prerequisite**: resolve that `LaunchRequest` has no `profile_name` field -- this must be threaded through before launch history can link to profile IDs. If `RecentFilesStore` migration did not land in Phase 1, it is the first task in Phase 2.
3. **Phase 3 - Catalog and Intelligence**: indexed community manifests, favorites/collections UX, usage insights, ProtonDB/cover-art/external metadata caches, optional FTS. Apply cache payload validation and size bounds. Do **not** create `collections.rs` or `cache.rs` stub files in Phase 1 -- defer file creation until the phase where those features are implemented.

### Quick Wins

- Stable profile IDs unlock rename-safe favorites, collections, recents, and usage history.
- Launcher mapping tables give CrossHook a durable answer to "which launcher belongs to which profile now?" -- replacing the current slug-comparison based orphan detection in `find_orphaned_launchers()`.
- Launch operation logging turns existing structured diagnostics into queryable local history with minimal conceptual change -- `DiagnosticReport` already has `severity`, `summary`, `pattern_matches`, and `analyzed_at` fields.
- Community manifest indexing avoids repeated recursive scans and creates a path toward fast local search -- the current `index_taps()` does a full directory walk on every call.
- `RecentFilesStore` migration is the simplest SQLite adoption target: proves the pattern, eliminates a separate TOML file, and produces immediate value.

### Future Enhancements

- Export/import of user collections and local annotations.
- Smarter launcher drift recovery based on watermark, slug history, and path similarity scoring.
- Local recommendation features such as "most launched this week" or "profiles with repeated Proton failures."
- Background cache refresh scheduling and prefetching for frequently used games.

### Risk Mitigations

- Enforce a documented authority matrix so new features cannot quietly shift canonical profile data into SQLite.
- Record immutable sync and rename events before updating current-state projections.
- Use transactions and UPSERT-based reconciliation for every sync pass.
- Add recovery commands that can rebuild projections from TOML, launcher scans, tap indexes, and cached sources.
- Start without FTS and advanced JSON querying unless query evidence shows the need.
- Apply path sanitization and input validation at the metadata module boundary (see Security Risks).
- Set `0o600` file permissions on `metadata.db` immediately after creation (see W1 in Security Risks).
- Use `rusqlite` 0.39.0 which bundles SQLite 3.51.3, resolving all known CVEs and the WAL corruption race.
- Use parameterized queries exclusively -- never `format!()` SQL strings with user data (see W4).
- Use `conn.pragma_update()` for any PRAGMA requiring a runtime value -- `execute_batch()` cannot accept parameters and silently no-ops on dynamic values (see W7).
- Add startup reconciliation scan comparing SQLite projections against TOML filenames to detect partial rename cascades.

### Decision Checklist

- Choose whether stable IDs remain local-only or are embedded into TOML for portability.
- Choose bundled vs system SQLite for AppImage predictability.
- Decide whether launcher drift repair should be conservative warning-only or permit high-confidence auto-relink.
- Decide whether collections/favorites stay purely local in v1 or need future export semantics.
- Decide first-run bootstrapping strategy (resolved: see Business Rule Resolutions below).
- Decide whether frontend transitions to ID-based profile references in Phase 1 or defers to Phase 2/3 (resolved: deferred, see RF-1).
- Choose UUID v4 vs ULID for stable ID format (see Technology Choices).

### Business Rule Resolutions

These resolutions were identified during cross-team analysis and inform the implementation approach:

**RF-1 (ID vs name transition)**: IDs are backend-only in Phase 1. The frontend continues to use `name: &str` throughout. No Tauri command signatures change in the first release. ID-based frontend references are deferred to Phase 2+.

**RF-2 (Launcher slug on rename)**: Existing behavior is the rule -- old launcher is deleted on rename (watermark-gated, best-effort), new export creates a new slug from the new display name. In SQLite: old `launcher_artifact` row is tombstoned at rename, new row is created on next explicit export. No in-place slug rename.

**RF-3 (First-run bootstrapping)**:
- Profiles: scan TOML directory, create identity rows with UUID + synthetic "created" event using file mtime as timestamp.
- Launchers: untracked external launchers are NOT retroactively mapped. They become tracked only after explicit re-export.
- Launch history: starts empty; no synthetic events.
- RecentFiles: migrate `recent.toml` into SQLite on first run, then delete the TOML file (no dual-write period).
- Failure mode: bootstrapping failure must not block app startup -- SQLite is additive, falls back to name-based operation.

**RF-4 (Community tap -- augment vs replace)**: SQLite augments, does not replace `index_taps()`. Git workspace scan remains source of truth. SQLite is a read cache: if HEAD unchanged, return cached index; if changed or absent, run scan and upsert. In-memory `CommunityProfileIndex` type continues to be used throughout the app.

**RF-5 (RecentFilesStore migration)**: In scope for the initial SQLite release. Simplest migration target, proves the pattern, eliminates a second TOML in the data directory. Confirms `metadata.db` placement at `~/.local/share/crosshook/metadata.db`.

---

## Implementation Recommendations

### Approach

**Recommended: Tauri-command-level sync with concrete MetadataStore**

The codebase already follows a consistent store pattern: `ProfileStore`, `SettingsStore`, `RecentFilesStore`, and `CommunityTapStore` are concrete structs with `try_new()`/`new()` constructors, managed as Tauri state via `.manage()`. The `MetadataStore` should follow this exact pattern:

```rust
// In crosshook-core/src/metadata/mod.rs
pub struct MetadataStore {
    connection: Arc<Mutex<rusqlite::Connection>>,
}

impl MetadataStore {
    pub fn try_new() -> Result<Self, MetadataStoreError> { ... }
}
```

The `Arc<Mutex<Connection>>` pattern is already established in the codebase (see `RotatingLogWriter` in `logging.rs` which uses `Arc<Mutex<RotatingLogState>>`). `MetadataStore` must be `Clone` for Tauri `.manage()`, which `Arc<Mutex<..>>` provides.

**Sync trigger placement**: Metadata sync hooks belong in the **Tauri command handlers** (`commands/profile.rs`, `commands/launch.rs`, `commands/export.rs`), **not** inside `ProfileStore` or `LauncherStore` internals. This preserves `ProfileStore` as a clean TOML I/O layer with a single responsibility. The Tauri command layer already orchestrates multi-step flows -- `profile_rename` already coordinates TOML rename, launcher rename, and settings updates. Adding metadata sync as another step in this orchestration is the natural fit.

**Fail-soft initialization**: Unlike other stores that `exit(1)` on failure, `MetadataStore` should log a warning and continue with `None` state, since metadata features are supplementary. Use `tauri::State<Option<MetadataStore>>` for type-level explicitness.

**Bootstrap timing**: The first-run profile census (`sync_profiles_from_store()`) should execute in the `setup()` callback in `src-tauri/src/lib.rs`, the same location as the existing auto-load-profile logic.

**Async bridge**: Some Tauri commands are `async fn` (e.g., `launch_game`). Since `rusqlite::Connection` is not `Send`, use `tokio::task::spawn_blocking` at the Tauri command boundary when calling metadata writes from async contexts. This is a standard Tokio pattern.

### Technology Choices

| Decision | Recommendation | Rationale |
| --- | --- | --- |
| Rust SQLite crate | `rusqlite` 0.39.0 with `bundled` feature | Fits synchronous `crosshook-core`; 40M+ downloads; bundles SQLite 3.51.3 which resolves all known CVEs and a WAL write+checkpoint data-corruption race (3.7.0–3.51.2); guarantees FTS5, JSONB across all distros |
| Connection model | `Arc<Mutex<Connection>>` singleton | Matches existing `logging.rs` pattern; WAL enables concurrent reads if needed later; single connection ensures PRAGMA enforcement |
| Migration strategy | Hand-rolled inline SQL + `PRAGMA user_version` | Simplest approach (~20 lines); no migration crate dependency; consistent with zero-framework codebase. `rusqlite_migration` 2.5.0 is a defensible alternative (~saves 20 lines, tested edge-case handling) but adds a dependency the codebase does not need at Phase 1 scope (2-3 migrations). **Explicit decision: hand-rolled.** |
| Error handling | Custom `MetadataStoreError` enum | Follows `ProfileStoreError`, `LauncherStoreError`, `SettingsStoreError` patterns exactly; use `From` impls for `rusqlite::Error` and `std::io::Error`; raw `rusqlite` errors must not escape the module boundary (A3) |
| ID generation | UUID v4 via `uuid` crate (`features = ["v4", "serde"]`) | Standard, well-understood; `created_at TEXT` (RFC 3339) provides sort order, making ULID time-ordering redundant; `uuid` crate is lightweight |
| State management | `tauri::State<Option<MetadataStore>>` | Fail-soft: `None` when SQLite init fails, checked at each call site; explicit in Rust's type system |
| DB location | `~/.local/share/crosshook/metadata.db` | Confirmed by existing `RecentFilesStore` and log writer both using `BaseDirs::data_local_dir()`; follows XDG conventions |
| Alternatives evaluated | sqlx (async-first, wrong fit), diesel (ORM overhead, no bundling), sea-orm (async), sled/redb (no relational queries) | All rejected; see Alternative Approaches section |

**New dependencies for Phase 1:**

```toml
rusqlite = { version = "0.39", features = ["bundled"] }
uuid     = { version = "1",    features = ["v4", "serde"] }
```

**Note**: `rusqlite` 0.39.0 bundles SQLite 3.51.3, which resolves all known CVEs (CVE-2025-3277, CVE-2025-6965) and a WAL write+checkpoint data-corruption race present in SQLite 3.7.0–3.51.2. No `libsqlite3-sys` version pinning is needed at this version.

**Build impact**: `rusqlite` with `bundled` compiles SQLite from C source, adding ~30 seconds to clean builds. Incremental builds are unaffected.

### Reusable Codebase Patterns

These existing patterns should be reused directly in the metadata module:

| Pattern | Source | Reuse in Metadata |
| --- | --- | --- |
| Store constructor: `try_new() -> Result<Self, Error>` + `with_path()` for tests | `ProfileStore`, `SettingsStore`, `RecentFilesStore` | `MetadataStore::try_new()` + `MetadataStore::with_path()` |
| XDG base: `BaseDirs::data_local_dir().join("crosshook")` | `RecentFilesStore`, `logging.rs` | DB path resolution |
| Name validation: `validate_name()` | `profile/toml_store.rs:300` | Validate profile names before SQL storage |
| Slug generation: `sanitize_launcher_slug()` | `export/launcher.rs` | Ensure metadata slugs match launcher_store output |
| Timestamps: `chrono` (already a dependency) | `Cargo.toml` | RFC 3339 timestamps for all event records |
| JSON serialization: `serde_json` (already a dependency) | `Cargo.toml` | DiagnosticReport JSON column storage |
| Test infrastructure: `tempfile::tempdir()` | dev-dependency | Integration tests with isolated DB files |
| Error enum: `enum XError { Io { action, path, source }, ... }` | Every core module | `MetadataStoreError` follows same structure |
| Mutex state: `Arc<Mutex<T>>` | `logging.rs:RotatingLogWriter` | `MetadataStore` connection wrapper |
| Path display sanitization: `sanitize_display_path()` | `commands/launch.rs:301` (private) | **Promote to `commands/shared.rs` before any metadata IPC commands land** -- currently private, called 8 times in one file; new metadata commands will need it for every stored path returned over IPC |

### Required New Shared Utilities (Priority Order)

These must be created as part of the metadata integration, not deferred:

| # | Utility | Location | Purpose |
| --- | --- | --- | --- |
| 1 | `sanitize_display_path()` promotion | Move from private in `commands/launch.rs` to `commands/shared.rs` | Prevent path leakage in new metadata IPC commands (W2). One-file move, no API change. **Must land before any metadata IPC commands.** |
| 2 | `open_metadata_connection()` factory | `metadata/db.rs` | Single connection factory with mandatory PRAGMAs. Doubles as PRAGMA enforcement point (A4). All code paths must go through this. |
| 3 | `conn.pragma_update()` pattern | Documented convention in `metadata/db.rs` | Prevents silent no-ops from `execute_batch()` with dynamic PRAGMA values (W7). |
| 4 | `validate_stored_path(path: &Path) -> Result<(), MetadataError>` | `metadata/db.rs` or `metadata/models.rs` | Validates SQLite-stored paths before filesystem operations: must be absolute, no `..` components, resolves within expected directory prefix. Full-path complement to `validate_name()` (which checks filename stems only). Apply at all SQLite-to-filesystem boundaries (W6). |
| 5 | `ensure_not_symlink(path: &Path)` | `metadata/db.rs` | Defensive check before `Connection::open()` (W5). |
| 6 | `MetadataStoreError` opaque boundary | `metadata/mod.rs` | Raw `rusqlite` errors logged internally via `tracing::error!`; only typed variants surface over IPC (A3). |
| 7 | Size limit constants | `metadata/models.rs` | `MAX_CACHE_PAYLOAD_BYTES = 512_000`, `MAX_DIAGNOSTIC_SUMMARY_BYTES = 4_096`. Enforced at `INSERT` boundaries (W3). |

### Phasing Verification

**Phase 1** is realistic as scoped. Key implementation observations:

- `ProfileStore::rename()` is a simple `fs::rename()` with no post-rename hooks -- but sync hooks go in the **Tauri command** `profile_rename`, not in `toml_store.rs`. The command already does a multi-step sequence (validate -> rename TOML -> rename launcher -> update settings).
- `ProfileStore::save()`, `delete()`, `duplicate()`, and `import_legacy()` all have clean return points. Their Tauri command wrappers are the natural place to append metadata sync calls.
- `SettingsStore.last_used_profile` stores profile name as string -- this continues working alongside SQLite IDs in Phase 1 with no migration needed.
- The Phase 1 schema should be **minimal**: `profiles` table + `profile_name_history` table + `is_favorite`/`is_pinned` columns on `profiles`. Cut `sync_runs`, `sync_issues`, `profile_file_snapshots` (as separate table), `collections`, and `external_cache_entries` from Phase 1.

**Phase 2** builds naturally on Phase 1 because:

- `DiagnosticReport` already derives `Serialize`/`Deserialize` and contains all fields needed for SQLite persistence: `severity`, `summary`, `exit_info`, `pattern_matches`, `suggestions`, `launch_method`, `log_tail_path`, `analyzed_at`. Store as JSON column.
- The launch commands in `commands/launch.rs` already construct `LaunchResult` with `helper_log_path` and `succeeded` -- adding metadata recording before the return is straightforward.
- Launcher drift detection currently uses slug-based comparison in `find_orphaned_launchers()` and staleness checking in `check_launcher_exists()` -- SQLite tables give these operations a durable backing store.
- **Prerequisite**: `LaunchRequest` currently has no `profile_name` field. This must be resolved (add the field or pass it alongside the request) before `record_launch_started()` can link launch events to profile IDs.

**Phase 3** has the most scope risk but is mitigated by:

- `CommunityTapSyncResult` already includes `head_commit` and a full `CommunityProfileIndex` -- upserting these into SQLite tables during `community_sync` is a data copy, not a new scan.
- SQLite augments `index_taps()` as a read cache with HEAD watermark; the git workspace scan remains source of truth (see RF-4).
- FTS should be deferred entirely unless search UX evidence demands it.

### Quick Wins (Ordered by Implementation Ease)

1. **MetadataStore bootstrap + migration framework** (~1 day): connection setup, PRAGMA configuration, `user_version` migration, `0o600` permissions, `PRAGMA quick_check` at startup.
2. **Profile identity table + sync from ProfileStore::list()** (~1 day): create `profiles` table, populate from existing TOML files on first run using file mtime for synthetic `created_at`.
3. **Rename history recording** (~0.5 day): append event from Tauri `profile_rename` command after successful TOML rename.
4. **RecentFilesStore migration** (~0.5 day): migrate `recent.toml` into SQLite, delete TOML after successful migration.
5. **Launcher mapping table** (~1 day): populate from `list_launchers()` scan, link to profile IDs. (Phase 2 start.)
6. **Launch operation recording** (~1 day): append-only insert from `launch_game`/`launch_trainer` commands. (Phase 2, requires profile_name on LaunchRequest.)

---

## Improvement Ideas

### Ideas from Cross-Team Analysis

1. **Keep `ProfileStore` pure**: Do not add metadata awareness to `toml_store.rs`. The Tauri command layer already orchestrates multi-step flows and is the correct place for metadata sync coordination. This preserves single responsibility and keeps `crosshook-core` stores testable without SQLite.

2. **Reuse `validate_name()` and `sanitize_launcher_slug()` for metadata input**: Both functions already exist and handle the exact sanitization needed for SQL parameter safety. Do not duplicate this logic.

3. **Store `DiagnosticReport` as JSON column**: Since `DiagnosticReport` already derives `Serialize`/`Deserialize`, launch history can store the full report as a JSON blob rather than decomposing every field into separate columns. This simplifies Phase 2 while keeping data queryable via SQLite JSON functions. Promote query-relevant fields (`severity`, `outcome`, `method`) to first-class columns; use JSON only for opaque diagnostic payloads.

4. **Migrate `RecentFilesStore` in Phase 1**: Current `recent.toml` is purely local metadata with no user-editing expectation. It is the simplest migration target, proves the SQLite pattern, and eliminates a separate TOML file in the data directory. Migration strategy: read existing `recent.toml`, insert into SQLite, delete TOML on success.

5. **Watermark-based launcher trust signal**: The existing `verify_crosshook_file()` function checks for `SCRIPT_WATERMARK` and `DESKTOP_ENTRY_WATERMARK` before deleting launchers. SQLite launcher records should store watermark verification status as a trust signal for drift detection confidence.

6. **First-run profile census with file mtime**: On first `MetadataStore` creation, scan profiles via `ProfileStore::list()` and use file mtime as the `created_at` timestamp for synthetic identity records. This provides meaningful sort order without fabricating events. Record as `source: "initial_census"`.

7. **Promote `sanitize_display_path()` to shared utility**: This function is currently private in `commands/launch.rs` and called 8 times in that one file. New metadata IPC commands will need it for every stored path returned over IPC. Promote to `commands/shared.rs` (which already hosts `create_log_path`). This is a one-file move with no API change and **must land before any metadata IPC commands**.

8. **Startup reconciliation scan (required)**: On every app startup, compare SQLite `profiles.current_name` projections against TOML filenames from `ProfileStore::list()`. Repair mismatches caused by partial rename cascades (e.g., TOML renamed but SQLite event not written due to crash). This is the safety net for the non-atomic 5-system rename cascade.

9. **Abandoned launch-operation sweep (required in Phase 2)**: On startup, mark any `launch_operations` rows with `outcome = 'started'` older than 24 hours as `outcome = 'abandoned'`. This prevents force-killed launches (common on Steam Deck -- user presses power button) from appearing as permanently "in progress" in the history UI.

10. **WAL checkpoint on clean shutdown**: Call `PRAGMA wal_checkpoint(TRUNCATE)` during app shutdown to fold committed WAL transactions into the main DB file. This prevents silent data loss if a user manually backs up only the `.db` file without the `-wal`/`-shm` sidecars.

11. **Competitive UX patterns**: Based on competitive analysis (Steam, Heroic, Playnite, Lutris), adopt these patterns for SQLite-backed features: two-tier loading (instant from SQLite, background filesystem/network refresh), inline status chips over blocking modals, optimistic updates for favorites/collections with visual rollback on failure, batch drift notifications (group simultaneous findings, never flood), explicit tombstone states for deleted profiles (avoid Playnite's "ghost profile" anti-pattern). Gamepad navigation must cover all new interactive SQLite-backed states -- this is a Steam Deck app.

12. **Unified store aggregate (deferred)**: Currently four stores are created independently in `lib.rs:run()`. A `CrossHookStores` aggregate struct could reduce `.manage()` calls and make fail-soft degradation cleaner. Not needed for v1 but useful if the store count keeps growing.

---

## Risk Assessment

### Technical Risks

| Risk | Likelihood | Impact | Mitigation |
| --- | --- | --- | --- |
| SQLite gradually becomes accidental source of truth for profile content | Medium | High | Codify authority matrix; keep file writes outside metadata module; code review gate; `ProfileStore` stays pure |
| External rename matching links the wrong profile history | Medium | High | Conservative matching: prefer creating new identity over wrong merge; use content hash + path similarity scoring; keep ambiguous cases unresolved |
| Rename cascade non-atomicity (now 5 systems: TOML, launcher files, settings TOML, RecentFiles, SQLite) | **High** | **High** | A partial rename (TOML renamed but SQLite event not written) is invisible to the user and silently corrupts launch history attribution. **Startup reconciliation scan is a required Phase 1 deliverable, not optional**: compare SQLite `profiles.current_name` against TOML filenames on every startup and repair mismatches. SQLite WAL journaling makes its own writes recoverable, but cross-system consistency requires explicit reconciliation. |
| AppImage SQLite feature mismatch | Low | Medium | Use `rusqlite` `bundled` feature for deterministic SQLite version; verify PRAGMA effectiveness at startup via `PRAGMA quick_check` |
| Metadata sync failures create stale projections | Medium | Medium | Log sync failures via `tracing::warn!`; support projection rebuilds from filesystem; never fail TOML operations due to sync errors |
| Event volume grows without pruning strategy | Low | Medium | Separate immutable events from compact projections; define retention policy before Phase 3 |
| `Arc<Mutex<Connection>>` contention under concurrent Tauri commands | Low | Low | Current command volume is low; WAL mode allows read concurrency; monitor before optimizing |
| First-run census creates noticeable startup delay with many profiles | Low | Low | Run census in `setup()` callback (same as auto-load-profile); acceptable for one-time operation |
| Profile name used as primary key across frontend/backend creates migration complexity | Medium | Medium | Phase 1 keeps name-based Tauri API; IDs are purely backend until frontend is ready to adopt (see RF-1) |
| Community tap HEAD tracking has a bug, users see stale catalog data with no indication | Low | Medium | SQLite augments but does not replace `index_taps()` (see RF-4); fallback to full scan always available |
| Launcher artifact orphaning after failed cleanup (files on disk, SQLite record tombstoned) | Low | Low | Existing `find_orphaned_launchers()` already handles this case; SQLite tombstones provide additional audit trail |
| `LaunchRequest` missing `profile_name` blocks Phase 2 launch history | High | Medium | Must resolve before Phase 2 implementation; add field to `LaunchRequest` or pass alongside |
| Incomplete `launch_operation` rows after force-kill (common on Steam Deck -- user presses power button) | Medium | Medium | Rows start as `outcome = 'started'` and update on terminal event. Force-kill leaves them as "in progress" forever. **Required Phase 2 deliverable**: startup sweep marks `started` rows older than 24 hours as `outcome = 'abandoned'` so history UI does not show stale in-progress entries. |
| First-run census failure (e.g., TOML directory permission denied) blocks app startup | Medium | High | **Required acceptance criterion**: if bootstrapping scan panics or errors, the app must start in degraded mode where SQLite features are disabled but TOML operations work normally. The "SQLite disabled" state is represented in `Option<MetadataStore>` (already `None` on failure). No sentinel row needed -- the `None` state is the signal. |
| `rusqlite` bundled feature adds ~30s to clean builds | Certain | Low | One-time cost per clean build; incremental builds unaffected; acceptable trade-off |

### Integration Challenges

- **Tauri command orchestration**: Metadata sync is another step in multi-step command flows. Must be non-blocking -- if SQLite write fails, the TOML operation must still succeed. Pattern: `if let Some(store) = metadata_store.as_ref() { if let Err(e) = store.sync_profile(...) { tracing::warn!(...) } }`.
- **Launcher sync must reconcile slug-derived paths with observed filesystem**: The current `derive_launcher_paths()` function builds paths from display name + steam app ID + trainer path. SQLite must store these derivation inputs alongside observed paths for reliable drift detection.
- **Community indexing must not mutate tap git workspaces**: Current `index_tap()` only reads; SQLite indexing must maintain this read-only relationship with tap workspaces.
- **Tauri and CLI launch paths must share one history API**: Both `src-tauri/commands/launch.rs` and `crosshook-cli/src/main.rs` consume `crosshook-core`. The `MetadataStore` API in `crosshook-core` is the shared surface. CLI metadata support should be implemented from Phase 1 but is exercised later.
- **Concurrent profile operations**: `save_launch_optimizations()` already notes that "concurrent `save` or `save_launch_optimizations` calls for the same profile are not synchronized" -- SQLite sync hooks inherit this limitation but UPSERT semantics make it safe.
- **Async Tauri commands**: `launch_game`/`launch_trainer` are `async fn`. Use `tokio::task::spawn_blocking` for metadata writes since `rusqlite::Connection` is not `Send`.
- **No existing test coverage for command cascade**: `profile_rename` and `profile_delete` integration tests exist at the Tauri command layer but not in `crosshook-core`. Metadata integration tests need their own harness using `MetadataStore::with_path()` + `tempdir()`.

### Security Risks

Severity levels follow the security research team's assessment. No CRITICAL findings -- the SQLite layer is local-only, single-user, with no external SQL injection surface.

#### Warnings (Must Address Before Shipping)

| ID | Severity | Risk | Mitigation |
| --- | --- | --- | --- |
| W1 | **HIGH** | World-readable DB permissions: SQLite creates files with umask-derived permissions (typically `0644`). The database contains launch history, trainer paths, usage patterns. | Call `set_permissions(0o600)` immediately after `Connection::open()`. WAL/SHM sidecars inherit parent permissions. **Easy fix, high impact.** |
| W2 | **HIGH** | Path leakage in new IPC commands: existing commands sanitize paths (`sanitize_display_path()` in launch.rs). New SQLite-backed commands will return stored path strings without sanitization unless explicitly handled. | Create shared `sanitize_path_for_display()` utility; apply to every path field in every IPC response. |
| W3 | **MEDIUM** | Unbounded payload sizes: `external_cache_entries.payload_json` and `launch_operations.diagnostic_summary` have no size limits. A large API response or malicious tap could bloat the database. | Enforce size limits before storage: 512 KB for cache payloads, 4 KB for diagnostic summaries. `MAX_LOG_TAIL_BYTES` (2 MB) already exists in diagnostics as a reference. |
| W4 | **MEDIUM** | SQL injection via dynamic query construction: `rusqlite` does not enforce parameterized queries at compile time. Developer discipline required. | Use `rusqlite`'s `params![]` macro exclusively; never use `format!()` in SQL strings. Add code review rule and consider CI lint. |
| W5 | **LOW** | Symlink attack on DB creation: symlink at the DB path before first run could cause SQLite to operate on an unintended file. | Check `symlink_metadata()` before `Connection::open()`. Lower priority -- requires attacker to have pre-existing write access to user home directory. |
| W6 | **MEDIUM** | Re-validate names/paths retrieved from SQLite before any filesystem operation: SQLite-stored paths could become stale or corrupted. | Create `validate_stored_path(path)` utility that checks: absolute, no `..` components, resolves within expected directory prefix. Apply at all SQLite-to-filesystem boundaries. Complements `validate_name()` (which validates filename stems only). |
| W7 | **MEDIUM** | `execute_batch()` accepts raw SQL with no parameter binding. Used for PRAGMA setup and migration blocks. If any contributor uses it with user-derived strings, it becomes a SQL injection vector. | Code review rule: `execute_batch()` must only receive hard-coded string literals. Use `conn.pragma_update(None, "key", &value)` for any PRAGMA requiring a runtime value (e.g., setting `user_version` during migrations). Document as required pattern in `metadata/db.rs`. |

**Path sanitization strategy**: Paths stored in SQLite follow two rules depending on usage:
- **Display-only paths** (log paths, summaries): sanitize at write time before storing, avoiding repeated sanitization on every read.
- **Filesystem-operation paths** (TOML path, script path): store raw; sanitize only when crossing the IPC boundary via the shared `sanitize_display_path()` utility.
- **`diagnostic_summary`**: store the sanitized form to avoid per-read sanitization overhead.

#### Advisories (Safe to Defer)

| ID | Risk | Note |
| --- | --- | --- |
| A1 | **Bundled SQLite CVE tracking**: Older `libsqlite3-sys` versions bundle SQLite with known CVEs (CVE-2025-3277 High, CVE-2025-6965 Medium) and a WAL write+checkpoint data-corruption race (3.7.0–3.51.2). | Using `rusqlite` 0.39.0 with `bundled` feature bundles SQLite 3.51.3, which resolves all known issues. Keep `rusqlite` updated and audit with `cargo audit`. |
| A2 | `PRAGMA secure_delete=ON`: deleted rows persist in free pages. | Minor concern for local single-user storage. Defer unless regulatory requirements change. |
| A3 | Error message leakage: raw `rusqlite` errors include SQL statement text and schema details. | Map to opaque `MetadataStoreError` enum at module boundary. Do not expose raw SQLite errors via IPC. |
| A4 | Single connection factory: required for PRAGMA enforcement. | Ensure all code paths go through `MetadataStore` -- no direct `Connection::open()` calls elsewhere. |
| A5 | Startup integrity check: detect corruption early. | Run `PRAGMA quick_check` at startup (not full `integrity_check`, which is slower). Consider running only on first open after a crash (detected via unclean WAL) rather than every launch for performance. |
| A6 | Community manifest field length bounds: string length limits on community tap data before indexing. | Enforce maximum lengths on `game_name`, `trainer_name`, etc. before SQLite insertion. |
| A7 | `synchronous=NORMAL` durability gap: with WAL + `synchronous=NORMAL`, the last committed transaction may not survive sudden power loss or OS crash (app crash is safe). | Acceptable for CrossHook's use case (cache/projection layer, not canonical data). Document so no one accidentally stores user-critical data that is not also written to TOML first. |
| A8 | FTS5 external-content trigger staleness: if triggers are bypassed (e.g., direct `DELETE` without FTS trigger firing), the FTS index silently diverges. | Fix is `INSERT INTO fts(fts) VALUES('rebuild')`, but requires detecting the divergence. Not a security risk; a data-integrity gotcha for Phase 3 FTS implementation. |

**Data sensitivity summary**: The database contains launch history (game names, timestamps, outcomes), trainer paths (may reveal pirated content), usage patterns, community tap URLs, and cached ProtonDB data. No credentials or secrets. Appropriate for local unencrypted storage with `0o600` file permissions. Local-only by design -- no external sync planned.

**Dependency posture**: `rusqlite` 0.39.0 is actively maintained with no known CVEs. It bundles SQLite 3.51.3, which resolves all known CVEs and the WAL corruption race. Bundled SQLite is strongly recommended over system SQLite for AppImage distribution -- especially for SteamOS users whose system SQLite may be older. The WAL corruption race in SQLite 3.7.0–3.51.2 (not a CVE, an upstream bug) is an additional strong argument for bundling.

### Practices Risks

| Risk | Mitigation |
| --- | --- |
| Over-engineering the metadata module with excessive abstraction layers (traits, generics, builders) | Follow existing concrete-struct pattern; no trait-based store abstraction until a second backend is needed |
| Duplicating error-handling boilerplate across metadata submodules | Create one `MetadataStoreError` enum covering all metadata operations; use `From` impls for `rusqlite::Error` and `std::io::Error` |
| Building custom migration framework when simpler approaches exist | Use `PRAGMA user_version` with sequential SQL blocks (~20 lines); avoid adding a migration crate dependency |
| Splitting the metadata module into too many files prematurely | Start with `mod.rs`, `db.rs`, and `models.rs`; split into `profile_sync.rs`, `launch_history.rs`, etc. only when files exceed ~300 lines |
| Adding FTS5 or complex JSON querying before proving the need | Defer FTS entirely; use simple `LIKE` queries and B-tree indexes for v1; measure before optimizing |
| Introducing async SQLite operations when the codebase is synchronous | Use `rusqlite` synchronously; use `spawn_blocking` at Tauri boundary only; do not add `tokio-rusqlite` |
| Including too many tables in Phase 1 schema | Minimal Phase 1: `profiles` + `profile_name_history` + `is_favorite`/`is_pinned` columns. Cut `sync_runs`, `sync_issues`, `profile_file_snapshots` (as separate table), `collections`, `external_cache_entries` |
| Adding materialized projection tables before scale justifies them | Compute derived state (health, staleness) on read with SQL aggregates; materialize only if query latency becomes a measured problem |

---

## Alternative Approaches

### 1. Full SQLite Replacement (Not Recommended)

Replace TOML profiles entirely with SQLite storage. This breaks the scriptability, git-friendliness, and manual editability that TOML provides. It also means CrossHook becomes useless without a working database. The codebase's `profile_to_shareable_toml()` function explicitly supports TOML export for sharing -- this workflow would be lost.

### 2. Key-Value Store (sled/redb) Instead of SQLite (Not Recommended)

Use an embedded key-value store for metadata. This loses relational queries (JOIN for launcher-profile relationships), standard tooling (sqlite3 CLI for debugging), and the rich SQL query surface for diagnostics/history. The additional complexity of managing relationships in application code outweighs the simpler storage model.

### 3. JSON Sidecar Files (Not Recommended)

Store metadata as JSON files alongside TOML profiles. This avoids the SQLite dependency but loses transactional consistency, efficient querying, and relational integrity. It also creates a file-management problem -- orphan JSON files, sync between JSON and TOML, no efficient indexing.

### 4. Hybrid: SQLite for History + TOML Sidecar for Preferences (Possible)

Use SQLite for append-only history/events and a TOML sidecar (e.g., `profile-meta.toml`) for user preferences like favorites and collections. This keeps preferences human-readable but splits authority across three storage systems (profile TOML, preference TOML, history SQLite). Not recommended for v1 due to complexity, but could be revisited if users strongly want editable preference files.

### 5. Deferred SQLite: Expand TOML First (Possible but Limited)

Add metadata fields directly to `GameProfile` TOML (e.g., `[metadata]` section with favorites, last_launched, etc.). This avoids a new dependency but cannot handle relational data (launcher mappings, cross-profile collections), append-only history, or efficient indexing. Suitable only for simple per-profile flags, not for the full feature scope described in the feature spec.

### 6. Alternative Rust SQLite Crates (Evaluated and Rejected)

| Crate | Verdict | Reason |
| --- | --- | --- |
| `sqlx` | Rejected | Async-first; forces async across all of `crosshook-core` (major architectural change); heavier dependency tree; no SQLite bundling |
| `diesel` | Rejected | Full ORM; complex setup; SQLite support via system `libsqlite3` (no easy bundling); schema migrations via external `diesel_cli` |
| `sea-orm` | Rejected | Async ORM; active-record pattern; same async-architecture-change issue as sqlx |

---

## Task Breakdown Preview

### Phase 1: Identity Foundation (Estimated: 7 tasks)

**Focus**: Bootstrap SQLite and make profile identity rename-safe.

**Security integration**: File permissions (`0o600`), PRAGMA verification, parameterized queries, path sanitization, and shared utility promotion are built into the bootstrap, not bolted on later.

**Practices integration**: Minimal schema; reuse existing patterns; no new abstractions; no stub files for Phase 2/3 features.

**Required acceptance criteria for Phase 1 completion**:
- Startup reconciliation scan detects and repairs partial rename cascades.
- First-run census failure does not block app startup -- `Option<MetadataStore>` is `None` and TOML operations work normally.
- All Tauri commands accepting profile names run `validate_name()` before any SQLite lookup -- SQLite lookups must never be the first line of defense against path traversal.

**Tasks**:

1. **Promote `sanitize_display_path()` + add `rusqlite`/`uuid` dependencies**
   - Move `sanitize_display_path()` from private in `commands/launch.rs` to `commands/shared.rs` (one-file move, no API change) -- **must land before any metadata IPC commands**
   - Add to `crosshook-core/Cargo.toml`: `rusqlite = { version = "0.39", features = ["bundled"] }`, `uuid = { version = "1", features = ["v4", "serde"] }`
   - Verify AppImage build succeeds with bundled SQLite (~30s clean build overhead)

2. **Create metadata module: `mod.rs` + `db.rs` (connection + migrations + PRAGMAs)**
   - `MetadataStore::try_new()` following `ProfileStore::try_new()` pattern
   - `MetadataStore::with_path()` for test injection (matching `ProfileStore::with_base_path()`)
   - `open_metadata_connection()` factory as the single connection entry point (A4)
   - `ensure_not_symlink()` check before `Connection::open()` (W5)
   - Enable `foreign_keys=ON`, `journal_mode=WAL` via `conn.pragma_update()` (W7 -- never `execute_batch()` for dynamic PRAGMAs)
   - Set `application_id` and `user_version` via `conn.pragma_update()`
   - Verify PRAGMA effectiveness after setting (SQLite silently ignores unknown PRAGMAs)
   - `set_permissions(0o600)` on `metadata.db` immediately after creation (W1)
   - `PRAGMA quick_check` at startup (A5)
   - Hand-rolled inline migration SQL blocks keyed to `user_version` (~20 lines)
   - Custom `MetadataStoreError` enum with `From<rusqlite::Error>` and `From<std::io::Error>`; raw `rusqlite` errors must not escape the module boundary (A3)
   - `execute_batch()` used only for hard-coded schema DDL string literals, never with user input (W7)

3. **Create metadata module: `models.rs` (schema + profile sync + reconciliation)**
   - Minimal Phase 1 schema: `profiles` table (with `is_favorite`, `is_pinned` columns) + `profile_name_history` table
   - Implement `sync_profiles_from_store()` with UPSERT semantics and parameterized queries only (W4)
   - Implement first-run census from `ProfileStore::list()` with file mtime as `created_at`
   - Implement **startup reconciliation scan**: compare `profiles.current_name` against TOML filenames; repair mismatches from partial rename cascades (required deliverable -- see risk table)
   - Validate all name inputs via `validate_name()` before any SQL storage or lookup
   - Create `validate_stored_path()` utility for SQLite-to-filesystem boundary checks (W6)
   - Define size limit constants: `MAX_CACHE_PAYLOAD_BYTES`, `MAX_DIAGNOSTIC_SUMMARY_BYTES` (W3 -- used in Phase 2/3)

4. **Register MetadataStore in Tauri state + hook profile commands**
   - Initialize as `Option<MetadataStore>` in `lib.rs:run()` (fail-soft: log warning, continue as `None`)
   - Run first-run census in `setup()` callback; **if census fails, app starts with `None` MetadataStore**
   - Run startup reconciliation scan after census (same `setup()` callback)
   - Update `commands/profile.rs`: `profile_save`, `profile_rename`, `profile_delete`, `profile_duplicate`, `profile_import_legacy` to optionally call metadata sync after successful TOML operations
   - **Critical**: every Tauri command accepting a profile name from the frontend must call `validate_name()` before any SQLite lookup -- this is a Phase 1 requirement, not a later hardening step
   - Non-blocking: log warnings on sync failure, never fail the TOML operation
   - Sanitize paths in any new IPC responses via promoted `sanitize_display_path()` (W2)

5. **Migrate `RecentFilesStore` into SQLite (nice-to-have in Phase 1; guaranteed in Phase 2)**
   - Read existing `recent.toml`, insert rows into SQLite
   - Delete `recent.toml` on successful migration (no dual-write period, per RF-3)
   - Add Tauri commands that read recent files from SQLite instead of TOML
   - Handle edge case: `RecentFilesStore` silently drops non-existent paths on load; decide whether SQLite version preserves stale paths with a "missing" flag or matches current drop behavior
   - **If Phase 1 scope is too large**: defer to first task of Phase 2 without user impact

6. **Add metadata module tests**
   - Unit tests for profile sync (create, rename, delete, duplicate) using `MetadataStore::with_path()` + `tempdir()`
   - Test fail-soft behavior when database is missing or corrupt
   - Test migration from version 0 to version 1
   - Test first-run census with empty profile directory and with existing profiles
   - Test startup reconciliation detects and repairs partial rename (TOML renamed, SQLite stale)
   - Test `0o600` permissions on created database file
   - Test that `validate_name()` is called before SQL operations

7. **WAL checkpoint on clean shutdown**
   - Call `PRAGMA wal_checkpoint(TRUNCATE)` during Tauri app shutdown to fold WAL into main DB
   - Prevents silent data loss if user manually backs up only the `.db` file without `-wal`/`-shm` sidecars

**Parallelization**: Task 1 (utility promotion + deps) can run in parallel with task 2 (db.rs) and task 3 (models). Task 4 depends on 1-3. Task 5 (RecentFiles) can run in parallel with task 4 once MetadataStore API exists, or slip to Phase 2. Tasks 6-7 run last.

### Phase 2: Operational History (Estimated: 5 tasks)

**Focus**: Persist launcher relationships and launch outcomes.

**Dependencies**: Phase 1 complete. **Critical prerequisite**: `LaunchRequest` must include `profile_name` (or equivalent profile identifier) before launch history can link to profile IDs.

**Tasks**:

1. **Resolve `LaunchRequest` profile identity gap**
   - Add `profile_name: String` field to `LaunchRequest` (or thread it alongside at the Tauri command level)
   - Ensure both Tauri and CLI launch paths provide this value

2. **Add launcher tables + sync logic**
   - Define `launchers` table with `launcher_id`, `profile_id` FK, `drift_state`, derivation inputs
   - Implement `sync_launcher_observations()` consuming `list_launchers()` output
   - Store watermark verification status as trust signal for drift confidence
   - On profile rename: tombstone old launcher row, new row created on next explicit export (per RF-2)

3. **Record launch operations**
   - Define `launch_operations` table with `operation_id`, `profile_id` FK, `method`, timestamps, `outcome`
   - Store `DiagnosticReport` as JSON column (leverage existing `Serialize` derive)
   - Promote `severity`, `outcome`, `method` to first-class columns for efficient queries
   - Validate and truncate diagnostic payloads before storage: 4 KB summary limit (W3)
   - Use `spawn_blocking` for metadata writes from async `launch_game`/`launch_trainer`

4. **Build derived projection queries**
   - Last success/failure per profile (SQL aggregate, not materialized)
   - Launcher drift detection backed by SQLite observations vs. current filesystem
   - Profile health state derived from recent launch history
   - Expose via new Tauri commands: `profile_launch_history`, `profile_health_summary`, `launcher_drift_check`

5. **Add Phase 2 tests**
   - Launcher sync and drift detection
   - Launch operation recording and query
   - Derived projection accuracy
   - Fail-soft behavior for all new paths

### Phase 3: Catalog and Intelligence (Estimated: 5 tasks)

**Focus**: Turn SQLite into a fast local intelligence layer.

**Dependencies**: Phase 2 complete.

**Tasks**:

1. **Index community tap manifests**
   - Define `community_taps` and `community_profiles` tables
   - Implement `sync_tap_index()` consuming `CommunityTapSyncResult`
   - Store `head_commit` as watermark; skip re-index when commit unchanged (per RF-4)
   - SQLite augments `index_taps()` as read cache; git workspace scan remains source of truth
   - Enforce field length bounds on community manifest data before insertion (A6)

2. **Add collections**
   - Define `collections` / `collection_profiles` tables
   - Implement CRUD operations backed by stable profile IDs
   - Ensure collections survive profile renames (the primary user value)

3. **Add usage insights**
   - Query `launch_operations` for usage counters, frequency, success rate
   - Build "most used this week", "recent failures", "never launched" views
   - Keep queries simple; avoid materialized views

4. **Add external metadata cache**
   - Define `external_cache_entries` table with freshness policy
   - Implement typed cache buckets (protondb, cover_art, steam_catalog)
   - Validate and bound cached payload sizes: 512 KB limit (W3)
   - Treat all cached external data as untrusted input (W3)
   - Support stale-while-revalidate pattern

5. **Optional: FTS5 for community search**
   - Only implement if B-tree indexes + `LIKE` queries prove insufficient
   - Use external-content FTS5 indexing canonical `community_profiles` rows
   - Gate behind a runtime feature check for bundled SQLite FTS5 support
   - Consider trigram tokenizer for fuzzy matching

---

## Key Decisions Needed

1. **Stable ID scope**: Local-only (recommended for v1) vs. embedded in TOML for portability.
2. **SQLite packaging**: Bundled via `rusqlite` `bundled` feature (recommended) vs. system SQLite.
3. **Launcher drift policy**: Warning-only with assisted relink (recommended) vs. auto-repair.
4. **ID format**: UUID v4 (recommended, simpler) vs. ULID (time-sortable, but `created_at` provides ordering).
5. **Collection portability**: Local-only in v1 or designed with future export/import semantics?
6. **RecentFilesStore migration behavior**: Match current "drop non-existent paths" behavior vs. preserve with "missing" flag?

## Open Questions

- Should the metadata module expose a `rebuild_from_filesystem()` command for manual recovery, and should it be accessible from the UI or CLI-only?
- What is the acceptable startup latency for first-run census with 50+ existing profiles?
- Should `MetadataStore` be passed to `crosshook-cli` as well from Phase 1, or is CLI metadata support deferred?
- How should the frontend display "metadata rebuilding" or "metadata unavailable" degraded states?
- Should community tap indexing in Phase 3 eventually replace the current `index_taps()` recursive scan entirely, or always keep both paths as fallback?
- ~~Should `PRAGMA wal_checkpoint(TRUNCATE)` be called on clean shutdown?~~ **Resolved**: Yes, included as Phase 1 Task 7.

---

## Research References

For detailed findings from each research dimension, see:

- [research-external.md](./research-external.md): SQLite capabilities, PRAGMAs, JSON/FTS, rusqlite evaluation, alternative crate analysis
- [research-business.md](./research-business.md): authority model, user stories, business rules, risk factor resolutions
- [research-technical.md](./research-technical.md): schema design, sync boundaries, API design, file-level impact
- [research-ux.md](./research-ux.md): user-facing implications, drift/history UX, accessibility, feedback states
- [research-security.md](./research-security.md): threat model, severity-leveled risks (W1-W7, A1-A8), data sensitivity, dependency posture
- [research-practices.md](./research-practices.md): reuse opportunities, KISS findings, build-vs-depend decisions, minimal schema guidance
- [feature-spec.md](./feature-spec.md): consolidated feature specification
