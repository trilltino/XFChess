import { test, expect } from '@playwright/test';

const cors = { 'access-control-allow-origin': '*', 'access-control-allow-headers': '*' };

test('signing up shows a confirmation and sends only the address', async ({ page }) => {
  let sent: unknown;
  await page.route('**/api/signup', async (route) => {
    if (route.request().method() === 'OPTIONS') {
      return route.fulfill({ status: 204, headers: cors });
    }
    sent = route.request().postDataJSON();
    await route.fulfill({
      status: 200,
      headers: cors,
      contentType: 'application/json',
      body: JSON.stringify({ ok: true, queued: true }),
    });
  });

  await page.goto('/');
  await page.getByLabel('Email').fill(' tino@example.com ');
  await page.getByRole('button', { name: 'Sign up' }).click();

  await expect(page.getByRole('status')).toContainText('tino@example.com');
  expect(sent).toEqual({ email: 'tino@example.com' });
});

test('a refused address shows a plain message, never the raw response', async ({ page }) => {
  await page.route('**/api/signup', async (route) => {
    if (route.request().method() === 'OPTIONS') {
      return route.fulfill({ status: 204, headers: cors });
    }
    await route.fulfill({ status: 503, headers: cors, contentType: 'text/html', body: '<html>nginx</html>' });
  });

  await page.goto('/');
  await page.getByLabel('Email').fill('tino@example.com');
  await page.getByRole('button', { name: 'Sign up' }).click();

  await expect(page.getByRole('alert')).toContainText('Too many attempts');
  await expect(page.getByRole('alert')).not.toContainText('nginx');
});

test('the sign-up button stays disabled until an address is entered', async ({ page }) => {
  await page.goto('/');
  await expect(page.getByRole('button', { name: 'Sign up' })).toBeDisabled();
});
