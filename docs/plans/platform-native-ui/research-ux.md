# UX Research: Platform-Native Linux UI for CrossHook

## Executive Summary

CrossHook's native Linux UI must serve two distinct interaction modes: a traditional desktop experience (mouse/keyboard on GNOME/KDE) and a controller-driven 10-foot interface for Steam Deck Gaming Mode. The recommended approach is a convergent UI architecture that adapts layout and input handling based on context, following patterns established by Lutris Gamepad UI, Playnite Fullscreen Mode, and KDE Kirigami's convergent design principles. The UI should adopt a dark gaming aesthetic with high-contrast focus indicators, progressive disclosure of advanced configuration, and a three-step primary workflow: select profile, launch game, monitor session.

**Confidence**: High -- based on convergent patterns from multiple successful Linux game launchers and official HIG documentation.

## User Workflows

### Primary Flow: Game Profile Launch

1. **Select Profile**: User browses game profiles displayed as cover-art cards in a grid/shelf layout. Controller: D-pad/stick navigation with large focus rings. Desktop: click or keyboard arrow keys.
   - System response: Selected profile card enlarges or gains a highlighted border; detail panel slides in showing trainer list, injection config, and last-played timestamp.

2. **Review Configuration**: User sees the assigned trainers (FLiNG, WeMod, etc.), DLL injection settings, and Proton/WINE runner version.
   - System response: Toggle switches for each trainer cheat are visible but disabled until launch. An "Edit Profile" action is available via secondary button (controller: Y/Triangle; desktop: right-click or gear icon).

3. **Launch Game**: User presses primary action (controller: A/Cross; desktop: double-click or Enter).
   - System response: Progress indicator appears with stages -- "Starting Steam..." then "Launching game..." then "Injecting trainer..." then "Monitoring." Each stage shows a brief status line. If the game requires Steam to be running, the app launches Steam first and waits.

4. **In-Session Monitoring**: UI transitions to a minimal "Now Playing" state showing game name, active trainer toggles, and session timer.
   - System response: Trainer toggles become interactive. Status indicators show injection health (green = active, yellow = warning, red = failed). Hotkey hints are displayed.

5. **Session End**: User closes game or stops session from CrossHook.
   - System response: Session summary with playtime, any errors encountered, and option to save/update the profile configuration.

**Confidence**: High -- workflow modeled after WeMod's three-step flow (select, configure, play) and Lutris Gamepad UI's "Now Playing" state pattern.

### Primary Flow: First-Time Setup

1. **Welcome Screen**: Brief introduction to CrossHook with a "Get Started" button. No configuration required to proceed.
   - System response: Single-page onboarding explaining what CrossHook does. Follow Bottles' pattern of progressive disclosure during first run.

2. **Steam Detection**: App automatically scans for Steam installation at standard paths (`~/.steam`, `~/.local/share/Steam`, Flatpak paths).
   - System response: If found, shows confirmation with detected path and game count. If not found, shows a file picker with guidance text: "Could not detect Steam. Please select your Steam installation directory."

3. **Game Library Scan**: App reads `steamapps/appmanifest_*.acf` files to build the game catalog.
   - System response: Progress bar with "Scanning library... Found X games" counter. Scan runs in background; user can proceed before completion.

4. **Trainer Configuration**: User is prompted to configure trainer sources (local trainer directory, WeMod path, FLiNG trainer folder).
   - System response: Auto-detect common trainer locations. Show file browser for manual selection. Allow skipping with "Configure Later" option.

5. **Completion**: Summary of detected configuration with "Start Using CrossHook" button.
   - System response: Transition to main library view with detected games. If no games found, show a placeholder page with guidance.

**Confidence**: High -- modeled after Bottles' first-run wizard pattern and Heroic Games Launcher's initial store login flow.

### Primary Flow: Game Profile Creation

1. **Initiate Creation**: User selects "New Profile" (+ button in header bar or FAB-style button in controller mode).
   - System response: Multi-step form appears -- either a dialog (desktop) or a full-screen flow (controller mode).

2. **Select Game**: Browse detected Steam games or manually specify an executable path.
   - System response: Searchable game list with cover art, sorted by recently played. Type-ahead search with on-screen keyboard for controller input.

3. **Choose Trainers**: Select from detected trainers compatible with the chosen game.
   - System response: List of available trainers with compatibility indicators (verified, untested, incompatible). Toggle to attach each trainer. Show version info and last-updated date.

4. **Configure Injection**: Set DLL injection parameters, timing (inject on launch, delayed inject, manual trigger), and WINE/Proton runner.
   - System response: Sensible defaults pre-selected (inject on launch, system default runner). Advanced options hidden behind expandable section.

5. **Save Profile**: Name the profile and save.
   - System response: Profile card appears in library with auto-generated or user-selected cover art. Toast notification confirms creation.

**Confidence**: High -- follows Bottles' bottle creation flow (environment selection then detail configuration) and Lutris' cascading configuration hierarchy.

### Alternative Flows

- **Quick Launch (no profile)**: User can launch any detected Steam game directly without creating a profile. Trainers must be manually attached during session. Useful for one-off testing.
- **Import Profile**: Import profiles from a JSON/YAML export file or from a shared community repository.
- **Clone Profile**: Duplicate an existing profile to create a variant (e.g., same game, different trainer set).
- **Steam Deck Gaming Mode Entry**: CrossHook is added as a non-Steam game. On launch from Gaming Mode, it opens directly in controller-optimized fullscreen layout, bypassing desktop mode entirely.

**Confidence**: Medium -- alternative flows based on patterns from Lutris (multiple launch configurations) and Playnite (fullscreen mode auto-detection).

### In-Session Management Flow

1. **Toggle Trainers**: During gameplay, user switches to CrossHook overlay or window to toggle individual cheat functions.
   - Controller mode: Steam button brings up Quick Access; CrossHook overlay accessible via configurable hotkey (Steam + specific button combination).
   - Desktop: Alt-Tab to CrossHook window showing active toggles.

2. **Pause/Resume Trainer**: Temporarily suspend all trainer injection without stopping the game.
   - System response: All toggles gray out. Status indicator changes to "Paused." Resume re-enables all previously active toggles.

3. **Hotkey Management**: Each trainer cheat maps to a configurable hotkey or controller button combination.
   - System response: On-screen hints show active hotkey bindings. Controller mode shows button glyphs matching the connected controller type (Xbox, PlayStation, generic).

