export function isExternalHref(href: string) {
  return /^(?:[a-z]+:)?\/\//i.test(href) || href.startsWith('mailto:') || href.startsWith('tel:')
}

export function isHashHref(href: string) {
  return href.startsWith('#')
}

export function isRoutableHref(href: string) {
  return href.startsWith('/') && !isExternalHref(href)
}
