# Integration Research: Trainer Onboarding

## Overview

The trainer-onboarding feature integrates with three backend systems: the Tauri IPC command layer, the TOML settings store, and the Steam discovery services. No new SQLite migration is needed — all persistent onboarding state lives in `settings.toml`. The three new commands (`check_readiness`, `dismiss_onboarding`, `get_trainer_guidance`) are thin wrappers around existing core functions, following the exact patterns established in `commands/steam.rs` and `commands/settings.rs`.

---

## Tauri IPC Commands

### Existing Relevant Commands

| Command                             | File                      | Signature                                                                       | Notes                                                                                  |
| ----------------------------------- | ------------------------- | ------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------- |
| `settings_load`                     | `commands/settings.rs:16` | `fn(State<'_, SettingsStore>) -> Result<AppSettingsData, String>`               | Frontend calls this on startup; will return `onboarding_completed` once field is added |
| `settings_save`                     | `commands/settings.rs:21` | `fn(AppSettingsData, State<'_, SettingsStore>) -> Result<(), String>`           | Full-struct overwrite; `dismiss_onboarding` prefers a targeted load-mutate-save        |
| `default_steam_client_install_path` | `commands/steam.rs:9`     | `fn() -> String`                                                                | Returns Steam root path from env or fallback — no `Result`, no state param             |
| `list_proton_installs`              | `commands/steam.rs:35`    | `fn(Option<String>) -> Result<Vec<ProtonInstall>, String>`                      | Calls `discover_steam_root_candidates` + `discover_compat_tools`                       |
| `auto_populate_steam`               | `commands/steam.rs:52`    | `async fn(SteamAutoPopulateRequest) -> Result<SteamAutoPopulateResult, String>` | Wraps blocking call in `spawn_blocking`                                                |
| `batch_validate_profiles`           | `commands/health.rs`      | `fn(State<'_, ProfileStore>, State<'_, MetadataStore>) -> ...`                  | Multi-state pattern example                                                            |

### Registration Pattern

All commands are registered in `src-tauri/src/lib.rs:123` inside `invoke_handler!`:

```rust
// commands/mod.rs — add:
pub mod onboarding;

// lib.rs invoke_handler — add three entries:
commands::onboarding::check_readiness,
commands::onboarding::dismiss_onboarding,
commands::onboarding::get_trainer_guidance,
```

### State Access Pattern

State is declared as a parameter and Tauri injects it at runtime. The convention across every command file:

```rust
use tauri::State;

// Sync command, one state parameter
#[tauri::command]
pub fn dismiss_onboarding(settings_store: State<'_, SettingsStore>) -> Result<(), String> {
    let mut settings = settings_store.load().map_err(|e| e.to_string())?;
    settings.onboarding_completed = true;
    settings_store.save(&settings).map_err(|e| e.to_string())
}

// No state, no Result — static content (matches get_trainer_guidance pattern)
#[tauri::command]
pub fn get_trainer_guidance() -> TrainerGuidanceContent { ... }

// Sync command, no state, returns Result — matches check_readiness pattern
#[tauri::command]
pub fn check_readiness() -> Result<ReadinessCheckResult, String> { ... }
```

Error mapping convention: `map_err(|e| e.to_string())` throughout all command files.

### Startup Event Pattern (for `onboarding-check`)

The feature spec calls for emitting an `onboarding-check` event at startup. The exact pattern is already in `lib.rs:59-70` (auto-load-profile event):

```rust
// In setup closure:
let app_handle = app.handle().clone();
tauri::async_runtime::spawn(async move {
    sleep(Duration::from_millis(350)).await;
    if let Err(error) = app_handle.emit("onboarding-check", &payload) {
        tracing::warn!(%error, "failed to emit onboarding-check event");
    }
});
```

The payload can be any `Serialize` type (e.g., `bool` for `onboarding_completed` or a `ReadinessCheckResult`).

---

## Database Schema

### Current Migration Version: **v10**

Migrations in `crates/crosshook-core/src/metadata/migrations.rs` — each migration is guarded by `if version < N { ... }` and updates `user_version` pragma immediately after.

### Tables Relevant to Onboarding

#### `profiles` (v1)

```sql
CREATE TABLE profiles (
    profile_id          TEXT PRIMARY KEY,
    current_filename    TEXT NOT NULL UNIQUE,  -- profile "name" (TOML filename sans .toml)
    current_path        TEXT NOT NULL,
    game_name           TEXT,
    launch_method       TEXT,
    content_hash        TEXT,
    is_favorite         INTEGER NOT NULL DEFAULT 0,
    source_profile_id   TEXT REFERENCES profiles(profile_id),
    deleted_at          TEXT,
    created_at          TEXT NOT NULL,
    updated_at          TEXT NOT NULL
);
```

