import { useCallback, useEffect, useMemo, useState } from 'react'
import {
  ArrowLeft,
  CircleDot,
  Link2,
  LoaderCircle,
  MessageSquareMore,
  PencilLine,
  Plus,
  Search,
  Tag,
  Trash2,
  UserRoundPlus,
  XCircle,
} from 'lucide-react'
import { api } from '../../../lib/api'
import { usePersistentDisplayName } from '../../../lib/usePersistentDisplayName'
import type {
  BootstrapResponse,
  IssueCommentResponse,
  IssueDetailResponse,
  IssueLabel,
  IssueLinkedPullRequestView,
  IssueLinkRelation,
  IssueLinkSource,
  IssueMetadataResponse,
  IssueMilestone,
  IssueReactionContent,
  IssueReactionSummary,
  IssueRecord,
  IssueStatus,
  IssueTimelineEvent,
  PullRequestRecord,
  UiRole,
} from '../../../lib/types'
import { Badge, Button, EmptyState, Panel, Spinner } from '../../atoms/Controls'
import { MarkdownSurface } from '../MarkdownSurface'
import { FieldError, TextArea, TextInput } from '../../molecules/Fields'

const ISSUE_REACTIONS: Array<{ content: IssueReactionContent; label: string }> = [
  { content: 'thumbs_up', label: '+1' },
  { content: 'thumbs_down', label: '-1' },
  { content: 'heart', label: 'Heart' },
  { content: 'rocket', label: 'Rocket' },
  { content: 'eyes', label: 'Eyes' },
]

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

function issueStatusTone(status: IssueStatus) {
  return status === 'closed' ? 'muted' : 'accent'
}

function actorLabel(role: UiRole, displayName: string) {
  return displayName.trim() || (role === 'owner' ? 'Owner' : 'Viewer')
}

function linkRelationLabel(relation: IssueLinkRelation) {
  return relation === 'closing' ? 'Closes' : 'Related'
}

function linkSourceLabel(source: IssueLinkSource) {
  switch (source) {
    case 'manual':
      return 'Manual'
    case 'issue_description':
      return 'Issue body'
    case 'issue_comment':
      return 'Issue comment'
    case 'pull_request_description':
      return 'PR body'
    case 'pull_request_comment':
      return 'PR comment'
    case 'pull_request_review':
      return 'PR review'
  }
}

function timelineLabel(event: IssueTimelineEvent, labelMap: Map<string, IssueLabel>, milestoneMap: Map<string, IssueMilestone>) {
  switch (event.kind) {
    case 'opened':
      return 'opened this issue'
    case 'commented':
      return 'left a comment'
    case 'edited':
      return 'edited the issue details'
    case 'closed':
      return 'closed this issue'
    case 'reopened':
      return 'reopened this issue'
    case 'labels_changed':
      return event.labels.length > 0
        ? `set labels to ${event.labels.map((labelId) => labelMap.get(labelId)?.name ?? labelId).join(', ')}`
        : 'cleared the labels'
    case 'assignees_changed':
      return event.assignee_user_ids.length > 0 ? 'updated the assignees' : 'cleared the assignees'
    case 'milestone_changed':
      return event.milestone_id
        ? `set milestone to ${milestoneMap.get(event.milestone_id)?.title ?? event.milestone_id}`
        : 'cleared the milestone'
    case 'pull_request_linked':
      return 'linked a pull request'
    case 'pull_request_unlinked':
      return 'unlinked a pull request'
    case 'reaction_toggled':
      return `toggled a ${event.reaction?.replaceAll('_', ' ') ?? 'reaction'} reaction`
  }
}

function ReactionButtons({
  summary,
  onToggle,
  disabled,
}: {
  summary: IssueReactionSummary[]
  onToggle: (content: IssueReactionContent) => Promise<void>
  disabled: boolean
}) {
  const byContent = new Map(summary.map((entry) => [entry.content, entry]))
  return (
    <div className="flex flex-wrap gap-2">
      {ISSUE_REACTIONS.map((reaction) => {
        const entry = byContent.get(reaction.content)
        return (
          <button
            className={`rounded-full border px-3 py-1.5 text-xs font-medium transition ${
              entry?.reacted
                ? 'border-accent/40 bg-accent/10 text-fg'
                : 'border-border bg-panel-subtle text-fg-muted hover:border-border-strong hover:text-fg'
            }`}
            disabled={disabled}
            key={reaction.content}
            onClick={() => void onToggle(reaction.content)}
            type="button"
          >
            {reaction.label} {entry?.count ? `(${entry.count})` : ''}
          </button>
        )
      })}
    </div>
  )
}

