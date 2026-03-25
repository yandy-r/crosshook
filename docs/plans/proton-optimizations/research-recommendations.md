# Recommendations: proton-optimizations

## Executive Summary

CrossHook should ship this as a curated launch-optimization layer, not a freeform launch-option editor. The safest implementation is a structured profile submodel that stores named toggles, converts them to a deterministic backend launch plan, and auto-saves only the optimization slice after a short debounce. The initial release should focus on a small set of broadly useful, low-risk options for Proton-backed profiles, while clearly separating Proton-CachyOS-only and experimental toggles that can break overlay, input, or Wayland behavior.

The research points to three sources of truth: Proton’s runtime config model in the upstream Proton README, MangoHud’s documented `mangohud %command%` wrapper, and GameMode’s `gamemoderun %command%` wrapper. CachyOS adds a larger set of Proton-CachyOS-specific environment variables, but those should be treated as advanced because they are not universal Proton behavior and several are vendor-, GPU-, or compositor-specific. The implementation should keep Steam vs direct Proton launch paths opaque to the user and translate the same profile options into the correct env/wrapper sequence in the backend.

### Recommended Implementation Strategy

- Add a dedicated `launch.optimizations` section to `GameProfile` rather than mixing these settings into `steam`, `runtime`, or the raw launch method string. That keeps launch tuning logically separate from game identity and Proton path configuration.
- Model the feature as a fixed enum/boolean catalog in the frontend and backend. Do not expose a raw command prefix text box.
- Persist only structured choices such as `showMangoHud`, `useGameMode`, `disableSteamInput`, `preferSDL`, `disableWindowDecorations`, and advanced flags like `enableHDR` or `useNTSync`.
- Auto-save optimization changes with a debounce, but do not reload the profile after every toggle. The current explicit save path is too heavy for checkbox-driven interaction because it refreshes profile state and metadata; that behavior should stay for manual Save, not for every small toggle.
- Apply launch translation in Rust, not in React. The frontend should only update profile state; the backend should build the final env map and wrapper chain for `steam_applaunch` and `proton_run`.
- Hide or disable the whole section for `native` launch profiles. These options are Proton/Steam launch concerns and should not clutter Linux-native execution.
- Use human labels and short helper text. Example: `Disable Steam Input`, `Prefer SDL controller handling`, `Show MangoHud overlay`, `Use GameMode`, `Enable HDR`, with the env var name shown only in secondary text or an advanced details line.

## Recommended Option Catalog

| Group         | User-facing label                        | Backend mapping                | Ship in v1                    | Notes                                                                                                                    |
| ------------- | ---------------------------------------- | ------------------------------ | ----------------------------- | ------------------------------------------------------------------------------------------------------------------------ |
| Input         | `Disable Steam Input`                    | `PROTON_NO_STEAMINPUT=1`       | Yes                           | Good first-line fix for controller/input weirdness on Proton-CachyOS; documented in CachyOS gaming guidance.             |
| Input         | `Prefer SDL controller handling`         | `PROTON_PREFER_SDL=1`          | Yes                           | Also documented by CachyOS; useful when controller detection is inconsistent.                                            |
| Presentation  | `Hide window decorations`                | `PROTON_NO_WM_DECORATION=1`    | Yes                           | Low-risk quality-of-life toggle for borderless/fullscreen issues.                                                        |
| Overlay       | `Show performance overlay`               | `mangohud %command%`           | Yes                           | Use the documented wrapper form from MangoHud, not a raw env var editor.                                                 |
| Performance   | `Use GameMode`                           | `gamemoderun %command%`        | Yes                           | Upstream GameMode documents Steam launch-option usage directly.                                                          |
| Graphics      | `Enable HDR`                             | `PROTON_ENABLE_HDR=1`          | No, advanced                  | Keep behind an advanced disclosure because it has setup prerequisites and can be compositor/GPU dependent.               |
| Graphics      | `Auto-upgrade FSR4`                      | `PROTON_FSR4_UPGRADE=1`        | No, advanced                  | Proton-CachyOS-specific; useful, but not universal Proton behavior.                                                      |
| Graphics      | `Use RDNA3-optimized FSR4`               | `PROTON_FSR4_RDNA3_UPGRADE=1`  | No, advanced                  | Only relevant for RDNA3; should be nested under an AMD-specific advanced group.                                          |
| Graphics      | `Auto-upgrade XeSS`                      | `PROTON_XESS_UPGRADE=1`        | No, advanced                  | Best treated as GPU/vendor-specific.                                                                                     |
| Graphics      | `Auto-upgrade DLSS`                      | `PROTON_DLSS_UPGRADE=1`        | No, advanced                  | Useful on NVIDIA, but not a v1 default.                                                                                  |
| Graphics      | `Show DLSS indicator`                    | `PROTON_DLSS_INDICATOR=1`      | No, advanced                  | Cosmetic/diagnostic rather than performance-critical.                                                                    |
| Graphics      | `Enable NVIDIA game libraries`           | `PROTON_NVIDIA_LIBS=1`         | No, advanced                  | Vendor-specific and easy to misapply.                                                                                    |
| Compatibility | `Use Steam Deck compatibility mode`      | `SteamDeck=1`                  | No, advanced/excluded from v1 | This is game-specific and community-driven, not a general optimization. Keep hidden unless the user explicitly wants it. |
| Runtime       | `Use native Wayland support`             | `PROTON_ENABLE_WAYLAND=1`      | No, advanced                  | CachyOS documents this as experimental and warns about overlay breakage.                                                 |
| Runtime       | `Use NTSync`                             | `PROTON_USE_NTSYNC=1`          | No, advanced                  | Beneficial on supported kernels, but still experimental enough to avoid v1.                                              |
| Runtime       | `Enable per-game shader cache isolation` | `PROTON_LOCAL_SHADER_CACHE=1`  | No, advanced                  | Helpful, but subtle enough that it should not crowd the first release.                                                   |
| Runtime       | `Enable AMD Anti-Lag`                    | `ENABLE_LAYER_MESA_ANTI_LAG=1` | No, advanced                  | Vendor-specific and should be conditional on AMD hardware detection.                                                     |
| Overlay       | `Use CachyOS game-performance wrapper`   | `game-performance %command%`   | No, advanced                  | Useful on CachyOS, but distro-specific and should only appear when installed/detected.                                   |

