# Technical Research: Protontricks Integration

## Executive Summary

The protontricks integration requires a new `prefix_deps` module in `crosshook-core` that manages WINE prefix dependency installation via protontricks or winetricks. The schema adds `required_protontricks: Vec<String>` to the `TrainerSection` of `GameProfile` (TOML-persisted), a new `prefix_dependency_state` table in SQLite (migration v14 → v15), and `protontricks_binary_path` to `AppSettingsData`. Four new Tauri IPC commands drive the UI. Process execution follows the existing `update::service` pattern: build a `tokio::process::Command`, spawn async, stream stdout/stderr to the frontend via `app.emit`, and use a `Mutex<Option<u32>>` for cancellation. Concurrent-install prevention is enforced by a Tauri-managed `PrefixDepsInstallState`.

---

## Architecture Design

### Component Diagram

```
UI (React)
  │  invoke("detect_protontricks_binary")
  │  invoke("check_prefix_dependencies", {profile_name, prefix_path, steam_app_id})
  │  invoke("install_prefix_dependency", {profile_name, prefix_path, package, steam_app_id})
  │  invoke("get_dependency_status", {profile_name})
  ▼
src-tauri/src/commands/prefix_deps.rs       ← thin Tauri layer, IPC glue
  │
  ▼
crosshook-core/src/prefix_deps/             ← all business logic
  ├── mod.rs                                ← public re-exports
  ├── models.rs                             ← request/response/error types
  ├── binary.rs                             ← detect_protontricks_binary()
  ├── checker.rs                            ← check whether packages are installed
  └── installer.rs                          ← build install Command, stream output
  │
  ▼
MetadataStore (SQLite)                      ← prefix_dependency_state table (v15)
AppSettingsData (TOML)                      ← protontricks_binary_path field
GameProfile::TrainerSection (TOML)          ← required_protontricks field
```

### New Components

| Component                       | Location                                | Responsibility          |
| ------------------------------- | --------------------------------------- | ----------------------- |
| `prefix_deps` module            | `crosshook-core/src/prefix_deps/`       | All business logic      |
| `prefix_deps.rs` command file   | `src-tauri/src/commands/prefix_deps.rs` | IPC wrappers, streaming |
| `PrefixDepsInstallState`        | `src-tauri/src/commands/prefix_deps.rs` | Concurrent-install lock |
| `prefix_dependency_state` table | SQLite migration v14→v15                | Persisted check results |

### Integration Points

- `crosshook-core/src/lib.rs` — add `pub mod prefix_deps;`
- `src-tauri/src/commands/mod.rs` — add `pub mod prefix_deps;`
- `src-tauri/src/lib.rs` — register four new invoke_handler entries, manage `PrefixDepsInstallState`
- `crosshook-core/src/metadata/migrations.rs` — add `migrate_14_to_15` function and call it
- `crosshook-core/src/metadata/mod.rs` — add `PrefixDependencyStateRow`, `upsert_prefix_dependency_state`, `get_prefix_dependency_states_for_profile`
- `crosshook-core/src/profile/models.rs` — add `required_protontricks: Vec<String>` to `TrainerSection`
- `crosshook-core/src/settings/mod.rs` — add `protontricks_binary_path: String` to `AppSettingsData`

---

## Data Models

### SQLite: `prefix_dependency_state` table (migration v14 → v15)

```sql
CREATE TABLE IF NOT EXISTS prefix_dependency_state (
    id               INTEGER PRIMARY KEY AUTOINCREMENT,
    profile_id       TEXT NOT NULL REFERENCES profiles(profile_id) ON DELETE CASCADE,
    package_name     TEXT NOT NULL,         -- e.g. "vcrun2019", "dotnet48"
    prefix_path      TEXT NOT NULL,         -- canonical compat_data_path
    state            TEXT NOT NULL DEFAULT 'unknown',
        -- 'unknown' | 'installed' | 'missing' | 'install_failed' | 'check_failed'
    checked_at       TEXT,                  -- ISO-8601 UTC, NULL if never checked
    installed_at     TEXT,                  -- ISO-8601 UTC, NULL if not installed
    last_error       TEXT,                  -- last error message, NULL if none
    created_at       TEXT NOT NULL,
    updated_at       TEXT NOT NULL
);

CREATE UNIQUE INDEX IF NOT EXISTS idx_prefix_dep_state_profile_package_prefix
    ON prefix_dependency_state(profile_id, package_name, prefix_path);

CREATE INDEX IF NOT EXISTS idx_prefix_dep_state_profile_id
    ON prefix_dependency_state(profile_id);
```

**Notes:**

- `prefix_path` stores the canonical `STEAM_COMPAT_DATA_PATH` (parent of `pfx/`), matching how `apply_runtime_proton_environment` resolves it. This ensures the key is consistent regardless of which path variant the user configured.
- `state` is a string enum (not INTEGER) to match the existing pattern in `health_snapshots.status`, `offline_readiness_snapshots.readiness_state`, etc.
- No foreign key to `prefix_path` — prefix paths are not separately tracked in SQLite.
- `installed_at` is nullable: set on successful install or when a check confirms presence.

### TOML: `GameProfile::TrainerSection` (profile files)

Add to `crosshook-core/src/profile/models.rs`:

```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TrainerSection {
    #[serde(default)]
    pub path: String,
    #[serde(rename = "type", default)]
    pub kind: String,
    #[serde(rename = "loading_mode", default)]
    pub loading_mode: TrainerLoadingMode,
    #[serde(
        default = "default_trainer_type",
        skip_serializing_if = "is_default_trainer_type"
    )]
    pub trainer_type: String,
    /// Protontricks/winetricks packages required in the WINE prefix.
    /// Empty list = no dependencies declared.
    #[serde(
        rename = "required_protontricks",
        default,
        skip_serializing_if = "Vec::is_empty"
    )]
    pub required_protontricks: Vec<String>,
}
```

**Persistence classification:** User-editable preference in TOML profile. Part of the profile's shareable state — included in community profile exports automatically (it is a field on `GameProfile::trainer`). No migration needed; `#[serde(default)]` handles existing profiles without the field.

### TOML: `AppSettingsData` (settings.toml)

Add to `crosshook-core/src/settings/mod.rs`:

