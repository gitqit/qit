use crate::UiRole;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum IssueStatus {
    Open,
    Closed,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum IssueReactionContent {
    ThumbsUp,
    ThumbsDown,
    Laugh,
    Hooray,
    Confused,
    Heart,
    Rocket,
    Eyes,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct IssueActor {
    pub role: UiRole,
    pub display_name: String,
    #[serde(default)]
    pub user_id: Option<String>,
    #[serde(default)]
    pub username: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct IssueReactionRecord {
    pub id: String,
    pub content: IssueReactionContent,
    pub actor: IssueActor,
    pub created_at_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct IssueReactionSummary {
    pub content: IssueReactionContent,
    pub count: usize,
    pub reacted: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct IssueCommentRecord {
    pub id: String,
    pub actor: IssueActor,
    pub body: String,
    pub created_at_ms: u64,
    pub updated_at_ms: u64,
    #[serde(default)]
    pub reactions: Vec<IssueReactionRecord>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum IssueTimelineEventKind {
    Opened,
    Commented,
    Edited,
    Closed,
    Reopened,
    LabelsChanged,
    AssigneesChanged,
    MilestoneChanged,
    PullRequestLinked,
    PullRequestUnlinked,
    ReactionToggled,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum IssueTimelineTarget {
    Issue,
    Comment,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct IssueTimelineEvent {
    pub id: String,
    pub kind: IssueTimelineEventKind,
    pub actor: IssueActor,
    #[serde(default)]
    pub body: Option<String>,
    #[serde(default)]
    pub title: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default)]
    pub labels: Vec<String>,
    #[serde(default)]
    pub assignee_user_ids: Vec<String>,
    #[serde(default)]
    pub milestone_id: Option<String>,
    #[serde(default)]
    pub pull_request_id: Option<String>,
    #[serde(default)]
    pub reaction: Option<IssueReactionContent>,
    #[serde(default)]
    pub target: Option<IssueTimelineTarget>,
    #[serde(default)]
    pub target_id: Option<String>,
    pub created_at_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum IssueLinkRelation {
    Related,
    Closing,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum IssueLinkSource {
    Manual,
    IssueDescription,
    IssueComment,
    PullRequestDescription,
    PullRequestComment,
    PullRequestReview,
}

const fn default_issue_link_relation() -> IssueLinkRelation {
    IssueLinkRelation::Related
}

const fn default_issue_link_source() -> IssueLinkSource {
    IssueLinkSource::Manual
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct IssueLinkedPullRequest {
    pub pull_request_id: String,
    #[serde(default = "default_issue_link_relation")]
    pub relation: IssueLinkRelation,
    #[serde(default = "default_issue_link_source")]
    pub source: IssueLinkSource,
    pub linked_at_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LinkedIssueReference {
    pub issue_id: String,
    pub issue_number: u64,
    pub relation: IssueLinkRelation,
    pub source: IssueLinkSource,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LinkedPullRequestReference {
    pub pull_request_id: String,
    pub relation: IssueLinkRelation,
    pub source: IssueLinkSource,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct IssueLabel {
    pub id: String,
    pub name: String,
    pub color: String,
    #[serde(default)]
    pub description: String,
    pub created_at_ms: u64,
    pub updated_at_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct IssueMilestone {
    pub id: String,
    pub title: String,
    #[serde(default)]
    pub description: String,
    pub created_at_ms: u64,
    pub updated_at_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct IssueRecord {
    pub id: String,
    pub number: u64,
    pub title: String,
    pub description: String,
    pub status: IssueStatus,
    pub author: IssueActor,
    pub created_at_ms: u64,
    pub updated_at_ms: u64,
    #[serde(default)]
    pub closed_at_ms: Option<u64>,
    #[serde(default)]
    pub label_ids: Vec<String>,
    #[serde(default)]
    pub assignee_user_ids: Vec<String>,
    #[serde(default)]
    pub milestone_id: Option<String>,
    #[serde(default)]
    pub linked_pull_requests: Vec<IssueLinkedPullRequest>,
    #[serde(default)]
    pub reactions: Vec<IssueReactionRecord>,
    #[serde(default)]
    pub comments: Vec<IssueCommentRecord>,
    #[serde(default)]
    pub timeline: Vec<IssueTimelineEvent>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct IssueSettings {
    #[serde(default = "default_next_issue_number")]
    pub next_issue_number: u64,
    #[serde(default)]
    pub labels: Vec<IssueLabel>,
    #[serde(default)]
    pub milestones: Vec<IssueMilestone>,
}

const fn default_next_issue_number() -> u64 {
    1
}

impl Default for IssueSettings {
    fn default() -> Self {
        Self {
            next_issue_number: default_next_issue_number(),
            labels: Vec::new(),
            milestones: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IssueActorInput {
    pub role: UiRole,
    pub display_name: Option<String>,
    pub user_id: Option<String>,
    pub username: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreateIssue {
    pub title: String,
    pub description: String,
    pub label_ids: Vec<String>,
    pub assignee_user_ids: Vec<String>,
    pub milestone_id: Option<String>,
    pub linked_pull_request_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct UpdateIssue {
    pub title: Option<String>,
    pub description: Option<String>,
    pub status: Option<IssueStatus>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CreateIssueComment {
    pub display_name: Option<String>,
    pub body: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpsertIssueLabel {
    pub id: Option<String>,
    pub name: String,
    pub color: String,
    pub description: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpsertIssueMilestone {
    pub id: Option<String>,
    pub title: String,
    pub description: String,
}
