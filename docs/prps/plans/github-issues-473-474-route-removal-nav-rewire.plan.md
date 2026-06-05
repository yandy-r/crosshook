# Plan: Route Removal and Hero Detail Navigation Rewire (Issues #473 and #474)

## Summary

Remove the legacy `profiles` and `launch` route values from the app route contract, then consume the resulting TypeScript failures as the rewire checklist for every remaining Profiles/Launch deep-link. The final implementation state opens Library Hero Detail on the correct `profiles` or `launch-options` tab for library cards, health remediation, install completion, collection actions, and command palette profile commands.

This plan intentionally combines #473 and #474 because #473's expected end state is a deliberate `tsc` failure. The implementation should still perform the Phase 8 shrink first and capture the stale caller list, but the delivered branch should finish Phase 9 with `typecheck` and `npm test` clean.

## User Story

As a CrossHook user, I want every legacy profile-edit or launch shortcut to open the selected game inside Hero Detail on the correct tab, so that Profiles and Launch are no longer standalone navigation destinations.

## Problem -> Solution

The visible sidebar already treats Library as the only Game route, but `AppRoute`, route metadata, command palette route commands, breadcrumbs, and multiple callsites still target hidden `profiles` and `launch` routes. -> Shrink the route union and metadata, then rewire every stale caller to the existing Library `openGameDetail` intent plus a Hero Detail tab intent.

## Metadata

- **Complexity**: Large
- **Source PRD**: `docs/prps/prds/unified-desktop-hero-detail-consolidation.prd.md`
- **PRD Phase**: Phase 8 (`#473`) plus Phase 9 (`#474`)
- **GitHub Issues**: #473, #474
- **Estimated Files**: 18
- **Research Dispatch**: Enhanced mode requested. Native enhanced preflight failed because the installed YCC plugin cache lacks `ycc/agents/prp-researcher`; equivalent seven-role research was performed with standalone agents where available plus local synthesis.
- **Worktree Mode**: Disabled by request (`--no-worktree`); no worktree setup section is included.
- **Confidence Score**: 8/10

---

## Storage Boundary & Persistence

| Datum                                                                  | Classification                                                     | Behavior                                                                                                   |
| ---------------------------------------------------------------------- | ------------------------------------------------------------------ | ---------------------------------------------------------------------------------------------------------- |
| `AppRoute` and route metadata values                                   | Runtime/type-level frontend contract                               | Compile-time-only route contract; no persisted data.                                                       |
| `AppNavigateOptions.heroDetailTab` / `OpenGameDetailIntent` tab target | Runtime-only React intent state                                    | One-shot navigation intent held in `AppShell` and consumed by `LibraryPage` / `GameDetail`; not persisted. |
| Selected profile from legacy flows                                     | Existing profile TOML selection via `ProfileContext.selectProfile` | Existing profile load path only; no profile data shape changes.                                            |
| Health fallback/toast text                                             | Runtime-only UI state                                              | Optional transient alert/toast for orphan profile targets; not persisted.                                  |
| SQLite metadata and settings TOML                                      | Unchanged                                                          | No DB migration and no `settings.toml` change.                                                             |

- **Migration / backward compatibility**: No persisted migration. Old `/profiles` and `/launch` route values stop compiling as route destinations; physical pages remain until Phase 10.
- **Offline behavior**: Fully local UI routing and profile selection. Existing health/install/profile IPC behavior is unchanged.
- **Degraded fallback**: Unknown or orphan profile intents must not crash; drop unknown Library detail intents or show a brief health-dashboard fallback message where the PRD requires it.
- **User visibility / editability**: Users see Library plus Hero Detail tabs; Profiles and Launch are removed as route-level destinations but remain editable inside Hero Detail.

---

## Batches

Tasks grouped by dependency for parallel execution. Tasks within the same batch can run concurrently; batches run in order.

| Batch | Tasks         | Depends On | Parallel Width |
| ----- | ------------- | ---------- | -------------- |
| B1    | 1.1, 1.2      | -          | 2              |
| B2    | 2.1, 2.2      | B1         | 2              |
| B3    | 3.1, 3.2, 3.3 | B2         | 3              |
| B4    | 4.1, 4.2      | B3         | 2              |
| B5    | 5.1           | B4         | 1              |

- **Total tasks**: 10
- **Total batches**: 5
- **Max parallel width**: 3
- **Same-file collision check**: No batch assigns the same file to two tasks. `AppShell.tsx` is isolated in B3. Tests that touch route fallout are isolated after the production rewires land.

---

## UX Design

### Before

```text
Sidebar:
  Game -> Library only

Hidden/stale route paths still exist:
  AppRoute: library | profiles | launch | ...
  Palette: Go to Profiles, Go to Launch
  Library/health/install/collection actions: navigate to profiles or launch
```

### After

```text
Sidebar:
  Game -> Library only

Legacy deep-links:
  Edit profile -> Library -> Hero Detail -> Profiles tab
  Launch profile -> Library -> Hero Detail -> Launch options tab
  Install completion -> Library -> new game's Hero Detail -> Profiles tab
  Collection modal -> close modal -> selected game's Hero Detail tab
```

### Interaction Changes

