import type { MDXComponents } from 'mdx/types'
import { DocsCodeBlock } from '../components/atoms/DocsCodeBlock'
import { isExternalHref } from './hrefs'
import { SiteLink } from './siteLinks'

export const mdxComponents: MDXComponents = {
  a: ({ href = '', ...props }) => {
    const external = isExternalHref(href)

    return (
      <SiteLink
        href={href}
        rel={external ? 'noreferrer' : props.rel}
        target={external ? '_blank' : props.target}
        {...props}
      />
    )
  },
  pre: (props) => <DocsCodeBlock {...props} />,
  code: (props) => <code className="docs-code" {...props} />,
}
