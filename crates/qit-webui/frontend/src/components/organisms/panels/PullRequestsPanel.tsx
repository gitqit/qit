import { useEffect, useMemo, useState } from 'react'
import {
  ArrowLeft,
  FileDiff,
  GitCommitHorizontal,
  GitMerge,
  GitPullRequestCreateArrow,
  Search,
} from 'lucide-react'
import { api } from '../../../lib/api'
import type {
  PullRequestDetailResponse,
  PullRequestRecord,
  PullRequestStatus,
  RefDiffFile,
} from '../../../lib/types'
import { Badge, Button, EmptyState, Panel, Spinner } from '../../atoms/Controls'
import { MonacoDiffSurface } from '../MonacoCodeSurface'
import { shortSha } from './panelUtils'

const STATUS_OPTIONS: Array<{ id: 'all' | PullRequestStatus; label: string }> = [
  { id: 'all', label: 'All' },
  { id: 'open', label: 'Open' },
  { id: 'merged', label: 'Merged' },
  { id: 'closed', label: 'Closed' },
]

function statusTone(status: PullRequestStatus) {
  return status === 'merged' ? 'success' : status === 'closed' ? 'muted' : 'accent'
}

function formatDate(value: number) {
  return new Date(value).toLocaleString()
}

function formatRelativeTime(value: number) {
  const deltaMs = Date.now() - value
  const minutes = Math.max(1, Math.round(deltaMs / 60000))
  if (minutes < 60) return `${minutes}m ago`
  const hours = Math.round(minutes / 60)
  if (hours < 24) return `${hours}h ago`
  const days = Math.round(hours / 24)
  return `${days}d ago`
}

function diffHeight(file: RefDiffFile) {
  const lineCount = Math.max(
    file.original?.text?.split('\n').length ?? 1,
    file.modified?.text?.split('\n').length ?? 1,
  )
  return Math.min(840, Math.max(220, lineCount * 20 + 56))
}

