import { test, expect } from '@playwright/test';

// Golden-path nav: the links that get a signed-out visitor from the
// homepage into the app, plus the wallet-connect entry point. Wallet
// connection itself isn't exercised — no extension is installed in this
// browser context — only that the picker modal opens and closes.

test.describe('Top nav', () => {
  test('Play link navigates to /play', async ({ page }) => {
    await page.goto('/home');
    await page.locator('nav.navbar').getByRole('link', { name: 'Play' }).click();
    await expect(page).toHaveURL(/\/play$/);
  });

  test('logo navigates back to /home', async ({ page }) => {
    await page.goto('/play');
    await page.locator('a.nav-logo').click();
    await expect(page).toHaveURL(/\/home$/);
  });

  test('Connect Wallet opens the wallet picker, overlay click closes it', async ({ page }) => {
    await page.goto('/home');
    await page.getByRole('button', { name: 'Connect Wallet' }).click();

    const modal = page.locator('.custom-wallet-modal');
    await expect(modal).toBeVisible();
    await expect(modal.getByRole('heading', { name: /Select Network Provider/ })).toBeVisible();

    // Click the overlay outside the modal card to dismiss it.
    await page.locator('.modal-overlay').click({ position: { x: 5, y: 5 } });
    await expect(modal).not.toBeVisible();
  });

  test('mobile menu toggle reveals nav links at narrow viewport', async ({ page }) => {
    await page.setViewportSize({ width: 375, height: 800 });
    await page.goto('/home');

    const navLinks = page.locator('.nav-links');
    const toggle = page.locator('.mobile-menu-toggle');

    await expect(navLinks).not.toHaveClass(/active/);
    await toggle.click();
    await expect(navLinks).toHaveClass(/active/);
    await toggle.click();
    await expect(navLinks).not.toHaveClass(/active/);
  });
});
