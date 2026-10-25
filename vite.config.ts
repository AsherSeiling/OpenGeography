import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

// Tauri expects a fixed dev port; the frontend lives in ui/.
export default defineConfig({
  root: "ui",
  plugins: [react()],
  clearScreen: false,
  worker: { format: "es" },
  server: { port: Number(process.env.GIS_VITE_PORT ?? 1420), strictPort: true, watch: { ignored: ["**/src-tauri/**", "**/target/**"] } },
  build: { outDir: "../dist", emptyOutDir: true, target: "es2022", chunkSizeWarningLimit: 4000 },
  test: { root: ".", include: ["ui/src/**/*.test.ts"] },
});