| Touchpoint                 | Before                                                                 | After                                                                     | Notes                                                                   |
| -------------------------- | ---------------------------------------------------------------------- | ------------------------------------------------------------------------- | ----------------------------------------------------------------------- |
| Route metadata/status      | `Profiles` and `Launch` can still render as routes                     | Only remaining `AppRoute` values can render route chrome                  | Physical legacy page files stay until Phase 10.                         |
| Library card Edit          | Select profile then navigate to `profiles`                             | Open Hero Detail for the card and select `profiles` tab                   | Preserve current `selectProfile` behavior.                              |
| Library card Launch        | Select profile then navigate to `launch`                               | Open Hero Detail for the card and select `launch-options` tab             | Launch button inside Hero Detail should remain usable.                  |
| Health Fix                 | Navigate to Profiles                                                   | Open Library Hero Detail on `profiles`; orphan profile falls back visibly | Keep row click/Enter semantics.                                         |
| Install review save        | Returns to Profiles view                                               | Returns to Library Hero Detail on `profiles` for the saved profile        | Update modal copy.                                                      |
| Collection modal           | Closes and routes to Profiles/Launch                                   | Closes and opens Hero Detail on the relevant tab                          | Preserve collection default context for launch loads.                   |
| Collection defaults editor | Link button says `Open in Profiles page`                               | Remove link-out button and stale copy                                     | Inline editor remains.                                                  |
| Command palette            | Profile commands route to Profiles/Launch; route commands include both | Profile commands open Hero Detail; deleted route commands disappear       | Palette icons may still use launch/profile glyphs for profile commands. |

---

## Mandatory Reading

Files that MUST be read before implementing:

| Priority       | File                                                                                 | Lines                             | Why                                                                                          |
| -------------- | ------------------------------------------------------------------------------------ | --------------------------------- | -------------------------------------------------------------------------------------------- |
| P0 (critical)  | `docs/prps/prds/unified-desktop-hero-detail-consolidation.prd.md`                    | 389-413                           | Exact Phase 8/9 scope, dependencies, and expected compiler-driven workflow.                  |
| P0 (critical)  | `src/crosshook-native/src/components/layout/Sidebar.tsx`                             | 22-37, 83-125                     | `AppRoute` owner and current sidebar shape.                                                  |
| P0 (critical)  | `src/crosshook-native/src/components/layout/routeMetadata.ts`                        | 43-139                            | Route metadata and route nav labels that must shrink with `AppRoute`.                        |
| P0 (critical)  | `src/crosshook-native/src/types/navigation.ts`                                       | 1-23                              | Existing `heroDetailTab`, `openGameDetail`, and intent types to extend rather than replace.  |
| P0 (critical)  | `src/crosshook-native/src/components/layout/AppShell.tsx`                            | 73-140, 187-205, 270-287, 515-532 | Shell-level intent creation, collection actions, command palette dispatch, and modal wiring. |
| P0 (critical)  | `src/crosshook-native/src/components/pages/LibraryPage.tsx`                          | 174-242, 445-457                  | Existing selection, open-detail intent consumption, and `GameDetail` mount.                  |
| P0 (critical)  | `src/crosshook-native/src/components/library/GameDetail.tsx`                         | 45-46, 174-215                    | Hero Detail tab state owner; must accept a one-shot initial/requested tab.                   |
| P0 (critical)  | `src/crosshook-native/src/components/library/HeroDetailTabs.tsx`                     | 11-45                             | Controlled Hero Detail tabs and stable tab test IDs.                                         |
| P1 (important) | `src/crosshook-native/src/components/pages/HealthDashboardPage.tsx`                  | 67-70, 214-220, 429-447           | Health remediation callsites and row action behavior.                                        |
| P1 (important) | `src/crosshook-native/src/components/pages/InstallPage.tsx`                          | 278-316                           | Post-install save redirect and stale user-facing copy.                                       |
| P1 (important) | `src/crosshook-native/src/components/collections/CollectionViewModal.tsx`            | 19-48, 126-136, 236-240           | Collection modal action contract and launch-default editor prop.                             |
| P1 (important) | `src/crosshook-native/src/components/collections/CollectionLaunchDefaultsEditor.tsx` | 62-83, 296-303                    | Profiles-page link-out contract to remove.                                                   |
| P1 (important) | `src/crosshook-native/src/lib/commands.ts`                                           | 68-134, 142-160                   | Command palette route/profile command definitions.                                           |
| P1 (important) | `src/crosshook-native/src/lib/validAppRoutes.ts`                                     | 1-19                              | Runtime route guard must shrink with `AppRoute`.                                             |
| P1 (important) | `src/crosshook-native/src/components/pages/__tests__/LibraryPage.test.tsx`           | 304-347                           | Existing open-detail intent and legacy route expectations to rewrite.                        |
| P1 (important) | `src/crosshook-native/src/components/library/__tests__/GameDetail.test.tsx`          | 17-48, 82-150                     | Mocked tab-state test pattern for requested initial tab coverage.                            |
| P1 (important) | `src/crosshook-native/src/components/layout/__tests__/AppShell.test.tsx`             | 500-573                           | Legacy breadcrumb/palette assertions that must be replaced or removed.                       |
| P2 (reference) | `src/crosshook-native/src/components/library/HeroDetailHeader.tsx`                   | 52-81                             | Header quick actions currently call navigate-heavy helpers.                                  |

## External Documentation

| Topic                | Source | Key Takeaway                                                                               |
| -------------------- | ------ | ------------------------------------------------------------------------------------------ |
| External APIs / SDKs | N/A    | No external research needed; this is internal React/TypeScript routing and state plumbing. |

---

## Patterns to Mirror

Code patterns discovered in the codebase. Follow these exactly.

### NAMING_CONVENTION

