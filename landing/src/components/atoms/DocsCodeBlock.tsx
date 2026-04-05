import { Check, Copy } from 'lucide-react'
import {
  Children,
  cloneElement,
  type ComponentPropsWithoutRef,
  isValidElement,
  type ReactNode,
  useMemo,
  useState,
} from 'react'
import { classNames } from '../../lib/classNames'

type CodeElementProps = {
  children?: ReactNode
  className?: string
  style?: ComponentPropsWithoutRef<'code'>['style']
  'data-language'?: string
}

type PreElementProps = ComponentPropsWithoutRef<'pre'> & {
  'data-language'?: string
}

function readTextContent(node: ReactNode): string {
  if (typeof node === 'string' || typeof node === 'number') {
    return String(node)
  }

  if (Array.isArray(node)) {
    return node.map((child) => readTextContent(child)).join('')
  }

  if (isValidElement<{ children?: ReactNode }>(node)) {
    return readTextContent(node.props.children)
  }

  return ''
}

function formatLanguageLabel(className?: string) {
  const match = className?.match(/language-([\w-]+)/)
  const rawLabel = match?.[1] ?? 'snippet'

  return rawLabel
    .split('-')
    .filter(Boolean)
    .map((segment) => segment.charAt(0).toUpperCase() + segment.slice(1))
    .join(' ')
}

export function DocsCodeBlock({
  children,
  className,
  ...props
}: PreElementProps) {
  const child = Children.count(children) === 1 ? Children.only(children) : null
  const codeChild = isValidElement<CodeElementProps>(child) ? child : null
  const languageClassName = codeChild?.props.className ?? className ?? ''
  const languageFromData = typeof codeChild?.props['data-language'] === 'string'
    ? codeChild.props['data-language']
    : typeof props['data-language'] === 'string'
      ? props['data-language']
      : undefined
  const codeContent = codeChild?.props.children ?? children
  const languageLabel = useMemo(
    () => formatLanguageLabel(languageFromData ? `language-${languageFromData}` : languageClassName),
    [languageClassName, languageFromData],
  )
  const copyValue = useMemo(() => readTextContent(codeContent).replace(/\n$/, ''), [codeContent])
  const [isCopied, setIsCopied] = useState(false)
  const renderedCode = codeChild
    ? cloneElement(codeChild, {
      className: classNames('docs-code-block__content', codeChild.props.className),
    })
    : <code className="docs-code-block__content">{codeContent}</code>

  async function handleCopy() {
    try {
      await navigator.clipboard.writeText(copyValue)
      setIsCopied(true)
      window.setTimeout(() => setIsCopied(false), 1600)
    } catch {
      setIsCopied(false)
    }
  }

  return (
    <figure className="docs-code-block">
      <figcaption className="docs-code-block__toolbar">
        <div aria-hidden="true" className="docs-code-block__traffic-lights">
          <span />
          <span />
          <span />
        </div>
        <span className="docs-code-block__label">{languageLabel}</span>
        <button
          className={classNames('docs-code-block__copy', isCopied && 'docs-code-block__copy--copied')}
          onClick={handleCopy}
          type="button"
        >
          {isCopied ? <Check aria-hidden="true" className="h-3.5 w-3.5" strokeWidth={2.2} /> : <Copy aria-hidden="true" className="h-3.5 w-3.5" strokeWidth={2.2} />}
          <span>{isCopied ? 'Copied' : 'Copy'}</span>
        </button>
      </figcaption>
      <pre className={classNames('docs-pre', className)} {...props}>
        {renderedCode}
      </pre>
    </figure>
  )
}
