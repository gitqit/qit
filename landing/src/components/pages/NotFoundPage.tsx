import { repoUrl } from '../../lib/content'
import { getPrimaryInstallCta } from '../../lib/install'
import { ButtonLink, SectionEyebrow, Surface } from '../atoms/primitives'
import { LandingShell } from '../templates/LandingShell'

const navItems = [
  { href: '/', label: 'Home' },
  { href: '/docs/install', label: 'Docs' },
  { href: repoUrl, label: 'GitHub' },
] as const

export function NotFoundPage({
  description = 'The page you requested does not exist in this static site build.',
  title = 'Page not found',
}: {
  title?: string
  description?: string
}) {
  const installCta = getPrimaryInstallCta()

  return (
    <LandingShell
      brandHref="/"
      ctaHref={installCta.href}
      ctaLabel={installCta.label}
      contentSpacingClass="space-y-0"
      headerClassName="mb-5"
      navItems={navItems}
    >
      <section className="mx-auto max-w-3xl py-16">
        <Surface className="panel-surface-soft space-y-5 text-center">
          <SectionEyebrow>404</SectionEyebrow>
          <div className="space-y-3">
            <h1 className="text-3xl font-black tracking-tight text-ink sm:text-4xl">{title}</h1>
            <p className="text-base leading-8 text-ink-muted">{description}</p>
          </div>
          <div className="flex flex-col justify-center gap-3 sm:flex-row">
            <ButtonLink href="/">Back to landing page</ButtonLink>
            <ButtonLink href="/docs/install" tone="secondary">
              Open docs
            </ButtonLink>
          </div>
        </Surface>
      </section>
    </LandingShell>
  )
}
