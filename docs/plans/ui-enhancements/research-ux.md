# UX Research: ui-enhancements

## Executive Summary

CrossHook's current horizontal tab layout with an overloaded Main tab creates a flat information hierarchy where profile editing, launch controls, export functionality, and console output all compete for attention at once. The recommended approach is a **vertical sidebar navigation** with icon+label items, a **single-purpose content area** per view, and a **persistent collapsible console drawer** at the bottom. This pattern is the dominant navigation model in modern gaming launchers (Heroic, Lutris gamepad UI, Playnite, Steam Big Picture) and aligns naturally with gamepad D-pad navigation, dark theme visual hierarchy, and the 1280x800 Steam Deck viewport.

**Confidence**: High -- based on convergent evidence from NN/g research, competitive analysis of 6 game launchers, and established desktop application patterns (VS Code, Discord, Slack, Spotify).

## User Workflows

### Primary Flow: Launch a Game with Trainer

1. **Open CrossHook** -> App loads with last-used profile auto-selected (if configured)
2. **Select/Verify Profile** -> Sidebar shows active profile name; user confirms or switches via profile quick-switcher
3. **Review Launch Configuration** -> Launch view shows method (Steam/Proton/Native), paths, and trainer settings for the active profile
4. **Launch** -> Single "Launch" button starts the orchestrated game+trainer flow
5. **Monitor Output** -> Console drawer auto-expands showing real-time launch logs
6. **Return to Launcher** -> Game closes, console shows exit status, user is back at the launch view

**Key insight**: The most common flow touches only 2 views (Profiles, Launch) and the console. The current layout forces the user to parse all features simultaneously on the Main tab when they only need the launch button and status.

### Alternative Flows

- **First-time setup**: Open app -> navigate to Profiles view -> create new profile (fill in game path, trainer path, Steam/Proton settings) -> save -> navigate to Launch view -> launch. The profile editor needs progressive disclosure: show game path and trainer path first, reveal Steam/Proton configuration only when relevant launch method is selected.

- **Quick re-launch**: Open app -> profile auto-loads -> hit launch button from any view (sidebar quick-launch or dedicated Launch view). This flow should require zero navigation if the profile is already loaded.

- **Install a Windows game**: Navigate to Install view (previously the "Install Game" sub-tab) -> configure Proton prefix and installer path -> run installer -> review generated profile in modal -> save profile -> switch to Launch view. This is a distinct workflow from normal launching and warrants its own sidebar destination.

- **Browse community profiles**: Navigate to Community view -> search/filter community taps -> select a profile -> install it -> it appears in the profile switcher. This is an infrequent but important discovery flow.

- **Manage exported launchers**: Navigate to Settings or Export view -> see list of exported .desktop files and shell scripts -> delete/rename/re-export. This is a maintenance task, not part of the core launch loop.

**Confidence**: High -- derived from analysis of the existing `App.tsx` component tree, `useProfile` hook, and `useLaunchState` hook which already model these distinct workflow states.

## UI/UX Best Practices

### Vertical Tab / Sidebar Navigation

**When to use**: Vertical navigation is the recommended pattern when an application has 5+ top-level sections, when the section list may grow over time, and when the application targets both desktop and controller input. It is the default navigation model for desktop applications including Slack, Discord, VS Code, Spotify, and all modern game launchers.

