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
  Shield,
} from 'lucide-react'
import { BrandLogo } from '../atoms/BrandLogo'
import { Button, Panel } from '../atoms/Controls'
import { KeyValueRow, SectionHeader, TextInput } from '../molecules/Fields'
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
import type {
  BlobContent,
  BootstrapResponse,
  BranchInfo,
  CommitDetail,
  CommitHistory,
  PullRequestRecord,
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

  const cloneUrl = bootstrap.public_repo_url ?? `${window.location.origin}${api.baseUrl}`
  const authCloneUrl =
    bootstrap.git_username && bootstrap.git_password
      ? (() => {
          const url = new URL(cloneUrl)
          url.username = bootstrap.git_username
          url.password = bootstrap.git_password
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
        className="z-30 mt-2 flex w-[min(28rem,calc(100vw-1.5rem))] max-w-[calc(100vw-1.5rem)] flex-col gap-3 rounded-[var(--radius-lg)] border border-border bg-panel p-3 shadow-[var(--shadow-raised)] outline-none"
      >
        <div className="border-b border-border/80 px-1 pb-3">
          <div className="flex items-center gap-2 text-sm font-semibold text-fg">
            <GitBranch className="h-4 w-4 text-success" strokeWidth={1.85} />
            <span>Clone this repository</span>
          </div>
          <p className="mt-1 text-sm text-fg-muted">
            Copy the served URL, session credentials, or a ready-to-run clone command.
          </p>
          <p className="mt-2 text-xs text-fg-subtle">Served branch: {bootstrap.exported_branch}</p>
        </div>

        <MenuItem>
          <div>
            <CopyAction label="HTTPS URL" value={cloneUrl} valueKey="url" />
          </div>
        </MenuItem>

        {bootstrap.git_username ? (
          <MenuItem>
            <div>
              <CopyAction label="Username" value={bootstrap.git_username} valueKey="username" />
            </div>
          </MenuItem>
        ) : null}

        {bootstrap.git_password ? (
          <MenuItem>
            <div>
              <CopyAction label="Password" secret value={bootstrap.git_password} valueKey="password" />
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
  selectedCommitId,
  treeCache,
  readme,
  codePath,
  codeTree,
  activeBlob,
  pullRequests,
  highlightedPullRequestId,
  selectedPullRequestId,
  loadingMoreCommits,
  onOpenTreeEntry,
  onBrowseTree,
  onLoadTreePath,
  onSelectCommit,
  onLoadMoreCommits,
  onCreateBranch,
  onCheckoutBranch,
  onSwitchBranch,
  onDeleteBranch,
  onCreatePullRequest,
  onUpdatePullRequest,
  onDeletePullRequest,
  onCommentPullRequest,
  onReviewPullRequest,
  onMergePullRequest,
  onSelectPullRequest,
  onClearSelectedPullRequest,
}: {
  actor: UiRole
  bootstrap: BootstrapResponse
  settings: SettingsResponse | null
  branches: BranchInfo[]
  history: CommitHistory | null
  commitDetail: CommitDetail | null
  selectedCommitId: string | null
  treeCache: Record<string, TreeEntry[]>
  readme: BlobContent | null
  codePath: string
  codeTree: TreeEntry[]
  activeBlob: BlobContent | null
  pullRequests: PullRequestRecord[]
  highlightedPullRequestId: string | null
  selectedPullRequestId: string | null
  loadingMoreCommits: boolean
  onOpenTreeEntry: (entry: TreeEntry) => Promise<void>
  onBrowseTree: (path: string) => Promise<void>
  onLoadTreePath: (path: string, force?: boolean) => Promise<TreeEntry[]>
  onSelectCommit: (commit: string) => void
  onLoadMoreCommits: () => Promise<void>
  onCreateBranch: (name: string, startPoint: string, force: boolean) => Promise<void>
  onCheckoutBranch: (name: string) => Promise<void>
  onSwitchBranch: (name: string) => Promise<void>
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
  onSelectPullRequest: (id: string) => void
  onClearSelectedPullRequest: () => void
}) {
  const [createBranchOpen, setCreateBranchOpen] = useState(false)
  const [createPullRequestOpen, setCreatePullRequestOpen] = useState(false)
  const [selectedTabId, setSelectedTabId] = useState(() => new URLSearchParams(window.location.search).get('tab') ?? 'code')

  const canEdit = actor === 'owner'
  const latestCommit = history?.commits[0] ?? null

  useEffect(() => {
    const params = new URLSearchParams(window.location.search)
    params.set('tab', selectedTabId)
    const query = params.toString()
    window.history.replaceState(null, '', query ? `${window.location.pathname}?${query}` : window.location.pathname)
  }, [selectedTabId])

  useEffect(() => {
    if (selectedPullRequestId && selectedTabId !== 'pull-requests') {
      setSelectedTabId('pull-requests')
    }
  }, [selectedPullRequestId, selectedTabId])

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
            headerAction={<CloneMenu bootstrap={bootstrap} />}
            latestCommit={latestCommit}
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
            loadingMore={loadingMoreCommits}
            onLoadMore={onLoadMoreCommits}
            onSelect={onSelectCommit}
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
          <div className="grid gap-6 xl:grid-cols-[minmax(0,1fr)_360px]">
            <SettingsPanel settings={settings} />
            <Panel title="Session role" subtitle="Only the owner can change branch state in this session.">
              <div className="space-y-4">
                <KeyValueRow
                  icon={<Shield className="h-4 w-4" strokeWidth={1.85} />}
                  label="Current role"
                  value={actor === 'owner' ? 'Owner' : 'Viewer'}
                />
                <KeyValueRow
                  icon={<Shield className="h-4 w-4" strokeWidth={1.85} />}
                  label="Access mode"
                  value={settings?.local_only_owner_mode ? 'Local-only owner access' : 'Credentialed web session'}
                />
              </div>
            </Panel>
          </div>
        ),
      },
    ],
    [
      activeBlob,
      actor,
      branches,
      bootstrap,
      canEdit,
      codePath,
      codeTree,
      commitDetail,
      highlightedPullRequestId,
      history,
      loadingMoreCommits,
      latestCommit,
      onCheckoutBranch,
      onBrowseTree,
      onCommentPullRequest,
      onClearSelectedPullRequest,
      onDeleteBranch,
      onDeletePullRequest,
      onLoadTreePath,
      onLoadMoreCommits,
      onMergePullRequest,
      onSelectPullRequest,
      onReviewPullRequest,
      onOpenTreeEntry,
      onSelectCommit,
      onSwitchBranch,
      onUpdatePullRequest,
      pullRequests,
      readme,
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
