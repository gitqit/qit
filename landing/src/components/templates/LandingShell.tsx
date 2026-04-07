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
  contentSpacingClass = 'space-y-12 sm:space-y-14 lg:space-y-16',
  ctaHref,
  brandHref = '#top',
  headerClassName,
  navItems = defaultNavItems,
}: PropsWithChildren<{
  className?: string
  contentSpacingClass?: string
  ctaHref: string
  brandHref?: string
  headerClassName?: string
  navItems?: ReadonlyArray<ShellNavItem>
}>) {
  const [mobileMenuOpen, setMobileMenuOpen] = useState(false)
  const ctaIsExternal = isExternalHref(ctaHref)
  const navCtaLabel = 'Install'

  return (
    <div className="relative overflow-x-clip">
      <a className="skip-link" href="#main-content">
        Skip to content
      </a>

        <div aria-hidden="true" className="pointer-events-none absolute inset-0">
          <div className="absolute -left-32 -top-24 h-72 w-72 rounded-full bg-accent/16 blur-3xl" />
          <div className="absolute -right-32 top-88 h-80 w-80 rounded-full bg-sky-300/20 blur-3xl" />
          <div className="absolute -bottom-48 left-1/2 h-96 w-96 -translate-x-1/2 rounded-full bg-lime-300/18 blur-3xl" />
      </div>

      <div className="relative mx-auto flex min-h-svh w-full max-w-344 flex-col px-3 pb-8 pt-3 sm:px-5 sm:pb-10 sm:pt-4 lg:px-7">
        <header className={classNames('sticky top-2 z-20 mb-6 sm:mb-7', headerClassName)}>
          <div className="shell-header px-3 py-2 sm:px-3.5 sm:py-2.5">
            <div className="flex items-center gap-2.5 sm:gap-3">
              <SiteLink
                className="inline-flex min-w-0 flex-1 items-center gap-3"
                href={brandHref}
                onClick={() => setMobileMenuOpen(false)}
              >
                <BrandLogo className="h-8 sm:h-9" />
                <span className="text-base font-black tracking-tight text-ink sm:text-lg">Qit</span>
              </SiteLink>
              <nav aria-label="Primary" className="hidden items-center gap-4 text-sm font-medium md:flex">
                {navItems.map((item) => (
                  <SiteLink className="site-nav-link" href={item.href} key={item.href}>
                    {item.label}
                  </SiteLink>
                ))}
              </nav>
              <div className="hidden sm:block">
                <ButtonLink
                  className="shrink-0 whitespace-nowrap px-3 py-2 text-[13px] sm:text-sm"
                  href={ctaHref}
                  rel={ctaIsExternal ? 'noreferrer' : undefined}
                  target={ctaIsExternal ? '_blank' : undefined}
                >
                  {navCtaLabel}
                </ButtonLink>
              </div>
              <button
                aria-controls="mobile-nav"
                aria-expanded={mobileMenuOpen}
                aria-label={mobileMenuOpen ? 'Close navigation menu' : 'Open navigation menu'}
                className="inline-flex h-9 w-9 items-center justify-center rounded-token border border-white/70 bg-white/82 text-ink shadow-sm transition hover:border-accent/30 hover:text-accent-strong md:hidden"
                onClick={() => setMobileMenuOpen((open) => !open)}
                type="button"
              >
                {mobileMenuOpen ? <X aria-hidden="true" className="h-5 w-5" /> : <Menu aria-hidden="true" className="h-5 w-5" />}
              </button>
            </div>

            {mobileMenuOpen ? (
              <div className="mt-3 border-t border-slate-900/6 pt-3 md:hidden" id="mobile-nav">
                <nav aria-label="Mobile" className="grid gap-1.5">
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
                  className="mt-3 w-full justify-center"
                  href={ctaHref}
                  rel={ctaIsExternal ? 'noreferrer' : undefined}
                  target={ctaIsExternal ? '_blank' : undefined}
                >
                  {navCtaLabel}
                </ButtonLink>
              </div>
            ) : null}
          </div>
        </header>

        <main className={classNames('flex-1 pb-4 sm:pb-6', contentSpacingClass, className)} id="main-content" tabIndex={-1}>
          <div id="top" />
          {children}
        </main>
      </div>
    </div>
  )
}
