# UX Research: launcher-delete

## Executive Summary

Launcher lifecycle management (auto-delete on profile delete, auto-rename on profile rename, manual management) must balance two competing UX forces: minimizing orphaned files that confuse users, and preventing accidental data loss from cascading destructive actions. The recommended approach is **automatic cascading with explicit notification** -- when a profile is deleted or renamed, CrossHook should detect associated launchers, inform the user about the cascading effect in a confirmation dialog, and perform the file operations atomically. For manual management in the Launcher Export panel, a lightweight inline confirmation pattern (not a modal) is sufficient because launcher files are low-cost to recreate. All interactions must support gamepad/controller navigation with A-confirm / B-cancel conventions and minimum 44px touch targets for Steam Deck usage.

**Confidence**: High -- based on convergent guidance from Nielsen Norman Group, Smashing Magazine, Cloudscape Design System, and competitive analysis of Steam, Lutris, and Heroic Games Launcher.

## User Workflows

### Primary Flow: Profile Delete with Launcher Cleanup

1. **User selects profile**: User picks an existing profile from the profile selector dropdown.
2. **User clicks Delete**: The existing "Delete" button in ProfileEditor is clicked (or activated via A button on gamepad).
3. **System checks for associated launchers**: Backend scans `~/.local/share/crosshook/launchers/` for a script matching the profile's launcher slug, and `~/.local/share/applications/` for the corresponding `.desktop` entry.
4. **Confirmation dialog appears**: If launchers exist, the dialog clearly states the cascading consequence:
   - Title: "Delete Profile"
   - Body: "This will permanently delete the profile **[Profile Name]** and its associated launcher files:"
   - List: `[slug]-trainer.sh`, `crosshook-[slug]-trainer.desktop`
   - Checkbox (optional): "Keep launcher files" (unchecked by default)
   - Buttons: "Delete Profile" (red/destructive) | "Cancel" (default focus)
5. **System executes**: Profile TOML deleted, then launcher script and `.desktop` entry deleted. If "Keep launcher files" was checked, only the profile is deleted.
6. **Success feedback**: Toast notification: "Profile [Name] and launcher files deleted." with no undo (files are gone).
7. **Fallback on partial failure**: If the profile deletes but a launcher file fails (permission error, file locked), show an error banner with the specific file path and a "Retry" action.

**Confidence**: High -- this flow follows the Cloudscape "delete with simple confirmation" pattern appropriate for medium-severity cascading deletions.

If no launchers exist for the profile, the confirmation should be simpler:

- Title: "Delete Profile"
- Body: "This will permanently delete the profile **[Profile Name]**. This cannot be undone."
- Buttons: "Delete Profile" (red/destructive) | "Cancel" (default focus)

### Primary Flow: Profile Rename with Launcher Update

1. **User changes profile name**: User modifies the profile name field and saves.
2. **System detects rename**: Backend compares the new profile name against the previously loaded profile name. If they differ and the old profile had a launcher exported, a rename cascade is needed.
3. **Automatic rename prompt**: Before saving, a notification panel appears inline (not a modal, to avoid flow disruption):
   - "Renaming this profile will also update the associated launcher files. The launcher display name and file paths will be updated to match."
   - Show old and new file paths for transparency.
   - Button: "Save and Update Launcher" (primary) | "Save Without Updating Launcher" (secondary) | "Cancel"
4. **System executes rename cascade**:
   - Save new profile TOML (with new name).
   - Delete old profile TOML.
   - Regenerate launcher script with new slug and display name.
   - Regenerate `.desktop` entry with new Name field, Exec path, and Comment.
   - Delete old launcher files.
5. **Success feedback**: Inline success banner: "Profile renamed. Launcher files updated."
6. **Fallback**: If old files cannot be deleted (permissions), show a warning: "New launcher created, but old files could not be removed: [paths]. You may delete them manually."

**Confidence**: Medium -- rename cascades are less common in competitive products (most game launchers do not auto-rename shortcuts). This is a differentiating feature for CrossHook. The "regenerate then delete old" approach is safer than in-place rename because it preserves atomicity.

### Manual Launcher Management

1. **Launcher status indicator**: In the Launcher Export panel, after a successful export, show a persistent status section:
   - Status badge: "Exported" (green dot) | "Not Exported" (gray) | "Stale" (amber, when profile has changed since last export)
   - File paths displayed (script and `.desktop` entry).
