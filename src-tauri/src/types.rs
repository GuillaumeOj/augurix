use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Project {
    pub id: Uuid,
    pub name: String,
    pub path: PathBuf,
    pub added_at: chrono::DateTime<chrono::Utc>,
    #[serde(default)]
    pub pinned: bool,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum AgentKind {
    ClaudeCode,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum AgentState {
    NotRunning,
    Idle,
    Running,
    AwaitingInput,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentStatus {
    pub kind: AgentKind,
    pub state: AgentState,
    pub pid: Option<u32>,
    pub session_id: Option<String>,
    pub last_message_preview: Option<String>,
    pub last_activity_at: Option<chrono::DateTime<chrono::Utc>>,
}

impl AgentStatus {
    pub fn not_running() -> Self {
        Self {
            kind: AgentKind::ClaudeCode,
            state: AgentState::NotRunning,
            pid: None,
            session_id: None,
            last_message_preview: None,
            last_activity_at: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GitStatus {
    pub is_repo: bool,
    pub branch: Option<String>,
    pub upstream: Option<String>,
    pub ahead: u32,
    pub behind: u32,
    pub modified: u32,
    pub staged: u32,
    pub untracked: u32,
    pub conflicted: u32,
}

impl GitStatus {
    pub fn not_a_repo() -> Self {
        Self {
            is_repo: false,
            branch: None,
            upstream: None,
            ahead: 0,
            behind: 0,
            modified: 0,
            staged: 0,
            untracked: 0,
            conflicted: 0,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "UPPERCASE")]
pub enum PrState {
    Open,
    Closed,
    Merged,
    Draft,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "UPPERCASE")]
pub enum ChecksRollup {
    Pending,
    Success,
    Failure,
    None,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PrStatus {
    pub number: u32,
    pub title: String,
    pub state: PrState,
    pub checks: ChecksRollup,
    pub url: String,
    pub is_draft: bool,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum InstanceKind {
    /// The repo's primary worktree (or the project root if it's not a git repo).
    MainWorktree,
    /// A linked worktree created via `git worktree add`.
    LinkedWorktree,
    /// A sub-directory of the project that has a Claude Code transcript.
    SubProject,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Instance {
    pub id: Uuid,
    pub project_id: Uuid,
    pub kind: InstanceKind,
    pub path: PathBuf,
    /// Display label: relative to project root, or absolute path if outside.
    pub label: String,
    /// Optional worktree branch (cheap to read while parsing `git worktree list`).
    pub branch_hint: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstanceStatus {
    pub instance_id: Uuid,
    pub agent: AgentStatus,
    pub git: GitStatus,
    pub pr: Option<PrStatus>,
    pub last_refreshed: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstanceWithStatus {
    #[serde(flatten)]
    pub instance: Instance,
    pub status: InstanceStatus,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProjectWithInstances {
    pub project: Project,
    pub instances: Vec<InstanceWithStatus>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolUseEntry {
    pub name: String,
    pub detail: Option<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum MessageRole {
    User,
    Assistant,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TranscriptMessage {
    /// `None` if the transcript entry had no parseable timestamp. The frontend
    /// renders this as an em-dash; never fabricate "now".
    pub timestamp: Option<chrono::DateTime<chrono::Utc>>,
    pub role: MessageRole,
    pub text: Option<String>,
    pub tool_uses: Vec<ToolUseEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiscoveredProject {
    pub path: PathBuf,
    pub name: String,
    pub last_session_at: Option<chrono::DateTime<chrono::Utc>>,
    pub already_added: bool,
}

/// Stable per-(project, path) UUID using v5 hashing.
pub fn instance_id(project_id: Uuid, path: &std::path::Path) -> Uuid {
    Uuid::new_v5(&project_id, path.to_string_lossy().as_bytes())
}
