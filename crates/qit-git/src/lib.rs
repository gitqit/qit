use async_trait::async_trait;
use git2::{Oid, Repository, Signature};
use ignore::WalkBuilder;
use qit_domain::{
    lock_workspace, ApplyOutcome, BranchRecord, RepoStore, RepositoryError, WorkspaceId,
    WorkspaceSpec,
};
use qit_http_backend::{
    BoxAsyncRead, GitHttpBackend, GitHttpBackendError, GitHttpBackendRequest,
    GitHttpBackendResponse,
};
use std::path::{Path, PathBuf};
use std::process::Stdio;
use thiserror::Error;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
#[derive(Debug, Error)]
pub enum GitStoreError {
    #[error("git: {0}")]
    Git(#[from] git2::Error),
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
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
    #[error("invalid path")]
    InvalidPath,
    #[error("snapshot traversal failed: {0}")]
    SnapshotWalk(String),
    #[error("worktree does not exist: {0}")]
    MissingWorktree(PathBuf),
    #[error("worktree is not a directory: {0}")]
    WorktreeNotDirectory(PathBuf),
}

impl From<GitStoreError> for RepositoryError {
    fn from(value: GitStoreError) -> Self {
        match value {
            GitStoreError::Git(error) => Self::Git(error.to_string()),
            GitStoreError::Io(error) => Self::Io {
                operation: "repository IO",
                message: error.to_string(),
            },
            GitStoreError::DirtyWorktree => Self::DirtyWorktree,
            GitStoreError::RefNotFound(name) => Self::RefNotFound(name),
            GitStoreError::BranchExists(name) => Self::BranchExists(name),
            GitStoreError::Unsupported(message) => Self::Unsupported(message),
            GitStoreError::CurrentBranch(name) => Self::CurrentBranch(name),
            GitStoreError::ServedBranch(name) => Self::ServedBranch(name),
            GitStoreError::BranchNotMerged(name) => Self::BranchNotMerged(name),
            GitStoreError::NotFastForward(message) => Self::NotFastForward(message),
            GitStoreError::InvalidPath => Self::InvalidPath("worktree path is invalid".into()),
            GitStoreError::SnapshotWalk(message) => Self::SnapshotWalk(message),
            GitStoreError::MissingWorktree(path) => Self::MissingWorktree(path),
            GitStoreError::WorktreeNotDirectory(path) => Self::WorktreeNotDirectory(path),
        }
    }
}

#[cfg(test)]
pub type CgiHttpResult = (u16, Vec<(String, String)>, Vec<u8>);

#[derive(Debug, Default)]
pub struct GitRepoStore;

#[derive(Debug, Default)]
pub struct GitHttpBackendAdapter;

#[async_trait]
impl GitHttpBackend for GitHttpBackendAdapter {
    async fn serve(
        &self,
        sidecar: &Path,
        request: GitHttpBackendRequest,
        body: BoxAsyncRead,
    ) -> Result<GitHttpBackendResponse, GitHttpBackendError> {
        run_git_http_backend(sidecar, request, body).await
    }
}

#[derive(Clone)]
struct ManagedWorkspace {
    _workspace_id: WorkspaceId,
    worktree: PathBuf,
    sidecar: PathBuf,
    exported_branch: String,
    checked_out_branch: String,
}

impl ManagedWorkspace {
    fn validate_worktree(&self) -> Result<(), GitStoreError> {
        if !self.worktree.exists() {
            return Err(GitStoreError::MissingWorktree(self.worktree.clone()));
        }
        if !self.worktree.is_dir() {
            return Err(GitStoreError::WorktreeNotDirectory(self.worktree.clone()));
        }
        Ok(())
    }

    fn reopen(workspace: &WorkspaceSpec) -> Self {
        Self {
            _workspace_id: workspace.id,
            worktree: workspace.worktree.clone(),
            sidecar: workspace.sidecar.clone(),
            exported_branch: workspace.exported_branch.clone(),
            checked_out_branch: workspace.checked_out_branch.clone(),
        }
    }

    fn init(workspace: &WorkspaceSpec) -> Result<Self, GitStoreError> {
        let mut managed = Self::reopen(workspace);
        managed.validate_worktree()?;
        std::fs::create_dir_all(&managed.sidecar)?;
        let repo = if managed.sidecar.join("HEAD").exists() {
            Repository::open_bare(&managed.sidecar)?
        } else {
            Repository::init_bare(&managed.sidecar)?
        };
        repo.set_workdir(&managed.worktree, false)?;

        let host_refname = managed.host_refname();
        if managed
            .open_repo()?
            .find_reference(&host_refname)
            .is_err()
        {
            managed.snapshot("Initial snapshot")?;
        }

        let repo = managed.open_repo()?;
        let exported_refname = managed.served_refname();
        if repo.find_reference(&exported_refname).is_err() {
            if let Ok(current_ref) = repo.find_reference(&host_refname) {
                let current_commit = current_ref.peel_to_commit()?;
                Self::set_ref_target(
                    &repo,
                    &exported_refname,
                    current_commit.id(),
                    "qit init exported branch",
                )?;
            }
        }
        if repo.find_reference(&exported_refname).is_ok() {
            repo.set_head(&exported_refname)?;
        }

        Ok(managed)
    }

    fn open_repo(&self) -> Result<Repository, GitStoreError> {
        self.validate_worktree()?;
        let repo = Repository::open_bare(&self.sidecar)?;
        repo.set_workdir(&self.worktree, false)?;
        Ok(repo)
    }

    fn served_refname(&self) -> String {
        Self::branch_refname(&self.exported_branch)
    }

    fn host_refname(&self) -> String {
        Self::branch_refname(&self.checked_out_branch)
    }

    fn applied_refname(&self) -> String {
        Self::applied_refname_for(&self.checked_out_branch)
    }

    fn branch_refname(branch: &str) -> String {
        format!("refs/heads/{branch}")
    }

    fn applied_refname_for(branch: &str) -> String {
        format!("refs/qit/applied/{branch}")
    }

    fn signature() -> Result<Signature<'static>, GitStoreError> {
        Ok(Signature::now("qit", "qit@localhost")?)
    }

    fn set_ref_target(
        repo: &Repository,
        refname: &str,
        oid: Oid,
        message: &str,
    ) -> Result<(), GitStoreError> {
        if let Ok(mut existing) = repo.find_reference(refname) {
            existing.set_target(oid, message)?;
        } else {
            repo.reference(refname, oid, true, message)?;
        }
        Ok(())
    }

