use anyhow::{anyhow, bail, Context, Result};
use clap::{ArgAction, Parser, Subcommand, ValueEnum};
use qit_domain::{
    resolve_pull_request_refs, BranchInfo, CreatePullRequest, CreatePullRequestComment,
    CreatePullRequestReview, CredentialIssuer, PullRequestRecord, PullRequestReviewState,
    PullRequestStatus, RefComparison, RefDiffFile, RepoReadStore, SessionCredentials, UiRole,
    UpdatePullRequest, WorkspaceService, WorkspaceSpec, DEFAULT_BRANCH,
};
use qit_git::{GitHttpBackendAdapter, GitRepoStore};
use qit_http::{repo_mount_path, GitHttpServer, GitHttpServerConfig, DEFAULT_MAX_BODY_BYTES};
use qit_storage::FilesystemRegistry;
use qit_transports::{expose, PublicTransport};
use qit_webui::{WebUiConfig, WebUiServer};
use rand::distributions::{Alphanumeric, DistString};
use similar::TextDiff;
use std::io::Write;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::Arc;
use tokio::net::TcpListener;
use tokio::time::{timeout, Duration};
use tracing_subscriber::EnvFilter;
use url::Url;

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
enum TransportArg {
    Ngrok,
    Tailscale,
    Local,
}

impl From<TransportArg> for PublicTransport {
    fn from(value: TransportArg) -> Self {
        match value {
            TransportArg::Ngrok => PublicTransport::Ngrok,
            TransportArg::Tailscale => PublicTransport::Tailscale,
            TransportArg::Local => PublicTransport::Local,
        }
    }
}

#[derive(Parser)]
#[command(name = "qit")]
#[command(about = "Host a folder as an authenticated Git remote.")]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,

    /// Folder to publish.
    path: Option<PathBuf>,

    /// Allow serving a folder that already contains its own `.git` directory.
    #[arg(long)]
    allow_existing_git: bool,

    /// Public transport to expose the repo.
    #[arg(long, value_enum)]
    transport: Option<TransportArg>,

    /// After successful pushes, fast-forward the host folder when it is clean.
    #[arg(long)]
    auto_apply: bool,

    /// Show the password in stdout and embed it in the suggested clone command.
    #[arg(long)]
    show_pass: bool,

    /// Backward-compatible no-op alias; credentials are hidden by default.
    #[arg(long, hide = true, conflicts_with = "show_pass")]
    hidden_pass: bool,

    /// Local port for Git Smart HTTP.
    #[arg(long, default_value_t = 8080, hide = true)]
    port: u16,

    /// Exported branch name.
    #[arg(long, hide = true)]
    branch: Option<String>,

    /// Maximum accepted Git HTTP request body size in bytes.
    #[arg(long, default_value_t = DEFAULT_MAX_BODY_BYTES, hide = true)]
    max_body_bytes: usize,

    /// Backward-compatible alias for `--transport local`.
    #[arg(long, hide = true)]
    local_only: bool,
}

