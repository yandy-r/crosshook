# UX Research: update-game

## Executive Summary

The update-game feature should be integrated into the existing Install Game page as a collapsible section below the current install shell, not as a separate route. This preserves the mental model that "Install Game" is the setup destination for Proton-based game management and avoids fracturing a small but related workflow across two sidebar entries. The update flow is simpler than install (profile already exists, prefix already exists) and should emphasize profile selection, file picker, pre-flight validation, and a confirmation dialog before applying -- with particular attention to gamepad-friendly file browsing and destructive-action warnings since updates modify files inside a Proton prefix.

**Confidence**: High -- based on analysis of the existing CrossHook codebase architecture, competitive launcher patterns (Lutris, Heroic, Bottles), console-first UI design principles, and established UX guidelines for destructive operations and progress indication.

## User Workflows

### Primary Flow: Apply Game Update

1. **Navigate to Install Game page**: User selects "Install Game" in sidebar (already exists as the `install` route under the "Setup" section).
   - System response: Page loads with the existing Install Game shell at the top and a new "Update Game" section below it.

2. **Expand or scroll to "Update Game" section**: The section is visible by default (no accordion collapse needed at 1280x800) but sits below the install shell. A `crosshook-heading-eyebrow` reading "Update Game" and a brief description anchor the section.
   - System response: Render the update form fields.

3. **Select Profile**: User picks from a dropdown/select of existing profiles that use `proton_run` or `steam_applaunch` launch methods. The select component filters to only profiles with a valid prefix path.
   - System response: Auto-populate the prefix path, Proton path, and display name from the selected profile. Show a read-only summary card with prefix path, current game executable, and launch method.