Recommended exclusions for v1:

- Freeform shell prefixes or arbitrary launch strings.
- Diagnostic flags like `PROTON_LOG`.
- Risky blanket `LD_PRELOAD` overrides.
- Low-level incompatibility workarounds that are better left to expert users unless tied to a specific supported profile or detection rule.

## Phased Scope

1. Phase 1: Ship the core Proton-friendly presets.
   - Add `launch.optimizations` to the profile model.
   - Expose the v1 set: Steam Input, SDL preference, window decoration suppression, MangoHud, and GameMode.
   - Auto-save the optimization group with debounce and no explicit Save click.
   - Apply the resulting env/wrapper plan for both `steam_applaunch` and `proton_run`.

2. Phase 2: Add advanced, gated options.
   - Add HDR, NTSync, Wayland, shader-cache, and vendor-specific graphics toggles.
   - Gate CachyOS-only or vendor-specific entries behind detection and short help text.
   - Add contextual warnings for options known to affect Steam Overlay or controller behavior.

3. Phase 3: Add launch-context intelligence.
   - Show wrapper choices only when the required binary/package is present.
   - Add GPU/vendor awareness so NVIDIA, AMD, and Intel-specific options are not equally prominent.
   - Consider presets such as `Input`, `Overlay`, `Performance`, and `Graphics` if the single-toggle layout feels too wide on Steam Deck displays.

### Risk Mitigations

- Keep the UI opinionated. The feature should never become a shell command builder, because that would reintroduce the exact ambiguity this feature is trying to remove.
- Use a deterministic wrapper order in the backend. If multiple wrappers are allowed, define the order centrally and keep it hidden from the user.
- Treat `game-performance` and `gamemoderun` as mutually exclusive or at least strongly discourage enabling both together, since both are performance wrappers and may conflict conceptually.
- Warn when selecting options that can break overlay or input behavior, especially `PROTON_ENABLE_WAYLAND=1`, `SteamDeck=1`, and some HDR paths.
- Scope autosave narrowly. Save only the optimization changes, debounce writes, and avoid refreshing profile metadata on every toggle.
- Degrade gracefully when a feature is unavailable. If CachyOS-specific Proton variants or wrapper binaries are not present, disable the option with an explanation instead of failing launch time.
- Keep native launch profiles clean. Showing Proton-only toggles there invites confusion and expands the support surface without value.

## Open Decisions

- Should `SteamDeck=1` be hidden entirely in v1, or exposed only in an advanced section with a strong warning?
- Should `game-performance` appear as a separate wrapper option, or should CrossHook only ship upstream `gamemoderun` and leave CachyOS-specific tooling to later phases?
- Should autosave persist the full profile file on each optimization change, or should CrossHook add a lighter save path for the optimization subtree only?
- Should CrossHook detect Proton-CachyOS, NVIDIA, AMD, and Intel variants automatically and hide unsupported toggles, or should it always show the full catalog with disabled states?
- Should optimization presets be stored as individual booleans, or as named presets that expand to a fixed set of toggles behind the scenes?
