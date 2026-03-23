# Code Analysis: Platform-Native UI Port

This document extracts actionable code patterns, data structures, and integration points from the existing C# codebase to inform the Tauri/Rust + React/TypeScript implementation of CrossHook's platform-native UI. Every service file, shell script, and the MainForm launch orchestration (lines 2648-2946) have been analyzed for exact field lists, algorithm details, and behavioral contracts.

## Executive Summary

The existing codebase follows a consistent Request/Result validation pattern across all services, uses static classes for stateless operations (Steam discovery, launch) and instance classes with a `base_path` for stateful ones (profiles, settings, recent files). The VDF parser is a ~125-line recursive-descent parser that must be ported or replaced with a Rust crate (case-insensitive dictionary keys are critical). The launch orchestration is a two-phase state machine driven by a single boolean (`_steamTrainerLaunchPending`). Shell scripts are reused directly -- they already run natively on Linux, accept CLI arguments via `--flag value` pairs, and handle their own environment isolation via `setsid env -i`.

## Existing Code Structure

### Service Architecture Summary

| Service                              | Type           | State                                     | Key Methods                                                                                | Lines |
| ------------------------------------ | -------------- | ----------------------------------------- | ------------------------------------------------------------------------------------------ | ----- |
| `SteamAutoPopulateService`           | `static class` | None                                      | `AttemptAutoPopulate`, `ParseKeyValueFile`, `ResolveProtonPath`                            | ~1286 |
| `SteamLaunchService`                 | `static class` | None                                      | `Validate`, `CreateHelperStartInfo`, `ConvertToUnixPath`, `GetEnvironmentVariablesToClear` | ~737  |
| `SteamExternalLauncherExportService` | `static class` | None                                      | `Validate`, `ExportLaunchers`, `BuildTrainerScriptContent`, `BuildDesktopEntryContent`     | ~350  |
| `ProfileService`                     | `sealed class` | `_profilesDirectoryPath`                  | `GetProfileNames`, `SaveProfile`, `LoadProfile`, `DeleteProfile`                           | ~225  |
| `AppSettingsService`                 | `sealed class` | `_settingsDirectoryPath`, `_settingsPath` | `LoadAppSettings`, `SaveAppSettings`                                                       | ~81   |
| `RecentFilesService`                 | `sealed class` | `_settingsPath`                           | `LoadRecentFiles`, `SaveRecentFiles`                                                       | ~131  |
| `CommandLineParser`                  | `sealed class` | None                                      | `Parse`                                                                                    | ~54   |

### Data Types -- Complete Field Lists

#### ProfileData (12 fields)

```
GamePath: String (default: "")
TrainerPath: String (default: "")
Dll1Path: String (default: "")
Dll2Path: String (default: "")
LaunchInject1: bool (default: false)
LaunchInject2: bool (default: false)
LaunchMethod: String (default: "")
UseSteamMode: bool (default: false)
SteamAppId: String (default: "")
SteamCompatDataPath: String (default: "")
SteamProtonPath: String (default: "")
SteamLauncherIconPath: String (default: "")
```

Serialization format: `Key=Value` lines, no quoting, no escaping, splits on first `=`. Boolean values parsed via `bool.TryParse` (case-insensitive: "True"/"False"). Write order matches the field list above exactly. File extension: `.profile`.

**Rust TOML equivalent** (from feature-spec.md): Convert to TOML with `serde`. The native app should use TOML for new profiles but also support reading legacy `.profile` files for migration.

#### AppSettingsData (2 fields)

```
AutoLoadLastProfile: bool (default: false)
LastUsedProfile: String (default: "")
```

File: `Settings/AppSettings.ini`, same `Key=Value` format.

#### RecentFilesData (3 lists)

```
GamePaths: List<String>
TrainerPaths: List<String>
DllPaths: List<String>
```

File: `settings.ini`, INI-style with `[RecentGamePaths]`, `[RecentTrainerPaths]`, `[RecentDllPaths]` sections. Entries are raw file paths, one per line. Lines starting with `;` are comments. Empty lines are skipped. **Critical**: entries where `File.Exists(line)` is false are silently dropped on load.

#### SteamAutoPopulateResult (7 fields)

```
SteamAppIdState: SteamAutoPopulateFieldState (NotFound | Found | Ambiguous)
SteamAppId: String (default: "")
SteamCompatDataPathState: SteamAutoPopulateFieldState
SteamCompatDataPath: String (default: "")
SteamProtonPathState: SteamAutoPopulateFieldState
SteamProtonPath: String (default: "")
Diagnostics: List<String> (deduplicated)
ManualHints: List<String> (deduplicated)
HasAnyMatch: bool (computed -- true if any field state is Found)
```

