import type { ReactNode } from 'react'
import { Menu, MenuButton, MenuItem, MenuItems } from '@headlessui/react'
import { Check, EllipsisVertical, Globe, Trash2 } from 'lucide-react'
import { Badge, Button, IconButton, Panel } from '../../atoms/Controls'
import type { BranchInfo } from '../../../lib/types'
import { shortSha } from './panelUtils'

export function BranchesPanel({
  branches,
  canEdit,
  headerAction,
  onSwitch,
  onCheckout,
  onDelete,
}: {
  branches: BranchInfo[]
  canEdit: boolean
  headerAction?: ReactNode
  onSwitch: (name: string) => void
  onCheckout: (name: string) => void
  onDelete: (name: string) => void
}) {
  return (
    <Panel
      action={headerAction}
      subtitle="Keep the checked-out branch, served branch, and next actions visible without opening a menu for every common task."
      title="Branches"
    >
      <div className="space-y-3">
        {branches.map((branch) => (
          <div className="flex flex-col gap-3 rounded-token border border-border bg-panel-subtle px-4 py-4 lg:flex-row lg:items-center lg:justify-between" key={branch.name}>
            <div className="min-w-0 flex-1 space-y-2">
              <div className="flex flex-wrap items-center gap-2">
                <p className="text-base font-semibold text-fg">{branch.name}</p>
                {branch.is_current ? <Badge tone="accent">Checked out</Badge> : null}
                {branch.is_served ? <Badge tone="success">Served</Badge> : null}
              </div>
              <div className="flex flex-wrap items-center gap-3 text-sm text-fg-muted">
                <span className="truncate">{branch.summary || 'No commit summary available.'}</span>
                <span className="font-mono text-xs text-fg-subtle">{shortSha(branch.commit)}</span>
              </div>
            </div>

            <div className="flex flex-wrap items-center gap-2">
              {canEdit ? (
                <>
                  {!branch.is_current ? (
                    <Button icon={<Check className="h-4 w-4" strokeWidth={1.9} />} onClick={() => onCheckout(branch.name)} tone="muted">
                      Check out
                    </Button>
                  ) : null}
                  {!branch.is_served ? (
                    <Button icon={<Globe className="h-4 w-4" strokeWidth={1.9} />} onClick={() => onSwitch(branch.name)} tone="muted">
                      Serve
                    </Button>
                  ) : null}
                  {!branch.is_current && !branch.is_served ? (
                    <Menu as="div" className="relative">
                      <MenuButton
                        aria-label={`Branch actions for ${branch.name}`}
                        as={IconButton}
                        className="border-transparent bg-transparent text-fg-subtle hover:border-transparent hover:bg-panel hover:text-fg"
                        icon={<EllipsisVertical className="h-4 w-4" strokeWidth={1.9} />}
                        label={`Branch actions for ${branch.name}`}
                        tone="muted"
                      />
                      <MenuItems anchor="bottom end" className="z-20 mt-2 w-48 rounded-token border border-border bg-panel p-2 shadow-panel">
                        <MenuItem>
                          <button
                            className="flex w-full items-center gap-2 rounded-token px-3 py-2 text-left text-sm text-danger data-focus:bg-danger/12"
                            onClick={() => onDelete(branch.name)}
                            type="button"
                          >
                            <Trash2 aria-hidden="true" className="h-4 w-4 shrink-0" strokeWidth={1.9} />
                            Delete branch
                          </button>
                        </MenuItem>
                      </MenuItems>
                    </Menu>
                  ) : null}
                </>
              ) : (
                <Badge tone="muted">Owner only</Badge>
              )}
            </div>
          </div>
        ))}
      </div>
    </Panel>
  )
}