```ts
// SOURCE: src/crosshook-native/src/components/library/hero-detail-model.ts:5-18
export type HeroDetailTabId = 'overview' | 'profiles' | 'launch-options' | 'trainer' | 'history' | 'compatibility';
export const HERO_DETAIL_TABS = [
  { id: 'profiles', label: 'Profiles' },
  { id: 'launch-options', label: 'Launch options' },
];
```

```ts
// SOURCE: src/crosshook-native/src/types/navigation.ts:16-23
export interface AppNavigateOptions {
  libraryFilter?: LibraryFilterKey;
  heroDetailTab?: HeroDetailTabId;
  profileName?: string;
  openGameDetail?: string;
}
```

```ts
// SOURCE: src/crosshook-native/src/lib/commands.ts:19, 142-160
export type CommandPaletteAction = 'route' | 'launch_profile' | 'edit_profile';
id: `profile:launch-current:${trimmed}`;
id: `profile:edit-current:${trimmed}`;
```

### ERROR_HANDLING

```ts
// SOURCE: src/crosshook-native/src/lib/validAppRoutes.ts:17-19
export function isAppRoute(value: string): value is AppRoute {
  return Object.prototype.hasOwnProperty.call(VALID_APP_ROUTES, value);
}
```

```ts
// SOURCE: src/crosshook-native/src/hooks/profile/useProfileCrud.ts:161-167
const fullMsg = loadOptions?.loadErrorContext ? `${loadOptions.loadErrorContext}: ${msg}` : msg;
setError(fullMsg);
if (loadOptions?.throwOnFailure) {
  throw fullMsg;
}
```

```tsx
// SOURCE: src/crosshook-native/src/components/layout/AppShell.tsx:544-556
<div className="crosshook-status-toast crosshook-rename-toast" role="status" aria-live="polite">
  <span>Name saved, but the description could not be saved.</span>
  <button type="button" className="crosshook-rename-toast-dismiss" aria-label="Dismiss">
```

### LOGGING_PATTERN

```ts
// SOURCE: src/crosshook-native/src/components/pages/HealthDashboardPage.tsx:103-107
function handleRetry() {
  if (error) {
    console.error('Health scan error (retrying):', error);
  }
  void batchValidate();
}
```

### REPOSITORY_PATTERN

```ts
// SOURCE: src/crosshook-native/src/components/layout/routeMetadata.ts:43-65
export const ROUTE_METADATA: Record<AppRoute, RouteMetadataEntry> = {
  library: { navLabel: 'Library', inspectorComponent: GameInspector },
  profiles: { navLabel: 'Profiles', Art: ProfilesArt },
  launch: { navLabel: 'Launch', Art: LaunchArt },
};
```

```ts
// SOURCE: src/crosshook-native/src/lib/commands.ts:124-134
export const ROUTE_COMMANDS: readonly CommandPaletteCommand[] = (
  Object.entries(ROUTE_TITLES) as Array<[AppRoute, string]>
).map(([route, title]) => ({
  id: `route:${route}`,
```

### SERVICE_PATTERN

```ts
// SOURCE: src/crosshook-native/src/components/layout/AppShell.tsx:113-139
if (options?.openGameDetail) {
  openGameDetailIntentTokenRef.current += 1;
  setOpenGameDetailIntent({
    profileName: options.openGameDetail,
    token: openGameDetailIntentTokenRef.current,
```

```ts
// SOURCE: src/crosshook-native/src/components/pages/LibraryPage.tsx:230-242
if (!summaries.some((s) => s.name === openGameDetailIntent.profileName)) {
  return;
}
handledOpenGameDetailTokenRef.current = openGameDetailIntent.token;
void handleOpenGameDetail(openGameDetailIntent.profileName);
```

```ts
// SOURCE: src/crosshook-native/src/components/library/GameDetail.tsx:174-184
const handleSetActiveTab = useCallback((tab: HeroDetailTabId, options?: HeroDetailTabRequestOptions) => {
  setProfilesScrollTarget(tab === 'profiles' ? (options?.profilesScrollTarget ?? null) : null);
  setActiveTab(tab);
}, []);
```

### TEST_STRUCTURE

```tsx
// SOURCE: src/crosshook-native/src/components/library/__tests__/GameDetail.test.tsx:17-23
const heroDetailTabsSpy = vi.fn<(props: HeroDetailTabsProps) => null>();
vi.mock('../HeroDetailTabs', () => ({
  HeroDetailTabs: (props: HeroDetailTabsProps) => {
    heroDetailTabsSpy(props);
```

```tsx
// SOURCE: src/crosshook-native/src/components/pages/__tests__/LibraryPage.test.tsx:330-346
renderLibraryHarness({}, undefined, undefined, { profileName: 'Test Game Alpha', token: 1 });
await waitFor(() => {
  expect(screen.getByTestId('game-detail')).toBeInTheDocument();
});
```

```json
// SOURCE: src/crosshook-native/package.json:16-26
"test": "vitest run",
"test:smoke": "playwright test",
"typecheck": "tsc --noEmit && tsc -p tsconfig.test.json --noEmit"
```

---

## Files to Change

