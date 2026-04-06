import { useEffect, useMemo, useRef, useState } from 'react'
import { GitPullRequestCreateArrow, LoaderCircle } from 'lucide-react'
import { api } from '../../lib/api'
import type { BootstrapResponse, BranchInfo, PullRequestRecord, RefComparison } from '../../lib/types'
import { Button, Spinner } from '../atoms/Controls'
import { ModalShell } from '../atoms/ModalShell'
import { BranchCombobox } from '../molecules/BranchCombobox'
import { FieldError, FormActions, TextArea, TextInput } from '../molecules/Fields'

function toTitleCase(value: string) {
  return value.replace(/\b\w/g, (character) => character.toUpperCase())
}

function defaultTitleForBranch(branchName: string) {
  const normalized = branchName
    .split('/')
    .filter(Boolean)
    .join(' ')
    .replace(/[-_]+/g, ' ')
    .trim()

  return normalized ? toTitleCase(normalized) : ''
}

function shortSha(value: string) {
  return value.slice(0, 7)
}

function isMacPlatform() {
  return typeof navigator !== 'undefined' && navigator.platform.toLowerCase().includes('mac')
}

function PreviewSkeleton() {
  return (
    <div className="space-y-3 rounded-token border border-border bg-panel px-4 py-4">
      <div className="flex flex-wrap gap-3">
        <div className="h-10 w-28 animate-pulse rounded-token bg-panel-subtle" />
        <div className="h-10 w-28 animate-pulse rounded-token bg-panel-subtle" />
      </div>
      <div className="space-y-2">
        <div className="h-12 animate-pulse rounded-token bg-panel-subtle" />
        <div className="h-12 animate-pulse rounded-token bg-panel-subtle" />
      </div>
    </div>
  )
}

function ComparisonPreview({
  comparison,
  loading,
  error,
}: {
  comparison: RefComparison | null
  loading: boolean
  error: string | null
}) {
  if (loading) {
    return <PreviewSkeleton />
  }

  if (error) {
    return (
      <div className="rounded-token border border-danger/40 bg-danger/12 px-4 py-3 text-sm text-danger">
        {error}
      </div>
    )
  }

  if (!comparison) {
    return (
      <div className="rounded-token border border-dashed border-border bg-panel-subtle px-4 py-4 text-sm text-fg-muted">
        Select valid source and target branches to preview the commit range before you open the pull request.
      </div>
    )
  }

  return (
    <div className="space-y-3 rounded-token border border-border bg-panel px-4 py-4">
      <div className="flex flex-wrap gap-3">
        <div className="min-w-28 rounded-token border border-border bg-panel-subtle px-3 py-2">
          <p className="text-[11px] font-semibold uppercase tracking-[0.14em] text-fg-subtle">Ahead</p>
          <p className="mt-1 text-lg font-semibold text-fg">{comparison.ahead_by}</p>
        </div>
        <div className="min-w-28 rounded-token border border-border bg-panel-subtle px-3 py-2">
          <p className="text-[11px] font-semibold uppercase tracking-[0.14em] text-fg-subtle">Behind</p>
          <p className="mt-1 text-lg font-semibold text-fg">{comparison.behind_by}</p>
        </div>
      </div>
      <div className="rounded-token border border-border bg-panel-subtle px-3 py-3">
        <div className="flex flex-wrap items-center gap-2 text-sm text-fg-muted">
          <span className="font-medium text-fg">{comparison.head_ref}</span>
            <span aria-hidden="true">to</span>
          <span>{comparison.base_ref}</span>
        </div>
        {comparison.merge_base ? (
          <p className="mt-2 text-xs text-fg-subtle">Merge base {shortSha(comparison.merge_base)}</p>
        ) : null}
      </div>
      <div className="space-y-2">
        {comparison.commits.length === 0 ? (
          <div className="rounded-token border border-border bg-panel-subtle px-3 py-3 text-sm text-fg-muted">
            These branches currently resolve to the same commit set.
          </div>
        ) : (
          comparison.commits.slice(0, 3).map((commit) => (
            <div
              className="rounded-token border border-border bg-panel-subtle px-3 py-3 transition duration-150"
              key={commit.id}
            >
              <p className="text-sm font-medium text-fg">{commit.summary || commit.id}</p>
              <div className="mt-2 flex flex-wrap items-center justify-between gap-3 text-xs text-fg-muted">
                <span>{commit.author}</span>
                <span className="font-mono">{shortSha(commit.id)}</span>
              </div>
            </div>
          ))
        )}
        {comparison.commits.length > 3 ? (
          <p className="text-xs text-fg-subtle">
            {comparison.commits.length - 3} more commit{comparison.commits.length - 3 === 1 ? '' : 's'} in this range.
          </p>
        ) : null}
      </div>
    </div>
  )
}

