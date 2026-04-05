import type { ReactNode } from 'react'
import { Tab, TabGroup, TabList, TabPanel, TabPanels } from '@headlessui/react'
import { GitBranch, Globe } from 'lucide-react'
import { Badge } from '../atoms/Controls'
import { BrandLogo } from '../atoms/BrandLogo'

export interface ShellTab {
  id: string
  label: string
  count?: number
  icon?: ReactNode
  action?: ReactNode
  content: ReactNode
}

export function AppShell({
  actor,
  repoName,
  checkedOutBranch,
  exportedBranch,
  branchCount,
  pullRequestCount,
  tabs,
  selectedTabId,
  onSelectTab,
}: {
  actor: string
  repoName: string
  checkedOutBranch: string
  exportedBranch: string
  branchCount: number
  pullRequestCount: number
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
      <header className="border-b border-border/80 bg-canvas-raised/90 backdrop-blur">
        <div className="mx-auto max-w-7xl px-5 sm:px-6">
          <div className="flex flex-col gap-6 py-5">
            <div className="flex flex-wrap items-center justify-between gap-3">
              <div className="flex min-w-0 items-center gap-3 sm:gap-4">
                <BrandLogo className="h-11 sm:h-12" />
                <div className="hidden h-8 w-px shrink-0 bg-border sm:block" />
                <div className="min-w-0 flex-wrap items-center gap-2 text-sm sm:flex">
                  <span className="hidden font-medium text-fg sm:inline">Repository</span>
                  <span className="hidden text-fg-subtle sm:inline">/</span>
                  <span className="block truncate font-medium text-fg-muted">{repoName}</span>
                </div>
              </div>
              <span className="rounded-full border border-border bg-panel px-2.5 py-1 text-xs font-semibold text-fg-muted">
                {actor === 'owner' ? 'Owner session' : 'Shared session'}
              </span>
            </div>

            <div className="flex flex-col gap-4 lg:flex-row lg:items-end lg:justify-between">
              <div className="space-y-2">
                <div className="flex flex-wrap items-center gap-2 text-sm text-fg-muted">
                  <span>Repository</span>
                  <span className="text-fg-subtle">/</span>
                  <span className="font-medium text-fg">{repoName}</span>
                </div>
                <h1 className="text-4xl font-semibold tracking-tight text-fg sm:text-5xl">{repoName}</h1>
                <p className="max-w-2xl text-sm leading-6 text-fg-muted sm:text-base">
                  Browse the published snapshot, inspect branch and pull-request state, and copy the exact clone credentials for this session.
                </p>
                <div className="flex flex-wrap items-center gap-2 text-sm text-fg-muted">
                  <span>{branchCount} branches</span>
                  <span className="text-fg-subtle">•</span>
                  <span>{pullRequestCount} pull requests</span>
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

      <main className="mx-auto max-w-7xl px-5 py-6 sm:px-6 sm:py-8">
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
                  className="group flex shrink-0 items-center gap-2 rounded-token border border-transparent px-3 py-2 text-sm font-semibold text-fg-muted outline-none transition hover:border-border hover:bg-panel-subtle hover:text-fg data-selected:border-accent/35 data-selected:bg-accent/10 data-selected:text-fg"
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
