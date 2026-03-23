# Platform-Native UI: Task Analysis & Phase Organization

## Executive Summary

This analysis decomposes the CrossHook platform-native UI (Tauri v2 + React/TypeScript + Rust) into 48 discrete tasks across 4 phases, each scoped to touch 1-3 files. The existing C# codebase contains 7 service files totaling ~3,500 lines of domain logic, 3 shell scripts (~630 lines) that are reused directly, and a ~2,900-line MainForm that defines the UI workflow. The most complex porting target is `SteamAutoPopulateService.cs` (1,286 lines with a recursive-descent VDF parser, multi-library discovery, and Proton tool resolution), which should be broken into 5+ sub-tasks. Phase 1 (MVP) has a critical path through Tauri scaffolding, profile data models, and shell script invocation; roughly 40% of Phase 1 tasks can run in parallel once scaffolding is complete.

---

## Recommended Phase Structure

### Phase 1: Foundation & MVP

**Goal**: Working profile-driven game+trainer launcher that invokes existing shell scripts.
**Estimated Tasks**: 16
**Estimated Duration**: 4-6 weeks
**Critical Path**: T1.1 -> T1.3 -> T1.5 -> T1.7 -> T1.10 -> T1.12 -> T1.14

| ID    | Task                                                                                                                                                                                                                                                                                                  | Files Touched                                                                                   | Depends On       | Parallel Group |
| ----- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------- | ---------------- | -------------- |
| T1.1  | **Rust workspace scaffolding**: Create `src/crosshook-native/` with Cargo workspace, `crosshook-core` lib crate, `crosshook-cli` bin crate, `src-tauri/` Tauri app                                                                                                                                    | `Cargo.toml` (workspace), `crates/crosshook-core/Cargo.toml`, `crates/crosshook-cli/Cargo.toml` | --               | A              |
| T1.2  | **React/Vite scaffolding**: Initialize `package.json`, `vite.config.ts`, `tsconfig.json`, `src/App.tsx` shell, Tauri integration                                                                                                                                                                      | `package.json`, `vite.config.ts`, `src/App.tsx`                                                 | --               | A              |
| T1.3  | **ProfileData model (Rust)**: Define `ProfileData` struct with serde Serialize/Deserialize matching the 12-field `.profile` format, plus TOML `GameProfile` struct                                                                                                                                    | `crates/crosshook-core/src/profile/mod.rs`, `crates/crosshook-core/src/profile/models.rs`       | T1.1             | B              |
| T1.4  | **ProfileData model (TypeScript)**: Define TypeScript interfaces mirroring Rust `ProfileData` and `GameProfile` for IPC                                                                                                                                                                               | `src/types/profile.ts`                                                                          | T1.2             | B              |
| T1.5  | **Legacy profile reader/writer**: Implement `Key=Value` parser (split on first `=`, `bool::from_str` for booleans), writer, list, delete -- port of `ProfileService.cs` (225 lines)                                                                                                                   | `crates/crosshook-core/src/profile/legacy.rs`                                                   | T1.3             | C              |
| T1.6  | **TOML profile reader/writer**: Implement TOML-based `GameProfile` load/save using `serde` + `toml` crate, XDG path resolution (`~/.config/crosshook/profiles/`)                                                                                                                                      | `crates/crosshook-core/src/profile/toml_store.rs`                                               | T1.3             | C              |
| T1.7  | **Shell script invocation wrapper**: Rust module that builds `Command` with correct args for `steam-launch-helper.sh` and `steam-launch-trainer.sh`, strips ~30 env vars, sets 3 required vars -- port of `SteamLaunchService.CreateHelperStartInfo` / `CreateTrainerStartInfo` logic (lines 139-196) | `crates/crosshook-core/src/launch/mod.rs`, `crates/crosshook-core/src/launch/script_runner.rs`  | T1.1             | C              |
| T1.8  | **Launch request validation**: Port `SteamLaunchService.Validate()` (lines 69-109) -- `SteamLaunchRequest` struct + `fn validate() -> Result<(), ValidationError>`                                                                                                                                    | `crates/crosshook-core/src/launch/request.rs`                                                   | T1.7             | D              |
| T1.9  | **Environment cleanup constants**: Port the 30 env vars from `SteamLaunchService.GetEnvironmentVariablesToClear()` (lines 233-269) and `steam-host-trainer-runner.sh` (lines 152-161) into a shared const array                                                                                       | `crates/crosshook-core/src/launch/env.rs`                                                       | T1.7             | D              |
| T1.10 | **Tauri IPC commands (profile)**: Wire `profile_list`, `profile_load`, `profile_save`, `profile_delete`, `profile_import_legacy` as Tauri commands calling crosshook-core                                                                                                                             | `src-tauri/src/commands/profile.rs`, `src-tauri/src/lib.rs`                                     | T1.5, T1.6       | E              |
| T1.11 | **Tauri IPC commands (launch)**: Wire `launch_game`, `launch_trainer`, `validate_launch` as Tauri commands, including async log streaming via Tauri events                                                                                                                                            | `src-tauri/src/commands/launch.rs`                                                              | T1.7, T1.8, T1.9 | E              |
| T1.12 | **React profile editor form**: Form with inputs for all 12 profile fields, file/directory browser dialogs (Tauri `dialog` plugin), save/load/delete actions                                                                                                                                           | `src/components/ProfileEditor.tsx`, `src/hooks/useProfile.ts`                                   | T1.4, T1.10      | F              |
| T1.13 | **React two-step launch UI**: Launch button that toggles between "Launch Game" and "Launch Trainer" states, status text, hint text -- replicating `UpdateSteamModeUiState()` (MainForm.cs lines 2119-2153)                                                                                            | `src/components/LaunchPanel.tsx`, `src/hooks/useLaunchState.ts`                                 | T1.4, T1.11      | F              |
| T1.14 | **React console log view**: Scrollable log panel that subscribes to Tauri event stream for real-time helper log output -- mirrors `StreamSteamHelperLogAsync` (MainForm.cs lines 2908-2946)                                                                                                           | `src/components/ConsoleView.tsx`                                                                | T1.11            | F              |
| T1.15 | **CLI launcher (headless)**: `crosshook-cli` binary that loads a profile by name and invokes the shell script wrapper, for scripting and Gaming Mode use                                                                                                                                              | `crates/crosshook-cli/src/main.rs`                                                              | T1.5, T1.7       | C              |
| T1.16 | **Shell script bundling**: Copy the 3 runtime-helper scripts into the Tauri bundle at build time, resolve their path at runtime via `tauri::api::path`                                                                                                                                                | `src-tauri/tauri.conf.json`, `src-tauri/src/paths.rs`                                           | T1.1, T1.7       | D              |

