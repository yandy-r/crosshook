# Plan: Unified Desktop Phase 5 Hero Detail Mode

## Summary

Implement Phase 5 of the Unified Desktop Redesign from GitHub issue #444 / #417: replace the blocking `GameDetailsModal` detail path with an in-shell `GameDetail` mode rendered inside `LibraryPage`. The detail view reuses existing profile, metadata, art, compatibility, health, offline-readiness, and launch-history data paths while keeping the shell sidebar and inspector mounted.

## User Story

As a CrossHook library user, I want game detail to open as a main-slot takeover with tabs and responsive panels, so that navigation, inspector context, and quick actions stay stable instead of disappearing behind a blocking modal.

## Problem -> Solution

Today, the Library route still opens `GameDetailsModal` through a portal that locks body scroll and inerts the app. Phase 5 changes that to a `LibraryPage` mode transition: `library` renders the existing grid/list surface, `detail` renders `GameDetail` inside the same route stack, and Back returns to the library without remounting the route or refetching summaries.

## Metadata

- **Complexity**: Large
- **Source PRD**: `docs/prps/prds/unified-desktop-redesign.prd.md`
- **PRD Phase**: Phase 5 - Hero Detail mode
- **Estimated Files**: 14
- **GitHub Issues**: #444 tracking, #417 deliverable
- **Persistence Classification**: runtime-only `LibraryPage` mode and selected detail summary; no new TOML settings; no new SQLite tables or migrations.

## Batches

Tasks grouped by dependency for parallel execution. Tasks within the same batch run concurrently; batches run in order.

| Batch | Tasks    | Depends On | Parallel Width |
| ----- | -------- | ---------- | -------------- |
| B1    | 1.1, 1.2 | -          | 2              |
| B2    | 2.1, 2.2 | B1         | 2              |
| B3    | 3.1      | B2         | 1              |
| B4    | 4.1, 4.2 | B3         | 2              |
| B5    | 5.1      | B4         | 1              |

- **Total tasks**: 8
- **Total batches**: 5
- **Max parallel width**: 2

---

## UX Design

### Before

```text
AppShell
├─ Sidebar
├─ ContentArea: Library grid/list
├─ Inspector
└─ Portal: GameDetailsModal
   ├─ aria-modal dialog
   ├─ body scroll lock
   └─ app siblings inert / hidden
```

### After

```text
AppShell
├─ Sidebar                    stays mounted
├─ ContentArea: LibraryPage
│  ├─ mode=library: banner + toolbar + grid/list
│  └─ mode=detail: GameDetail hero + tabs + responsive panels
└─ Inspector                  stays mounted at non-deck widths
```

### Interaction Changes

| Touchpoint                | Before                                                     | After                                                                                     | Notes                                                                     |
| ------------------------- | ---------------------------------------------------------- | ----------------------------------------------------------------------------------------- | ------------------------------------------------------------------------- |
| Card/list primary click   | Selects the game for inspector when `onSelect` is present  | Same                                                                                      | Preserve Phase 4 inspector behavior.                                      |
| Card/list details control | Opens `GameDetailsModal`                                   | Enters `GameDetail` mode                                                                  | Same `onOpenDetails(name)` contract until callsite migration is complete. |
| Double-click / Enter open | Not consistently represented in current card/list controls | Opens `GameDetail` mode                                                                   | Required by PRD / issue acceptance.                                       |
| Back from detail          | Close modal                                                | Switch `LibraryPage` mode back to `library`                                               | Must not route away or remount `LibraryPage`.                             |
| Detail quick actions      | Modal closes before launch/edit navigation                 | In-shell detail calls existing handlers and switches back only for navigation-heavy flows | Preserve launch `finally` cleanup and favorite rollback.                  |

---

## Mandatory Reading

Files that MUST be read before implementing:

