# Implementation Report: Extract Remaining Component `callCommand()` Usage Into Hooks (Issue #174)

## Summary

Implemented issue #174 by adding `useLaunchPrefixDependencyGate` for Launch page prefix-dependency IPC (`get_dependency_status`, `install_prefix_dependency`), extending `useAcknowledgeVersionChange` to return structured outcomes (including a silent `busy` path for in-flight guards), and refactoring `LaunchPage` and `ProfileActions` to consume hooks only—preserving `callCommand` for out-of-scope `check_gamescope_session` on Launch.

## Assessment vs Reality

| Metric | Predicted (Plan) | Actual |
| --- | --- | --- |
| Complexity | Medium | Medium |
| Confidence | — | High |
| Files Changed | 4–6 | 5 (4 updated + 1 new hook) |

## Tasks Completed

| # | Task | Status | Notes |
| --- | --- | --- | --- |
| 1 | Preflight and contract freeze | Complete | Scope: LaunchPage dep gate + ProfileActions acknowledge |
| 2 | Introduce/adapt hooks | Complete | New `useLaunchPrefixDependencyGate`; `useAcknowledgeVersionChange` returns `AcknowledgeVersionChangeOutcome` |
| 3 | Refactor `ProfileActions` | Complete | No `callCommand`; alerts preserved via outcome branches |
| 4 | Refactor `LaunchPage` | Complete | Dep IPC via hook; `check_gamescope_session` unchanged |
| 5 | Verification | Complete | `npm run build` + `cargo test -p crosshook-core` pass; smoke N/A (browser binaries missing locally) |

## Validation Results

| Level | Status | Notes |
| --- | --- | --- |
| Static Analysis | Pass | `cd src/crosshook-native && npm run build` |
| Unit Tests (Rust) | Pass | `cargo test --manifest-path src/crosshook-native/Cargo.toml -p crosshook-core` |
| Build | Pass | Vite production build |
| Integration / Smoke | Not run (env) | Playwright failed: Chromium executable not installed (`npx playwright install` required) |
| Edge Cases | Manual | Checklist left for author in native/browser dev |

## Files Changed

| File | Action |
| --- | --- |
| `src/crosshook-native/src/hooks/useLaunchPrefixDependencyGate.ts` | CREATED |
| `src/crosshook-native/src/hooks/useAcknowledgeVersionChange.ts` | UPDATED |
| `src/crosshook-native/src/components/pages/LaunchPage.tsx` | UPDATED |
| `src/crosshook-native/src/components/ProfileActions.tsx` | UPDATED |
| `.claude/PRPs/plans/completed/issue-174-hook-extraction.plan.md` | ARCHIVED (moved from `plans/`) |

## Deviations from Plan

- **`useAcknowledgeVersionChange`**: Replaced silent `catch` with structured outcomes. `LaunchPanel` still `await`s the function and ignores the return value (behavior compatible; acknowledge errors are now surfaced as return values instead of being swallowed—LaunchPanel still does not show alerts).
- **Targeted `rg` note**: `callCommand\(` does not match `callCommand<boolean>(` for `check_gamescope_session`; use `rg callCommand` or `rg 'callCommand'` if a literal paren match is required.

## Issues Encountered

- **Smoke tests**: Failed in this environment due to missing Playwright browser binaries, not due to application code.

## Tests Written

None (plan scoped out frontend test framework; Rust suite unchanged).

## Next Steps

- [ ] Run `npx playwright install` (or `npm run test:smoke:install`) and re-run `npm run test:smoke` locally or in CI.
- [ ] Manual parity: Profiles “Mark as Verified”, Launch dep gate modal/auto-install, native + `./scripts/dev-native.sh --browser`.
- [ ] Code review and PR linking `Closes #174`.
