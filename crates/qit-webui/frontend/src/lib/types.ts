export type UiRole = 'owner' | 'user'
export type AuthMode = 'shared_session' | 'request_based'
export type AuthMethod = 'request_access' | 'setup_token' | 'basic_auth'
export type RepoUserRole = 'owner' | 'user'
export type RepoUserStatus = 'pending_request' | 'approved_pending_setup' | 'active' | 'revoked'
export type AccessRequestStatus = 'pending' | 'approved' | 'rejected' | 'revoked'

export type PullRequestStatus = 'open' | 'merged' | 'closed'
export type IssueStatus = 'open' | 'closed'
export type IssueLinkRelation = 'related' | 'closing'
export type IssueLinkSource =
  | 'manual'
  | 'issue_description'
  | 'issue_comment'
  | 'pull_request_description'
  | 'pull_request_comment'
  | 'pull_request_review'

export type PullRequestReviewState = 'commented' | 'approved' | 'changes_requested'

export type IssueReactionContent =
  | 'thumbs_up'
  | 'thumbs_down'
  | 'laugh'
  | 'hooray'
  | 'confused'
  | 'heart'
  | 'rocket'
  | 'eyes'

export type PullRequestActivityKind =
  | 'opened'
  | 'commented'
  | 'reviewed'
  | 'edited'
  | 'closed'
  | 'reopened'
  | 'merged'

export type IssueTimelineEventKind =
  | 'opened'
  | 'commented'
  | 'edited'
  | 'closed'
  | 'reopened'
  | 'labels_changed'
  | 'assignees_changed'
  | 'milestone_changed'
  | 'pull_request_linked'
  | 'pull_request_unlinked'
  | 'reaction_toggled'

export type IssueTimelineTarget = 'issue' | 'comment'

export type TreeEntryKind = 'tree' | 'blob'

export interface BootstrapResponse {
  actor: UiRole | null
  principal: AuthenticatedPrincipal | null
  repo_name: string
  worktree: string
  exported_branch: string
  checked_out_branch: string
  description: string
  homepage_url: string
  auth_mode: AuthMode
  auth_methods: AuthMethod[]
  operator_override: boolean
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
  auth_mode: AuthMode
  auth_methods: AuthMethod[]
  local_only_owner_mode: boolean
  shared_remote_identity: boolean
  current_user: AuthenticatedPrincipal | null
  users: RepoUserView[]
  access_requests: AccessRequestView[]
  personal_access_tokens: PatRecordView[]
  repository: RepositorySettings
}

export interface AuthenticatedPrincipal {
  user_id: string
  name: string
  email: string
  username: string
  role: RepoUserRole
}

export interface RepoUserView {
  id: string
  name: string
  email: string
  username: string | null
  role: RepoUserRole
  status: RepoUserStatus
  created_at_ms: number
  approved_at_ms: number | null
  activated_at_ms: number | null
  revoked_at_ms: number | null
}

export interface AccessRequestView {
  id: string
  name: string
  email: string
  status: AccessRequestStatus
  created_at_ms: number
  reviewed_at_ms: number | null
}

export interface SubmittedAccessRequest {
  request: AccessRequestView
  secret: string
}

export interface AccessRequestProgress {
  id: string
  status: AccessRequestStatus
}

export interface PatRecordView {
  id: string
  label: string
  created_at_ms: number
  revoked_at_ms: number | null
}

export interface IssuedOnboarding {
  user_id: string
  email: string
  /** Present for manual setup codes only; omitted after access-request approval. */
  secret?: string | null
  expires_at_ms: number
}

export interface IssuedPat {
  id: string
  label: string
  secret: string
  created_at_ms: number
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

export interface IssueActor {
  role: UiRole
  display_name: string
  user_id: string | null
  username: string | null
}

export interface IssueReactionSummary {
  content: IssueReactionContent
  count: number
  reacted: boolean
}

export interface IssueReactionRecord {
  id: string
  content: IssueReactionContent
  actor: IssueActor
  created_at_ms: number
}

export interface IssueComment {
  id: string
  actor: IssueActor
  body: string
  created_at_ms: number
  updated_at_ms: number
  reactions: IssueReactionRecord[]
}

export interface IssueTimelineEvent {
  id: string
  kind: IssueTimelineEventKind
  actor: IssueActor
  body: string | null
  title: string | null
  description: string | null
  labels: string[]
  assignee_user_ids: string[]
  milestone_id: string | null
  pull_request_id: string | null
  reaction: IssueReactionContent | null
  target: IssueTimelineTarget | null
  target_id: string | null
  created_at_ms: number
}

export interface IssueLinkedPullRequest {
  pull_request_id: string
  relation: IssueLinkRelation
  source: IssueLinkSource
  linked_at_ms: number
}

export interface IssueLabel {
  id: string
  name: string
  color: string
  description: string
  created_at_ms: number
  updated_at_ms: number
}

export interface IssueMilestone {
  id: string
  title: string
  description: string
  created_at_ms: number
  updated_at_ms: number
}

export interface IssueRecord {
  id: string
  number: number
  title: string
  description: string
  status: IssueStatus
  author: IssueActor
  created_at_ms: number
  updated_at_ms: number
  closed_at_ms: number | null
  label_ids: string[]
  assignee_user_ids: string[]
  milestone_id: string | null
  linked_pull_requests: IssueLinkedPullRequest[]
  reactions: IssueReactionRecord[]
  comments: IssueComment[]
  timeline: IssueTimelineEvent[]
}

export interface IssueLinkedPullRequestView {
  relation: IssueLinkRelation
  source: IssueLinkSource
  pull_request: PullRequestRecord
}

export interface PullRequestLinkedIssueView {
  relation: IssueLinkRelation
  source: IssueLinkSource
  issue: IssueRecord
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
  linked_issues: PullRequestLinkedIssueView[]
  comments: PullRequestComment[]
  reviews: PullRequestReview[]
  review_summary: PullRequestReviewSummary
  activity: PullRequestActivity[]
}

export interface IssueAssigneeView {
  id: string
  name: string
  username: string
  role: RepoUserRole
}

export interface IssueMetadataResponse {
  labels: IssueLabel[]
  milestones: IssueMilestone[]
  assignees: IssueAssigneeView[]
}

export interface IssueCommentResponse {
  comment: IssueComment
  reaction_summary: IssueReactionSummary[]
}

export interface IssueDetailResponse {
  issue: IssueRecord
  comments: IssueCommentResponse[]
  timeline: IssueTimelineEvent[]
  linked_pull_requests: IssueLinkedPullRequestView[]
  reaction_summary: IssueReactionSummary[]
  metadata: IssueMetadataResponse
}

export interface IssuesResponse {
  issues: IssueRecord[]
}