| Priority | File                                                                       | Lines            | Why                                                                                       |
| -------- | -------------------------------------------------------------------------- | ---------------- | ----------------------------------------------------------------------------------------- |
| P0       | `docs/prps/prds/unified-desktop-redesign.prd.md`                           | 147-152, 247-251 | Phase 5 scope, tab list, scroll registration, success signal.                             |
| P0       | `src/crosshook-native/src/components/pages/LibraryPage.tsx`                | all              | Owns summaries, selection, modal state, launch/edit/favorite handlers, and render switch. |
| P0       | `src/crosshook-native/src/components/layout/AppShell.tsx`                  | 177-244          | Shell sibling layout proves in-route detail keeps sidebar/inspector mounted.              |
| P0       | `src/crosshook-native/src/components/library/GameDetailsModal.tsx`         | 1-479            | Existing detail data composition and modal behavior to replace.                           |
| P0       | `src/crosshook-native/src/components/library/LibraryCard.tsx`              | 1-220            | Grid card selection/open-details/keyboard entry contract.                                 |
| P0       | `src/crosshook-native/src/components/library/LibraryListRow.tsx`           | 1-222            | List row selection/open-details/keyboard entry contract.                                  |
| P1       | `src/crosshook-native/src/components/library/GameInspector.tsx`            | 1-248            | Existing in-shell game info sections and launch-history pattern.                          |
| P1       | `src/crosshook-native/src/hooks/useGameDetailsProfile.ts`                  | 1-70             | Full profile load state with stale request guard.                                         |
| P1       | `src/crosshook-native/src/hooks/useGameMetadata.ts`                        | 65-135           | Metadata loading/stale/unavailable contract.                                              |
| P1       | `src/crosshook-native/src/hooks/useGameCoverArt.ts`                        | 1-75             | Artwork loading and custom-art precedence support.                                        |
| P1       | `src/crosshook-native/src/hooks/useLaunchHistoryForProfile.ts`             | 1-54             | Existing History tab data source.                                                         |
| P1       | `src/crosshook-native/src/hooks/useScrollEnhance.ts`                       | 1-120            | Scrollable selector that must include `.crosshook-hero-detail__body`.                     |
| P1       | `src/crosshook-native/src/components/pages/__tests__/LibraryPage.test.tsx` | all              | Current Library + Inspector RTL harness.                                                  |
| P1       | `src/crosshook-native/tests/smoke.spec.ts`                                 | 89-120           | Existing Playwright library inspector smoke coverage to extend.                           |
| P2       | `docs/prps/reports/unified-desktop-phase-4-library-inspector-report.md`    | 44-58            | Phase 4 implementation notes and regressions to avoid.                                    |

## External Documentation

| Topic         | Source | Key Takeaway                                                                                                  |
| ------------- | ------ | ------------------------------------------------------------------------------------------------------------- |
| External docs | none   | No new library/API research needed; `@radix-ui/react-tabs` and existing IPC/hooks are already in the project. |

---

## Patterns to Mirror

Code patterns discovered in the codebase. Follow these exactly.

### NAMING_CONVENTION

```tsx
// SOURCE: src/crosshook-native/src/components/pages/LibraryPage.tsx:266-273
<div className="crosshook-page-scroll-shell crosshook-page-scroll-shell--fill crosshook-page-scroll-shell--library">
  <div className="crosshook-route-stack crosshook-library-page">
    <div className="crosshook-route-stack__body--fill crosshook-library-page__body">
```

Use existing `crosshook-route-*` shell classes for route structure and new BEM-like `crosshook-hero-detail__*` classes for the detail surface. Do not add one-off viewport height chains.

### TAB_PATTERN

```tsx
// SOURCE: src/crosshook-native/src/components/pages/CompatibilityPage.tsx:239-262
<Tabs.Root value={activeTab} onValueChange={(value) => setActiveTab(value as TabValue)}>
  <Tabs.List className="crosshook-subtab-row">
  <Tabs.Content value="trainer" forceMount>
```

Use controlled Radix tabs with shared `crosshook-subtab-row` / `crosshook-subtab` styling. Keep tab values typed, not stringly spread across the component.

### DETAIL_DATA_PATTERN

```tsx
// SOURCE: src/crosshook-native/src/hooks/useGameDetailsProfile.ts:32-64
const requestId = nextGameDetailsRequestId(requestCounter);
void callCommand<SerializedGameProfile>('profile_load', { name: trimmed })
  .then((data) => {
    if (requestId !== requestCounter.current) return;
```

