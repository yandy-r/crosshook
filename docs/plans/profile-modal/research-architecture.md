# Architecture Research: profile-modal

## System Overview

The main UI is organized around a single app-level `useProfile` state in `App.tsx`, with `ProfileEditorView` rendering either the normal profile editor or the install flow as a local subtab. The install flow is fully frontend-driven through `useInstallGame`, which validates inputs, calls Tauri install commands, derives a reviewable `GameProfile`, and currently hands that draft into the global profile editor instead of keeping review state local. A `profile-modal` implementation fits best by leaving install execution in `InstallGamePanel`, moving review-session ownership into `ProfileEditorView`, and reusing `useProfile` only for final persistence.

## Relevant Components

- /home/yandy/Projects/github.com/yandy-r/crosshook/docs/plans/profile-modal/feature-spec.md: Source requirements for modal ownership, save behavior, visible sections, and type placement.
- /home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/App.tsx: App-level composition point; owns `useProfile`, `useGamepadNav`, the main tab state, and the `profileEditorTab` signal that affects `LaunchPanel` and `LauncherExport`.
- /home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/components/ProfileEditor.tsx: Current bridge between install and profile flows; owns the local `editorTab`, the existing full profile form, and the current install handoff via `handleInstallReview`.
- /home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/components/InstallGamePanel.tsx: Install-flow container; owns `useInstallGame`, renders install inputs, final executable review, candidate selection, preview summaries, and the manual review action.
- /home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/hooks/useInstallGame.ts: Source of truth for install request state, stage transitions, backend install results, candidate options, default prefix resolution, and derived `reviewProfile`.
- /home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/hooks/useProfile.ts: Source of truth for saved profile selection and persistence; normalizes profiles for edit/save, validates save eligibility, invokes `profile_save`, syncs settings and recent files, refreshes the list, and reloads the saved profile.
- /home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/hooks/useGamepadNav.ts: Root-scoped keyboard/controller navigation and `Escape` handling; important because a portal modal can fall outside the current focus/navigation root.
- /home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/types/install.ts: Install transport types; the correct location for `InstallProfileReviewPayload`.
- /home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/types/profile.ts: Canonical `GameProfile` contract shared across install output, editor state, and persistence.
- /home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/styles/theme.css: Existing layout and responsive rules for install/profile surfaces that the modal should visually extend.
- /home/yandy/Projects/github.com/yandy-r/crosshook/src/crosshook-native/src/styles/focus.css: Existing focus-target styling for keyboard/controller navigation; modal content needs to remain inside this styling model.

## Data Flow

`InstallGamePanel` creates and updates an `InstallGameRequest` through `useInstallGame`. The hook resolves a default prefix path with `install_default_prefix_path`, validates the request through `validate_install_request`, then runs `install_game` and stores the returned `InstallGameResult`.

When `useInstallGame` receives a successful result, `setResult` copies `result.profile` into `reviewProfile`, copies the resolved executable and prefix back into request state, derives `candidateOptions` from `discovered_game_executable_candidates`, and sets the stage to `review_required` or `ready_to_save`. If the final executable changes, `setInstalledExecutablePath` updates both `request.installed_game_executable_path` and `reviewProfile.game.executable_path`, while also deriving `reviewProfile.runtime.working_directory` from the selected executable path.

Today the handoff is direct and destructive to the shared editor draft: `InstallGamePanel` calls `onReviewGeneratedProfile(profileName, reviewProfile)`, `ProfileEditorView.handleInstallReview` forwards that to `useProfile.hydrateProfile`, and then flips `editorTab` to `profile`. From that point onward the normal profile editor owns the draft, and `saveProfile()` validates `profile.game.executable_path`, normalizes the draft, invokes `profile_save`, syncs `settings_save` and `recent_files_save`, refreshes the profile list, and reloads the saved profile through `profile_load`.

For `profile-modal`, the clean seam is to stop rebinding global profile-editor state during review. `useInstallGame` should remain the source of install outputs, but `ProfileEditorView` should translate those outputs into a modal-local `ProfileReviewSession` based on `GameProfile`, candidate options, helper log path, and install status text. Only the final save should cross into `useProfile`, using a helper that persists an arbitrary `(name, profile)` pair without first overwriting the global editor draft.

## Integration Points

The first integration point is `InstallGamePanel.tsx`. Its current `onReviewGeneratedProfile(profileName, profile)` callback should become a payload-based modal opener that assembles data already present in the panel and hook: `result.profile_name`, `reviewProfile`, `candidateOptions`, `result.helper_log_path`, `result.message`, and a `source` discriminator for auto-open versus manual verify.

The second integration point is `ProfileEditor.tsx`. This component already sits at the boundary between the install subtab and the shared profile domain, so it is the right place to own `ProfileReviewSession`, modal open/close/discard state, and the explicit post-save transition back to the Profile tab with the saved profile selected. It is also the right place to extract reusable profile field sections out of the current inline editor markup so the modal and the full Profile tab share one editing surface.

The third integration point is `useProfile.ts`. `saveProfile()` is currently coupled to the hook’s internal `profileName` and `profile` state, but the normalization, validation, `profile_save`, metadata sync, refresh, and reload logic already exists there. A narrow `persistProfileDraft(name, profile)` helper should live beside `saveProfile()` so the modal can save through the same persistence pipeline without mutating the app-wide editor draft first.

The fourth integration point is `App.tsx` plus `useGamepadNav.ts`. `App.tsx` derives launch/export behavior from `profileEditorTab`, so leaving the editor on the install subtab while the modal is open preserves the current background context until save completion. The navigation constraint is more subtle: `useGamepadNav` only manages focus inside `rootRef`, so a portal mounted outside that subtree will not participate in controller navigation or `Escape` handling; the modal either needs to render inside the focus scope or the app needs an explicit focus-trap/root-switch strategy when the modal is open.

The fifth integration point is styling. `theme.css` already defines the install/profile shells, responsive single-column collapse, and panel styling used by the surrounding UI, while `focus.css` defines the controller-visible focus treatment. Modal shell, backdrop, sticky header/footer, and internal scrolling should be added in that same styling system so the review surface stays visually and behaviorally consistent with the current app.

## Key Dependencies

The implementation depends on React state/effect composition in the current frontend, plus `react-dom` portal rendering for the modal shell. It also depends on existing Tauri IPC commands exposed through `@tauri-apps/api/core`: `install_default_prefix_path`, `validate_install_request`, `install_game`, `profile_save`, `profile_list`, `profile_load`, `settings_load`, `settings_save`, `recent_files_load`, and `recent_files_save`.

For field interactions, the existing browse flows already rely on `@tauri-apps/plugin-dialog`, and the modal should reuse those same path pickers instead of adding new filesystem plumbing. Internally, the critical modules are `useInstallGame`, `useProfile`, `GameProfile`, `InstallGameExecutableCandidate`, `theme.css`, and `focus.css`; no backend crate changes are required for the architecture described by the current code.
