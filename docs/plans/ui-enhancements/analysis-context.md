# Context Analysis: ui-enhancements

## Executive Summary

Replace CrossHook's 3-tab horizontal layout (where a monolithic Main tab crams profile editing, launch controls, export, and console into a two-column 1280x800 view) with a vertical sidebar navigation containing 6 views (Profiles, Launch, Install, Browse, Compatibility, Settings), single-purpose content areas, and a persistent bottom console drawer. The implementation uses `@radix-ui/react-tabs` for accessible vertical navigation, `react-resizable-panels` for the sidebar/content split, and two React Contexts (`ProfileContext`, `PreferencesContext`) to replace prop-drilling through the current 369-line god component `App.tsx`.

## Architecture Context

- **System Structure**: Tauri v2 (Rust backend) + React 18 + TypeScript frontend. All backend ops go through `invoke()` IPC. State lives in custom hooks (`useProfile`, `useLaunchState`, `useInstallGame`, `useCommunityProfiles`, `useGamepadNav`). Styling split between `theme.css` BEM classes and ~200 inline `CSSProperties` objects scattered across components.
- **Data Flow**: `App.tsx` currently owns all top-level state, derives `effectiveLaunchMethod` and `launchRequest` via `useMemo`, and props-drills everything down. The restructure lifts profile state into `ProfileContext` and settings/paths into `PreferencesContext`, letting each page component consume what it needs directly. Route switching is a simple `useState<AppRoute>` -- no router library.
- **Integration Points**: (1) `ProfileContext` consumed by Profiles, Launch, Install pages. (2) `PreferencesContext` consumed by Settings, Export/Profiles pages. (3) `ConsoleDrawer` mounts at shell level outside route switching -- fixes the current bug where logs are lost on tab change. (4) `useGamepadNav` stays at App root, `rootRef` attaches to new layout wrapper. (5) `ProfileReviewModal` portal to `document.body` is layout-agnostic but sidebar must be inerted when modal opens.

## Critical Files Reference

- `src/crosshook-native/src/App.tsx`: Primary refactoring target -- 369 lines shrinks to ~60. Owns all state, tab logic, heading derivation, `effectiveLaunchMethod` override.
- `src/crosshook-native/src/components/ProfileEditor.tsx`: 588 lines -- split into ProfilesPage + InstallPage. Manages sub-tab state, review modal orchestration, delete confirmation.
- `src/crosshook-native/src/components/ProfileFormSections.tsx`: 695 lines -- reused as-is in ProfilesPage. Conditional field rendering by launch method. Dual-use with `reviewMode` prop.
- `src/crosshook-native/src/hooks/useGamepadNav.ts`: 473 lines -- highest risk during restructure. DOM-order traversal of focusable elements. Capture-phase `preventDefault()` on arrow keys blocks Radix's bubble-phase handler (correct behavior, needs verification).
- `src/crosshook-native/src/components/ProfileReviewModal.tsx`: 457 lines -- portal-based, focus trap with `hiddenNodesRef` inert handling. Must verify sidebar element is correctly inerted.
- `src/crosshook-native/src/hooks/useProfile.ts`: 479 lines -- clean `UseProfileResult` interface suitable for direct context wrapping.
- `src/crosshook-native/src/components/LaunchPanel.tsx`: 257 lines -- remove `context === 'install'` branch (~70 lines), migrate ~200 lines of inline style objects to CSS.
- `src/crosshook-native/src/components/LauncherExport.tsx`: 655 lines -- remove install context branch, migrate ~100 lines of style constants.
- `src/crosshook-native/src/components/ConsoleView.tsx`: 270 lines -- has `.crosshook-console__*` CSS classes already defined in theme.css but unused. Migrate from inline styles to those classes.
- `src/crosshook-native/src/styles/theme.css`: 870 lines -- monolithic stylesheet. Extend with sidebar, layout, drawer rules.
- `src/crosshook-native/src/styles/variables.css`: 48 lines -- add `--crosshook-sidebar-width`, `--crosshook-sidebar-width-collapsed`, `--crosshook-console-drawer-height`.
- `src/crosshook-native/src/styles/focus.css`: 108 lines -- contains unused `.crosshook-controller-prompts` class ready for activation.
- `src/crosshook-native/src/components/InstallGamePanel.tsx`: 547 lines -- no structural changes, rendered by new InstallPage.
- `src/crosshook-native/src/components/SettingsPanel.tsx`: 470 lines -- switch from props to PreferencesContext.
- `src/crosshook-native/src/components/CommunityBrowser.tsx`: 612 lines -- self-contained via `useCommunityProfiles` hook. Inline style migration candidate.
- `src/crosshook-native/src-tauri/tauri.conf.json`: Window config (1280x800, dark theme).
- `src/crosshook-native/package.json`: Frontend dependencies -- add `@radix-ui/react-tabs`, `react-resizable-panels`, optionally `@radix-ui/react-tooltip`.