```rust
/// Path to the protontricks or winetricks binary.
/// Empty = auto-detect on PATH. Restart not required.
#[serde(default, skip_serializing_if = "String::is_empty")]
pub protontricks_binary_path: String,
```

**Persistence classification:** User-editable preference in TOML settings. Auto-detection fills this from `$PATH` when empty. The saved value is a user override.

### Rust Structs

```rust
// crosshook-core/src/prefix_deps/models.rs

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct CheckPrefixDepsRequest {
    pub profile_name: String,
    pub prefix_path: String,        // configured prefix_path from profile runtime section
    pub steam_app_id: String,       // used by protontricks to target the right prefix
    pub packages: Vec<String>,      // from profile.trainer.required_protontricks
    pub binary_path: String,        // resolved protontricks path (empty = auto-detect)
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PackageDependencyState {
    pub package: String,
    pub state: DependencyState,     // see below
    pub checked_at: Option<String>,
    pub installed_at: Option<String>,
    pub last_error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum DependencyState {
    #[default]
    Unknown,
    Installed,
    Missing,
    InstallFailed,
    CheckFailed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct CheckPrefixDepsResult {
    pub profile_name: String,
    pub states: Vec<PackageDependencyState>,
    pub all_installed: bool,
    pub missing_packages: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct InstallDepRequest {
    pub profile_name: String,
    pub prefix_path: String,
    pub steam_app_id: String,
    pub package: String,
    pub binary_path: String,        // empty = auto-detect
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct InstallDepResult {
    pub succeeded: bool,
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PrefixDepsError {
    BinaryNotFound { searched_path: String },
    InvalidPackageName { package: String },
    PrefixPathMissing,
    SteamAppIdRequired,
    ProfileNameRequired,
    PackageRequired,
    SpawnFailed { message: String },
    InstallFailed { exit_code: Option<i32> },
    AlreadyInstalling,
    MetadataUnavailable,
}

// Row type for MetadataStore reads
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrefixDependencyStateRow {
    pub package_name: String,
    pub prefix_path: String,
    pub state: String,
    pub checked_at: Option<String>,
    pub installed_at: Option<String>,
    pub last_error: Option<String>,
}
```

### MetadataStore Methods

Add to `crosshook-core/src/metadata/mod.rs`:

```rust
pub fn upsert_prefix_dependency_state(
    &self,
    profile_id: &str,
    package_name: &str,
    prefix_path: &str,
    state: &str,
    installed_at: Option<&str>,
    last_error: Option<&str>,
) -> Result<(), MetadataStoreError>

pub fn get_prefix_dependency_states(
    &self,
    profile_id: &str,
) -> Result<Vec<PrefixDependencyStateRow>, MetadataStoreError>
```

Implement in a new `prefix_deps_store.rs` sub-module under `crosshook-core/src/metadata/`, following the pattern of `health_store.rs` and `offline_store.rs`.

---

## API Design

### Tauri IPC Commands (all in `src-tauri/src/commands/prefix_deps.rs`)

#### `detect_protontricks_binary`

```
// Request: none
// Response: DetectBinaryResult
// Errors: String (rare — only on I/O errors searching PATH)

#[tauri::command]
pub fn detect_protontricks_binary(
    settings: State<'_, SettingsStore>,
) -> DetectBinaryResult
```

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetectBinaryResult {
    pub found: bool,
    pub binary_path: String,      // resolved path, or empty if not found
    pub binary_name: String,      // "protontricks" | "winetricks" | ""
    pub source: String,           // "settings" | "path" | "flatpak" | "not_found"
}
```

**Detection algorithm** (in `crosshook-core/src/prefix_deps/binary.rs`):

1. If `settings.protontricks_binary_path` is non-empty and the file is executable → return it, source = "settings"
2. Search `$PATH` for `protontricks` → return first hit, source = "path"
3. Check `/usr/bin/flatpak` exists AND `flatpak run com.github.Matoking.protontricks --version` exits 0 → return `flatpak run com.github.Matoking.protontricks`, source = "flatpak"
4. Search `$PATH` for `winetricks` → return first hit, source = "path", binary_name = "winetricks"
5. Return not found

**Note:** Flatpak protontricks requires `--filesystem=host` and a Steam app ID; the caller must use the correct invocation pattern.

---

#### `check_prefix_dependencies`

```
// Request: CheckPrefixDepsRequest
// Response: CheckPrefixDepsResult
// Errors: String (serialized PrefixDepsError)

#[tauri::command]
pub async fn check_prefix_dependencies(
    app: AppHandle,
    request: CheckPrefixDepsRequest,
    metadata_store: State<'_, MetadataStore>,
) -> Result<CheckPrefixDepsResult, String>
```

**Behavior:**

1. Validate: `profile_name` non-empty, `prefix_path` dir exists, `packages` non-empty, package names valid (allowlist check — see Security section).
2. Resolve binary (same logic as detect, using `request.binary_path`).
3. For each package: run `protontricks --no-bwrap <steam_app_id> list-installed 2>&1` and grep for the package name.
   - Alternative check (more reliable): run `protontricks <steam_app_id> list 2>&1` or check the WINE registry prefix directly for known packages (see Codebase Analysis section for why registry check is preferred).
4. Upsert each package state into SQLite via `metadata_store.upsert_prefix_dependency_state`.
5. Return aggregated `CheckPrefixDepsResult`.

**Process management:** `spawn_blocking` wraps the synchronous check logic; each check spawns a child process with a 30-second timeout enforced via `tokio::time::timeout`.

---

#### `install_prefix_dependency`

```
// Request: InstallDepRequest
// Response: InstallDepResult
// Errors: String (serialized PrefixDepsError)

#[tauri::command]
pub async fn install_prefix_dependency(
    app: AppHandle,
    request: InstallDepRequest,
    state: State<'_, PrefixDepsInstallState>,
    metadata_store: State<'_, MetadataStore>,
) -> Result<InstallDepResult, String>
```

**Behavior:**

1. Validate request (same rules as check; single package only).
2. Check `PrefixDepsInstallState` mutex — if `is_installing` is true, return `PrefixDepsError::AlreadyInstalling`.
3. Set `is_installing = true`, store profile_name and package for the ongoing install.
4. Build `Command`: `protontricks [--no-bwrap] <steam_app_id> <package>`.
5. Spawn async child process; stream stdout/stderr lines to frontend via `app.emit("prefix-dep-install-log", line)`.
6. On exit: emit `"prefix-dep-install-complete"` event with `{ package, succeeded, exit_code }`.
7. Upsert state in SQLite: `installed` on success, `install_failed` on failure.
8. Clear `is_installing` in all exit paths (success, error, panic via Drop).

**Events emitted:**

- `"prefix-dep-install-log"` — `{ package: String, line: String }`
- `"prefix-dep-install-complete"` — `{ package: String, succeeded: bool, exit_code: Option<i32> }`

---

#### `get_dependency_status`

```
// Request: profile_name: String
// Response: Vec<PackageDependencyState>
// Errors: String

