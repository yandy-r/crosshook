# Implementation Report: Overview Panel Deep-Link Buttons

## Summary

Implemented GitHub issue #472 / Unified Desktop Hero Detail Phase 7. The Hero Detail Overview tab now renders Runtime, Active profile, Launch command, and Trainer hook action cards above the existing Store metadata and Health/offline readiness panels. The cards use the in-memory Hero Detail tab state to jump to Profiles or Launch options without URL, route, backend, TOML, or SQLite changes.

## Tasks Completed

- Added a typed runtime-only profiles scroll target to the Hero Detail model.
- Wired `GameDetail` to honor panel-originated tab requests and clear scroll targets after consumption.
- Added Overview action cards with disabled no-callback fallback behavior.
- Added a Runtime-section ref through `HeroDetailProfilesTab` and `HeroProfileEditorSections`.
- Added unit coverage for button-to-tab mapping, shell state threading, and runtime scroll consumption.
- Added browser smoke coverage for Overview `Edit launch config` -> Launch options.

## Files Changed

- `src/crosshook-native/src/components/library/hero-detail-model.ts`
- `src/crosshook-native/src/components/library/GameDetail.tsx`
- `src/crosshook-native/src/components/library/HeroDetailPanels.tsx`
- `src/crosshook-native/src/components/library/HeroDetailProfilesTab.tsx`
- `src/crosshook-native/src/components/library/profiles/HeroProfileEditorSections.tsx`
- `src/crosshook-native/src/styles/hero-detail.css`
- `src/crosshook-native/src/components/library/__tests__/GameDetail.test.tsx`
- `src/crosshook-native/src/components/library/__tests__/HeroDetailPanels.test.tsx`
- `src/crosshook-native/src/components/library/__tests__/HeroDetailProfilesTab.test.tsx`
- `src/crosshook-native/tests/smoke.spec.ts`

## Validation

- `npm exec vitest run src/components/library/__tests__/HeroDetailPanels.test.tsx src/components/library/__tests__/GameDetail.test.tsx src/components/library/__tests__/HeroDetailProfilesTab.test.tsx` - PASS
- `npm run typecheck` - PASS
- `./scripts/lint.sh --modified --ts` - PASS
- `npm run test:smoke -- --grep "overview deep-link"` - PASS
- `npm test` - PASS, 54 files / 438 tests
- `npm run build` - PASS
- `git diff -- src/crosshook-native/crates/crosshook-core/src/metadata/migrations.rs` - PASS, empty diff

## Persistence Boundary

No persisted data was added or changed. `activeTab` and `profilesScrollTarget` are runtime-only React state. Existing TOML-backed profile fields, launch preview data, metadata, and health/offline readiness are read only for Overview display.

## Deviations

- The plan allowed disabled-or-omitted behavior when `onSetActiveTab` is absent. The implementation renders disabled buttons so direct `HeroDetailPanels` renders keep the same visual structure while remaining non-interactive.
- The Runtime action scroll target is consumed immediately after `scrollIntoView` is requested; this prevents repeat scrolling on later profile-tab re-renders.

## Next Steps

Phase 7 is complete. Remaining PRD phases can continue with route cleanup and navigation rewiring once dependent phases are ready.
