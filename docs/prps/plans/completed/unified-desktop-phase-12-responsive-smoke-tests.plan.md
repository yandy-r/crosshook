# Phase 12 — Responsive Smoke Tests + Sweep Expansion

**Issue**: Closes #424, Part of #451  
**PRD**: `docs/prps/prds/unified-desktop-redesign.prd.md` § Phase 12  
**Success signal**: `npm run test:smoke` green with 4× breakpoint × route sweep, context-rail UW gate, and deck status-bar coverage

---

## Context

Phase 12 extends the existing Playwright smoke suite to guard the responsive contract
introduced in Phases 1–11. Research against the current `smoke.spec.ts` (348 lines)
confirms the following PRD requirements are already satisfied:

| PRD Requirement                                     | Status     | Evidence                                                                   |
| --------------------------------------------------- | ---------- | -------------------------------------------------------------------------- |
| `host-tools` + `proton-manager` in `ROUTE_ORDER`    | ✅ Done    | `smoke.spec.ts:39-40`                                                      |
| `⌘K open/close` smoke                               | ✅ Done    | `command palette smoke` describe, L253-291                                 |
| `Library → Detail → Library` smoke                  | ✅ Done    | `hero detail opens and Back returns…`, L151-167                            |
| `inspector-present-on-non-deck` smoke               | ✅ Done    | `inspector shows after selecting a card…`, L119-136                        |
| `status-bar-only-on-deck` smoke (deck < 1100px)     | ⚠️ Partial | Existing test at 1280px is `narrow` breakpoint; deck case (< 1100) missing |
| `context-rail-present-only-on-uw` smoke             | ❌ Missing | No `context-rail` testid referenced anywhere in the spec                   |
| Breakpoint sweep 1280/1920/2560/3440 for all routes | ❌ Missing | No multi-viewport route iteration exists                                   |

Three additive changes to `src/crosshook-native/tests/smoke.spec.ts` land Phase 12
fully. No other files change.

---

## Key Invariants

**Context rail gate** (`contextRailVariants.ts:22,35`):

- Visible only when `route === 'library'` AND `libraryMode !== 'detail'` AND `viewportWidth >= 3400`
- `CONTEXT_RAIL_MIN_VIEWPORT_WIDTH = 3400` — product acceptance: 3440 shows, 2560 hides
- Both 3440 and 2560 map to the `uw` breakpoint bucket (≥2200); the gate uses **raw pixel width**, not the bucket
- `data-testid="context-rail"` is set on the `<aside>` root in `ContextRail.tsx:92`
- When hidden the component is not mounted at all → use `.toHaveCount(0)` (not `.not.toBeVisible()`)

**Deck breakpoint** (`useBreakpoint.ts:BREAKPOINTS`):

- `deck` = `width < 1100`; `narrow` = `1100 ≤ width < 1440`
- 1280px (used by existing console chrome test) is **`narrow`**, not `deck`
- Use `1024×800` for deck-width tests — already the established convention in `library inspector` tests at L140,172

**Console-drawer mode condition** (`AppShell.tsx:77`):

- `breakpoint.isDeck || breakpoint.isNarrow || breakpoint.height <= COMPACT_CONSOLE_MAX_HEIGHT ? 'status' : 'drawer'`
- Status bar shows at both `deck` and `narrow` widths; the existing `narrow` test at 1280 validates the narrow arm; deck arm (1024) is missing

**Fixture default state** (`/?fixture=populated`):

- App loads to `library` route, `libraryMode = 'grid'` (not `'detail'`)
- Context rail visible at 3440 without clicking any card (default route + mode satisfies the gate)

**Serial execution** (`playwright.config.ts`): `fullyParallel: false, workers: 1` — no config changes needed

**Screenshot naming convention**: `test-results/smoke-${route}.png` (existing);
extend to `test-results/smoke-${route}-${width}x${height}.png` for the sweep

**Breakpoint mapping for the four PRD sweep widths**:

| Viewport  | Bucket   | Context rail                             |
| --------- | -------- | ---------------------------------------- |
| 1280×800  | `narrow` | hidden                                   |
| 1920×1080 | `desk`   | hidden                                   |
| 2560×1440 | `uw`     | hidden (2560 < 3400)                     |
| 3440×1440 | `uw`     | visible on `library` route (3440 ≥ 3400) |

---

## Worktree Setup

```bash
git worktree add ~/.claude-worktrees/crosshook-phase-12-smoke main
cd ~/.claude-worktrees/crosshook-phase-12-smoke/src/crosshook-native
npm install -D --no-save typescript biome
```

Run smoke tests from `src/crosshook-native/`: `npm run test:smoke`

---

## Tasks

All tasks modify a single file sequentially.

### Task 1 — Add `context rail smoke` describe block

