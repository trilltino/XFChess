import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'
import path from 'path'

// https://vite.dev/config/
export default defineConfig({
  plugins: [react()],
  // Use root-relative asset URLs; relative base paths break direct navigation
  // to nested routes.
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
  // Let the Privy dynamic import create its lazy chunk. A forced advancedChunks
  // group can pull the SDK into the entry graph; check emitted HTML script tags.
})
