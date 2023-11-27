import { defineConfig } from 'vite'
import path from 'path'
import wasm from "vite-plugin-wasm";
import topLevelAwait from "vite-plugin-top-level-await";

// https://vitejs.dev/config/
export default defineConfig({
  plugins: [
    wasm(),
    topLevelAwait()
  ],
  build: {
    lib: {
      entry: path.resolve(__dirname, 'src/lib.ts'),
      name: 'lib',
      fileName: (format) => `lib.${format}.js`
    }
  },
  optimizeDeps: {
    include: ['shared_types/types/shared_types', 'shared_types/bincode/mod',  'shared'],
  },
  server: {
    fs: {
      allow: ['..'],
    },
  },
})