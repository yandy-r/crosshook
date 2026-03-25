# Code Analysis: ui-enhancements

## Executive Summary

CrossHook's React frontend is a 367-line god component (`App.tsx`) that lifts all state (profile, settings, recent files, steam paths, tab navigation, derived launch data) and distributes it via props to a horizontal tab layout with a two-column Main tab. The planned UI enhancement restructures this into a sidebar + content area + console drawer layout with two React Contexts (`ProfileContext`, `PreferencesContext`) replacing the current prop-drilling. The existing code provides clean hook interfaces (`UseProfileResult`, `GamepadNavState`) that are already shaped for context wrapping, but the CSS situation is split between a comprehensive BEM-like class system in `theme.css` and heavy inline `CSSProperties` objects in nearly every component.

## Existing Code Structure

### Related Components

- `src/crosshook-native/src/App.tsx`: Root shell holding all top-level state, tab navigation, heading derivation, `useGamepadNav` mount point, and layout orchestration. 367 lines, primary refactoring target.
- `src/crosshook-native/src/components/ProfileEditor.tsx`: Profile editing shell with Profile/Install sub-tab switching, review modal orchestration, delete confirmation overlay. 588 lines, will be split across ProfilesPage and InstallPage.
- `src/crosshook-native/src/components/ProfileFormSections.tsx`: Pure form renderer with conditional fields by launch method. 695 lines, reused as-is. Exports `deriveSteamClientInstallPath` and `ProtonInstallOption` type consumed by other components.
- `src/crosshook-native/src/components/LaunchPanel.tsx`: Launch controls with install-context branch rendering alternate UI. 257 lines, heavy inline styles via `panelStyles` object.
- `src/crosshook-native/src/components/LauncherExport.tsx`: Export lifecycle management (export, delete, stale detection) with ~100 lines of inline style constants. 655 lines.
- `src/crosshook-native/src/components/ConsoleView.tsx`: Self-contained log stream via `listen('launch-log')` event, inline styles despite matching unused `.crosshook-console__*` CSS classes. 270 lines.
- `src/crosshook-native/src/components/SettingsPanel.tsx`: Settings UI with `layoutStyles` record of CSSProperties. 470 lines. Accepts all data as props from App.tsx.
- `src/crosshook-native/src/components/CommunityBrowser.tsx`: Community profile browser with `panelStyles` inline object. 612 lines.
- `src/crosshook-native/src/components/CompatibilityViewer.tsx`: Compatibility data grid with inline `cardStyle`/`filterRowStyle` objects. Props-driven, no IPC calls of its own.
- `src/crosshook-native/src/components/InstallGamePanel.tsx`: Install wizard flow. 547 lines. Self-contained with its own hook (`useInstallGame`).
- `src/crosshook-native/src/components/ProfileReviewModal.tsx`: Portal-based modal with focus trap, `inert` sibling handling, `data-crosshook-focus-root="modal"`. 457 lines. Works anywhere via `createPortal`.
- `src/crosshook-native/src/components/AutoPopulate.tsx`: Steam auto-discovery with inline styles. 320 lines.
- `src/crosshook-native/src/hooks/useProfile.ts`: Profile CRUD state machine. 479 lines. Returns `UseProfileResult` interface -- the exact shape needed for context value.
- `src/crosshook-native/src/hooks/useLaunchState.ts`: Launch phase state machine with `useReducer` pattern. 244 lines. Consumes `profileId`, `method`, `request` as args.
- `src/crosshook-native/src/hooks/useGamepadNav.ts`: Gamepad polling, keyboard nav, focus management. 473 lines. Returns `GamepadNavState` with `rootRef` that must attach to the outermost layout element.
- `src/crosshook-native/src/hooks/useCommunityProfiles.ts`: Community tap CRUD and sync state.
- `src/crosshook-native/src/hooks/useInstallGame.ts`: Install flow state management (prefix resolution, validation, stage tracking).

### File Organization Pattern

