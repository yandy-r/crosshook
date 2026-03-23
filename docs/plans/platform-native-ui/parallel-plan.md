# Platform-Native UI Implementation Plan

CrossHook's native UI is a Tauri v2 application (Rust backend + React/TypeScript frontend via Vite) that replaces the C#/WinForms app running under WINE. The Rust backend wraps a `crosshook-core` library crate porting domain logic from the existing Services layer — Steam discovery, launch orchestration, profile management — while the three `runtime-helpers/*.sh` shell scripts are bundled and invoked directly via `std::process::Command`. The project lives at `src/crosshook-native/` as a Cargo workspace with `crosshook-core` (lib), `crosshook-cli` (bin), and `src-tauri/` (Tauri app). Phase 1 delivers a working profile-driven launcher; Phase 2 adds Steam auto-discovery for feature parity.

## Critically Relevant Files and Documentation

- docs/plans/platform-native-ui/feature-spec.md: Master specification — architecture diagram, TOML data model, Rust trait APIs, CLI interface, 4-phase task breakdown, technology decisions
- docs/plans/platform-native-ui/research-business.md: 7 business rules, edge cases, domain model with 9 entities, launch workflow specification
- docs/plans/platform-native-ui/analysis-code.md: Complete field lists for all data types, Tauri IPC command mapping table, shell script CLI interface, 9 implementation patterns
- docs/plans/platform-native-ui/analysis-tasks.md: 48-task breakdown with file-to-task mapping, parallelization groups, dependency analysis
- docs/features/steam-proton-trainer-launch.doc.md: User-facing two-phase launch workflow that the native app must replicate
- tasks/lessons.md: Critical runtime gotchas — WINE path handling, dosdevices, env var stripping pitfalls
- src/CrossHookEngine.App/Services/SteamAutoPopulateService.cs: Most complex service to port (~1286 lines) — VDF parsing, library discovery, manifest matching, Proton resolution
- src/CrossHookEngine.App/Services/SteamLaunchService.cs: Launch request validation, canonical env var cleanup list, script argument construction (~737 lines)
- src/CrossHookEngine.App/Services/ProfileService.cs: Profile CRUD with 12-field Key=Value format (~225 lines)
- src/CrossHookEngine.App/Services/SteamExternalLauncherExportService.cs: Launcher script + .desktop generation (~350 lines)
- src/CrossHookEngine.App/runtime-helpers/steam-launch-helper.sh: Full game+trainer orchestrator — reused directly (~326 lines)
- src/CrossHookEngine.App/runtime-helpers/steam-launch-trainer.sh: Trainer-only launcher via setsid env -i (~128 lines)
- src/CrossHookEngine.App/runtime-helpers/steam-host-trainer-runner.sh: Clean-env trainer runner (~178 lines)
- src/CrossHookEngine.App/Forms/MainForm.cs: Lines 2648-2946 — launch orchestration workflow to replicate
- CLAUDE.md: Project conventions, build commands, code patterns, git workflow

## Implementation Plan

### Phase 1: Foundation & MVP

#### Task 1.1: Rust workspace scaffolding

Depends on [none]

**READ THESE BEFORE TASK**

- docs/plans/platform-native-ui/feature-spec.md (System Integration / Files to Create section)
- CLAUDE.md

**Instructions**

Files to Create

- src/crosshook-native/Cargo.toml
- src/crosshook-native/crates/crosshook-core/Cargo.toml
- src/crosshook-native/crates/crosshook-core/src/lib.rs
- src/crosshook-native/crates/crosshook-cli/Cargo.toml
- src/crosshook-native/crates/crosshook-cli/src/main.rs

Initialize a Cargo workspace at `src/crosshook-native/` with two member crates: `crosshook-core` (library) and `crosshook-cli` (binary depending on crosshook-core). Add initial dependencies to `crosshook-core/Cargo.toml`: `serde` (with derive), `toml`, `tokio` (with fs,process,sync features), `tracing`. Add `clap` (with derive) to `crosshook-cli`. The `lib.rs` should declare public modules: `profile`, `launch`, `steam`, `export`, `settings`. Each module starts as an empty `mod.rs` with a TODO comment. The `main.rs` should be a minimal clap skeleton that prints version info.

#### Task 1.2: React/Vite/Tauri scaffolding

Depends on [none]

**READ THESE BEFORE TASK**

- docs/plans/platform-native-ui/feature-spec.md (Technology Decisions section)

**Instructions**

Files to Create

- src/crosshook-native/package.json
- src/crosshook-native/vite.config.ts
- src/crosshook-native/tsconfig.json
- src/crosshook-native/src/App.tsx
- src/crosshook-native/src/main.tsx
- src/crosshook-native/src-tauri/Cargo.toml
- src/crosshook-native/src-tauri/src/lib.rs
- src/crosshook-native/src-tauri/tauri.conf.json

Run `cargo create-tauri-app` or manually scaffold Tauri v2 with React + TypeScript + Vite. Configure `tauri.conf.json` with app identifier `com.crosshook.native`, window title "CrossHook", default dark theme. The `src-tauri/Cargo.toml` should depend on `crosshook-core` (path = `../crates/crosshook-core`), `tauri` (v2), `tauri-plugin-shell`, `tauri-plugin-dialog`, `tauri-plugin-fs`, `serde_json`. The `src-tauri/src/lib.rs` should register empty command handlers and plugins. The React `App.tsx` should render a placeholder "CrossHook Native" header.

