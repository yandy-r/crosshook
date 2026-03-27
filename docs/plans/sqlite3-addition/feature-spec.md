# Feature Spec: SQLite Metadata Layer

## Executive Summary

This feature adds SQLite to CrossHook as a secondary local metadata, catalog, cache, and product-intelligence store while keeping TOML profiles and filesystem artifacts canonical. The implementation uses `rusqlite` 0.39.0 (bundling SQLite 3.51.3) in `crosshook-core` with a new `metadata` module. The design introduces stable local UUIDs, relationship tables, append-only event history, and derived projections so CrossHook can preserve identity across renames, track launcher drift, record launch outcomes and diagnostics, and support favorites, collections, and usage insights. Metadata sync hooks live in the Tauri command layer (not inside `ProfileStore`) following the existing multi-step orchestration pattern. The main risks are authority confusion, rename cascade non-atomicity across five systems, and over-scoping the first release; these are mitigated by a strict source-of-truth matrix, mandatory startup reconciliation scans, a minimal Phase 1 schema (3 tables, not 14), and rebuildable event-plus-scan sync model. No critical security findings were identified; the database is local-only with no network attack surface.

## External Dependencies

### APIs and Services

#### SQLite

- **Documentation**: <https://www.sqlite.org/docs.html>
- **Authentication**: None
- **Key Capabilities**:
  - `PRAGMA foreign_keys=ON`: enforce relational integrity per connection (disabled by default; must set per connection)
  - `PRAGMA journal_mode=WAL`: concurrent readers with local writes; persistent once set
  - `PRAGMA synchronous=NORMAL`: safe and fast with WAL mode
  - `PRAGMA busy_timeout=5000`: wait-and-retry before `SQLITE_BUSY`
  - `PRAGMA application_id` / `PRAGMA user_version`: file format ownership and schema versioning
  - `PRAGMA secure_delete=ON`: zero-fill deleted data (defense-in-depth)
  - `PRAGMA optimize`, `foreign_key_check`, `integrity_check`, `quick_check`: maintenance and validation
  - `INSERT ... ON CONFLICT DO UPDATE`: idempotent sync/upsert workflow (requires SQLite >= 3.24.0)
  - `BEGIN IMMEDIATE`: required for write transactions to avoid `SQLITE_BUSY` upgrade races
- **Rate Limits**: none
- **Pricing**: public domain

#### SQLite JSON / FTS5

- **Documentation**: <https://www.sqlite.org/json1.html>, <https://www.sqlite.org/fts5.html>
- **Purpose**: structured cached payloads and optional local search over manifests/catalog data
- **JSON built-in** since SQLite 3.38.0; JSONB binary format since 3.45.0
- **FTS5** requires `SQLITE_ENABLE_FTS5` compile flag; included in `bundled` feature by default
- **Constraint**: FTS5 should be additive, not foundational, for v1; defer unless query performance demands it

### Libraries and SDKs

| Library          | Version     | Purpose                                 | Installation                            |
| ---------------- | ----------- | --------------------------------------- | --------------------------------------- |
| `rusqlite`       | **0.39.0**  | Primary Rust SQLite integration         | `cargo add rusqlite --features bundled` |
| `libsqlite3-sys` | transitive  | Bundled SQLite **3.51.3** bindings      | via `rusqlite`                          |
| `uuid`           | **1.x**     | Stable ID generation (UUID v4)          | `cargo add uuid --features v4,serde`    |
| `sqlite3` CLI    | system tool | Local inspection, migrations, debugging | distro package                          |

**New `Cargo.toml` additions for Phase 1:**

```toml
rusqlite = { version = "0.39", features = ["bundled"] }
uuid     = { version = "1",    features = ["v4", "serde"] }
```

### External Documentation

