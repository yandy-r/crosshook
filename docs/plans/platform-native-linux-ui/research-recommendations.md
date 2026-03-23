# Recommendations: platform-native-linux-ui

## Executive Summary

The strongest path to a native Linux UI for CrossHook is **Tauri (Rust backend + web frontend)**, which leverages the existing working Bash launcher scripts as the immediate process orchestration layer while providing a cross-platform UI foundation that can eventually serve macOS and Windows too. The critical architectural insight is that CrossHook's Linux use case does NOT need the Win32 P/Invoke injection layer -- the launcher scripts already solve the trainer-launch problem natively -- so the native UI is fundamentally a **profile manager, process orchestrator, and Steam integration frontend** rather than a port of the WinForms injection engine. The primary risk is maintaining two codebases (WinForms for legacy WINE use + native UI for Linux), which can be mitigated by treating the WinForms app as frozen at its current feature set and investing all new feature work in the native app.

## Implementation Recommendations

### Recommended Approach

The native Linux UI should be built as a **thin orchestration frontend** that delegates actual game/trainer launching to the existing Bash scripts (`steam-launch-helper.sh`, `steam-host-trainer-runner.sh`, `steam-launch-trainer.sh`). The app's role is:

1. **Profile management** -- read/write `.profile` files (the format is already a simple `Key=Value` text format defined in `ProfileService.cs`)
2. **Steam auto-discovery** -- port the logic in `SteamAutoPopulateService.cs` (Steam library discovery, manifest parsing, Proton path resolution) to native code
3. **Process orchestration** -- assemble the correct CLI arguments and invoke the launcher scripts, then stream logs back to the UI
4. **External launcher export** -- generate `.desktop` entries and launcher scripts (already implemented in `SteamExternalLauncherExportService.cs`)

This approach avoids the need to reimplement any Win32 P/Invoke, DLL injection, or memory management -- those capabilities are only needed inside WINE and are already handled by the existing scripts invoking `proton run`.

### Technology Choices

| Component          | Recommendation                                                 | Rationale                                                                                                                                                                                                                                           |
| ------------------ | -------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| UI Framework       | Tauri v2 (Rust + web frontend)                                 | Native system webview eliminates Electron's overhead; Rust backend handles process spawning, file I/O, Steam path resolution with zero GC pauses; web frontend (TypeScript/Svelte or React) provides rapid UI iteration; single binary distribution |
| Frontend           | Svelte (via SvelteKit) or plain TypeScript                     | Minimal bundle size, excellent reactivity model, no heavy runtime; aligns with Steam Deck's limited resources                                                                                                                                       |
| Process Management | Rust `std::process::Command` wrapping existing Bash scripts    | Bash scripts are proven; Rust provides typed argument construction and log streaming                                                                                                                                                                |
| Profile Format     | JSON (migration from current `Key=Value` flat file)            | Enables nested structures for future community profile sharing; backward-compatible reader for legacy `.profile` files                                                                                                                              |
| Steam Integration  | Native Rust (VDF parser, library folder scanner)               | Ports the logic from `SteamAutoPopulateService.cs` without needing .NET; existing crates like `keyvalues-parser` handle Valve's VDF format                                                                                                          |
| Packaging          | Flatpak (primary) + AppImage (fallback) + native `.deb`/`.rpm` | Flatpak for Steam Deck/immutable distros; AppImage for universal compatibility; native packages for Arch/Fedora/Debian                                                                                                                              |

### Phasing Strategy

1. **Phase 1 -- MVP (4-6 weeks)**: Profile-driven launcher that invokes existing Bash scripts. Covers: load/save profiles, browse for game/trainer/Proton paths, launch game+trainer via `steam-launch-helper.sh`, stream helper log output to the UI. No Steam auto-discovery yet -- users enter paths manually (same as current WinForms workflow).

2. **Phase 2 -- Smart Discovery (3-4 weeks)**: Port `SteamAutoPopulateService` logic to Rust. Auto-detect Steam libraries, parse app manifests, resolve Proton versions and compatdata paths. Add the "Auto-Populate" button equivalent. Add external launcher export (`.desktop` files and standalone scripts, porting `SteamExternalLauncherExportService`).

