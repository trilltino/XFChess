import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import { nodePolyfills } from "vite-plugin-node-polyfills";

export default defineConfig({
  plugins: [
    react(),
    nodePolyfills({
      include: ["buffer"],
      globals: {
        Buffer: true,
      },
    }),
  ],
  clearScreen: false,
  server: {
    port: 5174,
    strictPort: true,
  },
  optimizeDeps: {
    include: [
      "@solana/spl-token",
      "@solana/web3.js",
    ],
  },
  base: "./",
  build: {
    outDir: "dist",
    emptyOutDir: true,
    rolldownOptions: {
      output: {
        codeSplitting: {
          groups: [{ name: "vendor", test: /node_modules/, maxSize: 350_000, entriesAware: true }],
        },
      },
    },
  },
  define: {
    "global": "globalThis",
  },
});
