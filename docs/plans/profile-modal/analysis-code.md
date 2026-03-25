# Code Analysis: profile-modal

## Executive Summary

`profile-modal` should be implemented as a frontend-only coordination change, not a new backend workflow. The existing code already has the right domain seams: `useInstallGame` owns install execution and produces a mutable `reviewProfile`, `ProfileEditorView` owns cross-tab coordination, and `useProfile` owns normalization, validation, persistence, and metadata sync.

The biggest implementation risk is not persistence. It is modal behavior against the current focus/controller system. `useGamepadNav` scopes navigation to a single root element in [`src/crosshook-native/src/App.tsx`](../../src/crosshook-native/src/App.tsx) and [`src/crosshook-native/src/hooks/useGamepadNav.ts`](../../src/crosshook-native/src/hooks/useGamepadNav.ts). A portal mounted outside that root will not participate in controller navigation, and a modal mounted inside that root still needs background focus suppression because the hook has no notion of modal layers.

The second critical risk is state ownership drift. The current install review handoff in [`src/crosshook-native/src/components/ProfileEditor.tsx`](../../src/crosshook-native/src/components/ProfileEditor.tsx) hydrates the global editor and switches tabs immediately. For `profile-modal`, the draft must stay local to the review session until explicit save, then use the existing `useProfile` save path.

## Existing Code Structure

### Related Components

- [`src/crosshook-native/src/components/ProfileEditor.tsx`](../../src/crosshook-native/src/components/ProfileEditor.tsx) is the current coordination point between the normal profile editor and install flow. `handleInstallReview` at lines 249-252 hydrates the global editor state and flips `editorTab` back to `profile`.
- The same file also contains nearly the entire editable profile form, inline browse helpers, Proton install loading, and conditional runtime sections. This is the main extraction target for shared form sections.
- [`src/crosshook-native/src/components/InstallGamePanel.tsx`](../../src/crosshook-native/src/components/InstallGamePanel.tsx) owns the install UI and already presents the install-specific review context: status, candidate list, final executable field, generated profile preview, and helper log path.
- [`src/crosshook-native/src/App.tsx`](../../src/crosshook-native/src/App.tsx) owns the global `useProfile` instance, the app tab layout, and the `useGamepadNav` root ref. `ProfileEditorView` is already embedded here as a child coordinator rather than managing app-wide state itself.

### File Organization Pattern

- Domain state lives in hooks, not components. `useInstallGame` and `useProfile` both expose derived booleans, callbacks, and already-normalized data.
- UI files are currently mixed. `ProfileEditor.tsx` is a monolith containing shared helpers, view state, async effects, and full form rendering. `InstallGamePanel.tsx` duplicates some of that helper logic.
- Types are split by domain:
  - [`src/crosshook-native/src/types/install.ts`](../../src/crosshook-native/src/types/install.ts) holds install request/result/stage types.
  - [`src/crosshook-native/src/types/profile.ts`](../../src/crosshook-native/src/types/profile.ts) holds `GameProfile`, which mirrors the persisted Rust shape.
- Backend command files are thin Tauri bridges over `crosshook-core` and are not shaping modal behavior directly.

## Implementation Patterns

### Pattern: Hook-Owned Domain State

- `useInstallGame` owns install request state, validation state, stage transitions, candidate derivation, prefix resolution, and the mutable install review profile in [`src/crosshook-native/src/hooks/useInstallGame.ts`](../../src/crosshook-native/src/hooks/useInstallGame.ts).
- `useProfile` owns profile normalization, validation, IPC save/load/delete, metadata sync, profile list refresh, and dirty/error state in [`src/crosshook-native/src/hooks/useProfile.ts`](../../src/crosshook-native/src/hooks/useProfile.ts).
- Actionable use: keep install execution and install-result derivation inside `useInstallGame`; keep persistence inside `useProfile`; add a narrow modal save helper instead of moving save logic into the modal component.

### Pattern: Parent-Owned Coordination

- `ProfileEditorView` coordinates the current handoff from install flow to profile editing through a callback passed into `InstallGamePanel`.
- The modal session should follow the same pattern: `InstallGamePanel` emits a payload, `ProfileEditorView` owns the session state, and the modal remains a rendered child owned by `ProfileEditorView`.
- Actionable use: do not introduce global store state for the modal.

### Pattern: Immutable Nested Updates