| File                                                                                 | Action | Justification                                                                                                                         |
| ------------------------------------------------------------------------------------ | ------ | ------------------------------------------------------------------------------------------------------------------------------------- |
| `src/crosshook-native/src/components/layout/Sidebar.tsx`                             | UPDATE | Remove `profiles` and `launch` from `AppRoute`; sidebar items already show only Library for Game.                                     |
| `src/crosshook-native/src/components/layout/routeMetadata.ts`                        | UPDATE | Remove deleted route metadata and nav-label entries; also remove now-unused Profiles/Launch banner art imports.                       |
| `src/crosshook-native/src/lib/validAppRoutes.ts`                                     | UPDATE | Shrink runtime route allowlist with the type union so Radix tab changes cannot accept stale routes.                                   |
| `src/crosshook-native/src/lib/commands.ts`                                           | UPDATE | Remove route command entries for deleted routes while preserving `launch_profile` and `edit_profile` command actions.                 |
| `src/crosshook-native/src/types/navigation.ts`                                       | UPDATE | Extend `OpenGameDetailIntent` with `heroDetailTab`; retire legacy origin fields only where no longer needed after rewiring.           |
| `src/crosshook-native/src/components/library/hero-detail-model.ts`                   | UPDATE | Add a runtime `isHeroDetailTabId` guard or equivalent normalizer for tab intents.                                                     |
| `src/crosshook-native/src/components/layout/AppShell.tsx`                            | UPDATE | Add centralized `openGameInHeroDetail({ profileName, tab, launch })`; rewire collection, palette, and modal callbacks.                |
| `src/crosshook-native/src/components/layout/ContentArea.tsx`                         | UPDATE | Remove `profiles` and `launch` switch cases/imports from route rendering without deleting page files.                                 |
| `src/crosshook-native/src/components/layout/game-detail-trail.ts`                    | UPDATE | Remove or replace the Phase-10 breadcrumb helper if no route still uses `GameDetailOrigin`.                                           |
| `src/crosshook-native/src/components/pages/LibraryPage.tsx`                          | UPDATE | Consume tab-bearing detail intents; rewire edit/launch handlers to open local Hero Detail tabs.                                       |
| `src/crosshook-native/src/components/library/GameDetail.tsx`                         | UPDATE | Accept a requested initial tab and apply it through existing tab-state logic.                                                         |
| `src/crosshook-native/src/components/library/HeroDetailHeader.tsx`                   | UPDATE | Keep header quick actions in Hero Detail instead of bouncing through removed routes.                                                  |
| `src/crosshook-native/src/components/library/LibraryGrid.tsx`                        | UPDATE | Remove unused legacy `onNavigate` prop/import; keep the current add-game CTA unless implementation reconfirms a stale route CTA.      |
| `src/crosshook-native/src/components/library/LibraryList.tsx`                        | UPDATE | Same unused legacy `onNavigate` cleanup as `LibraryGrid`.                                                                             |
| `src/crosshook-native/src/components/pages/HealthDashboardPage.tsx`                  | UPDATE | Rewire Fix / empty state actions to Library Hero Detail Profiles, with visible fallback for orphan profiles.                          |
| `src/crosshook-native/src/components/pages/InstallPage.tsx`                          | UPDATE | Post-save redirect and modal copy must point to Library Hero Detail Profiles instead of Profiles view.                                |
| `src/crosshook-native/src/components/collections/CollectionViewModal.tsx`            | UPDATE | Rename and rewire Profiles-page callbacks to Hero Detail tab opens.                                                                   |
| `src/crosshook-native/src/components/collections/CollectionLaunchDefaultsEditor.tsx` | UPDATE | Remove the `Open in Profiles page` link-out button and stale explanatory copy.                                                        |
| `src/crosshook-native/src/components/pages/LaunchPage.tsx`                           | UPDATE | Keep file but make it type-clean after route removal by removing `RouteBanner route="launch"` or replacing with non-route local copy. |
| `src/crosshook-native/src/components/pages/profiles/ProfilesHero.tsx`                | UPDATE | Keep legacy page file but make it type-clean after route removal by replacing `RouteBanner route="profiles"`.                         |
| `src/crosshook-native/src/components/library/__tests__/LibraryGrid.test.tsx`         | UPDATE | Update test props after `onNavigate` removal and keep empty-state Add game assertion aligned with current code.                       |
| `src/crosshook-native/src/components/pages/__tests__/LibraryPage.test.tsx`           | UPDATE | Replace legacy route assertions with Hero Detail requested-tab assertions and unknown-profile fallback coverage.                      |
| `src/crosshook-native/src/components/library/__tests__/GameDetail.test.tsx`          | UPDATE | Add requested initial tab / tab intent application coverage.                                                                          |
| `src/crosshook-native/src/components/layout/__tests__/AppShell.test.tsx`             | UPDATE | Replace legacy Profiles breadcrumb/palette tests with Hero Detail tab behavior.                                                       |
| `src/crosshook-native/src/components/layout/__tests__/RouteBanner.test.tsx`          | UPDATE | Replace deleted `launch` route fixtures.                                                                                              |
| `src/crosshook-native/src/components/layout/__tests__/Inspector.test.tsx`            | UPDATE | Replace deleted `profiles` route fixtures.                                                                                            |
| `src/crosshook-native/src/components/palette/__tests__/CommandPalette.test.tsx`      | UPDATE | Remove `route: 'profiles'` fixture and keep icon/render tests route-valid.                                                            |

## NOT Building

- Physical deletion of `ProfilesPage.tsx`, `LaunchPage.tsx`, `pages/profiles/`, `pages/launch/`, or legacy route tests; that is Phase 10 / #475.
- URL routing, browser history, query params, or a global profile library as a replacement for these route values.
- New backend IPC commands, new Tauri commands, schema migrations, settings keys, or host-command behavior.
- New Hero Detail tab components; `profiles` and `launch-options` already exist as `HeroDetailTabId` values.
- Runtime execution changes for launch hooks, launch-session cleanup, Proton Manager behavior, or collection default persistence.
- Broad palette redesign. Keep existing command actions and row rendering; only remove deleted route commands and change profile-command destinations.

