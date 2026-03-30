# Integration Research: cli-completion

## Overview

All business logic for the 6 unimplemented CLI commands lives in `crosshook-core`. The wiring
work consists of calling specific public functions with the correct types, building `LaunchRequest`
structs for all three launch methods, and handling two output paths (human text + `--json`).
The `diagnostics export` command is the only fully-wired reference implementation in the CLI today.

---

## Core Function Signatures

### ProfileStore

All methods are on `crosshook_core::profile::ProfileStore` (sync, no async).

```rust
// Construction
pub fn try_new() -> Result<Self, String>
pub fn new() -> Self  // panics on missing home dir
pub fn with_base_path(base_path: PathBuf) -> Self

// CRUD
pub fn load(&self, name: &str) -> Result<GameProfile, ProfileStoreError>
pub fn save(&self, name: &str, profile: &GameProfile) -> Result<(), ProfileStoreError>
pub fn list(&self) -> Result<Vec<String>, ProfileStoreError>
pub fn delete(&self, name: &str) -> Result<(), ProfileStoreError>
pub fn rename(&self, old_name: &str, new_name: &str) -> Result<(), ProfileStoreError>
pub fn duplicate(&self, source_name: &str) -> Result<DuplicateProfileResult, ProfileStoreError>

// Legacy import
pub fn import_legacy(&self, legacy_path: &Path) -> Result<GameProfile, ProfileStoreError>

// Optimization management
pub fn save_launch_optimizations(
    &self,
    name: &str,
    enabled_option_ids: Vec<String>,
    switch_active_preset: Option<String>,
) -> Result<(), ProfileStoreError>
```

`list()` returns names sorted alphabetically (file stems of `.toml` files), never returns an error
for an empty directory.

### Community Profile Exchange

In `crosshook_core::profile` (re-exported from `exchange.rs`):

```rust
pub fn export_community_profile(
    profiles_dir: &Path,
    profile_name: &str,
    output_path: &Path,
) -> Result<CommunityExportResult, CommunityExchangeError>

pub fn import_community_profile(
    json_path: &Path,
    profiles_dir: &Path,
) -> Result<CommunityImportResult, CommunityExchangeError>

pub fn preview_community_profile_import(
    json_path: &Path,
) -> Result<CommunityImportPreview, CommunityExchangeError>
```

`export_community_profile` internally constructs a `ProfileStore::with_base_path(profiles_dir)` —
the caller only passes the directory, not a store instance.

### Launch Orchestration

In `crosshook_core::launch`:

```rust
pub fn validate(request: &LaunchRequest) -> Result<(), ValidationError>
pub fn validate_all(request: &LaunchRequest) -> Vec<LaunchValidationIssue>
pub fn analyze(exit_status: Option<ExitStatus>, log_tail: &str, method: &str) -> DiagnosticReport
pub fn should_surface_report(report: &DiagnosticReport) -> bool
```

In `crosshook_core::launch::script_runner`:

```rust
pub fn build_helper_command(
    request: &LaunchRequest,
    script_path: &Path,
    log_path: &Path,
) -> tokio::process::Command  // steam_applaunch only

pub fn build_proton_game_command(
    request: &LaunchRequest,
    log_path: &Path,
) -> std::io::Result<tokio::process::Command>  // proton_run

pub fn build_native_game_command(
    request: &LaunchRequest,
    log_path: &Path,
) -> std::io::Result<tokio::process::Command>  // native
```

`build_proton_game_command` and `build_native_game_command` return `std::io::Result` (can fail
creating/opening the log file). `build_helper_command` returns `Command` directly.

### Steam Discovery

In `crosshook_core::steam`:

```rust
pub fn discover_steam_root_candidates(
    steam_client_install_path: impl AsRef<Path>,
    diagnostics: &mut Vec<String>,
) -> Vec<PathBuf>

pub fn discover_compat_tools(
    steam_root_candidates: &[PathBuf],
    diagnostics: &mut Vec<String>,
) -> Vec<ProtonInstall>

pub fn attempt_auto_populate(
    request: &SteamAutoPopulateRequest,
) -> SteamAutoPopulateResult
```

`discover_steam_root_candidates` takes an empty string (or empty `Path`) to skip the explicit
hint and fall back to `HOME`-relative defaults. Passing an invalid path simply skips it with a
diagnostic entry; no error is returned.

