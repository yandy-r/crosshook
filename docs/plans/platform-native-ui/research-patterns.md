# Pattern Research: platform-native-ui

This document catalogs the architectural patterns, coding conventions, error handling strategies, and shell script patterns used in the existing CrossHook C# codebase. These patterns should guide the Rust/Tauri native UI implementation to maintain conceptual consistency while adopting idiomatic Rust equivalents.

## Relevant Files

- `src/CrossHookEngine.App/Services/SteamAutoPopulateService.cs`: Steam library discovery, VDF parsing, Proton resolution (~1286 lines, most complex service)
- `src/CrossHookEngine.App/Services/SteamLaunchService.cs`: Launch validation, env cleanup, path conversion, script invocation (~736 lines)
- `src/CrossHookEngine.App/Services/SteamExternalLauncherExportService.cs`: `.sh` + `.desktop` file generation for standalone launchers (~350 lines)
- `src/CrossHookEngine.App/Services/ProfileService.cs`: Profile CRUD with `Key=Value` serialization (~225 lines)
- `src/CrossHookEngine.App/Services/AppSettingsService.cs`: App settings persistence with same `Key=Value` format (~81 lines)
- `src/CrossHookEngine.App/Services/RecentFilesService.cs`: INI-style section-based recent file tracking (~131 lines)
- `src/CrossHookEngine.App/Services/CommandLineParser.cs`: Simple `-p` and `-autolaunch` flag parser (~54 lines)
- `src/CrossHookEngine.App/Core/ProcessManager.cs`: Win32 process lifecycle with events (not directly portable, but pattern is relevant)
- `src/CrossHookEngine.App/Injection/InjectionManager.cs`: DLL injection via CreateRemoteThread (Win32-only, not needed for MVP)
- `src/CrossHookEngine.App/Diagnostics/AppDiagnostics.cs`: Centralized trace logging with file output (~127 lines)
- `src/CrossHookEngine.App/Forms/MainForm.cs`: Full WinForms UI, contains the Steam launch workflow state machine
- `src/CrossHookEngine.App/Forms/MainFormStartupCoordinator.cs`: Auto-load profile resolution logic
- `src/CrossHookEngine.App/Program.cs`: Entry point with single-instance Mutex enforcement
- `src/CrossHookEngine.App/runtime-helpers/steam-launch-helper.sh`: Full game+trainer launch orchestrator
- `src/CrossHookEngine.App/runtime-helpers/steam-launch-trainer.sh`: Trainer-only launcher with `setsid env -i` escape
- `src/CrossHookEngine.App/runtime-helpers/steam-host-trainer-runner.sh`: Detached trainer runner (child of `steam-launch-trainer.sh`)

## Architectural Patterns

### Request/Result Pattern (Validate-Then-Execute)

Every service operation follows a strict three-type contract: **Request DTO**, **ValidationResult**, and **ExecutionResult**. The caller builds a request, validates it, and only proceeds if valid.

**SteamLaunchService** (lines 10-57 of `SteamLaunchService.cs`):

- `SteamLaunchRequest` -- flat DTO with string properties, all defaulting to `string.Empty`
- `SteamLaunchValidationResult` -- immutable `(bool IsValid, string ErrorMessage)` via constructor
- `SteamLaunchExecutionResult` -- immutable `(bool Succeeded, string Message, string HelperLogPath)`

**SteamAutoPopulateService** (lines 9-45):

- `SteamAutoPopulateRequest` -- input with `GamePath` and `SteamClientInstallPath`
- `SteamAutoPopulateResult` -- rich result with per-field `SteamAutoPopulateFieldState` enum (`NotFound`, `Found`, `Ambiguous`), plus `Diagnostics` and `ManualHints` lists

**SteamExternalLauncherExportService** (lines 7-48):

- `SteamExternalLauncherExportRequest` -- input DTO
- `SteamExternalLauncherExportValidationResult` -- same `(IsValid, ErrorMessage)` pattern
- `SteamExternalLauncherExportResult` -- output with generated paths

**Rust equivalent**: Map to `struct XxxRequest`, `fn validate(&self) -> Result<(), ValidationError>`, and `fn execute(request: &XxxRequest) -> Result<XxxResult, XxxError>`. The per-field state enum maps naturally to a Rust enum.

### Static Service Layer (No State, No DI)

The three Steam-related services (`SteamLaunchService`, `SteamAutoPopulateService`, `SteamExternalLauncherExportService`) are all `public static class` with only static methods. They hold no state and take all inputs through parameters.

