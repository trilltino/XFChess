import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'

// https://vitejs.dev/config/
//
// Two modes:
//
//   npm run build  → compiles to dist/, served by the Tauri wallet bridge
//                    (loopback-only, :7454) inside the "tournament-admin"
//                    window. This is what `just admin` uses.
//
//   npm run dev    → HMR dev server on :5176. Point the desktop window at it
//                    with `just admin-dev` and UI edits apply instantly, with
//                    no rebuild and no app restart.
//
// `base` must stay '/tournament-admin/' for the built output (the bridge
// serves it under that path), but the dev server serves from root — hence the
// command-conditional base below. Getting this wrong makes the dev server
// return 404s for its own assets.
export default defineConfig(({ command }) => ({
  plugins: [react()],
  base: command === 'serve' ? '/' : '/tournament-admin/',
  server: {
    port: 5176,
    strictPort: true,
  },
}))