4. **Select Update Executable**: User taps "Browse" to open a file picker dialog (Tauri's `open` dialog with `.exe` filter) or manually types/pastes a path.
   - System response: Validate that the file exists and has a `.exe` extension. Display the file name prominently.

5. **Pre-flight Validation**: System checks: (a) profile exists and is loadable, (b) prefix directory exists on disk, (c) update executable path is a valid `.exe` file, (d) Proton path from the profile is still valid.
   - System response: Validation summary appears in a status card (reusing the `crosshook-install-card` pattern). Errors appear inline next to the offending field. Green "ready" indicator when all checks pass.

6. **Review and Confirm**: User presses "Apply Update". A confirmation dialog appears:
   - Title: "Apply update to [Profile Display Name]?"
   - Body: "This will run [update.exe] inside the Proton prefix at [prefix_path]. The update may modify game files. This action cannot be automatically undone."
   - Buttons: "Apply Update" (primary, accent color) and "Cancel" (secondary). Default focus lands on "Cancel" to prevent accidental confirmation.
   - System response: On confirm, transition to running state.

7. **Execution**: Backend runs the update executable inside the Proton prefix using the same mechanism as `install_game` (environment setup, Proton invocation).
   - System response: Stage indicator changes to "Running update..." with an indeterminate progress bar. Console output streams in the bottom ConsoleDrawer via the existing `launch-log` event system.

8. **Completion**: Backend returns success/failure result.
   - **Success state**: Status card shows green "Update applied successfully" message with the log path. User can close the section or apply another update.
   - **Failure state**: Status card shows red error message with the log path. "Retry" button becomes available. Error details appear in the console view.

### Alternative Flows

- **No eligible profiles**: If no profiles with Proton-based launch methods exist, the Update Game section shows an informational message: "No Proton-based profiles found. Create a profile with the Install Game shell above or the Profiles page first." with a link/button to navigate to the profiles route.

- **Missing prefix**: If the selected profile's prefix directory does not exist on disk, show a warning: "The prefix at [path] was not found. The game may need to be reinstalled." Disable the "Apply Update" button.

- **Multiple sequential updates**: After a successful update, the form retains the selected profile but clears the update executable field, making it easy to apply another patch to the same game without re-selecting the profile.

- **Update to a non-selected profile**: If the user wants to update a game that was installed outside CrossHook (no profile exists), they should use the "Install Game" flow to create a profile first or use the CLI. The UI should hint at this: "Don't see your game? Create a profile first using the install shell above."

## UI/UX Best Practices

### Industry Standards

**Confidence**: High -- patterns derived from multiple authoritative UX sources and verified against real launcher implementations.

- **Proximity of related tasks**: Install and Update are closely related operations on the same conceptual entity (a game in a Proton prefix). Placing them on the same page follows the proximity principle and reduces navigation overhead. Lutris places its "Run EXE inside Wine prefix" as a right-click context action on an existing game entry. Heroic Games Launcher handles updates through the same queue-based system as installs, accessible from the game's detail page.

- **Progressive disclosure**: Show the Update section below the Install shell but do not require the user to interact with the Install shell to reach it. Each section is self-contained. This matches the approach recommended for in-page sections: content that is "equally important" but contextually related should share a page without requiring tab switches ([Tabs UX, Eleken](https://www.eleken.co/blog-posts/tabs-ux)).

- **Confirmation before destructive action**: Applying an update modifies files in a Proton prefix -- a potentially destructive operation. Use a modal confirmation dialog with task-specific language ("Apply Update" not "OK/Yes"), default focus on the safe action ("Cancel"), and a clear description of consequences. This follows the NN/Group and Smashing Magazine guidance: reserve confirmation dialogs for truly destructive actions and use specific action verbs on buttons ([NN/G](https://www.nngroup.com/articles/confirmation-dialog/), [Smashing Magazine](https://www.smashingmagazine.com/2024/09/how-manage-dangerous-actions-user-interfaces/)).

- **Form validation before submission**: Validate all fields before enabling the primary action button. Inline field-level errors (the existing `crosshook-danger` class pattern) provide immediate feedback. A summary validation state in the status card gives at-a-glance readiness.

- **Reuse existing component patterns**: The update section should reuse `InstallField` for the executable picker (already supports browse with file filters), `ThemedSelect` for profile selection, and the status card pattern from `InstallGamePanel` for showing stage/status/hints. This maintains visual and interaction consistency.

### Gamepad/Controller Navigation

**Confidence**: High -- based on console-first UI design principles and CrossHook's existing gamepad navigation system.

- **Linear focus flow**: The update section should follow a top-to-bottom focus order: Profile Select -> Update Executable field + Browse button -> Apply Update button -> Reset button. The existing `useGamepadNav` hook manages focus zones and directional navigation; the update section should be wrapped in a `data-crosshook-focus-zone="update-game"` container.

- **Browse button as primary file selection**: On Steam Deck, typing file paths with the virtual keyboard (Steam + X) is painful. The "Browse" button that opens Tauri's native file dialog should be the primary interaction. The text input field serves as a display/manual override for advanced users. The Browse button should have a minimum touch target of 48x48px (matching the existing `--crosshook-touch-target-min` CSS variable).

- **Select component navigable via D-pad**: The `ThemedSelect` component already used in the install panel must be navigable via D-pad/analog stick. Profile selection should open on A-button press and allow scrolling with D-pad up/down, confirming with A-button.

- **B-button (back) behavior**: Pressing B in the update section should not navigate away from the page if a form is dirty. The existing `handleGamepadBack` function triggers modal close buttons; the update section does not use a modal for its form, so B-button should either do nothing or scroll to the top of the page.

- **Confirmation dialog gamepad support**: The confirmation modal should auto-focus the "Cancel" button. A-button confirms the focused button. B-button should trigger "Cancel" behavior (matching the existing `onBack` handler pattern in `useGamepadNav`).

### Steam Deck Specific

**Confidence**: High -- based on CrossHook's existing resolution targeting and Steam Deck's known constraints.

- **Resolution**: At 1280x800, vertical space is limited. The update section should be compact: a two-column grid for Profile Select + Update Executable on wider layouts, collapsing to single column at narrow widths. The status card should be a slim single-row summary, not a tall multi-row block. Avoid nesting more than two levels of information hierarchy in the visible area.

- **Input priority**: Controller-first, touch-second, keyboard-last. All interactive elements must be reachable via D-pad focus navigation. Touch targets at least 48px. File paths should be set via the Browse button dialog, not typed manually. The virtual keyboard on Steam Deck is invoked with Steam + X but is awkward for long paths -- the Browse dialog bypasses this entirely.

- **File picker**: Tauri's `dialog.open()` API presents the system file picker, which on Steam Deck (KDE Plasma desktop environment) is the Dolphin-based file dialog. This dialog is navigable via touchscreen and, to a limited extent, via gamepad in desktop mode. However, Steam Deck Gaming Mode does not have a built-in file browser ([Steam Community feature request](https://steamcommunity.com/app/1675200/discussions/2/5135803832908629822/)). CrossHook runs as a desktop application added to Steam, so the system file dialog is available. The UX should not require the user to type full paths -- the Browse button opens the dialog, and the input field shows the result.

- **Scroll behavior**: The Install Page already uses a scrollable content area with scroll-to-top on route change. The update section below the install shell means the user may need to scroll down. Consider adding a quick-jump anchor or ensuring the update section is visible without scrolling when the install shell is in its idle state.

## Error Handling

### Error States

**Confidence**: High -- error patterns derived from the existing InstallGamePanel implementation and UX best practices for destructive operations.

| Error                             | User Message                                                                                                       | Recovery Action                                                        |
| --------------------------------- | ------------------------------------------------------------------------------------------------------------------ | ---------------------------------------------------------------------- |
| No profiles available             | "No Proton-based profiles found. Create a profile with the install shell above or from the Profiles page."         | Navigate to Install or Profiles.                                       |
| Selected profile not found        | "Profile '[name]' could not be loaded. It may have been deleted or renamed."                                       | Re-select a profile from the dropdown.                                 |
| Prefix directory missing          | "The prefix directory at [path] does not exist. The game may need to be reinstalled."                              | Reinstall via the Install shell or fix the prefix path in the profile. |
| Update executable not found       | "The file at [path] does not exist or is not accessible."                                                          | Re-browse for the correct file.                                        |
| Update executable not .exe        | "Expected a Windows executable (.exe) file. The selected file does not have a .exe extension."                     | Select the correct file.                                               |
| Proton path invalid               | "The Proton installation at [path] is no longer available. Update the profile's Proton path in the Profiles view." | Navigate to Profiles to fix the Proton path.                           |
| Update process failed (exit code) | "The update process exited with code [N]. Check the console output for details."                                   | Review console logs, retry, or apply a different update.               |
| Update process crashed/timeout    | "The update process terminated unexpectedly. The prefix may be in an inconsistent state."                          | Review console logs. Consider verifying game files or reinstalling.    |
| Prefix locked by running game     | "The prefix appears to be in use by another process. Close any running games before applying updates."             | Close the running game, then retry.                                    |

### Validation Patterns

- **Profile selection required**: The "Apply Update" button is disabled until a profile is selected. The select component shows a placeholder "Select a profile..." with muted styling.

- **Executable path required**: The "Apply Update" button remains disabled until an update executable path is provided and passes basic validation (non-empty, ends with `.exe`).

- **File existence check**: When the user sets the executable path (via Browse or manual input), perform an async backend check (`invoke('file_exists', { path })`) and show inline feedback. Green help text "File found" or red error "File not found at this path."

- **Prefix existence check**: On profile selection, verify the prefix directory exists. Show a `crosshook-warning-banner` if the prefix is missing, and disable the apply button.

- **Pre-flight summary**: Before the confirmation dialog, display a validation summary card showing: profile name, prefix path (verified), update executable (verified), Proton version. All items show a checkmark or X indicator.

## Performance UX

### Progress Indicators

**Confidence**: Medium -- the exact progress granularity depends on backend implementation. The patterns below are based on established UX guidelines and what is feasible for a Proton-based executable run.

- **Pre-flight validation**: Indeterminate spinner while checking file/prefix existence (typically < 1 second, so may not need a visible spinner). If the check takes > 500ms, show a subtle loading state on the status card.

- **Update execution**: Indeterminate progress bar. Unlike file downloads, running an installer/updater executable inside Wine/Proton does not provide granular progress events -- the process runs opaquely. The status card should show "Running update..." with an animated indeterminate bar (CSS animation on a `crosshook-progress` element). This matches Steam's approach for operations where disk allocation progress is uncertain.

- **Completion**: Transition from indeterminate bar to a static "complete" or "failed" indicator. Use color: green (`--crosshook-color-success`) for success, red (`--crosshook-color-danger`) for failure.

- **Elapsed time**: Display a running timer "Elapsed: 0:42" during execution. This gives users confidence the process is still active, even without granular progress. Update every second via `setInterval`.

### Console Output

**Confidence**: High -- the existing ConsoleView/ConsoleDrawer infrastructure already handles streaming log output.

- **Reuse existing `launch-log` event system**: The update backend command should emit log lines via the same Tauri event system (`launch-log` events) that the ConsoleDrawer already listens to. No new frontend infrastructure is needed for streaming output.

- **Auto-expand console on update start**: When the update begins, if the ConsoleDrawer is collapsed, programmatically expand it (using the `PanelImperativeHandle.expand()` ref already available in `App.tsx`). This ensures the user sees live output without needing to manually open the drawer.

- **Clear console on new update**: Clear previous log lines when a new update operation starts, so the console shows only the current operation's output. The existing `setLines([])` mechanism in ConsoleView supports this.

- **Scroll-to-bottom behavior**: The existing `shouldFollowRef` auto-scroll logic in ConsoleView already handles keeping the view at the bottom for new lines while respecting manual scrollback. No changes needed.

### Loading States While Scanning

- **Profile list loading**: When the update section mounts, it fetches the list of eligible profiles. Show "Loading profiles..." placeholder in the select component while the backend responds.

- **Proton install detection**: The existing `list_proton_installs` call (already in InstallPage) can be shared. The update section does not need to independently fetch Proton installs since it reads the Proton path from the selected profile.

## Competitive Analysis

### Lutris

**Confidence**: High -- based on direct forum research and feature analysis.

- **Approach**: Lutris provides a "Run EXE inside Wine prefix" option as a right-click context menu action on a game entry. It also supports "installer scripts" that can include patch steps (download patch, extract, run). There is no dedicated "Update" UI -- updates are handled either by re-running an updated installer script or manually running an EXE in the prefix.

- **Strengths**: The "Run EXE" approach is flexible and covers any executable, not just updates. Script-based patching supports automated multi-step workflows.

- **Weaknesses**: The UX is widely criticized as "exceedingly obtuse and strange" ([Lutris Forums](https://forums.lutris.net/t/please-explain-how-to-update-games-this-really-needs-to-be-explained/22859)). Users report: dialog boxes appearing behind the main window, patch installers hanging with no progress indication, cryptic error messages like "Missing or Invalid Registry/INI", and no clear documentation on how to apply updates ([Lutris Forums - Patch Help](https://forums.lutris.net/t/help-installing-patch-on-a-manually-installed-game/22817)). Environment variables from system options are not applied when running EXEs in the prefix ([GitHub Issue #4260](https://github.com/lutris/lutris/issues/4260)), breaking some updates.

- **Lessons for CrossHook**: Avoid burying the update action in context menus. Provide explicit progress feedback (even if indeterminate). Ensure all profile environment settings (env vars, DLL overrides) carry through to the update execution. Surface errors clearly with actionable messages.

### Heroic Games Launcher

**Confidence**: High -- based on documentation, wiki, and feature analysis.

- **Approach**: Heroic integrates update functionality directly into the game management flow. Updates are detected automatically through store integrations (Epic/GOG/Amazon) and processed through the same download queue as initial installations. The game page shows an "Update" button when an update is available. Game settings now open in a dialog overlay rather than a separate page, keeping context ([Heroic 2.17.0](https://steamdeckhq.com/news/heroic-launcher-2-17-1-fixes-ui-issues/)).

- **Strengths**: Automatic update detection for supported stores. Queue-based processing with progress tracking. Settings-as-dialog pattern keeps the user grounded in the game context. Filter to show only games with updates available. Full Steam Deck/gamepad support with virtual keyboard for inputs and joystick navigation ([Heroic Wiki - Steam Deck](https://github.com/Heroic-Games-Launcher/HeroicGamesLauncher/wiki/Steam-Deck)).

- **Weaknesses**: Limited to store-managed games -- no support for manually applied patches/updates from external executables. UMU/Proton integration is still evolving.

- **Lessons for CrossHook**: The dialog-over-page pattern for settings/actions is effective for maintaining context. Heroic's approach of keeping update actions on the game page (not a separate update page) validates the "same page" integration strategy for CrossHook.

### Bottles

**Confidence**: Medium -- based on feature analysis and GitHub issues, not direct UI testing.

- **Approach**: Bottles provides a "Run Executable" button at the top of a bottle's detail page that opens a file picker dialog. The selected EXE runs inside that bottle's Wine prefix. There is no dedicated "update" concept -- running a patch EXE is the same action as running any EXE.

- **Strengths**: Simple one-button interaction for running any executable. File picker defaults to the bottle's directory (requested but not always implemented -- [GitHub Issue #3121](https://github.com/bottlesdevs/Bottles/issues/3121)). Automatic runner update checking and repair system for the Wine environment itself.

- **Weaknesses**: The "Run Executable" button does not pre-populate working directory correctly ([GitHub Issue #3476](https://github.com/bottlesdevs/Bottles/issues/3476)). File filter sometimes does not show `.exe` files ([GitHub Issue #3199](https://github.com/bottlesdevs/Bottles/issues/3199)). No progress or output streaming for the running executable. No confirmation before running -- single click executes immediately.

- **Lessons for CrossHook**: Default the file picker to a sensible starting directory (the profile's prefix `drive_c` path or the last-used directory). Always show a confirmation before applying updates. Stream execution output to the console.

### Playnite

**Confidence**: Low -- Playnite is Windows-only (Linux support planned but not available). Limited relevance for Steam Deck UX patterns.

- **Approach**: Playnite acts as a library aggregator and delegates updates to the underlying store clients (Steam, GOG Galaxy, etc.). It does not have its own update/patching mechanism for game files. The upcoming P11 rewrite is targeting multi-platform support via .NET but is still in private development.

- **Strengths**: Clean fullscreen mode designed for controller navigation. Library-centric design with metadata-rich game pages.

- **Weaknesses**: No direct update management -- fully delegates to store clients. Not available on Linux.

- **Lessons for CrossHook**: Playnite's fullscreen/controller mode validates the importance of large touch targets, clear focus indicators, and sequential navigation for gamepad-first interfaces.

### Steam

**Confidence**: High -- Steam is the platform CrossHook integrates with.

- **Approach**: Steam handles game updates automatically through its content distribution system. Updates show in the Downloads page with a redesigned progress interface: total progression bar (including disk allocation), faded progress bars for queued items, and an info tooltip showing update content types (game content, DLC, workshop, shader pre-caching) ([Steam Support](https://help.steampowered.com/en/faqs/view/0C48-FCBD-DA71-93EB)). The "Verify Integrity of Game Files" feature provides a separate validation flow with a progress bar and a summary of files validated/re-downloaded.

- **Strengths**: Comprehensive progress indication with multiple detail levels. Automatic update management. Clear distinction between downloading and disk writing phases. File verification as a repair mechanism.

- **Weaknesses**: No mechanism for applying manual/external patches. The update UX is tailored to Steam-managed content, not user-supplied executables.

- **Lessons for CrossHook**: The differentiation between "downloading" and "applying" phases is a useful mental model even for CrossHook's simpler case. Show distinct phases: "Preparing" -> "Running update" -> "Complete/Failed". Steam's verification-as-repair concept could inspire a future "verify prefix" feature.

## Recommendations

### Must Have

- **Same-page integration**: Add the Update Game section to the existing Install Page (`InstallPage.tsx`) below the `InstallGamePanel` component. Use the same visual language (eyebrow heading, section dividers, status card).

- **Profile-first selection**: The primary input is selecting an existing profile, not configuring paths manually. Auto-populate prefix, Proton, and display name from the profile. This eliminates the most error-prone manual input.

- **Confirmation dialog before apply**: Use a modal confirmation with task-specific language, consequence description, and default focus on "Cancel". Match the existing `ProfileReviewModal` confirmation overlay pattern.

- **Pre-flight validation with inline errors**: Validate profile existence, prefix existence, executable path, and Proton path before enabling the "Apply Update" button. Show errors using the existing `crosshook-danger` class pattern.

- **Console output streaming**: Reuse the `launch-log` event system and ConsoleDrawer for live output during update execution. Auto-expand the console when the update starts.

- **Gamepad-navigable form**: All interactive elements (select, browse button, apply button, reset button) must be reachable via D-pad focus navigation with visible focus indicators. Minimum 48px touch targets.

### Should Have

- **Elapsed time display**: Show a running timer during update execution to indicate the process is still active.

- **Post-update status persistence**: After a successful update, keep the status card showing "Update applied" until the user explicitly resets or selects a different profile. This provides confirmation that the action completed.

- **Smart file picker default directory**: When the Browse dialog opens for the update executable, start in a sensible directory: the profile's prefix `drive_c` path, the directory of the last browsed file, or the user's home directory -- in that priority order.

- **Profile filter**: Only show profiles that use `proton_run` or `steam_applaunch` launch methods in the profile selector. Native-launch profiles do not have a Proton prefix and are not eligible for this workflow.

- **Clear console on new update**: Automatically clear previous console output when starting a new update operation.

### Nice to Have

- **Prefix backup suggestion**: Before applying the update, suggest (via a dismissable info banner) that the user back up the prefix. Provide the prefix path for easy reference. A future iteration could automate prefix snapshotting.

- **Recent updates history**: Track the last N update operations (profile, executable, timestamp, success/failure) and display them in a collapsible "Recent Updates" section.

- **Drag-and-drop executable**: Allow dragging an `.exe` file from a file manager onto the update executable field.

- **Quick-jump from Profiles page**: Add an "Apply Update" action to the profile context menu or detail view that navigates to the Install page and pre-selects the profile in the update section.

### Install vs Update Page Decision

**Recommendation**: Same page, separate section -- not a separate route.

**Rationale**:

1. **Cognitive proximity**: Install and Update are two phases of the same lifecycle (setting up and maintaining a Proton-based game). Users think of "Setup" as a single destination, which the sidebar already reflects with the "Setup" section label containing "Install Game." Adding a second sidebar entry ("Update Game") under "Setup" would work structurally but fractures a small feature across two views unnecessarily.

2. **Reduced navigation overhead**: On Steam Deck with gamepad navigation, every sidebar route switch costs at least two button presses (D-pad to sidebar, D-pad to item, A to select). Keeping Update on the same page means zero navigation cost once the user is on the Install page.

3. **Shared context**: Both operations need Proton install detection, profile context, and the ConsoleDrawer for output streaming. The `InstallPage` already manages Proton installs and profile context; the Update section can share this state without redundant fetches.

4. **UX research alignment**: The [Eleken Tabs UX guide](https://www.eleken.co/blog-posts/tabs-ux) and [Smashing Magazine modal vs page decision tree](https://www.smashingmagazine.com/2026/03/modal-separate-page-ux-decision-tree/) recommend keeping related tasks on the same page when they share context and users switch between them. Standalone pages are better for "complex flows demanding full attention" -- Update is a 3-step flow (select profile, select exe, apply), not complex enough to warrant a dedicated page.

5. **Existing precedent**: Heroic Games Launcher keeps install/update/repair actions on the same game detail page. Bottles keeps "Run Executable" on the bottle's main page alongside other management actions.

**Implementation approach**: Add an `UpdateGameSection` component below `InstallGamePanel` in `InstallPage.tsx`. The section uses its own hook (`useUpdateGame`) for state management but shares the page-level `protonInstalls` state and the `ConsoleDrawer` output channel.

## Open Questions

- **Should the update section be collapsed by default?** At 1280x800, showing both the Install shell and Update section may require scrolling. If the Install shell is the primary use case, collapsing the Update section behind a "Show Update Game" button reduces initial visual load. However, this adds an extra click. Measure whether the Install shell in idle state plus a compact Update section fits without scrolling.

- **Should "Update Game" rename to "Run in Prefix"?** A more generic "Run EXE in Prefix" action would cover not just updates but any executable (config tools, DRM activators, registry editors). This is more flexible but less discoverable for the specific "apply game update" use case. The Bottles model ("Run Executable") supports this generic approach.

- **How should the backend handle Wine dialogs?** Some update installers pop up their own GUI dialogs (InstallShield, NSIS, etc.) that run inside the Proton prefix's virtual desktop. The backend needs to ensure these dialogs are visible to the user -- either by configuring a virtual desktop resolution or by running in windowed mode. This is a backend implementation detail but affects UX: users need to see and interact with the installer's own UI.

- **Should we support running multiple update executables in sequence?** Some games require applying patches in order (e.g., v1.0 -> v1.1 -> v1.2). A future enhancement could accept a list of executables and run them sequentially, but the initial implementation should handle one at a time.

- **What is the interaction between "Update Game" and the ConsoleDrawer?** If the console is collapsed, should it auto-expand? If the user has manually scrolled the console to review previous output, should it clear and jump to new output? The recommendation is to auto-expand and clear, but users who want to keep old logs may object. Consider a "pin console" option in a future iteration.

## Sources

### Game Launchers

- [Lutris v0.5.20 release notes](https://www.gamingonlinux.com/2026/02/game-manager-lutris-v0-5-20-released-with-proton-upgrades-store-updates-and-much-more/) (Feb 2026)
- [Lutris Forums: Game update UX pain points](https://forums.lutris.net/t/please-explain-how-to-update-games-this-really-needs-to-be-explained/22859)
- [Lutris Forums: Patch installation on manually installed game](https://forums.lutris.net/t/help-installing-patch-on-a-manually-installed-game/22817)
- [Lutris GitHub: Run EXE ignores envs](https://github.com/lutris/lutris/issues/4260)
- [Heroic Games Launcher 2.17.0 update](https://steamdeckhq.com/news/heroic-launcher-2-17-1-fixes-ui-issues/)
- [Heroic Games Launcher Steam Deck wiki](https://github.com/Heroic-Games-Launcher/HeroicGamesLauncher/wiki/Steam-Deck)
- [Heroic Settings Interface (DeepWiki)](https://deepwiki.com/Heroic-Games-Launcher/HeroicGamesLauncher/4.4-settings-interface)
- [Bottles GitHub: Run Executable default directory request](https://github.com/bottlesdevs/Bottles/issues/3121)
- [Bottles GitHub: Working directory bug](https://github.com/bottlesdevs/Bottles/issues/3476)
- [Bottles GitHub: File filter bug](https://github.com/bottlesdevs/Bottles/issues/3199)
- [Playnite Linux support plans](https://www.gamingonlinux.com/2025/02/popular-game-launcher-playnite-will-get-linux-support-but-its-still-a-while-away/)
- [Steam Support: Verify Integrity of Game Files](https://help.steampowered.com/en/faqs/view/0C48-FCBD-DA71-93EB)

### Proton/Wine Prefix Management

- [Running exe in existing Proton prefix (GitHub Gist)](https://gist.github.com/michaelbutler/f364276f4030c5f449252f2c4d960bd2)
- [Proton prefix removal for new Wine versions](https://www.gamingonlinux.com/2021/05/proton-ge-gets-some-big-updates-but-you-may-need-to-remove-old-wine-prefixes/)
- [ProtonTricks documentation](https://protontricks.com/)

### UX Design Patterns

- [NN/Group: Confirmation Dialogs Can Prevent User Errors](https://www.nngroup.com/articles/confirmation-dialog/)
- [Smashing Magazine: Managing Dangerous Actions in UIs](https://www.smashingmagazine.com/2024/09/how-manage-dangerous-actions-user-interfaces/)
- [UX Psychology: Destructive Action Modals](https://uxpsychology.substack.com/p/how-to-design-better-destructive)
- [Smashing Magazine: Modal vs Separate Page Decision Tree](https://www.smashingmagazine.com/2026/03/modal-separate-page-ux-decision-tree/)
- [Eleken: Tabs UX Best Practices](https://www.eleken.co/blog-posts/tabs-ux)
- [Mobbin: Progress Indicator Design](https://mobbin.com/glossary/progress-indicator)
- [Microsoft: Progress Controls Guidelines](https://learn.microsoft.com/en-us/windows/apps/develop/ui/controls/progress-controls)
- [Evil Martians: CLI Progress Display Patterns](https://evilmartians.com/chronicles/cli-ux-best-practices-3-patterns-for-improving-progress-displays)
- [Eleken: Wizard UI Pattern](https://www.eleken.co/blog-posts/wizard-ui-pattern-explained)

### Console-First / Gamepad UI Design

- [Game Developer: Console-First UI Design](https://www.gamedeveloper.com/design/secretly-console-first-a-better-approach-to-multi-platform-game-ui-design)
- [Punchev: Crafting Console-Specific UIs](https://punchev.com/blog/crafting-console-specific-user-interfaces)
- [Generalist Programmer: Game UI Design Guide 2025](https://generalistprogrammer.com/tutorials/game-ui-design-complete-interface-guide-2025)
- [OpenGamepadUI (open source gamepad-native launcher)](https://github.com/ShadowBlip/OpenGamepadUI)

### Steam Deck

- [Steam Deck Controller Guide](https://steamcommunity.com/sharedfiles/filedetails/?id=2804823261)
- [Steam Community: File browser in Gaming Mode request](https://steamcommunity.com/app/1675200/discussions/2/5135803832908629822/)
- [Eneba: Steam Deck virtual keyboard guide](https://www.eneba.com/hub/gaming-gear-guides/how-to-open-keyboard-on-steam-deck/)

### Search Queries Executed

1. `Lutris game installation update UX workflow Linux 2025`
2. `Heroic Games Launcher game patching update UX Linux Steam Deck`
3. `Bottles WINE prefix run executable update game Linux UX`
4. `Steam Deck gamepad UI file picker design patterns controller navigation`
5. `Playnite game launcher update UX patterns 2025`
6. `game launcher update patch UX design progress indicator confirmation dialog best practices`
7. `Lutris installer script update game patch mechanism UX flow`
8. `Steam Deck file browser gamepad navigation UX design`
9. `destructive operation confirmation UX pattern gamepad controller interface`
10. `Bottles run executable in prefix UI workflow file selection 2025`
11. `progress bar UX file copy extraction operation long running task best practices 2025`
12. `Heroic Games Launcher game settings page install update UI screenshot`
13. `Steam game update UX progress bar validation verify files interface`
14. `gamepad accessible file picker alternative patterns console UI design`
15. `WINE Proton prefix game update patch apply executable best practices Linux`
16. `"console first" UI design patterns navigation focus management controller gamepad`
17. `tabbed interface install update same page vs separate page UX decision pattern`
18. `Lutris "run exe" "wine prefix" update patch game UI`
19. `wizard stepper UI pattern gamepad controller accessible multi-step form 2025`
20. `Steam Deck virtual keyboard file path input workaround UX`

## Uncertainties and Gaps

- **No direct user testing data**: All recommendations are based on competitive analysis and established UX principles, not user testing with CrossHook's target audience (Steam Deck Linux gamers). Validation through user testing would strengthen confidence.

- **Wine/Proton installer GUI visibility**: How update installer GUIs (InstallShield, NSIS, Inno Setup) render when invoked through Proton is a backend/runtime concern that may affect UX. Some installers may not display correctly or may require specific Proton settings. This needs backend investigation.

- **Exact scroll position at 1280x800**: Whether both the Install shell (in idle state) and the Update section fit on screen without scrolling at 1280x800 depends on final layout sizing. This should be verified with a prototype.

- **Gamepad navigation through Tauri file dialog**: The Tauri `dialog.open()` API delegates to the system file picker (KDE on Steam Deck). Whether this dialog is fully navigable with a gamepad in Gaming Mode (when CrossHook is launched as a non-Steam game) needs testing. If it is not gamepad-navigable, a custom in-app file browser may be needed as a future enhancement.

- **Concurrent update and install operations**: The research did not investigate whether the backend should prevent running an update while an install is in progress (or vice versa). Both operations share the ConsoleDrawer. A mutex or queue would be needed to prevent interleaved log output.
