import type { ComponentPropsWithoutRef } from 'react'

type DocsImagePreviewProps = ComponentPropsWithoutRef<'img'>

function previewUrlForImage(src?: string) {
  const filename = src?.split('/').pop() ?? ''

  switch (filename) {
    case 'web-ui-code-tab.png':
      return 'https://example.com/?tab=code'
    case 'web-ui-commits-tab.png':
      return 'https://example.com/?tab=commits&commit=4f3c2a1'
    case 'web-ui-branches-tab.png':
      return 'https://example.com/?tab=branches'
    case 'web-ui-issues-tab.png':
      return 'https://example.com/?tab=issues&issue=42'
    case 'web-ui-pull-requests-tab.png':
      return 'https://example.com/?tab=pull-requests&pr=7'
    case 'web-ui-settings-tab.png':
      return 'https://example.com/?tab=settings'
    case 'web-ui-user-settings-tab.png':
      return 'https://example.com/account'
    case 'web-ui-auth-login.png':
      return 'https://example.com/login'
    default:
      return 'https://example.com/'
  }
}

export function DocsImagePreview({
  alt = '',
  loading = 'lazy',
  src,
  ...props
}: DocsImagePreviewProps) {
  const previewUrl = previewUrlForImage(src)

  return (
    <figure className="docs-code-block docs-browser-preview">
      <figcaption className="docs-code-block__toolbar">
        <div aria-hidden="true" className="docs-code-block__traffic-lights">
          <span />
          <span />
          <span />
        </div>
        <div aria-label={`Browser address bar showing ${previewUrl}`} className="docs-browser-preview__address-bar">
          <span className="docs-browser-preview__address-text">{previewUrl}</span>
        </div>
      </figcaption>
      <div className="docs-browser-preview__viewport">
        <img
          alt={alt}
          className="docs-browser-preview__image"
          loading={loading}
          src={src}
          {...props}
        />
      </div>
    </figure>
  )
}
