# CrossHook Native -- Complete Rewrite

[![Download CrossHook](https://img.shields.io/badge/Download-CrossHook_Native-00C853?style=for-the-badge)](https://github.com/yandy-r/crosshook/releases)
[![GitHub Release](https://img.shields.io/github/v/release/yandy-r/crosshook?style=for-the-badge&color=blue&label=Latest)](https://github.com/yandy-r/crosshook/releases)
[![Platforms](https://img.shields.io/badge/Platforms-Linux%20|%20Steam%20Deck-blue?style=for-the-badge&logo=linux)](https://github.com/yandy-r/crosshook)
[![License](https://img.shields.io/github/license/yandy-r/crosshook?style=for-the-badge&color=green)](LICENSE)

CrossHook Native is a ground-up rewrite. The original WinForms-based CrossHook Loader (a Windows binary running under Proton/WINE) has been replaced with a native Linux desktop application built on Rust, Tauri v2, and React/TypeScript. CrossHook itself no longer needs WINE or Proton to run -- only your games and trainers do.

## What Changed

### Native Linux Application

- CrossHook is now a **native Linux binary** distributed as an **AppImage**. No WINE or Proton required for CrossHook itself.
- Built with **Rust** (backend logic), **Tauri v2** (desktop shell), and **React 18 + TypeScript** (frontend UI).
- Runs natively on Linux desktops and Steam Deck without compatibility layers.

### Three Launch Modes

- **Steam App Launch**: Launches games via `steam://run` with trainers configured as launch options. Best for Steam Deck and games that require Steam integration.
- **Proton Run**: Runs games directly through a selected Proton version using `proton run`. Supports standalone prefixes for isolation.
- **Native**: Executes binaries directly without Proton. Useful for native Linux games or non-WINE tools.

### Steam Auto-Populate

- Automatically discovers installed Steam games, library folders, and Proton versions.
- Populates the profile editor with game executables, app IDs, and available Proton runners.
- Supports multiple Steam library paths and custom install locations.

### Community Profile Sharing

- Share and discover game profiles through git-based **taps** (community repositories).
- Browse, search, and import profiles from community taps.
- Compatibility viewer shows which profiles work with which Proton versions and platforms.

### Launcher Export

- Export launch configurations as standalone **shell scripts** and **.desktop entries**.
- Generated launchers can be used independently of CrossHook.
- Integrates with Steam as non-Steam game shortcuts.

### TOML-Based Profiles and Settings

- Profiles and application settings use human-readable **TOML** format.
- Profiles store game paths, trainer paths, launch mode, Proton version, environment variables, and pre/post-launch commands.

### UI and Experience

- **Dark theme** optimized for Steam Deck's display.
- **Gamepad/controller navigation** support for couch and handheld use.
- **Console view** for real-time runner output, launch logs, and process status.
- Tab-based navigation: Profile Editor, Launch, Community, Export, Settings.

### Developer Experience

- Rust workspace with two crates: `crosshook-core` (shared library) and `crosshook-cli` (standalone CLI).
- Tauri IPC command layer bridges Rust backend to React frontend.
- Vite-powered dev server with hot reload for frontend development.
- Container-based build option for reproducible AppImage builds.

## Removed

- Windows Forms (WinForms) UI
- C# / .NET runtime dependency
- Win32 P/Invoke and DLL injection system
- Windows-only `crosshook.exe` binary
- Dual `win-x64` / `win-x86` artifact builds

## Installation

Download the AppImage from the [GitHub Releases page](https://github.com/yandy-r/crosshook/releases).

### Linux Desktop

1. Download the AppImage.
2. Make it executable: `chmod +x CrossHook-*.AppImage`
3. Run it directly or integrate with your desktop environment.

### Steam Deck

1. Download the AppImage to the Desktop or a known location.
2. Add it as a Non-Steam Game in Steam.
3. Launch from Gaming Mode -- no Proton compatibility tool needed (CrossHook is native).

See `docs/getting-started/quickstart.md` for detailed setup instructions.

## Support

Found a bug or compatibility issue? File a report:
[![Report Issue](https://img.shields.io/badge/Report%20a%20Bug-GitHub%20Issues-red?style=for-the-badge)](https://github.com/yandy-r/crosshook/issues)

## License

CrossHook is open-source software under the MIT License.