## Patterns to Follow

- **Hook-based state management**: Each domain has its own hook. New views consume hooks via React Context, not by creating new state. See `src/crosshook-native/src/hooks/useProfile.ts`.
- **Tauri IPC via invoke()**: No direct filesystem calls in the frontend. See `App.tsx` lines 176-182 for the `Promise.all([invoke(...)])` load pattern.
- **BEM-like CSS naming**: `crosshook-component`, `crosshook-component--modifier`, `crosshook-component__element`. All new CSS must follow this. See `src/crosshook-native/src/styles/theme.css`.
- **CSS custom properties**: All colors, spacing, radii reference `--crosshook-*` tokens. No hardcoded hex values in new code. See `src/crosshook-native/src/styles/variables.css`.
- **Modal focus trapping**: Portal to `document.body`, inert siblings via `hiddenNodesRef`, `data-crosshook-focus-root="modal"` for gamepad hook scoping. See `ProfileReviewModal.tsx`.
- **Gamepad navigation scope**: `useGamepadNav` attaches to `rootRef`, traverses focusable elements in DOM order. Modal override via `MODAL_FOCUS_ROOT_SELECTOR`. Arrow events use capture-phase `preventDefault()`.
- **Touch target minimum**: `--crosshook-touch-target-min: 48px` on all interactive elements -- non-negotiable for Steam Deck.
- **Radix data-attribute styling**: Use `[data-state='active']` and `[data-orientation='vertical']` CSS selectors instead of manual class toggling.

## Cross-Cutting Concerns

- **Gamepad navigation is the highest-risk area**: Changing DOM structure changes `useGamepadNav`'s traversal order. The hook must be tested after every structural change. Zone-based navigation (sidebar zone + content zone with D-pad Left/Right switching) is the recommended enhancement but requires non-trivial refactoring of the 473-line hook. Start with linear traversal working, add zones in Phase 3.
- **CSS specificity during migration**: Inline styles have higher specificity than classes. When migrating a component from inline to CSS, verify no visual regressions. Migrate one component at a time with visual comparison.
- **ProfileReviewModal inert handling**: The modal inerts sibling nodes of its portal host. With the new sidebar element in the DOM tree, verify it gets correctly inerted when the modal opens.
- **effectiveLaunchMethod coupling**: Currently computed in App.tsx and depends on `profileEditorTab` state. With separate views, this override scopes to InstallPage alone. The Launch page always uses the profile's actual method.
- **Console log persistence**: The `listen('launch-log')` subscription in ConsoleView must remain mounted at all times. Moving ConsoleView into a persistent drawer at the shell level (outside route switching) fixes the existing bug where logs are lost on tab switch.
- **Radix keyboard handler vs useGamepadNav**: Radix Tabs listens for arrow keys in bubble phase. The gamepad hook uses capture-phase `preventDefault()` which should block Radix's handler. Needs integration testing to confirm no double-navigation.

## Parallelization Opportunities

### Can run fully in parallel

- `ProfileContext.tsx` and `PreferencesContext.tsx` creation (no shared code)
- All CSS file creation (`layout.css`, `sidebar.css`, `console-drawer.css`) -- no conflicts
- All page shell components (`ProfilesPage`, `LaunchPage`, `InstallPage`, `CommunityPage`, `CompatibilityPage`, `SettingsPage`) once contexts and ContentArea exist
- Inline style migration across different components (LaunchPanel, LauncherExport, ConsoleView, etc.)

### Requires coordination

- `App.tsx` refactor depends on contexts + shell components being ready
- `ContentArea.tsx` depends on route type definition and at least stub page components
- Navigation wiring depends on all pages existing
- Gamepad testing depends on full layout being assembled
- `ProfileEditor.tsx` split into ProfilesPage + InstallPage is the most complex decomposition -- affects both pages

### Critical path

Phase 1 Shell (contexts + sidebar + content area + drawer) -> Phase 2 Navigation Wiring (pages + route switching) -> Phase 3 Gamepad Testing + Polish

## Implementation Constraints

### Technical Constraints

