# Profile Modal Implementation Plan

`profile-modal` is a frontend-only coordination refactor that replaces the current install-to-profile tab jump with a modal-local review flow. The implementation should keep install execution and reviewable draft generation in `useInstallGame` and `InstallGamePanel`, move review-session ownership into `ProfileEditorView`, render a portal-based modal with shared profile form sections, and save through the existing `useProfile` pipeline before explicitly switching to the Profile tab. The core risks are not backend contracts; they are editor drift, focus/controller leakage behind the modal, preserving install-session state across dismiss and reopen, and keeping a form-heavy shell usable at the Tauri window baseline of `1280x800`.

## Critically Relevant Files and Documentation

- /docs/plans/profile-modal/feature-spec.md: Source of truth for fixed decisions, ownership boundaries, and success criteria.
- /docs/plans/profile-modal/shared.md: Condensed architecture, patterns, and must-read references for implementation.
- /docs/plans/profile-modal/analysis-context.md: Cross-cutting constraints, parallelization guidance, and integration seams.
- /docs/plans/profile-modal/analysis-code.md: Code-level patterns, gotchas, and file-create/modify guidance.
- /src/crosshook-native/src/components/ProfileEditor.tsx: Current install/profile bridge and future modal-session owner.
- /src/crosshook-native/src/components/InstallGamePanel.tsx: Current install review UI and payload handoff source.
- /src/crosshook-native/src/hooks/useInstallGame.ts: Install state machine, review profile derivation, and candidate handling.
- /src/crosshook-native/src/hooks/useProfile.ts: Normalization, validation, persistence, metadata sync, and profile refresh flow.
- /src/crosshook-native/src/hooks/useGamepadNav.ts: Controller/keyboard focus handling that must not leak behind the modal.
- /src/crosshook-native/src/App.tsx: App-level gamepad root and profile editor tab coordination.
- /src/crosshook-native/src/types/install.ts: Home for install-domain review payload types.
- /src/crosshook-native/src/types/profile.ts: Canonical `GameProfile` contract reused by modal draft and persistence.
- /src/crosshook-native/src/styles/theme.css: Shared layout and surface styling system to extend for modal UI.
- /src/crosshook-native/src/styles/focus.css: Focus styling system relevant to modal layering and controller navigation.
- /src/crosshook-native/src/components/LaunchPanel.tsx: In-app copy still describing review in the Profile tab.
- /src/crosshook-native/src/components/LauncherExport.tsx: In-app copy still describing save in the Profile tab.
- /docs/getting-started/quickstart.md: User-facing workflow doc that will become stale after the modal ships.
- /docs/features/steam-proton-trainer-launch.doc.md: Feature doc describing the old review/save handoff.

## Implementation Plan

### Phase 1: Contracts and Shared Seams

#### Task 1.1: Define review payload and modal session types Depends on [none]

**READ THESE BEFORE TASK**

- /docs/plans/profile-modal/feature-spec.md
- /src/crosshook-native/src/types/install.ts
- /src/crosshook-native/src/types/profile.ts
- /src/crosshook-native/src/types/index.ts

**Instructions**

Files to Create

- /src/crosshook-native/src/types/profile-review.ts

Files to Modify

- /src/crosshook-native/src/types/install.ts
- /src/crosshook-native/src/types/index.ts

- Add `InstallProfileReviewPayload` to `install.ts`, keeping it install-domain transport data built from current install-review state rather than raw backend result alone.
- Add `ProfileReviewSession` and any tightly related modal-only helper types to `profile-review.ts`; keep these UI-local and distinct from persisted `GameProfile`.
- Define the minimum payload/session contract explicitly in this task so downstream work does not guess. At minimum include:
  - payload: `source`, `profileName`, `generatedProfile`, `candidateOptions`, `helperLogPath`, `message`
  - session: `isOpen`, `source`, `profileName`, `originalProfile`, `draftProfile`, `candidateOptions`, `helperLogPath`, `installMessage`, `dirty`, `saveError`
- Preserve existing `GameProfile` field naming and snake_case compatibility. Do not “clean up” transport or persistence keys during this task.
- Export only the types that need broad consumption; avoid turning `profile-review.ts` into a dumping ground for unrelated install/editor models.
- Expected outcome: downstream tasks have a stable, agreed contract for modal open/reopen and local session ownership.

#### Task 1.2: Add an explicit draft persistence helper to `useProfile` Depends on [none]

**READ THESE BEFORE TASK**

