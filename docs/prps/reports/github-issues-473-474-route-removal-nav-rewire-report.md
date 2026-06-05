# Implementation Report: Route Removal and Navigation Rewire

## Summary

Implemented GitHub issues #473 and #474. The legacy `profiles` and `launch` routes are removed from app route metadata, route validation, sidebar navigation, command palette route commands, smoke route sweeps, and route-banner/inspector fixtures. Profile edit and launch entry points now open Library Hero Detail and select the `Profiles` or `Launch options` tab with runtime-only tab intent.

## Tasks Completed

- Removed `profiles` and `launch` from active route definitions and navigation metadata.
- Rewired AppShell command-palette profile commands, collection modal actions, install completion, and health remediation into Library Hero Detail tab intents.
- Added typed Hero Detail tab validation and `OpenGameDetailIntent.heroDetailTab`.
- Made `GameDetail` consume requested tab changes and made header actions switch tabs in-place.
- Removed collection defaults link-out copy and kept the inline editor as the sole edit surface.
- Updated unit, a11y, Vitest, and Playwright smoke coverage to assert Library Hero Detail behavior instead of legacy route commands.
- Kept legacy page files type-clean during transitional cleanup without exposing them as routes.

## Files Changed

- `src/crosshook-native/src/components/layout/AppShell.tsx`
- `src/crosshook-native/src/components/layout/ContentArea.tsx`
- `src/crosshook-native/src/components/layout/Sidebar.tsx`
- `src/crosshook-native/src/components/layout/routeMetadata.ts`
- `src/crosshook-native/src/lib/commands.ts`
- `src/crosshook-native/src/lib/validAppRoutes.ts`
- `src/crosshook-native/src/types/navigation.ts`
- `src/crosshook-native/src/types/profile.ts`
- `src/crosshook-native/src/components/library/GameDetail.tsx`
- `src/crosshook-native/src/components/library/HeroDetailHeader.tsx`
- `src/crosshook-native/src/components/library/hero-detail-model.ts`
- `src/crosshook-native/src/components/pages/LibraryPage.tsx`
- `src/crosshook-native/src/components/pages/HealthDashboardPage.tsx`
- `src/crosshook-native/src/components/pages/InstallPage.tsx`
- `src/crosshook-native/src/components/collections/CollectionLaunchDefaultsEditor.tsx`
- `src/crosshook-native/src/components/collections/CollectionViewModal.tsx`
- `src/crosshook-native/tests/navigation-helpers.ts`
- `src/crosshook-native/tests/pipeline.spec.ts`
- `src/crosshook-native/tests/smoke.spec.ts`
- Related unit/a11y test fixtures under `src/crosshook-native/src/**/__tests__/`

## Validation

- `npm run typecheck` - PASS
- `./scripts/lint.sh` - PASS
- `npm test -- GameDetail LibraryPage AppShell RouteBanner Inspector CommandPalette LibraryGrid CollectionLaunchDefaultsEditor` - PASS, 8 files / 70 tests
- `npm test` - PASS, 54 files / 437 tests
- `npm run test:smoke` - PASS, 86 tests
- `npm run build:binary` - PASS
- `rg -n "handleNavigate\\('profiles'|handleNavigate\\('launch'|onNavigate\\?\\.\\('profiles'|onNavigate\\?\\.\\('launch'" src/crosshook-native/src` - PASS, no matches
- `rg -n "route: 'profiles'|route: 'launch'|Go to Profiles|Go to Launch|Open in Profiles page" src/crosshook-native/src src/crosshook-native/tests` - PASS, no matches

## Persistence Boundary

No persisted data was added or changed. Hero Detail tab requests are runtime-only React navigation state carried through `OpenGameDetailIntent`. Existing profile, collection, launch, and install data remain in their current TOML/SQLite/runtime layers.

## Deviations

- Existing `LaunchPage.tsx` and `ProfilesHero.tsx` transitional changes were already present in the working tree before this implementation pass. They were preserved and kept type-clean rather than reverted.
- Biome still reports unrelated warning-only findings in existing files during lint, but the lint gate exits successfully.

## Next Steps

Issues #473 and #474 are implemented. The next cleanup can remove transitional legacy page modules once no dependent tests or imports require them.
