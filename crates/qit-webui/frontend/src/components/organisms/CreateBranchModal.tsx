import { useEffect, useState } from 'react'
import { GitBranchPlus } from 'lucide-react'
import type { BootstrapResponse } from '../../lib/types'
import { Button, Spinner } from '../atoms/Controls'
import { ModalShell } from '../atoms/ModalShell'
import { FieldError, FormActions, TextInput } from '../molecules/Fields'

export function CreateBranchModal({
  open,
  bootstrap,
  onClose,
  onCreateBranch,
}: {
  open: boolean
  bootstrap: BootstrapResponse
  onClose: () => void
  onCreateBranch: (name: string, startPoint: string, force: boolean) => Promise<void>
}) {
  const [branchName, setBranchName] = useState('')
  const [branchStartPoint, setBranchStartPoint] = useState('')
  const [forceBranch, setForceBranch] = useState(false)
  const [isSubmitting, setIsSubmitting] = useState(false)
  const [formError, setFormError] = useState<string | null>(null)

  useEffect(() => {
    if (!open) {
      return
    }

    setBranchName('')
    setBranchStartPoint('')
    setForceBranch(false)
    setFormError(null)
    setIsSubmitting(false)
  }, [open])

  return (
    <ModalShell
      allowClose={!isSubmitting}
      description="Create a branch from the current worktree branch or another starting point."
      onClose={onClose}
      open={open}
      size="sm"
      title="Create branch"
    >
      <form
        className="space-y-4"
        onSubmit={async (event) => {
          event.preventDefault()
          setFormError(null)
          setIsSubmitting(true)

          try {
            await onCreateBranch(branchName.trim(), branchStartPoint.trim(), forceBranch)
            onClose()
          } catch (error) {
            setFormError(error instanceof Error ? error.message : 'Failed to create branch.')
          } finally {
            setIsSubmitting(false)
          }
        }}
      >
        <TextInput
          autoFocus
          description="Use a branch name that makes the work easy to recognize later."
          disabled={isSubmitting}
          label="Branch name"
          onChange={setBranchName}
          placeholder="feature/new-branch"
          required
          value={branchName}
        />
        <TextInput
          description="Leave blank to branch from the checked-out branch."
          disabled={isSubmitting}
          label="Start point"
          onChange={setBranchStartPoint}
          placeholder={bootstrap.checked_out_branch}
          value={branchStartPoint}
        />
        <label className="flex items-start gap-3 rounded-token border border-border bg-panel-subtle px-3 py-3 text-sm text-fg-muted">
          <input
            checked={forceBranch}
            className="mt-0.5 h-4 w-4 rounded border-border bg-panel-subtle"
            disabled={isSubmitting}
            onChange={(event) => setForceBranch(event.target.checked)}
            type="checkbox"
          />
          <span>
            Replace the branch if it already exists.
          </span>
        </label>
        <FieldError message={formError} />
        <FormActions>
          <Button disabled={isSubmitting} onClick={onClose} tone="muted">
            Cancel
          </Button>
          <Button
            disabled={isSubmitting || !branchName.trim()}
            icon={isSubmitting ? <Spinner /> : <GitBranchPlus className="h-4 w-4" strokeWidth={1.9} />}
            type="submit"
          >
            {isSubmitting ? 'Creating...' : 'Create branch'}
          </Button>
        </FormActions>
      </form>
    </ModalShell>
  )
}