function PullRequestList({
  pullRequests,
  highlightedPullRequestId,
  canMerge,
  canCreate,
  onCreate,
  onMerge,
  onSelect,
}: {
  pullRequests: PullRequestRecord[]
  highlightedPullRequestId: string | null
  canMerge: boolean
  canCreate: boolean
  onCreate: () => void
  onMerge: (id: string) => Promise<void>
  onSelect: (id: string) => void
}) {
  const [search, setSearch] = useState('')
  const [statusFilter, setStatusFilter] = useState<'all' | PullRequestStatus>('all')

  useEffect(() => {
    if (!highlightedPullRequestId) {
      return
    }

    const card = document.getElementById(`pull-request-${highlightedPullRequestId}`)
    card?.scrollIntoView({ behavior: 'smooth', block: 'nearest' })
  }, [highlightedPullRequestId, pullRequests.length])

  const filteredPullRequests = useMemo(() => {
    const needle = search.trim().toLowerCase()
    return pullRequests.filter((pullRequest) => {
      const matchesStatus = statusFilter === 'all' || pullRequest.status === statusFilter
      if (!matchesStatus) {
        return false
      }
      if (!needle) {
        return true
      }
      const haystack = [
        pullRequest.title,
        pullRequest.description,
        pullRequest.source_branch,
        pullRequest.target_branch,
      ]
        .join(' ')
        .toLowerCase()
      return haystack.includes(needle)
    })
  }, [pullRequests, search, statusFilter])

  return (
    <Panel
      action={
        canCreate ? (
          <Button
            icon={<GitPullRequestCreateArrow className="h-4 w-4" strokeWidth={1.9} />}
            onClick={onCreate}
            title="Open a pull request"
          >
            Open pull request
          </Button>
        ) : null
      }
      subtitle="Filter by state, search by branch or title, and open any pull request for full review."
      title="Pull requests"
    >
      <div className="space-y-4">
        <div className="flex flex-col gap-3 lg:flex-row lg:items-center lg:justify-between">
          <div className="relative max-w-xl flex-1">
            <Search
              aria-hidden="true"
              className="pointer-events-none absolute left-3 top-1/2 h-4 w-4 -translate-y-1/2 text-fg-subtle"
              strokeWidth={1.9}
            />
            <input
              className="w-full rounded-token border border-border bg-panel-subtle py-2.5 pl-10 pr-3.5 text-sm text-fg outline-none transition placeholder:text-fg-subtle focus:border-accent focus:ring-2 focus:ring-accent/20"
              onChange={(event) => setSearch(event.target.value)}
              placeholder="Search pull requests"
              type="search"
              value={search}
            />
          </div>
          <div className="flex flex-wrap gap-2">
            {STATUS_OPTIONS.map((option) => (
              <button
                className={`rounded-full border px-3 py-1.5 text-sm font-medium transition ${
                  statusFilter === option.id
                    ? 'border-accent/40 bg-accent/10 text-fg'
                    : 'border-border bg-panel-subtle text-fg-muted hover:border-border-strong hover:text-fg'
                }`}
                key={option.id}
                onClick={() => setStatusFilter(option.id)}
                type="button"
              >
                {option.label}
              </button>
            ))}
          </div>
        </div>

        {pullRequests.length === 0 ? (
          <EmptyState
            title="No pull requests yet"
            message="Open a branch comparison when you are ready to package a change for review."
          />
        ) : filteredPullRequests.length === 0 ? (
          <EmptyState
            title="No matching pull requests"
            message="Try a different search or widen the state filter to see more results."
          />
        ) : (
          <div className="space-y-3">
            {filteredPullRequests.map((pullRequest) => {
              const isHighlighted = pullRequest.id === highlightedPullRequestId
              return (
                <button
                  className={`w-full rounded-token border px-4 py-4 text-left transition ${
                    isHighlighted
                      ? 'border-accent/35 bg-panel'
                      : 'border-border bg-panel-subtle hover:border-border-strong hover:bg-panel'
                  }`}
                  id={`pull-request-${pullRequest.id}`}
                  key={pullRequest.id}
                  onClick={() => onSelect(pullRequest.id)}
                  type="button"
                >
                  <div className="flex flex-wrap items-start justify-between gap-4">
                    <div className="min-w-0 flex-1">
                      <div className="flex flex-wrap items-center gap-2">
                        <h3 className="text-base font-semibold text-fg">{pullRequest.title}</h3>
                        <Badge tone={statusTone(pullRequest.status)}>{pullRequest.status}</Badge>
                      </div>
                      <p className="mt-1 text-sm text-fg-muted">
                        {pullRequest.source_branch} to {pullRequest.target_branch}
                      </p>
                      {pullRequest.description ? (
                        <p className="mt-3 line-clamp-2 text-sm leading-6 text-fg-muted">
                          {pullRequest.description}
                        </p>
                      ) : null}
                    </div>
                    {pullRequest.status === 'open' && canMerge ? (
                      <Button
                        icon={<GitMerge className="h-4 w-4" strokeWidth={1.9} />}
                        onClick={(event) => {
                          event.stopPropagation()
                          void onMerge(pullRequest.id)
                        }}
                        tone="muted"
                      >
                        Merge
                      </Button>
                    ) : null}
                  </div>
                  <div className="mt-4 flex flex-wrap items-center gap-3 text-xs text-fg-subtle">
                    <span>{formatRelativeTime(pullRequest.updated_at_ms)}</span>
                    <span>Updated {formatDate(pullRequest.updated_at_ms)}</span>
                  </div>
                </button>
              )
            })}
          </div>
        )}
      </div>
    </Panel>
  )
}