#[derive(Subcommand)]
enum Commands {
    /// Fast-forward the host folder to the latest exported branch in the sidecar repo.
    Apply {
        /// Folder previously published with qit.
        path: PathBuf,

        /// Apply a specific branch from the sidecar repo instead of the exported branch.
        #[arg(long)]
        branch: Option<String>,
    },
    /// List or manage sidecar branches for a served folder.
    Branch {
        /// Folder previously published with qit.
        path: PathBuf,

        /// Create a branch.
        #[arg(conflicts_with_all = ["list", "rename", "rename_force", "copy", "copy_force", "delete", "delete_force"])]
        name: Option<String>,

        /// Start point for a new branch.
        #[arg(requires = "name", conflicts_with_all = ["list", "rename", "rename_force", "copy", "copy_force", "delete", "delete_force"])]
        start_point: Option<String>,

        /// List branches matching optional patterns.
        #[arg(long, value_name = "PATTERN", num_args = 0.., action = ArgAction::Append, conflicts_with_all = ["name", "start_point", "rename", "rename_force", "copy", "copy_force", "delete", "delete_force"])]
        list: Vec<String>,

        /// Show commit details when listing branches.
        #[arg(short = 'v', action = ArgAction::Count)]
        verbose: u8,

        /// Rename a branch.
        #[arg(
            short = 'm',
            value_names = ["OLD", "NEW"],
            num_args = 2,
            conflicts_with_all = ["name", "start_point", "list", "rename_force", "copy", "copy_force", "delete", "delete_force"]
        )]
        rename: Option<Vec<String>>,

        /// Rename a branch, overwriting the destination if needed.
        #[arg(
            short = 'M',
            value_names = ["OLD", "NEW"],
            num_args = 2,
            conflicts_with_all = ["name", "start_point", "list", "rename", "copy", "copy_force", "delete", "delete_force"]
        )]
        rename_force: Option<Vec<String>>,

        /// Copy a branch.
        #[arg(
            short = 'c',
            value_names = ["OLD", "NEW"],
            num_args = 2,
            conflicts_with_all = ["name", "start_point", "list", "rename", "rename_force", "copy_force", "delete", "delete_force"]
        )]
        copy: Option<Vec<String>>,

        /// Copy a branch, overwriting the destination if needed.
        #[arg(
            short = 'C',
            value_names = ["OLD", "NEW"],
            num_args = 2,
            conflicts_with_all = ["name", "start_point", "list", "rename", "rename_force", "copy", "delete", "delete_force"]
        )]
        copy_force: Option<Vec<String>>,

        /// Delete a fully merged branch.
        #[arg(short = 'd', value_name = "NAME", conflicts_with_all = ["name", "start_point", "list", "rename", "rename_force", "copy", "copy_force", "delete_force"])]
        delete: Option<String>,

        /// Delete a branch even if it is not merged.
        #[arg(short = 'D', value_name = "NAME", conflicts_with_all = ["name", "start_point", "list", "rename", "rename_force", "copy", "copy_force", "delete"])]
        delete_force: Option<String>,
    },
    /// Switch the served folder to a sidecar branch.
    Switch {
        /// Folder previously published with qit.
        path: PathBuf,

        /// Branch to switch the host folder to.
        branch: String,
    },
    /// Check out the host folder to a sidecar branch without changing the served default branch.
    Checkout {
        /// Folder previously published with qit.
        path: PathBuf,

        /// Create and check out a new branch.
        #[arg(short = 'b', value_name = "NEW_BRANCH", conflicts_with_all = ["create_force", "detach", "merge"])]
        create: Option<String>,

        /// Create or reset and check out a branch.
        #[arg(short = 'B', value_name = "NEW_BRANCH", conflicts_with_all = ["create", "detach", "merge"])]
        create_force: Option<String>,

        /// Force checkout even if the worktree is dirty relative to the applied ref.
        #[arg(short = 'f')]
        force: bool,

        /// Accept Git-style tracking syntax without persisting upstream metadata.
        #[arg(long, conflicts_with = "no_track")]
        track: bool,

        /// Accept Git-style no-track syntax.
        #[arg(long, conflicts_with = "track")]
        no_track: bool,

        /// Merge-style checkout is not supported for served workspaces.
        #[arg(short = 'm')]
        merge: bool,

        /// Detached checkout is not supported for served workspaces.
        #[arg(long)]
        detach: bool,

        /// Branch to check out, or start-point when used with `-b` / `-B`.
        target: Option<String>,

        /// Path checkout is not supported for served workspaces.
        #[arg(last = true)]
        paths: Vec<String>,
    },
    /// Manage pull requests using GitHub CLI-style subcommands.
    Pr {
        #[command(subcommand)]
        command: PrCommands,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
enum PrStateArg {
    Open,
    Closed,
    Merged,
    All,
}

#[derive(Subcommand)]
enum PrCommands {
    /// List pull requests for a served folder.
    List {
        /// Folder previously published with qit.
        path: PathBuf,

        /// Filter by pull request state.
        #[arg(long, default_value = "open")]
        state: PrStateArg,
    },
    /// Show a pull request summary, discussion, reviews, and diff stats.
    View {
        /// Folder previously published with qit.
        path: PathBuf,

        /// Pull request id or unique id prefix.
        pull_request: String,
    },
    /// Show a patch-style diff for a pull request.
    Diff {
        /// Folder previously published with qit.
        path: PathBuf,

        /// Pull request id or unique id prefix.
        pull_request: String,
    },
    /// Show pull request status for the current and served branches.
    Status {
        /// Folder previously published with qit.
        path: PathBuf,
    },
    /// Create a pull request.
    Create {
        /// Folder previously published with qit.
        path: PathBuf,

        /// Pull request title.
        #[arg(long)]
        title: String,

        /// Pull request body/description.
        #[arg(long, default_value = "")]
        body: String,

        /// Source branch. Defaults to the checked out branch.
        #[arg(long)]
        head: Option<String>,

        /// Target branch. Defaults to the served branch.
        #[arg(long)]
        base: Option<String>,
    },
    /// Edit pull request details.
    Edit {
        /// Folder previously published with qit.
        path: PathBuf,

        /// Pull request id or unique id prefix.
        pull_request: String,

        /// Replace the title.
        #[arg(long)]
        title: Option<String>,

        /// Replace the body/description.
        #[arg(long)]
        body: Option<String>,

        /// Mark the pull request open.
        #[arg(long, conflicts_with = "close")]
        open: bool,

        /// Mark the pull request closed.
        #[arg(long, conflicts_with = "open")]
        close: bool,
    },
    /// Add a top-level comment to a pull request.
    Comment {
        /// Folder previously published with qit.
        path: PathBuf,

        /// Pull request id or unique id prefix.
        pull_request: String,

        /// Comment body.
        #[arg(long)]
        body: String,

        /// Display name to record with the comment.
        #[arg(long)]
        author: Option<String>,
    },
    /// Submit a pull request review.
    Review {
        /// Folder previously published with qit.
        path: PathBuf,

        /// Pull request id or unique id prefix.
        pull_request: String,

        /// Approve the pull request.
        #[arg(long, conflicts_with_all = ["request_changes", "comment"])]
        approve: bool,

        /// Request changes on the pull request.
        #[arg(long, conflicts_with_all = ["approve", "comment"])]
        request_changes: bool,

        /// Leave a comment-only review.
        #[arg(long, conflicts_with_all = ["approve", "request_changes"])]
        comment: bool,

        /// Review body.
        #[arg(long, default_value = "")]
        body: String,

        /// Display name to record with the review.
        #[arg(long)]
        author: Option<String>,
    },
    /// Merge a pull request.
    Merge {
        /// Folder previously published with qit.
        path: PathBuf,

        /// Pull request id or unique id prefix.
        pull_request: String,
    },
    /// Close a pull request.
    Close {
        /// Folder previously published with qit.
        path: PathBuf,

        /// Pull request id or unique id prefix.
        pull_request: String,
    },
    /// Reopen a pull request.
    Reopen {
        /// Folder previously published with qit.
        path: PathBuf,

        /// Pull request id or unique id prefix.
        pull_request: String,
    },
    /// Delete a pull request.
    Delete {
        /// Folder previously published with qit.
        path: PathBuf,

        /// Pull request id or unique id prefix.
        pull_request: String,
    },
}

struct RandomCredentialIssuer;

impl CredentialIssuer for RandomCredentialIssuer {
    fn issue(&self) -> SessionCredentials {
        let mut rng = rand::thread_rng();
        SessionCredentials {
            username: format!(
                "qit-{}",
                Alphanumeric.sample_string(&mut rng, 8).to_lowercase()
            ),
            password: Alphanumeric.sample_string(&mut rng, 24),
        }
    }
}

fn say(message: &str) {
    println!("{message}");
    let _ = std::io::stdout().flush();
}

fn repo_name_from_worktree(worktree: &std::path::Path) -> String {
    worktree
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty())
        .unwrap_or("repo")
        .to_string()
}

