# CrossHook

[![Download CrossHook](https://img.shields.io/badge/⬇_Download-CrossHook_v0.1.0-00C853?style=for-the-badge)](https://github.com/yandy-r/crosshook/releases)
[![GitHub Release](https://img.shields.io/github/v/release/yandy-r/crosshook?style=for-the-badge&color=blue&label=Latest)](https://github.com/yandy-r/crosshook/releases)
[![License](https://img.shields.io/github/license/yandy-r/crosshook?style=for-the-badge&color=green)](LICENSE)

CrossHook is a Proton/WINE trainer and DLL loader for Windows games run through Steam, Linux, Steam Deck, and macOS tooling such as Whisky. It launches games alongside trainers, patches, mods, and extra executables when Windows-only launch flows need help in Proton/WINE environments.

## Contents

- [Download](#download)
- [Quick Start](#quick-start)
- [Features](#features)
- [Docs](#docs)
- [Build](#build)
- [Release Notes](#release-notes)

## Download

Get the latest release from [GitHub Releases](https://github.com/yandy-r/crosshook/releases).

Release assets are published as two zip files:

- `crosshook-win-x64.zip`
- `crosshook-win-x86.zip`

Extract the full zip into a folder you want to keep, then launch `crosshook.exe` from the extracted directory. Do not run it from inside the zip or move only the EXE by itself.

## Quick Start

For the full setup flow, use the dedicated guide:

- [Quickstart guide](docs/getting-started/quickstart.md)

For Steam / Proton launch mode, compatdata, and external launcher export, see:

- [Steam / Proton trainer workflow](docs/features/steam-proton-trainer-launch.doc.md)

## Features

- Launch a game and trainer together.
- Support extra EXEs, patches, and DLL-based workflows.
- Save reusable profiles and launch configurations.
- Use Steam / Proton mode when the game must be launched through Steam.
- Export external launchers for Steam-mode trainer workflows.

## Docs

- [Steam / Proton trainer workflow](docs/features/steam-proton-trainer-launch.doc.md)
- [Quickstart guide](docs/getting-started/quickstart.md)
- [Documentation strategy](docs/plans/documentation-strategy.md)
- [Local build and publish notes](docs/internal-docs/local-build-publish.md)

## Build

The project targets `net9.0-windows`.

```bash
dotnet build src/CrossHookEngine.sln -c Release
./scripts/publish-dist.sh
```

If the repo-local SDK is present, use it before running `dotnet` commands:

```bash
export PATH="$PWD/.dotnet:$PATH"
export DOTNET_CLI_HOME="$PWD/.dotnet-cli-home"
```

## Release Notes

- Releases publish both `win-x64` and `win-x86` artifacts.
- The app is a directory-based self-contained publish, not a single-file EXE.
- `crosshook.exe` must stay beside the rest of the extracted publish output.
- User state such as `Profiles/`, `Settings/`, and `settings.ini` is not part of the release bundle.