#### Task 1.3: ProfileData model (Rust)

Depends on [1.1]

**READ THESE BEFORE TASK**

- src/CrossHookEngine.App/Services/ProfileService.cs (lines 199-224 for ProfileData class)
- docs/plans/platform-native-ui/analysis-code.md (Data Types section)

**Instructions**

Files to Create

- src/crosshook-native/crates/crosshook-core/src/profile/mod.rs
- src/crosshook-native/crates/crosshook-core/src/profile/models.rs

Define two structs: `LegacyProfileData` matching the exact 12-field `.profile` format (GamePath, TrainerPath, Dll1Path, Dll2Path, LaunchInject1, LaunchInject2, LaunchMethod, UseSteamMode, SteamAppId, SteamCompatDataPath, SteamProtonPath, SteamLauncherIconPath — all String except booleans), and `GameProfile` as the TOML-native struct with nested `[game]`, `[trainer]`, `[injection]`, `[steam]`, `[launch]` sections per the feature spec. Include `serde` Serialize/Deserialize derives. Add a `From<LegacyProfileData> for GameProfile` conversion.

#### Task 1.4: ProfileData model (TypeScript)

Depends on [1.2]

**READ THESE BEFORE TASK**

- docs/plans/platform-native-ui/analysis-code.md (Data Types / React State Needed sections)

**Instructions**

Files to Create

- src/crosshook-native/src/types/profile.ts
- src/crosshook-native/src/types/launch.ts
- src/crosshook-native/src/types/index.ts

Define TypeScript interfaces mirroring the Rust structs: `ProfileData` (12 fields), `GameProfile` (TOML structure), `LaunchPhase` enum (`Idle | GameLaunching | WaitingForTrainer | TrainerLaunching | SessionActive`), `SteamLaunchRequest`, `ValidationResult`, `LaunchResult`. Export from `index.ts`.

#### Task 1.5: Legacy profile reader/writer

Depends on [1.3]

**READ THESE BEFORE TASK**

- src/CrossHookEngine.App/Services/ProfileService.cs

**Instructions**

Files to Create

- src/crosshook-native/crates/crosshook-core/src/profile/legacy.rs

Port `ProfileService.cs` (225 lines). Implement: `fn load(profiles_dir: &Path, name: &str) -> Result<LegacyProfileData>` (read file line by line, split on first `=`, parse bools case-insensitively), `fn save(profiles_dir: &Path, name: &str, data: &LegacyProfileData) -> Result<()>` (write 12 Key=Value lines), `fn list(profiles_dir: &Path) -> Result<Vec<String>>` (enumerate `.profile` files), `fn delete(profiles_dir: &Path, name: &str) -> Result<()>`, `fn validate_name(name: &str) -> Result<()>` (reject empty, `.`/`..`, path separators, Windows reserved chars). Include `#[cfg(test)]` module with round-trip serialization tests. Handle legacy Windows paths (`Z:\...`) by detecting and converting the `Z:` prefix during load.

#### Task 1.6: TOML profile reader/writer

Depends on [1.3]

**READ THESE BEFORE TASK**

- docs/plans/platform-native-ui/feature-spec.md (Data Models / Game Profile section)

**Instructions**

Files to Create

- src/crosshook-native/crates/crosshook-core/src/profile/toml_store.rs

Implement `ProfileStore` struct with `base_path: PathBuf` (defaults to `~/.config/crosshook/profiles/` via `directories` crate). Methods: `fn load(&self, name: &str) -> Result<GameProfile>` (read TOML via `toml::from_str`), `fn save(&self, name: &str, profile: &GameProfile) -> Result<()>` (write via `toml::to_string_pretty`), `fn list(&self) -> Result<Vec<String>>` (enumerate `.toml` files), `fn delete(&self, name: &str) -> Result<()>`, `fn import_legacy(&self, legacy_path: &Path) -> Result<GameProfile>` (read `.profile`, convert via `From` impl, save as TOML). Include tests.

#### Task 1.7: Shell script invocation wrapper

Depends on [1.1]

**READ THESE BEFORE TASK**

- src/CrossHookEngine.App/Services/SteamLaunchService.cs (lines 139-196 CreateHelperStartInfo, lines 198-231 CreateTrainerStartInfo)
- src/CrossHookEngine.App/runtime-helpers/steam-launch-helper.sh (lines 1-40 for argument format)
- src/CrossHookEngine.App/runtime-helpers/steam-launch-trainer.sh (lines 1-20 for argument format)
- docs/plans/platform-native-ui/analysis-code.md (Pattern 5: Shell Script CLI Interface)

**Instructions**

Files to Create

- src/crosshook-native/crates/crosshook-core/src/launch/mod.rs
- src/crosshook-native/crates/crosshook-core/src/launch/script_runner.rs

