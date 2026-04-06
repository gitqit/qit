use anyhow::{anyhow, bail, Context, Result};
use axum::body::Body;
use axum::extract::{Request, State};
use axum::http::header::HOST;
use axum::http::{Response, StatusCode};
use axum::response::Html;
use axum::routing::{any, get, post};
use axum::{Json, Router};
use qit_domain::WorkspaceId;
use qit_http::repo_mount_path;
use qit_transports::{expose, PublicTransport};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tokio::net::TcpListener;
use tokio::sync::{watch, RwLock};
use url::Url;

const DISCOVERY_FILE: &str = "supervisor.json";
const ASSIGNMENTS_FILE: &str = "supervisor-routes.json";
const CONTROL_PREFIX: &str = "/__qit/supervisor";
const ROUTE_LEASE_MS: u64 = 15_000;
const HEARTBEAT_INTERVAL_MS: u64 = 5_000;
const CLEANUP_INTERVAL_MS: u64 = 2_000;
const IDLE_SHUTDOWN_MS: u64 = 30_000;

#[derive(Clone)]
pub struct SharedEntrypoint {
    pub label: String,
    pub local_base_url: Url,
    pub public_base_url: Url,
    pub control_url: Url,
}

#[derive(Clone)]
pub struct RouteLease {
    pub workspace_id: WorkspaceId,
    pub mount_path: String,
}

#[derive(Clone, Serialize, Deserialize)]
struct SupervisorDiscovery {
    label: String,
    local_base_url: String,
    public_base_url: String,
    control_url: String,
    pid: u32,
}

#[derive(Clone, Serialize, Deserialize, Default)]
struct PersistedAssignments {
    assignments: Vec<PersistedAssignment>,
}

#[derive(Clone, Serialize, Deserialize)]
struct PersistedAssignment {
    workspace_id: String,
    mount_path: String,
}

#[derive(Clone)]
struct ActiveRoute {
    mount_path: String,
    upstream_url: String,
    worktree: String,
    last_seen_ms: u64,
}

#[derive(Default)]
struct RouteRegistry {
    assignments: HashMap<String, String>,
    active_routes: HashMap<String, ActiveRoute>,
    idle_started_at_ms: Option<u64>,
}

#[derive(Clone)]
struct SupervisorState {
    data_root: PathBuf,
    discovery: SupervisorDiscovery,
    routes: Arc<RwLock<RouteRegistry>>,
    client: reqwest::Client,
}

#[derive(Deserialize)]
struct ClaimRouteRequest {
    workspace_id: String,
    preferred_repo_name: String,
}

#[derive(Serialize, Deserialize)]
struct ClaimRouteResponse {
    mount_path: String,
    label: String,
    local_base_url: String,
    public_base_url: String,
}

#[derive(Deserialize)]
struct RegisterRouteRequest {
    workspace_id: String,
    mount_path: String,
    upstream_url: String,
    worktree: String,
}

#[derive(Deserialize)]
struct HeartbeatRouteRequest {
    workspace_id: String,
}

#[derive(Deserialize)]
struct UnregisterRouteRequest {
    workspace_id: String,
}

#[derive(Serialize)]
struct HealthResponse {
    ok: bool,
}

pub fn heartbeat_interval() -> Duration {
    Duration::from_millis(HEARTBEAT_INTERVAL_MS)
}

pub async fn ensure_supervisor(
    current_exe: &Path,
    data_root: &Path,
    port: u16,
    transport: PublicTransport,
) -> Result<SharedEntrypoint> {
    if let Some(existing) = load_healthy_discovery(data_root).await? {
        return Ok(existing);
    }

    let mut command = std::process::Command::new(current_exe);
    command
        .arg("internal-supervisor")
        .arg("--port")
        .arg(port.to_string())
        .arg("--transport")
        .arg(match transport {
            PublicTransport::Ngrok => "ngrok",
            PublicTransport::Tailscale => "tailscale",
            PublicTransport::Lan => "lan",
            PublicTransport::Local => "local",
        })
        .stdout(Stdio::null())
        .stderr(Stdio::inherit());
    if let Some(root) = std::env::var_os("QIT_DATA_DIR") {
        command.env("QIT_DATA_DIR", root);
    }
    let _child = command.spawn().context("spawn qit supervisor")?;

    let deadline = tokio::time::Instant::now() + Duration::from_secs(15);
    while tokio::time::Instant::now() < deadline {
        if let Some(existing) = load_healthy_discovery(data_root).await? {
            return Ok(existing);
        }
        tokio::time::sleep(Duration::from_millis(200)).await;
    }

    bail!("timed out waiting for qit supervisor startup")
}

