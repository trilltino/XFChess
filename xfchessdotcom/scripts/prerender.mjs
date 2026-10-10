// Prerender public routes after vite build using its hashed HTML template.
// Exclude routes importing wallet adapters: they access browser globals at module scope.

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
  { path: '/home', file: 'src/pages/marketing/home.tsx', name: 'Home' },
  { path: '/play', file: 'src/pages/play/play.tsx', name: 'default' },
  { path: '/tournaments', file: 'src/pages/tournaments/tournaments.tsx', name: 'Tournaments' },
  { path: '/features', file: 'src/pages/marketing/features.tsx', name: 'Features' },
];

// React hoists title/meta/link; JSON-LD scripts remain in place and are valid there.
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
      // Neutralize server-rendered animation opacity so static content is visible
      // without a client animation loop.
      rendered = rendered.replace(/style="opacity:0;transform:translateY\(20px\)"/g, '');
      const { head, body } = splitHoistedHead(rendered);

      // Replace the template title with route metadata and render its body into root.
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