---

## Step-by-Step Tasks

### Task 1.1: Shrink Route Sources Of Truth — Depends on [none]

- **BATCH**: B1
- **ACTION**: Remove `profiles` and `launch` from every route registry source, then capture the expected `typecheck` failure list.
- **IMPLEMENT**: Update `AppRoute`, `ROUTE_METADATA`, `ROUTE_NAV_LABEL`, `VALID_APP_ROUTES`, and palette route maps so deleted routes are no longer route destinations. Do not delete legacy page files. Run `npm run typecheck` from `src/crosshook-native`; save the failures as the Phase 9 checklist in implementation notes or the eventual report.
- **MIRROR**: `REPOSITORY_PATTERN` and `ERROR_HANDLING` allowlist patterns.
- **IMPORTS**: Remove now-unused `ProfilesArt`, `LaunchArt`, and route command map entries as TypeScript exposes them.
- **GOTCHA**: `profiles` remains a valid `HeroDetailTabId`; delete it only from `AppRoute`-keyed maps, not from Hero Detail tabs or profile domain strings.
- **VALIDATE**: `cd src/crosshook-native && npm run typecheck` fails only on stale callers / tests / legacy page type fallout that must be consumed by later tasks.

### Task 1.2: Make Legacy Page Files Type-Clean Without Routing Them — Depends on [none]

- **BATCH**: B1
- **ACTION**: Keep legacy page files present but remove their dependency on deleted route IDs.
- **IMPLEMENT**: Remove `profiles` / `launch` cases and imports from `ContentArea`. In `LaunchPage.tsx` and `ProfilesHero.tsx`, replace `RouteBanner route="launch"` / `route="profiles"` with local markup or constants that do not require `AppRoute`; keep the files for Phase 10 deletion.
- **MIRROR**: `SERVICE_PATTERN` route rendering through `ContentArea` and `REPOSITORY_PATTERN` route metadata boundaries.
- **IMPORTS**: Remove unused `LaunchPage` / `ProfilesPage` imports from `ContentArea`; remove `RouteBanner` imports from orphaned pages if they become invalid.
- **GOTCHA**: Do not physically delete the pages or subdirectories in this phase, even if they are no longer reachable.
- **VALIDATE**: Re-run `cd src/crosshook-native && npm run typecheck`; the remaining failures should be navigation callsites/tests, not orphaned page render code.

### Task 2.1: Extend And Validate Hero Detail Intent Payloads — Depends on [1.1]

- **BATCH**: B2
- **ACTION**: Add tab-aware detail-open intent types and a runtime guard for Hero Detail tab IDs.
- **IMPLEMENT**: Extend `OpenGameDetailIntent` with `heroDetailTab?: HeroDetailTabId`. Add `isHeroDetailTabId(value: string): value is HeroDetailTabId` in `hero-detail-model.ts` or an equivalent local normalizer, mirroring `isAppRoute`; invalid external values fall back to `overview`.
- **MIRROR**: `NAMING_CONVENTION` for `heroDetailTab` and `ERROR_HANDLING` allowlist validation.
- **IMPORTS**: `HeroDetailTabId` from `hero-detail-model`; keep navigation type imports type-only.
- **GOTCHA**: Do not validate tab IDs through `AppRoute`; route and tab string domains intentionally overlap on `profiles`.
- **VALIDATE**: Add or extend a small type/unit test if practical; otherwise `npm run typecheck` must prove all consumers agree on the new intent shape.

### Task 2.2: Teach LibraryPage And GameDetail To Consume Requested Tabs — Depends on [2.1]

- **BATCH**: B2
- **ACTION**: Reuse LibraryPage's one-shot `openGameDetailIntent` path and GameDetail's controlled tab state to open the requested Hero Detail tab.
- **IMPLEMENT**: Split `handleOpenGameDetail` so it accepts `{ profileName, heroDetailTab }`, opens detail mode, selects the profile, and stores a one-shot requested tab for the next `GameDetail` render. Pass that requested tab into `GameDetail`; inside `GameDetail`, apply it through the existing `handleSetActiveTab` / `setActiveTab` path and clear stale profile scroll targets when the tab is not `profiles`.
- **MIRROR**: `SERVICE_PATTERN` for Library intent consumption and Hero tab controller state.
- **IMPORTS**: `HeroDetailTabId` and possibly `isHeroDetailTabId`; keep no new dependency.
- **GOTCHA**: Unknown profile intents are currently dropped silently. Change that only enough to surface a status/fallback when required by calling flows; do not let a stale token replay after summaries refresh.
- **VALIDATE**: Extend `LibraryPage.test.tsx` for an `openGameDetailIntent` with `heroDetailTab: 'profiles'` and `GameDetail.test.tsx` for requested `launch-options` / invalid-tab fallback.

### Task 3.1: Add AppShell openGameInHeroDetail Helper — Depends on [2.1, 2.2]

- **BATCH**: B3
- **ACTION**: Centralize legacy profile/launch navigation in one AppShell helper.
- **IMPLEMENT**: Implement `openGameInHeroDetail({ profileName, tab, launch })` in `AppShell`. Trim/validate `profileName`; validate `tab`; for profile editing use raw `selectProfile(profileName, { throwOnFailure: true })`; for launch-tab flows preserve collection context only where the old caller already loaded collection defaults. Then call `handleNavigate('library', { openGameDetail: profileName, profileName, heroDetailTab: tab })`.
- **MIRROR**: `SERVICE_PATTERN` central shell intent owner and `ERROR_HANDLING` `throwOnFailure` pattern.
- **IMPORTS**: `HeroDetailTabId`, `formatInvokeError` if helper-level failures become user-facing, and existing `selectProfile`.
- **GOTCHA**: The `launch` flag should preserve existing semantics for launch-oriented flows and collection default loading; do not auto-start the game unless the existing caller already did so.
- **VALIDATE**: Add helper-driven AppShell tests or targeted integration tests showing stale profile errors surface through a status toast instead of navigating to a deleted route.

