# External API Research: platform-native-linux-ui

## Executive Summary

Building a platform-native Linux UI for CrossHook requires three primary integration layers: (1) Steam game discovery and launch orchestration via VDF/ACF file parsing and the `steam -applaunch` / `steam://rungameid/` protocol, (2) process memory access for trainer injection via Linux-native `process_vm_readv`/`process_vm_writev` syscalls (or ptrace) targeting WINE/Proton game processes from outside the WINE environment, and (3) a native desktop UI framework -- with Tauri v2 (Rust + web frontend) as the recommended cross-platform option, or GTK4/libadwaita via Relm4 for a GNOME-native experience. The existing shell scripts (`steam-launch-helper.sh`, `steam-host-trainer-runner.sh`, `steam-launch-trainer.sh`) already demonstrate the working pattern of launching trainers via `proton run` in a clean environment, which the native app must replicate and extend.

## Primary APIs

### Steam CLI and steam:// Protocol

- **Documentation**: [Valve Developer Community - Steam Browser Protocol](https://developer.valvesoftware.com/wiki/Steam_browser_protocol) | [Valve Developer Community - Command Line Options](https://developer.valvesoftware.com/wiki/Command_line_options)
- **Authentication**: None required (local client communication)
- **Key Commands**:
  - `steam -applaunch <appid> [launch parameters]`: Launch a game from CLI. Most reliable method for automation. If Steam is not running, it silently starts the client first.
  - `steam steam://rungameid/<appid>`: Launch via browser protocol handler. Requires the steam:// URI scheme to be registered with the system.
  - `steam steam://run/<appid>/<language>/<url_encoded_params>`: Launch with specific launch options.
  - `steam steam://install/<appid>`: Trigger game installation.
  - `steam steam://validate/<appid>`: Validate local game files.
  - `steam steam://open/games`: Open the Steam library view.
- **Rate Limits**: None (local IPC)
- **Pricing**: Free (Steam client is free)
- **Confidence**: High -- multiple community sources confirm these commands, and the existing `steam-launch-helper.sh` already uses `steam -applaunch`.

**Practical notes**:

- The `steam -applaunch` method is preferred over `steam://` protocol URLs because it does not require browser/URI handler registration and works reliably from scripts.
- Most `steam://` subcommands are undocumented. The four game-launch variants (`run`, `rungameid`, `runsafe`, `rungame`) have subtle behavioral differences that are not officially documented.
- There is a known feature request ([Issue #4035](https://github.com/ValveSoftware/steam-for-linux/issues/4035)) for a headless launch mode that exits Steam after the game closes, but this is not yet implemented.

### Direct Proton CLI Execution

- **Documentation**: [GitHub Gist - Launch games via Proton from CLI](https://gist.github.com/sxiii/6b5cd2e7d2321df876730f8cafa12b2e) | [Proton FAQ](https://github.com/ValveSoftware/Proton/wiki/Proton-FAQ)
- **Authentication**: None
- **Key Environment Variables**:
  - `STEAM_COMPAT_DATA_PATH`: Points to `~/.local/share/Steam/steamapps/compatdata/<appid>` (the WINE prefix root)
  - `STEAM_COMPAT_CLIENT_INSTALL_PATH`: Points to `~/.local/share/Steam/` (Steam installation root)
  - `WINEPREFIX`: Set to `$STEAM_COMPAT_DATA_PATH/pfx`
- **Key Command**:
  ```bash
  export STEAM_COMPAT_DATA_PATH=~/.local/share/Steam/steamapps/compatdata/<appid>
  export STEAM_COMPAT_CLIENT_INSTALL_PATH=~/.local/share/Steam/
  ~/.local/share/Steam/steamapps/common/Proton\ <VERSION>/proton run <path-to-exe>
  ```
- **Critical Pattern**: The existing `steam-host-trainer-runner.sh` demonstrates the correct approach -- strip all inherited WINE/Proton environment variables (`WINESERVER`, `WINELOADER`, `WINEDLLPATH`, `LD_PRELOAD`, `LD_LIBRARY_PATH`, etc.) before setting a clean environment and calling `proton run`. This prevents the trainer's WINE session from conflicting with the game's WINE session.
- **Confidence**: High -- this is the proven working pattern from the existing codebase.

### Linux Process Memory Access APIs

- **Documentation**: [ptrace(2) man page](https://man7.org/linux/man-pages/man2/ptrace.2.html) | [process_vm_readv(2) man page](https://man7.org/linux/man-pages/man2/process_vm_readv.2.html)
- **Authentication**: Same-user ownership or `CAP_SYS_PTRACE` capability
- **Key Syscalls**:

  | Syscall                                         | Purpose                       | Requires ptrace attach?  | Kernel version |
  | ----------------------------------------------- | ----------------------------- | ------------------------ | -------------- |
  | `ptrace(PTRACE_ATTACH, pid)`                    | Attach to process as debugger | N/A (this IS the attach) | All            |
  | `ptrace(PTRACE_PEEKDATA, pid, addr)`            | Read word from process memory | Yes                      | All            |
  | `ptrace(PTRACE_POKEDATA, pid, addr, data)`      | Write word to process memory  | Yes                      | All            |
  | `process_vm_readv(pid, local_iov, remote_iov)`  | Bulk read from process memory | No                       | 3.2+           |
  | `process_vm_writev(pid, local_iov, remote_iov)` | Bulk write to process memory  | No                       | 3.2+           |
  | `/proc/<pid>/mem` (pread/pwrite)                | File-based memory access      | Yes (must attach first)  | All            |
  | `/proc/<pid>/maps`                              | Read memory layout/mappings   | No (same user)           | All            |

- **WINE Process Compatibility**: Wine/Proton processes are standard Linux processes. A native Linux process CAN use ptrace or process_vm_readv/writev to access WINE game process memory. WINE does not use ASLR for most Windows DLLs, making memory addresses predictable. Windows executables consistently load at `0x00400000` (32-bit), kernel32.dll.so at `0x7b400000`, ntdll.dll.so at `0x7bc00000`. PE headers are fully constructed in memory at runtime by WINE, so standard PE-header-parsing techniques work.
- **Confidence**: High -- confirmed by [Attacking WINE Part I](https://schlafwandler.github.io/posts/attacking-wine-part-i/) and [WineHQ Forum discussions](https://forum.winehq.org/viewtopic.php?t=37212).

**process_vm_readv/writev advantages over ptrace for this use case**:

- Do not require stopping the target process
- Higher throughput for bulk memory operations (single copy, no kernel intermediary)
- Same permission model as ptrace (same UID or CAP_SYS_PTRACE)
- Available since Linux 3.2 (all modern distributions)

### Yama LSM / ptrace_scope

- **Documentation**: [Yama LSM - Linux Kernel Docs](https://docs.kernel.org/admin-guide/LSM/Yama.html) | [Kernel.org Yama Documentation](https://www.kernel.org/doc/Documentation/security/Yama.txt)
- **Key Values** for `kernel.yama.ptrace_scope`:

  | Value | Name       | Effect                                         |
  | ----- | ---------- | ---------------------------------------------- |
  | 0     | Classic    | Any process can ptrace any other with same UID |
  | 1     | Restricted | Only parent processes can ptrace children      |
  | 2     | Admin-only | Only processes with `CAP_SYS_PTRACE`           |
  | 3     | No attach  | All ptrace blocked                             |

- **Default on most distributions**: 1 (restricted) -- Ubuntu, Fedora, Arch default to this.
- **SteamOS/Steam Deck**: Typically 1 or 0; Valve's use case requires process debugging for Proton.
- **Workarounds for scope=1**:
  - Set `CAP_SYS_PTRACE` capability on the CrossHook binary: `sudo setcap cap_sys_ptrace=eip ./crosshook`
  - Run as root (not recommended for desktop apps)
  - Use `prctl(PR_SET_PTRACER, PR_SET_PTRACER_ANY)` in the target process (not applicable here since we don't control the game)
  - Temporarily change sysctl: `sudo sysctl kernel.yama.ptrace_scope=0` (security risk)
- **User namespace escape**: Creating a user namespace inadvertently weakens Yama protections, as the process gains `CAP_SYS_PTRACE` within the child namespace.
- **Confidence**: High -- official kernel documentation.

### D-Bus Interfaces

- **Documentation**: [D-Bus Specification](https://dbus.freedesktop.org/doc/dbus-specification.html) | [D-Bus Tutorial](https://dbus.freedesktop.org/doc/dbus-tutorial.html)
- **Relevance**: Steam does not expose a public D-Bus API for game management. However, D-Bus is relevant for:
  - System notifications (e.g., `org.freedesktop.Notifications`)
  - Desktop integration (application menus, indicators)
  - Potentially communicating with MangoHud or GameScope if they expose D-Bus interfaces
- **Steam IPC**: Steam uses its own IPC mechanisms (Unix domain sockets, shared memory) rather than D-Bus. The Steam Runtime's pressure-vessel container uses direct socket access for audio (PulseAudio/PipeWire) rather than D-Bus.
- **Confidence**: Medium -- no official Steam D-Bus API exists; D-Bus utility is limited to desktop integration.

### Proton/WINE Prefix Management and Discovery

- **Documentation**: [Proton FAQ](https://github.com/ValveSoftware/Proton/wiki/Proton-FAQ)
- **Key Paths**:
  ```
  Steam root:           ~/.local/share/Steam/
  Library folders:      ~/.local/share/Steam/steamapps/libraryfolders.vdf
  App manifests:        <library>/steamapps/appmanifest_<appid>.acf
  Compatdata (prefix):  <library>/steamapps/compatdata/<appid>/
  WINE prefix:          <library>/steamapps/compatdata/<appid>/pfx/
  Drive C:              <library>/steamapps/compatdata/<appid>/pfx/drive_c/
  Game install:         <library>/steamapps/common/<game-folder>/
  Proton versions:      <library>/steamapps/common/Proton <version>/
  User config:          ~/.local/share/Steam/userdata/<userid>/config/
  Shortcuts:            ~/.local/share/Steam/userdata/<userid>/config/shortcuts.vdf
  ```
- **Discovery Algorithm**:
  1. Parse `~/.local/share/Steam/steamapps/libraryfolders.vdf` to find all Steam library folders
  2. For each library folder, scan for `appmanifest_<appid>.acf` files
  3. Parse each ACF file to get game name, install directory, Proton version
  4. Check for `compatdata/<appid>/` to confirm Proton prefix exists
  5. Cross-reference with Proton version in `<library>/steamapps/common/Proton*/`
- **VDF Format Notes**: `libraryfolders.vdf` has two format generations (old flat key-value vs new nested objects with "path" sub-fields). The new format includes a "mounted" field for active/inactive library folders.
- **Confidence**: High -- well-documented community knowledge; tools like BoilR, protontricks, and SteamTinkerLaunch all implement this pattern.

## Libraries and SDKs

### Recommended Libraries

#### UI Framework: Tauri v2 (Recommended Primary)

- **Language**: Rust backend + any web frontend (React, Vue, Svelte, etc.)
- **Install**: `cargo install create-tauri-app` then `cargo tauri init`
- **Docs**: [Tauri v2 Documentation](https://v2.tauri.app/)
- **Why Recommended**:
  - Cross-platform from day one (Linux, Windows, macOS, mobile)
  - Tiny bundle size (~3-5 MB vs Electron's 100+ MB)
  - Uses system WebView (WebKitGTK on Linux) -- no bundled browser engine
  - Rust backend enables direct use of `nix` crate for ptrace/process_vm_readv
  - Shell plugin supports child process spawning and management
  - Sidecar support for embedding helper scripts/binaries
  - Active development, stable v2 released October 2024
- **Key Plugins**:
  - `tauri-plugin-shell`: Spawn child processes, sidecars. [Docs](https://v2.tauri.app/plugin/shell/)
  - `tauri-plugin-process`: Current process info. [Docs](https://v2.tauri.app/plugin/process/)
  - `tauri-plugin-fs`: File system access for VDF/ACF parsing
- **Linux Requirements**: WebKitGTK 4.1 (available in Ubuntu 22.04+, all modern distros)
- **Bundle Size**: ~600KB minimum, typically 3-5 MB with dependencies
- **Confidence**: High -- stable release, extensive documentation, strong community.

#### UI Framework: GTK4 + libadwaita via Relm4 (Alternative -- GNOME-native)

- **Language**: Rust
- **Install**: `cargo add relm4 libadwaita gtk4`
- **Docs**: [Relm4 Book](https://relm4.org/book/stable/) | [gtk-rs GTK4 Book](https://gtk-rs.org/gtk4-rs/stable/latest/book/) | [libadwaita on Lib.rs](https://lib.rs/crates/libadwaita)
- **Why Consider**:
  - Truly native GNOME look and feel
  - Follows GNOME HIG (Human Interface Guidelines)
  - Elm-inspired reactive architecture (Relm4)
  - No WebView overhead
  - Built-in async support
- **Drawbacks**:
  - GNOME-centric; looks foreign on KDE/other DEs
  - Cross-platform story is weak (primarily Linux)
  - Steeper learning curve for non-GNOME developers
  - libadwaita theming is controlled by GNOME, not user-customizable
- **Confidence**: High for Linux-only; Medium for cross-platform ambitions.

#### UI Framework: Qt6 (Alternative -- Cross-platform native)

- **Language**: C++, Rust (via cxx-qt), Python (PySide6)
- **Docs**: [Qt6 Documentation](https://doc.qt.io/qt-6/) | [CXX-Qt](https://github.com/KDAB/cxx-qt)
- **Why Consider**:
  - True native widgets on all platforms
  - Strong cross-platform history
  - KDE/Plasma desktop integration
- **Drawbacks**:
  - Complex licensing (GPL/LGPL/commercial)
  - Rust bindings (cxx-qt) are less mature than gtk-rs
  - Large dependency footprint
  - QSS theming on GNOME can look inconsistent
- **Confidence**: Medium -- mature framework but Rust integration is early.

### Process Injection and Memory Libraries

#### nix crate (Rust -- recommended for syscalls)

- **Install**: `cargo add nix --features ptrace,process`
- **Docs**: [nix on docs.rs](https://docs.rs/nix/latest/nix/)
- **Provides**:
  - `nix::sys::ptrace` -- Safe Rust bindings to ptrace syscalls
  - `nix::sys::uio::process_vm_readv` / `process_vm_writev` -- Direct memory access
  - `nix::sys::signal` -- Signal management for process control
  - `nix::unistd` -- Process management (fork, exec, setuid, etc.)
- **Confidence**: High -- widely used, well-maintained.

#### process_vm_io crate (Rust -- higher-level memory access)

- **Install**: `cargo add process_vm_io`
- **Docs**: [process_vm_io on crates.io](https://crates.io/crates/process_vm_io) | [Lib.rs](https://lib.rs/crates/process_vm_io)
- **Provides**: Higher-level API for reading/writing process memory using process_vm_readv/writev. Useful for process monitoring, debugging, and communication.
- **Confidence**: Medium -- smaller community, but focused purpose.

#### procmem-linux crate (Rust -- memory access abstraction)

- **Install**: `cargo add procmem-linux`
- **Docs**: [procmem-linux on Lib.rs](https://lib.rs/crates/procmem-linux)
- **Provides**: Works in Syscall mode (process_vm_readv/writev) and File mode (/proc/pid/mem). Requires kernel >= 3.2 and glibc >= 2.15.
- **Confidence**: Medium -- useful abstraction layer.

### Steam File Parsers (Rust)

#### steam-vdf-parser (Recommended)

- **Install**: `cargo add steam-vdf-parser`
- **Docs**: [steam-vdf-parser on docs.rs](https://docs.rs/steam-vdf-parser/latest/steam_vdf_parser/)
- **Features**:
  - Zero-copy text format parsing (winnow-powered)
  - Binary format support (appinfo.vdf, shortcuts.vdf, packageinfo.vdf)
  - `no_std` compatible
  - Key functions: `parse_text()`, `parse_binary()`, `parse_appinfo()`, `parse_shortcuts()`
- **Confidence**: High -- comprehensive, performant.

#### steam_shortcuts_util (For shortcuts.vdf specifically)

- **Install**: `cargo add steam_shortcuts_util`
- **Docs**: [steam_shortcuts_util on docs.rs](https://docs.rs/steam_shortcuts_util/latest/steam_shortcuts_util/) | [GitHub](https://github.com/PhilipK/steam_shortcuts_util)
- **Features**:
  - Parse and write Steam shortcuts.vdf (binary format)
  - `parse_shortcuts()` from byte slice, `shortcuts_to_bytes()` for writing
  - `calculate_app_id_for_shortcut()` for generating shortcut AppIDs
  - From the BoilR author (PhilipK)
- **Caveat**: Steam must be restarted for shortcuts.vdf changes to take effect. Also, shortcut IDs change unpredictably on each addition ([Issue #9463](https://github.com/ValveSoftware/steam-for-linux/issues/9463)).
- **Confidence**: High -- battle-tested in BoilR.

#### andygrunwald/vdf (Go alternative)

- **Install**: `go get github.com/andygrunwald/vdf`
- **Docs**: [pkg.go.dev](https://pkg.go.dev/github.com/andygrunwald/vdf) | [GitHub](https://github.com/andygrunwald/vdf)
- **Provides**: VDF lexer and parser for Go. Only relevant if Go is chosen as the backend language.
- **Confidence**: High for Go projects.

### Reference Tools (Existing Linux Game Management)

#### SteamTinkerLaunch

- **Repo**: [GitHub](https://github.com/sonic2kk/steamtinkerlaunch)
- **Architecture**: Single large Bash script, used as a Steam launch option wrapper (`steamtinkerlaunch %command%`)
- **Relevance**: Demonstrates how to intercept Steam game launches, configure per-game settings, and run auxiliary tools. Shows the pattern of injecting into the Steam launch pipeline via launch options.
- **Confidence**: High as a reference implementation.

#### BoilR

- **Repo**: [GitHub](https://github.com/PhilipK/BoilR)
- **Architecture**: Rust application that synchronizes non-Steam games into Steam library
- **Relevance**: Demonstrates programmatic shortcuts.vdf manipulation, Steam library discovery, and artwork management. Available as both native and Flatpak.
- **Confidence**: High as a reference implementation.

#### scanmem / GameConqueror

- **Repo**: [GitHub](https://github.com/scanmem/scanmem)
- **Architecture**: C library (libscanmem) + GTK+ Python frontend (GameConqueror)
- **Relevance**: Demonstrates Linux game memory scanning and modification. Works with native Linux games, WINE games, and Proton games. Uses ptrace and /proc/pid/mem for memory access.
- **Confidence**: High -- mature, widely used.

#### PINCE

- **Repo**: [GitHub](https://github.com/korcankaraokcu/PINCE)
- **Architecture**: Python frontend over GDB, with libscanmem integration
- **Relevance**: Full Cheat Engine alternative for Linux. Demonstrates .so injection, memory scanning, breakpoints, and variable locking on Linux (including WINE processes).
- **Confidence**: High as a reference for advanced memory manipulation techniques.

## Integration Patterns

### Recommended Architecture

```
+------------------+     +-------------------+     +-------------------+
|   Native Linux   |     |    Steam Client   |     |  WINE/Proton      |
|   CrossHook UI   |     |    (background)   |     |  Game Process     |
|   (Tauri/GTK4)   |     |                   |     |  (target)         |
+--------+---------+     +--------+----------+     +--------+----------+
         |                         |                         |
         |  steam -applaunch      |   Launches game via     |
         +------------------------>   Proton runtime        |
         |                         +------------------------>
         |                                                   |
         |  process_vm_readv/writev (same-user)             |
         +-------------------------------------------------->
         |  /proc/<pid>/maps (memory layout discovery)      |
         +-------------------------------------------------->
         |                                                   |
         |  proton run <trainer.exe> (clean env)            |
         +-------------------------------------------------->
         |                                                   |
         |  pgrep / /proc enumeration (process monitoring)  |
         +-------------------------------------------------->
```

### Steam Game Discovery Flow

1. **Locate Steam installation**: Check `~/.local/share/Steam/` (default) or `~/.steam/steam/` (symlink)
2. **Parse library folders**: Read and parse `steamapps/libraryfolders.vdf` using `steam-vdf-parser`
3. **Enumerate installed games**: For each library folder, glob `steamapps/appmanifest_*.acf` files
4. **Parse app manifests**: Extract AppID, name, install directory, and state from each ACF file
5. **Detect Proton prefix**: Check for `steamapps/compatdata/<appid>/pfx/` existence
6. **Identify Proton version**: Parse `config_info` or match against `steamapps/common/Proton*/`
7. **Build game database**: Combine all data into a local game catalog for the UI

```rust
// Pseudocode for game discovery
use steam_vdf_parser::parse_text;

fn discover_games() -> Vec<GameInfo> {
    let steam_root = find_steam_root(); // ~/.local/share/Steam
    let library_vdf = fs::read_to_string(
        steam_root.join("steamapps/libraryfolders.vdf")
    )?;
    let libraries = parse_text(&library_vdf)?;

    let mut games = Vec::new();
    for library_path in extract_library_paths(&libraries) {
        let steamapps = library_path.join("steamapps");
        for entry in glob(&steamapps.join("appmanifest_*.acf")) {
            let acf = fs::read_to_string(&entry)?;
            let manifest = parse_text(&acf)?;
            let appid = manifest.get("appid")?;
            let name = manifest.get("name")?;
            let installdir = manifest.get("installdir")?;

            let compatdata = steamapps.join("compatdata").join(appid);
            let has_proton = compatdata.join("pfx").exists();

            games.push(GameInfo {
                appid, name, installdir,
                library_path: library_path.clone(),
                proton_prefix: if has_proton { Some(compatdata) } else { None },
            });
        }
    }
    games
}
```

### Game Launch Flow

```rust
// Pseudocode for launching a game via Steam CLI
use std::process::Command;

fn launch_game(appid: &str) -> Result<()> {
    let steam = find_steam_command(); // "steam" or path to steam.sh

    Command::new(&steam)
        .arg("-applaunch")
        .arg(appid)
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()?;

    Ok(())
}
```

### Trainer Injection Flow (Replicating Existing Shell Scripts)

The existing `steam-host-trainer-runner.sh` demonstrates the proven pattern:

1. **Clean the environment**: Strip all WINE/Proton variables inherited from any parent session
2. **Set fresh Proton environment**: Export `STEAM_COMPAT_DATA_PATH`, `STEAM_COMPAT_CLIENT_INSTALL_PATH`, `WINEPREFIX`
3. **Stage trainer**: Copy trainer executable into the compatdata prefix (`pfx/drive_c/CrossHook/StagedTrainers/`)
4. **Launch via Proton**: Execute `<proton_path>/proton run <windows_path_to_trainer>`

```rust
// Pseudocode for trainer launch (replicating shell script logic)
use std::process::Command;

fn launch_trainer(
    proton_path: &Path,
    compatdata: &Path,
    steam_client: &Path,
    trainer_host_path: &Path,
) -> Result<()> {
    // Stage trainer into compatdata prefix
    let trainer_name = trainer_host_path.file_name().unwrap();
    let staged_dir = compatdata.join("pfx/drive_c/CrossHook/StagedTrainers");
    fs::create_dir_all(&staged_dir)?;
    fs::copy(trainer_host_path, staged_dir.join(trainer_name))?;

    let windows_path = format!("C:\\CrossHook\\StagedTrainers\\{}", trainer_name.to_str().unwrap());

    // Launch with clean environment
    Command::new(proton_path.join("proton"))
        .arg("run")
        .arg(&windows_path)
        .env_clear()  // Strip ALL inherited env
        .env("HOME", env::var("HOME")?)
        .env("USER", env::var("USER")?)
        .env("PATH", "/usr/bin:/bin")
        .env("STEAM_COMPAT_DATA_PATH", compatdata)
        .env("STEAM_COMPAT_CLIENT_INSTALL_PATH", steam_client)
        .env("WINEPREFIX", compatdata.join("pfx"))
        .env("DISPLAY", env::var("DISPLAY").unwrap_or_default())
        .env("WAYLAND_DISPLAY", env::var("WAYLAND_DISPLAY").unwrap_or_default())
        .env("XDG_RUNTIME_DIR", env::var("XDG_RUNTIME_DIR").unwrap_or_default())
        .spawn()?;

    Ok(())
}
```

### Native Memory Access Pattern (Advanced -- Future Phase)

For direct trainer-like memory manipulation from the native app (bypassing the need to run Windows trainers under Proton):

```rust
use nix::sys::uio::{process_vm_readv, process_vm_writev, RemoteIoVec};
use nix::unistd::Pid;
use std::io::IoSlice;
use std::io::IoSliceMut;

fn read_game_memory(pid: i32, address: u64, size: usize) -> Result<Vec<u8>> {
    let mut buffer = vec![0u8; size];
    let local_iov = [IoSliceMut::new(&mut buffer)];
    let remote_iov = [RemoteIoVec { base: address as usize, len: size }];

    process_vm_readv(Pid::from_raw(pid), &local_iov, &remote_iov)?;
    Ok(buffer)
}

fn write_game_memory(pid: i32, address: u64, data: &[u8]) -> Result<()> {
    let local_iov = [IoSlice::new(data)];
    let remote_iov = [RemoteIoVec { base: address as usize, len: data.len() }];

    process_vm_writev(Pid::from_raw(pid), &local_iov, &remote_iov)?;
    Ok(())
}

fn find_game_pid(exe_name: &str) -> Option<i32> {
    // Enumerate /proc/*/cmdline or /proc/*/comm
    for entry in fs::read_dir("/proc").ok()? {
        let entry = entry.ok()?;
        let pid_str = entry.file_name().to_str()?.to_string();
        if pid_str.chars().all(|c| c.is_ascii_digit()) {
            let comm = fs::read_to_string(
                format!("/proc/{}/comm", pid_str)
            ).ok()?;
            if comm.trim() == exe_name || comm.trim() == exe_name.trim_end_matches(".exe") {
                return pid_str.parse().ok();
            }
        }
    }
    None
}

fn parse_memory_maps(pid: i32) -> Vec<MemoryRegion> {
    // Parse /proc/<pid>/maps to find module base addresses
    // WINE processes have predictable addresses:
    //   Game EXE:     0x00400000 (32-bit)
    //   kernel32:     0x7b400000
    //   ntdll:        0x7bc00000
    let maps = fs::read_to_string(format!("/proc/{}/maps", pid)).unwrap();
    // Parse each line: "start-end perms offset dev inode pathname"
    maps.lines().filter_map(|line| parse_map_line(line)).collect()
}
```

### Adding CrossHook as a Non-Steam Game

```rust
use steam_shortcuts_util::{parse_shortcuts, shortcuts_to_bytes, Shortcut};

fn add_crosshook_shortcut(
    steam_userdata: &Path,
    crosshook_path: &Path,
) -> Result<()> {
    let shortcuts_path = steam_userdata.join("config/shortcuts.vdf");
    let data = fs::read(&shortcuts_path)?;
    let mut shortcuts = parse_shortcuts(&data)?;

    let mut shortcut = Shortcut::new(
        "0",                           // id (auto-assigned)
        "CrossHook Loader",           // app_name
        crosshook_path.to_str()?,     // exe path
        "",                           // start_dir
        "",                           // icon
        "",                           // shortcut_path
        "",                           // launch_options
    );
    shortcut.tags = vec!["Tools".to_string()];

    shortcuts.push(shortcut);
    let bytes = shortcuts_to_bytes(&shortcuts);
    fs::write(&shortcuts_path, bytes)?;

    // NOTE: Steam must be restarted for changes to take effect
    Ok(())
}
```

### Gamescope / MangoHud Integration

- **Gamescope**: [ArchWiki - Gamescope](https://wiki.archlinux.org/title/Gamescope)
- Non-Steam games can be launched through Gamescope with: `gamescope -W 1920 -H 1080 -r 60 -- <application>`
- MangoHud integrates with Gamescope via the `--mangoapp` flag (not traditional `MANGOHUD=1`)
- Communication between MangoHud and Gamescope uses System V IPC message queues and X11 properties
- CrossHook should support passing Gamescope launch parameters as part of game configuration
- **Confidence**: High -- well-documented in ArchWiki.

## Constraints and Gotchas

### Steam Deck / SteamOS Constraints

- **Immutable filesystem**: SteamOS uses a read-only root partition. Native binaries should be distributed as AppImage or Flatpak. `sudo steamos-readonly disable` is temporary and reverted on updates. ([GamingOnLinux Guide](https://www.gamingonlinux.com/guides/view/how-to-install-extra-software-apps-and-games-on-steamos-and-steam-deck/))
- **Steam is native, not Flatpak**: On SteamOS, Steam is baked into the OS as a native application. This means CrossHook can interact with Steam's filesystem directly without Flatpak sandbox concerns IF CrossHook itself is also native (AppImage or placed in user home).
- **Gaming Mode visibility**: Native applications added as non-Steam games display correctly in Gaming Mode. Flatpak apps using their own bundled Steam Runtime may NOT display in the Gamescope session. ([Gamescope Issue #1341](https://github.com/ValveSoftware/gamescope/issues/1341))
- **Controller input**: Non-Steam games must be added to Steam library for Steam Input (controller remapping) to work. Flatpak apps may break Steam Overlay/Input. ([Heroic Launcher Issue #4708](https://github.com/Heroic-Games-Launcher/HeroicGamesLauncher/issues/4708))
- **Recommended distribution**: AppImage is the best format for Steam Deck -- no sandbox issues, single file, survives OS updates, can be added as non-Steam game.
- **Confidence**: High.

### Flatpak Sandbox Constraints

- **No /proc access**: Flatpak reserves `/proc` and `--filesystem=/proc` has no effect. This means a Flatpak CrossHook CANNOT use ptrace or process_vm_readv on game processes.
- **No ptrace**: Default Flatpak sandbox disallows ptrace. Only `--allow=devel` (for IDEs) enables it, and even then processes are in a PID namespace.
- **Inter-Flatpak communication**: Requires `org.freedesktop.Flatpak` portal permission, which breaks the sandbox model.
- **Conclusion**: **CrossHook must NOT be distributed as a Flatpak** if it needs process memory access. Use AppImage or native binary instead.
- **Confidence**: High -- [Flatpak Sandbox Documentation](https://docs.flatpak.org/en/latest/sandbox-permissions.html).

### Wayland vs X11 Considerations

- **Wayland overlay prohibition**: Wayland's design explicitly prevents one application from drawing over another application's window. Traditional game overlays (injecting into another app's rendering pipeline) are impossible under pure Wayland. ([Steam for Linux Issue #8020](https://github.com/ValveSoftware/steam-for-linux/issues/8020))
- **Gamescope as compositor**: On Steam Deck, Gamescope IS the compositor, and it has its own overlay mechanism (mangoapp). This is the correct approach for Gaming Mode overlays.
- **CrossHook UI approach**: CrossHook should run as a separate window, not an overlay. Use `layer-shell` protocol (wlr-layer-shell) if a persistent panel is needed, though this is compositor-specific.
- **X11 fallback**: Under X11, overlay windows are possible but being phased out. Targeting Wayland-first is the correct long-term strategy.
- **Proton games use XWayland**: Games running under Proton use XWayland, not native Wayland. This does not affect CrossHook's ability to access their memory.
- **Confidence**: High.

### Security and Permissions

- **ptrace_scope**: Most distributions default to `kernel.yama.ptrace_scope=1`, which blocks CrossHook from attaching to non-child processes. Solutions:
  - `setcap cap_sys_ptrace=eip` on the binary (recommended)
  - Polkit policy for privilege escalation
  - Configuration guide for users to adjust ptrace_scope
- **process_vm_readv/writev**: Subject to same restrictions as ptrace -- same UID or CAP_SYS_PTRACE required.
- **Game anti-cheat**: Some Linux-native anti-cheat systems (EAC, BattlEye with Proton support) may detect and block ptrace/memory access. CrossHook should document which games are compatible.
- **WINE process ownership**: WINE/Proton game processes run as the same user as Steam, which is the same user running CrossHook. Same-UID access is sufficient when ptrace_scope=0.
- **Confidence**: High.

### Process Namespace Isolation

- **WINE processes are standard Linux processes**: There is no namespace isolation between WINE processes and native Linux processes. WINE executables appear as regular processes in `/proc/`, can be listed with `ps`, and can be ptrace'd like any other process.
- **Proton's pressure-vessel container**: The Steam Runtime's container (pressure-vessel) uses user namespaces and mount namespaces, but the game process itself is still visible from the host PID namespace. Memory access from outside the container works normally.
- **PID visibility**: WINE game processes are visible via `pgrep`, `/proc` enumeration, and standard process monitoring tools. The process name in `/proc/<pid>/comm` is typically the executable name without `.exe` extension.
- **Confidence**: High -- confirmed by existing shell scripts using `pgrep -af` to detect WINE game processes.

## Code Examples

### Complete Tauri v2 Backend Setup for CrossHook

```rust
// src-tauri/src/lib.rs
use tauri::Manager;

mod steam;
mod process;
mod trainer;

#[tauri::command]
async fn discover_games() -> Result<Vec<steam::GameInfo>, String> {
    steam::discover_installed_games().map_err(|e| e.to_string())
}

#[tauri::command]
async fn launch_game(appid: String) -> Result<(), String> {
    steam::launch_game(&appid).map_err(|e| e.to_string())
}

#[tauri::command]
async fn launch_trainer(
    appid: String,
    trainer_path: String,
) -> Result<(), String> {
    let game_info = steam::get_game_info(&appid).map_err(|e| e.to_string())?;
    trainer::launch_trainer_proton(
        &game_info.proton_path,
        &game_info.compatdata_path,
        &game_info.steam_client_path,
        &trainer_path,
    ).map_err(|e| e.to_string())
}

#[tauri::command]
async fn find_game_process(exe_name: String) -> Result<Option<u32>, String> {
    Ok(process::find_process_by_name(&exe_name))
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_process::init())
        .invoke_handler(tauri::generate_handler![
            discover_games,
            launch_game,
            launch_trainer,
            find_game_process,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
```

### Memory Map Parsing for WINE Process

```rust
// Parse /proc/<pid>/maps to find Windows module base addresses
use std::fs;

#[derive(Debug)]
struct MemoryRegion {
    start: u64,
    end: u64,
    perms: String,
    pathname: Option<String>,
}

fn parse_proc_maps(pid: u32) -> Vec<MemoryRegion> {
    let maps = fs::read_to_string(format!("/proc/{}/maps", pid))
        .expect("Failed to read /proc maps");

    maps.lines().filter_map(|line| {
        let parts: Vec<&str> = line.splitn(6, char::is_whitespace).collect();
        if parts.len() < 5 { return None; }

        let addr_parts: Vec<&str> = parts[0].split('-').collect();
        let start = u64::from_str_radix(addr_parts[0], 16).ok()?;
        let end = u64::from_str_radix(addr_parts[1], 16).ok()?;
        let perms = parts[1].to_string();
        let pathname = if parts.len() >= 6 {
            Some(parts[5].trim().to_string())
        } else {
            None
        };

        Some(MemoryRegion { start, end, perms, pathname })
    }).collect()
}

fn find_module_base(pid: u32, module_name: &str) -> Option<u64> {
    let regions = parse_proc_maps(pid);
    regions.iter()
        .find(|r| r.pathname.as_ref()
            .map_or(false, |p| p.contains(module_name)))
        .map(|r| r.start)
}

// WINE-specific: well-known base addresses (32-bit games)
const WINE_EXE_BASE_32: u64 = 0x00400000;
const WINE_KERNEL32_BASE: u64 = 0x7b400000;
const WINE_NTDLL_BASE: u64 = 0x7bc00000;
```

## UI Framework Comparison Matrix

| Feature                    | Tauri v2                       | GTK4/Relm4                | Qt6 (cxx-qt)            |
| -------------------------- | ------------------------------ | ------------------------- | ----------------------- |
| **Language**               | Rust + Web frontend            | Pure Rust                 | Rust via cxx-qt         |
| **Bundle size**            | 3-5 MB                         | ~5-15 MB (depends on GTK) | ~20-50 MB               |
| **Cross-platform**         | Linux, Windows, macOS, mobile  | Linux (primarily)         | Linux, Windows, macOS   |
| **Native look**            | Uses system WebView            | Native GNOME              | Native per-platform     |
| **Rust maturity**          | Stable (v2)                    | Stable (gtk-rs 0.9+)      | Early (cxx-qt 0.7)      |
| **Learning curve**         | Moderate (web + Rust)          | Moderate (GTK + Elm)      | High (C++ bridge)       |
| **Steam Deck Gaming Mode** | Works as non-Steam game        | Works as non-Steam game   | Works as non-Steam game |
| **Process management**     | Via Shell plugin + Rust        | Via nix crate directly    | Via nix crate directly  |
| **Community**              | Very active (23k+ stars)       | Active (gtk-rs ecosystem) | Growing                 |
| **Best for**               | Cross-platform with web skills | GNOME-first, Linux-only   | KDE/Qt ecosystem        |

## Distribution Strategy Comparison

| Method             | Steam Deck                       | Regular Linux           | Survives OS Updates | Sandbox Issues           |
| ------------------ | -------------------------------- | ----------------------- | ------------------- | ------------------------ |
| **AppImage**       | Works, add as non-Steam game     | Works, download and run | Yes                 | None                     |
| **Flatpak**        | Works, but sandbox blocks ptrace | Works                   | Yes                 | **Blocks /proc, ptrace** |
| **Native binary**  | Needs `steamos-readonly disable` | Works                   | **Wiped on update** | None                     |
| **systemd-sysext** | Works, survives updates          | N/A (SteamOS feature)   | Yes                 | None                     |
| **Distrobox/Nix**  | Works                            | Works                   | Yes                 | May complicate paths     |

**Recommendation**: AppImage for Steam Deck and general Linux distribution. Provides zero sandbox restrictions and single-file simplicity.

## Open Questions

1. **Trainer format compatibility**: Can Linux-native memory manipulation replicate what Windows trainers (FLiNG, WeMod) do, or must we always run Windows trainers under Proton? FLiNG trainers use Windows APIs (ReadProcessMemory/WriteProcessMemory) which map to the same underlying operations, but cheat tables and trainer scripts would need porting.

2. **Anti-cheat interaction**: How do EAC/BattlEye Proton implementations respond to ptrace/process_vm_readv from outside the WINE environment? This needs empirical testing per game.

3. **Multiple Proton version support**: When a user has multiple Proton versions installed (stable, experimental, GE-Proton), how should CrossHook determine which version to use for launching trainers? Should it match the game's configured Proton version?

4. **Steam client restart requirement**: Modifying shortcuts.vdf requires restarting Steam. Is there a way to notify Steam of changes without a full restart? (Current evidence suggests no.)

5. **CAP_SYS_PTRACE UX**: How to gracefully handle the case where the user hasn't granted ptrace capabilities? Should CrossHook provide a setup wizard that runs `setcap` with polkit escalation?

6. **Gaming Mode UX**: In Steam Deck Gaming Mode, CrossHook runs as a non-Steam game. How should it present game selection and trainer configuration using only controller input? Gamescope provides touch support and virtual keyboard.

7. **Tauri WebView performance**: WebKitGTK is the only available WebView on Linux. Are there performance concerns for the types of UI CrossHook needs (game lists, configuration panels, process monitoring)?

## Uncertainties and Gaps

- **Steam private IPC**: Steam's internal IPC mechanisms (how it communicates between the Steam client and game processes) are proprietary and undocumented. There is no official API for programmatic game management beyond CLI commands.
- **Proton version detection per game**: There is no standardized file or API that maps a game's AppID to its configured Proton version. This may require parsing Steam's `config.vdf` or `localconfig.vdf` in the userdata directory, which is underdocumented.
- **WeMod compatibility**: WeMod uses its own injection mechanism. Whether WeMod trainers work when launched via `proton run` from outside WINE needs testing.
- **Steam Deck Gaming Mode controller mapping**: How non-Steam games receive controller input in Gaming Mode depends on Steam Input configuration; this needs practical testing with CrossHook's UI framework.

## Search Queries Executed

1. `Steam CLI launch game from external application Linux steam:// protocol 2025 2026`
2. `Linux process injection ptrace /proc/pid/mem LD_PRELOAD game trainer injection 2025`
3. `GTK4 libadwaita vs Qt6 vs Tauri Linux desktop application comparison 2025 2026`
4. `Steam Deck non-Steam game integration gaming mode Flatpak constraints 2025`
5. `Steam VDF ACF file parser library Python Rust Go parse Steam libraryfolders.vdf appmanifest`
6. `Tauri v2 desktop application Linux 2025 features cross-platform native`
7. `Steam Proton prefix discovery compatdata Linux find game prefix path`
8. `linux-inject ptrace dlopen shared library injection game modding 2024 2025`
9. `Steam IPC socket mechanism Linux D-Bus interface steam-runtime communication`
10. `Gamescope MangoHud integration non-Steam game overlay Linux 2025`
11. `ptrace WINE Proton process memory access cross-environment injection Linux native to WINE`
12. `Wayland X11 overlay window Linux game overlay toolkit 2025`
13. `Steam shortcuts.vdf add non-Steam game programmatically Linux BoilR`
14. `kubo injector library cross-platform process injection shared library Linux`
15. `Tauri v2 system commands process management spawn child process Rust backend`
16. `Steam libraryfolders.vdf location Linux multiple library folders parse installed games`
17. `Flatpak Steam Deck filesystem access /proc ptrace sandbox escape gaming tools`
18. `Rust GTK4 libadwaita application example gtk-rs 2025`
19. `Steam Deck SteamOS add non-Steam game shortcut programmatically desktop file gaming mode`
20. `BoilR Rust Steam shortcuts.vdf parser implementation non-Steam game management`
21. `process_vm_readv process_vm_writev Linux game memory read write without ptrace attach`
22. `Rust nix crate ptrace process_vm_readv Linux process management 2025`
23. `Steam Deck SteamOS native application /proc access ptrace gaming mode not Flatpak`
24. `Proton wine process tree Linux native process can ptrace wine game process memory`
25. `wine process /proc/pid/maps memory layout virtual address space PE module base address Linux`
26. `yama ptrace_scope Linux gaming security bypass CAP_SYS_PTRACE process_vm_readv 2025`
27. `SteamTinkerLaunch architecture how it works launch options wrapper script Linux`
28. `Linux game trainer cheat engine alternative open source scanmem GameConqueror process memory`
29. `Steam Deck native binary install not Flatpak SteamOS immutable filesystem workaround`
30. `PINCE Linux game memory editor debugger ptrace alternative cheat engine 2025`
31. `Rust process_vm_readv process_vm_writev crate Linux memory manipulation game`
32. `steam:// protocol URL handler rungameid applaunch command line Linux documentation`
33. `Tauri vs GTK4 Rust desktop application Linux performance bundle size comparison`
34. `Relm4 Rust GTK4 framework reactive UI 2025`
35. `AppImage Steam Deck distribution native Linux application packaging 2025`
36. `Steam browser protocol commands complete list run rungameid install connect open validate`

## Sources

### Steam Integration

- [Valve Developer Community - Steam Browser Protocol](https://developer.valvesoftware.com/wiki/Steam_browser_protocol)
- [Valve Developer Community - Command Line Options](https://developer.valvesoftware.com/wiki/Command_line_options)
- [Steam for Linux - CLI Launch Discussion](https://steamcommunity.com/app/221410/discussions/0/1621724915820500210/)
- [Steam for Linux - Headless Launch Feature Request #4035](https://github.com/ValveSoftware/steam-for-linux/issues/4035)
- [Steam for Linux - Shortcut ID Issue #9463](https://github.com/ValveSoftware/steam-for-linux/issues/9463)
- [Proton FAQ - ValveSoftware](https://github.com/ValveSoftware/Proton/wiki/Proton-FAQ)
- [Proton CLI Launch Gist](https://gist.github.com/sxiii/6b5cd2e7d2321df876730f8cafa12b2e)
- [Steam ArchWiki](https://wiki.archlinux.org/title/Steam)

### Process Injection and Memory Access

- [GreyNoise Labs - Linux Process Injection (Jan 2025)](https://www.labs.greynoise.io/grimoire/2025-01-28-process-injection/)
- [Akamai - Definitive Guide to Linux Process Injection](https://www.akamai.com/blog/security-research/the-definitive-guide-to-linux-process-injection)
- [ptrace(2) Linux man page](https://man7.org/linux/man-pages/man2/ptrace.2.html)
- [process_vm_readv(2) Linux man page](https://man7.org/linux/man-pages/man2/process_vm_readv.2.html)
- [Attacking WINE Part I - Memory Layout](https://schlafwandler.github.io/posts/attacking-wine-part-i/)
- [linux-inject - GitHub](https://github.com/gaffe23/linux-inject)
- [kubo/injector - GitHub (archived)](https://github.com/kubo/injector)
- [Yama LSM - Linux Kernel Documentation](https://docs.kernel.org/admin-guide/LSM/Yama.html)
- [nullprogram - Read/Write Process Memory](https://nullprogram.com/blog/2016/09/03/)

### UI Frameworks

- [Tauri v2 Documentation](https://v2.tauri.app/)
- [Tauri v2 Shell Plugin](https://v2.tauri.app/plugin/shell/)
- [Tauri v2 Sidecar Documentation](https://v2.tauri.app/develop/sidecar/)
- [Tauri GitHub Repository](https://github.com/tauri-apps/tauri)
- [Relm4 - Rust GTK4 Framework](https://relm4.org/)
- [Relm4 Book](https://relm4.org/book/stable/)
- [gtk-rs - GTK4 Rust Bindings](https://gtk-rs.org/)
- [libadwaita Rust Crate](https://lib.rs/crates/libadwaita)

### Rust Crates

- [nix crate - ptrace module](https://docs.rs/nix/latest/nix/sys/ptrace/index.html)
- [nix crate - process_vm_readv](https://docs.rs/nix/latest/nix/sys/uio/fn.process_vm_readv.html)
- [process_vm_io crate](https://crates.io/crates/process_vm_io)
- [procmem-linux crate](https://crates.io/crates/procmem-linux)
- [steam-vdf-parser crate](https://docs.rs/steam-vdf-parser/latest/steam_vdf_parser/)
- [steam_shortcuts_util crate](https://docs.rs/steam_shortcuts_util/latest/steam_shortcuts_util/)
- [steam_shortcuts_util GitHub](https://github.com/PhilipK/steam_shortcuts_util)

### Reference Implementations

- [SteamTinkerLaunch - GitHub](https://github.com/sonic2kk/steamtinkerlaunch)
- [BoilR - GitHub](https://github.com/PhilipK/BoilR)
- [scanmem/GameConqueror - GitHub](https://github.com/scanmem/scanmem)
- [PINCE - GitHub](https://github.com/korcankaraokcu/PINCE)
- [NonSteamLaunchers-On-Steam-Deck - GitHub](https://github.com/moraroy/NonSteamLaunchers-On-Steam-Deck)

### Steam Deck and SteamOS

- [Gamescope ArchWiki](https://wiki.archlinux.org/title/Gamescope)
- [MangoHud GitHub](https://github.com/flightlessmango/MangoHud)
- [Steam Deck FAQ - Steamworks](https://partner.steamgames.com/doc/steamdeck/faq)
- [Flatpak Sandbox Permissions](https://docs.flatpak.org/en/latest/sandbox-permissions.html)
- [GamingOnLinux - How to Install Extra Software on SteamOS](https://www.gamingonlinux.com/guides/view/how-to-install-extra-software-apps-and-games-on-steamos-and-steam-deck/)
- [Gamescope Issue #1341 - Flatpak visibility](https://github.com/ValveSoftware/gamescope/issues/1341)

### Wayland / Display

- [Steam for Linux - Wayland Overlay Issue #8020](https://github.com/ValveSoftware/steam-for-linux/issues/8020)
- [MangoHud 0.8 Release - Wayland Keybinds](https://www.gamingonlinux.com/2025/02/linux-gaming-overlay-mangohud-version-0-8-is-out-now-with-intel-gpu-support-improved-wayland-keybinds/)
