import { useState, type PropsWithChildren } from 'react'
import { Menu, X } from 'lucide-react'
import { classNames } from '../../lib/classNames'
import { isExternalHref } from '../../lib/hrefs'
import { SiteLink } from '../../lib/siteLinks'
import { BrandLogo, ButtonLink } from '../atoms/primitives'

const defaultNavItems = [
  { href: '#features', label: 'Features' },
  { href: '#preview', label: 'Preview' },
  { href: '#faq', label: 'FAQ' },
] as const

type ShellNavItem = {
  href: string
  label: string
}

export function LandingShell({
  children,
  className,
  contentSpacingClass = 'space-y-24',
  ctaLabel = 'Download binaries',
  ctaHref,
  brandHref = '#top',
  headerClassName,
  navItems = defaultNavItems,
}: PropsWithChildren<{
  className?: string
  contentSpacingClass?: string
  ctaHref: string
  ctaLabel?: string
  brandHref?: string
  headerClassName?: string
  navItems?: ReadonlyArray<ShellNavItem>
}>) {
  const [mobileMenuOpen, setMobileMenuOpen] = useState(false)
  const ctaIsExternal = isExternalHref(ctaHref)

  return (
    <div className="relative overflow-x-hidden">
      <a className="skip-link" href="#main-content">
        Skip to content
      </a>

        <div aria-hidden="true" className="pointer-events-none absolute inset-0">
          <div className="absolute -left-32 -top-24 h-72 w-72 rounded-full bg-accent/16 blur-3xl" />
          <div className="absolute -right-32 top-88 h-80 w-80 rounded-full bg-sky-300/20 blur-3xl" />
          <div className="absolute -bottom-48 left-1/2 h-96 w-96 -translate-x-1/2 rounded-full bg-lime-300/18 blur-3xl" />
      </div>

      <div className="relative mx-auto flex min-h-screen w-full max-w-7xl flex-col px-5 pb-12 pt-5 sm:px-6 lg:px-8">
        <header className={classNames('sticky top-3 z-20 mb-10', headerClassName)}>
          <div className="shell-header px-4 py-3">
            <div className="flex items-center gap-3 sm:gap-4">
              <SiteLink
                className="inline-flex min-w-0 flex-1 items-center gap-3"
                href={brandHref}
                onClick={() => setMobileMenuOpen(false)}
              >
                <BrandLogo className="h-9 sm:h-10" />
                <span className="min-w-0 text-sm text-ink-muted sm:hidden">Quick Git for normal folders</span>
              </SiteLink>
              <nav aria-label="Primary" className="hidden items-center gap-5 text-sm font-medium lg:flex">
                {navItems.map((item) => (
                  <SiteLink className="site-nav-link" href={item.href} key={item.href}>
                    {item.label}
                  </SiteLink>
                ))}
              </nav>
              <div className="hidden lg:block">
                <ButtonLink href={ctaHref} rel={ctaIsExternal ? 'noreferrer' : undefined} target={ctaIsExternal ? '_blank' : undefined}>
                  {ctaLabel}
                </ButtonLink>
              </div>
              <button
                aria-controls="mobile-nav"
                aria-expanded={mobileMenuOpen}
                aria-label={mobileMenuOpen ? 'Close navigation menu' : 'Open navigation menu'}
                className="inline-flex h-11 w-11 items-center justify-center rounded-token border border-white/70 bg-white/82 text-ink shadow-sm transition hover:border-accent/30 hover:text-accent-strong lg:hidden"
                onClick={() => setMobileMenuOpen((open) => !open)}
                type="button"
              >
                {mobileMenuOpen ? <X aria-hidden="true" className="h-5 w-5" /> : <Menu aria-hidden="true" className="h-5 w-5" />}
              </button>
            </div>

            {mobileMenuOpen ? (
              <div className="mt-4 border-t border-slate-900/6 pt-4 lg:hidden" id="mobile-nav">
                <nav aria-label="Mobile" className="grid gap-2">
                  {navItems.map((item) => (
                    <SiteLink
                      className="site-nav-link rounded-token px-3 py-2 text-sm font-medium"
                      href={item.href}
                      key={item.href}
                      onClick={() => setMobileMenuOpen(false)}
                    >
                      {item.label}
                    </SiteLink>
                  ))}
                </nav>
                <ButtonLink
                  className="mt-4 w-full sm:hidden"
                  href={ctaHref}
                  rel={ctaIsExternal ? 'noreferrer' : undefined}
                  target={ctaIsExternal ? '_blank' : undefined}
                >
                  {ctaLabel}
                </ButtonLink>
              </div>
            ) : null}
          </div>
        </header>

        <main className={classNames('flex-1 pb-8', contentSpacingClass, className)} id="main-content" tabIndex={-1}>
          <div id="top" />
          {children}
        </main>
      </div>
    </div>
  )
}
