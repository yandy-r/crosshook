# PR Review #457 — feat(native): add persistent library inspector rail

**Reviewed**: 2026-04-22T22:59:12-04:00
**Mode**: PR
**Author**: yandy-r
**Branch**: feat/unified-desktop-phase-4-library-inspector → main
**Decision**: REQUEST CHANGES

## Summary

The implementation covers the broad Phase 4 surface and most validation passes, but the review found user-visible completeness/correctness gaps in launch-history continuity and list-view inspector selection. The targeted smoke test passed, though Vite surfaced a React maximum update-depth warning from `LibraryPage`, so the inspector selection sync still needs a closer follow-up.

## Findings

### CRITICAL

None.

### HIGH

- **[F001]** `src/crosshook-native/crates/crosshook-core/src/metadata/launch_queries.rs:365` — Launch history is queried only by `launch_operations.profile_name`, so launches recorded before a profile rename disappear from the inspector after the rename even though `record_launch_started` also persists the stable `profile_id` and profile renames preserve that id.
  - **Status**: Fixed
  - **Category**: Correctness
  - **Suggested fix**: Resolve the current `profile_id` for the requested profile name and query by `profile_id`, with a legacy fallback for rows where `profile_id IS NULL AND profile_name = ?`; add a regression test that records a launch, renames the profile, then lists history under the new name.

- **[F002]** `src/crosshook-native/src/components/pages/LibraryPage.tsx:315` — List view does not participate in inspector selection: grid view passes `onSelect` and `inspectorPickName`, but list view still passes `selectedProfile` and `LibraryListRow` only opens the details modal, so users in list mode cannot populate the persistent inspector from rows.
  - **Status**: Fixed
  - **Category**: Completeness
  - **Suggested fix**: Add `onSelect` support to `LibraryList` and `LibraryListRow`, pass `selectedName={inspectorPickName ?? undefined}` and `onSelect={handleCardSelect}` from `LibraryPage`, and add a list-view inspector-selection test.

### MEDIUM

- **[F003]** `src/crosshook-native/src/components/pages/LibraryPage.tsx:237` — The inspector selection de-dupe compares only `name`, `isFavorite`, `gameName`, and `steamAppId`, but `GameInspector` renders other `LibraryCardData` fields such as `networkIsolation`; those changes can leave the inspector rail showing stale data after a summary refresh.
  - **Status**: Fixed
  - **Category**: Correctness
  - **Suggested fix**: Remove the manual partial equality and publish a fully up-to-date selected object, or centralize a typed equality helper that compares every `LibraryCardData` field used by inspector content.

- **[F004]** `src/crosshook-native/src/components/layout/AppShell.tsx:225` — The inspector panel renders on every desktop route, while only the library route has an inspector component; non-library routes lose 280-360px of width to a rail that only says “No inspector content for this route.”
  - **Status**: Fixed
  - **Category**: Completeness
  - **Suggested fix**: Render the panel only when `ROUTE_METADATA[route].inspectorComponent` is present, or make non-opt-in routes allocate no inspector width while preserving deck-width suppression.

- **[F005]** `src/crosshook-native/src/components/pages/LibraryPage.tsx:87` — The toolbar exposes sort/filter states that are not implemented: `recent`, `lastPlayed`, and `playtime` leave ordering unchanged, and `recentlyLaunched` replaces the library with a “not available yet” placeholder even though this PR adds launch-history IPC.
  - **Status**: Fixed
  - **Category**: Completeness
  - **Suggested fix**: Either implement the advertised states with real data-backed ordering/filtering, or hide/disable unsupported chips until their backing data contract exists.

- **[F006]** `src/crosshook-native/src/components/library/GameInspector.tsx:209` — `GameInspector` owns stateful IPC fetching directly via `callCommand`, which bypasses the repo convention to wrap stateful frontend IPC in hooks and makes retry/loading/error behavior harder to reuse as inspector content grows.
  - **Status**: Fixed
  - **Category**: Pattern Compliance
  - **Suggested fix**: Extract a `useLaunchHistoryForProfile(profileName, limit)` hook under `src/hooks/` or the library feature folder, keep `callCommand('list_launch_history_for_profile')` there, and have `GameInspector` render the hook state.

### LOW

None.

## Validation Results

