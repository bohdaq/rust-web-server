import { defineConfig } from "vite";

// One script and one stylesheet with fixed names, written straight into
// src/admin/ui/ (embedded by src/admin/ui.rs). Fonts are inlined into the
// stylesheet, so there is nothing else to serve.
export default defineConfig({
  publicDir: false,
  base: "/admin/",
  build: {
    outDir: "../src/admin/ui",
    emptyOutDir: true,
    assetsInlineLimit: 200_000,
    cssCodeSplit: false,
    modulePreload: false,
    rollupOptions: {
      output: {
        entryFileNames: "admin.js",
        chunkFileNames: "admin-[name].js",
        assetFileNames: (info) => (info.names?.[0]?.endsWith(".css") ? "admin.css" : "[name][extname]"),
      },
    },
  },
});
