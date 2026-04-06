import { useMemo } from 'react'
import { ArrowLeft, ArrowRightLeft, ChevronDown, Pencil, Plus, Trash2 } from 'lucide-react'
import { Badge, Button, EmptyState, Panel, Spinner } from '../../atoms/Controls'
import { buildCommitGraph, type CommitGraphRow } from '../../../lib/commitGraph'
import type { BlobContent, CommitDetail, CommitHistory, CommitHistoryNode, RefDiffFile, TreeEntry } from '../../../lib/types'
import { CodePanel } from './CodePanel'
import { AuthorBadge } from './shared'
import {
  formatDayHeading,
  formatRelativeTime,
  formatTimestamp,
  shortSha,
  toneForRef,
} from './panelUtils'

const GRAPH_COLORS = ['#5b9cff', '#52b36f', '#d7a23a', '#d07ae8', '#80b8ff', '#ef6b6b']
const GRAPH_LANE_WIDTH = 18
const GRAPH_HEIGHT = 78
const GRAPH_PADDING = 16
const GRAPH_MIN_WIDTH = 88

function commitChangeIcon(status: string) {
  const normalized = status.toLowerCase()

  if (normalized.startsWith('a') || normalized.includes('add')) {
    return <Plus className="h-4 w-4 text-success" strokeWidth={1.9} />
  }

  if (normalized.startsWith('d') || normalized.includes('delete') || normalized.includes('remove')) {
    return <Trash2 className="h-4 w-4 text-danger" strokeWidth={1.9} />
  }

  if (normalized.startsWith('r') || normalized.includes('rename') || normalized.includes('copy')) {
    return <ArrowRightLeft className="h-4 w-4 text-accent" strokeWidth={1.9} />
  }

  return <Pencil className="h-4 w-4 text-warning" strokeWidth={1.9} />
}

function laneX(column: number) {
  return GRAPH_PADDING + column * GRAPH_LANE_WIDTH
}

function HistoryGraph({
  commit,
  row,
  laneCount,
  width,
}: {
  commit: CommitHistoryNode
  row: CommitGraphRow
  laneCount: number
  width: number
}) {
  const centerY = GRAPH_HEIGHT / 2
  const centerX = laneX(row.lane)
  const nodeColor = GRAPH_COLORS[row.lane % GRAPH_COLORS.length]
  const mergeCommit = commit.parents.length > 1

  return (
    <svg
      aria-hidden="true"
      className="shrink-0 overflow-visible"
      height={GRAPH_HEIGHT}
      viewBox={`0 0 ${width} ${GRAPH_HEIGHT}`}
      width={width}
    >
      {Array.from({ length: laneCount }).map((_, column) => {
        const active = row.activeBefore.includes(column) || row.activeAfter.includes(column)
        const x = laneX(column)
        return active ? (
          <line
            key={`track-${commit.id}-${column}`}
            stroke={`${GRAPH_COLORS[column % GRAPH_COLORS.length]}18`}
            strokeLinecap="round"
            strokeWidth="6"
            x1={x}
            x2={x}
            y1="6"
            y2={GRAPH_HEIGHT - 6}
          />
        ) : null
      })}
      {row.activeBefore.map((column) => {
        const x = laneX(column)
        return <line key={`before-${commit.id}-${column}`} stroke={GRAPH_COLORS[column % GRAPH_COLORS.length]} strokeWidth="2" x1={x} x2={x} y1="0" y2={centerY} />
      })}
      {row.activeAfter.map((column) => {
        const x = laneX(column)
        return <line key={`after-${commit.id}-${column}`} stroke={GRAPH_COLORS[column % GRAPH_COLORS.length]} strokeWidth="2" x1={x} x2={x} y1={centerY} y2={GRAPH_HEIGHT} />
      })}
      {row.secondaryParentLanes.map((column, index) => {
        const x = laneX(column)
        return (
          <path
            d={`M ${centerX} ${centerY} C ${centerX} ${centerY + 10}, ${x} ${centerY + 10}, ${x} ${GRAPH_HEIGHT}`}
            fill="none"
            key={`parent-${commit.id}-${column}-${index}`}
            stroke={GRAPH_COLORS[column % GRAPH_COLORS.length]}
            strokeWidth="2"
          />
        )
      })}
      {mergeCommit ? (
        <>
          <rect
            fill={nodeColor}
            height="11"
            rx="2"
            transform={`translate(${centerX} ${centerY}) rotate(45) translate(-5.5 -5.5)`}
            width="11"
          />
          <circle cx={centerX} cy={centerY} fill="none" r="10" stroke={`${nodeColor}40`} strokeWidth="2" />
        </>
      ) : (
        <>
          <circle cx={centerX} cy={centerY} fill={nodeColor} r="5.5" />
          <circle cx={centerX} cy={centerY} fill="none" r="8.5" stroke={`${nodeColor}45`} strokeWidth="2" />
        </>
      )}
    </svg>
  )
}

