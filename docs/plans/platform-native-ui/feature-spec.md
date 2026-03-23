# Feature Spec: Platform-Native Linux UI for CrossHook

## Executive Summary

CrossHook's native Linux UI replaces the C#/WinForms application with a platform-native application that operates **outside** the WINE environment, solving the fundamental problem where trainers running inside WINE cannot access game memory. The app wraps the existing, proven shell-script-based trainer launch pipeline (`steam-launch-helper.sh`, `steam-host-trainer-runner.sh`) in a clean desktop and controller-friendly UI, providing Steam library auto-discovery, profile management, and process orchestration. The technology stack is **Tauri v2** (Rust backend + React/TypeScript frontend via Vite), targeting Linux first then macOS and Windows. Distribution is via **AppImage** to avoid Flatpak sandbox restrictions that block `/proc` and `ptrace` access. The primary risk is maintaining two codebases (WinForms + native), mitigated by freezing the WinForms app at its current feature set.

## External Dependencies

### APIs and Services

#### Steam CLI / steam:// Protocol

- **Documentation**: [Valve Developer Community - Steam Browser Protocol](https://developer.valvesoftware.com/wiki/Steam_browser_protocol) | [Command Line Options](https://developer.valvesoftware.com/wiki/Command_line_options)
- **Authentication**: None (local IPC)
- **Key Commands**:
  - `steam -applaunch <appid>`: Launch game from CLI (preferred, already used by existing scripts)
  - `steam steam://rungameid/<appid>`: Launch via URI scheme
  - `steam steam://install/<appid>`: Trigger game installation
- **Rate Limits**: None (local)
- **Pricing**: Free

#### Direct Proton CLI Execution

- **Documentation**: [Proton FAQ](https://github.com/ValveSoftware/Proton/wiki/Proton-FAQ)
- **Key Pattern**: `proton run <trainer.exe>` with clean environment (`STEAM_COMPAT_DATA_PATH`, `STEAM_COMPAT_CLIENT_INSTALL_PATH`, `WINEPREFIX`)
- **Critical**: Must strip ~30 inherited WINE/Proton environment variables before launching

#### Linux Process Memory APIs

- **Documentation**: [ptrace(2)](https://man7.org/linux/man-pages/man2/ptrace.2.html) | [process_vm_readv(2)](https://man7.org/linux/man-pages/man2/process_vm_readv.2.html)
- **Key Syscalls**: `process_vm_readv`/`process_vm_writev` for bulk memory access (no ptrace attach needed), `/proc/<pid>/maps` for memory layout, `/proc/<pid>/mem` for file-based access
- **Constraint**: `kernel.yama.ptrace_scope` defaults to 1 on most distros; primary trainer workflow (Proton run) is unaffected

### Libraries and SDKs

| Library                | Version | Purpose                                                             | Installation                              |
| ---------------------- | ------- | ------------------------------------------------------------------- | ----------------------------------------- |
| `nix`                  | 0.29+   | Safe Rust wrappers for Linux syscalls (ptrace, signals, process_vm) | `cargo add nix --features ptrace,process` |
| `steam-vdf-parser`     | latest  | Parse Steam VDF/ACF files (zero-copy, binary support)               | `cargo add steam-vdf-parser`              |
| `steam_shortcuts_util` | latest  | Read/write Steam shortcuts.vdf (non-Steam game shortcuts)           | `cargo add steam_shortcuts_util`          |
| `tokio`                | 1.x     | Async runtime for I/O operations                                    | `cargo add tokio --features full`         |
| `serde` + `toml`       | latest  | Profile and settings serialization                                  | `cargo add serde toml`                    |
| `clap`                 | 4.x     | CLI argument parsing                                                | `cargo add clap --features derive`        |
| `tracing`              | latest  | Structured logging                                                  | `cargo add tracing tracing-subscriber`    |

**UI Framework** (decided):

- **Tauri v2** (2.x) — Rust backend with IPC to frontend
- **React + TypeScript** via Vite — frontend UI
- Uses system WebView: WebKitGTK (Linux), WebKit (macOS), WebView2 (Windows)

### External Documentation

- [ArchWiki: Steam](https://wiki.archlinux.org/title/Steam): Steam path conventions on Linux
- [ArchWiki: Gamescope](https://wiki.archlinux.org/title/Gamescope): Steam Deck compositor integration
- [Yama LSM](https://docs.kernel.org/admin-guide/LSM/Yama.html): ptrace security restrictions
- [Flatpak Sandbox Docs](https://docs.flatpak.org/en/latest/sandbox-permissions.html): Why Flatpak is not viable

## Business Requirements

### User Stories

**Primary User: Steam Deck Gamer**

- As a Steam Deck gamer, I want to select a game and trainer from a simple interface so that I can launch them together without writing shell scripts
- As a Steam Deck gamer, I want CrossHook to auto-detect my Steam libraries, games, and Proton versions so that I don't need to manually locate file paths
- As a Steam Deck gamer, I want saved profiles to remember my configuration so that I can replay the same setup with one click
- As a Steam Deck gamer, I want the UI to work with a gamepad and touchscreen so that I can use it in Gaming Mode without a keyboard

**Secondary User: Linux Desktop Gamer**

- As a Linux desktop gamer, I want a native application that integrates with my desktop environment (system tray, notifications, .desktop launchers) so that it feels like a first-class Linux tool
- As a Linux desktop gamer, I want to manage multiple game profiles with different trainers and Proton versions
- As a Linux desktop gamer, I want to export standalone launcher scripts from my profiles so I can run trainers without opening the full app

**Tertiary User: Modding Enthusiast**

- As a modding enthusiast, I want a console/log view showing exactly what commands are being executed so I can diagnose failures
- As a modding enthusiast, I want to configure DLL paths and injection parameters alongside trainer settings

### Business Rules

1. **Trainer Must Run Outside WINE**: The trainer process must be launched from the native Linux environment using Proton directly against the game's compatdata prefix. This is the fundamental constraint that motivates the native UI.
   - Validation: Launch command uses `$PROTON run "$TRAINER_PATH"` with a clean environment
   - Exception: None

2. **Game Must Be Launched via Steam**: In Steam mode, games launch through `steam -applaunch <appid>` to properly initialize DRM, overlay, and Proton runtime.
   - Validation: Steam App ID must be valid and game must be installed
   - Exception: Direct mode for non-Steam games

3. **Trainer Staged into Compatdata**: Trainer executable must be copied into the game's compatdata prefix at `pfx/drive_c/CrossHook/StagedTrainers/` before Proton runs it.
   - Validation: Staged path must exist and file must be copied before launch
   - Exception: Trainers already within the compatdata prefix

4. **Clean Environment Required**: All WINE/Proton-specific environment variables (~30) must be stripped before launching a trainer. Only `STEAM_COMPAT_DATA_PATH`, `STEAM_COMPAT_CLIENT_INSTALL_PATH`, and `WINEPREFIX` should be set.
   - Validation: Launch process explicitly unsets all inherited WINE variables
   - Exception: None — inherited variables cause silent failures

5. **Two-Phase Steam Launch**: Workflow is always: (1) launch game via Steam, (2) wait for game to reach menu, then launch trainer. The UI must enforce this sequencing.
   - Validation: UI toggles between "Launch Game" and "Launch Trainer" states
   - Exception: Trainer-only mode (game already running) and game-only mode

6. **Profile Compatibility**: Profile data must remain compatible with the existing `.profile` key=value format for cross-compatibility with the WinForms app. New formats may be added with a migration path.
   - Fields: GamePath, TrainerPath, Dll1Path, Dll2Path, LaunchInject1, LaunchInject2, LaunchMethod, UseSteamMode, SteamAppId, SteamCompatDataPath, SteamProtonPath, SteamLauncherIconPath

### Edge Cases

| Scenario                                    | Expected Behavior                                           | Notes                                                  |
| ------------------------------------------- | ----------------------------------------------------------- | ------------------------------------------------------ |
| Multiple Steam libraries (different drives) | Auto-populate scans all libraries from `libraryfolders.vdf` | Already handled in existing `SteamAutoPopulateService` |
| Ambiguous App ID matching                   | Report ambiguity, don't guess                               | Return `Ambiguous` state                               |
| Custom Proton (GE-Proton, TKG)              | Search `compatibilitytools.d/` alongside official paths     | Both `~/.steam/root/` and system paths                 |
| Compatdata not yet created                  | Inform user to launch game through Steam once first         | Diagnostic hint, not hard failure                      |
| Flatpak Steam install                       | Additionally check `~/.var/app/com.valvesoftware.Steam/`    | Not handled by current C# code                         |
| Trainer already running                     | Detect via `pgrep`, skip re-launch                          | Shell scripts already do this                          |
| Steam Deck immutable filesystem             | Distribute as AppImage (survives OS updates)                | Avoid native packages on SteamOS                       |

### Success Criteria

- [ ] Native Linux app launches trainers with same reliability as manual shell scripts
- [ ] Steam library auto-detection works for standard, Flatpak, and multi-drive installs
- [ ] Proton version auto-detection resolves official and custom Proton builds
- [ ] Profiles saved by WinForms app can be loaded by native app (format compatibility)
- [ ] Two-phase Steam launch workflow enforced by UI
- [ ] External launcher export produces functional `.sh` and `.desktop` files
- [ ] App is usable on Steam Deck in Desktop Mode; aspirationally in Gaming Mode
- [ ] Console/log view surfaces helper script output in real-time
- [ ] Error messages are actionable (specific path/config that failed)
- [ ] App memory footprint < 100MB idle (< 50MB target for Tauri)

## Technical Specifications

### Architecture Overview

```
                           +--------------------------+
                           |   crosshook-cli          |
                           |   (headless / scripted)  |
                           +-----------+--------------+
                                       |
                 +---------------------+---------------------+
                 |                                           |
    +------------v-----------+                  +------------v-----------+
    |   UI Application       |                  |   D-Bus Service        |
    |   (GTK4 or Tauri)      |                  |   (notifications,     |
    |                        |                  |    status updates)     |
    +------------+-----------+                  +------------------------+
                 |
    +------------v-----------+
    |    crosshook-core      |
    |  (shared Rust library) |
    +--+--------+--------+--+
       |        |        |
+------v-+ +---v-----+ +--v-----------+
| Process | | Memory  | | Injection    |
| Manager | | Manager | | Manager      |
| (/proc, | | (/proc/ | | (Proton run, |
| signals)| | pid/mem)| | LD_PRELOAD)  |
+---------+ +---------+ +--------------+
       |        |        |
+------v--------v--------v--------------+
|       Linux Kernel APIs                |
| /proc  ptrace  signals  pidfd          |
+----------------------------------------+
       |
+------v---------------------------------+
|   Steam / Proton Integration           |
| libraryfolders.vdf  appmanifest_*.acf  |
| compatdata  proton run                 |
+----------------------------------------+
```

### Data Models

#### Game Profile (TOML — native format, with legacy `.profile` import)

```toml
# ~/.config/crosshook/profiles/elden-ring.toml
[game]
name = "Elden Ring"
executable_path = "/mnt/games/SteamLibrary/steamapps/common/ELDEN RING/Game/eldenring.exe"

[trainer]
path = "/home/user/trainers/EldenRing_FLiNG.exe"
type = "fling"  # fling | wemod | generic

[injection]
dll_paths = []
inject_on_launch = [false, false]

[steam]
enabled = true
app_id = "1245620"
compatdata_path = "/mnt/games/SteamLibrary/steamapps/compatdata/1245620"
proton_path = "/mnt/games/SteamLibrary/steamapps/common/Proton 9.0-4/proton"

[steam.launcher]
icon_path = ""
display_name = "Elden Ring"

[launch]
method = "proton_run"  # proton_run | direct | steam_applaunch
```

#### Win32 P/Invoke to Linux Equivalents (Key Mappings)

| Win32 API                        | Linux Equivalent                         | Notes                                  |
| -------------------------------- | ---------------------------------------- | -------------------------------------- |
| `CreateProcess`                  | `fork()` + `execve()` / `Command::new()` | For Proton: `proton run <exe>`         |
| `OpenProcess`                    | `/proc/<pid>/` filesystem                | UID match + `CAP_SYS_PTRACE`           |
| `SuspendThread` / `ResumeThread` | `SIGSTOP` / `SIGCONT` via `tgkill()`     | Per-thread control                     |
| `ReadProcessMemory`              | `pread()` on `/proc/<pid>/mem`           | Requires same-user or `CAP_SYS_PTRACE` |
| `WriteProcessMemory`             | `pwrite()` on `/proc/<pid>/mem`          | Same permission requirements           |
| `VirtualQueryEx`                 | Parse `/proc/<pid>/maps`                 | Memory region discovery                |
| `VirtualAllocEx`                 | ptrace + `mmap()` syscall injection      | Complex but documented                 |
| `CreateRemoteThread`             | ptrace register hijack → `dlopen()`      | Or use `LD_PRELOAD` at launch          |

**Key insight**: The primary Steam workflow does NOT use CreateRemoteThread/LoadLibraryA injection. It uses `proton run <trainer.exe>`. The native app should prioritize this proven pattern.

### API Design (Internal Rust Traits)

```rust
pub trait ProcessControl {
    fn launch(&mut self, config: &LaunchConfig) -> Result<ProcessHandle>;
    fn attach(&mut self, pid: u32) -> Result<ProcessHandle>;
    fn is_running(&self) -> bool;
    fn suspend(&self) -> Result<()>;
    fn resume(&self) -> Result<()>;
    fn kill(&mut self) -> Result<()>;
}

pub trait SteamDiscovery {
    fn discover_libraries(&self) -> Result<Vec<SteamLibrary>>;
    fn find_game(&self, game_path: &Path) -> Result<Option<SteamGameManifest>>;
    fn discover_proton_installs(&self) -> Result<Vec<ProtonInstall>>;
    fn auto_populate(&self, game_path: &Path) -> Result<SteamAutoPopulateResult>;
}

pub trait ProfileStore {
    fn list(&self) -> Result<Vec<String>>;
    fn load(&self, name: &str) -> Result<GameProfile>;
    fn save(&self, name: &str, profile: &GameProfile) -> Result<()>;
    fn import_legacy(&self, profile_path: &Path) -> Result<GameProfile>;
}

pub trait SteamLauncher {
    fn launch_game(&self, request: &SteamLaunchRequest) -> Result<SteamLaunchResult>;
    fn launch_trainer(&self, request: &SteamLaunchRequest) -> Result<SteamLaunchResult>;
    fn export_launcher_scripts(&self, request: &LauncherExportRequest) -> Result<LauncherExportResult>;
}
```

### CLI Interface

```
crosshook [OPTIONS] <COMMAND>

Commands:
  launch     Launch a game with optional trainer
  profile    Manage game profiles (list, load, save, delete, import)
  steam      Steam integration (discover, auto-populate, export)
  status     Show status of tracked processes

Options:
  -p, --profile <NAME>    Load a profile by name
  -v, --verbose           Increase log verbosity
  --json                  Output in JSON format (for scripting)

Examples:
  crosshook launch --profile elden-ring
  crosshook steam discover
  crosshook steam auto-populate --game-path /path/to/game.exe
  crosshook profile import --legacy-path /path/to/old.profile
```

### System Integration

#### Files to Create

New `src/crosshook-linux/` directory within the existing monorepo:

```
src/crosshook-native/
  Cargo.toml                    # Workspace root
  crates/
    crosshook-core/src/         # Shared Rust library: process, memory, injection, steam, profiles
    crosshook-cli/src/          # Headless CLI binary
  src-tauri/
    src/                        # Tauri Rust backend (IPC commands, plugin setup)
    Cargo.toml
    tauri.conf.json
  src/                          # React + TypeScript frontend (Vite)
    App.tsx
    components/
    hooks/
    styles/
  package.json
  vite.config.ts
  tsconfig.json
```

#### Files to Preserve/Reference (Porting Sources)

- `src/CrossHookEngine.App/Services/SteamAutoPopulateService.cs` → `steam/auto_populate.rs` (~900 lines, most valuable domain logic)
- `src/CrossHookEngine.App/Services/SteamLaunchService.cs` → `steam/launch.rs` (path conversion, env cleanup)
- `src/CrossHookEngine.App/Services/SteamExternalLauncherExportService.cs` → `steam/launcher_export.rs`
- `src/CrossHookEngine.App/Services/ProfileService.cs` → `profile/` (12-field key=value format)
- `src/CrossHookEngine.App/runtime-helpers/*.sh` → Bundled directly (or absorbed into Rust)

#### Configuration

- Profiles: `~/.config/crosshook/profiles/` (XDG compliant)
- Settings: `~/.config/crosshook/settings.toml`
- Logs: `~/.local/share/crosshook/logs/`
- Launchers: `~/.local/share/crosshook/launchers/`

## UX Considerations

### User Workflows

#### Primary Workflow: Steam Mode Game + Trainer Launch

1. **Select Profile**: User browses game profiles displayed as cover-art cards. Controller: D-pad navigation with focus rings. Desktop: click/keyboard.
   - System: Detail panel shows trainer list, Proton config, last-played timestamp.

2. **Launch Game**: User presses primary action (A button / Enter / double-click).
   - System: Multi-stage progress — "Starting Steam..." → "Launching game..." → "Waiting for game..."
   - System: Launches `steam -applaunch <appid>` and polls for game process via `pgrep`.

3. **Launch Trainer**: UI transitions to "Launch Trainer" state once game process detected.
   - System: Stages trainer into compatdata, strips env vars, runs `proton run`.
   - System: Streams helper log to console panel.

4. **Monitor Session**: Minimal "Now Playing" view with game name, session timer, trainer toggles, health indicator.
   - System: Green/yellow/red status dots. Trainer toggles become interactive.

5. **Session End**: Game or trainer exits.
   - System: Returns to library view with session summary.

#### Error Recovery Workflow

| Error                    | User Message                                         | Recovery Action                             |
| ------------------------ | ---------------------------------------------------- | ------------------------------------------- |
| Steam not running        | "Steam is not running."                              | "Start Steam" button + manual dismiss       |
| Trainer injection failed | "Could not inject [trainer]. May be incompatible."   | "Try Again" + "View Details" expandable     |
| ptrace permission denied | "CrossHook needs permission to attach."              | Step-by-step guide with `setcap` command    |
| Game not found           | "Could not find [game] at expected path."            | "Rescan Library" + "Browse for Game" picker |
| Proton runner missing    | "Configured Proton ([version]) not installed."       | "Select Different Runner" dropdown          |
| Compatdata missing       | "Game must be launched through Steam at least once." | Guidance text                               |

### UI Patterns

| Component       | Pattern                                                       | Notes                                                           |
| --------------- | ------------------------------------------------------------- | --------------------------------------------------------------- |
| Game Library    | Grid view (desktop), Shelf view (controller)                  | Cover-art cards with status indicators                          |
| Navigation      | Sidebar (desktop), Tab bar with bumper switching (controller) | Convergent: adapts to input mode                                |
| Configuration   | Progressive disclosure                                        | Simple toggles default, "Advanced" expandable sections          |
| Launch Progress | Multi-stage named steps                                       | "Starting Steam..." → "Launching..." → "Injecting..." → "Ready" |
| Now Playing     | Minimal dashboard                                             | Session timer, trainer toggles, health dots                     |
| Theme           | Dark by default                                               | Gaming aesthetic, high-contrast accent colors                   |

### Accessibility Requirements

- **Visible Focus**: Every interactive element must have 2px+ focus indicator with 3:1 contrast
- **Keyboard/Controller Navigation**: Full navigability without mouse; Tab, D-pad, Enter/A, Escape/B
- **Adaptive Button Prompts**: Display controller glyphs matching connected device (Xbox/PlayStation/generic)
- **Audio Feedback**: Subtle sounds for focus changes, selections, and errors (controller mode)
- **Reduced Motion**: Respect `prefers-reduced-motion` setting

### Performance UX

- **App Startup**: < 2 seconds to interactive (< 1 second target for native)
- **Profile Switch**: < 100ms to render new profile
- **Memory Footprint**: < 100MB idle (< 50MB target for Tauri, critical as app runs alongside games)
- **Loading States**: Skeleton screens, named multi-stage progress for launches, background library scanning

## Recommendations

### Implementation Approach

**Recommended Strategy**: Build as a **thin orchestration frontend** that delegates actual game/trainer launching to the existing proven Bash scripts. The app's core responsibilities are:

1. Profile management (read/write `.profile` and TOML files)
2. Steam auto-discovery (port `SteamAutoPopulateService.cs` VDF parsing to Rust)
3. Process orchestration (assemble CLI arguments, invoke scripts, stream logs)
4. External launcher export (generate `.desktop` entries and standalone scripts)

This avoids reimplementing any Win32 P/Invoke, DLL injection, or memory management for the MVP.

**Phasing:**

1. **Phase 1 — MVP (4-6 weeks)**: Profile-driven launcher invoking existing Bash scripts. Load/save profiles, browse for paths, launch game+trainer, stream log output. No auto-discovery — users enter paths manually.
2. **Phase 2 — Smart Discovery (3-4 weeks)**: Port `SteamAutoPopulateService` to Rust. Auto-detect Steam libraries, parse manifests, resolve Proton. Add "Auto-Populate" button and external launcher export.
3. **Phase 3 — Polish (3-4 weeks)**: System tray, notifications, Steam Deck controller layout, settings persistence, dark theme. Packaging: AppImage + AUR PKGBUILD.
4. **Phase 4 — Community (4-6 weeks)**: Community profile format (JSON), Git-based profile sharing ("taps" model), trainer compatibility database.

### Technology Decisions

| Decision           | Choice                                                       | Rationale                                                                                                                                   |
| ------------------ | ------------------------------------------------------------ | ------------------------------------------------------------------------------------------------------------------------------------------- |
| UI Framework       | Tauri v2 (Rust + React/TypeScript via Vite)                  | Cross-platform (Linux/macOS/Windows); system WebView (~5-10MB binary); fast UI iteration with React HMR; WebKitGTK pre-installed on SteamOS |
| Backend Language   | Rust                                                         | Zero-cost abstractions for `/proc`/`ptrace`; memory safety critical for process manipulation; single static binary for Steam Deck           |
| Frontend           | React + TypeScript + Vite                                    | Largest ecosystem, best Tauri community support, mature state management for complex launch flows, deep talent pool                         |
| Profile Format     | TOML (with legacy `.profile` reader)                         | Human-readable, supports nesting, excellent `serde` support                                                                                 |
| Process Monitoring | `pidfd_open` + `poll` (primary), `/proc` polling fallback    | Race-free, available on Linux 5.3+ (SteamOS ships 6.1+)                                                                                     |
| Trainer Launch     | `proton run` (primary), ptrace injection (optional advanced) | Proven pattern from existing scripts; ptrace is fragile against WINE internals                                                              |
| Project Structure  | Monorepo (`src/crosshook-native/`)                           | Share scripts, profile format spec, CI infrastructure with WinForms app                                                                     |
| Distribution       | AppImage (primary) + AUR PKGBUILD                            | AppImage: no sandbox, single file, survives SteamOS updates. NOT Flatpak (blocks `/proc`/`ptrace`)                                          |

### Quick Wins

- **Script-only CLI launcher (1-2 days)**: Rust CLI that reads a `.profile` file and invokes `steam-launch-helper.sh`. Headless alternative for Steam Deck Gaming Mode or scripting.
- **Desktop entry generator (1 day)**: Extract `.desktop` file generation into a standalone script for quick shortcut creation.
- **Profile format documentation (1 day)**: Document the `.profile` format so users can create profiles manually.

### Future Enhancements

- **Community Profile Sharing**: JSON profiles with Git-based "taps" — highest-leverage feature per prior research
- **Trainer Auto-Discovery**: Scan common download locations, match trainer filenames to games
- **Proton Version Manager UI**: Show installed versions, highlight per-game version, allow switching
- **Decky Loader Plugin**: Companion plugin exposing trainer toggles in Steam Deck Quick Access Menu
- **Plugin/Extension System**: Trait-based "trainer launcher" plugins for new trainer types

## Risk Assessment

### Technical Risks

| Risk                                                       | Likelihood | Impact | Mitigation                                                                                           |
| ---------------------------------------------------------- | ---------- | ------ | ---------------------------------------------------------------------------------------------------- |
| Bash scripts break on SteamOS updates                      | Medium     | High   | Scripts use only POSIX utilities + Proton's stable `proton run` interface; pin tested versions in CI |
| Flatpak sandboxing blocks `/proc`/`ptrace`                 | High       | High   | Do NOT distribute as Flatpak; use AppImage instead                                                   |
| WINE env variable contract changes between Proton versions | Medium     | Medium | Scripts already strip ALL vars and reconstruct clean env; monitor Proton changelogs                  |
| Two-codebase maintenance burden                            | High       | Medium | Freeze WinForms at current feature set; native app is the future                                     |
| Linux kernel tightens ptrace restrictions                  | Low        | Medium | Primary workflow (Proton run) doesn't use ptrace; only advanced features affected                    |
| WebKitGTK version gaps on minimal distros                  | Low        | Medium | AppImage bundles dependencies; WebKitGTK pre-installed on SteamOS and all major distros              |

### Integration Challenges

- **Steam client detection on immutable distros**: Must check Flatpak's `~/.var/app/com.valvesoftware.Steam/data/Steam` in addition to standard paths
- **Proton symlink resolution**: Existing `dosdevices` resolution logic needed only for legacy profile import
- **Process visibility across WINE boundaries**: WINE processes appear as standard Linux processes (confirmed by existing `pgrep -af` usage); names may be truncated
- **Gamescope integration**: Tauri runs in desktop session, not inside Gamescope (correct — app is a launcher, not overlay)

### Security Considerations

- **ptrace scope**: Document `setcap cap_sys_ptrace+ep` or polkit escalation for advanced features
- **File permissions**: `/proc/<pid>/mem` access requires same UID (normally satisfied since games run as same user)
- **No secrets storage**: App stores file paths and config, not credentials
- **Anti-cheat awareness**: Document which games with EAC/BattlEye may block memory access

## Task Breakdown Preview

### Phase 1: Foundation & MVP

**Focus**: Working profile-driven game+trainer launcher
**Tasks**:

- Initialize Rust workspace with `crosshook-core`, `crosshook-cli`, and UI crate
- Define data models for `ProfileData` (matching existing 12-field format)
- Implement profile reader (legacy `.profile` + TOML) and writer
- Build Rust wrapper around `steam-launch-helper.sh` invocation
- Implement log streaming (tail helper log, push to UI)
- Build UI: profile editor form, two-step launch flow, file/directory pickers, console output
  **Parallelization**: Data model + CLI scaffolding can run alongside UI scaffolding

### Phase 2: Smart Discovery

**Focus**: Feature parity with WinForms Steam workflow
**Dependencies**: Phase 1 core launch must work
**Tasks**:

- Port `SteamAutoPopulateService` VDF parsing to Rust
- Implement Steam library discovery, manifest matching, Proton resolution
- Build "Auto-Populate" button and result display in UI
- Port launcher export (`.sh` + `.desktop` generation)
  **Parallelization**: VDF parser and launcher export are independent

### Phase 3: Polish & Distribution

**Focus**: Steam Deck optimization and release packaging
**Tasks**:

- Settings persistence, recent files, auto-load last profile
- Controller/gamepad navigation support with large touch targets
- System tray integration, desktop notifications
- AppImage build pipeline, AUR PKGBUILD, GitHub Release workflow
- Dark theme polish, responsive layout for 1280x800 (Steam Deck)

### Phase 4: Community Features

**Focus**: Community profiles and extended integration
**Tasks**:

- Community profile JSON schema with compatibility metadata
- Git-based profile sharing ("taps" system)
- Profile import/export UI and search/browse
- Trainer compatibility database viewer

**Estimated Complexity**: ~45-55 discrete tasks across all phases
**Critical Path**: Phase 1 scaffolding → core launch → MVP release → Phase 2 auto-discovery → Phase 3 packaging
**Minimum Viable Release**: End of Phase 1 (profile editor + script-based launcher)
**Competitive Release**: End of Phase 2 (auto-discovery matches WinForms feature parity)

## Decisions Needed

1. **~~UI Framework~~** — **DECIDED: Tauri v2 + React/TypeScript + Vite**
   - Cross-platform (Linux → macOS → Windows), system WebView, ~5-10MB binary, fast UI iteration with React HMR

2. **Profile Format Migration**
   - Options: JSON from day one, TOML from day one, keep legacy `Key=Value` indefinitely
   - Impact: JSON/TOML enables community profiles with nested structures; legacy format has only 12 flat fields
   - Recommendation: **TOML** with backward-compatible reader for legacy `.profile` files

3. **Bash Script Bundling vs Inlining**
   - Options: Keep scripts external (Phase 1-2), absorb into Rust (Phase 3+)
   - Impact: External = proven, debuggable, independently testable. Inline = cleaner distribution, single binary
   - Recommendation: Keep external for Phase 1-2, evaluate inlining in Phase 3

4. **WinForms App Disposition**
   - Options: Freeze at current features, continue development in parallel, sunset immediately
   - Impact: Affects maintenance burden, feature parity requirements
   - Recommendation: **Freeze** at current feature set. Defer sunset until native app reaches Phase 2 and adoption data is available.

5. **Monorepo vs Separate Repository**
   - Options: `src/crosshook-linux/` in existing repo, new repository
   - Impact: Shared CI, scripts, and docs vs clean separation
   - Recommendation: **Monorepo** — share launcher scripts, profile spec, and issue tracking

## Research References

For detailed findings, see:

- [research-external.md](./research-external.md): Steam APIs, Linux process APIs, UI frameworks, VDF parsers, distribution strategies
- [research-business.md](./research-business.md): User stories, business rules, workflows, domain model, existing codebase analysis
- [research-technical.md](./research-technical.md): Architecture design, Win32→Linux API mapping, data models, Rust trait APIs, packaging
- [research-ux.md](./research-ux.md): UI patterns, competitive analysis (Lutris, Heroic, Bottles, WeMod, Playnite), Steam Deck UX, error handling
- [research-recommendations.md](./research-recommendations.md): Framework comparison, phasing strategy, risk assessment, task breakdown
