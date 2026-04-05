import type { ReactNode } from 'react'
import { Menu, MenuButton, MenuItem, MenuItems } from '@headlessui/react'
import { Check, Code2, EllipsisVertical, Globe, Trash2 } from 'lucide-react'
import { Badge, IconButton, Panel } from '../../atoms/Controls'
import type { BranchInfo } from '../../../lib/types'
import { shortSha } from './panelUtils'

export function BranchesPanel({
  branches,
  canEdit,
  headerAction,
  onOpen,
  onSwitch,
  onCheckout,
  onDelete,
}: {
  branches: BranchInfo[]
  canEdit: boolean
  headerAction?: ReactNode
  onOpen: (name: string) => void
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
            <button
              aria-label={`Browse code on ${branch.name}`}
              className="min-w-0 flex-1 rounded-token text-left outline-none transition hover:bg-panel/70 focus-visible:ring-2 focus-visible:ring-accent/30"
              onClick={() => onOpen(branch.name)}
              type="button"
            >
              <div className="space-y-2 px-1 py-1">
                <div className="flex flex-wrap items-center gap-2">
                  <p className="text-base font-semibold text-fg">{branch.name}</p>
                  {branch.is_current ? <Badge tone="accent">Checked out</Badge> : null}
                  {branch.is_served ? <Badge tone="success">Served</Badge> : null}
                  <Badge icon={<Code2 className="h-3.5 w-3.5" strokeWidth={1.9} />} tone="muted">
                    View code
                  </Badge>
                </div>
                <div className="flex flex-wrap items-center gap-3 text-sm text-fg-muted">
                  <span className="truncate">{branch.summary || 'No commit summary available.'}</span>
                  <span className="font-mono text-xs text-fg-subtle">{shortSha(branch.commit)}</span>
                </div>
              </div>
            </button>

            <div className="flex flex-wrap items-center gap-2">
              {canEdit ? (
                <>
                  {!branch.is_current || !branch.is_served ? (
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
                        {!branch.is_current ? (
                          <MenuItem>
                            <button
                              className="flex w-full items-center gap-2 rounded-token px-3 py-2 text-left text-sm text-fg data-focus:bg-panel-subtle"
                              onClick={() => onCheckout(branch.name)}
                              type="button"
                            >
                              <Check aria-hidden="true" className="h-4 w-4 shrink-0" strokeWidth={1.9} />
                              Check out
                            </button>
                          </MenuItem>
                        ) : null}
                        {!branch.is_served ? (
                          <MenuItem>
                            <button
                              className="flex w-full items-center gap-2 rounded-token px-3 py-2 text-left text-sm text-fg data-focus:bg-panel-subtle"
                              onClick={() => onSwitch(branch.name)}
                              type="button"
                            >
                              <Globe aria-hidden="true" className="h-4 w-4 shrink-0" strokeWidth={1.9} />
                              Serve
                            </button>
                          </MenuItem>
                        ) : null}
                        {!branch.is_current && !branch.is_served ? (
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
                        ) : null}
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
