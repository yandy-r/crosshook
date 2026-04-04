# Technical research: game-details-modal

## Executive Summary

The modal can be implemented in the existing React/Tauri architecture by composing already-available profile summaries, health/offline context, and existing actions. The recommended approach is a Library-scoped modal state with read-only section rendering and quick actions that reuse current handlers. Initial scope requires no new persistence and likely no new backend commands if current IPC coverage is sufficient.

## Architecture Approach

- Add a modal component in the Library surface that receives selected profile summary data and derived detail state.
- Trigger modal from card-body click while preserving footer button event isolation.
- Reuse existing hooks/contexts for data hydration (summaries, health/offline snapshots, cover art, and cached external lookups).
- Keep quick actions bound to current flows (launch/edit/favorite/export/copy options as finalized) to avoid duplicated business logic.
- Prefer lazy/detail fetch on modal open with robust loading/error states over eager full-grid hydration.

### Data Model Implications

- Initial version: no new SQLite tables, no migration, no new TOML settings keys.
- Data classification:
  - User-editable preferences: unchanged.
  - Operational/history/cache metadata: read existing metadata only.
  - Runtime state: modal open/close, selected profile for inspection, in-flight requests.
- If future enhancement adds persisted UI preferences (for example tab memory), classify and scope separately.

## API Design Considerations

- Reuse existing Tauri commands and frontend invoke wrappers where available.
- Keep command naming `snake_case` and Serde-backed payload contracts for any new IPC additions (only if gaps are found).
- Compose a view model in frontend from multiple data sources:
  - Library summary baseline (name, cover, favorite, Steam app id).
  - Health/offline/activity/organization snapshots and counters.
  - Optional ProtonDB cached/live status.
- Standardize per-section state contract: `loading | ready | unavailable | error`.

## System Constraints

- Modal must follow existing dialog/focus conventions in this codebase, including keyboard dismissal and backdrop behavior.
- Scroll containers inside modal must align with existing scroll-enhancement selector conventions to avoid dual-scroll issues.
- Responsive layout should support narrow desktop widths and Steam Deck-like dimensions.
- Avoid unnecessary full profile loads on open unless explicitly required by UX decisions.
- Keep network-dependent content non-blocking and degrade gracefully when unavailable.

## File-Level Impact Preview

Likely files to create:

- `src/crosshook-native/src/components/library/GameDetailsModal.tsx`
- `src/crosshook-native/src/components/library/GameDetailsModal.css` (or integrated section in existing library styles)

Likely files to modify:

- `src/crosshook-native/src/components/library/LibraryCard.tsx` (card-body click wiring)
- `src/crosshook-native/src/components/pages/LibraryPage.tsx` (modal state orchestration and action wiring)
- `src/crosshook-native/src/components/library/LibraryGrid.tsx` (prop threading if needed)
- `src/crosshook-native/src/hooks/useScrollEnhance.ts` (only if new scroll container selectors are introduced)
- `src/crosshook-native/src/types/library.ts` (only if modal requires additional summary fields not currently exposed)
