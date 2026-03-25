# UI Enhancements Implementation Plan

Replace CrossHook's 3-tab horizontal layout (where a 367-line god component `App.tsx` props-drills all state into a two-column Main tab) with a vertical sidebar navigation containing 6 items (Profiles, Launch, Install, Browse, Compatibility, Settings), single-purpose content areas, and a persistent console drawer. The implementation uses `@radix-ui/react-tabs` for accessible vertical tabs, `react-resizable-panels` for the resizable sidebar, and two React Contexts (`ProfileContext`, `PreferencesContext`) to replace prop-drilling. The work decomposes into 3 phases (Foundation, View Separation, Polish) across 27 tasks with a critical path through App.tsx refactor -> page extraction -> gamepad testing.

## Critically Relevant Files and Documentation

- src/crosshook-native/src/App.tsx: 367-line god component, primary refactoring target — all state, tab nav, heading derivation, layout
- src/crosshook-native/src/components/ProfileEditor.tsx: 588 lines — split into ProfilesPage + InstallPage
- src/crosshook-native/src/components/ProfileFormSections.tsx: 695 lines — reused as-is, exports `deriveSteamClientInstallPath`
- src/crosshook-native/src/components/LaunchPanel.tsx: 257 lines — remove install-context branch, migrate inline styles
- src/crosshook-native/src/components/LauncherExport.tsx: 655 lines — remove install branch, migrate ~100 lines style constants
- src/crosshook-native/src/components/ConsoleView.tsx: 270 lines — migrate inline styles to existing unused `.crosshook-console__*` classes
- src/crosshook-native/src/components/InstallGamePanel.tsx: 547 lines — rendered by new InstallPage, no changes
- src/crosshook-native/src/components/ProfileReviewModal.tsx: 457 lines — portal-based, layout-agnostic, z-index 1200
- src/crosshook-native/src/components/SettingsPanel.tsx: 470 lines — switch from props to PreferencesContext
- src/crosshook-native/src/components/CommunityBrowser.tsx: 612 lines — inline style migration candidate
- src/crosshook-native/src/components/CompatibilityViewer.tsx: Inline style migration candidate
- src/crosshook-native/src/components/AutoPopulate.tsx: 320 lines — inline style migration candidate
- src/crosshook-native/src/hooks/useProfile.ts: 479 lines — clean UseProfileResult interface, wrap in ProfileContext
- src/crosshook-native/src/hooks/useLaunchState.ts: 244 lines — useReducer pattern, consumed by LaunchPage
- src/crosshook-native/src/hooks/useGamepadNav.ts: 473 lines — highest risk during restructure, DOM-order traversal
- src/crosshook-native/src/hooks/useInstallGame.ts: Install flow state, consumed by InstallPage
- src/crosshook-native/src/hooks/useCommunityProfiles.ts: Community state, consumed by CommunityPage
- src/crosshook-native/src/styles/theme.css: 870 lines — extend with sidebar/layout/migration classes
- src/crosshook-native/src/styles/variables.css: 48 lines — add sidebar/drawer CSS variables
- src/crosshook-native/src/styles/focus.css: 108 lines — has unused `.crosshook-controller-prompts` class
- src/crosshook-native/src/types/index.ts: Type barrel — add AppRoute type
- src/crosshook-native/src/utils/dialog.ts: File/folder dialog wrappers
- src/crosshook-native/src/utils/profile-compare.ts: Profile structural equality
- src/crosshook-native/package.json: Add @radix-ui/react-tabs, react-resizable-panels
- docs/plans/ui-enhancements/feature-spec.md: All resolved decisions, architecture, success criteria
- docs/plans/ui-enhancements/research-technical.md: Component tree, state management details
- docs/plans/ui-enhancements/research-external.md: Radix API docs, CSS examples, gamepad compatibility
- docs/plans/ui-enhancements/research-ux.md: Competitive analysis, gamepad patterns, dark theme
- CLAUDE.md: Project conventions, commit format, build commands

## Implementation Plan

### Phase 1: Foundation (Shell Layout + State Contexts)

#### Task 1.1: Install dependencies and add CSS variables

Depends on [none]

**READ THESE BEFORE TASK**

- src/crosshook-native/package.json
- src/crosshook-native/src/styles/variables.css
- docs/plans/ui-enhancements/research-external.md

**Instructions**

Files to Create

- (none)

Files to Modify

- src/crosshook-native/package.json
- src/crosshook-native/src/styles/variables.css

