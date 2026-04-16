# Contrarian Analysis: The Case Against Bundling Tools in CrossHook's Flatpak

> **Perspective**: Devil's advocate / contrarian thinker
> **Date**: 2026-04-15
> **Status**: Phase 1 research

---

## Executive Summary

Bundling external tools (winetricks, winecfg, mangohud, gamescope, gamemode, umu-launcher, proton-manager) inside CrossHook's Flatpak would be **architecturally contradictory, technically problematic, and operationally burdensome**. CrossHook is a thin orchestrator that delegates all execution to the host via `flatpak-spawn --host`. Bundling the very tools it delegates to the host creates a circular dependency, inflates the attack surface, and shifts maintenance burden from distro packagers onto CrossHook maintainers — all for tools that either **cannot work inside a sandbox** (gamescope), **already work without bundling** (GameMode via D-Bus portal), or **must match the host's Wine/Proton** (winetricks, winecfg).

**Confidence**: High — arguments are grounded in CrossHook's actual architecture, Flatpak's documented sandbox model, and reproducible package data.

---

## Argument 1: Architectural Mismatch — Orchestrators Don't Execute

### The Core Contradiction

CrossHook's architecture is explicitly designed as a **host-delegating orchestrator**. Every external tool invocation goes through `flatpak-spawn --host`. The codebase proves this — `platform.rs` contains the `host_command_with()` and `host_command_with_env()` functions that wrap every command with `flatpak-spawn --host` when running inside Flatpak, and directly execute via `Command::new()` otherwise.

```rust
// From platform.rs — the actual architecture
fn host_command_with(program: &str, flatpak: bool) -> Command {
    if flatpak {
        let mut cmd = Command::new("flatpak-spawn");
        cmd.arg("--host").arg(program);
        cmd
    } else {
        Command::new(program)
    }
}
```

**Bundling tools inside the Flatpak sandbox would mean shipping binaries that CrossHook never invokes directly.** The bundled winetricks would sit unused inside the sandbox while `flatpak-spawn --host winetricks` calls the host's winetricks. Or worse, the architecture would need to be forked into two paths: "use the bundled one" vs "use the host one" — doubling the code paths and test matrix.

### The Orchestrator Pattern

