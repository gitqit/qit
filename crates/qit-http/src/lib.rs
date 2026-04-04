use axum::body::{Body, Bytes};
use axum::extract::Request;
use axum::http::header::CONTENT_LENGTH;
use axum::http::header::WWW_AUTHENTICATE;
use axum::http::{HeaderMap, HeaderName, HeaderValue, Response, StatusCode};
use axum::routing::any;
use axum::{extract::DefaultBodyLimit, Router};
use base64::engine::general_purpose::STANDARD as BASE64;
use base64::Engine;
use qit_domain::{RegistryStore, SessionCredentials, WorkspaceService, WorkspaceSpec};
use qit_http_backend::{
    GitHttpBackend, GitHttpBackendError, GitHttpBackendRequest, GitHttpBackendResponse,
};
use std::io;
use std::sync::Arc;
use tokio_stream::StreamExt;
use tokio_util::io::{ReaderStream, StreamReader};
use tracing::{info, warn};

pub const DEFAULT_MAX_BODY_BYTES: usize = 512 * 1024 * 1024;

pub struct GitHttpServerConfig {
    pub workspace: WorkspaceSpec,
    pub credentials: SessionCredentials,
    pub auto_apply: bool,
    pub repo_mount_path: String,
    pub request_scheme: String,
    pub max_body_bytes: usize,
}

#[derive(Clone)]
pub struct GitHttpServer {
    git_http_backend: Arc<dyn GitHttpBackend>,
    registry_store: Arc<dyn RegistryStore>,
    workspace_service: Arc<WorkspaceService>,
    workspace: WorkspaceSpec,
    credentials: SessionCredentials,
    auto_apply: bool,
    repo_mount_path: String,
    request_scheme: String,
    max_body_bytes: usize,
}

impl GitHttpServer {
    pub fn new(
        git_http_backend: Arc<dyn GitHttpBackend>,
        registry_store: Arc<dyn RegistryStore>,
        workspace_service: Arc<WorkspaceService>,
        config: GitHttpServerConfig,
    ) -> Self {
        Self {
            git_http_backend,
            registry_store,
            workspace_service,
            workspace: config.workspace,
            credentials: config.credentials,
            auto_apply: config.auto_apply,
            repo_mount_path: config.repo_mount_path,
            request_scheme: config.request_scheme,
            max_body_bytes: config.max_body_bytes,
        }
    }

    fn latest_workspace(&self) -> Result<WorkspaceSpec, qit_domain::RegistryError> {
        if let Some(record) = self.registry_store.load(self.workspace.id)? {
            return Ok(WorkspaceSpec {
                id: self.workspace.id,
                worktree: record.worktree.clone(),
                sidecar: record.sidecar,
                exported_branch: record.exported_branch.clone(),
                checked_out_branch: record.checked_out_branch.unwrap_or(record.exported_branch),
            });
        }
        Ok(self.workspace.clone())
    }

    pub fn router(self) -> Router {
        let max_body_bytes = self.max_body_bytes;
        let state = Arc::new(self);
        Router::new()
            .fallback(any(move |req: Request| {
                let state = state.clone();
                async move { state.handle(req).await }
            }))
            .layer(DefaultBodyLimit::max(max_body_bytes))
    }

