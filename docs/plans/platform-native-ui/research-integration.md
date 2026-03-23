# Integration Research: platform-native-ui

This document catalogs the exact APIs, data sources, file formats, and external system interactions that the native Tauri/Rust application must replicate. Every section links back to the existing C# implementation and shell scripts so that developers can trace behavior line-by-line during porting.

---

## Relevant Files

- `src/CrossHookEngine.App/Services/SteamAutoPopulateService.cs`: Steam library discovery, VDF/ACF parsing, Proton resolution (~1286 lines, most complex service)
- `src/CrossHookEngine.App/Services/SteamLaunchService.cs`: Script invocation, path conversion, environment variable cleanup, validation
- `src/CrossHookEngine.App/Services/SteamExternalLauncherExportService.cs`: `.sh` and `.desktop` file generation for standalone launchers
- `src/CrossHookEngine.App/Services/ProfileService.cs`: `.profile` key=value format read/write (12 fields)
- `src/CrossHookEngine.App/Services/AppSettingsService.cs`: App settings in `Settings/AppSettings.ini` (2 fields)
- `src/CrossHookEngine.App/Services/RecentFilesService.cs`: Recent file paths in INI-style `settings.ini` with sections
- `src/CrossHookEngine.App/Services/CommandLineParser.cs`: CLI argument parsing (`-p`, `-autolaunch`)
- `src/CrossHookEngine.App/Forms/MainForm.cs`: UI orchestration, two-phase Steam launch state machine, log streaming
- `src/CrossHookEngine.App/Forms/MainFormStartupCoordinator.cs`: Auto-load profile resolution at startup
- `src/CrossHookEngine.App/runtime-helpers/steam-launch-helper.sh`: Full game+trainer launch orchestrator (game via `steam -applaunch`, trainer via `proton run`)
- `src/CrossHookEngine.App/runtime-helpers/steam-host-trainer-runner.sh`: Standalone trainer runner with clean environment (used by `steam-launch-trainer.sh`)
- `src/CrossHookEngine.App/runtime-helpers/steam-launch-trainer.sh`: Trainer-only launcher that spawns `steam-host-trainer-runner.sh` in a detached session
- `tests/CrossHookEngine.App.Tests/SteamAutoPopulateServiceTests.cs`: Contains VDF/ACF format examples used in tests (essential for format reference)
- `tests/CrossHookEngine.App.Tests/ProfileServiceTests.cs`: Exact `.profile` format verification
- `tests/CrossHookEngine.App.Tests/SteamExternalLauncherExportServiceTests.cs`: Exact `.sh` and `.desktop` output verification

---

## Steam Integration

### Library Discovery

Steam libraries are discovered through a multi-step process implemented in `SteamAutoPopulateService.DiscoverSteamRootCandidates()` and `DiscoverSteamLibraries()`.

**Step 1: Locate Steam root candidates**

The service tries these locations in order:

1. User-provided `SteamClientInstallPath` (from env var `STEAM_COMPAT_CLIENT_INSTALL_PATH` or profile)
2. `$HOME/.steam/root` (symlink to real install, standard on most distros)
3. `$HOME/.local/share/Steam` (direct install path)

The `GetSteamClientInstallPath()` in `MainForm.cs` (line 2682) resolves the Steam client install path with this priority:

1. `STEAM_COMPAT_CLIENT_INSTALL_PATH` environment variable
2. Fallback: `$HOME/.steam/root`

**Not currently handled** (noted in feature-spec edge cases):

- Flatpak Steam: `~/.var/app/com.valvesoftware.Steam/data/Steam`

**Step 2: Parse libraryfolders.vdf**

Located at `<steam_root>/steamapps/libraryfolders.vdf`. Parsed using the custom VDF parser (see VDF Format section below).

The parser handles two VDF formats for library entries:

Format A (newer) -- each entry has a `path` child node:

```vdf
"libraryfolders"
{
    "0"
    {
        "path" "/mnt/sdx1/SteamLibrary"
    }
}
```

Format B (legacy) -- each entry's value IS the path:

```vdf
"libraryfolders"
{
    "0" "/home/deck/.local/share/Steam"
}
```

