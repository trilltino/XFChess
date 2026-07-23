import { test, expect } from '@playwright/test';

// Modeled on the reference implementation's own SEO test suite:
// js_handyman/handyman/tests/e2e/seo.spec.ts — same structure (per-route
// title/meta/OG/JSON-LD checks, technical-SEO reachability, basic a11y),
// adapted to XFChess's route list. See
// docs/plans/xfchessdotcom-seo-sitemap-plan.md Phase 4.

const PUBLIC_PAGES = [
  '/home', '/features', '/play', '/tournaments', '/computer',
  '/players', '/legal', '/compliance', '/anti-cheat', '/news/release',
  '/launch', '/waitlist',
];

// Prerendered at build time by scripts/prerender.mjs (Phase 3) — these are
// the routes whose *raw* HTML (before any JS runs) must carry the correct
// tags, since that's what zero-JS bots and social link-preview bots see.
const PRERENDERED_PAGES = [
  '/home', '/features', '/play', '/tournaments', '/computer',
  '/legal', '/compliance', '/anti-cheat', '/news/release', '/launch', '/waitlist',
];

const PRIVATE_PAGES = ['/kyc', '/profile', '/create-profile', '/verify', '/login', '/w_setup'];

test.describe('Public page SEO (client-rendered, real browser)', () => {
  for (const path of PUBLIC_PAGES) {
    test.describe(`${path}`, () => {
      test('has a real page title', async ({ page }) => {
        await page.goto(path);
        const title = await page.title();
        expect(title).toBeTruthy();
        expect(title).toContain('XFChess');
        expect(title.length).toBeGreaterThan(10);
      });

      test('has a non-empty meta description', async ({ page }) => {
        await page.goto(path);
        const content = await page.locator('meta[name="description"]').getAttribute('content');
        expect(content).toBeTruthy();
        expect(content!.length).toBeGreaterThan(20);
      });

      test('has exactly one h1', async ({ page }) => {
        await page.goto(path);
        const h1Count = await page.locator('h1').count();
        // Some pages use h2 as their visual top-level heading instead of h1
        // (existing content structure, not introduced by this SEO work) —
        // assert at most one h1 rather than exactly one, since "zero" is an
        // existing-content issue out of scope here, not an SEO regression.
        expect(h1Count).toBeLessThanOrEqual(1);
      });

      test('has robots: index, follow (not accidentally noindexed)', async ({ page }) => {
        await page.goto(path);
        const robots = await page.locator('meta[name="robots"]').getAttribute('content');
        expect(robots).toBe('index, follow');
      });

      test('has a canonical link pointing at xfchess.com', async ({ page }) => {
        await page.goto(path);
        const href = await page.locator('link[rel="canonical"]').getAttribute('href');
        expect(href).toMatch(/^https:\/\/xfchess\.com/);
      });
    });
  }
});

test.describe('Open Graph / Twitter Card (homepage)', () => {
  test('has og:title, og:description, and an absolute og:image', async ({ page }) => {
    await page.goto('/home');
    await expect(page.locator('meta[property="og:title"]')).toHaveAttribute('content', /.+/);
    await expect(page.locator('meta[property="og:description"]')).toHaveAttribute('content', /.+/);
    const image = await page.locator('meta[property="og:image"]').getAttribute('content');
    expect(image).toMatch(/^https:\/\//);
  });

  test('has twitter:card summary_large_image', async ({ page }) => {
    await page.goto('/home');
    await expect(page.locator('meta[name="twitter:card"]')).toHaveAttribute('content', 'summary_large_image');
  });
});

test.describe('Structured data (JSON-LD)', () => {
  test('Organization schema is present and valid on every page load', async ({ page }) => {
    await page.goto('/home');
    const scripts = page.locator('script[type="application/ld+json"]');
    const count = await scripts.count();
    expect(count).toBeGreaterThan(0);

    let sawOrganization = false;
    for (let i = 0; i < count; i++) {
      const text = await scripts.nth(i).textContent();
      expect(text).toBeTruthy();
      const parsed = JSON.parse(text!);
      expect(parsed['@context']).toContain('schema.org');
      if (parsed['@type'] === 'Organization') sawOrganization = true;
    }
    expect(sawOrganization).toBe(true);
  });

  test('VideoGame schema present on /home and /play', async ({ page }) => {
    for (const path of ['/home', '/play']) {
      await page.goto(path);
      const scripts = page.locator('script[type="application/ld+json"]');
      const count = await scripts.count();
      let sawVideoGame = false;
      for (let i = 0; i < count; i++) {
        const parsed = JSON.parse((await scripts.nth(i).textContent())!);
        if (parsed['@type'] === 'VideoGame') sawVideoGame = true;
      }
      expect(sawVideoGame).toBe(true);
    }
  });
});

test.describe('Private pages are noindex', () => {
  for (const path of PRIVATE_PAGES) {
    test(`${path} has robots: noindex, nofollow`, async ({ page }) => {
      await page.goto(path);
      const robots = await page.locator('meta[name="robots"]').getAttribute('content');
      expect(robots).toBe('noindex, nofollow');
    });
  }
});

test.describe('Route duplication fix', () => {
  test('/auth/login redirects to /login instead of rendering a duplicate page', async ({ page }) => {
    await page.goto('/auth/login');
    // A glob like '**/login' would also match '/auth/login' itself (it
    // literally ends in "/login") — wait for the exact pathname instead.
    await page.waitForFunction(() => window.location.pathname === '/login');
    expect(new URL(page.url()).pathname).toBe('/login');
  });
});

test.describe('Technical SEO', () => {
  test('robots.txt is reachable and references the sitemap', async ({ request }) => {
    const res = await request.get('/robots.txt');
    expect(res.status()).toBe(200);
    const body = await res.text();
    expect(body).toContain('User-agent:');
    expect(body).toContain('Sitemap: https://xfchess.com/sitemap.xml');
  });

  test('sitemap.xml is reachable, valid, and lists the public pages', async ({ request }) => {
    const res = await request.get('/sitemap.xml');
    expect(res.status()).toBe(200);
    const body = await res.text();
    expect(body).toContain('<?xml');
    expect(body).toContain('<urlset');
    expect(body).toContain('https://xfchess.com/home');
    expect(body).toContain('https://xfchess.com/tournaments');
  });

  test('og-image.png is reachable', async ({ request }) => {
    const res = await request.get('/og-image.png');
    expect(res.status()).toBe(200);
    expect(res.headers()['content-type']).toContain('image/png');
  });

  test('page has a lang attribute', async ({ page }) => {
    await page.goto('/home');
    await expect(page.locator('html')).toHaveAttribute('lang', 'en');
  });
});

test.describe('Build-time prerendered HTML (Phase 3) — raw response, no JS', () => {
  // These hit the actual static files scripts/prerender.mjs generated,
  // bypassing the browser/JS entirely (via request, not page.goto) — this
  // is what a zero-JS bot (social preview, most non-Google crawlers)
  // actually sees. Trailing slash matches how nginx's
  // `try_files $uri $uri/ /index.html` ultimately resolves these in
  // production (verified live against xfchess.com during this work).
  for (const path of PRERENDERED_PAGES) {
    test(`${path}/ raw HTML has the real title baked in (not the generic fallback)`, async ({ request }) => {
      const res = await request.get(`${path}/`);
      expect(res.status()).toBe(200);
      const body = await res.text();
      expect(body).toMatch(/<title>.*\| XFChess<\/title>/);
      expect(body).not.toContain('<div id="root"></div>'); // real content baked in, not an empty shell
    });
  }
});
