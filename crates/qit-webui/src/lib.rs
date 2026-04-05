use axum::extract::{Json, Path as AxumPath, Query, State};
use axum::http::header::{CONTENT_TYPE, HOST, SET_COOKIE};
use axum::http::{HeaderMap, HeaderValue, Response, StatusCode};
use axum::response::{Html, IntoResponse};
use axum::routing::{delete, get, post};
use axum::{body::Body, Router};
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;
use base64::Engine as _;
use qit_domain::{
    BlobContent, BranchInfo, CommitDetail, CommitHistory, CommitRefDecoration, CommitRefKind,
    CreatePullRequest, CreatePullRequestComment, CreatePullRequestReview, PullRequestActivityRecord,
    PullRequestRecord, PullRequestReviewRecord, PullRequestReviewState, PullRequestReviewSummary,
    PullRequestStatus, RefComparison, RefDiffFile, RepoReadStore, SessionCredentials, UiRole,
    UpdatePullRequest, WorkspaceService, WorkspaceSpec, WorkspaceWebUiState,
};
use rand::distributions::{Alphanumeric, DistString};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tokio::sync::RwLock;

const APP_JS: &str = include_str!("../frontend/dist/assets/app.js");
const APP_CSS: &str = include_str!("../frontend/dist/assets/index.css");
const QIT_LOGO_ON_DARK: &[u8] = include_bytes!("../frontend/dist/assets/qit-logo-on-dark.png");
const QIT_LOGO_ON_LIGHT: &[u8] = include_bytes!("../frontend/dist/assets/qit-logo-on-light.png");
const SESSION_TTL_MS: u64 = 12 * 60 * 60 * 1000;

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
    role: UiRole,
    expires_at_ms: u64,
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
}

#[derive(Serialize, Deserialize)]
struct BootstrapResponse {
    actor: Option<UiRole>,
    repo_name: String,
    worktree: String,
    exported_branch: String,
    checked_out_branch: String,
    local_only_owner_mode: bool,
    shared_remote_identity: bool,
    git_username: Option<String>,
    git_password: Option<String>,
    public_repo_url: Option<String>,
}

