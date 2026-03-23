# Context Analysis: platform-native-ui

## Executive Summary

CrossHook's native UI replaces the C#/WinForms application (which runs inside WINE) with a Tauri v2 application (Rust backend + React/TypeScript frontend via Vite) that runs natively on Linux, macOS, and Windows. The core architectural insight is that the native app is a **thin orchestration frontend** -- it manages profiles, discovers Steam libraries, and delegates actual game/trainer launching to three existing, proven shell scripts via `std::process::Command`. A shared `crosshook-core` Rust library crate contains all ported domain logic, consumed by both the Tauri UI and a headless `crosshook-cli` binary.

## Architecture Context

- **System Structure**: Cargo workspace at `src/crosshook-native/` with three crates: `crosshook-core` (library -- profiles, Steam discovery, launch orchestration, launcher export), `crosshook-cli` (binary -- headless launcher), and `src-tauri/` (Tauri binary -- IPC commands wrapping core). React frontend lives alongside in `src/` with Vite. The existing WinForms app at `src/CrossHookEngine.App/` is frozen at its current feature set.
- **Data Flow**: User configures profile in React UI --> Tauri IPC command validates via `crosshook-core` --> core builds CLI arguments --> `std::process::Command` invokes `steam-launch-helper.sh` (or `steam-launch-trainer.sh`) --> script launches game via `steam -applaunch`, stages trainer into compatdata, strips ~28 WINE env vars, runs `proton run <trainer>` via `setsid` --> log file is tailed by core and streamed to React via Tauri events.
- **Integration Points**: (1) Shell scripts bundled and invoked directly -- no WINE bridge needed. (2) VDF/ACF files parsed natively for Steam discovery. (3) Profile files read/written in both legacy `.profile` Key=Value format and new TOML format. (4) XDG-compliant paths (`~/.config/crosshook/`, `~/.local/share/crosshook/`). (5) `.desktop` entries and standalone `.sh` launcher scripts generated for desktop integration.

## Critical Files Reference

- `src/CrossHookEngine.App/Services/SteamAutoPopulateService.cs`: Most complex service to port (~1286 lines) -- VDF parsing, library discovery, manifest matching, Proton resolution with fuzzy alias matching. Contains the custom recursive-descent VDF parser.
- `src/CrossHookEngine.App/Services/SteamLaunchService.cs`: Launch request validation, canonical list of ~28 WINE env vars to strip, script argument construction (~737 lines). Path conversion logic is NOT needed natively (only for legacy `.profile` import).
- `src/CrossHookEngine.App/Services/SteamExternalLauncherExportService.cs`: Generates `.sh` trainer launchers and `.desktop` entries (~350 lines). Straightforward port.
- `src/CrossHookEngine.App/Services/ProfileService.cs`: Profile CRUD with 12-field Key=Value format, profile name validation (~225 lines). The format is the cross-app compatibility contract.
- `src/CrossHookEngine.App/Services/AppSettingsService.cs`: Settings persistence (AutoLoadLastProfile, LastUsedProfile) -- trivial port (~81 lines).
- `src/CrossHookEngine.App/Services/RecentFilesService.cs`: MRU paths with INI-style sections (~131 lines).
- `src/CrossHookEngine.App/Services/CommandLineParser.cs`: `-p <profile>` and `-autolaunch` parsing (~54 lines). Native CLI should support same contract.
- `src/CrossHookEngine.App/runtime-helpers/steam-launch-helper.sh`: Full game+trainer orchestrator (~326 lines) -- direct reuse, no porting.
- `src/CrossHookEngine.App/runtime-helpers/steam-launch-trainer.sh`: Trainer-only launcher via `setsid env -i` escape (~128 lines) -- direct reuse.
- `src/CrossHookEngine.App/runtime-helpers/steam-host-trainer-runner.sh`: Clean-env trainer runner (~178 lines) -- called internally by `steam-launch-trainer.sh`.
- `src/CrossHookEngine.App/Forms/MainForm.cs` (lines 2648-2946): `BuildSteamLaunchRequest`, `LaunchSteamModeAsync`, `RunSteamLaunchHelper`, `StreamSteamHelperLogAsync` -- defines the launch workflow to replicate.
- `src/CrossHookEngine.App/Forms/MainFormStartupCoordinator.cs`: Auto-load profile resolution logic at startup.
- `docs/plans/platform-native-ui/feature-spec.md`: Master specification -- architecture, data models, Rust traits, phased plan, success criteria.
- `tasks/lessons.md`: Critical runtime gotchas from live debugging (WINE paths, dosdevices, env var stripping, path normalization pitfalls).