2. **Delete launcher manually**: A "Delete Launcher" button appears when a launcher exists.
   - Inline confirmation: Button label changes to "Click again to delete" (or "Press A to confirm" on gamepad) with a 3-second timeout before reverting.
   - On confirm: Both launcher files are deleted. Status returns to "Not Exported".
3. **Re-export launcher**: An "Update Launcher" button appears when status is "Stale" (profile changed since last export).
   - No confirmation needed -- this is a non-destructive overwrite.
4. **Manual rename**: Not needed as a separate action. Renaming is handled through the profile rename cascade. The launcher display name is derived from the profile, so changing the profile name and re-exporting achieves the same result.

**Confidence**: High -- inline confirmation for low-severity single-resource deletion aligns with Smashing Magazine's recommendation to avoid modal overuse.

### Alternative Flows

- **Profile deleted but launcher files were manually moved**: The system checks for file existence before attempting delete. If files are not found at expected paths, log a debug message and skip deletion without error. The confirmation dialog should not list files that do not exist.
- **Multiple profiles sharing similar launcher names**: The slug generation already handles uniqueness via `sanitize_launcher_slug()`. However, if two profiles produce the same slug, the newer export overwrites the older. A future enhancement could detect this collision and warn.
- **User exports launcher, then edits profile without re-exporting**: The "Stale" indicator alerts the user. No automatic action is taken -- the user must explicitly re-export or the old launcher remains functional (it references file paths, not profile state).
- **User deletes `.desktop` file outside CrossHook**: On next app load or when viewing the launcher panel, the status check finds the file missing and shows "Not Exported" status. No error is shown -- this is a normal state.

## UI/UX Best Practices

### Destructive Action Patterns

- **Tiered severity approach** (from Cloudscape Design System and Smashing Magazine):
  - _Low severity_ (delete launcher only): Inline confirmation -- button label changes on first click, requires second click within timeout. No modal needed because launchers are trivially cheap to recreate.
  - _Medium severity_ (delete profile + cascading launcher delete): Simple confirmation modal with specific consequence language and red destructive button. Default focus on "Cancel".
  - _High severity_: Not applicable for CrossHook's current scope. Reserve for future operations like bulk-delete-all-profiles.

- **Button labeling** (from NN/G): Use action-specific labels, never "Yes/No". Examples: "Delete Profile and Launcher" / "Cancel", not "Are you sure? Yes / No". The button label should contain both the verb and the noun.

- **Visual differentiation**: Destructive buttons use a red color scheme (`background: rgba(185, 28, 28, 0.16)` with `border: 1px solid rgba(248, 113, 113, 0.28)` matching the existing error styling in the codebase). Non-destructive "Cancel" uses the existing `subtleButtonStyle`.

- **Microcopy**: State consequences explicitly. "This will permanently delete..." rather than "Are you sure?". Bold the resource name in the dialog body to help users verify they are deleting the correct item.

- **Avoid confirmation fatigue**: The confirmation dialog should only appear when there are associated launchers to delete. If the profile has no exported launcher, use a lighter confirmation or even skip it (with an undo toast) since profiles are stored as TOML files and could be re-created.

**Confidence**: High -- these patterns are well-documented across NN/G, Smashing Magazine, and Cloudscape Design System.

### Rename Patterns

- **Inline edit with save**: CrossHook already uses an inline text input for profile name. Renaming is simply editing the name field and clicking Save. This is the correct pattern -- it matches user mental models from file managers (click name, type new name, confirm).

- **Rename cascade notification**: Use an inline notification panel (not a modal) that appears between the name field and the Save button when a rename is detected and launchers exist. This avoids breaking the edit flow while ensuring the user understands the side effect.

- **Name validation**: Validate the new name before allowing save. Check for:
  - Empty name (already handled)
  - Name collision with existing profiles
  - Characters that would produce an empty or colliding launcher slug

- **Slug preview**: When the user edits the launcher display name or profile name, show a live preview of the resulting file paths. This already partially exists in the export result display but should be available pre-export.

**Confidence**: Medium -- inline notification for rename cascades is a custom pattern not widely used in competitors. However, it follows the principle of progressive disclosure and avoids modal fatigue.

### Status Indicators

