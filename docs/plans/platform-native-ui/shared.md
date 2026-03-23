# Platform-Native UI

CrossHook's native UI is a Tauri v2 application (Rust backend + React/TypeScript frontend via Vite) that replaces the C#/WinForms app for Linux, macOS, and Windows. The Rust backend wraps a `crosshook-core` library crate containing ported domain logic from the existing Services layer — primarily Steam library discovery (`SteamAutoPopulateService.cs`, 1286 lines of VDF parsing and manifest matching), launch orchestration (`SteamLaunchService.cs`, 737 lines of request validation and environment cleanup), profile management (`ProfileService.cs`, 12-field key=value format), and launcher export (`SteamExternalLauncherExportService.cs`, .sh + .desktop generation). The three runtime-helper shell scripts (`steam-launch-helper.sh`, `steam-launch-trainer.sh`, `steam-host-trainer-runner.sh`) are reused directly — they already run natively on Linux and handle the actual `proton run` trainer execution with clean environment isolation via `setsid env -i`.

## Relevant Files

- src/CrossHookEngine.App/Services/SteamAutoPopulateService.cs: Steam library discovery, VDF/ACF parsing, manifest matching, Proton resolution — most complex service to port (~1286 lines)
- src/CrossHookEngine.App/Services/SteamLaunchService.cs: Launch request validation, path conversion, ~30 WINE env vars to strip, script argument construction (~737 lines)
- src/CrossHookEngine.App/Services/SteamExternalLauncherExportService.cs: Generates standalone .sh launcher scripts and .desktop entry files (~350 lines)
- src/CrossHookEngine.App/Services/ProfileService.cs: Profile CRUD with 12-field Key=Value format, profile name validation (~225 lines)
- src/CrossHookEngine.App/Services/AppSettingsService.cs: AutoLoadLastProfile and LastUsedProfile settings persistence (~81 lines)
- src/CrossHookEngine.App/Services/RecentFilesService.cs: MRU paths with INI-style section format (~131 lines)
- src/CrossHookEngine.App/Services/CommandLineParser.cs: CLI args -p and -autolaunch parsing (~54 lines)
- src/CrossHookEngine.App/runtime-helpers/steam-launch-helper.sh: Full game+trainer orchestrator — direct reuse, no porting needed (~326 lines)
- src/CrossHookEngine.App/runtime-helpers/steam-launch-trainer.sh: Trainer-only launcher via setsid env -i escape (~128 lines)
- src/CrossHookEngine.App/runtime-helpers/steam-host-trainer-runner.sh: Clean-env trainer runner, stages into compatdata, runs proton (~178 lines)
- src/CrossHookEngine.App/Forms/MainForm.cs: Lines 2648-2946 contain BuildSteamLaunchRequest, LaunchSteamModeAsync, RunSteamLaunchHelper — defines the launch workflow the native app must replicate
- src/CrossHookEngine.App/Forms/MainFormStartupCoordinator.cs: Auto-load profile resolution logic at startup
- src/CrossHookEngine.App/Core/ProcessManager.cs: Win32 process lifecycle — NOT ported for MVP, but defines event-driven communication patterns
- src/CrossHookEngine.App/Diagnostics/AppDiagnostics.cs: Trace-based logging pattern (~127 lines)
- src/CrossHookEngine.App/Program.cs: Single-instance enforcement via named Mutex
- docs/plans/platform-native-ui/feature-spec.md: Master specification — architecture, data models, Rust traits, project structure, phased plan
- tasks/lessons.md: Critical runtime gotchas from live debugging (WINE paths, dosdevices, env var stripping)

## Relevant Patterns

**Request/Result Pattern (Validate-Then-Execute)**: Every service uses Request DTO → ValidationResult → ExecutionResult. Services validate before executing and return structured results rather than throwing. See [src/CrossHookEngine.App/Services/SteamLaunchService.cs](src/CrossHookEngine.App/Services/SteamLaunchService.cs) lines 10-57 for `SteamLaunchRequest`/`SteamLaunchValidationResult`/`SteamLaunchExecutionResult`. Rust equivalent: `struct XxxRequest`, `fn validate() -> Result<(), ValidationError>`, `fn execute() -> Result<XxxResult, XxxError>`.

