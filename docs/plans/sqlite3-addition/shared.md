# SQLite Metadata Layer — Phase 3: Catalog and Intelligence

Phase 3 extends the existing `MetadataStore` (schema v3 with `profiles`, `profile_name_history`, `launchers`, `launch_operations`) by adding five new tables (`community_taps`, `community_profiles`, `external_cache_entries`, `collections`, `collection_profiles`) and two new metadata module files (`community_index.rs`, `cache_store.rs`) following the same `with_conn` fail-soft delegation, free-function module, and warn-and-continue hook patterns established in Phases 1-2. The primary optimization is a HEAD commit watermark in `community_taps` that lets `community_sync` skip expensive `index_taps()` recursive filesystem scans when a tap's git HEAD is unchanged; when HEAD does change, the tap's `community_profiles` rows are replaced via transactional DELETE+INSERT (not UPSERT) to eliminate stale ghost entries. Collections/favorites leverage the existing `is_favorite`/`is_pinned` columns in the Phase 1 `profiles` table (never written to until now) plus new `collections`/`collection_profiles` join tables, usage insights are SQL aggregate projections over the Phase 2 `launch_operations` table (no materialized tables), and FTS5 for community search is deferred unless `LIKE` proves insufficient.

## Relevant Files

- src/crosshook-native/crates/crosshook-core/src/metadata/mod.rs: MetadataStore struct, `with_conn`/`with_conn_mut` fail-soft helpers, all public API methods — Phase 3 adds new delegates here
- src/crosshook-native/crates/crosshook-core/src/metadata/db.rs: Connection factory (`open_at_path`, `open_in_memory`), `new_id()` UUID v4, `configure_connection()` with PRAGMAs
- src/crosshook-native/crates/crosshook-core/src/metadata/migrations.rs: Sequential migration runner (v0→v1→v2→v3) — Phase 3 adds `migrate_3_to_4()` for all five new tables
- src/crosshook-native/crates/crosshook-core/src/metadata/models.rs: MetadataStoreError, SyncSource, LaunchOutcome, DriftState enums, MAX_DIAGNOSTIC_JSON_BYTES — Phase 3 adds new row structs, constants, and possibly CacheEntryStatus enum
- src/crosshook-native/crates/crosshook-core/src/metadata/profile_sync.rs: `lookup_profile_id(conn, name)` at lines 72-86 — reusable bridge for collection membership FK resolution
- src/crosshook-native/crates/crosshook-core/src/metadata/launcher_sync.rs: `with_conn_mut` usage and atomic transaction pattern — template for DELETE+INSERT re-index
- src/crosshook-native/crates/crosshook-core/src/metadata/launch_history.rs: JSON payload size-bounded storage pattern (MAX_DIAGNOSTIC_JSON_BYTES) — template for cache payload bounds
- src/crosshook-native/crates/crosshook-core/src/community/taps.rs: `CommunityTapStore`, `CommunityTapSubscription` (url + branch), `CommunityTapSyncResult` with `head_commit: String` at line 44 — the watermark source
- src/crosshook-native/crates/crosshook-core/src/community/index.rs: `CommunityProfileIndex`, `CommunityProfileIndexEntry`, `index_taps()` recursive filesystem scan, schema version check at line 145
- src/crosshook-native/crates/crosshook-core/src/community/mod.rs: Re-exports `CommunityProfileManifest`, `CommunityProfileMetadata`, `CompatibilityRating`, `COMMUNITY_PROFILE_SCHEMA_VERSION`
- src/crosshook-native/crates/crosshook-core/src/profile/community_schema.rs: CommunityProfileManifest, CommunityProfileMetadata (game_name, trainer_name, platform_tags, compatibility_rating, author, description), CompatibilityRating enum
- src/crosshook-native/src-tauri/src/commands/community.rs: `community_sync`, `community_list_profiles`, `community_add_tap`, `community_import_profile` — Phase 3 adds `State<MetadataStore>` and sync_tap_index hook
- src/crosshook-native/src-tauri/src/commands/export.rs: Warn-and-continue pattern for metadata hooks — template for Phase 3 command hooks
- src/crosshook-native/src-tauri/src/commands/profile.rs: Existing metadata sync hooks after profile CRUD — template for `profile_set_favorite`
- src/crosshook-native/src-tauri/src/lib.rs: MetadataStore `.manage()` registration at line 80, `invoke_handler!` command list — Phase 3 adds new commands here
- src/crosshook-native/src-tauri/src/startup.rs: `run_metadata_reconciliation()` — Phase 3 may add community index cleanup
- src/crosshook-native/crates/crosshook-core/src/settings/mod.rs: AppSettingsData — community_taps subscription list lives here (TOML canonical)
- src/crosshook-native/src/hooks/useCommunityProfiles.ts: React hook for community state — `syncTaps()` and `refreshProfiles()` invoke Tauri commands
- src/crosshook-native/src/components/CommunityBrowser.tsx: Client-side search via `matchesQuery()` at line 30, CollapsibleSection pattern, BEM class names
- src/crosshook-native/crates/crosshook-core/Cargo.toml: `rusqlite = { version = "0.38", features = ["bundled"] }` — FTS5 is available via bundled SQLite, no Cargo.toml change needed for basic FTS5