3. **Phase 3 -- Polish and Platform (3-4 weeks)**: System tray integration for background monitoring. Notifications when game process is detected (trainer ready to launch). Steam Deck game-mode-friendly layout (controller-navigable, large touch targets). Settings persistence, recent files, auto-load last profile. Dark theme aligned with Steam Deck aesthetic.

4. **Phase 4 -- Community Features (4-6 weeks)**: Community profile format (JSON/YAML with compatibility metadata). Git-based profile sharing ("taps" model from the existing research report). Trainer compatibility database viewer. Auto-update mechanism.

### Quick Wins

- **Script-only CLI launcher (1-2 days)**: A simple Rust CLI binary that reads a `.profile` file and invokes `steam-launch-helper.sh` with the correct arguments. Ships immediately as a headless alternative, useful for Steam Deck game mode or scripting.
- **Desktop entry generator standalone (1 day)**: Extract the `.desktop` file generation logic into a standalone script or small binary. Users can create launcher shortcuts without the full UI.
- **Profile format documentation (1 day)**: Document the `.profile` file format so users can create profiles manually. The format is trivially simple (`Key=Value` pairs).

### WinForms App Disposition

**Recommendation: Freeze the WinForms app at its current feature set. Do not sunset it immediately.**

Rationale:

- The WinForms app provides capabilities the native UI will not replicate: direct DLL injection via `CreateRemoteThread`/`LoadLibraryA`, process memory read/write, process suspend/resume, mini-dump creation. These features work when CrossHook itself runs inside WINE alongside the game.
- Some users may prefer the WINE-hosted approach for specific trainer types that require in-process injection.
- The native UI addresses a different use case: launching trainers as separate Proton processes in the same compatdata, which is the approach that actually works reliably for cross-process trainer access.
- Once the native UI reaches feature parity on the profile/launch workflow (Phase 2), the WinForms app can be marked as "legacy" in documentation.
- Eventual sunset decision should be driven by user adoption metrics, not a predetermined timeline.

## Improvement Ideas

### Related Features

- **Community Profile Sharing**: The existing research report (`research/crosshook-feature-enhancements/report.md`) identifies this as the highest-leverage feature, recommended by 6 of 8 research personas. A native app with JSON profiles and Git-based "taps" is the natural implementation vehicle. The native UI should be designed from the start with a profile import/export flow.

- **Steam Library Scanner UI**: The `SteamAutoPopulateService` already discovers Steam libraries, parses manifests, and resolves Proton paths. A native UI can present this as a visual game browser -- show all installed Steam games, highlight which ones have compatdata (i.e., have been run through Proton), and let users create profiles by clicking on a game.

- **Trainer Auto-Discovery**: Scan common trainer download locations (`~/Downloads`, known FLiNG/WeMod directories) and match trainer filenames against game names. Present a "suggested trainers" list when creating a profile.

- **Proton Version Manager**: Show all installed Proton versions (official + GE-Proton), highlight which version each game's compatdata was created with, and allow switching Proton versions per-profile.

### Future Enhancements

- **Plugin/Extension System (Medium complexity)**: Define a trait/interface for "trainer launchers" so the community can add support for new trainer types (e.g., Cheat Engine tables, Lua-based trainers) without modifying core code. Each plugin would provide: discovery logic, launch command construction, and compatibility metadata.

- **Cloud Sync of Profiles (Low-Medium complexity)**: Since profiles are small text files, sync via any file-sync service (Syncthing, Nextcloud, even a Git repo). The native app just needs a configurable profiles directory path. This is simpler than building a cloud backend.

- **Steam Workshop Integration (High complexity, speculative)**: Steam Workshop is primarily for game content, not trainers. However, CrossHook could theoretically publish profile configurations as Workshop items for games that support it. The practical value is limited and the legal/ToS risk is real. Not recommended for near-term planning.

- **Nexus Mods Integration (Medium complexity)**: Nexus Mods has an API. CrossHook could search for trainers/mods for a given game and present download links. This requires API key management and respecting Nexus's rate limits and ToS. Valuable but not MVP.

- **Gamepad-Navigable UI for Steam Deck Game Mode (Medium complexity)**: Steam Deck's game mode presents non-Steam apps in a controller-friendly overlay. The native UI should support keyboard/gamepad navigation with large touch targets and minimal text input. Tauri's web frontend makes this achievable with CSS media queries and focus management.

## Risk Assessment

