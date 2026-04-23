# Implementation Report: Unified Desktop Phase 10 — Route Rework (Install · Settings · Community · Discover)

## Summary

Implemented Phase 10 of the Unified Desktop Redesign (GitHub issues #422 deliverable / #449 tracker). Re-skinned the four non-editor routes — Install, Settings, Community, Discover — to match the steel-blue visual language established by Phase 9. Also split `OnboardingWizard.tsx` (606 lines) and `CommunityBrowser.tsx` (561 lines) under the 500-line soft cap, harmonized inline error banners on the canonical `crosshook-error-banner--section` + `role="alert"` shape, added five per-route stylesheets registered in `main.tsx`, and added focused RTL + Playwright smoke coverage so the four routes do not regress silently.

No IPC changes, no persistence (TOML/SQLite) changes, no route-order changes, no inspector wiring changes. Pure frontend rework preserving every behavior contract the plan called out.

## Assessment vs Reality

| Metric        | Predicted (Plan) | Actual                                                                                                                                          |
| ------------- | ---------------- | ----------------------------------------------------------------------------------------------------------------------------------------------- |
| Complexity    | Large            | Large — as planned                                                                                                                              |
| Files Changed | ~22              | 31 (24 files changed + 7 new tests). Overshoot from one extra wizard-footer extract (Task 1.1) and earlier test-file creation by Tasks 2.2/2.3. |

## Execution Mode

Parallel sub-agents (`--parallel`) with worktree isolation on by default.

- **Batches**: 3 (B1 width 3, B2 width 5, B3 width 2)
- **Total tasks**: 10 dispatched as standalone `ycc:implementor` agents
- **Worktrees**: 1 parent + 10 children under `~/.claude-worktrees/crosshook-unified-desktop-phase-10-route-rework*/`
- All 10 children merged into parent cleanly with the `ort` strategy — zero conflicts.

## Tasks Completed

| Task | Title                                                     | Status   | Notes                                                                                                                                                                                                                                                                    |
| ---- | --------------------------------------------------------- | -------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------ |
| 1.1  | Split `OnboardingWizard` under the 500-line soft cap      | Complete | Extracted 5 stage bodies **plus** `OnboardingWizardFooter.tsx` to stay under cap. Parent dropped from 606 → 500 lines (see Deviations).                                                                                                                                  |
| 1.2  | Split `CommunityBrowser` under the 500-line soft cap      | Complete | 561 → 290 lines. Extracted `TapChip`, `CompatibilityBadge`, `CommunityTapManagementSection`, `CommunityProfilesSection`.                                                                                                                                                 |
| 1.3  | Per-route stylesheet scaffolding + register in `main.tsx` | Complete | 5 empty CSS files created. `SCROLL_ENHANCE_SELECTORS` gained `.crosshook-install-page-tabs__panel-inner`.                                                                                                                                                                |
| 2.1  | Re-skin Install route                                     | Complete | InstallPage wrapped in `<DashboardPanelSection>`; 5 flow sections wrapped; `install-routes.css` populated (90 lines). `InstallGamePanel.tsx` = 483 lines (under cap).                                                                                                    |
| 2.2  | Re-skin Settings route                                    | Complete | Header + 2-column grid wrapped in a single `<DashboardPanelSection>`. 11 `CollapsibleSection` sub-sections preserved. Created `SettingsPanel.test.tsx` (6 tests) ahead of Task 3.1.                                                                                      |
| 2.3  | Re-skin Community route                                   | Complete | `CollapsibleSection` inside both sibling sections swapped for `<DashboardPanelSection>`; refresh/sync moved into `actions` slot. `crosshook-community-browser__error` class preserved for wizard coupling. Created `CommunityPage.test.tsx` (3 tests) ahead of Task 3.1. |
| 2.4  | Re-skin Discover route                                    | Complete | 3 `<DashboardPanelSection>` regions (Community / Search / Results). `crosshook-success-banner` not in theme.css → kept `crosshook-warning-banner` + `--section` modifier for the notice surface.                                                                         |
| 2.5  | Re-skin OnboardingWizard modal chrome                     | Complete | Header aligned to `crosshook-heading-eyebrow` + `crosshook-heading-title--card`. `profileError` harmonized. Portal/focus-trap untouched.                                                                                                                                 |
| 3.1  | Focused RTL route-shell coverage                          | Complete | Created 3 new test files (Install/Community/Discover; Settings already existed from 2.2). 17 new tests. Existing `SettingsPanel.test.tsx` + `CommunityPage.test.tsx` were reviewed and found sufficient.                                                                 |
| 3.2  | Expand Playwright smoke                                   | Complete | 4 new entries in `DASHBOARD_ROUTE_HEADINGS`. Route-body locator extended. Heading locator scoped to `.crosshook-dashboard-panel-section__title` to avoid `<h1>` (RouteBanner) / `<h2>` (panel) collision on `install`.                                                   |

## Validation Results

| Level           | Status | Notes                                                                                                                              |
| --------------- | ------ | ---------------------------------------------------------------------------------------------------------------------------------- |
| Static Analysis | Pass   | `npm run typecheck` clean. `./scripts/lint.sh` clean (2 pre-existing warnings in `LibraryToolbar.tsx` unrelated to Phase 10).      |
| Unit Tests      | Pass   | 31 test files, **152 tests** all passing (up from 126 pre-Phase-10). 26 new tests landed across Batches 2 and 3.                   |
| Build           | Pass   | `npm run build` succeeds (313 ms). Chunk-size + dynamic-import warnings are pre-existing/unrelated.                                |
| Integration     | Pass   | Playwright smoke on install/settings/community/discover all pass with zero `pageerror` / `console.error`. 28 smoke tests pass.     |
| Edge Cases      | Pass   | Covered via RTL tests: consent gate when `discovery_enabled=false`, cached-fallback banner, `forceMount` across tab switches, etc. |

### Smoke test caveat (pre-existing, non-blocking)

Three smoke tests currently fail on `main` independent of Phase 10:

- `tests/collections.spec.ts:58:3` — CollectionViewModal open/close
- `tests/smoke.spec.ts:62:5` — route: library (Library)
- `tests/smoke.spec.ts:120:3` — library inspector at desktop width

Root cause is a `Maximum update depth exceeded` render loop in `LibraryPage.tsx` on `main`, reproduced identically on the Phase 10 branch. Phase 10 touched no library/collections code. Confirmed pre-existing by running the same spec against `/home/yandy/Projects/github.com/yandy-r/crosshook` on commit 64af1e5. Out of scope for this phase — should be filed as a separate bug.

## Files Changed

31 files total — 14 CREATED, 14 UPDATED, 3 other. Aggregate: +1968 / −655 lines.

### CREATED (14)

| File                                                                              | Lines | Purpose                                                                |
| --------------------------------------------------------------------------------- | ----- | ---------------------------------------------------------------------- |
| `src/crosshook-native/src/components/onboarding/OnboardingIdentityStageBody.tsx`  | 36    | Stage body extraction (Task 1.1)                                       |
| `src/crosshook-native/src/components/onboarding/OnboardingRuntimeStageBody.tsx`   | 39    | Stage body extraction                                                  |
| `src/crosshook-native/src/components/onboarding/OnboardingTrainerStageBody.tsx`   | 30    | Stage body extraction                                                  |
| `src/crosshook-native/src/components/onboarding/OnboardingMediaStageBody.tsx`     | 16    | Stage body extraction                                                  |
| `src/crosshook-native/src/components/onboarding/OnboardingReviewStageBody.tsx`    | 82    | Stage body extraction                                                  |
| `src/crosshook-native/src/components/onboarding/OnboardingWizardFooter.tsx`       | 123   | **Deviation** — extra extraction to keep parent under 500 (see below)  |
| `src/crosshook-native/src/components/community/TapChip.tsx`                       | 61    | Tap row extraction (Task 1.2)                                          |
| `src/crosshook-native/src/components/community/CompatibilityBadge.tsx`            | 25    | Rating badge extraction                                                |
| `src/crosshook-native/src/components/community/CommunityTapManagementSection.tsx` | 129   | Tap management panel; now uses `<DashboardPanelSection>` post Task 2.3 |
| `src/crosshook-native/src/components/community/CommunityProfilesSection.tsx`      | 181   | Profiles panel; now uses `<DashboardPanelSection>` post Task 2.3       |
| `src/crosshook-native/src/styles/install-routes.css`                              | 90    | Phase 10 Install chrome                                                |
| `src/crosshook-native/src/styles/settings-routes.css`                             | 85    | Phase 10 Settings chrome                                               |
| `src/crosshook-native/src/styles/community-routes.css`                            | 38    | Phase 10 Community chrome                                              |
| `src/crosshook-native/src/styles/discover-routes.css`                             | 34    | Phase 10 Discover chrome                                               |
| `src/crosshook-native/src/styles/onboarding-wizard.css`                           | 38    | Phase 10 Onboarding modal chrome                                       |
| `src/crosshook-native/src/components/__tests__/SettingsPanel.test.tsx`            | 99    | 6 tests (Task 2.2)                                                     |
| `src/crosshook-native/src/components/__tests__/InstallGamePanel.test.tsx`         | 169   | 5 tests (Task 3.1)                                                     |
| `src/crosshook-native/src/components/__tests__/CommunityBrowser.test.tsx`         | 132   | 6 tests                                                                |
| `src/crosshook-native/src/components/__tests__/TrainerDiscoveryPanel.test.tsx`    | 134   | 6 tests                                                                |
| `src/crosshook-native/src/components/pages/__tests__/CommunityPage.test.tsx`      | 60    | 3 tests (Task 2.3)                                                     |

### UPDATED (14 substantive, 3 small)

| File                                                                   | Action  | +Ins / −Del | Notes                                                  |
| ---------------------------------------------------------------------- | ------- | ----------- | ------------------------------------------------------ |
| `src/crosshook-native/src/components/OnboardingWizard.tsx`             | UPDATED | +83 / −181  | Split + re-skin (Tasks 1.1, 2.5). Now 500 lines.       |
| `src/crosshook-native/src/components/CommunityBrowser.tsx`             | UPDATED | +65 / −336  | Split (Task 1.2). Now 290 lines.                       |
| `src/crosshook-native/src/components/InstallGamePanel.tsx`             | UPDATED | +38 / −21   | 5 flow sections wrapped (Task 2.1). Now 483 lines.     |
| `src/crosshook-native/src/components/pages/InstallPage.tsx`            | UPDATED | +10 / −1    | Outer tabs wrapped in `<DashboardPanelSection>`        |
| `src/crosshook-native/src/components/install/InstallReviewSummary.tsx` | UPDATED | +8 / −4     | `crosshook-danger` → `crosshook-error-banner--section` |
| `src/crosshook-native/src/components/SettingsPanel.tsx`                | UPDATED | +21 / −21   | Header + grid wrapped                                  |
| `src/crosshook-native/src/components/TrainerDiscoveryPanel.tsx`        | UPDATED | +109 / −84  | 3 panel sections + banner harmonization                |
| `src/crosshook-native/src/components/pages/DiscoverPage.tsx`           | UPDATED | +12 / −2    | Route-body stack                                       |
| `src/crosshook-native/src/hooks/useScrollEnhance.ts`                   | UPDATED | +1 / −1     | `.crosshook-install-page-tabs__panel-inner` registered |
| `src/crosshook-native/src/main.tsx`                                    | UPDATED | +5          | 5 per-route CSS imports                                |
| `src/crosshook-native/tests/smoke.spec.ts`                             | UPDATED | +17 / −2    | 4 new headings + route-body locator                    |

## Deviations from Plan

1. **Task 1.1 extracted an extra file** — `OnboardingWizardFooter.tsx` was not in the original 5-file list. The implementor determined the footer block (Back/Next/Save/Done/Run-Checks buttons + `ControllerPrompts`) was large enough that the parent would not fit under 500 lines with just the 5 stage-body extractions. Outcome: parent landed at 495 lines after Task 1.1.

2. **`OnboardingWizard.tsx` landed at exactly 500 lines after Task 2.5**, not strictly below. Task 2.5's heading class alignment and error-banner harmonization added ~5 lines to the post-split 495. The memory note "500-line soft cap — slightly over is OK when no clean split seam exists" applies here. The remaining body is portal/focus-trap lifecycle + handler registrations that cannot be cleanly split further without breaking the wizard contract. No further action taken.

3. **Tasks 2.2 and 2.3 each created a test file** ahead of Task 3.1's schedule — `SettingsPanel.test.tsx` and `CommunityPage.test.tsx`. These weren't in the original scope for those tasks but were produced opportunistically. Task 3.1 adjusted to create the remaining 3 test files (Install, Community, Discover) rather than overwriting the two that already existed.

4. **`crosshook-success-banner` CSS class does not exist in `theme.css`**, so Task 2.4 used `<div className="crosshook-warning-banner crosshook-warning-banner--section" role="status">` for the Discover notice surface instead of introducing a new success-banner class. This matches the fallback instruction in the plan GOTCHA.

5. **Task 3.2 scoped the smoke heading locator to `.crosshook-dashboard-panel-section__title`** because `RouteBanner` renders "Install & Run" in an `<h1>` while `DashboardPanelSection` renders it in an `<h2>`. The plan's default `page.getByRole('heading', …)` would have matched both and tripped Playwright strict-mode. The scope change keeps the assertion deterministic without weakening it.

## Issues Encountered

- **Pre-existing library smoke failures** — confirmed out-of-scope (render loop in `LibraryPage.tsx` on `main`, commit 64af1e5, unrelated to Phase 10). No action.
- No other runtime, build, or validation issues. All 3 batch-fanouts merged cleanly on the first attempt.

## Tests Written

| Test File                                                 | Tests | Coverage                                                                                        |
| --------------------------------------------------------- | ----- | ----------------------------------------------------------------------------------------------- |
| `src/components/__tests__/SettingsPanel.test.tsx`         | 6     | Dashboard panel section, 2-column grid, 11 sections, status chips, RecentFilesColumn (Task 2.2) |
| `src/components/pages/__tests__/CommunityPage.test.tsx`   | 3     | Banner heading, DashboardPanelSection headings, action buttons, empty-state (Task 2.3)          |
| `src/components/__tests__/InstallGamePanel.test.tsx`      | 5     | Shell chrome, happy-path no `role="alert"`, 5 tab triggers, `forceMount` persistence, Reset     |
| `src/components/__tests__/CommunityBrowser.test.tsx`      | 6     | Region role, alert regression, cached-fallback `role="status"`                                  |
| `src/components/__tests__/TrainerDiscoveryPanel.test.tsx` | 6     | Panel heading, alert regression, consent gate when `discovery_enabled=false`, enabled state     |
| `tests/smoke.spec.ts` (updated)                           | 4 new | `DASHBOARD_ROUTE_HEADINGS` entries for install/settings/community/discover                      |

Total new unit tests: **26 added** (126 → 152).
New Playwright assertions: 4 new route assertions, all green with zero console errors.

## Worktree Summary

Parent: `~/.claude-worktrees/crosshook-unified-desktop-phase-10-route-rework/` (branch: `feat/unified-desktop-phase-10-route-rework`)

All 10 child worktrees have been merged back and cleaned up. Run this to remove the parent after merging and pushing:

```bash
git worktree remove ~/.claude-worktrees/crosshook-unified-desktop-phase-10-route-rework/
git branch -d feat/unified-desktop-phase-10-route-rework   # once merged to main
```

## Next Steps

- [ ] Review changes locally via `/ycc:code-review`
- [ ] Open a PR via `/ycc:prp-pr`
- [ ] File a separate bug for the pre-existing `LibraryPage.tsx` render loop surfaced by smoke (out of Phase 10 scope)
- [ ] Phase 11 (Profiles + Launch editor rework) is the natural follow-on per the PRD phase table
