import { Menu, MenuButton, MenuItem, MenuItems } from '@headlessui/react'
import { useEffect, useMemo, useState } from 'react'
import {
  Check,
  ChevronDown,
  Code2,
  Copy,
  Eye,
  EyeOff,
  GitBranch,
  GitBranchPlus,
  LogIn,
} from 'lucide-react'
import { BrandLogo } from '../atoms/BrandLogo'
import { Badge, Button, Panel } from '../atoms/Controls'
import { SectionHeader, TextInput } from '../molecules/Fields'
import { CreateBranchModal } from '../organisms/CreateBranchModal'
import { CreatePullRequestModal } from '../organisms/CreatePullRequestModal'
import { BranchesPanel } from '../organisms/panels/BranchesPanel'
import { CodePanel } from '../organisms/panels/CodePanel'
import { CommitsPanel } from '../organisms/panels/CommitsPanel'
import { PullRequestsPanel } from '../organisms/panels/PullRequestsPanel'
import { SettingsPanel } from '../organisms/panels/SettingsPanel'
import { AppShell } from '../templates/AppShell'
import { shellTabIcons } from '../templates/shellTabIcons'
import { api } from '../../lib/api'
import { getQueryParam, mergeQueryParams } from '../../lib/queryState'
import type {
  BlobContent,
  BootstrapResponse,
  BranchInfo,
  CommitDetail,
  CommitHistory,
  PullRequestRecord,
  RefDiffFile,
  SettingsResponse,
  TreeEntry,
  UiRole,
} from '../../lib/types'

function copyToClipboard(value: string) {
  return navigator.clipboard.writeText(value)
}