    fn spawn_auto_apply(
        self: Arc<Self>,
        method: String,
        path: String,
        completion: tokio::task::JoinHandle<Result<(), GitHttpBackendError>>,
    ) {
        tokio::spawn(async move {
            match completion.await {
                Ok(Ok(())) => {}
                Ok(Err(error)) => {
                    warn!(
                        method = %method,
                        path = %path,
                        worktree = %self.workspace.worktree.display(),
                        %error,
                        "push completed with backend error; auto-apply skipped"
                    );
                    return;
                }
                Err(error) => {
                    warn!(
                        method = %method,
                        path = %path,
                        worktree = %self.workspace.worktree.display(),
                        %error,
                        "push completion task failed; auto-apply skipped"
                    );
                    return;
                }
            }

            let workspace = match self.latest_workspace() {
                Ok(workspace) => workspace,
                Err(error) => {
                    warn!(
                        method = %method,
                        path = %path,
                        worktree = %self.workspace.worktree.display(),
                        %error,
                        "push succeeded but registry reload failed before auto-apply"
                    );
                    return;
                }
            };
            if workspace.checked_out_branch != workspace.exported_branch {
                warn!(
                    method = %method,
                    path = %path,
                    worktree = %workspace.worktree.display(),
                    checked_out_branch = %workspace.checked_out_branch,
                    exported_branch = %workspace.exported_branch,
                    "push succeeded but auto-apply was skipped because the host folder is checked out to a different branch"
                );
                return;
            }

            match self
                .workspace_service
                .apply(workspace.worktree.clone(), &workspace.exported_branch, None)
                .await
            {
                Ok((_workspace, outcome)) => info!(
                    method = %method,
                    path = %path,
                    worktree = %workspace.worktree.display(),
                    branch = %outcome.merged_to,
                    commit = %outcome.commit,
                    "auto-applied pushed commit to host worktree"
                ),
                Err(error) => warn!(
                    method = %method,
                    path = %path,
                    worktree = %workspace.worktree.display(),
                    %error,
                    "push succeeded but auto-apply failed"
                ),
            }
        });
    }