- **Exported** (green dot + "Exported" label): Launcher files exist at expected paths and match current profile state.
- **Not Exported** (gray dot + "Not Exported" label): No launcher files found for this profile. Shows "Export Launcher" as the primary action.
- **Stale** (amber dot + "Stale -- profile changed since last export" label): Launcher files exist but the profile has been modified since the last export. Shows "Update Launcher" as the primary action. Determined by comparing the profile's last-modified timestamp against the launcher file's mtime, or by storing a hash of the export request in the profile/settings.
- **Error** (red dot + error message): Launcher files expected but inaccessible (permission denied, disk error).

Implementation approach: Use a small colored circle (8-10px) with a text label. Follow the PatternFly status indicator pattern -- use at least two differentiating attributes (color + icon shape or color + text label) for WCAG compliance. Do not rely on color alone.

**Confidence**: High -- status indicators follow established design system patterns (Carbon, PatternFly).

### Gamepad/Controller Considerations

- **A button = Confirm/Select, B button = Cancel/Back**: All confirmation dialogs must map Enter/Space to the confirm action and Escape to cancel. The existing `useGamepadNav` hook maps D-pad to arrow keys and A/B to Enter/Escape, so this should work out of the box with proper keyboard handling.

- **Focus management in dialogs**: When a confirmation dialog opens, focus must move to the dialog and be trapped within it. When using a `<dialog>` HTML element, focus trapping is built-in. Default focus should land on the "Cancel" button (the safe action), not the destructive button. This follows Microsoft's gamepad interaction guidelines.

- **Minimum interactive target size**: All buttons must be at least 44px tall (already the case in the current codebase -- `minHeight: 44` is used consistently). For gamepad navigation, ensure adequate spacing (at least 8px) between adjacent interactive elements to prevent accidental selection when using D-pad.

- **Inline confirmation timing**: The inline confirmation ("Click again to delete") must account for gamepad users who may navigate away and back. Use a timeout (3 seconds) after which the button reverts to its original state. If the user navigates away (blur event), immediately revert.

- **No hover-dependent interactions**: All status tooltips, context menus, and informational popovers must be accessible via focus, not just hover. The Steam Deck does not have a hover state.

- **Navigation order**: In the Launcher Export panel, the tab order should be: Status indicator -> Launcher Name input -> Export/Update button -> Delete button. Destructive actions should be last in tab order to prevent accidental activation.

**Confidence**: High -- based on Microsoft's gamepad interaction design guidelines and the existing `useGamepadNav` implementation in CrossHook.

## Error Handling

### Error States

| Error                                               | User Message                                                                                                        | Recovery Action                                                                                             |
| --------------------------------------------------- | ------------------------------------------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------- |
| Launcher script delete failed (permission denied)   | "Could not delete launcher script: [path]. Permission denied."                                                      | Show "Open folder" button to let user manually delete. Suggest running with appropriate permissions.        |
| `.desktop` entry delete failed (permission denied)  | "Could not delete desktop entry: [path]. Permission denied."                                                        | Same as above.                                                                                              |
| Launcher script deleted but `.desktop` entry failed | "Profile deleted. Launcher script removed, but desktop entry could not be deleted: [path]."                         | Show path and manual deletion instructions. Mark status as "Error".                                         |
| `.desktop` entry deleted but script failed          | "Profile deleted. Desktop entry removed, but launcher script could not be deleted: [path]."                         | Same as above.                                                                                              |
| Launcher files not found during delete              | No error shown. Silent success.                                                                                     | None needed -- files were already gone (manually deleted or moved).                                         |
| Launcher files not found during rename cascade      | "Profile renamed. No existing launcher files were found to update."                                                 | Show "Export Launcher" as the next action.                                                                  |
| Disk full during rename/export                      | "Could not write new launcher files: disk full or write error."                                                     | Show the specific IO error from Rust backend. Suggest freeing disk space.                                   |
| Profile save succeeds but launcher rename fails     | "Profile saved with new name. Launcher files could not be updated: [error]. Old launcher files remain at: [paths]." | Show "Retry Update" button. Old launchers still work.                                                       |
| Slug collision on rename                            | "A launcher with this name already exists. The existing launcher will be overwritten."                              | Show as a warning in the rename notification panel, not an error. Allow user to proceed or change the name. |

