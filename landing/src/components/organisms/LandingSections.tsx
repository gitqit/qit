import type { ReactNode } from 'react'
import { CheckCircle2, FolderGit2, Globe2, ShieldCheck, Sparkles } from 'lucide-react'
import { Pill, SectionEyebrow, Surface, TerminalWindow } from '../atoms/primitives'
import { FaqItem, FeatureCard, HeroActions, LinkCluster, SectionIntro } from '../molecules/cards'
import type { FaqContent, FeatureContent, FooterLinks, HeroContent, PreviewContent } from '../../lib/content'

export function LandingHero({
  content,
  primaryCtaHref = content.primaryCta.href,
  primaryCtaLabel = content.primaryCta.label,
}: {
  content: HeroContent
  primaryCtaHref?: string
  primaryCtaLabel?: string
}) {
  return (
    <section className="grid gap-8 lg:grid-cols-[1.05fr_0.95fr] lg:items-center">
      <div className="space-y-5">
        <SectionEyebrow>{content.eyebrow}</SectionEyebrow>
        <div className="space-y-3">
          <h1 className="max-w-3xl text-4xl font-black tracking-[-0.04em] text-ink sm:text-5xl">{content.title}</h1>
          <p className="max-w-2xl text-base leading-7 text-ink-muted sm:text-lg">{content.description}</p>
          <p className="max-w-2xl text-sm leading-6 text-ink-subtle">{content.supportingNote}</p>
        </div>
        <HeroActions
          primaryHref={primaryCtaHref}
          primaryLabel={primaryCtaLabel}
          secondaryHref={content.secondaryCta.href}
          secondaryLabel={content.secondaryCta.label}
        />
        <div className="flex flex-wrap gap-1.5">
          {content.highlights.map((highlight) => (
            <Pill key={highlight}>{highlight}</Pill>
          ))}
        </div>
      </div>

      <div className="grid gap-3">
        <Surface className="panel-surface-soft relative overflow-hidden">
          <div aria-hidden="true" className="absolute -right-8 -top-8 h-32 w-32 rounded-full bg-accent/10 blur-3xl" />
          <div aria-hidden="true" className="absolute -bottom-10 -left-4 h-28 w-28 rounded-full bg-lime-300/15 blur-3xl" />
          <div className="relative space-y-4">
            <div className="flex items-center justify-between gap-3">
              <div className="space-y-1.5">
                <p className="text-sm font-semibold uppercase tracking-[0.2em] text-ink-subtle">{content.quickStartLabel}</p>
                <h2 className="text-xl font-black tracking-tight text-ink">{content.quickStartTitle}</h2>
                <p className="max-w-xl text-sm leading-6 text-ink-muted">{content.quickStartDescription}</p>
              </div>
              <Sparkles aria-hidden="true" className="h-6 w-6 shrink-0 text-accent-strong" strokeWidth={2} />
            </div>
            <div className="terminal-code-block overflow-x-auto px-3.5 py-3 font-mono text-sm text-emerald-300">
              {content.quickStartCommand}
            </div>
            <div className="grid gap-2.5 sm:grid-cols-3" role="list" aria-label="Qit quick facts">
              <MiniStat icon={<FolderGit2 aria-hidden="true" className="h-4 w-4" strokeWidth={2} />} label="Host tree" value="Unchanged" />
              <MiniStat icon={<ShieldCheck aria-hidden="true" className="h-4 w-4" strokeWidth={2} />} label="Auth" value="Per session" />
              <MiniStat icon={<Globe2 aria-hidden="true" className="h-4 w-4" strokeWidth={2} />} label="Protocol" value="Smart HTTP" />
            </div>
          </div>
        </Surface>
      </div>
    </section>
  )
}

function MiniStat({
  icon,
  label,
  value,
}: {
  icon: ReactNode
  label: string
  value: string
}) {
  return (
    <div className="stats-card p-3" role="listitem">
      <div className="inline-flex items-center gap-1.5 text-xs font-semibold uppercase tracking-[0.16em] text-ink-subtle">
        {icon}
        <span>{label}</span>
      </div>
      <p className="mt-1.5 text-base font-bold text-ink">{value}</p>
    </div>
  )
}

