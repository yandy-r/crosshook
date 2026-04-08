import { test, expect, type Page } from '@playwright/test';

/**
 * Smoke test the 9 application routes in browser dev mode.
 *
 * Routing model: CrossHook does NOT use URL-based routing. The sidebar is a
 * Radix `Tabs.Root` whose `value` is held in React state inside `AppShell`.
 * Navigation happens by clicking `Tabs.Trigger` elements (rendered as
 * `role="tab"` with `aria-current="page"` once active). Each test:
 *
 *   1. Loads the app at `/` (with `?fixture=populated` so mock handlers
 *      seed the in-memory store with synthetic profiles).
 *   2. Confirms the dev-mode chip is rendered (proves `__WEB_DEV_MODE__`
 *      is true and the mock IPC chunk loaded successfully).
 *   3. Clicks the sidebar trigger for the route under test.
 *   4. Asserts `aria-current="page"` flips to that trigger.
 *   5. Captures a full-page screenshot into `test-results/`.
 *   6. Asserts no uncaught page errors or `console.error` calls.
 */

interface RouteDef {
  /** AppRoute value (Radix tab value, not URL path). */
  route: string;
  /** Visible nav label rendered inside the sidebar trigger. */
  navLabel: string;
}

// Source of truth: src/components/layout/Sidebar.tsx (AppRoute) plus
// src/components/layout/routeMetadata.ts (ROUTE_NAV_LABEL).
const ROUTES: readonly RouteDef[] = [
  { route: 'library', navLabel: 'Library' },
  { route: 'profiles', navLabel: 'Profiles' },
  { route: 'launch', navLabel: 'Launch' },
  { route: 'install', navLabel: 'Install & Run' },
  { route: 'community', navLabel: 'Browse' },
  { route: 'discover', navLabel: 'Discover' },
  { route: 'compatibility', navLabel: 'Compatibility' },
  { route: 'settings', navLabel: 'Settings' },
  { route: 'health', navLabel: 'Health' },
];

interface ConsoleCapture {
  errors: string[];
}

function attachConsoleCapture(page: Page): ConsoleCapture {
  const capture: ConsoleCapture = { errors: [] };
  page.on('pageerror', (err) => {
    capture.errors.push(`pageerror: ${err.message}`);
  });
  page.on('console', (msg) => {
    if (msg.type() === 'error') {
      capture.errors.push(`console.error: ${msg.text()}`);
    }
  });
  return capture;
}

test.describe('browser dev mode smoke', () => {
  for (const { route, navLabel } of ROUTES) {
    test(`route: ${route} (${navLabel})`, async ({ page }) => {
      const capture = attachConsoleCapture(page);

      await page.goto('/?fixture=populated');

      // Boot sanity: dev chip proves the webdev bundle + mock IPC are live.
      const devChip = page.getByRole('status', { name: /Browser dev mode active/i });
      await expect(devChip).toBeVisible();

      // Sidebar tab trigger for this route. Radix renders `Tabs.Trigger` as
      // `role="tab"`; the visible label inside makes it addressable by name.
      const trigger = page.getByRole('tab', { name: navLabel, exact: true });
      await expect(trigger).toBeVisible();
      await trigger.click();

      // Once selected, the trigger flips to aria-current="page" (see
      // SidebarTrigger in src/components/layout/Sidebar.tsx).
      await expect(trigger).toHaveAttribute('aria-current', 'page');

      // Allow any deferred renders / mock-handler resolves to settle.
      // networkidle may not fire in a pure-mock environment, so we cap it
      // and proceed regardless.
      await page.waitForLoadState('networkidle', { timeout: 5_000 }).catch(() => {
        /* expected: no network in mock mode */
      });

      await page.screenshot({
        path: `test-results/smoke-${route}.png`,
        fullPage: true,
      });

      expect(
        capture.errors,
        `Uncaught errors on route "${route}":\n${capture.errors.join('\n')}`
      ).toEqual([]);
    });
  }
});
