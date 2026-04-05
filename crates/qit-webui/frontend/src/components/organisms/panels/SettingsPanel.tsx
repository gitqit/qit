import { useEffect, useMemo, useState } from 'react'
import {
  ExternalLink,
  GitBranch,
  Globe,
  Shield,
  ShieldAlert,
  Sparkles,
  Trash2,
} from 'lucide-react'
import { Badge, Button, EmptyState, Panel } from '../../atoms/Controls'
import { FormActions, KeyValueRow, TextArea, TextInput } from '../../molecules/Fields'
import type { BootstrapResponse, BranchInfo, SettingsResponse } from '../../../lib/types'

type SectionId = 'general' | 'branches' | 'access'

function classNames(...values: Array<string | false | null | undefined>) {
  return values.filter(Boolean).join(' ')
}

function sectionCopy(section: SectionId) {
  switch (section) {
    case 'general':
      return {
        title: 'General',
        detail: 'Describe this repository the same way GitHub does: concise context, a useful homepage, and the right default branch.',
      }
    case 'branches':
      return {
        title: 'Branch rules',
        detail: 'Create simple protection rules for important branches before the deeper settings surface grows.',
      }
    case 'access':
      return {
        title: 'Access',
        detail: 'Keep the session trust model visible so collaborators understand how this repo is being shared.',
      }
    default:
      return {
        title: 'Settings',
        detail: '',
      }
  }
}

function RuleToggle({
  checked,
  description,
  label,
  onChange,
}: {
  checked: boolean
  description: string
  label: string
  onChange: (value: boolean) => void
}) {
  return (
    <label className="flex items-start gap-3 rounded-token border border-border/80 bg-panel-subtle px-4 py-3">
      <input
        checked={checked}
        className="mt-1 h-4 w-4 rounded border-border bg-panel-subtle text-accent focus:ring-accent/30"
        onChange={(event) => onChange(event.target.checked)}
        type="checkbox"
      />
      <span className="space-y-1">
        <span className="block text-sm font-medium text-fg">{label}</span>
        <span className="block text-xs leading-5 text-fg-subtle">{description}</span>
      </span>
    </label>
  )
}

