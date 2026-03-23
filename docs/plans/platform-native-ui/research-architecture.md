# Architecture Research: platform-native-ui

## System Overview

CrossHook's existing codebase is a monolithic C#/WinForms application (`src/CrossHookEngine.App/`) targeting `net9.0-windows` that runs under WINE/Proton on Linux. The application is organized into six namespace-based layers: Core (process lifecycle), Injection (DLL injection via CreateRemoteThread), Memory (process memory read/write), Services (profiles, Steam discovery, launch orchestration, settings), Forms (WinForms UI), and UI (overlay components). For the native UI, the most valuable porting targets are the **Services layer** -- specifically `SteamAutoPopulateService`, `SteamLaunchService`, `SteamExternalLauncherExportService`, and `ProfileService` -- and the three **runtime-helper shell scripts** which already run natively on Linux and handle the actual game+trainer launch pipeline. The Core/Injection/Memory layers are Win32 P/Invoke-heavy and should NOT be ported for the MVP (the native app delegates trainer execution to `proton run` via the shell scripts instead).

## Relevant Components

### Services (Port Priority: HIGH -- these contain the domain logic)

- `src/CrossHookEngine.App/Services/SteamAutoPopulateService.cs` (1,285 lines): Steam library discovery, VDF/ACF parsing, manifest matching, Proton version resolution. Contains a full VDF key-value parser (`ParseKeyValueContent`/`SteamKeyValueNode`), library enumeration from `libraryfolders.vdf`, app manifest matching, compat tool resolution from `config.vdf`/`localconfig.vdf`, and custom Proton detection from `compatibilitytools.d/`. This is the single most complex service to port.
- `src/CrossHookEngine.App/Services/SteamLaunchService.cs` (737 lines): Validates launch requests, constructs CLI arguments for shell scripts, manages Windows-to-Unix path conversion, cleans WINE environment variables (~30 vars), creates `ProcessStartInfo` for shell script invocation via `start.exe /unix /bin/bash`. Contains the canonical list of environment variables to strip.
- `src/CrossHookEngine.App/Services/SteamExternalLauncherExportService.cs` (350 lines): Generates standalone `.sh` trainer launcher scripts and `.desktop` entry files. Writes files to `~/.local/share/crosshook/launchers/` and `~/.local/share/applications/`.
- `src/CrossHookEngine.App/Services/ProfileService.cs` (225 lines): CRUD for `.profile` files using a flat `Key=Value` format with 12 fields. Profiles are stored in a `Profiles/` directory relative to the app startup path.
- `src/CrossHookEngine.App/Services/AppSettingsService.cs` (81 lines): Persists `AutoLoadLastProfile` and `LastUsedProfile` in `Settings/AppSettings.ini` using the same `Key=Value` format.
- `src/CrossHookEngine.App/Services/RecentFilesService.cs` (131 lines): Tracks recently used game, trainer, and DLL paths in `settings.ini` using an INI-like `[Section]` format.
- `src/CrossHookEngine.App/Services/CommandLineParser.cs` (54 lines): Parses `-p <profile>` and `-autolaunch <path>` CLI arguments.

### Runtime-Helper Shell Scripts (Reuse Priority: DIRECT -- no porting needed)

- `src/CrossHookEngine.App/runtime-helpers/steam-launch-helper.sh` (326 lines): The primary orchestration script. Accepts `--appid`, `--compatdata`, `--proton`, `--steam-client`, `--game-exe-name`, `--trainer-path`, `--trainer-host-path`, `--log-file`, plus timeout and mode flags (`--trainer-only`, `--game-only`). Launches game via `steam -applaunch`, waits for game process via `pgrep -af`, stages trainer into compatdata, strips all WINE env vars, then runs `proton run <trainer>` with `setsid`.
- `src/CrossHookEngine.App/runtime-helpers/steam-launch-trainer.sh` (128 lines): Trainer-only launcher that delegates to `steam-host-trainer-runner.sh` via `setsid env -i` for a fully clean environment. Launched by SteamLaunchService when `LaunchTrainerOnly` is true.
- `src/CrossHookEngine.App/runtime-helpers/steam-host-trainer-runner.sh` (178 lines): The actual trainer execution script. Strips all WINE/Proton env vars, stages trainer into compatdata prefix at `pfx/drive_c/CrossHook/StagedTrainers/`, runs `proton run <trainer>`. Designed to run completely outside any WINE session.

### Core/Injection/Memory (Port Priority: NONE for MVP)

