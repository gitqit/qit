import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'
import tailwindcss from '@tailwindcss/vite'

export default defineConfig({
  base: './',
  plugins: [react(), tailwindcss()],
  build: {
    outDir: 'dist',
    emptyOutDir: true,
    rollupOptions: {
      output: {
        entryFileNames: 'assets/app.js',
        chunkFileNames: 'assets/chunk-[name].js',
        manualChunks: (id) => {
          if (!id.includes('node_modules')) {
            return
          }

          if (id.includes('monaco-editor') || id.includes('@monaco-editor/react')) {
            return 'vendor-monaco'
          }

          if (id.includes('react-arborist')) {
            return 'vendor-tree'
          }

          if (
            id.includes('react-markdown') ||
            id.includes('remark-gfm') ||
            id.includes('remark-') ||
            id.includes('micromark') ||
            id.includes('mdast') ||
            id.includes('unist') ||
            id.includes('hast')
          ) {
            return 'vendor-markdown'
          }

          if (id.includes('lucide-react') || id.includes('@headlessui/react')) {
            return 'vendor-ui'
          }

          return 'vendor'
        },
        assetFileNames: (assetInfo) => {
          if (assetInfo.names.includes('style.css')) {
            return 'assets/app.css'
          }
          return 'assets/[name][extname]'
        },
      },
    },
  },
})
