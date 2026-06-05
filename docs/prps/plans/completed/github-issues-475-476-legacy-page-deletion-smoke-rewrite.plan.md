# Plan: Legacy Page Deletion and Smoke Rewrite (Issues #475 and #476)

## Summary

Physically remove the legacy `ProfilesPage`/`LaunchPage` modules left behind by PR #495 (PRD Phases 8–9) and lock the new shape in with Playwright smoke coverage. Six modules inside `pages/profiles/` and `pages/launch/` are **shared with live Hero Detail code and must be relocated first** — a naive `git rm -r` (as issue #475's body suggests) breaks `npm run typecheck`, `npm test`, and `./scripts/lint.sh`. The smoke work is far smaller than issue #476 implies: #495 already trimmed `ROUTE_ORDER`, already rewrote the pipeline/panel/console-chrome blocks against Hero Detail, and already shipped the Favorites + Currently Playing sidebar tests. The genuine smoke gaps are the appRoute regression guard, the profile-card-switch step in the Hero Detail flow (which needs a dev-mock fixture affordance), one toolbar-chip assertion, and pruning two dead CSS selectors from the route sweep.

## User Story

As a maintainer, I want no dead-route code and smoke coverage that fails loud if anyone reintroduces `/profiles`/`/launch` or breaks the Hero Detail edit/launch path, so the codebase doesn't carry parallel UIs for the same job.

## Problem -> Solution

Orphaned-but-compiling legacy page modules + smoke gaps -> relocate the 6 shared modules into `components/library/`, delete the 13 dead files, and add the regression-guard + card-switch smoke coverage in the same PR.

## Metadata

- **Complexity**: Medium-Large (large negative diff, small positive diff)
- **Source PRD**: `docs/prps/prds/unified-desktop-hero-detail-consolidation.prd.md`
- **PRD Phase**: Phase 10 (`#475`) plus Phase 11 (`#476`)
- **GitHub Issues**: #475, #476 (tracker #478)
- **Estimated Files**: ~34 (13 deleted, 4 moved, 3 added, ~14 edited)
- **Research Dispatch**: `--parallel --enhanced` — 7 standalone researchers (api / business / tech / ux / security / practices / recommendations); the security researcher failed mid-run (API error) and its IPC-orphan + tracker-issue gaps were closed by direct orchestrator verification
- **Worktree Mode**: Disabled by request (`--no-worktree`)
- **Confidence Score**: 9/10

---

## Storage Boundary & Persistence

**This work adds and changes ZERO persisted data.** All deltas are deletion of UI/test code plus test-only additions.

| Datum                                                            | Classification              | Behavior                                                                                                                                  |
| ---------------------------------------------------------------- | --------------------------- | ----------------------------------------------------------------------------------------------------------------------------------------- |
| Profile TOML (`profile.toml`)                                    | TOML settings (unchanged)   | Not touched. `pre_launch_hooks`/`post_exit_hooks` schema from Phase 3 already shipped; untouched here.                                    |
| SQLite metadata (`~/.local/share/crosshook/metadata.db`)         | SQLite metadata (unchanged) | Schema stays at version 23. `migrations.rs` not touched.                                                                                  |
| Hero Detail active tab, library `filterKey`, running-profile set | Runtime-only (unchanged)    | All already exist as ephemeral React state from prior phases (`HeroDetailTabId`, `LibraryFilterKey`, `useRunningProfiles`).               |
| Dev-mock seeded profile variant (new smoke fixture)              | Runtime-only (test-only)    | Lives in the browser-dev-mode mock store (`src/lib/mocks/`) for the duration of one Playwright test; never reaches production code paths. |

- **Migration / backward compatibility**: N/A — no persisted schema touched; existing profiles, collections, and metadata load unchanged.
- **Offline behavior**: unchanged — fully offline; deleting frontend modules has no network dependency.
- **Degraded fallback**: unchanged — the deleted routes were already unreachable post-#495 (no sidebar entry, no palette command, no nav callers). All surviving flows keep their existing error-toast/empty-state fallbacks.
- **User visibility/editability**: every capability the deleted pages offered remains user-visible and editable inside Library Hero Detail (Profiles tab, Launch options tab).

---

## Batches

| Batch | Tasks              | Depends On | Parallel Width |
| ----- | ------------------ | ---------- | -------------- |
| B1    | 1.1, 1.2, 1.3, 1.4 | -          | 4              |
| B2    | 2.1                | B1         | 1              |
| B3    | 3.1                | B2         | 1              |
| B4    | 4.1                | B3         | 1              |

- **Total tasks**: 7
- **Total batches**: 4
- **Max parallel width**: 4
- **Same-file collision check**: PASS — B1 task file sets are pairwise disjoint: 1.1 `{library/launch/*, HeroLaunchGate.tsx, HeroLaunchGate.test.tsx, library/__tests__/useLaunchDepGate.test.tsx}`, 1.2 `{library/profiles/useProfilesPageProton.ts, HeroDetailProfilesTab.tsx, HeroDetailProfilesTab.test.tsx, hooks/profile/communityExport.ts, hooks/profile/useProfileActions.ts, library/__tests__/useProfilesPageProton.test.tsx, pages/profiles/{utils,constants}.ts}`, 1.3 `{library/profiles/ProfilesOverlays.tsx, library/profiles/HeroProfileActionsBar.tsx}`, 1.4 `{__tests__/a11y/routes.a11y.test.tsx, layout/__tests__/AppShell.test.tsx}`. B2–B4 are single-task batches. Tasks run in one shared checkout (no worktree), so this disjointness is mandatory, not advisory.
- **Green gate between batches**: `npm run typecheck && npm test` must pass at the end of every batch. B2's `git rm` is the safety net — `tsc` fails loudly if any importer was missed in B1.

---

## UX Design

### Before

```
src/components/pages/
├── ProfilesPage.tsx        (336 ln — unrouted since #495, compiles, dead)
├── LaunchPage.tsx          (137 ln — unrouted, inlined legacy banner JSX)
├── profiles/  (9 files — 3 SHARED with Hero Detail, 6 dead)
├── launch/    (4 files — 2 SHARED with Hero Detail, 2 dead)
└── __tests__/{ProfilesRoute,LaunchRoute}.test.tsx  (dead-route RTL tests)

smoke.spec.ts: sweep selector still lists .crosshook-profiles-page__body /
.crosshook-launch-page__grid; no guard against route resurrection; Hero Detail
flow never switches profile cards (mock fixture has 1 profile per game).
```

### After

```
src/components/library/
├── launch/{useLaunchDepGate.ts, LaunchDepGateModal.tsx}   (relocated, beside HeroLaunchGate)
├── profiles/{useProfilesPageProton.ts, ProfilesOverlays.tsx}  (relocated, beside consumers)
src/hooks/profile/communityExport.ts                       (split out of pages/profiles/utils.ts)

pages/ProfilesPage.tsx, LaunchPage.tsx, pages/profiles/, pages/launch/,
ProfilesRoute.test.tsx, LaunchRoute.test.tsx               → deleted (−~2,500 LOC)

smoke.spec.ts: appRoute regression guard (sidebar exposes no Profiles/Launch tab);
Hero Detail flow covers second-card switch + hero Launch button + aria-current
stays on Library; toolbar Favorites chip asserted; dead selectors pruned.
```