Build `Command` instances for the two shell scripts. `fn build_helper_command(request: &SteamLaunchRequest, script_path: &Path, log_path: &Path) -> Command` — constructs `Command::new("/bin/bash").arg(script_path)` with `--appid`, `--compatdata`, `--proton`, `--steam-client`, `--game-exe-name`, `--trainer-path`, `--trainer-host-path`, `--log-file`, plus `--game-only` or `--trainer-only` flags based on launch phase. `fn build_trainer_command(request: &SteamLaunchRequest, script_path: &Path, log_path: &Path) -> Command` — same pattern for `steam-launch-trainer.sh`. Both must call `.env_clear()` then set only `HOME`, `USER`, `PATH`, `DISPLAY`, `WAYLAND_DISPLAY`, `XDG_RUNTIME_DIR`, `DBUS_SESSION_BUS_ADDRESS`, `STEAM_COMPAT_DATA_PATH`, `STEAM_COMPAT_CLIENT_INSTALL_PATH`, `WINEPREFIX`. Use `tokio::process::Command` for async spawning.

#### Task 1.8: Launch request validation

Depends on [1.7]

**READ THESE BEFORE TASK**

- src/CrossHookEngine.App/Services/SteamLaunchService.cs (lines 10-57 for request/result types, lines 69-109 for Validate)

**Instructions**

Files to Create

- src/crosshook-native/crates/crosshook-core/src/launch/request.rs

Define `SteamLaunchRequest` struct (GamePath, TrainerPath, TrainerHostPath, SteamAppId, SteamCompatDataPath, SteamProtonPath, SteamClientInstallPath: String; LaunchTrainerOnly, LaunchGameOnly: bool). Implement `fn validate(request: &SteamLaunchRequest) -> Result<(), ValidationError>` checking: required fields non-empty (GamePath only required when !LaunchTrainerOnly), SteamCompatDataPath directory exists, SteamProtonPath file exists and is executable, TrainerHostPath file exists. Return `ValidationError` enum with per-field variants. Include tests for each validation case.

#### Task 1.9: Environment cleanup constants

Depends on [1.7]

**READ THESE BEFORE TASK**

- src/CrossHookEngine.App/Services/SteamLaunchService.cs (lines 233-269 GetEnvironmentVariablesToClear)
- docs/plans/platform-native-ui/analysis-code.md (Pattern 3: Environment Variable Cleanup)

**Instructions**

Files to Create

- src/crosshook-native/crates/crosshook-core/src/launch/env.rs

Define `const WINE_ENV_VARS_TO_CLEAR: &[&str]` with all 30 variables (WINESERVER, WINELOADER, WINEDLLPATH, WINEDLLOVERRIDES, WINEDEBUG, WINEESYNC, WINEFSYNC, WINELOADERNOEXEC, WINE_LARGE_ADDRESS_AWARE, WINE_DISABLE_KERNEL_WRITEWATCH, WINE_HEAP_DELAY_FREE, WINEFSYNC_SPINCOUNT, LD_PRELOAD, LD_LIBRARY_PATH, GST_PLUGIN_PATH, GST_PLUGIN_SYSTEM_PATH, GST_PLUGIN_SYSTEM_PATH_1_0, SteamGameId, SteamAppId, GAMEID, PROTON_LOG, PROTON_DUMP_DEBUG_COMMANDS, PROTON_USE_WINED3D, PROTON_NO_ESYNC, PROTON_NO_FSYNC, PROTON_ENABLE_NVAPI, DXVK_CONFIG_FILE, DXVK_STATE_CACHE_PATH, DXVK_LOG_PATH, VKD3D_CONFIG, VKD3D_DEBUG). Also define `const REQUIRED_PROTON_VARS: &[&str]` = `["STEAM_COMPAT_DATA_PATH", "STEAM_COMPAT_CLIENT_INSTALL_PATH", "WINEPREFIX"]` and `const PASSTHROUGH_DISPLAY_VARS: &[&str]` for display server variables.

#### Task 1.10: Tauri IPC commands (profile)

Depends on [1.5, 1.6]

**READ THESE BEFORE TASK**

- docs/plans/platform-native-ui/analysis-code.md (Integration Points / Tauri IPC Commands Needed)

**Instructions**

Files to Create

- src/crosshook-native/src-tauri/src/commands/mod.rs
- src/crosshook-native/src-tauri/src/commands/profile.rs

Files to Modify

- src/crosshook-native/src-tauri/src/lib.rs

Implement `#[tauri::command]` functions: `profile_list() -> Vec<String>`, `profile_load(name: String) -> GameProfile`, `profile_save(name: String, data: GameProfile)`, `profile_delete(name: String)`, `profile_import_legacy(path: String) -> GameProfile`. Each wraps the corresponding `crosshook-core` function. Register all commands in `lib.rs` via `invoke_handler(tauri::generate_handler![...])`. Use `tauri::State<ProfileStore>` for the store instance initialized in `setup()`.

#### Task 1.11: Tauri IPC commands (launch)

Depends on [1.7, 1.8, 1.9]

**READ THESE BEFORE TASK**

- src/CrossHookEngine.App/Forms/MainForm.cs (lines 2767-2828 LaunchSteamModeAsync, lines 2908-2946 StreamSteamHelperLogAsync)
- docs/plans/platform-native-ui/analysis-code.md (Pattern 4: Two-Phase Launch, Pattern 7: Log Streaming)

