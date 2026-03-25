# UI Enhancements Parallel Plan Evaluation

Systematic evaluation of the 27-task parallel implementation plan in `docs/plans/ui-enhancements/parallel-plan.md` against the current codebase state. Every line number reference, file path, and structural claim was verified against the source files.

## Task Quality Summary

- **Total Tasks**: 27 (Phase 1: 8, Phase 2: 10, Phase 3: 10)
- **High Quality**: 19
- **Needs Improvement**: 6 (Tasks 1.5, 1.6, 2.4, 2.5, 3.7, 3.8)
- **Needs Rewrite**: 2 (Tasks 3.5, 3.6)

## Verification of Shared Claims

All line counts claimed in the plan header and `shared.md` match the actual codebase exactly:

| File                    | Claimed | Actual                        | Match                       |
| ----------------------- | ------- | ----------------------------- | --------------------------- |
| App.tsx                 | 367     | 369 (367 up to closing brace) | Yes (export default adds 2) |
| ProfileEditor.tsx       | 588     | 588                           | Yes                         |
| ProfileFormSections.tsx | 695     | 695                           | Yes                         |
| LaunchPanel.tsx         | 257     | 257                           | Yes                         |
| LauncherExport.tsx      | 655     | 655                           | Yes                         |
| ConsoleView.tsx         | 270     | 270                           | Yes                         |
| SettingsPanel.tsx       | 470     | 470                           | Yes                         |
| CommunityBrowser.tsx    | 612     | 612                           | Yes                         |
| InstallGamePanel.tsx    | 547     | 547                           | Yes                         |
| ProfileReviewModal.tsx  | 457     | 456                           | Off by 1                    |
| AutoPopulate.tsx        | 320     | 319                           | Off by 1                    |
| useProfile.ts           | 479     | 479                           | Yes                         |
| useLaunchState.ts       | 244     | 244                           | Yes                         |
| useGamepadNav.ts        | 473     | 473                           | Yes                         |
| theme.css               | 870     | 869                           | Off by 1                    |
| variables.css           | 48      | 48                            | Yes                         |
| focus.css               | 108     | 108                           | Yes                         |

Three off-by-one counts are cosmetic and do not affect implementation.

## Detailed Findings

### Phase 1: Foundation

#### Task 1.1: Install dependencies and add CSS variables

**Rating:** High Quality

- Clear purpose: Install two npm packages and add 4 CSS variables
- Specific files: `package.json`, `variables.css` -- both exist and are correct paths
- Actionable: Exact variable names and values given, exact placement instructions (`:root` block, `@media (max-width: 900px)` block)
- Scope: 2 files
- Verified: The `@media (max-width: 900px)` block exists at line 43 of `variables.css`. The `:root` block is lines 1-34.

#### Task 1.2: Create layout CSS files

**Rating:** High Quality

- Clear purpose: Create 3 CSS files for the new layout grid
- Specific files: `layout.css`, `sidebar.css`, `console-drawer.css` (new), `main.tsx` (modify)
- Actionable: BEM class names fully specified with structural descriptions. References correct CSS variables.
- Gotchas: Correctly identifies that `main.tsx` needs import additions -- verified that `main.tsx` currently only imports `theme.css` and `focus.css` (line 4-5).
- Scope: 3 new + 1 modify = 4 files (slightly over 3-file guideline but acceptable for pure CSS)

#### Task 1.3: Create ProfileContext

**Rating:** High Quality

- Clear purpose: Wrap `useProfile()` in a React Context to eliminate prop-drilling
- Specific files: `context/ProfileContext.tsx` (new)
- Actionable: Specifies exact interface reference (`UseProfileResult`, lines 14-36 of `useProfile.ts` -- verified correct). Calls out `autoSelectFirstProfile: false` (verified at `App.tsx` line 72). Identifies the `listen('auto-load-profile')` subscription (verified at lines 201-208). Calls out `deriveSteamClientInstallPath` living in `ProfileFormSections.tsx` (verified at line 6-9 of `ProfileEditor.tsx` and line 9 of `App.tsx`).
- Gotchas documented: Correctly flags the `autoSelectFirstProfile: false` vs default `true` behavior. Correctly identifies `deriveSteamClientInstallPath` import chain issue.
- Scope: 1 new file + utility extraction

