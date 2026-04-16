# Systems Mapping: Dependency Graphs & Permission Models for Flatpak Tool Bundling

**Perspective**: Systems Mapper
**Date**: 2026-04-15
**Scope**: Complete dependency graph, permission model, and execution chain analysis for each tool CrossHook might bundle in its Flatpak package.

---

## Table of Contents

1. [CrossHook's Current Flatpak Architecture](#1-crosshooks-current-flatpak-architecture)
2. [The Execution Chain](#2-the-execution-chain)
3. [Tool-by-Tool Dependency & Permission Mapping](#3-tool-by-tool-dependency--permission-mapping)
   - [3.1 Winetricks](#31-winetricks)
   - [3.2 Winecfg (Wine Configuration)](#32-winecfg-wine-configuration)
   - [3.3 MangoHud](#33-mangohud)
   - [3.4 Gamescope](#34-gamescope)
   - [3.5 Feral GameMode](#35-feral-gamemode)
   - [3.6 umu-launcher](#36-umu-launcher)
   - [3.7 CachyOS Optimizations](#37-cachyos-optimizations)
   - [3.8 Proton Manager (ProtonUp-Qt style)](#38-proton-manager-protonup-qt-style)
   - [3.9 Wine Wayland Driver](#39-wine-wayland-driver)
4. [Cross-Cutting Permission Analysis](#4-cross-cutting-permission-analysis)
5. [Feedback Loops & Failure Modes](#5-feedback-loops--failure-modes)
6. [Permission Escalation Paths](#6-permission-escalation-paths)
7. [Bundleability Matrix](#7-bundleability-matrix)

---

## 1. CrossHook's Current Flatpak Architecture

CrossHook's Flatpak manifest (`packaging/flatpak/dev.crosshook.CrossHook.yml`) declares:

### Runtime & SDK

- **Runtime**: `org.gnome.Platform` version `50`
- **SDK**: `org.gnome.Sdk`
- **Binary**: Pre-built `crosshook-native` (Phase 1 approach)

### Sandbox Permissions (finish-args)

| Permission                                           | Purpose                                     |
| ---------------------------------------------------- | ------------------------------------------- |
| `--socket=wayland`                                   | Primary display                             |
| `--socket=fallback-x11`                              | X11 fallback                                |
| `--share=ipc`                                        | Shared memory for X11                       |
| `--device=dri`                                       | GPU access (DRM/DRI)                        |
| `--socket=pulseaudio`                                | Audio for games via Proton                  |
| `--share=network`                                    | ProtonDB, SteamGridDB, downloads            |
| `--talk-name=org.freedesktop.Flatpak`                | **Critical**: `flatpak-spawn --host` access |
| `--env=WEBKIT_DISABLE_DMABUF_RENDERER=1`             | NVIDIA Wayland workaround                   |
| `--filesystem=home`                                  | Home directory access                       |
| `--filesystem=/mnt`                                  | External drives                             |
| `--filesystem=/run/media`                            | Removable media                             |
| `--filesystem=/media`                                | Legacy media mount                          |
| `--filesystem=~/.var/app/com.valvesoftware.Steam:ro` | Flatpak Steam discovery                     |
| `--filesystem=xdg-data/umu:create`                   | umu runtime shared directory                |

### Host Interaction Model

CrossHook's `platform.rs` implements a sophisticated host-command abstraction:

- **`host_command()`** / **`host_command_with_env()`**: Wraps commands with `flatpak-spawn --host` when `is_flatpak()` returns true.
- **`--clear-env`**: Used to prevent sandbox env vars from leaking to host processes.
- **`--env=KEY=VALUE`**: Threads env vars explicitly because `.env()` on `Command` is silently dropped by `flatpak-spawn`.
- **Custom env file handoff**: For user-controlled vars, writes a `0600` env file sourced by `bash` on the host to avoid exposing values in the process argv.
- **`host_command_exists()`**: Probes host `PATH` via `which` through `flatpak-spawn --host`.
- **XDG override**: Remaps sandbox XDG paths to host paths at startup so both AppImage and Flatpak share the same data directories.

**Confidence**: High (verified from source code in `crates/crosshook-core/src/platform.rs`)

---

## 2. The Execution Chain

```
CrossHook GUI (Tauri v2 / WebKitGTK)
    |
    v
crosshook-core (Rust business logic)
    |
    |-- is_flatpak()?
    |       |
    |       +-- YES: flatpak-spawn --host --clear-env --env=K=V ... <program> <args>
    |       |       |
    |       |       v
    |       |   org.freedesktop.Flatpak D-Bus service
    |       |       |
    |       |       v
    |       |   Host process (unsandboxed)
    |       |       |
    |       |       +-- umu-run / proton / wine
    |       |       |       |
    |       |       |       v
    |       |       |   Steam Linux Runtime (pressure-vessel container)
    |       |       |       |
    |       |       |       v
    |       |       |   Wine/Proton → Windows game + trainer
    |       |       |
    |       |       +-- gamescope (compositor wrapping game)
    |       |       +-- mangohud (Vulkan layer injection)
    |       |       +-- gamemode (D-Bus activation)
    |       |
    |       +-- NO: Direct process spawn
    |               |
    |               v
    |           Same chain as above, no flatpak-spawn wrapper
    |
    v
Result / process monitoring / watchdog
```

### Key Observation

The `--talk-name=org.freedesktop.Flatpak` permission is a **complete sandbox escape by design**. Any command run via `flatpak-spawn --host` executes unsandboxed with the user's full permissions. This means:

1. CrossHook's sandbox is effectively **cosmetic** for launched processes.
2. All host tools are accessible via this path regardless of bundling.
3. The sandbox only constrains CrossHook's own process, not anything it spawns on the host.

**Confidence**: High (confirmed by Flatpak documentation, CVE-2021-21261 advisory, and multiple community discussions)

---

## 3. Tool-by-Tool Dependency & Permission Mapping

### 3.1 Winetricks

#### What It Is

A POSIX shell script that automates installing Windows DLLs, fonts, redistributables, and tweaking Wine prefixes. It downloads and extracts components into a specific Wine prefix.

#### Runtime Dependencies

| Dependency                       | Type | Purpose                                |
| -------------------------------- | ---- | -------------------------------------- |
| **Wine** (any version)           | Hard | Executes Win32 installers/regedit      |
| **cabextract**                   | Hard | Extracts `.cab` archives               |
| **unzip**                        | Hard | Extracts `.zip` archives               |
| **wget** / **curl** / **aria2c** | Hard | Downloads components from the internet |
| **p7zip** / **7zip**             | Soft | Some verbs need 7z extraction          |
| **unrar**                        | Soft | RAR archive extraction                 |
| **zenity**                       | Soft | GUI dialogs (not needed for CLI mode)  |
| **perl**                         | Soft | Download progress display              |
| **sha256sum**                    | Soft | Integrity verification                 |
| **fuseiso** / **archivemount**   | Soft | Mounting `.iso` images                 |
| **xz**                           | Soft | Decompressing tar archives             |

#### Wine Version Coupling

- **Tight coupling**: Winetricks modifies the Wine prefix that belongs to a specific Wine/Proton version. The prefix format and registry structure must match the Wine version.
- Winetricks targets the **host's** Wine/Proton prefix, which lives in the user's home directory (e.g., `~/.local/share/Steam/steamapps/compatdata/<appid>/pfx/`).
- Cannot meaningfully operate against a different Wine version than the one that created the prefix.

#### Filesystem Needs

- **Read/Write**: Wine prefix directory (typically under `$HOME` or Steam library paths)
- **Read/Write**: Download cache (`~/.cache/winetricks/`)
- **Network**: Downloads from Microsoft, archive.org, and other sources

#### Can It Run Inside the Sandbox?

**No, not practically.** Winetricks must:

1. Execute the **host's** Wine/Proton binary (the one matching the game's prefix).
2. Write to prefix directories on the host filesystem.
3. Download and extract files that end up in the host prefix.
4. Shell out to `cabextract`, `wget`, etc., which must be host-installed.

Even with `--filesystem=home`, the Wine binary inside the GNOME runtime would be a different version from the host's Proton, causing prefix corruption.

**Known Flatpak Issue**: Lutris Flatpak 0.5.19 hit `libassuan.so.0: undefined symbol` errors when winetricks tried to use the sandbox's `wget`, because the sandbox runtime libraries diverged from what winetricks expected.

#### Bundling Verdict

**Cannot meaningfully bundle.** Must run on host via `flatpak-spawn --host`.

**Confidence**: High (source code analysis of winetricks, Lutris bug reports, Wine prefix architecture)

---

### 3.2 Winecfg (Wine Configuration)

#### What It Is

A GUI tool built into Wine that configures Wine settings: Windows version emulation, DLL overrides, display settings, audio drivers, and per-application settings. It is a Wine executable (`winecfg.exe` internally).

#### Runtime Dependencies

| Dependency                       | Type | Purpose                          |
| -------------------------------- | ---- | -------------------------------- |
| **Wine** (exact version)         | Hard | winecfg IS part of Wine          |
| **Display server** (X11/Wayland) | Hard | GUI rendering                    |
| **Wine prefix** (specific)       | Hard | Reads/writes registry and config |

#### Display Server Needs

- Requires a display connection (X11 or Wayland via XWayland).
- In Flatpak, display access is via `--socket=wayland` / `--socket=fallback-x11`.
- However, winecfg runs as a **Wine** process, meaning it needs the host Wine to render its Win32 GUI through XWayland.

#### Registry Access

- Directly reads/writes `system.reg`, `user.reg`, and `userdef.reg` in the Wine prefix.
- DLL override settings go to `HKEY_CURRENT_USER\Software\Wine\DllOverrides`.
- Display settings in `HKEY_CURRENT_USER\Software\Wine\X11 Driver` (or Wayland driver keys).

#### Bundling Means Bundling Wine

**Yes.** `winecfg` is not a standalone tool -- it is a component of Wine itself. Bundling winecfg means bundling a full Wine installation, including:

- Wine server (`wineserver`)
- Wine loader (`wine` / `wine64`)
- All Wine DLLs
- Gecko and Mono runtimes
- **Both 32-bit and 64-bit libraries** (multiarch)

This would be an enormous addition (~500MB-1.5GB depending on configuration) and would **conflict** with the host Wine/Proton version that created the game's prefix.

#### Bundling Verdict

**Cannot bundle.** Winecfg must match the exact Wine/Proton version that owns the target prefix. Running it from a bundled Wine against a host prefix would corrupt the prefix.

**Confidence**: High (Wine architecture is well-documented; prefix version coupling is fundamental)

---

### 3.3 MangoHud

#### What It Is

A Vulkan and OpenGL overlay for monitoring FPS, temperatures, CPU/GPU load, and more. Works via Vulkan implicit layer mechanism and `LD_PRELOAD` for OpenGL.

#### Runtime Dependencies

| Dependency                        | Type       | Purpose                    |
| --------------------------------- | ---------- | -------------------------- |
| **Vulkan ICD loader**             | Hard       | Vulkan layer registration  |
| **libMangoHud.so** (x86_64 + x86) | Hard       | The overlay library        |
| **libXNVCtrl**                    | Soft       | NVIDIA GPU monitoring      |
| **D-Bus**                         | Soft       | GameMode status display    |
| **glslang**                       | Build-only | Shader compilation         |
| **mesa**                          | Build-only | OpenGL development headers |

#### Vulkan Layer Mechanism

MangoHud registers as a **Vulkan implicit layer** via JSON manifest files:

- `/usr/share/vulkan/implicit_layer.d/MangoHud.x86_64.json`
- `/usr/share/vulkan/implicit_layer.d/MangoHud.x86.json`

The layer is automatically loaded by the Vulkan loader when `MANGOHUD=1` is set. The JSON points to `libMangoHud.so` with `enable_environment: MANGOHUD=1`.

#### LD_PRELOAD Injection (OpenGL)

For OpenGL games, MangoHud uses `LD_PRELOAD=libMangoHud.so` to intercept GL calls and render the overlay.

#### Flatpak Extension: `org.freedesktop.Platform.VulkanLayer.MangoHud`

**This is the correct Flatpak integration path.** Available on Flathub:

- Installation: `flatpak install org.freedesktop.Platform.VulkanLayer.MangoHud`
- Branches match platform versions: `21.08`, `22.08`, `24.08`, `25.08`
- Activation: `flatpak override --user --env=MANGOHUD=1 <app-id>`

The extension mounts MangoHud libraries and layer JSON files into the Flatpak runtime via the `org.freedesktop.Platform.VulkanLayer.*` extension point, which bind-mounts to `/usr/lib/extensions/vulkan/`.

**Known Issue**: Vulkan layers stopped working temporarily in Steam Flatpak (~March 2024) due to `pressure-vessel` not including the extensions data directory in `XDG_DATA_DIRS`.

#### For CrossHook Specifically

CrossHook launches games **on the host** via `flatpak-spawn --host`. This means:

- The game process runs outside the sandbox.
- MangoHud must be installed on the **host**, not in the sandbox.
- The sandbox MangoHud extension would only apply to CrossHook's own Vulkan rendering (the Tauri WebKitGTK window), which is irrelevant.
- The `MANGOHUD=1` env var is already threaded to host processes via `host_command_with_env()`.

#### Bundling Verdict

**Should NOT bundle.** Games run on the host; MangoHud must be host-installed. CrossHook already correctly passes `MANGOHUD=1` via env vars to host processes. Adding the VulkanLayer extension to CrossHook's manifest is unnecessary and could cause confusion.

**Confidence**: High (MangoHud architecture, CrossHook source code, Flatpak extension docs)

---

### 3.4 Gamescope

#### What It Is

A Wayland microcompositor from Valve (used on Steam Deck). Provides resolution spoofing, frame scaling, FSR upscaling, frame limiting, and an isolated display sandbox for games.

#### Runtime Dependencies

| Dependency            | Type | Purpose                     |
| --------------------- | ---- | --------------------------- |
| **libwlroots**        | Hard | Wayland compositor library  |
| **libdrm**            | Hard | DRM/KMS display management  |
| **vulkan-icd-loader** | Hard | GPU rendering               |
| **Xwayland**          | Hard | X11 compatibility for games |
| **libinput**          | Hard | Input device handling       |
| **libseat**           | Hard | Session/seat management     |
| **SDL2**              | Hard | Window management           |
| **PipeWire**          | Soft | Screen capture/streaming    |
| **libavif**           | Soft | Screenshot encoding         |
| **libliftoff**        | Hard | DRM plane offloading        |
| **libdisplay-info**   | Hard | EDID/display info parsing   |
| **openvr**            | Soft | VR headset support          |
| **libxkbcommon**      | Hard | Keyboard handling           |
| **libXNVCtrl**        | Soft | NVIDIA-specific control     |

This is a **massive** dependency tree -- the Arch Linux package lists 25+ runtime dependencies, and the build requires vendored submodules for `vkroots`, `wlroots`, `libliftoff`, and `libdisplay-info`.

#### KMS/DRM Requirements

- **Embedded mode** (Steam Deck session compositor): Requires direct KMS/DRM access. Can directly flip game frames to the screen. Needs `--device=dri` **and** full DRM master access.
- **Nested mode** (running on an existing desktop): Runs as a Wayland client on the host compositor. Renders to a Wayland surface. More permissive -- `--device=dri` is sufficient for GPU access.
- In both modes, `--device=dri` is required. Nested mode is what CrossHook users on a desktop would use.

#### Flatpak Extension: `org.freedesktop.Platform.VulkanLayer.gamescope`

Available as a Flatpak extension on Flathub:

- Install size: ~122.7 MB
- Mounts to `/usr/lib/extensions/vulkan/gamescope/`
- **Described as "a hack"** by the maintainers: the VulkanLayer extension point wasn't designed for full compositor binaries
- Introduces shared libraries that may **conflict** with app-provided ones: `libevdev`, `libfontenc`, `libinput`, `libliftoff`, `libmtdev`, `libseat`, `libwlroots`, `libxcvt`, `libXfont2`, `libXRes`
- Does NOT work with official Proton versions in nested sandbox mode (only community Proton builds)

#### For CrossHook Specifically

CrossHook uses gamescope as a **wrapper** around game launches. The `build_gamescope_args()` function constructs gamescope command-line arguments that wrap the Proton/Wine launch.

Since games launch on the **host** via `flatpak-spawn --host`:

- Gamescope must be the **host's** gamescope.
- The host gamescope needs direct access to the host's DRM devices, Wayland compositor, and display.
- A sandbox-bundled gamescope would need to start a compositor inside the sandbox and then somehow render through the host compositor -- this is architecturally nonsensical.

#### Bundling Verdict

**Cannot bundle.** Gamescope is a compositor that needs direct host display and DRM access. Games launch on the host. The Flatpak VulkanLayer extension is a known hack that doesn't work reliably. Host installation is the only viable path.

**Confidence**: High (gamescope architecture, Flatpak extension issues, CrossHook launch chain)

---

### 3.5 Feral GameMode

#### What It Is

A daemon that optimizes Linux system performance on demand: adjusts CPU governor, I/O priority, process niceness, and GPU performance mode when games are running.

#### Architecture (Daemon + Client Library)

```
gamemoded (daemon, runs on host, started via D-Bus activation)
    |
    +-- systemd user service (org.freedesktop.GameMode.service)
    +-- D-Bus interface: com.feralinteractive.GameMode
    +-- Polkit policy: com.feralinteractive.GameMode.policy
    |
libgamemode.so (client library, linked by game launchers)
    |
    +-- Communicates with daemon via sd-bus (systemd D-Bus)
    |
libgamemodeauto.so (auto-registering preload library)
    |
    +-- LD_PRELOAD into game process, auto-registers with daemon
```

#### Runtime Dependencies

| Dependency            | Type | Purpose                              |
| --------------------- | ---- | ------------------------------------ |
| **systemd** (sd-bus)  | Hard | Daemon-client communication          |
| **D-Bus** session bus | Hard | Service activation and IPC           |
| **inih**              | Hard | Config file parsing                  |
| **Polkit**            | Soft | Privilege elevation for CPU governor |

#### D-Bus Portal: `org.freedesktop.portal.GameMode`

The XDG Desktop Portal provides a **sandboxed interface** for GameMode:

- Added in GameMode 1.4
- Translates PID from Flatpak PID namespace to host namespace
- Methods: `RegisterGame`, `UnregisterGame`, `QueryStatus`
- The sandbox app talks to `org.freedesktop.portal.GameMode` which proxies to `com.feralinteractive.GameMode` on the host

**This means GameMode works from inside a Flatpak without any special permissions or bundling.** The host just needs the GameMode daemon installed.

#### For CrossHook Specifically

CrossHook's launch optimizations include GameMode. Since games launch on the **host**:

- `gamemoderun` (wrapper) or `libgamemodeauto.so` (preload) runs in the host process.
- The host daemon is activated via D-Bus on the session bus.
- CrossHook only needs to inject `gamemoderun %command%` or `LD_PRELOAD=libgamemodeauto.so` into the host launch command.

If CrossHook itself wanted to register as a game (unlikely), it could use the portal. But the actual game process runs on the host where `gamemoded` is directly accessible.

#### Known Issues

- Steam Flatpak: GameMode enables but doesn't properly register the game process PID (core pinning doesn't work).
- MangoHud can't use the portal for GameMode status display -- needs `--talk-name=com.feralinteractive.GameMode` override.

#### Bundling Verdict

**Should NOT bundle.** GameMode works perfectly via D-Bus portal from sandbox, and the actual game processes run on the host where the daemon is natively accessible. Bundling the daemon would conflict with the host daemon.

**Confidence**: High (GameMode architecture, portal documentation, CrossHook launch model)

---

### 3.6 umu-launcher

#### What It Is

A unified launcher for Windows games on Linux that provides Valve's Steam Runtime and Proton compatibility layer to non-Steam game launchers (Lutris, Heroic, Bottles, CrossHook).

#### Architecture

```
umu-run (entry point)
    |
    +-- Python 3.11+ application
    |       +-- Build deps: hatchling, build, installer
    |       +-- Runtime: tomllib (stdlib 3.11+)
    |
    +-- Downloads/manages Steam Linux Runtime (SLR)
    |       +-- SteamLinuxRuntime_sniper
    |       +-- pressure-vessel container
    |
    +-- Proton version management
    |       +-- Detects/downloads GE-Proton, CachyOS-Proton
    |       +-- STEAM_COMPAT_DATA_PATH, STEAM_COMPAT_CLIENT_INSTALL_PATH
    |
    v
_v2-entry-point (renamed to 'umu')
    |
    v
pressure-vessel container
    |
    v
Wine/Proton -> Windows game
```

#### Runtime Dependencies

| Dependency                  | Type  | Purpose              |
| --------------------------- | ----- | -------------------- |
| **Python 3.11+**            | Hard  | Core runtime         |
| **Cargo** (Rust)            | Build | Binary components    |
| **bash**                    | Hard  | Entry point scripts  |
| **Steam Runtime (sniper)**  | Hard  | Container runtime    |
| **pressure-vessel**         | Hard  | Container management |
| **Proton** (any compatible) | Hard  | Wine fork for gaming |

#### Flatpak Packaging Challenges

**This is the most problematic tool for Flatpak bundling.**

Active GitHub issue (#430) documents the difficulties:

1. **Pre-built zipapp** fails inside Flatpak with `libdl.so.2` shared library loading errors from pressure-vessel.
2. **Python dependency tree** is enormous -- Flatpak definitions for apps using umu become "incredibly huge due to the need to package all the python deps."
3. Three major Flatpak consumers (Heroic, Lutris, faugus-launcher) each handle umu differently:
   - Heroic: Ships a massive AppImage-in-Flatpak (workaround)
   - Lutris: Already a Python app, extends its extensive Python Flatpak
   - faugus-launcher: Also Python, includes full Python deps

#### CrossHook's Current Approach

CrossHook resolves umu-run on the **host** via `flatpak-spawn --host`:

- `resolve_umu_run_path()` in `runtime_helpers.rs` finds `umu-run` on the host PATH
- The `--filesystem=xdg-data/umu:create` permission shares the umu runtime directory between Flatpak and host
- XDG override in `platform.rs` ensures both AppImage and Flatpak see the same umu data

#### Bundling Verdict

**Cannot practically bundle.** The Python + Rust + Steam Runtime dependency chain is enormous, the pre-built zipapp fails inside Flatpak containers, and umu-run must interact with host Steam installations and Proton versions. Host-side installation is the only viable path. CrossHook's current `flatpak-spawn --host` approach is correct.

**Confidence**: High (umu-launcher GitHub issues, CrossHook source code, dependency analysis)

---

### 3.7 CachyOS Optimizations

#### What They Are

Kernel-level performance optimizations specific to CachyOS and similar performance-focused Linux distributions.

#### Components

| Component                          | Level                                | Bundleable?                         |
| ---------------------------------- | ------------------------------------ | ----------------------------------- |
| **BORE Scheduler**                 | Kernel patch to EEVDF                | **No** -- compiled into kernel      |
| **sched_ext (SCX)**                | Kernel framework + BPF schedulers    | **No** -- kernel feature            |
| **scx_bpfland**                    | Userspace BPF scheduler              | **No** -- requires kernel sched_ext |
| **CPU x86-64-v3/v4 optimizations** | Kernel + userspace compilation flags | **No** -- build-time                |
| **LTO (Link-Time Optimization)**   | Kernel compilation                   | **No** -- build-time                |
| **AutoFDO + Propeller**            | Profile-guided optimization          | **No** -- build-time                |
| **Timer frequency (1000Hz)**       | Kernel config                        | **No** -- kernel parameter          |
| **I/O schedulers**                 | Kernel modules                       | **No** -- kernel level              |
| **sysctl tunables**                | Kernel parameters                    | **No** -- requires root/privileged  |

#### Why None of This Can Be Bundled

Every CachyOS optimization operates at the **kernel level** or requires **privileged system access**:

1. **Kernel patches** (BORE, sched_ext): Compiled into the kernel itself. Cannot be applied from userspace.
2. **BPF schedulers** (scx_bpfland): Load into the kernel's sched_ext framework. Require the kernel to have sched_ext compiled in.
3. **CPU optimizations**: Affect how every binary on the system is compiled. Not a runtime toggle.
4. **sysctl tunables**: Require `root` or `CAP_SYS_ADMIN`. Flatpak sandbox explicitly prevents this.

#### What CrossHook Can Do

CrossHook can:

- **Detect** if the user is running CachyOS/cachyos kernel (via `os-release` and `uname`).
- **Recommend** sched_ext scheduler profiles if available.
- **Set process nice/ionice** via GameMode (already covered above).
- **Display** kernel optimization status in the UI for informational purposes.

CrossHook **cannot** enable, install, or configure any of these optimizations.

#### Bundling Verdict

**Literally impossible to bundle.** These are kernel-level features. The Flatpak sandbox is the polar opposite of kernel access.

**Confidence**: High (kernel architecture is fundamental; no amount of Flatpak permissions can grant kernel compilation access)

---

### 3.8 Proton Manager (ProtonUp-Qt style)

#### What It Is

A tool that downloads and manages third-party Proton/Wine compatibility tools (GE-Proton, CachyOS-Proton, Luxtorpeda, etc.) for Steam and other launchers.

#### Architecture

```
ProtonUp-Qt GUI (Python 3 + Qt 6)
    |
    +-- Downloads GE-Proton releases from GitHub
    +-- Extracts to Steam's compatibilitytools.d/
    +-- Manages multiple installed versions
    |
    v
~/.steam/root/compatibilitytools.d/
~/.local/share/Steam/compatibilitytools.d/
~/.var/app/com.valvesoftware.Steam/data/Steam/compatibilitytools.d/  (Flatpak Steam)
```

#### Runtime Dependencies

| Dependency                  | Type | Purpose                         |
| --------------------------- | ---- | ------------------------------- |
| **Python 3**                | Hard | Core runtime                    |
| **Qt 6 / PySide6**          | Hard | GUI framework                   |
| **Network access**          | Hard | Download releases from GitHub   |
| **Filesystem write access** | Hard | Extract to compatibilitytools.d |

#### Filesystem Requirements

- **Native Steam**: Write to `~/.steam/root/compatibilitytools.d/` or `~/.local/share/Steam/compatibilitytools.d/`
- **Flatpak Steam**: Write to `~/.var/app/com.valvesoftware.Steam/data/Steam/compatibilitytools.d/`
- **Lutris**: Write to `~/.local/share/lutris/runners/wine/`

#### For CrossHook

CrossHook needs to:

1. **Download** GE-Proton/CachyOS-Proton releases (network + write to host filesystem).
2. **Extract** them to the correct directory based on whether Steam is native or Flatpak.
3. **Present** available versions in the UI.

CrossHook's manifest already has `--filesystem=home` (covers native Steam paths) and `--filesystem=~/.var/app/com.valvesoftware.Steam:ro` (read-only for Flatpak Steam discovery).

**Problem**: The `:ro` on the Flatpak Steam path means CrossHook **cannot** write compatibility tools for Flatpak Steam installations. Writing to Flatpak Steam's data would require `--filesystem=~/.var/app/com.valvesoftware.Steam:rw`.

#### Alternative: GE-Proton as Flatpak Extension

Flathub offers `com.valvesoftware.Steam.CompatibilityTool.Proton-GE`, but it only supports one version at a time and is a pain for version management.

#### Bundling Verdict

**Partial capability -- not traditional "bundling" but feature integration.** CrossHook can implement Proton download/management as a built-in feature (no external tool needed) using its existing network and filesystem permissions. The core challenge is having write access to Flatpak Steam's directory. This is a feature, not a bundled tool.

**Confidence**: Medium (filesystem permission model is clear, but the UX for Flatpak Steam write access needs exploration)

---

### 3.9 Wine Wayland Driver

#### What It Is

An in-tree Wine driver (`winewayland.drv`) that allows Wine to render directly to Wayland compositors without XWayland intermediation.

#### Status Timeline

| Version     | Date      | Status                                                     |
| ----------- | --------- | ---------------------------------------------------------- |
| Wine 9.0    | Jan 2024  | Experimental, disabled by default                          |
| Wine 9.22   | Nov 2024  | Enabled in default config (used when X11 fails)            |
| Wine 10.0   | Jan 2025  | Significantly improved, OpenGL support, enabled by default |
| Wine 10.3   | Mar 2025  | Clipboard support added                                    |
| Wine 10.18+ | Late 2025 | Continued improvements                                     |

#### Dependencies

| Dependency                            | Type  | Purpose                    |
| ------------------------------------- | ----- | -------------------------- |
| **Wine** (built with Wayland support) | Hard  | The driver IS part of Wine |
| **Wayland compositor**                | Hard  | Display server             |
| **Wayland client libraries**          | Hard  | `libwayland-client.so`     |
| **wayland-protocols**                 | Build | Protocol definitions       |
| **XDG shell protocol**                | Hard  | Window management          |

#### Flatpak Implications

- The Flatpak manifest already has `--socket=wayland` (provides the Wayland compositor connection).
- But the Wine Wayland driver is part of the **host's Wine/Proton build**. CrossHook doesn't run Wine itself.
- Games launch via `flatpak-spawn --host` -> host Wine/Proton.
- Whether the Wine Wayland driver is available depends on the host Wine/Proton build configuration.
- As of Wine 10.0+, the driver is built and enabled by default, so most modern Wine/Proton installations will include it.

#### Gamescope Interaction

There is a feature request (gamescope #1107) for gamescope to support Wine's native Wayland driver instead of running XWayland. This would allow games to render directly to gamescope's Wayland compositor via Wine's Wayland driver, potentially reducing latency.

#### Bundling Verdict

**Cannot bundle independently.** It's part of Wine/Proton. CrossHook's role is to:

1. Detect if the host Wine/Proton has Wayland support.
2. Optionally set `WAYLAND_DISPLAY` or force Wayland mode via registry (`HKCU\Software\Wine\Drivers`).
3. Expose a UI toggle for users who want to force Wine Wayland mode.

**Confidence**: Medium (driver is still maturing; Proton adoption timeline unclear)

---

## 4. Cross-Cutting Permission Analysis

### Current Permission Sufficiency

| Tool           | Needs `--talk-name=org.freedesktop.Flatpak`? | Needs `--filesystem=home`? | Needs `--device=dri`? | Needs `--share=network`? | Additional Needs      |
| -------------- | -------------------------------------------- | -------------------------- | --------------------- | ------------------------ | --------------------- |
| Winetricks     | Yes (host execution)                         | Yes (prefix access)        | No                    | Yes (downloads)          | None beyond current   |
| Winecfg        | Yes (host Wine)                              | Yes (prefix access)        | No                    | No                       | Display passthrough   |
| MangoHud       | Yes (host process)                           | No                         | No                    | No                       | Env var threading     |
| Gamescope      | Yes (host compositor)                        | No                         | Yes (host DRI)        | No                       | DRM master (host)     |
| GameMode       | No (D-Bus portal works)                      | No                         | No                    | No                       | Portal is sufficient  |
| umu-launcher   | Yes (host execution)                         | Yes (umu data)             | No                    | Yes (downloads)          | `xdg-data/umu:create` |
| CachyOS opts   | N/A                                          | N/A                        | N/A                   | N/A                      | Impossible            |
| Proton Manager | Yes (if host tool)                           | Yes (compat tools)         | No                    | Yes (downloads)          | Flatpak Steam write   |
| Wine Wayland   | Yes (host Wine)                              | No                         | Yes (host DRI)        | No                       | `--socket=wayland`    |

### Key Insight

**Every tool except GameMode requires `flatpak-spawn --host` because games run on the host.** The `org.freedesktop.Flatpak` permission is the linchpin of CrossHook's entire architecture. Without it, CrossHook cannot function at all.

GameMode is the sole exception because its XDG Desktop Portal provides proper sandbox-to-host bridging.

---

## 5. Feedback Loops & Failure Modes

### Failure Mode 1: Host Tool Not Found

```
CrossHook -> flatpak-spawn --host which <tool>
    -> exit code 1 (not found)
    -> CrossHook shows "tool not available" in onboarding/UI
    -> User must install on host manually
```

**Impact**: Graceful degradation. CrossHook's `host_command_exists()` handles this.

### Failure Mode 2: flatpak-spawn Permission Denied

```
CrossHook -> flatpak-spawn --host <command>
    -> org.freedesktop.Flatpak not in finish-args
    -> D-Bus error: permission denied
    -> All host commands fail
    -> CrossHook is completely non-functional
```

**Impact**: Total failure. This is why the permission is non-negotiable.

### Failure Mode 3: Environment Variable Leakage

```
CrossHook -> flatpak-spawn --host (without --clear-env)
    -> Sandbox XDG vars leak to host process
    -> Proton/Wine looks for data in ~/.var/app/<id>/...
    -> Prefix creation fails or creates in wrong location
```

**Impact**: Subtle corruption. CrossHook already mitigates this with `--clear-env` + explicit `--env=` args.

### Failure Mode 4: Version Mismatch (Wine/Proton)

```
User has Proton 9.0 prefix
    -> CrossHook bundles winetricks with Wine 8.0
    -> winetricks modifies prefix with wrong Wine
    -> Prefix corruption, game won't launch
```

**Impact**: Data corruption. This is why Wine-dependent tools MUST use the host Wine version.

### Failure Mode 5: umu-launcher Steam Runtime Download Failure

```
umu-run launches
    -> Tries to download SteamLinuxRuntime_sniper
    -> Network fails or disk full
    -> pressure-vessel container can't start
    -> Game launch fails
```

**Impact**: Launch failure. umu-launcher handles this with retries and cached runtimes.

### Failure Mode 6: Gamescope Display Server Mismatch

```
CrossHook launches gamescope on host
    -> Host is Wayland-only, no XWayland
    -> Game expects X11
    -> Gamescope's Xwayland can't start
    -> Black screen or crash
```

**Impact**: Game doesn't display. Gamescope usually handles this by providing its own XWayland instance.

---

## 6. Permission Escalation Paths

### Path 1: flatpak-spawn --host (Current)

```
Sandbox (restricted) -> org.freedesktop.Flatpak D-Bus -> Host (unrestricted user-level)
```

This is the **primary escalation path** and is by design. CrossHook uses it for every tool interaction.

**Security implication**: Any code running inside the CrossHook sandbox can execute arbitrary commands on the host with the user's full permissions. This includes:

- Reading/writing any file the user can access
- Running any program the user can run
- Network access beyond what the sandbox allows
- Process management (kill, signal, etc.)

### Path 2: D-Bus Portal (GameMode)

```
Sandbox -> org.freedesktop.portal.GameMode -> xdg-desktop-portal -> gamemoded
```

This is the **proper sandboxed escalation path**. PID translation happens in the portal. The sandbox app can only register/unregister games, not modify system settings.

### Path 3: Polkit (GameMode CPU governor)

```
gamemoded -> Polkit (com.feralinteractive.GameMode.policy) -> CPU governor change
```

This path is entirely on the host side. CrossHook never participates in this escalation.

### Path 4: DRM/KMS (Gamescope embedded mode)

```
gamescope -> /dev/dri/* -> DRM master -> Direct display control
```

Only relevant for Steam Deck session compositor mode. In nested desktop mode, gamescope uses Wayland client protocol (no DRM master needed).

---

## 7. Bundleability Matrix

| Tool               | Can Bundle?                         | Should Bundle?       | Recommended Approach                    | Risk if Bundled                                   |
| ------------------ | ----------------------------------- | -------------------- | --------------------------------------- | ------------------------------------------------- |
| **Winetricks**     | Theoretically (it's a shell script) | **No**               | Host via `flatpak-spawn`                | Wine version mismatch -> prefix corruption        |
| **Winecfg**        | Only with full Wine                 | **No**               | Host via `flatpak-spawn`                | Massive size, version conflict, prefix corruption |
| **MangoHud**       | Via VulkanLayer extension           | **No**               | Host install, env var passthrough       | Wrong context (sandbox not game process)          |
| **Gamescope**      | Via VulkanLayer extension (hacky)   | **No**               | Host install, wrapper command           | Library conflicts, nested sandbox issues          |
| **GameMode**       | Via client library                  | **Unnecessary**      | D-Bus portal (already works)            | Daemon conflict with host                         |
| **umu-launcher**   | Extremely difficult                 | **No**               | Host install, `flatpak-spawn`           | Python deps explosion, pressure-vessel errors     |
| **CachyOS opts**   | Impossible                          | **No**               | Detection + recommendations only        | N/A                                               |
| **Proton Manager** | As built-in feature                 | **Yes** (as feature) | Implement download/extract in CrossHook | Need rw access for Flatpak Steam                  |
| **Wine Wayland**   | Part of Wine                        | **No**               | Detection + env var toggle              | Can't separate from Wine                          |

### Summary Verdict

**Only one "tool" is viable for integration: Proton version management**, and that's not bundling an external tool -- it's implementing download/extract functionality natively in CrossHook's Rust codebase.

Every other tool either:

1. **Must run on the host** because games run on the host (MangoHud, Gamescope, GameMode)
2. **Is tightly coupled to the host Wine/Proton version** (Winetricks, Winecfg, Wine Wayland)
3. **Operates at the kernel level** (CachyOS optimizations)
4. **Has prohibitive dependency chains** for Flatpak packaging (umu-launcher)

CrossHook's architecture -- launching everything on the host via `flatpak-spawn --host` -- is fundamentally incompatible with tool bundling because the tools need to operate in the same context as the game process, which is the host, not the sandbox.

---

## Sources

### Flatpak & Sandbox

- [Flatpak Sandbox Permissions Documentation](https://docs.flatpak.org/en/latest/sandbox-permissions.html)
- [Flatpak Extensions Documentation](https://docs.flatpak.org/en/latest/extension.html)
- [flatpak-spawn(1) man page](https://man7.org/linux/man-pages/man1/flatpak-spawn.1.html)
- [CVE-2021-21261: Flatpak sandbox escape via spawn portal](https://github.com/flatpak/flatpak/security/advisories/GHSA-4ppf-fxf6-vxg2)
- [Feature request: Warning for org.freedesktop.Flatpak sandbox escape](https://github.com/flatpak/flatpak/issues/5161)

### Winetricks

- [Winetricks source code](https://github.com/Winetricks/winetricks/blob/master/src/winetricks)
- [Winetricks Arch Linux package](https://archlinux.org/packages/extra/x86_64/winetricks/)
- [Lutris Flatpak winetricks bug #6144](https://github.com/lutris/lutris/issues/6144)
- [Wine integration challenges with Flatpak #6160](https://github.com/flatpak/flatpak/issues/6160)

### MangoHud

- [MangoHud GitHub](https://github.com/flightlessmango/MangoHud)
- [org.freedesktop.Platform.VulkanLayer.MangoHud](https://github.com/flathub/org.freedesktop.Platform.VulkanLayer.MangoHud)
- [MangoHud Flatpak portal issue #685](https://github.com/flightlessmango/MangoHud/issues/685)

### Gamescope

- [Gamescope GitHub](https://github.com/ValveSoftware/gamescope)
- [Gamescope ArchWiki](https://wiki.archlinux.org/title/Gamescope)
- [org.freedesktop.Platform.VulkanLayer.gamescope](https://github.com/flathub/org.freedesktop.Platform.VulkanLayer.gamescope)
- [Gamescope Flatpak nested sandbox issues](https://github.com/flathub/org.freedesktop.Platform.VulkanLayer.gamescope/issues/6)

### GameMode

- [GameMode GitHub](https://github.com/FeralInteractive/gamemode)
- [XDG Desktop Portal GameMode documentation](https://flatpak.github.io/xdg-desktop-portal/docs/doc-org.freedesktop.portal.GameMode.html)
- [GameMode 1.4 Flatpak portal PR #146](https://github.com/FeralInteractive/gamemode/pull/146)
- [GameMode Flatpak portal specification](https://github.com/flatpak/xdg-desktop-portal/blob/main/data/org.freedesktop.portal.GameMode.xml)

### umu-launcher

- [umu-launcher GitHub](https://github.com/Open-Wine-Components/umu-launcher)
- [umu-launcher FAQ Wiki](<https://github.com/Open-Wine-Components/umu-launcher/wiki/Frequently-asked-questions-(FAQ)>)
- [Flatpak packaging issue #430](https://github.com/Open-Wine-Components/umu-launcher/issues/430)

### CachyOS

- [CachyOS Kernel documentation](https://wiki.cachyos.org/features/kernel/)
- [sched-ext Tutorial](https://wiki.cachyos.org/configuration/sched-ext/)
- [linux-cachyos GitHub](https://github.com/CachyOS/linux-cachyos)

### ProtonUp-Qt

- [ProtonUp-Qt GitHub](https://github.com/DavidoTek/ProtonUp-Qt)
- [ProtonUp-Qt Flathub page](https://flathub.org/en/apps/net.davidotek.pupgui2)
- [GE-Proton Flatpak extension](https://github.com/flathub/com.valvesoftware.Steam.CompatibilityTool.Proton-GE)

### Wine Wayland

- [Wine 10.0 release with Wayland support (Phoronix)](https://www.phoronix.com/news/Wine-10.0-Released)
- [Wine Wayland driver year in review (Collabora)](https://www.collabora.com/news-and-blog/news-and-events/wine-on-wayland-a-year-in-review-and-a-look-ahead.html)
- [Wine 10.3 clipboard support (GamingOnLinux)](https://www.gamingonlinux.com/2025/03/wine-10-3-released-with-clipboard-support-in-the-wayland-driver-initial-vulkan-video-decoder-support/)
- [Wine 9.22 Wayland default config (GamingOnLinux)](https://www.gamingonlinux.com/2024/11/wine-922-released-noting-the-wayland-driver-enabled-in-default-configuration/)
