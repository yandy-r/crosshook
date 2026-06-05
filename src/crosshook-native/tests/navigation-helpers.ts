import { expect, type Page } from '@playwright/test';

/**
 * Open the command palette, search for a command, and run it. Waits for the palette to close.
 */
export async function navigateViaCommandPalette(page: Page, commandTitle: string): Promise<void> {
  await page.keyboard.press('Control+KeyK');
  const search = page.getByRole('searchbox', { name: 'Search commands' });
  await expect(search).toBeVisible();
  await search.fill(commandTitle);
  await expect(page.getByRole('button', { name: commandTitle, exact: true })).toBeVisible();
  await search.press('Enter');
  await expect(page.locator('[role="dialog"]')).toHaveCount(0);
}

export async function openLibraryHeroDetail(page: Page, profileName = 'Test Game Alpha'): Promise<void> {
  const libraryTab = page.getByRole('tab', { name: 'Library', exact: true });
  await expect(libraryTab).toBeVisible();
  await libraryTab.click();
  await expect(libraryTab).toHaveAttribute('aria-current', 'page');

  await page.getByRole('button', { name: `View details for ${profileName}` }).click();
  await expect(page.getByTestId('game-detail')).toBeVisible();
}

export async function openHeroDetailTab(
  page: Page,
  tabName: 'Launch options' | 'Profiles',
  profileName = 'Test Game Alpha'
): Promise<void> {
  if (!(await page.getByTestId('game-detail').isVisible().catch(() => false))) {
    await openLibraryHeroDetail(page, profileName);
  }
  const tab = page.getByTestId('game-detail').getByRole('tab', { name: tabName, exact: true });
  await expect(tab).toBeVisible();
  await tab.click();
  await expect(tab).toHaveAttribute('aria-selected', 'true');
}

/** Resolves after `main.tsx` assigns `window.__CROSSHOOK_DEV__` (dev + Playwright only). */
export async function waitForCrosshookDevIpc(page: Page): Promise<void> {
  await page.waitForFunction(() => Boolean(window.__CROSSHOOK_DEV__?.callCommand));
}

export async function seedMockProfileRunning(page: Page, profileName: string, running: boolean): Promise<void> {
  await page.evaluate(
    async ({ name, isRunning }) => {
      await window.__CROSSHOOK_DEV__?.callCommand<null>('_mock_set_profile_running', {
        profileName: name,
        running: isRunning,
      });
    },
    { name: profileName, isRunning: running }
  );
}
