// Build-time prerender for the public marketing routes (Phase 3 of
// docs/plans/web-solana-seo-sitemap-plan.md).
//
// Why this exists: web-solana is pure CSR (no SSR framework), served as flat
// static files by nginx. React 19's native <title>/<meta> hoisting (used by
// SeoHead.tsx) only ever runs client-side here, so zero-JS bots (social
// link-preview bots, most non-Google crawlers) never see it — only real
// browsers and Google's second-wave JS render do. This script closes that
// gap for the routes that matter most for SEO by rendering them to real
// static HTML at build time, via Vite's own SSR module loader (no extra
// framework, no routing migration) — see the plan's Option B / §5.
//
// Deliberately scoped to the 11 public routes that don't touch
// @solana/wallet-adapter-react: those libraries reach for `window`/storage
// at module scope and are not safe to execute in this Node render pass.
// Players.tsx and TournamentDetail.tsx stay CSR-only — their client-side
// SeoHead still covers real browsers and Googlebot's JS pass, just not
// zero-JS bots. That's a documented trade-off, not an oversight.
//
// Run after `vite build` (needs the hashed dist/index.html as a template).

import { createServer } from 'vite';
import { renderToStaticMarkup } from 'react-dom/server';
import { StaticRouter } from 'react-router-dom';
import React from 'react';
import fs from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const __dirname = path.dirname(fileURLToPath(import.meta.url));
const root = path.resolve(__dirname, '..');
const distDir = path.join(root, 'dist');

const ROUTES = [
  { path: '/home', file: 'src/pages/Home.tsx', name: 'Home' },
  { path: '/features', file: 'src/pages/Features.tsx', name: 'Features' },
  { path: '/play', file: 'src/pages/Play.tsx', name: 'default' },
  { path: '/tournaments', file: 'src/pages/Tournaments.tsx', name: 'Tournaments' },
  { path: '/computer', file: 'src/pages/ChessComputer.tsx', name: 'ChessComputer' },
  { path: '/legal', file: 'src/pages/Legal.tsx', name: 'default' },
  { path: '/compliance', file: 'src/pages/Compliance.tsx', name: 'default' },
  { path: '/anti-cheat', file: 'src/pages/AntiCheat.tsx', name: 'default' },
  { path: '/news/release', file: 'src/pages/NewsRelease.tsx', name: 'default' },
  { path: '/launch', file: 'src/pages/Launch.tsx', name: 'default' },
  { path: '/waitlist', file: 'src/pages/Waitlist.tsx', name: 'Waitlist' },
];

// React 19 hoists <title>/<meta>/<link> to the front of the rendered string
// (verified empirically against the installed react-dom version — see plan
// §5 notes). <script type="application/ld+json"> does NOT hoist and stays
// in place, which is fine: JSON-LD is valid anywhere in the document.
const HEAD_TAG = /^(?:<title>.*?<\/title>|<meta[^>]*\/?>|<link[^>]*\/?>)/;

function splitHoistedHead(html) {
  let rest = html;
  let head = '';
  for (;;) {
    const m = rest.match(HEAD_TAG);
    if (!m) break;
    head += m[0];
    rest = rest.slice(m[0].length);
  }
  return { head, body: rest };
}

async function main() {
  if (!fs.existsSync(distDir)) {
    throw new Error('dist/ not found — run `vite build` before prerender.mjs');
  }
  const template = fs.readFileSync(path.join(distDir, 'index.html'), 'utf-8');

  const vite = await createServer({
    root,
    server: { middlewareMode: true },
    appType: 'custom',
    logLevel: 'warn',
  });

  let ok = 0;
  for (const route of ROUTES) {
    try {
      const mod = await vite.ssrLoadModule(path.join(root, route.file));
      const Component = route.name === 'default' ? mod.default : mod[route.name];
      if (!Component) throw new Error(`export "${route.name}" not found in ${route.file}`);

      const { OrganizationSchema } = await vite.ssrLoadModule(
        path.join(root, 'src/components/StructuredData.tsx'),
      );

      let rendered = renderToStaticMarkup(
        React.createElement(StaticRouter, { location: route.path },
          React.createElement(React.Fragment, null,
            React.createElement(OrganizationSchema),
            React.createElement(Component),
          ),
        ),
      );
      // framer-motion's `initial={{ opacity: 0, y: 20 }}` page-transition
      // wrapper renders its literal initial values as an inline style with
      // no animation loop running server-side — left as-is, the static
      // snapshot would show content at opacity:0, which Google's renderer
      // can flag as intentionally hidden text. Neutralize it so crawlers
      // see the fully-visible, settled state (client JS animates in from
      // this same state on real page loads, so nothing user-visible changes).
      rendered = rendered.replace(/style="opacity:0;transform:translateY\(20px\)"/g, '');
      const { head, body } = splitHoistedHead(rendered);

      // index.html deliberately ships with only a bare <title> (see its own
      // comment — no default description/OG/canonical, to avoid duplicate
      // tags on non-prerendered routes once SeoHead hoists client-side).
      // Replace that bare title with this route's real hoisted tags, and
      // drop the rendered body into #root so crawlers get actual content,
      // not just an empty shell (client JS still remounts over it on load).
      let out = template.replace(/<title>.*?<\/title>/, '');
      out = out.replace('</head>', `${head}</head>`);
      out = out.replace('<div id="root"></div>', `<div id="root">${body}</div>`);

      const outDir = path.join(distDir, route.path.replace(/^\//, ''));
      fs.mkdirSync(outDir, { recursive: true });
      fs.writeFileSync(path.join(outDir, 'index.html'), out, 'utf-8');
      ok++;
      console.log(`prerendered ${route.path} -> dist${route.path}/index.html`);
    } catch (err) {
      console.error(`FAILED to prerender ${route.path}:`, err.message);
      throw err;
    }
  }

  await vite.close();
  console.log(`\nPrerendered ${ok}/${ROUTES.length} public routes.`);
}

main().catch((err) => {
  console.error(err);
  process.exit(1);
});