- /docs/plans/profile-modal/feature-spec.md
- /src/crosshook-native/src/hooks/useProfile.ts
- /src/crosshook-native/src/types/profile.ts
- /src/crosshook-native/src-tauri/src/commands/profile.rs

**Instructions**

Files to Create

- None.

Files to Modify

- /src/crosshook-native/src/hooks/useProfile.ts

- Add a narrow helper such as `persistProfileDraft(name, profile)` that reuses the existing normalization, validation, `profile_save`, metadata sync, profile-list refresh, and reload flow.
- Keep the helper explicit about save boundaries: validate first, preserve the caller’s draft on failure, and only update the global selected profile after a successful save.
- Make ownership explicit in the helper contract: `persistProfileDraft(...)` owns persistence plus reload/select of the saved profile, while the caller owns the post-save UI transition to `editorTab = 'profile'`.
- Do not require callers to hydrate global editor state before persistence. The modal must be able to save a local draft without clobbering the global editor during review.
- Keep existing `saveProfile()` behavior intact for the normal profile tab; the new helper should share logic instead of duplicating or silently changing old semantics.
- Expected outcome: modal integration can persist its local draft through the same pipeline the rest of the app already trusts.

#### Task 1.3: Extract reusable profile form sections from `ProfileEditor` Depends on [none]

**READ THESE BEFORE TASK**

- /docs/plans/profile-modal/feature-spec.md
- /src/crosshook-native/src/components/ProfileEditor.tsx
- /src/crosshook-native/src/types/profile.ts
- /src/crosshook-native/src/styles/theme.css

**Instructions**

Files to Create

- /src/crosshook-native/src/components/ProfileFormSections.tsx

Files to Modify

- /src/crosshook-native/src/components/ProfileEditor.tsx

- Pull the reusable profile field groups and section ordering out of `ProfileEditor.tsx` into a prop-driven shared component that can render against either the global profile-tab draft or a modal-local review draft.
- Encode the fixed section-visibility decisions in this extraction: keep install-critical fields open by default, collapse only empty Trainer details and Working Directory override, and avoid dragging Steam launcher fields into `proton_run` review unnecessarily.
- Preserve current browse interactions, Proton selector behavior, and nested immutable state updates. This extraction should reduce duplication without changing what the profile tab currently does.
- Keep the API narrow and explicit: pass in the current draft, launch method, Proton installs, error strings, and update callbacks rather than leaking hook internals into the shared component.
- Expected outcome: the app has one shared editing surface instead of separate profile-tab and modal editors drifting over time.

### Phase 2: Modal Surface and Install Handoff

#### Task 2.1: Build the modal shell and modal-specific styling Depends on [none]

**READ THESE BEFORE TASK**

- /docs/plans/profile-modal/research-ux.md
- /docs/plans/profile-modal/research-external.md
- /src/crosshook-native/src/styles/theme.css
- /src/crosshook-native/src/styles/focus.css

**Instructions**

Files to Create

- /src/crosshook-native/src/components/ProfileReviewModal.tsx

Files to Modify

- /src/crosshook-native/src/styles/theme.css
- /src/crosshook-native/src/styles/focus.css

- Build a portal-based modal shell with a dimmed backdrop, sticky header and footer, a scrollable body, and a compact summary strip for install status, profile name, executable, prefix, and helper log path.
- Make the shell large enough for dense form review but keep it viewport-safe at `1280x800` and smaller widths. Only the modal body should scroll.
- Implement real modal semantics: focus entry, focus restore, `role="dialog"`, `aria-modal="true"`, close affordances, and a structure that supports background suppression.
- Keep the shell reusable and prop-driven. Do not couple it directly to `useInstallGame` or `useProfile`.
- Expected outcome: the app has a standalone modal primitive that matches the CrossHook visual system and can host the shared form sections.

#### Task 2.2: Replace the install-panel review handoff with a payload callback Depends on [1.1, 1.3]

**READ THESE BEFORE TASK**

- /docs/plans/profile-modal/feature-spec.md
- /src/crosshook-native/src/components/InstallGamePanel.tsx
- /src/crosshook-native/src/components/ProfileEditor.tsx
- /src/crosshook-native/src/hooks/useInstallGame.ts
- /src/crosshook-native/src/types/install.ts

**Instructions**

Files to Create

- None.

Files to Modify

- /src/crosshook-native/src/components/InstallGamePanel.tsx
- /src/crosshook-native/src/components/ProfileEditor.tsx