Reuse `useGameDetailsProfile(profileName, open)` for full profile data. Keep the stale-request guard instead of introducing a second profile loader.

### ERROR_HANDLING

```tsx
// SOURCE: src/crosshook-native/src/components/pages/LibraryPage.tsx:119-143
setLaunchingName(name);
try {
  await selectProfile(name, { collectionId: collectionIdForLoad });
  onNavigate?.('launch');
} finally {
  setLaunchingName(undefined);
}
```

Detail quick actions must call the existing `handleLaunch`, `handleEdit`, and `handleToggleFavorite` flows so launch cleanup, collection context, and optimistic favorite rollback stay intact.

### LOGGING_PATTERN

```tsx
// SOURCE: src/crosshook-native/src/hooks/useGameMetadata.ts:106-117
console.error('Steam metadata lookup failed', {
  requestId,
  normalizedAppId,
  error: err,
});
```

Do not add production `console.debug` logging for Hero Detail. If new async logic is unavoidable, log only contextual failures like existing hooks, and prefer rendering user-visible status text for expected unavailable data.

### SCROLL_PATTERN

```ts
// SOURCE: src/crosshook-native/src/hooks/useScrollEnhance.ts:8-10
const SCROLLABLE = '.crosshook-route-card-scroll, ... .crosshook-inspector__body';
```

Any new `overflow-y: auto` detail body must be added to `SCROLLABLE` and use `overscroll-behavior: contain`.

### TEST_STRUCTURE

```tsx
// SOURCE: src/crosshook-native/src/components/pages/__tests__/LibraryPage.test.tsx:13-31
function LibraryPageWithInspector() {
  const { inspectorSelection, libraryInspectorHandlers } = useInspectorSelection();
  return <LibraryPage />;
}
```

Extend the existing provider-based Library harness for page-level mode tests. Use `data-testid="sidebar"` / `data-testid="inspector"` for persistence assertions only where the full shell is mounted.

### MOCK_AND_IPC_PATTERN

```ts
// SOURCE: src/crosshook-native/src/lib/ipc.ts:7-16
if (isTauri()) {
  return invoke<T>(name, args);
}
```

All frontend data reads must use existing hooks and `callCommand()`-backed APIs so browser dev mode and mock coverage continue to work.

---

## Files to Change

| File                                                                        | Action | Justification                                                                                                            |
| --------------------------------------------------------------------------- | ------ | ------------------------------------------------------------------------------------------------------------------------ | ------------------------------------------------------------------------- |
| `src/crosshook-native/src/components/library/hero-detail-model.ts`          | CREATE | Shared typed tab IDs, display helpers, hero-art resolution, and detail state types.                                      |
| `src/crosshook-native/src/components/library/GameDetail.tsx`                | CREATE | In-shell detail root that owns full profile load, metadata/art hooks, preview state, and tab selection.                  |
| `src/crosshook-native/src/components/library/HeroDetailHeader.tsx`          | CREATE | Hero artwork, portrait/fallback, Back button, title, and quick actions.                                                  |
| `src/crosshook-native/src/components/library/HeroDetailTabs.tsx`            | CREATE | Controlled Radix tab shell for Overview, Profiles, Launch options, Trainer, History, Compatibility.                      |
| `src/crosshook-native/src/components/library/HeroDetailPanels.tsx`          | CREATE | Panel bodies for profile paths, metadata, health/offline readiness, launch preview, trainer, history, and compatibility. |
| `src/crosshook-native/src/components/library/__tests__/GameDetail.test.tsx` | CREATE | Unit/interaction coverage for tab rendering, loading/error states, Back, quick actions, and preview/history panels.      |
| `src/crosshook-native/src/components/pages/LibraryPage.tsx`                 | UPDATE | Replace modal state/rendering with `mode: 'library'                                                                      | 'detail'`, selected detail summary, Back handler, and callsite migration. |
| `src/crosshook-native/src/components/library/LibraryCard.tsx`               | UPDATE | Add double-click / Enter open behavior while keeping single-click inspector selection.                                   |
| `src/crosshook-native/src/components/library/LibraryListRow.tsx`            | UPDATE | Add double-click / Enter open behavior for list view while keeping single-click inspector selection.                     |
| `src/crosshook-native/src/components/pages/__tests__/LibraryPage.test.tsx`  | UPDATE | Assert detail mode entry/back, inspector remains populated, no extra summary refetch, and list-view parity.              |
| `src/crosshook-native/src/components/layout/__tests__/AppShell.test.tsx`    | UPDATE | Add full-shell assertion that entering detail keeps `data-testid="sidebar"` and desktop inspector mounted.               |
| `src/crosshook-native/src/hooks/useScrollEnhance.ts`                        | UPDATE | Register `.crosshook-hero-detail__body` in `SCROLLABLE`.                                                                 |
| `src/crosshook-native/src/styles/library.css`                               | UPDATE | Add Hero Detail layout, responsive panel grid, tabs, hero art, state, and body scroll styles.                            |
| `src/crosshook-native/tests/smoke.spec.ts`                                  | UPDATE | Add browser smoke for desktop detail enter/back and deck detail behavior without inspector.                              |
| `src/crosshook-native/src/components/library/GameDetailsModal.tsx`          | DELETE | Blocking modal is replaced by in-shell detail mode.                                                                      |
| `src/crosshook-native/src/components/library/GameDetailsModal.css`          | DELETE | Modal-only styles removed after equivalent Hero Detail styles land in `library.css`.                                     |
| `src/crosshook-native/src/components/library/useGameDetailsModalState.ts`   | DELETE | Modal-specific state hook replaced by `LibraryPage` mode state.                                                          |

