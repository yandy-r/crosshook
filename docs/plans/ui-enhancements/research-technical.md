# Technical Specifications: ui-enhancements

## Executive Summary

Replace the current horizontal tab bar + monolithic Main tab layout with a persistent vertical sidebar navigation that groups features into logical sections (Profiles, Launch, Export, Install, Community, Settings) and relocates the console to a collapsible bottom drawer. This eliminates the cluttered two-column Main tab, removes the ambiguous Profile/Install sub-tab split, and provides a clear one-section-at-a-time content area that scales from 1280x800 (Steam Deck) to standard desktop resolutions with zero new runtime dependencies.

## Architecture Design

### Current Component Tree

```
App.tsx
  rootRef -> useGamepadNav
  useState: settings, recentFiles, activeTab, profileEditorTab, ...
  useMemo: launchMethod, effectiveLaunchMethod, steamClientInstallPath, targetHomePath, launchRequest

  +-- <header> (eyebrow, title, copy, status chips)
  +-- <TabRow> role="tablist" (Main | Settings | Community)
  +-- Main Tab (activeTab === 'main')
  |   +-- <div.crosshook-layout> two-column grid (1.3fr / 0.9fr)
  |   |   +-- Left column
  |   |   |   +-- ProfileEditorView (state=profileState)
  |   |   |       +-- Sub-tabs: Profile | Install Game
  |   |   |       +-- Profile sub-tab -> ProfileFormSections + Save/Delete buttons
  |   |   |       +-- Install sub-tab -> InstallGamePanel
  |   |   |       +-- ProfileReviewModal (portal)
  |   |   |       +-- Delete confirmation overlay
  |   |   +-- Right column
  |   |       +-- LaunchPanel
  |   |       +-- LauncherExport (conditional)
  |   +-- ConsoleView (full width, below columns)
  +-- Settings Tab (activeTab === 'settings')
  |   +-- SettingsPanel
  |       +-- Startup section
  |       +-- Profiles directory section
  |       +-- ManageLaunchersSection
  |       +-- RecentFilesSection (x3: Games, Trainers, DLLs)
  +-- Community Tab (activeTab === 'community')
      +-- CommunityBrowser
      +-- CompatibilityViewer
```

### Proposed Component Tree

```
App.tsx (slimmed down: sidebar state, shared preferences, context providers)
  +-- AppProviders (context wrapper: ProfileContext, PreferencesContext)
  |
  +-- <div.crosshook-app-layout> (sidebar + content + drawer)
      +-- Sidebar.tsx (persistent vertical nav)
      |   +-- App branding / logo area
      |   +-- NavSection "Game"
      |   |   +-- NavItem "Profiles" -> route: profiles
      |   |   +-- NavItem "Launch" -> route: launch
      |   |   +-- NavItem "Export" -> route: export
      |   +-- NavSection "Setup"
      |   |   +-- NavItem "Install Game" -> route: install
      |   +-- NavSection "Community"
      |   |   +-- NavItem "Browse" -> route: community
      |   |   +-- NavItem "Compatibility" -> route: compatibility
      |   +-- NavSection (bottom-pinned)
      |       +-- NavItem "Settings" -> route: settings
      |       +-- Status indicators (controller mode, last profile)
      |
      +-- ContentArea.tsx (single scrollable panel, renders active route)
      |   +-- route: profiles -> ProfilesPage.tsx
      |   |   +-- ProfileFormSections (existing, cleaned up)
      |   |   +-- ProfileActions (Save/Delete bar, extracted)
      |   +-- route: launch -> LaunchPage.tsx
      |   |   +-- LaunchPanel (existing, inline styles migrated to CSS)
      |   +-- route: export -> ExportPage.tsx
      |   |   +-- LauncherExport (existing, inline styles migrated to CSS)
      |   +-- route: install -> InstallPage.tsx
      |   |   +-- InstallGamePanel (existing)
      |   |   +-- ProfileReviewModal (portal, attached here)
      |   +-- route: community -> CommunityPage.tsx
      |   |   +-- CommunityBrowser (existing)
      |   +-- route: compatibility -> CompatibilityPage.tsx
      |   |   +-- CompatibilityViewer (existing)
      |   +-- route: settings -> SettingsPage.tsx
      |       +-- SettingsPanel (existing)
      |
      +-- ConsoleDrawer.tsx (bottom drawer, collapsible, persistent across routes)
          +-- ConsoleView (existing logic, new container)
```