### Task 3.2: Rewire Collection Modal And Command Palette Dispatch — Depends on [3.1]

- **BATCH**: B3
- **ACTION**: Replace AppShell's remaining collection and palette `handleNavigate('profiles'|'launch')` calls.
- **IMPLEMENT**: Change `handleLaunchFromCollection`, `handleEditFromCollection`, `launch_profile`, `edit_profile`, and the `CollectionViewModal` footer callback to call `openGameInHeroDetail` with `launch-options` or `profiles`. Remove route commands for `Go to Profiles` and `Go to Launch`, while keeping profile command IDs and icon IDs stable.
- **MIRROR**: `NAMING_CONVENTION` for command action names and `SERVICE_PATTERN` shell helper.
- **IMPORTS**: Keep `LaunchIcon` / `ProfilesIcon` only if still used by profile command rows.
- **GOTCHA**: Collection edit must not pass `collectionId`; merged collection defaults are for launch context and must not be saved from the Profiles tab.
- **VALIDATE**: Update `AppShell.test.tsx` and `CommandPalette.test.tsx` so profile commands open Hero Detail on `launch-options` / `profiles`, and route commands no longer include deleted routes.

### Task 3.3: Rewire Library Card And Header Actions — Depends on [2.2]

- **BATCH**: B3
- **ACTION**: Replace Library and Hero Detail quick-action route bounces with in-place Hero Detail tab switches.
- **IMPLEMENT**: In `LibraryPage`, make `handleLaunch` and `handleEdit` open the detail view with requested tabs instead of calling `onNavigate?.('launch'|'profiles')`. Update `HeroDetailHeader` quick actions so `Launch` switches to `launch-options` and `Edit profile` switches to `profiles` through a callback, rather than `gameDetailsLaunchThenNavigate` / `gameDetailsEditThenNavigate`.
- **MIRROR**: `SERVICE_PATTERN` `GameDetail.handleSetActiveTab` and `TEST_STRUCTURE` mocked Hero Detail tab state.
- **IMPORTS**: Remove `game-details-actions` imports if no longer used; remove `onNavigate` props from `LibraryGrid` / `LibraryList`.
- **GOTCHA**: The current empty Library CTA is `Add game`, not a Profiles-route CTA. Do not remove the add-game path unless implementation reconfirms an actual stale route CTA after Task 1.1.
- **VALIDATE**: Update `LibraryPage.test.tsx` and `LibraryGrid.test.tsx`; clicking Library Launch/Edit should leave route as Library and show/select the requested Hero Detail tab.

### Task 4.1: Rewire Health Dashboard And Install Completion — Depends on [3.1]

- **BATCH**: B4
- **ACTION**: Replace dashboard and install page redirects to Profiles with Library Hero Detail Profiles.
- **IMPLEMENT**: Change `HealthDashboardPage` `handleFixNavigation` and the empty-state action to call `onNavigate?.('library', { openGameDetail: profileName, profileName, heroDetailTab: 'profiles' })` after a successful raw profile load, or show a polite status fallback for orphan/cached profile names. Change `InstallPage` post-review save to navigate to the saved profile in Library Hero Detail Profiles and update stale copy that says it returns to the Profiles view.
- **MIRROR**: `ERROR_HANDLING` status toast/polite status and `SERVICE_PATTERN` Library open intent.
- **IMPORTS**: `AppNavigateOptions` in `HealthDashboardPage` prop typing if needed; `formatInvokeError` for load failures if surfaced.
- **GOTCHA**: `selectProfile()` swallows failures unless `throwOnFailure: true`; use that for user-triggered remediation flows.
- **VALIDATE**: Add tests for known profile fix, orphan profile fallback, and install save redirect payload.

### Task 4.2: Remove Collection Defaults Profiles-Page Link-Out — Depends on [3.2]

- **BATCH**: B4
- **ACTION**: Delete the stale Profiles-page affordance inside the collection launch defaults editor.
- **IMPLEMENT**: Remove `onOpenInProfilesPage` from `CollectionLaunchDefaultsEditor` props and from `CollectionViewModal` when no profile-specific target exists. Replace copy that says advanced overrides are managed from the Profiles page with Hero Detail Profiles / Launch options wording only if a replacement message is useful; otherwise omit the stale paragraph.
- **MIRROR**: `NOT Building` scope boundary and existing inline collection defaults editor pattern.
- **IMPORTS**: Remove now-unused callback prop types.
- **GOTCHA**: Do not expand the inline collection defaults editor to cover all profile-level overrides; this phase only removes a dead link.
- **VALIDATE**: Focused collection component tests compile; manual grep finds no `Open in Profiles page`.

### Task 5.1: Test Cleanup, Grep Guards, And Final Validation — Depends on [1.1, 1.2, 2.1, 2.2, 3.1, 3.2, 3.3, 4.1, 4.2]