#[tauri::command]
pub fn get_dependency_status(
    profile_name: String,
    metadata_store: State<'_, MetadataStore>,
) -> Result<Vec<PackageDependencyState>, String>
```

**Behavior:** Looks up `profile_id` from `profile_name`, then calls `metadata_store.get_prefix_dependency_states(profile_id)`. Pure SQLite read — no process spawning. Returns empty vec if profile has no recorded state (not an error).

---

### Frontend Invoke Examples

```typescript
// Detect binary
const result = await invoke<DetectBinaryResult>('detect_protontricks_binary');

// Check all deps for a profile
const check = await invoke<CheckPrefixDepsResult>('check_prefix_dependencies', {
  request: {
    profile_name: 'Elden Ring',
    prefix_path: '/home/user/.local/share/Steam/steamapps/compatdata/1245620',
    steam_app_id: '1245620',
    packages: ['vcrun2019', 'dotnet48'],
    binary_path: '', // empty = auto-detect
  },
});

// Install a single package (then listen for events)
const result = await invoke<InstallDepResult>('install_prefix_dependency', {
  request: {
    profile_name: 'Elden Ring',
    prefix_path: '...',
    steam_app_id: '1245620',
    package: 'vcrun2019',
    binary_path: '',
  },
});

// Get cached status from SQLite
const states = await invoke<PackageDependencyState[]>('get_dependency_status', {
  profileName: 'Elden Ring',
});
```

---

## System Constraints

### Process Management

**Command construction** follows `crosshook-core/src/update/service.rs` pattern:

```rust
// crosshook-core/src/prefix_deps/installer.rs

fn build_install_command(
    request: &InstallDepRequest,
    binary_path: &str,
    log_path: &Path,
) -> Result<Command, PrefixDepsError> {
    let mut cmd = Command::new(binary_path);

    // protontricks requires STEAM_COMPAT_DATA_PATH for the target prefix
    cmd.env_clear();
    apply_host_environment(&mut cmd);
    cmd.env("STEAM_COMPAT_DATA_PATH", &request.prefix_path);
    cmd.env("WINEPREFIX", resolve_wine_prefix_path(Path::new(&request.prefix_path)));

    // --no-bwrap: skip bwrap container; required when running inside an
    // existing container (AppImage sandbox), and generally more reliable.
    // Omit for winetricks (it has no --no-bwrap flag).
    if binary_path.contains("protontricks") {
        cmd.arg("--no-bwrap");
    }

    cmd.arg(&request.steam_app_id);
    cmd.arg(&request.package);

    attach_log_stdio(&mut cmd, log_path)?;
    Ok(cmd)
}
```

**Note on Flatpak protontricks:** When `source = "flatpak"`, the binary is `flatpak`, and the first args are `run --filesystem=host com.github.Matoking.protontricks`. The installer must detect this case and prepend those args correctly. The `binary.rs` resolver can return a `BinaryInvocation` struct rather than a bare path string:

```rust
pub struct BinaryInvocation {
    pub program: String,
    pub leading_args: Vec<String>,   // e.g. ["run", "--filesystem=host", "com.github.Matoking.protontricks"]
    pub binary_name: String,         // "protontricks" | "winetricks"
    pub source: String,
}
```

### Stdout/Stderr Streaming

The log-file-based streaming approach used in `commands/update.rs` is the established pattern. The `attach_log_stdio` helper in `runtime_helpers.rs` redirects both stdout and stderr to an append-mode log file. The async streamer in the command file polls the file and emits lines.

For protontricks installs, use the same approach: log path created via `create_log_path("prefix-dep", &slug)` using the existing `commands::shared::create_log_path` helper.

### Cancellation

```rust
// In src-tauri/src/commands/prefix_deps.rs

pub struct PrefixDepsInstallState {
    is_installing: Mutex<bool>,
    current_pid: Mutex<Option<u32>>,
}

