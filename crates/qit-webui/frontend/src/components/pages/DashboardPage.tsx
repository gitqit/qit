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
  LogOut,
  Settings,
  ShieldCheck,
  UserPlus,
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
import { UserSettingsPanel } from '../organisms/panels/UserSettingsPanel'
import { AppShell, type HeaderAlert } from '../templates/AppShell'
import { shellTabIcons } from '../templates/shellTabIcons'
import { api } from '../../lib/api'
import { getQueryParam, mergeQueryParams } from '../../lib/queryState'
import type {
  AuthMethod,
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
  IssuedOnboarding,
  IssuedPat,
} from '../../lib/types'

function copyToClipboard(value: string) {
  return navigator.clipboard.writeText(value)
}

type RequestAuthView = 'signin' | 'request' | 'setup'

function hasAuthMethod(methods: AuthMethod[], method: AuthMethod) {
  return methods.includes(method)
}

function CloneMenu({ bootstrap }: { bootstrap: BootstrapResponse }) {
  const [copiedKey, setCopiedKey] = useState<string | null>(null)
  const [passwordVisible, setPasswordVisible] = useState(false)
  const hasVisibleCredentials = bootstrap.git_credentials_visible && !!bootstrap.git_username && !!bootstrap.git_password
  const supportsBasicAuth = hasAuthMethod(bootstrap.auth_methods, 'basic_auth')
  const supportsRepoAccounts = hasAuthMethod(bootstrap.auth_methods, 'setup_token')

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
              {supportsBasicAuth && supportsRepoAccounts
                ? 'Use either the shared session credentials from the operator or your own repo username and password. PATs are available from your account menu.'
                : supportsBasicAuth
                  ? 'Git credentials stay with the owner session. Ask the operator for the startup username and password if you need clone or push access.'
                  : 'Use your repo username and password, or a PAT you create from your account menu.'}
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

function SessionMenu({
  canLogout,
  onOpenUserSettings,
  onLogout,
  triggerLabel,
}: {
  canLogout: boolean
  onOpenUserSettings: () => void
  onLogout: () => Promise<void>
  triggerLabel: string
}) {
  return (
    <Menu as="div" className="relative">
      <MenuButton className="inline-flex items-center gap-2 rounded-token px-2.5 py-2 text-sm font-medium text-fg-muted outline-none transition hover:bg-panel-subtle hover:text-fg focus-visible:ring-2 focus-visible:ring-accent/30 focus-visible:ring-offset-2 focus-visible:ring-offset-canvas data-active:bg-panel-subtle data-active:text-fg">
        <span className="max-w-[16rem] truncate">{triggerLabel}</span>
        <ChevronDown className="h-4 w-4" strokeWidth={1.9} />
      </MenuButton>
      <MenuItems
        anchor="bottom end"
        className="z-30 mt-2 w-64 rounded-token border border-border bg-panel p-2 shadow-panel outline-none"
      >
        <div className="border-b border-border/80 px-3 py-2">
          <p className="truncate text-sm font-semibold text-fg">{triggerLabel}</p>
        </div>
        <div className="p-2">
          <MenuItem>
            <button
              className="flex w-full items-center gap-2 rounded-token px-3 py-2 text-left text-sm font-medium text-fg-muted transition hover:bg-panel-subtle hover:text-fg"
              onClick={onOpenUserSettings}
              type="button"
            >
              <Settings className="h-4 w-4" strokeWidth={1.9} />
              <span>User settings</span>
            </button>
          </MenuItem>
          {canLogout ? (
            <MenuItem>
              <button
                className="mt-1 flex w-full items-center gap-2 rounded-token px-3 py-2 text-left text-sm font-medium text-fg-muted transition hover:bg-panel-subtle hover:text-fg"
                onClick={async () => {
                  await onLogout()
                }}
                type="button"
              >
                <LogOut className="h-4 w-4" strokeWidth={1.9} />
                <span>Log out</span>
              </button>
            </MenuItem>
          ) : null}
        </div>
      </MenuItems>
    </Menu>
  )
}

export function LoginPage({
  bootstrap,
  error,
  requestMessage,
  setupMessage,
  onLogin,
  onRequestAccess,
  onCompleteOnboarding,
}: {
  bootstrap: BootstrapResponse
  error: string | null
  requestMessage: string | null
  setupMessage: string | null
  onLogin: (username: string, password: string) => Promise<void>
  onRequestAccess: (name: string, email: string) => Promise<void>
  onCompleteOnboarding: (token: string, username: string, password: string) => Promise<void>
}) {
  const [username, setUsername] = useState('')
  const [password, setPassword] = useState('')
  const [requestName, setRequestName] = useState('')
  const [requestEmail, setRequestEmail] = useState('')
  const [setupToken, setSetupToken] = useState('')
  const [setupUsername, setSetupUsername] = useState('')
  const [setupPassword, setSetupPassword] = useState('')
  const supportsRequestAccess = hasAuthMethod(bootstrap.auth_methods, 'request_access')
  const supportsSetupToken = hasAuthMethod(bootstrap.auth_methods, 'setup_token')
  const supportsBasicAuth = hasAuthMethod(bootstrap.auth_methods, 'basic_auth')
  const tabs: Array<{ id: RequestAuthView; label: string }> = [
    { id: 'signin', label: 'Sign in' },
    ...(supportsRequestAccess ? [{ id: 'request' as const, label: 'Request access' }] : []),
    ...(supportsSetupToken ? [{ id: 'setup' as const, label: 'Use setup code' }] : []),
  ]
  const [activeView, setActiveView] = useState<RequestAuthView>('signin')

  useEffect(() => {
    if (requestMessage && supportsRequestAccess) {
      setActiveView('request')
      return
    }

    if (setupMessage && supportsSetupToken) {
      setActiveView('setup')
    }
  }, [requestMessage, setupMessage, supportsRequestAccess, supportsSetupToken])

  const flowMeta: Record<
    RequestAuthView,
    {
      title: string
      subtitle: string
      ctaHint: string
    }
  > = {
    signin: {
      title: 'Sign in',
      subtitle: supportsBasicAuth
        ? supportsSetupToken
          ? 'Use either the shared session credentials from the operator or your own repo account if it is already active.'
          : 'Use the shared session credentials printed when the server started.'
        : 'Use your repo username and password if your account is already active.',
      ctaHint: supportsRequestAccess
        ? 'New here? Request access first. If an owner already sent you a setup code, switch tabs and redeem it there.'
        : supportsSetupToken
          ? 'Already have a setup code from an owner? Redeem it once, then sign in normally after that.'
          : 'Sign in with the credentials available for this repository.',
    },
    request: {
      title: 'Request access',
      subtitle: 'Tell the owner who you are so they can approve a per-user account.',
      ctaHint: 'This tab stays here while approval status updates. If an owner already gave you a setup code, switch tabs and redeem it there.',
    },
    setup: {
      title: 'Use setup code',
      subtitle: 'Redeem a one-time setup code from an owner to create your repo username and password.',
      ctaHint: 'Once setup is complete, Qit will sign you in automatically.',
    },
  }

  const currentFlow = flowMeta[activeView]

  return (
    <div className="flex min-h-screen items-center justify-center bg-canvas px-6 py-10">
      <div className="w-full max-w-2xl space-y-6">
        <div className="flex justify-center">
          <BrandLogo className="h-24 sm:h-28" />
        </div>
        <SectionHeader
          eyebrow="Qit"
          title="Access this repository"
          detail={
            supportsRequestAccess
              ? 'Use your repo account if you already have one. New collaborators can request approval here and wait for an owner to share a one-time setup code.'
              : supportsSetupToken
                ? 'Use your repo account if it is already active, or redeem a one-time setup code from an owner.'
                : 'Use the shared session credentials printed when the server started.'
          }
        />
        <Panel title={currentFlow.title} subtitle={currentFlow.subtitle}>
          <div className="space-y-5">
            {tabs.length > 1 ? (
              <div className="inline-flex w-full flex-wrap gap-2 rounded-lg border border-border/80 bg-canvas-raised/70 p-1.5">
                {tabs.map((tab) => (
                  <Button
                    aria-pressed={activeView === tab.id}
                    className="flex-1 justify-center"
                    key={tab.id}
                    onClick={() => setActiveView(tab.id)}
                    tone={activeView === tab.id ? 'primary' : 'muted'}
                    type="button"
                  >
                    {tab.label}
                  </Button>
                ))}
              </div>
            ) : null}

            {error ? (
              <p className="rounded-token border border-danger/40 bg-danger/10 px-3.5 py-3 text-sm text-danger">
                {error}
              </p>
            ) : null}

            {activeView === 'signin' ? (
              <form
                className="space-y-4"
                onSubmit={async (event) => {
                  event.preventDefault()
                  await onLogin(username, password)
                }}
              >
                <TextInput
                  autoComplete="username"
                  autoFocus
                  label="Username"
                  onChange={setUsername}
                  placeholder={supportsBasicAuth ? 'shared session or repo username' : 'your repo username'}
                  required
                  value={username}
                />
                <TextInput
                  autoComplete="current-password"
                  label="Password"
                  onChange={setPassword}
                  placeholder={supportsBasicAuth ? 'shared password or account password' : 'your account password'}
                  required
                  type="password"
                  value={password}
                />
                <div className="flex flex-col gap-3 border-t border-border/80 pt-5 sm:flex-row sm:items-center sm:justify-between">
                  <p className="text-xs leading-5 text-fg-subtle">{currentFlow.ctaHint}</p>
                  <Button icon={<LogIn className="h-4 w-4" strokeWidth={1.9} />} type="submit">
                    Start session
                  </Button>
                </div>
              </form>
            ) : null}

            {activeView === 'request' && supportsRequestAccess ? (
              <form
                className="space-y-4"
                onSubmit={async (event) => {
                  event.preventDefault()
                  await onRequestAccess(requestName, requestEmail)
                }}
              >
                <TextInput
                  autoComplete="name"
                  autoFocus
                  label="Name"
                  onChange={setRequestName}
                  placeholder="Your full name"
                  required
                  value={requestName}
                />
                <TextInput
                  autoComplete="email"
                  label="Email"
                  onChange={setRequestEmail}
                  placeholder="you@example.com"
                  required
                  type="email"
                  value={requestEmail}
                />
                {requestMessage ? (
                  <p className="rounded-token border border-success/40 bg-success/10 px-3.5 py-3 text-sm text-success">
                    {requestMessage}
                  </p>
                ) : null}
                <div className="flex flex-col gap-3 border-t border-border/80 pt-5 sm:flex-row sm:items-center sm:justify-between">
                  <p className="text-xs leading-5 text-fg-subtle">{currentFlow.ctaHint}</p>
                  <Button icon={<UserPlus className="h-4 w-4" strokeWidth={1.9} />} type="submit">
                    Send request
                  </Button>
                </div>
              </form>
            ) : null}

            {activeView === 'setup' && supportsSetupToken ? (
              <form
                className="space-y-4"
                onSubmit={async (event) => {
                  event.preventDefault()
                  await onCompleteOnboarding(setupToken, setupUsername, setupPassword)
                }}
              >
                <TextInput
                  autoComplete="off"
                  autoFocus
                  label="Setup code"
                  onChange={setSetupToken}
                  placeholder="qit_setup..."
                  required
                  value={setupToken}
                />
                <TextInput
                  autoComplete="username"
                  label="Username"
                  onChange={setSetupUsername}
                  placeholder="Choose a repo username"
                  required
                  value={setupUsername}
                />
                <TextInput
                  autoComplete="new-password"
                  label="Password"
                  onChange={setSetupPassword}
                  placeholder="Create a password"
                  required
                  type="password"
                  value={setupPassword}
                />
                {setupMessage ? (
                  <p className="rounded-token border border-success/40 bg-success/10 px-3.5 py-3 text-sm text-success">
                    {setupMessage}
                  </p>
                ) : null}
                <div className="flex flex-col gap-3 border-t border-border/80 pt-5 sm:flex-row sm:items-center sm:justify-between">
                  <p className="text-xs leading-5 text-fg-subtle">{currentFlow.ctaHint}</p>
                  <Button icon={<ShieldCheck className="h-4 w-4" strokeWidth={1.9} />} type="submit">
                    Finish setup
                  </Button>
                </div>
              </form>
            ) : null}
          </div>
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
  onUpdateAuthMethods,
  onApproveAccessRequest,
  onIssueSetupToken,
  onRejectAccessRequest,
  onPromoteUser,
  onDemoteUser,
  onRevokeUser,
  onResetUserSetup,
  onCreatePat,
  onRevokePat,
  onLogout,
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
  onUpdateAuthMethods: (methods: AuthMethod[]) => Promise<void>
  onApproveAccessRequest: (id: string) => Promise<IssuedOnboarding>
  onIssueSetupToken: (name: string, email: string) => Promise<IssuedOnboarding>
  onRejectAccessRequest: (id: string) => Promise<void>
  onPromoteUser: (id: string) => Promise<void>
  onDemoteUser: (id: string) => Promise<void>
  onRevokeUser: (id: string) => Promise<void>
  onResetUserSetup: (id: string) => Promise<IssuedOnboarding>
  onCreatePat: (label: string) => Promise<IssuedPat>
  onRevokePat: (id: string) => Promise<void>
  onLogout: () => Promise<void>
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
  const supportsRequestAccess = bootstrap.auth_methods.includes('request_access')
  const pendingAccessRequests = settings?.access_requests ?? []
  const canEdit = actor === 'owner'
  const alerts = useMemo<HeaderAlert[]>(
    () =>
      canEdit && supportsRequestAccess
        ? pendingAccessRequests.map((request) => ({
            id: request.id,
            title: request.name,
            detail: request.email,
            primaryAction: {
              label: 'Approve',
              onSelect: async () => {
                await onApproveAccessRequest(request.id)
              },
            },
            secondaryAction: {
              label: 'Reject',
              onSelect: async () => {
                await onRejectAccessRequest(request.id)
              },
              tone: 'danger',
            },
          }))
        : [],
    [canEdit, onApproveAccessRequest, onRejectAccessRequest, pendingAccessRequests, supportsRequestAccess],
  )
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
            onApproveAccessRequest={onApproveAccessRequest}
            onIssueSetupToken={onIssueSetupToken}
            onDemoteUser={onDemoteUser}
            onDeleteBranchRule={onDeleteBranchRule}
            onPromoteUser={onPromoteUser}
            onRejectAccessRequest={onRejectAccessRequest}
            onResetUserSetup={onResetUserSetup}
            onRevokeUser={onRevokeUser}
            onUpdateAuthMethods={onUpdateAuthMethods}
            onSwitchBranch={onSwitchBranch}
            onUpdateBranchRule={onUpdateBranchRule}
            onUpdateSettings={onUpdateSettings}
            settings={settings}
          />
        ),
      },
      {
        id: 'user-settings',
        hidden: true,
        icon: shellTabIcons.settings,
        label: 'User settings',
        content: (
          <UserSettingsPanel
            bootstrap={bootstrap}
            onCreatePat={onCreatePat}
            onRevokePat={onRevokePat}
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
      onCreatePat,
      onIssueSetupToken,
      onSelectCommit,
      onSwitchBranch,
      onUpdateBranchRule,
      onUpdateSettings,
      onUpdatePullRequest,
      onViewBranchCode,
      onRevokePat,
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
        alerts={canEdit ? alerts : undefined}
        sessionControl={
          <SessionMenu
            canLogout={!bootstrap.operator_override}
            onOpenUserSettings={() => setSelectedTabId('user-settings')}
            onLogout={onLogout}
            triggerLabel={bootstrap.principal?.email ?? (bootstrap.operator_override ? 'Local operator' : 'Shared session')}
          />
        }
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