Install `@radix-ui/react-tabs` and `react-resizable-panels` via npm. Add these CSS custom properties to `variables.css`:

```css
--crosshook-sidebar-width: 200px;
--crosshook-sidebar-width-collapsed: 56px;
--crosshook-console-drawer-height: 280px;
--crosshook-console-drawer-handle-height: 40px;
```

Add them inside the existing `:root` block. Also add responsive overrides in the existing `@media (max-width: 900px)` block to set `--crosshook-sidebar-width: var(--crosshook-sidebar-width-collapsed)`.

#### Task 1.2: Create layout CSS files

Depends on [1.1]

**READ THESE BEFORE TASK**

- src/crosshook-native/src/styles/theme.css
- src/crosshook-native/src/styles/variables.css
- docs/plans/ui-enhancements/research-external.md

**Instructions**

Files to Create

- src/crosshook-native/src/styles/layout.css
- src/crosshook-native/src/styles/sidebar.css
- src/crosshook-native/src/styles/console-drawer.css

Files to Modify

- src/crosshook-native/src/main.tsx

Create three CSS files following the `crosshook-*` BEM naming convention using `--crosshook-*` variables:

**layout.css**: Define `.crosshook-app-layout` as a CSS Grid with `grid-template-columns: var(--crosshook-sidebar-width) 1fr` and `grid-template-rows: 1fr auto`. The second row is for the console drawer. Add `.crosshook-content-area` for the scrollable content region. Add `.crosshook-page-header` for consistent page titles.

**sidebar.css**: Define `.crosshook-sidebar` (flex column, dark surface background `var(--crosshook-color-surface-strong)`), `.crosshook-sidebar__brand`, `.crosshook-sidebar__nav`, `.crosshook-sidebar__section`, `.crosshook-sidebar__section-label` (uppercase eyebrow), `.crosshook-sidebar__item` (48px min-height, flex with gap for icon+label), `.crosshook-sidebar__item[data-state='active']` (accent gradient, matching current `.crosshook-tab--active`), `.crosshook-sidebar__footer` (margin-top: auto for bottom-pinned items). Include `:focus-visible` styles matching the existing `border-color + box-shadow` pattern. Include `@media (max-width: 900px)` for collapsed icon-only rail.

**console-drawer.css**: Define `.crosshook-console-drawer` (grid-row placement, border-top), `.crosshook-console-drawer--collapsed` (height: var(--crosshook-console-drawer-handle-height)), `.crosshook-console-drawer__toggle` (flex bar with label, line count, expand/collapse button), `.crosshook-console-drawer__body` (overflow-y auto, transition on height).

Import all three in `main.tsx` alongside the existing style imports.

#### Task 1.3: Create ProfileContext

Depends on [none]

**READ THESE BEFORE TASK**

- src/crosshook-native/src/hooks/useProfile.ts
- src/crosshook-native/src/App.tsx

**Instructions**

Files to Create

- src/crosshook-native/src/context/ProfileContext.tsx

Create a React context wrapping the `UseProfileResult` interface from `useProfile.ts` (lines 14-36). The provider component calls `useProfile({ autoSelectFirstProfile: false })` — this matches the current `App.tsx` line 72 behavior. Export:

1. `ProfileProvider` component (wraps children in context)
2. `useProfileContext()` hook that throws if used outside provider

Also move the derived values into the context: `launchMethod` (via `resolveLaunchMethod` from App.tsx lines 45-61), `steamClientInstallPath` (derived from `deriveSteamClientInstallPath`), and `targetHomePath` (from `deriveTargetHomePath` in App.tsx lines 33-43). Move `deriveSteamClientInstallPath` from `ProfileFormSections.tsx` to a new `utils/steam.ts` file (or inline it) to break the awkward import chain.

Include the `listen<string>('auto-load-profile')` subscription (App.tsx lines 201-208) in the provider since it calls `selectProfile`.

#### Task 1.4: Create PreferencesContext

Depends on [none]

**READ THESE BEFORE TASK**

- src/crosshook-native/src/App.tsx
- src/crosshook-native/src/types/settings.ts

**Instructions**

Files to Create

- src/crosshook-native/src/context/PreferencesContext.tsx

Create a React context wrapping settings and preferences state. Extract from App.tsx lines 74-77 (state declarations) and lines 173-241 (load/mutate functions). The context value interface:

```typescript
interface PreferencesContextValue {
  settings: AppSettingsData;
  recentFiles: RecentFilesData;
  settingsError: string | null;
  defaultSteamClientInstallPath: string;
  refreshPreferences: () => Promise<void>;
  handleAutoLoadChange: (enabled: boolean) => Promise<void>;
  clearRecentFiles: () => Promise<void>;
}
```

