export type UiRole = 'owner' | 'user'

export type PullRequestStatus = 'open' | 'merged' | 'closed'

export type PullRequestReviewState = 'commented' | 'approved' | 'changes_requested'

export type PullRequestActivityKind =
  | 'opened'
  | 'commented'
  | 'reviewed'
  | 'edited'
  | 'closed'
  | 'reopened'
  | 'merged'

export type TreeEntryKind = 'tree' | 'blob'

export interface BootstrapResponse {
  actor: UiRole | null
  repo_name: string
  worktree: string
  exported_branch: string
  checked_out_branch: string
  description: string
  homepage_url: string
  local_only_owner_mode: boolean
  shared_remote_identity: boolean
  git_credentials_visible: boolean
  git_username: string | null
  git_password: string | null
  public_repo_url: string | null
}

export interface BranchRule {
  pattern: string
  require_pull_request: boolean
  required_approvals: number
  dismiss_stale_approvals: boolean
  block_force_push: boolean
  block_delete: boolean
}

export interface RepositorySettings {
  description: string
  homepage_url: string
  branch_rules: BranchRule[]
}

export interface SettingsResponse {
  local_only_owner_mode: boolean
  shared_remote_identity: boolean
  repository: RepositorySettings
}

export interface BranchInfo {
  name: string
  is_current: boolean
  is_served: boolean
  commit: string
  summary: string
}

export interface BranchesResponse {
  branches: BranchInfo[]
}

export interface CommitSummary {
  id: string
  summary: string
  author: string
  authored_at: number
}

export type CommitRefKind = 'branch'

export interface CommitRefDecoration {
  name: string
  kind: CommitRefKind
  is_current: boolean
  is_served: boolean
}

export interface CommitHistoryNode {
  id: string
  summary: string
  author: string
  authored_at: number
  parents: string[]
  refs: CommitRefDecoration[]
}

export interface CommitHistory {
  reference: string
  offset: number
  limit: number
  has_more: boolean
  commits: CommitHistoryNode[]
}

export interface CommitsResponse {
  history: CommitHistory
}

export interface CommitFileChange {
  path: string
  status: string
  additions: number
  deletions: number
}

export interface CommitDetail {
  id: string
  summary: string
  message: string
  author: string
  authored_at: number
  parents: string[]
  changes: CommitFileChange[]
}

export interface TreeEntry {
  name: string
  path: string
  oid: string
  kind: TreeEntryKind
  size: number | null
}

export interface TreeResponse {
  entries: TreeEntry[]
}

export interface BlobContent {
  path: string
  text: string | null
  is_binary: boolean
  size: number
}

export interface BlobResponse {
  blob: BlobContent
}

export interface PullRequestRecord {
  id: string
  title: string
  description: string
  source_branch: string
  target_branch: string
  source_commit: string | null
  target_commit: string | null
  status: PullRequestStatus
  author_role: UiRole
  created_at_ms: number
  updated_at_ms: number
  merged_commit: string | null
  activities: PullRequestActivity[]
}

export interface PullRequestActivity {
  id: string
  kind: PullRequestActivityKind
  actor_role: UiRole
  display_name: string | null
  body: string | null
  review_state: PullRequestReviewState | null
  title: string | null
  description: string | null
  created_at_ms: number
}

export interface PullRequestComment {
  id: string
  actor_role: UiRole
  display_name: string
  body: string
  created_at_ms: number
}

export interface PullRequestReview {
  id: string
  actor_role: UiRole
  display_name: string
  body: string
  state: PullRequestReviewState
  created_at_ms: number
}

export interface PullRequestReviewSummaryEntry {
  actor_role: UiRole
  display_name: string
  state: PullRequestReviewState
  reviewed_at_ms: number
}

export interface PullRequestReviewSummary {
  approvals: number
  changes_requested: number
  comments: number
  latest_reviews: PullRequestReviewSummaryEntry[]
}

export interface PullRequestsResponse {
  pull_requests: PullRequestRecord[]
}

export interface RefComparison {
  base_ref: string
  head_ref: string
  merge_base: string | null
  ahead_by: number
  behind_by: number
  commits: CommitSummary[]
}

export interface CompareResponse {
  comparison: RefComparison
}

export interface RefDiffFile {
  path: string
  previous_path: string | null
  status: string
  additions: number
  deletions: number
  original: BlobContent | null
  modified: BlobContent | null
}

export interface PullRequestDetailResponse {
  pull_request: PullRequestRecord
  comparison: RefComparison | null
  diffs: RefDiffFile[] | null
  comments: PullRequestComment[]
  reviews: PullRequestReview[]
  review_summary: PullRequestReviewSummary
  activity: PullRequestActivity[]
}
