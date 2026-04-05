import { EmptyState, Panel } from '../../atoms/Controls'
import { KeyValueRow } from '../../molecules/Fields'
import type { SettingsResponse } from '../../../lib/types'

export function SettingsPanel({
  settings,
}: {
  settings: SettingsResponse | null
}) {
  return (
    <Panel
      subtitle="These values describe how this session is being served, so collaborators understand the trust model without reading internal docs."
      title="Access policy"
    >
      {!settings ? (
        <EmptyState title="Settings unavailable" message="Qit could not load the access policy for this session yet." />
      ) : (
        <div className="space-y-4">
          <div className="rounded-token border border-border bg-panel-subtle px-4 py-2">
            <KeyValueRow label="Access mode" value={settings.local_only_owner_mode ? 'Local-only owner access' : 'Credentialed web session'} />
            <KeyValueRow label="Remote identity" value="Shared session account" />
          </div>
          <p className="text-sm leading-6 text-fg-muted">
            Local-only sessions stay frictionless on localhost. Exposed sessions require the temporary credentials printed when the server starts.
          </p>
        </div>
      )}
    </Panel>
  )
}