pub async fn claim_mount_path(
    entrypoint: &SharedEntrypoint,
    workspace_id: WorkspaceId,
    preferred_repo_name: &str,
) -> Result<RouteLease> {
    let client = reqwest::Client::new();
    let response = client
        .post(control_url(&entrypoint.control_url, "/claim-route")?)
        .json(&serde_json::json!({
            "workspace_id": workspace_id.0.to_string(),
            "preferred_repo_name": preferred_repo_name,
        }))
        .send()
        .await
        .context("claim route from qit supervisor")?;
    if !response.status().is_success() {
        bail!(
            "qit supervisor route claim failed with {}",
            response.status()
        );
    }
    let payload: ClaimRouteResponse = response
        .json()
        .await
        .context("decode qit supervisor route claim")?;
    Ok(RouteLease {
        workspace_id,
        mount_path: payload.mount_path,
    })
}

pub async fn register_route(
    entrypoint: &SharedEntrypoint,
    lease: &RouteLease,
    upstream_url: &Url,
    worktree: &Path,
) -> Result<()> {
    let client = reqwest::Client::new();
    let response = client
        .post(control_url(&entrypoint.control_url, "/register-route")?)
        .json(&serde_json::json!({
            "workspace_id": lease.workspace_id.0.to_string(),
            "mount_path": lease.mount_path,
            "upstream_url": upstream_url.as_str().trim_end_matches('/'),
            "worktree": worktree.display().to_string(),
        }))
        .send()
        .await
        .context("register route with qit supervisor")?;
    if !response.status().is_success() {
        bail!(
            "qit supervisor route registration failed with {}",
            response.status()
        );
    }
    Ok(())
}

pub async fn heartbeat_route(entrypoint: &SharedEntrypoint, lease: &RouteLease) -> Result<()> {
    let client = reqwest::Client::new();
    let response = client
        .post(control_url(&entrypoint.control_url, "/heartbeat-route")?)
        .json(&serde_json::json!({
            "workspace_id": lease.workspace_id.0.to_string(),
        }))
        .send()
        .await
        .context("heartbeat qit supervisor route")?;
    if !response.status().is_success() {
        bail!(
            "qit supervisor route heartbeat failed with {}",
            response.status()
        );
    }
    Ok(())
}

pub async fn unregister_route(entrypoint: &SharedEntrypoint, lease: &RouteLease) -> Result<()> {
    let client = reqwest::Client::new();
    let response = client
        .post(control_url(&entrypoint.control_url, "/unregister-route")?)
        .json(&serde_json::json!({
            "workspace_id": lease.workspace_id.0.to_string(),
        }))
        .send()
        .await
        .context("unregister route from qit supervisor")?;
    if !response.status().is_success() {
        bail!(
            "qit supervisor route unregister failed with {}",
            response.status()
        );
    }
    Ok(())
}

