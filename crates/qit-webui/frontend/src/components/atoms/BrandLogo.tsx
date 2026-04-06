import logoOnDark from '../../assets/branding/qit-logo-on-dark.png'
import logoOnLight from '../../assets/branding/qit-logo-on-light.png'

function classNames(...values: Array<string | false | null | undefined>) {
  return values.filter(Boolean).join(' ')
}

export function BrandLogo({
  alt = 'Qit',
  className,
  theme = 'dark-surface',
}: {
  alt?: string
  className?: string
  theme?: 'dark-surface' | 'light-surface'
}) {
  const src = theme === 'light-surface' ? logoOnLight : logoOnDark

  return <img alt={alt} className={classNames('block h-auto w-auto', className)} src={src} />
}
