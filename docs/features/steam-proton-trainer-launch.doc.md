# Steam / Proton Trainer Workflow

## Overview

CrossHook includes a Steam mode for games that run through Proton. Steam mode is intended for profiles where the game must be launched by Steam and the trainer must target the game's Steam compatdata.

## Current Workflow

1. Enable `Use Steam / Proton launch mode`.
2. Set:
   - `Steam App ID`
   - `Compatdata Path`
   - `Proton Path`
   - `Trainer Path`
3. Press `Launch Game`.
4. Wait until the game reaches its in-game menu.
5. Press `Launch Trainer`.

## External Launcher Export

CrossHook can also generate external launchers for Steam-mode trainers:

- Shell script output: `~/.local/share/crosshook/launchers/`
- Desktop entry output: `~/.local/share/applications/`

Use `Create Script + Desktop` in the Steam settings area to generate both files from the current profile or Steam configuration.

The exported script uses the known-good host-side command pattern:

```bash
export STEAM_COMPAT_DATA_PATH=...
export STEAM_COMPAT_CLIENT_INSTALL_PATH=...
export WINEPREFIX="$STEAM_COMPAT_DATA_PATH/pfx"
"$PROTON" run "D:\\path\\to\\trainer.exe"
```

## Key Files

- `src/CrossHookEngine.App/Forms/MainForm.cs`
- `src/CrossHookEngine.App/Services/SteamLaunchService.cs`
- `src/CrossHookEngine.App/Services/SteamExternalLauncherExportService.cs`
- `src/CrossHookEngine.App/runtime-helpers/steam-launch-helper.sh`
- `src/CrossHookEngine.App/runtime-helpers/steam-launch-trainer.sh`
- `src/CrossHookEngine.App/runtime-helpers/steam-host-trainer-runner.sh`

## Limitations

- Steam mode does not support CrossHook's in-app DLL injection.
- Steam trainer launch behavior still depends on the target game's Proton/Wine runtime characteristics.
- Directory-based trainer bundles are not yet handled by the external launcher export path; the current flow is built around file-based trainer executables.
