# UX Research: dotnet-migrate

> Exploratory UX/background document. The active migration plan keeps WinForms and treats Avalonia as follow-up work.

## Executive Summary

Migrating CrossHook Loader from .NET Framework 4.8 to modern .NET (8/9) presents a critical UX crossroads: WinForms on modern .NET cannot reliably run under WINE/Proton without installing the .NET Desktop Runtime into the WINE prefix (a notoriously fragile process), while .NET Framework 4.8 WinForms is reasonably supported by wine-mono. The strongest path forward for cross-platform Linux/Steam Deck users is migrating to **Avalonia UI**, which renders natively on Linux without WINE, supports MVVM architecture, and has proven itself in production game-modding tools (NexusMods.App uses Avalonia). A phased migration -- first porting to .NET 8/9 while keeping WinForms functional under WINE, then incrementally rewriting the UI in Avalonia -- balances risk and delivers the best long-term UX.

## User Workflows

### Current Flow (WinForms on WINE)

The current CrossHookk Loaderr workflow runs as a Windows executable under Proton/WINE:

1. **Launch**: User adds CrossHookk Loaderr as a non-Steam game or launches via Proton prefix. WINE's wine-mono provides .NET Framework 4.8 compatibility. The WinForms UI renders through WINE's GDI translation layer.
2. **Configure paths**: User browses for game executable, trainer executable, and up to 2 DLLs using Windows file dialogs (rendered by WINE). MRU dropdown lists provide recent selections.
3. **Profile management**: User saves/loads `.ini` profile configurations. Auto-load last profile is available.
4. **Select launch method**: User picks from 6 radio button options (CreateProcess, CmdStart, CreateThreadInjection, RemoteThreadInjection, ShellExecute, ProcessStart).
5. **Launch and inject**: Single "Launch" button starts the game and trainer, performs DLL injection. Status updates appear in real-time in a console-style TextBox.
6. **Monitor**: The loaded DLLs ListBox shows injection results. The ResumePanel overlay provides pause/resume indication.

**Pain points under WINE:**

- File dialogs may render with incorrect themes or sizing
- Font rendering can be blurry due to ClearType emulation under WINE
- DPI scaling is unreliable -- the app uses Segoe UI fonts and pixel-based layouts that may not scale correctly
- Dark theme (custom-painted at RGB 30,30,30) may have rendering artifacts under WINE
- Owner-drawn tab controls may flicker or render incorrectly
- The compact/responsive layout system (switching at 950px width) may behave unexpectedly

**Confidence**: High -- based on direct codebase analysis and documented WINE rendering limitations.

### Post-Migration Flow

If migrating to .NET 8/9 while keeping WinForms:

1. **Runtime dependency changes**: The .NET Desktop Runtime 8 must be installed in the WINE prefix. This is [documented as highly problematic](https://github.com/Winetricks/winetricks/issues/2276) -- winetricks `dotnetdesktop8` frequently fails, and manual installation is unreliable.
2. **Font change impact**: .NET 8 WinForms changed the default font from Microsoft Sans Serif 8.25pt to Segoe UI 9pt, causing forms and controls to be [approximately 27% larger](https://github.com/dotnet/winforms/issues/524). Since CrossHookk Loaderr already uses Segoe UI explicitly, the impact is reduced but the AutoScaleMode behavior changes in .NET 8 could still shift layouts.
3. **DPI scaling changes**: .NET 8 introduces [breaking changes in form scaling](https://learn.microsoft.com/en-us/dotnet/core/compatibility/windows-forms/8.0/top-level-window-scaling) for PerMonitorV2 mode. Under WINE, DPI behavior is emulated and may not match native Windows behavior.
4. **Self-contained deployment**: Publishing as self-contained (`--self-contained`) bundles the runtime, potentially avoiding the WINE runtime installation problem. However, this is [not well-tested under WINE](https://forum.winehq.org/viewtopic.php?t=39911) for .NET 8 WinForms specifically.

**Confidence**: Medium -- based on multiple user reports of .NET 8 installation failures in WINE/Proton prefixes, but limited data on self-contained deployments specifically.

If migrating to Avalonia UI:

1. **Native Linux rendering**: No WINE layer needed for the UI. Avalonia renders via Skia (or the upcoming Impeller backend) directly on X11/Wayland.
2. **Preserved core logic**: ProcessManager, InjectionManager, and MemoryManager still use Win32 P/Invoke (kernel32.dll), so the injection operations themselves still require WINE. However, the UI runs natively.
3. **Hybrid architecture**: The UI application runs as a native Linux process, orchestrating game/trainer launches through WINE/Proton. This is how Lutris, Bottles, and Heroic Games Launcher work.
4. **Profile compatibility**: .ini profile format can be preserved for backwards compatibility.

**Confidence**: High -- NexusMods.App validates this architecture in production.

### Steam Deck Gaming Mode

Steam Deck Gaming Mode presents unique UX constraints:

1. **Controller-as-mouse**: In Gaming Mode, the right trackpad emulates mouse input. Users can click buttons and interact with standard UI elements, but small targets are frustrating. The Steam button + right trackpad provides cursor control, with L2/R2 triggers for left/right click.
2. **No keyboard by default**: Text input requires the on-screen keyboard (invoked via Steam + X). The current workflow has multiple text fields (paths, profile names) that are cumbersome with on-screen keyboard.
3. **Limited screen space**: The Steam Deck's 1280x800 display means the app needs to work well at that resolution. The current compact mode threshold (950px) is reasonable, but the layout in compact mode stacks panels vertically, requiring scrolling.
4. **Add as non-Steam game**: To use in Gaming Mode, CrossHookk Loaderr must be added as a non-Steam game. Controller layout must be configured to "Gamepad with Mouse Trackpad" or similar.
5. **Overlay limitations**: The custom ResumePanel overlay works within the WinForms window. Under WINE, this should render correctly, but the "CLICK TO RESUME" text (designed for mouse) is less intuitive with a trackpad.

**UX recommendations for Steam Deck:**

- Increase touch/click target sizes to minimum 48x48px
- Reduce text input requirements (use file pickers, profile dropdowns instead of text entry)
- Support a "Steam Deck mode" with larger fonts and simplified layout
- Consider mapping controller buttons to common actions (e.g., Start = Launch, Select = Load Profile)

**Confidence**: Medium -- based on Steam Deck documentation and community reports; no direct testing data for CrossHookk Loaderr on Steam Deck.

## UI Framework Options

### Option 1: WinForms on .NET 8/9 (via WINE)

- **Pros**:
  - Minimal code changes from .NET Framework 4.8
  - Microsoft's [official migration guide](https://learn.microsoft.com/en-us/dotnet/desktop/winforms/migration/) and .NET Upgrade Assistant available
  - Preserves existing UI layout and user familiarity
  - Access to .NET 8/9 runtime improvements (better GC, async patterns)
  - AllowUnsafeBlocks already enabled in csproj, easing P/Invoke migration
- **Cons**:
  - WinForms remains Windows-only; requires WINE on Linux
  - [.NET 8 Desktop Runtime installation in WINE is unreliable](https://github.com/Winetricks/winetricks/issues/2178)
  - wine-mono does NOT support modern .NET (it targets .NET Framework only) -- [wine-mono explicitly states it is a replacement for .NET Framework 4.8.1 and earlier](https://github.com/madewokherd/wine-mono)
  - Default font change (Microsoft Sans Serif -> Segoe UI 9pt) causes [~27% layout inflation](https://github.com/dotnet/winforms/issues/2111)
  - DPI scaling breaking changes in .NET 8 compound WINE rendering issues
  - No path to native Linux rendering
  - Single-file publish for WinForms [has reported issues](https://github.com/dotnet/winforms/issues/11473)
- **WINE compatibility**: Poor for .NET 8/9. wine-mono cannot run modern .NET WinForms. Users must install .NET Desktop Runtime 8 into the WINE prefix, which is a known pain point. Self-contained deployment may work but is untested/unreliable.

**Confidence**: High -- wine-mono maintainers explicitly state no modern .NET support; multiple GitHub issues document .NET 8 WINE installation failures.

### Option 2: Avalonia UI (Cross-platform native)

- **Pros**:
  - True cross-platform: renders natively on Linux (X11/Wayland), Windows, macOS
  - No WINE dependency for the UI layer
  - XAML-based with WPF-inspired API (familiar to .NET developers)
  - [Excellent Linux support](https://docs.avaloniaui.net/docs/overview/supported-platforms): Debian 9+, Ubuntu 16.04+, Fedora 30+
  - Production-validated for game modding: [NexusMods.App uses Avalonia](https://www.gamingonlinux.com/2023/11/nexus-mods-app-is-an-in-development-replacement-for-vortex-that-will-support-linux/)
  - Skia-based rendering engine delivers consistent visuals across platforms
  - Strong MVVM support with [Community MVVM Toolkit or ReactiveUI](https://docs.avaloniaui.net/docs/concepts/the-mvvm-pattern/avalonia-ui-and-mvvm)
  - Flatpak and AppImage deployment for Steam Deck/SteamOS
  - Dark theme built into framework (FluentTheme with Dark mode)
  - Active development: [MAUI backend coming via Avalonia](https://www.theregister.com/2025/11/13/dotnet_maui_linux_avalonia/)
  - [Gamepad support PR in progress](https://github.com/AvaloniaUI/Avalonia/pull/18445) (XY focus navigation)
- **Cons**:
  - WinForms-to-Avalonia migration is effectively a [near-100% UI rewrite](https://github.com/AvaloniaUI/Avalonia/discussions/11104) (code-behind to MVVM)
  - No incremental embedding path (unlike WPF-in-WinForms)
  - Learning curve for XAML/MVVM if team is WinForms-only
  - Gamepad/controller input support is [not yet merged](https://github.com/AvaloniaUI/Avalonia/issues/6945) (draft PR exists)
  - Estimated effort: ~4 minutes per line of XAML + code-behind for porting, or [~9 hours per view](https://avaloniaui.net/blog/the-expert-guide-to-porting-wpf-applications-to-avalonia) (from WPF; WinForms will be higher)
  - The P/Invoke-heavy injection code (kernel32.dll calls) still needs WINE for the game-side operations
- **Linux native**: Full native support. Renders via Skia on X11 or Wayland. No WINE needed for the UI.

**Confidence**: High -- extensive documentation, production examples, active development community.

### Option 3: Terminal UI (Spectre.Console / Terminal.Gui)

- **Pros**:
  - Zero graphical dependencies -- runs in any terminal
  - [Terminal.Gui](https://gui-cs.github.io/Terminal.Gui/) provides full TUI framework with windows, buttons, dialogs, menus
  - [Spectre.Console](https://spectreconsole.net/) provides rich formatting for CLI output (tables, progress bars, status)
  - Extremely lightweight -- minimal memory and CPU footprint
  - Works perfectly over SSH, in tmux/screen, and in Steam Deck's Konsole
  - Cross-platform natively (Linux, macOS, Windows)
  - Terminal.Gui v2 supports TrueColor and relative positioning (`Pos.Center()`, `Dim.Fill()`)
  - Aligns with CLI-first workflow (existing `-p`, `-autolaunch`, `-dllinject` flags)
- **Cons**:
  - No gamepad/controller support in terminal frameworks
  - Poor discoverability for casual gamers -- unfamiliar paradigm
  - Limited visual feedback for complex operations (no custom-drawn overlays like ResumePanel)
  - File browsing is functional but clunky compared to native file dialogs
  - Cannot display images, icons, or rich visual elements
  - Steam Deck Gaming Mode requires launching through a terminal emulator (adds friction)
  - Spectre.Console is primarily for [static output, not interactive TUIs](https://www.libhunt.com/compare-spectre.console-vs-Terminal.Gui)
- **Steam Deck**: Usable via Konsole terminal in Desktop Mode. In Gaming Mode, would need to be launched as a non-Steam app pointing to a terminal emulator script. Controller-as-mouse works for clicking TUI elements, but the experience is suboptimal.

**Confidence**: Medium -- Terminal.Gui is well-documented and proven, but the gaming audience fit is questionable.

### Option 4: .NET MAUI

- **Pros**:
  - Microsoft's official cross-platform UI framework
  - Familiar XAML syntax for .NET developers
  - Single codebase for Windows, Android, iOS, macOS
- **Cons**:
  - [No official Linux support from Microsoft](https://learn.microsoft.com/en-us/answers/questions/1637588/will-net-maui-include-official-support-for-linux)
  - Community Linux backends are experimental: [Avalonia backend preview expected Q1 2026](https://avaloniaui.net/blog/net-maui-is-coming-to-linux-and-the-browser-powered-by-avalonia), [GTK4 backend is experimental](https://www.phoronix.com/news/Microsoft-dotNET-MAUI-GTK4)
  - If using Avalonia as MAUI's Linux backend, you might as well use Avalonia directly
  - MAUI has had [stability and performance criticisms](https://dev.to/biozal/the-net-cross-platform-showdown-maui-vs-uno-vs-avalonia-and-why-avalonia-won-ian)
  - No Steam Deck / gaming-specific tooling
  - Desktop-class features lag behind mobile focus
- **Linux support**: Not officially supported. Community solutions (Avalonia backend, GTK4 backend, OpenMaui) are all experimental or preview-stage as of early 2026.

**Confidence**: High -- Microsoft's official documentation confirms no Linux desktop support; community efforts are pre-production.

### Recommendation

**Avalonia UI is the recommended framework** for the following reasons:

1. **Eliminates the WINE dependency for UI rendering** -- the single largest source of UX friction for Linux/Steam Deck users. wine-mono cannot support modern .NET WinForms, and installing .NET 8 Desktop Runtime in WINE prefixes is notoriously unreliable.

2. **Production-validated for game modding** -- NexusMods.App (the most prominent .NET game modding tool on Linux) chose Avalonia and ships on Linux including Steam Deck.

3. **Hybrid architecture is proven** -- The UI runs natively on Linux while game launching and DLL injection still operate through WINE/Proton. This is the same pattern used by Lutris (Python/GTK), Heroic Games Launcher (Electron), and Bottles (Python/GTK4).

4. **Dark theme is built-in** -- Avalonia's FluentTheme with Dark mode provides a polished dark UI out of the box, replacing the manual `Color.FromArgb(30, 30, 30)` theming in the current WinForms code.

5. **Deployment story is strong** -- Flatpak and AppImage deployment work on SteamOS/Steam Deck without requiring WINE.

6. **Future-proof** -- Gamepad support PR is in progress, MAUI is adopting Avalonia as a backend, and the framework is under active development with strong community momentum.

**Migration strategy**: Phase the migration rather than big-bang rewrite:

- Phase 1: Port business logic (ProcessManager, InjectionManager, MemoryManager) to .NET 8/9 class libraries
- Phase 2: Build Avalonia UI shell with core workflow (path selection, launch, status)
- Phase 3: Migrate profiles, MRU lists, and command-line support
- Phase 4: Add Steam Deck optimizations (larger targets, simplified layout)

## Error Handling

### Error States

| Error                                      | Current Behavior                                         | Recommended User Message                                                                                                     | Recovery Action                                                            |
| ------------------------------------------ | -------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------------------------------- |
| DLL architecture mismatch (32/64-bit)      | Silent validation failure, `InjectionFailed` event       | "Cannot inject [dll]: it is a [32/64]-bit DLL but the target game is [64/32]-bit. Select a matching DLL."                    | Highlight the DLL field, offer to browse for correct architecture          |
| DLL file not found                         | Returns `false` silently                                 | "DLL not found at [path]. The file may have been moved or deleted."                                                          | Clear the path field, offer to browse                                      |
| Process handle invalid                     | `InjectionFailed` event with "Process handle is invalid" | "Cannot connect to [game.exe]. The game may not be running or may have insufficient permissions."                            | Offer to refresh process list, suggest running as administrator            |
| LoadLibraryA returns 0                     | `InjectionFailed` event with "LoadLibraryA returned 0"   | "DLL injection failed for [dll]. The game may have anti-cheat protection or the DLL may be incompatible."                    | Log detailed Win32 error code, suggest trying alternative injection method |
| Game executable not found                  | Silent `false` return from `LaunchProcess`               | "Game executable not found at [path]. Please verify the file exists."                                                        | Highlight game path field, offer to browse                                 |
| Single instance conflict                   | MessageBox "already running"                             | "CrossHookk Loaderr is already running. Switch to the existing window or close it first."                                    | Bring existing instance to foreground if possible                          |
| Profile load failure                       | Likely unhandled exception                               | "Could not load profile [name]. The profile file may be corrupted."                                                          | Offer to delete corrupted profile, fall back to defaults                   |
| WINE/Proton not available                  | Not currently handled                                    | "WINE/Proton is required for game launching on Linux. Please ensure WINE is installed and configured."                       | Link to setup documentation, detect WINE availability at startup           |
| .NET runtime missing (WINE)                | System error dialog                                      | "The .NET Desktop Runtime could not be found. If running under WINE, install the runtime using: `winetricks dotnetdesktop8`" | Provide copy-pasteable command, link to troubleshooting guide              |
| Memory allocation failure (VirtualAllocEx) | Returns `false` without detail                           | "Could not allocate memory in the target process. The game may have restricted memory access."                               | Suggest restarting the game and trying again                               |

### Error UX Principles for Migration

1. **Fail visibly, not silently**: The current codebase has many paths that return `false` without user-visible feedback. Every failure should produce a status bar message at minimum.
2. **Distinguish recoverable from fatal**: DLL path errors are user-correctable; kernel32 API failures are not. Adjust messaging tone accordingly.
3. **Platform-aware messaging**: Detect WINE environment (`WINEPREFIX` env var or `wine_get_version` presence) and tailor error messages for Linux users.
4. **Log everything**: The current `Debug.WriteLine` calls are invisible to users. Add structured logging that writes to both the console panel and a log file.

**Confidence**: High -- based on direct analysis of error handling patterns in the codebase.

## Performance UX

### Startup Time

| Metric           | .NET Framework 4.8 (via wine-mono) | .NET 8/9 (via WINE)                                                                                        | .NET 8/9 (Avalonia native)                                                                                                  |
| ---------------- | ---------------------------------- | ---------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------- |
| Cold start       | 1-3 seconds typical                | 2-5 seconds (runtime initialization under WINE)                                                            | <1 second (native startup)                                                                                                  |
| Warm start       | <1 second                          | 1-2 seconds                                                                                                | <0.5 seconds                                                                                                                |
| ReadyToRun (R2R) | N/A                                | [Reduces JIT time significantly](https://devblogs.microsoft.com/dotnet/performance-improvements-in-net-8/) | Applicable, further reduces startup                                                                                         |
| Native AOT       | N/A                                | Not supported for WinForms                                                                                 | [Possible for Avalonia](https://learn.microsoft.com/en-us/dotnet/core/deploying/native-aot/) -- sub-50ms startup, <20MB RAM |

.NET 9 provides approximately [15% faster startup times](https://abp.io/community/articles/.net-9-performance-improvements-summary-gmww3gl8) compared to .NET 8. Native AOT compilation can [reduce startup by up to 75%](https://devblogs.microsoft.com/dotnet/performance-improvements-in-net-8/) and reduce memory by 30-40%.

**Confidence**: Medium -- startup benchmarks are from ASP.NET context; desktop app numbers may differ. WINE overhead is estimated from community reports.

### Runtime Performance

| Aspect         | .NET Framework 4.8           | .NET 8/9                                                                                                         | Impact on UX                                           |
| -------------- | ---------------------------- | ---------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------ |
| GC pauses      | Gen 2 pauses noticeable      | [Improved GC with reduced pause times](https://devblogs.microsoft.com/dotnet/performance-improvements-in-net-9/) | Smoother status updates during injection monitoring    |
| Async/await    | Limited Task support         | Full async pipeline                                                                                              | UI thread stays responsive during DLL injection        |
| Memory         | Higher baseline (~40-60MB)   | Lower baseline (~20-40MB), further with AOT                                                                      | Less impact on game performance when running alongside |
| Timer accuracy | System.Timers.Timer adequate | System.Timers.Timer + modern Task.Delay                                                                          | More precise monitoring intervals                      |

The current code uses a `System.Timers.Timer` with 1000ms interval for injection monitoring. .NET 8/9's improved timer resolution and async patterns would allow more responsive status updates without polling overhead.

**Confidence**: Medium -- general .NET performance data is well-documented; specific impact on this application's workload is estimated.

### Self-Contained vs Framework-Dependent

| Deployment Model             | Size     | Startup                                                                                                                            | User Experience                                                                                                                                                   |
| ---------------------------- | -------- | ---------------------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Framework-dependent (.NET 8) | ~5-10MB  | Fast (if runtime present)                                                                                                          | Requires .NET Desktop Runtime 8 installation -- **critical barrier under WINE**                                                                                   |
| Self-contained (.NET 8)      | ~60-80MB | Moderate                                                                                                                           | No runtime installation needed -- **may bypass WINE runtime issues**                                                                                              |
| Self-contained + trimmed     | ~30-50MB | Moderate                                                                                                                           | Smaller download, but [trimming can break reflection-heavy code](https://learn.microsoft.com/en-us/dotnet/core/deploying/trimming/prepare-libraries-for-trimming) |
| Native AOT (Avalonia)        | ~15-30MB | Very fast (<50ms)                                                                                                                  | Smallest runtime footprint, fastest startup, but limited reflection support                                                                                       |
| Single file                  | Varies   | [Slower first run](https://www.hanselman.com/blog/making-a-tiny-net-core-30-entirely-selfcontained-single-executable) (extraction) | Simpler distribution (one file), but WinForms single-file publish has [known issues](https://github.com/dotnet/winforms/issues/11473)                             |

**Recommendation**: If staying with WinForms under WINE, publish as self-contained to avoid requiring users to install .NET 8 Desktop Runtime in their WINE prefix. If migrating to Avalonia, a self-contained Linux binary distributed as AppImage or Flatpak provides the best user experience.

**Confidence**: High -- deployment model characteristics are well-documented by Microsoft.

## Competitive Analysis

### Lutris

- **Approach**: Native Linux game launcher that manages WINE prefixes, runtime installations, and game configurations. Launches games through WINE/Proton with pre-configured install scripts.
- **UI framework**: [Python 3 + GTK](https://github.com/lutris/lutris) (native Linux rendering, no WINE for the UI)
- **Architecture pattern**: The launcher UI is a native Linux application. Games run through WINE/Proton. Configuration, path management, and prefix setup are handled by the native UI layer.
- **Controller support**: Community project [lutris-gamepad-ui](https://github.com/andrew-ld/lutris-gamepad-ui) provides a controller-navigable frontend. Exploring Godot-based full-screen UI for controller/TV use.
- **Relevance to CrossHookk**: Validates the "native launcher + WINE game execution" hybrid architecture. Lutris's install scripts pattern is analogous toCrossHookok's profile system.

**Confidence**: High -- Lutris is a mature, widely-used project with public source code.

### Heroic Games Launcher

- **Approach**: Cross-platform game launcher for Epic, GOG, and Amazon games on Linux, Windows, and macOS.
- **UI framework**: [Electron (TypeScript/React)](https://github.com/Heroic-Games-Launcher/HeroicGamesLauncher) -- web-based rendering
- **Distribution**: AppImage, Flatpak, deb, RPM -- all standard Linux packaging formats
- **Relevance**: Demonstrates that [Electron can work well for game launchers](https://gardinerbryant.com/an-interview-with-the-dev-team-behind-heroic-games-launcher/) despite community skepticism. However, Electron's memory overhead (100-200MB baseline) is a disadvantage compared to native frameworks when running alongside games.

**Confidence**: High -- public source code and active community.

### Bottles

- **Approach**: Graphical WINE prefix manager for running Windows apps on Linux.
- **UI framework**: [Python + GTK4 + Libadwaita](https://github.com/bottlesdevs/Bottles) (native Linux with GNOME design language)
- **Key feature**: Creates isolated WINE prefixes ("bottles") with per-app dependency management
- **Native Wayland support**: [Added in Bottles 60.0](https://linuxiac.com/bottles-60-0-launches-with-native-wayland-support/)
- **Relevance**: Demonstrates GTK4/Libadwaita as a native Linux UI approach. The prefix isolation pattern could inform how CrossHookk manages game-specific configurations.

**Confidence**: High -- public source code, ArchWiki documentation.

### NexusMods.App

- **Approach**: Next-generation mod manager replacing Vortex, with native Linux support.
- **UI framework**: [C# + Avalonia UI](https://nexus-mods.github.io/NexusMods.App/) -- the most directly comparable project to CrossHookk
- **Architecture**: MVVM with clean separation of UI and business logic. Uses [CSS-like styling system](https://nexus-mods.github.io/NexusMods.App/developers/development-guidelines/UIStylingGuidelines/) from Avalonia.
- **Linux distribution**: Available as [AppImage on Linux](https://github.com/Nexus-Mods/NexusMods.App/releases), including Steam Deck
- **Key insight**: A major game modding organization chose Avalonia specifically for native Linux support after their previous tool (Vortex) required WINE on Linux.
- **Relevance**: **Most relevant competitive reference.** Validates Avalonia for game modding tools. Demonstrates that .NET + Avalonia can deliver production-quality native Linux game tool UX.

**Confidence**: High -- public source code, official documentation, active development.

### Vortex Mod Manager

- **Approach**: Current Nexus Mods manager (being replaced by NexusMods.App).
- **UI framework**: Electron (TypeScript/React)
- **Linux support**: [Unofficial, via WINE/Lutris](https://lutris.net/games/vortex-mod-manager/). Users report problems when both Proton and Vortex want to write to the same game prefix.
- **Relevance**: Demonstrates the problems with relying on WINE for a tool that manages WINE-based games. Prefix conflicts, path resolution issues, and DLL override collisions are common complaints.

**Confidence**: High -- extensive community documentation of WINE issues.

### Mod Organizer 2

- **Approach**: Advanced mod manager for Bethesda games, using virtual filesystem (USVFS).
- **UI framework**: C++ / Qt5
- **Linux support**: Community effort via WINE. The USVFS virtual filesystem layer adds complexity under WINE.
- **Relevance**: Qt5 is another viable cross-platform framework, but C++ is a different ecosystem from .NET. The virtual filesystem approach is more complex than CrossHookk's direct injection model.

**Confidence**: Medium -- Linux community reports are anecdotal.

### BepInEx

- **Approach**: Unity game modding framework using DLL proxy injection.
- **Linux support**: [Documented Proton/WINE guide](https://docs.bepinex.dev/articles/advanced/proton_wine.html) using `WINEDLLOVERRIDES` for DLL proxy loading
- **Key technique**: Uses `winhttp.dll` proxy DLL -- requires `WINEDLLOVERRIDES="winhttp=n,b"` to force native DLL loading under WINE
- **Relevance**: The `WINEDLLOVERRIDES` technique is directly applicable to CrossHookk's DLL injection workflow. This is how DLL injection tools typically work under Proton/WINE.

**Confidence**: High -- official BepInEx documentation with specific WINE instructions.

### Summary: What the Competition Teaches Us

| Tool                 | UI Framework | Linux UI Layer     | Game Execution |
| -------------------- | ------------ | ------------------ | -------------- |
| Lutris               | Python/GTK   | Native             | WINE/Proton    |
| Heroic               | Electron     | Native             | WINE/Proton    |
| Bottles              | Python/GTK4  | Native             | WINE           |
| NexusMods.App        | C#/Avalonia  | Native             | Native + WINE  |
| Vortex               | Electron     | WINE (problematic) | WINE/Proton    |
| CrossHookk (current) | C#/WinForms  | WINE               | WINE           |

**Every successful Linux game tool uses a native UI layer.** The tools that rely on WINE for their own UI (Vortex, Mod Organizer 2) are the ones with the most reported problems. CrossHookk should follow the pattern established by NexusMods.App: native .NET UI via Avalonia, with game operations through WINE/Proton.

## Recommendations

### Must Have

- **Native Linux UI rendering**: Eliminate WINE dependency for the CrossHookk Loaderr UI itself. This is the single highest-impact UX improvement. Wine-mono does not support modern .NET, and .NET 8 Desktop Runtime installation in WINE prefixes is unreliable.

- **Self-contained deployment**: Whether staying with WinForms (temporarily) or moving to Avalonia, publish as self-contained to avoid requiring users to install .NET runtimes. For Avalonia on Linux, distribute as AppImage or Flatpak.

- **Visible error handling**: Replace all silent `return false` patterns with user-visible status messages. Every failure in ProcessManager and InjectionManager should produce feedback in the console panel and status bar.

- **Platform detection**: At startup, detect whether running under WINE (check `WINEPREFIX` environment variable) and adjust behavior accordingly -- different error messages, different file path handling, WINE-specific launch method recommendations.

- **MVVM architecture**: Regardless of UI framework choice, separate business logic from UI. The current MainForm.cs has 800+ lines mixing UI construction, event handling, and business logic. This makes any UI migration exponentially harder.

### Should Have

- **Steam Deck layout optimization**: A layout mode specifically for 1280x800 resolution with large touch targets (minimum 48x48px), reduced text input, and prominent action buttons.

- **Improved profile UX**: Replace the current text-input profile dialog with a dropdown-based system. Auto-detect game names from executable paths. Show profile previews before loading.

- **Structured logging**: Replace `Debug.WriteLine` and the console TextBox with a proper logging system (Serilog or Microsoft.Extensions.Logging) that writes to both the UI and a log file for troubleshooting.

- **Dark theme as default**: Avalonia's FluentTheme with Dark mode variant provides a polished dark UI out of the box. The current manual color assignments (`Color.FromArgb(30, 30, 30)`) should be replaced with theme resources for consistency and maintainability.

- **WINE integration for injection**: When running on Linux, automatically set `WINEDLLOVERRIDES` for DLL injection rather than relying on kernel32.dll P/Invoke through WINE. Follow [BepInEx's Proton/WINE pattern](https://docs.bepinex.dev/articles/advanced/proton_wine.html).

### Nice to Have

- **Controller/gamepad navigation**: Avalonia has a [draft PR for gamepad support](https://github.com/AvaloniaUI/Avalonia/pull/18445). When merged, this would enable XY focus navigation for Steam Deck controller use without relying on trackpad-as-mouse.

- **CLI-first mode**: Enhance the existing command-line flags (`-p`, `-autolaunch`, `-dllinject`) into a full CLI mode using Spectre.Console for formatted output. This provides an alternative interface for power users and automation scripts.

- **Profile import/export**: Allow sharing profiles between users or machines. Consider JSON format instead of .ini for better structure and metadata support.

- **Auto-detect game architecture**: When the user selects a game executable, automatically determine 32/64-bit architecture and filter DLL selections accordingly. The `IsDll64Bit` PE header parsing already exists -- surface this information in the UI.

- **Launch method auto-selection**: Instead of exposing 6 radio buttons for launch methods, auto-detect the best method based on the target game and platform. Most users do not understand the difference between CreateProcess and ShellExecute. Provide "Simple" (auto) and "Advanced" (manual) modes.

- **Flatpak packaging**: Package the Avalonia version as a Flatpak for easy installation on SteamOS/Steam Deck via Flathub. [SteamOS natively supports Flatpak](https://www.gamingonlinux.com/guides/view/how-to-install-extra-software-apps-and-games-on-steamos-and-steam-deck/).

## Open Questions

- **Self-contained WinForms under WINE**: Does a self-contained .NET 8 WinForms publish actually run under WINE without separate runtime installation? This needs direct testing. Community reports are inconclusive -- some suggest it works, others report failures. This determines whether an interim WinForms-on-.NET-8 release is viable before the Avalonia migration.

- **Hybrid architecture for DLL injection**: If the UI runs as a native Linux process (Avalonia), how does it orchestrate DLL injection into a WINE-hosted game process? Options include: (a) spawning a small WINE-hosted helper process that performs the injection, (b) using `WINEDLLOVERRIDES` to load DLLs at game startup, or (c) communicating with the WINE prefix via pipes/sockets. NexusMods.App and BepInEx may provide patterns here.

- **Gamepad timeline**: Avalonia's gamepad support PR (#18445) is still a draft. If Steam Deck controller navigation is a priority, should CrossHookk implement its own gamepad-to-keyboard translation layer, or wait for framework support?

- **Profile format migration**: Should .ini profiles be migrated to JSON/TOML for better structure, or maintained for backward compatibility? Can both be supported with a migration path?

- **Scope of UI rewrite**: A WinForms-to-Avalonia migration is estimated as a near-100% UI rewrite. Given the current codebase has ~800+ lines of UI code in MainForm.cs plus Designer code, what is the realistic timeline? The business logic in ProcessManager (~500 lines), InjectionManager (~350 lines), and MemoryManager can be preserved in .NET 8 class libraries.

- **Native AOT feasibility**: Can Avalonia + the P/Invoke-heavy injection code compile to Native AOT? The `DllImport` attributes for kernel32.dll may need to be converted to `LibraryImport` (source-generated P/Invoke) for AOT compatibility.

## Sources

- [WinForms Migration Guide - Microsoft Learn](https://learn.microsoft.com/en-us/dotnet/desktop/winforms/migration/)
- [WinForms Default Font Change - dotnet/winforms #524](https://github.com/dotnet/winforms/issues/524)
- [.NET 8 Form Scaling Breaking Change](https://learn.microsoft.com/en-us/dotnet/core/compatibility/windows-forms/8.0/top-level-window-scaling)
- [wine-mono Repository](https://github.com/madewokherd/wine-mono)
- [Wine Mono 6.14 Release (March 2025)](https://tech.slashdot.org/story/25/03/10/2055237/wine-releases-framework-mono-614)
- [Winetricks dotnet8 Issues](https://github.com/Winetricks/winetricks/issues/2276)
- [Winetricks dotnetdesktop8 Issues](https://github.com/Winetricks/winetricks/issues/2178)
- [Proton .NET Framework Support Request](https://github.com/ValveSoftware/Proton/issues/1786)
- [DotNetDesktop8 in Proton Prefix - Fedora Discussion](https://discussion.fedoraproject.org/t/struggling-to-install-dotnetdesktop8-in-a-proton-prefix/151134)
- [Avalonia UI Platforms](https://avaloniaui.net/platforms)
- [Avalonia Supported Platforms](https://docs.avaloniaui.net/docs/overview/supported-platforms)
- [Avalonia MVVM Documentation](https://docs.avaloniaui.net/docs/concepts/the-mvvm-pattern/avalonia-ui-and-mvvm)
- [Avalonia Gamepad Support Issue #6945](https://github.com/AvaloniaUI/Avalonia/issues/6945)
- [Avalonia Gamepad Support PR #18445](https://github.com/AvaloniaUI/Avalonia/pull/18445)
- [Avalonia WinForms Migration Discussion](https://github.com/AvaloniaUI/Avalonia/discussions/11104)
- [Avalonia WPF Porting Guide](https://avaloniaui.net/blog/the-expert-guide-to-porting-wpf-applications-to-avalonia)
- [.NET MAUI Linux via Avalonia - The Register](https://www.theregister.com/2025/11/13/dotnet_maui_linux_avalonia/)
- [.NET MAUI Linux Support - Microsoft Q&A](https://learn.microsoft.com/en-us/answers/questions/1637588/will-net-maui-include-official-support-for-linux)
- [NexusMods.App - GamingOnLinux](https://www.gamingonlinux.com/2023/11/nexus-mods-app-is-an-in-development-replacement-for-vortex-that-will-support-linux/)
- [NexusMods.App GitHub](https://github.com/Nexus-Mods/NexusMods.App)
- [NexusMods.App UI Styling Guidelines](https://nexus-mods.github.io/NexusMods.App/developers/development-guidelines/UIStylingGuidelines/)
- [Lutris GitHub](https://github.com/lutris/lutris)
- [Heroic Games Launcher GitHub](https://github.com/Heroic-Games-Launcher/HeroicGamesLauncher)
- [Heroic Electron Decision - Interview](https://gardinerbryant.com/an-interview-with-the-dev-team-behind-heroic-games-launcher/)
- [Bottles GitHub](https://github.com/bottlesdevs/Bottles)
- [Bottles 60.0 Wayland Support](https://linuxiac.com/bottles-60-0-launches-with-native-wayland-support/)
- [BepInEx Proton/WINE Guide](https://docs.bepinex.dev/articles/advanced/proton_wine.html)
- [Terminal.Gui](https://gui-cs.github.io/Terminal.Gui/)
- [Spectre.Console](https://spectreconsole.net/)
- [Terminal.Gui vs Spectre.Console Comparison](https://www.libhunt.com/compare-spectre.console-vs-Terminal.Gui)
- [.NET 8 Performance Improvements](https://devblogs.microsoft.com/dotnet/performance-improvements-in-net-8/)
- [.NET 9 Performance Improvements](https://devblogs.microsoft.com/dotnet/performance-improvements-in-net-9/)
- [.NET 9 Performance Summary](https://abp.io/community/articles/.net-9-performance-improvements-summary-gmww3gl8)
- [Native AOT Deployment](https://learn.microsoft.com/en-us/dotnet/core/deploying/native-aot/)
- [.NET Publishing Overview](https://learn.microsoft.com/en-us/dotnet/core/deploying/)
- [WinForms Single File Publish Issue](https://github.com/dotnet/winforms/issues/11473)
- [Steam Deck Controller Guide](https://steamcommunity.com/sharedfiles/filedetails/?id=2804823261)
- [Steam Deck FAQ - Steamworks](https://partner.steamgames.com/doc/steamdeck/faq)
- [Steam Deck Desktop Mode as Gaming Mode](https://steamcommunity.com/app/1675200/discussions/0/3269060419613253936/)
- [Avalonia Flatpak Discussion](https://github.com/AvaloniaUI/Avalonia/discussions/10183)
- [SteamOS Flatpak Guide](https://www.gamingonlinux.com/guides/view/how-to-install-extra-software-apps-and-games-on-steamos-and-steam-deck/)
- [.NET Migration Guide - wojciechowski.app](https://wojciechowski.app/en/articles/dotnet-migration-guide)
- [.NET Cross-Platform Showdown: MAUI vs Uno vs Avalonia](https://dev.to/biozal/the-net-cross-platform-showdown-maui-vs-uno-vs-avalonia-and-why-avalonia-won-ian)
- [Vortex on Lutris](https://lutris.net/games/vortex-mod-manager/)

# Note

This file explores future UX and framework directions, including Avalonia. It is intentionally broader than the active migration plan. The current plan keeps WinForms and treats Avalonia as follow-up work.