## Relevant Tables

- profiles (v1): Stable UUID identity — `profile_id` FK target for `collection_profiles`; `is_favorite INTEGER DEFAULT 0` and `is_pinned INTEGER DEFAULT 0` columns exist but are never written until Phase 3
- profile_name_history (v1): Append-only rename events — unmodified by Phase 3
- launchers (v3): Launcher mapping with drift — unmodified by Phase 3
- launch_operations (v3): Launch history — Phase 3 reads for usage insights queries (SQL aggregates only, no schema changes)
- community_taps (Phase 3 NEW): `tap_id TEXT PK`, `tap_url TEXT NOT NULL`, `tap_branch TEXT NOT NULL DEFAULT ''`, `local_path TEXT NOT NULL`, `last_head_commit TEXT`, `profile_count INTEGER NOT NULL DEFAULT 0`, `last_indexed_at TEXT`, `created_at TEXT NOT NULL`, `updated_at TEXT NOT NULL`; UNIQUE index on `(tap_url, tap_branch)` — stores HEAD watermark per tap
- community_profiles (Phase 3 NEW): `id INTEGER PK AUTOINCREMENT`, `tap_id TEXT NOT NULL REFERENCES community_taps(tap_id)`, `relative_path TEXT NOT NULL`, `manifest_path TEXT NOT NULL`, `game_name TEXT`, `game_version TEXT`, `trainer_name TEXT`, `trainer_version TEXT`, `proton_version TEXT`, `compatibility_rating TEXT`, `author TEXT`, `description TEXT`, `platform_tags_json TEXT`, `schema_version INTEGER NOT NULL DEFAULT 1`, `created_at TEXT NOT NULL`; UNIQUE index on `(tap_id, relative_path)` — replaced via DELETE+INSERT per tap re-index
- external_cache_entries (Phase 3 NEW): `cache_id TEXT PK`, `source_url TEXT NOT NULL`, `cache_key TEXT NOT NULL UNIQUE`, `payload_json TEXT`, `payload_size INTEGER NOT NULL DEFAULT 0`, `fetched_at TEXT NOT NULL`, `expires_at TEXT`, `created_at TEXT NOT NULL`, `updated_at TEXT NOT NULL` — forward-looking infrastructure; no external HTTP client exists yet
- collections (Phase 3 NEW): `collection_id TEXT PK`, `name TEXT NOT NULL UNIQUE`, `description TEXT`, `created_at TEXT NOT NULL`, `updated_at TEXT NOT NULL` — local-only user curation groups
- collection_profiles (Phase 3 NEW): `collection_id TEXT NOT NULL REFERENCES collections(collection_id) ON DELETE CASCADE`, `profile_id TEXT NOT NULL REFERENCES profiles(profile_id)`, `added_at TEXT NOT NULL`; PRIMARY KEY `(collection_id, profile_id)` — many-to-many join