Replicate the `let active = true` guard pattern from App.tsx for the init `useEffect`. Export `PreferencesProvider` and `usePreferencesContext()`.

#### Task 1.5: Create Sidebar component

Depends on [1.2]

**READ THESE BEFORE TASK**

- src/crosshook-native/src/styles/sidebar.css
- docs/plans/ui-enhancements/research-external.md
- docs/plans/ui-enhancements/feature-spec.md

**Instructions**

Files to Create

- src/crosshook-native/src/components/layout/Sidebar.tsx

Create the vertical sidebar navigation component using `@radix-ui/react-tabs`. The sidebar renders as a Radix `Tabs.List` with `orientation="vertical"`. Props: `activeRoute: AppRoute`, `onNavigate: (route: AppRoute) => void`, `controllerMode: boolean`, `lastProfile: string`.

Structure with 3 NavSections:

- **Game**: Profiles, Launch
- **Setup**: Install Game
- **Community**: Browse, Compatibility

Bottom-pinned: Settings item, status indicators (controller mode, last profile name).

Each nav item is a Radix `Tabs.Trigger` with `className="crosshook-sidebar__item"` and `value` matching the route. Use `data-state="active"` (auto-set by Radix) for styling. All items must be `<button>` elements (required for `useGamepadNav` traversal). Ensure 48px min-height per `--crosshook-touch-target-min`.

Define `AppRoute` type: `'profiles' | 'launch' | 'install' | 'community' | 'compatibility' | 'settings'`. Export it from this file or a new `types/routes.ts`.

#### Task 1.6: Create ContentArea component

Depends on [1.2]

**READ THESE BEFORE TASK**

- src/crosshook-native/src/styles/layout.css
- src/crosshook-native/src/App.tsx

**Instructions**

Files to Create

- src/crosshook-native/src/components/layout/ContentArea.tsx

Create the route dispatcher component. Initially render existing components directly (before page shells exist in Phase 2):

```typescript
function ContentArea({ route }: { route: AppRoute }) {
  switch (route) {
    case 'profiles': return <ProfileEditorView ... />;
    case 'launch': return <LaunchPanel ... />;
    // etc. - stub or placeholder for each route
    default: return null;
  }
}
```

Wrap content in `<div className="crosshook-content-area">` with consistent page-level padding and scroll behavior. Each route gets a Radix `Tabs.Content` wrapper with `value` matching the route. Use `forceMount` to keep critical components mounted if needed.

This is a temporary scaffold — Phase 2 replaces the direct component renders with proper page shells.

#### Task 1.7: Create ConsoleDrawer component

Depends on [1.2]

**READ THESE BEFORE TASK**

- src/crosshook-native/src/components/ConsoleView.tsx
- src/crosshook-native/src/styles/console-drawer.css

**Instructions**

Files to Create

- src/crosshook-native/src/components/layout/ConsoleDrawer.tsx

Create a collapsible bottom drawer wrapping `ConsoleView`. State: `collapsed: boolean` (default `true`). The drawer renders:

1. A toggle bar (`.crosshook-console-drawer__toggle`) always visible — shows "Runtime Console" label, line count badge, and expand/collapse button.
2. When expanded, the drawer body (`.crosshook-console-drawer__body`) renders `<ConsoleView />`.

The drawer must **always be mounted** in the DOM (never conditionally rendered based on route). This fixes the log-loss-on-tab-switch bug. Use CSS height transitions for expand/collapse, not conditional rendering.

Use `react-resizable-panels` `PanelGroup` with vertical direction for the content area + drawer split, allowing the user to drag the drawer height. The drawer panel should have `collapsible` prop with `collapsedSize` matching the toggle bar height.

#### Task 1.8: Refactor App.tsx to shell layout

Depends on [1.3, 1.4, 1.5, 1.6, 1.7]

**READ THESE BEFORE TASK**

- src/crosshook-native/src/App.tsx
- src/crosshook-native/src/context/ProfileContext.tsx
- src/crosshook-native/src/context/PreferencesContext.tsx
- src/crosshook-native/src/components/layout/Sidebar.tsx
- src/crosshook-native/src/components/layout/ContentArea.tsx
- src/crosshook-native/src/components/layout/ConsoleDrawer.tsx

**Instructions**

Files to Modify

- src/crosshook-native/src/App.tsx

