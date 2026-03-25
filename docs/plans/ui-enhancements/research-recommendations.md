# Recommendations: ui-enhancements

## Executive Summary

The CrossHook UI has grown organically around a three-tab horizontal layout where the "Main" tab carries too much weight -- profile editing, launch controls, launcher export, and a console log viewer all compete for the same viewport. The recommended path is a **vertical sidebar navigation** that promotes each major feature area (Profiles, Launch, Export, Community, Settings) into its own route-level view, paired with a persistent console drawer and systematic CSS cleanup that replaces the ~200 inline style objects scattered across components. The largest risk is breaking the gamepad navigation system during restructuring, since `useGamepadNav` relies on DOM-order traversal within a single `data-crosshook-focus-root` scope.

## Implementation Recommendations

### Recommended Approach

Adopt a **vertical sidebar + content area** shell layout. The sidebar holds navigation items (icon + label), a profile quick-switcher, and a persistent status indicator. The content area renders the active view. The console becomes a collapsible bottom drawer that persists across all views (it currently re-mounts and loses log history when switching tabs). This approach naturally maps to the five distinct feature domains already present in the codebase and eliminates the "Main tab does everything" problem.

The sidebar should support both full-width labels (desktop) and icon-only collapsed mode (Steam Deck / narrow viewports). This aligns with the existing responsive breakpoints at 1360px and 900px in `variables.css`.

### Technology Choices

| Component         | Recommendation                                                               | Rationale                                                                                                                                                                                                                                                                                                                       |
| ----------------- | ---------------------------------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Navigation        | State-based navigation (keep current pattern, no router library)             | The app is a Tauri single-window desktop app with 5 views; adding react-router adds bundle weight and complexity for no real benefit. The current `useState<AppTab>` pattern is simple and works. Extend the `AppTab` union type to cover the new views.                                                                        |
| CSS Architecture  | Migrate inline styles to CSS classes; one CSS module per component directory | The current codebase has ~200 inline `CSSProperties` objects (LaunchPanel: 15, LauncherExport: 12, ConsoleView: 11, AutoPopulate: 8, CompatibilityViewer: 7, etc.) and a monolithic 870-line `theme.css`. Splitting into per-component CSS modules eliminates style collisions and makes the dark theme tokens actually usable. |
| Layout System     | CSS Grid shell with `grid-template-areas`                                    | The sidebar+content+drawer layout maps cleanly to grid areas. The existing `crosshook-layout` class already uses CSS Grid, so this is a natural extension.                                                                                                                                                                      |
| State Management  | Keep current hook-based approach with context for shared state               | No need for Redux/Zustand. The existing `useProfile`, `useLaunchState`, `useCommunityProfiles` hooks are well-structured. The main change is moving shared state (settings, profile, gamepad) into React Context so child views can access them without prop drilling through `App.tsx`.                                        |
| Component Library | None (keep custom components)                                                | The existing component vocabulary (`crosshook-button`, `crosshook-input`, `crosshook-panel`, etc.) is sufficient. Adding a UI library would conflict with the custom dark theme and gamepad navigation.                                                                                                                         |

### Phasing Strategy

1. **Phase 1 - Foundation (CSS + Shell Layout)**: Extract inline styles into CSS classes. Build the sidebar shell layout component. Refactor `App.tsx` into a thin shell that renders `<Sidebar>` + `<ContentArea>` + `<ConsoleDrawer>`. Introduce React Context for profile and settings state. This phase is purely structural -- no feature changes.

2. **Phase 2 - View Separation**: Split the "Main" tab into three distinct views: **Profiles** (ProfileEditor + ProfileFormSections), **Launch** (LaunchPanel + launch controls), **Export** (LauncherExport). Move Community and Settings into their own sidebar-level views. Wire up the sidebar navigation to switch between views.

