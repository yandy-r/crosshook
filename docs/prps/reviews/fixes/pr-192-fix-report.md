# Fix Report: PR #192 Review Findings

**Source review**: `docs/prps/reviews/pr-192-review.md`
**Date**: 2026-04-09
**Severity threshold**: LOW (fixed CRITICAL + HIGH + MEDIUM + LOW; skipped NITPICK)

## Summary

| Metric                    | Value                                      |
| ------------------------- | ------------------------------------------ |
| Findings processed        | 9                                          |
| Fixed                     | 8                                          |
| Skipped (below threshold) | 1 (F9 — nitpick)                           |
| Failed                    | 0                                          |
| Validation                | TS typecheck pass, cargo test 777/777 pass |

## Batch Execution

### Batch 1 — Independent files (parallel)

| Finding | Severity | File(s)                         | Status | Agent      |
| ------- | -------- | ------------------------------- | ------ | ---------- |
| F2      | Medium   | `.github/workflows/release.yml` | Fixed  | fixer-f2   |
| F4      | Medium   | `request.rs:323`                | Fixed  | fixer-f4f5 |
| F5      | Medium   | `request.rs:1752`               | Fixed  | fixer-f4f5 |

**Validation gate**: TS typecheck pass, cargo test 777/777 pass.

### Batch 2 — TS refactoring (parallel)

| Finding | Severity | File(s)                                      | Status | Agent        |
| ------- | -------- | -------------------------------------------- | ------ | ------------ |
| F1      | High     | `mapValidationToNode.ts:27`                  | Fixed  | fixer-f1f3f8 |
| F3      | Medium   | `mapValidationToNode.ts` + `LaunchPanel.tsx` | Fixed  | fixer-f1f3f8 |
| F8      | Low      | `types/launch.ts` + `mapValidationToNode.ts` | Fixed  | fixer-f1f3f8 |
| F7      | Low      | `derivePipelineNodes.ts:149`                 | Fixed  | fixer-f7     |

**Validation gate**: TS typecheck pass, cargo test 777/777 pass.

### Finding 6 — Verified (no code change)

- **Severity**: Medium (Performance)
- **Resolution**: Verified that `profile` is referentially stable. It comes from `useProfileContext()` → `ProfileProvider` where the context value is memoized via `useMemo` (line 59 of `ProfileContext.tsx`). The `profile` reference only changes on actual profile mutations, not parent re-renders. The `useMemo` in `LaunchPipeline` is effective.

## Fix Details

### F1 — `low_disk_space_advisory` mapped to wrong node (HIGH)

Removed `|| code.startsWith('low_disk_space')` from the `wine-prefix` rule in `mapValidationToNode.ts:27`. The code now falls through to `return 'launch'` default, matching the spec's authoritative mapping table.

### F2 — Mock sentinel not in CI check (MEDIUM)

Added `__MOCK_VALIDATION_ERROR__` to the grep alternation pattern and the sentinel-strings-checked echo line in `.github/workflows/release.yml`.

### F3 — `sortIssuesBySeverity` duplicated (MEDIUM)

Exported `SEVERITY_RANK` and `sortIssuesBySeverity` from `mapValidationToNode.ts`. Replaced the local copy in `LaunchPanel.tsx` with an import. `sortPatternMatchesBySeverity` (different type) stays local.

### F4 — Cross-boundary contract undocumented (MEDIUM)

Added a doc comment above `ValidationError::code()` in `request.rs` documenting the frontend coupling to `mapValidationToNode.ts`. Added a reciprocal `@see` comment in `mapValidationToNode.ts`.

### F5 — Rust test coverage gaps (MEDIUM)

Added three `assert_eq!` blocks to `validation_error_codes_are_populated` covering `UnsupportedMethod`, `OfflineReadinessInsufficient`, and `LowDiskSpaceAdvisory` variants.

### F7 — `buildLaunchNode` unnecessarily nullable param (LOW)

Narrowed `issuesByNode` from `Map | null` to `Map` in `buildLaunchNode`. Removed optional chaining. Added non-null assertion at the call site (guaranteed non-null in the `preview && id === 'launch'` branch).

### F8 — `PipelineNode.id` typed as `string` (LOW)

Moved `PipelineNodeId` to `types/launch.ts` as the canonical definition. Narrowed `PipelineNode.id` from `string` to `PipelineNodeId`. Re-exported from `mapValidationToNode.ts` to preserve existing import sites.

## Skipped

### F9 — Redundant `as const` on mock severity fields (NITPICK)

Below severity threshold. Remains `Status: Open` in the source review file.

## Files Modified

| File                                                               | Findings       |
| ------------------------------------------------------------------ | -------------- |
| `src/crosshook-native/src/utils/mapValidationToNode.ts`            | F1, F3, F4, F8 |
| `src/crosshook-native/src/components/LaunchPanel.tsx`              | F3             |
| `src/crosshook-native/src/types/launch.ts`                         | F8             |
| `src/crosshook-native/src/utils/derivePipelineNodes.ts`            | F7             |
| `src/crosshook-native/crates/crosshook-core/src/launch/request.rs` | F4, F5         |
| `.github/workflows/release.yml`                                    | F2             |
| `docs/prps/reviews/pr-192-review.md`                               | Status updates |
