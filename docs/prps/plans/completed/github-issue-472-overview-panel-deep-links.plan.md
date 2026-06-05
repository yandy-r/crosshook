# Plan: Overview Panel Deep-Link Buttons (Issue #472, Phase 7)

## Summary

Turn Hero Detail's Overview tab into an action dashboard by adding Runtime, Active profile, Launch command, and Trainer hook cards with buttons that request the existing in-memory Hero Detail tab switch. Wire the Phase 1 `onSetActiveTab` channel from `GameDetail`, add a runtime-only Profiles-tab scroll target for the Runtime action, and preserve the current metadata and health/offline overview cards.

No backend, IPC, SQLite, TOML, or route changes are introduced. This plan intentionally uses Hero Detail tab state, not URL hashes or legacy `/profiles` and `/launch` route navigation.

## User Story

As a Linux gamer using CrossHook's Hero Detail view, I want Overview panels for Runtime, Active profile, Launch command, and Trainer hook to jump to the relevant in-detail editor tab, so I can configure a game without sidebar or page detours.

## Problem -> Solution

Hero Detail's Overview currently renders only metadata plus health/offline readiness, and `GameDetail` still passes `onSetActiveTab: undefined`; users must manually find the right tab after reading Overview status. -> Add button-bearing Overview cards, wire `onSetActiveTab` to `setActiveTab`, and pass a runtime-only Profiles scroll target when the Runtime card jumps to the Profiles editor.

## Metadata

- **Complexity**: Medium
- **Source PRD**: `docs/prps/prds/unified-desktop-hero-detail-consolidation.prd.md`
- **PRD Phase**: Phase 7 - Overview tab deep-links
- **GitHub Issue**: #472
- **Estimated Files**: 10
- **Research Dispatch**: Enhanced mode requested; 6 standalone researcher lanes completed, with recommendations synthesized locally after the runtime hit the agent thread limit.
- **Worktree Mode**: Disabled by request (`--no-worktree`); no worktree setup section is included.
- **Confidence Score**: 8/10

---

## Storage Boundary & Persistence

| Datum                                              | Classification                                    | Behavior                                                                          |
| -------------------------------------------------- | ------------------------------------------------- | --------------------------------------------------------------------------------- |
| `activeTab` and `profilesScrollTarget`             | Runtime-only React state                          | Lives in `GameDetail`; cleared after the Profiles tab consumes the scroll target. |
| Overview card labels, fallback text, button wiring | Runtime-only UI rendering                         | Derived from existing profile, launch preview, and hook props; not persisted.     |
| Profile data shown in cards                        | Existing TOML-backed profile data, read-only here | This plan reads existing `GameProfile` fields only; no saves are triggered.       |
| Launch preview and health/offline data             | Existing runtime/metadata surfaces                | Reused as already passed into `HeroDetailPanels`; no new cache or schema.         |
| SQLite metadata and app settings                   | None introduced                                   | `metadata/migrations.rs` and `settings.toml` remain untouched.                    |

- **Migration / backward compatibility**: No migration. Profiles without runtime, trainer hooks, or launch preview data render fallback text.
- **Offline behavior**: Fully local UI navigation; existing offline readiness behavior remains unchanged.
- **Degraded fallback**: If a callback is absent in a direct `HeroDetailPanels` render, buttons should be disabled or omitted consistently with the no-op default.
- **User visibility / editability**: Users see new Overview shortcuts; editing still happens only in the Profiles and Launch options tabs.

---

## Batches

Tasks grouped by dependency for parallel execution. Tasks within the same batch can run concurrently; batches run in order.

| Batch | Tasks         | Depends On | Parallel Width |
| ----- | ------------- | ---------- | -------------- |
| B1    | 1.1, 1.2, 1.3 | -          | 3              |
| B2    | 2.1, 2.2, 2.3 | B1         | 3              |
| B3    | 3.1, 3.2, 3.3 | B2         | 3              |
| B4    | 4.1           | B3         | 1              |

- **Total tasks**: 10
- **Total batches**: 4
- **Max parallel width**: 3
- **Same-file collision check**: No batch assigns the same file to two tasks. Shared model/style work lands in B1; component wiring lands in B2; tests land in B3; smoke lands last.

---

## UX Design

### Before

```text
Hero Detail
  [Overview] [Profiles] [Launch options] [Trainer] [History] [Compatibility]

Overview:
  Store metadata
  Health and offline readiness

User reads status, then manually searches for the matching editor tab.
```

### After