**Instructions**

Files to Create

- src/crosshook-native/src-tauri/src/commands/launch.rs

Files to Modify

- src/crosshook-native/src-tauri/src/lib.rs

Implement: `validate_launch(request: SteamLaunchRequest) -> Result<(), String>`, `launch_game(app: AppHandle, request: SteamLaunchRequest) -> Result<LaunchResult, String>` (builds command via script_runner, spawns async, creates log file at `/tmp/crosshook-logs/`, starts log streaming task), `launch_trainer(app: AppHandle, request: SteamLaunchRequest) -> Result<LaunchResult, String>` (same but uses trainer script). Log streaming: spawn a `tokio::task` that reads the log file with a 500ms poll interval and emits `"launch-log"` events to the frontend via `app.emit("launch-log", line)`. Register commands.

#### Task 1.12: React profile editor form

Depends on [1.4, 1.10]

**READ THESE BEFORE TASK**

- src/CrossHookEngine.App/Forms/MainForm.cs (lines 1-200 for UI field layout)
- docs/plans/platform-native-ui/research-ux.md (User Workflows / Primary Flow sections)

**Instructions**

Files to Create

- src/crosshook-native/src/components/ProfileEditor.tsx
- src/crosshook-native/src/hooks/useProfile.ts

Build a form with inputs for all profile fields: game path (with browse button via `@tauri-apps/plugin-dialog`), trainer path (browse), Steam App ID, compatdata path (browse), Proton path (browse), launcher icon path (browse). Add a Steam Mode toggle that shows/hides the Steam-specific fields. Add profile selector dropdown (populated via `invoke("profile_list")`), Save button, Delete button. The `useProfile` hook manages load/save/delete via Tauri `invoke()` calls and tracks dirty state.

#### Task 1.13: React two-step launch UI

Depends on [1.4, 1.11]

**READ THESE BEFORE TASK**

- src/CrossHookEngine.App/Forms/MainForm.cs (lines 2119-2153 UpdateSteamModeUiState)
- docs/plans/platform-native-ui/analysis-code.md (Pattern 4: Two-Phase Launch State Machine)

**Instructions**

Files to Create

- src/crosshook-native/src/components/LaunchPanel.tsx
- src/crosshook-native/src/hooks/useLaunchState.ts

Implement the two-phase launch state machine using `useReducer` with `LaunchPhase` enum. Phase 1 (Idle): button reads "Launch Game", calls `invoke("launch_game")`. On success, transition to `WaitingForTrainer`: button reads "Launch Trainer", hint text says "Wait for game to reach the main menu, then click Launch Trainer". Phase 2: calls `invoke("launch_trainer")`, transitions to `SessionActive`. Loading a new profile or toggling Steam mode resets to `Idle`. Show status text for each phase. Disable button during `GameLaunching`/`TrainerLaunching` states.

#### Task 1.14: React console log view

Depends on [1.11]

**READ THESE BEFORE TASK**

- docs/plans/platform-native-ui/analysis-code.md (Pattern 7: Log Streaming)

**Instructions**

Files to Create

- src/crosshook-native/src/components/ConsoleView.tsx

Scrollable log panel that subscribes to Tauri `"launch-log"` events via `listen("launch-log", callback)` from `@tauri-apps/api/event`. Auto-scrolls to bottom on new entries. Monospace font, dark background. Show timestamps per line. Clear button to reset log. Expandable/collapsible panel. Display "Waiting for log output..." when empty and a launch is in progress.

#### Task 1.15: CLI launcher (headless)

Depends on [1.5, 1.7]

**READ THESE BEFORE TASK**

- src/CrossHookEngine.App/Services/CommandLineParser.cs

**Instructions**

Files to Modify

- src/crosshook-native/crates/crosshook-cli/src/main.rs

Implement a minimal `clap` CLI: `crosshook launch --profile <name>` loads the profile from the profiles directory, validates the launch request, and invokes `steam-launch-helper.sh`. Print log output to stdout. Support `--profile-dir <path>` to override the default profiles directory. Support `--scripts-dir <path>` to specify where the shell scripts are located. This provides an immediate headless tool for Steam Deck Gaming Mode.

#### Task 1.16: Shell script bundling

Depends on [1.1, 1.7]

**READ THESE BEFORE TASK**

- src/crosshook-native/src-tauri/tauri.conf.json

**Instructions**

Files to Modify

- src/crosshook-native/src-tauri/tauri.conf.json

Files to Create

- src/crosshook-native/src-tauri/src/paths.rs

Configure Tauri's resource bundling to include the 3 runtime-helper scripts. In `tauri.conf.json`, add `"bundle": { "resources": ["../../src/CrossHookEngine.App/runtime-helpers/*.sh"] }`. In `paths.rs`, implement `fn resolve_script_path(app: &AppHandle, script_name: &str) -> PathBuf` using `app.path().resolve_resource()` to locate bundled scripts at runtime. Ensure scripts have execute permission after bundling (add a `setup()` hook that `chmod +x` the scripts on first run).

### Phase 2: Smart Discovery

#### Task 2.1: VDF key-value parser