## Patterns to Follow

- **Request/Result (Validate-Then-Execute)**: Every service uses Request DTO -> ValidationResult -> ExecutionResult. Services validate before executing and return structured results rather than throwing. Rust equivalent: `struct XxxRequest`, `fn validate() -> Result<(), ValidationError>`, `fn execute() -> Result<XxxResult, XxxError>`. See `SteamLaunchService.cs` lines 10-57.
- **Static Service Layer (No DI)**: Stateless services are `static class` with only static methods. Stateful services hold only a `base_path`. Rust: module-level functions for stateless, structs with `base_path: PathBuf` for stateful. See `SteamAutoPopulateService.cs`.
- **Diagnostic Accumulation**: `AttemptAutoPopulate` threads two `Vec<String>` (diagnostics, manualHints) through every helper. Non-failing -- always returns partial results. Rust: `DiagnosticCollector` struct or `&mut Vec<Diagnostic>`. See `SteamAutoPopulateService.cs` lines 612-613.
- **Two-Phase Launch State Machine**: Phase 1 launches game (`LaunchGameOnly=true`), Phase 2 launches trainer (`LaunchTrainerOnly=true`). Track as `enum LaunchPhase { Idle, GameLaunching, WaitingForTrainer, TrainerLaunching, SessionActive }` -- not scattered booleans. See `MainForm.cs` line 2119.
- **Three-State Field Resolution**: `SteamAutoPopulateFieldState` enum (`NotFound`, `Found`, `Ambiguous`) per resolved field. Prevents silent guessing. Map to Rust enum.
- **setsid env -i Environment Escape**: Shell scripts use `setsid env -i` to create a fully detached process with a clean environment, escaping any WINE session. Rust equivalent: `Command::new().env_clear()` with explicit `.env()` calls, plus `pre_exec` with `libc::setsid()`.
- **Script-as-Boundary**: Shell scripts form a clean boundary between app and OS. The app constructs arguments and environment; the script handles process lifecycle. Preserve this in Phase 1-2.
- **Event-Driven Communication**: C# components use `EventHandler<T>` events. Tauri equivalent: IPC events via `app.emit()` for backend-to-frontend (log streaming, process status), and `#[tauri::command]` for frontend-to-backend calls.

## Cross-Cutting Concerns

- **Environment Variable Stripping**: 28 WINE/Proton vars must be stripped before trainer launch. Canonical list exists in both `SteamLaunchService.GetEnvironmentVariablesToClear()` and the shell scripts. The shell scripts are the authoritative runtime source. The native app should rely primarily on shell script stripping since it launches from a clean Linux environment, but must still define the list as a Rust constant for generated launcher scripts.
- **VDF Parser Case Sensitivity**: The C# parser uses case-insensitive dictionary keys (`StringComparer.OrdinalIgnoreCase`). Rust `HashMap` is case-sensitive by default. The `steam-vdf-parser` crate (or alternative `keyvalues-parser`) must be validated for case-insensitive key behavior, or a wrapper must normalize keys.
- **Legacy Profile Path Handling**: Legacy `.profile` files may contain Windows-style paths (`Z:\mnt\games\...`). The native app must detect and convert `Z:\` prefix paths during import. Most path conversion code from `SteamLaunchService` is NOT needed for new native profiles.
- **Trainer Staging**: Before `proton run`, trainer `.exe` is copied to `$compatdata/pfx/drive_c/CrossHook/StagedTrainers/<filename>`. The Windows path `C:\CrossHook\StagedTrainers\<filename>` is passed to `proton run`. Both shell scripts implement this independently.
- **Profile Name Validation**: Must reject characters invalid on both Windows AND Linux for cross-compatibility. Checks: empty/whitespace, `.`/`..`, rooted paths, path separators, Windows reserved characters (`<>:"/\|?*`).
- **Distribution**: AppImage is the primary distribution format (no sandbox restrictions on `/proc`/`ptrace`, single file, survives SteamOS updates). NOT Flatpak -- it blocks `/proc` and `ptrace` access. AUR PKGBUILD as secondary.
- **Testing**: No test framework exists in the C# codebase. The Rust port must include tests from the start: `#[cfg(test)]` modules, VDF parser tests with real `.vdf` fixtures, profile serialization round-trips, discovery logic with `tempdir` mock Steam directories.
- **Flatpak Steam Detection**: Current C# code does NOT check `~/.var/app/com.valvesoftware.Steam/data/Steam`. The native app must add this as a Steam root candidate.
- **Log Streaming**: Helper scripts redirect stdout+stderr to a log file (`exec >>"$log_file" 2>&1`). The app tails this file and pushes lines to the UI. Rust: `tokio::fs::File` with periodic reads or `inotify`, pushed to React via Tauri events.

