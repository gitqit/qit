import {
  faqContent,
  featureContent,
  footerLinks,
  heroContent,
  previewContent,
} from '../../lib/content'
import { getPrimaryInstallCta } from '../../lib/install'
import {
  FaqSection,
  FeatureGrid,
  LandingFooter,
  LandingHero,
  PreviewStrip,
} from '../organisms/LandingSections'
import { LandingShell } from '../templates/LandingShell'

export function LandingPage() {
  const installCta = getPrimaryInstallCta()

  return (
    <LandingShell
      ctaHref={installCta.href}
      ctaLabel={installCta.label}
      navItems={[
        { href: '#features', label: 'Features' },
        { href: '#preview', label: 'Preview' },
        { href: '#faq', label: 'FAQ' },
        { href: '/docs/install', label: 'Docs' },
      ]}
    >
      <LandingHero content={heroContent} primaryCtaHref={installCta.href} primaryCtaLabel={installCta.label} />
      <FeatureGrid content={featureContent} />
      <PreviewStrip content={previewContent} />
      <FaqSection content={faqContent} />
      <LandingFooter links={footerLinks} />
    </LandingShell>
  )
}