| Check      | Result                                                                                                                                                                                                       |
| ---------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| Type check | Pass (`npm run typecheck`)                                                                                                                                                                                   |
| Lint       | Pass (`npm run lint`)                                                                                                                                                                                        |
| Tests      | Pass (`npm test`; `cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core -- --test-threads=1`; first default-thread Rust run hit an unrelated `HOME` env race, targeted rerun passed) |
| Build      | Pass (`npm run build`)                                                                                                                                                                                       |
| Smoke      | Pass with warning (`npx playwright test tests/smoke.spec.ts --grep "library inspector"` passed 2 tests, but Vite logged a React “Maximum update depth exceeded” warning from `LibraryPage`)                  |

## Files Reviewed

- `docs/prps/plans/completed/unified-desktop-phase-4-library-inspector.plan.md` (Modified)
- `docs/prps/reports/unified-desktop-phase-4-library-inspector-report.md` (Added)
- `src/crosshook-native/crates/crosshook-core/src/metadata/launch_queries.rs` (Modified)
- `src/crosshook-native/crates/crosshook-core/src/metadata/launch_queries_tests.rs` (Modified)
- `src/crosshook-native/crates/crosshook-core/src/metadata/mod.rs` (Modified)
- `src/crosshook-native/crates/crosshook-core/src/metadata/models.rs` (Modified)
- `src/crosshook-native/src-tauri/src/commands/launch/mod.rs` (Modified)
- `src/crosshook-native/src-tauri/src/commands/launch/queries.rs` (Modified)
- `src/crosshook-native/src-tauri/src/lib.rs` (Modified)
- `src/crosshook-native/src/App.tsx` (Modified)
- `src/crosshook-native/src/components/layout/AppShell.tsx` (Modified)
- `src/crosshook-native/src/components/layout/Inspector.tsx` (Added)
- `src/crosshook-native/src/components/layout/Sidebar.tsx` (Modified)
- `src/crosshook-native/src/components/layout/__tests__/AppShell.test.tsx` (Modified)
- `src/crosshook-native/src/components/layout/__tests__/Inspector.test.tsx` (Added)
- `src/crosshook-native/src/components/layout/inspectorVariants.ts` (Added)
- `src/crosshook-native/src/components/layout/routeMetadata.ts` (Modified)
- `src/crosshook-native/src/components/library/GameInspector.tsx` (Added)
- `src/crosshook-native/src/components/library/LibraryCard.tsx` (Modified)
- `src/crosshook-native/src/components/library/LibraryGrid.tsx` (Modified)
- `src/crosshook-native/src/components/library/LibraryToolbar.tsx` (Modified)
- `src/crosshook-native/src/components/library/__tests__/GameInspector.test.tsx` (Added)
- `src/crosshook-native/src/components/library/__tests__/LibraryCard.test.tsx` (Modified)
- `src/crosshook-native/src/components/library/__tests__/LibraryGrid.test.tsx` (Modified)
- `src/crosshook-native/src/components/library/__tests__/LibraryToolbar.test.tsx` (Added)
- `src/crosshook-native/src/components/pages/LibraryPage.tsx` (Modified)
- `src/crosshook-native/src/components/pages/__tests__/LibraryPage.test.tsx` (Added)
- `src/crosshook-native/src/context/InspectorSelectionContext.tsx` (Added)
- `src/crosshook-native/src/hooks/useAccessibilityEnhancements.ts` (Modified)
- `src/crosshook-native/src/hooks/useScrollEnhance.ts` (Modified)
- `src/crosshook-native/src/lib/__tests__/runtime.test.ts` (Modified)
- `src/crosshook-native/src/lib/mocks/handlers/launch.ts` (Modified)
- `src/crosshook-native/src/styles/layout.css` (Modified)
- `src/crosshook-native/src/styles/library.css` (Modified)
- `src/crosshook-native/src/styles/sidebar.css` (Modified)
- `src/crosshook-native/src/test/__tests__/breakpoint.test.ts` (Added)
- `src/crosshook-native/src/test/breakpoint.ts` (Added)
- `src/crosshook-native/src/test/fixtures.ts` (Modified)
- `src/crosshook-native/src/types/library.ts` (Modified)
- `src/crosshook-native/tests/smoke.spec.ts` (Modified)
