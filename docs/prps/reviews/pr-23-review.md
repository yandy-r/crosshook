# Code Review — PR #23: fix(profiles): persist custom env var edits from the launch options page

- **Mode**: PR review (`/ycc:code-review --parallel`)
- **PR**: https://git.home.rfamily.dev/yandy/crosshook/pulls/23 (Closes #21)
- **Branch**: `fix/env-var-autosave-persistence`
- **Reviewers**: correctness-reviewer, security-reviewer, quality-reviewer (3 parallel sub-agents, findings merged and de-duplicated)
- **Validation**: `npx vitest run` (full suite pass), `npm run typecheck` (clean), `npx biome check` on changed files (clean)
- **Decision**: APPROVE (after fixes below applied on-branch)

## Findings

### PR23-C1 — Out-of-order persist resolution can stomp the dedupe baseline

- **Severity**: major (Correctness)
- **File**: `src/crosshook-native/src/hooks/profile/useLaunchEnvironmentAutosave.ts`
- **Status**: Fixed
- **Issue**: The persist success handler updated `lastPersistedEnvSignatureRef` on any `{ok:true}` resolution. Two overlapping persists can resolve out of invocation order (the `persistProfileDraft` chain awaits metadata sync/refresh/reload after the disk write), so an older resolution could overwrite the ref back to a stale signature; a later edit reverting to that value would then be wrongly deduped and silently never persisted. Empirically reproduced by the reviewer with deferred promises.
- **Fix applied**: `persistRequestSeqRef` monotonic counter; only the most recently scheduled persist for the still-current profile may update the baseline. Covered by a new out-of-order regression test.

### PR23-S1 — Forced full-profile persist on the first no-op blur after a profile switch

- **Severity**: major (Performance)
- **File**: `src/crosshook-native/src/hooks/profile/useLaunchEnvironmentAutosave.ts`
- **Status**: Fixed
- **Issue**: Resetting the baseline to `null` on `profileName` change made the first env-field blur after every profile switch fire a full `persistProfileDraft` (profile_save IPC + metadata sync + profile list refresh + reload) even with zero edits — and the draft is the whole live profile, so it could also commit unrelated unsaved edits.
- **Fix applied**: The baseline is now seeded from the newly loaded profile's `custom_env_vars` signature (and the request sequence is bumped to invalidate in-flight persists from the previous profile). New tests: no-op blur after switch does not persist; a real change does.

### PR23-C2 — Missing test coverage for overlapping persists

- **Severity**: minor (Correctness / test completeness)
- **File**: `src/crosshook-native/src/hooks/profile/__tests__/useLaunchEnvironmentAutosave.test.ts`
- **Status**: Fixed
- **Issue**: All original tests awaited each persist to completion before the next blur, so the PR23-C1 race was unexercised.
- **Fix applied**: Added a deferred-promise test resolving two overlapping persists out of order and asserting a subsequent revert-edit still persists.

### PR23-Q2 — Duplicated env-var signature helpers (pre-existing)

- **Severity**: minor (Maintainability / DRY)
- **Files**: `src/crosshook-native/src/hooks/profile/useLaunchEnvironmentAutosave.ts`, `src/crosshook-native/src/components/CustomEnvironmentVariablesSection.tsx`
- **Status**: Fixed
- **Issue**: `envVarSignature` (hook) and `customEnvRecordSignature` (section) implemented the same sorted-entry JSON signature. Pre-existing duplication, not introduced by this PR.
- **Fix applied**: Extracted a single shared helper `src/crosshook-native/src/utils/envVarSignature.ts`; both call sites now import it.

### PR23-Q1 — Divergent profile-switch reset mechanism vs sibling autosave hooks

- **Severity**: minor (Pattern compliance)
- **File**: `src/crosshook-native/src/hooks/profile/useLaunchEnvironmentAutosave.ts`
- **Status**: No change needed
- **Issue**: Sibling autosave paths reset their `lastSaved*` refs through the `setLastSavedProfileSnapshot` chain; this hook uses a `profileName`-keyed effect. Justified by its standalone wiring in `useLaunchSubTabsProps`; consolidation is only worthwhile if the hook is ever folded into the `useProfileCrud` composition.

## Security review

No findings. The signature ref is in-memory only, never logged or transported; no new IPC surface, injection vector, or serialization change. Env values already crossed `profile_save` before this PR.

## Post-fix validation

- `npx vitest run src/hooks/profile/__tests__/useLaunchEnvironmentAutosave.test.ts` → 8/8 pass
- `npx vitest run` (full) → 65 files, 497/497 pass
- `npm run typecheck` → clean
- `npx biome check` (4 changed files) → clean
