# Futurist Extrapolation: Flatpak Gaming Trajectory (2026-2029)

> **Perspective**: Futurist Extrapolator
> **Date**: 2026-04-15
> **Scope**: 1-3 year projections for the Flatpak gaming ecosystem and CrossHook's strategic positioning

---

## Executive Summary

The Linux gaming ecosystem is in the midst of a structural transformation driven by five converging forces: (1) the Wayland transition reaching completion, (2) immutable distros becoming the dominant gaming platform, (3) Valve's expanding hardware ecosystem, (4) Flatpak's sandbox and portal modernization, and (5) the formation of cross-distro collaboration bodies like the Open Gaming Collective. For CrossHook, these trends collectively shift the argument **toward** bundling critical tools inside a Flatpak package, because the "just use host tools" assumption increasingly breaks on the platforms where CrossHook's users actually live.

**Confidence**: Medium-High (based on observable trends with clear momentum, though specific timelines remain uncertain)

---

## 1. Flatpak Platform Evolution (2026-2029)

### 1.1 systemd-appd and Nested Sandboxing

Sebastian Wick (Red Hat/GNOME) and Adrian Vovk are developing **systemd-appd**, a service for querying running app instances that serves as a prerequisite for nested sandboxing, improved PipeWire support, and eliminating the D-Bus proxy. This was detailed in Wick's ["Flatpak Happenings" blog post](https://blog.sebastianwick.net/posts/flatpak-happenings/) (November 2025) and his Linux Application Summit talk covered by [LWN.net](https://lwn.net/Articles/1020571/).

**Current state**: Flatpak cannot support nested sandboxing. Applications like web browsers that sandbox their own tabs via Bubblewrap cannot do so inside Flatpak. The sub-sandbox API exists but is more restrictive than needed.

**Projection (2027-2028)**: systemd-appd lands and enables:

- Authenticated app instances via cgroup metadata (building on [`SO_PEERPIDFD` improvements](https://blog.sebastianwick.net/posts/so-peerpidfd-gets-more-useful/))
- Nested sandboxing for complex apps
- Elimination of the D-Bus proxy

**CrossHook impact**: If CrossHook runs tools like gamescope or trainers as sub-processes, nested sandboxing could allow CrossHook to create per-tool sandboxes with appropriate permissions, rather than requiring broad `--device=all` access for the entire app.

**Confidence**: Medium (systemd-appd is in active planning but no shipped code yet; Wick himself cautioned that a hypothetical "Flatpak-Next" rewrite would still face these same limitations without upstream kernel/systemd work)

### 1.2 Portal API Development

The xdg-desktop-portal ecosystem is actively addressing gaming-adjacent needs:

| Portal                              | Status                                                                      | Gaming Relevance                                  |
| ----------------------------------- | --------------------------------------------------------------------------- | ------------------------------------------------- |
| **USB Portal**                      | Shipped in xdg-desktop-portal 1.19.1 (Dec 2024)                             | USB game controllers, custom peripherals          |
| **Input Capture**                   | Shipped                                                                     | Controller/joystick capture for Steam, Wine       |
| **Joystick/Game Controller Portal** | [Open issue #536](https://github.com/flatpak/xdg-desktop-portal/issues/536) | Would eliminate need for `--device=all` for games |
| **Notification v2**                 | Shipped in 1.19.1                                                           | Rich notifications for game events                |
| **Host App Registry**               | Shipped Feb 2025                                                            | Better app identification for non-sandboxed apps  |

**Projection (2027-2028)**: A dedicated gaming input portal resolving issue #536 would be transformative. Currently, Steam's Flatpak and game launchers require `--device=all` to enumerate joysticks, HID devices, and evdev nodes. A proper portal would allow sandboxed game launchers to request only the specific input devices they need.

**Confidence**: Low-Medium (the issue has been open since 2021; progress is slow but the underlying infrastructure like `--device=input` is being built)

### 1.3 GPU Virtualization for Flatpak

Sebastian Wick's January 2026 blog post ["Improving the Flatpak Graphics Drivers Situation"](https://blog.sebastianwick.net/posts/flatpak-graphics-drivers/) describes using VirtIO-GPU with Mesa Venus to avoid shipping host-matched GPU drivers inside the Flatpak runtime.

**Current problem**: GPU drivers inside Flatpak must match the host kernel version and be built against the runtime. This breaks when:

- The runtime is end-of-life
- The host kernel is newer than the runtime's driver build
- NVIDIA driver versions don't match host-to-container

**Projection**: GPU virtualization via Venus would decouple the container's graphics stack from the host, but with potential performance overhead. This is unlikely to be the default path for gaming (where raw GPU performance matters), but could serve as a robust fallback.

**CrossHook impact**: CrossHook itself doesn't need heavy GPU access (it's an orchestrator, not a renderer), but gamescope does. If GPU virtualization becomes the standard Flatpak approach, bundling gamescope becomes more feasible but potentially with performance caveats.

**Confidence**: Low (exploratory; Wick acknowledges "a bunch of issues and unknowns")

### 1.4 Flatpak Extension Mechanism

Flatpak extensions remain the primary mechanism for optional shared libraries. Key 2025 developments:

- The GNOME runtime [dropped its 32-bit compatibility extension](https://blogs.gnome.org/alatiera/2025/10/13/flatpak-32bit/) (`org.gnome.Platform.i386.Compat`), pushing gaming apps toward the Freedesktop runtime's `GL32` and `Compat.i386` extensions directly
- The [gamescope Vulkan layer extension](https://github.com/flathub/org.freedesktop.Platform.VulkanLayer.gamescope) (`org.freedesktop.Platform.VulkanLayer.gamescope`) exists on Flathub but is described by its own maintainers as a "hack" using the Vulkan layer mount point for a purpose it wasn't designed for
- RetroDECK demonstrated an [advanced layered library architecture](https://retrodeck.readthedocs.io/en/latest/blog/2025/11/25/november-2025-finally-a-fatpak/) within a single Flatpak, solving the multiple-runtime-versions problem

**Projection (2027-2028)**: Extension mechanisms will likely mature to better support gaming use cases. RetroDECK's approach of component-specific library layers within a single Flatpak could become a pattern for complex gaming apps.

**CrossHook impact**: CrossHook could use extensions to provide optional gamescope/tool bundles without bloating the base package. However, the gamescope extension's "hack" status suggests this isn't yet a stable, endorsed pattern.

**Confidence**: Medium

---

## 2. Wine/Proton Evolution

### 2.1 Wine Wayland Driver Timeline

| Version   | Date     | Wayland Status                                                                                                                                                         |
| --------- | -------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Wine 10.0 | Jan 2025 | Wayland driver enabled by default; OpenGL support; [X11 fallback still primary](https://www.phoronix.com/news/Wine-10.0-Released)                                      |
| Wine 10.3 | Mar 2025 | [Clipboard support added](https://www.gamingonlinux.com/2025/03/wine-10-3-released-with-clipboard-support-in-the-wayland-driver-initial-vulkan-video-decoder-support/) |
| Wine 11.0 | Jan 2026 | [Shaped windows, input methods added; still "experimental"](https://biggo.com/news/202601151322_Wine-11-Stable-Release-Performance-Compatibility-Gains)                |

**Projection (2027-2028)**: The Wine Wayland driver will reach parity with X11 for most gaming scenarios. With GNOME 50 (March 2026) shipping zero X11 code and KDE Plasma 6.8 (~October 2026) going Wayland-only, XWayland becomes the only X11 path. Wine's Wayland driver will be forced to mature rapidly once users have no native X11 session to fall back to.

**Proton native Wayland**: Currently [not in official Proton](https://github.com/ValveSoftware/Proton/issues/4638), though community builds (TKG) report it working. A Proton 10.x release with native Wayland support is anticipated but no firm date from Valve.

**CrossHook impact**: When Wine games run natively on Wayland, XWayland dependencies are eliminated from the graphics path. This simplifies the stack but also means gamescope's role changes -- it's no longer needed to bridge X11-to-Wayland but remains essential for HDR, VRR, frame limiting, and resolution scaling.

**Confidence**: High (the trajectory is clear and both major DEs are forcing the issue)

### 2.2 NTSync: Kernel-Level Gaming Performance

Wine 11.0 (January 2026) integrated [NTSync](https://windowsforum.com/threads/wine-11-0-ntsync-wow64-overhaul-and-wayland-updates-for-better-linux-gaming.406839/), a Linux kernel module (kernel 6.14+) that efficiently emulates Windows NT synchronization primitives. This is a significant performance boost for thread-heavy game engines.

**Projection**: NTSync becomes the standard for gaming on kernels 6.14+. All major gaming distros (Bazzite, CachyOS, Nobara) will ship kernels with NTSync enabled. This benefits all Wine/Proton games regardless of launcher.

**CrossHook impact**: Minimal direct impact on bundling decisions, but confirms that the kernel-level gaming stack is maturing independently of any particular launcher's approach.

**Confidence**: High (already shipped in Wine 11.0; kernel support is upstream)

### 2.3 Proton's Container Approach (pressure-vessel)

Valve's [pressure-vessel](https://www.gamingonlinux.com/2020/10/valve-put-their-pressure-vessel-container-source-for-linux-games-up-on-gitlab/) containerizes each game in its own Steam Runtime environment. It continues receiving maintenance (runtime version `0.20251201.0+srt1` confirmed in January 2026 issue reports), but there are no announced major architectural changes.

**Projection**: Pressure-vessel will continue as the container runtime for Proton games launched through Steam. For non-Steam launchers (where CrossHook operates), umu-launcher replicates this behavior. The two container approaches (Flatpak's bubblewrap and Valve's pressure-vessel) will coexist rather than merge.

**CrossHook impact**: CrossHook must remain aware of two container layers when running inside Flatpak: the Flatpak sandbox and the pressure-vessel container that Proton creates. `flatpak-spawn --host` currently bridges this gap, but a bundled approach would need to ensure the inner container (pressure-vessel) still functions correctly.

**Confidence**: Medium-High

---

## 3. Compositor Evolution

### 3.1 Gamescope Trajectory

Gamescope remains the [critical microcompositor for Linux gaming](https://wiki.archlinux.org/title/Gamescope), providing HDR, VRR, resolution scaling, and frame synchronization. Key developments:

- **Open Gaming Collective fork**: The OGC (formed January 2026) maintains a [specialized gamescope fork](https://www.gamingonlinux.com/2026/01/open-gaming-collective-ogc-formed-to-push-linux-gaming-even-further/) designed to expand hardware support
- **NVIDIA support**: NVIDIA is [actively working on gamescope driver support](https://www.phoronix.com/news/Gamescope-NVIDIA-Pending)
- **Flatpak availability**: The `org.freedesktop.Platform.VulkanLayer.gamescope` extension exists but has known issues (library conflicts, HDR not working in some Flatpak scenarios)

**Projection (2027-2029)**:

1. **Short term (2026-2027)**: Gamescope remains essential. HDR on Linux requires it for most setups. The OGC fork becomes the de facto version for non-SteamOS distros.

2. **Medium term (2027-2028)**: As KDE and GNOME compositors gain native HDR/VRR support (KDE 6.4+ is already making progress; [Wayland color management protocol merged Feb 2025](https://www.gamingonlinux.com/2025/02/wayland-colour-management-and-hdr-protocol-finally-merged/)), gamescope's role narrows to advanced scenarios: nested compositor for resolution spoofing, FSR upscaling, and Steam Deck session mode.

3. **Long term (2028-2029)**: Gamescope may become optional for desktop gaming as native compositor HDR/VRR matures, but remains essential for handheld/HTPC (Steam Deck, Steam Machine) use cases.

**CrossHook impact**: In the near term, gamescope remains a critical tool that CrossHook needs to orchestrate. The question of bundling vs. host-detection depends on which gamescope variant users have (OGC, distro-packaged, or Flatpak extension). By 2028, if desktop compositors absorb gamescope's key features, CrossHook may need gamescope less frequently.

**Confidence**: Medium (gamescope's necessity depends on how fast desktop compositors catch up)

### 3.2 HDR/VRR on Linux

The HDR landscape transformed in 2025:

| Component                             | Status                                                                                                                                |
| ------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------- |
| Wayland color management protocol     | [Merged Feb 2025](https://www.gamingonlinux.com/2025/02/wayland-colour-management-and-hdr-protocol-finally-merged/)                   |
| KDE Plasma HDR                        | Experimental since 6.0; streamlined in 6.4 with Mesa 25.1                                                                             |
| GNOME Mutter `wp_color_management_v1` | [Merged in GNOME 48](https://canartuc.medium.com/gnome-completely-drops-x11-support-the-wayland-era-begins-387e961926c0) (March 2025) |
| Mesa 25.1 Vulkan HDR extensions       | Shipped mid-2025                                                                                                                      |
| NVIDIA 595.x Vulkan HDR               | Shipped 2025                                                                                                                          |
| Wine/DXVK HDR                         | Available via `DXVK_HDR=1` with `PROTON_ENABLE_WAYLAND=1`                                                                             |

**Projection (2027)**: HDR becomes a "mostly works" experience on KDE Plasma with AMD GPUs. NVIDIA and GNOME lag 6-12 months behind. Gamescope remains the most reliable HDR path for gaming through at least 2027.

**CrossHook impact**: If CrossHook wants to offer HDR launch options, it needs gamescope integration. This is a strong argument for either bundling gamescope or ensuring robust detection of the host's gamescope installation.

**Confidence**: High (the protocol foundation is laid; implementation is progressing measurably)

---

## 4. Immutable Distro Dominance

### 4.1 Bazzite as the Gaming Standard

[Bazzite](https://innovariatech.blog/best-linux-distros-for-gaming-in-2026/) (Universal Blue, built on Fedora Atomic) has emerged as the de facto gaming distro recommendation for 2026. Multiple independent sources rank it as the top choice for "set and forget" gaming, handhelds, and HTPCs.

Key characteristics relevant to CrossHook:

- **Immutable base**: Users cannot easily `dnf install` arbitrary packages
- **Atomic updates**: System updates are all-or-nothing with rollback
- **Flatpak-first**: Applications are expected to come from Flathub
- **Pre-installed gaming stack**: Ships gamescope, Steam, and gaming optimizations
- **OGC member**: Adopts shared OGC kernel, InputPlumber for controllers

**Projection (2027-2028)**: Bazzite or similar immutable gaming distros become the majority platform for non-Steam-Deck Linux gamers. Traditional mutable distros (Arch, Manjaro, Pop!\_OS) retain a power-user audience but are no longer the default recommendation.

**Confidence**: Medium-High (Bazzite's momentum is strong; the "set and forget" appeal aligns with gaming's mainstream audience)

### 4.2 The "Can't Install Host Tools" Problem

On immutable distros, users face friction installing host-level tools:

| Distro Type                            | Install Method                                           | Friction Level            |
| -------------------------------------- | -------------------------------------------------------- | ------------------------- |
| Traditional (Arch, Fedora Workstation) | `pacman -S` / `dnf install`                              | Low                       |
| Fedora Atomic / Bazzite                | `rpm-ostree install` (requires reboot) or `brew`/toolbox | Medium                    |
| SteamOS                                | Read-only rootfs; `pacman` works but reverts on update   | High                      |
| NixOS                                  | Declarative config change + rebuild                      | Medium-High for beginners |

**Projection**: As immutable distros gain share, the percentage of CrossHook users who can trivially install host tools **decreases**. On SteamOS specifically, host tool installation is actively discouraged and doesn't survive updates.

**CrossHook impact**: This is the strongest argument for bundling. If CrossHook's target audience is increasingly on Bazzite, SteamOS, or Fedora Atomic, asking them to install gamescope, mangohud, or other tools at the host level becomes a significant usability barrier.

**Confidence**: High

### 4.3 NixOS and Declarative Flatpak

NixOS has growing Flatpak integration via [nix-flatpak](https://github.com/gmodena/nix-flatpak) (presented at NixCon 2025) and an upstream PR (#347605) for NixOS-integrated Flatpak management. However, the NixOS community is philosophically split on whether Flatpak is appropriate at all.

**Projection**: NixOS remains a niche gaming platform but validates the trend toward declarative package management where Flatpak serves as the "escape hatch" for desktop apps.

**CrossHook impact**: Minimal direct impact; NixOS users are sophisticated enough to handle any installation method. But it confirms that even the most "customizable" distros are embracing Flatpak for desktop software.

**Confidence**: Medium

---

## 5. umu-launcher Evolution

### 5.1 Current State

umu-launcher has become [the standard bridge for non-Steam Proton usage](https://github.com/Open-Wine-Components/umu-launcher). As of early 2026:

- **Lutris 0.5.20** (February 2026) made [umu-launcher the default way of running GE-Proton](https://www.patreon.com/posts/lutris-0-5-20-150945758)
- Heroic Games Launcher and other frontends support umu
- Flatpak packaging exists (app ID: `org.openwinecomponents.umu.umu-launcher`) with `--prefix /app` support
- umu-protonfixes provide launcher-agnostic game fixes, making some launcher-specific scripts redundant

### 5.2 Projection (2027-2029)

1. **umu absorbs more orchestration**: The centralized game database (matching store titles to umu IDs) will make umu the universal game identification layer. CrossHook's trainer-to-game matching could potentially leverage this.

2. **Flatpak-native umu**: As Lutris's Flatpak slims down to "bare essentials" (targeting the Steam runtime rather than its own), umu-launcher becomes even more central. A well-packaged umu Flatpak reduces CrossHook's need to handle Proton discovery itself.

3. **Relationship with Steam Runtime**: Lutris is deprecating its Ubuntu 18.04 runtime and rebuilding all runners to target the Steam runtime. This convergence means all paths (Steam, Lutris, Heroic, CrossHook) will share the same runtime foundation.

**CrossHook impact**: umu-launcher's maturation may reduce CrossHook's need to bundle Proton-related tooling, since umu handles Proton discovery, runtime setup, and containerization. CrossHook can focus on trainer orchestration while delegating launch mechanics to umu. However, CrossHook still needs to detect and invoke umu itself.

**Confidence**: High (umu's adoption trajectory is clear; GloriousEggroll and Valve are aligned)

---

## 6. Valve's Expanding Influence

### 6.1 Steam Deck & Hardware Ecosystem

| Product                                   | Status                                                                                                                                                     | Target               |
| ----------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------- | -------------------- |
| Steam Deck (LCD/OLED)                     | Shipping; ~5.6M units by mid-2025                                                                                                                          | Handheld gaming      |
| Steam Machine                             | [Announced Nov 2025; targeting H1 2026](https://www.engadget.com/gaming/pc/valves-steam-machine-launches-in-2026-everything-we-know-so-far-200458597.html) | Living room mini-PC  |
| Steam Frame                               | Announced Nov 2025                                                                                                                                         | Wireless VR headset  |
| Steam Controller 2                        | Announced Nov 2025                                                                                                                                         | Input device         |
| Third-party handhelds (Legion Go S, etc.) | [SteamOS support from May 2025](https://www.xda-developers.com/valve-steamos-general-release-preview/)                                                     | Various form factors |

**Market impact**: Linux gaming hit [5% on Steam](https://www.webpronews.com/linux-crosses-the-5-threshold-on-steam-and-this-time-it-might-actually-matter/) (June 2025) with SteamOS representing ~27% of all Linux Steam installations. The Steam Deck alone drove Linux from ~2% to over 5% in four years.

### 6.2 SteamOS on Desktop Hardware

SteamOS 3.8.0 (March 2026) includes [initial Steam Machine support and desktop system improvements](https://gamerant.com/steam-os-update-patch-notes-march-2026/). However, a general-purpose SteamOS desktop release for arbitrary PCs has not materialized.

**Projection (2027-2028)**: The Steam Machine launch forces Valve to polish SteamOS for non-Deck form factors. A "SteamOS for desktops" release becomes likely as Valve needs the same OS to run on diverse hardware. This would create another immutable, Flatpak-first gaming platform.

**CrossHook impact**: SteamOS users are the hardest audience for "install host tools" -- the rootfs is read-only and packages don't survive updates. If SteamOS expands to desktops and the Steam Machine, CrossHook's Flatpak must be self-sufficient.

**Confidence**: Medium-High (Valve's hardware announcements strongly imply broader SteamOS availability)

### 6.3 Valve's Open-Source Contributions

Valve's influence extends beyond hardware:

- **Proton/Wine**: Directly funds development; NTSync, DXVK, VKD3D-Proton
- **Arch Linux partnership**: Funding Arch development (SteamOS's base)
- **Mesa/GPU drivers**: Contributing to AMD driver stack
- **Gamescope**: Primary developer and maintainer
- **Anti-cheat progress**: EAC and BattlEye support Proton (developers just need to enable it)

**Projection**: Valve continues to be the largest single contributor to Linux gaming infrastructure. Their engineering decisions (e.g., pressure-vessel containerization, gamescope for HDR) become de facto standards.

**Confidence**: High

---

## 7. Display Technology Impact

### 7.1 HDR Timeline

HDR on Linux has progressed from "impossible" to "mostly works with effort" in 2025:

- **Protocol foundation**: Wayland color management protocol merged (Feb 2025)
- **Compositor support**: KDE Plasma (experimental), GNOME 48 (basic), Hyprland (experimental)
- **GPU drivers**: AMD (Mesa 25.1+) and NVIDIA (595.x+) support Vulkan HDR extensions
- **Application support**: Chromium, Firefox (experimental), SDL, MPV
- **Gaming**: Via gamescope `--hdr-enabled` or Wine/DXVK with `DXVK_HDR=1`

**Projection (2027)**: HDR becomes a first-class desktop feature on KDE Plasma with AMD. GNOME follows 6-12 months later. Gamescope remains the most reliable path for game-specific HDR through 2027.

**CrossHook impact**: HDR launch configuration is a high-value feature for CrossHook. If gamescope is the primary HDR enabler, CrossHook needs reliable gamescope access -- strengthening the bundling argument.

**Confidence**: Medium-High

### 7.2 VRR/FreeSync

VRR is more mature than HDR:

- Gamescope supports VRR natively
- KDE Plasma supports VRR on Wayland
- GNOME Mutter has basic VRR support

**Projection**: VRR becomes a non-issue by 2027 as all major compositors support it natively. Gamescope is no longer the only path to VRR.

**Confidence**: High

### 7.3 Multi-Monitor Wayland Gaming

Multi-monitor gaming on Wayland remains challenging. Gamescope's nested compositor approach naturally handles single-display gaming, but multi-monitor setups require compositor-level support.

**Projection**: This improves incrementally but remains a pain point through 2027. Gamescope's "nested single display" model actually sidesteps the problem.

**Confidence**: Medium

---

## 8. The Open Gaming Collective (OGC) Factor

The [OGC](https://opengamingcollective.org/), formed January 2026, unifies kernel patches, input tooling, and gaming packages across Bazzite, ASUS Linux, ShadowBlip, PikaOS, Fyra Labs, ChimeraOS, Nobara, and Playtron.

### Key Deliverables

- **Shared OGC Kernel**: Unified kernel with gaming patches, [upstream-first philosophy](https://www.howtogeek.com/these-gaming-linux-distros-are-teaming-up-to-fix-bigger-problems/)
- **Gamescope fork**: Expanded hardware support for handhelds and desktop GPUs
- **InputPlumber**: Replacing HandyGCCS/HandHeld Daemon for controller management
- **Shared Mesa/Vulkan patches**: Fewer regressions across distributions

### Notable Absences

CachyOS [declined to join](https://www.kitguru.net/gaming/joao-silva/open-gaming-collective-ogc-formed-to-unify-linux-gaming/), citing concerns about bureaucracy and motivations.

**Projection (2027-2028)**: The OGC becomes the coordination body for gaming-distro tool packaging. If the OGC standardizes a gamescope package (version, patches, build flags), CrossHook can target that standard rather than dealing with per-distro variation.

**CrossHook impact**: The OGC reduces fragmentation in the tools CrossHook needs (gamescope, input handling). This slightly weakens the bundling argument (if host tools converge, detection becomes simpler) but doesn't eliminate it (immutable distro users still can't install missing tools).

**Confidence**: Medium (the OGC is new; its long-term effectiveness is unproven)

---

## 9. Wayland Transition Completion

### 9.1 X11 Session Removal Timeline

| Desktop Environment | X11 Session Removal                                                                          | X11 Code Removal                                                                                                            | Source                         |
| ------------------- | -------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------- | ------------------------------ |
| GNOME               | GNOME 49 (disables by default)                                                               | [GNOME 50 (March 2026)](https://canartuc.medium.com/gnome-completely-drops-x11-support-the-wayland-era-begins-387e961926c0) | Mutter commits merged Nov 2025 |
| KDE Plasma          | [Plasma 6.8 (~Oct 2026)](https://blogs.kde.org/2025/11/26/going-all-in-on-a-wayland-future/) | TBD (6.7 X11 supported through early 2027)                                                                                  | KDE blog announcement          |

**User adoption**: 70-80% of KDE users already on Wayland as of late 2025.

### 9.2 Implications for CrossHook

The Wayland transition affects CrossHook in several ways:

1. **XWayland becomes the compatibility layer**: X11 apps still work through XWayland, but new features (HDR, improved sync) require native Wayland support
2. **Gamescope's role shifts**: From "Wayland bridge for X11 games" to "specialized gaming microcompositor for HDR/scaling/VRR"
3. **Wine Wayland matures**: Games may eventually run natively on Wayland without XWayland, but Proton's official support lags upstream Wine
4. **Flatpak portals become critical**: Without X11's permissive model, sandboxed apps must use portals for screen capture, input, and display features

**Confidence**: High (both major DEs have announced firm timelines)

---

## 10. Flathub Growth Trajectory

Flathub's [2025 Year in Review](https://flathub.org/en/year-in-review/2025) shows explosive growth:

- **433.5 million downloads** in 2025 (20.3% increase over 2024)
- **3,243 total apps** (440 new in 2025)
- **723.9 million app updates**
- Growth from 27.3M (2020) to 433.5M (2025) -- a 15x increase in five years
- Bottles (Wine prefix manager) was the most downloaded utility at 1.6M downloads
- Gaming emulators (RetroArch, Dolphin) and store frontends (Steam, Heroic) rank among top downloads

**Projection (2027)**: Flathub reaches 700M+ annual downloads. Gaming-related apps continue to be a major category. The platform's policies and review processes become increasingly important for app discoverability and trust.

**CrossHook impact**: A Flathub presence is effectively mandatory for reaching the growing Linux gaming audience. The platform's policies on permissions, extensions, and tool bundling directly constrain CrossHook's architecture choices.

**Confidence**: High

---

## 11. Strategic Projections for CrossHook

### 11.1 Scenario Matrix (2027-2028)

| Scenario                                      | Probability       | Bundling Implication                                                     |
| --------------------------------------------- | ----------------- | ------------------------------------------------------------------------ |
| Immutable distros dominate gaming             | High (70%)        | **Strong bundle**: Users can't install host tools                        |
| Gamescope absorbed by desktop compositors     | Low-Medium (25%)  | **Weaker bundle**: Less need to ship gamescope                           |
| umu-launcher handles all Proton orchestration | High (80%)        | **Mixed**: Less bundling of Proton tools, but umu itself needs detection |
| Flatpak portal covers all gaming input needs  | Low (15%)         | **Weaker bundle**: Less need for broad device access                     |
| OGC standardizes gaming tool packaging        | Medium (50%)      | **Mixed**: Easier host detection, but immutable users still need bundles |
| SteamOS expands to desktops                   | Medium-High (60%) | **Strong bundle**: Read-only rootfs can't host extra tools               |
| Wine Wayland eliminates XWayland for gaming   | Medium (40%)      | **Neutral**: Changes compositor needs but not tool dependency model      |

### 11.2 What CrossHook Should Prepare For

**Near-term (2026-2027)**:

1. **Design for Flatpak-first users**: Assume the majority of new users cannot easily install host packages. The `flatpak-spawn --host` bridge should be a fallback, not the primary strategy.
2. **Leverage extensions over monolithic bundling**: Use Flatpak extension points for optional tools (gamescope, mangohud) rather than shipping everything in the base package.
3. **Integrate with umu-launcher**: As umu becomes the standard Proton bridge, align CrossHook's launch path with umu rather than maintaining independent Proton discovery.
4. **Track the gamescope Vulkan layer extension**: Monitor `org.freedesktop.Platform.VulkanLayer.gamescope` for stability improvements, as this is the blessed Flatpak path for gamescope access.

**Medium-term (2027-2028)**: 5. **Prepare for nested sandboxing**: When systemd-appd lands, CrossHook could use nested sandboxes for trainer isolation, which is both a security feature and a UX improvement. 6. **Watch for gaming input portals**: If issue #536 results in a proper gaming input portal, CrossHook can reduce its permission footprint. 7. **Adapt to compositor HDR**: As desktop compositors gain native HDR, CrossHook's gamescope dependency may shift from "required for HDR" to "optional for advanced scenarios."

**Long-term (2028-2029)**: 8. **Plan for a Wayland-native world**: All new compositor features will be Wayland-only. Any X11-dependent code paths in CrossHook become legacy. 9. **Consider OGC alignment**: If the OGC standardizes gaming tool packages, CrossHook's Flatpak could declare them as optional dependencies or extensions, simplifying the bundling problem.

---

## 12. Key Uncertainties and Gaps

1. **systemd-appd timeline**: No shipped code, no firm timeline. Nested sandboxing may be 2+ years away.
2. **Valve's Flatpak stance**: Valve distributes Steam as a .deb and has not embraced Flatpak officially. The community Flatpak exists but isn't Valve-endorsed. If Valve eventually endorses Flatpak Steam, the ecosystem dynamics change significantly.
3. **OGC durability**: The collective is 3 months old. Whether it survives governance disputes and maintains momentum is unknown.
4. **GPU virtualization feasibility for gaming**: Performance overhead is unclear; likely too high for latency-sensitive gaming.
5. **Proton native Wayland timeline**: No official Valve commitment despite Wine 10/11 progress.
6. **Flathub policy evolution**: How Flathub will handle gaming apps that need broad permissions (`--device=all`, `--filesystem=host`) as they tighten standards is unclear.

---

## Sources

### Flatpak Platform

- [Flatpak Happenings - Sebastian Wick's Blog](https://blog.sebastianwick.net/posts/flatpak-happenings/) (Nov 2025)
- [The Future of Flatpak - LWN.net](https://lwn.net/Articles/1020571/) (2025)
- [systemd-appd - Phoronix](https://www.phoronix.com/news/systemd-appd-Flatpak-Dev) (2025)
- [SO_PEERPIDFD Gets More Useful - Sebastian Wick](https://blog.sebastianwick.net/posts/so-peerpidfd-gets-more-useful/) (Oct 2025)
- [Improving Flatpak Graphics Drivers - Sebastian Wick](https://blog.sebastianwick.net/posts/flatpak-graphics-drivers/) (Jan 2026)
- [Flatpak Exploring GPU Virtualization - Phoronix](https://www.phoronix.com/news/Flatpak-GPU-Virtualization) (2026)
- [XDG Desktop Portal 1.19.1 - Phoronix](https://www.phoronix.com/news/XDG-Desktop-Porta-1.19.1) (Dec 2024)
- [Joystick/Game Controller Portal Issue #536](https://github.com/flatpak/xdg-desktop-portal/issues/536) (ongoing)
- [Flatpak 32-bit Extension Dropped - GNOME Blog](https://blogs.gnome.org/alatiera/2025/10/13/flatpak-32bit/) (Oct 2025)
- [RetroDECK Layered Architecture](https://retrodeck.readthedocs.io/en/latest/blog/2025/11/25/november-2025-finally-a-fatpak/) (Nov 2025)
- [Flatpak 1.16.4 Security Fix](https://www.webpronews.com/flatpak-1-16-4-patches-a-silent-security-flaw-that-let-sandboxed-apps-peek-outside-their-walls/) (Jun 2025)
- [The Future of Flatpak - Hacker News](https://news.ycombinator.com/item?id=44068400) (Jun 2025)
- [Flathub 2025 Year in Review](https://flathub.org/en/year-in-review/2025)
- [Flathub 435 Million Downloads - Linuxiac](https://linuxiac.com/flathub-sees-over-435-million-downloads-in-2025/) (2025)

### Wine/Proton

- [Wine 10.0 Released - Phoronix](https://www.phoronix.com/news/Wine-10.0-Released) (Jan 2025)
- [Wine 10.3 Released - GamingOnLinux](https://www.gamingonlinux.com/2025/03/wine-10-3-released-with-clipboard-support-in-the-wayland-driver-initial-vulkan-video-decoder-support/) (Mar 2025)
- [Wine 11.0 Stable Release - BigGo News](https://biggo.com/news/202601151322_Wine-11-Stable-Release-Performance-Compatibility-Gains) (Jan 2026)
- [Wine 11.0 NTSync - Windows Forum](https://windowsforum.com/threads/wine-11-0-ntsync-wow64-overhaul-and-wayland-updates-for-better-linux-gaming.406839/) (Jan 2026)
- [Proton Native Wayland Issue #4638](https://github.com/ValveSoftware/Proton/issues/4638) (ongoing)
- [Valve Pressure Vessel - GamingOnLinux](https://www.gamingonlinux.com/2020/10/valve-put-their-pressure-vessel-container-source-for-linux-games-up-on-gitlab/)

### Compositor / Display

- [Gamescope - ArchWiki](https://wiki.archlinux.org/title/Gamescope)
- [Gamescope Architecture - DeepWiki](https://deepwiki.com/ValveSoftware/gamescope/2-architecture)
- [Gamescope Vulkan Layer Extension - Flathub](https://github.com/flathub/org.freedesktop.Platform.VulkanLayer.gamescope)
- [Wayland Color Management Protocol Merged - GamingOnLinux](https://www.gamingonlinux.com/2025/02/wayland-colour-management-and-hdr-protocol-finally-merged/) (Feb 2025)
- [HDR on Linux with KDE 6.3 - CatWithCode](https://catwithcode.moe/Blog/2025.04.17_Linux_HDR_Ultrawide/Linux_HDR_Ultrawide.html) (Apr 2025)
- [KDE Going All-In on Wayland - KDE Blog](https://blogs.kde.org/2025/11/26/going-all-in-on-a-wayland-future/) (Nov 2025)
- [GNOME Drops X11 - Can Artuc / Medium](https://canartuc.medium.com/gnome-completely-drops-x11-support-the-wayland-era-begins-387e961926c0) (Nov 2025)
- [Wayland Q1 2025 Exciting - Phoronix](https://www.phoronix.com/news/Wayland-Q1-2025) (2025)
- [NVIDIA Gamescope Support - Phoronix](https://www.phoronix.com/news/Gamescope-NVIDIA-Pending)

### Immutable Distros

- [Best Linux Distros for Gaming 2026 - InnovariaTech](https://innovariatech.blog/best-linux-distros-for-gaming-in-2026/)
- [Immutable Linux Distros Rise - Can Artuc / Medium](https://canartuc.medium.com/immutable-linux-distributions-rise-fedora-silverblue-and-the-future-of-desktop-stability-36e968e5befb)
- [Immutable Distros Solve a Problem - XDA Developers](https://www.xda-developers.com/immutable-linux-distros-solve-a-problem-most-home-users-dont-have/)
- [Personal Reflections on Immutable Linux - Hackaday](https://hackaday.com/2025/07/10/personal-reflections-on-immutable-linux/) (Jul 2025)
- [NixOS Flatpak Integration - nix-flatpak](https://github.com/gmodena/nix-flatpak)
- [NixOS Flatpak PR #347605](https://github.com/NixOS/nixpkgs/pull/347605)

### umu-launcher

- [umu-launcher GitHub](https://github.com/Open-Wine-Components/umu-launcher)
- [umu-launcher First Release - GamingOnLinux](https://www.gamingonlinux.com/2024/10/unified-linux-wine-game-launcher-umu-gets-a-first-official-release/) (Oct 2024)
- [Lutris 0.5.20 Release - Patreon](https://www.patreon.com/posts/lutris-0-5-20-150945758) (Feb 2026)
- [umu Integration in Bottles - Issue #3537](https://github.com/bottlesdevs/Bottles/issues/3537)
- [umu-launcher FAQ Wiki](<https://github.com/Open-Wine-Components/umu-launcher/wiki/Frequently-asked-questions-(FAQ)>)

### Valve / Steam Deck / SteamOS

- [Linux Crosses 5% on Steam - WebProNews](https://www.webpronews.com/linux-crosses-the-5-threshold-on-steam-and-this-time-it-might-actually-matter/) (Jun 2025)
- [Linux 3.20% Steam Nov 2025 - WebProNews](https://www.webpronews.com/linux-reaches-record-3-20-share-in-steams-november-2025-survey/) (Nov 2025)
- [Linux Gaming Feb 2026 Stagnation - WebProNews](https://www.webpronews.com/linux-gaming-hits-a-wall-steams-february-2026-survey-reveals-a-platform-at-a-crossroads/) (Feb 2026)
- [Steam Deck Year of Linux Desktop - XDA](https://www.xda-developers.com/steam-deck-year-linux-desktop/)
- [SteamOS Wikipedia](https://en.wikipedia.org/wiki/SteamOS)
- [SteamOS General Release - XDA](https://www.xda-developers.com/valve-steamos-general-release-preview/) (May 2025)
- [Steam Machine 2026 - Engadget](https://www.engadget.com/gaming/pc/valves-steam-machine-launches-in-2026-everything-we-know-so-far-200458597.html) (2026)
- [SteamOS 3.8 Update - GameRant](https://gamerant.com/steam-os-update-patch-notes-march-2026/) (Mar 2026)
- [Steam Machine Announcement - Phoronix](https://www.phoronix.com/news/Steam-Machines-Frame-2026) (Nov 2025)
- [SteamOS Non-Deck Handhelds - PC Gamer](https://www.pcgamer.com/hardware/big-update-to-steamos-improves-support-for-non-valve-handhelds-newer-platforms-discrete-gpus-and-steam-machine/) (2026)

### Open Gaming Collective

- [OGC Formation - GamingOnLinux](https://www.gamingonlinux.com/2026/01/open-gaming-collective-ogc-formed-to-push-linux-gaming-even-further/) (Jan 2026)
- [OGC Official Site](https://opengamingcollective.org/)
- [OGC Announcement - XDA Developers](https://www.xda-developers.com/bazzite-reveals-the-open-gaming-collective-to-make-gaming-on-linux-even-better/) (Jan 2026)
- [OGC Founding Members - VideoCardz](https://videocardz.com/newz/bazzite-and-asus-linux-shadowblip-pikaos-fyra-labs-launch-open-gaming-collective) (Jan 2026)
- [OGC Unification - HowToGeek](https://www.howtogeek.com/these-gaming-linux-distros-are-teaming-up-to-fix-bigger-problems/) (2026)
- [OGC Impact Analysis - FinalBoss.io](https://finalboss.io/linux-gaming-unification-the-open-gaming-collective-and) (2026)
- [CachyOS Declines OGC - KitGuru](https://www.kitguru.net/gaming/joao-silva/open-gaming-collective-ogc-formed-to-unify-linux-gaming/) (Jan 2026)

---

## Search Queries Executed

1. "Flatpak roadmap 2025 2026 gaming features portal API improvements"
2. "Wine Wayland driver 2025 2026 stability timeline Wine 10 features gaming"
3. "gamescope compositor evolution 2025 2026 Flatpak nested Wayland HDR VRR"
4. "immutable Linux distro adoption 2025 2026 Fedora Atomic Universal Blue Bazzite gaming trends"
5. "umu-launcher evolution 2025 2026 Flatpak packaging Proton integration roadmap"
6. "Valve Steam Deck 2025 2026 SteamOS desktop Linux market share open source influence"
7. "Flatpak sandbox relaxation tightening 2025 2026 device access permissions gaming"
8. "HDR Linux gaming 2025 2026 KDE GNOME Wayland compositor native support timeline"
9. "SteamOS 3.6 desktop release 2025 2026 general availability non-Deck hardware"
10. "Flatpak extension mechanism gaming runtime shared libraries 2025"
11. "pressure-vessel Steam Linux Runtime container 2025 2026 evolution Proton"
12. "NixOS Flatpak integration 2025 immutable distro gaming package management"
13. "Flatpak gaming portal GPU access 2025 2026 xdg-desktop-portal gaming features"
14. "Wayland XWayland deprecation timeline 2025 2026 KDE GNOME X11 removal"
15. "gamescope rewrite 2025 2026 Valve restructure modular architecture future"
16. "Open Gaming Collective 2025 2026 Linux gaming collaboration shared kernel"
17. "Flatpak nested sandboxing systemd-appd 2025 2026 Sebastian Wick blog post"
18. "Wine Proton Wayland native gaming without XWayland 2026 timeline progress"
19. "Flathub 2025 year review statistics growth gaming apps downloads"
