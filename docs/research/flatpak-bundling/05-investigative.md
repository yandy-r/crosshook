# Investigative Report: Current State of the Flatpak Gaming Ecosystem (2025-2026)

> **Perspective**: Investigative Journalist
> **Date**: April 2026
> **Scope**: Flathub policies, tool availability, portal APIs, Wine Wayland, immutable distros, community sentiment

---

## Executive Summary

The Flatpak gaming ecosystem in 2025-2026 is undergoing rapid maturation. Flathub surpassed 438 million downloads in 2025, and Valve's SteamOS has cemented Flatpak as the de facto software distribution method for gaming-adjacent Linux distributions. However, a fundamental tension persists: gaming applications like CrossHook need host system access (launching Proton, interacting with system tools) that conflicts with Flatpak's sandboxing model. The `--talk-name=org.freedesktop.Flatpak` permission -- CrossHook's primary mechanism for host execution via `flatpak-spawn --host` -- is explicitly discouraged by Flathub and can trigger app rejections. The Lutris Flatpak is the key precedent: it does declare this permission and is accepted on Flathub, establishing that gaming launchers with legitimate host-execution needs can pass review with sufficient justification.

**Confidence**: High -- based on direct manifest analysis, Flathub documentation, and multiple corroborating sources.

---

## 1. Flathub Policies & Review Process

### 1.1 Current Permission Model

Flathub's official requirements mandate minimal static permissions:

> "Static permissions must be kept to an absolute minimum. Applications should rely on XDG Portals and follow established XDG standards wherever possible."

