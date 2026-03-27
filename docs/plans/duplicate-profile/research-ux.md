# UX Research: Duplicate Profile Feature

## Executive Summary

This research examines user experience patterns for duplicating/cloning configurations in desktop applications, with specific focus on game launchers and Steam Deck gamepad accessibility. The key findings indicate that the optimal approach for CrossHook is: (1) use "Duplicate" as the action label (not "Copy" or "Clone"), (2) auto-generate names using the pattern "Name (Copy)" with numeric suffixes for subsequent duplicates, (3) auto-select the new profile for immediate editing, and (4) ensure the action is accessible via both a button in the profile actions area and a keyboard shortcut (Ctrl+D).

The research draws from competitive analysis of Lutris, VS Code, JetBrains IDEs, Bottles, Firefox, macOS Finder, and Windows Explorer, as well as industry UX guidelines from Nielsen Norman Group and established desktop application conventions.

---

## User Workflows

### Primary Flow (Mouse/Keyboard)

The recommended workflow for duplicating a profile:

1. **Trigger**: User has a profile loaded in the Profile Editor and clicks a "Duplicate" button in the profile actions area (alongside Save and Delete), or presses Ctrl+D
2. **Execution**: The system creates a copy of the current profile with an auto-generated name (e.g., "Dark Souls III (Copy)")
3. **Feedback**: The new profile is immediately selected in the profile selector dropdown, the profile name field is focused with the name selected (ready for inline rename), and a brief status message appears (e.g., "Profile duplicated")
4. **Editing**: The user can immediately rename and modify the duplicated profile
5. **Completion**: The user saves the modified profile as usual

**Confidence**: High -- This pattern is consistent across VS Code profiles, JetBrains run configurations, and macOS Finder duplicates. All prioritize immediacy: the duplicate is created, selected, and ready for editing in one action.

### Alternative Flow: Prompted Name

Some applications prompt for a name before creating the duplicate (Firefox profile manager, some IDE configuration dialogs). This adds friction but avoids the "Name (Copy)" artifact if the user forgets to rename.

**Recommendation**: Auto-generate the name without prompting. The friction of a dialog is worse than the minor naming artifact, and the profile name field being focused with text selected makes renaming effortless. CrossHook already has inline editing for profile names, so this flow is natural.

**Confidence**: High -- Nielsen Norman Group guidelines emphasize reducing unnecessary dialogs and interruptions. The auto-generate + focus-for-rename pattern is the modern standard.

### Gamepad Flow (Steam Deck)

1. **Trigger**: User navigates to the profile actions area using D-pad, highlights "Duplicate" button, presses A to activate
2. **Execution**: Same as mouse/keyboard flow
3. **Feedback**: The profile selector updates to show the new profile name; the name input field gains focus. The controller prompt bar at the bottom should show "A: Select | B: Back" as it does currently
4. **Editing**: User can navigate to the profile name field and use the on-screen keyboard to rename

**Key consideration**: The Steam Deck's on-screen keyboard appears automatically when a text field gains focus in Steam Input. By auto-focusing the profile name field with text selected, the gamepad user gets the same "ready to rename" experience as mouse/keyboard users without any additional interaction steps.

**Confidence**: Medium -- This pattern follows Steam Deck conventions for text input, but the exact behavior depends on how Steam Input handles focus events in Tauri webview apps. Testing on actual hardware is recommended.

---

## UI/UX Best Practices

### Action Placement

#### Where Duplication Actions Appear in Desktop Applications

| Application          | Placement                                   | Trigger                                 |
| -------------------- | ------------------------------------------- | --------------------------------------- |
| **macOS Finder**     | File menu + keyboard shortcut               | Cmd+D on selected file                  |
| **Windows Explorer** | Ctrl+C/Ctrl+V in same folder                | Keyboard shortcut                       |
| **Lutris**           | Right-click context menu on game            | Context menu "Duplicate"                |
| **VS Code**          | Command Palette + Profiles editor           | "Profiles: Create from Current Profile" |
| **JetBrains IDEs**   | Toolbar button in Run Configurations dialog | "Copy Configuration" icon button        |
| **Figma**            | Context menu + keyboard shortcut            | Ctrl+D or right-click "Duplicate"       |
| **Postman**          | Context menu on collection/request          | Right-click "Duplicate"                 |

**Confidence**: High -- sourced from official documentation and confirmed behavior across multiple applications.

