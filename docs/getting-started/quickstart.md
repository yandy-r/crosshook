# CrossHook Quickstart

CrossHook is a Windows app that you run under Proton or WINE on supported hosts. This quickstart is written for end users who want the shortest path from download to a working game-plus-trainer setup.

If you want the deeper Steam-specific workflow details, jump to the [Steam / Proton feature guide](../features/steam-proton-trainer-launch.doc.md).

## Table of Contents

- [Supported environments](#supported-environments)
- [What you need](#what-you-need)
- [Install CrossHook](#install-crosshook)
- [Linux / Steam Deck](#linux--steam-deck)
- [macOS / Whisky](#macos--whisky)
- [External launcher export](#external-launcher-export)
- [Troubleshooting](#troubleshooting)
- [Related guides](#related-guides)

## Supported Environments

| Environment | How CrossHook runs | Best use case |
| --- | --- | --- |
| Linux / Steam Deck | Add `crosshook.exe` to Steam as a non-Steam game and force Proton | SteamOS and Linux desktop users running games through Steam |
| macOS / Whisky | Open `crosshook.exe` inside a Whisky bottle | macOS users who already manage Windows apps through Whisky |
| External launcher export | Generate a shell script and desktop entry from Steam mode | Users who want a reusable launcher outside the CrossHook window |

## What You Need

- The CrossHook release zip extracted into a normal folder. Do not run it from inside the zip.
- A game executable and the trainer or mod executable you want to launch with it.
- A Proton or WINE runtime that can run both CrossHook and your trainer.
- For Steam mode, the Steam App ID, compatdata path, Proton path, and Steam client install path for the target game.
- For launcher export, a Steam install path that CrossHook can use to derive your host-side launcher directory.

## Install CrossHook

1. Download the latest release zip from the [GitHub Releases page](https://github.com/yandy-r/crosshook/releases).
2. Extract the full zip into a folder you want to keep, such as `~/Applications/CrossHook`.
3. Launch `crosshook.exe` from the extracted folder in the environment you are using.
4. Keep the whole extracted folder together. CrossHook is not a single-file executable.

## Linux / Steam Deck

1. Switch your Steam Deck to Desktop Mode if you are on SteamOS.
2. Add the extracted `crosshook.exe` to Steam as a non-Steam game.
3. Open the shortcut properties and force Proton for CrossHook.
4. Launch CrossHook from Steam.
5. Pick your game path, trainer path, and any optional extra executables or DLLs.
6. Save a profile if you want to reuse the same setup later.
7. Launch the game, then start the trainer once the game has reached the in-game menu.

If you need a Steam-mode-specific explanation of the launch flow, read [Steam / Proton trainer workflow](../features/steam-proton-trainer-launch.doc.md).

## macOS / Whisky

1. Install Whisky.
2. Create a new bottle for CrossHook.
3. Use Whisky to run the extracted `crosshook.exe`.
4. Configure the game path, trainer path, and any extra files you want CrossHook to manage.
5. Launch the game from CrossHook, wait for the menu, then launch the trainer.
6. Save the profile when the setup works so you can reopen it later.

If a trainer requires Steam-mode handling instead of a direct CrossHook launch, use the [Steam / Proton feature guide](../features/steam-proton-trainer-launch.doc.md) to decide whether export is a better fit.

## External Launcher Export

Use external launcher export when you want CrossHook to build a reusable launcher on the host system instead of only launching from inside the app.

1. Open CrossHook with Steam mode configured for the target game.
2. Fill in the required Steam fields:
   - Trainer path
   - Steam App ID
   - Compatdata path
   - Proton path
3. Choose a launcher name if you want a friendlier menu entry than the default trainer filename.
4. Click `Create Script + Desktop`.
5. CrossHook writes the generated files to:
   - `~/.local/share/crosshook/launchers/`
   - `~/.local/share/applications/`
6. Start the game in Steam first.
7. Wait until the game reaches the in-game menu.
8. Run the generated desktop launcher or the script that CrossHook created.

The generated script uses Proton directly against the target compatdata, so it is meant for the same game session that Steam already started. For the full workflow and limitations, see the [Steam / Proton feature guide](../features/steam-proton-trainer-launch.doc.md).

## Troubleshooting

- If CrossHook refuses to export launchers, check that every required Steam field is filled in.
- If the trainer opens too early, wait until the game has reached the in-game menu before launching it.
- If Steam mode complains about DLL options, clear the DLL fields and use Steam mode for the game-plus-trainer flow only.
- If the export writes to the wrong place, verify the Steam client install path CrossHook is using and whether your Steam install follows a standard layout.
- If your Steam install uses a standard layout, CrossHook can usually derive the launcher output directory automatically.
- If a trainer still fails under your chosen runtime, verify that the Proton or WINE version you selected can run that trainer outside CrossHook first.

## Related Guides

- [Steam / Proton trainer workflow](../features/steam-proton-trainer-launch.doc.md)
- [README](../../README.md)