CrossHook follows the same pattern as VS Code's Flatpak: delegate to host tools via `flatpak-spawn --host`. [VS Code Flatpak's issue tracker documents](https://github.com/flathub/com.visualstudio.code.oss/issues/45) how this is the accepted approach — IDE Flatpaks don't bundle compilers, linters, or debuggers; they delegate to the host.

**Confidence**: High — derived directly from CrossHook's own source code and the documented Flatpak orchestrator pattern.

---

## Argument 2: Binary Size Explosion

### Per-Tool Size Analysis (from `pacman -Si` on CachyOS)

| Tool           | Download Size | Installed Size | Key Dependencies                                                             |
| -------------- | ------------- | -------------- | ---------------------------------------------------------------------------- |
| gamescope      | ~1.4 MiB      | ~4.9 MiB       | 30+ libs (wayland, vulkan, libdrm, xcb, libinput, luajit, sdl2, xwayland...) |
| mangohud       | ~2.0 MiB      | ~10.5 MiB      | Vulkan layer + ICD loader                                                    |
| lib32-mangohud | ~1.5 MiB      | ~6.5 MiB       | 32-bit Vulkan counterpart                                                    |
| winetricks     | 160 KiB       | 875 KiB        | **Requires Wine** (~573+ MiB installed)                                      |
| gamemode       | ~78 KiB       | ~284 KiB       | D-Bus, systemd, polkit                                                       |
| umu-launcher   | ~369 KiB      | ~1.2 MiB       | Python, lib32 stack, vulkan drivers, 30+ deps                                |

### Cumulative Impact

The tools alone total **~24 MiB installed** — seemingly manageable. But the transitive dependency chain tells the real story:

- **Winetricks requires Wine**: Wine alone is 573+ MiB installed. Bundling winetricks without Wine is useless; bundling it with Wine explodes the Flatpak by **>600 MiB**.
- **Gamescope requires xorg-server-xwayland, libdrm, wayland, libinput, seatd**: These are system-level compositor dependencies totaling **hundreds of MiB**.
- **umu-launcher requires lib32 multilib stack**: `lib32-glibc`, `lib32-gcc-libs`, `lib32-vulkan-driver`, etc. — a 32-bit compatibility layer that can add **200+ MiB**.
- **MangoHud needs both 64-bit and 32-bit Vulkan layers**: Games may be either architecture.

A conservative estimate for bundling all tools with their dependencies: **800 MiB to 1.5 GiB** of additional Flatpak size, turning a lightweight orchestrator AppImage into a bloated mega-bundle.

Flatpak's OSTree deduplication helps if a user already has matching runtime versions — but CrossHook's users are gamers, not GNOME app users. The runtime overlap is minimal. As [Alexander Larsson's Flatpak blog post](https://blogs.gnome.org/alexl/2017/10/02/on-application-sizes-and-bloat-in-flatpak/) acknowledges: "bundling does generally increase size."

[Fedora users regularly complain](https://discussion.fedoraproject.org/t/how-to-solve-the-huge-disk-space-utilization-caused-by-flatpak-repository-and-dependencies/165197) about Flatpak disk usage exceeding the entire OS for just a few apps. Steam Deck users — a primary CrossHook audience — have notoriously limited storage.

**Confidence**: High — based on actual package sizes from the system package manager and documented Flatpak size behavior.

---

## Argument 3: Maintenance Burden Shifts to CrossHook

### Who Updates Bundled Tools?

When winetricks is a host package, the distro (Arch, Fedora, Ubuntu) patches it. Their security teams track CVEs, their build infrastructure rebuilds, and users get updates via `pacman -Syu` or `apt upgrade`.

The moment CrossHook bundles winetricks, **CrossHook's maintainers own that update cycle.** Every winetricks release, every Wine compatibility fix, every security patch — CrossHook must rebuild and push a new Flatpak. This is the pattern that causes [bundled Flatpak dependencies to lag months behind distro patches](https://www.linuxjournal.com/content/when-flatpaks-sandbox-cracks-real-life-security-issues-beyond-ideal).

### The Compounding Problem

Multiply this by 7 tools:

- winetricks: ~monthly releases
- gamescope: irregular Valve releases
- mangohud: active development, frequent releases
- gamemode: periodic releases
- umu-launcher: rapid development (v1.3.0+)
- proton-manager: unknown cadence
- winecfg: tied to Wine version

That's potentially **7+ update cycles** CrossHook must track, test, and ship — on top of its own development. For an open-source project, this is a maintenance sinkhole.

### Evidence from the Wild

[The flatkill.org critique](https://flatkill.org/) documented that CVE-2018-11235 was patched in distros but remained unfixed in Flatpak VSCode, Android Studio, and Sublime Text for 4+ months. A [Linux Journal investigation](https://www.linuxjournal.com/content/when-flatpaks-sandbox-cracks-real-life-security-issues-beyond-ideal) found that "Flatpak runtimes often lag behind distro updates, so vulnerabilities in built-in libraries remain unpatched for months."

**Confidence**: High — documented pattern across the Flatpak ecosystem with concrete CVE examples.

---

## Argument 4: Version Conflicts — Bundled vs. Host Tools on the Same Prefix

### The Winetricks Paradox

Winetricks modifies Wine prefixes. CrossHook's users have Wine prefixes managed by the host's Proton/Wine. If a bundled winetricks (v20260125) modifies a prefix that was set up by the host's Wine (v9.x), version mismatches cause:

- **Windows version regression**: [Winetricks issue #2218](https://github.com/Winetricks/winetricks/issues/2218) documents how `vcrun2019` installation changes the prefix's Windows version from win10 to win7.
- **Architecture confusion**: [Winetricks issue #2084](https://github.com/Winetricks/winetricks/issues/2084) shows 64-bit/32-bit prefix mismatches causing outright failures.
- **Wrong prefix targeting**: [Winetricks issue #1442](https://github.com/Winetricks/winetricks/issues/1442) documents winetricks installing to the default prefix instead of the specified one.

### The Path Problem

Inside Flatpak, `WINEPREFIX` paths point to host filesystem locations (e.g., `/home/user/.local/share/Steam/...`). A bundled winetricks would need `--filesystem=host` to even reach these paths — punching a massive hole in the sandbox. Meanwhile, `flatpak-spawn --host winetricks` naturally operates in the host's filesystem context with the host's Wine.

### Winecfg: Same Problem, Worse

`winecfg` is literally part of Wine. Bundling a version of winecfg that doesn't match the host's Wine version is guaranteed to cause prefix corruption or at minimum misleading configuration displays.

**Confidence**: High — based on documented winetricks bugs and the fundamental mismatch between sandbox filesystem and host prefix locations.

---

## Argument 5: Flathub Review Friction

### More Binaries = More Scrutiny

[Flathub's review process](https://docs.flathub.org/docs/for-app-authors/maintenance) requires that all sources be declared, checksummed, and built without network access. Every bundled tool adds:

- A new source declaration in the Flatpak manifest
- Checksums that must be updated on each upstream release
- Build steps that the Flathub infrastructure must execute
- Moderation review when permissions change

[Flathub issue #5733](https://github.com/flathub/flathub/issues/5733) specifically calls for transparency about whether apps are built from source or bundle pre-built binaries. Bundling Wine ecosystem binaries — which are complex, C/C++ code with security implications — will invite **additional review scrutiny**.

### The Update Bottleneck

Flathub builds are approved "usually within 1-2 hours unless held in moderation." More bundled dependencies means more things for moderators to check, more potential for permission changes to be flagged, and longer times between upstream fix and user delivery. As of [Flathub's 2025 year in review](https://flathub.org/en/year-in-review/2025), the platform serves thousands of apps with a finite moderation team.

### Policy Direction

Flathub's stated direction is toward **source-built, minimal, well-sandboxed apps**: "Open source software must be built from source on trusted infrastructure. Applications must not depend on end-of-life runtimes. Sandbox holes must be phased out." ([Flathub safety overview](https://www.osnews.com/story/141777/flathub-safety-a-layered-approach-from-source-to-user/)). Bundling a Wine/Proton toolchain goes against this trajectory.

**Confidence**: Medium — Flathub policies are evolving and exact review timelines vary; the directional argument is solid but specific friction is hard to quantify.

---

## Argument 6: Gamescope Cannot Meaningfully Run Inside Flatpak

### Compositor Requires System-Level Access

Gamescope is a Wayland/Vulkan microcompositor that needs:

- **DRM/KMS access** for direct frame flipping
- **seatd / libseat** for session management
- **libinput** for input device access
- **xorg-server-xwayland** for X11 game compatibility

These are **compositor-level system resources** that Flatpak's sandbox fundamentally restricts. The [Arch Wiki's gamescope article](https://wiki.archlinux.org/title/Gamescope) documents how Flatpak gamescope fails to access NVIDIA's GBM backend, requires `--filesystem=host` for GPU access, and breaks with official Proton's nested sandbox.

### Documented Flatpak Gamescope Failures

- [Steam community reports](https://steamcommunity.com/app/221410/discussions/0/5733664933468711365/): Games launch with audio but no window/graphics when using Flatpak gamescope.
- [Gamescope Flatpak issue #6](https://github.com/flathub/org.freedesktop.Platform.VulkanLayer.gamescope/issues/6): The Flatpak package "does not work with nested sandboxes" used by official Proton.
- [Gamescope issue #2081](https://github.com/ValveSoftware/gamescope/issues/2081): DRM format modifier failures on NVIDIA under Wayland (Feb 2026).
- NVIDIA GBM backend workaround requires re-running `flatpak override --env=GBM_BACKENDS_PATH=...` on **every driver update**.

### The Verdict

Even Valve's own Flatpak gamescope extension (`com.valvesoftware.Steam.Utility.gamescope`) is semi-broken. CrossHook bundling gamescope would inherit all of these problems — plus the additional complexity of gamescope needing to compose frames for games launched via `flatpak-spawn --host`.

**Confidence**: High — multiple independent sources document gamescope's fundamental incompatibility with Flatpak's sandbox model.

---

## Argument 7: GameMode Already Works via D-Bus Portal — Bundling Is Redundant

### The Portal Solution Already Exists

GameMode 1.4 added explicit Flatpak support via the [`org.freedesktop.portal.GameMode`](https://flatpak.github.io/xdg-desktop-portal/docs/doc-org.freedesktop.portal.GameMode.html) XDG Desktop Portal. This portal:

- Automatically handles PID namespace translation between sandbox and host
- Is accessible to sandboxed apps **without any extra permissions** (portals under `org.freedesktop.portal.*` are whitelisted by default)
- Proxies requests to the host's `com.feralinteractive.GameMode` daemon

### Bundling GameMode Is Actively Harmful

GameMode's daemon (`gamemoded`) manages **system-wide** CPU governor settings, I/O schedulers, and GPU clock profiles. A bundled copy inside Flatpak would either:

1. **Fail to access system resources** — because the sandbox prevents kernel-level tuning
2. **Conflict with the host daemon** — two gamemoded instances fighting over CPU governors
3. **Duplicate functionality** — the portal already provides everything needed

As [Phoronix reported](https://www.phoronix.com/news/Feral-GameMode-1.4-Released), the portal-based approach is the **official, upstream-supported** method for Flatpak integration.

**Confidence**: High — GameMode's D-Bus portal is documented, implemented, and the official solution.

---

## Argument 8: Bundling Creates a _Different_ "Works on My Machine" Problem

### The Illusion of Consistency

Bundling proponents argue it eliminates "tool not found" errors. But bundling creates **new** divergence scenarios:

| Scenario                          | Without Bundling    | With Bundling                        |
| --------------------------------- | ------------------- | ------------------------------------ |
| User has newer host tool          | CrossHook uses it   | Ignored; stale bundled version used  |
| User has custom Wine build        | Works naturally     | Conflicts with bundled Wine          |
| Host tool has distro patches      | Inherited           | Missed; raw upstream used            |
| Tool needs kernel features        | Works (same kernel) | May target wrong kernel version      |
| User troubleshoots with host tool | Same binary         | Different binary, different behavior |

### The Support Nightmare

When a user reports "winetricks fails to install vcrun2019," the first question becomes: "Is it the bundled winetricks or the host winetricks?" Debugging doubles because there are now **two environments** where the tool exists with potentially different versions, different configurations, and different filesystem views.

### Real-World Precedent

[MangoHud's Flatpak integration](https://github.com/flightlessmango/MangoHud/issues/1275) suffers exactly this: Flatpak MangoHud doesn't communicate with system MangoHud, different launch methods (`mangohud %command%` vs `MANGOHUD=1`) behave differently, and [the old/new extension naming](https://github.com/flathub/com.valvesoftware.Steam.VulkanLayer.MangoHud/issues/7) creates confusion. The [pressure-vessel breakage](https://github.com/ValveSoftware/steam-runtime/issues/662) from March 2024 affected Flatpak Vulkan layers for weeks.

**Confidence**: High — the MangoHud/Steam Flatpak experience is well-documented and directly analogous.

---

## Argument 9: Security Surface Area Expansion

### More Code = More Attack Surface

Each bundled tool brings its own dependency tree into the sandbox:

- **Wine** (if bundled for winetricks): Massive C/C++ codebase implementing Windows NT APIs
- **Gamescope**: Compositor with DRM/Vulkan surface management
- **MangoHud**: Vulkan layer that intercepts rendering calls
- **umu-launcher**: Python runtime with network capabilities (downloads Steam Runtime)

### Flatpak's Own CVE History Shows the Risk

Recent Flatpak CVEs demonstrate that **more complexity inside the sandbox creates more escape vectors**:

- **CVE-2024-32462**: Portal parsing vulnerability allowed sandbox escape via crafted `.desktop` files ([GitHub advisory](https://github.com/flatpak/flatpak/security/advisories/GHSA-7hgv-f2j8-xw87))
- **CVE-2024-42472**: Symlink exploit in persistent directories broke sandbox isolation
- **CVE-2026-34078/34079** (April 2026): [Debian DSA-6207-1](https://linuxsecurity.com/advisories/debian/debian-dsa-6207-1-flatpak) — arbitrary file deletion and sandbox breakout

### The Patch Propagation Problem

[Nearly 42% of Flatpak apps](https://www.linuxjournal.com/content/when-flatpaks-sandbox-cracks-real-life-security-issues-beyond-ideal) override or misconfigure sandboxing. When bundled libraries have CVEs, the traditional distro model patches them system-wide in hours. Bundled libraries in Flatpaks [remain unpatched for months](https://flatkill.org/) until the app maintainer rebuilds.

CrossHook bundling Wine tools would make it responsible for tracking CVEs in Wine, winetricks, gamescope, mangohud, and their transitive dependencies — a security monitoring burden that distro security teams handle today at no cost to CrossHook.

**Confidence**: High — CVE evidence is concrete and the maintenance burden argument is structurally sound.

---

## Argument 10: CachyOS Kernel Optimizations Are Literally Unbundleable

### Kernel-Level Performance Is Where Gaming Wins

CrossHook targets gaming-focused Linux users. Many use CachyOS or similar distributions with kernel-level optimizations:

- **[BORE scheduler](https://wiki.cachyos.org/features/kernel/)**: Burst-Oriented Response Enhancer patches to EEVDF for "maximum interactivity" and "snappiness under load"
- **Architecture-specific builds**: x86-64-v3, x86-64-v4, Zen4 optimized kernels
- **LTO + AutoFDO + Propeller**: Clang-compiled kernels with profile-guided optimization
- **Gaming scheduler profiles**: Preset scheduler configurations for gaming workloads
- **Real-time support**: RT kernel builds with BORE integration
- **Sysctl tuning**: 70+ documented kernel parameter tweaks for desktop performance ([CachyOS Settings](https://github.com/CachyOS/CachyOS-Settings))
- **NVIDIA driver parameters**: Frame-pacing and PAT optimizations

### None of This Can Be Bundled

These optimizations exist at the kernel level, the scheduler level, and the driver level. No Flatpak bundle can ship a kernel, a scheduler, or a GPU driver. The performance gains that actually matter to CrossHook's audience come from the **host system**, not from anything a Flatpak can provide.

### The Implication

If the performance-critical components must come from the host, and the tools that interact with those components (gamescope for compositor-level frame management, gamemode for CPU governor tuning) also need host-level access, then **bundling is solving the wrong problem**. The real value is in graceful detection and guidance toward host-installed tools, not in creating a parallel universe inside the sandbox.

**Confidence**: High — kernel-level optimizations are by definition unbundleable; this is a structural fact, not an opinion.

---

## Synthesis: The Fundamental Counter-Position

### Why "Bundle Everything" Is the Wrong Mental Model

The bundling instinct comes from a solved problem in other domains: web apps bundle their dependencies to avoid "dependency hell." But CrossHook's situation is different:

1. **CrossHook doesn't execute these tools** — it delegates to them via `flatpak-spawn --host`
2. **The tools need host-level access** — compositor, kernel, D-Bus, Wine prefixes are all on the host
3. **Bundling doesn't eliminate the host dependency** — users still need Proton/Wine on the host
4. **The security/maintenance cost is real** — 7 tools × N dependencies × ongoing CVE tracking
5. **Better alternatives exist** — D-Bus portals (GameMode), host delegation (everything else), user guidance (missing tools)

### What CrossHook Should Do Instead

- **Detect host tools at startup**: Report what's available, what's missing, and how to install missing tools
- **Use D-Bus portals where available**: GameMode portal is already the correct integration path
- **Delegate everything else via `flatpak-spawn --host`**: This is already the architecture — lean into it
- **Provide install guidance**: Platform-specific install commands (`pacman -S gamescope`, `apt install mangohud`, etc.)
- **Accept that some tools are optional**: Not every user needs gamescope; not every user needs winetricks

---

## Sources

### Gamescope / Compositor Limitations

- [Gamescope - ArchWiki](https://wiki.archlinux.org/title/Gamescope)
- [Gamescope Flatpak nested sandbox issue #6](https://github.com/flathub/org.freedesktop.Platform.VulkanLayer.gamescope/issues/6)
- [Gamescope DRM format modifiers issue #2081](https://github.com/ValveSoftware/gamescope/issues/2081)
- [Gamescope not working in Flatpak Steam](https://steamcommunity.com/app/221410/discussions/0/5733664933468711365/)

### GameMode D-Bus Portal

- [XDG Desktop Portal - GameMode](https://flatpak.github.io/xdg-desktop-portal/docs/doc-org.freedesktop.portal.GameMode.html)
- [Feral GameMode 1.4 Flatpak support (Phoronix)](https://www.phoronix.com/news/Feral-GameMode-1.4-Released)
- [XDG Desktop Portal - GameMode PR #314](https://github.com/flatpak/xdg-desktop-portal/pull/314)

### Flatpak Size & Bloat

- [On application sizes and bloat in Flatpak (Alexander Larsson)](https://blogs.gnome.org/alexl/2017/10/02/on-application-sizes-and-bloat-in-flatpak/)
- [Why Flatpak Apps Use So Much Disk Space (OSTechNix)](https://ostechnix.com/why-flatpak-apps-use-so-much-disk-space/)
- [Fedora Discussion - Flatpak disk space](https://discussion.fedoraproject.org/t/how-to-solve-the-huge-disk-space-utilization-caused-by-flatpak-repository-and-dependencies/165197)

### Flathub Review & Policy

- [Flathub Maintenance docs](https://docs.flathub.org/docs/for-app-authors/maintenance)
- [Flathub source vs binary transparency issue #5733](https://github.com/flathub/flathub/issues/5733)
- [Flathub safety: a layered approach (OSnews)](https://www.osnews.com/story/141777/flathub-safety-a-layered-approach-from-source-to-user/)
- [Flathub 2025 Year in Review](https://flathub.org/en/year-in-review/2025)

### Security & CVEs

- [When Flatpak's Sandbox Cracks (Linux Journal)](https://www.linuxjournal.com/content/when-flatpaks-sandbox-cracks-real-life-security-issues-beyond-ideal)
- [Flatpak security nightmare (flatkill.org)](https://flatkill.org/)
- [CVE-2024-42472 (GitHub advisory)](https://github.com/flatpak/flatpak/security/advisories/GHSA-7hgv-f2j8-xw87)
- [Debian DSA-6207-1 (April 2026)](https://linuxsecurity.com/advisories/debian/debian-dsa-6207-1-flatpak)

### Winetricks Version Conflicts

- [Winetricks vcrun2019 version bug #2218](https://github.com/Winetricks/winetricks/issues/2218)
- [Winetricks 64-bit prefix issue #2084](https://github.com/Winetricks/winetricks/issues/2084)
- [Winetricks wrong prefix issue #1442](https://github.com/Winetricks/winetricks/issues/1442)

### MangoHud Flatpak Issues

- [MangoHud inconsistent Flatpak functionality #1275](https://github.com/flightlessmango/MangoHud/issues/1275)
- [MangoHud extension EOL confusion #7](https://github.com/flathub/com.valvesoftware.Steam.VulkanLayer.MangoHud/issues/7)
- [Pressure-vessel Vulkan layers broken #662](https://github.com/ValveSoftware/steam-runtime/issues/662)

### CachyOS Kernel

- [CachyOS Kernel Wiki](https://wiki.cachyos.org/features/kernel/)
- [CachyOS Settings (GitHub)](https://github.com/CachyOS/CachyOS-Settings)
- [BORE scheduler for SteamOS proposal](https://github.com/ValveSoftware/SteamOS/issues/1600)

### Flatpak Architecture

- [flatpak-spawn(1) man page](https://man7.org/linux/man-pages/man1/flatpak-spawn.1.html)
- [host-spawn alternative](https://github.com/1player/host-spawn)
- [VS Code Flatpak host tools issue #45](https://github.com/flathub/com.visualstudio.code.oss/issues/45)
- [Flatpak Sandbox Permissions docs](https://docs.flatpak.org/en/latest/sandbox-permissions.html)

### umu-launcher

- [umu-launcher (GitHub)](https://github.com/Open-Wine-Components/umu-launcher)
- [umu-launcher CachyOS package](https://packages.cachyos.org/package/cachyos/x86_64/umu-launcher)
- [umu-launcher Arch Linux package](https://archlinux.org/packages/multilib/x86_64/umu-launcher/)
