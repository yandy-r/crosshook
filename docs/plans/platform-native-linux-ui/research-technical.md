# Technical Specifications: platform-native-linux-ui

## Executive Summary

CrossHook's native Linux UI replaces the WinForms+Win32 P/Invoke stack with a Linux-native application that operates outside WINE, using `/proc`, `ptrace`, and direct Proton invocation to manage game trainers. The recommended architecture is a Rust application with GTK4 for the UI, a service layer that replicates ProcessManager/InjectionManager/MemoryManager using Linux kernel interfaces, and a CLI mode for headless Steam Deck usage. The existing runtime-helper shell scripts (`steam-launch-helper.sh`, `steam-host-trainer-runner.sh`) already prove the core pattern works natively; this project wraps that pattern in a full application with profile management, Steam library discovery, and process lifecycle control.

## Architecture Design

### Component Diagram

```
                                +--------------------------+
                                |    crosshook-linux CLI   |
                                |   (headless / scripted)  |
                                +-----------+--------------+
                                            |
                  +-------------------------+-------------------------+
                  |                                                   |
     +------------v-----------+                          +------------v-----------+
     |   GTK4 UI Application  |                          |    D-Bus Service       |
     |   (crosshook-gtk)      |                          |    (notifications,     |
     |                        |                          |     status updates)    |
     +------------+-----------+                          +------------------------+
                  |
     +------------v-----------+
     |    Application Core    |
     |  (shared library/crate)|
     +--+--------+--------+--+
        |        |        |
+-------v--+ +--v------+ +--v-----------+
| Process   | | Memory  | | Injection    |
| Manager   | | Manager | | Manager      |
| (/proc,   | | (/proc/ | | (LD_PRELOAD, |
|  signals, | |  pid/mem)| |  ptrace,     |
|  waitpid) | |         | |  Proton run) |
+-----------+ +---------+ +--------------+
        |        |        |
+-------v--------v--------v--------------+
|          Linux Kernel APIs              |
|  /proc  ptrace  signals  memfd         |
+-----------------------------------------+
        |
+-------v---------------------------------+
|    Steam / Proton Integration           |
|  libraryfolders.vdf  appmanifest_*.acf  |
|  compatdata  proton run                 |
+-----------------------------------------+
```

### New Components

