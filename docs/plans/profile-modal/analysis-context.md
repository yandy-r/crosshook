# Context Analysis: profile-modal

## Executive Summary

`profile-modal` is a frontend-only refactor of the install-to-profile handoff. Keep install execution and reviewable draft generation in `useInstallGame` and `InstallGamePanel`, move review-session ownership into `ProfileEditorView`, render a real portal-based modal with shared profile form sections, and save through the existing `useProfile` persistence pipeline. The core goal is to replace the current forced tab switch with an in-flow review step that preserves install context and does not clobber the global profile editor draft until save succeeds.

The highest-risk areas are not backend plumbing. They are editor drift, focus/controller leakage behind a portaled modal, viewport-safe layout at `1280x800`, and preserving existing install-session behavior when the modal is dismissed, reopened, or replaced by a newer install result.

## Architecture Context

- Keep the change frontend-only for v1. No new Tauri commands, backend schema changes, migrations, or Rust persistence changes are required.
- `useInstallGame` remains the source of truth for install request state, install result, `reviewProfile`, candidate executables, and derived working-directory behavior.
- `InstallGamePanel` should stop handing the draft directly into global profile state. It should emit an explicit `InstallProfileReviewPayload` assembled from current frontend state: `profileName`, `reviewProfile`, `candidateOptions`, `helperLogPath`, `message`, and `source`.
- `ProfileEditorView` is the correct owner for `ProfileReviewSession` because it already coordinates the install/profile subtab boundary.
- The modal draft must be isolated from `useProfile` draft state. Auto-open after install must not overwrite whatever the user had been editing in the Profile tab.
- Save should go through a narrow `persistProfileDraft(name, profile)` helper inside `useProfile` so normalization, validation, `profile_save`, metadata sync, refresh, and reload stay in one place.
- Post-save behavior is explicit: persist, close modal, select the saved profile, then switch `editorTab` to `profile`.
- Preferred modal primitive is a custom `createPortal` overlay, not browser-default `<dialog>`, unless `<dialog>` is validated in target Tauri webviews first.

## Critical Files Reference

- `docs/plans/profile-modal/feature-spec.md`: primary source of truth for ownership, save boundary, visible sections, risks, and implementation phases.
- `docs/plans/profile-modal/research-technical.md`: best concise technical blueprint for component boundaries, payload/session types, and CSS envelope.
- `docs/plans/profile-modal/research-patterns.md`: codebase patterns to preserve when extracting shared form sections and save helpers.
- `docs/plans/profile-modal/research-integration.md`: exact Tauri/frontend integration points and transport-model constraints.
- `docs/plans/profile-modal/research-ux.md`: modal sizing, sticky chrome, focus placement, and blocked/incomplete review behavior.
- `src/crosshook-native/src/components/InstallGamePanel.tsx`: current install UI owner; replace `onReviewGeneratedProfile(...)` with payload-based modal opening.
- `src/crosshook-native/src/components/ProfileEditor.tsx`: current install/profile bridge; future owner of `ProfileReviewSession`, modal open/close logic, and post-save tab switch.
- `src/crosshook-native/src/hooks/useInstallGame.ts`: preserves install result derivation, executable-candidate handling, and `runtime.working_directory` updates when executable changes.
- `src/crosshook-native/src/hooks/useProfile.ts`: existing normalization, save validation, `profile_save`, settings sync, recent-files sync, list refresh, and saved-profile reload path.
- `src/crosshook-native/src/hooks/useGamepadNav.ts` and `src/crosshook-native/src/App.tsx`: critical for focus scope, controller traversal, `Escape`, and background-interaction constraints.
- `src/crosshook-native/src/types/install.ts`: correct home for `InstallProfileReviewPayload`.
- `src/crosshook-native/src/types/profile.ts`: canonical `GameProfile` shared by install result, modal draft, and TOML persistence.
- `src/crosshook-native/src/types/profile-review.ts`: planned new home for modal-local `ProfileReviewSession`.
- `src/crosshook-native/src/styles/theme.css`, `src/crosshook-native/src/styles/focus.css`, `src/crosshook-native/src/styles/variables.css`: shared shell, focus, spacing, and viewport styling system the modal should extend.
- `tasks/lessons.md`: relevant repo-specific warnings about gamepad handlers in editable controls and verifying dialog plugin permissions.

## Patterns to Follow

