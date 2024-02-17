import { defineConfig } from "vite";
import path from "path";
import wasm from "vite-plugin-wasm";
import topLevelAwait from "vite-plugin-top-level-await";

export default defineConfig({
  plugins: [wasm(), topLevelAwait()],
  build: {
    lib: {
      entry: path.resolve(__dirname, "src/worklet.ts"),
      fileName: (format, entry) => `${entry}.${format}.js`,
      name: "au-core",
      formats: ["es"]
    },
    outDir: path.resolve(__dirname, "../web-leptos/public/worklet"),
    emptyOutDir: true,
  },
});
