import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

// Settings recommended for Tauri: fixed port, and don't watch the Rust folders.
export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    watch: { ignored: ["**/src-tauri/**", "**/crates/**", "**/target/**"] },
  },
  build: {
    target: "es2022",
    chunkSizeWarningLimit: 1500,
  },
});
