import { test, expect } from '@playwright/test';

// Golden-path coverage for the homepage: the hero and feature sections
// actually render real content (not the CSR fallback shell), and the
// top-nav is present and interactive. seo.spec.ts covers meta/OG/JSON-LD
// on the same route — this covers visible page content instead.

test.describe('Homepage', () => {
  test('renders the hero and all three feature sections', async ({ page }) => {
    await page.goto('/home');

    await expect(
      page.getByAltText('XFChess — Competitive Chess Server')
    ).toBeVisible();

    await expect(page.getByRole('heading', { name: 'Stake Your Rating' })).toBeVisible();
    await expect(page.getByRole('heading', { name: '2D or 3D' })).toBeVisible();
    await expect(page.getByRole('heading', { name: 'Open Source' })).toBeVisible();
  });

  test('top nav is visible with Home and Play links', async ({ page }) => {
    await page.goto('/home');

    const nav = page.locator('nav.navbar');
    await expect(nav).toBeVisible();
    await expect(nav.getByRole('link', { name: 'Home' })).toBeVisible();
    await expect(nav.getByRole('link', { name: 'Play' })).toBeVisible();
  });
});