## Relevant Patterns

**`with_conn` Fail-Soft Delegation**: Every public MetadataStore method delegates through `with_conn(action, |conn| submodule::fn(conn, ...))` which no-ops when disabled (`T::default()`). Phase 3 methods replicate this exact shape. `with_conn_mut` is used when `&mut Connection` is needed for `Transaction::new`. See [src/crosshook-native/crates/crosshook-core/src/metadata/mod.rs](src/crosshook-native/crates/crosshook-core/src/metadata/mod.rs) lines 59-95.

**Free Function + Module Delegation**: Sync logic in submodules (`profile_sync.rs`, `launcher_sync.rs`, `launch_history.rs`) uses free functions with `conn: &Connection` first arg. Phase 3 adds `community_index.rs` and `cache_store.rs` following this pattern. See [src/crosshook-native/crates/crosshook-core/src/metadata/profile_sync.rs](src/crosshook-native/crates/crosshook-core/src/metadata/profile_sync.rs).

**Transactional DELETE+INSERT for Re-Index**: Community profile re-indexing MUST use DELETE+INSERT in a transaction (not UPSERT) to eliminate stale ghost entries when profiles are removed from a tap. The `observe_launcher_renamed` in `launcher_sync.rs` demonstrates the `Transaction::new(conn, TransactionBehavior::Immediate)` pattern. See [src/crosshook-native/crates/crosshook-core/src/metadata/launcher_sync.rs](src/crosshook-native/crates/crosshook-core/src/metadata/launcher_sync.rs).

**Warn-and-Continue**: Tauri commands call metadata hooks in `if let Err(e) { tracing::warn!(...) }` blocks — metadata failures never block the primary operation. See [src/crosshook-native/src-tauri/src/commands/export.rs](src/crosshook-native/src-tauri/src/commands/export.rs) lines 26-38.

**Enum with `as_str()`**: Metadata enums derive `Debug, Clone, Copy, Serialize, Deserialize` with `#[serde(rename_all = "snake_case")]` and expose `as_str() -> &'static str`. See [src/crosshook-native/crates/crosshook-core/src/metadata/models.rs](src/crosshook-native/crates/crosshook-core/src/metadata/models.rs) lines 69-137.

**Size-Bounded JSON Storage**: Payloads serialized to JSON are validated against a `MAX_*_BYTES` constant before INSERT; oversized payloads stored as NULL (not truncated). Phase 3 adds `MAX_CACHE_PAYLOAD_BYTES = 512_000` for external cache. See [src/crosshook-native/crates/crosshook-core/src/metadata/launch_history.rs](src/crosshook-native/crates/crosshook-core/src/metadata/launch_history.rs) lines 66-82.

**Sequential Migration Runner**: `if version < N { migrate_N_minus_1_to_N(conn)?; pragma_update(N)?; }` guards with `execute_batch()` for literal-only DDL. Phase 3 adds `migrate_3_to_4()`. See [src/crosshook-native/crates/crosshook-core/src/metadata/migrations.rs](src/crosshook-native/crates/crosshook-core/src/metadata/migrations.rs).

**`lookup_profile_id` Reuse**: `profile_sync.rs:72-86` exposes `pub fn lookup_profile_id(conn, name) -> Result<Option<String>>` to resolve profile name to stable UUID. Collection membership and favorites must use this — do not duplicate the lookup query.

**`map_error` Helper in Community Commands**: `community.rs:8-10` has a private `fn map_error(e: impl ToString) -> String` used throughout. Phase 3 community commands use this same helper.

## Relevant Docs

**docs/plans/sqlite3-addition/feature-spec.md**: You _must_ read this when working on any Phase 3 task. Phase 3 schema (line 223), Phase 3 task list (lines 524-534), security findings W3/W6/W8/A6, business rules 7/13, edge cases, success criteria.

**docs/plans/sqlite3-addition/research-architecture.md**: You _must_ read this when understanding the community tap sync flow, frontend integration points, and where Phase 3 hooks plug into existing commands.

