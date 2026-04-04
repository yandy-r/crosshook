# Pattern Research: game-details-modal

## Architectural Patterns

**Page-Orchestrated Modal State**: Container pages keep selection/open state and pass intent callbacks into presentational components. Example flow: `components/pages/LibraryPage.tsx` -> `components/library/LibraryGrid.tsx` -> `components/library/LibraryCard.tsx`.

**Existing Modal Shell Reuse**: Modals use `crosshook-modal` classes, dialog semantics, and explicit close affordances. Reference implementations: `components/ProfileReviewModal.tsx`, `components/ProfilePreviewModal.tsx`, `components/OfflineTrainerInfoModal.tsx`.

**Hook-Wrapped IPC Access**: Frontend wraps command invocations in hooks with local loading/error state. Examples: `hooks/useLibrarySummaries.ts`, `hooks/useGameMetadata.ts`, `hooks/useGameCoverArt.ts`, `hooks/useProtonDbLookup.ts`.

## Code Conventions

- React components use `PascalCase`; hooks use `camelCase` with `use*`.
- Tauri commands are `snake_case` and registered in `src-tauri/src/lib.rs`.
- CSS uses `crosshook-*` BEM-like class naming; modal classes are centralized in `src/crosshook-native/src/styles/theme.css`.
- IPC boundary types use Serde camelCase conversion in Rust command DTOs (for example, `src-tauri/src/commands/profile.rs`).

## Error Handling

- Frontend hooks use `try/catch/finally`, set explicit error/loading state, and log context (`useLibrarySummaries.ts`).
- Async request guards are used where needed to avoid stale writes when parameters change (`useGameMetadata.ts` request-id pattern).
- Rust commands return `Result<_, String>` and delegate to core services; command layer remains thin.

## Testing Approach

- No dedicated frontend test framework is configured; UI changes are validated via dev/build scripts and manual verification.
- If backend/core behavior changes, run `cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core`.
- For modal work, manual checks should cover focus trapping, keyboard/gamepad close actions, backdrop behavior, and scroll behavior.

## Patterns to Follow

- Reuse modal accessibility conventions: `role="dialog"`, `aria-modal="true"`, `data-crosshook-focus-root="modal"`, and close controls marked with `data-crosshook-modal-close`.
- Keep feature state local to library page scope first; avoid premature global stores.
- Reuse existing hooks/commands before adding new IPC surface.
- Keep modal body as the primary scroll region (`.crosshook-modal__body`) and only add new scroll containers with `useScrollEnhance` updates.
