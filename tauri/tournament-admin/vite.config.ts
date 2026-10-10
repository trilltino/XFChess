import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'

// Built assets use /tournament-admin/ under the bridge. The HMR dev server
// uses root; base must follow the selected command.
export default defineConfig(({ command }) => ({
  plugins: [react()],
  base: command === 'serve' ? '/' : '/tournament-admin/',
  server: {
    port: 5176,
    strictPort: true,
  },
}))
