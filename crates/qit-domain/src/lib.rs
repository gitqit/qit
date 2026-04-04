use async_trait::async_trait;
use fs2::FileExt;
use serde::{Deserialize, Serialize};
use std::fs::{File, OpenOptions};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use thiserror::Error;
use uuid::Uuid;

pub const DEFAULT_BRANCH: &str = "main";

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionCredentials {
    pub username: String,
    pub password: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApplyOutcome {
    pub merged_to: String,
    pub commit: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreparedServe {
    pub workspace: WorkspaceSpec,
    pub credentials: SessionCredentials,
    pub snapshot_commit: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
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

#[derive(Debug, Error)]
pub enum DomainError {
    #[error("path resolution failed: {0}")]
    PathResolution(#[source] RegistryError),
    #[error("registry operation failed: {0}")]
    Registry(#[source] RegistryError),
    #[error("repository operation failed: {0}")]
    Repository(#[source] RepositoryError),
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
                .any(|pattern| Self::glob_match(pattern.as_bytes(), name.as_bytes()))
    }

    fn glob_match(pattern: &[u8], text: &[u8]) -> bool {
        if pattern.is_empty() {
            return text.is_empty();
        }
        match pattern[0] {
            b'*' => {
                Self::glob_match(&pattern[1..], text)
                    || (!text.is_empty() && Self::glob_match(pattern, &text[1..]))
            }
            b'?' => !text.is_empty() && Self::glob_match(&pattern[1..], &text[1..]),
            byte => {
                !text.is_empty() && byte == text[0] && Self::glob_match(&pattern[1..], &text[1..])
            }
        }
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

    pub async fn prepare_serve(
        &self,
        path: PathBuf,
        requested_exported_branch: Option<&str>,
        snapshot_message: &str,
    ) -> Result<PreparedServe, DomainError> {
        let fallback_branch = requested_exported_branch.unwrap_or(DEFAULT_BRANCH);
        let initial_workspace = self
            .resolve_workspace(path.clone(), fallback_branch)
            .map_err(DomainError::PathResolution)?;
        let _lock = self.lock_resolved_workspace(&initial_workspace)?;
        let workspace = self
            .resolve_workspace(path, fallback_branch)
            .map_err(DomainError::PathResolution)?;
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

    fn save_workspace(&self, workspace: &WorkspaceSpec) -> Result<(), DomainError> {
        self.registry_store
            .save(
                workspace.id,
                WorkspaceRecord {
                    worktree: workspace.worktree.clone(),
                    sidecar: workspace.sidecar.clone(),
                    exported_branch: workspace.exported_branch.clone(),
                    checked_out_branch: Some(workspace.checked_out_branch.clone()),
                },
            )
            .map_err(DomainError::Registry)
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
            Ok(vec![
                BranchRecord {
                    name: workspace.checked_out_branch.clone(),
                    commit: "def456".into(),
                    summary: "checked out".into(),
                },
                BranchRecord {
                    name: workspace.exported_branch.clone(),
                    commit: "abc123".into(),
                    summary: "served".into(),
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
            .prepare_serve(PathBuf::from("."), Some(DEFAULT_BRANCH), "snapshot")
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
                },
            )])),
        });
        let service =
            WorkspaceService::new(Arc::new(StubRepoStore), registry, Arc::new(StubIssuer));

        std::fs::create_dir_all(&sidecar).unwrap();
        assert!(matches!(
            service
                .prepare_serve(PathBuf::from("."), Some("main"), "snapshot")
                .await,
            Err(DomainError::ExportedBranchConflict { current, requested })
                if current == "feature" && requested == "main"
        ));
    }
}
