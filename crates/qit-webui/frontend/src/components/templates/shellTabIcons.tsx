import type { ReactNode } from 'react'
import { CircleDot, Code2, GitBranch, GitCommitHorizontal, GitPullRequest, Settings } from 'lucide-react'

export const shellTabIcons = {
  code: <Code2 className="h-4 w-4" strokeWidth={1.85} />,
  commits: <GitCommitHorizontal className="h-4 w-4" strokeWidth={1.85} />,
  branches: <GitBranch className="h-4 w-4" strokeWidth={1.85} />,
  issues: <CircleDot className="h-4 w-4" strokeWidth={1.85} />,
  'pull-requests': <GitPullRequest className="h-4 w-4" strokeWidth={1.85} />,
  settings: <Settings className="h-4 w-4" strokeWidth={1.85} />,
} satisfies Record<string, ReactNode>