**Issue**: The task says to also move `deriveSteamClientInstallPath` to `utils/steam.ts` "or inline it" -- this is ambiguous. An implementer may skip this or create conflicts with Task 2.2 (ProfilesPage) which also references this function. Should specify definitively where it goes.

#### Task 1.4: Create PreferencesContext

**Rating:** High Quality

- Clear purpose: Extract settings/preferences state from App.tsx into a context
- Specific files: `context/PreferencesContext.tsx` (new)
- Actionable: Full TypeScript interface provided. Line references for state declarations (74-77) and load/mutate functions (173-241) are verified correct.
- Gotchas: Correctly identifies the `let active = true` guard pattern.
- Scope: 1 new file

#### Task 1.5: Create Sidebar component

**Rating:** Needs Improvement

- Clear purpose: Create vertical sidebar with Radix Tabs
- Specific files: `components/layout/Sidebar.tsx` (new)
- Actionable: Good structural description (3 NavSections, bottom-pinned items, Radix Tabs.Trigger usage)

**Issues**:

1. The `AppRoute` type is defined here but Task 1.6 also says to export it. Task 1.8 uses it for `useState<AppRoute>`. The plan says "Export it from this file or a new `types/routes.ts`" -- this needs a definitive decision. If an implementer picks one location and another task expects the other, there will be import conflicts.
2. No icon system is specified. The sidebar needs icons for each nav item, but the plan says nothing about what icons to use. Task 3.9 later says "inline SVG or emoji for now" for collapsed mode, but the expanded sidebar also needs icons alongside labels. This is a UX detail that will force the implementer to make unguided decisions.
3. The `lastProfile: string` prop implies reading from a context or prop that does not yet exist at the Sidebar level -- it comes from `settings.last_used_profile` which will be in `PreferencesContext`, but the Sidebar is rendered in App.tsx which owns the route state. The prop threading is unclear.

#### Task 1.6: Create ContentArea component

**Rating:** Needs Improvement

- Clear purpose: Route dispatcher component
- Specific files: `components/layout/ContentArea.tsx` (new)

**Issues**:

1. The task says "Initially render existing components directly (before page shells exist in Phase 2)" with a code snippet showing `<ProfileEditorView ... />`, `<LaunchPanel ... />`, etc. But these components require specific props that come from state currently in `App.tsx`. The task does not explain how to thread props through to these temporary renders. `ProfileEditorView` needs a `UseProfileResult` state object, `LaunchPanel` needs `profileId`, `method`, `request`, etc. The implementer must figure out context consumption or prop passing for a temporary scaffold that gets replaced in Phase 2.
2. The `forceMount` mention is vague -- "Use `forceMount` to keep critical components mounted if needed" does not specify which components are critical. ConsoleView has already moved to the drawer (Task 1.7), so what else needs forceMount?
3. The `...` in the code snippet hides the actual complexity of prop passing.

#### Task 1.7: Create ConsoleDrawer component

**Rating:** High Quality

- Clear purpose: Collapsible bottom drawer wrapping ConsoleView
- Specific files: `components/layout/ConsoleDrawer.tsx` (new)
- Actionable: State behavior specified (collapsed default, always mounted, CSS transitions not conditional rendering). Integration with `react-resizable-panels` API described with specific props (`collapsible`, `collapsedSize`).
- Gotchas: Correctly flags the log-loss-on-tab-switch bug and specifies the fix (always mounted).
- Scope: 1 file

**Minor issue**: The `collapsedSize` matching "the toggle bar height" is in percentage terms for react-resizable-panels, not pixels. The implementer needs to calculate what percentage `40px` (the handle height variable) represents of the total height. This is a minor but potentially confusing detail.

#### Task 1.8: Refactor App.tsx to shell layout

**Rating:** High Quality

- Clear purpose: Replace the god component with a ~60-line shell
- Specific files: `App.tsx` (modify)
- Actionable: Exhaustive list of what to remove, what to keep, and what to replace with. Full JSX tree provided as a code block.
- Gotchas: Correctly identifies this as the critical bottleneck. Correctly identifies `useGamepadNav` and `rootRef` as things to preserve.
- Scope: 1 file (appropriate for the complexity)

