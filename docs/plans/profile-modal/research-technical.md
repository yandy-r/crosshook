# Profile Modal Technical Specification

## Executive Summary

The profile-modal feature should stay entirely in the frontend and reuse the existing install result payload, `GameProfile` shape, and Tauri profile save flow. The safest architecture is to open a large review modal from the install flow while editing a modal-local draft, so CrossHook avoids forcing a tab switch and also avoids overwriting the app-wide `useProfile` draft state when install completion auto-opens review.

The modal should be visually aligned with the current CrossHook dark theme, sized for form-heavy content, and constrained with a scrollable body so it remains usable on 1280x800 and Steam Deck class viewports. Backend changes are not required for the initial implementation.

### Architecture Approach

- Component and service boundaries
- `InstallGamePanel` remains the install-flow orchestrator. It should continue owning `useInstallGame`, candidate selection, and review readiness detection, but stop directly handing off to the profile tab as the primary path.
- `ProfileEditorView` should own modal session state because it already coordinates the install tab, profile tab, and the shared profile domain concepts.
- Introduce a new `ProfileReviewModal` component responsible only for overlay rendering, focus management, scroll behavior, action layout, and wiring field groups into a modal shell.
- Extract the reusable profile field groups from `ProfileEditor.tsx` into a shared presentational unit such as `ProfileFormSections` or `ProfileEditorSections`. That avoids maintaining two divergent editors while preserving the existing section ordering and field semantics.
- Keep persistence in `useProfile`. Add a narrow imperative save helper for supplied profile data, instead of binding the modal directly to the app-wide `profile`, `profileName`, and `dirty` state.

- Recommended ownership model
- `useInstallGame` stays the source of truth for install output: `result`, `reviewProfile`, `candidateOptions`, `request.installed_game_executable_path`, and stage.
- The modal should use a transient `ProfileReviewSession` in React state, initialized from install output and owned by `ProfileEditorView`.
- The modal draft should be independent from the app-wide profile editor draft. This prevents install-complete auto-open from clobbering a profile the user had been editing before switching to the install tab.
- On save, the modal should call a new frontend helper such as `persistProfileDraft(name, profile)` inside `useProfile`, which reuses current normalization, validation, `profile_save`, settings sync, recent-files sync, and profile-list refresh behavior.

- Integration points with the existing system
- Replace the current `onReviewGeneratedProfile(profileName, profile)` handoff from `InstallGamePanel` with a payload-oriented callback such as `onOpenProfileReview(payload)`.
- `InstallGamePanel` should trigger that callback in two cases:
- automatically when install reaches `ready_to_save` and a valid `reviewProfile` exists
- manually when the user presses the existing secondary verify/review action after executable confirmation
- The payload should be assembled from existing frontend state, not new backend fields: `result.profile_name`, `reviewProfile`, `candidateOptions`, `result.helper_log_path`, `result.message`, and a `source` discriminator.
- `ProfileEditorView` should open the modal on that callback, seed the draft state, and leave the current `editorTab` unchanged.
- After save succeeds, close the modal, preserve install context, and optionally surface a lightweight success state with a secondary action to jump to the Profile tab if the user wants deeper edits.

- Modal layout contract
- Use a large centered overlay with a fixed header and footer plus a scrollable body.
- The modal body should render the same profile sections users already recognize in `ProfileEditor.tsx`: identity, game, trainer when relevant, and runtime-specific sections.
- Candidate executable review should stay visible near the top of the modal, because it is the core install-review action and already exists in `InstallGamePanel.tsx`.
- The install panel summary and log path can remain behind the modal in the underlying page, but the modal should repeat the critical install outcome fields needed for review so it stands alone.

### Data Model Implications

- Entities, tables, and collections
- No database tables, indexes, or backend collections are introduced.
- Persistent storage remains the existing profile TOML files handled by the current `profile_save` and `profile_load` Tauri commands.
- The canonical persisted entity remains `GameProfile` from [profile.ts](/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/types/profile.ts).

- New transient frontend models
- Introduce a modal-only view model, for example:

```ts
interface InstallProfileReviewPayload {
  source: 'install-complete' | 'manual-verify';
  profileName: string;
  generatedProfile: GameProfile;
  candidateOptions: InstallGameExecutableCandidate[];
  helperLogPath: string;
  message: string;
}

interface ProfileReviewSession {
  isOpen: boolean;
  source: 'install-complete' | 'manual-verify';
  profileName: string;
  originalProfile: GameProfile;
  draftProfile: GameProfile;
  candidateOptions: InstallGameExecutableCandidate[];
  helperLogPath: string;
  installMessage: string;
  dirty: boolean;
}
```

- `originalProfile` supports reset/discard behavior and future diff highlighting without re-querying the backend.
- `draftProfile` should be normalized with the same edit-time rules currently used by `useProfile`.
- The selected executable path does not need its own persistent model if it remains reflected in `draftProfile.game.executable_path`.

- Indexes and migration considerations
- No filesystem migration is needed because the saved TOML schema does not change.
- No new backend serialization fields are needed for the initial version.
- If future work adds modal-specific telemetry or saved review checkpoints, that should be a separate design because it would introduce new persistence concerns not required for this feature.

### API Design Considerations

