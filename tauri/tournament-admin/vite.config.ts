import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'

// https://vitejs.dev/config/
//
// Port 7455 (vite dev) with base `/tournament-admin/`. The wallet bridge
// running in the Tauri process owns :7454 and proxies /tournament-admin/ to
// this server in dev. The Tauri window still loads from :7454/tournament-admin/
// so tauri.conf.json does not need changing.
export default defineConfig({
  plugins: [react()],
  base: '/tournament-admin/',
  server: {
    host: true,
    port: 7455,
    strictPort: true,
  },
})