- Replace `onReviewGeneratedProfile(profileName, profile)` with a payload-driven callback using the new `InstallProfileReviewPayload`.
- Assemble the payload from the current frontend install state, not only the raw backend result. Use the current `reviewProfile`, candidate list, helper log path, result message, and a `source` that distinguishes auto-open from manual verify.
- Update the parent callback signature in `ProfileEditor.tsx` at the same time, even if the full modal-session wiring still lands later. This task should leave the handoff contract compiling without forcing the old `(name, profile)` adapter to linger.
- Add auto-open behavior when a reviewable draft exists after install succeeds, but do not key this solely off `ready_to_save`; the existing install flow can be reviewable while still marked `review_required`.
- Preserve manual verify reopen behavior and keep install execution, reset, retry, and candidate-selection responsibilities inside `InstallGamePanel`.
- Expected outcome: the install flow emits a clean modal-open contract without mutating global profile-editor state directly.

### Phase 3: Review Session Integration

#### Task 3.1: Make `ProfileEditorView` own modal review sessions Depends on [1.2, 1.3, 2.1, 2.2]

**READ THESE BEFORE TASK**

- /docs/plans/profile-modal/feature-spec.md
- /src/crosshook-native/src/components/ProfileEditor.tsx
- /src/crosshook-native/src/hooks/useProfile.ts

**Instructions**

Files to Create

- None.

Files to Modify

- /src/crosshook-native/src/components/ProfileEditor.tsx
- /src/crosshook-native/src/components/ProfileReviewModal.tsx
- /src/crosshook-native/src/components/ProfileFormSections.tsx

- Introduce `ProfileReviewSession` state in `ProfileEditorView` and make it the owner of modal open, close, discard, draft updates, and save orchestration.
- Render the extracted `ProfileFormSections` inside both the normal profile tab and the modal, keeping field semantics and update behavior aligned.
- On modal save, call the new `persistProfileDraft(...)` helper, then switch `editorTab` to `profile` only after the helper resolves successfully. Do not duplicate selected-profile logic if the helper already reloads and selects the saved profile.
- Keep the install panel visible in the background while the modal is open so dismissal returns the user to the install context cheaply.
- Expected outcome: the app uses a modal-local draft during review and only crosses into global profile-tab state after successful persistence.

#### Task 3.2: Add dirty-dismiss and install-replacement guards Depends on [3.1]

**READ THESE BEFORE TASK**

- /docs/plans/profile-modal/research-business.md
- /tasks/lessons.md
- /src/crosshook-native/src/components/ProfileEditor.tsx
- /src/crosshook-native/src/components/InstallGamePanel.tsx

**Instructions**

Files to Create

- None.

Files to Modify

- /src/crosshook-native/src/components/ProfileEditor.tsx
- /src/crosshook-native/src/components/InstallGamePanel.tsx
- /src/crosshook-native/src/components/ProfileReviewModal.tsx

- Add explicit confirmation before closing a dirty modal or replacing it with a new install result via retry, reset, or a new install completion.
- Use an in-app confirmation surface that matches the CrossHook theme and existing Tauri UI expectations; do not fall back to browser `confirm()` or a separate Tauri system dialog for this flow.
- Preserve the current review draft across dismiss and manual reopen. Modal dismissal must not call `reset()` on `useInstallGame`.
- Make incomplete review states explicit instead of silently falling back to tab-switch behavior. If the final executable is still missing, keep save blocked and keep the session intact.
- Keep the happy path simple: if the modal is clean, close should be immediate; if the user confirms discard, replacement or reset can proceed.
- Expected outcome: review-state continuity is reliable, and users cannot accidentally lose meaningful edits during install iteration.

### Phase 4: Hardening, Copy, and Documentation

#### Task 4.1: Harden controller and focus behavior for the modal Depends on [3.2]

**READ THESE BEFORE TASK**

- /tasks/lessons.md
- /src/crosshook-native/src/App.tsx
- /src/crosshook-native/src/hooks/useGamepadNav.ts
- /docs/plans/profile-modal/feature-spec.md

**Instructions**

Files to Create

- None.

Files to Modify

- /src/crosshook-native/src/App.tsx
- /src/crosshook-native/src/hooks/useGamepadNav.ts
- /src/crosshook-native/src/components/ProfileReviewModal.tsx