3. **Phase 3 - Polish and Enhancement**: Add quick-launch shortcuts, profile quick-switcher in sidebar, status bar with active session indicator, keyboard shortcuts for view switching (Ctrl+1 through Ctrl+5), and a persistent console drawer with expand/collapse animation.

### Quick Wins

- **Console as persistent drawer**: Move `ConsoleView` outside the tab content area so log history persists across navigation. Currently logs are lost when switching away from Main tab and back. Impact: Significant UX improvement, low-risk change.
- **Extract App.tsx derived state into a hook**: The `resolveLaunchMethod`, `deriveSteamClientInstallPath`, `deriveTargetHomePath`, heading text computation, and `launchRequest` memo can all move into a `useAppDerived` hook, cutting App.tsx from ~367 lines to ~150. Impact: Immediate readability improvement.
- **Replace the 3 hardcoded color values in LaunchPanel inline styles**: `#7bb0ff`, `#9fb1d6`, `#5e77ff` etc. all have CSS variable equivalents already defined in `variables.css` but are not used. Impact: Theme consistency with zero risk.
- **Add `crosshook-button--danger` class**: The delete confirm button in `LauncherExport.tsx` (lines 85-95) duplicates a pattern already used in `SettingsPanel.tsx` but with a raw style object. Impact: Consistency.

## Improvement Ideas

### Related Features

- **Profile Quick-Switcher in Sidebar**: A dropdown or searchable list in the sidebar that shows saved profiles and allows one-click switching. Currently requires navigating to the Profile tab and using the select dropdown. This becomes more valuable when profiles have their own dedicated view.
- **Quick Launch Button**: A persistent "Launch" button in the sidebar or status bar that launches the currently loaded profile without navigating to the Launch view. The `useLaunchState` hook already supports this -- it just needs a UI surface.
- **Breadcrumb Context Bar**: A thin bar below the sidebar header showing the current context: `Profiles > elden-ring > Steam Launch`. This replaces the dynamic `headingTitle` / `headingCopy` system in `App.tsx` (lines 128-158) which computes the heading text from launch method and editor tab state.
- **Activity/Status Bar**: A bottom bar showing: active session status (from `useLaunchState.phase`), controller mode indicator (from `useGamepadNav.controllerMode`), and last profile name. This consolidates the status chips currently scattered in the header.

### Future Enhancements

- **Keyboard shortcuts for view switching**: `Ctrl+1` through `Ctrl+5` to switch sidebar views. The `useGamepadNav` hook already intercepts keyboard events at the document level, so adding shortcuts is a matter of extending the keydown handler.
- **Console as slide-up drawer with resize handle**: Instead of the current inline block with fixed `minHeight: 280px` and `maxHeight: 52vh`, make it a resizable drawer panel that can be dragged to adjust height. The collapsed state already exists (`useState<boolean>(false)` in ConsoleView).
- **Search across all views**: A global search input in the sidebar that filters profiles, community entries, and settings. Currently search only exists within CommunityBrowser (the `query` state on line 287).
- **Recently used profiles list**: Show the last 5 used profiles as quick-access items in the sidebar. The `settings.last_used_profile` only tracks one; extend `RecentFilesData` or `AppSettingsData` to track a list.
- **Drag-and-drop profile import**: Allow dragging a `.toml` profile file onto the app window to import it. Tauri v2 supports file drop events.
- **Collapsible sidebar for Steam Deck**: Auto-collapse to icon-only mode when viewport matches the Steam Deck resolution (1280x800) as detected by `isSteamDeckRuntime()` in `useGamepadNav.ts`.

## Risk Assessment

### Technical Risks