    async fn handle(self: Arc<Self>, req: Request) -> Result<Response<Body>, StatusCode> {
        if !authorize(req.headers(), &self.credentials) {
            return Ok(unauthorized_response());
        }

        let method = req.method().clone();
        let uri = req.uri().clone();
        let workspace = self.latest_workspace().map_err(|error| {
            warn!(
                method = %method,
                path = %uri.path(),
                worktree = %self.workspace.worktree.display(),
                %error,
                "failed to reload workspace state before handling git request"
            );
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
        let path_info = match strip_repo_mount(uri.path(), &self.repo_mount_path) {
            Some(path_info) => path_info,
            None => return Err(StatusCode::NOT_FOUND),
        };
        let query = uri.query().map(ToString::to_string);
        let is_receive_pack =
            method == axum::http::Method::POST && path_info.ends_with("git-receive-pack");
        let request_scheme = request_scheme(&self.request_scheme);
        let content_length = req
            .headers()
            .get(CONTENT_LENGTH)
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.parse::<u64>().ok());

        let mut headers = Vec::new();
        for (key, value) in req.headers() {
            let lower = key.as_str().to_ascii_lowercase();
            if matches!(
                lower.as_str(),
                "authorization" | "connection" | "transfer-encoding" | "te" | "trailer"
            ) {
                continue;
            }
            if let Ok(value) = value.to_str() {
                headers.push((key.as_str().to_string(), value.to_string()));
            }
        }

        let body = req.into_body().into_data_stream().map(|result| {
            result.map_err(|error| io::Error::new(io::ErrorKind::InvalidData, error.to_string()))
        });
        let body_reader = Box::new(StreamReader::new(body));

        let request = GitHttpBackendRequest {
            method: method.to_string(),
            path_info,
            query,
            headers,
            content_length,
            allow_push: true,
            request_scheme,
        };

        let mut response = self
            .git_http_backend
            .serve(&workspace.sidecar, request, body_reader)
            .await
            .map_err(|error| {
                warn!(
                    method = %method,
                    path = %uri.path(),
                    worktree = %workspace.worktree.display(),
                    %error,
                    "git http-backend request failed"
                );
                StatusCode::INTERNAL_SERVER_ERROR
            })?;

        if is_receive_pack && self.auto_apply {
            if let Some(completion) = response.completion.take() {
                self.clone().spawn_auto_apply(
                    method.to_string(),
                    uri.path().to_string(),
                    completion,
                );
            }
        }

        Ok(build_response(response))
    }
}

pub fn authorize(headers: &HeaderMap, credentials: &SessionCredentials) -> bool {
    let Some(header) = headers.get(axum::http::header::AUTHORIZATION) else {
        return false;
    };
    let Ok(header) = header.to_str() else {
        return false;
    };
    let Some(encoded) = header.strip_prefix("Basic ") else {
        return false;
    };
    let Ok(decoded) = BASE64.decode(encoded) else {
        return false;
    };
    let Ok(decoded) = String::from_utf8(decoded) else {
        return false;
    };
    let Some((username, password)) = decoded.split_once(':') else {
        return false;
    };
    username == credentials.username && password == credentials.password
}

pub fn request_scheme(configured_scheme: &str) -> String {
    if configured_scheme == "https" {
        "https".to_string()
    } else {
        "http".to_string()
    }
}

fn build_response(streaming_response: GitHttpBackendResponse) -> Response<Body> {
    let mut builder = Response::builder()
        .status(StatusCode::from_u16(streaming_response.status).unwrap_or(StatusCode::OK));
    for (key, value) in streaming_response.headers {
        let Ok(name) = HeaderName::from_bytes(key.as_bytes()) else {
            continue;
        };
        let Ok(value) = HeaderValue::from_str(&value) else {
            continue;
        };
        builder = builder.header(name, value);
    }
    let body = match streaming_response.stdout {
        Some(stdout) if streaming_response.body_prefix.is_empty() => {
            Body::from_stream(ReaderStream::new(stdout))
        }
        Some(stdout) => Body::from_stream(
            tokio_stream::iter(vec![Ok::<Bytes, io::Error>(Bytes::from(
                streaming_response.body_prefix,
            ))])
            .chain(ReaderStream::new(stdout)),
        ),
        None => Body::from(streaming_response.body_prefix),
    };
    builder.body(body).unwrap_or_else(|_| {
        Response::builder()
            .status(StatusCode::INTERNAL_SERVER_ERROR)
            .body(Body::empty())
            .unwrap()
    })
}

pub fn repo_mount_path(repo_name: &str) -> String {
    format!("/{}", sanitize_repo_name(repo_name))
}

pub fn sanitize_repo_name(value: &str) -> String {
    let mut slug = String::with_capacity(value.len());
    let mut last_was_dash = false;
    for ch in value.chars() {
        let safe = ch.is_ascii_alphanumeric() || matches!(ch, '.' | '_' | '-');
        if safe {
            slug.push(ch);
            last_was_dash = false;
        } else if !last_was_dash {
            slug.push('-');
            last_was_dash = true;
        }
    }

    let slug = slug.trim_matches('-');
    if slug.is_empty() {
        "repo".to_string()
    } else {
        slug.to_string()
    }
}

pub fn strip_repo_mount(path: &str, repo_mount_path: &str) -> Option<String> {
    if path == repo_mount_path {
        return Some("/".to_string());
    }
    let prefix = format!("{repo_mount_path}/");
    path.strip_prefix(&prefix)
        .map(|suffix| format!("/{}", suffix))
}

fn unauthorized_response() -> Response<Body> {
    Response::builder()
        .status(StatusCode::UNAUTHORIZED)
        .header(WWW_AUTHENTICATE, r#"Basic realm="qit""#)
        .body(Body::from("authentication required"))
        .unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn credentials() -> SessionCredentials {
        SessionCredentials {
            username: "user".into(),
            password: "pass".into(),
        }
    }

    #[test]
    fn authorize_accepts_matching_basic_auth() {
        let token = BASE64.encode("user:pass");
        let mut headers = HeaderMap::new();
        headers.insert(
            axum::http::header::AUTHORIZATION,
            HeaderValue::from_str(&format!("Basic {token}")).unwrap(),
        );
        assert!(authorize(&headers, &credentials()));
    }

    #[test]
    fn authorize_rejects_missing_and_wrong_credentials() {
        let headers = HeaderMap::new();
        assert!(!authorize(&headers, &credentials()));

        let token = BASE64.encode("user:nope");
        let mut headers = HeaderMap::new();
        headers.insert(
            axum::http::header::AUTHORIZATION,
            HeaderValue::from_str(&format!("Basic {token}")).unwrap(),
        );
        assert!(!authorize(&headers, &credentials()));
    }

    #[test]
    fn request_scheme_uses_configured_transport_scheme() {
        assert_eq!(request_scheme("https"), "https");
        assert_eq!(request_scheme("http"), "http");
    }

    #[test]
    fn repo_mount_is_slugged_and_stripped() {
        assert_eq!(repo_mount_path("My Project"), "/My-Project");
        assert_eq!(
            strip_repo_mount("/My-Project/info/refs", "/My-Project"),
            Some("/info/refs".into())
        );
        assert_eq!(
            strip_repo_mount("/My-Projectish/info/refs", "/My-Project"),
            None
        );
        assert_eq!(strip_repo_mount("/other/info/refs", "/My-Project"), None);
    }
}
