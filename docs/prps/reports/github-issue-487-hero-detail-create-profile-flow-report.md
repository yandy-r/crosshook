# Implementation Report: Hero Detail Create-Profile Wizard and Creation Flow (Phase 5c)

**Plan**: `docs/prps/plans/completed/github-issue-487-hero-detail-create-profile-flow.plan.md`
**Issue**: [#487](https://github.com/yandy-r/crosshook/issues/487) (Part of #478)
**Branch**: `feat/487-hero-detail-create-profile`
**Mode**: `--parallel` sub-agent batches, `--no-worktree` (current checkout)
**Date**: 2026-06-04

## Summary

Hardened the existing Hero Detail Profiles-tab create flow (`+ New` → `OnboardingWizard mode="create"`, landed in #469) into a complete, self-sufficient creation surface: game-context prefill, post-create refresh/selection, cancel-state restore, create-mode duplicate-name guard, and an empty-state primary CTA. Reused the single `persistProfileDraft → profile_save` persistence path — zero new save calls, zero schema changes, zero new dependencies, zero Rust changes.

## Batch execution

| Batch | Tasks                                                                       | Mode       | Result                                         |
| ----- | --------------------------------------------------------------------------- | ---------- | ---------------------------------------------- |
| 1     | Task 1 (wizardSteps extraction), Task 2 (`useProfileSummaries` refresh fix) | parallel   | ✅ both green; inter-batch gate 364 tests pass |
| 2     | Task 3 (seed + duplicate guard + widened `onComplete`)                      | solo       | ✅ gate 390 tests pass                         |
| 3     | Task 4 (HeroProfileCardList wiring + empty-state CTA)                       | solo       | ✅ gate 413 tests pass                         |
| 4     | Task 5 (tab integration tests) → Task 6 (final gate + lint fixes)           | sequential | ✅ final 417 tests pass                        |

## Files changed

### New

- `src/crosshook-native/src/components/wizard/wizardSteps.ts` (35 ln) — `STAGE_TITLES`, `getVisibleStepNumber`, `getTotalVisibleSteps` extracted unchanged from the wizard (line-budget move).
- `src/crosshook-native/src/components/wizard/profileCreateSeed.ts` (59 ln) — `ProfileCreateSeed` interface + pure `applyCreateSeed` (numeric-only `/^\d{1,12}$/` Steam App ID guard, omit-empty semantics, non-mutating).
- `src/crosshook-native/src/components/wizard/__tests__/profileCreateSeed.test.ts` — 19 unit tests (mapping, appId guard, empty-seed no-op, purity).
- `src/crosshook-native/src/hooks/__tests__/useProfileSummaries.test.ts` — 3 tests (initial fetch, `profiles-changed` refetch, unsubscribe on unmount).
- `src/crosshook-native/src/components/library/profiles/__tests__/HeroProfileCardList.test.tsx` — 23 tests (seed builder unit coverage + wizard wiring, callbacks, empty-state CTA).

### Modified

- `src/crosshook-native/src/components/OnboardingWizard.tsx` (500 → 511 ln) — `createSeed?: ProfileCreateSeed` prop; `onComplete` widened to `(createdName?: string) => void` (assignment-compatible with all legacy call sites); ref-captured seed applied after the create-mode blank reset with cancellation guard (effect deps unchanged `[open, mode, selectProfile]`); create-mode-only duplicate-name guard (`profiles.includes(trimmedName)` → banner, no persist; cleared on rename); `onComplete(trimmedName)` on success; context-aware completed-stage copy when seeded.
- `src/crosshook-native/src/hooks/useProfileSummaries.ts` — subscribes to `profiles-changed` (mirrors `useProfile.ts:197-214` idiom); signature unchanged. This was the load-bearing fix — without it the new card never appears after create.
- `src/crosshook-native/src/components/library/profiles/HeroProfileCardList.tsx` (112 → 177 ln) — exported pure `buildHeroCreateSeed` (gameName, numeric appId, managed-media art paths, executable only when the singleton context profile owns one of this game's cards); memoized seed; `onComplete` (close + belt-and-suspenders `selectProfile(createdName)`); `onDismiss` (close + restore `selectProfile(selectedTrimmed)` when non-empty); empty-state `crosshook-panel role="status"` with primary `Create profile` CTA sharing the `+ New` open handler.
- `src/crosshook-native/src/components/__tests__/OnboardingWizard.test.tsx` — +10 tests (seed application, no re-reset on seed identity change, duplicate guard + recovery, name-carrying completion, failed-save keeps wizard open, legacy no-seed parity).
- `src/crosshook-native/src/components/library/__tests__/HeroDetailProfilesTab.test.tsx` — prop-capturing wizard stub + 4 integration tests (creation-without-navigation headline, cancellation restore, tab-level seed correctness, stays-open-on-failure smoke).

### Untouched (verified)

- `AppShell.tsx` / `ProfilesOverlays.tsx` legacy wizard call sites — byte-for-byte unchanged, compile clean.
- `package.json` / `package-lock.json` — `git diff` empty.
- All Rust (`src-tauri/`, `crates/`) — no changes; `cargo test` not required per plan.
- `useProfileCrud.ts`, `theme.css` — no changes needed (existing button/panel classes sufficed).

## Validation results (all 5 levels)

| Level            | Command                                                                                                        | Result                                                                                                                                               |
| ---------------- | -------------------------------------------------------------------------------------------------------------- | ---------------------------------------------------------------------------------------------------------------------------------------------------- |
| 1 Static         | `npm run typecheck` (app + test tsconfigs)                                                                     | ✅ zero errors                                                                                                                                       |
| 1 Static         | `./scripts/lint.sh`                                                                                            | ✅ zero errors in touched files (remaining 5 warnings / 26 infos are pre-existing in untouched files)                                                |
| 2 Unit           | `npm test -- OnboardingWizard profileCreateSeed useProfileSummaries HeroProfileCardList HeroDetailProfilesTab` | ✅ all pass                                                                                                                                          |
| 3 Build/contract | a11y modal suite `npm test -- modals.a11y`                                                                     | ✅ 3/3 (dialog contract unchanged)                                                                                                                   |
| 4 Integration    | full frontend suite `npm test`                                                                                 | ✅ 52 files / 417 tests (was 50/364 at baseline; +53 tests added)                                                                                    |
| 5 Edge/guards    | `grep` for `callCommand('profile_save'`/`invoke('profile_save'` in `src/components/`                           | ✅ no matches (single persistence path preserved; the lone grep hit on the raw word is a pre-existing doc comment in `HeroProfileActionsBar.tsx:10`) |
| 5 Edge/guards    | dependency guard `git diff package*.json`                                                                      | ✅ empty                                                                                                                                             |
| 5 Edge/guards    | host-gateway + legacy-palette checks (lint.sh)                                                                 | ✅ pass                                                                                                                                              |

## Deviations from plan

1. **`OnboardingWizard.tsx` at 511 lines** (plan target ≤ ~510) — 1 line over after lint suppressions; within the repo's soft-cap lesson (seed logic fully externalized to `profileCreateSeed.ts`; no clean further seam).
2. **Lint follow-ups during Task 6**: implementor agents emitted `eslint-disable` comments (repo uses Biome) and two unused imports; replaced with the repo's `// biome-ignore lint/correctness/useExhaustiveDependencies: trigger-only dep …` convention and removed the imports. Import-order auto-fixes applied via `biome check --write`.
3. **`theme.css` not touched** — the empty-state CTA reuses existing `crosshook-panel` / `crosshook-button--primary` classes (plan allowed this file "only if needed").

## Acceptance criteria coverage

All nine ACs from #487 covered per the plan's coverage map; headline AC (create without navigating to `/profiles`) proven by the `HeroDetailProfilesTab` integration test plus the `useProfileSummaries` refresh test.

## Next steps

- Open PR: `feat(library): hero detail create-profile wizard flow (#487)` — `Part of #478`, `Closes #487`; labels `type:feature`, `area:profiles`, `area:ui`, `priority:high`.
- Unblocks #473 (Phase 8), #474 (Phase 9), #475 (Phase 10 route removal).
