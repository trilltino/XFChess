import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'

// https://vitejs.dev/config/
//
// Port 7454 and base path `/tournament-admin/` are required because the Tauri
// shell loads this UI from `http://localhost:7454/tournament-admin/` (see
// `tauri/tauri.conf.json` -> the `tournament-admin` window). Changing these
// here without updating `tauri.conf.json` will result in a blank Tauri window.
export default defineConfig({
  plugins: [react()],
  base: '/tournament-admin/',
  server: {
    host: true,
    port: 7454,
    strictPort: true,
  },
})