```
src/crosshook-native/src/
  App.tsx                   # Root component (single file, no directory)
  main.tsx                  # React entry point (imports App, styles)
  components/               # Flat directory, no subdirectories
    ProfileEditor.tsx       # Composite component (sub-tabs + modal + form)
    ProfileFormSections.tsx  # Reusable form renderer
    LaunchPanel.tsx          # Domain panel
    ...
  hooks/                    # One hook per domain area
    useProfile.ts           # Profile CRUD
    useLaunchState.ts       # Launch phase machine
    useGamepadNav.ts        # Gamepad/keyboard navigation
    useInstallGame.ts       # Install flow state
    useCommunityProfiles.ts # Community taps
  styles/                   # Global CSS (no CSS modules, no component-scoped CSS)
    theme.css               # All component classes (870 lines)
    variables.css            # CSS custom properties (48 lines)
    focus.css                # Focus/controller navigation (108 lines)
  types/                    # TypeScript type definitions
    index.ts                 # Re-exports all types
    profile.ts               # GameProfile, LaunchMethod
    launch.ts                # LaunchRequest, LaunchPhase enum
    launcher.ts              # LauncherInfo, LauncherDeleteResult
    settings.ts              # AppSettingsData, RecentFilesData
    install.ts               # InstallGameRequest, InstallProfileReviewPayload
    profile-review.ts        # ProfileReviewSession
  utils/                    # Small utility modules
    dialog.ts                # Tauri file/folder dialog wrappers
    profile-compare.ts       # JSON-based structural equality
```

The planned restructure introduces two new subdirectories:

- `components/layout/` for Sidebar, ContentArea, ConsoleDrawer, NavSection, NavItem
- `components/pages/` for ProfilesPage, LaunchPage, InstallPage, CommunityPage, CompatibilityPage, SettingsPage
- `context/` for ProfileContext, PreferencesContext

## Implementation Patterns

### Pattern: Hook-Based State Machine

**Description**: Each domain area encapsulates state, loading flags, error state, and action functions in a single custom hook. The hook returns a flat object interface that components consume. State transitions are explicit (no shared mutable state).

**Example**: See `src/crosshook-native/src/hooks/useProfile.ts` lines 204-479. The `useProfile()` hook returns `UseProfileResult` (lines 14-36) which is a flat interface with 18 members covering state (`profiles`, `profile`, `dirty`, `loading`, `saving`, `deleting`, `error`, `profileExists`, `pendingDelete`), setters (`setProfileName`), and async actions (`selectProfile`, `saveProfile`, `confirmDelete`, `executeDelete`, `cancelDelete`, `refreshProfiles`, `persistProfileDraft`, `hydrateProfile`, `updateProfile`).

**Apply to**: `ProfileContext` should wrap exactly this return type. The context provider calls `useProfile()` and exposes the result unchanged. No transformation needed.

### Pattern: useReducer for Multi-Phase State

**Description**: Complex state machines with multiple phases use `useReducer` with a discriminated union of action types. This prevents impossible state combinations.

**Example**: See `src/crosshook-native/src/hooks/useLaunchState.ts` lines 7-69. `LaunchState` has `phase`, `errorMessage`, `helperLogPath`. `LaunchAction` is a union of 6 action types. The `reducer` function handles transitions cleanly.

**Apply to**: If console drawer needs unread count tracking or auto-expand behavior, `useReducer` is the established pattern for multi-state components.

### Pattern: Tauri IPC via invoke() with Cleanup Guards

**Description**: All backend calls use `invoke()` from `@tauri-apps/api/core`. Effects that call `invoke` use a `let active = true` guard pattern with cleanup function setting `active = false` to prevent state updates on unmounted components.

**Example**: See `src/crosshook-native/src/App.tsx` lines 173-209:

```typescript
useEffect(() => {
  let active = true;
  async function loadPreferences() {
    try {
      const [...] = await Promise.all([
        invoke<AppSettingsData>('settings_load'),
        invoke<RecentFilesData>('recent_files_load'),
        invoke<string>('default_steam_client_install_path'),
      ]);
      if (!active) return;
      // set state...
    } catch (error) { if (active) { setSettingsError(...); } }
  }
  void loadPreferences();
  return () => { active = false; };
}, [selectProfile]);
```