- `src/CrossHookEngine.App/Core/ProcessManager.cs` (863 lines): Win32 process lifecycle management via P/Invoke (`CreateProcess`, `OpenProcess`, `SuspendThread`, `ResumeThread`, minidump). Entirely Win32-dependent. The native app should use `std::process::Command` and Linux signals instead.
- `src/CrossHookEngine.App/Injection/InjectionManager.cs` (443 lines): DLL injection via `LoadLibraryA` + `CreateRemoteThread`. Not needed for the native app's primary workflow (which uses `proton run`).
- `src/CrossHookEngine.App/Memory/MemoryManager.cs` (397 lines): Process memory read/write via `ReadProcessMemory`/`WriteProcessMemory`. Future native equivalent would use `/proc/<pid>/mem`.
- `src/CrossHookEngine.App/Interop/Kernel32Interop.cs` (31 lines): Shared Win32 P/Invoke declarations (`OpenProcess`, `CloseHandle`, `CreateRemoteThread`, `WriteProcessMemory`, `VirtualAllocEx`, `VirtualFreeEx`).
- `src/CrossHookEngine.App/Interop/Win32ErrorHelper.cs` (17 lines): Formats Win32 error codes into messages.

### Forms/UI (Port Priority: NONE -- will be replaced entirely)

- `src/CrossHookEngine.App/Forms/MainForm.cs` (3,602 lines): Monolithic WinForms form containing all UI controls, event handlers, and integration glue. Acts as the central orchestrator connecting Services, Core, Injection, and Memory. Key integration methods documented below under Data Flow.
- `src/CrossHookEngine.App/Forms/MainForm.Designer.cs`: WinForms designer-generated code.
- `src/CrossHookEngine.App/Forms/MainFormStartupCoordinator.cs` (35 lines): Resolves which profile to auto-load at startup based on settings and CLI args.
- `src/CrossHookEngine.App/UI/ResumePanel.cs` (135 lines): Custom WinForms overlay panel for pause/resume UI.

### Infrastructure

- `src/CrossHookEngine.App/Diagnostics/AppDiagnostics.cs` (127 lines): Trace-based logging to `%LOCALAPPDATA%/CrossHookEngine/logs/crosshook.log`.
- `src/CrossHookEngine.App/Program.cs` (78 lines): Entry point with single-instance enforcement via named `Mutex`, global exception handling.
- `src/CrossHookEngine.App/CrossHookEngine.App.csproj`: SDK-style project targeting `net9.0-windows`, `WinExe` output, `AllowUnsafeBlocks` enabled. Runtime-helper scripts are copied to output via `<None Include="runtime-helpers\**\*">`.
- `scripts/publish-dist.sh`: Publish script that produces `win-x64` and `win-x86` self-contained artifacts.

## Data Flow

### Profile Load/Save Flow

1. **Load**: User selects profile name from `cmbProfiles` dropdown -> `MainForm.LoadProfile(profileName)` -> `ProfileService.LoadProfile(profileName)` reads `Profiles/<name>.profile` line by line, splits on `=`, populates `ProfileData` with 12 fields -> MainForm sets all UI controls from `ProfileData` fields, including `_selectedGamePath`, `_selectedTrainerPath`, Steam fields, launch method.
2. **Save**: User clicks Save -> `ProfileInputDialog` prompts for name -> `MainForm.SaveProfile(profileName)` reads current UI state into a new `ProfileData` -> `ProfileService.SaveProfile(profileName, profile)` writes 12 `Key=Value` lines to disk.
3. **Profile Data Model** (exact 12 fields): `GamePath`, `TrainerPath`, `Dll1Path`, `Dll2Path`, `LaunchInject1` (bool), `LaunchInject2` (bool), `LaunchMethod` (enum string), `UseSteamMode` (bool), `SteamAppId`, `SteamCompatDataPath`, `SteamProtonPath`, `SteamLauncherIconPath`.
4. **Profile Storage**: Currently stored relative to `Application.StartupPath` (the WINE prefix app directory). The native app should use `~/.config/crosshook/profiles/` per XDG conventions.

### Steam Auto-Populate Flow (Game Path -> App ID -> Compatdata -> Proton)