### New Components

- **`Sidebar`**: Persistent vertical navigation rail. Renders `NavSection` groups and `NavItem` links. Manages the active route indicator. Collapses to an icon rail at narrow widths. Holds the app branding and bottom-pinned settings/status area.
- **`NavSection`**: A labeled group within the sidebar (renders a heading and a list of `NavItem` children).
- **`NavItem`**: A single clickable navigation item with an icon placeholder, label, and active state.
- **`ContentArea`**: Wrapper that renders the correct page component based on the active route. Provides consistent page-level padding, scroll behavior, and heading structure.
- **`ConsoleDrawer`**: Bottom-anchored panel that wraps `ConsoleView` with a toggle handle. Persists across route changes so log streams are never lost.
- **`ProfilesPage`**: Thin page shell that composes `ProfileFormSections` and `ProfileActions`. Replaces the Profile sub-tab of `ProfileEditorView`.
- **`LaunchPage`**: Page shell for `LaunchPanel`, receiving profile/launch state from context.
- **`ExportPage`**: Page shell for `LauncherExport`.
- **`InstallPage`**: Page shell for `InstallGamePanel` and `ProfileReviewModal`.
- **`CommunityPage`**: Page shell wrapping `CommunityBrowser`.
- **`CompatibilityPage`**: Page shell wrapping `CompatibilityViewer`.
- **`SettingsPage`**: Page shell wrapping `SettingsPanel`.
- **`ProfileActions`**: Extracted from `ProfileEditorView`. Contains Save, Delete, and dirty-state indicator buttons.
- **`AppProviders`**: Composes React context providers (`ProfileContext`, `PreferencesContext`) so child pages can access shared state without prop drilling through the sidebar/content boundary.

### Integration Points

**State flow changes:**

The fundamental challenge of this restructure is that the current `App.tsx` lifts profile state, settings state, and derived launch state into a single component, then distributes them through props to children that live in the same visual hierarchy. With a sidebar layout, the profile editor and launch panel are no longer siblings under the same parent -- they are separate route pages.

1. **ProfileContext** -- Wraps `useProfile()` result. Consumed by `ProfilesPage`, `LaunchPage`, `ExportPage`, and the header area. This context already exists as the return value of `useProfile()` -- the change is lifting it from `App.tsx` state into a React context so any page can read it.

2. **PreferencesContext** -- Wraps `settings`, `recentFiles`, `steamClientInstallPath`, `targetHomePath`, `defaultSteamClientInstallPath`, and their mutators. Currently these are `App.tsx` state variables threaded as props to `SettingsPanel` and `LauncherExport`.

3. **Derived values** (`launchMethod`, `effectiveLaunchMethod`, `launchRequest`, heading text) stay as `useMemo` computations but move into the contexts or into the consuming page components.

4. **Install flow state** (`profileEditorTab`, profile review session) moves entirely into `InstallPage`, since it no longer needs to influence the launch panel's context flag or the header title. The `effectiveLaunchMethod` override that forces `proton_run` during install can be scoped to `InstallPage` alone.

5. **Console log stream** is already self-contained in `ConsoleView` via `listen('launch-log')`. Moving it to `ConsoleDrawer` requires zero state changes.

6. **Gamepad navigation** (`useGamepadNav`) stays at the `App` root level. The `rootRef` attaches to the new `crosshook-app-layout` wrapper. The existing `MODAL_FOCUS_ROOT_SELECTOR` strategy for modal focus trapping is unchanged.

## Layout Architecture

### Navigation Pattern

**Vertical sidebar** replaces the horizontal tab row.

```
+--sidebar--(200px)--+--------content-area--------+
| [CrossHook]        |                             |
|                    |   Page Title                |
| GAME               |   Page description          |
|  > Profiles        |                             |
|    Launch          |   [ page content ]          |
|    Export          |                             |
|                    |                             |
| SETUP              |                             |
|    Install Game    |                             |
|                    |                             |
| COMMUNITY          |                             |
|    Browse          |                             |
|    Compatibility   |                             |
|                    |                             |
|                    |                             |
| ---- (spacer) --- |                             |
|    Settings        |                             |
|  [controller: On] |                             |
+--------------------+-----------------------------+
|        ConsoleDrawer (collapsible)               |
+--------------------------------------------------+
```

