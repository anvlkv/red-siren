import { defineConfig } from 'vite'
import path from 'path'

// https://vitejs.dev/config/
export default defineConfig({
  base: '/pkg/worklet/dist',
  build: {
    lib: {
      entry: [path.resolve(__dirname, 'src/lib.ts'), path.resolve(__dirname, 'src/worklet.ts')],
      fileName: (format, entry) => `${entry}.${format}.js`,
    },
  },
  optimizeDeps: {
    include: ['shared_types/types/au_types', 'shared_types/bincode/mod',  'shared'],
  },
  server: {
    fs: {
      allow: ['..'],
    },
  },
})