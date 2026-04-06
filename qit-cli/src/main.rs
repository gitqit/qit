mod serve_output;
mod supervisor;

use anyhow::{anyhow, bail, Context, Result};
use clap::{ArgAction, Parser, Subcommand, ValueEnum};
use qit_domain::{
    resolve_pull_request_refs, AccessRequestStatus, AuthActor, AuthMethod, AuthMode, BranchInfo,
    CreateIssue, CreateIssueComment, CreatePullRequest, CreatePullRequestComment,
    CreatePullRequestReview, CredentialIssuer, IssueLabel, IssueLinkRelation, IssueLinkSource,
    IssueMilestone, IssueRecord, IssueStatus, PullRequestRecord, PullRequestReviewState,
    PullRequestStatus, RefComparison, RefDiffFile, RepoAuthState, RepoReadStore, RepoUserRole,
    RepoUserStatus, RepositorySettings, SessionCredentials, UiRole, UpdateIssue, UpdatePullRequest,
    UpdateRepositorySettings, UpsertBranchRule, UpsertIssueLabel, UpsertIssueMilestone,
    WorkspaceService, WorkspaceSpec, DEFAULT_BRANCH,
};
use qit_git::{GitHttpBackendAdapter, GitRepoStore};
use qit_http::{GitHttpServer, GitHttpServerConfig, DEFAULT_MAX_BODY_BYTES};
use qit_storage::FilesystemRegistry;
use qit_transports::PublicTransport;
use qit_webui::{WebUiConfig, WebUiServer};
use rand::distributions::{Alphanumeric, DistString};
use serve_output::{
    clone_command, print_serve_summary, repo_name_from_worktree, repo_url,
    repo_url_with_credentials, say, write_credentials_file,
};
use similar::TextDiff;
use std::io::Write;
use std::path::PathBuf;
use std::sync::Arc;
use supervisor::{
    claim_mount_path, ensure_supervisor, heartbeat_interval, heartbeat_route, register_route,
    run_internal_supervisor, unregister_route,
};
use tokio::net::TcpListener;
use tokio::time::{timeout, Duration};
use tracing_subscriber::EnvFilter;
use url::Url;

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
enum TransportArg {
    Ngrok,
    Tailscale,
    Lan,
    Local,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
enum ServeAuthModeArg {
    SharedSession,
    RequestBased,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
enum ServeAuthMethodArg {
    RequestAccess,
    SetupToken,
    BasicAuth,
}

impl From<ServeAuthModeArg> for AuthMode {
    fn from(value: ServeAuthModeArg) -> Self {
        match value {
            ServeAuthModeArg::SharedSession => AuthMode::SharedSession,
            ServeAuthModeArg::RequestBased => AuthMode::RequestBased,
        }
    }
}

impl From<ServeAuthMethodArg> for AuthMethod {
    fn from(value: ServeAuthMethodArg) -> Self {
        match value {
            ServeAuthMethodArg::RequestAccess => AuthMethod::RequestAccess,
            ServeAuthMethodArg::SetupToken => AuthMethod::SetupToken,
            ServeAuthMethodArg::BasicAuth => AuthMethod::BasicAuth,
        }
    }
}

impl From<TransportArg> for PublicTransport {
    fn from(value: TransportArg) -> Self {
        match value {
            TransportArg::Ngrok => PublicTransport::Ngrok,
            TransportArg::Tailscale => PublicTransport::Tailscale,
            TransportArg::Lan => PublicTransport::Lan,
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

    /// Hide the password from stdout and keep it in a local credentials file instead.
    #[arg(long, conflicts_with = "show_pass")]
    hidden_pass: bool,

    /// Backward-compatible alias; passwords are shown by default.
    #[arg(long, hide = true)]
    show_pass: bool,

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

    /// Initial auth mode for this served repo.
    #[arg(long, value_enum)]
    auth_mode: Option<ServeAuthModeArg>,

    /// Enable one or more auth methods for this served repo.
    #[arg(long = "auth-method", value_enum, action = ArgAction::Append)]
    auth_methods: Vec<ServeAuthMethodArg>,
}

#[derive(Subcommand)]
enum Commands {
    #[command(hide = true)]
    InternalSupervisor {
        #[arg(long)]
        port: u16,

        #[arg(long, value_enum)]
        transport: TransportArg,
    },
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
    /// Manage issues using GitHub CLI-style subcommands.
    Issue {
        #[command(subcommand)]
        command: IssueCommands,
    },
    /// Manage pull requests using GitHub CLI-style subcommands.
    Pr {
        #[command(subcommand)]
        command: PrCommands,
    },
    /// View and edit repository settings.
    Settings {
        #[command(subcommand)]
        command: SettingsCommands,
    },
    /// Manage request-based auth, users, and PATs for a served folder.
    Auth {
        #[command(subcommand)]
        command: AuthCommands,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
enum PrStateArg {
    Open,
    Closed,
    Merged,
    All,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, ValueEnum)]
enum IssueStateArg {
    Open,
    Closed,
    All,
}

#[derive(Subcommand)]
enum IssueCommands {
    /// List issues for a served folder.
    List {
        /// Folder previously published with qit.
        path: PathBuf,

        /// Filter by issue state.
        #[arg(long, default_value = "open")]
        state: IssueStateArg,
    },
    /// Show an issue summary, metadata, comments, and activity.
    View {
        /// Folder previously published with qit.
        path: PathBuf,

        /// Issue number, id, or unique id prefix.
        issue: String,
    },
    /// Create an issue with metadata and optional links.
    Create {
        /// Folder previously published with qit.
        path: PathBuf,

        /// Issue title.
        #[arg(long)]
        title: String,

        /// Issue body/description. Supports markdown and references like `Related to #12`.
        #[arg(long, default_value = "")]
        body: String,

        /// Add one or more labels by name.
        #[arg(long = "label", action = ArgAction::Append)]
        labels: Vec<String>,

        /// Assign one or more active users by username.
        #[arg(long = "assignee", action = ArgAction::Append)]
        assignees: Vec<String>,

        /// Set the milestone by title, creating it if needed.
        #[arg(long)]
        milestone: Option<String>,

        /// Link one or more pull requests by id or unique id prefix.
        #[arg(long = "link-pr", action = ArgAction::Append)]
        link_prs: Vec<String>,
    },
    /// Edit issue details.
    Edit {
        /// Folder previously published with qit.
        path: PathBuf,

        /// Issue number, id, or unique id prefix.
        issue: String,

        /// Replace the title.
        #[arg(long)]
        title: Option<String>,

        /// Replace the body/description.
        #[arg(long)]
        body: Option<String>,

        /// Mark the issue open.
        #[arg(long, conflicts_with = "close")]
        open: bool,

        /// Mark the issue closed.
        #[arg(long, conflicts_with = "open")]
        close: bool,
    },
    /// Add a top-level markdown comment to an issue.
    Comment {
        /// Folder previously published with qit.
        path: PathBuf,

        /// Issue number, id, or unique id prefix.
        issue: String,

        /// Comment body.
        #[arg(long)]
        body: String,

        /// Display name to record with the comment when no durable identity exists.
        #[arg(long)]
        author: Option<String>,
    },
    /// Close an issue.
    Close {
        /// Folder previously published with qit.
        path: PathBuf,

        /// Issue number, id, or unique id prefix.
        issue: String,
    },
    /// Reopen an issue.
    Reopen {
        /// Folder previously published with qit.
        path: PathBuf,

        /// Issue number, id, or unique id prefix.
        issue: String,
    },
    /// Delete an issue.
    Delete {
        /// Folder previously published with qit.
        path: PathBuf,

        /// Issue number, id, or unique id prefix.
        issue: String,
    },
    /// Add or remove labels on an issue.
    Label {
        /// Folder previously published with qit.
        path: PathBuf,

        /// Issue number, id, or unique id prefix.
        issue: String,

        /// Add one or more labels by name.
        #[arg(long = "add", action = ArgAction::Append)]
        add: Vec<String>,

        /// Remove one or more labels by name.
        #[arg(long = "remove", action = ArgAction::Append)]
        remove: Vec<String>,
    },
    /// Assign or unassign users on an issue.
    Assign {
        /// Folder previously published with qit.
        path: PathBuf,

        /// Issue number, id, or unique id prefix.
        issue: String,

        /// Add one or more assignees by username.
        #[arg(long = "add", action = ArgAction::Append)]
        add: Vec<String>,

        /// Remove one or more assignees by username.
        #[arg(long = "remove", action = ArgAction::Append)]
        remove: Vec<String>,
    },
    /// Set or clear the issue milestone.
    Milestone {
        /// Folder previously published with qit.
        path: PathBuf,

        /// Issue number, id, or unique id prefix.
        issue: String,

        /// Set the milestone by title, creating it if needed.
        #[arg(long, conflicts_with = "clear")]
        set: Option<String>,

        /// Clear the milestone.
        #[arg(long, conflicts_with = "set")]
        clear: bool,
    },
    /// Link a pull request to an issue.
    LinkPr {
        /// Folder previously published with qit.
        path: PathBuf,

        /// Issue number, id, or unique id prefix.
        issue: String,

        /// Pull request id or unique id prefix.
        pull_request: String,
    },
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

        /// Pull request body/description. Supports markdown and references like `Fixes #12`.
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

#[derive(Subcommand)]
enum SettingsCommands {
    /// Show repository metadata, default branch, and branch rules.
    View {
        /// Folder previously published with qit.
        path: PathBuf,
    },
    /// Update repository metadata or the default branch.
    Set {
        /// Folder previously published with qit.
        path: PathBuf,

        /// Replace the repository description.
        #[arg(long)]
        description: Option<String>,

        /// Replace the homepage URL.
        #[arg(long)]
        homepage: Option<String>,

        /// Change the served/default branch.
        #[arg(long)]
        default_branch: Option<String>,
    },
    /// List, add, or remove branch rules.
    Rule {
        /// Folder previously published with qit.
        path: PathBuf,

        /// Branch pattern for the rule to add or update.
        #[arg(long)]
        pattern: Option<String>,

        /// Delete a branch rule by pattern.
        #[arg(long, conflicts_with = "pattern")]
        delete: Option<String>,

        /// Require pull requests before merge.
        #[arg(long)]
        require_pr: bool,

        /// Minimum approvals required before merge.
        #[arg(long)]
        approvals: Option<u8>,

        /// Ignore approvals after new commits land on the source branch.
        #[arg(long)]
        dismiss_stale: bool,

        /// Reject non-fast-forward pushes.
        #[arg(long)]
        block_force_push: bool,

        /// Reject branch deletion.
        #[arg(long)]
        block_delete: bool,
    },
}

#[derive(Subcommand)]
enum AuthCommands {
    /// View or change the repository auth mode.
    Mode {
        /// Folder previously published with qit.
        path: PathBuf,

        /// Enable request-based auth.
        #[arg(long, conflicts_with = "shared_session")]
        request_based: bool,

        /// Keep the legacy shared-session auth mode.
        #[arg(long, conflicts_with = "request_based")]
        shared_session: bool,
    },
    /// List, approve, or reject access requests.
    Requests {
        /// Folder previously published with qit.
        path: PathBuf,

        /// Approve a request by id.
        #[arg(long, conflicts_with = "reject")]
        approve: Option<String>,

        /// Reject a request by id.
        #[arg(long, conflicts_with = "approve")]
        reject: Option<String>,
    },
    /// List or manage repo users.
    Users {
        /// Folder previously published with qit.
        path: PathBuf,

        /// Promote a user to owner.
        #[arg(long, conflicts_with_all = ["demote", "revoke", "reset_setup"])]
        promote: Option<String>,

        /// Demote an owner back to user.
        #[arg(long, conflicts_with_all = ["promote", "revoke", "reset_setup"])]
        demote: Option<String>,

        /// Revoke a user account.
        #[arg(long, conflicts_with_all = ["promote", "demote", "reset_setup"])]
        revoke: Option<String>,

        /// Reset setup and issue a new one-time onboarding token.
        #[arg(long = "reset-setup", conflicts_with_all = ["promote", "demote", "revoke"])]
        reset_setup: Option<String>,
    },
    /// List or revoke personal access tokens.
    Pats {
        /// Folder previously published with qit.
        path: PathBuf,

        /// Revoke a PAT by id.
        #[arg(long)]
        revoke: Option<String>,
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

fn format_branch_rule_summary(
    pattern: &str,
    require_pull_request: bool,
    required_approvals: u8,
    dismiss_stale_approvals: bool,
    block_force_push: bool,
    block_delete: bool,
) -> String {
    let mut parts = Vec::new();
    if require_pull_request || required_approvals > 0 {
        parts.push("require PR".to_string());
    }
    if required_approvals > 0 {
        parts.push(format!("{required_approvals} approval(s)"));
    }
    if dismiss_stale_approvals {
        parts.push("dismiss stale approvals".to_string());
    }
    if block_force_push {
        parts.push("block force-push".to_string());
    }
    if block_delete {
        parts.push("block delete".to_string());
    }
    let summary = if parts.is_empty() {
        "no protections".to_string()
    } else {
        parts.join(", ")
    };
    format!("{pattern}: {summary}")
}

fn print_repository_settings(workspace: &WorkspaceSpec, settings: &RepositorySettings) {
    say(&format!("settings for {}:", workspace.worktree.display()));
    say(&format!("  default branch: {}", workspace.exported_branch));
    say(&format!(
        "  description: {}",
        if settings.description.is_empty() {
            "not set"
        } else {
            &settings.description
        }
    ));
    say(&format!(
        "  homepage: {}",
        if settings.homepage_url.is_empty() {
            "not set"
        } else {
            &settings.homepage_url
        }
    ));
    say("  branch rules:");
    if settings.branch_rules.is_empty() {
        say("    none");
        return;
    }
    for rule in &settings.branch_rules {
        say(&format!(
            "    - {}",
            format_branch_rule_summary(
                &rule.pattern,
                rule.require_pull_request,
                rule.required_approvals,
                rule.dismiss_stale_approvals,
                rule.block_force_push,
                rule.block_delete,
            )
        ));
    }
}

fn format_auth_mode(mode: AuthMode) -> &'static str {
    match mode {
        AuthMode::SharedSession => "shared_session",
        AuthMode::RequestBased => "request_based",
    }
}

fn print_auth_state(workspace: &WorkspaceSpec, auth: &qit_domain::RepoAuthState) {
    say(&format!("auth for {}:", workspace.worktree.display()));
    say(&format!("  mode: {}", format_auth_mode(auth.mode.clone())));
    say("  pending requests:");
    let pending_requests = auth
        .access_requests
        .iter()
        .filter(|request| request.status == AccessRequestStatus::Pending)
        .collect::<Vec<_>>();
    if pending_requests.is_empty() {
        say("    none");
    } else {
        for request in pending_requests {
            say(&format!(
                "    - {} {} <{}>",
                request.id.chars().take(8).collect::<String>(),
                request.name,
                request.email
            ));
        }
    }
    say("  users:");
    if auth.users.is_empty() {
        say("    none");
    } else {
        for user in &auth.users {
            let username = user.username.as_deref().unwrap_or("not set");
            let role = match user.role {
                RepoUserRole::Owner => "owner",
                RepoUserRole::User => "user",
            };
            let status = match user.status {
                RepoUserStatus::PendingRequest => "pending_request",
                RepoUserStatus::ApprovedPendingSetup => "approved_pending_setup",
                RepoUserStatus::Active => "active",
                RepoUserStatus::Revoked => "revoked",
            };
            say(&format!(
                "    - {} {} <{}> user={} role={} status={}",
                user.id.chars().take(8).collect::<String>(),
                user.name,
                user.email,
                username,
                role,
                status
            ));
        }
    }
    say("  personal access tokens:");
    let active_pats = auth
        .personal_access_tokens
        .iter()
        .filter(|token| token.revoked_at_ms.is_none())
        .collect::<Vec<_>>();
    if active_pats.is_empty() {
        say("    none");
    } else {
        for token in active_pats {
            let owner = auth
                .users
                .iter()
                .find(|user| user.id == token.user_id)
                .and_then(|user| user.username.as_deref())
                .unwrap_or("unknown");
            say(&format!(
                "    - {} {} ({owner})",
                token.id.chars().take(8).collect::<String>(),
                token.label
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

fn cli_issue_actor(author: Option<String>) -> qit_domain::IssueActorInput {
    qit_domain::IssueActorInput {
        role: UiRole::Owner,
        display_name: Some(author.unwrap_or_else(default_display_name)),
        user_id: None,
        username: None,
    }
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

fn issue_state_matches(issue: &IssueRecord, state: IssueStateArg) -> bool {
    match state {
        IssueStateArg::All => true,
        IssueStateArg::Open => issue.status == IssueStatus::Open,
        IssueStateArg::Closed => issue.status == IssueStatus::Closed,
    }
}

fn sort_issues(issues: &mut [IssueRecord]) {
    issues.sort_by(|left, right| {
        right
            .updated_at_ms
            .cmp(&left.updated_at_ms)
            .then_with(|| right.number.cmp(&left.number))
    });
}

fn select_issue<'a>(issues: &'a [IssueRecord], selector: &str) -> Result<&'a IssueRecord> {
    if let Some(exact) = issues.iter().find(|issue| issue.id == selector) {
        return Ok(exact);
    }
    if let Ok(number) = selector.parse::<u64>() {
        if let Some(exact) = issues.iter().find(|issue| issue.number == number) {
            return Ok(exact);
        }
    }
    let mut matches = issues.iter().filter(|issue| issue.id.starts_with(selector));
    let first = matches
        .next()
        .ok_or_else(|| anyhow!("issue `{selector}` was not found"))?;
    if matches.next().is_some() {
        bail!("issue selector `{selector}` is ambiguous");
    }
    Ok(first)
}

fn select_pull_request<'a>(
    pull_requests: &'a [PullRequestRecord],
    selector: &str,
) -> Result<&'a PullRequestRecord> {
    if let Some(exact) = pull_requests
        .iter()
        .find(|pull_request| pull_request.id == selector)
    {
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

fn select_by_id_prefix<'a, T>(
    values: &'a [T],
    selector: &str,
    id_of: impl Fn(&'a T) -> &'a str,
    noun: &str,
) -> Result<&'a T> {
    if let Some(exact) = values.iter().find(|value| id_of(value) == selector) {
        return Ok(exact);
    }
    let mut matches = values
        .iter()
        .filter(|value| id_of(value).starts_with(selector));
    let first = matches
        .next()
        .ok_or_else(|| anyhow!("{noun} `{selector}` was not found"))?;
    if matches.next().is_some() {
        bail!("{noun} selector `{selector}` is ambiguous");
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

async fn load_issues(
    service: &WorkspaceService,
    path: PathBuf,
    default_branch: &str,
) -> Result<(WorkspaceSpec, qit_domain::WorkspaceWebUiState)> {
    let (workspace, mut web_ui) = service.load_web_ui(path, default_branch)?;
    sort_issues(&mut web_ui.issues);
    Ok((workspace, web_ui))
}

fn issue_label_names(issue: &IssueRecord, labels: &[IssueLabel]) -> Vec<String> {
    issue
        .label_ids
        .iter()
        .filter_map(|label_id| labels.iter().find(|label| label.id == *label_id))
        .map(|label| label.name.clone())
        .collect()
}

fn issue_assignee_names(issue: &IssueRecord, auth: &RepoAuthState) -> Vec<String> {
    issue
        .assignee_user_ids
        .iter()
        .filter_map(|user_id| auth.users.iter().find(|user| user.id == *user_id))
        .map(|user| user.username.clone().unwrap_or_else(|| user.name.clone()))
        .collect()
}

fn issue_milestone_title(issue: &IssueRecord, milestones: &[IssueMilestone]) -> Option<String> {
    issue
        .milestone_id
        .as_ref()
        .and_then(|milestone_id| {
            milestones
                .iter()
                .find(|milestone| milestone.id == *milestone_id)
        })
        .map(|milestone| milestone.title.clone())
}

fn link_relation_label(relation: &IssueLinkRelation) -> &'static str {
    match relation {
        IssueLinkRelation::Related => "related",
        IssueLinkRelation::Closing => "closing",
    }
}

fn link_source_label(source: &IssueLinkSource) -> &'static str {
    match source {
        IssueLinkSource::Manual => "manual",
        IssueLinkSource::IssueDescription => "issue body",
        IssueLinkSource::IssueComment => "issue comment",
        IssueLinkSource::PullRequestDescription => "PR body",
        IssueLinkSource::PullRequestComment => "PR comment",
        IssueLinkSource::PullRequestReview => "PR review",
    }
}

async fn load_pull_request_detail(
    service: &WorkspaceService,
    repo_read_store: &dyn RepoReadStore,
    path: PathBuf,
    default_branch: &str,
    selector: &str,
) -> Result<(
    WorkspaceSpec,
    qit_domain::WorkspaceWebUiState,
    PullRequestRecord,
    Option<RefComparison>,
    Option<Vec<RefDiffFile>>,
)> {
    let (workspace, web_ui) = service.load_web_ui(path, default_branch)?;
    let pull_request = select_pull_request(&web_ui.pull_requests, selector)?.clone();
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
    Ok((workspace, web_ui, pull_request, comparison, diffs))
}

fn print_pr_summary(
    pull_request: &PullRequestRecord,
    issues: &[IssueRecord],
    pull_requests: &[PullRequestRecord],
    comparison: Option<&RefComparison>,
) {
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
    let linked_issues =
        WorkspaceService::linked_issues_for_pull_request(pull_request, issues, pull_requests);
    if !linked_issues.is_empty() {
        say("  linked_issues:");
        for link in linked_issues {
            if let Some(issue) = issues.iter().find(|issue| issue.id == link.issue_id) {
                say(&format!(
                    "    - #{} [{} via {}] {}",
                    issue.number,
                    link_relation_label(&link.relation),
                    link_source_label(&link.source),
                    issue.title
                ));
            }
        }
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
            .unwrap_or_else(default_display_name);
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

fn print_issue_summary(
    issue: &IssueRecord,
    labels: &[IssueLabel],
    milestones: &[IssueMilestone],
    auth: &RepoAuthState,
    issues: &[IssueRecord],
    pull_requests: &[PullRequestRecord],
) {
    say(&format!(
        "#{} {} [{}]",
        issue.number,
        issue.title,
        match issue.status {
            IssueStatus::Open => "open",
            IssueStatus::Closed => "closed",
        }
    ));
    say(&format!(
        "  id: {}",
        issue.id.chars().take(12).collect::<String>()
    ));
    say(&format!("  author: {}", issue.author.display_name));
    let label_names = issue_label_names(issue, labels);
    if !label_names.is_empty() {
        say(&format!("  labels: {}", label_names.join(", ")));
    }
    let assignees = issue_assignee_names(issue, auth);
    if !assignees.is_empty() {
        say(&format!("  assignees: {}", assignees.join(", ")));
    }
    if let Some(milestone) = issue_milestone_title(issue, milestones) {
        say(&format!("  milestone: {milestone}"));
    }
    let linked_pull_requests =
        WorkspaceService::linked_pull_requests_for_issue(issue, issues, pull_requests);
    if !linked_pull_requests.is_empty() {
        say("  linked_prs:");
        for link in linked_pull_requests {
            if let Some(pull_request) = pull_requests
                .iter()
                .find(|pull_request| pull_request.id == link.pull_request_id)
            {
                say(&format!(
                    "    - {} [{} via {}] {}",
                    pull_request.id.chars().take(8).collect::<String>(),
                    link_relation_label(&link.relation),
                    link_source_label(&link.source),
                    pull_request.title
                ));
            }
        }
    }
    if !issue.description.is_empty() {
        say("");
        say(&issue.description);
    }
}

fn print_issue_discussion(issue: &IssueRecord) {
    if issue.comments.is_empty() {
        return;
    }
    say("");
    say("Comments");
    for comment in &issue.comments {
        say(&format!(
            "  - {}: {}",
            comment.actor.display_name, comment.body
        ));
    }
}

fn print_issue_activity(issue: &IssueRecord, labels: &[IssueLabel], milestones: &[IssueMilestone]) {
    if issue.timeline.is_empty() {
        return;
    }
    say("");
    say("Activity");
    for event in issue.timeline.iter().rev() {
        let message = match event.kind {
            qit_domain::IssueTimelineEventKind::Opened => "opened the issue".to_string(),
            qit_domain::IssueTimelineEventKind::Commented => {
                format!("commented: {}", event.body.clone().unwrap_or_default())
            }
            qit_domain::IssueTimelineEventKind::Edited => "edited the issue".to_string(),
            qit_domain::IssueTimelineEventKind::Closed => "closed the issue".to_string(),
            qit_domain::IssueTimelineEventKind::Reopened => "reopened the issue".to_string(),
            qit_domain::IssueTimelineEventKind::LabelsChanged => {
                if event.labels.is_empty() {
                    "cleared the labels".to_string()
                } else {
                    let names = event
                        .labels
                        .iter()
                        .filter_map(|label_id| labels.iter().find(|label| label.id == *label_id))
                        .map(|label| label.name.clone())
                        .collect::<Vec<_>>();
                    format!("set labels: {}", names.join(", "))
                }
            }
            qit_domain::IssueTimelineEventKind::AssigneesChanged => "updated assignees".to_string(),
            qit_domain::IssueTimelineEventKind::MilestoneChanged => {
                if let Some(milestone_id) = &event.milestone_id {
                    let title = milestones
                        .iter()
                        .find(|milestone| milestone.id == *milestone_id)
                        .map(|milestone| milestone.title.clone())
                        .unwrap_or_else(|| milestone_id.clone());
                    format!("set milestone: {title}")
                } else {
                    "cleared the milestone".to_string()
                }
            }
            qit_domain::IssueTimelineEventKind::PullRequestLinked => format!(
                "linked pull request {}",
                event
                    .pull_request_id
                    .clone()
                    .unwrap_or_default()
                    .chars()
                    .take(8)
                    .collect::<String>()
            ),
            qit_domain::IssueTimelineEventKind::PullRequestUnlinked => format!(
                "unlinked pull request {}",
                event
                    .pull_request_id
                    .clone()
                    .unwrap_or_default()
                    .chars()
                    .take(8)
                    .collect::<String>()
            ),
            qit_domain::IssueTimelineEventKind::ReactionToggled => format!(
                "toggled reaction {}",
                event
                    .reaction
                    .as_ref()
                    .map(|reaction| format!("{reaction:?}").to_ascii_lowercase())
                    .unwrap_or_else(|| "unknown".into())
            ),
        };
        say(&format!("  - {} {}", event.actor.display_name, message));
    }
}

async fn ensure_label_ids(
    service: &WorkspaceService,
    path: PathBuf,
    default_branch: &str,
    names: &[String],
) -> Result<Vec<String>> {
    let (_, web_ui) = service.load_web_ui(path.clone(), default_branch)?;
    let mut ids = Vec::new();
    for name in names
        .iter()
        .map(|name| name.trim())
        .filter(|name| !name.is_empty())
    {
        if let Some(existing) = web_ui
            .issue_settings
            .labels
            .iter()
            .find(|label| label.name.eq_ignore_ascii_case(name))
        {
            ids.push(existing.id.clone());
            continue;
        }
        let label = service
            .upsert_issue_label(
                path.clone(),
                default_branch,
                UpsertIssueLabel {
                    id: None,
                    name: name.to_string(),
                    color: String::new(),
                    description: String::new(),
                },
            )?
            .1;
        ids.push(label.id);
    }
    ids.sort();
    ids.dedup();
    Ok(ids)
}

fn assignee_ids_from_usernames(
    web_ui: &qit_domain::WorkspaceWebUiState,
    usernames: &[String],
) -> Result<Vec<String>> {
    let mut ids = Vec::new();
    for username in usernames
        .iter()
        .map(|username| username.trim())
        .filter(|username| !username.is_empty())
    {
        let user = web_ui
            .auth
            .users
            .iter()
            .find(|user| {
                user.status == RepoUserStatus::Active
                    && user
                        .username
                        .as_deref()
                        .is_some_and(|candidate| candidate == username)
            })
            .ok_or_else(|| anyhow!("user `{username}` was not found"))?;
        ids.push(user.id.clone());
    }
    ids.sort();
    ids.dedup();
    Ok(ids)
}

async fn ensure_milestone_id(
    service: &WorkspaceService,
    path: PathBuf,
    default_branch: &str,
    title: Option<String>,
) -> Result<Option<String>> {
    let Some(title) = title
        .map(|title| title.trim().to_string())
        .filter(|title| !title.is_empty())
    else {
        return Ok(None);
    };
    let (_, web_ui) = service.load_web_ui(path.clone(), default_branch)?;
    if let Some(existing) = web_ui
        .issue_settings
        .milestones
        .iter()
        .find(|milestone| milestone.title.eq_ignore_ascii_case(&title))
    {
        return Ok(Some(existing.id.clone()));
    }
    let milestone = service
        .upsert_issue_milestone(
            path,
            default_branch,
            UpsertIssueMilestone {
                id: None,
                title,
                description: String::new(),
            },
        )?
        .1;
    Ok(Some(milestone.id))
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
            Commands::InternalSupervisor { port, transport } => {
                run_internal_supervisor(
                    registry_store.data_root().to_path_buf(),
                    port,
                    transport.into(),
                )
                .await?;
                return Ok(());
            }
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
            Commands::Settings { command } => match command {
                SettingsCommands::View { path } => {
                    let (workspace, settings) =
                        service.read_repository_settings(path, default_branch)?;
                    print_repository_settings(&workspace, &settings);
                    return Ok(());
                }
                SettingsCommands::Set {
                    path,
                    description,
                    homepage,
                    default_branch: next_default_branch,
                } => {
                    let mut final_workspace =
                        service.resolve_workspace(path.clone(), default_branch)?;
                    if description.is_some() || homepage.is_some() {
                        let (workspace, settings) = service.update_repository_settings(
                            path.clone(),
                            default_branch,
                            UpdateRepositorySettings {
                                description,
                                homepage_url: homepage,
                            },
                        )?;
                        final_workspace = workspace;
                        say("updated repository metadata");
                        print_repository_settings(&final_workspace, &settings);
                    }
                    if let Some(branch_name) = next_default_branch {
                        let (workspace, outcome) = service
                            .switch_branch(path, default_branch, &branch_name)
                            .await?;
                        final_workspace = workspace;
                        say(&format!(
                            "switched default branch from `{}` to `{}` at `{}`",
                            outcome.previous_branch, outcome.current_branch, outcome.commit
                        ));
                    }
                    let (_, settings) = service.read_repository_settings(
                        final_workspace.worktree.clone(),
                        &final_workspace.exported_branch,
                    )?;
                    print_repository_settings(&final_workspace, &settings);
                    return Ok(());
                }
                SettingsCommands::Rule {
                    path,
                    pattern,
                    delete,
                    require_pr,
                    approvals,
                    dismiss_stale,
                    block_force_push,
                    block_delete,
                } => {
                    if let Some(pattern) = delete {
                        let (workspace, settings) =
                            service.delete_branch_rule(path, default_branch, &pattern)?;
                        say(&format!("deleted branch rule `{pattern}`"));
                        print_repository_settings(&workspace, &settings);
                        return Ok(());
                    }
                    if let Some(pattern) = pattern {
                        let approvals = approvals.unwrap_or(0);
                        let (workspace, settings) = service.upsert_branch_rule(
                            path,
                            default_branch,
                            UpsertBranchRule {
                                pattern: pattern.clone(),
                                require_pull_request: require_pr,
                                required_approvals: approvals,
                                dismiss_stale_approvals: dismiss_stale,
                                block_force_push,
                                block_delete,
                            },
                        )?;
                        say(&format!(
                            "saved branch rule `{}`",
                            format_branch_rule_summary(
                                &pattern,
                                require_pr,
                                approvals,
                                dismiss_stale,
                                block_force_push,
                                block_delete,
                            )
                        ));
                        print_repository_settings(&workspace, &settings);
                        return Ok(());
                    }
                    let (workspace, settings) =
                        service.read_repository_settings(path, default_branch)?;
                    print_repository_settings(&workspace, &settings);
                    return Ok(());
                }
            },
            Commands::Auth { command } => match command {
                AuthCommands::Mode {
                    path,
                    request_based,
                    shared_session,
                } => {
                    if request_based || shared_session {
                        let mode = if request_based {
                            AuthMode::RequestBased
                        } else {
                            AuthMode::SharedSession
                        };
                        let (workspace, auth) = service.update_auth_mode(
                            path,
                            default_branch,
                            mode.clone(),
                            &AuthActor::Operator,
                        )?;
                        say(&format!(
                            "set auth mode for {} to {}",
                            workspace.worktree.display(),
                            format_auth_mode(mode)
                        ));
                        print_auth_state(&workspace, &auth);
                        return Ok(());
                    }
                    let (workspace, auth) = service.read_auth_state(path, default_branch)?;
                    print_auth_state(&workspace, &auth);
                    return Ok(());
                }
                AuthCommands::Requests {
                    path,
                    approve,
                    reject,
                } => {
                    let (workspace, auth) =
                        service.read_auth_state(path.clone(), default_branch)?;
                    if let Some(selector) = approve {
                        let request = select_by_id_prefix(
                            &auth.access_requests,
                            &selector,
                            |request| &request.id,
                            "request",
                        )?;
                        let (_, _user, onboarding) = service.approve_access_request(
                            path,
                            default_branch,
                            &request.id,
                            RepoUserRole::User,
                            &AuthActor::Operator,
                        )?;
                        say(&format!("approved {} <{}>", request.name, request.email));
                        if let Some(secret) = onboarding.secret.as_deref() {
                            say("share this one-time setup token with the approved user now:");
                            say(&format!("  {}", secret));
                        } else {
                            say(
                                "no setup code: they can finish from the browser where they requested access.",
                            );
                        }
                        return Ok(());
                    }
                    if let Some(selector) = reject {
                        let request = select_by_id_prefix(
                            &auth.access_requests,
                            &selector,
                            |request| &request.id,
                            "request",
                        )?;
                        service.reject_access_request(
                            path,
                            default_branch,
                            &request.id,
                            &AuthActor::Operator,
                        )?;
                        say(&format!("rejected {} <{}>", request.name, request.email));
                        return Ok(());
                    }
                    say(&format!(
                        "access requests for {}:",
                        workspace.worktree.display()
                    ));
                    let pending = auth
                        .access_requests
                        .iter()
                        .filter(|request| request.status == AccessRequestStatus::Pending)
                        .collect::<Vec<_>>();
                    if pending.is_empty() {
                        say("  none");
                    } else {
                        for request in pending {
                            say(&format!(
                                "  {} {} <{}>",
                                request.id.chars().take(8).collect::<String>(),
                                request.name,
                                request.email
                            ));
                        }
                    }
                    return Ok(());
                }
                AuthCommands::Users {
                    path,
                    promote,
                    demote,
                    revoke,
                    reset_setup,
                } => {
                    let (workspace, auth) =
                        service.read_auth_state(path.clone(), default_branch)?;
                    if let Some(selector) = promote {
                        let user =
                            select_by_id_prefix(&auth.users, &selector, |user| &user.id, "user")?;
                        service.promote_user(
                            path,
                            default_branch,
                            &user.id,
                            &AuthActor::Operator,
                        )?;
                        say(&format!("promoted {} to owner", user.email));
                        return Ok(());
                    }
                    if let Some(selector) = demote {
                        let user =
                            select_by_id_prefix(&auth.users, &selector, |user| &user.id, "user")?;
                        service.demote_user(
                            path,
                            default_branch,
                            &user.id,
                            &AuthActor::Operator,
                        )?;
                        say(&format!("demoted {} to user", user.email));
                        return Ok(());
                    }
                    if let Some(selector) = revoke {
                        let user =
                            select_by_id_prefix(&auth.users, &selector, |user| &user.id, "user")?;
                        service.revoke_user(
                            path,
                            default_branch,
                            &user.id,
                            &AuthActor::Operator,
                        )?;
                        say(&format!("revoked {}", user.email));
                        return Ok(());
                    }
                    if let Some(selector) = reset_setup {
                        let user =
                            select_by_id_prefix(&auth.users, &selector, |user| &user.id, "user")?;
                        let (_, _, onboarding) = service.reset_user_setup(
                            path,
                            default_branch,
                            &user.id,
                            &AuthActor::Operator,
                        )?;
                        say(&format!("reset setup for {}", user.email));
                        say("share this one-time setup token with the user now:");
                        say(&format!(
                            "  {}",
                            onboarding
                                .secret
                                .as_deref()
                                .expect("reset issues a setup token")
                        ));
                        return Ok(());
                    }
                    say(&format!("users for {}:", workspace.worktree.display()));
                    if auth.users.is_empty() {
                        say("  none");
                    } else {
                        for user in &auth.users {
                            let role = match user.role {
                                RepoUserRole::Owner => "owner",
                                RepoUserRole::User => "user",
                            };
                            let status = match user.status {
                                RepoUserStatus::PendingRequest => "pending_request",
                                RepoUserStatus::ApprovedPendingSetup => "approved_pending_setup",
                                RepoUserStatus::Active => "active",
                                RepoUserStatus::Revoked => "revoked",
                            };
                            say(&format!(
                                "  {} {} <{}> role={} status={}",
                                user.id.chars().take(8).collect::<String>(),
                                user.name,
                                user.email,
                                role,
                                status
                            ));
                        }
                    }
                    return Ok(());
                }
                AuthCommands::Pats { path, revoke } => {
                    let (workspace, auth) =
                        service.read_auth_state(path.clone(), default_branch)?;
                    if let Some(selector) = revoke {
                        let pat = select_by_id_prefix(
                            &auth.personal_access_tokens,
                            &selector,
                            |token| &token.id,
                            "personal access token",
                        )?;
                        service.revoke_pat(path, default_branch, &pat.id, &AuthActor::Operator)?;
                        say(&format!("revoked PAT {}", pat.label));
                        return Ok(());
                    }
                    say(&format!(
                        "personal access tokens for {}:",
                        workspace.worktree.display()
                    ));
                    let active = auth
                        .personal_access_tokens
                        .iter()
                        .filter(|token| token.revoked_at_ms.is_none())
                        .collect::<Vec<_>>();
                    if active.is_empty() {
                        say("  none");
                    } else {
                        for token in active {
                            say(&format!(
                                "  {} {}",
                                token.id.chars().take(8).collect::<String>(),
                                token.label
                            ));
                        }
                    }
                    return Ok(());
                }
            },
            Commands::Issue { command } => match command {
                IssueCommands::List { path, state } => {
                    let (workspace, web_ui) =
                        load_issues(service.as_ref(), path, default_branch).await?;
                    say(&format!("issues for {}:", workspace.worktree.display()));
                    let mut count = 0usize;
                    for issue in web_ui
                        .issues
                        .iter()
                        .filter(|issue| issue_state_matches(issue, state))
                    {
                        count += 1;
                        let linked_count = WorkspaceService::linked_pull_requests_for_issue(
                            issue,
                            &web_ui.issues,
                            &web_ui.pull_requests,
                        )
                        .len();
                        say(&format!(
                            "  #{} [{}] {}  ({} comment{}, {} linked PR{})",
                            issue.number,
                            match issue.status {
                                IssueStatus::Open => "open",
                                IssueStatus::Closed => "closed",
                            },
                            issue.title,
                            issue.comments.len(),
                            if issue.comments.len() == 1 { "" } else { "s" },
                            linked_count,
                            if linked_count == 1 { "" } else { "s" }
                        ));
                    }
                    if count == 0 {
                        say("  no matching issues");
                    }
                    return Ok(());
                }
                IssueCommands::View { path, issue } => {
                    let (_workspace, web_ui) =
                        load_issues(service.as_ref(), path, default_branch).await?;
                    let issue = select_issue(&web_ui.issues, &issue)?;
                    print_issue_summary(
                        issue,
                        &web_ui.issue_settings.labels,
                        &web_ui.issue_settings.milestones,
                        &web_ui.auth,
                        &web_ui.issues,
                        &web_ui.pull_requests,
                    );
                    print_issue_discussion(issue);
                    print_issue_activity(
                        issue,
                        &web_ui.issue_settings.labels,
                        &web_ui.issue_settings.milestones,
                    );
                    return Ok(());
                }
                IssueCommands::Create {
                    path,
                    title,
                    body,
                    labels,
                    assignees,
                    milestone,
                    link_prs,
                } => {
                    let (_, web_ui) =
                        load_issues(service.as_ref(), path.clone(), default_branch).await?;
                    let label_ids =
                        ensure_label_ids(service.as_ref(), path.clone(), default_branch, &labels)
                            .await?;
                    let assignee_user_ids = assignee_ids_from_usernames(&web_ui, &assignees)?;
                    let milestone_id = ensure_milestone_id(
                        service.as_ref(),
                        path.clone(),
                        default_branch,
                        milestone,
                    )
                    .await?;
                    let mut linked_pull_request_ids = Vec::new();
                    if !link_prs.is_empty() {
                        let (_, pull_requests) =
                            load_pull_requests(service.as_ref(), path.clone(), default_branch)
                                .await?;
                        for selector in link_prs {
                            linked_pull_request_ids
                                .push(select_pull_request(&pull_requests, &selector)?.id.clone());
                        }
                    }
                    let issue = service
                        .create_issue(
                            path,
                            default_branch,
                            CreateIssue {
                                title,
                                description: body,
                                label_ids,
                                assignee_user_ids,
                                milestone_id,
                                linked_pull_request_ids,
                            },
                            cli_issue_actor(None),
                        )
                        .await?
                        .1;
                    say(&format!(
                        "created issue #{} ({})",
                        issue.number, issue.title
                    ));
                    return Ok(());
                }
                IssueCommands::Edit {
                    path,
                    issue,
                    title,
                    body,
                    open,
                    close,
                } => {
                    let (_, web_ui) =
                        load_issues(service.as_ref(), path.clone(), default_branch).await?;
                    let issue = select_issue(&web_ui.issues, &issue)?;
                    let updated = service
                        .update_issue(
                            path,
                            default_branch,
                            &issue.id,
                            UpdateIssue {
                                title,
                                description: body,
                                status: if open {
                                    Some(IssueStatus::Open)
                                } else if close {
                                    Some(IssueStatus::Closed)
                                } else {
                                    None
                                },
                            },
                            cli_issue_actor(None),
                        )
                        .await?
                        .1;
                    say(&format!(
                        "updated issue #{} [{}]",
                        updated.number,
                        match updated.status {
                            IssueStatus::Open => "open",
                            IssueStatus::Closed => "closed",
                        }
                    ));
                    return Ok(());
                }
                IssueCommands::Comment {
                    path,
                    issue,
                    body,
                    author,
                } => {
                    let (_, web_ui) =
                        load_issues(service.as_ref(), path.clone(), default_branch).await?;
                    let issue = select_issue(&web_ui.issues, &issue)?;
                    service
                        .comment_issue(
                            path,
                            default_branch,
                            &issue.id,
                            CreateIssueComment {
                                display_name: author.clone(),
                                body,
                            },
                            cli_issue_actor(author),
                        )
                        .await?;
                    say(&format!("added comment to issue #{}", issue.number));
                    return Ok(());
                }
                IssueCommands::Close { path, issue } => {
                    let (_, web_ui) =
                        load_issues(service.as_ref(), path.clone(), default_branch).await?;
                    let issue = select_issue(&web_ui.issues, &issue)?;
                    service
                        .update_issue(
                            path,
                            default_branch,
                            &issue.id,
                            UpdateIssue {
                                title: None,
                                description: None,
                                status: Some(IssueStatus::Closed),
                            },
                            cli_issue_actor(None),
                        )
                        .await?;
                    say(&format!("closed issue #{}", issue.number));
                    return Ok(());
                }
                IssueCommands::Reopen { path, issue } => {
                    let (_, web_ui) =
                        load_issues(service.as_ref(), path.clone(), default_branch).await?;
                    let issue = select_issue(&web_ui.issues, &issue)?;
                    service
                        .update_issue(
                            path,
                            default_branch,
                            &issue.id,
                            UpdateIssue {
                                title: None,
                                description: None,
                                status: Some(IssueStatus::Open),
                            },
                            cli_issue_actor(None),
                        )
                        .await?;
                    say(&format!("reopened issue #{}", issue.number));
                    return Ok(());
                }
                IssueCommands::Delete { path, issue } => {
                    let (_, web_ui) =
                        load_issues(service.as_ref(), path.clone(), default_branch).await?;
                    let issue = select_issue(&web_ui.issues, &issue)?;
                    service
                        .delete_issue(path, default_branch, &issue.id)
                        .await?;
                    say("deleted issue");
                    return Ok(());
                }
                IssueCommands::Label {
                    path,
                    issue,
                    add,
                    remove,
                } => {
                    let (_, web_ui) =
                        load_issues(service.as_ref(), path.clone(), default_branch).await?;
                    let issue = select_issue(&web_ui.issues, &issue)?;
                    let mut next_label_ids = issue.label_ids.clone();
                    let add_ids =
                        ensure_label_ids(service.as_ref(), path.clone(), default_branch, &add)
                            .await?;
                    for label_id in add_ids {
                        if !next_label_ids.iter().any(|existing| existing == &label_id) {
                            next_label_ids.push(label_id);
                        }
                    }
                    for name in remove
                        .iter()
                        .map(|name| name.trim())
                        .filter(|name| !name.is_empty())
                    {
                        let label = web_ui
                            .issue_settings
                            .labels
                            .iter()
                            .find(|label| label.name.eq_ignore_ascii_case(name))
                            .ok_or_else(|| anyhow!("label `{name}` was not found"))?;
                        next_label_ids.retain(|label_id| label_id != &label.id);
                    }
                    service
                        .set_issue_labels(
                            path,
                            default_branch,
                            &issue.id,
                            next_label_ids,
                            cli_issue_actor(None),
                        )
                        .await?;
                    say("updated issue labels");
                    return Ok(());
                }
                IssueCommands::Assign {
                    path,
                    issue,
                    add,
                    remove,
                } => {
                    let (_, web_ui) =
                        load_issues(service.as_ref(), path.clone(), default_branch).await?;
                    let issue = select_issue(&web_ui.issues, &issue)?;
                    let mut next_assignee_ids = issue.assignee_user_ids.clone();
                    let add_ids = assignee_ids_from_usernames(&web_ui, &add)?;
                    for user_id in add_ids {
                        if !next_assignee_ids
                            .iter()
                            .any(|existing| existing == &user_id)
                        {
                            next_assignee_ids.push(user_id);
                        }
                    }
                    for username in remove
                        .iter()
                        .map(|username| username.trim())
                        .filter(|username| !username.is_empty())
                    {
                        let user = web_ui
                            .auth
                            .users
                            .iter()
                            .find(|user| {
                                user.username
                                    .as_deref()
                                    .is_some_and(|candidate| candidate == username)
                            })
                            .ok_or_else(|| anyhow!("user `{username}` was not found"))?;
                        next_assignee_ids.retain(|user_id| user_id != &user.id);
                    }
                    service
                        .set_issue_assignees(
                            path,
                            default_branch,
                            &issue.id,
                            next_assignee_ids,
                            cli_issue_actor(None),
                        )
                        .await?;
                    say("updated issue assignees");
                    return Ok(());
                }
                IssueCommands::Milestone {
                    path,
                    issue,
                    set,
                    clear,
                } => {
                    let (_, web_ui) =
                        load_issues(service.as_ref(), path.clone(), default_branch).await?;
                    let issue = select_issue(&web_ui.issues, &issue)?;
                    let milestone_id = if clear {
                        None
                    } else {
                        ensure_milestone_id(service.as_ref(), path.clone(), default_branch, set)
                            .await?
                    };
                    service
                        .set_issue_milestone(
                            path,
                            default_branch,
                            &issue.id,
                            milestone_id,
                            cli_issue_actor(None),
                        )
                        .await?;
                    say("updated issue milestone");
                    return Ok(());
                }
                IssueCommands::LinkPr {
                    path,
                    issue,
                    pull_request,
                } => {
                    let (_, web_ui) =
                        load_issues(service.as_ref(), path.clone(), default_branch).await?;
                    let issue = select_issue(&web_ui.issues, &issue)?;
                    let (_, pull_requests) =
                        load_pull_requests(service.as_ref(), path.clone(), default_branch).await?;
                    let pull_request = select_pull_request(&pull_requests, &pull_request)?;
                    service
                        .link_issue_pull_request(
                            path,
                            default_branch,
                            &issue.id,
                            &pull_request.id,
                            cli_issue_actor(None),
                        )
                        .await?;
                    say("linked pull request");
                    return Ok(());
                }
            },
            Commands::Pr { command } => match command {
                PrCommands::List { path, state } => {
                    let (workspace, pull_requests) =
                        load_pull_requests(service.as_ref(), path, default_branch).await?;
                    say(&format!(
                        "pull requests for {}:",
                        workspace.worktree.display()
                    ));
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
                    let (_workspace, web_ui, pull_request, comparison, diffs) =
                        load_pull_request_detail(
                            service.as_ref(),
                            repo_store.as_ref(),
                            path,
                            default_branch,
                            &pull_request,
                        )
                        .await?;
                    print_pr_summary(
                        &pull_request,
                        &web_ui.issues,
                        &web_ui.pull_requests,
                        comparison.as_ref(),
                    );
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
                    let (_workspace, _web_ui, pull_request, _comparison, diffs) =
                        load_pull_request_detail(
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
                                pull_request.source_branch,
                                pull_request.target_branch,
                                pull_request.title
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
                                pull_request.source_branch,
                                pull_request.target_branch,
                                pull_request.title
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
                                source_branch: head
                                    .unwrap_or_else(|| workspace.checked_out_branch.clone()),
                                target_branch: base
                                    .unwrap_or_else(|| workspace.exported_branch.clone()),
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
                        .merge_pull_request(
                            path,
                            default_branch,
                            &pull_request.id,
                            cli_issue_actor(None),
                        )
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
            },
        }
    }

    run_shared_serve(cli, repo_store, registry_store, service).await
}

async fn run_shared_serve(
    cli: Cli,
    repo_store: Arc<GitRepoStore>,
    registry_store: Arc<FilesystemRegistry>,
    service: Arc<WorkspaceService>,
) -> Result<()> {
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
    let requested_auth_methods = if !cli.auth_methods.is_empty() {
        cli.auth_methods
            .iter()
            .copied()
            .map(Into::into)
            .collect::<Vec<_>>()
    } else if let Some(mode) = cli.auth_mode.map(Into::into) {
        RepoAuthState::methods_for_mode(&mode)
    } else {
        Vec::new()
    };
    if !requested_auth_methods.is_empty() {
        service.update_auth_methods(
            workspace.worktree.clone(),
            &workspace.exported_branch,
            requested_auth_methods,
            &AuthActor::Operator,
        )?;
    }
    let effective_auth = service
        .read_auth_state(workspace.worktree.clone(), &workspace.exported_branch)?
        .1;
    let reveal_password = !cli.hidden_pass;
    let credentials_path = write_credentials_file(
        &credentials,
        effective_auth.has_method(&AuthMethod::BasicAuth) && !reveal_password,
    )?;
    let current_exe = std::env::current_exe().context("locate qit executable")?;
    let transport = if cli.local_only {
        PublicTransport::Local
    } else {
        cli.transport
            .map(Into::into)
            .unwrap_or(PublicTransport::Ngrok)
    };
    let shared_entrypoint = ensure_supervisor(
        &current_exe,
        registry_store.data_root(),
        cli.port,
        transport,
    )
    .await?;
    let route_lease = claim_mount_path(
        &shared_entrypoint,
        workspace.id,
        &repo_name_from_worktree(&workspace.worktree),
    )
    .await?;
    let request_scheme = if shared_entrypoint.public_base_url.scheme() == "https" {
        "https".to_string()
    } else {
        "http".to_string()
    };
    let listener = TcpListener::bind(("127.0.0.1", 0))
        .await
        .context("bind qit worker listener")?;
    let upstream_url = Url::parse(&format!(
        "http://127.0.0.1:{}/",
        listener
            .local_addr()
            .context("read qit worker addr")?
            .port()
    ))
    .context("build qit worker URL")?;
    let worker_app = build_worker_app(
        repo_store.clone(),
        registry_store.clone(),
        service.clone(),
        workspace.clone(),
        credentials.clone(),
        route_lease.mount_path.clone(),
        cli.auto_apply,
        cli.max_body_bytes,
        request_scheme,
        shared_entrypoint.public_base_url.as_str(),
    );
    register_route(
        &shared_entrypoint,
        &route_lease,
        &upstream_url,
        &workspace.worktree,
    )
    .await?;

    let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel::<()>();
    let mut serve = tokio::spawn(async move {
        axum::serve(listener, worker_app)
            .with_graceful_shutdown(async move {
                let _ = shutdown_rx.await;
            })
            .await
            .map_err(anyhow::Error::new)
    });

    let local_browser_url = repo_url(&shared_entrypoint.local_base_url, &route_lease.mount_path)?;
    let public_repo_url = repo_url(&shared_entrypoint.public_base_url, &route_lease.mount_path)?;
    let clone_url = if effective_auth.has_method(&AuthMethod::BasicAuth) {
        repo_url_with_credentials(&public_repo_url, &credentials, reveal_password)?
    } else {
        public_repo_url.clone()
    };
    let shared_transport = match shared_entrypoint.label.as_str() {
        "NGROK" => PublicTransport::Ngrok,
        "TAILSCALE" => PublicTransport::Tailscale,
        "LAN" => PublicTransport::Lan,
        _ => PublicTransport::Local,
    };
    let clone_cmd = clone_command(&clone_url, shared_transport);
    print_serve_summary(
        &workspace.worktree,
        &workspace.exported_branch,
        effective_auth.methods.clone(),
        &shared_entrypoint.label,
        &public_repo_url,
        &local_browser_url,
        &public_repo_url,
        &credentials,
        credentials_path.as_deref(),
        reveal_password,
        &clone_cmd,
        cli.auto_apply,
        shared_entrypoint
            .local_base_url
            .as_str()
            .trim_end_matches('/'),
        &route_lease.mount_path,
    );
    println!("Ctrl+C to stop.");
    println!();
    let _ = std::io::stdout().flush();

    let heartbeat_entrypoint = shared_entrypoint.clone();
    let heartbeat_lease = route_lease.clone();
    let mut heartbeat = tokio::spawn(async move {
        loop {
            tokio::time::sleep(heartbeat_interval()).await;
            heartbeat_route(&heartbeat_entrypoint, &heartbeat_lease).await?;
        }
        #[allow(unreachable_code)]
        Ok::<(), anyhow::Error>(())
    });

    tokio::select! {
        signal = tokio::signal::ctrl_c() => {
            signal.context("ctrl_c")?;
        }
        result = &mut serve => {
            match result {
                Ok(Ok(())) => bail!("qit worker server exited unexpectedly"),
                Ok(Err(error)) => return Err(error),
                Err(error) => bail!("qit worker server task failed: {error}"),
            }
        }
        result = &mut heartbeat => {
            match result {
                Ok(Ok(())) => bail!("qit supervisor heartbeat stopped unexpectedly"),
                Ok(Err(error)) => return Err(error),
                Err(error) => bail!("qit supervisor heartbeat task failed: {error}"),
            }
        }
    }

    heartbeat.abort();
    let _ = shutdown_tx.send(());
    let _ = unregister_route(&shared_entrypoint, &route_lease).await;
    match timeout(Duration::from_secs(5), serve).await {
        Ok(Ok(Ok(()))) => {}
        Ok(Ok(Err(error))) => return Err(error),
        Ok(Err(error)) => bail!("qit worker server task failed: {error}"),
        Err(_) => bail!("timed out waiting for qit worker server shutdown"),
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn build_worker_app(
    repo_store: Arc<GitRepoStore>,
    registry_store: Arc<FilesystemRegistry>,
    service: Arc<WorkspaceService>,
    workspace: WorkspaceSpec,
    credentials: SessionCredentials,
    mount_path: String,
    auto_apply: bool,
    max_body_bytes: usize,
    request_scheme: String,
    public_base_url: &str,
) -> axum::Router {
    let git_router = GitHttpServer::new(
        Arc::new(GitHttpBackendAdapter),
        registry_store,
        service.clone(),
        GitHttpServerConfig {
            workspace: workspace.clone(),
            credentials: credentials.clone(),
            auto_apply,
            repo_mount_path: mount_path.clone(),
            request_scheme,
            max_body_bytes,
        },
    )
    .git_router();
    let public_repo_url = format!(
        "{}/{}",
        public_base_url.trim_end_matches('/'),
        mount_path.trim_start_matches('/')
    );
    let web_router = WebUiServer::new(
        repo_store,
        service,
        WebUiConfig {
            workspace,
            repo_mount_path: mount_path,
            credentials,
            implicit_owner_mode: true,
            secure_cookies: public_base_url.starts_with("https://"),
            public_repo_url: Some(public_repo_url),
        },
    )
    .router();
    web_router.merge(git_router)
}