### Technical Risks

| Risk                                                                                     | Likelihood | Impact | Mitigation                                                                                                                                                                                                                                                                                                                       |
| ---------------------------------------------------------------------------------------- | ---------- | ------ | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Bash scripts break on Steam Deck OS updates (SteamOS 3.x kernel/userspace changes)       | Medium     | High   | Pin tested SteamOS versions in CI; the scripts use only POSIX utilities (`pgrep`, `setsid`, `readlink`) and Proton's own `proton run` interface, which Valve maintains                                                                                                                                                           |
| Flatpak sandboxing prevents access to Steam directories and game files                   | High       | High   | Request `--filesystem=home` and `--filesystem=/run/media` in Flatpak manifest; document that Flatpak may require `--talk-name=org.freedesktop.Flatpak` for host process spawning; provide AppImage as sandbox-free alternative                                                                                                   |
| Proton/WINE environment variable contract changes between versions                       | Medium     | Medium | The scripts already strip all WINE/Proton env vars (`WINESERVER`, `WINEDLLPATH`, etc.) and reconstruct a clean environment. This pattern is resilient to additions. Monitor Proton changelogs.                                                                                                                                   |
| Linux kernel restricts `ptrace` / process inspection further (Yama LSM scope tightening) | Low        | Medium | CrossHook's native UI does not use `ptrace` -- it launches trainers as independent Proton processes. The trainers themselves may use process manipulation, but that happens inside WINE's address space. The only host-side process operation is `pgrep` for detection, which reads `/proc` and is unaffected by `ptrace_scope`. |
| Tauri v2 has breaking changes or slow adoption                                           | Low        | Medium | Tauri v2 is stable (released 2024). The web frontend can be extracted to any other framework if needed. The Rust backend logic (process spawning, file I/O, VDF parsing) is framework-independent.                                                                                                                               |
| Two-codebase maintenance burden (WinForms + native)                                      | High       | Medium | Freeze WinForms at current feature set. Share profile format specification. Do not attempt to keep feature parity -- the native app is the future; the WinForms app is a legacy fallback.                                                                                                                                        |
| Profile format migration (from `.profile` Key=Value to JSON)                             | Low        | Low    | Write a one-time migration tool. Support reading both formats in the native app. The current format has only 12 fields.                                                                                                                                                                                                          |

### Integration Challenges

- **Steam client detection on immutable distros (SteamOS, Bazzite, etc.)**: Steam may be installed as a Flatpak, a native package, or in a custom location. The `SteamAutoPopulateService` logic already handles multiple candidate paths (`~/.steam/root`, `~/.local/share/Steam`). The native app must additionally check Flatpak's `~/.var/app/com.valvesoftware.Steam/data/Steam`.

- **Proton symlink resolution**: The `SteamLaunchService.ResolveDosDevicesPath` and `ResolveDosDeviceLinkTarget` methods handle complex WINE dosdevices symlink chains. The native Rust equivalent needs to resolve these same chains, but benefits from direct access to the Linux filesystem without the WINE abstraction layer.

- **Process visibility across WINE/Proton boundaries**: The `steam-launch-helper.sh` uses `pgrep -af` to detect game processes. This works because WINE processes appear as host Linux processes. However, process names may be truncated or suffixed differently across Proton versions. The scripts handle this by checking both `game.exe` and `game` (without extension).

- **Display server compatibility**: The launcher scripts pass through `DISPLAY`, `WAYLAND_DISPLAY`, `XDG_RUNTIME_DIR`, and `DBUS_SESSION_BUS_ADDRESS` to the detached runner process. The native UI must ensure these are available in its own environment, which may be restricted under Flatpak.

## Alternative Approaches

### Option A: Rust + GTK4 (Full Native)

- **Pros**: Maximum native Linux feel; GTK4 is the standard toolkit for GNOME/Steam Deck; excellent performance; no webview dependency; integrates with system themes and accessibility frameworks; `libadwaita` provides polished adaptive layouts.
- **Cons**: GTK4's Rust bindings (`gtk4-rs`) are mature but have a steeper learning curve than web technologies; GTK4 is GNOME-centric and looks foreign on KDE; harder to port to macOS/Windows later; designer tooling is limited compared to web; slower UI iteration cycle.
- **Effort**: 6-8 weeks to MVP. Higher upfront investment but lower long-term maintenance.
- **Best for**: If the project commits to Linux-first and GNOME ecosystem integration indefinitely.

