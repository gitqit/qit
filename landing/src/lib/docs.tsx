import type { ComponentType } from 'react'
import type { MDXComponents } from 'mdx/types'

type DocComponentProps = {
  components?: MDXComponents
}

export type DocFrontmatter = {
  title: string
  description: string
  slug: string
  section?: string
  sidebar_label?: string
  sidebar_position?: number
  sidebar_hidden?: boolean
}

export type DocEntry = DocFrontmatter & {
  id: string
  href: string
  section: string
  sidebarLabel: string
  sidebarHidden: boolean
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

function readOptionalStringField(data: Record<string, unknown>, key: keyof DocFrontmatter) {
  const value = data[key]
  if (value === undefined) {
    return undefined
  }

  if (typeof value !== 'string' || value.trim().length === 0) {
    throw new Error(`Expected docs frontmatter field "${key}" to be a non-empty string when present.`)
  }

  return value.trim()
}

function readOptionalBooleanField(data: Record<string, unknown>, key: keyof DocFrontmatter) {
  const value = data[key]
  if (value === undefined) {
    return false
  }

  if (typeof value !== 'boolean') {
    throw new Error(`Expected docs frontmatter field "${key}" to be a boolean when present.`)
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
  const section = readOptionalStringField(frontmatter, 'section') ?? 'Other'
  const sidebarLabel = typeof frontmatter.sidebar_label === 'string' && frontmatter.sidebar_label.trim().length > 0
    ? frontmatter.sidebar_label.trim()
    : title

  return {
    id: path,
    title,
    description,
    slug,
    section,
    sidebar_label: typeof frontmatter.sidebar_label === 'string' ? frontmatter.sidebar_label : undefined,
    sidebar_position: readOptionalNumberField(frontmatter, 'sidebar_position'),
    sidebarLabel,
    sidebar_hidden: readOptionalBooleanField(frontmatter, 'sidebar_hidden'),
    sidebarHidden: readOptionalBooleanField(frontmatter, 'sidebar_hidden'),
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

export const visibleDocs = docs.filter((doc) => !doc.sidebarHidden)

export const docsBySection = visibleDocs.reduce<Array<{ section: string, docs: DocEntry[] }>>((groups, doc) => {
  const existing = groups.find((group) => group.section === doc.section)
  if (existing) {
    existing.docs.push(doc)
    return groups
  }

  groups.push({
    section: doc.section,
    docs: [doc],
  })
  return groups
}, [])

export const defaultDoc = visibleDocs[0] ?? docs[0] ?? null

export function getDocBySlug(slug: string) {
  const normalizedSlug = normalizeSlug(slug)
  return docs.find((doc) => doc.slug === normalizedSlug) ?? null
}

export function getAdjacentDocs(slug: string) {
  const index = visibleDocs.findIndex((doc) => doc.slug === normalizeSlug(slug))
  if (index === -1) {
    return { previous: null, next: null }
  }

  return {
    previous: visibleDocs[index - 1] ?? null,
    next: visibleDocs[index + 1] ?? null,
  }
}
