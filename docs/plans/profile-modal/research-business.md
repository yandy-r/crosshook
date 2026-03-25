# Profile Modal Business Research

## Executive Summary

The install-game flow should keep users in context by opening a large review modal as soon as a successful install produces a reviewable profile, instead of forcing a tab switch into the full Profile editor. The modal should operate on the same unsaved draft profile concept used today, let users verify or update the generated settings in-place, and preserve install context so review can be resumed from a verify action without rerunning the install.

## User Stories

- Primary: As a user finishing an install, I want the generated profile review to appear immediately in the install flow so I can confirm or correct launch settings without leaving the current task.
- Primary: As a user whose install produced multiple possible game executables, I want to review the suggested executable and change it before saving so CrossHook launches the right game binary.
- Primary: As a user on a smaller display such as Steam Deck, I want the review experience to fit in the viewport and scroll internally so I can access the whole profile without the page becoming unusable.
- Secondary: As a user who dismisses review to check something else in the install panel, I want to reopen the same review draft from a verify action and continue where I left off.
- Secondary: As a user making edits after install, I want those changes to remain a draft until I explicitly save so dismissing the modal does not silently persist incomplete or incorrect settings.
- Secondary: As a user relying on keyboard or controller navigation, I want the modal flow to remain usable without needing pointer-only interactions.

## Business Rules

### Core Rules

- The modal is part of the install-game workflow, not a separate navigation destination. A successful install should open the profile review modal in-context instead of switching the user to the Profile tab.
- The modal may also be reopened from an explicit verify action as long as the current install session still has a generated review draft.
- The generated review draft is the source of truth for modal edits until the user explicitly saves. Closing the modal must not silently save.
- The install session must remain intact while the modal is open or dismissed. Discovered executable candidates, helper log path, stage, and generated draft must remain available until the user resets the form, retries the install, or replaces the result with a newer install.
- The modal should expose all fields required for a viable saved profile and, where practical, most existing profile settings so the user does not need to leave the flow for routine review or correction.
- The modal should preserve CrossHook's existing profile semantics: install creates a reviewable draft, and save is the boundary that turns that draft into a persisted profile.
- The modal should be visually consistent with the current CrossHook dark theme and sized for dense form review. If content exceeds available height, only the modal body should scroll.
- Modal review must not break keyboard or controller-based traversal of the flow.

### Validations And Exceptions

- Failed installs do not open the modal. They stay in the install panel's error path and require the user to correct install inputs or retry.
- Save or confirm is blocked until the review draft satisfies the same blocking conditions as the current profile save path: a non-empty profile name and a non-empty final game executable path.
- If the install result did not discover a usable game executable, the modal still opens, but the review remains incomplete until the user selects or browses to the final executable.
- If a previously selected executable is cleared, the review returns to an incomplete state and save must be blocked again.
- Install-time validation rules remain separate from review-time profile editing. Installer path, Proton path, and prefix preparation must be valid before install starts; post-install modal edits do not rerun the installer.
- Editing high-impact runtime fields after install, such as launch method, prefix path, or Proton path, changes future launch behavior only. It does not retroactively change what the installer already did inside the original prefix.
- Actions that would discard the current review draft, such as reset or retry, should not silently throw away unsaved review edits.

## Workflows

### Primary Flow

1. The user completes the install form and starts the install.
2. CrossHook validates the install request, provisions the prefix if needed, and runs the installer through Proton.
3. When install succeeds, CrossHook derives a review draft profile, discovers executable candidates, pre-fills the best candidate when available, and opens the profile review modal immediately.
4. The user reviews and updates the draft inside the modal, with particular attention to the final executable, game name, trainer path, runtime settings, and other launch-critical profile fields.
5. If the draft is complete, the user saves from the modal. CrossHook persists the profile, updates related profile metadata, closes the modal, and leaves the user in the install flow with clear confirmation that the profile is now saved.
6. If the user dismisses the modal without saving, the install panel remains in a reviewable state and offers a verify action to reopen the same draft.

### Error Recovery Flow

1. If install validation fails before launch, CrossHook keeps the user in the install panel, shows field-level or general errors, and does not open the modal.
2. If the installer process fails, CrossHook keeps the user in the install panel, surfaces the failure message and log location when available, and allows retry after corrections.
3. If save fails from the review modal, the modal remains open, the current edits stay intact, and CrossHook shows the error without discarding the draft.
4. If the user dismisses the modal with an incomplete draft, CrossHook preserves the draft and returns the user to the install panel with review still pending.
5. If the user attempts to reset or retry while an unsaved review draft exists, CrossHook should require an explicit discard decision before replacing that draft with a new install session.

## Domain Concepts

- Install Session: The active install request plus its stage, validation state, helper log path, discovered executable candidates, and generated install result.
- Review Draft Profile: The editable, unsaved profile derived from the install result. This is the object the modal presents and mutates.
- Persisted Profile: The TOML-backed profile saved through the existing profile save flow. It is distinct from the draft until save succeeds.
- Review Completeness: A business state that is incomplete when the final executable is missing and complete when the executable is set and the draft is eligible for save.
- Executable Candidate Set: The ordered list of discovered game executables found in the installed prefix. The first candidate is the system recommendation but not an irreversible choice.
- Install Stage Transition: `idle -> preparing -> running_installer -> failed | review_required`. After a final executable is confirmed, the successful path advances to `ready_to_save`.
- Review Lifecycle Transition: `generated draft -> edited draft -> saved profile` or `generated draft -> dismissed draft -> reopened draft`.
- Draft Replacement Boundary: Resetting the install flow or completing a new install result replaces the previous review draft unless the product chooses to preserve drafts explicitly.

## Success Criteria

- Successful installs no longer require a tab switch to verify or update the generated profile.
- A successful install always yields an in-flow review opportunity: auto-open on completion, plus a verify action for reopening while the session is still active.
- At the application's target 1280x800 class viewport, the modal shows all or most key profile fields without leaving the viewport, and excess content is handled by an internal modal-body scroll area rather than page overflow.
- Users can save the generated profile from the modal without going to the Profile tab, and save failures do not discard in-progress edits.
- Review state is durable within the install session: dismissing and reopening the modal restores the current draft instead of regenerating or losing it.
- Users can complete the review flow with mouse, keyboard, or controller navigation.

## Open Questions

- Should the modal open automatically after every successful install, or only when the user has not already chosen to suppress auto-open for the current session?
- After a successful modal save, should the install panel show a distinct saved/completed state, or remain in `ready_to_save` with an option to reopen and edit?
- Should the modal expose the full existing profile editor surface, or should advanced sections be collapsed or deferred while still keeping the user in-flow?
- Which fields are intentionally editable in this modal even though changing them after install can diverge from the original install environment, especially launch method, prefix path, and Proton path?
- What exact discard experience is required when the user retries install, resets the form, closes the app, or navigates away while an unsaved review draft exists?
- The request references broader install-game planning docs under `docs/plans/install-game/`, but those documents are not present in this checkout. Is there another source of truth that should constrain the modal business rules further?
