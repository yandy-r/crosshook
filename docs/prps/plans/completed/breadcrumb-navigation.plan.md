# Plan: Breadcrumb navigation (Library → game → edit/launch)

- **Date**: 2026-06-03
- **Source spec**: `docs/prps/specs/breadcrumb-navigation.spec.md`
- **Branch**: `feat/468-launch-hooks-schema` (current checkout — **no worktree**, per user instruction)
- **Mode**: parallel-capable (see `## Batches`)

## Goal

Add a reusable, accessible `Breadcrumb` component rendered in eyebrow slots; wire a derived trail
`Library › {game} › Edit profile|Launch` onto the Profiles/Launch pages when reached from a game's
Hero Detail, and convert Hero Detail's static `Library` eyebrow into a clickable trail. Clicking the
game crumb reopens that game's Hero Detail via a token intent; clicking Library returns to the grid.

## Spec deviations (resolved during research)

1. **ProfilesPage does not render `RouteBanner`** — it renders `ProfilesHero`
   (`components/pages/profiles/ProfilesHero.tsx`), which has **no eyebrow slot**. The spec's
   "RouteBanner trail" approach only fits LaunchPage (`LaunchPage.tsx:83`). Resolution: ProfilesPage
   renders a standalone `<Breadcrumb>` at the top of `.crosshook-profiles-page__body`, **only when an
   origin trail exists**; direct sidebar/palette visits render the page exactly as today (R3, R7).
   This is interim code deleted with consolidation Phase 10 anyway.
2. **No snapshot testing exists in this repo** (zero `toMatchSnapshot` usages). The spec's "RTL
   snapshot guard" is implemented as explicit DOM assertions instead: assert exactly one of
   {static eyebrow `<p>`, `<nav aria-label="Breadcrumb">`} renders per branch.
3. **`: JSX.Element` return annotations** are omitted (codebase convention — no layout component
   annotates returns).

## Key context (verified, with line refs)

### Token-intent pattern to copy — `AppShell.tsx:104-116`

```ts
const handleNavigate = useCallback((nextRoute: AppRoute, options?: AppNavigateOptions) => {
  if (options?.libraryFilter) {
    setActiveLibraryFilter(options.libraryFilter);
    libraryFilterIntentTokenRef.current += 1;
    setLibraryFilterIntent({
      filterKey: options.libraryFilter,
      token: libraryFilterIntentTokenRef.current,
    });
  } else {
    setLibraryFilterIntent(null);
  }
  setRoute(nextRoute);
}, []);
```

- State at `AppShell.tsx:71-74`; intent threaded to `ContentArea` at `:402-404` **and** `:425-427`.
- **GOTCHA**: `<ContentArea>` is rendered **twice** (drawer mode ~`:400`, stack mode ~`:424`) — every
  new prop goes on **both** call sites.
- **GOTCHA**: `handleNavigate` has dep array `[]` — only use stable setters/refs inside it.

### Intent consumption to copy — `LibraryPage.tsx:99-106`

```ts
useEffect(() => {
  if (!libraryFilterIntent) return;
  setPageMode('library');
  setLibraryShellMode('library');
  setFilterKey(libraryFilterIntent.filterKey);
}, [libraryFilterIntent, setLibraryShellMode]);
```

- `handleOpenGameDetail` (`LibraryPage.tsx:187-201`) already early-returns when no summary matches —
  R6's silent drop is built in. The new effect must **gate on summaries being loaded** (they are `[]`
  on first render; effect re-runs when `summaries` changes).
- `handleEdit` (`:178-184`), `handleLaunch` (`:157-175`, navigates at `:169`), props interface
  (`:35-40`), summaries from `useLibrarySummaries` (`:56`).
- **Display name derivation** (no `displayName` field exists): `summary.gameName || summary.name` —
  the inline idiom used in `GameDetail.tsx:87`, `LibraryCard.tsx:78`, `LibraryListRow.tsx:73`.
- **GOTCHA**: `game-details-actions.ts` calls `onBack()` (detail teardown) **before** `onEdit`/
  `onLaunch` — compute origin display name from `summaries`, never from detail state.

