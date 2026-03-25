# Steam / Proton Trainer Workflow

CrossHook is a native Linux application that orchestrates game and trainer launches through Steam and Proton. CrossHook itself runs directly on the host -- it does not run under WINE. Trainers are Windows executables that CrossHook launches into the game's Proton compatdata prefix using a clean environment.

CrossHook also includes an `Install Game` sub-tab inside the Profile panel. That flow uses direct `proton run`, defaults new prefixes under `~/.local/share/crosshook/prefixes/<slug>`, and opens a review modal for the generated `GameProfile` before you save it. Saving the draft persists the profile and then opens the Profile tab with that new profile selected.

This guide covers the three launch methods, auto-discovery, launcher export, and the console view. If you are just getting started, read the [CrossHook quickstart](../getting-started/quickstart.md) first.

## Table of Contents

- [Overview](#overview)
- [Launch methods](#launch-methods)
- [Auto-populate and Steam discovery](#auto-populate-and-steam-discovery)
- [Workflow in CrossHook](#workflow-in-crosshook)
- [Launcher export](#launcher-export)
- [Launcher lifecycle management](#launcher-lifecycle-management)
- [Console view](#console-view)
- [Current limitations](#current-limitations)
- [Troubleshooting](#troubleshooting)
- [Related guides](#related-guides)

## Overview

CrossHook supports three launch methods:

| Method | Key | When to use |
| --- | --- | --- |
| Steam App Launch | `steam_applaunch` | Games installed through Steam where Steam must own the game launch (DRM, overlay, Proton runtime) |
| Proton Run | `proton_run` | Games or trainers that should run directly through Proton against a specific prefix, without going through the Steam client |
| Native | `native` | Linux-native executables that do not need Proton or WINE |

All three methods are set per-profile in the `[launch]` section:

```toml
[launch]
method = "steam_applaunch"
```

## Launch Methods

### Steam App Launch (`steam_applaunch`)

This is the primary mode for Steam-managed games. The workflow has two phases:

1. **Game launch**: CrossHook runs `steam -applaunch <appid>` to start the game. Steam initializes DRM, the Steam overlay, and the correct Proton runtime.
2. **Trainer launch**: Once the game process is detected, CrossHook stages the trainer into the game's compatdata prefix at `pfx/drive_c/CrossHook/StagedTrainers/`, strips all inherited WINE/Proton environment variables, and runs the trainer through Proton with a clean environment.

Required profile fields:

```toml
[steam]
enabled = true
app_id = "1245620"
compatdata_path = "/mnt/games/SteamLibrary/steamapps/compatdata/1245620"
proton_path = "/mnt/games/SteamLibrary/steamapps/common/Proton 9.0-4/proton"
```

CrossHook auto-populates all of these fields when it discovers your Steam libraries.

### Proton Run (`proton_run`)

This mode launches both the game and trainer directly through Proton against a specified prefix, bypassing the Steam client entirely.

- Useful for non-Steam games using a standalone Proton/WINE prefix.
- Also useful when you need full control over the prefix path and Proton version.
- The two-step launch flow still applies: launch the game first, then launch the trainer.
- CrossHook uses the same direct `proton run` path for `Install Game`, then opens a review modal for explicit review and save. Saving that draft takes you to the Profile tab with the saved profile selected.
- The Profile editor also shows a `Launch Optimizations` panel for `proton_run` profiles. It is not part of Steam App Launch or Native mode.
- Each visible optimization has an info tooltip that explains what it does, when it helps, and its main caveat.
- Existing saved profiles autosave checkbox changes for this section only. New unsaved profiles stay in draft mode until the first manual save.
- The v1 option catalog is intentionally conservative. Common launch fixes are shown first, while advanced or community-documented options are grouped separately and may be hardware-specific or experimental.

Required profile fields:

```toml
[runtime]
prefix_path = "/home/user/.wine-prefixes/mygame"
proton_path = "/home/user/.steam/root/compatibilitytools.d/GE-Proton9-18/proton"
```

## Launch Optimizations

`Launch Optimizations` is a `proton_run`-only panel in the right column of the Profile editor.

- It presents curated toggles instead of raw env-var editing.
- It keeps the option labels readable and grouped by area such as input, performance, display, graphics, and compatibility.
- Every visible option has an info icon that opens help text in place.
- Saved profiles autosave this section after a short debounce; new profiles show a save-first warning until they are written once.
- Advanced and community-documented entries are still available, but they are visually separated so the common options stay obvious.

### Native (`native`)

This mode launches a Linux-native executable directly on the host system without Proton or WINE.

- Does not support the two-step trainer workflow. Trainers are Windows executables and require Proton.
- Rejects `.exe` files -- only Linux-native binaries are accepted.
- Useful for running native games or tools alongside CrossHook's profile management.

## Auto-Populate and Steam Discovery

CrossHook automatically discovers your Steam installation and populates profile fields. This eliminates the need to manually locate compatdata paths, Proton versions, or App IDs.

### What gets discovered

- **Steam library folders**: Parsed from `steamapps/libraryfolders.vdf`, including libraries on secondary drives.
- **Installed games**: Matched from `steamapps/appmanifest_*.acf` files using the `AppState.appid` and `AppState.installdir` fields.
- **Compatdata paths**: Derived as `<library>/steamapps/compatdata/<appid>` for each matched game.
- **Proton versions**: Resolved from Steam's `config.vdf` and `localconfig.vdf` compat tool mappings. CrossHook finds both official Proton installs and custom versions (GE-Proton, TKG) in `compatibilitytools.d/`.

### Discovery search paths

CrossHook checks these locations in order:

1. `~/.steam/root` (symlink to the active Steam install)
2. `~/.local/share/Steam` (default install path)
3. `~/.var/app/com.valvesoftware.Steam/data/Steam` (Flatpak Steam)

Custom Proton versions are searched in:

- `~/.steam/root/compatibilitytools.d/`
- `/usr/share/steam/compatibilitytools.d/`
- `/usr/local/share/steam/compatibilitytools.d/`

### How auto-populate works

1. CrossHook scans all discovered Steam libraries for an `appmanifest` that matches the selected game path.
2. If a match is found, it populates the Steam App ID and derives the compatdata path.
3. It then resolves the Proton version configured for that App ID (or the default Proton version if none is configured per-game).
4. If the match is ambiguous (multiple manifests matched), CrossHook reports the ambiguity and does not guess.

## Workflow in CrossHook

### Full game-plus-trainer launch (Steam App Launch)

1. Select or load a profile with `steam_applaunch` as the launch method.
2. Verify that the Steam fields are populated (App ID, compatdata path, Proton path). Use auto-populate if they are not.
3. Click the launch button. CrossHook starts the game through Steam.
4. Wait until the game reaches the in-game menu. CrossHook polls for the game process and transitions to the "Launch Trainer" state once the game is detected.
5. Click the launch button again. CrossHook:
   - Stages the trainer `.exe` into the compatdata prefix.
   - Strips all WINE/Proton environment variables (~30 variables) to prevent conflicts.
   - Sets only `STEAM_COMPAT_DATA_PATH`, `STEAM_COMPAT_CLIENT_INSTALL_PATH`, and `WINEPREFIX`.
   - Runs `proton run <trainer>` in a clean session via `setsid`.
6. The trainer starts inside the game's WINE prefix and can modify the running game.

### Trainer-only launch

If the game is already running (you started it from Steam directly), you can skip the game launch step:

1. Load a profile and use the trainer-only launch option.
2. CrossHook stages and launches the trainer against the configured compatdata prefix without touching the game process.

### Native launch

1. Select or load a profile with `native` as the launch method.
2. Click the launch button. CrossHook runs the Linux-native executable directly.
3. There is no trainer step in native mode.

## Launcher Export

CrossHook generates standalone shell scripts and `.desktop` entries from your profiles so you can launch trainers without opening the full application.

### Generating launchers

1. Open a saved profile with Steam-mode fields configured.
2. Use the export function. Optionally set a custom display name for the menu entry.
3. CrossHook validates the required fields (trainer path, App ID, compatdata path, Proton path) and generates the files.

### Output locations

| File type | Path | Naming pattern |
| --- | --- | --- |
| Shell script | `~/.local/share/crosshook/launchers/` | `<slug>-trainer.sh` |
| Desktop entry | `~/.local/share/applications/` | `crosshook-<slug>-trainer.desktop` |

### Generated script structure

The shell script sets the Proton environment and runs the trainer directly:

```bash
export STEAM_COMPAT_DATA_PATH='...'
export STEAM_COMPAT_CLIENT_INSTALL_PATH='...'
export WINEPREFIX="$STEAM_COMPAT_DATA_PATH/pfx"
exec "$PROTON" run "$TRAINER_WINDOWS_PATH"
```

The `.desktop` entry runs the script with `/bin/bash`, making the trainer launchable from your desktop's application menu or a file manager.

### Using exported launchers

1. Start the game through Steam first and wait for the in-game menu.
2. Run the exported trainer launcher from your desktop menu or the terminal.
3. The launcher uses the same Proton environment and compatdata prefix as CrossHook itself.

## Launcher Lifecycle Management

CrossHook tracks the state of exported launcher files and provides tools for deleting, renaming, and cleaning up launchers as your profiles change. This prevents stale desktop entries from accumulating and keeps your application menu in sync with your actual profiles.

### Launcher status

After you export a launcher, CrossHook monitors the generated files and reports one of four statuses:

| Status | Meaning |
| --- | --- |
| **Exported** | Both the `.sh` script and `.desktop` entry exist and match the current profile |
| **Not Exported** | Neither file exists yet -- the profile has not been exported |
| **Partial** | Only one of the two files exists (the other was deleted externally or failed to write) |
| **Stale** | The files exist but their content no longer matches the current profile (e.g., the display name changed) |

The status badge appears in the Launcher Export panel. When a launcher is stale, CrossHook displays a notification with a **Re-export Launcher** button so you can regenerate the files with the updated profile data.

### Stale detection

CrossHook detects staleness by reading the `Name=` field in the `.desktop` entry and comparing it against the expected display name derived from the profile. If the profile's display name has changed since the last export, the launcher is marked stale.

Generated `.desktop` entries also include two metadata fields that CrossHook uses internally:

```ini
X-CrossHook-Profile=Elden Ring
X-CrossHook-Slug=elden-ring
```

These fields link the desktop entry back to its source profile and slug.

### Deleting launchers

You can delete exported launcher files from the Launcher Export panel. The delete button uses a click-again confirmation pattern -- click once to arm, click again within 3 seconds to confirm. Clicking away or waiting cancels the confirmation.

CrossHook performs two safety checks before deleting any file:

1. **Regular file check**: The file must be a regular file, not a symlink or directory. Symlinks are refused to prevent accidental deletion of targets outside the launchers directory.
2. **Watermark verification**: The file must contain a CrossHook watermark (`# Generated by CrossHook` for scripts, `Generated by CrossHook` for desktop entries). Files that do not contain the watermark are skipped and the skip reason is reported.

If a file passes both checks, it is removed. If a file fails either check, it is skipped and the reason is included in the result. The other file is still processed independently -- a failed script check does not block desktop entry deletion.

### Cascade on profile delete

When you delete a profile, CrossHook automatically attempts to clean up its exported launcher files. This is a best-effort operation: if the launcher cleanup fails (file permissions, missing files, etc.), the profile deletion still proceeds. A warning is logged but the user is not blocked.

The cascade only applies to non-native profiles. Native profiles do not generate launcher files.

### Renaming launchers

When you rename a profile or change its display name, CrossHook can update the corresponding launcher files. The rename uses a **write-then-delete** strategy rather than a filesystem rename, because both the `.sh` script and `.desktop` entry embed display names and paths as plaintext content that must be regenerated.

The rename process:

1. Generate new file content with the updated display name and paths.
2. Write the new files to their new paths (derived from the new slug).
3. If the slug changed (i.e., the new display name produces a different slug), delete the old files.
4. If the slug is unchanged, the new content overwrites the existing files in place.

If neither the old script nor the old desktop entry exists, the rename returns immediately without creating new files.

### Listing all launchers

CrossHook can scan the launchers directory (`~/.local/share/crosshook/launchers/`) for all exported trainer scripts. For each file matching the `*-trainer.sh` naming pattern, it derives the slug, checks for a matching `.desktop` entry in `~/.local/share/applications/`, and extracts the display name from the desktop entry's `Name=` field.

This listing is used by the orphan detection feature described below.

### Orphan detection

An orphaned launcher is a launcher file on disk that does not correspond to any known profile. Orphans can accumulate when profiles are deleted without cascade cleanup, when profile files are removed manually, or when a profile's slug changes after export.

CrossHook identifies orphans by comparing the slugs of all discovered launcher files against the slugs of all saved profiles. Any launcher whose slug does not match a known profile slug is flagged as orphaned.

You can review and clean up orphaned launchers from the Settings panel.

### Summary of lifecycle operations

| Operation | Trigger | What happens |
| --- | --- | --- |
| Status check | Opening the Launcher Export panel | Checks if `.sh` and `.desktop` files exist and whether they are stale |
| Export | Clicking "Export Launcher" | Generates new `.sh` and `.desktop` files from the current profile |
| Re-export | Clicking "Re-export Launcher" on a stale notification | Overwrites existing files with updated content |
| Delete | Clicking "Delete Launcher" (with confirmation) | Removes `.sh` and `.desktop` files after watermark verification |
| Profile delete cascade | Deleting a profile | Best-effort removal of the profile's launcher files |
| Rename | Changing a profile's display name | Write-then-delete regeneration of launcher files under the new slug |
| List | Settings panel or internal scan | Enumerates all CrossHook launcher scripts on disk |
| Orphan detection | Settings panel | Identifies launchers that no longer match any saved profile |

## Console View

CrossHook includes a console panel that streams runner output in real-time. When you launch a game or trainer, the console shows:

- The exact commands being executed (Proton paths, environment variables, script arguments).
- Stdout and stderr from the runner scripts.
- Process detection messages (game found, trainer staged, trainer started).
- Error messages with actionable context if something fails.

The console view is the first place to look when a launch does not behave as expected. All output is also written to log files in `~/.local/share/crosshook/logs/`.

## Current Limitations

- **Game must be launched through Steam first (in `steam_applaunch` mode).** CrossHook does not bypass Steam's DRM or overlay initialization. The game must be started via `steam -applaunch` before the trainer can be launched.
- **Trainer staging requires write access to compatdata.** The trainer `.exe` is copied into the game's compatdata prefix before Proton runs it. If the prefix is on a read-only filesystem, staging will fail.
- **Exported launchers are for single-file trainers.** Directory-based trainer bundles are not supported by the generated launcher scripts.
- **No macOS support yet.** The native application currently targets Linux. macOS support is planned for a future release.
- **Trainer compatibility depends on the Proton version.** A trainer that fails under one Proton version may work under a different one (especially GE-Proton). CrossHook does not control Proton compatibility.
- **Native mode does not support trainers.** The `native` launch method runs Linux-native executables only and does not include a trainer step.

## Troubleshooting

- **Auto-populate does not find the game.** Make sure the game has been installed through Steam and that its library folder is discoverable. If the game is on a secondary drive, verify that `libraryfolders.vdf` includes that library path.
- **Auto-populate finds the game but not the Proton version.** The Proton version may not be mapped in Steam's config files yet. Launch the game once from Steam to ensure the compat tool mapping is written, then try auto-populate again.
- **The game starts but the trainer does not.** Wait until the game has fully reached the in-game menu before launching the trainer. Some trainers require the game to be fully initialized.
- **The trainer starts but has no effect.** The trainer may be incompatible with the Proton version. Try a different Proton or GE-Proton version. Also confirm the trainer version matches the game version.
- **Exported launcher produces an error.** Run the generated `.sh` script manually from a terminal to see the full error output. Common causes: the Proton path has changed, the compatdata was deleted, or the trainer file was moved.
- **Launcher shows "Stale" status.** The profile's display name has changed since the launcher was last exported. Click "Re-export Launcher" to regenerate the files with the current profile data.
- **Launcher delete skipped a file.** CrossHook only deletes files it created. If a file is missing the CrossHook watermark or is a symlink, it is skipped and the reason is shown. Check whether the file was modified or replaced by another tool.
- **Orphaned launchers appear in Settings.** These are launcher files on disk that no longer match any saved profile. This can happen if a profile was deleted before the cascade cleanup feature was added, or if profile files were removed manually. Use the Settings panel to review and delete them.
- **Steam is not detected.** CrossHook checks `~/.steam/root`, `~/.local/share/Steam`, and the Flatpak location. If your Steam is installed elsewhere, set the path manually in Settings.
- **Compatdata does not exist.** The game must be launched through Steam at least once so that Steam creates the compatdata prefix. Run the game briefly from Steam, then return to CrossHook.
- **Console shows environment variable errors.** CrossHook strips ~30 inherited WINE/Proton variables before launching the trainer. If you see unexpected variable warnings, the clean-environment step may have encountered a non-standard Proton setup. Check the console output for specifics.

## Related Guides

- [CrossHook quickstart](../getting-started/quickstart.md)
