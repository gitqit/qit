import type { AnchorHTMLAttributes, PropsWithChildren } from 'react'
import { ArrowUpRight, FoldVertical, Terminal } from 'lucide-react'
import { classNames } from '../../lib/classNames'
import { SiteLink } from '../../lib/siteLinks'
import logoOnDark from '../../assets/branding/qit-logo-on-dark.png'
import logoOnLight from '../../assets/branding/qit-logo-on-light.png'

export function SectionEyebrow({ children }: PropsWithChildren) {
  return (
    <p className="eyebrow-pill inline-flex items-center gap-2 px-3 py-1 text-xs font-semibold uppercase tracking-[0.2em] text-ink-subtle">
      <FoldVertical aria-hidden="true" className="h-3.5 w-3.5 text-accent-strong" strokeWidth={2} />
      <span>{children}</span>
    </p>
  )
}

export function Pill({ children }: PropsWithChildren) {
  return (
    <span className="accent-pill inline-flex items-center px-3 py-1 text-xs font-semibold text-accent-strong">
      {children}
    </span>
  )
}

export function ButtonLink({
  children,
  className,
  href,
  tone = 'primary',
  ...props
}: PropsWithChildren<
  Omit<AnchorHTMLAttributes<HTMLAnchorElement>, 'href'> & {
    href: string
    tone?: 'primary' | 'secondary' | 'ghost'
  }
>) {
  const tones = {
    primary: 'button-link-primary',
    secondary: 'button-link-secondary',
    ghost: 'button-link-ghost',
  } as const

  return (
    <SiteLink
      className={classNames(
        'button-link inline-flex items-center justify-center gap-2 rounded-token border px-5 py-3 text-sm font-semibold focus-visible:outline-2 focus-visible:outline-offset-4 focus-visible:outline-accent',
        tones[tone],
        className,
      )}
      href={href}
      {...props}
    >
      <span>{children}</span>
      {tone !== 'ghost' ? <ArrowUpRight aria-hidden="true" className="h-4 w-4" strokeWidth={2} /> : null}
    </SiteLink>
  )
}

export function BrandLogo({
  alt = 'Qit',
  className,
  theme = 'light-surface',
}: {
  alt?: string
  className?: string
  theme?: 'dark-surface' | 'light-surface'
}) {
  const src = theme === 'light-surface' ? logoOnLight : logoOnDark

  return (
    <img alt={alt} className={classNames('block h-auto w-auto', className)} src={src} />
  )
}

export function Surface({
  children,
  className,
}: PropsWithChildren<{ className?: string }>) {
  return (
    <div
      className={classNames(
        'panel-surface p-6',
        className,
      )}
    >
      {children}
    </div>
  )
}

export function TerminalWindow({
  children,
  title,
}: PropsWithChildren<{ title: string }>) {
  return (
    <Surface className="terminal-surface overflow-hidden p-0 text-slate-100">
      <div className="flex items-center justify-between border-b border-white/10 px-4 py-3">
        <div aria-hidden="true" className="flex items-center gap-2">
          <span className="h-2.5 w-2.5 rounded-full bg-rose-400" />
          <span className="h-2.5 w-2.5 rounded-full bg-amber-300" />
          <span className="h-2.5 w-2.5 rounded-full bg-emerald-400" />
        </div>
        <div className="inline-flex items-center gap-2 text-xs font-medium text-slate-400">
          <Terminal aria-hidden="true" className="h-3.5 w-3.5" strokeWidth={1.9} />
          <span>{title}</span>
        </div>
      </div>
      <div className="space-y-2 px-4 py-4 font-mono text-sm leading-6">{children}</div>
    </Surface>
  )
}
