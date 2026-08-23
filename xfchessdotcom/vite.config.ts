import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'
import path from 'path'

// https://vite.dev/config/
export default defineConfig({
  plugins: [react()],
  // Absolute root-relative (Vite's default — spelled out to make the
  // intent explicit and stop anyone reintroducing base: './'). This app is
  // served from the domain root (xfchess.com/), not a subpath, so absolute
  // /assets/... references are correct. A relative base broke JS/CSS
  // loading for any direct navigation to a 2+ segment URL (e.g.
  // /news/release, /tournament/:id/standings): the browser resolves
  // ./assets/ relative to the CURRENT URL's directory, not the site root,
  // so a nested path 404s every asset and the page never boots. Only
  // affects direct/fresh navigation (a crawler, a shared link, a page
  // refresh) — in-app <Link> client-side routing never re-resolves asset
  // paths, so this was invisible during normal use.
  base: '/',
  resolve: {
    alias: {
      '/wasm/xfchess_wasm.js': path.resolve(__dirname, '../xfchess-wasm/pkg/xfchess_wasm.js'),
    },
  },
  server: {
    port: 5173,
    strictPort: true,
    fs: {
      allow: ['..', './pkg'],
    },
    proxy: {
      // Forward /api/** to the local backend in dev so apiPost('') relative URLs resolve.
      '/api': {
        target: 'http://localhost:8090',
        changeOrigin: true,
      },
    },
  },
  optimizeDeps: {
    exclude: ['xfchess-wasm'],
  },
  // NOTE: deliberately no `advancedChunks` group for Privy.
  //
  // The Privy SDK is ~3.8 MB raw / ~1.1 MB gzip — on its own, five times the
  // weight of the entire rest of this app. It is therefore loaded via dynamic
  // import (see src/privy/privyRuntime.ts), which rolldown already splits into
  // its own lazily-fetched chunk.
  //
  // An earlier attempt added `advancedChunks: { groups: [{ name: 'privy', ... }] }`
  // to "isolate" it. That was actively harmful: forcing those modules into one
  // named chunk pulls the chunk into the entry graph, so it loaded eagerly again
  // and the dynamic import bought nothing. Verified by inspecting the script
  // tags rolldown emits into dist/index.html — check there, not the chunk list,
  // when reasoning about what actually loads on first paint.
})