### Validation Patterns

- **Profile name**: Validate on blur and on save attempt. Show inline error below the field: "Profile name is required" or "A profile with this name already exists". This validation already exists in the codebase.
- **Launcher slug preview**: Show the derived slug in real-time as the user types a launcher name. If the slug would be empty or would collide with an existing launcher, show an amber warning.
- **File path existence**: Before displaying delete options, verify file existence via the backend. Do not show "Delete Launcher" if no files exist. This prevents confusion where a user clicks delete and nothing visibly happens.

**Confidence**: High -- error states are derived from the existing `SteamExternalLauncherExportError` enum in the Rust backend and standard filesystem error conditions.

## Performance UX

### Loading States

- **File system operations are near-instantaneous for local files**: Deleting or renaming a `.sh` script and a `.desktop` file takes <1ms on any modern filesystem. No loading spinner or progress indicator is needed for individual operations.
- **Button state during operation**: Disable the button and change its label during the async operation (e.g., "Deleting..." / "Updating..."). The codebase already uses this pattern with `isExporting` state. Apply the same pattern for delete and rename operations.
- **Avoid artificial delays**: Do not add fake loading spinners for operations that complete instantly. This adds unnecessary friction, especially on Steam Deck where users may be navigating with a controller.

### Optimistic Updates

- **Profile list after delete**: Immediately remove the deleted profile from the UI profile list before the backend confirms. If the backend delete fails, re-add it and show the error. This matches the existing pattern where `setProfiles(names)` is called after the backend operation completes, but could be improved with an optimistic removal.
- **Launcher status after delete**: Immediately update the status indicator to "Not Exported" when the user confirms deletion. If the backend fails, revert to the previous status and show an error.
- **Launcher status after export/update**: Immediately update to "Exported" when the export command is invoked. If it fails, revert to previous state.
- **Caution for destructive actions**: Optimistic updates for deletes should use a brief "Deleted" state with an undo window (2-3 seconds) before the actual backend call. However, given that launcher files are trivially cheap to recreate, this complexity may not be worth implementing in v1. A simpler approach is to perform the delete synchronously (it is fast) and update the UI on completion.

**Confidence**: Medium -- optimistic updates are a well-known pattern, but their value is limited when the underlying operation is nearly instantaneous. The recommendation is to keep it simple for v1.

### Progress Feedback for Rename Cascades

A rename cascade involves multiple file operations (write new profile, delete old profile, write new script, write new `.desktop`, delete old script, delete old `.desktop`). While each individual operation is fast, the cascade as a whole should be treated as a single atomic operation from the user's perspective:

1. Button shows "Saving..." during the entire cascade.
2. On success: single success banner, not six individual notifications.
3. On partial failure: single error banner listing all failed operations.

## Competitive Analysis

### Steam