- **BATCH**: B5
- **ACTION**: Update stale route tests and run all required validation gates.
- **IMPLEMENT**: Replace deleted route fixtures in `RouteBanner.test.tsx`, `Inspector.test.tsx`, `AppShell.test.tsx`, `LibraryPage.test.tsx`, `LibraryGrid.test.tsx`, and `CommandPalette.test.tsx`. Add grep guards for production route dispatch and route command generation. Fix only fallout from the rewire.
- **MIRROR**: `TEST_STRUCTURE` `renderWithMocks`, provider harnesses, and Hero Detail tab-state spy patterns.
- **IMPORTS**: Test-only imports from `@testing-library/react`, `userEvent`, and local fixtures as existing tests already use.
- **GOTCHA**: Do not delete Phase 10-owned tests/files unless TypeScript cannot compile without narrowly rewriting them.
- **VALIDATE**: Run every command in `Validation Commands`; final `npm run typecheck` and `npm test` must pass.

---

## Testing Strategy

### Unit Tests

| Test                                    | Input                                                                     | Expected Output                                                           | Edge Case? |
| --------------------------------------- | ------------------------------------------------------------------------- | ------------------------------------------------------------------------- | ---------- |
| `GameDetail` requested tab              | `requestedTab="profiles"` and `requestedTab="launch-options"`             | Mocked `HeroDetailTabs` receives matching `activeTab`                     | No         |
| `GameDetail` invalid requested tab      | Bad string normalized before reaching `GameDetail`                        | Active tab falls back to `overview`                                       | Yes        |
| `LibraryPage` open intent               | `{ profileName: 'Test Game Alpha', heroDetailTab: 'profiles', token: 1 }` | Detail mode opens and Profiles tab is selected                            | No         |
| `LibraryPage` stale open intent         | Unknown profile name                                                      | No crash; fallback status or no replay according caller contract          | Yes        |
| `AppShell` palette launch command       | `launch_profile` command                                                  | Library Hero Detail opens on `launch-options`; no `launch` route dispatch | No         |
| `AppShell` palette edit command         | `edit_profile` command                                                    | Library Hero Detail opens on `profiles`; no `profiles` route dispatch     | No         |
| `HealthDashboardPage` fix action        | Known unhealthy profile                                                   | Navigate payload targets Library + Hero Detail Profiles                   | No         |
| `HealthDashboardPage` orphan fix action | Cached profile absent from library summaries / failed load                | Polite fallback status and no deleted route navigation                    | Yes        |
| `InstallPage` review save               | Successful `persistProfileDraft`                                          | Navigate payload targets Library + Hero Detail Profiles for saved profile | No         |
| `CollectionLaunchDefaultsEditor`        | Render editor                                                             | No `Open in Profiles page` button or stale Profiles copy                  | No         |

### Edge Cases Checklist

- [ ] Blank `profileName` in `openGameInHeroDetail`.
- [ ] Deleted/stale profile command still present in an old command palette render.
- [ ] Invalid `heroDetailTab` option.
- [ ] Health snapshot row exists but no Library summary card exists.
- [ ] Collection launch preserves collection-default load context only for launch-oriented flow.
- [ ] Collection edit opens raw profile, not collection-merged profile.
- [ ] `profiles` remains a Hero Detail tab ID while no longer being an `AppRoute`.
- [ ] `launch` remains valid domain language/icon/pipeline text while no longer being an `AppRoute`.

---

## Validation Commands

### Intermediate Phase 8 Compiler Checklist

```bash
cd src/crosshook-native
npm run typecheck
```

EXPECT: Immediately after Task 1.1 / Task 1.2, this may fail. The failure list must be captured and fully consumed by Tasks 2-5; do not stop with this as the final branch state.

### Static Analysis

```bash
cd src/crosshook-native
npm run typecheck
```

EXPECT: Final state has zero app or test TypeScript errors.

### Focused Unit Tests

```bash
cd src/crosshook-native
npm test -- GameDetail LibraryPage AppShell RouteBanner Inspector CommandPalette LibraryGrid CollectionLaunchDefaultsEditor
```

EXPECT: Focused tests pass and no stale route expectations remain.

### Full Frontend Suite

```bash
cd src/crosshook-native
npm test
```

EXPECT: All Vitest tests pass.

### Grep Guards

```bash
rg -n "handleNavigate\\('profiles'|handleNavigate\\('launch'|onNavigate\\?\\.\\('profiles'|onNavigate\\?\\.\\('launch'" src/crosshook-native/src
```

EXPECT: No production callsites remain. Test references are acceptable only when asserting deleted-route absence.

```bash
rg -n "route: 'profiles'|route: 'launch'|Go to Profiles|Go to Launch|Open in Profiles page" src/crosshook-native/src
```

EXPECT: No stale route commands or stale Profiles-page link-out copy remain.

### Lint

```bash
./scripts/lint.sh
```

EXPECT: Repo lint passes for touched frontend files.

### Smoke Validation

```bash
cd src/crosshook-native
npm run test:smoke
```

EXPECT: Browser-dev smoke passes. At minimum, command-palette and Hero Detail navigation smoke should not route to Profiles/Launch.

### Binary Build Gate

```bash
npm run build:binary
```

EXPECT: Web production build and native binary build complete. Use this as the final end-to-end gate if time permits.

### Manual Validation

- [ ] Open Library and confirm the sidebar still shows only Library under Game.
- [ ] Use a Library card Edit action; verify Hero Detail opens on Profiles.
- [ ] Use a Library card Launch action; verify Hero Detail opens on Launch options.
- [ ] Use command palette `Edit <active profile>` and `Launch <active profile>`; verify both land in Library Hero Detail tabs.
- [ ] Open a collection modal; verify Edit/Launch card actions close the modal and open the right Hero Detail tab.
- [ ] Save an install review draft; verify the saved profile opens in Library Hero Detail Profiles.
- [ ] Open Health Dashboard and use Fix on an unhealthy known profile; verify Library Hero Detail Profiles opens.

