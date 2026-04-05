use anyhow::{anyhow, bail, Context, Result};
use qit_domain::{CredentialIssuer, SessionCredentials, WorkspaceService};
use qit_git::{GitHttpBackendAdapter, GitRepoStore};
use qit_http::{repo_mount_path, GitHttpServer, GitHttpServerConfig, DEFAULT_MAX_BODY_BYTES};
use qit_storage::FilesystemRegistry;
use qit_webui::{WebUiConfig, WebUiServer};
use std::io::{BufRead, BufReader};
use std::net::TcpListener as StdTcpListener;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Output, Stdio};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::{Duration, Instant};
use tempfile::TempDir;
use tokio::net::TcpListener;
use url::Url;

struct StaticIssuer;

impl CredentialIssuer for StaticIssuer {
    fn issue(&self) -> SessionCredentials {
        SessionCredentials {
            username: "tester".into(),
            password: "secret".into(),
        }
    }
}

fn workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .to_path_buf()
}

fn qit_binary_path() -> Result<PathBuf> {
    static BIN_PATH: OnceLock<Mutex<Option<PathBuf>>> = OnceLock::new();
    let cache = BIN_PATH.get_or_init(|| Mutex::new(None));
    let mut cached = cache.lock().unwrap();
    if let Some(path) = cached.clone() {
        return Ok(path);
    }

    let root = workspace_root();
    let bin_name = if cfg!(windows) { "qit.exe" } else { "qit" };
    let bin = root.join("target").join("debug").join(bin_name);
    let status = Command::new("cargo")
        .arg("build")
        .arg("--manifest-path")
        .arg(root.join("Cargo.toml"))
        .arg("-p")
        .arg("qit")
        .status()
        .context("build qit binary")?;
    if !status.success() {
        bail!("failed to build qit binary");
    }
    *cached = Some(bin.clone());
    Ok(bin)
}

fn free_port() -> Result<u16> {
    let listener = StdTcpListener::bind("127.0.0.1:0").context("reserve free port")?;
    Ok(listener.local_addr()?.port())
}

fn run_git(dir: Option<&Path>, args: &[&str]) -> Result<()> {
    let mut command = Command::new("git");
    if let Some(dir) = dir {
        command.current_dir(dir);
    }
    let status = command
        .args(args)
        .status()
        .with_context(|| format!("run git {:?}", args))?;
    if !status.success() {
        bail!("git command failed: {:?}", args);
    }
    Ok(())
}

fn spawn_qit_serve(
    worktree: &Path,
    port: u16,
) -> Result<(Child, std::sync::mpsc::Receiver<String>)> {
    spawn_qit_serve_with_options(worktree, port, false, None, true, Some("shared-session"))
}

fn spawn_qit_serve_with_env(
    worktree: &Path,
    port: u16,
    auto_apply: bool,
    data_dir: Option<&Path>,
) -> Result<(Child, std::sync::mpsc::Receiver<String>)> {
    spawn_qit_serve_with_options(
        worktree,
        port,
        auto_apply,
        data_dir,
        true,
        Some("shared-session"),
    )
}

fn spawn_qit_serve_with_auth_mode(
    worktree: &Path,
    port: u16,
    data_dir: Option<&Path>,
    auth_mode: &str,
) -> Result<(Child, std::sync::mpsc::Receiver<String>)> {
    spawn_qit_serve_with_options(worktree, port, false, data_dir, true, Some(auth_mode))
}

fn spawn_qit_serve_with_options(
    worktree: &Path,
    port: u16,
    auto_apply: bool,
    data_dir: Option<&Path>,
    show_pass: bool,
    auth_mode: Option<&str>,
) -> Result<(Child, std::sync::mpsc::Receiver<String>)> {
    let bin = qit_binary_path()?;
    let mut command = Command::new(bin);
    command
        .arg("--transport")
        .arg("local")
        .arg("--port")
        .arg(port.to_string());
    if !show_pass {
        command.arg("--hidden-pass");
    }
    if let Some(auth_mode) = auth_mode {
        command.arg("--auth-mode").arg(auth_mode);
    }
    if auto_apply {
        command.arg("--auto-apply");
    }
    if let Some(data_dir) = data_dir {
        command.env("QIT_DATA_DIR", data_dir);
    }
    let mut child = command
        .arg(worktree)
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .context("spawn qit serve")?;

    let stdout = child.stdout.take().context("take qit stdout")?;
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::spawn(move || {
        let reader = BufReader::new(stdout);
        for line in reader.lines().map_while(Result::ok) {
            let _ = tx.send(line);
        }
    });

    Ok((child, rx))
}

fn run_qit_output(args: &[&str]) -> Result<Output> {
    run_qit_output_with_env(args, None)
}

fn run_qit_output_with_env(args: &[&str], data_dir: Option<&Path>) -> Result<Output> {
    let bin = qit_binary_path()?;
    let mut command = Command::new(bin);
    if let Some(data_dir) = data_dir {
        command.env("QIT_DATA_DIR", data_dir);
    }
    command
        .args(args)
        .output()
        .with_context(|| format!("run qit {:?}", args))
}

fn run_qit(args: &[&str]) -> Result<String> {
    run_qit_with_env(args, None)
}

