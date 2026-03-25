## Executive Summary

CrossHook should treat Proton optimizations as profile-level launch preferences rather than ad hoc command strings. The feature exists to let users reproduce known-good Steam launch options with named toggles, keep those choices attached to a profile, and reduce repetitive manual editing across launches.

The business behavior has to align with the current product model: profiles are the persistent unit of launch configuration, Steam and direct Proton launches are first-class, native Linux launch remains a separate path, and the install-review workflow intentionally delays persistence until the user saves. That means optimization controls should feel immediate for an already selected profile, but they must still respect profile identity, validation, and mode-specific applicability.

### User Stories

- As a Steam Deck or Linux user, I want to enable common Proton launch tweaks with readable labels so I do not have to remember or retype environment variable names.
- As a user who maintains many profiles, I want the selected launch optimizations to save with each profile automatically so every game remembers its own compatibility and performance settings.
- As a user switching between Steam app launch and direct Proton launch, I want CrossHook to show only the options that make sense for the active runner so the UI does not imply unsupported behavior.
- As a user troubleshooting a stubborn title, I want compatibility toggles such as Steam Deck spoofing or window-decoration disabling to be easy to discover and reverse.
- As a user exporting or revisiting a profile later, I want launch optimizations to remain understandable in the app as named capabilities, not opaque shell fragments.
- As a user running native Linux games, I want this feature to stay out of my way and not suggest Proton-only tweaks apply to native launches.
- As a power user, I want a curated set of useful options rather than a raw command editor, because the product value is safe repeatability, not unrestricted shell composition.

### Business Rules

- Launch optimizations belong to the profile and must persist per profile, not globally.
- The feature applies only to Proton-capable workflows. `steam_applaunch` and `proton_run` may expose Proton optimization controls; `native` must not be degraded or mislabeled as supporting Proton options.
- Options must be presented with human-friendly labels and short explanations. Raw variable names may appear as secondary detail or debugging metadata, but not as the primary UX.
- The initial option catalog should be curated. CrossHook should expose vetted toggles and wrappers, not a free-form launch command field.
- Each option must have a clear applicability rule:
  - Proton env toggle only
  - Steam/Proton compatibility toggle
  - command wrapper or overlay
  - unsupported for the current launch method
- When an option is unavailable because of the selected runner or missing required profile fields, the UI should communicate why rather than silently ignoring the choice.
- Auto-save should occur only when CrossHook has enough profile identity to persist safely. In the current product model, that means at minimum a valid profile name and a saveable profile state.
- Auto-save for launch options must not unexpectedly persist unrelated unfinished edits from the rest of the profile form unless the product intentionally defines the profile as a single shared draft.
- The install workflow remains a separate business path. During install review, CrossHook currently does not persist until the user saves the reviewed profile; optimization toggles should not bypass that rule without an explicit product decision.
- Resetting a launch session must not erase saved optimization preferences. Session reset affects the live launch state, not profile configuration.
- Deleting or renaming a profile must carry the associated optimization settings with the profile lifecycle automatically because they are part of the profile document.
- CrossHook should prefer deterministic Boolean or enumerated options over ambiguous "performance boost" claims. If an optimization is experimental or hardware-specific, it should be labeled as such.
- Wrapper-style features such as overlays should be modeled as explicit launch behaviors, not as arbitrary prepend strings the user cannot inspect.

### Workflows

- Primary flow: existing profile
  - User loads an existing Proton or Steam profile.
  - User opens the launch-optimization section near the LaunchPanel.
  - User enables or disables one or more named options.
  - CrossHook validates that the current profile can be persisted and auto-saves the change.
  - The UI confirms the saved state without requiring the main Save button.
  - The next game or trainer launch uses the saved option set.
- Primary flow: new profile being authored
  - User begins a new profile and configures game/runtime data.
  - The launch-optimization section appears only when the profile has enough runner context to determine applicable options.
  - If the profile is not yet saveable, CrossHook should either disable auto-persisting optimization controls or make it explicit that selections are local until the profile is first saved.
  - Once the profile becomes saveable, optimization changes can persist automatically.
- Primary flow: switching runner method
  - User changes the runner from Steam to Proton runtime or native.
  - CrossHook re-evaluates which options are applicable.
  - Inapplicable options are hidden, disabled, or marked inactive, but previously saved values should not be destroyed casually if the user may switch back.
- Recovery flow: auto-save failure
  - User toggles an option.
  - Persistence fails because the profile name is invalid, required fields are missing, or the save operation errors.
  - CrossHook surfaces the failure inline near the optimization section and preserves the user’s intended selection long enough to retry or reconcile.
- Recovery flow: unsupported dependency or environment
  - User enables an overlay or wrapper that depends on external host tooling.
  - CrossHook warns that the feature requires the tool to be installed and indicates that launch may fail or the option may be skipped based on the eventual product decision.
- Install review flow
  - User is in the install-review path where generated profiles remain editable but unsaved.
  - Optimization settings should follow the same review-first rule as the rest of the install result unless product decides to introduce draft-only persistence.

### Domain Concepts

- Profile: the durable unit of launch behavior stored as TOML and selected from the profile editor.
- Launch optimization: a named, persisted preference that affects how CrossHook launches a Proton-backed game or trainer.
- Option catalog: the curated list of supported toggles and wrappers that CrossHook chooses to expose.
- Applicability: the rule set that determines whether an option is relevant for `steam_applaunch`, `proton_run`, or neither.
- Auto-save boundary: the product rule describing when optimization edits become durable without using the explicit Save button.
- Draft state: in-memory edits that have not yet been persisted; this matters for new profiles and install review.
- Experimental option: a supported but caveated optimization whose effect is game-, hardware-, or driver-dependent.
- Wrapper feature: a launch behavior that changes the executed command shape, such as an overlay or performance tool, rather than only setting environment variables.
- Compatibility toggle: an option used to make a title run correctly rather than to improve performance, such as Steam Deck spoofing or window-management behavior.

### Success Criteria

- Users can configure a recognizable set of Proton/Steam launch optimizations from the UI without typing raw environment variables.
- Optimization choices persist per profile and are restored automatically when the profile is reloaded.
- The feature does not present Proton-only options for native Linux launch profiles.
- The install-review workflow and regular profile-edit workflow have clearly defined and internally consistent save behavior.
- Users receive immediate feedback when an optimization change is saved, pending, inapplicable, or failed.
- The option catalog remains curated enough that a typical user can understand it without external documentation.
- Advanced compatibility options requested by users, including Steam Deck spoofing, can be enabled without manual shell editing.
- The product avoids creating a second unsupervised command-authoring surface that undermines launch reliability or supportability.

### Open Questions

- Should optimization toggles be completely unavailable until a new profile has been saved once, or should CrossHook allow temporary draft selections and persist them later?
- Should auto-save for launch options persist only that section, or should it implicitly save the entire profile draft as it exists at the moment of the toggle?
- In the install-review modal, should optimization controls be editable but deferred until final save, or omitted entirely until the reviewed profile is accepted?
- When a user switches a profile from Proton-capable to native launch, should prior Proton optimization values be retained for future runner changes or cleared as invalid?
- Should wrapper-dependent options such as overlays be treated as hard requirements that block launch when missing, or soft options that warn the user and continue?
- How much detail should the UI expose for each option: simple label only, label plus description, or label plus underlying variable name for debugging?
- Should CrossHook support ordering or mutual exclusion rules between wrapper-style options if more than one is selected?
