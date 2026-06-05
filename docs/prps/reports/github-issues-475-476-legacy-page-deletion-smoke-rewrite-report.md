# Implementation Report: Legacy Page Deletion and Smoke Rewrite

## Summary

Implemented `docs/prps/plans/github-issues-475-476-legacy-page-deletion-smoke-rewrite.plan.md` in the current checkout with `--parallel --no-worktree`.

The legacy `/profiles` and `/launch` page implementation was removed after moving the still-used dependency gate, Proton manager overlay, profile overlay, and community export helpers into Library-owned modules. Route/a11y/AppShell tests no longer reference the deleted pages, obsolete page-specific CSS and banner art exports were pruned, and the smoke suite now guards against resurrecting legacy sidebar routes while covering the Hero Detail profile-card launch path.

During smoke validation, the new card-switch flow exposed that the Hero Detail header launch/edit actions still targeted the game summary name instead of the active profile card. `GameDetail` now passes the active profile name to `HeroDetailHeader`, so header launch/edit actions stay aligned with the selected card.

## Scope Completed

| Task                                               | Result   |
| -------------------------------------------------- | -------- |
| B1: Extract survivors from legacy pages            | Complete |
| B2: Delete legacy pages, tests, and CSS            | Complete |
| B3: Rewrite smoke coverage for Library Hero Detail | Complete |
| B4: Run guard grep checks and update PRD rows      | Complete |
| B5: Validate, report, and archive plan             | Complete |

## Files Changed

- Moved reusable launch/profile modules into `src/crosshook-native/src/components/library/`.
- Added focused tests for `useLaunchDepGate` and `useProfilesPageProton`.
- Added `src/crosshook-native/src/hooks/profile/communityExport.ts`.
- Removed legacy `ProfilesPage`, `LaunchPage`, their route tests, and page-local helper modules.
- Updated Library Hero Detail components and tests for active-profile launch/edit behavior.
- Extended mock profile mutation handlers and smoke navigation helpers for seeded profile variants.
- Rewrote smoke coverage around Library navigation, quick filters, and Hero Detail card switching.
- Updated `docs/prps/prds/unified-desktop-hero-detail-consolidation.prd.md` rows 10 and 11 to complete.

## Validation

| Command                | Result                                                                                 |
| ---------------------- | -------------------------------------------------------------------------------------- |
| `npm run typecheck`    | Pass                                                                                   |
| `npm test`             | Pass: 54 files, 426 tests                                                              |
| `npm run test:smoke`   | Pass: 88 tests                                                                         |
| `./scripts/lint.sh`    | Pass, with existing warnings in `useAutoSaveChip.ts` and `Breadcrumb.tsx`              |
| `npm run build:binary` | Pass, binary copied to `/home/yandy/.local/share/crosshook/artifacts/crosshook-native` |

Focused validation also passed during implementation:

- `npm run typecheck && npm test -- HeroLaunchGate useLaunchDepGate HeroDetailProfilesTab useProfilesPageProton useProfileActions HeroProfileActionsBar routes.a11y AppShell`
- `npx playwright test tests/smoke.spec.ts -g "profiles card switch stays in Library"`

## Guard Checks

All required grep guards passed:

- No imports of deleted `ProfilesPage`, `LaunchPage`, `ProfilesRoute`, or `LaunchRoute`.
- No imports from deleted `components/pages/profiles` or `components/pages/launch` helper modules.
- No stale legacy page CSS selectors.
- No AppShell/nav tests expecting legacy `/profiles` or `/launch` sidebar tabs.

## Deviations

- Added a small active-profile plumbing fix in `HeroDetailHeader` because the new smoke path proved the header action was selecting the game summary profile rather than the active profile card.
- The smoke test uses mock-only `_mock_add_profile` and `_mock_remove_profile` handlers to create and clean up a second profile variant for the same game.

## Follow-Up

Ready for code review and PR creation.
