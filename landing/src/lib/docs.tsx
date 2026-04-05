import type { ComponentType } from 'react'
import type { MDXComponents } from 'mdx/types'

type DocComponentProps = {
  components?: MDXComponents
}

export type DocFrontmatter = {
  title: string
  description: string
  slug: string
  sidebar_label?: string
  sidebar_position?: number
}

export type DocEntry = DocFrontmatter & {
  id: string
  href: string
  sidebarLabel: string
  Component: ComponentType<DocComponentProps>
}

type DocModule = {
  default: ComponentType<DocComponentProps>
  frontmatter?: Record<string, unknown>
}

const docModules = import.meta.glob<DocModule>('../docs/**/*.mdx', { eager: true })

function normalizeSlug(value: string) {
  return value.replace(/^\/+|\/+$/g, '')
}

function readStringField(data: Record<string, unknown>, key: keyof DocFrontmatter) {
  const value = data[key]
  if (typeof value !== 'string' || value.trim().length === 0) {
    throw new Error(`Expected docs frontmatter field "${key}" to be a non-empty string.`)
  }

  return value.trim()
}

function readOptionalNumberField(data: Record<string, unknown>, key: keyof DocFrontmatter) {
  const value = data[key]
  if (value === undefined) {
    return undefined
  }

  if (typeof value !== 'number' || Number.isNaN(value)) {
    throw new Error(`Expected docs frontmatter field "${key}" to be a number when present.`)
  }

  return value
}

function parseDoc(path: string, module: DocModule): DocEntry {
  const frontmatter = module.frontmatter
  if (!frontmatter) {
    throw new Error(`Missing exported frontmatter for "${path}".`)
  }

  const slug = normalizeSlug(readStringField(frontmatter, 'slug'))

  if (!slug) {
    throw new Error(`Doc "${path}" must define a non-root slug.`)
  }

  const title = readStringField(frontmatter, 'title')
  const description = readStringField(frontmatter, 'description')
  const sidebarLabel = typeof frontmatter.sidebar_label === 'string' && frontmatter.sidebar_label.trim().length > 0
    ? frontmatter.sidebar_label.trim()
    : title

  return {
    id: path,
    title,
    description,
    slug,
    sidebar_label: typeof frontmatter.sidebar_label === 'string' ? frontmatter.sidebar_label : undefined,
    sidebar_position: readOptionalNumberField(frontmatter, 'sidebar_position'),
    sidebarLabel,
    href: `/docs/${slug}`,
    Component: module.default,
  }
}

export const docs = Object.entries(docModules)
  .map(([path, module]) => parseDoc(path, module))
  .sort((left, right) => {
    const leftPosition = left.sidebar_position ?? Number.MAX_SAFE_INTEGER
    const rightPosition = right.sidebar_position ?? Number.MAX_SAFE_INTEGER

    if (leftPosition !== rightPosition) {
      return leftPosition - rightPosition
    }

    return left.title.localeCompare(right.title)
  })

const seenSlugs = new Set<string>()
for (const doc of docs) {
  if (seenSlugs.has(doc.slug)) {
    throw new Error(`Duplicate docs slug "${doc.slug}" detected.`)
  }

  seenSlugs.add(doc.slug)
}

export const defaultDoc = docs[0] ?? null

export function getDocBySlug(slug: string) {
  const normalizedSlug = normalizeSlug(slug)
  return docs.find((doc) => doc.slug === normalizedSlug) ?? null
}

export function getAdjacentDocs(slug: string) {
  const index = docs.findIndex((doc) => doc.slug === normalizeSlug(slug))
  if (index === -1) {
    return { previous: null, next: null }
  }

  return {
    previous: docs[index - 1] ?? null,
    next: docs[index + 1] ?? null,
  }
}