- Use hook-owned domain state with thin view components. Do not move install orchestration into the modal.
- Coordinate across feature slices in the parent component. `ProfileEditorView` should own review-session lifecycle; `InstallGamePanel` should only emit review payloads.
- Prefer derived state over duplicate truth. Do not add a second source for final executable; keep it on `draftProfile.game.executable_path`.
- Normalize at edit/save boundaries, not with UI fallbacks. Reuse `useProfile` normalization and validation rules.
- Split types by ownership: install transport in `install.ts`, modal-local session in `profile-review.ts`.
- Reuse shared field groups instead of cloning the editor. Extract form sections out of `ProfileEditor.tsx` before expanding modal-specific behavior.
- Follow existing async safety patterns: cancellation guards for late async work, `finally` for busy flags, and `string | null` error state.
- Extend the shared theme system (`crosshook-*` classes and CSS variables) instead of copying the older inline-style-heavy overlay pattern.

## Cross-Cutting Concerns

- Accessibility: modal must behave as a real modal with `role="dialog"`, `aria-modal="true"`, stable labelling, focus trap, `Escape` close, and focus restore.
- Controller/keyboard behavior: a portal can fall outside the current `useGamepadNav` root. Background controls must not remain reachable while the modal is open.
- Editable controls: do not let global controller handlers capture typing keys inside `input`, `textarea`, `select`, or `contenteditable` targets.
- Layout: target a large form-first shell that stays within the `1280x800` Tauri window. Header/footer stay visible; only the body scrolls.
- Review-state continuity: dismissing the modal preserves the current draft; closing must not call `reset()` on `useInstallGame`.
- Save boundary: no silent persistence. Save remains blocked until profile name and final executable are present.
- Install/result continuity: install failures remain in the install panel; manual verify with no active draft should show an explanatory blocked or empty state.
- Data safety: modal paths are user-editable text only. Opening the modal must not execute, resolve, or otherwise act on them.
- Doc/copy drift: existing user-facing copy still refers to reviewing in the Profile tab and will become stale once the modal ships.

## Parallelization Opportunities

- Stream 1: modal shell and styling.
  Deliver `ProfileReviewModal`, portal mounting, backdrop, sticky header/footer, internal scrolling, focus trap, and responsive CSS.
- Stream 2: shared form extraction.
  Pull reusable field groups from `ProfileEditor.tsx` into a shared component without changing field semantics.
- Stream 3: state and persistence wiring.
  Add `InstallProfileReviewPayload`, `ProfileReviewSession`, `onOpenProfileReview(...)`, and `persistProfileDraft(...)`.
- Stream 4: install-flow integration and behavior.
  Wire auto-open/manual reopen, dirty-session preservation, discard/replace confirmation, and post-save tab switch.
- Stream 5: copy/docs follow-up.
  Update UI copy and stale docs that still describe the old tab-switch handoff.

Dependencies:

- Stream 2 and Stream 3 can proceed in parallel.
- Stream 4 depends on the payload/session contract from Stream 3.
- Stream 1 can proceed mostly in parallel, but final focus/controller handling must be validated against Stream 4 integration.
- Stream 5 should wait until the final post-save UX is fixed.

## Implementation Constraints

- Do not add backend commands or persistence schema changes for v1.
- Do not hydrate the global profile draft just to open review. Save is the only point where the modal should cross into `useProfile`.
- Build the open payload from current frontend install state, not raw `InstallGameResult` alone. `reviewProfile` may already reflect user executable selection, and that derived state must be preserved.
- Preserve the current executable-to-working-directory derivation when modal edits change the executable.
- Auto-open should key off the presence of a reviewable draft, not only `ready_to_save`; `install_game` always returns `needs_executable_confirmation: true`.
- Modal close on dirty state should require explicit discard confirmation, especially before retry/reset/new install replaces the session.
- Candidate executable review should stay near the top of the modal and remain part of the same editable draft surface.
- Keep install-critical fields visible by default. Optional sections such as empty trainer details and working-directory override are the first candidates for collapse.
- Reuse Tauri dialog pickers for browse actions. Plugin access depends on capabilities already being granted; if picker behavior regresses, verify permissions before debugging elsewhere.
- Verification will be build/manual heavy. There is no dedicated frontend test harness, so plan on type/build validation plus manual checks for auto-open, reopen, blocked save, failed save, dirty close, controller navigation, and `1280x800` layout.

## Key Recommendations

- Treat this as a coordination refactor, not a new editor. The safest path is shared form extraction plus a thin modal shell.
- Implement `persistProfileDraft(...)` before wiring final save UI so the modal never needs to manipulate global draft state as a workaround.
- Validate focus/controller behavior early, not after layout polish. This is the most likely place for subtle regressions.
- Keep the modal standalone enough to review without the background install panel, but preserve the install page behind it so dismissal is cheap and contextual.
- Prefer deterministic, explicit state transitions: open from payload, edit local draft, save through shared pipeline, then switch tabs only after success.
- Plan a small copy/docs cleanup immediately after implementation, because multiple repo docs still describe the old “review in Profile tab” flow.