- Endpoints and interfaces
- Do not add new Tauri commands for the initial implementation.
- Reuse the current install command result contract defined in [install.ts](/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/types/install.ts).
- Extend frontend-only interfaces instead:
- `InstallGamePanelProps` should accept something like `onOpenProfileReview(payload: InstallProfileReviewPayload): void`
- `ProfileReviewModalProps` should accept `open`, `session`, `onClose`, `onDiscard`, `onSave`, and `onUpdateProfile`
- `useProfile` should expose a narrow save helper for externally supplied drafts, for example `persistProfileDraft(name: string, profile: GameProfile): Promise<void>`

- Request and response shape guidance
- The modal-open payload should be fully derived from the existing frontend install state and should not mirror raw `InstallGameResult` wholesale.
- Keep the payload explicit and UI-oriented:

```ts
type InstallProfileReviewPayload = {
  source: 'install-complete' | 'manual-verify';
  profileName: string;
  generatedProfile: GameProfile;
  candidateOptions: InstallGameExecutableCandidate[];
  helperLogPath: string;
  message: string;
};
```

- `persistProfileDraft` should not require the modal to mutate the global `useProfile` draft first. It should normalize, validate, invoke `profile_save`, sync metadata, refresh the list, and optionally select the saved profile only after success.
- The modal component should treat candidate executable choice as a normal field update on `draftProfile.game.executable_path`, with `runtime.working_directory` updated using the same derivation rule already present in `useInstallGame`.

- Error handling model
- Install-time failures remain owned by `useInstallGame` and stay rendered in the install panel.
- Modal validation failures should use the same fail-fast rule as `useProfile`: block save when `game.executable_path` is empty.
- Save failures from `profile_save` or metadata sync should appear in the modal header or footer as a single prominent error message, with inline field errors only where the UI can map the failure deterministically.
- If the modal is dirty and the user tries to close it, require an explicit discard confirmation. Silent discard would be risky because install-complete auto-open makes the modal part of the happy path.
- Reopening review for the same install result should reuse the current draft session until the user discards or a new install run supersedes it.

### System Constraints

- Performance constraints
- Opening the modal must not trigger a backend fetch; it should be seeded from already available frontend install state.
- Save is the only required network or IPC boundary for the initial implementation.
- Reusing extracted field groups is preferable to mounting the entire existing `ProfileEditorView`, which would pull in tab chrome and unrelated profile-loading logic.

- Layout and viewport constraints
- The modal should be wide enough to show most two-column profile sections without collapsing immediately on desktop.
- Recommended CSS envelope:
- width: `min(1120px, calc(100vw - 32px))`
- max-height: `min(90vh, 860px)`
- body overflow: `auto`
- header and footer: `position: sticky` inside the dialog container or non-scrolling flex regions
- On narrower widths, existing two-column groups should collapse to one column using the same responsive behavior already used elsewhere in the theme.
- The page backdrop should remain fixed while only the modal body scrolls.

- Theme and UX constraints
- Reuse theme tokens from [variables.css](/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/styles/variables.css) and the current install/profile surface language from [theme.css](/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/styles/theme.css).
- The modal should look like an elevated CrossHook panel, not a browser-default dialog.
- Keep the primary action singular and explicit: `Save Profile` or `Confirm and Save`.
- Provide a secondary `Close` or `Return to Install` action and a tertiary `Open in Profile Tab` only if product wants a post-save deep-edit path.

- Accessibility and compatibility constraints
- The modal must use `role="dialog"` and `aria-modal="true"`.
- Focus should move into the modal on open, trap within it while open, close on `Escape`, and restore to the triggering verify button when closed.
- The current app uses `useGamepadNav` at the root level in [App.tsx](/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/App.tsx). The modal implementation must not leave background controls reachable by keyboard or controller while the modal is active.
- Steam Deck compatibility matters. The design should assume 1280x800 as a first-class target and verify that the footer actions remain visible while the body scrolls.

- Security and state consistency constraints
- Treat all profile paths as plain text user input. Do not execute or resolve filesystem paths merely because the modal is opened.
- Continue using Tauri dialog pickers for browse actions instead of custom path parsing.
- Closing the modal must not call `reset()` on `useInstallGame`.
- Opening the modal must not mutate the global profile editor draft unless save succeeds or the user explicitly chooses to open the saved profile in the Profile tab.

### File-Level Impact Preview

- Likely files to create
- [ProfileReviewModal.tsx](/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/components/ProfileReviewModal.tsx)
- [ProfileFormSections.tsx](/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/components/ProfileFormSections.tsx) or a similarly named shared field-group component
- [install-review.ts](/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/types/install-review.ts) if the team prefers keeping the modal payload/session types out of `install.ts`

- Likely files to modify
- [InstallGamePanel.tsx](/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/components/InstallGamePanel.tsx)
- [ProfileEditor.tsx](/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/components/ProfileEditor.tsx)
- [useProfile.ts](/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/hooks/useProfile.ts)
- [theme.css](/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/styles/theme.css)
- [install.ts](/home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/types/install.ts) only if the team decides the review payload types belong beside install types

- Likely files with no required change
- Rust backend install commands and profile persistence crates, because the existing install result already includes the generated profile and executable candidates needed by the modal.
