# Game details modal — implementation handoff

## Execution order (completed in repo)

1. Modal shell + `crosshook-modal` semantics (`GameDetailsModal.tsx`, `GameDetailsModal.css`, `theme.css` surface width).
2. Library wiring: `useGameDetailsModalState`, `LibraryPage`, `LibraryGrid`, `LibraryCard`, `library.css` hitbox stacking.
3. Data: `useGameDetailsProfile`, `useGameDetailsRequestGuards`, `GameDetailsMetadataSection`, compatibility/health sections.
4. Actions: `game-details-actions.ts` (close-then-navigate for Launch/Edit).
5. Verification artifacts: `manual-checklist.md`, `verification-results.md`, this file.

## Ownership suggestions

- **Library UX / interactions**: `LibraryCard.tsx`, `library.css`, `LibraryGrid.tsx`, `LibraryPage.tsx`.
- **Modal content**: `GameDetailsModal.tsx` and `components/library/GameDetails*Section.tsx`.
- **Async safety**: `useGameDetailsProfile.ts`, `useGameDetailsRequestGuards.ts`.

## Do not change without an explicit issue (v1 constraints)

- **No new persistence**: No TOML settings keys, no SQLite migrations for this feature.
- **Reuse-first IPC**: Do not add aggregation commands unless a field is proven missing from existing `profile_load`, `fetch_game_metadata`, `fetch_game_cover_art`, `protondb_lookup`, health/offline commands.
- **Safe text**: Keep Steam/Proton/community strings as plain text; no HTML rendering paths.
- **Scroll contract**: New inner scrollers must be registered in `useScrollEnhance` `SCROLLABLE`; this implementation scrolls only `.crosshook-modal__body`.

## Quick pointers

- Gamepad back: close controls use `data-crosshook-modal-close` and `data-crosshook-focus-root="modal"` consistent with `ProfileReviewModal`.
- Card footer buttons keep `stopPropagation` so footer actions stay isolated from the details hitbox.
