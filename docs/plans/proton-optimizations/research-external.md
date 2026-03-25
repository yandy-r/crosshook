# Research: proton-optimizations

## Executive Summary

CrossHook can support a launch-optimization panel, but the controls should be treated as three different execution types: Proton environment variables, Steam launch-option wrappers, and Steam/client-side flags. The most authoritative semantics come from Valve’s Proton README and Steam launch-option docs, while many of the newer knobs the user asked for are only documented in community-maintained Proton forks and distro guides. That means the UI should clearly label upstream-supported items versus community-documented extras, and it should warn on hardware- or compositor-dependent options like HDR, Wayland, NTSync, and wrapper ordering.

### Candidate APIs and Services

### Proton runtime configuration

- Source: [ValveSoftware/Proton](https://github.com/ValveSoftware/Proton)
- What it covers: Proton runtime options are set per launch by prepending `KEY=value` before `%command%`. Valve’s README also documents the global `user_settings.py` path for applying defaults to all games.
- Key point for CrossHook: this is the core model for profile-scoped options. It is safe to store these as structured profile data and render them into launch env vars at runtime.

### Steam launch-option semantics

- Source: [Valve Developer Community - Command line options (Steam)](https://developer.valvesoftware.com/wiki/Command_line_options_%28Steam%29) and [Command line options](https://developer.valvesoftware.com/wiki/Command_line_options)
- What it covers: Steam launch options are extra command-line arguments inserted before program start. `%command%` is the placeholder Steam substitutes with the real executable invocation.
- Key point for CrossHook: wrappers must be assembled as a single prefix chain with one `%command%` sentinel, not as multiple independent launch strings.

### Proton-CachyOS documented toggles

- Source: [CachyOS/proton-cachyos](https://github.com/CachyOS/proton-cachyos)
- What it covers: community-documented runtime toggles for `PROTON_ENABLE_HDR`, `PROTON_PREFER_SDL`, `PROTON_NO_STEAMINPUT`, `PROTON_NO_WM_DECORATION`, `PROTON_ENABLE_WAYLAND`, `PROTON_USE_NTSYNC`, `PROTON_FSR4_UPGRADE`, `PROTON_FSR4_RDNA3_UPGRADE`, `PROTON_DLSS_UPGRADE`, `PROTON_DLSS_INDICATOR`, `PROTON_XESS_UPGRADE`, and `PROTON_NVIDIA_LIBS`.
- Key point for CrossHook: these are the best source for the newer upsell-style switches, but they should be marked as community-maintained rather than Valve-official.

### MangoHud overlay

- Source: [flightlessmango/MangoHud](https://github.com/flightlessmango/MangoHud)
- What it covers: `mangohud %command%` is the documented Steam launch-option form, with `MANGOHUD=1` as a Vulkan-only alternative and `MANGOHUD_DLSYM=0` for some OpenGL cases.
- Key point for CrossHook: MangoHud is a wrapper, not a Proton env var, so it belongs in a command-prefix section.

### GameMode

- Source: [FeralInteractive/gamemode](https://github.com/FeralInteractive/gamemode)
- What it covers: `gamemoderun %command%` is the standard Steam launch-option form. GameMode is a daemon/lib combo that can request CPU governor, I/O, niceness, scheduler, and GPU performance changes.
- Key point for CrossHook: this is a viable alternative to distro-specific performance wrappers, but it should not be stacked blindly with other profile-switching wrappers.

### CachyOS `game-performance`

- Sources: [CachyOS-Settings](https://github.com/CachyOS/CachyOS-Settings) and [Gaming with CachyOS Guide](https://wiki.cachyos.org/configuration/gaming/)
- What it covers: `game-performance` is a CachyOS wrapper that switches the system to `performance` through `power-profiles-daemon`, then restores the previous profile when the game exits. CachyOS docs also show it being used in Steam as `game-performance %command%`.
- Key point for CrossHook: this is beneficial for CachyOS users, but it is distro-specific and should be treated as an optional wrapper, not a generic Proton feature.

### Steam Deck / deck-mode behavior

- Sources: [ValveSoftware/steam-for-linux issue #11947](https://github.com/ValveSoftware/steam-for-linux/issues/11947) and [GE-Proton releases](https://github.com/GloriousEggroll/proton-ge-custom/releases)
- What it covers: Steam Deck behavior is not a Proton env var in the Valve docs, but the community documents `SteamDeck=1` as a real launch-time environment signal that can change game behavior and UI defaults.
- Key point for CrossHook: this should be treated as an advanced, experimental compatibility flag, not a default optimization checkbox.

### Optional advanced candidates

- `PROTON_ENABLE_AMD_AGS=1`: community-reported HDR helper for some games on Deck and desktop, with evidence in [ValveSoftware/Proton issue #7292](https://github.com/ValveSoftware/Proton/issues/7292).
- `PROTON_ENABLE_WAYLAND=1`: documented in [CachyOS/proton-cachyos](https://github.com/CachyOS/proton-cachyos), but it carries Steam Input and overlay caveats.
- `PROTON_USE_NTSYNC=1`: documented in [CachyOS gaming guide](https://wiki.cachyos.org/configuration/gaming/) and GE-Proton release notes; requires kernel support and may still be experimental on some setups.

## Libraries and SDKs

| Component              | Type                          | Why it matters                                                                                   |
| ---------------------- | ----------------------------- | ------------------------------------------------------------------------------------------------ |
| Proton runtime options | Launch-time config surface    | CrossHook can represent these as typed profile metadata and render them into env vars at launch. |
| MangoHud               | Overlay runtime               | Best fit for FPS/frametime/status overlays and launch-prefix integration.                        |
| GameMode               | Daemon/lib + CLI wrapper      | Useful for a generic “performance mode” option via `gamemoderun %command%`.                      |
| `game-performance`     | Distro wrapper script         | Good for CachyOS-specific power-profile switching, but not portable enough to assume everywhere. |
| Steam launch options   | Client-side command expansion | Required for `%command%` composition and wrapper ordering.                                       |

## Integration Patterns

- Model each option as a descriptor with `id`, human label, help text, source URL, and execution type (`env`, `wrapper`, `steam_flag`).
- Keep wrappers separate from env vars in the UI. Examples:
  - env vars: `PROTON_ENABLE_HDR=1`, `PROTON_NO_STEAMINPUT=1`
  - wrappers: `mangohud`, `gamemoderun`, `game-performance`
  - client flags: `SteamDeck=1`
- Emit a single launch string with one `%command%` placeholder. Wrapper chains should be ordered explicitly rather than concatenated ad hoc.
- Only surface Proton/Steam-specific toggles for `steam_applaunch` and `proton_run` profiles. Hide or disable them for native Linux launch profiles.
- Persist changes automatically at toggle time, but debounce the save path so the UI does not write on every intermediate React state update.
- Consider mutually exclusive groups for performance wrappers. `gamemoderun` and `game-performance` solve the same problem in different ways, so the UI should not imply they are additive.
- Use hardware/context warnings inline for HDR, Wayland, and NTSync instead of burying them in help text.

## Constraints and Gotchas

- Valve’s Proton README makes the runtime options explicitly non-persistent for the prefix. CrossHook should keep them in profile data rather than editing Proton install files.
- Many of the newer toggles the user wants are community-maintained, not Valve-official. The UI should say that plainly to avoid overstating support.
- `PROTON_NVIDIA_LIBS` is incompatible with `PROTON_USE_WOW64=1`; CachyOS documents a `PROTON_NVIDIA_LIBS_NO_32BIT` variant for newer RTX cases.
- `PROTON_ENABLE_HDR` commonly depends on HDR-capable compositor/runtime support; CachyOS documents `gamescope --hdr-enabled` or `PROTON_ENABLE_WAYLAND=1`, and GE-Proton notes that Wayland can break Steam Input or the in-game overlay.
- `PROTON_FSR4_UPGRADE` and `PROTON_FSR4_RDNA3_UPGRADE` can download `amdxcffx64.dll` and may disable AMD Anti-Lag 2 in community builds. They are powerful but risky enough to deserve a warning label.
- `PROTON_DLSS_UPGRADE` can take a version string and also affects NVAPI preset behavior; `PROTON_DLSS_INDICATOR` / FSR4 watermark naming is inconsistent across community docs, so the UI should use a generic “show upscaler indicator” label.
- `MANGOHUD_DLSYM=0` may be needed for some OpenGL games, and some native launch scripts override `LD_PRELOAD`, which can break the overlay.
- `game-performance` may not help older CPUs, and CachyOS warns about conflicts with `ananicy-cpp`.
- `SteamDeck=1` is behavior-changing and can lock some games to deck-style behavior; it should not be presented as a default performance tuning option.

## Open Decisions

- Should CrossHook expose a small “recommended” set by default and tuck the rest behind an advanced expander, or should every toggle be visible up front?
- Should `SteamDeck=1` exist at all in CrossHook, or only as an advanced compatibility flag with a warning?
- Should performance wrappers be mutually exclusive choices (`GameMode` vs `game-performance`) instead of independent checkboxes?
- Should HDR be one toggle or a small cluster that includes `PROTON_ENABLE_HDR`, `PROTON_ENABLE_WAYLAND`, and the relevant Steam Deck / gamescope caveats?
- Should CrossHook include `PROTON_ENABLE_AMD_AGS` as an optional HDR helper even though the best evidence is a Valve issue report rather than official Proton docs?