**File**: `src/crosshook-native/tests/smoke.spec.ts`  
**Worktree**: `~/.claude-worktrees/crosshook-phase-12-smoke`  
**Depends on**: nothing (first edit, append to end of file)

Append a new `test.describe('context rail smoke', ...)` block at the end of the file.

```ts
test.describe('context rail smoke', () => {
  test('visible on library route at ultrawide (3440×1440)', async ({ page }) => {
    const capture = attachConsoleCapture(page);
    await page.setViewportSize({ width: 3440, height: 1440 });
    await page.goto('/?fixture=populated');
    const libraryTab = page.getByRole('tab', { name: 'Library', exact: true });
    await libraryTab.click();
    await expect(libraryTab).toHaveAttribute('aria-current', 'page');
    await expect(page.getByTestId('context-rail')).toBeVisible();
    expect(capture.errors, `Context rail UW errors:\n${capture.errors.join('\n')}`).toEqual([]);
  });

  test('absent on library route below CONTEXT_RAIL_MIN_VIEWPORT_WIDTH (2560×1440)', async ({ page }) => {
    const capture = attachConsoleCapture(page);
    await page.setViewportSize({ width: 2560, height: 1440 });
    await page.goto('/?fixture=populated');
    const libraryTab = page.getByRole('tab', { name: 'Library', exact: true });
    await libraryTab.click();
    await expect(libraryTab).toHaveAttribute('aria-current', 'page');
    await expect(page.locator('[data-testid="context-rail"]')).toHaveCount(0);
    expect(capture.errors, `Context rail sub-UW errors:\n${capture.errors.join('\n')}`).toEqual([]);
  });
});
```

Why `toHaveCount(0)` for the absent case: `ContextRail` is conditionally mounted via
`{contextRailLayout.visible && contextRailLayout.width > 0 ? <Panel>...</Panel> : null}`
in `AppShell.tsx:412`. When hidden, the element is absent from the DOM entirely —
`.not.toBeVisible()` would hang. Matches the inspector pattern at `smoke.spec.ts:147`.

---

### Task 2 — Add deck-width test to `console chrome smoke`

**File**: `src/crosshook-native/tests/smoke.spec.ts`  
**Worktree**: `~/.claude-worktrees/crosshook-phase-12-smoke`  
**Depends on**: Task 1 (same file, sequential)

Insert one new test inside the existing `console chrome smoke` describe block, after
the two existing tests (after the closing brace of `keeps the drawer collapsed…` at
line 347, before the `});` closing the describe at line 348).

```ts
test('renders the compact status bar at deck width (1024×800)', async ({ page }) => {
  const capture = attachConsoleCapture(page);
  await page.setViewportSize({ width: 1024, height: 800 });
  await page.goto('/?fixture=populated');
  await expect(page.getByTestId('console-status-bar')).toBeVisible();
  await expect(page.getByTestId('console-drawer')).toHaveCount(0);
  await expect(page.getByText('⌘K commands')).toBeVisible();
  expect(capture.errors, `Deck console chrome errors:\n${capture.errors.join('\n')}`).toEqual([]);
});
```

`data-testid` values: `"console-status-bar"` (`ConsoleDrawer.tsx:65`), `"console-drawer"` (`ConsoleDrawer.tsx:166`).

Note: 1024 < 1100 → `deck` breakpoint. Mirrors the existing narrow test at L294-305
(which uses 1280). Together the two tests prove status-bar is shown and drawer is absent
at both `narrow` and `deck` widths, fully satisfying `status-bar-only-on-deck`.

---

### Task 3 — Add `responsive breakpoint sweep` describe block

**File**: `src/crosshook-native/tests/smoke.spec.ts`  
**Worktree**: `~/.claude-worktrees/crosshook-phase-12-smoke`  
**Depends on**: Task 2 (same file, sequential)

Add a `SWEEP_VIEWPORTS` constant and a new `test.describe('responsive breakpoint sweep', ...)`
block at the end of the file (after the context rail block from Task 1).