## NOT Building

- No new route, URL state, deep linking, or browser history integration; detail mode is internal `LibraryPage` runtime state.
- No new backend, Tauri command, SQLite migration, or TOML setting.
- No Media tab for v1.
- No persisted selected detail, selected tab, inspector collapsed state, or sidebar variant override.
- No command palette work; the existing Phase 6 `console.debug('Command palette (Phase 6)')` placeholder stays as-is.
- No context rail, console status bar, or non-Library route redesign work.
- No new npm dependency; use existing React, Radix Tabs, and project hooks.

---

## Step-by-Step Tasks

### Task 1.1: Add Hero Detail model helpers — Depends on [none]

- **BATCH**: B1
- **ACTION**: Create `src/crosshook-native/src/components/library/hero-detail-model.ts`.
- **IMPLEMENT**: Define `HeroDetailTabId = 'overview' | 'profiles' | 'launch-options' | 'trainer' | 'history' | 'compatibility'`, `HERO_DETAIL_TABS`, display helpers such as `displayPath(value)`, and pure hero-art resolution equivalent to the modal's `resolveGameDetailsHero`. Include only pure functions and types so this file is easy to unit test or reuse.
- **MIRROR**: `Type Definitions` and `DETAIL_DATA_PATTERN`; keep typed unions like existing launch/profile types.
- **IMPORTS**: `GameProfile` from `@/types`, `LibraryCardData` from `@/types/library` only if needed for helper signatures.
- **GOTCHA**: Do not put hooks in this helper file; hooks belong in `GameDetail.tsx` or panel components.
- **VALIDATE**: `cd src/crosshook-native && npm run typecheck` reaches this file with no implicit `any` or unused type errors.

### Task 1.2: Add Hero Detail style and scroll contracts — Depends on [none]

- **BATCH**: B1
- **ACTION**: Update `src/crosshook-native/src/styles/library.css` and `src/crosshook-native/src/hooks/useScrollEnhance.ts`.
- **IMPLEMENT**: Add `.crosshook-hero-detail`, `.crosshook-hero-detail__body`, hero header, tab row, panel grid, state text, command block, and responsive grid rules. `.crosshook-hero-detail__body` must be the only new vertical scroll body and must set `overflow-y: auto; overscroll-behavior: contain;`. Append `.crosshook-hero-detail__body` to the `SCROLLABLE` selector.
- **MIRROR**: `SCROLL_PATTERN` and `NAMING_CONVENTION`.
- **IMPORTS**: None.
- **GOTCHA**: Keep styles in globally imported `library.css`; do not leave new Hero Detail styles dependent on `GameDetailsModal.css`, because the modal file is deleted later.
- **VALIDATE**: `rg -n "crosshook-hero-detail__body" src/crosshook-native/src/hooks/useScrollEnhance.ts src/crosshook-native/src/styles/library.css` shows both the selector registration and CSS rule.