This is the critical bottleneck task. Replace the entire 367-line god component with a ~60-line shell:

1. **Remove**: All `useState` for settings, recentFiles, defaultSteamClientInstallPath, activeTab, profileEditorTab, settingsError. Remove all derived `useMemo` chains (launchMethod, effectiveLaunchMethod, steamClientInstallPath, targetHomePath, launchRequest, headingTitle, headingCopy, compatibilityEntries, shouldShowLauncherExport). Remove all handler functions (refreshPreferences, handleAutoLoadChange, clearRecentFiles). Remove the header, tab row JSX, and all conditional tab content rendering.

2. **Keep**: `useGamepadNav({ onBack: handleGamepadBack })` at root. The `handleGamepadBack` function. The `rootRef` attachment (now on `.crosshook-app-layout` div).

3. **Replace with**: `useState<AppRoute>('profiles')` for route. Wrap children in `<ProfileProvider>` and `<PreferencesProvider>`. Render:

```tsx
<main ref={gamepadNav.rootRef} className="crosshook-app crosshook-focus-scope">
  <ProfileProvider>
    <PreferencesProvider>
      <Tabs.Root orientation="vertical" value={route} onValueChange={setRoute}>
        <PanelGroup direction="horizontal">
          <Panel defaultSize={15} minSize={10} maxSize={25} collapsible>
            <Sidebar activeRoute={route} onNavigate={setRoute} ... />
          </Panel>
          <PanelResizeHandle className="crosshook-resize-handle" />
          <Panel defaultSize={85}>
            <PanelGroup direction="vertical">
              <Panel defaultSize={75}>
                <ContentArea route={route} />
              </Panel>
              <PanelResizeHandle />
              <Panel defaultSize={25} collapsible collapsedSize={5}>
                <ConsoleDrawer />
              </Panel>
            </PanelGroup>
          </Panel>
        </PanelGroup>
      </Tabs.Root>
    </PreferencesProvider>
  </ProfileProvider>
</main>
```

Target: ~60 lines. Every Phase 2 task depends on this completing.

### Phase 2: View Separation (Page Shells + Cleanup)

#### Task 2.1: Extract ProfileActions component

Depends on [1.8]

**READ THESE BEFORE TASK**

- src/crosshook-native/src/components/ProfileEditor.tsx

**Instructions**

Files to Create

- src/crosshook-native/src/components/ProfileActions.tsx

Extract the Save/Delete/dirty-indicator bar from `ProfileEditorView` lines 438-455. The component receives from ProfileContext: `profileName`, `dirty`, `loading`, `saving`, `deleting`, `profileExists`, `saveProfile`, `confirmDelete`. Renders the Save button, Delete button, and dirty state indicator. Include the error banner rendering (line 455).

#### Task 2.2: Create ProfilesPage

Depends on [2.1]

**READ THESE BEFORE TASK**

- src/crosshook-native/src/components/ProfileEditor.tsx
- src/crosshook-native/src/components/ProfileFormSections.tsx
- src/crosshook-native/src/components/LauncherExport.tsx
- src/crosshook-native/src/context/ProfileContext.tsx

**Instructions**

Files to Create

- src/crosshook-native/src/components/pages/ProfilesPage.tsx

Compose `ProfileFormSections` + `ProfileActions` + `LauncherExport` (as a subsection). Read profile state from `useProfileContext()`. Read preferences from `usePreferencesContext()` for `steamClientInstallPath` and `targetHomePath`.

Load proton installs (replicate the `useEffect` from ProfileEditor.tsx lines 317-354). Render a page header (eyebrow + title + description). Below the form sections, render `ProfileActions`. Below that, render `LauncherExport` as a collapsible subsection with the heading "Launcher Export" — only show when the effective launch method supports it (steam_applaunch or proton_run).

Include the delete confirmation overlay (ProfileEditor.tsx lines 532-565) since it's part of profile management.

#### Task 2.3: Create LaunchPage

Depends on [1.8]

**READ THESE BEFORE TASK**

- src/crosshook-native/src/components/LaunchPanel.tsx
- src/crosshook-native/src/hooks/useLaunchState.ts
- src/crosshook-native/src/context/ProfileContext.tsx

**Instructions**

Files to Create

- src/crosshook-native/src/components/pages/LaunchPage.tsx

Read profile state from `useProfileContext()`. Derive `launchMethod` and `launchRequest` from the profile (replicate the derivation from App.tsx lines 84-126, but without the install-context override). Render a page header and mount `LaunchPanel` with the derived props. The `context` prop is removed — LaunchPage always uses the `'default'` context.

