# Task Structure Analysis: ui-enhancements

## Executive Summary

The UI restructure replaces a 367-line god component (`App.tsx`) and its overloaded horizontal tab layout with a vertical sidebar navigation, 6 single-purpose page views, and a persistent console drawer. The work decomposes into 3 phases across roughly 24 discrete tasks. Phase 1 (Foundation) establishes context providers, CSS infrastructure, and the shell layout -- it is the critical-path bottleneck since every subsequent task depends on it. Phases 2 and 3 offer high parallelism because page shells and inline-style migrations are independent of each other.

The key design constraint is the resolved decision that Export is a subsection of ProfilesPage (not its own sidebar item), yielding 6 sidebar views: Profiles, Launch, Install, Browse, Compatibility, Settings. Note that `research-technical.md` still references the older 7-view model with Export as a separate page; the `feature-spec.md` resolved decisions take precedence.

## Recommended Phase Structure

### Phase 1: Foundation (Shell Layout + State Contexts)

**Purpose**: Build the structural skeleton -- context providers, CSS variables, layout CSS, sidebar, content area, console drawer -- without changing any existing component behavior. At the end of Phase 1, the app renders the new sidebar layout but each "page" just mounts the existing component unchanged.

**Suggested Tasks**:

1. **T1-1: CSS variables + layout CSS** -- Add sidebar/drawer/layout variables to `variables.css`, create `layout.css` for the app-level grid, `sidebar.css` for nav sections/items, `console-drawer.css` for the bottom drawer.
2. **T1-2: ProfileContext** -- Create `context/ProfileContext.tsx` wrapping the `UseProfileResult` interface from `useProfile.ts`.
3. **T1-3: PreferencesContext** -- Create `context/PreferencesContext.tsx` wrapping settings, recentFiles, steamClientInstallPath, targetHomePath, and their mutators. Move the `loadPreferences` effect and `refreshPreferences`/`handleAutoLoadChange`/`clearRecentFiles` functions out of `App.tsx`.
4. **T1-4: Sidebar component** -- Create `components/layout/Sidebar.tsx` with NavSection/NavItem subcomponents, vertical orientation, active-route indicator, bottom-pinned settings item, status chips (controller mode, last profile).
5. **T1-5: ContentArea component** -- Create `components/layout/ContentArea.tsx` as a route dispatcher switching on `AppRoute`. Initially renders existing components directly (no page shells yet).
6. **T1-6: ConsoleDrawer component** -- Create `components/layout/ConsoleDrawer.tsx` wrapping existing `ConsoleView` in a collapsible bottom drawer.
7. **T1-7: App.tsx refactor** -- Replace the current horizontal tab bar, two-column grid, and prop-drilling with `<AppProviders>` + `<Sidebar>` + `<ContentArea>` + `<ConsoleDrawer>`. Remove `headingTitle`/`headingCopy` derivation, `profileEditorTab` state, `shouldShowLauncherExport` conditional, and the `isInstallEditorContext` flag. Target ~60 lines.

**Parallelization**: T1-1, T1-2, T1-3 can run in parallel (no interdependencies). T1-4 depends on T1-1 (sidebar CSS). T1-5 and T1-6 depend on T1-1 (layout CSS). T1-7 depends on all of T1-1 through T1-6. Maximum 3 parallel tasks.

**Estimated effort**: 7 tasks, ~600 new lines.

### Phase 2: View Separation (Page Shells + Navigation Wiring)

**Purpose**: Split `ProfileEditorView` into `ProfilesPage` + `InstallPage`, create thin page shells for all 6 routes, remove install-context branches from `LaunchPanel` and `LauncherExport`, and wire sidebar navigation to all views. At the end of Phase 2, every route renders a dedicated page component that reads shared state from context.

**Dependencies**: Phase 1 complete (App.tsx refactor, contexts, layout shell).

**Suggested Tasks**:

