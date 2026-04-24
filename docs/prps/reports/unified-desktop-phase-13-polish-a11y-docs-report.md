# Implementation Report: Phase 13 — Polish, Accessibility, and Documentation

## Summary

Closed the Phase 13 quality-gate for the unified-desktop redesign. Delivered:

- **Automated a11y scanning**: `jest-axe` wired into Vitest; 18 axe tests (11 route pages + 7 shell components) passing.
- **Focus-visible CSS rings** on library toolbar (search, chips, view-btn, palette-trigger), library list-row buttons (launch, icon), and themed-select items.
- **`prefers-reduced-motion` CSS guards** for 5 previously-uncovered animation surfaces (themed-select, host-tool-dashboard skeleton pulses, sidebar, library list rows, palette rows).
- **ARIA fixes**: explicit `aria-label` on ConsoleDrawer toggle; `'inspector'` added to `FocusZone` type.
- **Residual a11y fixes surfaced by axe**: heading-order (h1→h3 skip), unnamed ThemedSelect triggers, broken `aria-labelledby` references, duplicate landmark labels, null crashes in Settings/Profiles render paths.
- **Reduced-motion Playwright smoke test** for library + command palette.
- **Design-token catalogue expansion** from 111 → 264 lines (10 new sections covering typography, radius/shadow, layout, capability indicators, pipeline connectors, autosave, palette, controller-mode, responsive, high-contrast).
- **CI integration**: `check-legacy-palette.sh` wired into `.github/workflows/lint.yml` `shell` job.
- **Steam Deck manual QA checklist** (Phase 13 section appended to `steam-deck-validation-checklist.md`).

## Assessment vs Reality

| Metric        | Predicted (Plan)                             | Actual                                                                 |
| ------------- | -------------------------------------------- | ---------------------------------------------------------------------- |
| Complexity    | Medium (CSS + test scaffolding + doc append) | Medium-high (axe surfaced residual source fixes across 10+ components) |
| Confidence    | High                                         | High — all mechanical gates green                                      |
| Files Changed | ~15 files across styles, tests, CI, docs     | 28 files (source fixes from Task 2.1 expanded scope)                   |
| Batches       | 3 (5 parallel + 3 parallel + 3 sequential)   | Delivered as planned                                                   |

## Tasks Completed

| #   | Task                                            | Status          | Notes                                                                                             |
| --- | ----------------------------------------------- | --------------- | ------------------------------------------------------------------------------------------------- |
| 1.1 | Install jest-axe + setup.ts                     | Complete        | Also added `@types/jest-axe`. Biome auto-hoisted imports to top.                                  |
| 1.2 | Focus-visible CSS (library, themed-select)      | Complete        |                                                                                                   |
| 1.3 | Reduced-motion CSS (5 files)                    | Complete        | host-tool-dashboard used concrete class names instead of `[class*=]` pattern.                     |
| 1.4 | ARIA: ConsoleDrawer aria-label + FocusZone type | Complete        | State variable is `collapsed` (inverted); no switch-statements needed patching (all use if/else). |
| 1.5 | CI: wire check-legacy-palette.sh                | Complete        | Step ordering verified: host-gateway → legacy-palette → mock-coverage.                            |
| 2.1 | axe unit tests (routes + components)            | Complete        | **Expanded scope**: also fixed residual source-level violations (Task 3.1 work absorbed).         |
| 2.2 | Reduced-motion Playwright smoke                 | Complete        | Appended block to existing `smoke.spec.ts`; role name `tab` matched existing pattern.             |
| 2.3 | Expand design-tokens.md                         | Complete        | 111 → 264 lines, 10+ sections added.                                                              |
| 3.1 | Residual axe fixes                              | Merged into 2.1 | Task 2.1's implementor fixed violations as they surfaced — no separate work required.             |
| 3.2 | Steam Deck QA checklist                         | Complete        | Appended to `steam-deck-validation-checklist.md` (existing file) rather than creating duplicate.  |
| 3.3 | Final lint + format + cargo test                | Complete        | All green.                                                                                        |