export function SettingsPanel({
  bootstrap,
  branches,
  canEdit,
  settings,
  onDeleteBranchRule,
  onSwitchBranch,
  onUpdateBranchRule,
  onUpdateSettings,
}: {
  bootstrap: BootstrapResponse
  branches: BranchInfo[]
  canEdit: boolean
  settings: SettingsResponse | null
  onDeleteBranchRule: (pattern: string) => Promise<void>
  onSwitchBranch: (name: string) => Promise<void>
  onUpdateBranchRule: (payload: {
    pattern: string
    require_pull_request: boolean
    required_approvals: number
    dismiss_stale_approvals: boolean
    block_force_push: boolean
    block_delete: boolean
  }) => Promise<void>
  onUpdateSettings: (payload: { description?: string; homepage_url?: string }) => Promise<void>
}) {
  const [selectedSection, setSelectedSection] = useState<SectionId>('general')
  const [description, setDescription] = useState(settings?.repository.description ?? '')
  const [homepageUrl, setHomepageUrl] = useState(settings?.repository.homepage_url ?? '')
  const [defaultBranch, setDefaultBranch] = useState(bootstrap.exported_branch)
  const [pattern, setPattern] = useState('')
  const [requiredApprovals, setRequiredApprovals] = useState('0')
  const [requirePullRequest, setRequirePullRequest] = useState(false)
  const [dismissStaleApprovals, setDismissStaleApprovals] = useState(false)
  const [blockForcePush, setBlockForcePush] = useState(false)
  const [blockDelete, setBlockDelete] = useState(false)
  const [editingPattern, setEditingPattern] = useState<string | null>(null)
  const [savingGeneral, setSavingGeneral] = useState(false)
  const [savingBranch, setSavingBranch] = useState(false)
  const [savingRule, setSavingRule] = useState(false)
  const [error, setError] = useState<string | null>(null)

  useEffect(() => {
    setDescription(settings?.repository.description ?? '')
    setHomepageUrl(settings?.repository.homepage_url ?? '')
  }, [settings])

  useEffect(() => {
    setDefaultBranch(bootstrap.exported_branch)
  }, [bootstrap.exported_branch])

  const sections = useMemo(
    () => [
      { id: 'general' as const, label: 'General', count: undefined },
      { id: 'branches' as const, label: 'Branches', count: settings?.repository.branch_rules.length ?? 0 },
      { id: 'access' as const, label: 'Access', count: undefined },
    ],
    [settings?.repository.branch_rules.length],
  )

  const copy = sectionCopy(selectedSection)
  const rules = settings?.repository.branch_rules ?? []
  const selectClassName =
    'w-full rounded-token border border-border bg-panel-subtle px-3.5 py-2.5 text-sm text-fg outline-none transition-colors focus:border-accent focus:ring-2 focus:ring-accent/20'

  const resetRuleForm = () => {
    setPattern('')
    setRequiredApprovals('0')
    setRequirePullRequest(false)
    setDismissStaleApprovals(false)
    setBlockForcePush(false)
    setBlockDelete(false)
    setEditingPattern(null)
  }

  const startEditingRule = (rule: NonNullable<typeof settings>['repository']['branch_rules'][number]) => {
    setSelectedSection('branches')
    setPattern(rule.pattern)
    setRequiredApprovals(String(rule.required_approvals))
    setRequirePullRequest(rule.require_pull_request)
    setDismissStaleApprovals(rule.dismiss_stale_approvals)
    setBlockForcePush(rule.block_force_push)
    setBlockDelete(rule.block_delete)
    setEditingPattern(rule.pattern)
  }

  if (!settings) {
    return (
      <Panel subtitle="Qit could not load repository settings yet." title="Repository settings">
        <EmptyState title="Settings unavailable" message="Refresh the session and try again once the repository metadata is loaded." />
      </Panel>
    )
  }

  return (
    <div className="grid gap-6 lg:grid-cols-[220px_minmax(0,1fr)]">
      <Panel subtitle="GitHub-style settings navigation inside this repo session." title="Settings">
        <nav className="space-y-1">
          {sections.map((section) => {
            const active = section.id === selectedSection
            return (
              <button
                className={classNames(
                  'flex w-full items-center justify-between rounded-token border px-3 py-2.5 text-left text-sm font-medium transition',
                  active
                    ? 'border-accent/35 bg-accent/10 text-fg'
                    : 'border-transparent text-fg-muted hover:border-border hover:bg-panel-subtle hover:text-fg',
                )}
                key={section.id}
                onClick={() => setSelectedSection(section.id)}
                type="button"
              >
                <span>{section.label}</span>
                {typeof section.count === 'number' ? (
                  <span className="rounded-full border border-border bg-canvas px-2 py-0.5 text-[11px] text-fg-subtle">
                    {section.count}
                  </span>
                ) : null}
              </button>
            )
          })}
        </nav>
      </Panel>

      <div className="space-y-6">
        <Panel
          subtitle={copy.detail}
          title={copy.title}
        >
          <div className="flex flex-wrap items-center gap-2">
            <Badge icon={<Sparkles className="h-3.5 w-3.5" strokeWidth={1.85} />} tone="accent">
              {bootstrap.repo_name}
            </Badge>
            {settings.repository.homepage_url ? (
              <a
                className="inline-flex items-center gap-1 text-sm text-accent hover:text-accent-strong"
                href={settings.repository.homepage_url}
                rel="noreferrer"
                target="_blank"
              >
                <ExternalLink className="h-3.5 w-3.5" strokeWidth={1.85} />
                <span>{settings.repository.homepage_url}</span>
              </a>
            ) : null}
          </div>
        </Panel>

        {selectedSection === 'general' ? (
          <Panel subtitle="These fields become the lightweight identity of the repository across the app." title="Repository profile">
            <div className="space-y-4">
              <TextArea
                description="Keep it short, descriptive, and useful in lists."
                label="Description"
                onChange={setDescription}
                rows={3}
                value={description}
              />
              <TextInput
                description="Optional canonical docs, project page, or deployment URL."
                label="Homepage URL"
                onChange={setHomepageUrl}
                placeholder="https://example.com"
                value={homepageUrl}
              />
              <FormActions hint="This mirrors the high-signal metadata GitHub shows at the top of repository settings.">
                <Button
                  disabled={!canEdit || savingGeneral}
                  onClick={async () => {
                    setSavingGeneral(true)
                    setError(null)
                    try {
                      await onUpdateSettings({
                        description,
                        homepage_url: homepageUrl,
                      })
                    } catch (saveError) {
                      setError(saveError instanceof Error ? saveError.message : 'Unable to save repository settings.')
                    } finally {
                      setSavingGeneral(false)
                    }
                  }}
                >
                  {savingGeneral ? 'Saving…' : 'Save profile'}
                </Button>
              </FormActions>
            </div>
          </Panel>
        ) : null}

        {selectedSection === 'branches' ? (
          <>
            <Panel subtitle="Change which branch the session advertises and treats as the repository default." title="Default branch">
              <div className="space-y-4">
                <label className="block space-y-2">
                  <span className="text-sm font-medium text-fg">Served branch</span>
                  <select
                    className={selectClassName}
                    disabled={!canEdit || savingBranch}
                    onChange={(event) => setDefaultBranch(event.target.value)}
                    value={defaultBranch}
                  >
                    {branches.map((branch) => (
                      <option key={branch.name} value={branch.name}>
                        {branch.name}
                      </option>
                    ))}
                  </select>
                </label>
                <FormActions hint="Qit uses the served branch as the repository default branch for collaborators.">
                  <Button
                    disabled={!canEdit || savingBranch || defaultBranch === bootstrap.exported_branch}
                    icon={<GitBranch className="h-4 w-4" strokeWidth={1.9} />}
                    onClick={async () => {
                      setSavingBranch(true)
                      setError(null)
                      try {
                        await onSwitchBranch(defaultBranch)
                      } catch (saveError) {
                        setError(saveError instanceof Error ? saveError.message : 'Unable to switch the served branch.')
                      } finally {
                        setSavingBranch(false)
                      }
                    }}
                    tone="muted"
                  >
                    {savingBranch ? 'Updating…' : 'Update default branch'}
                  </Button>
                </FormActions>
              </div>
            </Panel>

            <Panel subtitle="Start with simple GitHub-style protections for important branches." title="Branch rules">
              <div className="space-y-6">
                {rules.length ? (
                  <div className="space-y-3">
                    {rules.map((rule) => (
                      <div
                        className="rounded-token border border-border/80 bg-panel-subtle px-4 py-4"
                        key={rule.pattern}
                      >
                        <div className="flex flex-col gap-3 sm:flex-row sm:items-start sm:justify-between">
                          <div className="space-y-2">
                            <div className="flex flex-wrap items-center gap-2">
                              <Badge tone="muted">{rule.pattern}</Badge>
                              {rule.require_pull_request ? <Badge tone="accent">Pull request required</Badge> : null}
                              {rule.required_approvals > 0 ? (
                                <Badge tone="success">{rule.required_approvals} approval(s)</Badge>
                              ) : null}
                              {rule.dismiss_stale_approvals ? <Badge tone="danger">Dismiss stale approvals</Badge> : null}
                              {rule.block_force_push ? <Badge tone="danger">Block force-push</Badge> : null}
                              {rule.block_delete ? <Badge tone="danger">Block delete</Badge> : null}
                            </div>
                            <p className="text-sm text-fg-muted">
                              Applies to branches matching <code>{rule.pattern}</code>.
                            </p>
                          </div>
                          <div className="flex items-center gap-2">
                            <Button onClick={() => startEditingRule(rule)} tone="muted">
                              Edit
                            </Button>
                            <Button
                              icon={<Trash2 className="h-4 w-4" strokeWidth={1.9} />}
                              onClick={async () => {
                                if (!window.confirm(`Delete the branch rule for "${rule.pattern}"?`)) {
                                  return
                                }
                                setSavingRule(true)
                                setError(null)
                                try {
                                  await onDeleteBranchRule(rule.pattern)
                                  if (editingPattern === rule.pattern) {
                                    resetRuleForm()
                                  }
                                } catch (saveError) {
                                  setError(saveError instanceof Error ? saveError.message : 'Unable to delete the branch rule.')
                                } finally {
                                  setSavingRule(false)
                                }
                              }}
                              tone="danger"
                            >
                              Delete
                            </Button>
                          </div>
                        </div>
                      </div>
                    ))}
                  </div>
                ) : (
                  <EmptyState
                    message="No branch rules yet. Add a simple protection for `main`, `release/*`, or another important branch pattern."
                    title="No rules configured"
                  />
                )}

                <div className="space-y-4 rounded-lg border border-border/80 bg-canvas-raised/60 p-5">
                  <div className="space-y-1">
                    <h3 className="text-base font-semibold text-fg">
                      {editingPattern ? `Edit rule for ${editingPattern}` : 'Add branch rule'}
                    </h3>
                    <p className="text-sm leading-6 text-fg-muted">
                      Pattern matching uses the same simple glob style already used elsewhere in Qit.
                    </p>
                  </div>
                  <TextInput
                    description="Examples: `main`, `release/*`, `hotfix/*`."
                    label="Branch pattern"
                    onChange={setPattern}
                    value={pattern}
                  />
                  <TextInput
                    description="Set to `0` to allow merges without approvals."
                    label="Required approvals"
                    onChange={setRequiredApprovals}
                    type="number"
                    value={requiredApprovals}
                  />
                  <div className="grid gap-3 md:grid-cols-2">
                    <RuleToggle
                      checked={requirePullRequest}
                      description="Only merges through pull requests should land on matching branches."
                      label="Require pull request"
                      onChange={setRequirePullRequest}
                    />
                    <RuleToggle
                      checked={dismissStaleApprovals}
                      description="Ignore prior approvals when the source branch moves after review."
                      label="Dismiss stale approvals"
                      onChange={setDismissStaleApprovals}
                    />
                    <RuleToggle
                      checked={blockForcePush}
                      description="Reject non-fast-forward pushes on matching branches."
                      label="Block force-push"
                      onChange={setBlockForcePush}
                    />
                    <RuleToggle
                      checked={blockDelete}
                      description="Reject branch deletions over Git transport and in the UI."
                      label="Block delete"
                      onChange={setBlockDelete}
                    />
                  </div>
                  <FormActions hint="Approvals automatically imply pull-request-only merges.">
                    {editingPattern ? (
                      <Button disabled={savingRule} onClick={resetRuleForm} tone="muted">
                        Cancel
                      </Button>
                    ) : null}
                    <Button
                      disabled={!canEdit || savingRule}
                      onClick={async () => {
                        setSavingRule(true)
                        setError(null)
                        try {
                          await onUpdateBranchRule({
                            pattern,
                            require_pull_request: requirePullRequest,
                            required_approvals: Number(requiredApprovals || '0'),
                            dismiss_stale_approvals: dismissStaleApprovals,
                            block_force_push: blockForcePush,
                            block_delete: blockDelete,
                          })
                          resetRuleForm()
                        } catch (saveError) {
                          setError(saveError instanceof Error ? saveError.message : 'Unable to save the branch rule.')
                        } finally {
                          setSavingRule(false)
                        }
                      }}
                    >
                      {savingRule ? 'Saving…' : editingPattern ? 'Save rule' : 'Add rule'}
                    </Button>
                  </FormActions>
                </div>
              </div>
            </Panel>
          </>
        ) : null}

        {selectedSection === 'access' ? (
          <Panel subtitle="Keep the trust model visible without forcing people to read startup logs." title="Access policy">
            <div className="space-y-4">
              <div className="rounded-token border border-border bg-panel-subtle px-4 py-2">
                <KeyValueRow
                  icon={<Shield className="h-4 w-4" strokeWidth={1.85} />}
                  label="Access mode"
                  value={settings.local_only_owner_mode ? 'Local-only owner access' : 'Credentialed web session'}
                />
                <KeyValueRow
                  icon={<ShieldAlert className="h-4 w-4" strokeWidth={1.85} />}
                  label="Remote identity"
                  value="Shared session account"
                />
                <KeyValueRow
                  icon={<Globe className="h-4 w-4" strokeWidth={1.85} />}
                  label="Homepage"
                  value={settings.repository.homepage_url || 'Not set'}
                />
              </div>
              <p className="text-sm leading-6 text-fg-muted">
                Local-only sessions stay frictionless on localhost. Exposed sessions require the temporary credentials printed when the server starts.
              </p>
            </div>
          </Panel>
        ) : null}

        {error ? <p className="text-sm text-danger">{error}</p> : null}
      </div>
    </div>
  )
}