    fn resolve_commit<'repo>(
        &self,
        repo: &'repo Repository,
        start_point: Option<&str>,
    ) -> Result<git2::Commit<'repo>, GitStoreError> {
        if let Some(start_point) = start_point {
            let object = repo
                .revparse_single(start_point)
                .map_err(|_| GitStoreError::RefNotFound(start_point.to_string()))?;
            return object
                .peel_to_commit()
                .map_err(|_| GitStoreError::RefNotFound(start_point.to_string()));
        }

        Ok(repo
            .find_reference(&self.host_refname())?
            .peel_to_commit()?)
    }

    fn sync_applied_to_current(&self, repo: &Repository) -> Result<(), GitStoreError> {
        let current_ref = repo.find_reference(&self.host_refname())?;
        let current_commit = current_ref.peel_to_commit()?;
        Self::set_ref_target(
            repo,
            &self.applied_refname(),
            current_commit.id(),
            "qit sync applied ref",
        )
    }

    fn snapshot(&mut self, message: &str) -> Result<Option<String>, GitStoreError> {
        let repo = self.open_repo()?;
        let mut index = repo.index()?;
        index.clear()?;

        let walker = WalkBuilder::new(&self.worktree)
            .current_dir(&self.worktree)
            .hidden(false)
            .require_git(false)
            .git_global(false)
            .git_ignore(true)
            .git_exclude(true)
            .build();

        for entry in walker {
            let entry = entry.map_err(|err| GitStoreError::SnapshotWalk(err.to_string()))?;
            if entry.depth() == 0 {
                continue;
            }
            if entry.file_type().map(|file| file.is_dir()).unwrap_or(false) {
                continue;
            }
            let rel = entry
                .path()
                .strip_prefix(&self.worktree)
                .map_err(|_| GitStoreError::InvalidPath)?;
            if rel.as_os_str().is_empty() {
                continue;
            }
            index.add_path(rel)?;
        }

        let tree_id = index.write_tree()?;
        let tree = repo.find_tree(tree_id)?;
        let refname = self.host_refname();
        let head_commit = repo
            .find_reference(&refname)
            .ok()
            .and_then(|reference| reference.peel_to_commit().ok());
        let parent_tree = head_commit
            .as_ref()
            .map(|commit| commit.tree())
            .transpose()?;
        if let Some(parent_tree) = parent_tree {
            if parent_tree.id() == tree.id() {
                self.sync_applied_to_current(&repo)?;
                return Ok(None);
            }
        }

        let sig = Self::signature()?;
        let commit_id = if let Some(ref parent) = head_commit {
            repo.commit(Some(&refname), &sig, &sig, message, &tree, &[parent])?
        } else {
            repo.commit(Some(&refname), &sig, &sig, message, &tree, &[])?
        };
        Self::set_ref_target(
            &repo,
            &self.applied_refname(),
            commit_id,
            "qit snapshot applied ref",
        )?;
        Ok(Some(commit_id.to_string()))
    }

    fn applied_commit<'a>(&self, repo: &'a Repository) -> Result<git2::Commit<'a>, GitStoreError> {
        if let Ok(applied) = repo.find_reference(&self.applied_refname()) {
            return Ok(applied.peel_to_commit()?);
        }
        Ok(repo
            .find_reference(&self.host_refname())?
            .peel_to_commit()?)
    }

    fn is_dirty_vs_applied(&self) -> Result<bool, GitStoreError> {
        let repo = self.open_repo()?;
        let applied_tree = self.applied_commit(&repo)?.tree()?;
        let mut opts = git2::DiffOptions::new();
        opts.include_untracked(true).recurse_untracked_dirs(true);
        let diff = repo.diff_tree_to_workdir(Some(&applied_tree), Some(&mut opts))?;
        Ok(diff.deltas().len() > 0)
    }

    fn apply_fast_forward(&self, source_ref: &str) -> Result<ApplyOutcome, GitStoreError> {
        if self.is_dirty_vs_applied()? {
            return Err(GitStoreError::DirtyWorktree);
        }

        let repo = self.open_repo()?;
        let src = repo
            .find_reference(source_ref)
            .map_err(|_| GitStoreError::RefNotFound(source_ref.to_string()))?;
        let src_commit = src.peel_to_commit()?;
        let applied_commit = self.applied_commit(&repo)?;
        let is_ff = src_commit.id() == applied_commit.id()
            || repo.graph_descendant_of(src_commit.id(), applied_commit.id())?;
        if !is_ff {
            return Err(GitStoreError::NotFastForward(
                "target is not a descendant of the applied commit".into(),
            ));
        }

        let mut checkout = git2::build::CheckoutBuilder::new();
        checkout.force();
        repo.checkout_tree(src_commit.as_object(), Some(&mut checkout))?;
        if let Err(error) = Self::set_ref_target(
            &repo,
            &self.applied_refname(),
            src_commit.id(),
            "qit apply_fast_forward applied ref",
        ) {
            let mut rollback = git2::build::CheckoutBuilder::new();
            rollback.force();
            let _ = repo.checkout_tree(applied_commit.as_object(), Some(&mut rollback));
            return Err(error);
        }

        Ok(ApplyOutcome {
            merged_to: self.checked_out_branch.clone(),
            commit: src_commit.id().to_string(),
        })
    }

    fn list_branches(&self) -> Result<Vec<BranchRecord>, GitStoreError> {
        let repo = self.open_repo()?;
        let mut branches = repo
            .references_glob("refs/heads/*")?
            .filter_map(Result::ok)
            .filter_map(|reference| {
                let name = reference
                    .name()
                    .and_then(|name| name.strip_prefix("refs/heads/"))
                    .map(ToString::to_string)?;
                let commit = reference.peel_to_commit().ok()?;
                Some(BranchRecord {
                    name,
                    commit: commit.id().to_string(),
                    summary: commit.summary().unwrap_or("").to_string(),
                })
            })
            .collect::<Vec<_>>();
        branches.sort_by(|left, right| left.name.cmp(&right.name));
        Ok(branches)
    }

    fn create_branch(
        &self,
        name: &str,
        start_point: Option<&str>,
        force: bool,
    ) -> Result<String, GitStoreError> {
        let repo = self.open_repo()?;
        let refname = Self::branch_refname(name);
        if repo.find_reference(&refname).is_ok() && !force {
            return Err(GitStoreError::BranchExists(name.to_string()));
        }
        if force && (name == self.checked_out_branch || name == self.exported_branch) {
            return Err(GitStoreError::Unsupported(format!(
                "refusing to reset protected branch `{name}`"
            )));
        }
        let commit = self.resolve_commit(&repo, start_point)?;
        repo.reference(&refname, commit.id(), true, "qit create branch")?;
        Self::set_ref_target(
            &repo,
            &Self::applied_refname_for(name),
            commit.id(),
            "qit create branch applied ref",
        )?;
        Ok(commit.id().to_string())
    }

    fn rename_branch(
        &self,
        old_name: &str,
        new_name: &str,
        force: bool,
    ) -> Result<(), GitStoreError> {
        if old_name == new_name {
            return Ok(());
        }

        let repo = self.open_repo()?;
        let old_refname = Self::branch_refname(old_name);
        let new_refname = Self::branch_refname(new_name);
        if repo.find_reference(&new_refname).is_ok() && !force {
            return Err(GitStoreError::BranchExists(new_name.to_string()));
        }
        if force && (new_name == self.checked_out_branch || new_name == self.exported_branch) {
            return Err(GitStoreError::Unsupported(format!(
                "refusing to overwrite protected branch `{new_name}`"
            )));
        }
        let mut branch = repo
            .find_reference(&old_refname)
            .map_err(|_| GitStoreError::RefNotFound(old_refname.clone()))?;
        if force && repo.find_reference(&new_refname).is_ok() {
            repo.find_reference(&new_refname)?.delete()?;
            if let Ok(mut applied) = repo.find_reference(&Self::applied_refname_for(new_name)) {
                applied.delete()?;
            }
        }
        branch.rename(&new_refname, false, "qit rename branch")?;

        let old_applied = Self::applied_refname_for(old_name);
        let new_applied = Self::applied_refname_for(new_name);
        if let Ok(mut applied) = repo.find_reference(&old_applied) {
            applied.rename(&new_applied, false, "qit rename applied branch")?;
        }

        if self.exported_branch == old_name {
            repo.set_head(&new_refname)?;
        }
        Ok(())
    }

    fn delete_branch(&self, name: &str, force: bool) -> Result<(), GitStoreError> {
        if name == self.checked_out_branch {
            return Err(GitStoreError::CurrentBranch(name.to_string()));
        }
        if name == self.exported_branch {
            return Err(GitStoreError::ServedBranch(name.to_string()));
        }

        let repo = self.open_repo()?;
        let branch_refname = Self::branch_refname(name);
        let mut branch = repo
            .find_reference(&branch_refname)
            .map_err(|_| GitStoreError::RefNotFound(branch_refname.clone()))?;

        if !force {
            let current = repo.find_reference(&self.host_refname())?;
            let current_commit = current.peel_to_commit()?;
            let branch_commit = branch.peel_to_commit()?;
            let merged = current_commit.id() == branch_commit.id()
                || repo.graph_descendant_of(current_commit.id(), branch_commit.id())?;
            if !merged {
                return Err(GitStoreError::BranchNotMerged(name.to_string()));
            }
        }

        branch.delete()?;
        if let Ok(mut applied) = repo.find_reference(&Self::applied_refname_for(name)) {
            applied.delete()?;
        }
        Ok(())
    }

    fn switch_branch(&self, name: &str) -> Result<String, GitStoreError> {
        let commit = self.checkout_branch(name, false)?;
        let repo = self.open_repo()?;
        repo.set_head(&Self::branch_refname(name))?;
        Ok(commit)
    }

    fn checkout_branch(&self, name: &str, force: bool) -> Result<String, GitStoreError> {
        let repo = self.open_repo()?;
        let target_refname = Self::branch_refname(name);
        let target = repo
            .find_reference(&target_refname)
            .map_err(|_| GitStoreError::RefNotFound(target_refname.clone()))?;
        let target_commit = target.peel_to_commit()?;
        let previous_commit = repo.find_reference(&self.host_refname())?.peel_to_commit()?;

        if name == self.checked_out_branch {
            return Ok(target_commit.id().to_string());
        }

        if !force && self.is_dirty_vs_applied()? {
            return Err(GitStoreError::DirtyWorktree);
        }

        let mut checkout = git2::build::CheckoutBuilder::new();
        checkout.force();
        repo.checkout_tree(target_commit.as_object(), Some(&mut checkout))?;
        if let Err(error) = Self::set_ref_target(
            &repo,
            &Self::applied_refname_for(name),
            target_commit.id(),
            "qit checkout_branch applied ref",
        ) {
            let mut rollback = git2::build::CheckoutBuilder::new();
            rollback.force();
            let _ = repo.checkout_tree(previous_commit.as_object(), Some(&mut rollback));
            return Err(error);
        }
        Ok(target_commit.id().to_string())
    }
}

