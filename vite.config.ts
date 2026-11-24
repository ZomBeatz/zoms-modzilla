/// <reference types="vitest/config" />
import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import fs from "node:fs";

export default defineConfig({
  plugins: [react()],
  // The online demo lives under the repository's name on GitHub Pages.
  base: process.env.NEXUS_BASE ?? "/",
  // Shown in Settings > About.
  define: { __APP_VERSION__: JSON.stringify(JSON.parse(fs.readFileSync("package.json", "utf8")).version) },
  clearScreen: false,
  server: { port: 1440, strictPort: true },
  test: { environment: "node", include: ["src/**/*.test.ts"] },
});
