import { useCallback, useEffect, useMemo, useState } from 'react'
import {
  ArrowLeft,
  FileDiff,
  GitCommitHorizontal,
  GitMerge,
  GitPullRequestCreateArrow,
  MessageSquareMore,
  PencilLine,
  RotateCcw,
  Save,
  Search,
  ThumbsDown,
  ThumbsUp,
  Trash2,
  XCircle,
} from 'lucide-react'
import { api } from '../../../lib/api'
import { usePersistentDisplayName } from '../../../lib/usePersistentDisplayName'
import type {
  PullRequestActivity,
  PullRequestDetailResponse,
  PullRequestRecord,
  PullRequestReviewState,
  PullRequestStatus,
  RefDiffFile,
  UiRole,
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

function actorLabel(actorRole: UiRole, displayName?: string | null) {
  return displayName?.trim() || (actorRole === 'owner' ? 'Owner' : 'Viewer')
}

function reviewTone(state: PullRequestReviewState) {
  return state === 'approved'
    ? 'success'
    : state === 'changes_requested'
      ? 'danger'
      : 'muted'
}

function reviewLabel(state: PullRequestReviewState) {
  return state === 'changes_requested'
    ? 'Changes requested'
    : state === 'approved'
      ? 'Approved'
      : 'Commented'
}

function activityLabel(activity: PullRequestActivity) {
  switch (activity.kind) {
    case 'opened':
      return 'opened this pull request'
    case 'commented':
      return 'left a comment'
    case 'reviewed':
      return activity.review_state ? reviewLabel(activity.review_state).toLowerCase() : 'reviewed'
    case 'edited':
      return 'edited the pull request details'
    case 'closed':
      return 'closed this pull request'
    case 'reopened':
      return 'reopened this pull request'
    case 'merged':
      return 'merged this pull request'
  }
}

function PullRequestList({
  pullRequests,
  highlightedPullRequestId,
  canManage,
  canCreate,
  onCreate,
  onMerge,
  onSelect,
}: {
  pullRequests: PullRequestRecord[]
  highlightedPullRequestId: string | null
  canManage: boolean
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
                    {pullRequest.status === 'open' && canManage ? (
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
  actor,
  pullRequestId,
  canManage,
  onBack,
  onComment,
  onDelete,
  onMerge,
  onReview,
  onUpdate,
}: {
  actor: UiRole
  pullRequestId: string
  canManage: boolean
  onBack: () => void
  onComment: (id: string, payload: { display_name: string; body: string }) => Promise<PullRequestRecord>
  onDelete: (id: string) => Promise<PullRequestRecord>
  onMerge: (id: string) => Promise<void>
  onReview: (
    id: string,
    payload: { display_name: string; body: string; state: PullRequestReviewState },
  ) => Promise<PullRequestRecord>
  onUpdate: (
    id: string,
    payload: { title?: string; description?: string; status?: 'open' | 'closed' },
  ) => Promise<PullRequestRecord>
}) {
  const [detail, setDetail] = useState<PullRequestDetailResponse | null>(null)
  const [loading, setLoading] = useState(true)
  const [loadError, setLoadError] = useState<string | null>(null)
  const [actionError, setActionError] = useState<string | null>(null)
  const [pendingAction, setPendingAction] = useState<string | null>(null)
  const [isEditing, setIsEditing] = useState(false)
  const [titleDraft, setTitleDraft] = useState('')
  const [descriptionDraft, setDescriptionDraft] = useState('')
  const [commentBody, setCommentBody] = useState('')
  const [reviewBody, setReviewBody] = useState('')
  const [displayName, setDisplayName] = usePersistentDisplayName()

  const loadDetail = useCallback(async () => {
    setLoading(true)
    setLoadError(null)
    setActionError(null)
    try {
      const response = await api.pullRequest(pullRequestId)
      setDetail(response)
      setTitleDraft(response.pull_request.title)
      setDescriptionDraft(response.pull_request.description)
      setIsEditing(false)
    } catch (nextError) {
      setLoadError(nextError instanceof Error ? nextError.message : 'Failed to load the pull request.')
    } finally {
      setLoading(false)
    }
  }, [pullRequestId])

  useEffect(() => {
    void loadDetail()
  }, [loadDetail])

  const runAction = useCallback(
    async (label: string, action: () => Promise<void>, reload = true) => {
      setPendingAction(label)
      setActionError(null)
      try {
        await action()
        if (reload) {
          await loadDetail()
        }
      } catch (nextError) {
        setActionError(nextError instanceof Error ? nextError.message : 'Pull request action failed.')
      } finally {
        setPendingAction(null)
      }
    },
    [loadDetail],
  )

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

  const { pull_request: pullRequest, comparison, diffs, comments, reviews, review_summary: reviewSummary, activity } = detail
  const isBusy = pendingAction !== null
  const reversedActivity = [...activity].reverse()

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
        title={isEditing ? 'Edit pull request' : pullRequest.title}
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

          {canManage ? (
            <div className="space-y-4 rounded-token border border-border bg-panel-subtle px-4 py-4">
              <div className="flex flex-wrap items-center justify-between gap-3">
                <div>
                  <p className="text-sm font-semibold text-fg">Pull request details</p>
                  <p className="text-sm text-fg-muted">Owners can refine the title, description, and status.</p>
                </div>
                <Button
                  icon={<PencilLine className="h-4 w-4" strokeWidth={1.9} />}
                  onClick={() => {
                    if (isEditing) {
                      setTitleDraft(pullRequest.title)
                      setDescriptionDraft(pullRequest.description)
                    }
                    setIsEditing((current) => !current)
                  }}
                  tone="muted"
                >
                  {isEditing ? 'Cancel edit' : 'Edit details'}
                </Button>
              </div>
              {isEditing ? (
                <div className="space-y-4">
                  <input
                    className="w-full rounded-token border border-border bg-panel px-3.5 py-2.5 text-sm text-fg outline-none transition placeholder:text-fg-subtle focus:border-accent focus:ring-2 focus:ring-accent/20"
                    onChange={(event) => setTitleDraft(event.target.value)}
                    placeholder="Pull request title"
                    value={titleDraft}
                  />
                  <textarea
                    className="min-h-28 w-full rounded-token border border-border bg-panel px-3.5 py-3 text-sm text-fg outline-none transition placeholder:text-fg-subtle focus:border-accent focus:ring-2 focus:ring-accent/20"
                    onChange={(event) => setDescriptionDraft(event.target.value)}
                    placeholder="Describe this pull request"
                    value={descriptionDraft}
                  />
                  <div className="flex flex-wrap items-center gap-3">
                    <Button
                      disabled={isBusy}
                      icon={pendingAction === 'save' ? <Spinner /> : <Save className="h-4 w-4" strokeWidth={1.9} />}
                      onClick={() =>
                        void runAction('save', async () => {
                          await onUpdate(pullRequest.id, {
                            title: titleDraft,
                            description: descriptionDraft,
                          })
                        })
                      }
                    >
                      {pendingAction === 'save' ? 'Saving...' : 'Save details'}
                    </Button>
                    {actionError ? <p className="text-sm text-danger">{actionError}</p> : null}
                  </div>
                </div>
              ) : pullRequest.description ? (
                <p className="max-w-4xl text-sm leading-7 text-fg-muted">{pullRequest.description}</p>
              ) : (
                <p className="text-sm text-fg-subtle">No description was added to this pull request.</p>
              )}
            </div>
          ) : pullRequest.description ? (
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

          <div className="flex flex-wrap items-center gap-3">
            {pullRequest.status === 'open' && canManage ? (
              <>
                <Button
                  disabled={isBusy}
                  icon={pendingAction === 'merge' ? <Spinner /> : <GitMerge className="h-4 w-4" strokeWidth={1.9} />}
                  onClick={() =>
                    void runAction('merge', async () => {
                      await onMerge(pullRequest.id)
                    })
                  }
                >
                  {pendingAction === 'merge' ? 'Merging...' : 'Merge pull request'}
                </Button>
                <Button
                  disabled={isBusy}
                  icon={pendingAction === 'close' ? <Spinner /> : <XCircle className="h-4 w-4" strokeWidth={1.9} />}
                  onClick={() =>
                    void runAction('close', async () => {
                      await onUpdate(pullRequest.id, { status: 'closed' })
                    })
                  }
                  tone="muted"
                >
                  {pendingAction === 'close' ? 'Closing...' : 'Close'}
                </Button>
              </>
            ) : null}
            {pullRequest.status === 'closed' && canManage ? (
              <Button
                disabled={isBusy}
                icon={pendingAction === 'reopen' ? <Spinner /> : <RotateCcw className="h-4 w-4" strokeWidth={1.9} />}
                onClick={() =>
                  void runAction('reopen', async () => {
                    await onUpdate(pullRequest.id, { status: 'open' })
                  })
                }
                tone="muted"
              >
                {pendingAction === 'reopen' ? 'Reopening...' : 'Reopen'}
              </Button>
            ) : null}
            {canManage ? (
              <Button
                disabled={isBusy}
                icon={pendingAction === 'delete' ? <Spinner /> : <Trash2 className="h-4 w-4" strokeWidth={1.9} />}
                onClick={() =>
                  void runAction(
                    'delete',
                    async () => {
                      await onDelete(pullRequest.id)
                      onBack()
                    },
                    false,
                  )
                }
                tone="danger"
              >
                {pendingAction === 'delete' ? 'Deleting...' : 'Delete'}
              </Button>
            ) : null}
            {actionError ? <p className="text-sm text-danger">{actionError}</p> : null}
          </div>
        </div>
      </Panel>

      <Panel
        subtitle="Leave a discussion comment or a formal review, and keep your preferred display name in this browser."
        title="Discussion and review"
      >
        <div className="space-y-6">
          <div className="grid gap-4 lg:grid-cols-[minmax(0,1fr)_280px]">
            <div className="space-y-4 rounded-token border border-border bg-panel-subtle px-4 py-4">
              <div className="grid gap-3 sm:grid-cols-2">
                <label className="space-y-2">
                  <span className="text-sm font-medium text-fg">Display name</span>
                  <input
                    className="w-full rounded-token border border-border bg-panel px-3.5 py-2.5 text-sm text-fg outline-none transition placeholder:text-fg-subtle focus:border-accent focus:ring-2 focus:ring-accent/20"
                    onChange={(event) => setDisplayName(event.target.value)}
                    placeholder={actor === 'owner' ? 'Owner display name' : 'Viewer display name'}
                    value={displayName}
                  />
                </label>
              </div>
              <label className="space-y-2">
                <span className="text-sm font-medium text-fg">Comment</span>
                <textarea
                  className="min-h-28 w-full rounded-token border border-border bg-panel px-3.5 py-3 text-sm text-fg outline-none transition placeholder:text-fg-subtle focus:border-accent focus:ring-2 focus:ring-accent/20"
                  onChange={(event) => {
                    setCommentBody(event.target.value)
                    setReviewBody(event.target.value)
                  }}
                  placeholder="Add context, feedback, or a decision."
                  value={commentBody}
                />
              </label>
              <div className="flex flex-wrap items-center gap-3">
                <Button
                  disabled={isBusy}
                  icon={pendingAction === 'comment' ? <Spinner /> : <MessageSquareMore className="h-4 w-4" strokeWidth={1.9} />}
                  onClick={() =>
                    void runAction('comment', async () => {
                      await onComment(pullRequest.id, {
                        display_name: displayName,
                        body: commentBody,
                      })
                      setCommentBody('')
                      setReviewBody('')
                    })
                  }
                  tone="muted"
                >
                  {pendingAction === 'comment' ? 'Posting...' : 'Add comment'}
                </Button>
                <Button
                  disabled={isBusy}
                  icon={pendingAction === 'comment_review' ? <Spinner /> : <MessageSquareMore className="h-4 w-4" strokeWidth={1.9} />}
                  onClick={() =>
                    void runAction('comment_review', async () => {
                      await onReview(pullRequest.id, {
                        display_name: displayName,
                        body: reviewBody,
                        state: 'commented',
                      })
                      setCommentBody('')
                      setReviewBody('')
                    })
                  }
                  tone="muted"
                >
                  {pendingAction === 'comment_review' ? 'Submitting...' : 'Comment review'}
                </Button>
                <Button
                  disabled={isBusy}
                  icon={pendingAction === 'approve' ? <Spinner /> : <ThumbsUp className="h-4 w-4" strokeWidth={1.9} />}
                  onClick={() =>
                    void runAction('approve', async () => {
                      await onReview(pullRequest.id, {
                        display_name: displayName,
                        body: reviewBody,
                        state: 'approved',
                      })
                      setCommentBody('')
                      setReviewBody('')
                    })
                  }
                >
                  {pendingAction === 'approve' ? 'Submitting...' : 'Approve'}
                </Button>
                <Button
                  disabled={isBusy}
                  icon={pendingAction === 'request_changes' ? <Spinner /> : <ThumbsDown className="h-4 w-4" strokeWidth={1.9} />}
                  onClick={() =>
                    void runAction('request_changes', async () => {
                      await onReview(pullRequest.id, {
                        display_name: displayName,
                        body: reviewBody,
                        state: 'changes_requested',
                      })
                      setCommentBody('')
                      setReviewBody('')
                    })
                  }
                  tone="danger"
                >
                  {pendingAction === 'request_changes' ? 'Submitting...' : 'Request changes'}
                </Button>
              </div>
              {actionError ? <p className="text-sm text-danger">{actionError}</p> : null}
            </div>

            <div className="space-y-3 rounded-token border border-border bg-panel-subtle px-4 py-4">
              <p className="text-sm font-semibold text-fg">Review summary</p>
              <div className="flex flex-wrap gap-2">
                <Badge tone="success">{reviewSummary.approvals} approvals</Badge>
                <Badge tone="danger">{reviewSummary.changes_requested} changes requested</Badge>
                <Badge tone="muted">{reviewSummary.comments} comment-only reviews</Badge>
              </div>
              {reviewSummary.latest_reviews.length === 0 ? (
                <p className="text-sm text-fg-subtle">No reviews yet.</p>
              ) : (
                <div className="space-y-2">
                  {reviewSummary.latest_reviews.map((entry) => (
                    <div className="rounded-token border border-border bg-panel px-3 py-3" key={`${entry.actor_role}-${entry.display_name}`}>
                      <div className="flex flex-wrap items-center justify-between gap-2">
                        <p className="text-sm font-medium text-fg">{entry.display_name}</p>
                        <Badge tone={reviewTone(entry.state)}>{reviewLabel(entry.state)}</Badge>
                      </div>
                      <p className="mt-1 text-xs text-fg-subtle">
                        {formatRelativeTime(entry.reviewed_at_ms)} · {formatDate(entry.reviewed_at_ms)}
                      </p>
                    </div>
                  ))}
                </div>
              )}
            </div>
          </div>

          <div className="grid gap-6 xl:grid-cols-2">
            <div className="space-y-3">
              <p className="text-sm font-semibold text-fg">Comments</p>
              {comments.length === 0 ? (
                <EmptyState title="No comments yet" message="Start the discussion with context, review notes, or follow-up questions." />
              ) : (
                comments.map((comment) => (
                  <div className="rounded-token border border-border bg-panel-subtle px-4 py-4" key={comment.id}>
                    <div className="flex flex-wrap items-center gap-2">
                      <p className="text-sm font-semibold text-fg">{comment.display_name}</p>
                      <Badge tone={comment.actor_role === 'owner' ? 'accent' : 'muted'}>
                        {comment.actor_role === 'owner' ? 'Owner' : 'Viewer'}
                      </Badge>
                    </div>
                    <p className="mt-2 whitespace-pre-wrap text-sm leading-6 text-fg-muted">{comment.body}</p>
                    <p className="mt-3 text-xs text-fg-subtle">
                      {formatRelativeTime(comment.created_at_ms)} · {formatDate(comment.created_at_ms)}
                    </p>
                  </div>
                ))
              )}
            </div>

            <div className="space-y-3">
              <p className="text-sm font-semibold text-fg">Reviews</p>
              {reviews.length === 0 ? (
                <EmptyState title="No reviews yet" message="Formal review decisions will appear here as approvals, changes requested, or comment-only reviews." />
              ) : (
                reviews
                  .slice()
                  .reverse()
                  .map((review) => (
                    <div className="rounded-token border border-border bg-panel-subtle px-4 py-4" key={review.id}>
                      <div className="flex flex-wrap items-center justify-between gap-2">
                        <div className="flex flex-wrap items-center gap-2">
                          <p className="text-sm font-semibold text-fg">{review.display_name}</p>
                          <Badge tone={review.actor_role === 'owner' ? 'accent' : 'muted'}>
                            {review.actor_role === 'owner' ? 'Owner' : 'Viewer'}
                          </Badge>
                        </div>
                        <Badge tone={reviewTone(review.state)}>{reviewLabel(review.state)}</Badge>
                      </div>
                      {review.body ? (
                        <p className="mt-2 whitespace-pre-wrap text-sm leading-6 text-fg-muted">{review.body}</p>
                      ) : (
                        <p className="mt-2 text-sm text-fg-subtle">No review body provided.</p>
                      )}
                      <p className="mt-3 text-xs text-fg-subtle">
                        {formatRelativeTime(review.created_at_ms)} · {formatDate(review.created_at_ms)}
                      </p>
                    </div>
                  ))
              )}
            </div>
          </div>
        </div>
      </Panel>

      <Panel subtitle="A running timeline of pull request activity." title="Activity">
        {reversedActivity.length === 0 ? (
          <EmptyState title="No activity yet" message="Pull request events will appear here as the review progresses." />
        ) : (
          <div className="space-y-3">
            {reversedActivity.map((entry) => (
              <div className="rounded-token border border-border bg-panel-subtle px-4 py-4" key={entry.id}>
                <div className="flex flex-wrap items-center gap-2">
                  <p className="text-sm font-semibold text-fg">{actorLabel(entry.actor_role, entry.display_name)}</p>
                  <Badge tone={entry.actor_role === 'owner' ? 'accent' : 'muted'}>
                    {entry.actor_role === 'owner' ? 'Owner' : 'Viewer'}
                  </Badge>
                  {entry.review_state ? (
                    <Badge tone={reviewTone(entry.review_state)}>{reviewLabel(entry.review_state)}</Badge>
                  ) : null}
                </div>
                <p className="mt-2 text-sm text-fg-muted">{activityLabel(entry)}</p>
                {entry.body ? (
                  <p className="mt-2 whitespace-pre-wrap text-sm leading-6 text-fg-muted">{entry.body}</p>
                ) : null}
                {entry.kind === 'edited' && (entry.title || entry.description) ? (
                  <div className="mt-2 rounded-token border border-border bg-panel px-3 py-3 text-sm text-fg-muted">
                    {entry.title ? <p>Title: {entry.title}</p> : null}
                    {entry.description ? <p className="mt-1 whitespace-pre-wrap">Description: {entry.description}</p> : null}
                  </div>
                ) : null}
                <p className="mt-3 text-xs text-fg-subtle">
                  {formatRelativeTime(entry.created_at_ms)} · {formatDate(entry.created_at_ms)}
                </p>
              </div>
            ))}
          </div>
        )}
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
  actor,
  pullRequests,
  highlightedPullRequestId,
  selectedPullRequestId,
  canManage,
  canCreate,
  onBack,
  onComment,
  onCreate,
  onDelete,
  onMerge,
  onReview,
  onSelect,
  onUpdate,
}: {
  actor: UiRole
  pullRequests: PullRequestRecord[]
  highlightedPullRequestId: string | null
  selectedPullRequestId: string | null
  canManage: boolean
  canCreate: boolean
  onBack: () => void
  onComment: (id: string, payload: { display_name: string; body: string }) => Promise<PullRequestRecord>
  onCreate: () => void
  onDelete: (id: string) => Promise<PullRequestRecord>
  onMerge: (id: string) => Promise<void>
  onReview: (
    id: string,
    payload: { display_name: string; body: string; state: PullRequestReviewState },
  ) => Promise<PullRequestRecord>
  onSelect: (id: string) => void
  onUpdate: (
    id: string,
    payload: { title?: string; description?: string; status?: 'open' | 'closed' },
  ) => Promise<PullRequestRecord>
}) {
  if (selectedPullRequestId) {
    return (
      <PullRequestDetail
        actor={actor}
        canManage={canManage}
        onBack={onBack}
        onComment={onComment}
        onDelete={onDelete}
        onMerge={onMerge}
        onReview={onReview}
        onUpdate={onUpdate}
        pullRequestId={selectedPullRequestId}
      />
    )
  }

  return (
    <PullRequestList
      canCreate={canCreate}
      canManage={canManage}
      highlightedPullRequestId={highlightedPullRequestId}
      onCreate={onCreate}
      onMerge={onMerge}
      onSelect={onSelect}
      pullRequests={pullRequests}
    />
  )
}
