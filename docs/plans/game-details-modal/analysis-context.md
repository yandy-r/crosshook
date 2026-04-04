# Context Analysis: game-details-modal

## Executive Summary

The feature adds a read-only game details modal in the library flow so users can inspect game/profile context without route changes. Implementation should be frontend-first by composing existing hooks and command contracts, with no new persistence for v1. `LibraryPage` should orchestrate modal state and selection updates, while modal presentation follows existing `crosshook-modal` accessibility and focus conventions.

## Architecture Context

- **System Structure**: Tauri v2 shell + React/TypeScript frontend, thin `src-tauri` command layer, business logic in `crosshook-core`.
- **Data Flow**: Card interaction in library triggers modal state in `LibraryPage`; modal loads summary/profile/metadata/art/proton data via existing hooks.
- **Integration Points**: `LibraryPage`, `LibraryGrid`, `LibraryCard`, new `GameDetailsModal`, and existing command registrations in `src-tauri/src/lib.rs`.

## Critical Files Reference

- `/src/crosshook-native/src/components/pages/LibraryPage.tsx`: orchestrates library behavior and should own modal state.
- `/src/crosshook-native/src/components/library/LibraryCard.tsx`: card body/footer click semantics must be split safely.
- `/src/crosshook-native/src/components/library/LibraryGrid.tsx`: callback propagation for details-open behavior.
- `/src/crosshook-native/src/components/ProfileReviewModal.tsx`: strongest modal lifecycle/focus reference.
- `/src/crosshook-native/src/styles/theme.css`: shared modal class contract.
- `/src/crosshook-native/src/hooks/useLibrarySummaries.ts`: summary-loading and hook error pattern.
- `/src/crosshook-native/src/hooks/useGameMetadata.ts`: stale-request guard pattern for async metadata.
- `/src/crosshook-native/src/hooks/useGameCoverArt.ts`: art resolution and local asset conversion.
- `/src/crosshook-native/src/hooks/useScrollEnhance.ts`: scroll target contract for modal body/inner scrollers.
- `/src/crosshook-native/src-tauri/src/lib.rs`: command registration and naming integrity.

## Patterns to Follow

- **Page-Orchestrated State**: Keep modal open/selected state in `LibraryPage` and pass explicit callbacks down.
- **Hook-Wrapped IPC**: Reuse existing hooks and command strings rather than adding v1 aggregation IPC.
- **Modal A11y Contract**: Reuse `crosshook-modal` structure, close controls, and focus-root markers.
- **Thin Command Layer**: If backend expansion becomes necessary, keep new `#[tauri::command]` wrappers thin and `snake_case`.

## Cross-Cutting Concerns

- Card body open behavior must not break footer action semantics (`Launch`, `Favorite`, `Edit`).
- Rapid profile switching requires stale-response protection in modal async paths.
- Offline/degraded states need explicit UI labels for unavailable vs cached data.
- Nested modal scroll regions can trigger dual-scroll issues unless aligned with `useScrollEnhance`.

## Security Constraints

- Render remote metadata strings as safe text (no raw HTML rendering paths).
- Avoid new generic URL/path IPC input surfaces; preserve normalized fixed-host patterns.
- Keep capability/CSP boundaries tight when adding external link affordances or remote media.

## Reuse Opportunities

- `/src/crosshook-native/src/components/ProfileReviewModal.tsx`: modal portal/focus semantics.
- `/src/crosshook-native/src/hooks/useGameMetadata.ts`: async request sequencing and cancellation pattern.
- `/src/crosshook-native/src/hooks/useGameCoverArt.ts`: existing art pipeline.
- `/src/crosshook-native/src/components/pages/LibraryPage.tsx`: existing handlers to delegate quick actions.

## Parallelization Opportunities

- Modal shell implementation and library callback wiring can proceed in parallel once modal props are agreed.
- Enrichment sections (metadata/proton/health/offline) can be split into independent follow-up tasks.
- UX polish (responsive/focus/scroll) can run after functional shell lands.

## Implementation Constraints

- No v1 TOML or SQLite schema changes.
- No frontend test framework is configured; plan for manual UI validation steps.
- Keep command naming/typing alignment across frontend invoke strings and Rust command registration.

## Key Recommendations

- Land modal shell + entrypoint wiring first, then progressively hydrate sections.
- Preserve event isolation between card body and footer controls from the start.
- Treat scroll/focus behavior as a first-class acceptance area, not post-hoc polish.