export function FeatureGrid({
  content,
}: {
  content: FeatureContent
}) {
  return (
    <section className="space-y-6" id="features">
      <SectionIntro
        align="center"
        description="Qit keeps the collaboration surface small: normal folders, familiar Git clients, clear session auth, and an apply step you control."
        eyebrow="Why it feels different"
        title="A smaller collaboration surface with fewer surprises."
      />
      <div className="grid gap-3.5 md:grid-cols-2">
        {content.map((feature) => (
          <FeatureCard
            description={feature.description}
            eyebrow={feature.eyebrow}
            key={feature.title}
            title={feature.title}
          />
        ))}
      </div>
    </section>
  )
}

export function PreviewStrip({
  content,
}: {
  content: PreviewContent
}) {
  return (
    <section className="space-y-6" id="preview">
      <SectionIntro
        eyebrow={content.eyebrow}
        title={content.title}
        description={content.description}
      />
      <div className="grid gap-3.5 xl:grid-cols-[1.1fr_0.9fr]">
        <TerminalWindow title={content.terminalTitle}>
          <ul aria-label={content.terminalTitle} className="space-y-1.5">
            {content.terminalLines.map((line) => (
              <li className="flex gap-2.5" key={line}>
                <span aria-hidden="true" className="mt-[0.55rem] h-1.5 w-1.5 shrink-0 rounded-full bg-emerald-400" />
                <span className={line.startsWith('$') ? 'text-emerald-300' : 'text-slate-200'}>{line}</span>
              </li>
            ))}
          </ul>
        </TerminalWindow>

        <div className="grid gap-3.5">
          <Surface className="panel-surface-soft space-y-4">
            <div className="space-y-1.5">
              <p className="text-sm font-semibold uppercase tracking-[0.2em] text-ink-subtle">Review flow</p>
              <h3 className="text-xl font-black tracking-tight text-ink">Push into the sidecar first.</h3>
            </div>
            <ol className="space-y-2">
              {content.flowSteps.map((step) => (
                <li className="flex gap-2.5" key={step}>
                  <CheckCircle2 aria-hidden="true" className="mt-1 h-5 w-5 shrink-0 text-lime-500" strokeWidth={2} />
                  <p className="text-sm leading-6 text-ink-muted">{step}</p>
                </li>
              ))}
            </ol>
          </Surface>

          <Surface className="panel-surface-soft space-y-3">
            <div className="flex items-center justify-between gap-3">
              <div>
                <p className="text-sm font-semibold uppercase tracking-[0.2em] text-ink-subtle">Web UI snapshot</p>
                <h3 className="text-xl font-black tracking-tight text-ink">See what is live before anyone clones.</h3>
              </div>
              <div className="accent-pill inline-flex items-center px-2.5 py-0.5 text-xs font-semibold text-accent-strong">
                Preview
              </div>
            </div>
            <dl className="grid gap-2.5 sm:grid-cols-2">
              {content.uiCards.map((card) => (
                <div className="muted-card px-3.5 py-3" key={card.label}>
                  <dt className="text-xs font-semibold uppercase tracking-[0.16em] text-ink-subtle">{card.label}</dt>
                  <dd className="mt-1.5 text-base font-bold text-ink">{card.value}</dd>
                </div>
              ))}
            </dl>
          </Surface>
        </div>
      </div>
    </section>
  )
}

export function FaqSection({
  content,
}: {
  content: FaqContent
}) {
  return (
    <section className="space-y-6" id="faq">
      <SectionIntro
        eyebrow="FAQ"
        title="Questions developers ask before they trust a new workflow."
        description="The goal is lightweight collaboration, not a hidden sync engine. These are the tradeoffs Qit makes explicit."
      />
      <div className="grid gap-3">
        {content.map((item) => (
          <FaqItem answer={item.answer} key={item.question} question={item.question} />
        ))}
      </div>
    </section>
  )
}

export function LandingFooter({
  links,
}: {
  links: FooterLinks
}) {
  return (
    <footer className="space-y-5 border-t border-slate-900/8 pt-6">
      <div className="space-y-2">
        <SectionEyebrow>Install, docs, and links</SectionEyebrow>
        <h2 className="text-3xl font-black tracking-tight text-ink">Ship the folder. Keep the workflow readable.</h2>
        <p className="max-w-2xl text-base leading-7 text-ink-muted">
          Qit is for the moment when you need to share live work quickly, keep Git client compatibility, and avoid forcing a local folder into a heavier collaboration shape.
        </p>
      </div>
      <LinkCluster links={links} />
    </footer>
  )
}
