import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'

// https://vitejs.dev/config/
//
// Build-only config: the admin UI is compiled to dist/ and served by the
// Tauri wallet bridge (loopback-only, :7454) inside the desktop
// "tournament-admin" window. There is no standalone dev/web server —
// rebuild with `npm run build` (or `just build-admin-ui-force`) to see changes.
export default defineConfig({
  plugins: [react()],
  base: '/tournament-admin/',
})