pub async fn run_internal_supervisor(
    data_root: PathBuf,
    port: u16,
    transport: PublicTransport,
) -> Result<()> {
    std::fs::create_dir_all(&data_root)
        .with_context(|| format!("create qit data directory {}", data_root.display()))?;

    let edge_bind_host = supervisor_edge_bind_host(transport);
    let edge_listener = TcpListener::bind((edge_bind_host, port))
        .await
        .with_context(|| format!("bind qit supervisor edge listener on {edge_bind_host}:{port}"))?;
    let edge_local_url =
        Url::parse(&format!("http://127.0.0.1:{port}/")).context("build supervisor edge URL")?;
    let endpoint = expose(transport, &edge_local_url).await?;

    let control_listener = TcpListener::bind(("127.0.0.1", 0))
        .await
        .context("bind qit supervisor control listener")?;
    let control_port = control_listener
        .local_addr()
        .context("read qit supervisor control addr")?
        .port();
    let control_url = Url::parse(&format!("http://127.0.0.1:{control_port}/"))
        .context("build supervisor control URL")?;

    let discovery = SupervisorDiscovery {
        label: endpoint.label.to_string(),
        local_base_url: edge_local_url.as_str().trim_end_matches('/').to_string(),
        public_base_url: endpoint
            .public_url
            .as_str()
            .trim_end_matches('/')
            .to_string(),
        control_url: control_url.as_str().trim_end_matches('/').to_string(),
        pid: std::process::id(),
    };
    let registry = load_assignments(&data_root)?;
    let state = SupervisorState {
        data_root: data_root.clone(),
        discovery: discovery.clone(),
        routes: Arc::new(RwLock::new(registry)),
        client: reqwest::Client::new(),
    };
    write_json_file(discovery_path(&data_root), &discovery)?;

    let (shutdown_tx, shutdown_rx) = watch::channel(false);
    let control_app = Router::new()
        .route(&format!("{CONTROL_PREFIX}/health"), get(health))
        .route(&format!("{CONTROL_PREFIX}/claim-route"), post(claim_route))
        .route(
            &format!("{CONTROL_PREFIX}/register-route"),
            post(register_route_handler),
        )
        .route(
            &format!("{CONTROL_PREFIX}/heartbeat-route"),
            post(heartbeat_route_handler),
        )
        .route(
            &format!("{CONTROL_PREFIX}/unregister-route"),
            post(unregister_route_handler),
        )
        .with_state(state.clone());
    let edge_app = Router::new()
        .route("/", get(index))
        .route("/{*path}", any(proxy_request))
        .with_state(state.clone());

    let control_shutdown = shutdown_rx.clone();
    let control_task = tokio::spawn(async move {
        axum::serve(control_listener, control_app)
            .with_graceful_shutdown(wait_for_shutdown(control_shutdown))
            .await
            .map_err(anyhow::Error::new)
    });
    let edge_shutdown = shutdown_rx.clone();
    let edge_task = tokio::spawn(async move {
        axum::serve(edge_listener, edge_app)
            .with_graceful_shutdown(wait_for_shutdown(edge_shutdown))
            .await
            .map_err(anyhow::Error::new)
    });

    let janitor_state = state.clone();
    let janitor_tx = shutdown_tx.clone();
    let janitor = tokio::spawn(async move {
        loop {
            tokio::time::sleep(Duration::from_millis(CLEANUP_INTERVAL_MS)).await;
            let now_ms = now_ms();
            let should_shutdown = {
                let mut routes = janitor_state.routes.write().await;
                routes
                    .active_routes
                    .retain(|_, route| now_ms.saturating_sub(route.last_seen_ms) <= ROUTE_LEASE_MS);
                if routes.active_routes.is_empty() {
                    let idle_started_at = routes.idle_started_at_ms.get_or_insert(now_ms);
                    now_ms.saturating_sub(*idle_started_at) >= IDLE_SHUTDOWN_MS
                } else {
                    routes.idle_started_at_ms = None;
                    false
                }
            };
            if should_shutdown {
                let _ = janitor_tx.send(true);
                break;
            }
        }
    });

    let ctrlc_tx = shutdown_tx.clone();
    let ctrlc = tokio::spawn(async move {
        if tokio::signal::ctrl_c().await.is_ok() {
            let _ = ctrlc_tx.send(true);
        }
    });

    let control_result = control_task
        .await
        .context("join qit supervisor control task")??;
    let edge_result = edge_task.await.context("join qit supervisor edge task")??;
    let _ = control_result;
    let _ = edge_result;
    let _ = janitor.await;
    let _ = ctrlc.await;
    remove_discovery_file_if_owned(&data_root, discovery.pid);
    endpoint
        .shutdown()
        .await
        .context("shutdown supervisor public endpoint")?;
    Ok(())
}

async fn health() -> Json<HealthResponse> {
    Json(HealthResponse { ok: true })
}