export function CommitsPanel({
  history,
  detail,
  loadingDetail,
  selectedCommitId,
  snapshotActivePath,
  snapshotCommit,
  snapshotTree,
  snapshotPath,
  snapshotReadme,
  snapshotBlob,
  snapshotDiff,
  snapshotTreeCache,
  loadingMore,
  onSelect,
  onBack,
  onBrowseSnapshot,
  onLoadSnapshotTreePath,
  onOpenSnapshotEntry,
  onLoadMore,
}: {
  history: CommitHistory | null
  detail: CommitDetail | null
  loadingDetail: boolean
  selectedCommitId: string | null
  snapshotActivePath: string | null
  snapshotCommit: CommitHistoryNode | null
  snapshotTree: TreeEntry[]
  snapshotPath: string
  snapshotReadme: BlobContent | null
  snapshotBlob: BlobContent | null
  snapshotDiff: RefDiffFile | null
  snapshotTreeCache: Record<string, TreeEntry[]>
  loadingMore: boolean
  onSelect: (commit: string) => void
  onBack: () => void
  onBrowseSnapshot: (path: string) => Promise<void>
  onLoadSnapshotTreePath: (path: string, force?: boolean) => Promise<TreeEntry[]>
  onOpenSnapshotEntry: (entry: TreeEntry) => Promise<void>
  onLoadMore: () => Promise<void>
}) {
  const graph = useMemo(() => buildCommitGraph(history?.commits ?? []), [history])
  const graphRows = useMemo(() => new Map(graph.rows.map((row) => [row.id, row])), [graph.rows])
  const graphWidth = useMemo(
    () => Math.max(GRAPH_MIN_WIDTH, Math.max(graph.laneCount, 1) * GRAPH_LANE_WIDTH + GRAPH_PADDING * 2),
    [graph.laneCount],
  )
  const groupedCommits = useMemo(() => {
    const groups: Array<{ label: string; commits: CommitHistoryNode[] }> = []
    for (const commit of history?.commits ?? []) {
      const label = formatDayHeading(commit.authored_at)
      const current = groups.at(-1)
      if (!current || current.label !== label) {
        groups.push({ label, commits: [commit] })
      } else {
        current.commits.push(commit)
      }
    }
    return groups
  }, [history])

  if (selectedCommitId) {
    if (loadingDetail) {
      return (
        <Panel
          action={
            <Button icon={<ArrowLeft className="h-4 w-4" strokeWidth={1.9} />} onClick={onBack} tone="muted">
              Back to list
            </Button>
          }
          subtitle="Loading the commit metadata and repository snapshot."
          title="Commit"
        >
          <div className="flex items-center gap-3 text-sm text-fg-muted">
            <Spinner />
            <span>Loading commit snapshot…</span>
          </div>
        </Panel>
      )
    }

    if (!detail) {
      return (
        <Panel
          action={
            <Button icon={<ArrowLeft className="h-4 w-4" strokeWidth={1.9} />} onClick={onBack} tone="muted">
              Back to list
            </Button>
          }
          subtitle={selectedCommitId}
          title="Commit"
        >
          <EmptyState title="Unable to load commit" message="This commit could not be resolved for the current session." />
        </Panel>
      )
    }

    return (
      <div className="space-y-6">
        <Panel
          action={
            <Button icon={<ArrowLeft className="h-4 w-4" strokeWidth={1.9} />} onClick={onBack} tone="muted">
              Back to list
            </Button>
          }
          subtitle="Browse this exact commit, inspect its metadata, and explore the repository snapshot captured at that point in history."
          title={detail.summary || detail.id}
        >
          <div className="space-y-6">
            <div className="flex flex-wrap items-center gap-2">
              <Badge tone="accent">{detail.parents.length > 1 ? 'Merge commit' : 'Commit'}</Badge>
              <Badge tone="muted">{shortSha(detail.id)}</Badge>
              <Badge tone="muted">{detail.changes.length} changed files</Badge>
              {detail.parents.length === 0 ? <Badge tone="muted">Root commit</Badge> : null}
            </div>

            <div className="rounded-token border border-border bg-panel-subtle px-4 py-4">
              <pre className="whitespace-pre-wrap text-sm leading-6 text-fg-muted">{detail.message}</pre>
            </div>

            <div className="grid gap-3 md:grid-cols-2 xl:grid-cols-4">
              <div className="rounded-token border border-border bg-panel-subtle px-4 py-3">
                <p className="text-[11px] font-semibold uppercase tracking-[0.14em] text-fg-subtle">Author</p>
                <div className="mt-2">
                  <AuthorBadge name={detail.author} />
                </div>
              </div>
              <div className="rounded-token border border-border bg-panel-subtle px-4 py-3">
                <p className="text-[11px] font-semibold uppercase tracking-[0.14em] text-fg-subtle">Authored at</p>
                <p className="mt-2 text-sm font-medium text-fg">{formatTimestamp(detail.authored_at)}</p>
                <p className="mt-1 text-xs text-fg-muted">{formatRelativeTime(detail.authored_at)}</p>
              </div>
              <div className="rounded-token border border-border bg-panel-subtle px-4 py-3">
                <p className="text-[11px] font-semibold uppercase tracking-[0.14em] text-fg-subtle">Parents</p>
                <p className="mt-2 text-sm font-medium text-fg">
                  {detail.parents.length ? detail.parents.map(shortSha).join(', ') : 'Root commit'}
                </p>
              </div>
              <div className="rounded-token border border-border bg-panel-subtle px-4 py-3">
                <p className="text-[11px] font-semibold uppercase tracking-[0.14em] text-fg-subtle">Commit SHA</p>
                <p className="mt-2 break-all font-mono text-sm text-fg">{detail.id}</p>
              </div>
            </div>
          </div>
        </Panel>

        <Panel subtitle="Open any changed file below to load its commit diff into the editor preview." title="Changed files">
          <div className="space-y-2">
            {detail.changes.length === 0 ? (
              <EmptyState title="No file changes reported" message="This commit does not include changed file stats." />
            ) : (
              detail.changes.map((change) => {
                return (
                  <button
                    className="flex w-full items-center justify-between gap-4 rounded-token border border-border bg-panel-subtle px-4 py-3 text-left text-sm transition hover:border-border-strong hover:bg-panel"
                    key={`${change.path}-${change.status}`}
                    onClick={() =>
                      void onOpenSnapshotEntry({
                        kind: 'blob',
                        name: change.path.split('/').at(-1) ?? change.path,
                        oid: '',
                        path: change.path,
                        size: null,
                      })
                    }
                    type="button"
                  >
                    <div className="flex min-w-0 items-center gap-3">
                      <span className="shrink-0">{commitChangeIcon(change.status)}</span>
                      <div className="min-w-0">
                        <p className="truncate font-medium text-fg">{change.path}</p>
                        <p className="mt-1 text-xs text-fg-muted">{change.status}</p>
                      </div>
                    </div>
                    <div className="shrink-0 text-right text-xs">
                      <p className="text-success">+{change.additions}</p>
                      <p className="mt-1 text-danger">-{change.deletions}</p>
                    </div>
                  </button>
                )
              })
            )}
          </div>
        </Panel>

        <section className="space-y-3">
          <div className="flex flex-col gap-1">
            <h2 className="text-lg font-semibold tracking-tight text-fg">Commit snapshot</h2>
            <p className="max-w-3xl text-sm leading-6 text-fg-muted">
              Browse the repository exactly as it existed at this commit, with changed files opening directly into the diff editor preview.
            </p>
          </div>
          <CodePanel
            activePath={snapshotActivePath}
            blob={snapshotBlob}
            diff={snapshotDiff}
            currentPath={snapshotPath}
            displayMode="editor"
            entries={snapshotTree}
            latestCommit={snapshotCommit}
            rawReference={selectedCommitId}
            onBrowse={onBrowseSnapshot}
            onLoadTreePath={onLoadSnapshotTreePath}
            onOpen={onOpenSnapshotEntry}
            readme={snapshotReadme}
            treeCache={snapshotTreeCache}
          />
        </section>
      </div>
    )
  }

  return (
    <Panel
      action={history ? <Badge tone="accent">{history.reference}</Badge> : undefined}
      subtitle="Trace the published history with stronger branch and merge cues, then open any commit as its own snapshot page."
      title="Commit history"
    >
      {!history || history.commits.length === 0 ? (
        <EmptyState title="No commit history yet" message="This reference does not have commits to inspect yet." />
      ) : (
        <div className="space-y-6">
          {groupedCommits.map((group) => (
            <div className="space-y-2" key={group.label}>
              <div className="flex items-center gap-3 px-1">
                <p className="text-xs font-semibold uppercase tracking-[0.14em] text-fg-subtle">{group.label}</p>
                <div className="h-px flex-1 bg-border" />
              </div>
              {group.commits.map((commit) => {
                const row = graphRows.get(commit.id)
                return (
                  <button
                    className="w-full rounded-token border border-border bg-panel-subtle px-4 text-left transition hover:border-border-strong hover:bg-panel"
                    key={commit.id}
                    onClick={() => onSelect(commit.id)}
                    type="button"
                  >
                    <div className="grid min-h-19 grid-cols-[auto_minmax(0,1fr)] gap-4">
                      <div className="flex items-center py-2">
                        {row ? <HistoryGraph commit={commit} laneCount={graph.laneCount} row={row} width={graphWidth} /> : null}
                      </div>
                      <div className="grid min-h-19 grid-cols-[minmax(0,1fr)_auto] gap-4 py-4">
                        <div className="min-w-0">
                          <div className="flex flex-wrap items-start gap-2">
                            <p className="line-clamp-2 min-w-0 flex-1 text-sm font-semibold leading-6 text-fg">
                              {commit.summary || commit.id}
                            </p>
                            {commit.parents.length > 1 ? <Badge tone="accent">Merge</Badge> : null}
                          </div>
                          <div className="mt-2 flex flex-wrap items-center gap-x-4 gap-y-2">
                            <AuthorBadge name={commit.author} />
                            <span className="text-xs text-fg-muted">{formatRelativeTime(commit.authored_at)}</span>
                            <span className="text-xs text-fg-subtle">{formatTimestamp(commit.authored_at)}</span>
                          </div>
                        </div>
                        <div className="flex min-w-0 flex-col items-end justify-between gap-2">
                          <span className="rounded-full border border-border bg-canvas px-2 py-0.5 font-mono text-[11px] text-fg-subtle">
                            {shortSha(commit.id)}
                          </span>
                          <div className="flex max-w-xs flex-wrap justify-end gap-2">
                            {commit.refs.map((ref) => (
                              <Badge key={`${commit.id}-${ref.name}`} tone={toneForRef(ref)}>
                                {ref.name}
                              </Badge>
                            ))}
                          </div>
                        </div>
                      </div>
                    </div>
                  </button>
                )
              })}
            </div>
          ))}

          {history.has_more ? (
            <div className="flex justify-center pt-2">
              <Button disabled={loadingMore} icon={<ChevronDown className="h-4 w-4" strokeWidth={1.85} />} onClick={() => void onLoadMore()} tone="muted">
                {loadingMore ? 'Loading older commits…' : 'Load older commits'}
              </Button>
            </div>
          ) : null}
        </div>
      )}
    </Panel>
  )
}
