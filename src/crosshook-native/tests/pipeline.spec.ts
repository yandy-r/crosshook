import { test, expect, type Page } from '@playwright/test';

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

test.describe('launch pipeline visualization', () => {
  test.beforeEach(async ({ page }) => {
    await page.goto('/?fixture=populated');
    // Navigate to Launch page
    const launchTab = page.getByRole('tab', { name: 'Launch', exact: true });
    await expect(launchTab).toBeVisible();
    await launchTab.click();
    await expect(launchTab).toHaveAttribute('aria-current', 'page');
  });

  test('renders correct number of pipeline nodes for proton_run method', async ({ page }) => {
    const capture = attachConsoleCapture(page);

    const pipeline = page.locator('.crosshook-launch-pipeline');
    await expect(pipeline).toBeVisible();

    // proton_run method shows 6 nodes: game, wine-prefix, proton, trainer, optimizations, launch
    const nodes = page.locator('.crosshook-launch-pipeline__node');
    await expect(nodes).toHaveCount(6);

    expect(capture.errors).toEqual([]);
  });

  test('all nodes have data-status attributes', async ({ page }) => {
    const capture = attachConsoleCapture(page);

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
    const capture = attachConsoleCapture(page);

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
    const capture = attachConsoleCapture(page);

    const liveRegion = page.locator('.crosshook-launch-pipeline .crosshook-visually-hidden[aria-live="polite"]');
    await expect(liveRegion).toBeAttached();

    const text = await liveRegion.textContent();
    expect(text).toBeTruthy();

    expect(capture.errors).toEqual([]);
  });

  test('tooltip appears on hover for detail-bearing nodes', async ({ page }) => {
    const capture = attachConsoleCapture(page);

    // Wait for pipeline to be visible
    await expect(page.locator('.crosshook-launch-pipeline')).toBeVisible();

    // Find a node with a tooltip trigger (nodes with detail text have triggers)
    const triggers = page.locator('.crosshook-launch-pipeline__node-trigger');
    const triggerCount = await triggers.count();

    if (triggerCount > 0) {
      // Hover the first trigger to open tooltip
      await triggers.first().hover();
      // Radix tooltip appears in a portal with role="tooltip"
      const tooltip = page.getByRole('tooltip');
      await expect(tooltip).toBeVisible({ timeout: 3000 });
    }

    expect(capture.errors).toEqual([]);
  });

  test('no console errors on launch page', async ({ page }) => {
    const capture = attachConsoleCapture(page);

    // Wait for pipeline to settle
    await expect(page.locator('.crosshook-launch-pipeline')).toBeVisible();
    await page.waitForTimeout(1000);

    expect(capture.errors).toEqual([]);
  });
});
