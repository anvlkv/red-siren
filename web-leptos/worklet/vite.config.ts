import { defineConfig } from 'vite'
import path from 'path'

// https://vitejs.dev/config/
export default defineConfig({
  base: '/worklet',
  build: {
    lib: {
      entry: [path.resolve(__dirname, 'src/lib.ts'), path.resolve(__dirname, 'src/worklet.ts')],
      fileName: (format, entry) => `${entry}.${format}.js`,
    },
    outDir: '../public/worklet',
    emptyOutDir: true,
  },
  optimizeDeps: {
    include: ['typegen/types/au_types', 'typegen/bincode/mod',  'aucore'],
  },
  server: {
    fs: {
      allow: ['..'],
    },
  },
})