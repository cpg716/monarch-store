import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import path from "path";
import os from "os";

const host = process.env.TAURI_DEV_HOST;
const isDev = process.env.NODE_ENV !== "production";

// In dev: fresh cache per process so each "npm run tauri dev" loads current code (no stale UI).
// In build: single project cache for speed.
const cacheDir = isDev
  ? path.join(os.tmpdir(), `monarch-vite-${process.pid}`)
  : ".vite-cache";

// https://vite.dev/config/
export default defineConfig(async () => ({
  plugins: [
    react({
      // Suppress "[BABEL] Note: The code generator has deoptimised the styling..." for large deps (e.g. react-dom_client.js)
      babel: { compact: false },
    }),
  ],
  cacheDir,

  // Vite options tailored for Tauri development and only applied in `tauri dev` or `tauri build`
  //
  // 1. prevent Vite from obscuring rust errors
  clearScreen: false,
  // 2. tauri expects a fixed port, fail if that port is not available
  server: {
    port: 1420,
    strictPort: true,
    host: host || false,
    // Force re-optimize deps on dev server start so we never serve stale pre-bundles
    force: isDev,
    hmr: host
      ? {
        protocol: "ws",
        host,
        port: 1421,
      }
      : undefined,
    watch: {
      // 3. tell Vite to ignore watching `src-tauri`
      ignored: ["**/src-tauri/**"],
    },
  },
  build: {
    chunkSizeWarningLimit: 1000, // Increase limit slightly for desktop apps
    ...(isDev ? {} : { esbuild: { drop: ['console', 'debugger'] } }),
    rollupOptions: {
      output: {
        manualChunks: {
          vendor: ['react', 'react-dom', 'zustand'],
          ui: ['framer-motion', 'lucide-react', 'clsx', 'tailwind-merge'],
        },
      },
    },
  },
}));
