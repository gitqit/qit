use super::{
    cookie_value, LoginAttemptRecord, ResolvedSession, SessionRecord, WebUiServer,
    LOGIN_FAILURE_LIMIT, LOGIN_LOCKOUT_MS, LOGIN_WINDOW_MS,
};
use axum::http::HeaderMap;
use axum::http::HeaderValue;
use qit_domain::{AuthMethod, UiRole};
use std::time::{SystemTime, UNIX_EPOCH};

impl WebUiServer {
    pub(super) fn session_cookie(&self, token: &str) -> HeaderValue {
        let secure = if self.secure_cookies { "; Secure" } else { "" };
        HeaderValue::from_str(&format!(
            "{}={token}; Path={}; HttpOnly; SameSite=Lax{}",
            Self::cookie_name(),
            self.repo_mount_path,
            secure
        ))
        .expect("valid session cookie")
    }

    pub(super) fn clear_cookie(&self) -> HeaderValue {
        let secure = if self.secure_cookies { "; Secure" } else { "" };
        HeaderValue::from_str(&format!(
            "{}=; Path={}; Max-Age=0; HttpOnly; SameSite=Lax{}",
            Self::cookie_name(),
            self.repo_mount_path,
            secure
        ))
        .expect("valid clear cookie")
    }

    pub(super) fn now_ms() -> u64 {
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|duration| duration.as_millis() as u64)
            .unwrap_or(0)
    }

    pub(super) fn default_session(&self, headers: &HeaderMap) -> Option<ResolvedSession> {
        (self.implicit_owner_mode && host_is_loopback(headers)).then_some(ResolvedSession {
            actor: UiRole::Owner,
            principal: None,
            operator_override: true,
        })
    }

    pub(super) async fn current_session(
        &self,
        headers: &HeaderMap,
    ) -> Result<Option<ResolvedSession>, axum::http::StatusCode> {
        if let Some(session) = self.default_session(headers) {
            return Ok(Some(session));
        }
        let Some(token) = cookie_value(headers, Self::cookie_name()) else {
            return Ok(None);
        };
        let now_ms = Self::now_ms();
        let mut sessions = self.sessions.write().await;
        sessions.retain(|_, session: &mut SessionRecord| session.expires_at_ms > now_ms);
        let Some(session) = sessions.get(token).cloned() else {
            return Ok(None);
        };
        if let Some(user_id) = session.user_id {
            match self.workspace_service.resolve_active_principal(
                self.workspace.worktree.clone(),
                &self.workspace.exported_branch,
                &user_id,
            ) {
                Ok((_workspace, principal)) => {
                    return Ok(Some(ResolvedSession {
                        actor: principal.ui_role(),
                        principal: Some(principal),
                        operator_override: false,
                    }))
                }
                Err(_) => {
                    sessions.remove(token);
                    return Ok(None);
                }
            }
        }
        Ok(Some(ResolvedSession {
            actor: session.role,
            principal: None,
            operator_override: false,
        }))
    }

    pub(super) async fn require_session(
        &self,
        headers: &HeaderMap,
    ) -> Result<ResolvedSession, axum::http::StatusCode> {
        self.current_session(headers)
            .await?
            .ok_or(axum::http::StatusCode::UNAUTHORIZED)
    }

    pub(super) async fn require_actor(
        &self,
        headers: &HeaderMap,
    ) -> Result<UiRole, axum::http::StatusCode> {
        Ok(self.require_session(headers).await?.actor)
    }

    pub(super) async fn require_owner(
        &self,
        headers: &HeaderMap,
    ) -> Result<(), axum::http::StatusCode> {
        match self.require_actor(headers).await? {
            UiRole::Owner => Ok(()),
            UiRole::User => Err(axum::http::StatusCode::FORBIDDEN),
        }
    }

    pub(super) fn can_view_git_credentials(
        &self,
        session: Option<&ResolvedSession>,
        auth_methods: &[AuthMethod],
    ) -> bool {
        auth_methods.contains(&AuthMethod::BasicAuth)
            && matches!(session, Some(ResolvedSession { operator_override: true, .. }))
    }

    fn login_attempt_key(headers: &HeaderMap) -> String {
        headers
            .get(axum::http::header::HOST)
            .and_then(|value| value.to_str().ok())
            .unwrap_or("unknown-host")
            .trim()
            .to_string()
    }

    pub(super) async fn allow_login_attempt(
        &self,
        headers: &HeaderMap,
    ) -> Result<(), axum::http::StatusCode> {
        let now_ms = Self::now_ms();
        let key = Self::login_attempt_key(headers);
        let mut attempts = self.login_attempts.write().await;
        attempts.retain(|_, record| record.locked_until_ms > now_ms || record.failures > 0);
        if attempts
            .get(&key)
            .is_some_and(|record| record.locked_until_ms > now_ms)
        {
            return Err(axum::http::StatusCode::TOO_MANY_REQUESTS);
        }
        Ok(())
    }

    pub(super) async fn record_login_failure(&self, headers: &HeaderMap) {
        let now_ms = Self::now_ms();
        let key = Self::login_attempt_key(headers);
        let mut attempts = self.login_attempts.write().await;
        let record = attempts.entry(key).or_insert_with(|| LoginAttemptRecord {
            failures: 0,
            window_started_at_ms: now_ms,
            locked_until_ms: 0,
        });
        if now_ms.saturating_sub(record.window_started_at_ms) > LOGIN_WINDOW_MS {
            record.failures = 0;
            record.window_started_at_ms = now_ms;
            record.locked_until_ms = 0;
        }
        record.failures = record.failures.saturating_add(1);
        if record.failures >= LOGIN_FAILURE_LIMIT {
            record.locked_until_ms = now_ms.saturating_add(LOGIN_LOCKOUT_MS);
        }
    }

    pub(super) async fn clear_login_attempts(&self, headers: &HeaderMap) {
        let key = Self::login_attempt_key(headers);
        self.login_attempts.write().await.remove(&key);
    }
}

pub(super) fn host_is_loopback(headers: &HeaderMap) -> bool {
    let Some(host) = headers
        .get(axum::http::header::HOST)
        .and_then(|value| value.to_str().ok())
    else {
        return false;
    };
    let host = host.trim();
    let host = if let Some(stripped) = host.strip_prefix('[') {
        stripped.split(']').next().unwrap_or(stripped)
    } else {
        host.split(':').next().unwrap_or(host)
    };
    matches!(host, "localhost" | "127.0.0.1" | "::1")
}
