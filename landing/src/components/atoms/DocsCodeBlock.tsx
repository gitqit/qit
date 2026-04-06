import { AlertCircle, Check, Copy } from 'lucide-react'
import {
  Children,
  cloneElement,
  type ComponentPropsWithoutRef,
  isValidElement,
  type ReactNode,
  useEffect,
  useId,
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

let mermaidInitialized = false
let mermaidModulePromise: Promise<typeof import('mermaid')> | null = null

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

function readLanguage(className?: string, dataLanguage?: string) {
  if (typeof dataLanguage === 'string' && dataLanguage.trim().length > 0) {
    return dataLanguage.trim().toLowerCase()
  }

  const match = className?.match(/language-([\w-]+)/)
  return match?.[1]?.toLowerCase() ?? 'snippet'
}

function formatLanguageLabel(rawLabel: string) {
  return rawLabel
    .trim()
    .toLowerCase()
    .split('-')
    .filter(Boolean)
    .map((segment) => segment.charAt(0).toUpperCase() + segment.slice(1))
    .join(' ')
}

function ensureMermaidInitialized() {
  return loadMermaid().then((mermaid) => {
    if (mermaidInitialized) {
      return mermaid
    }

    mermaid.initialize({
      startOnLoad: false,
      securityLevel: 'strict',
      theme: 'base',
      flowchart: {
        curve: 'basis',
        padding: 20,
        rankSpacing: 54,
        nodeSpacing: 42,
        useMaxWidth: false,
        htmlLabels: false,
      },
      themeVariables: {
        background: '#0f172a',
        primaryColor: '#182634',
        noteBkgColor: '#1a2336',
        noteTextColor: '#e2e8f0',
        radius: 16,
        strokeWidth: 1.6,
        useGradient: false,
        dropShadow: '0 16px 30px rgba(2, 6, 23, 0.28)',
        primaryTextColor: '#e2e8f0',
        secondaryColor: '#122a24',
        tertiaryColor: '#2a1836',
        primaryBorderColor: '#c4b5fd',
        secondaryBorderColor: '#86efac',
        tertiaryBorderColor: '#f9a8d4',
        secondaryTextColor: '#dcfce7',
        tertiaryTextColor: '#fce7f3',
        lineColor: '#94a3b8',
        arrowheadColor: '#c4b5fd',
        textColor: '#e2e8f0',
        nodeBkg: '#182634',
        mainBkg: '#0f172a',
        nodeBorder: '#c4b5fd',
        clusterBkg: '#111c2f',
        clusterBorder: '#475569',
        titleColor: '#f8fafc',
        edgeLabelBackground: '#0f172a',
        labelBackgroundColor: '#0f172a',
        nodeTextColor: '#e2e8f0',
        fontFamily: 'Inter, "SF Pro Text", "Segoe UI", ui-sans-serif, system-ui, sans-serif',
        fontSize: '16px',
      },
    })

    mermaidInitialized = true
    return mermaid
  })
}

function loadMermaid() {
  if (!mermaidModulePromise) {
    mermaidModulePromise = import('mermaid')
  }

  return mermaidModulePromise.then((module) => module.default)
}

function DocsMermaidBlock({
  source,
}: {
  source: string
}) {
  const reactId = useId()
  const diagramId = useMemo(() => `docs-mermaid-${reactId.replace(/[:]/g, '')}`, [reactId])
  const [svg, setSvg] = useState('')
  const [error, setError] = useState<string | null>(null)

  useEffect(() => {
    let cancelled = false

    async function renderDiagram() {
      try {
        const mermaid = await ensureMermaidInitialized()
        const { svg: renderedSvg } = await mermaid.render(diagramId, source)
        if (cancelled) {
          return
        }

        setSvg(renderedSvg)
        setError(null)
      } catch (renderError) {
        if (cancelled) {
          return
        }

        setSvg('')
        setError(renderError instanceof Error ? renderError.message : 'Unable to render Mermaid diagram.')
      }
    }

    void renderDiagram()

    return () => {
      cancelled = true
    }
  }, [diagramId, source])

  if (error) {
    return (
      <div className="docs-mermaid-fallback" role="status">
        <div className="docs-mermaid-fallback__header">
          <AlertCircle aria-hidden="true" className="h-4 w-4" strokeWidth={2.2} />
          <span>Unable to render this Mermaid diagram.</span>
        </div>
        <p className="docs-mermaid-fallback__message">{error}</p>
        <pre className="docs-pre docs-mermaid-fallback__source">
          <code className="docs-code-block__content">{source}</code>
        </pre>
      </div>
    )
  }

  return (
    <div className="docs-mermaid-surface">
      <div
        aria-label="Mermaid diagram"
        className="docs-mermaid__canvas"
        dangerouslySetInnerHTML={{ __html: svg }}
      />
    </div>
  )
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
  const rawLanguage = readLanguage(languageClassName, languageFromData)
  const languageLabel = formatLanguageLabel(rawLanguage)
  const isMermaidBlock = rawLanguage === 'mermaid'
  const copyValue = readTextContent(codeContent).replace(/\n$/, '')
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
    <figure className={classNames('docs-code-block', isMermaidBlock && 'docs-code-block--diagram')}>
      <figcaption className="docs-code-block__toolbar">
        <div aria-hidden="true" className="docs-code-block__traffic-lights">
          <span />
          <span />
          <span />
        </div>
        <span className="docs-code-block__label">{languageLabel}</span>
        {!isMermaidBlock ? (
          <button
            className={classNames('docs-code-block__copy', isCopied && 'docs-code-block__copy--copied')}
            onClick={handleCopy}
            type="button"
          >
            {isCopied ? <Check aria-hidden="true" className="h-3.5 w-3.5" strokeWidth={2.2} /> : <Copy aria-hidden="true" className="h-3.5 w-3.5" strokeWidth={2.2} />}
            <span>{isCopied ? 'Copied' : 'Copy'}</span>
          </button>
        ) : null}
      </figcaption>
      {isMermaidBlock ? (
        <DocsMermaidBlock source={copyValue} />
      ) : (
        <pre className={classNames('docs-pre', className)} {...props}>
          {renderedCode}
        </pre>
      )}
    </figure>
  )
}