```text
Hero Detail
  [Overview] [Profiles] [Launch options] [Trainer] [History] [Compatibility]

Overview:
  Runtime                [Open runtime]
  Active profile         [Open profile]
  Launch command         [Edit launch config]
  Trainer hook           [Manage hooks]
  Store metadata
  Health and offline readiness

Runtime jumps to Profiles and scrolls the editor to Runtime.
Other buttons switch to their target Hero Detail tab.
```

### Interaction Changes

| Touchpoint              | Before                                 | After                                                        | Notes                                                                 |
| ----------------------- | -------------------------------------- | ------------------------------------------------------------ | --------------------------------------------------------------------- |
| Overview dashboard role | Metadata and health/offline cards only | Adds four action cards above the existing cards              | Do not remove current metadata or health/offline cards.               |
| Runtime action          | No direct action                       | `Open runtime` switches to `profiles` and scrolls to Runtime | Only Runtime requires pre-scroll.                                     |
| Active profile action   | No direct action                       | `Open profile` switches to `profiles`                        | Current selected profile/card behavior remains owned by Profiles tab. |
| Launch command action   | No direct action                       | `Edit launch config` switches to `launch-options`            | The launch command section already exists in that tab.                |
| Trainer hook action     | No direct action                       | `Manage hooks` switches to `launch-options`                  | No hook-section pre-scroll is required by issue #472.                 |
| Health panel            | Current card                           | Unchanged                                                    | PRD explicitly leaves health unchanged.                               |

---

## Mandatory Reading

Files that MUST be read before implementing:

| Priority       | File                                                                                       | Lines                     | Why                                                                               |
| -------------- | ------------------------------------------------------------------------------------------ | ------------------------- | --------------------------------------------------------------------------------- |
| P0 (critical)  | `docs/prps/prds/unified-desktop-hero-detail-consolidation.prd.md`                          | 377-386, 484              | Phase 7 scope and the state-driven deep-link decision.                            |
| P0 (critical)  | `src/crosshook-native/src/components/library/HeroDetailPanels.tsx`                         | 17-43, 111-176            | Existing panel prop contract, current overview case, and tab branch switch.       |
| P0 (critical)  | `src/crosshook-native/src/components/library/GameDetail.tsx`                               | 45, 173-220, 243-245      | `activeTab` owner and the current `onSetActiveTab: undefined` TODO.               |
| P0 (critical)  | `src/crosshook-native/src/components/library/HeroDetailProfilesTab.tsx`                    | 20-26, 171-244            | Profiles tab props and editor scroll container location.                          |
| P0 (critical)  | `src/crosshook-native/src/components/library/profiles/HeroProfileEditorSections.tsx`       | 48-59, 137-160            | Runtime section location inside the flattened editor.                             |
| P0 (critical)  | `src/crosshook-native/src/components/library/hero-detail-model.ts`                         | 5-29                      | Stable Hero Detail tab ids and tab test-id helpers.                               |
| P1 (important) | `src/crosshook-native/src/components/library/HeroDetailTabs.tsx`                           | 11-45                     | Controlled Radix tab shell and panel prop spread path.                            |
| P1 (important) | `src/crosshook-native/src/components/layout/DashboardPanelSection.tsx`                     | 53-90                     | Existing card/header/actions composition to mirror if adding action sections.     |
| P1 (important) | `src/crosshook-native/src/hooks/useScrollEnhance.ts`                                       | 8-11, 49-63               | Existing scroll containers; avoid adding unregistered vertical scroll containers. |
| P1 (important) | `src/crosshook-native/src/styles/hero-detail.css`                                          | 202-207, 307-312, 480-621 | Overview grid, card, header row, kv, text, and button-adjacent layout classes.    |
| P1 (important) | `src/crosshook-native/src/components/library/__tests__/HeroDetailPanels.test.tsx`          | 133-226                   | Full-props render factory and no-op default assertions.                           |
| P1 (important) | `src/crosshook-native/src/components/library/__tests__/GameDetail.test.tsx`                | 17-42, 76-125             | Mocked `HeroDetailTabs` prop capture and current tab-state tests.                 |
| P1 (important) | `src/crosshook-native/src/components/library/__tests__/HeroDetailProfilesTab.test.tsx`     | 1-30, 500-590             | Profiles-tab mock strategy and existing user-event patterns.                      |
| P2 (reference) | `src/crosshook-native/src/components/library/__tests__/HeroProfileEditorSections.test.tsx` | 473-486                   | Existing `scrollIntoView` spy pattern.                                            |
| P2 (reference) | `src/crosshook-native/tests/smoke.spec.ts`                                                 | 150-184                   | Browser-dev Hero Detail smoke path to extend.                                     |
| P2 (reference) | `src/crosshook-native/playwright.config.ts`                                                | 54-62                     | Smoke dev server setup and loopback browser dev mode.                             |