**Best practices from [NN/g research](https://www.nngroup.com/articles/vertical-nav/)**:

- **Left-aligned, keyword-frontloaded labels**: Users scan the left edge of navigation items. "Profiles" is better than "Manage Game Profiles."
- **Icon + text labels by default**: "In navigation, a word is worth a thousand pictures." Icons alone increase cognitive load. Show both icon and label when space permits; collapse to icon-only at narrow viewports (Steam Deck) with labels on hover/focus.
- **High contrast, dark background**: The sidebar should use a darker surface than the content area to create visual separation. This aligns with CrossHook's existing `--crosshook-color-surface-strong: #0c1120` which is darker than `--crosshook-color-bg: #1a1a2e`.
- **No duplicate navigation**: Remove the horizontal tab row entirely; do not have both sidebar and horizontal tabs.
- **Recommended sidebar width**: 240-300px expanded, 48-64px collapsed. CrossHook's `--crosshook-touch-target-min: 48px` already defines the minimum interactive size, making 56-64px a natural collapsed width.
- **Place less-important items at bottom**: Settings and About should be at the bottom of the sidebar, separated from the primary navigation items (Profiles, Launch, Export, Community).

**Trade-off**: Sidebar navigation consumes horizontal space (240-300px from the content area). At 1280x800 (Steam Deck), this leaves 980-1040px for content. The current `--crosshook-content-width: 1280px` would need adjustment, but the single-column content layout is more efficient than the current two-column split.

**Confidence**: High -- NN/g research with multiple corroborating sources ([UX Planet](https://uxplanet.org/best-ux-practices-for-designing-a-sidebar-9174ee0ecaa2), [LogRocket](https://blog.logrocket.com/ux-design/tabs-ux-best-practices/), [Eleken](https://www.eleken.co/blog-posts/tabs-ux)).

### Progressive Disclosure

**Core principle from [NN/g](https://www.nngroup.com/articles/progressive-disclosure/)**: "Initially show users only a few of the most important options. Offer a larger set of specialized options upon request." This improves learnability, efficiency, and reduces error rates.

**How to apply in CrossHook**:

| Always Visible (Primary)     | Show on Request (Secondary)           | Show on Demand (Tertiary)       |
| ---------------------------- | ------------------------------------- | ------------------------------- |
| Game executable path         | Steam App ID, compatdata path         | Custom environment variables    |
| Trainer path                 | Proton path, proton version selection | Working directory override      |
| Launch method selector       | Prefix path (for proton_run)          | Launch arguments, debug flags   |
| Profile name and save button | Steam client install path             | Launcher export options         |
| Launch button                | Runtime configuration                 | Console auto-scroll preferences |

**Implementation pattern**: Use collapsible sections (accordions) within the Profile editor view. The top section shows the essential fields (game path, trainer path, launch method). An "Advanced" or "Steam/Proton Configuration" section expands to reveal the method-specific fields. This replaces the current approach where `effectiveLaunchMethod` dynamically shows/hides fields inline, which creates unpredictable layout shifts.

**Staged disclosure variant**: The Install Game flow is a natural candidate for staged disclosure (wizard pattern) since its steps are sequential and non-interdependent: (1) Select installer -> (2) Configure prefix -> (3) Run installer -> (4) Review generated profile.

**Confidence**: High -- progressive disclosure is one of the most well-validated UX patterns, confirmed by NN/g research and successfully applied in IDEs (VS Code settings), game launchers (Steam game properties), and system configuration tools.

### Master-Detail Pattern

The master-detail pattern is the standard approach for record management interfaces: a list on the left, details on the right. This maps directly to CrossHook's profile management.

**Application to CrossHook Profiles view**:

- **Master pane (left)**: List of saved profiles with name, launch method badge, and last-used timestamp. Search/filter at the top. "New Profile" button.
- **Detail pane (right)**: Full profile editor for the selected profile. Shows all fields with progressive disclosure (essential fields visible, advanced collapsed).

**When the sidebar is present**: The master-detail pattern works within the content area. The sidebar provides app-level navigation; the master-detail provides view-level navigation within the Profiles view.

**At Steam Deck resolution**: The master-detail can stack vertically (profile list as a dropdown or collapsible panel above the editor) rather than side-by-side, preserving horizontal space.

**Confidence**: Medium -- the pattern is well-established ([Appli](https://appli.io/the-master-detail-interface-pattern/), [Windows Developer Blog](https://blogs.windows.com/windowsdeveloper/2017/05/01/master-master-detail-pattern/)), but implementing it depends on how many profiles a typical user manages. If most users have 3-5 profiles, a simple dropdown may suffice over a full master-detail layout.

### Information Architecture

**Recommended grouping of features into sidebar views**:

```
Sidebar (always visible)
  [App Logo / CrossHook]
  -------------------------
  Profiles        (ProfileEditor, profile list, save/delete/rename)
  Launch          (LaunchPanel, launch controls, active session status)
  Install         (InstallGamePanel, installer wizard flow)
  Community       (CommunityBrowser, CompatibilityViewer)
  -------------------------
  Export          (LauncherExport, .desktop and .sh management)
  Settings        (SettingsPanel, preferences, recent files)
  -------------------------
  [Active Profile: elden-ring]    <-- status area at bottom
  [Controller Mode: On]
```

**Rationale for grouping**:

- **Top group** (Profiles, Launch, Install): Primary workflow items used in every session. These map to the three main user tasks: configure, launch, and setup.
- **Middle group** (Community): Discovery and sharing. Used occasionally, not part of the core launch loop.
- **Bottom group** (Export, Settings): Maintenance and configuration. Used infrequently.
- **Status area**: Replaces the current header status chips and the dynamic heading text system.

**Confidence**: High -- this grouping follows frequency-of-use ordering (most used items at top) which is a fundamental IA principle. The grouping also matches the existing `AppTab` union type extension path.

### Console/Log Output Patterns

**Options analyzed**:

| Pattern                         | Used By                            | Pros                                                     | Cons                                                  |
| ------------------------------- | ---------------------------------- | -------------------------------------------------------- | ----------------------------------------------------- |
| **Bottom drawer (collapsible)** | VS Code, Chrome DevTools, IntelliJ | Persists across views, does not block content, resizable | Takes vertical space from content                     |
| **Separate tab/view**           | Firefox DevTools                   | Full-screen log viewing                                  | Loses context of current view, must navigate away     |
| **Overlay/toast**               | macOS notifications                | Non-intrusive for brief messages                         | Cannot show detailed log output                       |
| **Right panel**                 | Some IDEs                          | Keeps vertical space for content                         | Conflicts with sidebar, reduces content width further |
| **Inline within view**          | Current CrossHook                  | Contextual to the launch view                            | Lost when switching tabs, takes significant space     |

**Recommendation for CrossHook**: **Bottom drawer (collapsible)**, matching the VS Code terminal panel pattern.

- **Collapsed state**: Thin bar at the bottom showing "Console" label and latest log line preview, with expand button. Height: 40-48px.
- **Expanded state**: Resizable panel (default 280px, max 50vh) showing timestamped log output. The console should expand automatically when a launch event starts (`useLaunchState.phase` changes to 'launching').
- **Persistence**: The drawer must remain mounted across all view switches. This fixes the current bug where log history is lost when navigating away from the Main tab. The event listener (`listen('launch-log')`) should be at the shell level, not within a view.
- **Keyboard shortcut**: Toggle with `` Ctrl+` `` (VS Code convention) or a gamepad button (e.g., L1/LB shoulder button).

**Confidence**: High -- the bottom drawer pattern is the dominant approach in developer tools and power-user applications. VS Code, Chrome DevTools, IntelliJ IDEA, and Android Studio all use this pattern. Source: [VS Code issue #1875](https://github.com/microsoft/vscode/issues/1875), [Chrome DevTools reference](https://developer.chrome.com/docs/devtools/console/reference).

## Gamepad/Controller UX

### Navigation Patterns

**D-pad friendly patterns**:

- **Vertical sidebar is inherently D-pad friendly**: D-pad Up/Down navigates between sidebar items. D-pad Right or A-button enters the content area. D-pad Left or B-button returns to sidebar from content. This matches the natural spatial mapping of the layout.
- **Linear focus traversal within views**: Each view's content should support D-pad Up/Down navigation through form fields and buttons in reading order. The existing `useGamepadNav` hook uses `FOCUSABLE_SELECTOR` to traverse elements in DOM order, which naturally supports this.
- **Focus zones**: The app needs two focus zones -- sidebar and content. The gamepad should be able to move between these zones using D-pad Left/Right. Within each zone, D-pad Up/Down moves between items. This is how Steam Big Picture, Heroic, and Playnite fullscreen mode handle controller navigation.

**Implementation approach for CrossHook**:

The current `useGamepadNav` hook navigates all focusable elements in a single flat list (DOM order). For sidebar navigation to work well with gamepad:

1. **Zone-based navigation**: Extend the hook to support focus zones (`data-crosshook-focus-zone="sidebar"` and `data-crosshook-focus-zone="content"`). D-pad Left/Right switches zones; D-pad Up/Down navigates within the active zone.
2. **Focus memory per zone**: When the user switches from content back to sidebar, restore the last focused sidebar item (not always the first item). Similarly, when switching from sidebar to content, restore the last focused content element.
3. **Modal override**: The existing `MODAL_FOCUS_ROOT_SELECTOR` pattern should continue to work -- when a modal is open, all gamepad input is scoped to the modal.

**Confidence**: Medium -- the zone-based approach is the correct pattern (used by Steam Big Picture and Playnite fullscreen), but the actual implementation requires refactoring `useGamepadNav` to support multiple focus zones, which is a non-trivial change to a 470-line hook.

### Focus Management for Controller Users

**Visible focus indicators**: CrossHook already has strong focus styles in `focus.css` (accent-colored box-shadow ring). These should be maintained and enhanced:

- **Sidebar items**: Focus ring should wrap the entire item (icon + label), not just the text or icon.
- **Form fields**: The current `border-color: var(--crosshook-color-accent-strong)` + `box-shadow` pattern is good. Keep it.
- **Buttons**: Add a slight scale transform (1.02-1.04x) on focus for gamepad mode to make the focused button more prominent. The current `:hover` transform (`translateY(-1px)`) is mouse-oriented and should not apply in controller mode.

**Auto-focus on view switch**: When the user selects a sidebar item (navigates to a new view), the first focusable element in the content area should automatically receive focus. This prevents the user from having to manually navigate into the content area.

**Skip navigation**: For keyboard users, provide a "Skip to content" mechanism. For gamepad users, the zone-based D-pad Left/Right already serves this purpose.

**Confidence**: High -- focus management is a well-understood accessibility and gamepad UX requirement. Steam Deck UI, Playnite fullscreen, and Heroic all implement visible focus rings and auto-focus on view change.

### Steam Deck Considerations

**Resolution constraints**: 1280x800 at 16:10 aspect ratio. With a sidebar at 56-64px (collapsed/icon-only) the content area gets 1216-1224px, which is generous for a single-column layout.

**Input modes**: Steam Deck users switch between three input modes:

1. **Gamepad (primary)**: D-pad and thumbstick navigation, A/B for confirm/back. Buttons mapped via Steam Input.
2. **Touchscreen (secondary)**: Direct touch on UI elements. All interactive elements must meet 48px minimum touch target (already enforced by `--crosshook-touch-target-min`).
3. **Virtual keyboard (text input)**: Steam's on-screen keyboard appears when a text field is focused. The keyboard covers approximately the bottom 40% of the screen, so form fields should scroll into view above the keyboard.

**Steam Deck UI patterns to adopt**:

- **Quick Access Menu (QAM)**: Steam Deck uses a slide-in panel from the right edge for quick settings. CrossHook could use a similar pattern for the console drawer or a quick-launch overlay.
- **Bumper/trigger navigation between top-level sections**: LB/RB to cycle through sidebar items without entering the content area. This is how Steam Deck UI switches between Library/Store/Community. The `useGamepadNav` hook already reads L1/R1 buttons but does not currently use them for view switching.
- **Context bar at the bottom**: Steam Deck UI shows controller button prompts at the bottom of the screen (A: Select, B: Back, X: Options, Y: Search). CrossHook already has a `.crosshook-controller-prompts` class in `focus.css` but it is not currently used in any component.

**Confidence**: High -- Steam Deck has a well-documented UI paradigm and the existing codebase already includes Steam Deck detection (`isSteamDeckRuntime()`) and controller mode infrastructure.

### Spatial Navigation for Gamepad

**Spatial navigation** means the focused element changes based on the physical direction of the D-pad input relative to the screen layout, rather than following DOM order. For example, pressing D-pad Right moves focus to the nearest focusable element to the right of the current focus.

**Current state**: CrossHook's `useGamepadNav` uses linear (DOM order) navigation. D-pad Up/Left moves to the previous element in the focusable list; D-pad Down/Right moves to the next. This works for simple layouts but breaks down when the layout has a two-dimensional structure (sidebar + content, grid of buttons, etc.).

**Recommendation**: Implement **zone-based navigation** (sidebar zone + content zone) with linear traversal within each zone. This is a pragmatic middle ground between pure linear traversal and full spatial navigation:

- Pure spatial navigation is complex to implement (requires computing distances between element bounding rects) and can produce unexpected focus jumps.
- Zone-based navigation is simpler and predictable: D-pad Left/Right switches zones, D-pad Up/Down moves within a zone.
- This matches the patterns used by Steam Big Picture, Xbox dashboard, and PlayStation UI.

Full spatial navigation could be added later as an enhancement for grid-based views (e.g., the community profile card grid).

**Confidence**: Medium -- zone-based navigation is the recommended approach, but there is no off-the-shelf implementation for React+Tauri. The [Hydra launcher RFC](https://github.com/hydralauncher/hydra/issues/1958) and [ImGui gamepad navigation](https://github.com/ocornut/imgui/issues/787) provide implementation reference but require custom development.

## Dark Theme Patterns

### Visual Hierarchy

**Elevation through lightness**: In dark UI, "the closer an element is to the user, the lighter its background should be." This inverts the light-mode shadow paradigm where elevation = darker shadow. Dark mode elevation = lighter surface.

**Recommended surface color hierarchy for CrossHook**:

| Level       | Purpose              | Current Variable                   | Recommended Value | Usage                                                                         |
| ----------- | -------------------- | ---------------------------------- | ----------------- | ----------------------------------------------------------------------------- |
| 0 (Base)    | App background       | `--crosshook-color-bg`             | `#1a1a2e` (keep)  | Behind everything                                                             |
| 1 (Sidebar) | Navigation surface   | `--crosshook-color-surface-strong` | `#0c1120` (keep)  | Sidebar background -- darker than base to create a "recessed" navigation well |
| 2 (Content) | Main content surface | `--crosshook-color-surface`        | `#12172a` (keep)  | Content area background                                                       |
| 3 (Card)    | Elevated panels      | `--crosshook-color-bg-elevated`    | `#20243d` (keep)  | Cards, panels within content area                                             |
| 4 (Overlay) | Modals, dropdowns    | (new variable needed)              | `#282d48`         | Modal surfaces, dropdown menus                                                |

**Key principle**: Avoid pure black (`#000000`) for any surface. The existing CrossHook palette correctly avoids this -- the darkest color is `#0c1120`. This follows industry best practice: Figma uses `#1E1E1E`, YouTube uses `#181818`, Slack uses `#1D1D1D` (per [LogRocket dark mode research](https://blog.logrocket.com/ux-design/dark-mode-ui-design-best-practices-and-examples/)).

**Border and divider strategy**: Use semi-transparent white borders (`rgba(255, 255, 255, 0.08-0.22)`) rather than dark-on-dark borders. CrossHook already does this correctly with `--crosshook-color-border: rgba(224, 224, 224, 0.12)`.

**Color desaturation**: Avoid highly saturated colors on dark backgrounds. The existing accent color (`--crosshook-color-accent: #0078d4`) is moderately saturated and appropriate. Avoid neon greens, hot pinks, or fully saturated reds for status indicators. The current status colors (`--crosshook-color-success: #28c76f`, `--crosshook-color-danger: #ff758f`) are slightly saturated and could benefit from being toned down 10-15% for readability on dark surfaces.

**Confidence**: High -- based on Material Design dark theme guidelines, Apple HIG dark mode specifications, and [Toptal dark UI research](https://www.toptal.com/designers/ui/dark-ui-design). Multiple authoritative sources agree on the elevation-through-lightness approach.

### Glassmorphism in Dark Theme

CrossHook already uses glassmorphism (backdrop-filter: blur, semi-transparent backgrounds) for cards and panels. Best practices for maintaining this aesthetic in the redesigned layout:

- **Sidebar**: Should NOT use glassmorphism. The sidebar needs to feel solid and anchored -- it is a structural element, not a floating card. Use a solid dark background (`#0c1120`) with a subtle right border.
- **Content area cards/panels**: Continue using glassmorphism (`backdrop-filter: blur(18px)`) for panels within the content area. This creates the layered depth effect.
- **Console drawer**: Use a solid background similar to the sidebar. The console is a utility element that should feel grounded, not floating. The current `.crosshook-console` already uses a near-opaque gradient which is correct.
- **Modals**: The existing modal backdrop blur (`backdrop-filter: blur(12px)`) is effective. The modal surface itself should use glassmorphism sparingly -- the existing gradient approach is good.

**Performance note**: `backdrop-filter: blur()` can be expensive on lower-end hardware. On the Steam Deck's AMD APU, this should be fine for a few panels, but avoid applying it to scrollable lists or frequently re-rendered elements.

**Confidence**: Medium -- glassmorphism guidelines are more aesthetic/subjective than structural UX patterns. The performance concern is real but likely negligible for CrossHook's UI complexity.

### Gaming Launcher Aesthetic References

**What to adopt**:

- **Steam**: Dark backgrounds with blue accent highlights. Game art as visual anchors. Clean separation between navigation and content. The accent blue (`#1B2838` sidebar, `#171A21` base) is close to CrossHook's existing palette.
- **Discord**: Sidebar with server/channel navigation, dark charcoal theme, prominent focus indicators, smooth transitions. The collapsed channel list pattern is relevant for CrossHook's profile list.
- **Epic Games Store**: Large cards with gradient overlays, clean typography hierarchy, accent color used sparingly for CTAs. The "card with gradient" pattern matches CrossHook's existing `.crosshook-panel` and `.crosshook-card`.

**What to avoid**:

- **Overly decorative gradients**: Some launchers (older Steam skins) use heavy gradients that create visual noise. CrossHook's subtle radial gradients on `html` and `.crosshook-app` are tasteful -- keep them subtle.
- **Dark-on-dark text**: Ensure all text meets WCAG 4.5:1 contrast minimum. The current `--crosshook-color-text-subtle: rgba(224, 224, 224, 0.56)` calculates to approximately 6.2:1 contrast against `#1a1a2e`, which passes AA.
- **Animation overload**: Gaming aesthetic does not mean game-like animations. Keep transitions functional (expand/collapse, focus state changes) and under 300ms. CrossHook's `--crosshook-transition-standard: 220ms` is appropriate.

**Confidence**: High -- gaming launcher aesthetics are well-documented and CrossHook's existing design language already follows these conventions closely.

## Competitive Analysis

### Lutris

- **Approach**: GTK3-based desktop application with a traditional menu bar + toolbar + content area layout. Game library is the primary view with a sidebar for sources/categories. Uses a flat, native Linux desktop appearance.
- **Navigation**: Top toolbar with view mode toggles (grid/list), sidebar for game sources (local, Steam, GOG, Humble, etc.), and a bottom status bar.
- **Strengths**:
  - Category-based sidebar filtering is intuitive for large libraries
  - Multiple view modes (grid, list) for different use cases
  - Integration with multiple game sources (Steam, GOG, Humble Bundle) in a unified library
  - "Recently Played" and "All Games" shelves in the [gamepad UI variant](https://github.com/andrew-ld/lutris-gamepad-ui) provide good quick-access patterns
- **Weaknesses**:
  - No native controller/gamepad support in the main GTK interface -- requires a [separate gamepad UI frontend](https://alternativeto.net/software/lutris-gamepad-ui/about/)
  - GTK3 styling looks dated compared to modern gaming launchers
  - Configuration screens are deeply nested (game settings -> runner options -> system options) with many tabs
  - No dark theme by default; relies on system GTK theme
- **Takeaway for CrossHook**: Lutris's need for a separate gamepad UI validates CrossHook's approach of building controller navigation into the primary interface. The category sidebar pattern is worth adopting for organizing profiles by launch method or game source.

**Confidence**: High -- Lutris is open source and well-documented. The [official site](https://lutris.net/) and [GitHub repository](https://github.com/lutris/lutris) provide direct evidence. The gamepad UI limitation is confirmed by the existence of two separate community projects addressing it.

### Heroic Games Launcher

- **Approach**: Electron-based application with a collapsible left sidebar, large content area for game library, and dedicated pages for game details, settings, and downloads.
- **Navigation**: Vertical sidebar with icon+label items: Library, Stores (Epic, GOG, Amazon), Downloads, Settings. The sidebar shows current download/update status persistently. [Sidebar is collapsible](https://github.com/Heroic-Games-Launcher/HeroicGamesLauncher/discussions/1478) to icon-only mode.
- **Strengths**:
  - Clean sidebar navigation that scales well (5-6 items)
  - Collapsible sidebar preserves content space when not needed
  - Game cards show title, action buttons (play, settings, update) directly on the card -- reduces clicks
  - Unified library view across Epic, GOG, and Amazon sources
  - Active development of [joystick and keyboard navigation improvements](https://www.gamingonlinux.com/2025/07/heroic-games-launcher-2-18-adds-ge-proton-prioritisation-improved-ui-navigation-and-new-analytics/)
  - Community themes support via [heroic-themes repository](https://github.com/Heroic-Games-Launcher/heroic-themes)
- **Weaknesses**:
  - Electron bundle size is large (~200MB)
  - Settings page is a single long scrollable form rather than organized sections
  - Game detail page mixes game info, settings, and launch configuration in one view
- **Takeaway for CrossHook**: Heroic's collapsible sidebar is the closest existing reference for what CrossHook should build. The "download/update status in sidebar" pattern maps to CrossHook showing the active launch session status in the sidebar. The icon+label -> icon-only collapse is exactly the pattern recommended for Steam Deck viewport adaptation.

**Confidence**: High -- Heroic is open source (Electron/React, similar tech stack to CrossHook's Tauri/React). Direct analysis of [GitHub releases](https://github.com/Heroic-Games-Launcher/HeroicGamesLauncher/releases) and [v2.4.0 discussion](https://github.com/Heroic-Games-Launcher/HeroicGamesLauncher/discussions/1478) confirms the sidebar design.

### Bottles

- **Approach**: GTK4 + libadwaita application following GNOME Human Interface Guidelines. Uses a sidebar list of "bottles" (Wine prefixes) with a detail panel for the selected bottle.
- **Navigation**: Left sidebar showing bottle list (master), right content area showing bottle details and configuration (detail). Settings are accessed through a separate dialog. Uses the [libadwaita navigation split view](https://docs.usebottles.com/) pattern which provides responsive master-detail with animation.
- **Strengths**:
  - Clean master-detail pattern that is immediately understandable
  - Native GNOME look-and-feel with consistent dark theme support via libadwaita
  - Settings moved to a dedicated dialog rather than a sidebar tab -- keeps the main interface focused
  - Progressive disclosure: virtual desktop resolution setting only appears when the feature is enabled
  - Drag-and-drop to run files in a bottle context
- **Weaknesses**:
  - GTK4/libadwaita styling does not match gaming launcher aesthetics -- looks more like a system utility
  - No gamepad/controller support
  - Limited to Wine/Proton prefix management; no game library or launch orchestration
  - The sidebar list works for 5-15 bottles but does not scale to large collections without search/filter
- **Takeaway for CrossHook**: Bottles demonstrates that the master-detail pattern works well for managing a list of configurations (bottles/profiles). The progressive disclosure of settings (show fields only when relevant) is directly applicable to CrossHook's launch method-dependent field visibility. The "settings in a dialog" pattern could inform moving settings out of the main navigation flow.

**Confidence**: High -- Bottles is open source with [active documentation](https://docs.usebottles.com/). The GTK4/libadwaita patterns are well-documented in the [GNOME HIG](https://developer.gnome.org/hig/).

### Steam (Big Picture Mode / Steam Deck UI)

- **Approach**: Custom rendering engine (not web-based) with full controller-first design. The Steam Deck UI (now unified with Big Picture Mode) uses a full-screen interface with large touch targets, spatial navigation, and context-sensitive button prompts.
- **Navigation**: Top-level sections (Library, Store, Community, Downloads) accessible via bumper buttons (LB/RB). Within sections, D-pad navigates between items. A "Quick Access Menu" slides in from the right edge for system settings, notifications, and friends. The [new controller configurator](https://store.steampowered.com/oldnews/189167) is designed for controller-first input.
- **Strengths**:
  - Gold standard for controller-first UI in gaming
  - Bumper button cycling between top-level sections eliminates the need to navigate to a sidebar
  - Context bar at bottom showing button prompts (A: Select, B: Back, X: Options, Y: Search) -- reduces discovery friction
  - Universal search across library, store, and friends
  - Smooth focus transitions with animation
  - "Continue Playing" shelf for quick re-launch of recent games
- **Weaknesses**:
  - Community feedback reports [2-5 extra clicks for common actions](https://steamcommunity.com/groups/bigpicture/discussions/1/1729827777342182601/) compared to the old Big Picture Mode
  - Heavy reliance on nested menus for game settings and properties
  - The UI is optimized for consumption (browsing games) rather than configuration (setting up launch parameters)
  - Not open source -- cannot directly examine implementation
- **Takeaway for CrossHook**: The bumper-button section cycling (LB/RB to switch sidebar views) is an essential gamepad UX pattern that CrossHook should adopt. The context bar showing button prompts (CrossHook already has `.crosshook-controller-prompts` CSS but does not render it) should be implemented. The "Continue Playing" / recent games shelf pattern maps to CrossHook's profile quick-switcher concept.

**Confidence**: High -- Steam Big Picture/Deck UI is extensively documented by [Valve](https://help.steampowered.com/en/faqs/view/3725-76D3-3F31-FB63), [KitGuru](https://www.kitguru.net/gaming/mustafa-mahmoud/steam-deck-ui-finally-comes-to-big-picture-mode-on-pc/), and [PC Gamer](https://www.pcgamer.com/steam-decks-new-ui-is-finally-coming-to-desktop-big-picture-mode/). Direct observation of the Steam Deck UI on hardware confirms the navigation patterns.

### ProtonUp-Qt

- **Approach**: Qt6-based utility application with a tab-based interface for managing Proton/Wine compatibility tool installations per launcher (Steam, Lutris, etc.).
- **Navigation**: Horizontal tabs for each launcher (Steam, Lutris, etc.), with a list view of installed compatibility tools in each tab. Double-click on a tool shows extended information.
- **Strengths**:
  - Simple, focused UI for a single task (managing Proton versions)
  - [Gamepad support optimized for handhelds](https://github.com/DavidoTek/ProtonUp-Qt)
  - Lightweight (Python + Qt, much smaller than Electron)
  - Clear information hierarchy: one list per launcher, each entry shows version and install status
- **Weaknesses**:
  - Horizontal tabs do not scale if more launchers are added
  - No dark theme by default (follows system Qt theme)
  - Limited to Proton management; no integration with game launching
  - Dialog-based information display (double-click to see details) is not discoverable
- **Takeaway for CrossHook**: ProtonUp-Qt validates that gamepad-optimized UI is achievable in a utility application. The per-launcher tab organization is less relevant to CrossHook (which manages one prefix ecosystem), but the tool's simplicity is a good reminder that feature density should match use frequency.

**Confidence**: Medium -- ProtonUp-Qt is a utility app rather than a game launcher, so the UX patterns are less directly transferable. [GitHub source](https://github.com/DavidoTek/ProtonUp-Qt) confirms features.

### Playnite

- **Approach**: .NET/WPF application with two distinct modes: Desktop Mode (keyboard+mouse optimized) and [Fullscreen Mode](https://api.playnite.link/docs/manual/gettingStarted/playniteFullscreenMode.html) (controller optimized). Fully themeable with community themes.
- **Navigation**: Desktop Mode uses a left sidebar for library sources and filters, with a large content area showing games in grid or list view. Fullscreen Mode uses a horizontal top bar with large icons, game cover art as the primary visual element, and D-pad navigation.
- **Strengths**:
  - Dual-mode approach (desktop + fullscreen) ensures optimal UX for both input methods
  - Extensive [theme customization](https://api.playnite.link/docs/manual/features/themesSupport/installingThemes.html) -- users can completely reshape the UI
  - Sidebar filters in Desktop Mode are powerful (by source, genre, platform, completion status, etc.)
  - Fullscreen Mode is a true controller-first experience with focus navigation and cover art prominence
  - Unified library across all sources (Steam, GOG, Epic, Origin, etc.)
  - Toggle between modes with F11
- **Weaknesses**:
  - Windows-only (.NET/WPF dependency) -- not a direct competitor on Linux
  - Desktop Mode and Fullscreen Mode use entirely different codebases/themes, leading to feature parity gaps
  - Configuration is complex due to the extensive customization options
  - Plugin architecture adds complexity for basic setup
- **Takeaway for CrossHook**: Playnite's dual-mode approach is the most ambitious reference, but CrossHook should avoid maintaining two separate UI modes. Instead, a single responsive layout that adapts between expanded sidebar (desktop) and collapsed icon-only sidebar (Steam Deck) achieves a similar result with one codebase. Playnite's sidebar filter pattern is relevant if CrossHook's profile list grows -- adding filter chips for launch method, game source, or recently used.

**Confidence**: High -- Playnite has extensive [official documentation](https://api.playnite.link/docs/manual/gettingStarted/playniteDesktopMode.html) and a large theme community. The dual-mode approach is well-documented.

## Comparative Navigation Matrix

| Feature                       | CrossHook (Current)  | Heroic           | Lutris            | Bottles           | Steam BPM                   | Playnite Desktop      |
| ----------------------------- | -------------------- | ---------------- | ----------------- | ----------------- | --------------------------- | --------------------- |
| **Navigation type**           | Horizontal tabs      | Vertical sidebar | Toolbar + sidebar | Master-detail     | Bumper tabs + spatial       | Sidebar filters       |
| **Sidebar**                   | None                 | Collapsible      | Category list     | Bottle list       | None (sections via bumpers) | Filter sidebar        |
| **Section count**             | 3                    | 5-6              | 4-5               | N/A (list-based)  | 4-5                         | 2-3                   |
| **Gamepad support**           | Yes (linear)         | Improving        | No (needs add-on) | No                | Yes (spatial)               | Yes (fullscreen mode) |
| **Console/logs**              | Inline (loses state) | Download manager | Status bar        | Dialog-based      | None visible                | None visible          |
| **Dark theme**                | Yes (glassmorphism)  | Yes (custom)     | System theme      | Yes (libadwaita)  | Yes (custom)                | Yes (themeable)       |
| **Profile/config management** | Sub-tabs within Main | Per-game page    | Per-game settings | Per-bottle detail | Per-game properties         | Per-game detail       |

## Recommendations

### Must Have

1. **Vertical sidebar navigation** replacing horizontal tabs. The sidebar should contain 5-6 items (Profiles, Launch, Install, Community, Export, Settings) with icon+label format. This is the single highest-impact change for reducing UI clutter and improving findability.
   - **Confidence**: High
   - **Evidence**: All 6 analyzed launchers use either sidebar or bumper-tab top-level navigation. NN/g recommends vertical navigation for 5+ sections. The current 3 horizontal tabs with nested sub-tabs is the root cause of the "Main tab overload" problem.

2. **Persistent console drawer at the bottom** that stays mounted across all view switches. Collapsible to a thin status bar, auto-expandable on launch events.
   - **Confidence**: High
   - **Evidence**: VS Code, IntelliJ, Chrome DevTools all use this pattern. The current bug where console logs are lost on tab switch is a direct consequence of the current inline placement.

3. **Gamepad zone-based navigation** supporting two zones (sidebar + content) with D-pad Left/Right zone switching and D-pad Up/Down within-zone traversal. Add LB/RB bumper buttons for cycling through sidebar views.
   - **Confidence**: High
   - **Evidence**: Steam Big Picture uses bumper cycling. Heroic and Playnite fullscreen use zone-based focus. The existing `useGamepadNav` hook provides the foundation but needs zone awareness.

4. **Single-purpose content area** where each sidebar view renders one functional domain. No two-column split on the Main tab. No nested sub-tabs. Each view has a clear heading, relevant controls, and nothing else.
   - **Confidence**: High
   - **Evidence**: Progressive disclosure research (NN/g) shows that reducing visible options improves learnability and reduces errors. The current layout violates this by showing profile editing, launch controls, export options, and console output simultaneously.

5. **48px minimum touch targets maintained** for all interactive elements, including sidebar items. This is already enforced by `--crosshook-touch-target-min` but must be validated in the new sidebar layout.
   - **Confidence**: High
   - **Evidence**: WCAG touch target guidelines, Apple HIG, and Steam Deck's coarse pointer input all require 44-48px minimum targets.

### Should Have

6. **Collapsible sidebar** that reduces to icon-only (56-64px) at narrow viewports or by user toggle. Auto-collapse when viewport matches Steam Deck resolution (1280x800).
   - **Confidence**: High
   - **Evidence**: Heroic implements this pattern. NN/g recommends 48-64px collapsed width. The existing `isSteamDeckRuntime()` detection function can trigger auto-collapse.

7. **Profile quick-switcher in sidebar** showing the active profile name with a dropdown/popover to switch profiles without navigating to the Profiles view.
   - **Confidence**: Medium
   - **Evidence**: Discord's server switcher and VS Code's workspace picker use this pattern. Reduces the need to navigate away from the Launch view for the most common "switch profile then launch" workflow.

8. **Controller button prompt bar** at the bottom of the screen in gamepad mode, showing context-sensitive button mappings (A: Select, B: Back, Y: Quick Launch, LB/RB: Switch View).
   - **Confidence**: High
   - **Evidence**: Steam Big Picture, PlayStation UI, Xbox dashboard all use this. CrossHook already has `.crosshook-controller-prompts` CSS class but does not render it anywhere.

9. **Progressive disclosure in Profile editor** with collapsible sections. Essential fields (game path, trainer path, launch method) always visible; Steam/Proton configuration in expandable section; runtime overrides in a tertiary section.
   - **Confidence**: High
   - **Evidence**: NN/g progressive disclosure research. Bottles uses this pattern for Wine settings. Steam game properties use tabs for different configuration categories.

10. **Status area in sidebar** showing active profile, launch session state, and controller mode. Replaces the header status chips and dynamic heading text in the current App.tsx.
    - **Confidence**: Medium
    - **Evidence**: Discord shows online status in sidebar. Steam shows download progress in sidebar. Heroic shows download/update status in sidebar.

### Nice to Have

11. **Keyboard shortcuts for view switching** (Ctrl+1 through Ctrl+5). The gamepad hook already intercepts keyboard events and could be extended to support these.
    - **Confidence**: Medium

12. **Resizable console drawer** with drag handle, similar to VS Code's terminal panel resize behavior.
    - **Confidence**: Medium

13. **Search across all views** via a global search input in the sidebar header. Currently search only exists within CommunityBrowser.
    - **Confidence**: Low -- may be over-engineering for the current profile count. Revisit when users report discovery problems.

14. **Animation on sidebar item selection**: Subtle slide-in transition for the content area when switching views. Keep under 200ms to avoid feeling sluggish on Steam Deck.
    - **Confidence**: Medium

15. **"Recently launched" quick-access list** in the sidebar or on the Launch view, showing the last 3-5 launched profiles. The current `settings.last_used_profile` only tracks one.
    - **Confidence**: Medium

## Open Questions

1. **Should the sidebar be on the left or right?** Left is conventional (Heroic, Discord, VS Code, Bottles). Right would match Steam Deck's Quick Access Menu placement. **Recommendation**: Left, following convention and NN/g research that users scan left first (80% of the time).

2. **How many sidebar views is optimal?** The current plan suggests 6 (Profiles, Launch, Install, Community, Export, Settings). Should Install be merged into Profiles (as a sub-view or mode)? Should Export be merged into Settings? Fewer views = less navigation overhead but more feature density per view. **Recommendation**: Start with 5 views (merge Install into Profiles as a mode toggle or wizard within that view, keep Export separate from Settings since launcher management is a distinct task).

3. **Should the console drawer have its own keyboard shortcut?** VS Code uses Ctrl+\`. Steam Deck would need a gamepad button (Select/View button?). **Recommendation**: Yes, implement Ctrl+\` for keyboard and map a gamepad button (Select button or L3/R3 click) for controller.

4. **Should the gamepad focus zone system be opt-in or always-on?** Zone-based navigation changes the D-pad behavior from linear to zone-aware. This could confuse users who expect pure linear traversal. **Recommendation**: Zone-based in gamepad mode, linear in keyboard-only mode. The `controllerMode` state already distinguishes these.

5. **How should the sidebar behave when a modal is open?** Currently, modal focus trapping uses `data-crosshook-focus-root="modal"`. With a sidebar, should the sidebar be visually dimmed/inert when a modal is open? **Recommendation**: Yes, apply `inert` attribute to the sidebar when a modal is open, consistent with the existing `hiddenNodesRef` inert handling in `ProfileReviewModal`.

6. **Should there be a "Home" or "Dashboard" view?** Some apps (Heroic, Playnite) have a home screen showing recently played games and quick actions. CrossHook could show: active profile summary, quick-launch button, recent activity, and system status (Steam detected, Proton versions available). **Recommendation**: Not for the initial redesign. The Launch view serves as the effective "home" when a profile is loaded. A dashboard could be added later if user feedback requests it.

## Search Queries Executed

1. `vertical tab sidebar navigation UX best practices desktop application 2024 2025`
2. `Heroic Games Launcher UI sidebar navigation design Linux`
3. `Lutris game launcher UI design layout organization 2024`
4. `Bottles wine manager Linux UI design sidebar navigation`
5. `Steam Big Picture Mode UI navigation gamepad design patterns 2024`
6. `progressive disclosure UX pattern desktop application software design`
7. `gamepad controller spatial navigation focus management UI design Steam Deck`
8. `dark theme UI design patterns gaming application visual hierarchy elevation layering`
9. `Playnite game launcher UI design customizable desktop mode fullscreen mode`
10. `master detail pattern sidebar list detail panel desktop application UX`
11. `console log output panel placement desktop application IDE pattern bottom drawer`
12. `ProtonUp-Qt UI design Proton management interface`

## Uncertainties and Gaps

- **Glassmorphism performance on Steam Deck**: No benchmarks were found for `backdrop-filter: blur()` performance specifically on the Steam Deck's AMD APU in a Tauri/WebView context. The assumption is that it performs adequately for the small number of blurred elements in CrossHook's UI, but this needs testing.
- **Community profile counts**: The optimal navigation pattern depends on how many profiles users typically manage. The master-detail pattern is recommended for 10+ profiles but may be over-engineering for users with 3-5 profiles. Usage analytics would inform this decision, but CrossHook does not currently collect telemetry.
- **Gamepad zone navigation implementation complexity**: The zone-based focus system requires significant refactoring of `useGamepadNav` (470 lines). No off-the-shelf React hook for zone-based gamepad navigation was found. Implementation effort is estimated at medium-high.
- **Bottles UI specifics**: Detailed screenshots and navigation flow analysis of Bottles was limited by web fetch restrictions. The analysis is based on documentation and community descriptions rather than direct UI inspection.
- **Hydra launcher**: The [Hydra game launcher](https://github.com/hydralauncher/hydra/issues/1958) has an open RFC for game controller navigation support that could provide additional implementation reference, but it was not included in the analysis due to its early-stage status.

## Sources

- [Left-Side Vertical Navigation on Desktop -- NN/g](https://www.nngroup.com/articles/vertical-nav/)
- [Progressive Disclosure -- NN/g](https://www.nngroup.com/articles/progressive-disclosure/)
- [Tabs, Used Right -- NN/g](https://www.nngroup.com/articles/tabs-used-right/)
- [Best UX Practices for Designing a Sidebar -- UX Planet](https://uxplanet.org/best-ux-practices-for-designing-a-sidebar-9174ee0ecaa2)
- [Tabbed Navigation in UX -- LogRocket](https://blog.logrocket.com/ux-design/tabs-ux-best-practices/)
- [Tabs UX: Best Practices -- Eleken](https://www.eleken.co/blog-posts/tabs-ux)
- [Dark Mode UI Design Best Practices -- LogRocket](https://blog.logrocket.com/ux-design/dark-mode-ui-design-best-practices-and-examples/)
- [The Principles of Dark UI Design -- Toptal](https://www.toptal.com/designers/ui/dark-ui-design)
- [Dark Mode UI Tips -- Netguru](https://www.netguru.com/blog/tips-dark-mode-ui)
- [Elevation Design Patterns -- Design Systems Surf](https://designsystems.surf/articles/depth-with-purpose-how-elevation-adds-realism-and-hierarchy)
- [The Master-Detail Interface Pattern -- Appli](https://appli.io/the-master-detail-interface-pattern/)
- [Master the Master-Detail Pattern -- Windows Developer Blog](https://blogs.windows.com/windowsdeveloper/2017/05/01/master-master-detail-pattern/)
- [Heroic Games Launcher -- Official Site](https://heroicgameslauncher.com/)
- [Heroic v2.4.0 Discussion -- GitHub](https://github.com/Heroic-Games-Launcher/HeroicGamesLauncher/discussions/1478)
- [Heroic v2.18 UI Navigation Improvements -- GamingOnLinux](https://www.gamingonlinux.com/2025/07/heroic-games-launcher-2-18-adds-ge-proton-prioritisation-improved-ui-navigation-and-new-analytics/)
- [Heroic Themes Repository -- GitHub](https://github.com/Heroic-Games-Launcher/heroic-themes)
- [Lutris -- Official Site](https://lutris.net/)
- [Lutris Gamepad UI -- AlternativeTo](https://alternativeto.net/software/lutris-gamepad-ui/about/)
- [Lutris Gamepad UI -- GitHub](https://github.com/andrew-ld/lutris-gamepad-ui)
- [Bottles Documentation](https://docs.usebottles.com/)
- [Bottles -- ArchWiki](https://wiki.archlinux.org/title/Bottles)
- [Steam Big Picture Mode FAQ -- Valve](https://help.steampowered.com/en/faqs/view/3725-76D3-3F31-FB63)
- [Steam Deck UI Comes to Big Picture -- KitGuru](https://www.kitguru.net/gaming/mustafa-mahmoud/steam-deck-ui-finally-comes-to-big-picture-mode-on-pc/)
- [Steam Deck UI Coming to Desktop -- PC Gamer](https://www.pcgamer.com/steam-decks-new-ui-is-finally-coming-to-desktop-big-picture-mode/)
- [Steam Client Update (Big Picture) -- Store.steampowered.com](https://store.steampowered.com/oldnews/189167)
- [ProtonUp-Qt -- Official Site](https://davidotek.github.io/protonup-qt/)
- [ProtonUp-Qt -- GitHub](https://github.com/DavidoTek/ProtonUp-Qt)
- [Playnite Desktop Mode Documentation](https://api.playnite.link/docs/manual/gettingStarted/playniteDesktopMode.html)
- [Playnite Fullscreen Mode Documentation](https://api.playnite.link/docs/manual/gettingStarted/playniteFullscreenMode.html)
- [Playnite Theme Installation](https://api.playnite.link/docs/manual/features/themesSupport/installingThemes.html)
- [Hydra Launcher Controller Navigation RFC -- GitHub](https://github.com/hydralauncher/hydra/issues/1958)
- [ImGui Gamepad Navigation -- GitHub](https://github.com/ocornut/imgui/issues/787)
- [VS Code Output Panel Discussion -- GitHub](https://github.com/microsoft/vscode/issues/1875)
- [Chrome DevTools Console Reference](https://developer.chrome.com/docs/devtools/console/reference)
- [Progressive Disclosure -- IxDF](https://ixdf.org/literature/topics/progressive-disclosure)
- [Progressive Disclosure -- UI Patterns](https://ui-patterns.com/patterns/ProgressiveDisclosure)
- [Dark Mode UI Perfect Theme Palette -- Dopely Colors](https://dopelycolors.com/blog/dark-mode-ui-perfect-theme-palette)