function CloneMenu({ bootstrap }: { bootstrap: BootstrapResponse }) {
  const [copiedKey, setCopiedKey] = useState<string | null>(null)
  const [passwordVisible, setPasswordVisible] = useState(false)
  const hasVisibleCredentials = bootstrap.git_credentials_visible && !!bootstrap.git_username && !!bootstrap.git_password

  const cloneUrl = bootstrap.public_repo_url ?? `${window.location.origin}${api.baseUrl}`
  const authCloneUrl =
    hasVisibleCredentials
      ? (() => {
          const url = new URL(cloneUrl)
          url.username = bootstrap.git_username!
          url.password = bootstrap.git_password!
          return url.toString()
        })()
      : cloneUrl
  const cloneCommand = `git clone ${authCloneUrl}`

  const markCopied = (key: string) => {
    setCopiedKey(key)
    window.setTimeout(() => {
      setCopiedKey((current) => (current === key ? null : current))
    }, 1500)
  }

  const CopyAction = ({
    label,
    value,
    valueKey,
    secret = false,
  }: {
    label: string
    value: string
    valueKey: string
    secret?: boolean
  }) => {
    const isPassword = secret && valueKey === 'password'
    const inputType = isPassword && !passwordVisible ? 'password' : 'text'

    return (
      <div className="space-y-2">
        <p className="text-[11px] font-semibold uppercase tracking-[0.14em] text-fg-subtle">{label}</p>
        <div className="relative">
          <input
            className="w-full rounded-token border border-border bg-panel-subtle px-3.5 py-2.5 pr-20 text-sm text-fg outline-none"
            readOnly
            type={inputType}
            value={value}
          />
          <div className="absolute inset-y-0 right-2 flex items-center gap-1">
            {isPassword ? (
              <button
                aria-label={passwordVisible ? 'Hide password' : 'Show password'}
                className="inline-flex h-8 w-8 items-center justify-center rounded-token text-fg-muted transition hover:bg-panel-subtle hover:text-fg"
                onClick={() => setPasswordVisible((visible) => !visible)}
                title={passwordVisible ? 'Hide password' : 'Show password'}
                type="button"
              >
                {passwordVisible ? (
                  <EyeOff className="h-4 w-4" strokeWidth={1.9} />
                ) : (
                  <Eye className="h-4 w-4" strokeWidth={1.9} />
                )}
              </button>
            ) : null}
            <button
              aria-label={`Copy ${label.toLowerCase()}`}
              className="inline-flex h-8 w-8 items-center justify-center rounded-token text-fg-muted transition hover:bg-panel-subtle hover:text-fg"
              onClick={() => {
                void copyToClipboard(value).then(() => markCopied(valueKey))
              }}
              title={`Copy ${label.toLowerCase()}`}
              type="button"
            >
              {copiedKey === valueKey ? (
                <Check className="h-4 w-4" strokeWidth={1.9} />
              ) : (
                <Copy className="h-4 w-4" strokeWidth={1.9} />
              )}
            </button>
          </div>
        </div>
      </div>
    )
  }

  return (
    <Menu as="div" className="relative">
      <MenuButton
        as={Button}
        icon={<Code2 className="h-4 w-4" strokeWidth={1.9} />}
        iconPosition="leading"
        tone="primary"
      >
        <span className="inline-flex items-center gap-1.5">
          <span>Clone</span>
          <ChevronDown className="h-4 w-4" strokeWidth={1.9} />
        </span>
      </MenuButton>
      <MenuItems
        anchor="bottom end"
        className="z-30 mt-2 flex w-[min(28rem,calc(100vw-1.5rem))] max-w-[calc(100vw-1.5rem)] flex-col gap-3 rounded-lg border border-border bg-panel p-3 shadow-(--shadow-raised) outline-none"
      >
        <div className="border-b border-border/80 px-1 pb-3">
          <div className="flex items-center gap-2 text-sm font-semibold text-fg">
            <GitBranch className="h-4 w-4 text-success" strokeWidth={1.85} />
            <span>Clone this repository</span>
          </div>
          <p className="mt-1 text-sm text-fg-muted">
            Copy the served URL and a ready-to-run clone command for this session.
          </p>
          <p className="mt-2 text-xs text-fg-subtle">Served branch: {bootstrap.exported_branch}</p>
          {!hasVisibleCredentials ? (
            <p className="mt-2 text-xs text-fg-subtle">
              Git credentials stay with the owner session. Ask the operator for the startup username and password if you need clone or push access.
            </p>
          ) : null}
        </div>

        <MenuItem>
          <div>
            <CopyAction label="HTTPS URL" value={cloneUrl} valueKey="url" />
          </div>
        </MenuItem>

        {hasVisibleCredentials ? (
          <MenuItem>
            <div>
              <CopyAction label="Username" value={bootstrap.git_username!} valueKey="username" />
            </div>
          </MenuItem>
        ) : null}

        {hasVisibleCredentials ? (
          <MenuItem>
            <div>
              <CopyAction label="Password" secret value={bootstrap.git_password!} valueKey="password" />
            </div>
          </MenuItem>
        ) : null}

        <MenuItem>
          <div>
            <CopyAction label="Clone command" value={cloneCommand} valueKey="command" />
          </div>
        </MenuItem>
      </MenuItems>
    </Menu>
  )
}

export function LoginPage({
  error,
  onLogin,
}: {
  error: string | null
  onLogin: (username: string, password: string) => Promise<void>
}) {
  const [username, setUsername] = useState('')
  const [password, setPassword] = useState('')

  return (
    <div className="flex min-h-screen items-center justify-center bg-canvas px-6">
      <div className="w-full max-w-md space-y-6">
        <div className="flex justify-center">
          <BrandLogo className="h-24 sm:h-28" />
        </div>
        <SectionHeader
          eyebrow="Qit"
          title="Sign in to this session"
          detail="Public sessions use the temporary Git credentials printed when the server started, then convert that sign-in into a same-origin web session."
        />
        <Panel
          title="Session credentials"
          subtitle="Local sessions stay open on localhost. Shared sessions must authenticate explicitly."
        >
          <form
            className="space-y-4"
            onSubmit={async (event) => {
              event.preventDefault()
              await onLogin(username, password)
            }}
          >
            <TextInput label="Username" onChange={setUsername} value={username} />
            <TextInput label="Password" onChange={setPassword} value={password} />
            {error ? <p className="text-sm text-danger">{error}</p> : null}
            <Button icon={<LogIn className="h-4 w-4" strokeWidth={1.9} />} type="submit">
              Start session
            </Button>
          </form>
        </Panel>
      </div>
    </div>
  )
}

