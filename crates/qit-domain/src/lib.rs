mod branch_rules;

use async_trait::async_trait;
use fs2::FileExt;
use git2::Repository;
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
    #[error("invalid repository settings: {0}")]
    InvalidSettings(String),
    #[error("branch rule blocked the operation: {0}")]
    BranchRuleViolation(String),
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
            return Err(DomainError::InvalidPullRequest(format!("{field} is required")));
        }
        Ok(normalized.to_string())
    }

    fn normalize_optional(value: &str) -> String {
        value.trim().to_string()
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

    pub fn pull_request_comments(pull_request: &PullRequestRecord) -> Vec<PullRequestCommentRecord> {
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
                    display_name: activity
                        .display_name
                        .clone()
                        .unwrap_or_else(|| match activity.actor_role {
                            UiRole::Owner => "Owner".into(),
                            UiRole::User => "Viewer".into(),
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
                    display_name: activity
                        .display_name
                        .clone()
                        .unwrap_or_else(|| match activity.actor_role {
                            UiRole::Owner => "Owner".into(),
                            UiRole::User => "Viewer".into(),
                        }),
                    body: activity.body.clone().unwrap_or_default(),
                    state: activity.review_state.clone().unwrap_or(PullRequestReviewState::Commented),
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
                    activity.kind == PullRequestActivityKind::Reviewed
                        && activity.id == review.id
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
        Ok(state)
    }

    fn save_workspace_with_web_ui(
        &self,
        workspace: &WorkspaceSpec,
        mut web_ui: WorkspaceWebUiState,
    ) -> Result<(), DomainError> {
        web_ui.repository = Self::normalize_repository_settings(web_ui.repository)?;
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
        let current_source_commit = Self::branch_head_commit(
            &branches,
            &web_ui.pull_requests[index].source_branch,
        );
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
                    web_ui.pull_requests[index].target_branch,
                    protection.required_approvals
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

        let timestamp = Self::now_ms();
        web_ui.pull_requests[index].status = PullRequestStatus::Merged;
        web_ui.pull_requests[index].updated_at_ms = timestamp;
        web_ui.pull_requests[index].merged_commit = Some(merged_commit);
        Self::push_activity(
            &mut web_ui.pull_requests[index],
            PullRequestActivityKind::Merged,
            UiRole::Owner,
            None,
            None,
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
        if let Some(commit) = history.commits.iter().find(|commit| commit.authored_at <= cutoff) {
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
        assert_eq!(record.web_ui.repository.branch_rules[0].required_approvals, 2);
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
            .merge_pull_request(PathBuf::from("."), DEFAULT_BRANCH, &created.id)
            .await
            .unwrap_err();
        assert!(matches!(error, DomainError::BranchRuleViolation(_)));
    }
}
