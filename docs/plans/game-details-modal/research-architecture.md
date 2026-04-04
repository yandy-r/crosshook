# Architecture Research: game-details-modal

## System Overview

CrossHook Native is a Tauri v2 desktop shell with a React 18 + TypeScript frontend in `src/crosshook-native/src/` and thin Rust IPC handlers in `src-tauri` backed by `crosshook-core`. The library surface is profile-centric: `LibraryPage` renders profile cards and already consumes summary data and art/metadata hooks. A game details modal should plug into this existing library composition path without adding new routing.

## Relevant Components

- `src/crosshook-native/src/App.tsx`: top-level route shell and gamepad-back modal close behavior.
- `src/crosshook-native/src/components/layout/ContentArea.tsx`: route-to-page mapping for `library`.
- `src/crosshook-native/src/components/pages/LibraryPage.tsx`: library orchestration and best owner for modal open/close state.
- `src/crosshook-native/src/components/library/LibraryGrid.tsx`: renders card collection; likely receives details-open callback.
- `src/crosshook-native/src/components/library/LibraryCard.tsx`: clickable card surface and art display.
- `src/crosshook-native/src/components/ProfileReviewModal.tsx`: full modal pattern with portal, focus management, and inert siblings.
- `src/crosshook-native/src/components/OfflineTrainerInfoModal.tsx`: lighter modal pattern for simpler dialog flows.
- `src/crosshook-native/src/hooks/useLibrarySummaries.ts`: library list data from IPC summaries.
- `src/crosshook-native/src/hooks/useGameMetadata.ts`: Steam metadata fetch state machine.
- `src/crosshook-native/src/hooks/useGameCoverArt.ts`: cover art resolution + caching path conversion.
- `src/crosshook-native/src/hooks/useScrollEnhance.ts`: enhanced scrolling target selector; impacts modal inner scroll behavior.
- `src/crosshook-native/src-tauri/src/lib.rs`: source of truth for registered Tauri command names.

## Data Flow

Library rendering starts in `LibraryPage`, which loads profile summaries via `useLibrarySummaries` (`invoke("profile_list_summaries")`). `LibraryCard` consumes summary data and art through `useGameCoverArt` (`invoke("fetch_game_cover_art")`), while metadata-rich UI can use `useGameMetadata` (`invoke("fetch_game_metadata")`) and optional `useProtonDbLookup` (`invoke("protondb_lookup")`). A modal should consume the selected profile summary and fan out to these existing hooks, preserving existing command contracts and avoiding redundant backend endpoints.

## Integration Points

The clean insertion point is in `LibraryPage`: add selected-profile modal state and pass a details trigger into `LibraryGrid`/`LibraryCard`. Implement modal UI using existing `crosshook-modal` structure and `data-crosshook-focus-root="modal"` close semantics to stay compatible with `handleGamepadBack()` in `App.tsx`. If modal body creates additional nested `overflow-y: auto` regions, update the `SCROLLABLE` selector in `useScrollEnhance.ts`.

## Key Dependencies

- `@tauri-apps/api/core` for IPC `invoke`.
- `@tauri-apps/api/event` for event-based refresh in surrounding profile/health hooks.
- Existing frontend type models under `src/crosshook-native/src/types/` (notably library and metadata shapes).
- Rust command modules in `src/crosshook-native/src-tauri/src/commands/` and core service logic in `src/crosshook-native/crates/crosshook-core/src/`.
