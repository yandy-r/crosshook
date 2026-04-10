/**
 * Playwright smoke tests for the Profile Collections feature (Phase 5).
 *
 * Mirrors the `smoke.spec.ts` skeleton: `attachConsoleCapture`,
 * `?fixture=populated`, zero-console-error assertion.
 *
 * **MockStore singleton gotcha**: `playwright.config.ts` pins
 * `fullyParallel: false` and `workers: 1`. The module-scoped `MockStore`
 * in `lib/mocks/handlers/collections.ts` resets on full page reload.
 * Each test uses `test.beforeEach` to `page.goto('/?fixture=populated')`
 * for a clean store per test case.
 */

import { test, expect } from '@playwright/test';
import { attachConsoleCapture, type ConsoleCapture } from './helpers';

test.describe('collections smoke', () => {
  let capture: ConsoleCapture;

  test.beforeEach(async ({ page }) => {
    capture = attachConsoleCapture(page);
    await page.goto('/?fixture=populated');

    // Boot sanity: dev chip proves mock IPC loaded.
    const devChip = page.getByRole('status', { name: /Browser dev mode active/i });
    await expect(devChip).toBeVisible();
  });

  test('create collection flow via sidebar CTA', async ({ page }) => {
    // Click the "New Collection" CTA in the sidebar.
    const newCollectionCta = page.getByRole('button', { name: 'New Collection' });
    await expect(newCollectionCta).toBeVisible();
    await newCollectionCta.click();

    // CollectionEditModal should open.
    const dialog = page.getByRole('dialog');
    await expect(dialog).toBeVisible();

    // Type a name and click the Create button.
    const nameInput = dialog.locator('input[type="text"]').first();
    await nameInput.fill('Test Collection');
    const createBtn = dialog.getByRole('button', { name: 'Create' });
    await createBtn.click();

    // Modal should close after successful create.
    await expect(dialog).not.toBeVisible({ timeout: 10_000 });

    // Allow any deferred renders to settle.
    await page.waitForLoadState('networkidle', { timeout: 5_000 }).catch(() => {});

    await expect(page.getByRole('button', { name: /Test Collection/ })).toBeVisible({
      timeout: 10_000,
    });

    expect(capture.errors, `Uncaught errors during create collection flow:\n${capture.errors.join('\n')}`).toEqual([]);
  });

  test('open and close CollectionViewModal', async ({ page }) => {
    // Click an existing collection in the sidebar.
    const collectionItem = page.locator('.crosshook-collections-sidebar__item').first();
    // Wait for the sidebar to render at least one collection.
    await expect(collectionItem).toBeVisible({ timeout: 5_000 });
    await collectionItem.click();

    // CollectionViewModal should open.
    const dialog = page.getByRole('dialog');
    await expect(dialog).toBeVisible();

    // Press Escape to close.
    await page.keyboard.press('Escape');
    await expect(dialog).not.toBeVisible({ timeout: 5_000 });

    await page.waitForLoadState('networkidle', { timeout: 5_000 }).catch(() => {});

    expect(capture.errors, `Uncaught errors during view modal flow:\n${capture.errors.join('\n')}`).toEqual([]);
  });

  test('assign menu opens on library card context menu and keyboard navigates', async ({ page }) => {
    // Navigate to Library tab.
    const libraryTrigger = page.getByRole('tab', { name: 'Library', exact: true });
    await expect(libraryTrigger).toBeVisible();
    await libraryTrigger.click();
    await expect(libraryTrigger).toHaveAttribute('aria-current', 'page');

    // Wait for library cards to render.
    const firstCard = page.locator('.crosshook-library-card').first();
    await expect(firstCard).toBeVisible({ timeout: 10_000 });

    await firstCard.focus();
    // Right-click to open assign menu (desktop analogue of context-menu invoke).
    await firstCard.click({ button: 'right' });

    // Assign menu (dialog) should open.
    const assignMenu = page.locator('.crosshook-collection-assign-menu');
    await expect(assignMenu).toBeVisible({ timeout: 5_000 });

    const checkboxes = assignMenu.locator('.crosshook-collection-assign-menu__option input[type="checkbox"]');
    const newCollectionBtn = assignMenu.getByRole('button', { name: /New collection/i });
    // Populated mock has exactly one collection — navigation wraps [checkbox] ↔ [+ New collection].
    await expect(checkboxes).toHaveCount(1);
    await expect(checkboxes.first()).toBeFocused();
    await page.keyboard.press('ArrowDown');
    await expect(newCollectionBtn).toBeFocused();
    await page.keyboard.press('ArrowDown');
    await expect(checkboxes.first()).toBeFocused();

    await page.keyboard.press('Escape');
    await expect(assignMenu).not.toBeVisible({ timeout: 5_000 });
    await expect(firstCard).toBeFocused();

    await page.waitForLoadState('networkidle', { timeout: 5_000 }).catch(() => {});

    expect(capture.errors, `Uncaught errors during assign menu flow:\n${capture.errors.join('\n')}`).toEqual([]);
  });

  test('import preset flow opens BrowserDevPresetExplainerModal', async ({ page }) => {
    // Click "Import Preset" CTA.
    const importCta = page.getByRole('button', { name: 'Import Preset' });
    await expect(importCta).toBeVisible();
    await importCta.click();

    // BrowserDevPresetExplainerModal should open (browser dev mode path).
    const dialog = page.getByRole('dialog');
    await expect(dialog).toBeVisible();

    // Click Continue to proceed to import review.
    const continueBtn = dialog.getByRole('button', { name: 'Continue' });
    await expect(continueBtn).toBeVisible();
    await continueBtn.click();

    const reviewDialog = page.getByRole('dialog', { name: /import collection preset/i });
    await expect(reviewDialog).toBeVisible({ timeout: 5_000 });

    await page.keyboard.press('Escape');
    await expect(reviewDialog).not.toBeVisible({ timeout: 5_000 });

    expect(capture.errors, `Uncaught errors during import preset flow:\n${capture.errors.join('\n')}`).toEqual([]);
  });
});
