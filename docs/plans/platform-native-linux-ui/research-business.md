# Business Logic Research: platform-native-linux-ui

## Executive Summary

CrossHook's core problem is architectural: when the WinForms app runs inside WINE and loads a game inside WINE, trainers cannot access the game's memory because they share the same WINE session. The team already proved that launching trainers from a native Linux shell (outside WINE) with a clean Proton environment works reliably. The native Linux UI must wrap this proven shell-based approach in a user-friendly application while extracting all game knowledge, profile management, Steam discovery, and Proton configuration logic from the existing C# codebase. The business value is eliminating manual script creation and providing a first-class native experience for the 100% of CrossHook's target audience that runs on Linux.

## User Stories

### Primary User: Steam Deck Gamer

- As a Steam Deck gamer, I want to select a game and trainer from a simple interface so that I can launch them together without writing shell scripts.
- As a Steam Deck gamer, I want CrossHook to auto-detect my Steam libraries, game installations, and Proton versions so that I do not have to manually locate file paths.
- As a Steam Deck gamer, I want saved profiles to remember my game/trainer/Proton configuration so that I can replay the same setup with one click.
- As a Steam Deck gamer, I want the UI to work well with a gamepad and touchscreen so that I can use it in Gaming Mode without a keyboard.

### Secondary User: Linux Desktop Gamer

- As a Linux desktop gamer, I want a native application that integrates with my desktop environment (system tray, notifications, .desktop launchers) so that it feels like a first-class Linux tool.
- As a Linux desktop gamer, I want to manage multiple game profiles with different trainers and Proton versions so that I can maintain configurations for my entire library.
- As a Linux desktop gamer, I want to export standalone launcher scripts from my profiles so that I can run trainers without opening the full application.

### Tertiary User: Modding Enthusiast

- As a modding enthusiast, I want to configure DLL paths and injection parameters alongside trainer settings so that I can manage complex mod setups through one tool.
- As a modding enthusiast, I want a console/log view showing exactly what commands are being executed so that I can diagnose failures and understand the launch pipeline.

## Business Rules

### Core Rules

1. **Trainer Must Run Outside WINE**: The trainer process must be launched from the native Linux environment using Proton directly against the game's compatdata prefix. Running a trainer inside the same WINE session as CrossHook causes memory access failures.
   - Validation: The launch command must use `$PROTON run "$TRAINER_PATH"` with a clean environment, not a WINE-bridged process.
   - Exception: None. This is the fundamental architectural constraint that motivates the entire native UI effort.

