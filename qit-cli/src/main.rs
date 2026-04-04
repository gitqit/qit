use anyhow::{anyhow, bail, Context, Result};
use clap::{ArgAction, Parser, Subcommand, ValueEnum};
use qit_domain::{
    BranchInfo, CredentialIssuer, SessionCredentials, WorkspaceService, DEFAULT_BRANCH,
};
use qit_git::{GitHttpBackendAdapter, GitRepoStore};
use qit_http::{repo_mount_path, GitHttpServer, DEFAULT_MAX_BODY_BYTES};
use qit_storage::FilesystemRegistry;
use qit_transports::{expose, PublicTransport};
use rand::distributions::{Alphanumeric, DistString};
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

    /// Folder to publish (no `.git` in this folder required).
    path: Option<PathBuf>,

    /// Public transport to expose the repo.
    #[arg(long, value_enum)]
    transport: Option<TransportArg>,

    /// After successful pushes, fast-forward the host folder when it is clean.
    #[arg(long)]
    auto_apply: bool,

    /// Hide the password from stdout and omit it from the suggested clone command.
    #[arg(long)]
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
    hidden_pass: bool,
) -> Result<Url> {
    let mut url = public_url.clone();
    if hidden_pass {
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

fn print_banner(
    label: &str,
    public_url: &Url,
    credentials: &SessionCredentials,
    credentials_path: &Path,
    hidden_pass: bool,
    clone_cmd: &str,
    note: &str,
) {
    println!();
    println!("===========================================================");
    println!(
        "  {label} URL  -> {}/",
        public_url.as_str().trim_end_matches('/')
    );
    println!("===========================================================");
    println!();
    println!("Session credentials:");
    println!("  username: {}", credentials.username);
    println!("  file: {}", credentials_path.display());
    if hidden_pass {
        println!("  password: hidden from stdout; open the credentials file above if you need it");
    } else {
        println!("  password: {}", credentials.password);
    }
    println!();
    println!("Repo URL:");
    println!("  {}/", public_url.as_str().trim_end_matches('/'));
    println!();
    println!("Clone this repo:");
    println!("  {clone_cmd}");
    if hidden_pass {
        println!(
            "  Git will prompt for the username/password from the credentials file unless you use a credential helper."
        );
    } else {
        println!("  The clone command already includes the session credentials.");
    }
    println!();
    println!("{note}");
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

                let (workspace, branches) = service.list_branches(path, default_branch, &list).await?;
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
        }
    }

    let path = cli.path.context("path is required")?;
    say("qit: starting...");

    let prepared = service
        .prepare_serve(path, cli.branch.as_deref(), "qit snapshot")
        .await?;
    let workspace = prepared.workspace.clone();
    let credentials = prepared.credentials.clone();
    let credentials_path = write_credentials_file(&credentials)?;
    let repo_mount_path = repo_mount_path(&repo_name_from_worktree(&workspace.worktree));

    say(&format!("  folder: {}", workspace.worktree.display()));
    say(&format!("  sidecar: {}", workspace.sidecar.display()));
    if let Some(snapshot_commit) = &prepared.snapshot_commit {
        say(&format!("  snapshot: {snapshot_commit}"));
    } else {
        say("  snapshot: no file changes detected");
    }
    if cli.auto_apply {
        say("  auto-apply: enabled");
    }

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

    let server = GitHttpServer::new(
        Arc::new(GitHttpBackendAdapter),
        registry_store,
        service.clone(),
        workspace.clone(),
        credentials.clone(),
        cli.auto_apply,
        repo_mount_path.clone(),
        request_scheme,
        cli.max_body_bytes,
    )
    .router();
    let (shutdown_tx, shutdown_rx) = tokio::sync::oneshot::channel::<()>();
    let serve = tokio::spawn(async move {
        axum::serve(listener, server)
            .with_graceful_shutdown(async move {
                let _ = shutdown_rx.await;
            })
            .await
            .map_err(|err| anyhow::anyhow!(err))
    });

    say(&format!(
        "  local Git HTTP: {}/",
        local_url.as_str().trim_end_matches('/')
    ));
    let endpoint = expose(transport, &local_url).await?;
    let public_repo_url = repo_url(&endpoint.public_url, &repo_mount_path)?;
    let clone_url = repo_url_with_credentials(&public_repo_url, &credentials, cli.hidden_pass)?;
    let clone_cmd = clone_command(&clone_url, transport);
    print_banner(
        endpoint.label,
        &public_repo_url,
        &credentials,
        &credentials_path,
        cli.hidden_pass,
        &clone_cmd,
        &endpoint.note,
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
    endpoint.shutdown().await.context("shutdown public endpoint")?;
    Ok(())
}
