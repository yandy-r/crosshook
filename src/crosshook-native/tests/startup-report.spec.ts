import { expect, test } from '@playwright/test';

import { navigateViaCommandPalette } from './navigation-helpers';

/**
 * Startup-report toggle behavior (browser dev mode).
 *
 * These specs verify that the `?onboarding=show` and `?autoload=` mock toggles
 * surface through the shell: the shell pulls `get_startup_report` on mount and
 * applies `report.onboarding` / `report.auto_load_profile`. They do not prove
 * any timing or race guarantee — the mock computes the report on request, so
 * a delayed mount would echo the same toggles. Late-mount, event-before-report,
 * and terminal-state fallback races are covered by the Vitest hook tests
 * (useProfileHealth / useOfflineReadiness startup tests).
 */
test.describe('startup report (mock toggles)', () => {
  test('onboarding wizard opens from report.onboarding', async ({ page }) => {
    await page.goto('/?fixture=populated&onboarding=show');

    await expect(page.getByRole('status', { name: /Browser dev mode active/i })).toBeVisible({ timeout: 15_000 });
    await expect(page.getByRole('dialog')).toBeVisible();
    await expect(page.getByRole('heading', { name: 'Identity & Game' })).toBeVisible();
    await expect(page.getByRole('dialog')).toHaveCount(1);
  });

  test('auto-load selects report.auto_load_profile', async ({ page }) => {
    await page.goto('/?fixture=populated&autoload=Test%20Game%20Alpha');

    await expect(page.getByRole('status', { name: /Browser dev mode active/i })).toBeVisible({ timeout: 15_000 });
    // Profile-scoped palette commands exist only when a profile is selected.
    await navigateViaCommandPalette(page, 'Launch Test Game Alpha');
  });
});
