# Implementation Report: Breadcrumb navigation (Library → game → edit/launch)

- **Date**: 2026-06-03
- **Plan**: `docs/prps/plans/completed/breadcrumb-navigation.plan.md`
- **Spec**: `docs/prps/specs/breadcrumb-navigation.spec.md`
- **Branch**: `feat/468-launch-hooks-schema` (current checkout, no worktree — per user instruction)
- **Mode**: parallel sub-agents (`/ycc:prp-implement --parallel --no-worktree`), 4 batches, 7 tasks

## Summary

Added a reusable, accessible `Breadcrumb` component and wired it three ways:

1. **Hero Detail header (durable)** — the hardcoded `Library` eyebrow is now a trail:
   `Library` (clickable → `onBack`) `› {game}` (current, `aria-current="page"`). Back button retained.
2. **Profiles/Launch interim trails** — reaching either page via a game's Edit profile / Launch
   action shows `Library › {game} › Edit profile|Launch`; the game crumb reopens that game's Hero
   Detail via a new `openGameDetailIntent` token (mirrors `LibraryFilterIntent`); the Library crumb
   returns to the grid. Direct sidebar/palette visits are unchanged (origin cleared on every
   navigation that doesn't set it — R3).
3. **`RouteBanner.trail` prop (durable)** — optional; all 11 existing call sites untouched.

All interim code is annotated `// NOTE(hero-detail-consolidation): delete with Phase 10 route removal.`
(greppable: `types/navigation.ts`, `AppShell.tsx`, `ContentArea.tsx`, `LibraryPage.tsx`,
`ProfilesPage.tsx`, `LaunchPage.tsx`, `game-detail-trail.ts`). Durable pieces (`Breadcrumb`,
`RouteBanner.trail`, Hero Detail trail, `OpenGameDetailIntent`) carry no markers — they survive the
consolidation PRD (Phase 9 reuses the intent mechanism).

## Files changed (13 files, +379 / −23)

| File                                          | Change                                                                                                                                                                  |
| --------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `src/components/layout/Breadcrumb.tsx`        | **New** — pure presentational `<nav aria-label="Breadcrumb">` + `<ol>`; ghost-text crumb buttons; `aria-hidden` `›` separators; `className` passthrough for slot sizing |
| `src/styles/breadcrumb.css`                   | **New** — base matches `crosshook-heading-eyebrow` metrics; tokens only                                                                                                 |
| `src/main.tsx`                                | +1 CSS import                                                                                                                                                           |
| `src/types/navigation.ts`                     | `GameDetailOrigin` (interim), `OpenGameDetailIntent` (durable), `AppNavigateOptions.gameDetailOrigin/openGameDetail`                                                    |
| `src/components/layout/RouteBanner.tsx`       | Optional `trail` prop; breadcrumb replaces static eyebrow when present                                                                                                  |
| `src/components/library/HeroDetailHeader.tsx` | Eyebrow → `Breadcrumb`; Back button untouched                                                                                                                           |
| `src/styles/hero-detail.css`                  | Orphaned `__eyebrow` rule removed; `__breadcrumb` override (0.72rem/0.04em/text-subtle) keeps hero slot visually identical                                              |
| `src/components/pages/LibraryPage.tsx`        | Origin passed on edit/launch (from `summaries`, `gameName \|\| name`); `openGameDetailIntent` consumption effect with R6 silent drop                                    |
| `src/components/layout/AppShell.tsx`          | Origin set-or-clear + intent token conversion in `handleNavigate` (dep array still `[]`); both `<ContentArea>` instances threaded                                       |
| `src/components/layout/ContentArea.tsx`       | Props threaded to LibraryPage/ProfilesPage/LaunchPage                                                                                                                   |
| `src/components/layout/game-detail-trail.ts`  | **New** (interim) — shared `buildGameDetailTrail` helper (DRY across both pages)                                                                                        |
| `src/components/pages/LaunchPage.tsx`         | `RouteBanner trail={…}`                                                                                                                                                 |
| `src/components/pages/ProfilesPage.tsx`       | Standalone `<Breadcrumb>` above `ProfilesHero` when trail exists (see deviations)                                                                                       |

Tests: `Breadcrumb.test.tsx` (new, 5), `RouteBanner.test.tsx` (new, 8), `HeroDetailHeader.test.tsx`
(new, 4), `LibraryPage.test.tsx` (+4), `AppShell.test.tsx` (+3), `tests/smoke.spec.ts` (+1 flow).

## Spec deviations

1. **ProfilesPage has no `RouteBanner`** — it renders `ProfilesHero` (no eyebrow slot). The interim
   trail is a standalone `<Breadcrumb>` at the top of `.crosshook-profiles-page__body`, rendered
   only when an origin exists; the page is byte-identical otherwise (R3/R7). LaunchPage uses the
   specced `RouteBanner.trail`.
2. **No snapshots** — the repo has zero snapshot tests; the spec's "RTL snapshot guard" became
   explicit DOM assertions (exactly one of {static eyebrow, breadcrumb nav} per branch).
3. **`AppShell.test.tsx` gained the standard `vi.mock('@/lib/ipc')` boilerplate** — previously its
   tests bypassed `handlerOverrides`; all 17 pre-existing tests verified passing after the change.
   Also adopted the documented `routes.a11y.test.tsx` empty-profile-list workaround for the
   happy-dom datalist crash.

## Validation results

All from `src/crosshook-native/`:

| Level         | Command                           | Result                                                                                                                                  |
| ------------- | --------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------- |
| 1 Static      | `npm run typecheck`               | **PASS**                                                                                                                                |
| 1 Static      | `npm run lint` (Biome, 460 files) | **PASS** — 0 findings from this change (7 pre-existing warnings in unrelated files)                                                     |
| 2 Unit        | `npm test`                        | **PASS** — 229/229 (was 210 before; +19 new)                                                                                            |
| 3 Build       | `npm run build`                   | **PASS** (pre-existing chunk-size warning only)                                                                                         |
| 4 Integration | `npm run test:smoke`              | **PASS** — 75/75 incl. new breadcrumb round-trip flow                                                                                   |
| 5 Edge cases  | in-suite                          | R6 unknown-profile intent drop; R3 origin clear on plain navigation; R7 trail-less `RouteBanner` unchanged; R5 axe pass on `Breadcrumb` |

## Known issues (pre-existing, out of scope)

- **`ProfilesRoute.test.tsx` is flaky** (~1-in-3 failures, varying tests: `(b) auto-load-profile…`,
  `(d) ProtonDB section…`). Reproduced at the same rate on a clean baseline with all breadcrumb
  changes stashed — not introduced by this work. Worth a follow-up issue.

## Storage boundary (confirmed)

Runtime-only state exclusively (`gameDetailOrigin`, `openGameDetailIntent` in AppShell memory).
No TOML, no SQLite changes — schema stays v23. No `useScrollEnhance` changes (no new scroll container).
