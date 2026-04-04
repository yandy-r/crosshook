# game-details-modal

The game details modal fits into the existing library page composition path, where `LibraryPage` owns selection state and `LibraryGrid`/`LibraryCard` trigger UI intents for specific profiles. Data should be assembled from existing hook-wrapped IPC calls (`profile_list_summaries`, `fetch_game_metadata`, `fetch_game_cover_art`, optional `protondb_lookup`) instead of introducing a new aggregation endpoint for v1. Modal behavior should reuse established `crosshook-modal` accessibility and focus patterns so keyboard/gamepad close semantics continue to work with the global shell in `App.tsx`. The implementation should remain frontend-first, preserving current persistence boundaries (no new settings or SQLite schema) and only extending backend command surfaces if proven required by missing fields.

## Relevant Files

- /src/crosshook-native/src/components/pages/LibraryPage.tsx: primary state owner for selected profile and modal open/close wiring.
- /src/crosshook-native/src/components/library/LibraryGrid.tsx: list/grid renderer where details callbacks are propagated.
- /src/crosshook-native/src/components/library/LibraryCard.tsx: card interaction surface likely extended with details affordance.
- /src/crosshook-native/src/components/ProfileReviewModal.tsx: comprehensive modal pattern for portal, focus trap, and close semantics.
- /src/crosshook-native/src/components/ProfilePreviewModal.tsx: additional modal lifecycle/focus reference.
- /src/crosshook-native/src/styles/theme.css: shared crosshook-modal classes and dialog styling contract.
- /src/crosshook-native/src/hooks/useLibrarySummaries.ts: profile summary fetch and error/loading pattern.
- /src/crosshook-native/src/hooks/useGameMetadata.ts: metadata loading pattern with stale-request protection.
- /src/crosshook-native/src/hooks/useGameCoverArt.ts: cover art resolution pattern using local asset conversion.
- /src/crosshook-native/src/hooks/useProtonDbLookup.ts: optional compatibility enrichment pattern.
- /src/crosshook-native/src/hooks/useScrollEnhance.ts: scrollable selector contract affecting new modal body containers.
- /src/crosshook-native/src-tauri/src/lib.rs: canonical command registration and naming alignment source.
- /src/crosshook-native/src-tauri/src/commands/game_metadata.rs: command layer for metadata and cover art lookups.
- /src/crosshook-native/src-tauri/src/commands/profile.rs: summary/load command contracts used by library and modal.

## Relevant Tables

- `external_cache_entries`: caches remote metadata/proton responses consumed by existing commands.
- `health_snapshots`: optional diagnostics details source if modal surfaces profile health.
- `offline_readiness_snapshots`: optional offline-readiness detail source.
- `version_snapshots`: optional version/correlation detail source for advanced sections.

## Relevant Patterns

**Page-Orchestrated UI State**: Route/page containers own interaction state and pass intent handlers into presentational children. See `src/crosshook-native/src/components/pages/LibraryPage.tsx`.

**Hook-Wrapped Tauri Invoke**: Frontend uses hooks to encapsulate `invoke()` calls with loading/error guards and stale-response handling. See `src/crosshook-native/src/hooks/useLibrarySummaries.ts` and `src/crosshook-native/src/hooks/useGameMetadata.ts`.

**Modal Accessibility Contract**: Dialogs use `crosshook-modal` classes, focus roots, and explicit close controls compatible with app-level gamepad back behavior. See `src/crosshook-native/src/components/ProfileReviewModal.tsx`.

**Thin Tauri Command Layer**: Command handlers remain small wrappers over core services with stable snake_case names and Serde boundary types. See `src/crosshook-native/src-tauri/src/commands/profile.rs`.

## Relevant Docs

**`docs/plans/game-details-modal/feature-spec.md`**: You _must_ read this when working on scope, acceptance criteria, and file-level implementation mapping.

**`docs/plans/game-details-modal/research-recommendations.md`**: You _must_ read this when working on rollout sequence, interaction design tradeoffs, and reuse-first guidance.

**`AGENTS.md`**: You _must_ read this when working on route layout contracts, modal scroll behavior, and backend/frontend architecture boundaries.

**`docs/plans/game-details-modal/research-technical.md`**: You _must_ read this when working on IPC integration points and no-new-persistence expectations.

## Security Considerations

- Remote/cached metadata must be rendered as safe text by default; do not introduce raw HTML rendering paths for Steam/Proton content.
- New IPC inputs must preserve fixed-host and normalized-input patterns; avoid generic URL/path fetch command surfaces.
- Keep CSP/capabilities strict when adding external links or media; prefer existing local-asset cover art flow over broad remote `img-src` changes.
- Preserve existing modal focus/close semantics (`data-crosshook-focus-root="modal"` and close controls) to avoid interaction ambiguity and broken navigation.

## Reuse Opportunities

- `src/crosshook-native/src/components/ProfileReviewModal.tsx`: extend/copy established modal lifecycle and focus logic rather than inventing a new shell.
- `src/crosshook-native/src/hooks/useGameMetadata.ts`: reuse metadata loading/cancellation behavior for modal sections.
- `src/crosshook-native/src/hooks/useGameCoverArt.ts`: reuse existing image resolution and local-asset conversion.
- `src/crosshook-native/src/components/pages/LibraryPage.tsx`: keep modal orchestration local and avoid premature global state abstractions.
- `src/crosshook-native/src/hooks/useScrollEnhance.ts`: align new scroll containers with existing selector-based enhancement instead of custom scroll hacks.