### Task 2.1: Build the Hero Detail header component — Depends on [1.1, 1.2]

- **BATCH**: B2
- **ACTION**: Create `src/crosshook-native/src/components/library/HeroDetailHeader.tsx`.
- **IMPLEMENT**: Render Back, Launch, Favorite, Edit profile actions; hero background; portrait/fallback; title; profile name; Steam App ID; favorite/network pills; and profile load state summary. Accept `summary`, `profile`, art URLs/loading flags, `heroResolved`, `launchingName`, and callback props from `GameDetail`.
- **MIRROR**: `ERROR_HANDLING` for quick actions and `NAMING_CONVENTION` for class names.
- **IMPORTS**: React types as needed, `LibraryCardData`, `GameProfile`, and helper types from `hero-detail-model.ts`.
- **GOTCHA**: Back is an in-page mode switch, not browser history. Do not call `onNavigate` from Back.
- **VALIDATE**: `cd src/crosshook-native && npm test -- src/components/library/__tests__/GameDetail.test.tsx` after Task 5.1 covers header rendering and Back callback.

### Task 2.2: Build Hero Detail tab panels — Depends on [1.1, 1.2]

- **BATCH**: B2
- **ACTION**: Create `src/crosshook-native/src/components/library/HeroDetailPanels.tsx`.
- **IMPLEMENT**: Implement panel sections for Overview, Profiles, Launch options, Trainer, History, and Compatibility. Reuse `useLaunchHistoryForProfile` for History, existing ProtonDB/metadata/offline/health data passed down from `GameDetail`, and `buildProfileLaunchRequest` + `usePreviewState` output for Launch options. Render unavailable/loading/stale states as status text rather than throwing.
- **MIRROR**: `DETAIL_DATA_PATTERN`, `ERROR_HANDLING`, and `MOCK_AND_IPC_PATTERN`.
- **IMPORTS**: `GameProfile`, `LaunchPreview`, `UseGameMetadataResult`, health/offline types, `LaunchPipeline` only if a compact pipeline is rendered.
- **GOTCHA**: `buildProfileLaunchRequest` needs a resolved launch method, Steam client install path, profile name, and `settings.umu_preference`; do not call `preview_launch` with a partial request.
- **VALIDATE**: Unit tests in Task 5.1 cover each tab label and at least one loading/unavailable branch.

### Task 3.1: Compose GameDetail root and Radix tabs — Depends on [2.1, 2.2]

- **BATCH**: B3
- **ACTION**: Create `src/crosshook-native/src/components/library/GameDetail.tsx` and `src/crosshook-native/src/components/library/HeroDetailTabs.tsx`.
- **IMPLEMENT**: `GameDetail` accepts `summary`, `onBack`, `healthByName`, `healthLoading`, `offlineReportFor`, `offlineError`, `onLaunch`, `onEdit`, `onToggleFavorite`, and `launchingName`. It calls `useGameDetailsProfile(summary.name, true)`, `useGameMetadata`, `useGameCoverArt`, `usePreviewState`, and `usePreferencesContext`; computes the launch preview request only when full profile data is ready; then renders `HeroDetailHeader` and `HeroDetailTabs` with force-mounted tab content.
- **MIRROR**: `TAB_PATTERN`, `DETAIL_DATA_PATTERN`, and `MOCK_AND_IPC_PATTERN`.
- **IMPORTS**: `@radix-ui/react-tabs`, `useGameDetailsProfile`, `useGameMetadata`, `useGameCoverArt`, `usePreviewState`, `usePreferencesContext`, `buildProfileLaunchRequest`, `resolveLaunchMethod`, and Hero Detail components/helpers.
- **GOTCHA**: Do not recreate modal behavior: no portal, no `aria-modal`, no body scroll lock, no inert siblings, no private focus trap.
- **VALIDATE**: `cd src/crosshook-native && npm test -- src/components/library/__tests__/GameDetail.test.tsx` passes once tests are added.

