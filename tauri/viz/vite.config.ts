import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';

// Standalone Tauri app: fixed port so tauri.conf.json devUrl matches.
export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  server: {
    port: 5180,
    strictPort: true,
  },
});