export function DashboardPage({
  actor,
  bootstrap,
  settings,
  branches,
  history,
  commitDetail,
  loadingCommitDetail,
  selectedCommitId,
  commitSnapshotActivePath,
  commitSnapshotTreeCache,
  commitSnapshotDiff,
  commitSnapshotReadme,
  commitSnapshotPath,
  commitSnapshotTree,
  commitSnapshotBlob,
  treeCache,
  readme,
  codePath,
  codeTree,
  activeBlob,
  pullRequests,
  highlightedPullRequestId,
  selectedBranch,
  selectedPullRequestId,
  loadingMoreCommits,
  onOpenTreeEntry,
  onBrowseTree,
  onLoadTreePath,
  onOpenCommitTreeEntry,
  onBrowseCommitTree,
  onLoadCommitTreePath,
  onSelectCommit,
  onClearSelectedCommit,
  onLoadMoreCommits,
  onCreateBranch,
  onCheckoutBranch,
  onSwitchBranch,
  onUpdateSettings,
  onUpdateBranchRule,
  onDeleteBranchRule,
  onDeleteBranch,
  onCreatePullRequest,
  onUpdatePullRequest,
  onDeletePullRequest,
  onCommentPullRequest,
  onReviewPullRequest,
  onMergePullRequest,
  onViewBranchCode,
  onSelectPullRequest,
  onClearSelectedPullRequest,
}: {
  actor: UiRole
  bootstrap: BootstrapResponse
  settings: SettingsResponse | null
  branches: BranchInfo[]
  history: CommitHistory | null
  commitDetail: CommitDetail | null
  loadingCommitDetail: boolean
  selectedCommitId: string | null
  commitSnapshotActivePath: string | null
  commitSnapshotTreeCache: Record<string, TreeEntry[]>
  commitSnapshotDiff: RefDiffFile | null
  commitSnapshotReadme: BlobContent | null
  commitSnapshotPath: string
  commitSnapshotTree: TreeEntry[]
  commitSnapshotBlob: BlobContent | null
  treeCache: Record<string, TreeEntry[]>
  readme: BlobContent | null
  codePath: string
  codeTree: TreeEntry[]
  activeBlob: BlobContent | null
  pullRequests: PullRequestRecord[]
  highlightedPullRequestId: string | null
  selectedBranch: string | null
  selectedPullRequestId: string | null
  loadingMoreCommits: boolean
  onOpenTreeEntry: (entry: TreeEntry) => Promise<void>
  onBrowseTree: (path: string) => Promise<void>
  onLoadTreePath: (path: string, force?: boolean) => Promise<TreeEntry[]>
  onOpenCommitTreeEntry: (entry: TreeEntry) => Promise<void>
  onBrowseCommitTree: (path: string) => Promise<void>
  onLoadCommitTreePath: (path: string, force?: boolean) => Promise<TreeEntry[]>
  onSelectCommit: (commit: string) => void
  onClearSelectedCommit: () => void
  onLoadMoreCommits: () => Promise<void>
  onCreateBranch: (name: string, startPoint: string, force: boolean) => Promise<void>
  onCheckoutBranch: (name: string) => Promise<void>
  onSwitchBranch: (name: string) => Promise<void>
  onUpdateSettings: (payload: { description?: string; homepage_url?: string }) => Promise<void>
  onUpdateBranchRule: (payload: {
    pattern: string
    require_pull_request: boolean
    required_approvals: number
    dismiss_stale_approvals: boolean
    block_force_push: boolean
    block_delete: boolean
  }) => Promise<void>
  onDeleteBranchRule: (pattern: string) => Promise<void>
  onDeleteBranch: (name: string) => Promise<void>
  onCreatePullRequest: (payload: {
    title: string
    description: string
    source_branch: string
    target_branch: string
  }) => Promise<PullRequestRecord>
  onUpdatePullRequest: (
    id: string,
    payload: { title?: string; description?: string; status?: 'open' | 'closed' },
  ) => Promise<PullRequestRecord>
  onDeletePullRequest: (id: string) => Promise<PullRequestRecord>
  onCommentPullRequest: (
    id: string,
    payload: { display_name: string; body: string },
  ) => Promise<PullRequestRecord>
  onReviewPullRequest: (
    id: string,
    payload: { display_name: string; body: string; state: 'commented' | 'approved' | 'changes_requested' },
  ) => Promise<PullRequestRecord>
  onMergePullRequest: (id: string) => Promise<void>
  onViewBranchCode: (name: string) => void
  onSelectPullRequest: (id: string) => void
  onClearSelectedPullRequest: () => void
}) {
  const [createBranchOpen, setCreateBranchOpen] = useState(false)
  const [createPullRequestOpen, setCreatePullRequestOpen] = useState(false)
  const [selectedTabId, setSelectedTabId] = useState(() => getQueryParam(window.location.search, 'tab') ?? 'code')

  const canEdit = actor === 'owner'
  const latestCommit = history?.commits[0] ?? null
  const codeHeaderAction = (
    <div className="flex flex-wrap items-center justify-end gap-2">
      {selectedBranch ? <Badge tone="accent">Browsing branch: {selectedBranch}</Badge> : null}
      <CloneMenu bootstrap={bootstrap} />
    </div>
  )
  const selectedCommit = useMemo(
    () =>
      history?.commits.find((commit) => commit.id === selectedCommitId) ??
      (commitDetail
        ? {
            id: commitDetail.id,
            summary: commitDetail.summary,
            author: commitDetail.author,
            authored_at: commitDetail.authored_at,
            parents: commitDetail.parents,
            refs: [],
          }
        : null),
    [commitDetail, history, selectedCommitId],
  )

  useEffect(() => {
    window.history.replaceState(
      null,
      '',
      mergeQueryParams(window.location.pathname, window.location.search, { tab: selectedTabId }),
    )
  }, [selectedTabId])

  useEffect(() => {
    const handlePopState = () => {
      setSelectedTabId(getQueryParam(window.location.search, 'tab') ?? 'code')
    }

    window.addEventListener('popstate', handlePopState)
    return () => window.removeEventListener('popstate', handlePopState)
  }, [])

  useEffect(() => {
    if (selectedPullRequestId && selectedTabId !== 'pull-requests') {
      setSelectedTabId('pull-requests')
    }
  }, [selectedPullRequestId, selectedTabId])

  useEffect(() => {
    if (selectedCommitId && selectedTabId !== 'commits') {
      setSelectedTabId('commits')
    }
  }, [selectedCommitId, selectedTabId])

  const tabs = useMemo(
    () => [
      {
        id: 'code',
        icon: shellTabIcons.code,
        label: 'Code',
        count: codeTree.length,
        content: (
          <CodePanel
            activePath={activeBlob?.path ?? null}
            blob={activeBlob}
            currentPath={codePath}
            entries={codeTree}
            headerAction={codeHeaderAction}
            latestCommit={latestCommit}
            rawReference={selectedBranch ?? bootstrap.checked_out_branch}
            onBrowse={onBrowseTree}
            onLoadTreePath={onLoadTreePath}
            onOpen={onOpenTreeEntry}
            readme={readme}
            treeCache={treeCache}
          />
        ),
      },
      {
        id: 'commits',
        icon: shellTabIcons.commits,
        label: 'Commits',
        count: history?.commits.length ?? 0,
        content: (
          <CommitsPanel
            detail={commitDetail}
            history={history}
            loadingDetail={loadingCommitDetail}
            loadingMore={loadingMoreCommits}
            onBack={onClearSelectedCommit}
            onBrowseSnapshot={onBrowseCommitTree}
            onLoadMore={onLoadMoreCommits}
            onLoadSnapshotTreePath={onLoadCommitTreePath}
            onOpenSnapshotEntry={onOpenCommitTreeEntry}
            onSelect={onSelectCommit}
            snapshotActivePath={commitSnapshotActivePath}
            snapshotBlob={commitSnapshotBlob}
            snapshotCommit={selectedCommit}
            snapshotDiff={commitSnapshotDiff}
            snapshotPath={commitSnapshotPath}
            snapshotReadme={commitSnapshotReadme}
            snapshotTree={commitSnapshotTree}
            snapshotTreeCache={commitSnapshotTreeCache}
            selectedCommitId={selectedCommitId}
          />
        ),
      },
      {
        id: 'branches',
        icon: shellTabIcons.branches,
        label: 'Branches',
        count: branches.length,
        content: (
          <BranchesPanel
            branches={branches}
            canEdit={canEdit}
            headerAction={
              canEdit ? (
                <Button
                  icon={<GitBranchPlus className="h-4 w-4" strokeWidth={1.9} />}
                  onClick={() => setCreateBranchOpen(true)}
                >
                  Create branch
                </Button>
              ) : null
            }
            onOpen={(name) => {
              setSelectedTabId('code')
              onViewBranchCode(name)
            }}
            onCheckout={onCheckoutBranch}
            onDelete={onDeleteBranch}
            onSwitch={onSwitchBranch}
          />
        ),
      },
      {
        id: 'pull-requests',
        icon: shellTabIcons['pull-requests'],
        label: 'Pull requests',
        count: pullRequests.length,
        content: (
          <PullRequestsPanel
            actor={actor}
            canCreate={canEdit}
            canManage={canEdit}
            highlightedPullRequestId={highlightedPullRequestId}
            onBack={onClearSelectedPullRequest}
            onComment={onCommentPullRequest}
            onCreate={() => setCreatePullRequestOpen(true)}
            onDelete={onDeletePullRequest}
            onMerge={onMergePullRequest}
            onReview={onReviewPullRequest}
            onSelect={onSelectPullRequest}
            onUpdate={onUpdatePullRequest}
            pullRequests={pullRequests}
            selectedPullRequestId={selectedPullRequestId}
          />
        ),
      },
      {
        id: 'settings',
        icon: shellTabIcons.settings,
        label: 'Settings',
        content: (
          <SettingsPanel
            bootstrap={bootstrap}
            branches={branches}
            canEdit={canEdit}
            onDeleteBranchRule={onDeleteBranchRule}
            onSwitchBranch={onSwitchBranch}
            onUpdateBranchRule={onUpdateBranchRule}
            onUpdateSettings={onUpdateSettings}
            settings={settings}
          />
        ),
      },
    ],
    [
      activeBlob,
      actor,
      branches,
      bootstrap,
      canEdit,
      commitSnapshotActivePath,
      commitSnapshotBlob,
      commitSnapshotDiff,
      commitSnapshotPath,
      commitSnapshotReadme,
      commitSnapshotTree,
      commitSnapshotTreeCache,
      codePath,
      codeTree,
      commitDetail,
      highlightedPullRequestId,
      history,
      loadingCommitDetail,
      loadingMoreCommits,
      latestCommit,
      codeHeaderAction,
      onBrowseCommitTree,
      onCheckoutBranch,
      onBrowseTree,
      onCommentPullRequest,
      onClearSelectedCommit,
      onClearSelectedPullRequest,
      onDeleteBranch,
      onDeleteBranchRule,
      onDeletePullRequest,
      onLoadCommitTreePath,
      onLoadTreePath,
      onLoadMoreCommits,
      onMergePullRequest,
      onOpenCommitTreeEntry,
      onSelectPullRequest,
      onReviewPullRequest,
      onOpenTreeEntry,
      onSelectCommit,
      onSwitchBranch,
      onUpdateBranchRule,
      onUpdateSettings,
      onUpdatePullRequest,
      onViewBranchCode,
      pullRequests,
      readme,
      selectedBranch,
      selectedCommit,
      selectedPullRequestId,
      selectedCommitId,
      settings,
      treeCache,
    ],
  )

  return (
    <>
      <AppShell
        actor={actor}
        branchCount={branches.length}
        checkedOutBranch={bootstrap.checked_out_branch}
        exportedBranch={bootstrap.exported_branch}
        pullRequestCount={pullRequests.length}
        repoDescription={bootstrap.description}
        repoHomepageUrl={bootstrap.homepage_url}
        repoName={bootstrap.repo_name}
        selectedTabId={selectedTabId}
        onSelectTab={setSelectedTabId}
        tabs={tabs}
      />

      <CreateBranchModal
        bootstrap={bootstrap}
        onClose={() => setCreateBranchOpen(false)}
        onCreateBranch={onCreateBranch}
        open={createBranchOpen}
      />

      <CreatePullRequestModal
        bootstrap={bootstrap}
        branches={branches}
        onClose={() => setCreatePullRequestOpen(false)}
        onCreatePullRequest={onCreatePullRequest}
        open={createPullRequestOpen}
      />
    </>
  )
}
