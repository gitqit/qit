import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'
import tailwindcss from '@tailwindcss/vite'

/** Proxy API + Git to `qit serve`; set QIT_DEV_MOUNT to the path segment from serve output (e.g. /temp-git). */
const qitDevApi = process.env.QIT_DEV_API ?? 'http://127.0.0.1:8080'
const qitDevMount = process.env.QIT_DEV_MOUNT ?? ''

export default defineConfig(({ command }) => ({
  base: './',
  plugins: [
    react(),
    tailwindcss(),
    {
      name: 'qit-dev-proxy-meta',
      configureServer(server) {
        if (!qitDevMount) {
          server.config.logger.warn(
            'QIT_DEV_MOUNT is unset. Set it to the repo path from `qit serve` (e.g. QIT_DEV_MOUNT=/temp-git) so /api and Git requests proxy correctly.',
          )
        }
      },
      transformIndexHtml: {
        order: 'pre',
        handler(html) {
          const base = command === 'serve' && qitDevMount ? qitDevMount : ''
          return html.replace(/%QIT_REPO_BASE%/g, base)
        },
      },
    },
  ],
  server:
    command === 'serve' && qitDevMount
      ? {
          proxy: {
            [qitDevMount]: {
              target: qitDevApi,
              changeOrigin: true,
            },
          },
        }
      : undefined,
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
}))
