export const homebrewTap = 'gitqit/homebrew-qit'
export const homebrewFormula = `${homebrewTap}/qit`
export const homebrewInstallCommand = `brew install ${homebrewFormula}`
export const homebrewDocsHref = '/docs/install#install-with-homebrew'

export function prefersHomebrewInstall() {
  if (typeof navigator === 'undefined') {
    return false
  }

  return /(Macintosh|Mac OS X)/i.test(navigator.userAgent)
}

export function getPrimaryInstallCta() {
  if (prefersHomebrewInstall()) {
    return {
      href: homebrewDocsHref,
      label: 'Install with Homebrew',
    }
  }

  return {
    href: 'https://github.com/gitqit/qit/releases',
    label: 'Download binaries',
  }
}