| Risk                                                 | Likelihood | Impact | Mitigation                                                                                                                                                                                                                                                                                                                                                                                                                                                      |
| ---------------------------------------------------- | ---------- | ------ | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Gamepad navigation breaks when DOM structure changes | High       | High   | The `useGamepadNav` hook traverses focusable elements in DOM order via `FOCUSABLE_SELECTOR`. Changing the layout (sidebar + content) changes the DOM tree and traversal order. **Mitigation**: Keep `data-crosshook-focus-root` scoping intact, test gamepad navigation after every structural change. The modal focus scoping (`MODAL_FOCUS_ROOT_SELECTOR`) must remain functional.                                                                            |
| CSS migration introduces visual regressions          | Medium     | Medium | Inline styles have higher specificity than class-based styles. Moving to CSS classes may reveal hidden specificity conflicts with `theme.css`. **Mitigation**: Migrate one component at a time with visual comparison. Start with components that already use CSS classes (SettingsPanel, CommunityBrowser) before tackling the fully-inline ones (LaunchPanel, ConsoleView, LauncherExport).                                                                   |
| Profile review modal behavior with new layout        | Medium     | High   | The `ProfileReviewModal` uses portal-based rendering (`createPortal`) and manages its own focus trap with inert sibling handling (lines 185-225 of ProfileReviewModal.tsx). Changing the DOM structure of the host app could break the inert handling. **Mitigation**: The modal already isolates itself via a portal host appended to `document.body`, so it should be layout-agnostic. Verify that `hiddenNodesRef` correctly inerts the new sidebar element. |
| State management complexity when splitting views     | Medium     | Medium | Currently `App.tsx` holds all top-level state and passes it down as props. Splitting into separate views means either (a) maintaining the prop drilling through a layout component, or (b) introducing React Context. **Mitigation**: Use context for shared state (profile, settings, gamepad), keep view-local state in hooks.                                                                                                                                |
| ConsoleView log persistence across view switches     | Low        | Medium | Moving ConsoleView to a persistent drawer means it must remain mounted when switching views. Currently it unmounts when leaving the Main tab. **Mitigation**: Lift ConsoleView to the shell level outside the view-switching conditional. The event listener (`listen('launch-log')`) will keep collecting logs regardless of view.                                                                                                                             |
| Performance with more complex layout                 | Low        | Low    | The sidebar + content + drawer layout adds a few more DOM nodes but removes the two-column grid within Main tab. Net DOM complexity is similar. The most expensive component is `CommunityBrowser` with its profile card grid, which is unchanged. **Mitigation**: Use `React.memo` on sidebar items and view containers to prevent unnecessary re-renders when switching views.                                                                                |

### Integration Challenges

- **`useGamepadNav` root ref scope**: The gamepad hook attaches to `rootRef` which is currently the `<main>` element wrapping the entire app (line 244 of App.tsx). With a sidebar layout, the navigation root needs to scope to either (a) the entire app (current behavior) or (b) just the content area. Option (a) means gamepad D-pad navigation will traverse sidebar items too, which may be desirable for Steam Deck. Option (b) requires a way to switch focus scope between sidebar and content.
- **`handleGamepadBack` modal close behavior**: The global back handler (line 63 of App.tsx) queries `data-crosshook-modal-close` buttons. This should continue to work since the modal renders via portal, but the sidebar adds a new navigation context that "back" could mean "go to previous view" rather than "close modal". Consider extending the back handler to support both cases.
- **`effectiveLaunchMethod` depends on `profileEditorTab`**: Lines 85-91 of App.tsx override the launch method to `proton_run` when on the Install tab. With views separated, this coupling between the profile editor's internal tab state and the launch method needs to be preserved, possibly via a shared context or by having the Install view explicitly set its own launch method.
- **`shouldShowLauncherExport` conditional**: Line 96-99 of App.tsx determines whether to show the LauncherExport panel based on the active editor tab and launch method. With separate views, the Export view is always shown (it is its own view), so this conditional becomes irrelevant.

## Alternative Approaches

### Option A: Vertical Sidebar Navigation

Replace the horizontal tab row with a persistent left sidebar containing navigation items for: Profiles, Launch, Export, Community, Settings. Each item opens its dedicated content view. Console is a persistent bottom drawer.