### Eyebrow slots

- `RouteBanner.tsx:18`: `<p className="crosshook-route-banner__eyebrow crosshook-heading-eyebrow">{meta.sectionEyebrow}</p>` — props `{ route: AppRoute }` (`:4-6`).
- `HeroDetailHeader.tsx:123`: `<p className="crosshook-hero-detail__eyebrow">Library</p>` inside
  `__title-block` (`:122`); Back button at `:53-55`; props `HeroDetailHeaderProps` (`:6-24`) include
  `displayName: string`, `onBack: () => void`, `summary: LibraryCardData`.
- `crosshook-heading-eyebrow` — `styles/theme.css:669-675`: `0.75rem / 700 / 0.2em / uppercase / var(--crosshook-color-accent-strong)`.
- `crosshook-hero-detail__eyebrow` — `styles/hero-detail.css:39-46`: `0.72rem / 700 / 0.04em / uppercase / var(--crosshook-color-text-subtle)`. **Two different eyebrow scales** — Breadcrumb base
  matches `crosshook-heading-eyebrow`; hero-detail slot uses a `className` passthrough +
  hero-detail.css override.
- No bare text-link button class exists (`crosshook-button--ghost` is a bordered 48px pill — too
  heavy). New `crosshook-breadcrumb__crumb` style required; hover color `var(--crosshook-color-accent)`
  (mirrors `theme.css:1002-1005`); focus handled globally by `focus.css` (covers all `button`).
- Aria pattern to mirror: `LaunchPipeline.tsx:73-119` (`<nav aria-label>` + `<ol>` + `aria-hidden`
  indicators); `aria-current="page"` precedent: `Sidebar.tsx:134`. List reset utility:
  `crosshook-list-reset` (`styles/utilities.css:25-29`).
- CSS home: new `src/styles/breadcrumb.css` imported in `main.tsx` CSS block (lines 5-23) —
  do **not** append to theme.css (138k monolith).

### Page wiring

- `ContentArea.tsx:50-52` renders `<ProfilesPage />` / `<LaunchPage />` with **zero props** today;
  `ContentAreaProps` (`:18-24`) already has `onNavigate`. `InstallPage`/`HealthDashboardPage` show
  the prop-passing precedent (`:55,65`).
- `LaunchPage.tsx:9` imports `RouteBanner`, renders `<RouteBanner route="launch" />` at `:83`.
- `ProfilesPage.tsx` — no props; body opens `.crosshook-profiles-page__body` then `<ProfilesHero …>`.
- Other `RouteBanner` call sites must compile untouched (`trail` optional).

### Testing (conventions)

- Vitest: happy-dom, setup `src/test/setup.ts` (jest-dom, `axe` from jest-axe with color-contrast
  off, global cleanup). Tests in colocated `__tests__/`. Render via `renderWithMocks`
  (`src/test/render.tsx`); pure presentational components may use bare `render` + `vi.fn()`.
- `vi.mock('@/lib/ipc', …)` boilerplate (verbatim, `LibraryPage.test.tsx:16-19`):
  ```ts
  vi.mock('@/lib/ipc', async () => {
    const { mockCallCommand } = await import('@/test/render');
    return { callCommand: mockCallCommand };
  });
  ```
- Intent-prop test pattern: `LibraryPage.test.tsx:225-249` (pass intent on initial render +
  `handlerOverrides` + `waitFor`). Fixture factory: `makeLibraryCardData` (`src/test/fixtures.ts:64-76`).
- **GOTCHA**: tests touching `LibraryPage`/`AppShell` must stub `localStorage` in `beforeEach`
  (`LibraryPage.test.tsx:77-93`, `AppShell.test.tsx:70-86`).
- `AppShell.test.tsx` drives navigation through real UI and asserts observable DOM (no spying on
  `handleNavigate`); stubs `localStorage`, `vi.unstubAllGlobals()` in `afterEach`.
