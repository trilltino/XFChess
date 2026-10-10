import { test, expect } from '@playwright/test';

// No extension is installed in this browser context; exercise only opening and closing the picker.

test.describe('Top nav', () => {
  test('Play link navigates to /play', async ({ page }) => {
    await page.goto('/home');
    await page.locator('nav.navbar').getByRole('link', { name: 'Play' }).click();
    await expect(page).toHaveURL(/\/play$/);
  });

  test('logo navigates back to home', async ({ page }) => {
    await page.goto('/play');
    await page.locator('a.nav-logo').click();
    await expect(page).toHaveURL(/\/(home)?$/);
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