The code at line 219-228 checks both: if the entry's `Value` is non-empty, use it directly; otherwise look for a `path` child node.

**Step 3: Validate library directories**

Each library candidate is validated by checking that `<library_path>/steamapps/` exists as a directory. Libraries are deduplicated by path.

### App Manifest Parsing

Game manifests are `appmanifest_<appid>.acf` files in `<library>/steamapps/`. They use the same VDF text format.

**Key fields extracted** (from `FindGameMatch()`, line 241):

- `AppState.appid`: The Steam App ID (string)
- `AppState.installdir`: Directory name under `steamapps/common/` (NOT a full path)

**Matching algorithm**:

1. Enumerate all `appmanifest_*.acf` files in each library's `steamapps/` directory
2. Parse each manifest to get `appid` and `installdir`
3. Construct the install path: `<library>/steamapps/common/<installdir>`
4. Check if the user-selected game executable path is the same as, or a child of, the install path
5. If exactly one manifest matches, return `Found`; if multiple match, return `Ambiguous`

**Fallback App ID extraction**: If `appid` is missing from the manifest content, the service falls back to extracting it from the filename (`appmanifest_287700.acf` -> `287700`).

**Compatdata path derivation**: Once an App ID is matched, compatdata path is constructed as `<library>/steamapps/compatdata/<appid>`. Existence is verified with `Directory.Exists()`.

### Proton Resolution

Proton resolution happens in `ResolveProtonPath()` (line 371) through two sub-steps:

**Step 1: Collect compat tool mappings from config files**

`CollectCompatToolMappings()` (line 431) searches these VDF config files:

- `<steam_root>/config/config.vdf` -- global config
- `<steam_root>/userdata/<userid>/config/localconfig.vdf` -- per-user config (multiple users possible)

It performs a recursive search for the `CompatToolMapping` key anywhere in the VDF tree. The mapping structure is:

```vdf
"CompatToolMapping"
{
    "287700"
    {
        "name" "proton_9"
    }
    "0"
    {
        "name" "proton_8"
    }
}
```

- App ID `"0"` is the global default Proton
- App-specific entries (e.g., `"287700"`) override the default
- The `name` child's value is the compat tool internal name (not the display name)

**Step 2: Discover installed Proton versions**

`DiscoverCompatTools()` (line 498) searches for `proton` executables in these directories:

- `<steam_root>/steamapps/common/*/proton` -- official Proton installs (e.g., `Proton 9.0-4/proton`)
- `<steam_root>/compatibilitytools.d/*/proton` -- custom tools (e.g., `GE-Proton9-20/proton`)
- System compat tool roots (line 1107):
  - `/usr/share/steam/compatibilitytools.d`
  - `/usr/share/steam/compatibilitytools`
  - `/usr/local/share/steam/compatibilitytools.d`
  - `/usr/local/share/steam/compatibilitytools`

**Alias resolution**: Each tool directory is checked for `compatibilitytool.vdf` which defines aliases:

```vdf
"compatibilitytools"
{
    "compat_tools"
    {
        "proton_9"
        {
            "display_name" "Proton 9.0"
        }
    }
}
```

Aliases collected per tool include:

1. The directory name (e.g., `Proton 9.0-4`)
2. The `compat_tools` entry key (e.g., `proton_9`)
3. The `display_name` value (e.g., `Proton 9.0`)

**Matching logic** (`ResolveCompatToolByName()`, line 545):

1. Exact alias match (case-insensitive)
2. Normalized alias match (strip all non-alphanumeric, lowercase)
3. Heuristic match: substring containment or version-number extraction for `proton*` names

### Steam Game Launch

The game is launched via `steam -applaunch <appid>`. The script (`steam-launch-helper.sh`) resolves the `steam` command with:

1. `command -v steam` -- check if `steam` is in PATH
2. Fallback: `$steam_client/steam.sh` -- use the Steam install directory's script

The launch command runs in the background: `"$steam_command" -applaunch "$appid" >/dev/null 2>&1 &`

### Two-Phase Launch State Machine

The `MainForm` manages a boolean `_steamTrainerLaunchPending` that controls the two-step flow:

1. **Phase 1 (Game)**: `_steamTrainerLaunchPending = false`. Button shows "Launch Game". Invokes `steam-launch-helper.sh` with `--game-only`. On success, sets `_steamTrainerLaunchPending = true`, minimizes the app.
2. **Phase 2 (Trainer)**: `_steamTrainerLaunchPending = true`. Button shows "Launch Trainer". Invokes `steam-launch-trainer.sh` (or `steam-launch-helper.sh --trainer-only`).

The script uses `pgrep -af` to detect running processes by executable name, with fallback to name-without-extension (e.g., `pgrep -af "mgsvtpp"` if `pgrep -af "mgsvtpp.exe"` fails).

---

## File Formats

### Profile Format (`.profile`)

Plain-text key=value pairs, one per line. No quoting, no escaping, no comments. The `=` delimiter splits on the first `=` only (values can contain `=`).

**Exact field list** (12 fields, fixed order on write):

| Field                   | Type   | Default | Notes                                                                                                                 |
| ----------------------- | ------ | ------- | --------------------------------------------------------------------------------------------------------------------- |
| `GamePath`              | string | `""`    | Windows or Unix path to game executable                                                                               |
| `TrainerPath`           | string | `""`    | Windows or Unix path to trainer executable                                                                            |
| `Dll1Path`              | string | `""`    | Path to first DLL for injection                                                                                       |
| `Dll2Path`              | string | `""`    | Path to second DLL for injection                                                                                      |
| `LaunchInject1`         | bool   | `false` | `True`/`False` (C# `bool.TryParse` format)                                                                            |
| `LaunchInject2`         | bool   | `false` | `True`/`False`                                                                                                        |
| `LaunchMethod`          | string | `""`    | One of: `CreateProcess`, `CmdStart`, `CreateThreadInjection`, `RemoteThreadInjection`, `ShellExecute`, `ProcessStart` |
| `UseSteamMode`          | bool   | `false` | `True`/`False`                                                                                                        |
| `SteamAppId`            | string | `""`    | e.g., `287700`                                                                                                        |
| `SteamCompatDataPath`   | string | `""`    | Unix path to compatdata directory                                                                                     |
| `SteamProtonPath`       | string | `""`    | Unix path to `proton` executable                                                                                      |
| `SteamLauncherIconPath` | string | `""`    | Unix path to icon image (.png/.jpg)                                                                                   |

**Example file** (from test):

```
GamePath=/games/hades.exe
TrainerPath=/trainers/hades=godmode.exe
Dll1Path=/mods/first.dll
Dll2Path=/mods/second.dll
LaunchInject1=True
LaunchInject2=False
LaunchMethod=CmdStart
UseSteamMode=True
SteamAppId=287700
SteamCompatDataPath=/mnt/sdb/SteamLibrary/steamapps/compatdata/287700
SteamProtonPath=/usr/share/steam/compatibilitytools.d/proton-cachyos-slr/proton
SteamLauncherIconPath=/home/yandy/Pictures/mgs-tpp-icon.png
```

**Parsing rules**:

- Lines without `=` are silently skipped
- Unknown keys are silently ignored
- Missing fields retain their default values
- Boolean parsing uses `bool.TryParse` (case-insensitive: `true`, `True`, `TRUE` all work; invalid values leave the default)

**File naming**: `<profilename>.profile` stored in a `Profiles/` subdirectory relative to the app's startup path. Profile names are validated to exclude path separators and reserved characters.

### VDF/ACF Format

Steam's Valve Data Format is a text-based key-value format. The existing code implements a **custom recursive-descent parser** in `SteamAutoPopulateService.ParseKeyValueContent()` (line 587).

**Format specification** (as implemented):

```
<file>     ::= <keyvalue>*
<keyvalue> ::= <token> ( '{' <keyvalue>* '}' | <token> )
<token>    ::= '"' <escaped-string> '"' | <bare-word>
<comment>  ::= '//' <text-to-eol>
```

**Parser behavior**:

- Tokens are either double-quoted strings or bare words (terminated by whitespace, `{`, or `}`)
- Escape sequences in quoted strings: `\\`, `\"`, `\n`, `\r`, `\t` (unknown escapes pass through the character after backslash)
- `//` comments are skipped to end-of-line
- Keys are case-insensitive in the `SteamKeyValueNode.Children` dictionary (`StringComparer.OrdinalIgnoreCase`)
- If a key is followed by `{`, it opens a child object; otherwise, the next token is its string value

**Data structure** (`SteamKeyValueNode`):

- `Value: string` -- leaf value (empty string for container nodes)
- `Children: Dictionary<string, SteamKeyValueNode>` -- child nodes (case-insensitive keys)
- `GetChild(key)` -- lookup child by key

**ACF files** use the same format. Example `appmanifest_287700.acf`:

```vdf
"AppState"
{
    "appid" "287700"
    "installdir" "MGS_TPP"
}
```

**The `FindDescendantByKey()` utility** (line 865) performs recursive depth-first search for a key anywhere in the tree. This is used for `CompatToolMapping` which is deeply nested.

### AppSettings Format (`Settings/AppSettings.ini`)

Same key=value format as `.profile`, no sections.

**Fields** (2):

| Field                 | Type   | Default |
| --------------------- | ------ | ------- |
| `AutoLoadLastProfile` | bool   | `false` |
| `LastUsedProfile`     | string | `""`    |

### Recent Files Format (`settings.ini`)

INI-style with sections. Each section contains raw file paths, one per line. Lines starting with `;` are comments. Empty lines are skipped. Non-existent paths are filtered out on load.

**Sections**:

- `[RecentGamePaths]`
- `[RecentTrainerPaths]`
- `[RecentDllPaths]`

**Example**:

```ini
[RecentGamePaths]
/mnt/sdb/SteamLibrary/steamapps/common/MGS_TPP/mgsvtpp.exe

[RecentTrainerPaths]
/home/user/trainers/mgs-tpp-fling.exe

[RecentDllPaths]
```

---

## External Services

### Shell Script Invocation

The WinForms app launches scripts through WINE's `start.exe /unix` bridge because the C# process runs inside WINE. In the native Rust app, scripts can be invoked directly via `std::process::Command`.

**Script: `steam-launch-helper.sh`** (full game+trainer orchestrator)

Arguments:

```
--appid <steam_app_id>
--compatdata <unix_path>
--proton <unix_path_to_proton_executable>
--steam-client <unix_path_to_steam_install>
--game-exe-name <filename_only>
--trainer-path <windows_path_to_trainer>
--trainer-host-path <unix_path_to_trainer_source>
--log-file <unix_path_to_log_output>
--game-startup-delay-seconds <int> (default: 30)
--game-timeout-seconds <int> (default: 90)
--trainer-timeout-seconds <int> (default: 10)
--trainer-only (flag, no value)
--game-only (flag, no value)
```

Key behaviors:

- Validates all required arguments, checks `compatdata` dir exists, `proton` is executable, `trainer_host_path` is a file
- Redirects all output to `$log_file` via `exec >>"$log_file" 2>&1`
- Resolves all paths via `realpath`
- Stages trainer: copies `$trainer_host_path` into `$compatdata/pfx/drive_c/CrossHook/StagedTrainers/<filename>`
- Sets staged Windows path to `C:\CrossHook\StagedTrainers\<filename>`
- Launches game: `"$steam_command" -applaunch "$appid" >/dev/null 2>&1 &`
- Waits for game process via `pgrep -af -- "$game_exe_name"` (with and without `.exe` extension)
- Closes inherited file descriptors > 2 (from WINE session) before launching trainer
- Strips all WINE/Proton env vars (28 variables), then re-exports only 3
- Launches trainer: `setsid "$proton" run "$trainer_path"` (using the staged Windows path)

**Script: `steam-launch-trainer.sh`** (trainer-only, used for the "Launch Trainer" phase)

Arguments: same as `steam-host-trainer-runner.sh` (no `--appid`, `--game-exe-name`, or timing args).

Key behavior:

- Resolves co-located `steam-host-trainer-runner.sh` via `BASH_SOURCE[0]`
- Uses `setsid env -i` to launch the runner in a completely clean environment
- Only passes through: `HOME`, `USER`, `LOGNAME`, `SHELL`, `PATH`, `DISPLAY`, `WAYLAND_DISPLAY`, `XDG_RUNTIME_DIR`, `DBUS_SESSION_BUS_ADDRESS`
- Runner launches as background process, PID captured and logged

**Script: `steam-host-trainer-runner.sh`** (actual trainer execution)

The core trainer runner that:

- Closes inherited file descriptors > 2
- Unsets all 28 WINE/Proton environment variables
- Exports clean env: `STEAM_COMPAT_DATA_PATH`, `STEAM_COMPAT_CLIENT_INSTALL_PATH`, `WINEPREFIX`
- Stages trainer into compatdata
- Executes: `"$proton" run "$trainer_path"`

### Environment Variable Management

**Variables to strip** (28 total, from `SteamLaunchService.GetEnvironmentVariablesToClear()`, line 233):

WINE variables:

- `WINESERVER`, `WINELOADER`, `WINEDLLPATH`, `WINEDLLOVERRIDES`, `WINEDEBUG`
- `WINEESYNC`, `WINEFSYNC`, `WINELOADERNOEXEC`
- `WINE_LARGE_ADDRESS_AWARE`, `WINE_DISABLE_KERNEL_WRITEWATCH`
- `WINE_HEAP_DELAY_FREE`, `WINEFSYNC_SPINCOUNT`

System/library variables:

- `LD_PRELOAD`, `LD_LIBRARY_PATH`
- `GST_PLUGIN_PATH`, `GST_PLUGIN_SYSTEM_PATH`, `GST_PLUGIN_SYSTEM_PATH_1_0`

Steam/game ID variables:

- `SteamGameId`, `SteamAppId`, `GAMEID`

Proton variables:

- `PROTON_LOG`, `PROTON_DUMP_DEBUG_COMMANDS`, `PROTON_USE_WINED3D`
- `PROTON_NO_ESYNC`, `PROTON_NO_FSYNC`, `PROTON_ENABLE_NVAPI`

DXVK/VKD3D variables:

- `DXVK_CONFIG_FILE`, `DXVK_STATE_CACHE_PATH`, `DXVK_LOG_PATH`
- `VKD3D_CONFIG`, `VKD3D_DEBUG`

**Variables to set** (3):

- `STEAM_COMPAT_DATA_PATH` = `<compatdata_path>`
- `STEAM_COMPAT_CLIENT_INSTALL_PATH` = `<steam_client_path>`
- `WINEPREFIX` = `<compatdata_path>/pfx`

**Native app implication**: In the Rust app, `std::process::Command` can use `.env_clear()` or `.env_remove()` to strip variables. The shell scripts' `env -i` approach with explicit pass-through of `HOME`, `USER`, etc. is the cleanest pattern to replicate.

### Process Detection

The scripts use `pgrep -af -- "$process_name"` to check if a process is running. The `-a` flag matches against the full command line, `-f` flag matches against the full process name.

The script also tries matching without the `.exe` extension (line 196-207 in `steam-launch-helper.sh`):

```bash
process_name_without_extension="${process_name%.exe}"
pgrep -af -- "$process_name_without_extension"
```

**Native app replacement**: Use `/proc` enumeration or `pgrep` subprocess. The feature spec recommends `pidfd_open` + `poll` for race-free process monitoring on Linux 5.3+.

### Trainer Staging

Before Proton can run a trainer, the trainer `.exe` must be placed inside the game's compatdata prefix. The staging path is:

```
$compatdata/pfx/drive_c/CrossHook/StagedTrainers/<trainer_filename>
```

The corresponding Windows path (passed to `proton run`) is:

```
C:\CrossHook\StagedTrainers\<trainer_filename>
```

This is because Proton maps `drive_c` to the WINEPREFIX's virtual C: drive. The `mkdir -p` and `cp -f` operations ensure the directory exists and the file is overwritten on each launch.

### Desktop Entry Generation

Generated by `SteamExternalLauncherExportService.BuildDesktopEntryContent()` (line 251).

**Format** (freedesktop.org standard):

```desktop
[Desktop Entry]
Type=Application
Version=1.0
Name=<display_name> - Trainer
Comment=Trainer launcher for <display_name>. Generated by CrossHook: https://github.com/yandy-r/crosshook
Exec=/bin/bash <escaped_script_path>
Terminal=false
Categories=Game;
Icon=<icon_path_or_applications-games>
StartupNotify=false
```

**Icon fallback**: If no icon path is provided, defaults to the `applications-games` theme icon name.

**Exec escaping**: Backslashes are doubled, spaces are backslash-escaped, quotes are backslash-escaped.

**Output paths**:

- Script: `$HOME/.local/share/crosshook/launchers/<slug>-trainer.sh`
- Desktop entry: `$HOME/.local/share/applications/crosshook-<slug>-trainer.desktop`

**Slug generation** (`SanitizeLauncherSlug()`): Lowercase, replace non-alphanumeric with hyphens, collapse consecutive hyphens, trim leading/trailing hyphens. Fallback: `crosshook-trainer`.

### Exported Trainer Script Format

Generated by `BuildTrainerScriptContent()` (line 232):

```bash
#!/usr/bin/env bash
set -euo pipefail

# <display_name> - Trainer launcher
# Generated by CrossHook
# https://github.com/yandy-r/crosshook
# Launch this after the Steam game has reached the in-game menu.
export STEAM_COMPAT_DATA_PATH='<compatdata_path>'
export STEAM_COMPAT_CLIENT_INSTALL_PATH='<steam_client_path>'
export WINEPREFIX="$STEAM_COMPAT_DATA_PATH/pfx"
PROTON='<proton_path>'
TRAINER_WINDOWS_PATH='<trainer_windows_path>'
exec "$PROTON" run "$TRAINER_WINDOWS_PATH"
```

All dynamic values are single-quoted with `'...'` and internal single quotes are escaped via the `'\"'\"'` idiom.

### Log Streaming

The `MainForm.StreamSteamHelperLogAsync()` method (line 2908) implements file-based log tailing:

1. Poll every 500ms for up to 2 minutes
2. Open log file with `FileShare.ReadWrite` (allows concurrent writing by the script)
3. Seek to last read position
4. Read new lines and push to console UI via `LogToConsole()`

**Native app implication**: In Rust/Tauri, this maps to `tokio::fs::File` with periodic reads, or `inotify`/`kqueue` for file change notification. The Tauri IPC event system can push log lines to the React frontend.

---

## Internal Services

### Service Communication Patterns

The WinForms app uses **direct static method calls** between services (no dependency injection, no message bus). All services are stateless except for `ProfileService`, `AppSettingsService`, and `RecentFilesService` which hold directory paths.

- `MainForm` -> `SteamAutoPopulateService.AttemptAutoPopulate()` (async via `Task.Run`)
- `MainForm` -> `SteamLaunchService.Validate()`, `.CreateHelperStartInfo()`, `.ConvertToUnixPath()`
- `MainForm` -> `SteamExternalLauncherExportService.ExportLaunchers()`
- `MainForm` -> `ProfileService.SaveProfile()`, `.LoadProfile()`, `.GetProfileNames()`
- `MainForm` -> `AppSettingsService.LoadAppSettings()`, `.SaveAppSettings()`
- `MainForm` -> `RecentFilesService.LoadRecentFiles()`, `.SaveRecentFiles()`

### Data Structures Passed Between Components

**`SteamAutoPopulateRequest`**: `GamePath` + `SteamClientInstallPath` -> returns `SteamAutoPopulateResult` with per-field states (`Found`/`NotFound`/`Ambiguous`), resolved values, diagnostics list, and manual hints list.

**`SteamLaunchRequest`**: Contains all fields needed to build shell script arguments: `GamePath`, `TrainerPath`, `TrainerHostPath`, `SteamAppId`, `SteamCompatDataPath`, `SteamProtonPath`, `SteamClientInstallPath`, `LaunchTrainerOnly`, `LaunchGameOnly`.

**`SteamExternalLauncherExportRequest`**: `LauncherName`, `TrainerPath`, `LauncherIconPath`, `SteamAppId`, `SteamCompatDataPath`, `SteamProtonPath`, `SteamClientInstallPath`, `TargetHomePath`.

**`ProfileData`**: The 12-field flat struct (see Profile Format section).

### Async/Background Work

The existing app uses these async patterns:

1. **`Task.Run()` for CPU/IO work**: `SteamAutoPopulateService.AttemptAutoPopulate()` runs on a thread pool thread
2. **`Task.Run()` for log streaming**: `StreamSteamHelperLogAsync()` polls on a background thread
3. **UI marshaling**: `LogToConsole()` uses `Invoke()` to marshal back to the UI thread

**Native app equivalents in Tauri**:

- Tauri commands are async by default (`#[tauri::command]`)
- Use `tokio::spawn` for background tasks
- Push events to frontend via `app.emit()` or `window.emit()`
- Log streaming should use `tokio::fs` with `tokio::time::interval` or `inotify`

---

## Configuration

### Paths Used by the Native App (XDG Compliant)

| Purpose         | Path                                   | Notes                                           |
| --------------- | -------------------------------------- | ----------------------------------------------- |
| Profiles        | `~/.config/crosshook/profiles/`        | TOML format (new) with legacy `.profile` import |
| Settings        | `~/.config/crosshook/settings.toml`    | App preferences                                 |
| Logs            | `~/.local/share/crosshook/logs/`       | Steam helper logs                               |
| Launchers       | `~/.local/share/crosshook/launchers/`  | Exported `.sh` scripts                          |
| Desktop entries | `~/.local/share/applications/`         | `.desktop` files                                |
| Temporary logs  | `$TMPDIR/crosshook-steam-helper-logs/` | Runtime helper logs                             |

### Steam Paths to Search

| Path                                                | Purpose                                       |
| --------------------------------------------------- | --------------------------------------------- |
| `$HOME/.steam/root`                                 | Primary Steam root (symlink)                  |
| `$HOME/.local/share/Steam`                          | Direct Steam install                          |
| `$HOME/.var/app/com.valvesoftware.Steam/data/Steam` | Flatpak Steam (not in current C# code)        |
| `<steam_root>/steamapps/libraryfolders.vdf`         | Library folder registry                       |
| `<library>/steamapps/appmanifest_*.acf`             | Per-game manifests                            |
| `<library>/steamapps/compatdata/<appid>/`           | Game WINE prefixes                            |
| `<steam_root>/config/config.vdf`                    | Global Steam config (compat tool mappings)    |
| `<steam_root>/userdata/*/config/localconfig.vdf`    | Per-user config (compat tool mappings)        |
| `<steam_root>/steamapps/common/*/proton`            | Official Proton installs                      |
| `<steam_root>/compatibilitytools.d/*/proton`        | Custom Proton installs (GE-Proton, etc.)      |
| `<tool_dir>/compatibilitytool.vdf`                  | Proton tool metadata (aliases, display names) |
| `/usr/share/steam/compatibilitytools.d/`            | System-wide compat tools                      |
| `/usr/local/share/steam/compatibilitytools.d/`      | Local system compat tools                     |

### Mounted Drive Search Roots

For resolving WINE `dosdevices` paths to real host paths (from `SteamLaunchService.GetMountedHostSearchRoots()`):

- `/mnt`
- `/media`
- `/run/media`
- `/var/run/media`

The search enumerates 2 levels deep within each root to find matching paths.

---

## Architectural Patterns

- **Stateless services with static methods**: `SteamAutoPopulateService`, `SteamLaunchService`, and `SteamExternalLauncherExportService` are all `static` classes. Port as free functions or lightweight Rust structs.
- **Diagnostics accumulator pattern**: `SteamAutoPopulateService` collects a `List<string> diagnostics` throughout the entire operation, building a trace of what was tried. This pattern should carry over to the Rust implementation for debuggability.
- **Three-state field resolution**: `SteamAutoPopulateFieldState` enum (`NotFound`, `Found`, `Ambiguous`) is used for each resolved field. This prevents silent guessing when multiple candidates exist.
- **Path normalization pipeline**: Every path goes through normalization before use: `NormalizePathForHostLookup()` handles Windows paths, dosdevices symlinks, and mounted drive scanning. The native app eliminates most of this since paths are already native Unix.
- **Validation-then-execute pattern**: Both `SteamLaunchService` and `SteamExternalLauncherExportService` separate validation from execution, returning typed result objects.
- **Script-as-boundary**: The shell scripts form a clean boundary between the app and the OS. The app constructs arguments and environment, the script handles process lifecycle. This boundary is preserved in the native app (Phase 1-2).

---

## Gotchas and Edge Cases

- **Path normalization is the hardest part to port correctly**: The C# code has extensive logic for Windows-to-Unix path conversion (`Z:\mnt\...` -> `/mnt/...`), dosdevices symlink resolution, and mounted drive scanning. In the native app, most of this is unnecessary since paths are already Unix -- but legacy `.profile` files may contain Windows paths that need the Z-drive conversion.
- **VDF parser handles both object and leaf values for the same key pattern**: The `libraryfolders.vdf` entries can be either `"0" "/path"` (leaf) or `"0" { "path" "/path" }` (object). The parser must handle both.
- **CompatToolMapping keys are App ID strings, not integers**: The key `"0"` means "global default", not app ID zero. All comparisons are string-based.
- **Proton tool name resolution is fuzzy**: Exact match, normalized match (strip non-alphanumeric), and heuristic substring matching are all attempted. This handles version number variations like `proton_9` vs `Proton 9.0` vs `proton-9.0-4`.
- **File descriptors must be closed before launching trainer**: The shell scripts close all FDs > 2 from `/proc/self/fd/*` to prevent WINE's wineserver connections from leaking into the trainer's Proton session. In Rust, ensure `Command::new()` does not inherit unnecessary FDs (use `.stdin(Stdio::null())` etc.).
- **Trainer path is a Windows path**: Even though the trainer `.exe` is on a Linux filesystem, `proton run` expects a Windows-style path (`C:\CrossHook\StagedTrainers\trainer.exe`) pointing to the staged copy inside the WINEPREFIX.
- **Flatpak Steam not handled**: The current C# code does not check `~/.var/app/com.valvesoftware.Steam/`. The native app should add this as a Steam root candidate.
- **`setsid` for trainer launch**: The trainer is launched in a new session (`setsid`) to fully detach from the launching process. This prevents the trainer from being killed when CrossHook exits.
- **Log streaming 2-minute timeout**: The log poll has a hard 2-minute deadline. This is sufficient for startup but may miss late-arriving log lines.
- **Profile names are validated against both Unix AND Windows invalid characters**: The validation in `ProfileService.ValidateProfileName()` checks `Path.GetInvalidFileNameChars()` plus a hardcoded Windows reserved character set (`<>:"/\|?*`). This ensures profiles created on Linux can be loaded on Windows and vice versa.
- **`TrainerHostPath` vs `TrainerPath`**: `TrainerPath` is the Windows-style path used inside WINE. `TrainerHostPath` is the native Linux path to the trainer source file. The staging process copies from host path to compatdata, then `proton run` uses the Windows path. The native app simplifies this since it operates on Linux paths directly, but must still construct the Windows path for `proton run`.

---

## Other Docs

- `docs/plans/platform-native-ui/feature-spec.md`: Full feature specification with architecture, data models, and phasing
- `docs/plans/platform-native-ui/research-external.md`: Steam APIs, Linux process APIs, VDF parsers, distribution
- `docs/plans/platform-native-ui/research-business.md`: User stories, business rules, domain model
- `docs/plans/platform-native-ui/research-technical.md`: Architecture, Win32-to-Linux mapping, Rust traits
- `docs/plans/platform-native-ui/research-ux.md`: UI patterns, competitive analysis, Steam Deck UX
- `docs/plans/platform-native-ui/research-recommendations.md`: Framework comparison, phasing, risk assessment
- [Valve Developer Community - Steam Browser Protocol](https://developer.valvesoftware.com/wiki/Steam_browser_protocol)
- [Proton FAQ (GitHub Wiki)](https://github.com/ValveSoftware/Proton/wiki/Proton-FAQ)
- [ArchWiki: Steam/Troubleshooting](https://wiki.archlinux.org/title/Steam/Troubleshooting)
- [freedesktop.org Desktop Entry Specification](https://specifications.freedesktop.org/desktop-entry-spec/latest/)