### Diagnostics Export (already wired — reference pattern)

```rust
pub fn export_diagnostic_bundle(
    profile_store: &ProfileStore,
    settings_store: &SettingsStore,
    options: &DiagnosticBundleOptions,
) -> Result<DiagnosticBundleResult, DiagnosticBundleError>
```

---

## Data Models

### GameProfile (crossing CLI→core boundary)

`crosshook_core::profile::GameProfile` — the primary type; all fields are `String`/`bool`/`Vec`
with `#[serde(default)]`. Key sections:

| Section          | Key fields                                                                |
| ---------------- | ------------------------------------------------------------------------- |
| `game`           | `name: String`, `executable_path: String`                                 |
| `trainer`        | `path: String`, `kind: String`, `loading_mode: TrainerLoadingMode`        |
| `injection`      | `dll_paths: Vec<String>`, `inject_on_launch: Vec<bool>`                   |
| `steam`          | `enabled: bool`, `app_id: String`, `compatdata_path`, `proton_path`       |
| `runtime`        | `prefix_path: String`, `proton_path: String`, `working_directory: String` |
| `launch`         | `method: String`, `optimizations: LaunchOptimizationsSection`             |
| `local_override` | machine-specific path overrides layered over portable base fields         |

The `load()` method applies `effective_profile()` (merges local overrides) before returning, so
the caller receives a flat, resolved `GameProfile` with no `local_override` fields set.

### LaunchRequest

`crosshook_core::launch::LaunchRequest` — the central struct for all three launch methods:

```rust
pub struct LaunchRequest {
    pub method: String,               // "steam_applaunch" | "proton_run" | "native"
    pub game_path: String,
    pub trainer_path: String,
    pub trainer_host_path: String,
    pub trainer_loading_mode: TrainerLoadingMode,
    pub steam: SteamLaunchConfig,     // steam_applaunch fields
    pub runtime: RuntimeLaunchConfig, // proton_run fields
    pub optimizations: LaunchOptimizationsRequest,
    pub launch_trainer_only: bool,
    pub launch_game_only: bool,
    pub profile_name: Option<String>,
}

pub struct SteamLaunchConfig {
    pub app_id: String,
    pub compatdata_path: String,
    pub proton_path: String,
    pub steam_client_install_path: String,  // required for steam_applaunch
}

pub struct RuntimeLaunchConfig {
    pub prefix_path: String,   // required for proton_run
    pub proton_path: String,   // required for proton_run
    pub working_directory: String,  // optional
}
```

### SteamAutoPopulateRequest / Result

```rust
pub struct SteamAutoPopulateRequest {
    pub game_path: PathBuf,
    pub steam_client_install_path: PathBuf,  // empty PathBuf = auto-detect
}

pub struct SteamAutoPopulateResult {
    pub app_id_state: SteamAutoPopulateFieldState,  // NotFound | Found | Ambiguous
    pub app_id: String,
    pub compatdata_state: SteamAutoPopulateFieldState,
    pub compatdata_path: PathBuf,
    pub proton_state: SteamAutoPopulateFieldState,
    pub proton_path: PathBuf,
    pub diagnostics: Vec<String>,
    pub manual_hints: Vec<String>,
}
```

### CommunityExportResult / ImportResult

```rust
pub struct CommunityExportResult {
    pub profile_name: String,
    pub output_path: PathBuf,
    pub manifest: CommunityProfileManifest,
}

pub struct CommunityImportResult {
    pub profile_name: String,
    pub source_path: PathBuf,
    pub profile_path: PathBuf,
    pub profile: GameProfile,
    pub manifest: CommunityProfileManifest,
}
```

### ProtonInstall

```rust
pub struct ProtonInstall {
    pub name: String,
    pub path: PathBuf,   // points to the `proton` executable
    pub is_official: bool,
    pub aliases: Vec<String>,
    pub normalized_aliases: BTreeSet<String>,
}
```

### DuplicateProfileResult

```rust
pub struct DuplicateProfileResult {
    pub name: String,          // generated copy name e.g. "MyGame (Copy)"
    pub profile: GameProfile,
}
```

---

## External Services

No external APIs or remote services are involved. All integrations are local filesystem and process-based.

## Filesystem Integration

### Profile Store Path

