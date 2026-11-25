import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

// https://vite.dev/config/
export default defineConfig({
  plugins: [react()],

  // Prevent vite from obscuring Rust errors
  clearScreen: false,

  server: {
    // pdao's dev server uses 5173; keep OpenStart on 5174 so both can run.
    port: 5174,
    strictPort: true,
    // Allow Tauri to connect from its own host
    host: process.env.TAURI_DEV_HOST || "localhost",
  },

  // Env variables starting with TAURI_ will be exposed to tauri's source code
  envPrefix: ["VITE_", "TAURI_"],
});