Stateful services (`ProfileService`, `AppSettingsService`, `RecentFilesService`) are `sealed class` instances constructed with a `startupPath` string. They hold only the filesystem root directory as state.

**Rust equivalent**: Stateless services become module-level functions. Stateful services become structs with a `base_path: PathBuf` field.

### Diagnostic Accumulation Pattern

`SteamAutoPopulateService.AttemptAutoPopulate` creates two `List<string>` collections (`diagnostics` and `manualHints`) at the entry point and passes them by reference to every helper method. Each step appends human-readable diagnostic messages. The final result deduplicates them via `Distinct()` (line 612-613).

This is a non-failing diagnostic pattern -- the operation always returns a result even when individual discovery steps fail. Partial success is the norm.

**Key methods that accumulate diagnostics**:

- `DiscoverSteamRootCandidates()` -- logs each root candidate tried
- `DiscoverSteamLibraries()` -- logs VDF parse failures
- `FindGameMatch()` -- logs manifest parse errors and ambiguous matches
- `ResolveProtonPath()` -- logs Proton tool resolution steps
- `CollectCompatToolMappings()` -- logs config.vdf parse errors

**Rust equivalent**: A `DiagnosticCollector` struct or `Vec<Diagnostic>` passed through the pipeline. Consider using `tracing::info!` spans alongside the collected diagnostics for observability.

### Two-Phase Steam Launch State Machine

The MainForm tracks a `_steamTrainerLaunchPending` boolean flag that drives a two-phase launch workflow:

1. **Phase 1 (Game Launch)**: `_steamTrainerLaunchPending = false` -- button reads "Launch Game", request sets `LaunchGameOnly = true`
2. **Phase 2 (Trainer Launch)**: After successful game launch, `_steamTrainerLaunchPending = true` -- button reads "Launch Trainer", request sets `LaunchTrainerOnly = true`
3. **Reset**: Toggling Steam mode off, loading a new profile, or a failed launch resets the flag to `false`

The `UpdateSteamModeUiState()` method (line 2119) drives all UI state changes: button text, hint labels, and field enabled/disabled states.

**Rust/Tauri equivalent**: Model as a proper enum state machine:

```
enum LaunchPhase { Idle, GameLaunching, WaitingForTrainer, TrainerLaunching, SessionActive }
```

### Event-Driven Component Communication

`ProcessManager`, `InjectionManager`, and `MemoryManager` use C# `EventHandler<TEventArgs>` events:

- `ProcessManager`: `ProcessStarted`, `ProcessStopped`, `ProcessAttached`, `ProcessDetached`
- `InjectionManager`: `InjectionSucceeded`, `InjectionFailed`
- `MemoryManager`: `MemoryOperationSucceeded`, `MemoryOperationFailed`
- `ResumePanel`: `Resumed`

Each component defines dedicated `EventArgs` classes. The pattern is:

1. Component exposes `event EventHandler<XxxEventArgs> XxxOccurred`
2. Protected `OnXxx(XxxEventArgs e)` method invokes the event
3. MainForm subscribes in `RegisterEventHandlers()` (line 1621)

**Rust/Tauri equivalent**: Use Tauri's event system (`app.emit(...)`) or channels (`tokio::sync::broadcast`) for backend-to-frontend communication.

### VDF Key-Value Parser (Custom Implementation)

The codebase implements its own Steam VDF parser rather than using a library. The parser (lines 739-863 of `SteamAutoPopulateService.cs`) handles:

- Quoted string tokens with escape sequences (`\\`, `\"`, `\n`, `\r`, `\t`)
- Unquoted tokens (terminated by whitespace, `{`, or `}`)
- Nested `{ }` blocks as child nodes
- `//` comment lines
- Recursive descent via `ParseKeyValueObject(content, ref index, stopOnClosingBrace)`

The data structure is `SteamKeyValueNode`:

- `Value: string` -- for leaf nodes
- `Children: Dictionary<string, SteamKeyValueNode>` -- case-insensitive keys
- `GetChild(key)` -- null-safe child lookup

This parser is used for: `libraryfolders.vdf`, `appmanifest_*.acf`, `config.vdf`, `localconfig.vdf`, and `compatibilitytool.vdf`.

**Rust equivalent**: The feature spec recommends the `steam-vdf-parser` crate. However, the custom parser's specific behaviors (case-insensitive keys, recursive `FindDescendantByKey`) should be validated against the chosen crate's API.

