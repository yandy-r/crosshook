import { test, expect } from '@playwright/test';

import type { AppRoute } from '../src/components/layout/Sidebar';
import { ROUTE_NAV_LABEL } from '../src/components/layout/routeMetadata';
import { attachConsoleCapture, type ConsoleCapture } from './helpers';

/**
 * Smoke test the application routes in browser dev mode.
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

/** Order matches sidebar primary nav (see Sidebar.tsx). */
const ROUTE_ORDER: readonly AppRoute[] = [
  'library',
  'profiles',
  'launch',
  'install',
  'health',
  'host-tools',
  'proton-manager',
  'community',
  'discover',
  'compatibility',
  'settings',
];

const DASHBOARD_ROUTE_HEADINGS: Partial<Record<AppRoute, string>> = {
  health: 'Monitor profile readiness across launch, version, and offline checks',
  'host-tools': 'Check runtime coverage before you launch',
  'proton-manager': 'Manage installed Proton builds',
  compatibility: 'Keep trainer reports and Proton runtimes in the same workflow',
  install: 'Install & Run',
  settings: 'App preferences and storage',
  community: 'Community Profiles',
  discover: 'Trainer Discovery',
};

// Labels from ROUTE_NAV_LABEL (routeMetadata.ts) — same source as Sidebar triggers.
const ROUTES: readonly RouteDef[] = ROUTE_ORDER.map((route) => ({
  route,
  navLabel: ROUTE_NAV_LABEL[route],
}));

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

      const dashboardHeading = DASHBOARD_ROUTE_HEADINGS[route];
      if (dashboardHeading) {
        // Scope to the DashboardPanelSection title to avoid collision with
        // RouteBanner headings that share the same text on some routes (e.g.
        // install has "Install & Run" in both the banner h1 and the panel h2).
        await expect(
          page.locator('.crosshook-dashboard-panel-section__title', { hasText: dashboardHeading }).first()
        ).toBeVisible();
        await expect(
          page
            .locator(
              '.crosshook-dashboard-route-body, .crosshook-host-tool-dashboard, .crosshook-install-page-tabs, .crosshook-settings-panel, .crosshook-community-browser, .crosshook-discovery-panel'
            )
            .first()
        ).toBeVisible();
      }

      await page.screenshot({
        path: `test-results/smoke-${route}.png`,
        fullPage: true,
      });

      expect(capture.errors, `Uncaught errors on route "${route}":\n${capture.errors.join('\n')}`).toEqual([]);
    });
  }
});

test.describe('library inspector', () => {
  test('inspector shows after selecting a card at desktop width', async ({ page }) => {
    const capture = attachConsoleCapture(page);
    await page.setViewportSize({ width: 1920, height: 1080 });
    await page.goto('/?fixture=populated');

    const devChip = page.getByRole('status', { name: /Browser dev mode active/i });
    await expect(devChip).toBeVisible();

    const libraryTab = page.getByRole('tab', { name: 'Library', exact: true });
    await libraryTab.click();
    await expect(libraryTab).toHaveAttribute('aria-current', 'page');

    await page.getByRole('button', { name: /^Select /i }).first().click();
    await expect(page.getByTestId('inspector')).toBeVisible();
    await expect(page.getByTestId('inspector')).not.toContainText('Select a game to see details');

    expect(capture.errors).toEqual([]);
  });

  test('inspector rail is absent at deck width', async ({ page }) => {
    const capture = attachConsoleCapture(page);
    await page.setViewportSize({ width: 1024, height: 800 });
    await page.goto('/?fixture=populated');

    const libraryTab = page.getByRole('tab', { name: 'Library', exact: true });
    await libraryTab.click();

    await page.getByRole('button', { name: /^Select /i }).first().click();
    await expect(page.locator('[data-testid="inspector"]')).toHaveCount(0);
    expect(capture.errors).toEqual([]);
  });

  test('hero detail opens and Back returns to library grid at desktop width', async ({ page }) => {
    const capture = attachConsoleCapture(page);
    await page.setViewportSize({ width: 1920, height: 1080 });
    await page.goto('/?fixture=populated');

    const libraryTab = page.getByRole('tab', { name: 'Library', exact: true });
    await libraryTab.click();

    await page.getByRole('button', { name: 'View details for Test Game Alpha' }).click();
    await expect(page.getByTestId('game-detail')).toBeVisible();
    await expect(page.getByRole('button', { name: 'Back' })).toBeVisible();

    await page.getByRole('button', { name: 'Back' }).click();
    await expect(page.getByTestId('game-detail')).toHaveCount(0);
    await expect(page.getByRole('button', { name: 'Open command palette' })).toBeVisible();

    expect(capture.errors).toEqual([]);
  });

  test('hero detail works at deck width without inspector', async ({ page }) => {
    const capture = attachConsoleCapture(page);
    await page.setViewportSize({ width: 1024, height: 800 });
    await page.goto('/?fixture=populated');

    const libraryTab = page.getByRole('tab', { name: 'Library', exact: true });
    await libraryTab.click();

    await page.getByRole('button', { name: 'View details for Test Game Alpha' }).click();
    await expect(page.getByTestId('game-detail')).toBeVisible();
    await expect(page.locator('[data-testid="inspector"]')).toHaveCount(0);

    await page.getByRole('button', { name: 'Back' }).click();
    await expect(page.getByTestId('game-detail')).toHaveCount(0);

    expect(capture.errors).toEqual([]);
  });
});