`ProfileStore::try_new()` resolves to `$XDG_CONFIG_HOME/crosshook/profiles` (or `~/.config/crosshook/profiles` when `XDG_CONFIG_HOME` is unset), using the `directories` crate `BaseDirs::config_dir()`.

Override with `--config <PATH>` or `--profile-dir <PATH>` → `ProfileStore::with_base_path(path)`.

Each profile is stored as `<name>.toml`.

### Settings Store Path

`SettingsStore::try_new()` resolves to `~/.config/crosshook/settings.toml`.

### Launch Log Path

The existing `launch_profile()` function in `main.rs` uses:

```rust
fn launch_log_path(profile_name: &str) -> PathBuf {
    PathBuf::from("/tmp/crosshook-logs").join(format!("{safe_name}.log"))
}
```

The Tauri app uses `create_log_path(slug, target)` in `src-tauri/src/commands/shared.rs` which
also resolves to `/tmp/crosshook-logs/`. Same directory for both CLI and Tauri.

### Legacy Profile Path

Legacy `.profile` files (key=value format) are separate from TOML profiles. `import_legacy` reads
from the path provided; the profile name is derived from the file stem.

### Community Export Output Path

`export_community_profile` takes an explicit `output_path: &Path`. The CLI `profile export`
command takes `--output <PATH>`; when unset, a sensible default should be constructed (e.g.,
`~/.config/crosshook/exports/<profile_name>.json`).

---

## Launch System

### Method Selection

`resolve_launch_method(&profile)` (from `crosshook_core::profile::models`) auto-detects the
method when `profile.launch.method` is blank:

1. `"steam_applaunch"` if `profile.steam.enabled == true`
2. `"proton_run"` if `game_path` ends with `.exe`
3. `"native"` otherwise

The `LaunchRequest::resolved_method()` applies the same logic on the request struct.

### Building LaunchRequest from GameProfile (per method)

**steam_applaunch** (current pattern in `main.rs:steam_launch_request_from_profile`):

```rust
LaunchRequest {
    method: METHOD_STEAM_APPLAUNCH.to_string(),
    game_path: profile.game.executable_path.clone(),
    trainer_path: profile.trainer.path.clone(),
    trainer_host_path: profile.trainer.path.clone(),
    trainer_loading_mode: profile.trainer.loading_mode,
    steam: SteamLaunchConfig {
        app_id: profile.steam.app_id.clone(),
        compatdata_path: profile.steam.compatdata_path.clone(),
        proton_path: profile.steam.proton_path.clone(),
        steam_client_install_path: <resolved from env or profile>,
    },
    runtime: RuntimeLaunchConfig::default(),
    optimizations: LaunchOptimizationsRequest::default(),
    launch_game_only: true,
    launch_trainer_only: false,
    profile_name: Some(profile_name.to_string()),
}
```

**proton_run** (new, modeled after Tauri `launch.rs`):

```rust
LaunchRequest {
    method: METHOD_PROTON_RUN.to_string(),
    game_path: profile.game.executable_path.clone(),
    trainer_path: profile.trainer.path.clone(),
    trainer_host_path: profile.trainer.path.clone(),
    trainer_loading_mode: profile.trainer.loading_mode,
    steam: SteamLaunchConfig::default(),  // unused for proton_run
    runtime: RuntimeLaunchConfig {
        prefix_path: profile.runtime.prefix_path.clone(),
        proton_path: profile.runtime.proton_path.clone(),
        working_directory: profile.runtime.working_directory.clone(),
    },
    optimizations: LaunchOptimizationsRequest {
        enabled_option_ids: profile.launch.optimizations.enabled_option_ids.clone(),
    },
    launch_game_only: true,
    launch_trainer_only: false,
    profile_name: Some(profile_name.to_string()),
}
```

**native**:

```rust
LaunchRequest {
    method: METHOD_NATIVE.to_string(),
    game_path: profile.game.executable_path.clone(),
    trainer_path: String::new(),    // trainer unsupported for native
    trainer_host_path: String::new(),
    trainer_loading_mode: TrainerLoadingMode::default(),
    steam: SteamLaunchConfig::default(),
    runtime: RuntimeLaunchConfig {
        working_directory: profile.runtime.working_directory.clone(),
        ..RuntimeLaunchConfig::default()
    },
    optimizations: LaunchOptimizationsRequest::default(),
    launch_game_only: true,
    launch_trainer_only: false,
    profile_name: Some(profile_name.to_string()),
}
```

