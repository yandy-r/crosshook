# SQLite Metadata Layer

CrossHook's backend is a Rust workspace (`crosshook-core` library + `crosshook-cli` binary + `src-tauri` Tauri v2 shell) with a React 18 TypeScript frontend. The new `MetadataStore` adds SQLite (`rusqlite` 0.39.0, bundled SQLite 3.51.3) as a secondary metadata store inside `crosshook-core/src/metadata/`, keeping TOML profiles canonical. Metadata sync hooks live exclusively in Tauri command handlers (`src-tauri/src/commands/`), following the existing best-effort cascade pattern where `ProfileStore` remains a pure TOML I/O layer. The store uses `Arc<Mutex<Connection>>` (matching the `RotatingLogWriter` precedent), is registered via `.manage()` in `lib.rs`, and carries an internal `available` flag for fail-soft degradation — methods no-op when SQLite is unavailable.

## Relevant Files

- src/crosshook-native/crates/crosshook-core/src/lib.rs: Module root; add `pub mod metadata;` alongside existing community, export, launch, profile, settings, steam modules
- src/crosshook-native/crates/crosshook-core/Cargo.toml: Add `rusqlite = { version = "0.39", features = ["bundled"] }` and `uuid = { version = "1", features = ["v4", "serde"] }` to dependencies
- src/crosshook-native/crates/crosshook-core/src/profile/toml_store.rs: Primary store pattern template — `try_new()`, `with_base_path()`, `validate_name()`, error enum; NOT modified for metadata
- src/crosshook-native/crates/crosshook-core/src/profile/models.rs: `GameProfile` struct — `observe_profile_write` extracts `game_name` and `launch_method` from this
- src/crosshook-native/crates/crosshook-core/src/export/launcher_store.rs: `LauncherInfo`, `LauncherDeleteResult`, `LauncherRenameResult`, `sanitize_launcher_slug()`, `derive_launcher_paths()` — Phase 2 launcher table maps to these
- src/crosshook-native/crates/crosshook-core/src/launch/request.rs: `LaunchRequest` — missing `profile_name` field (Phase 2 blocker)
- src/crosshook-native/crates/crosshook-core/src/launch/diagnostics/models.rs: `DiagnosticReport`, `FailureMode`, `ExitCodeInfo` — Phase 2 launch_operations stores serialized DiagnosticReport
- src/crosshook-native/crates/crosshook-core/src/settings/mod.rs: `SettingsStore` pattern; `AppSettingsData` includes `community_taps` and `last_used_profile`
- src/crosshook-native/crates/crosshook-core/src/settings/recent.rs: `RecentFilesStore` — uses `data_local_dir()`, confirms correct base for `metadata.db`
- src/crosshook-native/crates/crosshook-core/src/community/taps.rs: `CommunityTapStore`, `CommunityTapSyncResult` with `head_commit` for idempotent re-indexing
- src/crosshook-native/crates/crosshook-core/src/community/index.rs: `index_tap()` recursive manifest scan — Phase 3 SQLite augments this as read cache
- src/crosshook-native/crates/crosshook-core/src/logging.rs: `Arc<Mutex<RotatingLogState>>` — precedent for `MetadataStore` connection wrapper pattern
- src/crosshook-native/src-tauri/src/lib.rs: Store initialization, `.manage()` registration, `invoke_handler` command list — MetadataStore goes here
- src/crosshook-native/src-tauri/src/startup.rs: Auto-load profile; add startup reconciliation scan (`sync_profiles_from_store`)
- src/crosshook-native/src-tauri/src/commands/profile.rs: Profile lifecycle commands — metadata sync hooks after `save`, `rename`, `delete`, `duplicate`, `import_legacy`
- src/crosshook-native/src-tauri/src/commands/launch.rs: Async launch commands — Phase 2 `record_launch_started/finished` via `spawn_blocking`; `sanitize_display_path()` at line ~301 (must promote to shared.rs)
- src/crosshook-native/src-tauri/src/commands/export.rs: Launcher export/delete/rename — Phase 2 metadata sync for launcher observations
- src/crosshook-native/src-tauri/src/commands/community.rs: `community_sync` — Phase 3 `sync_tap_index()` after `sync_many()`
- src/crosshook-native/src-tauri/src/commands/shared.rs: `create_log_path`, `slugify_target` — destination for promoted `sanitize_display_path()`
- src/crosshook-native/src-tauri/src/commands/mod.rs: Add `pub mod metadata;` for new metadata Tauri commands

## Relevant Tables

