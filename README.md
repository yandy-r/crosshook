# CrossHook

[![Download CrossHook](https://img.shields.io/badge/Download-CrossHook_AppImage-00C853?style=for-the-badge)](https://github.com/yandy-r/crosshook/releases)
[![GitHub Release](https://img.shields.io/github/v/release/yandy-r/crosshook?style=for-the-badge&color=blue&label=Latest)](https://github.com/yandy-r/crosshook/releases)
[![License](https://img.shields.io/github/license/yandy-r/crosshook?style=for-the-badge&color=green)](LICENSE)
[![Platform](https://img.shields.io/badge/Platform-Linux%20%7C%20Steam%20Deck-blue?style=for-the-badge&logo=linux)](https://github.com/yandy-r/crosshook)

CrossHook is a native Linux trainer launcher for Steam and Proton games. It runs directly on your Linux desktop or Steam Deck -- no WINE needed for CrossHook itself -- and orchestrates trainers, mods, and patches that run under the game's own Proton/WINE prefix.

CrossHook also includes an `Install Game` flow inside the Profile panel. It installs Windows games through direct `proton run`, defaults new prefixes under `~/.local/share/crosshook/prefixes/<slug>`, and then returns you to the normal profile editor so you can review the generated profile before saving it.

## Contents

- [Download](#download)
- [Quick Start](#quick-start)
- [Launch Modes](#launch-modes)
- [Features](#features)
- [Docs](#docs)
- [Build](#build)
- [Release Notes](#release-notes)

## Download

Get the latest AppImage from [GitHub Releases](https://github.com/yandy-r/crosshook/releases).

```bash
# Download the AppImage (adjust the version as needed)
chmod +x CrossHook_*.AppImage
./CrossHook_*.AppImage
```

No installation required. The AppImage is a single portable file that runs on any modern Linux distribution.

## Quick Start

1. Download the AppImage from the [Releases page](https://github.com/yandy-r/crosshook/releases).
2. Make it executable: `chmod +x CrossHook_*.AppImage`
3. Launch it: `./CrossHook_*.AppImage`
4. Select a game from the auto-populated Steam library.
5. Choose a trainer or mod, pick a launch mode, and hit Launch.

For the full setup walkthrough, see the [Quickstart guide](docs/getting-started/quickstart.md).

For Steam/Proton-specific workflow details, see the [Steam / Proton trainer workflow](docs/features/steam-proton-trainer-launch.doc.md).

## Launch Modes

CrossHook supports three launch modes depending on how your game and trainer need to run.

### Steam App Launch

Launches the game through Steam using `steam -applaunch <appid>`, then runs the trainer under the game's Proton prefix. Use this when Steam must own the game launch (DRM, cloud saves, overlay).

### Proton Run

Runs the trainer directly using `proton run <trainer.exe>` against the game's compatdata prefix. Useful when you want to launch a trainer standalone without going through Steam, or when the game is already running.

The same direct Proton path is used by the `Install Game` workflow in the Profile panel. That flow writes the prefix under `~/.local/share/crosshook/prefixes/<slug>` and hands you back a normal `GameProfile` for review and save.

Profiles that use `proton_run` also show a `Launch Optimizations` panel in the right column. It uses a curated v1 option catalog, shows an info tooltip on every visible option, and autosaves checkbox changes only after the profile already exists on disk.

### Native

For trainers or tools that run natively on Linux without WINE/Proton. CrossHook launches them as regular Linux processes alongside the game.

## Features

- **Steam Library Auto-Populate** -- Discovers installed Steam games, their App IDs, Proton versions, and compatdata paths automatically.
- **Profile Management** -- Save and load launch configurations per game. Switch between trainer setups instantly.
- **Install Game Workflow** -- Install a Windows game from the Profile panel, review the generated profile, and save it explicitly after install.
- **Launch Optimizations** -- Adjust curated `proton_run` tweaks with readable labels, per-option info tooltips, and autosave for already-saved profiles.
- **Launcher Export** -- Generate standalone shell scripts and `.desktop` entries from any profile for one-click launching without opening CrossHook.
- **Community Profile Sharing** -- Share and import launch profiles with other users.
- **Proton Selector** -- Choose which Proton version to use for each trainer, with auto-detection of installed versions.
- **Gamepad Navigation** -- Full controller and touchscreen support for Steam Deck Gaming Mode.
- **Console Log Viewer** -- See exactly what commands CrossHook executes, with real-time process output for debugging.
- **Dark Theme** -- Native dark UI that fits in on Steam Deck and Linux desktops.

## Docs

- [Quickstart guide](docs/getting-started/quickstart.md)
- [Steam / Proton trainer workflow](docs/features/steam-proton-trainer-launch.doc.md)

## Build

CrossHook is built with [Tauri v2](https://v2.tauri.app/) (Rust backend + React/TypeScript frontend). Building from source requires Rust, Node.js, and system libraries for WebKitGTK.

### Prerequisites

Install build dependencies automatically using the included script:

```bash
# Supports pacman, apt, dnf, and zypper
./scripts/install-native-build-deps.sh
```

Or install manually. You need: `cargo`, `npm`, `patchelf`, and development libraries for GTK3, libsoup3, WebKitGTK 4.1, and OpenSSL.

### Build the AppImage

```bash
./scripts/build-native.sh
```

The AppImage is written to `dist/`. Additional options:

```bash
# Build the release binary only (skip AppImage bundling)
./scripts/build-native.sh --binary-only

# Install dependencies first, then build
./scripts/build-native.sh --install-deps --yes
```

### Development

```bash
./scripts/dev-native.sh
```

This starts the Tauri dev server with hot-reload for the React frontend and Rust backend.

### CI

The [release](.github/workflows/release.yml) GitHub Actions workflow builds and uploads the AppImage to a GitHub Release on every version tag push (`v*`).

## Release Notes

- Releases publish a single **AppImage** artifact for x86_64 Linux.
- The AppImage is self-contained and portable -- no system-level installation needed.
- User state (profiles, settings) is stored in `~/.config/crosshook/` or the XDG config directory, separate from the application binary.
- Install prefixes default under `~/.local/share/crosshook/prefixes/<slug>` and are only saved into a profile after review in the Profile panel.
- macOS support is planned for a future release.

## License

CrossHook is open-source software under the [MIT License](LICENSE).
