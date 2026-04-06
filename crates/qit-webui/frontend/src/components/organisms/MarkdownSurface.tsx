import ReactMarkdown from 'react-markdown'
import remarkGfm from 'remark-gfm'

export function MarkdownSurface({ source }: { source: string }) {
  return (
    <div className="markdown-body">
      <ReactMarkdown
        components={{
          a: ({ children, ...props }) => (
            <a {...props} className="text-accent-strong hover:underline" rel="noreferrer" target="_blank">
              {children}
            </a>
          ),
          code: ({ children, className, ...props }) => {
            const isBlock = Boolean(className)
            return (
              <code
                {...props}
                className={isBlock ? className : 'rounded-md border border-border bg-canvas px-1.5 py-0.5 text-[0.95em]'}
              >
                {children}
              </code>
            )
          },
          table: ({ children }) => (
            <div className="overflow-x-auto">
              <table>{children}</table>
            </div>
          ),
        }}
        remarkPlugins={[remarkGfm]}
      >
        {source}
      </ReactMarkdown>
    </div>
  )
}
