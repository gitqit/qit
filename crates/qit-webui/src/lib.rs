mod auth;

use axum::extract::{Json, Path as AxumPath, Query, State};
use axum::http::header::{CONTENT_TYPE, SET_COOKIE};
use axum::http::{HeaderMap, HeaderValue, Response, StatusCode};
use axum::response::{Html, IntoResponse};
use axum::routing::{delete, get, post, put};
use axum::{body::Body, Router};
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use base64::Engine as _;
use qit_domain::{
    AccessRequestView, AuthActor, AuthMethod, AuthMode, AuthenticatedPrincipal, BlobContent,
    BranchInfo, CommitDetail, CommitHistory, CommitRefDecoration, CommitRefKind, CreateIssue,
    CreateIssueComment, CreatePullRequest, CreatePullRequestComment, CreatePullRequestReview,
    DomainError, IssueActor, IssueActorInput, IssueCommentRecord, IssueLabel, IssueLinkRelation,
    IssueLinkSource, IssueMilestone, IssueReactionContent, IssueReactionSummary, IssueRecord,
    IssueStatus, IssueTimelineEvent, PatRecordView, PullRequestActivityRecord, PullRequestRecord,
    PullRequestReviewRecord, PullRequestReviewState, PullRequestReviewSummary, PullRequestStatus,
    RefComparison, RefDiffFile, RepoReadStore, RepoUserRole, RepoUserStatus, RepoUserView,
    RepositorySettings, SessionCredentials, UiRole, UpdateIssue, UpdatePullRequest,
    UpdateRepositorySettings, UpsertBranchRule, UpsertIssueLabel, UpsertIssueMilestone,
    WorkspaceService, WorkspaceSpec, WorkspaceWebUiState,
};
use rand::distributions::{Alphanumeric, DistString};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::RwLock;

#[cfg(test)]
use axum::http::header::HOST;

const APP_JS: &[u8] = include_bytes!("../frontend/dist/assets/app.js");
const APP_CSS: &[u8] = include_bytes!("../frontend/dist/assets/index.css");
const CHUNK_ROLLDOWN_RUNTIME_JS: &[u8] =
    include_bytes!("../frontend/dist/assets/chunk-rolldown-runtime.js");
const CHUNK_VENDOR_JS: &[u8] = include_bytes!("../frontend/dist/assets/chunk-vendor.js");
const CHUNK_VENDOR_UI_JS: &[u8] = include_bytes!("../frontend/dist/assets/chunk-vendor-ui.js");
const CHUNK_VENDOR_MARKDOWN_JS: &[u8] =
    include_bytes!("../frontend/dist/assets/chunk-vendor-markdown.js");
const CHUNK_VENDOR_TREE_JS: &[u8] = include_bytes!("../frontend/dist/assets/chunk-vendor-tree.js");
const CHUNK_VENDOR_MONACO_JS: &[u8] =
    include_bytes!("../frontend/dist/assets/chunk-vendor-monaco.js");
const CHUNK_MONACO_CODE_SURFACE_JS: &[u8] =
    include_bytes!("../frontend/dist/assets/chunk-MonacoCodeSurface.js");
const QIT_LOGO_ON_DARK: &[u8] = include_bytes!("../frontend/dist/assets/qit-logo-on-dark.png");
const QIT_LOGO_ON_LIGHT: &[u8] = include_bytes!("../frontend/dist/assets/qit-logo-on-light.png");
const SESSION_TTL_MS: u64 = 12 * 60 * 60 * 1000;
const LOGIN_WINDOW_MS: u64 = 60 * 1000;
const LOGIN_LOCKOUT_MS: u64 = 60 * 1000;
const LOGIN_FAILURE_LIMIT: u8 = 5;

pub struct WebUiConfig {
    pub workspace: WorkspaceSpec,
    pub repo_mount_path: String,
    pub credentials: SessionCredentials,
    pub implicit_owner_mode: bool,
    pub secure_cookies: bool,
    pub public_repo_url: Option<String>,
}

#[derive(Clone)]
struct SessionRecord {
    user_id: Option<String>,
    role: UiRole,
    expires_at_ms: u64,
}

#[derive(Clone)]
struct ResolvedSession {
    actor: UiRole,
    principal: Option<AuthenticatedPrincipal>,
    operator_override: bool,
}

#[derive(Clone, Default)]
struct LoginAttemptRecord {
    failures: u8,
    window_started_at_ms: u64,
    locked_until_ms: u64,
}

#[derive(Clone)]
pub struct WebUiServer {
    repo_read_store: Arc<dyn RepoReadStore>,
    workspace_service: Arc<WorkspaceService>,
    workspace: WorkspaceSpec,
    repo_mount_path: String,
    credentials: SessionCredentials,
    implicit_owner_mode: bool,
    secure_cookies: bool,
    public_repo_url: Option<String>,
    sessions: Arc<RwLock<HashMap<String, SessionRecord>>>,
    login_attempts: Arc<RwLock<HashMap<String, LoginAttemptRecord>>>,
}

#[derive(Serialize, Deserialize)]
struct BootstrapResponse {
    actor: Option<UiRole>,
    principal: Option<AuthenticatedPrincipal>,
    repo_name: String,
    worktree: String,
    exported_branch: String,
    checked_out_branch: String,
    description: String,
    homepage_url: String,
    auth_mode: AuthMode,
    auth_methods: Vec<AuthMethod>,
    operator_override: bool,
    local_only_owner_mode: bool,
    shared_remote_identity: bool,
    git_credentials_visible: bool,
    git_username: Option<String>,
    git_password: Option<String>,
    public_repo_url: Option<String>,
}

#[derive(Serialize, Deserialize)]
struct SettingsResponse {
    auth_mode: AuthMode,
    auth_methods: Vec<AuthMethod>,
    local_only_owner_mode: bool,
    shared_remote_identity: bool,
    current_user: Option<AuthenticatedPrincipal>,
    users: Vec<RepoUserView>,
    access_requests: Vec<AccessRequestView>,
    personal_access_tokens: Vec<PatRecordView>,
    repository: RepositorySettings,
}

#[derive(Serialize)]
struct BranchesResponse {
    branches: Vec<BranchInfo>,
}

#[derive(Serialize)]
struct CommitsResponse {
    history: CommitHistory,
}

#[derive(Serialize)]
struct PullRequestsResponse {
    pull_requests: Vec<PullRequestRecord>,
}

#[derive(Serialize)]
struct IssuesResponse {
    issues: Vec<IssueRecord>,
}

#[derive(Serialize)]
struct IssueAssigneeView {
    id: String,
    name: String,
    username: String,
    role: RepoUserRole,
}

#[derive(Serialize)]
struct IssueMetadataResponse {
    labels: Vec<IssueLabel>,
    milestones: Vec<IssueMilestone>,
    assignees: Vec<IssueAssigneeView>,
}

#[derive(Serialize)]
struct IssueCommentResponse {
    comment: IssueCommentRecord,
    reaction_summary: Vec<IssueReactionSummary>,
}

#[derive(Serialize)]
struct IssueDetailResponse {
    issue: IssueRecord,
    comments: Vec<IssueCommentResponse>,
    timeline: Vec<IssueTimelineEvent>,
    linked_pull_requests: Vec<IssueLinkedPullRequestView>,
    reaction_summary: Vec<IssueReactionSummary>,
    metadata: IssueMetadataResponse,
}

#[derive(Serialize)]
struct IssueLinkedPullRequestView {
    relation: IssueLinkRelation,
    source: IssueLinkSource,
    pull_request: PullRequestRecord,
}

#[derive(Serialize)]
struct TreeResponse {
    entries: Vec<qit_domain::TreeEntry>,
}

#[derive(Serialize)]
struct CompareResponse {
    comparison: qit_domain::RefComparison,
}

#[derive(Serialize)]
struct PullRequestDetailResponse {
    pull_request: PullRequestRecord,
    comparison: Option<RefComparison>,
    diffs: Option<Vec<RefDiffFile>>,
    linked_issues: Vec<PullRequestLinkedIssueView>,
    comments: Vec<qit_domain::PullRequestCommentRecord>,
    reviews: Vec<PullRequestReviewRecord>,
    review_summary: PullRequestReviewSummary,
    activity: Vec<PullRequestActivityRecord>,
}

#[derive(Serialize)]
struct PullRequestLinkedIssueView {
    relation: IssueLinkRelation,
    source: IssueLinkSource,
    issue: IssueRecord,
}

#[derive(Serialize)]
struct BlobResponse {
    blob: BlobContent,
}

#[derive(Serialize)]
struct BranchMutationResponse {
    exported_branch: String,
    checked_out_branch: String,
}

#[derive(Deserialize)]
struct LoginRequest {
    username: String,
    password: String,
}

#[derive(Deserialize)]
struct AccessRequestCreateRequest {
    name: String,
    email: String,
}

#[derive(Deserialize)]
struct AccessRequestStatusRequest {
    token: String,
}

#[derive(Deserialize)]
struct ManualSetupTokenRequest {
    name: String,
    email: String,
}

#[derive(Deserialize)]
struct OnboardingCompleteRequest {
    token: String,
    username: String,
    password: String,
}

#[derive(Deserialize)]
struct PatCreateRequest {
    label: String,
}

#[derive(Deserialize)]
struct AuthModeUpdateRequest {
    #[serde(default)]
    mode: Option<AuthMode>,
    #[serde(default)]
    methods: Option<Vec<AuthMethod>>,
}

#[derive(Deserialize)]
struct BranchCreateRequest {
    name: String,
    start_point: Option<String>,
    #[serde(default)]
    force: bool,
}

#[derive(Deserialize)]
struct BranchSelectionRequest {
    name: String,
    #[serde(default)]
    force: bool,
}

#[derive(Deserialize)]
struct PullRequestCreateRequest {
    title: String,
    description: String,
    source_branch: String,
    target_branch: String,
}

#[derive(Deserialize)]
struct PullRequestUpdateRequest {
    title: Option<String>,
    description: Option<String>,
    status: Option<PullRequestStatusRequest>,
}

#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
enum PullRequestStatusRequest {
    Open,
    Closed,
}

#[derive(Deserialize)]
struct PullRequestCommentRequest {
    display_name: String,
    body: String,
}

#[derive(Deserialize)]
struct PullRequestReviewRequest {
    display_name: String,
    body: String,
    state: PullRequestReviewState,
}

#[derive(Deserialize)]
struct IssueCreateRequest {
    title: String,
    #[serde(default)]
    description: String,
    #[serde(default)]
    display_name: Option<String>,
    #[serde(default)]
    label_ids: Vec<String>,
    #[serde(default)]
    assignee_user_ids: Vec<String>,
    #[serde(default)]
    milestone_id: Option<String>,
    #[serde(default)]
    linked_pull_request_ids: Vec<String>,
}

#[derive(Deserialize)]
struct IssueUpdateRequest {
    title: Option<String>,
    description: Option<String>,
    status: Option<IssueStatusRequest>,
}

#[derive(Deserialize)]
#[serde(rename_all = "snake_case")]
enum IssueStatusRequest {
    Open,
    Closed,
}

#[derive(Deserialize)]
struct IssueCommentRequest {
    #[serde(default)]
    display_name: Option<String>,
    body: String,
}

#[derive(Deserialize)]
struct IssueReactionRequest {
    content: IssueReactionContent,
    #[serde(default)]
    display_name: Option<String>,
}

#[derive(Deserialize)]
struct IssueLabelsRequest {
    label_ids: Vec<String>,
}

#[derive(Deserialize)]
struct IssueAssigneesRequest {
    assignee_user_ids: Vec<String>,
}

#[derive(Deserialize)]
struct IssueMilestoneRequest {
    #[serde(default)]
    milestone_id: Option<String>,
}

#[derive(Deserialize)]
struct IssueLinkPullRequestRequest {
    pull_request_id: String,
}

#[derive(Deserialize)]
struct IssueLabelUpsertRequest {
    id: Option<String>,
    name: String,
    #[serde(default)]
    color: String,
    #[serde(default)]
    description: String,
}

#[derive(Deserialize)]
struct IssueMilestoneUpsertRequest {
    id: Option<String>,
    title: String,
    #[serde(default)]
    description: String,
}

#[derive(Deserialize)]
struct SettingsUpdateRequest {
    description: Option<String>,
    homepage_url: Option<String>,
}

#[derive(Deserialize)]
struct BranchRuleRequest {
    pattern: String,
    #[serde(default)]
    require_pull_request: bool,
    #[serde(default)]
    required_approvals: u8,
    #[serde(default)]
    dismiss_stale_approvals: bool,
    #[serde(default)]
    block_force_push: bool,
    #[serde(default)]
    block_delete: bool,
}

#[derive(Deserialize)]
struct TreeQuery {
    reference: Option<String>,
    path: Option<String>,
}

#[derive(Deserialize)]
struct BlobQuery {
    reference: Option<String>,
    path: String,
}

#[derive(Deserialize)]
struct CompareQuery {
    base: String,
    head: String,
}

#[derive(Deserialize)]
struct CommitListQuery {
    reference: Option<String>,
    offset: Option<usize>,
    limit: Option<usize>,
}

const DEFAULT_HISTORY_LIMIT: usize = 40;
const MAX_HISTORY_LIMIT: usize = 120;

fn clamp_history_limit(limit: Option<usize>) -> usize {
    limit
        .unwrap_or(DEFAULT_HISTORY_LIMIT)
        .clamp(1, MAX_HISTORY_LIMIT)
}

fn decorate_history(history: &mut CommitHistory, branches: &[BranchInfo]) {
    let mut refs_by_commit: HashMap<&str, Vec<CommitRefDecoration>> = HashMap::new();
    for branch in branches {
        refs_by_commit
            .entry(branch.commit.as_str())
            .or_default()
            .push(CommitRefDecoration {
                name: branch.name.clone(),
                kind: CommitRefKind::Branch,
                is_current: branch.is_current,
                is_served: branch.is_served,
            });
    }

    for commit in &mut history.commits {
        if let Some(refs) = refs_by_commit.get(commit.id.as_str()) {
            let mut refs = refs.clone();
            refs.sort_by(|left, right| {
                right
                    .is_current
                    .cmp(&left.is_current)
                    .then_with(|| right.is_served.cmp(&left.is_served))
                    .then_with(|| left.name.cmp(&right.name))
            });
            commit.refs = refs;
        }
    }
}