**Key design decisions:**

- Sidebar width: `200px` fixed. Collapses to `56px` icon rail below `900px` viewport width.
- Active nav item uses the same accent gradient as current `.crosshook-tab--active`.
- Section headings are uppercase eyebrow-style labels (matches existing `.crosshook-heading-eyebrow`).
- Settings is pinned to the bottom of the sidebar, separated by a flex spacer.
- Status indicators (controller mode, last profile) move to the sidebar footer.

### Content Area

- Single scrollable column, `max-width: 960px`, centered with `margin: 0 auto`.
- Each page gets a consistent header: eyebrow + title + description (matches existing pattern).
- The current two-column layout within the Main tab is eliminated. The profile form, launch panel, and export panel each get their own page, reducing per-page complexity.
- Pages render one primary card/panel at a time instead of trying to show everything simultaneously.

### Responsive Behavior

| Breakpoint   | Sidebar                            | Content                                        | Console                           |
| ------------ | ---------------------------------- | ---------------------------------------------- | --------------------------------- |
| > 1360px     | 200px full sidebar                 | Max 960px centered, remaining space as margins | Bottom drawer, 280px default      |
| 900px-1360px | 200px full sidebar                 | Fluid fill                                     | Bottom drawer, 240px              |
| < 900px      | 56px icon rail (tooltips on hover) | Fluid fill minus rail                          | Bottom drawer, 200px or collapsed |

At the Steam Deck resolution of 1280x800:

- Sidebar at 200px leaves 1080px for the content area, which is more than adequate for a single-column form layout.
- Console drawer defaults to collapsed with a thin handle bar showing the last log line and an unread count badge.

The existing CSS media queries at `1360px` and `900px` stay but shift from toggling `.crosshook-layout` columns to toggling sidebar collapse.

### Console/Log Panel Placement

**Bottom drawer** approach (recommended over side panel or dedicated route):

- Persists across all routes -- never loses log output on navigation.
- Toggle bar is always visible at the bottom (24px collapsed height) with a "Runtime Console" label, line count, and expand/collapse button.
- Expanded: 280px height with resize handle for dragging.
- The existing `ConsoleView` component slots into the drawer body with minimal changes (remove its outer `<section>` border/shadow since the drawer provides the container).

### Modal Layering

- `ProfileReviewModal` continues using `createPortal` to `document.body`. The existing z-index of `1200` on `.crosshook-modal-portal` clears the sidebar (z-index `100`) and console drawer (z-index `50`).
- The delete confirmation overlay in `ProfileEditorView` remains as a fixed-position overlay at z-index `1000`.
- No changes needed to the modal portal strategy.

## Component Restructuring

### Files to Create

All paths relative to `src/crosshook-native/src/`.

- `context/ProfileContext.tsx`: React context wrapping `UseProfileResult`. Provider component calls `useProfile()` and exposes the result. Consumed by profile, launch, export, and install pages.
- `context/PreferencesContext.tsx`: React context wrapping settings, recent files, steam paths, and their mutators. Provider calls `invoke` on mount.
- `components/layout/Sidebar.tsx`: Vertical navigation component. Renders nav sections and items. Manages active route indicator. ~150 lines.
- `components/layout/NavSection.tsx`: Section heading + children wrapper for sidebar groups. ~30 lines.
- `components/layout/NavItem.tsx`: Single navigation item (button, label, active state). ~40 lines.
- `components/layout/ContentArea.tsx`: Route-to-page dispatcher. ~60 lines.
- `components/layout/ConsoleDrawer.tsx`: Bottom drawer container wrapping `ConsoleView`. Toggle state, resize handle, badge. ~120 lines.
- `components/pages/ProfilesPage.tsx`: Page shell composing `ProfileFormSections` + `ProfileActions`. ~80 lines.
- `components/pages/LaunchPage.tsx`: Page shell composing `LaunchPanel`. Reads profile from context. ~50 lines.
- `components/pages/ExportPage.tsx`: Page shell composing `LauncherExport`. Reads profile from context. ~50 lines.
- `components/pages/InstallPage.tsx`: Page shell composing `InstallGamePanel` + `ProfileReviewModal`. Owns install-specific state (review session, confirmation). ~200 lines (absorbs logic from `ProfileEditorView`).
- `components/pages/CommunityPage.tsx`: Page shell composing `CommunityBrowser`. ~30 lines.
- `components/pages/CompatibilityPage.tsx`: Page shell composing `CompatibilityViewer`. ~30 lines.
- `components/pages/SettingsPage.tsx`: Page shell composing `SettingsPanel`. ~30 lines.
- `components/ProfileActions.tsx`: Extracted Save/Delete/dirty-indicator bar. ~60 lines.
- `styles/sidebar.css`: Sidebar, nav section, nav item styles. ~150 lines.
- `styles/console-drawer.css`: Console drawer container, toggle bar, resize handle. ~80 lines.
- `styles/layout.css`: App layout grid (sidebar + content + drawer). ~50 lines.