**Apply to**: `PreferencesContext` provider must replicate this exact pattern for loading settings, recent files, and steam paths on mount.

### Pattern: Tauri Event Listener with Cleanup

**Description**: Event subscriptions use `listen()` from `@tauri-apps/api/event`. The `listen()` returns a `Promise<UnlistenFn>`, which is cleaned up in the effect return.

**Example**: See `src/crosshook-native/src/components/ConsoleView.tsx` lines 86-110:

```typescript
useEffect(() => {
  let active = true;
  const unlistenPromise = listen<unknown>('launch-log', (event) => {
    // process...
    if (active) setLines((current) => [...current, entry]);
  });
  return () => {
    active = false;
    void unlistenPromise.then((unlisten) => unlisten());
  };
}, []);
```

**Apply to**: ConsoleDrawer must keep this subscription mounted at all times (never conditionally rendered based on route). This is the core fix for the log-loss-on-tab-switch bug.

### Pattern: Derived State via useMemo

**Description**: Computed values that depend on profile state are derived via `useMemo` chains. Multiple derived values cascade from the profile object.

**Example**: See `src/crosshook-native/src/App.tsx` lines 84-100:

```typescript
const launchMethod = useMemo(() => resolveLaunchMethod(profile), [profile]);
const effectiveLaunchMethod = useMemo<...>(() => {
  if (activeTab === 'main' && profileEditorTab === 'install') return 'proton_run';
  return launchMethod;
}, [activeTab, launchMethod, profileEditorTab]);
const steamClientInstallPath = useMemo(() => {
  return defaultSteamClientInstallPath || deriveSteamClientInstallPath(profile.steam.compatdata_path);
}, [defaultSteamClientInstallPath, profile.steam.compatdata_path]);
```

**Apply to**: These derivations should move either into `ProfileContext` (for `launchMethod`, `steamClientInstallPath`, `targetHomePath`) or into the consuming page (for `effectiveLaunchMethod` which depends on install page state). The `effectiveLaunchMethod` override that forces `proton_run` during install context can be scoped entirely to `InstallPage`.

### Pattern: BEM-Like CSS Class Naming

**Description**: CSS classes follow `crosshook-component`, `crosshook-component--modifier`, `crosshook-component__element`. All defined in `theme.css` with CSS custom properties from `variables.css`.

**Example**: See `src/crosshook-native/src/styles/theme.css`:

- Block: `.crosshook-console`
- Element: `.crosshook-console__header`, `.crosshook-console__body`, `.crosshook-console__line`, `.crosshook-console__timestamp`, `.crosshook-console__code`, `.crosshook-console__empty`
- Modifier: `.crosshook-modal__status-chip--warning`, `.crosshook-button--secondary`, `.crosshook-tab--active`

**Apply to**: All new sidebar/layout CSS must follow this convention. New classes should be:

- `.crosshook-sidebar`, `.crosshook-sidebar__brand`, `.crosshook-sidebar__nav`, `.crosshook-sidebar__item`, `.crosshook-sidebar__item--active`
- `.crosshook-app-layout` (grid container)
- `.crosshook-console-drawer`, `.crosshook-console-drawer--collapsed`, `.crosshook-console-drawer__toggle`

### Pattern: Inline Style Objects at Module Scope

**Description**: Several components define `CSSProperties` constants at module scope and apply them via `style={}` props. This is the "split personality" the codebase has with CSS.

**Example**: See `src/crosshook-native/src/components/LauncherExport.tsx` lines 32-110 (8 separate style constants: `panelStyle`, `sectionStyle`, `labelStyle`, `inputStyle`, `buttonStyle`, `subtleButtonStyle`, `deleteButtonStyle`, `helperStyle`, `infoCalloutStyle`). See `src/crosshook-native/src/components/ConsoleView.tsx` lines 258-268 (`buttonStyle` constant). See `src/crosshook-native/src/components/SettingsPanel.tsx` lines 27-59 (`layoutStyles` record).

