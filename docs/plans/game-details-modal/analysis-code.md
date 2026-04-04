# Code Analysis: game-details-modal

## Executive Summary

Current library architecture already has the required data/hook surfaces for a details modal, but lacks a dedicated modal component and card-level details affordance. The best fit is a new `GameDetailsModal` component composed from existing modal shell patterns and hook contracts. Changes should remain mostly frontend and localized to library components.

## Existing Code Structure

### Related Components

- `/src/crosshook-native/src/components/pages/LibraryPage.tsx`: parent page with library data/actions and route navigation hooks.
- `/src/crosshook-native/src/components/library/LibraryGrid.tsx`: simple mapper from summaries to cards.
- `/src/crosshook-native/src/components/library/LibraryCard.tsx`: primary interaction point for opening details.
- `/src/crosshook-native/src/components/ProfileReviewModal.tsx`: richest reusable modal implementation details.
- `/src/crosshook-native/src/components/ProfilePreviewModal.tsx`: additional modal portal/focus pattern.

### File Organization Pattern

Library UI is organized by page container (`components/pages`) plus feature components (`components/library`). Data fetching is encapsulated in hooks under `src/hooks`, while Tauri contracts are centralized in `src-tauri/src/commands` and registered in `src-tauri/src/lib.rs`.

## Implementation Patterns

### Pattern: Route/Page Owns Intent State

**Description**: Parent page stores interaction state and delegates event callbacks through child components.
**Example**: `/src/crosshook-native/src/components/pages/LibraryPage.tsx`
**Apply to**: Modal `open/close/selectedProfile` state and callback wiring.

### Pattern: Hook-Based IPC Loading

**Description**: Hooks wrap `invoke()` with explicit loading/error semantics and stale-request protection.
**Example**: `/src/crosshook-native/src/hooks/useGameMetadata.ts`
**Apply to**: Modal section hydration and rapid profile switching behavior.

### Pattern: Shared Modal Contract

**Description**: Portal + `crosshook-modal` classes + focus root/close attributes ensure consistent behavior.
**Example**: `/src/crosshook-native/src/components/ProfileReviewModal.tsx`
**Apply to**: `GameDetailsModal` shell, close semantics, and keyboard/gamepad compatibility.

## Integration Points

### Files to Create

- `/src/crosshook-native/src/components/library/GameDetailsModal.tsx`: new modal component for details rendering.
- `/src/crosshook-native/src/components/library/GameDetailsModal.css`: modal-specific layout/styling additions (if not fully in theme styles).

### Files to Modify

- `/src/crosshook-native/src/components/pages/LibraryPage.tsx`: modal state, render path, and quick-action delegation.
- `/src/crosshook-native/src/components/library/LibraryGrid.tsx`: callback threading for details-open behavior.
- `/src/crosshook-native/src/components/library/LibraryCard.tsx`: body/footer interaction split and details trigger.
- `/src/crosshook-native/src/hooks/useScrollEnhance.ts`: only if new inner scroll containers are introduced.

## Code Conventions

### Naming

Use `PascalCase` for new component files (`GameDetailsModal.tsx`) and `camelCase` for handlers/hooks (`handleOpenDetails`).

### Error Handling

Prefer section-scoped loading/error states with concise failure messaging; follow hook patterns with `try/catch/finally` and stale-request guards.

### Testing

No frontend test framework is configured; include explicit manual validation checklist in plan tasks. Run core Rust tests only if backend/core changes occur.

## Dependencies and Services

### Available Utilities

- `useLibrarySummaries`: baseline profile summary data.
- `useGameMetadata`: Steam metadata and request sequencing.
- `useGameCoverArt`: image resolution and local asset conversion.
- `useProtonDbLookup`: optional compatibility enrichment.
- `ProfileReviewModal` pattern: trusted modal shell/accessibility behavior.

### Required Dependencies

- Existing project dependencies only (`react`, `@tauri-apps/api/core`); no new packages required for v1.

## Gotchas and Warnings

- `LibraryCard` currently uses whole-card click to select profile; details-open behavior must not regress footer actions.
- Gamepad back logic depends on modal close marker attributes; missing attributes break close behavior.
- Additional inner scrollers can cause dual-scroll jank if not registered in `useScrollEnhance`.
- Remote text fields must remain safe text rendering.

## Reuse and Modularity Guidance

- **Reuse First**: existing modal shells and metadata/art hooks.
- **Keep Feature-Local**: modal-specific section composition/helpers until reuse is proven.
- **Build vs. Depend**: avoid introducing new modal/state libraries; existing patterns are sufficient.

## Task-Specific Guidance

- **For UI tasks**: prioritize event semantics and accessibility before section enrichment.
- **For IPC tasks**: only add new commands if required fields are unavailable from existing surfaces.
- **For persistence tasks**: none in v1; treat modal state as runtime-only.