### Profile Serialization (Key=Value Flat Format)

`ProfileService` uses a simple `Key=Value` line format with exactly 12 fields. No quoting, no escaping, no sections. Order matches the write order. Boolean values use `bool.TryParse` for safe parsing.

Fields: `GamePath`, `TrainerPath`, `Dll1Path`, `Dll2Path`, `LaunchInject1` (bool), `LaunchInject2` (bool), `LaunchMethod`, `UseSteamMode` (bool), `SteamAppId`, `SteamCompatDataPath`, `SteamProtonPath`, `SteamLauncherIconPath`.

Profile name validation (line 165) rejects: empty/whitespace, `.`/`..`, rooted paths, path separators, and characters invalid for Windows filenames.

**Rust equivalent**: The native app will use TOML for new profiles and must implement a legacy reader for this format. The legacy reader is straightforward: split on first `=`, match key, parse value.

## Code Conventions

### Naming Conventions

- **Private fields**: `_camelCase` prefix (e.g., `_profilesDirectoryPath`, `_steamTrainerLaunchPending`)
- **Public properties**: `PascalCase` (e.g., `GamePath`, `IsValid`, `SteamAppId`)
- **Methods**: `PascalCase` (e.g., `AttemptAutoPopulate`, `ResolveGameExecutableName`)
- **Parameters**: `camelCase` (e.g., `steamRootCandidates`, `normalizedGamePath`)
- **Local variables**: `camelCase` (e.g., `libraries`, `matchSelection`)
- **Constants**: `PascalCase` for C# constants (e.g., `RuntimeHelpersDirectoryName`), `UPPER_SNAKE_CASE` for Win32 constants (e.g., `PROCESS_ALL_ACCESS`)
- **Namespaces**: `CrossHookEngine.App.{Layer}` (Services, Core, Forms, Injection, Memory, UI, Diagnostics, Interop)

### File Organization

- One service class per file in `Services/`
- Related DTOs (Request, Result, internal helper types) are co-located in the same file as their service
- Internal helper types placed at the bottom of the file, after the service class
- `sealed class` for all DTOs and service instances (no inheritance)
- `static class` for stateless service logic
- `partial class` for Win32 interop (separates P/Invoke from logic)
- `#region Win32 API` blocks group P/Invoke declarations

### Method Visibility Pattern

- **`public static`**: Entry-point service methods callable from outside
- **`internal static`**: Helper methods visible for testing but not part of the public API
- **`private static`**: Implementation details
- Many methods that could be private are marked `internal` -- this suggests an intent toward testability even without a current test framework

### Path Handling Conventions