```ts
const SWEEP_VIEWPORTS = [
  { width: 1280, height: 800 }, // narrow
  { width: 1920, height: 1080 }, // desk
  { width: 2560, height: 1440 }, // uw (context rail hidden: 2560 < 3400)
  { width: 3440, height: 1440 }, // uw (context rail visible on library: 3440 >= 3400)
] as const;

test.describe('responsive breakpoint sweep', () => {
  for (const { width, height } of SWEEP_VIEWPORTS) {
    test.describe(`${width}x${height}`, () => {
      for (const { route, navLabel } of ROUTES) {
        test(`route: ${route}`, async ({ page }) => {
          const capture = attachConsoleCapture(page);
          await page.setViewportSize({ width, height });
          await page.goto('/?fixture=populated');

          const devChip = page.getByRole('status', { name: /Browser dev mode active/i });
          await expect(devChip).toBeVisible();

          const trigger = page.getByRole('tab', { name: navLabel, exact: true });
          await expect(trigger).toBeVisible();
          await trigger.click();
          await expect(trigger).toHaveAttribute('aria-current', 'page');

          await page.waitForLoadState('networkidle', { timeout: 5_000 }).catch(() => {
            /* expected: no network in mock mode */
          });

          const dashboardHeading = DASHBOARD_ROUTE_HEADINGS[route];
          if (dashboardHeading) {
            await expect(
              page.locator('.crosshook-dashboard-panel-section__title', { hasText: dashboardHeading }).first()
            ).toBeVisible();
          }

          await page.screenshot({
            path: `test-results/smoke-${route}-${width}x${height}.png`,
            fullPage: true,
          });

          expect(
            capture.errors,
            `[${width}x${height}] Uncaught errors on route "${route}":\n${capture.errors.join('\n')}`
          ).toEqual([]);
        });
      }
    });
  }
});
```

This produces **4 × 11 = 44 new test instances**. The nested `describe` structure groups
screenshots and errors by viewport in the Playwright HTML report, making failures trivial
to triage by breakpoint.

**Why the inner body-class assertion is omitted**: the existing `browser dev mode smoke`
loop at L94-106 also asserts `.crosshook-dashboard-route-body, ...` container class
visibility inside the active tabpanel. At narrower breakpoints some routes hide body
containers via CSS, which would make the assertion flaky. The
`.crosshook-dashboard-panel-section__title` visibility check is sufficient to confirm
the route rendered its primary content. The body-class check stays only in the
single-viewport `browser dev mode smoke` loop where it was originally authored and
validated.

---

### Task 4 — Verify: run smoke tests and confirm green

**Worktree**: `~/.claude-worktrees/crosshook-phase-12-smoke`  
**Depends on**: Task 3

```bash
cd src/crosshook-native
npm run test:smoke:install   # only needed if Chromium not already cached
npm run test:smoke
```

Expected: all tests pass. Total test count after changes: 21 existing + 2 context-rail + 1 deck-status-bar + 44 sweep = **68 tests**.

**Triage guide for failures**:

| Symptom                                                  | Likely cause                                                  | Fix                                                                                        |
| -------------------------------------------------------- | ------------------------------------------------------------- | ------------------------------------------------------------------------------------------ |
| Context rail test: `toBeVisible()` fails at 3440         | `libraryMode` starts in `detail`; or mock fixture not loading | Verify app loads to `library` `grid` mode by default; check mock handlers                  |
| Context rail test: `toHaveCount(0)` fails at 2560        | Component mounted despite width < 3400                        | Check `contextRailLayoutForShell` call path in `AppShell.tsx:95-105`                       |
| Deck status-bar: `console-status-bar` not found at 1024  | Breakpoint threshold mismatch                                 | Verify `BREAKPOINTS.narrow = 1100` and 1024 < 1100 is `deck`                               |
| Sweep test: `console.error` on specific route × viewport | Mock handler not covering that route at that viewport         | Check `src/lib/mocks/` handlers; viewport is irrelevant to IPC mocks                       |
| Sweep test: heading not visible at narrow                | Route hides heading below `narrow`                            | Add `if (width >= 1440)` guard around the `dashboardHeading` assertion for that route only |

---

## File Size Check

| Segment                                    | Lines          |
| ------------------------------------------ | -------------- |
| Current `smoke.spec.ts`                    | 348            |
| Task 1: `context rail smoke` (2 tests)     | +28            |
| Task 2: deck status-bar test               | +12            |
| Task 3: `SWEEP_VIEWPORTS` + sweep describe | +48            |
| **Projected total**                        | **~436 lines** |

Within the 500-line soft cap.

---

## Batches Summary

All tasks modify the same file; no intra-phase parallelism.

| Batch | Tasks         | Notes                                                                              |
| ----- | ------------- | ---------------------------------------------------------------------------------- |
| 1     | Tasks 1, 2, 3 | Sequential (same file); each task appends/inserts a distinct non-overlapping block |
| 2     | Task 4        | Blocked by Batch 1 — verification only                                             |

---

## Acceptance Criteria

- [ ] `npm run test:smoke` passes with 0 failures
- [ ] `data-testid="context-rail"` visible at 3440×1440 on `library` route
- [ ] `data-testid="context-rail"` absent at 2560×1440 on `library` route
- [ ] `data-testid="console-status-bar"` visible + `console-drawer` absent at both 1280×800 (narrow) and 1024×800 (deck)
- [ ] 44 breakpoint-sweep tests (4 viewports × 11 routes) all pass
- [ ] Screenshots saved as `test-results/smoke-${route}-${width}x${height}.png` for all 44 sweep combinations
- [ ] No `console.error` or `pageerror` in any sweep test
- [ ] `smoke.spec.ts` stays under 500 lines