#### Task 2.4: Create InstallPage

Depends on [1.8]

**READ THESE BEFORE TASK**

- src/crosshook-native/src/components/ProfileEditor.tsx
- src/crosshook-native/src/components/InstallGamePanel.tsx
- src/crosshook-native/src/components/ProfileReviewModal.tsx
- src/crosshook-native/src/types/profile-review.ts
- src/crosshook-native/src/types/install.ts

**Instructions**

Files to Create

- src/crosshook-native/src/components/pages/InstallPage.tsx

This is the most complex page shell (~200 lines). Absorb from `ProfileEditorView`:

1. **Review session state**: `profileReviewSession`, `reviewConfirmation`, `reviewConfirmationResolverRef` (lines 84-86)
2. **Review helper functions**: `updateProfileReviewSession`, `isProfileReviewSessionDirty`, `createProfileReviewSessionState` (lines 15-46), `resolveReviewConfirmation`, `requestReviewConfirmation` (lines 92-133)
3. **Review handlers**: `handleOpenProfileReview`, `handleCloseProfileReview`, `handleProfileReviewNameChange`, `handleProfileReviewUpdate`, `handleInstallActionConfirmation`, `handleSaveProfileReview` (lines 135-315)
4. **Review modal rendering**: The `<ProfileReviewModal>` JSX block (lines 464-530)

Scope `effectiveLaunchMethod` to `'proton_run'` locally (install always uses Proton). Load proton installs same as ProfilesPage. Render page header, `InstallGamePanel`, and the review modal.

The `persistProfileDraft` call on save (line 303) should navigate the user to the Profiles route after saving — accept an `onNavigate` prop or use a callback.

#### Task 2.5: Create CommunityPage and CompatibilityPage

Depends on [1.8]

**READ THESE BEFORE TASK**

- src/crosshook-native/src/components/CommunityBrowser.tsx
- src/crosshook-native/src/components/CompatibilityViewer.tsx
- src/crosshook-native/src/hooks/useCommunityProfiles.ts

**Instructions**

Files to Create

- src/crosshook-native/src/components/pages/CommunityPage.tsx
- src/crosshook-native/src/components/pages/CompatibilityPage.tsx

**CommunityPage**: Instantiate `useCommunityProfiles({ profilesDirectoryPath: DEFAULT_PROFILES_DIRECTORY })`. Render page header and `<CommunityBrowser state={communityState} />`.

**CompatibilityPage**: Accept community state as a prop or use shared context. Derive `compatibilityEntries` from `communityState.index.entries` (replicate App.tsx lines 160-171). Render page header and `<CompatibilityViewer entries={compatibilityEntries} ... />`.

Consider sharing community state between both pages via a lightweight `CommunityContext` or lifting the hook to App.tsx/a shared provider.

#### Task 2.6: Create SettingsPage

Depends on [1.8]

**READ THESE BEFORE TASK**

- src/crosshook-native/src/components/SettingsPanel.tsx
- src/crosshook-native/src/context/PreferencesContext.tsx

**Instructions**

Files to Create

- src/crosshook-native/src/components/pages/SettingsPage.tsx

Read from `usePreferencesContext()`. Translate context values to `SettingsPanel` props. Render page header and `<SettingsPanel ... />`. The `steamClientInstallPath` and `targetHomePath` can come from `useProfileContext()` (derived values) or `usePreferencesContext()` depending on where they were placed in Task 1.3/1.4.

#### Task 2.7: Remove LaunchPanel install-context branch

Depends on [1.8]

**READ THESE BEFORE TASK**

- src/crosshook-native/src/components/LaunchPanel.tsx

**Instructions**

Files to Modify

- src/crosshook-native/src/components/LaunchPanel.tsx

Remove the `context` prop from `LaunchPanelProps`. Remove the entire `if (isInstallContext)` block (lines 53-127, ~74 lines of install-specific JSX). Remove the `context` parameter from the function signature and the `isInstallContext` variable. The component now always renders the default launch view.

#### Task 2.8: Remove LauncherExport install-context branch

Depends on [1.8]

**READ THESE BEFORE TASK**

- src/crosshook-native/src/components/LauncherExport.tsx

**Instructions**

Files to Modify

- src/crosshook-native/src/components/LauncherExport.tsx

Remove the `context` prop from `LauncherExportProps`. Remove the entire `if (context === 'install')` block (lines 270-336, ~66 lines of install-review informational panel). Remove the `context` parameter from the function signature. The component now always renders the default export view.