- Both `ProfileEditor.tsx` and `useInstallGame.ts` update nested `GameProfile` state with functional `setState` callbacks and object spreads.
- Actionable use: modal draft editing should keep the same updater shape so extracted form sections can work against both the profile tab and modal draft.

### Pattern: Normalize At Edit And Save Boundaries

- `normalizeProfileForEdit` and `normalizeProfileForSave` in [`src/crosshook-native/src/hooks/useProfile.ts`](../../src/crosshook-native/src/hooks/useProfile.ts) are the canonical profile cleanup points.
- `normalizeProfileForEdit` resolves launch method, trims runtime fields, strips the automatic launcher suffix, and forces `steam.enabled` based on resolved launch mode.
- `normalizeProfileForSave` derives `game.name` and launcher display name before persistence.
- Actionable use: modal save must call these same helpers. Do not duplicate normalization in the modal component.

### Pattern: Derived State Over Duplicate Truth

- Install candidate options are derived from the backend result in `createCandidateOptions`.
- Install review stage and copy are derived from the install result and current executable confirmation state.
- `setInstalledExecutablePath` updates both the request field and the review profile, and also derives `runtime.working_directory` from the selected executable.
- Actionable use: keep candidate options and launch-critical draft fields derived from a single review-session source of truth rather than tracking separate modal-only mirrors of executable and working directory.

### Pattern: Async Effect With Active Guard

- Both `ProfileEditor.tsx` and `InstallGamePanel.tsx` fetch Proton installs with an `active` boolean to avoid setting stale state after unmount.
- Actionable use: reuse that pattern for focus restoration targets, modal open side effects, or any lazy data load tied to modal lifecycle.

### Pattern: Thin Tauri Command Boundary

- [`src/crosshook-native/src-tauri/src/commands/profile.rs`](../../src/crosshook-native/src-tauri/src/commands/profile.rs) and [`src/crosshook-native/src-tauri/src/commands/install.rs`](../../src/crosshook-native/src-tauri/src/commands/install.rs) only map request/response values and errors. They do not contain frontend workflow policy.
- Actionable use: modal behavior should stay frontend-only unless a genuine backend data contract change is required.

### Pattern: CSS Utility Classes Over One-Off Modal Styling

- [`src/crosshook-native/src/styles/theme.css`](../../src/crosshook-native/src/styles/theme.css) already defines reusable classes for panels, cards, fields, buttons, install layouts, and responsive breakpoints.
- [`src/crosshook-native/src/styles/focus.css`](../../src/crosshook-native/src/styles/focus.css) defines controller-aware focus treatments and touch-target minimums.
- Actionable use: the modal shell should introduce new `crosshook-*` classes beside these patterns instead of adding another large block of inline styles like the legacy `ProfileEditor.tsx` form.

## Integration Points

### Files to Create

- `src/crosshook-native/src/components/ProfileReviewModal.tsx`
  - Portal-based shell, backdrop, sticky header/footer, summary strip, focus entry/restore, and close/save actions.
- `src/crosshook-native/src/components/ProfileFormSections.tsx`
  - Extracted shared form groups from `ProfileEditor.tsx` with prop-driven draft updates and optional section collapsing.
- `src/crosshook-native/src/types/profile-review.ts`
  - Modal-local session state such as `isOpen`, `draftProfile`, `originalProfile`, `dirty`, `saveError`, and candidate/log metadata.

### Files to Modify

- [`src/crosshook-native/src/components/ProfileEditor.tsx`](../../src/crosshook-native/src/components/ProfileEditor.tsx)
  - Replace `handleInstallReview` global hydration with modal session ownership.
  - Render shared form sections in both the profile tab and modal.
  - Add explicit post-save behavior: select saved profile and switch back to the Profile tab.
- [`src/crosshook-native/src/components/InstallGamePanel.tsx`](../../src/crosshook-native/src/components/InstallGamePanel.tsx)
  - Replace `onReviewGeneratedProfile(name, profile)` with a payload callback.
  - Auto-open from successful install result and keep manual verify reopen behavior.
  - Preserve install panel ownership of install execution, candidate selection, and reset/retry.
- [`src/crosshook-native/src/hooks/useProfile.ts`](../../src/crosshook-native/src/hooks/useProfile.ts)
  - Expose a `persistProfileDraft(name, profile)`-style helper that reuses normalization, validation, `profile_save`, metadata sync, and list refresh without requiring pre-save global hydration.