## Parallelization Opportunities

- **Phase 1 parallel tracks**: (A) Rust data models + profile reader/writer + CLI scaffolding can proceed independently from (B) Tauri project init + React UI scaffolding + component stubs.
- **Phase 1 within backend**: Profile service, settings service, and launch wrapper are independent modules that can be developed in parallel.
- **Phase 2 parallel tracks**: (A) VDF parser + Steam library discovery + manifest matching + Proton resolution is independent from (B) launcher export (`.sh` + `.desktop` generation).
- **Frontend/backend split**: React component development (profile form, launch flow, console panel) can proceed with mock data while Rust backend services are built.
- **Shared coordination points**: The `ProfileData` struct and Tauri IPC command signatures must be agreed upon early -- they are the contract between frontend and backend. The `LaunchPhase` enum state machine must be designed collaboratively.

## Implementation Constraints

- **Technical Constraints**:
  - Target `net9.0-windows` codebase runs under WINE -- path conversion logic (`ConvertToUnixPath`, `dosdevices` resolution) is WINE-specific and NOT needed in the native app except for legacy `.profile` import.
  - Shell scripts use POSIX utilities (`pgrep`, `setsid`, `readlink`, `realpath`, `basename`, `cp`, `mkdir`, `env`, `ps`). These are available on all target platforms (SteamOS, desktop Linux, macOS via Homebrew).
  - Proton tool name matching uses fuzzy heuristics (exact, normalized, substring). Handles `proton_9` vs `Proton 9.0` vs `proton-9.0-4`.
  - `libraryfolders.vdf` has two format generations (old flat `"0" "/path"` vs new nested `"0" { "path" "/path" }`). Parser must handle both.
  - PR #18 review identified a shell script exit code capture bug in `steam-host-trainer-runner.sh` and `steam-launch-helper.sh` -- documented but not confirmed fixed.
  - WebKitGTK is pre-installed on SteamOS (Steam Deck target). Tauri uses system WebView.
  - Memory footprint target: < 100MB idle (< 50MB target for Tauri). Critical since app runs alongside games.
- **Business Constraints**:
  - WinForms app frozen at current feature set -- no new features, but maintained for legacy WINE-hosted use cases.
  - Profile format compatibility: native app MUST read legacy `.profile` files saved by WinForms app.
  - Two-phase launch must be enforced by UI -- cannot combine game + trainer into single action.
  - DLL injection is NOT supported in Steam mode and is NOT part of the native app MVP. Only `proton run` trainer workflow.
  - No test framework exists -- Rust implementation must establish testing from day one.

## Key Recommendations

- **Phase organization**: Phase 1 (MVP, 4-6 weeks) = profile-driven launcher invoking shell scripts. Phase 2 (Smart Discovery, 3-4 weeks) = port `SteamAutoPopulateService` VDF parsing + launcher export. Phase 3 (Polish, 3-4 weeks) = settings, controller navigation, system tray, AppImage packaging. Phase 4 (Community, 4-6 weeks) = community profiles, Git-based sharing.
- **Start with CLI**: Build `crosshook-cli` first (1-2 days) as a profile reader that invokes `steam-launch-helper.sh`. This validates the core launch pipeline before any UI work, provides an immediate headless tool for Steam Deck Gaming Mode, and forces clean separation between core logic and UI.
- **Trait-based design**: Define `SteamDiscovery`, `ProfileStore`, `SteamLauncher`, `ProcessControl` traits in `crosshook-core` per the feature spec. This enables mock implementations for testing and future platform abstraction (macOS, Windows).
- **VDF crate evaluation is blocking**: The `steam-vdf-parser` crate must be validated against the five VDF file types (`libraryfolders.vdf`, `appmanifest_*.acf`, `config.vdf`, `localconfig.vdf`, `compatibilitytool.vdf`) and the custom parser's case-insensitive key behavior before committing to Phase 2.
- **Critical path**: Project scaffolding --> core data models --> CLI launch wrapper --> Tauri IPC commands --> React profile form --> two-phase launch flow --> MVP release --> VDF parser --> auto-discovery --> launcher export --> competitive release.
- **Minimum viable release**: End of Phase 1 (profile editor + script-based launcher). Users enter paths manually.
- **Competitive release**: End of Phase 2 (auto-discovery matches WinForms feature parity on Steam workflow).
- **~45-55 discrete tasks** across all four phases. Task breakdown should split along the parallel tracks identified above.