Depends on [none]

**READ THESE BEFORE TASK**

- src/CrossHookEngine.App/Services/SteamAutoPopulateService.cs (lines 739-863 ParseKeyValueContent, ParseKeyValueObject, ReadToken)
- docs/plans/platform-native-ui/analysis-code.md (Pattern 2: VDF Parser)

**Instructions**

Files to Create

- src/crosshook-native/crates/crosshook-core/src/steam/vdf.rs

First evaluate the `steam-vdf-parser` crate: test it against real `libraryfolders.vdf`, `appmanifest_*.acf`, `config.vdf`, and `compatibilitytool.vdf` files. Verify it handles case-insensitive key lookups and recursive descendant search. If the crate works, wrap it with a case-insensitive key adapter. If not, port the C# recursive-descent parser (~125 lines): handle quoted strings with `\n`/`\r`/`\t`/`\\`/`\"` escapes, unquoted tokens (terminate at whitespace/braces), nested `{}` blocks, `//` comment lines. Use a `VdfNode` struct with `value: Option<String>` and `children: HashMap<String, VdfNode>` (case-insensitive via `unicase` crate or key normalization). Include `find_descendant(key)` for recursive DFS. Write comprehensive tests with fixture files copied from real Steam installations.

#### Task 2.2: Steam root discovery

Depends on [none]

**READ THESE BEFORE TASK**

- src/CrossHookEngine.App/Services/SteamAutoPopulateService.cs (lines 164-188 DiscoverSteamRootCandidates)

**Instructions**

Files to Create

- src/crosshook-native/crates/crosshook-core/src/steam/discovery.rs

