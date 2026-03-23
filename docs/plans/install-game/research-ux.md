## Executive Summary

This feature should feel like a guided install workflow embedded inside the existing Profile panel, not like a second unrelated tool. The best shape is a small sub-tab switcher inside the panel with one tab for normal profile editing and one tab for "Install Windows Game", where the install view uses the same field language and detected-Proton interaction pattern the app already uses elsewhere.

The UX needs to solve two failure-prone moments clearly: choosing a durable prefix before install starts, and choosing the real game executable after the installer exits. Good inline instructions, native file dialogs, and explicit post-install confirmation matter more here than visual complexity because the dominant user risk is saving a profile that points to the installer instead of the installed game.

### Core User Workflows

- Happy path:
  1. User opens the `Install Windows Game` sub-tab inside the Profile panel.
  2. CrossHook pre-fills a profile name and default prefix path derived from the entered name or installer filename.
  3. User selects a detected Proton version from a dropdown that fills an editable path field.
  4. User chooses the installer `.exe` with the native file dialog.
  5. User optionally chooses a trainer `.exe`.
  6. User reviews the prefix note and starts install.
  7. The UI switches into an inline running state with logs/progress copy near the action button.
  8. After installer exit, CrossHook presents candidate executables found in the prefix and asks the user to confirm the final game executable.
  9. User saves the generated profile and is returned to the normal profile-edit/launch flow with that profile selected.

- Recovery flow:
  1. A required field is missing or invalid.
  2. CrossHook keeps the user's entries, focuses the first failing field, and shows one concise inline summary plus field-level messaging.
  3. If the installer fails to start, the form remains intact and the primary action changes to `Retry Install`.
  4. If the installer ran but the final executable is still unknown, the UI shifts to a confirmation step rather than claiming success.

### UI and Interaction Patterns

- Sub-tab layout:
  - Use a small two-option segmented control or tab row inside `ProfileEditor`, not a new top-level app tab
  - Labels should be explicit: `Profile` and `Install Windows Game`
  - Keep the install tab self-contained so the main app's existing tab model does not expand unnecessarily
- Field grouping:
  - Step 1: identity (`Profile Name`)
  - Step 2: runtime (`Detected Proton`, editable Proton path, prefix path)
  - Step 3: media (`Installer EXE`, optional `Trainer EXE`)
  - Step 4: execution (`Install`, status, logs, next-step messaging)
- Input behavior:
  - Keep the detected-Proton dropdown filling an editable text field, matching the project's existing lesson
  - Use native file dialogs for installer and trainer selection, consistent with GNOME guidance around native file dialogs
  - Make the prefix field browseable as a directory but also editable as raw text
- Post-install confirmation:
  - Show a shortlist of likely executables first, with a manual browse option
  - Present why the step exists in plain language: "Installers are not always the game executable"

### Accessibility Considerations

- Labels and instructions:
  - Every field needs a visible label and descriptive accessible name
  - Put a short instruction block at the top of the form describing required fields and the default prefix convention, aligning with W3C guidance on instructions for forms
- Focus management:
  - On validation failure, move focus to the first invalid field
  - After installer exit, move focus to the executable-confirmation heading or list
- Status communication:
  - Do not rely on color alone for status; pair status colors with text labels such as `Installing`, `Needs confirmation`, or `Failed`
  - Ensure logs and status messages are readable by screen readers where feasible
- Keyboard flow:
  - Tab order must remain linear through the sub-tab switcher, fields, file-picker buttons, and primary action
  - The install sub-tab should not break the repo's existing gamepad/keyboard-focus behavior

### Feedback and State Design

- Suggested UI states:
  - `Idle`: no installer selected yet
  - `Ready`: required fields complete, install can start
  - `Installing`: process running
  - `Awaiting confirmation`: installer exited, final game executable still needed
  - `Ready to save`: final executable confirmed
  - `Saved`: profile generated successfully
  - `Failed`: validation or process launch failed
- Feedback patterns:
  - For short preflight actions, use small inline spinners near the primary button
  - For installs that can run longer, show an inline progress region instead of a detached modal, consistent with GNOME HIG guidance
  - If exact progress is unknowable, show activity mode with task-stage text such as `Launching installer`, `Waiting for installer to exit`, `Scanning prefix for executables`
  - Provide a persistent note about where logs are streaming from if log output is visible
- Action labeling:
  - Before start: `Install Game`
  - During run: `Installing...`
  - After failure: `Retry Install`
  - After executable confirmation: `Save Profile`

### UX Risks

- Users may confuse the installer executable with the final game executable.
  - Mitigation: require an explicit post-install confirmation step and never auto-save the installer path as the runtime target.
- Users may not understand what a prefix is.
  - Mitigation: add one sentence under the field explaining it stores the Windows environment for the installed game and defaults under `~/.config/crosshook/prefixes/...`.
- Long-running installers can make the app look hung.
  - Mitigation: show nearby activity feedback within the panel after a short delay, then keep logs/status visible inline.
- Existing prefix reuse can be surprising.
  - Mitigation: detect non-empty prefixes and show a warning before install starts.
- Embedding too many advanced options in v1 can overwhelm first-time users.
  - Mitigation: keep the main form compact and defer advanced environment controls to a later release.
