# Implementation Report: Launch Pipeline Phase 2 (Preview-Derived Status)

## Summary

Implemented Tier 2 pipeline status: `ValidationError::code()` populates IPC issue codes; `mapValidationToNode` / `groupIssuesByNode` map codes to nodes; `derivePipelineNodes()` uses preview validation, `directives_error`, and resolved preview fields for `detail`; `LaunchPanel` passes live `preview`; `LaunchPipeline` shows `detail`, tooltips, and `aria-current` on first error or not-configured step; mock `preview_launch` returns validation fixtures when `game_path` is empty or `__MOCK_VALIDATION_ERROR__`. PRD Phase 2 marked complete.

## Assessment vs Reality

| Metric        | Predicted (Plan) | Actual |
| ------------- | ---------------- | ------ |
| Complexity    | Medium           | Medium |
| Confidence    | (plan default)   | High   |
| Files Changed | ~5               | 8      |

## Tasks Completed

| #   | Task                                                  | Status   | Notes                                                                                                                                |
| --- | ----------------------------------------------------- | -------- | ------------------------------------------------------------------------------------------------------------------------------------ |
| T1  | Rust `code()` + `issue()`                             | Complete | Updated existing unit test expecting `code: None`                                                                                    |
| T2  | `mapValidationToNode.ts`                              | Complete |                                                                                                                                      |
| T3  | Mock preview fixtures                                 | Complete |                                                                                                                                      |
| T4  | `derivePipelineNodes` Tier 2                          | Complete |                                                                                                                                      |
| T5  | Wire `preview` in `LaunchPanel`                       | Complete |                                                                                                                                      |
| T6  | Pipeline UI `detail`                                  | Complete |                                                                                                                                      |
| T7  | `aria-current` for errors                             | Complete |                                                                                                                                      |
| T8  | Rust unit test `validation_error_codes_are_populated` | Complete |                                                                                                                                      |
| T9  | Verification suite                                    | Partial  | `cargo test` + `npm run build` pass; `npm run test:smoke` not run (Playwright browser binary missing / CDN unreachable in agent env) |
| T10 | PRD Phase 2 update                                    | Complete |                                                                                                                                      |

## Validation Results

| Level            | Status          | Notes                                                |
| ---------------- | --------------- | ---------------------------------------------------- |
| Static Analysis  | Pass            | `tsc` via `npm run build`                            |
| Unit Tests       | Pass            | `cargo test -p crosshook-core` (777 tests)           |
| Build            | Pass            | Vite production build                                |
| Integration      | N/A             |                                                      |
| Edge Cases       | N/A             |                                                      |
| Playwright smoke | Not run locally | Requires `npx playwright install` and network to CDN |

## Files Changed

| File                                                                      | Action                               |
| ------------------------------------------------------------------------- | ------------------------------------ |
| `src/crosshook-native/crates/crosshook-core/src/launch/request.rs`        | UPDATED — `code()`, `issue()`, tests |
| `src/crosshook-native/src/utils/mapValidationToNode.ts`                   | CREATED                              |
| `src/crosshook-native/src/utils/derivePipelineNodes.ts`                   | UPDATED — Tier 2                     |
| `src/crosshook-native/src/components/LaunchPanel.tsx`                     | UPDATED — `preview` prop             |
| `src/crosshook-native/src/components/LaunchPipeline.tsx`                  | UPDATED — detail, aria, title        |
| `src/crosshook-native/src/lib/mocks/handlers/launch.ts`                   | UPDATED — validation fixture branch  |
| `docs/prps/prds/launch-pipeline-visualization.prd.md`                     | UPDATED — Phase 2 complete           |
| `docs/prps/plans/completed/launch-pipeline-phase2-preview-status.plan.md` | ARCHIVED                             |

## Deviations from Plan

- **Smoke tests**: Not executed here; environment lacked Playwright Chromium install (and `playwright install` failed with DNS `EAI_AGAIN` to `cdn.playwright.dev`). Run `npm run test:smoke` locally after `npx playwright install`.
- **Existing Rust test**: `validation_error_issue_packages_message_help_and_severity` updated to expect `code: Some("unsupported_method")` instead of `None` (behavior change is intentional).

## Issues Encountered

- One existing test asserted `code: None` on `issue()`; updated to match populated codes.

## Tests Written

| Test File                                             | Tests | Coverage                                     |
| ----------------------------------------------------- | ----- | -------------------------------------------- |
| `request.rs` (`validation_error_codes_are_populated`) | 1     | `ValidationError::code()` and `issue().code` |

## Next Steps

- [ ] Run `npx playwright install` and `npm run test:smoke` in a normal dev environment
- [ ] `/ycc:code-review` or manual review
- [ ] Commit and open PR (`Closes #188`)