function MultiSelectField({
  label,
  options,
  value,
  onChange,
  disabled = false,
}: {
  label: string
  options: Array<{ id: string; label: string }>
  value: string[]
  onChange: (value: string[]) => void
  disabled?: boolean
}) {
  return (
    <div className="space-y-2">
      <label className="block text-sm font-medium text-fg">{label}</label>
      <select
        className="min-h-28 w-full rounded-token border border-border bg-panel-subtle px-3.5 py-2.5 text-sm text-fg outline-none transition-colors focus:border-accent focus:ring-2 focus:ring-accent/20"
        disabled={disabled}
        multiple
        onChange={(event) => onChange(Array.from(event.target.selectedOptions, (option) => option.value))}
        value={value}
      >
        {options.map((option) => (
          <option key={option.id} value={option.id}>
            {option.label}
          </option>
        ))}
      </select>
    </div>
  )
}

function IssueList({
  issues,
  metadata,
  highlightedIssueId,
  canCreate,
  onCreate,
  onSelect,
}: {
  issues: IssueRecord[]
  metadata: IssueMetadataResponse
  highlightedIssueId: string | null
  canCreate: boolean
  onCreate: () => void
  onSelect: (id: string) => void
}) {
  const [search, setSearch] = useState('')
  const [statusFilter, setStatusFilter] = useState<'all' | IssueStatus>('all')
  const [labelFilter, setLabelFilter] = useState('all')
  const labelMap = useMemo(() => new Map(metadata.labels.map((label) => [label.id, label.name])), [metadata.labels])

  useEffect(() => {
    if (!highlightedIssueId) {
      return
    }
    document.getElementById(`issue-${highlightedIssueId}`)?.scrollIntoView({ behavior: 'smooth', block: 'nearest' })
  }, [highlightedIssueId, issues.length])

  const filteredIssues = useMemo(() => {
    const needle = search.trim().toLowerCase()
    return issues.filter((issue) => {
      if (statusFilter !== 'all' && issue.status !== statusFilter) {
        return false
      }
      if (labelFilter !== 'all' && !issue.label_ids.includes(labelFilter)) {
        return false
      }
      if (!needle) {
        return true
      }
      const haystack = [issue.title, issue.description, `#${issue.number}`].join(' ').toLowerCase()
      return haystack.includes(needle)
    })
  }, [issues, labelFilter, search, statusFilter])

  return (
    <Panel
      action={
        canCreate ? (
          <Button icon={<CircleDot className="h-4 w-4" strokeWidth={1.9} />} onClick={onCreate}>
            New issue
          </Button>
        ) : null
      }
      subtitle="Search, filter, and open GitHub-style issues without leaving the repo session."
      title="Issues"
    >
      <div className="space-y-4">
        <div className="grid gap-3 lg:grid-cols-[minmax(0,1fr)_220px_auto]">
          <div className="relative">
            <Search
              aria-hidden="true"
              className="pointer-events-none absolute left-3 top-1/2 h-4 w-4 -translate-y-1/2 text-fg-subtle"
              strokeWidth={1.9}
            />
            <input
              className="w-full rounded-token border border-border bg-panel-subtle py-2.5 pl-10 pr-3.5 text-sm text-fg outline-none transition placeholder:text-fg-subtle focus:border-accent focus:ring-2 focus:ring-accent/20"
              onChange={(event) => setSearch(event.target.value)}
              placeholder="Search issues"
              type="search"
              value={search}
            />
          </div>
          <select
            className="rounded-token border border-border bg-panel-subtle px-3.5 py-2.5 text-sm text-fg outline-none transition-colors focus:border-accent focus:ring-2 focus:ring-accent/20"
            onChange={(event) => setLabelFilter(event.target.value)}
            value={labelFilter}
          >
            <option value="all">All labels</option>
            {metadata.labels.map((label) => (
              <option key={label.id} value={label.id}>
                {label.name}
              </option>
            ))}
          </select>
          <div className="flex flex-wrap gap-2">
            {(['all', 'open', 'closed'] as const).map((option) => (
              <button
                className={`rounded-full border px-3 py-1.5 text-sm font-medium transition ${
                  statusFilter === option
                    ? 'border-accent/40 bg-accent/10 text-fg'
                    : 'border-border bg-panel-subtle text-fg-muted hover:border-border-strong hover:text-fg'
                }`}
                key={option}
                onClick={() => setStatusFilter(option)}
                type="button"
              >
                {option === 'all' ? 'All' : option[0].toUpperCase() + option.slice(1)}
              </button>
            ))}
          </div>
        </div>
        {issues.length === 0 ? (
          <EmptyState title="No issues yet" message="Create the first issue to track work, bugs, or follow-up ideas." />
        ) : filteredIssues.length === 0 ? (
          <EmptyState title="No matching issues" message="Try a different search or widen the filters." />
        ) : (
          <div className="space-y-3">
            {filteredIssues.map((issue) => {
              const isHighlighted = issue.id === highlightedIssueId
              return (
                <button
                  className={`w-full rounded-token border px-4 py-4 text-left transition ${
                    isHighlighted
                      ? 'border-accent/35 bg-panel'
                      : 'border-border bg-panel-subtle hover:border-border-strong hover:bg-panel'
                  }`}
                  id={`issue-${issue.id}`}
                  key={issue.id}
                  onClick={() => onSelect(issue.id)}
                  type="button"
                >
                  <div className="flex flex-wrap items-start justify-between gap-4">
                    <div className="min-w-0 flex-1">
                      <div className="flex flex-wrap items-center gap-2">
                        <h3 className="text-base font-semibold text-fg">
                          #{issue.number} {issue.title}
                        </h3>
                        <Badge tone={issueStatusTone(issue.status)}>{issue.status}</Badge>
                      </div>
                      {issue.description ? (
                        <p className="mt-3 line-clamp-2 text-sm leading-6 text-fg-muted">{issue.description}</p>
                      ) : null}
                      <div className="mt-3 flex flex-wrap gap-2">
                        {issue.label_ids.map((labelId) => (
                          <Badge key={labelId} tone="muted">
                            {labelMap.get(labelId) ?? labelId}
                          </Badge>
                        ))}
                      </div>
                      <div className="mt-3 flex flex-wrap items-center gap-3 text-xs text-fg-subtle">
                        <span>{issue.comments.length} comment{issue.comments.length === 1 ? '' : 's'}</span>
                        {issue.assignee_user_ids.length > 0 ? (
                          <span>{issue.assignee_user_ids.length} assignee{issue.assignee_user_ids.length === 1 ? '' : 's'}</span>
                        ) : null}
                      </div>
                    </div>
                    <div className="text-right text-xs text-fg-subtle">
                      <p>Updated {formatRelativeTime(issue.updated_at_ms)}</p>
                      <p className="mt-1">{formatDate(issue.updated_at_ms)}</p>
                    </div>
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

function IssuesDetail({
  bootstrap,
  pullRequests,
  issueId,
  canManage,
  onBack,
  onDelete,
  onComment,
  onUpdate,
  onReact,
  onReactComment,
  onSetLabels,
  onSetAssignees,
  onSetMilestone,
  onLinkPullRequest,
  onUnlinkPullRequest,
  onUpsertLabel,
  onDeleteLabel,
  onUpsertMilestone,
  onDeleteMilestone,
}: {
  bootstrap: BootstrapResponse
  pullRequests: PullRequestRecord[]
  issueId: string
  canManage: boolean
  onBack: () => void
  onDelete: (id: string) => Promise<IssueRecord>
  onComment: (id: string, payload: { display_name?: string | null; body: string }) => Promise<IssueRecord>
  onUpdate: (id: string, payload: { title?: string; description?: string; status?: 'open' | 'closed' }) => Promise<IssueRecord>
  onReact: (id: string, payload: { content: IssueReactionContent; display_name?: string | null }) => Promise<IssueRecord>
  onReactComment: (
    id: string,
    commentId: string,
    payload: { content: IssueReactionContent; display_name?: string | null },
  ) => Promise<IssueRecord>
  onSetLabels: (id: string, labelIds: string[]) => Promise<IssueRecord>
  onSetAssignees: (id: string, assigneeUserIds: string[]) => Promise<IssueRecord>
  onSetMilestone: (id: string, milestoneId: string | null) => Promise<IssueRecord>
  onLinkPullRequest: (id: string, pullRequestId: string) => Promise<IssueRecord>
  onUnlinkPullRequest: (id: string, pullRequestId: string) => Promise<IssueRecord>
  onUpsertLabel: (payload: { id?: string; name: string; color?: string; description?: string }) => Promise<void>
  onDeleteLabel: (id: string) => Promise<void>
  onUpsertMilestone: (payload: { id?: string; title: string; description?: string }) => Promise<void>
  onDeleteMilestone: (id: string) => Promise<void>
}) {
  const [detail, setDetail] = useState<IssueDetailResponse | null>(null)
  const [loading, setLoading] = useState(true)
  const [loadError, setLoadError] = useState<string | null>(null)
  const [actionError, setActionError] = useState<string | null>(null)
  const [pendingAction, setPendingAction] = useState<string | null>(null)
  const [isEditing, setIsEditing] = useState(false)
  const [titleDraft, setTitleDraft] = useState('')
  const [descriptionDraft, setDescriptionDraft] = useState('')
  const [commentBody, setCommentBody] = useState('')
  const [newLabelName, setNewLabelName] = useState('')
  const [newMilestoneTitle, setNewMilestoneTitle] = useState('')
  const [displayName, setDisplayName] = usePersistentDisplayName('qit.issue.display_name')

  const loadDetail = useCallback(async () => {
    setLoading(true)
    setLoadError(null)
    setActionError(null)
    try {
      const response = await api.issue(issueId)
      setDetail(response)
      setTitleDraft(response.issue.title)
      setDescriptionDraft(response.issue.description)
      setIsEditing(false)
    } catch (error) {
      setLoadError(error instanceof Error ? error.message : 'Failed to load the issue.')
    } finally {
      setLoading(false)
    }
  }, [issueId])

  useEffect(() => {
    void loadDetail()
  }, [loadDetail])

  const runAction = useCallback(
    async (label: string, action: () => Promise<void>) => {
      setPendingAction(label)
      setActionError(null)
      try {
        await action()
        await loadDetail()
      } catch (error) {
        setActionError(error instanceof Error ? error.message : 'Issue action failed.')
      } finally {
        setPendingAction(null)
      }
    },
    [loadDetail],
  )

  if (loading) {
    return (
      <Panel subtitle="Loading issue details, comments, and metadata." title="Issue">
        <div className="flex items-center gap-3 text-sm text-fg-muted">
          <Spinner />
          <span>Loading issue…</span>
        </div>
      </Panel>
    )
  }

  if (loadError || !detail) {
    return (
      <Panel subtitle="This issue could not be loaded right now." title="Issue">
        <div className="space-y-4">
          <Button icon={<ArrowLeft className="h-4 w-4" strokeWidth={1.9} />} onClick={onBack} tone="muted">
            Back to list
          </Button>
          <EmptyState title="Unable to load issue" message={loadError ?? 'The issue is unavailable.'} />
        </div>
      </Panel>
    )
  }

  const issue = detail.issue
  const metadata = detail.metadata
  const labelMap = new Map(metadata.labels.map((label) => [label.id, label]))
  const milestoneMap = new Map(metadata.milestones.map((milestone) => [milestone.id, milestone]))
  const assigneeMap = new Map(metadata.assignees.map((assignee) => [assignee.id, assignee]))
  const availablePullRequests = pullRequests.filter(
    (pullRequest) =>
      !detail.linked_pull_requests.some((linked) => linked.pull_request.id === pullRequest.id),
  )
  const isBusy = pendingAction !== null
  const canEditIssue = canManage || (bootstrap.principal?.user_id != null && bootstrap.principal.user_id === issue.author.user_id)
  const needsDisplayName = !bootstrap.principal

  return (
    <div className="space-y-6">
      <Panel
        action={
          <Button icon={<ArrowLeft className="h-4 w-4" strokeWidth={1.9} />} onClick={onBack} tone="muted">
            Back to list
          </Button>
        }
        subtitle="Track the full issue conversation, metadata, and linked pull requests in one place."
        title={`#${issue.number} ${isEditing ? 'Edit issue' : issue.title}`}
      >
        <div className="space-y-5">
          <div className="flex flex-wrap items-center gap-2">
            <Badge tone={issueStatusTone(issue.status)}>{issue.status}</Badge>
            {issue.label_ids.map((labelId) => {
              const label = labelMap.get(labelId)
              return <Badge key={labelId} tone="muted">{label?.name ?? labelId}</Badge>
            })}
          </div>
          <div className="grid gap-3 md:grid-cols-2 xl:grid-cols-4">
            <div className="rounded-token border border-border bg-panel-subtle px-4 py-3">
              <p className="text-[11px] font-semibold uppercase tracking-[0.14em] text-fg-subtle">Opened</p>
              <p className="mt-1 text-sm font-medium text-fg">{formatDate(issue.created_at_ms)}</p>
            </div>
            <div className="rounded-token border border-border bg-panel-subtle px-4 py-3">
              <p className="text-[11px] font-semibold uppercase tracking-[0.14em] text-fg-subtle">Comments</p>
              <p className="mt-1 text-lg font-semibold text-fg">{detail.comments.length}</p>
            </div>
            <div className="rounded-token border border-border bg-panel-subtle px-4 py-3">
              <p className="text-[11px] font-semibold uppercase tracking-[0.14em] text-fg-subtle">Linked PRs</p>
              <p className="mt-1 text-lg font-semibold text-fg">{detail.linked_pull_requests.length}</p>
            </div>
            <div className="rounded-token border border-border bg-panel-subtle px-4 py-3">
              <p className="text-[11px] font-semibold uppercase tracking-[0.14em] text-fg-subtle">Assignees</p>
              <p className="mt-1 text-lg font-semibold text-fg">{issue.assignee_user_ids.length}</p>
            </div>
          </div>
          {isEditing ? (
            <div className="space-y-4">
              <TextInput label="Title" onChange={setTitleDraft} value={titleDraft} />
              <TextArea label="Description" onChange={setDescriptionDraft} rows={10} value={descriptionDraft} />
              <div className="flex flex-wrap gap-3">
                <Button
                  disabled={isBusy || !titleDraft.trim()}
                  icon={isBusy && pendingAction === 'save issue' ? <LoaderCircle className="h-4 w-4 animate-spin" /> : <PencilLine className="h-4 w-4" strokeWidth={1.9} />}
                  onClick={() => void runAction('save issue', async () => {
                    await onUpdate(issue.id, { title: titleDraft.trim(), description: descriptionDraft.trim() })
                  })}
                >
                  Save issue
                </Button>
                <Button disabled={isBusy} onClick={() => setIsEditing(false)} tone="muted">
                  Cancel
                </Button>
              </div>
            </div>
          ) : issue.description ? (
            <MarkdownSurface source={issue.description} />
          ) : (
            <EmptyState title="No description yet" message="Add a fuller description when the scope or context becomes clearer." />
          )}
          <div className="flex flex-wrap gap-3 text-sm text-fg-muted">
            <span>Opened by {actorLabel(issue.author.role, issue.author.display_name)}</span>
            <span>{formatDate(issue.created_at_ms)}</span>
          </div>
          <ReactionButtons
            disabled={isBusy}
            onToggle={async (content) => {
              await runAction('toggle reaction', async () => {
                await onReact(issue.id, { content, display_name: needsDisplayName ? displayName : null })
              })
            }}
            summary={detail.reaction_summary}
          />
          {canEditIssue ? (
            <div className="flex flex-wrap gap-3">
              <Button disabled={isBusy} icon={<PencilLine className="h-4 w-4" strokeWidth={1.9} />} onClick={() => setIsEditing(true)} tone="muted">
                Edit
              </Button>
              <Button
                disabled={isBusy}
                icon={issue.status === 'open' ? <XCircle className="h-4 w-4" strokeWidth={1.9} /> : <CircleDot className="h-4 w-4" strokeWidth={1.9} />}
                onClick={() => void runAction(issue.status === 'open' ? 'close issue' : 'reopen issue', async () => {
                  await onUpdate(issue.id, { status: issue.status === 'open' ? 'closed' : 'open' })
                })}
                tone="muted"
              >
                {issue.status === 'open' ? 'Close issue' : 'Reopen issue'}
              </Button>
              {canManage ? (
                <Button
                  disabled={isBusy}
                  icon={<Trash2 className="h-4 w-4" strokeWidth={1.9} />}
                  onClick={() => void runAction('delete issue', async () => {
                    await onDelete(issue.id)
                    onBack()
                  })}
                  tone="danger"
                >
                  Delete
                </Button>
              ) : null}
            </div>
          ) : null}
          <FieldError message={actionError} />
        </div>
      </Panel>

      <Panel subtitle="Comment on the issue and capture follow-up context in markdown." title="Discussion">
        <div className="space-y-4">
          {needsDisplayName ? (
            <TextInput label="Display name" onChange={setDisplayName} value={displayName} />
          ) : null}
          <TextArea label="New comment" onChange={setCommentBody} rows={5} value={commentBody} />
          <div className="flex justify-end">
            <Button
              disabled={isBusy || !commentBody.trim()}
              icon={isBusy && pendingAction === 'comment issue' ? <LoaderCircle className="h-4 w-4 animate-spin" /> : <MessageSquareMore className="h-4 w-4" strokeWidth={1.9} />}
              onClick={() => void runAction('comment issue', async () => {
                await onComment(issue.id, { body: commentBody.trim(), display_name: needsDisplayName ? displayName : null })
                setCommentBody('')
              })}
            >
              Comment
            </Button>
          </div>
          {detail.comments.length === 0 ? (
            <EmptyState title="No comments yet" message="Start the discussion with a note, question, or implementation update." />
          ) : (
            <div className="space-y-4">
              {detail.comments.map(({ comment, reaction_summary }: IssueCommentResponse) => (
                <div className="rounded-token border border-border bg-panel-subtle px-4 py-4" key={comment.id}>
                  <div className="flex flex-wrap items-center justify-between gap-3">
                    <div>
                      <p className="font-medium text-fg">{actorLabel(comment.actor.role, comment.actor.display_name)}</p>
                      <p className="text-xs text-fg-subtle">{formatDate(comment.created_at_ms)}</p>
                    </div>
                  </div>
                  <div className="mt-4">
                    <MarkdownSurface source={comment.body} />
                  </div>
                  <div className="mt-4">
                    <ReactionButtons
                      disabled={isBusy}
                      onToggle={async (content) => {
                        await runAction('react to comment', async () => {
                          await onReactComment(issue.id, comment.id, {
                            content,
                            display_name: needsDisplayName ? displayName : null,
                          })
                        })
                      }}
                      summary={reaction_summary}
                    />
                  </div>
                </div>
              ))}
            </div>
          )}
        </div>
      </Panel>

      <Panel subtitle="Apply labels, assignees, milestones, and pull-request links with GitHub-style repo metadata." title="Metadata">
        <div className="grid gap-6 xl:grid-cols-2">
          <div className="space-y-5">
            <MultiSelectField
              disabled={!canManage}
              label="Labels"
              onChange={(value) => {
                void runAction('set labels', async () => {
                  await onSetLabels(issue.id, value)
                })
              }}
              options={metadata.labels.map((label) => ({ id: label.id, label: label.name }))}
              value={issue.label_ids}
            />
            <MultiSelectField
              disabled={!canManage}
              label="Assignees"
              onChange={(value) => {
                void runAction('set assignees', async () => {
                  await onSetAssignees(issue.id, value)
                })
              }}
              options={metadata.assignees.map((assignee) => ({
                id: assignee.id,
                label: `${assignee.username} (${assignee.name})`,
              }))}
              value={issue.assignee_user_ids}
            />
            <div className="space-y-2">
              <label className="block text-sm font-medium text-fg" htmlFor="issue-milestone-select">
                Milestone
              </label>
              <select
                className="w-full rounded-token border border-border bg-panel-subtle px-3.5 py-2.5 text-sm text-fg outline-none transition-colors focus:border-accent focus:ring-2 focus:ring-accent/20"
                disabled={!canManage}
                id="issue-milestone-select"
                onChange={(event) => {
                  void runAction('set milestone', async () => {
                    await onSetMilestone(issue.id, event.target.value || null)
                  })
                }}
                value={issue.milestone_id ?? ''}
              >
                <option value="">No milestone</option>
                {metadata.milestones.map((milestone) => (
                  <option key={milestone.id} value={milestone.id}>
                    {milestone.title}
                  </option>
                ))}
              </select>
            </div>
            <div className="space-y-3">
              <div className="flex items-center justify-between gap-3">
                <h3 className="text-sm font-semibold text-fg">Linked pull requests</h3>
              </div>
              <div className="space-y-2">
                {detail.linked_pull_requests.length === 0 ? (
                  <EmptyState title="No linked pull requests" message="Link a pull request to show implementation progress here." />
                ) : (
                  detail.linked_pull_requests.map((linked: IssueLinkedPullRequestView) => (
                    <div
                      className="flex items-center justify-between gap-3 rounded-token border border-border bg-panel-subtle px-3.5 py-3"
                      key={`${linked.pull_request.id}-${linked.source}-${linked.relation}`}
                    >
                      <div>
                        <div className="flex flex-wrap items-center gap-2">
                          <p className="font-medium text-fg">{linked.pull_request.title}</p>
                          <Badge tone={linked.relation === 'closing' ? 'accent' : 'muted'}>
                            {linkRelationLabel(linked.relation)}
                          </Badge>
                          <Badge tone="muted">{linkSourceLabel(linked.source)}</Badge>
                        </div>
                        <p className="mt-1 text-xs text-fg-subtle">
                          {linked.pull_request.source_branch} to {linked.pull_request.target_branch}
                        </p>
                      </div>
                      <Button
                        disabled={isBusy || !canManage || linked.source !== 'manual'}
                        icon={<Link2 className="h-4 w-4" strokeWidth={1.9} />}
                        onClick={() => void runAction('unlink pull request', async () => {
                          await onUnlinkPullRequest(issue.id, linked.pull_request.id)
                        })}
                        tone="muted"
                      >
                        {linked.source === 'manual' ? 'Unlink' : 'Auto-linked'}
                      </Button>
                    </div>
                  ))
                )}
              </div>
              <select
                className="w-full rounded-token border border-border bg-panel-subtle px-3.5 py-2.5 text-sm text-fg outline-none transition-colors focus:border-accent focus:ring-2 focus:ring-accent/20"
                defaultValue=""
                disabled={!canManage}
                onChange={(event) => {
                  const pullRequestId = event.target.value
                  if (!pullRequestId) {
                    return
                  }
                  event.currentTarget.value = ''
                  void runAction('link pull request', async () => {
                    await onLinkPullRequest(issue.id, pullRequestId)
                  })
                }}
              >
                <option value="">Link a pull request…</option>
                {availablePullRequests.map((pullRequest) => (
                  <option key={pullRequest.id} value={pullRequest.id}>
                    {pullRequest.title}
                  </option>
                ))}
              </select>
            </div>
          </div>
          <div className="space-y-5">
            {canManage ? (
              <>
                <div className="space-y-3 rounded-token border border-border bg-panel-subtle px-4 py-4">
                  <div className="flex items-center gap-2">
                    <Tag className="h-4 w-4 text-fg-muted" strokeWidth={1.9} />
                    <h3 className="text-sm font-semibold text-fg">Create label</h3>
                  </div>
                  <TextInput label="Label name" onChange={setNewLabelName} value={newLabelName} />
                  <div className="flex justify-end">
                    <Button
                      disabled={isBusy || !newLabelName.trim()}
                      icon={<Plus className="h-4 w-4" strokeWidth={1.9} />}
                      onClick={() => void runAction('create label', async () => {
                        await onUpsertLabel({ name: newLabelName.trim() })
                        setNewLabelName('')
                      })}
                      tone="muted"
                    >
                      Save label
                    </Button>
                  </div>
                  <div className="space-y-2">
                    {metadata.labels.map((label) => (
                      <div className="flex items-center justify-between gap-3" key={label.id}>
                        <Badge tone="muted">{label.name}</Badge>
                        <Button
                          disabled={isBusy}
                          icon={<Trash2 className="h-4 w-4" strokeWidth={1.9} />}
                          onClick={() => void runAction('delete label', async () => {
                            await onDeleteLabel(label.id)
                          })}
                          tone="muted"
                        >
                          Delete
                        </Button>
                      </div>
                    ))}
                  </div>
                </div>
                <div className="space-y-3 rounded-token border border-border bg-panel-subtle px-4 py-4">
                  <div className="flex items-center gap-2">
                    <UserRoundPlus className="h-4 w-4 text-fg-muted" strokeWidth={1.9} />
                    <h3 className="text-sm font-semibold text-fg">Create milestone</h3>
                  </div>
                  <TextInput label="Milestone title" onChange={setNewMilestoneTitle} value={newMilestoneTitle} />
                  <div className="flex justify-end">
                    <Button
                      disabled={isBusy || !newMilestoneTitle.trim()}
                      icon={<Plus className="h-4 w-4" strokeWidth={1.9} />}
                      onClick={() => void runAction('create milestone', async () => {
                        await onUpsertMilestone({ title: newMilestoneTitle.trim() })
                        setNewMilestoneTitle('')
                      })}
                      tone="muted"
                    >
                      Save milestone
                    </Button>
                  </div>
                  <div className="space-y-2">
                    {metadata.milestones.map((milestone) => (
                      <div className="flex items-center justify-between gap-3" key={milestone.id}>
                        <Badge tone="muted">{milestone.title}</Badge>
                        <Button
                          disabled={isBusy}
                          icon={<Trash2 className="h-4 w-4" strokeWidth={1.9} />}
                          onClick={() => void runAction('delete milestone', async () => {
                            await onDeleteMilestone(milestone.id)
                          })}
                          tone="muted"
                        >
                          Delete
                        </Button>
                      </div>
                    ))}
                  </div>
                </div>
              </>
            ) : (
              <div className="rounded-token border border-dashed border-border bg-panel-subtle px-4 py-4 text-sm text-fg-muted">
                Only owners can create repo-level labels and milestones.
              </div>
            )}
            <div className="rounded-token border border-border bg-panel-subtle px-4 py-4">
              <h3 className="text-sm font-semibold text-fg">Current assignees</h3>
              <div className="mt-3 flex flex-wrap gap-2">
                {issue.assignee_user_ids.length === 0 ? (
                  <span className="text-sm text-fg-muted">Nobody assigned yet.</span>
                ) : (
                  issue.assignee_user_ids.map((assigneeId) => (
                    <Badge key={assigneeId} tone="muted">
                      {assigneeMap.get(assigneeId)?.username ?? assigneeId}
                    </Badge>
                  ))
                )}
              </div>
            </div>
            <div className="rounded-token border border-border bg-panel-subtle px-4 py-4">
              <h3 className="text-sm font-semibold text-fg">Current milestone</h3>
              <div className="mt-3">
                {issue.milestone_id ? (
                  <Badge tone="muted">{milestoneMap.get(issue.milestone_id)?.title ?? issue.milestone_id}</Badge>
                ) : (
                  <span className="text-sm text-fg-muted">No milestone selected.</span>
                )}
              </div>
            </div>
          </div>
        </div>
      </Panel>

      <Panel subtitle="See every edit, status transition, and metadata change in order." title="Timeline">
        <div className="space-y-4">
          {detail.timeline.length === 0 ? (
            <EmptyState title="No timeline events" message="Issue timeline events will appear here as work evolves." />
          ) : (
            [...detail.timeline].reverse().map((event) => (
              <div className="rounded-token border border-border bg-panel-subtle px-4 py-4" key={event.id}>
                <div className="flex flex-wrap items-center justify-between gap-3">
                  <p className="text-sm text-fg">
                    <span className="font-medium">{actorLabel(event.actor.role, event.actor.display_name)}</span>{' '}
                    {timelineLabel(event, labelMap, milestoneMap)}
                  </p>
                  <span className="text-xs text-fg-subtle">{formatDate(event.created_at_ms)}</span>
                </div>
              </div>
            ))
          )}
        </div>
      </Panel>
    </div>
  )
}

export function IssuesPanel({
  bootstrap,
  canCreate,
  canManage,
  issues,
  metadata,
  pullRequests,
  highlightedIssueId,
  selectedIssueId,
  onBack,
  onCreate,
  onDelete,
  onComment,
  onUpdate,
  onReact,
  onReactComment,
  onSetLabels,
  onSetAssignees,
  onSetMilestone,
  onLinkPullRequest,
  onUnlinkPullRequest,
  onUpsertLabel,
  onDeleteLabel,
  onUpsertMilestone,
  onDeleteMilestone,
  onSelect,
}: {
  bootstrap: BootstrapResponse
  canCreate: boolean
  canManage: boolean
  issues: IssueRecord[]
  metadata: IssueMetadataResponse
  pullRequests: PullRequestRecord[]
  highlightedIssueId: string | null
  selectedIssueId: string | null
  onBack: () => void
  onCreate: () => void
  onDelete: (id: string) => Promise<IssueRecord>
  onComment: (id: string, payload: { display_name?: string | null; body: string }) => Promise<IssueRecord>
  onUpdate: (id: string, payload: { title?: string; description?: string; status?: 'open' | 'closed' }) => Promise<IssueRecord>
  onReact: (id: string, payload: { content: IssueReactionContent; display_name?: string | null }) => Promise<IssueRecord>
  onReactComment: (
    id: string,
    commentId: string,
    payload: { content: IssueReactionContent; display_name?: string | null },
  ) => Promise<IssueRecord>
  onSetLabels: (id: string, labelIds: string[]) => Promise<IssueRecord>
  onSetAssignees: (id: string, assigneeUserIds: string[]) => Promise<IssueRecord>
  onSetMilestone: (id: string, milestoneId: string | null) => Promise<IssueRecord>
  onLinkPullRequest: (id: string, pullRequestId: string) => Promise<IssueRecord>
  onUnlinkPullRequest: (id: string, pullRequestId: string) => Promise<IssueRecord>
  onUpsertLabel: (payload: { id?: string; name: string; color?: string; description?: string }) => Promise<void>
  onDeleteLabel: (id: string) => Promise<void>
  onUpsertMilestone: (payload: { id?: string; title: string; description?: string }) => Promise<void>
  onDeleteMilestone: (id: string) => Promise<void>
  onSelect: (id: string) => void
}) {
  if (!selectedIssueId) {
    return (
      <IssueList
        canCreate={canCreate}
        highlightedIssueId={highlightedIssueId}
        issues={issues}
        metadata={metadata}
        onCreate={onCreate}
        onSelect={onSelect}
      />
    )
  }

  return (
    <IssuesDetail
      bootstrap={bootstrap}
      canManage={canManage}
      issueId={selectedIssueId}
      pullRequests={pullRequests}
      onBack={onBack}
      onComment={onComment}
      onDelete={onDelete}
      onDeleteLabel={onDeleteLabel}
      onDeleteMilestone={onDeleteMilestone}
      onLinkPullRequest={onLinkPullRequest}
      onReact={onReact}
      onReactComment={onReactComment}
      onSetAssignees={onSetAssignees}
      onSetLabels={onSetLabels}
      onSetMilestone={onSetMilestone}
      onUnlinkPullRequest={onUnlinkPullRequest}
      onUpdate={onUpdate}
      onUpsertLabel={onUpsertLabel}
      onUpsertMilestone={onUpsertMilestone}
    />
  )
}
