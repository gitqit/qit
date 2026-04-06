mod auth;
mod branch_rules;
mod issues;

use async_trait::async_trait;
use auth::{
    hash_secret, issue_access_request_secret, issue_onboarding_secret, issue_pat_secret,
    parse_access_request_secret, parse_onboarding_secret, parse_pat_secret, verify_secret,
};
pub use auth::{
    AccessRequest, AccessRequestProgress, AccessRequestStatus, AccessRequestView, AuthActivityKind,
    AuthActivityRecord, AuthActor, AuthActorKind, AuthMethod, AuthMode, AuthenticatedPrincipal,
    IssuedOnboarding, IssuedPat, PatRecord, PatRecordView, RepoAuthState, RepoUser, RepoUserRole,
    RepoUserStatus, RepoUserView, SubmittedAccessRequest, ONBOARDING_TOKEN_TTL_MS,
};
use fs2::FileExt;
use git2::Repository;
pub use issues::{
    CreateIssue, CreateIssueComment, IssueActor, IssueActorInput, IssueCommentRecord, IssueLabel,
    IssueLinkRelation, IssueLinkSource, IssueLinkedPullRequest, IssueMilestone,
    IssueReactionContent, IssueReactionRecord, IssueReactionSummary, IssueRecord, IssueSettings,
    IssueStatus, IssueTimelineEvent, IssueTimelineEventKind, IssueTimelineTarget,
    LinkedIssueReference, LinkedPullRequestReference, UpdateIssue, UpsertIssueLabel,
    UpsertIssueMilestone,
};
use serde::{Deserialize, Serialize};
use std::fs::{File, OpenOptions};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use thiserror::Error;
use uuid::Uuid;

pub const DEFAULT_BRANCH: &str = "main";
pub use branch_rules::{BranchProtection, BranchRule};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct WorkspaceId(pub Uuid);