### Option B: Tauri v2 (Rust + Web Frontend) -- RECOMMENDED

- **Pros**: Native system webview (no bundled Chromium, unlike Electron); Rust backend provides type-safe process management and file I/O; web frontend enables rapid UI iteration with familiar tools; single binary under 10MB; cross-platform by design (Linux, macOS, Windows); excellent Steam Deck compatibility (WebKitGTK is pre-installed on SteamOS); plugin system via Tauri's IPC.
- **Cons**: Depends on system webview (WebKitGTK on Linux); web UI may feel slightly less native than GTK4; debugging requires both Rust and web toolchains; Tauri v2 is newer than Electron (smaller ecosystem, fewer tutorials).
- **Effort**: 4-6 weeks to MVP. Fastest path to a cross-platform native app.
- **Best for**: This project's specific constraints: cross-platform ambition, small team, need for rapid iteration, Steam Deck as primary target.

### Option C: Python + GTK4

- **Pros**: Fastest development cycle; excellent GTK4 bindings via PyGObject; large ecosystem for subprocess management and file parsing; lowest barrier to contribution; GNOME Builder provides a decent development experience.
- **Cons**: Python distribution is painful (bundling a Python runtime, managing dependencies); performance overhead for a long-running app; type safety is opt-in (mypy); harder to distribute as a single binary; Flatpak packaging with Python is well-understood but adds complexity.
- **Effort**: 3-5 weeks to MVP. Fastest to prototype but slowest to polish.
- **Best for**: If the team prioritizes development speed over distribution simplicity and long-term maintainability.

### Option D: C# Avalonia (Code Reuse)

- **Pros**: Maximum code reuse from existing C# codebase; `ProfileService`, `SteamAutoPopulateService`, `SteamLaunchService`, `SteamExternalLauncherExportService`, `AppSettingsService`, and `CommandLineParser` could be shared directly; Avalonia is cross-platform and mature; familiar language for existing contributors.
- **Cons**: Requires .NET runtime on Linux (either self-contained publish at ~80MB or framework-dependent requiring user to install .NET); Avalonia's Linux rendering uses Skia, not native GTK, so it looks non-native; the Win32 P/Invoke code in `ProcessManager`, `InjectionManager`, and `MemoryManager` cannot be shared (it is Windows-only by design); .NET on Linux adds a layer of complexity for Steam Deck deployment; Flatpak packaging with .NET is unusual.
- **Effort**: 5-7 weeks to MVP. Moderate reuse offset by Avalonia learning curve and Linux deployment challenges.
- **Best for**: If preserving C# investment is the top priority and the team accepts non-native UI appearance.

### Option E: Hybrid (Native Daemon + Web UI)

- **Pros**: Clean separation of concerns; Rust daemon handles all process management and exposes a local REST/WebSocket API; any frontend (web browser, Electron, Tauri, even a terminal TUI) can connect; enables headless/remote use cases (SSH to Steam Deck, manage from phone).
- **Cons**: Over-engineered for the current scope; adds network layer complexity; security considerations for a local API serving game modification commands; two deployment units instead of one.
- **Effort**: 6-10 weeks to MVP. Highest architectural investment.
- **Best for**: If the long-term vision includes remote management, headless operation, or multiple simultaneous frontends. Premature for current needs.

### Recommendation

**Option B (Tauri v2)** is the strongest fit for CrossHook's specific situation:

1. The Rust backend can directly spawn the existing Bash scripts via `std::process::Command`, providing a typed wrapper around the proven launch workflow.
2. The web frontend enables rapid UI iteration -- critical when the team is still discovering the right UX for Steam Deck.
3. WebKitGTK is pre-installed on SteamOS, so no additional runtime dependencies on the primary target platform.
4. The single-binary output (~5-10MB) is trivially distributable as an AppImage or Flatpak.
5. The same codebase can serve macOS and Windows in the future, unlike GTK4.
6. Tauri's IPC system provides a natural boundary for a future plugin system.

The decision is close between Tauri and GTK4. GTK4 wins on native feel; Tauri wins on cross-platform reach, development speed, and distribution simplicity. Given that CrossHook's roadmap explicitly includes macOS support and the team is small, Tauri is the pragmatic choice.

