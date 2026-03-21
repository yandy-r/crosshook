# Steam / Proton Trainer Workflow

CrossHook's Steam mode is for cases where Steam must launch the game, but the trainer or mod should still run against the same Proton compatdata. This guide explains how the workflow works, what CrossHook generates, and where the current limits are.

If you are just getting started, read the [CrossHook quickstart](../getting-started/quickstart.md) first.

## Table of Contents

- [Overview](#overview)
- [Workflow in CrossHook](#workflow-in-crosshook)
- [Generated launchers](#generated-launchers)
- [Current limitations](#current-limitations)
- [Troubleshooting](#troubleshooting)
- [Related guides](#related-guides)

## Overview

Steam mode is useful when:

- Steam needs to own the game launch.
- The trainer must run inside the same compatdata as the game.
- You want a repeatable trainer launcher that can be exported for later use.

In this mode, CrossHook does not try to behave like a direct Windows launcher. Instead, it prepares the Steam-specific paths and uses Proton against the target game's compatdata.

## Workflow in CrossHook

1. Enable `Use Steam / Proton launch mode`.
2. Fill in the Steam fields:
   - `Game Path`
   - `Trainer Path`
   - `Steam App ID`
   - `Compatdata Path`
   - `Proton Path`
3. Launch the game from CrossHook.
4. Wait until the game reaches the in-game menu.
5. Launch the trainer from CrossHook.

CrossHook uses the Steam helper path for this workflow. Direct launch methods and in-app DLL injection do not apply in Steam mode.

## Generated Launchers

CrossHook can generate host-side launchers for Steam-mode trainers from the same configuration.

Use `Create Script + Desktop` in the Steam settings area to generate both files from the current profile.

The generated outputs are written to:

- `~/.local/share/crosshook/launchers/`
- `~/.local/share/applications/`

The filenames follow the current launcher name or trainer name:

- `*-trainer.sh`
- `crosshook-*-trainer.desktop`

The script uses the same known-good Proton pattern CrossHook uses internally:

```bash
export STEAM_COMPAT_DATA_PATH='...'
export STEAM_COMPAT_CLIENT_INSTALL_PATH='...'
export WINEPREFIX="$STEAM_COMPAT_DATA_PATH/pfx"
exec "$PROTON" run "$TRAINER_WINDOWS_PATH"
```

The desktop entry simply runs the script with `/bin/bash`, which makes it usable from desktop menus and launchers.

## Current Limitations

- Steam mode does not support CrossHook's in-app DLL injection yet.
- The game still has to be launched through Steam's Proton/Wine runtime.
- Exported launchers are built around a file-based trainer executable, not a directory-based trainer bundle.
- Export requires a trainer path, Steam App ID, compatdata path, and Proton path. CrossHook derives the launcher output location from the current Steam install path and host environment.
- CrossHook is still dependent on the target game's Proton/Wine behavior, so a trainer that fails inside that runtime may need a different compatibility setup.

## Troubleshooting

- If export fails immediately, check the required fields first. Missing trainer path, Steam App ID, compatdata path, or Proton path will stop the export.
- If the game starts but the trainer does not, wait until the game is at the in-game menu before launching the trainer.
- If CrossHook warns about DLL options, clear those fields and use Steam mode for the game-plus-trainer workflow only.
- If the generated launcher lands in an unexpected location, verify the Steam client install path that CrossHook is detecting and whether your Steam install follows a standard layout.
- CrossHook normally derives the launcher output location automatically. If your setup is unusual, verify the detected paths before exporting again.
- If a trainer still fails after export, test the same trainer under the same Proton path outside CrossHook to confirm the runtime itself is viable.

## Related Guides

- [CrossHook quickstart](../getting-started/quickstart.md)