### Spawn and Stream Pattern

For `proton_run` and `native`, commands are `tokio::process::Command` returned by
`build_proton_game_command` / `build_native_game_command` — the existing
`spawn_helper` + `stream_helper_log` functions in `main.rs` cannot be reused as-is because they
use `Stdio::null()` on stdout/stderr and build a script command. For direct proton/native
commands, the log attachment is handled inside `build_proton_game_command` via
`attach_log_stdio(&mut command, log_path)`.

The post-launch analysis loop using `analyze()` + `should_surface_report()` applies to all three
methods, as shown in the existing `launch_profile()` function.

### Validate-only vs Execute

`validate(&request)` returns `Err(ValidationError)` on first fatal issue. For `proton_run`,
`validate_proton_run` internally calls `resolve_launch_directives(request)?`, which validates
that all `enabled_option_ids` are known catalog IDs and checks for incompatibilities/dependencies.

---

## Import/Export

### Legacy Import (`profile import --legacy-path <path>`)

```rust
store.import_legacy(legacy_path: &Path) -> Result<GameProfile, ProfileStoreError>
```

- Derives profile name from `legacy_path.file_stem()`.
- Reads `<stem>.profile` (key=value text format) from `legacy_path.parent()`.
- Converts via `GameProfile::from(LegacyProfileData)` applying `derive_launch_method_from_legacy`.
- Saves as TOML under `store.base_path/<name>.toml` (overwrites existing).
- Returns the converted `GameProfile`.

The legacy format normalizes `Z:\...` Windows paths to `/...` via `normalize_legacy_windows_path`.

### Community Export (`profile export --profile <name> [--output <path>]`)

```rust
export_community_profile(
    profiles_dir: &Path,
    profile_name: &str,
    output_path: &Path,
) -> Result<CommunityExportResult, CommunityExchangeError>
```

- Sanitizes machine-specific paths (clears `executable_path`, `trainer.path`, `dll_paths`,
  `compatdata_path`, `steam.proton_path`, `runtime.proton_path`, `working_directory`,
  `launcher.icon_path`) before writing.
- Output is a JSON file with `schema_version`, `metadata`, and `profile` fields.
- `profiles_dir` must be the directory containing `.toml` files (i.e. `store.base_path`).

---

## Error Types

### ProfileStoreError

```rust
pub enum ProfileStoreError {
    InvalidName(String),
    NotFound(PathBuf),
    AlreadyExists(String),
    InvalidLaunchOptimizationId(String),
    LaunchPresetNotFound(String),
    ReservedLaunchPresetName(String),
    InvalidLaunchPresetName(String),
    Io(std::io::Error),
    TomlDe(toml::de::Error),
    TomlSer(toml::ser::Error),
}
```

Implements `Display` and `std::error::Error`. Safe to convert with `.to_string()` for CLI output.

### ValidationError

```rust
pub enum ValidationError {
    GamePathRequired, GamePathMissing, GamePathNotFile,
    TrainerPathRequired, TrainerHostPathRequired, TrainerHostPathMissing, TrainerHostPathNotFile,
    SteamAppIdRequired,
    SteamCompatDataPathRequired, SteamCompatDataPathMissing, SteamCompatDataPathNotDirectory,
    SteamProtonPathRequired, SteamProtonPathMissing, SteamProtonPathNotExecutable,
    SteamClientInstallPathRequired,
    RuntimePrefixPathRequired, RuntimePrefixPathMissing, RuntimePrefixPathNotDirectory,
    RuntimeProtonPathRequired, RuntimeProtonPathMissing, RuntimeProtonPathNotExecutable,
    UnknownLaunchOptimization(String),
    DuplicateLaunchOptimization(String),
    LaunchOptimizationsUnsupportedForMethod(String),
    LaunchOptimizationNotSupportedForMethod { option_id: String, method: String },
    IncompatibleLaunchOptimizations { first: String, second: String },
    LaunchOptimizationDependencyMissing { option_id: String, dependency: String },
    NativeWindowsExecutableNotSupported,
    NativeTrainerLaunchUnsupported,
    UnsupportedMethod(String),
}
```

`.severity()` always returns `ValidationSeverity::Fatal`. `.message()` and `.help()` provide
human-readable descriptions. `.issue()` wraps into `LaunchValidationIssue` (the IPC form).