The three-state enum (`NotFound`, `Found`, `Ambiguous`) is critical for UI behavior: `Found` auto-fills the field, `Ambiguous` leaves it unchanged with a warning, `NotFound` leaves it unchanged with guidance.

#### SteamLaunchRequest (9 fields)

```
GamePath: String
TrainerPath: String
TrainerHostPath: String (normalized Unix path of TrainerPath)
SteamAppId: String
SteamCompatDataPath: String
SteamProtonPath: String
SteamClientInstallPath: String
LaunchTrainerOnly: bool
LaunchGameOnly: bool
```

`LaunchTrainerOnly` and `LaunchGameOnly` are mutually exclusive phase selectors. Phase 1 sets `LaunchGameOnly=true, LaunchTrainerOnly=false`. Phase 2 sets `LaunchTrainerOnly=true, LaunchGameOnly=false`.

#### SteamLaunchValidationResult / SteamLaunchExecutionResult

Validation: `IsValid: bool`, `ErrorMessage: String`. Six required-field checks (GamePath, TrainerPath, TrainerHostPath, SteamAppId, SteamCompatDataPath, SteamProtonPath, SteamClientInstallPath). GamePath is only required when `LaunchTrainerOnly` is false.

Execution: `Succeeded: bool`, `Message: String`, `HelperLogPath: String`.

#### SteamExternalLauncherExportRequest (8 fields)

```
LauncherName: String
TrainerPath: String
LauncherIconPath: String
SteamAppId: String
SteamCompatDataPath: String
SteamProtonPath: String
SteamClientInstallPath: String
TargetHomePath: String
```

#### CommandLineOptions (3 fields)

```
ProfilesToLoad: List<String> (populated via -p flag, repeatable)
AutoLaunchPath: String (populated via -autolaunch flag, consumes rest of args)
AutoLaunchRequested: bool
```

### Internal Data Structures (SteamAutoPopulateService)

#### SteamKeyValueNode (VDF parse tree)

```
Value: String (leaf value, default: "")
Children: Dictionary<String, SteamKeyValueNode> (case-insensitive keys via StringComparer.OrdinalIgnoreCase)
GetChild(key): SteamKeyValueNode? (case-insensitive lookup)
```

#### SteamLibraryInfo

```
LibraryPath: String (e.g., "/home/user/.local/share/Steam")
SteamAppsPath: String (e.g., "/home/user/.local/share/Steam/steamapps")
```

#### SteamGameMatch

```
SteamAppId: String
LibraryPath: String
InstallDirectoryPath: String
ManifestPath: String
```

#### SteamCompatToolInstall

```
ProtonPath: String (path to the `proton` executable)
IsOfficial: bool (true if under steamapps/common, false if compatibilitytools.d or system)
Aliases: List<String> (directory name + VDF display_name + VDF compat_tools keys)
NormalizedAliases: Set<String> (lowercase, alphanumeric only -- for fuzzy matching)
```

## Implementation Patterns

### Pattern 1: Request/Validate/Execute Pipeline

Every service follows this exact pattern:

1. Build a request DTO with all needed data
2. Call `Validate(request)` returning a validation result
3. If valid, call the execution method
4. Return a structured result (never throw for expected failures)

**C# example (SteamLaunchService)**:

```
request = BuildSteamLaunchRequest()       // MainForm builds DTO
validation = SteamLaunchService.Validate(request)  // static validation
if (!validation.IsValid) { show error; return }
result = RunSteamLaunchHelper(request)     // execution
```

**Rust equivalent**:

```rust
// Module-level function for stateless services
pub fn validate(request: &SteamLaunchRequest) -> Result<(), ValidationError> { ... }
pub fn execute(request: &SteamLaunchRequest) -> Result<SteamLaunchResult, LaunchError> { ... }

// Struct with base_path for stateful services
impl ProfileService {
    pub fn new(base_path: PathBuf) -> Self { ... }
    pub fn load(&self, name: &str) -> Result<ProfileData, ProfileError> { ... }
}
```

**Tauri IPC mapping**: Each service method becomes a `#[tauri::command]` function. The frontend calls `invoke("validate_steam_launch", { request })` and receives a typed result.

### Pattern 2: VDF Parser (Recursive Descent)