async fn claim_route(
    State(state): State<SupervisorState>,
    Json(request): Json<ClaimRouteRequest>,
) -> Result<Json<ClaimRouteResponse>, StatusCode> {
    if request.workspace_id.trim().is_empty() {
        return Err(StatusCode::BAD_REQUEST);
    }
    let mut routes = state.routes.write().await;
    let mount_path = allocate_mount_path(
        &mut routes,
        &request.workspace_id,
        &request.preferred_repo_name,
    )
    .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    persist_assignments(&state.data_root, &routes)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(ClaimRouteResponse {
        mount_path,
        label: state.discovery.label.clone(),
        local_base_url: state.discovery.local_base_url.clone(),
        public_base_url: state.discovery.public_base_url.clone(),
    }))
}

async fn register_route_handler(
    State(state): State<SupervisorState>,
    Json(request): Json<RegisterRouteRequest>,
) -> Result<StatusCode, StatusCode> {
    if request.workspace_id.trim().is_empty()
        || request.mount_path.trim().is_empty()
        || request.upstream_url.trim().is_empty()
    {
        return Err(StatusCode::BAD_REQUEST);
    }
    let now_ms = now_ms();
    let mut routes = state.routes.write().await;
    let expected_mount = routes.assignments.get(&request.workspace_id).cloned();
    if expected_mount.as_deref() != Some(request.mount_path.as_str()) {
        return Err(StatusCode::CONFLICT);
    }
    routes.active_routes.insert(
        request.workspace_id.clone(),
        ActiveRoute {
            mount_path: request.mount_path,
            upstream_url: request.upstream_url.trim_end_matches('/').to_string(),
            worktree: request.worktree,
            last_seen_ms: now_ms,
        },
    );
    routes.idle_started_at_ms = None;
    Ok(StatusCode::NO_CONTENT)
}

async fn heartbeat_route_handler(
    State(state): State<SupervisorState>,
    Json(request): Json<HeartbeatRouteRequest>,
) -> Result<StatusCode, StatusCode> {
    let now_ms = now_ms();
    let mut routes = state.routes.write().await;
    let Some(route) = routes.active_routes.get_mut(&request.workspace_id) else {
        return Err(StatusCode::NOT_FOUND);
    };
    route.last_seen_ms = now_ms;
    Ok(StatusCode::NO_CONTENT)
}

async fn unregister_route_handler(
    State(state): State<SupervisorState>,
    Json(request): Json<UnregisterRouteRequest>,
) -> Result<StatusCode, StatusCode> {
    let mut routes = state.routes.write().await;
    routes.active_routes.remove(&request.workspace_id);
    if routes.active_routes.is_empty() {
        routes.idle_started_at_ms = Some(now_ms());
    }
    Ok(StatusCode::NO_CONTENT)
}

async fn index(State(state): State<SupervisorState>) -> Html<String> {
    let routes = state.routes.read().await;
    let mut items = routes.active_routes.values().cloned().collect::<Vec<_>>();
    items.sort_by(|left, right| left.mount_path.cmp(&right.mount_path));
    let entries = items
        .into_iter()
        .map(|route| {
            format!(
                "<li><a href=\"{path}\">{path}</a><br><small>{worktree}</small></li>",
                path = route.mount_path,
                worktree = html_escape(&route.worktree)
            )
        })
        .collect::<Vec<_>>()
        .join("");
    Html(format!(
        "<!doctype html><html><head><meta charset=\"utf-8\"><title>Qit</title></head><body><h1>Qit</h1><p>Shared entrypoint active on {}</p><ul>{}</ul></body></html>",
        html_escape(&state.discovery.public_base_url),
        entries
    ))
}

async fn proxy_request(
    State(state): State<SupervisorState>,
    request: Request,
) -> Result<Response<Body>, StatusCode> {
    let path = request.uri().path().to_string();
    let Some(route) = ({
        let routes = state.routes.read().await;
        match_route(&routes, &path)
    }) else {
        return Err(StatusCode::NOT_FOUND);
    };
    let (parts, body) = request.into_parts();
    let path_and_query = parts
        .uri
        .path_and_query()
        .map(|value| value.as_str())
        .unwrap_or("/");
    let target = format!("{}{}", route.upstream_url, path_and_query);
    let body_stream = body.into_data_stream();
    let original_host = parts.headers.get(HOST).cloned();
    let mut upstream = state.client.request(parts.method.clone(), &target);
    for (name, value) in &parts.headers {
        upstream = upstream.header(name, value);
    }
    if let Some(host) = original_host {
        upstream = upstream.header(HOST, host);
    }
    let response = upstream
        .body(reqwest::Body::wrap_stream(body_stream))
        .send()
        .await
        .map_err(|error| {
            eprintln!(
                "qit supervisor proxy failed for {} via {}: {}",
                path, route.upstream_url, error
            );
            StatusCode::BAD_GATEWAY
        })?;

    let status = response.status();
    let headers = response.headers().clone();
    let mut builder = Response::builder().status(status);
    for (name, value) in &headers {
        builder = builder.header(name, value);
    }
    builder
        .body(Body::from_stream(response.bytes_stream()))
        .map_err(|_| StatusCode::BAD_GATEWAY)
}