#[async_trait]
impl RepoStore for GitRepoStore {
    async fn ensure_initialized(&self, workspace: &WorkspaceSpec) -> Result<(), RepositoryError> {
        tokio::task::spawn_blocking({
            let workspace = workspace.clone();
            move || ManagedWorkspace::init(&workspace).map(|_| ())
        })
        .await
        .map_err(|err| RepositoryError::Io {
            operation: "join repository initialization task",
            message: err.to_string(),
        })?
        .map_err(RepositoryError::from)
    }

    async fn snapshot(
        &self,
        workspace: &WorkspaceSpec,
        message: &str,
    ) -> Result<Option<String>, RepositoryError> {
        let message = message.to_string();
        tokio::task::spawn_blocking({
            let workspace = workspace.clone();
            move || {
                let mut managed = ManagedWorkspace::reopen(&workspace);
                managed.snapshot(&message)
            }
        })
        .await
        .map_err(|err| RepositoryError::Io {
            operation: "join snapshot task",
            message: err.to_string(),
        })?
        .map_err(RepositoryError::from)
    }

    async fn apply_fast_forward(
        &self,
        workspace: &WorkspaceSpec,
        source_ref: &str,
    ) -> Result<ApplyOutcome, RepositoryError> {
        let source_ref = source_ref.to_string();
        tokio::task::spawn_blocking({
            let workspace = workspace.clone();
            move || ManagedWorkspace::reopen(&workspace).apply_fast_forward(&source_ref)
        })
        .await
        .map_err(|err| RepositoryError::Io {
            operation: "join apply task",
            message: err.to_string(),
        })?
        .map_err(RepositoryError::from)
    }

    async fn list_branches(
        &self,
        workspace: &WorkspaceSpec,
    ) -> Result<Vec<BranchRecord>, RepositoryError> {
        tokio::task::spawn_blocking({
            let workspace = workspace.clone();
            move || ManagedWorkspace::reopen(&workspace).list_branches()
        })
        .await
        .map_err(|err| RepositoryError::Io {
            operation: "join branch listing task",
            message: err.to_string(),
        })?
        .map_err(RepositoryError::from)
    }

    async fn create_branch(
        &self,
        workspace: &WorkspaceSpec,
        name: &str,
        start_point: Option<&str>,
        force: bool,
    ) -> Result<String, RepositoryError> {
        let name = name.to_string();
        let start_point = start_point.map(ToString::to_string);
        tokio::task::spawn_blocking({
            let workspace = workspace.clone();
            move || {
                ManagedWorkspace::reopen(&workspace).create_branch(
                    &name,
                    start_point.as_deref(),
                    force,
                )
            }
        })
        .await
        .map_err(|err| RepositoryError::Io {
            operation: "join branch creation task",
            message: err.to_string(),
        })?
        .map_err(RepositoryError::from)
    }