**Issue**: The JSX code block shows `<Tabs.Root orientation="vertical" value={route} onValueChange={setRoute}>` but `onValueChange` passes a `string`, not `AppRoute`. The implementer needs to cast: `onValueChange={(v) => setRoute(v as AppRoute)}`. This is a small but real type-safety issue that should be documented.

### Phase 2: View Separation

#### Task 2.1: Extract ProfileActions component

**Rating:** High Quality

- Clear purpose: Extract the Save/Delete/dirty-indicator bar
- Specific files: `components/ProfileActions.tsx` (new)
- Actionable: Exact line range (438-455) verified -- the save/delete bar is at lines 438-455 of ProfileEditor.tsx. Lists all consumed context values.
- Scope: 1 new file

#### Task 2.2: Create ProfilesPage

**Rating:** High Quality

- Clear purpose: Compose profile editing page from extracted components
- Specific files: `components/pages/ProfilesPage.tsx` (new)
- Actionable: Component composition clearly described. References correct `useEffect` for proton installs (lines 317-354, verified). Includes delete confirmation overlay (lines 532-565, verified).
- Scope: 1 new file

**Minor issue**: The task says `LauncherExport` is rendered "as a collapsible subsection" but does not specify the collapsible mechanism (CSS details, disclosure widget, etc.).

#### Task 2.3: Create LaunchPage

**Rating:** High Quality

- Clear purpose: Dedicated launch view
- Specific files: `components/pages/LaunchPage.tsx` (new)
- Actionable: Clear derivation instructions. Correctly notes removing the install-context override.
- Scope: 1 new file

#### Task 2.4: Create InstallPage

**Rating:** Needs Improvement

- Clear purpose: Most complex page shell absorbing review modal orchestration
- Specific files: `components/pages/InstallPage.tsx` (new)
- Scope: 1 new file (but ~200 lines of complex logic)

**Issues**:

1. The line references for what to absorb from ProfileEditorView are listed as items 1-4 with line ranges, but some ranges overlap or are imprecise:
   - Item 1 says "lines 84-86" for review session state -- verified correct (`profileReviewSession`, `reviewConfirmation`, `reviewConfirmationResolverRef`).
   - Item 2 says "lines 15-46" for helper functions -- these are module-level functions, not inside the component. The task conflates module-level functions with component-internal functions.
   - Item 3 says "lines 92-133 and 135-315" for review handlers -- verified correct.
   - Item 4 says "lines 464-530" for review modal JSX -- verified correct (actually 464-530).
2. The task mentions `persistProfileDraft` call on save (line 303) should navigate to Profiles route, saying "accept an `onNavigate` prop or use a callback." This is another "pick one" ambiguity. Since this is the most complex task, it should be definitive.
3. The Advice section correctly warns this should take 2-3x longer and suggests splitting into sub-tasks, but does not actually split it. For a plan claiming 1-3 file scope, a ~200-line extraction of complex stateful logic with promise-based confirmation flows deserves sub-tasks.
4. Missing: The `reviewDescription` and `reviewModalStatusTone` and `reviewFinalExecutableMissing` derived values (used in the JSX at lines 464-530) are not listed as things to absorb. These are computed in ProfileEditorView but not mentioned in the extraction list. An implementer who only follows items 1-4 will hit undefined references.

#### Task 2.5: Create CommunityPage and CompatibilityPage

**Rating:** Needs Improvement

- Clear purpose: Two simple page shells
- Specific files: `components/pages/CommunityPage.tsx`, `components/pages/CompatibilityPage.tsx` (new)

**Issues**:

1. Two pages in one task violates the plan's own "1-3 files maximum" scope guideline in spirit -- the two pages have different data flow concerns.
2. The task says CompatibilityPage should "Accept community state as a prop or use shared context" and then says "Consider sharing community state between both pages via a lightweight `CommunityContext`." This is a significant architectural decision (new context vs prop drilling) left unresolved. If CommunityContext is needed, it should be its own task.
3. The `DEFAULT_PROFILES_DIRECTORY` constant is used in CommunityPage but currently lives in `App.tsx` (line 31). The plan does not mention where this constant should be relocated or imported from.