export function CreatePullRequestModal({
  open,
  bootstrap,
  branches,
  onClose,
  onCreatePullRequest,
}: {
  open: boolean
  bootstrap: BootstrapResponse
  branches: BranchInfo[]
  onClose: () => void
  onCreatePullRequest: (payload: {
    title: string
    description: string
    source_branch: string
    target_branch: string
  }) => Promise<PullRequestRecord>
}) {
  const formRef = useRef<HTMLFormElement>(null)
  const [prTitle, setPrTitle] = useState('')
  const [prDescription, setPrDescription] = useState('')
  const [prSource, setPrSource] = useState('')
  const [prTarget, setPrTarget] = useState('')
  const [isSubmitting, setIsSubmitting] = useState(false)
  const [formError, setFormError] = useState<string | null>(null)
  const [previewError, setPreviewError] = useState<string | null>(null)
  const [previewComparison, setPreviewComparison] = useState<RefComparison | null>(null)
  const [previewLoading, setPreviewLoading] = useState(false)
  const [titleManuallyEdited, setTitleManuallyEdited] = useState(false)
  const [touched, setTouched] = useState<{ title: boolean; source: boolean; target: boolean }>({
    title: false,
    source: false,
    target: false,
  })
  const [showValidation, setShowValidation] = useState(false)

  const branchNames = useMemo(() => new Set(branches.map((branch) => branch.name)), [branches])
  const shortcutLabel = isMacPlatform() ? 'Cmd' : 'Ctrl'
  const trimmedTitle = prTitle.trim()
  const trimmedSource = prSource.trim()
  const trimmedTarget = prTarget.trim()
  const sameBranch = trimmedSource !== '' && trimmedTarget !== '' && trimmedSource === trimmedTarget
  const sourceExists = trimmedSource !== '' && branchNames.has(trimmedSource)
  const targetExists = trimmedTarget !== '' && branchNames.has(trimmedTarget)

  const titleError = !trimmedTitle ? 'Title is required.' : null
  const sourceError = !trimmedSource
    ? 'Choose a source branch.'
    : !sourceExists
      ? 'Source branch not found.'
      : sameBranch
        ? 'Source and target branches must be different.'
        : null
  const targetError = !trimmedTarget
    ? 'Choose a target branch.'
    : !targetExists
      ? 'Target branch not found.'
      : sameBranch
        ? 'Source and target branches must be different.'
        : null

  const isFormValid = !titleError && !sourceError && !targetError

  useEffect(() => {
    if (!open) {
      return
    }

    const nextSource = bootstrap.checked_out_branch
    const nextTarget = bootstrap.exported_branch
    setPrSource(nextSource)
    setPrTarget(nextTarget)
    setPrTitle(defaultTitleForBranch(nextSource))
    setPrDescription('')
    setIsSubmitting(false)
    setFormError(null)
    setPreviewError(null)
    setPreviewComparison(null)
    setPreviewLoading(false)
    setTitleManuallyEdited(false)
    setTouched({ title: false, source: false, target: false })
    setShowValidation(false)
  }, [bootstrap.checked_out_branch, bootstrap.exported_branch, open])

  useEffect(() => {
    if (!open || titleManuallyEdited) {
      return
    }

    setPrTitle(defaultTitleForBranch(trimmedSource))
  }, [open, prSource, titleManuallyEdited, trimmedSource])

  useEffect(() => {
    if (!open) {
      return
    }

    if (!sourceExists || !targetExists || sameBranch) {
      setPreviewComparison(null)
      setPreviewError(null)
      setPreviewLoading(false)
      return
    }

    let active = true
    setPreviewLoading(true)
    setPreviewError(null)

    const timeoutId = window.setTimeout(() => {
      void api
        .compare(trimmedTarget, trimmedSource)
        .then((comparison) => {
          if (active) {
            setPreviewComparison(comparison)
          }
        })
        .catch((error) => {
          if (active) {
            setPreviewComparison(null)
            setPreviewError(error instanceof Error ? error.message : 'Unable to preview this comparison.')
          }
        })
        .finally(() => {
          if (active) {
            setPreviewLoading(false)
          }
        })
    }, 250)

    return () => {
      active = false
      window.clearTimeout(timeoutId)
    }
  }, [open, sameBranch, sourceExists, targetExists, trimmedSource, trimmedTarget])

  return (
    <ModalShell
      allowClose={!isSubmitting}
      description="Choose the source and target branches, review the branch gap, and open a pull request with the right context."
      onClose={onClose}
      open={open}
      title="Open pull request"
    >
      <form
        className="space-y-5"
        onKeyDown={(event) => {
          if ((event.metaKey || event.ctrlKey) && event.key === 'Enter') {
            event.preventDefault()
            formRef.current?.requestSubmit()
          }
        }}
        onSubmit={async (event) => {
          event.preventDefault()
          setShowValidation(true)
          setTouched({ title: true, source: true, target: true })

          if (!isFormValid) {
            return
          }

          setFormError(null)
          setIsSubmitting(true)

          try {
            await onCreatePullRequest({
              title: trimmedTitle,
              description: prDescription.trim(),
              source_branch: trimmedSource,
              target_branch: trimmedTarget,
            })
            onClose()
          } catch (error) {
            setFormError(error instanceof Error ? error.message : 'Failed to create pull request.')
          } finally {
            setIsSubmitting(false)
          }
        }}
        ref={formRef}
      >
        <TextInput
          autoFocus
          description="Write the one-line summary reviewers should scan first."
          disabled={isSubmitting}
          error={showValidation || touched.title ? titleError : null}
          label="Title"
          onBlur={() => setTouched((current) => ({ ...current, title: true }))}
          onChange={(value) => {
            setPrTitle(value)
            setTitleManuallyEdited(true)
          }}
          required
          value={prTitle}
        />

        <div className="space-y-3 rounded-token border border-border bg-panel-subtle/60 p-4">
          <div>
            <p className="text-sm font-medium text-fg">Branch selection</p>
            <p className="mt-1 text-sm text-fg-muted">
              Choose the source branch that contains the change and the target branch that should receive it.
            </p>
          </div>

          <div className="grid gap-4 md:grid-cols-2">
            <BranchCombobox
              autoFocus={false}
              branches={branches}
              description="Defaults to the branch currently checked out in this worktree."
              disabled={isSubmitting}
              error={showValidation || touched.source ? sourceError : null}
              label="Source branch"
              onBlur={() => setTouched((current) => ({ ...current, source: true }))}
              onChange={(value) => {
                setPrSource(value)
                if (!titleManuallyEdited) {
                  setPrTitle(defaultTitleForBranch(value))
                }
              }}
              required
              value={prSource}
            />
            <BranchCombobox
              branches={branches}
              description="Defaults to the branch currently being served."
              disabled={isSubmitting}
              error={showValidation || touched.target ? targetError : null}
              label="Target branch"
              onBlur={() => setTouched((current) => ({ ...current, target: true }))}
              onChange={setPrTarget}
              required
              value={prTarget}
            />
          </div>

          {sameBranch ? (
            <div className="rounded-token border border-warning/40 bg-warning/12 px-3 py-3 text-sm text-warning">
              A pull request needs two different branches. Choose a target branch that will receive this work.
            </div>
          ) : null}

          <div className="space-y-3">
            <div className="flex items-center gap-2 text-sm font-medium text-fg">
              {previewLoading ? <LoaderCircle className="h-4 w-4 animate-spin text-accent" strokeWidth={1.9} /> : null}
              <span>Comparison preview</span>
            </div>
            <ComparisonPreview comparison={previewComparison} error={previewError} loading={previewLoading} />
          </div>
        </div>

        <TextArea
          description="Optional context for reviewers, such as intent, constraints, or follow-up work."
          disabled={isSubmitting}
          label="Description"
          onChange={setPrDescription}
          onKeyDown={() => {
            setFormError(null)
          }}
          rows={5}
          value={prDescription}
        />

        <FieldError message={formError} />

        <FormActions hint={`Press ${shortcutLabel}+Enter to create the pull request.`}>
          <Button disabled={isSubmitting} onClick={onClose} tone="muted">
            Cancel
          </Button>
          <Button
            disabled={isSubmitting || !isFormValid}
            icon={isSubmitting ? <Spinner /> : <GitPullRequestCreateArrow className="h-4 w-4" strokeWidth={1.9} />}
            title={`Create pull request (${shortcutLabel}+Enter)`}
            type="submit"
          >
            {isSubmitting ? 'Creating...' : 'Create pull request'}
          </Button>
        </FormActions>
      </form>
    </ModalShell>
  )
}
