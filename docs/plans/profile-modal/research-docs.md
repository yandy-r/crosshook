# Documentation Research: profile-modal

## Architecture Docs

- /home/yandy/Projects/github.com/yandy-r/crosshook/docs/plans/profile-modal/feature-spec.md: primary source of truth for the profile-modal design; defines the frontend-only architecture, data ownership split between install handoff payload and modal session state, save behavior, viewport/accessibility requirements, affected files, and concrete implementation phases.
- /home/yandy/Projects/github.com/yandy-r/crosshook/docs/plans/profile-modal/research-technical.md: implementation-oriented architecture notes covering component boundaries, modal ownership in `ProfileEditorView`, reuse of `useInstallGame` and `useProfile`, suggested prop shapes, CSS envelope, and file-level impact.
- /home/yandy/Projects/github.com/yandy-r/crosshook/docs/plans/profile-modal/research-business.md: product and workflow contract for the feature; explains install session lifecycle, draft persistence expectations, auto-open/manual reopen behavior, save boundary, and discard/retry rules.
- /home/yandy/Projects/github.com/yandy-r/crosshook/docs/plans/profile-modal/research-recommendations.md: condensed implementation strategy with phased rollout, risk mitigations, and the rationale for a modal overlay instead of tab switching.
- /home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/components/InstallGamePanel.tsx: code-adjacent module documenting the current install flow UI, current “Review in Profile” handoff, candidate selection model, status copy, and the explicit “generated profile stays editable until the later save step” contract.
- /home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/components/ProfileEditor.tsx: current profile/install shell; documents where install handoff is consumed today and where modal ownership will likely move.
- /home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/install/service.rs: backend install pipeline for default prefix resolution, validation, installer execution, discovered executable candidates, and generation of the reviewable profile returned to the frontend.
- /home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/install/models.rs: install-domain model definitions and the `reviewable_profile()` contract that shapes the initial modal draft.
- /home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/profile/mod.rs: documented Rust module entry point for profile models and persistence helpers.
- /home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/profile/toml_store.rs: profile persistence behavior, profile naming rules, and save/load/list/delete semantics for the eventual modal save path.

## API Docs

- /home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/types/install.ts: frontend install request/result/stage types; current home of the install result payload and candidate definitions the modal must build on.
- /home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/types/profile.ts: frontend `GameProfile` schema that both the install draft and persisted profile use.
- /home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/profile/models.rs: canonical Rust `GameProfile` serialization model, including `runtime`, `steam`, `trainer`, and `launch` sections saved to TOML.
- /home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/hooks/useInstallGame.ts: frontend state machine for install flow; documents `review_required` vs `ready_to_save`, `reviewProfile` derivation, candidate option creation, working-directory derivation, and review handoff readiness.
- /home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/hooks/useProfile.ts: frontend profile normalization and save contract; defines the current save boundary, metadata sync, validation rule that blocks save without a final executable, and `hydrateProfile()` behavior used by the current tab handoff.
- /home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src-tauri/src/commands/profile.rs: Tauri command surface for `profile_list`, `profile_load`, `profile_save`, and `profile_delete`; relevant because the modal is expected to reuse the existing save/load path.
- /home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/hooks/useGamepadNav.ts: controller and keyboard traversal contract, including focus scope, editable-element exclusions, confirm/back handling, and the root-scoped assumptions a portaled modal must not violate.
- /home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/App.tsx: app-level composition that applies `useGamepadNav` to the main root; relevant for understanding how a portal modal can accidentally leave background controls reachable.
- <https://react.dev/reference/react-dom/createPortal>: linked from local profile-modal docs; defines the portal behavior recommended for rendering the modal outside layout constraints while preserving React context.
- <https://www.w3.org/WAI/ARIA/apg/patterns/dialog-modal/>: linked from local profile-modal docs; accessibility contract for a real modal dialog, including focus movement, trapping, escape handling, and labelling.
- <https://developer.mozilla.org/en-US/docs/Web/API/HTMLDialogElement/showModal>: linked from local profile-modal docs; native dialog alternative and browser behavior reference.
- <https://developer.mozilla.org/en-US/docs/Web/Accessibility/ARIA/Roles/dialog_role>: linked from local profile-modal docs; dialog labelling and ARIA usage guidance.
- <https://developer.mozilla.org/en-US/docs/Web/HTML/Reference/Global_attributes/inert>: linked from local profile-modal docs; background inertness guidance for custom modal shells.
- <https://developer.mozilla.org/en-US/docs/Web/CSS/overflow>: linked from local profile-modal docs; required for the internal scroll container behavior.
- <https://developer.mozilla.org/en-US/docs/Web/CSS/scrollbar-gutter>: linked from local profile-modal docs; useful for scroll-stable modal layout.
- <https://v2.tauri.app/reference/javascript/api/>: linked from local profile-modal docs; confirms the modal should keep using the existing Tauri JS bridge instead of adding modal-specific backend APIs.
- <https://v2.tauri.app/reference/javascript/api/namespacecore/>: linked from local profile-modal docs; `invoke()` reference for save/load reuse.

## Development Guides