- All path normalization goes through `NormalizeHostStylePath()` which: trims, replaces `\` with `/`, resolves `.` and `..`, collapses repeated `/`
- `CombineNormalizedPathSegments()` is the universal path join (never uses `Path.Combine` for Unix-style paths)
- Windows-to-Unix conversion via `ConvertToUnixPath()` handles the `Z:\` drive letter shortcut and falls back to `winepath.exe`
- `LooksLikeWindowsPath()` checks for `X:\` or `X:/` pattern
- `NormalizeSteamHostPath()` chains `ConvertToUnixPath` + `ResolveDosDevicesPath` for full resolution

## Error Handling

### Validation-First, Never Throw From Services

Services validate input before execution and return structured results rather than throwing. The caller decides how to surface errors:

```
SteamLaunchValidationResult validation = SteamLaunchService.Validate(request);
if (!validation.IsValid) {
    LogToConsole(validation.ErrorMessage);
    MessageBox.Show(validation.ErrorMessage, ...);
    return;
}
```

This pattern means services are composable without try/catch at every call site.

**Exception**: `SteamExternalLauncherExportService.ExportLaunchers()` calls `Validate()` internally and throws `InvalidOperationException` on failure (line 117). This is the "execute-or-throw" variant used when the caller has already validated but wants defense-in-depth.

### ArgumentNullException.ThrowIfNull Guard Pattern

Every public method starts with `ArgumentNullException.ThrowIfNull(parameter)` for reference-type parameters. This is consistent across all services.

**Rust equivalent**: Not needed -- Rust's type system prevents null. Use `Option<T>` where absence is meaningful.

### Swallowed Exceptions in Discovery

`SteamAutoPopulateService` uses a "best-effort discovery" pattern where filesystem errors are caught and added to diagnostics rather than propagated:

```csharp
catch (Exception ex)
{
    diagnostics.Add($"Failed to parse Steam library file '{libraryFoldersPath}': {ex.Message}");
}
```

This appears in: `DiscoverSteamLibraries` (line 232), `FindGameMatch` (line 283), `CollectCompatToolMappings` (line 490), `TryAddCompatToolInstall` (line 726).

`SafeEnumerateFiles` and `SafeEnumerateDirectories` (lines 893-925) swallow all exceptions and return empty arrays. This is intentional for scanning directories the user may not have permission to read.

**Rust equivalent**: Use `Result` types and collect errors into a `Vec<String>` diagnostic list. The `?` operator should not be used for discovery steps -- use `match` or `.ok()` and log failures.

### Mounted Filesystem Resilience

`EnumerateMountedBaseDirectories()` (line 514-586) wraps each directory enumeration step in individual try/catch blocks. The inner loop can fail (e.g., permission denied on a mount point) without stopping enumeration of sibling directories. This is a deliberate resilience pattern for scanning `/mnt`, `/media`, and `/run/media`.

### Graceful Degradation for Path Resolution

`ResolveDosDevicesPath()` has a multi-level fallback chain:

1. Try resolving symlink via `FileSystemInfo.LinkTarget`
2. Try `ResolveLinkTarget(returnFinalTarget: true)`
3. Try host-side `readlink -f` via subprocess
4. Try `ResolveMountedHostPathByScanning()` (brute-force filesystem scan)
5. Return the original path unchanged

This pattern ensures the operation never fails -- it just returns the best path it can find.

## Shell Script Patterns

### Three-Script Architecture

The scripts form a chain of delegation:

1. **`steam-launch-helper.sh`** -- Full orchestrator: launches game via Steam, waits for process, then launches trainer. Accepts `--trainer-only` and `--game-only` flags for partial workflows.
2. **`steam-launch-trainer.sh`** -- Trainer-only launcher: uses `setsid env -i` to escape the WINE session, then delegates to the runner. This is the critical "escape hatch" script.
3. **`steam-host-trainer-runner.sh`** -- Detached runner: receives a clean environment, stages the trainer, runs `proton run`. Called by #2 in a detached session.

### Environment Cleanup (`setsid env -i` Pattern)

`steam-launch-trainer.sh` (lines 99-125) is the most architecturally significant script. It uses:

```bash
setsid env -i \
    HOME="${HOME:-}" \
    USER="${USER:-}" \
    LOGNAME="${LOGNAME:-}" \
    SHELL="${SHELL:-/bin/bash}" \
    PATH="/usr/bin:/bin" \
    DISPLAY="${DISPLAY:-}" \
    WAYLAND_DISPLAY="${WAYLAND_DISPLAY:-}" \
    XDG_RUNTIME_DIR="${XDG_RUNTIME_DIR:-}" \
    DBUS_SESSION_BUS_ADDRESS="${DBUS_SESSION_BUS_ADDRESS:-}" \
    /bin/bash "$runner_script" ...
