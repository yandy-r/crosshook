## Executive Summary

CrossHook does not need a new cloud API for this feature, but it does depend on a few external runtime contracts that materially affect the design. The most important ones are Valve's Proton compatibility-tool layout, `proton run` semantics, and `umu-launcher`'s documented environment-variable contract for launching Windows executables outside Steam with a managed runtime container.

The current codebase already discovers Proton installs and launches Proton executables directly for `proton_run`, so the lowest-risk integration is to keep CrossHook centered on the Proton executable path it already stores. `umu-run` is still a valuable reference because the existing shell script uses it and its docs clarify how `WINEPREFIX`, `GAMEID`, and `PROTONPATH` should behave when CrossHook wants a more Steam-like runtime environment later.

### Candidate APIs and Services

#### Valve Proton Compatibility Tools

- Documentation URL: <https://raw.githubusercontent.com/ValveSoftware/Proton/proton_10.0/README.md>
- Auth model: none
- Key capabilities:
  - Documents how local Proton builds are installed under `~/.steam/root/compatibilitytools.d/<tool>/`
  - Confirms a valid compatibility tool includes a `proton` executable plus supporting manifests/files
  - Establishes Steam's compatibility-tools directory structure, which aligns with CrossHook's current Proton discovery code
- Rate limits/quotas: none
- Pricing notes: none
- Implementation impact:
  - CrossHook should continue storing a resolved Proton executable path, because that is what its current runtime launcher consumes
  - Discovery should keep recognizing both Steam-managed and custom compatibility-tool roots

#### umu-launcher / umu-run

- Documentation URL: <https://raw.githubusercontent.com/Open-Wine-Components/umu-launcher/main/README.md>
- FAQ URL: <https://github.com/Open-Wine-Components/umu-launcher/wiki/Frequently-asked-questions-%28FAQ%29>
- Auth model: none
- Key capabilities:
  - Launches Windows executables on Linux using Proton inside a Steam-runtime-style container
  - Supports explicit `WINEPREFIX`, `GAMEID`, and `PROTONPATH`
  - Defaults the prefix path if `WINEPREFIX` is omitted, but allows callers to force a durable prefix path
  - Supports system-installed compatibility tools when `PROTONPATH` is explicitly set
- Rate limits/quotas: none
- Pricing notes: none
- Implementation impact:
  - The local `game-install.sh` script uses `SteamDeck=1`, `WINEPREFIX`, `GAMEID=0`, `PROTONPATH`, and `umu-run installer.exe`
  - `umu-run` expects `PROTONPATH` to identify the Proton directory/tool root, not the `proton` executable file
  - `umu-run` is optional for v1 because CrossHook already has a direct Proton execution path, but it is the clearest compatibility reference for a future containerized install path

#### GNOME HIG and W3C Accessibility Guidance

- Documentation URLs:
  - <https://developer.gnome.org/hig/patterns/feedback/progress-bars.html>
  - <https://developer.gnome.org/hig/patterns/feedback/spinners.html>
  - <https://developer.gnome.org/documentation/tutorials/beginners/components/file_dialog.html>
  - <https://www.w3.org/WAI/WCAG22/Techniques/general/G184>
- Auth model: none
- Key capabilities:
  - Progress guidance for short vs long-running tasks
  - Inline feedback recommendations instead of detached progress windows
  - Native file dialog guidance for desktop apps
  - Form-instruction guidance so users understand path conventions and required inputs before submission
- Rate limits/quotas: none
- Pricing notes: none
- Implementation impact:
  - The install sub-tab should use inline status and progress copy near the action controls
  - Prefix-path rules and installer expectations should be explained before install starts
  - File selection should keep using the native Tauri dialog path the app already uses

### Libraries and SDKs

- Rust / Tauri runtime:
  - Recommended package: no new frontend or backend SDK is required for v1
  - Rationale: the repo already has Tauri commands, dialog integration, Proton discovery, and direct Proton launch primitives
- Optional system tool:
  - Recommended package: `umu-run` only as an optional runtime enhancement, not a hard dependency for the first implementation
  - Rationale: making the feature depend on a user-installed binary would add packaging and support burden immediately, while the app already launches Proton directly
- Existing reusable dependencies:
  - `@tauri-apps/plugin-dialog` is already present for native file/directory selection
  - `tokio::process::Command` is already used for long-running launch helpers and can support installer execution

### Integration Patterns

- Recommended v1 launch pattern:
  - Reuse CrossHook's direct Proton execution model rather than shelling out to the existing script
  - Set `WINEPREFIX` to the chosen prefix directory
  - Set `STEAM_COMPAT_DATA_PATH` consistently from the prefix path, matching the current `apply_runtime_proton_environment` behavior
  - Reuse or infer `STEAM_COMPAT_CLIENT_INSTALL_PATH` when available, as current `proton_run` launches already do
  - Execute the selected installer with `proton run <installer.exe>`
- Recommended compatibility pattern:
  - Treat `umu-run` as a documented compatibility reference, not as the primary execution path for v1
  - If a future release adds optional `umu-run` integration, convert CrossHook's stored Proton executable path to the parent tool directory when populating `PROTONPATH`
- Recommended prefix strategy:
  - Default to a durable CrossHook-managed prefix under `~/.config/crosshook/prefixes/<profile-slug>`
  - Allow manual override before install starts
  - Preserve the chosen prefix for later `proton_run` launches and trainer staging
- Recommended post-install pattern:
  - Do not assume installer media is the final runtime target
  - After installer exit, scan the prefix for likely Windows executables and ask the user to confirm the final game executable before profile save

### Constraints and Gotchas

- `umu-run` and CrossHook do not represent Proton paths the same way:
  - `umu-run` documents `PROTONPATH` as a Proton tool directory
  - CrossHook currently stores and validates a direct executable path ending in `.../proton`
- The install flow is not the same as the existing game-launch flow:
  - installers can spawn secondary processes, patchers, or launchers and exit before the user is truly finished
  - progress is often not measurable from outside the process
- Proton/prefix environment consistency matters:
  - the `umu-launcher` FAQ explicitly warns that mixing incompatible configuration across launches in the same prefix can cause unexpected behavior
  - CrossHook should use the same selected Proton for install and the generated profile by default
- Steam-runtime exposure can matter for some installers:
  - `umu-launcher` documents extra filesystem exposure via `STEAM_COMPAT_LIBRARY_PATHS` or pressure-vessel variables
  - CrossHook should avoid building v1 around these advanced paths unless a real compatibility case requires them
- Packaging risk:
  - a hard requirement on `umu-run` would require distro/AppImage packaging decisions that the repo does not currently make

### Open Decisions

- Should v1 stay entirely on CrossHook's direct `proton run` path, or optionally prefer `umu-run` when it is installed?
- If `umu-run` support is added later, should CrossHook store both Proton tool directory and Proton executable path, or derive the directory from the executable path when needed?
- Does the product want to support installer arguments in v1, or keep scope to executable-only selection?
- Should CrossHook surface an advanced environment section for edge cases like `STEAM_COMPAT_LIBRARY_PATHS`, or defer that entirely?
- How aggressively should the app try to infer the final installed executable after install exit versus forcing explicit user confirmation?
