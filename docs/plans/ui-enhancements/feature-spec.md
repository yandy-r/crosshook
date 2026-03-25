# Feature Spec: UI Enhancements

## Executive Summary

CrossHook's current horizontal tab layout with an overloaded Main tab crams profile editing, launch controls, launcher export, and a console log viewer into a single two-column view, creating a flat information hierarchy that forces users to context-switch between unrelated tasks on a 1280x800 Steam Deck screen. The recommended approach replaces the three horizontal tabs with a **vertical sidebar navigation** containing 5-6 icon+label items (Profiles, Launch, Install, Community, Export, Settings), each rendering a **single-purpose content area**. A **persistent collapsible console drawer** at the bottom retains log history across all view switches. The implementation uses `@radix-ui/react-tabs` for accessible vertical tab navigation and `react-resizable-panels` for the split-pane content layout, adding ~15-25 kB gzipped to the bundle. Shared state crosses the sidebar/content boundary via two React Contexts (`ProfileContext`, `PreferencesContext`), eliminating the current prop-drilling through `App.tsx`. The largest risk is breaking the `useGamepadNav` hook's linear DOM-order traversal during the layout restructure.

## External Dependencies

### Libraries and SDKs

| Library                   | Version | Purpose                                                                                                                             | Installation                          | Bundle Impact     |
| ------------------------- | ------- | ----------------------------------------------------------------------------------------------------------------------------------- | ------------------------------------- | ----------------- |
| `@radix-ui/react-tabs`    | Latest  | Headless vertical tab navigation with WAI-ARIA compliance, `orientation="vertical"`, `data-state`/`data-orientation` CSS attributes | `npm install @radix-ui/react-tabs`    | ~3-5 kB gzipped   |
| `react-resizable-panels`  | ^4.x    | Split-pane layout with collapsible panels, keyboard-accessible resize handles, touch-friendly hit targets, layout persistence       | `npm install react-resizable-panels`  | ~10-15 kB gzipped |
| `@radix-ui/react-tooltip` | Latest  | (Optional) Tooltips for icon-only sidebar items when collapsed                                                                      | `npm install @radix-ui/react-tooltip` | ~3-5 kB gzipped   |

### External Documentation