impl PrefixDepsInstallState {
    pub fn new() -> Self {
        Self {
            is_installing: Mutex::new(false),
            current_pid: Mutex::new(None),
        }
    }
}
```

No separate `cancel_prefix_dep_install` command is strictly required for v1 (installs are typically short, < 2 minutes), but the architecture supports adding it: the `current_pid` field allows SIGTERM via `kill(pid)`, following the `cancel_update` pattern.

### Concurrent Install Prevention

Only one package may be installed at a time across all profiles. The `PrefixDepsInstallState` mutex enforces this. If `is_installing` is true, `install_prefix_dependency` returns immediately with `PrefixDepsError::AlreadyInstalling`. The UI should disable the install button and show in-progress state by listening to install events.

### Package Name Allowlist

The package name parameter must be validated before being passed to the shell. Use an explicit allowlist of known safe characters: `[a-z0-9_-]` only. Reject any package name containing characters outside this set. This prevents injection even though we use `Command::arg` (which does not invoke a shell), but it also prevents nonsensical packages from being attempted.

```rust
// crosshook-core/src/prefix_deps/models.rs
pub fn is_valid_package_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 64
        && name.chars().all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
}
```

---

## Codebase Changes

### Files to Create

| File                                               | Description                                                      |
| -------------------------------------------------- | ---------------------------------------------------------------- |
| `crosshook-core/src/prefix_deps/mod.rs`            | Module definition and public re-exports                          |
| `crosshook-core/src/prefix_deps/models.rs`         | Request/response/error structs, `is_valid_package_name`          |
| `crosshook-core/src/prefix_deps/binary.rs`         | `detect_binary()` returning `BinaryInvocation`                   |
| `crosshook-core/src/prefix_deps/checker.rs`        | `check_installed()` per-package check logic                      |
| `crosshook-core/src/prefix_deps/installer.rs`      | `build_install_command()`                                        |
| `crosshook-core/src/metadata/prefix_deps_store.rs` | `upsert_prefix_dependency_state`, `get_prefix_dependency_states` |
| `src-tauri/src/commands/prefix_deps.rs`            | Four IPC commands + `PrefixDepsInstallState`                     |

### Files to Modify

| File                                        | Change                                                                           |
| ------------------------------------------- | -------------------------------------------------------------------------------- |
| `crosshook-core/src/lib.rs`                 | Add `pub mod prefix_deps;`                                                       |
| `crosshook-core/src/profile/models.rs`      | Add `required_protontricks` field to `TrainerSection`                            |
| `crosshook-core/src/settings/mod.rs`        | Add `protontricks_binary_path` field to `AppSettingsData` and its `Default` impl |
| `crosshook-core/src/metadata/mod.rs`        | Add `mod prefix_deps_store;`, `PrefixDependencyStateRow`, two new public methods |
| `crosshook-core/src/metadata/migrations.rs` | Add `migrate_14_to_15()`, call it in `run_migrations()`                          |
| `src-tauri/src/commands/mod.rs`             | Add `pub mod prefix_deps;`                                                       |
| `src-tauri/src/lib.rs`                      | Register four commands in `invoke_handler!`, manage `PrefixDepsInstallState`     |

### Dependencies

No new crate dependencies required. All primitives needed (`tokio::process::Command`, `rusqlite`, `serde`, `tracing`) are already in `crosshook-core/Cargo.toml`. The Flatpak detection requires checking if `/usr/bin/flatpak` is executable — standard `std::fs::metadata`.

---

## Technical Decisions

### Decision 1: How to Check if a Package is Installed

**Option A: `protontricks <app_id> list-installed`**

- Pro: Explicit, uses protontricks' own knowledge.
- Con: Launches a full protontricks session; slow (3–10s per package on first run). `list-installed` may not exist in all versions. Output format varies.

**Option B: Check WINE registry directly**

- Read `$WINEPREFIX/system.reg` or `user.reg` for known registry keys. vcrun installs write to `HKLM\SOFTWARE\Microsoft\VisualC\14`. dotnet installs write to `HKLM\SOFTWARE\Microsoft\.NETFramework`.
- Pro: No process spawn, instant, reliable.
- Con: Registry keys are not standardized across package versions; requires per-package key knowledge. Winetricks schema may change.

**Option C: Run `protontricks <app_id> <package>` and check exit code**

- Pro: Idempotent — if already installed, protontricks exits 0 quickly.
- Con: Longer, modifies prefix state during a "check", not a pure read.

**Recommendation: Option A with `list` subcommand**, parsing output for package mentions, with a fallback to Option B for the most common packages (vcrun, dotnet, d3dx9). This gives reasonable accuracy without requiring per-package registry key knowledge at MVP.

The actual check: `protontricks [--no-bwrap] <steam_app_id> list 2>&1` outputs all installed packages. Parse the output for the target package name.

### Decision 2: `--no-bwrap` Default

**Recommendation:** Always pass `--no-bwrap` for protontricks invocations. The CrossHook AppImage runs inside a sandbox/Fuse environment, and bwrap (bubblewrap) inside bwrap fails. The `--no-bwrap` flag is safe on host systems — it simply runs without the extra namespace isolation that protontricks uses by default. This matches the established pattern of `env_clear()` + explicit env setup in `runtime_helpers`.

### Decision 3: Flatpak Protontricks Invocation

**Recommendation:** Handle Flatpak as a `BinaryInvocation` struct with leading args `["run", "--filesystem=host", "com.github.Matoking.protontricks"]`. The `--no-bwrap` arg is NOT passed for Flatpak (Flatpak handles its own sandboxing). The installer and checker must use the full `program + leading_args + pkg_args` invocation.

### Decision 4: SQLite Check Result Caching TTL

**Recommendation:** No TTL enforced in the DB schema. The UI controls when to re-check (explicit "Refresh" or on profile load). The `checked_at` timestamp is stored so the UI can display "checked X minutes ago" and optionally auto-trigger a recheck if stale (> 24 hours). This avoids complexity in the backend.

### Decision 5: Community Profile `required_protontricks` Validation on Import

**Recommendation:** On community profile import (`import_community_profile`), validate each entry in `required_protontricks` with `is_valid_package_name`. Reject imports with invalid package names. This is the same approach used for other profile field validation (e.g., `validate_steam_app_id`).

---

## Open Questions

1. **Protontricks `list` command output format** — needs empirical testing against protontricks v1.x and winetricks to confirm parsing approach. The api-researcher teammate should verify the exact CLI interface.

2. **`--no-bwrap` availability** — older protontricks versions may not support `--no-bwrap`. Should we check the version first, or catch the error and retry without?

3. **Steam App ID for non-Steam prefixes** — if the prefix was created by CrossHook's install flow (not Steam), there is no Steam App ID. The feature spec says `steam_app_id` is required by protontricks, but some CrossHook profiles use CrossHook-managed prefixes. Decision needed: either (a) require Steam-mode profiles for dependency installation, or (b) use a synthetic App ID (protontricks supports app ID 0 for non-Steam prefixes in some versions).

4. **`winetricks` vs `protontricks` feature parity** — winetricks does not accept a Steam App ID argument; it operates on `$WINEPREFIX` directly. If the binary is winetricks, the invocation is `winetricks <package>` with `WINEPREFIX` set. The checker and installer must branch on `binary_name`.

5. **WINE prefix initialization requirement** — protontricks fails on an uninitialized prefix. Should `check_prefix_dependencies` detect an uninitialized prefix (no `pfx/system.reg`) and return a `PrefixNotInitialized` state rather than running protontricks?

---

## Relevant Files

- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/profile/models.rs` — `TrainerSection` struct to extend
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/settings/mod.rs` — `AppSettingsData` to extend
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/metadata/migrations.rs` — migration v14→v15 location
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/metadata/mod.rs` — MetadataStore methods to add
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/launch/runtime_helpers.rs` — `apply_host_environment`, `resolve_wine_prefix_path`, `attach_log_stdio`, `is_executable_file`
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/update/service.rs` — reference pattern for process building and spawning
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src-tauri/src/commands/update.rs` — reference pattern for streaming + cancellation state
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src-tauri/src/commands/onboarding.rs` — reference for binary detection pattern
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src-tauri/src/lib.rs` — invoke_handler registration site
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/lib.rs` — module registration site
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/profile/community_schema.rs` — `CommunityProfileManifest` (required_protontricks propagates via GameProfile)
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/metadata/health_store.rs` — reference for metadata sub-module pattern

## Persistence & Usability

| Datum                                  | Classification                | Notes                                               |
| -------------------------------------- | ----------------------------- | --------------------------------------------------- |
| `required_protontricks`                | TOML profile (user-editable)  | Per-profile, exported in community profiles         |
| `protontricks_binary_path`             | TOML settings (user-editable) | App-wide, not per-profile                           |
| `prefix_dependency_state` rows         | SQLite metadata               | Per-profile+package+prefix; survives app restart    |
| Active installation progress           | Runtime only                  | In-memory `PrefixDepsInstallState`; lost on restart |
| Dependency check results (live output) | Runtime only                  | Streamed via Tauri events; not persisted separately |

**Migration:** No migration needed for TOML changes (`#[serde(default)]` handles existing files). SQLite migration v14→v15 is purely additive (new table + indexes). Rollback is safe: the old schema version will not have the table, but the app will fail-fast on migration if something goes wrong (existing pattern in `run_migrations`).