#### Task 2.9: Wire ContentArea to page components

Depends on [2.2, 2.3, 2.4, 2.5, 2.6]

**READ THESE BEFORE TASK**

- src/crosshook-native/src/components/layout/ContentArea.tsx

**Instructions**

Files to Modify

- src/crosshook-native/src/components/layout/ContentArea.tsx

Replace the temporary direct-component renders with proper page shells. Each route renders its page component wrapped in a Radix `Tabs.Content`:

```typescript
case 'profiles': return <ProfilesPage />;
case 'launch': return <LaunchPage />;
case 'install': return <InstallPage onNavigate={onNavigate} />;
case 'community': return <CommunityPage />;
case 'compatibility': return <CompatibilityPage />;
case 'settings': return <SettingsPage />;
```

Remove all temporary imports of old components (ProfileEditorView, LaunchPanel, etc.) that were used as placeholders.

#### Task 2.10: Deprecate ProfileEditorView

Depends on [2.2, 2.4]

**READ THESE BEFORE TASK**

- src/crosshook-native/src/components/ProfileEditor.tsx

**Instructions**

Files to Modify

- src/crosshook-native/src/components/ProfileEditor.tsx

Remove `ProfileEditorView` (the export that managed sub-tabs, review modal, and delete confirmation). Remove the standalone `ProfileEditor` component (lines 570-586, unused). Keep only imports/exports that other files still need (check for remaining references). If nothing remains, the file can be deleted entirely. Clean up any orphaned imports across the codebase (grep for `ProfileEditor` imports).

### Phase 3: Polish and Cleanup (CSS Migration + Gamepad + Enhancements)

#### Task 3.1: Migrate ConsoleView inline styles to CSS classes

Depends on [2.9]

**READ THESE BEFORE TASK**

- src/crosshook-native/src/components/ConsoleView.tsx
- src/crosshook-native/src/styles/theme.css

**Instructions**

Files to Modify

- src/crosshook-native/src/components/ConsoleView.tsx

Replace all inline `style={{...}}` with the existing `.crosshook-console__*` CSS classes already defined in `theme.css` (lines 416-479) but currently unused. Remove the `buttonStyle` constant (lines 258-268). Replace the outer `<section>` inline styles with `.crosshook-console` class. Replace header inline styles with `.crosshook-console__header`. Replace body inline styles with `.crosshook-console__body`. Replace line items with `.crosshook-console__line`. Verify visual parity after migration.

This is the lowest-risk migration — the CSS classes already exist and match the inline styles.

#### Task 3.2: Migrate LaunchPanel inline styles to CSS classes

Depends on [2.7]

**READ THESE BEFORE TASK**

- src/crosshook-native/src/components/LaunchPanel.tsx
- src/crosshook-native/src/styles/theme.css

**Instructions**

Files to Modify

- src/crosshook-native/src/components/LaunchPanel.tsx
- src/crosshook-native/src/styles/theme.css

Remove the `panelStyles.card` object. Replace all inline `style={{...}}` props with CSS classes. Create new classes in `theme.css`:

- `.crosshook-launch-panel`: The card container (replaces panelStyles.card)
- `.crosshook-launch-panel__eyebrow`: Method label
- `.crosshook-launch-panel__status`: Status chip
- `.crosshook-launch-panel__info`: Inner info card
- `.crosshook-launch-panel__actions`: Button row
- `.crosshook-launch-panel__indicator`: Dot + label status row

Map hardcoded colors to variables: `#7bb0ff` -> `var(--crosshook-color-accent-strong)`, `#9fb1d6` -> `var(--crosshook-color-text-muted)`, `#5e77ff` -> `var(--crosshook-color-accent)`.

#### Task 3.3: Migrate LauncherExport inline styles to CSS classes

Depends on [2.8]

**READ THESE BEFORE TASK**

- src/crosshook-native/src/components/LauncherExport.tsx
- src/crosshook-native/src/styles/theme.css

**Instructions**

Files to Modify

- src/crosshook-native/src/components/LauncherExport.tsx
- src/crosshook-native/src/styles/theme.css

Remove all 8 style constants (lines 32-110: `panelStyle`, `sectionStyle`, `labelStyle`, `inputStyle`, `buttonStyle`, `subtleButtonStyle`, `deleteButtonStyle`, `helperStyle`, `infoCalloutStyle`). Create corresponding CSS classes in `theme.css`:

- `.crosshook-export-panel`: Main card
- `.crosshook-export-section`: Inner section
- `.crosshook-export-label`: Field label
- `.crosshook-export-callout`: Info callout
- `.crosshook-export-result`: Export result card
- `.crosshook-export-status`: Launcher status indicator
- `.crosshook-button--danger`: Formalize the delete button style (also used informally in SettingsPanel)

#### Task 3.4: Migrate SettingsPanel inline styles to CSS classes

Depends on [2.9]

**READ THESE BEFORE TASK**

- src/crosshook-native/src/components/SettingsPanel.tsx
- src/crosshook-native/src/styles/theme.css

**Instructions**

Files to Modify

- src/crosshook-native/src/components/SettingsPanel.tsx
- src/crosshook-native/src/styles/theme.css

Remove the `layoutStyles` record (lines 27-115). Replace with CSS classes:

- `.crosshook-settings-grid`: Two-column grid layout
- `.crosshook-settings-section`: Section group
- `.crosshook-settings-field-row`: Form field stack
- `.crosshook-settings-checkbox-row`: Checkbox + label row
- `.crosshook-recent-list`: Recent files list
- `.crosshook-recent-item`: Recent file entry

#### Task 3.5: Migrate CommunityBrowser and CompatibilityViewer inline styles

Depends on [2.9]

**READ THESE BEFORE TASK**

- src/crosshook-native/src/components/CommunityBrowser.tsx
- src/crosshook-native/src/components/CompatibilityViewer.tsx
- src/crosshook-native/src/styles/theme.css

**Instructions**

Files to Modify

- src/crosshook-native/src/components/CommunityBrowser.tsx
- src/crosshook-native/src/components/CompatibilityViewer.tsx
- src/crosshook-native/src/styles/theme.css

Remove `panelStyles` and `ratingStyles` records from CommunityBrowser. Remove inline style objects from CompatibilityViewer. Create CSS classes:

- `.crosshook-community-toolbar`: Search/filter toolbar
- `.crosshook-community-grid`: Profile card grid
- `.crosshook-community-card`: Profile card
- `.crosshook-community-badge--platinum`, `--working`, `--partial`, `--broken`, `--unknown`: Rating badges
- `.crosshook-compatibility-*`: CompatibilityViewer classes

#### Task 3.6: Migrate AutoPopulate inline styles

Depends on [2.9]

**READ THESE BEFORE TASK**

- src/crosshook-native/src/components/AutoPopulate.tsx
- src/crosshook-native/src/styles/theme.css

**Instructions**

Files to Modify

- src/crosshook-native/src/components/AutoPopulate.tsx
- src/crosshook-native/src/styles/theme.css

Remove all inline style objects from AutoPopulate.tsx. Create `.crosshook-auto-populate-*` CSS classes using `--crosshook-*` variables.

#### Task 3.7: Gamepad navigation adaptation

Depends on [2.9]

**READ THESE BEFORE TASK**

- src/crosshook-native/src/hooks/useGamepadNav.ts
- src/crosshook-native/src/styles/focus.css
- docs/plans/ui-enhancements/research-ux.md

**Instructions**

Files to Modify

- src/crosshook-native/src/hooks/useGamepadNav.ts

This is the highest-risk task. Adapt `useGamepadNav` for the new sidebar + content layout:

1. **Zone-based navigation**: Add support for focus zones via `data-crosshook-focus-zone="sidebar"` and `data-crosshook-focus-zone="content"` attributes. D-pad Left moves focus to sidebar zone. D-pad Right moves focus to content zone. D-pad Up/Down navigates within the current zone.
2. **Focus memory per zone**: When switching zones, restore the last focused element in the target zone.
3. **LB/RB bumper cycling**: Map L1/R1 gamepad buttons to cycle through sidebar items (previous/next view).
4. **Extended back handler**: When no modal is open and focus is in content zone, D-pad Left or B button returns focus to sidebar. When in sidebar, B button does nothing (or cycles to previous view).
5. **Auto-focus on route change**: When route changes, move focus to the first focusable element in the content zone.

Test thoroughly with a gamepad or by simulating controller input.

#### Task 3.8: Console drawer auto-expand on launch

Depends on [2.9]

**READ THESE BEFORE TASK**

- src/crosshook-native/src/components/layout/ConsoleDrawer.tsx
- src/crosshook-native/src/hooks/useLaunchState.ts

**Instructions**

Files to Modify

- src/crosshook-native/src/components/layout/ConsoleDrawer.tsx

