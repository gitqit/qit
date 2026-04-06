declare module '*.mdx' {
  import type { ComponentType } from 'react'

  export const frontmatter: Record<string, string | number | undefined>
  const MDXContent: ComponentType
  export default MDXContent
}