**Offline behavior:** If the SQLite metadata store is unavailable (`MetadataStore::disabled()`), `check_prefix_dependencies` and `install_prefix_dependency` can still execute (they spawn processes; SQLite is for result caching). `get_dependency_status` returns an empty vec when the store is unavailable.

**Degraded fallback:** If protontricks/winetricks binary is not found, all four commands surface a clear `BinaryNotFound` error. The UI should guide the user to install protontricks and optionally configure the binary path in settings.

**User visibility/editability:** `required_protontricks` is visible and editable in the profile TOML. `protontricks_binary_path` is visible and editable in app settings UI. Dependency state is visible in the UI dependency panel; users cannot directly edit the SQLite rows (it is operational metadata).

---

## Architectural Alternatives Evaluated

The following alternatives were raised during team review and evaluated against actual codebase evidence.

### Alt 1: Lazy (launch-time) vs. on-demand dependency checking

**Proposal:** Check dependencies only when launching, blocking the launch flow if deps are missing.

**Evaluation:** Rejected for the primary check path. Protontricks checks are slow (3–30s per package); blocking a game or trainer launch is unacceptable UX. The design already uses on-demand checking (explicit UI action), with cached SQLite results for fast subsequent reads. A lightweight prefix-initialization guard (check for `pfx/system.reg` existence) can run at launch time without spawning protontricks, covering the most common failure mode at zero cost. This guard should return a `ValidationSeverity::Warning` via the existing `LaunchValidationIssue` path — not a hard block.

### Alt 2: Binary path as `Option<String>` (matching `steamgriddb_api_key`)

**Proposal:** Model `protontricks_binary_path` as `Option<String>` to match the `steamgriddb_api_key` pattern in `AppSettingsData`.

**Evaluation:** Rejected. `steamgriddb_api_key` is `Option<String>` because `None` means "feature disabled" — no key means no SteamGridDB lookups. `protontricks_binary_path` is a path override, not a feature toggle. Empty string already means "auto-detect" across all other path fields in this codebase (`proton_path`, `working_directory`, `steam_compat_data_path`). Using `String` with `#[serde(skip_serializing_if = "String::is_empty")]` is consistent with that convention. `Option<String>` would require `.as_deref().unwrap_or("")` or `.unwrap_or_default()` call-sites everywhere `trim().is_empty()` currently suffices.

### Alt 3: Plug dependency warnings into `batch_check_health_with_enrich`

**Proposal:** Add protontricks dependency checking to the `batch_check_health_with_enrich` closure in `profile/health.rs`.

**Evaluation:** Full protontricks process-based checking: **rejected**. The `batch_check_health_with_enrich` signature is `FnMut(&str, &GameProfile, &mut ProfileHealthReport)` — fully synchronous, called once per profile in a tight loop. Spawning a protontricks child process (3–30s per package) inside this closure would block the entire health scan for minutes.

Lightweight derivative — **accepted as an additive**: if a profile has non-empty `required_protontricks`, `check_profile_health` can read the cached `DependencyState` values from `MetadataStore` (synchronous SQLite reads) and synthesize a `HealthIssue` with `HealthIssueSeverity::Warning` for any packages in `missing` or `install_failed` state. This adds zero latency to the health scan and keeps dependency status visible in the existing health UI without a separate code path. This is an additive follow-on change that does not replace the dedicated dependency commands.

### Alt 4: Async process execution pattern

**Proposal:** Confirm all process spawning uses `env_clear()` + explicit env vars.

**Evaluation:** Confirmed and already specified. The `build_install_command` design calls `env_clear()` then `apply_host_environment()` then sets `STEAM_COMPAT_DATA_PATH` and `WINEPREFIX` explicitly — identical to how `build_update_command` and `new_direct_proton_command_with_wrappers` operate in the existing codebase.

### Alt 5: `required_protontricks` in `CommunityProfileMetadata` vs. `TrainerSection`

**Proposal:** Place `required_protontricks` in `CommunityProfileMetadata` (inside `CommunityProfileManifest`) rather than `GameProfile::TrainerSection`, treating it as a community-only hint.

**Evaluation:** Rejected. `CommunityProfileMetadata` is stripped during import — only `profile: GameProfile` is preserved in the user's local TOML. If `required_protontricks` were only in `CommunityProfileMetadata`, it would be lost the moment the user saves their local profile, making the feature non-functional after the first edit. Placing it in `TrainerSection` (part of `GameProfile`) ensures it survives import, export, local saves, and round-trips. Community profiles automatically include it because `CommunityProfileManifest.profile` embeds the full `GameProfile`.

### Alt 6: No dependency state in TOML profiles

**Proposal:** Dependency check state (installed/missing/failed timestamps) must not be written to TOML profile files.

**Evaluation:** Confirmed and already specified. The `prefix_dependency_state` SQLite table holds all runtime state. TOML profile files hold only `required_protontricks: Vec<String>` (the declared dependency list — portable, machine-independent). No check results or timestamps are ever written to TOML.