**Onboarding relevance**: The empty-state banner on ProfilesPage can check `profile_store.list()` (TOML) instead of querying SQLite — avoids the `MetadataStore` unavailability risk.

#### `launch_operations` (v3)

```sql
CREATE TABLE launch_operations (
    operation_id    TEXT PRIMARY KEY,
    profile_id      TEXT REFERENCES profiles(profile_id),
    profile_name    TEXT,
    launch_method   TEXT NOT NULL,
    status          TEXT NOT NULL DEFAULT 'started',
    exit_code       INTEGER,
    signal          INTEGER,
    log_path        TEXT,
    diagnostic_json TEXT,
    severity        TEXT,
    failure_mode    TEXT,
    started_at      TEXT NOT NULL,
    finished_at     TEXT
);
```

**Onboarding relevance**: The feature spec explicitly avoids querying `launch_operations` for the `game_launched_once` check — use `is_dir()` on `steamapps/compatdata/*/pfx` instead. `MetadataStore` may be disabled.

#### `health_snapshots` (v6, schema stabilized at v7)

```sql
CREATE TABLE health_snapshots (
    profile_id  TEXT PRIMARY KEY REFERENCES profiles(profile_id) ON DELETE CASCADE,
    status      TEXT NOT NULL,
    issue_count INTEGER NOT NULL DEFAULT 0,
    checked_at  TEXT NOT NULL
);
```

**Onboarding relevance**: Post-onboarding health check (Phase 4) can write a snapshot here for the newly created profile.

#### `version_snapshots` (v9)

```sql
CREATE TABLE version_snapshots (
    id                INTEGER PRIMARY KEY AUTOINCREMENT,
    profile_id        TEXT NOT NULL REFERENCES profiles(profile_id) ON DELETE CASCADE,
    steam_app_id      TEXT NOT NULL DEFAULT '',
    steam_build_id    TEXT,
    trainer_version   TEXT,
    trainer_file_hash TEXT,
    human_game_ver    TEXT,
    status            TEXT NOT NULL DEFAULT 'untracked',
    checked_at        TEXT NOT NULL
);
```

**Onboarding relevance**: Phase 4 records initial `trainer_file_hash` in `version_snapshots` on profile creation. No migration needed — table exists.

### MetadataStore Unavailability Pattern

`MetadataStore` can fail to open (symlink, permissions, corrupt DB). `lib.rs:32-35` handles this:

```rust
let metadata_store = MetadataStore::try_new().unwrap_or_else(|error| {
    tracing::warn!(%error, "metadata store unavailable — SQLite features disabled");
    MetadataStore::disabled()
});
```

`MetadataStore::disabled()` returns a struct with `available: false`. All methods on a disabled store return `Ok(Default::default())` via `with_conn()` at `metadata/mod.rs:85-101`. **`check_readiness()` must not depend on `MetadataStore`** — all four checks are pure filesystem operations.

### No New Migration Needed

Onboarding state persists in `settings.toml` via `onboarding_completed: bool`. The next migration (v11) would only be needed if SQLite storage were added later.

---

## Settings Store

### `AppSettingsData` — Current Structure (`settings/mod.rs:19-25`)

```rust
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize, Default)]
#[serde(default)]
pub struct AppSettingsData {
    pub auto_load_last_profile: bool,
    pub last_used_profile: String,
    pub community_taps: Vec<CommunityTapSubscription>,
}
```

### Adding `onboarding_completed`

```rust
pub struct AppSettingsData {
    pub auto_load_last_profile: bool,
    pub last_used_profile: String,
    pub community_taps: Vec<CommunityTapSubscription>,
    pub onboarding_completed: bool,  // NEW — serde(default) on struct = false when absent
}
```

The `#[serde(default)]` on the struct guarantees backward compatibility: existing `settings.toml` files without the field deserialize to `false` (the `bool` default). No migration or fallback needed.

### Read/Write Pattern

```rust
// File: ~/.config/crosshook/settings.toml (resolved via BaseDirs::config_dir())

// Load — returns AppSettingsData::default() if file missing
pub fn load(&self) -> Result<AppSettingsData, SettingsStoreError>

// Save — overwrites entire file with TOML pretty-print
pub fn save(&self, settings: &AppSettingsData) -> Result<(), SettingsStoreError>
```

**Critical**: `save()` is a full-struct overwrite. `dismiss_onboarding` must load first, mutate the single flag, then save — not construct a new default struct. Otherwise it would erase `auto_load_last_profile`, `last_used_profile`, and `community_taps`.

### How Settings Are Loaded at Startup