- Playwright smoke: `tests/smoke.spec.ts`, profiles+launch block at `:237-277`; navigate via
  `navigateViaCommandPalette(page, \`Go to ${ROUTE_NAV_LABEL[route]}\`)`(Profiles/Launch are NOT in
the sidebar); hero-detail entry idiom at`:150-167` (`View details for Test Game Alpha`→`getByTestId('game-detail')`); console-capture assertion closes every test; `workers: 1` —
  keep flows sequential and reset any mutated MockStore state.

## Step-by-Step Tasks

> All paths below are relative to `src/crosshook-native/` unless prefixed. Every interim item is
> annotated `// NOTE(hero-detail-consolidation): delete with Phase 10 route removal.` Durable items
> (Breadcrumb, RouteBanner.trail, Hero Detail trail, openGameDetailIntent) are NOT annotated.

### Task 1 — Navigation types

**Files**: `src/types/navigation.ts`
**Depends on**: []

1. Add exported `GameDetailOrigin { profileName: string; displayName: string }` with the
   `NOTE(hero-detail-consolidation)` annotation (interim).
2. Add exported `OpenGameDetailIntent { profileName: string; token: number }` modeled on
   `LibraryFilterIntent` (durable — Phase 9 reuses it).
3. Extend `AppNavigateOptions` with `gameDetailOrigin?: GameDetailOrigin` (annotated interim) and
   `openGameDetail?: string` (durable; profile name).

**Validate**: `npm run typecheck` (from `src/crosshook-native/`).

### Task 2 — Breadcrumb component + styles + tests

**Files**: `src/components/layout/Breadcrumb.tsx` (new), `src/styles/breadcrumb.css` (new),
`src/main.tsx` (one import line), `src/components/layout/__tests__/Breadcrumb.test.tsx` (new)
**Depends on**: []

1. `Breadcrumb.tsx` — pure presentational, named exports, no IPC, no state:
   ```tsx
   export interface BreadcrumbSegment {
     label: string;
     /** Absent = current page: rendered as plain text with aria-current="page". */
     onNavigate?: () => void;
   }
   export interface BreadcrumbProps {
     segments: BreadcrumbSegment[];
     /** Extra class on the <nav> root for slot-specific sizing (e.g. hero detail). */
     className?: string;
   }
   export function Breadcrumb({ segments, className }: BreadcrumbProps) { … }
   ```

   - Markup: `<nav className={'crosshook-breadcrumb' + optional className} aria-label="Breadcrumb">`
     → `<ol className="crosshook-breadcrumb__list crosshook-list-reset">` → one `<li
className="crosshook-breadcrumb__item">` per segment.
   - Segment with `onNavigate`: `<button type="button" className="crosshook-breadcrumb__crumb"
onClick={onNavigate}>{label}</button>`. Terminal/no-callback segment: `<span
className="crosshook-breadcrumb__current" aria-current="page">{label}</span>`.
   - Separator: `<span className="crosshook-breadcrumb__separator" aria-hidden="true">›</span>`
     rendered inside each `<li>` **after the first** (before the crumb), mirroring
     `LaunchPipeline.tsx` aria-hidden indicators.
2. `src/styles/breadcrumb.css` — base sized to `crosshook-heading-eyebrow` metrics
   (`theme.css:669-675`): `font-size: 0.75rem; font-weight: 700; letter-spacing: 0.2em;
text-transform: uppercase; color: var(--crosshook-color-accent-strong);` on the nav; `__list`
   as inline flex with small gap; `__crumb` as a bare text button (transparent background, no
   border, padding 0, inherits font, `cursor: pointer`, hover `color: var(--crosshook-color-accent)`,
   transition `var(--crosshook-transition-fast)`); `__current` and `__separator` inherit; separator
   `color: var(--crosshook-color-text-subtle)`. Tokens only — no literals.
3. `src/main.tsx` — add `import './styles/breadcrumb.css';` to the CSS block (lines 5-23).
4. `Breadcrumb.test.tsx` (bare `render` is fine — no IPC):
   - renders segments in order inside `nav[aria-label="Breadcrumb"]` > `ol` > `li`;
   - terminal segment has `aria-current="page"` and is **not** a button;
   - separators are `aria-hidden` and there are `segments.length - 1` of them;
   - clicking a crumb fires its `onNavigate` exactly once;
   - axe pass: `const results = await axe(container); expect(results).toHaveNoViolations();`
     (import `axe` from `@/test/setup`).