4. **Status Monitoring**: Real-time indicators for process health, memory usage, and injection status.
   - System response: Minimal HUD-style status bar with color-coded indicators. Expandable for detailed metrics.

**Confidence**: Medium -- based on WeMod's in-game overlay patterns and FLiNG's hotkey toggle system, adapted for controller input using Lutris Gamepad UI's approach.

## UI/UX Best Practices

### Linux Desktop Design Standards

#### GNOME Human Interface Guidelines (HIG)

- **Design for People**: Seek to be as inclusive as possible, accommodating different abilities and input methods. Software should require minimal specialist knowledge.
- **Make it Simple**: CrossHook does one thing well -- manage and launch game trainers. Employ progressive disclosure to hide advanced injection/memory settings behind expandable sections.
- **Reduce User Effort**: Auto-detect Steam installation, scan game libraries, and pre-configure sensible defaults. Minimize manual configuration steps.
- **Be Considerate**: Enable undo for destructive actions (e.g., deleting a profile). Prevent common mistakes by validating trainer compatibility before launch.
- **Pattern Categories**: Use GNOME's four pattern categories -- Containers (header bars, boxed lists, grid views), Navigation (sidebars, tabs, search), Controls (buttons, switches, toggles), and Feedback (toasts, banners, progress bars, spinners).

