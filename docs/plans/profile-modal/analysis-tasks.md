# Task Analysis: profile-modal

## Executive Summary

The cleanest implementation shape is four phases: contracts, shared UI extraction, flow integration, and hardening. The main bottleneck is [`src/crosshook-native/src/components/ProfileEditor.tsx`](/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/components/ProfileEditor.tsx), which is already 849 lines and currently owns tab orchestration plus the full profile form, so extraction and modal-session wiring should not be merged into one task.

The codebase already exposes most of the install-side data needed for the modal through [`src/crosshook-native/src/hooks/useInstallGame.ts`](/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/hooks/useInstallGame.ts), so there is no reason to create a core task for backend or install-hook changes. The fixed decisions fit naturally into the frontend split: `InstallProfileReviewPayload` belongs in `install.ts`, `ProfileReviewSession` belongs in a modal-owned type file, the modal should keep install-critical sections open while collapsing only the agreed optional sections, and successful save should still land in the Profile tab.

## Proposed Phase Structure

1. Phase 1, Contracts and Seams. Add the payload and session types, then carve the reusable form surface out of `ProfileEditor.tsx` before any modal logic lands.
2. Phase 2, Modal Surface. Build the portal shell, scrolling chrome, and modal-specific presentation around the extracted form sections and summary strip.
3. Phase 3, Flow Integration. Rewire `InstallGamePanel` to open the modal, add the narrow persistence helper in `useProfile`, and make `ProfileEditorView` own review-session state plus the post-save jump to the Profile tab.
4. Phase 4, Hardening and Copy. Add discard and replacement guards, verify controller and focus behavior, and update the install-context copy that still says review happens in the Profile tab.

## Candidate Tasks

1. `T1 Review payload and session types`
   Files: `src/crosshook-native/src/types/install.ts`, `src/crosshook-native/src/types/profile-review.ts`, `src/crosshook-native/src/types/index.ts`
   Scope: Add `InstallProfileReviewPayload` in `install.ts`, add `ProfileReviewSession` in a modal-owned type file, and export the new type only where shared consumption needs it. This task encodes the ownership split decision early and gives later component work stable contracts.

2. `T2 Shared profile form extraction`
   Files: `src/crosshook-native/src/components/ProfileFormSections.tsx`, `src/crosshook-native/src/components/ProfileEditor.tsx`
   Scope: Move the reusable field groups and section-order logic out of `ProfileEditor.tsx` into a shared presentational component. This is where the collapsed-sections policy should live: keep install-critical sections open, collapse empty Trainer details and Working Directory override, and avoid carrying Steam-only launcher fields into the install review path for `proton_run` drafts.

3. `T3 Modal shell and modal styling`
   Files: `src/crosshook-native/src/components/ProfileReviewModal.tsx`, `src/crosshook-native/src/styles/theme.css`, `src/crosshook-native/src/styles/focus.css`
   Scope: Build the portal-based shell, sticky header and footer, summary strip, scrollable body, backdrop, and focus affordances. Keep this task structural and styling-focused so it can proceed before the full data wiring is complete.

4. `T4 Install-flow handoff contract`
   Files: `src/crosshook-native/src/components/InstallGamePanel.tsx`
   Scope: Replace `onReviewGeneratedProfile` with a payload callback, assemble the payload from existing `result`, `reviewProfile`, `candidateOptions`, and install messaging, and preserve both auto-open and manual verify entry points. `useInstallGame.ts` should stay untouched unless implementation uncovers a missing field, because the hook already exposes the needed review state.

5. `T5 External draft persistence helper`
   Files: `src/crosshook-native/src/hooks/useProfile.ts`
   Scope: Add `persistProfileDraft(name, profile)` that reuses the existing normalize, validate, save, metadata-sync, and refresh flow without hydrating the global draft first. This task should keep the save boundary identical to today and leave the tab switch decision to the caller.

6. `T6 ProfileEditor review-session orchestration`
   Files: `src/crosshook-native/src/components/ProfileEditor.tsx`, `src/crosshook-native/src/components/ProfileReviewModal.tsx`
   Scope: Make `ProfileEditorView` own the modal session, open and reopen it from install callbacks, wire draft updates into the shared form sections, and call `persistProfileDraft` on save. This is the task that applies the post-save decision: successful save closes the modal, selects the saved profile, and switches `editorTab` to `profile`.