    async fn rename_branch(
        &self,
        workspace: &WorkspaceSpec,
        old_name: &str,
        new_name: &str,
        force: bool,
    ) -> Result<(), RepositoryError> {
        let old_name = old_name.to_string();
        let new_name = new_name.to_string();
        tokio::task::spawn_blocking({
            let workspace = workspace.clone();
            move || ManagedWorkspace::reopen(&workspace).rename_branch(&old_name, &new_name, force)
        })
        .await
        .map_err(|err| RepositoryError::Io {
            operation: "join branch rename task",
            message: err.to_string(),
        })?
        .map_err(RepositoryError::from)
    }

    async fn delete_branch(
        &self,
        workspace: &WorkspaceSpec,
        name: &str,
        force: bool,
    ) -> Result<(), RepositoryError> {
        let name = name.to_string();
        tokio::task::spawn_blocking({
            let workspace = workspace.clone();
            move || ManagedWorkspace::reopen(&workspace).delete_branch(&name, force)
        })
        .await
        .map_err(|err| RepositoryError::Io {
            operation: "join branch delete task",
            message: err.to_string(),
        })?
        .map_err(RepositoryError::from)
    }

    async fn switch_branch(
        &self,
        workspace: &WorkspaceSpec,
        name: &str,
    ) -> Result<String, RepositoryError> {
        let name = name.to_string();
        tokio::task::spawn_blocking({
            let workspace = workspace.clone();
            move || ManagedWorkspace::reopen(&workspace).switch_branch(&name)
        })
        .await
        .map_err(|err| RepositoryError::Io {
            operation: "join branch switch task",
            message: err.to_string(),
        })?
        .map_err(RepositoryError::from)
    }

    async fn checkout_branch(
        &self,
        workspace: &WorkspaceSpec,
        name: &str,
        force: bool,
    ) -> Result<String, RepositoryError> {
        let name = name.to_string();
        tokio::task::spawn_blocking({
            let workspace = workspace.clone();
            move || ManagedWorkspace::reopen(&workspace).checkout_branch(&name, force)
        })
        .await
        .map_err(|err| RepositoryError::Io {
            operation: "join branch checkout task",
            message: err.to_string(),
        })?
        .map_err(RepositoryError::from)
    }
}

pub async fn run_git_http_backend(
    git_dir: &Path,
    request: GitHttpBackendRequest,
    mut body: BoxAsyncRead,
) -> Result<GitHttpBackendResponse, GitHttpBackendError> {
    let workspace = WorkspaceSpec {
        id: WorkspaceId(uuid::Uuid::nil()),
        worktree: git_dir.to_path_buf(),
        sidecar: git_dir.to_path_buf(),
        exported_branch: String::new(),
        checked_out_branch: String::new(),
    };
    let workspace_lock = lock_workspace(&workspace).map_err(|error| GitHttpBackendError::Io {
        operation: "lock workspace for git http-backend",
        message: error.to_string(),
    })?;
    if request.path_info.contains("git-receive-pack") && !request.allow_push {
        return Ok(GitHttpBackendResponse {
            status: 403,
            headers: vec![("Content-Type".into(), "text/plain".into())],
            body_prefix: b"push not permitted for this session".to_vec(),
            stdout: None,
            completion: None,
        });
    }

    let mut cmd = tokio::process::Command::new("git");
    cmd.args([
        "-c",
        "http.receivepack=true",
        "-c",
        "http.uploadpack=true",
        "http-backend",
    ]);
    let project_root = dunce::canonicalize(git_dir).unwrap_or_else(|_| git_dir.to_path_buf());
    cmd.env(
        "GIT_PROJECT_ROOT",
        project_root
            .to_str()
            .ok_or_else(|| GitHttpBackendError::Io {
                operation: "encode git project root",
                message: "git repo path is not valid UTF-8".into(),
            })?
            .to_string(),
    );
    cmd.env("GIT_HTTP_EXPORT_ALL", "1");
    cmd.env("REQUEST_METHOD", &request.method);
    cmd.env("PATH_INFO", &request.path_info);
    cmd.env("QUERY_STRING", request.query.as_deref().unwrap_or(""));
    cmd.env("SCRIPT_NAME", "");
    cmd.env("REMOTE_ADDR", "127.0.0.1");
    cmd.env("SERVER_PROTOCOL", "HTTP/1.1");
    cmd.env("SERVER_SOFTWARE", "qit/0.1");
    cmd.env(
        "REQUEST_SCHEME",
        if request.request_scheme == "https" {
            "https"
        } else {
            "http"
        },
    );

    for (key, value) in &request.headers {
        let upper = key.to_ascii_uppercase().replace('-', "_");
        let env_name = if upper == "CONTENT_TYPE" || upper == "CONTENT_LENGTH" {
            upper
        } else {
            format!("HTTP_{upper}")
        };
        cmd.env(env_name, value);
    }

    let buffered_body = if request.method.eq_ignore_ascii_case("POST") && request.content_length.is_none() {
        let mut bytes = Vec::new();
        body.read_to_end(&mut bytes)
            .await
            .map_err(|err| GitHttpBackendError::Io {
                operation: "buffer git http-backend request body",
                message: err.to_string(),
            })?;
        tracing::info!(
            path = %request.path_info,
            buffered_bytes = bytes.len(),
            "buffered chunked git HTTP request to synthesize Content-Length"
        );
        Some(bytes)
    } else {
        None
    };
    if request.method.eq_ignore_ascii_case("POST") {
        let content_length = request
            .content_length
            .or_else(|| buffered_body.as_ref().map(|bytes| bytes.len() as u64))
            .ok_or_else(|| GitHttpBackendError::Io {
                operation: "validate git http-backend request",
                message: "streaming git HTTP requests require Content-Length for POST".into(),
            })?;
        cmd.env("CONTENT_LENGTH", content_length.to_string());
    } else {
        cmd.env("CONTENT_LENGTH", "0");
    }

    cmd.stdin(Stdio::piped());
    cmd.stdout(Stdio::piped());
    cmd.stderr(Stdio::piped());

    let mut child = cmd.spawn().map_err(|err| {
        if err.kind() == std::io::ErrorKind::NotFound {
            GitHttpBackendError::GitNotFound
        } else {
            GitHttpBackendError::Io {
                operation: "spawn git http-backend",
                message: err.to_string(),
            }
        }
    })?;

    let mut stdin = child.stdin.take().ok_or_else(|| GitHttpBackendError::Io {
        operation: "capture git http-backend stdin",
        message: "stdin pipe missing".into(),
    })?;
    let mut stdout = child.stdout.take().ok_or_else(|| GitHttpBackendError::Io {
        operation: "capture git http-backend stdout",
        message: "stdout pipe missing".into(),
    })?;
    let mut stderr = child.stderr.take().ok_or_else(|| GitHttpBackendError::Io {
        operation: "capture git http-backend stderr",
        message: "stderr pipe missing".into(),
    })?;

    let stdin_task = tokio::spawn(async move {
        if let Some(bytes) = buffered_body {
            stdin
                .write_all(&bytes)
                .await
                .map_err(|err| GitHttpBackendError::Io {
                    operation: "write buffered request body to git http-backend",
                    message: err.to_string(),
                })?;
        } else {
            tokio::io::copy(&mut body, &mut stdin)
                .await
                .map_err(|err| GitHttpBackendError::Io {
                    operation: "stream request body to git http-backend",
                    message: err.to_string(),
                })?;
        }
        stdin.shutdown().await.map_err(|err| GitHttpBackendError::Io {
            operation: "close git http-backend stdin",
            message: err.to_string(),
        })
    });
    let stderr_task = tokio::spawn(async move {
        let mut bytes = Vec::new();
        stderr
            .read_to_end(&mut bytes)
            .await
            .map_err(|err| GitHttpBackendError::Io {
                operation: "read git http-backend stderr",
                message: err.to_string(),
            })?;
        Ok::<String, GitHttpBackendError>(String::from_utf8_lossy(&bytes).trim().to_string())
    });

    let (status, headers, body_prefix) = match read_streaming_cgi_response(&mut stdout).await {
        Ok(response) => response,
        Err(error) => {
            stdin_task.abort();
            stderr_task.abort();
            let _ = child.kill().await;
            let _ = child.wait().await;
            return Err(error);
        }
    };

    let completion = tokio::spawn(async move {
        let _workspace_lock = workspace_lock;
        match stdin_task.await {
            Ok(Ok(())) => {}
            Ok(Err(error)) => return Err(error),
            Err(error) => {
                return Err(GitHttpBackendError::Io {
                    operation: "join git http-backend stdin task",
                    message: error.to_string(),
                })
            }
        }

        let stderr = match stderr_task.await {
            Ok(Ok(stderr)) => stderr,
            Ok(Err(error)) => return Err(error),
            Err(error) => {
                return Err(GitHttpBackendError::Io {
                    operation: "join git http-backend stderr task",
                    message: error.to_string(),
                })
            }
        };

        let exit = child.wait().await.map_err(|error| GitHttpBackendError::Io {
            operation: "wait for git http-backend",
            message: error.to_string(),
        })?;
        if exit.success() {
            return Ok(());
        }
        tracing::warn!(status = %exit, stderr = %stderr, "git http-backend failed");
        Err(GitHttpBackendError::ProcessStatus(exit.to_string()))
    });

    Ok(GitHttpBackendResponse {
        status,
        headers,
        body_prefix,
        stdout: Some(stdout),
        completion: Some(completion),
    })
}

