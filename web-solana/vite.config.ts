import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'
import path from 'path'

// https://vite.dev/config/
export default defineConfig({
  plugins: [react()],
  base: './',
  resolve: {
    alias: {
      '/wasm/xfchess_wasm.js': path.resolve(__dirname, '../xfchess-wasm/pkg/xfchess_wasm.js'),
    },
  },
  server: {
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
})
