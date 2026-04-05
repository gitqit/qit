import { useMemo, useState, type ReactNode } from 'react'
import { ArrowUp, Copy, Download, PanelLeftClose, PanelLeftOpen } from 'lucide-react'
import ReactMarkdown from 'react-markdown'
import remarkGfm from 'remark-gfm'
import { Badge, EmptyState, IconButton } from '../../atoms/Controls'
import { RepoEntryIcon } from '../../../lib/fileIcons'
import { MonacoCodeSurface } from '../MonacoCodeSurface'
import { RepoFileTree } from '../RepoFileTree'
import type { BlobContent, CommitHistoryNode, TreeEntry } from '../../../lib/types'
import { AuthorBadge, RepoPathBreadcrumbs } from './shared'
import {
  formatBytes,
  formatRelativeTime,
  isMarkdownPath,
  parentPath,
  shortSha,
  sortTreeEntries,
} from './panelUtils'

function LatestCommitBar({ commit }: { commit: CommitHistoryNode | null }) {
  if (!commit) {
    return null
  }

  return (
    <div className="flex flex-col gap-3 border-b border-border bg-panel-subtle px-4 py-3 lg:flex-row lg:items-center lg:justify-between">
      <div className="flex min-w-0 items-center gap-3">
        <AuthorBadge name={commit.author} />
        <p className="truncate text-sm text-fg">
          <span className="font-medium">{commit.summary || commit.id}</span>
        </p>
      </div>
      <div className="flex flex-wrap items-center gap-2 text-xs text-fg-muted">
        <span className="rounded-full border border-border bg-canvas px-2 py-0.5 font-mono text-fg-subtle">
          {shortSha(commit.id)}
        </span>
        <span>{formatRelativeTime(commit.authored_at)}</span>
      </div>
    </div>
  )
}

function TreeRow({
  entry,
  active,
  onOpen,
}: {
  entry: TreeEntry
  active: boolean
  onOpen: (entry: TreeEntry) => Promise<void>
}) {
  return (
    <button
      className={`grid w-full grid-cols-[minmax(0,1fr)_80px_100px] items-center gap-3 px-4 py-3 text-left text-sm transition hover:bg-panel-subtle ${
        active ? 'bg-accent/8' : ''
      }`}
      onClick={() => void onOpen(entry)}
      type="button"
    >
      <span className="flex min-w-0 items-center gap-3">
        <RepoEntryIcon className="h-4 w-4" kind={entry.kind} name={entry.name} />
        <span className={`truncate ${entry.kind === 'tree' ? 'font-semibold text-accent-strong' : 'text-fg'}`}>
          {entry.name}
        </span>
      </span>
      <span className="text-xs uppercase tracking-[0.12em] text-fg-subtle">
        {entry.kind === 'tree' ? 'Folder' : 'File'}
      </span>
      <span className="text-right text-xs text-fg-muted">{formatBytes(entry.size)}</span>
    </button>
  )
}

function RepoTreeCard({
  currentPath,
  entries,
  latestCommit,
  activePath,
  action,
  onBrowse,
  onOpen,
}: {
  currentPath: string
  entries: TreeEntry[]
  latestCommit: CommitHistoryNode | null
  activePath: string | null
  action?: ReactNode
  onBrowse: (path: string) => Promise<void>
  onOpen: (entry: TreeEntry) => Promise<void>
}) {
  const sortedEntries = useMemo(() => sortTreeEntries(entries), [entries])

  return (
    <section className="overflow-hidden rounded-[var(--radius-lg)] border border-border bg-panel">
      <div className="flex flex-col gap-3 border-b border-border bg-canvas-raised/65 px-4 py-4 lg:flex-row lg:items-center lg:justify-between">
        <RepoPathBreadcrumbs onBrowse={onBrowse} path={currentPath} />
        <div className="flex flex-col gap-2 lg:items-end">
          {action ? <div className="flex justify-end text-sm">{action}</div> : null}
          <div className="flex flex-wrap items-center gap-2 text-xs text-fg-muted lg:justify-end">
            {currentPath ? (
              <IconButton
                icon={<ArrowUp className="h-4 w-4" strokeWidth={1.9} />}
                label="Browse parent directory"
                onClick={() => void onBrowse(parentPath(currentPath))}
                tone="muted"
              />
            ) : null}
          </div>
        </div>
      </div>
      <LatestCommitBar commit={latestCommit} />
      {sortedEntries.length === 0 ? (
        <div className="p-5">
          <EmptyState title="No files here yet" message="This folder is empty or the snapshot could not be resolved." />
        </div>
      ) : (
        <div>
          <div className="grid grid-cols-[minmax(0,1fr)_80px_100px] gap-3 border-b border-border px-4 py-2 text-[11px] font-semibold uppercase tracking-[0.14em] text-fg-subtle">
            <span>Name</span>
            <span>Type</span>
            <span className="text-right">Size</span>
          </div>
          {sortedEntries.map((entry) => (
            <div className="border-b border-border/80 last:border-b-0" key={entry.path}>
              <TreeRow active={activePath === entry.path} entry={entry} onOpen={onOpen} />
            </div>
          ))}
        </div>
      )}
    </section>
  )
}

