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
    rollupOptions: {
      output: {
        manualChunks(id) {
          if (id.includes("node_modules/react") || id.includes("node_modules/react-dom")) {
            return "vendor";
          }
          if (id.includes("node_modules/@solana/web3.js")) {
            return "solana";
          }
        },
      },
    },
  },
  define: {
    "global": "globalThis",
  },
});