### CommunityExchangeError

```rust
pub enum CommunityExchangeError {
    Io { action: String, path: PathBuf, message: String },
    Json { path: PathBuf, message: String },
    InvalidManifest { message: String },
    UnsupportedSchemaVersion { version: u32, supported: u32 },
    ProfileStore { message: String },
}
```

Implements `Display` and `std::error::Error`. The `ProfileStore` variant wraps a
`ProfileStoreError.to_string()`.

### DiagnosticBundleError

```rust
pub enum DiagnosticBundleError {
    Io { action: &'static str, path: PathBuf, source: io::Error },
    Archive(String),
    ProfileStore(String),
}
```

### io::Result from script_runner

`build_proton_game_command` and `build_native_game_command` return `std::io::Result<Command>`.
Failure occurs when `attach_log_stdio` cannot open the log file (parent directory creation
is not automatic — caller must ensure `/tmp/crosshook-logs/` exists or handle the error).

---

## Edgecases

- `discover_steam_root_candidates` accepts an empty string/empty Path with no error — it simply skips the explicit hint and uses HOME-based fallbacks. Passing `""` is the correct way to do a default discovery.
- `ProfileStore::load()` always applies `effective_profile()` and clears `local_override` before returning — the returned `GameProfile` has machine-specific paths promoted to their canonical field positions, not in `local_override`.
- `export_community_profile` internally constructs a `ProfileStore::with_base_path(profiles_dir)` — do not pass a store instance; pass the directory path directly.
- `import_legacy` derives the profile name from `legacy_path.file_stem()`, not from the profile contents. If the file stem contains characters invalid for a profile name, `InvalidName` is returned before any disk I/O.
- The `method` field on `LaunchRequest` may be an empty string. `resolved_method()` auto-detects in that case, but `validate()` only accepts explicit known values or empty string (empty string defers to `resolved_method()`). Passing an unknown non-empty string produces `UnsupportedMethod`.
- `build_proton_game_command` calls `resolve_launch_directives(request)` internally (same as `validate_proton_run`) — calling `validate` then `build_proton_game_command` is redundant for optimization validation but not harmful.
- The `steam_client_install_path` field in `SteamLaunchConfig` is required for `steam_applaunch` validation (`SteamClientInstallPathRequired`) but is not populated by `ProfileStore::load()` — the CLI must resolve it from `$STEAM_COMPAT_CLIENT_INSTALL_PATH` env var or by walking `compatdata_path` ancestors (current logic in `resolve_steam_client_install_path` in `main.rs`).
- Log path parent directory (`/tmp/crosshook-logs`) must be created before calling `build_proton_game_command` or `build_native_game_command` — `attach_log_stdio` opens the file for writing but does not create parent directories.

## Relevant Files

- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-cli/src/main.rs`: All CLI dispatch; existing steam_applaunch + diagnostics wiring
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-cli/src/args.rs`: Clap arg structs for all commands
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/profile/toml_store.rs`: `ProfileStore` with all CRUD methods
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/profile/exchange.rs`: `export_community_profile`, `import_community_profile`
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/profile/models.rs`: `GameProfile`, all section structs, `resolve_launch_method`
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/profile/legacy.rs`: Legacy `.profile` key=value reader
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/launch/request.rs`: `LaunchRequest`, `validate()`, `ValidationError`
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/launch/script_runner.rs`: `build_helper_command`, `build_proton_game_command`, `build_native_game_command`
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/launch/diagnostics/mod.rs`: `analyze()`, `should_surface_report()`
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/steam/mod.rs`: Public re-exports for all Steam functions
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/steam/models.rs`: `SteamAutoPopulateRequest`, `SteamAutoPopulateResult`, `ProtonInstall`
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/steam/discovery.rs`: `discover_steam_root_candidates`
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/steam/proton.rs`: `discover_compat_tools`
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/steam/auto_populate.rs`: `attempt_auto_populate`
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/settings/mod.rs`: `SettingsStore`, `AppSettingsData`
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/export/diagnostics.rs`: `DiagnosticBundleOptions`, `DiagnosticBundleResult`
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src-tauri/src/commands/launch.rs`: Reference for proton_run + native wiring (lines 52–116)
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src-tauri/src/commands/steam.rs`: Reference for steam discover + auto-populate wiring
