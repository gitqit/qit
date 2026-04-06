import { copyFile } from 'node:fs/promises'
import { resolve } from 'node:path'
import mdx from '@mdx-js/rollup'
import rehypePrettyCode from 'rehype-pretty-code'
import { defineConfig, type PluginOption } from 'vite'
import react from '@vitejs/plugin-react'
import remarkFrontmatter from 'remark-frontmatter'
import remarkMdxFrontmatter from 'remark-mdx-frontmatter'
import tailwindcss from '@tailwindcss/vite'
import remarkGfm from 'remark-gfm'

const prettyCodeOptions = {
  theme: 'catppuccin-mocha',
  keepBackground: false,
  defaultLang: {
    block: 'text',
    inline: 'text',
  },
}

function githubPagesSpaFallback(): PluginOption {
  return {
    name: 'github-pages-spa-fallback',
    apply: 'build',
    async closeBundle() {
      const distDir = resolve(__dirname, 'dist')
      await copyFile(resolve(distDir, 'index.html'), resolve(distDir, '404.html'))
    },
  }
}

export default defineConfig({
  base: process.env.VITE_BASE_PATH ?? '/',
  plugins: [
    mdx({
      remarkPlugins: [remarkGfm, remarkFrontmatter, [remarkMdxFrontmatter, { name: 'frontmatter' }]],
      rehypePlugins: [[rehypePrettyCode, prettyCodeOptions]],
    }) as PluginOption,
    react(),
    tailwindcss(),
    githubPagesSpaFallback(),
  ],
})