Wire `useLaunchState` phase changes to auto-expand the console drawer. When `phase` transitions from `Idle` to any launching state, expand the drawer. When `phase` returns to `Idle`, optionally auto-collapse after a delay. Use the `react-resizable-panels` `panelRef.expand()` API.

Note: The drawer needs access to launch phase state. Either consume `useLaunchState` directly (it needs `profileId`, `method`, `request` — which come from ProfileContext), or listen for `launch-log` events as a simpler trigger (if a log event arrives and the drawer is collapsed, expand it).

#### Task 3.9: Responsive sidebar collapse

Depends on [1.5, 1.8]

**READ THESE BEFORE TASK**

- src/crosshook-native/src/components/layout/Sidebar.tsx
- src/crosshook-native/src/styles/sidebar.css

**Instructions**

Files to Modify

- src/crosshook-native/src/components/layout/Sidebar.tsx
- src/crosshook-native/src/styles/sidebar.css

Implement auto-collapse to 56px icon-only rail below 900px viewport width. Add a `collapsed` state driven by `matchMedia('(max-width: 900px)')`. When collapsed, hide label text and show only icon placeholders (inline SVG or emoji for now). If `@radix-ui/react-tooltip` is installed, wrap collapsed items in `Tooltip` components for hover/focus labels.

#### Task 3.10: Controller prompt bar

Depends on [3.7]

**READ THESE BEFORE TASK**

- src/crosshook-native/src/styles/focus.css
- src/crosshook-native/src/hooks/useGamepadNav.ts

**Instructions**

Files to Create

- src/crosshook-native/src/components/layout/ControllerPrompts.tsx

Files to Modify

- src/crosshook-native/src/styles/focus.css

Create a controller prompt bar component that renders at the bottom of the screen when `controllerMode` is `true`. Show context-sensitive button mappings: A: Select, B: Back, LB/RB: Switch View, Y: Quick Launch (optional). The CSS class `.crosshook-controller-prompts` already exists in `focus.css` but is unrendered — wire it up.

Mount this component in the App shell, conditionally rendered only when `gamepadNav.controllerMode` is `true`.

## Advice

- **T1.8 is the linchpin**: Every Phase 2 task depends on the App.tsx refactor. Keep it focused on structural changes only — no inline style migration, no new features. The smaller the diff, the faster the review.
- **ProfileEditor.tsx split is the hardest decomposition**: T2.4 (InstallPage) absorbs ~230 lines of review modal orchestration with dirty checks, confirmation promises, and session state. Plan for this to take 2-3x longer than other page extractions. Consider splitting T2.4 into two sub-tasks: (a) create InstallPage with InstallGamePanel mount, (b) move review modal logic separately.
- **CSS classes already exist for ConsoleView**: `theme.css` defines `.crosshook-console__*` classes (lines 416-479) that `ConsoleView.tsx` completely ignores. T3.1 just wires them up — start CSS migration here as it's zero-risk and proves the pattern.
- **`useProfile({ autoSelectFirstProfile: false })` matters**: App.tsx explicitly passes `false` while the hook defaults to `true`. The ProfileContext provider must preserve this. Getting it wrong would auto-select the first profile on load, which is not the intended behavior.
- **`deriveSteamClientInstallPath` lives in the wrong file**: It's exported from `ProfileFormSections.tsx` (a component) and imported by `App.tsx`. Move it to `utils/` during T1.3 to break this awkward import chain.
- **Modal z-index stacking**: ProfileReviewModal uses z-index 1200, delete overlay uses 1000. Sidebar should be ~100, console drawer ~50. No conflicts, but document the z-index stack in a comment in `layout.css`.
- **Feature branch per phase**: Create `feat/ui-phase-1`, `feat/ui-phase-2`, `feat/ui-phase-3`. Merge each to main before starting the next. This keeps PRs reviewable and ensures each phase works independently.
- **Gamepad testing is the final gate**: T3.7 requires manual testing with a controller or Steam Deck. Do not ship without verifying sidebar navigation, zone switching, bumper cycling, and modal focus trapping all work correctly with the new DOM structure.
- **Install review session persistence**: If users should be able to navigate away from InstallPage without losing their review draft, the session state needs to be in a context (not local to InstallPage). The feature-spec mentions showing an "unsaved draft" indicator in the sidebar — this requires session awareness at the App level.
- **Inline style migration is embarrassingly parallel**: T3.1-T3.6 each touch separate component files with no shared dependencies. Assign all 6 to parallel agents for maximum throughput.