fn repo_url(base: &Url, repo_mount_path: &str) -> Result<Url> {
    let mut url = base.clone();
    url.set_path(repo_mount_path);
    Ok(url)
}

fn clone_command(public_url: &Url, transport: PublicTransport) -> String {
    let repo_url = public_url.as_str().trim_end_matches('/').to_string();
    match transport {
        PublicTransport::Ngrok => {
            format!("git -c http.extraHeader=\"ngrok-skip-browser-warning: 1\" clone {repo_url}/")
        }
        _ => format!("git clone {repo_url}/"),
    }
}

fn repo_url_with_credentials(
    public_url: &Url,
    credentials: &SessionCredentials,
    show_pass: bool,
) -> Result<Url> {
    let mut url = public_url.clone();
    if !show_pass {
        return Ok(url);
    }
    url.set_username(&credentials.username)
        .map_err(|_| anyhow!("failed to encode username in clone URL"))?;
    url.set_password(Some(&credentials.password))
        .map_err(|_| anyhow!("failed to encode password in clone URL"))?;
    Ok(url)
}

fn write_credentials_file(credentials: &SessionCredentials) -> Result<PathBuf> {
    let mut file = tempfile::Builder::new()
        .prefix("qit-credentials-")
        .suffix(".txt")
        .tempfile()
        .context("create credentials file")?;
    writeln!(file, "username: {}", credentials.username)?;
    writeln!(file, "password: {}", credentials.password)?;
    file.flush()?;
    let (_persisted, path) = file
        .keep()
        .map_err(|error| anyhow::Error::new(error.error))?;
    Ok(path)
}

fn print_serve_summary(
    worktree: &Path,
    exported_branch: &str,
    label: &str,
    public_url: &Url,
    local_browser_url: &Url,
    browser_url: &Url,
    credentials: &SessionCredentials,
    credentials_path: &Path,
    show_pass: bool,
    clone_cmd: &str,
    auto_apply: bool,
) {
    println!();
    println!("Serving");
    println!("  path: {}", worktree.display());
    println!("  branch: {exported_branch}");
    println!("  transport: {}", label.to_ascii_lowercase());
    if auto_apply {
        println!("  auto-apply: on");
    }
    println!();
    println!("Web UI");
    println!("  local: {}", local_browser_url.as_str().trim_end_matches('/'));
    if local_browser_url != browser_url {
        println!("  public: {}", browser_url.as_str().trim_end_matches('/'));
    }
    println!();
    println!("Git");
    println!("  repo: {}/", public_url.as_str().trim_end_matches('/'));
    println!("  clone: {clone_cmd}");
    println!();
    println!("Session");
    println!("  username: {}", credentials.username);
    if show_pass {
        println!("  password: {}", credentials.password);
    } else {
        println!("  password: hidden (see file)");
    }
    println!("  file: {}", credentials_path.display());
}

fn print_branch_list(branches: &[BranchInfo], verbose: u8) {
    for branch in branches {
        let marker = if branch.is_current { "*" } else { " " };
        let served = if branch.is_served { " [served]" } else { "" };
        if verbose == 0 {
            say(&format!("{marker} {}{served}", branch.name));
            continue;
        }

        let commit = if verbose > 1 {
            branch.commit.clone()
        } else {
            branch.commit.chars().take(12).collect::<String>()
        };
        if branch.summary.is_empty() {
            say(&format!("{marker} {} {commit}{served}", branch.name));
        } else {
            say(&format!(
                "{marker} {} {commit} {}{served}",
                branch.name, branch.summary
            ));
        }
    }
}

fn default_display_name() -> String {
    std::env::var("QIT_DISPLAY_NAME")
        .ok()
        .or_else(|| std::env::var("GIT_AUTHOR_NAME").ok())
        .or_else(|| std::env::var("USER").ok())
        .or_else(|| std::env::var("USERNAME").ok())
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| "Qit CLI".to_string())
}

