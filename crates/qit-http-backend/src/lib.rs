use async_trait::async_trait;
use std::path::Path;
use thiserror::Error;
use tokio::io::AsyncRead;

pub type BoxAsyncRead = Box<dyn AsyncRead + Unpin + Send>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GitHttpBackendRequest {
    pub method: String,
    pub path_info: String,
    pub query: Option<String>,
    pub headers: Vec<(String, String)>,
    pub content_length: Option<u64>,
    pub allow_push: bool,
    pub request_scheme: String,
}

pub struct GitHttpBackendResponse {
    pub status: u16,
    pub headers: Vec<(String, String)>,
    pub body_prefix: Vec<u8>,
    pub stdout: Option<tokio::process::ChildStdout>,
    pub completion: Option<tokio::task::JoinHandle<Result<(), GitHttpBackendError>>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum GitHttpBackendError {
    #[error("io failed during {operation}: {message}")]
    Io {
        operation: &'static str,
        message: String,
    },
    #[error("git binary not found; install Git and ensure it is on PATH")]
    GitNotFound,
    #[error("invalid git http-backend response")]
    InvalidResponse,
    #[error("git http-backend exited with status {0}")]
    ProcessStatus(String),
}

#[async_trait]
pub trait GitHttpBackend: Send + Sync {
    async fn serve(
        &self,
        sidecar: &Path,
        request: GitHttpBackendRequest,
        body: BoxAsyncRead,
    ) -> Result<GitHttpBackendResponse, GitHttpBackendError>;
}
