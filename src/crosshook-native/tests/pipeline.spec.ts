import { test, expect, type Page } from '@playwright/test';
import { attachConsoleCapture, type ConsoleCapture } from './helpers';

async function navigateViaCommandPalette(page: Page, commandTitle: 'Go to Launch' | 'Go to Profiles'): Promise<void> {
  await page.keyboard.press('Control+KeyK');
  const search = page.getByRole('searchbox', { name: 'Search commands' });
  await expect(search).toBeVisible();
  await search.fill(commandTitle);
  await expect(page.getByRole('button', { name: commandTitle, exact: true })).toBeVisible();
  await search.press('Enter');
  await expect(page.locator('[role="dialog"]')).toHaveCount(0);
}

test.describe('launch pipeline visualization', () => {
  let capture: ConsoleCapture;

  test.beforeEach(async ({ page }) => {
    capture = attachConsoleCapture(page);
    await page.goto('/?fixture=populated');
    await navigateViaCommandPalette(page, 'Go to Launch');
  });

  test('renders correct number of pipeline nodes for proton_run method', async ({ page }) => {
    const pipeline = page.locator('.crosshook-launch-pipeline');
    await expect(pipeline).toBeVisible();

    // proton_run method shows 6 nodes: game, wine-prefix, proton, trainer, optimizations, launch
    const nodes = page.locator('.crosshook-launch-pipeline__node');
    await expect(nodes).toHaveCount(6);

    expect(capture.errors).toEqual([]);
  });

  test('all nodes have data-status attributes', async ({ page }) => {
    const nodes = page.locator('.crosshook-launch-pipeline__node');
    const count = await nodes.count();

    for (let i = 0; i < count; i++) {
      const status = await nodes.nth(i).getAttribute('data-status');
      expect(status).toBeTruthy();
      expect(['configured', 'not-configured', 'error', 'active', 'complete']).toContain(status);
    }

    expect(capture.errors).toEqual([]);
  });

  test('nodes have accessible aria-labels', async ({ page }) => {
    const nodes = page.locator('.crosshook-launch-pipeline__node');
    const count = await nodes.count();

    for (let i = 0; i < count; i++) {
      const label = await nodes.nth(i).getAttribute('aria-label');
      expect(label).toBeTruthy();
      // aria-label format: "NodeLabel: StatusText"
      expect(label).toContain(':');
    }

    expect(capture.errors).toEqual([]);
  });

  test('aria-live region exists for status announcements', async ({ page }) => {
    const liveRegion = page.locator('.crosshook-launch-pipeline .crosshook-visually-hidden[aria-live="polite"]');
    await expect(liveRegion).toBeAttached();

    const text = await liveRegion.textContent();
    expect(text).toBeTruthy();

    expect(capture.errors).toEqual([]);
  });

  test('tooltip appears on hover for detail-bearing nodes', async ({ page }) => {
    // Wait for pipeline to be visible
    await expect(page.locator('.crosshook-launch-pipeline')).toBeVisible();

    // Browser dev mode does not auto-select a profile on startup, so load one
    // through the real launch-page control before requesting a preview.
    const profileSelect = page.locator('#launch-profile-selector');
    await expect(profileSelect).toBeVisible();
    await profileSelect.click();
    await page.getByRole('option', { name: 'Test Game Alpha', exact: true }).click();
    await expect(profileSelect).toContainText('Test Game Alpha');

    // Seeded mock profiles intentionally start without an executable path, so
    // update the draft through the real profile form before previewing.
    await navigateViaCommandPalette(page, 'Go to Profiles');

    const gamePathField = page.getByLabel('Game Path', { exact: true });
    await expect(gamePathField).toBeVisible();
    await gamePathField.fill('/home/devuser/Games/TestGameAlpha/game.exe');

    await navigateViaCommandPalette(page, 'Go to Launch');

    // Launch the game to drive the pipeline into the two-step waiting state.
    // That overlay adds a detail-bearing trainer node without opening the
    // preview modal, so the tooltip trigger remains interactable.
    const launchGameButton = page.getByRole('button', { name: /^launch game$/i });
    await expect(launchGameButton).toBeVisible();
    await expect(launchGameButton).toBeEnabled();
    await launchGameButton.click();

    // Wait for the waiting-state detail trigger to appear.
    const triggers = page.locator('.crosshook-launch-pipeline__node-trigger');
    await expect(triggers).not.toHaveCount(0);

    // Hover the first trigger to open tooltip
    await triggers.first().hover();
    // Radix tooltip appears in a portal with role="tooltip"
    const tooltip = page.getByRole('tooltip');
    await expect(tooltip).toBeVisible({ timeout: 3000 });

    expect(capture.errors).toEqual([]);
  });

  test('no console errors on launch page', async ({ page }) => {
    // Wait for pipeline to settle
    await expect(page.locator('.crosshook-launch-pipeline')).toBeVisible();
    await page.waitForTimeout(1000);

    expect(capture.errors).toEqual([]);
  });

  test('warning-severity validation does not produce error nodes', async ({ page }) => {
    // Navigate with a game path that triggers the __MOCK_VALIDATION_WARNING__ fixture.
    // The mock preview_launch handler returns warning-severity issues, but
    // derivePipelineNodes only promotes `fatal` to `error` — trainer stays `configured`.
    capture = attachConsoleCapture(page);
    await page.goto('/?fixture=populated&gamePath=__MOCK_VALIDATION_WARNING__');
    await navigateViaCommandPalette(page, 'Go to Launch');

    const pipeline = page.locator('.crosshook-launch-pipeline');
    await expect(pipeline).toBeVisible();

    // No nodes should have error status (warning-severity doesn't promote to error)
    const errorNodes = page.locator('.crosshook-launch-pipeline__node[data-status="error"]');
    await expect(errorNodes).toHaveCount(0);

    expect(capture.errors).toEqual([]);
  });
});