### Task 4.1: Migrate LibraryPage from modal state to in-shell mode — Depends on [3.1]

- **BATCH**: B4
- **ACTION**: Update `src/crosshook-native/src/components/pages/LibraryPage.tsx`.
- **IMPLEMENT**: Remove `GameDetailsModal` and `useGameDetailsModalState` imports/usages. Add local state like `{ mode: 'library' | 'detail'; detailName: string | null }`; derive the active summary from current `summaries` with a fallback snapshot so favorite changes and refreshed summaries update detail without losing Back. Replace `handleOpenGameDetails` with `handleOpenGameDetail` that sets inspector selection, enters detail mode, and preserves the existing `await selectProfile(name)` side effect. Render `GameDetail` inside the route-card body when `mode === 'detail'`; render toolbar/grid/list when `mode === 'library'`.
- **MIRROR**: `Similar Implementations`, `ERROR_HANDLING`, and `TEST_STRUCTURE`.
- **IMPORTS**: `GameDetail` from `../library/GameDetail`; remove modal imports.
- **GOTCHA**: Do not key the route or unmount `LibraryPage` on mode changes. Back must not call `refreshProfiles()` or reset search/sort/filter/view state.
- **VALIDATE**: `cd src/crosshook-native && npm test -- src/components/pages/__tests__/LibraryPage.test.tsx` verifies enter/back and no extra summary refetch.

### Task 4.2: Wire card and list keyboard/open semantics — Depends on [3.1]

- **BATCH**: B4
- **ACTION**: Update `src/crosshook-native/src/components/library/LibraryCard.tsx` and `src/crosshook-native/src/components/library/LibraryListRow.tsx`.
- **IMPLEMENT**: Preserve single-click inspector selection when `onSelect` is provided. Add double-click on the card/list item or primary hitbox to call `onOpenDetails(profile.name)`. Add Enter handling that opens details while retaining existing context-menu keyboard handling. Keep the visible details icon/button behavior unchanged.
- **MIRROR**: `Similar Implementations` for current `handleHitboxClick` and `TEST_STRUCTURE` for interaction assertions.
- **IMPORTS**: Existing React `KeyboardEvent` imports are already present; add no dependency.
- **GOTCHA**: Avoid opening detail on Space if Space is already used for button activation; prevent duplicate open calls from nested buttons by stopping propagation where controls already do.
- **VALIDATE**: `cd src/crosshook-native && npm test -- src/components/library/__tests__/LibraryCard.test.tsx src/components/pages/__tests__/LibraryPage.test.tsx` covers grid and list entry paths.

### Task 5.1: Add tests, smoke coverage, and remove modal files — Depends on [4.1, 4.2]

- **BATCH**: B5
- **ACTION**: Create/update tests and delete modal-only files after all callsites are migrated.
- **IMPLEMENT**: Add `GameDetail.test.tsx`; extend `LibraryPage.test.tsx` for detail entry/back, list parity, preserved inspector selection, and no extra `profile_list_summaries` call on Back; extend `AppShell.test.tsx` for sidebar/inspector persistence after entering detail at desktop width; extend `tests/smoke.spec.ts` with desktop Hero Detail enter/back and deck Hero Detail smoke. Delete `GameDetailsModal.tsx`, `GameDetailsModal.css`, and `useGameDetailsModalState.ts`. Run `rg -n "GameDetailsModal|useGameDetailsModalState|crosshook-game-details-modal" src/crosshook-native/src` and remove any leftover imports/classes unless intentionally retained in test names.
- **MIRROR**: `TEST_STRUCTURE`, `SCROLL_PATTERN`, and `Configuration` validation commands.
- **IMPORTS**: Testing Library `screen`, `waitFor`, `within` if useful; existing `renderWithMocks`; Playwright `test`/`expect`.
- **GOTCHA**: `CollectionViewModal` imports `game-details-actions.ts`; do not delete that helper unless its remaining usage is migrated or removed. The modal file can be deleted independently.
- **VALIDATE**: `cd src/crosshook-native && npm test -- src/components/library/__tests__/GameDetail.test.tsx src/components/pages/__tests__/LibraryPage.test.tsx src/components/layout/__tests__/AppShell.test.tsx && npm run typecheck && npm run lint`.