fn run_qit_with_env(args: &[&str], data_dir: Option<&Path>) -> Result<String> {
    let output = run_qit_output_with_env(args, data_dir)?;
    if !output.status.success() {
        bail!(
            "qit command failed: {:?}\nstdout:\n{}\nstderr:\n{}",
            args,
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }
    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

fn first_pr_selector(output: &str) -> Result<String> {
    output
        .lines()
        .find_map(|line| {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with("pull requests for ") {
                return None;
            }
            trimmed.split_whitespace().next().map(ToString::to_string)
        })
        .ok_or_else(|| anyhow!("failed to parse pull request selector from output"))
}

fn wait_for_clone_url(rx: &std::sync::mpsc::Receiver<String>) -> Result<String> {
    let deadline = Instant::now() + Duration::from_secs(20);
    let mut clone_url = None;
    let mut username = None;
    let mut password = None;
    while Instant::now() < deadline {
        let remaining = deadline.saturating_duration_since(Instant::now());
        match rx.recv_timeout(remaining.min(Duration::from_millis(250))) {
            Ok(line) if line.contains("git clone ") => {
                clone_url = line
                    .split_whitespace()
                    .last()
                    .map(ToString::to_string)
                    .ok_or_else(|| anyhow!("failed to parse clone command line"))
                    .ok();
            }
            Ok(line) if line.trim_start().starts_with("username: ") => {
                username = line
                    .split_once(':')
                    .map(|(_, value)| value.trim().to_string());
            }
            Ok(line) if line.trim_start().starts_with("password: ") => {
                password = line
                    .split_once(':')
                    .map(|(_, value)| value.trim().to_string());
            }
            Ok(line) if line.contains("Ctrl+C to stop.") => {
                if let Some(clone_url) = clone_url.clone() {
                    if let (Some(username), Some(password)) = (username.clone(), password.clone()) {
                        let url = Url::parse(&clone_url).context("parse qit clone URL")?;
                        if !url.username().is_empty() || url.password().is_some() {
                            return Ok(url.to_string());
                        }
                        let mut url = url;
                        url.set_username(&username)
                            .map_err(|_| anyhow!("failed to encode username in URL"))?;
                        url.set_password(Some(&password))
                            .map_err(|_| anyhow!("failed to encode password in URL"))?;
                        return Ok(url.to_string());
                    }
                    return Ok(clone_url);
                }
            }
            Ok(_) => continue,
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => continue,
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                bail!("qit server exited before printing a clone URL")
            }
        }
        if let (Some(clone_url), Some(username), Some(password)) =
            (clone_url.clone(), username.clone(), password.clone())
        {
            let url = Url::parse(&clone_url).context("parse qit clone URL")?;
            if !url.username().is_empty() || url.password().is_some() {
                return Ok(url.to_string());
            }
            let mut url = url;
            url.set_username(&username)
                .map_err(|_| anyhow!("failed to encode username in URL"))?;
            url.set_password(Some(&password))
                .map_err(|_| anyhow!("failed to encode password in URL"))?;
            return Ok(url.to_string());
        }
    }
    bail!("timed out waiting for qit clone URL")
}

async fn start_server(
    root: &TempDir,
    auto_apply: bool,
) -> Result<(
    u16,
    tokio::task::JoinHandle<std::io::Result<()>>,
    SessionCredentials,
    PathBuf,
    String,
)> {
    let worktree = root.path().join("host");
    std::fs::create_dir_all(&worktree)?;
    std::fs::write(worktree.join("README.md"), "hello\n")?;

    let repo_store = Arc::new(GitRepoStore);
    let registry = Arc::new(FilesystemRegistry::with_root(root.path().join("data")));
    let issuer = Arc::new(StaticIssuer);
    let service = Arc::new(WorkspaceService::new(
        repo_store.clone(),
        registry.clone(),
        issuer,
    ));
    let prepared = service
        .prepare_serve(
            worktree.clone(),
            Some("main"),
            "integration snapshot",
            false,
        )
        .await
        .map_err(|err| anyhow!(err.to_string()))?;
    let credentials = prepared.credentials.clone();
    let workspace = prepared.workspace.clone();
    service
        .update_auth_mode(
            workspace.worktree.clone(),
            &workspace.exported_branch,
            qit_domain::AuthMode::SharedSession,
            &qit_domain::AuthActor::Operator,
        )
        .map_err(|err| anyhow!(err.to_string()))?;
    let mount_path = repo_mount_path("host");

    let listener = TcpListener::bind(("127.0.0.1", 0)).await?;
    let port = listener.local_addr()?.port();
    let git_router = GitHttpServer::new(
        Arc::new(GitHttpBackendAdapter),
        registry.clone(),
        service.clone(),
        GitHttpServerConfig {
            workspace: workspace.clone(),
            credentials: credentials.clone(),
            auto_apply,
            repo_mount_path: mount_path.clone(),
            request_scheme: "http".into(),
            max_body_bytes: DEFAULT_MAX_BODY_BYTES,
        },
    )
    .git_router();
    let web_router = WebUiServer::new(
        repo_store.clone(),
        service.clone(),
        WebUiConfig {
            workspace,
            repo_mount_path: mount_path.clone(),
            credentials: credentials.clone(),
            implicit_owner_mode: false,
            secure_cookies: false,
            public_repo_url: None,
        },
    )
    .router();
    let app = web_router.merge(git_router);
    let task = tokio::spawn(async move {
        axum::serve(
            listener,
            app.into_make_service_with_connect_info::<std::net::SocketAddr>(),
        )
        .await
    });
    Ok((port, task, credentials, worktree, mount_path))
}

fn wait_for_file_contents(path: &Path, expected: &str) -> Result<()> {
    let deadline = Instant::now() + Duration::from_secs(10);
    while Instant::now() < deadline {
        if std::fs::read_to_string(path)
            .ok()
            .map(|contents| contents.replace("\r\n", "\n"))
            .as_deref()
            == Some(expected)
        {
            return Ok(());
        }
        std::thread::sleep(Duration::from_millis(200));
    }
    bail!("timed out waiting for {}", path.display())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn git_http_requires_basic_auth() -> Result<()> {
    let root = TempDir::new()?;
    let (port, task, credentials, _worktree, mount_path) = start_server(&root, false).await?;

    let client = reqwest::Client::new();
    let url = format!("http://127.0.0.1:{port}{mount_path}/info/refs?service=git-upload-pack");
    let unauthorized = client.get(&url).send().await?;
    assert_eq!(unauthorized.status(), reqwest::StatusCode::UNAUTHORIZED);

    let authorized = client
        .get(&url)
        .basic_auth(&credentials.username, Some(&credentials.password))
        .send()
        .await?;
    assert!(authorized.status().is_success());

    task.abort();
    Ok(())
}

#[test]
fn serve_rejects_existing_git_worktree_without_flag() -> Result<()> {
    let root = TempDir::new()?;
    let worktree = root.path().join("repo");
    std::fs::create_dir_all(&worktree)?;
    run_git(Some(&worktree), &["init"])?;

    let output = run_qit_output(&["--transport", "local", worktree.to_str().unwrap()])?;

    assert!(!output.status.success());
    assert!(String::from_utf8_lossy(&output.stderr).contains("--allow-existing-git"));
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn auto_apply_updates_clean_worktree_after_push() -> Result<()> {
    let root = TempDir::new()?;
    let (port, task, credentials, worktree, mount_path) = start_server(&root, true).await?;
    let clone_url = {
        let mut url = Url::parse(&format!("http://127.0.0.1:{port}{mount_path}"))?;
        url.set_username(&credentials.username).unwrap();
        url.set_password(Some(&credentials.password)).unwrap();
        url.to_string()
    };

    let clone_dir = root.path().join("clone");
    run_git(None, &["clone", &clone_url, clone_dir.to_str().unwrap()])?;
    run_git(Some(&clone_dir), &["config", "user.name", "Qit Tester"])?;
    run_git(
        Some(&clone_dir),
        &["config", "user.email", "qit@example.com"],
    )?;
    std::fs::write(clone_dir.join("README.md"), "updated\n")?;
    run_git(Some(&clone_dir), &["commit", "-am", "update readme"])?;
    run_git(Some(&clone_dir), &["push", "origin", "HEAD:main"])?;

    wait_for_file_contents(&worktree.join("README.md"), "updated\n")?;
    task.abort();
    Ok(())
}

#[test]
fn qit_cli_serves_and_manual_apply_updates_host() -> Result<()> {
    let root = TempDir::new()?;
    let worktree = root.path().join("host");
    std::fs::create_dir_all(&worktree)?;
    std::fs::write(worktree.join("README.md"), "initial\n")?;

    let port = free_port()?;
    let (mut child, rx) = spawn_qit_serve(&worktree, port)?;
    let clone_url = wait_for_clone_url(&rx)?;
    assert!(clone_url.contains("/host"));

    let clone_dir = root.path().join("clone");
    run_git(None, &["clone", &clone_url, clone_dir.to_str().unwrap()])?;
    run_git(Some(&clone_dir), &["config", "user.name", "Qit Tester"])?;
    run_git(
        Some(&clone_dir),
        &["config", "user.email", "qit@example.com"],
    )?;
    std::fs::write(clone_dir.join("README.md"), "from clone\n")?;
    run_git(Some(&clone_dir), &["commit", "-am", "rewrite readme"])?;
    run_git(Some(&clone_dir), &["push", "origin", "HEAD:main"])?;

    assert_eq!(
        std::fs::read_to_string(worktree.join("README.md"))?.replace("\r\n", "\n"),
        "initial\n"
    );

    let bin = qit_binary_path()?;
    let status = Command::new(bin)
        .arg("apply")
        .arg(&worktree)
        .status()
        .context("run qit apply")?;
    if !status.success() {
        bail!("qit apply failed");
    }

    wait_for_file_contents(&worktree.join("README.md"), "from clone\n")?;
    let _ = child.kill();
    let _ = child.wait();
    Ok(())
}

#[tokio::test]
async fn qit_cli_shared_supervisor_serves_multiple_repos_on_one_entrypoint() -> Result<()> {
    let root = TempDir::new()?;
    let data_dir = root.path().join("data");
    let worktree_one = root.path().join("alpha").join("host");
    let worktree_two = root.path().join("beta").join("host");
    std::fs::create_dir_all(&worktree_one)?;
    std::fs::create_dir_all(&worktree_two)?;
    std::fs::write(worktree_one.join("README.md"), "alpha\n")?;
    std::fs::write(worktree_two.join("README.md"), "beta\n")?;

    let port = free_port()?;
    let (mut first_child, first_rx) =
        spawn_qit_serve_with_env(&worktree_one, port, false, Some(&data_dir))?;
    let first_clone_url = wait_for_clone_url(&first_rx)?;
    let (mut second_child, second_rx) =
        spawn_qit_serve_with_env(&worktree_two, port, false, Some(&data_dir))?;
    let second_clone_url = wait_for_clone_url(&second_rx)?;

    let first_url = Url::parse(&first_clone_url)?;
    let second_url = Url::parse(&second_clone_url)?;
    assert_eq!(first_url.host_str(), second_url.host_str());
    assert_eq!(first_url.port_or_known_default(), second_url.port_or_known_default());
    assert_ne!(first_url.path(), second_url.path());
    assert_eq!(first_url.path(), "/host/");
    assert!(second_url.path().starts_with("/host-"));

    let status_page = reqwest::get(format!("http://127.0.0.1:{port}/"))
        .await?
        .text()
        .await?;
    assert!(status_page.contains("/host"));
    assert!(status_page.contains(second_url.path().trim_end_matches('/')));

    let clone_one = root.path().join("clone-one");
    let clone_two = root.path().join("clone-two");
    run_git(None, &["clone", &first_clone_url, clone_one.to_str().unwrap()])?;
    run_git(None, &["clone", &second_clone_url, clone_two.to_str().unwrap()])?;
    assert_eq!(
        std::fs::read_to_string(clone_one.join("README.md"))?.replace("\r\n", "\n"),
        "alpha\n"
    );
    assert_eq!(
        std::fs::read_to_string(clone_two.join("README.md"))?.replace("\r\n", "\n"),
        "beta\n"
    );

    let _ = second_child.kill();
    let _ = second_child.wait();
    let _ = first_child.kill();
    let _ = first_child.wait();
    Ok(())
}

#[tokio::test]
async fn remote_host_through_shared_supervisor_does_not_get_local_operator_mode() -> Result<()> {
    let root = TempDir::new()?;
    let data_dir = root.path().join("data");
    let worktree = root.path().join("host");
    std::fs::create_dir_all(&worktree)?;
    std::fs::write(worktree.join("README.md"), "hello\n")?;

    let port = free_port()?;
    let (mut child, rx) = spawn_qit_serve_with_env(&worktree, port, false, Some(&data_dir))?;
    let clone_url = wait_for_clone_url(&rx)?;
    let mount_path = Url::parse(&clone_url)?.path().trim_end_matches('/').to_string();

    let payload = reqwest::Client::new()
        .get(format!("http://127.0.0.1:{port}{mount_path}/api/bootstrap"))
        .header("Host", "demo.example.ts.net")
        .send()
        .await?
        .text()
        .await?;

    let _ = child.kill();
    let _ = child.wait();

    assert!(payload.contains("\"operator_override\":false"));
    assert!(payload.contains("\"actor\":null"));
    Ok(())
}

#[test]
fn qit_cli_hides_password_and_clone_credentials_by_default() -> Result<()> {
    let root = TempDir::new()?;
    let worktree = root.path().join("host");
    std::fs::create_dir_all(&worktree)?;
    std::fs::write(worktree.join("README.md"), "initial\n")?;

    let port = free_port()?;
    let (mut child, rx) =
        spawn_qit_serve_with_options(&worktree, port, false, None, false, Some("shared-session"))?;
    let deadline = Instant::now() + Duration::from_secs(20);
    let mut saw_hidden_password = false;
    let mut saw_credentials_file = false;
    let mut saw_uncredentialed_clone = false;
    let mut saw_old_banner = false;
    let mut saw_internal_startup_noise = false;
    while Instant::now() < deadline {
        match rx.recv_timeout(Duration::from_millis(250)) {
            Ok(line) if line.trim_start().starts_with("password: ") => {
                saw_hidden_password = line.contains("hidden (--hidden-pass enabled)");
            }
            Ok(line) if line.trim_start().starts_with("file: ") => {
                saw_credentials_file = true;
            }
            Ok(line) if line.contains("git clone ") && !line.contains('@') => {
                saw_uncredentialed_clone = true;
            }
            Ok(line) if line.contains("===========================================================") => {
                saw_old_banner = true;
            }
            Ok(line)
                if line.contains("qit: starting...")
                    || line.contains("sidecar:")
                    || line.contains("snapshot:")
                    || line.contains("local Git HTTP:") =>
            {
                saw_internal_startup_noise = true;
            }
            Ok(line) if line.contains("Ctrl+C to stop.") => break,
            Ok(_) => continue,
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => continue,
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                bail!("qit server exited before printing banner")
            }
        }
    }

    let _ = child.kill();
    let _ = child.wait();
    assert!(saw_hidden_password);
    assert!(saw_credentials_file);
    assert!(saw_uncredentialed_clone);
    assert!(!saw_old_banner);
    assert!(!saw_internal_startup_noise);
    Ok(())
}

#[test]
fn qit_cli_shows_password_with_show_pass() -> Result<()> {
    let root = TempDir::new()?;
    let worktree = root.path().join("host");
    std::fs::create_dir_all(&worktree)?;
    std::fs::write(worktree.join("README.md"), "initial\n")?;

    let port = free_port()?;
    let (mut child, rx) =
        spawn_qit_serve_with_options(&worktree, port, false, None, true, Some("shared-session"))?;
    let deadline = Instant::now() + Duration::from_secs(20);
    let mut saw_password = false;
    let mut saw_credentialed_clone = false;
    while Instant::now() < deadline {
        match rx.recv_timeout(Duration::from_millis(250)) {
            Ok(line) if line.trim_start().starts_with("password: ") => {
                saw_password = !line.contains("hidden");
            }
            Ok(line) if line.contains("git clone ") && line.contains('@') => {
                saw_credentialed_clone = true;
            }
            Ok(line) if line.contains("Ctrl+C to stop.") => break,
            Ok(_) => continue,
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => continue,
            Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                bail!("qit server exited before printing banner")
            }
        }
    }

    let _ = child.kill();
    let _ = child.wait();
    assert!(saw_password);
    assert!(saw_credentialed_clone);
    Ok(())
}

#[test]
fn qit_cli_branch_commands_manage_sidecar_branches() -> Result<()> {
    let root = TempDir::new()?;
    let worktree = root.path().join("host");
    std::fs::create_dir_all(&worktree)?;
    std::fs::write(worktree.join("README.md"), "initial\n")?;

    let port = free_port()?;
    let (mut child, rx) = spawn_qit_serve(&worktree, port)?;
    let clone_url = wait_for_clone_url(&rx)?;

    let clone_dir = root.path().join("clone");
    run_git(None, &["clone", &clone_url, clone_dir.to_str().unwrap()])?;
    run_git(Some(&clone_dir), &["config", "user.name", "Qit Tester"])?;
    run_git(
        Some(&clone_dir),
        &["config", "user.email", "qit@example.com"],
    )?;

    run_qit(&["branch", worktree.to_str().unwrap(), "feature"])?;
    run_git(Some(&clone_dir), &["checkout", "-b", "feature"])?;
    std::fs::write(clone_dir.join("README.md"), "feature branch\n")?;
    run_git(Some(&clone_dir), &["commit", "-am", "feature readme"])?;
    run_git(Some(&clone_dir), &["push", "origin", "HEAD:feature"])?;

    assert_eq!(
        std::fs::read_to_string(worktree.join("README.md"))?.replace("\r\n", "\n"),
        "initial\n"
    );

    run_qit(&["checkout", worktree.to_str().unwrap(), "feature"])?;
    wait_for_file_contents(&worktree.join("README.md"), "feature branch\n")?;

    let clone_main_dir = root.path().join("clone-main");
    run_git(
        None,
        &["clone", &clone_url, clone_main_dir.to_str().unwrap()],
    )?;
    assert_eq!(
        std::fs::read_to_string(clone_main_dir.join("README.md"))?.replace("\r\n", "\n"),
        "initial\n"
    );

    let listed = run_qit(&["branch", worktree.to_str().unwrap()])?;
    assert!(listed.contains("* feature"));
    assert!(listed.contains(" main [served]"));

    run_qit(&["switch", worktree.to_str().unwrap(), "feature"])?;
    wait_for_file_contents(&worktree.join("README.md"), "feature branch\n")?;

    let listed = run_qit(&["branch", worktree.to_str().unwrap()])?;
    assert!(listed.contains("* feature"));
    assert!(listed.contains("[served]"));

    run_qit(&[
        "branch",
        worktree.to_str().unwrap(),
        "-m",
        "feature",
        "renamed",
    ])?;
    let listed = run_qit(&["branch", worktree.to_str().unwrap()])?;
    assert!(listed.contains("* renamed"));
    assert!(!listed.contains("feature"));

    run_qit(&["branch", worktree.to_str().unwrap(), "temp"])?;
    run_qit(&["branch", worktree.to_str().unwrap(), "-d", "temp"])?;
    let listed = run_qit(&["branch", worktree.to_str().unwrap()])?;
    assert!(!listed.contains("temp"));

    let delete_current = run_qit_output(&["branch", worktree.to_str().unwrap(), "-D", "renamed"])?;
    assert!(!delete_current.status.success());

    let _ = child.kill();
    let _ = child.wait();
    Ok(())
}

#[test]
fn qit_cli_git_parity_options_cover_branch_and_checkout() -> Result<()> {
    let root = TempDir::new()?;
    let worktree = root.path().join("host");
    std::fs::create_dir_all(&worktree)?;
    std::fs::write(worktree.join("README.md"), "initial\n")?;

    let port = free_port()?;
    let (mut child, rx) = spawn_qit_serve(&worktree, port)?;
    let clone_url = wait_for_clone_url(&rx)?;

    let clone_dir = root.path().join("clone");
    run_git(None, &["clone", &clone_url, clone_dir.to_str().unwrap()])?;
    run_git(Some(&clone_dir), &["config", "user.name", "Qit Tester"])?;
    run_git(
        Some(&clone_dir),
        &["config", "user.email", "qit@example.com"],
    )?;

    run_qit(&["branch", worktree.to_str().unwrap(), "feature", "main"])?;
    run_git(Some(&clone_dir), &["checkout", "-b", "feature"])?;
    std::fs::write(clone_dir.join("README.md"), "feature branch\n")?;
    run_git(Some(&clone_dir), &["commit", "-am", "feature readme"])?;
    run_git(Some(&clone_dir), &["push", "origin", "HEAD:feature"])?;

    let verbose = run_qit(&["branch", worktree.to_str().unwrap(), "--list", "fea*", "-v"])?;
    assert!(verbose.contains("feature"));
    assert!(verbose.contains("feature readme"));
    assert!(!verbose.contains(" main "));

    run_qit(&[
        "branch",
        worktree.to_str().unwrap(),
        "-c",
        "feature",
        "feature-copy",
    ])?;
    let listed = run_qit(&["branch", worktree.to_str().unwrap(), "--list", "feature*"])?;
    assert!(listed.contains("feature-copy"));

    run_qit(&["branch", worktree.to_str().unwrap(), "rename-src"])?;
    run_qit(&["branch", worktree.to_str().unwrap(), "rename-dst"])?;
    run_qit(&[
        "branch",
        worktree.to_str().unwrap(),
        "-M",
        "rename-src",
        "rename-dst",
    ])?;
    let listed = run_qit(&["branch", worktree.to_str().unwrap(), "--list", "rename*"])?;
    assert!(listed.contains("rename-dst"));
    assert!(!listed.contains("rename-src"));

    run_qit(&[
        "checkout",
        worktree.to_str().unwrap(),
        "-b",
        "scratch",
        "--track",
        "main",
    ])?;
    wait_for_file_contents(&worktree.join("README.md"), "initial\n")?;

    std::fs::write(worktree.join("README.md"), "dirty\n")?;
    run_qit(&["checkout", worktree.to_str().unwrap(), "-f", "feature"])?;
    wait_for_file_contents(&worktree.join("README.md"), "feature branch\n")?;

    let detach = run_qit_output(&["checkout", worktree.to_str().unwrap(), "--detach", "HEAD~1"])?;
    assert!(!detach.status.success());
    assert!(String::from_utf8_lossy(&detach.stderr).contains("detached checkout"));

    let merge = run_qit_output(&["checkout", worktree.to_str().unwrap(), "-m", "feature"])?;
    assert!(!merge.status.success());
    assert!(String::from_utf8_lossy(&merge.stderr).contains("merge-style checkout"));

    let path_checkout = run_qit_output(&[
        "checkout",
        worktree.to_str().unwrap(),
        "feature",
        "--",
        "README.md",
    ])?;
    assert!(!path_checkout.status.success());
    assert!(String::from_utf8_lossy(&path_checkout.stderr).contains("path checkout"));

    let _ = child.kill();
    let _ = child.wait();
    Ok(())
}

#[test]
fn qit_cli_pr_commands_cover_core_workflow() -> Result<()> {
    let root = TempDir::new()?;
    let data_dir = root.path().join("qit-data");
    let worktree = root.path().join("host");
    std::fs::create_dir_all(&worktree)?;
    std::fs::write(worktree.join("README.md"), "initial\n")?;

    let port = free_port()?;
    let (mut child, rx) = spawn_qit_serve_with_env(&worktree, port, false, Some(&data_dir))?;
    let clone_url = wait_for_clone_url(&rx)?;

    let clone_dir = root.path().join("clone");
    run_git(None, &["clone", &clone_url, clone_dir.to_str().unwrap()])?;
    run_git(Some(&clone_dir), &["config", "user.name", "Qit Tester"])?;
    run_git(
        Some(&clone_dir),
        &["config", "user.email", "qit@example.com"],
    )?;

    run_git(Some(&clone_dir), &["checkout", "-b", "feature"])?;
    std::fs::write(clone_dir.join("README.md"), "feature branch\n")?;
    run_git(Some(&clone_dir), &["commit", "-am", "feature readme"])?;
    run_git(Some(&clone_dir), &["push", "origin", "HEAD:feature"])?;

    run_qit_with_env(
        &["checkout", worktree.to_str().unwrap(), "feature"],
        Some(&data_dir),
    )?;

    let status = run_qit_with_env(&["pr", "status", worktree.to_str().unwrap()], Some(&data_dir))?;
    assert!(status.contains("current branch: feature"));

    let created = run_qit_with_env(
        &[
            "pr",
            "create",
            worktree.to_str().unwrap(),
            "--title",
            "Feature PR",
            "--body",
            "Adds the feature branch",
            "--head",
            "feature",
            "--base",
            "main",
        ],
        Some(&data_dir),
    )?;
    assert!(created.contains("created pull request"));

    let listed = run_qit_with_env(
        &["pr", "list", worktree.to_str().unwrap(), "--state", "open"],
        Some(&data_dir),
    )?;
    assert!(listed.contains("Feature PR"));
    let selector = first_pr_selector(&listed)?;

    let diff = run_qit_with_env(
        &["pr", "diff", worktree.to_str().unwrap(), &selector],
        Some(&data_dir),
    )?;
    assert!(diff.contains("--- a/README.md"));
    assert!(diff.contains("+++ b/README.md"));

    run_qit_with_env(
        &[
            "pr",
            "comment",
            worktree.to_str().unwrap(),
            &selector,
            "--body",
            "Please double check the copy.",
            "--author",
            "Alice",
        ],
        Some(&data_dir),
    )?;
    run_qit_with_env(
        &[
            "pr",
            "review",
            worktree.to_str().unwrap(),
            &selector,
            "--approve",
            "--body",
            "Looks good to me.",
            "--author",
            "Alice",
        ],
        Some(&data_dir),
    )?;
    run_qit_with_env(
        &[
            "pr",
            "edit",
            worktree.to_str().unwrap(),
            &selector,
            "--title",
            "Updated Feature PR",
            "--body",
            "Updated body",
            "--close",
        ],
        Some(&data_dir),
    )?;

    let viewed = run_qit_with_env(
        &["pr", "view", worktree.to_str().unwrap(), &selector],
        Some(&data_dir),
    )?;
    assert!(viewed.contains("Updated Feature PR"));
    assert!(viewed.contains("Alice"));
    assert!(viewed.contains("approved"));
    assert!(viewed.contains("closed the pull request"));

    run_qit_with_env(
        &["pr", "reopen", worktree.to_str().unwrap(), &selector],
        Some(&data_dir),
    )?;
    run_qit_with_env(
        &["pr", "merge", worktree.to_str().unwrap(), &selector],
        Some(&data_dir),
    )?;

    let merged = run_qit_with_env(
        &["pr", "list", worktree.to_str().unwrap(), "--state", "merged"],
        Some(&data_dir),
    )?;
    assert!(merged.contains("Updated Feature PR"));

    run_git(Some(&clone_dir), &["checkout", "main"])?;
    run_git(Some(&clone_dir), &["checkout", "-b", "cleanup"])?;
    std::fs::write(clone_dir.join("cleanup.txt"), "cleanup\n")?;
    run_git(Some(&clone_dir), &["add", "cleanup.txt"])?;
    run_git(Some(&clone_dir), &["commit", "-m", "cleanup change"])?;
    run_git(Some(&clone_dir), &["push", "origin", "HEAD:cleanup"])?;

    run_qit_with_env(
        &[
            "pr",
            "create",
            worktree.to_str().unwrap(),
            "--title",
            "Cleanup PR",
            "--body",
            "Temporary cleanup",
            "--head",
            "cleanup",
            "--base",
            "main",
        ],
        Some(&data_dir),
    )?;
    let listed = run_qit_with_env(
        &["pr", "list", worktree.to_str().unwrap(), "--state", "open"],
        Some(&data_dir),
    )?;
    assert!(listed.contains("Cleanup PR"));
    let cleanup_selector = listed
        .lines()
        .find(|line| line.contains("Cleanup PR"))
        .and_then(|line| line.split_whitespace().next())
        .map(ToString::to_string)
        .context("parse cleanup selector")?;
    run_qit_with_env(
        &["pr", "delete", worktree.to_str().unwrap(), &cleanup_selector],
        Some(&data_dir),
    )?;
    let listed = run_qit_with_env(
        &["pr", "list", worktree.to_str().unwrap(), "--state", "all"],
        Some(&data_dir),
    )?;
    assert!(!listed.contains("Cleanup PR"));

    let _ = child.kill();
    let _ = child.wait();
    Ok(())
}

#[test]
fn auto_apply_skips_when_checked_out_branch_differs_from_served_branch() -> Result<()> {
    let root = TempDir::new()?;
    let data_dir = root.path().join("qit-data");
    let worktree = root.path().join("host");
    std::fs::create_dir_all(&worktree)?;
    std::fs::write(worktree.join("README.md"), "initial\n")?;

    let port = free_port()?;
    let (mut child, rx) = spawn_qit_serve_with_env(&worktree, port, true, Some(&data_dir))?;
    let clone_url = wait_for_clone_url(&rx)?;

    let clone_dir = root.path().join("clone");
    run_git(None, &["clone", &clone_url, clone_dir.to_str().unwrap()])?;
    run_git(Some(&clone_dir), &["config", "user.name", "Qit Tester"])?;
    run_git(
        Some(&clone_dir),
        &["config", "user.email", "qit@example.com"],
    )?;

    run_qit_with_env(
        &["branch", worktree.to_str().unwrap(), "feature"],
        Some(&data_dir),
    )?;
    run_git(Some(&clone_dir), &["checkout", "-b", "feature"])?;
    std::fs::write(clone_dir.join("README.md"), "feature branch\n")?;
    run_git(Some(&clone_dir), &["commit", "-am", "feature readme"])?;
    run_git(Some(&clone_dir), &["push", "origin", "HEAD:feature"])?;

    run_qit_with_env(
        &["checkout", worktree.to_str().unwrap(), "feature"],
        Some(&data_dir),
    )?;
    wait_for_file_contents(&worktree.join("README.md"), "feature branch\n")?;

    run_git(Some(&clone_dir), &["checkout", "main"])?;
    std::fs::write(clone_dir.join("README.md"), "main update\n")?;
    run_git(Some(&clone_dir), &["commit", "-am", "main update"])?;
    run_git(Some(&clone_dir), &["push", "origin", "HEAD:main"])?;

    std::thread::sleep(Duration::from_secs(2));
    assert_eq!(
        std::fs::read_to_string(worktree.join("README.md"))?.replace("\r\n", "\n"),
        "feature branch\n"
    );

    let _ = child.kill();
    let _ = child.wait();
    Ok(())
}

#[test]
fn settings_commands_manage_metadata_and_branch_rules() -> Result<()> {
    let root = TempDir::new()?;
    let data_dir = root.path().join("qit-data");
    let worktree = root.path().join("host");
    std::fs::create_dir_all(&worktree)?;
    std::fs::write(worktree.join("README.md"), "initial\n")?;

    let port = free_port()?;
    let (mut child, _rx) = spawn_qit_serve_with_env(&worktree, port, false, Some(&data_dir))?;
    std::thread::sleep(Duration::from_secs(2));

    let updated = run_qit_with_env(
        &[
            "settings",
            "set",
            worktree.to_str().unwrap(),
            "--description",
            "Demo repo",
            "--homepage",
            "https://example.com/docs",
        ],
        Some(&data_dir),
    )?;
    assert!(updated.contains("Demo repo"));
    assert!(updated.contains("https://example.com/docs"));

    let rules = run_qit_with_env(
        &[
            "settings",
            "rule",
            worktree.to_str().unwrap(),
            "--pattern",
            "main",
            "--require-pr",
            "--approvals",
            "1",
            "--dismiss-stale",
            "--block-force-push",
            "--block-delete",
        ],
        Some(&data_dir),
    )?;
    assert!(rules.contains("main: require PR"));
    assert!(rules.contains("1 approval(s)"));
    assert!(rules.contains("block force-push"));
    assert!(rules.contains("block delete"));

    let listed = run_qit_with_env(
        &["settings", "view", worktree.to_str().unwrap()],
        Some(&data_dir),
    )?;
    assert!(listed.contains("description: Demo repo"));
    assert!(listed.contains("homepage: https://example.com/docs"));
    assert!(listed.contains("main: require PR"));

    let deleted = run_qit_with_env(
        &[
            "settings",
            "rule",
            worktree.to_str().unwrap(),
            "--delete",
            "main",
        ],
        Some(&data_dir),
    )?;
    assert!(deleted.contains("deleted branch rule `main`"));
    assert!(deleted.contains("branch rules:"));

    let _ = child.kill();
    let _ = child.wait();
    Ok(())
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn request_based_auth_cli_and_git_flow_works_end_to_end() -> Result<()> {
    let root = TempDir::new()?;
    let data_dir = root.path().join("qit-data");
    let worktree = root.path().join("host");
    std::fs::create_dir_all(&worktree)?;
    std::fs::write(worktree.join("README.md"), "initial\n")?;

    let port = free_port()?;
    let (mut child, rx) =
        spawn_qit_serve_with_auth_mode(&worktree, port, Some(&data_dir), "request-based")?;
    let legacy_clone_url = wait_for_clone_url(&rx)?;
    assert!(!legacy_clone_url.contains('@'));

    let client = reqwest::Client::new();
    let request_url = format!("http://127.0.0.1:{port}/host/api/access-requests");
    let request_response = client
        .post(&request_url)
        .header("content-type", "application/json")
        .body(r#"{"name":"Alice","email":"alice@example.com"}"#)
        .send()
        .await?;
    assert!(request_response.status().is_success());

    let listed = run_qit_with_env(
        &["auth", "requests", worktree.to_str().unwrap()],
        Some(&data_dir),
    )?;
    let selector = listed
        .lines()
        .find_map(|line| {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed == "none" || trimmed.starts_with("access requests for ") {
                return None;
            }
            trimmed.split_whitespace().next().map(ToString::to_string)
        })
        .context("parse request selector")?;

    let approved = run_qit_with_env(
        &[
            "auth",
            "requests",
            worktree.to_str().unwrap(),
            "--approve",
            &selector,
        ],
        Some(&data_dir),
    )?;
    let onboarding = approved
        .lines()
        .find_map(|line| line.contains("qit_setup.").then_some(line.trim().to_string()))
        .context("parse onboarding token")?;

    let onboarding_response = client
        .post(format!("http://127.0.0.1:{port}/host/api/onboarding/complete"))
        .header("content-type", "application/json")
        .body(format!(
            r#"{{"token":"{}","username":"alice","password":"very-secret-pass"}}"#,
            onboarding
        ))
        .send()
        .await?;
    assert!(onboarding_response.status().is_success());

    let legacy = client.get(format!("http://127.0.0.1:{port}/host/info/refs?service=git-upload-pack"));
    let legacy_url = Url::parse(&legacy_clone_url)?;
    let legacy_response = legacy
        .basic_auth(legacy_url.username(), legacy_url.password())
        .send()
        .await?;
    assert_eq!(legacy_response.status(), reqwest::StatusCode::UNAUTHORIZED);

    let request_based_response = client
        .get(format!(
            "http://127.0.0.1:{port}/host/info/refs?service=git-upload-pack"
        ))
        .basic_auth("alice", Some("very-secret-pass"))
        .send()
        .await?;
    assert!(request_based_response.status().is_success());

    let promoted = run_qit_with_env(
        &[
            "auth",
            "users",
            worktree.to_str().unwrap(),
            "--promote",
            "alice",
        ],
        Some(&data_dir),
    );
    assert!(promoted.is_err());

    let users = run_qit_with_env(
        &["auth", "users", worktree.to_str().unwrap()],
        Some(&data_dir),
    )?;
    let user_selector = users
        .lines()
        .find_map(|line| {
            let trimmed = line.trim();
            trimmed
                .contains("alice@example.com")
                .then(|| trimmed.split_whitespace().next().unwrap_or_default().to_string())
        })
        .context("parse user selector")?;
    let promoted = run_qit_with_env(
        &[
            "auth",
            "users",
            worktree.to_str().unwrap(),
            "--promote",
            &user_selector,
        ],
        Some(&data_dir),
    )?;
    assert!(promoted.contains("promoted"));

    let _ = child.kill();
    let _ = child.wait();
    Ok(())
}