- **Pros**: Clean separation of concerns. Each feature gets full viewport width. Natural fit for Steam Deck where vertical space is limited (800px). Familiar pattern from apps like VS Code, Discord, Steam client. Sidebar can show profile quick-switcher and status indicators. Scales well as more features are added.
- **Cons**: Requires the most structural refactoring. Sidebar takes horizontal space on narrow viewports (mitigated by collapsible mode). The "two-column layout" within Main tab (ProfileEditor left, LaunchPanel right) is lost -- these become separate views.
- **Effort**: Medium-High. ~3-4 phases, largest change is the CSS migration and shell restructuring.

### Option B: Dashboard + Vertical Nav

Keep a top-level "Dashboard" home view that shows a summary card for each feature area (profile status, launch readiness, export status, community count). Clicking a card navigates to the detailed view. Vertical sidebar provides navigation between views.

- **Pros**: Dashboard provides at-a-glance status. Good for users who launch infrequently and want to see overall state. Vertical nav handles the detailed views.
- **Cons**: Extra view to maintain. The dashboard is an additional surface that needs to stay in sync with multiple data sources. Users who always go straight to "Launch" have an extra click. More complex than Option A for marginal benefit.
- **Effort**: High. Everything in Option A plus a dashboard component with summary cards and status aggregation.

### Option C: Wizard-based Flows

Replace the tab navigation with task-oriented wizard flows: "Set Up Profile" (step-by-step), "Launch Game" (guided flow), "Export Launcher" (wizard). Each flow guides the user through the required steps.

- **Pros**: Very beginner-friendly. Reduces cognitive load by showing only relevant fields at each step. Aligns with the existing Install Game flow which is already wizard-like.
- **Cons**: Frustrating for experienced users who know exactly what field they need to change. The Profile editor has too many conditional sections (steam_applaunch vs proton_run vs native) to fit cleanly into a wizard. Conflicts with the "power user" nature of CrossHook's target audience (people configuring Proton prefixes and trainer injection). Gamepad navigation within wizards needs careful step-by-step focus management.
- **Effort**: High. Requires rethinking every component as a wizard step. The ProfileFormSections component (695 lines) would need to be split into at least 4 wizard steps.

### Recommendation

**Option A: Vertical Sidebar Navigation** is the strongest choice. It provides the clearest separation of concerns, the most natural fit for the existing component architecture, and the best scaling path for future features. The "Main tab does everything" problem is solved by giving each feature area its own view with full viewport width. The sidebar pattern is well-understood on Steam Deck (the Steam client uses it) and supports both keyboard and gamepad navigation naturally.

Option B adds the dashboard surface without enough payoff -- CrossHook is a "configure once, launch often" tool, so a dashboard is rarely the first thing users want to see. Option C fundamentally conflicts with the power-user audience and the conditional complexity of the profile editor.

## Task Breakdown Preview

### Phase 1: Foundation (CSS Cleanup + Shell Layout)

**Task Group 1.1 -- CSS Migration** (can be parallelized across components)

- Extract LaunchPanel inline styles to `LaunchPanel.css` (15 style objects)
- Extract LauncherExport inline styles to `LauncherExport.css` (12 style objects)
- Extract ConsoleView inline styles to `ConsoleView.css` (11 style objects)
- Extract AutoPopulate inline styles to `AutoPopulate.css` (8 style objects)
- Extract CompatibilityViewer inline styles to `CompatibilityViewer.css` (7 style objects)
- Extract ProfileReviewModal inline styles (confirmationBackdropStyle, confirmationCardStyle)
- Audit and consolidate duplicate color values against `variables.css` tokens

**Task Group 1.2 -- Shell Layout** (sequential, depends on 1.1 being started)

- Create `AppShell.tsx` with sidebar + content area + console drawer grid layout
- Create `Sidebar.tsx` component with navigation items
- Create `ConsoleDrawer.tsx` wrapper around existing ConsoleView (persistent mount)
- Refactor `App.tsx` to render `<AppShell>` instead of the current inline layout