---

## Acceptance Criteria

- [ ] `AppRoute` no longer includes `profiles` or `launch`.
- [ ] `ROUTE_METADATA`, `ROUTE_NAV_LABEL`, and `VALID_APP_ROUTES` no longer include `profiles` or `launch`.
- [ ] `ROUTE_COMMANDS` no longer generates `Go to Profiles` or `Go to Launch`.
- [ ] `ProfilesPage.tsx`, `LaunchPage.tsx`, `pages/profiles/`, and `pages/launch/` are not physically deleted in this phase.
- [ ] The intermediate Phase 8 `typecheck` failure list is captured and consumed as the Phase 9 stale-caller checklist.
- [ ] Final `npm run typecheck` passes cleanly.
- [ ] Final `npm test` passes cleanly.
- [ ] No production code dispatches `handleNavigate('profiles'|'launch')` or `onNavigate?.('profiles'|'launch')`.
- [ ] `launch_profile` opens Library Hero Detail on `launch-options`; `edit_profile` opens Library Hero Detail on `profiles`.
- [ ] Library card/header launch and edit actions keep the user in Library detail mode and switch to the requested Hero Detail tab.
- [ ] Health Dashboard fix actions open Library Hero Detail Profiles for known profiles and show a fallback for orphan/stale profiles.
- [ ] Install success opens the newly saved profile in Library Hero Detail Profiles.
- [ ] Collection modal launch/edit closes the modal and opens Library Hero Detail on the correct tab.
- [ ] Collection defaults editor no longer exposes `Open in Profiles page`.
- [ ] Collection launch/edit rewires preserve raw-edit versus collection-merged launch semantics.
- [ ] No new backend endpoint, host command, Tauri command, settings key, or database migration is introduced.

## Completion Checklist

- [ ] Route/type shrink done before callsite rewires so TypeScript drove the checklist.
- [ ] All stale compiler errors were either fixed or explicitly explained as test-only string/domain references.
- [ ] `profiles` was removed from route maps but preserved as a Hero Detail tab ID.
- [ ] `launch` was removed from route maps but preserved for icons, launch pipeline text, and command action semantics where not route-specific.
- [ ] Unknown profile/tab inputs are validated or normalized.
- [ ] Health orphan fallback is user-visible and polite.
- [ ] Collection edit path never saves a collection-merged profile.
- [ ] Tests assert Hero Detail tabs, not deleted route banners.
- [ ] No unrelated Phase 10 deletion work was pulled into this branch.
- [ ] No dependency or package-lock churn.

## Risks

| Risk                                                                                                            | Likelihood | Impact | Mitigation                                                                                                              |
| --------------------------------------------------------------------------------------------------------------- | ---------- | ------ | ----------------------------------------------------------------------------------------------------------------------- |
| Removing only `AppRoute` / metadata leaves `validAppRoutes` or palette route commands accepting deleted routes. | Medium     | High   | Task 1.1 includes `VALID_APP_ROUTES` and `ROUTE_COMMANDS`; grep guard checks route commands and stale copy.             |
| `profiles` is deleted from Hero Detail tab IDs by mistake because the string overlaps with removed route IDs.   | Medium     | High   | Treat route IDs and tab IDs as separate domains; add `isHeroDetailTabId` rather than reusing `isAppRoute`.              |
| Unknown profile intents are silently dropped and users lose remediation context.                                | Medium     | Medium | Replace silent drops where user-triggered flows require feedback; use status toast/polite status pattern.               |
| `selectProfile()` failure is swallowed, causing navigation to a stale profile.                                  | Medium     | Medium | Use `throwOnFailure: true` in helper/remediation flows and catch with user-visible fallback.                            |
| Collection edit loads collection-merged defaults and later saves them as raw profile data.                      | Low        | High   | For `tab: 'profiles'`, do not pass `collectionId`; reserve collection context for launch-oriented paths only.           |
| Header quick actions still bounce through deleted routes.                                                       | Medium     | Medium | Task 3.3 explicitly rewires `HeroDetailHeader`; grep guard catches deleted route dispatches.                            |
| Phase 10 deletion leaks into this branch.                                                                       | Medium     | Medium | NOT Building section is explicit; keep pages type-clean but present.                                                    |
| Browser smoke still expects Profiles/Launch route pages.                                                        | Medium     | Medium | Update focused tests now; run smoke if time permits and defer route-page smoke deletion only if it belongs to Phase 10. |

## Notes

- Enhanced preflight could not run in this installed plugin cache: `/home/yandy/.codex/plugins/cache/local-ycc-plugins/ycc/agents` is missing. The plan was still synthesized with the enhanced seven-role coverage using standalone agents and local codebase discovery.
- Issue #473's acceptance criteria are unusual: an intermediate compiler failure is success only for that phase. The final branch for this combined #473/#474 plan must be clean.
- The current `LibraryGrid` / `LibraryList` empty state already uses an Add game CTA. Treat the issue's "remove empty-state CTAs" line as removal of stale route navigation only unless current code exposes a real Profiles-route CTA after Task 1.1.
- Suggested implementation command: `$ycc:prp-implement --parallel --no-worktree docs/prps/plans/github-issues-473-474-route-removal-nav-rewire.plan.md`