fn match_route(routes: &RouteRegistry, path: &str) -> Option<ActiveRoute> {
    routes
        .active_routes
        .values()
        .filter(|route| {
            path == route.mount_path || path.starts_with(&format!("{}/", route.mount_path))
        })
        .max_by_key(|route| route.mount_path.len())
        .cloned()
}

fn allocate_mount_path(
    routes: &mut RouteRegistry,
    workspace_id: &str,
    preferred_repo_name: &str,
) -> Result<String> {
    if let Some(existing) = routes.assignments.get(workspace_id) {
        if !mount_in_use_by_other(routes, existing, workspace_id) {
            return Ok(existing.clone());
        }
    }

    let base_mount = repo_mount_path(preferred_repo_name);
    if !mount_in_use_by_other(routes, &base_mount, workspace_id) {
        routes
            .assignments
            .insert(workspace_id.to_string(), base_mount.clone());
        return Ok(base_mount);
    }

    let short_id = workspace_id.chars().take(8).collect::<String>();
    let fallback_mount = format!("{base_mount}-{short_id}");
    if !mount_in_use_by_other(routes, &fallback_mount, workspace_id) {
        routes
            .assignments
            .insert(workspace_id.to_string(), fallback_mount.clone());
        return Ok(fallback_mount);
    }

    for index in 2..1000 {
        let candidate = format!("{fallback_mount}-{index}");
        if !mount_in_use_by_other(routes, &candidate, workspace_id) {
            routes
                .assignments
                .insert(workspace_id.to_string(), candidate.clone());
            return Ok(candidate);
        }
    }

    Err(anyhow!("failed to allocate route mount path"))
}

fn mount_in_use_by_other(routes: &RouteRegistry, mount_path: &str, workspace_id: &str) -> bool {
    routes
        .assignments
        .iter()
        .any(|(assigned_workspace, assigned_mount)| {
            assigned_mount == mount_path && assigned_workspace != workspace_id
        })
}

fn load_assignments(data_root: &Path) -> Result<RouteRegistry> {
    let path = assignments_path(data_root);
    if !path.exists() {
        return Ok(RouteRegistry::default());
    }
    let payload = std::fs::read(&path)
        .with_context(|| format!("read supervisor assignments {}", path.display()))?;
    let decoded: PersistedAssignments = serde_json::from_slice(&payload)
        .with_context(|| format!("decode supervisor assignments {}", path.display()))?;
    let assignments = decoded
        .assignments
        .into_iter()
        .map(|entry| (entry.workspace_id, entry.mount_path))
        .collect::<HashMap<_, _>>();
    Ok(RouteRegistry {
        assignments,
        active_routes: HashMap::new(),
        idle_started_at_ms: Some(now_ms()),
    })
}

fn persist_assignments(data_root: &Path, routes: &RouteRegistry) -> Result<()> {
    let payload = PersistedAssignments {
        assignments: routes
            .assignments
            .iter()
            .map(|(workspace_id, mount_path)| PersistedAssignment {
                workspace_id: workspace_id.clone(),
                mount_path: mount_path.clone(),
            })
            .collect(),
    };
    write_json_file(assignments_path(data_root), &payload)
}

fn write_json_file<T: Serialize>(path: PathBuf, value: &T) -> Result<()> {
    let encoded = serde_json::to_vec_pretty(value).context("encode qit supervisor json")?;
    let temp_path = path.with_extension("tmp");
    std::fs::write(&temp_path, encoded)
        .with_context(|| format!("write {}", temp_path.display()))?;
    std::fs::rename(&temp_path, &path).with_context(|| format!("replace {}", path.display()))?;
    Ok(())
}