- /home/yandy/Projects/github.com/yandy-r/crosshook/docs/plans/profile-modal/research-ux.md: detailed UX guidance for modal sizing, sticky header/footer, scroll behavior, focus placement, validation feedback, and Steam Deck-friendly interaction.
- /home/yandy/Projects/github.com/yandy-r/crosshook/docs/plans/profile-modal/research-external.md: external API/library evaluation for portals, dialog semantics, inert background handling, scroll behavior, and Tauri bridge reuse.
- /home/yandy/Projects/github.com/yandy-r/crosshook/docs/features/steam-proton-trainer-launch.doc.md: user-facing workflow guide that explains the current install flow, Proton-run semantics, and the existing “return to profile editor for review and save” model that the modal will replace.
- /home/yandy/Projects/github.com/yandy-r/crosshook/docs/getting-started/quickstart.md: end-user guide for profile creation, install flow expectations, save semantics, and where install-generated profiles live.
- /home/yandy/Projects/github.com/yandy-r/crosshook/docs/pr-reviews/pr-23-review.md: review notes for the original install-game PR; important because it documents the existing executable-confirmation gate and calls out the missing regression coverage around review handoff.
- /home/yandy/Projects/github.com/yandy-r/crosshook/docs/internal-docs/launcher-delete-plan-evaluation.md: not about profile-modal directly, but it is the clearest existing internal guidance on CrossHook dialog expectations, including gamepad accessibility requirements and `<dialog>`-based focus trapping considerations.
- /home/yandy/Projects/github.com/yandy-r/crosshook/tasks/lessons.md: implementation pitfalls already learned in this repo, especially the rule not to let controller/gamepad handlers capture typing keys inside editable controls and the note about verifying Tauri dialog plugin permissions.
- /home/yandy/Projects/github.com/yandy-r/crosshook/AGENTS.md: repo-level architecture summary and workflow guidance; useful for finding the relevant frontend/backend files and understanding the existing app structure.
- /home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/styles/theme.css: theme primitives for panels, cards, tab rows, inputs, spacing, and current visual language that the modal should reuse.
- /home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/styles/variables.css: design tokens for color, typography, shadows, content width, touch target minimums, and responsive spacing.
- /home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/components/LaunchPanel.tsx: user-facing explanatory copy that currently tells users to review generated profiles in the Profile tab before saving; this copy will need updating alongside the modal implementation.
- /home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/components/LauncherExport.tsx: install-context guidance panel that currently documents the save boundary and explicitly says save happens in the Profile tab; another place that will need copy and behavior review.

## README Files

- /home/yandy/Projects/github.com/yandy-r/crosshook/README.md: high-level product overview, install-flow summary, launch modes, and current documentation that still describes handing the generated profile back to the normal profile editor/Profile tab for review and save.

## Must-Read Documents

- /home/yandy/Projects/github.com/yandy-r/crosshook/docs/plans/profile-modal/feature-spec.md: read first for the agreed feature contract, ownership split, save handoff behavior, and affected files.
- /home/yandy/Projects/github.com/yandy-r/crosshook/docs/plans/profile-modal/research-technical.md: read before coding to understand the intended component/hook boundaries and why the feature should stay frontend-only.
- /home/yandy/Projects/github.com/yandy-r/crosshook/docs/plans/profile-modal/research-ux.md: read before building the shell so the modal honors viewport, focus, and sticky-chrome requirements.
- /home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/components/InstallGamePanel.tsx: read to understand the current handoff, candidate-selection UI, readiness gating, and strings that the modal replaces.
- /home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/components/ProfileEditor.tsx: read to understand the current profile/install tab ownership and where the modal session likely belongs.
- /home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/hooks/useInstallGame.ts: read for the install-stage model, derived review profile, executable confirmation behavior, and working-directory updates.
- /home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/hooks/useProfile.ts: read for normalization, validation, and the persistence path the modal should reuse instead of inventing another save flow.
- /home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/hooks/useGamepadNav.ts and /home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/App.tsx: read together for focus-scope and controller navigation constraints that a portal overlay must respect.
- /home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/install/models.rs and /home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/crates/crosshook-core/src/install/service.rs: read for the exact backend-generated draft profile semantics and candidate discovery contract.
- /home/yandy/Projects/github.com/yandy-r/crosshook/docs/pr-reviews/pr-23-review.md: read for the existing executable-confirmation regression risk and the missing test coverage that should probably be addressed when the handoff changes.
- <https://www.w3.org/WAI/ARIA/apg/patterns/dialog-modal/> and <https://react.dev/reference/react-dom/createPortal>: read before implementing the shell; these are the two external references that most directly constrain the modal primitive and accessibility behavior.

## Documentation Gaps

- The repo has no `docs/plans/install-game/` directory in this checkout even though `/home/yandy/Projects/github.com/yandy-r/crosshook/docs/plans/profile-modal/research-business.md` references broader install-game planning docs as a possible source of truth.
- User-facing docs still describe the old tab-switch handoff. At minimum, `/home/yandy/Projects/github.com/yandy-r/crosshook/README.md`, `/home/yandy/Projects/github.com/yandy-r/crosshook/docs/getting-started/quickstart.md`, `/home/yandy/Projects/github.com/yandy-r/crosshook/docs/features/steam-proton-trainer-launch.doc.md`, `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/components/LaunchPanel.tsx`, and `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/components/LauncherExport.tsx` will become outdated once profile-modal ships.
- There is no dedicated local documentation yet for the extracted shared form sections (`ProfileFormSections` or equivalent) because that component does not exist; the current knowledge is split across `ProfileEditor.tsx` and the profile-modal plan docs.
- There is no focused local test-plan document for modal-specific keyboard/controller behavior. The closest guidance is `research-ux.md`, `research-external.md`, `tasks/lessons.md`, and the unrelated launcher-delete dialog evaluation.
- There is no existing local doc for modal session persistence/discard UX beyond the profile-modal plan set, so implementers will need to treat the plan docs as the only current source of truth until product docs are updated.