**Apply to**: The CSS migration must convert these to `.crosshook-*` classes. Priority: ConsoleView first (`.crosshook-console__*` classes already exist in theme.css but are unused), then LaunchPanel, then LauncherExport.

### Pattern: Gamepad Navigation Scope via rootRef + Modal Override

**Description**: `useGamepadNav` attaches to a `rootRef` on the outermost layout element. It traverses focusable elements in DOM order within the navigation root. When a modal is open, it detects `[data-crosshook-focus-root="modal"]` and restricts traversal to the modal subtree.

**Example**: See `src/crosshook-native/src/hooks/useGamepadNav.ts` lines 69-76:

```typescript
const MODAL_FOCUS_ROOT_SELECTOR = '[data-crosshook-focus-root="modal"]';
function getNavigationRoot(rootRef) {
  const modalRoots = document.querySelectorAll<HTMLElement>(MODAL_FOCUS_ROOT_SELECTOR);
  return modalRoots.item(modalRoots.length - 1) ?? getRootElement(rootRef);
}
```

And lines 322-377: Keyboard handler uses capture-phase (`addEventListener('keydown', handler, true)`) and calls `event.preventDefault()` for arrow keys, Tab, Enter, Space, Escape. Editable elements are excluded (lines 163-194).

**Apply to**: The `rootRef` must attach to the new `.crosshook-app-layout` div. The sidebar items must be standard `<button>` elements to be included in the focusable element traversal. The current linear DOM-order traversal means sidebar buttons will be traversed before content area elements since they appear first in DOM order. This is actually desirable for the sidebar-first navigation pattern. However, implementing zone-based navigation (sidebar vs. content) may require modifying `useGamepadNav` to support focus zones.

### Pattern: Portal-Based Modal with Focus Trap

**Description**: `ProfileReviewModal` uses `createPortal(content, document.body)` and manages: scroll lock (adding `crosshook-modal-open` class to body), sibling inert (hiding all root-level nodes except the portal), keyboard escape, and the `data-crosshook-focus-root="modal"` attribute for gamepad nav.

**Example**: See `src/crosshook-native/src/components/ProfileReviewModal.tsx` lines 1-44 for the interface. The modal's z-index is `1200` (set on `.crosshook-modal-portal` in theme.css line 486-488).

**Apply to**: No changes needed. The portal renders to `document.body`, independent of component tree position. The z-index of `1200` clears the planned sidebar (`z-index: 100`) and console drawer (`z-index: 50`). The `InstallPage` will render `ProfileReviewModal` just as `ProfileEditorView` does today.

### Pattern: Conditional Component Rendering via Context Prop

**Description**: Several components accept a `context` prop (`'default' | 'install'`) that toggles between two entirely different render paths. This is a code smell the restructure eliminates.

**Example**: See `src/crosshook-native/src/components/LaunchPanel.tsx` lines 51-127 (install context returns completely different JSX). See `src/crosshook-native/src/components/LauncherExport.tsx` lines 270-336 (install context returns a static informational panel).

**Apply to**: Remove these `context === 'install'` branches entirely. When each domain gets its own page, the install-specific UI is handled by `InstallPage` directly, not by cramming two UIs into one component.

## Integration Points

### Files to Create

All paths relative to `src/crosshook-native/src/`:

