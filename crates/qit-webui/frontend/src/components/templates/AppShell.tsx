import { Menu, MenuButton, MenuItem, MenuItems, Tab, TabGroup, TabList, TabPanel, TabPanels } from '@headlessui/react'
import { Bell, CheckCircle2, GitBranch, Globe, X } from 'lucide-react'
import { useState, type ReactNode } from 'react'
import { Badge, Button } from '../atoms/Controls'
import { BrandLogo } from '../atoms/BrandLogo'

export interface ShellTab {
  id: string
  label: string
  count?: number
  icon?: ReactNode
  action?: ReactNode
  content: ReactNode
  hidden?: boolean
}

export interface HeaderAlert {
  id: string
  title: string
  detail: string
  primaryAction?: {
    label: string
    onSelect: () => Promise<void>
    icon?: ReactNode
  }
  secondaryAction?: {
    label: string
    onSelect: () => Promise<void>
    tone?: 'danger' | 'muted'
    icon?: ReactNode
  }
}

function AlertsMenu({
  alerts,
}: {
  alerts: HeaderAlert[]
}) {
  const [pendingAction, setPendingAction] = useState<{ id: string; kind: 'approve' | 'reject' } | null>(null)
  const alertCount = alerts.length

  return (
    <Menu as="div" className="relative">
      <MenuButton
        aria-label={alertCount ? `${alertCount} pending alerts` : 'Alerts'}
        className="relative inline-flex h-10 w-10 items-center justify-center rounded-token text-fg-muted outline-none transition hover:bg-panel-subtle hover:text-fg focus-visible:ring-2 focus-visible:ring-accent/30 focus-visible:ring-offset-2 focus-visible:ring-offset-canvas data-active:bg-panel-subtle data-active:text-fg"
      >
        <Bell className="h-4.5 w-4.5" strokeWidth={1.9} />
        {alertCount ? (
          <span className="absolute -right-1 -top-1 inline-flex min-w-5 items-center justify-center rounded-full border border-danger/40 bg-danger px-1.5 py-0.5 text-[10px] font-semibold text-canvas">
            {alertCount}
          </span>
        ) : null}
      </MenuButton>
      <MenuItems
        anchor="bottom end"
        className="z-30 mt-2 w-[24rem] rounded-token border border-border bg-panel p-2 shadow-panel outline-none"
      >
        <div className="border-b border-border/80 px-3 py-2">
          <p className="text-sm font-semibold text-fg">Alerts</p>
          <p className="text-xs text-fg-subtle">
            {alertCount
              ? `${alertCount} item${alertCount === 1 ? '' : 's'} waiting for attention.`
              : 'Important updates will appear here.'}
          </p>
        </div>
        <div className="max-h-96 space-y-2 overflow-y-auto p-2">
          {alerts.length ? (
            alerts.map((alert) => {
              const isPrimaryPending = pendingAction?.id === alert.id && pendingAction.kind === 'approve'
              const isSecondaryPending = pendingAction?.id === alert.id && pendingAction.kind === 'reject'

              return (
                <div className="rounded-token border border-border/80 bg-panel-subtle px-3 py-3" key={alert.id}>
                  <div className="space-y-1">
                    <p className="text-sm font-medium text-fg">{alert.title}</p>
                    <p className="text-xs text-fg-subtle">{alert.detail}</p>
                  </div>
                  {alert.primaryAction || alert.secondaryAction ? (
                    <div className="mt-3 flex gap-2">
                      {alert.primaryAction ? (
                        <MenuItem>
                          <Button
                            className="flex-1"
                            disabled={Boolean(pendingAction)}
                            icon={alert.primaryAction.icon ?? <CheckCircle2 className="h-4 w-4" strokeWidth={1.9} />}
                            onClick={async () => {
                              setPendingAction({ id: alert.id, kind: 'approve' })
                              try {
                                await alert.primaryAction?.onSelect()
                              } finally {
                                setPendingAction(null)
                              }
                            }}
                          >
                            {isPrimaryPending ? `${alert.primaryAction.label}…` : alert.primaryAction.label}
                          </Button>
                        </MenuItem>
                      ) : null}
                      {alert.secondaryAction ? (
                        <MenuItem>
                          <Button
                            className="flex-1"
                            disabled={Boolean(pendingAction)}
                            icon={alert.secondaryAction.icon ?? <X className="h-4 w-4" strokeWidth={1.9} />}
                            onClick={async () => {
                              setPendingAction({ id: alert.id, kind: 'reject' })
                              try {
                                await alert.secondaryAction?.onSelect()
                              } finally {
                                setPendingAction(null)
                              }
                            }}
                            tone={alert.secondaryAction.tone ?? 'danger'}
                          >
                            {isSecondaryPending ? `${alert.secondaryAction.label}…` : alert.secondaryAction.label}
                          </Button>
                        </MenuItem>
                      ) : null}
                    </div>
                  ) : null}
                </div>
              )
            })
          ) : (
            <div className="rounded-token border border-dashed border-border/80 bg-panel-subtle px-4 py-6 text-center text-sm text-fg-muted">
              No active alerts.
            </div>
          )}
        </div>
      </MenuItems>
    </Menu>
  )
}

