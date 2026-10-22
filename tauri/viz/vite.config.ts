import { defineConfig } from 'vite';
import react from '@vitejs/plugin-react';

// Standalone Tauri app: fixed port so tauri.conf.json devUrl matches.
export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  build: {
    rolldownOptions: {
      output: {
        codeSplitting: {
          groups: [{ name: 'vendor', test: /node_modules/, maxSize: 350_000, entriesAware: true }],
        },
      },
    },
  },
  server: {
    port: 5180,
    strictPort: true,
  },
});