- `context/ProfileContext.tsx`: React context wrapping `UseProfileResult`. Provider calls `useProfile({ autoSelectFirstProfile: false })` (matching current App.tsx behavior). Export `useProfileContext()` consumer hook.
- `context/PreferencesContext.tsx`: React context wrapping settings, recentFiles, steamClientInstallPath, targetHomePath, and mutator functions (refreshPreferences, handleAutoLoadChange, clearRecentFiles). Provider replicates the init logic from App.tsx lines 173-241.
- `components/layout/Sidebar.tsx`: Vertical nav with NavSection/NavItem. Accepts `activeRoute` and `onNavigate` props.
- `components/layout/NavSection.tsx`: Section heading + children wrapper.
- `components/layout/NavItem.tsx`: Button with icon placeholder, label, active state indicator.
- `components/layout/ContentArea.tsx`: Switch on `AppRoute`, render correct page.
- `components/layout/ConsoleDrawer.tsx`: Bottom drawer wrapping ConsoleView. Manages collapsed/expanded state. Must always be mounted.
- `components/pages/ProfilesPage.tsx`: ProfileFormSections + ProfileActions. Reads from ProfileContext.
- `components/pages/LaunchPage.tsx`: LaunchPanel. Reads profile/launch state from ProfileContext.
- `components/pages/InstallPage.tsx`: InstallGamePanel + ProfileReviewModal. Absorbs review session state from ProfileEditorView (lines 84-316 of ProfileEditor.tsx).
- `components/pages/ExportPage.tsx`: LauncherExport. Reads profile from ProfileContext. (Note: feature-spec says Export is a subsection within Profiles, not a separate page.)
- `components/pages/CommunityPage.tsx`: CommunityBrowser wrapper.
- `components/pages/CompatibilityPage.tsx`: CompatibilityViewer wrapper.
- `components/pages/SettingsPage.tsx`: SettingsPanel wrapper, reading from PreferencesContext.
- `components/ProfileActions.tsx`: Extracted Save/Delete/dirty bar from ProfileEditorView lines 438-455.
- `styles/layout.css`: App layout grid (sidebar + content + drawer).
- `styles/sidebar.css`: Sidebar, nav section, nav item styles.
- `styles/console-drawer.css`: Drawer container, toggle bar, resize handle.

### Files to Modify

- `App.tsx`: Replace 367-line god component with ~60-line shell. Remove: all `useState` for settings/recentFiles/defaultSteamClientInstallPath/activeTab/profileEditorTab/settingsError. Remove: all derived `useMemo` chains. Remove: heading derivation logic (lines 128-158). Remove: tab row JSX and conditional tab rendering. Replace with: `<AppProviders>` wrapping `<div.crosshook-app-layout>` containing `<Sidebar>`, `<ContentArea>`, `<ConsoleDrawer>`. Keep: `useGamepadNav` at root with `rootRef` on layout div. Keep: `handleGamepadBack` function.
- `ProfileEditor.tsx`: Split. Profile editing portion (lines 389-456, the form + save/delete buttons) moves to `ProfilesPage`. Install/review orchestration (lines 81-316, review session state, confirmation handling, handleOpenProfileReview, handleCloseProfileReview, handleSaveProfileReview) moves to `InstallPage`. Delete confirmation dialog (lines 532-565) stays with profile editing. The standalone `ProfileEditor` export (lines 570-586) can be removed (unused).
- `LaunchPanel.tsx`: Remove `context` prop and install context branch (lines 51-127). Migrate `panelStyles.card` inline style to `.crosshook-launch-panel` CSS class. Migrate all inline `style={}` props to CSS classes.
- `LauncherExport.tsx`: Remove `context` prop and install context branch (lines 270-336). Convert 8 style constants (lines 32-110) to CSS classes. Remove ~100 lines of CSSProperties.
- `ConsoleView.tsx`: Remove outer `<section>` wrapper with border/shadow (ConsoleDrawer provides this). Migrate inline styles to existing `.crosshook-console__*` CSS classes that already exist in theme.css (lines 416-479) but are currently unused. Convert the `buttonStyle` constant (lines 258-268) to a CSS class.
- `SettingsPanel.tsx`: Remove `targetHomePath` and `steamClientInstallPath` props. Read from `PreferencesContext`. Optionally migrate `layoutStyles` record to CSS.
- `styles/theme.css`: Add new CSS classes for sidebar layout, content area, and migrated inline styles. The `.crosshook-console__*` classes (lines 416-479) are already defined and waiting to be consumed.
- `styles/variables.css`: Add `--crosshook-sidebar-width: 200px`, `--crosshook-sidebar-width-collapsed: 56px`, `--crosshook-console-drawer-height: 280px`.
- `main.tsx`: Add imports for new CSS files (`layout.css`, `sidebar.css`, `console-drawer.css`).

## Code Conventions

