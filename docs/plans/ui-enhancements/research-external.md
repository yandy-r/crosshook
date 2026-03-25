# External API Research: ui-enhancements

## Executive Summary

The recommended approach for CrossHook's UI restructuring is **Radix Primitives for headless accessible components** (tabs, navigation, dialogs) combined with **react-resizable-panels for the split-pane layout**, both styled with the existing custom CSS variable system. Radix provides WAI-ARIA-compliant vertical tabs with orientation-aware keyboard navigation out of the box, integrates cleanly with vanilla CSS via `className` props and `data-state`/`data-orientation` attributes, and adds minimal bundle overhead (~3-5 kB gzipped per primitive). The existing `useGamepadNav` hook is already well-architected for focus-based navigation and will work with Radix components without modification since Radix renders standard focusable DOM elements. No client-side router is needed -- Radix Tabs with vertical orientation replaces horizontal tab navigation and organizes views without URL routing overhead.

---

## Primary Libraries

### Radix Primitives (`@radix-ui/react-*`)

- **Documentation**: <https://www.radix-ui.com/primitives>
- **Key Features**:
  - Unstyled, accessible React primitives (tabs, dialogs, navigation menus, tooltips, dropdowns)
  - WAI-ARIA compliant with full keyboard navigation built in
  - `orientation="vertical"` prop on Tabs for sidebar-style navigation
  - Data attributes (`data-state`, `data-orientation`, `data-disabled`) for CSS-only state styling
  - Modular per-component packages -- install only what you use
  - `asChild` pattern for composing with existing elements without wrapper divs
  - Loop focus support on Tab lists (`loop` prop, default `true`)
  - Controlled and uncontrolled modes
  - `activationMode`: `"automatic"` (activate on focus) or `"manual"` (activate on Enter/Space)
- **Bundle Size**: ~3-5 kB gzipped per primitive component (Radix Primitives use individual packages; the full library is not bundled together). The npm unpacked size for `@radix-ui/react-tabs` is ~52 kB, but tree-shaking and gzip compression bring the wire cost down to a few kilobytes per component.
- **Gamepad Compatibility**: Radix renders standard focusable DOM elements (`button`, `div[tabindex]`). The existing `useGamepadNav` hook navigates focusable elements via `querySelectorAll(FOCUSABLE_SELECTOR)` and will automatically discover Radix triggers and content areas. No changes to the gamepad hook are required.
- **Dark Theme Support**: Ships with zero styles. All visual styling comes from your CSS. The `data-state` and `data-orientation` attributes integrate directly with the existing `--crosshook-*` CSS custom properties.

**Confidence**: High -- Radix is the most widely adopted headless library for React with excellent documentation, 70%+ adoption growth in 2025, and proven compatibility with custom CSS systems.

Sources:

- [Radix Primitives Docs](https://www.radix-ui.com/primitives)
- [Radix Tabs Component](https://www.radix-ui.com/primitives/docs/components/tabs)
- [Radix Styling Guide](https://www.radix-ui.com/primitives/docs/guides/styling)
- [Styling Radix UI with CSS - Samuel Kraft](https://samuelkraft.com/blog/radix-ui-styling-with-css)

---

### react-resizable-panels

- **Documentation**: <https://react-resizable-panels.vercel.app/>
- **Repository**: <https://github.com/bvaughn/react-resizable-panels>
- **Key Features**:
  - Three components: `PanelGroup`, `Panel`, `PanelResizeHandle`
  - Horizontal and vertical orientation
  - Collapsible panels with `collapsible` prop
  - Min/max size constraints (percentages, pixels, rem, vh, vw)
  - Layout persistence via `onLayoutChange` callback
  - Keyboard resizing (arrow keys, following WAI-ARIA Window Splitter pattern)
  - Touch and mouse drag support
  - `resizeTargetMinimumSize` prop for ensuring adequate hit targets (critical for Steam Deck touch input)
  - `panelRef` exposes `collapse()`, `expand()`, `getSize()`, `isCollapsed()`, `resize()` APIs
  - shadcn/ui's Resizable component is built on this library (v4)
- **Bundle Size**: ~10-15 kB gzipped (zero external dependencies beyond React). The npm unpacked size is ~115 kB, but the gzipped production bundle is significantly smaller.
- **Gamepad Compatibility**: Resize handles are focusable and support arrow key resizing natively. The `useGamepadNav` hook's D-pad/stick navigation will focus resize handles as standard focusable elements. Users can resize panels with D-pad arrow keys while a handle is focused.
- **Dark Theme Support**: Ships with no styles. All visual customization is via CSS classes on the three components. The resize handle can be styled to match the `--crosshook-color-border` and `--crosshook-color-accent` tokens.

**Confidence**: High -- Created by Brian Vaughn (former React core team), 1700+ dependents on npm, actively maintained (v4.7.6 as of March 2026), battle-tested via shadcn/ui adoption.

Sources:

- [react-resizable-panels npm](https://www.npmjs.com/package/react-resizable-panels)
- [GitHub Repository](https://github.com/bvaughn/react-resizable-panels)
- [shadcn/ui Resizable](https://ui.shadcn.com/docs/components/radix/resizable)

---

## Libraries and SDKs

### Recommended Libraries

#### 1. `@radix-ui/react-tabs` -- Vertical Tab Navigation

- **Why recommended**: Native `orientation="vertical"` support, WAI-ARIA Tabs pattern compliance, data attributes for CSS styling, zero styling opinions, automatic keyboard navigation direction switching.
- **Install**: `npm install @radix-ui/react-tabs`
- **Docs**: <https://www.radix-ui.com/primitives/docs/components/tabs>

#### 2. `react-resizable-panels` -- Split Pane Layout

- **Why recommended**: Mature API, collapsible sidebar support, keyboard-accessible resize handles, layout persistence, pixel and percentage sizing, touch-friendly hit targets.
- **Install**: `npm install react-resizable-panels`
- **Docs**: <https://react-resizable-panels.vercel.app/>

#### 3. `@radix-ui/react-tooltip` -- Optional Enhancement

- **Why recommended**: Useful for icon-only sidebar items when collapsed. Accessible tooltips with hover/focus triggers and proper ARIA labeling.
- **Install**: `npm install @radix-ui/react-tooltip`
- **Docs**: <https://www.radix-ui.com/primitives/docs/components/tooltip>

### Alternative Options Evaluated

#### React Aria (Adobe) -- `react-aria-components`

- **Pros**: Best-in-class accessibility (43 components), hooks-based API gives maximum control, excellent documentation.
- **Cons**: Larger per-component bundle size vs Radix, steeper learning curve, hook-based API is more verbose than Radix's compound component pattern. The render prop abstraction adds boilerplate that doesn't benefit this use case.
- **Verdict**: Overkill for CrossHook's needs. Radix provides equivalent accessibility with less code.

**Confidence**: Medium -- React Aria is excellent but the integration overhead isn't justified for the 2-3 primitives CrossHook needs.

Sources:

- [React Aria Tabs](https://react-aria.adobe.com/Tabs)
- [React Aria Bundle Size Discussion](https://github.com/adobe/react-spectrum/discussions/5636)

#### Ark UI -- `@ark-ui/react`

- **Pros**: 45+ components, cross-framework (React/Vue/Solid), state-machine based (Zag.js), lazy mounting for tabs, `unmountOnExit` prop.
- **Cons**: Younger ecosystem than Radix, `data-part`/`data-scope` attribute styling convention differs from Radix's more intuitive `data-state`, smaller community, dependency on Zag.js state machines adds bundle weight.
- **Verdict**: Viable alternative but Radix has a more mature ecosystem and simpler CSS integration model for this project.

**Confidence**: Medium -- Ark UI is solid but the Zag.js dependency and smaller ecosystem make Radix the safer choice.

Sources:

- [Ark UI](https://ark-ui.com/)
- [Ark UI Tabs](https://ark-ui.com/react/docs/components/tabs)
- [Ark UI GitHub](https://github.com/chakra-ui/ark)

#### Headless UI (Tailwind Labs) -- `@headlessui/react`

- **Pros**: Clean API, good accessibility.
- **Cons**: No native vertical tab orientation support (open GitHub discussion requesting it), tightly coupled to Tailwind CSS conventions, smaller component selection (6 components vs 28+ for Radix).
- **Verdict**: Not recommended. Missing vertical tabs is a dealbreaker, and the Tailwind coupling conflicts with CrossHook's vanilla CSS approach.

**Confidence**: High (that it should be excluded) -- The missing vertical tabs feature and Tailwind coupling are confirmed via GitHub issues.

Sources:

- [Headless UI Tabs](https://headlessui.com/react/tabs)
- [Vertical Tabs Discussion #2149](https://github.com/tailwindlabs/headlessui/discussions/2149)

#### shadcn/ui

- **Pros**: Built on Radix, beautiful defaults, copy-paste component ownership model.
- **Cons**: Requires Tailwind CSS, copy-paste model adds maintenance burden vs installing Radix directly, opinionated styling that would conflict with existing `--crosshook-*` design tokens.
- **Verdict**: Not recommended. CrossHook should use Radix Primitives directly and apply its own CSS rather than adopting shadcn/ui's Tailwind-based component files.

**Confidence**: High -- shadcn/ui is excellent for Tailwind projects but adds unnecessary complexity for a project with an established custom CSS system.

Sources:

- [shadcn/ui](https://ui.shadcn.com/)
- [Radix vs shadcn/ui Comparison](https://workos.com/blog/what-is-the-difference-between-radix-and-shadcn-ui)

#### @tanstack/react-router

- **Pros**: Type-safe file-based routing, works in Tauri v2.
- **Cons**: Unnecessary for CrossHook's use case. The app has 3-5 views, not dozens of routes. Radix Tabs with controlled state achieves the same view switching without URL routing, hash routing, or route-based code splitting overhead.
- **Verdict**: Not recommended. Adds complexity without benefit. Controlled Radix Tabs provide the same UX with simpler state management.

**Confidence**: High -- Client-side routing is designed for multi-page SPAs, not a desktop app with a handful of views.

Sources:

- [TanStack Router](https://tanstack.com/router/latest)
- [Tauri + TanStack Router Discussion](https://github.com/tauri-apps/tauri/discussions/7899)

---

## Integration Patterns

### Recommended Approach: Headless Primitives + Existing CSS System

The integration strategy preserves CrossHook's existing design system while replacing hand-rolled tab and layout logic with accessible primitives.

**Architecture**:

```
[Vertical Radix Tabs Sidebar] | [react-resizable-panels Content Area]
                               |
  - Profile                    |  [Left Panel: Editor]  |  [Right Panel: Actions]
  - Install Game               |                        |
  - Settings                   |  (ProfileEditor)       |  (LaunchPanel + Export)
  - Community                  |                        |
                               |  [Bottom: ConsoleView]
```

**Key principles**:

1. Radix Tabs replaces both the horizontal `crosshook-tab-row` and the `crosshook-subtab-row`
2. `react-resizable-panels` replaces the fixed `crosshook-layout` grid for the main content area
3. All styling uses existing `--crosshook-*` CSS variables via `className` props on Radix components
4. Radix `data-state` and `data-orientation` attributes replace manual `crosshook-tab--active` class toggling
5. The `useGamepadNav` hook continues to work unchanged since all Radix/panel elements are standard focusable DOM elements

**Migration path**: Incremental. Each Radix primitive can replace one custom component at a time without breaking the rest of the UI.

### Vertical Tab Implementation

#### Pattern 1: Radix Tabs with Vertical Orientation (Recommended)

```tsx
import * as Tabs from '@radix-ui/react-tabs';

type AppView = 'profile' | 'install' | 'settings' | 'community';

interface SidebarNavProps {
  activeView: AppView;
  onViewChange: (view: AppView) => void;
}

function SidebarNav({ activeView, onViewChange }: SidebarNavProps) {
  return (
    <Tabs.Root
      orientation="vertical"
      value={activeView}
      onValueChange={(value) => onViewChange(value as AppView)}
      activationMode="automatic"
    >
      <Tabs.List className="crosshook-sidebar" aria-label="CrossHook sections">
        <Tabs.Trigger className="crosshook-sidebar__item" value="profile">
          <ProfileIcon />
          <span className="crosshook-sidebar__label">Profile</span>
        </Tabs.Trigger>
        <Tabs.Trigger className="crosshook-sidebar__item" value="install">
          <InstallIcon />
          <span className="crosshook-sidebar__label">Install Game</span>
        </Tabs.Trigger>
        <Tabs.Trigger className="crosshook-sidebar__item" value="settings">
          <SettingsIcon />
          <span className="crosshook-sidebar__label">Settings</span>
        </Tabs.Trigger>
        <Tabs.Trigger className="crosshook-sidebar__item" value="community">
          <CommunityIcon />
          <span className="crosshook-sidebar__label">Community</span>
        </Tabs.Trigger>
      </Tabs.List>

      <Tabs.Content className="crosshook-view" value="profile">
        {/* ProfileEditor + LaunchPanel + LauncherExport */}
      </Tabs.Content>
      <Tabs.Content className="crosshook-view" value="install">
        {/* Install Game flow */}
      </Tabs.Content>
      <Tabs.Content className="crosshook-view" value="settings">
        <SettingsPanel />
      </Tabs.Content>
      <Tabs.Content className="crosshook-view" value="community">
        <CommunityBrowser />
      </Tabs.Content>
    </Tabs.Root>
  );
}
```

#### CSS for Vertical Tabs (using existing design tokens)

```css
/* Sidebar container -- vertical tab list */
.crosshook-sidebar {
  display: flex;
  flex-direction: column;
  gap: 4px;
  padding: 12px 8px;
  min-width: 200px;
  background: var(--crosshook-color-surface);
  border-right: 1px solid var(--crosshook-color-border);
  border-radius: var(--crosshook-radius-lg) 0 0 var(--crosshook-radius-lg);
}

/* Individual sidebar item */
.crosshook-sidebar__item {
  display: flex;
  align-items: center;
  gap: 12px;
  min-height: var(--crosshook-touch-target-min);
  padding: 0 16px;
  border-radius: var(--crosshook-radius-sm);
  border: 1px solid transparent;
  background: transparent;
  color: var(--crosshook-color-text-muted);
  cursor: pointer;
  font-weight: 600;
  font-size: 0.95rem;
  transition:
    background var(--crosshook-transition-standard) ease,
    color var(--crosshook-transition-standard) ease,
    border-color var(--crosshook-transition-standard) ease;
}

/* Active state via Radix data attribute */
.crosshook-sidebar__item[data-state='active'] {
  background: var(--crosshook-color-accent-soft);
  color: var(--crosshook-color-text);
  border-color: rgba(0, 120, 212, 0.3);
}

/* Hover state */
.crosshook-sidebar__item:hover:not([data-state='active']) {
  background: rgba(255, 255, 255, 0.04);
  color: var(--crosshook-color-text);
}

/* Focus state for gamepad/keyboard navigation */
.crosshook-sidebar__item:focus-visible {
  outline: none;
  border-color: var(--crosshook-color-accent-strong);
  box-shadow: 0 0 0 3px var(--crosshook-color-accent-soft);
}

/* Disabled state */
.crosshook-sidebar__item[data-disabled] {
  opacity: 0.5;
  cursor: not-allowed;
}

/* Orientation data attribute (set by Radix) */
.crosshook-sidebar[data-orientation='vertical'] {
  /* Already flex-direction: column above, but this selector
     can be used for responsive overrides */
}

/* View content area */
.crosshook-view {
  flex: 1;
  min-width: 0;
  min-height: 0;
}
```

### Resizable Panel Layout

```tsx
import { Panel, PanelGroup, PanelResizeHandle } from 'react-resizable-panels';

function MainContentLayout() {
  return (
    <PanelGroup direction="horizontal" id="main-layout">
      {/* Left panel: Profile editor or current view content */}
      <Panel id="editor-panel" defaultSize={55} minSize={35} maxSize={75}>
        <ProfileEditorView />
      </Panel>

      <PanelResizeHandle className="crosshook-resize-handle" />

      {/* Right panel: Launch controls + export */}
      <Panel id="actions-panel" defaultSize={45} minSize={25} collapsible collapsedSize={0}>
        <div className="crosshook-actions-stack">
          <LaunchPanel />
          <LauncherExport />
        </div>
      </Panel>
    </PanelGroup>
  );
}
```

#### CSS for Resize Handle

```css
.crosshook-resize-handle {
  width: 8px;
  background: transparent;
  border: none;
  cursor: col-resize;
  position: relative;
  transition: background var(--crosshook-transition-fast) ease;
}

.crosshook-resize-handle::after {
  content: '';
  position: absolute;
  top: 50%;
  left: 50%;
  transform: translate(-50%, -50%);
  width: 4px;
  height: 32px;
  border-radius: 2px;
  background: var(--crosshook-color-border);
  transition: background var(--crosshook-transition-fast) ease;
}

.crosshook-resize-handle:hover::after,
.crosshook-resize-handle:focus-visible::after {
  background: var(--crosshook-color-accent);
}

.crosshook-resize-handle:focus-visible {
  outline: none;
  box-shadow: 0 0 0 2px var(--crosshook-color-accent-soft);
}
```

### Full Layout Composition

```tsx
import * as Tabs from '@radix-ui/react-tabs';
import { Panel, PanelGroup, PanelResizeHandle } from 'react-resizable-panels';

function AppShell() {
  const [activeView, setActiveView] = useState<AppView>('profile');
  const gamepadNav = useGamepadNav({ onBack: handleGamepadBack });

  return (
    <main ref={gamepadNav.rootRef} className="crosshook-app crosshook-focus-scope">
      <Tabs.Root
        orientation="vertical"
        value={activeView}
        onValueChange={(v) => setActiveView(v as AppView)}
        className="crosshook-app-layout"
      >
        {/* Vertical sidebar */}
        <Tabs.List className="crosshook-sidebar" aria-label="CrossHook navigation">
          <div className="crosshook-sidebar__brand">
            <span className="crosshook-heading-eyebrow">CrossHook</span>
          </div>
          <Tabs.Trigger className="crosshook-sidebar__item" value="profile">
            Profile
          </Tabs.Trigger>
          <Tabs.Trigger className="crosshook-sidebar__item" value="install">
            Install Game
          </Tabs.Trigger>
          <Tabs.Trigger className="crosshook-sidebar__item" value="settings">
            Settings
          </Tabs.Trigger>
          <Tabs.Trigger className="crosshook-sidebar__item" value="community">
            Community
          </Tabs.Trigger>
          <div className="crosshook-sidebar__footer">
            <span className="crosshook-status-chip">Controller: {gamepadNav.controllerMode ? 'On' : 'Off'}</span>
          </div>
        </Tabs.List>

        {/* Main content area */}
        <div className="crosshook-main">
          <Tabs.Content className="crosshook-view" value="profile">
            <PanelGroup direction="horizontal" id="profile-layout">
              <Panel id="editor" defaultSize={58} minSize={35}>
                <ProfileEditorView />
              </Panel>
              <PanelResizeHandle className="crosshook-resize-handle" />
              <Panel id="actions" defaultSize={42} minSize={28}>
                <LaunchPanel />
                <LauncherExport />
              </Panel>
            </PanelGroup>
            <ConsoleView />
          </Tabs.Content>

          <Tabs.Content className="crosshook-view" value="install">
            {/* Install game flow */}
          </Tabs.Content>

          <Tabs.Content className="crosshook-view" value="settings">
            <SettingsPanel />
          </Tabs.Content>

          <Tabs.Content className="crosshook-view" value="community">
            <CommunityBrowser />
            <CompatibilityViewer />
          </Tabs.Content>
        </div>
      </Tabs.Root>
    </main>
  );
}
```

#### Top-Level Layout CSS

```css
.crosshook-app-layout {
  display: flex;
  min-height: 100vh;
  gap: 0;
}

.crosshook-sidebar {
  display: flex;
  flex-direction: column;
  width: 220px;
  flex-shrink: 0;
  padding: 16px 8px;
  background: var(--crosshook-color-surface);
  border-right: 1px solid var(--crosshook-color-border);
}

.crosshook-sidebar__brand {
  padding: 8px 16px 20px;
}

.crosshook-sidebar__footer {
  margin-top: auto;
  padding: 12px 8px 4px;
}

.crosshook-main {
  flex: 1;
  min-width: 0;
  padding: var(--crosshook-page-padding);
  overflow-y: auto;
}

.crosshook-view {
  display: grid;
  gap: var(--crosshook-grid-gap);
  height: 100%;
}

/* Hide inactive tab content */
.crosshook-view[data-state='inactive'] {
  display: none;
}
```

---

## Constraints and Gotchas

### Tauri v2 WebView Constraints (Linux)

- **WebKitGTK**: Tauri v2 uses WebKit2GTK-4.1 on Linux. CSS Grid, Flexbox, CSS custom properties, and standard CSS transitions all work reliably.
- **`backdrop-filter` limitations**: WebKitGTK has inconsistent support for `backdrop-filter: blur()`. The existing theme already uses this (`.crosshook-panel`), so it works on the target platform but may not render on all Linux distros. Consider fallback `background` values with higher opacity.
- **No native widgets**: All UI is rendered in the WebView. This is already the case and is not a constraint change.
- **Impact**: Low. The proposed libraries use standard CSS and DOM APIs that are well-supported in WebKitGTK.

**Confidence**: High

Sources:

- [Tauri v2 Webview Versions](https://v2.tauri.app/reference/webview-versions/)
- [WebKitGTK CSS Discussion](https://github.com/tauri-apps/tauri/discussions/8808)

### Steam Deck Resolution (1280x800)

- **Touch target sizing**: The existing `--crosshook-touch-target-min: 48px` meets Apple HIG and WCAG touch target guidelines. All Radix triggers and panel resize handles must maintain this minimum.
- **Sidebar width budget**: At 1280px, a 200-220px sidebar leaves 1060-1080px for content. The existing two-column layout within that space is achievable with `react-resizable-panels` constraining the narrower panel to a minimum of 280px.
- **Vertical space**: 800px is tight. The ConsoleView's `min-height: 280px` combined with header content may require the console to be collapsible or placed in its own panel.
- **Gamepad D-pad**: Radix Tabs with `orientation="vertical"` automatically uses Up/Down arrow keys for navigation (instead of Left/Right for horizontal). The `useGamepadNav` hook maps D-pad Up/Down to `focusPrevious`/`focusNext`, which aligns with Radix's vertical keyboard behavior.
- **Impact**: Medium. Vertical space is the primary constraint. Collapsible panels and a compact sidebar design are essential.

**Confidence**: High

Sources:

- [Steam Deck Display Specifications](https://store.steampowered.com/steamdeck)
- [Heroic Launcher UI Discussion](https://github.com/Heroic-Games-Launcher/HeroicGamesLauncher/discussions/1287)

### Bundle Size for AppImage

- **Total addition**: ~15-25 kB gzipped for Radix Tabs + react-resizable-panels + Radix Tooltip.
- **Context**: The existing React + ReactDOM is ~45 kB gzipped. The proposed additions represent a ~35-55% increase in library JS, which is negligible for an AppImage distribution (typically 50-100+ MB).
- **Tree-shaking**: Both Radix and react-resizable-panels support tree-shaking. Only imported components are bundled.
- **Impact**: Negligible. AppImage size is dominated by the Tauri binary and WebKitGTK runtime, not JS bundle size.

**Confidence**: High

### Gamepad Navigation Compatibility

- **No changes needed to `useGamepadNav`**: The hook discovers focusable elements via `querySelectorAll` on standard HTML selectors (`button`, `input`, `[tabindex]`, etc.). Radix renders these standard elements. The hook's `focusNext`/`focusPrevious` traversal and `confirm` (click) behavior work with Radix triggers.
- **Modal focus trapping**: The existing `MODAL_FOCUS_ROOT_SELECTOR` (`[data-crosshook-focus-root="modal"]`) pattern continues to work. Radix Dialog can be configured with this attribute.
- **Potential issue**: Radix Tabs' built-in keyboard handler also listens for arrow keys. When in controller mode, both Radix's internal handler and `useGamepadNav` may respond to the same key event. The gamepad hook uses `capture: true` on its keydown listener, which fires before Radix's bubble-phase handler. This could cause double-navigation. **Mitigation**: The gamepad hook already calls `event.preventDefault()` on arrow keys, which will prevent Radix's handler from also firing. This is the correct behavior.
- **Impact**: Low. Existing architecture handles the interaction correctly, but should be verified during implementation.

**Confidence**: Medium -- The event propagation interaction between `useGamepadNav` and Radix's internal keyboard handler needs integration testing.

### Accessibility Requirements

- **Keyboard navigation**: Radix Tabs provides full WAI-ARIA Tabs pattern compliance including Tab, Arrow keys, Home/End navigation. This replaces the manual keyboard handling in the current custom tab implementation.
- **Screen readers**: Radix automatically sets `role="tablist"`, `role="tab"`, `role="tabpanel"`, and `aria-selected` attributes. The current implementation manually sets `role="tablist"` on the tab row but lacks `role="tab"` and `role="tabpanel"`.
- **react-resizable-panels**: Resize handles automatically include `role="separator"` and WAI-ARIA Window Splitter properties.
- **Impact**: Positive. Accessibility improves significantly by adopting Radix.

**Confidence**: High

### CSS Spatial Navigation (W3C Draft)

- **Status**: The CSS Spatial Navigation spec (W3C Working Draft) defines directional focus navigation with arrow keys. It has a polyfill but no browser has shipped it by default. Chromium has an experimental implementation behind a flag.
- **Relevance**: The `useGamepadNav` hook already implements equivalent functionality. No benefit to adopting the polyfill.
- **Impact**: None. The existing hook is more mature and tailored to CrossHook's needs.

**Confidence**: High

Sources:

- [CSS Spatial Navigation W3C Draft](https://www.w3.org/TR/css-nav-1/)
- [WICG Spatial Navigation](https://wicg.github.io/spatial-navigation/)

---

## Open Questions

1. **Sidebar collapsibility**: Should the vertical sidebar collapse to icon-only mode on smaller viewports or via a toggle button? If so, `@radix-ui/react-tooltip` should be added for icon tooltips. This affects the horizontal space budget at 1280px.

2. **ConsoleView placement**: The console currently sits at the bottom of the Main tab content. In the new layout, it could be:
   - A collapsible bottom panel within the Profile view (using a vertical `PanelGroup`)
   - A dedicated sidebar tab (Console view)
   - An always-visible footer strip
   - A toggleable drawer/overlay
     The choice affects vertical space allocation on the 800px display.

3. **Install Game as separate view vs sub-tab**: The current ProfileEditor has sub-tabs (Profile / Install Game). Moving Install Game to a top-level sidebar item simplifies the UI but changes the mental model. Should it remain a sub-context of Profile or become independent?

4. **Layout persistence**: `react-resizable-panels` supports saving/restoring panel sizes. Should panel sizes persist across sessions (via Tauri's filesystem) or reset on each launch?

5. **Keyboard navigation priority**: When using a gamepad, should sidebar navigation (Up/Down on sidebar items) take priority over content navigation, or should a focus-group boundary (press Right to enter content, Left to return to sidebar) be implemented? The current `useGamepadNav` does linear traversal across all focusable elements.

---

## Search Queries Executed

1. `Radix UI React headless components 2025 2026 bundle size tabs navigation menu`
2. `react-resizable-panels npm library split pane layout 2025`
3. `shadcn/ui vs Radix UI vs Ark UI headless component library comparison React 2025`
4. `Radix UI tabs vertical orientation implementation React example`
5. `Ark UI React headless components bundle size tabs 2025`
6. `React Aria Adobe accessible components tabs navigation keyboard gamepad 2025`
7. `Tauri v2 webview limitations CSS constraints desktop app React 2025`
8. `desktop application UI layout sidebar vertical tabs React best practices pattern`
9. `gamepad controller navigation React web application focus management accessibility`
10. `Steam Deck game launcher UI design 1280x800 resolution React desktop app`
11. `Headless UI Tailwind Labs React tabs vertical orientation gamepad accessible 2025`
12. `@tanstack/react-router client-side routing Tauri desktop app views navigation 2025`
13. `Radix UI integration existing custom CSS design system no Tailwind vanilla CSS 2025`
14. `vertical sidebar navigation pattern desktop app React collapsible panel icons labels best practice`
15. `react headless UI library bundle size comparison radix ark react-aria 2025`
16. `react-resizable-panels keyboard accessibility resize handle arrow keys touch support`
17. `Tauri v2 Linux WebKitGTK version CSS features supported backdrop-filter 2025`
18. `CSS spatial navigation W3C spec polyfill gamepad focus management web`
19. `Focus and spatial navigation in React (whoisryosuke.com)`

---

## Uncertainties and Gaps

1. **Exact gzipped bundle sizes**: Bundlephobia dynamically renders size data and could not be scraped. The stated sizes (~3-5 kB for Radix Tabs, ~10-15 kB for react-resizable-panels) are based on npm unpacked sizes and community reports, not direct measurement. These should be verified by installing the packages and checking the Vite build output.

2. **Radix keyboard handler vs useGamepadNav interaction**: The event propagation analysis (capture phase vs bubble phase) is based on code reading, not runtime testing. The `preventDefault()` call should prevent double-navigation, but edge cases around focus trapping and modal contexts need integration testing.

3. **WebKitGTK CSS feature coverage**: Specific CSS features used by react-resizable-panels (e.g., `cursor: col-resize`) have not been verified on WebKitGTK. The library does offer a `disableCursor` prop as a fallback.

4. **react-gamepads library**: The `react-gamepads` library by whoisryosuke was identified during research but is not recommended because CrossHook's `useGamepadNav` is already more tailored and does not need replacement. Documented for completeness.

5. **Spatial navigation libraries**: Two spatial navigation libraries were found (Norigin Spatial Navigation, BBC LRUD) that implement directional 2D focus management. These could enhance the gamepad experience if linear traversal proves inadequate for the new sidebar+content layout, but evaluating them is deferred until the layout is built and tested.

Sources:

- [Focus and Spatial Navigation in React](https://whoisryosuke.com/blog/2024/focus-and-spatial-navigation-in-react)
- [react-gamepads](https://github.com/whoisryosuke/react-gamepads)
- [Navigating the Web with a Gamepad](https://www.voorhoede.nl/en/blog/navigating-the-web-with-a-gamepad/)
