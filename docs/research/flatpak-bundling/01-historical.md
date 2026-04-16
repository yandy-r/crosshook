# Historical Analysis: Flatpak Tool Bundling in Linux Gaming Apps

> **Research date**: 2026-04-15
> **Scope**: How Linux gaming launchers and tools have handled bundling vs. host delegation within Flatpak
> **Confidence baseline**: See per-section ratings

---

## Executive Summary

The Linux gaming Flatpak ecosystem has converged on a clear pattern: **launchers bundle their own Wine/Proton runners internally** while relying on **Flatpak extensions and D-Bus portals** for GPU overlays, compositors, and system-level optimizations. No major gaming app successfully uses `flatpak-spawn --host` as its primary tool execution mechanism. The projects that tried per-app Wine bundling (winepak) failed; the projects that built internal runner managers (Bottles, Lutris, Heroic) succeeded. Tools requiring kernel/system-level access (GameMode, MangoHud) work via D-Bus portals or VulkanLayer extensions, not bundled binaries.

---

## 1. Lutris on Flathub

### Approach: Internal Runner Bundling with Extension Dependencies

**Confidence**: High (multiple primary sources: GitHub issues, official FAQ, Flathub repo, forum discussions)

Lutris uses "runners" -- programs that execute games (Wine, DOSBox, MAME, etc.). Its Flatpak bundles Wine runners **inside the sandbox**, downloading them to `~/.var/app/net.lutris.Lutris/data/lutris/runners/wine/`.

**Key decisions:**

- Wine runners are managed internally; Lutris downloads and installs its own builds (e.g., GE-Proton, wine-ge, Lutris custom builds)
- DXVK and VKD3D are auto-managed based on GPU PCI IDs; Lutris ships its own builds and can delay broken upstream releases
- Since v0.5.19, Lutris stopped offering DXVK/VKD3D/D3D Extras on Proton versions, deferring to Proton's built-in copies
- The Flatpak requires i386 compatibility extensions (`org.gnome.Platform.Compat.i386`, `org.freedesktop.Platform.GL32.default`)

**What failed:**