## External Documentation

| Topic                | Source | Key Takeaway                                                                            |
| -------------------- | ------ | --------------------------------------------------------------------------------------- |
| External APIs / SDKs | N/A    | No external research needed; this is internal React state and DOM scroll behavior only. |

---

## Patterns to Mirror

Code patterns discovered in the codebase. Follow these exactly.

### NAMING_CONVENTION

```ts
// SOURCE: src/crosshook-native/src/components/library/hero-detail-model.ts:5-10
export type HeroDetailTabId = 'overview' | 'profiles' | 'launch-options' | 'trainer' | 'history' | 'compatibility';
```

```ts
// SOURCE: src/crosshook-native/src/components/library/HeroDetailPanels.tsx:37-38
/** Phase 1 channel: panel-body -> shell request, distinct from `HeroDetailTabs#onActiveTabChange`. */
onSetActiveTab?: (tab: HeroDetailTabId) => void;
```

### ERROR_HANDLING

```tsx
// SOURCE: src/crosshook-native/src/components/library/HeroDetailPanels.tsx:137-140
{
  loadState === 'loading' ? <p className="crosshook-hero-detail__muted">Loading profile details...</p> : null;
}
{
  loadState === 'error' ? (
    <p className="crosshook-hero-detail__warn">{profileError ?? 'Failed to load profile.'}</p>
  ) : null;
}
```

### LOGGING_PATTERN

```text
// SOURCE: current Hero Detail frontend components
No logging is used for tab switching or scroll affordances.
Do not add console logging for this UI-only feature.
```

### REPOSITORY_PATTERN

```text
// SOURCE: src/crosshook-native/src/components/library/GameDetail.tsx:45
const [activeTab, setActiveTab] = useState<HeroDetailTabId>('overview');
// Active tab and pending scroll target are runtime-only React state.
// No repository, IPC, TOML, or SQLite write path is involved.
```

### SERVICE_PATTERN

```tsx
// SOURCE: src/crosshook-native/src/components/library/HeroDetailTabs.tsx:13-16
<Tabs.Root
  className="crosshook-subtabs-root"
  value={activeTab}
  onValueChange={(value) => onActiveTabChange(value as HeroDetailTabId)}
>
```

```tsx
// SOURCE: src/crosshook-native/src/components/library/HeroDetailTabs.tsx:39-40
<div className="crosshook-subtab-content__inner crosshook-subtab-content__inner--scroll crosshook-hero-detail__panel-inner">
  <HeroDetailPanels mode={tab.id} {...panelProps} />