## Task Breakdown Preview

### Phase 1: Foundation and MVP (4-6 weeks)

**Task Group 1.1: Project Scaffolding (Week 1)**

- Initialize Tauri v2 project with Rust backend and Svelte frontend
- Set up CI (GitHub Actions: `cargo build`, `cargo test`, `npm run build`)
- Define Rust data models for `ProfileData` (matching the 12 fields in the existing `.profile` format)
- Implement profile file reader (support both legacy `Key=Value` and new JSON format)
- Implement profile file writer (JSON)

**Task Group 1.2: Core Launch Workflow (Week 2-3)**

- Build Rust wrapper around `steam-launch-helper.sh` invocation (argument construction, environment setup)
- Build Rust wrapper around `steam-launch-trainer.sh` invocation
- Implement log streaming (tail the helper log file and push lines to the frontend via Tauri events)
- Build frontend: profile editor form (game path, trainer path, Steam App ID, compatdata path, Proton path)
- Build frontend: launch button with game-first-then-trainer two-step flow (matching current WinForms behavior)

**Task Group 1.3: File Browsing and Validation (Week 3-4)**

- Implement native file/directory picker dialogs via Tauri's dialog API
- Path validation (check existence of game exe, trainer exe, Proton binary, compatdata directory)
- Console output panel (display launch logs, validation messages, helper script output)

**Parallel opportunities**: Task Groups 1.1 and the data model work in 1.2 can proceed in parallel. Frontend form building (1.2) and file browsing (1.3) are independent.

### Phase 2: Smart Discovery (3-4 weeks)

**Task Group 2.1: Steam Library Discovery (Week 5-6)**

- Port `DiscoverSteamRootCandidates` logic to Rust
- Implement VDF parser for `libraryfolders.vdf`
- Port `DiscoverSteamLibraries` and manifest parsing
- Port `FindGameMatch` (match game executable against Steam app manifests)
- Port Proton version resolution (`ResolveProtonPath`)
- Build frontend: "Auto-Populate" button and result display

**Task Group 2.2: External Launcher Export (Week 6-7)**

- Port `BuildTrainerScriptContent` and `BuildDesktopEntryContent` to Rust
- Implement `.desktop` file generation
- Build frontend: "Export Launchers" button with success feedback

**Parallel opportunities**: 2.1 and 2.2 are largely independent; the export service only needs the profile data, not the auto-populate result.

### Phase 3: Polish and Steam Deck (3-4 weeks)

**Task Group 3.1: Settings and Persistence (Week 8)**

- App settings (auto-load last profile, recent files)
- Profile list management (list, rename, delete)
- Recent file paths in dropdowns

**Task Group 3.2: Steam Deck Optimization (Week 9-10)**

- Large touch-target layout variant
- Controller/gamepad navigation support
- System tray integration (minimize to tray, notification on game detection)
- Responsive layout for various screen sizes

**Task Group 3.3: Packaging and Distribution (Week 10-11)**

- Flatpak manifest with correct filesystem permissions
- AppImage build pipeline
- AUR PKGBUILD (for Arch-based systems including SteamOS)
- GitHub Release workflow update to produce Linux artifacts alongside Windows

### Phase 4: Community Features (4-6 weeks)

**Task Group 4.1: Community Profile Format**

- Define JSON schema for community profiles (game metadata, trainer metadata, compatibility ratings, launch configuration)
- Profile import/export UI
- Profile validation

**Task Group 4.2: Git-Based Profile Sharing**

- "Tap" system: add/remove Git repository URLs as profile sources
- Fetch and cache remote profile indexes
- Search and browse community profiles in the UI
- One-click import from community profile to local profile

### Estimated Complexity

- **Total tasks**: ~45-55 discrete tasks across all phases
- **Critical path**: Phase 1 scaffolding -> core launch workflow -> MVP release -> Phase 2 auto-discovery -> Phase 3 packaging
- **Minimum viable release**: End of Phase 1 (profile editor + script-based launcher)
- **Competitive release**: End of Phase 2 (with auto-discovery matching WinForms feature parity on the Steam workflow)

## Relevant Files

The following existing files contain logic or patterns that directly inform the native UI implementation:

- `src/CrossHookEngine.App/Services/ProfileService.cs`: Profile data model and file format (12 fields, `Key=Value` text). The native app must read this format for backward compatibility.
- `src/CrossHookEngine.App/Services/SteamLaunchService.cs`: Steam launch request/validation/execution workflow, path conversion (`ConvertToUnixPath`, `NormalizeSteamHostPath`), environment variable management, and helper script invocation. This is the most critical file to port.
- `src/CrossHookEngine.App/Services/SteamAutoPopulateService.cs`: Steam library discovery, VDF manifest parsing, game-to-AppID matching, Proton path resolution. ~500 lines of logic that must be reimplemented natively.
- `src/CrossHookEngine.App/Services/SteamExternalLauncherExportService.cs`: `.desktop` entry and launcher script generation. Straightforward to port.
- `src/CrossHookEngine.App/Services/AppSettingsService.cs`: Simple settings persistence. Trivial to port.
- `src/CrossHookEngine.App/Services/CommandLineParser.cs`: CLI argument parsing for `-p` (profile) and `-autolaunch`. Informs the native app's CLI interface.
- `src/CrossHookEngine.App/runtime-helpers/steam-launch-helper.sh`: The primary launcher script. The native UI invokes this directly. Must be bundled with the native app.
- `src/CrossHookEngine.App/runtime-helpers/steam-launch-trainer.sh`: Trainer-only launcher (detached mode). Invoked for the "Launch Trainer" step.
- `src/CrossHookEngine.App/runtime-helpers/steam-host-trainer-runner.sh`: The actual Proton runner, invoked by `steam-launch-trainer.sh` in a clean environment.
- `src/CrossHookEngine.App/Forms/MainForm.cs` (lines 2648-2946): The `BuildSteamLaunchRequest`, `LaunchSteamModeAsync`, `RunSteamLaunchHelper`, and `StreamSteamHelperLogAsync` methods define the full launch orchestration flow that the native UI must replicate.

## Key Decisions Needed

- **Profile format migration**: Should the native app use JSON from day one and include a one-time migration tool for legacy `.profile` files, or should it read/write the legacy format indefinitely? JSON is recommended for extensibility (community profiles need nested structures), with a backward-compatible reader for the legacy format.

- **Bash script bundling vs. inlining**: Should the native app continue to invoke the external Bash scripts, or should the script logic be absorbed into Rust? Recommendation: keep the scripts as external files for Phase 1-2 (proven, debuggable, independently testable), then evaluate inlining in Phase 3 if distribution complexity justifies it.

- **Flatpak vs. AppImage as primary distribution**: Flatpak is preferred for Steam Deck but introduces sandboxing friction. AppImage is simpler but lacks auto-update and desktop integration. Recommendation: support both, with Flatpak as the recommended install method and AppImage as the zero-install alternative.

- **WinForms deprecation timeline**: When (if ever) should the WinForms app be officially deprecated? Recommendation: defer this decision until the native UI reaches Phase 2 completion and adoption metrics are available.

- **Monorepo vs. separate repository**: Should the native app live in this repository or a new one? Recommendation: monorepo (e.g., `src/crosshook-native/`) to share the launcher scripts, profile format specification, and CI infrastructure. The WinForms app stays in `src/CrossHookEngine.App/`.

## Open Questions

- What is the current user distribution across Steam Deck, desktop Linux, and macOS? This affects Phase 3 prioritization (Steam Deck game mode optimizations vs. standard desktop polish).

- Are there specific trainer types (FLiNG, WeMod standalone, Cheat Engine tables) that the community uses most frequently? This affects which trainer launch patterns the MVP must support.

- Should the native app support the "direct launch" mode (launching a game exe directly without Steam, using `CreateProcess` or `ShellExecute`)? This mode is primarily useful on Windows. On Linux, games are almost always launched through Steam/Proton. Recommendation: omit direct launch from the native app MVP.

- Is there interest in a terminal UI (TUI) mode for headless/SSH use cases? If so, the Rust backend should be designed with a clean separation between UI and business logic from day one, which Tauri's architecture naturally encourages.

- What is the acceptable binary size for the native app? Tauri produces ~5-10MB binaries (using system webview). GTK4 binaries are ~2-5MB but require GTK4 libraries at runtime. Avalonia self-contained is ~80MB+. This may influence the framework decision for Steam Deck users with limited storage.