**Static Service Layer (No DI)**: Steam-related services are `static class` with only static methods and no state. Stateful services (ProfileService, AppSettingsService) hold only a `base_path`. See [src/CrossHookEngine.App/Services/SteamAutoPopulateService.cs](src/CrossHookEngine.App/Services/SteamAutoPopulateService.cs). Rust equivalent: module-level functions for stateless, structs with `base_path: PathBuf` for stateful.

**Diagnostic Accumulation**: `SteamAutoPopulateService.AttemptAutoPopulate` threads two `List<string>` (diagnostics, manualHints) through every helper method. Non-failing — always returns a result even when sub-steps fail. See [src/CrossHookEngine.App/Services/SteamAutoPopulateService.cs](src/CrossHookEngine.App/Services/SteamAutoPopulateService.cs) line 612-613. Rust equivalent: `DiagnosticCollector` struct or `Vec<Diagnostic>` passed through pipeline.

**Two-Phase Launch State Machine**: MainForm tracks `_steamTrainerLaunchPending` boolean — Phase 1 launches game (`LaunchGameOnly=true`), Phase 2 launches trainer (`LaunchTrainerOnly=true`). See [src/CrossHookEngine.App/Forms/MainForm.cs](src/CrossHookEngine.App/Forms/MainForm.cs) line 2119 `UpdateSteamModeUiState()`. Rust/React equivalent: `enum LaunchPhase { Idle, GameLaunching, WaitingForTrainer, TrainerLaunching, SessionActive }`.

**VDF Key-Value Parser**: Custom recursive-descent parser handling quoted strings with escapes, unquoted tokens, nested `{}` blocks, `//` comments, case-insensitive dictionary keys. See [src/CrossHookEngine.App/Services/SteamAutoPopulateService.cs](src/CrossHookEngine.App/Services/SteamAutoPopulateService.cs) lines 739-863. Rust alternative: `steam-vdf-parser` crate (validate its case-insensitive key behavior).

**setsid env -i Environment Escape**: Shell scripts use `setsid env -i bash -c '...'` to create a fully detached process with a clean environment, escaping the WINE session. See [src/CrossHookEngine.App/runtime-helpers/steam-launch-trainer.sh](src/CrossHookEngine.App/runtime-helpers/steam-launch-trainer.sh). Critical for trainer isolation.

**Profile Key=Value Serialization**: 12 fields, no quoting, no escaping, splits on first `=`. Boolean values via `bool.TryParse`. Order matches write order. See [src/CrossHookEngine.App/Services/ProfileService.cs](src/CrossHookEngine.App/Services/ProfileService.cs).

## Relevant Docs

**docs/plans/platform-native-ui/feature-spec.md**: You _must_ read this when implementing any part of the native UI. Contains the architecture diagram, TOML data model, Rust trait APIs, Win32→Linux mapping table, CLI interface, project directory structure, and 4-phase task breakdown.

**docs/plans/platform-native-ui/research-business.md**: You _must_ read this when implementing Steam workflows, profile management, or launch orchestration. Contains 7 business rules, edge cases, domain model with 9 entities, state transitions, and the full launch workflow specification.

**docs/features/steam-proton-trainer-launch.doc.md**: You _must_ read this when implementing the two-phase launch flow or launcher export. Defines the user-facing workflow, generated launcher format, and current limitations.

**tasks/lessons.md**: You _must_ read this when implementing Steam/Proton integration in Rust. Contains critical runtime gotchas from live debugging: WINE path handling, dosdevices resolution, env var stripping requirements.

**CLAUDE.md**: You _must_ read this when contributing to the project. Defines code conventions, build commands, git workflow, and label taxonomy.

**research/crosshook-feature-enhancements/report.md**: Reference when planning Phase 3-4 features. Contains the "Dual Cockpit" architecture vision and community profile sharing strategy (highest-leverage feature per 6/8 research personas).
