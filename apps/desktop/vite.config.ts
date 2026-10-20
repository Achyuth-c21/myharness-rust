import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

// Renderer for the myharness desktop shell (apps/desktop/src-tauri/tauri.conf.json points here).
export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  server: { port: 5173, strictPort: true },
  build: { outDir: "dist" },
});