**Recommendation for CrossHook**: Place the "Duplicate" button in the `ProfileActions` component, alongside the existing Save and Delete buttons. This matches the JetBrains pattern (toolbar button alongside other configuration actions) and is the most accessible placement for both mouse and gamepad users.

The button should be:

- Positioned between Save and Delete (logical grouping: constructive actions left, destructive right)
- Styled as a secondary button (`crosshook-button--secondary`) to match Delete's visual weight
- Disabled when no profile is loaded or when a save/delete operation is in progress

### Naming Strategy

#### How Desktop Applications Name Duplicated Items

| Application          | Naming Pattern                   | Progressive Duplicates               |
| -------------------- | -------------------------------- | ------------------------------------ |
| **macOS Finder**     | "Name copy"                      | "Name copy 2", "Name copy 3"         |
| **Windows Explorer** | "Name - Copy"                    | "Name - Copy (2)", "Name - Copy (3)" |
| **JetBrains IDEs**   | "Name (1)"                       | "Name (2)", "Name (3)"               |
| **Figma**            | "Name Copy"                      | "Name Copy 2", "Name Copy 3"         |
| **Lutris**           | "Name" (same name, different ID) | N/A (uses internal IDs)              |
| **Google Docs**      | "Copy of Name"                   | "Copy of Copy of Name" (stacking)    |

**Confidence**: High -- verified through official documentation and user reports for each application.

**Recommendation for CrossHook**: Use the pattern `"Name (Copy)"` for the first duplicate, then `"Name (Copy 2)"`, `"Name (Copy 3)"` for subsequent duplicates. Rationale:

1. **Parenthetical suffix** avoids the Google Docs anti-pattern of stacking prefixes ("Copy of Copy of...")
2. **"(Copy)" rather than "(1)"** is more descriptive for non-technical users -- it is immediately clear this is a duplicate, not a version number
3. **Checking existing names** prevents conflicts: if "Name (Copy)" already exists, skip to "Name (Copy 2)"
4. This matches the hybrid approach used by Figma and is similar to macOS Finder

### Feedback Patterns

#### Post-Duplication Feedback Across Applications

| Application      | Feedback Mechanism                                                   |
| ---------------- | -------------------------------------------------------------------- |
| **macOS Finder** | New file appears in folder, selected and ready for rename            |
| **VS Code**      | New profile appears in list, profile editor shows new profile        |
| **JetBrains**    | Configuration dialog updates to show new config, name field editable |
| **Figma**        | New element appears on canvas, selected, layers panel scrolls to it  |
| **Lutris**       | New game entry appears in library list                               |

**Confidence**: High -- consistent pattern across all surveyed applications.

**Recommended feedback for CrossHook (in order of execution)**:

1. **Profile list update**: The profiles dropdown immediately includes the new profile name
2. **Auto-select**: The new profile is selected in the dropdown
3. **Name field focus**: The profile name input field is focused with text selected (ready for rename)
4. **Status indicator**: The "No unsaved changes" text in ProfileActions updates to reflect the new state (the profile was just saved during duplication, so it should show as clean)
5. **No toast notification needed**: The visible state change (dropdown update + name field focus) provides sufficient feedback. Toast notifications are recommended only when the result of an action is not immediately visible in the UI (per Nielsen Norman Group guidelines).

### Terminology: "Duplicate" vs "Copy" vs "Clone"

Research from UX writing specialists indicates:

- **"Copy"** implies transferring content elsewhere (clipboard metaphor). Users associate it with copy-paste workflows. Using "Copy" for in-place duplication can cause confusion.
- **"Clone"** has technical connotations (Git, virtual machines). It may imply a deeper relationship between source and result.
- **"Duplicate"** clearly communicates "make another one right here." It has no clipboard or version-control baggage and is the term used by macOS Finder, Figma, Lutris, and Postman.

**Confidence**: Medium -- The UX Writing Hub article and broader industry usage support "Duplicate" as the preferred term, but there is no universal standard. The terms are often used interchangeably.

**Recommendation**: Use "Duplicate" as the action label. It aligns with CrossHook's existing vocabulary (the app does not use "clone" or "copy" for profile operations).

### Keyboard Shortcut

**Ctrl+D** is the established standard for duplicate actions across desktop applications:

| Application    | Shortcut                |
| -------------- | ----------------------- |
| macOS Finder   | Cmd+D                   |
| Figma          | Ctrl+D / Cmd+D          |
| JetBrains IDEs | Ctrl+D (duplicate line) |
| Visual Studio  | Ctrl+D (duplicate code) |
| FL Studio      | Ctrl+D                  |
| Miro           | Ctrl+D                  |

**Confidence**: High -- Ctrl+D is consistently used for duplication across creative tools, IDEs, and file managers.

**Recommendation**: Bind Ctrl+D (Cmd+D on macOS if relevant) to the duplicate action when the Profiles page is active and a profile is loaded. Ensure this does not conflict with existing keybindings (CrossHook does not currently use Ctrl+D based on the codebase review).

---

## Error Handling UX

### Error States

| Error Condition                          | UX Approach                               | Rationale                                                                                                                                     |
| ---------------------------------------- | ----------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------- |
| **Name conflict**                        | Auto-resolve by incrementing suffix       | Never prompt the user to resolve a naming conflict for a generated name. The system created the name, the system should resolve the conflict. |
| **Source profile not saved**             | Disable Duplicate button when dirty       | Prevent duplicating unsaved state. The button tooltip should explain: "Save profile before duplicating"                                       |
| **Disk/filesystem error**                | Show error in ProfileActions error banner | Use the existing `error` state in ProfileActions, consistent with how save/delete errors are shown                                            |
| **Source profile deleted mid-operation** | Show error in ProfileActions error banner | Extremely unlikely in a single-user desktop app, but handle gracefully with "Profile not found" error                                         |
| **Profile list at maximum**              | Not applicable                            | TOML files on disk have no practical limit                                                                                                    |

**Confidence**: High -- These patterns match existing CrossHook error handling (the ProfileActions component already shows errors via `crosshook-error-banner`).

### Validation Rules

- The generated name must not be empty (guaranteed by the naming algorithm since the source name is always non-empty)
- The generated name must not collide with an existing profile name (resolved by the incremental suffix algorithm)
- The Duplicate button should be disabled when: no profile is loaded, profile has unsaved changes, a save/delete/duplicate operation is in progress

### Loading State

While the duplicate operation runs (writing TOML file to disk):

- The Duplicate button text should change to "Duplicating..." (matching "Saving..." and "Deleting..." patterns)
- Save, Delete, and Duplicate buttons should all be disabled during the operation
- This is consistent with the existing `saving` and `deleting` boolean states in ProfileActions

---

## Steam Deck / Gamepad Considerations

### Current CrossHook Gamepad Support

Based on the codebase review:

- CrossHook uses a `useGamepadNav` hook for controller navigation
- The `ControllerPrompts` component shows A (Select), B (Back), LB/RB (Switch View) at the bottom of the screen
- Focus zones are marked with `data-crosshook-focus-zone` attributes
- The app targets 1280x800 resolution (Steam Deck native)

### Gamepad-Specific UX Recommendations

1. **Button accessibility**: The Duplicate button in ProfileActions will be naturally accessible via D-pad navigation since it sits alongside Save and Delete, which are already gamepad-navigable. No additional gamepad bindings are needed.

2. **Focus management after duplication**: After the duplicate completes, focus should move to the profile name input field. This triggers the Steam Input on-screen keyboard, giving the user an immediate path to rename. The `data-crosshook-focus-zone` system should handle this automatically if the focus is set programmatically.

3. **No context menus**: Context menus (right-click) are not gamepad-friendly. Lutris's right-click "Duplicate" pattern does not translate well to gamepad. CrossHook's approach of using explicit buttons is correct for gamepad accessibility.

4. **Controller prompts update**: Consider adding a contextual controller prompt for the Duplicate action. However, the current prompt bar shows generic navigation (A/B/LB/RB), and adding context-specific prompts may require significant UX rework. This is a "nice to have" for later.

