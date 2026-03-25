## Executive Summary

The modal should behave as a true review step, not as a secondary tab: open automatically when install reaches a reviewable state, and open manually from the verify/review action with the same layout and state. Make it large enough to show the common profile fields by default, but cap its height to the viewport and let only the modal body scroll so the user never loses context or has to leave the install flow.

CrossHook already uses dark, glassy panels, strong borders, and bright blue accent states, so the modal should reuse that visual language rather than introducing a separate chrome. The safest UX pattern here is a centered modal dialog with a sticky header and footer, clear sectioning, and a scrollable interior that preserves keyboard focus and returns the user to the install flow when dismissed.

### Core User Workflows

- Happy path flow: the installer finishes, CrossHook detects that the generated profile is reviewable, and the modal opens automatically with the profile preloaded and ready to inspect. The user verifies the executable, prefix, and runner-related fields, makes any final edits, and confirms the profile without navigating to another tab.
- Happy path flow: if the user clicks the verify action manually, the same modal opens with the same data and state, so the manual and automatic entry points do not feel like different features. Keep the entry point flexible, but keep the review surface identical.
- Recovery/error flow: if install fails, the modal should not open automatically; the install panel should keep the error visible and the user should retry from the source workflow. If review data is incomplete, open the modal in a blocked review state that explains what is missing and points directly to the field or action needed to recover.
- Recovery/error flow: if the generated profile cannot be confirmed yet, keep the modal open with a clear reason and a single next action instead of dumping the user back into the main profile editor. This reduces back-and-forth when the only missing step is a final executable or path confirmation.

### UI and Interaction Patterns

- Use a centered modal dialog with a dimmed backdrop, matching the existing panel/card treatment in [theme.css](/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/styles/theme.css) and the blue accent system in [variables.css](/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/styles/variables.css). Keep the shell visually distinct from the page, but not so different that it feels like a different app.
- Give the modal a generous width on desktop, then fall back to a near-full-screen layout on smaller screens. A practical target is a width that can show most profile fields side-by-side on a 1280x800 window, with `max-height` constrained to `100dvh` minus outer padding.
- Make the header and footer sticky, and let only the body scroll. This keeps the title, primary action, and dismiss action visible while the user moves through long content.
- Reuse the existing profile section order from [ProfileEditor.tsx](/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/components/ProfileEditor.tsx): profile identity, game, trainer, and runtime-related sections. Users should recognize the structure immediately.
- If the modal still overflows at the chosen width, collapse to one column rather than shrinking controls below comfortable touch and pointer targets. The goal is to reduce horizontal scanning, not to compress the form into unreadable density.
- Use a compact summary area at the top of the modal for the profile name, target executable, prefix, and current state so the user can orient themselves before scrolling.
- Keep the close affordance obvious and persistent. Do not force dismissal through a nested control or a keyboard shortcut only.

### Accessibility Considerations

- Use `role="dialog"` with `aria-modal="true"` and a clear `aria-labelledby` title, per WAI-ARIA APG guidance for modal dialogs. For complex, sectioned content, avoid overloading `aria-describedby` with a long blob of text; announce the dialog title and let the structured content speak for itself. See the WAI APG [Dialog (Modal)](https://www.w3.org/WAI/ARIA/apg/patterns/dialog-modal/) pattern and [MDN dialog role](https://developer.mozilla.org/en-US/docs/Web/Accessibility/ARIA/Roles/dialog_role).
- Move focus into the modal on open and trap focus inside it until dismissal. When the user closes the modal, restore focus to the invoking control, which will usually be the install-panel review button or the control that launched verification.
- For automatic open after install, land focus on the modal heading or the first meaningful control, not on a hidden action or a field far down the form. This makes the first screen-reader announcement useful and prevents a disorienting jump.
- Support `Escape` to close, but keep a visible close button in the header for pointer users and for users who do not rely on keyboard shortcuts.
- Keep all interactive controls at or above the existing 48px touch target used by the CrossHook theme. The modal should feel usable on a Steam Deck trackpad and on a desktop mouse.
- If the modal includes a scrollable body, ensure focused elements are never obscured by sticky chrome or clipped at the edges. Internal scroll containers should use `overflow-y: auto` and avoid allowing the page behind the dialog to scroll.
- When a field-level validation error appears inside the modal, move focus to the first invalid field or an error summary if the dialog opens in an error state. Do not leave the user hunting for the problem after a failed verify/save action.

### Feedback and State Design

- Loading state: show a short in-modal loading state while the generated profile or latest reviewable data is being hydrated. Keep the shell visible so the user understands they are still in the review flow.
- Empty state: if there is no generated profile yet or the install did not produce a reviewable result, explain that explicitly and offer the primary next step instead of an empty form. This is especially important for the manual verify action.
- Success state: after confirm/save, either close the modal immediately and return the user to the install flow, or leave a concise success summary and a clear “continue” action. Do not keep the user staring at a dormant form after completion.
- Error state: surface install or validation errors inline near the top of the modal, then anchor the first relevant field error in the body. If the backend returns a recoverable issue, explain the fix in user language rather than only echoing a raw command or path error.
- Transitional state: when the install result has arrived but the profile is still being normalized, show a “ready for review” state that can open automatically without pretending the profile is final. This avoids opening a modal that looks broken or half-populated.

### UX Risks

- The modal may become too tall on smaller windows or Steam Deck resolutions. Mitigate this with a strict viewport cap, a scrollable body, and a one-column fallback.
- Automatic opening can feel intrusive if the profile is not actually ready to inspect. Mitigate by opening only when the review data is complete enough to act on, and keep the manual verify action available as the fallback.
- Users may lose context if the close action returns them to an unexpected place. Mitigate by restoring focus to the triggering control and preserving install state on close.
- Internal scrolling can hide critical controls if the footer is not sticky. Mitigate by keeping the main action row visible and by allowing the body to scroll independently.
- Screen-reader users may miss the purpose of the dialog if the title and initial focus are weak. Mitigate with a strong dialog label, immediate announcement of the review state, and a predictable focus order.