</div>
```

### TEST_STRUCTURE

```tsx
// SOURCE: src/crosshook-native/src/components/library/__tests__/HeroDetailPanels.test.tsx:133-157
function renderHeroDetailPanels(overrides: Partial<HeroDetailPanelsProps> = {}) {
  const props: HeroDetailPanelsProps = {
    mode: 'launch-options',
    ...overrides,
  };
```

```tsx
// SOURCE: src/crosshook-native/src/components/library/__tests__/GameDetail.test.tsx:17-23
const heroDetailTabsSpy = vi.fn<(props: HeroDetailTabsProps) => null>();
vi.mock('../HeroDetailTabs', () => ({
  HeroDetailTabs: (props: HeroDetailTabsProps) => {
    heroDetailTabsSpy(props);
```

### SCROLL_TARGET_PATTERN

```tsx
// SOURCE: src/crosshook-native/src/components/library/profiles/HeroProfileEditorExtras.tsx:113-115
onClick={
  healthIssuesRef?.current
    ? () => healthIssuesRef?.current?.scrollIntoView({ behavior: 'smooth', block: 'start' })
```

```ts
// SOURCE: src/crosshook-native/src/hooks/useScrollEnhance.ts:8-9
export const SCROLL_ENHANCE_SELECTORS =
  '..., .crosshook-hero-detail__body, .crosshook-hero-detail__profiles-editor, ...';
```

### BUTTON_AND_CARD_PATTERN

```tsx
// SOURCE: src/crosshook-native/src/components/layout/DashboardPanelSection.tsx:60-83
<div className={joinClasses('crosshook-dashboard-panel-section__header', headerClassName)}>
  <div className="crosshook-dashboard-panel-section__heading-group">
    ...
  </div>
  {actions ? <div className="crosshook-dashboard-panel-section__actions">{actions}</div> : null}
```

```tsx
// SOURCE: src/crosshook-native/src/components/library/launch/HeroLaunchCommandSection.tsx:222-230
<button
  type="button"
  className="crosshook-button crosshook-button--secondary"
  disabled={!canPreview}
>
```

---

## Files to Change

| File                                                                                   | Action | Justification                                                                                                        |
| -------------------------------------------------------------------------------------- | ------ | -------------------------------------------------------------------------------------------------------------------- |
| `src/crosshook-native/src/components/library/hero-detail-model.ts`                     | UPDATE | Add a small runtime-only Profiles scroll target type and, if useful, a stable Runtime anchor/test id constant.       |
| `src/crosshook-native/src/components/library/GameDetail.tsx`                           | UPDATE | Replace the Phase 7 TODO with a real `onSetActiveTab` handler and pending Profiles scroll-target state.              |
| `src/crosshook-native/src/components/library/HeroDetailPanels.tsx`                     | UPDATE | Destructure/use `onSetActiveTab`; add four Overview action cards while preserving metadata and health/offline cards. |
| `src/crosshook-native/src/components/library/HeroDetailProfilesTab.tsx`                | UPDATE | Accept and consume the pending Profiles scroll target; call `scrollIntoView` only after the Runtime anchor exists.   |
| `src/crosshook-native/src/components/library/profiles/HeroProfileEditorSections.tsx`   | UPDATE | Add a stable wrapper/ref target around the existing `RuntimeSection` without changing Runtime fields.                |
| `src/crosshook-native/src/styles/hero-detail.css`                                      | UPDATE | Add minimal overview action/card wrapping styles only if existing classes do not keep buttons responsive.            |
| `src/crosshook-native/src/components/library/__tests__/HeroDetailPanels.test.tsx`      | UPDATE | Verify Overview button labels and target tab callback mappings.                                                      |
| `src/crosshook-native/src/components/library/__tests__/GameDetail.test.tsx`            | UPDATE | Verify a panel deep-link click changes the controlled active tab and updates the panel callback contract.            |
| `src/crosshook-native/src/components/library/__tests__/HeroDetailProfilesTab.test.tsx` | UPDATE | Verify the Runtime scroll target calls `scrollIntoView` and is consumed.                                             |
| `src/crosshook-native/tests/smoke.spec.ts`                                             | UPDATE | Add browser dev smoke for one Overview deep-link click -> target tab visible/selected.                               |

## NOT Building

- URL hash/query navigation or legacy `/profiles` and `/launch` route navigation; the PRD explicitly chose state-driven `onSetActiveTab`.
- AppRoute shrink, sidebar/palette rewiring, or route deletion; those remain owned by later Hero Detail consolidation phases.
- Hook runtime execution, new host-tool calls, or launch IPC changes; hook execution remains out of scope.
- New npm dependencies or a generic dashboard/deep-link framework; this is a small local UI wiring feature.
- New vertical scroll containers; use the already registered Hero Detail and Profiles editor scroll containers.
- Health panel deep-link behavior; Phase 7 says health is unchanged.

---

## Step-by-Step Tasks

### Task 1.1: Add deep-link target model types - Depends on [none]

- **BATCH**: B1
- **ACTION**: Extend `hero-detail-model.ts` with the small runtime-only target shape needed by Overview deep-links.
- **IMPLEMENT**: Add `HeroDetailProfilesScrollTarget = 'runtime'` and `HeroDetailTabRequestOptions` with optional `profilesScrollTarget`. Add a stable `HERO_DETAIL_RUNTIME_SECTION_TEST_ID = 'hero-detail-runtime-section'` constant so tests and implementation do not duplicate string literals.
- **MIRROR**: `NAMING_CONVENTION`; keep `HeroDetail*` names near the existing tab id/test-id helpers.
- **IMPORTS**: None.
- **GOTCHA**: Do not add route ids or URL/hash helpers. These types describe only in-shell Hero Detail tab requests.
- **VALIDATE**: `cd src/crosshook-native && npm run typecheck`

### Task 1.2: Add Runtime editor anchor - Depends on [none]

- **BATCH**: B1
- **ACTION**: Add a stable Runtime scroll target around the existing `RuntimeSection`.
- **IMPLEMENT**: Update `HeroProfileEditorSectionsProps` with `runtimeSectionRef?: RefObject<HTMLDivElement>`. Wrap the `RuntimeSection` call in a `div` using that ref, `data-testid={HERO_DETAIL_RUNTIME_SECTION_TEST_ID}`, and a lightweight `crosshook-hero-detail__profiles-section-anchor` class; leave `RuntimeSection` props and behavior unchanged.
- **MIRROR**: `SCROLL_TARGET_PATTERN`; this mirrors the existing health-issues scroll target without creating a new scroll container.
- **IMPORTS**: `HERO_DETAIL_RUNTIME_SECTION_TEST_ID` from `../hero-detail-model` or the correct relative path from `components/library/profiles/`.
- **GOTCHA**: Do not move Runtime earlier/later in the editor order. The Phase 4 Profiles editor order remains Identity, Runner method, Runtime, Game, metadata, Media, Trainer, extras.
- **VALIDATE**: `cd src/crosshook-native && npm exec vitest run src/components/library/__tests__/HeroProfileEditorSections.test.tsx`

### Task 1.3: Add minimal Overview action styles - Depends on [none]

- **BATCH**: B1
- **ACTION**: Add only the CSS needed for responsive Overview action rows and anchor spacing.
- **IMPLEMENT**: In `hero-detail.css`, add small classes such as `crosshook-hero-detail__overview-actions` and `crosshook-hero-detail__profiles-section-anchor`. Use `display: flex`, `flex-wrap: wrap`, `gap: 8px`, and `min-width: 0`; do not introduce `overflow-y: auto`.
- **MIRROR**: `BUTTON_AND_CARD_PATTERN`; use existing `crosshook-button`, card, kv, and section typography classes.
- **IMPORTS**: None.
- **GOTCHA**: If a new vertical scroll container becomes necessary, register it in `useScrollEnhance.ts`; the expected implementation should not need one.
- **VALIDATE**: `./scripts/lint.sh --modified --ts`

### Task 2.1: Wire `GameDetail` panel requests - Depends on [1.1]

- **BATCH**: B2
- **ACTION**: Replace the Phase 7 TODO in `GameDetail.tsx` with real tab request handling.
- **IMPLEMENT**: Add `profilesScrollTarget` state and a memoized `handleSetActiveTab(tab, options)` that stores `options?.profilesScrollTarget ?? null` when `tab === 'profiles'`, clears it for other tabs, and calls `setActiveTab(tab)`. Pass `handleSetActiveTab`, `profilesScrollTarget`, and `onProfilesScrollTargetConsumed` through `panelProps`; also replace the raw `setActiveTab` prop passed to `HeroDetailTabs` with a small handler that clears stale scroll targets on manual non-Profiles navigation.
- **MIRROR**: `SERVICE_PATTERN`; `GameDetail` remains the owner of controlled Hero Detail tab state.
- **IMPORTS**: `useCallback`; `HeroDetailProfilesScrollTarget` and `HeroDetailTabRequestOptions` from `hero-detail-model`.
- **GOTCHA**: Every new value referenced inside `panelProps` must appear in the `useMemo` dependency array. Do not leave the old `onSetActiveTab: undefined` assertion path.
- **VALIDATE**: `cd src/crosshook-native && npm exec vitest run src/components/library/__tests__/GameDetail.test.tsx`

### Task 2.2: Consume Profiles Runtime scroll target - Depends on [1.1, 1.2]

- **BATCH**: B2
- **ACTION**: Make `HeroDetailProfilesTab` scroll to the Runtime anchor when asked by Overview.
- **IMPLEMENT**: Add optional props `scrollTarget?: HeroDetailProfilesScrollTarget | null` and `onScrollTargetConsumed?: () => void`. Create `runtimeSectionRef`, pass it into `HeroProfileEditorSections`, and add an effect that calls `runtimeSectionRef.current.scrollIntoView({ behavior: 'smooth', block: 'start' })` only when `scrollTarget === 'runtime'` and the ref exists; call `onScrollTargetConsumed` after a successful scroll.
- **MIRROR**: `SCROLL_TARGET_PATTERN`; use the existing `scrollIntoView` options and existing Profiles editor scroll container.
- **IMPORTS**: `HeroDetailProfilesScrollTarget` from `hero-detail-model`.
- **GOTCHA**: Do not consume/clear the target before the selected profile editor has mounted. If the editor is not ready, keep the target pending for the next render.
- **VALIDATE**: `cd src/crosshook-native && npm exec vitest run src/components/library/__tests__/HeroDetailProfilesTab.test.tsx`

### Task 2.3: Add Overview deep-link cards and buttons - Depends on [1.1, 1.3]

- **BATCH**: B2
- **ACTION**: Update the `overview` branch in `HeroDetailPanels.tsx` to render the four new action cards.
- **IMPLEMENT**: Destructure `onSetActiveTab`. Above the existing `GameDetailsMetadataSection` and `GameDetailsHealthSection`, render Runtime, Active profile, Launch command, and Trainer hook cards using existing `crosshook-hero-detail__section--card`, kv/text, and button classes. Runtime button calls `onSetActiveTab?.('profiles', { profilesScrollTarget: 'runtime' })`; Active profile calls `onSetActiveTab?.('profiles')`; Launch command and Trainer hook call `onSetActiveTab?.('launch-options')`.
- **MIRROR**: `BUTTON_AND_CARD_PATTERN`; use native `button type="button"` controls with accessible labels.
- **IMPORTS**: `HeroDetailTabRequestOptions` if needed for helper typing; reuse `displayPath` for path fallbacks.
- **GOTCHA**: Preserve the current loading/error messages and existing metadata plus health/offline cards. Health gets no new deep-link.
- **VALIDATE**: `cd src/crosshook-native && npm exec vitest run src/components/library/__tests__/HeroDetailPanels.test.tsx`

### Task 3.1: Cover Overview button mappings - Depends on [2.3]

- **BATCH**: B3
- **ACTION**: Add focused `HeroDetailPanels` tests for the Overview deep-link buttons.
- **IMPLEMENT**: Import `userEvent`, render `mode="overview"` with `onSetActiveTab=vi.fn()`, and click `Open runtime`, `Open profile`, `Edit launch config`, and `Manage hooks`. Assert the runtime click includes `{ profilesScrollTarget: 'runtime' }`, the profile click targets `profiles`, both launch/hook clicks target `launch-options`, and existing Store metadata/Health cards still render.
- **MIRROR**: `TEST_STRUCTURE`; use the existing `renderHeroDetailPanels(overrides)` factory and role/name queries.
- **IMPORTS**: `userEvent` from `@testing-library/user-event`; `makeProfileDraft` if richer profile data is needed.
- **GOTCHA**: Avoid callback-only coverage as the only test for the feature; this task verifies mappings, while Task 3.2 verifies controlled tab state changes.
- **VALIDATE**: `cd src/crosshook-native && npm exec vitest run src/components/library/__tests__/HeroDetailPanels.test.tsx`

### Task 3.2: Cover one-click controlled tab change - Depends on [2.1]

- **BATCH**: B3
- **ACTION**: Update `GameDetail.test.tsx` so a mocked Overview deep-link changes the active Hero Detail tab.
- **IMPLEMENT**: Change the panel-contract assertion to expect `onSetActiveTab: expect.any(Function)`. Extend the mocked `HeroDetailTabs` test double with a `button` that calls `props.panelProps.onSetActiveTab?.('profiles', { profilesScrollTarget: 'runtime' })`, click it with `userEvent`, and `waitFor` the latest captured props to show `activeTab: 'profiles'`.
- **MIRROR**: `TEST_STRUCTURE`; this file already captures `HeroDetailTabs` props and verifies tab state changes.
- **IMPORTS**: `waitFor` if not already imported from `@testing-library/react`.
- **GOTCHA**: Keep the existing manual tab switch test for Radix-style tab changes. This new test covers the panel-body channel specifically.
- **VALIDATE**: `cd src/crosshook-native && npm exec vitest run src/components/library/__tests__/GameDetail.test.tsx`

### Task 3.3: Cover Runtime pre-scroll consumption - Depends on [2.2]

- **BATCH**: B3
- **ACTION**: Add Profiles-tab coverage for Runtime anchor scrolling.
- **IMPLEMENT**: Update the `HeroProfileEditorSections` mock in `HeroDetailProfilesTab.test.tsx` to attach `props.runtimeSectionRef` to an element with the Runtime test id. Spy on `window.HTMLElement.prototype.scrollIntoView`, render `HeroDetailProfilesTab` with `scrollTarget="runtime"` and `onScrollTargetConsumed=vi.fn()`, then assert `scrollIntoView({ behavior: 'smooth', block: 'start' })` and the consume callback were called.
- **MIRROR**: `SCROLL_TARGET_PATTERN`; follow the existing `HeroProfileEditorSections.test.tsx` `scrollIntoView` spy style.
- **IMPORTS**: `HERO_DETAIL_RUNTIME_SECTION_TEST_ID` if asserting the test id directly.
- **GOTCHA**: Restore the `scrollIntoView` spy after the test to avoid cross-test pollution in happy-dom.
- **VALIDATE**: `cd src/crosshook-native && npm exec vitest run src/components/library/__tests__/HeroDetailProfilesTab.test.tsx`

### Task 4.1: Add browser smoke for Overview deep-link - Depends on [3.1, 3.2, 3.3]

- **BATCH**: B4
- **ACTION**: Add one Playwright browser-dev smoke test for the user-facing deep-link flow.
- **IMPLEMENT**: In `smoke.spec.ts`, reuse the existing library inspector/Hero Detail setup: load `/?fixture=populated`, open Hero Detail for `Test Game Alpha`, click one Overview deep-link button such as `Edit launch config`, and assert the `Launch options` tab is selected and `hero-detail-launch-tab` is visible. Keep console error capture assertions.
- **MIRROR**: `SMOKE_ROUTE_PATTERN`; use accessible role queries and the existing populated fixture.
- **IMPORTS**: None beyond existing Playwright imports.
- **GOTCHA**: Do not depend on screenshot diffs for this behavioral smoke. The assertion must be click -> target tab state/visible panel.
- **VALIDATE**: `cd src/crosshook-native && npm run test:smoke -- --grep "hero detail"`

---

## Testing Strategy

### Unit and Component Tests

| Test                                   | Input                                                                                                  | Expected Output                                                                    | Edge Case? |
| -------------------------------------- | ------------------------------------------------------------------------------------------------------ | ---------------------------------------------------------------------------------- | ---------- |
| `HeroDetailPanels` Overview mappings   | Render `mode="overview"` with callback spy                                                             | Four buttons render and call the correct tab ids/options                           | No         |
| `HeroDetailPanels` no callback         | Render `mode="overview"` with `onSetActiveTab` omitted                                                 | No thrown error; buttons are disabled or consistently non-actionable               | Yes        |
| `GameDetail` panel deep-link state     | Mock child button calls `panelProps.onSetActiveTab?.('profiles', { profilesScrollTarget: 'runtime' })` | Latest captured `HeroDetailTabs` props show `activeTab: 'profiles'`                | No         |
| `HeroDetailProfilesTab` Runtime scroll | Render with `scrollTarget="runtime"` and mounted runtime ref                                           | `scrollIntoView({ behavior: 'smooth', block: 'start' })` and consume callback fire | No         |
| Browser smoke                          | Populated fixture -> open Hero Detail -> click `Edit launch config`                                    | `Launch options` tab is selected and `hero-detail-launch-tab` is visible           | No         |

### Edge Cases Checklist

- [ ] `onSetActiveTab` omitted in direct component tests.
- [ ] `profile` is `null` or `loadState !== 'ready'`.
- [ ] `launchRequest` or `preview` is `null`.
- [ ] `profile.pre_launch_hooks` and `profile.post_exit_hooks` are missing or empty.
- [ ] Runtime scroll target is set before the profile editor has mounted.
- [ ] User manually switches away from Profiles before/after a pending scroll target.
- [ ] Narrow viewport wraps Overview action buttons without horizontal overflow.

---

## Validation Commands

### Focused Tests

```bash
cd src/crosshook-native && npm exec vitest run \
  src/components/library/__tests__/HeroDetailPanels.test.tsx \
  src/components/library/__tests__/GameDetail.test.tsx \
  src/components/library/__tests__/HeroDetailProfilesTab.test.tsx
```

EXPECT: New Overview deep-link tests, controlled tab-state test, and Runtime pre-scroll test pass.

### Static Analysis

```bash
cd src/crosshook-native && npm run typecheck
```

EXPECT: Zero TypeScript errors.

### Modified Lint

```bash
./scripts/lint.sh --modified --ts
```

EXPECT: Modified TypeScript/CSS files pass Biome and TypeScript checks.

### Browser Smoke

```bash
cd src/crosshook-native && npm run test:smoke -- --grep "hero detail"
```

EXPECT: Hero Detail browser-dev smoke tests pass, including the new Overview deep-link click.

### Full Frontend Suite

```bash
cd src/crosshook-native && npm test
```

EXPECT: Vitest suite passes without regressions.

### Database Validation

```bash
git diff -- src/crosshook-native/crates/crosshook-core/src/metadata/migrations.rs
```

EXPECT: No metadata migration changes.

### Manual Validation

- [ ] Open Hero Detail for a populated game.
- [ ] Confirm Overview shows Runtime, Active profile, Launch command, and Trainer hook cards above Store metadata and Health/offline readiness.
- [ ] Click `Open runtime`; confirm Profiles tab opens and the editor scrolls to Runtime.
- [ ] Return to Overview and click `Open profile`; confirm Profiles tab opens without requiring a route change.
- [ ] Return to Overview and click `Edit launch config`; confirm Launch options opens with the launch command surface.
- [ ] Return to Overview and click `Manage hooks`; confirm Launch options opens and the Pre/post hooks surface remains available in the tab.
- [ ] Confirm Health/offline readiness behavior did not change.

---

## Acceptance Criteria

- [ ] Overview renders accessible deep-link buttons for `Open runtime`, `Open profile`, `Edit launch config`, and `Manage hooks` when the tab-change callback is wired.
- [ ] Clicking `Open runtime` changes the active Hero Detail tab to `profiles` and scrolls/prepositions the Profiles editor to the Runtime section.
- [ ] Clicking `Open profile` changes the active Hero Detail tab to `profiles`.
- [ ] Clicking `Edit launch config` changes the active Hero Detail tab to `launch-options` and exposes the launch command surface.
- [ ] Clicking `Manage hooks` changes the active Hero Detail tab to `launch-options` and leaves the Pre/post hooks surface reachable in that tab.
- [ ] Health overview behavior remains unchanged; no new health deep-link is added.
- [ ] At least one test performs a user click from Overview and asserts the active tab changes; callback-only assertions are not the sole coverage.
- [ ] No route, URL hash, backend, IPC, SQLite, or settings change is introduced.

## Completion Checklist

- [ ] Code follows `HeroDetail*` naming and keeps tab ids `profiles` and `launch-options` stable.
- [ ] `onSetActiveTab` remains the panel-body -> shell request channel and is distinct from `HeroDetailTabs#onActiveTabChange`.
- [ ] `GameDetail` `panelProps` memo dependencies include every new callback/state value.
- [ ] Runtime scroll target is runtime-only state and is consumed only after a successful scroll.
- [ ] Existing Store metadata and Health/offline readiness cards still render on Overview.
- [ ] Overview buttons are real `button type="button"` elements with accessible names.
- [ ] No new vertical scroll container is added without `useScrollEnhance.ts` registration.
- [ ] Focused tests, typecheck, modified lint, smoke, and full frontend suite pass.
- [ ] Package manifests and lockfiles are unchanged.
- [ ] Rust and metadata migration files are unchanged.

## Risks

| Risk                                                                            | Likelihood | Impact | Mitigation                                                                                                    |
| ------------------------------------------------------------------------------- | ---------- | ------ | ------------------------------------------------------------------------------------------------------------- |
| Stale PRD line references cause implementation to edit the wrong overview shape | Medium     | Medium | Use current code: `HeroDetailPanels` overview now contains metadata/health components, not the old kv panels. |
| Runtime scroll target is cleared before the Profiles editor mounts              | Medium     | Medium | Consume the target only after `runtimeSectionRef.current` exists and `scrollIntoView` has been called.        |
| `panelProps` memo misses new dependencies, producing stale callbacks            | Medium     | Medium | Add every new state/callback to the dependency array and cover with `GameDetail.test.tsx`.                    |
| Buttons overflow on narrow/deck widths                                          | Low        | Medium | Add a minimal wrapping action-row class and verify with smoke/manual viewport checks.                         |
| Tests assert only callback calls but not real tab state                         | Medium     | Medium | Add the `GameDetail` one-click controlled-tab test and Playwright smoke.                                      |
| Direct `HeroDetailPanels` renders have no callback                              | Low        | Low    | Keep buttons disabled or otherwise non-actionable when `onSetActiveTab` is absent; no throw.                  |

## Notes

- GitHub issue: https://github.com/yandy-r/crosshook/issues/472
- Issue #472 has no comments as of this planning run; scope comes from the issue body plus PRD Phase 7.
- Enhanced preflight note: the cached YCC script path reported missing plugin-local `ycc/agents`, but the active source bundle at `/home/yandy/Projects/github.com/yandy-r/claude-plugins/ycc` contains `agents/prp-researcher.md` and passes the same preflight check. Planning continued with enhanced research plus local synthesis for the blocked recommendations lane.
- External research: none needed.
- Security research: no significant security risks identified for this UI-only feature.
- Suggested implementation command: `$ycc:prp-implement --parallel --no-worktree docs/prps/plans/github-issue-472-overview-panel-deep-links.plan.md`