fn split_cgi_headers_body(raw: &[u8]) -> Option<(&[u8], &[u8])> {
    if let Some(index) = raw.windows(4).position(|window| window == b"\r\n\r\n") {
        return Some((&raw[..index], &raw[index + 4..]));
    }
    if let Some(index) = raw.windows(2).position(|window| window == b"\n\n") {
        return Some((&raw[..index], &raw[index + 2..]));
    }
    None
}

fn parse_cgi_headers(
    headers_bytes: &[u8],
) -> Result<(u16, Vec<(String, String)>), GitHttpBackendError> {
    let header_text =
        std::str::from_utf8(headers_bytes).map_err(|_| GitHttpBackendError::InvalidResponse)?;

    let mut status = 200u16;
    let mut headers = Vec::new();
    for line in header_text.lines() {
        let line = line.trim_end_matches('\r');
        if line.is_empty() {
            continue;
        }
        if let Some(rest) = line.strip_prefix("Status:") {
            status = rest
                .split_whitespace()
                .next()
                .and_then(|value| value.parse().ok())
                .unwrap_or(200);
            continue;
        }
        if let Some((key, value)) = line.split_once(':') {
            headers.push((key.trim().to_string(), value.trim().to_string()));
        }
    }

    Ok((status, headers))
}

#[cfg(test)]
fn parse_cgi_response(raw: &[u8]) -> Result<CgiHttpResult, GitHttpBackendError> {
    let (headers_bytes, body) =
        split_cgi_headers_body(raw).ok_or(GitHttpBackendError::InvalidResponse)?;
    let (status, headers) = parse_cgi_headers(headers_bytes)?;
    Ok((status, headers, body.to_vec()))
}

async fn read_streaming_cgi_response(
    stdout: &mut tokio::process::ChildStdout,
) -> Result<(u16, Vec<(String, String)>, Vec<u8>), GitHttpBackendError> {
    const MAX_CGI_HEADER_BYTES: usize = 64 * 1024;
    let mut buffer = Vec::new();
    let mut chunk = [0_u8; 8192];

    loop {
        if let Some((headers_bytes, body)) = split_cgi_headers_body(&buffer) {
            let (status, headers) = parse_cgi_headers(headers_bytes)?;
            return Ok((status, headers, body.to_vec()));
        }
        if buffer.len() >= MAX_CGI_HEADER_BYTES {
            return Err(GitHttpBackendError::InvalidResponse);
        }
        let read = stdout.read(&mut chunk).await.map_err(|err| GitHttpBackendError::Io {
            operation: "read git http-backend stdout",
            message: err.to_string(),
        })?;
        if read == 0 {
            return Err(GitHttpBackendError::InvalidResponse);
        }
        buffer.extend_from_slice(&chunk[..read]);
    }
}


