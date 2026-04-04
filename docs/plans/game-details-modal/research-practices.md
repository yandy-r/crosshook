# Practices Research: game-details-modal

## Executive Summary

This feature has strong reuse opportunities in existing modal shells, library data hooks, and metadata/art integration hooks. The most maintainable approach is to keep orchestration in `LibraryPage`, implement a focused `GameDetailsModal` component, and compose existing hooks/commands before considering new abstractions. Duplication risk exists in modal focus/portal logic, but a first delivery should prioritize a proven pattern clone over a broad modal framework refactor.

## Existing Reusable Code

| Module/Utility         | Location                                                     | Purpose                                   | How to Reuse for This Feature                                              |
| ---------------------- | ------------------------------------------------------------ | ----------------------------------------- | -------------------------------------------------------------------------- |
| Library summaries hook | `src/crosshook-native/src/hooks/useLibrarySummaries.ts`      | Loads card-ready profile summaries        | Use selected summary as modal seed state.                                  |
| Cover art hook         | `src/crosshook-native/src/hooks/useGameCoverArt.ts`          | Resolves local/remote game imagery        | Reuse for modal hero/art panel.                                            |
| Metadata hook          | `src/crosshook-native/src/hooks/useGameMetadata.ts`          | Steam metadata loading and stale handling | Reuse for description/genres/details sections.                             |
| ProtonDB hook          | `src/crosshook-native/src/hooks/useProtonDbLookup.ts`        | ProtonDB lookup + status                  | Reuse optional compatibility section.                                      |
| Full modal pattern     | `src/crosshook-native/src/components/ProfileReviewModal.tsx` | Portal, focus trap, close conventions     | Copy/compose modal behavior for consistent UX/a11y.                        |
| Modal CSS system       | `src/crosshook-native/src/styles/theme.css`                  | Shared `crosshook-modal` styles           | Apply existing classes instead of new design system.                       |
| Library orchestration  | `src/crosshook-native/src/components/pages/LibraryPage.tsx`  | Page-level library data + actions         | Own `open/selected` state for details modal.                               |
| Scroll enhancer        | `src/crosshook-native/src/hooks/useScrollEnhance.ts`         | Scroll behavior normalization             | Keep modal body as primary scroll surface; extend selector only if needed. |

## Modularity Findings

- Keep modal state and handlers in `LibraryPage.tsx` and pass explicit callbacks to `LibraryGrid`/`LibraryCard`.
- Keep `GameDetailsModal` mostly presentational, with minimal local state for section toggles/loading fallback.
- Prefer feature-local helper functions in modal files first; only extract shared modules after repeated reuse appears.
- Preserve thin `src-tauri` command wrappers and keep new backend logic, if any, in `crosshook-core`.

## KISS Assessment

- Use one proven modal implementation pattern (preferably `ProfileReviewModal` structure) rather than creating a new modal framework.
- Reuse existing command/hook surfaces instead of introducing a synthetic "get_game_details" aggregation endpoint for v1.
- Defer generalized modal abstraction work until at least one more modal needs the same extraction.

## Build vs. Depend Decisions

- **Use existing code**: `crosshook-modal` styles, library hooks, metadata/proton/art hooks, existing commands.
- **Extend existing modules**: `LibraryCard` and `LibraryPage` callback surfaces for modal open behavior.
- **Avoid new dependencies**: no additional modal/state libraries required for this scope.
- **Only add backend API** if needed fields are unavailable through current commands and hooks.

## Persistence and Usability

- **TOML settings**: no new modal-owned settings keys are introduced. The modal is read-only and continues to consume existing profile data loaded through `profile_load`; anything users can edit still belongs in the Profiles / Launch flows rather than `crosshook-modal`.
- **SQLite metadata**: no new tables or migrations are required for the current modal. `useGameMetadata`, `useProtonDbLookup`, health context data, and offline readiness already read persisted cache / metadata through existing hooks and commands, so the modal only reflects what those layers already store.
- **Runtime-only state**: `LibraryPage`, `LibraryCard`, `useGameDetailsModalState`, and `GameDetailsModal` own transient selection, open/close state, focus trapping, and in-flight section loading. This state must reset safely on close, reload, offline failures, and upgrades; it is not user-editable after the modal closes.
- **Migration and backward compatibility**: current implementation is a no-migration case. Upgrades must continue to tolerate missing cache entries, sparse `profile_load` payloads, and empty Steam App IDs by falling back to loading / unavailable copy instead of blocking the modal. Only a future feature that persists modal-specific preferences or introduces new cached detail data would require an explicit TOML or SQLite migration plan.
- **Offline fallback and degraded behavior**: when Steam metadata, ProtonDB, or offline-readiness data is unavailable, the modal should keep rendering via cached or unavailable states sourced from existing hooks instead of adding ad-hoc fallbacks. Manual checks should cover offline metadata / ProtonDB messaging, stale-cache messaging, rapid A-to-B switching without stale overwrite, and focus restoring to the invoking library control after close.
- **User visibility and editability**: users can see read-only detail aggregates from `LibraryPage` / `LibraryCard` and the library hooks, but they do not edit those values inside the modal. Any durable edits still happen through the existing profile editor, launch configuration, or settings surfaces; modal focus-locking and `crosshook-modal` restore behavior remain runtime-only UX state.

## Testability Notes

- Manual UI verification is primary (no configured frontend test framework).
- Keep logic in hook composition and deterministic props to simplify manual validation matrices.
- If backend/core changes are introduced, run `cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core`.
- Define explicit manual checks for keyboard/gamepad close, focus return, and offline/error section rendering.