**Validate**: `npm run typecheck && npm test -- Breadcrumb`.

### Task 3 — RouteBanner `trail` prop + tests

**Files**: `src/components/layout/RouteBanner.tsx`,
`src/components/layout/__tests__/RouteBanner.test.tsx` (new)
**Depends on**: [Task 2]

1. `RouteBannerProps` gains `trail?: BreadcrumbSegment[]`. In the render, replace the eyebrow line:
   `trail && trail.length > 0 ? <Breadcrumb segments={trail} className="crosshook-route-banner__eyebrow" /> : <p …existing line 18 unchanged…>`.
   All existing call sites (`LibraryPage.tsx:314`, `LaunchPage.tsx:83`, any others — grep before
   editing) compile untouched.
2. `RouteBanner.test.tsx` (explicit-DOM guard, replaces spec's "snapshot"):
   - without `trail`: static eyebrow text from `ROUTE_METADATA` present; `queryByRole('navigation', { name: 'Breadcrumb' })` absent;
   - with `trail`: breadcrumb nav present; static eyebrow `<p>` absent; title/summary unchanged.

**Validate**: `npm run typecheck && npm test -- RouteBanner`.

### Task 4 — HeroDetailHeader trail (durable) + tests

**Files**: `src/components/library/HeroDetailHeader.tsx`, `src/styles/hero-detail.css`,
`src/components/library/__tests__/HeroDetailHeader.test.tsx` (new)
**Depends on**: [Task 2]

1. Replace `HeroDetailHeader.tsx:123` `<p className="crosshook-hero-detail__eyebrow">Library</p>`
   with:
   ```tsx
   <Breadcrumb
     className="crosshook-hero-detail__breadcrumb"
     segments={[{ label: 'Library', onNavigate: onBack }, { label: displayName }]}
   />
   ```
   Back button (`:53-55`) untouched.
2. `hero-detail.css` — add `.crosshook-hero-detail__breadcrumb` override next to the old
   `__eyebrow` rule (`:39-46`) matching its metrics (`font-size: 0.72rem; letter-spacing: 0.04em;
color: var(--crosshook-color-text-subtle);` margin reset). Keep the now-unused
   `.crosshook-hero-detail__eyebrow` rule **removed** only if no other usage exists (grep
   `crosshook-hero-detail__eyebrow` first; remove rule + class if orphaned — no dead code).
3. `HeroDetailHeader.test.tsx` — build `summary` with `makeLibraryCardData`; render with `vi.fn()`
   handlers (check existing props in `HeroDetailHeaderProps:6-24` for required ones):
   - Library crumb is a button; clicking fires `onBack` once;
   - game `displayName` is the terminal segment with `aria-current="page"`, not a button;
   - the standalone `Back` button still renders and fires `onBack`.

**Validate**: `npm run typecheck && npm test -- HeroDetailHeader`.

### Task 5 — LibraryPage: origin on edit/launch + intent consumption + tests

**Files**: `src/components/pages/LibraryPage.tsx`,
`src/components/pages/__tests__/LibraryPage.test.tsx`
**Depends on**: [Task 1]

1. Props: add `openGameDetailIntent?: OpenGameDetailIntent | null` to `LibraryPageProps` (`:35-40`).
2. `handleEdit` (`:178-184`) and `handleLaunch` (`:157-175`): before navigating, derive
   `const card = summaries.find((s) => s.name === name);` and pass
   `onNavigate?.('profiles' /* or 'launch' */, { gameDetailOrigin: { profileName: name, displayName: card ? card.gameName || card.name : name } })`.
   Annotate the option-passing lines `// NOTE(hero-detail-consolidation): delete with Phase 10 route removal.`
   (The `onBack`-before-`onEdit` ordering in `game-details-actions.ts` is safe — `summaries` is
   unaffected by detail teardown.)
3. Intent consumption (durable) — new effect mirroring `:99-106`:
   ```ts
   useEffect(() => {
     if (!openGameDetailIntent) return;
     if (!summaries.some((s) => s.name === openGameDetailIntent.profileName)) return; // R6: drop silently
     void handleOpenGameDetail(openGameDetailIntent.profileName);
   }, [openGameDetailIntent, handleOpenGameDetail]);
   ```
   (`handleOpenGameDetail` re-creates when `summaries` changes, so the effect re-runs once
   summaries load; no separate `summaries` dep needed. Verify lint accepts the dep array.)
4. Tests (extend existing harness; thread the new prop through `renderLibraryHarness` /
   `LibraryPageWithInspector`; keep `localStorage` stubs):
   - edit action calls `onNavigate` with `('profiles', { gameDetailOrigin: { profileName, displayName } })`;
     same for launch → `'launch'` (provide an `onNavigate` `vi.fn()` to the harness);
   - `openGameDetailIntent` for an existing profile opens detail once summaries load
     (`waitFor(() => getByTestId('game-detail'))`);
   - intent for an unknown profile: summaries settle, `queryByTestId('game-detail')` stays absent.

**Validate**: `npm run typecheck && npm test -- LibraryPage`.

### Task 6 — AppShell origin/intent state + ContentArea threading + Profiles/Launch trails + tests

**Files**: `src/components/layout/AppShell.tsx`, `src/components/layout/ContentArea.tsx`,
`src/components/pages/LaunchPage.tsx`, `src/components/pages/ProfilesPage.tsx`,
`src/components/layout/game-detail-trail.ts` (new, interim),
`src/components/layout/__tests__/AppShell.test.tsx`
**Depends on**: [Task 1, Task 2, Task 3, Task 5]

1. **AppShell** (mirror `:71-74` / `:104-116` exactly):
   - State: `const [gameDetailOrigin, setGameDetailOrigin] = useState<GameDetailOrigin | null>(null);`
     (annotated interim) and `const [openGameDetailIntent, setOpenGameDetailIntent] = useState<OpenGameDetailIntent | null>(null);`
     - `const openGameDetailIntentTokenRef = useRef(0);` (durable).
   - In `handleNavigate`: set-or-clear `gameDetailOrigin` from `options?.gameDetailOrigin`
     (else-branch clears — guarantees R3); convert `options?.openGameDetail` into a token intent
     exactly like `libraryFilter` (increment ref, set `{ profileName, token }`, else clear).
     Dep array stays `[]` (setters/refs only).
   - Thread to **both** `<ContentArea>` instances (~`:400` and ~`:424`):
     `gameDetailOrigin={gameDetailOrigin}` and `openGameDetailIntent={openGameDetailIntent}`.
2. **ContentArea**: `ContentAreaProps` gains `gameDetailOrigin?: GameDetailOrigin | null`
   (annotated interim) and `openGameDetailIntent?: OpenGameDetailIntent | null`. `renderPage()`:
   pass `openGameDetailIntent` to `<LibraryPage …>`; pass
   `origin={gameDetailOrigin} onNavigate={onNavigate}` to `<ProfilesPage>` and `<LaunchPage>`
   (annotated interim on those two lines).
3. **`game-detail-trail.ts`** (new, whole file annotated interim) — DRY helper shared by both pages:
   ```ts
   export function buildGameDetailTrail(
     origin: GameDetailOrigin | null | undefined,
     onNavigate: ((route: AppRoute, options?: AppNavigateOptions) => void) | undefined,
     terminalLabel: 'Edit profile' | 'Launch'
   ): BreadcrumbSegment[] | undefined;
   ```
   Returns `undefined` when `origin` or `onNavigate` is missing; else
   `[{ label: 'Library', onNavigate: () => onNavigate('library') }, { label: origin.displayName, onNavigate: () => onNavigate('library', { openGameDetail: origin.profileName }) }, { label: terminalLabel }]`.
4. **LaunchPage**: add props interface `{ origin?: GameDetailOrigin | null; onNavigate?: (route: AppRoute, options?: AppNavigateOptions) => void }`
   (annotated interim); `<RouteBanner route="launch" trail={buildGameDetailTrail(origin, onNavigate, 'Launch')} />`.
5. **ProfilesPage**: same props interface (annotated interim). At the top of
   `.crosshook-profiles-page__body`, before `<ProfilesHero …>`, conditionally render:
   ```tsx
   {
     trail ? <Breadcrumb segments={trail} /> : null;
   }
   ```
   with `const trail = buildGameDetailTrail(origin, onNavigate, 'Edit profile');` — page is
   byte-identical when no origin (R3/R7). (Spec deviation #1.)
6. **AppShell.test.tsx** additions (existing harness; keep `localStorage` stubs + real-UI driving):
   - open `Test Game Alpha` detail → click `Edit profile` → assert
     `getByRole('navigation', { name: 'Breadcrumb' })` visible and contains `Library`, the game
     name, and `Edit profile`;
   - click the game-name crumb → `waitFor` `getByTestId('game-detail')` visible again (intent
     round-trip);
   - navigate to Profiles via command palette (plain navigation) → breadcrumb nav absent (origin
     cleared, R3).

**Validate**: `npm run typecheck && npm test -- AppShell`.

### Task 7 — Playwright smoke flow

**Files**: `tests/smoke.spec.ts`
**Depends on**: [Task 6]

1. Add one test to the `profiles + launch panel landing smoke` describe block (`:237-277`):
   - open library → `getByRole('button', { name: 'View details for Test Game Alpha' }).click()` →
     `expect(getByTestId('game-detail')).toBeVisible()`;
   - click the `Edit profile` action → assert
     `page.getByRole('navigation', { name: 'Breadcrumb' })` visible and showing
     `Library › Test Game Alpha › Edit profile`;
   - click the `Test Game Alpha` crumb → `expect(getByTestId('game-detail')).toBeVisible()`;
   - close with the standard console-capture assertion (`expect(capture.errors).toEqual([])`).
2. Follow the block's existing style (command-palette nav helpers, no new fixtures, no MockStore
   mutation; if any state is mutated, reset it like `smoke.spec.ts:217`).

**Validate**: `npm run test:smoke` (requires `npm run test:smoke:install` once).

## Batches

| Batch | Tasks                  | Rationale                                                         |
| ----- | ---------------------- | ----------------------------------------------------------------- |
| 1     | Task 1, Task 2         | Independent foundations; disjoint files                           |
| 2     | Task 3, Task 4, Task 5 | Consumers of Batch 1; pairwise-disjoint files                     |
| 3     | Task 6                 | Touches AppShell/ContentArea/both pages — single integration task |
| 4     | Task 7                 | End-to-end smoke over the finished wiring                         |

Between batches: `npm run typecheck && npm test` from `src/crosshook-native/`.

## Validation Levels

All from `src/crosshook-native/` unless noted:

1. **Static**: `npm run typecheck` && `npm run lint` (root `./scripts/lint.sh` for the full sweep)
2. **Unit**: `npm test`
3. **Build**: `npm run build` (vite build; or `./scripts/build-native.sh --binary-only` if Rust touched — it is not)
4. **Integration**: `npm run test:smoke`
5. **Edge cases**: covered in-suite — unknown-profile intent drop (R6), origin cleared on plain
   navigation (R3), trail-less RouteBanner unchanged (R7), axe pass on Breadcrumb (R5)

## Acceptance Criteria (from spec)

- R1–R7 as written in the spec, with deviation #1 (ProfilesPage standalone breadcrumb instead of a
  RouteBanner eyebrow swap) documented above.
- All interim code carries `NOTE(hero-detail-consolidation)` markers (greppable for Phase 10).
- No persisted data changes; SQLite schema stays v23; no `useScrollEnhance` changes.

## Report

After completion, write `docs/prps/reports/breadcrumb-navigation.report.md` and archive this plan
to `docs/prps/plans/archive/`.
