import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'

// The admin window only navigates within /tournament-admin/ (see
// src/windows/tournament_admin.rs), so the HMR dev server serves there too:
// XFCHESS_ADMIN_DEV_URL=http://localhost:5176 loads http://localhost:5176/tournament-admin/.
export default defineConfig({
  plugins: [react()],
  base: '/tournament-admin/',
  server: {
    port: 5176,
    strictPort: true,
  },
})