---

## Testing Strategy

### Unit Tests

| Test                                      | Input                                                                               | Expected Output                                                                                                         | Edge Case? |
| ----------------------------------------- | ----------------------------------------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------- | ---------- |
| `GameDetail` renders overview             | `summary=makeLibraryCardData({ name: 'Synthetic Quest' })` with mocked profile load | Heading, Back button, tabs, quick actions, and profile status render                                                    | No         |
| `GameDetail` handles profile load failure | `profile_load` mock throws                                                          | Error/status copy renders inside detail, shell remains usable                                                           | Yes        |
| `GameDetail` tabs switch                  | Click Profiles, Launch options, Trainer, History, Compatibility                     | Matching tab panel becomes active and force-mounted content is stable                                                   | No         |
| Launch preview panel                      | Full profile + preferences available                                                | Preview command/status renders from existing `preview_launch` mock or shows unavailable copy when request is incomplete | Yes        |
| Library enter/back                        | Click details control from grid                                                     | `GameDetail` appears, Back returns to grid/list with search/sort/filter intact                                          | No         |
| Library no-refetch on Back                | Instrument `profile_list_summaries` handler call count                              | Enter detail and Back does not trigger a new summary fetch                                                              | Yes        |
| List parity                               | Switch to list, open details with details icon or Enter                             | Same detail mode renders                                                                                                | No         |
| Sidebar/inspector persistence             | Full `AppShell` at 1920 width, enter detail                                         | `data-testid="sidebar"` and `data-testid="inspector"` remain in DOM                                                     | Yes        |
| Deck behavior                             | Full `AppShell` at deck width, enter detail                                         | Detail renders and inspector remains absent                                                                             | Yes        |

### Edge Cases Checklist

- [ ] Profile summary has no numeric Steam App ID.
- [ ] Full profile load fails after detail opens.
- [ ] Metadata, ProtonDB, or cover-art lookups are unavailable/offline.
- [ ] Launch preview request cannot be built because game executable is empty.
- [ ] Favorite toggle fails and rolls back while detail is open.
- [ ] Detail summary disappears from the filtered list while detail is open.
- [ ] Rapidly opening detail for different games does not show stale profile data.
- [ ] Deck width has no inspector, but detail Back and tabs still work.
- [ ] `.crosshook-hero-detail__body` scrolls without scroll chaining or WebKitGTK wheel jank.

---

## Validation Commands

### Static Analysis

```bash
cd src/crosshook-native && npm run typecheck
```

EXPECT: zero TypeScript errors across source and test configs.

### Targeted Unit Tests

```bash
cd src/crosshook-native && npm test -- src/components/library/__tests__/GameDetail.test.tsx src/components/pages/__tests__/LibraryPage.test.tsx src/components/layout/__tests__/AppShell.test.tsx
```

EXPECT: Hero Detail, Library mode, and shell persistence tests pass.

### Frontend Lint

```bash
cd src/crosshook-native && npm run lint
```

EXPECT: Biome reports no lint or formatting errors in `src/`.

### Browser Smoke

```bash
cd src/crosshook-native && npm run test:smoke
```

EXPECT: Existing route smoke, library inspector smoke, and new Hero Detail enter/back smoke pass in browser dev mode.

### Mock Coverage

```bash
cd src/crosshook-native && npm run dev:browser:check
```

EXPECT: mock command coverage remains complete. This should not require adding handlers because Phase 5 uses existing commands.

### Repo-Level Check

```bash
./scripts/lint.sh --ts
```

EXPECT: TypeScript/Biome checks pass through the repository wrapper. Full `./scripts/lint.sh` may also be run before PR if Rust/shell checks are desired.

### Grep Checks

```bash
rg -n "GameDetailsModal|useGameDetailsModalState|crosshook-game-details-modal" src/crosshook-native/src
rg -n "crosshook-hero-detail__body" src/crosshook-native/src/hooks/useScrollEnhance.ts src/crosshook-native/src/styles/library.css
```