test.describe('launch pipeline smoke', () => {
  test('pipeline renders on launch page', async ({ page }) => {
    const capture = attachConsoleCapture(page);

    await page.goto('/?fixture=populated');
    const launchTab = page.getByRole('tab', { name: 'Launch', exact: true });
    await launchTab.click();
    await expect(launchTab).toHaveAttribute('aria-current', 'page');

    await expect(page.locator('.crosshook-launch-pipeline')).toBeVisible();
    await expect(page.locator('.crosshook-launch-pipeline__node')).toHaveCount(6);

    expect(capture.errors).toEqual([]);
  });
});

test.describe('command palette smoke', () => {
  test('opens with shortcut, executes a route command, and keeps capture clean', async ({ page }) => {
    const capture = attachConsoleCapture(page);

    await page.setViewportSize({ width: 1920, height: 1080 });
    await page.goto('/?fixture=populated');

    const libraryTab = page.getByRole('tab', { name: 'Library', exact: true });
    await libraryTab.click();
    await expect(libraryTab).toHaveAttribute('aria-current', 'page');

    await page.keyboard.press('Control+KeyK');
    const search = page.getByRole('searchbox', { name: 'Search commands' });
    await expect(search).toBeVisible();

    await search.fill('settings');
    await search.press('Enter');

    await expect(page.getByRole('tab', { name: 'Settings', exact: true })).toHaveAttribute('aria-current', 'page');

    expect(capture.errors, `Command-palette execution errors:\n${capture.errors.join('\n')}`).toEqual([]);
  });

  test('opens and closes from toolbar trigger', async ({ page }) => {
    const capture = attachConsoleCapture(page);

    await page.setViewportSize({ width: 1920, height: 1080 });
    await page.goto('/?fixture=populated');
    await page.getByRole('button', { name: 'Open command palette' }).click();

    const search = page.getByRole('searchbox', { name: 'Search commands' });
    await expect(search).toBeVisible();

    await page.keyboard.press('Escape');
    await expect(search).not.toBeVisible();

    expect(capture.errors, `Command-palette toolbar errors:\n${capture.errors.join('\n')}`).toEqual([]);
  });
});

test.describe('console chrome smoke', () => {
  test('renders the compact status bar at narrow width', async ({ page }) => {
    const capture = attachConsoleCapture(page);

    await page.setViewportSize({ width: 1280, height: 800 });
    await page.goto('/?fixture=populated');

    await expect(page.getByTestId('console-status-bar')).toBeVisible();
    await expect(page.getByTestId('console-drawer')).toHaveCount(0);
    await expect(page.getByText('⌘K commands')).toBeVisible();

    expect(capture.errors, `Narrow console chrome errors:\n${capture.errors.join('\n')}`).toEqual([]);
  });

  test('keeps the drawer collapsed on desktop after log output arrives', async ({ page }) => {
    const capture = attachConsoleCapture(page);

    await page.setViewportSize({ width: 1920, height: 1080 });
    await page.goto('/?fixture=populated');

    const drawer = page.getByTestId('console-drawer');
    const toggle = page.getByRole('button', { name: 'Runtime console' });
    await expect(drawer).toBeVisible();
    await expect(toggle).toHaveAttribute('aria-expanded', 'false');

    const launchTab = page.getByRole('tab', { name: 'Launch', exact: true });
    await launchTab.click();
    await expect(launchTab).toHaveAttribute('aria-current', 'page');

    const profileSelect = page.locator('#launch-profile-selector');
    await expect(profileSelect).toBeVisible();
    await profileSelect.click();
    await page.getByRole('option', { name: 'Test Game Alpha', exact: true }).click();
    await expect(profileSelect).toContainText('Test Game Alpha');

    const profilesTab = page.getByRole('tab', { name: 'Profiles', exact: true });
    await profilesTab.click();
    await expect(profilesTab).toHaveAttribute('aria-current', 'page');

    const gamePathField = page.getByLabel('Game Path', { exact: true });
    await expect(gamePathField).toBeVisible();
    await gamePathField.fill('/home/devuser/Games/TestGameAlpha/game.exe');

    await launchTab.click();
    await expect(launchTab).toHaveAttribute('aria-current', 'page');

    const launchGameButton = page.getByRole('button', { name: /^launch game$/i });
    await expect(launchGameButton).toBeEnabled();
    await launchGameButton.click();

    await expect(page.getByText(/^[1-9][0-9]* lines?$/)).toBeVisible();
    await expect(toggle).toHaveAttribute('aria-expanded', 'false');

    expect(capture.errors, `Desktop console chrome errors:\n${capture.errors.join('\n')}`).toEqual([]);
  });
});