7. `T7 Dirty-dismiss and replacement guards`
   Files: `src/crosshook-native/src/components/ProfileEditor.tsx`, `src/crosshook-native/src/components/InstallGamePanel.tsx`, `src/crosshook-native/src/components/ProfileReviewModal.tsx`
   Scope: Add explicit discard confirmation for dirty close, and guard retry or reset flows so a newer install result cannot silently replace a dirty modal session. Keeping this separate from the base integration task reduces regression risk in the happy path.

8. `T8 Install-context copy alignment`
   Files: `src/crosshook-native/src/components/LaunchPanel.tsx`, `src/crosshook-native/src/components/LauncherExport.tsx`
   Scope: Replace the remaining “review in Profile” and “save in the Profile tab” language with modal-first copy that still explains the post-save jump. This task is low-risk and should stay isolated from the workflow wiring.

9. `CT1 Conditional controller-scope fallback`
   Files: `src/crosshook-native/src/App.tsx`, `src/crosshook-native/src/hooks/useGamepadNav.ts`
   Scope: Only add this task if the chosen portal target escapes the existing `gamepadNav.rootRef` focus scope and controller navigation can reach background controls. The preferred implementation is to avoid this task by keeping the modal host inside the current app focus scope.

## Dependency Recommendations

`T1` should land first because it stabilizes the callback and modal contracts.

`T2`, `T3`, and `T5` can start after `T1` and run independently because they touch different files and represent different concerns.

`T4` depends on `T1` but not on modal rendering or persistence internals.

`T6` should wait for `T2`, `T3`, `T4`, and `T5`. It is the integration point where the extracted form, modal shell, install payload, and persistence helper finally meet.

`T7` depends on `T6` because it hardens session lifecycle after the base open, edit, save, and reopen flow exists.

`T8` can wait until `T6` is stable so the final wording matches shipped behavior.

`CT1` should only be pulled in after `T3` proves that the modal host cannot stay within the current focus scope.

## Parallelization Strategy

The best first wave is four parallel lanes: `T2` on shared form extraction, `T3` on modal shell and CSS, `T4` on install-panel callback conversion, and `T5` on persistence. `T1` is small enough to finish before that wave starts.

Do not parallelize tasks that edit [`src/crosshook-native/src/components/ProfileEditor.tsx`](/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/components/ProfileEditor.tsx). `T2`, `T6`, and `T7` all touch that file and should be serialized to avoid conflict in the biggest file involved.

Do not parallelize `T3`, `T6`, and `T7` if the modal component is still moving quickly, because they all converge on `ProfileReviewModal.tsx`.

`T8` is a safe trailing parallel task once the final UX behavior is known.

## Risk-Weighted Ordering

1. `T1` first because it is cheap and removes ambiguity around type ownership.
2. `T2` second because shared-form extraction is the highest drift risk and sits inside the largest file.
3. `T5` third because persistence behavior is already critical and easier to verify before UI integration.
4. `T3` fourth because modal structure and viewport behavior are substantial but mostly local.
5. `T4` fifth because install-panel wiring is straightforward once the payload shape is settled.
6. `T6` sixth because it is the main join point and should happen only after the surrounding seams are stable.
7. `T7` seventh because dirty-session rules are easier to reason about after the base flow works.
8. `T8` last because copy should follow settled behavior.
9. `CT1` only if testing shows controller or focus leakage that cannot be solved within the modal task itself.

## Suggested Task Granularity

Six to eight core implementation tasks is the right size here. That keeps each task inside a 1 to 3 file boundary and avoids creating a single oversized “build the modal flow” task that would sprawl across `ProfileEditor.tsx`, `InstallGamePanel.tsx`, `useProfile.ts`, CSS, and types at once.

The strongest boundary is between extraction and integration. Any plan that combines `ProfileFormSections` extraction with modal-session ownership will be harder to review and harder to parallelize, because both changes need the same large source file and solve different risks.

The second strong boundary is between persistence and UI. `useProfile.ts` should be changed in one focused task, then consumed later from `ProfileEditor.tsx`, rather than mixing save-path refactors into modal wiring.

Backend and Tauri command work should stay out of the default breakdown. The current frontend state already carries `reviewProfile`, executable candidates, helper log path, and save commands, so adding backend tasks up front would widen the plan without reducing actual risk.
