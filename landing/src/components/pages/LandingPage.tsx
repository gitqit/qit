import {
  faqContent,
  featureContent,
  footerLinks,
  heroContent,
  previewContent,
} from '../../lib/content'
import {
  FaqSection,
  FeatureGrid,
  LandingFooter,
  LandingHero,
  PreviewStrip,
} from '../organisms/LandingSections'
import { LandingShell } from '../templates/LandingShell'

export function LandingPage() {
  return (
    <LandingShell ctaHref={heroContent.primaryCta.href}>
      <LandingHero content={heroContent} />
      <FeatureGrid content={featureContent} />
      <PreviewStrip content={previewContent} />
      <FaqSection content={faqContent} />
      <LandingFooter links={footerLinks} />
    </LandingShell>
  )
}