1. User clicks "Auto Populate" button -> `BtnAttemptSteamAutoPopulate_Click` -> builds `SteamAutoPopulateRequest` with current `GamePath` and `SteamClientInstallPath` (from `STEAM_COMPAT_CLIENT_INSTALL_PATH` env var or `~/.steam/root`).
2. `SteamAutoPopulateService.AttemptAutoPopulate(request)` runs on a background thread:
   a. **Normalize game path**: Converts Windows paths to Unix via `NormalizePathForHostLookup()` which calls `SteamLaunchService.NormalizeSteamHostPath()` (handles `Z:\` drive prefix, `dosdevices/` symlinks, mounted drive scanning).
   b. **Discover Steam roots**: Checks configured path, then `~/.steam/root` and `~/.local/share/Steam`.
   c. **Discover Steam libraries**: Parses `steamapps/libraryfolders.vdf` from each root using the custom VDF parser (`ParseKeyValueContent` -> `SteamKeyValueNode` tree). Each library entry can have a `path` child or a direct string value.
   d. **Match game to manifest**: For each library, enumerates `steamapps/appmanifest_*.acf` files, parses `AppState.appid` and `AppState.installdir`, checks if the game path falls within `steamapps/common/<installdir>/`. Returns `Found`, `NotFound`, or `Ambiguous` (multiple manifests matched).
   e. **Derive compatdata path**: Constructs `<library>/steamapps/compatdata/<appid>`, checks `Directory.Exists`.
   f. **Resolve Proton path**: Collects compat tool mappings from `config/config.vdf` and `userdata/*/config/localconfig.vdf` (looks for `CompatToolMapping.<appid>.name`). Discovers installed tools from `steamapps/common/*/proton` (official) and `compatibilitytools.d/*/proton` (custom, including GE-Proton). Also checks system roots (`/usr/share/steam/compatibilitytools.d/`). Matches by alias (directory name, `compatibilitytool.vdf` display_name, normalized fuzzy match). Returns `Found`, `NotFound`, or `Ambiguous`.
3. Result applied to UI: `ApplySteamAutoPopulateResult` sets `txtSteamAppId`, `txtSteamCompatDataPath`, `txtSteamProtonPath` text fields. Diagnostics and manual hints logged to console.

### Steam Launch Flow (User -> Scripts -> Game + Trainer)

1. User clicks "Launch" with Steam mode enabled -> `BtnLaunch_Click` -> `LaunchSteamModeAsync()`.
2. **First launch (game + trainer)**: Builds `SteamLaunchRequest` with `LaunchGameOnly=true` (actually `!_steamTrainerLaunchPending`). Validates all required fields. Resolves helper script path via `SteamLaunchService.ResolveHelperScriptPath`. Converts all paths to Unix format via `SteamLaunchService.ConvertToUnixPath` (uses `winepath.exe -u`).
3. Creates `ProcessStartInfo` targeting `start.exe /unix /bin/bash <script>` with all arguments as CLI flags. Strips ~30 WINE/Proton environment variables and sets only `STEAM_COMPAT_DATA_PATH`, `STEAM_COMPAT_CLIENT_INSTALL_PATH`, `WINEPREFIX`.
4. Script execution: `steam-launch-helper.sh` launches game via `steam -applaunch <appid>`, waits for game process via `pgrep -af`, stages trainer into `compatdata/pfx/drive_c/CrossHook/StagedTrainers/`, strips all WINE env vars, runs `proton run <trainer_windows_path>` via `setsid`.
5. After first launch: `_steamTrainerLaunchPending = true`, UI transitions to "Launch Trainer" state. Next click sends `LaunchTrainerOnly=true`, which invokes `steam-launch-trainer.sh` instead (trainer-only flow).
6. Helper log is streamed to UI console via `StreamSteamHelperLogAsync` polling the log file.

### Launcher Export Flow

1. User clicks "Export Steam Launchers" -> `BtnExportSteamLaunchers_Click` -> builds `SteamExternalLauncherExportRequest`.
2. `SteamExternalLauncherExportService.ExportLaunchers` validates, generates a launcher slug from display name.
3. Writes standalone bash script to `~/.local/share/crosshook/launchers/<slug>-trainer.sh` (sets env vars, runs `exec "$PROTON" run "$TRAINER_WINDOWS_PATH"`).
4. Writes `.desktop` entry to `~/.local/share/applications/crosshook-<slug>-trainer.desktop`.

### Native App Equivalent Data Flow

For the Tauri/Rust native app, the data flows simplify significantly because there is no WINE boundary:

- Path conversion (`ConvertToUnixPath`, `ConvertToWindowsPath`, `NormalizeSteamHostPath`, `dosdevices` resolution) is **not needed** -- the native app works with Linux paths directly.
- Script invocation uses `std::process::Command` directly instead of `start.exe /unix /bin/bash`.
- The VDF parsing logic from `SteamAutoPopulateService` must be ported to Rust (or replaced by the `steam-vdf-parser` crate).
- Profile reading/writing can be implemented natively for both legacy `.profile` format and new TOML format.
- Environment variable stripping is still required when invoking `proton run` but is simpler since there is no inherited WINE session.

## Integration Points

### Shell Scripts: Direct Reuse

The three shell scripts can be bundled with the native app and invoked directly via `std::process::Command`:

| Script                         | Native Invocation                                        | Notes                                           |
| ------------------------------ | -------------------------------------------------------- | ----------------------------------------------- |
| `steam-launch-helper.sh`       | `Command::new("/bin/bash").arg(script_path).args([...])` | Same CLI args. No `start.exe` bridge needed.    |
| `steam-launch-trainer.sh`      | `Command::new("/bin/bash").arg(script_path).args([...])` | Trainer-only mode.                              |
| `steam-host-trainer-runner.sh` | Not invoked directly by the app                          | Called by `steam-launch-trainer.sh` internally. |

The native app should pass the same CLI arguments as `SteamLaunchService.CreateHelperStartInfo` and `CreateTrainerStartInfo`, minus the `start.exe /unix` prefix. Environment cleanup is still needed but is simpler (no inherited WINE session to strip from).

### Profile Format Compatibility

The native app must support reading and writing the existing `.profile` format for cross-compatibility:

```
GamePath=<path>
TrainerPath=<path>
Dll1Path=<path>
Dll2Path=<path>
LaunchInject1=<True|False>
LaunchInject2=<True|False>
LaunchMethod=<CreateProcess|CmdStart|...>
UseSteamMode=<True|False>
SteamAppId=<string>
SteamCompatDataPath=<path>
SteamProtonPath=<path>
SteamLauncherIconPath=<path>
```

New TOML profiles should map these same fields with additional nesting.

### Configuration Files Shared Between Old and New Apps

| File               | Old Location                          | New Location                                        | Format                            |
| ------------------ | ------------------------------------- | --------------------------------------------------- | --------------------------------- |
| Game profiles      | `<AppDir>/Profiles/*.profile`         | `~/.config/crosshook/profiles/*.toml` + `*.profile` | Key=Value (legacy), TOML (native) |
| App settings       | `<AppDir>/Settings/AppSettings.ini`   | `~/.config/crosshook/settings.toml`                 | Key=Value (legacy), TOML (native) |
| Recent files       | `<AppDir>/settings.ini`               | `~/.config/crosshook/recent.toml`                   | INI-like (legacy), TOML (native)  |
| Helper logs        | `%TEMP%/crosshook-steam-helper-logs/` | `~/.local/share/crosshook/logs/`                    | Plain text                        |
| Exported launchers | `~/.local/share/crosshook/launchers/` | Same                                                | Shell scripts                     |
| Desktop entries    | `~/.local/share/applications/`        | Same                                                | .desktop format                   |

### VDF Parsing (Must Be Ported or Replaced)

The custom VDF parser in `SteamAutoPopulateService` handles:

- Quoted string tokens with escape sequences (`\\`, `\"`, `\n`, `\r`, `\t`)
- Nested `{ }` blocks
- `//` line comments
- Unquoted tokens

The `steam-vdf-parser` Rust crate (recommended in the feature spec) should handle this. The following VDF files are parsed:

- `steamapps/libraryfolders.vdf` -- Steam library paths
- `steamapps/appmanifest_*.acf` -- Per-game metadata (`appid`, `installdir`)
- `config/config.vdf` -- Global compat tool mappings
- `userdata/*/config/localconfig.vdf` -- Per-user compat tool mappings
- `compatibilitytools.d/*/compatibilitytool.vdf` -- Custom Proton tool metadata

### Steam Root Discovery Paths (Must Be Replicated)

The auto-populate service checks these paths in order:

1. `STEAM_COMPAT_CLIENT_INSTALL_PATH` environment variable
2. `$HOME/.steam/root` (symlink to Steam install)
3. `$HOME/.local/share/Steam` (default install path)
4. **Not yet handled**: `~/.var/app/com.valvesoftware.Steam/data/Steam` (Flatpak Steam -- noted as missing in feature spec)

System compat tool roots:

- `/usr/share/steam/compatibilitytools.d`
- `/usr/share/steam/compatibilitytools`
- `/usr/local/share/steam/compatibilitytools.d`
- `/usr/local/share/steam/compatibilitytools`

Mounted drive search roots (for dosdevices resolution, less relevant for native):

- `/mnt`, `/media`, `/run/media`, `/var/run/media`

## Key Dependencies

### External Libraries (Current C# App)

None -- the WinForms app has zero NuGet dependencies. All functionality is hand-rolled, including VDF parsing, path conversion, and process management.

### Runtime Dependencies

- **.NET 9 SDK** (pinned via local `.dotnet/` directory to avoid conflicts with system .NET 10)
- **WINE/Proton** runtime (the C# app runs inside WINE)
- **Steam client** (for `steam -applaunch` and library/manifest files)
- **Proton** executable (for `proton run <trainer>`)
- **Standard Linux utilities**: `bash`, `pgrep`, `realpath`, `basename`, `cp`, `mkdir`, `setsid`, `env`, `ps`, `wc`, `readlink`

### Internal Module Dependencies

```
MainForm (orchestrator)
  |-- ProcessManager (game process lifecycle)
  |-- InjectionManager (DLL injection) -> ProcessManager
  |-- MemoryManager (memory read/write) -> ProcessManager
  |-- ProfileService (profile CRUD)
  |-- AppSettingsService (settings persistence)
  |-- RecentFilesService (recent paths tracking)
  |-- SteamAutoPopulateService (static, discovery logic) -> SteamLaunchService (path conversion)
  |-- SteamLaunchService (static, launch request building, path conversion, env cleanup)
  |-- SteamExternalLauncherExportService (static, launcher export) -> SteamLaunchService
  |-- CommandLineParser (CLI arg parsing)
  |-- MainFormStartupCoordinator (static, auto-load logic)
  |-- ResumePanel (UI overlay)
  |-- AppDiagnostics (static, logging)
```

Services with no UI coupling (portable as-is logic):

- `SteamAutoPopulateService` (static class, pure logic + filesystem access)
- `SteamLaunchService` (static class, but path conversion uses `winepath.exe` which is WINE-specific -- native app does not need this)
- `SteamExternalLauncherExportService` (static class, filesystem operations)
- `ProfileService` (instance, filesystem CRUD)
- `AppSettingsService` (instance, filesystem CRUD)
- `RecentFilesService` (instance, filesystem CRUD)
- `CommandLineParser` (instance, pure logic)

## Architectural Patterns

- **Static service classes**: `SteamAutoPopulateService`, `SteamLaunchService`, `SteamExternalLauncherExportService` are all `static` classes with no instance state. All methods are static and testable in isolation (good for porting -- the logic maps directly to Rust module-level functions or struct methods).
- **Request/Result pattern**: Every service operation uses explicit request DTOs (`SteamLaunchRequest`, `SteamAutoPopulateRequest`, `SteamExternalLauncherExportRequest`) and result DTOs (`SteamLaunchExecutionResult`, `SteamAutoPopulateResult`). The native app should replicate this pattern with Rust structs.
- **Validate-then-execute**: `SteamLaunchService.Validate()` and `SteamExternalLauncherExportService.Validate()` are always called before execution. Validation returns structured error messages. This maps to Rust's `Result<T, E>` pattern.
- **Event-driven component communication**: `ProcessManager`, `InjectionManager`, and `MemoryManager` communicate via C# events (`EventHandler<T>`). MainForm subscribes to all events and updates UI. In Tauri, this maps to IPC events via `tauri::Manager::emit`.
- **Two-phase launch state machine**: The Steam launch flow has two states: initial (launch game + set `_steamTrainerLaunchPending = true`) and pending (launch trainer only). The UI button text and behavior change between phases. This should be modeled as an explicit state enum in the native app.
- **Monolithic form**: MainForm (3,602 lines) acts as god-object orchestrator. All service creation, event wiring, and UI updates happen here. The native app should decompose this into separate Tauri commands and React components.
- **Diagnostics/tracing**: Uses .NET `Trace` listeners writing to a log file. Maps directly to Rust `tracing` crate.

## Gotchas and Edge Cases

- **Path conversion is pervasive but unnecessary natively**: The C# app runs inside WINE, so all user-visible paths are Windows paths (`Z:\home\user\...`). `SteamLaunchService.ConvertToUnixPath` and `NormalizeSteamHostPath` exist solely to bridge the WINE boundary. The native app works with Linux paths directly, eliminating ~300 lines of path conversion code. However, the native app must still handle legacy `.profile` files that may contain Windows-style paths (e.g., `Z:\mnt\games\...`).
- **`dosdevices` symlink resolution**: `SteamLaunchService.ResolveDosDevicesPath` and `SteamAutoPopulateService.ResolveRemainingDosDevicesPath` handle WINE's `dosdevices/` directory structure where drive letters map to Linux paths via symlinks. This is only needed for legacy profile import.
- **Environment variable stripping has two sources**: The canonical list of ~30 vars to strip exists in both `SteamLaunchService.GetEnvironmentVariablesToClear()` (C# side) and `steam-launch-helper.sh`/`steam-host-trainer-runner.sh` (shell side). The shell scripts are the authoritative source since they run at execution time. The C# list is used when constructing `ProcessStartInfo` to pre-strip the environment before invoking `start.exe`. The native app should rely solely on the shell script stripping, since it launches from a clean Linux environment.
- **Two trainer launch scripts**: `steam-launch-helper.sh` does game + trainer in one script (used for first launch). `steam-launch-trainer.sh` + `steam-host-trainer-runner.sh` is a two-script chain for trainer-only launches (used for second-phase trainer launch after game is already running). The trainer script uses `setsid env -i` to create a maximally clean environment.
- **Trainer staging into compatdata**: Before `proton run`, the trainer `.exe` is copied into the game's compatdata prefix at `pfx/drive_c/CrossHook/StagedTrainers/`. This is critical -- Proton needs the trainer to be accessible within the WINE prefix. Both shell scripts implement this independently.
- **VDF parser handles both old and new libraryfolders.vdf format**: Older Steam versions used `"0" "path"` (direct string value), newer versions use `"0" { "path" "..." }` (nested object). The parser handles both via `entry.Value.Value` and `entry.Value.GetChild("path")`.
- **Proton tool matching uses fuzzy heuristics**: When exact alias matching fails, `ToolMatchesRequestedNameHeuristically` does substring matching and version-number extraction (e.g., "Proton 9.0-4" matching "proton904"). This handles Steam's inconsistent naming between config files and directory names.
- **SteamAutoPopulateService runs under WINE but accesses host filesystem**: Because the C# app runs under WINE, accessing Linux-native paths requires going through WINE's `Z:\` drive mapping. The auto-populate service normalizes all paths to host-style Unix paths before doing filesystem operations. The native app eliminates this entirely.
- **`SteamClientInstallPath` fallback chain**: Retrieved from `STEAM_COMPAT_CLIENT_INSTALL_PATH` env var first, then falls back to `$HOME/.steam/root`. In the native app, this env var will not be set (it is a WINE/Proton-specific variable), so the fallback chain must start with `$HOME/.steam/root` and `$HOME/.local/share/Steam`.
- **No test framework**: The project has no configured test framework (`/tests/` directory exists but is empty). All services use `internal` visibility with some methods exposed for testing via `internal static`. The native Rust implementation should include tests from the start.
- **Profile name validation**: `ProfileService.ValidateProfileName` checks for Windows-reserved characters, path separators, and relative path segments. The native app should replicate these validations for legacy compatibility.
- **Two-step game launch is enforced by UI state**: After the first launch call, `_steamTrainerLaunchPending` flips to `true` and the button text changes to "Launch Trainer". The next click sends `LaunchTrainerOnly=true` to `steam-launch-trainer.sh`. This state machine is critical for the user workflow and must be replicated in the native UI.

## Other Docs

- `docs/plans/platform-native-ui/feature-spec.md`: Comprehensive feature specification including architecture diagrams, data models, API traits, phasing strategy, and risk assessment
- `docs/plans/platform-native-ui/research-business.md`: User stories, business rules, domain model analysis
- `docs/plans/platform-native-ui/research-external.md`: Steam APIs, Linux process APIs, UI frameworks, VDF parsers, distribution strategies
- `docs/plans/platform-native-ui/research-technical.md`: Architecture design, Win32 to Linux API mapping, data models, Rust trait APIs, packaging
- `docs/plans/platform-native-ui/research-ux.md`: UI patterns, competitive analysis (Lutris, Heroic, Bottles, WeMod, Playnite), Steam Deck UX
- `docs/plans/platform-native-ui/research-recommendations.md`: Framework comparison, phasing strategy, risk assessment, task breakdown
- `CLAUDE.md`: Project guidelines including build commands, architecture overview, code conventions