2. **Game Must Be Launched via Steam**: When using Steam mode, the game must be launched through Steam's own launch mechanism (`steam -applaunch <appid>`) so that Steam's DRM, overlay, and Proton runtime are properly initialized.
   - Validation: The Steam App ID must be valid and the game must be installed.
   - Exception: Direct mode (non-Steam games or games that do not require Steam's launcher).

3. **Trainer Staged into Compatdata**: Before Proton runs a trainer, the trainer executable must be copied into the target game's compatdata prefix at `pfx/drive_c/CrossHook/StagedTrainers/`. This ensures the trainer is accessible from the Windows path namespace that Proton presents.
   - Validation: The staged path must exist and the file must be successfully copied before launch.
   - Exception: Trainers that are already located within the compatdata prefix.

4. **Clean Environment Required**: All WINE/Proton-specific environment variables inherited from any parent WINE session must be stripped before launching a trainer with Proton. The required environment consists of exactly three variables: `STEAM_COMPAT_DATA_PATH`, `STEAM_COMPAT_CLIENT_INSTALL_PATH`, and `WINEPREFIX`.
   - Validation: The launch process must explicitly unset 30+ inherited WINE/Proton/DXVK variables (see `GetEnvironmentVariablesToClear()` in `SteamLaunchService.cs` and the `unset` block in the shell scripts).
   - Exception: None. Inherited variables cause silent failures.

5. **Two-Phase Steam Launch**: In Steam mode, the workflow is always two phases: (1) launch the game via Steam, (2) wait until the game reaches the in-game menu, then launch the trainer. These cannot be combined into a single action because the game needs time to initialize its process and memory layout before the trainer can attach.
   - Validation: The UI must enforce this sequencing (the current WinForms app toggles the launch button between "Launch Game" and "Launch Trainer" states).
   - Exception: Trainer-only mode (game already running) and game-only mode (testing without trainer).

6. **Profile Compatibility**: Profile data must remain compatible with the existing `.profile` file format. A profile stores: GamePath, TrainerPath, Dll1Path, Dll2Path, LaunchInject1, LaunchInject2, LaunchMethod, UseSteamMode, SteamAppId, SteamCompatDataPath, SteamProtonPath, SteamLauncherIconPath.
   - Validation: Profiles saved by the WinForms app must be loadable by the native app and vice versa.
   - Exception: New fields may be added with backward-compatible defaults.

7. **Architecture Matching**: DLL injection requires that the DLL architecture (32-bit vs 64-bit) matches the target process architecture. The existing codebase validates this by reading the PE header's Optional Header magic value.
   - Validation: `IMAGE_NT_OPTIONAL_HDR32_MAGIC` (0x10B) vs `IMAGE_NT_OPTIONAL_HDR64_MAGIC` (0x20B).
   - Exception: DLL injection is a direct-mode feature. Steam mode does not support DLL injection.

### Edge Cases

- **Multiple Steam Libraries**: Games may be installed across multiple Steam library folders (e.g., different drives). The auto-populate service parses `libraryfolders.vdf` to discover all library roots and matches the game path against manifests in each.
- **Ambiguous App ID Matching**: If multiple Steam manifests match a game executable path, the system must refuse to guess and report the ambiguity. The `SteamAutoPopulateService` returns `SteamAutoPopulateFieldState.Ambiguous` in this case.
- **Custom Proton Versions**: Users may use GE-Proton, TKG, or other custom Proton builds located in `~/.steam/root/compatibilitytools.d/` rather than the official paths under `steamapps/common/`. The Proton resolution logic searches both official and custom tool directories.
- **Compatdata Not Yet Created**: A game's compatdata directory may not exist until the game has been launched at least once through Steam. The auto-populate service reports this as a diagnostic hint rather than a hard failure.
- **WINE Z: Drive Mapping**: When CrossHook runs inside WINE, paths starting with `Z:\` map to the Linux root filesystem. The `ConvertToUnixPath()` method handles this by checking for the `Z:` drive letter pattern, and `ResolveDosDevicesPath()` follows dosdevices symlinks to reconstruct native host paths.
- **Mounted External Drives**: Steam libraries on external drives (e.g., SD cards on Steam Deck at `/run/media/`) require scanning `/mnt`, `/media`, `/run/media`, and `/var/run/media` to resolve paths. The existing `ResolveMountedHostPathByScanning()` method handles this.
- **Trainer Already Running**: If the trainer process is already visible (via `pgrep`), the launch scripts skip re-launching it. The native UI should detect and report this state.
- **Steam Command Discovery**: The shell scripts discover the Steam CLI by checking for `steam` on PATH or falling back to `$STEAM_CLIENT/steam.sh`. The native app should replicate this discovery.

## Workflows

### Primary Workflow: Steam Mode Game + Trainer Launch

1. User opens native Linux UI.
2. User selects a game executable (or browses for one).
3. System auto-populates Steam App ID, compatdata path, and Proton path by scanning Steam library manifests and config files.
4. User selects a trainer executable.
5. User saves the configuration as a named profile.
6. User clicks "Launch Game".
7. System launches the game via `steam -applaunch <appid>` in the background.
8. System waits for the game process to become visible (polling via `pgrep`).
9. UI transitions to "Launch Trainer" state with guidance to wait for the in-game menu.
10. User clicks "Launch Trainer" once the game is at the menu.
11. System stages the trainer into the compatdata prefix.
12. System strips inherited environment variables.
13. System launches the trainer via `$PROTON run "C:\CrossHook\StagedTrainers\trainer.exe"` in a detached session (`setsid`).
14. System streams the helper log to the UI console.
15. Success: Trainer attaches to the game process with memory access.

### Secondary Workflow: Profile-Based Quick Launch

1. User opens native Linux UI.
2. System loads saved profiles from the `Profiles/` directory.
3. User selects a saved profile.
4. System populates all fields from the profile data.
5. User clicks "Launch Game" (proceeds to step 7 of primary workflow).

### Tertiary Workflow: External Launcher Export

1. User configures a Steam mode profile (game, trainer, Steam fields).
2. User clicks "Export Launcher".
3. System generates a bash script at `~/.local/share/crosshook/launchers/<slug>-trainer.sh`.
4. System generates a `.desktop` entry at `~/.local/share/applications/crosshook-<slug>-trainer.desktop`.
5. User can now launch the trainer from their desktop application menu without opening CrossHook.

### Error Recovery

- **Game launch fails (Steam not found)**: System checks for `steam` on PATH and `$STEAM_CLIENT/steam.sh`. If neither exists, display a clear error with instructions to verify the Steam installation.
- **Proton path not executable**: Validate that the resolved Proton path has execute permissions before attempting launch. Display the exact path that failed.
- **Trainer file not found**: Validate trainer host path existence before staging. If the path is stale (from a profile), prompt the user to re-select.
- **Compatdata directory missing**: Inform the user that the game must be launched through Steam at least once to create the compatdata prefix.
- **Trainer fails to start**: Stream the helper log to the console so the user can see the Proton error output. Common causes: wrong Proton version, missing runtime dependencies, trainer incompatibility.
- **Game process not detected after timeout**: The helper scripts wait up to 90 seconds (configurable via `--game-timeout-seconds`). If the process is not detected, log a warning and proceed with trainer launch anyway (the game may use a different process name).

## Domain Model

### Key Entities

- **Game**: A Windows game executable that runs through Proton/WINE. Key attributes: executable path (host-side), Steam App ID, install directory, game process name.
- **Trainer**: A Windows executable that modifies a running game's memory for cheats/mods (e.g., FLiNG, WeMod standalone trainers). Key attributes: host path, Windows path (after staging), trainer process name.
- **Profile**: A saved configuration that binds a game to a trainer and all required launch parameters. Key attributes: profile name, game path, trainer path, DLL paths (2), launch method, Steam mode flag, Steam App ID, compatdata path, Proton path, launcher icon path, auto-inject flags.
- **Proton Prefix (Compatdata)**: The per-game WINE prefix managed by Proton, located at `<library>/steamapps/compatdata/<appid>/`. Contains the virtual Windows filesystem (`pfx/drive_c/`), registry, and WINE configuration. The trainer must be staged into this prefix and Proton must target it.
- **Proton Runtime**: The specific Proton version used to translate Windows API calls. Can be official (Valve Proton) or custom (GE-Proton, TKG). Located either under `steamapps/common/` or `compatibilitytools.d/`. Key attribute: the `proton` executable script at its root.
- **Steam Library**: A directory containing `steamapps/` with game installations, app manifests (`appmanifest_*.acf`), and compatdata prefixes. Multiple libraries can exist across drives.
- **App Manifest**: A Steam VDF file (`appmanifest_<appid>.acf`) that maps a Steam App ID to an install directory. Used by the auto-populate service to match a game path to its Steam configuration.
- **Compat Tool Mapping**: Steam's `config.vdf` and per-user `localconfig.vdf` contain `CompatToolMapping` entries that specify which Proton version is configured for each game (by App ID) or as the global default (App ID "0").
- **Launch Method**: An enum of Windows process creation strategies (CreateProcess, CmdStart, ShellExecute, ProcessStart, plus two stubs: CreateThreadInjection, RemoteThreadInjection). Relevant only for direct mode; Steam mode bypasses this entirely.
- **External Launcher**: A generated pair of files (bash script + .desktop entry) that encapsulates a trainer launch configuration for use outside CrossHook.

### State Transitions

- **Idle** -> **Configuring**: User opens the app or loads a profile.
- **Configuring** -> **Game Launching**: User clicks "Launch Game". System invokes `steam -applaunch`.
- **Game Launching** -> **Awaiting Trainer Launch**: Game process detected (or startup delay elapsed). UI shows "Launch Trainer" button.
- **Awaiting Trainer Launch** -> **Trainer Launching**: User clicks "Launch Trainer". System stages trainer and invokes Proton.
- **Trainer Launching** -> **Running**: Trainer process started successfully. Log streaming active.
- **Running** -> **Idle**: Game or trainer exits. System cleans up state.
- **Any State** -> **Error**: A validation or launch failure occurs. System displays error message and returns to the last valid state.
- **Configuring** -> **Exporting**: User clicks "Export Launcher". System writes script and .desktop files, then returns to Configuring.

### Lifecycle Events

- Profile loaded / saved / deleted
- Steam auto-populate completed (with results: found, ambiguous, not found per field)
- Game launch requested / game process detected / game process timeout
- Trainer staged / trainer launch requested / trainer process started / trainer process exited
- Helper log line received
- External launcher exported

## Existing Codebase Integration

### Related Features

- `/src/CrossHookEngine.App/Services/SteamAutoPopulateService.cs`: The largest service file (~900 lines). Contains all Steam library discovery, VDF manifest parsing, game-to-App-ID matching, compatdata path derivation, Proton version resolution, and compat tool mapping extraction. This is the single most valuable piece of domain logic to reuse or port.
- `/src/CrossHookEngine.App/Services/SteamLaunchService.cs`: Builds the launch command and ProcessStartInfo for Steam helper scripts. Contains path conversion (Windows/Unix), dosdevices symlink resolution, environment variable management, and the list of variables to clear. Critical domain knowledge.
- `/src/CrossHookEngine.App/Services/SteamExternalLauncherExportService.cs`: Generates trainer launch scripts and .desktop entries. The generated script content (`BuildTrainerScriptContent`) encodes the exact proven launch pattern.
- `/src/CrossHookEngine.App/Services/ProfileService.cs`: Profile CRUD with `.profile` file format (key=value text). Includes profile name validation (Windows-safe characters). The file format is the compatibility contract.
- `/src/CrossHookEngine.App/Services/AppSettingsService.cs`: Simple key=value settings persistence. Stores `AutoLoadLastProfile` and `LastUsedProfile`.
- `/src/CrossHookEngine.App/Services/RecentFilesService.cs`: MRU (most recently used) file lists with section-based INI format. Tracks recent game paths, trainer paths, and DLL paths.
- `/src/CrossHookEngine.App/Services/CommandLineParser.cs`: Parses `-p <profile>` and `-autolaunch <path>` CLI arguments. The native app should support the same CLI contract for compatibility.
- `/src/CrossHookEngine.App/runtime-helpers/steam-launch-helper.sh`: The full game+trainer launch orchestration script. Handles Steam launch, process detection, startup delay, trainer staging, environment cleanup, and Proton invocation.
- `/src/CrossHookEngine.App/runtime-helpers/steam-launch-trainer.sh`: The trainer-only launch script. Spawns a detached host runner (`setsid env -i ... /bin/bash runner_script`) with a minimal environment to fully escape any parent WINE session.
- `/src/CrossHookEngine.App/runtime-helpers/steam-host-trainer-runner.sh`: The actual trainer execution script. Stages trainer, cleans environment, and runs `$proton run $trainer_path`.
- `/src/CrossHookEngine.App/Core/ProcessManager.cs`: Process lifecycle management with Win32 P/Invoke (launch, attach, suspend, resume, kill). Six launch methods. The native app needs a Linux-native equivalent (likely simpler, using standard process APIs).
- `/src/CrossHookEngine.App/Injection/InjectionManager.cs`: DLL injection via LoadLibraryA + CreateRemoteThread. This is Windows-only and cannot work natively on Linux. For the native UI, DLL injection would need to be delegated to a WINE/Proton context.
- `/src/CrossHookEngine.App/Memory/MemoryManager.cs`: Process memory read/write via Win32 APIs. Like injection, this is Windows-only. The native Linux equivalent would use `process_vm_readv`/`process_vm_writev` or `/proc/<pid>/mem`.
- `/src/CrossHookEngine.App/Forms/MainForm.cs`: The 3600+ line UI monolith. Contains all UI construction, event handlers, state management, and launch orchestration. Not directly reusable, but defines all the user-facing workflows and state transitions.

### Patterns to Follow

- **Service Extraction Pattern**: The codebase already separates domain logic into static service classes (`SteamAutoPopulateService`, `SteamLaunchService`, `SteamExternalLauncherExportService`) that are independent of WinForms. These can be ported or called from a native app.
- **Request/Result Pattern**: Services use dedicated request and result types (e.g., `SteamLaunchRequest` -> `SteamLaunchValidationResult`, `SteamAutoPopulateRequest` -> `SteamAutoPopulateResult`). The native app should follow this pattern.
- **Validation-Before-Execution**: Every service has a `Validate()` method that returns a result with `IsValid` and `ErrorMessage` before any side-effecting operation. The native app must replicate this.
- **Diagnostic Accumulation**: The auto-populate service collects diagnostics and manual hints as lists of strings, surfacing them to the user. The native app should present these in the console/log view.
- **VDF Parsing**: The codebase includes a complete Steam VDF (Valve Data Format) key-value parser (`ParseKeyValueFile`, `ParseKeyValueContent`). This is critical domain logic for reading Steam manifests and config files.
- **Path Normalization**: Extensive path conversion between Windows, Unix, and dosdevices paths. The native app (running natively on Linux) will not need most of the WINE path conversion, but it will still need to parse manifests that contain Windows-style paths.

### Components to Leverage

- **Shell Scripts (Direct Reuse)**: The three runtime helper scripts (`steam-launch-helper.sh`, `steam-launch-trainer.sh`, `steam-host-trainer-runner.sh`) are already native Linux scripts. A native app can invoke them directly without WINE bridging. This is the proven working approach.
- **SteamAutoPopulateService Logic (Port)**: The Steam library discovery, VDF parsing, manifest matching, and Proton resolution logic must be reimplemented natively. The C# code serves as the authoritative specification.
- **ProfileService Format (Direct Compatibility)**: The `.profile` file format (key=value text) is simple enough to read/write from any language. The native app should use the same format for cross-compatibility.
- **SteamExternalLauncherExportService (Port)**: The script and .desktop generation logic should be reimplemented natively.
- **Environment Variable Cleanup List (Direct Reuse)**: The list of 30+ environment variables to clear before Proton launch is critical domain knowledge. It appears in both `SteamLaunchService.GetEnvironmentVariablesToClear()` and the shell scripts.

## Success Criteria

- [ ] Native Linux application launches trainers with the same reliability as the manual shell scripts
- [ ] Steam library auto-detection works for standard, Flatpak, and multi-drive Steam installs
- [ ] Proton version auto-detection resolves official Valve Proton and custom builds (GE-Proton, TKG)
- [ ] Profiles saved by the WinForms app can be loaded by the native app (format compatibility)
- [ ] The two-phase Steam launch workflow (game first, then trainer) is enforced by the UI
- [ ] External launcher export produces functional .sh and .desktop files
- [ ] The application is usable on Steam Deck in both Desktop Mode and (aspirationally) Gaming Mode
- [ ] Console/log view surfaces helper script output in real-time
- [ ] Game process detection works via native Linux process enumeration (no WINE dependency)
- [ ] Error messages are actionable, pointing to the specific path or configuration that failed

## Open Questions

- **Technology Choice**: Should the native UI be built with GTK4 (GNOME-native), Qt (KDE-native, also works on GNOME), Electron/Tauri (web-based), or a terminal UI (TUI)? The Steam Deck runs KDE, which favors Qt. The broader Linux desktop is split.
- **DLL Injection in Native Context**: The current DLL injection path (LoadLibraryA + CreateRemoteThread) is Windows-only. Should the native app attempt to support DLL injection by delegating to a WINE/Proton subprocess, or should DLL injection remain a WinForms-only feature while the native app focuses on the trainer launch workflow?
- **Profile Storage Location**: Should the native app store profiles in the same directory as the WinForms app (beside the executable) or follow XDG conventions (`~/.config/crosshook/profiles/`, `~/.local/share/crosshook/`)? Cross-compatibility with the WinForms app complicates XDG adoption.
- **Flatpak Steam Detection**: Flatpak Steam installs use different paths (`~/.var/app/com.valvesoftware.Steam/`). The current `SteamAutoPopulateService` does not explicitly handle Flatpak paths. The native app should.
- **Game Process Monitoring**: The WinForms app uses Win32 process APIs. The native app needs a Linux-native approach. Options include polling `/proc/`, using `pgrep` (as the shell scripts do), or using process monitoring via inotify/pidfd.
- **macOS Support Timeline**: The feature description focuses on Linux first. When (if ever) should macOS native UI be considered? The architecture should allow for it.
- **WinForms App Sunset**: Is the WinForms app being maintained alongside the native app, or does the native app eventually replace it? This affects profile format decisions and feature parity requirements.
- **Community Profiles**: The feature enhancement research strongly recommended community profile sharing. Should the native app be designed from the start with a profile import/sync mechanism, or is that a future feature?