- [SQLite WAL](https://www.sqlite.org/wal.html): journaling, concurrency, sidecar file behavior
- [SQLite Foreign Keys](https://www.sqlite.org/foreignkeys.html): runtime enforcement and indexing rules
- [SQLite UPSERT](https://sqlite.org/lang_upsert.html): idempotent reconciliation behavior
- [SQLite JSON Functions](https://www.sqlite.org/json1.html): cache payload handling
- [SQLite PRAGMAs](https://www.sqlite.org/pragma.html): complete PRAGMA reference
- [rusqlite docs](https://docs.rs/rusqlite/latest/rusqlite/): Rust connection/transaction API
- [rusqlite feature flags](https://docs.rs/crate/rusqlite/latest/features): all 46 available features

## Business Requirements

### User Stories

**Primary User: CrossHook user with many evolving profiles**

- As a user, I want profile identity to survive filename renames so that favorites, collections, launcher mappings, and usage history remain attached.
- As a user, I want CrossHook to track exported launcher relationships separately from launcher names and slugs so that external launcher drift can be detected and repaired.
- As a user, I want launch attempts and failures recorded historically so that I can see what changed when a setup stops working.
- As a user, I want cached compatibility/catalog metadata available locally so that browsing stays fast after the first sync.
- As a user, I want recently used paths surfaced without relying on filesystem scans that silently filter out moved files.
- As a user, I want a profile duplication to be recognized as derived from its source rather than appearing wholly unrelated.

**Secondary User: CrossHook maintainer**

- As a maintainer, I want a durable local relationship layer so new product features do not depend on repeated recursive scans of TOML, launcher files, and community taps.
- As a maintainer, I want a clear authority split so CrossHook stays scriptable and debuggable while still gaining richer intelligence features.
- As a maintainer, I want community tap HEAD commits tracked so re-indexing can be skipped when nothing changed.

### Business Rules

1. **Canonical Profile Content**: TOML files remain authoritative for editable `GameProfile` content and shareable runtime configuration. SQLite never shadows TOML fields (`game`, `trainer`, `injection`, `steam`, `runtime`, `launch`).
2. **Stable ID Rule**: SQLite is authoritative for local stable UUIDs that decouple identity from filenames, display names, slugs, and paths. In v1, IDs are local-only and backend-only (not exposed to the frontend).
3. **Relationship Rule**: favorites, collections, launcher mappings, rename history, launch history, and cache ownership live in SQLite.
4. **Event Rule**: launch outcomes, sync operations, rename history, and drift observations are append-only events.
5. **Projection Rule**: health/staleness, most-used, last-success, launcher drift, and cache freshness are derived from events and scans via SQL aggregates (not materialized tables in v1).
6. **Rebuildability Rule**: SQLite projections must be reconstructable from TOML/filesystem/tap state plus retained event history.
7. **Fail-Soft Rule**: if SQLite is missing or corrupt, core profile editing and launching still work from canonical files, with metadata features degraded. `MetadataStore` is always present in Tauri state with an internal `available` flag; methods return early when disabled.
8. **Explainability Rule**: when SQLite-derived state disagrees with the filesystem, CrossHook explains which authority won and why.
9. **Profile Name Constraint Rule**: profile names follow `validate_name()` rules. SQLite identity rows enforce the same constraints.
10. **Launcher Watermark Rule**: SQLite must not record an artifact as "owned" if the watermark verification would fail at delete time.
11. **Rename Cascade Rule**: SQLite integrates into the existing cascade (TOML rename -> launcher cleanup -> display_name update -> settings update) as another best-effort step in the Tauri command. Failed SQLite writes do not block TOML operations.
12. **Delete Cascade Rule**: profile deletion soft-deletes the SQLite identity row (tombstone) for history preservation.
13. **Tap Sync Idempotency Rule**: SQLite tracks HEAD commit per tap; re-indexing skips taps where HEAD has not changed.

### Edge Cases

| Scenario                                           | Expected Behavior                                                                                                                      | Notes                            |
| -------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------- |
| User renames a profile TOML file outside CrossHook | Retroactive detection at next profile list open; disambiguate if multiple candidates match                                             | avoid silent wrong merges        |
| User renames launcher files outside CrossHook      | Keep historical launcher record, mark drift, surface repair/relink option                                                              | filesystem remains authoritative |
| Profile TOML deleted externally                    | SQLite record becomes tombstone displayed as "Removed from filesystem" with Delete/Restore/Archive actions; never shows as launchable  | anti-ghost-profile rule          |
| Profile TOML parse fails during scan               | Preserve profile identity/history rows, record sync issue, do not delete prior metadata                                                | important for recovery           |
| SQLite unavailable                                 | Launch/profile flows continue; metadata-driven features individually suppressed                                                        | fail-soft requirement            |
| SQLite file deleted or corrupt                     | Recreate database and rebuild from TOML/filesystem/tap state; event/history loss limited to SQLite-only data                           | rebuild runs in background       |
| Cached ProtonDB/art data expired                   | Use stale cache with freshness label; never block launch                                                                               | offline-first                    |
| First-run bootstrapping with existing profiles     | Census creates UUID per TOML file using file mtime as `created_at`; no prior history fabricated; launchers untracked until next export | see RF-3 in recommendations      |
| Force-kill during launch (Steam Deck power button) | `launch_operation` row left as `status = 'incomplete'`; startup sweep marks stale rows as `abandoned` after 24h                        | Phase 2 requirement              |

### Success Criteria

- [ ] Profile-related preferences/history survive renames without depending on profile filename stability.
- [ ] Launcher mappings persist separately from launcher slugs and can detect external drift.
- [ ] Launch operations, outcomes, timestamps, and diagnostic summaries are queryable locally.
- [ ] Community manifest browsing uses indexed local metadata instead of repeated recursive scans.
- [ ] The authority boundary between TOML/filesystem and SQLite is explicit in code and documentation.
- [ ] Startup reconciliation scan detects and repairs SQLite/TOML name mismatches.
- [ ] Metadata sync failure never blocks a TOML profile operation.

## Technical Specifications

### Architecture Overview

```text
                    canonical writes                    derived sync

ProfileStore/TOML -------------------------------> metadata::profile_sync
Launcher export + scan --------------------------> metadata::launcher_sync
Launch commands + diagnostics -------------------> metadata::launch_history
Community tap sync/index -----------------------> metadata::community_index
External metadata fetch ------------------------> metadata::cache_store

                     +-------------------------------+
                     | SQLite metadata database      |
                     | ~/.local/share/crosshook/     |
                     |   metadata.db                 |
                     |-------------------------------|
                     | stable UUIDs                  |
                     | relationships                 |
                     | event logs                    |
                     | SQL aggregate projections     |
                     | caches / indexes              |
                     +-------------------------------+

CRITICAL: Metadata sync hooks live in Tauri command handlers,
NOT inside ProfileStore or LauncherStore internals.
ProfileStore remains a pure TOML I/O layer.
```

### Data Models

#### Phase 1 Schema (Minimal)

##### `profiles`

| Field               | Type    | Constraints         | Description                           |
| ------------------- | ------- | ------------------- | ------------------------------------- |
| `profile_id`        | TEXT    | PK                  | UUID v4 for local identity            |
| `current_filename`  | TEXT    | NOT NULL UNIQUE     | Current profile TOML filename stem    |
| `current_path`      | TEXT    | NOT NULL            | Current canonical TOML path           |
| `game_name`         | TEXT    | NULL                | Extracted from `GameSection.name`     |
| `launch_method`     | TEXT    | NULL                | Extracted from `LaunchSection.method` |
| `content_hash`      | TEXT    | NULL                | Last observed normalized content hash |
| `is_favorite`       | INTEGER | NOT NULL DEFAULT 0  | User favorite flag                    |
| `is_pinned`         | INTEGER | NOT NULL DEFAULT 0  | User pinned flag                      |
| `source_profile_id` | TEXT    | NULL FK -> profiles | Duplication lineage                   |
| `deleted_at`        | TEXT    | NULL                | Soft-delete tombstone timestamp       |
| `created_at`        | TEXT    | NOT NULL            | First seen timestamp                  |
| `updated_at`        | TEXT    | NOT NULL            | Latest projection update              |

**Indexes:** `idx_profiles_current_filename` on (`current_filename`) UNIQUE

##### `profile_name_history`

| Field        | Type    | Constraints                 | Description                                                                  |
| ------------ | ------- | --------------------------- | ---------------------------------------------------------------------------- |
| `id`         | INTEGER | PK AUTOINCREMENT            | Row identity                                                                 |
| `profile_id` | TEXT    | FK -> `profiles.profile_id` | Stable profile owner                                                         |
| `old_name`   | TEXT    | NULL                        | Previous observed name                                                       |
| `new_name`   | TEXT    | NOT NULL                    | Current observed name                                                        |
| `old_path`   | TEXT    | NULL                        | Previous observed path                                                       |
| `new_path`   | TEXT    | NOT NULL                    | Current observed path                                                        |
| `source`     | TEXT    | NOT NULL                    | `app_rename`, `app_duplicate`, `filesystem_scan`, `import`, `initial_census` |
| `created_at` | TEXT    | NOT NULL                    | Event timestamp                                                              |

#### Phase 2 Schema (Additions)

##### `launchers`

| Field                | Type | Constraints                 | Description                                           |
| -------------------- | ---- | --------------------------- | ----------------------------------------------------- |
| `profile_id`         | TEXT | FK -> `profiles.profile_id` | Owning profile                                        |
| `launcher_slug`      | TEXT | NOT NULL                    | Current export slug (from `sanitize_launcher_slug()`) |
| `display_name`       | TEXT | NOT NULL                    | Latest expected launcher title                        |
| `script_path`        | TEXT | NULL                        | Expected script path                                  |
| `desktop_entry_path` | TEXT | NULL                        | Expected desktop file path                            |
| `drift_state`        | TEXT | NOT NULL DEFAULT 'unknown'  | `aligned`, `missing`, `moved`, `stale`, `unknown`     |
| `created_at`         | TEXT | NOT NULL                    | First export timestamp                                |
| `updated_at`         | TEXT | NOT NULL                    | Last observation refresh                              |

**Primary Key:** (`profile_id`, `launcher_slug`)

##### `launch_operations`

| Field             | Type    | Constraints                   | Description                                      |
| ----------------- | ------- | ----------------------------- | ------------------------------------------------ |
| `id`              | INTEGER | PK AUTOINCREMENT              | Row identity                                     |
| `profile_id`      | TEXT    | FK -> `profiles.profile_id`   | Target profile                                   |
| `method`          | TEXT    | NOT NULL                      | `steam_applaunch`, `proton_run`, `native`        |
| `game_path`       | TEXT    | NULL                          | Game executable path                             |
| `trainer_path`    | TEXT    | NULL                          | Trainer path                                     |
| `started_at`      | TEXT    | NOT NULL                      | Start timestamp                                  |
| `ended_at`        | TEXT    | NULL                          | End timestamp                                    |
| `outcome`         | TEXT    | NOT NULL DEFAULT 'incomplete' | `incomplete`, `succeeded`, `failed`, `abandoned` |
| `exit_code`       | INTEGER | NULL                          | Exit code                                        |
| `signal`          | INTEGER | NULL                          | Signal if present                                |
| `log_path`        | TEXT    | NULL                          | Referenced log path                              |
| `diagnostic_json` | TEXT    | NULL                          | Serialized `DiagnosticReport` (max 4 KB)         |
| `severity`        | TEXT    | NULL                          | Promoted from diagnostic for efficient query     |
| `failure_mode`    | TEXT    | NULL                          | Promoted from diagnostic for efficient query     |

#### Phase 3 Schema (Additions)

Tables: `community_taps`, `community_profiles`, `external_cache_entries`, `collections`, `collection_profiles`.

### API Design

#### `MetadataStore` Public API

```rust
pub struct MetadataStore { /* conn: Arc<Mutex<Connection>> */ }

impl MetadataStore {
    pub fn try_new() -> Result<Self, String>                      // Tauri startup
    pub fn with_path(path: &Path) -> Result<Self, MetadataError>  // test injection
    pub fn open_in_memory() -> Result<Self, MetadataError>        // unit tests

    // Phase 1
    pub fn observe_profile_write(&self, name: &str, profile: &GameProfile, path: &Path, source: SyncSource) -> Result<(), MetadataError>
    pub fn observe_profile_rename(&self, old_name: &str, new_name: &str, old_path: &Path, new_path: &Path) -> Result<(), MetadataError>
    pub fn observe_profile_delete(&self, name: &str) -> Result<(), MetadataError>
    pub fn sync_profiles_from_store(&self, store: &ProfileStore) -> Result<SyncReport, MetadataError>

    // Phase 2
    pub fn record_launch_started(&self, profile_name: &str, method: &str) -> Result<String, MetadataError>
    pub fn record_launch_finished(&self, operation_id: &str, outcome: LaunchOutcome, exit_code: Option<i32>) -> Result<(), MetadataError>
    pub fn observe_launcher_exported(&self, profile_name: &str, slug: &str, script_path: &str, desktop_path: &str) -> Result<(), MetadataError>
}
```

**Key design constraints:**

- Methods take `profile_name: &str` (not `profile_id`) — the metadata layer resolves its own stable ID from the name.
- `try_new()` returns `Result<Self, String>` matching every other store in the codebase.
- `MetadataStore` is `Clone` via `Arc<Mutex<Connection>>` for Tauri `.manage()`.
- All SQL uses parameterized queries exclusively — never `format!()` in SQL strings.

#### Fail-Soft Integration Pattern

```rust
// In Tauri command — proceed even if metadata fails
store.save(name, &profile).map_err(map_error)?;
// MetadataStore is always present; methods no-op internally when available=false
if let Err(error) = state.metadata_store.observe_profile_write(name, &profile, &path, SyncSource::AppWrite) {
    tracing::warn!(%error, profile_name = name, "metadata sync failed after profile save");
}
```

### System Integration

#### Files to Create

- `src/crosshook-native/crates/crosshook-core/src/metadata/mod.rs`: module root, `MetadataStore` struct, public API
- `src/crosshook-native/crates/crosshook-core/src/metadata/db.rs`: connection factory, PRAGMA setup, permission enforcement, symlink check
- `src/crosshook-native/crates/crosshook-core/src/metadata/migrations.rs`: schema DDL, `user_version`-based migration runner
- `src/crosshook-native/crates/crosshook-core/src/metadata/models.rs`: SQLite-facing structs (`ProfileRow`, `SyncReport`, `SyncSource`, `LaunchOutcome`, `MetadataError`)
- `src/crosshook-native/crates/crosshook-core/src/metadata/profile_sync.rs`: profile lifecycle reconciliation
- `src/crosshook-native/crates/crosshook-core/src/metadata/launcher_sync.rs`: launcher mapping and drift (Phase 2)
- `src/crosshook-native/crates/crosshook-core/src/metadata/launch_history.rs`: launch operation recording (Phase 2)
- `src/crosshook-native/crates/crosshook-core/src/metadata/community_index.rs`: tap/catalog indexing (Phase 3)
- `src/crosshook-native/crates/crosshook-core/src/metadata/cache_store.rs`: external metadata cache (Phase 3)

#### Files to Modify

- `crates/crosshook-core/Cargo.toml`: add `rusqlite` and `uuid` dependencies
- `crates/crosshook-core/src/lib.rs`: add `pub mod metadata;`
- `src-tauri/src/lib.rs`: initialize `MetadataStore` (always-present with internal `available` flag), add to `.manage()`, run census in `setup()`
- `src-tauri/src/commands/profile.rs`: add metadata sync calls after `profile_save`, `profile_rename`, `profile_delete`, `profile_duplicate`, `profile_import_legacy`
- `src-tauri/src/commands/launch.rs`: add `record_launch_started`/`record_launch_finished` (Phase 2)
- `src-tauri/src/commands/export.rs`: add metadata sync after launcher export/delete/rename (Phase 2)
- `src-tauri/src/commands/community.rs`: add `sync_tap_index()` after `community_sync()` (Phase 3)
- `src-tauri/src/startup.rs`: add startup reconciliation scan

**NOT modified:** `crates/crosshook-core/src/profile/toml_store.rs` — `ProfileStore` remains a pure TOML I/O layer.

#### Configuration

- `metadata.db_path`: `~/.local/share/crosshook/metadata.db` (via `BaseDirs::data_local_dir()`)
- `metadata.file_permissions`: `0o600` on DB file; `0o700` on parent directory
- `metadata.sqlite_foreign_keys`: enabled on every connection
- `metadata.sqlite_journal_mode`: `WAL` (persistent)
- `metadata.sqlite_synchronous`: `NORMAL`
- `metadata.sqlite_busy_timeout`: 5000ms
- `metadata.sqlite_application_id`: CrossHook-specific 32-bit integer
- `metadata.schema_user_version`: managed by hand-rolled migration runner

## UX Considerations

### User Workflows

#### Primary Workflow: Rename-Safe Profile Context

1. **Rename Profile**: user renames in CrossHook or externally.
2. **System**: preserves stable ID, updates current-name projection, appends rename history event. Launcher cleanup fires as existing behavior; outcome is now persisted.
3. **User sees**: "Renamed profile. History and launcher mappings were preserved."
4. **Ambiguous external rename**: lightweight disambiguation prompt at next profile list open: "Is this the same profile as _[old name]_?"

#### Error Recovery Workflow

1. **Error Occurs**: launcher or profile file changes outside CrossHook.
2. **User Sees**: drift/stale messaging — "Launcher moved or renamed outside CrossHook."
3. **Recovery**: one-click Re-link (high confidence) or explicit Rebuild/Dismiss choices. Never silent auto-repair.
4. **Batch drift**: grouped summary chip ("3 launchers need attention") — never individual toasts.

#### Corrupt DB Recovery

1. Fall back to TOML-only mode for core operations (launch, edit).
2. SQLite-dependent features (history, favorites, collections, drift) individually suppressed.
3. Background rebuild from TOML/filesystem; surface "Rebuilding metadata cache..." in status bar.

### UI Patterns

| Component         | Pattern                               | Notes                                                |
| ----------------- | ------------------------------------- | ---------------------------------------------------- |
| Profile details   | current-state plus expandable history | keeps main editing view focused                      |
| Launcher status   | drift badge + repair action           | explain authority clearly; require user confirmation |
| Cached metadata   | freshness timestamp + stale state     | never blocks primary workflows                       |
| Community browser | fast indexed local search             | SQLite-backed but invisible to users                 |
| Status chips      | icon + color + text label             | never color alone; meet WCAG AA contrast (4.5:1)     |
| Launch history    | card summary + expandable panel       | not tooltip (gamepad incompatible)                   |

### Accessibility Requirements

- Statuses (stale, drifted, cached, failed) include text labels and icons, not color alone.
- History timelines expose timestamps and outcomes in screen-reader-friendly text.
- All new interactive elements meet 44x44px minimum touch target for Steam Deck touchscreen.
- Inline prompts (disambiguation, batch drift) wired into `useGamepadNav` spatial navigation graph.
- Undo toasts reachable via controller (dedicated button binding or spatial focus).

### Performance UX

- **Two-tier loading**: instant from SQLite projections, then async filesystem/network refresh.
- **Optimistic Updates**: favorites, collections, local annotations update SQLite immediately.
- **Error Feedback**: if metadata rebuild needed, show "Rebuilding local metadata" — never imply data loss.
- **Debouncing**: filesystem scan results debounced (500ms) before UI update to avoid flicker.

## Security Considerations

### Findings by Severity

#### Critical -- Hard Stops

No critical findings identified. The database is local-only, single-user, stores no credentials or secrets, and `rusqlite`'s parameterized query API prevents injection when used correctly.

#### Warnings -- Must Address

| ID  | Finding                                                   | Risk                                                                                   | Mitigation                                                                                          |
| --- | --------------------------------------------------------- | -------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------- |
| W1  | World-readable DB permissions (`0644` default)            | Aggregated launch history, trainer paths, usage patterns readable by any local process | `chmod 0600` on DB file immediately after creation; parent directory `0700`                         |
| W2  | Inconsistent path sanitization in new IPC commands        | Full home directory paths leak to frontend                                             | Create shared `sanitize_path_for_display()` utility; apply to every IPC path field                  |
| W3  | Unbounded cached payload sizes                            | Memory pressure, unbounded disk growth from malformed external data                    | Enforce limits: 512 KB per cache entry, 4 KB per diagnostic summary                                 |
| W4  | SQL injection via dynamic query construction              | `format!()` in SQL strings creates injection vectors                                   | All SQL strings must be literals; use `params![]` exclusively; code review rule                     |
| W5  | Symlink attack on DB creation                             | Corrupt unrelated file or read attacker-controlled data                                | Check `symlink_metadata()` before `Connection::open()`                                              |
| W6  | Stored paths used in filesystem ops without re-validation | Path traversal from corrupted/tampered DB                                              | Re-apply `validate_name()` / path-safety checks on SQLite-sourced paths before fs ops               |
| W7  | `execute_batch()` with non-literal strings                | SQL injection; `execute_batch()` has no parameter binding                              | Only hard-coded string literals in `execute_batch()`; use `conn.pragma_update()` for runtime values |
| W8  | Community tap fields rendered in React WebView            | XSS if `dangerouslySetInnerHTML` used for untrusted manifest data                      | Audit React components; use `{value}` JSX interpolation exclusively                                 |

#### Advisories -- Best Practices

- **A1**: Track bundled SQLite version; use `rusqlite` 0.39.0+ (SQLite 3.51.3, resolves WAL-reset bug + CVEs)
- **A2**: Enable `PRAGMA secure_delete=ON` for defense-in-depth on deleted rows
- **A3**: Map `rusqlite` errors to opaque `MetadataError` enum at module boundary; never expose raw SQL errors via IPC
- **A4**: All connection opens go through single `open_connection()` factory for PRAGMA enforcement
- **A5**: Run `PRAGMA quick_check` at startup; trigger rebuild path on failure
- **A6**: Validate string lengths before inserting community manifest rows (game_name <= 512B, description <= 4KB)

### Dependency Security

- `rusqlite` 0.39.0 bundles SQLite 3.51.3 — resolves all known CVEs (CVE-2025-3277, CVE-2025-6965, CVE-2025-7458, CVE-2025-7709) and the WAL write+checkpoint data-corruption race (3.7.0-3.51.2).
- `libsqlite3-sys` 0.37.0: zero known CVEs (Meterian score: 100/100).
- `bundled` feature is required for AppImage to avoid host SQLite version mismatches (SteamOS may ship affected versions).
- Supply chain: MIT-licensed, actively maintained, bundles auditable upstream SQLite amalgamation source.

### Data Sensitivity

| Data                                  | Sensitivity | Note                                                             |
| ------------------------------------- | ----------- | ---------------------------------------------------------------- |
| Launch history (timestamps, outcomes) | Medium-High | Reveals which games/trainers user runs and when                  |
| Trainer/game paths                    | Medium      | May reveal install locations or content provenance               |
| Tombstone records                     | Medium      | History survives profile deletion; never store raw CLI arguments |
| Community manifest cache              | Low-Medium  | Public data from git repositories                                |
| Profile preferences                   | Low         | Favorites, pins                                                  |

### Secure Coding Guidelines

1. **Parameterized queries only** -- never `format!()` in SQL strings
2. **Single connection factory** -- all opens via `open_connection()` with PRAGMA setup
3. **File permissions** -- `0o600` on DB, WAL, SHM; `0o700` on parent dir
4. **Error opacity** -- `MetadataError` enum with opaque variants; log full detail via `tracing::error!`
5. **Path sanitization** -- shared utility for all IPC responses; `$HOME` -> `~`
6. **Payload bounds** -- enforce before storage; reject oversized payloads
7. **Re-validate stored paths** -- before filesystem operations, never assume stored data is safe
8. **No raw arguments in history** -- store only structured diagnostic fields, never CLI argument lists

## Recommendations

### Implementation Approach

**Recommended Strategy**: make SQLite authoritative for local identity, relationships, history, and caches while preserving TOML/filesystem authority for profile content and runtime artifacts.

**Critical architectural decision**: metadata sync hooks in Tauri command handlers, NOT inside `ProfileStore`.

**Chosen defaults for this spec**:

- **Stable IDs**: UUID v4, local-only in SQLite for v1; backend-only (frontend continues using profile names)
- **SQLite packaging**: bundled via `rusqlite` 0.39.0 (`bundled` feature); live database at `~/.local/share/crosshook/metadata.db`
- **Launcher drift recovery**: warning-only detection plus one-click assisted relink; no silent auto-repair
- **Migration strategy**: hand-rolled inline SQL + `PRAGMA user_version` (~20 lines)
- **Connection model**: `Arc<Mutex<Connection>>` singleton (matches `logging.rs` pattern)
- **State management**: always-present `MetadataStore` with internal `available: bool` flag; methods no-op when disabled

**Phasing:**

1. **Phase 1 - Foundation**: stable IDs, migrations, profile lifecycle sync, rename history, favorites/preferences, startup reconciliation, security hardening. Minimal schema: `profiles` + `profile_name_history`.
2. **Phase 2 - Core Features**: launcher mappings, launch operation history, diagnostic summaries, derived health/drift projections. Prerequisite: add `profile_name` to `LaunchRequest`.
3. **Phase 3 - Polish**: community catalog indexing, collections, usage insights, external metadata caches, optional FTS.

### Technology Decisions

| Decision              | Recommendation                                                                        | Rationale                                                                                     |
| --------------------- | ------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------- |
| Rust SQLite crate     | `rusqlite` 0.39.0 with `bundled`                                                      | synchronous fit; 40M+ downloads; bundles SQLite 3.51.3 resolving all CVEs + WAL race          |
| ID generation         | UUID v4 via `uuid` crate                                                              | standard; `created_at` provides sort order, making ULID redundant                             |
| Migration strategy    | Hand-rolled + `PRAGMA user_version`                                                   | ~20 lines; zero-framework codebase preference; `rusqlite_migration` acceptable if count grows |
| Connection model      | `Arc<Mutex<Connection>>` singleton                                                    | matches existing `RotatingLogWriter` pattern; WAL for concurrent reads                        |
| DB authority          | relationships/history only                                                            | preserves TOML scriptability and debuggability                                                |
| Sync model            | event + scan hybrid                                                                   | supports both app-driven writes and external file changes                                     |
| Launcher drift repair | warning-only + one-click assisted relink                                              | avoids wrong silent repair while reducing user friction                                       |
| FTS                   | defer unless proven necessary                                                         | avoids over-engineering; `LIKE` queries sufficient for v1                                     |
| Alternatives rejected | sqlx (async-first), diesel (ORM overhead), sea-orm (async), sled/redb (no relational) | all incompatible with synchronous `crosshook-core` architecture                               |

### Quick Wins

- Stable UUIDs for profiles enable rename-safe favorites, collections, recents, and usage history
- Rename history and current-name projection tables
- Launch operation persistence using existing `DiagnosticReport` serialization
- Indexed community manifest tables for faster local browsing (Phase 3)
- `RecentFilesStore` migration as simplest SQLite adoption proof-of-concept

### Future Enhancements

- Exportable collections and annotations
- Auto-relink heuristics for externally renamed launchers
- Usage-based recommendations ("most launched this week", "profiles with repeated failures")
- Scheduled cache refresh and stale-data policies
- Cross-machine profile portability via embedded IDs in TOML

## Risk Assessment

### Technical Risks

| Risk                                                                                | Likelihood | Impact | Mitigation                                                                             |
| ----------------------------------------------------------------------------------- | ---------- | ------ | -------------------------------------------------------------------------------------- |
| Rename cascade non-atomicity (5 systems: TOML, launchers, settings, recent, SQLite) | High       | High   | Mandatory startup reconciliation scan comparing SQLite vs TOML filenames               |
| SQLite gradually becomes accidental source of truth for profile content             | Medium     | High   | codify authority matrix; `ProfileStore` stays pure TOML I/O                            |
| External rename matching links the wrong profile history                            | Medium     | High   | conservative confidence thresholds; create new identity over wrong merge               |
| `LaunchRequest` missing `profile_name` blocks Phase 2 launch history                | High       | Medium | must resolve before Phase 2; add field or pass alongside                               |
| Incomplete `launch_operation` rows after force-kill (Steam Deck)                    | Medium     | Medium | startup sweep marks `started` rows > 24h old as `abandoned`                            |
| AppImage SQLite feature mismatch                                                    | Low        | Medium | `bundled` feature for deterministic behavior                                           |
| Metadata sync failures create stale projections                                     | Medium     | Medium | log sync failures; support projection rebuilds; never fail TOML ops                    |
| Event volume grows without pruning                                                  | Low        | Medium | separate events from projections; define retention before Phase 3                      |
| First-run census failure blocks startup                                             | Medium     | High   | `MetadataStore.available = false` ensures degraded mode; no hard requirement on SQLite |

### Integration Challenges

- Tauri command orchestration: metadata sync is best-effort in multi-step flows; must never block TOML operations.
- Launcher sync must reconcile `derive_launcher_paths()` derivation inputs with observed filesystem state.
- Community indexing must not mutate tap git workspaces; SQLite augments but does not replace `index_taps()`.
- Tauri and CLI launch paths must share one `MetadataStore` API in `crosshook-core`.
- Async Tauri commands need `spawn_blocking` for `rusqlite` calls since `Connection` is not `Send`.

## Task Breakdown Preview

### Phase 1: Identity Foundation

**Focus**: bootstrap SQLite and make profile identity rename-safe.

**Tasks**:

1. Add `rusqlite` + `uuid` dependencies to `Cargo.toml`; verify AppImage build
2. Create metadata module: `mod.rs` + `db.rs` (connection factory, PRAGMAs, permissions, symlink check, migrations)
3. Create `models.rs` + `profile_sync.rs` (minimal schema, UPSERT sync, first-run census)
4. Register `MetadataStore` in Tauri state; hook profile commands (`save`, `rename`, `delete`, `duplicate`, `import_legacy`)
5. Add startup reconciliation scan in `startup.rs`
6. Add metadata module tests (unit + integration)

**Parallelization**: Tasks 1-2 (dependency + bootstrap) can run in parallel with task 3 (models + sync). Task 4 depends on 1-3. Task 5-6 can run after task 4.

### Phase 2: Operational History

**Focus**: persist launcher relationships and launch outcomes.
**Dependencies**: Phase 1 complete; `LaunchRequest.profile_name` gap resolved.

**Tasks**:

1. Resolve `LaunchRequest` profile identity gap
2. Add launcher tables + sync logic + drift observation
3. Record launch operations + diagnostic persistence
4. Build derived projection queries (last success, health, drift)
5. Startup sweep for abandoned `launch_operation` rows
6. Add Phase 2 tests

### Phase 3: Catalog and Intelligence

**Focus**: fast local intelligence layer.

**Tasks**:

1. Index community tap manifests with HEAD watermark skip
2. Add collections/favorites UX
3. Add usage insights queries
4. Add external metadata cache with payload validation + size bounds
5. Optional FTS5 for community search (only if `LIKE` proves insufficient)

## Decisions Resolved

All open decisions have been resolved:

1. **RecentFilesStore migration behavior**: preserve stale paths with a `missing` flag rather than silently dropping them. Users can then Locate or Remove stale entries explicitly.
2. **Collection portability**: designed with future export/import semantics in mind. Schema should support a portable serialization format even though v1 collections are local-only.
3. **CLI metadata support**: deferred. `crosshook-cli` does not integrate with `MetadataStore` in Phase 1. CLI metadata sync is a Phase 2+ concern.
4. **`MetadataStore` in Tauri state**: always-present struct with internal `available: bool` flag. This avoids `Option<MetadataStore>` checks at every call site. Methods return early with a logged warning when `available` is false.
5. **Data retention policy**: configurable, 30-365 days. Default to 90 days. Stored as a new field in `AppSettingsData` (`launch_history_retention_days: u32`). Pruning runs at startup after reconciliation. Expose in Settings panel.

## Adopted Defaults

1. **Stable ID Portability**
   - **Decision**: UUIDs remain local-only in SQLite for v1; backend-only (frontend uses profile names).
   - **Why**: delivers rename-safety immediately without modifying TOML format or complicating scripting.
   - **Revisit**: if cross-machine portability or exported collections become a core requirement.

2. **SQLite Packaging Strategy**
   - **Decision**: bundle SQLite 3.51.3 via `rusqlite` `bundled` feature; live DB at `~/.local/share/crosshook/metadata.db`.
   - **Why**: AppImage determinism; resolves all known CVEs + WAL race; avoids SteamOS version mismatches.

3. **Launcher Drift Recovery Policy**
   - **Decision**: warning-only detection plus one-click assisted relink.
   - **Why**: incorrect auto-repair worse than clear warning; assisted relink keeps users in control.

4. **Phase 1 Schema Simplification**
   - **Decision**: 3 tables (`profiles`, `profile_name_history`, plus preference columns inline) — not the full 14-table vision.
   - **Why**: KISS first; build only what Phase 1 features consume; defer audit trails, caches, and projections.

5. **Sync Hook Placement**
   - **Decision**: Tauri command handlers, not `ProfileStore` internals.
   - **Why**: preserves `ProfileStore` as pure TOML I/O; matches existing multi-step orchestration pattern; keeps core stores testable without SQLite.

## Research References

For detailed findings, see:

- [research-external.md](./research-external.md): SQLite capabilities, PRAGMAs, JSON/FTS, rusqlite 0.39.0 evaluation, version verification
- [research-business.md](./research-business.md): authority model, user stories, business rules, risk factor resolutions (RF-1 through RF-5), UX-driven rules
- [research-technical.md](./research-technical.md): schema design, type mappings, sync boundaries, API design, cross-team refinements, file-level impact
- [research-ux.md](./research-ux.md): user-facing workflows, competitive analysis (Steam/Heroic/Playnite/Lutris), accessibility, gamepad support, error state design
- [research-security.md](./research-security.md): threat model, severity-leveled risks (W1-W8, A1-A6), dependency posture, secure coding guidelines, data sensitivity
- [research-practices.md](./research-practices.md): reuse opportunities, KISS assessment, minimal schema guidance, build-vs-depend decisions, testability patterns
- [research-recommendations.md](./research-recommendations.md): rollout strategy, phasing, security/practices integration, alternative approaches, decision checklist