- Verify that the chosen portal host keeps modal focusables inside the effective gamepad/keyboard scope. If not, adjust the modal host placement or gamepad root handling so the modal is navigable and the background is not.
- Preserve the repo lesson about editable controls: global controller handlers must not capture typing keys inside inputs, selects, textareas, or contenteditable content.
- Ensure `Escape` and focus restore are deterministic, and background controls cannot be reached while the modal is open.
- Keep this task conditional in spirit but explicit in execution: solve the real focus-scope behavior uncovered by the modal implementation instead of assuming CSS alone is sufficient.
- Expected outcome: keyboard and controller interaction is modal-correct, not just visually modal.

#### Task 4.2: Update in-app copy for the modal-first review flow Depends on [3.1]

**READ THESE BEFORE TASK**

- /docs/plans/profile-modal/feature-spec.md
- /src/crosshook-native/src/components/LaunchPanel.tsx
- /src/crosshook-native/src/components/LauncherExport.tsx

**Instructions**

Files to Create

- None.

Files to Modify

- /src/crosshook-native/src/components/LaunchPanel.tsx
- /src/crosshook-native/src/components/LauncherExport.tsx

- Replace the remaining “review in Profile” / “save in the Profile tab” messaging with modal-first copy that matches the shipped behavior.
- Keep the post-save handoff explicit in the primary action or adjacent helper text, for example `Save and Open Profile Tab`, so users know what happens next.
- Make sure the copy still explains the save boundary clearly: install creates a reviewable draft, and save makes it a normal profile.
- Expected outcome: the UI no longer teaches the old workflow after the modal lands.

#### Task 4.3: Update user-facing docs for the new install review workflow Depends on [3.1]

**READ THESE BEFORE TASK**

- /docs/getting-started/quickstart.md
- /docs/features/steam-proton-trainer-launch.doc.md
- /docs/plans/profile-modal/feature-spec.md

**Instructions**

Files to Create

- None.

Files to Modify

- /docs/getting-started/quickstart.md
- /docs/features/steam-proton-trainer-launch.doc.md

- Update the user-facing install-flow docs to describe the modal review step, the saved-profile handoff, and the fact that save success opens the Profile tab with the new profile selected.
- Keep the docs aligned with the actual collapsed-section and save-boundary behavior; avoid inventing additional advanced controls or backend changes.
- If the implementation reveals the README also needs updating, do that as a small follow-up once the docs and UI copy already match.
- Expected outcome: product docs no longer describe a tab-switch-only review flow.

#### Task 4.4: Run the final verification pass for modal workflow regressions Depends on [4.1, 4.2, 4.3]

**READ THESE BEFORE TASK**

- /docs/plans/profile-modal/feature-spec.md
- /tasks/lessons.md
- /src/crosshook-native/src/components/ProfileEditor.tsx
- /src/crosshook-native/src/components/InstallGamePanel.tsx

**Instructions**

Files to Create

- None.

Files to Modify

- /tasks/todo.md

- Run the final verification checklist against the implemented feature, not just the happy path:
  - install success auto-opens the modal when a reviewable draft exists
  - dismiss and manual reopen restore the same draft
  - dirty-dismiss and retry/reset replacement confirmations behave correctly
  - save failure keeps the modal open and preserves edits
  - save success persists, reloads/selects the saved profile, and opens the Profile tab
  - controller, keyboard `Tab`, and `Escape` do not reach background controls
  - modal layout remains usable at `1280x800` with only the body scrolling
- Use the repo’s real verification path:
  - run `npm run build` in `src/crosshook-native` for the frontend typecheck/build proof
  - run `cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core` only if implementation changed backend contracts or shared core behavior
  - run the Tauri UI manually through `./scripts/dev-native.sh` to verify the modal interaction, focus, and viewport behavior in the actual app shell
- Record verification results and any residual risks in `tasks/todo.md` so implementation close-out is evidence-based.
- Expected outcome: the feature is not considered done until the key interaction, layout, and focus regressions have been explicitly checked.

## Advice

- Treat `ProfileEditor.tsx` as the main serialization point. It is already the largest and riskiest file involved, so avoid parallel edits there across multiple active tasks.
- Keep the modal draft isolated until save succeeds. Hydrating global profile state too early recreates the exact user-flow break this feature is supposed to remove.
- Do not key modal open behavior only off `ready_to_save`; the current install result can be reviewable while still marked `review_required`.
- Preserve the executable-to-working-directory derivation when the user picks or edits the final executable, but be careful not to overwrite an intentional working-directory override once that section is expanded.
- Validate controller and focus behavior as soon as the modal can open. This is a higher regression risk than persistence because the current app has no modal-layer concept in `useGamepadNav`.