`SettingsStore` is initialized in `lib.rs:20-23` and injected as managed state. Frontend reads it via `settings_load` (`commands/settings.rs:16`). `App.tsx` listens for the `auto-load-profile` event emitted from startup and can similarly listen for `onboarding-check`.

---

## Steam Discovery Services

### Functions to Reuse in `check_system_readiness()`

#### `discover_steam_root_candidates` (`steam/discovery.rs:11`)

```rust
pub fn discover_steam_roots_candidates(
    steam_client_install_path: impl AsRef<Path>,
    diagnostics: &mut Vec<String>,
) -> Vec<PathBuf>
```

- Pass `""` as the path to use home-directory fallbacks
- Returns `Vec<PathBuf>` of roots that have `steamapps/` as a subdirectory
- **Readiness check**: `steam_installed` passes when `!candidates.is_empty()`
- Handles Flatpak Steam at `~/.var/app/com.valvesoftware.Steam/data/Steam` automatically
- The existing `default_steam_client_install_path()` Tauri command (not a core function) applies the same env-var + fallback logic; in `check_system_readiness()` pass `""` and let the function discover via `$HOME`

#### `discover_compat_tools` (`steam/proton.rs:24`)

```rust
pub fn discover_compat_tools(
    steam_root_candidates: &[PathBuf],
    diagnostics: &mut Vec<String>,
) -> Vec<ProtonInstall>
```

- Takes the output of `discover_steam_root_candidates`
- Scans official (`steamapps/common/`), custom (`compatibilitytools.d/`), and system compat tool roots
- **Readiness check**: `proton_available` passes when `!installs.is_empty()`
- `ProtonInstall` struct has: `name: String`, `path: PathBuf`, `is_official: bool`, `aliases: Vec<String>`

#### Compatdata Detection (Filesystem scan — no existing function)

No existing utility scans all `steamapps/compatdata/*/pfx` dirs. The readiness check implements this inline:

```rust
// For each steam_root in steam_root_candidates:
//   steam_root/steamapps/compatdata/<appid>/pfx  — is_dir() check
let compatdata_root = steam_root.join("steamapps").join("compatdata");
let has_any_compatdata = fs::read_dir(&compatdata_root)
    .ok()
    .into_iter()
    .flatten()
    .filter_map(|entry| entry.ok())
    .any(|entry| entry.path().join("pfx").is_dir());
```

This is per A-9: path is derived from `discover_steam_libraries()` + scan, not from profile's `steam.compatdata_path`.

#### `attempt_auto_populate` (`steam/auto_populate.rs:12`) — Not Used by Readiness

```rust
pub fn attempt_auto_populate(request: &SteamAutoPopulateRequest) -> SteamAutoPopulateResult
```

This is per-game (requires a game path). It is called during **profile creation** (wizard step 3), not in `check_readiness`. It already has a Tauri command (`auto_populate_steam`) that the wizard can invoke directly.

### Discovery Composure for `check_system_readiness()`

```rust
pub fn check_system_readiness() -> ReadinessCheckResult {
    let mut diagnostics = Vec::new();
    let steam_roots = discover_steam_root_candidates("", &mut diagnostics);

    let steam_check = build_steam_check(&steam_roots);          // discover_steam_root_candidates
    let proton_check = build_proton_check(&steam_roots, &mut diagnostics); // discover_compat_tools
    let compatdata_check = build_compatdata_check(&steam_roots); // fs::read_dir scan
    let trainer_check = build_trainer_info_check();              // always Info

    let checks = vec![steam_check, proton_check, compatdata_check, trainer_check];
    let critical_failures = checks.iter().filter(|c| matches!(c.severity, HealthIssueSeverity::Error)).count();
    let warnings = checks.iter().filter(|c| matches!(c.severity, HealthIssueSeverity::Warning)).count();
    let all_passed = critical_failures == 0 && warnings == 0;

    ReadinessCheckResult { checks, all_passed, critical_failures, warnings }
}
```

### `HealthIssue` Type Reuse

`ReadinessCheckResult.checks: Vec<HealthIssue>` reuses the existing type from `profile/health.rs:31-37`:

```rust
pub struct HealthIssue {
    pub field: String,
    pub path: String,       // "" for system checks with no specific path
    pub message: String,
    pub remediation: String,
    pub severity: HealthIssueSeverity,  // Error | Warning | Info
}
```

`HealthIssueSeverity` is already `Serialize`/`Deserialize` and crosses the IPC boundary in the health dashboard — no new type needed.

---

## Configuration (Capabilities / Permissions)

### Current `capabilities/default.json`

```json
{
  "$schema": "../gen/schemas/desktop-schema.json",
  "identifier": "default",
  "description": "Default desktop capability for the main CrossHook window.",
  "windows": ["main"],
  "permissions": ["core:default", "dialog:default"]
}
```

### Required Changes for Onboarding

