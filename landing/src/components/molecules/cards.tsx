import { Disclosure, DisclosureButton, DisclosurePanel } from '@headlessui/react'
import { ChevronDown } from 'lucide-react'
import { classNames } from '../../lib/classNames'
import { ButtonLink, Pill, SectionEyebrow, Surface } from '../atoms/primitives'

export function SectionIntro({
  eyebrow,
  title,
  description,
  align = 'left',
}: {
  eyebrow: string
  title: string
  description: string
  align?: 'left' | 'center'
}) {
  return (
    <div className={classNames('space-y-4', align === 'center' ? 'mx-auto max-w-3xl text-center' : 'max-w-2xl')}>
      <SectionEyebrow>{eyebrow}</SectionEyebrow>
      <div className="space-y-3">
        <h2 className="text-3xl font-black tracking-tight text-ink sm:text-4xl">{title}</h2>
        <p className="text-base leading-7 text-ink-muted sm:text-lg">{description}</p>
      </div>
    </div>
  )
}

export function HeroActions({
  primaryHref,
  primaryLabel,
  secondaryHref,
  secondaryLabel,
}: {
  primaryHref: string
  primaryLabel: string
  secondaryHref: string
  secondaryLabel: string
}) {
  return (
    <div className="flex flex-col gap-3 sm:flex-row">
      <ButtonLink href={primaryHref} rel="noreferrer" target="_blank">
        {primaryLabel}
      </ButtonLink>
      <ButtonLink href={secondaryHref} rel="noreferrer" target="_blank" tone="secondary">
        {secondaryLabel}
      </ButtonLink>
    </div>
  )
}

export function FeatureCard({
  eyebrow,
  title,
  description,
}: {
  eyebrow: string
  title: string
  description: string
}) {
  return (
    <Surface className="panel-surface-soft h-full">
      <div className="space-y-4">
        <Pill>{eyebrow}</Pill>
        <div className="space-y-3">
          <h3 className="text-xl font-bold tracking-tight text-ink">{title}</h3>
          <p className="text-sm leading-7 text-ink-muted">{description}</p>
        </div>
      </div>
    </Surface>
  )
}

export function FaqItem({
  answer,
  question,
}: {
  question: string
  answer: string
}) {
  return (
    <Disclosure as="div">
      {({ open }) => (
        <Surface className="panel-surface-soft p-0">
          <DisclosureButton className="flex w-full items-center justify-between gap-4 px-5 py-4 text-left">
            <span className="text-base font-semibold text-ink sm:text-lg">{question}</span>
            <ChevronDown
              aria-hidden="true"
              className={classNames('h-5 w-5 shrink-0 text-accent-strong transition-transform', open && 'rotate-180')}
              strokeWidth={2}
            />
          </DisclosureButton>
          <DisclosurePanel className="px-5 pb-5 pt-0 text-sm leading-7 text-ink-muted">
            {answer}
          </DisclosurePanel>
        </Surface>
      )}
    </Disclosure>
  )
}

export function LinkCluster({
  links,
}: {
  links: ReadonlyArray<{ href: string; label: string }>
}) {
  return (
    <div className="flex flex-wrap items-center gap-3">
      {links.map((link) => (
        <ButtonLink
          className="px-4 py-2.5 text-sm"
          href={link.href}
          key={link.href}
          rel="noreferrer"
          target="_blank"
          tone="secondary"
        >
          {link.label}
        </ButtonLink>
      ))}
    </div>
  )
}