impl WorkspaceId {
    pub fn from_worktree(worktree: &Path) -> Self {
        Self(Uuid::new_v5(
            &Uuid::NAMESPACE_URL,
            worktree.to_string_lossy().as_bytes(),
        ))
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct WorkspaceRecord {
    pub worktree: PathBuf,
    pub sidecar: PathBuf,
    pub exported_branch: String,
    #[serde(default)]
    pub checked_out_branch: Option<String>,
    #[serde(default)]
    pub web_ui: WorkspaceWebUiState,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceSpec {
    pub id: WorkspaceId,
    pub worktree: PathBuf,
    pub sidecar: PathBuf,
    /// Branch served to collaborators and advertised as the sidecar HEAD.
    pub exported_branch: String,
    /// Branch currently materialized in the host folder and tracked by apply state.
    pub checked_out_branch: String,
}

pub struct WorkspaceLockGuard {
    _file: File,
}

#[derive(Clone, PartialEq, Eq)]
pub struct SessionCredentials {
    pub username: String,
    pub password: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct RepositorySettings {
    #[serde(default)]
    pub description: String,
    #[serde(default)]
    pub homepage_url: String,
    #[serde(default)]
    pub branch_rules: Vec<BranchRule>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApplyOutcome {
    pub merged_to: String,
    pub commit: String,
}

#[derive(Clone, PartialEq, Eq)]
pub struct PreparedServe {
    pub workspace: WorkspaceSpec,
    pub credentials: SessionCredentials,
    pub snapshot_commit: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct BranchInfo {
    pub name: String,
    pub is_current: bool,
    pub is_served: bool,
    pub commit: String,
    pub summary: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BranchCreateOutcome {
    pub branch: String,
    pub commit: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BranchSwitchOutcome {
    pub previous_branch: String,
    pub current_branch: String,
    pub commit: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BranchCheckoutOutcome {
    pub previous_branch: String,
    pub current_branch: String,
    pub commit: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BranchRecord {
    pub name: String,
    pub commit: String,
    pub summary: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum UiRole {
    Owner,
    User,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PullRequestStatus {
    Open,
    Merged,
    Closed,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PullRequestReviewState {
    Commented,
    Approved,
    ChangesRequested,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PullRequestActivityKind {
    Opened,
    Commented,
    Reviewed,
    Edited,
    Closed,
    Reopened,
    Merged,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PullRequestActivityRecord {
    pub id: String,
    pub kind: PullRequestActivityKind,
    pub actor_role: UiRole,
    #[serde(default)]
    pub display_name: Option<String>,
    #[serde(default)]
    pub body: Option<String>,
    #[serde(default)]
    pub review_state: Option<PullRequestReviewState>,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub source_commit: Option<String>,
    #[serde(default)]
    pub target_commit: Option<String>,
    pub created_at_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PullRequestCommentRecord {
    pub id: String,
    pub actor_role: UiRole,
    pub display_name: String,
    pub body: String,
    pub created_at_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PullRequestReviewRecord {
    pub id: String,
    pub actor_role: UiRole,
    pub display_name: String,
    pub body: String,
    pub state: PullRequestReviewState,
    pub created_at_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PullRequestReviewSummaryEntry {
    pub actor_role: UiRole,
    pub display_name: String,
    pub state: PullRequestReviewState,
    pub reviewed_at_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PullRequestReviewSummary {
    pub approvals: usize,
    pub changes_requested: usize,
    pub comments: usize,
    pub latest_reviews: Vec<PullRequestReviewSummaryEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PullRequestRecord {
    pub id: String,
    pub title: String,
    pub description: String,
    pub source_branch: String,
    pub target_branch: String,
    #[serde(default)]
    pub source_commit: Option<String>,
    #[serde(default)]
    pub target_commit: Option<String>,
    pub status: PullRequestStatus,
    pub author_role: UiRole,
    pub created_at_ms: u64,
    pub updated_at_ms: u64,
    #[serde(default)]
    pub merged_commit: Option<String>,
    #[serde(default)]
    pub activities: Vec<PullRequestActivityRecord>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreatePullRequest {
    pub title: String,
    pub description: String,
    pub source_branch: String,
    pub target_branch: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct UpdatePullRequest {
    pub title: Option<String>,
    pub description: Option<String>,
    pub status: Option<PullRequestStatus>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreatePullRequestComment {
    pub display_name: String,
    pub body: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreatePullRequestReview {
    pub display_name: String,
    pub body: String,
    pub state: PullRequestReviewState,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct UpdateRepositorySettings {
    pub description: Option<String>,
    pub homepage_url: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpsertBranchRule {
    pub pattern: String,
    pub require_pull_request: bool,
    pub required_approvals: u8,
    pub dismiss_stale_approvals: bool,
    pub block_force_push: bool,
    pub block_delete: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct WorkspaceWebUiState {
    #[serde(default)]
    pub pull_requests: Vec<PullRequestRecord>,
    #[serde(default)]
    pub repository: RepositorySettings,
    #[serde(default)]
    pub issue_settings: IssueSettings,
    #[serde(default)]
    pub issues: Vec<IssueRecord>,
    #[serde(default)]
    pub auth: RepoAuthState,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct CommitSummary {
    pub id: String,
    pub summary: String,
    pub author: String,
    pub authored_at: i64,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CommitRefKind {
    Branch,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct CommitRefDecoration {
    pub name: String,
    pub kind: CommitRefKind,
    pub is_current: bool,
    pub is_served: bool,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct CommitHistoryNode {
    pub id: String,
    pub summary: String,
    pub author: String,
    pub authored_at: i64,
    pub parents: Vec<String>,
    #[serde(default)]
    pub refs: Vec<CommitRefDecoration>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct CommitHistory {
    pub reference: String,
    pub offset: usize,
    pub limit: usize,
    pub has_more: bool,
    pub commits: Vec<CommitHistoryNode>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct CommitFileChange {
    pub path: String,
    pub status: String,
    pub additions: usize,
    pub deletions: usize,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct CommitDetail {
    pub id: String,
    pub summary: String,
    pub message: String,
    pub author: String,
    pub authored_at: i64,
    pub parents: Vec<String>,
    pub changes: Vec<CommitFileChange>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TreeEntryKind {
    Tree,
    Blob,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct TreeEntry {
    pub name: String,
    pub path: String,
    pub oid: String,
    pub kind: TreeEntryKind,
    pub size: Option<usize>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct BlobContent {
    pub path: String,
    pub text: Option<String>,
    pub is_binary: bool,
    pub size: usize,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct RefComparison {
    pub base_ref: String,
    pub head_ref: String,
    pub merge_base: Option<String>,
    pub ahead_by: usize,
    pub behind_by: usize,
    pub commits: Vec<CommitSummary>,
}

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct RefDiffFile {
    pub path: String,
    pub previous_path: Option<String>,
    pub status: String,
    pub additions: usize,
    pub deletions: usize,
    pub original: Option<BlobContent>,
    pub modified: Option<BlobContent>,
}

#[derive(Debug, Error)]
pub enum DomainError {
    #[error("path resolution failed: {0}")]
    PathResolution(#[source] RegistryError),
    #[error("registry operation failed: {0}")]
    Registry(#[source] RegistryError),
    #[error("repository operation failed: {0}")]
    Repository(#[source] RepositoryError),
    #[error("invalid pull request state: {0}")]
    InvalidPullRequest(String),
    #[error("invalid issue state: {0}")]
    InvalidIssue(String),
    #[error("invalid repository settings: {0}")]
    InvalidSettings(String),
    #[error("branch rule blocked the operation: {0}")]
    BranchRuleViolation(String),
    #[error("invalid auth state: {0}")]
    InvalidAuth(String),
    #[error("access request not found: {0}")]
    AccessRequestNotFound(String),
    #[error("user not found: {0}")]
    UserNotFound(String),
    #[error("issue not found: {0}")]
    IssueNotFound(String),
    #[error("issue label not found: {0}")]
    IssueLabelNotFound(String),
    #[error("issue milestone not found: {0}")]
    IssueMilestoneNotFound(String),
    #[error("personal access token not found: {0}")]
    PatNotFound(String),
    #[error("onboarding token is invalid or expired")]
    InvalidOnboardingToken,
    #[error("authentication failed")]
    AuthenticationFailed,
    #[error(
        "refusing to serve existing Git worktree {0}; rerun with --allow-existing-git to opt in"
    )]
    ExistingGitWorktreeRequiresFlag(PathBuf),
    #[error("sidecar repo not found: {0}")]
    MissingSidecar(PathBuf),
    #[error(
        "workspace already serves branch `{current}`; restart without `--branch` or use `switch` to change it to `{requested}`"
    )]
    ExportedBranchConflict { current: String, requested: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum RegistryError {
    #[error("current directory lookup failed: {0}")]
    CurrentDirectory(String),
    #[error("worktree does not exist: {0}")]
    MissingWorktree(PathBuf),
    #[error("worktree is not a directory: {0}")]
    WorktreeNotDirectory(PathBuf),
    #[error("registry IO failed during {operation} for {path}: {message}")]
    Io {
        operation: &'static str,
        path: PathBuf,
        message: String,
    },
    #[error("registry JSON is invalid at {path}: {message}")]
    CorruptRegistry { path: PathBuf, message: String },
    #[error(
        "workspace record for {id:?} points at {actual_worktree} instead of {expected_worktree}"
    )]
    WorkspaceRecordMismatch {
        id: WorkspaceId,
        expected_worktree: PathBuf,
        actual_worktree: PathBuf,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum RepositoryError {
    #[error("git operation failed: {0}")]
    Git(String),
    #[error("io failed during {operation}: {message}")]
    Io {
        operation: &'static str,
        message: String,
    },
    #[error("workspace has local changes not snapshotted; snapshot or stash before apply")]
    DirtyWorktree,
    #[error("ref not found: {0}")]
    RefNotFound(String),
    #[error("branch already exists: {0}")]
    BranchExists(String),
    #[error("unsupported operation: {0}")]
    Unsupported(String),
    #[error("cannot delete the current branch: {0}")]
    CurrentBranch(String),
    #[error("cannot delete the served branch: {0}")]
    ServedBranch(String),
    #[error("branch is not fully merged: {0}")]
    BranchNotMerged(String),
    #[error("fast-forward apply not possible: {0}")]
    NotFastForward(String),
    #[error("invalid path: {0}")]
    InvalidPath(String),
    #[error("snapshot traversal failed: {0}")]
    SnapshotWalk(String),
    #[error("worktree does not exist: {0}")]
    MissingWorktree(PathBuf),
    #[error("worktree is not a directory: {0}")]
    WorktreeNotDirectory(PathBuf),
}

#[async_trait]
pub trait RepoStore: Send + Sync {
    async fn ensure_initialized(&self, workspace: &WorkspaceSpec) -> Result<(), RepositoryError>;
    async fn snapshot(
        &self,
        workspace: &WorkspaceSpec,
        message: &str,
    ) -> Result<Option<String>, RepositoryError>;
    async fn apply_fast_forward(
        &self,
        workspace: &WorkspaceSpec,
        source_ref: &str,
    ) -> Result<ApplyOutcome, RepositoryError>;
    async fn list_branches(
        &self,
        workspace: &WorkspaceSpec,
    ) -> Result<Vec<BranchRecord>, RepositoryError>;
    async fn create_branch(
        &self,
        workspace: &WorkspaceSpec,
        name: &str,
        start_point: Option<&str>,
        force: bool,
    ) -> Result<String, RepositoryError>;
    async fn rename_branch(
        &self,
        workspace: &WorkspaceSpec,
        old_name: &str,
        new_name: &str,
        force: bool,
    ) -> Result<(), RepositoryError>;
    async fn delete_branch(
        &self,
        workspace: &WorkspaceSpec,
        name: &str,
        force: bool,
    ) -> Result<(), RepositoryError>;
    async fn switch_branch(
        &self,
        workspace: &WorkspaceSpec,
        name: &str,
    ) -> Result<String, RepositoryError>;
    async fn checkout_branch(
        &self,
        workspace: &WorkspaceSpec,
        name: &str,
        force: bool,
    ) -> Result<String, RepositoryError>;
    async fn merge_branch(
        &self,
        workspace: &WorkspaceSpec,
        source_branch: &str,
        target_branch: &str,
    ) -> Result<String, RepositoryError>;
}

#[async_trait]
pub trait RepoReadStore: Send + Sync {
    async fn list_commits(
        &self,
        workspace: &WorkspaceSpec,
        reference: Option<&str>,
        offset: usize,
        limit: usize,
    ) -> Result<CommitHistory, RepositoryError>;
    async fn read_commit(
        &self,
        workspace: &WorkspaceSpec,
        commitish: &str,
    ) -> Result<CommitDetail, RepositoryError>;
    async fn list_tree(
        &self,
        workspace: &WorkspaceSpec,
        reference: &str,
        path: Option<&Path>,
    ) -> Result<Vec<TreeEntry>, RepositoryError>;
    async fn read_blob(
        &self,
        workspace: &WorkspaceSpec,
        reference: &str,
        path: &Path,
    ) -> Result<BlobContent, RepositoryError>;
    async fn compare_refs(
        &self,
        workspace: &WorkspaceSpec,
        base_ref: &str,
        head_ref: &str,
        limit: usize,
    ) -> Result<RefComparison, RepositoryError>;
    async fn diff_refs(
        &self,
        workspace: &WorkspaceSpec,
        base_ref: &str,
        head_ref: &str,
    ) -> Result<Vec<RefDiffFile>, RepositoryError>;
}

pub trait RegistryStore: Send + Sync {
    fn canonical_worktree(&self, worktree: &Path) -> Result<PathBuf, RegistryError>;
    fn default_sidecar_path(&self, id: WorkspaceId) -> Result<PathBuf, RegistryError>;
    fn load(&self, id: WorkspaceId) -> Result<Option<WorkspaceRecord>, RegistryError>;
    fn save(&self, id: WorkspaceId, record: WorkspaceRecord) -> Result<(), RegistryError>;
}

pub trait CredentialIssuer: Send + Sync {
    fn issue(&self) -> SessionCredentials;
}

pub fn lock_workspace(workspace: &WorkspaceSpec) -> Result<WorkspaceLockGuard, RepositoryError> {
    std::fs::create_dir_all(&workspace.sidecar).map_err(|error| RepositoryError::Io {
        operation: "create workspace lock directory",
        message: error.to_string(),
    })?;
    let lock_path = workspace.sidecar.join("qit.lock");
    let file = OpenOptions::new()
        .read(true)
        .write(true)
        .create(true)
        .truncate(false)
        .open(&lock_path)
        .map_err(|error| RepositoryError::Io {
            operation: "open workspace lock",
            message: format!("{}: {}", lock_path.display(), error),
        })?;
    file.lock_exclusive().map_err(|error| RepositoryError::Io {
        operation: "lock workspace",
        message: format!("{}: {}", lock_path.display(), error),
    })?;
    Ok(WorkspaceLockGuard { _file: file })
}

#[derive(Clone)]
pub struct WorkspaceService {
    repo_store: Arc<dyn RepoStore>,
    registry_store: Arc<dyn RegistryStore>,
    credential_issuer: Arc<dyn CredentialIssuer>,
}

impl WorkspaceService {
    fn now_ms() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_millis() as u64)
            .unwrap_or(0)
    }

    fn normalize_required(value: &str, field: &str) -> Result<String, DomainError> {
        let normalized = value.trim();
        if normalized.is_empty() {
            return Err(DomainError::InvalidPullRequest(format!(
                "{field} is required"
            )));
        }
        Ok(normalized.to_string())
    }

    fn normalize_issue_required(value: &str, field: &str) -> Result<String, DomainError> {
        let normalized = value.trim();
        if normalized.is_empty() {
            return Err(DomainError::InvalidIssue(format!("{field} is required")));
        }
        Ok(normalized.to_string())
    }

    fn normalize_optional(value: &str) -> String {
        value.trim().to_string()
    }

    fn normalize_issue_actor(actor: IssueActorInput) -> Result<IssueActor, DomainError> {
        let fallback = match actor.role {
            UiRole::Owner => "Owner",
            UiRole::User => "Viewer",
        };
        let display_name = actor
            .display_name
            .as_deref()
            .map(Self::normalize_optional)
            .filter(|value| !value.is_empty())
            .or_else(|| actor.username.as_deref().map(Self::normalize_optional))
            .filter(|value| !value.is_empty())
            .unwrap_or_else(|| fallback.to_string());
        if display_name.len() > 120 {
            return Err(DomainError::InvalidIssue(
                "display name must be 120 characters or fewer".into(),
            ));
        }
        Ok(IssueActor {
            role: actor.role,
            display_name,
            user_id: actor
                .user_id
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty()),
            username: actor
                .username
                .map(|value| value.trim().to_string())
                .filter(|value| !value.is_empty()),
        })
    }

    fn stable_issue_actor_key(actor: &IssueActor) -> Option<String> {
        actor
            .user_id
            .as_ref()
            .map(|value| format!("user:{value}"))
            .or_else(|| {
                actor
                    .username
                    .as_ref()
                    .map(|value| format!("username:{value}"))
            })
    }

    fn interaction_issue_actor_key(actor: &IssueActor) -> String {
        Self::stable_issue_actor_key(actor).unwrap_or_else(|| {
            format!(
                "role:{:?}:{}",
                actor.role,
                actor.display_name.trim().to_ascii_lowercase()
            )
        })
    }

    fn can_manage_issue(actor: &IssueActor, issue: &IssueRecord) -> bool {
        if actor.role == UiRole::Owner {
            return true;
        }
        match (
            Self::stable_issue_actor_key(actor),
            Self::stable_issue_actor_key(&issue.author),
        ) {
            (Some(actor_key), Some(author_key)) => actor_key == author_key,
            _ => false,
        }
    }

    fn normalize_auth_required(
        value: &str,
        field: &str,
        max_len: usize,
    ) -> Result<String, DomainError> {
        let normalized = value.trim();
        if normalized.is_empty() {
            return Err(DomainError::InvalidAuth(format!("{field} is required")));
        }
        if normalized.len() > max_len {
            return Err(DomainError::InvalidAuth(format!(
                "{field} must be {max_len} characters or fewer"
            )));
        }
        Ok(normalized.to_string())
    }

    fn normalize_email(value: &str) -> Result<String, DomainError> {
        let normalized = Self::normalize_auth_required(value, "email", 320)?.to_ascii_lowercase();
        if !normalized.contains('@') || normalized.starts_with('@') || normalized.ends_with('@') {
            return Err(DomainError::InvalidAuth("email must be valid".into()));
        }
        Ok(normalized)
    }

    fn normalize_username(value: &str) -> Result<String, DomainError> {
        let normalized = Self::normalize_auth_required(value, "username", 32)?.to_ascii_lowercase();
        if normalized.len() < 3 {
            return Err(DomainError::InvalidAuth(
                "username must be at least 3 characters".into(),
            ));
        }
        if !normalized.chars().all(|ch| {
            ch.is_ascii_lowercase() || ch.is_ascii_digit() || matches!(ch, '.' | '_' | '-')
        }) {
            return Err(DomainError::InvalidAuth(
                "username may only contain lowercase letters, digits, '.', '_' and '-'".into(),
            ));
        }
        Ok(normalized)
    }

    fn normalize_pat_label(value: &str) -> Result<String, DomainError> {
        Self::normalize_auth_required(value, "token label", 80)
    }

    fn owner_count(auth: &RepoAuthState) -> usize {
        auth.users
            .iter()
            .filter(|user| {
                user.role == RepoUserRole::Owner && user.status != RepoUserStatus::Revoked
            })
            .count()
    }

    fn ensure_not_last_owner(auth: &RepoAuthState, user_id: &str) -> Result<(), DomainError> {
        let is_owner = auth.users.iter().any(|user| {
            user.id == user_id
                && user.role == RepoUserRole::Owner
                && user.status != RepoUserStatus::Revoked
        });
        if is_owner && Self::owner_count(auth) <= 1 {
            return Err(DomainError::InvalidAuth(
                "cannot remove or demote the last durable owner".into(),
            ));
        }
        Ok(())
    }

    fn record_auth_activity(
        auth: &mut RepoAuthState,
        actor: &AuthActor,
        kind: AuthActivityKind,
        target_user_id: Option<String>,
        request_id: Option<String>,
        pat_id: Option<String>,
        detail: Option<String>,
        created_at_ms: u64,
    ) {
        auth.activity.push(AuthActivityRecord {
            id: Uuid::new_v4().to_string(),
            kind,
            actor_kind: actor.kind(),
            actor_label: actor.label(),
            target_user_id,
            request_id,
            pat_id,
            detail,
            created_at_ms,
        });
    }

    fn normalize_auth_state(mut auth: RepoAuthState) -> Result<RepoAuthState, DomainError> {
        if auth.methods.is_empty() {
            auth.methods = RepoAuthState::methods_for_mode(&auth.mode);
        }
        auth.methods.sort();
        auth.methods.dedup();
        auth.mode = RepoAuthState::compatibility_mode_from_methods(&auth.methods);
        for request in &mut auth.access_requests {
            request.name = Self::normalize_auth_required(&request.name, "request name", 120)?;
            request.email = Self::normalize_email(&request.email)?;
        }
        for user in &mut auth.users {
            user.name = Self::normalize_auth_required(&user.name, "user name", 120)?;
            user.email = Self::normalize_email(&user.email)?;
            if let Some(username) = &user.username {
                user.username = Some(Self::normalize_username(username)?);
            }
        }
        for pat in &mut auth.personal_access_tokens {
            pat.label = Self::normalize_pat_label(&pat.label)?;
        }

        let mut emails = std::collections::HashSet::new();
        let mut usernames = std::collections::HashSet::new();
        for user in &auth.users {
            if !emails.insert(user.email.clone()) {
                return Err(DomainError::InvalidAuth(format!(
                    "duplicate user email `{}`",
                    user.email
                )));
            }
            if let Some(username) = &user.username {
                if !usernames.insert(username.clone()) {
                    return Err(DomainError::InvalidAuth(format!(
                        "duplicate username `{username}`"
                    )));
                }
            }
        }
        for token in &auth.onboarding_tokens {
            if !auth.users.iter().any(|user| user.id == token.user_id) {
                return Err(DomainError::InvalidAuth(format!(
                    "onboarding token references missing user `{}`",
                    token.user_id
                )));
            }
        }
        for pat in &auth.personal_access_tokens {
            if !auth.users.iter().any(|user| user.id == pat.user_id) {
                return Err(DomainError::InvalidAuth(format!(
                    "token `{}` references missing user `{}`",
                    pat.id, pat.user_id
                )));
            }
        }
        Ok(auth)
    }

    fn require_auth_method(auth: &RepoAuthState, method: AuthMethod) -> Result<(), DomainError> {
        if !auth.has_method(&method) {
            return Err(DomainError::InvalidAuth(format!(
                "{} is not enabled for this repo",
                method.as_str().replace('_', "-")
            )));
        }
        Ok(())
    }

    fn find_user_mut<'a>(
        auth: &'a mut RepoAuthState,
        user_id: &str,
    ) -> Result<&'a mut RepoUser, DomainError> {
        auth.users
            .iter_mut()
            .find(|user| user.id == user_id)
            .ok_or_else(|| DomainError::UserNotFound(user_id.to_string()))
    }

    fn find_user<'a>(auth: &'a RepoAuthState, user_id: &str) -> Result<&'a RepoUser, DomainError> {
        auth.users
            .iter()
            .find(|user| user.id == user_id)
            .ok_or_else(|| DomainError::UserNotFound(user_id.to_string()))
    }

    fn find_active_user_by_username<'a>(
        auth: &'a RepoAuthState,
        username: &str,
    ) -> Option<&'a RepoUser> {
        auth.users.iter().find(|user| {
            user.status == RepoUserStatus::Active
                && user
                    .username
                    .as_deref()
                    .is_some_and(|candidate| candidate == username)
        })
    }

    fn access_request_index_from_secret(
        auth: &RepoAuthState,
        secret: &str,
    ) -> Result<usize, DomainError> {
        let (request_id, raw_secret) =
            parse_access_request_secret(secret).ok_or(DomainError::AuthenticationFailed)?;
        let request_index = auth
            .access_requests
            .iter()
            .position(|request| request.id == request_id)
            .ok_or(DomainError::AuthenticationFailed)?;
        let verifier = auth.access_requests[request_index]
            .request_secret_verifier
            .as_deref()
            .ok_or(DomainError::AuthenticationFailed)?;
        if !verify_secret(raw_secret, verifier) {
            return Err(DomainError::AuthenticationFailed);
        }
        Ok(request_index)
    }

    fn issue_onboarding_for_user(
        auth: &mut RepoAuthState,
        user_id: &str,
        now_ms: u64,
    ) -> Result<IssuedOnboarding, DomainError> {
        let user = Self::find_user(auth, user_id)?.clone();
        for token in auth
            .onboarding_tokens
            .iter_mut()
            .filter(|token| token.user_id == user_id && token.redeemed_at_ms.is_none())
        {
            token.redeemed_at_ms = Some(now_ms);
        }
        let token_id = Uuid::new_v4().to_string();
        let (secret, verifier) = issue_onboarding_secret(&token_id)?;
        let expires_at_ms = now_ms.saturating_add(ONBOARDING_TOKEN_TTL_MS);
        auth.onboarding_tokens.push(auth::OnboardingToken {
            id: token_id,
            user_id: user_id.to_string(),
            verifier,
            created_at_ms: now_ms,
            expires_at_ms,
            redeemed_at_ms: None,
        });
        Ok(IssuedOnboarding {
            user_id: user.id,
            email: user.email,
            secret: Some(secret),
            expires_at_ms,
        })
    }

    fn push_activity(
        pull_request: &mut PullRequestRecord,
        kind: PullRequestActivityKind,
        actor_role: UiRole,
        display_name: Option<String>,
        body: Option<String>,
        review_state: Option<PullRequestReviewState>,
        title: Option<String>,
        description: Option<String>,
        source_commit: Option<String>,
        target_commit: Option<String>,
        created_at_ms: u64,
    ) {
        pull_request.activities.push(PullRequestActivityRecord {
            id: Uuid::new_v4().to_string(),
            kind,
            actor_role,
            display_name,
            body,
            review_state,
            title,
            description,
            source_commit,
            target_commit,
            created_at_ms,
        });
    }

    fn normalize_branch_rule(rule: UpsertBranchRule) -> Result<BranchRule, DomainError> {
        let pattern = branch_rules::normalize_branch_rule_pattern(&rule.pattern)?;
        Ok(BranchRule {
            pattern,
            require_pull_request: rule.require_pull_request || rule.required_approvals > 0,
            required_approvals: rule.required_approvals,
            dismiss_stale_approvals: rule.dismiss_stale_approvals,
            block_force_push: rule.block_force_push,
            block_delete: rule.block_delete,
        })
    }

    fn normalize_repository_settings(
        settings: RepositorySettings,
    ) -> Result<RepositorySettings, DomainError> {
        let description = Self::normalize_optional(&settings.description);
        if description.len() > 280 {
            return Err(DomainError::InvalidSettings(
                "repository description must be 280 characters or fewer".into(),
            ));
        }
        let homepage_url = Self::normalize_optional(&settings.homepage_url);
        if homepage_url.len() > 512 {
            return Err(DomainError::InvalidSettings(
                "homepage URL must be 512 characters or fewer".into(),
            ));
        }
        let mut branch_rules = Vec::with_capacity(settings.branch_rules.len());
        for rule in settings.branch_rules {
            branch_rules.push(Self::normalize_branch_rule(UpsertBranchRule {
                pattern: rule.pattern,
                require_pull_request: rule.require_pull_request,
                required_approvals: rule.required_approvals,
                dismiss_stale_approvals: rule.dismiss_stale_approvals,
                block_force_push: rule.block_force_push,
                block_delete: rule.block_delete,
            })?);
        }
        branch_rules.sort_by(|left, right| left.pattern.cmp(&right.pattern));
        branch_rules.dedup_by(|left, right| left.pattern == right.pattern);
        Ok(RepositorySettings {
            description,
            homepage_url,
            branch_rules,
        })
    }

    fn default_issue_label_color(name: &str) -> String {
        let palette = [
            "1d76db", "8250df", "bf3989", "cf222e", "fb8c00", "9a6700", "2da44e", "0a7ea4",
        ];
        let hash = name.bytes().fold(0usize, |accumulator, byte| {
            accumulator.wrapping_add(byte as usize)
        });
        palette[hash % palette.len()].to_string()
    }

    fn normalize_issue_label(
        label: UpsertIssueLabel,
        existing_id: Option<&str>,
        now_ms: u64,
    ) -> Result<IssueLabel, DomainError> {
        let name = Self::normalize_issue_required(&label.name, "label name")?;
        if name.len() > 40 {
            return Err(DomainError::InvalidIssue(
                "label name must be 40 characters or fewer".into(),
            ));
        }
        let description = Self::normalize_optional(&label.description);
        if description.len() > 200 {
            return Err(DomainError::InvalidIssue(
                "label description must be 200 characters or fewer".into(),
            ));
        }
        let mut color = label
            .color
            .trim()
            .trim_start_matches('#')
            .to_ascii_lowercase();
        if color.is_empty() {
            color = Self::default_issue_label_color(&name);
        }
        if color.len() != 6 || !color.chars().all(|character| character.is_ascii_hexdigit()) {
            return Err(DomainError::InvalidIssue(
                "label color must be a 6-character hex value".into(),
            ));
        }
        Ok(IssueLabel {
            id: label
                .id
                .or_else(|| existing_id.map(ToString::to_string))
                .unwrap_or_else(|| Uuid::new_v4().to_string()),
            name,
            color,
            description,
            created_at_ms: now_ms,
            updated_at_ms: now_ms,
        })
    }

    fn normalize_issue_milestone(
        milestone: UpsertIssueMilestone,
        existing_id: Option<&str>,
        now_ms: u64,
    ) -> Result<IssueMilestone, DomainError> {
        let title = Self::normalize_issue_required(&milestone.title, "milestone title")?;
        if title.len() > 80 {
            return Err(DomainError::InvalidIssue(
                "milestone title must be 80 characters or fewer".into(),
            ));
        }
        let description = Self::normalize_optional(&milestone.description);
        if description.len() > 500 {
            return Err(DomainError::InvalidIssue(
                "milestone description must be 500 characters or fewer".into(),
            ));
        }
        Ok(IssueMilestone {
            id: milestone
                .id
                .or_else(|| existing_id.map(ToString::to_string))
                .unwrap_or_else(|| Uuid::new_v4().to_string()),
            title,
            description,
            created_at_ms: now_ms,
            updated_at_ms: now_ms,
        })
    }

    fn normalize_issue_settings(settings: IssueSettings) -> Result<IssueSettings, DomainError> {
        let mut normalized = IssueSettings {
            next_issue_number: settings.next_issue_number.max(1),
            labels: Vec::with_capacity(settings.labels.len()),
            milestones: Vec::with_capacity(settings.milestones.len()),
        };
        let now_ms = Self::now_ms();
        for label in settings.labels {
            let mut normalized_label = Self::normalize_issue_label(
                UpsertIssueLabel {
                    id: Some(label.id.clone()),
                    name: label.name,
                    color: label.color,
                    description: label.description,
                },
                Some(&label.id),
                now_ms,
            )?;
            normalized_label.created_at_ms = label.created_at_ms;
            normalized_label.updated_at_ms = label.updated_at_ms;
            normalized.labels.push(normalized_label);
        }
        normalized.labels.sort_by(|left, right| {
            left.name
                .to_ascii_lowercase()
                .cmp(&right.name.to_ascii_lowercase())
        });
        for pair in normalized.labels.windows(2) {
            if pair[0].name.eq_ignore_ascii_case(&pair[1].name) {
                return Err(DomainError::InvalidIssue(format!(
                    "duplicate issue label `{}`",
                    pair[0].name
                )));
            }
        }
        for milestone in settings.milestones {
            let mut normalized_milestone = Self::normalize_issue_milestone(
                UpsertIssueMilestone {
                    id: Some(milestone.id.clone()),
                    title: milestone.title,
                    description: milestone.description,
                },
                Some(&milestone.id),
                now_ms,
            )?;
            normalized_milestone.created_at_ms = milestone.created_at_ms;
            normalized_milestone.updated_at_ms = milestone.updated_at_ms;
            normalized.milestones.push(normalized_milestone);
        }
        normalized.milestones.sort_by(|left, right| {
            left.title
                .to_ascii_lowercase()
                .cmp(&right.title.to_ascii_lowercase())
        });
        for pair in normalized.milestones.windows(2) {
            if pair[0].title.eq_ignore_ascii_case(&pair[1].title) {
                return Err(DomainError::InvalidIssue(format!(
                    "duplicate issue milestone `{}`",
                    pair[0].title
                )));
            }
        }
        Ok(normalized)
    }

    fn push_issue_timeline_event(
        issue: &mut IssueRecord,
        kind: IssueTimelineEventKind,
        actor: IssueActor,
        body: Option<String>,
        title: Option<String>,
        description: Option<String>,
        labels: Vec<String>,
        assignee_user_ids: Vec<String>,
        milestone_id: Option<String>,
        pull_request_id: Option<String>,
        reaction: Option<IssueReactionContent>,
        target: Option<IssueTimelineTarget>,
        target_id: Option<String>,
        created_at_ms: u64,
    ) {
        issue.timeline.push(IssueTimelineEvent {
            id: Uuid::new_v4().to_string(),
            kind,
            actor,
            body,
            title,
            description,
            labels,
            assignee_user_ids,
            milestone_id,
            pull_request_id,
            reaction,
            target,
            target_id,
            created_at_ms,
        });
    }

    fn find_issue_index(issues: &[IssueRecord], issue_id: &str) -> Result<usize, DomainError> {
        issues
            .iter()
            .position(|issue| issue.id == issue_id)
            .ok_or_else(|| DomainError::IssueNotFound(issue_id.to_string()))
    }

    fn validate_issue_label_ids(
        settings: &IssueSettings,
        label_ids: &[String],
    ) -> Result<Vec<String>, DomainError> {
        let mut normalized = label_ids
            .iter()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .collect::<Vec<_>>();
        normalized.sort();
        normalized.dedup();
        for label_id in &normalized {
            if !settings.labels.iter().any(|label| label.id == *label_id) {
                return Err(DomainError::IssueLabelNotFound(label_id.clone()));
            }
        }
        Ok(normalized)
    }

    fn validate_issue_assignees(
        auth: &RepoAuthState,
        assignee_user_ids: &[String],
    ) -> Result<Vec<String>, DomainError> {
        let mut normalized = assignee_user_ids
            .iter()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .collect::<Vec<_>>();
        normalized.sort();
        normalized.dedup();
        for user_id in &normalized {
            let user = Self::find_user(auth, user_id)?;
            if user.status != RepoUserStatus::Active {
                return Err(DomainError::InvalidIssue(format!(
                    "assignee `{}` is not active",
                    user_id
                )));
            }
        }
        Ok(normalized)
    }

    fn validate_issue_milestone_id(
        settings: &IssueSettings,
        milestone_id: Option<String>,
    ) -> Result<Option<String>, DomainError> {
        let milestone_id = milestone_id
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());
        if let Some(milestone_id) = &milestone_id {
            if !settings
                .milestones
                .iter()
                .any(|milestone| milestone.id == *milestone_id)
            {
                return Err(DomainError::IssueMilestoneNotFound(milestone_id.clone()));
            }
        }
        Ok(milestone_id)
    }

    fn validate_linked_pull_request_ids(
        pull_requests: &[PullRequestRecord],
        linked_pull_request_ids: &[String],
        now_ms: u64,
    ) -> Result<Vec<IssueLinkedPullRequest>, DomainError> {
        let mut normalized = linked_pull_request_ids
            .iter()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty())
            .collect::<Vec<_>>();
        normalized.sort();
        normalized.dedup();
        for pull_request_id in &normalized {
            if !pull_requests
                .iter()
                .any(|pull_request| pull_request.id == *pull_request_id)
            {
                return Err(DomainError::InvalidIssue(format!(
                    "linked pull request `{}` was not found",
                    pull_request_id
                )));
            }
        }
        Ok(normalized
            .into_iter()
            .map(|pull_request_id| IssueLinkedPullRequest {
                pull_request_id,
                relation: IssueLinkRelation::Related,
                source: IssueLinkSource::Manual,
                linked_at_ms: now_ms,
            })
            .collect())
    }

    fn issue_link_relation_rank(relation: &IssueLinkRelation) -> u8 {
        match relation {
            IssueLinkRelation::Related => 0,
            IssueLinkRelation::Closing => 1,
        }
    }

    fn issue_link_source_rank(source: &IssueLinkSource) -> u8 {
        match source {
            IssueLinkSource::Manual => 0,
            IssueLinkSource::PullRequestDescription => 1,
            IssueLinkSource::PullRequestComment => 2,
            IssueLinkSource::PullRequestReview => 3,
            IssueLinkSource::IssueDescription => 4,
            IssueLinkSource::IssueComment => 5,
        }
    }

    fn should_replace_issue_link(
        current_relation: &IssueLinkRelation,
        current_source: &IssueLinkSource,
        next_relation: &IssueLinkRelation,
        next_source: &IssueLinkSource,
    ) -> bool {
        let current_relation_rank = Self::issue_link_relation_rank(current_relation);
        let next_relation_rank = Self::issue_link_relation_rank(next_relation);
        if next_relation_rank != current_relation_rank {
            return next_relation_rank > current_relation_rank;
        }
        Self::issue_link_source_rank(next_source) < Self::issue_link_source_rank(current_source)
    }

    fn is_reference_word(byte: u8) -> bool {
        byte.is_ascii_alphanumeric() || byte == b'_'
    }

    fn matches_reference_keyword(haystack: &str, start: usize, keyword: &str) -> bool {
        if !haystack[start..].starts_with(keyword) {
            return false;
        }
        if start > 0 && Self::is_reference_word(haystack.as_bytes()[start - 1]) {
            return false;
        }
        let end = start + keyword.len();
        if end < haystack.len() && Self::is_reference_word(haystack.as_bytes()[end]) {
            return false;
        }
        true
    }

    fn skip_reference_spacing(haystack: &str, mut cursor: usize) -> usize {
        while cursor < haystack.len() {
            let next = &haystack[cursor..];
            if next.starts_with("issue ") {
                cursor += "issue ".len();
                continue;
            }
            if next.starts_with("issues ") {
                cursor += "issues ".len();
                continue;
            }
            let byte = haystack.as_bytes()[cursor];
            if byte.is_ascii_whitespace() || matches!(byte, b':' | b'(' | b'[') {
                cursor += 1;
                continue;
            }
            break;
        }
        cursor
    }

    fn parse_reference_number(haystack: &str, cursor: usize) -> Option<(u64, usize)> {
        if cursor >= haystack.len() || haystack.as_bytes()[cursor] != b'#' {
            return None;
        }
        let digits_start = cursor + 1;
        let mut digits_end = digits_start;
        while digits_end < haystack.len() && haystack.as_bytes()[digits_end].is_ascii_digit() {
            digits_end += 1;
        }
        if digits_end == digits_start {
            return None;
        }
        haystack[digits_start..digits_end]
            .parse::<u64>()
            .ok()
            .map(|number| (number, digits_end))
    }

    fn consume_reference_separator(haystack: &str, mut cursor: usize) -> Option<usize> {
        while cursor < haystack.len() && haystack.as_bytes()[cursor].is_ascii_whitespace() {
            cursor += 1;
        }
        let next = &haystack[cursor..];
        if next.starts_with(',') {
            return Some(cursor + 1);
        }
        if next.starts_with("and ") {
            return Some(cursor + "and ".len());
        }
        if next.starts_with("or ") {
            return Some(cursor + "or ".len());
        }
        None
    }

    fn parse_issue_references(text: &str) -> Vec<(u64, IssueLinkRelation)> {
        const CLOSING_KEYWORDS: [&str; 6] =
            ["fixes", "fix", "closes", "close", "resolves", "resolve"];
        const RELATED_KEYWORDS: [&str; 5] =
            ["related to", "relates to", "refs", "ref", "references"];

        let lowercase = text.to_ascii_lowercase();
        let mut references = Vec::new();
        let mut cursor = 0usize;
        while cursor < lowercase.len() {
            let mut matched = None;
            for keyword in CLOSING_KEYWORDS {
                if Self::matches_reference_keyword(&lowercase, cursor, keyword) {
                    matched = Some((IssueLinkRelation::Closing, keyword.len()));
                    break;
                }
            }
            if matched.is_none() {
                for keyword in RELATED_KEYWORDS {
                    if Self::matches_reference_keyword(&lowercase, cursor, keyword) {
                        matched = Some((IssueLinkRelation::Related, keyword.len()));
                        break;
                    }
                }
            }
            let Some((relation, keyword_len)) = matched else {
                cursor += 1;
                continue;
            };

            let mut next_cursor = Self::skip_reference_spacing(&lowercase, cursor + keyword_len);
            while let Some((number, after_number)) =
                Self::parse_reference_number(&lowercase, next_cursor)
            {
                references.push((number, relation.clone()));
                let Some(separator_cursor) =
                    Self::consume_reference_separator(&lowercase, after_number)
                else {
                    next_cursor = after_number;
                    break;
                };
                next_cursor = Self::skip_reference_spacing(&lowercase, separator_cursor);
            }
            cursor = next_cursor.max(cursor + keyword_len);
        }
        references
    }

    fn parse_pull_request_reference_selectors(text: &str) -> Vec<String> {
        const KEYWORDS: [&str; 2] = ["pull request", "pr"];

        let lowercase = text.to_ascii_lowercase();
        let mut selectors = Vec::new();
        let mut cursor = 0usize;
        while cursor < lowercase.len() {
            let mut keyword_len = None;
            for keyword in KEYWORDS {
                if Self::matches_reference_keyword(&lowercase, cursor, keyword) {
                    keyword_len = Some(keyword.len());
                    break;
                }
            }
            let Some(keyword_len) = keyword_len else {
                cursor += 1;
                continue;
            };

            let mut token_start = cursor + keyword_len;
            while token_start < lowercase.len()
                && (lowercase.as_bytes()[token_start].is_ascii_whitespace()
                    || lowercase.as_bytes()[token_start] == b'#'
                    || lowercase.as_bytes()[token_start] == b':')
            {
                token_start += 1;
            }
            let mut token_end = token_start;
            while token_end < lowercase.len() {
                let byte = lowercase.as_bytes()[token_end];
                if byte.is_ascii_alphanumeric() || byte == b'-' {
                    token_end += 1;
                    continue;
                }
                break;
            }
            if token_end > token_start {
                selectors.push(text[token_start..token_end].trim().to_string());
                cursor = token_end;
            } else {
                cursor += keyword_len;
            }
        }
        selectors
    }

    fn resolve_pull_request_selector(
        pull_requests: &[PullRequestRecord],
        selector: &str,
    ) -> Option<String> {
        let selector = selector.trim();
        if selector.is_empty() {
            return None;
        }
        if let Some(exact) = pull_requests
            .iter()
            .find(|pull_request| pull_request.id == selector)
        {
            return Some(exact.id.clone());
        }
        let mut matches = pull_requests
            .iter()
            .filter(|pull_request| pull_request.id.starts_with(selector));
        let first = matches.next()?;
        if matches.next().is_some() {
            return None;
        }
        Some(first.id.clone())
    }

    fn accumulate_issue_pull_request_link(
        by_issue: &mut std::collections::BTreeMap<
            String,
            std::collections::BTreeMap<String, LinkedPullRequestReference>,
        >,
        by_pull_request: &mut std::collections::BTreeMap<
            String,
            std::collections::BTreeMap<String, LinkedIssueReference>,
        >,
        issue: &IssueRecord,
        pull_request_id: &str,
        relation: IssueLinkRelation,
        source: IssueLinkSource,
    ) {
        let issue_entry = by_issue.entry(issue.id.clone()).or_default();
        match issue_entry.get_mut(pull_request_id) {
            Some(existing)
                if Self::should_replace_issue_link(
                    &existing.relation,
                    &existing.source,
                    &relation,
                    &source,
                ) =>
            {
                existing.relation = relation.clone();
                existing.source = source.clone();
            }
            None => {
                issue_entry.insert(
                    pull_request_id.to_string(),
                    LinkedPullRequestReference {
                        pull_request_id: pull_request_id.to_string(),
                        relation: relation.clone(),
                        source: source.clone(),
                    },
                );
            }
            _ => {}
        }

        let pull_request_entry = by_pull_request
            .entry(pull_request_id.to_string())
            .or_default();
        match pull_request_entry.get_mut(&issue.id) {
            Some(existing)
                if Self::should_replace_issue_link(
                    &existing.relation,
                    &existing.source,
                    &relation,
                    &source,
                ) =>
            {
                existing.relation = relation;
                existing.source = source;
            }
            None => {
                pull_request_entry.insert(
                    issue.id.clone(),
                    LinkedIssueReference {
                        issue_id: issue.id.clone(),
                        issue_number: issue.number,
                        relation,
                        source,
                    },
                );
            }
            _ => {}
        }
    }

    fn issue_pull_request_cross_links(
        issues: &[IssueRecord],
        pull_requests: &[PullRequestRecord],
    ) -> (
        std::collections::BTreeMap<String, Vec<LinkedPullRequestReference>>,
        std::collections::BTreeMap<String, Vec<LinkedIssueReference>>,
    ) {
        let mut by_issue = std::collections::BTreeMap::<
            String,
            std::collections::BTreeMap<String, LinkedPullRequestReference>,
        >::new();
        let mut by_pull_request = std::collections::BTreeMap::<
            String,
            std::collections::BTreeMap<String, LinkedIssueReference>,
        >::new();
        let issue_by_number = issues
            .iter()
            .map(|issue| (issue.number, issue))
            .collect::<std::collections::BTreeMap<_, _>>();

        for issue in issues {
            for linked in &issue.linked_pull_requests {
                if pull_requests
                    .iter()
                    .any(|pull_request| pull_request.id == linked.pull_request_id)
                {
                    Self::accumulate_issue_pull_request_link(
                        &mut by_issue,
                        &mut by_pull_request,
                        issue,
                        &linked.pull_request_id,
                        linked.relation.clone(),
                        linked.source.clone(),
                    );
                }
            }

            for selector in Self::parse_pull_request_reference_selectors(&issue.description) {
                if let Some(pull_request_id) =
                    Self::resolve_pull_request_selector(pull_requests, &selector)
                {
                    Self::accumulate_issue_pull_request_link(
                        &mut by_issue,
                        &mut by_pull_request,
                        issue,
                        &pull_request_id,
                        IssueLinkRelation::Related,
                        IssueLinkSource::IssueDescription,
                    );
                }
            }
            for comment in &issue.comments {
                for selector in Self::parse_pull_request_reference_selectors(&comment.body) {
                    if let Some(pull_request_id) =
                        Self::resolve_pull_request_selector(pull_requests, &selector)
                    {
                        Self::accumulate_issue_pull_request_link(
                            &mut by_issue,
                            &mut by_pull_request,
                            issue,
                            &pull_request_id,
                            IssueLinkRelation::Related,
                            IssueLinkSource::IssueComment,
                        );
                    }
                }
            }
        }

        for pull_request in pull_requests {
            for (issue_number, relation) in Self::parse_issue_references(&pull_request.description)
            {
                if let Some(issue) = issue_by_number.get(&issue_number) {
                    Self::accumulate_issue_pull_request_link(
                        &mut by_issue,
                        &mut by_pull_request,
                        issue,
                        &pull_request.id,
                        relation,
                        IssueLinkSource::PullRequestDescription,
                    );
                }
            }
            for comment in Self::pull_request_comments(pull_request) {
                for (issue_number, relation) in Self::parse_issue_references(&comment.body) {
                    if let Some(issue) = issue_by_number.get(&issue_number) {
                        Self::accumulate_issue_pull_request_link(
                            &mut by_issue,
                            &mut by_pull_request,
                            issue,
                            &pull_request.id,
                            relation,
                            IssueLinkSource::PullRequestComment,
                        );
                    }
                }
            }
            for review in Self::pull_request_reviews(pull_request) {
                for (issue_number, relation) in Self::parse_issue_references(&review.body) {
                    if let Some(issue) = issue_by_number.get(&issue_number) {
                        Self::accumulate_issue_pull_request_link(
                            &mut by_issue,
                            &mut by_pull_request,
                            issue,
                            &pull_request.id,
                            relation,
                            IssueLinkSource::PullRequestReview,
                        );
                    }
                }
            }
        }

        let mut issue_links = std::collections::BTreeMap::new();
        for (issue_id, links) in by_issue {
            let mut values = links.into_values().collect::<Vec<_>>();
            values.sort_by(|left, right| {
                Self::issue_link_relation_rank(&right.relation)
                    .cmp(&Self::issue_link_relation_rank(&left.relation))
                    .then_with(|| {
                        Self::issue_link_source_rank(&left.source)
                            .cmp(&Self::issue_link_source_rank(&right.source))
                    })
                    .then_with(|| left.pull_request_id.cmp(&right.pull_request_id))
            });
            issue_links.insert(issue_id, values);
        }

        let mut pull_request_links = std::collections::BTreeMap::new();
        for (pull_request_id, links) in by_pull_request {
            let mut values = links.into_values().collect::<Vec<_>>();
            values.sort_by(|left, right| {
                Self::issue_link_relation_rank(&right.relation)
                    .cmp(&Self::issue_link_relation_rank(&left.relation))
                    .then_with(|| {
                        Self::issue_link_source_rank(&left.source)
                            .cmp(&Self::issue_link_source_rank(&right.source))
                    })
                    .then_with(|| left.issue_number.cmp(&right.issue_number))
            });
            pull_request_links.insert(pull_request_id, values);
        }

        (issue_links, pull_request_links)
    }

    pub fn linked_pull_requests_for_issue(
        issue: &IssueRecord,
        issues: &[IssueRecord],
        pull_requests: &[PullRequestRecord],
    ) -> Vec<LinkedPullRequestReference> {
        let (mut issue_links, _) = Self::issue_pull_request_cross_links(issues, pull_requests);
        issue_links.remove(&issue.id).unwrap_or_default()
    }

    pub fn linked_issues_for_pull_request(
        pull_request: &PullRequestRecord,
        issues: &[IssueRecord],
        pull_requests: &[PullRequestRecord],
    ) -> Vec<LinkedIssueReference> {
        let (_, mut pull_request_links) =
            Self::issue_pull_request_cross_links(issues, pull_requests);
        pull_request_links
            .remove(&pull_request.id)
            .unwrap_or_default()
    }

    fn issue_reaction_summary_for_records(
        reactions: &[IssueReactionRecord],
        viewer_actor: Option<&IssueActor>,
    ) -> Vec<IssueReactionSummary> {
        let viewer_key = viewer_actor.map(Self::interaction_issue_actor_key);
        let mut groups = std::collections::BTreeMap::<IssueReactionContent, (usize, bool)>::new();
        for reaction in reactions {
            let entry = groups.entry(reaction.content.clone()).or_insert((0, false));
            entry.0 += 1;
            if viewer_key
                .as_ref()
                .is_some_and(|key| *key == Self::interaction_issue_actor_key(&reaction.actor))
            {
                entry.1 = true;
            }
        }
        groups
            .into_iter()
            .map(|(content, (count, reacted))| IssueReactionSummary {
                content,
                count,
                reacted,
            })
            .collect()
    }

    pub fn pull_request_comments(
        pull_request: &PullRequestRecord,
    ) -> Vec<PullRequestCommentRecord> {
        pull_request
            .activities
            .iter()
            .filter_map(|activity| {
                if activity.kind != PullRequestActivityKind::Commented {
                    return None;
                }
                Some(PullRequestCommentRecord {
                    id: activity.id.clone(),
                    actor_role: activity.actor_role.clone(),
                    display_name: activity.display_name.clone().unwrap_or_else(|| {
                        match activity.actor_role {
                            UiRole::Owner => "Owner".into(),
                            UiRole::User => "Viewer".into(),
                        }
                    }),
                    body: activity.body.clone().unwrap_or_default(),
                    created_at_ms: activity.created_at_ms,
                })
            })
            .collect()
    }

    pub fn pull_request_reviews(pull_request: &PullRequestRecord) -> Vec<PullRequestReviewRecord> {
        pull_request
            .activities
            .iter()
            .filter_map(|activity| {
                if activity.kind != PullRequestActivityKind::Reviewed {
                    return None;
                }
                Some(PullRequestReviewRecord {
                    id: activity.id.clone(),
                    actor_role: activity.actor_role.clone(),
                    display_name: activity.display_name.clone().unwrap_or_else(|| {
                        match activity.actor_role {
                            UiRole::Owner => "Owner".into(),
                            UiRole::User => "Viewer".into(),
                        }
                    }),
                    body: activity.body.clone().unwrap_or_default(),
                    state: activity
                        .review_state
                        .clone()
                        .unwrap_or(PullRequestReviewState::Commented),
                    created_at_ms: activity.created_at_ms,
                })
            })
            .collect()
    }

    pub fn pull_request_review_summary(
        pull_request: &PullRequestRecord,
    ) -> PullRequestReviewSummary {
        Self::pull_request_review_summary_for_source(
            pull_request,
            pull_request.source_commit.as_deref(),
            false,
        )
    }

    pub fn pull_request_review_summary_for_source(
        pull_request: &PullRequestRecord,
        current_source_commit: Option<&str>,
        dismiss_stale_approvals: bool,
    ) -> PullRequestReviewSummary {
        let mut latest_by_reviewer: Vec<PullRequestReviewSummaryEntry> = Vec::new();
        for review in Self::pull_request_reviews(pull_request) {
            if dismiss_stale_approvals {
                let matching_review = pull_request.activities.iter().find(|activity| {
                    activity.kind == PullRequestActivityKind::Reviewed && activity.id == review.id
                });
                if let Some(source_commit) = current_source_commit {
                    if matching_review
                        .and_then(|activity| activity.source_commit.as_deref())
                        .is_some_and(|review_commit| review_commit != source_commit)
                    {
                        continue;
                    }
                }
            }
            if let Some(existing) = latest_by_reviewer.iter_mut().find(|entry| {
                entry.actor_role == review.actor_role && entry.display_name == review.display_name
            }) {
                if existing.reviewed_at_ms <= review.created_at_ms {
                    existing.state = review.state;
                    existing.reviewed_at_ms = review.created_at_ms;
                }
            } else {
                latest_by_reviewer.push(PullRequestReviewSummaryEntry {
                    actor_role: review.actor_role,
                    display_name: review.display_name,
                    state: review.state,
                    reviewed_at_ms: review.created_at_ms,
                });
            }
        }
        latest_by_reviewer.sort_by(|left, right| right.reviewed_at_ms.cmp(&left.reviewed_at_ms));
        let approvals = latest_by_reviewer
            .iter()
            .filter(|entry| entry.state == PullRequestReviewState::Approved)
            .count();
        let changes_requested = latest_by_reviewer
            .iter()
            .filter(|entry| entry.state == PullRequestReviewState::ChangesRequested)
            .count();
        let comments = latest_by_reviewer
            .iter()
            .filter(|entry| entry.state == PullRequestReviewState::Commented)
            .count();
        PullRequestReviewSummary {
            approvals,
            changes_requested,
            comments,
            latest_reviews: latest_by_reviewer,
        }
    }

    pub fn issue_reaction_summary(
        issue: &IssueRecord,
        viewer_actor: Option<&IssueActor>,
    ) -> Vec<IssueReactionSummary> {
        Self::issue_reaction_summary_for_records(&issue.reactions, viewer_actor)
    }

    pub fn issue_comment_reaction_summary(
        comment: &IssueCommentRecord,
        viewer_actor: Option<&IssueActor>,
    ) -> Vec<IssueReactionSummary> {
        Self::issue_reaction_summary_for_records(&comment.reactions, viewer_actor)
    }

    fn branch_head_commit(branches: &[BranchInfo], name: &str) -> Option<String> {
        branches
            .iter()
            .find(|branch| branch.name == name)
            .map(|branch| branch.commit.clone())
    }

    fn lock_resolved_workspace(
        &self,
        workspace: &WorkspaceSpec,
    ) -> Result<WorkspaceLockGuard, DomainError> {
        lock_workspace(workspace).map_err(DomainError::Repository)
    }

    fn matches_branch_patterns(name: &str, patterns: &[String]) -> bool {
        patterns.is_empty()
            || patterns
                .iter()
                .any(|pattern| branch_rules::glob_match(pattern.as_bytes(), name.as_bytes()))
    }

    pub fn branch_protection(settings: &RepositorySettings, name: &str) -> BranchProtection {
        let mut protection = BranchProtection::default();
        for rule in settings
            .branch_rules
            .iter()
            .filter(|rule| branch_rules::glob_match(rule.pattern.as_bytes(), name.as_bytes()))
        {
            protection.patterns.push(rule.pattern.clone());
            protection.require_pull_request |= rule.require_pull_request;
            protection.required_approvals =
                protection.required_approvals.max(rule.required_approvals);
            protection.dismiss_stale_approvals |= rule.dismiss_stale_approvals;
            protection.block_force_push |= rule.block_force_push;
            protection.block_delete |= rule.block_delete;
        }
        if protection.required_approvals > 0 {
            protection.require_pull_request = true;
        }
        protection
    }

    pub fn new(
        repo_store: Arc<dyn RepoStore>,
        registry_store: Arc<dyn RegistryStore>,
        credential_issuer: Arc<dyn CredentialIssuer>,
    ) -> Self {
        Self {
            repo_store,
            registry_store,
            credential_issuer,
        }
    }

    fn existing_git_worktree_branch(worktree: &Path) -> Result<Option<String>, RepositoryError> {
        let worktree = dunce::canonicalize(worktree).unwrap_or_else(|_| worktree.to_path_buf());
        if !worktree.join(".git").exists() {
            return Ok(None);
        }

        let repo = Repository::discover(&worktree).map_err(|error| {
            RepositoryError::Git(format!(
                "inspect existing Git worktree {}: {}",
                worktree.display(),
                error
            ))
        })?;
        if repo.is_bare() {
            return Ok(None);
        }

        let Some(repo_workdir) = repo.workdir() else {
            return Ok(None);
        };
        let repo_workdir =
            dunce::canonicalize(repo_workdir).unwrap_or_else(|_| repo_workdir.to_path_buf());
        if repo_workdir != worktree {
            return Ok(None);
        }

        let branch = repo
            .head()
            .ok()
            .filter(|head| head.is_branch())
            .and_then(|head| head.shorthand().map(ToString::to_string))
            .unwrap_or_else(|| DEFAULT_BRANCH.to_string());
        Ok(Some(branch))
    }

    fn resolve_serve_workspace(
        &self,
        path: PathBuf,
        fallback_branch: &str,
        requested_exported_branch: Option<&str>,
        allow_existing_git: bool,
    ) -> Result<WorkspaceSpec, DomainError> {
        let mut workspace = self
            .resolve_workspace(path, fallback_branch)
            .map_err(DomainError::PathResolution)?;
        let has_record = self
            .registry_store
            .load(workspace.id)
            .map_err(DomainError::PathResolution)?
            .is_some();
        if has_record {
            return Ok(workspace);
        }

        if let Some(current_branch) = Self::existing_git_worktree_branch(&workspace.worktree)
            .map_err(DomainError::Repository)?
        {
            if !allow_existing_git {
                return Err(DomainError::ExistingGitWorktreeRequiresFlag(
                    workspace.worktree.clone(),
                ));
            }
            workspace.checked_out_branch = current_branch.clone();
            workspace.exported_branch = requested_exported_branch
                .unwrap_or(&current_branch)
                .to_string();
        }

        Ok(workspace)
    }

    pub async fn prepare_serve(
        &self,
        path: PathBuf,
        requested_exported_branch: Option<&str>,
        snapshot_message: &str,
        allow_existing_git: bool,
    ) -> Result<PreparedServe, DomainError> {
        let fallback_branch = requested_exported_branch.unwrap_or(DEFAULT_BRANCH);
        let initial_workspace = self.resolve_serve_workspace(
            path.clone(),
            fallback_branch,
            requested_exported_branch,
            allow_existing_git,
        )?;
        let _lock = self.lock_resolved_workspace(&initial_workspace)?;
        let workspace = self.resolve_serve_workspace(
            path,
            fallback_branch,
            requested_exported_branch,
            allow_existing_git,
        )?;
        if let Some(requested_branch) = requested_exported_branch {
            if workspace.exported_branch != requested_branch {
                return Err(DomainError::ExportedBranchConflict {
                    current: workspace.exported_branch.clone(),
                    requested: requested_branch.to_string(),
                });
            }
        }

        self.repo_store
            .ensure_initialized(&workspace)
            .await
            .map_err(DomainError::Repository)?;

        let snapshot_commit = self
            .repo_store
            .snapshot(&workspace, snapshot_message)
            .await
            .map_err(DomainError::Repository)?;

        self.save_workspace(&workspace)?;

        Ok(PreparedServe {
            workspace,
            credentials: self.credential_issuer.issue(),
            snapshot_commit,
        })
    }

    pub fn resolve_workspace(
        &self,
        path: PathBuf,
        fallback_branch: &str,
    ) -> Result<WorkspaceSpec, RegistryError> {
        let worktree = self.registry_store.canonical_worktree(&path)?;
        let id = WorkspaceId::from_worktree(&worktree);
        if let Some(record) = self.registry_store.load(id)? {
            if record.worktree != worktree {
                return Err(RegistryError::WorkspaceRecordMismatch {
                    id,
                    expected_worktree: worktree,
                    actual_worktree: record.worktree,
                });
            }
            let exported_branch = record.exported_branch;
            return Ok(WorkspaceSpec {
                id,
                worktree,
                sidecar: record.sidecar,
                exported_branch: exported_branch.clone(),
                checked_out_branch: record.checked_out_branch.unwrap_or(exported_branch),
            });
        }

        let sidecar = self.registry_store.default_sidecar_path(id)?;
        Ok(WorkspaceSpec {
            id,
            worktree,
            sidecar,
            exported_branch: fallback_branch.to_string(),
            checked_out_branch: fallback_branch.to_string(),
        })
    }

    fn existing_workspace(
        &self,
        path: PathBuf,
        fallback_branch: &str,
    ) -> Result<WorkspaceSpec, DomainError> {
        let workspace = self
            .resolve_workspace(path, fallback_branch)
            .map_err(DomainError::PathResolution)?;
        if !workspace.sidecar.exists() {
            return Err(DomainError::MissingSidecar(workspace.sidecar.clone()));
        }
        Ok(workspace)
    }

    fn load_web_ui_state(
        &self,
        workspace: &WorkspaceSpec,
    ) -> Result<WorkspaceWebUiState, DomainError> {
        let mut state = self
            .registry_store
            .load(workspace.id)
            .map_err(DomainError::Registry)?
            .map(|record| record.web_ui)
            .unwrap_or_default();
        state.repository = Self::normalize_repository_settings(state.repository)?;
        state.issue_settings = Self::normalize_issue_settings(state.issue_settings)?;
        state.auth = Self::normalize_auth_state(state.auth)?;
        Ok(state)
    }

    fn save_workspace_with_web_ui(
        &self,
        workspace: &WorkspaceSpec,
        mut web_ui: WorkspaceWebUiState,
    ) -> Result<(), DomainError> {
        web_ui.repository = Self::normalize_repository_settings(web_ui.repository)?;
        web_ui.issue_settings = Self::normalize_issue_settings(web_ui.issue_settings)?;
        web_ui.auth = Self::normalize_auth_state(web_ui.auth)?;
        self.registry_store
            .save(
                workspace.id,
                WorkspaceRecord {
                    worktree: workspace.worktree.clone(),
                    sidecar: workspace.sidecar.clone(),
                    exported_branch: workspace.exported_branch.clone(),
                    checked_out_branch: Some(workspace.checked_out_branch.clone()),
                    web_ui,
                },
            )
            .map_err(DomainError::Registry)
    }

    fn save_workspace(&self, workspace: &WorkspaceSpec) -> Result<(), DomainError> {
        let web_ui = self.load_web_ui_state(workspace)?;
        self.save_workspace_with_web_ui(workspace, web_ui)
    }

    pub fn load_web_ui(
        &self,
        path: PathBuf,
        fallback_branch: &str,
    ) -> Result<(WorkspaceSpec, WorkspaceWebUiState), DomainError> {
        let workspace = self.existing_workspace(path, fallback_branch)?;
        let web_ui = self.load_web_ui_state(&workspace)?;
        Ok((workspace, web_ui))
    }

    pub fn read_auth_state(
        &self,
        path: PathBuf,
        fallback_branch: &str,
    ) -> Result<(WorkspaceSpec, RepoAuthState), DomainError> {
        let (workspace, web_ui) = self.load_web_ui(path, fallback_branch)?;
        Ok((workspace, web_ui.auth))
    }

    pub fn update_auth_mode(
        &self,
        path: PathBuf,
        fallback_branch: &str,
        mode: AuthMode,
        actor: &AuthActor,
    ) -> Result<(WorkspaceSpec, RepoAuthState), DomainError> {
        self.update_auth_methods(
            path,
            fallback_branch,
            RepoAuthState::methods_for_mode(&mode),
            actor,
        )
    }

    pub fn update_auth_methods(
        &self,
        path: PathBuf,
        fallback_branch: &str,
        methods: Vec<AuthMethod>,
        actor: &AuthActor,
    ) -> Result<(WorkspaceSpec, RepoAuthState), DomainError> {
        let initial_workspace = self.existing_workspace(path.clone(), fallback_branch)?;
        let _lock = self.lock_resolved_workspace(&initial_workspace)?;
        let workspace = self.existing_workspace(path, fallback_branch)?;
        let mut web_ui = self.load_web_ui_state(&workspace)?;
        web_ui.auth.methods = methods;
        web_ui.auth = Self::normalize_auth_state(web_ui.auth)?;
        let method_detail = web_ui.auth.method_labels().join(",");
        let now_ms = Self::now_ms();
        Self::record_auth_activity(
            &mut web_ui.auth,
            actor,
            AuthActivityKind::AuthModeChanged,
            None,
            None,
            None,
            Some(method_detail),
            now_ms,
        );
        let auth = web_ui.auth.clone();
        self.save_workspace_with_web_ui(&workspace, web_ui)?;
        Ok((workspace, auth))
    }

    pub fn submit_access_request(
        &self,
        path: PathBuf,
        fallback_branch: &str,
        name: &str,
        email: &str,
    ) -> Result<(WorkspaceSpec, SubmittedAccessRequest), DomainError> {
        let initial_workspace = self.existing_workspace(path.clone(), fallback_branch)?;
        let _lock = self.lock_resolved_workspace(&initial_workspace)?;
        let workspace = self.existing_workspace(path, fallback_branch)?;
        let mut web_ui = self.load_web_ui_state(&workspace)?;
        Self::require_auth_method(&web_ui.auth, AuthMethod::RequestAccess)?;
        let name = Self::normalize_auth_required(name, "name", 120)?;
        let email = Self::normalize_email(email)?;
        if web_ui.auth.users.iter().any(|user| user.email == email) {
            return Err(DomainError::InvalidAuth(
                "a user with that email already exists".into(),
            ));
        }
        if web_ui
            .auth
            .access_requests
            .iter()
            .any(|request| request.email == email && request.status == AccessRequestStatus::Pending)
        {
            return Err(DomainError::InvalidAuth(
                "a pending request already exists for that email".into(),
            ));
        }
        let now_ms = Self::now_ms();
        let request_id = Uuid::new_v4().to_string();
        let (secret, verifier) = issue_access_request_secret(&request_id)?;
        let request = AccessRequest {
            id: request_id,
            name,
            email,
            status: AccessRequestStatus::Pending,
            request_secret_verifier: Some(verifier),
            linked_user_id: None,
            created_at_ms: now_ms,
            reviewed_at_ms: None,
        };
        let view = AccessRequestView::from(&request);
        web_ui.auth.access_requests.push(request.clone());
        Self::record_auth_activity(
            &mut web_ui.auth,
            &AuthActor::Anonymous,
            AuthActivityKind::AccessRequested,
            None,
            Some(request.id),
            None,
            Some(request.email),
            now_ms,
        );
        self.save_workspace_with_web_ui(&workspace, web_ui)?;
        Ok((
            workspace,
            SubmittedAccessRequest {
                request: view,
                secret,
            },
        ))
    }

    pub fn read_access_request_progress(
        &self,
        path: PathBuf,
        fallback_branch: &str,
        secret: &str,
    ) -> Result<(WorkspaceSpec, AccessRequestProgress), DomainError> {
        let (workspace, auth) = self.read_auth_state(path, fallback_branch)?;
        let request_index = Self::access_request_index_from_secret(&auth, secret)?;
        let request = &auth.access_requests[request_index];
        Ok((
            workspace,
            AccessRequestProgress {
                id: request.id.clone(),
                status: request.status.clone(),
            },
        ))
    }

    pub fn approve_access_request(
        &self,
        path: PathBuf,
        fallback_branch: &str,
        request_id: &str,
        role: RepoUserRole,
        actor: &AuthActor,
    ) -> Result<(WorkspaceSpec, RepoUserView, IssuedOnboarding), DomainError> {
        let initial_workspace = self.existing_workspace(path.clone(), fallback_branch)?;
        let _lock = self.lock_resolved_workspace(&initial_workspace)?;
        let workspace = self.existing_workspace(path, fallback_branch)?;
        let mut web_ui = self.load_web_ui_state(&workspace)?;
        Self::require_auth_method(&web_ui.auth, AuthMethod::RequestAccess)?;
        Self::require_auth_method(&web_ui.auth, AuthMethod::SetupToken)?;
        let now_ms = Self::now_ms();
        let request_index = web_ui
            .auth
            .access_requests
            .iter()
            .position(|request| request.id == request_id)
            .ok_or_else(|| DomainError::AccessRequestNotFound(request_id.to_string()))?;
        let request = web_ui.auth.access_requests[request_index].clone();
        if request.status != AccessRequestStatus::Pending {
            return Err(DomainError::InvalidAuth(
                "only pending requests can be approved".into(),
            ));
        }
        if web_ui
            .auth
            .users
            .iter()
            .any(|user| user.email == request.email)
        {
            return Err(DomainError::InvalidAuth(
                "a user with that email already exists".into(),
            ));
        }
        let user = RepoUser {
            id: Uuid::new_v4().to_string(),
            name: request.name.clone(),
            email: request.email.clone(),
            username: None,
            password_verifier: None,
            role,
            status: RepoUserStatus::ApprovedPendingSetup,
            created_at_ms: now_ms,
            updated_at_ms: now_ms,
            approved_at_ms: Some(now_ms),
            activated_at_ms: None,
            revoked_at_ms: None,
        };
        web_ui.auth.access_requests[request_index].status = AccessRequestStatus::Approved;
        web_ui.auth.access_requests[request_index].linked_user_id = Some(user.id.clone());
        web_ui.auth.access_requests[request_index].reviewed_at_ms = Some(now_ms);
        web_ui.auth.users.push(user.clone());
        let issued = IssuedOnboarding {
            user_id: user.id.clone(),
            email: user.email.clone(),
            secret: None,
            expires_at_ms: 0,
        };
        Self::record_auth_activity(
            &mut web_ui.auth,
            actor,
            AuthActivityKind::AccessApproved,
            Some(user.id.clone()),
            Some(request.id),
            None,
            Some(user.email.clone()),
            now_ms,
        );
        self.save_workspace_with_web_ui(&workspace, web_ui)?;
        Ok((workspace, RepoUserView::from(&user), issued))
    }

    pub fn issue_setup_token(
        &self,
        path: PathBuf,
        fallback_branch: &str,
        name: &str,
        email: &str,
        role: RepoUserRole,
        actor: &AuthActor,
    ) -> Result<(WorkspaceSpec, RepoUserView, IssuedOnboarding), DomainError> {
        let initial_workspace = self.existing_workspace(path.clone(), fallback_branch)?;
        let _lock = self.lock_resolved_workspace(&initial_workspace)?;
        let workspace = self.existing_workspace(path, fallback_branch)?;
        let mut web_ui = self.load_web_ui_state(&workspace)?;
        Self::require_auth_method(&web_ui.auth, AuthMethod::SetupToken)?;
        let now_ms = Self::now_ms();
        let name = Self::normalize_auth_required(name, "name", 120)?;
        let email = Self::normalize_email(email)?;
        if web_ui.auth.users.iter().any(|user| user.email == email) {
            return Err(DomainError::InvalidAuth(
                "a user with that email already exists".into(),
            ));
        }

        let user = RepoUser {
            id: Uuid::new_v4().to_string(),
            name,
            email: email.clone(),
            username: None,
            password_verifier: None,
            role,
            status: RepoUserStatus::ApprovedPendingSetup,
            created_at_ms: now_ms,
            updated_at_ms: now_ms,
            approved_at_ms: Some(now_ms),
            activated_at_ms: None,
            revoked_at_ms: None,
        };
        let linked_request_id = web_ui
            .auth
            .access_requests
            .iter()
            .position(|request| {
                request.email == email && request.status == AccessRequestStatus::Pending
            })
            .map(|request_index| {
                let request = &mut web_ui.auth.access_requests[request_index];
                request.status = AccessRequestStatus::Approved;
                request.linked_user_id = Some(user.id.clone());
                request.reviewed_at_ms = Some(now_ms);
                request.id.clone()
            });
        web_ui.auth.users.push(user.clone());
        let onboarding = Self::issue_onboarding_for_user(&mut web_ui.auth, &user.id, now_ms)?;
        if let Some(request_id) = linked_request_id.clone() {
            Self::record_auth_activity(
                &mut web_ui.auth,
                actor,
                AuthActivityKind::AccessApproved,
                Some(user.id.clone()),
                Some(request_id),
                None,
                Some(email.clone()),
                now_ms,
            );
        }
        Self::record_auth_activity(
            &mut web_ui.auth,
            actor,
            AuthActivityKind::UserSetupIssued,
            Some(user.id.clone()),
            linked_request_id,
            None,
            Some(email),
            now_ms,
        );
        self.save_workspace_with_web_ui(&workspace, web_ui)?;
        Ok((workspace, RepoUserView::from(&user), onboarding))
    }

    pub fn reject_access_request(
        &self,
        path: PathBuf,
        fallback_branch: &str,
        request_id: &str,
        actor: &AuthActor,
    ) -> Result<(WorkspaceSpec, AccessRequestView), DomainError> {
        let initial_workspace = self.existing_workspace(path.clone(), fallback_branch)?;
        let _lock = self.lock_resolved_workspace(&initial_workspace)?;
        let workspace = self.existing_workspace(path, fallback_branch)?;
        let mut web_ui = self.load_web_ui_state(&workspace)?;
        Self::require_auth_method(&web_ui.auth, AuthMethod::RequestAccess)?;
        let now_ms = Self::now_ms();
        let request = web_ui
            .auth
            .access_requests
            .iter_mut()
            .find(|request| request.id == request_id)
            .ok_or_else(|| DomainError::AccessRequestNotFound(request_id.to_string()))?;
        request.status = AccessRequestStatus::Rejected;
        request.reviewed_at_ms = Some(now_ms);
        let request_view = AccessRequestView::from(&*request);
        let request_id = request.id.clone();
        let request_email = request.email.clone();
        Self::record_auth_activity(
            &mut web_ui.auth,
            actor,
            AuthActivityKind::AccessRejected,
            None,
            Some(request_id),
            None,
            Some(request_email),
            now_ms,
        );
        self.save_workspace_with_web_ui(&workspace, web_ui)?;
        Ok((workspace, request_view))
    }

    pub fn promote_user(
        &self,
        path: PathBuf,
        fallback_branch: &str,
        user_id: &str,
        actor: &AuthActor,
    ) -> Result<(WorkspaceSpec, RepoUserView), DomainError> {
        let initial_workspace = self.existing_workspace(path.clone(), fallback_branch)?;
        let _lock = self.lock_resolved_workspace(&initial_workspace)?;
        let workspace = self.existing_workspace(path, fallback_branch)?;
        let mut web_ui = self.load_web_ui_state(&workspace)?;
        let now_ms = Self::now_ms();
        let view = {
            let user = Self::find_user_mut(&mut web_ui.auth, user_id)?;
            if user.status == RepoUserStatus::Revoked {
                return Err(DomainError::InvalidAuth(
                    "cannot promote a revoked user".into(),
                ));
            }
            user.role = RepoUserRole::Owner;
            user.updated_at_ms = now_ms;
            RepoUserView::from(&*user)
        };
        Self::record_auth_activity(
            &mut web_ui.auth,
            actor,
            AuthActivityKind::UserPromoted,
            Some(user_id.to_string()),
            None,
            None,
            None,
            now_ms,
        );
        self.save_workspace_with_web_ui(&workspace, web_ui)?;
        Ok((workspace, view))
    }

    pub fn demote_user(
        &self,
        path: PathBuf,
        fallback_branch: &str,
        user_id: &str,
        actor: &AuthActor,
    ) -> Result<(WorkspaceSpec, RepoUserView), DomainError> {
        let initial_workspace = self.existing_workspace(path.clone(), fallback_branch)?;
        let _lock = self.lock_resolved_workspace(&initial_workspace)?;
        let workspace = self.existing_workspace(path, fallback_branch)?;
        let mut web_ui = self.load_web_ui_state(&workspace)?;
        Self::ensure_not_last_owner(&web_ui.auth, user_id)?;
        let now_ms = Self::now_ms();
        let view = {
            let user = Self::find_user_mut(&mut web_ui.auth, user_id)?;
            if user.status == RepoUserStatus::Revoked {
                return Err(DomainError::InvalidAuth(
                    "cannot demote a revoked user".into(),
                ));
            }
            user.role = RepoUserRole::User;
            user.updated_at_ms = now_ms;
            RepoUserView::from(&*user)
        };
        Self::record_auth_activity(
            &mut web_ui.auth,
            actor,
            AuthActivityKind::UserDemoted,
            Some(user_id.to_string()),
            None,
            None,
            None,
            now_ms,
        );
        self.save_workspace_with_web_ui(&workspace, web_ui)?;
        Ok((workspace, view))
    }

    pub fn revoke_user(
        &self,
        path: PathBuf,
        fallback_branch: &str,
        user_id: &str,
        actor: &AuthActor,
    ) -> Result<(WorkspaceSpec, RepoUserView), DomainError> {
        let initial_workspace = self.existing_workspace(path.clone(), fallback_branch)?;
        let _lock = self.lock_resolved_workspace(&initial_workspace)?;
        let workspace = self.existing_workspace(path, fallback_branch)?;
        let mut web_ui = self.load_web_ui_state(&workspace)?;
        Self::ensure_not_last_owner(&web_ui.auth, user_id)?;
        let now_ms = Self::now_ms();
        {
            let user = Self::find_user_mut(&mut web_ui.auth, user_id)?;
            user.status = RepoUserStatus::Revoked;
            user.updated_at_ms = now_ms;
            user.revoked_at_ms = Some(now_ms);
            user.password_verifier = None;
        }
        for token in web_ui
            .auth
            .onboarding_tokens
            .iter_mut()
            .filter(|token| token.user_id == user_id && token.redeemed_at_ms.is_none())
        {
            token.redeemed_at_ms = Some(now_ms);
        }
        for pat in web_ui
            .auth
            .personal_access_tokens
            .iter_mut()
            .filter(|pat| pat.user_id == user_id && pat.revoked_at_ms.is_none())
        {
            pat.revoked_at_ms = Some(now_ms);
        }
        let view = RepoUserView::from(Self::find_user(&web_ui.auth, user_id)?);
        Self::record_auth_activity(
            &mut web_ui.auth,
            actor,
            AuthActivityKind::UserRevoked,
            Some(user_id.to_string()),
            None,
            None,
            None,
            now_ms,
        );
        self.save_workspace_with_web_ui(&workspace, web_ui)?;
        Ok((workspace, view))
    }

    pub fn reset_user_setup(
        &self,
        path: PathBuf,
        fallback_branch: &str,
        user_id: &str,
        actor: &AuthActor,
    ) -> Result<(WorkspaceSpec, RepoUserView, IssuedOnboarding), DomainError> {
        let initial_workspace = self.existing_workspace(path.clone(), fallback_branch)?;
        let _lock = self.lock_resolved_workspace(&initial_workspace)?;
        let workspace = self.existing_workspace(path, fallback_branch)?;
        let mut web_ui = self.load_web_ui_state(&workspace)?;
        Self::require_auth_method(&web_ui.auth, AuthMethod::SetupToken)?;
        let now_ms = Self::now_ms();
        {
            let user = Self::find_user_mut(&mut web_ui.auth, user_id)?;
            if user.status == RepoUserStatus::Revoked {
                return Err(DomainError::InvalidAuth(
                    "cannot reset setup for a revoked user".into(),
                ));
            }
            user.status = RepoUserStatus::ApprovedPendingSetup;
            user.updated_at_ms = now_ms;
            user.activated_at_ms = None;
            user.password_verifier = None;
            user.username = None;
        }
        for pat in web_ui
            .auth
            .personal_access_tokens
            .iter_mut()
            .filter(|pat| pat.user_id == user_id && pat.revoked_at_ms.is_none())
        {
            pat.revoked_at_ms = Some(now_ms);
        }
        let onboarding = Self::issue_onboarding_for_user(&mut web_ui.auth, user_id, now_ms)?;
        let view = RepoUserView::from(Self::find_user(&web_ui.auth, user_id)?);
        Self::record_auth_activity(
            &mut web_ui.auth,
            actor,
            AuthActivityKind::UserSetupReset,
            Some(user_id.to_string()),
            None,
            None,
            None,
            now_ms,
        );
        self.save_workspace_with_web_ui(&workspace, web_ui)?;
        Ok((workspace, view, onboarding))
    }

    pub fn complete_onboarding(
        &self,
        path: PathBuf,
        fallback_branch: &str,
        secret: &str,
        username: &str,
        password: &str,
    ) -> Result<(WorkspaceSpec, AuthenticatedPrincipal), DomainError> {
        let initial_workspace = self.existing_workspace(path.clone(), fallback_branch)?;
        let _lock = self.lock_resolved_workspace(&initial_workspace)?;
        let workspace = self.existing_workspace(path, fallback_branch)?;
        let mut web_ui = self.load_web_ui_state(&workspace)?;
        Self::require_auth_method(&web_ui.auth, AuthMethod::SetupToken)?;
        let now_ms = Self::now_ms();
        let username = Self::normalize_username(username)?;
        if Self::find_active_user_by_username(&web_ui.auth, &username).is_some() {
            return Err(DomainError::InvalidAuth(
                "that username is already in use".into(),
            ));
        }
        if password.trim().len() < 10 {
            return Err(DomainError::InvalidAuth(
                "password must be at least 10 characters".into(),
            ));
        }
        let user_id = if let Some((token_id, raw_secret)) = parse_onboarding_secret(secret) {
            let token_index = web_ui
                .auth
                .onboarding_tokens
                .iter()
                .position(|token| token.id == token_id)
                .ok_or(DomainError::InvalidOnboardingToken)?;
            let token = web_ui.auth.onboarding_tokens[token_index].clone();
            if token.redeemed_at_ms.is_some() || token.expires_at_ms < now_ms {
                return Err(DomainError::InvalidOnboardingToken);
            }
            if !verify_secret(raw_secret, &token.verifier) {
                return Err(DomainError::InvalidOnboardingToken);
            }
            web_ui.auth.onboarding_tokens[token_index].redeemed_at_ms = Some(now_ms);
            token.user_id
        } else {
            let request_index = Self::access_request_index_from_secret(&web_ui.auth, secret)?;
            let request = web_ui.auth.access_requests[request_index].clone();
            if request.status != AccessRequestStatus::Approved {
                return Err(DomainError::InvalidAuth(
                    "that access request has not been approved yet".into(),
                ));
            }
            let user_id = request
                .linked_user_id
                .clone()
                .ok_or(DomainError::InvalidOnboardingToken)?;
            web_ui.auth.access_requests[request_index].request_secret_verifier = None;
            user_id
        };
        {
            let user = Self::find_user_mut(&mut web_ui.auth, &user_id)?;
            if user.status != RepoUserStatus::ApprovedPendingSetup {
                return Err(DomainError::InvalidOnboardingToken);
            }
            user.username = Some(username.clone());
            user.password_verifier = Some(hash_secret(password)?);
            user.status = RepoUserStatus::Active;
            user.updated_at_ms = now_ms;
            user.activated_at_ms = Some(now_ms);
        }
        let user = Self::find_user(&web_ui.auth, &user_id)?;
        let principal = AuthenticatedPrincipal {
            user_id: user.id.clone(),
            name: user.name.clone(),
            email: user.email.clone(),
            username,
            role: user.role.clone(),
        };
        Self::record_auth_activity(
            &mut web_ui.auth,
            &AuthActor::User {
                user_id: principal.user_id.clone(),
                username: principal.username.clone(),
                role: principal.role.clone(),
            },
            AuthActivityKind::UserSetupCompleted,
            Some(principal.user_id.clone()),
            None,
            None,
            None,
            now_ms,
        );
        self.save_workspace_with_web_ui(&workspace, web_ui)?;
        Ok((workspace, principal))
    }

    pub fn authenticate_web_user(
        &self,
        path: PathBuf,
        fallback_branch: &str,
        username: &str,
        password: &str,
    ) -> Result<(WorkspaceSpec, AuthenticatedPrincipal), DomainError> {
        let (workspace, auth) = self.read_auth_state(path, fallback_branch)?;
        let username =
            Self::normalize_username(username).map_err(|_| DomainError::AuthenticationFailed)?;
        let user = Self::find_active_user_by_username(&auth, &username)
            .ok_or(DomainError::AuthenticationFailed)?;
        let verifier = user
            .password_verifier
            .as_deref()
            .ok_or(DomainError::AuthenticationFailed)?;
        if !verify_secret(password, verifier) {
            return Err(DomainError::AuthenticationFailed);
        }
        Ok((
            workspace,
            AuthenticatedPrincipal {
                user_id: user.id.clone(),
                name: user.name.clone(),
                email: user.email.clone(),
                username,
                role: user.role.clone(),
            },
        ))
    }

    pub fn authenticate_git_user(
        &self,
        path: PathBuf,
        fallback_branch: &str,
        username: &str,
        secret: &str,
    ) -> Result<(WorkspaceSpec, AuthenticatedPrincipal), DomainError> {
        let (workspace, auth) = self.read_auth_state(path, fallback_branch)?;
        let username =
            Self::normalize_username(username).map_err(|_| DomainError::AuthenticationFailed)?;
        let user = Self::find_active_user_by_username(&auth, &username)
            .ok_or(DomainError::AuthenticationFailed)?;
        if let Some(verifier) = user.password_verifier.as_deref() {
            if verify_secret(secret, verifier) {
                return Ok((
                    workspace,
                    AuthenticatedPrincipal {
                        user_id: user.id.clone(),
                        name: user.name.clone(),
                        email: user.email.clone(),
                        username,
                        role: user.role.clone(),
                    },
                ));
            }
        }
        if let Some((pat_id, raw_secret)) = parse_pat_secret(secret) {
            if let Some(pat) = auth.personal_access_tokens.iter().find(|pat| {
                pat.id == pat_id && pat.user_id == user.id && pat.revoked_at_ms.is_none()
            }) {
                if verify_secret(raw_secret, &pat.verifier) {
                    return Ok((
                        workspace,
                        AuthenticatedPrincipal {
                            user_id: user.id.clone(),
                            name: user.name.clone(),
                            email: user.email.clone(),
                            username,
                            role: user.role.clone(),
                        },
                    ));
                }
            }
        }
        Err(DomainError::AuthenticationFailed)
    }

    pub fn resolve_active_principal(
        &self,
        path: PathBuf,
        fallback_branch: &str,
        user_id: &str,
    ) -> Result<(WorkspaceSpec, AuthenticatedPrincipal), DomainError> {
        let (workspace, auth) = self.read_auth_state(path, fallback_branch)?;
        let user = Self::find_user(&auth, user_id)?;
        if user.status != RepoUserStatus::Active {
            return Err(DomainError::AuthenticationFailed);
        }
        let username = user
            .username
            .clone()
            .ok_or(DomainError::AuthenticationFailed)?;
        Ok((
            workspace,
            AuthenticatedPrincipal {
                user_id: user.id.clone(),
                name: user.name.clone(),
                email: user.email.clone(),
                username,
                role: user.role.clone(),
            },
        ))
    }

    pub fn create_pat(
        &self,
        path: PathBuf,
        fallback_branch: &str,
        user_id: &str,
        label: &str,
        actor: &AuthActor,
    ) -> Result<(WorkspaceSpec, PatRecordView, IssuedPat), DomainError> {
        let initial_workspace = self.existing_workspace(path.clone(), fallback_branch)?;
        let _lock = self.lock_resolved_workspace(&initial_workspace)?;
        let workspace = self.existing_workspace(path, fallback_branch)?;
        let mut web_ui = self.load_web_ui_state(&workspace)?;
        let user = Self::find_user(&web_ui.auth, user_id)?;
        if user.status != RepoUserStatus::Active {
            return Err(DomainError::InvalidAuth(
                "only active users can create personal access tokens".into(),
            ));
        }
        let now_ms = Self::now_ms();
        let label = Self::normalize_pat_label(label)?;
        let pat_id = Uuid::new_v4().to_string();
        let (secret, verifier) = issue_pat_secret(&pat_id)?;
        let record = PatRecord {
            id: pat_id.clone(),
            user_id: user_id.to_string(),
            label: label.clone(),
            verifier,
            created_at_ms: now_ms,
            revoked_at_ms: None,
        };
        web_ui.auth.personal_access_tokens.push(record.clone());
        Self::record_auth_activity(
            &mut web_ui.auth,
            actor,
            AuthActivityKind::PatCreated,
            Some(user_id.to_string()),
            None,
            Some(pat_id.clone()),
            Some(label.clone()),
            now_ms,
        );
        self.save_workspace_with_web_ui(&workspace, web_ui)?;
        Ok((
            workspace,
            PatRecordView::from(&record),
            IssuedPat {
                id: pat_id,
                label,
                secret,
                created_at_ms: now_ms,
            },
        ))
    }

    pub fn revoke_pat(
        &self,
        path: PathBuf,
        fallback_branch: &str,
        pat_id: &str,
        actor: &AuthActor,
    ) -> Result<(WorkspaceSpec, PatRecordView), DomainError> {
        let initial_workspace = self.existing_workspace(path.clone(), fallback_branch)?;
        let _lock = self.lock_resolved_workspace(&initial_workspace)?;
        let workspace = self.existing_workspace(path, fallback_branch)?;
        let mut web_ui = self.load_web_ui_state(&workspace)?;
        let now_ms = Self::now_ms();
        let pat = web_ui
            .auth
            .personal_access_tokens
            .iter_mut()
            .find(|pat| pat.id == pat_id)
            .ok_or_else(|| DomainError::PatNotFound(pat_id.to_string()))?;
        pat.revoked_at_ms = Some(now_ms);
        let view = PatRecordView::from(&*pat);
        let pat_user_id = pat.user_id.clone();
        let pat_id = pat.id.clone();
        let pat_label = pat.label.clone();
        Self::record_auth_activity(
            &mut web_ui.auth,
            actor,
            AuthActivityKind::PatRevoked,
            Some(pat_user_id),
            None,
            Some(pat_id),
            Some(pat_label),
            now_ms,
        );
        self.save_workspace_with_web_ui(&workspace, web_ui)?;
        Ok((workspace, view))
    }

    pub fn read_repository_settings(
        &self,
        path: PathBuf,
        fallback_branch: &str,
    ) -> Result<(WorkspaceSpec, RepositorySettings), DomainError> {
        let (workspace, web_ui) = self.load_web_ui(path, fallback_branch)?;
        Ok((workspace, web_ui.repository))
    }

    pub fn update_repository_settings(
        &self,
        path: PathBuf,
        fallback_branch: &str,
        update: UpdateRepositorySettings,
    ) -> Result<(WorkspaceSpec, RepositorySettings), DomainError> {
        let initial_workspace = self.existing_workspace(path.clone(), fallback_branch)?;
        let _lock = self.lock_resolved_workspace(&initial_workspace)?;
        let workspace = self.existing_workspace(path, fallback_branch)?;
        let mut web_ui = self.load_web_ui_state(&workspace)?;
        if let Some(description) = update.description {
            web_ui.repository.description = Self::normalize_optional(&description);
        }
        if let Some(homepage_url) = update.homepage_url {
            web_ui.repository.homepage_url = Self::normalize_optional(&homepage_url);
        }
        web_ui.repository = Self::normalize_repository_settings(web_ui.repository)?;
        let settings = web_ui.repository.clone();
        self.save_workspace_with_web_ui(&workspace, web_ui)?;
        Ok((workspace, settings))
    }

    pub fn upsert_branch_rule(
        &self,
        path: PathBuf,
        fallback_branch: &str,
        rule: UpsertBranchRule,
    ) -> Result<(WorkspaceSpec, RepositorySettings), DomainError> {
        let initial_workspace = self.existing_workspace(path.clone(), fallback_branch)?;
        let _lock = self.lock_resolved_workspace(&initial_workspace)?;
        let workspace = self.existing_workspace(path, fallback_branch)?;
        let mut web_ui = self.load_web_ui_state(&workspace)?;
        let rule = Self::normalize_branch_rule(rule)?;
        if let Some(existing) = web_ui
            .repository
            .branch_rules
            .iter_mut()
            .find(|existing| existing.pattern == rule.pattern)
        {
            *existing = rule;
        } else {
            web_ui.repository.branch_rules.push(rule);
        }
        web_ui.repository = Self::normalize_repository_settings(web_ui.repository)?;
        let settings = web_ui.repository.clone();
        self.save_workspace_with_web_ui(&workspace, web_ui)?;
        Ok((workspace, settings))
    }

    pub fn delete_branch_rule(
        &self,
        path: PathBuf,
        fallback_branch: &str,
        pattern: &str,
    ) -> Result<(WorkspaceSpec, RepositorySettings), DomainError> {
        let initial_workspace = self.existing_workspace(path.clone(), fallback_branch)?;
        let _lock = self.lock_resolved_workspace(&initial_workspace)?;
        let workspace = self.existing_workspace(path, fallback_branch)?;
        let mut web_ui = self.load_web_ui_state(&workspace)?;
        let pattern = pattern.trim();
        if pattern.is_empty() {
            return Err(DomainError::InvalidSettings(
                "branch rule pattern is required".into(),
            ));
        }
        web_ui
            .repository
            .branch_rules
            .retain(|rule| rule.pattern != pattern);
        web_ui.repository = Self::normalize_repository_settings(web_ui.repository)?;
        let settings = web_ui.repository.clone();
        self.save_workspace_with_web_ui(&workspace, web_ui)?;
        Ok((workspace, settings))
    }

    pub async fn create_pull_request(
        &self,
        path: PathBuf,
        fallback_branch: &str,
        draft: CreatePullRequest,
        author_role: UiRole,
    ) -> Result<(WorkspaceSpec, PullRequestRecord), DomainError> {
        let initial_workspace = self.existing_workspace(path.clone(), fallback_branch)?;
        let _lock = self.lock_resolved_workspace(&initial_workspace)?;
        let workspace = self.existing_workspace(path, fallback_branch)?;
        let title = Self::normalize_required(&draft.title, "pull request title")?;
        let description = draft.description.trim().to_string();
        let branches = self
            .repo_store
            .list_branches(&workspace)
            .await
            .map_err(DomainError::Repository)?;
        let source_branch = branches
            .iter()
            .find(|branch| branch.name == draft.source_branch);
        let Some(source_branch) = source_branch else {
            return Err(DomainError::Repository(RepositoryError::RefNotFound(
                draft.source_branch.clone(),
            )));
        };
        let target_branch = branches
            .iter()
            .find(|branch| branch.name == draft.target_branch);
        let Some(target_branch) = target_branch else {
            return Err(DomainError::Repository(RepositoryError::RefNotFound(
                draft.target_branch.clone(),
            )));
        };
        let mut web_ui = self.load_web_ui_state(&workspace)?;
        let timestamp = Self::now_ms();
        let mut pull_request = PullRequestRecord {
            id: Uuid::new_v4().to_string(),
            title,
            description,
            source_branch: draft.source_branch,
            target_branch: draft.target_branch,
            source_commit: Some(source_branch.commit.clone()),
            target_commit: Some(target_branch.commit.clone()),
            status: PullRequestStatus::Open,
            author_role: author_role.clone(),
            created_at_ms: timestamp,
            updated_at_ms: timestamp,
            merged_commit: None,
            activities: Vec::new(),
        };
        let opened_source_commit = pull_request.source_commit.clone();
        let opened_target_commit = pull_request.target_commit.clone();
        Self::push_activity(
            &mut pull_request,
            PullRequestActivityKind::Opened,
            author_role,
            None,
            None,
            None,
            None,
            None,
            opened_source_commit,
            opened_target_commit,
            timestamp,
        );
        web_ui.pull_requests.push(pull_request.clone());
        self.save_workspace_with_web_ui(&workspace, web_ui)?;
        Ok((workspace, pull_request))
    }

    pub async fn merge_pull_request(
        &self,
        path: PathBuf,
        fallback_branch: &str,
        pull_request_id: &str,
        actor: IssueActorInput,
    ) -> Result<(WorkspaceSpec, PullRequestRecord), DomainError> {
        let initial_workspace = self.existing_workspace(path.clone(), fallback_branch)?;
        let _lock = self.lock_resolved_workspace(&initial_workspace)?;
        let workspace = self.existing_workspace(path, fallback_branch)?;
        let mut web_ui = self.load_web_ui_state(&workspace)?;
        let Some(index) = web_ui
            .pull_requests
            .iter()
            .position(|pull_request| pull_request.id == pull_request_id)
        else {
            return Err(DomainError::Repository(RepositoryError::RefNotFound(
                pull_request_id.to_string(),
            )));
        };
        if web_ui.pull_requests[index].status != PullRequestStatus::Open {
            return Ok((workspace, web_ui.pull_requests[index].clone()));
        }

        let protection = Self::branch_protection(
            &web_ui.repository,
            &web_ui.pull_requests[index].target_branch,
        );
        let branches = self
            .repo_store
            .list_branches(&workspace)
            .await
            .map_err(DomainError::Repository)?
            .into_iter()
            .map(|record| BranchInfo {
                is_current: record.name == workspace.checked_out_branch,
                is_served: record.name == workspace.exported_branch,
                commit: record.commit,
                summary: record.summary,
                name: record.name,
            })
            .collect::<Vec<_>>();
        let current_source_commit =
            Self::branch_head_commit(&branches, &web_ui.pull_requests[index].source_branch);
        if protection.required_approvals > 0 {
            let summary = Self::pull_request_review_summary_for_source(
                &web_ui.pull_requests[index],
                current_source_commit.as_deref(),
                protection.dismiss_stale_approvals,
            );
            if summary.changes_requested > 0 {
                return Err(DomainError::BranchRuleViolation(format!(
                    "`{}` still has requested changes",
                    web_ui.pull_requests[index].target_branch
                )));
            }
            if summary.approvals < protection.required_approvals as usize {
                return Err(DomainError::BranchRuleViolation(format!(
                    "`{}` requires {} approval(s) before merge",
                    web_ui.pull_requests[index].target_branch, protection.required_approvals
                )));
            }
        }

        let merged_commit = self
            .repo_store
            .merge_branch(
                &workspace,
                &web_ui.pull_requests[index].source_branch,
                &web_ui.pull_requests[index].target_branch,
            )
            .await
            .map_err(DomainError::Repository)?;

        let merge_actor = Self::normalize_issue_actor(actor)?;
        let timestamp = Self::now_ms();
        web_ui.pull_requests[index].status = PullRequestStatus::Merged;
        web_ui.pull_requests[index].updated_at_ms = timestamp;
        web_ui.pull_requests[index].merged_commit = Some(merged_commit);
        Self::push_activity(
            &mut web_ui.pull_requests[index],
            PullRequestActivityKind::Merged,
            merge_actor.role.clone(),
            Some(merge_actor.display_name.clone()),
            None,
            None,
            None,
            None,
            None,
            None,
            timestamp,
        );
        let merged_pull_request = web_ui.pull_requests[index].clone();
        let closing_issue_ids = Self::linked_issues_for_pull_request(
            &merged_pull_request,
            &web_ui.issues,
            &web_ui.pull_requests,
        )
        .into_iter()
        .filter(|link| link.relation == IssueLinkRelation::Closing)
        .map(|link| link.issue_id)
        .collect::<Vec<_>>();
        for issue_id in closing_issue_ids {
            let Ok(issue_index) = Self::find_issue_index(&web_ui.issues, &issue_id) else {
                continue;
            };
            let issue = &mut web_ui.issues[issue_index];
            if issue.status == IssueStatus::Closed {
                continue;
            }
            issue.status = IssueStatus::Closed;
            issue.closed_at_ms = Some(timestamp);
            issue.updated_at_ms = timestamp;
            Self::push_issue_timeline_event(
                issue,
                IssueTimelineEventKind::Closed,
                merge_actor.clone(),
                None,
                None,
                None,
                Vec::new(),
                Vec::new(),
                None,
                Some(merged_pull_request.id.clone()),
                None,
                Some(IssueTimelineTarget::Issue),
                None,
                timestamp,
            );
        }
        self.save_workspace_with_web_ui(&workspace, web_ui)?;
        Ok((workspace, merged_pull_request))
    }

    pub async fn update_pull_request(
        &self,
        path: PathBuf,
        fallback_branch: &str,
        pull_request_id: &str,
        update: UpdatePullRequest,
        actor_role: UiRole,
    ) -> Result<(WorkspaceSpec, PullRequestRecord), DomainError> {
        let initial_workspace = self.existing_workspace(path.clone(), fallback_branch)?;
        let _lock = self.lock_resolved_workspace(&initial_workspace)?;
        let workspace = self.existing_workspace(path, fallback_branch)?;
        let mut web_ui = self.load_web_ui_state(&workspace)?;
        let Some(index) = web_ui
            .pull_requests
            .iter()
            .position(|pull_request| pull_request.id == pull_request_id)
        else {
            return Err(DomainError::Repository(RepositoryError::RefNotFound(
                pull_request_id.to_string(),
            )));
        };

        let mut changed = false;
        let timestamp = Self::now_ms();
        let pull_request = &mut web_ui.pull_requests[index];
        if pull_request.status == PullRequestStatus::Merged && update.status.is_some() {
            return Err(DomainError::InvalidPullRequest(
                "merged pull requests cannot be reopened or closed".into(),
            ));
        }
        if let Some(title) = update.title {
            let title = Self::normalize_required(&title, "pull request title")?;
            if title != pull_request.title {
                pull_request.title = title;
                changed = true;
            }
        }
        if let Some(description) = update.description {
            let description = description.trim().to_string();
            if description != pull_request.description {
                pull_request.description = description;
                changed = true;
            }
        }
        if changed {
            pull_request.updated_at_ms = timestamp;
            let next_title = pull_request.title.clone();
            let next_description = pull_request.description.clone();
            Self::push_activity(
                pull_request,
                PullRequestActivityKind::Edited,
                actor_role.clone(),
                None,
                None,
                None,
                Some(next_title),
                Some(next_description),
                None,
                None,
                timestamp,
            );
        }

        if let Some(status) = update.status {
            let current_status = pull_request.status.clone();
            match (&current_status, status.clone()) {
                (PullRequestStatus::Open, PullRequestStatus::Closed) => {
                    pull_request.status = PullRequestStatus::Closed;
                    pull_request.updated_at_ms = timestamp;
                    Self::push_activity(
                        pull_request,
                        PullRequestActivityKind::Closed,
                        actor_role,
                        None,
                        None,
                        None,
                        None,
                        None,
                        None,
                        None,
                        timestamp,
                    );
                }
                (PullRequestStatus::Closed, PullRequestStatus::Open) => {
                    pull_request.status = PullRequestStatus::Open;
                    pull_request.updated_at_ms = timestamp;
                    Self::push_activity(
                        pull_request,
                        PullRequestActivityKind::Reopened,
                        actor_role,
                        None,
                        None,
                        None,
                        None,
                        None,
                        None,
                        None,
                        timestamp,
                    );
                }
                (PullRequestStatus::Merged, PullRequestStatus::Merged)
                | (PullRequestStatus::Open, PullRequestStatus::Open)
                | (PullRequestStatus::Closed, PullRequestStatus::Closed) => {}
                (_, PullRequestStatus::Merged) => {
                    return Err(DomainError::InvalidPullRequest(
                        "pull requests must be merged through the merge action".into(),
                    ));
                }
                _ => {
                    return Err(DomainError::InvalidPullRequest(format!(
                        "cannot transition pull request from {:?} to {:?}",
                        current_status, status
                    )));
                }
            }
        }

        let pull_request = web_ui.pull_requests[index].clone();
        self.save_workspace_with_web_ui(&workspace, web_ui)?;
        Ok((workspace, pull_request))
    }

    pub async fn comment_pull_request(
        &self,
        path: PathBuf,
        fallback_branch: &str,
        pull_request_id: &str,
        comment: CreatePullRequestComment,
        actor_role: UiRole,
    ) -> Result<(WorkspaceSpec, PullRequestRecord), DomainError> {
        let initial_workspace = self.existing_workspace(path.clone(), fallback_branch)?;
        let _lock = self.lock_resolved_workspace(&initial_workspace)?;
        let workspace = self.existing_workspace(path, fallback_branch)?;
        let mut web_ui = self.load_web_ui_state(&workspace)?;
        let Some(index) = web_ui
            .pull_requests
            .iter()
            .position(|pull_request| pull_request.id == pull_request_id)
        else {
            return Err(DomainError::Repository(RepositoryError::RefNotFound(
                pull_request_id.to_string(),
            )));
        };

        let display_name = Self::normalize_required(&comment.display_name, "display name")?;
        let body = Self::normalize_required(&comment.body, "comment body")?;
        let timestamp = Self::now_ms();
        web_ui.pull_requests[index].updated_at_ms = timestamp;
        Self::push_activity(
            &mut web_ui.pull_requests[index],
            PullRequestActivityKind::Commented,
            actor_role,
            Some(display_name),
            Some(body),
            None,
            None,
            None,
            None,
            None,
            timestamp,
        );
        let pull_request = web_ui.pull_requests[index].clone();
        self.save_workspace_with_web_ui(&workspace, web_ui)?;
        Ok((workspace, pull_request))
    }

    pub async fn review_pull_request(
        &self,
        path: PathBuf,
        fallback_branch: &str,
        pull_request_id: &str,
        review: CreatePullRequestReview,
        actor_role: UiRole,
    ) -> Result<(WorkspaceSpec, PullRequestRecord), DomainError> {
        let initial_workspace = self.existing_workspace(path.clone(), fallback_branch)?;
        let _lock = self.lock_resolved_workspace(&initial_workspace)?;
        let workspace = self.existing_workspace(path, fallback_branch)?;
        let mut web_ui = self.load_web_ui_state(&workspace)?;
        let Some(index) = web_ui
            .pull_requests
            .iter()
            .position(|pull_request| pull_request.id == pull_request_id)
        else {
            return Err(DomainError::Repository(RepositoryError::RefNotFound(
                pull_request_id.to_string(),
            )));
        };

        let display_name = Self::normalize_required(&review.display_name, "display name")?;
        let body = review.body.trim().to_string();
        let branches = self
            .repo_store
            .list_branches(&workspace)
            .await
            .map_err(DomainError::Repository)?
            .into_iter()
            .map(|record| BranchInfo {
                is_current: record.name == workspace.checked_out_branch,
                is_served: record.name == workspace.exported_branch,
                commit: record.commit,
                summary: record.summary,
                name: record.name,
            })
            .collect::<Vec<_>>();
        let source_commit =
            Self::branch_head_commit(&branches, &web_ui.pull_requests[index].source_branch);
        let target_commit =
            Self::branch_head_commit(&branches, &web_ui.pull_requests[index].target_branch);
        let timestamp = Self::now_ms();
        web_ui.pull_requests[index].updated_at_ms = timestamp;
        Self::push_activity(
            &mut web_ui.pull_requests[index],
            PullRequestActivityKind::Reviewed,
            actor_role,
            Some(display_name),
            Some(body),
            Some(review.state),
            None,
            None,
            source_commit,
            target_commit,
            timestamp,
        );
        let pull_request = web_ui.pull_requests[index].clone();
        self.save_workspace_with_web_ui(&workspace, web_ui)?;
        Ok((workspace, pull_request))
    }

    pub async fn delete_pull_request(
        &self,
        path: PathBuf,
        fallback_branch: &str,
        pull_request_id: &str,
    ) -> Result<(WorkspaceSpec, PullRequestRecord), DomainError> {
        let initial_workspace = self.existing_workspace(path.clone(), fallback_branch)?;
        let _lock = self.lock_resolved_workspace(&initial_workspace)?;
        let workspace = self.existing_workspace(path, fallback_branch)?;
        let mut web_ui = self.load_web_ui_state(&workspace)?;
        let Some(index) = web_ui
            .pull_requests
            .iter()
            .position(|pull_request| pull_request.id == pull_request_id)
        else {
            return Err(DomainError::Repository(RepositoryError::RefNotFound(
                pull_request_id.to_string(),
            )));
        };
        let pull_request = web_ui.pull_requests.remove(index);
        self.save_workspace_with_web_ui(&workspace, web_ui)?;
        Ok((workspace, pull_request))
    }

    pub async fn create_issue(
        &self,
        path: PathBuf,
        fallback_branch: &str,
        draft: CreateIssue,
        actor: IssueActorInput,
    ) -> Result<(WorkspaceSpec, IssueRecord), DomainError> {
        let initial_workspace = self.existing_workspace(path.clone(), fallback_branch)?;
        let _lock = self.lock_resolved_workspace(&initial_workspace)?;
        let workspace = self.existing_workspace(path, fallback_branch)?;
        let mut web_ui = self.load_web_ui_state(&workspace)?;
        web_ui.issue_settings = Self::normalize_issue_settings(web_ui.issue_settings)?;
        let author = Self::normalize_issue_actor(actor)?;
        let title = Self::normalize_issue_required(&draft.title, "issue title")?;
        let description = Self::normalize_optional(&draft.description);
        let timestamp = Self::now_ms();
        let label_ids = Self::validate_issue_label_ids(&web_ui.issue_settings, &draft.label_ids)?;
        let assignee_user_ids =
            Self::validate_issue_assignees(&web_ui.auth, &draft.assignee_user_ids)?;
        let milestone_id =
            Self::validate_issue_milestone_id(&web_ui.issue_settings, draft.milestone_id)?;
        let linked_pull_requests = Self::validate_linked_pull_request_ids(
            &web_ui.pull_requests,
            &draft.linked_pull_request_ids,
            timestamp,
        )?;
        let number = web_ui.issue_settings.next_issue_number;
        web_ui.issue_settings.next_issue_number =
            web_ui.issue_settings.next_issue_number.saturating_add(1);

        let mut issue = IssueRecord {
            id: Uuid::new_v4().to_string(),
            number,
            title: title.clone(),
            description: description.clone(),
            status: IssueStatus::Open,
            author: author.clone(),
            created_at_ms: timestamp,
            updated_at_ms: timestamp,
            closed_at_ms: None,
            label_ids: label_ids.clone(),
            assignee_user_ids: assignee_user_ids.clone(),
            milestone_id: milestone_id.clone(),
            linked_pull_requests: linked_pull_requests.clone(),
            reactions: Vec::new(),
            comments: Vec::new(),
            timeline: Vec::new(),
        };
        Self::push_issue_timeline_event(
            &mut issue,
            IssueTimelineEventKind::Opened,
            author.clone(),
            None,
            Some(title),
            Some(description),
            label_ids,
            assignee_user_ids,
            milestone_id,
            None,
            None,
            Some(IssueTimelineTarget::Issue),
            None,
            timestamp,
        );
        for linked in linked_pull_requests {
            Self::push_issue_timeline_event(
                &mut issue,
                IssueTimelineEventKind::PullRequestLinked,
                author.clone(),
                None,
                None,
                None,
                Vec::new(),
                Vec::new(),
                None,
                Some(linked.pull_request_id),
                None,
                Some(IssueTimelineTarget::Issue),
                None,
                timestamp,
            );
        }
        web_ui.issues.insert(0, issue.clone());
        self.save_workspace_with_web_ui(&workspace, web_ui)?;
        Ok((workspace, issue))
    }

    pub async fn update_issue(
        &self,
        path: PathBuf,
        fallback_branch: &str,
        issue_id: &str,
        update: UpdateIssue,
        actor: IssueActorInput,
    ) -> Result<(WorkspaceSpec, IssueRecord), DomainError> {
        let initial_workspace = self.existing_workspace(path.clone(), fallback_branch)?;
        let _lock = self.lock_resolved_workspace(&initial_workspace)?;
        let workspace = self.existing_workspace(path, fallback_branch)?;
        let mut web_ui = self.load_web_ui_state(&workspace)?;
        let actor = Self::normalize_issue_actor(actor)?;
        let issue_index = Self::find_issue_index(&web_ui.issues, issue_id)?;
        if !Self::can_manage_issue(&actor, &web_ui.issues[issue_index]) {
            return Err(DomainError::InvalidIssue(
                "only issue authors or owners can edit this issue".into(),
            ));
        }

        let timestamp = Self::now_ms();
        let issue = &mut web_ui.issues[issue_index];
        let mut edited = false;

        if let Some(title) = update.title {
            let title = Self::normalize_issue_required(&title, "issue title")?;
            if title != issue.title {
                issue.title = title;
                edited = true;
            }
        }
        if let Some(description) = update.description {
            let description = Self::normalize_optional(&description);
            if description != issue.description {
                issue.description = description;
                edited = true;
            }
        }
        if edited {
            issue.updated_at_ms = timestamp;
            Self::push_issue_timeline_event(
                issue,
                IssueTimelineEventKind::Edited,
                actor.clone(),
                None,
                Some(issue.title.clone()),
                Some(issue.description.clone()),
                Vec::new(),
                Vec::new(),
                None,
                None,
                None,
                Some(IssueTimelineTarget::Issue),
                None,
                timestamp,
            );
        }
        if let Some(status) = update.status {
            match (&issue.status, status) {
                (IssueStatus::Open, IssueStatus::Closed) => {
                    issue.status = IssueStatus::Closed;
                    issue.closed_at_ms = Some(timestamp);
                    issue.updated_at_ms = timestamp;
                    Self::push_issue_timeline_event(
                        issue,
                        IssueTimelineEventKind::Closed,
                        actor,
                        None,
                        None,
                        None,
                        Vec::new(),
                        Vec::new(),
                        None,
                        None,
                        None,
                        Some(IssueTimelineTarget::Issue),
                        None,
                        timestamp,
                    );
                }
                (IssueStatus::Closed, IssueStatus::Open) => {
                    issue.status = IssueStatus::Open;
                    issue.closed_at_ms = None;
                    issue.updated_at_ms = timestamp;
                    Self::push_issue_timeline_event(
                        issue,
                        IssueTimelineEventKind::Reopened,
                        actor,
                        None,
                        None,
                        None,
                        Vec::new(),
                        Vec::new(),
                        None,
                        None,
                        None,
                        Some(IssueTimelineTarget::Issue),
                        None,
                        timestamp,
                    );
                }
                (IssueStatus::Open, IssueStatus::Open)
                | (IssueStatus::Closed, IssueStatus::Closed) => {}
            }
        }

        let issue = web_ui.issues[issue_index].clone();
        self.save_workspace_with_web_ui(&workspace, web_ui)?;
        Ok((workspace, issue))
    }

    pub async fn delete_issue(
        &self,
        path: PathBuf,
        fallback_branch: &str,
        issue_id: &str,
    ) -> Result<(WorkspaceSpec, IssueRecord), DomainError> {
        let initial_workspace = self.existing_workspace(path.clone(), fallback_branch)?;
        let _lock = self.lock_resolved_workspace(&initial_workspace)?;
        let workspace = self.existing_workspace(path, fallback_branch)?;
        let mut web_ui = self.load_web_ui_state(&workspace)?;
        let issue_index = Self::find_issue_index(&web_ui.issues, issue_id)?;
        let issue = web_ui.issues.remove(issue_index);
        self.save_workspace_with_web_ui(&workspace, web_ui)?;
        Ok((workspace, issue))
    }

    pub async fn comment_issue(
        &self,
        path: PathBuf,
        fallback_branch: &str,
        issue_id: &str,
        comment: CreateIssueComment,
        actor: IssueActorInput,
    ) -> Result<(WorkspaceSpec, IssueRecord), DomainError> {
        let initial_workspace = self.existing_workspace(path.clone(), fallback_branch)?;
        let _lock = self.lock_resolved_workspace(&initial_workspace)?;
        let workspace = self.existing_workspace(path, fallback_branch)?;
        let mut web_ui = self.load_web_ui_state(&workspace)?;
        let issue_index = Self::find_issue_index(&web_ui.issues, issue_id)?;
        let actor = Self::normalize_issue_actor(IssueActorInput {
            display_name: comment.display_name,
            ..actor
        })?;
        let body = Self::normalize_issue_required(&comment.body, "comment body")?;
        let timestamp = Self::now_ms();
        let issue = &mut web_ui.issues[issue_index];
        let comment_id = Uuid::new_v4().to_string();
        issue.comments.push(IssueCommentRecord {
            id: comment_id.clone(),
            actor: actor.clone(),
            body: body.clone(),
            created_at_ms: timestamp,
            updated_at_ms: timestamp,
            reactions: Vec::new(),
        });
        issue.updated_at_ms = timestamp;
        Self::push_issue_timeline_event(
            issue,
            IssueTimelineEventKind::Commented,
            actor,
            Some(body),
            None,
            None,
            Vec::new(),
            Vec::new(),
            None,
            None,
            None,
            Some(IssueTimelineTarget::Comment),
            Some(comment_id),
            timestamp,
        );
        let issue = web_ui.issues[issue_index].clone();
        self.save_workspace_with_web_ui(&workspace, web_ui)?;
        Ok((workspace, issue))
    }

    pub fn upsert_issue_label(
        &self,
        path: PathBuf,
        fallback_branch: &str,
        label: UpsertIssueLabel,
    ) -> Result<(WorkspaceSpec, IssueLabel), DomainError> {
        let initial_workspace = self.existing_workspace(path.clone(), fallback_branch)?;
        let _lock = self.lock_resolved_workspace(&initial_workspace)?;
        let workspace = self.existing_workspace(path, fallback_branch)?;
        let mut web_ui = self.load_web_ui_state(&workspace)?;
        web_ui.issue_settings = Self::normalize_issue_settings(web_ui.issue_settings)?;
        let timestamp = Self::now_ms();
        let mut normalized =
            Self::normalize_issue_label(label.clone(), label.id.as_deref(), timestamp)?;
        if web_ui.issue_settings.labels.iter().any(|existing| {
            existing.id != normalized.id && existing.name.eq_ignore_ascii_case(&normalized.name)
        }) {
            return Err(DomainError::InvalidIssue(format!(
                "issue label `{}` already exists",
                normalized.name
            )));
        }
        if let Some(existing) = web_ui
            .issue_settings
            .labels
            .iter_mut()
            .find(|existing| existing.id == normalized.id)
        {
            normalized.created_at_ms = existing.created_at_ms;
            *existing = normalized.clone();
        } else {
            web_ui.issue_settings.labels.push(normalized.clone());
        }
        web_ui.issue_settings = Self::normalize_issue_settings(web_ui.issue_settings)?;
        self.save_workspace_with_web_ui(&workspace, web_ui)?;
        Ok((workspace, normalized))
    }

    pub fn delete_issue_label(
        &self,
        path: PathBuf,
        fallback_branch: &str,
        label_id: &str,
        actor: IssueActorInput,
    ) -> Result<(WorkspaceSpec, IssueLabel), DomainError> {
        let initial_workspace = self.existing_workspace(path.clone(), fallback_branch)?;
        let _lock = self.lock_resolved_workspace(&initial_workspace)?;
        let workspace = self.existing_workspace(path, fallback_branch)?;
        let mut web_ui = self.load_web_ui_state(&workspace)?;
        let actor = Self::normalize_issue_actor(actor)?;
        let label_index = web_ui
            .issue_settings
            .labels
            .iter()
            .position(|label| label.id == label_id)
            .ok_or_else(|| DomainError::IssueLabelNotFound(label_id.to_string()))?;
        let deleted = web_ui.issue_settings.labels.remove(label_index);
        let timestamp = Self::now_ms();
        for issue in web_ui
            .issues
            .iter_mut()
            .filter(|issue| issue.label_ids.iter().any(|existing| existing == label_id))
        {
            issue.label_ids.retain(|existing| existing != label_id);
            issue.updated_at_ms = timestamp;
            Self::push_issue_timeline_event(
                issue,
                IssueTimelineEventKind::LabelsChanged,
                actor.clone(),
                None,
                None,
                None,
                issue.label_ids.clone(),
                Vec::new(),
                None,
                None,
                None,
                Some(IssueTimelineTarget::Issue),
                None,
                timestamp,
            );
        }
        self.save_workspace_with_web_ui(&workspace, web_ui)?;
        Ok((workspace, deleted))
    }

    pub fn upsert_issue_milestone(
        &self,
        path: PathBuf,
        fallback_branch: &str,
        milestone: UpsertIssueMilestone,
    ) -> Result<(WorkspaceSpec, IssueMilestone), DomainError> {
        let initial_workspace = self.existing_workspace(path.clone(), fallback_branch)?;
        let _lock = self.lock_resolved_workspace(&initial_workspace)?;
        let workspace = self.existing_workspace(path, fallback_branch)?;
        let mut web_ui = self.load_web_ui_state(&workspace)?;
        web_ui.issue_settings = Self::normalize_issue_settings(web_ui.issue_settings)?;
        let timestamp = Self::now_ms();
        let mut normalized =
            Self::normalize_issue_milestone(milestone.clone(), milestone.id.as_deref(), timestamp)?;
        if web_ui.issue_settings.milestones.iter().any(|existing| {
            existing.id != normalized.id && existing.title.eq_ignore_ascii_case(&normalized.title)
        }) {
            return Err(DomainError::InvalidIssue(format!(
                "issue milestone `{}` already exists",
                normalized.title
            )));
        }
        if let Some(existing) = web_ui
            .issue_settings
            .milestones
            .iter_mut()
            .find(|existing| existing.id == normalized.id)
        {
            normalized.created_at_ms = existing.created_at_ms;
            *existing = normalized.clone();
        } else {
            web_ui.issue_settings.milestones.push(normalized.clone());
        }
        web_ui.issue_settings = Self::normalize_issue_settings(web_ui.issue_settings)?;
        self.save_workspace_with_web_ui(&workspace, web_ui)?;
        Ok((workspace, normalized))
    }

    pub fn delete_issue_milestone(
        &self,
        path: PathBuf,
        fallback_branch: &str,
        milestone_id: &str,
        actor: IssueActorInput,
    ) -> Result<(WorkspaceSpec, IssueMilestone), DomainError> {
        let initial_workspace = self.existing_workspace(path.clone(), fallback_branch)?;
        let _lock = self.lock_resolved_workspace(&initial_workspace)?;
        let workspace = self.existing_workspace(path, fallback_branch)?;
        let mut web_ui = self.load_web_ui_state(&workspace)?;
        let actor = Self::normalize_issue_actor(actor)?;
        let milestone_index = web_ui
            .issue_settings
            .milestones
            .iter()
            .position(|milestone| milestone.id == milestone_id)
            .ok_or_else(|| DomainError::IssueMilestoneNotFound(milestone_id.to_string()))?;
        let deleted = web_ui.issue_settings.milestones.remove(milestone_index);
        let timestamp = Self::now_ms();
        for issue in web_ui
            .issues
            .iter_mut()
            .filter(|issue| issue.milestone_id.as_deref() == Some(milestone_id))
        {
            issue.milestone_id = None;
            issue.updated_at_ms = timestamp;
            Self::push_issue_timeline_event(
                issue,
                IssueTimelineEventKind::MilestoneChanged,
                actor.clone(),
                None,
                None,
                None,
                Vec::new(),
                Vec::new(),
                None,
                None,
                None,
                Some(IssueTimelineTarget::Issue),
                None,
                timestamp,
            );
        }
        self.save_workspace_with_web_ui(&workspace, web_ui)?;
        Ok((workspace, deleted))
    }

    pub async fn set_issue_labels(
        &self,
        path: PathBuf,
        fallback_branch: &str,
        issue_id: &str,
        label_ids: Vec<String>,
        actor: IssueActorInput,
    ) -> Result<(WorkspaceSpec, IssueRecord), DomainError> {
        let initial_workspace = self.existing_workspace(path.clone(), fallback_branch)?;
        let _lock = self.lock_resolved_workspace(&initial_workspace)?;
        let workspace = self.existing_workspace(path, fallback_branch)?;
        let mut web_ui = self.load_web_ui_state(&workspace)?;
        let actor = Self::normalize_issue_actor(actor)?;
        let issue_index = Self::find_issue_index(&web_ui.issues, issue_id)?;
        let label_ids = Self::validate_issue_label_ids(&web_ui.issue_settings, &label_ids)?;
        if web_ui.issues[issue_index].label_ids != label_ids {
            let timestamp = Self::now_ms();
            let issue = &mut web_ui.issues[issue_index];
            issue.label_ids = label_ids.clone();
            issue.updated_at_ms = timestamp;
            Self::push_issue_timeline_event(
                issue,
                IssueTimelineEventKind::LabelsChanged,
                actor,
                None,
                None,
                None,
                label_ids,
                Vec::new(),
                None,
                None,
                None,
                Some(IssueTimelineTarget::Issue),
                None,
                timestamp,
            );
        }
        let issue = web_ui.issues[issue_index].clone();
        self.save_workspace_with_web_ui(&workspace, web_ui)?;
        Ok((workspace, issue))
    }

    pub async fn set_issue_assignees(
        &self,
        path: PathBuf,
        fallback_branch: &str,
        issue_id: &str,
        assignee_user_ids: Vec<String>,
        actor: IssueActorInput,
    ) -> Result<(WorkspaceSpec, IssueRecord), DomainError> {
        let initial_workspace = self.existing_workspace(path.clone(), fallback_branch)?;
        let _lock = self.lock_resolved_workspace(&initial_workspace)?;
        let workspace = self.existing_workspace(path, fallback_branch)?;
        let mut web_ui = self.load_web_ui_state(&workspace)?;
        let actor = Self::normalize_issue_actor(actor)?;
        let issue_index = Self::find_issue_index(&web_ui.issues, issue_id)?;
        let assignee_user_ids = Self::validate_issue_assignees(&web_ui.auth, &assignee_user_ids)?;
        if web_ui.issues[issue_index].assignee_user_ids != assignee_user_ids {
            let timestamp = Self::now_ms();
            let issue = &mut web_ui.issues[issue_index];
            issue.assignee_user_ids = assignee_user_ids.clone();
            issue.updated_at_ms = timestamp;
            Self::push_issue_timeline_event(
                issue,
                IssueTimelineEventKind::AssigneesChanged,
                actor,
                None,
                None,
                None,
                Vec::new(),
                assignee_user_ids,
                None,
                None,
                None,
                Some(IssueTimelineTarget::Issue),
                None,
                timestamp,
            );
        }
        let issue = web_ui.issues[issue_index].clone();
        self.save_workspace_with_web_ui(&workspace, web_ui)?;
        Ok((workspace, issue))
    }

    pub async fn set_issue_milestone(
        &self,
        path: PathBuf,
        fallback_branch: &str,
        issue_id: &str,
        milestone_id: Option<String>,
        actor: IssueActorInput,
    ) -> Result<(WorkspaceSpec, IssueRecord), DomainError> {
        let initial_workspace = self.existing_workspace(path.clone(), fallback_branch)?;
        let _lock = self.lock_resolved_workspace(&initial_workspace)?;
        let workspace = self.existing_workspace(path, fallback_branch)?;
        let mut web_ui = self.load_web_ui_state(&workspace)?;
        let actor = Self::normalize_issue_actor(actor)?;
        let issue_index = Self::find_issue_index(&web_ui.issues, issue_id)?;
        let milestone_id = Self::validate_issue_milestone_id(&web_ui.issue_settings, milestone_id)?;
        if web_ui.issues[issue_index].milestone_id != milestone_id {
            let timestamp = Self::now_ms();
            let issue = &mut web_ui.issues[issue_index];
            issue.milestone_id = milestone_id.clone();
            issue.updated_at_ms = timestamp;
            Self::push_issue_timeline_event(
                issue,
                IssueTimelineEventKind::MilestoneChanged,
                actor,
                None,
                None,
                None,
                Vec::new(),
                Vec::new(),
                milestone_id,
                None,
                None,
                Some(IssueTimelineTarget::Issue),
                None,
                timestamp,
            );
        }
        let issue = web_ui.issues[issue_index].clone();
        self.save_workspace_with_web_ui(&workspace, web_ui)?;
        Ok((workspace, issue))
    }

    pub async fn link_issue_pull_request(
        &self,
        path: PathBuf,
        fallback_branch: &str,
        issue_id: &str,
        pull_request_id: &str,
        actor: IssueActorInput,
    ) -> Result<(WorkspaceSpec, IssueRecord), DomainError> {
        let initial_workspace = self.existing_workspace(path.clone(), fallback_branch)?;
        let _lock = self.lock_resolved_workspace(&initial_workspace)?;
        let workspace = self.existing_workspace(path, fallback_branch)?;
        let mut web_ui = self.load_web_ui_state(&workspace)?;
        if !web_ui
            .pull_requests
            .iter()
            .any(|pull_request| pull_request.id == pull_request_id)
        {
            return Err(DomainError::InvalidIssue(format!(
                "linked pull request `{}` was not found",
                pull_request_id
            )));
        }
        let actor = Self::normalize_issue_actor(actor)?;
        let issue_index = Self::find_issue_index(&web_ui.issues, issue_id)?;
        if !web_ui.issues[issue_index]
            .linked_pull_requests
            .iter()
            .any(|linked| linked.pull_request_id == pull_request_id)
        {
            let timestamp = Self::now_ms();
            let issue = &mut web_ui.issues[issue_index];
            issue.linked_pull_requests.push(IssueLinkedPullRequest {
                pull_request_id: pull_request_id.to_string(),
                relation: IssueLinkRelation::Related,
                source: IssueLinkSource::Manual,
                linked_at_ms: timestamp,
            });
            issue.updated_at_ms = timestamp;
            Self::push_issue_timeline_event(
                issue,
                IssueTimelineEventKind::PullRequestLinked,
                actor,
                None,
                None,
                None,
                Vec::new(),
                Vec::new(),
                None,
                Some(pull_request_id.to_string()),
                None,
                Some(IssueTimelineTarget::Issue),
                None,
                timestamp,
            );
        }
        let issue = web_ui.issues[issue_index].clone();
        self.save_workspace_with_web_ui(&workspace, web_ui)?;
        Ok((workspace, issue))
    }

    pub async fn unlink_issue_pull_request(
        &self,
        path: PathBuf,
        fallback_branch: &str,
        issue_id: &str,
        pull_request_id: &str,
        actor: IssueActorInput,
    ) -> Result<(WorkspaceSpec, IssueRecord), DomainError> {
        let initial_workspace = self.existing_workspace(path.clone(), fallback_branch)?;
        let _lock = self.lock_resolved_workspace(&initial_workspace)?;
        let workspace = self.existing_workspace(path, fallback_branch)?;
        let mut web_ui = self.load_web_ui_state(&workspace)?;
        let actor = Self::normalize_issue_actor(actor)?;
        let issue_index = Self::find_issue_index(&web_ui.issues, issue_id)?;
        if web_ui.issues[issue_index]
            .linked_pull_requests
            .iter()
            .any(|linked| linked.pull_request_id == pull_request_id)
        {
            let timestamp = Self::now_ms();
            let issue = &mut web_ui.issues[issue_index];
            issue
                .linked_pull_requests
                .retain(|linked| linked.pull_request_id != pull_request_id);
            issue.updated_at_ms = timestamp;
            Self::push_issue_timeline_event(
                issue,
                IssueTimelineEventKind::PullRequestUnlinked,
                actor,
                None,
                None,
                None,
                Vec::new(),
                Vec::new(),
                None,
                Some(pull_request_id.to_string()),
                None,
                Some(IssueTimelineTarget::Issue),
                None,
                timestamp,
            );
        }
        let issue = web_ui.issues[issue_index].clone();
        self.save_workspace_with_web_ui(&workspace, web_ui)?;
        Ok((workspace, issue))
    }

    pub async fn toggle_issue_reaction(
        &self,
        path: PathBuf,
        fallback_branch: &str,
        issue_id: &str,
        reaction: IssueReactionContent,
        actor: IssueActorInput,
    ) -> Result<(WorkspaceSpec, IssueRecord), DomainError> {
        let initial_workspace = self.existing_workspace(path.clone(), fallback_branch)?;
        let _lock = self.lock_resolved_workspace(&initial_workspace)?;
        let workspace = self.existing_workspace(path, fallback_branch)?;
        let mut web_ui = self.load_web_ui_state(&workspace)?;
        let actor = Self::normalize_issue_actor(actor)?;
        let actor_key = Self::interaction_issue_actor_key(&actor);
        let issue_index = Self::find_issue_index(&web_ui.issues, issue_id)?;
        let timestamp = Self::now_ms();
        let issue = &mut web_ui.issues[issue_index];
        if let Some(existing_index) = issue.reactions.iter().position(|existing| {
            existing.content == reaction
                && Self::interaction_issue_actor_key(&existing.actor) == actor_key
        }) {
            issue.reactions.remove(existing_index);
        } else {
            issue.reactions.push(IssueReactionRecord {
                id: Uuid::new_v4().to_string(),
                content: reaction.clone(),
                actor: actor.clone(),
                created_at_ms: timestamp,
            });
        }
        issue.updated_at_ms = timestamp;
        Self::push_issue_timeline_event(
            issue,
            IssueTimelineEventKind::ReactionToggled,
            actor,
            None,
            None,
            None,
            Vec::new(),
            Vec::new(),
            None,
            None,
            Some(reaction),
            Some(IssueTimelineTarget::Issue),
            None,
            timestamp,
        );
        let issue = web_ui.issues[issue_index].clone();
        self.save_workspace_with_web_ui(&workspace, web_ui)?;
        Ok((workspace, issue))
    }

    pub async fn toggle_issue_comment_reaction(
        &self,
        path: PathBuf,
        fallback_branch: &str,
        issue_id: &str,
        comment_id: &str,
        reaction: IssueReactionContent,
        actor: IssueActorInput,
    ) -> Result<(WorkspaceSpec, IssueRecord), DomainError> {
        let initial_workspace = self.existing_workspace(path.clone(), fallback_branch)?;
        let _lock = self.lock_resolved_workspace(&initial_workspace)?;
        let workspace = self.existing_workspace(path, fallback_branch)?;
        let mut web_ui = self.load_web_ui_state(&workspace)?;
        let actor = Self::normalize_issue_actor(actor)?;
        let actor_key = Self::interaction_issue_actor_key(&actor);
        let issue_index = Self::find_issue_index(&web_ui.issues, issue_id)?;
        let timestamp = Self::now_ms();
        let issue = &mut web_ui.issues[issue_index];
        let comment = issue
            .comments
            .iter_mut()
            .find(|comment| comment.id == comment_id)
            .ok_or_else(|| {
                DomainError::InvalidIssue(format!("comment `{comment_id}` was not found"))
            })?;
        if let Some(existing_index) = comment.reactions.iter().position(|existing| {
            existing.content == reaction
                && Self::interaction_issue_actor_key(&existing.actor) == actor_key
        }) {
            comment.reactions.remove(existing_index);
        } else {
            comment.reactions.push(IssueReactionRecord {
                id: Uuid::new_v4().to_string(),
                content: reaction.clone(),
                actor: actor.clone(),
                created_at_ms: timestamp,
            });
        }
        issue.updated_at_ms = timestamp;
        Self::push_issue_timeline_event(
            issue,
            IssueTimelineEventKind::ReactionToggled,
            actor,
            None,
            None,
            None,
            Vec::new(),
            Vec::new(),
            None,
            None,
            Some(reaction),
            Some(IssueTimelineTarget::Comment),
            Some(comment_id.to_string()),
            timestamp,
        );
        let issue = web_ui.issues[issue_index].clone();
        self.save_workspace_with_web_ui(&workspace, web_ui)?;
        Ok((workspace, issue))
    }

    pub async fn apply(
        &self,
        path: PathBuf,
        fallback_branch: &str,
        branch_override: Option<String>,
    ) -> Result<(WorkspaceSpec, ApplyOutcome), DomainError> {
        let initial_workspace = self.existing_workspace(path.clone(), fallback_branch)?;
        let _lock = self.lock_resolved_workspace(&initial_workspace)?;
        let workspace = self.existing_workspace(path, fallback_branch)?;
        let branch = branch_override.unwrap_or_else(|| workspace.exported_branch.clone());
        let source_ref = format!("refs/heads/{branch}");
        let mut apply_workspace = workspace.clone();
        // The checked-out branch names the tree currently materialized in the host folder.
        // Applying a different branch therefore changes the host branch state as well.
        apply_workspace.checked_out_branch = branch;
        let outcome = self
            .repo_store
            .apply_fast_forward(&apply_workspace, &source_ref)
            .await
            .map_err(DomainError::Repository)?;
        self.save_workspace(&apply_workspace)?;

        Ok((apply_workspace, outcome))
    }

    pub async fn list_branches(
        &self,
        path: PathBuf,
        fallback_branch: &str,
        patterns: &[String],
    ) -> Result<(WorkspaceSpec, Vec<BranchInfo>), DomainError> {
        let initial_workspace = self.existing_workspace(path.clone(), fallback_branch)?;
        let _lock = self.lock_resolved_workspace(&initial_workspace)?;
        let workspace = self.existing_workspace(path, fallback_branch)?;
        let branches = self
            .repo_store
            .list_branches(&workspace)
            .await
            .map_err(DomainError::Repository)?;
        let branches = branches
            .into_iter()
            .filter(|record| Self::matches_branch_patterns(&record.name, patterns))
            .map(|record| BranchInfo {
                is_current: record.name == workspace.checked_out_branch,
                is_served: record.name == workspace.exported_branch,
                commit: record.commit,
                summary: record.summary,
                name: record.name,
            })
            .collect();
        Ok((workspace, branches))
    }

    pub async fn create_branch(
        &self,
        path: PathBuf,
        fallback_branch: &str,
        name: &str,
        start_point: Option<&str>,
        force: bool,
    ) -> Result<(WorkspaceSpec, BranchCreateOutcome), DomainError> {
        let initial_workspace = self.existing_workspace(path.clone(), fallback_branch)?;
        let _lock = self.lock_resolved_workspace(&initial_workspace)?;
        let workspace = self.existing_workspace(path, fallback_branch)?;
        let commit = self
            .repo_store
            .create_branch(&workspace, name, start_point, force)
            .await
            .map_err(DomainError::Repository)?;
        Ok((
            workspace,
            BranchCreateOutcome {
                branch: name.to_string(),
                commit,
            },
        ))
    }

    pub async fn rename_branch(
        &self,
        path: PathBuf,
        fallback_branch: &str,
        old_name: &str,
        new_name: &str,
        force: bool,
    ) -> Result<WorkspaceSpec, DomainError> {
        let initial_workspace = self.existing_workspace(path.clone(), fallback_branch)?;
        let _lock = self.lock_resolved_workspace(&initial_workspace)?;
        let mut workspace = self.existing_workspace(path, fallback_branch)?;
        self.repo_store
            .rename_branch(&workspace, old_name, new_name, force)
            .await
            .map_err(DomainError::Repository)?;
        if workspace.exported_branch == old_name {
            workspace.exported_branch = new_name.to_string();
        }
        if workspace.checked_out_branch == old_name {
            workspace.checked_out_branch = new_name.to_string();
        }
        self.save_workspace(&workspace)?;
        Ok(workspace)
    }

    pub async fn delete_branch(
        &self,
        path: PathBuf,
        fallback_branch: &str,
        name: &str,
        force: bool,
    ) -> Result<WorkspaceSpec, DomainError> {
        let initial_workspace = self.existing_workspace(path.clone(), fallback_branch)?;
        let _lock = self.lock_resolved_workspace(&initial_workspace)?;
        let workspace = self.existing_workspace(path, fallback_branch)?;
        let web_ui = self.load_web_ui_state(&workspace)?;
        let protection = Self::branch_protection(&web_ui.repository, name);
        if protection.block_delete {
            return Err(DomainError::BranchRuleViolation(format!(
                "`{name}` cannot be deleted because it matches protected rule(s): {}",
                protection.patterns.join(", ")
            )));
        }
        self.repo_store
            .delete_branch(&workspace, name, force)
            .await
            .map_err(DomainError::Repository)?;
        Ok(workspace)
    }

    pub async fn switch_branch(
        &self,
        path: PathBuf,
        fallback_branch: &str,
        name: &str,
    ) -> Result<(WorkspaceSpec, BranchSwitchOutcome), DomainError> {
        let initial_workspace = self.existing_workspace(path.clone(), fallback_branch)?;
        let _lock = self.lock_resolved_workspace(&initial_workspace)?;
        let workspace = self.existing_workspace(path, fallback_branch)?;
        let web_ui = self.load_web_ui_state(&workspace)?;
        let _protection = Self::branch_protection(&web_ui.repository, name);
        let commit = self
            .repo_store
            .switch_branch(&workspace, name)
            .await
            .map_err(DomainError::Repository)?;
        let mut switched = workspace.clone();
        let previous_branch = switched.checked_out_branch.clone();
        switched.exported_branch = name.to_string();
        switched.checked_out_branch = name.to_string();
        self.save_workspace(&switched)?;
        Ok((
            switched,
            BranchSwitchOutcome {
                previous_branch,
                current_branch: name.to_string(),
                commit,
            },
        ))
    }

    pub async fn checkout_branch(
        &self,
        path: PathBuf,
        fallback_branch: &str,
        name: &str,
    ) -> Result<(WorkspaceSpec, BranchCheckoutOutcome), DomainError> {
        let initial_workspace = self.existing_workspace(path.clone(), fallback_branch)?;
        let _lock = self.lock_resolved_workspace(&initial_workspace)?;
        let workspace = self.existing_workspace(path, fallback_branch)?;
        let commit = self
            .repo_store
            .checkout_branch(&workspace, name, false)
            .await
            .map_err(DomainError::Repository)?;
        let mut checked_out = workspace.clone();
        let previous_branch = checked_out.checked_out_branch.clone();
        checked_out.checked_out_branch = name.to_string();
        self.save_workspace(&checked_out)?;
        Ok((
            checked_out,
            BranchCheckoutOutcome {
                previous_branch,
                current_branch: name.to_string(),
                commit,
            },
        ))
    }

    pub async fn create_and_checkout_branch(
        &self,
        path: PathBuf,
        fallback_branch: &str,
        name: &str,
        start_point: Option<&str>,
        create_force: bool,
        checkout_force: bool,
    ) -> Result<(WorkspaceSpec, BranchCheckoutOutcome), DomainError> {
        let initial_workspace = self.existing_workspace(path.clone(), fallback_branch)?;
        let _lock = self.lock_resolved_workspace(&initial_workspace)?;
        let workspace = self.existing_workspace(path, fallback_branch)?;
        self.repo_store
            .create_branch(&workspace, name, start_point, create_force)
            .await
            .map_err(DomainError::Repository)?;
        let commit = self
            .repo_store
            .checkout_branch(&workspace, name, checkout_force)
            .await
            .map_err(DomainError::Repository)?;
        let mut checked_out = workspace.clone();
        let previous_branch = checked_out.checked_out_branch.clone();
        checked_out.checked_out_branch = name.to_string();
        self.save_workspace(&checked_out)?;
        Ok((
            checked_out,
            BranchCheckoutOutcome {
                previous_branch,
                current_branch: name.to_string(),
                commit,
            },
        ))
    }

    pub async fn checkout_branch_with_force(
        &self,
        path: PathBuf,
        fallback_branch: &str,
        name: &str,
        force: bool,
    ) -> Result<(WorkspaceSpec, BranchCheckoutOutcome), DomainError> {
        let initial_workspace = self.existing_workspace(path.clone(), fallback_branch)?;
        let _lock = self.lock_resolved_workspace(&initial_workspace)?;
        let workspace = self.existing_workspace(path, fallback_branch)?;
        let commit = self
            .repo_store
            .checkout_branch(&workspace, name, force)
            .await
            .map_err(DomainError::Repository)?;
        let mut checked_out = workspace.clone();
        let previous_branch = checked_out.checked_out_branch.clone();
        checked_out.checked_out_branch = name.to_string();
        self.save_workspace(&checked_out)?;
        Ok((
            checked_out,
            BranchCheckoutOutcome {
                previous_branch,
                current_branch: name.to_string(),
                commit,
            },
        ))
    }
}

const PULL_REQUEST_HISTORY_LOOKBACK_LIMIT: usize = 120;

pub async fn resolve_branch_commit_at_time(
    repo_read_store: &dyn RepoReadStore,
    workspace: &WorkspaceSpec,
    branch: &str,
    timestamp_ms: u64,
) -> Option<String> {
    let cutoff = (timestamp_ms / 1000) as i64;
    let mut offset = 0;
    loop {
        let history = repo_read_store
            .list_commits(
                workspace,
                Some(branch),
                offset,
                PULL_REQUEST_HISTORY_LOOKBACK_LIMIT,
            )
            .await
            .ok()?;
        if history.commits.is_empty() {
            return None;
        }
        if let Some(commit) = history
            .commits
            .iter()
            .find(|commit| commit.authored_at <= cutoff)
        {
            return Some(commit.id.clone());
        }
        if !history.has_more {
            return None;
        }
        offset += history.commits.len();
    }
}

pub async fn resolve_pull_request_refs(
    repo_read_store: &dyn RepoReadStore,
    workspace: &WorkspaceSpec,
    pull_request: &PullRequestRecord,
) -> (String, String) {
    let base_ref = if let Some(target_commit) = &pull_request.target_commit {
        target_commit.clone()
    } else {
        resolve_branch_commit_at_time(
            repo_read_store,
            workspace,
            &pull_request.target_branch,
            pull_request.created_at_ms,
        )
        .await
        .unwrap_or_else(|| pull_request.target_branch.clone())
    };

    let head_ref = if let Some(source_commit) = &pull_request.source_commit {
        source_commit.clone()
    } else {
        resolve_branch_commit_at_time(
            repo_read_store,
            workspace,
            &pull_request.source_branch,
            pull_request.created_at_ms,
        )
        .await
        .or_else(|| pull_request.merged_commit.clone())
        .unwrap_or_else(|| pull_request.source_branch.clone())
    };

    (base_ref, head_ref)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::sync::Mutex;
    use tempfile::TempDir;

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
            Ok(Some("abc123".into()))
        }

        async fn apply_fast_forward(
            &self,
            workspace: &WorkspaceSpec,
            _source_ref: &str,
        ) -> Result<ApplyOutcome, RepositoryError> {
            Ok(ApplyOutcome {
                merged_to: workspace.checked_out_branch.clone(),
                commit: "def456".into(),
            })
        }

        async fn list_branches(
            &self,
            workspace: &WorkspaceSpec,
        ) -> Result<Vec<BranchRecord>, RepositoryError> {
            let mut branches = vec![BranchRecord {
                name: workspace.checked_out_branch.clone(),
                commit: "def456".into(),
                summary: "checked out".into(),
            }];
            if workspace.exported_branch != workspace.checked_out_branch {
                branches.push(BranchRecord {
                    name: workspace.exported_branch.clone(),
                    commit: "abc123".into(),
                    summary: "served".into(),
                });
            }
            if workspace.checked_out_branch != "feature" && workspace.exported_branch != "feature" {
                branches.push(BranchRecord {
                    name: "feature".into(),
                    commit: "222222222222".into(),
                    summary: "feature".into(),
                });
            }
            Ok(branches)
        }

        async fn create_branch(
            &self,
            _workspace: &WorkspaceSpec,
            _name: &str,
            _start_point: Option<&str>,
            _force: bool,
        ) -> Result<String, RepositoryError> {
            Ok("def456".into())
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
            workspace: &WorkspaceSpec,
            _name: &str,
        ) -> Result<String, RepositoryError> {
            Ok(format!("{}-head", workspace.checked_out_branch))
        }

        async fn checkout_branch(
            &self,
            workspace: &WorkspaceSpec,
            _name: &str,
            _force: bool,
        ) -> Result<String, RepositoryError> {
            Ok(format!("{}-checkout", workspace.checked_out_branch))
        }

        async fn merge_branch(
            &self,
            _workspace: &WorkspaceSpec,
            source_branch: &str,
            _target_branch: &str,
        ) -> Result<String, RepositoryError> {
            Ok(format!("{source_branch}-merged"))
        }
    }

    struct StubRegistry {
        worktree: PathBuf,
        default_sidecar: PathBuf,
        records: Mutex<HashMap<WorkspaceId, WorkspaceRecord>>,
    }

    impl RegistryStore for StubRegistry {
        fn canonical_worktree(&self, _worktree: &Path) -> Result<PathBuf, RegistryError> {
            Ok(self.worktree.clone())
        }

        fn default_sidecar_path(&self, _id: WorkspaceId) -> Result<PathBuf, RegistryError> {
            Ok(self.default_sidecar.clone())
        }

        fn load(&self, id: WorkspaceId) -> Result<Option<WorkspaceRecord>, RegistryError> {
            Ok(self.records.lock().unwrap().get(&id).cloned())
        }

        fn save(&self, id: WorkspaceId, record: WorkspaceRecord) -> Result<(), RegistryError> {
            self.records.lock().unwrap().insert(id, record);
            Ok(())
        }
    }

    struct StubIssuer;

    impl CredentialIssuer for StubIssuer {
        fn issue(&self) -> SessionCredentials {
            SessionCredentials {
                username: "user".into(),
                password: "pass".into(),
            }
        }
    }

    fn temp_workspace() -> (TempDir, PathBuf, PathBuf, WorkspaceId) {
        let temp = TempDir::new().unwrap();
        let worktree = temp.path().join("worktree");
        let sidecar = temp.path().join("sidecar.git");
        std::fs::create_dir_all(&worktree).unwrap();
        (
            temp,
            worktree.clone(),
            sidecar,
            WorkspaceId::from_worktree(&worktree),
        )
    }

    fn service_with_workspace(
        exported_branch: &str,
        checked_out_branch: &str,
    ) -> (TempDir, Arc<StubRegistry>, WorkspaceService) {
        let (temp, worktree, sidecar, workspace_id) = temp_workspace();
        std::fs::create_dir_all(&sidecar).unwrap();
        let registry = Arc::new(StubRegistry {
            worktree: worktree.clone(),
            default_sidecar: sidecar.clone(),
            records: Mutex::new(HashMap::from([(
                workspace_id,
                WorkspaceRecord {
                    worktree,
                    sidecar,
                    exported_branch: exported_branch.into(),
                    checked_out_branch: Some(checked_out_branch.into()),
                    web_ui: WorkspaceWebUiState::default(),
                },
            )])),
        });
        let service = WorkspaceService::new(
            Arc::new(StubRepoStore),
            registry.clone(),
            Arc::new(StubIssuer),
        );
        (temp, registry, service)
    }

    #[tokio::test]
    async fn prepare_serve_uses_stable_workspace_identity() {
        let (_temp, worktree, sidecar, _workspace_id) = temp_workspace();
        let registry = Arc::new(StubRegistry {
            worktree: worktree.clone(),
            default_sidecar: sidecar,
            records: Mutex::new(HashMap::new()),
        });
        let service =
            WorkspaceService::new(Arc::new(StubRepoStore), registry, Arc::new(StubIssuer));

        let prepared = service
            .prepare_serve(PathBuf::from("."), Some(DEFAULT_BRANCH), "snapshot", false)
            .await
            .unwrap();

        assert_eq!(prepared.workspace.id, WorkspaceId::from_worktree(&worktree));
        assert_eq!(prepared.snapshot_commit.as_deref(), Some("abc123"));
    }

    #[tokio::test]
    async fn switch_branch_persists_new_exported_branch() {
        let (_temp, worktree, sidecar, workspace_id) = temp_workspace();
        let registry = Arc::new(StubRegistry {
            worktree: worktree.clone(),
            default_sidecar: sidecar.clone(),
            records: Mutex::new(HashMap::from([(
                workspace_id,
                WorkspaceRecord {
                    worktree: worktree.clone(),
                    sidecar: sidecar.clone(),
                    exported_branch: "main".into(),
                    checked_out_branch: Some("main".into()),
                    web_ui: WorkspaceWebUiState::default(),
                },
            )])),
        });
        let service = WorkspaceService::new(
            Arc::new(StubRepoStore),
            registry.clone(),
            Arc::new(StubIssuer),
        );

        std::fs::create_dir_all(&sidecar).unwrap();
        let (workspace, outcome) = service
            .switch_branch(PathBuf::from("."), DEFAULT_BRANCH, "feature")
            .await
            .unwrap();

        assert_eq!(workspace.exported_branch, "feature");
        assert_eq!(workspace.checked_out_branch, "feature");
        assert_eq!(outcome.previous_branch, "main");
        assert_eq!(outcome.current_branch, "feature");
        assert_eq!(
            registry
                .records
                .lock()
                .unwrap()
                .get(&workspace_id)
                .unwrap()
                .exported_branch,
            "feature"
        );
        assert_eq!(
            registry
                .records
                .lock()
                .unwrap()
                .get(&workspace_id)
                .unwrap()
                .checked_out_branch
                .as_deref(),
            Some("feature")
        );
    }

    #[tokio::test]
    async fn checkout_branch_preserves_exported_branch() {
        let (_temp, worktree, sidecar, workspace_id) = temp_workspace();
        let registry = Arc::new(StubRegistry {
            worktree: worktree.clone(),
            default_sidecar: sidecar.clone(),
            records: Mutex::new(HashMap::from([(
                workspace_id,
                WorkspaceRecord {
                    worktree: worktree.clone(),
                    sidecar: sidecar.clone(),
                    exported_branch: "main".into(),
                    checked_out_branch: Some("main".into()),
                    web_ui: WorkspaceWebUiState::default(),
                },
            )])),
        });
        let service = WorkspaceService::new(
            Arc::new(StubRepoStore),
            registry.clone(),
            Arc::new(StubIssuer),
        );

        std::fs::create_dir_all(&sidecar).unwrap();
        let (workspace, outcome) = service
            .checkout_branch(PathBuf::from("."), DEFAULT_BRANCH, "feature")
            .await
            .unwrap();

        assert_eq!(workspace.exported_branch, "main");
        assert_eq!(workspace.checked_out_branch, "feature");
        assert_eq!(outcome.previous_branch, "main");
        assert_eq!(outcome.current_branch, "feature");
        assert_eq!(
            registry
                .records
                .lock()
                .unwrap()
                .get(&workspace_id)
                .unwrap()
                .exported_branch,
            "main"
        );
        assert_eq!(
            registry
                .records
                .lock()
                .unwrap()
                .get(&workspace_id)
                .unwrap()
                .checked_out_branch
                .as_deref(),
            Some("feature")
        );
    }

    #[tokio::test]
    async fn rename_branch_persists_exported_and_checked_out_branch() {
        let (_temp, worktree, sidecar, workspace_id) = temp_workspace();
        let registry = Arc::new(StubRegistry {
            worktree: worktree.clone(),
            default_sidecar: sidecar.clone(),
            records: Mutex::new(HashMap::from([(
                workspace_id,
                WorkspaceRecord {
                    worktree: worktree.clone(),
                    sidecar: sidecar.clone(),
                    exported_branch: "main".into(),
                    checked_out_branch: Some("main".into()),
                    web_ui: WorkspaceWebUiState::default(),
                },
            )])),
        });
        let service = WorkspaceService::new(
            Arc::new(StubRepoStore),
            registry.clone(),
            Arc::new(StubIssuer),
        );

        std::fs::create_dir_all(&sidecar).unwrap();
        let workspace = service
            .rename_branch(PathBuf::from("."), DEFAULT_BRANCH, "main", "renamed", false)
            .await
            .unwrap();

        assert_eq!(workspace.exported_branch, "renamed");
        assert_eq!(workspace.checked_out_branch, "renamed");
        assert_eq!(
            registry
                .records
                .lock()
                .unwrap()
                .get(&workspace_id)
                .unwrap()
                .exported_branch,
            "renamed"
        );
        assert_eq!(
            registry
                .records
                .lock()
                .unwrap()
                .get(&workspace_id)
                .unwrap()
                .checked_out_branch
                .as_deref(),
            Some("renamed")
        );
    }

    #[tokio::test]
    async fn apply_branch_override_switches_checked_out_branch_explicitly() {
        let (_temp, worktree, sidecar, workspace_id) = temp_workspace();
        let registry = Arc::new(StubRegistry {
            worktree: worktree.clone(),
            default_sidecar: sidecar.clone(),
            records: Mutex::new(HashMap::from([(
                workspace_id,
                WorkspaceRecord {
                    worktree: worktree.clone(),
                    sidecar: sidecar.clone(),
                    exported_branch: "main".into(),
                    checked_out_branch: Some("feature".into()),
                    web_ui: WorkspaceWebUiState::default(),
                },
            )])),
        });
        let service = WorkspaceService::new(
            Arc::new(StubRepoStore),
            registry.clone(),
            Arc::new(StubIssuer),
        );

        std::fs::create_dir_all(&sidecar).unwrap();
        let (workspace, outcome) = service
            .apply(PathBuf::from("."), DEFAULT_BRANCH, Some("main".to_string()))
            .await
            .unwrap();

        assert_eq!(workspace.exported_branch, "main");
        assert_eq!(workspace.checked_out_branch, "main");
        assert_eq!(outcome.merged_to, "main");
        assert_eq!(
            registry
                .records
                .lock()
                .unwrap()
                .get(&workspace_id)
                .unwrap()
                .checked_out_branch
                .as_deref(),
            Some("main")
        );
    }

    #[tokio::test]
    async fn prepare_serve_rejects_conflicting_explicit_branch_for_existing_workspace() {
        let (_temp, worktree, sidecar, workspace_id) = temp_workspace();
        let registry = Arc::new(StubRegistry {
            worktree: worktree.clone(),
            default_sidecar: sidecar.clone(),
            records: Mutex::new(HashMap::from([(
                workspace_id,
                WorkspaceRecord {
                    worktree: worktree.clone(),
                    sidecar: sidecar.clone(),
                    exported_branch: "feature".into(),
                    checked_out_branch: Some("feature".into()),
                    web_ui: WorkspaceWebUiState::default(),
                },
            )])),
        });
        let service =
            WorkspaceService::new(Arc::new(StubRepoStore), registry, Arc::new(StubIssuer));

        std::fs::create_dir_all(&sidecar).unwrap();
        assert!(matches!(
            service
                .prepare_serve(PathBuf::from("."), Some("main"), "snapshot", false)
                .await,
            Err(DomainError::ExportedBranchConflict { current, requested })
                if current == "feature" && requested == "main"
        ));
    }

    #[tokio::test]
    async fn prepare_serve_rejects_existing_git_worktree_without_flag() {
        let (_temp, worktree, sidecar, _workspace_id) = temp_workspace();
        Repository::init(&worktree).unwrap();
        let registry = Arc::new(StubRegistry {
            worktree: worktree.clone(),
            default_sidecar: sidecar,
            records: Mutex::new(HashMap::new()),
        });
        let service =
            WorkspaceService::new(Arc::new(StubRepoStore), registry, Arc::new(StubIssuer));

        assert!(matches!(
            service
                .prepare_serve(PathBuf::from("."), None, "snapshot", false)
                .await,
            Err(DomainError::ExistingGitWorktreeRequiresFlag(path)) if path == worktree
        ));
    }

    #[tokio::test]
    async fn prepare_serve_allows_existing_git_worktree_with_flag_and_infers_branch() {
        let (_temp, worktree, sidecar, _workspace_id) = temp_workspace();
        let repo = Repository::init(&worktree).unwrap();
        std::fs::write(worktree.join("README.md"), "hello\n").unwrap();
        let mut index = repo.index().unwrap();
        index.add_path(std::path::Path::new("README.md")).unwrap();
        let tree_id = index.write_tree().unwrap();
        let tree = repo.find_tree(tree_id).unwrap();
        let signature = git2::Signature::now("tester", "tester@example.com").unwrap();
        let commit_id = repo
            .commit(Some("HEAD"), &signature, &signature, "init", &tree, &[])
            .unwrap();
        let commit = repo.find_commit(commit_id).unwrap();
        repo.branch("trunk", &commit, true).unwrap();
        repo.set_head("refs/heads/trunk").unwrap();
        let registry = Arc::new(StubRegistry {
            worktree: worktree.clone(),
            default_sidecar: sidecar,
            records: Mutex::new(HashMap::new()),
        });
        let service =
            WorkspaceService::new(Arc::new(StubRepoStore), registry, Arc::new(StubIssuer));

        let prepared = service
            .prepare_serve(PathBuf::from("."), None, "snapshot", true)
            .await
            .unwrap();

        assert_eq!(prepared.workspace.checked_out_branch, "trunk");
        assert_eq!(prepared.workspace.exported_branch, "trunk");
    }

    #[tokio::test]
    async fn pull_request_lifecycle_tracks_discussion_reviews_and_status() {
        let (_temp, registry, service) = service_with_workspace("main", "feature");

        let (_, created) = service
            .create_pull_request(
                PathBuf::from("."),
                DEFAULT_BRANCH,
                CreatePullRequest {
                    title: "Feature PR".into(),
                    description: "Initial description".into(),
                    source_branch: "feature".into(),
                    target_branch: "main".into(),
                },
                UiRole::User,
            )
            .await
            .unwrap();
        assert_eq!(created.activities.len(), 1);
        assert_eq!(created.activities[0].kind, PullRequestActivityKind::Opened);

        let (_, updated) = service
            .update_pull_request(
                PathBuf::from("."),
                DEFAULT_BRANCH,
                &created.id,
                UpdatePullRequest {
                    title: Some("Updated PR".into()),
                    description: Some("Updated description".into()),
                    status: Some(PullRequestStatus::Closed),
                },
                UiRole::Owner,
            )
            .await
            .unwrap();
        assert_eq!(updated.title, "Updated PR");
        assert_eq!(updated.status, PullRequestStatus::Closed);

        let (_, commented) = service
            .comment_pull_request(
                PathBuf::from("."),
                DEFAULT_BRANCH,
                &created.id,
                CreatePullRequestComment {
                    display_name: "Casey".into(),
                    body: "Looks close.".into(),
                },
                UiRole::User,
            )
            .await
            .unwrap();

        let (_, reviewed) = service
            .review_pull_request(
                PathBuf::from("."),
                DEFAULT_BRANCH,
                &created.id,
                CreatePullRequestReview {
                    display_name: "Casey".into(),
                    body: "Please tighten this up.".into(),
                    state: PullRequestReviewState::ChangesRequested,
                },
                UiRole::User,
            )
            .await
            .unwrap();

        let (_, reopened) = service
            .update_pull_request(
                PathBuf::from("."),
                DEFAULT_BRANCH,
                &created.id,
                UpdatePullRequest {
                    title: None,
                    description: None,
                    status: Some(PullRequestStatus::Open),
                },
                UiRole::Owner,
            )
            .await
            .unwrap();
        assert_eq!(reopened.status, PullRequestStatus::Open);

        let summary = WorkspaceService::pull_request_review_summary(&reviewed);
        assert_eq!(summary.changes_requested, 1);
        assert_eq!(summary.latest_reviews[0].display_name, "Casey");
        assert_eq!(WorkspaceService::pull_request_comments(&commented).len(), 1);

        let stored = registry.records.lock().unwrap();
        let stored_pr = stored
            .values()
            .next()
            .unwrap()
            .web_ui
            .pull_requests
            .first()
            .unwrap()
            .clone();
        assert_eq!(stored_pr.status, PullRequestStatus::Open);
        assert!(stored_pr
            .activities
            .iter()
            .any(|activity| activity.kind == PullRequestActivityKind::Closed));
        assert!(stored_pr
            .activities
            .iter()
            .any(|activity| activity.kind == PullRequestActivityKind::Reopened));
        assert!(stored_pr
            .activities
            .iter()
            .any(|activity| activity.kind == PullRequestActivityKind::Reviewed));
    }

    #[tokio::test]
    async fn deleting_pull_requests_removes_them_from_workspace_state() {
        let (_temp, registry, service) = service_with_workspace("main", "feature");
        let (_, created) = service
            .create_pull_request(
                PathBuf::from("."),
                DEFAULT_BRANCH,
                CreatePullRequest {
                    title: "Disposable".into(),
                    description: String::new(),
                    source_branch: "feature".into(),
                    target_branch: "main".into(),
                },
                UiRole::Owner,
            )
            .await
            .unwrap();

        let (_, deleted) = service
            .delete_pull_request(PathBuf::from("."), DEFAULT_BRANCH, &created.id)
            .await
            .unwrap();
        assert_eq!(deleted.id, created.id);

        let stored = registry.records.lock().unwrap();
        assert!(stored
            .values()
            .next()
            .unwrap()
            .web_ui
            .pull_requests
            .is_empty());
    }

    #[tokio::test]
    async fn issue_lifecycle_tracks_metadata_comments_and_reactions() {
        let (_temp, registry, service) = service_with_workspace("main", "feature");
        let (_, label) = service
            .upsert_issue_label(
                PathBuf::from("."),
                DEFAULT_BRANCH,
                UpsertIssueLabel {
                    id: None,
                    name: "bug".into(),
                    color: "cf222e".into(),
                    description: "Bug work".into(),
                },
            )
            .unwrap();
        let (_, milestone) = service
            .upsert_issue_milestone(
                PathBuf::from("."),
                DEFAULT_BRANCH,
                UpsertIssueMilestone {
                    id: None,
                    title: "v1".into(),
                    description: "First release".into(),
                },
            )
            .unwrap();
        let (_, pull_request) = service
            .create_pull_request(
                PathBuf::from("."),
                DEFAULT_BRANCH,
                CreatePullRequest {
                    title: "Fix PR".into(),
                    description: String::new(),
                    source_branch: "feature".into(),
                    target_branch: "main".into(),
                },
                UiRole::Owner,
            )
            .await
            .unwrap();

        let actor = IssueActorInput {
            role: UiRole::Owner,
            display_name: Some("Taylor".into()),
            user_id: Some("owner-1".into()),
            username: Some("taylor".into()),
        };
        let (_, created) = service
            .create_issue(
                PathBuf::from("."),
                DEFAULT_BRANCH,
                CreateIssue {
                    title: "Broken flow".into(),
                    description: "Something is off".into(),
                    label_ids: vec![label.id.clone()],
                    assignee_user_ids: Vec::new(),
                    milestone_id: Some(milestone.id.clone()),
                    linked_pull_request_ids: vec![pull_request.id.clone()],
                },
                actor.clone(),
            )
            .await
            .unwrap();
        assert_eq!(created.number, 1);
        assert_eq!(created.timeline[0].kind, IssueTimelineEventKind::Opened);

        let (_, commented) = service
            .comment_issue(
                PathBuf::from("."),
                DEFAULT_BRANCH,
                &created.id,
                CreateIssueComment {
                    display_name: Some("Taylor".into()),
                    body: "Investigating now.".into(),
                },
                actor.clone(),
            )
            .await
            .unwrap();
        assert_eq!(commented.comments.len(), 1);

        let (_, reacted) = service
            .toggle_issue_reaction(
                PathBuf::from("."),
                DEFAULT_BRANCH,
                &created.id,
                IssueReactionContent::ThumbsUp,
                actor.clone(),
            )
            .await
            .unwrap();
        assert_eq!(
            WorkspaceService::issue_reaction_summary(&reacted, Some(&created.author))[0].count,
            1
        );

        let (_, closed) = service
            .update_issue(
                PathBuf::from("."),
                DEFAULT_BRANCH,
                &created.id,
                UpdateIssue {
                    title: Some("Broken login flow".into()),
                    description: None,
                    status: Some(IssueStatus::Closed),
                },
                actor,
            )
            .await
            .unwrap();
        assert_eq!(closed.status, IssueStatus::Closed);

        let stored = registry.records.lock().unwrap();
        let stored_issue = stored
            .values()
            .next()
            .unwrap()
            .web_ui
            .issues
            .first()
            .unwrap()
            .clone();
        assert_eq!(stored_issue.title, "Broken login flow");
        assert!(stored_issue
            .timeline
            .iter()
            .any(|event| event.kind == IssueTimelineEventKind::Commented));
        assert!(stored_issue
            .timeline
            .iter()
            .any(|event| event.kind == IssueTimelineEventKind::PullRequestLinked));
    }

    #[tokio::test]
    async fn smart_links_are_derived_in_both_directions_without_duplicates() {
        let (_temp, _registry, service) = service_with_workspace("main", "feature");
        let actor = IssueActorInput {
            role: UiRole::Owner,
            display_name: Some("Taylor".into()),
            user_id: Some("owner-1".into()),
            username: Some("taylor".into()),
        };
        let (_, issue_one) = service
            .create_issue(
                PathBuf::from("."),
                DEFAULT_BRANCH,
                CreateIssue {
                    title: "Broken login".into(),
                    description: String::new(),
                    label_ids: Vec::new(),
                    assignee_user_ids: Vec::new(),
                    milestone_id: None,
                    linked_pull_request_ids: Vec::new(),
                },
                actor.clone(),
            )
            .await
            .unwrap();
        let (_, issue_two) = service
            .create_issue(
                PathBuf::from("."),
                DEFAULT_BRANCH,
                CreateIssue {
                    title: "Docs follow-up".into(),
                    description: String::new(),
                    label_ids: Vec::new(),
                    assignee_user_ids: Vec::new(),
                    milestone_id: None,
                    linked_pull_request_ids: Vec::new(),
                },
                actor.clone(),
            )
            .await
            .unwrap();
        let (_, pull_request_one) = service
            .create_pull_request(
                PathBuf::from("."),
                DEFAULT_BRANCH,
                CreatePullRequest {
                    title: "Login fix".into(),
                    description: format!(
                        "Fixes #{}\nRelated to #{}",
                        issue_one.number, issue_two.number
                    ),
                    source_branch: "feature".into(),
                    target_branch: "main".into(),
                },
                UiRole::Owner,
            )
            .await
            .unwrap();
        let (_, pull_request_two) = service
            .create_pull_request(
                PathBuf::from("."),
                DEFAULT_BRANCH,
                CreatePullRequest {
                    title: "Docs cleanup".into(),
                    description: String::new(),
                    source_branch: "feature".into(),
                    target_branch: "main".into(),
                },
                UiRole::Owner,
            )
            .await
            .unwrap();

        let (_, issue_two) = service
            .comment_issue(
                PathBuf::from("."),
                DEFAULT_BRANCH,
                &issue_two.id,
                CreateIssueComment {
                    display_name: Some("Taylor".into()),
                    body: format!(
                        "Related to PR {} and also PR {}",
                        pull_request_two.id.chars().take(8).collect::<String>(),
                        pull_request_two.id.chars().take(8).collect::<String>()
                    ),
                },
                actor.clone(),
            )
            .await
            .unwrap();
        let (_, web_ui) = service
            .load_web_ui(PathBuf::from("."), DEFAULT_BRANCH)
            .unwrap();

        let linked_issue_one = WorkspaceService::linked_pull_requests_for_issue(
            &issue_one,
            &web_ui.issues,
            &web_ui.pull_requests,
        );
        assert_eq!(linked_issue_one.len(), 1);
        assert_eq!(linked_issue_one[0].pull_request_id, pull_request_one.id);
        assert_eq!(linked_issue_one[0].relation, IssueLinkRelation::Closing);
        assert_eq!(
            linked_issue_one[0].source,
            IssueLinkSource::PullRequestDescription
        );

        let linked_issue_two = WorkspaceService::linked_pull_requests_for_issue(
            &issue_two,
            &web_ui.issues,
            &web_ui.pull_requests,
        );
        assert_eq!(linked_issue_two.len(), 2);
        assert!(linked_issue_two.iter().any(|link| {
            link.pull_request_id == pull_request_one.id
                && link.relation == IssueLinkRelation::Related
                && link.source == IssueLinkSource::PullRequestDescription
        }));
        assert!(linked_issue_two.iter().any(|link| {
            link.pull_request_id == pull_request_two.id
                && link.relation == IssueLinkRelation::Related
                && link.source == IssueLinkSource::IssueComment
        }));

        let linked_pull_request_one = WorkspaceService::linked_issues_for_pull_request(
            &pull_request_one,
            &web_ui.issues,
            &web_ui.pull_requests,
        );
        assert_eq!(linked_pull_request_one.len(), 2);
        assert!(linked_pull_request_one.iter().any(|link| {
            link.issue_id == issue_one.id && link.relation == IssueLinkRelation::Closing
        }));
        assert!(linked_pull_request_one.iter().any(|link| {
            link.issue_id == issue_two.id && link.relation == IssueLinkRelation::Related
        }));

        let linked_pull_request_two = WorkspaceService::linked_issues_for_pull_request(
            &pull_request_two,
            &web_ui.issues,
            &web_ui.pull_requests,
        );
        assert_eq!(linked_pull_request_two.len(), 1);
        assert_eq!(linked_pull_request_two[0].issue_id, issue_two.id);
        assert_eq!(
            linked_pull_request_two[0].source,
            IssueLinkSource::IssueComment
        );
    }

    #[tokio::test]
    async fn merging_pull_request_closes_issues_referenced_with_closing_keywords() {
        let (_temp, _registry, service) = service_with_workspace("main", "feature");
        let actor = IssueActorInput {
            role: UiRole::Owner,
            display_name: Some("Taylor".into()),
            user_id: Some("owner-1".into()),
            username: Some("taylor".into()),
        };
        let (_, issue) = service
            .create_issue(
                PathBuf::from("."),
                DEFAULT_BRANCH,
                CreateIssue {
                    title: "Broken login".into(),
                    description: String::new(),
                    label_ids: Vec::new(),
                    assignee_user_ids: Vec::new(),
                    milestone_id: None,
                    linked_pull_request_ids: Vec::new(),
                },
                actor.clone(),
            )
            .await
            .unwrap();
        let (_, pull_request) = service
            .create_pull_request(
                PathBuf::from("."),
                DEFAULT_BRANCH,
                CreatePullRequest {
                    title: "Login fix".into(),
                    description: format!("Fixes #{}", issue.number),
                    source_branch: "feature".into(),
                    target_branch: "main".into(),
                },
                UiRole::Owner,
            )
            .await
            .unwrap();

        let (_, merged) = service
            .merge_pull_request(PathBuf::from("."), DEFAULT_BRANCH, &pull_request.id, actor)
            .await
            .unwrap();
        assert_eq!(merged.status, PullRequestStatus::Merged);

        let (_, web_ui) = service
            .load_web_ui(PathBuf::from("."), DEFAULT_BRANCH)
            .unwrap();
        let stored_issue = web_ui
            .issues
            .iter()
            .find(|candidate| candidate.id == issue.id)
            .unwrap();
        assert_eq!(stored_issue.status, IssueStatus::Closed);
        assert!(stored_issue.closed_at_ms.is_some());
    }

    #[tokio::test]
    async fn repository_settings_round_trip_and_branch_rules_persist() {
        let (_temp, registry, service) = service_with_workspace("main", "main");

        let (_, settings) = service
            .update_repository_settings(
                PathBuf::from("."),
                DEFAULT_BRANCH,
                UpdateRepositorySettings {
                    description: Some("Friendly repo".into()),
                    homepage_url: Some("https://example.com/docs".into()),
                },
            )
            .unwrap();
        assert_eq!(settings.description, "Friendly repo");
        assert_eq!(settings.homepage_url, "https://example.com/docs");

        let (_, settings) = service
            .upsert_branch_rule(
                PathBuf::from("."),
                DEFAULT_BRANCH,
                UpsertBranchRule {
                    pattern: "main".into(),
                    require_pull_request: true,
                    required_approvals: 2,
                    dismiss_stale_approvals: true,
                    block_force_push: true,
                    block_delete: true,
                },
            )
            .unwrap();
        assert_eq!(settings.branch_rules.len(), 1);
        assert_eq!(settings.branch_rules[0].pattern, "main");

        let stored = registry.records.lock().unwrap();
        let record = stored.values().next().unwrap();
        assert_eq!(record.web_ui.repository.description, "Friendly repo");
        assert_eq!(
            record.web_ui.repository.homepage_url,
            "https://example.com/docs"
        );
        assert_eq!(
            record.web_ui.repository.branch_rules[0].required_approvals,
            2
        );
    }

    #[tokio::test]
    async fn protected_branch_delete_is_rejected() {
        let (_temp, _registry, service) = service_with_workspace("main", "main");
        service
            .upsert_branch_rule(
                PathBuf::from("."),
                DEFAULT_BRANCH,
                UpsertBranchRule {
                    pattern: "main".into(),
                    require_pull_request: false,
                    required_approvals: 0,
                    dismiss_stale_approvals: false,
                    block_force_push: false,
                    block_delete: true,
                },
            )
            .unwrap();

        let error = service
            .delete_branch(PathBuf::from("."), DEFAULT_BRANCH, "main", false)
            .await
            .unwrap_err();
        assert!(matches!(error, DomainError::BranchRuleViolation(_)));
    }

    #[tokio::test]
    async fn branch_rules_reject_shell_sensitive_patterns() {
        let (_temp, _registry, service) = service_with_workspace("main", "main");
        let error = service
            .upsert_branch_rule(
                PathBuf::from("."),
                DEFAULT_BRANCH,
                UpsertBranchRule {
                    pattern: "main; rm -rf /".into(),
                    require_pull_request: false,
                    required_approvals: 0,
                    dismiss_stale_approvals: false,
                    block_force_push: true,
                    block_delete: true,
                },
            )
            .unwrap_err();
        assert!(matches!(error, DomainError::InvalidSettings(_)));
    }

    #[tokio::test]
    async fn merge_requires_fresh_approvals_when_branch_rule_demands_it() {
        let (_temp, _registry, service) = service_with_workspace("main", "feature");
        service
            .upsert_branch_rule(
                PathBuf::from("."),
                DEFAULT_BRANCH,
                UpsertBranchRule {
                    pattern: "main".into(),
                    require_pull_request: true,
                    required_approvals: 1,
                    dismiss_stale_approvals: true,
                    block_force_push: false,
                    block_delete: false,
                },
            )
            .unwrap();

        let (_, created) = service
            .create_pull_request(
                PathBuf::from("."),
                DEFAULT_BRANCH,
                CreatePullRequest {
                    title: "Protected".into(),
                    description: String::new(),
                    source_branch: "feature".into(),
                    target_branch: "main".into(),
                },
                UiRole::Owner,
            )
            .await
            .unwrap();

        service
            .review_pull_request(
                PathBuf::from("."),
                DEFAULT_BRANCH,
                &created.id,
                CreatePullRequestReview {
                    display_name: "Casey".into(),
                    body: "Looks good".into(),
                    state: PullRequestReviewState::Approved,
                },
                UiRole::Owner,
            )
            .await
            .unwrap();

        service
            .checkout_branch(PathBuf::from("."), DEFAULT_BRANCH, "main")
            .await
            .unwrap();

        let error = service
            .merge_pull_request(
                PathBuf::from("."),
                DEFAULT_BRANCH,
                &created.id,
                IssueActorInput {
                    role: UiRole::Owner,
                    display_name: Some("Taylor".into()),
                    user_id: Some("owner-1".into()),
                    username: Some("taylor".into()),
                },
            )
            .await
            .unwrap_err();
        assert!(matches!(error, DomainError::BranchRuleViolation(_)));
    }

    #[test]
    fn request_based_auth_flow_supports_onboarding_and_pat_auth() {
        let (_temp, _registry, service) = service_with_workspace("main", "main");
        service
            .update_auth_mode(
                PathBuf::from("."),
                DEFAULT_BRANCH,
                AuthMode::RequestBased,
                &AuthActor::Operator,
            )
            .unwrap();
        let (_, request) = service
            .submit_access_request(
                PathBuf::from("."),
                DEFAULT_BRANCH,
                "Alice",
                "alice@example.com",
            )
            .unwrap();
        assert!(request.secret.starts_with("qit_request."));
        let (_, progress) = service
            .read_access_request_progress(PathBuf::from("."), DEFAULT_BRANCH, &request.secret)
            .unwrap();
        assert_eq!(progress.status, AccessRequestStatus::Pending);
        let (_, user, _onboarding) = service
            .approve_access_request(
                PathBuf::from("."),
                DEFAULT_BRANCH,
                &request.request.id,
                RepoUserRole::User,
                &AuthActor::Operator,
            )
            .unwrap();
        assert_eq!(user.status, RepoUserStatus::ApprovedPendingSetup);

        let (_, principal) = service
            .complete_onboarding(
                PathBuf::from("."),
                DEFAULT_BRANCH,
                &request.secret,
                "alice",
                "very-secret-pass",
            )
            .unwrap();
        assert_eq!(principal.username, "alice");

        let (_, web_principal) = service
            .authenticate_web_user(
                PathBuf::from("."),
                DEFAULT_BRANCH,
                "alice",
                "very-secret-pass",
            )
            .unwrap();
        assert_eq!(web_principal.user_id, principal.user_id);

        let (_, _pat, issued_pat) = service
            .create_pat(
                PathBuf::from("."),
                DEFAULT_BRANCH,
                &principal.user_id,
                "laptop",
                &AuthActor::User {
                    user_id: principal.user_id.clone(),
                    username: principal.username.clone(),
                    role: principal.role.clone(),
                },
            )
            .unwrap();
        let (_, git_principal) = service
            .authenticate_git_user(
                PathBuf::from("."),
                DEFAULT_BRANCH,
                "alice",
                &issued_pat.secret,
            )
            .unwrap();
        assert_eq!(git_principal.user_id, principal.user_id);
    }

    #[test]
    fn issuing_manual_setup_token_approves_matching_request_without_auto_setup() {
        let (_temp, _registry, service) = service_with_workspace("main", "main");
        service
            .update_auth_mode(
                PathBuf::from("."),
                DEFAULT_BRANCH,
                AuthMode::RequestBased,
                &AuthActor::Operator,
            )
            .unwrap();
        let (_, request) = service
            .submit_access_request(
                PathBuf::from("."),
                DEFAULT_BRANCH,
                "Alice",
                "alice@example.com",
            )
            .unwrap();

        let (_, user, onboarding) = service
            .issue_setup_token(
                PathBuf::from("."),
                DEFAULT_BRANCH,
                "Alice",
                "alice@example.com",
                RepoUserRole::User,
                &AuthActor::Operator,
            )
            .unwrap();
        assert_eq!(user.status, RepoUserStatus::ApprovedPendingSetup);
        assert!(onboarding
            .secret
            .as_deref()
            .is_some_and(|secret| secret.starts_with("qit_setup.")));

        let (_, progress) = service
            .read_access_request_progress(PathBuf::from("."), DEFAULT_BRANCH, &request.secret)
            .unwrap();
        assert_eq!(progress.status, AccessRequestStatus::Approved);

        let (_, principal) = service
            .complete_onboarding(
                PathBuf::from("."),
                DEFAULT_BRANCH,
                onboarding
                    .secret
                    .as_deref()
                    .expect("manual setup issues a token"),
                "alice",
                "very-secret-pass",
            )
            .unwrap();
        assert_eq!(principal.username, "alice");
    }

    #[test]
    fn request_based_auth_prevents_removing_last_owner() {
        let (_temp, _registry, service) = service_with_workspace("main", "main");
        service
            .update_auth_mode(
                PathBuf::from("."),
                DEFAULT_BRANCH,
                AuthMode::RequestBased,
                &AuthActor::Operator,
            )
            .unwrap();
        let (_, request) = service
            .submit_access_request(
                PathBuf::from("."),
                DEFAULT_BRANCH,
                "Owner",
                "owner@example.com",
            )
            .unwrap();
        let (_, owner, _issued) = service
            .approve_access_request(
                PathBuf::from("."),
                DEFAULT_BRANCH,
                &request.request.id,
                RepoUserRole::Owner,
                &AuthActor::Operator,
            )
            .unwrap();
        service
            .complete_onboarding(
                PathBuf::from("."),
                DEFAULT_BRANCH,
                &request.secret,
                "owner-user",
                "very-secret-pass",
            )
            .unwrap();

        let demote_error = service
            .demote_user(
                PathBuf::from("."),
                DEFAULT_BRANCH,
                &owner.id,
                &AuthActor::Operator,
            )
            .unwrap_err();
        assert!(matches!(demote_error, DomainError::InvalidAuth(_)));

        let revoke_error = service
            .revoke_user(
                PathBuf::from("."),
                DEFAULT_BRANCH,
                &owner.id,
                &AuthActor::Operator,
            )
            .unwrap_err();
        assert!(matches!(revoke_error, DomainError::InvalidAuth(_)));
    }
}