1. **T2-1: ProfileActions extraction** -- Extract Save/Delete/dirty-indicator bar from `ProfileEditorView` into `components/ProfileActions.tsx` (~60 lines). This unblocks ProfilesPage.
2. **T2-2: ProfilesPage** -- Create `components/pages/ProfilesPage.tsx` composing `ProfileFormSections` + `ProfileActions` + `LauncherExport` (as an embedded subsection). Reads profile state from `ProfileContext`. Reads preferences from `PreferencesContext` for `steamClientInstallPath`/`targetHomePath`.
3. **T2-3: LaunchPage** -- Create `components/pages/LaunchPage.tsx`. Derives `launchMethod`, `effectiveLaunchMethod`, and `launchRequest` from `ProfileContext`. Mounts `LaunchPanel`.
4. **T2-4: InstallPage** -- Create `components/pages/InstallPage.tsx`. Absorbs the install sub-tab logic, profile review session state, and review modal orchestration from `ProfileEditorView`. Scopes the `effectiveLaunchMethod = 'proton_run'` override locally. This is the largest page shell (~200 lines).
5. **T2-5: CommunityPage + CompatibilityPage** -- Create thin wrappers. `CommunityPage` instantiates `useCommunityProfiles` and mounts `CommunityBrowser`. `CompatibilityPage` derives `compatibilityEntries` from the community index and mounts `CompatibilityViewer`.
6. **T2-6: SettingsPage** -- Create `components/pages/SettingsPage.tsx` that reads from `PreferencesContext` and mounts `SettingsPanel`, translating context values to props.
7. **T2-7: LaunchPanel cleanup** -- Remove the `context === 'install'` branch (lines 53-126 of LaunchPanel.tsx, ~74 lines of install-context JSX). Remove the `context` prop entirely.
8. **T2-8: LauncherExport cleanup** -- Remove the `context` prop and any install-mode conditional rendering. `LauncherExport` now only renders in the Profiles view context.
9. **T2-9: ProfileEditorView deprecation** -- Once ProfilesPage and InstallPage are wired, remove `ProfileEditorView` and the standalone `ProfileEditor` component from `ProfileEditor.tsx`. Clean up imports.
10. **T2-10: ContentArea wiring** -- Update `ContentArea.tsx` to dispatch to all 6 page components. Verify sidebar navigation triggers correct route changes.

**Parallelization**: T2-1 must complete before T2-2. T2-2 through T2-6 can run in parallel (each creates an independent page shell). T2-7 and T2-8 can run in parallel. T2-9 depends on T2-2 and T2-4. T2-10 depends on all page shells. Maximum 5 parallel tasks.

**Estimated effort**: 10 tasks, ~700 new lines, ~400 lines removed.

### Phase 3: Polish and Cleanup (CSS Migration + Gamepad + Enhancements)

**Purpose**: Migrate inline styles to CSS classes, refine gamepad navigation for the new layout, add sidebar-specific UX enhancements.

**Dependencies**: Phase 2 complete (all pages wired, install-context branches removed).

**Suggested Tasks**:

1. **T3-1: ConsoleView inline style migration** -- Replace all inline `style={{...}}` with existing `.crosshook-console__*` CSS classes that are already defined in `theme.css` (lines 416-479) but unused by the component. High-value, low-risk -- the CSS classes already exist.
2. **T3-2: LaunchPanel inline style migration** -- Replace ~200 lines of inline style objects (`panelStyles.card`, hardcoded hex colors like `#7bb0ff`, `#9fb1d6`) with `.crosshook-card` / `.crosshook-launch__*` CSS classes using `--crosshook-*` variables.
3. **T3-3: LauncherExport inline style migration** -- Replace ~100 lines of style constants (`panelStyle`, `sectionStyle`, `labelStyle`, `inputStyle`, `buttonStyle`, `subtleButtonStyle`) with CSS classes.
4. **T3-4: SettingsPanel inline style migration** -- Replace the `layoutStyles` record (~80 lines) with CSS classes.
5. **T3-5: CommunityBrowser inline style migration** -- Replace `panelStyles` record and `ratingStyles` with CSS classes.
6. **T3-6: CompatibilityViewer + AutoPopulate inline style migration** -- Replace `cardStyle`, `filterRowStyle` and AutoPopulate inline styles.
7. **T3-7: Gamepad navigation adaptation** -- Update `useGamepadNav` for zone-based navigation (sidebar zone + content zone). Add D-pad Left/Right zone switching. Add LB/RB bumper cycling through sidebar views. Update `handleGamepadBack` to return focus to sidebar from content. Highest risk task.
8. **T3-8: Console drawer auto-expand** -- Wire `useLaunchState.phase` changes to auto-expand the console drawer when a launch sequence starts.
9. **T3-9: Responsive sidebar collapse** -- Implement auto-collapse to 56px icon rail at < 900px breakpoint. Add `@media` queries to `sidebar.css`.
10. **T3-10: Controller prompt bar** -- Render the `.crosshook-controller-prompts` CSS class (already defined in `focus.css` lines 76-95 but unrendered) with context-sensitive button mappings.

