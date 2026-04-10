# Code Review: PR #194 — feat(ui): add launch pipeline phase 4 polish & accessibility

**PR**: [yandy-r/crosshook#194](https://github.com/yandy-r/crosshook/pull/194)
**Branch**: `feat/launch-pipeline-phase4-polish` -> `main`
**Author**: yandy-r
**Reviewed**: 2026-04-09
**Mode**: Parallel (3 specialized reviewers: correctness, security, quality)

## Summary

Final phase (Phase 4) of the launch pipeline visualization feature (#74). Adds `aria-live` status
announcements, Radix tooltips on detail-bearing nodes, active/error connector coloring via
`color-mix()` CSS tokens, not-configured contrast bump toward WCAG 1.4.11, vertical connectors at
<=640px, a `__MOCK_VALIDATION_WARNING__` sentinel for warning-severity fixtures, CI sentinel
update, and Playwright test coverage (6 dedicated pipeline tests + 1 smoke test).

**Scope**: 9 files, +364/-9

## Validation Results

| Check                          | Status                    |
| ------------------------------ | ------------------------- |
| `npx tsc --noEmit`             | Pass (zero errors)        |
| `cargo test -p crosshook-core` | Pass (4/4 tests)          |
| `npm run build`                | Pass (per PR description) |

## Decision: APPROVE

No critical or high-severity issues. Five medium findings are maintainability and completeness
concerns, not runtime defects. The accessibility additions are well-structured (`aria-live` pattern,
keyboard-accessible tooltips, focus-visible outlines). Security properties are sound: no XSS path,
mock code tree-shaken behind `__WEB_DEV_MODE__`, CI sentinel catches new mock string. CSS additions
follow existing conventions and are WebKitGTK-compatible.

---

## Findings

### F-01: `crosshook-visually-hidden` class may not ship in production builds

- **Severity**: medium
- **Status**: Fixed
- **File**: `src/crosshook-native/src/components/LaunchPipeline.tsx:119`
- **Category**: Correctness
- **Finding**: The `aria-live` div uses `className="crosshook-visually-hidden"`, which is defined
  in `src/lib/dev-indicator.css` — a file loaded alongside the `DevModeBanner` component that is
  conditionally rendered only in dev mode. If `DevModeBanner` and its CSS are tree-shaken in
  production Tauri builds, the `crosshook-visually-hidden` class definition will be absent, and the
  aria-live div will be visually visible instead of hidden.
- **Suggestion**: Move the `crosshook-visually-hidden` utility class to a global CSS file (e.g.,
  `styles/utilities.css` imported from the app root) so it ships in all builds. Alternatively,
  verify via build inspection that `dev-indicator.css` is unconditionally included.

### F-02: Warning-severity fixture produces no observable pipeline change

- **Severity**: medium
- **Status**: Fixed
- **File**: `src/crosshook-native/src/lib/mocks/handlers/launch.ts:265`
- **Category**: Completeness
- **Finding**: The `__MOCK_VALIDATION_WARNING__` fixture is wired in the mock handler but
  `derivePipelineNodes` only promotes `fatal`-severity issues to `error` status. A `warning`-severity
  issue leaves the trainer node as `configured` with no visual or accessible indication of the
  warning in the pipeline. The fixture exists but the display path is a no-op — users get zero
  pipeline feedback for warning-severity issues.
- **Suggestion**: If warning display in the pipeline is intentionally deferred, add a comment in
  `derivePipelineNodes.ts` near `buildTier2Node` documenting the gap and link to a tracking issue.
  If it should be visible, add a `warning` tone/indicator path.

### F-03: No Playwright test exercises the `__MOCK_VALIDATION_WARNING__` fixture

- **Severity**: medium
- **Status**: Fixed
- **File**: `src/crosshook-native/tests/pipeline.spec.ts`
- **Category**: Completeness
- **Finding**: The mock handler adds a full warning fixture path, the CI sentinel is updated, but
  no E2E test navigates to the warning fixture to verify the pipeline renders without errors and
  that the trainer node retains `configured` status despite the warning. The fixture is dead test
  infrastructure at the E2E level.
- **Suggestion**: Add a test block that sets the game path to `__MOCK_VALIDATION_WARNING__` and
  asserts the trainer node has `data-status="configured"` and no console errors occur.

### F-04: `attachConsoleCapture` duplicated across three test files

- **Severity**: medium
- **Status**: Fixed
- **Files**: `tests/pipeline.spec.ts:7`, `tests/smoke.spec.ts:54`, `tests/collections.spec.ts:20`
- **Category**: Maintainability
- **Finding**: The `ConsoleCapture` interface and `attachConsoleCapture()` function are now
  byte-for-byte identical in three test files. `collections.spec.ts` already has a comment
  acknowledging this duplication.
- **Suggestion**: Extract to `tests/helpers.ts` and import from all three spec files.

### F-05: Inline `color-mix()` on waiting connector breaks token pattern

- **Severity**: medium
- **Status**: Fixed
- **File**: `src/crosshook-native/src/styles/launch-pipeline.css:164`
- **Category**: Pattern Compliance
- **Finding**: The active/waiting connector uses a raw `color-mix()` expression directly in the
  rule body while all other connector colors (`connector-success`, `connector-active`,
  `connector-error`) are defined as tokens in `variables.css`. This breaks the established pattern
  and means future theme changes must find and update an inline expression.
- **Suggestion**: Define `--crosshook-color-pipeline-connector-waiting` in `variables.css`
  alongside the other three connector tokens and reference it here.

### F-06: Tooltip trigger `<span>` lacks `aria-label` — accessible name at focus is unclear

- **Severity**: low
- **Status**: Fixed
- **File**: `src/crosshook-native/src/components/LaunchPipeline.tsx:86`
- **Category**: Accessibility
- **Finding**: The `<li>` carries `aria-label={...}` but is not focusable. The inner `<span>`
  trigger with `tabIndex={0}` is the keyboard target, but has no `aria-label`. When a screen reader
  focuses the span, the announced name comes from Radix's internal wiring rather than the
  carefully constructed node label. Compare to `InfoTooltip.tsx` which puts `aria-label` directly
  on the trigger.
- **Suggestion**: Add `aria-label={...}` to the trigger span:
  `<span className="..." tabIndex={0} aria-label={\`${node.label}: ${statusText}\`}>`

### F-07: Vertical connector `::before` uses hardcoded `left: 14px` misaligned at narrow widths

- **Severity**: low
- **Status**: Fixed
- **File**: `src/crosshook-native/src/styles/launch-pipeline.css:306`
- **Category**: Correctness
- **Finding**: At <=640px the `::before` connector is at `left: 14px` (center of 28px indicator),
  but the 1023px breakpoint also applies and shrinks the indicator to 22px — so the center should be
  11px, making the connector ~3px off-center from the indicator.
- **Suggestion**: Use `left: 11px` to match the 22px indicator at narrow widths, or use a CSS
  custom property for the indicator half-width.

### F-08: Inline style objects on `Tooltip.Content` recreated every render

- **Severity**: low
- **Status**: Fixed
- **File**: `src/crosshook-native/src/components/LaunchPipeline.tsx:94-105`
- **Category**: Performance
- **Finding**: The style object literals on `Tooltip.Content` and `Tooltip.Arrow` are created on
  every render pass. The values are static. While the practical performance impact is negligible
  (6 nodes max), hoisting to module scope is cleaner and avoids per-render allocations.
- **Suggestion**: Extract to module-level constants:

  ```tsx
  const TOOLTIP_CONTENT_STYLE: React.CSSProperties = { maxWidth: 280, ... };
  const TOOLTIP_ARROW_STYLE: React.CSSProperties = { fill: '...' };
  ```

### F-09: `__MOCK_VALIDATION_WARNING__` block is near-verbatim duplicate of populated path

- **Severity**: low
- **Status**: Fixed
- **File**: `src/crosshook-native/src/lib/mocks/handlers/launch.ts:265-309`
- **Category**: Maintainability
- **Finding**: The warning fixture constructs a `LaunchPreview` structurally identical to the
  populated path (lines 312-346) with only `validation.issues`, `wrappers`, and `display_text`
  differing. ~30 lines of duplication.
- **Suggestion**: Build the populated preview first, then construct the warning variant via spread:
  `return { ...preview, validation: { issues: [...] }, display_text: '...' };`

### F-10: `crosshook-color-surface-raised` token referenced but undefined

- **Severity**: nit
- **Status**: Open
- **File**: `src/crosshook-native/src/components/LaunchPipeline.tsx:101,108`
- **Category**: Pattern Compliance
- **Finding**: `var(--crosshook-color-surface-raised, #2a2a2e)` is used with a hardcoded fallback
  but the token is not defined in `variables.css`. Pre-existing pattern from `InfoTooltip.tsx`, not
  introduced by this PR, but now referenced in two components.
- **Suggestion**: Register `--crosshook-color-surface-raised: #2a2a2e;` in `variables.css`.

### F-11: `page.waitForTimeout(1000)` in pipeline test is fragile

- **Severity**: nit
- **Status**: Open
- **File**: `src/crosshook-native/tests/pipeline.spec.ts:101`
- **Category**: Test Quality
- **Finding**: The "no console errors" test uses an unconditional 1-second sleep. Playwright's
  event-driven waiters are preferred for deterministic assertions.
- **Suggestion**: Replace with `await expect(page.locator('.crosshook-launch-pipeline__node')).toHaveCount(6);`

### F-12: Duplicate smoke test overlaps with `pipeline.spec.ts`

- **Severity**: nit
- **Status**: Open
- **File**: `src/crosshook-native/tests/smoke.spec.ts:105-119`
- **Category**: Test Quality
- **Finding**: The `launch pipeline smoke` block in `smoke.spec.ts` tests visibility and node count,
  which `pipeline.spec.ts` also covers. Both run on every PR. When node count changes, two files
  need updating.
- **Suggestion**: Either remove the smoke block (deferring to `pipeline.spec.ts`) or scope it to
  just a visibility check without the count assertion.

---

## Summary Table

| ID   | Severity | Category           | File                                     |
| ---- | -------- | ------------------ | ---------------------------------------- |
| F-01 | medium   | Correctness        | `LaunchPipeline.tsx:119`                 |
| F-02 | medium   | Completeness       | `launch.ts:265`                          |
| F-03 | medium   | Completeness       | `pipeline.spec.ts`                       |
| F-04 | medium   | Maintainability    | `pipeline.spec.ts:7`, `smoke.spec.ts:54` |
| F-05 | medium   | Pattern Compliance | `launch-pipeline.css:164`                |
| F-06 | low      | Accessibility      | `LaunchPipeline.tsx:86`                  |
| F-07 | low      | Correctness        | `launch-pipeline.css:306`                |
| F-08 | low      | Performance        | `LaunchPipeline.tsx:94-105`              |
| F-09 | low      | Maintainability    | `launch.ts:265-309`                      |
| F-10 | nit      | Pattern Compliance | `LaunchPipeline.tsx:101,108`             |
| F-11 | nit      | Test Quality       | `pipeline.spec.ts:101`                   |
| F-12 | nit      | Test Quality       | `smoke.spec.ts:105-119`                  |

**Critical: 0 | High: 0 | Medium: 5 | Low: 4 | Nit: 3**

## Security Assessment

- **XSS/Injection**: Pass — all text rendered as React children (no `dangerouslySetInnerHTML`)
- **Mock leakage**: Pass — `__MOCK_VALIDATION_WARNING__` gated behind `__WEB_DEV_MODE__` define;
  CI sentinel grep pattern updated correctly
- **CI/CD safety**: Pass — sentinel grep catches string literals that survive minification
- **Dependencies**: Pass — `@radix-ui/react-tooltip` already installed, no new deps