## Validation Results

| Level           | Status                              | Notes                                                                                                                                                                            |
| --------------- | ----------------------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Static Analysis | **Pass**                            | `tsc --noEmit` clean. `./scripts/lint.sh` exit 0 (6 Biome warnings are info-level "unsafe fix" suggestions, not CI blockers). Legacy-palette, host-gateway, shellcheck all pass. |
| Unit Tests      | **Pass (w/ pre-existing failures)** | 187/189 tests pass. **2 failures pre-date Phase 13** — reproduced identically on `main` (AppShell palette-autofocus tests). Not regressions.                                     |
| Build           | **Pass**                            | `npm run build` (tsc + vite) exits 0. Pre-existing chunk-size / dynamic-import warnings are unchanged.                                                                           |
| Integration     | **N/A**                             | Playwright smoke requires dev-server + Chromium install; verification deferred to CI / hardware QA.                                                                              |
| Edge Cases      | **Documented**                      | Steam Deck manual QA checklist appended for hardware sign-off (gate for closing #425).                                                                                           |

## Files Changed

| Category                | Files                                                                                                                                                                                                                                                                                                          |
| ----------------------- | -------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| **CSS**                 | `library.css`, `themed-select.css`, `host-tool-dashboard.css`, `sidebar.css`, `palette.css`                                                                                                                                                                                                                    |
| **Components (ARIA)**   | `ConsoleDrawer.tsx`, `InstallPage.tsx`, `ProfileSubTabs.tsx`, `LaunchSubTabs.tsx`, `ProfilesHero.tsx`, `RunnerMethodSection.tsx`, `CommunityProfilesSection.tsx`, `HostToolFilterBar.tsx`, `SettingsPanel.tsx`, `ProfilesSection.tsx`, `InstallGamePanel.tsx`, `UpdateGamePanel.tsx`, `RunExecutablePanel.tsx` |
| **Types**               | `hooks/gamepad-nav/types.ts`                                                                                                                                                                                                                                                                                   |
| **Test infrastructure** | `src/test/setup.ts` (jest-axe wiring), `src/__tests__/a11y/routes.a11y.test.tsx` (new), `src/__tests__/a11y/components.a11y.test.tsx` (new), `tests/smoke.spec.ts` (reduced-motion describe)                                                                                                                   |
| **Dependencies**        | `package.json`, `package-lock.json` (+ `jest-axe@^8`, `@types/jest-axe@^3.5.9`)                                                                                                                                                                                                                                |
| **Docs**                | `docs/internal-docs/design-tokens.md` (+153 lines), `docs/internal-docs/steam-deck-validation-checklist.md` (+52 lines)                                                                                                                                                                                        |
| **CI**                  | `.github/workflows/lint.yml` (new step)                                                                                                                                                                                                                                                                        |

**28 files changed; 1,318 insertions / 13 deletions** (git diff --stat main..).

## Deviations from Plan

1. **Worktree path**: Plan specifies `~/.claude-worktrees/crosshook-feat/unified-desktop-phase-13` (with `/` separator); implementation uses `~/.claude-worktrees/crosshook-unified-desktop-phase-13` (flat slug). The skill's `merge-children.sh` requires the flat `<repo>-<slug>` format. Branch name `feat/unified-desktop-phase-13` matches the plan exactly; only the directory path differs.
2. **Steam Deck checklist filename**: Plan says `steam-deck-checklist.md`; existing file at `docs/internal-docs/steam-deck-validation-checklist.md` was extended rather than creating a duplicate.
3. **Task 3.1 → Task 2.1 absorption**: Task 2.1's implementor proactively fixed residual axe violations as they surfaced during test authoring. This was the intent of Task 3.1 (fix-after-discovery). Net result: zero axe violations across all 18 test cases.
4. **`@types/jest-axe` added**: Plan listed only `jest-axe`. TypeScript types required `@types/jest-axe@^3.5.9` for clean typecheck.
5. **host-tool-dashboard reduced-motion selector**: Plan suggested `[class*='...'][class*='skeleton']` pattern; implementor found 6 concrete class names at `host-tool-dashboard.css:362-393` and used those directly — stricter and no collateral effect.

## Issues Encountered

- **2 pre-existing AppShell test failures** (`opens the command palette with Ctrl+K and focuses the search input`, `restores focus to the invoking palette button after Escape close`): reproduced on `main` at HEAD `56b695c`. Not Phase 13 regressions. Out of scope for this PR; should be filed as a separate issue against the command palette autofocus flow.
- **Biome auto-hoisted imports** in `src/test/setup.ts` on commit (moved appended `jest-axe` imports to the top). Runtime behavior unchanged.

## Tests Written

| Test File                                     | Tests    | Coverage                                                                                                                                     |
| --------------------------------------------- | -------- | -------------------------------------------------------------------------------------------------------------------------------------------- |
| `src/__tests__/a11y/routes.a11y.test.tsx`     | 11 tests | All route pages: Library, Profiles, Launch, HealthDashboard, HostTools, ProtonManager, Community, Discover, Compatibility, Settings, Install |
| `src/__tests__/a11y/components.a11y.test.tsx` | 7 tests  | CommandPalette, Inspector, ContextRail, GameDetail, HeroDetailHeader, HeroDetailTabs, LibraryListRow                                         |
| `tests/smoke.spec.ts` (appended describe)     | 2 tests  | Library + CommandPalette under `prefers-reduced-motion: reduce`                                                                              |

**Total new tests: 20** (18 Vitest + 2 Playwright). All passing (unit) / typecheck-clean (smoke, pending hardware run).

## Commits (on `feat/unified-desktop-phase-13`)

```
2c2ecf3 docs(internal): add Phase 13 Steam Deck manual QA checklist
fa23f0c (merge) 2-3 docs(internal): expand design-tokens.md
cd10eb6 (merge) 2-2 test(smoke): add reduced-motion Playwright smoke test
86379da (merge) 2-1 test(a11y): add axe unit tests for all route pages and shell components
5f764d3 (merge) 1-5 ci: invoke check-legacy-palette.sh
6f810dc (merge) 1-4 fix(ui): aria-label on ConsoleDrawer + FocusZone inspector
e9ca4e7 (merge) 1-3 fix(ui): prefers-reduced-motion guards
9a7cae2 (merge) 1-2 fix(ui): focus-visible rings
e578b28 (merge) 1-1 chore(a11y): install jest-axe
```

Underlying feature commits on child branches: `f0f8d88`, `2b1bf6d`, `3550792`, `e61399c`, `ab4f853`, `216811c`, `3d01561`, `5366b3e`. All child branches have been deleted after fan-in merge.

## Worktree Summary

Current state (after fan-in cleanup):

| Path                                                     | Branch                          | Status |
| -------------------------------------------------------- | ------------------------------- | ------ |
| `~/.claude-worktrees/crosshook-unified-desktop-phase-13` | `feat/unified-desktop-phase-13` | parent |

All 8 child worktrees (Batches 1 & 2) have been merged and removed. Only the parent remains; it carries the full delta.

**Cleanup after PR merges to `main`**:

```bash
git worktree remove ~/.claude-worktrees/crosshook-unified-desktop-phase-13
git branch -d feat/unified-desktop-phase-13
```

## Next Steps

- [ ] Code review via `/ycc:code-review`
- [ ] Create PR via `/ycc:prp-pr` — **title**: `feat(ui): Phase 13 polish, accessibility, and design-token docs`; **body**: references `Closes #425, Part of #452`
- [ ] Steam Deck hardware pass through the appended checklist (sign-off gate for #425)
- [ ] Triage pre-existing AppShell palette-autofocus test failures in a separate issue
