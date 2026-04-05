import { ChevronRight, Home } from 'lucide-react'

export function AuthorBadge({ name }: { name: string }) {
  return (
    <span className="inline-flex items-center gap-2 text-xs text-fg-muted">
      <span className="flex h-6 w-6 items-center justify-center rounded-full border border-border bg-panel-emphasis text-[11px] font-semibold uppercase text-fg">
        {name.trim().charAt(0) || '?'}
      </span>
      <span>{name}</span>
    </span>
  )
}

function pathSegments(path: string) {
  return path.split('/').filter(Boolean)
}

export function RepoPathBreadcrumbs({
  path,
  onBrowse,
}: {
  path: string
  onBrowse: (path: string) => Promise<void>
}) {
  const segments = pathSegments(path)

  return (
    <div className="flex min-w-0 flex-wrap items-center gap-1 text-sm">
      <button
        aria-label="Browse repository root"
        className="inline-flex items-center rounded-md px-2 py-1 text-fg hover:bg-panel-subtle"
        onClick={() => void onBrowse('')}
        title="Repository root"
        type="button"
      >
        <Home aria-hidden="true" className="h-4 w-4" strokeWidth={1.85} />
      </button>
      {segments.map((segment, index) => {
        const nextPath = segments.slice(0, index + 1).join('/')
        const isCurrent = index === segments.length - 1
        return (
          <span className="flex items-center gap-1" key={nextPath}>
            <ChevronRight aria-hidden="true" className="h-3.5 w-3.5 text-fg-subtle" strokeWidth={1.85} />
            {isCurrent ? (
              <span className="rounded-md bg-panel px-2 py-1 font-medium text-fg">{segment}</span>
            ) : (
              <button
                className="rounded-md px-2 py-1 text-fg hover:bg-panel-subtle"
                onClick={() => void onBrowse(nextPath)}
                type="button"
              >
                {segment}
              </button>
            )}
          </span>
        )
      })}
    </div>
  )
}