### Naming

- **Files**: `PascalCase.tsx` for components, `camelCase.ts` for hooks and utilities, `kebab-case.css` for styles, `kebab-case.ts` for types.
- **Components**: `PascalCase` function exports. Named exports preferred (e.g., `export function ProfileEditorView`). Default exports also provided for backward compat (e.g., `export default LaunchPanel`).
- **Hooks**: `camelCase` prefixed with `use`. Return typed interfaces (e.g., `UseProfileResult`, `GamepadNavState`).
- **CSS classes**: `crosshook-block`, `crosshook-block__element`, `crosshook-block--modifier`. All use `--crosshook-*` custom properties.
- **Types**: `PascalCase` interfaces in dedicated type files. Barrel export via `types/index.ts`.
- **Props interfaces**: `ComponentNameProps` pattern (e.g., `LaunchPanelProps`, `SettingsPanelProps`, `ProfileEditorProps`).

### Error Handling

- Tauri `invoke()` errors caught in try/catch, displayed via component-level `error` state or `settingsError` at App level.
- Pattern: `error instanceof Error ? error.message : String(error)` for consistent string conversion.
- No global error boundary exists. Each component manages its own error display.
- Async handlers in JSX callbacks use `void functionCall().catch(...)` pattern to avoid unhandled promise warnings.

### Testing

- No frontend test framework is configured (confirmed in CLAUDE.md).
- Rust tests exist for `crosshook-core` (`cargo test -p crosshook-core`).
- New context providers and layout components should be structured to be testable, even if tests are not written initially.

## Dependencies and Services

### Available Utilities

- `@tauri-apps/api/core` `invoke()`: All backend RPC. Used for profile CRUD, settings, launcher export, steam discovery, launch orchestration.
- `@tauri-apps/api/event` `listen()`: Event subscription for `launch-log` and `auto-load-profile` events.
- `@tauri-apps/plugin-dialog` `open()`: Native file/folder dialogs, wrapped in `utils/dialog.ts`.
- `utils/profile-compare.ts` `profilesEqual()`: JSON-based structural equality for profile drafts.
- `ProfileFormSections` exports `deriveSteamClientInstallPath()`: Utility function used by App.tsx and ProfileEditor.tsx to derive steam client path from compatdata path.

### Required New Dependencies

- `@radix-ui/react-tabs` (latest): Headless vertical tab navigation. Provides `orientation="vertical"`, `data-state`/`data-orientation` CSS attributes, WAI-ARIA compliance. ~3-5 kB gzipped.
- `react-resizable-panels` (^4.x): Split-pane layout for sidebar + content, and console drawer resize. Keyboard-accessible resize handles, touch-friendly, layout persistence. ~10-15 kB gzipped.
- `@radix-ui/react-tooltip` (latest, optional): Tooltips for icon-only sidebar items when collapsed. ~3-5 kB gzipped.

### Existing Dependencies (package.json)

- `react` ^18.3.1, `react-dom` ^18.3.1: Stable. No upgrade needed.
- `@tauri-apps/api` ^2.0.0: Tauri v2 IPC.
- `@tauri-apps/plugin-dialog` ^2.0.0, `@tauri-apps/plugin-fs` ^2.0.0, `@tauri-apps/plugin-shell` ^2.0.0: Tauri plugins.
- `typescript` ^5.6.3, `vite` ^8.0.2, `@vitejs/plugin-react` ^5.2.0: Build tooling.

## Gotchas and Warnings