#### Task 2.6: Create SettingsPage

**Rating:** High Quality

- Clear purpose: Simple settings wrapper
- Specific files: `components/pages/SettingsPage.tsx` (new)
- Actionable: Clear context consumption pattern
- Scope: 1 new file

**Minor issue**: The task says `steamClientInstallPath` and `targetHomePath` "can come from `useProfileContext()` or `usePreferencesContext()` depending on where they were placed in Task 1.3/1.4." This should have been resolved during planning, not deferred to implementation time.

#### Task 2.7: Remove LaunchPanel install-context branch

**Rating:** High Quality

- Clear purpose: Remove dead code path after install view separation
- Specific files: `LaunchPanel.tsx` (modify)
- Actionable: Exact line range for removal (lines 53-127, ~74 lines). Verified: `if (isInstallContext)` block starts at line 53 and the install-specific return ends at line 127. The "~74 lines" count is accurate.
- Scope: 1 file

#### Task 2.8: Remove LauncherExport install-context branch

**Rating:** High Quality

- Clear purpose: Remove dead code path after install view separation
- Specific files: `LauncherExport.tsx` (modify)
- Actionable: Exact line range for removal (lines 270-336, ~66 lines). Verified: `if (context === 'install')` starts at line 270 and closes at line 337. The count is accurate.
- Scope: 1 file

#### Task 2.9: Wire ContentArea to page components

**Rating:** High Quality

- Clear purpose: Replace temporary scaffold with final page routing
- Specific files: `components/layout/ContentArea.tsx` (modify)
- Actionable: Clear switch statement with all route mappings
- Scope: 1 file

#### Task 2.10: Deprecate ProfileEditorView

**Rating:** High Quality

- Clear purpose: Remove the now-dead god component from ProfileEditor.tsx
- Specific files: `ProfileEditor.tsx` (modify or delete)
- Actionable: Identifies lines 570-586 as the unused standalone `ProfileEditor` component -- verified correct. Correctly advises grepping for remaining imports.
- Scope: 1 file + cleanup grep

### Phase 3: Polish and Cleanup

#### Task 3.1: Migrate ConsoleView inline styles to CSS classes

**Rating:** High Quality

- Clear purpose: Replace inline styles with existing unused CSS classes
- Specific files: `ConsoleView.tsx` (modify)
- Actionable: Identifies the exact CSS classes already defined in theme.css (lines 416-479, verified). Names the `buttonStyle` constant to remove (lines 258-268, verified).
- Gotchas: Correctly identifies this as zero-risk since classes already exist.
- Scope: 1 file

#### Task 3.2: Migrate LaunchPanel inline styles to CSS classes

**Rating:** High Quality

- Clear purpose: Replace inline styles with new CSS classes
- Specific files: `LaunchPanel.tsx` (modify), `theme.css` (modify)
- Actionable: Class names specified. Color mapping from hardcoded values to CSS variables provided.
- Scope: 2 files

#### Task 3.3: Migrate LauncherExport inline styles to CSS classes

**Rating:** High Quality

- Clear purpose: Remove ~80 lines of style constants
- Specific files: `LauncherExport.tsx` (modify), `theme.css` (modify)
- Actionable: Lists all 8 style constants to remove (verified: `panelStyle` at line 32, `sectionStyle` at 45, `labelStyle` at 50, `inputStyle` at 57, `buttonStyle` at 69, `subtleButtonStyle` at 80, `deleteButtonStyle` at 85, `helperStyle` at 97, `infoCalloutStyle` at 104). The plan says "8 style constants" but there are actually 9 (includes `deleteButtonConfirmingStyle` at line 92). Minor miss.
- Scope: 2 files

#### Task 3.4: Migrate SettingsPanel inline styles to CSS classes

**Rating:** High Quality

- Clear purpose: Remove the `layoutStyles` record
- Specific files: `SettingsPanel.tsx` (modify), `theme.css` (modify)
- Actionable: `layoutStyles` record at lines 27-115 verified. CSS class names specified.
- Scope: 2 files

