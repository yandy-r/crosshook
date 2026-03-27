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
- [Contributing](#contributing)

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

Launches the game through Steam using `steam -applaunch <appid>`, then runs the trainer against the same Proton prefix. By default the trainer is launched from its original directory so stateful bundles like Aurora keep one shared install, and profiles can opt into `Copy into prefix` when needed for compatibility. Use this when Steam must own the game launch (DRM, cloud saves, overlay).

### Proton Run

Runs the trainer directly using `proton run <trainer.exe>` against the game's compatdata prefix. Useful when you want to launch a trainer standalone without going through Steam, or when the

## Contributing

Contributions are welcome! If you'd like to improve CrossHook:
1. **Report Bugs:** Open an issue with your system specs and the game/trainer being used.
2. **Submit PRs:** Ensure your code is formatted with `cargo fmt`. Check out the [Build](#build) section for local setup.
3. **Docs:** Improvements to the documentation or new game-specific profiles are highly appreciated.