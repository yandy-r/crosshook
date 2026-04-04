# Protontricks Integration Implementation Plan

CrossHook's protontricks-integration feature adds WINE prefix dependency management (vcrun2019, dotnet48, etc.) by creating a new `prefix_deps` module in `crosshook-core` that detects winetricks/protontricks binaries via PATH walk, checks installed packages via `winetricks list-installed`, installs missing packages with streamed output, and persists state in a new SQLite `prefix_dependency_state` table (migration v14 to v15). The implementation follows the established three-layer architecture: business logic in `crosshook-core`, thin IPC wrappers in `src-tauri/commands/prefix_deps.rs` exposing 4 new Tauri commands, and a React `PrefixDepsPanel` component with a `usePrefixDeps` hook consuming those commands. No new crates are required -- all subprocess, SQLite, serialization, and event-streaming infrastructure is already in the dependency tree.

## Resolved Decisions

These 4 decisions are resolved as of 2026-04-03:

1. **Field name**: `required_protontricks` -- community familiarity; winetricks verbs are the canonical naming regardless of tool
2. **Verb allowlist approach**: Structural regex `^[a-z0-9][a-z0-9_\-]{0,63}$` as hard gate + static known-verb set (~12 common trainer verbs) for advisory warnings only; unknown-but-valid verbs pass the regex without blocking
3. **`user_skipped` reset**: Per-package action; TTL expiry (24h) acts as automatic reset
4. **TTL value**: 24 hours -- matches existing `health_snapshots` staleness model in the codebase

## Critically Relevant Files and Documentation

### Must-Read Before Any Task

- docs/plans/protontricks-integration/feature-spec.md: Authoritative resolved spec -- all data models, 4 IPC command contracts, 14 business rules, security table, phasing strategy
- docs/plans/protontricks-integration/research-security.md: 7 CRITICAL findings (S-01, S-02, S-03, S-06, S-19, S-22, S-27) that block ship -- command injection prevention, `--` separator, no raw subprocess output to UI
- docs/plans/protontricks-integration/research-practices.md: Exact reusable file inventory with line numbers -- prevents reimplementing existing utilities
- AGENTS.md: Platform rules, directory map, SQLite schema inventory, IPC naming conventions

### Core Pattern Files

- src/crosshook-native/crates/crosshook-core/src/launch/runtime_helpers.rs: `resolve_umu_run_path()` at line 302 (PATH walk), `apply_host_environment()` at line 153, `is_executable_file()` at line 314
- src/crosshook-native/crates/crosshook-core/src/metadata/migrations.rs: Sequential `if version < N` migration pattern; currently at v14
- src/crosshook-native/crates/crosshook-core/src/metadata/health_store.rs: Template for new `prefix_deps_store.rs` -- CRUD functions taking bare `&Connection`
- src/crosshook-native/crates/crosshook-core/src/metadata/mod.rs: `MetadataStore` with `with_conn()` / `with_conn_mut()` dispatch pattern
- src/crosshook-native/crates/crosshook-core/src/community/taps.rs: `validate_branch_name` -- security template for CLI arg validation
- src/crosshook-native/src-tauri/src/commands/update.rs: `Mutex<Option<u32>>` concurrent-install lock pattern
- src/crosshook-native/src-tauri/src/commands/launch.rs: Async spawn + `app.emit()` log streaming at line 350
- src/crosshook-native/src/hooks/useProtonInstalls.ts: React hook pattern with cleanup flag

### Research References

- docs/plans/protontricks-integration/research-technical.md: Component diagram, SQLite migration DDL, TOML field additions, IPC integration points
- docs/plans/protontricks-integration/research-recommendations.md: Phasing strategy, blocking decisions
- docs/plans/protontricks-integration/research-ux.md: `DependencyStatusBadge` design, `DepStatus` type, ConsoleDrawer integration
- docs/plans/protontricks-integration/research-external.md: CLI reference for winetricks/protontricks -- env vars, exit codes, prefix path conventions
- docs/plans/protontricks-integration/research-business.md: 14 business rules (BR-1 to BR-14)
- docs/features/steam-proton-trainer-launch.doc.md: Existing launch flows, prefix handling, ConsoleView

## Implementation Plan

### Phase 1: Foundation -- Types, Detection, and Schema Extensions

#### Task 1.1: Binary detection module and core types Depends on [none]

**READ THESE BEFORE TASK**

- src/crosshook-native/crates/crosshook-core/src/launch/runtime_helpers.rs (lines 301-325: `resolve_umu_run_path()`, `is_executable_file()`)
- src/crosshook-native/crates/crosshook-core/src/launch/mod.rs (module structure pattern)
- docs/plans/protontricks-integration/feature-spec.md (data models section)
- docs/plans/protontricks-integration/research-security.md (S-01, S-02: command injection prevention)

**Instructions**

Files to Create

- src/crosshook-native/crates/crosshook-core/src/prefix_deps/mod.rs
- src/crosshook-native/crates/crosshook-core/src/prefix_deps/detection.rs
- src/crosshook-native/crates/crosshook-core/src/prefix_deps/models.rs

Files to Modify

- src/crosshook-native/crates/crosshook-core/src/lib.rs

Create the `prefix_deps` module directory structure. In `lib.rs`, add `pub mod prefix_deps;` in alphabetical order among existing module declarations.

**`models.rs`**: Define core types that all other phases depend on:

- `DependencyState` enum: `Unknown`, `Installed`, `Missing`, `InstallFailed`, `CheckFailed`, `UserSkipped`
- `PrefixDependencyStatus` struct: `package_name: String`, `state: DependencyState`, `checked_at: Option<String>`, `installed_at: Option<String>`, `last_error: Option<String>`
- `BinaryDetectionResult` struct: `found: bool`, `binary_path: Option<String>`, `binary_name: String`, `source: String` (one of: "settings", "path", "flatpak", "not_found")
- `PrefixDepsError` enum with variants: `BinaryNotFound { tool: String }`, `PrefixNotInitialized { path: String }`, `ValidationError(String)`, `ProcessFailed { exit_code: Option<i32>, stderr: String }`, `Timeout { seconds: u64 }`, `AlreadyInstalling { prefix_path: String }`, `Database { action: &'static str, source: rusqlite::Error }`
- All types derive `Debug, Clone, Serialize, Deserialize` (those crossing IPC)

**`detection.rs`**: Implement binary detection using the `resolve_umu_run_path()` pattern verbatim:

- `resolve_winetricks_path() -> Option<String>`: Walk PATH for `winetricks` using `is_executable_file()`
- `resolve_protontricks_path() -> Option<String>`: Walk PATH for `protontricks` using `is_executable_file()`
- `detect_binary(settings_path: &str) -> BinaryDetectionResult`: Priority order: (1) settings override if non-empty and executable, (2) `winetricks` on PATH, (3) `protontricks` on PATH. Return structured result.
- Import and reuse `is_executable_file` from `launch::runtime_helpers` (re-export it as `pub` if needed)

**`mod.rs`**: Re-export public API: `pub mod detection; pub mod models; pub use models::*;`

**Tests**: In `detection.rs`, write unit tests:

- `detect_binary_returns_not_found_when_no_tool_on_path`: Empty PATH → `found: false`
- `detect_binary_prefers_settings_override`: When settings path is executable, source is "settings"
- Use `ScopedCommandSearchPath` test pattern if available, otherwise `tempdir` with fake executables

Run `cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core` to verify.

#### Task 1.2: TOML profile schema extension Depends on [none]

**READ THESE BEFORE TASK**

- src/crosshook-native/crates/crosshook-core/src/profile/models.rs (TrainerSection struct, serde patterns)
- src/crosshook-native/crates/crosshook-core/src/profile/community_schema.rs (CommunityProfileManifest)

**Instructions**

Files to Modify

- src/crosshook-native/crates/crosshook-core/src/profile/models.rs
- src/crosshook-native/crates/crosshook-core/src/profile/community_schema.rs

In `models.rs`, add to `TrainerSection`:

```rust
#[serde(default, skip_serializing_if = "Vec::is_empty")]
pub required_protontricks: Vec<String>,
```

Update `TrainerSection`'s `Default` impl to include `required_protontricks: Vec::new()`.