function PullRequestDetail({
  pullRequestId,
  canMerge,
  onBack,
  onMerge,
}: {
  pullRequestId: string
  canMerge: boolean
  onBack: () => void
  onMerge: (id: string) => Promise<void>
}) {
  const [detail, setDetail] = useState<PullRequestDetailResponse | null>(null)
  const [loading, setLoading] = useState(true)
  const [loadError, setLoadError] = useState<string | null>(null)
  const [actionError, setActionError] = useState<string | null>(null)
  const [merging, setMerging] = useState(false)

  useEffect(() => {
    let active = true
    setLoading(true)
    setLoadError(null)
    setActionError(null)
    setDetail(null)

    void api
      .pullRequest(pullRequestId)
      .then((response) => {
        if (active) {
          setDetail(response)
        }
      })
      .catch((loadError) => {
        if (active) {
          setLoadError(loadError instanceof Error ? loadError.message : 'Failed to load the pull request.')
        }
      })
      .finally(() => {
        if (active) {
          setLoading(false)
        }
      })

    return () => {
      active = false
    }
  }, [pullRequestId])

  if (loading) {
    return (
      <Panel subtitle="Loading pull request details and diffs." title="Pull request">
        <div className="flex items-center gap-3 text-sm text-fg-muted">
          <Spinner />
          <span>Loading pull request…</span>
        </div>
      </Panel>
    )
  }

  if (loadError || !detail) {
    return (
      <Panel subtitle="This pull request could not be loaded right now." title="Pull request">
        <div className="space-y-4">
          <Button
            icon={<ArrowLeft className="h-4 w-4" strokeWidth={1.9} />}
            onClick={onBack}
            tone="muted"
          >
            Back to list
          </Button>
          <EmptyState
            title="Unable to load pull request"
            message={loadError ?? 'The pull request is unavailable.'}
          />
        </div>
      </Panel>
    )
  }

  const { pull_request: pullRequest, comparison, diffs } = detail

  return (
    <div className="space-y-6">
      <Panel
        action={
          <Button
            icon={<ArrowLeft className="h-4 w-4" strokeWidth={1.9} />}
            onClick={onBack}
            tone="muted"
          >
            Back to list
          </Button>
        }
        subtitle="Review the pull request summary, commit range, and file-level changes in one place."
        title={pullRequest.title}
      >
        <div className="space-y-6">
          <div className="flex flex-wrap items-center gap-2">
            <Badge tone={statusTone(pullRequest.status)}>{pullRequest.status}</Badge>
            <Badge tone="muted">{pullRequest.source_branch}</Badge>
            <span className="text-xs text-fg-subtle">to</span>
            <Badge tone="muted">{pullRequest.target_branch}</Badge>
            {pullRequest.merged_commit ? (
              <Badge tone="success">Merged at {shortSha(pullRequest.merged_commit)}</Badge>
            ) : null}
          </div>

          {pullRequest.description ? (
            <p className="max-w-4xl text-sm leading-7 text-fg-muted">{pullRequest.description}</p>
          ) : (
            <p className="text-sm text-fg-subtle">No description was added to this pull request.</p>
          )}

          <div className="grid gap-3 md:grid-cols-4">
            <div className="rounded-token border border-border bg-panel-subtle px-4 py-3">
              <p className="text-[11px] font-semibold uppercase tracking-[0.14em] text-fg-subtle">Updated</p>
              <p className="mt-1 text-sm font-medium text-fg">{formatDate(pullRequest.updated_at_ms)}</p>
            </div>
            <div className="rounded-token border border-border bg-panel-subtle px-4 py-3">
              <p className="text-[11px] font-semibold uppercase tracking-[0.14em] text-fg-subtle">Created</p>
              <p className="mt-1 text-sm font-medium text-fg">{formatDate(pullRequest.created_at_ms)}</p>
            </div>
            <div className="rounded-token border border-border bg-panel-subtle px-4 py-3">
              <p className="text-[11px] font-semibold uppercase tracking-[0.14em] text-fg-subtle">Ahead</p>
              <p className="mt-1 text-lg font-semibold text-fg">{comparison?.ahead_by ?? 0}</p>
            </div>
            <div className="rounded-token border border-border bg-panel-subtle px-4 py-3">
              <p className="text-[11px] font-semibold uppercase tracking-[0.14em] text-fg-subtle">Behind</p>
              <p className="mt-1 text-lg font-semibold text-fg">{comparison?.behind_by ?? 0}</p>
            </div>
          </div>

          {pullRequest.status === 'open' && canMerge ? (
            <div className="flex flex-wrap items-center gap-3">
              <Button
                icon={merging ? <Spinner /> : <GitMerge className="h-4 w-4" strokeWidth={1.9} />}
                onClick={async () => {
                  setMerging(true)
                  setActionError(null)
                  try {
                    await onMerge(pullRequest.id)
                    setDetail(await api.pullRequest(pullRequest.id))
                  } catch (mergeError) {
                    setActionError(
                      mergeError instanceof Error ? mergeError.message : 'Failed to merge pull request.',
                    )
                  } finally {
                    setMerging(false)
                  }
                }}
              >
                {merging ? 'Merging...' : 'Merge pull request'}
              </Button>
              {actionError ? <p className="text-sm text-danger">{actionError}</p> : null}
            </div>
          ) : null}
        </div>
      </Panel>

      <Panel
        subtitle="Commits currently present on the source branch and not yet in the target branch."
        title="Commit range"
      >
        {!comparison ? (
          <EmptyState
            title="No comparison available"
            message="Qit could not load the live branch comparison for this pull request."
          />
        ) : comparison.commits.length === 0 ? (
          <EmptyState
            title="No commits in range"
            message="These branches currently resolve to the same commit set."
          />
        ) : (
          <div className="space-y-3">
            {comparison.commits.map((commit) => (
              <div
                className="rounded-token border border-border bg-panel-subtle px-4 py-3"
                key={commit.id}
              >
                <div className="flex flex-wrap items-center justify-between gap-3">
                  <div className="min-w-0">
                    <div className="flex items-center gap-2">
                      <GitCommitHorizontal className="h-4 w-4 text-accent" strokeWidth={1.9} />
                      <p className="truncate text-sm font-medium text-fg">
                        {commit.summary || commit.id}
                      </p>
                    </div>
                    <p className="mt-1 text-xs text-fg-muted">{commit.author}</p>
                  </div>
                  <span className="font-mono text-xs text-fg-subtle">{shortSha(commit.id)}</span>
                </div>
              </div>
            ))}
          </div>
        )}
      </Panel>

      <Panel
        subtitle="Every changed file is shown with a side-by-side Monaco diff editor."
        title="Diffs"
      >
        {!diffs ? (
          <EmptyState
            title="No diffs available"
            message="Qit could not load file-level diffs for this pull request."
          />
        ) : diffs.length === 0 ? (
          <EmptyState
            title="No file changes"
            message="This pull request does not currently change any files."
          />
        ) : (
          <div className="space-y-5">
            {diffs.map((file) => {
              const isBinary = file.original?.is_binary || file.modified?.is_binary
              return (
                <section className="overflow-hidden rounded-token border border-border" key={`${file.previous_path ?? ''}-${file.path}`}>
                  <div className="border-b border-border bg-canvas-raised/65 px-4 py-3">
                    <div className="flex flex-wrap items-center gap-2">
                      <FileDiff className="h-4 w-4 text-accent" strokeWidth={1.9} />
                      <p className="font-mono text-sm text-fg">{file.path}</p>
                      <Badge tone="muted">{file.status}</Badge>
                      <Badge tone="success">+{file.additions}</Badge>
                      <Badge tone="danger">-{file.deletions}</Badge>
                    </div>
                    {file.previous_path ? (
                      <p className="mt-2 text-xs text-fg-subtle">Renamed from {file.previous_path}</p>
                    ) : null}
                  </div>
                  <div className="bg-panel">
                    {isBinary ? (
                      <div className="px-4 py-5 text-sm text-fg-muted">
                        Binary file diff preview is not available.
                      </div>
                    ) : (
                      <MonacoDiffSurface
                        height={diffHeight(file)}
                        modified={file.modified?.text ?? ''}
                        original={file.original?.text ?? ''}
                        path={file.path}
                      />
                    )}
                  </div>
                </section>
              )
            })}
          </div>
        )}
      </Panel>
    </div>
  )
}

export function PullRequestsPanel({
  pullRequests,
  highlightedPullRequestId,
  selectedPullRequestId,
  canMerge,
  canCreate,
  onBack,
  onCreate,
  onMerge,
  onSelect,
}: {
  pullRequests: PullRequestRecord[]
  highlightedPullRequestId: string | null
  selectedPullRequestId: string | null
  canMerge: boolean
  canCreate: boolean
  onBack: () => void
  onCreate: () => void
  onMerge: (id: string) => Promise<void>
  onSelect: (id: string) => void
}) {
  if (selectedPullRequestId) {
    return (
      <PullRequestDetail
        canMerge={canMerge}
        onBack={onBack}
        onMerge={onMerge}
        pullRequestId={selectedPullRequestId}
      />
    )
  }

  return (
    <PullRequestList
      canCreate={canCreate}
      canMerge={canMerge}
      highlightedPullRequestId={highlightedPullRequestId}
      onCreate={onCreate}
      onMerge={onMerge}
      onSelect={onSelect}
      pullRequests={pullRequests}
    />
  )
}
