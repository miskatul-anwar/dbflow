import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";
import tailwindcss from "@tailwindcss/vite";
import path from "path"; // <-- Import path module
import { rmSync } from "fs";

const host = process.env.TAURI_DEV_HOST;

// https://vite.dev/config/
/**
 * transformers.js pulls in onnxruntime-web, whose bundle makes Vite emit a 27 MB
 * WebAssembly file. The embedding worker loads its own copy from /ort (set via
 * `wasmPaths`), so the emitted one is never used; drop it from the installer.
 */
const dropUnusedOrtWasm = {
  name: 'drop-unused-ort-wasm',
  generateBundle(_options: unknown, bundle: Record<string, unknown>) {
    for (const name of Object.keys(bundle)) {
      if (/ort-wasm-.*\.wasm$/.test(name)) delete bundle[name];
    }
  },
};

/**
 * The phone app does not run the relation-naming model (see modelNamingAvailable): leave its ~390 MB out of the
 * Android and iOS packages. Tauri sets TAURI_ENV_PLATFORM while it runs the build for a platform.
 */
const mobileBuild = /^(android|ios)$/.test(process.env.TAURI_ENV_PLATFORM ?? '');
let outDir = path.resolve(__dirname, 'dist');
const dropNamingModelOnMobile = {
  name: 'drop-naming-model-on-mobile',
  apply: 'build' as const,
  configResolved(config: { root: string; build: { outDir: string } }) {
    outDir = path.resolve(config.root, config.build.outDir);
  },
  closeBundle() {
    if (!mobileBuild) return;
    const dir = path.join(outDir, 'models/HuggingFaceTB');
    rmSync(dir, { recursive: true, force: true });
    console.log(`[mobile] left the naming model out of the package (${dir})`);
  },
};

export default defineConfig(async () => ({
  plugins: [react(), tailwindcss(), dropUnusedOrtWasm, dropNamingModelOnMobile],
  resolve: {
    alias: {
      '@': path.resolve(__dirname, 'src'), // <-- This enables @/ alias
    },
  },
  // Workers are created with { type: 'module' }; build them as ES modules so they can
  // lazy-load code (the spreadsheet parser) instead of bundling it into every start.
  worker: { format: 'es' as const },
  build: {
    // What remains above 500 kB is loaded on demand from the installed app's own files (no network):
    // Graphviz WebAssembly, Mermaid diagram engines, the spreadsheet parser and the embedding worker.
    chunkSizeWarningLimit: 900,
    rollupOptions: {
      output: {
        // Stable vendor chunks keep the start-up bundle small and let the cache survive app-only changes.
        manualChunks(id: string) {
          if (!id.includes('node_modules')) return undefined;
          if (id.includes('@xyflow') || id.includes('d3-')) return 'flow';
          if (/node_modules\/(react|react-dom|react-router|react-router-dom|scheduler)\//.test(id)) return 'react';
          return undefined;
        },
      },
    },
  },
  // Vite options tailored for Tauri development and only applied in `tauri dev` or `tauri build`
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    host: host || false,
    hmr: host
      ? {
          protocol: "ws",
          host,
          port: 1421,
        }
      : undefined,
    watch: {
      ignored: ["**/src-tauri/**"],
    },
  },
}));