import { useEffect, useMemo, useRef, useState } from 'react'
import { ChevronLeft, ChevronRight } from 'lucide-react'
import { releasesUrl, repoUrl } from '../../lib/content'
import { docsBySection, getAdjacentDocs, type DocEntry } from '../../lib/docs'
import { mdxComponents } from '../../lib/mdxComponents'
import { classNames } from '../../lib/classNames'
import { SiteLink } from '../../lib/siteLinks'
import { ButtonLink, SectionEyebrow, Surface } from '../atoms/primitives'
import { LandingShell } from '../templates/LandingShell'

const docsNavItems = [
  { href: '/', label: 'Home' },
  { href: '/docs/install', label: 'Docs' },
  { href: repoUrl, label: 'GitHub' },
] as const

type TocHeading = {
  id: string
  level: 2 | 3 | 4
  text: string
}

type TocNode = TocHeading & {
  children: TocNode[]
}

function slugifyHeading(text: string) {
  return text
    .toLowerCase()
    .trim()
    .replace(/['"]/g, '')
    .replace(/[^a-z0-9]+/g, '-')
    .replace(/^-+|-+$/g, '')
}

function buildTocTree(headings: TocHeading[]) {
  const roots: TocNode[] = []
  const stack: TocNode[] = []

  for (const heading of headings) {
    const node: TocNode = {
      ...heading,
      children: [],
    }

    while (stack.length > 0 && stack[stack.length - 1].level >= node.level) {
      stack.pop()
    }

    if (stack.length === 0) {
      roots.push(node)
    } else {
      stack[stack.length - 1].children.push(node)
    }

    stack.push(node)
  }

  return roots
}

export function DocsPage({
  doc,
}: {
  doc: DocEntry
}) {
  const { next, previous } = getAdjacentDocs(doc.slug)
  const DocContent = doc.Component
  const articleRef = useRef<HTMLElement | null>(null)
  const [tocHeadings, setTocHeadings] = useState<TocHeading[]>([])
  const [activeHeadingId, setActiveHeadingId] = useState<string | null>(null)
  const tocTree = useMemo(() => buildTocTree(tocHeadings), [tocHeadings])

  useEffect(() => {
    const article = articleRef.current
    if (!article) {
      return
    }

    const headingElements = Array.from(article.querySelectorAll<HTMLHeadingElement>('h2, h3, h4'))
    const headingCounts = new Map<string, number>()
    const headings = headingElements.map((heading, index) => {
      const text = heading.textContent?.trim() ?? ''
      const level = Number(heading.tagName.slice(1)) as TocHeading['level']
      const fallbackId = `section-${index + 1}`
      const baseId = slugifyHeading(text) || fallbackId
      const duplicateCount = headingCounts.get(baseId) ?? 0
      const id = duplicateCount === 0 ? baseId : `${baseId}-${duplicateCount + 1}`

      headingCounts.set(baseId, duplicateCount + 1)
      heading.id = id

      return {
        element: heading,
        id,
        level,
        text: text || `Section ${index + 1}`,
      }
    })

    setTocHeadings(headings.map((heading) => ({
      id: heading.id,
      level: heading.level,
      text: heading.text,
    })))

    if (headings.length === 0) {
      setActiveHeadingId(null)
      return
    }

    const syncActiveHeading = () => {
      const scrollThreshold = 112
      let nextActiveId = headings[0]?.id ?? null

      for (const heading of headings) {
        if (heading.element.getBoundingClientRect().top <= scrollThreshold) {
          nextActiveId = heading.id
        } else {
          break
        }
      }

      setActiveHeadingId(nextActiveId)
    }

    let frameId = 0
    const scheduleSync = () => {
      if (frameId !== 0) {
        return
      }

      frameId = window.requestAnimationFrame(() => {
        frameId = 0
        syncActiveHeading()
      })
    }

    syncActiveHeading()

    const handleWindowScroll = () => {
      scheduleSync()
    }

    window.addEventListener('scroll', handleWindowScroll, { passive: true })
    window.addEventListener('resize', scheduleSync)

    return () => {
      if (frameId !== 0) {
        window.cancelAnimationFrame(frameId)
      }

      window.removeEventListener('scroll', handleWindowScroll)
      window.removeEventListener('resize', scheduleSync)
    }
  }, [doc.slug])

  return (
    <LandingShell
      brandHref="/"
      className="pb-0"
      ctaHref={releasesUrl}
      contentSpacingClass="space-y-0"
      headerClassName="mb-5"
      navItems={docsNavItems}
    >
      <section className="grid gap-5 md:grid-cols-[220px_minmax(0,1fr)] md:items-start xl:grid-cols-[240px_minmax(0,1fr)_230px]">
        <aside className="md:sticky md:top-24 md:max-h-[calc(100svh-7rem)] md:self-start">
          <Surface className="panel-surface-soft max-h-full space-y-3 overflow-y-auto p-4">
            <div className="space-y-1.5">
              <SectionEyebrow>Docs</SectionEyebrow>
              <h1 className="text-xl font-black tracking-tight text-ink">Qit documentation</h1>
            </div>

            <nav aria-label="Documentation" className="space-y-3">
              {docsBySection.map((group) => (
                <div className="space-y-1" key={group.section}>
                  <p className="px-1 text-[11px] font-semibold uppercase tracking-[0.18em] text-ink-subtle">
                    {group.section}
                  </p>
                  <div className="grid gap-0.5">
                    {group.docs.map((item) => (
                      <SiteLink
                        className={classNames(
                          'relative rounded-xs px-1 py-1 pl-3 text-sm font-medium leading-5 transition-colors before:absolute before:bottom-1 before:left-0 before:top-1 before:w-px before:rounded-full before:transition-colors',
                          item.slug === doc.slug
                            ? 'text-accent-strong before:bg-accent'
                            : 'text-ink-muted before:bg-slate-300/70 hover:text-ink hover:before:bg-slate-400',
                        )}
                        href={item.href}
                        key={item.slug}
                      >
                        {item.sidebarLabel}
                      </SiteLink>
                    ))}
                  </div>
                </div>
              ))}
            </nav>
          </Surface>
        </aside>

        <div className="space-y-4 md:pr-2">
          <Surface className="panel-surface-soft docs-article-shell p-0">
            <header className="border-b border-slate-900/8 px-5 py-5 sm:px-6">
              <div className="space-y-2">
                <SectionEyebrow>{doc.sidebarLabel}</SectionEyebrow>
                <div className="space-y-2">
                  <h2 className="text-3xl font-black tracking-tight text-ink sm:text-4xl">{doc.title}</h2>
                  <p className="max-w-3xl text-base leading-7 text-ink-muted sm:text-lg">{doc.description}</p>
                </div>
              </div>
            </header>

            <article className="docs-prose px-5 py-6 sm:px-6" ref={articleRef}>
              <DocContent components={mdxComponents} />
            </article>
          </Surface>

          <div className="grid gap-3 md:grid-cols-2">
            {previous ? (
              <DocPagerCard direction="previous" doc={previous} />
            ) : (
              <div aria-hidden="true" className="hidden md:block" />
            )}
            {next ? <DocPagerCard direction="next" doc={next} /> : null}
          </div>
        </div>

        {tocTree.length > 0 ? (
          <aside className="hidden xl:block xl:self-start">
            <div className="docs-toc-rail">
              <DocsTableOfContents activeHeadingId={activeHeadingId} items={tocTree} />
            </div>
          </aside>
        ) : null}
      </section>
    </LandingShell>
  )
}

function DocsTableOfContents({
  activeHeadingId,
  items,
}: {
  activeHeadingId: string | null
  items: TocNode[]
}) {
  return (
    <Surface className="panel-surface-soft max-h-full space-y-3 overflow-y-auto p-4">
      <div className="space-y-1.5">
        <div className="space-y-1">
          <p className="text-xs font-semibold uppercase tracking-[0.18em] text-ink-subtle">On this page</p>
        </div>

        <nav aria-label="On this page" className="docs-toc">
          <ol className="docs-toc__list">
            {items.map((item) => (
              <DocsTocItem activeHeadingId={activeHeadingId} item={item} key={item.id} />
            ))}
          </ol>
        </nav>
      </div>
    </Surface>
  )
}

function DocsTocItem({
  activeHeadingId,
  item,
}: {
  activeHeadingId: string | null
  item: TocNode
}) {
  const active = item.id === activeHeadingId

  return (
    <li className="docs-toc__item">
      <a
        aria-current={active ? 'location' : undefined}
        className={classNames('docs-toc__link', active ? 'docs-toc__link--active' : undefined)}
        href={`#${item.id}`}
      >
        {item.text}
      </a>

      {item.children.length > 0 ? (
        <ol className="docs-toc__list docs-toc__list--nested">
          {item.children.map((child) => (
            <DocsTocItem activeHeadingId={activeHeadingId} item={child} key={child.id} />
          ))}
        </ol>
      ) : null}
    </li>
  )
}

function DocPagerCard({
  direction,
  doc,
}: {
  direction: 'next' | 'previous'
  doc: DocEntry
}) {
  const icon = direction === 'previous'
    ? <ChevronLeft aria-hidden="true" className="h-4 w-4" strokeWidth={2} />
    : <ChevronRight aria-hidden="true" className="h-4 w-4" strokeWidth={2} />

  return (
    <Surface className="panel-surface-soft h-full p-4">
      <div className="space-y-2">
        <p className="text-xs font-semibold uppercase tracking-[0.16em] text-ink-subtle">
          {direction === 'previous' ? 'Previous' : 'Next'}
        </p>
        <SiteLink className="group inline-flex items-start gap-2 text-left" href={doc.href}>
          {direction === 'previous' ? icon : null}
          <span className="space-y-1">
            <span className="block text-base font-bold tracking-tight text-ink group-hover:text-accent-strong">{doc.title}</span>
            <span className="block text-sm leading-6 text-ink-muted">{doc.description}</span>
          </span>
          {direction === 'next' ? icon : null}
        </SiteLink>
      </div>
    </Surface>
  )
}

export function DocsEmptyState() {
  return (
    <LandingShell
      brandHref="/"
      ctaHref={releasesUrl}
      contentSpacingClass="space-y-0"
      headerClassName="mb-5"
      navItems={docsNavItems}
    >
      <section className="mx-auto max-w-3xl py-12">
        <Surface className="panel-surface-soft space-y-4 text-center">
          <SectionEyebrow>Docs</SectionEyebrow>
          <div className="space-y-2">
            <h1 className="text-3xl font-black tracking-tight text-ink sm:text-4xl">Documentation is not available yet.</h1>
            <p className="text-base leading-8 text-ink-muted">
              Add MDX files to `landing/src/docs` so the documentation section has pages to render.
            </p>
          </div>
          <div className="flex justify-center">
            <ButtonLink href="/" tone="secondary">
              Back to landing page
            </ButtonLink>
          </div>
        </Surface>
      </section>
    </LandingShell>
  )
}