If `LocalOverrideTrainerSection` exists, add `extra_protontricks: Vec<String>` with same serde attributes (allows local profiles to declare additional deps beyond the community profile's list).

In `community_schema.rs`, verify `CommunityProfileManifest` wraps `GameProfile` -- since `TrainerSection` is nested inside `GameProfile`, the new field automatically appears in community profiles. No code change needed unless there's a separate community manifest struct.

**Tests**: Add TOML round-trip tests in `models.rs`:

- `trainer_section_roundtrip_with_required_protontricks`: Serialize TrainerSection with `required_protontricks: vec!["vcrun2019", "dotnet48"]`, deserialize, assert equal
- `trainer_section_roundtrip_without_required_protontricks`: Serialize with empty vec, verify field is absent from TOML output (`skip_serializing_if`)
- `trainer_section_deserialize_without_field`: Deserialize existing TOML without the field, assert `required_protontricks` defaults to empty vec (backward compatibility)

#### Task 1.3: Settings extension and onboarding check Depends on [none] (soft dep on 1.1 for winetricks detection wiring)

**READ THESE BEFORE TASK**

- src/crosshook-native/crates/crosshook-core/src/settings/mod.rs (AppSettingsData, Default impl, Debug impl)
- src/crosshook-native/crates/crosshook-core/src/onboarding/readiness.rs (check_system_readiness pattern -- if this file exists)
- src/crosshook-native/src-tauri/src/commands/onboarding.rs (readiness check command)

**Instructions**

Files to Modify

- src/crosshook-native/crates/crosshook-core/src/settings/mod.rs
- src/crosshook-native/crates/crosshook-core/src/onboarding/readiness.rs (if exists; otherwise check `src-tauri/src/commands/onboarding.rs` for the readiness check logic)

In `settings/mod.rs`, add to `AppSettingsData`:

```rust
#[serde(default, skip_serializing_if = "String::is_empty")]
pub protontricks_binary_path: String,

#[serde(default)]
pub auto_install_prefix_deps: bool,
```

Update `Default` impl (both fields default to empty string / false). Update `Debug` impl if it's manually implemented.

For the onboarding readiness check: add a winetricks/protontricks binary detection check with `Info` severity (not blocking). If neither tool is found, report an informational readiness item "winetricks not found -- WINE prefix dependency management unavailable". Use the `resolve_winetricks_path()` function from Task 1.1 -- if Task 1.1 hasn't landed yet, add a `TODO` comment and wire it up later.

**Tests**:

- Settings TOML round-trip with new fields (serialize with `protontricks_binary_path: "/usr/bin/protontricks"`, deserialize, assert equal)
- Settings backward compat (deserialize existing TOML without new fields, assert defaults)

### Phase 2: Storage -- SQLite Migration and Store

#### Task 2.1: SQLite migration v14 to v15 Depends on [none]

**READ THESE BEFORE TASK**

- src/crosshook-native/crates/crosshook-core/src/metadata/migrations.rs (full file -- understand the migration runner pattern, `migrate_13_to_14()` as template)
- src/crosshook-native/crates/crosshook-core/src/metadata/db.rs (`open_in_memory()` for tests)
- docs/plans/protontricks-integration/research-technical.md (DDL for `prefix_dependency_state` table)

**Instructions**

Files to Modify

- src/crosshook-native/crates/crosshook-core/src/metadata/migrations.rs

Add `migrate_14_to_15(conn: &Connection) -> Result<(), MetadataStoreError>` function with this DDL:

```sql
CREATE TABLE IF NOT EXISTS prefix_dependency_state (
    id               INTEGER PRIMARY KEY AUTOINCREMENT,
    profile_id       TEXT NOT NULL REFERENCES profiles(profile_id) ON DELETE CASCADE,
    package_name     TEXT NOT NULL,
    prefix_path      TEXT NOT NULL,
    state            TEXT NOT NULL DEFAULT 'unknown',
    checked_at       TEXT,
    installed_at     TEXT,
    last_error       TEXT,
    created_at       TEXT NOT NULL,
    updated_at       TEXT NOT NULL
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_prefix_dep_state_profile_package_prefix
    ON prefix_dependency_state(profile_id, package_name, prefix_path);

CREATE INDEX IF NOT EXISTS idx_prefix_dep_state_profile_id
    ON prefix_dependency_state(profile_id);
```

Use `conn.execute_batch(sql)` (not individual `execute()` calls -- matches existing migration pattern).

In `run_migrations()`, add immediately after the `if version < 14` block:

```rust
if version < 15 {
    migrate_14_to_15(conn)?;
    conn.pragma_update(None, "user_version", 15_u32)
        .map_err(|source| MetadataStoreError::Database {
            action: "set user_version to 15",
            source,
        })?;
}
```

**Tests**: Add `migration_14_to_15_creates_prefix_dependency_state_table` test following the pattern at the bottom of migrations.rs:

- Call `open_in_memory()`, run `run_migrations()`, verify `user_version` is 15
- Query `sqlite_master` for `prefix_dependency_state` table existence
- Query `sqlite_master` for the unique index existence
- Verify `PRAGMA foreign_keys` is on and the FK to `profiles` works (insert a profile, insert a dep state row, delete the profile, verify cascade delete)

#### Task 2.2: Prefix dependency state store module Depends on [2.1]

**READ THESE BEFORE TASK**

- src/crosshook-native/crates/crosshook-core/src/metadata/health_store.rs (template for store function signatures and `&Connection` parameter pattern)
- src/crosshook-native/crates/crosshook-core/src/metadata/mod.rs (`with_conn()` / `with_conn_mut()` dispatch, `MetadataStore` method additions)
- src/crosshook-native/crates/crosshook-core/src/metadata/models.rs (MetadataStoreError variants, row struct patterns)

**Instructions**

Files to Create

- src/crosshook-native/crates/crosshook-core/src/metadata/prefix_deps_store.rs

Files to Modify

- src/crosshook-native/crates/crosshook-core/src/metadata/mod.rs

**`prefix_deps_store.rs`**: Implement standalone functions (not methods) taking `&Connection`:

- `upsert_dependency_state(conn: &Connection, profile_id: &str, package_name: &str, prefix_path: &str, state: &str, error: Option<&str>) -> Result<(), MetadataStoreError>`: INSERT OR REPLACE with `created_at` = COALESCE(existing, now), `updated_at` = now. Use `chrono::Utc::now().to_rfc3339()` for timestamps.
- `load_dependency_states(conn: &Connection, profile_id: &str) -> Result<Vec<PrefixDependencyStateRow>, MetadataStoreError>`: Select all rows for profile, ordered by package_name.
- `load_dependency_state(conn: &Connection, profile_id: &str, package_name: &str, prefix_path: &str) -> Result<Option<PrefixDependencyStateRow>, MetadataStoreError>`: Single row lookup by unique index.
- `clear_dependency_states(conn: &Connection, profile_id: &str) -> Result<(), MetadataStoreError>`: Delete all rows for profile.
- `clear_stale_states(conn: &Connection, ttl_hours: i64) -> Result<u64, MetadataStoreError>`: Delete rows where `checked_at` is older than TTL. Return count of deleted rows.

Define `PrefixDependencyStateRow` struct in `metadata/models.rs` (consistent with existing row struct placement): `id: i64`, `profile_id: String`, `package_name: String`, `prefix_path: String`, `state: String`, `checked_at: Option<String>`, `installed_at: Option<String>`, `last_error: Option<String>`, `created_at: String`, `updated_at: String`. Derive `Debug, Clone, Serialize, Deserialize`.

**`mod.rs`**: Add `pub mod prefix_deps_store;`. Add convenience methods to `MetadataStore` that dispatch via `with_conn()` / `with_conn_mut()`:

```rust
pub fn upsert_prefix_dep_state(&self, ...) -> Result<(), MetadataStoreError> {
    self.with_conn_mut("upsert prefix dep state", |conn| {
        prefix_deps_store::upsert_dependency_state(conn, ...)
    })
}
```

Add similar wrappers for `load_prefix_dep_states`, `load_prefix_dep_state`, `clear_prefix_dep_states`, `clear_stale_prefix_dep_states`.

**Tests**: In `prefix_deps_store.rs`, use `db::open_in_memory()` + `run_migrations()`:

- `upsert_and_load_round_trip`: Insert a row, load it, assert fields match
- `upsert_overwrites_existing`: Insert twice for same (profile, package, prefix), verify only one row with updated timestamp
- `clear_stale_removes_old_entries`: Insert with old `checked_at`, call `clear_stale_states`, verify removed
- `cascade_delete_on_profile_removal`: Insert a profile + dep state, delete profile, verify dep state gone

### Phase 3: Core Logic -- Validation, Runner, and Lock

#### Task 3.1: Verb validation module Depends on [1.1]

**READ THESE BEFORE TASK**

- src/crosshook-native/crates/crosshook-core/src/community/taps.rs (`validate_branch_name` -- security validation template)
- docs/plans/protontricks-integration/research-security.md (S-01, S-02, S-03, S-06: injection prevention, `--` separator, per-verb `.arg()`)
- docs/plans/protontricks-integration/research-business.md (BR-3: known verb allowlist)

**Instructions**

Files to Create

- src/crosshook-native/crates/crosshook-core/src/prefix_deps/validation.rs

Files to Modify

- src/crosshook-native/crates/crosshook-core/src/prefix_deps/mod.rs (add `pub mod validation;`)

Implement `validate_protontricks_verbs(verbs: &[String]) -> Result<(), PrefixDepsError>`:

- Structural regex gate: each verb must match `^[a-z0-9][a-z0-9_\-]{0,63}$`
- Reject any verb starting with `-` (flag injection prevention -- S-06)
- Max 50 verbs per batch (DoS prevention)
- Empty verbs list → error
- Return `PrefixDepsError::ValidationError(message)` with details of which verbs failed

Implement `is_known_verb(verb: &str) -> bool`: Static `HashSet` or `phf` set of known winetricks verbs commonly used for game trainers: `vcrun2019`, `vcrun2022`, `dotnet48`, `dotnet40`, `dotnet35`, `d3dx9`, `d3dcompiler_47`, `dxvk`, `xact`, `xinput`, `corefonts`, `allfonts`. This is advisory only -- unknown verbs that pass structural validation are allowed.

**Tests**:

- `valid_verbs_pass`: `["vcrun2019", "dotnet48"]` → Ok
- `reject_flag_injection`: `["-q"]`, `["--help"]` → Err
- `reject_shell_metachar`: `["vcrun;rm -rf"]`, `["dotnet$(cmd)"]` → Err
- `reject_empty_list`: `[]` → Err
- `reject_too_many_verbs`: 51 verbs → Err
- `reject_empty_verb`: `[""]` → Err
- `unknown_verb_passes_structural_validation`: `["somecustomverb123"]` → Ok (just not known)

#### Task 3.2: Winetricks/protontricks runner Depends on [1.1, 2.2]

**READ THESE BEFORE TASK**

- src/crosshook-native/crates/crosshook-core/src/launch/runtime_helpers.rs (lines 153-167: `apply_host_environment()`, lines 198-210: `resolve_wine_prefix_path()`)
- src/crosshook-native/crates/crosshook-core/src/launch/script_runner.rs (subprocess spawn patterns)
- src/crosshook-native/crates/crosshook-core/src/install/service.rs (`build_install_command()` pattern)
- docs/plans/protontricks-integration/research-external.md (winetricks CLI: `list-installed`, `-q` flag, env vars)
- docs/plans/protontricks-integration/research-security.md (ALL CRITICAL findings -- this task is the primary security surface)

**Instructions**

Files to Create

- src/crosshook-native/crates/crosshook-core/src/prefix_deps/runner.rs

Files to Modify

- src/crosshook-native/crates/crosshook-core/src/prefix_deps/mod.rs (add `pub mod runner;`)

Implement two core functions:

**`check_installed(binary_path: &str, prefix_path: &str) -> Result<Vec<String>, PrefixDepsError>`**:

- Build `Command::new(binary_path)` with `.arg("list-installed")`
- Set `WINEPREFIX` env var (use `resolve_wine_prefix_path()` to normalize the path to the `pfx/` directory)
- Call `apply_host_environment()` -- do NOT use `env_clear()` (winetricks/protontricks need full POSIX env)
- Set `.kill_on_drop(true)`
- Apply `tokio::time::timeout(Duration::from_secs(30), child.wait_with_output())`
- Parse stdout: split by whitespace, filter empty strings, collect into `Vec<String>`
- On non-zero exit: return `PrefixDepsError::ProcessFailed` with sanitized stderr (no filesystem paths -- strip with regex or truncate)

**`install_packages(binary_path: &str, prefix_path: &str, verbs: &[String], steam_app_id: Option<&str>) -> Result<tokio::process::Child, PrefixDepsError>`**:

- Call `validate_protontricks_verbs(verbs)?` first
- Check prefix is initialized: verify `pfx/` subdirectory exists, else return `PrefixDepsError::PrefixNotInitialized`
- Build `Command::new(binary_path)`:
  - If using protontricks and `steam_app_id` is Some: `.arg(app_id).arg("-q")`
  - If using winetricks: `.arg("-q")`
  - Add `.arg("--")` separator before verbs (S-06)
  - Add each verb as individual `.arg(verb)` -- NEVER join verbs into a single argument
- Set `WINEPREFIX` env var
- Call `apply_host_environment()`
- Set `.stdout(Stdio::piped()).stderr(Stdio::piped()).kill_on_drop(true)`
- Return the spawned `Child` (caller handles streaming)

**Security checklist for this task (all mandatory)**:

- [ ] Per-verb `.arg()` calls (never `.args([joined_string])`)
- [ ] `cmd.arg("--")` before first verb argument
- [ ] `validate_protontricks_verbs()` called before command construction
- [ ] `apply_host_environment()` used (not `env_clear()` alone)
- [ ] `.kill_on_drop(true)` set
- [ ] Prefix path normalized via `resolve_wine_prefix_path()`
- [ ] `pfx/` existence check before invocation
- [ ] stderr sanitized before inclusion in error messages (no raw filesystem paths)

**Tests**: Use `FakeRunner` approach -- create a temp directory with a shell script (`#!/bin/sh\necho "vcrun2019 dotnet48"`) set as executable, then pass its path as `binary_path`. This avoids needing real winetricks in CI. Do NOT require real winetricks binary:

- `check_installed_parses_whitespace_output`: Fake script prints "vcrun2019 dotnet48\n" to stdout, verify vec!["vcrun2019", "dotnet48"]
- `install_packages_rejects_invalid_verbs`: Pass `["-q"]` → Err(ValidationError)
- `install_rejects_uninitialized_prefix`: Non-existent pfx/ → Err(PrefixNotInitialized)
- `command_uses_arg_separator`: Verify `--` is present before verbs in constructed command

#### Task 3.3: Global install lock Depends on [1.1]

**READ THESE BEFORE TASK**

- src/crosshook-native/src-tauri/src/commands/update.rs (`UpdateProcessState`, `Mutex<Option<u32>>` pattern)

**Instructions**

Files to Create

- src/crosshook-native/crates/crosshook-core/src/prefix_deps/lock.rs

Files to Modify

- src/crosshook-native/crates/crosshook-core/src/prefix_deps/mod.rs (add `pub mod lock;`)

Implement `PrefixDepsInstallLock`:

```rust
pub struct PrefixDepsInstallLock {
    active: tokio::sync::Mutex<Option<String>>, // holds active prefix_path
}
```

Methods:

- `new() -> Self`
- `try_acquire(&self, prefix_path: String) -> Result<PrefixDepsLockGuard, PrefixDepsError>`: If `active` is `None`, set to `Some(prefix_path)` and return guard. If `Some(existing)`, return `PrefixDepsError::AlreadyInstalling { prefix_path: existing }`.
- `PrefixDepsLockGuard`: On drop, sets `active` back to `None`. Use a custom Drop impl or return a closure-based guard.
- `is_locked(&self) -> bool`: Check if any install is active (for UI status queries)
- `active_prefix(&self) -> Option<String>`: Return which prefix is currently being installed (for UI disable logic)

**Tests**:

- `lock_acquire_succeeds_when_free`: Acquire → Ok
- `lock_rejects_concurrent_install`: Acquire first, try second → Err(AlreadyInstalling)
- `lock_releases_on_guard_drop`: Acquire, drop guard, acquire again → Ok
- `active_prefix_returns_correct_path`: Acquire with path "A", assert `active_prefix()` returns "A"

#### Task 3.4: Integration test -- runner + store round-trip Depends on [3.1, 3.2, 2.2]

**READ THESE BEFORE TASK**

- src/crosshook-native/crates/crosshook-core/src/metadata/db.rs (`open_in_memory()`)
- The runner and store modules from Tasks 3.2 and 2.2

**Instructions**

Files to Modify

- src/crosshook-native/crates/crosshook-core/src/prefix_deps/mod.rs (add integration test module)

Write integration tests in `prefix_deps/mod.rs` under `#[cfg(test)] mod integration_tests`:

- `full_check_and_store_cycle`: Create in-memory MetadataStore, run migrations, simulate a check result (use mock/known verbs), upsert state rows, query back, verify state transitions
- `stale_cache_cleared_after_ttl`: Insert state with old `checked_at`, call `clear_stale_states()`, verify removed
- `validation_blocks_bad_verbs_before_store`: Call `validate_protontricks_verbs(["-q"])`, assert error, verify nothing was stored

These tests validate the Phase 2 + Phase 3 integration without a real winetricks binary.

### Phase 4: Health System Integration

#### Task 4.1: Health report dependency enrichment Depends on [2.2]

**READ THESE BEFORE TASK**

- src/crosshook-native/crates/crosshook-core/src/profile/health.rs (`ProfileHealthReport`, `HealthIssue`, `HealthStatus`)
- src/crosshook-native/crates/crosshook-core/src/metadata/health_store.rs (existing health store query patterns)

**Instructions**

Files to Modify

- src/crosshook-native/crates/crosshook-core/src/profile/health.rs

Add a helper function `build_dependency_health_issues(dep_states: &[PrefixDependencyStateRow], required_verbs: &[String]) -> Vec<HealthIssue>`:

- For each verb in `required_verbs`:
  - If state is `Installed`: skip (healthy)
  - If state is `Missing` or `InstallFailed`: emit amber `HealthIssue` with `HealthStatus::Warning` -- message: "Required WINE dependency '{verb}' is not installed"
  - If state is `CheckFailed` or `Unknown`: emit `HealthIssue` with `HealthStatus::Stale` -- message: "Dependency status unknown for '{verb}'"
  - If state is `UserSkipped`: emit `HealthIssue` with `HealthStatus::Info` -- message: "User skipped '{verb}' installation"
- If profile has `required_protontricks` but no dep states at all: emit `HealthStatus::Stale` -- "Prefix dependencies have not been checked"

**CRITICAL**: This function must be pure -- takes data in, returns issues out. NO subprocess spawning, NO async, NO `Command`. The health scan runs at startup; it must be fast and synchronous.

If multiple profiles share the same prefix path, add an `Info`-level issue: "Prefix is shared with other profiles -- dependency changes affect all"

**Tests**:

- `installed_verbs_produce_no_issues`: All installed → empty vec
- `missing_verb_produces_warning`: One missing → one Warning issue
- `unknown_state_produces_stale`: No check done → Stale issue
- `skipped_verb_produces_info`: User skipped → Info issue

#### Task 4.2: Tauri health enrichment wiring Depends on [4.1]

**READ THESE BEFORE TASK**

- src/crosshook-native/src-tauri/src/commands/health.rs (`build_enriched_health_summary()` or equivalent batch health scan function)
- Task 4.1 output (the `build_dependency_health_issues` function)

**Instructions**

Files to Modify

- src/crosshook-native/src-tauri/src/commands/health.rs

In the existing `build_enriched_health_summary()` (or equivalent batch health function that runs at startup and on-demand):

- After existing health checks, query `MetadataStore::load_prefix_dep_states(profile_id)` for each profile
- Load the profile's `required_protontricks` from `TrainerSection`
- Call `build_dependency_health_issues(dep_states, required_verbs)`
- Append returned issues to the profile's health report

**CRITICAL constraint**: The MetadataStore query is synchronous via `with_conn()` -- this is fine. Do NOT add `check_installed()` (subprocess spawn) to this path. The health scan only reads cached state from SQLite.

If `MetadataStore` is disabled (`.available == false`), skip the enrichment gracefully -- `with_conn()` returns `T::default()` (empty vec).

### Phase 5a: Tauri IPC Layer

#### Task 5a.1: IPC command module Depends on [3.2, 3.3, 4.1]

**READ THESE BEFORE TASK**

- src/crosshook-native/src-tauri/src/commands/launch.rs (async spawn + streaming pattern at line 350)
- src/crosshook-native/src-tauri/src/commands/steam.rs (discovery command returning `Result<Vec<T>, String>`)
- src/crosshook-native/src-tauri/src/commands/update.rs (`UpdateProcessState` managed state pattern)
- src/crosshook-native/src-tauri/src/commands/settings.rs (settings load/save IPC pattern, `merge_settings_from_request`)

**Instructions**

Files to Create

- src/crosshook-native/src-tauri/src/commands/prefix_deps.rs

Files to Modify

- src/crosshook-native/src-tauri/src/commands/mod.rs (add `pub mod prefix_deps;`)

Define `PrefixDepsInstallState` struct wrapping the `PrefixDepsInstallLock` from Task 3.3:

```rust
pub struct PrefixDepsInstallState {
    lock: PrefixDepsInstallLock,
}
impl PrefixDepsInstallState {
    pub fn new() -> Self { Self { lock: PrefixDepsInstallLock::new() } }
}
```

Implement 4 Tauri commands (all `snake_case`):

**`detect_protontricks_binary`**: Sync command. Extract `SettingsStore` state, load settings, call `detect_binary(settings.protontricks_binary_path)`. Return `Result<BinaryDetectionResult, String>`.

**`check_prefix_dependencies`**: Async command. Params: `profile_name: String`, `prefix_path: String`, `packages: Vec<String>`. Detect binary, call `check_installed()` with 30s timeout, compare against `packages`, upsert each package's state in SQLite via `MetadataStore`. Return `Result<Vec<PrefixDependencyStatus>, String>`.

**`install_prefix_dependency`**: Async command. Params: `profile_name: String`, `prefix_path: String`, `packages: Vec<String>`, `app: AppHandle`. Acquire install lock. Detect binary. Call `install_packages()` to get `Child`. Spawn background task: read stdout/stderr line by line, emit `app.emit("prefix-dep-log", line)` per line (strip ANSI, sanitize paths), emit `app.emit("prefix-dep-complete", { succeeded, exit_code })` on exit. Upsert final state in SQLite. Release lock (via guard drop). Return `Result<(), String>` immediately after spawn.

**`get_dependency_status`**: Sync command (via `spawn_blocking`). Param: `profile_name: String`. Query `MetadataStore::load_prefix_dep_states(profile_id)`. Return `Result<Vec<PrefixDependencyStatus>, String>`.

**Settings IPC update (do this first)**: Update `commands/settings.rs` -- add `protontricks_binary_path` and `auto_install_prefix_deps` to `AppSettingsIpcData` (load DTO), `SettingsSaveRequest` (save DTO), and `merge_settings_from_request()`. This is a prerequisite for the frontend settings UI in Task 5b.4.

#### Task 5a.2: Tauri app registration Depends on [5a.1]

**READ THESE BEFORE TASK**

- src/crosshook-native/src-tauri/src/lib.rs (`.manage()` calls, `invoke_handler!` macro)

**Instructions**

Files to Modify

- src/crosshook-native/src-tauri/src/lib.rs

Add managed state before plugin setup:

```rust
.manage(commands::prefix_deps::PrefixDepsInstallState::new())
```

Add all 4 commands to the `invoke_handler!` macro:

```rust
commands::prefix_deps::detect_protontricks_binary,
commands::prefix_deps::check_prefix_dependencies,
commands::prefix_deps::install_prefix_dependency,
commands::prefix_deps::get_dependency_status,
```

Verify the full project compiles: `cargo build --manifest-path src/crosshook-native/Cargo.toml`. The command names here MUST match exactly what the frontend `invoke()` calls will use.

### Phase 5b: React Frontend

#### Task 5b.1: TypeScript types and usePrefixDeps hook Depends on [5a.1]

**READ THESE BEFORE TASK**

- src/crosshook-native/src/types/profile.ts (GameProfile TypeScript mirror)
- src/crosshook-native/src/types/settings.ts (AppSettingsData, SettingsSaveRequest split DTO)
- src/crosshook-native/src/hooks/useProtonInstalls.ts (canonical hook pattern with cleanup flag)
- docs/plans/protontricks-integration/research-ux.md (DepStatus type, hook interface)

**Instructions**

Files to Create

- src/crosshook-native/src/types/prefix-deps.ts
- src/crosshook-native/src/hooks/usePrefixDeps.ts

Files to Modify

- src/crosshook-native/src/types/profile.ts (add `required_protontricks?: string[]` to trainer section)
- src/crosshook-native/src/types/settings.ts (add `protontricks_binary_path: string`, `auto_install_prefix_deps: boolean`)

**`prefix-deps.ts`**: Define TypeScript interfaces mirroring Rust types (field names in `snake_case`):

- `BinaryDetectionResult`: `{ found: boolean, binary_path: string | null, binary_name: string, source: string }`
- `PrefixDependencyStatus`: `{ package_name: string, state: DepState, checked_at: string | null, installed_at: string | null, last_error: string | null }`
- `DepState` type: `'unknown' | 'installed' | 'missing' | 'install_failed' | 'check_failed' | 'user_skipped'`

**`usePrefixDeps.ts`**: Follow `useProtonInstalls.ts` exactly:

```typescript
export function usePrefixDeps(profileName: string, prefixPath: string) {
    const [deps, setDeps] = useState<PrefixDependencyStatus[]>([]);
    const [loading, setLoading] = useState(true);
    const [error, setError] = useState<string | null>(null);
    const [reloadVersion, setReloadVersion] = useState(0);

    useEffect(() => {
        let active = true;
        async function load() { ... invoke('get_dependency_status', { profile_name: profileName }) ... }
        void load();
        return () => { active = false; };
    }, [profileName, reloadVersion]);

    const checkDeps = useCallback(async (packages: string[]) => { ... invoke('check_prefix_dependencies', ...) ... }, [...]);
    const installDep = useCallback(async (packages: string[]) => { ... invoke('install_prefix_dependency', ...) ... }, [...]);
    const reload = useCallback(() => setReloadVersion(v => v + 1), []);

    return { deps, loading, error, checkDeps, installDep, reload };
}
```

Export types from `src/types/index.ts` if that barrel file exists.

#### Task 5b.2: DependencyStatusBadge and PrefixDepsPanel scaffold Depends on [5b.1]

**READ THESE BEFORE TASK**

- docs/plans/protontricks-integration/research-ux.md (DependencyStatusBadge design, DepStatus visual mapping, ConsoleDrawer integration)
- src/crosshook-native/src/components/pages/ProfilesPage.tsx (where PrefixDepsPanel will be embedded)
- src/crosshook-native/src/styles/variables.css (CSS variables for theming)

**Instructions**

Files to Create

- src/crosshook-native/src/components/PrefixDepsPanel.tsx

Files to Modify

- src/crosshook-native/src/components/pages/ProfilesPage.tsx (embed PrefixDepsPanel in a collapsible section)

**`PrefixDepsPanel.tsx`**: Create a new component (do NOT extend HealthBadge):

- `DependencyStatusBadge` sub-component: renders a status chip per package using `crosshook-status-chip` CSS class. Map `DepState` to visual: `installed` → green, `missing` → amber, `install_failed` → red, `unknown`/`check_failed` → gray, `user_skipped` → muted
- Package list showing each `required_protontricks` entry with its status badge
- [Check Now] button calling `checkDeps()`
- [Install All Missing] button calling `installDep()` for packages with state `missing` or `install_failed`
- Per-package [Install] / [Retry] buttons
- Loading state with indeterminate progress indicator during check/install operations
- Tauri event listeners for `prefix-dep-log` and `prefix-dep-complete` using `listen()` from `@tauri-apps/api/event` with proper cleanup

Embed in `ProfilesPage.tsx` inside a collapsible section (similar to existing collapsibles), only visible when `profile.trainer?.required_protontricks?.length > 0`.

**CSS**: Use BEM-like `crosshook-prefix-deps-*` classes. Use CSS variables from `variables.css` for colors.

#### Task 5b.3: Install flow and confirmation modal Depends on [5b.2]

**READ THESE BEFORE TASK**

- docs/plans/protontricks-integration/research-ux.md (install confirmation dialog, pre-launch gate modal)
- docs/plans/protontricks-integration/research-business.md (BR-6: soft-block, BR-7: pre-launch gate, BR-9: skip and remember)

**Instructions**

Files to Modify

- src/crosshook-native/src/components/PrefixDepsPanel.tsx

Add install flow interactions:

- **Install confirmation dialog**: Before calling `installDep()`, show modal listing packages to install with human-readable labels where available. Include slow-install warning ("Installation may take several minutes and requires internet access"). Show [Install] / [Cancel] buttons.
- **Pre-launch gate modal**: Intercept the launch flow in `ProfilesPage.tsx` (the launch button handler) or `useLaunchState.ts` -- when user clicks Launch and there are missing deps, show gate modal with: list of missing packages, [Install + Launch] (install then auto-launch), [Skip and Launch] (proceed without installing, mark as `user_skipped`), [Cancel]. Check the `handleLaunch` or equivalent callback in `ProfilesPage.tsx` to find the insertion point.
- **Per-package retry**: For `install_failed` state, show [Retry] button that re-invokes `installDep([package])`.
- **Global install lock UI**: When `PrefixDepsInstallLock` is active (check via separate query or listen for `prefix-dep-complete`), disable all install/retry buttons across the UI. Show "Installing..." status.
- **Console output**: During install, display streaming log output from `prefix-dep-log` events in an inline log area or the existing ConsoleDrawer if appropriate.

#### Task 5b.4: Settings UI, import preview, and scroll registration Depends on [5b.1]

**READ THESE BEFORE TASK**

- src/crosshook-native/src/components/SettingsPanel.tsx (existing settings UI structure)
- src/crosshook-native/src/hooks/useScrollEnhance.ts (SCROLLABLE selector -- CLAUDE.md hard rule)
- src/crosshook-native/crates/crosshook-core/src/profile/exchange.rs (CommunityImportPreview struct)
- docs/plans/protontricks-integration/research-business.md (BR-12: community trust disclosure)

**Instructions**

Files to Modify

- src/crosshook-native/src/components/SettingsPanel.tsx
- src/crosshook-native/src/hooks/useScrollEnhance.ts
- src/crosshook-native/crates/crosshook-core/src/profile/exchange.rs (Rust file -- modify `CommunityImportPreview`)

**Settings UI**: In `SettingsPanel.tsx`, add a "Prefix Dependencies" section:

- Text input for `protontricks_binary_path` with [Browse] button (if file picker is available) and live validation indicator (green check if executable exists, gray if empty/auto-detect, red X if path set but not executable)
- Toggle for `auto_install_prefix_deps` (label: "Auto-install prefix dependencies on first launch")
- Help note: "If left empty, CrossHook will auto-detect winetricks/protontricks from PATH"
- If only Flatpak protontricks is found, show info note about sandbox restrictions

**Community import preview** (Rust + React): In `exchange.rs`, add `required_prefix_deps: Vec<String>` to `CommunityImportPreview` struct. In the community import preview React component, show the list of required prefix deps with explicit trust notice: "This community profile requires the following WINE prefix dependencies to be installed: [list]. These packages will be downloaded from the internet."

**Scroll registration**: If `PrefixDepsPanel` introduces a scrollable container (`overflow-y: auto`), add its CSS selector to the `SCROLLABLE` constant in `useScrollEnhance.ts`. Also add `overscroll-behavior: contain` to the container's CSS. This is a CLAUDE.md hard rule -- missing it causes dual-scroll jank on WebKitGTK.

## Advice

- **4 decisions are resolved**: Field name (`required_protontricks`), allowlist (structural regex hard gate + static advisory), `user_skipped` reset (per-package action + 24h TTL auto-reset), TTL (24h). See "Resolved Decisions" section above. Do not revisit mid-implementation.

- **Phase 2 (Storage) is the highest-leverage early deliverable**: The migration + store API unblocks all downstream work. Define `PrefixDependencyStateRow` and store function signatures first, then implement -- downstream tasks can code against the signatures.

- **Security is not a Phase 5 afterthought -- it's Phase 3's primary concern**: The `runner.rs` and `validation.rs` implementations ARE the security surface. All 7 CRITICAL findings (S-01, S-02, S-03, S-06, S-19, S-22, S-27) must be addressed in Phase 3. Do not proceed to IPC/UI work until the runner's `Command` construction is reviewed for per-verb `.arg()`, `--` separator, and input validation.

- **`env_clear()` must NOT be used for winetricks/protontricks**: This is the single most common mistake. All existing Proton commands in the codebase call `env_clear()`. Protontricks/winetricks are Python/shell scripts requiring HOME, USER, PATH, XDG_RUNTIME_DIR. Use `apply_host_environment()` to provide the POSIX env they need.

- **Store placement is `metadata/prefix_deps_store.rs`**: Not `prefix_deps/store.rs`. All other stores (health_store, offline_store, cache_store) live in `metadata/`. Breaking this convention creates a confusing precedent.

- **Health path must be read-only**: `build_enriched_health_summary()` runs at startup. The dependency enrichment (Task 4.2) must only read SQLite -- never spawn a subprocess. If someone adds `Command::new()` to health.rs, reject it immediately.

- **`exchange.rs` in Task 5b.4 is a Rust file**: The community import preview trust disclosure requires modifying `CommunityImportPreview` in `crosshook-core/src/profile/exchange.rs`. This Rust change requires recompilation, even though 5b.4 is primarily a frontend task. Plan accordingly.

- **Prefix path normalization is critical**: `RuntimeSection.prefix_path` may or may not contain a trailing `pfx/` subdirectory. Always call `resolve_wine_prefix_path()` before setting `WINEPREFIX`. For `STEAM_COMPAT_DATA_PATH`, use the parent directory (without `pfx/`). Inconsistent path handling creates duplicate rows in `prefix_dependency_state`.

- **Lock on prefix_path, not PID**: The concurrent install lock should hold the active `prefix_path` string (not the child PID). This lets the UI know which prefix is being installed and disable buttons appropriately. The PID is not useful for the "already installing" check.

- **No cancellation in v1**: Killing winetricks mid-install can corrupt the WINE prefix. Show a slow-install warning in the confirmation dialog instead. Cancel support can be added in a future version with prefix backup/restore.

- **Test without real winetricks**: CI environments won't have winetricks installed. Use `FakeRunner` (mock) for all unit tests. Integration tests with real winetricks are manual/optional. The `ScopedCommandSearchPath` pattern from `launch/test_support.rs` enables PATH manipulation for detection tests.