---

## Business Rules Integration

The following evaluates the business-analyzer's proposals against the actual codebase, with accept/reject/modify decisions for each.

### BR-1: New `DependenciesSection` struct with `required_protontricks` + `user_extra_protontricks`

**Proposal:** Add a new top-level `[dependencies]` TOML section to `GameProfile` with two fields: `required_protontricks` (community-authored, exported) and `user_extra_protontricks` (user-only, NOT exported to community).

**Decision: Partially accepted with modification.**

The `user_extra_protontricks` distinction is a valid business rule — user additions should not pollute community exports. However, a new top-level `DependenciesSection` is rejected (see Alt 5 analysis). The correct implementation:

- `required_protontricks: Vec<String>` goes in `TrainerSection` (survives import/export/save round-trips).
- `user_extra_protontricks: Vec<String>` goes in `LocalOverrideSection` as `LocalOverrideTrainerSection.extra_protontricks`, following the exact pattern used for game paths, steam paths, and runtime paths. `LocalOverrideSection` is already stripped during community export by `sanitize_profile_for_community_export` → `portable_profile()` → `local_override = LocalOverrideSection::default()`. This gives the desired separation with zero new abstraction.

```rust
// In LocalOverrideTrainerSection (models.rs):
pub struct LocalOverrideTrainerSection {
    #[serde(default)]
    pub path: String,
    /// User-only extra packages; stripped on community export.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub extra_protontricks: Vec<String>,
}
```

The effective package list at install time = `profile.trainer.required_protontricks` + `profile.local_override.trainer.extra_protontricks` (deduplicated).

### BR-2: `COMMUNITY_PROFILE_SCHEMA_VERSION` bump to 2

**Decision: Rejected.**

A schema version bump is only warranted when the import validator (`validate_manifest_value`) must reject old manifests that are now structurally incompatible. Adding an optional field with `#[serde(default)]` does not require this — serde deserializes old manifests cleanly. `validate_schema_version` in `exchange.rs` uses `version > COMMUNITY_PROFILE_SCHEMA_VERSION` to reject future versions, so bumping to v2 would break all existing v1 community profile imports until every tap updates. Keep at v1; `required_protontricks` absence in old manifests deserializes as an empty vec.

### BR-3: `AppSettingsData` gains `protontricks_binary_path` + `auto_install_prefix_deps`

**Decision: Partially accepted.**

- `protontricks_binary_path: String` — **accepted**, already in the spec.
- `auto_install_prefix_deps: bool` (default false) — **accepted** as a settings field. However, it should NOT trigger silent background installs; it should only auto-present the install dialog on profile load, with user confirmation still required. Silent auto-install of arbitrary community-declared packages without per-session user confirmation is a security risk and a poor UX pattern.

```rust
// In AppSettingsData:
/// When true, prompt to install missing prefix dependencies on profile load.
/// Installs still require explicit user confirmation per session.
#[serde(default)]
pub auto_check_prefix_deps: bool,
```

Field renamed to `auto_check_prefix_deps` to accurately reflect behavior (check and prompt, not silently install).

### BR-4: SQLite table name `prefix_dependency_states` (plural) and keyed `(profile_id, package_name)` only (no `prefix_path`)

**Decision: Partially accepted.**

- **Table name:** The spec uses `prefix_dependency_state` (singular), consistent with `health_snapshots`, `offline_readiness_snapshots`, `version_snapshots` (all singular). **Keep singular: `prefix_dependency_state`.**
- **Key without `prefix_path`:** The proposal keys on `(profile_id, package_name)` only, asserting "different profiles sharing a prefix may need different packages." This is correct for the business case. However, a profile's `prefix_path` can change (user reconfigures the runtime path). Without `prefix_path` in the key, a stale check result from an old prefix would be shown as valid for a new prefix. **Decision: keep `(profile_id, package_name, prefix_path)` unique key** to correctly invalidate state when the prefix path changes. This is a correctness requirement, not just an implementation detail.
- **`status` enum:** The proposed values `unchecked | checking | installed | missing | unknown | install_failed | user_skipped` are accepted with modifications:
  - Drop `checking` — active install state is runtime-only (not persisted to SQLite).
  - Keep: `unknown` (never checked), `installed`, `missing`, `install_failed`, `user_skipped`.
  - `unchecked` is redundant with `unknown`; use `unknown` only.

Final states: `unknown | installed | missing | install_failed | user_skipped | check_failed`.

- **`source` column (`declared | user_added`):** **Accepted.** Useful for UI display and for correctly assembling the effective package list. Add `source TEXT NOT NULL DEFAULT 'declared'` to the table.

### BR-5: Core validation rules

**Decision: Partially accepted.**

1. **Static allowlist of winetricks verbs** — the character-based allowlist (`[a-z0-9_-]`) is already specified. A static hardcoded verb list is rejected: it would require maintenance with every winetricks release and would block legitimate packages not yet in the list. The character allowlist + max-length validation is sufficient for security (protontricks/winetricks validate their own verb names and fail cleanly on unknown packages).

2. **Install requires binary + prefix_path + DISPLAY** — **accepted**. Add `DISPLAY` env var check to `check_prefix_dependencies` and `install_prefix_dependency` validation. If `DISPLAY` and `WAYLAND_DISPLAY` are both unset, return a `PrefixDepsError::DisplayRequired` error.

3. **Only one active install per prefix path** — **accepted as clarification**: the existing global lock (`PrefixDepsInstallState`) prevents all concurrent installs. Per-prefix locking is more complex and not needed at v1 — the global lock is the correct default.

4. **Batch install in one invocation** — **partially accepted**. For the install command, the `install_prefix_dependency` IPC command installs one package at a time (simpler streaming, clearer error attribution). If the UI wants to install multiple packages, it calls the command sequentially. However, for efficiency the core `installer.rs` should support `install_deps_batch(packages: &[String], ...)` for a single protontricks invocation with multiple package names. This allows future optimization without changing the IPC contract. If the batch fails, all packages in the batch are marked `install_failed`.

5. **`protontricks <appid>` vs `WINEPREFIX=<path> winetricks`** — **accepted**. This exactly matches the `BinaryInvocation` branching already specified. When `binary_name == "winetricks"`, pass `WINEPREFIX` env instead of steam_app_id arg.

