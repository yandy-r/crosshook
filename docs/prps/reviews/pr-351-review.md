# PR Review #351 — test(integration): Integrate Vitest and Playwright as CI jobs

**Reviewed**: 2026-04-19T13:39:00Z
**Mode**: PR
**Author**: Claude
**Branch**: claude/add-vitest-playwright-smoke-job -> main
**Decision**: REQUEST CHANGES

## Summary

The CI wiring is close, but the Playwright change currently fixes a flaky tooltip assertion by skipping the check entirely, which removes meaningful coverage from the launch-pipeline suite. The workflow also still skips the repo's broader frontend typecheck path, so the new test/config surface is not fully validated in CI.

Mock coverage drift validation passed locally on the PR head. Frontend typecheck, Vitest, and build also passed; the full smoke suite produced one flaky failure in `tests/collections.spec.ts` on the first run, and the targeted rerun of that single test passed.

## Findings

### CRITICAL

### HIGH

- **[F001]** `src/crosshook-native/tests/pipeline.spec.ts:74` — The new `test.skip(triggerCount === 0, ...)` turns a real tooltip regression into a green test instead of fixing the underlying wait condition.
  - **Status**: Open
  - **Category**: Correctness
  - **Suggested fix**: Wait for the Tier 2 preview state to settle and assert that at least one trigger appears, e.g. by waiting on `triggers.first()` or polling until the trigger count becomes non-zero before hovering.

### MEDIUM

- **[F002]** `.github/workflows/lint.yml:73` — The TypeScript CI job still runs `npx tsc --noEmit`, which skips the repo's dedicated test/config typecheck path (`tsconfig.test.json`), so the new Vitest/test-support surface is not fully type-checked in CI.
  - **Status**: Open
  - **Category**: Completeness
  - **Suggested fix**: Replace the step with `npm run typecheck`, or add a second `tsc -p tsconfig.test.json --noEmit` step so `vitest.config.ts`, `src/test/**`, and test files covered by that config are checked.

### LOW

## Validation Results

| Check      | Result                                                                                                                                                                                                  |
| ---------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| Type check | Pass (`npm run typecheck`)                                                                                                                                                                              |
| Lint       | Pass (`npm run lint`; emitted one existing Biome warning in `src/hooks/useAccessibilityEnhancements.ts:16`)                                                                                             |
| Tests      | Fail (`npm test` passed, `bash ../../scripts/check-mock-coverage.sh` passed, `npm run test:smoke` failed once in `tests/collections.spec.ts:135`; targeted rerun of that single Playwright test passed) |
| Build      | Pass (`npm run build`)                                                                                                                                                                                  |

## Files Reviewed

- `.github/workflows/lint.yml` (Modified)
- `.gitignore` (Modified)
- `scripts/check-mock-coverage.sh` (Modified)
- `src/crosshook-native/src/lib/mocks/handlers/system.ts` (Modified)
- `src/crosshook-native/tests/pipeline.spec.ts` (Modified)