impl WebUiServer {
    pub fn new(
        repo_read_store: Arc<dyn RepoReadStore>,
        workspace_service: Arc<WorkspaceService>,
        config: WebUiConfig,
    ) -> Self {
        Self {
            repo_read_store,
            workspace_service,
            workspace: config.workspace,
            repo_mount_path: config.repo_mount_path,
            credentials: config.credentials,
            implicit_owner_mode: config.implicit_owner_mode,
            secure_cookies: config.secure_cookies,
            public_repo_url: config.public_repo_url,
            sessions: Arc::new(RwLock::new(HashMap::new())),
            login_attempts: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    pub fn router(self) -> Router {
        let mount = self.repo_mount_path.clone();
        let state = Arc::new(self);
        Router::new()
            .route(&mount, get(index))
            .route(&format!("{mount}/"), get(index))
            .route(&format!("{mount}/assets/{{*asset_path}}"), get(asset))
            .route(&format!("{mount}/assets/qit-og.svg"), get(qit_og_image))
            .route(&format!("{mount}/api/bootstrap"), get(bootstrap))
            .route(&format!("{mount}/api/session/login"), post(login))
            .route(&format!("{mount}/api/session/logout"), post(logout))
            .route(&format!("{mount}/api/auth/mode"), post(update_auth_mode))
            .route(
                &format!("{mount}/api/access-requests"),
                post(create_access_request),
            )
            .route(
                &format!("{mount}/api/access-requests/status"),
                post(read_access_request_status),
            )
            .route(
                &format!("{mount}/api/access-requests/{{id}}/approve"),
                post(approve_access_request),
            )
            .route(
                &format!("{mount}/api/access-requests/{{id}}/reject"),
                post(reject_access_request),
            )
            .route(
                &format!("{mount}/api/users/setup-token"),
                post(issue_setup_token),
            )
            .route(
                &format!("{mount}/api/onboarding/complete"),
                post(complete_onboarding),
            )
            .route(
                &format!("{mount}/api/settings"),
                get(get_settings).patch(update_settings),
            )
            .route(
                &format!("{mount}/api/users/{{id}}/promote"),
                post(promote_user),
            )
            .route(
                &format!("{mount}/api/users/{{id}}/demote"),
                post(demote_user),
            )
            .route(
                &format!("{mount}/api/users/{{id}}/revoke"),
                post(revoke_user),
            )
            .route(
                &format!("{mount}/api/users/{{id}}/reset-setup"),
                post(reset_user_setup),
            )
            .route(&format!("{mount}/api/profile/pats"), post(create_pat))
            .route(
                &format!("{mount}/api/profile/pats/{{id}}"),
                delete(revoke_pat),
            )
            .route(
                &format!("{mount}/api/settings/branch-rules"),
                put(upsert_branch_rule),
            )
            .route(
                &format!("{mount}/api/settings/branch-rules/{{pattern}}"),
                delete(delete_branch_rule),
            )
            .route(
                &format!("{mount}/api/branches"),
                get(list_branches).post(create_branch),
            )
            .route(
                &format!("{mount}/api/branches/checkout"),
                post(checkout_branch),
            )
            .route(&format!("{mount}/api/branches/switch"), post(switch_branch))
            .route(
                &format!("{mount}/api/branches/{{name}}"),
                delete(delete_branch),
            )
            .route(&format!("{mount}/api/commits"), get(list_commits))
            .route(&format!("{mount}/api/commits/{{commit}}"), get(read_commit))
            .route(&format!("{mount}/api/code/tree"), get(list_tree))
            .route(&format!("{mount}/api/code/blob"), get(read_blob))
            .route(&format!("{mount}/api/code/raw"), get(read_blob_raw))
            .route(&format!("{mount}/api/compare"), get(compare_refs))
            .route(
                &format!("{mount}/api/pull-requests"),
                get(list_pull_requests).post(create_pull_request),
            )
            .route(
                &format!("{mount}/api/pull-requests/{{id}}"),
                get(read_pull_request)
                    .patch(update_pull_request)
                    .delete(delete_pull_request),
            )
            .route(
                &format!("{mount}/api/pull-requests/{{id}}/comments"),
                post(comment_pull_request),
            )
            .route(
                &format!("{mount}/api/pull-requests/{{id}}/reviews"),
                post(review_pull_request),
            )
            .route(
                &format!("{mount}/api/pull-requests/{{id}}/merge"),
                post(merge_pull_request),
            )
            .route(
                &format!("{mount}/api/issues"),
                get(list_issues).post(create_issue),
            )
            .route(&format!("{mount}/api/issues/meta"), get(issue_metadata))
            .route(
                &format!("{mount}/api/issues/labels"),
                put(upsert_issue_label),
            )
            .route(
                &format!("{mount}/api/issues/labels/{{id}}"),
                delete(delete_issue_label),
            )
            .route(
                &format!("{mount}/api/issues/milestones"),
                put(upsert_issue_milestone),
            )
            .route(
                &format!("{mount}/api/issues/milestones/{{id}}"),
                delete(delete_issue_milestone),
            )
            .route(
                &format!("{mount}/api/issues/{{id}}"),
                get(read_issue).patch(update_issue).delete(delete_issue),
            )
            .route(
                &format!("{mount}/api/issues/{{id}}/comments"),
                post(comment_issue),
            )
            .route(
                &format!("{mount}/api/issues/{{id}}/reactions"),
                post(react_issue),
            )
            .route(
                &format!("{mount}/api/issues/{{id}}/comments/{{comment_id}}/reactions"),
                post(react_issue_comment),
            )
            .route(
                &format!("{mount}/api/issues/{{id}}/labels"),
                put(set_issue_labels),
            )
            .route(
                &format!("{mount}/api/issues/{{id}}/assignees"),
                put(set_issue_assignees),
            )
            .route(
                &format!("{mount}/api/issues/{{id}}/milestone"),
                put(set_issue_milestone),
            )
            .route(
                &format!("{mount}/api/issues/{{id}}/links/pull-requests"),
                post(link_issue_pull_request),
            )
            .route(
                &format!("{mount}/api/issues/{{id}}/links/pull-requests/{{pull_request_id}}"),
                delete(unlink_issue_pull_request),
            )
            .with_state(state)
    }

    fn repo_name(&self) -> String {
        self.workspace
            .worktree
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("repo")
            .to_string()
    }

    fn cookie_name() -> &'static str {
        "qit_ui_session"
    }

    fn repo_og_svg(&self) -> String {
        let repo = self.repo_name();
        let title_lines = split_repo_name(&repo);
        let title_font_size = if title_lines
            .iter()
            .map(|line| line.chars().count())
            .max()
            .unwrap_or(0)
            > 20
        {
            54
        } else {
            64
        };
        let title_line_height = title_font_size + 12;
        let title_start_y = if title_lines.len() > 1 { 264 } else { 300 };
        let title_tspans = title_lines
            .iter()
            .enumerate()
            .map(|(index, line)| {
                format!(
                    r#"<tspan x="88" y="{}">{}</tspan>"#,
                    title_start_y + (index as i32 * title_line_height),
                    escape_html(line)
                )
            })
            .collect::<Vec<_>>()
            .join("");
        let logo_data_uri = format!(
            "data:image/png;base64,{}",
            BASE64_STANDARD.encode(QIT_LOGO_ON_DARK)
        );

        format!(
            r##"<svg xmlns="http://www.w3.org/2000/svg" width="1200" height="630" viewBox="0 0 1200 630" fill="none">
  <rect width="1200" height="630" rx="0" fill="#0D1117" />
  <rect x="40" y="40" width="1120" height="550" rx="32" fill="url(#panel)" stroke="#30363D" />
  <rect x="88" y="84" width="148" height="40" rx="20" fill="#161B22" stroke="#30363D" />
  <circle cx="112" cy="104" r="6" fill="#3FB950" />
  <text x="128" y="110" fill="#8B949E" font-family="Inter, ui-sans-serif, system-ui, sans-serif" font-size="20" font-weight="600">Hosted on Qit</text>
  <text x="88" y="184" fill="#8B949E" font-family="Inter, ui-sans-serif, system-ui, sans-serif" font-size="28" font-weight="500">Repository preview</text>
  <text fill="#F0F6FC" font-family="Inter, ui-sans-serif, system-ui, sans-serif" font-size="{title_font_size}" font-weight="700">{title_tspans}</text>
  <text x="88" y="470" fill="#8B949E" font-family="Inter, ui-sans-serif, system-ui, sans-serif" font-size="26" font-weight="500">Browse code, branches, pull requests, and clone details in one shareable session.</text>
  <rect x="88" y="506" width="252" height="36" rx="18" fill="#0D1117" stroke="#30363D" />
  <text x="112" y="530" fill="#7EE787" font-family="SFMono-Regular, ui-monospace, monospace" font-size="20" font-weight="600">qit</text>
  <text x="154" y="530" fill="#C9D1D9" font-family="SFMono-Regular, ui-monospace, monospace" font-size="20">serve {repo}</text>
  <rect x="834" y="84" width="278" height="278" rx="28" fill="#0D1117" stroke="#30363D" />
  <image href="{logo_data_uri}" x="856" y="106" width="234" height="234" preserveAspectRatio="xMidYMid meet" />
  <path d="M808 590C926 528 1011 458 1063 380" stroke="url(#accent)" stroke-width="20" stroke-linecap="round" opacity="0.9"/>
  <path d="M808 590C925 600 1027 596 1114 578" stroke="#21262D" stroke-width="14" stroke-linecap="round"/>
  <defs>
    <linearGradient id="panel" x1="72" y1="56" x2="1128" y2="574" gradientUnits="userSpaceOnUse">
      <stop stop-color="#11161D" />
      <stop offset="1" stop-color="#0D1117" />
    </linearGradient>
    <linearGradient id="accent" x1="808" y1="590" x2="1063" y2="380" gradientUnits="userSpaceOnUse">
      <stop stop-color="#19C176" />
      <stop offset="1" stop-color="#52E39A" />
    </linearGradient>
  </defs>
</svg>"##,
            logo_data_uri = logo_data_uri,
            repo = escape_html(&repo),
            title_font_size = title_font_size,
            title_tspans = title_tspans,
        )
    }

    fn index_html(&self) -> String {
        let repo = self.repo_name();
        let title = format!("{repo} · Qit");
        let favicon = format!("{}/assets/qit-logo-on-dark.png", self.repo_mount_path);
        let og_image = format!("{}/assets/qit-og.svg", self.repo_mount_path);
        format!(
            r#"<!doctype html>
<html lang="en">
  <head>
    <meta charset="UTF-8" />
    <meta name="viewport" content="width=device-width, initial-scale=1.0" />
    <meta name="qit-base" content="{base}" />
    <meta name="qit-repo" content="{repo}" />
    <meta property="og:title" content="{title}" />
    <meta property="og:image" content="{og_image}" />
    <meta property="og:image:type" content="image/svg+xml" />
    <meta property="og:image:width" content="1200" />
    <meta property="og:image:height" content="630" />
    <meta name="twitter:card" content="summary_large_image" />
    <meta name="twitter:image" content="{og_image}" />
    <title>{title}</title>
    <link rel="icon" type="image/png" href="{favicon}" />
    <link rel="stylesheet" href="{base}/assets/app.css" />
  </head>
  <body>
    <div id="root"></div>
    <script type="module" src="{base}/assets/app.js"></script>
  </body>
</html>"#,
            base = self.repo_mount_path,
            favicon = favicon,
            og_image = og_image,
            repo = repo,
            title = title
        )
    }

    fn latest_workspace(&self) -> Result<(WorkspaceSpec, WorkspaceWebUiState), StatusCode> {
        self.workspace_service
            .load_web_ui(
                self.workspace.worktree.clone(),
                &self.workspace.exported_branch,
            )
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
    }
}

fn settings_response(
    state: &WebUiServer,
    web_ui: &WorkspaceWebUiState,
    session: Option<&ResolvedSession>,
) -> SettingsResponse {
    let auth_mode = web_ui.auth.mode.clone();
    let auth_methods = web_ui.auth.methods.clone();
    let current_user = session.and_then(|session| session.principal.clone());
    let can_manage_access = session
        .map(|session| session.operator_override || session.actor == UiRole::Owner)
        .unwrap_or(false);
    let current_user_id = current_user.as_ref().map(|user| user.user_id.clone());
    SettingsResponse {
        auth_mode: auth_mode.clone(),
        auth_methods,
        local_only_owner_mode: state.implicit_owner_mode,
        shared_remote_identity: web_ui.auth.has_method(&AuthMethod::BasicAuth),
        current_user,
        users: if can_manage_access {
            web_ui.auth.users.iter().map(RepoUserView::from).collect()
        } else {
            Vec::new()
        },
        access_requests: if can_manage_access {
            web_ui
                .auth
                .access_requests
                .iter()
                .filter(|request| request.status == qit_domain::AccessRequestStatus::Pending)
                .map(AccessRequestView::from)
                .collect()
        } else {
            Vec::new()
        },
        personal_access_tokens: current_user_id
            .as_deref()
            .map(|user_id| {
                web_ui
                    .auth
                    .personal_access_tokens
                    .iter()
                    .filter(|token| token.user_id == user_id && token.revoked_at_ms.is_none())
                    .map(PatRecordView::from)
                    .collect()
            })
            .unwrap_or_default(),
        repository: web_ui.repository.clone(),
    }
}

fn auth_actor_from_session(session: &ResolvedSession) -> AuthActor {
    if session.operator_override {
        AuthActor::Operator
    } else if let Some(principal) = &session.principal {
        AuthActor::User {
            user_id: principal.user_id.clone(),
            username: principal.username.clone(),
            role: principal.role.clone(),
        }
    } else {
        AuthActor::Operator
    }
}

fn issue_actor_input_from_session(
    session: &ResolvedSession,
    display_name: Option<String>,
) -> IssueActorInput {
    IssueActorInput {
        role: session.actor.clone(),
        display_name,
        user_id: session
            .principal
            .as_ref()
            .map(|principal| principal.user_id.clone()),
        username: session
            .principal
            .as_ref()
            .map(|principal| principal.username.clone()),
    }
}

fn issue_viewer_actor(session: Option<&ResolvedSession>) -> Option<IssueActor> {
    session.map(|session| IssueActor {
        role: session.actor.clone(),
        display_name: session
            .principal
            .as_ref()
            .map(|principal| principal.name.clone())
            .unwrap_or_else(|| match session.actor {
                UiRole::Owner => "Owner".into(),
                UiRole::User => "Viewer".into(),
            }),
        user_id: session
            .principal
            .as_ref()
            .map(|principal| principal.user_id.clone()),
        username: session
            .principal
            .as_ref()
            .map(|principal| principal.username.clone()),
    })
}

fn issue_metadata_response(web_ui: &WorkspaceWebUiState) -> IssueMetadataResponse {
    IssueMetadataResponse {
        labels: web_ui.issue_settings.labels.clone(),
        milestones: web_ui.issue_settings.milestones.clone(),
        assignees: web_ui
            .auth
            .users
            .iter()
            .filter(|user| user.status == RepoUserStatus::Active)
            .filter_map(|user| {
                user.username.as_ref().map(|username| IssueAssigneeView {
                    id: user.id.clone(),
                    name: user.name.clone(),
                    username: username.clone(),
                    role: user.role.clone(),
                })
            })
            .collect(),
    }
}

fn issue_linked_pull_request_views(
    issue: &IssueRecord,
    web_ui: &WorkspaceWebUiState,
) -> Vec<IssueLinkedPullRequestView> {
    WorkspaceService::linked_pull_requests_for_issue(issue, &web_ui.issues, &web_ui.pull_requests)
        .into_iter()
        .filter_map(|link| {
            web_ui
                .pull_requests
                .iter()
                .find(|pull_request| pull_request.id == link.pull_request_id)
                .cloned()
                .map(|pull_request| IssueLinkedPullRequestView {
                    relation: link.relation,
                    source: link.source,
                    pull_request,
                })
        })
        .collect()
}

fn pull_request_linked_issue_views(
    pull_request: &PullRequestRecord,
    web_ui: &WorkspaceWebUiState,
) -> Vec<PullRequestLinkedIssueView> {
    WorkspaceService::linked_issues_for_pull_request(
        pull_request,
        &web_ui.issues,
        &web_ui.pull_requests,
    )
    .into_iter()
    .filter_map(|link| {
        web_ui
            .issues
            .iter()
            .find(|issue| issue.id == link.issue_id)
            .cloned()
            .map(|issue| PullRequestLinkedIssueView {
                relation: link.relation,
                source: link.source,
                issue,
            })
    })
    .collect()
}

fn domain_status(error: &DomainError) -> StatusCode {
    match error {
        DomainError::InvalidPullRequest(_)
        | DomainError::InvalidIssue(_)
        | DomainError::InvalidSettings(_)
        | DomainError::BranchRuleViolation(_)
        | DomainError::InvalidAuth(_)
        | DomainError::ExportedBranchConflict { .. } => StatusCode::BAD_REQUEST,
        DomainError::AccessRequestNotFound(_)
        | DomainError::UserNotFound(_)
        | DomainError::IssueNotFound(_)
        | DomainError::IssueLabelNotFound(_)
        | DomainError::IssueMilestoneNotFound(_)
        | DomainError::PatNotFound(_)
        | DomainError::MissingSidecar(_) => StatusCode::NOT_FOUND,
        DomainError::InvalidOnboardingToken | DomainError::AuthenticationFailed => {
            StatusCode::UNAUTHORIZED
        }
        _ => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

fn cookie_value<'a>(headers: &'a HeaderMap, name: &str) -> Option<&'a str> {
    let cookie_header = headers.get(axum::http::header::COOKIE)?.to_str().ok()?;
    cookie_header.split(';').find_map(|pair| {
        let (key, value) = pair.trim().split_once('=')?;
        if key == name {
            Some(value)
        } else {
            None
        }
    })
}

fn escape_html(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

fn truncate_with_ellipsis(value: &str, max_chars: usize) -> String {
    let chars: Vec<char> = value.chars().collect();
    if chars.len() <= max_chars {
        return value.to_string();
    }
    chars[..max_chars.saturating_sub(1)]
        .iter()
        .collect::<String>()
        + "…"
}

fn split_repo_name(repo: &str) -> Vec<String> {
    let repo = repo.trim();
    let chars: Vec<char> = repo.chars().collect();
    if chars.len() <= 18 {
        return vec![repo.to_string()];
    }

    let target = 18.min(chars.len().saturating_sub(1));
    let split_at = chars
        .iter()
        .enumerate()
        .take(target + 1)
        .filter_map(|(index, ch)| matches!(ch, '-' | '_' | '.' | ' ').then_some(index + 1))
        .last()
        .unwrap_or(target);

    let first = chars[..split_at].iter().collect::<String>();
    let second = chars[split_at..].iter().collect::<String>();
    let second = second.trim_matches(|ch: char| matches!(ch, '-' | '_' | '.' | ' '));
    let second = if second.is_empty() {
        truncate_with_ellipsis(repo, 18)
    } else {
        truncate_with_ellipsis(second, 22)
    };

    vec![first, second]
}

fn secure_eq(left: &str, right: &str) -> bool {
    if left.len() != right.len() {
        return false;
    }
    left.as_bytes()
        .iter()
        .zip(right.as_bytes())
        .fold(0_u8, |acc, (lhs, rhs)| acc | (lhs ^ rhs))
        == 0
}

fn credentials_match(username: &str, password: &str, credentials: &SessionCredentials) -> bool {
    secure_eq(username, &credentials.username) && secure_eq(password, &credentials.password)
}

async fn index(State(state): State<Arc<WebUiServer>>) -> Html<String> {
    Html(state.index_html())
}

fn static_asset(asset_path: &str) -> Option<(&'static str, &'static [u8])> {
    match asset_path {
        "app.js" => Some(("text/javascript; charset=utf-8", APP_JS)),
        "app.css" => Some(("text/css; charset=utf-8", APP_CSS)),
        "chunk-rolldown-runtime.js" => {
            Some(("text/javascript; charset=utf-8", CHUNK_ROLLDOWN_RUNTIME_JS))
        }
        "chunk-vendor.js" => Some(("text/javascript; charset=utf-8", CHUNK_VENDOR_JS)),
        "chunk-vendor-ui.js" => Some(("text/javascript; charset=utf-8", CHUNK_VENDOR_UI_JS)),
        "chunk-vendor-markdown.js" => {
            Some(("text/javascript; charset=utf-8", CHUNK_VENDOR_MARKDOWN_JS))
        }
        "chunk-vendor-tree.js" => Some(("text/javascript; charset=utf-8", CHUNK_VENDOR_TREE_JS)),
        "chunk-vendor-monaco.js" => {
            Some(("text/javascript; charset=utf-8", CHUNK_VENDOR_MONACO_JS))
        }
        "chunk-MonacoCodeSurface.js" => Some((
            "text/javascript; charset=utf-8",
            CHUNK_MONACO_CODE_SURFACE_JS,
        )),
        "qit-logo-on-dark.png" => Some(("image/png", QIT_LOGO_ON_DARK)),
        "qit-logo-on-light.png" => Some(("image/png", QIT_LOGO_ON_LIGHT)),
        _ => None,
    }
}

async fn asset(AxumPath(asset_path): AxumPath<String>) -> Result<impl IntoResponse, StatusCode> {
    let Some((content_type, body)) = static_asset(&asset_path) else {
        return Err(StatusCode::NOT_FOUND);
    };
    Ok((
        [(CONTENT_TYPE, HeaderValue::from_static(content_type))],
        body,
    ))
}

async fn qit_og_image(State(state): State<Arc<WebUiServer>>) -> impl IntoResponse {
    (
        [(
            CONTENT_TYPE,
            HeaderValue::from_static("image/svg+xml; charset=utf-8"),
        )],
        state.repo_og_svg(),
    )
}

async fn bootstrap(
    State(state): State<Arc<WebUiServer>>,
    headers: HeaderMap,
) -> Result<Json<BootstrapResponse>, StatusCode> {
    let (workspace, web_ui) = state.latest_workspace()?;
    let session = state.current_session(&headers).await?;
    let auth_mode = web_ui.auth.mode.clone();
    let git_credentials_visible =
        state.can_view_git_credentials(session.as_ref(), &web_ui.auth.methods);
    Ok(Json(BootstrapResponse {
        actor: session.as_ref().map(|session| session.actor.clone()),
        principal: session
            .as_ref()
            .and_then(|session| session.principal.clone()),
        repo_name: state.repo_name(),
        worktree: workspace.worktree.display().to_string(),
        exported_branch: workspace.exported_branch,
        checked_out_branch: workspace.checked_out_branch,
        description: web_ui.repository.description,
        homepage_url: web_ui.repository.homepage_url,
        auth_mode: auth_mode.clone(),
        auth_methods: web_ui.auth.methods.clone(),
        operator_override: session
            .as_ref()
            .map(|session| session.operator_override)
            .unwrap_or(false),
        local_only_owner_mode: state.implicit_owner_mode,
        shared_remote_identity: web_ui.auth.has_method(&AuthMethod::BasicAuth),
        git_credentials_visible,
        git_username: git_credentials_visible.then(|| state.credentials.username.clone()),
        git_password: git_credentials_visible.then(|| state.credentials.password.clone()),
        public_repo_url: state.public_repo_url.clone(),
    }))
}

async fn login(
    State(state): State<Arc<WebUiServer>>,
    headers: HeaderMap,
    Json(body): Json<LoginRequest>,
) -> Result<Response<Body>, StatusCode> {
    if let Some(session) = state.default_session(&headers) {
        let payload =
            serde_json::to_vec(&session.actor).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        return Response::builder()
            .status(StatusCode::OK)
            .header(CONTENT_TYPE, "application/json")
            .header(SET_COOKIE, state.clear_cookie())
            .body(Body::from(payload))
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR);
    }
    state.allow_login_attempt(&headers).await?;
    let (_, auth) = state
        .workspace_service
        .read_auth_state(
            state.workspace.worktree.clone(),
            &state.workspace.exported_branch,
        )
        .map_err(|error| domain_status(&error))?;
    let session_record = if auth.has_method(&AuthMethod::BasicAuth)
        && credentials_match(&body.username, &body.password, &state.credentials)
    {
        SessionRecord {
            user_id: None,
            role: UiRole::User,
            expires_at_ms: WebUiServer::now_ms().saturating_add(SESSION_TTL_MS),
        }
    } else {
        let (_, principal) = match state.workspace_service.authenticate_web_user(
            state.workspace.worktree.clone(),
            &state.workspace.exported_branch,
            &body.username,
            &body.password,
        ) {
            Ok(value) => value,
            Err(error) => {
                if matches!(error, DomainError::AuthenticationFailed) {
                    state.record_login_failure(&headers).await;
                    return Err(StatusCode::UNAUTHORIZED);
                }
                return Err(domain_status(&error));
            }
        };
        SessionRecord {
            user_id: Some(principal.user_id.clone()),
            role: principal.ui_role(),
            expires_at_ms: WebUiServer::now_ms().saturating_add(SESSION_TTL_MS),
        }
    };
    state.clear_login_attempts(&headers).await;
    let token = Alphanumeric.sample_string(&mut rand::thread_rng(), 32);
    state
        .sessions
        .write()
        .await
        .insert(token.clone(), session_record.clone());
    let payload =
        serde_json::to_vec(&session_record.role).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Response::builder()
        .status(StatusCode::OK)
        .header(CONTENT_TYPE, "application/json")
        .header(SET_COOKIE, state.session_cookie(&token))
        .body(Body::from(payload))
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

async fn logout(
    State(state): State<Arc<WebUiServer>>,
    headers: HeaderMap,
) -> Result<Response<Body>, StatusCode> {
    if let Some(token) = cookie_value(&headers, WebUiServer::cookie_name()) {
        state.sessions.write().await.remove(token);
    }
    Response::builder()
        .status(StatusCode::NO_CONTENT)
        .header(SET_COOKIE, state.clear_cookie())
        .body(Body::empty())
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

async fn update_auth_mode(
    State(state): State<Arc<WebUiServer>>,
    headers: HeaderMap,
    Json(body): Json<AuthModeUpdateRequest>,
) -> Result<Json<SettingsResponse>, StatusCode> {
    let session = state.require_session(&headers).await?;
    if session.actor != UiRole::Owner {
        return Err(StatusCode::FORBIDDEN);
    }
    let methods = if let Some(methods) = body.methods {
        methods
    } else if let Some(mode) = body.mode {
        qit_domain::RepoAuthState::methods_for_mode(&mode)
    } else {
        return Err(StatusCode::BAD_REQUEST);
    };
    state
        .workspace_service
        .update_auth_methods(
            state.workspace.worktree.clone(),
            &state.workspace.exported_branch,
            methods,
            &auth_actor_from_session(&session),
        )
        .map_err(|error| domain_status(&error))?;
    let (_, web_ui) = state.latest_workspace()?;
    Ok(Json(settings_response(&state, &web_ui, Some(&session))))
}

async fn create_access_request(
    State(state): State<Arc<WebUiServer>>,
    Json(body): Json<AccessRequestCreateRequest>,
) -> Result<Json<qit_domain::SubmittedAccessRequest>, StatusCode> {
    let (_, request) = state
        .workspace_service
        .submit_access_request(
            state.workspace.worktree.clone(),
            &state.workspace.exported_branch,
            &body.name,
            &body.email,
        )
        .map_err(|error| domain_status(&error))?;
    Ok(Json(request))
}

async fn read_access_request_status(
    State(state): State<Arc<WebUiServer>>,
    Json(body): Json<AccessRequestStatusRequest>,
) -> Result<Json<qit_domain::AccessRequestProgress>, StatusCode> {
    let (_, progress) = state
        .workspace_service
        .read_access_request_progress(
            state.workspace.worktree.clone(),
            &state.workspace.exported_branch,
            &body.token,
        )
        .map_err(|error| domain_status(&error))?;
    Ok(Json(progress))
}

async fn approve_access_request(
    State(state): State<Arc<WebUiServer>>,
    headers: HeaderMap,
    AxumPath(id): AxumPath<String>,
) -> Result<Json<qit_domain::IssuedOnboarding>, StatusCode> {
    let session = state.require_session(&headers).await?;
    if session.actor != UiRole::Owner {
        return Err(StatusCode::FORBIDDEN);
    }
    let (_, _user, onboarding) = state
        .workspace_service
        .approve_access_request(
            state.workspace.worktree.clone(),
            &state.workspace.exported_branch,
            &id,
            RepoUserRole::User,
            &auth_actor_from_session(&session),
        )
        .map_err(|error| domain_status(&error))?;
    Ok(Json(onboarding))
}

async fn reject_access_request(
    State(state): State<Arc<WebUiServer>>,
    headers: HeaderMap,
    AxumPath(id): AxumPath<String>,
) -> Result<Json<AccessRequestView>, StatusCode> {
    let session = state.require_session(&headers).await?;
    if session.actor != UiRole::Owner {
        return Err(StatusCode::FORBIDDEN);
    }
    let (_, request) = state
        .workspace_service
        .reject_access_request(
            state.workspace.worktree.clone(),
            &state.workspace.exported_branch,
            &id,
            &auth_actor_from_session(&session),
        )
        .map_err(|error| domain_status(&error))?;
    Ok(Json(request))
}

async fn complete_onboarding(
    State(state): State<Arc<WebUiServer>>,
    Json(body): Json<OnboardingCompleteRequest>,
) -> Result<Response<Body>, StatusCode> {
    let (_, principal) = state
        .workspace_service
        .complete_onboarding(
            state.workspace.worktree.clone(),
            &state.workspace.exported_branch,
            &body.token,
            &body.username,
            &body.password,
        )
        .map_err(|error| domain_status(&error))?;
    let token = Alphanumeric.sample_string(&mut rand::thread_rng(), 32);
    state.sessions.write().await.insert(
        token.clone(),
        SessionRecord {
            user_id: Some(principal.user_id.clone()),
            role: principal.ui_role(),
            expires_at_ms: WebUiServer::now_ms().saturating_add(SESSION_TTL_MS),
        },
    );
    let payload =
        serde_json::to_vec(&principal.ui_role()).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Response::builder()
        .status(StatusCode::OK)
        .header(CONTENT_TYPE, "application/json")
        .header(SET_COOKIE, state.session_cookie(&token))
        .body(Body::from(payload))
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

async fn issue_setup_token(
    State(state): State<Arc<WebUiServer>>,
    headers: HeaderMap,
    Json(body): Json<ManualSetupTokenRequest>,
) -> Result<Json<qit_domain::IssuedOnboarding>, StatusCode> {
    let session = state.require_session(&headers).await?;
    if session.actor != UiRole::Owner {
        return Err(StatusCode::FORBIDDEN);
    }
    let (_, _user, onboarding) = state
        .workspace_service
        .issue_setup_token(
            state.workspace.worktree.clone(),
            &state.workspace.exported_branch,
            &body.name,
            &body.email,
            RepoUserRole::User,
            &auth_actor_from_session(&session),
        )
        .map_err(|error| domain_status(&error))?;
    Ok(Json(onboarding))
}

async fn promote_user(
    State(state): State<Arc<WebUiServer>>,
    headers: HeaderMap,
    AxumPath(id): AxumPath<String>,
) -> Result<Json<RepoUserView>, StatusCode> {
    let session = state.require_session(&headers).await?;
    if session.actor != UiRole::Owner {
        return Err(StatusCode::FORBIDDEN);
    }
    let (_, user) = state
        .workspace_service
        .promote_user(
            state.workspace.worktree.clone(),
            &state.workspace.exported_branch,
            &id,
            &auth_actor_from_session(&session),
        )
        .map_err(|error| domain_status(&error))?;
    Ok(Json(user))
}

async fn demote_user(
    State(state): State<Arc<WebUiServer>>,
    headers: HeaderMap,
    AxumPath(id): AxumPath<String>,
) -> Result<Json<RepoUserView>, StatusCode> {
    let session = state.require_session(&headers).await?;
    if session.actor != UiRole::Owner {
        return Err(StatusCode::FORBIDDEN);
    }
    let (_, user) = state
        .workspace_service
        .demote_user(
            state.workspace.worktree.clone(),
            &state.workspace.exported_branch,
            &id,
            &auth_actor_from_session(&session),
        )
        .map_err(|error| domain_status(&error))?;
    Ok(Json(user))
}

async fn revoke_user(
    State(state): State<Arc<WebUiServer>>,
    headers: HeaderMap,
    AxumPath(id): AxumPath<String>,
) -> Result<Json<RepoUserView>, StatusCode> {
    let session = state.require_session(&headers).await?;
    if session.actor != UiRole::Owner {
        return Err(StatusCode::FORBIDDEN);
    }
    let (_, user) = state
        .workspace_service
        .revoke_user(
            state.workspace.worktree.clone(),
            &state.workspace.exported_branch,
            &id,
            &auth_actor_from_session(&session),
        )
        .map_err(|error| domain_status(&error))?;
    Ok(Json(user))
}

async fn reset_user_setup(
    State(state): State<Arc<WebUiServer>>,
    headers: HeaderMap,
    AxumPath(id): AxumPath<String>,
) -> Result<Json<qit_domain::IssuedOnboarding>, StatusCode> {
    let session = state.require_session(&headers).await?;
    if session.actor != UiRole::Owner {
        return Err(StatusCode::FORBIDDEN);
    }
    let (_, _user, onboarding) = state
        .workspace_service
        .reset_user_setup(
            state.workspace.worktree.clone(),
            &state.workspace.exported_branch,
            &id,
            &auth_actor_from_session(&session),
        )
        .map_err(|error| domain_status(&error))?;
    Ok(Json(onboarding))
}

async fn create_pat(
    State(state): State<Arc<WebUiServer>>,
    headers: HeaderMap,
    Json(body): Json<PatCreateRequest>,
) -> Result<Json<qit_domain::IssuedPat>, StatusCode> {
    let session = state.require_session(&headers).await?;
    let Some(principal) = session.principal.clone() else {
        return Err(StatusCode::FORBIDDEN);
    };
    let (_, _pat, issued) = state
        .workspace_service
        .create_pat(
            state.workspace.worktree.clone(),
            &state.workspace.exported_branch,
            &principal.user_id,
            &body.label,
            &auth_actor_from_session(&session),
        )
        .map_err(|error| domain_status(&error))?;
    Ok(Json(issued))
}

async fn revoke_pat(
    State(state): State<Arc<WebUiServer>>,
    headers: HeaderMap,
    AxumPath(id): AxumPath<String>,
) -> Result<Json<PatRecordView>, StatusCode> {
    let session = state.require_session(&headers).await?;
    let Some(principal) = session.principal.clone() else {
        return Err(StatusCode::FORBIDDEN);
    };
    let (_, web_ui) = state.latest_workspace()?;
    let allowed = web_ui
        .auth
        .personal_access_tokens
        .iter()
        .any(|token| token.id == id && token.user_id == principal.user_id);
    if !allowed {
        return Err(StatusCode::FORBIDDEN);
    }
    let (_, pat) = state
        .workspace_service
        .revoke_pat(
            state.workspace.worktree.clone(),
            &state.workspace.exported_branch,
            &id,
            &auth_actor_from_session(&session),
        )
        .map_err(|error| domain_status(&error))?;
    Ok(Json(pat))
}

async fn get_settings(
    State(state): State<Arc<WebUiServer>>,
    headers: HeaderMap,
) -> Result<Json<SettingsResponse>, StatusCode> {
    let session = state.require_session(&headers).await?;
    let (_, web_ui) = state.latest_workspace()?;
    Ok(Json(settings_response(&state, &web_ui, Some(&session))))
}

async fn update_settings(
    State(state): State<Arc<WebUiServer>>,
    headers: HeaderMap,
    Json(body): Json<SettingsUpdateRequest>,
) -> Result<Json<SettingsResponse>, StatusCode> {
    let session = state.require_session(&headers).await?;
    if session.actor != UiRole::Owner {
        return Err(StatusCode::FORBIDDEN);
    }
    state
        .workspace_service
        .update_repository_settings(
            state.workspace.worktree.clone(),
            &state.workspace.exported_branch,
            UpdateRepositorySettings {
                description: body.description,
                homepage_url: body.homepage_url,
            },
        )
        .map_err(|error| domain_status(&error))?;
    let (_, web_ui) = state.latest_workspace()?;
    Ok(Json(settings_response(&state, &web_ui, Some(&session))))
}

async fn upsert_branch_rule(
    State(state): State<Arc<WebUiServer>>,
    headers: HeaderMap,
    Json(body): Json<BranchRuleRequest>,
) -> Result<Json<SettingsResponse>, StatusCode> {
    let session = state.require_session(&headers).await?;
    if session.actor != UiRole::Owner {
        return Err(StatusCode::FORBIDDEN);
    }
    state
        .workspace_service
        .upsert_branch_rule(
            state.workspace.worktree.clone(),
            &state.workspace.exported_branch,
            UpsertBranchRule {
                pattern: body.pattern,
                require_pull_request: body.require_pull_request,
                required_approvals: body.required_approvals,
                dismiss_stale_approvals: body.dismiss_stale_approvals,
                block_force_push: body.block_force_push,
                block_delete: body.block_delete,
            },
        )
        .map_err(|error| domain_status(&error))?;
    let (_, web_ui) = state.latest_workspace()?;
    Ok(Json(settings_response(&state, &web_ui, Some(&session))))
}

async fn delete_branch_rule(
    State(state): State<Arc<WebUiServer>>,
    headers: HeaderMap,
    AxumPath(pattern): AxumPath<String>,
) -> Result<Json<SettingsResponse>, StatusCode> {
    let session = state.require_session(&headers).await?;
    if session.actor != UiRole::Owner {
        return Err(StatusCode::FORBIDDEN);
    }
    state
        .workspace_service
        .delete_branch_rule(
            state.workspace.worktree.clone(),
            &state.workspace.exported_branch,
            &pattern,
        )
        .map_err(|error| domain_status(&error))?;
    let (_, web_ui) = state.latest_workspace()?;
    Ok(Json(settings_response(&state, &web_ui, Some(&session))))
}

async fn list_branches(
    State(state): State<Arc<WebUiServer>>,
    headers: HeaderMap,
) -> Result<Json<BranchesResponse>, StatusCode> {
    state.require_actor(&headers).await?;
    let (_, branches) = state
        .workspace_service
        .list_branches(
            state.workspace.worktree.clone(),
            &state.workspace.exported_branch,
            &[],
        )
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(BranchesResponse { branches }))
}

async fn create_branch(
    State(state): State<Arc<WebUiServer>>,
    headers: HeaderMap,
    Json(body): Json<BranchCreateRequest>,
) -> Result<Json<BranchMutationResponse>, StatusCode> {
    state.require_owner(&headers).await?;
    let (workspace, _) = state
        .workspace_service
        .create_branch(
            state.workspace.worktree.clone(),
            &state.workspace.exported_branch,
            &body.name,
            body.start_point.as_deref(),
            body.force,
        )
        .await
        .map_err(|_| StatusCode::BAD_REQUEST)?;
    Ok(Json(BranchMutationResponse {
        exported_branch: workspace.exported_branch,
        checked_out_branch: workspace.checked_out_branch,
    }))
}

async fn checkout_branch(
    State(state): State<Arc<WebUiServer>>,
    headers: HeaderMap,
    Json(body): Json<BranchSelectionRequest>,
) -> Result<Json<BranchMutationResponse>, StatusCode> {
    state.require_owner(&headers).await?;
    let (workspace, _) = state
        .workspace_service
        .checkout_branch_with_force(
            state.workspace.worktree.clone(),
            &state.workspace.exported_branch,
            &body.name,
            body.force,
        )
        .await
        .map_err(|_| StatusCode::BAD_REQUEST)?;
    Ok(Json(BranchMutationResponse {
        exported_branch: workspace.exported_branch,
        checked_out_branch: workspace.checked_out_branch,
    }))
}

async fn switch_branch(
    State(state): State<Arc<WebUiServer>>,
    headers: HeaderMap,
    Json(body): Json<BranchSelectionRequest>,
) -> Result<Json<BranchMutationResponse>, StatusCode> {
    state.require_owner(&headers).await?;
    let (workspace, _) = state
        .workspace_service
        .switch_branch(
            state.workspace.worktree.clone(),
            &state.workspace.exported_branch,
            &body.name,
        )
        .await
        .map_err(|_| StatusCode::BAD_REQUEST)?;
    Ok(Json(BranchMutationResponse {
        exported_branch: workspace.exported_branch,
        checked_out_branch: workspace.checked_out_branch,
    }))
}

async fn delete_branch(
    State(state): State<Arc<WebUiServer>>,
    headers: HeaderMap,
    AxumPath(name): AxumPath<String>,
) -> Result<StatusCode, StatusCode> {
    state.require_owner(&headers).await?;
    state
        .workspace_service
        .delete_branch(
            state.workspace.worktree.clone(),
            &state.workspace.exported_branch,
            &name,
            false,
        )
        .await
        .map_err(|_| StatusCode::BAD_REQUEST)?;
    Ok(StatusCode::NO_CONTENT)
}

async fn list_commits(
    State(state): State<Arc<WebUiServer>>,
    headers: HeaderMap,
    Query(query): Query<CommitListQuery>,
) -> Result<Json<CommitsResponse>, StatusCode> {
    state.require_actor(&headers).await?;
    let workspace = state.latest_workspace()?.0;
    let reference = query
        .reference
        .unwrap_or_else(|| workspace.checked_out_branch.clone());
    let offset = query.offset.unwrap_or(0);
    let limit = clamp_history_limit(query.limit);
    let mut history = state
        .repo_read_store
        .list_commits(&workspace, Some(&reference), offset, limit)
        .await
        .map_err(|_| StatusCode::BAD_REQUEST)?;
    let branches = state
        .workspace_service
        .list_branches(workspace.worktree.clone(), &workspace.exported_branch, &[])
        .await
        .map_err(|_| StatusCode::BAD_REQUEST)?
        .1;
    decorate_history(&mut history, &branches);
    Ok(Json(CommitsResponse { history }))
}

async fn read_commit(
    State(state): State<Arc<WebUiServer>>,
    headers: HeaderMap,
    AxumPath(commit): AxumPath<String>,
) -> Result<Json<CommitDetail>, StatusCode> {
    state.require_actor(&headers).await?;
    let detail = state
        .repo_read_store
        .read_commit(&state.latest_workspace()?.0, &commit)
        .await
        .map_err(|_| StatusCode::BAD_REQUEST)?;
    Ok(Json(detail))
}

async fn list_tree(
    State(state): State<Arc<WebUiServer>>,
    headers: HeaderMap,
    Query(query): Query<TreeQuery>,
) -> Result<Json<TreeResponse>, StatusCode> {
    state.require_actor(&headers).await?;
    let workspace = state.latest_workspace()?.0;
    let reference = query
        .reference
        .unwrap_or_else(|| workspace.checked_out_branch.clone());
    let path = query.path.map(PathBuf::from);
    let entries = state
        .repo_read_store
        .list_tree(&workspace, &reference, path.as_deref())
        .await
        .map_err(|_| StatusCode::BAD_REQUEST)?;
    Ok(Json(TreeResponse { entries }))
}

async fn read_blob(
    State(state): State<Arc<WebUiServer>>,
    headers: HeaderMap,
    Query(query): Query<BlobQuery>,
) -> Result<Json<BlobResponse>, StatusCode> {
    state.require_actor(&headers).await?;
    let workspace = state.latest_workspace()?.0;
    let reference = query
        .reference
        .unwrap_or_else(|| workspace.checked_out_branch.clone());
    let blob = state
        .repo_read_store
        .read_blob(&workspace, &reference, PathBuf::from(query.path).as_path())
        .await
        .map_err(|_| StatusCode::BAD_REQUEST)?;
    Ok(Json(BlobResponse { blob }))
}

async fn read_blob_raw(
    State(state): State<Arc<WebUiServer>>,
    headers: HeaderMap,
    Query(query): Query<BlobQuery>,
) -> Result<Response<Body>, StatusCode> {
    state.require_actor(&headers).await?;
    let workspace = state.latest_workspace()?.0;
    let reference = query
        .reference
        .unwrap_or_else(|| workspace.checked_out_branch.clone());
    let blob = state
        .repo_read_store
        .read_blob(&workspace, &reference, PathBuf::from(query.path).as_path())
        .await
        .map_err(|_| StatusCode::BAD_REQUEST)?;

    if blob.is_binary {
        return Err(StatusCode::BAD_REQUEST);
    }

    Response::builder()
        .status(StatusCode::OK)
        .header(CONTENT_TYPE, "text/plain; charset=utf-8")
        .body(Body::from(blob.text.unwrap_or_default()))
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

async fn compare_refs(
    State(state): State<Arc<WebUiServer>>,
    headers: HeaderMap,
    Query(query): Query<CompareQuery>,
) -> Result<Json<CompareResponse>, StatusCode> {
    state.require_actor(&headers).await?;
    let comparison = state
        .repo_read_store
        .compare_refs(&state.latest_workspace()?.0, &query.base, &query.head, 25)
        .await
        .map_err(|_| StatusCode::BAD_REQUEST)?;
    Ok(Json(CompareResponse { comparison }))
}

async fn list_pull_requests(
    State(state): State<Arc<WebUiServer>>,
    headers: HeaderMap,
) -> Result<Json<PullRequestsResponse>, StatusCode> {
    state.require_actor(&headers).await?;
    let (_, web_ui) = state.latest_workspace()?;
    Ok(Json(PullRequestsResponse {
        pull_requests: web_ui.pull_requests,
    }))
}

async fn read_pull_request(
    State(state): State<Arc<WebUiServer>>,
    headers: HeaderMap,
    AxumPath(id): AxumPath<String>,
) -> Result<Json<PullRequestDetailResponse>, StatusCode> {
    state.require_actor(&headers).await?;
    let (workspace, web_ui) = state.latest_workspace()?;
    let Some(pull_request) = web_ui
        .pull_requests
        .iter()
        .find(|pull_request| pull_request.id == id)
        .cloned()
    else {
        return Err(StatusCode::NOT_FOUND);
    };
    let (base_ref, head_ref) = qit_domain::resolve_pull_request_refs(
        state.repo_read_store.as_ref(),
        &workspace,
        &pull_request,
    )
    .await;

    let comparison_result = state
        .repo_read_store
        .compare_refs(&workspace, &base_ref, &head_ref, 25)
        .await;
    let diffs_result = state
        .repo_read_store
        .diff_refs(&workspace, &base_ref, &head_ref)
        .await;
    let comparison = comparison_result.ok();
    let diffs = diffs_result.ok();
    let linked_issues = pull_request_linked_issue_views(&pull_request, &web_ui);
    let comments = WorkspaceService::pull_request_comments(&pull_request);
    let reviews = WorkspaceService::pull_request_reviews(&pull_request);
    let review_summary = WorkspaceService::pull_request_review_summary(&pull_request);
    let activity = pull_request.activities.clone();

    Ok(Json(PullRequestDetailResponse {
        pull_request,
        comparison,
        diffs,
        linked_issues,
        comments,
        reviews,
        review_summary,
        activity,
    }))
}

async fn create_pull_request(
    State(state): State<Arc<WebUiServer>>,
    headers: HeaderMap,
    Json(body): Json<PullRequestCreateRequest>,
) -> Result<Json<PullRequestRecord>, StatusCode> {
    let actor = state.require_actor(&headers).await?;
    let (_, pull_request) = state
        .workspace_service
        .create_pull_request(
            state.workspace.worktree.clone(),
            &state.workspace.exported_branch,
            CreatePullRequest {
                title: body.title,
                description: body.description,
                source_branch: body.source_branch,
                target_branch: body.target_branch,
            },
            actor,
        )
        .await
        .map_err(|_| StatusCode::BAD_REQUEST)?;
    Ok(Json(pull_request))
}

async fn update_pull_request(
    State(state): State<Arc<WebUiServer>>,
    headers: HeaderMap,
    AxumPath(id): AxumPath<String>,
    Json(body): Json<PullRequestUpdateRequest>,
) -> Result<Json<PullRequestRecord>, StatusCode> {
    let actor = state.require_actor(&headers).await?;
    if actor != UiRole::Owner {
        return Err(StatusCode::FORBIDDEN);
    }
    let status = body.status.map(|status| match status {
        PullRequestStatusRequest::Open => PullRequestStatus::Open,
        PullRequestStatusRequest::Closed => PullRequestStatus::Closed,
    });
    let (_, pull_request) = state
        .workspace_service
        .update_pull_request(
            state.workspace.worktree.clone(),
            &state.workspace.exported_branch,
            &id,
            UpdatePullRequest {
                title: body.title,
                description: body.description,
                status,
            },
            actor,
        )
        .await
        .map_err(|_| StatusCode::BAD_REQUEST)?;
    Ok(Json(pull_request))
}

async fn delete_pull_request(
    State(state): State<Arc<WebUiServer>>,
    headers: HeaderMap,
    AxumPath(id): AxumPath<String>,
) -> Result<Json<PullRequestRecord>, StatusCode> {
    state.require_owner(&headers).await?;
    let (_, pull_request) = state
        .workspace_service
        .delete_pull_request(
            state.workspace.worktree.clone(),
            &state.workspace.exported_branch,
            &id,
        )
        .await
        .map_err(|_| StatusCode::BAD_REQUEST)?;
    Ok(Json(pull_request))
}

async fn comment_pull_request(
    State(state): State<Arc<WebUiServer>>,
    headers: HeaderMap,
    AxumPath(id): AxumPath<String>,
    Json(body): Json<PullRequestCommentRequest>,
) -> Result<Json<PullRequestRecord>, StatusCode> {
    let actor = state.require_actor(&headers).await?;
    let (_, pull_request) = state
        .workspace_service
        .comment_pull_request(
            state.workspace.worktree.clone(),
            &state.workspace.exported_branch,
            &id,
            CreatePullRequestComment {
                display_name: body.display_name,
                body: body.body,
            },
            actor,
        )
        .await
        .map_err(|_| StatusCode::BAD_REQUEST)?;
    Ok(Json(pull_request))
}

async fn review_pull_request(
    State(state): State<Arc<WebUiServer>>,
    headers: HeaderMap,
    AxumPath(id): AxumPath<String>,
    Json(body): Json<PullRequestReviewRequest>,
) -> Result<Json<PullRequestRecord>, StatusCode> {
    let actor = state.require_actor(&headers).await?;
    let (_, pull_request) = state
        .workspace_service
        .review_pull_request(
            state.workspace.worktree.clone(),
            &state.workspace.exported_branch,
            &id,
            CreatePullRequestReview {
                display_name: body.display_name,
                body: body.body,
                state: body.state,
            },
            actor,
        )
        .await
        .map_err(|_| StatusCode::BAD_REQUEST)?;
    Ok(Json(pull_request))
}

async fn merge_pull_request(
    State(state): State<Arc<WebUiServer>>,
    headers: HeaderMap,
    AxumPath(id): AxumPath<String>,
) -> Result<Json<PullRequestRecord>, StatusCode> {
    let session = state.require_session(&headers).await?;
    if session.actor != UiRole::Owner {
        return Err(StatusCode::FORBIDDEN);
    }
    let (_, pull_request) = state
        .workspace_service
        .merge_pull_request(
            state.workspace.worktree.clone(),
            &state.workspace.exported_branch,
            &id,
            issue_actor_input_from_session(&session, None),
        )
        .await
        .map_err(|_| StatusCode::BAD_REQUEST)?;
    Ok(Json(pull_request))
}

async fn list_issues(
    State(state): State<Arc<WebUiServer>>,
    headers: HeaderMap,
) -> Result<Json<IssuesResponse>, StatusCode> {
    state.require_actor(&headers).await?;
    let (_, web_ui) = state.latest_workspace()?;
    Ok(Json(IssuesResponse {
        issues: web_ui.issues,
    }))
}

async fn issue_metadata(
    State(state): State<Arc<WebUiServer>>,
    headers: HeaderMap,
) -> Result<Json<IssueMetadataResponse>, StatusCode> {
    state.require_actor(&headers).await?;
    let (_, web_ui) = state.latest_workspace()?;
    Ok(Json(issue_metadata_response(&web_ui)))
}

async fn read_issue(
    State(state): State<Arc<WebUiServer>>,
    headers: HeaderMap,
    AxumPath(id): AxumPath<String>,
) -> Result<Json<IssueDetailResponse>, StatusCode> {
    let session = state.require_session(&headers).await?;
    let (_, web_ui) = state.latest_workspace()?;
    let issue = web_ui
        .issues
        .iter()
        .find(|issue| issue.id == id)
        .cloned()
        .ok_or(StatusCode::NOT_FOUND)?;
    let viewer_actor = issue_viewer_actor(Some(&session));
    let comments = issue
        .comments
        .iter()
        .cloned()
        .map(|comment| IssueCommentResponse {
            reaction_summary: WorkspaceService::issue_comment_reaction_summary(
                &comment,
                viewer_actor.as_ref(),
            ),
            comment,
        })
        .collect();
    let linked_pull_requests = issue_linked_pull_request_views(&issue, &web_ui);
    Ok(Json(IssueDetailResponse {
        reaction_summary: WorkspaceService::issue_reaction_summary(&issue, viewer_actor.as_ref()),
        comments,
        timeline: issue.timeline.clone(),
        linked_pull_requests,
        metadata: issue_metadata_response(&web_ui),
        issue,
    }))
}

async fn create_issue(
    State(state): State<Arc<WebUiServer>>,
    headers: HeaderMap,
    Json(body): Json<IssueCreateRequest>,
) -> Result<Json<IssueRecord>, StatusCode> {
    let session = state.require_session(&headers).await?;
    let (_, issue) = state
        .workspace_service
        .create_issue(
            state.workspace.worktree.clone(),
            &state.workspace.exported_branch,
            CreateIssue {
                title: body.title,
                description: body.description,
                label_ids: body.label_ids,
                assignee_user_ids: body.assignee_user_ids,
                milestone_id: body.milestone_id,
                linked_pull_request_ids: body.linked_pull_request_ids,
            },
            issue_actor_input_from_session(&session, body.display_name),
        )
        .await
        .map_err(|error| domain_status(&error))?;
    Ok(Json(issue))
}

async fn update_issue(
    State(state): State<Arc<WebUiServer>>,
    headers: HeaderMap,
    AxumPath(id): AxumPath<String>,
    Json(body): Json<IssueUpdateRequest>,
) -> Result<Json<IssueRecord>, StatusCode> {
    let session = state.require_session(&headers).await?;
    let status = body.status.map(|status| match status {
        IssueStatusRequest::Open => IssueStatus::Open,
        IssueStatusRequest::Closed => IssueStatus::Closed,
    });
    let (_, issue) = state
        .workspace_service
        .update_issue(
            state.workspace.worktree.clone(),
            &state.workspace.exported_branch,
            &id,
            UpdateIssue {
                title: body.title,
                description: body.description,
                status,
            },
            issue_actor_input_from_session(&session, None),
        )
        .await
        .map_err(|error| domain_status(&error))?;
    Ok(Json(issue))
}

async fn delete_issue(
    State(state): State<Arc<WebUiServer>>,
    headers: HeaderMap,
    AxumPath(id): AxumPath<String>,
) -> Result<Json<IssueRecord>, StatusCode> {
    state.require_owner(&headers).await?;
    let (_, issue) = state
        .workspace_service
        .delete_issue(
            state.workspace.worktree.clone(),
            &state.workspace.exported_branch,
            &id,
        )
        .await
        .map_err(|error| domain_status(&error))?;
    Ok(Json(issue))
}

async fn comment_issue(
    State(state): State<Arc<WebUiServer>>,
    headers: HeaderMap,
    AxumPath(id): AxumPath<String>,
    Json(body): Json<IssueCommentRequest>,
) -> Result<Json<IssueRecord>, StatusCode> {
    let session = state.require_session(&headers).await?;
    let (_, issue) = state
        .workspace_service
        .comment_issue(
            state.workspace.worktree.clone(),
            &state.workspace.exported_branch,
            &id,
            CreateIssueComment {
                display_name: body.display_name.clone(),
                body: body.body,
            },
            issue_actor_input_from_session(&session, body.display_name),
        )
        .await
        .map_err(|error| domain_status(&error))?;
    Ok(Json(issue))
}

async fn react_issue(
    State(state): State<Arc<WebUiServer>>,
    headers: HeaderMap,
    AxumPath(id): AxumPath<String>,
    Json(body): Json<IssueReactionRequest>,
) -> Result<Json<IssueRecord>, StatusCode> {
    let session = state.require_session(&headers).await?;
    let (_, issue) = state
        .workspace_service
        .toggle_issue_reaction(
            state.workspace.worktree.clone(),
            &state.workspace.exported_branch,
            &id,
            body.content,
            issue_actor_input_from_session(&session, body.display_name),
        )
        .await
        .map_err(|error| domain_status(&error))?;
    Ok(Json(issue))
}

async fn react_issue_comment(
    State(state): State<Arc<WebUiServer>>,
    headers: HeaderMap,
    AxumPath((id, comment_id)): AxumPath<(String, String)>,
    Json(body): Json<IssueReactionRequest>,
) -> Result<Json<IssueRecord>, StatusCode> {
    let session = state.require_session(&headers).await?;
    let (_, issue) = state
        .workspace_service
        .toggle_issue_comment_reaction(
            state.workspace.worktree.clone(),
            &state.workspace.exported_branch,
            &id,
            &comment_id,
            body.content,
            issue_actor_input_from_session(&session, body.display_name),
        )
        .await
        .map_err(|error| domain_status(&error))?;
    Ok(Json(issue))
}

async fn set_issue_labels(
    State(state): State<Arc<WebUiServer>>,
    headers: HeaderMap,
    AxumPath(id): AxumPath<String>,
    Json(body): Json<IssueLabelsRequest>,
) -> Result<Json<IssueRecord>, StatusCode> {
    let session = state.require_session(&headers).await?;
    if session.actor != UiRole::Owner {
        return Err(StatusCode::FORBIDDEN);
    }
    let (_, issue) = state
        .workspace_service
        .set_issue_labels(
            state.workspace.worktree.clone(),
            &state.workspace.exported_branch,
            &id,
            body.label_ids,
            issue_actor_input_from_session(&session, None),
        )
        .await
        .map_err(|error| domain_status(&error))?;
    Ok(Json(issue))
}

async fn set_issue_assignees(
    State(state): State<Arc<WebUiServer>>,
    headers: HeaderMap,
    AxumPath(id): AxumPath<String>,
    Json(body): Json<IssueAssigneesRequest>,
) -> Result<Json<IssueRecord>, StatusCode> {
    let session = state.require_session(&headers).await?;
    if session.actor != UiRole::Owner {
        return Err(StatusCode::FORBIDDEN);
    }
    let (_, issue) = state
        .workspace_service
        .set_issue_assignees(
            state.workspace.worktree.clone(),
            &state.workspace.exported_branch,
            &id,
            body.assignee_user_ids,
            issue_actor_input_from_session(&session, None),
        )
        .await
        .map_err(|error| domain_status(&error))?;
    Ok(Json(issue))
}

async fn set_issue_milestone(
    State(state): State<Arc<WebUiServer>>,
    headers: HeaderMap,
    AxumPath(id): AxumPath<String>,
    Json(body): Json<IssueMilestoneRequest>,
) -> Result<Json<IssueRecord>, StatusCode> {
    let session = state.require_session(&headers).await?;
    if session.actor != UiRole::Owner {
        return Err(StatusCode::FORBIDDEN);
    }
    let (_, issue) = state
        .workspace_service
        .set_issue_milestone(
            state.workspace.worktree.clone(),
            &state.workspace.exported_branch,
            &id,
            body.milestone_id,
            issue_actor_input_from_session(&session, None),
        )
        .await
        .map_err(|error| domain_status(&error))?;
    Ok(Json(issue))
}

async fn link_issue_pull_request(
    State(state): State<Arc<WebUiServer>>,
    headers: HeaderMap,
    AxumPath(id): AxumPath<String>,
    Json(body): Json<IssueLinkPullRequestRequest>,
) -> Result<Json<IssueRecord>, StatusCode> {
    let session = state.require_session(&headers).await?;
    if session.actor != UiRole::Owner {
        return Err(StatusCode::FORBIDDEN);
    }
    let (_, issue) = state
        .workspace_service
        .link_issue_pull_request(
            state.workspace.worktree.clone(),
            &state.workspace.exported_branch,
            &id,
            &body.pull_request_id,
            issue_actor_input_from_session(&session, None),
        )
        .await
        .map_err(|error| domain_status(&error))?;
    Ok(Json(issue))
}

async fn unlink_issue_pull_request(
    State(state): State<Arc<WebUiServer>>,
    headers: HeaderMap,
    AxumPath((id, pull_request_id)): AxumPath<(String, String)>,
) -> Result<Json<IssueRecord>, StatusCode> {
    let session = state.require_session(&headers).await?;
    if session.actor != UiRole::Owner {
        return Err(StatusCode::FORBIDDEN);
    }
    let (_, issue) = state
        .workspace_service
        .unlink_issue_pull_request(
            state.workspace.worktree.clone(),
            &state.workspace.exported_branch,
            &id,
            &pull_request_id,
            issue_actor_input_from_session(&session, None),
        )
        .await
        .map_err(|error| domain_status(&error))?;
    Ok(Json(issue))
}

async fn upsert_issue_label(
    State(state): State<Arc<WebUiServer>>,
    headers: HeaderMap,
    Json(body): Json<IssueLabelUpsertRequest>,
) -> Result<Json<IssueLabel>, StatusCode> {
    state.require_owner(&headers).await?;
    let (_, label) = state
        .workspace_service
        .upsert_issue_label(
            state.workspace.worktree.clone(),
            &state.workspace.exported_branch,
            UpsertIssueLabel {
                id: body.id,
                name: body.name,
                color: body.color,
                description: body.description,
            },
        )
        .map_err(|error| domain_status(&error))?;
    Ok(Json(label))
}

async fn delete_issue_label(
    State(state): State<Arc<WebUiServer>>,
    headers: HeaderMap,
    AxumPath(id): AxumPath<String>,
) -> Result<Json<IssueLabel>, StatusCode> {
    let session = state.require_session(&headers).await?;
    if session.actor != UiRole::Owner {
        return Err(StatusCode::FORBIDDEN);
    }
    let (_, label) = state
        .workspace_service
        .delete_issue_label(
            state.workspace.worktree.clone(),
            &state.workspace.exported_branch,
            &id,
            issue_actor_input_from_session(&session, None),
        )
        .map_err(|error| domain_status(&error))?;
    Ok(Json(label))
}

async fn upsert_issue_milestone(
    State(state): State<Arc<WebUiServer>>,
    headers: HeaderMap,
    Json(body): Json<IssueMilestoneUpsertRequest>,
) -> Result<Json<IssueMilestone>, StatusCode> {
    state.require_owner(&headers).await?;
    let (_, milestone) = state
        .workspace_service
        .upsert_issue_milestone(
            state.workspace.worktree.clone(),
            &state.workspace.exported_branch,
            UpsertIssueMilestone {
                id: body.id,
                title: body.title,
                description: body.description,
            },
        )
        .map_err(|error| domain_status(&error))?;
    Ok(Json(milestone))
}

async fn delete_issue_milestone(
    State(state): State<Arc<WebUiServer>>,
    headers: HeaderMap,
    AxumPath(id): AxumPath<String>,
) -> Result<Json<IssueMilestone>, StatusCode> {
    let session = state.require_session(&headers).await?;
    if session.actor != UiRole::Owner {
        return Err(StatusCode::FORBIDDEN);
    }
    let (_, milestone) = state
        .workspace_service
        .delete_issue_milestone(
            state.workspace.worktree.clone(),
            &state.workspace.exported_branch,
            &id,
            issue_actor_input_from_session(&session, None),
        )
        .map_err(|error| domain_status(&error))?;
    Ok(Json(milestone))
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use axum::extract::ConnectInfo;
    use axum::http::Request;
    use http_body_util::BodyExt;
    use qit_domain::{
        ApplyOutcome, BranchRecord, CommitDetail, CommitSummary, RefComparison, RegistryError,
        RegistryStore, RepoReadStore, RepoStore, RepositoryError, TreeEntry, TreeEntryKind,
        WorkspaceId, WorkspaceRecord,
    };
    use qit_http::{GitHttpServer, GitHttpServerConfig};
    use qit_http_backend::{
        BoxAsyncRead, GitHttpBackend, GitHttpBackendError, GitHttpBackendRequest,
        GitHttpBackendResponse,
    };
    use std::net::SocketAddr;
    use std::path::Path;
    use std::sync::{Arc, Mutex};
    use tower::ServiceExt;

    #[derive(Default)]
    struct StubRepoStore;

    #[async_trait]
    impl RepoStore for StubRepoStore {
        async fn ensure_initialized(
            &self,
            _workspace: &WorkspaceSpec,
        ) -> Result<(), RepositoryError> {
            Ok(())
        }

        async fn snapshot(
            &self,
            _workspace: &WorkspaceSpec,
            _message: &str,
        ) -> Result<Option<String>, RepositoryError> {
            Ok(Some("snapshot".into()))
        }

        async fn apply_fast_forward(
            &self,
            workspace: &WorkspaceSpec,
            _source_ref: &str,
        ) -> Result<ApplyOutcome, RepositoryError> {
            Ok(ApplyOutcome {
                merged_to: workspace.checked_out_branch.clone(),
                commit: "applied".into(),
            })
        }

        async fn list_branches(
            &self,
            workspace: &WorkspaceSpec,
        ) -> Result<Vec<BranchRecord>, RepositoryError> {
            Ok(vec![
                BranchRecord {
                    name: workspace.exported_branch.clone(),
                    commit: "111111111111".into(),
                    summary: "main".into(),
                },
                BranchRecord {
                    name: "feature".into(),
                    commit: "222222222222".into(),
                    summary: "feature".into(),
                },
            ])
        }

        async fn create_branch(
            &self,
            _workspace: &WorkspaceSpec,
            _name: &str,
            _start_point: Option<&str>,
            _force: bool,
        ) -> Result<String, RepositoryError> {
            Ok("created".into())
        }

        async fn rename_branch(
            &self,
            _workspace: &WorkspaceSpec,
            _old_name: &str,
            _new_name: &str,
            _force: bool,
        ) -> Result<(), RepositoryError> {
            Ok(())
        }

        async fn delete_branch(
            &self,
            _workspace: &WorkspaceSpec,
            _name: &str,
            _force: bool,
        ) -> Result<(), RepositoryError> {
            Ok(())
        }

        async fn switch_branch(
            &self,
            _workspace: &WorkspaceSpec,
            _name: &str,
        ) -> Result<String, RepositoryError> {
            Ok("switched".into())
        }

        async fn checkout_branch(
            &self,
            _workspace: &WorkspaceSpec,
            _name: &str,
            _force: bool,
        ) -> Result<String, RepositoryError> {
            Ok("checked-out".into())
        }

        async fn merge_branch(
            &self,
            _workspace: &WorkspaceSpec,
            _source_branch: &str,
            _target_branch: &str,
        ) -> Result<String, RepositoryError> {
            Ok("merged-sha".into())
        }
    }

    #[async_trait]
    impl RepoReadStore for StubRepoStore {
        async fn list_commits(
            &self,
            _workspace: &WorkspaceSpec,
            reference: Option<&str>,
            offset: usize,
            _limit: usize,
        ) -> Result<CommitHistory, RepositoryError> {
            let (reference, commits) = match reference.unwrap_or("main") {
                "feature" => (
                    "feature",
                    vec![
                        qit_domain::CommitHistoryNode {
                            id: "feature-current".into(),
                            summary: "feature current".into(),
                            author: "qit".into(),
                            authored_at: 25,
                            parents: vec!["222222222222".into()],
                            refs: vec![],
                        },
                        qit_domain::CommitHistoryNode {
                            id: "222222222222".into(),
                            summary: "feature at pr open".into(),
                            author: "qit".into(),
                            authored_at: 12,
                            parents: vec!["111111111111".into()],
                            refs: vec![],
                        },
                    ],
                ),
                _ => (
                    "main",
                    vec![
                        qit_domain::CommitHistoryNode {
                            id: "main-current".into(),
                            summary: "main current".into(),
                            author: "qit".into(),
                            authored_at: 30,
                            parents: vec!["111111111111".into()],
                            refs: vec![],
                        },
                        qit_domain::CommitHistoryNode {
                            id: "111111111111".into(),
                            summary: "main at pr open".into(),
                            author: "qit".into(),
                            authored_at: 10,
                            parents: vec![],
                            refs: vec![],
                        },
                    ],
                ),
            };
            Ok(CommitHistory {
                reference: reference.into(),
                offset,
                limit: 40,
                has_more: false,
                commits,
            })
        }

        async fn read_commit(
            &self,
            _workspace: &WorkspaceSpec,
            _commitish: &str,
        ) -> Result<CommitDetail, RepositoryError> {
            Ok(CommitDetail {
                id: "abc123".into(),
                summary: "initial".into(),
                message: "initial".into(),
                author: "qit".into(),
                authored_at: 1,
                parents: vec![],
                changes: vec![],
            })
        }

        async fn list_tree(
            &self,
            _workspace: &WorkspaceSpec,
            _reference: &str,
            _path: Option<&Path>,
        ) -> Result<Vec<TreeEntry>, RepositoryError> {
            Ok(vec![TreeEntry {
                name: "README.md".into(),
                path: "README.md".into(),
                oid: "blob".into(),
                kind: TreeEntryKind::Blob,
                size: Some(5),
            }])
        }

        async fn read_blob(
            &self,
            _workspace: &WorkspaceSpec,
            _reference: &str,
            path: &Path,
        ) -> Result<BlobContent, RepositoryError> {
            Ok(BlobContent {
                path: path.display().to_string(),
                text: Some("hello".into()),
                is_binary: false,
                size: 5,
            })
        }

        async fn compare_refs(
            &self,
            _workspace: &WorkspaceSpec,
            base_ref: &str,
            head_ref: &str,
            _limit: usize,
        ) -> Result<RefComparison, RepositoryError> {
            Ok(RefComparison {
                base_ref: base_ref.into(),
                head_ref: head_ref.into(),
                merge_base: Some("base".into()),
                ahead_by: 1,
                behind_by: 0,
                commits: vec![CommitSummary {
                    id: "cmp123".into(),
                    summary: "feature change".into(),
                    author: "qit".into(),
                    authored_at: 2,
                }],
            })
        }

        async fn diff_refs(
            &self,
            _workspace: &WorkspaceSpec,
            _base_ref: &str,
            _head_ref: &str,
        ) -> Result<Vec<RefDiffFile>, RepositoryError> {
            Ok(vec![RefDiffFile {
                path: "README.md".into(),
                previous_path: None,
                status: "modified".into(),
                additions: 1,
                deletions: 1,
                original: Some(BlobContent {
                    path: "README.md".into(),
                    text: Some("hello\n".into()),
                    is_binary: false,
                    size: 6,
                }),
                modified: Some(BlobContent {
                    path: "README.md".into(),
                    text: Some("hello world\n".into()),
                    is_binary: false,
                    size: 12,
                }),
            }])
        }
    }

    struct StubRegistry {
        workspace: WorkspaceSpec,
        record: Mutex<WorkspaceRecord>,
    }

    impl RegistryStore for StubRegistry {
        fn canonical_worktree(&self, _worktree: &Path) -> Result<PathBuf, RegistryError> {
            Ok(self.workspace.worktree.clone())
        }

        fn default_sidecar_path(&self, _id: WorkspaceId) -> Result<PathBuf, RegistryError> {
            Ok(self.workspace.sidecar.clone())
        }

        fn load(&self, _id: WorkspaceId) -> Result<Option<WorkspaceRecord>, RegistryError> {
            Ok(Some(self.record.lock().unwrap().clone()))
        }

        fn save(&self, _id: WorkspaceId, record: WorkspaceRecord) -> Result<(), RegistryError> {
            *self.record.lock().unwrap() = record;
            Ok(())
        }
    }

    struct StubGitBackend;

    #[async_trait]
    impl GitHttpBackend for StubGitBackend {
        async fn serve(
            &self,
            _sidecar: &Path,
            _request: GitHttpBackendRequest,
            _body: BoxAsyncRead,
        ) -> Result<GitHttpBackendResponse, GitHttpBackendError> {
            Ok(GitHttpBackendResponse {
                status: 200,
                headers: vec![("Content-Type".into(), "text/plain".into())],
                body_prefix: b"git".to_vec(),
                stdout: None,
                completion: None,
            })
        }
    }

    fn test_workspace() -> WorkspaceSpec {
        let root = std::env::temp_dir().join(format!("qit-webui-test-{}", uuid::Uuid::new_v4()));
        let worktree = root.join("host");
        let sidecar = root.join("sidecar.git");
        std::fs::create_dir_all(&worktree).unwrap();
        std::fs::create_dir_all(&sidecar).unwrap();
        WorkspaceSpec {
            id: WorkspaceId(uuid::Uuid::new_v4()),
            worktree,
            sidecar,
            exported_branch: "main".into(),
            checked_out_branch: "main".into(),
        }
    }

    fn app_with_web_ui(
        implicit_owner_mode: bool,
        secure_cookies: bool,
        web_ui: WorkspaceWebUiState,
    ) -> Router {
        let repo_store = Arc::new(StubRepoStore);
        let workspace = test_workspace();
        let registry = Arc::new(StubRegistry {
            workspace: workspace.clone(),
            record: Mutex::new(WorkspaceRecord {
                worktree: workspace.worktree.clone(),
                sidecar: workspace.sidecar.clone(),
                exported_branch: workspace.exported_branch.clone(),
                checked_out_branch: Some(workspace.checked_out_branch.clone()),
                web_ui,
            }),
        });
        let service = Arc::new(WorkspaceService::new(
            repo_store.clone(),
            registry.clone(),
            Arc::new(TestIssuer),
        ));
        let web = WebUiServer::new(
            repo_store.clone(),
            service.clone(),
            WebUiConfig {
                workspace: workspace.clone(),
                repo_mount_path: "/repo".into(),
                credentials: SessionCredentials {
                    username: "tester".into(),
                    password: "secret".into(),
                },
                implicit_owner_mode,
                secure_cookies,
                public_repo_url: None,
            },
        )
        .router();
        let git = GitHttpServer::new(
            Arc::new(StubGitBackend),
            registry,
            service,
            GitHttpServerConfig {
                workspace,
                credentials: SessionCredentials {
                    username: "tester".into(),
                    password: "secret".into(),
                },
                auto_apply: false,
                repo_mount_path: "/repo".into(),
                request_scheme: "http".into(),
                max_body_bytes: 1024 * 1024,
            },
        )
        .git_router();
        web.merge(git)
    }

    fn basic_auth_state() -> qit_domain::RepoAuthState {
        qit_domain::RepoAuthState {
            mode: qit_domain::AuthMode::SharedSession,
            methods: vec![qit_domain::AuthMethod::BasicAuth],
            ..Default::default()
        }
    }

    fn app(implicit_owner_mode: bool, secure_cookies: bool) -> Router {
        app_with_web_ui(
            implicit_owner_mode,
            secure_cookies,
            WorkspaceWebUiState {
                auth: basic_auth_state(),
                ..Default::default()
            },
        )
    }

    struct TestIssuer;

    impl qit_domain::CredentialIssuer for TestIssuer {
        fn issue(&self) -> SessionCredentials {
            SessionCredentials {
                username: "tester".into(),
                password: "secret".into(),
            }
        }
    }

    fn request_with_remote(uri: &str, remote: SocketAddr, host: &str) -> Request<Body> {
        let mut request = Request::builder()
            .uri(uri)
            .header(HOST, host)
            .body(Body::empty())
            .unwrap();
        request.extensions_mut().insert(ConnectInfo(remote));
        request
    }

    async fn login_cookie(app: &Router, remote: SocketAddr, host: &str) -> String {
        let login = Request::builder()
            .method("POST")
            .uri("/repo/api/session/login")
            .header(HOST, host)
            .header("content-type", "application/json")
            .body(Body::from(r#"{"username":"tester","password":"secret"}"#))
            .unwrap();
        let mut login = login;
        login.extensions_mut().insert(ConnectInfo(remote));
        let login_response = app.clone().oneshot(login).await.unwrap();
        assert_eq!(login_response.status(), StatusCode::OK);
        login_response
            .headers()
            .get(SET_COOKIE)
            .unwrap()
            .to_str()
            .unwrap()
            .split(';')
            .next()
            .unwrap()
            .to_string()
    }

    #[tokio::test]
    async fn local_only_mode_bootstrap_is_owner_and_git_stays_authenticated() {
        let app = app(true, false);
        let localhost = SocketAddr::from(([127, 0, 0, 1], 3000));
        let response = app
            .clone()
            .oneshot(request_with_remote(
                "/repo/api/bootstrap",
                localhost,
                "127.0.0.1:8080",
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = response.into_body().collect().await.unwrap().to_bytes();
        let payload: BootstrapResponse = serde_json::from_slice(&body).unwrap();
        assert_eq!(payload.actor, Some(UiRole::Owner));
        assert!(payload.local_only_owner_mode);
        assert!(payload.git_credentials_visible);
        assert_eq!(payload.git_username.as_deref(), Some("tester"));
        assert_eq!(payload.git_password.as_deref(), Some("secret"));

        let git_response = app
            .oneshot(request_with_remote(
                "/repo/info/refs?service=git-upload-pack",
                SocketAddr::from(([10, 0, 0, 2], 3000)),
                "example.ngrok.app",
            ))
            .await
            .unwrap();
        assert_eq!(git_response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn public_mode_requires_login_and_logout_revokes_session() {
        let app = app(false, true);
        let remote = SocketAddr::from(([10, 0, 0, 2], 3000));
        let bootstrap = app
            .clone()
            .oneshot(request_with_remote(
                "/repo/api/bootstrap",
                remote,
                "demo.ngrok.app",
            ))
            .await
            .unwrap();
        let payload: BootstrapResponse =
            serde_json::from_slice(&bootstrap.into_body().collect().await.unwrap().to_bytes())
                .unwrap();
        assert_eq!(payload.actor, None);
        assert!(!payload.local_only_owner_mode);

        let login = Request::builder()
            .method("POST")
            .uri("/repo/api/session/login")
            .header(HOST, "demo.ngrok.app")
            .header("content-type", "application/json")
            .body(Body::from(r#"{"username":"tester","password":"secret"}"#))
            .unwrap();
        let mut login = login;
        login.extensions_mut().insert(ConnectInfo(remote));
        let login_response = app.clone().oneshot(login).await.unwrap();
        assert_eq!(login_response.status(), StatusCode::OK);
        assert!(login_response
            .headers()
            .get(SET_COOKIE)
            .unwrap()
            .to_str()
            .unwrap()
            .contains("Secure"));
        let cookie = login_response
            .headers()
            .get(SET_COOKIE)
            .unwrap()
            .to_str()
            .unwrap()
            .split(';')
            .next()
            .unwrap()
            .to_string();

        let authenticated_bootstrap = Request::builder()
            .uri("/repo/api/bootstrap")
            .header(HOST, "demo.ngrok.app")
            .header("cookie", cookie.clone())
            .body(Body::empty())
            .unwrap();
        let mut authenticated_bootstrap = authenticated_bootstrap;
        authenticated_bootstrap
            .extensions_mut()
            .insert(ConnectInfo(remote));
        let authenticated_bootstrap_response =
            app.clone().oneshot(authenticated_bootstrap).await.unwrap();
        assert_eq!(authenticated_bootstrap_response.status(), StatusCode::OK);
        let authenticated_bootstrap: BootstrapResponse = serde_json::from_slice(
            &authenticated_bootstrap_response
                .into_body()
                .collect()
                .await
                .unwrap()
                .to_bytes(),
        )
        .unwrap();
        assert_eq!(authenticated_bootstrap.actor, Some(UiRole::User));
        assert!(!authenticated_bootstrap.git_credentials_visible);
        assert_eq!(authenticated_bootstrap.git_username, None);
        assert_eq!(authenticated_bootstrap.git_password, None);

        let create_pr = Request::builder()
            .method("POST")
            .uri("/repo/api/pull-requests")
            .header(HOST, "demo.ngrok.app")
            .header("content-type", "application/json")
            .header("cookie", cookie.clone())
            .body(Body::from(
                r#"{"title":"Feature PR","description":"compare branches","source_branch":"feature","target_branch":"main"}"#,
            ))
            .unwrap();
        let mut create_pr = create_pr;
        create_pr.extensions_mut().insert(ConnectInfo(remote));
        let create_response = app.clone().oneshot(create_pr).await.unwrap();
        assert_eq!(create_response.status(), StatusCode::OK);
        let created: PullRequestRecord = serde_json::from_slice(
            &create_response
                .into_body()
                .collect()
                .await
                .unwrap()
                .to_bytes(),
        )
        .unwrap();
        assert_eq!(created.status, qit_domain::PullRequestStatus::Open);
        assert_eq!(created.target_commit.as_deref(), Some("111111111111"));
        assert_eq!(created.source_commit.as_deref(), Some("222222222222"));

        let detail = Request::builder()
            .uri(format!("/repo/api/pull-requests/{}", created.id))
            .header(HOST, "demo.ngrok.app")
            .header("cookie", cookie.clone())
            .body(Body::empty())
            .unwrap();
        let mut detail = detail;
        detail.extensions_mut().insert(ConnectInfo(remote));
        let detail_response = app.clone().oneshot(detail).await.unwrap();
        assert_eq!(detail_response.status(), StatusCode::OK);
        let detail: serde_json::Value = serde_json::from_slice(
            &detail_response
                .into_body()
                .collect()
                .await
                .unwrap()
                .to_bytes(),
        )
        .unwrap();
        assert_eq!(detail["pull_request"]["id"], created.id);
        assert!(detail["comparison"].is_object());
        assert!(detail["diffs"].is_array());
        assert_eq!(detail["comparison"]["base_ref"], "111111111111");
        assert_eq!(detail["comparison"]["head_ref"], "222222222222");

        let merge = Request::builder()
            .method("POST")
            .uri(format!("/repo/api/pull-requests/{}/merge", created.id))
            .header(HOST, "demo.ngrok.app")
            .header("cookie", cookie.clone())
            .body(Body::empty())
            .unwrap();
        let mut merge = merge;
        merge.extensions_mut().insert(ConnectInfo(remote));
        let merge_response = app.clone().oneshot(merge).await.unwrap();
        assert_eq!(merge_response.status(), StatusCode::FORBIDDEN);

        let logout = Request::builder()
            .method("POST")
            .uri("/repo/api/session/logout")
            .header(HOST, "demo.ngrok.app")
            .header("cookie", cookie.clone())
            .body(Body::empty())
            .unwrap();
        let mut logout = logout;
        logout.extensions_mut().insert(ConnectInfo(remote));
        let logout_response = app.clone().oneshot(logout).await.unwrap();
        assert_eq!(logout_response.status(), StatusCode::NO_CONTENT);

        let list_after_logout = Request::builder()
            .uri("/repo/api/pull-requests")
            .header(HOST, "demo.ngrok.app")
            .header("cookie", cookie)
            .body(Body::empty())
            .unwrap();
        let mut list_after_logout = list_after_logout;
        list_after_logout
            .extensions_mut()
            .insert(ConnectInfo(remote));
        let after_logout_response = app.oneshot(list_after_logout).await.unwrap();
        assert_eq!(after_logout_response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn request_based_mode_supports_onboarding_and_pat_git_auth() {
        let app = app_with_web_ui(
            true,
            false,
            WorkspaceWebUiState {
                pull_requests: Vec::new(),
                repository: RepositorySettings::default(),
                issue_settings: Default::default(),
                issues: Vec::new(),
                auth: qit_domain::RepoAuthState {
                    mode: qit_domain::AuthMode::RequestBased,
                    ..Default::default()
                },
            },
        );
        let remote = SocketAddr::from(([10, 0, 0, 2], 3000));
        let localhost = SocketAddr::from(([127, 0, 0, 1], 3000));

        let request = Request::builder()
            .method("POST")
            .uri("/repo/api/access-requests")
            .header(HOST, "demo.ngrok.app")
            .header("content-type", "application/json")
            .body(Body::from(
                r#"{"name":"Alice","email":"alice@example.com"}"#,
            ))
            .unwrap();
        let mut request = request;
        request.extensions_mut().insert(ConnectInfo(remote));
        let request_response = app.clone().oneshot(request).await.unwrap();
        assert_eq!(request_response.status(), StatusCode::OK);
        let request: qit_domain::SubmittedAccessRequest = serde_json::from_slice(
            &request_response
                .into_body()
                .collect()
                .await
                .unwrap()
                .to_bytes(),
        )
        .unwrap();
        assert!(request.secret.starts_with("qit_request."));

        let status = Request::builder()
            .method("POST")
            .uri("/repo/api/access-requests/status")
            .header(HOST, "demo.ngrok.app")
            .header("content-type", "application/json")
            .body(Body::from(format!(r#"{{"token":"{}"}}"#, request.secret)))
            .unwrap();
        let mut status = status;
        status.extensions_mut().insert(ConnectInfo(remote));
        let status_response = app.clone().oneshot(status).await.unwrap();
        assert_eq!(status_response.status(), StatusCode::OK);
        let status: qit_domain::AccessRequestProgress = serde_json::from_slice(
            &status_response
                .into_body()
                .collect()
                .await
                .unwrap()
                .to_bytes(),
        )
        .unwrap();
        assert_eq!(status.status, qit_domain::AccessRequestStatus::Pending);

        let approve = Request::builder()
            .method("POST")
            .uri(format!(
                "/repo/api/access-requests/{}/approve",
                request.request.id
            ))
            .header(HOST, "127.0.0.1:8080")
            .body(Body::empty())
            .unwrap();
        let mut approve = approve;
        approve.extensions_mut().insert(ConnectInfo(localhost));
        let approve_response = app.clone().oneshot(approve).await.unwrap();
        assert_eq!(approve_response.status(), StatusCode::OK);
        let onboarding: qit_domain::IssuedOnboarding = serde_json::from_slice(
            &approve_response
                .into_body()
                .collect()
                .await
                .unwrap()
                .to_bytes(),
        )
        .unwrap();
        assert!(onboarding.secret.is_none());

        let complete = Request::builder()
            .method("POST")
            .uri("/repo/api/onboarding/complete")
            .header(HOST, "demo.ngrok.app")
            .header("content-type", "application/json")
            .body(Body::from(format!(
                r#"{{"token":"{}","username":"alice","password":"very-secret-pass"}}"#,
                request.secret
            )))
            .unwrap();
        let mut complete = complete;
        complete.extensions_mut().insert(ConnectInfo(remote));
        let complete_response = app.clone().oneshot(complete).await.unwrap();
        assert_eq!(complete_response.status(), StatusCode::OK);
        let cookie = complete_response
            .headers()
            .get(SET_COOKIE)
            .unwrap()
            .to_str()
            .unwrap()
            .split(';')
            .next()
            .unwrap()
            .to_string();

        let settings = Request::builder()
            .uri("/repo/api/settings")
            .header(HOST, "demo.ngrok.app")
            .header("cookie", cookie.clone())
            .body(Body::empty())
            .unwrap();
        let mut settings = settings;
        settings.extensions_mut().insert(ConnectInfo(remote));
        let settings_response = app.clone().oneshot(settings).await.unwrap();
        assert_eq!(settings_response.status(), StatusCode::OK);
        let settings: serde_json::Value = serde_json::from_slice(
            &settings_response
                .into_body()
                .collect()
                .await
                .unwrap()
                .to_bytes(),
        )
        .unwrap();
        assert_eq!(settings["current_user"]["username"], "alice");

        let create_pat = Request::builder()
            .method("POST")
            .uri("/repo/api/profile/pats")
            .header(HOST, "demo.ngrok.app")
            .header("content-type", "application/json")
            .header("cookie", cookie)
            .body(Body::from(r#"{"label":"laptop"}"#))
            .unwrap();
        let mut create_pat = create_pat;
        create_pat.extensions_mut().insert(ConnectInfo(remote));
        let create_pat_response = app.clone().oneshot(create_pat).await.unwrap();
        assert_eq!(create_pat_response.status(), StatusCode::OK);
        let pat: qit_domain::IssuedPat = serde_json::from_slice(
            &create_pat_response
                .into_body()
                .collect()
                .await
                .unwrap()
                .to_bytes(),
        )
        .unwrap();
        assert!(pat.secret.starts_with("qit_pat."));

        let legacy = Request::builder()
            .uri("/repo/info/refs?service=git-upload-pack")
            .header(HOST, "demo.ngrok.app")
            .header(
                axum::http::header::AUTHORIZATION,
                HeaderValue::from_str(&format!(
                    "Basic {}",
                    BASE64_STANDARD.encode("tester:secret")
                ))
                .unwrap(),
            )
            .body(Body::empty())
            .unwrap();
        let mut legacy = legacy;
        legacy.extensions_mut().insert(ConnectInfo(remote));
        let legacy_response = app.clone().oneshot(legacy).await.unwrap();
        assert_eq!(legacy_response.status(), StatusCode::UNAUTHORIZED);

        let pat_request = Request::builder()
            .uri("/repo/info/refs?service=git-upload-pack")
            .header(HOST, "demo.ngrok.app")
            .header(
                axum::http::header::AUTHORIZATION,
                HeaderValue::from_str(&format!(
                    "Basic {}",
                    BASE64_STANDARD.encode(format!("alice:{}", pat.secret))
                ))
                .unwrap(),
            )
            .body(Body::empty())
            .unwrap();
        let mut pat_request = pat_request;
        pat_request.extensions_mut().insert(ConnectInfo(remote));
        let pat_response = app.oneshot(pat_request).await.unwrap();
        assert_eq!(pat_response.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn web_ui_serves_brand_assets() {
        let app = app(true, false);
        let remote = SocketAddr::from(([127, 0, 0, 1], 3000));
        let response = app
            .clone()
            .oneshot(request_with_remote(
                "/repo/assets/qit-logo-on-dark.png",
                remote,
                "127.0.0.1:8080",
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response.headers().get(CONTENT_TYPE).unwrap(),
            HeaderValue::from_static("image/png")
        );
        let body = response.into_body().collect().await.unwrap().to_bytes();
        assert!(!body.is_empty());

        let chunk_response = app
            .clone()
            .oneshot(request_with_remote(
                "/repo/assets/chunk-vendor-ui.js",
                remote,
                "127.0.0.1:8080",
            ))
            .await
            .unwrap();
        assert_eq!(chunk_response.status(), StatusCode::OK);
        assert_eq!(
            chunk_response.headers().get(CONTENT_TYPE).unwrap(),
            HeaderValue::from_static("text/javascript; charset=utf-8")
        );
        let chunk_body = chunk_response
            .into_body()
            .collect()
            .await
            .unwrap()
            .to_bytes();
        assert!(!chunk_body.is_empty());

        let og_response = app
            .oneshot(request_with_remote(
                "/repo/assets/qit-og.svg",
                remote,
                "127.0.0.1:8080",
            ))
            .await
            .unwrap();
        assert_eq!(og_response.status(), StatusCode::OK);
        assert_eq!(
            og_response.headers().get(CONTENT_TYPE).unwrap(),
            HeaderValue::from_static("image/svg+xml; charset=utf-8")
        );
        let og_body = String::from_utf8(
            og_response
                .into_body()
                .collect()
                .await
                .unwrap()
                .to_bytes()
                .to_vec(),
        )
        .unwrap();
        assert!(og_body.contains("host"));
        assert!(og_body.contains("Hosted on Qit"));
    }

    #[tokio::test]
    async fn index_html_includes_favicon_and_social_image() {
        let app = app(true, false);
        let remote = SocketAddr::from(([127, 0, 0, 1], 3000));
        let response = app
            .oneshot(request_with_remote("/repo", remote, "127.0.0.1:8080"))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = String::from_utf8(
            response
                .into_body()
                .collect()
                .await
                .unwrap()
                .to_bytes()
                .to_vec(),
        )
        .unwrap();
        assert!(body.contains(
            r#"<link rel="icon" type="image/png" href="/repo/assets/qit-logo-on-dark.png" />"#
        ));
        assert!(body.contains(r#"<meta property="og:image" content="/repo/assets/qit-og.svg" />"#));
        assert!(body.contains(r#"<meta property="og:image:type" content="image/svg+xml" />"#));
        assert!(body.contains(r#"<meta property="og:image:width" content="1200" />"#));
        assert!(body.contains(r#"<meta property="og:image:height" content="630" />"#));
        assert!(body.contains(r#"<meta name="twitter:card" content="summary_large_image" />"#));
        assert!(body.contains(r#"<meta name="twitter:image" content="/repo/assets/qit-og.svg" />"#));
    }

    #[tokio::test]
    async fn legacy_pull_request_detail_backfills_refs_from_branch_history() {
        let app = app_with_web_ui(
            false,
            true,
            WorkspaceWebUiState {
                pull_requests: vec![PullRequestRecord {
                    id: "legacy-pr".into(),
                    title: "Legacy PR".into(),
                    description: "created before commit snapshots".into(),
                    source_branch: "feature".into(),
                    target_branch: "main".into(),
                    source_commit: None,
                    target_commit: None,
                    status: qit_domain::PullRequestStatus::Merged,
                    author_role: UiRole::Owner,
                    created_at_ms: 15_000,
                    updated_at_ms: 20_000,
                    merged_commit: Some("feature-current".into()),
                    activities: Vec::new(),
                }],
                repository: RepositorySettings::default(),
                issue_settings: Default::default(),
                issues: Vec::new(),
                auth: basic_auth_state(),
            },
        );
        let remote = SocketAddr::from(([10, 0, 0, 2], 3000));

        let login = Request::builder()
            .method("POST")
            .uri("/repo/api/session/login")
            .header(HOST, "demo.ngrok.app")
            .header("content-type", "application/json")
            .body(Body::from(r#"{"username":"tester","password":"secret"}"#))
            .unwrap();
        let mut login = login;
        login.extensions_mut().insert(ConnectInfo(remote));
        let login_response = app.clone().oneshot(login).await.unwrap();
        assert_eq!(login_response.status(), StatusCode::OK);
        let cookie = login_response
            .headers()
            .get(SET_COOKIE)
            .unwrap()
            .to_str()
            .unwrap()
            .split(';')
            .next()
            .unwrap()
            .to_string();

        let detail = Request::builder()
            .uri("/repo/api/pull-requests/legacy-pr")
            .header(HOST, "demo.ngrok.app")
            .header("cookie", cookie)
            .body(Body::empty())
            .unwrap();
        let mut detail = detail;
        detail.extensions_mut().insert(ConnectInfo(remote));
        let detail_response = app.oneshot(detail).await.unwrap();
        assert_eq!(detail_response.status(), StatusCode::OK);
        let detail: serde_json::Value = serde_json::from_slice(
            &detail_response
                .into_body()
                .collect()
                .await
                .unwrap()
                .to_bytes(),
        )
        .unwrap();
        assert_eq!(detail["comparison"]["base_ref"], "111111111111");
        assert_eq!(detail["comparison"]["head_ref"], "222222222222");
    }

    #[tokio::test]
    async fn owner_settings_endpoints_round_trip_metadata_and_branch_rules() {
        let app = app_with_web_ui(
            true,
            false,
            WorkspaceWebUiState {
                pull_requests: Vec::new(),
                repository: RepositorySettings::default(),
                issue_settings: Default::default(),
                issues: Vec::new(),
                auth: basic_auth_state(),
            },
        );
        let localhost = SocketAddr::from(([127, 0, 0, 1], 3000));

        let settings = app
            .clone()
            .oneshot(request_with_remote(
                "/repo/api/settings",
                localhost,
                "127.0.0.1:8080",
            ))
            .await
            .unwrap();
        assert_eq!(settings.status(), StatusCode::OK);
        let settings: serde_json::Value =
            serde_json::from_slice(&settings.into_body().collect().await.unwrap().to_bytes())
                .unwrap();
        assert_eq!(settings["repository"]["description"], "");

        let update = Request::builder()
            .method("PATCH")
            .uri("/repo/api/settings")
            .header(HOST, "127.0.0.1:8080")
            .header("content-type", "application/json")
            .body(Body::from(
                r#"{"description":"Demo repo","homepage_url":"https://example.com"}"#,
            ))
            .unwrap();
        let mut update = update;
        update.extensions_mut().insert(ConnectInfo(localhost));
        let updated = app.clone().oneshot(update).await.unwrap();
        assert_eq!(updated.status(), StatusCode::OK);
        let updated: serde_json::Value =
            serde_json::from_slice(&updated.into_body().collect().await.unwrap().to_bytes())
                .unwrap();
        assert_eq!(updated["repository"]["description"], "Demo repo");
        assert_eq!(updated["repository"]["homepage_url"], "https://example.com");

        let add_rule = Request::builder()
            .method("PUT")
            .uri("/repo/api/settings/branch-rules")
            .header(HOST, "127.0.0.1:8080")
            .header("content-type", "application/json")
            .body(Body::from(
                r#"{"pattern":"main","require_pull_request":true,"required_approvals":1,"dismiss_stale_approvals":true,"block_force_push":true,"block_delete":true}"#,
            ))
            .unwrap();
        let mut add_rule = add_rule;
        add_rule.extensions_mut().insert(ConnectInfo(localhost));
        let added = app.clone().oneshot(add_rule).await.unwrap();
        assert_eq!(added.status(), StatusCode::OK);
        let added: serde_json::Value =
            serde_json::from_slice(&added.into_body().collect().await.unwrap().to_bytes()).unwrap();
        assert_eq!(added["repository"]["branch_rules"][0]["pattern"], "main");
        assert_eq!(
            added["repository"]["branch_rules"][0]["required_approvals"],
            1
        );

        let delete_rule = Request::builder()
            .method("DELETE")
            .uri("/repo/api/settings/branch-rules/main")
            .header(HOST, "127.0.0.1:8080")
            .body(Body::empty())
            .unwrap();
        let mut delete_rule = delete_rule;
        delete_rule.extensions_mut().insert(ConnectInfo(localhost));
        let deleted = app.clone().oneshot(delete_rule).await.unwrap();
        assert_eq!(deleted.status(), StatusCode::OK);
        let deleted: serde_json::Value =
            serde_json::from_slice(&deleted.into_body().collect().await.unwrap().to_bytes())
                .unwrap();
        assert_eq!(
            deleted["repository"]["branch_rules"]
                .as_array()
                .unwrap()
                .len(),
            0
        );
    }

    #[tokio::test]
    async fn raw_blob_endpoint_serves_plain_text() {
        let app = app(true, false);
        let remote = SocketAddr::from(([127, 0, 0, 1], 3000));
        let response = app
            .oneshot(request_with_remote(
                "/repo/api/code/raw?path=README.md&reference=main",
                remote,
                "127.0.0.1:8080",
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response.headers().get(CONTENT_TYPE).unwrap(),
            HeaderValue::from_static("text/plain; charset=utf-8")
        );
        let body = String::from_utf8(
            response
                .into_body()
                .collect()
                .await
                .unwrap()
                .to_bytes()
                .to_vec(),
        )
        .unwrap();
        assert_eq!(body, "hello");
    }

    #[tokio::test]
    async fn viewers_can_comment_and_review_but_not_manage_pull_requests() {
        let app = app(false, true);
        let remote = SocketAddr::from(([10, 0, 0, 2], 3000));
        let cookie = login_cookie(&app, remote, "demo.ngrok.app").await;

        let create_pr = Request::builder()
            .method("POST")
            .uri("/repo/api/pull-requests")
            .header(HOST, "demo.ngrok.app")
            .header("content-type", "application/json")
            .header("cookie", cookie.clone())
            .body(Body::from(
                r#"{"title":"Review me","description":"needs feedback","source_branch":"feature","target_branch":"main"}"#,
            ))
            .unwrap();
        let mut create_pr = create_pr;
        create_pr.extensions_mut().insert(ConnectInfo(remote));
        let create_response = app.clone().oneshot(create_pr).await.unwrap();
        let created: PullRequestRecord = serde_json::from_slice(
            &create_response
                .into_body()
                .collect()
                .await
                .unwrap()
                .to_bytes(),
        )
        .unwrap();

        let comment = Request::builder()
            .method("POST")
            .uri(format!("/repo/api/pull-requests/{}/comments", created.id))
            .header(HOST, "demo.ngrok.app")
            .header("content-type", "application/json")
            .header("cookie", cookie.clone())
            .body(Body::from(
                "{\"display_name\":\"Casey\",\"body\":\"Please add tests.\"}",
            ))
            .unwrap();
        let mut comment = comment;
        comment.extensions_mut().insert(ConnectInfo(remote));
        let comment_response = app.clone().oneshot(comment).await.unwrap();
        assert_eq!(comment_response.status(), StatusCode::OK);

        let review = Request::builder()
            .method("POST")
            .uri(format!("/repo/api/pull-requests/{}/reviews", created.id))
            .header(HOST, "demo.ngrok.app")
            .header("content-type", "application/json")
            .header("cookie", cookie.clone())
            .body(Body::from(
                r#"{"display_name":"Casey","body":"One more pass, please.","state":"changes_requested"}"#,
            ))
            .unwrap();
        let mut review = review;
        review.extensions_mut().insert(ConnectInfo(remote));
        let review_response = app.clone().oneshot(review).await.unwrap();
        assert_eq!(review_response.status(), StatusCode::OK);

        let update = Request::builder()
            .method("PATCH")
            .uri(format!("/repo/api/pull-requests/{}", created.id))
            .header(HOST, "demo.ngrok.app")
            .header("content-type", "application/json")
            .header("cookie", cookie.clone())
            .body(Body::from(r#"{"status":"closed"}"#))
            .unwrap();
        let mut update = update;
        update.extensions_mut().insert(ConnectInfo(remote));
        let update_response = app.clone().oneshot(update).await.unwrap();
        assert_eq!(update_response.status(), StatusCode::FORBIDDEN);

        let delete = Request::builder()
            .method("DELETE")
            .uri(format!("/repo/api/pull-requests/{}", created.id))
            .header(HOST, "demo.ngrok.app")
            .header("cookie", cookie.clone())
            .body(Body::empty())
            .unwrap();
        let mut delete = delete;
        delete.extensions_mut().insert(ConnectInfo(remote));
        let delete_response = app.clone().oneshot(delete).await.unwrap();
        assert_eq!(delete_response.status(), StatusCode::FORBIDDEN);

        let detail = Request::builder()
            .uri(format!("/repo/api/pull-requests/{}", created.id))
            .header(HOST, "demo.ngrok.app")
            .header("cookie", cookie)
            .body(Body::empty())
            .unwrap();
        let mut detail = detail;
        detail.extensions_mut().insert(ConnectInfo(remote));
        let detail_response = app.clone().oneshot(detail).await.unwrap();
        let payload: serde_json::Value = serde_json::from_slice(
            &detail_response
                .into_body()
                .collect()
                .await
                .unwrap()
                .to_bytes(),
        )
        .unwrap();
        assert_eq!(payload["comments"][0]["display_name"], "Casey");
        assert_eq!(payload["reviews"][0]["state"], "changes_requested");
        assert_eq!(payload["review_summary"]["changes_requested"], 1);
        assert!(payload["activity"].as_array().unwrap().len() >= 3);
    }

    #[tokio::test]
    async fn owners_can_edit_close_reopen_and_delete_pull_requests() {
        let app = app(true, false);
        let remote = SocketAddr::from(([127, 0, 0, 1], 3000));

        let create_pr = Request::builder()
            .method("POST")
            .uri("/repo/api/pull-requests")
            .header(HOST, "127.0.0.1:8080")
            .header("content-type", "application/json")
            .body(Body::from(
                r#"{"title":"Owner PR","description":"owner flow","source_branch":"feature","target_branch":"main"}"#,
            ))
            .unwrap();
        let mut create_pr = create_pr;
        create_pr.extensions_mut().insert(ConnectInfo(remote));
        let create_response = app.clone().oneshot(create_pr).await.unwrap();
        let created: PullRequestRecord = serde_json::from_slice(
            &create_response
                .into_body()
                .collect()
                .await
                .unwrap()
                .to_bytes(),
        )
        .unwrap();

        let close = Request::builder()
            .method("PATCH")
            .uri(format!("/repo/api/pull-requests/{}", created.id))
            .header(HOST, "127.0.0.1:8080")
            .header("content-type", "application/json")
            .body(Body::from(
                r#"{"title":"Owner PR updated","status":"closed"}"#,
            ))
            .unwrap();
        let mut close = close;
        close.extensions_mut().insert(ConnectInfo(remote));
        let close_response = app.clone().oneshot(close).await.unwrap();
        let closed: PullRequestRecord = serde_json::from_slice(
            &close_response
                .into_body()
                .collect()
                .await
                .unwrap()
                .to_bytes(),
        )
        .unwrap();
        assert_eq!(closed.title, "Owner PR updated");
        assert_eq!(closed.status, qit_domain::PullRequestStatus::Closed);

        let reopen = Request::builder()
            .method("PATCH")
            .uri(format!("/repo/api/pull-requests/{}", created.id))
            .header(HOST, "127.0.0.1:8080")
            .header("content-type", "application/json")
            .body(Body::from(r#"{"status":"open"}"#))
            .unwrap();
        let mut reopen = reopen;
        reopen.extensions_mut().insert(ConnectInfo(remote));
        let reopen_response = app.clone().oneshot(reopen).await.unwrap();
        let reopened: PullRequestRecord = serde_json::from_slice(
            &reopen_response
                .into_body()
                .collect()
                .await
                .unwrap()
                .to_bytes(),
        )
        .unwrap();
        assert_eq!(reopened.status, qit_domain::PullRequestStatus::Open);

        let delete = Request::builder()
            .method("DELETE")
            .uri(format!("/repo/api/pull-requests/{}", created.id))
            .header(HOST, "127.0.0.1:8080")
            .body(Body::empty())
            .unwrap();
        let mut delete = delete;
        delete.extensions_mut().insert(ConnectInfo(remote));
        let delete_response = app.clone().oneshot(delete).await.unwrap();
        assert_eq!(delete_response.status(), StatusCode::OK);

        let list = app
            .oneshot(request_with_remote(
                "/repo/api/pull-requests",
                remote,
                "127.0.0.1:8080",
            ))
            .await
            .unwrap();
        let payload: serde_json::Value =
            serde_json::from_slice(&list.into_body().collect().await.unwrap().to_bytes()).unwrap();
        assert!(payload["pull_requests"].as_array().unwrap().is_empty());
    }

    #[tokio::test]
    async fn issues_support_metadata_comments_and_owner_updates() {
        let app = app(true, false);
        let remote = SocketAddr::from(([127, 0, 0, 1], 3000));

        let label = Request::builder()
            .method("PUT")
            .uri("/repo/api/issues/labels")
            .header(HOST, "127.0.0.1:8080")
            .header("content-type", "application/json")
            .body(Body::from(
                r#"{"name":"bug","color":"cf222e","description":"Bug work"}"#,
            ))
            .unwrap();
        let mut label = label;
        label.extensions_mut().insert(ConnectInfo(remote));
        let label_response = app.clone().oneshot(label).await.unwrap();
        assert_eq!(label_response.status(), StatusCode::OK);
        let label_body = label_response
            .into_body()
            .collect()
            .await
            .unwrap()
            .to_bytes();
        let label: qit_domain::IssueLabel = serde_json::from_slice(&label_body).unwrap();

        let milestone = Request::builder()
            .method("PUT")
            .uri("/repo/api/issues/milestones")
            .header(HOST, "127.0.0.1:8080")
            .header("content-type", "application/json")
            .body(Body::from(
                r#"{"title":"v1","description":"First release"}"#,
            ))
            .unwrap();
        let mut milestone = milestone;
        milestone.extensions_mut().insert(ConnectInfo(remote));
        let milestone_response = app.clone().oneshot(milestone).await.unwrap();
        assert_eq!(milestone_response.status(), StatusCode::OK);
        let milestone_body = milestone_response
            .into_body()
            .collect()
            .await
            .unwrap()
            .to_bytes();
        let milestone: qit_domain::IssueMilestone =
            serde_json::from_slice(&milestone_body).unwrap();

        let create_issue = Request::builder()
            .method("POST")
            .uri("/repo/api/issues")
            .header(HOST, "127.0.0.1:8080")
            .header("content-type", "application/json")
            .body(Body::from(format!(
                r#"{{"title":"Broken flow","description":"Need a fix","label_ids":["{}"],"milestone_id":"{}"}}"#,
                label.id, milestone.id
            )))
            .unwrap();
        let mut create_issue = create_issue;
        create_issue.extensions_mut().insert(ConnectInfo(remote));
        let create_issue_response = app.clone().oneshot(create_issue).await.unwrap();
        let issue: qit_domain::IssueRecord = serde_json::from_slice(
            &create_issue_response
                .into_body()
                .collect()
                .await
                .unwrap()
                .to_bytes(),
        )
        .unwrap();

        let comment = Request::builder()
            .method("POST")
            .uri(format!("/repo/api/issues/{}/comments", issue.id))
            .header(HOST, "127.0.0.1:8080")
            .header("content-type", "application/json")
            .body(Body::from(r#"{"body":"Looking into this now."}"#))
            .unwrap();
        let mut comment = comment;
        comment.extensions_mut().insert(ConnectInfo(remote));
        let comment_response = app.clone().oneshot(comment).await.unwrap();
        assert_eq!(comment_response.status(), StatusCode::OK);

        let react = Request::builder()
            .method("POST")
            .uri(format!("/repo/api/issues/{}/reactions", issue.id))
            .header(HOST, "127.0.0.1:8080")
            .header("content-type", "application/json")
            .body(Body::from(r#"{"content":"thumbs_up"}"#))
            .unwrap();
        let mut react = react;
        react.extensions_mut().insert(ConnectInfo(remote));
        let react_response = app.clone().oneshot(react).await.unwrap();
        assert_eq!(react_response.status(), StatusCode::OK);

        let close = Request::builder()
            .method("PATCH")
            .uri(format!("/repo/api/issues/{}", issue.id))
            .header(HOST, "127.0.0.1:8080")
            .header("content-type", "application/json")
            .body(Body::from(
                r#"{"title":"Broken login flow","status":"closed"}"#,
            ))
            .unwrap();
        let mut close = close;
        close.extensions_mut().insert(ConnectInfo(remote));
        let close_response = app.clone().oneshot(close).await.unwrap();
        let closed: qit_domain::IssueRecord = serde_json::from_slice(
            &close_response
                .into_body()
                .collect()
                .await
                .unwrap()
                .to_bytes(),
        )
        .unwrap();
        assert_eq!(closed.status, qit_domain::IssueStatus::Closed);

        let detail = Request::builder()
            .uri(format!("/repo/api/issues/{}", issue.id))
            .header(HOST, "127.0.0.1:8080")
            .body(Body::empty())
            .unwrap();
        let mut detail = detail;
        detail.extensions_mut().insert(ConnectInfo(remote));
        let detail_response = app.clone().oneshot(detail).await.unwrap();
        let payload: serde_json::Value = serde_json::from_slice(
            &detail_response
                .into_body()
                .collect()
                .await
                .unwrap()
                .to_bytes(),
        )
        .unwrap();
        assert_eq!(payload["issue"]["title"], "Broken login flow");
        assert_eq!(payload["reaction_summary"][0]["count"], 1);
        assert_eq!(
            payload["comments"][0]["comment"]["body"],
            "Looking into this now."
        );
        assert!(payload["timeline"].as_array().unwrap().len() >= 3);
    }

    #[tokio::test]
    async fn loopback_owner_bypass_does_not_apply_to_tunnel_hosts() {
        let app = app(true, true);
        let remote = SocketAddr::from(([127, 0, 0, 1], 3000));
        let response = app
            .oneshot(request_with_remote(
                "/repo/api/bootstrap",
                remote,
                "my-machine.tailnet.ts.net",
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = response.into_body().collect().await.unwrap().to_bytes();
        let payload: BootstrapResponse = serde_json::from_slice(&body).unwrap();
        assert_eq!(payload.actor, None);
        assert!(payload.local_only_owner_mode);
    }

    #[tokio::test]
    async fn repeated_failed_logins_are_rate_limited() {
        let app = app(false, true);
        let remote = SocketAddr::from(([10, 0, 0, 2], 3000));

        for _ in 0..5 {
            let login = Request::builder()
                .method("POST")
                .uri("/repo/api/session/login")
                .header(HOST, "demo.ngrok.app")
                .header("content-type", "application/json")
                .body(Body::from(r#"{"username":"tester","password":"wrong"}"#))
                .unwrap();
            let mut login = login;
            login.extensions_mut().insert(ConnectInfo(remote));
            let response = app.clone().oneshot(login).await.unwrap();
            assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        }

        let login = Request::builder()
            .method("POST")
            .uri("/repo/api/session/login")
            .header(HOST, "demo.ngrok.app")
            .header("content-type", "application/json")
            .body(Body::from(r#"{"username":"tester","password":"secret"}"#))
            .unwrap();
        let mut login = login;
        login.extensions_mut().insert(ConnectInfo(remote));
        let response = app.oneshot(login).await.unwrap();
        assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
    }
}