Sources: [GNOME HIG Design Principles](https://developer.gnome.org/hig/principles.html), [GNOME HIG Patterns](https://developer.gnome.org/hig/patterns.html)

**Confidence**: High -- official GNOME documentation, current as of 2025.

#### KDE Kirigami Convergent Design

- **Context-Aware Adaptation**: UI morphs based on input method (mouse/keyboard vs. controller/touch) and screen size (desktop vs. Steam Deck 1280x800).
- **Layout Rules**: Portrait/narrow screens show single-column hierarchies; landscape/wide screens reveal multiple columns simultaneously. Larger screens expose more permanent controls; smaller screens prioritize essential controls, hiding secondary options until requested.
- **Input-Specific Interactions**: Desktop versions leverage hover effects for inline controls; controller versions implement directional navigation with prominent focus indicators.

Source: [KDE Kirigami HIG - Optimized Convergence](https://community.kde.org/KDE_Visual_Design_Group/KirigamiHIG/Principles/OptimizedConverence)

**Confidence**: High -- official KDE documentation on convergent UI patterns.

### 10-Foot Interface / Steam Deck Gaming Mode

#### Core 10-Foot Design Guidelines

- **Font Sizing**: All text must be readable at 10 feet (approximately 3 meters). Minimum body text size should be equivalent to 24px at 1280x800 resolution. Headers at 32-48px equivalent.
- **Touch/Focus Targets**: Minimum interactive element size of 48x48 device-independent pixels. Focus rings must be at least 2px thick with 3:1 contrast ratio against background.
- **Safe Zone Margins**: Keep all critical UI elements within the inner 85-90% of the screen. On Steam Deck (1280x800), this means approximately 64-96px margins on each edge.
- **Color Considerations**: Avoid pure white (#ffffff) as it can create halo effects on certain displays. Clamp highlight colors to reduce visual artifacts. Prefer off-white (#f0f0f0 or lighter grays) for text on dark backgrounds.
- **Navigation Depth**: Content should be accessible within 1-3 controller actions from any screen. Avoid deeply nested menus.
- **Line Thickness**: Borders and separators should be at least 2px to prevent rendering artifacts on various display types.
- **Controller Input Vocabulary**: Design around six core inputs -- up, down, left, right, confirm (A), back (B). Everything must be navigable and activatable without looking at the controller.

Sources: [Microsoft 10-Foot Experience Guide](https://learn.microsoft.com/en-us/windows/win32/dxtecharts/introduction-to-the-10-foot-experience-for-windows-game-developers), [Grokipedia 10-Foot UI](https://grokipedia.com/page/10-foot_user_interface)

**Confidence**: High -- Microsoft's 10-foot experience guidelines are the industry standard reference, updated 2025.

#### Steam Deck-Specific Integration

- **Non-Steam Game Addition**: CrossHook is added via "Add a Non-Steam Game" in Desktop Mode. Custom artwork (grid, hero, logo, icon) should be provided for proper library presentation.
- **Controller Layout**: Ship a recommended Steam Input controller layout configuration. Map trainer toggles to button combinations (e.g., back grip + face button). Provide a template users can import.
- **Quick Access Integration**: If possible, integrate with Decky Loader's plugin architecture for a native Quick Access Menu panel. This would allow toggling trainers without leaving the game.
- **Gaming Mode Launch**: When launched from Gaming Mode, detect the Steam overlay environment and automatically enter fullscreen/controller mode. Check environment variable `SteamDeck=1` or `$STEAM_DECK` for detection.
- **Known Limitation**: In Desktop Mode, non-Steam games always use the Desktop controller layout. Controller layout customization only works reliably in Gaming Mode.

Sources: [Steam Deck Controller Guide](https://steamcommunity.com/sharedfiles/filedetails/?id=2804823261), [Valve Steam for Linux Issues](https://github.com/ValveSoftware/steam-for-linux/issues/8904)

**Confidence**: High -- based on documented Steam Deck behavior and community-verified integration patterns.

### Game Launcher UI Patterns (from existing tools)

#### Library Views

- **Grid View (Primary)**: Games displayed as cover-art cards in a responsive grid. Each card shows game name, cover art, and a subtle status indicator (installed, running, has trainer). This is the dominant pattern across Lutris, Heroic, Playnite, and Steam itself.
- **List View (Secondary)**: Compact rows with game name, trainer count, last played, and status. Better for large libraries (100+ games).
- **Shelf View (Controller Mode)**: Horizontal scrolling shelves grouped by category -- "Recently Played," "All Games," custom categories. Each shelf scrolls independently. Pattern from Lutris Gamepad UI and Steam Big Picture.

Sources: [Lutris Desktop Client](https://github.com/lutris/lutris), [Lutris Gamepad UI](https://github.com/andrew-ld/lutris-gamepad-ui), [Playnite Fullscreen Mode](https://api.playnite.link/docs/manual/gettingStarted/playniteFullscreenMode.html)

**Confidence**: High -- consistent pattern across all major Linux game launchers.

#### Sidebar Navigation

- **Desktop Mode**: Collapsible sidebar with sections -- Library, Profiles, Downloads, Settings. Filter controls (platform, trainer type, status) in the sidebar. Follows GNOME's sidebar navigation pattern.
- **Controller Mode**: Replace sidebar with a top-level tab bar or bumper-switchable screens (LB/RB to switch sections). Matches Steam Big Picture and Playnite Fullscreen navigation patterns.

**Confidence**: High -- consistent across GNOME apps (Bottles, Lutris) and controller-mode launchers.

#### Detail Panels

- **Split View (Desktop)**: Selecting a game in the library opens a detail panel on the right side (or as an overlay). Shows cover art, description, trainer list, launch button, and settings. Follows Bottles' bottle detail page pattern.
- **Full-Page Detail (Controller)**: Selecting a game transitions to a full-page detail view with large cover art, trainer toggles, and a prominent "Play" button. Back button returns to library. Follows Heroic's game page and Steam's game detail pattern.

**Confidence**: High -- well-established pattern in both desktop and controller interfaces.

### Dark Theme / Gaming Aesthetic

- **Default to Dark**: Gaming applications universally default to dark themes. Use a dark background (e.g., #1a1a2e or #0f0f23) with high-contrast text (#e0e0e0 or #f5f5f5).
- **Accent Colors**: Use a vibrant accent color for interactive elements, focus indicators, and active states. Common gaming accents: electric blue (#0078d4), neon green (#00ff41), purple (#7b2ff7).
- **GTK4/libadwaita Theming**: If building with GTK4, use libadwaita's built-in dark mode with `AdwStyleManager`. Override named colors for the gaming accent. This ensures the app respects system-wide dark/light preference while defaulting to dark.
- **Custom Theming (Non-GTK)**: If using a web-based renderer (Electron, Tauri), implement a JSON-based theme system following Lutris Gamepad UI's pattern -- ship a `theme.default.json` reference and allow `theme.json` user overrides. Apply changes without restart.

Sources: [Lutris Gamepad UI theming](https://github.com/andrew-ld/lutris-gamepad-ui), [libadwaita documentation](https://gnome.pages.gitlab.gnome.org/libadwaita/), [Arch Wiki GTK Theming](https://wiki.archlinux.org/title/GTK)

**Confidence**: High -- dark theme is a universal gaming app convention; implementation details well-documented.

### Dashboard Patterns for Active Sessions

- **Now Playing State**: When a game is running, the main view transitions to a minimal dashboard showing:
  - Game name and cover art (large)
  - Session timer (elapsed time)
  - Active trainer toggles (switches that can be toggled in real-time)
  - Process health indicator (green/yellow/red dot)
  - Hotkey reference card (collapsible)
- **Overlay Widget Pattern**: For in-game status, use a small floating widget (similar to Android Game Dashboard's shortcut bar) that shows key metrics without obscuring gameplay. Keep it to the corner of the screen with adjustable transparency and position.
- **Performance Metrics**: CPU/memory usage of the trainer process displayed as numerical values or small sparkline graphs. Follow FPS Monitor's pattern of real-time stats overlay with configurable verbosity levels.

Sources: [Android Game Dashboard Components](https://developer.android.com/games/gamedashboard/components), [Overwolf In-Game Overlay Guidelines](https://dev.overwolf.com/ow-electron/guides/product-guidelines/app-screen-behavior/in-game-overlays/), [FPS Monitor](https://fpsmon.com/en/)

**Confidence**: Medium -- patterns translated from game overlay tools and mobile game dashboards to a desktop/Steam Deck context.

### Accessibility

#### WCAG-Aligned Focus Management

- **Visible Focus**: Every interactive element must have a visible focus indicator when navigated to via keyboard or controller. Use a minimum 2px border with 3:1 contrast ratio against the element's unfocused background.
- **Logical Focus Order**: Tab/D-pad order must follow the visual layout -- left-to-right, top-to-bottom. Never use arbitrary focus ordering.
- **Focus Trapping in Modals**: When a dialog or overlay is open, focus must be trapped within it. Pressing Back/Escape closes the modal and returns focus to the trigger element.

Source: [WCAG 2.4.7 Focus Visible](https://www.w3.org/WAI/WCAG22/Understanding/focus-visible.html), [WCAG 2.4.3 Focus Order](https://www.digitala11y.com/focus-order-understanding-sc-2-4-3/)

**Confidence**: High -- WCAG standards are authoritative and directly applicable.

#### Controller-Specific Accessibility

- **Adaptive Button Prompts**: Display controller glyphs matching the connected device (Xbox, PlayStation, generic). Lutris Gamepad UI dynamically adapts button prompts to the connected controller, reducing cognitive load.
- **Audio Feedback**: Provide subtle sound effects for navigation (focus change), selection (confirm), and errors (rejection buzz). Lutris Gamepad UI implements audio feedback for all interactions -- essential for Steam Deck users who may not maintain constant visual focus.
- **On-Screen Keyboard**: Provide a built-in on-screen keyboard for text input in controller mode. Integrate with Steam's virtual keyboard if available, or provide a custom one for non-Steam launch contexts.

Source: [Lutris Gamepad UI](https://github.com/andrew-ld/lutris-gamepad-ui)

**Confidence**: High -- patterns validated in production by Lutris Gamepad UI.

#### Desktop Accessibility

- **Keyboard Navigation**: Full keyboard navigability without mouse. Standard shortcuts: Tab for focus cycling, Enter/Space for activation, Escape for cancel/back, Ctrl+F for search.
- **Screen Reader Support**: If using GTK4, leverage ATK/AT-SPI accessibility framework. Label all interactive elements. Provide text alternatives for icons.
- **Reduced Motion**: Respect `prefers-reduced-motion` setting. Disable animations and transitions for users who have enabled this.

**Confidence**: Medium -- GTK4 has built-in accessibility support but requires explicit developer effort to implement fully.

## Error Handling

### Error Design Principles

- **Proximity**: Place error messages as close to their cause as possible. Inline errors next to the failed element are more effective than distant toasts.
- **Persistence**: Error messages for actionable items must persist until the user acknowledges or resolves them. Do not auto-dismiss error toasts for failures that require user action.
- **Specificity**: Error messages must say what went wrong AND what the user can do about it. Avoid generic "Something went wrong" messages.
- **Progressive Detail**: Show a brief user-friendly message with an expandable "Technical Details" section for advanced users.

Sources: [Smashing Magazine Error Messages UX](https://www.smashingmagazine.com/2022/08/error-messages-ux-design/), [NN/g Error Message Guidelines](https://www.nngroup.com/articles/error-message-guidelines/)

**Confidence**: High -- based on established UX research from Nielsen Norman Group and Smashing Magazine.

### Error States

| Error                              | User Message                                                                                                      | Recovery Action                                                                                                                         | UI Pattern                                                   |
| ---------------------------------- | ----------------------------------------------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------------------------------ |
| Trainer injection failed           | "Could not inject [trainer name] into [game]. The trainer may be incompatible with this game version."            | "Try Again" button + "View Details" expandable with error code. Offer to check for trainer updates.                                     | Inline banner at top of Now Playing view (persistent).       |
| Game process crashed               | "It looks like [game] stopped unexpectedly."                                                                      | "Relaunch" button + "View Crash Log" link. Auto-detach trainer to prevent re-crash on restart.                                          | Full-screen placeholder page replacing Now Playing view.     |
| ptrace permission denied           | "CrossHook needs permission to attach to game processes. Your system's security settings currently prevent this." | Step-by-step guide: "Run `sudo sysctl kernel.yama.ptrace_scope=0` or add CrossHook to the allowed ptrace group." Link to documentation. | First-run check + blocking dialog if detected during launch. |
| Steam not running                  | "Steam is not running. CrossHook needs Steam to launch games."                                                    | "Start Steam" button (auto-launch if path known) + "I'll start it manually" dismiss option.                                             | Banner notification with action button.                      |
| Game not found                     | "Could not find [game] at the expected path. It may have been moved or uninstalled."                              | "Rescan Library" button + "Browse for Game" file picker.                                                                                | Inline error on the profile card + detail panel warning.     |
| Trainer version mismatch           | "This trainer was built for [game] v1.2 but you have v1.5. It may not work correctly."                            | "Launch Anyway" (with risk acknowledgment) + "Check for Updates" button.                                                                | Warning banner (yellow, dismissable) in profile detail view. |
| DLL not found                      | "The DLL file [name.dll] is missing from [path]."                                                                 | "Locate File" file picker + "Remove from Profile" option.                                                                               | Inline error on the DLL entry in profile configuration.      |
| WINE/Proton runner missing         | "The configured Proton runner ([version]) is not installed."                                                      | "Install via ProtonUp-Qt" link + "Select Different Runner" dropdown.                                                                    | Blocking dialog before launch attempt.                       |
| Network timeout (trainer download) | "Could not download [trainer]. Check your internet connection."                                                   | "Retry" button + "Use Offline" option (if cached version available).                                                                    | Toast notification with retry action.                        |

**Confidence**: High -- error states derived from CrossHook's actual architecture (ptrace, DLL injection, Steam integration) combined with established error UX patterns.

### Controller-Mode Error Handling

- **Simplified Messages**: In controller mode, show shorter error messages with fewer options. Primary action on A button, dismiss on B button.
- **No Text Input Required**: Error recovery actions in controller mode must not require typing. Use selection lists instead of text fields for path corrections.
- **Audio Cues**: Play a distinct error sound when an error state is reached. Different tones for warnings (recoverable) vs. errors (blocking).

**Confidence**: Medium -- inferred from controller-mode UX best practices; no direct competitor implements trainer error handling in controller mode.

## Performance UX

### Loading States

- **Initial App Load**: Skeleton screen showing the layout structure (empty grid placeholders, sidebar silhouette) while data loads. Show a progress bar in the header if the initial game scan takes more than 1 second. Avoid blank screens or spinners without context.
- **Game Library Scan**: Background task with a subtle progress indicator in the header or status bar. "Scanning... 42/128 games found" counter. User can interact with already-loaded games during scan. Follow Lutris' pattern of allowing use before scan completion.
- **Profile Loading**: Instant for local profiles (< 100ms target). If loading trainer metadata from network, show the profile with a skeleton loader for the trainer section.
- **Game Launch Sequence**: Multi-stage progress with named steps:
  1. "Starting Steam..." (if not running)
  2. "Launching [game]..." (waiting for process to appear)
  3. "Attaching trainer..." (injection phase)
  4. "Ready" (monitoring active)
     Each stage shows a brief animated transition (spinner or progress arc). Total expected time displayed if estimable.

**Confidence**: High -- loading state patterns are well-established in modern UI frameworks.

### Real-Time Monitoring

- **Process Health Indicator**: A small colored dot (green/yellow/red) next to the game name in the Now Playing view. Green = process running and trainer active. Yellow = process running but trainer detached or unstable. Red = process crashed or trainer failed.
- **Memory/CPU Metrics**: Optional expandable panel showing trainer process memory usage and CPU utilization. Displayed as numerical values with trend arrows (up/down/stable). Update interval: 1-2 seconds.
- **Injection Status Per-DLL**: Each injected DLL shows its own status indicator. Hover (desktop) or select (controller) for details: load address, status, any warnings.

**Confidence**: Medium -- monitoring patterns adapted from system monitoring tools (FPS Monitor, MangoHud) and Android Game Dashboard. Specific trainer monitoring UI is novel.

### Background Operations

- **Trainer Downloads/Updates**: Background download with progress bar in a notification area or downloads section. Do not block the main UI. Show estimated time remaining for large downloads. Provide a download queue view.
- **Game Library Rescan**: Triggered manually or on Steam library change detection. Runs in background with minimal UI indication (subtle refresh icon animation in header). New games appear incrementally as discovered.
- **Automatic Update Checks**: Check for trainer compatibility updates on app launch (configurable). Show a badge/counter on the Updates section if updates are available. Never auto-download without user consent.

**Confidence**: Medium -- patterns from Heroic Games Launcher (background downloads) and ProtonUp-Qt (version management UI).

### Performance Budget Targets

- **App Startup**: < 2 seconds to interactive main view (target < 1 second for Tauri/native, acceptable < 2 seconds for Electron).
- **Profile Switch**: < 100ms to render new profile details.
- **Trainer Toggle**: < 50ms visual feedback on toggle action (actual injection may take longer; show immediate optimistic UI update with rollback on failure).
- **Memory Footprint**: < 100MB idle (Tauri target: < 50MB). CrossHook is a utility that runs alongside resource-intensive games; minimal footprint is critical.

**Confidence**: Medium -- targets based on Tauri vs. Electron benchmarks and general game utility performance expectations.

## Competitive Analysis

### Lutris (Linux Game Manager)

- **Approach**: GTK-based desktop application with a sidebar + grid/list library view. Supports multiple "runners" (WINE, DOSBox, emulators, native Linux) with a three-tier cascading configuration system (system -> runner -> game). Community-driven install scripts automate game setup.
- **Strengths**:
  - Mature, well-tested GTK UI that integrates natively with GNOME and other GTK-based desktops.
  - Powerful cascading configuration (system-level defaults, runner-level overrides, game-level specifics) -- directly applicable to CrossHook's trainer/injection hierarchy.
  - Community install scripts for automated game setup reduce user effort dramatically.
  - Separate [Lutris Gamepad UI project](https://github.com/andrew-ld/lutris-gamepad-ui) provides a 10-foot interface with controller-adaptive button prompts, audio feedback, horizontal shelf navigation, and system controls (audio, Bluetooth, display, power).
- **Weaknesses**:
  - Desktop and gamepad UIs are separate applications, not a unified convergent design. Users must choose one or the other.
  - Gamepad UI is a third-party project, not officially maintained by Lutris.
  - No built-in trainer/cheat management -- purely a game launcher.
  - The 2025 review notes that while Lutris has made great progress, "the road is still long" in terms of polish.
- **Key Takeaway for CrossHook**: Adopt the cascading configuration model. Build convergent desktop/controller modes into a single application rather than separate projects. Emulate the Gamepad UI's shelf navigation and adaptive button prompts for controller mode.

Sources: [Lutris Desktop Client](https://github.com/lutris/lutris), [Lutris Gamepad UI](https://github.com/andrew-ld/lutris-gamepad-ui), [Lutris 2025 Review](https://www.dedoimedo.com/games/lutris-2025-review.html)

**Confidence**: High -- based on direct examination of Lutris architecture and multiple reviews.

### Heroic Games Launcher

- **Approach**: Electron-based cross-platform launcher for Epic, GOG, and Amazon games. Features a sidebar navigation, grid/list library views, per-game settings pages, and built-in WINE/Proton management.
- **Strengths**:
  - Clean, modern web-based UI with a consistent dark theme.
  - Per-game detail pages with integrated settings (WINE version, launch options, DLC management).
  - "Auto add to Steam" feature for seamless Steam library integration -- games added to Heroic automatically appear in Steam's library as non-Steam shortcuts.
  - Active development with regular UI/UX improvements (v2.17 in 2025).
  - Flatpak distribution for Steam Deck compatibility.
- **Weaknesses**:
  - Electron overhead: significantly higher memory usage and slower startup compared to native alternatives.
  - UI scaling issues on Steam Deck Gaming Mode -- the UI can appear 2x smaller than intended when launched from SteamUI, though it works properly in Desktop Mode.
  - Gamepad input issues with certain Flatpak runtimes (runtime 24.08 broke gamepad input, requiring revert to 23.08).
  - No dedicated controller-optimized fullscreen mode -- relies on the same desktop UI scaled to screen.
- **Key Takeaway for CrossHook**: The "auto add to Steam" pattern is essential -- CrossHook should offer one-click Steam shortcut creation for each game profile. Learn from Heroic's UI scaling problems by testing controller mode rendering explicitly. Avoid Electron if possible to reduce resource competition with games.

Sources: [Heroic Games Launcher](https://heroicgameslauncher.com/), [Heroic Steam Deck Wiki](https://github.com/Heroic-Games-Launcher/HeroicGamesLauncher/wiki/Steam-Deck), [Heroic UI Scaling Issue](https://github.com/Heroic-Games-Launcher/HeroicGamesLauncher/discussions/1287)

**Confidence**: High -- based on official documentation and community issue reports.

### Bottles (WINE Prefix Manager)

- **Approach**: GTK4/libadwaita application for managing WINE prefixes ("bottles"). Uses GNOME Adwaita design language with a wizard-based onboarding flow, environment presets (Gaming, Application, Custom), and per-bottle detail views for DLL overrides, environment variables, and runner selection.
- **Strengths**:
  - Excellent GNOME integration via libadwaita -- looks and feels native on GNOME desktops.
  - Progressive disclosure in bottle creation: users choose a preset environment first (Gaming/Application/Custom), then customize later. Separates initial setup from ongoing configuration.
  - First-run experience is well-designed: welcome screen, automatic component download with checksum verification, offline handling, then guided bottle creation.
  - DLL override management, environment variables, and runner selection are directly relevant UI patterns for CrossHook's injection configuration.
  - Bottles 60.0 added native Wayland support and redesigned the creation dialog and detail views for consistency.
- **Weaknesses**:
  - No controller/gamepad support -- purely a desktop application.
  - GNOME-specific design may feel out of place on KDE or other desktop environments.
  - No game library management -- focused on WINE prefixes, not game organization.
- **Key Takeaway for CrossHook**: Directly emulate Bottles' configuration UI patterns for DLL injection settings, environment variables, and runner selection. Adopt the progressive disclosure model (preset -> customize) for profile creation. Use libadwaita if targeting GTK4 for native GNOME feel.

Sources: [Bottles First Run](https://docs.usebottles.com/getting-started/first-run), [Bottles Environments](https://docs.usebottles.com/getting-started/environments), [Bottles 60.0 Release](https://linuxiac.com/bottles-60-0-launches-with-native-wayland-support/)

**Confidence**: High -- based on official Bottles documentation and release notes.

### ProtonUp-Qt (Proton Version Manager)

- **Approach**: Qt 6-based utility for installing and managing compatibility tools (GE-Proton, Luxtorpeda, Wine-GE) for Steam and Lutris. Focused, single-purpose tool with a minimal UI.
- **Strengths**:
  - Single-purpose clarity -- does one thing (manage Proton/WINE versions) and does it well.
  - Qt-based for cross-desktop compatibility (works on GNOME, KDE, and others without looking out of place).
  - Available as both Flatpak and AppImage for broad distribution.
  - Dark theme support via Qt theming with user-accessible settings.
- **Weaknesses**:
  - Very utilitarian UI with minimal visual polish.
  - No game library integration -- users must know which compatibility tool they need.
  - No controller support.
- **Key Takeaway for CrossHook**: Consider Qt 6 as a toolkit option for cross-desktop compatibility. If CrossHook includes runner/Proton version management, the integration should be more contextual than ProtonUp-Qt's standalone approach -- show runner version selection within profile configuration, not as a separate tool.

Sources: [ProtonUp-Qt](https://davidotek.github.io/protonup-qt/), [ProtonUp-Qt GitHub](https://github.com/DavidoTek/ProtonUp-Qt)

**Confidence**: High -- based on official project documentation.

### WeMod (Windows Trainer Platform)

- **Approach**: Windows-native trainer platform with a clean, modern UI. Features a game library sidebar, per-game trainer panel with toggleable cheats, one-click activation, and an in-game overlay for real-time toggle adjustment.
- **Strengths**:
  - Three-step workflow (Download -> Choose Features -> Play Enhanced) is the gold standard for trainer UX simplicity.
  - Cheats are presented as simple toggles -- no technical knowledge required. Each cheat has a clear name (e.g., "Infinite Health," "Unlimited Ammo") and an on/off switch.
  - In-game overlay allows real-time cheat toggling without alt-tabbing.
  - All changes are temporary -- toggling off restores original game behavior. This reduces user anxiety about permanent modifications.
  - Automatic game detection -- scans PC for installed games and matches them to available trainers.
- **Weaknesses**:
  - Windows-only -- Linux users must use community workarounds (DeckCheatz/wemod-launcher).
  - The game library sidebar is always visible and cannot be hidden, wasting screen space during active sessions.
  - Limited hotkey customization -- users often resort to AutoHotKey for remapping.
  - No controller-native interaction -- trainers require keyboard hotkeys, problematic for Steam Deck.
  - WeMod's Linux launcher (DeckCheatz) relies on WINE prefix management and has a complex setup process that undermines WeMod's simplicity promise.
- **Key Takeaway for CrossHook**: Replicate WeMod's toggle-based cheat presentation and three-step activation flow. Solve WeMod's weakness by building controller-native trainer toggling from the start. Implement the "temporary changes" principle -- make it clear that trainer effects are reversible. Avoid WeMod's locked sidebar by providing a collapsible or hideable library panel.

Sources: [WeMod](https://www.wemod.com/), [WeMod UI Customization Discussion](https://community.wemod.com/t/ui-customization/371434), [DeckCheatz wemod-launcher](https://github.com/DeckCheatz/wemod-launcher)

**Confidence**: High -- based on official WeMod documentation and community feedback.

### FLiNG Trainers (Windows Trainer Creator)

- **Approach**: Standalone trainer executables per-game, built using WPF. Each trainer has a self-contained window listing available cheats with associated keyboard hotkeys. A collection manager app (FLiNG Trainer Collection) provides a unified download/management interface.
- **Strengths**:
  - Hotkey-driven interaction -- each cheat has a dedicated keyboard shortcut. Trainers can run in the background while the game is in focus.
  - Toggle-all functionality -- a master hotkey to enable/disable all hotkey listening, useful when the game needs the same keys.
  - Trainers are self-contained -- no always-online requirement or account system.
- **Weaknesses**:
  - Per-game standalone executables with no unified management (addressed by the third-party Collection app).
  - No controller support -- strictly keyboard-driven hotkeys.
  - No overlay -- users must alt-tab to see trainer state.
  - Visual design is functional but dated -- minimal aesthetic effort.
- **Key Takeaway for CrossHook**: Adopt FLiNG's hotkey model but extend it with controller mapping. Implement the "toggle all" master switch. Provide unified trainer management (which FLiNG lacks) as a core feature.

Sources: [FLiNG Trainer](https://flingtrainer.com/), [FLiNG Trainer Collection (GitHub)](https://github.com/Melon-Studio/FLiNG-Trainer-Collection)

**Confidence**: Medium -- limited public documentation on FLiNG's UX design; based on community descriptions and the Collection app.

### Playnite (Windows Game Launcher with Fullscreen Mode)

- **Approach**: Windows-based unified game library manager with two distinct modes: Desktop Mode (traditional UI) and Fullscreen Mode (controller-optimized 10-foot interface). Fullscreen mode uses SDL for controller input and provides a grid-based library with bumper-switchable filter presets.
- **Strengths**:
  - Dual-mode design (desktop + fullscreen) in a single application -- the exact convergent pattern CrossHook needs.
  - Fullscreen mode uses SDL library for broad controller compatibility (XInput and DirectInput).
  - LB/RB bumper switching for filter presets in the top panel -- efficient controller navigation for library filtering.
  - Extensive customization: visibility of UI elements, font selection, panel placement, cover height, spacing. Community themes available.
  - F11 toggle between desktop and fullscreen modes.
- **Weaknesses**:
  - Windows-only (no native Linux support).
  - Controller support is exclusively in Fullscreen mode -- Desktop mode has no gamepad navigation.
  - Relies on community-maintained SDL gamepad mappings for hardware compatibility.
- **Key Takeaway for CrossHook**: Emulate Playnite's dual-mode architecture with a desktop view and a controller-optimized fullscreen view, switchable via hotkey or auto-detected via launch context. Use SDL (or similar) for broad controller compatibility. Adopt bumper-switchable tab navigation for controller mode.

Sources: [Playnite Fullscreen Mode](https://api.playnite.link/docs/manual/gettingStarted/playniteFullscreenMode.html), [Playnite](https://playnite.link/), [XDA Developers Playnite Review](https://www.xda-developers.com/playnite-alternative-steam-big-picture-handhelds/)

**Confidence**: High -- well-documented features in official Playnite documentation.

### Steam Big Picture / Gaming Mode

- **Approach**: Valve's controller-optimized interface for Steam, shared between Steam Deck Gaming Mode and desktop Big Picture Mode. React-based UI injected into the Steam client.
- **Strengths**:
  - Universal Search at the top of the UI for quick access to Library, Friends, and Store.
  - Quick Access Menu (Steam + A) for notifications, friends, quick settings -- a side panel that slides in without leaving the current context.
  - System menu (Steam/Guide button) for top-level navigation between different sections.
  - Controller configurator designed for ease-of-use -- pick, adjust, or create custom controller configurations per game.
  - Virtual menus on touchpad/D-pad/thumbstick with visual overlays during gameplay.
- **Weaknesses**:
  - Custom artwork for non-Steam games is limited -- cannot easily add custom images through the new UI.
  - Custom launch options for emulators/launchers can break in the new Big Picture mode.
- **Key Takeaway for CrossHook**: Follow Steam's Quick Access Menu pattern for in-session trainer management -- a side panel that slides in on a button press without obscuring the full screen. Adopt the Universal Search pattern for finding games and trainers quickly. Support Steam's virtual menu overlays for trainer toggle mapping.

Sources: [Steam Big Picture Mode updates](https://www.gamingonlinux.com/2022/10/steam-deck-ui-comes-to-desktop-in-beta-to-replace-big-picture-mode/), [Steam Deck Controller Guide](https://steamcommunity.com/sharedfiles/filedetails/?id=2804823261)

**Confidence**: High -- Steam's UI patterns are the de facto standard for controller-driven game interfaces.

### Decky Loader (Steam Deck Plugin Framework)

- **Approach**: Homebrew plugin loader that injects React components into the Steam Deck Gaming Mode UI. Plugins appear in the Quick Access Menu and can provide overlays, system tweaks, and additional functionality.
- **Strengths**:
  - Native integration with Gaming Mode -- plugins feel like built-in features.
  - Marketplace for discovering and installing plugins without leaving controller mode.
  - Overlay capabilities for performance monitoring with configurable verbosity (four levels from minimal to detailed).
  - Plugins can modify Steam UI elements (themes, sounds, system settings).
- **Weaknesses**:
  - Requires homebrew installation -- not officially supported by Valve.
  - Plugin stability can be an issue -- certain plugins cause Decky to fail to start with no indication of the problem.
- **Key Takeaway for CrossHook**: Consider building a Decky Loader plugin as a companion to the main app. This would allow trainer toggles directly from the Steam Deck Quick Access Menu without switching away from the game. The plugin could communicate with the main CrossHook process via local socket/IPC.

Source: [Decky Loader](https://github.com/SteamDeckHomebrew/decky-loader), [Decky Loader Website](https://decky.xyz/)

**Confidence**: Medium -- Decky Loader integration is a stretch goal dependent on API stability and community framework support.

### Comparison Matrix

| Feature                | Lutris          | Heroic         | Bottles             | WeMod            | Playnite         | CrossHook (Target)      |
| ---------------------- | --------------- | -------------- | ------------------- | ---------------- | ---------------- | ----------------------- |
| Linux Native           | Yes (GTK)       | Yes (Electron) | Yes (GTK4)          | No               | No               | Yes                     |
| Controller Mode        | Separate app    | No             | No                  | No               | Yes (Fullscreen) | Yes (Built-in)          |
| Dark Theme             | System          | Yes            | System              | Yes              | Yes (themes)     | Yes (Default)           |
| Game Library           | Yes             | Yes            | No                  | Yes              | Yes              | Yes                     |
| Trainer Management     | No              | No             | No                  | Yes              | No               | Yes                     |
| DLL Injection Config   | No              | No             | Yes (DLL overrides) | Hidden           | No               | Yes                     |
| Progressive Disclosure | Some            | Some           | Yes                 | Yes              | Some             | Yes                     |
| Audio Feedback         | Gamepad UI only | No             | No                  | No               | No               | Yes                     |
| Steam Integration      | Some            | Auto-add       | No                  | Auto-detect      | No               | Deep (launch via Steam) |
| Flatpak Distribution   | Yes             | Yes            | Yes                 | No               | No               | Yes (Target)            |
| First-Run Wizard       | No              | Login flow     | Yes                 | Account creation | No               | Yes                     |

## Recommendations

### Must Have

1. **Convergent Desktop/Controller Architecture**: A single application with two presentation modes (desktop sidebar+grid and fullscreen shelf+detail), switchable via hotkey (F11), launch argument, or auto-detection of Steam Deck Gaming Mode environment. This is the single most important architectural decision.

2. **Three-Step Launch Workflow**: Select Profile -> Launch -> Monitor. Directly replicate WeMod's simplicity. Every additional step between "I want to play" and "I'm playing with cheats active" is friction that will drive users back to WINE-based WeMod.

3. **Controller-Native Trainer Toggling**: Map trainer cheats to controller button combinations. Use Steam Input's virtual menu overlay system for toggle selection. Provide adaptive button prompts (Xbox/PlayStation glyphs). This solves the primary UX gap that WeMod and FLiNG have on Steam Deck.

4. **Dark Gaming Theme by Default**: Ship with a dark theme using high-contrast accent colors. Provide theme customization via a JSON config file. Respect system dark/light preference as an override option.

5. **Progressive Disclosure for Configuration**: Show simple toggle UI by default. Hide DLL paths, injection timing, memory settings, and WINE runner selection behind expandable "Advanced" sections. Follow Bottles' pattern of preset-then-customize.

6. **Clear Error Recovery Paths**: Every error message must include a specific recovery action. Use inline errors near their cause rather than distant toasts. Provide one-click resolution where possible (e.g., "Start Steam" button, "Rescan Library" button).

7. **First-Run Setup Wizard**: Auto-detect Steam, scan game library, configure trainer directories. Allow skipping each step. Complete in under 60 seconds for a typical setup.

8. **Steam Non-Steam Game Integration**: Provide one-click creation of Steam shortcuts for CrossHook itself and for individual game profiles. Include custom artwork assets (grid, hero, logo, icon) for proper Steam library presentation.

### Should Have

9. **Shelf Navigation in Controller Mode**: Horizontal scrolling shelves for "Recently Played," "All Games," and custom categories. Each shelf scrolls independently. Games displayed as large cover-art cards with status indicators.

10. **Audio Feedback for Controller Navigation**: Subtle sounds for focus changes, selections, errors, and state transitions. Essential for Steam Deck users who may not maintain constant visual focus.

11. **Multi-Stage Launch Progress**: Named progress steps during game launch ("Starting Steam...", "Launching game...", "Injecting trainer...", "Ready"). Users need to know what is happening during the 5-30 second launch sequence.

12. **Now Playing Dashboard**: Minimal view during active gaming sessions showing game name, session timer, active trainer toggles, and process health indicator. Collapsible hotkey reference card.

13. **Profile Import/Export**: JSON-based profile format for sharing configurations. Community profile repository consideration for future releases.

14. **Cascading Configuration**: System-level defaults -> per-runner settings -> per-game profile overrides. Follow Lutris' proven three-tier model.

### Nice to Have

15. **Decky Loader Plugin**: A companion Decky plugin that exposes trainer toggles in the Steam Deck Quick Access Menu. Communicates with the main CrossHook process via IPC.

16. **On-Screen Keyboard Integration**: Custom on-screen keyboard for controller-mode text input (search, file paths). Integrate with Steam's virtual keyboard when available.

17. **System Controls Panel**: Following Lutris Gamepad UI, include audio volume, Bluetooth, display brightness, and power controls accessible from controller mode. Useful for Steam Deck users who don't want to leave CrossHook to adjust system settings.

18. **Trainer Overlay Widget**: A small, translucent floating widget that shows active trainer status during gameplay. Configurable position and transparency. Expandable for full toggle access.

19. **JSON Theme System**: User-overridable theme configuration (`theme.json`) for accent colors, font sizes, and spacing. Hot-reload without restart.

20. **Community Profile Sharing**: Browse and download game profiles with pre-configured trainer setups from other CrossHook users.

## Open Questions

1. **Toolkit Selection**: GTK4/libadwaita (native GNOME feel, strong Bottles/Lutris precedent, but KDE integration requires extra work) vs. Qt 6 (cross-desktop compatibility, ProtonUp-Qt precedent, but less GNOME-native) vs. Tauri (web-based UI with Rust backend, smallest footprint at ~30-50MB, but WebKitGTK rendering may lag behind) vs. Electron (maximum UI flexibility and fastest development velocity, Heroic precedent, but highest memory overhead at 200-300MB). This decision has major implications for Steam Deck performance and desktop integration.

2. **Overlay Implementation Strategy**: How will the in-game trainer overlay be rendered? Options include: (a) a separate transparent window positioned over the game, (b) integration with Decky Loader for native Gaming Mode overlay, (c) Steam Input virtual menu overlays for toggle mapping, or (d) MangoHud-style Vulkan/OpenGL overlay injection. Each has different complexity, compatibility, and performance tradeoffs.

3. **Trainer Format Abstraction**: Should CrossHook define its own trainer descriptor format, or directly support WeMod/FLiNG trainer binaries? A unified format would simplify the UI but adds a translation layer. Direct support preserves compatibility but complicates the architecture.

4. **Automatic vs. Manual Steam Integration**: Should CrossHook automatically add itself as a non-Steam game during first-run setup, or require the user to do it manually? Auto-addition is more seamless but modifies the user's Steam library without explicit consent.

5. **Offline Capability**: How much functionality should work without an internet connection? Game profiles and local trainers should work fully offline. But should CrossHook cache trainer compatibility databases for offline lookup? What about trainer updates?

6. **Accessibility Scope**: Given the niche audience (Linux gamers using trainers), what level of accessibility investment is appropriate? Full WCAG 2.1 AA compliance adds significant development effort. A pragmatic approach might focus on keyboard/controller navigability and visible focus indicators while deferring screen reader support.

7. **Single-Instance vs. Multi-Instance**: Should CrossHook allow multiple instances (e.g., managing two games simultaneously), or enforce single-instance like the current WinForms app? Multi-instance adds complexity but supports advanced use cases.

## Sources

### Official Design Guidelines

- [GNOME Human Interface Guidelines](https://developer.gnome.org/hig/)
- [GNOME HIG Design Principles](https://developer.gnome.org/hig/principles.html)
- [GNOME HIG Patterns](https://developer.gnome.org/hig/patterns.html)
- [KDE Kirigami HIG - Optimized Convergence](https://community.kde.org/KDE_Visual_Design_Group/KirigamiHIG/Principles/OptimizedConverence)
- [KDE Kirigami UI Framework](https://kde.org/products/kirigami/)
- [Microsoft 10-Foot Experience Guide](https://learn.microsoft.com/en-us/windows/win32/dxtecharts/introduction-to-the-10-foot-experience-for-windows-game-developers)
- [WCAG 2.4.7 Focus Visible](https://www.w3.org/WAI/WCAG22/Understanding/focus-visible.html)

### Competitor Applications

- [Lutris Desktop Client](https://github.com/lutris/lutris)
- [Lutris Gamepad UI](https://github.com/andrew-ld/lutris-gamepad-ui)
- [Heroic Games Launcher](https://heroicgameslauncher.com/)
- [Heroic Steam Deck Wiki](https://github.com/Heroic-Games-Launcher/HeroicGamesLauncher/wiki/Steam-Deck)
- [Bottles Documentation](https://docs.usebottles.com/)
- [Bottles First Run](https://docs.usebottles.com/getting-started/first-run)
- [Bottles Environments](https://docs.usebottles.com/getting-started/environments)
- [ProtonUp-Qt](https://github.com/DavidoTek/ProtonUp-Qt)
- [WeMod](https://www.wemod.com/)
- [DeckCheatz wemod-launcher](https://github.com/DeckCheatz/wemod-launcher)
- [FLiNG Trainer](https://flingtrainer.com/)
- [Playnite Fullscreen Mode](https://api.playnite.link/docs/manual/gettingStarted/playniteFullscreenMode.html)

### Steam Deck & Gaming Mode

- [Steam Deck Controller Guide](https://steamcommunity.com/sharedfiles/filedetails/?id=2804823261)
- [Steam Deck Desktop FAQ](https://help.steampowered.com/en/faqs/view/671A-4453-E8D2-323C)
- [Decky Loader](https://github.com/SteamDeckHomebrew/decky-loader)
- [Steam Big Picture Mode on Desktop](https://www.gamingonlinux.com/2022/10/steam-deck-ui-comes-to-desktop-in-beta-to-replace-big-picture-mode/)

### UX Research & Patterns

- [NN/g Error Message Guidelines](https://www.nngroup.com/articles/error-message-guidelines/)
- [Smashing Magazine Error Messages UX](https://www.smashingmagazine.com/2022/08/error-messages-ux-design/)
- [NN/g Progressive Disclosure](https://www.nngroup.com/articles/progressive-disclosure/)
- [Game UI Database](https://gameuidatabase.com/)
- [Overwolf In-Game Overlay Guidelines](https://dev.overwolf.com/ow-electron/guides/product-guidelines/app-screen-behavior/in-game-overlays/)
- [Android Game Dashboard Components](https://developer.android.com/games/gamedashboard/components)

### Technology Comparisons

- [Tauri vs. Electron Performance Comparison](https://www.gethopp.app/blog/tauri-vs-electron)
- [Tauri vs. Electron DoltHub](https://www.dolthub.com/blog/2025-11-13-electron-vs-tauri/)
- [libadwaita Documentation](https://gnome.pages.gitlab.gnome.org/libadwaita/)

### Linux System Integration

- [ptrace Documentation](https://man7.org/linux/man-pages/man2/ptrace.2.html)
- [Linux Capabilities](https://man7.org/linux/man-pages/man7/capabilities.7.html)
- [Steam on Linux - ArchWiki](https://wiki.archlinux.org/title/Steam)