EXPECT: first command has no production references after cleanup; second command shows both scroll registration and CSS.

### Manual Validation

- [ ] At 1920x1080 browser dev mode, open Library, select a game for inspector, then open detail; sidebar and inspector remain visible.
- [ ] Back returns to the same Library view mode and keeps the current search/sort/filter state.
- [ ] At 1024x800 browser dev mode, open detail; layout stacks cleanly and inspector remains absent.
- [ ] Launch/Favorite/Edit profile quick actions behave like the existing Library card actions.
- [ ] Tab content is keyboard reachable and does not trap focus like a modal.

---

## Acceptance Criteria

- [ ] `GameDetail.tsx` renders in the Library route main slot with hero artwork, portrait/fallback, quick actions, and tabs.
- [ ] Tabs exist for Overview, Profiles, Launch options, Trainer, History, and Compatibility; Media is not present.
- [ ] Library grid and list can enter detail mode through details control, Enter, and double-click behavior.
- [ ] Back returns to Library without route navigation, `LibraryPage` remount, or an unnecessary summary refetch.
- [ ] Sidebar remains mounted after entering detail.
- [ ] Inspector remains mounted at desktop/narrow widths and remains absent at deck width.
- [ ] `.crosshook-hero-detail__body` is registered in `useScrollEnhance` and has `overscroll-behavior: contain`.
- [ ] `GameDetailsModal.tsx`, `GameDetailsModal.css`, and `useGameDetailsModalState.ts` are removed after all callsites are migrated.
- [ ] No new persisted data, backend command, dependency, or route is introduced.
- [ ] Targeted unit tests, typecheck, lint, mock coverage, and smoke tests pass.

## Completion Checklist

- [ ] Code follows route-shell, BEM class, Radix tabs, and existing hook patterns.
- [ ] Existing launch/edit/favorite action semantics are preserved.
- [ ] Profile/metadata/art/compatibility/history failures render status copy instead of crashing.
- [ ] Tests cover grid and list entry paths.
- [ ] Tests cover desktop sidebar/inspector persistence and deck inspector absence.
- [ ] Scroll selector registration is verified.
- [ ] Modal-only files and CSS are removed.
- [ ] `rg` shows no stale modal production references.
- [ ] Issue #417 and tracking issue #444 can be linked from the eventual PR.

## Risks

| Risk                                                                         | Likelihood | Impact | Mitigation                                                                                                                                                                                       |
| ---------------------------------------------------------------------------- | ---------- | ------ | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| Opening detail accidentally remounts `LibraryPage` and refetches summaries   | Medium     | Medium | Keep mode as local state under the existing `library` route and add call-count test around `profile_list_summaries`.                                                                             |
| Double-click / Enter opens detail twice because nested buttons bubble events | Medium     | Medium | Keep stop-propagation on nested buttons and test exact open callback counts.                                                                                                                     |
| Launch preview request is built with incomplete settings/profile data        | Medium     | Medium | Use `buildProfileLaunchRequest` with `resolveLaunchMethod`, `defaultSteamClientInstallPath`, selected profile name, and `settings.umu_preference`; render unavailable copy when request is null. |
| Deleting modal CSS breaks reused section styles                              | Medium     | Medium | Do not depend on `.crosshook-game-details-modal__*`; create Hero Detail classes in `library.css` before deleting the modal stylesheet.                                                           |
| Inspector selection loops regress from Phase 4                               | Low        | High   | Preserve `libraryCardDataEqual` guarded selection sync and add no new effect that writes inspector selection from derived objects every render.                                                  |
| Browser smoke fails due missing mock command coverage                        | Low        | Medium | Use existing IPC commands only and run `npm run dev:browser:check`.                                                                                                                              |

## Notes

- `--no-worktree` was requested, so this plan intentionally omits `## Worktree Setup` and per-task worktree fields.
- This is a frontend-only PRP plan. It changes runtime state and UI composition, not persisted data.
- Existing issue labels include `area:ui`, `phase:5`, `type:feature`, `feat:hero-detail-mode`, and `source:prd`; do not invent new labels when opening the PR.
- PR title should be Conventional Commits, for example `feat(ui): add in-shell hero detail mode`.