- **Console log loss on navigation**: Currently `ConsoleView` is rendered conditionally inside the Main tab. Switching to Settings or Community unmounts the component and loses the `listen('launch-log')` subscription. The ConsoleDrawer must be always-mounted, never conditionally rendered based on the active route.
- **Gamepad linear DOM traversal**: `useGamepadNav` traverses focusable elements in DOM order within the navigation root. With a sidebar, pressing Down from the last sidebar item will jump to the first content area element. This is adequate for basic navigation but may feel jarring. Zone-based navigation (sidebar vs. content focus zones with Left/Right to switch) would require modifying `useGamepadNav` or adding a wrapper layer.
- **Keyboard handler capture phase**: `useGamepadNav` registers keydown on `document` in capture phase (`true` third arg). It calls `preventDefault()` on ArrowDown/Up/Left/Right/Tab/Enter/Space/Escape. Editable elements (inputs, textareas, selects) are excluded. The sidebar items (buttons) will be non-editable and subject to these handlers. This means arrow keys will navigate between sidebar items as expected.
- **Modal focus scope survives restructure**: `ProfileReviewModal` uses `createPortal(content, document.body)` and `data-crosshook-focus-root="modal"` to override gamepad navigation scope. The modal z-index (1200) clears all planned layout layers. No changes needed, but if `InstallPage` is unmounted while the review modal is open (user navigates away), the portal will also unmount. The `InstallPage` should remain mounted when a review session is active, or the session state needs to be lifted to context.
- **Install review session state lifting**: Currently `ProfileEditorView` owns the review session state (`profileReviewSession`, `reviewConfirmation`). When splitting to `InstallPage`, this state moves there. But if the user navigates away from the Install page mid-review, the session will be lost unless the state is preserved in a context or the navigation is blocked. The feature spec says to persist sessions with a sidebar badge indicator -- this requires the review session state to be in a context rather than in `InstallPage` local state.
- **`effectiveLaunchMethod` override scoping**: Currently App.tsx overrides `effectiveLaunchMethod` to `proton_run` when `profileEditorTab === 'install'`. After restructuring, this override should be scoped to `InstallPage` only, not in the global profile context. The `LaunchPage` should always use the real launch method from the profile.
- **`deriveSteamClientInstallPath` import chain**: This utility is exported from `ProfileFormSections.tsx` (a component file) and imported by both `App.tsx` and `ProfileEditor.tsx`. It should be moved to `utils/` during restructuring to break the awkward import dependency from App.tsx into a component file.
- **Unused CSS classes**: `theme.css` lines 416-479 define `.crosshook-console__*` classes that `ConsoleView.tsx` does not use (it uses inline styles instead). The migration should wire these up rather than creating duplicates.
- **CSS custom properties for colors not used consistently**: Many components hardcode hex colors (e.g., `'#7bb0ff'`, `'#9fb1d6'`, `'#60a5fa'`, `'#fee2e2'`) in inline styles instead of using `--crosshook-color-*` variables. The CSS migration should map these to the design token system.
- **Delete confirmation overlay z-index conflict**: The profile delete overlay (lines 532-565 in ProfileEditor.tsx) uses `z-index: 1000` via `.crosshook-profile-editor-delete-overlay`. The console drawer should use a lower z-index (50 as planned) to avoid conflicts.
- **`handleGamepadBack` function**: Currently defined at module scope in App.tsx (lines 63-69). It finds `[data-crosshook-modal-close]` buttons inside `[data-crosshook-focus-root="modal"]` and clicks them. This stays at the App level. After restructuring, consider also using it to return focus from content area to sidebar when no modal is open.
- **No `autoSelectFirstProfile` default alignment**: `App.tsx` calls `useProfile({ autoSelectFirstProfile: false })` but the hook defaults to `true`. The `ProfileContext` provider must preserve `{ autoSelectFirstProfile: false }`.
- **Type re-exports via barrel**: `types/index.ts` re-exports all types. New types for routes (`AppRoute`) and context values should follow this barrel pattern.
- **Stale launcher detection**: `LauncherExport` checks launcher status on mount and when profile changes via `refreshLauncherStatus()` callback. If Export becomes a subsection of ProfilesPage (per feature-spec), this callback must trigger when the subsection becomes visible, not just on component mount.

## Task-Specific Guidance

### For Context Providers (ProfileContext, PreferencesContext)

