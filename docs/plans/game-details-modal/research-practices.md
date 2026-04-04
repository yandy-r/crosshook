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

## Testability Notes

- Manual UI verification is primary (no configured frontend test framework).
- Keep logic in hook composition and deterministic props to simplify manual validation matrices.
- If backend/core changes are introduced, run `cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core`.
- Define explicit manual checks for keyboard/gamepad close, focus return, and offline/error section rendering.
