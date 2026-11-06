import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  server: { port: 5181, strictPort: true },
  build: { target: "es2021", chunkSizeWarningLimit: 2000 },
});
