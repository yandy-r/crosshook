## Executive Summary

This feature adds a guided Proton-only install flow inside the existing Profile panel so users can create a runnable CrossHook profile while they install a Windows game, instead of installing the game externally and then filling the profile form by hand. The business value is reducing setup friction for non-Steam games and making CrossHook the single place where users choose Proton, create a prefix, capture installer media, and optionally attach a trainer.

The install flow should behave like a lightweight setup wizard embedded as a sub-tab, but its output is still a normal saved CrossHook profile that feeds the existing Proton launch path. From a business perspective, the core promise is: if the installer succeeds or the user finishes setup, CrossHook preserves enough information to reopen, adjust, and launch the installed game later without re-entering the same runtime details.

### User Stories

- As a Linux user installing a non-Steam Windows game, I want to pick a detected Proton build instead of typing a Proton path so I can start quickly and avoid path mistakes.
- As a user with multiple prefixes, I want CrossHook to suggest a default prefix location but still let me override it so I can keep installs organized the way I prefer.
- As a user installing from an `.exe` on external media, I want to browse to the installer directly from the app so I do not have to switch to a terminal script.
- As a user who plans to use a trainer, I want to attach the trainer during setup so the saved profile is launch-ready after installation.
- As a returning user, I want the generated profile to appear in the normal profile list so I can manage it the same way as any manually created profile.
- As a cautious user, I want CrossHook to distinguish installer media from the final game executable so I do not accidentally save a profile that launches the setup program forever.

### Business Rules

- The install sub-tab is only for Proton-based Windows installs. It must not expose Steam app launch or native Linux runner selection.
- Proton selection is required before install can start. The user may choose a detected install or manually specify a valid Proton path if detection is incomplete.
- Installation media is required and must be a Windows executable file intended to start the installer or bootstrapper.
- Prefix location is required. CrossHook should prefill a default under `~/.config/crosshook/prefixes/<derived-profile-or-game-slug>` but the user can override it before running install.
- The chosen prefix must represent the long-lived runtime environment for the installed game, not a temporary installer-only sandbox.
- Trainer selection is optional. Omitting a trainer must not block install or profile generation.
- A generated profile must be a standard `proton_run` profile with the selected Proton path and chosen prefix path stored in the runtime fields.
- CrossHook must not assume the installer path is the final game executable path. After installation, the user needs a clear confirmation step to choose or confirm the actual installed game executable before the profile is finalized.
- If the install step launches successfully but the user cancels setup before confirming the final game executable, CrossHook should preserve draft state or keep the profile incomplete rather than silently saving a broken runnable profile.
- Existing saved profiles remain authoritative. Creating an install-generated profile with an already used name should require an explicit overwrite/rename decision.
- The flow should treat profile creation as part of setup completion, not as an unrelated second task. Users should leave the installer tab with either a saved profile, an explicit draft, or a clear failure state.

### Workflows

- Primary flow:
  1. User opens the Profile panel and switches to the new install sub-tab.
  2. User enters a game/profile name or accepts a name derived from the installer.
  3. User selects a Proton version from detected installs or manually corrects the Proton path.
  4. User accepts or overrides the default prefix path.
  5. User selects installation media (`setup.exe`, launcher bootstrapper, etc.).
  6. User optionally selects a trainer executable.
  7. User starts installation. CrossHook launches the installer inside the chosen Proton prefix.
  8. After the install process exits, CrossHook asks the user to identify the installed game executable and reviews the generated profile fields.
  9. User saves the generated profile, which then appears in the normal profile list and becomes launchable from the existing main-tab workflow.

- Error recovery flow:
  1. User starts setup with invalid or missing required inputs.
  2. CrossHook blocks launch, keeps the entered values, and points to the specific field that failed validation.
  3. If the installer fails to launch, CrossHook preserves the chosen Proton, prefix, installer, and trainer values so the user can retry without re-entering data.
  4. If the installer runs but no final game executable is confirmed, CrossHook leaves the setup in a draft/incomplete state and tells the user what remains unresolved.
  5. If the selected prefix already contains an existing install, CrossHook should surface that condition and ask whether the user intends to reuse it instead of silently clobbering expectations.

### Domain Concepts

- Install setup: A bounded workflow that gathers runtime choices, launches installer media, and transitions into profile generation.
- Install draft: User-entered setup data that is complete enough to retry or resume, but not yet a finalized runnable profile.
- Finalized install profile: A normal CrossHook game profile produced by the install flow, with `launch.method = proton_run` and populated runtime fields.
- Proton selection: The specific Proton runtime the user intends both for installation and for later game/trainer launch.
- Prefix ownership: The relationship between one install flow and the Wine prefix that stores the installed game, registry state, and staged runtime data.
- Installer media: The executable the user launches to perform installation. This is distinct from the game executable stored in the finished profile.
- Runtime target: The installed game executable that CrossHook should launch after setup is complete.
- Optional trainer attachment: An auxiliary executable path that can be captured during setup and carried into the generated profile without being required for install success.
- Install status:
  - Draft: inputs collected, install not yet completed.
  - Installing: installer launched and expected to mutate the prefix.
  - Needs confirmation: installer returned and CrossHook still needs the final game executable.
  - Ready: generated profile saved successfully.
  - Failed: install launch or profile finalization could not complete.

### Success Criteria

- Users can complete a Proton game install without leaving CrossHook for a terminal script.
- The default pathing model is understandable enough that most users accept the suggested prefix path unchanged.
- The produced profile is immediately visible in the existing profile list and requires no manual repair for common install cases.
- Users can optionally attach a trainer during setup without being forced to do so.
- CrossHook prevents the two most common setup mistakes: saving the installer path as the game path and saving an empty/invalid Proton or prefix configuration.
- Retry behavior is efficient: a failed or interrupted install attempt does not force users to re-enter all fields.
- The flow supports first-time setup for non-Steam Windows games while remaining consistent with existing CrossHook profile concepts.

### Open Questions

- Should CrossHook save a draft profile before installation starts, or only after the user confirms the final installed executable?
- What is the canonical default prefix slug: profile name, derived game name, installer filename stem, timestamped slug, or a combination?
- When the chosen prefix already exists, should the product treat that as reuse, warn-only, or require explicit confirmation before proceeding?
- After installer exit, should CrossHook help scan the prefix for likely game executables, or require explicit manual selection in the first version?
- Should the install sub-tab support bootstrap launchers that download more content and may need multiple reruns, or should v1 explicitly scope to a single installer-launch step?
- Is trainer selection meant only to populate the generated profile, or should the install flow also support staging/installing trainer dependencies later?