The three new commands are all read-only or settings-only operations:

- `check_readiness` — filesystem reads only (`is_dir()`, `read_dir()`)
- `dismiss_onboarding` — writes to `settings.toml` (TOML store, not shell)
- `get_trainer_guidance` — returns compiled static data, no I/O

**No new Tauri permissions are needed for v1.** These commands operate within the existing `core:default` scope. They do not invoke the shell, open URLs, or access FS outside the app's data directory.

### Future: `opener:open-url` for "Install Steam" Button (W-3)

When the wizard adds an "Install Steam" button (post-v1), add `opener:allow-open-url` with a URL pattern allowlist. Per W-3: URLs must be hardcoded frontend constants, not derived from scan results. The capability entry would look like:

```json
{
  "identifier": "opener:allow-open-url",
  "allow": [{ "url": "https://store.steampowered.com/*" }]
}
```

Do not add this in v1 — it is not needed and adds surface area.

---

## Relevant Files

| File                                               | Description                                                                           |
| -------------------------------------------------- | ------------------------------------------------------------------------------------- |
| `src-tauri/src/lib.rs`                             | Command registration in `invoke_handler!`, managed state setup, startup event pattern |
| `src-tauri/src/commands/mod.rs`                    | Module declarations — add `pub mod onboarding;` here                                  |
| `src-tauri/src/commands/settings.rs`               | Canonical pattern for `SettingsStore`-backed commands                                 |
| `src-tauri/src/commands/steam.rs`                  | Canonical pattern for sync and async discovery commands                               |
| `src-tauri/capabilities/default.json`              | Tauri capability/permission file — no changes needed for v1                           |
| `crates/crosshook-core/src/settings/mod.rs`        | `AppSettingsData`, `SettingsStore::load/save`, TOML round-trip                        |
| `crates/crosshook-core/src/steam/discovery.rs`     | `discover_steam_root_candidates()`                                                    |
| `crates/crosshook-core/src/steam/proton.rs`        | `discover_compat_tools()`                                                             |
| `crates/crosshook-core/src/steam/auto_populate.rs` | `attempt_auto_populate()` — used in wizard step 3, not readiness                      |
| `crates/crosshook-core/src/profile/health.rs`      | `HealthIssue`, `HealthIssueSeverity` — reused by `ReadinessCheckResult`               |
| `crates/crosshook-core/src/metadata/mod.rs`        | `MetadataStore::disabled()`, `is_available()`, `with_conn()` pattern                  |
| `crates/crosshook-core/src/metadata/migrations.rs` | Current schema v10 — no new migration needed for onboarding                           |
| `crates/crosshook-core/src/metadata/models.rs`     | `MetadataStoreError`, table row types, `MAX_DIAGNOSTIC_JSON_BYTES`                    |
| `crates/crosshook-core/src/lib.rs`                 | Add `pub mod onboarding;` here                                                        |

---

## Architectural Patterns

- **Sync vs async commands**: Use `fn` (sync) for `check_readiness` and `dismiss_onboarding` — all operations are fast filesystem checks. Reserve `async fn` + `spawn_blocking` for genuinely blocking operations (see `auto_populate_steam`).
- **No `MetadataStore` dependency in readiness**: `MetadataStore` may be disabled; `check_readiness()` uses only `SettingsStore` (indirectly via `dismiss_onboarding`) and raw filesystem — no SQLite.
- **Full-struct settings overwrite**: `SettingsStore::save()` always writes the entire struct. Always load-then-mutate; never construct a fresh default and save.
- **Static guidance content**: `get_trainer_guidance()` returns `&'static str` constants compiled into the binary — zero latency, no injection surface from taps.
- **`HealthIssue` reuse**: The readiness check reuses the existing health type, avoiding a parallel type and keeping IPC types minimal.
- **Startup event emission**: Use `tauri::async_runtime::spawn` + `sleep(Duration::from_millis(N))` inside the setup closure for deferred events — matches the `auto-load-profile` and `profile-health-batch-complete` patterns.

---

## Edge Cases

- `discover_steam_root_candidates("")` with no Steam installed returns `[]` — `steam_installed` check correctly fails with an `Error` severity `HealthIssue`.
- `discover_compat_tools(&[], &mut diagnostics)` with empty roots returns `[]` — `proton_available` check correctly fails.
- Flatpak Steam is handled transparently by `discover_steam_root_candidates` — no special case needed.
- Existing users upgrading: `onboarding_completed` deserializes as `false` from existing settings files (via `#[serde(default)]`) — wizard auto-shows but is dismissible in one click.
- `SettingsStore` missing file: `load()` returns `AppSettingsData::default()` (all fields zero-valued, `onboarding_completed = false`) — correct behavior for first run.