export function AppShell({
  actor,
  repoName,
  repoDescription,
  repoHomepageUrl,
  checkedOutBranch,
  exportedBranch,
  branchCount,
  pullRequestCount,
  alerts,
  sessionControl,
  tabs,
  selectedTabId,
  onSelectTab,
}: {
  actor: string
  repoName: string
  repoDescription?: string
  repoHomepageUrl?: string | null
  checkedOutBranch: string
  exportedBranch: string
  branchCount: number
  pullRequestCount: number
  alerts?: HeaderAlert[]
  sessionControl?: ReactNode
  tabs: ShellTab[]
  selectedTabId: string
  onSelectTab: (id: string) => void
}) {
  const selectedIndex = Math.max(
    0,
    tabs.findIndex((tab) => tab.id === selectedTabId),
  )
  const selectedTab = tabs[selectedIndex]

  return (
    <div className="min-h-screen bg-canvas text-fg">
      <a className="skip-link" href="#qit-main">
        Skip to content
      </a>
      <header className="border-b border-border/80 bg-canvas-raised/90 backdrop-blur">
        <div className="mx-auto max-w-7xl px-5 sm:px-6">
          <div className="flex flex-col gap-6 py-5">
            <div className="flex flex-wrap items-center justify-between gap-3">
              <div className="flex min-w-0 items-center gap-3 sm:gap-4">
                <BrandLogo className="h-11 sm:h-12" />
                <div className="hidden h-8 w-px shrink-0 bg-border sm:block" />
                <span className="block min-w-0 truncate text-sm font-medium text-fg-muted">
                  {repoName}
                </span>
              </div>
              <div className="flex items-center gap-2">
                {alerts ? <AlertsMenu alerts={alerts} /> : null}
                {sessionControl ?? (
                  <span className="rounded-full border border-border bg-panel px-2.5 py-1 text-xs font-semibold text-fg-muted">
                    {actor === 'owner' ? 'Owner session' : 'Shared session'}
                  </span>
                )}
              </div>
            </div>

            <div className="flex flex-col gap-4 lg:flex-row lg:items-end lg:justify-between">
              <div className="space-y-2">
                <h1 className="text-4xl font-semibold tracking-tight text-fg sm:text-5xl">{repoName}</h1>
                {repoDescription ? (
                  <p className="max-w-2xl text-sm leading-6 text-fg-muted sm:text-base">{repoDescription}</p>
                ) : null}
                <div className="flex flex-wrap items-center gap-2 text-sm text-fg-muted">
                  <span>{branchCount} branches</span>
                  <span className="text-fg-subtle">•</span>
                  <span>{pullRequestCount} pull requests</span>
                  {repoHomepageUrl ? (
                    <>
                      <span className="text-fg-subtle">•</span>
                      <a className="text-accent hover:text-accent-strong" href={repoHomepageUrl} rel="noreferrer" target="_blank">
                        Homepage
                      </a>
                    </>
                  ) : null}
                </div>
              </div>

              <div className="flex flex-wrap items-center gap-2 text-sm text-fg-muted">
                <Badge icon={<GitBranch className="h-3.5 w-3.5" strokeWidth={1.85} />} tone="accent">
                  Checked out: {checkedOutBranch}
                </Badge>
                <Badge icon={<Globe className="h-3.5 w-3.5" strokeWidth={1.85} />} tone="success">
                  Served: {exportedBranch}
                </Badge>
              </div>
            </div>
          </div>
        </div>
      </header>

      <main className="mx-auto max-w-7xl px-5 pt-3 pb-6 sm:px-6 sm:pt-4 sm:pb-8" id="qit-main">
        <TabGroup
          onChange={(index) => {
            const nextTab = tabs[index]
            if (nextTab) {
              onSelectTab(nextTab.id)
            }
          }}
          selectedIndex={selectedIndex}
        >
          <div className="flex flex-col gap-3 border-b border-border/80 pb-3 sm:gap-4 sm:pb-4">
            <TabList className="flex min-w-0 gap-1 overflow-x-auto">
              {tabs.map((tab) => (
                <Tab
                  className={
                    tab.hidden
                      ? 'hidden'
                      : 'group flex shrink-0 items-center gap-2 rounded-token border border-transparent px-3 py-2 text-sm font-semibold text-fg-muted outline-none transition hover:border-border hover:bg-panel-subtle hover:text-fg data-selected:border-accent/35 data-selected:bg-accent/10 data-selected:text-fg'
                  }
                  key={tab.id}
                >
                  {tab.icon ? (
                    <span aria-hidden="true" className="shrink-0 text-fg-subtle transition group-data-selected:text-accent">
                      {tab.icon}
                    </span>
                  ) : null}
                  <span>{tab.label}</span>
                  {typeof tab.count === 'number' ? (
                    <span className="rounded-full border border-border bg-canvas px-2 py-0.5 text-[11px] text-fg-subtle transition group-data-selected:border-accent/30 group-data-selected:bg-accent/10 group-data-selected:text-fg">
                      {tab.count}
                    </span>
                  ) : null}
                </Tab>
              ))}
            </TabList>
            {selectedTab?.action ? <div className="flex items-center justify-start sm:justify-end">{selectedTab.action}</div> : null}
          </div>
          <TabPanels className="mt-6">
            {tabs.map((tab) => (
              <TabPanel className="outline-none" key={tab.id}>
                {tab.content}
              </TabPanel>
            ))}
          </TabPanels>
        </TabGroup>
      </main>
    </div>
  )
}