#### Task 3.5: Migrate CommunityBrowser and CompatibilityViewer inline styles

**Rating:** Needs Rewrite

- Combines two separate component migrations into one task
- Specific files: `CommunityBrowser.tsx`, `CompatibilityViewer.tsx`, `theme.css` (3 files)

**Issues**:

1. Three files modified is at the scope limit, but the task covers two unrelated components with different inline style patterns.
2. CommunityBrowser CSS class names are specified (`crosshook-community-*`), but CompatibilityViewer classes are punted with a wildcard: `.crosshook-compatibility-*`. The implementer has no guidance on what classes to create for CompatibilityViewer (382 lines of component code).
3. The task does not identify which specific style objects to remove from CommunityBrowser (e.g., the `panelStyles` and `ratingStyles` records are mentioned but line numbers are not given).
4. Should be split into two tasks: one for CommunityBrowser, one for CompatibilityViewer.

#### Task 3.6: Migrate AutoPopulate inline styles

**Rating:** Needs Rewrite

- Specific files: `AutoPopulate.tsx` (modify), `theme.css` (modify)

**Issues**:

1. The entire instruction is: "Remove all inline style objects from AutoPopulate.tsx. Create `.crosshook-auto-populate-*` CSS classes using `--crosshook-*` variables." This is the vaguest task in the entire plan.
2. No identification of which inline style objects exist in AutoPopulate.tsx (319 lines).
3. No class names specified beyond the wildcard prefix.
4. No color-to-variable mapping provided (unlike Task 3.2 which is thorough).
5. An implementer would need to read the entire 319-line file, catalog all inline styles, design a class naming scheme, and map colors to variables -- essentially doing the research that should have been in the plan.

#### Task 3.7: Gamepad navigation adaptation

**Rating:** Needs Improvement

- Clear purpose: Adapt gamepad nav for sidebar+content layout
- Specific files: `useGamepadNav.ts` (modify)

**Issues**:

1. This is correctly identified as the highest-risk task, but the 5 bullet points describe desired behavior without explaining how to achieve it within the existing `useGamepadNav.ts` architecture.
2. The hook currently uses `rootRef` and traverses focusable elements in DOM order. Adding zone-based navigation requires understanding the internal `focusableElements` collection, the `handleGamepadInput` function structure, and the focus cycling logic. None of this internal structure is referenced.
3. "Focus memory per zone" requires a new data structure (Map of zone to last-focused element) but no implementation guidance is given.
4. "Test thoroughly with a gamepad" is the only testing guidance for the highest-risk change. Should specify what specific test scenarios to verify (e.g., "navigate to sidebar, press Down 3 times, press Right to enter content, press Left to return to sidebar -- verify focus returns to the 3rd sidebar item").
5. The task modifies only 1 file but the behavioral changes are complex enough to warrant sub-tasks (e.g., zone detection, zone switching, focus memory, bumper cycling, auto-focus on route change).

#### Task 3.8: Console drawer auto-expand on launch

**Rating:** Needs Improvement

- Clear purpose: Auto-expand drawer when launch starts
- Specific files: `ConsoleDrawer.tsx` (modify)

**Issues**:

1. The task correctly identifies the challenge: "The drawer needs access to launch phase state." But then offers two approaches (consume `useLaunchState` directly, or listen for `launch-log` events) without choosing one. The `useLaunchState` hook requires `profileId`, `method`, and `request` props which come from ProfileContext -- this is a non-trivial wiring challenge that should be decided in the plan.
2. The simpler `launch-log` event listener approach is mentioned as an alternative but would require knowing the Tauri event name and payload shape. This is not documented.
3. The `panelRef.expand()` API reference is correct for react-resizable-panels but requires setting up an `ImperativePanelHandle` ref, which is not mentioned.

#### Task 3.9: Responsive sidebar collapse

**Rating:** High Quality

- Clear purpose: Auto-collapse sidebar to icon rail below 900px
- Specific files: `Sidebar.tsx` (modify), `sidebar.css` (modify)
- Actionable: Breakpoint specified (900px), collapsed width specified (56px), `matchMedia` API specified.
- Scope: 2 files

