# External API Research: update-game

## Executive Summary

The update-game feature requires running a Windows update/patch executable inside an existing Proton prefix -- fundamentally the same operation CrossHook already performs for install-game, but targeting an existing prefix with an existing game installation rather than provisioning a new one. The core technical challenge is correctly resolving the target prefix (whether a Steam-managed `compatdata/<appid>/pfx` prefix or a standalone CrossHook-managed prefix), setting the required environment variables (`STEAM_COMPAT_DATA_PATH`, `WINEPREFIX`, `STEAM_COMPAT_CLIENT_INSTALL_PATH`), and invoking `proton run <updater.exe>` with the `waitforexitandrun` verb. CrossHook's existing `launch/runtime_helpers.rs` and `install/service.rs` already contain 90%+ of the infrastructure needed; the new module primarily needs to compose these primitives with prefix-existence validation and appropriate working directory resolution.

---

## Primary APIs

### Proton CLI (proton script)

- **Documentation**: [ValveSoftware/Proton GitHub](https://github.com/ValveSoftware/Proton) | [DeepWiki: Wine Prefix Management](https://deepwiki.com/ValveSoftware/Proton/2.2-wine-prefix-management)
- **Authentication**: None -- local filesystem binary
- **Key Commands/Verbs**:
  - `proton run <exe>` -- Launches an executable within the configured prefix. This is the default verb CrossHook already uses for `proton_run` launches. It sets up the prefix, applies DLL overrides, configures graphics translation layers, and runs the target.
  - `proton waitforexitandrun <exe>` -- Same as `run` but explicitly waits for the wineserver to exit before returning. This is the verb umu-launcher defaults to. For a blocking update operation, this is the preferred verb.
  - `proton runinprefix <exe>` -- Runs a command within the prefix without waiting for wineserver shutdown. Useful if running a second executable concurrently in the same prefix.
  - `proton getcompatpath <linux-path>` -- Converts a Linux path to a Windows path via winepath. Could be useful for translating update executable paths for display purposes.
  - `proton createprefix` -- Creates/initializes a prefix without running an executable. Not needed for update-game since the prefix already exists.
- **Rate Limits**: N/A
- **Constraints**:
  - Modern Proton (6+) prevents simultaneous execution when Steam actively controls the prefix. The game must not be running during the update.
  - Proton acquires a `pfx.lock` file lock (`FileLock(self.path("pfx.lock"), timeout=-1)`) during prefix operations. If the game is running, the lock will block indefinitely.
  - Prefix version upgrades may trigger automatically when running with a different Proton version than was used to create the prefix. This is usually harmless but should be noted in logs.

**Confidence**: High -- Verified against Proton source code and multiple community guides.

### Environment Variables (Proton/WINE)

- **Documentation**: [Proton README](https://github.com/ValveSoftware/Proton) | [Run .exe in existing prefix gist](https://gist.github.com/michaelbutler/f364276f4030c5f449252f2c4d960bd2) | [Proton FAQ](https://github.com/ValveSoftware/Proton/wiki/Proton-FAQ)

#### Required Variables

| Variable                           | Purpose                                                                                                                                             | How CrossHook Sets It                                                                                                                                                                              |
| ---------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `STEAM_COMPAT_DATA_PATH`           | Points to the compatdata root (parent of `pfx/`). Proton uses this to locate the prefix and manage version tracking, tracked files, and lock files. | `runtime_helpers::apply_runtime_proton_environment()` -- resolves from configured prefix path. If the prefix path ends in `pfx`, it walks up to the parent.                                        |
| `WINEPREFIX`                       | Points to the actual Wine prefix directory (the `pfx/` subdirectory or the prefix root for standalone prefixes).                                    | Same function. Uses `resolve_wine_prefix_path()` to detect whether a `pfx/` child exists.                                                                                                          |
| `STEAM_COMPAT_CLIENT_INSTALL_PATH` | Path to the Steam client installation. Required by Proton for locating Steam runtime components.                                                    | `runtime_helpers::resolve_steam_client_install_path()` -- falls back through configured value, environment variable, and well-known paths (`~/.local/share/Steam`, `~/.steam/root`, Flatpak path). |

#### Optional Variables Relevant to Updates

| Variable                    | Purpose                                                                                                   | Default                                                                     |
| --------------------------- | --------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------- |
| `PROTON_LOG`                | Enables Wine debug logging to `~/steam-<appid>.log`                                                       | Disabled                                                                    |
| `PROTON_VERB`               | Controls execution behavior (`run`, `waitforexitandrun`, `runinprefix`)                                   | `waitforexitandrun` when using umu-run; `run` when invoking proton directly |
| `STEAM_COMPAT_INSTALL_PATH` | Game installation directory. Used by some Proton features (S: drive mapping via `PROTON_SET_GAME_DRIVE`). | Not set by default                                                          |

#### Variables CrossHook Clears Before Launch

CrossHook's `env.rs` defines `WINE_ENV_VARS_TO_CLEAR` (31 variables including `WINESERVER`, `WINEDLLPATH`, `LD_PRELOAD`, `SteamGameId`, `PROTON_LOG`, etc.) and `PASSTHROUGH_DISPLAY_VARS` (display server variables). The `runtime_helpers::apply_host_environment()` function selectively passes through only safe host variables (`HOME`, `USER`, `PATH`, `DISPLAY`, `WAYLAND_DISPLAY`, `XDG_RUNTIME_DIR`, `DBUS_SESSION_BUS_ADDRESS`).

**Confidence**: High -- CrossHook already implements this exact pattern for game launch and install.

### Protontricks CLI

- **Documentation**: [GitHub - Matoking/protontricks](https://github.com/Matoking/protontricks) | [protontricks man page](https://linuxcommandlibrary.com/man/protontricks)
- **Key Commands**:
  - `protontricks <APPID> wine <exe>` -- Runs an executable in a Steam game's prefix. Handles environment setup automatically.
  - `protontricks-launch --appid <APPID> <exe>` -- Alternative launch syntax.
  - `protontricks -c "<command>" <APPID>` -- Runs arbitrary commands in a prefix.
- **Assessment for CrossHook**: Protontricks is a **complementary tool, not a dependency**. CrossHook already manages prefix environment setup directly through Proton. Adding protontricks as a dependency would introduce a Python/winetricks runtime requirement. CrossHook should continue invoking `proton run` directly, which is more predictable and has fewer moving parts.

**Confidence**: High -- Well-documented, actively maintained project.

### UMU Launcher

- **Documentation**: [GitHub - Open-Wine-Components/umu-launcher](https://github.com/Open-Wine-Components/umu-launcher) | [umu(1) man page](https://man.archlinux.org/man/umu.1.en) | [FAQ](<https://github.com/Open-Wine-Components/umu-launcher/wiki/Frequently-asked-questions-(FAQ)>)
- **Key Features**:
  - Runs Windows games via Proton outside of Steam, replicating Steam's containerized runtime environment.
  - Supports TOML configuration files for persistent game settings.
  - Automatically downloads and manages the Steam Runtime.
  - Supports `PROTON_VERB` for controlling execution behavior.
- **Key Environment Variables**:
  - `WINEPREFIX` -- prefix path
  - `PROTONPATH` -- Proton version path (or name like `GE-Proton9-5`)
  - `GAMEID` -- maps to umu-database for game-specific fixes
  - `PROTON_VERB` -- execution verb (`waitforexitandrun`, `run`, `runinprefix`)
- **Assessment for CrossHook**: UMU is the direction Lutris, Heroic, and Bottles are converging on for running Proton outside Steam. However, CrossHook profiles already store prefix path and Proton path explicitly, and CrossHook invokes `proton run` directly rather than going through a launcher layer. **UMU integration is a separate feature** (useful for non-Steam-installed Proton builds like GE-Proton), not a prerequisite for update-game.
- **Rust Wrapper**: The [`umu-wrapper`](https://crates.io/crates/umu-wrapper) crate exists on crates.io but appears to be an early-stage wrapper. Not recommended as a dependency at this time.

**Confidence**: Medium -- UMU is actively evolving. The recommendation to defer integration is high confidence.

---

## Libraries and SDKs

### Recommended Libraries (Already in CrossHook)

CrossHook's existing dependency set is sufficient for update-game. No new crates are required.

- **`tokio` (1.x)** with `process` feature -- Async process spawning, environment injection, stdout/stderr capture. Already used by `install/service.rs` and `launch/script_runner.rs`.
  - Docs: [tokio::process::Command](https://docs.rs/tokio/latest/tokio/process/struct.Command.html)
  - Pattern: `Command::new(proton_path).arg("run").arg(updater_exe).env_clear().env(key, value).spawn()`

- **`serde` (1.x)** with `derive` -- Serialization for `UpdateGameRequest`/`UpdateGameResult` types crossing the IPC boundary.

- **`toml` (0.8)** -- TOML parsing for profile files (reading prefix path, Proton path from existing profiles).

- **`directories` (5.x)** -- Resolving platform-appropriate paths for log files and default prefix locations.

- **`tracing` (0.1)** -- Structured logging for update operations.

**Confidence**: High -- These are existing dependencies with proven patterns in the codebase.

### Alternative/Optional Libraries

| Crate                                                                                    | Purpose                                                 | Assessment                                                                                                    |
| ---------------------------------------------------------------------------------------- | ------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------- |
| [`steam-vdf-parser`](https://lib.rs/crates/steam-vdf-parser)                             | Zero-copy VDF parser supporting text and binary formats | CrossHook has its own `steam/vdf.rs` parser. No need to add.                                                  |
| [`vdf-reader`](https://lib.rs/crates/vdf-reader)                                         | VDF v1 (KeyValues) parser                               | Same -- CrossHook's existing parser is sufficient.                                                            |
| [`umu-wrapper`](https://crates.io/crates/umu-wrapper)                                    | Rust wrapper around umu-launcher                        | Early stage, limited documentation. Not recommended yet.                                                      |
| [`tokio-process-tools`](https://docs.rs/tokio-process-tools/latest/tokio_process_tools/) | Advanced output handling for spawned processes          | Potentially useful for real-time progress streaming, but overkill for the initial update-game implementation. |

**Confidence**: High -- The assessment that no new dependencies are needed is based on direct codebase inspection.

---

## Integration Patterns

### Recommended Approach: Reuse `install/service.rs` Pattern

The update-game feature should follow the exact same pattern as `install_game()` in `install/service.rs`, with these key differences:

1. **Prefix must already exist** (validation, not provisioning)
2. **No executable discovery** needed post-update (the game executable path is already known from the profile)
3. **Working directory** defaults to the updater executable's parent directory

#### Execution Flow

```
User selects profile --> Profile provides prefix_path + proton_path
                    --> User browses to update .exe
                    --> CrossHook validates inputs
                    --> CrossHook builds `proton run <updater.exe>` Command
                    --> CrossHook spawns process, waits for exit
                    --> CrossHook reports success/failure with log path
```

### Prefix Resolution Flow

The prefix path comes from the selected profile. CrossHook must handle two prefix layouts:

```
Steam-managed prefix (profile has steam.compatdata_path):
  ~/.local/share/Steam/steamapps/compatdata/<appid>/
    pfx/              <-- WINEPREFIX points here
      drive_c/        <-- Game files live here
      system.reg
      user.reg
    pfx.lock          <-- Proton file lock
    version           <-- Prefix version tracking
    tracked_files     <-- Proton-managed file list

Standalone CrossHook prefix (profile has runtime.prefix_path):
  ~/.local/share/crosshook/prefixes/<slug>/
    [pfx/]            <-- May or may not have pfx/ subdirectory
      drive_c/
      system.reg
      user.reg
```

CrossHook's existing `resolve_wine_prefix_path()` in `runtime_helpers.rs` already handles both layouts:

- If the path ends in `pfx`, use it directly as WINEPREFIX
- If a `pfx/` child directory exists, use that as WINEPREFIX
- Otherwise, use the path itself as WINEPREFIX

The `resolve_compat_data_path()` function correctly derives `STEAM_COMPAT_DATA_PATH` by walking up from the wine prefix path when it ends in `pfx`.

### Environment Setup

Reuse `apply_runtime_proton_environment()` and `apply_host_environment()` directly:

```rust
// Pseudocode for build_update_command()
fn build_update_command(
    request: &UpdateGameRequest,
    log_path: &Path,
) -> Result<Command, UpdateGameError> {
    let mut command = new_direct_proton_command(request.proton_path.trim());
    command.arg(request.updater_path.trim());
    apply_host_environment(&mut command);
    apply_runtime_proton_environment(
        &mut command,
        request.prefix_path.trim(),
        request.steam_client_install_path.trim(),
    );
    apply_working_directory(
        &mut command,
        "",
        Path::new(request.updater_path.trim()),
    );
    attach_log_stdio(&mut command, log_path)?;
    Ok(command)
}
```

This produces the equivalent of the manual command:

```bash
STEAM_COMPAT_DATA_PATH="/path/to/compatdata" \
STEAM_COMPAT_CLIENT_INSTALL_PATH="/home/user/.local/share/Steam" \
WINEPREFIX="/path/to/compatdata/pfx" \
HOME="$HOME" USER="$USER" PATH="$PATH" \
DISPLAY="$DISPLAY" WAYLAND_DISPLAY="$WAYLAND_DISPLAY" \
XDG_RUNTIME_DIR="$XDG_RUNTIME_DIR" \
/path/to/proton run /path/to/update.exe
```

### How Other Launchers Handle This

| Launcher                  | Approach                                                                                                                                                                                               | Reference                                                                                                                                     |
| ------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | --------------------------------------------------------------------------------------------------------------------------------------------- |
| **Heroic Games Launcher** | `runWineCommand()` in `launcher.ts` sets WINEPREFIX and calls wine/proton with the executable. Uses game settings to determine prefix and wine version. Provides "Run EXE in Prefix" as a menu option. | [GitHub PR #1568](https://github.com/Heroic-Games-Launcher/HeroicGamesLauncher/pull/1568)                                                     |
| **Lutris**                | v0.5.20+ uses umu-launcher for Proton integration. Manages one prefix per game. Running executables in a prefix goes through the same launch pipeline with a different target executable.              | [GamingOnLinux](https://www.gamingonlinux.com/2026/02/game-manager-lutris-v0-5-20-released-with-proton-upgrades-store-updates-and-much-more/) |
| **Bottles**               | Python-based `libwine` library executes .exe/.msi/.bat files inside a wineprefix. Supports running arbitrary executables through context menu integration. One bottle can host multiple applications.  | [GitHub - bottlesdevs/Bottles](https://github.com/bottlesdevs/Bottles)                                                                        |
| **protontricks**          | `protontricks <APPID> wine <exe>` wraps environment setup and invokes wine directly. Desktop integration allows opening .exe files via file manager.                                                   | [GitHub - Matoking/protontricks](https://github.com/Matoking/protontricks)                                                                    |

**Key Pattern**: All launchers follow the same fundamental approach -- set `WINEPREFIX` (and `STEAM_COMPAT_DATA_PATH` for Proton), invoke the Proton/Wine binary with `run` verb, pass the target executable as an argument. CrossHook already implements this pattern.

**Confidence**: High -- Consistent pattern across all major Linux game managers.

---

## Constraints and Gotchas

### 1. Prefix Must Not Be In Use During Update

- **Impact**: If the game is running (or another Proton process holds `pfx.lock`), the update command will block indefinitely waiting for the lock.
- **Workaround**: Validate that no wineserver is running for the target prefix before starting the update. CrossHook can check for the `pfx.lock` file or attempt a non-blocking lock test. Alternatively, simply document that the game must be closed first and rely on the UI state (if a game is launched through CrossHook, the launch panel shows it as running).
- **Confidence**: High

### 2. Proton Version Mismatch May Trigger Prefix Upgrades

- **Impact**: If the user selects a different Proton version for the update than the one used to create the prefix, Proton will automatically upgrade the prefix. This is usually benign but can cause issues with tracked files, DLL versions, or registry entries.
- **Workaround**: Default to the Proton version stored in the profile. Show a warning if the user selects a different version. The update form should pre-populate the Proton path from the selected profile.
- **Confidence**: High -- Documented in Proton source (prefix version checking in `CompatData.setup_prefix()`).

### 3. Updater Working Directory Matters

- **Impact**: Some game updaters expect to be run from the game's installation directory, not from the updater's own directory. If the working directory is wrong, the updater may fail to find files to patch or create files in the wrong location.
- **Workaround**: Default the working directory to the updater executable's parent directory (matching `apply_working_directory()` behavior). Provide an optional override field in the UI so users can point it at the game's installation directory within the prefix if needed.
- **Confidence**: Medium -- This is a known issue in the WINE gaming community but varies by updater.

### 4. Updater May Require Specific DLL Overrides or WINE Components

- **Impact**: Some updaters (especially those using .NET, Visual C++ redistributables, or custom installers like InnoSetup/NSIS) may require specific WINE components or DLL overrides that are not present in the default prefix.
- **Workaround**: This is beyond the scope of the initial update-game feature. Users who encounter this can use protontricks separately to install prerequisites. Document this as a known limitation.
- **Confidence**: Medium -- Common enough to document but not frequent enough to block implementation.

### 5. Steam Compatibility Data Directory Structure

- **Impact**: Steam stores compatdata in the same library as the game. If the game is on an external drive, the compatdata is on that same external drive, not in the default Steam library.
- **Workaround**: CrossHook's profile already stores the prefix path explicitly (either `steam.compatdata_path` or `runtime.prefix_path`), so this is a non-issue -- the path is already resolved at profile creation time via auto-populate or manual configuration.
- **Confidence**: High

### 6. File Permission Issues With Proton Prefixes

- **Impact**: Files created by Proton inside the prefix may have restrictive permissions. Some updaters may fail if they cannot write to certain directories within the prefix.
- **Workaround**: Proton handles permission issues gracefully since version 10.0-105 with the `creation_sync_guard` mechanism. For older Proton versions, this is a known limitation. The update log should capture any permission errors from the Proton process.
- **Confidence**: Medium -- Observed in community reports but uncommon with modern Proton.

### 7. MSI Installers and Silent Flags

- **Impact**: Windows update executables come in various formats -- standalone `.exe` patchers, `.msi` packages, or self-extracting archives. MSI packages require `msiexec.exe` which is provided by WINE within the prefix. Some updaters support silent/unattended flags.
- **Workaround**: For the initial implementation, pass the executable directly to `proton run`. MSI files would need the user to provide the full command (or CrossHook could detect `.msi` extensions and prepend `msiexec.exe /i`). This is a potential enhancement for a later iteration.
- **Confidence**: Medium

---

## Code Examples

### Basic Update Command Construction (Rust)

This example shows how to build the update command using CrossHook's existing infrastructure. It mirrors the pattern in `install/service.rs::build_install_command()`:

```rust
use std::path::Path;
use tokio::process::Command;

use crate::launch::runtime_helpers::{
    apply_host_environment,
    apply_runtime_proton_environment,
    apply_working_directory,
    attach_log_stdio,
    new_direct_proton_command,
};

/// Builds a Proton command to run an update executable in an existing prefix.
///
/// Environment setup mirrors `build_install_command()` in install/service.rs
/// and `build_proton_game_command()` in launch/script_runner.rs.
fn build_update_command(
    proton_path: &str,
    updater_path: &str,
    prefix_path: &str,
    steam_client_install_path: &str,
    log_path: &Path,
) -> std::io::Result<Command> {
    let mut command = new_direct_proton_command(proton_path);
    command.arg(updater_path);
    apply_host_environment(&mut command);
    apply_runtime_proton_environment(&mut command, prefix_path, steam_client_install_path);
    apply_working_directory(&mut command, "", Path::new(updater_path));
    attach_log_stdio(&mut command, log_path)?;
    Ok(command)
}
```

### Manual Shell Equivalent

For reference, the shell command CrossHook will effectively produce:

```bash
# For a Steam-managed prefix (e.g., appid 12345):
env -i \
  HOME="$HOME" \
  USER="$USER" \
  PATH="/usr/bin:/bin" \
  DISPLAY="$DISPLAY" \
  WAYLAND_DISPLAY="$WAYLAND_DISPLAY" \
  XDG_RUNTIME_DIR="$XDG_RUNTIME_DIR" \
  DBUS_SESSION_BUS_ADDRESS="$DBUS_SESSION_BUS_ADDRESS" \
  STEAM_COMPAT_DATA_PATH="$HOME/.local/share/Steam/steamapps/compatdata/12345" \
  STEAM_COMPAT_CLIENT_INSTALL_PATH="$HOME/.local/share/Steam" \
  WINEPREFIX="$HOME/.local/share/Steam/steamapps/compatdata/12345/pfx" \
  "$HOME/.local/share/Steam/steamapps/common/Proton 10.0/proton" \
  run \
  /path/to/game-update-v1.2.exe

# For a standalone CrossHook prefix:
env -i \
  HOME="$HOME" \
  USER="$USER" \
  PATH="/usr/bin:/bin" \
  DISPLAY="$DISPLAY" \
  WAYLAND_DISPLAY="$WAYLAND_DISPLAY" \
  XDG_RUNTIME_DIR="$XDG_RUNTIME_DIR" \
  DBUS_SESSION_BUS_ADDRESS="$DBUS_SESSION_BUS_ADDRESS" \
  STEAM_COMPAT_DATA_PATH="$HOME/.local/share/crosshook/prefixes/my-game" \
  WINEPREFIX="$HOME/.local/share/crosshook/prefixes/my-game" \
  "$HOME/.local/share/Steam/steamapps/common/Proton 10.0/proton" \
  run \
  /path/to/game-update-v1.2.exe
```

### Request/Result Model Sketch

Following the pattern established by `InstallGameRequest`/`InstallGameResult`:

```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct UpdateGameRequest {
    /// Profile name -- used to resolve prefix and Proton paths from the profile.
    pub profile_name: String,
    /// Path to the update/patch executable (.exe).
    pub updater_path: String,
    /// Path to the Proton executable. Pre-populated from profile.
    pub proton_path: String,
    /// Path to the existing prefix. Pre-populated from profile.
    pub prefix_path: String,
    /// Steam client install path. Pre-populated from profile or auto-detected.
    pub steam_client_install_path: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct UpdateGameResult {
    pub succeeded: bool,
    pub message: String,
    pub helper_log_path: String,
}
```

---

## Proton Prefix Directory Reference

For completeness, the full layout of a Proton-managed prefix:

```
steamapps/compatdata/<appid>/
    pfx/                          # WINEPREFIX target
        drive_c/                  # C:\ drive
            windows/
                system32/         # 64-bit Windows DLLs
                syswow64/         # 32-bit Windows DLLs
            users/
                steamuser/        # User profile directory
            Program Files/        # 64-bit application installs
            Program Files (x86)/  # 32-bit application installs
            Games/                # Common game install location
        dosdevices/               # Drive letter symlinks
            c:  -> ../drive_c
            z:  -> /              # Maps Z: to Linux root
            s:  -> <game-library> # Optional: PROTON_SET_GAME_DRIVE
        system.reg                # HKEY_LOCAL_MACHINE registry hive
        user.reg                  # HKEY_CURRENT_USER registry hive
        userdef.reg               # Default user registry
    pfx.lock                      # File lock (prevents concurrent access)
    version                       # Prefix version string (e.g., "10.1000-105")
    tracked_files                 # List of Proton-managed files
    creation_sync_guard           # Durability marker (Proton 10.0-105+)
    config_info                   # Proton configuration snapshot
```

---

## Open Questions

1. **Should update-game support `steam_applaunch` profiles?** The existing install-game only supports `proton_run` (standalone prefix). Steam-managed profiles have a `compatdata_path` instead of a `runtime.prefix_path`. The update command needs to handle both, or the feature could be limited to `proton_run` profiles initially and extended later.

2. **Should the updater working directory default to the game's install directory rather than the updater's directory?** Some updaters expect to run from the game directory. The profile already stores `game.executable_path`, so CrossHook could derive the game directory from that. This could be an advanced option.

3. **Should CrossHook detect `.msi` files and automatically prepend `msiexec.exe /i`?** This would improve UX for MSI-based updates but adds complexity. Could be deferred to a follow-up enhancement.

4. **Should the UI allow selecting launch optimizations (mangohud, gamemode) for the update process?** These are unlikely to be useful for an updater, but the infrastructure exists. Recommendation: omit for simplicity.

5. **Should CrossHook verify the prefix is not in use before starting?** Checking for a running wineserver or testing `pfx.lock` would prevent the indefinite-hang scenario. This adds robustness but also complexity. A simpler approach: if the profile is currently shown as "launched" in CrossHook's UI, disable the update button.

---

## Search Queries Executed

1. `Proton WINE prefix management STEAM_COMPAT_DATA_PATH environment variables game update Linux`
2. `protontricks run executable Proton prefix command line API 2025`
3. `Lutris Heroic Bottles game update patch Proton prefix integration pattern Linux`
4. `Rust crate WINE prefix management process execution environment injection`
5. `Rust tokio Command process spawn environment variables WINEPREFIX stdout stderr streaming`
6. `Steam compatdata directory structure appmanifest ACF VDF file format specification`
7. `WINE Proton prefix environment variables complete list STEAM_COMPAT_INSTALL_PATH`
8. `umu-launcher GitHub API Rust integration run executable Proton prefix outside Steam 2025`
9. `WINE Proton run installer update executable prefix PROTON_VERB waitforexitandrun run runinprefix`
10. `Proton verbs run waitforexitandrun runinprefix createprefix getcompatpath list explained`
11. `Rust crate VDF parser Steam valve data format crates.io 2025`
12. `Heroic Games Launcher runWineCommand launcher.ts wine prefix environment setup source code`
13. `Bottles source code run executable wine prefix Python implementation GitHub game patching`
14. `Proton pfx.lock prefix locking mechanism concurrent access wineserver safety`
15. `Linux game patcher updater WINE prefix file locking considerations concurrent access registry changes`
16. `Proton source code proton script verbs ValveSoftware GitHub`

---

## Sources

- [ValveSoftware/Proton GitHub](https://github.com/ValveSoftware/Proton) -- Official Proton source and documentation
- [DeepWiki: Wine Prefix Management](https://deepwiki.com/ValveSoftware/Proton/2.2-wine-prefix-management) -- Detailed prefix lifecycle documentation
- [Proton FAQ](https://github.com/ValveSoftware/Proton/wiki/Proton-FAQ) -- Official FAQ with environment variable references
- [Run .exe in existing Proton prefix (gist)](https://gist.github.com/michaelbutler/f364276f4030c5f449252f2c4d960bd2) -- Community guide for running executables in prefixes
- [Matoking/protontricks](https://github.com/Matoking/protontricks) -- Protontricks wrapper for running commands in Proton prefixes
- [Open-Wine-Components/umu-launcher](https://github.com/Open-Wine-Components/umu-launcher) -- Unified launcher for Windows games on Linux
- [umu-launcher FAQ](<https://github.com/Open-Wine-Components/umu-launcher/wiki/Frequently-asked-questions-(FAQ)>) -- FAQ covering PROTON_VERB, prefix management
- [umu(1) man page](https://man.archlinux.org/man/umu.1.en) -- Complete umu-run CLI reference
- [tokio::process::Command](https://docs.rs/tokio/latest/tokio/process/struct.Command.html) -- Rust async process spawning
- [Heroic Games Launcher PR #1568](https://github.com/Heroic-Games-Launcher/HeroicGamesLauncher/pull/1568) -- runWineCommand refactoring
- [Heroic Games Launcher](https://github.com/Heroic-Games-Launcher) -- Open-source game launcher with Wine prefix management
- [bottlesdevs/Bottles](https://github.com/bottlesdevs/Bottles) -- Wine prefix manager with executable running support
- [bottlesdevs/libwine](https://github.com/bottlesdevs/libwine) -- Python library for Wine interaction
- [Lutris v0.5.20 release](https://www.gamingonlinux.com/2026/02/game-manager-lutris-v0-5-20-released-with-proton-upgrades-store-updates-and-much-more/) -- Lutris umu-launcher integration
- [Steam compatdata structure](https://deepwiki.com/akorb/SteamShutdown/4.1-steam-file-formats) -- Steam file format documentation
- [VDF crates on crates.io](https://crates.io/keywords/vdf) -- Rust VDF parser crate ecosystem
- [Steam for Linux Issue #5766](https://github.com/ValveSoftware/steam-for-linux/issues/5766) -- Request for "Run in prefix" feature in Steam
- [umu-wrapper crate](https://crates.io/crates/umu-wrapper) -- Rust wrapper for umu-launcher
- [tokio-process-tools](https://docs.rs/tokio-process-tools/latest/tokio_process_tools/) -- Advanced process output handling

---

## Uncertainties and Gaps

- **umu-launcher Rust bindings maturity**: The `umu-wrapper` crate exists but its stability and feature completeness are unclear. The crates.io page did not render content during research. This is not blocking since CrossHook invokes Proton directly.

- **Proton verb behavior differences**: The exact behavioral difference between `run` and `waitforexitandrun` when invoked directly (not via umu) is not fully documented in Proton's README. From source code analysis, `waitforexitandrun` calls `wineserver -w` after the process exits, while `run` does not. CrossHook currently uses `run` for all direct Proton invocations, which should be fine for updates since the `block_on(child.wait())` pattern already waits for the process to exit.

- **Prefix version compatibility**: When using a newer Proton version to run an update in a prefix created by an older version, Proton will silently upgrade the prefix. The impact on game compatibility is generally positive but not guaranteed. No programmatic way exists to check prefix version compatibility without parsing the `version` file and comparing against Proton's built-in version string.

- **MSI support scope**: The proportion of game updates distributed as `.msi` vs standalone `.exe` patchers is unknown. Community anecdotes suggest most game updaters are self-extracting `.exe` files, but `.msi` support may be needed for some titles.