### BR-6: `CommunityImportPreview` gains `required_prefix_deps: Vec<String>`

**Decision: Accepted.** The `CommunityImportPreview` struct in `exchange.rs` should surface the declared dependency list so the import UI can inform the user before they accept the import. This is a read-through of `manifest.profile.trainer.required_protontricks` — no new data, just promoted to the preview struct for convenient IPC access.

```rust
// In exchange.rs:
pub struct CommunityImportPreview {
    pub profile_name: String,
    pub source_path: PathBuf,
    pub profile: GameProfile,
    pub manifest: CommunityProfileManifest,
    /// Declared prefix dependencies from the imported profile.
    /// Convenience field — equal to profile.trainer.required_protontricks.
    pub required_prefix_deps: Vec<String>,
}
```

---

## Revised Data Model Summary (incorporating business rules)

### `TrainerSection` changes

```rust
pub struct TrainerSection {
    // ... existing fields ...
    #[serde(rename = "required_protontricks", default, skip_serializing_if = "Vec::is_empty")]
    pub required_protontricks: Vec<String>,  // community-declared; exported
}
```

### `LocalOverrideTrainerSection` changes

```rust
pub struct LocalOverrideTrainerSection {
    #[serde(default)]
    pub path: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub extra_protontricks: Vec<String>,  // user-added; stripped on export
}
```

### `AppSettingsData` changes

```rust
pub protontricks_binary_path: String,    // empty = auto-detect
pub auto_check_prefix_deps: bool,        // default false; prompts (does not silently install)
```

### SQLite `prefix_dependency_state` table (final schema, migration v14→v15)

```sql
CREATE TABLE IF NOT EXISTS prefix_dependency_state (
    id               INTEGER PRIMARY KEY AUTOINCREMENT,
    profile_id       TEXT NOT NULL REFERENCES profiles(profile_id) ON DELETE CASCADE,
    package_name     TEXT NOT NULL,
    prefix_path      TEXT NOT NULL,         -- canonical compat_data_path; key for invalidation
    source           TEXT NOT NULL DEFAULT 'declared',   -- 'declared' | 'user_added'
    state            TEXT NOT NULL DEFAULT 'unknown',
        -- 'unknown' | 'installed' | 'missing' | 'install_failed' | 'user_skipped' | 'check_failed'
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

### `PrefixDepsError` additions

```rust
pub enum PrefixDepsError {
    // ... existing variants ...
    DisplayRequired,   // DISPLAY and WAYLAND_DISPLAY both unset
}
```

### `CommunityImportPreview` addition

Add `required_prefix_deps: Vec<String>` field (populated as `profile.trainer.required_protontricks`).

---

## CLI API Findings Integration

External API research confirmed several assumptions and corrected others. Each finding is evaluated and integrated below.

### Finding 1: Correct command format uses `-q` flag

**Confirmed:** The install invocation is `protontricks [--no-bwrap] <APPID> -q <VERB> [<VERB>...]`. The `-q` (quiet/noninteractive) flag is required — without it, winetricks prompts interactively and hangs. The spec's `build_install_command` must include `-q` before the package name(s).

```rust
// In installer.rs — corrected:
cmd.arg("--no-bwrap");  // protontricks only
cmd.arg(&request.steam_app_id);
cmd.arg("-q");          // required: suppress interactive prompts
for package in &packages {
    cmd.arg(package);
}
```

For winetricks direct: `WINEPREFIX=<path> winetricks -q <VERB> [<VERB>...]` — same `-q` placement.

### Finding 2: No native "list installed" in protontricks — use winetricks directly

**Significant correction.** The spec assumed `protontricks <appid> list 2>&1` could check installed packages. API research shows there is no protontricks-native list command. Detection must use one of:

**Option A (preferred): Parse `$WINEPREFIX/winetricks.log`**
- File contains one verb per line in install order.
- Pure filesystem read — no process spawn.
- Fast, no timeout needed.
- Limitation: only records verbs installed via winetricks/protontricks; manual installs not reflected.
- Correct `WINEPREFIX` path = canonical `prefix_path/pfx/` (via `resolve_wine_prefix_path()`).

**Option B: `WINEPREFIX=<pfx> winetricks list-installed`**
- Spawns winetricks process, parses stdout (space-delimited verb names).
- Slower (1–3s), requires winetricks binary.
- More accurate for edge cases.

**Decision: Option A (winetricks.log parse) as primary; Option B as fallback when log absent.**

This is a major simplification — `check_prefix_dependencies` becomes a filesystem read for the common case, no process spawn needed. `checker.rs` implements:

```rust
pub fn is_package_installed(wineprefix: &Path, package: &str) -> bool {
    let log_path = wineprefix.join("winetricks.log");
    match std::fs::read_to_string(&log_path) {
        Ok(content) => content.lines().any(|line| line.trim() == package),
        Err(_) => false,  // log absent = not installed (trigger Option B fallback)
    }
}
```

When `winetricks.log` does not exist (prefix not yet initialized, or no winetricks installs ever performed), fall back to spawning `WINEPREFIX=<pfx> winetricks list-installed` with a 30-second timeout.

**Important:** If neither winetricks.log nor a winetricks binary is available, state is `check_failed` — not `missing`. This distinction matters for UX: `check_failed` means "we couldn't determine status" rather than "definitely not installed."

### Finding 3: `WINEPREFIX` path convention

**Confirmed:** The canonical `WINEPREFIX` is `~/.local/share/Steam/steamapps/compatdata/<APPID>/pfx/`. This matches `resolve_wine_prefix_path()` in `runtime_helpers.rs` which already handles the `pfx/` suffix resolution. No change to the spec needed — the existing helper is correct.

### Finding 4: Exit codes — only 0 vs non-zero is reliable

**Confirmed and spec updated.** Only exit code `0` (success) vs non-zero (failure) is reliable. The `install_prefix_dependency` command should treat any non-zero exit as `install_failed` and capture stderr content for `last_error`. No special-casing of specific exit codes.

### Finding 5: `which` crate — do not add as dependency

**Decision: Rejected.** The `which` crate is not in any Cargo.toml. Adding it for binary detection would be a new dependency for functionality already implemented in `resolve_umu_run_path()` in `runtime_helpers.rs`. The `binary.rs` module reuses the same manual PATH-scan pattern with `is_executable_file()` — both are already pub in `runtime_helpers`. No new dependency needed.

### Finding 6: `--no-bwrap` should be a configurable setting

**Accepted.** The API research cites multiple real-world environments where bwrap works fine (e.g. native installs), and some where it fails (AppImage, some container setups). Add `protontricks_no_bwrap: bool` (default `true` for safety) to `AppSettingsData` rather than hardcoding. The AppImage default should be `true`.

```rust
// In AppSettingsData:
/// Whether to pass --no-bwrap to protontricks (recommended for AppImage).
/// Default true. Disable only if protontricks fails with --no-bwrap on your system.
#[serde(default = "default_protontricks_no_bwrap")]
pub protontricks_no_bwrap: bool,