- **ProfileContext**: Wrap `UseProfileResult` (defined at `hooks/useProfile.ts` lines 14-36). Call `useProfile({ autoSelectFirstProfile: false })` in the provider. Export a `useProfileContext()` hook that throws if used outside the provider. The return type is already a flat interface -- no reshaping needed.
- **PreferencesContext shape**: Extract from App.tsx lines 74-77 + lines 211-241. The context value should include: `settings: AppSettingsData`, `recentFiles: RecentFilesData`, `settingsError: string | null`, `defaultSteamClientInstallPath: string`, `steamClientInstallPath: string` (derived), `targetHomePath: string` (derived), `refreshPreferences: () => Promise<void>`, `handleAutoLoadChange: (enabled: boolean) => Promise<void>`, `clearRecentFiles: () => Promise<void>`.
- **Provider composition**: Create an `AppProviders` component that nests `<ProfileProvider>` inside `<PreferencesProvider>` (or vice versa, order depends on whether preferences need profile data). Current code has preferences loading independent of profile, so order does not matter.
- **Auto-load profile event**: The `listen<string>('auto-load-profile')` subscription (App.tsx lines 201-203) calls `selectProfile(event.payload)`. This must live in the `ProfileContext` provider since it depends on `selectProfile` from `useProfile()`.

### For Sidebar/Layout

- **Route type**: `type AppRoute = 'profiles' | 'launch' | 'install' | 'community' | 'compatibility' | 'settings'`. Simple `useState<AppRoute>('profiles')` in App.tsx.
- **Sidebar structure**: Use `<nav>` with `<button>` elements for nav items. Buttons are automatically included in `useGamepadNav` focusable traversal. Apply `.crosshook-sidebar__item--active` class to the active route item.
- **Gamepad rootRef**: Attach `gamepadNav.rootRef` to the outermost `<div className="crosshook-app-layout">` element (replaces the current `<main ref={gamepadNav.rootRef}>` on line 244 of App.tsx).
- **Controller mode indicator**: Move from App.tsx header (line 253) to sidebar footer area.
- **Layout CSS Grid**: `grid-template-columns: var(--crosshook-sidebar-width) 1fr; grid-template-rows: 1fr auto;` where the second row is the console drawer.

### For Console Drawer

- **Always mounted**: The `ConsoleView` component's `listen('launch-log')` subscription must never unmount. The drawer collapses visually (CSS height) but stays in the DOM.
- **Existing CSS classes**: `theme.css` already defines `.crosshook-console`, `.crosshook-console__header`, `.crosshook-console__body`, `.crosshook-console__line`, `.crosshook-console__timestamp`, `.crosshook-console__code`, `.crosshook-console__empty`. Wire `ConsoleView` to use these.
- **Remove outer section**: `ConsoleView` currently wraps everything in a `<section>` with inline border/shadow styles (lines 119-128). The `ConsoleDrawer` will provide the outer container, so `ConsoleView` should become a pure body renderer.
- **Auto-scroll behavior**: `ConsoleView` already has auto-scroll logic (lines 67-84) via `shouldFollowRef`. This works independently of the drawer.

### For CSS Migration

- **Priority order**: (1) ConsoleView to `.crosshook-console__*` classes (already defined, just unused), (2) LaunchPanel inline styles to new `.crosshook-launch-panel*` classes, (3) LauncherExport style constants to new `.crosshook-launcher-export*` classes, (4) SettingsPanel `layoutStyles` to CSS, (5) CommunityBrowser `panelStyles` to CSS.
- **Color token mapping**: Common hardcoded colors to variables -- `#7bb0ff` / `#60a5fa` -> `--crosshook-color-accent-strong`, `#9fb1d6` -> `--crosshook-color-text-muted`, `#f8fafc` / `#eef4ff` -> `--crosshook-color-text`, `#94a3b8` -> `--crosshook-color-text-subtle`, `#fee2e2` -> near `--crosshook-color-danger`.
- **New CSS files**: `layout.css`, `sidebar.css`, `console-drawer.css`. Import in `main.tsx` alongside `theme.css` and `focus.css`.
- **Touch target minimum**: All interactive sidebar items must respect `--crosshook-touch-target-min: 48px`.
- **Responsive breakpoints**: Existing breakpoints at `1360px` and `900px` remain. Sidebar collapses to 56px icon rail below 900px.