**Task Group 1.3 -- State Context** (can parallel with 1.2)

- Create `ProfileContext` wrapping `useProfile` result
- Create `SettingsContext` wrapping settings/recentFiles state
- Create `AppContext` wrapping derived state (launchMethod, steamClientInstallPath, etc.)
- Remove prop drilling from App.tsx to child components

**Parallel opportunities**: All CSS migration tasks (1.1) can run in parallel. Shell layout (1.2) and state context (1.3) can run in parallel with each other but depend on at least some of 1.1 being done to avoid merge conflicts.

### Phase 2: Core Implementation (View Separation)

**Task Group 2.1 -- View Components** (sequential within group, parallel across views)

- Create `ProfilesView.tsx` containing ProfileEditorView + ProfileFormSections
- Create `LaunchView.tsx` containing LaunchPanel + launch status
- Create `ExportView.tsx` containing LauncherExport
- Adapt `CommunityView.tsx` from existing CommunityBrowser + CompatibilityViewer
- Adapt `SettingsView.tsx` from existing SettingsPanel

**Task Group 2.2 -- Navigation Wiring**

- Extend `AppTab` type to `'profiles' | 'launch' | 'export' | 'community' | 'settings'`
- Wire sidebar items to view switching
- Preserve the Install Game sub-flow within ProfilesView (it currently lives under a sub-tab)
- Update `effectiveLaunchMethod` logic to work without `profileEditorTab` coupling

**Dependencies**: Phase 2 depends on Phase 1 (shell layout must exist before views can be placed into it). Within Phase 2, view components (2.1) can be built in parallel, but navigation wiring (2.2) depends on all views existing.

### Phase 3: Integration, Testing, and Polish

**Task Group 3.1 -- Gamepad Navigation**

- Test and fix `useGamepadNav` with new sidebar+content DOM structure
- Add sidebar focus scope for gamepad D-pad (LB/RB to switch between sidebar and content)
- Verify modal focus trapping still works with new layout
- Test `handleGamepadBack` behavior with sidebar context

**Task Group 3.2 -- Polish Features**

- Profile quick-switcher dropdown in sidebar
- Keyboard shortcuts (Ctrl+1 through Ctrl+5) for view switching
- Status bar with session indicator and controller mode
- Console drawer resize handle and animation
- Responsive sidebar collapse at 900px breakpoint

**Task Group 3.3 -- Regression Testing**

- Verify all Tauri IPC commands still work (profile CRUD, launch, export, community)
- Test the full Install Game flow with profile review modal
- Test launcher export with delete/status/stale detection
- Test auto-populate Steam flow
- Verify responsive behavior at 1360px and 900px breakpoints

### Estimated Complexity

- **Total tasks**: ~25-30 discrete tasks across 3 phases
- **Critical path**: Phase 1.2 (Shell Layout) -> Phase 2.2 (Navigation Wiring) -> Phase 3.1 (Gamepad Navigation). Everything else can run in parallel around this path.
- **Estimated effort**: Phase 1 is the largest (CSS migration is tedious but parallelizable). Phase 2 is moderate (mostly moving existing components into new wrappers). Phase 3 is the most uncertain (gamepad testing requires manual verification on Steam Deck or with a controller).
- **Lines of code affected**: ~3,000-4,000 lines touched across CSS and component files. Net new code is estimated at ~800-1,200 lines (shell, sidebar, contexts, new CSS classes), with ~1,500-2,000 lines of inline styles removed.

## Key Decisions Needed

