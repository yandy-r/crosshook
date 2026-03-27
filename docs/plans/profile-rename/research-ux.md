# UX Research: Profile Rename

## Executive Summary

The current CrossHook profile editor conflates "rename" with "save as new" — editing the profile name field and saving creates a new profile while the old one persists. This is a well-documented UX anti-pattern that causes confusion, profile sprawl, and data management friction. Research across desktop applications, game launchers, design systems, and accessibility standards strongly recommends implementing rename as a **distinct, explicit operation** separate from the save workflow.

The recommended approach is a **dedicated rename action** (context menu or action bar button) that opens a **lightweight modal dialog** with inline validation, optimistic UI updates, and an undo toast — balancing gamepad accessibility on Steam Deck with the precision needed for name conflict resolution.

**Confidence**: High — supported by PatternFly, Carbon, NNGroup guidelines, competitive analysis of 5+ launchers, and Steam Deck input documentation.

---

## User Workflows

### Primary Flow: Explicit Rename via Action Bar

**Trigger**: User clicks "Rename" button in the profile action bar (next to Save/Duplicate/Delete).

1. User selects an existing profile from the dropdown
2. User clicks "Rename" button (or presses gamepad shortcut)
3. A compact modal dialog appears with:
   - Current name pre-filled and fully selected
   - Inline validation as user types
   - "Rename" (primary) and "Cancel" (secondary) buttons
4. User types new name
5. Validation runs on blur or debounced keystroke (300ms)
6. User confirms via Enter key, "Rename" button click, or gamepad A button
7. Profile is renamed atomically on backend
8. Profile list refreshes, selection updates to new name
9. Success toast appears: "Profile renamed to 'New Name'" with Undo action
10. Associated launchers/desktop entries cascade-update automatically

**Why modal over inline**: The profile name field in `ProfileFormSections` serves double duty — it's used for both creating new profiles and editing existing ones. Making it "sometimes rename, sometimes create" based on context would be confusing. A separate modal makes the intent unambiguous.

**Confidence**: High — PatternFly and Carbon design systems both recommend separating edit-in-place from complex rename operations when side effects exist.

### Alternative Flow A: Context Menu Rename

**Trigger**: User right-clicks (or long-presses on gamepad) a profile in the dropdown.

1. Context menu appears with: Load, Rename, Duplicate, Delete
2. User selects "Rename"
3. Same modal dialog flow as primary flow (steps 3-10)

This mirrors how Bottles handles bottle renaming (three-dot menu > Rename) and how Steam handles custom game names (right-click > Manage > Set Custom Name).

**Confidence**: Medium — requires context menu infrastructure that may not exist in the current dropdown component.

### Alternative Flow B: Inline Edit in Dropdown

**Trigger**: User double-clicks or presses F2 on a profile name in the dropdown list.

1. Profile name becomes an editable text field inline
2. Original name is fully selected
3. User types new name
4. Enter confirms, Escape cancels
5. Inline validation shows errors below the field
6. On confirm, same backend flow as primary flow

**Not recommended for CrossHook** because:

- The profile "dropdown" is a `<select>`-like component, not a list with individually editable items
- Inline editing in a select/dropdown has poor gamepad accessibility
- No standard pattern exists for inline editing within a selector

