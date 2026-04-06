import { useEffect, useMemo, useState } from 'react'
import { CircleDot, LoaderCircle } from 'lucide-react'
import type {
  BootstrapResponse,
  IssueAssigneeView,
  IssueLabel,
  IssueMetadataResponse,
  IssueMilestone,
  IssueRecord,
  PullRequestRecord,
} from '../../lib/types'
import { usePersistentDisplayName } from '../../lib/usePersistentDisplayName'
import { Button } from '../atoms/Controls'
import { ModalShell } from '../atoms/ModalShell'
import { FieldError, FormActions, TextArea, TextInput } from '../molecules/Fields'

function MultiSelect({
  label,
  options,
  value,
  onChange,
  emptyLabel,
}: {
  label: string
  options: Array<{ id: string; label: string }>
  value: string[]
  onChange: (next: string[]) => void
  emptyLabel: string
}) {
  return (
    <div className="space-y-2">
      <label className="block text-sm font-medium text-fg">{label}</label>
      {options.length === 0 ? (
        <div className="rounded-token border border-dashed border-border bg-panel-subtle px-3.5 py-3 text-sm text-fg-muted">
          {emptyLabel}
        </div>
      ) : (
        <select
          className="min-h-32 w-full rounded-token border border-border bg-panel-subtle px-3.5 py-2.5 text-sm text-fg outline-none transition-colors focus:border-accent focus:ring-2 focus:ring-accent/20"
          multiple
          onChange={(event) =>
            onChange(Array.from(event.target.selectedOptions, (option) => option.value))
          }
          value={value}
        >
          {options.map((option) => (
            <option key={option.id} value={option.id}>
              {option.label}
            </option>
          ))}
        </select>
      )}
    </div>
  )
}

export function CreateIssueModal({
  open,
  bootstrap,
  metadata,
  pullRequests,
  onClose,
  onCreateIssue,
}: {
  open: boolean
  bootstrap: BootstrapResponse
  metadata: IssueMetadataResponse
  pullRequests: PullRequestRecord[]
  onClose: () => void
  onCreateIssue: (payload: {
    title: string
    description: string
    display_name?: string | null
    label_ids: string[]
    assignee_user_ids: string[]
    milestone_id: string | null
    linked_pull_request_ids: string[]
  }) => Promise<IssueRecord>
}) {
  const [title, setTitle] = useState('')
  const [description, setDescription] = useState('')
  const [labelIds, setLabelIds] = useState<string[]>([])
  const [assigneeIds, setAssigneeIds] = useState<string[]>([])
  const [linkedPullRequestIds, setLinkedPullRequestIds] = useState<string[]>([])
  const [milestoneId, setMilestoneId] = useState('')
  const [displayName, setDisplayName] = usePersistentDisplayName('qit.issue.display_name')
  const [formError, setFormError] = useState<string | null>(null)
  const [isSubmitting, setIsSubmitting] = useState(false)

  useEffect(() => {
    if (!open) {
      return
    }
    setTitle('')
    setDescription('')
    setLabelIds([])
    setAssigneeIds([])
    setLinkedPullRequestIds([])
    setMilestoneId('')
    setFormError(null)
    setIsSubmitting(false)
  }, [open])

  const titleError = !title.trim() ? 'Title is required.' : null
  const needsDisplayName = !bootstrap.principal
  const displayNameError = needsDisplayName && !displayName.trim() ? 'Display name is required.' : null
  const labelOptions = useMemo(
    () => metadata.labels.map((label: IssueLabel) => ({ id: label.id, label: label.name })),
    [metadata.labels],
  )
  const assigneeOptions = useMemo(
    () =>
      metadata.assignees.map((assignee: IssueAssigneeView) => ({
        id: assignee.id,
        label: `${assignee.username} (${assignee.name})`,
      })),
    [metadata.assignees],
  )
  const milestoneOptions = useMemo(
    () => metadata.milestones.map((milestone: IssueMilestone) => ({ id: milestone.id, label: milestone.title })),
    [metadata.milestones],
  )
  const pullRequestOptions = useMemo(
    () =>
      pullRequests.map((pullRequest) => ({
        id: pullRequest.id,
        label: `${pullRequest.title} (${pullRequest.source_branch} -> ${pullRequest.target_branch})`,
      })),
    [pullRequests],
  )

  const isValid = !titleError && !displayNameError

  return (
    <ModalShell
      onClose={() => {
        if (!isSubmitting) {
          onClose()
        }
      }}
      open={open}
      title="New issue"
    >
      <div className="space-y-5">
        <TextInput
          autoFocus
          error={titleError}
          label="Title"
          onChange={setTitle}
          placeholder="Briefly describe the problem or request"
          value={title}
        />
        <TextArea
          label="Description"
          onChange={setDescription}
          rows={8}
          value={description}
        />
        {needsDisplayName ? (
          <TextInput
            error={displayNameError}
            label="Display name"
            onChange={setDisplayName}
            placeholder="How should your comments appear?"
            value={displayName}
          />
        ) : null}
        <MultiSelect
          emptyLabel="Create labels first to tag this issue."
          label="Labels"
          onChange={setLabelIds}
          options={labelOptions}
          value={labelIds}
        />
        <MultiSelect
          emptyLabel="No active collaborators are available to assign."
          label="Assignees"
          onChange={setAssigneeIds}
          options={assigneeOptions}
          value={assigneeIds}
        />
        <div className="space-y-2">
          <label className="block text-sm font-medium text-fg" htmlFor="create-issue-milestone">
            Milestone
          </label>
          <select
            className="w-full rounded-token border border-border bg-panel-subtle px-3.5 py-2.5 text-sm text-fg outline-none transition-colors focus:border-accent focus:ring-2 focus:ring-accent/20"
            id="create-issue-milestone"
            onChange={(event) => setMilestoneId(event.target.value)}
            value={milestoneId}
          >
            <option value="">No milestone</option>
            {milestoneOptions.map((milestone) => (
              <option key={milestone.id} value={milestone.id}>
                {milestone.label}
              </option>
            ))}
          </select>
        </div>
        <MultiSelect
          emptyLabel="There are no pull requests to link yet."
          label="Linked pull requests"
          onChange={setLinkedPullRequestIds}
          options={pullRequestOptions}
          value={linkedPullRequestIds}
        />
        <FieldError message={formError} />
        <FormActions
          hint="Create the issue with markdown, metadata, and any initial pull-request links. References like PR ids can be linked later too."
        >
          <Button disabled={isSubmitting} onClick={onClose} tone="muted">
            Cancel
          </Button>
          <Button
            disabled={!isValid || isSubmitting}
            icon={
              isSubmitting ? <LoaderCircle className="h-4 w-4 animate-spin" /> : <CircleDot className="h-4 w-4" strokeWidth={1.9} />
            }
            onClick={() => {
              if (!isValid) {
                setFormError(titleError ?? displayNameError ?? 'Issue details are incomplete.')
                return
              }
              setIsSubmitting(true)
              setFormError(null)
              void onCreateIssue({
                title: title.trim(),
                description: description.trim(),
                display_name: needsDisplayName ? displayName.trim() : null,
                label_ids: labelIds,
                assignee_user_ids: assigneeIds,
                milestone_id: milestoneId || null,
                linked_pull_request_ids: linkedPullRequestIds,
              })
                .then(() => onClose())
                .catch((error) => {
                  setFormError(error instanceof Error ? error.message : 'Unable to create the issue.')
                })
                .finally(() => setIsSubmitting(false))
            }}
          >
            Create issue
          </Button>
        </FormActions>
      </div>
    </ModalShell>
  )
}