- **Sidebar width and collapse behavior**: Fixed width (240px) with collapse to icon-only (56px) at breakpoint, or user-resizable? The Steam Deck viewport is 1280x800 -- a 240px sidebar leaves 1040px for content, which is generous. Recommendation: fixed 240px, auto-collapse at 900px.
- **Console drawer default state**: Collapsed or expanded on app start? Currently the ConsoleView starts expanded with a "Collapse" button. For a persistent drawer, starting collapsed makes more sense -- users can expand it when they launch something. The console currently has `minHeight: 280px` which is quite tall for a drawer; consider defaulting to a 2-3 line preview.
- **View transition animation**: Fade, slide, or instant? Given this is a desktop app targeting Steam Deck, instant transitions are simplest and most performant. Adding CSS transitions later is trivial. Recommendation: start with instant, add 150ms fade in Phase 3 if desired.
- **Should Export be its own top-level view?**: The LauncherExport component is tightly coupled to the current profile (it reads `profile.steam.launcher.*`, `profile.trainer.path`, etc.). Making it a separate view means it needs access to the profile context. An alternative is to keep Export as a section within the Profiles view. Recommendation: separate view with profile context -- this gives it room to grow (e.g., showing all exported launchers, not just the current profile's).

## Open Questions

- What are the desired sidebar navigation items and their ordering? Suggested: Profiles, Launch, Export, Community, Settings (matching the current feature importance hierarchy).
- Should the sidebar be always visible on Steam Deck, or should it auto-hide and be toggled via a controller button (e.g., the Steam/Guide button)?
- Is there a preference for icon style (outlined, filled, or a specific icon set)? The app currently has no icons for navigation. Inline SVG icons would avoid adding a dependency.
- Should the profile review modal flow be preserved as-is, or should it be reconsidered as part of the UI overhaul? It is the most complex interaction in the app (nested confirmation dialogs, dirty state tracking, promise-based resolution).
- What is the target timeline? The phasing strategy assumes incremental delivery, but if there is a release milestone, certain phases could be compressed or expanded.

## Relevant Files

- `src/crosshook-native/src/App.tsx`: Root shell, all top-level state, tab navigation (367 lines -- primary refactoring target)
- `src/crosshook-native/src/components/ProfileEditor.tsx`: Profile editor with review modal orchestration (588 lines)
- `src/crosshook-native/src/components/ProfileFormSections.tsx`: Conditional form rendering by launch method (695 lines)
- `src/crosshook-native/src/components/LaunchPanel.tsx`: Launch controls with heavy inline styles (257 lines)
- `src/crosshook-native/src/components/LauncherExport.tsx`: Export with delete/status lifecycle (655 lines)
- `src/crosshook-native/src/components/ConsoleView.tsx`: Log stream with inline styles (270 lines)
- `src/crosshook-native/src/components/SettingsPanel.tsx`: Settings sub-sections, partially uses CSS classes (470 lines)
- `src/crosshook-native/src/components/CommunityBrowser.tsx`: Community profiles with panelStyles object (612 lines)
- `src/crosshook-native/src/components/CompatibilityViewer.tsx`: Compatibility data with inline styles (383 lines)
- `src/crosshook-native/src/components/AutoPopulate.tsx`: Steam auto-populate with inline styles (320 lines)
- `src/crosshook-native/src/components/InstallGamePanel.tsx`: Install wizard flow (547 lines)
- `src/crosshook-native/src/components/ProfileReviewModal.tsx`: Portal-based modal with focus trap (457 lines)
- `src/crosshook-native/src/hooks/useGamepadNav.ts`: Gamepad/keyboard navigation (473 lines -- highest risk during restructure)
- `src/crosshook-native/src/hooks/useProfile.ts`: Profile CRUD state management (479 lines)
- `src/crosshook-native/src/hooks/useLaunchState.ts`: Launch process state machine (244 lines)
- `src/crosshook-native/src/hooks/useCommunityProfiles.ts`: Community tap state management
- `src/crosshook-native/src/styles/theme.css`: Monolithic stylesheet (870 lines -- split target)
- `src/crosshook-native/src/styles/variables.css`: CSS custom properties (48 lines -- well-structured, extend for sidebar)
- `src/crosshook-native/src/styles/focus.css`: Focus/controller navigation styles (108 lines -- extend for sidebar focus)
