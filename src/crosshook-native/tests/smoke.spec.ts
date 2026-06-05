import { test, expect } from '@playwright/test';

import type { AppRoute } from '../src/components/layout/Sidebar';
import { ROUTE_NAV_LABEL } from '../src/components/layout/routeMetadata';
import { attachConsoleCapture, type ConsoleCapture } from './helpers';
import {
  navigateViaCommandPalette,
  openHeroDetailTab,
  openLibraryHeroDetail,
  removeMockProfileVariant,
  seedMockProfileVariant,
  seedMockProfileRunning,
  waitForCrosshookDevIpc,
} from './navigation-helpers';

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
  install: 'Installation options',
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
        const activeTabPanel = page.locator('[role="tabpanel"]:not([hidden])');
        // Scope to the DashboardPanelSection title to avoid collision with
        // RouteBanner headings that may share text on other routes (e.g.
        // several dashboards duplicate their banner h1 as the panel h2).
        await expect(
          page.locator('.crosshook-dashboard-panel-section__title', { hasText: dashboardHeading }).first()
        ).toBeVisible();
        await expect(
          activeTabPanel.locator(
            '.crosshook-dashboard-route-body, .crosshook-host-tool-dashboard, .crosshook-install-page-tabs, .crosshook-settings-panel, .crosshook-community-browser, .crosshook-discovery-panel'
          )
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

test.describe('appRoute regression guard', () => {
  test('sidebar exposes no Profiles or Launch route tabs', async ({ page }) => {
    const capture = attachConsoleCapture(page);

    await page.goto('/?fixture=populated');

    const sidebar = page.getByTestId('sidebar');
    await expect(sidebar).toBeVisible();
    await expect(sidebar.getByRole('tab', { name: 'Profiles', exact: true })).toHaveCount(0);
    await expect(sidebar.getByRole('tab', { name: 'Launch', exact: true })).toHaveCount(0);
    await expect(sidebar.getByRole('tab', { name: 'Library', exact: true })).toBeVisible();

    expect(capture.errors, `appRoute guard errors:\n${capture.errors.join('\n')}`).toEqual([]);
  });
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

  test('hero detail overview deep-link opens launch options tab', async ({ page }) => {
    const capture = attachConsoleCapture(page);
    await page.setViewportSize({ width: 1920, height: 1080 });
    await page.goto('/?fixture=populated');

    const libraryTab = page.getByRole('tab', { name: 'Library', exact: true });
    await libraryTab.click();

    await page.getByRole('button', { name: 'View details for Test Game Alpha' }).click();
    const gameDetail = page.getByTestId('game-detail');
    await expect(gameDetail).toBeVisible();

    await gameDetail.getByRole('button', { name: 'Edit launch config' }).click();

    const launchTab = gameDetail.getByRole('tab', { name: 'Launch options' });
    await expect(launchTab).toHaveAttribute('aria-selected', 'true');
    await expect(gameDetail.getByTestId('hero-detail-launch-tab')).toBeVisible();

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

test.describe('library sidebar quick filters', () => {
  test('Favorites and Currently Playing activate matching library chips', async ({ page }) => {
    const capture = attachConsoleCapture(page);
    await page.setViewportSize({ width: 1920, height: 1080 });
    await page.goto('/?fixture=populated');
    await waitForCrosshookDevIpc(page);
    await seedMockProfileRunning(page, 'Test Game Alpha', true);

    const libraryTab = page.getByRole('tab', { name: 'Library', exact: true });
    await expect(libraryTab).toBeVisible();
    await libraryTab.click();

    const sidebar = page.getByTestId('sidebar');
    const favorites = sidebar.getByRole('button', { name: 'Favorites', exact: true });
    await expect(favorites).toBeVisible();
    await favorites.click();
    await expect(sidebar.getByRole('button', { name: 'Favorites', exact: true })).toHaveAttribute(
      'aria-pressed',
      'true'
    );
    await expect(
      page.locator('.crosshook-library-toolbar').getByRole('button', { name: 'Favorites', exact: true })
    ).toHaveAttribute('aria-pressed', 'true');

    const currentlyPlaying = sidebar.getByRole('button', { name: 'Currently Playing', exact: true });
    await expect(currentlyPlaying).toBeVisible();
    await currentlyPlaying.click();
    await expect(page.getByRole('button', { name: 'Running', exact: true })).toHaveAttribute('aria-pressed', 'true');

    await expect(page.getByText('Test Game Alpha', { exact: true })).toBeVisible();
    await expect(page.getByText('Dev Game Beta', { exact: true })).toHaveCount(0);

    await seedMockProfileRunning(page, 'Test Game Alpha', false);

    expect(capture.errors, `Library quick-filter errors:\n${capture.errors.join('\n')}`).toEqual([]);
  });
});

test.describe('launch pipeline smoke', () => {
  test('pipeline renders on Library launch options tab', async ({ page }) => {
    const capture = attachConsoleCapture(page);

    await page.goto('/?fixture=populated');
    await openHeroDetailTab(page, 'Launch options');

    await expect(page.locator('.crosshook-launch-pipeline')).toBeVisible();
    await expect(page.locator('.crosshook-launch-pipeline__node')).toHaveCount(6);

    expect(capture.errors).toEqual([]);
  });
});

test.describe('Library hero detail panel smoke', () => {
  const HERO_DETAIL_TABS = [
    { tabName: 'Profiles', tabTestId: 'hero-detail-profiles-tab' },
    { tabName: 'Launch options', tabTestId: 'hero-detail-launch-tab' },
  ] as const;

  for (const { tabName, tabTestId } of HERO_DETAIL_TABS) {
    test(`panel sections render on ${tabName} tab`, async ({ page }) => {
      const capture = attachConsoleCapture(page);

      await page.goto('/?fixture=populated');
      await openHeroDetailTab(page, tabName);

      await page.waitForLoadState('networkidle', { timeout: 5_000 }).catch(() => {
        /* expected: no network in mock mode */
      });

      await expect(page.getByTestId('game-detail')).toBeVisible();
      await expect(page.getByTestId('game-detail').getByRole('tab', { name: tabName, exact: true })).toHaveAttribute(
        'aria-selected',
        'true'
      );

      // At least one DashboardPanelSection must be present (attached to DOM) in the route body.
      // NOTE: subtab contents use `display: none` on inactive panels so visibility is gated by
      // the active tab — assert attachment to avoid flaky hidden-tab failures.
      await expect(page.locator('section.crosshook-dashboard-panel-section').first()).toBeAttached();
      await expect(page.getByTestId(tabTestId)).toBeVisible();

      expect(capture.errors, `Hero detail panel errors on tab "${tabName}":\n${capture.errors.join('\n')}`).toEqual([]);
    });
  }

  test('launch pipeline node count is stable on hero detail launch options', async ({ page }) => {
    const capture = attachConsoleCapture(page);

    await page.goto('/?fixture=populated');
    await openHeroDetailTab(page, 'Launch options');

    await expect(page.locator('.crosshook-launch-pipeline__node')).toHaveCount(6);
    await expect(page.locator('section.crosshook-dashboard-panel-section').first()).toBeAttached();

    expect(capture.errors, `Launch options pipeline errors:\n${capture.errors.join('\n')}`).toEqual([]);
  });

  test('Edit profile opens the profiles tab inside Library hero detail', async ({ page }) => {
    const capture = attachConsoleCapture(page);

    await page.setViewportSize({ width: 1920, height: 1080 });
    await page.goto('/?fixture=populated');

    await openLibraryHeroDetail(page);

    // Click "Edit profile" — scope to game-detail to avoid the inspector rail's duplicate button.
    await page.getByTestId('game-detail').getByRole('button', { name: 'Edit profile' }).click();
    await expect(page.getByTestId('game-detail').getByRole('tab', { name: 'Profiles', exact: true })).toHaveAttribute(
      'aria-selected',
      'true'
    );

    const breadcrumb = page.getByRole('navigation', { name: 'Breadcrumb' });
    await expect(breadcrumb).toBeVisible();
    await expect(breadcrumb).toContainText('Library');
    await expect(breadcrumb).toContainText('Test Game Alpha');

    expect(capture.errors, `Hero detail profile-tab smoke errors:\n${capture.errors.join('\n')}`).toEqual([]);
  });

  test('profiles card switch stays in Library and hero Launch registers output', async ({ page }) => {
    const capture = attachConsoleCapture(page);
    const variantName = 'Test Game Alpha - Modded';

    await page.setViewportSize({ width: 1920, height: 1080 });
    await page.goto('/?fixture=populated');
    await waitForCrosshookDevIpc(page);

    try {
      await openLibraryHeroDetail(page, 'Test Game Alpha');
      await seedMockProfileVariant(page, variantName, 'Test Game Alpha');

      const gameDetail = page.getByTestId('game-detail');
      const libraryTab = page.getByRole('tab', { name: 'Library', exact: true });
      await expect(libraryTab).toHaveAttribute('aria-current', 'page');

      await openHeroDetailTab(page, 'Profiles');
      const profileCards = gameDetail.getByRole('list', { name: 'Profile cards' }).locator('> li');
      await expect(profileCards).toHaveCount(2);

      const secondCard = gameDetail.getByRole('button', {
        name: `${variantName} - Test Game Alpha`,
        exact: true,
      });
      await secondCard.click();
      await expect(secondCard).toHaveAttribute('aria-current', 'true');
      await expect(secondCard.getByText('Active', { exact: true })).toBeVisible();
      await expect(gameDetail.getByRole('heading', { level: 3, name: variantName })).toBeVisible();

      const heroLaunchButton = gameDetail
        .locator('.crosshook-hero-detail__quick-actions')
        .getByRole('button', { name: 'Launch', exact: true });
      await expect(heroLaunchButton).toBeEnabled();
      await heroLaunchButton.click();
      await expect(gameDetail.getByRole('tab', { name: 'Launch options', exact: true })).toHaveAttribute(
        'aria-selected',
        'true'
      );
      await expect(gameDetail.getByRole('heading', { name: 'Launch command' })).toBeVisible();

      const launchGameButton = gameDetail.getByRole('button', { name: 'Launch Game', exact: true });
      await expect(launchGameButton).toBeEnabled();
      await launchGameButton.click();

      await expect(page.getByText(/^[1-9][0-9]* lines?$/)).toBeVisible();
      await expect(libraryTab).toHaveAttribute('aria-current', 'page');
    } finally {
      await removeMockProfileVariant(page, variantName);
    }

    expect(capture.errors, `Hero detail card-switch launch errors:\n${capture.errors.join('\n')}`).toEqual([]);
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

    await openHeroDetailTab(page, 'Launch options');

    await openHeroDetailTab(page, 'Profiles');

    const gamePathField = page.getByLabel('Game Path', { exact: true });
    await expect(gamePathField).toBeVisible();
    await gamePathField.fill('/home/devuser/Games/TestGameAlpha/game.exe');

    await openHeroDetailTab(page, 'Launch options');

    const launchGameButton = page.getByRole('button', { name: /^launch game$/i });
    await expect(launchGameButton).toBeEnabled();
    await launchGameButton.click();

    await expect(page.getByText(/^[1-9][0-9]* lines?$/)).toBeVisible();
    await expect(toggle).toHaveAttribute('aria-expanded', 'false');

    expect(capture.errors, `Desktop console chrome errors:\n${capture.errors.join('\n')}`).toEqual([]);
  });

  test('renders the compact status bar at deck width (1024×800)', async ({ page }) => {
    const capture = attachConsoleCapture(page);
    await page.setViewportSize({ width: 1024, height: 800 });
    await page.goto('/?fixture=populated');
    await expect(page.getByTestId('console-status-bar')).toBeVisible();
    await expect(page.getByTestId('console-drawer')).toHaveCount(0);
    await expect(page.getByText('⌘K commands')).toBeVisible();
    expect(capture.errors, `Deck console chrome errors:\n${capture.errors.join('\n')}`).toEqual([]);
  });
});

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

/**
 * Viewports to sweep for the hero detail overflow assertions.
 * Extends SWEEP_VIEWPORTS with the 1024×800 Steam Deck / narrow-portrait case.
 */
const HERO_DETAIL_OVERFLOW_VIEWPORTS = [
  ...SWEEP_VIEWPORTS,
  { width: 1024, height: 800 }, // deck / narrow-portrait
] as const;

test.describe('hero detail responsive no-horizontal-overflow', () => {
  for (const { width, height } of HERO_DETAIL_OVERFLOW_VIEWPORTS) {
    test.describe(`${width}x${height}`, () => {
      /**
       * Helper: navigate to library, open hero detail for Test Game Alpha,
       * then click the given tab trigger and wait for it to be visible.
       */
      async function openHeroDetailTab(
        page: import('@playwright/test').Page,
        tabLabel: string
      ): Promise<void> {
        const devChip = page.getByRole('status', { name: /Browser dev mode active/i });
        await expect(devChip).toBeVisible();

        const libraryTab = page.getByRole('tab', { name: 'Library', exact: true });
        await libraryTab.click();
        await expect(libraryTab).toHaveAttribute('aria-current', 'page');

        await page.getByRole('button', { name: 'View details for Test Game Alpha' }).click();
        await expect(page.getByTestId('game-detail')).toBeVisible();

        await page.getByRole('tab', { name: tabLabel, exact: true }).click();

        await page.waitForLoadState('networkidle', { timeout: 5_000 }).catch(() => {
          /* expected: no network in mock mode */
        });
      }

      test('launch-options tab: no horizontal overflow', async ({ page }) => {
        const capture = attachConsoleCapture(page);
        await page.setViewportSize({ width, height });
        await page.goto('/?fixture=populated');

        await openHeroDetailTab(page, 'Launch options');

        // The launch tab container must exist
        await expect(
          page.locator('.crosshook-hero-detail__launch-tab').first()
        ).toBeAttached();

        // Assert no horizontal overflow on document root
        const rootOverflowing = await page.evaluate(() => {
          return document.documentElement.scrollWidth > document.documentElement.clientWidth;
        });
        expect(
          rootOverflowing,
          `[${width}x${height}] launch-options: document root must not overflow horizontally`
        ).toBe(false);

        // Assert no horizontal overflow on the launch tab container
        const launchTabOverflowing = await page.evaluate(() => {
          const el = document.querySelector('.crosshook-hero-detail__launch-tab');
          if (!el) {
            throw new Error('crosshook-hero-detail__launch-tab not found');
          }
          return el.scrollWidth > el.clientWidth;
        });
        expect(
          launchTabOverflowing,
          `[${width}x${height}] launch-options: .crosshook-hero-detail__launch-tab must not overflow horizontally`
        ).toBe(false);

        await page.screenshot({
          path: `test-results/hero-detail-launch-options-overflow-${width}x${height}.png`,
          fullPage: true,
        });

        expect(
          capture.errors,
          `[${width}x${height}] hero-detail launch-options console errors:\n${capture.errors.join('\n')}`
        ).toEqual([]);
      });

      test('profiles tab: no horizontal overflow', async ({ page }) => {
        const capture = attachConsoleCapture(page);
        await page.setViewportSize({ width, height });
        await page.goto('/?fixture=populated');

        await openHeroDetailTab(page, 'Profiles');

        // The profiles editor must exist (it is always rendered once the tab is active)
        await expect(
          page.locator('.crosshook-hero-detail__profiles-editor').first()
        ).toBeAttached();

        // Assert no horizontal overflow on document root
        const rootOverflowing = await page.evaluate(() => {
          return document.documentElement.scrollWidth > document.documentElement.clientWidth;
        });
        expect(
          rootOverflowing,
          `[${width}x${height}] profiles: document root must not overflow horizontally`
        ).toBe(false);

        // Assert no horizontal overflow on the profiles editor container
        const editorOverflowing = await page.evaluate(() => {
          const el = document.querySelector('.crosshook-hero-detail__profiles-editor');
          if (!el) {
            throw new Error('crosshook-hero-detail__profiles-editor not found');
          }
          return el.scrollWidth > el.clientWidth;
        });
        expect(
          editorOverflowing,
          `[${width}x${height}] profiles: .crosshook-hero-detail__profiles-editor must not overflow horizontally`
        ).toBe(false);

        await page.screenshot({
          path: `test-results/hero-detail-profiles-overflow-${width}x${height}.png`,
          fullPage: true,
        });

        expect(
          capture.errors,
          `[${width}x${height}] hero-detail profiles console errors:\n${capture.errors.join('\n')}`
        ).toEqual([]);
      });
    });
  }
});

test.describe('reduced-motion smoke', () => {
  test.beforeEach(async ({ page }) => {
    await page.emulateMedia({ reducedMotion: 'reduce' });
  });

  test('library renders without console errors under prefers-reduced-motion', async ({ page }) => {
    const capture = attachConsoleCapture(page);
    await page.setViewportSize({ width: 1920, height: 1080 });
    await page.goto('/?fixture=populated');
    const libraryTab = page.getByRole('tab', { name: 'Library', exact: true });
    await libraryTab.click();
    await expect(libraryTab).toHaveAttribute('aria-current', 'page');
    await expect(
      page.locator('.crosshook-library-card__hover-reveal').first(),
      'library hover-reveal element must be present to validate reduced-motion transition'
    ).toBeAttached();
    const hoverRevealSeconds = await page.evaluate(() => {
      const el = document.querySelector('.crosshook-library-card__hover-reveal');
      if (!el) {
        throw new Error('hover-reveal element missing after attach check');
      }
      return Number.parseFloat(window.getComputedStyle(el).transitionDuration);
    });
    // Project's global reduced-motion rule (theme.css) collapses to 0.01ms so
    // `transitionend` still fires — anything under 1ms is "effectively zero".
    expect(hoverRevealSeconds, 'hover-reveal transition must be near-zero under reduced-motion').toBeLessThan(0.001);
    await expect(
      page.locator('.crosshook-library-card').first(),
      'library card root must be present to validate reduced-motion transition'
    ).toBeAttached();
    const cardSeconds = await page.evaluate(() => {
      const el = document.querySelector('.crosshook-library-card');
      if (!el) {
        throw new Error('library card element missing after attach check');
      }
      return Number.parseFloat(window.getComputedStyle(el).transitionDuration);
    });
    expect(cardSeconds, 'card root transition must be near-zero under reduced-motion').toBeLessThan(0.001);
    expect(capture.errors, `Reduced-motion library errors:\n${capture.errors.join('\n')}`).toEqual([]);
  });

  test('command palette opens without animation under prefers-reduced-motion', async ({ page }) => {
    const capture = attachConsoleCapture(page);
    await page.setViewportSize({ width: 1920, height: 1080 });
    await page.goto('/?fixture=populated');
    await page.keyboard.press('Control+k');
    const dialog = page.locator('[role="dialog"]');
    await expect(dialog).toBeVisible({ timeout: 3000 });
    await expect(
      page.locator('.crosshook-palette__row').first(),
      'palette row must be present to validate reduced-motion transition'
    ).toBeAttached();
    const paletteRowSeconds = await page.evaluate(() => {
      const el = document.querySelector('.crosshook-palette__row');
      if (!el) {
        throw new Error('palette row element missing after attach check');
      }
      return Number.parseFloat(window.getComputedStyle(el).transitionDuration);
    });
    // See the library-card test for the 0.01ms global-rule rationale.
    expect(paletteRowSeconds, 'palette row transition must be near-zero under reduced-motion').toBeLessThan(0.001);
    await page.getByRole('button', { name: 'Close command palette' }).click();
    await expect(dialog).toHaveCount(0);
    expect(capture.errors, `Reduced-motion palette errors:\n${capture.errors.join('\n')}`).toEqual([]);
  });
});