- [Radix Tabs](https://www.radix-ui.com/primitives/docs/components/tabs): Vertical orientation, `activationMode`, keyboard navigation
- [Radix Styling Guide](https://www.radix-ui.com/primitives/docs/guides/styling): CSS integration via `data-state`/`data-orientation` attributes
- [react-resizable-panels](https://react-resizable-panels.vercel.app/): Panel API, collapse behavior, layout persistence
- [NN/g Vertical Navigation](https://www.nngroup.com/articles/vertical-nav/): Research backing sidebar navigation pattern
- [NN/g Progressive Disclosure](https://www.nngroup.com/articles/progressive-disclosure/): Pattern for reducing form complexity

### Libraries Explicitly Rejected

| Library                | Reason                                                                        |
| ---------------------- | ----------------------------------------------------------------------------- |
| React Aria             | Too verbose for 2-3 primitives needed; hooks API adds boilerplate             |
| Ark UI                 | Younger ecosystem, Zag.js dependency adds bundle weight                       |
| Headless UI            | No vertical tab orientation support; Tailwind-coupled                         |
| shadcn/ui              | Requires Tailwind CSS; conflicts with existing `--crosshook-*` design tokens  |
| @tanstack/react-router | Unnecessary for 5-7 views in a desktop app; `useState`-based routing suffices |

## Business Requirements

### User Stories

**Primary User: Steam Deck Gamer**

- As a Steam Deck gamer, I want to load an existing profile and launch my game + trainer with minimal navigation so that I can start playing quickly from the couch
- As a Steam Deck gamer, I want launch status and console output visible while I wait for my game to boot so that I know whether to proceed with the trainer step
- As a Steam Deck gamer, I want gamepad-friendly navigation with large touch targets and clear focus indicators so that I do not need to reach for a keyboard
- As a Steam Deck gamer, I want to install a new Windows game through a guided flow without needing to understand which sub-tab or panel I need next

**Primary User: Desktop Linux Power User**

- As a power user, I want to edit profile details and see the effect on the launch panel in real time so that I can verify my configuration before launching
- As a power user, I want to export a launcher script and desktop entry from my current profile without leaving the profile context so that the export always reflects the latest edits
- As a power user, I want to manage all exported launchers in one place so that I do not have orphaned files on disk
- As a power user, I want the install game flow to feel like a distinct wizard, not a tab swap inside the profile editor, so that I understand the separate lifecycle

### Business Rules

1. **Profile Must Have Executable Path Before Save**: `game.executable_path` must be non-empty. Enforced in `useProfile.ts`.
2. **Launch Method Determines Visible Fields**: The selected `launch.method` determines which runtime fields appear in `ProfileFormSections`. Steam shows App ID, compatdata, proton path. Proton shows prefix, proton path, working directory. Native shows only working directory.
3. **Install Context Forces proton_run**: When in the Install view, the effective launch method is always `proton_run` and LaunchPanel request is `null`.
4. **Launcher Export Requires Trainer + Runtime Paths**: Export is disabled unless trainer_path, prefix_path, and proton_path are all non-empty.
5. **Profile Delete Cascades to Launcher Files**: Deleting a profile checks for associated launcher files and includes them in the confirmation dialog.
6. **Two-Step Launch Flow**: For steam_applaunch and proton_run, launching is: Launch Game -> Wait for trainer -> Launch Trainer. Native goes directly to SessionActive.
7. **Console Log Stream Must Persist Across Views**: The `listen('launch-log')` subscription must remain mounted regardless of active view. This fixes the current bug where logs are lost on tab switch.

### Edge Cases

| Scenario                                           | Expected Behavior                                                 | Notes                                                      |
| -------------------------------------------------- | ----------------------------------------------------------------- | ---------------------------------------------------------- |
| Install review session active, user navigates away | Session persists; show "unsaved draft" indicator in sidebar       | Currently dirty session prompts confirmation               |
| Community profile imported                         | Offer to navigate to Profiles view with imported profile selected | Currently disconnected -- user must manually switch tabs   |
| Stale launcher detected                            | Show warning badge on Export nav item                             | Currently only visible within LauncherExport panel         |
| Gamepad back button pressed in content area        | Return focus to sidebar                                           | Currently closes modals only                               |
| Console drawer expanded during launch              | Auto-expand on launch events, stay expanded                       | Currently console is inline and may be scrolled off screen |

### Success Criteria

- [ ] Every user workflow has a clear navigation path with no more than one level of nesting
- [ ] The primary workflow (load profile -> launch game -> launch trainer) requires at most 2 navigation actions from app open
- [ ] The install game flow is visually distinct from profile editing, with its own dedicated view
- [ ] Console output is accessible from any view where launch or install operations are active
- [ ] Gamepad navigation works correctly with the new layout, including zone-based sidebar/content switching
- [ ] The 1280x800 viewport shows all critical information without horizontal scrolling
- [ ] Community profile import offers to navigate to the imported profile in the editor
- [ ] The total number of top-level sidebar navigation items is 6 (Profiles, Launch, Install, Browse, Compatibility, Settings)

## Technical Specifications

### Architecture Overview

```
+-- Sidebar (resizable, ~200px) ----+---- Content Area (fluid) ----+
| [CrossHook]                       |                              |
|                                   |   Page Title                 |
| GAME                              |   Page description           |
|   > Profiles                      |                              |
|     Launch                        |   [ page content ]           |
|                                   |                              |
| SETUP                             |                              |
|     Install Game                  |                              |
|                                   |                              |
| COMMUNITY                         |                              |
|     Browse                        |                              |
|     Compatibility                 |                              |
|                                   |                              |
| ---- (spacer) ----                |                              |
|     Settings                      |                              |
|   [controller: On]               |                              |
|   [profile: elden-ring]          |                              |
+-----------------------------------+------------------------------+
|           ConsoleDrawer (collapsed by default, persistent)       |
+------------------------------------------------------------------+
```

Note: Export/Launcher management is a subsection within the Profiles view, not a separate sidebar item.

### Proposed Component Tree

```
App.tsx (slimmed: sidebar state, context providers, ~60 lines)
  +-- AppProviders (ProfileContext + PreferencesContext)
  |
  +-- <div.crosshook-app-layout>
      +-- Sidebar.tsx (persistent vertical nav, user-resizable via react-resizable-panels)
      |   +-- NavSection "Game"
      |   |   +-- NavItem "Profiles" -> route: profiles
      |   |   +-- NavItem "Launch" -> route: launch
      |   +-- NavSection "Setup"
      |   |   +-- NavItem "Install Game" -> route: install
      |   +-- NavSection "Community"
      |   |   +-- NavItem "Browse" -> route: community
      |   |   +-- NavItem "Compatibility" -> route: compatibility
      |   +-- Footer (Settings nav item, status indicators)
      |
      +-- ContentArea.tsx (route dispatcher)
      |   +-- profiles -> ProfilesPage (ProfileFormSections + ProfileActions + LauncherExport subsection)
      |   +-- launch -> LaunchPage (LaunchPanel via ProfileContext)
      |   +-- install -> InstallPage (InstallGamePanel + ProfileReviewModal)
      |   +-- community -> CommunityPage (CommunityBrowser)
      |   +-- compatibility -> CompatibilityPage (CompatibilityViewer)
      |   +-- settings -> SettingsPage (SettingsPanel)
      |
      +-- ConsoleDrawer.tsx (persistent, collapsible)
          +-- ConsoleView (existing logic, new container)
```

### State Management

**ProfileContext** -- Wraps `useProfile()` result. Consumed by Profiles, Launch, Export, and Install pages. The hook already returns a clean interface (`UseProfileResult`) suitable for context wrapping.

**PreferencesContext** -- Wraps `settings`, `recentFiles`, `steamClientInstallPath`, `targetHomePath`, and their mutators. Currently these are `App.tsx` state variables.

**Route State** -- Simple `useState<AppRoute>` in `App.tsx`:

```typescript
type AppRoute = 'profiles' | 'launch' | 'install' | 'community' | 'compatibility' | 'settings';
```

No router library needed. `ContentArea` switches on this value.

### System Integration

#### Files to Create

All paths relative to `src/crosshook-native/src/`:

| File                                     | Purpose                                                          | Est. Lines |
| ---------------------------------------- | ---------------------------------------------------------------- | ---------- |
| `context/ProfileContext.tsx`             | React context wrapping `UseProfileResult`                        | ~40        |
| `context/PreferencesContext.tsx`         | React context wrapping settings, steam paths                     | ~60        |
| `components/layout/Sidebar.tsx`          | Vertical navigation with NavSection/NavItem                      | ~150       |
| `components/layout/ContentArea.tsx`      | Route-to-page dispatcher                                         | ~60        |
| `components/layout/ConsoleDrawer.tsx`    | Bottom drawer wrapping ConsoleView                               | ~120       |
| `components/pages/ProfilesPage.tsx`      | ProfileFormSections + ProfileActions + LauncherExport subsection | ~120       |
| `components/pages/LaunchPage.tsx`        | LaunchPanel from context                                         | ~50        |
| `components/pages/InstallPage.tsx`       | InstallGamePanel + review modal                                  | ~200       |
| `components/pages/CommunityPage.tsx`     | CommunityBrowser wrapper                                         | ~30        |
| `components/pages/CompatibilityPage.tsx` | CompatibilityViewer wrapper                                      | ~30        |
| `components/pages/SettingsPage.tsx`      | SettingsPanel wrapper                                            | ~30        |
| `components/ProfileActions.tsx`          | Extracted Save/Delete/dirty bar                                  | ~60        |
| `styles/sidebar.css`                     | Sidebar, nav section, nav item styles                            | ~150       |
| `styles/console-drawer.css`              | Drawer container, toggle bar                                     | ~80        |
| `styles/layout.css`                      | App layout grid                                                  | ~50        |

#### Files to Modify

| File                   | Change                                                                                                                                                     | Impact                         |
| ---------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------------- | ------------------------------ |
| `App.tsx`              | Major refactor: remove tab state, profile/settings state, heading logic. Replace with `<AppProviders>` + `<Sidebar>` + `<ContentArea>` + `<ConsoleDrawer>` | Shrinks from ~367 to ~60 lines |
| `ProfileEditor.tsx`    | Split: profile editing -> ProfilesPage, install/review -> InstallPage                                                                                      | Eliminates sub-tab state       |
| `LaunchPanel.tsx`      | Remove `context === 'install'` branch, migrate inline styles to CSS                                                                                        | Removes ~70 lines              |
| `LauncherExport.tsx`   | Remove install context branch, migrate ~100 lines of style constants to CSS                                                                                | Major cleanup                  |
| `ConsoleView.tsx`      | Remove outer section wrapper (ConsoleDrawer provides container), migrate inline styles to existing `.crosshook-console__*` CSS classes                     | Cleanup                        |
| `SettingsPanel.tsx`    | Read from PreferencesContext instead of props                                                                                                              | Minor                          |
| `styles/theme.css`     | Add sidebar layout rules, migrate inline style classes                                                                                                     | Extend                         |
| `styles/variables.css` | Add `--crosshook-sidebar-width`, `--crosshook-console-drawer-height`                                                                                       | 3-4 new vars                   |

## UX Considerations

### User Workflows

#### Primary Workflow: Launch Game with Trainer

1. **Open App** -> Profile auto-loads if configured; sidebar shows active profile name
2. **Navigate to Launch** -> Click "Launch" in sidebar (or auto-land if profile loaded)
3. **Launch Game** -> Single button starts orchestrated flow; console drawer auto-expands
4. **Launch Trainer** -> Second button appears after game reaches main menu
5. **Monitor** -> Console drawer streams real-time logs throughout

Key insight: Most common flow touches only 2 views (Profiles, Launch) + console drawer.

#### Install Game Workflow

1. **Navigate to Install** -> Dedicated sidebar item (no more sub-tab hunting)
2. **Configure** -> Profile name, installer path, Proton version, prefix
3. **Run Installer** -> Console drawer shows progress
4. **Review** -> ProfileReviewModal opens automatically with generated profile
5. **Save** -> Profile saved, sidebar navigates to Profiles view

### UI Patterns

| Component      | Pattern                         | Notes                                                                     |
| -------------- | ------------------------------- | ------------------------------------------------------------------------- |
| Navigation     | Vertical sidebar (icon+label)   | Replaces horizontal tabs. Collapsible to icon-only at 900px.              |
| Content area   | Single-purpose views            | One functional domain per view. No two-column splits.                     |
| Console        | Bottom drawer (VS Code pattern) | Persistent, collapsible, auto-expands on launch events                    |
| Profile editor | Progressive disclosure          | Essential fields visible, Steam/Proton config in expandable sections      |
| Gamepad nav    | Zone-based (sidebar + content)  | D-pad Left/Right switches zones, Up/Down within zone. LB/RB cycles views. |

### Accessibility Requirements

- **Sidebar**: `<nav aria-label="CrossHook navigation">` with `aria-current="page"` on active item
- **Radix Tabs**: Automatic `role="tablist"`, `role="tab"`, `role="tabpanel"`, `aria-selected`
- **Console drawer**: `role="region"` with `aria-label`, `aria-expanded` toggle, `aria-hidden` when collapsed
- **Focus on route change**: Auto-focus page heading element for screen reader announcements
- **Touch targets**: All interactive elements maintain 48px minimum (`--crosshook-touch-target-min`)
- **Reduced motion**: Existing `prefers-reduced-motion` rules preserved

### Responsive Behavior

| Breakpoint | Sidebar                        | Content            | Console              |
| ---------- | ------------------------------ | ------------------ | -------------------- |
| > 1360px   | ~200px resizable (default)     | Max 960px centered | Collapsed toggle bar |
| 900-1360px | ~200px resizable               | Fluid fill         | Collapsed toggle bar |
| < 900px    | 56px icon rail (auto-collapse) | Fluid minus rail   | Collapsed toggle bar |

At 1280x800 (Steam Deck): Sidebar at ~200px default leaves ~1080px for content. User can resize smaller/larger via drag handle. Console drawer defaults to collapsed thin toggle bar; auto-expands on launch events.

### Gamepad/Controller UX

- **Zone-based navigation**: Sidebar zone + content zone. D-pad Left/Right switches zones.
- **Bumper cycling**: LB/RB cycles through sidebar views (Steam Big Picture pattern)
- **Focus memory**: Each zone remembers last focused element
- **Controller prompt bar**: Bottom bar showing context-sensitive button mappings (A: Select, B: Back, LB/RB: Switch View). CSS class `.crosshook-controller-prompts` exists but is currently unrendered.
- **Auto-focus on view switch**: First focusable element in content area receives focus automatically

### Competitive Analysis Summary

| Feature           | CrossHook Current    | Heroic              | Steam BPM         | Recommendation      |
| ----------------- | -------------------- | ------------------- | ----------------- | ------------------- |
| Navigation        | 3 horizontal tabs    | Collapsible sidebar | Bumper tabs       | Vertical sidebar    |
| Gamepad           | Linear traversal     | Improving           | Spatial + bumper  | Zone-based + bumper |
| Console/logs      | Inline (loses state) | Download manager    | None              | Persistent drawer   |
| Config management | Sub-tabs in Main     | Per-game page       | Properties dialog | Dedicated views     |

## Recommendations

### Implementation Approach

**Recommended Strategy**: Vertical sidebar navigation (Option A) with incremental migration. Build the shell layout first, then migrate existing components into new page shells, then clean up inline styles.

**Phasing:**

1. **Phase 1 - Foundation**: Create context providers, build sidebar/content/drawer shell, wire route switching. Each "route" initially renders existing components unchanged.
2. **Phase 2 - View Separation**: Split ProfileEditorView into ProfilesPage + InstallPage. Create page shells for Launch, Export, Community, Compatibility, Settings. Remove install-context branches from LaunchPanel/LauncherExport.
3. **Phase 3 - Polish**: Migrate inline styles to CSS classes. Add profile quick-switcher, keyboard shortcuts, controller prompt bar, console drawer resize handle. Test gamepad navigation.

### Technology Decisions

| Decision          | Recommendation                        | Rationale                                                                 |
| ----------------- | ------------------------------------- | ------------------------------------------------------------------------- |
| Routing           | `useState<AppRoute>`                  | 5-7 views; no URL routing needed in a Tauri desktop app                   |
| State sharing     | React Context                         | 2 shared state objects; existing hooks wrap cleanly in context            |
| Console placement | Bottom drawer                         | Must persist across routes; VS Code/IDE standard pattern                  |
| Navigation        | Vertical sidebar                      | Scales to 7+ items; natural D-pad mapping; matches Heroic/Steam           |
| CSS migration     | Layout first, components later        | Keeps restructure PR reviewable; follow-up PRs for inline style cleanup   |
| ProfileEditorView | Split into ProfilesPage + InstallPage | Eliminates sub-tab pattern, install context overrides, 568-line component |

### Quick Wins

- **Console as persistent drawer**: Move outside tab content so logs survive navigation (significant UX fix, low risk)
- **Extract App.tsx derived state into hooks**: Cut App.tsx from ~367 to ~150 lines
- **Replace hardcoded hex colors with CSS variables**: LaunchPanel/ConsoleView use `#7bb0ff`, `#9fb1d6` etc. that already have `--crosshook-*` equivalents
- **Define `.crosshook-button--danger` class**: Already used informally, needs formal CSS definition

### Future Enhancements

- Profile quick-switcher dropdown in sidebar
- Keyboard shortcuts (Ctrl+1 through Ctrl+5) for view switching
- Global search across profiles and community entries
- Recently launched profiles quick-access list
- Drag-and-drop `.toml` profile import via Tauri file drop events
- Collapsible sidebar auto-triggered by Steam Deck resolution detection

## Risk Assessment

### Technical Risks

| Risk                                                      | Likelihood | Impact | Mitigation                                                                                                                                                                               |
| --------------------------------------------------------- | ---------- | ------ | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Gamepad navigation breaks when DOM structure changes      | High       | High   | Keep `data-crosshook-focus-root` scoping intact. Test after every structural change. The `useGamepadNav` hook traverses DOM-order focusable elements -- sidebar changes traversal order. |
| CSS migration introduces visual regressions               | Medium     | Medium | Inline styles have higher specificity than classes. Migrate one component at a time with visual comparison. Start with components partially using CSS classes already.                   |
| ProfileReviewModal inert handling with new sidebar        | Medium     | High   | Modal uses portal to `document.body` and inerts sibling nodes via `hiddenNodesRef`. Verify sidebar element is correctly inerted when modal opens.                                        |
| State management complexity when splitting views          | Medium     | Medium | Use context for shared state (profile, preferences). Keep view-local state in page components.                                                                                           |
| Radix keyboard handler vs useGamepadNav double-navigation | Low        | Medium | Gamepad hook uses capture-phase `preventDefault()` which blocks Radix's bubble-phase handler. Needs integration testing.                                                                 |

### Integration Challenges

- **`effectiveLaunchMethod` depends on `profileEditorTab`**: Lines 85-91 of App.tsx override launch method to `proton_run` during install. With separate views, scope this override to InstallPage alone.
- **`shouldShowLauncherExport` conditional**: Becomes irrelevant when Export is its own view.
- **`handleGamepadBack` scope**: Currently closes modals. With sidebar, "back" could also mean "return to sidebar from content". Extend handler for both cases.

## Task Breakdown Preview

### Phase 1: Foundation

**Focus**: Shell layout, state contexts, route switching -- no feature changes

**Tasks**:

- Create `ProfileContext` and `PreferencesContext` wrapping existing hooks/state
- Create `Sidebar.tsx` with nav sections and items, CSS
- Create `ContentArea.tsx` route dispatcher
- Create `ConsoleDrawer.tsx` wrapping existing ConsoleView
- Create `layout.css`, `sidebar.css`, `console-drawer.css`
- Refactor `App.tsx` to render shell layout with context providers
- Add `--crosshook-sidebar-width` and related CSS variables

**Parallelization**: Context providers and CSS can run in parallel. Shell components depend on CSS.

### Phase 2: View Separation

**Focus**: Split Main tab into distinct views, remove sub-tab patterns

**Dependencies**: Phase 1 complete
**Tasks**:

- Create ProfilesPage composing ProfileFormSections + ProfileActions + LauncherExport subsection
- Create LaunchPage consuming ProfileContext
- Create InstallPage absorbing review modal logic from ProfileEditorView
- Create CommunityPage, CompatibilityPage, SettingsPage wrappers
- Remove install-context branches from LaunchPanel and LauncherExport
- Wire sidebar navigation to all views
- Remove `profileEditorTab` state and `effectiveLaunchMethod` install override from App.tsx

**Parallelization**: Page shells can be built in parallel. Navigation wiring depends on all pages existing.

### Phase 3: Polish and Cleanup

**Focus**: Inline style migration, gamepad refinement, enhancements

**Tasks**:

- Migrate ConsoleView inline styles to `.crosshook-console__*` CSS classes (already defined but unused)
- Migrate LaunchPanel inline styles (~200 lines) to CSS classes
- Migrate LauncherExport inline styles (~100 lines) to CSS classes
- Migrate SettingsPanel, CommunityBrowser, CompatibilityViewer inline styles
- Test and fix gamepad navigation with new layout
- Add LB/RB bumper cycling for sidebar views
- Add profile quick-switcher in sidebar
- Add controller prompt bar
- Add console drawer auto-expand on launch events
- Responsive sidebar collapse at 900px breakpoint

**Estimated Complexity**:

- **Total tasks**: ~25-30 discrete tasks across 3 phases
- **Critical path**: Phase 1 Shell -> Phase 2 Navigation Wiring -> Phase 3 Gamepad Testing
- **Lines affected**: ~3,000-4,000 touched. Net new: ~800-1,200. Inline styles removed: ~1,500-2,000.

## Decisions (Resolved)

1. **Export as subsection of Profiles** (decided): Export is tightly coupled to profile state and belongs within the Profiles view as a subsection/panel, not a separate sidebar item. This reduces sidebar items from 7 to 6: Profiles, Launch, Install, Browse, Compatibility, Settings.

2. **Sidebar is user-resizable** (decided): The sidebar uses `react-resizable-panels` with a reasonable default width (~200px) and a resize handle. Users can drag to adjust. Auto-collapses to icon-only rail at 900px breakpoint.

3. **Console drawer defaults to collapsed** (decided): Since the console is a persistent bottom drawer (not its own tab/view), it defaults to a collapsed thin toggle bar. Auto-expands on launch events when `useLaunchState.phase` changes to a launching state.

4. **6 sidebar views** (decided): Profiles (with Export subsection), Launch, Install, Browse, Compatibility, Settings. The original proposal of 7 views adjusted for Export merging into Profiles.

5. **Use Radix UI** (decided): `@radix-ui/react-tabs` provides WAI-ARIA-compliant vertical tabs with `orientation="vertical"`, `data-state`/`data-orientation` CSS attributes for styling, and built-in keyboard navigation -- all for ~3-5 kB gzipped. The accessibility and CSS integration benefits outweigh the minimal bundle cost.

## Research References

For detailed findings, see:

- [research-external.md](./research-external.md): Library evaluation (Radix, react-resizable-panels, alternatives)
- [research-business.md](./research-business.md): User workflows, component coupling, pain points
- [research-technical.md](./research-technical.md): Component tree, state management, CSS architecture
- [research-ux.md](./research-ux.md): Competitive analysis (6 launchers), gamepad patterns, dark theme
- [research-recommendations.md](./research-recommendations.md): Phased strategy, risk assessment, alternatives