function MarkdownDocument({ source }: { source: string }) {
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

function BlobPreview({
  blob,
  emptyTitle,
  emptyMessage,
}: {
  blob: BlobContent | null
  emptyTitle: string
  emptyMessage: string
}) {
  if (!blob) {
    return <EmptyState title={emptyTitle} message={emptyMessage} />
  }

  if (blob.is_binary) {
    return <EmptyState title="Binary file" message="This file is binary, so Qit does not render it inline." />
  }

  if (!blob.text) {
    return <EmptyState title="Empty file" message="This file has no text content to render." />
  }

  if (isMarkdownPath(blob.path)) {
    return <MarkdownDocument source={blob.text} />
  }

  return (
    <pre className="max-h-[70vh] overflow-auto rounded-[var(--radius-sm)] bg-canvas px-4 py-4 text-[13px] leading-6 text-fg">
      {blob.text}
    </pre>
  )
}

function lineStats(text: string) {
  const lines = text.split('\n')
  return {
    lines: lines.length,
    loc: lines.filter((line) => line.trim().length > 0).length,
  }
}

function BlobCodeView({
  blob,
  latestCommit,
  treeCache,
  currentPath,
  onBrowse,
  onLoadTreePath,
  onOpen,
}: {
  blob: BlobContent
  latestCommit: CommitHistoryNode | null
  treeCache: Record<string, TreeEntry[]>
  currentPath: string
  onBrowse: (path: string) => Promise<void>
  onLoadTreePath: (path: string) => Promise<TreeEntry[]>
  onOpen: (entry: TreeEntry) => Promise<void>
}) {
  const [treeOpen, setTreeOpen] = useState(true)
  const stats = blob.text ? lineStats(blob.text) : null

  const downloadBlob = () => {
    if (!blob.text) return

    const anchor = document.createElement('a')
    const url = URL.createObjectURL(new Blob([blob.text], { type: 'text/plain;charset=utf-8' }))
    anchor.href = url
    anchor.download = blob.path.split('/').at(-1) ?? 'file.txt'
    anchor.click()
    URL.revokeObjectURL(url)
  }

  return (
    <div className="space-y-4">
      <div className="flex flex-wrap items-center gap-3">
        <IconButton
          icon={
            treeOpen ? (
              <PanelLeftClose className="h-4 w-4" strokeWidth={1.85} />
            ) : (
              <PanelLeftOpen className="h-4 w-4" strokeWidth={1.85} />
            )
          }
          label={treeOpen ? 'Hide file tree' : 'Show file tree'}
          onClick={() => setTreeOpen((open) => !open)}
          tone="muted"
        />
        <div className="min-w-0 flex-1">
          <RepoPathBreadcrumbs onBrowse={onBrowse} path={blob.path} />
        </div>
      </div>

      <div className={`grid items-stretch gap-4 ${treeOpen ? 'lg:grid-cols-[280px_minmax(0,1fr)]' : 'grid-cols-1'}`}>
        {treeOpen ? (
          <aside className="min-w-0">
            <RepoFileTree
              activePath={blob.path}
              currentPath={currentPath}
              onLoadPath={async (path) => {
                await onLoadTreePath(path)
              }}
              onOpenFile={async (path) => {
                await onOpen({
                  kind: 'blob',
                  name: path.split('/').at(-1) ?? path,
                  oid: '',
                  path,
                  size: null,
                })
              }}
              treeCache={treeCache}
            />
          </aside>
        ) : null}

        <section className="flex min-h-[24rem] flex-col overflow-hidden rounded-[var(--radius-lg)] border border-border bg-panel lg:h-[44rem]">
          <div className="flex flex-col gap-3 border-b border-border bg-panel-subtle px-4 py-3 lg:flex-row lg:items-center lg:justify-between">
            <div className="min-w-0 space-y-1">
              <div className="flex min-w-0 flex-wrap items-center gap-2">
                <h3 className="truncate text-base font-semibold text-fg">{blob.path}</h3>
                <Badge tone="accent">Code</Badge>
              </div>
              {latestCommit ? (
                <div className="flex flex-wrap items-center gap-2 text-xs text-fg-muted">
                  <AuthorBadge name={latestCommit.author} />
                  <span className="truncate">{latestCommit.summary || latestCommit.id}</span>
                  <span className="rounded-full border border-border bg-canvas px-2 py-0.5 font-mono text-fg-subtle">
                    {shortSha(latestCommit.id)}
                  </span>
                  <span>{formatRelativeTime(latestCommit.authored_at)}</span>
                </div>
              ) : null}
            </div>
            <div className="flex flex-wrap items-center gap-2">
              {blob.text ? (
                <IconButton
                  icon={<Copy className="h-4 w-4" strokeWidth={1.9} />}
                  label="Copy raw file contents"
                  onClick={() => {
                    void navigator.clipboard.writeText(blob.text ?? '')
                  }}
                  tone="muted"
                />
              ) : null}
              {blob.text ? (
                <IconButton
                  icon={<Download className="h-4 w-4" strokeWidth={1.9} />}
                  label="Download file"
                  onClick={downloadBlob}
                  tone="muted"
                />
              ) : null}
            </div>
          </div>
          <div className="border-b border-border bg-canvas px-4 py-2 text-xs text-fg-muted">
            {stats ? `${stats.lines} lines (${stats.loc} loc) · ${formatBytes(blob.size)}` : formatBytes(blob.size)}
          </div>
          {blob.is_binary || !blob.text ? (
            <div className="flex min-h-0 flex-1 items-center justify-center p-6">
              <EmptyState title="Binary file" message="This file is binary, so Qit does not render it inline." />
            </div>
          ) : (
            <div className="min-h-0 flex-1 bg-canvas">
              <MonacoCodeSurface height="100%" path={blob.path} value={blob.text} />
            </div>
          )}
        </section>
      </div>
    </div>
  )
}

export function CodePanel({
  entries,
  currentPath,
  activePath,
  blob,
  headerAction,
  latestCommit,
  onBrowse,
  onLoadTreePath,
  onOpen,
  readme,
  treeCache,
}: {
  entries: TreeEntry[]
  currentPath: string
  activePath: string | null
  blob: BlobContent | null
  headerAction?: ReactNode
  latestCommit: CommitHistoryNode | null
  onBrowse: (path: string) => Promise<void>
  onLoadTreePath: (path: string) => Promise<TreeEntry[]>
  onOpen: (entry: TreeEntry) => Promise<void>
  readme: BlobContent | null
  treeCache: Record<string, TreeEntry[]>
}) {
  if (blob) {
    return (
      <BlobCodeView
        blob={blob}
        currentPath={currentPath}
        latestCommit={latestCommit}
        onBrowse={onBrowse}
        onLoadTreePath={onLoadTreePath}
        onOpen={onOpen}
        treeCache={treeCache}
      />
    )
  }

  return (
    <div className="space-y-6">
      <RepoTreeCard
        action={headerAction}
        activePath={activePath}
        currentPath={currentPath}
        entries={entries}
        latestCommit={latestCommit}
        onBrowse={onBrowse}
        onOpen={onOpen}
      />

      {readme ? (
        <section className="overflow-hidden rounded-[var(--radius-lg)] border border-border bg-panel">
          <div className="flex flex-col gap-2 border-b border-border bg-canvas-raised/65 px-4 py-3 sm:flex-row sm:items-center sm:justify-between">
            <div className="flex items-center gap-2">
              <span className="rounded-full border border-border bg-panel-subtle px-2.5 py-1 text-xs font-semibold text-fg">
                README
              </span>
              <span className="text-sm text-fg-muted">{readme.path}</span>
            </div>
            <span className="text-xs text-fg-subtle">Rendered markdown</span>
          </div>
          <div className="bg-panel px-6 py-6">
            <BlobPreview blob={readme} emptyMessage="No README was found for this folder." emptyTitle="README unavailable" />
          </div>
        </section>
      ) : null}
    </div>
  )
}
