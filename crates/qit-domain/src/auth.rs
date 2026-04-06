use crate::{DomainError, UiRole};
use argon2::{
    password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};
use rand::distributions::{Alphanumeric, DistString};
use serde::{Deserialize, Serialize};

pub const ONBOARDING_TOKEN_TTL_MS: u64 = 24 * 60 * 60 * 1000;
const ONBOARDING_TOKEN_PREFIX: &str = "qit_setup";
const PAT_TOKEN_PREFIX: &str = "qit_pat";
const ACCESS_REQUEST_RECEIPT_PREFIX: &str = "qit_request";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub enum AuthMode {
    #[default]
    SharedSession,
    RequestBased,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum AuthMethod {
    RequestAccess,
    SetupToken,
    BasicAuth,
}

impl AuthMethod {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::RequestAccess => "request_access",
            Self::SetupToken => "setup_token",
            Self::BasicAuth => "basic_auth",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RepoUserRole {
    Owner,
    User,
}

impl RepoUserRole {
    pub fn as_ui_role(&self) -> UiRole {
        match self {
            Self::Owner => UiRole::Owner,
            Self::User => UiRole::User,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RepoUserStatus {
    PendingRequest,
    ApprovedPendingSetup,
    Active,
    Revoked,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AccessRequestStatus {
    Pending,
    Approved,
    Rejected,
    Revoked,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AccessRequest {
    pub id: String,
    pub name: String,
    pub email: String,
    pub status: AccessRequestStatus,
    #[serde(default)]
    pub request_secret_verifier: Option<String>,
    #[serde(default)]
    pub linked_user_id: Option<String>,
    pub created_at_ms: u64,
    #[serde(default)]
    pub reviewed_at_ms: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RepoUser {
    pub id: String,
    pub name: String,
    pub email: String,
    #[serde(default)]
    pub username: Option<String>,
    #[serde(default)]
    pub password_verifier: Option<String>,
    pub role: RepoUserRole,
    pub status: RepoUserStatus,
    pub created_at_ms: u64,
    pub updated_at_ms: u64,
    #[serde(default)]
    pub approved_at_ms: Option<u64>,
    #[serde(default)]
    pub activated_at_ms: Option<u64>,
    #[serde(default)]
    pub revoked_at_ms: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct OnboardingToken {
    pub id: String,
    pub user_id: String,
    pub verifier: String,
    pub created_at_ms: u64,
    pub expires_at_ms: u64,
    #[serde(default)]
    pub redeemed_at_ms: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PatRecord {
    pub id: String,
    pub user_id: String,
    pub label: String,
    pub verifier: String,
    pub created_at_ms: u64,
    #[serde(default)]
    pub revoked_at_ms: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AuthActorKind {
    Anonymous,
    Operator,
    User,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AuthActivityKind {
    AuthModeChanged,
    AccessRequested,
    AccessApproved,
    AccessRejected,
    UserSetupIssued,
    UserPromoted,
    UserDemoted,
    UserRevoked,
    UserSetupReset,
    UserSetupCompleted,
    PasswordReset,
    PatCreated,
    PatRevoked,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AuthActivityRecord {
    pub id: String,
    pub kind: AuthActivityKind,
    pub actor_kind: AuthActorKind,
    pub actor_label: String,
    #[serde(default)]
    pub target_user_id: Option<String>,
    #[serde(default)]
    pub request_id: Option<String>,
    #[serde(default)]
    pub pat_id: Option<String>,
    #[serde(default)]
    pub detail: Option<String>,
    pub created_at_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RepoAuthState {
    #[serde(default)]
    pub mode: AuthMode,
    #[serde(default)]
    pub methods: Vec<AuthMethod>,
    #[serde(default)]
    pub access_requests: Vec<AccessRequest>,
    #[serde(default)]
    pub users: Vec<RepoUser>,
    #[serde(default)]
    pub onboarding_tokens: Vec<OnboardingToken>,
    #[serde(default)]
    pub personal_access_tokens: Vec<PatRecord>,
    #[serde(default)]
    pub activity: Vec<AuthActivityRecord>,
}

impl Default for RepoAuthState {
    fn default() -> Self {
        Self {
            mode: AuthMode::RequestBased,
            methods: vec![AuthMethod::RequestAccess, AuthMethod::SetupToken],
            access_requests: Vec::new(),
            users: Vec::new(),
            onboarding_tokens: Vec::new(),
            personal_access_tokens: Vec::new(),
            activity: Vec::new(),
        }
    }
}

impl RepoAuthState {
    pub fn methods_for_mode(mode: &AuthMode) -> Vec<AuthMethod> {
        match mode {
            AuthMode::SharedSession => vec![AuthMethod::BasicAuth],
            AuthMode::RequestBased => vec![AuthMethod::RequestAccess, AuthMethod::SetupToken],
        }
    }

    pub fn compatibility_mode_from_methods(methods: &[AuthMethod]) -> AuthMode {
        if methods.len() == 1 && methods[0] == AuthMethod::BasicAuth {
            AuthMode::SharedSession
        } else {
            AuthMode::RequestBased
        }
    }

    pub fn has_method(&self, method: &AuthMethod) -> bool {
        self.methods.iter().any(|candidate| candidate == method)
    }

    pub fn method_labels(&self) -> Vec<&'static str> {
        self.methods.iter().map(AuthMethod::as_str).collect()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AuthenticatedPrincipal {
    pub user_id: String,
    pub name: String,
    pub email: String,
    pub username: String,
    pub role: RepoUserRole,
}

impl AuthenticatedPrincipal {
    pub fn ui_role(&self) -> UiRole {
        self.role.as_ui_role()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct RepoUserView {
    pub id: String,
    pub name: String,
    pub email: String,
    #[serde(default)]
    pub username: Option<String>,
    pub role: RepoUserRole,
    pub status: RepoUserStatus,
    pub created_at_ms: u64,
    #[serde(default)]
    pub approved_at_ms: Option<u64>,
    #[serde(default)]
    pub activated_at_ms: Option<u64>,
    #[serde(default)]
    pub revoked_at_ms: Option<u64>,
}

impl From<&RepoUser> for RepoUserView {
    fn from(value: &RepoUser) -> Self {
        Self {
            id: value.id.clone(),
            name: value.name.clone(),
            email: value.email.clone(),
            username: value.username.clone(),
            role: value.role.clone(),
            status: value.status.clone(),
            created_at_ms: value.created_at_ms,
            approved_at_ms: value.approved_at_ms,
            activated_at_ms: value.activated_at_ms,
            revoked_at_ms: value.revoked_at_ms,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AccessRequestView {
    pub id: String,
    pub name: String,
    pub email: String,
    pub status: AccessRequestStatus,
    pub created_at_ms: u64,
    #[serde(default)]
    pub reviewed_at_ms: Option<u64>,
}

impl From<&AccessRequest> for AccessRequestView {
    fn from(value: &AccessRequest) -> Self {
        Self {
            id: value.id.clone(),
            name: value.name.clone(),
            email: value.email.clone(),
            status: value.status.clone(),
            created_at_ms: value.created_at_ms,
            reviewed_at_ms: value.reviewed_at_ms,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SubmittedAccessRequest {
    pub request: AccessRequestView,
    pub secret: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct AccessRequestProgress {
    pub id: String,
    pub status: AccessRequestStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct PatRecordView {
    pub id: String,
    pub label: String,
    pub created_at_ms: u64,
    #[serde(default)]
    pub revoked_at_ms: Option<u64>,
}

impl From<&PatRecord> for PatRecordView {
    fn from(value: &PatRecord) -> Self {
        Self {
            id: value.id.clone(),
            label: value.label.clone(),
            created_at_ms: value.created_at_ms,
            revoked_at_ms: value.revoked_at_ms,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct IssuedOnboarding {
    pub user_id: String,
    pub email: String,
    /// One-time `qit_setup…` code for manual owner onboarding. Absent when access was approved via request (requester finishes with their `qit_request…` receipt).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub secret: Option<String>,
    pub expires_at_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct IssuedPat {
    pub id: String,
    pub label: String,
    pub secret: String,
    pub created_at_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuthActor {
    Anonymous,
    Operator,
    User {
        user_id: String,
        username: String,
        role: RepoUserRole,
    },
}

impl AuthActor {
    pub fn kind(&self) -> AuthActorKind {
        match self {
            Self::Anonymous => AuthActorKind::Anonymous,
            Self::Operator => AuthActorKind::Operator,
            Self::User { .. } => AuthActorKind::User,
        }
    }

    pub fn label(&self) -> String {
        match self {
            Self::Anonymous => "anonymous".to_string(),
            Self::Operator => "localhost-operator".to_string(),
            Self::User { username, .. } => username.clone(),
        }
    }
}

pub(crate) fn hash_secret(secret: &str) -> Result<String, DomainError> {
    let salt = SaltString::generate(&mut OsRng);
    Argon2::default()
        .hash_password(secret.as_bytes(), &salt)
        .map(|hash| hash.to_string())
        .map_err(|error| DomainError::InvalidAuth(format!("failed to hash secret: {error}")))
}

pub(crate) fn verify_secret(secret: &str, verifier: &str) -> bool {
    let Ok(parsed) = PasswordHash::new(verifier) else {
        return false;
    };
    Argon2::default()
        .verify_password(secret.as_bytes(), &parsed)
        .is_ok()
}

fn issue_secret(prefix: &str, id: &str) -> Result<(String, String), DomainError> {
    let raw_secret = Alphanumeric.sample_string(&mut rand::thread_rng(), 32);
    let verifier = hash_secret(&raw_secret)?;
    Ok((format!("{prefix}.{id}.{raw_secret}"), verifier))
}

fn parse_secret<'a>(secret: &'a str, prefix: &str) -> Option<(&'a str, &'a str)> {
    let mut parts = secret.splitn(3, '.');
    let actual_prefix = parts.next()?;
    let id = parts.next()?;
    let raw_secret = parts.next()?;
    (actual_prefix == prefix && !id.is_empty() && !raw_secret.is_empty())
        .then_some((id, raw_secret))
}

pub(crate) fn issue_onboarding_secret(id: &str) -> Result<(String, String), DomainError> {
    issue_secret(ONBOARDING_TOKEN_PREFIX, id)
}

pub(crate) fn parse_onboarding_secret(secret: &str) -> Option<(&str, &str)> {
    parse_secret(secret, ONBOARDING_TOKEN_PREFIX)
}

pub(crate) fn issue_pat_secret(id: &str) -> Result<(String, String), DomainError> {
    issue_secret(PAT_TOKEN_PREFIX, id)
}

pub(crate) fn parse_pat_secret(secret: &str) -> Option<(&str, &str)> {
    parse_secret(secret, PAT_TOKEN_PREFIX)
}

pub(crate) fn issue_access_request_secret(id: &str) -> Result<(String, String), DomainError> {
    issue_secret(ACCESS_REQUEST_RECEIPT_PREFIX, id)
}

pub(crate) fn parse_access_request_secret(secret: &str) -> Option<(&str, &str)> {
    parse_secret(secret, ACCESS_REQUEST_RECEIPT_PREFIX)
}