- **Winetricks inside the sandbox**: In v0.5.19, Winetricks fails with `libassuan.so.0` symbol errors (wget) and `LIBJPEGTURBO_6.2` version mismatches (zenity). The Flatpak runtime's library versions conflict with what Winetricks' helper tools expect. ([GitHub #6144](https://github.com/lutris/lutris/issues/6144))
- **`flatpak-spawn --host` from Lutris**: Users attempted to run host Wine via `flatpak-spawn --host` within Lutris, but encountered `Portal call failed: org.freedesktop.DBus.Error.ServiceUnknown` errors when launched from Lutris's game pipeline (despite working from a direct shell session in the sandbox). The portal service communication path breaks when invoked through Lutris's subprocess chain. ([GitHub #274](https://github.com/flathub/net.lutris.Lutris/issues/274))
- **Missing extensions**: Users frequently encounter "GL compat extension is missing, wine apps won't work properly" when i386 extensions aren't auto-installed. ([GitHub #191](https://github.com/flathub/net.lutris.Lutris/issues/191))
- **Host Wine not visible**: The Flatpak sandbox cannot access `/usr/bin` or host-installed Wine versions, forcing reliance on internally managed runners

**Lesson**: Internal runner bundling works for the primary Wine/Proton execution path, but ancillary tools (Winetricks, host Wine discovery) break at the sandbox boundary. The Lutris FAQ still recommends native install over Flatpak for full functionality.

**Sources:**

- [Lutris FAQ](https://lutris.net/faq)
- [flathub/net.lutris.Lutris](https://github.com/flathub/net.lutris.Lutris)
- [Lutris Flatpak Winetricks failure #6144](https://github.com/lutris/lutris/issues/6144)
- [flatpak-spawn from Lutris #274](https://github.com/flathub/net.lutris.Lutris/issues/274)
- [GamingOnLinux: Lutris Flatpak Beta](https://www.gamingonlinux.com/2022/04/lutris-now-has-a-flatpak-beta-available-and-updated-for-the-steam-deck/)
- [Lutris Flatpak Wine runner issues #191](https://github.com/flathub/net.lutris.Lutris/issues/191)

---

## 2. Bottles on Flathub

### Approach: Flatpak-Only, Fully Self-Contained

**Confidence**: High (official docs, GitHub, release notes, 3M+ Flathub installs)

Bottles is the strongest example of the "Flatpak-first" philosophy in Linux gaming. It is distributed **exclusively** as a Flatpak -- no APT, RPM, Snap, or other packages are officially supported.

**Architecture:**

- **Everything inside the sandbox**: Wine runners (Soda, Caffe, Vaniglia, Proton-GE), DXVK, VKD3D, winetricks components, and dependency managers are all bundled within or downloaded into the Flatpak data directory (`~/.var/app/com.usebottles.bottles/`)
- **Runner management**: Bottles maintains its own runner repository. Soda (based on Valve's Wine fork + Proton/TKG/GE patches) is the primary gaming runner; Caffe (wine-tkg-git based) and Vaniglia (vanilla Wine + staging) are alternatives
- **Per-bottle isolation**: Each "bottle" is a separate Wine prefix with its own runner selection, DLL overrides, and environment variables
- **Sandboxing uses `flatpak-spawn`**: When running as Flatpak, Bottles uses `flatpak-spawn` for sandbox isolation; for non-Flatpak installs, it uses bubblewrap directly
- **SDK dependencies**: `org.gnome.Sdk`, `org.gnome.Sdk.Compat.i386`, `org.freedesktop.Sdk.Extension.toolchain-i386`

**Extension handling (v63.0+):**

- Bottles v63.0 added "robust availability checks for Flatpak extensions (Gamescope, MangoHud, OBS)" before attempting to use them ([Bottles 63.0 release](https://www.linuxcompatible.org/story/bottles-630-released))
- Prior to this, MangoHud/Gamescope/OBS toggles were greyed out or silently failed when the corresponding Flatpak extensions weren't installed -- a long-standing UX problem ([GitHub #2801](https://github.com/bottlesdevs/Bottles/issues/2801), [#2450](https://github.com/bottlesdevs/Bottles/issues/2450), [#2008](https://github.com/bottlesdevs/Bottles/issues/2008))

**GameMode handling:**

- GameMode libraries **are bundled** with Bottles, but host installation is still required for full system-wide optimizations (CPU governor, process niceness). The bundled library can communicate with the host daemon via D-Bus portal. ([Discussion #3635](https://github.com/orgs/bottlesdevs/discussions/3635))

**What worked:**

- Consistent cross-distro experience; 3M+ installations on Flathub
- Full isolation per bottle
- NLnet Foundation 2025 Commons Fund grant for "Bottles Next"

**What caused friction:**

- Flatpak-only stance frustrates users on distros where Flatpak isn't default
- Native Wine installed on host isn't visible to Bottles Flatpak ([GitHub #3379](https://github.com/bottlesdevs/Bottles/issues/3379))
- GPU driver path issues in some Flatpak sandbox configurations

**Sources:**

- [Bottles Installation Docs](https://docs.usebottles.com/getting-started/installation)
- [Bottles Runners Docs](https://docs.usebottles.com/components/runners)
- [Bottles GitHub](https://github.com/bottlesdevs/Bottles)
- [Bottles Flathub](https://flathub.org/en/apps/com.usebottles.bottles)
- [Bottles 63.0 Release](https://www.linuxcompatible.org/story/bottles-630-released)
- [MangoHud/OBS greyed out #2450](https://github.com/bottlesdevs/Bottles/issues/2450)
- [GamingOnLinux: Bottles default runner based on Valve's Wine fork](https://www.gamingonlinux.com/2022/07/wine-manager-bottles-default-runner-now-based-on-valves-wine-fork-and-proton/)

---

## 3. Heroic Games Launcher

### Approach: Internal Wine Manager with Flatpak as Primary Channel

**Confidence**: High (official wiki, GitHub, Flathub listing)

Heroic is an Electron-based launcher for Epic, GOG, and Amazon games. Flathub is the recommended distribution method, described as "ideal" because "it is a more reproducible and controllable way for everyone to test."

**Key design:**

- **Built-in Wine Manager**: Downloads Wine-GE, Proton-GE, Wine-Lutris, and other runners directly into `~/.config/heroic/tools/wine` (or `~/.config/heroic/tools/proton`). Uses [heroic-wine-downloader](https://github.com/Heroic-Games-Launcher/heroic-wine-downloader), a dedicated Node.js library for fetching runner builds
- **No Flatpak Wine support**: "Heroic will not find or use Wine and Proton Flatpak versions" -- the Wine Manager is the only supported mechanism in Flatpak mode
- **Steam Proton discovery**: Can find Proton from Steam's `compatibilitytools.d` (both native and Flatpak Steam) if the library is in `$HOME`
- **Cannot access `/usr/bin`**: Host-installed Wine/Proton is invisible from the Flatpak sandbox
- **Winetricks/WineCFG via internal runner**: Heroic runs Winetricks and WineCFG using its own managed Wine binary within the correct prefix -- no need for external protontricks

**Runtime version decisions (2024-2025):**

- Flatpak reverted to Runtime 23.08 for stable due to gamepad input issues on Steam Deck Gaming Mode
- Runtime 24.08 (with HDR/Wayland support) available only on the Beta Flathub branch
- HDR requires the Gamescope Flatpak extension, and the Flatpak Gamescope version must match the natively installed version -- AppImage avoids this issue entirely

**Lesson**: Heroic demonstrates that an Electron app can successfully bundle its own runner management inside Flatpak, but runtime version pinning creates a tension between compatibility (Steam Deck) and features (HDR/Wayland).

**Sources:**

- [Heroic Wiki: Wine and Proton](https://github.com/Heroic-Games-Launcher/HeroicGamesLauncher/wiki/How-To:-Wine-and-Proton)
- [Heroic Wiki: Linux Quick Start](https://github.com/Heroic-Games-Launcher/HeroicGamesLauncher/wiki/Linux-Quick-Start-Guide)
- [Heroic Wiki: Steam Deck](https://github.com/Heroic-Games-Launcher/HeroicGamesLauncher/wiki/Steam-Deck)
- [Heroic Flathub](https://flathub.org/en/apps/com.heroicgameslauncher.hgl)
- [heroic-wine-downloader](https://github.com/Heroic-Games-Launcher/heroic-wine-downloader)

---

## 4. ProtonUp-Qt

### Approach: Pure Download Manager, Agnostic to Packaging

**Confidence**: Medium (GitHub, Flathub listing; limited architectural discussion found)

ProtonUp-Qt is a runner version manager (not a launcher). It downloads and installs Proton-GE, Wine-GE, DXVK, VKD3D, and other compatibility tools into the correct directories for Steam, Lutris, Heroic, and other launchers.

**Key design:**

- Supports native, Snap, and Flatpak installations of target apps (Steam, Lutris)
- Distributed as Flatpak on Flathub and AppImage on GitHub
- Does not bundle runners itself -- it downloads them from upstream sources and places them in the correct filesystem locations
- Works within the Flatpak sandbox by writing to accessible directories within `$HOME`

**Relevance to CrossHook**: ProtonUp-Qt validates the pattern where a Flatpak app manages downloadable tools by writing to user data directories (`~/.local/share/` or `~/.var/app/`). It does not need `flatpak-spawn --host` because it writes files rather than executing host binaries.

**Sources:**

- [ProtonUp-Qt GitHub](https://github.com/DavidoTek/ProtonUp-Qt)
- [ProtonUp-Qt Flathub](https://flathub.org/en/apps/net.davidotek.pupgui2)
- [ProtonUp-Qt Website](https://davidotek.github.io/protonup-qt/)

---

## 5. Steam Flatpak (Valve's Approach)

### Approach: Container-in-a-Container (pressure-vessel)

**Confidence**: High (Valve's official GitLab/GitHub repos, extensive documentation)

The Steam Flatpak is architecturally unique: it runs the Steam client inside a Flatpak sandbox, and then runs each game inside a _nested_ container called **pressure-vessel** (marketed as "Steam Linux Runtime").

**Architecture:**

- **pressure-vessel** is "a simple version of Flatpak made for Steam games" -- each game gets its own container based on a specific Steam Runtime version (scout/soldier/sniper/medic)
- Old games run in old runtimes; new games run in new runtimes; this solves ABI compatibility across decades of Linux gaming
- Takes some host libraries (GPU drivers) and mounts them into the container
- Proton runs inside pressure-vessel, providing Wine translation
- The entire Steam client lives in `~/.var/app/com.valvesoftware.Steam/`

**Permissions model:**

- Steam Flatpak has `filesystem=host` equivalent permissions for game library access, effectively reducing sandbox security
- Adding `home` filesystem override causes Steam to refuse to launch (safety check)
- External drive libraries require parent-directory access for Steam to read disk space correctly
- Sub-sandboxing for individual games requires shared `/tmp` and process IDs between Steam client and game

**2024 breakage -- Vulkan Layers in pressure-vessel:**

- Around March 2024, MangoHud and VkBasalt stopped working in Steam Flatpak due to a pressure-vessel update. The overrides data directory mounted inside the container (`/usr/lib/pressure-vessel/overrides/share`) wasn't included in `XDG_DATA_DIRS`, so Vulkan-Loader couldn't find the layers. ([GitHub #662](https://github.com/ValveSoftware/steam-runtime/issues/662))

**Lesson**: Valve's approach works because they control the entire stack (client + runtime + container + Proton). The pressure-vessel model is not replicable by third-party apps that don't control their own container runtime.

**Sources:**

- [pressure-vessel docs (GitLab)](https://gitlab.steamos.cloud/steamrt/steam-runtime-tools/-/blob/main/pressure-vessel/wrap.1.md)
- [Valve steam-runtime (GitHub)](https://github.com/ValveSoftware/steam-runtime)
- [Vulkan Layers broken in pressure-vessel #662](https://github.com/ValveSoftware/steam-runtime/issues/662)
- [flathub/com.valvesoftware.Steam Wiki](https://github.com/flathub/com.valvesoftware.Steam/wiki)
- [GamingOnLinux: Valve pressure-vessel source on GitLab](https://www.gamingonlinux.com/2020/10/valve-put-their-pressure-vessel-container-source-for-linux-games-up-on-gitlab/)
- [Flatpak sub-sandboxing discussion #3797](https://github.com/flatpak/flatpak/issues/3797)

---

## 6. MangoHud

### Approach: Flatpak VulkanLayer Extension

**Confidence**: High (official Flathub repo, working branches for 21.08-25.08)

MangoHud is distributed as `org.freedesktop.Platform.VulkanLayer.MangoHud` -- a Flatpak extension that plugs into the `VulkanLayer` mount point at `/usr/lib/extensions/vulkan/MangoHud/`.

**How it works:**

- The Freedesktop runtime has an extension mount point that bind-mounts any `VulkanLayer` extension into the app's filesystem
- MangoHud injects itself as a Vulkan layer via `LD_PRELOAD` of `libMangoHud.so`
- Apps enable it via environment variable: `MANGOHUD=1`
- Config files require explicit filesystem access: `flatpak override --user --filesystem=xdg-config/MangoHud:ro`

**Active branches**: 21.08, 22.08, 23.08, 24.08, 25.08 -- one per Flatpak runtime generation

**Migration history**: MangoHud was previously available as `com.valvesoftware.Steam.Utility.MangoHud` (Steam-specific). This was marked end-of-life in favor of the `org.freedesktop.Platform.VulkanLayer` version, which works with **any** Flatpak app (Steam, Lutris, Bottles, Heroic, etc.).

**Known issue**: MangoHud/VkBasalt broken in Steam Flatpak circa March 2024 due to pressure-vessel path not in `XDG_DATA_DIRS` (see Steam section above).

**Sources:**

- [flathub/org.freedesktop.Platform.VulkanLayer.MangoHud](https://github.com/flathub/org.freedesktop.Platform.VulkanLayer.MangoHud)
- [MangoHud GitHub](https://github.com/flightlessmango/MangoHud)
- [MangoHud Flathub PR #1371](https://github.com/flathub/flathub/pull/1371)

---

## 7. GameMode

### Approach: D-Bus Portal (Not Bundled Binary)

**Confidence**: High (official XDG portal spec, Feral Interactive releases, GNOME integration docs)

GameMode is the canonical example of using D-Bus portals instead of bundled binaries in Flatpak.

**The problem**: GameMode's client library calls `RegisterGame(getpid())`. In Flatpak's PID namespace isolation, `getpid()` returns the container PID, which is meaningless to the host-side `gamemoded` daemon.

**The solution**: `org.freedesktop.portal.GameMode` -- an XDG Desktop Portal that:

1. Accepts `RegisterGame` calls from sandboxed apps
2. Translates the container PID to the host PID
3. Proxies the request to the host `gamemoded` daemon
4. Auto-cleans up if the client terminates without unregistering

**Portal API (version 4):**

- `QueryStatus(pid)`, `RegisterGame(pid)`, `UnregisterGame(pid)` -- basic PID-based
- `*ByPid(target, requester)` -- proxy variants
- `*ByPIDFd(target_pidfd, requester_pidfd)` -- modern pidfd variants
- `Active` property -- boolean, readable, indicates if GameMode is active system-wide

**Key release**: GameMode 1.4 added Flatpak support via the portal, switched client library from sd-bus to libdbus, and added automatic Flatpak detection in the client library.

**For CrossHook**: GameMode integration from a Flatpak **must** use the D-Bus portal, not a bundled `gamemoderun` binary. The client library auto-detects Flatpak and uses the portal transparently. Bundling `gamemoderun` would fail due to PID namespace mismatch.

**Sources:**

- [XDG Desktop Portal: GameMode](https://flatpak.github.io/xdg-desktop-portal/docs/doc-org.freedesktop.portal.GameMode.html)
- [Feral GameMode 1.4 (Phoronix)](https://www.phoronix.com/news/Feral-GameMode-1.4-Released)
- [GameMode GitHub](https://github.com/FeralInteractive/gamemode)
- [Christian Kellner: GameMode improvements for GNOME 3.34](https://christian.kellner.me/2019/09/25/gamemode-improvements-for-gnome-3-34-and-fedora-31/)
- [Steam Flatpak GameMode support #77](https://github.com/flathub/com.valvesoftware.Steam/issues/77)
- [GameMode ArchWiki](https://wiki.archlinux.org/title/GameMode)

---

## 8. Winetricks

### Approach: Bundled (Poorly) or Host-Delegated

**Confidence**: Medium (multiple issue reports, limited official documentation on Flatpak strategy)

Winetricks has no official Flatpak packaging strategy. It is a shell script with deep dependencies on host tools (wget, zenity/kdialog, cabextract, p7zip, etc.).

**How launchers handle it:**

- **Lutris Flatpak**: Bundles Winetricks inside the sandbox, but it fails due to library conflicts between the Flatpak runtime's versions and what Winetricks' helper tools expect ([GitHub #6144](https://github.com/lutris/lutris/issues/6144))
- **Heroic**: Runs Winetricks using its own managed Wine binary within the correct prefix -- avoids the host dependency issue by using the runner it already manages
- **Bottles**: Has its own built-in dependency installer that replaces Winetricks functionality for common Windows redistributables
- **flatpak-wine**: Simply bundles Wine + Winetricks in a single Flatpak ([flatpak-wine](https://github.com/fastrizwaan/flatpak-wine))

**Core challenge**: Winetricks' design assumes unrestricted access to the host filesystem, network (wget), and GUI toolkit (zenity). These assumptions break in a sandboxed environment.

**Sources:**

- [Lutris Winetricks failure #6144](https://github.com/lutris/lutris/issues/6144)
- [Wine integration challenges with Flatpak #6160](https://github.com/flatpak/flatpak/issues/6160)
- [flatpak-wine project](https://github.com/fastrizwaan/flatpak-wine)

---

## 9. Gamescope (Flatpak Extension)

### Approach: VulkanLayer Extension (Acknowledged Hack)

**Confidence**: Medium (official repo acknowledges the approach is a hack; limited Flathub policy documentation)

Gamescope is packaged as `org.freedesktop.Platform.VulkanLayer.gamescope`, abusing the VulkanLayer mount point to inject a microcompositor.

**Why it's a hack:**

- The `VulkanLayer` extension point was designed for Vulkan layers (MangoHud, vkBasalt), not window managers
- Gamescope introduces shared libraries (libevdev, libinput, libliftoff, libseat, libwlroots, libxcvt, etc.) that can conflict with app-provided libraries
- Does not work with Proton's nested sandbox (Proton 5.13+)
- The original author explicitly stated: "I'm very much against [submitting to Flathub] due to the current limitations of runtime extensions"

**Migration history:**

- Originally packaged as `com.valvesoftware.Steam.Utility.gamescope` (Steam-only)
- Migrated to `org.freedesktop.Platform.VulkanLayer.gamescope` after requests from Bottles and Lutris developers who also needed Gamescope support ([GitHub #59](https://github.com/flathub/com.valvesoftware.Steam.Utility.gamescope/issues/59))
- The Steam.Utility version was archived March 19, 2024

**For CrossHook**: Gamescope-as-extension exists but is fragile. If CrossHook needs Gamescope, host delegation (via `flatpak-spawn --host` or portal) may be more reliable than depending on the extension, especially for features requiring matching host/extension versions.

**Sources:**

- [flathub/org.freedesktop.Platform.VulkanLayer.gamescope](https://github.com/flathub/org.freedesktop.Platform.VulkanLayer.gamescope)
- [tinywrkb GameScope extension](https://github.com/tinywrkb/org.freedesktop.Platform.VulkanLayer.GameScope)
- [Gamescope packaging request #59](https://github.com/flathub/com.valvesoftware.Steam.Utility.gamescope/issues/59)
- [Gamescope ArchWiki](https://wiki.archlinux.org/title/Gamescope)

---

## 10. umu-launcher

### Approach: Standalone Flatpak with Internal Proton/Runtime Management

**Confidence**: Medium (GitHub repo, packaging directory, wiki; limited Flathub submission history)

umu-launcher reproduces Steam's runtime container environment outside Steam, allowing any launcher to run games via Proton identically to Steam.

**Flatpak design:**

- App ID: `org.openwinecomponents.umu.umu-launcher`
- Builds via `flatpak-builder` with Flathub dependencies
- Configuration via environment variables: `GAMEID`, `WINEPREFIX`, `PROTONPATH`
- Auto-downloads the required Steam Runtime to `$HOME/.local/share/umu`
- Only searches `$HOME/.local/share/Steam/compatibilitytools.d` for Proton builds
- Does **not** search system paths -- intentionally restricts to SLR-compiled builds

**Integration**: Lutris, Heroic, and Faugus Launcher already support umu-launcher. Games get protonfixes from a shared database keyed by `GAMEID`.

**Sources:**

- [umu-launcher GitHub](https://github.com/Open-Wine-Components/umu-launcher)
- [umu-launcher Flatpak packaging](https://github.com/Open-Wine-Components/umu-launcher/tree/main/packaging/flatpak)
- [umu-launcher FAQ](<https://github.com/Open-Wine-Components/umu-launcher/wiki/Frequently-asked-questions-(FAQ)>)
- [GamingOnLinux: UMU v1.1.3](https://www.gamingonlinux.com/2024/10/unified-launcher-for-windows-games-on-linux-umu-v113-out-now/)

---

## 11. Winepak (Discontinued)

### Approach: Per-App Wine Bundling in Flatpak (Failed)

**Confidence**: High (GitHub issues document the failure clearly)

Winepak attempted to create individual Flatpak packages for each Windows application, each bundling Wine and all dependencies.

**Why it failed:**

1. **Didn't scale**: Creating a separate Flatpak for each Windows app was "a lot of work and doesn't scale" ([GitHub #17](https://github.com/winepak/winepak/issues/17))
2. **Build times**: 20 minutes to hours per bundle
3. **Massive size**: ~2.5 GB base runtime before any application
4. **Hosting costs**: Required cheap/unlimited egress bandwidth
5. **Architectural mistakes**: Using Wine-staging as extensions was "a gross abuse of extensions"; users could uninstall them, causing crashes
6. **Solo maintainer**: Project died when the single maintainer couldn't sustain the workload
7. **Last commit**: June 2018; fork (finepak) created but also appears inactive

**Lesson**: Per-app bundling of Wine is economically and technically unsustainable. The ecosystem moved to launcher-managed runners (Lutris/Bottles/Heroic) and unified runtimes (umu-launcher/pressure-vessel) instead.

**Sources:**

- [winepak GitHub](https://github.com/winepak/winepak)
- [winepak website](https://winepak.github.io/)
- [Generic winepak proposal #17](https://github.com/winepak/winepak/issues/17)
- [winepak abandonment #143](https://github.com/winepak/applications/issues/143)
- [finepak fork](https://github.com/finepak/finepak)

---

## 12. Flatpak Extension Architecture for Gaming

### Two Extension Point Families

**Confidence**: High (Flatpak docs, Flathub repos, migration PRs)

| Extension Point                               | Scope                                      | Mount Path                           | Examples                             |
| --------------------------------------------- | ------------------------------------------ | ------------------------------------ | ------------------------------------ |
| `org.freedesktop.Platform.VulkanLayer.*`      | All Flatpak apps using Freedesktop runtime | `/usr/lib/extensions/vulkan/<name>/` | MangoHud, vkBasalt, gamescope        |
| `com.valvesoftware.Steam.Utility.*`           | Steam Flatpak only                         | Steam-specific paths                 | gamescope (archived), MangoHud (EOL) |
| `com.valvesoftware.Steam.CompatibilityTool.*` | Steam Flatpak only                         | Steam compat tool paths              | Proton, Proton-GE                    |

**Trend**: Clear migration from Steam-specific extensions to `org.freedesktop.Platform.VulkanLayer.*`. Bottles and Lutris drove this -- they couldn't use `Steam.Utility` extensions due to runtime version mismatches that caused crashes.

**Auto-install gap**: Extensions are not auto-installed when an app is installed. Lutris, Bottles, and others have had to add explicit detection and user guidance for missing extensions ([Lutris #5898](https://github.com/lutris/lutris/issues/5898), [Bottles #2008](https://github.com/bottlesdevs/Bottles/issues/2008), [Flathub Discourse](https://discourse.flathub.org/t/how-to-autoinstall-platform-extension-e-g-gamescope/4699)).

---

## 13. Flathub Policy: `flatpak-spawn --host` and Sandbox Escape

### The Gatekeeper Rule

**Confidence**: High (official Flathub linter documentation)

Flathub's linter enforces strict rules around sandbox-escaping permissions:

- `--talk-name=org.freedesktop.Flatpak` (which enables `flatpak-spawn --host`) is **restricted and granted on a case-by-case basis**
- "This must not be used unless absolutely necessary and when no existing solutions using Flatpak or portals exist" ([Flathub Linter Docs](https://docs.flathub.org/docs/for-app-authors/linter))
- Wildcard talk-names (`org.freedesktop.*`, `org.gnome.*`) are flagged as security issues

**Security context (April 2026):**

- CVE-2026-34078: Critical sandbox escape via symlink in `sandbox-expose` paths -- every Flatpak app could read/write arbitrary host files. Fixed in Flatpak 1.16.4. ([GitHub Advisory](https://github.com/flatpak/flatpak/security/advisories/GHSA-cc2q-qc34-jprg))
- CVE-2021-21261: Earlier sandbox escape via spawn portal environment variables ([GitHub Advisory](https://github.com/flatpak/flatpak/security/advisories/GHSA-4ppf-fxf6-vxg2))
- ~42% of Flatpak apps on Flathub override or misconfigure sandboxing, resulting in overprivilege ([flatkill.org](https://flatkill.org/), [Linux Journal analysis](https://www.linuxjournal.com/content/when-flatpaks-sandbox-cracks-real-life-security-issues-beyond-ideal))

**Implication for CrossHook**: If CrossHook currently uses `flatpak-spawn --host` for all tool execution, submitting to Flathub would require justifying this permission. Flathub reviewers will push back on blanket host execution access. The trend is toward portals and extensions, not host delegation.

**Sources:**

- [Flathub Linter Docs](https://docs.flathub.org/docs/for-app-authors/linter)
- [CVE-2026-34078](https://github.com/flatpak/flatpak/security/advisories/GHSA-cc2q-qc34-jprg)
- [CVE-2021-21261](https://github.com/flatpak/flatpak/security/advisories/GHSA-4ppf-fxf6-vxg2)
- [Flatpak 1.16.4 (Phoronix)](https://www.phoronix.com/news/Flatpak-1.16.4-Released)

---

## Cross-Cutting Patterns

### What Works

| Pattern                                                      | Used By                              | Why It Works                                                            |
| ------------------------------------------------------------ | ------------------------------------ | ----------------------------------------------------------------------- |
| Internal runner manager (download Wine/Proton into app data) | Lutris, Bottles, Heroic              | Self-contained; no host dependency; works on immutable distros          |
| D-Bus portal for system services                             | GameMode, (future: power management) | PID namespace translation; Flathub-approved; host daemon does real work |
| VulkanLayer extensions for GPU tools                         | MangoHud, vkBasalt, gamescope        | Cross-app compatible; versioned per runtime; Flathub-supported          |
| Explicit extension availability checks                       | Bottles v63+, Lutris                 | UX clarity when optional extensions are missing                         |

### What Fails

| Pattern                                            | Attempted By                                | Why It Fails                                               |
| -------------------------------------------------- | ------------------------------------------- | ---------------------------------------------------------- |
| Per-app Wine bundling                              | Winepak                                     | Doesn't scale; enormous size; unsustainable maintenance    |
| `flatpak-spawn --host` from subprocess chains      | Lutris (users)                              | DBus portal errors when invoked from deep subprocess trees |
| Bundled Winetricks in sandbox                      | Lutris Flatpak                              | Library version conflicts between runtime and helper tools |
| Steam-specific extension points for non-Steam apps | Bottles, Lutris (attempted `Steam.Utility`) | Runtime version mismatches cause crashes                   |
| Assuming host Wine/tools are visible               | All Flatpak gaming apps                     | `/usr/bin` is invisible; sandbox boundary is absolute      |

### The Emerging Stack

```
Layer 4: App UI (Lutris/Bottles/Heroic/CrossHook)
  |
Layer 3: Runner Manager (download + manage Wine/Proton builds)
  |         |
  |         +-- umu-launcher (unified protonfixes + Steam Runtime)
  |
Layer 2: Extensions (MangoHud, gamescope, vkBasalt via VulkanLayer)
  |
Layer 1: Portals (GameMode, FileChooser, Screenshot, etc.)
  |
Layer 0: Host System (GPU drivers, kernel, gamemoded, gamescope-host)
```

The trend is clear: gaming launchers are moving **up the stack** -- bundling runner management (Layer 3) while delegating system integration **down** to portals (Layer 1) and extensions (Layer 2). Host delegation via `flatpak-spawn --host` is increasingly seen as a legacy pattern that Flathub will resist.