fn pr_state_matches(pull_request: &PullRequestRecord, state: PrStateArg) -> bool {
    match state {
        PrStateArg::All => true,
        PrStateArg::Open => pull_request.status == PullRequestStatus::Open,
        PrStateArg::Closed => pull_request.status == PullRequestStatus::Closed,
        PrStateArg::Merged => pull_request.status == PullRequestStatus::Merged,
    }
}

fn sort_pull_requests(pull_requests: &mut [PullRequestRecord]) {
    pull_requests.sort_by(|left, right| {
        right
            .updated_at_ms
            .cmp(&left.updated_at_ms)
            .then_with(|| left.title.cmp(&right.title))
    });
}

fn select_pull_request<'a>(
    pull_requests: &'a [PullRequestRecord],
    selector: &str,
) -> Result<&'a PullRequestRecord> {
    if let Some(exact) = pull_requests.iter().find(|pull_request| pull_request.id == selector) {
        return Ok(exact);
    }
    let mut matches = pull_requests
        .iter()
        .filter(|pull_request| pull_request.id.starts_with(selector));
    let first = matches
        .next()
        .ok_or_else(|| anyhow!("pull request `{selector}` was not found"))?;
    if matches.next().is_some() {
        bail!("pull request selector `{selector}` is ambiguous");
    }
    Ok(first)
}

async fn load_pull_requests(
    service: &WorkspaceService,
    path: PathBuf,
    default_branch: &str,
) -> Result<(WorkspaceSpec, Vec<PullRequestRecord>)> {
    let (workspace, web_ui) = service.load_web_ui(path, default_branch)?;
    let mut pull_requests = web_ui.pull_requests;
    sort_pull_requests(&mut pull_requests);
    Ok((workspace, pull_requests))
}

async fn load_pull_request_detail(
    service: &WorkspaceService,
    repo_read_store: &dyn RepoReadStore,
    path: PathBuf,
    default_branch: &str,
    selector: &str,
) -> Result<(WorkspaceSpec, PullRequestRecord, Option<RefComparison>, Option<Vec<RefDiffFile>>)> {
    let (workspace, pull_requests) = load_pull_requests(service, path, default_branch).await?;
    let pull_request = select_pull_request(&pull_requests, selector)?.clone();
    let (base_ref, head_ref) =
        resolve_pull_request_refs(repo_read_store, &workspace, &pull_request).await;
    let comparison = repo_read_store
        .compare_refs(&workspace, &base_ref, &head_ref, 25)
        .await
        .ok();
    let diffs = repo_read_store
        .diff_refs(&workspace, &base_ref, &head_ref)
        .await
        .ok();
    Ok((workspace, pull_request, comparison, diffs))
}

fn print_pr_summary(pull_request: &PullRequestRecord, comparison: Option<&RefComparison>) {
    say(&format!(
        "{} [{}]",
        pull_request.title, 
        match pull_request.status {
            PullRequestStatus::Open => "open",
            PullRequestStatus::Closed => "closed",
            PullRequestStatus::Merged => "merged",
        }
    ));
    say(&format!(
        "  id: {}",
        pull_request.id.chars().take(12).collect::<String>()
    ));
    say(&format!(
        "  branches: {} -> {}",
        pull_request.source_branch, pull_request.target_branch
    ));
    if let Some(comparison) = comparison {
        say(&format!(
            "  commits: {} ahead, {} behind",
            comparison.ahead_by, comparison.behind_by
        ));
    }
    if let Some(merged_commit) = &pull_request.merged_commit {
        say(&format!(
            "  merged: {}",
            merged_commit.chars().take(12).collect::<String>()
        ));
    }
    if !pull_request.description.is_empty() {
        say("");
        say(&pull_request.description);
    }
}

fn print_pr_discussion(pull_request: &PullRequestRecord) {
    let comments = WorkspaceService::pull_request_comments(pull_request);
    let reviews = WorkspaceService::pull_request_reviews(pull_request);
    let summary = WorkspaceService::pull_request_review_summary(pull_request);
    say("");
    say("Reviews");
    say(&format!(
        "  approvals: {}  changes_requested: {}  comment_only: {}",
        summary.approvals, summary.changes_requested, summary.comments
    ));
    for review in reviews.iter().rev() {
        say(&format!(
            "  - {} [{}] {}",
            review.display_name,
            match review.state {
                PullRequestReviewState::Approved => "approved",
                PullRequestReviewState::ChangesRequested => "changes_requested",
                PullRequestReviewState::Commented => "commented",
            },
            review.body
        ));
    }
    if !comments.is_empty() {
        say("");
        say("Comments");
        for comment in comments {
            say(&format!("  - {}: {}", comment.display_name, comment.body));
        }
    }
}

fn print_pr_activity(pull_request: &PullRequestRecord) {
    if pull_request.activities.is_empty() {
        return;
    }
    say("");
    say("Activity");
    for activity in pull_request.activities.iter().rev() {
        let actor = activity
            .display_name
            .clone()
            .unwrap_or_else(|| default_display_name());
        say(&format!("  - {} {}", actor, format_activity(activity)));
    }
}