5. **Touch input**: The Steam Deck touchscreen provides a fallback for any interaction that is difficult via gamepad. The Duplicate button should have adequate touch target size (minimum 44x44px per WCAG guidelines, which CrossHook's existing buttons already meet).

**Confidence**: Medium -- The gamepad navigation system is in place, but the exact behavior of focus management after the duplicate operation requires testing on actual hardware or the Steam Deck emulator.

### Radial Menu Consideration

Steam Input supports radial menus that could theoretically expose profile actions (Save, Delete, Duplicate, Export) via a single button hold. However, this is a Steam Input configuration concern, not a CrossHook application concern. The app should focus on making all actions keyboard/D-pad accessible; power users can configure Steam Input radial menus themselves.

---

## Competitive Analysis

### Game Launchers

#### Lutris

- **Duplicate mechanism**: Right-click context menu on any game entry, select "Duplicate"
- **Naming**: Creates a new entry with the same name but different internal ID
- **Scope**: Duplicates game configuration but NOT the Wine prefix (this was a deliberate design decision per GitHub issue #5447; prefix duplication was explicitly rejected by the project lead)
- **Known issues**: Cloned games share image settings with the original (bug #4786); categories are not copied (fixed in later commit)
- **Confirmation**: Shows a "Yes/No" popup dialog before duplicating
- **Post-action**: New game appears in library list, no auto-selection
- **Lessons for CrossHook**: The Lutris confirmation dialog adds unnecessary friction for a non-destructive operation. CrossHook should skip confirmation since duplication is easily reversible (just delete the duplicate).

**Confidence**: High -- verified through GitHub issues #4786, #5447, #3045, and user reports.

#### Heroic Games Launcher

- **No built-in duplicate feature**: Heroic focuses on GOG and Epic game library management with per-game configuration but does not offer a "duplicate game configuration" action
- **Configuration**: Each game has individual settings (Wine/Proton version, launch options) but these are tied to the game identity, not freely duplicable

**Confidence**: High -- confirmed through official documentation and FAQ.

#### Bottles

- **Configuration model**: Each "Bottle" is a complete Wine prefix with its own configuration, runners, and dependencies
- **Duplication**: Bottles supports backup/export of bottles and importing them, but does not have a one-click "duplicate bottle" action
- **Templates**: Bottles uses predefined templates (Gaming, Application, Custom) for new bottle creation rather than cloning existing ones
- **Lessons for CrossHook**: The template approach is interesting but different from CrossHook's profile model. CrossHook profiles are lightweight TOML files, making duplication trivial compared to Bottles' heavy Wine prefix approach.

**Confidence**: Medium -- based on official documentation at docs.usebottles.com and ArchWiki.

#### Steam (Launch Configurations)

- **No duplicate feature**: Steam's per-game launch options are a single text field, not a configuration object that can be duplicated
- **Controller configurations**: Steam Input profiles can be exported/shared but not "duplicated" in the traditional sense; users create new layouts from templates

**Confidence**: High -- Steam's configuration model is fundamentally different from CrossHook's profile system.

### Browsers

#### Firefox

- **Profile Manager**: Allows creating new profiles and manually copying profile directories
- **Naming**: User must provide a name when creating a profile (no auto-generation)
- **No one-click duplicate**: Duplicating a Firefox profile requires manual filesystem operations (copy the profile directory, register it in profiles.ini)
- **Lessons for CrossHook**: Firefox's manual approach is a negative example. CrossHook should make duplication a single-action operation.

**Confidence**: High -- confirmed through Mozilla support documentation.

#### Chrome

- **Profile model**: Chrome profiles are tied to Google accounts; "duplicating" is not a supported workflow
- **Not applicable**: Chrome's profile model is fundamentally different from CrossHook's

### IDEs and Configuration Tools

#### VS Code Profiles

- **Duplication**: "Profiles: Create from Current Profile" command creates a new profile based on the current one
- **Naming**: User provides a name in the profile creation dialog (no auto-generation)
- **Scope**: Copies settings, extensions, keybindings, and other profile contents
- **No live link**: "This creates a copy of the settings in the new profile but does not maintain a link to the profile you used as a source"
- **Known issue**: Clone command occasionally fails on first attempt (GitHub issue #185186)
- **Lessons for CrossHook**: VS Code's explicit "no live link" design matches CrossHook's needs. Duplicated profiles should be fully independent.

**Confidence**: High -- verified through official VS Code documentation.

#### JetBrains IDEs (IntelliJ IDEA, Rider)

- **Duplication**: "Copy Configuration" toolbar button in the Run/Debug Configurations dialog
- **Naming**: Appends a numeric suffix (e.g., "MyConfig (1)")
- **Behavior**: Opens the copied configuration in the editor immediately for modification
- **Folder handling**: Issue IJPL-15870 requested keeping duplicates in the same folder as the original
- **Keyboard shortcut**: Ctrl+D duplicates the current line/selection in the editor (separate from configuration duplication)
- **Lessons for CrossHook**: JetBrains' approach of immediate editing after duplication is the recommended pattern.

**Confidence**: Medium -- behavior confirmed through documentation and YouTrack issues, but exact naming convention sourced indirectly.

#### Figma

- **Duplication**: Ctrl+D or right-click "Duplicate"
- **Naming**: "Name Copy", then "Name Copy 2", "Name Copy 3"
- **Behavior**: New element appears selected on canvas; layers panel scrolls to show it
- **Feedback**: Immediate visual feedback (element appears), no toast notification
- **Lessons for CrossHook**: Figma's immediate selection + scroll-to-new pattern is the gold standard for duplicate feedback.

**Confidence**: High -- well-documented behavior in Figma's design system.

### OS-Level Patterns

#### macOS Finder

- **Naming**: "Name copy", then "Name copy 2", "Name copy 3"
- **Keyboard shortcut**: Cmd+D
- **Behavior**: Duplicate appears in same folder, selected and ready for rename
- **Feedback**: File appears instantly, name is editable

**Confidence**: High -- verified through Apple support documentation and user guides.

#### Windows Explorer

- **Naming**: "Name - Copy", then "Name - Copy (2)", "Name - Copy (3)"
- **Keyboard shortcut**: Ctrl+C then Ctrl+V in same folder
- **Behavior**: Copy appears in same folder
- **Customization**: Naming template is customizable via Windows Registry (CopyNameTemplate value)

**Confidence**: High -- verified through official Microsoft documentation and registry guides.

---

## Recommendations

### Must Have (MVP)

1. **"Duplicate" button** in ProfileActions alongside Save and Delete, styled as `crosshook-button--secondary`
2. **Auto-generated name** using "Name (Copy)" pattern with conflict resolution via numeric suffix
3. **Auto-select new profile** in the profile selector dropdown after duplication
4. **Focus profile name field** with text selected for immediate inline rename
5. **Disable during operations**: Button disabled when no profile loaded, unsaved changes exist, or another operation is in progress
6. **Loading state**: "Duplicating..." button text during the operation
7. **Error handling**: Display errors in existing `crosshook-error-banner` component
8. **Fully independent copy**: No link maintained between source and duplicate (matching VS Code's explicit design)

### Should Have

9. **Keyboard shortcut (Ctrl+D)**: Bind to duplicate action on the Profiles page when a profile is loaded
10. **Tooltip on disabled state**: Explain why the button is disabled (e.g., "Save profile before duplicating")
11. **Skip confirmation dialog**: Duplication is non-destructive and easily reversible; a confirmation dialog adds unnecessary friction (lesson from Lutris's approach)

### Nice to Have (Future)

12. **Contextual controller prompt**: Show "Y: Duplicate" in the ControllerPrompts bar when the Profiles page is active
13. **Undo support**: Allow undoing the duplicate operation (delete the newly created profile) -- low priority since manual deletion is straightforward
14. **Batch duplicate from community profiles**: Allow duplicating a community profile directly into the user's profile list (related to the community taps workflow)

---

## Open Questions

1. **Should the duplicated profile be marked as dirty or clean?** If the profile is saved to disk during duplication (creating a new TOML file), it should be clean. If only created in memory, it should be marked dirty. Recommendation: save to disk immediately (clean state), matching the "create a real file" approach used by macOS Finder and VS Code.

2. **Should exported launchers be duplicated along with the profile?** Lutris explicitly chose NOT to duplicate associated resources (Wine prefixes). For CrossHook, launcher scripts are derived from the profile, so duplicating them would create scripts pointing to the same game. Recommendation: do NOT duplicate launcher exports; the user can re-export from the duplicated profile if needed.

3. **Where should the Duplicate button appear relative to Save and Delete?** Options: (a) Save | Duplicate | Delete, (b) Save | Delete | Duplicate. Recommendation: (a) groups constructive actions (Save, Duplicate) on the left and the destructive action (Delete) on the right, following the convention of placing dangerous actions last.

4. **Should duplicating a profile that has unsaved changes duplicate the saved state or the current (dirty) state?** Recommendation: require saving first (disable Duplicate when dirty). This avoids ambiguity about what is being duplicated and matches the mental model of "I am duplicating this profile" not "I am duplicating my current unsaved edits."

5. **Gamepad focus behavior**: After duplication, should the on-screen keyboard automatically appear (via profile name field focus), or should focus land on the Duplicate button so the user can navigate to the name field manually? Recommendation: auto-focus the name field for consistency with mouse/keyboard flow; the user can dismiss the on-screen keyboard with B if they do not want to rename.

---

## Sources

- [Copy and Duplicate: How are they being used in UX Writing](https://uxwritinghub.com/copy-vs-duplicate-ux-writing/) -- UX Writing Hub
- [UI Copy: UX Guidelines for Command Names and Keyboard Shortcuts](https://www.nngroup.com/articles/ui-copy/) -- Nielsen Norman Group
- [Profiles in Visual Studio Code](https://code.visualstudio.com/docs/configure/profiles) -- VS Code Documentation
- [Lutris: Cloned games share image settings (Issue #4786)](https://github.com/lutris/lutris/issues/4786) -- Lutris GitHub
- [Lutris: Duplicate feature should ask about prefix copying (Issue #5447)](https://github.com/lutris/lutris/issues/5447) -- Lutris GitHub
- [Lutris: Duplicate game settings feature request (Issue #3045)](https://github.com/lutris/lutris/issues/3045) -- Lutris GitHub
- [JetBrains: Keep duplicate in same folder (Issue IJPL-15870)](https://youtrack.jetbrains.com/issue/IJPL-15870) -- JetBrains YouTrack
- [JetBrains: Run/debug configurations](https://www.jetbrains.com/help/idea/run-debug-configuration.html) -- IntelliJ IDEA Documentation
- [How to Make a Copy of Files on Mac with Duplicate](https://osxdaily.com/2018/03/11/make-duplicate-file-folder-mac/) -- OSXDaily
- [How to Change File Copy Name Template in Windows 11](https://winaero.com/how-to-change-file-copy-name-template-in-windows-11/) -- Winaero
- [What is a toast notification? Best practices for UX](https://blog.logrocket.com/ux-design/toast-notifications/) -- LogRocket Blog
- [Bottles Documentation](https://docs.usebottles.com/) -- Bottles
- [Firefox Profile Manager](https://support.mozilla.org/en-US/kb/profile-manager-create-remove-switch-firefox-profiles) -- Mozilla Support
- [Steam Deck Controller Guide](https://steamcommunity.com/sharedfiles/filedetails/?id=2804823261) -- Steam Community
- [Controller Support in Greenlight (Discussion #1051)](https://github.com/unknownskl/greenlight/discussions/1051) -- Greenlight GitHub
- [Radial Menus - Steamworks Documentation](https://partner.steamgames.com/doc/features/steam_controller/radial_menus) -- Valve

---

## Search Queries Executed

1. "UX best practices duplicate clone item desktop application 2024 2025"
2. "Lutris Heroic Bottles game launcher duplicate profile configuration clone"
3. "Steam Deck gamepad UX patterns context menu actions controller navigation"
4. "duplicate naming convention UX 'Copy' 'Copy 1' desktop application pattern"
5. "VS Code duplicate profile configuration clone UX interaction"
6. "Firefox Chrome browser duplicate profile clone naming convention"
7. "'copy vs duplicate' terminology UX writing user interface label best practice"
8. "Lutris duplicate game configuration right click context menu behavior"
9. "JetBrains IntelliJ duplicate run configuration clone naming pattern"
10. "desktop application duplicate action feedback pattern toast notification scroll to new item UX"
11. "Bottles app wine prefix duplicate clone configuration Linux"
12. "Steam Deck big picture mode gamepad context menu action button mapping UI pattern"
13. "Windows duplicate file naming '- Copy' '- Copy (2)' convention standard"
14. "keyboard shortcut duplicate clone item desktop application Ctrl+D standard convention"
15. "Greenlight Xbox streaming app Steam Deck controller support UI navigation context menu"
16. "macOS Finder duplicate file naming convention 'copy' suffix pattern operating system"
17. "Postman Figma duplicate item auto naming scroll to new item interaction pattern"
18. "NNGroup Nielsen Norman duplicate clone action UX guidelines desktop application"
19. "JetBrains IntelliJ 'copy of' 'duplicate' run configuration naming suffix"
20. "gamepad accessible context menu actions Steam Deck application design guidelines"