fn default_protontricks_no_bwrap() -> bool { true }
```

In `build_install_command`:
```rust
if binary_invocation.binary_name == "protontricks" && settings.protontricks_no_bwrap {
    cmd.arg("--no-bwrap");
}
```

### Finding 7: Flatpak detection via path, not subprocess

**Accepted and simplified.** The spec's original Flatpak detection called `flatpak run ... --version` as a subprocess. API research confirms a simpler approach: check if the resolved `protontricks` binary path is under `/var/lib/flatpak/` or `~/.local/share/flatpak/`. If so, set `source = "flatpak"` and emit a warning about secondary Steam library paths needing `flatpak override --filesystem=<path>`. No subprocess call needed for detection.

```rust
// In binary.rs:
fn is_flatpak_path(path: &Path) -> bool {
    let s = path.to_string_lossy();
    s.contains("/var/lib/flatpak/") || s.contains("/.local/share/flatpak/")
}
```

The `BinaryInvocation` for Flatpak protontricks stays as specified (`program = "flatpak"`, `leading_args = ["run", "--filesystem=host", "com.github.Matoking.protontricks"]`), but detection is path-based, not subprocess-based.

### Finding 8: Two invocation strategies

**Clarifies the winetricks fallback design.** When `binary_name == "winetricks"` and `steam_app_id` is empty:
- Use `WINEPREFIX=<resolved_pfx_path> winetricks -q <package>` with no App ID argument.

When `binary_name == "protontricks"` and `steam_app_id` is non-empty:
- Use `protontricks [--no-bwrap] <steam_app_id> -q <package>`.

When `binary_name == "protontricks"` and `steam_app_id` is empty:
- Cannot use protontricks — must surface `PrefixDepsError::SteamAppIdRequired` and suggest falling back to winetricks or configuring the Steam App ID.

This clarifies open question #3 (non-Steam prefixes): protontricks cannot install to non-Steam prefixes without an App ID. The answer is "winetricks fallback" — if no App ID, require winetricks or fail with a clear error.

---

## Revised `checker.rs` Design

```rust
// crosshook-core/src/prefix_deps/checker.rs

/// Check installation state for a single package.
/// Primary: parse winetricks.log (no process spawn).
/// Fallback: run `winetricks list-installed` when log is absent.
pub fn check_package_installed(
    wineprefix: &Path,
    package: &str,
    winetricks_binary: Option<&str>,
) -> DependencyState {
    let log_path = wineprefix.join("winetricks.log");

    match std::fs::read_to_string(&log_path) {
        Ok(content) => {
            if content.lines().any(|line| line.trim() == package) {
                DependencyState::Installed
            } else {
                DependencyState::Missing
            }
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            // Log absent: try winetricks list-installed as fallback
            match winetricks_binary {
                Some(bin) => check_via_winetricks_list(wineprefix, package, bin),
                None => DependencyState::CheckFailed,
            }
        }
        Err(_) => DependencyState::CheckFailed,
    }
}

fn check_via_winetricks_list(
    wineprefix: &Path,
    package: &str,
    binary: &str,
) -> DependencyState {
    // Synchronous — called from spawn_blocking context
    let output = std::process::Command::new(binary)
        .env("WINEPREFIX", wineprefix)
        .arg("list-installed")
        .output();

    match output {
        Ok(out) if out.status.success() => {
            let stdout = String::from_utf8_lossy(&out.stdout);
            if stdout.split_whitespace().any(|v| v == package) {
                DependencyState::Installed
            } else {
                DependencyState::Missing
            }
        }
        _ => DependencyState::CheckFailed,
    }
}
```

---

## Updated Files to Modify

The `AppSettingsData` changes now include three fields (not two):

| Field | Type | Default | Notes |
| --- | --- | --- | --- |
| `protontricks_binary_path` | `String` | `""` | Empty = auto-detect |
| `auto_check_prefix_deps` | `bool` | `false` | Prompt on profile load |
| `protontricks_no_bwrap` | `bool` | `true` | Pass `--no-bwrap` to protontricks |

---

## Open Questions — Status Update

1. **Protontricks `list` command output format** — **RESOLVED.** No such command exists. Detection uses `winetricks.log` parse (primary) or `winetricks list-installed` (fallback). See "CLI API Findings Integration" above.

2. **`--no-bwrap` availability** — **RESOLVED.** `--no-bwrap` is a standard protontricks flag. Made configurable via `protontricks_no_bwrap: bool` in `AppSettingsData` (default `true`). If a user's system requires bwrap, they disable the setting.

3. **Steam App ID for non-Steam prefixes** — **RESOLVED.** Protontricks requires an App ID; non-Steam profiles must use winetricks direct (`WINEPREFIX=<pfx> winetricks -q <package>`). If no App ID and binary is protontricks, surface `PrefixDepsError::SteamAppIdRequired` with remediation: configure steam.app_id or install winetricks.

4. **winetricks vs protontricks invocation divergence** — **RESOLVED.** `BinaryInvocation.binary_name` drives branching: protontricks uses `<appid> -q <packages>` (env + arg), winetricks uses `WINEPREFIX=<pfx> -q <packages>` (env only, no appid arg). Handled in `installer.rs` and `checker.rs`.

5. **WINE prefix initialization requirement** — **PARTIALLY RESOLVED.** A missing `winetricks.log` is handled gracefully (fallback to `winetricks list-installed`). A completely uninitialized prefix (no `pfx/system.reg`) will cause the winetricks fallback to fail and return `CheckFailed`. Adding an explicit uninitialized-prefix guard remains a recommended enhancement but is not blocking.