### Files to Modify

- `App.tsx`: **Major refactor.** Remove all tab state, profile/settings state, derived computations, heading logic, and layout rendering. Replace with `<AppProviders>` wrapping a `<div.crosshook-app-layout>` containing `<Sidebar>`, `<ContentArea>`, and `<ConsoleDrawer>`. Shrinks from ~367 lines to ~60 lines.
- `components/ProfileEditor.tsx`: **Split.** `ProfileEditorView` loses the sub-tab row, the Install Game sub-tab rendering, and the profile review modal management. The profile-editing portion moves to `ProfilesPage`. The install/review orchestration moves to `InstallPage`. The standalone `ProfileEditor` export (used nowhere in the app currently) can be removed. The delete confirmation dialog stays with profile editing.
- `components/LaunchPanel.tsx`: **Migrate inline styles to CSS classes.** The `context === 'install'` branch is removed (Install Page handles its own status display). Reads profile/launch state from `ProfileContext` instead of props. Component shrinks by removing the install context block (~70 lines).
- `components/LauncherExport.tsx`: **Migrate inline styles to CSS classes.** The `context === 'install'` branch is removed. Reads profile from `ProfileContext`. All the `CSSProperties` objects at the top of the file (~100 lines of style constants) convert to CSS classes in `theme.css` or a new `launcher-export.css`.
- `components/ConsoleView.tsx`: **Migrate inline styles to CSS classes.** The outer `<section>` wrapper with its border/shadow is removed (provided by `ConsoleDrawer`). The component becomes a pure log body. Existing `.crosshook-console__*` classes in `theme.css` already exist but are unused -- the component uses inline styles instead. Migrate to use those classes.
- `components/SettingsPanel.tsx`: **Minor.** Remove `targetHomePath` and `steamClientInstallPath` props; read from `PreferencesContext` instead. The `layoutStyles` record of inline CSSProperties objects can be migrated to CSS classes.
- `components/CommunityBrowser.tsx`: **Minor.** The `panelStyles` record of inline CSSProperties objects can be migrated to CSS classes.
- `components/CompatibilityViewer.tsx`: **Minor.** Same inline style migration.
- `components/ProfileFormSections.tsx`: **Minor.** Remove `optionalSectionStyle` / `optionalSectionSummaryStyle` inline objects, use CSS classes. No structural changes.
- `components/InstallGamePanel.tsx`: **No structural changes.** Stays as-is, rendered by `InstallPage`.
- `components/ProfileReviewModal.tsx`: **No structural changes.** Portal-based, works anywhere.
- `styles/theme.css`: **Add** sidebar layout rules, content area rules, drawer rules. **Migrate** unused `.crosshook-console__*` classes from aspirational to actually consumed. Add new classes for `LaunchPanel`, `LauncherExport`, `ConsoleView` inline style replacements. Add sidebar responsive breakpoints.
- `styles/variables.css`: **Add** `--crosshook-sidebar-width: 200px`, `--crosshook-sidebar-width-collapsed: 56px`, `--crosshook-console-drawer-height: 280px`.
- `hooks/useProfile.ts`: **No changes.** Stays as-is, called from `ProfileContext` provider.
- `hooks/useGamepadNav.ts`: **No changes.** Stays at App root.

### Files to Delete

- None. All existing components are reused. `ProfileEditor.tsx` is restructured but not deleted (it still exports `ProfileFormSections`-adjacent utilities).

