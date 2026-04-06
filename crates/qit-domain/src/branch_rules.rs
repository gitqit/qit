use crate::DomainError;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Default)]
pub struct BranchRule {
    pub pattern: String,
    #[serde(default)]
    pub require_pull_request: bool,
    #[serde(default)]
    pub required_approvals: u8,
    #[serde(default)]
    pub dismiss_stale_approvals: bool,
    #[serde(default)]
    pub block_force_push: bool,
    #[serde(default)]
    pub block_delete: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct BranchProtection {
    pub patterns: Vec<String>,
    pub require_pull_request: bool,
    pub required_approvals: u8,
    pub dismiss_stale_approvals: bool,
    pub block_force_push: bool,
    pub block_delete: bool,
}

pub(crate) fn normalize_branch_rule_pattern(pattern: &str) -> Result<String, DomainError> {
    let pattern = pattern.trim().to_string();
    if pattern.is_empty() {
        return Err(DomainError::InvalidSettings(
            "branch rule pattern is required".into(),
        ));
    }
    if pattern.len() > 256 {
        return Err(DomainError::InvalidSettings(
            "branch rule pattern must be 256 characters or fewer".into(),
        ));
    }
    if !pattern
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '/' | '.' | '_' | '-' | '*' | '?'))
    {
        return Err(DomainError::InvalidSettings(
            "branch rule patterns may only use letters, numbers, '/', '.', '_', '-', '*', and '?'"
                .into(),
        ));
    }
    Ok(pattern)
}

pub(crate) fn glob_match(pattern: &[u8], text: &[u8]) -> bool {
    if pattern.is_empty() {
        return text.is_empty();
    }
    match pattern[0] {
        b'*' => {
            glob_match(&pattern[1..], text) || (!text.is_empty() && glob_match(pattern, &text[1..]))
        }
        b'?' => !text.is_empty() && glob_match(&pattern[1..], &text[1..]),
        byte => !text.is_empty() && byte == text[0] && glob_match(&pattern[1..], &text[1..]),
    }
}
