import { useState } from 'react'
import { GitBranchPlus, GitPullRequestCreateArrow } from 'lucide-react'
import { Badge, Button, EmptyState, Panel } from '../atoms/Controls'
import { SectionHeader, TextArea, TextInput } from '../molecules/Fields'

export function PreviewPage() {
  const [title, setTitle] = useState('Refine Qit shell and states')
  const [description, setDescription] = useState('Tighten hierarchy, remove internal copy, and improve branch and pull-request review flows.')

  return (
    <div className="min-h-screen bg-canvas text-fg">
      <div className="mx-auto max-w-6xl px-5 py-8 sm:px-6">
        <SectionHeader
          eyebrow="Qit UI Preview"
          title="Component review"
          detail="Use this preview surface to review tokens and primitives in isolation while refining the product UI."
        />

        <div className="mt-8 grid gap-6 lg:grid-cols-2">
          <Panel
            title="Buttons and badges"
            subtitle="Primary actions should be clear. Status should be calm. Nothing here should feel semantically confused."
          >
            <div className="space-y-5">
              <div className="flex flex-wrap gap-3">
                <Button icon={<GitBranchPlus className="h-4 w-4" strokeWidth={1.9} />}>Create branch</Button>
                <Button tone="muted">Secondary action</Button>
                <Button tone="danger">Delete branch</Button>
              </div>
              <div className="flex flex-wrap gap-2">
                <Badge tone="accent">Checked out</Badge>
                <Badge tone="success">Served</Badge>
                <Badge tone="muted">Draft</Badge>
                <Badge tone="danger">Blocked</Badge>
              </div>
            </div>
          </Panel>

          <Panel
            title="Form controls"
            subtitle="Inputs should read as one family and preserve a clear label, comfortable spacing, and strong focus state."
            action={<Button icon={<GitPullRequestCreateArrow className="h-4 w-4" strokeWidth={1.9} />}>Primary CTA</Button>}
          >
            <div className="space-y-4">
              <TextInput
                description="This mirrors the tighter headline pattern used in the pull-request form."
                label="Title"
                onChange={setTitle}
                value={title}
              />
              <TextArea
                description="Use this area to review text spacing, hint styling, and input rhythm."
                label="Description"
                onChange={setDescription}
                rows={5}
                value={description}
              />
            </div>
          </Panel>
        </div>

        <div className="mt-6">
          <Panel
            title="Empty states"
            subtitle="Empty states should preserve layout and explain the next move without sounding like an implementation error."
          >
            <EmptyState title="No pull requests yet" message="Open a branch comparison when you are ready to package a change for review." />
          </Panel>
        </div>
      </div>
    </div>
  )
}