### Route State

Since this is a Tauri single-page app with no URL routing, "routes" are managed via a simple `useState<AppRoute>` in `App.tsx`:

```typescript
type AppRoute = 'profiles' | 'launch' | 'export' | 'install' | 'community' | 'compatibility' | 'settings';
```

No router library is needed. The `ContentArea` component switches on this value.

## CSS Architecture

### Migration Strategy

The codebase has a split personality: `theme.css` defines a comprehensive set of BEM-like classes, but many components (especially `LaunchPanel`, `LauncherExport`, `ConsoleView`, `CommunityBrowser`, `CompatibilityViewer`, `SettingsPanel`) define large inline `CSSProperties` objects at module scope and use `style={}` props. The migration plan:

1. **Phase 1 (with restructure):** New layout components (`Sidebar`, `ConsoleDrawer`, page shells) use CSS classes exclusively. No new inline styles.
2. **Phase 2 (parallel):** Migrate existing component inline styles to CSS classes. Priority order:
   - `ConsoleView` (already has `.crosshook-console__*` classes defined but unused)
   - `LaunchPanel` (heavy inline styles, ~200 lines of style objects)
   - `LauncherExport` (~100 lines of style constants)
   - `SettingsPanel` (`layoutStyles` record)
   - `CommunityBrowser` (`panelStyles` record)
   - `CompatibilityViewer` (inline style objects)
3. **Phase 3 (cleanup):** Remove orphaned style constants from component files after migration.

### New CSS Classes Needed

**Layout (in `styles/layout.css`):**

- `.crosshook-app-layout` -- CSS Grid: `grid-template-columns: var(--crosshook-sidebar-width) 1fr; grid-template-rows: 1fr auto;`
- `.crosshook-content-area` -- Scrollable content region with max-width and auto margins.
- `.crosshook-page-header` -- Consistent page title + description block.

**Sidebar (in `styles/sidebar.css`):**

- `.crosshook-sidebar` -- Fixed-height sidebar, flex column, background.
- `.crosshook-sidebar__brand` -- Top branding area.
- `.crosshook-sidebar__nav` -- Scrollable nav list.
- `.crosshook-sidebar__section` -- Section group with heading.
- `.crosshook-sidebar__section-label` -- Uppercase eyebrow label.
- `.crosshook-sidebar__item` -- Nav item button.
- `.crosshook-sidebar__item--active` -- Active state with accent gradient.
- `.crosshook-sidebar__footer` -- Bottom-pinned status area.
- `.crosshook-sidebar--collapsed` -- Icon-rail mode.

**Console drawer (in `styles/console-drawer.css`):**

- `.crosshook-console-drawer` -- Bottom drawer container.
- `.crosshook-console-drawer--collapsed` -- Collapsed state (24px height).
- `.crosshook-console-drawer__toggle` -- Toggle bar with label and buttons.
- `.crosshook-console-drawer__badge` -- Unread line count badge.
- `.crosshook-console-drawer__body` -- Scrollable log area.

**LaunchPanel classes (in `theme.css`):**

- `.crosshook-launch-card` -- Replaces inline `panelStyles.card`.
- `.crosshook-launch-eyebrow` -- Replaces inline uppercase method label.
- `.crosshook-launch-status` -- Status chip in launch panel.
- `.crosshook-launch-info-card` -- Inner info card.
- `.crosshook-launch-button` -- Primary launch action button.
- `.crosshook-launch-indicator` -- Dot + label status row.

**LauncherExport classes (in `theme.css`):**

- `.crosshook-export-card` -- Replaces inline `panelStyle`.
- `.crosshook-export-section` -- Replaces inline `sectionStyle`.
- `.crosshook-export-label` -- Replaces inline `labelStyle`.
- `.crosshook-export-input` -- Replaces inline `inputStyle`.
- `.crosshook-export-button` -- Replaces inline `buttonStyle`.
- `.crosshook-export-callout` -- Replaces inline `infoCalloutStyle`.
- `.crosshook-export-result` -- Export result card.
- `.crosshook-export-status` -- Launcher status indicator.

**Danger/delete button class:**

- `.crosshook-button--danger` -- Already partially used in `SettingsPanel` but not defined in `theme.css`. Needs formal definition.

