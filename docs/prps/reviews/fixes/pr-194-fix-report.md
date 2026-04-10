# Fix Report: PR #194 Review Findings

**Source**: `docs/prps/reviews/pr-194-review.md`
**Fixed**: 2026-04-09
**Mode**: Parallel (6 agents, 1 batch)
**Severity threshold**: LOW (fixed MEDIUM + LOW, skipped NIT)

## Validation

| Check                  | Result              |
| ---------------------- | ------------------- |
| `tsc --noEmit`         | Pass (zero errors)  |
| `cargo test -p crosshook-core` | Pass (3/3 tests) |

## Fixed Findings (9/12)

### F-01 (medium) -- `crosshook-visually-hidden` class may not ship in production

- **Action**: Moved `.crosshook-visually-hidden` from `dev-indicator.css` to new `styles/utilities.css`; imported globally in `main.tsx`; removed from `dev-indicator.css`.
- **Files**: `styles/utilities.css` (new), `main.tsx`, `lib/dev-indicator.css`

### F-02 (medium) -- Warning-severity fixture produces no observable pipeline change

- **Action**: Added 4-line documentation comment in `buildTier2Node` explaining that only `fatal` severity promotes to `error` status, referencing the `__MOCK_VALIDATION_WARNING__` fixture and #74.
- **Files**: `utils/derivePipelineNodes.ts`

### F-03 (medium) -- No Playwright test exercises the warning fixture

- **Action**: Added `warning-severity validation does not produce error nodes` test that navigates to `/?fixture=populated&gamePath=__MOCK_VALIDATION_WARNING__`, asserts zero error-status nodes, and asserts no console errors.
- **Files**: `tests/pipeline.spec.ts`

### F-04 (medium) -- `attachConsoleCapture` duplicated across three test files

- **Action**: Extracted `ConsoleCapture` interface and `attachConsoleCapture()` to new `tests/helpers.ts`; all three spec files now import from it.
- **Files**: `tests/helpers.ts` (new), `tests/pipeline.spec.ts`, `tests/smoke.spec.ts`, `tests/collections.spec.ts`

### F-05 (medium) -- Inline `color-mix()` on waiting connector breaks token pattern

- **Action**: Defined `--crosshook-color-pipeline-connector-waiting` token in `variables.css`; replaced inline `color-mix()` in `launch-pipeline.css` with the token reference.
- **Files**: `styles/variables.css`, `styles/launch-pipeline.css`

### F-06 (low) -- Tooltip trigger `<span>` lacks `aria-label`

- **Action**: Added `aria-label={`${node.label}: ${statusText}`}` to the focusable trigger span.
- **Files**: `components/LaunchPipeline.tsx`

### F-07 (low) -- Vertical connector `::before` misaligned at narrow widths

- **Action**: Changed `left: 14px` to `left: 11px` in the `<=640px` vertical connector `::before` rule to center on the 22px indicator.
- **Files**: `styles/launch-pipeline.css`

### F-08 (low) -- Inline style objects on `Tooltip.Content` recreated every render

- **Action**: Hoisted `TOOLTIP_CONTENT_STYLE` and `TOOLTIP_ARROW_STYLE` to module-level `CSSProperties` constants. Imported `CSSProperties` from `react` (project uses `jsx: react-jsx`).
- **Files**: `components/LaunchPipeline.tsx`

### F-09 (low) -- Warning fixture mock is near-verbatim duplicate of populated path

- **Action**: Built the populated `LaunchPreview` first as a shared `preview` variable; warning branch now spreads it and overrides only `validation.issues`, `wrappers`, and `display_text`.
- **Files**: `lib/mocks/handlers/launch.ts`

## Skipped Findings (3/12 -- nit severity)

| ID   | Finding                                          | Reason        |
| ---- | ------------------------------------------------ | ------------- |
| F-10 | `crosshook-color-surface-raised` token undefined | Below threshold |
| F-11 | `page.waitForTimeout(1000)` in test is fragile   | Below threshold |
| F-12 | Duplicate smoke test overlaps pipeline.spec.ts   | Below threshold |

## File Impact

```
11 files changed, -50 lines net
 components/LaunchPipeline.tsx        | 35 +++++++-----
 lib/dev-indicator.css                | 12 ----
 lib/mocks/handlers/launch.ts        | 66 +++++++---------------
 main.tsx                             |  1 +
 styles/launch-pipeline.css           |  4 +-
 styles/utilities.css                 | new
 styles/variables.css                 |  5 ++
 utils/derivePipelineNodes.ts         |  4 ++
 tests/helpers.ts                     | new
 tests/collections.spec.ts           | 20 +------
 tests/pipeline.spec.ts              | 41 ++++++++------
 tests/smoke.spec.ts                 | 20 +------
```