- **Approach**: Steam manages game shortcuts through a right-click context menu: "Manage" > "Add/Remove desktop shortcut". Shortcuts are `.desktop` files on Linux or `.lnk` files on Windows. Non-Steam game shortcuts can be renamed via right-click > "Properties" > editing the shortcut name field.
- **Strengths**: Simple mental model -- one toggle to add/remove. No confirmation dialog for shortcut removal (low-severity action). Shortcut state is visible in the context menu ("Remove" only appears if shortcut exists).
- **Weaknesses**: Renaming non-Steam games has documented bugs where editing one field clears all fields (GitHub issue #9524 on steam-for-linux). No cascading rename -- renaming a non-Steam game does not update its desktop shortcut; users must manually remove and re-add. Shortcut deletion sometimes fails silently and shortcuts reappear after restart.
- **Lesson for CrossHook**: Adopt Steam's "one-click toggle" simplicity for launcher existence, but exceed it by automating the rename cascade that Steam lacks. Avoid Steam's pitfall of silent failures -- always show explicit status.

**Confidence**: High -- based on Steam community discussions, bug reports, and direct platform behavior.

### Lutris

- **Approach**: Lutris provides right-click menu options to "Create desktop shortcut", "Create application menu shortcut", and corresponding delete options. When a game is uninstalled from Lutris, associated shortcuts are supposed to be removed automatically.
- **Strengths**: Automatic cleanup on game removal is the correct UX -- users should not have to manually hunt for orphaned shortcuts.
- **Weaknesses**: Significant documented bugs: deleting a game may leave orphaned shortcuts (issue #2243), manually deleting a shortcut breaks the "Create shortcut" option permanently (issue #2146), and version upgrades can break all existing shortcuts (forum reports for v5.8). The delete-shortcut menu item sometimes does nothing. No status indicator for shortcut state.
- **Lesson for CrossHook**: Lutris demonstrates that automatic cleanup is expected behavior, but the implementation must be robust. CrossHook should verify file existence before attempting operations and handle "already deleted" gracefully. A status indicator would have prevented many of Lutris's confusing states.

**Confidence**: High -- based on Lutris GitHub issues and forum discussions documenting persistent shortcut management bugs.

### Heroic Games Launcher

- **Approach**: Heroic uses the `heroic://` protocol for shortcuts, meaning all shortcuts route through the Heroic launcher. Desktop shortcuts and application menu shortcuts are automatically created/removed via the game's context menu. When a game is uninstalled, Heroic automatically removes both desktop and application shortcuts.
- **Strengths**: Automatic cleanup is well-implemented -- logs show explicit "Desktop shortcut removed" and "Applications shortcut removed" during uninstall. The protocol-based approach means shortcuts are inherently tied to Heroic's game registry.
- **Weaknesses**: The `heroic://` protocol requires the launcher to always be running, which is a disadvantage vs direct execution shortcuts. Removing shortcuts from Steam (via "Add to Steam" feature) sometimes fails silently (issue #1905). Uninstalling a game also removes Proton prefixes, which some users did not expect (issue #5011) -- a cascading delete without sufficient warning.
- **Lesson for CrossHook**: Heroic's automatic cleanup on uninstall is the gold standard for this feature. CrossHook should match this behavior. However, CrossHook generates standalone `.sh` scripts (not protocol-based), so the cleanup must be path-based. The Heroic prefix-deletion issue (#5011) is a cautionary tale: always enumerate cascading effects in the confirmation dialog.

**Confidence**: High -- based on Heroic GitHub issues and wiki documentation.

### Desktop Application Managers (Linux)

- **Approach**: Linux desktop environments (GNOME, KDE Plasma, XFCE) rely on `.desktop` files in `~/.local/share/applications/` for application menu entries. Package managers handle cleanup on uninstall. For manually created `.desktop` files, no automatic lifecycle management exists -- users must manually delete them.
- **freedesktop.org spec considerations**: The Desktop Entry Specification requires that compliant implementations not remove fields they do not understand. When renaming, the `Name` field in the `.desktop` file must be updated, and `desktop-file-install --set-name` is the canonical tool for this. CrossHook already generates `.desktop` files directly (via Rust `write_host_text_file`), which is fine for an application-managed file.
- **Orphan cleanup**: Linux has no built-in mechanism for detecting orphaned `.desktop` files. Some system cleanup tools (like Ubuntu's SystemCleanUpTool) attempt to find orphans, but this is not reliable for application-generated entries. CrossHook must manage its own file lifecycle.
- **Lesson for CrossHook**: CrossHook "owns" the launcher files it creates and is solely responsible for their lifecycle. The prefix `crosshook-` in `.desktop` filenames is good practice for namespacing. Consider adding an `X-CrossHook-Profile` key to the `.desktop` file to store the originating profile name, enabling future "scan for orphaned CrossHook launchers" functionality.

**Confidence**: Medium -- based on freedesktop.org specification and Arch Wiki documentation. The custom `X-CrossHook-Profile` key is a recommendation, not an established pattern.

## Recommendations

### Must Have

1. **Cascading launcher delete on profile delete**: When a profile is deleted, detect associated launchers and include them in the confirmation dialog. Delete both files on confirmation. Handle partial failures gracefully.
2. **Confirmation dialog for profile delete with launcher cascade**: Show a specific, consequence-focused modal dialog listing all files that will be deleted. Default focus on "Cancel". Use action-specific button labels ("Delete Profile and Launcher" / "Cancel").
3. **Launcher status indicator**: Show "Exported" / "Not Exported" / "Stale" status in the Launcher Export panel. Verify file existence when the panel renders.
4. **Error handling for all file operations**: Display specific error messages with file paths. Never silently fail. Handle "file not found" as a non-error (already cleaned up).
5. **Gamepad-accessible confirmation dialogs**: Ensure A=confirm, B=cancel mapping. Focus trap within the dialog. Default focus on the safe action.

### Should Have

6. **Cascading launcher rename on profile rename**: When a profile name changes and launchers exist, offer to regenerate launcher files with the new name. Use an inline notification panel, not a modal.
7. **Manual launcher delete from Launcher Export panel**: Add a "Delete Launcher" button with inline confirmation (click-again pattern with timeout). Only visible when launchers exist.
8. **Manual launcher update (re-export)**: Add an "Update Launcher" button visible when status is "Stale". No confirmation needed for non-destructive overwrite.
9. **`X-CrossHook-Profile` metadata in `.desktop` files**: Store the originating profile name in the `.desktop` entry for future orphan detection.

### Nice to Have

10. **Undo for launcher delete**: After deleting launcher files, show a 5-second toast with "Undo" that re-exports the launcher from the profile data (if profile still exists) or from cached export parameters.
11. **Batch launcher management**: A settings-panel section showing all exported CrossHook launchers across all profiles, with bulk delete/update capabilities. Useful for power users with many profiles.
12. **Stale detection via content hash**: Store a hash of the export request parameters in the profile TOML. Compare against current profile state to determine staleness without relying on file modification times.
13. **Slug collision detection and warning**: When exporting or renaming, check if the resulting launcher slug would overwrite an existing launcher from a different profile. Show a warning if so.

## Open Questions

1. **Should profile delete without launchers require a confirmation dialog?** The current codebase has no confirmation. Adding one would be consistent with the "delete with launcher" flow, but risks confirmation fatigue for power users who frequently create/delete test profiles. Recommendation: add a lightweight confirmation (not a full modal -- consider inline confirmation on the Delete button itself) even without launchers.

2. **Should the "Keep launcher files" checkbox exist in the delete dialog?** This adds complexity but covers the edge case where a user wants to delete a profile but keep the standalone launcher script functional. The script references file paths, not profile state, so it would continue to work. Recommendation: include it, unchecked by default.

3. **How should staleness be determined?** Options:
   - (a) Store export timestamp in profile TOML, compare against profile's last-modified time.
   - (b) Store a hash of the export request parameters, compare against current profile state.
   - (c) Check file existence only (simplest, but does not detect content drift).
     Recommendation: option (b) for accuracy, falling back to (c) for v1 simplicity.

4. **Should rename cascade be automatic or opt-in?** Automatic rename ensures consistency but may surprise users. Opt-in (via the inline notification panel) gives users control. Recommendation: opt-in with clear explanation of what will change.

5. **Should CrossHook scan for orphaned launchers on startup?** This would catch launchers left behind by profile deletions that happened outside the app (e.g., manually deleting a TOML file). Recommendation: defer to a future release. For v1, rely on the in-app lifecycle management.

## Sources

- [Confirmation Dialogs Can Prevent User Errors - Nielsen Norman Group](https://www.nngroup.com/articles/confirmation-dialog/)
- [How To Manage Dangerous Actions In User Interfaces - Smashing Magazine](https://www.smashingmagazine.com/2024/09/how-manage-dangerous-actions-user-interfaces/)
- [Confirmation Dialogs Without Irritation - UX Planet](https://uxplanet.org/confirmation-dialogs-how-to-design-dialogues-without-irritation-7b4cf2599956)
- [How to Design Destructive Actions That Prevent Data Loss - UX Movement](https://uxmovement.com/buttons/how-to-design-destructive-actions-that-prevent-data-loss/)
- [Delete Patterns - Cloudscape Design System (AWS)](https://cloudscape.design/patterns/resource-management/delete/)
- [Delete with Additional Confirmation - Cloudscape Design System](https://cloudscape.design/patterns/resource-management/delete/delete-with-additional-confirmation/)
- [Status Indicator Pattern - Carbon Design System (IBM)](https://carbondesignsystem.com/patterns/status-indicator-pattern/)
- [Stale Data Warning - PatternFly (Red Hat)](https://www.patternfly.org/component-groups/status-and-state-indicators/stale-data-warning/)
- [Gamepad and Remote Control Interactions - Microsoft Learn](https://learn.microsoft.com/en-us/windows/apps/design/input/gamepad-and-remote-interactions)
- [Tauri v2 Dialog Plugin](https://v2.tauri.app/plugin/dialog/)
- [Steam Non-Steam Game Shortcut Bugs - GitHub](https://github.com/ValveSoftware/steam-for-linux/issues/9524)
- [Lutris Shortcut Deletion Issues - GitHub](https://github.com/lutris/lutris/issues/2146)
- [Lutris Application Menu Shortcut Issues - GitHub](https://github.com/lutris/lutris/issues/2243)
- [Heroic Games Launcher Steam Shortcut Removal - GitHub](https://github.com/Heroic-Games-Launcher/HeroicGamesLauncher/issues/1905)
- [Heroic Prefix Deletion on Uninstall - GitHub](https://github.com/Heroic-Games-Launcher/HeroicGamesLauncher/issues/5011)
- [Direct Desktop Shortcuts Feature Request - Heroic GitHub](https://github.com/Heroic-Games-Launcher/HeroicGamesLauncher/issues/3060)
- [Desktop Entry Specification - freedesktop.org](https://specifications.freedesktop.org/desktop-entry-desktop-entry-spec-latest.html)
- [Desktop Entries - ArchWiki](https://wiki.archlinux.org/title/Desktop_entries)
- [Optimistic UI Pattern - freeCodeCamp](https://www.freecodecamp.org/news/how-to-use-the-optimistic-ui-pattern-with-the-useoptimistic-hook-in-react/)
- [Toast Notification Best Practices - MagicBell](https://www.magicbell.com/blog/what-is-a-toast-message-and-how-do-you-use-it)
- [Focus Trap in Modals - UXPin](https://www.uxpin.com/studio/blog/how-to-build-accessible-modals-with-focus-traps/)
- [No Need to Trap Focus on Dialog Element - CSS-Tricks](https://css-tricks.com/there-is-no-need-to-trap-focus-on-a-dialog-element/)

## Search Queries Executed

1. "destructive action confirmation dialog UX best practices 2024 2025"
2. "Steam game shortcut management delete rename UX"
3. "Lutris game launcher shortcut management delete UX Linux"
4. "Heroic Games Launcher shortcut desktop entry management Linux 2024"
5. "inline rename UX pattern file manager best practices"
6. "Steam Deck gamepad controller confirmation dialog UX design patterns"
7. "undo pattern toast notification delete action UX design 2024"
8. ".desktop file management Linux application shortcut lifecycle stale orphan cleanup"
9. "optimistic UI updates file system operations delete rename UX pattern"
10. "Smashing Magazine dangerous actions user interfaces 2024 confirmation patterns"
11. "Heroic Games Launcher remove game uninstall shortcut desktop entry delete flow"
12. "status indicator badge stale outdated sync state UI component design pattern"
13. "cascading delete associated resources UX pattern notify user side effects"
14. "Tauri v2 dialog confirmation modal pattern React component"
15. "gamepad accessible modal dialog focus trap controller navigation A button confirm B button cancel"
16. "Linux desktop entry .desktop file rename update Name field best practices freedesktop"
17. "game launcher profile rename cascading update associated files UX"

## Uncertainties and Gaps

1. **No direct competitor implements profile-rename-to-launcher-rename cascade**: This is a novel feature for CrossHook. No established UX convention exists for this specific flow. The recommended inline notification approach is based on general UX principles rather than proven game launcher patterns.

2. **Tauri v2 native dialog limitations**: Tauri's built-in `confirm()` dialog does not support custom content (checkboxes, file lists). Implementing the recommended cascading-delete confirmation dialog with file list and "Keep launcher files" checkbox will require a custom React modal component, not the native Tauri dialog.

3. **Gamepad interaction testing**: The Microsoft gamepad interaction guidelines are Windows/UWP-focused. CrossHook's `useGamepadNav` hook implements custom gamepad navigation in a web context. Confirmation dialog focus management with gamepad requires testing on actual Steam Deck hardware to verify behavior.

4. **Staleness detection accuracy**: The recommended content-hash approach for staleness detection has not been validated in the CrossHook architecture. The profile TOML format may need a new field to store the last export hash.

5. **Undo feasibility for launcher delete**: Implementing undo for file deletion requires either soft-delete (move to a temp location) or caching the export parameters. The current architecture does not have a undo/history mechanism. This is categorized as "Nice to Have" and may be deferred.