### Interaction Changes

| Touchpoint                     | Before                                                         | After                                                                | Notes                                                               |
| ------------------------------ | -------------------------------------------------------------- | -------------------------------------------------------------------- | ------------------------------------------------------------------- |
| User-visible UI                | `/profiles` and `/launch` already unreachable (post-#495)      | Identical — zero user-visible change                                 | This PR is deletion + test coverage; the UX change shipped in #495. |
| Smoke: route resurrection      | Nothing fails if someone re-adds a Profiles/Launch sidebar tab | `appRoute regression guard` fails the suite                          | PRD Success Metric L48.                                             |
| Smoke: Hero Detail edit/launch | Single-profile flow; in-tab "Launch Game" button only          | Two-card switch + active pill + hero-header "Launch" + log assertion | PRD Phase 11 step 4; `aria-current` stays `library` (Metric L53).   |
| Smoke: sidebar Favorites       | Sidebar button `aria-pressed` asserted                         | Plus toolbar Favorites chip `aria-pressed="true"`                    | PRD Success Metric L55 wording targets the chip.                    |

---

## Mandatory Reading

| Priority | File                                                                               | Lines                                              | Why                                                                                                                     |
| -------- | ---------------------------------------------------------------------------------- | -------------------------------------------------- | ----------------------------------------------------------------------------------------------------------------------- |
| P0       | `docs/prps/prds/unified-desktop-hero-detail-consolidation.prd.md`                  | 269–291, 415–445                                   | Phase table + Phase 10/11 details. **All line refs inside Phase 11 are stale (pre-#495)** — trust this plan instead.    |
| P0       | `docs/prps/reports/github-issues-473-474-route-removal-nav-rewire-report.md`       | all (62 ln)                                        | What #495 actually did; "kept legacy page files type-clean" deviation; validation-command precedent.                    |
| P0       | `src/crosshook-native/src/components/library/launch/HeroLaunchGate.tsx`            | 22–23, 92, 217, 236                                | Surviving consumer of the dep-gate pair; import lines to repoint.                                                       |
| P0       | `src/crosshook-native/src/components/library/HeroDetailProfilesTab.tsx`            | 14, 87, 192–211                                    | Surviving consumer of `useProfilesPageProton`; card-list/editor markup the smoke test asserts.                          |
| P0       | `src/crosshook-native/src/components/library/profiles/HeroProfileActionsBar.tsx`   | 31, 243                                            | Surviving consumer of `ProfilesOverlays`.                                                                               |
| P0       | `src/crosshook-native/src/hooks/profile/useProfileActions.ts`                      | 21                                                 | Surviving consumer of `suggestedCommunityExportFilename`.                                                               |
| P0       | `src/crosshook-native/src/__tests__/a11y/routes.a11y.test.tsx`                     | 10, 12, 92–104, 159–213, 252–278                   | Surviving test that hard-imports both deleted pages — **not listed in issue #475's scope**; must be edited.             |
| P1       | `src/crosshook-native/tests/smoke.spec.ts`                                         | 40–50, 106–110, 215–248, 250–262, 264–331, 373–427 | Current (post-#495) smoke shape; what already exists vs. the genuine gaps.                                              |
| P1       | `src/crosshook-native/tests/navigation-helpers.ts`                                 | 16–57                                              | `openLibraryHeroDetail`, `openHeroDetailTab`, `waitForCrosshookDevIpc`, `seedMockProfileRunning` — reuse, don't inline. |
| P1       | `src/crosshook-native/src/lib/mocks/handlers/launch.ts`                            | 415–422, 465–483                                   | `_mock_set_profile_running` precedent for the new `_mock_add_profile` dev helper.                                       |
| P1       | `src/crosshook-native/src/lib/mocks/handlers/profile-core.ts`                      | 20–56, 85, 94–102                                  | `DEMO_PROFILE_SEEDS` shape to mirror; `gameName` derivation (`profile.game.name`); favorites handler.                   |
| P1       | `src/crosshook-native/src/components/pages/__tests__/LaunchRoute.test.tsx`         | 287–389                                            | Port source: dep-gate silent-catch + `prefix-dep-complete` no-op behavioral tests (surviving logic, dying test file).   |
| P1       | `src/crosshook-native/src/components/pages/__tests__/ProfilesRoute.test.tsx`       | 226–279                                            | Port source: ProtonDB fetch/sort/suggestion coverage for the relocated proton hook.                                     |
| P2       | `src/crosshook-native/src/components/layout/Sidebar.tsx`                           | 81–123, 129–168, 253                               | Ground truth: routes are `role="tab"`, quick-filters are `<button aria-pressed>`, sidebar testid.                       |
| P2       | `src/crosshook-native/src/components/layout/__tests__/AppShell.test.tsx`           | 19–40, 170–201, 495–516                            | `NO_DATALIST_OVERRIDES` + two `NOTE(hero-detail-consolidation): delete with Phase 10` markers; keep the palette guard.  |
| P2       | `src/crosshook-native/src/styles/theme.css`                                        | 208–215, 350–359, 530–538                          | Orphan CSS rule ranges (verify each class with grep before deleting).                                                   |
| P2       | `docs/prps/plans/completed/github-issues-473-474-route-removal-nav-rewire.plan.md` | skim                                               | Format + validation-ladder reference (same PRD, previous phase pair).                                                   |

## External Documentation

| Resource                        | URL                                                                    | Why                                                                                                                                      |
| ------------------------------- | ---------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------- |
| Vitest `vi.mock`                | https://vitest.dev/api/vi.html#vi-mock                                 | Mock specifiers are path-literal and NOT type-checked — stale paths silently no-op (empirically confirmed in this repo on Vitest 4.1.4). |
| Playwright locators / getByRole | https://playwright.dev/docs/locators#locate-by-role                    | `exact: true` + container scoping for the regression guard; strict-mode behavior with duplicate accessible names.                        |
| Testing Library `renderHook`    | https://testing-library.com/docs/react-testing-library/api/#renderhook | Pattern for the two ported hook test files.                                                                                              |

---

## Patterns to Mirror

### NAMING_CONVENTION

Components `PascalCase`, hooks `use`-prefixed `camelCase`, file name matches the export exactly. Relocated modules **keep their names** (`useLaunchDepGate.ts`, `LaunchDepGateModal.tsx`, `useProfilesPageProton.ts`, `ProfilesOverlays.tsx`) — smaller diff, and the vi.mock updates stay string-swaps. Hero-Detail-scoped helpers live in `components/library/launch/` and `components/library/profiles/` beside their consumers (existing siblings: `useHeroLaunchHooksAutosave.ts`, `HeroProfileActionsBar.tsx`). CSS classes are BEM-like `crosshook-<block>__<element>--<modifier>`.

### ERROR_HANDLING

Smoke tests guard against silent console errors:

```ts
// SOURCE: src/crosshook-native/tests/smoke.spec.ts:69-121 (pattern used by every describe)
const capture = attachConsoleCapture(page);
// ... interactions ...
expect(capture.errors, `context:\n${capture.errors.join('\n')}`).toEqual([]);
```

Fail fast in tasks: every batch ends with `npm run typecheck` — a dangling import is a hard error, never a warning to defer.

### LOGGING_PATTERN

No backend logging is touched. Playwright uses `list` + `html` reporters (`playwright.config.ts:42`); per-route screenshots are throwaway artifacts under gitignored `test-results/` (`page.screenshot({ path: 'test-results/...' })`) — **not** committed baselines.

### REPOSITORY_PATTERN (import paths)

- `@/` alias (= `src/crosshook-native/src/`) for cross-tree imports: `@/lib/ipc`, `@/context/*`, `@/hooks/*`, `@/types/*`.
- Relative `./` / `../` only within the same feature subtree.
- **Relocated files convert their old `../../../context/...`-style deep-relative imports to `@/` form** — matches the surviving Hero Detail files (e.g. `HeroDetailProfilesTab.tsx:2-12` uses `@/` throughout) and avoids fragile depth recomputation. Biome enforces import sort: run `npx biome check --fix` (or `./scripts/lint.sh --fix`) after each move.

```ts
// SOURCE: src/crosshook-native/src/components/library/GameDetail.tsx:2-14
import { usePreferencesContext } from '@/context/PreferencesContext';
import { useGameCoverArt } from '@/hooks/useGameCoverArt';
import type { LibraryCardData } from '@/types/library';
```

### SERVICE_PATTERN (dev-mock IPC helpers)

New test fixtures go through the sanctioned `_mock_`-prefixed dev-command pattern (excluded from `scripts/check-mock-coverage.sh:94`):

```ts
// SOURCE: src/crosshook-native/src/lib/mocks/handlers/launch.ts:475-483
// _mock_set_profile_running mutates module-scope state consumed by list_running_profiles
// SOURCE: src/crosshook-native/tests/navigation-helpers.ts:45-57
export async function seedMockProfileRunning(page: Page, profileName: string, running: boolean) {
  await page.evaluate(/* window.__CROSSHOOK_DEV__.callCommand('_mock_set_profile_running', …) */);
}
// Always await waitForCrosshookDevIpc(page) first (navigation-helpers.ts:41-43).
```

### TEST_STRUCTURE

Vitest unit tests use `renderWithMocks` (`src/test/render.tsx`) with the standard IPC mock:

```tsx
// SOURCE: src/crosshook-native/src/components/library/__tests__/GameDetail.test.tsx:12-23
vi.mock('@/lib/ipc', async () => {
  const { mockCallCommand } = await import('@/test/render');
  return { callCommand: mockCallCommand };
});
```

Hook tests use `renderHook` + `vi.mock('@/lib/ipc')` / `vi.mock('@/lib/events')` (mirror `src/hooks/__tests__/useProfileSummaries.test.ts:1-70`). Smoke tests reuse `navigation-helpers.ts` helpers — never inline `page.goto` + click chains. **Gotcha (empirically verified in this repo)**: `vi.mock('<nonexistent path>', factory)` does NOT error — the real module loads and the test passes-but-lies. Every relocation task updates its vi.mock specifiers in the same task, and Batch 4 greps for stragglers.

---

## Files to Change

| File                                                                                          | Action | Justification                                                                                                                                                                                                                                                    |
| --------------------------------------------------------------------------------------------- | ------ | ---------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `src/components/pages/launch/useLaunchDepGate.ts` → `src/components/library/launch/`          | MOVE   | Shared: `HeroLaunchGate.tsx:23` imports it.                                                                                                                                                                                                                      |
| `src/components/pages/launch/LaunchDepGateModal.tsx` → `src/components/library/launch/`       | MOVE   | Shared: `HeroLaunchGate.tsx:22`; imports `DepGateState` from its sibling — moves as a pair.                                                                                                                                                                      |
| `src/components/pages/profiles/useProfilesPageProton.ts` → `src/components/library/profiles/` | MOVE   | Shared: `HeroDetailProfilesTab.tsx:14`.                                                                                                                                                                                                                          |
| `src/components/pages/profiles/ProfilesOverlays.tsx` → `src/components/library/profiles/`     | MOVE   | Shared: `HeroProfileActionsBar.tsx:31`.                                                                                                                                                                                                                          |
| `src/hooks/profile/communityExport.ts`                                                        | ADD    | New home for `suggestedCommunityExportFilename` (from dissolved `pages/profiles/utils.ts`).                                                                                                                                                                      |
| `src/components/library/__tests__/useLaunchDepGate.test.tsx`                                  | ADD    | Re-homed coverage for surviving dep-gate logic (ported from `LaunchRoute.test.tsx:287-389`).                                                                                                                                                                     |
| `src/components/library/__tests__/useProfilesPageProton.test.tsx`                             | ADD    | Re-homed coverage for surviving ProtonDB suggestion logic (ported from `ProfilesRoute.test.tsx:226-279`).                                                                                                                                                        |
| `src/components/library/launch/HeroLaunchGate.tsx`                                            | EDIT   | Repoint imports at lines 22–23 to `./LaunchDepGateModal` / `./useLaunchDepGate`.                                                                                                                                                                                 |
| `src/components/library/__tests__/HeroLaunchGate.test.tsx`                                    | EDIT   | Repoint vi.mock specifiers at lines 45 and 129.                                                                                                                                                                                                                  |
| `src/components/library/HeroDetailProfilesTab.tsx`                                            | EDIT   | Repoint import at line 14 to `./profiles/useProfilesPageProton`.                                                                                                                                                                                                 |
| `src/components/library/__tests__/HeroDetailProfilesTab.test.tsx`                             | EDIT   | Repoint vi.mock specifier at line 170.                                                                                                                                                                                                                           |
| `src/hooks/profile/useProfileActions.ts`                                                      | EDIT   | Repoint import at line 21 to `./communityExport`.                                                                                                                                                                                                                |
| `src/components/library/profiles/HeroProfileActionsBar.tsx`                                   | EDIT   | Repoint import at line 31 to `./ProfilesOverlays`.                                                                                                                                                                                                               |
| `src/__tests__/a11y/routes.a11y.test.tsx`                                                     | EDIT   | Remove ProfilesPage/LaunchPage imports, `ROUTE_PAGES` entries, populated-LaunchPage block + its overrides.                                                                                                                                                       |
| `src/components/layout/__tests__/AppShell.test.tsx`                                           | EDIT   | Remove the two `NOTE(hero-detail-consolidation)` markers; drop `NO_DATALIST_OVERRIDES` if the test stays green.                                                                                                                                                  |
| `src/components/pages/ProfilesPage.tsx`                                                       | DELETE | Unrouted since #495; last importer removed in Task 1.4.                                                                                                                                                                                                          |
| `src/components/pages/LaunchPage.tsx`                                                         | DELETE | Unrouted since #495; last importer removed in Task 1.4.                                                                                                                                                                                                          |
| `src/components/pages/profiles/` (6 remaining files)                                          | DELETE | Pure orphans after B1: `constants.ts`*, `ProfilesHealthIssues.tsx`, `ProfilesHero.tsx`, `useProfilesCollectionState.ts`, `useProfilesPageNotifications.ts`, `useProfilesPageState.ts` (*deleted in Task 1.2 with `utils.ts`).                                    |
| `src/components/pages/launch/` (2 remaining files)                                            | DELETE | Pure orphans: `LaunchProfileSelector.tsx`, `useLaunchPageState.ts`.                                                                                                                                                                                              |
| `src/components/pages/__tests__/ProfilesRoute.test.tsx`                                       | DELETE | Dead-route RTL test (issue #475 scope; live behaviors ported in B1).                                                                                                                                                                                             |
| `src/components/pages/__tests__/LaunchRoute.test.tsx`                                         | DELETE | Dead-route RTL test (issue #475 scope; live behaviors ported in B1).                                                                                                                                                                                             |
| `src/components/layout/PageBanner.tsx`                                                        | EDIT   | Remove now-orphaned `LaunchArt` + `ProfilesArt` exports (only consumers were `LaunchPage.tsx:7` / `ProfilesHero.tsx:3`).                                                                                                                                         |
| `src/styles/theme.css`                                                                        | EDIT   | Prune orphan rules: `.crosshook-profiles-page*`, `.crosshook-profiles-editor-host`, `.crosshook-profiles-hero-*`, `.crosshook-profiles-subtabs`, `.crosshook-launch-page__grid`, `.crosshook-page-scroll-shell--profiles/--launch`, `.crosshook-legacy-*-title`. |
| `tests/smoke.spec.ts`                                                                         | EDIT   | Prune 2 dead selectors at L108; add appRoute regression guard; strengthen Hero Detail flow; toolbar-chip assertion.                                                                                                                                              |
| `tests/navigation-helpers.ts`                                                                 | EDIT   | Add `seedMockProfileVariant` helper (mirrors `seedMockProfileRunning`).                                                                                                                                                                                          |
| `src/lib/mocks/handlers/profile-core.ts` (or `profile-mutations.ts`, implementor's call)      | EDIT   | Add `_mock_add_profile` / `_mock_remove_profile` dev-only handlers.                                                                                                                                                                                              |
| `docs/prps/prds/unified-desktop-hero-detail-consolidation.prd.md`                             | EDIT   | Flip phase-table rows 8–11 to `complete`; add PRP links for 10/11.                                                                                                                                                                                               |

## NOT Building

- **`ProfilesIcon` / `LaunchIcon` removal** (issue #475 step 10, PRD Phase 10 step 10, Phase 12 step 4): **the "if no caller remains" guard FAILS** — both icons are live in `palette/CommandPalette.tsx:12,14,101,103` for the `launch_profile`/`edit_profile` palette commands. Removing them breaks the build. Recorded as a verified no-op.
- **`.crosshook-launch-pipeline` CSS or its smoke/pipeline assertions**: the pipeline did not die — it moved into Hero Detail (`HeroLaunchGate.tsx:217`). `smoke.spec.ts:250-262`, `:297-307`, and all of `tests/pipeline.spec.ts` test surviving UI. PRD Phase 11 step 2's "remove launch pipeline smoke" referred to the pre-#495 route-based block, which no longer exists.
- **`lib/commands.ts` `'profiles'`/`'launch'` strings**: these are `CommandPaletteIconId` keys, not routes. Survive.
- **Backend / Rust changes**: zero orphaned IPC commands — every command invoked by deleted files (`community_list_indexed_profiles`, `list_proton_installs`, `profile_list_summaries`) keeps surviving callers (`useProtonInstalls.ts`, `useProfileSummaries.ts`, `useLibrarySummaries.ts`, relocated proton hook). `platform.rs`/host-gateway untouched.
- **Screenshot baseline regeneration**: there are NO committed Playwright baselines (`toHaveScreenshot` never used; `test-results/` gitignored; `playwright.config.ts:46`). Issue #476's `test:smoke:update` step is a no-op — the gate is `npm run test:smoke` green.
- **Phase 12 items**: `docs/internal-docs/design-tokens.md` updates, release-notes bullet, manual Steam Deck pass. (Orphan CSS pruning IS pulled forward — see Task 2.1 — because this plan creates those orphans and the repo's no-dead-code rule applies.)
- **PRD table rows 2 and 4**: also stale (`pending`/`in-progress` though shipped in #481/#493), but owned by other PRs' history. Noted as drift in the PR body only.
- **Env-var autosave-gate smoke depth**: `LaunchRoute.test.tsx:223-285`'s env-typing-specific assertion is consciously dropped — gating semantics are covered by `HeroLaunchGate.test.tsx:487-520,560-592`.
- **Smoke pipeline dedupe** (3 places assert the 6-node count): redundant but live and cheap; out of scope.
- **Runtime execution of pre/post hooks**: separate PRD per Decisions Log.

---

## Step-by-Step Tasks

### Task 1.1: Relocate Launch Dep-Gate Pair and Port Its Behavioral Tests — Depends on [none]

- **BATCH**: B1
- **ACTION**: Move `useLaunchDepGate.ts` + `LaunchDepGateModal.tsx` to `components/library/launch/`, repoint the consumer + mocks, and port the dep-gate behaviors out of the dying `LaunchRoute.test.tsx`.
- **IMPLEMENT**: `git mv src/crosshook-native/src/components/pages/launch/useLaunchDepGate.ts src/crosshook-native/src/components/library/launch/useLaunchDepGate.ts` and likewise for `LaunchDepGateModal.tsx` (they are a coupled pair — `LaunchDepGateModal.tsx:3` imports `type { DepGateState } from './useLaunchDepGate'`; the sibling import survives the move verbatim). Convert their deep-relative imports (`../../../context/LaunchStateContext`, `../../../hooks/useLaunchPrefixDependencyGate`, `../../../types/profile`) to `@/` form. Update `HeroLaunchGate.tsx:22-23` to `import { LaunchDepGateModal } from './LaunchDepGateModal'` / `import { useLaunchDepGate } from './useLaunchDepGate'`. Update both vi.mock specifiers in `HeroLaunchGate.test.tsx` (lines 45, 129) to `@/components/library/launch/useLaunchDepGate` / `@/components/library/launch/LaunchDepGateModal`. Create `components/library/__tests__/useLaunchDepGate.test.tsx` porting two behaviors from `LaunchRoute.test.tsx:287-389` as `renderHook` tests: (1) `handleBeforeLaunch` silently catches a rejected `getDependencyStatus` and still allows launch; (2) a `prefix-dep-complete` event is a no-op while the modal is closed.
- **MIRROR**: `REPOSITORY_PATTERN` (convert to `@/` imports), `TEST_STRUCTURE` (renderHook + `vi.mock('@/lib/ipc')` per `useProfileSummaries.test.ts`), `NAMING_CONVENTION` (keep file names).
- **IMPORTS**: Do NOT touch `LaunchPage.tsx:9,11` — its imports dangle intentionally and die with the file in Task 2.1. Editing it would churn a file scheduled for deletion.
- **GOTCHA**: A stale vi.mock path does NOT error in Vitest — the real module loads and tests pass-but-lie (empirically verified in this repo). Update both specifiers in THIS task, never later.
- **VALIDATE**: `cd src/crosshook-native && npm run typecheck && npm test -- HeroLaunchGate useLaunchDepGate` — EXPECT: zero TS errors; HeroLaunchGate suite green; the two new ported tests green.

### Task 1.2: Relocate useProfilesPageProton, Dissolve utils.ts and constants.ts, Port Proton Hook Test — Depends on [none]

- **BATCH**: B1
- **ACTION**: Move the proton-suggestion hook beside its Hero Detail consumer, split the two-export `utils.ts` to its consumers' trees, delete the dead `constants.ts` re-export barrel, and port ProtonDB coverage.
- **IMPLEMENT**: `git mv src/crosshook-native/src/components/pages/profiles/useProfilesPageProton.ts src/crosshook-native/src/components/library/profiles/useProfilesPageProton.ts`. Inside the moved file: re-point `import type { CommunityIndexedProfileRow } from './constants'` (line 7) directly to `@/hooks/profile/profileNotificationConstants` (`constants.ts` is a pure re-export barrel of that module — do not relocate the barrel); fold `sortProtonInstalls` (from `pages/profiles/utils.ts`) in as a module-private function (its only surviving consumer); convert remaining deep-relative imports (`../../hooks/useProtonUp`, `../../types/*`) to `@/`. Create `src/hooks/profile/communityExport.ts` exporting `suggestedCommunityExportFilename` (verbatim from `utils.ts`); update `hooks/profile/useProfileActions.ts:21` to `import { suggestedCommunityExportFilename } from './communityExport'`. Update `HeroDetailProfilesTab.tsx:14` to `import { useProfilesPageProton } from './profiles/useProfilesPageProton'`. Update the vi.mock specifier at `HeroDetailProfilesTab.test.tsx:170` to `@/components/library/profiles/useProfilesPageProton`. `git rm src/crosshook-native/src/components/pages/profiles/utils.ts src/crosshook-native/src/components/pages/profiles/constants.ts` (both now zero-importer). Create `components/library/__tests__/useProfilesPageProton.test.tsx` porting the essence of `ProfilesRoute.test.tsx:226-279`: hook fetches `list_proton_installs` + `community_list_indexed_profiles`, sorts installs, and surfaces the suggestion row (renderHook + mocked IPC).
- **MIRROR**: `REPOSITORY_PATTERN`, `TEST_STRUCTURE` (hook-test pattern), `NAMING_CONVENTION`.
- **IMPORTS**: Note a separate module-private `sortProtonInstalls` already exists at `hooks/useProtonInstalls.ts:20` — duplicate logic, but do NOT merge or export it; leave it untouched.
- **GOTCHA**: Splitting `utils.ts` (not moving it wholesale) keeps the dependency direction clean — a wholesale move to `components/library/` would force `hooks/profile/useProfileActions.ts` to import from the components tree (inverted layering). `pages/health-dashboard/constants.ts` is a DIFFERENT `./constants` — don't conflate.
- **VALIDATE**: `cd src/crosshook-native && npm run typecheck && npm test -- HeroDetailProfilesTab useProfilesPageProton useProfileActions` — EXPECT: zero TS errors; all suites green including the new ported test.

### Task 1.3: Relocate ProfilesOverlays — Depends on [none]

- **BATCH**: B1
- **ACTION**: Move `ProfilesOverlays.tsx` beside its only surviving consumer.
- **IMPLEMENT**: `git mv src/crosshook-native/src/components/pages/profiles/ProfilesOverlays.tsx src/crosshook-native/src/components/library/profiles/ProfilesOverlays.tsx`. Its own deps (`ConfigHistoryPanel`, `OnboardingWizard`, `ProfilePreviewModal`) live directly in `components/` and both old and new locations are exactly two levels under `components/`, so `../../X` resolves unchanged — still convert to `@/components/...` form per convention. Update `HeroProfileActionsBar.tsx:31` to `import { ProfilesOverlays } from './ProfilesOverlays'`.
- **MIRROR**: `REPOSITORY_PATTERN`, `NAMING_CONVENTION`.
- **IMPORTS**: No vi.mock exists for `ProfilesOverlays` anywhere (verified) — only the one production import moves.
- **GOTCHA**: `ProfilesPage.tsx` also imports `ProfilesOverlays` — leave that import dangling at the OLD path? No: after `git mv` the old path is gone, so `ProfilesPage.tsx` breaks `tsc` until Task 2.1 deletes it. **Therefore Task 1.3's gate runs `tsc` scoped expectations**: `ProfilesPage.tsx`'s import must ALSO be updated in this task to the new path (one-line edit; the file still dies in B2). Same applies in Tasks 1.1/1.2 for `LaunchPage.tsx:9,11`, `useProfilesPageState.ts:15` (imports `useProfilesPageProton`), and `ProfilesPage.tsx`'s `utils`/`constants`/`ProfilesHero` imports if any break — keep the doomed files compiling with minimal one-line repoints so every batch stays green. (This supersedes the "do not touch" note in Tasks 1.1/1.2 ONLY where `tsc` actually breaks; verify with the task's typecheck.)
- **VALIDATE**: `cd src/crosshook-native && npm run typecheck && npm test -- HeroProfileActionsBar` — EXPECT: zero TS errors; suite green.

### Task 1.4: Surviving-Test Surgery (a11y Routes + AppShell) — Depends on [none]

- **BATCH**: B1
- **ACTION**: Remove the deleted pages from the a11y route matrix and clean the Phase-10 markers out of `AppShell.test.tsx`.
- **IMPLEMENT**: In `src/__tests__/a11y/routes.a11y.test.tsx`: delete imports `LaunchPage` (line 10) and `ProfilesPage` (line 12); delete `ROUTE_PAGES` entries at lines 94–95; delete the `'LaunchPage with populated profiles…'` it-block (lines 252–263) and the now-unused `POPULATED_LAUNCH_OVERRIDES` const (lines 159–213); reword the `HealthDashboardPage` populated-test comment (lines 266–268) which currently says it "substitutes for ProfilesPage" — the rationale must stand on its own now; prune the stale ProfilesPage-datalist explanation block (~lines 144–157) accordingly. **a11y parity**: Hero Detail tab axe coverage already exists in `src/__tests__/a11y/components.a11y.test.tsx:230+` (`hero-detail-profiles-tab` and `launch-options` axe runs), so the net axe-covered surface does not shrink — state this in the diff's comment if reviewers ask. In `src/components/layout/__tests__/AppShell.test.tsx`: delete the two `NOTE(hero-detail-consolidation): delete with Phase 10 route removal` markers (lines 26, 35); then attempt removing `NO_DATALIST_OVERRIDES` (lines 37–40) and its usage at line 176 — run `npm test -- AppShell`; if green, keep the removal; if the quick-filter test fails, restore the override and instead rewrite the lines 19–36 JSDoc to drop the ProfilesPage framing. Keep the `'does not expose deleted Profiles route command'` guard (lines 495–516) untouched.
- **MIRROR**: `TEST_STRUCTURE`.
- **IMPORTS**: Only removals in this task.
- **GOTCHA**: The pages still exist during B1, so this file compiles before AND after the edit — order within B1 doesn't matter. Issue #475's body never mentions this file; it is the hidden hard blocker for deletion.
- **VALIDATE**: `cd src/crosshook-native && npm run typecheck && npm test -- routes.a11y AppShell` — EXPECT: zero TS errors; a11y suite green with 9 route entries; AppShell suite green.

### Task 2.1: Physical Deletion + Orphan Asset Sweep — Depends on [1.1, 1.2, 1.3, 1.4]

- **BATCH**: B2
- **ACTION**: `git rm` the dead modules and tests; remove the orphaned banner-art exports and CSS rules this deletion creates.
- **IMPLEMENT**: Pre-flight gate: `grep -rn "components/pages/profiles\|components/pages/launch" src/crosshook-native/src src/crosshook-native/tests --include='*.ts*'` must return matches ONLY inside the files this task deletes (after B1, the expected survivors are zero). Then:
  `git rm src/crosshook-native/src/components/pages/ProfilesPage.tsx src/crosshook-native/src/components/pages/LaunchPage.tsx src/crosshook-native/src/components/pages/__tests__/ProfilesRoute.test.tsx src/crosshook-native/src/components/pages/__tests__/LaunchRoute.test.tsx`
  `git rm -r src/crosshook-native/src/components/pages/profiles/ src/crosshook-native/src/components/pages/launch/` (remaining contents: `ProfilesHealthIssues.tsx`, `ProfilesHero.tsx`, `useProfilesCollectionState.ts`, `useProfilesPageNotifications.ts`, `useProfilesPageState.ts` / `LaunchProfileSelector.tsx`, `useLaunchPageState.ts` — all verified zero-survivor-importer; matches like `HeroProfileActionsBar.tsx:14,17,116` and `HeroProfileEditorExtras.tsx:176,209` are JSDoc comments, not imports).
  In `layout/PageBanner.tsx`: remove the `LaunchArt` and `ProfilesArt` exports (only consumers were `LaunchPage.tsx:7,72` and `ProfilesHero.tsx:3,73`); leave sibling `*Art` exports intact.
  In `styles/theme.css`: delete the verified-orphan rules — `.crosshook-profiles-editor-host` (~210, ~355), `.crosshook-profiles-page` block (~208, ~351–357), `.crosshook-profiles-hero-outer` / `.crosshook-profiles-hero-status` (~530–531), `.crosshook-profiles-subtabs`, `.crosshook-launch-page__grid` (~536–538), `.crosshook-page-scroll-shell--profiles`, `.crosshook-page-scroll-shell--launch`, `.crosshook-legacy-profiles-title`, `.crosshook-legacy-launch-title`. **Grep each class name across `src/` immediately before deleting its rule** — line numbers drift; the class list is the contract. Do NOT touch `.crosshook-launch-subtabs`, `.crosshook-route-hero-launch-panel`, `styles/launch-pipeline.css`, or any `.crosshook-hero-detail__*` rule (all live).
- **MIRROR**: `ERROR_HANDLING` (fail fast — `tsc` is the missed-importer safety net).
- **IMPORTS**: `ContentArea.tsx` needs NO change — already clean post-#495 (issue #475 steps 7–8 are done; record as verified no-op). `RouteBanner route="profiles"/"launch"` grep — zero matches expected (#495 inlined then orphaned them; record as verified no-op).
- **GOTCHA**: Do NOT remove `ProfilesIcon`/`LaunchIcon` from `icons/SidebarIcons.tsx` (issue #475 step 10's condition fails — `CommandPalette.tsx:12,14,101,103` still consumes both). Deleting them breaks the build.
- **VALIDATE**: `cd src/crosshook-native && npm run typecheck && npm test && cd ../.. && ./scripts/lint.sh` — EXPECT: zero TS errors; full Vitest suite green (2 fewer test files); lint exit 0.

### Task 3.1: Smoke Rewrite — Regression Guard, Card-Switch Flow, Chip Assertion, Selector Prune — Depends on [2.1]

- **BATCH**: B3
- **ACTION**: Close the four genuine #476 gaps against the post-#495 smoke file.
- **IMPLEMENT**:
  1. **Selector prune**: in `tests/smoke.spec.ts` (~line 108), remove `, .crosshook-profiles-page__body, .crosshook-launch-page__grid` from the dashboard-body locator union (those CSS classes died in Task 2.1). Verify `ROUTE_ORDER` (lines 40–50) contains no `'profiles'`/`'launch'` — already done in #495; no edit.
  2. **appRoute regression guard** (the single net-new describe issue #476 still requires):
     ```ts
     test.describe('appRoute regression guard', () => {
       test('sidebar exposes no Profiles or Launch route tabs', async ({ page }) => {
         const capture = attachConsoleCapture(page);
         await page.goto('/?fixture=populated');
         const sidebar = page.getByTestId('sidebar'); // Sidebar.tsx:253
         await expect(sidebar).toBeVisible();
         await expect(sidebar.getByRole('tab', { name: 'Profiles', exact: true })).toHaveCount(0);
         await expect(sidebar.getByRole('tab', { name: 'Launch', exact: true })).toHaveCount(0);
         // Positive control: proves the query mechanism works.
         await expect(sidebar.getByRole('tab', { name: 'Library', exact: true })).toBeVisible();
         expect(capture.errors).toEqual([]);
       });
     });
     ```
     Scope to the sidebar testid and `exact: true` — Hero Detail legitimately renders `role="tab"` "Profiles"/"Launch options" inside `game-detail`, which must NOT be caught.
  3. **Mock fixture affordance**: every demo seed has a distinct `game.name` (`profile-core.ts:20-56`), so no game has two profile cards and the PRD's "select second profile card" step is impossible today (`profile_duplicate` changes `game.name`, creating a separate game). Add a dev-only handler pair in `src/lib/mocks/handlers/profile-core.ts` (or `profile-mutations.ts` — wherever the store mutators live): `_mock_add_profile` ({ profileName, gameName }) inserting a DEMO-shaped profile sharing the given `gameName`, and `_mock_remove_profile` ({ profileName }) for cleanup (mirror `_mock_set_profile_running` at `launch.ts:475-483`; the `_mock_` prefix is excluded from `check-mock-coverage.sh:94`). Add `seedMockProfileVariant(page, profileName, gameName)` + removal counterpart to `tests/navigation-helpers.ts` (mirror `seedMockProfileRunning` at lines 45–57; always `waitForCrosshookDevIpc` first).
  4. **Hero Detail flow strengthening** (extend the existing desktop console-chrome test at ~lines 387–416, or add a sibling test in the same describe): open `Test Game Alpha` Hero Detail → Profiles tab → seed `'Test Game Alpha - Modded'` variant via the new helper → expect TWO cards in the `Profile cards` list (`HeroProfileCardList.tsx:107`) → click the second card → assert it gains `aria-current="true"` + the `Active` pill (`HeroProfileCardList.tsx:116-135`) and the editor `<h3>` updates (`HeroDetailProfilesTab.tsx:209-211`) → switch to Launch options tab → assert the Launch-command section renders (`HeroLaunchCommandSection` — `DashboardPanelSection` titled "Launch command") → click the hero-header `Launch` button scoped to `game-detail` (`HeroDetailHeader.tsx:60-70`; the inspector duplicates the name) → assert launch registered via the existing log-line pattern (`smoke.spec.ts:412`) → assert the Library sidebar tab kept `aria-current="page"` for the whole flow (PRD Success Metric L53: zero route changes) → remove the seeded variant before the test ends.
  5. **Toolbar chip assertion**: in the existing `library sidebar quick filters` describe (lines 215–248), after clicking sidebar Favorites, additionally assert the TOOLBAR Favorites chip has `aria-pressed="true"` (`LibraryToolbar.tsx:69-76`) — PRD Success Metric L55 targets the chip, the current test only asserts the sidebar button.
- **MIRROR**: `SERVICE_PATTERN` (`_mock_` handler + helper), `TEST_STRUCTURE` (smoke helpers, console capture), `ERROR_HANDLING`.
- **IMPORTS**: Reuse `openLibraryHeroDetail`/`openHeroDetailTab`/`waitForCrosshookDevIpc` — never inline goto/click chains.
- **GOTCHA**: Seed the variant only AFTER Hero Detail is open and remove it BEFORE the test ends — if the library grid (one card per profile summary) re-renders with two cards titled "Test Game Alpha" while grid locators run, Playwright strict mode throws on duplicate accessible names (`View details for Test Game Alpha`). The variant's card title inside Hero Detail is disambiguated by profile name (`profileCardTitle` = `"<name> - <gameName>"`, `HeroProfileCardList.tsx:21-23`). Also: `typecheck` does NOT cover `tests/` (`tsconfig.test.json` excludes it) — smoke edits are validated only by running Playwright.
- **VALIDATE**: `cd src/crosshook-native && npm run test:smoke` — EXPECT: full smoke suite green including the new guard + strengthened flow (first run may need `npm run test:smoke:install`).

### Task 4.1: Grep Guards, PRD Table Update, Issue Hygiene — Depends on [3.1]

- **BATCH**: B4
- **ACTION**: Run the permanent regression-guard set, flip the PRD phase-table rows this serial chain completed, and finish the documentation trail.
- **IMPLEMENT**: Run every command in `Validation Commands > Grep Guards` below; each must return zero matches. In `docs/prps/prds/unified-desktop-hero-detail-consolidation.prd.md` (table rows at lines 287–290): set Phase 8 and Phase 9 Status to `complete` (shipped in PR #495 — their PR omitted the flip; cite `9f72b4f5` in the cell or leave the PRP column `-`), set Phase 10 and Phase 11 to `complete` with the PRP column linking `../plans/github-issues-475-476-legacy-page-deletion-smoke-rewrite.plan.md` (precedent: commit `f378cb91` added Phase 7's links). Do NOT touch rows 2/4 (stale, but owned by #481/#493 — note the drift in the PR body instead).
- **MIRROR**: Validation-command style of `docs/prps/reports/github-issues-473-474-route-removal-nav-rewire-report.md` §Validation.
- **IMPORTS**: None.
- **GOTCHA**: Issue #476's literal acceptance criterion — "grep `'profiles'|'launch'` in src/ excluding tests returns 0" — is **unachievable as written**: 30+ legitimate survivors exist (`HeroDetailTabId` values `'profiles'`/`'launch-options'`, `PipelineNodeId` `'launch'`, `GameProfile['launch']` TOML section, palette icon keys). The Grep Guards section below is the faithful interpretation (PRD L49: "as `AppRoute` values"); say so in the PR body.
- **VALIDATE**: All Grep Guards return zero matches; `cd src/crosshook-native && npm run typecheck && npm test && npm run test:smoke && cd ../.. && ./scripts/lint.sh` — EXPECT: everything green.

---

## Testing Strategy

### Unit Tests

| Test                                                     | Input / Setup                                                            | Expected                                                                      | Edge case? |
| -------------------------------------------------------- | ------------------------------------------------------------------------ | ----------------------------------------------------------------------------- | ---------- |
| `useLaunchDepGate` silent-catch (new, ported)            | `getDependencyStatus` IPC rejects; call `handleBeforeLaunch`             | No throw; launch proceeds (returns allow)                                     | yes        |
| `useLaunchDepGate` event no-op (new, ported)             | Emit `prefix-dep-complete` while modal closed                            | State unchanged; no re-fetch                                                  | yes        |
| `useProfilesPageProton` fetch + sort (new, ported)       | Mock `list_proton_installs` unsorted + `community_list_indexed_profiles` | Sorted installs; suggestion row surfaced                                      | no         |
| `HeroLaunchGate` suite (existing, repointed mocks)       | vi.mocks at NEW `@/components/library/launch/*` paths                    | Suite green AND mocks actually intercept (spot-check a mocked return value)   | yes        |
| `HeroDetailProfilesTab` suite (existing, repointed mock) | vi.mock at NEW `@/components/library/profiles/useProfilesPageProton`     | Suite green with mock intercepting                                            | yes        |
| `routes.a11y` matrix (edited)                            | 9 remaining route pages                                                  | Axe-clean; no ProfilesPage/LaunchPage entries; populated HealthDashboard kept | no         |
| `AppShell` quick-filter test (edited)                    | `NO_DATALIST_OVERRIDES` removed (if green)                               | Favorites/running intents still asserted                                      | yes        |

### Edge Cases Checklist

- [ ] Stale vi.mock specifier silently mocks nothing — guarded by same-task updates + Batch 4 grep (`vi\.mock\(.*pages/(launch|profiles)`).
- [ ] Doomed files (`ProfilesPage.tsx`, `LaunchPage.tsx`, `useProfilesPageState.ts`) must keep compiling through B1 — repoint their imports of moved modules with one-line edits where `tsc` demands it (Task 1.3 GOTCHA).
- [ ] Playwright strict-mode duplicate-name collision when the variant profile exists — seed after Hero Detail opens, remove before test end.
- [ ] `_mock_add_profile` summary shape must match `DEMO_PROFILE_SEEDS` derivation (`gameName` from `profile.game.name`, `profile-core.ts:85`) or the card list filter won't pick it up.
- [ ] `npm run typecheck` does NOT cover `tests/` — smoke edits validated only by `npm run test:smoke`.
- [ ] Currently Playing / Favorites with zero matches render the filtered empty state ("No games match your search or filters.") — existing tests already pin this; don't break it.
- [ ] `theme.css` line numbers drift — grep each orphan class before deleting its rule.

---

## Validation Commands

### Static Analysis

```bash
cd src/crosshook-native
npm run typecheck
```

EXPECT: Zero errors in both app and test tsconfigs after every batch.

### Focused Unit Tests (per task — see each task's VALIDATE)

```bash
cd src/crosshook-native
npm test -- HeroLaunchGate useLaunchDepGate HeroDetailProfilesTab useProfilesPageProton useProfileActions HeroProfileActionsBar routes.a11y AppShell
```

EXPECT: All focused suites pass.

### Full Frontend Suite

```bash
cd src/crosshook-native
npm test
```

EXPECT: Full Vitest suite green; exactly 2 fewer test files than before (ProfilesRoute, LaunchRoute) plus 2 new ones (useLaunchDepGate, useProfilesPageProton).

### Grep Guards

```bash
cd src/crosshook-native
# G1 — no legacy page modules on disk
test ! -e src/components/pages/ProfilesPage.tsx && test ! -e src/components/pages/LaunchPage.tsx \
  && test ! -d src/components/pages/profiles && test ! -d src/components/pages/launch \
  && test ! -e src/components/pages/__tests__/ProfilesRoute.test.tsx \
  && test ! -e src/components/pages/__tests__/LaunchRoute.test.tsx && echo G1-PASS
# G2 — no surviving import or mock of the legacy paths
rg -n "components/pages/(profiles|launch)|pages/ProfilesPage|pages/LaunchPage" src tests ; test $? -eq 1 && echo G2-PASS
# G3 — no AppRoute resurrection (report-precedent pattern)
rg -n "onNavigate\('(profiles|launch)'|handleNavigate\('(profiles|launch)'|route: '(profiles|launch)'|Go to (Profiles|Launch)|Open in Profiles page" src tests ; test $? -eq 1 && echo G3-PASS
# G4 — no orphan CSS class references
rg -n "crosshook-(profiles-page|launch-page|profiles-editor-host|legacy-(profiles|launch)-title|page-scroll-shell--(profiles|launch))" src tests ; test $? -eq 1 && echo G4-PASS
```

EXPECT: G1–G4 all PASS. (Bare-word `'profiles'`/`'launch'` greps are intentionally NOT used — `HeroDetailTabId`, `PipelineNodeId`, `GameProfile['launch']`, and palette icon keys are expected survivors.)

### Lint

```bash
./scripts/lint.sh
```

EXPECT: Exit 0 (Biome import-sort clean on all moved files).

### Smoke Validation

```bash
cd src/crosshook-native
npm run test:smoke:install   # first run only
npm run test:smoke
```

EXPECT: Full Playwright suite green, including `appRoute regression guard` and the strengthened Hero Detail flow. (No committed screenshot baselines exist — `test:smoke:update` is not a deliverable.)

### Binary Build Gate

```bash
npm run build:binary
```

EXPECT: Web build + binary build complete without error.

### Manual Validation

```bash
./scripts/dev-native.sh --browser
```

EXPECT: Sidebar shows Library/Install/…/Settings + Favorites + Currently Playing; no Profiles/Launch entries; Hero Detail Profiles + Launch options tabs fully functional.

---

## Acceptance Criteria

- [ ] All 13 legacy files deleted; 4 shared modules relocated under `components/library/`; `suggestedCommunityExportFilename` lives in `hooks/profile/communityExport.ts` (#475).
- [ ] `npm run typecheck`, `npm test`, `./scripts/lint.sh`, `npm run test:smoke` all green (#475 + #476).
- [ ] Grep Guards G1–G4 return zero matches — the faithful interpretation of #476's "grep returns 0" criterion (documented reinterpretation; bare words survive by design).
- [ ] Smoke: `appRoute regression guard` asserts the sidebar exposes no `role="tab"` named Profiles/Launch, with a Library positive control (#476, PRD Metric L48).
- [ ] Smoke: Hero Detail flow covers card list → second-card switch → Active pill/`aria-current` → Launch options → command section → hero-header Launch → launch registered → Library tab `aria-current` unchanged throughout (#476, PRD Metric L53).
- [ ] Smoke: Favorites sidebar entry asserts BOTH the sidebar button and the toolbar chip `aria-pressed="true"`; Currently Playing coverage retained (#476, PRD Metrics L55–L56).
- [ ] `ProfilesIcon`/`LaunchIcon` NOT removed (verified live in CommandPalette) — recorded no-op for #475 step 10.
- [ ] ContentArea/RouteBanner steps recorded as verified no-ops (done in #495).
- [ ] PRD phase table rows 8–11 flipped to `complete`; 10/11 link this plan.

## Completion Checklist

- [ ] All 3 vi.mock specifiers repointed in the same tasks as their module moves (Batch 4 grep confirms zero `pages/(launch|profiles)` mocks).
- [ ] Doomed-file interim repoints did not survive (the files are deleted — G2 proves it).
- [ ] New `_mock_add_profile`/`_mock_remove_profile` handlers carry the `_mock_` prefix (check-mock-coverage exclusion) and the smoke test cleans up its seeded variant.
- [ ] `theme.css` prune touched ONLY the verified-orphan classes; `.crosshook-launch-pipeline`, `.crosshook-launch-subtabs`, `.crosshook-hero-detail__*` untouched.
- [ ] Implementation report written to `docs/prps/reports/github-issues-475-476-legacy-page-deletion-smoke-rewrite-report.md` (mirror the 473-474 report sections, incl. Persistence Boundary + Deviations).
- [ ] Plan archived to `docs/prps/plans/completed/` on completion.
- [ ] PR opened per Notes below (one PR; `Part of #478`; `Closes #475`, `Closes #476`).

## Risks

| Risk                                                                        | Likelihood                       | Impact | Mitigation                                                                                                         |
| --------------------------------------------------------------------------- | -------------------------------- | ------ | ------------------------------------------------------------------------------------------------------------------ |
| Hidden importer of a deleted module surfaces at `git rm`                    | Low                              | Low    | Relocation-first ordering + Task 2.1 pre-flight grep + `tsc` gate after every batch.                               |
| Stale vi.mock path passes-but-mocks-nothing (empirically confirmed silent)  | High if deferred                 | High   | Specifier updates in the SAME task as each move; Batch 4 grep guard; spot-check one mocked return value.           |
| Variant-profile seed breaks strict-mode locators in other tests             | Medium                           | Medium | Seed inside one test only, after Hero Detail opens; `_mock_remove_profile` before test end; distinct profile name. |
| `AppShell` quick-filter test fails without `NO_DATALIST_OVERRIDES`          | Medium                           | Low    | Task 1.4 tries removal, keeps the override with reworded JSDoc on failure — both outcomes specified.               |
| Over-deletion of live CSS (`.crosshook-launch-pipeline` etc.)               | Medium if PRD followed literally | High   | Explicit DO-NOT-TOUCH list; grep-before-delete per class; smoke pipeline assertions stay as the tripwire.          |
| PRD/issue staleness misleads an implementor (line refs, `git rm -r`, icons) | High                             | High   | This plan supersedes the issue bodies; every stale instruction is called out inline with verified current state.   |

## Notes

- **One PR for both issues** (precedent: #495 closed #473+#474 together; #476's own Risk-H note demands smoke deletion in the same PR; this repo has zero post-hoc "fix smoke" commits — smoke always rides the feature PR).
  - **Title** (squash subject → CHANGELOG verbatim): `feat(ui): remove legacy profile and launch page modules` (mirrors #495's phrasing; `refactor(ui):` is defensible since the user-visible change shipped in #495 — follow `feat` precedent unless the maintainer prefers otherwise).
  - **Body**: `Part of #478` (umbrella tracker — verified OPEN with `tracking` label) + `Closes #475` + `Closes #476`; note the PRD rows 2/4 staleness drift; note the reinterpreted acceptance criteria (grep guards; "only deletions" impossible — a11y edit + relocations unavoidable; icons no-op).
  - **Labels**: `type:feature`, `area:ui`, `priority:high`, `feat:hero-detail-consolidation`, `phase:10`, `phase:11` (mirror the issues' label sets; #475 is priority:high, #476 medium — take the max).
- **This plan file's commit**: `docs(internal): issue 475 and 476 plan` (historical pattern: `33739c2f`).
- Optional doc hygiene NOT required for green: 20+ "mirrors ProfilesPage.tsx:NN" JSDoc breadcrumbs in `HeroProfileActionsBar.tsx`/`HeroProfileEditorExtras.tsx` become dangling after deletion; `utils/launch.ts:13` and `hooks/launch/useLaunchSubTabsProps.ts:13,33` mention LaunchPage in comments. Clean opportunistically or leave.
- Next step: `/ycc:prp-implement --parallel docs/prps/plans/github-issues-475-476-legacy-page-deletion-smoke-rewrite.plan.md` (B1 fans out 4 implementors; B2–B4 run single-implementor).