#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::sync::{Mutex, OnceLock};
    use uuid::Uuid;

    fn env_lock() -> &'static Mutex<()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(()))
    }

    #[test]
    fn snapshot_applies_gitignore_when_worktree_has_no_dot_git() {
        let base = tempfile::tempdir().unwrap();
        let worktree = base.path().join("wt");
        let sidecar = base.path().join("side.git");
        fs::create_dir_all(&worktree).unwrap();
        fs::write(worktree.join(".gitignore"), "*.ignored\n").unwrap();
        fs::write(worktree.join("visible.txt"), "v").unwrap();
        fs::write(worktree.join("x.ignored"), "big").unwrap();

        let workspace = WorkspaceSpec {
            id: WorkspaceId(Uuid::nil()),
            worktree: worktree.clone(),
            sidecar: sidecar.clone(),
            exported_branch: "main".into(),
            checked_out_branch: "main".into(),
        };
        let managed = ManagedWorkspace::init(&workspace).unwrap();
        let repo = managed.open_repo().unwrap();
        let head = repo.find_reference("refs/heads/main").unwrap();
        let commit = head.peel_to_commit().unwrap();
        let tree = commit.tree().unwrap();
        let names: Vec<String> = tree
            .iter()
            .map(|entry| entry.name().unwrap().to_string())
            .collect();

        assert!(names.contains(&"visible.txt".to_string()));
        assert!(!names.iter().any(|name| name.ends_with(".ignored")));
        assert_eq!(workspace.id, WorkspaceId(Uuid::nil()));
    }

    #[test]
    fn snapshot_keeps_vendor_directories_as_user_content() {
        let base = tempfile::tempdir().unwrap();
        let worktree = base.path().join("wt");
        let sidecar = base.path().join("side.git");
        fs::create_dir_all(worktree.join("node_modules/pkg")).unwrap();
        fs::write(worktree.join("node_modules/pkg/index.js"), "module.exports = 1;\n").unwrap();

        let workspace = WorkspaceSpec {
            id: WorkspaceId(Uuid::nil()),
            worktree: worktree.clone(),
            sidecar,
            exported_branch: "main".into(),
            checked_out_branch: "main".into(),
        };
        let managed = ManagedWorkspace::init(&workspace).unwrap();
        let repo = managed.open_repo().unwrap();
        let head = repo.find_reference("refs/heads/main").unwrap();
        let commit = head.peel_to_commit().unwrap();
        let tree = commit.tree().unwrap();
        let modules = tree.get_name("node_modules").unwrap().to_object(&repo).unwrap();
        let modules = modules.peel_to_tree().unwrap();
        let pkg = modules.get_name("pkg").unwrap().to_object(&repo).unwrap();
        let pkg = pkg.peel_to_tree().unwrap();

        assert!(pkg.get_name("index.js").is_some());
    }

    #[test]
    fn snapshot_ignores_machine_global_gitignore_rules() {
        let _guard = env_lock().lock().unwrap();
        let base = tempfile::tempdir().unwrap();
        let worktree = base.path().join("wt");
        let sidecar = base.path().join("side.git");
        let xdg = base.path().join("xdg");
        fs::create_dir_all(xdg.join("git")).unwrap();
        fs::create_dir_all(&worktree).unwrap();
        fs::write(xdg.join("git/ignore"), "*.global\n").unwrap();
        fs::write(worktree.join("tracked.global"), "keep me\n").unwrap();

        let old_xdg = std::env::var_os("XDG_CONFIG_HOME");
        let old_home = std::env::var_os("HOME");
        std::env::set_var("XDG_CONFIG_HOME", &xdg);
        std::env::set_var("HOME", base.path());

        let workspace = WorkspaceSpec {
            id: WorkspaceId(Uuid::nil()),
            worktree: worktree.clone(),
            sidecar,
            exported_branch: "main".into(),
            checked_out_branch: "main".into(),
        };
        let managed = ManagedWorkspace::init(&workspace).unwrap();
        let repo = managed.open_repo().unwrap();
        let head = repo.find_reference("refs/heads/main").unwrap();
        let commit = head.peel_to_commit().unwrap();
        let tree = commit.tree().unwrap();

        match old_xdg {
            Some(value) => std::env::set_var("XDG_CONFIG_HOME", value),
            None => std::env::remove_var("XDG_CONFIG_HOME"),
        }
        match old_home {
            Some(value) => std::env::set_var("HOME", value),
            None => std::env::remove_var("HOME"),
        }

        assert!(tree.get_name("tracked.global").is_some());
    }

    #[test]
    fn apply_fast_forward_uses_applied_ref_not_latest_head() {
        let base = tempfile::tempdir().unwrap();
        let worktree = base.path().join("wt");
        let remote_worktree = base.path().join("remote");
        let sidecar = base.path().join("side.git");
        fs::create_dir_all(&worktree).unwrap();
        fs::create_dir_all(&remote_worktree).unwrap();
        fs::write(worktree.join("file.txt"), "v1\n").unwrap();
        fs::write(remote_worktree.join("file.txt"), "v2\n").unwrap();

        let workspace = WorkspaceSpec {
            id: WorkspaceId(Uuid::nil()),
            worktree: worktree.clone(),
            sidecar: sidecar.clone(),
            exported_branch: "main".into(),
            checked_out_branch: "main".into(),
        };
        let managed = ManagedWorkspace::init(&workspace).unwrap();

        let repo = Repository::open_bare(&sidecar).unwrap();
        repo.set_workdir(&remote_worktree, false).unwrap();
        let mut index = repo.index().unwrap();
        index.clear().unwrap();
        index.add_path(Path::new("file.txt")).unwrap();
        let tree_id = index.write_tree().unwrap();
        let tree = repo.find_tree(tree_id).unwrap();
        let parent = repo
            .find_reference("refs/heads/main")
            .unwrap()
            .peel_to_commit()
            .unwrap();
        let sig = Signature::now("tester", "tester@example.com").unwrap();
        repo.commit(
            Some("refs/heads/main"),
            &sig,
            &sig,
            "remote push",
            &tree,
            &[&parent],
        )
        .unwrap();

        let outcome = managed.apply_fast_forward("refs/heads/main").unwrap();
        assert_eq!(outcome.merged_to, "main");
        assert_eq!(
            fs::read_to_string(worktree.join("file.txt")).unwrap(),
            "v2\n"
        );
    }

    #[cfg(unix)]
    #[test]
    fn apply_fast_forward_rolls_back_when_applied_ref_update_fails() {
        use std::os::unix::fs::PermissionsExt;

        let base = tempfile::tempdir().unwrap();
        let worktree = base.path().join("wt");
        let remote_worktree = base.path().join("remote");
        let sidecar = base.path().join("side.git");
        fs::create_dir_all(&worktree).unwrap();
        fs::create_dir_all(&remote_worktree).unwrap();
        fs::write(worktree.join("file.txt"), "v1\n").unwrap();
        fs::write(remote_worktree.join("file.txt"), "v2\n").unwrap();

        let workspace = WorkspaceSpec {
            id: WorkspaceId(Uuid::nil()),
            worktree: worktree.clone(),
            sidecar: sidecar.clone(),
            exported_branch: "main".into(),
            checked_out_branch: "main".into(),
        };
        let managed = ManagedWorkspace::init(&workspace).unwrap();

        let repo = Repository::open_bare(&sidecar).unwrap();
        repo.set_workdir(&remote_worktree, false).unwrap();
        let original_commit = repo
            .find_reference("refs/heads/main")
            .unwrap()
            .peel_to_commit()
            .unwrap();
        let mut index = repo.index().unwrap();
        index.clear().unwrap();
        index.add_path(Path::new("file.txt")).unwrap();
        let tree_id = index.write_tree().unwrap();
        let tree = repo.find_tree(tree_id).unwrap();
        let sig = Signature::now("tester", "tester@example.com").unwrap();
        let next_commit = repo
            .commit(
                Some("refs/heads/main"),
                &sig,
                &sig,
                "remote push",
                &tree,
                &[&original_commit],
            )
            .unwrap();

        let applied_dir = sidecar.join("refs/qit/applied");
        let applied_ref = applied_dir.join("main");
        let original_dir_mode = fs::metadata(&applied_dir).unwrap().permissions().mode();
        let original_ref_mode = fs::metadata(&applied_ref).unwrap().permissions().mode();
        fs::set_permissions(&applied_dir, fs::Permissions::from_mode(0o500)).unwrap();
        fs::set_permissions(&applied_ref, fs::Permissions::from_mode(0o400)).unwrap();

        let result = managed.apply_fast_forward("refs/heads/main");

        fs::set_permissions(&applied_ref, fs::Permissions::from_mode(original_ref_mode)).unwrap();
        fs::set_permissions(&applied_dir, fs::Permissions::from_mode(original_dir_mode)).unwrap();

        assert!(matches!(result, Err(GitStoreError::Git(_))));
        assert_eq!(
            repo.find_reference("refs/heads/main")
                .unwrap()
                .peel_to_commit()
                .unwrap()
                .id(),
            next_commit
        );
        assert_eq!(
            fs::read_to_string(worktree.join("file.txt")).unwrap(),
            "v1\n"
        );
    }

    #[test]
    fn checkout_branch_rejects_untracked_dirty_worktree() {
        let base = tempfile::tempdir().unwrap();
        let worktree = base.path().join("wt");
        let sidecar = base.path().join("side.git");
        fs::create_dir_all(&worktree).unwrap();
        fs::write(worktree.join("file.txt"), "main\n").unwrap();

        let workspace = WorkspaceSpec {
            id: WorkspaceId(Uuid::nil()),
            worktree: worktree.clone(),
            sidecar,
            exported_branch: "main".into(),
            checked_out_branch: "main".into(),
        };
        let managed = ManagedWorkspace::init(&workspace).unwrap();
        managed.create_branch("feature", None, false).unwrap();
        fs::write(worktree.join("new.txt"), "untracked\n").unwrap();

        assert!(matches!(
            managed.checkout_branch("feature", false),
            Err(GitStoreError::DirtyWorktree)
        ));
    }

    #[test]
    fn parse_cgi_response_supports_lf_and_crlf() {
        let lf = b"Content-Type: text/plain\n\nhello";
        let crlf = b"Content-Type: text/plain\r\n\r\nhello";

        let (_, _, lf_body) = parse_cgi_response(lf).unwrap();
        let (_, _, crlf_body) = parse_cgi_response(crlf).unwrap();

        assert_eq!(lf_body, b"hello");
        assert_eq!(crlf_body, b"hello");
    }

    #[test]
    fn branch_management_lists_switches_and_deletes_sidecar_branches() {
        let base = tempfile::tempdir().unwrap();
        let worktree = base.path().join("wt");
        let remote_worktree = base.path().join("remote");
        let sidecar = base.path().join("side.git");
        std::fs::create_dir_all(&worktree).unwrap();
        std::fs::create_dir_all(&remote_worktree).unwrap();
        std::fs::write(worktree.join("file.txt"), "main\n").unwrap();
        std::fs::write(remote_worktree.join("file.txt"), "feature\n").unwrap();

        let workspace = WorkspaceSpec {
            id: WorkspaceId(Uuid::nil()),
            worktree: worktree.clone(),
            sidecar: sidecar.clone(),
            exported_branch: "main".into(),
            checked_out_branch: "main".into(),
        };
        let managed = ManagedWorkspace::init(&workspace).unwrap();
        let main_commit = managed.create_branch("feature", None, false).unwrap();
        let mut branches = managed
            .list_branches()
            .unwrap()
            .into_iter()
            .map(|record| record.name)
            .collect::<Vec<_>>();
        assert_eq!(branches, vec!["feature".to_string(), "main".to_string()]);

        let repo = Repository::open_bare(&sidecar).unwrap();
        repo.set_workdir(&remote_worktree, false).unwrap();
        let mut index = repo.index().unwrap();
        index.clear().unwrap();
        index.add_path(Path::new("file.txt")).unwrap();
        let tree_id = index.write_tree().unwrap();
        let tree = repo.find_tree(tree_id).unwrap();
        let parent = repo
            .find_reference("refs/heads/feature")
            .unwrap()
            .peel_to_commit()
            .unwrap();
        let sig = Signature::now("tester", "tester@example.com").unwrap();
        let feature_commit = repo
            .commit(
                Some("refs/heads/feature"),
                &sig,
                &sig,
                "feature update",
                &tree,
                &[&parent],
            )
            .unwrap();

        let switched = managed.switch_branch("feature").unwrap();
        assert_eq!(switched, feature_commit.to_string());
        assert_eq!(
            std::fs::read_to_string(worktree.join("file.txt")).unwrap(),
            "feature\n"
        );

        managed.rename_branch("feature", "renamed", false).unwrap();
        branches = managed
            .list_branches()
            .unwrap()
            .into_iter()
            .map(|record| record.name)
            .collect::<Vec<_>>();
        assert_eq!(branches, vec!["main".to_string(), "renamed".to_string()]);

        assert!(matches!(
            managed.delete_branch("renamed", false),
            Err(GitStoreError::BranchNotMerged(name)) if name == "renamed"
        ));
        managed.delete_branch("renamed", true).unwrap();
        branches = managed
            .list_branches()
            .unwrap()
            .into_iter()
            .map(|record| record.name)
            .collect::<Vec<_>>();
        assert_eq!(branches, vec!["main".to_string()]);
        assert_eq!(main_commit.len(), 40);
    }

    #[test]
    fn init_rejects_missing_worktree() {
        let base = tempfile::tempdir().unwrap();
        let worktree = base.path().join("missing");
        let sidecar = base.path().join("side.git");
        let workspace = WorkspaceSpec {
            id: WorkspaceId(Uuid::nil()),
            worktree: worktree.clone(),
            sidecar,
            exported_branch: "main".into(),
            checked_out_branch: "main".into(),
        };

        assert!(matches!(
            ManagedWorkspace::init(&workspace),
            Err(GitStoreError::MissingWorktree(path)) if path == worktree
        ));
    }

    #[test]
    fn force_rename_validates_source_before_deleting_destination() {
        let base = tempfile::tempdir().unwrap();
        let worktree = base.path().join("wt");
        let sidecar = base.path().join("side.git");
        std::fs::create_dir_all(&worktree).unwrap();
        std::fs::write(worktree.join("file.txt"), "main\n").unwrap();

        let workspace = WorkspaceSpec {
            id: WorkspaceId(Uuid::nil()),
            worktree: worktree.clone(),
            sidecar: sidecar.clone(),
            exported_branch: "main".into(),
            checked_out_branch: "main".into(),
        };
        let managed = ManagedWorkspace::init(&workspace).unwrap();
        managed.create_branch("existing", None, false).unwrap();

        assert!(matches!(
            managed.rename_branch("missing", "existing", true),
            Err(GitStoreError::RefNotFound(name)) if name == "refs/heads/missing"
        ));

        let repo = Repository::open_bare(&sidecar).unwrap();
        assert!(repo.find_reference("refs/heads/existing").is_ok());
        assert!(repo.find_reference("refs/qit/applied/existing").is_ok());
    }

    #[test]
    fn rename_branch_rejects_existing_destination_without_force_and_protected_overwrite() {
        let base = tempfile::tempdir().unwrap();
        let worktree = base.path().join("wt");
        let sidecar = base.path().join("side.git");
        std::fs::create_dir_all(&worktree).unwrap();
        std::fs::write(worktree.join("file.txt"), "main\n").unwrap();

        let workspace = WorkspaceSpec {
            id: WorkspaceId(Uuid::nil()),
            worktree: worktree.clone(),
            sidecar,
            exported_branch: "main".into(),
            checked_out_branch: "main".into(),
        };
        let managed = ManagedWorkspace::init(&workspace).unwrap();
        managed.create_branch("feature", None, false).unwrap();
        managed.create_branch("other", None, false).unwrap();

        assert!(matches!(
            managed.rename_branch("feature", "other", false),
            Err(GitStoreError::BranchExists(name)) if name == "other"
        ));
        assert!(matches!(
            managed.rename_branch("feature", "main", true),
            Err(GitStoreError::Unsupported(message))
                if message == "refusing to overwrite protected branch `main`"
        ));
    }

    #[test]
    fn switch_branch_rejects_dirty_worktree_and_current_delete() {
        let base = tempfile::tempdir().unwrap();
        let worktree = base.path().join("wt");
        let sidecar = base.path().join("side.git");
        std::fs::create_dir_all(&worktree).unwrap();
        std::fs::write(worktree.join("file.txt"), "main\n").unwrap();

        let workspace = WorkspaceSpec {
            id: WorkspaceId(Uuid::nil()),
            worktree: worktree.clone(),
            sidecar: sidecar.clone(),
            exported_branch: "main".into(),
            checked_out_branch: "main".into(),
        };
        let managed = ManagedWorkspace::init(&workspace).unwrap();
        managed.create_branch("feature", None, false).unwrap();
        std::fs::write(worktree.join("file.txt"), "dirty\n").unwrap();

        assert!(matches!(
            managed.switch_branch("feature"),
            Err(GitStoreError::DirtyWorktree)
        ));
        assert!(matches!(
            managed.delete_branch("main", true),
            Err(GitStoreError::CurrentBranch(name)) if name == "main"
        ));
    }

    #[test]
    fn checkout_branch_keeps_served_head_on_exported_branch() {
        let base = tempfile::tempdir().unwrap();
        let worktree = base.path().join("wt");
        let remote_worktree = base.path().join("remote");
        let sidecar = base.path().join("side.git");
        std::fs::create_dir_all(&worktree).unwrap();
        std::fs::create_dir_all(&remote_worktree).unwrap();
        std::fs::write(worktree.join("file.txt"), "main\n").unwrap();
        std::fs::write(remote_worktree.join("file.txt"), "feature\n").unwrap();

        let workspace = WorkspaceSpec {
            id: WorkspaceId(Uuid::nil()),
            worktree: worktree.clone(),
            sidecar: sidecar.clone(),
            exported_branch: "main".into(),
            checked_out_branch: "main".into(),
        };
        let managed = ManagedWorkspace::init(&workspace).unwrap();
        managed.create_branch("feature", None, false).unwrap();

        let repo = Repository::open_bare(&sidecar).unwrap();
        repo.set_workdir(&remote_worktree, false).unwrap();
        let mut index = repo.index().unwrap();
        index.clear().unwrap();
        index.add_path(Path::new("file.txt")).unwrap();
        let tree_id = index.write_tree().unwrap();
        let tree = repo.find_tree(tree_id).unwrap();
        let parent = repo
            .find_reference("refs/heads/feature")
            .unwrap()
            .peel_to_commit()
            .unwrap();
        let sig = Signature::now("tester", "tester@example.com").unwrap();
        repo.commit(
            Some("refs/heads/feature"),
            &sig,
            &sig,
            "feature update",
            &tree,
            &[&parent],
        )
        .unwrap();

        managed.checkout_branch("feature", false).unwrap();
        assert_eq!(repo.head().unwrap().name(), Some("refs/heads/main"));
        assert_eq!(
            std::fs::read_to_string(worktree.join("file.txt")).unwrap(),
            "feature\n"
        );
    }

    #[test]
    fn create_branch_can_use_explicit_start_point() {
        let base = tempfile::tempdir().unwrap();
        let worktree = base.path().join("wt");
        let remote_worktree = base.path().join("remote");
        let sidecar = base.path().join("side.git");
        std::fs::create_dir_all(&worktree).unwrap();
        std::fs::create_dir_all(&remote_worktree).unwrap();
        std::fs::write(worktree.join("file.txt"), "main\n").unwrap();
        std::fs::write(remote_worktree.join("file.txt"), "feature\n").unwrap();

        let workspace = WorkspaceSpec {
            id: WorkspaceId(Uuid::nil()),
            worktree: worktree.clone(),
            sidecar: sidecar.clone(),
            exported_branch: "main".into(),
            checked_out_branch: "main".into(),
        };
        let managed = ManagedWorkspace::init(&workspace).unwrap();
        managed.create_branch("feature", None, false).unwrap();

        let repo = Repository::open_bare(&sidecar).unwrap();
        repo.set_workdir(&remote_worktree, false).unwrap();
        let mut index = repo.index().unwrap();
        index.clear().unwrap();
        index.add_path(Path::new("file.txt")).unwrap();
        let tree_id = index.write_tree().unwrap();
        let tree = repo.find_tree(tree_id).unwrap();
        let parent = repo
            .find_reference("refs/heads/feature")
            .unwrap()
            .peel_to_commit()
            .unwrap();
        let sig = Signature::now("tester", "tester@example.com").unwrap();
        let feature_commit = repo
            .commit(
                Some("refs/heads/feature"),
                &sig,
                &sig,
                "feature update",
                &tree,
                &[&parent],
            )
            .unwrap();

        managed
            .create_branch("release", Some("refs/heads/feature"), false)
            .unwrap();
        let release_commit = repo
            .find_reference("refs/heads/release")
            .unwrap()
            .peel_to_commit()
            .unwrap();

        assert_eq!(release_commit.id(), feature_commit);
    }
}