```

- **`setsid`**: Creates a new session, detaching from the WINE process tree
- **`env -i`**: Completely wipes the environment (the nuclear option)
- **Explicit whitelist**: Only passes through ~9 variables needed for a Linux session
- **`</dev/null >/dev/null 2>&1 &`**: Full detachment -- no stdin, stdout, stderr, background
- The PID of the detached process is captured via `$!`

**Rust equivalent**: `std::process::Command` with `.env_clear()` and explicit `.env("KEY", "value")` calls. Use `setsid()` via the `nix` crate or `pre_exec` with `libc::setsid()`.

### Environment Variable Stripping (Redundant Safety)

Both the C# service (`SteamLaunchService.GetEnvironmentVariablesToClear()`) and the shell scripts (`unset WINESERVER WINELOADER ...`) independently strip the same ~30 WINE/Proton environment variables. This redundancy is intentional -- the C# side clears them from the ProcessStartInfo, and the shell side clears them again in case anything leaked through.

The variable list (28 variables across 6 categories):

1. **WINE core**: `WINESERVER`, `WINELOADER`, `WINEDLLPATH`, `WINEDLLOVERRIDES`, `WINEDEBUG`, `WINEESYNC`, `WINEFSYNC`, `WINELOADERNOEXEC`, `WINE_LARGE_ADDRESS_AWARE`, `WINE_DISABLE_KERNEL_WRITEWATCH`, `WINE_HEAP_DELAY_FREE`, `WINEFSYNC_SPINCOUNT`
2. **Linker**: `LD_PRELOAD`, `LD_LIBRARY_PATH`
3. **GStreamer**: `GST_PLUGIN_PATH`, `GST_PLUGIN_SYSTEM_PATH`, `GST_PLUGIN_SYSTEM_PATH_1_0`
4. **Steam/Game IDs**: `SteamGameId`, `SteamAppId`, `GAMEID`
5. **Proton flags**: `PROTON_LOG`, `PROTON_DUMP_DEBUG_COMMANDS`, `PROTON_USE_WINED3D`, `PROTON_NO_ESYNC`, `PROTON_NO_FSYNC`, `PROTON_ENABLE_NVAPI`
6. **DXVK/VKD3D**: `DXVK_CONFIG_FILE`, `DXVK_STATE_CACHE_PATH`, `DXVK_LOG_PATH`, `VKD3D_CONFIG`, `VKD3D_DEBUG`

### File Descriptor Cleanup

Both `steam-launch-helper.sh` (line 255) and `steam-host-trainer-runner.sh` (line 145) close all file descriptors > 2:

```bash
for fd in /proc/self/fd/*; do
  fd_num="$(basename "$fd")"
  if ((fd_num > 2)); then
    eval "exec ${fd_num}>&-" 2>/dev/null || true
  fi
done
```

This prevents CrossHook's wineserver file descriptors from being inherited by the Proton trainer process, which would cause the trainer to hang or fail.

**Rust equivalent**: Not needed when the native app runs outside WINE. The native app will launch processes directly without inherited WINE FDs.

### Trainer Staging Into Compatdata

`stage_trainer_into_compatdata()` copies the trainer executable into a predictable path within the game's compatdata prefix:

```
$compatdata/pfx/drive_c/CrossHook/StagedTrainers/$trainer_file_name
```

The Windows-style path becomes `C:\CrossHook\StagedTrainers\$trainer_file_name`. This staging is necessary because Proton needs the trainer to be accessible from within the WINE prefix.

### Process Detection via pgrep

```bash
linux_process_visible() {
  local process_name="$1"
  local process_name_without_extension="${process_name%.exe}"
  pgrep -af -- "$process_name" >/dev/null 2>&1 ||
  pgrep -af -- "$process_name_without_extension" >/dev/null 2>&1
}
```

Two `pgrep` checks: with `.exe` extension and without. WINE processes sometimes appear with or without the extension in `/proc`. The `-af` flags search full command lines.

**Rust equivalent**: Read `/proc/*/cmdline` or use `sysinfo` crate. Must handle the same with/without-extension ambiguity.

### Log Redirection and Streaming

All scripts redirect stdout+stderr to a log file early:

```bash
mkdir -p "$(dirname "$log_file")"
exec >>"$log_file" 2>&1
```

The C# `MainForm` then streams this log file to the console panel via `StreamSteamHelperLogAsync()` (a tail-follow pattern using `FileStream` with polling).

### PATH Safety (`ensure_standard_path`)

All scripts call `ensure_standard_path()` to guarantee `/usr/bin` and `/bin` are on `PATH`. Inside a WINE session, `PATH` may only contain WINE-internal directories, causing `pgrep`, `ps`, and other standard tools to fail.

## Testing Approach

No test framework is currently configured. However, the codebase is structured for testability:

- Services marked `internal` methods are testable from within the same assembly
- Static, stateless services with pure-function methods are trivially unit testable
- VDF parsing is isolated into `ParseKeyValueContent(string)` -- easy to test with string inputs
- Profile serialization round-trips are testable without filesystem via `StringReader`/`StringWriter`

For the Rust port:

- Use `#[cfg(test)]` modules co-located with each module
- VDF parser tests should use real `.vdf` fixtures from Steam installations
- Profile serialization tests should cover the legacy `Key=Value` import path
- Discovery logic should use `tempdir` crates with mock Steam directory structures
- Shell script invocation should be integration-tested on a real Linux system with Steam installed

## Patterns to Follow

### For crosshook-core (Rust Library)

