# Profile Modal External Research

## Executive Summary

CrossHook does not need a new modal package for this feature. The existing React 18 + Tauri v2 stack can implement the review surface with `react-dom` portals, browser-native dialog/accessibility primitives, and the current install-review state already produced by `useInstallGame` and `InstallGamePanel` ([InstallGamePanel.tsx](/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/components/InstallGamePanel.tsx), [useInstallGame.ts](/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/hooks/useInstallGame.ts)).

The main implementation risk is accessibility and focus management, not data plumbing. The modal should be treated as an overlay with a scrollable body, sticky chrome, and explicit focus return so the user can verify or update the generated profile without leaving the install flow.

### Candidate APIs and Services

#### React DOM `createPortal`

- Documentation URL: [React `createPortal`](https://react.dev/reference/react-dom/createPortal)
- Auth model: N/A
- Key endpoints/capabilities: Renders modal content outside the normal DOM subtree while preserving React context and event bubbling; ideal for escaping layout clipping in the install panel.
- Rate limits/quotas: None
- Pricing notes: None

#### HTML `dialog` element and `showModal()`

- Documentation URL: [MDN `HTMLDialogElement.showModal()`](https://developer.mozilla.org/en-US/docs/Web/API/HTMLDialogElement/showModal)
- Auth model: N/A
- Key endpoints/capabilities: Native modal top-layer behavior, built-in backdrop support, and browser-managed inert background handling.
- Rate limits/quotas: None
- Pricing notes: None

#### WAI-ARIA Modal Dialog Pattern

- Documentation URL: [W3C APG: Dialog (Modal) Pattern](https://www.w3.org/WAI/ARIA/apg/patterns/dialog-modal/)
- Auth model: N/A
- Key endpoints/capabilities: Defines the expected semantics for `role="dialog"`, `aria-modal="true"`, `aria-labelledby`, `Escape` handling, tab trapping, and initial focus placement for large content.
- Rate limits/quotas: None
- Pricing notes: None

#### `inert` global attribute / `HTMLElement.inert`

- Documentation URL: [MDN `inert` global attribute](https://developer.mozilla.org/en-US/docs/Web/HTML/Reference/Global_attributes/inert)
- Auth model: N/A
- Key endpoints/capabilities: Removes background content from the tab order and accessibility tree; useful if the modal is custom-built instead of using native `<dialog>`.
- Rate limits/quotas: None
- Pricing notes: None

#### CSS `overflow` and `scrollbar-gutter`

- Documentation URL: [MDN `overflow`](https://developer.mozilla.org/en-US/docs/Web/CSS/Reference/Properties/overflow) and [MDN `scrollbar-gutter`](https://developer.mozilla.org/en-US/docs/Web/CSS/Reference/Properties/scrollbar-gutter)
- Auth model: N/A
- Key endpoints/capabilities: `overflow-y: auto` plus a viewport-constrained `max-height` keeps long profile sections usable; `scrollbar-gutter: stable` helps prevent width shifts when scrollbars appear.
- Rate limits/quotas: None
- Pricing notes: None

#### Tauri JS bridge (`@tauri-apps/api/core`)

- Documentation URL: [Tauri JS API](https://v2.tauri.app/reference/javascript/api/) and [Tauri core namespace](https://v2.tauri.app/reference/javascript/api/namespacecore/)
- Auth model: Local app bridge; no external auth.
- Key endpoints/capabilities: Existing `invoke`-style backend calls can hydrate, validate, save, or refresh profile data without adding a second data path for the modal.
- Rate limits/quotas: None
- Pricing notes: None

## Libraries and SDKs

- `react-dom`: recommended because it already ships with the app and provides `createPortal`, which is the cleanest way to mount a modal above the install layout without introducing a new dependency.
- `@tauri-apps/api`: keep using the existing bridge for any profile hydration, validation, or save actions that need Rust-side logic; the modal should consume the same state and commands rather than own persistence.
- `@tauri-apps/plugin-dialog`: already present, but it is only relevant for native file pickers in the install flow. It does not solve the in-app review modal problem and should not be treated as a modal abstraction.

## Integration Patterns

- Recommended auth flow: none for the modal itself. Treat it as a local UI layer over the profile state already produced by the install flow, then hand any final write actions back through the existing profile/save commands.
- Sync/event/webhook strategy: no webhook or remote sync is needed. Open the modal from local React state when install reaches a reviewable state or when the user clicks verify, and close it by restoring focus to the trigger that opened it. If the backend must refresh data after a mutation, use the existing Tauri command bridge rather than a new event system.
- Pagination/error handling approach: profile review is not paginated, but long content should be chunked into sections with a scrollable body rather than a single oversized pane. Keep field-level validation inline, surface a concise error summary near the top of the modal, and do not let a failed verify/save action dismiss the dialog.

## Constraints and Gotchas

- A custom portal-based modal still needs real modal behavior: `role="dialog"`, `aria-modal="true"`, focus trap, `Escape` close, and focus restoration on dismiss.
- If you use native `<dialog>`, validate it in the target Tauri webviews. MDN shows broad browser support, but Tauri runs on embedded engines, so Linux/macOS/Windows behavior should be verified in-app rather than assumed.
- The modal body must be scrollable independently from the page. Without `max-height` plus `overflow-y: auto`, the dialog will overflow small windows and Steam Deck-sized viewports.
- For long content, APG recommends initial focus on a static heading or top-of-dialog element instead of the first interactive control, so the title and beginning of the review content stay visible.
- Background content should be made inert or otherwise non-interactive. APG warns that a dialog marked modal must actually behave modally for all users, not just sighted mouse users.
- The existing gamepad navigation hook (`useGamepadNav`) will need a deliberate plan if the modal is portaled outside the normal app root, otherwise keyboard and controller traversal can drift behind the overlay.
- `scrollbar-gutter: stable` is useful if the modal body scrolls, but it is not a substitute for a fixed-height dialog shell.

## Open Decisions

- Should the modal render the full `ProfileEditor` form, or only the review-critical sections used during install?
- Should the modal open automatically when install becomes reviewable, or only on an explicit verify action?
- Should save/confirm happen inside the modal, or should the modal only hand control back to the existing profile tab flow?
- Should we implement a custom portal overlay, or use native `<dialog>` and style the shell around it?
- How should controller navigation behave while the modal is open, given the current `useGamepadNav` pattern?
