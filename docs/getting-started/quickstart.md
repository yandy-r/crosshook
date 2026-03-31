# CrossHook Quickstart

CrossHook is a native Linux desktop application that launches game trainers alongside Steam/Proton games. CrossHook runs directly on your Linux system -- no WINE or Proton required for CrossHook itself. Trainers still run under Proton, targeting the game's compatdata prefix. The same direct Proton path is used by the `Install Game` sub-tab inside the Profile panel when you are setting up a new Windows game, but that flow now opens a review modal so you can confirm the generated profile before saving it.

If you want the deeper Steam-specific workflow details, jump to the [Steam / Proton feature guide](../features/steam-proton-trainer-launch.doc.md).

## Table of Contents

1. [CrossHook Quickstart](#crosshook-quickstart)
   1. [Table of Contents](#table-of-contents)
   2. [Supported Environments](#supported-environments)
   3. [What You Need](#what-you-need)
   4. [Install CrossHook](#install-crosshook)
      1. [Linux Desktop](#linux-desktop)
      2. [Steam Deck](#steam-deck)
   5. [First Launch](#first-launch)
   6. [Create a Profile](#create-a-profile)
   7. [Custom environment variables](#custom-environment-variables)
   8. [Launch a Game with a Trainer](#launch-a-game-with-a-trainer)
   9. [Dry Run / Preview Mode](#dry-run--preview-mode)
   10. [Launch Modes](#launch-modes)
       1. [Steam App Launch (`steam_applaunch`)](#steam-app-launch-steam_applaunch)
       2. [Proton Run (`proton_run`)](#proton-run-proton_run)
       3. [Native (`native`)](#native-native)
   11. [External Launcher Export](#external-launcher-export)
   12. [Community Profiles](#community-profiles)
   13. [Pinned Profiles](#pinned-profiles)
   14. [Health Dashboard](#health-dashboard)
   15. [Diagnostic Export](#diagnostic-export)
   16. [Using the CLI](#using-the-cli)
       1. [Check system status](#check-system-status)
       2. [Manage profiles](#manage-profiles)
       3. [Steam discovery](#steam-discovery)
       4. [Launch a game](#launch-a-game)
       5. [Generate shell completions](#generate-shell-completions)
       6. [Export diagnostics](#export-diagnostics)
   16. [Troubleshooting](#troubleshooting)
   17. [Related Guides](#related-guides)

## Supported Environments

| Environment   | How CrossHook runs               | Best use case                                        |
| ------------- | -------------------------------- | ---------------------------------------------------- |
| Linux Desktop | Run the AppImage directly        | Desktop Linux users with Steam/Proton games          |
| Steam Deck    | Run the AppImage in Desktop Mode | SteamOS users running trainers alongside Steam games |

## What You Need

- The CrossHook AppImage.
- A game installed through Steam with Proton, or a native Linux game.
- A trainer executable (FLiNG, WeMod standalone, or similar) for the game you want to modify.
- For **Steam App Launch** mode: the game's Steam App ID. CrossHook auto-populates this when it discovers your Steam libraries.
- For **Proton Run** mode: a configured Proton/WINE prefix path and the Proton runner you want to use.

## Install CrossHook

### Linux Desktop

1. Download the latest AppImage from the [GitHub Releases page](https://github.com/yandy-r/crosshook/releases).
2. Make the AppImage executable:

   ```bash
   chmod +x CrossHook-*.AppImage
   ```

3. Run it:

   ```bash
   ./CrossHook-*.AppImage
   ```

4. Optionally, move the AppImage to a permanent location such as `~/Applications/` and add it to your application launcher.

### Steam Deck

1. Switch to Desktop Mode.
2. Download the latest AppImage from the GitHub Releases page using the built-in browser.
3. Open a terminal (Konsole) and make the AppImage executable:

   ```bash
   chmod +x ~/Downloads/CrossHook-*.AppImage
   ```

4. Run the AppImage directly from Desktop Mode:

   ```bash
   ~/Downloads/CrossHook-*.AppImage
   ```

The AppImage is a self-contained binary. It survives SteamOS updates and does not require Flatpak or any package manager.

## First Launch

When CrossHook starts for the first time, it automatically discovers your Steam libraries and installed games. This includes:

- All Steam library folders (including secondary drives).
- Installed games and their App IDs from `appmanifest_*.acf` files.
- Available Proton versions (official and custom, such as GE-Proton).
- Compatdata paths for each game.

This auto-populate step means you typically do not need to enter any Steam paths manually. If CrossHook cannot find your Steam installation (for example, a non-standard install location or Flatpak Steam), you can configure the Steam root path in Settings.

## Create a Profile

Profiles save your game, trainer, and launch configuration so you can reuse the same setup with one click.

1. Select a game from the discovered library, or browse for a game executable manually.
2. Browse for the trainer executable you want to use with the game.
3. CrossHook auto-populates the Steam App ID, compatdata path, and Proton version for the selected game.
4. Choose a launch mode (see [Launch modes](#launch-modes) below).
5. Save the profile with a descriptive name.

Profiles are saved as TOML files in `~/.config/crosshook/profiles/`. When you use `Install Game`, CrossHook defaults the prefix under `~/.local/share/crosshook/prefixes/<slug>`, runs the installer through `proton run`, then opens the generated profile in a review modal. You can adjust the draft there and save it explicitly; once save succeeds, CrossHook opens the Profile tab with that new profile selected. You can edit saved profiles by hand if needed. A profile looks like this:

```toml
[game]
name = "Elden Ring"
executable_path = "/mnt/games/SteamLibrary/steamapps/common/ELDEN RING/Game/eldenring.exe"

[trainer]
path = "/home/user/trainers/EldenRing_FLiNG.exe"
type = "fling"
loading_mode = "source_directory"

[steam]
enabled = true
app_id = "1245620"
compatdata_path = "/mnt/games/SteamLibrary/steamapps/compatdata/1245620"
proton_path = "/mnt/games/SteamLibrary/steamapps/common/Proton 9.0-4/proton"

[launch]
method = "steam_applaunch"
```

When a profile uses `proton_run`, the Profile editor also shows a `Launch Optimizations` panel in the right column. The panel is limited to `proton_run` profiles, and each visible option has an info tooltip that explains what it does, when it helps, and the main caveat. Existing saved profiles autosave only the optimization section, while new unsaved profiles show `Save profile first to enable autosave` until you save them once.

## Custom environment variables

In the Profile editor and in the **Profile Setup Wizard** (New Profile / Edit in Wizard on the Profiles page), **Custom Environment Variables** lets you add arbitrary `KEY=value` pairs that CrossHook applies when building launches.

**Precedence**

- For **Steam App Launch** and **Proton Run**, effective environment is built as: base/method and optimization-derived variables first, then your custom map. If the same key appears in both **Launch Optimizations** and **Custom Environment Variables**, the **custom** value wins.
- For **Native** launches, CrossHook starts from the host environment and applies your custom map on top (there are no launch optimizations in that mode).

**Steam App Launch:** CrossHook does not write into Steam. Custom variables are included in the **Steam launch options** copy/paste line (after optimization env assignments, before wrappers), consistent with [Dry run / preview mode](#dry-run--preview-mode).

**Reserved keys:** You cannot override runtime-managed variables through custom env. CrossHook rejects `WINEPREFIX`, `STEAM_COMPAT_DATA_PATH`, and `STEAM_COMPAT_CLIENT_INSTALL_PATH` so prefix and Steam paths stay under CrossHook’s control.

**Syntax rules:** Keys must be non-empty, must not contain `=`, and neither keys nor values may contain NUL bytes. Duplicate keys are avoided by the editor shape; invalid entries are surfaced in validation.

**Troubleshooting:** If a variable does not appear to take effect, use dry run and check the environment list — entries sourced from the profile show as **Profile custom**. For Steam games, confirm you pasted an updated launch options line after changing custom vars. For more Steam-specific detail, see the [Steam / Proton feature guide](../features/steam-proton-trainer-launch.doc.md#custom-environment-variables).

## Launch a Game with a Trainer

CrossHook uses a two-step launch flow for Steam/Proton games:

1. **Launch the game.** Click the launch button. CrossHook starts the game through Steam (or Proton directly, depending on the mode). Wait for the game to reach the in-game menu.
2. **Launch the trainer.** Once the game is running, click the launch button again. CrossHook runs the trainer through Proton with a clean environment. By default it uses the trainer's current directory so stateful bundles keep one shared install, and you can switch a profile to `Copy into prefix` when a trainer needs staged files inside the target prefix.

If you used `Install Game` first, make sure you save the reviewed profile before launching. After a successful save, CrossHook switches you to the Profile tab with that profile selected.

The console view in CrossHook streams the runner output in real-time so you can see exactly what is happening at each step.

## Dry Run / Preview Mode

Before launching, you can preview exactly what commands CrossHook will execute. Use the dry run option to see the full command line, environment variables, and launch sequence without starting any processes. This is useful for debugging launch configurations or verifying that paths are correct. The environment list is merged the same way as a real launch (including [custom environment variables](#custom-environment-variables)), and shows which values come from **Profile custom** versus optimizations.

## Launch Modes

CrossHook supports three launch methods. The right choice depends on how your game is installed and run.

### Steam App Launch (`steam_applaunch`)

The default mode for games installed through Steam.

- CrossHook launches the game using `steam -applaunch <appid>`, which lets Steam handle DRM, the overlay, and the Proton runtime.
- The trainer is then launched separately against the same prefix using Proton directly.
- The Trainer section lets you choose `Run from current directory` or `Copy into prefix`. The default source-directory mode is better for stateful trainers such as Aurora.
- Requires: Steam App ID, compatdata path, Proton path, Steam client install path. All of these are auto-populated by CrossHook's Steam discovery.

### Proton Run (`proton_run`)

For launching games and trainers directly through Proton without going through the Steam client.

- CrossHook launches the game (and trainer) using `proton run` against a specified WINE prefix.
- Trainers use the same per-profile loading mode as Steam launches: source-directory by default, optional copy-to-prefix for compatibility cases.
- Useful for non-Steam games that use a standalone Proton/WINE prefix, or when you need full control over the prefix path.
- Requires: a WINE/Proton prefix path and the Proton runner path.
- The `Install Game` flow uses the same direct Proton path, then opens a review modal for the generated profile before save. Saving the modal draft opens the Profile tab with the saved profile selected.
- The `Launch Optimizations` panel is available here and nowhere else; it stays scoped to `proton_run`, shows per-option help icons, and only autosaves after the profile already exists.

### Native (`native`)

For Linux-native executables that do not run under Proton or WINE.

- CrossHook launches the game executable directly on the host system.
- Does not support the two-step trainer workflow (trainers are Windows executables and need Proton).
- Useful for running native Linux games alongside other tools.

## External Launcher Export

CrossHook can generate standalone shell scripts and `.desktop` entries so you can launch a trainer without opening CrossHook.

1. Open a saved profile with a Steam-mode configuration.
2. Use the export function to generate launcher files.
3. CrossHook writes the generated files to:
   - `~/.local/share/crosshook/launchers/` (shell scripts)
   - `~/.local/share/applications/` (desktop entries)

The generated script sets the required Proton environment variables and follows the profile's trainer loading mode:

```bash
export STEAM_COMPAT_DATA_PATH='...'
export STEAM_COMPAT_CLIENT_INSTALL_PATH='...'
export WINEPREFIX="$STEAM_COMPAT_DATA_PATH/pfx"
exec "$PROTON" run "$TRAINER_HOST_PATH"
```

The `.desktop` entry runs the script with `/bin/bash`, making the trainer launchable from your desktop's application menu.

Start the game through Steam first, wait for the in-game menu, then run the exported launcher.

## Community Profiles

CrossHook supports community profile sharing through a taps system, similar to Homebrew taps. A tap is a Git repository containing shared game profiles.

- **Add a tap**: Provide the URL of a Git repository containing CrossHook profiles. CrossHook clones the repository locally.
- **Browse profiles**: View all profiles from your subscribed taps. Each profile includes the game name, trainer type, and recommended configuration.
- **Import a profile**: Copy a community profile into your local profile library. You can then customize it for your system (paths, Proton version, etc.).
- **Sync**: Pull the latest updates from all subscribed taps.

Community profiles are a convenient starting point, but you will still need to adjust paths to match your local system.

## Pinned Profiles

Pin your most-used profiles for quick access. Pinned profiles appear as a strip at the top of both the Profiles and Launch pages, letting you switch configurations with a single click. Pin and unpin profiles from the profile action bar.

## Health Dashboard

The Health Dashboard provides an overview of all your profiles' health. It shows:

- Health scores calculated from path validity, configuration completeness, and staleness indicators.
- Sortable metadata columns including last launched, launch count, and profile age.
- Trend analysis showing whether profile health is improving or degrading over time.

Access the Health Dashboard from the main navigation tabs.

## Diagnostic Export

If you run into issues, CrossHook can generate a diagnostic bundle containing system information, application state, and relevant logs. Use this when reporting bugs or troubleshooting launch problems. The export is available from the Settings panel.

## Using the CLI

CrossHook includes a standalone CLI binary (`crosshook`) for headless operation, scripting, and automation. All commands support `--json` for machine-readable output and `--verbose` for diagnostic detail.

### Check system status

```bash
crosshook status
crosshook status --json
```

### Manage profiles

```bash
# List all saved profiles
crosshook profile list

# Import a legacy .profile file
crosshook profile import --legacy-path ~/.config/crosshook-old/elden-ring.profile

# Export a profile as a portable community JSON file
crosshook profile export --profile elden-ring
crosshook profile export --profile elden-ring --output /tmp/elden-ring.json
```

### Steam discovery

```bash
# Scan for Steam installations, libraries, and Proton versions
crosshook steam discover

# Auto-populate Steam metadata from a game executable
crosshook steam auto-populate --game-path /mnt/games/SteamLibrary/steamapps/common/ELDEN\ RING/Game/eldenring.exe
```

### Launch a game

```bash
# Launch using a saved profile
crosshook launch --profile elden-ring

# Preview what would be launched without executing
crosshook launch --profile elden-ring --dry-run
crosshook launch --profile elden-ring --dry-run --json
```

### Generate shell completions

```bash
# Generate completions for your shell (bash, zsh, fish, powershell)
crosshook completions zsh > ~/.zfunc/_crosshook
crosshook completions bash > /etc/bash_completion.d/crosshook
```

### Export diagnostics

```bash
crosshook diagnostics export
crosshook diagnostics export --redact-paths --output /tmp/diag
```

## Troubleshooting

- **CrossHook does not discover my Steam games.** Verify that Steam is installed and that `~/.steam/root` or `~/.local/share/Steam` exists. For Flatpak Steam, CrossHook also checks `~/.var/app/com.valvesoftware.Steam/data/Steam`. If your install is somewhere else, set the Steam root path in Settings.
- **Auto-populate cannot find the Proton version.** CrossHook searches both official Proton installs and custom versions in `~/.steam/root/compatibilitytools.d/`. Make sure the Proton version you want is installed through Steam or placed in the correct directory.
- **The trainer opens but does not affect the game.** Confirm the game has reached the in-game menu before launching the trainer. Some trainers require a specific timing window.
- **Compatdata path does not exist.** The game must be launched through Steam at least once so that Steam creates its compatdata prefix. Launch the game normally from Steam, let it run briefly, then try again.
- **The trainer fails to start under Proton.** Test the same trainer manually with `proton run` outside CrossHook to confirm the Proton version supports that trainer. Some trainers only work with specific Proton or GE-Proton versions.
- **Exported launcher does not work.** Verify that all required fields (trainer path, Steam App ID, compatdata path, Proton path) were populated when you exported. Run the generated `.sh` script manually from a terminal to see error output.
- **Permission errors when attaching to a process.** Advanced features that use `ptrace` may be restricted by your kernel's `yama.ptrace_scope` setting. The primary trainer workflow (Proton run) is unaffected by this.
- **Health scores show "unknown" for a profile.** Launch the game at least once so CrossHook can record operational metadata. Health scores require at least one launch event to calculate.
- **Dry run output differs from actual launch.** The dry run captures the planned command at preview time. If you change profile fields after the preview, the actual launch will use the updated values.

## Related Guides

- [Steam / Proton trainer workflow](../features/steam-proton-trainer-launch.doc.md)
- [Profile duplication](../features/profile-duplication.doc.md)
- [README](../../README.md)