- **1280x800 viewport**: Sidebar at ~200px default leaves ~1080px for content. Console drawer defaults collapsed (24px toggle bar). No horizontal scrolling permitted.
- **Sidebar collapse**: Auto-collapse to 56px icon rail below 900px viewport width. Requires `@radix-ui/react-tooltip` for icon-only tooltips.
- **No router library**: `useState<AppRoute>` with 6 values. Route type: `'profiles' | 'launch' | 'install' | 'community' | 'compatibility' | 'settings'`.
- **Export merged into Profiles view**: Export is a subsection within ProfilesPage, not a separate sidebar item. Reduces sidebar from 7 to 6 items.
- **Zero new runtime dependencies beyond Radix + react-resizable-panels**: ~15-25 kB gzipped total addition. No state management libraries, no router, no animation libraries.
- **WebKitGTK on Linux**: CSS Grid, Flexbox, custom properties, transitions all supported. `backdrop-filter: blur()` works but should not be applied to scrollable/frequently-rerendered elements.
- **No frontend test framework**: Verification is manual + Rust tests for `crosshook-core`. Gamepad testing requires controller or Steam Deck hardware.

### Business Constraints

- **Profile must have executable path before save**: Hard validation in `useProfile.ts`.
- **Install context always forces proton_run**: Scope this to InstallPage.
- **Launcher export requires trainer + runtime paths**: Disable export button when paths are missing.
- **Profile delete cascades to launcher files**: Confirmation dialog must show associated launcher files.
- **Two-step launch flow**: steam_applaunch and proton_run are Launch Game -> Wait -> Launch Trainer. Native goes direct to SessionActive.
- **Console log stream must persist across views**: The `listen('launch-log')` subscription must remain mounted regardless of active view.

## Key Recommendations

### Phase Organization

1. **Phase 1 -- Foundation** (~10 tasks): Create contexts, build sidebar/content/drawer shell, wire route switching. Each route initially renders existing components unchanged. CSS files created. Variables added.
2. **Phase 2 -- View Separation** (~10 tasks): Split ProfileEditorView into ProfilesPage + InstallPage. Create remaining page shells. Remove install-context branches from LaunchPanel/LauncherExport. Remove `profileEditorTab` state from App.tsx.
3. **Phase 3 -- Polish** (~10-15 tasks): Migrate inline styles to CSS classes (one component per task). Test/fix gamepad navigation. Add LB/RB bumper cycling. Add controller prompt bar. Console drawer auto-expand on launch events. Responsive sidebar collapse.

### Task Breakdown Advice

- Keep the Phase 1 shell PR as a pure structural change with no feature modifications -- makes it reviewable.
- CSS migration tasks (Phase 3) are trivially parallelizable and low-risk. Good candidate for batch processing.
- The `ProfileEditor.tsx` split is the single most complex task. It eliminates sub-tab state, `onEditorTabChange`, `effectiveLaunchMethod` override, and conditional install context in sibling components. Plan for this to take 2-3x longer than other page extractions.
- Gamepad testing is the final gate. Do not ship Phase 2 without at least basic gamepad navigation verification.

### Dependency Management

- Contexts are leaf dependencies -- build first, nothing depends on them being absent.
- `ConsoleDrawer` is independent of routing -- can be built and wired immediately.
- Page shells are thin wrappers that can be stubbed quickly, then fleshed out incrementally.
- The `Sidebar` component depends on the `AppRoute` type definition and CSS -- build CSS first.

### Resolved Decisions (do not re-litigate)

1. Export is a subsection of Profiles, not a separate sidebar item.
2. Sidebar is user-resizable via react-resizable-panels.
3. Console drawer defaults collapsed, auto-expands on launch events.
4. 6 sidebar views: Profiles, Launch, Install, Browse, Compatibility, Settings.
5. Use Radix UI (`@radix-ui/react-tabs`) for vertical tab navigation.
6. Use `useState<AppRoute>` routing, not a router library.
7. Use React Context for shared state, not Zustand/Jotai.
8. Split ProfileEditorView into ProfilesPage + InstallPage.
9. CSS migration happens incrementally (layout first, components in follow-up PRs).

### Estimated Scope

- **Total tasks**: ~25-30 across 3 phases
- **Lines affected**: ~3,000-4,000 touched
- **Net new code**: ~800-1,200 lines
- **Inline styles removed**: ~1,500-2,000 lines
- **New files**: ~15 (3 CSS, 2 contexts, 4 layout components, 6 page components, 1 ProfileActions extraction)
- **Files deleted**: None (all existing components reused)
