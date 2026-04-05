use anyhow::{anyhow, Context, Result};
use qit_domain::SessionCredentials;
use qit_transports::PublicTransport;
use std::io::Write;
use std::path::{Path, PathBuf};
use url::Url;

pub fn say(message: &str) {
    println!("{message}");
    let _ = std::io::stdout().flush();
}

pub fn repo_name_from_worktree(worktree: &Path) -> String {
    worktree
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty())
        .unwrap_or("repo")
        .to_string()
}

pub fn repo_url(base: &Url, repo_mount_path: &str) -> Result<Url> {
    let mut url = base.clone();
    url.set_path(repo_mount_path);
    Ok(url)
}

pub fn clone_command(public_url: &Url, transport: PublicTransport) -> String {
    let repo_url = public_url.as_str().trim_end_matches('/').to_string();
    match transport {
        PublicTransport::Ngrok => {
            format!("git -c http.extraHeader=\"ngrok-skip-browser-warning: 1\" clone {repo_url}/")
        }
        _ => format!("git clone {repo_url}/"),
    }
}

pub fn repo_url_with_credentials(
    public_url: &Url,
    credentials: &SessionCredentials,
    reveal_password: bool,
) -> Result<Url> {
    let mut url = public_url.clone();
    if !reveal_password {
        return Ok(url);
    }
    url.set_username(&credentials.username)
        .map_err(|_| anyhow!("failed to encode username in clone URL"))?;
    url.set_password(Some(&credentials.password))
        .map_err(|_| anyhow!("failed to encode password in clone URL"))?;
    Ok(url)
}

pub fn write_credentials_file(
    credentials: &SessionCredentials,
    persist_to_disk: bool,
) -> Result<Option<PathBuf>> {
    if !persist_to_disk {
        return Ok(None);
    }
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
    #[cfg(unix)]
    {
        use std::fs;
        use std::os::unix::fs::PermissionsExt;

        fs::set_permissions(&path, fs::Permissions::from_mode(0o600))
            .context("restrict credentials file permissions")?;
    }
    Ok(Some(path))
}

pub fn print_serve_summary(
    worktree: &Path,
    exported_branch: &str,
    label: &str,
    public_url: &Url,
    local_browser_url: &Url,
    browser_url: &Url,
    credentials: &SessionCredentials,
    credentials_path: Option<&Path>,
    reveal_password: bool,
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
    if reveal_password {
        println!("  password: {}", credentials.password);
    } else {
        println!("  password: hidden (--hidden-pass enabled)");
    }
    if let Some(credentials_path) = credentials_path {
        println!("  file: {}", credentials_path.display());
    }
}
