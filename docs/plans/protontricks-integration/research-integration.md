# Integration Research: protontricks-integration

**Research date**: 2026-04-03
**Author**: integration-researcher

---

## Overview

CrossHook already has a complete Tauri IPC command infrastructure, a SQLite metadata DB at schema v14, TOML-based profile/settings stores, and process execution patterns (via `tokio::process::Command`) for Proton, game, and trainer launch. Protontricks/winetricks integration slots into these existing patterns: a new `prefix_deps` module in `crosshook-core` manages CLI process invocation, a `prefix_dependency_state` table (migration v14→v15) persists package installation state, and four new Tauri `#[tauri::command]` handlers wire the frontend. All subprocess, IPC, and error-handling conventions are directly reusable.

---

## Relevant Files

- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src-tauri/src/lib.rs` — Tauri app initialization, `invoke_handler` registration, `manage()` state calls
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src-tauri/src/commands/` — all existing command modules (install, launch, profile, settings, steam, update, health, onboarding)
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/launch/runtime_helpers.rs` — `resolve_umu_run_path()` at line 301: the canonical pattern for binary detection (walk `$PATH`, check execute bit); `apply_host_environment()`: host env forwarding; `apply_runtime_proton_environment()`: sets `WINEPREFIX`/`STEAM_COMPAT_DATA_PATH`
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/launch/script_runner.rs` — command construction patterns for Proton game/trainer launch
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/metadata/mod.rs` — `MetadataStore` struct, `with_conn()` pattern, all store method registrations
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/metadata/migrations.rs` — all 14 SQLite migrations; schema v14 is the current target; next new table goes in `migrate_14_to_15`
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/metadata/models.rs` — row structs, error types, constants (MAX_HISTORY_LIST_LIMIT, etc.)
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/profile/models.rs` — `GameProfile`, `TrainerSection`, `RuntimeSection`, `LaunchSection`; TOML field naming
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/settings/mod.rs` — `AppSettingsData`, `SettingsStore`; where `protontricks_binary_path` field must be added
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/install/service.rs` — `build_install_command()` pattern: `new_direct_proton_command`, `apply_host_environment`, `apply_runtime_proton_environment`, `attach_log_stdio`
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/steam/proton.rs` — Proton discovery, `CompatToolMappings`, `discover_compat_tools_with_roots`
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src-tauri/src/commands/steam.rs` — `list_proton_installs`, `auto_populate_steam` — model for new prefix-deps detection commands
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/runtime-helpers/steam-launch-helper.sh` — shell runtime helper showing full WINE env clearing pattern (`unset WINESERVER WINELOADER WINEDLLPATH ...`); `WINEPREFIX=$compatdata/pfx` convention

---

## API Endpoints / Tauri Commands

### Existing Commands (reuse patterns from these)

| Command (snake_case)              | Location                    | Notes                                                            |
| --------------------------------- | --------------------------- | ---------------------------------------------------------------- |
| `install_game`                    | `commands/install.rs:30`    | Pattern for blocking subprocess + log path                       |
| `launch_game`                     | `commands/launch.rs:183`    | Pattern for async spawn + log stream + metadata recording        |
| `launch_trainer`                  | `commands/launch.rs:267`    | Same as above; trainer-specific                                  |
| `update_game` / `cancel_update`   | `commands/update.rs`        | Pattern for cancellable async operation via `Mutex<Option<u32>>` |
| `list_proton_installs`            | `commands/steam.rs:35`      | Pattern for discovery commands with diagnostics Vec              |
| `check_readiness`                 | `commands/onboarding.rs:10` | Pattern for system readiness check returning structured result   |
| `check_offline_readiness`         | `commands/offline.rs`       | Pattern for per-profile async readiness check                    |
| `settings_load` / `settings_save` | `commands/settings.rs`      | Pattern for TOML settings round-trip                             |

### New Commands to Add (protontricks-integration)

| Command (snake_case)           | Input                                                                       | Output                          | Notes                                                                                           |
| ------------------------------ | --------------------------------------------------------------------------- | ------------------------------- | ----------------------------------------------------------------------------------------------- |
| `detect_protontricks_binary`   | none                                                                        | `Option<String>` path           | Walk `$PATH` for `protontricks`; follow `resolve_umu_run_path()` pattern exactly                |
| `detect_winetricks_binary`     | none                                                                        | `Option<String>` path           | Walk `$PATH` for `winetricks`; same pattern                                                     |
| `check_prefix_dependencies`    | `{ profile_name, prefix_path, steam_app_id?, packages: Vec<String> }`       | `Vec<PackageDependencyStatus>`  | Run `winetricks list-installed` with `WINEPREFIX` set; parse stdout                             |
| `install_prefix_dependency`    | `{ profile_name, prefix_path, package, steam_app_id?, protontricks_path? }` | streaming `String` events       | Spawn protontricks/winetricks, emit `prefix-dep-log` events, emit `prefix-dep-complete` on exit |
| `get_prefix_dependency_states` | `{ profile_name }`                                                          | `Vec<PrefixDependencyStateRow>` | Read from SQLite `prefix_dependency_state` table                                                |

All new commands must be registered in `src-tauri/src/lib.rs` invoke_handler, follow `snake_case` naming, and use Serde-serializable input/output types.

---

## Database Schema

### Current Schema (v14)

The metadata DB lives at `~/.local/share/crosshook/metadata.db` (from `MetadataStore::try_new()` at `metadata/mod.rs:55-59`).

| Table                            | Key Columns                                                                                         | Migration            |
| -------------------------------- | --------------------------------------------------------------------------------------------------- | -------------------- |
| `profiles`                       | `profile_id TEXT PK`, `current_filename`, `game_name`, `launch_method`, `is_favorite`, `deleted_at` | v0→v1                |
| `profile_name_history`           | `profile_id FK`, `old_name`, `new_name`, `source`                                                   | v0→v1                |
| `launchers`                      | `launcher_id PK`, `profile_id FK`, `launcher_slug`, `drift_state`                                   | v2→v3                |
| `launch_operations`              | `operation_id PK`, `profile_id FK`, `launch_method`, `status`, `exit_code`, `diagnostic_json`       | v2→v3                |
| `community_taps`                 | `tap_id PK`, `tap_url`, `tap_branch`, `local_path`, `last_head_commit`                              | v3→v4                |
| `community_profiles`             | `tap_id FK`, `relative_path`, `game_name`, `trainer_name`, `proton_version`                         | v3→v4                |
| `external_cache_entries`         | `cache_id PK`, `source_url`, `cache_key`, `payload_json`, `expires_at`                              | v3→v4                |
| `collections`                    | `collection_id PK`, `name UNIQUE`                                                                   | v3→v4                |
| `collection_profiles`            | `(collection_id, profile_id) PK`                                                                    | v3→v4                |
| `health_snapshots`               | `profile_id PK FK`, `status`, `issue_count`, `checked_at`                                           | v5→v6, rebuilt v6→v7 |
| `version_snapshots`              | `profile_id FK`, `steam_app_id`, `steam_build_id`, `trainer_file_hash`, `status`                    | v8→v9                |
| `bundled_optimization_presets`   | `preset_id PK`, `vendor`, `mode`, `option_ids_json`                                                 | v9→v10               |
| `profile_launch_preset_metadata` | `(profile_id, preset_name) PK`, `origin`                                                            | v9→v10               |
| `config_revisions`               | `profile_id FK`, `source`, `content_hash`, `snapshot_toml`, `is_last_known_working`                 | v10→v11              |
| `optimization_catalog`           | `id PK`, `applies_to_method`, `env_json`, `wrappers_json`, `required_binary`, `category`            | v11→v12              |
| `trainer_hash_cache`             | `profile_id FK`, `file_path`, `sha256_hash`, `verified_at`                                          | v12→v13              |
| `offline_readiness_snapshots`    | `profile_id PK FK`, `readiness_state`, `readiness_score`, `trainer_type`                            | v12→v13              |
| `community_tap_offline_state`    | `tap_id PK FK`, `has_local_clone`, `last_sync_at`                                                   | v12→v13              |
| `game_image_cache`               | `cache_id PK`, `steam_app_id`, `image_type`, `source`, `file_path`, `expires_at`                    | v13→v14              |

### New Table: `prefix_dependency_state` (migration v14→v15)

```sql
CREATE TABLE IF NOT EXISTS prefix_dependency_state (
    id               INTEGER PRIMARY KEY AUTOINCREMENT,
    profile_id       TEXT NOT NULL REFERENCES profiles(profile_id) ON DELETE CASCADE,
    package_name     TEXT NOT NULL,
    prefix_path      TEXT NOT NULL,         -- canonical WINEPREFIX path (resolved)
    state            TEXT NOT NULL DEFAULT 'unknown',
        -- values: 'unknown' | 'installed' | 'missing' | 'install_failed' | 'check_failed'
    checked_at       TEXT,                  -- ISO-8601 UTC, NULL = never checked
    installed_at     TEXT,                  -- ISO-8601 UTC, NULL = not recorded as installed
    last_error       TEXT,                  -- last error message, NULL if none
    created_at       TEXT NOT NULL,
    updated_at       TEXT NOT NULL
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_prefix_dep_state_profile_package_prefix
    ON prefix_dependency_state(profile_id, package_name, prefix_path);

CREATE INDEX IF NOT EXISTS idx_prefix_dep_state_profile_id
    ON prefix_dependency_state(profile_id);
```

**Persistence classification:**

- `prefix_dependency_state` table: SQLite metadata DB (operational/cache metadata, not user-editable)
- `required_protontricks` field in `TrainerSection`: TOML profile (user-editable, committed to profile file)
- `protontricks_binary_path` in `AppSettingsData`: TOML settings (user-editable, `~/.config/crosshook/settings.toml`)

### TOML Profile Changes

Add to `TrainerSection` in `profile/models.rs`:

```rust
#[serde(default, skip_serializing_if = "Vec::is_empty")]
pub required_protontricks: Vec<String>,
```

This means the TOML profile gains a field under `[trainer]`:

```toml
[trainer]
path = "/path/to/trainer.exe"
required_protontricks = ["vcrun2019", "dotnet48"]
```

### TOML Settings Changes

Add to `AppSettingsData` in `settings/mod.rs`:

```rust
/// Path to the protontricks binary. Empty = auto-detect from PATH.
#[serde(default, skip_serializing_if = "String::is_empty")]
pub protontricks_binary_path: String,
```

---

## External Services

### Protontricks CLI

**Executable**: `protontricks` (native package) or `flatpak run com.github.Matoking.protontricks` (Flatpak)
**Detection**: walk `$PATH` using `resolve_umu_run_path()` pattern in `launch/runtime_helpers.rs:301`

```
protontricks [OPTIONS] <APPID> <WINETRICKS_VERBS...>
```

Key flags:

- `--no-bwrap`: disable bubblewrap sandboxing (required workaround on some systems)
- `-q`: non-interactive/unattended (required for automation)

Key environment variables:

- `STEAM_DIR`: override Steam installation directory
- `STEAM_COMPAT_DATA_PATH`: override prefix path (sets `WINEPREFIX=$STEAM_COMPAT_DATA_PATH/pfx`)
- `WINETRICKS`: path to custom winetricks script

Exit codes: `0` = success, non-zero = failure, `141` = SIGPIPE.

**Flatpak restriction**: Flatpak protontricks has sandbox restrictions that break multi-library setups. The native package is strongly preferred. CrossHook should detect both paths and prefer the native binary.

### Winetricks CLI

**Executable**: `winetricks`
**Usage**: direct invocation when protontricks is not available or for `list-installed` queries

Key commands:

- `WINEPREFIX=/path/to/prefix winetricks list-installed` — stdout = space-separated installed verbs
- `WINEPREFIX=/path/to/prefix winetricks -q <verb>` — install verb non-interactively

**Do NOT parse `$WINEPREFIX/winetricks.log`** — internal file, unstable format. Use `list-installed` stdout for bootstrapping, then maintain state in SQLite.

### Network Dependencies (winetricks)

Winetricks downloads packages at runtime from Microsoft CDNs (vcrun2019, dotnet48) or other sources. These downloads require network access. CrossHook's offline mode check should warn when `required_protontricks` packages are not yet installed.

---

## Internal Services

### crosshook-core Modules

| Module                    | Relevance                                                                                                          |
| ------------------------- | ------------------------------------------------------------------------------------------------------------------ |
| `launch::runtime_helpers` | Binary detection pattern (`resolve_umu_run_path`), host env application, `WINEPREFIX` resolution                   |
| `launch::request`         | `LaunchRequest` struct; `RuntimeLaunchConfig.prefix_path` contains the WINE prefix path needed for winetricks      |
| `install::service`        | `build_install_command()` pattern: direct Proton command construction, log stdio attachment                        |
| `metadata::mod`           | `MetadataStore`, `with_conn()` pattern, migration runner; add new store methods here                               |
| `metadata::migrations`    | `run_migrations()`, `migrate_13_to_14()` as template for v14→v15                                                   |
| `profile::models`         | `TrainerSection`, `RuntimeSection`; TOML field naming conventions (snake_case, `#[serde(default)]`)                |
| `settings::mod`           | `AppSettingsData` with `#[serde(default)]` fields; `protontricks_binary_path` belongs here                         |
| `steam::proton`           | `discover_compat_tools_with_roots()`, compat tool discovery; prefix paths under `steamapps/compatdata/<APPID>/pfx` |

### IPC Patterns

**Command registration** (`src-tauri/src/lib.rs`):

```rust
// Add to invoke_handler! macro:
commands::prefix_deps::detect_protontricks_binary,
commands::prefix_deps::check_prefix_dependencies,
commands::prefix_deps::install_prefix_dependency,
commands::prefix_deps::get_prefix_dependency_states,
```

**Managed state** for concurrent-install prevention (from `update` command pattern):

```rust
// In lib.rs setup:
.manage(commands::prefix_deps::PrefixDepsInstallState::new())
```

**Async streaming** (from `commands/launch.rs:350-383`):

- Spawn `tokio::process::Command`, pipe stdout+stderr
- Emit `prefix-dep-log` events per line via `app.emit(...)`
- Emit `prefix-dep-complete` with exit code on process exit

**Blocking DB operations** (from all existing commands):

```rust
tauri::async_runtime::spawn_blocking(move || {
    metadata_store.upsert_prefix_dependency_state(...)
}).await?
```

### Environment Variable Handling for Protontricks

When invoking protontricks, the command must receive the correct environment. Follow `apply_host_environment()` in `runtime_helpers.rs:153-167` for base environment, then add:

- `STEAM_DIR`: from Steam discovery (via `discover_steam_root_candidates`)
- `STEAM_COMPAT_DATA_PATH`: the `compatdata_path` from the profile's `SteamSection` or `RuntimeSection.prefix_path`
- `WINEPREFIX`: resolved via `resolve_wine_prefix_path()` (adds `/pfx` suffix if needed)

**Do not use `env_clear()`** for protontricks invocations — unlike Proton, protontricks and winetricks require a real `HOME`, `USER`, `PATH`, `DISPLAY`/`WAYLAND_DISPLAY`, and `XDG_RUNTIME_DIR` from the host environment. Contrast with `new_direct_proton_command()` which calls `env_clear()`.

---

## Configuration

### File Paths

| File                                        | Purpose                                                                         |
| ------------------------------------------- | ------------------------------------------------------------------------------- |
| `~/.config/crosshook/settings.toml`         | `AppSettingsData` including new `protontricks_binary_path`                      |
| `~/.config/crosshook/profiles/<name>.toml`  | `GameProfile` including `[trainer] required_protontricks`                       |
| `~/.local/share/crosshook/metadata.db`      | SQLite DB; new `prefix_dependency_state` table in v15                           |
| `~/.local/share/crosshook/prefixes/<slug>/` | Default WINE prefix root (`DEFAULT_PREFIX_ROOT_SEGMENT` = `crosshook/prefixes`) |

### Prefix Path Convention

CrossHook uses two conventions:

1. **Steam prefix** (via `steam_applaunch` / `SteamSection`): `$STEAM_COMPAT_DATA_PATH/pfx/` where `STEAM_COMPAT_DATA_PATH = profile.steam.compatdata_path`
2. **Direct Proton prefix** (via `proton_run` / `RuntimeSection`): `profile.runtime.prefix_path` — the `pfx/` subdirectory may or may not be present (resolved by `resolve_wine_prefix_path()` in `runtime_helpers.rs:198`)

Protontricks uses `STEAM_COMPAT_DATA_PATH` (the parent of `pfx/`), not `WINEPREFIX` (the `pfx/` directory itself). For winetricks direct invocation, `WINEPREFIX` = the `pfx/` directory. Ensure the correct path is forwarded for each tool.

### Binary Discovery Priority

1. If `AppSettingsData.protontricks_binary_path` is non-empty and the file is executable: use it
2. Walk `$PATH` entries (using `DEFAULT_HOST_PATH` fallback) looking for `protontricks`
3. If not found: return `None`; UI shows "protontricks not installed" with setup guidance

---

## Gotchas and Edge Cases

- **Do not add the `which` crate**: `resolve_umu_run_path()` in `runtime_helpers.rs:301-312` already implements PATH walking with executable check. Replicate, do not add a new crate.
- **env_clear() must NOT be used for protontricks**: All existing Proton commands call `env_clear()` before rebuilding a clean env. Protontricks is a Python script that needs a full POSIX environment including `HOME`, `USER`, `XDG_RUNTIME_DIR`, etc.
- **Prefix path ambiguity**: `RuntimeSection.prefix_path` may or may not contain a trailing `pfx/` subdirectory. Always call `resolve_wine_prefix_path()` to normalize before setting `WINEPREFIX`; use the parent for `STEAM_COMPAT_DATA_PATH`.
- **winetricks list-installed does not list settings verbs** (GitHub Issue #936): This is not a concern for trainer dependencies (vcrun2019, dotnet48, etc.) but must be documented.
- **list-installed is not authoritative ongoing truth**: Use it only for bootstrap/sync. SQLite `prefix_dependency_state` is the runtime source of truth; upsert on each install success.
- **Flatpak protontricks breaks multi-library setups**: Detect and prefer the native binary; warn the user if only Flatpak is found.
- **WINE session inheritance**: The `steam-launch-helper.sh` script unsets a large list of WINE/Proton env vars before re-invoking Proton (see `run_proton_with_clean_env()`). Protontricks invoked inside CrossHook (which is itself running under a Proton session for trainers) must similarly not inherit stale `WINEPREFIX`, `WINESERVER`, etc. Use a fresh env.
- **Migration must guard against column existence**: Some migrations (`migrate_7_to_8`) use `PRAGMA table_info` to check for column existence before dropping. Follow this pattern in v14→v15 if adding columns to existing tables.
- **Concurrent install prevention**: The `update` module uses `Mutex<Option<u32>>` to hold the child PID and prevent concurrent updates. The same pattern is required for `install_prefix_dependency` to prevent running two protontricks processes against the same prefix simultaneously.
- **SQLite `prefix_path` should be canonical**: Call `std::fs::canonicalize()` on the prefix path before storing in the DB to avoid duplicate rows from path aliasing (symlinks, relative paths, etc.).

---

## Relevant Docs

- `/home/yandy/Projects/github.com/yandy-r/crosshook/docs/plans/protontricks-integration/research-external.md` — External API research: full protontricks/winetricks CLI reference, Tokio process patterns
- `/home/yandy/Projects/github.com/yandy-r/crosshook/docs/plans/protontricks-integration/research-technical.md` — Technical architecture design: component diagram, data models, IPC design
- `/home/yandy/Projects/github.com/yandy-r/crosshook/AGENTS.md` — Project architecture overview
- `/home/yandy/Projects/github.com/yandy-r/crosshook/CLAUDE.md` — Agent rules, IPC naming conventions, Tauri/Rust patterns
