# Profile Modal Research Recommendations

## Executive Summary

Use the install flow's existing review payload as the source of truth and surface it in a dedicated modal instead of switching tabs. The modal should be large, theme-matched, keyboard accessible, and internally scrollable so users can verify or adjust the created profile without leaving the install context.

This is best implemented as a thin UI layer on top of the current profile state and edit handlers, not as a second profile editor. That keeps the change localized, preserves CrossHook's existing profile model, and reduces the risk of diverging behavior between the install review path and the normal profile tab.

### Recommended Implementation Strategy

- High-confidence approach: introduce a `ProfileReviewModal` that opens from the install completion path and from any explicit verify action, then render the existing profile editing surface inside that modal or a modal-specific wrapper around the same field groups.
- Keep `useInstallGame` as the producer of the generated profile data and keep `ProfileEditor` as the owner of profile mutation and persistence. The modal should receive a hydrated profile snapshot plus callbacks for save, close, and edit-in-place actions.
- Size the modal for dense forms rather than a compact dialog: use a wide content area with `max-height` tied to viewport height and a scrollable body. That gives enough room for most sections while still preventing overflow on Steam Deck-sized displays.
- Prefer a single modal container over routing, tab switching, or a separate page. The user stays in context, the install state remains visible behind the modal, and the handoff becomes a lightweight confirmation step instead of a navigation event.
- Tradeoff: reusing the full profile editor in a modal reduces duplication, but it may require a modest refactor to separate shell/layout from form sections. That is still preferable to maintaining two parallel profile UIs.

## Phased Rollout Suggestion

- Phase 1: add the modal shell, open/close state, focus handling, viewport sizing, and scroll behavior, then route the current install review action into it while keeping the underlying profile data unchanged.
- Phase 2: move the profile review form content into the modal or extract shared form sections so the modal can edit the same fields the profile tab exposes. Validate that save and cancel behavior matches the existing profile editor.
- Phase 3: unify the install-review and profile-tab editing paths further by sharing validation, section rendering, and state synchronization, then retire the tab-switch handoff if it is no longer needed.

## Quick Wins

- Reuse existing theme primitives such as `crosshook-panel`, `crosshook-card`, `crosshook-button`, and the current dark palette instead of designing a one-off overlay.
- Add a prominent modal header with the generated profile name, a concise status line, and a clear primary action so users know they are reviewing a generated profile, not starting a new workflow.
- Make the body scroll independently from the modal chrome so long profile sections remain usable on smaller windows.
- Preserve the current install result summary behind the modal, so users can dismiss and return to the install workflow without losing context.

## Future Enhancements

- Add section-level anchors or a mini in-modal navigation rail for very long profiles.
- Support a diff-style review mode that highlights values derived from install output versus values the user manually changed.
- Remember the last modal section the user visited, especially if the profile review can be reopened multiple times during install iteration.
- Introduce modal-specific telemetry or event logging to measure how often users confirm versus edit generated profiles.

### Risk Mitigations

- Risk: the modal duplicates profile editor logic and drifts over time. Mitigation: extract reusable profile form sections or field groups before expanding modal functionality.
- Risk: the modal overflows on smaller screens or Steam Deck resolutions. Mitigation: enforce `max-height` based on viewport units, cap width at a large desktop-friendly size, and keep only the body scrollable.
- Risk: keyboard and gamepad navigation become harder in a dialog context. Mitigation: implement focus trapping, escape-to-close, restore focus to the triggering control, and verify controller traversal explicitly.
- Risk: users lose the current install context when closing the modal. Mitigation: keep install state intact, treat the modal as an overlay, and do not reset install state on dismiss.
- Risk: the save/confirm path becomes ambiguous. Mitigation: define one primary action in the modal, one secondary edit/return action, and make post-save behavior deterministic.

## Decision Checklist

- Should the modal reuse the full `ProfileEditor` form or a focused review subset?
- Should confirm/save happen inside the modal, or should the modal only hand off to the existing save flow after edits?
- Should the modal open automatically on install completion, or only after explicit user verification when the install result is ready?
- Should closing the modal return the user to the install panel, or advance them into the profile tab after the profile is hydrated?
- What is the minimum field set that must remain visible without scrolling on the target minimum window size?