- [`src/crosshook-native/src/types/install.ts`](../../src/crosshook-native/src/types/install.ts)
  - Add `InstallProfileReviewPayload` because this is install-domain transport data.
- [`src/crosshook-native/src/styles/theme.css`](../../src/crosshook-native/src/styles/theme.css)
  - Add overlay, shell, body scroll, sticky chrome, summary strip, and responsive modal sizing classes.
- [`src/crosshook-native/src/styles/focus.css`](../../src/crosshook-native/src/styles/focus.css)
  - Likely minor additions for modal focus scope or layered focus styles if the shell introduces new wrappers.
- [`src/crosshook-native/src/App.tsx`](../../src/crosshook-native/src/App.tsx)
  - Possibly no code change is required, but this becomes a modification target if portal mounting or gamepad root scoping needs an in-root mount node or app-level background suppression.

Backend files are integration references rather than expected modification targets. Existing profile and install commands already support the needed save and install flows.

## Code Conventions

### Naming

- React components use `PascalCase`; hooks use `useX`; local helpers use `camelCase`.
- Backend-mirrored install request fields are `snake_case` in TypeScript because they cross the Tauri boundary as-is in [`src/crosshook-native/src/types/install.ts`](../../src/crosshook-native/src/types/install.ts).
- `GameProfile` also uses nested `snake_case` keys such as `executable_path`, `prefix_path`, and `working_directory` because it mirrors the Rust serde schema in [`src/crosshook-native/crates/crosshook-core/src/profile/models.rs`](../../src/crosshook-native/crates/crosshook-core/src/profile/models.rs).
- New frontend-only session types should use normal TS `camelCase` names. Do not “fix” persisted `GameProfile` keys to camelCase.
- Important mapping gotcha: TS uses `trainer.type`, while Rust stores that field as `kind` with `#[serde(rename = "type")]`. Preserve the existing external field name.

### Error Handling

- Frontend hooks store user-facing errors as `string | null`; they do not rethrow for UI flow control.
- `useInstallGame` maps backend validation message substrings back onto field errors. This is fragile but currently established behavior.
- `useProfile` validates locally before invoking `profile_save`.
- Tauri commands stringify backend errors instead of returning typed frontend error objects.
- Modal save should follow the same pattern: validate first, preserve draft on failure, and surface inline string errors instead of closing or switching tabs.

### Testing

- Existing automated coverage in the relevant files is Rust-side:
  - install validation, prefix derivation, discovered executable preference, and generated profile shape in [`src/crosshook-native/crates/crosshook-core/src/install/service.rs`](../../src/crosshook-native/crates/crosshook-core/src/install/service.rs)
  - profile delete launcher cleanup behavior in [`src/crosshook-native/src-tauri/src/commands/profile.rs`](../../src/crosshook-native/src-tauri/src/commands/profile.rs)
- No frontend test harness is visible in the relevant files, so the modal feature should plan on manual verification unless the broader repo already has unused frontend test setup elsewhere.
- Practical verification checklist:
  - successful install auto-opens the modal when a draft exists
  - dismiss and reopen restores the same unsaved draft
  - save failure keeps the modal open and preserves edits
  - save success persists, refreshes profiles, selects the saved profile, and switches to the Profile tab
  - controller, keyboard `Tab`, and `Escape` do not reach background controls
  - layout remains usable at 1280x800 with only modal body scroll

## Dependencies and Services

### Available Utilities

- `normalizeProfileForEdit`, `normalizeProfileForSave`, `validateProfileForSave`, `mergeRecentPaths`, and `syncProfileMetadata` in [`src/crosshook-native/src/hooks/useProfile.ts`](../../src/crosshook-native/src/hooks/useProfile.ts)
- `setInstalledExecutablePath`, `createCandidateOptions`, `resolveDefaultPrefixPath`, and stage/copy derivation helpers in [`src/crosshook-native/src/hooks/useInstallGame.ts`](../../src/crosshook-native/src/hooks/useInstallGame.ts)
- `chooseFile`, `chooseDirectory`, `formatProtonInstallLabel`, and `deriveSteamClientInstallPath` already exist but are duplicated across components and are candidates for extraction during form sharing
- Existing Tauri commands:
  - `profile_save`, `profile_load`, `profile_list`
  - `settings_load`, `settings_save`
  - `recent_files_load`, `recent_files_save`
  - `validate_install_request`, `install_default_prefix_path`, `install_game`
  - `list_proton_installs`