**Parallelization**: T3-1 through T3-6 are fully independent (each touches a single component's styles). T3-7 through T3-10 are independent of each other but should follow T3-1/T3-2 to avoid merge conflicts with the same files. Maximum 6 parallel tasks.

**Estimated effort**: 10 tasks, ~500 new CSS lines, ~1,500 inline style lines removed.

## Task Granularity Recommendations

### Appropriate Task Sizes

- **Context providers** (T1-2, T1-3): ~40-60 lines each, single file, clear interface boundary. Ideal task size.
- **Page shells** (T2-2 through T2-6): ~30-120 lines each, 1 new file + minor import changes. Ideal task size.
- **Inline style migration per component** (T3-1 through T3-6): 1 component + 1 CSS file. Clearly scoped.
- **LaunchPanel/LauncherExport cleanup** (T2-7, T2-8): Single file each, removing a code branch. Clean scope.

### Tasks to Split

- **T1-7 (App.tsx refactor)**: This is the largest single task. Consider splitting into:
  - T1-7a: Strip state out of App.tsx into contexts (remove settings/recentFiles state, profile state management, derived heading logic).
  - T1-7b: Replace JSX tree with shell layout (`<AppProviders>` + layout components).
  - Rationale: The refactor touches state management and JSX structure simultaneously, and a partial refactor is hard to review.
  - Counter-argument: These two sub-tasks are tightly coupled and splitting them means App.tsx is broken between T1-7a and T1-7b. Better to do as one atomic task.

- **T2-4 (InstallPage)**: At ~200 lines and absorbing the review modal orchestration logic from ProfileEditorView, this is the most complex page shell. Consider splitting into:
  - T2-4a: Create InstallPage with InstallGamePanel mount, scoped `effectiveLaunchMethod` override.
  - T2-4b: Move profile review session state and review modal orchestration from ProfileEditorView into InstallPage.
  - Rationale: The review modal logic is complex (dirty checks, confirmation promise, session state). Moving it separately reduces risk.

### Tasks to Combine

- **T2-5 (CommunityPage + CompatibilityPage)**: These are thin wrappers (~30 lines each). Combine into one task since they share the community data flow and are rendered together in the current Community tab.
- **T3-5 + T3-6 (CommunityBrowser + CompatibilityViewer + AutoPopulate)**: All three are community-adjacent components with similar inline style patterns. Combining reduces the number of CSS file touches.

## Dependency Analysis

### Independent Tasks (No Prerequisites Beyond Phase Gate)

Within Phase 1:

- T1-1 (CSS variables + layout CSS): No code dependencies.
- T1-2 (ProfileContext): Only depends on `hooks/useProfile.ts` types (already stable).
- T1-3 (PreferencesContext): Only depends on `types/settings.ts` types (already stable).

Within Phase 2:

- T2-3 (LaunchPage): Independent of other page shells.
- T2-5 (CommunityPage + CompatibilityPage): Independent of other page shells.
- T2-6 (SettingsPage): Independent of other page shells.
- T2-7 (LaunchPanel cleanup): Independent of page shell creation.
- T2-8 (LauncherExport cleanup): Independent of page shell creation.

Within Phase 3:

- T3-1 through T3-6 (all inline style migrations): Fully independent of each other.
- T3-7 through T3-10 (gamepad, auto-expand, responsive, prompts): Independent of each other.

### Sequential Dependencies

```
T1-1 (CSS) ----+---> T1-4 (Sidebar) ------+
               |                           |
T1-2 (ProfileCtx) ---+                    +---> T1-7 (App.tsx refactor)
               |      |                   |
T1-3 (PrefsCtx) -----+---> T1-5 (Content) +
                      |                    |
                      +---> T1-6 (Drawer) -+

T1-7 ---------> T2-1 (ProfileActions) --> T2-2 (ProfilesPage)
            |
            +--> T2-3 (LaunchPage)
            +--> T2-4 (InstallPage)
            +--> T2-5 (Community/Compat)
            +--> T2-6 (SettingsPage)
            +--> T2-7 (LaunchPanel cleanup)
            +--> T2-8 (LauncherExport cleanup)

T2-2 + T2-4 --> T2-9 (ProfileEditorView deprecation)
T2-2..T2-6 ---> T2-10 (ContentArea wiring)

Phase 2 complete --> T3-1..T3-10 (all parallel)
```

### Potential Bottlenecks

1. **T1-7 (App.tsx refactor)**: Every Phase 2 task depends on this. It is the single most blocking task. If it stalls, nothing in Phase 2 can start.
   - Mitigation: Keep T1-7 focused on structural changes only -- no inline style migration, no new features. The smaller the diff, the faster the review.

2. **T1-1 (CSS infrastructure)**: T1-4, T1-5, T1-6 all depend on the layout CSS existing. However, these can use placeholder class names and refine CSS later.
   - Mitigation: Define class name conventions early. Layout components can stub CSS.

3. **T2-4 (InstallPage)**: The most complex page shell because it absorbs the review modal orchestration from `ProfileEditorView`. T2-9 (deprecation of ProfileEditorView) cannot proceed until both T2-2 and T2-4 land.
   - Mitigation: Split T2-4 as recommended above. Or defer T2-9 to a follow-up and keep `ProfileEditorView` as dead code temporarily.

## File-to-Task Mapping

### Files to Create

| File                                         | Suggested Task | Phase | Dependencies |
| -------------------------------------------- | -------------- | ----- | ------------ |
| `src/context/ProfileContext.tsx`             | T1-2           | 1     | None         |
| `src/context/PreferencesContext.tsx`         | T1-3           | 1     | None         |
| `src/styles/layout.css`                      | T1-1           | 1     | None         |
| `src/styles/sidebar.css`                     | T1-1           | 1     | None         |
| `src/styles/console-drawer.css`              | T1-1           | 1     | None         |
| `src/components/layout/Sidebar.tsx`          | T1-4           | 1     | T1-1         |
| `src/components/layout/ContentArea.tsx`      | T1-5           | 1     | T1-1         |
| `src/components/layout/ConsoleDrawer.tsx`    | T1-6           | 1     | T1-1         |
| `src/components/ProfileActions.tsx`          | T2-1           | 2     | T1-7         |
| `src/components/pages/ProfilesPage.tsx`      | T2-2           | 2     | T2-1         |
| `src/components/pages/LaunchPage.tsx`        | T2-3           | 2     | T1-7         |
| `src/components/pages/InstallPage.tsx`       | T2-4           | 2     | T1-7         |
| `src/components/pages/CommunityPage.tsx`     | T2-5           | 2     | T1-7         |
| `src/components/pages/CompatibilityPage.tsx` | T2-5           | 2     | T1-7         |
| `src/components/pages/SettingsPage.tsx`      | T2-6           | 2     | T1-7         |

All paths relative to `src/crosshook-native/src/`.

### Files to Modify

| File                                 | Suggested Task | Phase | Change Summary                                                                                                                                            |
| ------------------------------------ | -------------- | ----- | --------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `styles/variables.css`               | T1-1           | 1     | Add `--crosshook-sidebar-width`, `--crosshook-sidebar-width-collapsed`, `--crosshook-console-drawer-height`, `--crosshook-console-drawer-handle-height`   |
| `main.tsx`                           | T1-1           | 1     | Import new CSS files (`layout.css`, `sidebar.css`, `console-drawer.css`)                                                                                  |
| `App.tsx`                            | T1-7           | 1     | Major refactor: ~367 -> ~60 lines. Remove tab state, profile state lifting, heading derivation, two-column grid, prop drilling                            |
| `styles/theme.css`                   | T2-2, T3-\*    | 2-3   | Remove `.crosshook-tab-row`, `.crosshook-tab`, `.crosshook-tab--active` (replaced by sidebar). Add page-level classes. Extend with migrated inline styles |
| `components/LaunchPanel.tsx`         | T2-7           | 2     | Remove `context` prop and install-context branch (lines 53-126). Remaining: ~130 lines                                                                    |
| `components/LauncherExport.tsx`      | T2-8           | 2     | Remove `context` prop and install-mode branch                                                                                                             |
| `components/ProfileEditor.tsx`       | T2-9           | 2     | Delete or gut: `ProfileEditorView` and standalone `ProfileEditor` are replaced by ProfilesPage + InstallPage                                              |
| `components/ConsoleView.tsx`         | T3-1           | 3     | Replace all inline styles with `.crosshook-console__*` classes (already defined in theme.css)                                                             |
| `components/LaunchPanel.tsx`         | T3-2           | 3     | Replace `panelStyles` object and all inline `style={{...}}` with CSS classes                                                                              |
| `components/LauncherExport.tsx`      | T3-3           | 3     | Replace `panelStyle`, `sectionStyle`, `labelStyle`, `inputStyle`, `buttonStyle`, `subtleButtonStyle` constants with CSS classes                           |
| `components/SettingsPanel.tsx`       | T3-4           | 3     | Replace `layoutStyles` record with CSS classes                                                                                                            |
| `components/CommunityBrowser.tsx`    | T3-5           | 3     | Replace `panelStyles` record and `ratingStyles` with CSS classes                                                                                          |
| `components/CompatibilityViewer.tsx` | T3-6           | 3     | Replace `cardStyle`, `filterRowStyle` with CSS classes                                                                                                    |
| `components/AutoPopulate.tsx`        | T3-6           | 3     | Replace inline styles with CSS classes                                                                                                                    |
| `hooks/useGamepadNav.ts`             | T3-7           | 3     | Add zone-based navigation, bumper cycling, extended back handler                                                                                          |
| `styles/focus.css`                   | T3-10          | 3     | Controller prompt bar rendering (classes exist, need to be wired)                                                                                         |

## Optimization Opportunities

### Maximize Parallelism

- **Phase 1 triple-start**: T1-1, T1-2, T1-3 have zero interdependencies. Assign to parallel agents or developers immediately.
- **Phase 2 fan-out**: After T1-7 completes, tasks T2-1, T2-3, T2-4, T2-5, T2-6, T2-7, T2-8 can all start simultaneously (T2-2 waits only on T2-1). This is the widest fan-out point -- up to 7 tasks in parallel.
- **Phase 3 is embarrassingly parallel**: All 6 inline-style migration tasks (T3-1 through T3-6) touch separate component files. All 4 enhancement tasks (T3-7 through T3-10) touch separate hook/CSS files.
- **CSS-first approach**: Creating all CSS files in T1-1 before any components means layout components can be built without waiting for CSS reviews.

### Minimize Risk

- **Critical path**: T1-1 -> T1-4/T1-5/T1-6 -> T1-7 -> T2-4 -> T2-9. This is the longest sequential chain.
- **Highest-risk task**: T3-7 (gamepad navigation adaptation). The `useGamepadNav` hook traverses DOM-order focusable elements. Adding a sidebar fundamentally changes the traversal order. This task should include manual testing on a controller or Steam Deck.
- **Second-highest risk**: T1-7 (App.tsx refactor). This touches every import and every prop flow. A broken App.tsx blocks the entire Phase 2.
- **Modal focus trapping**: T2-4 (InstallPage) must verify that `ProfileReviewModal`'s `hiddenNodesRef` / inert sibling handling still works with the new sidebar element in the DOM tree.

## Implementation Strategy Recommendations

### Recommended Execution Order

1. Start with T1-1, T1-2, T1-3 in parallel. These are pure additions -- no existing code changes.
2. Once T1-1 lands, start T1-4, T1-5, T1-6 in parallel. These create new layout components.
3. T1-7 (App.tsx refactor) is the linchpin. Give it focused attention. Do not combine it with other work.
4. After T1-7, open the Phase 2 fan-out. Prioritize T2-1 (ProfileActions) immediately since T2-2 depends on it.
5. T2-4 (InstallPage) is the most complex Phase 2 task. Assign it to whoever has the deepest familiarity with the review modal flow.
6. Phase 3 tasks can be distributed freely. Prioritize T3-1 (ConsoleView) since it is the lowest risk (CSS classes already exist) and serves as a proof-of-concept for the migration pattern.
7. Save T3-7 (gamepad navigation) for last -- it requires the full new layout to be in place for proper testing.

### Testing Strategy

- **Phase 1**: After T1-7, verify that the app renders the sidebar layout and all existing components mount without errors. Console log stream should persist across route switches (the primary UX fix).
- **Phase 2**: After each page shell, verify the route renders the correct content and reads context state correctly. After T2-9, verify that no dead imports remain. Special attention to T2-4: test the full install -> review -> save flow.
- **Phase 3**: After each inline style migration, visually compare the component before/after. The components should look identical -- the change is purely mechanical (inline `style={{...}}` to `className`).
- **Gamepad**: T3-7 requires manual testing with a gamepad. Verify: D-pad up/down navigates within a zone, D-pad left/right switches between sidebar and content, LB/RB cycles sidebar views, B button returns to sidebar from content and closes modals when in a modal.

### Integration Approach

- **Feature branch per phase**: Create `feat/ui-phase-1`, `feat/ui-phase-2`, `feat/ui-phase-3`. Merge each phase to main before starting the next. This keeps PRs reviewable.
- **Alternative -- feature branch per task**: For maximum parallelism, create a branch per task and merge to a long-lived `feat/ui-enhancements` integration branch. Merge the integration branch to main once all Phase tasks are green.
- **Key invariant**: At no point should the app be in a broken state on the integration branch. Each task should produce a working (if partially migrated) app.

### Discrepancy to Resolve

The `research-technical.md` component tree still shows Export as a separate sidebar item and route (`ExportPage.tsx`). The `feature-spec.md` resolved decision (item 1) explicitly states: "Export is tightly coupled to profile state and belongs within the Profiles view as a subsection/panel, not a separate sidebar item." All tasks above follow the feature-spec resolved decisions (6 sidebar items, Export as a ProfilesPage subsection). If the research-technical.md is used as a reference during implementation, it should be updated to reflect this decision.
