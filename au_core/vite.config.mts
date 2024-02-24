import { defineConfig } from "vite";
import { viteStaticCopy } from 'vite-plugin-static-copy'
import path from "path";

export default defineConfig({
  plugins: [
    viteStaticCopy({
      targets: [
        {
          src: 'pkg/au_core_bg.wasm',
          dest: 'wasm'
        }
      ]
    })
  ],
  build: {
    lib: {
      entry: path.resolve(__dirname, "src/worklet.ts"),
      name: "au-core",
      formats: ["es"]
    },
    outDir: path.resolve(__dirname, "../web-leptos/public/worklet"),
    emptyOutDir: true,
  },
});
