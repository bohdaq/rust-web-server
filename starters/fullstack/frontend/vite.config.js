import { defineConfig } from "vite";

// `npm run dev` serves the frontend on :5173 and sends /api to the rws app
// (`cargo run`, :7878); `npm run build` writes dist/, which rws serves.
export default defineConfig({
  build: { outDir: "dist", emptyOutDir: true },
  server: { proxy: { "/api": "http://127.0.0.1:7878" } },
});