**Confidence**: High (that this is NOT the right approach for CrossHook's current component architecture).

### Alternative Flow C: Rename on Save (Current Behavior, Improved)

**Trigger**: User changes the name field in the profile editor and clicks Save.

If the old name differs from the new name AND the old profile exists:

1. Show a disambiguation dialog: "You changed the profile name. What would you like to do?"
   - "Rename to 'New Name'" (renames existing profile)
   - "Save as New Profile" (keeps old, creates new)
   - "Cancel" (revert name change)
2. Proceed based on user choice

**Pros**: No new UI elements needed, leverages existing workflow.
**Cons**: Adds friction to every rename, the disambiguation dialog interrupts flow, and users must remember to change the name field before saving.

**Confidence**: Medium — functional but suboptimal; creates a decision point where users may not understand the consequences.

### Decision Points Summary

| Decision                                  | Recommendation                                        | Confidence |
| ----------------------------------------- | ----------------------------------------------------- | ---------- |
| Separate rename action vs overloaded save | Separate action                                       | High       |
| Modal dialog vs inline edit               | Modal dialog                                          | High       |
| Context menu vs action bar button         | Action bar button (primary), context menu (secondary) | Medium     |
| Undo toast vs confirmation dialog         | Undo toast                                            | High       |
| Optimistic UI vs wait-for-backend         | Wait-for-backend (rename has side effects)            | High       |

---

## UI/UX Best Practices

### Industry Standards for Rename Operations

#### 1. Rename as a Distinct Operation

Every major desktop environment treats rename as a separate, explicit action — never conflated with "save" or "save as":

- **Windows Explorer**: F2 key, right-click > Rename, or slow double-click triggers inline edit
- **macOS Finder**: Enter key or right-click > Rename triggers inline edit
- **Linux file managers (Nautilus, Dolphin, Thunar)**: F2 key or right-click > Rename
- **VS Code**: Profiles editor with dedicated rename in overflow menu
- **Steam**: Right-click > Manage > Set Custom Name

**Pattern**: Rename is always an intentional, user-initiated action — never a side effect of saving.

**Confidence**: High — universal across all major desktop platforms.

Sources:

- [Smart Ways to Rename Files](https://www.namequick.app/blog/how-to-rename-a-file)
- [Files Community Issue #9591](https://github.com/files-community/Files/issues/9591)

#### 2. Modal Dialog for Rename (When Side Effects Exist)

PatternFly's inline edit guidelines state that inline editing is appropriate when "all editable elements can be viewed within the row" and "the data needs to be updated frequently." However, when a rename operation has cascading side effects (updating launchers, desktop entries, settings references), a modal dialog is preferred because:

- It creates a clear "transaction boundary" — the user knows exactly when the rename happens
- It provides space for validation feedback and conflict resolution
- It's naturally accessible via keyboard and gamepad (focus trapping, Enter/Escape)
- It prevents accidental renames from casual editing

**Confidence**: High — PatternFly, Carbon, and NNGroup all recommend modal for operations with side effects.

Sources:

- [PatternFly Inline Edit Guidelines](https://www.patternfly.org/components/inline-edit/design-guidelines/)
- [Carbon Edit Pattern](https://carbondesignsystem.com/community/patterns/edit-pattern/)

#### 3. Pre-filled, Fully Selected Text

When opening a rename dialog or inline edit:

- Pre-fill with the current name
- Select all text (so typing immediately replaces)
- Position cursor at end if user clicks into the field

This mirrors the universal rename pattern in file managers and IDEs.

**Confidence**: High — universal standard.

#### 4. Undo Over Confirm for Non-Destructive Actions

NNGroup and UX research strongly favor **undo** over **confirmation dialogs** for rename operations:

> "The main benefit of allowing the user to undo is that the interface doesn't second guess the user. The interface does what it's supposed to do without asking the user if they're sure." — Josh Wayne, [Confirm or Undo?](https://joshwayne.com/posts/confirm-or-undo/)
> "Use a confirmation dialog before committing to actions with serious consequences — such as destroying users' work." — [NNGroup, Confirmation Dialogs](https://www.nngroup.com/articles/confirmation-dialog/)

Rename is easily reversible (rename back), so a **toast with undo** is the optimal feedback pattern:

- Show success toast: "Renamed to 'New Name'" with [Undo] button
- Undo window: 5-8 seconds
- If undone, revert to original name atomically

**Confidence**: High — NNGroup, A List Apart, and UX Movement all recommend undo for reversible operations.

Sources:

- [Confirm or Undo?](https://joshwayne.com/posts/confirm-or-undo/)
- [NNGroup Confirmation Dialogs](https://www.nngroup.com/articles/confirmation-dialog/)
- [Never Use a Warning When You Mean Undo](https://alistapart.com/article/neveruseawarning/)

### Accessibility

#### Keyboard Navigation

| Key    | Action                                             |
| ------ | -------------------------------------------------- |
| Enter  | Confirm rename                                     |
| Escape | Cancel rename, restore original name               |
| Tab    | Move focus to next control (Rename/Cancel buttons) |
| F2     | Open rename dialog (standard desktop shortcut)     |

#### ARIA Requirements

- Rename dialog: `role="dialog"`, `aria-modal="true"`, `aria-labelledby` pointing to dialog title
- Input field: `<input type="text">` with visible `<label>` (preferred over `aria-label`)
- Error messages: `aria-describedby` linking input to error text, `role="alert"` for dynamic errors
- Focus management: Auto-focus input on dialog open, return focus to trigger on close

**Confidence**: High — WCAG 2.1 AA and ARIA Authoring Practices.

Sources:

- [MDN ARIA textbox role](https://developer.mozilla.org/en-US/docs/Web/Accessibility/ARIA/Roles/textbox_role)
- [Keyboard Accessibility - Microsoft Learn](https://learn.microsoft.com/en-us/windows/apps/design/accessibility/keyboard-accessibility)

### Gamepad / Steam Deck Considerations

#### Virtual Keyboard Integration

Steam Deck requires text input to trigger the on-screen keyboard. Key recommendations from Valve's Steamworks documentation:

> "Games are strongly recommended to automatically display an on-screen keyboard when requiring the user to input text."

For Tauri v2 apps on Steam Deck:

- The rename dialog input field should request focus automatically
- Steam's `ShowFloatingGamepadTextInput` or `ShowGamepadTextInput` APIs may not be available in non-game contexts — the user will use Steam+X to invoke the keyboard
- Design the dialog to accommodate the virtual keyboard overlay (bottom half of screen)

#### Dialog Sizing for Steam Deck

- Steam Deck display: 1280x800 (7" screen)
- CrossHook's Tauri config targets 1280x800
- Rename dialog should be compact: ~400px wide, centered vertically in the top half of the screen
- Input field and buttons should use minimum 44px touch targets (WCAG 2.5.5)
- Font size: minimum 16px to prevent zoom on mobile-like inputs

#### Gamepad Button Mapping

| Gamepad Button | Action                                                   |
| -------------- | -------------------------------------------------------- |
| A              | Confirm selection / Open keyboard / Press focused button |
| B              | Cancel / Close dialog                                    |
| D-pad Up/Down  | Navigate between input field and buttons                 |
| Start          | Confirm rename (alternative)                             |

**Confidence**: High — based on Valve's Steamworks documentation and CrossHook's existing gamepad navigation hook.

Sources:

- [Steamworks: Getting your game ready for Steam Deck](https://partner.steamgames.com/doc/steamdeck/recommendations)
- [How to Use Virtual Keyboard on Steam Deck](https://gamerant.com/steam-deck-virtual-keyboard-how-use-game-mode-desktop/)

---

## Error Handling

### Error States Table

| Error Condition          | Trigger                                   | Message                                                                                | Severity | Recovery              |
| ------------------------ | ----------------------------------------- | -------------------------------------------------------------------------------------- | -------- | --------------------- |
| Empty name               | User clears input field                   | "Profile name cannot be empty."                                                        | Blocking | Type a name           |
| Name conflict            | Name matches existing profile             | "A profile named '[name]' already exists."                                             | Blocking | Choose different name |
| Same name                | New name equals current name              | Dismiss dialog silently (no-op)                                                        | None     | N/A                   |
| Invalid characters       | Name contains `/`, `\`, or null bytes     | "Profile names cannot contain / or \\ characters."                                     | Blocking | Remove invalid chars  |
| Name too long            | Exceeds filesystem limit (~255 bytes)     | "Profile name is too long (max 255 characters)."                                       | Blocking | Shorten name          |
| Whitespace-only          | Name is only spaces/tabs                  | "Profile name cannot be blank."                                                        | Blocking | Type a real name      |
| Backend failure          | File I/O error, permission denied         | "Failed to rename profile: [backend error]"                                            | Error    | Retry or cancel       |
| Launcher cascade failure | Profile renamed but launcher update fails | "Profile renamed, but launcher update failed: [error]. Re-export the launcher to fix." | Warning  | Re-export launcher    |

### Validation Patterns

#### When to Validate

- **On input change (debounced 300ms)**: Check for empty, whitespace-only, invalid characters, length
- **On blur**: Check for name conflicts (requires profile list lookup)
- **On submit**: Final validation gate before IPC call

This follows the research consensus: "Validation should occur after the user leaves a value in a field rather than pre-emptively." Debounced validation during typing is acceptable for fast, local checks (empty/invalid chars) but conflict checking should wait for blur or submit to avoid flickering errors.

**Confidence**: High — LogRocket, Pencil & Paper, and UX Design Bootcamp all recommend this timing pattern.

Sources:

- [Error Handling UX Design Patterns](https://medium.com/design-bootcamp/error-handling-ux-design-patterns-c2a5bbae5f8d)
- [Error Message UX, Handling & Feedback](https://www.pencilandpaper.io/articles/ux-pattern-analysis-error-feedback)

#### Error Display

- Inline error text below the input field, colored red (`#ff6b6b` or similar)
- Input field border changes to red on error
- Error text uses `role="alert"` for screen reader announcement
- "Rename" button disabled while errors are present
- Error clears automatically when user corrects the input

#### Name Conflict Resolution

When a user enters a name that already exists, offer clear options:

1. **Primary**: Show inline error "A profile named 'X' already exists." — user must choose a different name
2. **Do NOT offer to overwrite** — overwriting is a destructive action that conflates rename with replace and should never be silently available in a rename flow
3. **Critical**: The backend currently performs **silent overwrite** if target name exists — frontend conflict validation is safety-critical, not optional

**Confidence**: High — overwrite-on-rename is a well-known source of data loss in file managers. Backend silent overwrite confirmed by API research.

---

## Performance UX

### Loading States

#### Rename Dialog Submission

The profile rename backend operation is a **filesystem rename syscall** — near-instant (sub-millisecond) for the core rename, with minimal additional time for launcher cascade and settings update. No loading spinner or progress indicator is needed.

**Updated flow** (based on teammate API research confirming near-instant operation):

1. **On submit**: Disable input and buttons briefly (prevent double-click)
2. **Duration**: < 10ms typical — the user will not perceive any delay
3. **On success**: Close dialog immediately, update profile list, show success toast
4. **On failure**: Re-enable input, show error inline, keep dialog open

A "Renaming..." spinner state is unnecessary and would add visual noise. The operation completes before any spinner would render.

#### Why NOT Optimistic UI for Rename

Despite the near-instant backend, optimistic UI is still **not the right pattern** because:

- The operation is so fast that "wait for confirmation" and "optimistic" are indistinguishable to the user
- Rename has cascading side effects (launchers, desktop entries, settings) that should be confirmed before reflecting in UI
- Error handling is simpler when the UI waits for the backend (no rollback needed in React state)

**Confidence**: High — the near-instant operation makes this a non-issue; just await the IPC call.

#### Critical Backend Hazard: Silent Overwrite

The current backend implementation **silently overwrites** if the target name already exists (confirmed by API research). The frontend **must** validate for name conflicts before calling `profile_rename` — the inline error "A profile named 'X' already exists" is a safety-critical validation, not just a UX nicety.

### Feedback Timing

| Phase                    | Duration                              | UI Feedback                   |
| ------------------------ | ------------------------------------- | ----------------------------- |
| Dialog open              | Instant                               | Focus on input, text selected |
| Validation               | < 50ms (local)                        | Inline error/clear            |
| Conflict check           | < 1ms (in-memory profile list lookup) | Inline error if conflict      |
| Backend rename + cascade | < 10ms                                | Buttons disabled momentarily  |
| Success                  | Instant after backend                 | Dialog closes, toast appears  |
| Undo window              | 5-8 seconds                           | Toast with Undo button        |

### Offline / Error Recovery

- If backend fails, dialog stays open with error — user can retry or cancel
- No offline mode needed (Tauri IPC is always local)
- If the app crashes mid-rename, the backend should ensure atomicity (rename fully completes or fully rolls back)

---

## Competitive Analysis

### Game Launchers

| Launcher    | Rename Support                  | UI Pattern                                                                            | Cascading Updates                                                                                                                | Gamepad Support                                    |
| ----------- | ------------------------------- | ------------------------------------------------------------------------------------- | -------------------------------------------------------------------------------------------------------------------------------- | -------------------------------------------------- |
| **Steam**   | Custom name for library display | Right-click > Manage > Set Custom Name — opens inline text field in Properties dialog | Desktop shortcuts not updated; library-only                                                                                      | Full gamepad in Game Mode; limited in Desktop Mode |
| **Bottles** | Bottle display name             | Settings page text field with checkmark confirm                                       | **Broken** — desktop entries reference old name after rename (Issue [#1392](https://github.com/bottlesdevs/Bottles/issues/1392)) | Limited; Flatpak GTK app                           |
| **Lutris**  | Game configuration name         | Right-click > Configure > edit name field in dialog                                   | Manual re-export needed                                                                                                          | No native gamepad support                          |
| **Heroic**  | No rename feature found         | N/A — games use store names                                                           | N/A                                                                                                                              | Partial gamepad via Steam Input                    |
| **VS Code** | Profile rename                  | Profiles editor > overflow menu > edit name inline                                    | Workspace associations auto-update                                                                                               | N/A (desktop IDE)                                  |

### Key Competitive Insights

1. **Bottles is a cautionary tale**: Their rename feature is buggy (Issue [#2304](https://github.com/bottlesdevs/Bottles/issues/2304) — rename fails silently) and breaks desktop entries (Issue [#1392](https://github.com/bottlesdevs/Bottles/issues/1392)). CrossHook must handle launcher cascade correctly or users will have broken `.desktop` entries and shell scripts.

2. **Steam separates display name from identity**: Steam's "Set Custom Name" only changes the library display — it doesn't rename the app manifest or affect launch shortcuts. This is simpler but less powerful than a true rename.

3. **No launcher does rename well**: None of the Linux game launchers surveyed have a polished, reliable rename flow with proper cascading. This is an opportunity for CrossHook to set a standard.

4. **VS Code's profile management is the gold standard**: Create, rename, delete, duplicate, import/export — all accessible from a unified Profiles editor. CrossHook's profile action bar (Save, Duplicate, Delete) is already heading in this direction; adding Rename completes the set.

**Confidence**: High for Steam, Bottles, VS Code; Medium for Lutris, Heroic (limited documentation available).

Sources:

- [Bottles Issue #2304 - Cannot rename bottle](https://github.com/bottlesdevs/Bottles/issues/2304)
- [Bottles Issue #1392 - Desktop entry breaks on rename](https://github.com/bottlesdevs/Bottles/issues/1392)
- [VS Code Profiles](https://code.visualstudio.com/docs/configure/profiles)
- [Steam: Changing Your Game's Name](https://partner.steamgames.com/doc/store/editing/name)

---

## Recommendations

### Must Have (P0)

1. **Dedicated "Rename" button in ProfileActions** — placed between Duplicate and Delete, disabled when no saved profile is selected. This makes rename a first-class operation, eliminating the "edit name field + save = accidental new profile" confusion.

2. **Modal rename dialog** — compact dialog with pre-filled + fully-selected current name, inline validation, Enter to confirm, Escape to cancel. Modal is preferred over inline for gamepad accessibility and because rename has cascading side effects.

3. **Inline validation** — empty name, invalid characters, name conflict, and length checks with clear error messages below the input field. Validate on debounced input (300ms) for local checks, on blur for conflict checks.

4. **Backend atomicity** — rename must be all-or-nothing: profile file renamed, launcher references updated, settings references updated, or everything rolled back. Learn from Bottles' mistakes (Issues #1392, #2304).

5. **Launcher cascade** — when a profile is renamed, automatically update associated `.sh` scripts and `.desktop` entries. Show a warning toast if cascade partially fails ("Profile renamed, but launcher update failed — re-export to fix.").

6. **Profile name field becomes read-only for existing profiles** — when editing an existing profile, the name field in ProfileFormSections should be read-only (or visually de-emphasized with a lock icon) to prevent the "edit name + save = new profile" confusion. Rename is done via the dedicated button.

### Should Have (P1)

7. **Success toast with Undo** — "Renamed to 'New Name'" toast with [Undo] button, 5-8 second window. Undo atomically reverts the rename (profile file, launchers, settings). This follows NNGroup's recommendation to prefer undo over confirmation for reversible actions.

8. **Keyboard shortcut (F2)** — when a profile is selected, F2 opens the rename dialog. This matches the universal desktop rename shortcut (Windows, Linux file managers).

9. **Gamepad-optimized dialog layout** — dialog positioned in the top half of the screen (below virtual keyboard overlay), 44px minimum touch targets, 16px+ font, D-pad navigation between input and buttons.

10. **Disambiguation on name change + save** — if the user manages to change the name field and clicks Save (e.g., name field is editable for new profiles), show: "Do you want to rename 'Old' to 'New', or save as a new profile?" This is a safety net, not the primary flow.

### Nice to Have (P2)

11. **Context menu rename** — right-click (or long-press on gamepad) on a profile in the dropdown to access Rename, Duplicate, Delete. This provides a secondary entry point familiar to desktop users.

12. **Animated transition** — when profile list updates after rename, smoothly animate the item to its new position (if list is sorted alphabetically) rather than jumping.

13. **Batch rename** — if CrossHook ever supports selecting multiple profiles, batch rename with find/replace could be useful. Not needed for initial implementation.

14. **Rename history** — track previous names in profile metadata for disambiguation when users have many similarly-named profiles. Low priority.

---

## Open Questions

1. **Should the name field be fully read-only for existing profiles, or just visually discouraged?** Making it read-only is cleaner but means users can't use the field for "save as new" (which Duplicate already covers). Fully read-only with Duplicate for copying is the cleaner model.

2. **What characters should be forbidden in profile names?** TOML file names on Linux allow most characters, but `/`, `\`, null bytes, and leading/trailing dots should be forbidden for filesystem safety. Should we also forbid `:`, `*`, `?`, `"`, `<`, `>`, `|` for potential future cross-platform compatibility?

3. **Should rename update `last_used_profile` in settings?** Yes — if the user's last-used profile was "Old Name" and they rename it to "New Name", settings should reflect the new name. The current `syncProfileMetadata` pattern already handles this for save; rename should do the same.

4. **How should the Undo toast interact with subsequent operations?** If the user renames a profile and then immediately deletes it before the undo window expires, the undo should be invalidated (dismiss the toast). Similarly, if the user renames and then renames again, the first undo should be invalidated.

5. **Should renaming a profile also rename its TOML filename?** The current implementation uses the profile name as the TOML filename. Renaming must update the filename to maintain this invariant. If a future migration decouples display name from filename (like Bottles does with name vs path), this becomes simpler but adds complexity elsewhere.

---

## Search Queries Executed

1. "UX best practices rename operation desktop application inline editing vs dialog 2025"
2. "Steam Deck gamepad controller text input rename UX patterns"
3. "Lutris Heroic game launcher profile rename UX interaction pattern"
4. "inline rename pattern file manager desktop app accessibility keyboard UX 2025"
5. "Bottles flatpak game launcher rename game configuration prefix 2025"
6. "rename confirmation dialog UX pattern undo vs confirm desktop application"
7. "rename vs save as new UX confusion duplicate name validation error handling patterns"
8. "optimistic UI update rename operation loading state feedback timing best practice"
9. "Steam client game properties rename shortcut game management UX 2025"
10. "ARIA accessibility pattern inline edit rename editable text field role specification"
11. "PatternFly Carbon design system inline edit component rename pattern"
12. "toast notification undo pattern react implementation rename feedback"
13. "Steam Deck virtual keyboard on-screen keyboard text input UX design gamepad text entry"
14. "Bottles app rename bottle WINE prefix UX right-click context menu"
15. "Lutris Linux game launcher rename game configuration right-click 2024 2025"
16. "Heroic Games Launcher game rename custom name feature GitHub"
17. "NNGroup inline editing design pattern guidelines usability"
18. "profile rename cascading side effects update references launchers shortcuts best practice"

## Sources

- [PatternFly Inline Edit Design Guidelines](https://www.patternfly.org/components/inline-edit/design-guidelines/)
- [Carbon Design System Edit Pattern](https://carbondesignsystem.com/community/patterns/edit-pattern/)
- [NNGroup: Confirmation Dialogs Can Prevent User Errors](https://www.nngroup.com/articles/confirmation-dialog/)
- [Josh Wayne: Confirm or Undo?](https://joshwayne.com/posts/confirm-or-undo/)
- [A List Apart: Never Use a Warning When You Mean Undo](https://alistapart.com/article/neveruseawarning/)
- [Bottles Issue #2304 - Cannot rename bottle](https://github.com/bottlesdevs/Bottles/issues/2304)
- [Bottles Issue #1392 - Desktop entry breaks on rename](https://github.com/bottlesdevs/Bottles/issues/1392)
- [Files Community Issue #9591 - Consistency between New and Rename](https://github.com/files-community/Files/issues/9591)
- [VS Code Profiles Documentation](https://code.visualstudio.com/docs/configure/profiles)
- [Steamworks: Getting your game ready for Steam Deck](https://partner.steamgames.com/doc/steamdeck/recommendations)
- [Steam: Changing Your Game's Name](https://partner.steamgames.com/doc/store/editing/name)
- [MDN: ARIA textbox role](https://developer.mozilla.org/en-US/docs/Web/Accessibility/ARIA/Roles/textbox_role)
- [Microsoft: Keyboard Accessibility](https://learn.microsoft.com/en-us/windows/apps/design/accessibility/keyboard-accessibility)
- [React useOptimistic Hook](https://react.dev/reference/react/useOptimistic)
- [LogRocket: Understanding Optimistic UI](https://blog.logrocket.com/understanding-optimistic-ui-react-useoptimistic-hook/)
- [Error Handling UX Design Patterns](https://medium.com/design-bootcamp/error-handling-ux-design-patterns-c2a5bbae5f8d)
- [Pencil & Paper: Error Message UX](https://www.pencilandpaper.io/articles/ux-pattern-analysis-error-feedback)
- [How to Use Virtual Keyboard on Steam Deck](https://gamerant.com/steam-deck-virtual-keyboard-how-use-game-mode-desktop/)
- [React-Toastify: Add an undo action to a toast](https://fkhadra.github.io/react-toastify/add-an-undo-action-to-a-toast/)
- [UXPin: Keyboard Navigation Patterns for Complex Widgets](https://www.uxpin.com/studio/blog/keyboard-navigation-patterns-complex-widgets/)