## System Constraints

### Performance

- **Re-render scope reduction**: The current `App.tsx` holds all state, so any change re-renders the entire tree including all three tab panels (even though only one is visible). With contexts, only subscribers re-render. `ProfileContext` consumers (Profiles, Launch, Export pages) re-render on profile changes; other pages do not.
- **Console log accumulation**: `ConsoleView` appends to `lines` state on every `launch-log` event. With the drawer persisting across routes, this is unchanged. The existing approach of unbounded `lines` growth is a pre-existing concern unrelated to this restructure.
- **Route switching cost**: Switching routes unmounts/remounts page components. This is acceptable since none of them hold long-lived subscriptions except `ConsoleView` (which stays mounted in the drawer). `useMemo` values in page components recompute on mount but are cheap (string derivation).
- **Context splitting**: Splitting profile and preferences into separate contexts prevents settings changes from re-rendering launch/profile pages and vice versa.

### Gamepad Navigation

- **Vertical sidebar with controller**: The sidebar is a vertical list of focusable buttons. The existing `useGamepadNav` D-pad up/down maps naturally to previous/next nav item. D-pad right could be extended to move focus into the content area, but the current implementation already uses a linear focus list of all visible focusable elements, so sidebar items and content area elements will be in the same focus sequence automatically.
- **`getNavigationRoot`**: Currently uses `rootRef.current` with modal override. This continues to work with the new layout since the root ref moves to the app layout wrapper and the modal focus-root selector is unchanged.
- **Console drawer**: When expanded, the drawer's Clear/Collapse buttons join the focusable element list. When collapsed, only the toggle bar is focusable. This is handled automatically by `isFocusable` checking visibility.
- **Sidebar collapse at narrow viewports**: Collapsed sidebar items still need focus and gamepad accessibility. They render as icon buttons with `aria-label` attributes for screen readers and gamepad prompt overlays.

### Accessibility

- **Sidebar navigation pattern**: Uses `<nav aria-label="CrossHook navigation">` with `role="navigation"`. Each section uses a heading element. Nav items are `<button>` elements with `aria-current="page"` on the active item.
- **Vertical tab-like behavior**: Unlike horizontal tabs which use `role="tablist"` + `role="tab"` + `role="tabpanel"`, a sidebar navigation pattern uses `<nav>` + `<a>` or `<button>` elements. This matches WAI-ARIA navigation landmark pattern rather than tabs pattern, which is more appropriate for a sidebar that controls page-level content.
- **Console drawer**: Uses `role="region"` with `aria-label="Runtime console"`. The toggle button has `aria-expanded` state. When collapsed, the log area has `aria-hidden="true"`.
- **Focus management on route change**: When the active route changes, focus moves to the page's heading element (using a ref + `focus()` call), ensuring screen readers announce the new page. This mirrors how `ProfileReviewModal` focuses its heading on open.

### Bundle Size

- **Zero new dependencies.** The sidebar, drawer, and routing are all plain React components and CSS. No router library, no state management library, no animation library.
- **Potential reduction**: Eliminating inline style objects from components reduces the JavaScript bundle slightly (CSS classes are in separate files loaded once).
- **Code splitting**: Not needed at this scale. The entire frontend is ~150KB uncompressed JS. Page components are lightweight shells.

## Technical Decisions

### Decision 1: Routing approach

- **Options**: (A) React Router, (B) useState-based routing, (C) Tauri multi-window
- **Recommendation**: B -- useState-based routing
- **Rationale**: This is a single-window Tauri app with 7 routes. React Router adds ~14KB to the bundle and introduces URL/history management that is unnecessary for a desktop app with no browser navigation. A simple `useState<AppRoute>` with a switch in `ContentArea` is sufficient and keeps the dependency count at zero. If the app grows to 15+ routes in the future, revisit this decision.

### Decision 2: State management approach

- **Options**: (A) Prop drilling through sidebar/content boundary, (B) React Context, (C) External state library (Zustand, Jotai)
- **Recommendation**: B -- React Context
- **Rationale**: The app has exactly two pieces of shared state (profile and preferences) with well-defined update patterns already encapsulated in hooks. Context is the right tool for this scale. External state libraries add bundle size and conceptual overhead for marginal benefit. The existing `useProfile` hook already returns a complete state interface -- wrapping it in context is a minimal change.

