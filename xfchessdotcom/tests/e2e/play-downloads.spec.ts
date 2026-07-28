import { test, expect } from '@playwright/test';

// Golden-path coverage for src/pages/Play.tsx's platform download buttons.
// Each button fetches the GitHub "latest release" API, picks the asset
// matching its platform's filename pattern, and redirects the page straight
// to that asset's download URL — falling back to the releases page if the
// API call fails or no matching asset is found. The GitHub API and the
// asset download itself are both mocked so this stays deterministic and
// network-independent.

const RELEASES_URL = 'https://github.com/trilltino/XFChess/releases';

const FAKE_RELEASE = {
  assets: [
    { name: 'XFChess-Setup-1.2.3.exe', browser_download_url: 'https://example.com/download/win.exe' },
    { name: 'XFChess-1.2.3.dmg', browser_download_url: 'https://example.com/download/mac.dmg' },
    { name: 'XFChess-linux-x86_64-1.2.3.tar.gz', browser_download_url: 'https://example.com/download/linux.tar.gz' },
  ],
};

async function mockLatestRelease(page: import('@playwright/test').Page, status = 200) {
  await page.route('https://api.github.com/repos/trilltino/XFChess/releases/latest', (route) =>
    route.fulfill({
      status,
      contentType: 'application/json',
      body: JSON.stringify(status === 200 ? FAKE_RELEASE : {}),
    })
  );
  // The redirect target itself must resolve to something so the navigation
  // completes instead of hanging on a real network lookup.
  await page.route('https://example.com/download/**', (route) =>
    route.fulfill({ status: 200, contentType: 'text/plain', body: 'ok' })
  );
}

test.describe('Play page platform downloads', () => {
  test('Windows button redirects to the matching release asset', async ({ page }) => {
    await mockLatestRelease(page);
    await page.goto('/play');
    await page.locator('button', { hasText: 'Windows' }).click();
    await expect(page).toHaveURL('https://example.com/download/win.exe');
  });

  test('macOS button redirects to the matching release asset', async ({ page }) => {
    await mockLatestRelease(page);
    await page.goto('/play');
    await page.locator('button', { hasText: 'macOS' }).click();
    await expect(page).toHaveURL('https://example.com/download/mac.dmg');
  });

  test('Linux button redirects to the matching release asset', async ({ page }) => {
    await mockLatestRelease(page);
    await page.goto('/play');
    await page.locator('button', { hasText: 'Linux' }).click();
    await expect(page).toHaveURL('https://example.com/download/linux.tar.gz');
  });

  test('falls back to the releases page when the GitHub API call fails', async ({ page }) => {
    await mockLatestRelease(page, 500);
    await page.goto('/play');
    await page.locator('button', { hasText: 'Windows' }).click();
    await expect(page).toHaveURL(RELEASES_URL);
  });

  test('Instructions link points at the install docs', async ({ page }) => {
    await page.goto('/play');
    const link = page.getByRole('link', { name: 'Instructions' });
    await expect(link).toHaveAttribute(
      'href',
      'https://github.com/trilltino/XFChess/blob/main/docs/INSTALL.md'
    );
    await expect(link).toHaveAttribute('target', '_blank');
  });
});
