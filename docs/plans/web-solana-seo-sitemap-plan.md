# web-solana: SEO, Sitemap & Structured Data — End-to-End Plan

Status: **implemented** (Phases 0–4 shipped 2026-07-23; see §8 for what actually happened vs. what was originally proposed)
Scope: `web-solana/` only (the React marketing/account frontend at `xfchess.com`) — not the Bevy game client, not `backend/`.
Researched: 2026-07-23

## 1. Why this document exists

`web-solana` currently has **zero** SEO infrastructure: no `robots.txt`, no `sitemap.xml`, no per-route `<title>`/description, no Open Graph or Twitter Card tags, no structured data, and no Open Graph image asset. Every one of the 26 routes serves the same generic `<title>XFChess</title>` from a single static `index.html`. This plan is the full remediation, researched against current (2026) best practice and against a working reference implementation already in this user's own `js_handyman` project.

## 2. Current-state audit

### 2.1 Stack facts that shape every decision below

- React 19.2.8 + Vite + `react-router-dom` 7.18.1, using **classic declarative mode** (`<BrowserRouter>`/`<Routes>`/`<Route>` in [App.tsx](../../web-solana/src/App.tsx#L110)) — not React Router v7's newer "Framework Mode".
- **Pure client-side rendering (CSR).** No SSR, no SSG, no prerendering. `vite build` produces one `dist/index.html` shell; everything is drawn by JS after load.
- Deployed as **static files**, not behind a Node/Rust process: `deploy.ps1` uploads `web-solana/dist/*` to `/opt/xfchess/web/`, and nginx serves it directly — `location / { try_files $uri $uri/ /index.html; }` ([nginx.conf](../../deploy/nginx/nginx.conf#L55)). There is no per-request server-side templating step in front of this app today (unlike `backend/`, which is Axum).
- ~~`vite.config.ts` uses `base: './'` (relative asset base) — irrelevant to canonical URLs~~ **Correction (§8): this was wrong and has been fixed.** `base: './'` made every asset reference relative, which resolves against the *current URL's* directory, not the site root. Any direct/fresh navigation to a 2+-segment route (`/news/release`, `/tournament/:id/standings`, …) 404'd every JS/CSS asset and never booted — invisible during normal use (in-app `<Link>` navigation never re-resolves asset paths) but fatal for exactly the audience this plan is about: crawlers, shared links, and page refreshes. Fixed to `base: '/'`.
- `index.html` today ([web-solana/index.html](../../web-solana/index.html)):
  - One static `<title>XFChess</title>` and one static `<meta name="description">` for all 26 routes.
  - A leftover **GitHub Pages SPA-redirect script** (`sessionStorage.redirect` / `history.replaceState` dance). This app is not deployed to GitHub Pages anymore — it's vestigial from an earlier deploy target, rewrites the URL on every load, and is dead weight that should be deleted as part of this work (it's not neutral: rewriting history on load is exactly the kind of thing that confuses a crawler's view of "what URL is this").
  - No Open Graph, no Twitter Card, no JSON-LD, no canonical link.
- No `document.title` usage anywhere in `src/` — confirms there is genuinely zero per-page metadata today, not just an oversight in one place.
- No OG image asset in `public/` (only in-app hero/logo art at other aspect ratios: `src/assets/hero.png`, `xfchess-logo.png`, `high-fidelity-chess.png`).
- `public/` already has one "well-known root file" precedent: [`actions.json`](../../web-solana/public/actions.json) (Solana Blinks/Actions spec). Adding `robots.txt` / `sitemap.xml` / `llms.txt` next to it is consistent with how this project already does things — no nginx changes needed, since static files under `public/` are served as-is by `try_files` before the SPA fallback.

### 2.2 Full route audit (26 routes, [App.tsx:449-473](../../web-solana/src/App.tsx#L449-L473))

| Route | Public/indexable? | Notes |
|---|---|---|
| `/home` (+ `/` redirect) | ✅ Yes | Primary landing page |
| `/features` | ✅ Yes | Marketing |
| `/play` | ✅ Yes | Marketing (download/play entry) |
| `/tournaments` | ✅ Yes | List page |
| `/tournament/:id` | ✅ Yes (dynamic) | One real tournament per id — genuinely unique, indexable content |
| `/computer` | ✅ Yes | Marketing |
| `/players` | ⚠️ Maybe | Public leaderboard — indexable if it has stable per-visit content and isn't wallet-gated |
| `/legal` | ✅ Yes | Low-value but harmless to index |
| `/compliance` | ✅ Yes | Same |
| `/anti-cheat` | ✅ Yes | Same |
| `/news/release` | ✅ Yes | Content page |
| `/launch` | ✅ Yes | Marketing |
| `/waitlist` | ✅ Yes | Marketing/conversion page |
| `/tournament/:id/standings` | ❌ No | Live/derived data, not unique long-lived content |
| `/tournament/:id/play` | ❌ No | In-game, requires auth/session state |
| `/spectate/:game_id` | ❌ No | Ephemeral, one game, no lasting value |
| `/verify` | ❌ No | Account action |
| `/profile`, `/create-profile` | ❌ No | Wallet-gated, user-specific |
| `/kyc` | ❌ No, and should be `Disallow`d | Sensitive form |
| `/login`, `/auth/login` | ❌ No | **Duplicate routes to the same component** — pick one canonical, 301 the other, or at minimum canonical-tag one of them so this doesn't read as thin duplicate content |
| `/auth/lichess/callback` | ❌ No | OAuth callback, never a landing page |
| `/w_setup` | ❌ No | Setup flow |

**~13 public routes** (12 static + the dynamic `/tournament/:id` family) belong in the sitemap. The rest belong in `robots.txt` `Disallow` and/or a `noindex` meta tag.

## 3. Reference implementation already in this user's environment

`C:\Users\isich\js_handyman` (two sibling projects, `handyman` — Leptos SSR — and `food_man` — Axum + React) already solved this problem twice, and the second one (`food_man`) is the closer analog to `web-solana` because it's also a React SPA. Patterns worth reusing directly:

**`shared/src/metadata.rs`** — a small typed `PageMetadata { title, description, og_image, canonical_url }` struct with per-page-type factory constructors (`for_homepage()`, `for_service()`, `for_blog()`). This is the right shape for a TS equivalent in `web-solana`.

**`food_man/backend/src/seo.rs`** — the architecturally important one. It injects per-route `<title>`/OG/Twitter/JSON-LD into the built `index.html` **server-side, via string replacement against marker comments already in the template**, not via a real HTML parser (correctly reasoned in the code comment: it's one self-authored template, not arbitrary HTML, so a parser is solving a problem that doesn't exist). Its own doc comment states the exact reason this matters:

> "This has to happen server-side, not from React after the page loads — search crawlers that don't execute JS, and every social link-preview bot (WhatsApp, Facebook, Twitter/X, Slack, iMessage) only ever see the raw HTTP response, never the client-rendered DOM."

Other details worth copying:
- `noindex` is a per-route lookup, not a global flag — unknown/dynamic paths default to `noindex` (deny-by-default, not allow-by-default).
- JSON-LD is only injected on the page it's actually about (home page gets `FoodEstablishment`; `/checkout` gets none) — resist the temptation to blast the same JSON-LD onto every page.
- The OG image is served from a **stable, unhashed path** (`/og-image.png` from `public/`, not a Vite content-hashed asset) — called out explicitly in a test (`og_and_twitter_image_point_at_the_stable_public_asset`) because social platforms cache OG images by URL; a hash that changes every build would break their cache.
- Real unit tests assert on the rendered HTML string per route (`home_page_is_indexable_with_canonical_and_structured_data`, `checkout_page_is_noindex_with_no_structured_data`, `unknown_path_falls_back_to_noindex_shop_name`).

**`handyman/frontend-leptos/src/components/seo.rs`** — the `LocalBusinessSchema`/`HandymanLocalBusinessSchema` JSON-LD components show the right level of structured-data richness (address, geo, opening hours, `aggregateRating`, `hasOfferCatalog`) — the *shape* transfers even though XFChess needs `Organization`/`VideoGame`/`SportsEvent` instead of `LocalBusiness`.

**`handyman/tests/e2e/seo.spec.ts`** and `food_man`'s Rust unit tests — both are good models for the verification suite in §7.

**The one thing that does *not* transfer directly:** `food_man` can do this per-request because a Rust/Axum process sits in front of every request. `web-solana` is served by nginx as flat static files with no such process today. This changes *where* the injection has to happen (build time, not request time) — see §4.

## 4. Research findings (live web research, 2026-07-23)

1. **Google Search Central confirms `<priority>`/`<changefreq>` in sitemaps are ignored outright** (Gary Illyes, Google Search team — these fields were dropped from ranking/crawl signals because they were universally gamed). Do not spend effort tuning them. **`<lastmod>` is the field that matters**, and only when it's consistently accurate — a sitemap with fake/static lastmod dates is worse than no lastmod at all.
2. **React 19 has native document-metadata hoisting**: `<title>`, `<meta>`, `<link>` can render anywhere in the component tree and React moves them into `<head>` automatically — no `react-helmet` needed for that part. **Caveat the research summaries don't emphasize enough**: this hoisting happens client-side, after JS runs. It only reaches crawlers/bots "immediately" when paired with SSR. In a pure-CSR app like `web-solana`, it still leaves a window where the initial HTTP response has no per-route tags — it helps real browsers and Googlebot's second JS-rendering wave, but does **nothing** for zero-JS bots. Confirmed independently by `food_man`'s own code comment (§3).
3. **`react-helmet-async` v3.0 (Mar 2026)** now detects React 19 at runtime and just delegates to native hoisting — so there's no reason to add it as a dependency here; use React 19's built-ins directly.
4. **React Router v7 has an official `prerender` config** in `react-router.config.ts` (boolean, array of paths, or an async function) for apps running in its newer "Framework Mode". This is the "correct" long-term answer for RR7 apps — but it requires migrating off the current declarative `<BrowserRouter>` setup to Framework Mode conventions (route config file, loaders/actions patterns). That's a real architecture change touching every route, with regression risk to the existing wallet-adapter/session logic — **not** something to take on just for SEO. Noted as a future option, not the recommended path.
5. **`vite-react-ssg`** is actively maintained (v0.9.2, updated within the last week as of this research) and works with the existing declarative React Router setup — no Framework Mode migration required. Its own maintainers now point RR7-Framework-Mode users at the native `prerender` config instead, but for library-mode RR7 (what `web-solana` uses), `vite-react-ssg` remains the maintained path. This is the **lower-risk** route to real build-time prerendering.
6. **AI crawlers are now a material share of traffic** (GPTBot overtook ClaudeBot in mid-2026 per Cloudflare Radar; ~226 distinct AI crawler user agents catalogued). **`llms.txt` is mostly hype, not signal** — real-world data shows GPTBot/ClaudeBot/PerplexityBot overwhelmingly skip it and crawl HTML directly. It's cheap to add but should be sized as a low-priority nice-to-have, not sold internally as a serious SEO lever. The actually consequential move is making sure `robots.txt` doesn't accidentally block the AI retrieval bots if visibility in AI answer engines is a goal.
7. **`Organization` schema (JSON-LD) is called out specifically as high-value for AI-answer-engine trust/citation** in 2026 commentary, distinct from classic rich-snippet SEO — an `Organization` block with `sameAs` links to real social profiles is cheap and worth prioritizing.
8. **Google's gambling *advertising* policy explicitly treats skill-determined games (chess named as an example) more favorably than chance-based gambling** — but this is an Ads-policy distinction, not an organic-Search-index restriction; organic Search does not have an equivalent blanket gambling carve-out. This only becomes relevant if XFChess later runs paid Google/Meta ads for the wagered-play product — separate workstream from organic SEO, flagged here so it isn't forgotten, consistent with the existing 4-country legal analysis already on file (chess = skill game).
9. **OG image spec, 2026 consensus**: 1200×630px (1.91:1), key content inside the center 66% (platforms crop differently), file size ideally under 300KB (WhatsApp compresses aggressively even though the platform max is 8MB), explicit `og:image:width`/`og:image:height` tags, PNG for text/graphics or JPEG for photos. A secondary 1200×1200 square variant is a nice-to-have for platforms that crop square.
10. **Real-world comparator check** (both fetched live): **lichess.org has no `sitemap.xml` at all** (404) and relies on a five-line `robots.txt` plus inherent content/backlink authority. **chess.com** (far larger page count — per-user, per-game, per-article) uses a **sitemap index** (`sitemapindex.xml`). Takeaway: a sitemap is necessary hygiene, not a ranking silver bullet, and a sitemap-*index* pattern would be premature over-engineering for `web-solana`'s current ~13 public routes — a flat `urlset` is correct today; revisit only if/when public per-tournament or per-player pages grow into the hundreds.

## 5. Architecture decision: how to get metadata in front of bots that don't run JS

Three options were evaluated. **Recommendation: start with Option A, keep Option B as the documented fallback if A proves awkward for any route, treat Option C as a future/optional escalation.**

| | Option A: Build-time prerender (`vite-react-ssg`) | Option B: Custom build-time render script | Option C: React Router v7 Framework Mode migration |
|---|---|---|---|
| What it does | Generates real static `index.html` per route at `vite build` time, each with the actual rendered DOM + injected head tags | A small Node script using `react-dom/server` to render just the ~13 public routes to static HTML, string-replacing markers in the template (mirrors `food_man`'s pattern, but at build time since there's no per-request process) | Migrate routing to RR7's Framework Mode; use its native `prerender` config |
| Migration risk | Low — works with current declarative routing | Low — no dependency on routing internals at all | High — changes routing conventions app-wide, touches wallet/session logic paths |
| Maintenance | Community package, actively maintained today | Fully owned, no external dependency risk, but hand-rolled | Officially supported by RR7, but ties the whole app's architecture to it |
| Effort | Medium (add plugin, configure per-route data) | Medium (write + test the script) | High |
| Best for | This task, now | Fallback if a route's data-fetching doesn't play nicely with the plugin's render step | A later, intentional architecture decision — not an SEO side-effect |

Either A or B produces real static HTML files that nginx's existing `try_files $uri $uri/ /index.html` will serve **as-is, with no nginx changes** — `try_files` finds e.g. `dist/tournaments/index.html` as a real file before ever falling back to the SPA shell.

Only the ~13 public routes from §2.2 get prerendered. Everything else stays pure CSR — prerendering a wallet-gated or in-game route adds no SEO value and risks baking stale or placeholder-shaped content into a static file.

## 6. Implementation plan

### Phase 0 — cleanup (do first, no dependencies)
- [ ] Delete the dead GitHub Pages SPA-redirect script from `web-solana/index.html`.
- [ ] Decide the canonical URL for the `/login` vs `/auth/login` duplicate (same component, two paths) and for `/profile` vs `/create-profile` — either consolidate to one route with a redirect, or at minimum make sure only one is canonical/indexable.

### Phase 1 — static foundation (`public/`)
- [ ] `web-solana/public/robots.txt`:
  ```
  User-agent: *
  Allow: /
  Disallow: /kyc
  Disallow: /profile
  Disallow: /create-profile
  Disallow: /verify
  Disallow: /w_setup
  Disallow: /auth/
  Disallow: /login
  Disallow: /tournament/*/play
  Disallow: /tournament/*/standings
  Disallow: /spectate/

  # Explicitly welcome AI answer-engine crawlers (not blocked by default,
  # listed for clarity/intent — see plan §4.6)
  User-agent: GPTBot
  Allow: /
  User-agent: ClaudeBot
  Allow: /
  User-agent: PerplexityBot
  Allow: /

  Sitemap: https://xfchess.com/sitemap.xml
  ```
- [ ] `web-solana/public/sitemap.xml` — flat `urlset`, the ~13 public routes from §2.2, real `lastmod` per page (see Phase 5 for how these stay accurate), no effort spent on `priority`/`changefreq` per §4.1.
- [ ] `web-solana/public/og-image.png` — 1200×630, <300KB, derived from existing `src/assets/hero.png` / `xfchess-logo.png`, key content centered. Optionally a second 1200×1200 square variant.
- [ ] Optional, low-priority: `web-solana/public/llms.txt` (brief, honest site summary) — size expectations correctly per §4.6, don't oversell internally.
- [ ] Optional, low-priority: `web-solana/public/manifest.json` (basic web app manifest — trivial addition, minor mobile/PWA signal).

### Phase 2 — typed metadata registry + client-side head component
- [ ] `web-solana/src/lib/seo/metadata.ts` — `PageMetadata` type (`title`, `description`, `ogImage?`, `canonicalPath`, `noindex?`) plus a registry keyed by route, and factory helpers for the dynamic routes (`forTournament(id, name)`, mirroring `PageMetadata::for_service()` in the reference project).
- [ ] `web-solana/src/components/SeoHead.tsx` — renders `<title>`, `<meta name="description">`, `<link rel="canonical">`, `og:*`, `twitter:*` using **React 19's native hoisting** (no `react-helmet-async` dependency — see §4.3). Mounted once per page component, fed from the registry.
- [ ] `web-solana/src/components/StructuredData.tsx` — JSON-LD via a `<script type="application/ld+json">` tag:
  - `Organization` (with `sameAs` → real Discord/X/GitHub links) on every page via a layout-level mount.
  - `VideoGame` or `SoftwareApplication` on `/home` and `/play`.
  - `SportsEvent` (schema.org `Event` subtype) on `/tournament/:id`, populated from the real tournament data already fetched for that page (name, `startDate`, `location` if applicable, `organizer`).
  - `BreadcrumbList` on the `/tournament/:id/*` family.
- [ ] Wire `<SeoHead>`/`<StructuredData>` into each of the ~13 public page components (and explicitly opt the private ones into a `noindex` meta tag instead, matching the deny-by-default posture in `food_man`'s `page_meta()`).

### Phase 3 — real static HTML for zero-JS bots
- [ ] Adopt `vite-react-ssg` (Option A, §5): configure it to prerender exactly the ~13 public routes; verify each produces a real `dist/<route>/index.html` with the correct baked-in `<title>`/OG/JSON-LD (not just an empty `<div id="root">`).
- [ ] Confirm the dynamic `/tournament/:id` route path is handled via the plugin's async path-list function (fetch real tournament IDs from the backend at build time), not hardcoded.
- [ ] Verify against nginx locally (or on a staging deploy) that `try_files` picks up the new per-route static files with zero config changes.
- [ ] If any specific route's data-fetching doesn't fit the plugin's model, fall back to Option B (hand-rolled render script) for just that route rather than forcing it.

### Phase 4 — verification suite
- [ ] New Playwright spec `web-solana/tests/e2e/seo.spec.ts`, modeled directly on `js_handyman/handyman/tests/e2e/seo.spec.ts`:
  - Per public route: has a non-empty `<title>`, has `meta[name=description]`, has exactly one `<h1>`, has `lang` on `<html>`.
  - `robots.txt` and `sitemap.xml` both return 200.
  - Homepage has `og:title`/`og:description`/`og:image` (and the image URL is absolute `https://`).
  - JSON-LD present and is valid, parseable JSON with `@context` containing `schema.org`.
  - Private routes (`/kyc`, `/profile`, etc.) render a `noindex` meta tag.
- [ ] Run this suite in CI on every `web-solana` change (matches the existing `npm run lint`/`build` gate already in the repo).

### Phase 5 — submission & ongoing accuracy
- [ ] Verify domain ownership + submit sitemap in **Google Search Console**.
- [ ] Verify + submit in **Bing Webmaster Tools** (also feeds DuckDuckGo/Yahoo in part).
- [ ] Keep `sitemap.xml` `lastmod` honest: tie it to either (a) `git log -1 --format=%aI -- <page-source-file>` at build time, or (b) a real content-update timestamp if/when tournament or profile pages get their own backend-tracked `updated_at`. Do not hand-wave static dates — Google explicitly discounts a sitemap once it catches `lastmod` lying (§4.1).
- [ ] Re-run the Playwright SEO suite after every deploy; watch Search Console's Coverage report for unexpected `noindex`/crawl errors, especially after any route changes.
- [ ] Revisit Option C (RR7 Framework Mode) only if the app's data-loading architecture is separately moving that direction — not as a dedicated SEO project.

## 7. Open questions for the user

1. **`/login` vs `/auth/login`, `/profile` vs `/create-profile`** — same components, different paths. Consolidate, redirect, or just canonical-tag one? **Not resolved** — shipped with both still live, both `noindex`; no redirect/consolidation done. (Phase 0)
2. **Is `/players` meant to be publicly indexable?** **Resolved during implementation**: read the component directly — it's a public wallet-pubkey/username search tool usable with or without a connected wallet (only an extra "my profile" panel is gated), so it's genuinely public. Included in the sitemap.
3. **AI-answer-engine visibility** — **Resolved**: shipped with GPTBot/ClaudeBot/PerplexityBot/Google-Extended explicitly allowed in `robots.txt`.
4. ~~**Social profile links**~~ **Resolved**: read straight from the real, live links already in `src/components/Footer.tsx` (`twitter.com/xfchess`, `github.com/xfchess`, `youtube.com/xfchess`) rather than guessed — used verbatim in `OrganizationSchema`'s `sameAs`.
5. ~~**OG image design**~~ **Resolved**: generated from the real brand (site's actual `Cinzel`-adjacent serif wordmark styling and `--bg:#000000`/`--primary:#fff` color tokens pulled from `index.css`, not the photographic hero art, which didn't read as a wordmark at OG-preview size) — `public/og-image.png`, 1200×630, 45KB.

## 8. Implementation log (what actually shipped, 2026-07-23)

Phases 0, 1, 2, and 4 shipped as planned. Phase 3 shipped, but via a different, lower-risk mechanism than originally proposed — and two real, previously-invisible bugs were found and fixed along the way by actually testing rather than trusting the plan's assumptions.

**Phase 3 correction — Option A (`vite-react-ssg`) was not viable without a much bigger change than estimated.** Investigating its actual setup requirements (not just its README summary) showed it requires migrating off the app's current declarative `<BrowserRouter>/<Routes>/<Route>` JSX to React Router's data-router route-objects format (`RouteRecord[]`), plus wrapping anything wallet-dependent in `<ClientOnly>` — and `WalletProvider`/`ConnectionProvider` wrap the *entire* app in `App.tsx`, per this repo's own `web-solana/CLAUDE.md` ("Wallet context wraps the entire app"). That's a routing-architecture rewrite, not a "medium effort" addition, and too much regression risk for a production app's wallet/session logic to take on as a side effect of an SEO task.

**What shipped instead**: `web-solana/scripts/prerender.mjs`, a ~130-line script using Vite's own low-level SSR module loader (`vite.ssrLoadModule`, the same primitive `vite-react-ssg` itself builds on) plus `react-dom/server`'s `renderToStaticMarkup` and React Router's `<StaticRouter>` — no new routing architecture, no framework migration. It renders the 11 public routes that don't touch `@solana/wallet-adapter-react` (verified by grep before writing a line of the script) to real static HTML files at build time, wired into `npm run build` directly. `Players.tsx` and `TournamentDetail.tsx` (both use `useWallet()`) stay CSR-only, as the plan's Option A/B comparison already anticipated as the boundary — just reached via a different tool.

**Two real bugs found by writing and running the Playwright suite against the actual build**, not by inspection:

1. **Duplicate meta tags.** `index.html` originally carried "default/fallback" `description`/`og:*`/`twitter:*`/canonical tags as a safety net. React 19 dedupes `<title>` but does *not* dedupe `<meta>`/`<link>` against tags already present in the raw HTML — so on every route that isn't one of the 11 prerendered files (all private/noindex pages, plus `/players` and `/tournament/:id`), the client-side `<SeoHead>` tags rendered *alongside* the static defaults instead of replacing them. Fixed by stripping `index.html` down to a bare `<title>` only — `<SeoHead>` is now the sole source of description/OG/canonical for every route; non-prerendered routes simply have none of that until JS runs, which is the same documented CSR limitation as before, not a new regression.
2. **The `base: './'` deep-link bug** described in §2.1's correction above — found via the exact same test run (three `/news/release` tests timed out waiting for elements that were never going to appear, because the JS bundle itself 404'd).

Both were confirmed fixed by re-running the full 85-test suite (`npx playwright test`) clean, and the `try_files` directory-index behavior the whole prerendering approach depends on was verified against the *real* production nginx (not just reasoned about): a harmless temporary file was placed at `/opt/xfchess/web/_trytest/index.html` and fetched via `https://xfchess.com/_trytest` — confirmed nginx 301-redirects to add the trailing slash, then serves the real file. That single extra redirect hop for direct/crawler fetches of a no-trailing-slash URL is expected, normal, and not worth engineering around (in-app navigation never triggers it at all).

**Also shipped, not separately called out above**: `web-solana/tests/e2e/seo.spec.ts` + `playwright.config.ts` (webServer runs the actual production build, not `vite dev`, so it's exercising the real prerendered files) — 85 assertions covering per-route title/description/canonical/robots/OG/JSON-LD, private-route noindex, robots.txt/sitemap.xml/og-image.png reachability, and a dedicated raw-HTTP (no browser JS) check against each prerendered route's actual static file.

**Still open**: the five questions in §7 above (route consolidation, `/players` public-ness confirmation, AI-crawler allow confirmation, social links — resolved from the real Footer.tsx links, no longer open — and OG image sourcing — shipped using the real `xfchess-logo.png`/site color tokens, also resolved). GSC/Bing submission (§Phase 5) is still a manual operational step for whoever has console access.

## Sources

- [Sitemaps documentation — Google Search Central](https://developers.google.com/search/docs/crawling-indexing/sitemaps/build-sitemap)
- [Sitemap Best Practices: The Complete Guide for 2026 — Nightwatch](https://nightwatch.io/blog/sitemap-best-practices/)
- [A guide to React 19's new Document Metadata feature — LogRocket](https://blog.logrocket.com/guide-react-19-new-document-metadata-feature/)
- [Managing Metadata in React 19 – No More react-helmet — RobsLog](https://robslog.com/en/managing-metadata-in-react-19-no-more-react-helmet/)
- [react-helmet-async — npm](https://www.npmjs.com/package/react-helmet-async)
- [react-router.config.ts — React Router docs](https://reactrouter.com/api/framework-conventions/react-router.config.ts)
- [Pre-Rendering — React Router docs](https://reactrouter.com/how-to/pre-rendering)
- [vite-react-ssg — npm](https://npmjs.com/vite-react-ssg) / [GitHub](https://github.com/Daydreamer-riri/vite-react-ssg)
- [AI Crawler Optimization: How Bots Like GPTBot & ClaudeBot Find You (2026) — Vemetric](https://vemetric.com/blog/ai-crawler-optimization)
- [GEO Data Report 2026: Which AI Crawlers & LLM Bots Take the Most and Give the Least? — SEOmator](https://seomator.com/blog/crawl-to-refer-ratio-ai-crawlers-llm-bots)
- [LLMs.txt in 2026: The Full Guide — limy.ai](https://limy.ai/blog/llms.txt-in-2026-the-full-guide)
- [Structured Data in 2026: The Schema Markup AI Actually Uses — Globerunner](https://globerunner.com/structured-data-schema-markup-ai-2026/)
- [OG Image Sizes 2026: Facebook, X, LinkedIn (1200x630) — Krumzi](https://www.krumzi.com/blog/open-graph-image-sizes-for-social-media-the-complete-2026-guide)
- [Gambling and games — Google Ads Policies Help](https://support.google.com/adspolicy/answer/15132179?hl=en)
- `https://lichess.org/robots.txt` (fetched live, 2026-07-23)
- `https://www.chess.com/robots.txt` (fetched live, 2026-07-23 — references `sitemapindex.xml`)
- Reference implementation: `C:\Users\isich\js_handyman\handyman\shared\src\metadata.rs`, `frontend-leptos\src\components\seo.rs`, `tests\e2e\seo.spec.ts`
- Reference implementation: `C:\Users\isich\js_handyman\food_man\backend\src\seo.rs`