fn format_activity(activity: &qit_domain::PullRequestActivityRecord) -> String {
    match activity.kind {
        qit_domain::PullRequestActivityKind::Opened => "opened the pull request".into(),
        qit_domain::PullRequestActivityKind::Commented => {
            format!("commented: {}", activity.body.clone().unwrap_or_default())
        }
        qit_domain::PullRequestActivityKind::Reviewed => format!(
            "reviewed ({}){}",
            match activity
                .review_state
                .clone()
                .unwrap_or(PullRequestReviewState::Commented)
            {
                PullRequestReviewState::Approved => "approved",
                PullRequestReviewState::ChangesRequested => "changes_requested",
                PullRequestReviewState::Commented => "commented",
            },
            activity
                .body
                .as_ref()
                .map(|body| format!(": {body}"))
                .unwrap_or_default()
        ),
        qit_domain::PullRequestActivityKind::Edited => "edited the pull request".into(),
        qit_domain::PullRequestActivityKind::Closed => "closed the pull request".into(),
        qit_domain::PullRequestActivityKind::Reopened => "reopened the pull request".into(),
        qit_domain::PullRequestActivityKind::Merged => "merged the pull request".into(),
    }
}

fn print_pr_diff(diffs: &[RefDiffFile]) {
    for file in diffs {
        say("");
        say(&format!(
            "{} [{}] +{} -{}",
            file.path, file.status, file.additions, file.deletions
        ));
        if file.original.as_ref().is_some_and(|blob| blob.is_binary)
            || file.modified.as_ref().is_some_and(|blob| blob.is_binary)
        {
            say("  binary diff not shown");
            continue;
        }
        let patch = TextDiff::from_lines(
            file.original
                .as_ref()
                .and_then(|blob| blob.text.as_deref())
                .unwrap_or(""),
            file.modified
                .as_ref()
                .and_then(|blob| blob.text.as_deref())
                .unwrap_or(""),
        )
        .unified_diff()
        .context_radius(3)
        .header(
            &format!("a/{}", file.previous_path.as_deref().unwrap_or(&file.path)),
            &format!("b/{}", file.path),
        )
        .to_string();
        for line in patch.lines() {
            say(line);
        }
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    if std::env::var_os("RUST_LOG").is_none() {
        std::env::set_var("RUST_LOG", "warn");
    }
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    let cli = Cli::parse();
    let default_branch = cli.branch.as_deref().unwrap_or(DEFAULT_BRANCH);
    let repo_store = Arc::new(GitRepoStore);
    let registry_store = Arc::new(FilesystemRegistry::new().map_err(anyhow::Error::msg)?);
    let credential_issuer = Arc::new(RandomCredentialIssuer);
    let service = Arc::new(WorkspaceService::new(
        repo_store.clone(),
        registry_store.clone(),
        credential_issuer,
    ));

    if let Some(command) = cli.command {
        match command {
            Commands::Apply { path, branch } => {
                let (workspace, outcome) = service.apply(path, default_branch, branch).await?;
                say(&format!(
                    "applied branch `{}` at `{}` to {} (checked out branch is now `{}`)",
                    outcome.merged_to,
                    outcome.commit,
                    workspace.worktree.display(),
                    workspace.checked_out_branch
                ));
                return Ok(());
            }
            Commands::Branch {
                path,
                name,
                start_point,
                list,
                verbose,
                rename,
                rename_force,
                copy,
                copy_force,
                delete,
                delete_force,
            } => {
                if let Some(names) = copy {
                    let old_name = &names[0];
                    let new_name = &names[1];
                    let (workspace, outcome) = service
                        .create_branch(path, default_branch, new_name, Some(old_name), false)
                        .await?;
                    say(&format!(
                        "copied branch `{old_name}` to `{}` at `{}` for {}",
                        outcome.branch,
                        outcome.commit,
                        workspace.worktree.display()
                    ));
                    return Ok(());
                }

                if let Some(names) = copy_force {
                    let old_name = &names[0];
                    let new_name = &names[1];
                    let (workspace, outcome) = service
                        .create_branch(path, default_branch, new_name, Some(old_name), true)
                        .await?;
                    say(&format!(
                        "force copied branch `{old_name}` to `{}` at `{}` for {}",
                        outcome.branch,
                        outcome.commit,
                        workspace.worktree.display()
                    ));
                    return Ok(());
                }

                if let Some(names) = rename {
                    let old_name = &names[0];
                    let new_name = &names[1];
                    let workspace = service
                        .rename_branch(path, default_branch, old_name, new_name, false)
                        .await?;
                    say(&format!(
                        "renamed branch `{old_name}` to `{new_name}` for {}",
                        workspace.worktree.display()
                    ));
                    return Ok(());
                }

                if let Some(names) = rename_force {
                    let old_name = &names[0];
                    let new_name = &names[1];
                    let workspace = service
                        .rename_branch(path, default_branch, old_name, new_name, true)
                        .await?;
                    say(&format!(
                        "force renamed branch `{old_name}` to `{new_name}` for {}",
                        workspace.worktree.display()
                    ));
                    return Ok(());
                }

                if let Some(branch_name) = delete {
                    let workspace = service
                        .delete_branch(path, default_branch, &branch_name, false)
                        .await?;
                    say(&format!(
                        "deleted branch `{branch_name}` for {}",
                        workspace.worktree.display()
                    ));
                    return Ok(());
                }

                if let Some(branch_name) = delete_force {
                    let workspace = service
                        .delete_branch(path, default_branch, &branch_name, true)
                        .await?;
                    say(&format!(
                        "force deleted branch `{branch_name}` for {}",
                        workspace.worktree.display()
                    ));
                    return Ok(());
                }

                if let Some(branch_name) = name {
                    let (workspace, outcome) = service
                        .create_branch(
                            path,
                            default_branch,
                            &branch_name,
                            start_point.as_deref(),
                            false,
                        )
                        .await?;
                    say(&format!(
                        "created branch `{}` at `{}` for {}",
                        outcome.branch,
                        outcome.commit,
                        workspace.worktree.display()
                    ));
                    return Ok(());
                }

                let (workspace, branches) =
                    service.list_branches(path, default_branch, &list).await?;
                say(&format!("branches for {}:", workspace.worktree.display()));
                print_branch_list(&branches, verbose);
                return Ok(());
            }
            Commands::Switch { path, branch } => {
                let (workspace, outcome) =
                    service.switch_branch(path, default_branch, &branch).await?;
                say(&format!(
                    "switched {} from `{}` to `{}` at `{}`",
                    workspace.worktree.display(),
                    outcome.previous_branch,
                    outcome.current_branch,
                    outcome.commit
                ));
                return Ok(());
            }
            Commands::Checkout {
                path,
                create,
                create_force,
                force,
                track: _,
                no_track: _,
                merge,
                detach,
                target,
                paths,
            } => {
                if merge {
                    bail!("merge-style checkout is not supported for served workspaces");
                }
                if detach {
                    bail!("detached checkout is not supported for served workspaces");
                }
                if !paths.is_empty() {
                    bail!("path checkout is not supported for served workspaces");
                }

                let (workspace, outcome) = if let Some(branch_name) = create {
                    service
                        .create_and_checkout_branch(
                            path,
                            default_branch,
                            &branch_name,
                            target.as_deref(),
                            false,
                            force,
                        )
                        .await?
                } else if let Some(branch_name) = create_force {
                    service
                        .create_and_checkout_branch(
                            path,
                            default_branch,
                            &branch_name,
                            target.as_deref(),
                            true,
                            force,
                        )
                        .await?
                } else {
                    let branch = target.context("branch is required")?;
                    service
                        .checkout_branch_with_force(path, default_branch, &branch, force)
                        .await?
                };
                say(&format!(
                    "checked out {} from `{}` to `{}` at `{}` while serving `{}`",
                    workspace.worktree.display(),
                    outcome.previous_branch,
                    outcome.current_branch,
                    outcome.commit,
                    workspace.exported_branch
                ));
                return Ok(());
            }
            Commands::Pr { command } => {
                match command {
                    PrCommands::List { path, state } => {
                        let (workspace, pull_requests) =
                            load_pull_requests(service.as_ref(), path, default_branch).await?;
                        say(&format!("pull requests for {}:", workspace.worktree.display()));
                        let mut count = 0usize;
                        for pull_request in pull_requests
                            .iter()
                            .filter(|pull_request| pr_state_matches(pull_request, state))
                        {
                            count += 1;
                            say(&format!(
                                "  {} [{}] {} -> {} ({})",
                                pull_request.id.chars().take(8).collect::<String>(),
                                match pull_request.status {
                                    PullRequestStatus::Open => "open",
                                    PullRequestStatus::Closed => "closed",
                                    PullRequestStatus::Merged => "merged",
                                },
                                pull_request.source_branch,
                                pull_request.target_branch,
                                pull_request.title
                            ));
                        }
                        if count == 0 {
                            say("  no matching pull requests");
                        }
                        return Ok(());
                    }
                    PrCommands::View { path, pull_request } => {
                        let (_workspace, pull_request, comparison, diffs) = load_pull_request_detail(
                            service.as_ref(),
                            repo_store.as_ref(),
                            path,
                            default_branch,
                            &pull_request,
                        )
                        .await?;
                        print_pr_summary(&pull_request, comparison.as_ref());
                        print_pr_discussion(&pull_request);
                        print_pr_activity(&pull_request);
                        if let Some(diffs) = diffs {
                            say("");
                            say("Files");
                            for file in diffs {
                                say(&format!(
                                    "  - {} [{}] +{} -{}",
                                    file.path, file.status, file.additions, file.deletions
                                ));
                            }
                        }
                        return Ok(());
                    }
                    PrCommands::Diff { path, pull_request } => {
                        let (_workspace, pull_request, _comparison, diffs) = load_pull_request_detail(
                            service.as_ref(),
                            repo_store.as_ref(),
                            path,
                            default_branch,
                            &pull_request,
                        )
                        .await?;
                        say(&format!("diff for {}", pull_request.title));
                        if let Some(diffs) = diffs {
                            print_pr_diff(&diffs);
                        } else {
                            say("no diff available");
                        }
                        return Ok(());
                    }
                    PrCommands::Status { path } => {
                        let (workspace, pull_requests) =
                            load_pull_requests(service.as_ref(), path, default_branch).await?;
                        let current_branch_prs = pull_requests
                            .iter()
                            .filter(|pull_request| {
                                pull_request.status == PullRequestStatus::Open
                                    && pull_request.source_branch == workspace.checked_out_branch
                            })
                            .collect::<Vec<_>>();
                        let target_branch_prs = pull_requests
                            .iter()
                            .filter(|pull_request| {
                                pull_request.status == PullRequestStatus::Open
                                    && pull_request.target_branch == workspace.exported_branch
                            })
                            .collect::<Vec<_>>();
                        say(&format!("current branch: {}", workspace.checked_out_branch));
                        if current_branch_prs.is_empty() {
                            say("  no open pull request for the current branch");
                        } else {
                            for pull_request in current_branch_prs {
                                say(&format!(
                                    "  {} -> {} {}",
                                    pull_request.source_branch, pull_request.target_branch, pull_request.title
                                ));
                            }
                        }
                        say(&format!("served branch: {}", workspace.exported_branch));
                        if target_branch_prs.is_empty() {
                            say("  no open pull requests targeting the served branch");
                        } else {
                            for pull_request in target_branch_prs {
                                say(&format!(
                                    "  {} -> {} {}",
                                    pull_request.source_branch, pull_request.target_branch, pull_request.title
                                ));
                            }
                        }
                        return Ok(());
                    }
                    PrCommands::Create {
                        path,
                        title,
                        body,
                        head,
                        base,
                    } => {
                        let (workspace, _) =
                            load_pull_requests(service.as_ref(), path.clone(), default_branch).await?;
                        let pull_request = service
                            .create_pull_request(
                                path,
                                default_branch,
                                CreatePullRequest {
                                    title,
                                    description: body,
                                    source_branch: head.unwrap_or_else(|| workspace.checked_out_branch.clone()),
                                    target_branch: base.unwrap_or_else(|| workspace.exported_branch.clone()),
                                },
                                UiRole::Owner,
                            )
                            .await?
                            .1;
                        say(&format!(
                            "created pull request {} {} -> {} ({})",
                            pull_request.id.chars().take(8).collect::<String>(),
                            pull_request.source_branch,
                            pull_request.target_branch,
                            pull_request.title
                        ));
                        return Ok(());
                    }
                    PrCommands::Edit {
                        path,
                        pull_request,
                        title,
                        body,
                        open,
                        close,
                    } => {
                        let (_, pull_requests) =
                            load_pull_requests(service.as_ref(), path.clone(), default_branch).await?;
                        let pull_request = select_pull_request(&pull_requests, &pull_request)?;
                        let updated = service
                            .update_pull_request(
                                path,
                                default_branch,
                                &pull_request.id,
                                UpdatePullRequest {
                                    title,
                                    description: body,
                                    status: if open {
                                        Some(PullRequestStatus::Open)
                                    } else if close {
                                        Some(PullRequestStatus::Closed)
                                    } else {
                                        None
                                    },
                                },
                                UiRole::Owner,
                            )
                            .await?
                            .1;
                        say(&format!(
                            "updated pull request {} [{}]",
                            updated.id.chars().take(8).collect::<String>(),
                            match updated.status {
                                PullRequestStatus::Open => "open",
                                PullRequestStatus::Closed => "closed",
                                PullRequestStatus::Merged => "merged",
                            }
                        ));
                        return Ok(());
                    }
                    PrCommands::Comment {
                        path,
                        pull_request,
                        body,
                        author,
                    } => {
                        let (_, pull_requests) =
                            load_pull_requests(service.as_ref(), path.clone(), default_branch).await?;
                        let pull_request = select_pull_request(&pull_requests, &pull_request)?;
                        service
                            .comment_pull_request(
                                path,
                                default_branch,
                                &pull_request.id,
                                CreatePullRequestComment {
                                    display_name: author.unwrap_or_else(default_display_name),
                                    body,
                                },
                                UiRole::Owner,
                            )
                            .await?;
                        say("comment added");
                        return Ok(());
                    }
                    PrCommands::Review {
                        path,
                        pull_request,
                        approve,
                        request_changes,
                        comment: _comment,
                        body,
                        author,
                    } => {
                        let (_, pull_requests) =
                            load_pull_requests(service.as_ref(), path.clone(), default_branch).await?;
                        let pull_request = select_pull_request(&pull_requests, &pull_request)?;
                        let state = if approve {
                            PullRequestReviewState::Approved
                        } else if request_changes {
                            PullRequestReviewState::ChangesRequested
                        } else {
                            PullRequestReviewState::Commented
                        };
                        service
                            .review_pull_request(
                                path,
                                default_branch,
                                &pull_request.id,
                                CreatePullRequestReview {
                                    display_name: author.unwrap_or_else(default_display_name),
                                    body,
                                    state: state.clone(),
                                },
                                UiRole::Owner,
                            )
                            .await?;
                        say(&format!(
                            "submitted {} review",
                            match state {
                                PullRequestReviewState::Approved => "approved",
                                PullRequestReviewState::ChangesRequested => "changes_requested",
                                PullRequestReviewState::Commented => "comment",
                            }
                        ));
                        return Ok(());
                    }
                    PrCommands::Merge { path, pull_request } => {
                        let (_, pull_requests) =
                            load_pull_requests(service.as_ref(), path.clone(), default_branch).await?;
                        let pull_request = select_pull_request(&pull_requests, &pull_request)?;
                        let merged = service
                            .merge_pull_request(path, default_branch, &pull_request.id)
                            .await?
                            .1;
                        say(&format!(
                            "merged pull request {} at {}",
                            merged.id.chars().take(8).collect::<String>(),
                            merged
                                .merged_commit
                                .unwrap_or_else(|| "unknown".into())
                                .chars()
                                .take(12)
                                .collect::<String>()
                        ));
                        return Ok(());
                    }
                    PrCommands::Close { path, pull_request } => {
                        let (_, pull_requests) =
                            load_pull_requests(service.as_ref(), path.clone(), default_branch).await?;
                        let pull_request = select_pull_request(&pull_requests, &pull_request)?;
                        service
                            .update_pull_request(
                                path,
                                default_branch,
                                &pull_request.id,
                                UpdatePullRequest {
                                    title: None,
                                    description: None,
                                    status: Some(PullRequestStatus::Closed),
                                },
                                UiRole::Owner,
                            )
                            .await?;
                        say("closed pull request");
                        return Ok(());
                    }
                    PrCommands::Reopen { path, pull_request } => {
                        let (_, pull_requests) =
                            load_pull_requests(service.as_ref(), path.clone(), default_branch).await?;
                        let pull_request = select_pull_request(&pull_requests, &pull_request)?;
                        service
                            .update_pull_request(
                                path,
                                default_branch,
                                &pull_request.id,
                                UpdatePullRequest {
                                    title: None,
                                    description: None,
                                    status: Some(PullRequestStatus::Open),
                                },
                                UiRole::Owner,
                            )
                            .await?;
                        say("reopened pull request");
                        return Ok(());
                    }
                    PrCommands::Delete { path, pull_request } => {
                        let (_, pull_requests) =
                            load_pull_requests(service.as_ref(), path.clone(), default_branch).await?;
                        let pull_request = select_pull_request(&pull_requests, &pull_request)?;
                        service
                            .delete_pull_request(path, default_branch, &pull_request.id)
                            .await?;
                        say("deleted pull request");
                        return Ok(());
                    }
                }
            }
        }
    }

    let path = cli.path.context("path is required")?;
    let prepared = service
        .prepare_serve(
            path,
            cli.branch.as_deref(),
            "qit snapshot",
            cli.allow_existing_git,
        )
        .await?;
    let workspace = prepared.workspace.clone();
    let credentials = prepared.credentials.clone();
    let credentials_path = write_credentials_file(&credentials)?;
    let repo_mount_path = repo_mount_path(&repo_name_from_worktree(&workspace.worktree));

    let addr = SocketAddr::from_str(&format!("127.0.0.1:{}", cli.port)).expect("valid socket addr");
    let listener = TcpListener::bind(addr)
        .await
        .with_context(|| format!("bind 127.0.0.1:{}", cli.port))?;
    let local_url = Url::parse(&format!("http://127.0.0.1:{}/", cli.port)).context("local URL")?;
    let transport = if cli.local_only {
        PublicTransport::Local
    } else {
        cli.transport
            .map(Into::into)
            .unwrap_or(PublicTransport::Ngrok)
    };
    let request_scheme = if transport == PublicTransport::Local {
        "http".to_string()
    } else {
        "https".to_string()
    };
    let show_pass = cli.show_pass;

    let git_router = GitHttpServer::new(
        Arc::new(GitHttpBackendAdapter),
        registry_store,
        service.clone(),
        GitHttpServerConfig {
            workspace: workspace.clone(),
            credentials: credentials.clone(),
            auto_apply: cli.auto_apply,
            repo_mount_path: repo_mount_path.clone(),
            request_scheme,
            max_body_bytes: cli.max_body_bytes,
        },
    )
    .git_router();
    let local_browser_url = repo_url(&local_url, &repo_mount_path)?;
    let endpoint = expose(transport, &local_url).await?;
    let public_repo_url = repo_url(&endpoint.public_url, &repo_mount_path)?;
    let clone_url = repo_url_with_credentials(&public_repo_url, &credentials, show_pass)?;
    let clone_cmd = clone_command(&clone_url, transport);
    let web_router = WebUiServer::new(
        repo_store.clone(),
        service.clone(),
        WebUiConfig {
            workspace: workspace.clone(),
            repo_mount_path: repo_mount_path.clone(),
            credentials: credentials.clone(),
            implicit_owner_mode: true,
            secure_cookies: transport != PublicTransport::Local,
            public_repo_url: Some(public_repo_url.as_str().trim_end_matches('/').to_string()),
        },
    )
    .router();
    let app = web_router.merge(git_router);
    let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel::<()>();
    let serve = tokio::spawn(async move {
        axum::serve(
            listener,
            app.into_make_service_with_connect_info::<SocketAddr>(),
        )
        .with_graceful_shutdown(async move {
            let _ = shutdown_rx.await;
        })
        .await
        .map_err(|err| anyhow::anyhow!(err))
    });
    print_serve_summary(
        &workspace.worktree,
        &workspace.exported_branch,
        endpoint.label,
        &public_repo_url,
        &local_browser_url,
        &public_repo_url,
        &credentials,
        &credentials_path,
        show_pass,
        &clone_cmd,
        cli.auto_apply,
    );
    println!("Ctrl+C to stop.");
    println!();
    let _ = std::io::stdout().flush();

    tokio::signal::ctrl_c().await.context("ctrl_c")?;
    let _ = shutdown_tx.send(());
    match timeout(Duration::from_secs(5), serve).await {
        Ok(Ok(Ok(()))) => {}
        Ok(Ok(Err(error))) => return Err(error),
        Ok(Err(error)) => bail!("git http server task failed: {error}"),
        Err(_) => bail!("timed out waiting for git http server shutdown"),
    }
    endpoint
        .shutdown()
        .await
        .context("shutdown public endpoint")?;
    Ok(())
}
