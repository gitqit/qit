import { useMemo } from 'react'
import { ArrowRightLeft, ChevronDown, Pencil, Plus, Trash2 } from 'lucide-react'
import { Badge, Button, EmptyState, Panel } from '../../atoms/Controls'
import { KeyValueRow } from '../../molecules/Fields'
import { buildCommitGraph } from '../../../lib/commitGraph'
import type { CommitDetail, CommitHistory, CommitHistoryNode } from '../../../lib/types'
import { AuthorBadge } from './shared'
import {
  formatDayHeading,
  formatRelativeTime,
  formatTimestamp,
  shortSha,
  toneForRef,
} from './panelUtils'

const GRAPH_COLORS = ['#5b9cff', '#52b36f', '#d7a23a', '#d07ae8', '#80b8ff', '#ef6b6b']

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

function HistoryGraph({
  commit,
  lane,
  laneCount,
  activeBefore,
  activeAfter,
  parentLanes,
}: {
  commit: CommitHistoryNode
  lane: number
  laneCount: number
  activeBefore: number[]
  activeAfter: number[]
  parentLanes: number[]
}) {
  const laneWidth = 18
  const width = Math.max(laneCount, 1) * laneWidth + 12
  const height = 56
  const centerY = 28
  const centerX = 12 + lane * laneWidth
  const nodeColor = GRAPH_COLORS[lane % GRAPH_COLORS.length]

  return (
    <svg
      aria-hidden="true"
      className="mt-0.5 shrink-0 overflow-visible"
      height={height}
      viewBox={`0 0 ${width} ${height}`}
      width={width}
    >
      {activeBefore.map((column) => {
        const x = 12 + column * laneWidth
        return <line key={`before-${commit.id}-${column}`} stroke={GRAPH_COLORS[column % GRAPH_COLORS.length]} strokeWidth="2" x1={x} x2={x} y1="0" y2={centerY} />
      })}
      {activeAfter.map((column) => {
        const x = 12 + column * laneWidth
        return <line key={`after-${commit.id}-${column}`} stroke={GRAPH_COLORS[column % GRAPH_COLORS.length]} strokeWidth="2" x1={x} x2={x} y1={centerY} y2={height} />
      })}
      {parentLanes.map((column, index) => {
        const x = 12 + column * laneWidth
        return (
          <path
            d={`M ${centerX} ${centerY} Q ${centerX} ${centerY + 8} ${x} ${height}`}
            fill="none"
            key={`parent-${commit.id}-${column}-${index}`}
            stroke={GRAPH_COLORS[column % GRAPH_COLORS.length]}
            strokeWidth="2"
          />
        )
      })}
      <circle cx={centerX} cy={centerY} fill={nodeColor} r="5.5" />
      <circle cx={centerX} cy={centerY} fill="none" r="8" stroke={`${nodeColor}55`} strokeWidth="2" />
    </svg>
  )
}

export function CommitsPanel({
  history,
  detail,
  selectedCommitId,
  loadingMore,
  onSelect,
  onLoadMore,
}: {
  history: CommitHistory | null
  detail: CommitDetail | null
  selectedCommitId: string | null
  loadingMore: boolean
  onSelect: (commit: string) => void
  onLoadMore: () => Promise<void>
}) {
  const graph = useMemo(() => buildCommitGraph(history?.commits ?? []), [history])
  const graphRows = useMemo(() => new Map(graph.rows.map((row) => [row.id, row])), [graph.rows])
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

  return (
    <div className="grid gap-6 xl:grid-cols-[minmax(0,1.2fr)_minmax(18rem,0.8fr)]">
      <Panel
        action={history ? <Badge tone="accent">{history.reference}</Badge> : undefined}
        subtitle="Trace how the published snapshot evolved, with branch markers and commit details kept close together."
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
                  const selected = selectedCommitId === commit.id
                  return (
                    <button
                      className={`w-full rounded-token border px-4 py-3 text-left transition ${
                        selected
                          ? 'border-accent/45 bg-accent/10 shadow-[inset_0_0_0_1px_rgba(91,156,255,0.12)]'
                          : 'border-border bg-panel-subtle hover:border-border-strong hover:bg-panel'
                      }`}
                      key={commit.id}
                      onClick={() => onSelect(commit.id)}
                      type="button"
                    >
                      <div className="flex gap-3">
                        {row ? (
                          <HistoryGraph
                            activeAfter={row.activeAfter}
                            activeBefore={row.activeBefore}
                            commit={commit}
                            lane={row.lane}
                            laneCount={graph.laneCount}
                            parentLanes={row.parentLanes}
                          />
                        ) : null}
                        <div className="min-w-0 flex-1">
                          <div className="flex flex-wrap items-center gap-2">
                            <p className="min-w-0 flex-1 truncate text-sm font-semibold text-fg">
                              {commit.summary || commit.id}
                            </p>
                            <span className="rounded-full border border-border bg-canvas px-2 py-0.5 font-mono text-[11px] text-fg-subtle">
                              {shortSha(commit.id)}
                            </span>
                            {commit.refs.map((ref) => (
                              <Badge key={`${commit.id}-${ref.name}`} tone={toneForRef(ref)}>
                                {ref.name}
                              </Badge>
                            ))}
                          </div>
                          <div className="mt-2 flex flex-wrap items-center gap-x-4 gap-y-2">
                            <AuthorBadge name={commit.author} />
                            <span className="text-xs text-fg-muted">{formatRelativeTime(commit.authored_at)}</span>
                            <span className="text-xs text-fg-subtle">{formatTimestamp(commit.authored_at)}</span>
                            {commit.parents.length > 1 ? <Badge tone="muted">Merge</Badge> : null}
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

      <Panel
        subtitle={detail ? detail.id : 'Choose a commit to inspect its author, message, and changed files.'}
        title={detail?.summary ?? 'Commit detail'}
      >
        {!detail ? (
          <EmptyState title="No commit selected" message="Choose a commit from the list to inspect its message and file changes." />
        ) : (
          <div className="space-y-6">
            <div className="rounded-token border border-border bg-panel-subtle px-4 py-4">
              <div className="flex flex-wrap items-center gap-2">
                <Badge tone="muted">{shortSha(detail.id)}</Badge>
                <Badge tone="accent">{detail.parents.length > 1 ? 'Merge commit' : 'Commit'}</Badge>
                <Badge tone="muted">{detail.changes.length} changed files</Badge>
              </div>
              <pre className="mt-4 whitespace-pre-wrap text-sm leading-6 text-fg-muted">{detail.message}</pre>
            </div>

            <div className="rounded-token border border-border bg-panel-subtle px-4 py-2">
              <KeyValueRow label="Author" value={detail.author} />
              <KeyValueRow label="Authored at" value={formatTimestamp(detail.authored_at)} />
              <KeyValueRow label="Parents" value={detail.parents.length ? detail.parents.map(shortSha).join(', ') : 'Root commit'} />
            </div>

            <div className="space-y-2">
              {detail.changes.map((change) => (
                <div className="flex items-center justify-between gap-4 rounded-token border border-border bg-panel-subtle px-4 py-3 text-sm" key={`${change.path}-${change.status}`}>
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
                </div>
              ))}
              {detail.changes.length === 0 ? (
                <EmptyState title="No file changes reported" message="This commit does not include changed file stats." />
              ) : null}
            </div>
          </div>
        )}
      </Panel>
    </div>
  )
}