**Parallelization Map (Phase 1)**:

- **Group A** (T1.1, T1.2): Independent scaffolding, fully parallel.
- **Group B** (T1.3, T1.4): Data models, parallel after their respective scaffolding.
- **Group C** (T1.5, T1.6, T1.7, T1.15): Core Rust logic, parallel after T1.3.
- **Group D** (T1.8, T1.9, T1.16): Launch helpers, parallel after T1.7.
- **Group E** (T1.10, T1.11): Tauri IPC layer, parallel after their respective core modules.
- **Group F** (T1.12, T1.13, T1.14): React UI components, parallel after IPC commands.

---

### Phase 2: Smart Discovery

**Goal**: Feature parity with WinForms Steam auto-discovery workflow.
**Estimated Tasks**: 14
**Estimated Duration**: 3-4 weeks
**Dependencies**: Phase 1 core launch must work (T1.7, T1.10, T1.11 complete).
**Critical Path**: T2.1 -> T2.3 -> T2.5 -> T2.8 -> T2.10

| ID    | Task                                                                                                                                                                                                                                                                                         | Files Touched                                        | Depends On       | Parallel Group |
| ----- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------- | ---------------- | -------------- |
| T2.1  | **VDF key-value parser**: Port or integrate VDF parsing -- the C# `ParseKeyValueContent` / `ParseKeyValueObject` / `ReadToken` recursive-descent parser (SteamAutoPopulateService.cs lines 587-863). Evaluate `steam-vdf-parser` crate for case-insensitive key matching before hand-rolling | `crates/crosshook-core/src/steam/vdf.rs`             | T1.1             | A              |
| T2.2  | **Steam root discovery**: Port `DiscoverSteamRootCandidates` (lines 164-188) -- checks `STEAM_COMPAT_CLIENT_INSTALL_PATH`, `$HOME/.steam/root`, `$HOME/.local/share/Steam`, and Flatpak path `~/.var/app/com.valvesoftware.Steam/` (new, not in C#)                                          | `crates/crosshook-core/src/steam/discovery.rs`       | T1.1             | A              |
| T2.3  | **Steam library discovery**: Port `DiscoverSteamLibraries` (lines 190-239) -- parse `libraryfolders.vdf`, extract library paths, validate `steamapps/` subdirectory exists                                                                                                                   | `crates/crosshook-core/src/steam/libraries.rs`       | T2.1, T2.2       | B              |
| T2.4  | **Steam library data models**: Define Rust structs for `SteamLibraryInfo`, `SteamGameMatch`, `SteamGameMatchSelection`, `SteamAutoPopulateResult`, `SteamAutoPopulateFieldState` enum                                                                                                        | `crates/crosshook-core/src/steam/models.rs`          | T1.1             | A              |
| T2.5  | **Manifest matching**: Port `FindGameMatch` (lines 241-314) -- enumerate `appmanifest_*.acf` files, parse each, match `installdir` against game path using `PathIsSameOrChild`                                                                                                               | `crates/crosshook-core/src/steam/manifest.rs`        | T2.1, T2.3, T2.4 | C              |
| T2.6  | **Proton tool discovery**: Port `DiscoverCompatTools` (lines 498-543) -- scan `steamapps/common/`, `compatibilitytools.d/`, system paths (`/usr/share/steam/...`), parse `compatibilitytool.vdf` for aliases                                                                                 | `crates/crosshook-core/src/steam/proton.rs`          | T2.1             | B              |
| T2.7  | **Proton resolution**: Port `ResolveProtonPath` (lines 371-429) + `CollectCompatToolMappings` (lines 431-496) -- read `config.vdf` / `localconfig.vdf` CompatToolMapping sections, match against installed tools by alias                                                                    | `crates/crosshook-core/src/steam/proton.rs` (extend) | T2.6             | C              |
| T2.8  | **Auto-populate orchestrator**: Port `AttemptAutoPopulate` (lines 49-162) -- compose discovery, matching, proton resolution, diagnostic accumulation into single result                                                                                                                      | `crates/crosshook-core/src/steam/auto_populate.rs`   | T2.3, T2.5, T2.7 | D              |
| T2.9  | **Diagnostic collector**: Implement `DiagnosticCollector` struct (replaces C# `List<string> diagnostics, List<string> manualHints` pattern) with typed diagnostic entries                                                                                                                    | `crates/crosshook-core/src/steam/diagnostics.rs`     | T2.4             | A              |
| T2.10 | **Tauri IPC commands (steam)**: Wire `steam_discover`, `steam_auto_populate`, `steam_list_proton` as Tauri commands                                                                                                                                                                          | `src-tauri/src/commands/steam.rs`                    | T2.8             | E              |
| T2.11 | **React auto-populate UI**: "Auto-Populate" button, result display with Found/NotFound/Ambiguous states per field, diagnostics expandable panel, manual hints                                                                                                                                | `src/components/AutoPopulate.tsx`                    | T2.10            | F              |
| T2.12 | **Launcher export (Rust)**: Port `SteamExternalLauncherExportService.ExportLaunchers` (350 lines) -- generate `.sh` script and `.desktop` entry, validate icon, resolve home path                                                                                                            | `crates/crosshook-core/src/export/launcher.rs`       | T1.7             | B              |
| T2.13 | **Tauri IPC commands (export)**: Wire `export_launcher` as Tauri command                                                                                                                                                                                                                     | `src-tauri/src/commands/export.rs`                   | T2.12            | E              |
| T2.14 | **React launcher export UI**: Export button with name field, icon picker, success/error feedback showing generated file paths                                                                                                                                                                | `src/components/LauncherExport.tsx`                  | T2.13            | F              |

**Parallelization Map (Phase 2)**:

- **Group A** (T2.1, T2.2, T2.4, T2.9): Foundation modules, all parallel.
- **Group B** (T2.3, T2.6, T2.12): Discovery + export, parallel after Group A.
- **Group C** (T2.5, T2.7): Matching + resolution, parallel after Group B.
- **Group D** (T2.8): Orchestrator, sequential after Group C.
- **Group E** (T2.10, T2.13): IPC layer, parallel after their respective backends.
- **Group F** (T2.11, T2.14): UI, parallel after IPC.

---

### Phase 3: Polish & Distribution

**Goal**: Production-ready release with settings persistence, theming, packaging.
**Estimated Tasks**: 12
**Estimated Duration**: 3-4 weeks
**Dependencies**: Phase 2 MVP functional.

| ID    | Task                                                                                                                                                                                  | Files Touched                                                     | Depends On  | Parallel Group |
| ----- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------- | ----------- | -------------- |
| T3.1  | **Settings persistence (Rust)**: Port `AppSettingsService.cs` (81 lines) -- TOML-based `settings.toml` at `~/.config/crosshook/`, `auto_load_last_profile`, `last_used_profile`       | `crates/crosshook-core/src/settings/mod.rs`                       | T1.1        | A              |
| T3.2  | **Recent files tracking (Rust)**: Port `RecentFilesService.cs` (131 lines) -- MRU lists for game, trainer, DLL paths, persisted in TOML or dedicated file                             | `crates/crosshook-core/src/settings/recent.rs`                    | T1.1        | A              |
| T3.3  | **Auto-load profile at startup**: Port `MainFormStartupCoordinator.ResolveAutoLoadProfileName` (34 lines) -- check settings, resolve against available profiles, apply on app init    | `src-tauri/src/startup.rs`                                        | T3.1, T1.10 | B              |
| T3.4  | **Tauri IPC commands (settings)**: Wire `settings_load`, `settings_save`, `recent_files_load`, `recent_files_add`                                                                     | `src-tauri/src/commands/settings.rs`                              | T3.1, T3.2  | B              |
| T3.5  | **React settings panel**: Settings UI with auto-load toggle, recent files display, profile path config                                                                                | `src/components/SettingsPanel.tsx`                                | T3.4        | C              |
| T3.6  | **Dark gaming theme**: CSS theme with dark background, high-contrast accents, respect `prefers-reduced-motion`, gaming aesthetic -- targets 1280x800 Steam Deck resolution            | `src/styles/theme.css`, `src/styles/variables.css`                | T1.2        | A              |
| T3.7  | **Controller/gamepad navigation**: Focus ring management, keyboard/D-pad traversal, large touch targets, Tab/Enter/Escape flow, adaptive button prompt glyphs                         | `src/hooks/useGamepadNav.ts`, `src/styles/focus.css`              | T3.6        | C              |
| T3.8  | **AppImage packaging**: Tauri AppImage build configuration, bundled WebKitGTK, CI-friendly build script                                                                               | `src-tauri/tauri.conf.json` (update), `scripts/build-appimage.sh` | T1.1        | A              |
| T3.9  | **AUR PKGBUILD**: Arch Linux packaging metadata for AUR distribution                                                                                                                  | `packaging/PKGBUILD`                                              | T3.8        | D              |
| T3.10 | **CI/CD integration**: GitHub Actions workflow for building, testing, and publishing AppImage + AUR artifacts on release                                                              | `.github/workflows/native-build.yml`                              | T3.8        | D              |
| T3.11 | **CLI argument parsing**: Port `CommandLineParser.cs` (54 lines) to `clap` derive-based parser -- `-p`, `--profile`, `--autolaunch`, `--verbose`, `--json`                            | `crates/crosshook-cli/src/args.rs`                                | T1.15       | A              |
| T3.12 | **Structured logging**: Integrate `tracing` + `tracing-subscriber` for file and console output at `~/.local/share/crosshook/logs/` -- port of `AppDiagnostics.cs` pattern (127 lines) | `crates/crosshook-core/src/logging.rs`                            | T1.1        | A              |

**Parallelization Map (Phase 3)**:

- **Group A** (T3.1, T3.2, T3.6, T3.8, T3.11, T3.12): All independent, fully parallel.
- **Group B** (T3.3, T3.4): Settings IPC, after Group A.
- **Group C** (T3.5, T3.7): UI, after Group B.
- **Group D** (T3.9, T3.10): Packaging, after T3.8.

---

### Phase 4: Community Features

**Goal**: Community profile sharing and extended integration.
**Estimated Tasks**: 6
**Estimated Duration**: 4-6 weeks
**Dependencies**: Phase 3 packaging complete.

| ID   | Task                                                                                                                                                                    | Files Touched                                                                                 | Depends On | Parallel Group |
| ---- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------- | ---------- | -------------- |
| T4.1 | **Community profile JSON schema**: Define JSON schema for shareable profiles with compatibility metadata (game version, trainer version, Proton version, platform tags) | `crates/crosshook-core/src/profile/community_schema.rs`, `schemas/community-profile.json`     | T1.3       | A              |
| T4.2 | **Profile import/export**: Import community JSON profiles into local TOML, export local profiles to community JSON format with validation                               | `crates/crosshook-core/src/profile/exchange.rs`                                               | T4.1       | B              |
| T4.3 | **Git-based profile sharing**: "Taps" system -- clone/pull Git repos containing community profiles, index available profiles, manage tap subscriptions                  | `crates/crosshook-core/src/community/taps.rs`, `crates/crosshook-core/src/community/index.rs` | T4.1       | B              |
| T4.4 | **Tauri IPC commands (community)**: Wire `community_add_tap`, `community_list_profiles`, `community_import_profile`, `community_sync`                                   | `src-tauri/src/commands/community.rs`                                                         | T4.2, T4.3 | C              |
| T4.5 | **React profile browser UI**: Searchable/filterable list of community profiles from taps, import action, compatibility badges, tap management                           | `src/components/CommunityBrowser.tsx`, `src/hooks/useCommunityProfiles.ts`                    | T4.4       | D              |
| T4.6 | **Trainer compatibility database viewer**: Display known trainer-game compatibility data from community profiles, filter by game/trainer/platform                       | `src/components/CompatibilityViewer.tsx`                                                      | T4.4       | D              |

**Parallelization Map (Phase 4)**:

- **Group A** (T4.1): Schema definition, standalone.
- **Group B** (T4.2, T4.3): Exchange + taps, parallel after schema.
- **Group C** (T4.4): IPC, after backends.
- **Group D** (T4.5, T4.6): UI, parallel after IPC.

---

## Task Granularity Recommendations

### Tasks That Should NOT Be Combined

1. **VDF parser (T2.1) must stay separate from library discovery (T2.3)**: The VDF parser is 125 lines of recursive-descent C# with edge cases around quoted string escapes, unquoted tokens, `//` comments, and case-insensitive keys. It needs its own unit tests. The `steam-vdf-parser` crate should be evaluated first -- if it handles case-insensitive key lookups (the C# code uses `StringComparer.OrdinalIgnoreCase` dictionary), it replaces this entire task.

2. **Profile legacy reader (T1.5) must stay separate from TOML reader (T1.6)**: Different serialization formats, different edge cases (the legacy format splits on first `=` with no quoting), and different test strategies.

3. **Launch request validation (T1.8) must stay separate from script invocation (T1.7)**: The validation logic is pure (no I/O) and independently testable, mirroring the C# `Validate()`-then-`Execute()` pattern.

### Tasks That Could Be Combined If Needed

- T1.8 + T1.9 (validation + env constants): Both are small, pure data modules.
- T3.1 + T3.2 (settings + recent files): Both are simple TOML-based persistence, ~210 lines combined in C#.
- T3.9 + T3.10 (PKGBUILD + CI): Both are packaging/infrastructure tasks.

### Largest Porting Targets (Require Sub-Task Discipline)

| C# Source                               | Lines | Rust Destination                  | Risk                                                                                                    |
| --------------------------------------- | ----- | --------------------------------- | ------------------------------------------------------------------------------------------------------- |
| `SteamAutoPopulateService.cs`           | 1,286 | 5 modules (T2.1-T2.9)             | Highest -- recursive VDF parser, multi-library discovery, Proton alias matching with heuristic fallback |
| `SteamLaunchService.cs`                 | 737   | 3 modules (T1.7-T1.9)             | Medium -- path conversion logic is WINE-specific and NOT needed in native app (paths are already Unix)  |
| `SteamExternalLauncherExportService.cs` | 350   | 1 module (T2.12)                  | Low -- straightforward string generation                                                                |
| `ProfileService.cs`                     | 225   | 2 modules (T1.5, T1.6)            | Low -- simple format                                                                                    |
| `MainForm.cs` (launch workflow)         | ~300  | 2 React components (T1.13, T1.14) | Medium -- two-phase state machine needs careful state modeling                                          |

---

## Dependency Analysis

### Cross-Phase Dependencies

```
Phase 1 Foundation                   Phase 2 Discovery              Phase 3 Polish           Phase 4 Community
=====================               ====================           ==============           =================
T1.1 Workspace scaffold --------+-> T2.1 VDF parser                T3.1 Settings            T4.1 JSON schema
T1.2 React scaffold             |   T2.2 Steam root discovery      T3.2 Recent files        T4.2 Import/Export
T1.3 ProfileData model ---------+-> T2.4 Steam data models         T3.6 Dark theme          T4.3 Git taps
T1.5 Legacy profile reader      |   T2.12 Launcher export          T3.8 AppImage            T4.4 IPC commands
T1.6 TOML profile reader        |                                  T3.11 CLI args           T4.5 Browser UI
T1.7 Script invocation ---------|-> T2.12 Launcher export          T3.12 Logging
T1.10 Tauri IPC (profile) ------|-> T3.3 Auto-load startup
T1.11 Tauri IPC (launch) ------+
T1.15 CLI launcher
```

### Hard Blockers

- **T1.1 blocks everything in Rust**: No crate structure, no code.
- **T1.2 blocks everything in React**: No frontend scaffolding.
- **T2.1 (VDF parser) blocks T2.3, T2.5, T2.6, T2.7**: All discovery and matching depends on parsing VDF/ACF files.
- **T2.8 (auto-populate orchestrator) blocks T2.10 and T2.11**: IPC layer cannot expose auto-populate without the orchestrator.

### Soft Dependencies (Can Be Stubbed)

- T1.12 (profile editor) can use hardcoded data until T1.10 (IPC) is ready.
- T1.13 (launch UI) can show state transitions without actual launch until T1.11 is ready.
- T2.11 (auto-populate UI) can show mock results until T2.10 is connected.

---

## File-to-Task Mapping

### C# Source -> Rust Target Tasks

| C# File                                                                         | Rust Module                                                        | Tasks      |
| ------------------------------------------------------------------------------- | ------------------------------------------------------------------ | ---------- |
| `Services/ProfileService.cs`                                                    | `crosshook-core/src/profile/legacy.rs`                             | T1.5       |
| `Services/ProfileService.cs` (ProfileData class)                                | `crosshook-core/src/profile/models.rs`                             | T1.3       |
| `Services/AppSettingsService.cs`                                                | `crosshook-core/src/settings/mod.rs`                               | T3.1       |
| `Services/RecentFilesService.cs`                                                | `crosshook-core/src/settings/recent.rs`                            | T3.2       |
| `Services/CommandLineParser.cs`                                                 | `crosshook-cli/src/args.rs` (via `clap`)                           | T3.11      |
| `Services/SteamLaunchService.cs` (Validate)                                     | `crosshook-core/src/launch/request.rs`                             | T1.8       |
| `Services/SteamLaunchService.cs` (CreateHelperStartInfo)                        | `crosshook-core/src/launch/script_runner.rs`                       | T1.7       |
| `Services/SteamLaunchService.cs` (GetEnvironmentVariablesToClear)               | `crosshook-core/src/launch/env.rs`                                 | T1.9       |
| `Services/SteamLaunchService.cs` (ConvertToUnixPath, dosdevices)                | **NOT PORTED** -- native app runs on Linux, paths are already Unix | --         |
| `Services/SteamAutoPopulateService.cs` (ParseKeyValue\*)                        | `crosshook-core/src/steam/vdf.rs`                                  | T2.1       |
| `Services/SteamAutoPopulateService.cs` (DiscoverSteamRootCandidates)            | `crosshook-core/src/steam/discovery.rs`                            | T2.2       |
| `Services/SteamAutoPopulateService.cs` (DiscoverSteamLibraries)                 | `crosshook-core/src/steam/libraries.rs`                            | T2.3       |
| `Services/SteamAutoPopulateService.cs` (FindGameMatch)                          | `crosshook-core/src/steam/manifest.rs`                             | T2.5       |
| `Services/SteamAutoPopulateService.cs` (ResolveProtonPath, DiscoverCompatTools) | `crosshook-core/src/steam/proton.rs`                               | T2.6, T2.7 |
| `Services/SteamAutoPopulateService.cs` (AttemptAutoPopulate)                    | `crosshook-core/src/steam/auto_populate.rs`                        | T2.8       |
| `Services/SteamAutoPopulateService.cs` (data types)                             | `crosshook-core/src/steam/models.rs`                               | T2.4       |
| `Services/SteamExternalLauncherExportService.cs`                                | `crosshook-core/src/export/launcher.rs`                            | T2.12      |
| `Forms/MainForm.cs` (UpdateSteamModeUiState)                                    | `src/components/LaunchPanel.tsx`                                   | T1.13      |
| `Forms/MainForm.cs` (StreamSteamHelperLogAsync)                                 | `src/components/ConsoleView.tsx`                                   | T1.14      |
| `Forms/MainForm.cs` (BuildSteamLaunchRequest)                                   | `src-tauri/src/commands/launch.rs`                                 | T1.11      |
| `Forms/MainFormStartupCoordinator.cs`                                           | `src-tauri/src/startup.rs`                                         | T3.3       |
| `Diagnostics/AppDiagnostics.cs`                                                 | `crosshook-core/src/logging.rs`                                    | T3.12      |

### Shell Scripts -> Bundling

| Shell Script                                               | Disposition                          | Task  |
| ---------------------------------------------------------- | ------------------------------------ | ----- |
| `runtime-helpers/steam-launch-helper.sh` (326 lines)       | Direct reuse, bundled into Tauri app | T1.16 |
| `runtime-helpers/steam-launch-trainer.sh` (128 lines)      | Direct reuse, bundled into Tauri app | T1.16 |
| `runtime-helpers/steam-host-trainer-runner.sh` (178 lines) | Direct reuse, bundled into Tauri app | T1.16 |

### Code NOT Ported (Intentional Omissions)

| C# File                                                                                                         | Reason                                                                                          |
| --------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------- |
| `Core/ProcessManager.cs`                                                                                        | Win32 P/Invoke process lifecycle -- not needed for MVP; native app delegates to shell scripts   |
| `Injection/InjectionManager.cs`                                                                                 | Win32 `CreateRemoteThread`/`LoadLibraryA` DLL injection -- not relevant to Linux native app     |
| `Memory/MemoryManager.cs`                                                                                       | Win32 `ReadProcessMemory`/`WriteProcessMemory` -- future Phase 5+ if needed via `/proc/pid/mem` |
| `Interop/Kernel32Interop.cs`                                                                                    | Win32 interop declarations -- entirely platform-specific                                        |
| `Interop/Win32ErrorHelper.cs`                                                                                   | Win32 error formatting -- not applicable                                                        |
| `UI/ResumePanel.cs`                                                                                             | WinForms overlay control -- replaced by React components                                        |
| `Program.cs`                                                                                                    | Single-instance Mutex, WinForms startup -- replaced by Tauri lifecycle                          |
| `SteamLaunchService.cs` (path conversion: `ConvertToUnixPath`, `ConvertToWindowsPath`, `ResolveDosDevicesPath`) | WINE path translation is not needed when running natively on Linux                              |

---

## Optimization Opportunities

### 1. Crate Evaluation Can Eliminate Porting Work

- **`steam-vdf-parser` crate**: If it supports case-insensitive key lookup (the C# `SteamKeyValueNode.Children` uses `StringComparer.OrdinalIgnoreCase`), it replaces 125 lines of hand-rolled parser code (T2.1). **Validate before implementing.**
- **`steam_shortcuts_util` crate**: Could simplify non-Steam shortcut integration in Phase 4.
- **`clap` derive macro**: Replaces the entire manual `CommandLineParser.cs` with a 20-line struct definition (T3.11).

### 2. Path Conversion Code is Eliminated

The native app runs on Linux, so the following C# code (~200 lines total) has no Rust equivalent and should not be ported:

- `SteamLaunchService.ConvertToUnixPath` / `ConvertToWindowsPath` (lines 271-349)
- `SteamLaunchService.ResolveDosDevicesPath` / `ResolveDosDeviceLinkTarget` (lines 366-467)
- `SteamLaunchService.LooksLikeWindowsPath` (lines 357-364)
- `SteamAutoPopulateService.NormalizePathForHostLookup` WINE path handling (lines 927-954)

### 3. Env Var Stripping is Simpler Natively

The C# `ApplyCleanSteamEnvironment` removes vars from a `ProcessStartInfo.Environment` dictionary because the WinForms app runs inside WINE and inherits WINE vars. The native app runs outside WINE, so the env var stripping is only needed for the `Command::new()` that invokes `proton run` -- but the shell scripts already handle this independently. The Rust wrapper (T1.7) should still clear vars for defense-in-depth, but it is simpler than the C# version.

### 4. Flatpak Steam Path Detection is a Net-New Feature

The C# codebase does NOT check `~/.var/app/com.valvesoftware.Steam/`. The Rust `DiscoverSteamRootCandidates` (T2.2) should add this as a new search path, filling a gap documented in the feature spec edge cases table.

---

## Implementation Strategy Recommendations

### Start With the "Golden Path" End-to-End

Before parallelizing, one developer should complete the minimum vertical slice:

1. T1.1 (workspace) -> T1.3 (models) -> T1.5 (legacy reader) -> T1.7 (script runner) -> T1.16 (script bundling) -> manual CLI test
2. This proves the Rust -> shell script -> Proton pipeline works natively before any UI is built.

### Use the Feature Spec's Recommended Architecture

The `shared.md` and `feature-spec.md` define clear patterns:

- **Request/Result pattern**: Every service method uses `struct XxxRequest -> fn validate() -> Result<(), Error> -> fn execute() -> Result<XxxResult, Error>`. Maintain this in Rust.
- **Static services become module-level functions**: C# `static class SteamAutoPopulateService` becomes `pub mod auto_populate` with free functions.
- **Stateful services become structs**: C# `ProfileService(startupPath)` becomes `pub struct ProfileStore { base_path: PathBuf }`.

### Test Strategy

- **Unit tests for all pure logic**: VDF parser, profile serialization, env var list, validation, path matching.
- **Integration tests with fixture files**: Create `.profile`, `.vdf`, `.acf` fixture files in a `tests/fixtures/` directory. The C# codebase has no tests, so the Rust port is the opportunity to add coverage.
- **Manual E2E test**: Load a real profile, invoke `steam-launch-helper.sh` with `--game-only`, verify log output streams to UI.

### Risk Mitigation

| Risk                                                            | Mitigation                                                                                                                  |
| --------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------- |
| `steam-vdf-parser` crate does not support case-insensitive keys | Evaluate crate in T2.1 before committing; keep hand-rolled parser as fallback                                               |
| Shell script paths differ between dev and Tauri bundle          | T1.16 must resolve paths via `tauri::api::path::resolve_resource` at runtime                                                |
| Two-phase launch state machine is complex in React              | Use `useReducer` with explicit `LaunchPhase` enum (Idle, GameLaunching, WaitingForTrainer, TrainerLaunching, SessionActive) |
| Log streaming perf with large helper logs                       | Use Tauri events (not polling) with debounced UI updates                                                                    |

---

## Relevant Source Files

- `/home/yandy/Projects/github.com/yandy-r/crosshook/docs/plans/platform-native-ui/feature-spec.md`: Master specification with architecture, data models, Rust traits, phased plan
- `/home/yandy/Projects/github.com/yandy-r/crosshook/docs/plans/platform-native-ui/shared.md`: File inventory, patterns, and cross-references
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/CrossHookEngine.App/Services/SteamAutoPopulateService.cs`: Largest porting target (1,286 lines)
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/CrossHookEngine.App/Services/SteamLaunchService.cs`: Launch orchestration (737 lines, ~200 lines NOT ported)
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/CrossHookEngine.App/Services/SteamExternalLauncherExportService.cs`: Launcher export (350 lines)
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/CrossHookEngine.App/Services/ProfileService.cs`: Profile CRUD (225 lines)
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/CrossHookEngine.App/Services/AppSettingsService.cs`: Settings persistence (81 lines)
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/CrossHookEngine.App/Services/RecentFilesService.cs`: MRU tracking (131 lines)
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/CrossHookEngine.App/Services/CommandLineParser.cs`: CLI args (54 lines)
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/CrossHookEngine.App/Forms/MainForm.cs`: Lines 2119-2153 (two-phase state), 2648-2946 (launch workflow)
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/CrossHookEngine.App/Forms/MainFormStartupCoordinator.cs`: Auto-load profile (34 lines)
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/CrossHookEngine.App/Diagnostics/AppDiagnostics.cs`: Logging pattern (127 lines)
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/CrossHookEngine.App/runtime-helpers/steam-launch-helper.sh`: Game+trainer orchestrator (326 lines, reused directly)
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/CrossHookEngine.App/runtime-helpers/steam-launch-trainer.sh`: Trainer-only launcher (128 lines, reused directly)
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/CrossHookEngine.App/runtime-helpers/steam-host-trainer-runner.sh`: Clean-env trainer runner (178 lines, reused directly)
- `/home/yandy/Projects/github.com/yandy-r/crosshook/tasks/lessons.md`: Critical runtime gotchas for WINE path handling, env var stripping
