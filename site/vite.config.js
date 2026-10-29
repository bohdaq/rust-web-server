import { defineConfig } from "vite";

// Vite builds only the browser bundle (one entry, hashed); scripts/prerender.js
// then writes every page's HTML around it, using the manifest for file names.
export default defineConfig({
  publicDir: false,
  build: {
    outDir: "dist",
    emptyOutDir: true,
    manifest: true,
    assetsDir: "assets",
    rollupOptions: { input: "src/client/main.js" },
  },
});