**docs/plans/sqlite3-addition/research-patterns.md**: You _must_ read this when creating new metadata module files. Exact code shapes for `with_conn`, free functions, enums, UPSERT, migrations, tests — all verified from live source. Includes critical gotchas: DELETE+INSERT for community_profiles, NULL uniqueness for tap_branch, FTS5 content table sync.

**docs/plans/sqlite3-addition/research-integration.md**: You _must_ read this when modifying community commands or building collection IPC. All Tauri command signatures, proposed collection command shapes, usage insights SQL queries, current v3 schema detail, frontend IPC data shapes.

**docs/plans/sqlite3-addition/research-docs.md**: You _must_ read this for Phase 3 business rules, security findings, edge cases, A6 string length bounds, and the prioritized must-read document list.

**CLAUDE.md**: You _must_ read this for project conventions — commit messages, build commands, Rust style, test commands, label taxonomy.

## Design Decisions (Locked)

| Decision                                | Choice                                                                                             | Rationale                                                                                                                                 |
| --------------------------------------- | -------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------- |
| Community profile re-indexing strategy  | Transactional DELETE+INSERT per tap (not UPSERT)                                                   | UPSERT leaves stale ghost rows when profiles are removed from a tap; DELETE+INSERT is clean and consistent with Phase 1 tombstone cleanup |
| `tap_branch` NULL handling              | Store as `NOT NULL DEFAULT ''` (empty string for absent branch)                                    | SQLite `NULL != NULL` in UNIQUE indexes allows duplicate rows; `COALESCE` in index is fragile; empty string avoids the problem entirely   |
| `community_taps` watermark source       | `CommunityTapSyncResult.head_commit` (already populated by `taps.rs:44`)                           | No new git operation needed; `rev_parse_head()` already runs during sync                                                                  |
| Favorites columns                       | Reuse existing `profiles.is_favorite` and `profiles.is_pinned` from Phase 1 schema                 | No migration DDL needed; only new MetadataStore API methods and Tauri commands                                                            |
| Usage insights implementation           | SQL aggregate projections over `launch_operations` (no materialized tables)                        | Follows spec "Projection Rule"; avoids sync burden of materialized views                                                                  |
| FTS5 for community search               | Defer unless `LIKE` proves insufficient                                                            | `bundled` rusqlite already includes FTS5; schema is extensible; YAGNI for v1                                                              |
| External cache HTTP fetch location      | Tauri command layer (not crosshook-core)                                                           | No HTTP client exists in crosshook-core; matches existing boundary (commands fetch, core stores)                                          |
| Collections scope                       | Local-only in Phase 3; schema supports future export via stable `profile_id` FK                    | Feature spec mandates portability-ready design even though v1 is local-only                                                               |
| `platform_tags` storage format          | Space-separated string (not JSON array)                                                            | Better FTS5 tokenization if FTS is added later; simpler `LIKE` queries; `"linux steam-deck"` vs `'["linux","steam-deck"]'`                |
| Cache payload bound                     | `MAX_CACHE_PAYLOAD_BYTES = 512_000` (512 KB)                                                       | Security finding W3; separate from Phase 2's 4 KB diagnostic limit                                                                        |
| A6 string length bounds                 | `game_name` <= 512B, `description` <= 4KB, `platform_tags` <= 2KB, `trainer_name`/`author` <= 512B | Advisory A6 from security review; reject with diagnostic, don't silently truncate                                                         |
| New Tauri commands file for collections | `src-tauri/src/commands/collections.rs` (new file)                                                 | One-domain-per-file convention; `profile_set_favorite` goes in existing `commands/profile.rs`                                             |
| `community_profiles` unique key         | `(tap_id, relative_path)`                                                                          | Natural composite key — one manifest per relative path per tap                                                                            |
| Schema version for Phase 3              | v4 (single `migrate_3_to_4` for all five tables)                                                   | All Phase 3 tables in one migration; consistent with existing pattern                                                                     |
