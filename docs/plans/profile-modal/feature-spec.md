# Feature Spec: Install-Flow Profile Review Modal

## Executive Summary

This feature replaces the current install-flow tab handoff with a large review modal that opens after install and can be reopened from a verify action. The goal is to let users confirm or correct the generated profile without leaving the install flow. The recommended implementation stays frontend-only: keep `useInstallGame` as the install-result source, add a modal-local review session in `ProfileEditorView`, render shared profile form sections inside a viewport-safe dialog shell, and save through the existing profile persistence path. The main risks are editor drift, modal accessibility and controller behavior, and keeping the form usable on 1280x800 and Steam Deck-sized viewports.

## External Dependencies

### APIs and Services

#### React DOM Portal

- **Documentation**: [React `createPortal`](https://react.dev/reference/react-dom/createPortal)
- **Authentication**: None
- **Key Endpoints**:
  - `createPortal(children, domNode)`: mount modal content outside the install panel layout while preserving React context and event bubbling
- **Rate Limits**: None
- **Pricing**: None
- **Recommendation**: Use this as the primary modal primitive because it requires no new dependency and avoids relying on browser-default dialog styling.

#### WAI-ARIA Modal Dialog Pattern

- **Documentation**: [WAI-ARIA APG Dialog (Modal) Pattern](https://www.w3.org/WAI/ARIA/apg/patterns/dialog-modal/)
- **Authentication**: None
- **Key Endpoints**:
  - `role="dialog"` and `aria-modal="true"` semantics
  - focus placement, focus trapping, `Escape` close, and focus restoration expectations
- **Rate Limits**: None
- **Pricing**: None
- **Recommendation**: Treat this as the accessibility contract for the custom modal shell.

#### HTML `dialog`

- **Documentation**: [MDN `HTMLDialogElement.showModal()`](https://developer.mozilla.org/en-US/docs/Web/API/HTMLDialogElement/showModal)
- **Authentication**: None
- **Key Endpoints**:
  - `showModal()`
  - top-layer backdrop behavior
  - built-in inert background behavior in supporting engines
- **Rate Limits**: None
- **Pricing**: None
- **Recommendation**: Keep as a fallback option, but do not make it the primary design assumption until it is validated in target Tauri webviews.

#### Tauri JavaScript Core API

- **Documentation**: [Tauri JavaScript API](https://v2.tauri.app/reference/javascript/api/) and [Core namespace](https://v2.tauri.app/reference/javascript/api/namespacecore/)
- **Authentication**: Local bridge only
- **Key Endpoints**:
  - existing `invoke()` calls for `profile_save`, `profile_list`, `profile_load`, settings sync, and recent-files sync
- **Rate Limits**: None
- **Pricing**: None
- **Recommendation**: Reuse the existing IPC path for save and refresh operations instead of adding new backend modal-specific commands.

### Libraries and SDKs

| Library                     | Version               | Purpose                                                                     | Installation      |
| --------------------------- | --------------------- | --------------------------------------------------------------------------- | ----------------- |
| `react-dom`                 | existing repo version | `createPortal` for overlay rendering without layout clipping                | already installed |
| `@tauri-apps/api`           | existing repo version | save, refresh, and metadata sync through current Tauri commands             | already installed |
| `@tauri-apps/plugin-dialog` | existing repo version | existing file and directory browse actions inside the shared profile fields | already installed |

### External Documentation

- [WAI-ARIA APG Dialog (Modal) Pattern](https://www.w3.org/WAI/ARIA/apg/patterns/dialog-modal/): modal accessibility behavior and focus requirements.
- [MDN dialog role](https://developer.mozilla.org/en-US/docs/Web/Accessibility/ARIA/Roles/dialog_role): labeling and accessibility usage notes.
- [MDN `inert`](https://developer.mozilla.org/en-US/docs/Web/HTML/Reference/Global_attributes/inert): background interaction suppression if a custom modal shell is used.
- [MDN `overflow`](https://developer.mozilla.org/en-US/docs/Web/CSS/overflow) and [MDN `scrollbar-gutter`](https://developer.mozilla.org/en-US/docs/Web/CSS/scrollbar-gutter): internal scrolling behavior for a large, viewport-capped modal.

## Business Requirements

### User Stories

**Primary User: Player finishing a non-Steam install**

- As a user, I want the generated profile review to appear immediately after install succeeds so I can verify or correct it without leaving the install flow.
- As a user, I want to review the suggested final executable, prefix, Proton path, and other launch-critical settings in one place before saving.
- As a user on a smaller display, I want the review experience to stay inside the viewport and scroll internally instead of overflowing the page.

**Secondary User: Returning user refining the generated profile**

- As a user, I want to reopen the same review draft from a verify action if I dismissed the modal without saving.
- As a user, I want modal edits to remain draft-only until I explicitly save.
- As a user using keyboard or controller navigation, I want the modal to be fully usable without relying on pointer-only interactions.

### Business Rules

1. **In-flow review**: Successful installs must review the generated profile in-place instead of switching the user to the Profile tab.
   - Validation: install success opens the review modal automatically when a reviewable profile exists.
   - Exception: failed installs stay in the install panel and do not open the modal.

2. **Manual reopen**: The install flow must expose a verify action that reopens the current review draft if the session still holds one.
   - Validation: dismissing the modal preserves the session draft until reset, retry, or replacement by a newer install result.
   - Exception: if no review draft exists, verify opens an explanatory empty or blocked state rather than a blank editor.

3. **Explicit save boundary**: Closing the modal must not silently persist the profile.
   - Validation: persistence only occurs through a modal save action.
   - Exception: none.

4. **Post-save handoff**: Saving from the modal must immediately switch the user to the Profile tab with the saved profile selected.
   - Validation: successful save persists the draft, selects the saved profile, and changes `editorTab` to `profile`.
   - Exception: save failures keep the user in the modal.

5. **Viewport-safe presentation**: The modal must be large enough to show all or most commonly edited fields without overflowing the page.
   - Validation: at 1280x800, the shell remains within the viewport and the body scrolls internally when needed.
   - Exception: on narrower windows, the layout may collapse to one column.

6. **Launch-critical completeness**: Save must stay blocked until the generated draft satisfies the existing save boundary.
   - Validation: non-empty profile name and final executable path are required before save.
   - Exception: none.

7. **Install/session separation**: Post-install review edits change future launch behavior only; they do not rerun or retroactively change the completed installer execution.
   - Validation: modal edits do not call install commands.
   - Exception: retrying install starts a new install path and may replace the draft after explicit discard confirmation.

### Edge Cases

| Scenario                                              | Expected Behavior                                                                 | Notes                                             |
| ----------------------------------------------------- | --------------------------------------------------------------------------------- | ------------------------------------------------- |
| Install succeeds but no executable candidate is found | Open the modal in an incomplete review state and focus the final executable field | Keeps user in-flow while blocking save            |
| User dismisses the modal without saving               | Preserve the current draft and allow reopening from verify                        | No silent persistence                             |
| User retries install while the review draft is dirty  | Require explicit discard confirmation before replacing the draft                  | Prevent accidental data loss                      |
| Save fails inside the modal                           | Keep the modal open, preserve edits, and show the error inline                    | No forced tab switch                              |
| Save succeeds from the modal                          | Switch to the Profile tab with the saved profile selected                         | Transition must be explicit in the action copy    |
| Content exceeds modal height                          | Only the modal body scrolls; header and footer remain visible                     | Required for Steam Deck and small desktop windows |

### Success Criteria

- [ ] Successful installs no longer require a tab switch to verify or update the generated profile.
- [ ] The modal opens automatically on install completion when a reviewable draft exists and can be reopened from a verify action.
- [ ] Users can save the generated profile from the modal and are then taken to the Profile tab with that profile selected.
- [ ] At 1280x800, the modal remains within the viewport and long content is handled by internal scrolling.
- [ ] Dismissing and reopening the modal restores the in-progress draft for the active install session.
- [ ] Keyboard and controller users can complete the review flow without interacting with background controls.

## Technical Specifications

### Architecture Overview

```text
[InstallGamePanel]
       |
       v
[useInstallGame] ----> [InstallProfileReviewPayload]
       |                         |
       v                         v
[ProfileEditorView] ----> [ProfileReviewModal via createPortal]
       |                         |
       v                         v
[useProfile.persistProfileDraft] -> [Tauri profile_save + metadata sync]
```

Recommended approach:

- Keep the implementation frontend-only for v1.
- Let `InstallGamePanel` continue owning install execution, install stages, candidate discovery display, and review-draft readiness.
- Move review-session ownership to `ProfileEditorView`, which already coordinates the install tab and profile domain.
- Introduce a dedicated `ProfileReviewModal` shell rendered via `createPortal`.
- Extract reusable field groups from `ProfileEditor.tsx` so the modal and the normal profile tab share one form surface instead of diverging.
- Keep the install-critical fields visible by default: Profile Identity, Game, Runner Method, final executable review, Prefix Path, Proton Path, and Trainer when populated.
- Collapse only optional or override-heavy sections in the modal: empty Trainer details and Working Directory override. Hide Steam-only launcher fields entirely for install-generated `proton_run` profiles instead of collapsing them.

### Data Models

#### `InstallProfileReviewPayload`

| Field              | Type                                    | Constraints | Description                                                     |
| ------------------ | --------------------------------------- | ----------- | --------------------------------------------------------------- |
| `source`           | `'install-complete' \| 'manual-verify'` | required    | Tells the modal whether it auto-opened or was reopened manually |
| `profileName`      | `string`                                | required    | Draft profile name to save under                                |
| `generatedProfile` | `GameProfile`                           | required    | Reviewable profile snapshot produced by install flow            |
| `candidateOptions` | `InstallGameExecutableCandidate[]`      | required    | Existing executable candidates for quick final-target selection |
| `helperLogPath`    | `string`                                | optional    | Install log location for diagnostics                            |
| `message`          | `string`                                | optional    | Install result status summary                                   |

**Ownership:**

- Keep this type in `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/types/install.ts` because it is an install-flow handoff contract assembled from install result data.

#### `ProfileReviewSession`

| Field              | Type                                    | Constraints | Description                                  |
| ------------------ | --------------------------------------- | ----------- | -------------------------------------------- |
| `isOpen`           | `boolean`                               | required    | Modal visibility                             |
| `source`           | `'install-complete' \| 'manual-verify'` | required    | Entry mode                                   |
| `profileName`      | `string`                                | required    | Editable profile identifier                  |
| `originalProfile`  | `GameProfile`                           | required    | Baseline snapshot for discard/reset behavior |
| `draftProfile`     | `GameProfile`                           | required    | Current in-modal editable draft              |
| `candidateOptions` | `InstallGameExecutableCandidate[]`      | required    | Existing quick-pick executable candidates    |
| `helperLogPath`    | `string`                                | optional    | Diagnostics display                          |
| `installMessage`   | `string`                                | optional    | Result summary display                       |
| `dirty`            | `boolean`                               | required    | Unsaved modal-change tracking                |
| `saveError`        | `string \| null`                        | optional    | Modal-specific persistence error             |

**Ownership:**

- Place this type in `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/types/profile-review.ts` because it models modal-local state, not install transport data.

**Indexes:**

- None. These are transient frontend-only models.

**Relationships:**

- `generatedProfile`, `originalProfile`, and `draftProfile` all reuse the existing `GameProfile` schema.
- `draftProfile.game.executable_path` remains the canonical final-target field for save eligibility.

### API Design

#### Frontend callback: `onOpenProfileReview`

**Purpose**: Open or reopen the modal from the install flow with an explicit payload.

**Signature:**

```ts
type InstallGamePanelProps = {
  onOpenProfileReview: (payload: InstallProfileReviewPayload) => void;
};
```

**Rules:**

- Trigger automatically after a successful install yields a reviewable draft.
- Trigger manually from the verify action while the current session still holds the draft.
- Do not mutate the global profile-tab editor state when constructing this payload.

#### Frontend helper: `persistProfileDraft`

**Purpose**: Save a supplied modal draft through the current profile persistence path without first rebinding global editor state.

**Signature:**

```ts
type PersistProfileDraftResult = { ok: true } | { ok: false; error: string };

type PersistProfileDraft = (name: string, profile: GameProfile) => Promise<PersistProfileDraftResult>;
```

**Behavior:**

- Normalize with the same logic currently used by `useProfile`.
- Validate with the same save boundary currently enforced by `saveProfile`.
- Invoke `profile_save`, metadata sync, and profile-list refresh.
- Select the saved profile and switch the editor to the Profile tab only after successful persistence.

**Errors:**

| Status            | Condition                                | Response                              |
| ----------------- | ---------------------------------------- | ------------------------------------- |
| client validation | missing profile name or final executable | keep modal open and show inline error |
| IPC failure       | `profile_save` or metadata sync fails    | keep modal open and preserve draft    |

### System Integration

#### Files to Create

- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/components/ProfileReviewModal.tsx`: modal shell, focus management, sticky header/footer, backdrop, and action layout.
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/components/ProfileFormSections.tsx`: shared profile field groups reused by both the modal and the normal profile tab.
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/types/profile-review.ts`: transient modal session types and modal-only view helpers.

#### Files to Modify

- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/components/InstallGamePanel.tsx`: replace the tab-switch callback with modal-open payload callbacks; keep verify action for reopen.
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/components/ProfileEditor.tsx`: own modal session state and integrate the shared form sections.
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/hooks/useProfile.ts`: expose a narrow save helper for externally supplied drafts.
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/styles/theme.css`: modal shell, overlay, internal scroll, and responsive layout rules.
- `/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/types/install.ts`: add `InstallProfileReviewPayload` alongside existing install result types.

#### Configuration

- No new backend configuration or Tauri capability changes are required for v1.

## UX Considerations

### User Workflows

#### Primary Workflow: Auto-open Review

1. **Install completes**
   - User: finishes installer flow inside the install panel.
   - System: receives the generated reviewable profile and opens the profile review modal automatically.

2. **Review and adjust**
   - User: verifies the executable, prefix, Proton path, trainer path, and other fields; edits any incorrect values.
   - System: updates the modal-local draft and keeps the save button disabled until required fields are complete.

3. **Save Profile**
   - User: clicks `Save and Open Profile Tab`.
   - System: persists the profile, closes the modal, selects the saved profile, and switches to the Profile tab.

#### Error Recovery Workflow

1. **Install fails**
   - User sees: failure remains in the install panel, not in a modal.
   - Recovery: correct install inputs or retry install.

2. **Review is incomplete**
   - User sees: modal opens in a blocked-but-editable state with the missing field clearly indicated.
   - Recovery: choose or browse to the final executable, then save.

3. **Modal is dismissed**
   - User sees: install panel remains reviewable with a verify action.
   - Recovery: reopen the same draft without rerunning install.

### UI Patterns

| Component           | Pattern                       | Notes                                                                                            |
| ------------------- | ----------------------------- | ------------------------------------------------------------------------------------------------ |
| Review shell        | large centered modal overlay  | Built with portal rendering, dimmed backdrop, and CrossHook theme tokens                         |
| Modal chrome        | sticky header + sticky footer | Keeps title and actions visible while body scrolls                                               |
| Body layout         | reused profile form sections  | Preserve the existing section order so the review surface feels familiar                         |
| Summary strip       | compact top summary card      | Show profile name, current executable, prefix, install status, and helper log path               |
| Candidate selection | quick-pick list near the top  | Keep executable confirmation close to the primary install-review task                            |
| Collapsed content   | optional overrides only       | Collapse empty Trainer details and Working Directory override; keep install-critical fields open |

### Accessibility Requirements

- Use `role="dialog"`, `aria-modal="true"`, and a stable `aria-labelledby` title.
- Move focus into the modal on open and restore it to the invoking control on close.
- Trap focus inside the modal while open and support `Escape` to dismiss.
- Prevent background interaction while the modal is active; background controls must not stay keyboard- or controller-reachable.
- Keep target sizes aligned with the existing 48px touch target minimum.
- Ensure internal scrolling does not hide the focused element behind sticky header or footer regions.

### Performance UX

- **Loading States**: opening the modal should use existing frontend install state and avoid a backend fetch.
- **Optimistic Updates**: do not optimistically claim persistence success; only close after the save path completes.
- **Error Feedback**: show save failures inline in the modal and preserve current edits.
- **Viewport Handling**: use a large desktop width but cap the shell to viewport height and scroll only the body.
- **Action Copy**: primary action copy should explicitly announce the transition, for example `Save and Open Profile Tab`, with helper text such as `Saves this profile and switches you to the Profile tab for further edits.`

## Recommendations

### Implementation Approach

**Recommended Strategy**: Build a custom portal-based `ProfileReviewModal` that opens from install success and manual verify, edits a modal-local `GameProfile` draft, and saves through a new narrow `useProfile` helper instead of hydrating and switching the full profile tab.

**Phasing:**

1. **Phase 1 - Modal Shell**: add overlay rendering, focus management, sticky chrome, viewport-safe sizing, and internal scrolling.
2. **Phase 2 - Shared Form Surface**: extract reusable profile form sections from `ProfileEditor.tsx` and render them inside the modal with the install summary and candidate picker.
3. **Phase 3 - Persistence Integration**: add `persistProfileDraft`, wire save and discard behavior, and remove the tab-switch handoff as the primary path.

### Technology Decisions

| Decision           | Recommendation                                               | Rationale                                                                               |
| ------------------ | ------------------------------------------------------------ | --------------------------------------------------------------------------------------- |
| Modal primitive    | custom portal overlay                                        | Better control over styling and predictable behavior across Tauri webviews              |
| Data source        | existing install result and `reviewProfile`                  | Avoids backend expansion for v1                                                         |
| Save path          | narrow `useProfile` helper plus post-save tab switch         | Reuses normalization and persistence without mutating global editor state before save   |
| Form reuse         | shared field-group extraction                                | Prevents UI drift between modal and Profile tab                                         |
| Open behavior      | automatic on successful reviewable result plus manual reopen | Matches the requested UX and preserves user control                                     |
| Collapsed sections | only optional overrides                                      | Keeps most settings visible while avoiding unnecessary height                           |
| Type placement     | split by ownership                                           | Install payload stays in `install.ts`; modal session state lives in `profile-review.ts` |

### Quick Wins

- Reuse existing CrossHook panel, card, button, and theme variables for visual consistency.
- Keep the executable candidate list and summary strip at the top of the modal so the install-specific task is immediately obvious.
- Make the body scroll independently from the page so long forms stay usable on small windows.
- Preserve install session state after dismiss so the verify action can reopen the same draft.

### Future Enhancements

- Add section anchors or a mini navigation rail if the shared form becomes very long.
- Highlight fields derived from install output versus fields manually edited by the user.
- Add a user preference to suppress auto-open if it proves too intrusive for repeat installers.

## Risk Assessment

### Technical Risks

| Risk                                                       | Likelihood | Impact | Mitigation                                                                                          |
| ---------------------------------------------------------- | ---------- | ------ | --------------------------------------------------------------------------------------------------- |
| Modal and Profile tab drift into separate editors          | Medium     | High   | Extract shared form sections before expanding modal-specific behavior                               |
| Focus and controller navigation leak behind the overlay    | Medium     | High   | Implement focus trapping, focus restore, background inertness, and explicit controller verification |
| Modal overflows smaller viewports                          | Medium     | High   | Cap width and height to viewport, keep header/footer fixed, and scroll only the body                |
| Saving through global profile state causes clobbering      | Medium     | Medium | Use a modal-local draft and a narrow external save helper                                           |
| Auto-open feels intrusive when the draft is not actionable | Low        | Medium | Auto-open only when a reviewable draft exists and keep manual verify reopen available               |

### Integration Challenges

- `useInstallGame` currently models install completion as `review_required` even when a recommended executable is already prefilled, so the open condition should key off the presence of a reviewable draft rather than only `ready_to_save`.
- `ProfileEditor.tsx` currently mixes editor shell and form fields in one file, so sharing the form surface will require a small extraction refactor before the modal can reuse it cleanly.
- `useGamepadNav` must be checked with the portal overlay to ensure controller navigation does not reach background content.

### Security Considerations

- Treat all paths shown in the modal as user-editable text only; opening the modal must not execute or resolve them.
- Keep existing native file-picker behavior for browse actions rather than adding new path parsing or file-system heuristics in the modal.
- Preserve the current explicit-save boundary; dismiss actions must not imply persistence.

## Decisions Needed

- Should closing a dirty modal require confirmation immediately, or only when the user is about to reset, retry install, or navigate away?
- Should empty Trainer details start collapsed but auto-expand once a trainer path is present, or always stay collapsed behind an explicit toggle?
- Should the post-save helper note appear only in the modal footer, or also as part of the primary button label in narrower layouts?

## Risk Mitigations

- Extract shared profile sections before adding modal-only editing features.
- Keep persistence narrow and explicit through `persistProfileDraft`.
- Use a portal-based shell with ARIA-compliant modal behavior instead of relying on browser defaults.
- Validate the resulting layout and controller/focus behavior at the project’s real target viewport, especially 1280x800.

## Task Breakdown Preview

1. Extract reusable profile field sections from `ProfileEditor.tsx` so they can render in both the normal tab and the new modal.
2. Add transient review payload and session types plus `ProfileReviewModal` with portal rendering, sticky chrome, and scrollable body.
3. Replace `onReviewGeneratedProfile` with an in-flow review-open callback from `InstallGamePanel`.
4. Add `persistProfileDraft` to `useProfile` so the modal can save without hydrating global editor state first.
5. Update theme styles for overlay, shell, summary strip, responsive width, and internal scrolling.
6. Verify keyboard, controller, save-error, dismiss-reopen, and 1280x800 viewport behavior.

## Research References

- [research-external.md](./research-external.md)
- [research-business.md](./research-business.md)
- [research-technical.md](./research-technical.md)
- [research-ux.md](./research-ux.md)
- [research-recommendations.md](./research-recommendations.md)