### Decision 3: Console placement

- **Options**: (A) Bottom drawer (persistent), (B) Dedicated route page, (C) Side panel, (D) Floating window
- **Recommendation**: A -- Bottom drawer
- **Rationale**: The console must persist across route changes (log output should never be lost when switching views). A dedicated route page would lose visibility. A side panel competes with the sidebar for horizontal space, especially at 1280px. A floating window requires Tauri multi-window configuration changes. A bottom drawer is the standard pattern for persistent log output in development tools and terminal-based applications.

### Decision 4: Sidebar vs. improved horizontal tabs

- **Options**: (A) Vertical sidebar, (B) Horizontal tabs with sub-navigation, (C) Hybrid (horizontal top-level + vertical sub-nav)
- **Recommendation**: A -- Vertical sidebar
- **Rationale**: The current horizontal tabs have 3 items, but the Main tab contains 4 distinct features (profile editing, launch, export, install) that are crammed into a two-column layout with sub-tabs. A vertical sidebar with 7 items provides direct access to each feature without nesting. It also scales better as features are added (horizontal tabs overflow at 5-6 items on 1280px screens). The sidebar pattern is standard for multi-view desktop applications.

### Decision 5: CSS inline style migration timing

- **Options**: (A) Migrate all inline styles in the same PR, (B) Migrate layout first, components later, (C) Leave inline styles as-is
- **Recommendation**: B -- Migrate layout first, components later
- **Rationale**: Migrating all inline styles simultaneously would make the PR enormous and hard to review. The restructure PR should create all new layout components with CSS classes and migrate only the components that change structurally (ConsoleView moves to drawer, LaunchPanel loses install context branch). Remaining component style migrations (LauncherExport, CommunityBrowser, CompatibilityViewer, SettingsPanel) happen in follow-up PRs.

### Decision 6: ProfileEditorView decomposition

- **Options**: (A) Keep ProfileEditorView as-is and render it in ProfilesPage, (B) Split it into ProfilesPage + InstallPage, (C) Extract only ProfileActions
- **Recommendation**: B -- Split into ProfilesPage + InstallPage
- **Rationale**: `ProfileEditorView` is 568 lines because it manages two completely different workflows (profile editing and install review) via sub-tabs. The sub-tab pattern was necessary when both lived in the same Main tab column, but with a sidebar, each workflow gets its own route/page. Splitting eliminates the sub-tab state, the `onEditorTabChange` prop, the `effectiveLaunchMethod` override, and the conditional install context in LaunchPanel and LauncherExport.

## Implementation Order

Suggested ordering for incremental PRs:

1. **Context providers** -- Create `ProfileContext` and `PreferencesContext`. Wire `App.tsx` to use them. Verify existing behavior is unchanged.
2. **Layout shell** -- Create `Sidebar`, `ContentArea`, `ConsoleDrawer`, and the CSS. Replace the tab row and two-column layout. Each "route" initially renders the existing components unchanged.
3. **Page extraction** -- Create page shells (`ProfilesPage`, `LaunchPage`, `ExportPage`, `InstallPage`, etc.) that consume context and compose existing components. Remove the `ProfileEditorView` sub-tab split.
4. **Component cleanup** -- Remove install-context branches from `LaunchPanel` and `LauncherExport`. Slim down `App.tsx`.
5. **CSS migration** -- Migrate inline styles to CSS classes in follow-up PRs per component.

## Open Questions

- Should the sidebar include an active profile indicator (name + dirty state) below the branding area, or is the status chip in the sidebar footer sufficient?
- Should the console drawer height be user-resizable (drag handle) or fixed with only expand/collapse? A drag handle adds complexity to the gamepad navigation model.
- Should the "Profiles" page include a profile list/selector as a sidebar sub-list (clicking a profile name in the sidebar loads it), or keep the current dropdown selector within the page? A sidebar profile list would add a second level of navigation complexity.
- The `AutoPopulate` component is currently embedded deep inside `ProfileFormSections` for the `steam_applaunch` method. Should it be promoted to its own section within `ProfilesPage` or stay nested?
- The `ProfileEditor` standalone export (lines 570-588 of `ProfileEditor.tsx`) is not used anywhere in the app. Should it be deleted during this restructure?