- **crosshook-core** (Rust library crate): Platform-abstracted service layer containing ProcessManager, InjectionManager, MemoryManager, ProfileService, SteamDiscoveryService, and SteamLaunchService. All business logic lives here, shared between CLI and GUI.
- **crosshook-gtk** (Rust binary crate): GTK4 application providing the graphical interface. Replaces WinForms MainForm. Uses libadwaita for modern GNOME integration and adaptive layouts (relevant for Steam Deck's 1280x800 display).
- **crosshook-cli** (Rust binary crate): Headless CLI tool for scripted and SSH-based operation. Mirrors all core functionality without a display server dependency.
- **crosshook-dbus** (module within crosshook-core): D-Bus client for desktop notifications (org.freedesktop.Notifications) and optional service exposure for external tools.

### Integration Points

- **Steam** <-> **SteamDiscoveryService**: Parses `libraryfolders.vdf` and `appmanifest_*.acf` files directly (replicating `SteamAutoPopulateService.cs` VDF parsing). Discovers compatdata paths, Proton installations, and game install directories.
- **Proton** <-> **SteamLaunchService**: Invokes Proton directly via `proton run <trainer.exe>` with cleaned environment variables (replicating `run_proton_with_clean_env()` from `steam-launch-helper.sh`). No WINE bridge needed since the app runs natively.
- **Linux Kernel** <-> **ProcessManager**: Uses `/proc/<pid>/status`, `/proc/<pid>/maps`, `kill(pid, 0)`, `waitpid()`, and `SIGSTOP`/`SIGCONT` for process lifecycle management.
- **Linux Kernel** <-> **MemoryManager**: Uses `/proc/<pid>/mem` for read/write with `/proc/<pid>/maps` for region discovery. Replaces `ReadProcessMemory`/`WriteProcessMemory`/`VirtualQueryEx`.
- **Linux Kernel** <-> **InjectionManager**: Uses `ptrace(PTRACE_ATTACH)` + `ptrace(PTRACE_POKETEXT)` for runtime injection, or `LD_PRELOAD` for launch-time injection. The primary trainer workflow uses `proton run` (not ptrace injection), matching the proven shell script pattern.

## Win32 P/Invoke to Linux Equivalents

This is the critical mapping table. Every Win32 API used in the existing codebase has a Linux equivalent.

### ProcessManager Mappings

| Win32 API                         | Linux Equivalent                                                  | Notes                                                                                                                                                            |
| --------------------------------- | ----------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `CreateProcess`                   | `fork()` + `execve()` or `Command::new()` in Rust                 | For launching game/trainer executables. Native Linux processes use fork/exec. For Proton games, invoke `proton run <exe>` directly.                              |
| `OpenProcess(PROCESS_ALL_ACCESS)` | `/proc/<pid>/` filesystem access                                  | No handle concept. Access is governed by UID match and `CAP_SYS_PTRACE`. Check `/proc/<pid>/status` for process existence.                                       |
| `CloseHandle`                     | No-op                                                             | No handles to close. File descriptors from `/proc` are closed normally.                                                                                          |
| `OpenThread`                      | `/proc/<pid>/task/<tid>/`                                         | Thread IDs are visible as subdirectories of `/proc/<pid>/task/`.                                                                                                 |
| `SuspendThread`                   | `kill(tid, SIGSTOP)` via `tgkill()`                               | Per-thread stop. Alternatively, `ptrace(PTRACE_ATTACH, tid)` auto-stops the thread.                                                                              |
| `ResumeThread`                    | `kill(tid, SIGCONT)` via `tgkill()`                               | Per-thread resume. Or `ptrace(PTRACE_DETACH, tid)`.                                                                                                              |
| `MiniDumpWriteDump`               | `/proc/<pid>/coredump_filter` + `kill(pid, SIGABRT)` or `gcore`   | Core dumps are the Linux equivalent. Can also use `ptrace` to read all memory regions and write a custom dump.                                                   |
| `GetProcessById`                  | `kill(pid, 0)` + `/proc/<pid>/status`                             | Signal 0 checks existence without affecting the process. Parse `/proc/<pid>/status` for details.                                                                 |
| `Process.Modules`                 | `/proc/<pid>/maps`                                                | Parse the memory maps file for loaded shared objects (`.so` files). Each mapped file is a loaded module.                                                         |
| `Process.MainWindowHandle`        | X11: `_NET_WM_PID` property scan; Wayland: not reliably available | For process readiness detection. Alternative: check if `/proc/<pid>/fd/` contains display server sockets, or poll for window appearance via compositor protocol. |

### InjectionManager Mappings

| Win32 API                         | Linux Equivalent                                     | Notes                                                                                                                                                                                                     |
| --------------------------------- | ---------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `GetModuleHandle("kernel32.dll")` | N/A                                                  | No equivalent needed. Linux injection does not use the LoadLibrary pattern.                                                                                                                               |
| `GetProcAddress`                  | `dlsym()`                                            | Resolves symbol addresses in shared libraries. Used if doing ptrace-based `dlopen()` injection.                                                                                                           |
| `LoadLibrary` / `FreeLibrary`     | `dlopen()` / `dlclose()`                             | For in-process validation. On Linux, validate ELF headers directly instead of trial-loading.                                                                                                              |
| `VirtualAllocEx`                  | `ptrace` + `mmap` syscall injection                  | Allocate memory in remote process by injecting an `mmap()` syscall via ptrace. Complex but well-documented pattern.                                                                                       |
| `WriteProcessMemory`              | `ptrace(PTRACE_POKETEXT)` or `/proc/<pid>/mem` write | `/proc/<pid>/mem` is simpler for bulk writes. ptrace is needed for single-word pokes when the process must be stopped.                                                                                    |
| `CreateRemoteThread`              | `ptrace` + hijack existing thread's registers        | No direct equivalent. Inject a `dlopen()` call by: (1) ptrace attach, (2) save registers, (3) set RIP to `dlopen`, (4) set args, (5) continue, (6) restore registers. Or use `LD_PRELOAD` at launch time. |
| `WaitForSingleObject`             | `waitpid()` or `ptrace(PTRACE_CONT)` + `waitpid()`   | Wait for ptrace events or process state changes.                                                                                                                                                          |
| `GetExitCodeThread`               | Parse ptrace wait status                             | `WEXITSTATUS()`, `WIFSTOPPED()`, etc.                                                                                                                                                                     |

**Key architectural difference**: The existing WinForms app's primary Steam workflow does not actually use CreateRemoteThread/LoadLibraryA injection for trainers. It uses `proton run <trainer.exe>` via shell scripts. The native Linux app should prioritize this pattern (which already works) and treat ptrace-based injection as a secondary, advanced feature.

### MemoryManager Mappings

| Win32 API            | Linux Equivalent                | Notes                                                                                                                         |
| -------------------- | ------------------------------- | ----------------------------------------------------------------------------------------------------------------------------- |
| `ReadProcessMemory`  | `pread()` on `/proc/<pid>/mem`  | Requires same-user or `CAP_SYS_PTRACE`. Must ptrace-attach first on hardened kernels (where `kernel.yama.ptrace_scope >= 1`). |
| `WriteProcessMemory` | `pwrite()` on `/proc/<pid>/mem` | Same permission requirements as read. Pages must be writable or protection changed via ptrace.                                |
| `VirtualQueryEx`     | Parse `/proc/<pid>/maps`        | Each line gives: address range, permissions (rwxp), offset, device, inode, pathname. Replaces `MEMORY_BASIC_INFORMATION`.     |

### PE Validation Equivalent

The existing `IsDll64Bit()` reads PE headers (MZ magic, PE signature, Optional Header magic). The Linux equivalent validates ELF headers:

| PE Concept                              | ELF Equivalent                            |
| --------------------------------------- | ----------------------------------------- |
| `0x3C` PE header offset                 | ELF header at offset 0 (magic: `\x7fELF`) |
| `IMAGE_NT_OPTIONAL_HDR32_MAGIC` (0x10B) | `EI_CLASS = ELFCLASS32` (byte 4 = 1)      |
| `IMAGE_NT_OPTIONAL_HDR64_MAGIC` (0x20B) | `EI_CLASS = ELFCLASS64` (byte 4 = 2)      |

For trainer validation, the native app should validate that the trainer `.exe` is a valid PE binary (keeping the existing PE header parsing logic) since trainers are still Windows executables that run under Proton.

## Data Models

### Game Profile (TOML format)

The existing `.profile` format is a flat key=value file. The native app should migrate to TOML for richer structure while maintaining backward compatibility by reading old `.profile` files.

```toml
# ~/.config/crosshook/profiles/elden-ring.toml

[game]
name = "Elden Ring"
executable_path = "/mnt/games/SteamLibrary/steamapps/common/ELDEN RING/Game/eldenring.exe"
working_directory = ""  # defaults to exe parent directory

[trainer]
path = "/home/user/trainers/EldenRing_FLiNG.exe"
type = "fling"  # fling | wemod | generic

[injection]
dll_paths = []
auto_inject = false
inject_on_launch = [false, false]

[steam]
enabled = true
app_id = "1245620"
compatdata_path = "/mnt/games/SteamLibrary/steamapps/compatdata/1245620"
proton_path = "/mnt/games/SteamLibrary/steamapps/common/Proton 9.0-4/proton"
client_install_path = "/home/user/.steam/root"

[steam.launcher]
icon_path = ""
display_name = "Elden Ring"

[launch]
method = "proton_run"  # proton_run | direct | steam_applaunch
```

### ProfileData mapping from existing format

| Existing `.profile` key | New TOML path                   | Notes                                          |
| ----------------------- | ------------------------------- | ---------------------------------------------- |
| `GamePath`              | `game.executable_path`          |                                                |
| `TrainerPath`           | `trainer.path`                  |                                                |
| `Dll1Path`              | `injection.dll_paths[0]`        | Array instead of numbered fields               |
| `Dll2Path`              | `injection.dll_paths[1]`        |                                                |
| `LaunchInject1`         | `injection.inject_on_launch[0]` |                                                |
| `LaunchInject2`         | `injection.inject_on_launch[1]` |                                                |
| `LaunchMethod`          | `launch.method`                 | Enum values change (no Win32-specific methods) |
| `UseSteamMode`          | `steam.enabled`                 |                                                |
| `SteamAppId`            | `steam.app_id`                  |                                                |
| `SteamCompatDataPath`   | `steam.compatdata_path`         | Already stored as Unix paths                   |
| `SteamProtonPath`       | `steam.proton_path`             |                                                |
| `SteamLauncherIconPath` | `steam.launcher.icon_path`      |                                                |

### Application Settings

```toml
# ~/.config/crosshook/settings.toml

[general]
auto_load_last_profile = true
last_used_profile = "elden-ring"
theme = "system"  # system | dark | light

[paths]
profiles_directory = ""  # defaults to ~/.config/crosshook/profiles/
trainers_directory = ""  # optional default browse location
log_directory = ""       # defaults to ~/.local/share/crosshook/logs/

[steam]
client_install_path = ""  # auto-detected if empty
library_paths = []        # additional library paths beyond auto-detected

[notifications]
use_desktop_notifications = true
notify_on_trainer_ready = true
notify_on_injection_complete = true

[advanced]
ptrace_scope_warning_dismissed = false
process_poll_interval_ms = 250
process_ready_timeout_ms = 15000
```

### Recent Files

```toml
# ~/.local/share/crosshook/recent.toml

[recent]
game_paths = [
  "/mnt/games/SteamLibrary/steamapps/common/ELDEN RING/Game/eldenring.exe",
]
trainer_paths = [
  "/home/user/trainers/EldenRing_FLiNG.exe",
]
dll_paths = []
```

### Steam Library Discovery Data (runtime, not persisted)

```rust
pub struct SteamLibrary {
    pub path: PathBuf,                    // e.g., /mnt/games/SteamLibrary
    pub steamapps_path: PathBuf,          // e.g., /mnt/games/SteamLibrary/steamapps
    pub games: Vec<SteamGameManifest>,
}

pub struct SteamGameManifest {
    pub app_id: String,
    pub name: String,
    pub install_dir: String,
    pub manifest_path: PathBuf,
    pub compatdata_path: Option<PathBuf>,
}

pub struct ProtonInstall {
    pub name: String,                     // e.g., "Proton 9.0-4"
    pub path: PathBuf,                    // path to the proton executable
    pub source: ProtonSource,             // official | ge | custom
}

pub enum ProtonSource {
    SteamappsCommon,      // steamapps/common/Proton X.Y
    CompatibilityToolsD,  // ~/.steam/root/compatibilitytools.d/
    System,               // /usr/share/steam/compatibilitytools.d/
}
```

## API Design

### Internal Service APIs (Rust traits)

```rust
// Process management
pub trait ProcessControl {
    fn launch(&mut self, config: &LaunchConfig) -> Result<ProcessHandle>;
    fn attach(&mut self, pid: u32) -> Result<ProcessHandle>;
    fn detach(&mut self) -> Result<()>;
    fn kill(&mut self) -> Result<()>;
    fn suspend(&self) -> Result<()>;
    fn resume(&self) -> Result<()>;
    fn is_running(&self) -> bool;
    fn pid(&self) -> Option<u32>;
    fn wait_for_ready(&self, opts: &ReadinessOptions) -> ReadinessResult;
    fn modules(&self) -> Result<Vec<ProcessModule>>;
    fn threads(&self) -> Result<Vec<ThreadInfo>>;
}

pub struct LaunchConfig {
    pub executable: PathBuf,
    pub working_dir: Option<PathBuf>,
    pub method: LaunchMethod,
    pub environment: HashMap<String, String>,
}

pub enum LaunchMethod {
    Direct,          // fork/exec
    ProtonRun,       // proton run <exe>
    SteamAppLaunch,  // steam -applaunch <appid>
}

// Memory operations
pub trait MemoryAccess {
    fn read(&self, address: usize, size: usize) -> Result<Vec<u8>>;
    fn write(&self, address: usize, data: &[u8]) -> Result<usize>;
    fn query_regions(&self) -> Result<Vec<MemoryRegion>>;
    fn save_state(&self) -> Result<MemoryState>;
    fn restore_state(&self, state: &MemoryState) -> Result<()>;
}

pub struct MemoryRegion {
    pub start: usize,
    pub end: usize,
    pub permissions: Permissions,
    pub offset: u64,
    pub device: String,
    pub inode: u64,
    pub pathname: Option<PathBuf>,
}

pub struct Permissions {
    pub read: bool,
    pub write: bool,
    pub execute: bool,
    pub shared: bool,  // vs private
}

// Injection
pub trait Injector {
    fn inject_library(&self, pid: u32, library_path: &Path) -> Result<()>;
    fn validate_library(&self, library_path: &Path) -> Result<LibraryInfo>;
}

pub enum InjectionMethod {
    LdPreload,       // Set LD_PRELOAD before launch
    PtraceInject,    // ptrace-based dlopen injection
    ProtonRun,       // Run trainer as separate Proton process (primary method)
}

// Steam integration
pub trait SteamDiscovery {
    fn discover_libraries(&self) -> Result<Vec<SteamLibrary>>;
    fn find_game(&self, game_path: &Path) -> Result<Option<SteamGameManifest>>;
    fn discover_proton_installs(&self) -> Result<Vec<ProtonInstall>>;
    fn resolve_compatdata(&self, app_id: &str) -> Result<Option<PathBuf>>;
    fn auto_populate(&self, game_path: &Path) -> Result<SteamAutoPopulateResult>;
}

// Profile management
pub trait ProfileStore {
    fn list(&self) -> Result<Vec<String>>;
    fn load(&self, name: &str) -> Result<GameProfile>;
    fn save(&self, name: &str, profile: &GameProfile) -> Result<()>;
    fn delete(&self, name: &str) -> Result<()>;
    fn import_legacy(&self, profile_path: &Path) -> Result<GameProfile>;
}

// Steam launch orchestration
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
  launch     Launch a game with optional trainer and injection
  profile    Manage game profiles (list, load, save, delete, import)
  steam      Steam integration commands (discover, auto-populate, export)
  inject     Inject a shared library into a running process
  memory     Memory operations on a running process
  status     Show status of tracked processes

Options:
  -p, --profile <NAME>    Load a profile by name
  --config <PATH>         Override config directory
  -v, --verbose           Increase log verbosity
  -q, --quiet             Suppress non-error output
  --json                  Output in JSON format (for scripting)

Examples:
  crosshook launch --profile elden-ring
  crosshook launch --profile elden-ring --trainer-only
  crosshook steam discover
  crosshook steam auto-populate --game-path /path/to/game.exe
  crosshook steam export --profile elden-ring
  crosshook profile import --legacy-path /path/to/old.profile
  crosshook inject --pid 12345 --library /path/to/mod.so
  crosshook memory read --pid 12345 --address 0x7fff1234 --size 256
```

### D-Bus Integration

```
Bus Name: com.crosshook.App
Object Path: /com/crosshook/App

Interface: com.crosshook.App
  Methods:
    LaunchProfile(profile_name: string) -> (success: bool, message: string)
    GetStatus() -> (json: string)
    ListProfiles() -> (profiles: array<string>)

  Signals:
    ProcessStarted(pid: uint32, name: string)
    ProcessStopped(pid: uint32, name: string)
    TrainerReady(profile_name: string)
    InjectionComplete(pid: uint32, library: string, success: bool)
```

Desktop notifications use `org.freedesktop.Notifications` directly (no custom D-Bus service required for basic notifications).

## System Constraints

### Security Requirements

**ptrace scope**: Modern Linux distributions set `kernel.yama.ptrace_scope = 1` by default (Ubuntu, Fedora, SteamOS). This means a process can only ptrace its own descendants. Implications:

- Reading/writing `/proc/<pid>/mem` of a non-child process requires `ptrace(PTRACE_ATTACH)` first, which requires `CAP_SYS_PTRACE` or `ptrace_scope = 0`.
- The primary trainer workflow (Proton run) is unaffected because the trainer runs as a child process of Proton, not as an injected library.
- For the advanced injection/memory features, the app should:
  1. Check `kernel.yama.ptrace_scope` at startup and warn the user.
  2. Offer a polkit-based privilege escalation path for ptrace operations.
  3. Document `sudo setcap cap_sys_ptrace+ep /usr/bin/crosshook` as an alternative.

**File permissions**: `/proc/<pid>/mem` access requires the reader to have the same UID as the target process or `CAP_SYS_PTRACE`. Since games launched via Steam run as the same user, this is normally satisfied.

**Flatpak sandbox**: Flatpak sandboxing would prevent `/proc` access to processes outside the sandbox. CrossHook must run as a native package, not a Flatpak, for process manipulation features. Flatpak is explicitly not supported for the core functionality.

### Performance Requirements

| Operation               | Target  | Notes                                         |
| ----------------------- | ------- | --------------------------------------------- |
| Process existence check | < 1ms   | `kill(pid, 0)` is a single syscall            |
| Process list scan       | < 50ms  | Iterate `/proc/*/status`                      |
| Memory region query     | < 10ms  | Parse `/proc/<pid>/maps`                      |
| Memory read (4KB)       | < 1ms   | Single `pread()` on `/proc/<pid>/mem`         |
| Memory write (4KB)      | < 1ms   | Single `pwrite()` on `/proc/<pid>/mem`        |
| Steam library discovery | < 500ms | VDF file parsing, directory enumeration       |
| Profile load            | < 5ms   | TOML deserialization                          |
| UI responsiveness       | 60fps   | GTK4 main loop; all I/O on background threads |

Process monitoring should poll at configurable intervals (default 250ms, matching `ProcessReadinessOptions.PollIntervalMs`). Use `inotify` on `/proc/<pid>/` for event-driven process exit detection where available.

### Platform Compatibility

| Distribution             | Status         | Notes                                            |
| ------------------------ | -------------- | ------------------------------------------------ |
| SteamOS 3.x (Steam Deck) | Primary target | Arch-based, GTK4 available, Gamescope compositor |
| Arch Linux               | Supported      | Rolling release, latest GTK4                     |
| Fedora 39+               | Supported      | GTK4 included in base                            |
| Ubuntu 24.04+            | Supported      | GTK4 available in repos                          |
| Debian 12+               | Supported      | GTK4 in backports or directly                    |

**Display server compatibility**:

- Wayland: Full support via GTK4 (native Wayland backend). Steam Deck uses Gamescope (Wayland compositor).
- X11: Supported via GTK4's X11 backend (XWayland fallback also works).
- No display server (SSH/console): CLI mode works without any display server.

### Packaging Strategy

| Format        | Priority      | Notes                                                                  |
| ------------- | ------------- | ---------------------------------------------------------------------- |
| AppImage      | Primary       | Self-contained, no root required, works on Steam Deck without `pacman` |
| AUR (Arch)    | High          | Native package for Arch/SteamOS users                                  |
| Flatpak       | Not supported | Cannot access `/proc` of host processes                                |
| .deb/.rpm     | Medium        | For Ubuntu/Fedora users                                                |
| Static binary | High          | Single binary for CLI-only usage                                       |

## Codebase Changes

### Files to Create (new `src/crosshook-linux/` directory)

```
src/crosshook-linux/
  Cargo.toml                          # Workspace root
  Cargo.lock
  crates/
    crosshook-core/
      Cargo.toml
      src/
        lib.rs                        # Crate root, re-exports
        process/
          mod.rs                      # ProcessManager trait + Linux impl
          linux.rs                    # /proc, ptrace, signals
          readiness.rs                # Process readiness polling
        memory/
          mod.rs                      # MemoryManager trait + Linux impl
          linux.rs                    # /proc/pid/mem read/write
          regions.rs                  # /proc/pid/maps parser
          state.rs                    # Save/restore memory state
        injection/
          mod.rs                      # Injector trait
          ld_preload.rs               # LD_PRELOAD injection
          ptrace.rs                   # ptrace-based dlopen injection
          proton_run.rs               # Proton run (primary trainer method)
          pe_validate.rs              # PE header validation for .exe trainers
        steam/
          mod.rs                      # Steam integration module
          discovery.rs                # Library/game/Proton discovery
          vdf_parser.rs               # Valve Data Format parser
          launch.rs                   # Steam launch orchestration
          auto_populate.rs            # Auto-populate from game path
          environment.rs              # Clean environment for Proton
          launcher_export.rs          # .sh + .desktop generation
        profile/
          mod.rs                      # Profile CRUD
          toml_store.rs               # TOML-based profile storage
          legacy_import.rs            # Import .profile files from WinForms
        settings/
          mod.rs                      # Application settings
        diagnostics/
          mod.rs                      # Logging, crash reports
        events.rs                     # Event/callback types
        errors.rs                     # Error types

    crosshook-cli/
      Cargo.toml
      src/
        main.rs                       # CLI entry point
        commands/
          mod.rs
          launch.rs
          profile.rs
          steam.rs
          inject.rs
          memory.rs
          status.rs

    crosshook-gtk/
      Cargo.toml
      src/
        main.rs                       # GTK4 application entry point
        app.rs                        # Application struct, activation
        window.rs                     # Main window (replaces MainForm)
        widgets/
          mod.rs
          profile_panel.rs            # Profile selection/management
          game_config.rs              # Game/trainer path selection
          steam_config.rs             # Steam mode configuration
          launch_panel.rs             # Launch controls
          console_output.rs           # Log/console text view
          process_status.rs           # Process status display
          injection_panel.rs          # DLL/SO injection controls
        dialogs/
          mod.rs
          profile_dialog.rs           # Save profile dialog
          file_chooser.rs             # File/directory selection
          steam_auto_populate.rs      # Auto-populate results dialog
        state.rs                      # Application state management
        style.css                     # GTK4 CSS for dark theme
```

### Dependencies

| Crate                            | Purpose                                                  |
| -------------------------------- | -------------------------------------------------------- |
| `gtk4` (0.9+)                    | GTK4 Rust bindings                                       |
| `libadwaita` (0.7+)              | Adaptive layouts, dark theme                             |
| `tokio` (1.x)                    | Async runtime for I/O operations                         |
| `serde` + `toml`                 | Profile and settings serialization                       |
| `clap` (4.x)                     | CLI argument parsing                                     |
| `nix` (0.29+)                    | Safe wrappers for Linux syscalls (ptrace, kill, waitpid) |
| `procfs` (0.17+)                 | `/proc` filesystem parsing                               |
| `zbus` (5.x)                     | D-Bus integration                                        |
| `tracing` + `tracing-subscriber` | Structured logging                                       |
| `notify-rust`                    | Desktop notifications                                    |
| `gobject-macros`                 | GObject subclassing for custom GTK widgets               |
| `directories`                    | XDG directory resolution                                 |

### Existing Files to Preserve/Reference

These existing files contain patterns and logic that should be directly translated:

- `src/CrossHookEngine.App/Services/SteamAutoPopulateService.cs` -> `crosshook-core/src/steam/auto_populate.rs` (VDF parsing, library discovery, manifest matching)
- `src/CrossHookEngine.App/Services/SteamLaunchService.cs` -> `crosshook-core/src/steam/launch.rs` (environment cleanup, path conversion no longer needed natively)
- `src/CrossHookEngine.App/Services/SteamExternalLauncherExportService.cs` -> `crosshook-core/src/steam/launcher_export.rs` (script and .desktop generation)
- `src/CrossHookEngine.App/Services/ProfileService.cs` -> `crosshook-core/src/profile/` (profile CRUD, with TOML upgrade)
- `src/CrossHookEngine.App/Services/AppSettingsService.cs` -> `crosshook-core/src/settings/` (settings persistence)
- `src/CrossHookEngine.App/Services/CommandLineParser.cs` -> `crosshook-cli/src/commands/` (replaced by clap)
- `src/CrossHookEngine.App/runtime-helpers/steam-launch-helper.sh` -> Logic absorbed into `crosshook-core/src/steam/launch.rs` (the native app can do this directly without shell script intermediaries)
- `src/CrossHookEngine.App/runtime-helpers/steam-host-trainer-runner.sh` -> Logic absorbed into `crosshook-core/src/injection/proton_run.rs`

## Technical Decisions

### Decision 1: Programming Language

- **Options**: Rust, Python (PyGObject), C# (Avalonia), Go (gotk4), C++ (gtkmm)
- **Recommendation**: Rust
- **Rationale**:
  - Zero-cost abstractions for low-level `/proc` and `ptrace` operations without garbage collection pauses during memory operations.
  - `nix` crate provides safe, typed wrappers for all required Linux syscalls.
  - `gtk4-rs` and `libadwaita-rs` are mature, well-maintained bindings with strong GNOME ecosystem support.
  - Single static binary possible for CLI mode (important for Steam Deck where installing dependencies is friction).
  - Memory safety guarantees are critical for a tool that manipulates other processes' memory.
  - C# with Avalonia was considered but adds a .NET runtime dependency on Linux, and the whole point is to escape the WINE/.NET dependency chain. Avalonia also lacks the native desktop integration (D-Bus, XDG, system tray) that GTK4 provides out of the box.

### Decision 2: UI Toolkit

- **Options**: GTK4+libadwaita, Qt6, egui, Iced, Terminal UI (ratatui)
- **Recommendation**: GTK4 + libadwaita
- **Rationale**:
  - Steam Deck runs SteamOS (Arch-based with GNOME/KDE), where GTK4 is a native citizen.
  - libadwaita provides adaptive layouts that handle the Deck's 1280x800 screen gracefully.
  - Native Wayland support (critical for Gamescope on Steam Deck).
  - Dark theme support built in (matching the existing WinForms dark theme).
  - D-Bus integration is trivial from GTK4 apps.
  - Qt6 is viable but has heavier Rust binding ergonomics and licensing considerations.

### Decision 3: Profile Storage Format

- **Options**: TOML, JSON, SQLite, INI (existing format)
- **Recommendation**: TOML with legacy `.profile` import
- **Rationale**:
  - TOML is human-readable and supports nested structures (unlike INI).
  - `serde` + `toml` crate makes serialization trivial in Rust.
  - JSON lacks comments and is less pleasant to hand-edit.
  - SQLite is overkill for a few dozen profiles.
  - Backward compatibility: implement a one-time migration path that reads existing `.profile` files and converts them to TOML.

### Decision 4: Process Monitoring Strategy

- **Options**: Polling `/proc`, `inotify` on `/proc/<pid>`, `pidfd_open` + `poll`, `netlink` process connector
- **Recommendation**: `pidfd_open` + `poll` (primary) with `/proc` polling fallback
- **Rationale**:
  - `pidfd_open` (Linux 5.3+) provides race-free process handle and can be polled for exit notification. SteamOS 3.x ships kernel 6.1+ so this is available.
  - `/proc/<pid>/status` polling as fallback for older kernels.
  - `netlink` process connector (`PROC_EVENT_EXIT`) gives system-wide process exit events but requires `CAP_NET_ADMIN` or root.
  - Avoids the thundering herd problem of polling many processes.

### Decision 5: Trainer Launch Strategy

- **Options**: Direct ptrace injection, LD_PRELOAD, Proton run, Hybrid
- **Recommendation**: Proton run as primary, ptrace injection as optional advanced feature
- **Rationale**:
  - The existing shell scripts prove that `proton run <trainer.exe>` works reliably for FLiNG, WeMod, and similar trainers.
  - ptrace-based injection into a running WINE/Proton process is fragile because WINE's internal state (threading, memory layout) is not designed for external manipulation.
  - LD_PRELOAD only works at process launch time and requires hooking into the Proton launch sequence.
  - The native app should make the proven pattern the default and expose ptrace as an expert option.

### Decision 6: Project Structure Relationship

- **Options**: Separate repository, subdirectory of existing repo, workspace in existing repo
- **Recommendation**: New directory within existing repository (`src/crosshook-linux/`)
- **Rationale**:
  - Shared documentation, issue tracking, and release process.
  - The WinForms app continues to exist for pure Windows users.
  - CI can build both targets from the same repo.
  - Eventually, `crosshook-core` could become a shared library if a macOS native UI is added later.

## Gotchas and Edge Cases

### Steam Environment Variable Contamination

The existing `steam-launch-helper.sh` and `SteamLaunchService.cs` both meticulously unset ~30 WINE/Proton/Steam environment variables before launching a trainer. The native app must replicate this exactly. The full list is in `SteamLaunchService.GetEnvironmentVariablesToClear()` and the `run_proton_with_clean_env()` function. A native Linux process inherits fewer of these variables, but if launched from a Steam shortcut or Gamescope session, many will still be present.

### Trainer Staging into Compatdata

The shell scripts copy the trainer executable into `$STEAM_COMPAT_DATA_PATH/pfx/drive_c/CrossHook/StagedTrainers/` before running it. This is because the trainer's Windows path must be relative to the WINE prefix's C: drive. The native app must replicate this staging step when using `proton run`.

### File Descriptor Inheritance

The shell scripts close all inherited file descriptors above fd 2 before launching the trainer Proton process. This prevents the trainer's wineserver from inheriting connections to CrossHook's wineserver. The native app launching via `Command::new("proton")` must similarly ensure `close_fds`/`pre_exec` fd cleanup.

### Steam Deck Gamescope Integration

On Steam Deck, games run inside Gamescope (a nested Wayland compositor). The GTK4 app will run in the regular desktop session, not inside Gamescope. This is correct behavior -- the app is a launcher, not a game overlay. However, window focus switching between Gamescope and the desktop session requires special handling (the existing app minimizes itself after launching a game).

### WINE Path Conversion No Longer Needed

The existing `SteamLaunchService.ConvertToUnixPath()` and `ConvertToWindowsPath()` use `winepath.exe` because the WinForms app runs inside WINE and receives Windows-style paths. The native Linux app receives Unix paths directly. However, `ConvertToWindowsPath()` is still needed for one case: converting the trainer's host path to a Windows path for `proton run` arguments (e.g., `C:\CrossHook\StagedTrainers\trainer.exe`). This conversion can be done with simple string manipulation since the staged path is always relative to `drive_c` in the prefix.

### Profile Path Differences

Existing profiles store paths as Windows paths (e.g., `Z:\home\user\trainer.exe`). The native app stores Unix paths. The legacy import must convert `Z:\...` paths to their Unix equivalents by stripping the `Z:` prefix and converting backslashes.

### ptrace_scope on Steam Deck

SteamOS 3.x ships with `kernel.yama.ptrace_scope = 1` by default. The Proton run trainer workflow does not require ptrace. The advanced memory read/write and injection features do. The app must detect this setting at startup and either:

1. Guide users to `echo 0 > /proc/sys/kernel/yama/ptrace_scope` (temporary).
2. Use `pkexec` for elevated ptrace operations.
3. Clearly communicate which features require elevated privileges.

### dosdevices Symlink Resolution

The existing `SteamAutoPopulateService` and `SteamLaunchService` contain extensive logic for resolving WINE `dosdevices` symlinks (e.g., `d:` -> `/mnt/games`). The native app does not need this because it operates directly on Unix paths. However, if importing profiles from the WinForms app, the `dosdevices` resolution logic must be available for the migration path.

## Open Questions

- **macOS support timeline**: Should `crosshook-core` abstract platform-specific operations behind traits from day one to prepare for a future macOS native UI, or should Linux-specific implementations be hardcoded initially?
- **WeMod integration**: WeMod trainers have a more complex launch sequence than FLiNG trainers. Does the native app need to handle WeMod's authentication/download flow, or is it sufficient to launch a pre-downloaded WeMod trainer executable?
- **Steam Deck Game Mode integration**: Should the app register as a Steam non-Steam game shortcut for launch from Game Mode, or is Desktop Mode the only supported context?
- **Shared profile format**: Should the native app and WinForms app share a common profile format (e.g., both read/write TOML), or should they maintain separate formats with a one-way migration tool?
- **DLL injection via Proton**: The WinForms app's in-app DLL injection (CreateRemoteThread + LoadLibraryA) does not work in Steam mode today. Should the native app attempt ptrace-based DLL injection into a running WINE process, or should DLL injection remain a non-goal for the Steam/Proton workflow?
- **Automated testing strategy**: The existing codebase has no test framework. Should the native app ship with integration tests that launch actual Proton processes, or should testing be limited to unit tests with mocked `/proc` data?

## Relevant Existing Documentation

- `docs/plans/dotnet-migrate/research-architecture.md`: Detailed architecture analysis of the existing WinForms codebase including data flow diagrams and all 29 P/Invoke declaration sites.
- `docs/plans/dotnet-migrate/research-technical.md`: Technical constraints of the .NET migration, including the ASCII injection path limitation and architecture drift risks.
- `docs/features/steam-proton-trainer-launch.doc.md`: User-facing documentation of the Steam/Proton trainer workflow, generated launcher format, and current limitations.
- `CLAUDE.md` (project root): Build commands, architecture overview, and code conventions for the existing codebase.
