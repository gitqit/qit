import { useState } from 'react'
import { KeyRound, UserCircle2 } from 'lucide-react'
import { Button, EmptyState, Panel } from '../../atoms/Controls'
import { FormActions, KeyValueRow, TextInput } from '../../molecules/Fields'
import type { BootstrapResponse, IssuedPat, SettingsResponse } from '../../../lib/types'

export function UserSettingsPanel({
  bootstrap,
  onCreatePat,
  onRevokePat,
  settings,
}: {
  bootstrap: BootstrapResponse
  onCreatePat: (label: string) => Promise<IssuedPat>
  onRevokePat: (id: string) => Promise<void>
  settings: SettingsResponse | null
}) {
  const [patLabel, setPatLabel] = useState('')
  const [saving, setSaving] = useState(false)
  const [issuedPat, setIssuedPat] = useState<IssuedPat | null>(null)
  const [error, setError] = useState<string | null>(null)
  const currentUser = settings?.current_user ?? bootstrap.principal

  if (!settings) {
    return (
      <Panel subtitle="Qit could not load user settings yet." title="User settings">
        <EmptyState title="Settings unavailable" message="Refresh the session and try again once the account metadata is loaded." />
      </Panel>
    )
  }

  return (
    <div className="grid gap-6 lg:grid-cols-[220px_minmax(0,1fr)]">
      <Panel subtitle="Personal settings for the current session." title="User settings">
        <nav className="space-y-1">
          <button
            className="flex w-full items-center justify-between rounded-token border border-accent/35 bg-accent/10 px-3 py-2.5 text-left text-sm font-medium text-fg"
            type="button"
          >
            <span>Personal access tokens</span>
          </button>
        </nav>
      </Panel>

      <div className="space-y-6">
        <Panel
          subtitle={currentUser ? 'Account-scoped settings for your signed-in identity.' : 'Local operator sessions do not have a repo user profile.'}
          title="Profile"
        >
          <div className="rounded-token border border-border bg-panel-subtle px-4 py-2">
            <KeyValueRow
              icon={<UserCircle2 className="h-4 w-4" strokeWidth={1.85} />}
              label="Session"
              value={currentUser ? currentUser.email : bootstrap.operator_override ? 'Local operator' : 'Shared session'}
            />
            <KeyValueRow
              icon={<UserCircle2 className="h-4 w-4" strokeWidth={1.85} />}
              label="Username"
              value={currentUser?.username ?? 'Not available'}
            />
            <KeyValueRow
              icon={<UserCircle2 className="h-4 w-4" strokeWidth={1.85} />}
              label="Role"
              value={currentUser?.role ?? (bootstrap.operator_override ? 'operator' : 'session')}
            />
          </div>
        </Panel>

        <Panel
          subtitle={
            currentUser
              ? 'Create personal access tokens for Git without reusing your password.'
              : 'Personal access tokens are only available for signed-in repo accounts.'
          }
          title="Personal access tokens"
        >
          {currentUser ? (
            <div className="space-y-4">
              <TextInput
                description="Give the token a memorable label for this device or workflow."
                label="New PAT label"
                onChange={setPatLabel}
                value={patLabel}
              />
              <FormActions hint="PAT secrets are shown only once after creation.">
                <Button
                  disabled={saving || !patLabel.trim()}
                  icon={<KeyRound className="h-4 w-4" strokeWidth={1.9} />}
                  onClick={async () => {
                    setSaving(true)
                    setError(null)
                    try {
                      const issued = await onCreatePat(patLabel)
                      setIssuedPat(issued)
                      setPatLabel('')
                    } catch (saveError) {
                      setError(saveError instanceof Error ? saveError.message : 'Unable to create the PAT.')
                    } finally {
                      setSaving(false)
                    }
                  }}
                >
                  {saving ? 'Creating…' : 'Create PAT'}
                </Button>
              </FormActions>
              {settings.personal_access_tokens.length ? (
                <div className="space-y-2">
                  {settings.personal_access_tokens.map((token) => (
                    <div className="flex flex-wrap items-center justify-between gap-3 rounded-token border border-border/80 bg-panel-subtle px-4 py-3" key={token.id}>
                      <div>
                        <p className="text-sm font-medium text-fg">{token.label}</p>
                        <p className="text-xs text-fg-subtle">{token.id.slice(0, 8)}</p>
                      </div>
                      <Button
                        disabled={saving}
                        onClick={async () => {
                          setSaving(true)
                          setError(null)
                          try {
                            await onRevokePat(token.id)
                          } catch (saveError) {
                            setError(saveError instanceof Error ? saveError.message : 'Unable to revoke the PAT.')
                          } finally {
                            setSaving(false)
                          }
                        }}
                        tone="danger"
                      >
                        Revoke
                      </Button>
                    </div>
                  ))}
                </div>
              ) : (
                <EmptyState
                  title="No personal access tokens"
                  message="Create a PAT when you want Git access without using your password directly."
                />
              )}
              {issuedPat ? (
                <div className="rounded-token border border-success/30 bg-success/10 px-4 py-3 text-sm text-fg">
                  <p className="font-medium">New PAT created for {issuedPat.label}</p>
                  <p className="mt-1 break-all font-mono text-xs text-fg-muted">{issuedPat.secret}</p>
                </div>
              ) : null}
              {error ? (
                <p className="rounded-token border border-danger/40 bg-danger/10 px-3 py-2 text-sm text-danger">{error}</p>
              ) : null}
            </div>
          ) : (
            <EmptyState
              title="No repo account"
              message="Sign in with a repo account to manage your personal access tokens."
            />
          )}
        </Panel>
      </div>
    </div>
  )
}