- profiles: Stable UUID identity, current filename/path, game_name, launch_method, is_favorite, is_pinned, source_profile_id, deleted_at, created_at, updated_at
- profile_name_history: Append-only rename events — profile_id FK, old/new name/path, source (app_rename, filesystem_scan, import, initial_census), created_at
- launchers (Phase 2): Composite PK (profile_id, launcher_slug), display_name, script_path, desktop_entry_path, drift_state, created_at, updated_at
- launch_operations (Phase 2): Launch attempts — profile_id FK, method, game/trainer paths, outcome (incomplete/succeeded/failed/abandoned), exit_code, signal, diagnostic_json (max 4KB), severity, failure_mode
- community_taps (Phase 3): (tap_url, tap_branch) PK, head_commit for idempotent skip, last_synced_at
- community_profiles (Phase 3): Indexed manifest rows — tap FK, game_name, trainer_name, compatibility_rating, platform_tags_json
- external_cache_entries (Phase 3): Typed cache with freshness — cache_bucket, cache_key, payload_json (max 512KB), fetched_at, expires_at

## Relevant Patterns

**Three-Constructor Store Pattern**: Every store exposes `try_new() -> Result<Self, String>` (production), `new() -> Self` (panic wrapper), and `with_base_path()`/`with_path()` (test injection). See [src/crosshook-native/crates/crosshook-core/src/profile/toml_store.rs](src/crosshook-native/crates/crosshook-core/src/profile/toml_store.rs) lines 83-98.

**Best-Effort Cascade**: Multi-step Tauri commands where the critical TOML operation propagates errors with `?` and all subsequent steps (launcher cleanup, display_name update, settings update) use `if let Err(e) { tracing::warn!(...) }`. See [src/crosshook-native/src-tauri/src/commands/profile.rs](src/crosshook-native/src-tauri/src/commands/profile.rs) `profile_rename` at lines 149-194. Metadata sync hooks follow this exact pattern as additional best-effort steps.

**IPC Error Boundary**: All Tauri commands return `Result<T, String>`. Domain errors are converted via `.map_err(|e| e.to_string())` or a private `map_error` helper. Raw `rusqlite::Error` must never reach the frontend. See [src/crosshook-native/src-tauri/src/commands/profile.rs](src/crosshook-native/src-tauri/src/commands/profile.rs) lines 9-11.

**Arc<Mutex<...>> Shared State**: `RotatingLogWriter` wraps mutable state in `Arc<Mutex<RotatingLogState>>` for `Clone` + thread-safe access. `MetadataStore` uses `Arc<Mutex<Connection>>` identically. See [src/crosshook-native/crates/crosshook-core/src/logging.rs](src/crosshook-native/crates/crosshook-core/src/logging.rs) lines 118-120.

**Structured Error Enum**: Each module defines its own error enum with `Io { action: &'static str, path: PathBuf, source }` variant, `Display` impl, `Error` impl, and `From` impls. See [src/crosshook-native/crates/crosshook-core/src/community/taps.rs](src/crosshook-native/crates/crosshook-core/src/community/taps.rs) lines 48-91.

**UPSERT Reconciliation**: SQLite `INSERT ... ON CONFLICT DO UPDATE` for idempotent sync from TOML/filesystem scans. Required for `sync_profiles_from_store()` and all observation writes.

**spawn_blocking Async Bridge**: `rusqlite::Connection` is `!Send`. Async Tauri commands (`launch_game`/`launch_trainer`) must use `tokio::task::spawn_blocking` for metadata writes. No existing example in codebase — new pattern for Phase 2.

## Relevant Docs

**docs/plans/sqlite3-addition/feature-spec.md**: You _must_ read this when working on any sqlite3-addition task. Master spec with authority matrix, Phase 1/2/3 schemas, business rules, success criteria, security findings, and adopted defaults.

**docs/plans/sqlite3-addition/research-technical.md**: You _must_ read this when creating new metadata module files or modifying existing files. Verified file inventory, type-to-table mappings, API design, integration points with exact function signatures.

**docs/plans/sqlite3-addition/research-practices.md**: You _must_ read this when designing MetadataStore interfaces or writing tests. Existing reusable code with file:line references, KISS assessment, minimal Phase 1 schema guidance, testability patterns.

**docs/plans/sqlite3-addition/research-security.md**: You _must_ read this when implementing connection setup, path handling, or IPC responses. W1-W8 security findings with required mitigations (file permissions, parameterized queries, path sanitization, payload bounds).

**docs/plans/sqlite3-addition/research-integration.md**: You _must_ read this when adding metadata sync hooks to Tauri commands. All 30 Tauri IPC command signatures, store APIs, launch system hooks, filesystem paths.

**docs/plans/sqlite3-addition/research-patterns.md**: You _must_ read this when following codebase conventions. Three-constructor pattern, error enum pattern, cascade pattern, testing patterns with concrete code examples.

**CLAUDE.md**: You _must_ read this for project conventions — commit messages, build commands, Rust style, test commands, label taxonomy.