Key policy points ([source](https://docs.flathub.org/docs/for-app-authors/requirements)):

- All broad static permissions must be **justified by the submitter** during review
- Reviewers check the manifest, request changes/rationale, or **reject entirely**
- Permissions are **re-reviewed when they change** -- updates get blocked until review completes
- `--socket=system-bus` and `--socket=session-bus` are restricted to development tools
- Applications relying on "host components or complicated post-installation setups for core functionality will not be accepted" (case-by-case exceptions)

**Confidence**: High -- directly from [Flathub official documentation](https://docs.flathub.org/docs/for-app-authors/requirements) and [Flathub safety blog](https://docs.flathub.org/blog/app-safety-layered-approach-source-to-user) (February 2025).

### 1.2 The `--talk-name=org.freedesktop.Flatpak` Question

This permission enables `flatpak-spawn --host`, allowing **arbitrary code execution** on the host system. It is described as "possibly the broadest permission we have" by Flatpak developers.

**Current Flathub stance**: The permission is **discouraged** and is "the cause of potential rejections in publication" ([flatpak/flatpak#5538](https://github.com/flatpak/flatpak/issues/5538)). There is a feature request to add fine-grained command-level filtering to `flatpak-spawn`, but it has **not been implemented** as of April 2026.

**What this means for CrossHook**: The app will face heightened scrutiny during Flathub review. The permission must be justified with a clear explanation of why portals are insufficient and what host commands are needed.

**Confidence**: High -- corroborated by multiple Flatpak GitHub issues and Flathub documentation.

### 1.3 Flathub Review Depth

Once accepted, apps have more leeway -- but there's an open issue ([flathub/flathub#1337](https://github.com/flathub/flathub/issues/1337)) proposing automatic re-review when apps add new sandbox holes. Currently, post-acceptance permission changes are caught because Flathub **blocks updates when permissions change** until manual review occurs.

A study found ~42% of Flatpak apps either override or misconfigure sandboxing ([Linux Journal, 2025](https://www.linuxjournal.com/content/when-flatpaks-sandbox-cracks-real-life-security-issues-beyond-ideal)). This context means Flathub is under pressure to tighten review, not loosen it.

**Confidence**: Medium -- the 42% figure comes from a single study; the re-review blocking is confirmed by Flathub docs.

### 1.4 Wine/Gaming-Specific Policies

Flathub explicitly states:

> "Windows software submissions using Wine or any submissions that aren't native to the Linux desktop... will only be accepted if they are submitted officially by upstream with the intention of maintaining it in an official capacity."

CrossHook is a **native Linux app** that orchestrates Wine/Proton game launches -- it is NOT a Wine wrapper itself. This distinction is favorable for Flathub acceptance.

**Confidence**: High -- direct from Flathub requirements documentation.

### 1.5 Recent Security Context: CVE-2026-34078

One week before this report (early April 2026), a **critical sandbox escape vulnerability** (CVE-2026-34078) was disclosed: every Flatpak app could read/write arbitrary host files and execute code via symlink exploitation in portal `sandbox-expose` options. Patched in Flatpak 1.16.4 ([Phoronix](https://www.phoronix.com/news/Flatpak-1.16.4-Released), [GitHub Advisory](https://github.com/flatpak/flatpak/security/advisories/GHSA-cc2q-qc34-jprg)).

**Implication**: This fresh CVE will likely make Flathub reviewers **more cautious** about sandbox-escaping permissions in the near term.

**Confidence**: High -- directly from Flatpak security advisory.

---

## 2. Gaming App Precedents on Flathub

### 2.1 Lutris (`net.lutris.Lutris`) -- KEY PRECEDENT

Lutris is the most directly relevant precedent for CrossHook. Its current Flatpak manifest ([source](https://github.com/flathub/net.lutris.Lutris)) includes:

| Permission                            | Status                                     |
| ------------------------------------- | ------------------------------------------ |
| `--talk-name=org.freedesktop.Flatpak` | **YES -- declared in manifest**            |
| `--filesystem=home`                   | Yes                                        |
| `--filesystem=/run/media`             | Yes                                        |
| `--socket=x11`, `--socket=wayland`    | Yes                                        |
| `--socket=pulseaudio`                 | Yes                                        |
| `--device=all`                        | Yes                                        |
| `--allow=devel`, `--allow=multiarch`  | Yes                                        |
| `--share=network`, `--share=ipc`      | Yes                                        |
| MangoHud config access                | Yes (`xdg-config/MangoHud:ro`)             |
| Steam directory access                | Yes (`~/.var/app/com.valvesoftware.Steam`) |
| Gamescope HDR support                 | Yes (env vars + filesystem)                |
| UMU launcher runtime                  | Yes                                        |

**Key finding**: Lutris **does** declare `--talk-name=org.freedesktop.Flatpak` in its official Flathub manifest, and it is accepted. This establishes that gaming launchers with host-execution needs CAN pass Flathub review.

Additional D-Bus permissions: `org.gnome.Mutter.DisplayConfig`, `org.freedesktop.ScreenSaver`, `org.kde.StatusNotifierWatcher`, `org.freedesktop.UDisks2` (system).

**Confidence**: High -- directly verified from the Flathub manifest repository.

### 2.2 Bottles (`com.usebottles.bottles`)

Bottles takes the **opposite approach** -- it does NOT request `--talk-name=org.freedesktop.Flatpak`:

| Permission                                   | Status             |
| -------------------------------------------- | ------------------ |
| `--talk-name=org.freedesktop.Flatpak`        | **NO**             |
| `--filesystem=host` or `--filesystem=home`   | **NO**             |
| `--allow=devel`, `--allow=multiarch`         | Yes                |
| `--device=all`                               | Yes                |
| `--socket=x11`, `--socket=wayland`           | Yes                |
| `--system-talk-name=org.freedesktop.UDisks2` | Yes                |
| Vulkan extension paths (MangoHud, gamescope) | Yes (via PATH env) |

Bottles uses portals for file access and bundles Wine within the Flatpak. It previously had 12+ filesystem permissions but reduced them to rely on portals ([flathub/com.usebottles.bottles#120](https://github.com/flathub/com.usebottles.bottles/issues/120)).

**Implication**: Bottles proves you CAN build a Wine-based gaming app without `org.freedesktop.Flatpak`, but only because it bundles Wine inside the sandbox. CrossHook's architecture (delegating to host Proton/Wine) makes this approach difficult to replicate.

**Confidence**: High -- directly verified from manifest.

### 2.3 Heroic Games Launcher (`com.heroicgameslauncher.hgl`)

Heroic does **NOT** include `--talk-name=org.freedesktop.Flatpak` in its default manifest. Users who need host execution must add it manually via `flatpak override` or Flatseal. The Heroic wiki documents this as an optional user step for advanced scenarios (running other Flatpaks, network isolation per-game).

**Confidence**: High -- confirmed via multiple GitHub issues and wiki documentation.

### 2.4 Steam (`com.valvesoftware.Steam`)

The Steam Flatpak is a special case. Steam is NOT a Flatpak on the Steam Deck itself (it's baked into SteamOS). On other distros, the Flatpak Steam client has broad permissions including device access, network, multiarch, and Vulkan layer extensions. It uses its own nested sandbox for Proton.

**Confidence**: High -- multiple sources including [Steam Flatpak GitHub](https://github.com/flathub/com.valvesoftware.Steam) and [SteamOS FAQ](https://partner.steamgames.com/doc/steamdeck/faq).

---

## 3. Tool Availability in the Flatpak Ecosystem

### 3.1 MangoHud -- Flatpak VulkanLayer Extension

**Status**: Fully available as `org.freedesktop.Platform.VulkanLayer.MangoHud`

**How to use**:

- Install matching branch (e.g., `//25.08` for Freedesktop SDK 25.08)
- Enable via `MANGOHUD=1` environment variable
- Config access requires: `flatpak override --user --filesystem=xdg-config/MangoHud:ro`

**Key detail**: MangoHud is distributed as a VulkanLayer extension mounted at `/usr/lib/extensions/vulkan/MangoHud/`. Both Bottles and Lutris include the MangoHud path in their `PATH` environment variable.

**Confidence**: High -- [GitHub repo](https://github.com/flathub/org.freedesktop.Platform.VulkanLayer.MangoHud), active with branch/25.08.

### 3.2 Gamescope -- VulkanLayer Extension (Hack)

**Status**: Available via TWO packages, both with significant caveats:

1. **`com.valvesoftware.Steam.Utility.gamescope`** -- Steam-specific utility extension
2. **`org.freedesktop.Platform.VulkanLayer.gamescope`** -- Runtime extension using the VulkanLayer mount point

The VulkanLayer approach is **acknowledged as a hack** by maintainers: "this specific extension mount point was likely not intended to be used with this kind of tool" ([source](https://github.com/flathub/org.freedesktop.Platform.VulkanLayer.gamescope)).

**Critical issue**: Flatpak gamescope does NOT work with nested sandboxes (Proton 5.13+). Community-made Proton versions are required, not official Valve builds. This issue remains **unresolved as of mid-2025** ([issue #6](https://github.com/flathub/org.freedesktop.Platform.VulkanLayer.gamescope/issues/6)).

**Library conflicts**: The extension introduces libraries (libevdev, libinput, libliftoff, libseat, libwlroots, libxcvt) that may conflict with app-bundled versions.

**NVIDIA issues**: Fails to access NVIDIA DRM's GBM backend without `GBM_BACKENDS_PATH` override.

**Confidence**: High -- direct from GitHub repositories and issue trackers.

### 3.3 GameMode -- Portal-Based Access

**Status**: Available via `org.freedesktop.portal.GameMode` (version 4)

GameMode 1.4+ supports Flatpak through the XDG portal. The portal proxies requests to the host `com.feralinteractive.GameMode` daemon with PID namespace translation.

**Known bug**: GameMode activates but doesn't properly **register processes** in some Flatpak contexts. `gamemoded` shows "gamemode is active but [PID] not registered," preventing features like core pinning on AMD 7950X3D. Manual `gamemoded -r<PID>` works ([flathub/com.valvesoftware.Steam#1270](https://github.com/flathub/com.valvesoftware.Steam/issues/1270)).

**Requirements**: `gamemoded` must be installed as a system package on the host. Proton >= 7.0 loads `libgamemodeauto.so` automatically.

**Confidence**: Medium -- the PID registration bug is documented but unclear if fixed in latest releases.

### 3.4 Winetricks -- No Standalone Flatpak

**Status**: No standalone Flatpak on Flathub. Bundled within other packages:

- Wine Flathub package (`org.winehq.Wine`) includes winetricks
- Lutris bundles winetricks
- `flatpak-wine` project bundles wine + winetricks

**Known issues**: Winetricks fails in Flatpak Lutris 0.5.19 on Fedora Atomic/Bazzite due to `wget` (libassuan) and `zenity` (libjpeg) library conflicts ([lutris/lutris#6144](https://github.com/lutris/lutris/issues/6144), May 2025).

**Protontricks** (`com.github.Matoking.protontricks`) is available on Flathub as a winetricks wrapper for Proton.

**Confidence**: High -- verified via Flathub search and GitHub issues.

### 3.5 umu-launcher -- No Official Flatpak

**Status**: No official standalone Flatpak on Flathub as of April 2026

**Build support**: The repository includes Flatpak build manifests under `packaging/flatpak/` for self-building ([source](https://github.com/Open-Wine-Components/umu-launcher/tree/main/packaging/flatpak)).

**Integration**: umu-launcher is increasingly bundled within other launchers:

- Lutris 0.5.20 (Feb 2026) made umu-launcher the default for GE-Proton
- The Lutris Flatpak manifest includes umu runtime support

**Known issue**: Using the prebuilt zipapp artifact inside a Flatpak results in `libdl.so.2` errors within pressure-vessel ([umu-launcher#430](https://github.com/Open-Wine-Components/umu-launcher/issues/430)). Packaging all Python dependencies makes the Flatpak "incredibly huge."

**Confidence**: High -- confirmed via GitHub issues #335, #430, and release notes.

---

## 4. Portal APIs for Gaming

### 4.1 `org.freedesktop.portal.GameMode` (Established)

**Interface version**: 4
**Status**: Stable, integrated into xdg-desktop-portal

Methods: `QueryStatus`, `RegisterGame`, `UnregisterGame`, `QueryStatusByPid`

The portal translates PIDs from sandbox namespace to host namespace. Automatic cleanup occurs when clients terminate without calling `UnregisterGame`.

**Adoption**: Used by Steam Flatpak, Lutris, and any Flatpak app loading `libgamemodeauto.so`. The PID registration bug (Section 3.3) limits effectiveness.

**Confidence**: High -- [official docs](https://flatpak.github.io/xdg-desktop-portal/docs/doc-org.freedesktop.portal.GameMode.html), [source code](https://github.com/flatpak/xdg-desktop-portal/blob/main/src/gamemode.c).

### 4.2 `org.freedesktop.portal.Background` (Risk for CrossHook)

**Status**: Stable, but potentially **dangerous for game launchers**

The Background portal monitors applications without active windows. If a Flatpak app loses its foreground window, it can be classified as "background" and **silently killed** by xdg-desktop-portal if the user hasn't granted background permission.

**Critical issue**: When xdg-desktop-portal terminates a background process, it **doesn't log anything visible to users** ([xdg-desktop-portal#1104](https://github.com/flatpak/xdg-desktop-portal/issues/1104)). Games managed by CrossHook could be silently killed.

**Mitigation**: CrossHook must call `RequestBackground` from `org.freedesktop.portal.Background` to prevent game process termination.

**Confidence**: High -- documented in [portal docs](https://flatpak.github.io/xdg-desktop-portal/docs/doc-org.freedesktop.portal.Background.html) and GitHub issues.

### 4.3 Proposed: Game Status Portal (Discussion Only)

A community proposal ([xdg-desktop-portal#1222](https://github.com/flatpak/xdg-desktop-portal/discussions/1222)) suggests a portal for sharing game-running status (for Discord presence, system optimizations). Discussion remains open with no committed development path.

Participants noted the existing GameMode portal might be expanded rather than creating a new one.

**Confidence**: Low -- exploratory discussion only, no implementation timeline.

### 4.4 Recent New Portals (2025-2026)

| Portal                | Status      | Relevance                                                                                  |
| --------------------- | ----------- | ------------------------------------------------------------------------------------------ |
| USB Portal            | New in 2025 | Could enable controller passthrough                                                        |
| Notification v2       | New in 2025 | Sounds, categories, purpose fields                                                         |
| Host App Registry     | New in 2025 | Lets non-sandboxed apps register with portals                                              |
| Background (improved) | Ongoing     | [Issue #899](https://github.com/flatpak/xdg-desktop-portal/issues/899) tracks improvements |

**Confidence**: Medium -- from xdg-desktop-portal release notes; gaming relevance is speculative.

---

## 5. Wine Wayland Driver

### 5.1 Evolution Timeline

| Version   | Date         | Wayland Status                                               |
| --------- | ------------ | ------------------------------------------------------------ |
| Wine 9.0  | Early 2024   | Experimental driver merged, basic use cases                  |
| Wine 10.0 | January 2025 | "Working fairly well," HiDPI support, mode-setting emulation |
| Wine 11.0 | January 2026 | Clipboard fixed, input handling improved, HDR/VRR prep       |

### 5.2 Current Capabilities (Wine 11.0)

- Clipboard operations between Windows and Linux apps work reliably
- Window positioning/sizing more predictable (fullscreen, multi-monitor)
- Relative mouse motion and keyboard focus improved (reduced input lag)
- Preparing for HDR and VRR standards
- Still defaults to X11 when `DISPLAY` is set; Wayland used when X11 unavailable

**Confidence**: High -- [Wine 10.0 release](https://www.phoronix.com/news/Wine-10.0-Released), [Wine 11.0 announcement](https://windowsforum.com/threads/wine-11-0-ntsync-wow64-overhaul-and-wayland-updates-to-transform-linux-gaming.406839/).

### 5.3 Proton Integration

- Proton-GE enables Wayland driver via `PROTON_ENABLE_WAYLAND=1`
- HDR auto-enables Wine-Wayland (required dependency)
- **Steam Overlay does NOT work** with Wine Wayland enabled
- **Steam Input broken** with Wine Wayland
- Mouse input issues and FPS capping reported in some games

**Confidence**: High -- [Proton-GE GitHub](https://github.com/GloriousEggroll/proton-ge-custom), [CachyOS Forum](https://discuss.cachyos.org/t/proton-wine-wayland-driver/4497).

### 5.4 Flatpak Implications

For Wine Wayland to work inside a Flatpak:

- The app needs `--socket=wayland` (most gaming Flatpaks already declare this)
- XWayland fallback needs `--socket=x11` (also common)
- No additional special permissions needed for the Wayland driver itself

**NTSYNC (the bigger story)**: Merged into Linux kernel 6.14 (January 2025). Performance gains of 124-678% depending on game threading. Wine 11 auto-detects kernel 6.14+ and enables NTSYNC. This is orthogonal to Flatpak sandboxing -- NTSYNC works at the kernel level regardless of packaging.

**Confidence**: High -- [Wine 11 NTSYNC analysis](https://byteiota.com/wine-11-ntsync-kernel-rewrite-678-gaming-performance-gains/), kernel changelogs.

### 5.5 Performance: Wayland vs XWayland

| Aspect               | Native Wayland | XWayland                      |
| -------------------- | -------------- | ----------------------------- |
| Input latency        | Lower (direct) | Higher (translation layer)    |
| HiDPI support        | Native         | Scaling artifacts             |
| HDR support          | In development | Not planned                   |
| VRR support          | In development | Limited                       |
| Steam Overlay        | **Broken**     | Works                         |
| Compatibility        | Growing        | Mature                        |
| XWayland maintenance | N/A            | Declining developer attention |

**Confidence**: Medium -- performance claims from Collabora articles; real-world benchmarks vary.

---

## 6. Community Sentiment & Discussions

### 6.1 Flathub Gaming App Reviews

The Arch Wiki warns: "Many Flatpak applications available on Flathub are not effectively sandboxed by default" and advises reviewing permission manifests ([ArchWiki: Flatpak](https://wiki.archlinux.org/title/Flatpak)).

A game example (CorsixTH) was flagged for `--filesystem=home:ro` just to read game data files, with reviewers recommending predefined sandbox directories instead ([flathub/com.corsixth.corsixth#1](https://github.com/flathub/com.corsixth.corsixth/issues/1)).

### 6.2 Steam Flatpak User Experience

Mixed but trending positive:

- "Steam flatpak has been overall the best experience" -- [GamingOnLinux commenter](https://www.gamingonlinux.com/2026/01/steam-frame-and-steam-machine-will-be-another-good-boost-for-flatpaks-and-desktop-linux-overall-too/)
- NVIDIA driver sync issues between host and Flatpak runtime remain a pain point
- Controller mapping issues when Steam Overlay can't run

### 6.3 Reddit/Forum Consensus (2025-2026)

Key themes from r/linux_gaming and forums:

- **Flatpak is becoming default** for gaming tools on immutable distros
- **Permission management is confusing** for non-technical users (Flatseal helps but adds friction)
- **Fragmentation concerns** (deb, rpm, Flatpak, AppImage) still exist but Flatpak is "winning"
- **Performance impact of Flatpak is negligible** for most gaming scenarios ([Linux Mint Forums](https://forums.linuxmint.com/viewtopic.php?t=418114))

**Confidence**: Medium -- community sentiment is directional, not rigorous.

---

## 7. Immutable Distro Landscape

### 7.1 Fedora Atomic (Silverblue/Kinoite)

- **Silverblue 42** released April 15, 2025 with GNOME 48
- Flatpak is the **primary** application installation method
- rpm-ostree layering exists but erodes the immutable model
- Gaming guide recommends Steam, Heroic, Lutris all via Flatpak ([Zihad Labs, 2025](https://zihad.com.bd/posts/fedora-atomic-gaming-guide-2025/))

**Confidence**: High -- official Fedora release notes.

### 7.2 SteamOS & Steam Deck

- SteamOS has a **read-only immutable** filesystem
- Valve recommends Flatpak for all additional software
- Steam client itself is **NOT** a Flatpak (baked into OS image)
- Flathub is the default software source in Discover (Desktop Mode)
- **Steam Frame and Steam Machine** (2026) will further cement Flatpak as standard ([GamingOnLinux](https://www.gamingonlinux.com/2026/01/steam-frame-and-steam-machine-will-be-another-good-boost-for-flatpaks-and-desktop-linux-overall-too/))

**Confidence**: High -- [SteamOS documentation](https://partner.steamgames.com/doc/steamdeck/faq), multiple news sources.

### 7.3 Bazzite (Universal Blue)

The most popular gaming-focused immutable distro. Based on Fedora Atomic:

**Pre-installed gaming tools**:

- Steam and Lutris (layered packages, not Flatpaks)
- vkBasalt, MangoHud, OBS VkCapture
- LAVD and BORE CPU schedulers
- Waydroid for Android apps

**Flatpak integration**:

- Automatic Flatpak updates via uupd/topgrade
- Bazzite Portal provides curated Flatpak gaming app installation
- Old portal list included: BoilR, Bottles, chiaki-ng, Discord, Heroic, itch, Moonlight, etc.

**Recent**: Release 43.20260302 (March 2026) ships KDE 6.6. Team joined "Open Gaming Collective" for sustainable ecosystem development ([source](https://github.com/ublue-os/bazzite)).

**Confidence**: High -- direct from [Bazzite GitHub](https://github.com/ublue-os/bazzite), [LWN.net review](https://lwn.net/Articles/1046228/).

### 7.4 Market Context

| Metric                              | Value                         | Source                |
| ----------------------------------- | ----------------------------- | --------------------- |
| Flathub downloads (2025)            | 438.2 million                 | GamingOnLinux         |
| Linux Steam market share (Nov 2025) | 3.20% (~4.22M users)          | Steam Hardware Survey |
| Year-over-year growth               | 57%                           | commandlinux.com      |
| Most popular distro on ProtonDB     | CachyOS (2026)                | BoilingSteam          |
| GNOME 50                            | Dropped X11 sessions entirely | Multiple sources      |

---

## 8. Implications for CrossHook

### 8.1 The Lutris Precedent is Favorable

Lutris successfully ships on Flathub with `--talk-name=org.freedesktop.Flatpak`. CrossHook's use case (orchestrating game launches via host Proton/Wine) is directly analogous. This is the strongest precedent.

However: Lutris is a well-established, widely-used project. CrossHook will need to demonstrate equivalent legitimacy and justify the permission with concrete technical rationale.

### 8.2 Tool Access Strategy

| Tool         | Flatpak Availability            | CrossHook Strategy                          |
| ------------ | ------------------------------- | ------------------------------------------- |
| MangoHud     | VulkanLayer extension           | Reference via extension path in sandbox     |
| Gamescope    | VulkanLayer extension (hacky)   | Consider host gamescope via `flatpak-spawn` |
| GameMode     | Portal API                      | Use `org.freedesktop.portal.GameMode`       |
| Winetricks   | Bundled in Wine/Lutris Flatpaks | Bundle or delegate to host                  |
| umu-launcher | No standalone Flatpak           | Bundle as module or delegate to host        |
| Wine/Proton  | Host system                     | Access via `flatpak-spawn --host`           |

### 8.3 Critical Risks

1. **Background portal kills**: Games launched by CrossHook could be silently terminated. MUST implement `RequestBackground` portal call.
2. **Permission review timing**: Post-CVE-2026-34078, reviewers may be more stringent about sandbox-escaping permissions.
3. **Gamescope nested sandbox incompatibility**: If CrossHook uses gamescope via Flatpak extension, it won't work with official Proton builds.
4. **umu-launcher packaging complexity**: Python dependency tree makes Flatpak bundling "incredibly huge."
5. **NVIDIA driver sync**: Flatpak runtime drivers must match host drivers or games won't render.

### 8.4 Immutable Distro Importance

CrossHook's Flatpak is **essential** for reaching users on:

- Steam Deck / SteamOS (read-only filesystem, Flatpak is the only option)
- Bazzite (most popular gaming immutable distro)
- Fedora Atomic variants (growing user base)

These platforms represent a significant and growing share of the Linux gaming audience. Not having a Flatpak locks CrossHook out of these ecosystems.

---

## 9. Uncertainties & Gaps

1. **Flathub reviewer discretion**: No published rubric for how much justification is "enough" for `--talk-name=org.freedesktop.Flatpak`. Approval is case-by-case.
2. **Fine-grained flatpak-spawn filtering**: [Feature request #5538](https://github.com/flatpak/flatpak/issues/5538) would let apps declare allowed host commands. No timeline for implementation. If/when it ships, CrossHook should adopt it immediately.
3. **GameMode PID registration bug**: Unclear if this is fixed in latest GameMode/portal versions. Needs testing.
4. **Wine Wayland in Proton timeline**: No official Valve timeline for Proton adopting Wine Wayland driver by default.
5. **Game Status Portal**: Community discussion only; no committed development path.
6. **Post-CVE policy changes**: Too early to know if CVE-2026-34078 will trigger Flathub policy tightening.

---

## 10. Search Queries Executed

1. `Flathub policy talk-name org.freedesktop.Flatpak sandbox escape 2025 2026`
2. `Lutris Flatpak Flathub acceptance review gaming app sandbox permissions 2025`
3. `MangoHud Flatpak VulkanLayer extension 2025 2026 how to use`
4. `GameMode Flatpak native activation portal 2025 2026`
5. `org.freedesktop.portal.GameMode status adoption 2025 2026`
6. `Wine Wayland driver experimental status 2025 2026 performance vs XWayland`
7. `winetricks Flatpak packaging 2025 2026 status`
8. `gamescope Flatpak packaging 2025 2026 nested compositor`
9. `umu-launcher Flatpak packaging status 2025 2026`
10. `xdg-desktop-portal Background portal long-running game process 2025 2026`
11. `Flathub GitHub issues gaming app policy sandbox escape review 2025`
12. `Fedora Atomic Silverblue Flatpak gaming 2025 2026 immutable distro`
13. `reddit linux_gaming Flatpak gaming experience bundling tools 2025 2026`
14. `SteamOS Steam Deck Flatpak usage gaming apps 2025 2026`
15. `Universal Blue Bazzite Flatpak gaming tools bundled 2025 2026`
16. `freedesktop GitLab gaming portal proposals new portals 2025 2026`
17. `Flathub Lutris net.lutris.Lutris manifest permissions talk-name org.freedesktop.Flatpak`
18. `Flatpak gaming app host execution flatpak-spawn review policy precedent 2025`
19. `Heroic Games Launcher Flatpak flathub permissions talk-name org.freedesktop.Flatpak manifest`
20. `Bottles Flatpak Flathub permissions sandbox host execution manifest 2025`
21. `Flathub requirements app authors permissions review 2025 documentation`
22. `Wine Wayland driver Proton integration Flatpak wayland socket 2025 2026`
23. `ProtonDB Flatpak gaming experience community feedback 2025 2026`

Plus direct URL fetches of:

- Flathub requirements documentation
- Lutris Flatpak manifest (raw GitHub)
- Bottles Flatpak manifest (raw GitHub)
- xdg-desktop-portal Game Status Portal discussion

---

## Sources

### Flathub & Flatpak Policy

- [Flathub Requirements for App Authors](https://docs.flathub.org/docs/for-app-authors/requirements)
- [Flathub Safety: A Layered Approach](https://docs.flathub.org/blog/app-safety-layered-approach-source-to-user)
- [Flatpak Sandbox Permissions](https://docs.flatpak.org/en/latest/sandbox-permissions.html)
- [CVE-2026-34078 Advisory](https://github.com/flatpak/flatpak/security/advisories/GHSA-cc2q-qc34-jprg)
- [Flatpak 1.16.4 Security Fix (Phoronix)](https://www.phoronix.com/news/Flatpak-1.16.4-Released)
- [Fine-Grained flatpak-spawn Permissions (Issue #5538)](https://github.com/flatpak/flatpak/issues/5538)
- [Re-review on Sandbox Changes (flathub/flathub#1337)](https://github.com/flathub/flathub/issues/1337)
- [Flatpak Sandbox Security Study (Linux Journal)](https://www.linuxjournal.com/content/when-flatpaks-sandbox-cracks-real-life-security-issues-beyond-ideal)

### Gaming App Manifests

- [Lutris Flathub Manifest](https://github.com/flathub/net.lutris.Lutris)
- [Bottles Flathub Manifest](https://github.com/flathub/com.usebottles.bottles/blob/master/com.usebottles.bottles.yml)
- [Heroic Games Launcher on Flathub](https://flathub.org/en/apps/com.heroicgameslauncher.hgl)
- [Steam Flatpak](https://github.com/flathub/com.valvesoftware.Steam)
- [Lutris Running Flatpak-Spawn (Issue #274)](https://github.com/flathub/net.lutris.Lutris/issues/274)
- [Bottles Reduce Permissions (Issue #120)](https://github.com/flathub/com.usebottles.bottles/issues/120)

### Gaming Tools

- [MangoHud VulkanLayer Extension](https://github.com/flathub/org.freedesktop.Platform.VulkanLayer.MangoHud)
- [Gamescope VulkanLayer Extension](https://github.com/flathub/org.freedesktop.Platform.VulkanLayer.gamescope)
- [Gamescope Steam Utility](https://github.com/flathub/com.valvesoftware.Steam.Utility.gamescope)
- [Gamescope Nested Sandbox Issue](https://github.com/flathub/org.freedesktop.Platform.VulkanLayer.gamescope/issues/6)
- [umu-launcher](https://github.com/Open-Wine-Components/umu-launcher)
- [umu-launcher Flatpak Issue (#430)](https://github.com/Open-Wine-Components/umu-launcher/issues/430)
- [Protontricks on Flathub](https://flathub.org/en/apps/com.github.Matoking.protontricks)
- [Winetricks Failures in Lutris Flatpak (Issue #6144)](https://github.com/lutris/lutris/issues/6144)

### Portal APIs

- [GameMode Portal Documentation](https://flatpak.github.io/xdg-desktop-portal/docs/doc-org.freedesktop.portal.GameMode.html)
- [GameMode Portal Source](https://github.com/flatpak/xdg-desktop-portal/blob/main/src/gamemode.c)
- [GameMode Portal PR #314](https://github.com/flatpak/xdg-desktop-portal/pull/314)
- [Background Portal Documentation](https://flatpak.github.io/xdg-desktop-portal/docs/doc-org.freedesktop.portal.Background.html)
- [Background Portal Kill Logging (Issue #1104)](https://github.com/flatpak/xdg-desktop-portal/issues/1104)
- [Improve Background Apps (Issue #899)](https://github.com/flatpak/xdg-desktop-portal/issues/899)
- [Game Status Portal Proposal (Discussion #1222)](https://github.com/flatpak/xdg-desktop-portal/discussions/1222)
- [GameMode PID Registration Bug (Issue #1270)](https://github.com/flathub/com.valvesoftware.Steam/issues/1270)

### Wine & Proton

- [Wine 10.0 Release (Phoronix)](https://www.phoronix.com/news/Wine-10.0-Released)
- [Wine 11.0 NTSYNC Analysis](https://byteiota.com/wine-11-ntsync-kernel-rewrite-678-gaming-performance-gains/)
- [Wine on Wayland Year in Review (Collabora)](https://www.collabora.com/news-and-blog/news-and-events/wine-on-wayland-a-year-in-review-and-a-look-ahead.html)
- [Proton-GE GitHub](https://github.com/GloriousEggroll/proton-ge-custom)
- [Wine Wayland in Proton (CachyOS Forum)](https://discuss.cachyos.org/t/proton-wine-wayland-driver/4497)

### Immutable Distros & Community

- [Bazzite GitHub](https://github.com/ublue-os/bazzite)
- [Bazzite LWN.net Review](https://lwn.net/Articles/1046228/)
- [Fedora Atomic Gaming Guide 2025](https://zihad.com.bd/posts/fedora-atomic-gaming-guide-2025/)
- [Steam Frame & Flatpak (GamingOnLinux)](https://www.gamingonlinux.com/2026/01/steam-frame-and-steam-machine-will-be-another-good-boost-for-flatpaks-and-desktop-linux-overall-too/)
- [SteamOS FAQ](https://partner.steamgames.com/doc/steamdeck/faq)
- [Linux Gaming Market Share 2026](https://commandlinux.com/statistics/linux-gaming-market-share-steam-survey/)
- [CachyOS Most Popular on ProtonDB](https://boilingsteam.com/cachy-os-is-now-the-most-popular-distro-on-proton-db/)
- [ArchWiki: Flatpak](https://wiki.archlinux.org/title/Flatpak)
- [GamingOnLinux: SteamOS Extra Software Guide](https://www.gamingonlinux.com/guides/view/how-to-install-extra-software-apps-and-games-on-steamos-and-steam-deck/)
