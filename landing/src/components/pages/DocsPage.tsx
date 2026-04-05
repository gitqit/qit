import { ChevronLeft, ChevronRight } from 'lucide-react'
import { releasesUrl, repoUrl } from '../../lib/content'
import { docs, getAdjacentDocs, type DocEntry } from '../../lib/docs'
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

export function DocsPage({
  doc,
}: {
  doc: DocEntry
}) {
  const { next, previous } = getAdjacentDocs(doc.slug)
  const DocContent = doc.Component

  return (
    <LandingShell
      brandHref="/"
      ctaHref={releasesUrl}
      contentSpacingClass="space-y-0"
      headerClassName="mb-5"
      navItems={docsNavItems}
    >
      <section className="grid gap-8 md:grid-cols-[240px_minmax(0,1fr)] md:items-start xl:grid-cols-[260px_minmax(0,1fr)]">
        <aside className="md:sticky md:top-28">
          <Surface className="panel-surface-soft space-y-5 p-5">
            <div className="space-y-3">
              <SectionEyebrow>Docs</SectionEyebrow>
              <div className="space-y-2">
                <h1 className="text-2xl font-black tracking-tight text-ink">Quick-start documentation</h1>
                <p className="text-sm leading-7 text-ink-muted">
                  Lean, task-oriented guidance for getting Qit installed, shared, and understood.
                </p>
              </div>
            </div>

            <nav aria-label="Documentation" className="grid gap-2">
              {docs.map((item) => (
                <SiteLink
                  className={classNames(
                    'rounded-token border px-3 py-2.5 text-sm font-medium transition',
                    item.slug === doc.slug
                      ? 'border-accent/20 bg-accent/8 text-accent-strong shadow-sm'
                      : 'border-transparent text-ink-muted hover:border-slate-900/8 hover:bg-white/65 hover:text-ink',
                  )}
                  href={item.href}
                  key={item.slug}
                >
                  {item.sidebarLabel}
                </SiteLink>
              ))}
            </nav>
          </Surface>
        </aside>

        <div className="space-y-6">
          <Surface className="panel-surface-soft docs-article-shell p-0">
            <header className="border-b border-slate-900/8 px-6 py-6 sm:px-8">
              <div className="space-y-3">
                <SectionEyebrow>{doc.sidebarLabel}</SectionEyebrow>
                <div className="space-y-3">
                  <h2 className="text-4xl font-black tracking-tight text-ink sm:text-5xl">{doc.title}</h2>
                  <p className="max-w-3xl text-base leading-8 text-ink-muted sm:text-lg">{doc.description}</p>
                </div>
              </div>
            </header>

            <article className="docs-prose px-6 py-8 sm:px-8">
              <DocContent components={mdxComponents} />
            </article>
          </Surface>

          <div className="grid gap-4 md:grid-cols-2">
            {previous ? (
              <DocPagerCard direction="previous" doc={previous} />
            ) : (
              <div aria-hidden="true" className="hidden md:block" />
            )}
            {next ? <DocPagerCard direction="next" doc={next} /> : null}
          </div>
        </div>
      </section>
    </LandingShell>
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
    <Surface className="panel-surface-soft h-full p-5">
      <div className="space-y-3">
        <p className="text-xs font-semibold uppercase tracking-[0.16em] text-ink-subtle">
          {direction === 'previous' ? 'Previous' : 'Next'}
        </p>
        <SiteLink className="group inline-flex items-start gap-2 text-left" href={doc.href}>
          {direction === 'previous' ? icon : null}
          <span className="space-y-1">
            <span className="block text-lg font-bold tracking-tight text-ink group-hover:text-accent-strong">{doc.title}</span>
            <span className="block text-sm leading-7 text-ink-muted">{doc.description}</span>
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
      <section className="mx-auto max-w-3xl py-16">
        <Surface className="panel-surface-soft space-y-5 text-center">
          <SectionEyebrow>Docs</SectionEyebrow>
          <div className="space-y-3">
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