#[derive(Serialize, Deserialize)]
struct SettingsResponse {
    local_only_owner_mode: bool,
    shared_remote_identity: bool,
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
    comments: Vec<qit_domain::PullRequestCommentRecord>,
    reviews: Vec<PullRequestReviewRecord>,
    review_summary: PullRequestReviewSummary,
    activity: Vec<PullRequestActivityRecord>,
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
        }
    }

    pub fn router(self) -> Router {
        let mount = self.repo_mount_path.clone();
        let state = Arc::new(self);
        Router::new()
            .route(&mount, get(index))
            .route(&format!("{mount}/"), get(index))
            .route(&format!("{mount}/assets/app.js"), get(app_js))
            .route(&format!("{mount}/assets/app.css"), get(app_css))
            .route(
                &format!("{mount}/assets/qit-logo-on-dark.png"),
                get(qit_logo_on_dark),
            )
            .route(
                &format!("{mount}/assets/qit-logo-on-light.png"),
                get(qit_logo_on_light),
            )
            .route(&format!("{mount}/assets/qit-og.svg"), get(qit_og_image))
            .route(&format!("{mount}/api/bootstrap"), get(bootstrap))
            .route(&format!("{mount}/api/session/login"), post(login))
            .route(&format!("{mount}/api/session/logout"), post(logout))
            .route(&format!("{mount}/api/settings"), get(get_settings))
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
            .route(&format!("{mount}/api/compare"), get(compare_refs))
            .route(
                &format!("{mount}/api/pull-requests"),
                get(list_pull_requests).post(create_pull_request),
            )
            .route(
                &format!("{mount}/api/pull-requests/{{id}}"),
                get(read_pull_request).patch(update_pull_request).delete(delete_pull_request),
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

    fn session_cookie(&self, token: &str) -> HeaderValue {
        let secure = if self.secure_cookies { "; Secure" } else { "" };
        HeaderValue::from_str(&format!(
            "{}={token}; Path={}; HttpOnly; SameSite=Lax{}",
            Self::cookie_name(),
            self.repo_mount_path,
            secure
        ))
        .expect("valid session cookie")
    }

    fn clear_cookie(&self) -> HeaderValue {
        let secure = if self.secure_cookies { "; Secure" } else { "" };
        HeaderValue::from_str(&format!(
            "{}=; Path={}; Max-Age=0; HttpOnly; SameSite=Lax{}",
            Self::cookie_name(),
            self.repo_mount_path,
            secure
        ))
        .expect("valid clear cookie")
    }

    fn now_ms() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_millis() as u64)
            .unwrap_or(0)
    }

    fn default_actor(&self, headers: &HeaderMap) -> Option<UiRole> {
        (self.implicit_owner_mode && host_is_loopback(headers)).then_some(UiRole::Owner)
    }

    fn repo_og_svg(&self) -> String {
        let repo = self.repo_name();
        let title_lines = split_repo_name(&repo);
        let title_font_size = if title_lines.iter().map(|line| line.chars().count()).max().unwrap_or(0) > 20 {
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

    async fn current_actor(
        &self,
        headers: &HeaderMap,
    ) -> Result<Option<UiRole>, StatusCode> {
        if let Some(actor) = self.default_actor(headers) {
            return Ok(Some(actor));
        }
        let Some(token) = cookie_value(headers, Self::cookie_name()) else {
            return Ok(None);
        };
        let now_ms = Self::now_ms();
        let mut sessions = self.sessions.write().await;
        sessions.retain(|_, session| session.expires_at_ms > now_ms);
        Ok(sessions.get(token).map(|session| session.role.clone()))
    }

    async fn require_actor(
        &self,
        headers: &HeaderMap,
    ) -> Result<UiRole, StatusCode> {
        self.current_actor(headers).await?.ok_or(StatusCode::UNAUTHORIZED)
    }

    async fn require_owner(&self, headers: &HeaderMap) -> Result<(), StatusCode> {
        match self.require_actor(headers).await? {
            UiRole::Owner => Ok(()),
            UiRole::User => Err(StatusCode::FORBIDDEN),
        }
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

fn host_is_loopback(headers: &HeaderMap) -> bool {
    let Some(host) = headers.get(HOST).and_then(|value| value.to_str().ok()) else {
        return false;
    };
    let host = host.trim();
    let host = if let Some(stripped) = host.strip_prefix('[') {
        stripped.split(']').next().unwrap_or(stripped)
    } else {
        host.split(':').next().unwrap_or(host)
    };
    matches!(host, "localhost" | "127.0.0.1" | "::1")
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
        .filter_map(|(index, ch)| matches!(ch, '-' | '_' | '.' | ' ') .then_some(index + 1))
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

async fn app_js() -> impl IntoResponse {
    (
        [(
            CONTENT_TYPE,
            HeaderValue::from_static("text/javascript; charset=utf-8"),
        )],
        APP_JS,
    )
}

async fn app_css() -> impl IntoResponse {
    (
        [(
            CONTENT_TYPE,
            HeaderValue::from_static("text/css; charset=utf-8"),
        )],
        APP_CSS,
    )
}

async fn qit_logo_on_dark() -> impl IntoResponse {
    (
        [(CONTENT_TYPE, HeaderValue::from_static("image/png"))],
        QIT_LOGO_ON_DARK,
    )
}

async fn qit_logo_on_light() -> impl IntoResponse {
    (
        [(CONTENT_TYPE, HeaderValue::from_static("image/png"))],
        QIT_LOGO_ON_LIGHT,
    )
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
    let (workspace, _) = state.latest_workspace()?;
    let actor = state.current_actor(&headers).await?;
    let is_authenticated = actor.is_some();
    Ok(Json(BootstrapResponse {
        actor,
        repo_name: state.repo_name(),
        worktree: workspace.worktree.display().to_string(),
        exported_branch: workspace.exported_branch,
        checked_out_branch: workspace.checked_out_branch,
        local_only_owner_mode: state.implicit_owner_mode,
        shared_remote_identity: true,
        git_username: is_authenticated.then(|| state.credentials.username.clone()),
        git_password: is_authenticated.then(|| state.credentials.password.clone()),
        public_repo_url: state.public_repo_url.clone(),
    }))
}

async fn login(
    State(state): State<Arc<WebUiServer>>,
    headers: HeaderMap,
    Json(body): Json<LoginRequest>,
) -> Result<Response<Body>, StatusCode> {
    if let Some(actor) = state.default_actor(&headers) {
        let payload = serde_json::to_vec(&actor).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
        return Response::builder()
            .status(StatusCode::OK)
            .header(CONTENT_TYPE, "application/json")
            .header(SET_COOKIE, state.clear_cookie())
            .body(Body::from(payload))
            .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR);
    }
    if !credentials_match(&body.username, &body.password, &state.credentials) {
        return Err(StatusCode::UNAUTHORIZED);
    }
    let token = Alphanumeric.sample_string(&mut rand::thread_rng(), 32);
    state
        .sessions
        .write()
        .await
        .insert(
            token.clone(),
            SessionRecord {
                role: UiRole::User,
                expires_at_ms: WebUiServer::now_ms().saturating_add(SESSION_TTL_MS),
            },
        );
    let payload =
        serde_json::to_vec(&UiRole::User).map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
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
        .status(StatusCode::OK)
        .header(SET_COOKIE, state.clear_cookie())
        .body(Body::empty())
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

async fn get_settings(
    State(state): State<Arc<WebUiServer>>,
    headers: HeaderMap,
) -> Result<Json<SettingsResponse>, StatusCode> {
    state.require_actor(&headers).await?;
    Ok(Json(SettingsResponse {
        local_only_owner_mode: state.implicit_owner_mode,
        shared_remote_identity: true,
    }))
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
        .into_iter()
        .find(|pull_request| pull_request.id == id)
    else {
        return Err(StatusCode::NOT_FOUND);
    };
    let (base_ref, head_ref) =
        qit_domain::resolve_pull_request_refs(state.repo_read_store.as_ref(), &workspace, &pull_request)
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
    let comments = WorkspaceService::pull_request_comments(&pull_request);
    let reviews = WorkspaceService::pull_request_reviews(&pull_request);
    let review_summary = WorkspaceService::pull_request_review_summary(&pull_request);
    let activity = pull_request.activities.clone();

    Ok(Json(PullRequestDetailResponse {
        pull_request,
        comparison,
        diffs,
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
    state.require_owner(&headers).await?;
    let (_, pull_request) = state
        .workspace_service
        .merge_pull_request(
            state.workspace.worktree.clone(),
            &state.workspace.exported_branch,
            &id,
        )
        .await
        .map_err(|_| StatusCode::BAD_REQUEST)?;
    Ok(Json(pull_request))
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

    fn app(implicit_owner_mode: bool, secure_cookies: bool) -> Router {
        app_with_web_ui(implicit_owner_mode, secure_cookies, WorkspaceWebUiState::default())
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
            .oneshot(request_with_remote("/repo/api/bootstrap", localhost, "127.0.0.1:8080"))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let body = response.into_body().collect().await.unwrap().to_bytes();
        let payload: BootstrapResponse = serde_json::from_slice(&body).unwrap();
        assert_eq!(payload.actor, Some(UiRole::Owner));
        assert!(payload.local_only_owner_mode);

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
            .oneshot(request_with_remote("/repo/api/bootstrap", remote, "demo.ngrok.app"))
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
        assert_eq!(logout_response.status(), StatusCode::OK);

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
            og_response.into_body().collect().await.unwrap().to_bytes().to_vec(),
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
        assert!(body.contains(r#"<link rel="icon" type="image/png" href="/repo/assets/qit-logo-on-dark.png" />"#));
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
            &create_response.into_body().collect().await.unwrap().to_bytes(),
        )
        .unwrap();

        let comment = Request::builder()
            .method("POST")
            .uri(format!("/repo/api/pull-requests/{}/comments", created.id))
            .header(HOST, "demo.ngrok.app")
            .header("content-type", "application/json")
            .header("cookie", cookie.clone())
            .body(Body::from("{\"display_name\":\"Casey\",\"body\":\"Please add tests.\"}"))
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
            &detail_response.into_body().collect().await.unwrap().to_bytes(),
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
            &create_response.into_body().collect().await.unwrap().to_bytes(),
        )
        .unwrap();

        let close = Request::builder()
            .method("PATCH")
            .uri(format!("/repo/api/pull-requests/{}", created.id))
            .header(HOST, "127.0.0.1:8080")
            .header("content-type", "application/json")
            .body(Body::from(r#"{"title":"Owner PR updated","status":"closed"}"#))
            .unwrap();
        let mut close = close;
        close.extensions_mut().insert(ConnectInfo(remote));
        let close_response = app.clone().oneshot(close).await.unwrap();
        let closed: PullRequestRecord = serde_json::from_slice(
            &close_response.into_body().collect().await.unwrap().to_bytes(),
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
            &reopen_response.into_body().collect().await.unwrap().to_bytes(),
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
}