### Required Dependencies

- `react-dom` for `createPortal`
- `@tauri-apps/api/core` for `invoke`
- `@tauri-apps/plugin-dialog` for file and directory pickers inside shared form sections

No new Rust crates or Tauri commands appear necessary for v1.

## Gotchas and Warnings

- `useInstallGame` currently reports successful install results as `review_required` whenever `needs_executable_confirmation` is true, and the backend currently always sets that flag to `true` in [`src/crosshook-native/crates/crosshook-core/src/install/service.rs`](../../src/crosshook-native/crates/crosshook-core/src/install/service.rs). That means the existing “Review in Profile” button is additionally gated on `stage === 'ready_to_save'`, even when the backend already returned a prefilled executable. The modal open condition should key off “reviewable draft exists”, not only `ready_to_save`.
- `setInstalledExecutablePath` updates `runtime.working_directory` whenever the executable changes. If the modal lets users edit working directory manually, executable quick-picks can overwrite that draft unless you explicitly separate “derived default” from “user override” behavior.
- `useGamepadNav` scopes focus traversal to `rootRef`. A portal mounted outside `rootRef.current` will not participate in its keyboard/controller loop.
- Even if the modal is mounted inside the gamepad root, `useGamepadNav` has no built-in modal stack. It will still collect every focusable element inside the root unless the background becomes genuinely non-focusable.
- `focus.css` respects `aria-hidden="true"` and `tabindex="-1"` on individual elements, but `useGamepadNav` itself does not explicitly check `inert` or modal boundaries. Background suppression needs to be validated in real behavior, not assumed from markup alone.
- `ProfileEditor.tsx` and `InstallGamePanel.tsx` currently duplicate browse helpers and Proton install selectors. If `ProfileFormSections` does not absorb these shared pieces, drift will continue.
- TS `GameProfile.runtime` is always present, but Rust skips serializing empty runtime sections. New code should treat the TS runtime object as required and rely on existing normalization/persistence behavior.
- `useProfile.saveProfile` refreshes the profile list and then reloads the saved profile. If you extract a reusable helper, do not accidentally hydrate global editor state before persistence or duplicate side effects in the wrong order.
- `reset()` in `useInstallGame` clears `result`, `reviewProfile`, and install state. Modal dismiss should not call install reset; only explicit reset/retry/discard should replace the session.

## Task-Specific Guidance

1. Start by extracting shared form sections from [`src/crosshook-native/src/components/ProfileEditor.tsx`](../../src/crosshook-native/src/components/ProfileEditor.tsx), not by building the modal first. That file is the main source of editor drift risk.
2. Define two separate types:
   - install-domain payload in `types/install.ts`
   - modal-local session in `types/profile-review.ts`
3. Keep `InstallGamePanel` responsible for install execution and review payload emission. It should not own modal visibility.
4. Put modal session ownership in `ProfileEditorView`, since it already coordinates install and profile editor tabs.
5. Add a narrow `useProfile` persistence helper that accepts `(name, profile)` and reuses `normalizeProfileForSave`, `validateProfileForSave`, `profile_save`, metadata sync, and profile refresh.
6. Use `createPortal`, but choose the mount location deliberately:
   - safest option is an in-root modal mount node so `useGamepadNav` can still see modal focusables
   - if mounted outside root, you will need extra navigation handling because the current hook will not manage it
7. Preserve current `GameProfile` field names and Rust compatibility. The modal should edit the same `GameProfile` shape the existing editor uses.
8. Keep install-critical sections expanded by default:
   - profile identity
   - game
   - runner method
   - final executable
   - prefix path
   - Proton path
   - trainer when populated
9. Treat backend files as read-only reuse unless implementation uncovers a real contract gap. The current install result already returns:
   - `profile_name`
   - `message`
   - `helper_log_path`
   - candidate paths
   - a reviewable `GameProfile`
10. Verify behavior against the actual high-risk cases, not just happy path save:

- prefilled executable with `review_required`
- no candidate found
- dirty dismiss and reopen
- retry install while draft is dirty
- controller navigation with portal modal open