The VDF parser (lines 739-863 of SteamAutoPopulateService.cs) is a custom recursive-descent parser with these rules:

1. **Tokens**: Either quoted strings (with `\n`, `\r`, `\t`, `\\`, `\"` escape sequences) or unquoted runs of non-whitespace/non-brace characters
2. **Structure**: `key value` pairs where value is either a token or a `{ ... }` block (recursive)
3. **Comments**: `//` to end of line
4. **Dictionary keys**: Case-insensitive (`StringComparer.OrdinalIgnoreCase`)
5. **Descendant search**: `FindDescendantByKey` does recursive DFS with case-insensitive key comparison

**Critical for Rust port**: If using a `steam-vdf-parser` crate, verify it handles:

- Case-insensitive key lookups (the C# code uses `OrdinalIgnoreCase` for all dictionary keys)
- Recursive descendant search (not just direct children)
- `//` comment lines
- Escape sequences in quoted strings

**Files parsed by the auto-populate pipeline**:

- `steamapps/libraryfolders.vdf` -- library discovery
- `appmanifest_*.acf` -- game manifest matching (keys: `AppState.appid`, `AppState.installdir`)
- `config/config.vdf` -- compat tool mappings (key path: `...CompatToolMapping.<appid>.name`)
- `userdata/<id>/config/localconfig.vdf` -- per-user compat tool mappings
- `compatibilitytool.vdf` -- tool metadata (keys: `compat_tools.<name>`, `compat_tools.<name>.display_name`)

### Pattern 3: Environment Variable Cleanup (30 variables)

The exact list of environment variables that MUST be stripped before launching a trainer. This list appears in three places and must stay synchronized:

**SteamLaunchService.GetEnvironmentVariablesToClear() -- C# side (used by start.exe bridge)**:

```
WINESERVER, WINELOADER, WINEDLLPATH, WINEDLLOVERRIDES, WINEDEBUG,
WINEESYNC, WINEFSYNC, WINELOADERNOEXEC, WINE_LARGE_ADDRESS_AWARE,
WINE_DISABLE_KERNEL_WRITEWATCH, WINE_HEAP_DELAY_FREE, WINEFSYNC_SPINCOUNT,
LD_PRELOAD, LD_LIBRARY_PATH,
GST_PLUGIN_PATH, GST_PLUGIN_SYSTEM_PATH, GST_PLUGIN_SYSTEM_PATH_1_0,
SteamGameId, SteamAppId, GAMEID,
PROTON_LOG, PROTON_DUMP_DEBUG_COMMANDS, PROTON_USE_WINED3D,
PROTON_NO_ESYNC, PROTON_NO_FSYNC, PROTON_ENABLE_NVAPI,
DXVK_CONFIG_FILE, DXVK_STATE_CACHE_PATH, DXVK_LOG_PATH,
VKD3D_CONFIG, VKD3D_DEBUG
```

**Shell scripts (steam-launch-helper.sh line 264, steam-host-trainer-runner.sh line 152) -- `unset` block**:
Same list minus `WINE_HEAP_DELAY_FREE` and `WINEFSYNC_SPINCOUNT` (those two only appear in the C# list). The shell scripts also unset `WINEPREFIX` (then re-export it).

**For the native Rust app**: Since the Rust process runs natively on Linux (not under WINE), there is no inherited WINE environment to strip. However, the shell scripts handle their own cleanup. The Rust side only needs to ensure it does NOT pass WINE variables when spawning child processes. The shell scripts' `setsid env -i` pattern already guarantees a clean environment.

### Pattern 4: Two-Phase Launch State Machine

**State variable**: `_steamTrainerLaunchPending: bool` (MainForm field)

**Phase transitions** (MainForm.LaunchSteamModeAsync, lines 2767-2828):

```
Phase 1 (Initial state: _steamTrainerLaunchPending = false):
  -> BuildSteamLaunchRequest() sets LaunchGameOnly=true, LaunchTrainerOnly=false
  -> RunSteamLaunchHelper() calls steam-launch-helper.sh with --game-only
  -> On success: set _steamTrainerLaunchPending = true, update UI, minimize window
  -> Start log streaming in background

Phase 2 (After user clicks "Launch Trainer": _steamTrainerLaunchPending = true):
  -> BuildSteamLaunchRequest() sets LaunchTrainerOnly=true, LaunchGameOnly=false
  -> RunSteamLaunchHelper() calls steam-launch-trainer.sh (NOT steam-launch-helper.sh)
  -> steam-launch-trainer.sh invokes steam-host-trainer-runner.sh via setsid env -i
```

**Script selection logic** (RunSteamLaunchHelper, line 2834):

- `LaunchTrainerOnly=true` -> `SteamLaunchService.ResolveTrainerScriptPath()` -> `steam-launch-trainer.sh`
- `LaunchTrainerOnly=false` -> `SteamLaunchService.ResolveHelperScriptPath()` -> `steam-launch-helper.sh`

**React state equivalent**:

```typescript
enum LaunchPhase {
  Idle,
  GameLaunching, // Phase 1 in progress
  WaitingForTrainer, // Phase 1 complete, waiting for user
  TrainerLaunching, // Phase 2 in progress
  SessionActive, // Both launched
}
```

### Pattern 5: Shell Script CLI Interface

#### steam-launch-helper.sh (full orchestrator)

Arguments:

```
--appid <steam_app_id>           (required)
--compatdata <unix_path>         (required)
--proton <unix_path>             (required)
--steam-client <unix_path>       (required)
--game-exe-name <filename>       (required, e.g., "game.exe")
--trainer-path <windows_path>    (required, e.g., "C:\CrossHook\StagedTrainers\trainer.exe")
--trainer-host-path <unix_path>  (required, original host path of trainer)
--log-file <unix_path>           (required)
--game-startup-delay-seconds <n> (default: 30)
--game-timeout-seconds <n>       (default: 90)
--trainer-timeout-seconds <n>    (default: 10)
--trainer-only                   (flag, no value)
--game-only                      (flag, no value)
```

Behavior: If `--game-only`, launches game via `steam -applaunch <appid>`, waits for process, exits. If `--trainer-only`, skips game launch, runs trainer directly. Otherwise: launches game, waits, then launches trainer.

#### steam-launch-trainer.sh (trainer-only launcher)

Arguments:

```
--compatdata <unix_path>         (required)
--proton <unix_path>             (required)
--steam-client <unix_path>       (required)
--trainer-path <windows_path>    (required)
--trainer-host-path <unix_path>  (required)
--log-file <unix_path>           (required)
```

Behavior: Resolves the co-located `steam-host-trainer-runner.sh` script. Launches it via `setsid env -i` with only essential environment variables passed through: `HOME`, `USER`, `LOGNAME`, `SHELL`, `PATH`, `DISPLAY`, `WAYLAND_DISPLAY`, `XDG_RUNTIME_DIR`, `DBUS_SESSION_BUS_ADDRESS`.

#### steam-host-trainer-runner.sh (clean-env runner)

Same arguments as `steam-launch-trainer.sh`. Behavior: Closes inherited file descriptors > 2, unsets all WINE/Proton variables, re-exports the three required variables (`STEAM_COMPAT_DATA_PATH`, `STEAM_COMPAT_CLIENT_INSTALL_PATH`, `WINEPREFIX`), stages trainer into compatdata, runs `$proton run "$trainer_path"`.

**Trainer staging** (all scripts share this pattern):

```bash
staged_trainer_directory_path="$compatdata/pfx/drive_c/CrossHook/StagedTrainers"
trainer_file_name="$(basename "$trainer_host_path")"
staged_trainer_host_path="$staged_trainer_directory_path/$trainer_file_name"
staged_trainer_windows_path="C:\\CrossHook\\StagedTrainers\\$trainer_file_name"
mkdir -p "$staged_trainer_directory_path"
cp -f "$trainer_host_path" "$staged_trainer_host_path"
```

### Pattern 6: Path Conversion (WINE-specific, NOT needed in native app)

The C# codebase contains significant path conversion logic that is specific to running inside WINE:

- `ConvertToUnixPath()`: Uses `winepath.exe -u` to convert Windows paths to Unix
- `ConvertToWindowsPath()`: Uses `winepath.exe -w` to convert Unix paths to Windows
- `NormalizeSteamHostPath()`: Converts + resolves dosdevices symlinks
- `ResolveDosDevicesPath()`: Traverses `pfx/dosdevices/` symlinks to find real host paths

**For the native Rust app**: NONE of this path conversion logic is needed. The native app works with Linux paths directly. This eliminates ~400 lines of the most complex and fragile code in `SteamLaunchService.cs`.

### Pattern 7: Log Streaming

MainForm.StreamSteamHelperLogAsync (lines 2908-2946) implements a tail-follow pattern:

1. Open log file with `FileShare.ReadWrite` (so the script can still write)
2. Track `lastPosition` (byte offset)
3. Poll every 500ms for new content
4. Emit non-empty lines to UI console
5. Timeout after 2 minutes

**Tauri equivalent**: Use `tauri::ipc::Channel` or a Tauri event to stream log lines from the Rust backend to the React frontend. The Rust backend can use `tokio::fs::File` + `BufReader` with a poll loop or `notify` crate for file change events.

### Pattern 8: Auto-Populate Pipeline

The full auto-populate pipeline in `SteamAutoPopulateService.AttemptAutoPopulate`:

1. Normalize game path for host lookup (handle dosdevices, Windows paths)
2. Discover Steam root candidates (from `STEAM_COMPAT_CLIENT_INSTALL_PATH` env var, or `~/.steam/root`, or `~/.local/share/Steam`)
3. Discover Steam libraries (parse `steamapps/libraryfolders.vdf` in each root)
4. Resolve game path against Steam libraries (handle dosdevices paths that match library leaf names)
5. Find game match by scanning `appmanifest_*.acf` files and checking if game path is inside the manifest's `installdir`
6. Derive `compatdata` path from matched library + appid
7. Resolve Proton path by parsing `config/config.vdf` and `localconfig.vdf` for compat tool mappings, then matching against discovered tool installs
8. Accumulate diagnostics and manual hints throughout

**For the native app**: Steps 1 and 4 are dramatically simplified because paths are already native Linux paths. The VDF parsing (step 3, 5, 7) and manifest matching (step 5) must be ported in full.

### Pattern 9: Startup Coordination

`MainFormStartupCoordinator.ResolveAutoLoadProfileName`:

1. Check if `AutoLoadLastProfile` setting is true AND `LastUsedProfile` is non-empty
2. If CLI `--p` profiles were specified, skip auto-load (CLI takes priority)
3. Verify `LastUsedProfile` exists in the available profiles list
4. Return the profile name if found, empty string otherwise

## Integration Points

### Tauri IPC Commands Needed

Map each C# service method to a `#[tauri::command]` function:

| Tauri Command               | Source C# Method                                              | Input                               | Output                    |
| --------------------------- | ------------------------------------------------------------- | ----------------------------------- | ------------------------- |
| `list_profiles`             | `ProfileService.GetProfileNames()`                            | none                                | `Vec<String>`             |
| `load_profile`              | `ProfileService.LoadProfile(name)`                            | `name: String`                      | `ProfileData`             |
| `save_profile`              | `ProfileService.SaveProfile(name, data)`                      | `name: String, data: ProfileData`   | `()`                      |
| `delete_profile`            | `ProfileService.DeleteProfile(name)`                          | `name: String`                      | `()`                      |
| `load_settings`             | `AppSettingsService.LoadAppSettings()`                        | none                                | `AppSettingsData`         |
| `save_settings`             | `AppSettingsService.SaveAppSettings(data)`                    | `data: AppSettingsData`             | `()`                      |
| `load_recent_files`         | `RecentFilesService.LoadRecentFiles()`                        | none                                | `RecentFilesData`         |
| `save_recent_files`         | `RecentFilesService.SaveRecentFiles(data)`                    | `data: RecentFilesData`             | `()`                      |
| `auto_populate_steam`       | `SteamAutoPopulateService.AttemptAutoPopulate(request)`       | `request: SteamAutoPopulateRequest` | `SteamAutoPopulateResult` |
| `validate_steam_launch`     | `SteamLaunchService.Validate(request)`                        | `request: SteamLaunchRequest`       | `ValidationResult`        |
| `launch_steam_game`         | Phase 1 of `LaunchSteamModeAsync`                             | `request: SteamLaunchRequest`       | `LaunchResult`            |
| `launch_steam_trainer`      | Phase 2 of `LaunchSteamModeAsync`                             | `request: SteamLaunchRequest`       | `LaunchResult`            |
| `validate_launcher_export`  | `SteamExternalLauncherExportService.Validate(request)`        | `request: ExportRequest`            | `ValidationResult`        |
| `export_steam_launchers`    | `SteamExternalLauncherExportService.ExportLaunchers(request)` | `request: ExportRequest`            | `ExportResult`            |
| `resolve_auto_load_profile` | `MainFormStartupCoordinator.ResolveAutoLoadProfileName(...)`  | settings + options                  | `Option<String>`          |

### React State Needed

```typescript
interface AppState {
  // Profile
  currentProfile: ProfileData | null;
  profileNames: string[];
  isDirty: boolean;

  // Steam auto-populate
  autoPopulateResult: SteamAutoPopulateResult | null;
  isAutoPopulating: boolean;

  // Launch state machine
  launchPhase: LaunchPhase;
  launchLog: string[]; // streaming log lines
  helperLogPath: string | null;

  // Settings
  settings: AppSettingsData;
  recentFiles: RecentFilesData;

  // UI
  steamModeEnabled: boolean;
  selectedGamePath: string;
  selectedTrainerPath: string;
}
```

### File I/O Patterns Needed in Rust

| Operation             | C# Pattern                                                | Rust Equivalent                                                               |
| --------------------- | --------------------------------------------------------- | ----------------------------------------------------------------------------- |
| Profile CRUD          | `File.ReadAllLines` + `Key=Value` parse                   | `std::fs::read_to_string` + line iterator, or `toml::from_str` for new format |
| Settings CRUD         | Same `Key=Value` format                                   | Same approach, or TOML                                                        |
| Recent Files          | INI sections with `[Header]` + path lines                 | Custom parser or `ini` crate                                                  |
| VDF parsing           | Custom recursive descent (`ParseKeyValueContent`)         | `steam-vdf-parser` crate or port the 125-line parser                          |
| Manifest scanning     | `Directory.EnumerateFiles("appmanifest_*.acf")`           | `std::fs::read_dir` + glob filter                                             |
| Log streaming         | `FileStream` with `FileShare.ReadWrite`, poll every 500ms | `tokio::fs::File` + `BufReader` + `tokio::time::interval`                     |
| Script execution      | `Process.Start` -> `start.exe /unix /bin/bash`            | `tokio::process::Command::new("/bin/bash")` directly                          |
| File writing (export) | `File.WriteAllText` with `\r\n` -> `\n` normalization     | `std::fs::write` (Linux already uses `\n`)                                    |
| Directory creation    | `Directory.CreateDirectory`                               | `std::fs::create_dir_all`                                                     |
| Symlink resolution    | `FileSystemInfo.ResolveLinkTarget`                        | `std::fs::read_link` + `std::fs::canonicalize`                                |

## Code Conventions

### Naming Conventions to Carry Forward

- Services: `{Domain}Service` (Rust: `mod {domain}`)
- Request DTOs: `{Domain}{Operation}Request` (Rust: `struct {Domain}{Operation}Request`)
- Result types: `{Domain}{Operation}Result` (Rust: `Result<{Domain}{Operation}Output, {Domain}Error>`)
- Validation: separate `validate()` function returning typed error, called before execute
- Internal helpers: `internal static` in C# -> `pub(crate) fn` in Rust

### Error Handling Philosophy

The C# code does NOT throw for expected business errors. It returns structured result types with success/failure states. The Rust port should follow the same pattern using `Result<T, E>` with domain-specific error enums rather than panicking or using `anyhow` for business logic errors.

Exception: `ProfileService.LoadProfile` and `DeleteProfile` throw `FileNotFoundException` when the profile does not exist. The Rust port should return `Err(ProfileError::NotFound(name))`.

### Diagnostic Accumulation

`SteamAutoPopulateService` threads `List<string> diagnostics` and `List<string> manualHints` through every helper method. Both are deduplicated before returning. The Rust equivalent should use `Vec<String>` passed by mutable reference or a `DiagnosticCollector` struct.

## Dependencies and Services

### Steam Root Discovery Order

1. `STEAM_COMPAT_CLIENT_INSTALL_PATH` environment variable (preferred)
2. `$HOME/.steam/root` (default Steam root symlink)
3. `$HOME/.local/share/Steam` (default XDG install location)

### Proton Discovery Locations

1. `<steam_root>/steamapps/common/` (official Proton installs, e.g., `Proton 9.0-4`)
2. `<steam_root>/compatibilitytools.d/` (custom Proton installs, e.g., GE-Proton)
3. System-wide locations:
   - `/usr/share/steam/compatibilitytools.d`
   - `/usr/share/steam/compatibilitytools`
   - `/usr/local/share/steam/compatibilitytools.d`
   - `/usr/local/share/steam/compatibilitytools`

### Proton Matching Algorithm

1. Parse `config/config.vdf` and each `userdata/<id>/config/localconfig.vdf` for `CompatToolMapping`
2. Look up app-specific mapping (`CompatToolMapping.<appid>.name`)
3. If no app-specific mapping, use default mapping (`CompatToolMapping.0.name`)
4. If multiple mappings found, return `Ambiguous`
5. Resolve the tool name against discovered installs:
   - Exact alias match (case-insensitive)
   - Normalized alias match (lowercase alphanumeric only)
   - Heuristic match (substring containment, version number extraction for "protonN" patterns)
6. If exactly one install matches, return its `proton` executable path
7. If multiple or zero, return `Ambiguous` or `NotFound`

### Home Path Resolution (for launcher export)

`SteamExternalLauncherExportService.ResolveTargetHomePath`:

1. Try the provided `preferredHomePath` -- use if it starts with `/` and does not contain `/compatdata/`
2. Try deriving from `steamClientInstallPath`:
   - Strip `/.local/share/Steam` suffix
   - Strip `/.steam/root` suffix
3. Fall back to the original preferred path

### Mounted Host Search Roots (for dosdevices resolution)

Search order: `/mnt`, `/media`, `/run/media`, `/var/run/media`. Scans up to 2 directory levels deep. Only used in the WINE-hosted app for resolving mapped drive paths. **Not needed in the native app.**

## Gotchas and Warnings

### Path Handling

- **dosdevices resolution is WINE-only**: The entire `ResolveDosDevicesPath`, `NormalizeSteamHostPath`, `ConvertToUnixPath`, `ConvertToWindowsPath` machinery exists because the C# app runs inside WINE where file paths may be Windows-style (`Z:\home\...`) or contain dosdevices references. The native Rust app does NOT need any of this -- it works with Linux paths natively. This is 400+ lines of code that should NOT be ported.

- **Path normalization still needed**: Even in the native app, paths from VDF files may contain backslashes or `.`/`..` segments. The `NormalizeHostStylePath` pattern (replace `\` with `/`, resolve `.`/`..`, collapse repeated `/`) should be ported.

### VDF Parser Edge Cases

- **Case-insensitive keys**: The `SteamKeyValueNode.Children` dictionary uses `StringComparer.OrdinalIgnoreCase`. If the Rust VDF crate uses case-sensitive keys, lookups like `GetChild("AppState")` vs `GetChild("appstate")` will break manifest parsing.

- **FindDescendantByKey is recursive DFS**: It searches the entire tree, not just immediate children. This is used for finding `CompatToolMapping` which can be nested several levels deep in `config.vdf`.

- **Unquoted tokens**: The parser handles both quoted (`"value"`) and unquoted (`value`) tokens. Unquoted tokens terminate at whitespace or `{`/`}`.

### Launch Orchestration

- **Phase 1 uses steam-launch-helper.sh with `--game-only`**: The helper script still receives the trainer path but does not launch it.

- **Phase 2 uses steam-launch-trainer.sh (different script!)**: Not `steam-launch-helper.sh --trainer-only`. The trainer-only script invokes the host runner via `setsid env -i` to fully escape the calling process's environment.

- **steam-launch-helper.sh also has `--trainer-only` mode**: This is the in-script trainer launch path that does NOT use `setsid env -i`. It uses the `run_proton_with_clean_env` function which does in-process `unset` + `setsid` (but inherits the bash environment). The trainer script + host runner approach is the preferred path for Phase 2 launches from the native app because it provides fuller isolation.

- **Log file is created by the caller, not the script**: The caller creates the log file path (`/tmp/crosshook-steam-helper-logs/steam-helper-YYYYMMDD-HHmmssfff.log`), passes it via `--log-file`, and the script redirects its stdout/stderr to it.

### Auto-Populate

- **Non-failing by design**: `AttemptAutoPopulate` always returns a result, even when sub-steps fail. Parse errors for individual manifests are caught and logged as diagnostics but do not abort the scan.

- **Game path matching uses `PathIsSameOrChild`**: The game exe path must be inside the manifest's `installdir` under `steamapps/common/<installdir>`. This is a prefix match with path normalization.

- **Multiple manifest matches = Ambiguous**: If the same game path matches manifests with different App IDs, the result is `Ambiguous` (not an error). The UI should display all candidates.

- **RecentFilesService drops non-existent paths**: On load, any path where `File.Exists()` returns false is silently dropped. The native app should preserve this behavior (stale MRU entries are useless).

### Launcher Export

- **Generated scripts use single-quoted shell literals**: `ToShellSingleQuotedLiteral` replaces `'` with `'"'"'` (end single quote, double-quoted single quote, start single quote). This is the correct POSIX way to include a literal single quote in a single-quoted string.

- **Desktop entry Exec escaping**: Backslashes, spaces, and double quotes are escaped per the Desktop Entry Specification.

- **Icon fallback**: If no icon path is provided, the `.desktop` entry uses `applications-games` (a standard freedesktop icon name).

- **Home path derivation from Steam path**: The export service can derive the home path from the Steam client install path by stripping known suffixes. This logic should be preserved in the Rust port.

## Task-Specific Guidance

### Phase 1: Profile Management + Settings

Port `ProfileService`, `AppSettingsService`, `RecentFilesService`. The Rust versions should:

- Use TOML format for new profiles (via `serde + toml` crates)
- Include a legacy reader for `.profile` files (simple `Key=Value` parser)
- Use `directories` crate for XDG paths instead of hardcoded relative paths
- Expose CRUD operations as Tauri commands

### Phase 2: Steam Auto-Populate

Port `SteamAutoPopulateService` minus all WINE path conversion. The Rust version:

- Uses native Linux paths everywhere (no dosdevices, no winepath)
- VDF parsing via `steam-vdf-parser` crate (verify case-insensitive keys) or port the 125-line parser
- Manifest scanning via `std::fs::read_dir` + `appmanifest_*.acf` glob
- Proton discovery including system-wide locations
- Return same three-state (`NotFound`/`Found`/`Ambiguous`) result structure

### Phase 3: Launch Orchestration

The native app invokes the shell scripts directly via `Command::new("/bin/bash")`:

- No `start.exe /unix` bridge needed
- No environment variable cleanup in Rust (scripts handle their own cleanup)
- Build argument list matching the CLI interface documented above
- Stream log output using Tauri events or IPC channels
- Implement the two-phase state machine in React

### Phase 4: Launcher Export

Port `SteamExternalLauncherExportService`:

- Generate `.sh` trainer scripts (same format as C# `BuildTrainerScriptContent`)
- Generate `.desktop` entries (same format as C# `BuildDesktopEntryContent`)
- Write to `~/.local/share/crosshook/launchers/` and `~/.local/share/applications/`
- Slug generation: lowercase, replace non-alphanumeric with `-`, collapse consecutive `-`

## Relevant Files

- `src/CrossHookEngine.App/Services/SteamAutoPopulateService.cs`: Most complex service; VDF parser at lines 739-863, auto-populate pipeline at lines 49-162, Proton resolution at lines 371-429, internal data types at lines 1180-1285
- `src/CrossHookEngine.App/Services/SteamLaunchService.cs`: Launch validation, CLI argument construction (lines 139-196), env var cleanup list (lines 233-269), path conversion (lines 271-418)
- `src/CrossHookEngine.App/Services/SteamExternalLauncherExportService.cs`: Launcher export with script/desktop generation (lines 232-265), home path resolution (lines 143-158, 279-304)
- `src/CrossHookEngine.App/Services/ProfileService.cs`: Profile data model (lines 199-224), CRUD operations, profile name validation (lines 165-196)
- `src/CrossHookEngine.App/Services/AppSettingsService.cs`: Two-field settings with Key=Value format
- `src/CrossHookEngine.App/Services/RecentFilesService.cs`: INI-style sections with path-existence filtering on load
- `src/CrossHookEngine.App/Services/CommandLineParser.cs`: CLI args `-p` (repeatable) and `-autolaunch` (consumes rest)
- `src/CrossHookEngine.App/runtime-helpers/steam-launch-helper.sh`: Full game+trainer orchestrator, reused directly
- `src/CrossHookEngine.App/runtime-helpers/steam-launch-trainer.sh`: Trainer launcher via `setsid env -i`, reused directly
- `src/CrossHookEngine.App/runtime-helpers/steam-host-trainer-runner.sh`: Clean-env runner, reused directly
- `src/CrossHookEngine.App/Forms/MainForm.cs`: Lines 2648-2662 (BuildSteamLaunchRequest), 2767-2828 (LaunchSteamModeAsync), 2830-2898 (RunSteamLaunchHelper), 2908-2946 (StreamSteamHelperLogAsync)
- `src/CrossHookEngine.App/Forms/MainFormStartupCoordinator.cs`: Auto-load profile resolution
- `src/CrossHookEngine.App/Diagnostics/AppDiagnostics.cs`: Trace-based logging pattern
- `src/CrossHookEngine.App/Program.cs`: Single-instance Mutex pattern, exception handling