1. **Module per domain**: `steam/discovery.rs`, `steam/launch.rs`, `steam/launcher_export.rs`, `profile/mod.rs`, `process/mod.rs`
2. **Request/Result structs**: Every public operation takes a `XxxRequest` and returns `Result<XxxResult, XxxError>`
3. **Diagnostic accumulation**: Pass `&mut Vec<Diagnostic>` or use a `DiagnosticCollector` for discovery operations
4. **Per-field state enums**: Port `SteamAutoPopulateFieldState` as `enum FieldState { NotFound, Found, Ambiguous }`
5. **Graceful degradation**: Discovery must never panic. Each step that can fail returns partial results.
6. **Path normalization**: Create a `normalize_path()` utility early. The native app does not need Windows path conversion but must handle symlinks, `..`, and repeated `/`
7. **Environment variable stripping**: Define the 28-variable list as a constant array. Apply it both in Rust process spawning and in any generated shell scripts.

### For Tauri IPC (Backend)

1. **Commands mirror services**: One Tauri `#[command]` per service operation (e.g., `auto_populate`, `validate_launch`, `execute_launch`, `save_profile`)
2. **Validation before execution**: Frontend calls `validate_*` first, shows errors, then calls `execute_*`
3. **Async operations**: Game/trainer launching should be async commands that emit progress events
4. **Log streaming**: Use Tauri events to push log lines from the helper log file to the frontend

### For React Frontend

1. **Two-phase launch state machine**: Model the launch flow as a state enum in React state management, not as scattered boolean flags
2. **Diagnostic display**: The `ManualHints` and `Diagnostics` lists from auto-populate should render as expandable sections
3. **Per-field status indicators**: Use the `FieldState` enum to show green/yellow/red indicators next to each auto-populated field (App ID, Compatdata, Proton)
4. **Profile form**: 12 fields from `ProfileData`, mapped to controlled form inputs. Boolean fields are checkboxes. Path fields have "Browse" buttons.

### Patterns to NOT Port

1. **`start.exe /unix` bridge**: The native app runs outside WINE and does not need the WINE bridge for launching scripts
2. **`winepath.exe` path conversion**: Not needed -- the native app works with native Linux paths
3. **`dosdevices/` symlink resolution**: Only needed for legacy profile import where paths were stored in Windows format
4. **Win32 P/Invoke (ProcessManager, InjectionManager, MemoryManager)**: Replaced by Linux-native equivalents (`/proc`, `ptrace`, `process_vm_readv`)
5. **Single-instance Mutex**: Replace with a Unix domain socket or PID file

## Edgecases

- Steam client install path detection tries `STEAM_COMPAT_CLIENT_INSTALL_PATH` env var first, then falls back to `~/.steam/root` -- the native app should additionally check `~/.local/share/Steam` and Flatpak paths
- `SteamAutoPopulateService.DiscoverSteamRootCandidates()` only checks Flatpak-style paths if they exist on disk -- the native app should proactively check `~/.var/app/com.valvesoftware.Steam/data/Steam`
- Profile name validation rejects characters invalid on Windows, even though the native app only targets Linux. This is necessary for cross-compatibility with the WinForms app.
- VDF parser uses case-insensitive dictionary keys (`StringComparer.OrdinalIgnoreCase`) -- Rust `HashMap` is case-sensitive by default, so the port must handle this explicitly
- `pgrep -af` searches full command lines, which can false-positive on similarly named processes. The scripts accept this risk rather than requiring exact PID tracking.
- `steam-launch-helper.sh` continues with trainer launch even if the game process is not confirmed after the startup delay (line 306-307) -- this is intentional to handle games that spawn under different process names
- The `EnumerateMountedBaseDirectories` scanner goes exactly 2 levels deep under `/mnt`, `/media`, etc. -- this handles typical Linux mount hierarchies like `/run/media/$USER/$DISK`

## Other Docs

- `docs/plans/platform-native-ui/feature-spec.md`: Full feature specification with architecture, data models, and phasing
- `docs/plans/platform-native-ui/research-business.md`: User stories, business rules, domain model analysis
- `docs/plans/platform-native-ui/research-technical.md`: Win32-to-Linux API mapping, Rust trait APIs, packaging strategy
- `docs/plans/platform-native-ui/research-external.md`: Steam APIs, Linux process APIs, VDF parsers, distribution strategies
- `docs/plans/platform-native-ui/research-ux.md`: UI patterns, competitive analysis, Steam Deck UX
- `docs/plans/platform-native-ui/research-recommendations.md`: Framework comparison, phasing, risk assessment
- [Valve Developer Community - VDF Format](https://developer.valvesoftware.com/wiki/KeyValues): Official VDF specification
- [Tauri v2 IPC Guide](https://v2.tauri.app/develop/calling-rust/): Tauri command/event patterns for Rust-to-frontend communication
