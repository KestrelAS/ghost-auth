import { defineConfig } from "vite";
import { svelte } from "@sveltejs/vite-plugin-svelte";
import tailwindcss from "@tailwindcss/vite";
import { resolve } from "path";
import { copyFileSync, existsSync, mkdirSync, readdirSync } from "fs";

const browser = process.env.BROWSER ?? "chrome";

function copyExtensionAssets() {
  return {
    name: "copy-extension-assets",
    writeBundle() {
      const outDir = resolve(__dirname, `dist/${browser}`);

      // Copy manifest
      const manifestSrc = resolve(__dirname, `manifest.${browser}.json`);
      if (existsSync(manifestSrc)) {
        copyFileSync(manifestSrc, resolve(outDir, "manifest.json"));
      }

      // Copy icons
      const iconsDir = resolve(__dirname, "icons");
      const outIcons = resolve(outDir, "icons");
      if (existsSync(iconsDir)) {
        mkdirSync(outIcons, { recursive: true });
        for (const file of readdirSync(iconsDir)) {
          copyFileSync(resolve(iconsDir, file), resolve(outIcons, file));
        }
      }
    },
  };
}

export default defineConfig({
  plugins: [svelte(), tailwindcss(), copyExtensionAssets()],
  resolve: {
    alias: {
      $lib: resolve(__dirname, "src/lib"),
      $core: resolve(__dirname, "src/core"),
      $locales: resolve(__dirname, "../src/lib/i18n/locales"),
    },
  },
  base: "./",
  build: {
    outDir: `dist/${browser}`,
    emptyOutDir: true,
    rollupOptions: {
      input: {
        popup: resolve(__dirname, "src/popup/index.html"),
        "service-worker": resolve(__dirname, "src/background/service-worker.ts"),
      },
      output: {
        entryFileNames: "[name].js",
        chunkFileNames: "chunks/[name]-[hash].js",
        assetFileNames: "assets/[name]-[hash][extname]",
      },
    },
  },
  define: {
    __BROWSER__: JSON.stringify(browser),
  },
});