**Minor issue**: References `@radix-ui/react-tooltip` for collapsed item labels, but this package is not in the dependency installation task (Task 1.1 only installs `@radix-ui/react-tabs` and `react-resizable-panels`). Either add it to Task 1.1 or remove the tooltip suggestion.

#### Task 3.10: Controller prompt bar

**Rating:** High Quality

- Clear purpose: Render context-sensitive gamepad button prompts
- Specific files: `components/layout/ControllerPrompts.tsx` (new), `focus.css` (modify)
- Actionable: Correctly identifies that `.crosshook-controller-prompts` and `.crosshook-controller-prompts__glyph` already exist in `focus.css` (verified at lines 76-95). Specifies button mappings.
- Scope: 1 new + 1 modify

## Priority Improvements

1. **Task 2.4 (InstallPage) -- missing derived values**: Add `reviewDescription`, `reviewModalStatusTone`, and `reviewFinalExecutableMissing` to the extraction list. Without these, the review modal JSX will have undefined references. This is a blocking omission.

2. **Task 3.6 (AutoPopulate migration) -- rewrite entirely**: The task needs to catalog the specific inline style objects in AutoPopulate.tsx, name the CSS classes, and provide color-to-variable mappings -- the same level of detail given to Tasks 3.2-3.4.

3. **Task 3.5 (CommunityBrowser + CompatibilityViewer) -- split into two tasks**: Each component has distinct style patterns. CompatibilityViewer's classes need to be specified rather than punted with a wildcard.

4. **Task 2.5 (CommunityPage + CompatibilityPage) -- resolve the CommunityContext decision**: The plan should decide definitively whether community state is shared via context or props, and where `DEFAULT_PROFILES_DIRECTORY` is imported from.

5. **Task 1.5 (Sidebar) -- decide AppRoute location and icon system**: Pin the `AppRoute` type to `types/routes.ts` and specify what icons to use (even if placeholder emojis, name them per route).

6. **Task 1.6 (ContentArea) -- document temporary prop threading**: The Phase 1 temporary scaffold needs clear guidance on how existing components get their props before contexts exist. Consider specifying that ContentArea does not render full components initially -- just placeholder text per route until Phase 2.

7. **Task 3.7 (Gamepad adaptation) -- add implementation hooks and test scenarios**: Reference specific functions/data structures inside `useGamepadNav.ts` that need modification. Add concrete test scenarios.

8. **Task 3.8 (Console auto-expand) -- pick one approach**: Choose between `useLaunchState` consumption and `launch-log` event listening. Document the `ImperativePanelHandle` ref requirement.

9. **Task 1.1 -- add @radix-ui/react-tooltip if Task 3.9 needs it**: Or remove the tooltip suggestion from Task 3.9.

10. **Task 1.3 -- pin `deriveSteamClientInstallPath` destination**: Say definitively it goes to `utils/steam.ts`, not "or inline it."

## Overall Assessment

The plan is well-structured and demonstrates thorough knowledge of the codebase. The phased dependency graph (Foundation -> View Separation -> Polish) is sound, and the critical path through Task 1.8 is correctly identified. Line number references are accurate across all 17 source files (only 3 trivial off-by-one discrepancies). File paths are all real and correct. The Advice section contains genuinely useful implementation guidance.

**Strengths**: Phase 1 and Phase 2 tasks are generally implementation-ready with precise line references, verified code structure, and clear dependency chains. The CSS migration tasks (3.1-3.4) are particularly well-specified with exact constant names, line ranges, and color-to-variable mappings.

**Weaknesses**: Phase 3 has the most quality variance -- Tasks 3.5 and 3.6 are too vague to implement without additional research, and Task 3.7 describes desired behavior without implementation-level guidance for the most complex code change. Several tasks defer architectural decisions ("or", "consider", "depending on") that should be resolved in the plan to avoid divergent implementations.

**Implementation readiness**: 19 of 27 tasks (70%) are immediately implementable. The remaining 8 need improvement ranging from minor clarifications (pinning a type export location) to full rewrites (Task 3.6). After addressing the priority improvements above, the plan would be suitable for parallel execution by multiple agents with minimal coordination overhead.