Port `DiscoverSteamRootCandidates`: check `STEAM_COMPAT_CLIENT_INSTALL_PATH` env var, then `$HOME/.steam/root`, then `$HOME/.local/share/Steam`. Add new Flatpak path: `$HOME/.var/app/com.valvesoftware.Steam/data/Steam` (not in the C# code). Validate each candidate by checking `steamapps/` subdirectory exists. Return `Vec<PathBuf>` of valid roots, deduplicated. Accept `diagnostics: &mut Vec<String>` parameter for accumulating discovery steps.

#### Task 2.3: Steam library discovery

Depends on [2.1, 2.2]

**READ THESE BEFORE TASK**

- src/CrossHookEngine.App/Services/SteamAutoPopulateService.cs (lines 190-239 DiscoverSteamLibraries)

**Instructions**

Files to Create

- src/crosshook-native/crates/crosshook-core/src/steam/libraries.rs

Port `DiscoverSteamLibraries`: for each root, parse `steamapps/libraryfolders.vdf` using the VDF parser. Handle both format generations — Format A (entry has `path` child node) and Format B (entry value IS the path). Validate each library by checking `steamapps/` directory exists. Deduplicate by path. Return `Vec<SteamLibrary>` with `path` and `steamapps_path` fields. Thread `diagnostics` through for VDF parse failures.

#### Task 2.4: Steam data models

Depends on [none]

**READ THESE BEFORE TASK**

- docs/plans/platform-native-ui/analysis-code.md (Internal Data Structures section)

**Instructions**

Files to Create

- src/crosshook-native/crates/crosshook-core/src/steam/mod.rs
- src/crosshook-native/crates/crosshook-core/src/steam/models.rs

Define all Steam-related types: `SteamLibrary { path, steamapps_path }`, `SteamGameMatch { app_id, library_path, install_dir_path, manifest_path }`, `SteamAutoPopulateFieldState` enum (`NotFound, Found, Ambiguous`), `SteamAutoPopulateResult { app_id_state, app_id, compatdata_state, compatdata_path, proton_state, proton_path, diagnostics, manual_hints }`, `SteamAutoPopulateRequest { game_path, steam_client_install_path }`, `ProtonInstall { name, path, is_official, aliases, normalized_aliases }`, `DiagnosticCollector { diagnostics: Vec<String>, manual_hints: Vec<String> }`. All with serde derives.

#### Task 2.5: Manifest matching

Depends on [2.1, 2.3, 2.4]

**READ THESE BEFORE TASK**

- src/CrossHookEngine.App/Services/SteamAutoPopulateService.cs (lines 241-314 FindGameMatch)

**Instructions**

Files to Create

- src/crosshook-native/crates/crosshook-core/src/steam/manifest.rs

Port `FindGameMatch`: for each library, enumerate `appmanifest_*.acf` files, parse each via VDF parser, extract `AppState.appid` and `AppState.installdir`, construct install path as `library/steamapps/common/<installdir>`, check if the user's game path starts with (is same as or child of) the install path. Return `Found` with match data if exactly one manifest matches, `Ambiguous` if multiple, `NotFound` if none. Fallback: extract App ID from filename if missing from manifest content. Derive compatdata path as `library/steamapps/compatdata/<appid>`, verify with `Path::exists()`.

#### Task 2.6: Proton tool discovery

Depends on [2.1]

**READ THESE BEFORE TASK**

- src/CrossHookEngine.App/Services/SteamAutoPopulateService.cs (lines 498-543 DiscoverCompatTools)

**Instructions**

Files to Create

- src/crosshook-native/crates/crosshook-core/src/steam/proton.rs

Port `DiscoverCompatTools`: scan `steamapps/common/*/proton` (official), `compatibilitytools.d/*/proton` (custom GE-Proton), and system paths (`/usr/share/steam/compatibilitytools.d/`, `/usr/local/share/steam/compatibilitytools.d/`). For each found `proton` executable, read `compatibilitytool.vdf` if present for display_name and compat_tools aliases. Build `ProtonInstall` with directory name + VDF names + normalized aliases (lowercase alphanumeric only). Return `Vec<ProtonInstall>`.

#### Task 2.7: Proton resolution

Depends on [2.6]

**READ THESE BEFORE TASK**

- src/CrossHookEngine.App/Services/SteamAutoPopulateService.cs (lines 371-429 ResolveProtonPath, lines 431-496 CollectCompatToolMappings)

**Instructions**

Files to Modify

- src/crosshook-native/crates/crosshook-core/src/steam/proton.rs

Port `CollectCompatToolMappings`: parse `config/config.vdf` and each `userdata/*/config/localconfig.vdf`, find `CompatToolMapping` via recursive descendant search, extract `<appid>.name` entries. Port `ResolveProtonPath`: look up the app-specific mapping (or default `"0"`), then match the tool name against discovered installs by: (1) exact alias match (case-insensitive), (2) normalized alias match (alphanumeric only), (3) heuristic substring/version match. Return `Found`/`Ambiguous`/`NotFound` with the resolved `proton` executable path.

#### Task 2.8: Auto-populate orchestrator

Depends on [2.3, 2.5, 2.7, 2.9]

**READ THESE BEFORE TASK**

- src/CrossHookEngine.App/Services/SteamAutoPopulateService.cs (lines 49-162 AttemptAutoPopulate)

**Instructions**

Files to Create

- src/crosshook-native/crates/crosshook-core/src/steam/auto_populate.rs

Port `AttemptAutoPopulate`: compose the full pipeline — discover roots, discover libraries, find game match, derive compatdata path, resolve Proton path. Thread `DiagnosticCollector` through every step. The native version is simpler: no WINE path normalization (paths are already native Linux), no dosdevices resolution. Return `SteamAutoPopulateResult` with per-field states. Deduplicate diagnostics and manual_hints before returning. Include integration test with a mock Steam directory tree.

#### Task 2.9: Diagnostic collector

Depends on [2.4]

**READ THESE BEFORE TASK**

- docs/plans/platform-native-ui/analysis-context.md (Patterns to Follow / Diagnostic Accumulation)

**Instructions**

Files to Create

- src/crosshook-native/crates/crosshook-core/src/steam/diagnostics.rs

Implement `DiagnosticCollector` struct: `diagnostics: Vec<String>`, `manual_hints: Vec<String>`. Methods: `fn add_diagnostic(&mut self, msg: impl Into<String>)`, `fn add_hint(&mut self, msg: impl Into<String>)`, `fn finalize(self) -> (Vec<String>, Vec<String>)` (deduplicates both lists). Optionally integrate with `tracing::info!` to also log diagnostics at the info level.

#### Task 2.10: Tauri IPC commands (steam)

Depends on [2.8]

**READ THESE BEFORE TASK**

- docs/plans/platform-native-ui/analysis-code.md (Tauri IPC Commands Needed table)

**Instructions**

Files to Create

- src/crosshook-native/src-tauri/src/commands/steam.rs

Files to Modify

- src/crosshook-native/src-tauri/src/lib.rs

Implement `#[tauri::command] async fn auto_populate_steam(request: SteamAutoPopulateRequest) -> Result<SteamAutoPopulateResult, String>`. Register command. This wraps `auto_populate::attempt_auto_populate()` and runs on a background thread via `tauri::async_runtime::spawn_blocking` since VDF parsing and filesystem scanning are I/O heavy.

#### Task 2.11: React auto-populate UI

Depends on [2.10]

**READ THESE BEFORE TASK**

- docs/plans/platform-native-ui/research-ux.md (User Workflows / Primary Flow)

**Instructions**

Files to Create

- src/crosshook-native/src/components/AutoPopulate.tsx

"Auto-Populate" button that calls `invoke("auto_populate_steam")`. Display per-field results with icons: green check for `Found` (auto-fill the field), yellow warning for `Ambiguous` (show candidates), red X for `NotFound` (show guidance). Expandable diagnostics panel showing the discovery steps. Manual hints section with actionable guidance text. Loading spinner during discovery.

#### Task 2.12: Launcher export (Rust)

Depends on [1.7]

**READ THESE BEFORE TASK**

- src/CrossHookEngine.App/Services/SteamExternalLauncherExportService.cs

**Instructions**

Files to Create

- src/crosshook-native/crates/crosshook-core/src/export/mod.rs
- src/crosshook-native/crates/crosshook-core/src/export/launcher.rs

Port `SteamExternalLauncherExportService` (350 lines): validate request, generate slug from display name (lowercase, replace non-alphanumeric with `-`, collapse consecutive `-`), generate `.sh` trainer script using the proven Proton launch pattern (set env vars, `exec "$PROTON" run "$TRAINER_WINDOWS_PATH"`), generate `.desktop` entry with Name, Exec (escaped), Icon (fallback to `applications-games`), Categories=Game. Write to `~/.local/share/crosshook/launchers/` and `~/.local/share/applications/`. Include shell single-quote escaping (`'"'"'` pattern). Include home path resolution with Steam path suffix stripping.

#### Task 2.13: Tauri IPC commands (export)

Depends on [2.12]

**Instructions**

Files to Create

- src/crosshook-native/src-tauri/src/commands/export.rs

Files to Modify

- src/crosshook-native/src-tauri/src/lib.rs

Implement `validate_launcher_export(request: ExportRequest) -> Result<(), String>` and `export_launchers(request: ExportRequest) -> Result<ExportResult, String>`. Register commands.

#### Task 2.14: React launcher export UI

Depends on [2.13]

**Instructions**

Files to Create

- src/crosshook-native/src/components/LauncherExport.tsx

Export button with launcher name field (pre-filled from profile), icon picker (browse), submit action. Show success feedback with paths to generated `.sh` and `.desktop` files. Show validation errors inline.

### Phase 3: Polish & Distribution

#### Task 3.1: Settings persistence (Rust)

Depends on [none]

**READ THESE BEFORE TASK**

- src/CrossHookEngine.App/Services/AppSettingsService.cs

**Instructions**

Files to Create

- src/crosshook-native/crates/crosshook-core/src/settings/mod.rs

Port `AppSettingsService` (81 lines) to TOML format at `~/.config/crosshook/settings.toml`. Fields: `auto_load_last_profile: bool`, `last_used_profile: String`. Use `serde` + `toml`. Create directory if missing.

#### Task 3.2: Recent files tracking (Rust)

Depends on [none]

**Instructions**

Files to Create

- src/crosshook-native/crates/crosshook-core/src/settings/recent.rs

Port `RecentFilesService` (131 lines) to TOML at `~/.local/share/crosshook/recent.toml`. Three lists: `game_paths`, `trainer_paths`, `dll_paths`. Drop non-existent paths on load (matching C# behavior). Cap list size (default 10 entries).

#### Task 3.3: Auto-load profile at startup

Depends on [3.1, 1.10]

**Instructions**

Files to Create

- src/crosshook-native/src-tauri/src/startup.rs

Port `MainFormStartupCoordinator.ResolveAutoLoadProfileName` (34 lines): check settings for auto_load, verify profile exists, return name. Call from Tauri `setup()` hook and emit `"auto-load-profile"` event to React frontend.

#### Task 3.4: Tauri IPC commands (settings)

Depends on [3.1, 3.2]

**Instructions**

Files to Create

- src/crosshook-native/src-tauri/src/commands/settings.rs

Implement `settings_load`, `settings_save`, `recent_files_load`, `recent_files_save`. Register commands.

#### Task 3.5: React settings panel

Depends on [3.4]

**Instructions**

Files to Create

- src/crosshook-native/src/components/SettingsPanel.tsx

Settings UI: auto-load toggle, recent files display, profiles directory configuration. Simple form layout.

#### Task 3.6: Dark gaming theme

Depends on [1.2]

**Instructions**

Files to Create

- src/crosshook-native/src/styles/theme.css
- src/crosshook-native/src/styles/variables.css

Dark background (#1a1a2e), high-contrast text (#e0e0e0), accent color (electric blue #0078d4), monospace log font, large touch targets (min 48px), responsive for 1280x800 (Steam Deck). Respect `prefers-reduced-motion`.

#### Task 3.7: Controller/gamepad navigation

Depends on [3.6]

**Instructions**

Files to Create

- src/crosshook-native/src/hooks/useGamepadNav.ts
- src/crosshook-native/src/styles/focus.css

Focus ring management (2px+ with 3:1 contrast), Tab/D-pad traversal, Enter/A confirm, Escape/B back. Large touch targets. Detect `SteamDeck=1` env var for auto-enabling controller mode.

#### Task 3.8: AppImage packaging

Depends on [1.1]

**Instructions**

Files to Modify

- src/crosshook-native/src-tauri/tauri.conf.json

Files to Create

- scripts/build-native.sh

Configure Tauri's AppImage build target. Create build script that runs `cargo tauri build --target x86_64-unknown-linux-gnu` and outputs the AppImage to `dist/`.

#### Task 3.9: AUR PKGBUILD

Depends on [3.8]

**Instructions**

Files to Create

- packaging/PKGBUILD

Arch Linux PKGBUILD for AUR distribution. Source from GitHub releases. Build with `cargo tauri build`. Install binary and shell scripts.

#### Task 3.10: CI/CD integration

Depends on [3.8]

**Instructions**

Files to Create

- .github/workflows/native-build.yml

GitHub Actions workflow: trigger on `v*` tags, set up Rust toolchain + Node.js, install WebKitGTK dev packages, run `cargo test` on crosshook-core, build Tauri AppImage, upload as release artifact alongside existing .NET artifacts.

#### Task 3.11: CLI argument parsing

Depends on [1.15]

**Instructions**

Files to Create

- src/crosshook-native/crates/crosshook-cli/src/args.rs

Full `clap` derive parser: `launch --profile <name>`, `profile list|import|export`, `steam discover|auto-populate`, `--verbose`, `--json`, `--config <path>`. Matches the CLI interface from the feature spec.

#### Task 3.12: Structured logging

Depends on [none]

**Instructions**

Files to Create

- src/crosshook-native/crates/crosshook-core/src/logging.rs

Set up `tracing` + `tracing-subscriber` with file output to `~/.local/share/crosshook/logs/crosshook.log` and optional stdout. Rotate logs. Port the pattern from `AppDiagnostics.cs`.

### Phase 4: Community Features

#### Task 4.1: Community profile JSON schema

Depends on [1.3]

**Instructions**

Files to Create

- src/crosshook-native/crates/crosshook-core/src/profile/community_schema.rs
- schemas/community-profile.json

Define JSON schema for shareable profiles with metadata: game name, game version, trainer name, trainer version, Proton version, platform tags, compatibility rating, author, description.

#### Task 4.2: Profile import/export

Depends on [4.1]

**Instructions**

Files to Create

- src/crosshook-native/crates/crosshook-core/src/profile/exchange.rs

Import community JSON profiles into local TOML. Export local profiles to community JSON with validation. Handle version compatibility.

#### Task 4.3: Git-based profile sharing

Depends on [4.1]

**Instructions**

Files to Create

- src/crosshook-native/crates/crosshook-core/src/community/taps.rs
- src/crosshook-native/crates/crosshook-core/src/community/index.rs

"Taps" system: clone/pull Git repos containing community profiles, index available profiles, manage tap subscriptions. Store tap URLs in settings. Periodic sync.

#### Task 4.4: Tauri IPC commands (community)

Depends on [4.2, 4.3]

**Instructions**

Files to Create

- src/crosshook-native/src-tauri/src/commands/community.rs

Wire `community_add_tap`, `community_list_profiles`, `community_import_profile`, `community_sync`.

#### Task 4.5: React profile browser UI

Depends on [4.4]

**Instructions**

Files to Create

- src/crosshook-native/src/components/CommunityBrowser.tsx
- src/crosshook-native/src/hooks/useCommunityProfiles.ts

Searchable/filterable list of community profiles from taps. Import action. Compatibility badges. Tap management panel.

#### Task 4.6: Trainer compatibility database viewer

Depends on [4.4]

**Instructions**

Files to Create

- src/crosshook-native/src/components/CompatibilityViewer.tsx

Display known trainer-game compatibility data. Filter by game/trainer/platform.

## Advice

- **Start with the CLI golden path** (T1.1 → T1.3 → T1.5 → T1.7 → T1.15 → T1.16): prove the Rust → shell script → Proton pipeline works natively before building any UI. This is 1-2 days of work that validates the core architecture.

- **The VDF parser (T2.1) is the Phase 2 linchpin**: evaluate `steam-vdf-parser` crate immediately, even during Phase 1 work. Its case-insensitive key behavior determines whether you port 125 lines of parser code or use a 1-line crate dependency. Test against real Steam files from your machine.

- **Path conversion code from SteamLaunchService.cs is NOT needed**: the native app works with Linux paths directly. Do NOT port `ConvertToUnixPath`, `ConvertToWindowsPath`, `ResolveDosDevicesPath`, or `NormalizeSteamHostPath` (~400 lines). The only exception: legacy `.profile` import must detect and convert `Z:\` prefix paths.

- **The two-phase launch state machine is deceptively simple**: the C# version uses a single boolean (`_steamTrainerLaunchPending`). The React version should use a proper `LaunchPhase` enum with `useReducer` — this prevents the scattered boolean state bugs that the C# monolith encourages.

- **Shell scripts handle their own env cleanup**: when running natively (not under WINE), there are far fewer inherited WINE variables to strip. The scripts' `setsid env -i` pattern already provides a clean slate. The Rust `Command` should still use `.env_clear()` for defense-in-depth, but don't over-engineer this.

- **Phase 2 script selection matters**: Phase 1 game launch uses `steam-launch-helper.sh --game-only`. Phase 2 trainer launch uses `steam-launch-trainer.sh` (a DIFFERENT script), NOT `steam-launch-helper.sh --trainer-only`. The trainer script provides fuller environment isolation via `setsid env -i` delegation to the host runner.

- **Flatpak Steam detection is a net-new feature**: the C# code does NOT check `~/.var/app/com.valvesoftware.Steam/`. Adding this in T2.2 fills a documented gap that affects Steam Deck users with Flatpak Steam.

- **PR #18 identified shell script bugs**: exit code capture in `steam-host-trainer-runner.sh` and `steam-launch-helper.sh` may be incorrect. Investigate and fix before or during T1.16. These scripts are your production runtime.

- **Profile format is a cross-app contract**: the native app MUST read `.profile` files from the WinForms app. Test with real profiles. The `Z:\` path prefix, `True`/`False` boolean casing, and missing-field defaults are all edge cases to cover in T1.5.

- **React component development can use mock data**: T1.12, T1.13, T1.14 can proceed with hardcoded test data while Tauri IPC commands are being wired up. The `ProfileData` TypeScript types (T1.4) are the shared contract.