fn remove_discovery_file_if_owned(data_root: &Path, pid: u32) {
    let path = discovery_path(data_root);
    let Ok(payload) = std::fs::read(&path) else {
        return;
    };
    let Ok(discovery) = serde_json::from_slice::<SupervisorDiscovery>(&payload) else {
        return;
    };
    if discovery.pid == pid {
        let _ = std::fs::remove_file(path);
    }
}

async fn load_healthy_discovery(data_root: &Path) -> Result<Option<SharedEntrypoint>> {
    let path = discovery_path(data_root);
    let payload = match tokio::fs::read(&path).await {
        Ok(payload) => payload,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(anyhow!(error).context(format!("read {}", path.display()))),
    };
    let discovery: SupervisorDiscovery =
        serde_json::from_slice(&payload).with_context(|| format!("decode {}", path.display()))?;
    let entrypoint = parse_discovery(discovery)?;
    let response = reqwest::Client::new()
        .get(control_url(&entrypoint.control_url, "/health")?)
        .send()
        .await;
    match response {
        Ok(response) if response.status().is_success() => Ok(Some(entrypoint)),
        _ => {
            let _ = tokio::fs::remove_file(path).await;
            Ok(None)
        }
    }
}

fn parse_discovery(discovery: SupervisorDiscovery) -> Result<SharedEntrypoint> {
    Ok(SharedEntrypoint {
        label: discovery.label,
        local_base_url: Url::parse(&discovery.local_base_url)
            .context("parse supervisor local URL")?,
        public_base_url: Url::parse(&discovery.public_base_url)
            .context("parse supervisor public URL")?,
        control_url: Url::parse(&discovery.control_url).context("parse supervisor control URL")?,
    })
}

fn control_url(base: &Url, suffix: &str) -> Result<Url> {
    base.join(&format!("{}{}", CONTROL_PREFIX, suffix))
        .context("build supervisor control URL")
}

fn discovery_path(data_root: &Path) -> PathBuf {
    data_root.join(DISCOVERY_FILE)
}

fn assignments_path(data_root: &Path) -> PathBuf {
    data_root.join(ASSIGNMENTS_FILE)
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_millis() as u64)
        .unwrap_or(0)
}

async fn wait_for_shutdown(mut shutdown_rx: watch::Receiver<bool>) {
    loop {
        if *shutdown_rx.borrow() {
            break;
        }
        if shutdown_rx.changed().await.is_err() {
            break;
        }
    }
}

fn html_escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

fn supervisor_edge_bind_host(transport: PublicTransport) -> &'static str {
    match transport {
        PublicTransport::Lan => "0.0.0.0",
        _ => "127.0.0.1",
    }
}

#[cfg(test)]
mod tests {
    use super::{allocate_mount_path, supervisor_edge_bind_host, RouteRegistry};
    use qit_transports::PublicTransport;

    #[test]
    fn mount_paths_reuse_saved_assignment() {
        let mut routes = RouteRegistry::default();
        let first = allocate_mount_path(&mut routes, "aaaabbbb-1111", "repo").unwrap();
        let second = allocate_mount_path(&mut routes, "aaaabbbb-1111", "repo").unwrap();
        assert_eq!(first, "/repo");
        assert_eq!(second, "/repo");
    }

    #[test]
    fn mount_paths_disambiguate_conflicts() {
        let mut routes = RouteRegistry::default();
        let first = allocate_mount_path(&mut routes, "aaaabbbb-1111", "repo").unwrap();
        let second = allocate_mount_path(&mut routes, "ccccdddd-2222", "repo").unwrap();
        assert_eq!(first, "/repo");
        assert_eq!(second, "/repo-ccccdddd");
    }

    #[test]
    fn lan_supervisor_binds_on_all_interfaces() {
        assert_eq!(supervisor_edge_bind_host(PublicTransport::Lan), "0.0.0.0");
        assert_eq!(
            supervisor_edge_bind_host(PublicTransport::Local),
            "127.0.0.1"
        );
    }
}
