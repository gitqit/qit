import type { AnchorHTMLAttributes, PropsWithChildren } from 'react'
import { Link } from 'react-router-dom'
import { isExternalHref, isHashHref, isRoutableHref } from './hrefs'

export function SiteLink({
  children,
  href,
  ...props
}: PropsWithChildren<
  Omit<AnchorHTMLAttributes<HTMLAnchorElement>, 'href'> & {
    href: string
  }
>) {
  if (props.target === '_blank' || isExternalHref(href) || isHashHref(href) || !isRoutableHref(href)) {
    return (
      <a href={href} {...props}>
        {children}
      </a>
    )
  }

  return (
    <Link to={href} {...props}>
      {children}
    </Link>
  )
}
