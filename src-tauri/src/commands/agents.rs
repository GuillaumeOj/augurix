use crate::caches::ProcessSnapshot;
use crate::claude_path::{claude_projects_root, encoded_path};
use crate::types::{
    AgentKind, AgentState, AgentStatus, MessageRole, PendingInteraction, PendingQuestion,
    QuestionOption, ToolUseEntry, TranscriptMessage,
};
use chrono::{DateTime, Utc};
use serde::Deserialize;
use std::collections::HashSet;
use std::io::{Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

const FRESH_ACTIVITY_SECS: i64 = 25;
const TAIL_LINES: usize = 256;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LastRole {
    User,
    Assistant,
    Other,
}

/// A single transcript file summarized.
#[derive(Debug, Clone)]
pub struct TranscriptSummary {
    pub file_path: PathBuf,
    pub session_id: String,
    pub mtime: DateTime<Utc>,
    /// `cwd` from the most-recent entry, if present.
    pub cwd: Option<PathBuf>,
    pub last_role: LastRole,
    pub preview: Option<String>,
}

/// Scan all transcript files under any encoded directory that begins with
/// the project's encoded path. Returns one summary per `.jsonl` file.
pub fn scan_transcripts(project_root: &Path) -> Vec<TranscriptSummary> {
    let Some(root) = claude_projects_root() else {
        return vec![];
    };
    if !root.exists() {
        return vec![];
    }
    let prefix = encoded_path(project_root);
    let prefix_with_sep = format!("{prefix}-");

    let mut out = Vec::new();
    let Ok(dirs) = std::fs::read_dir(&root) else {
        return out;
    };
    for dir_entry in dirs.flatten() {
        let Ok(md) = dir_entry.metadata() else {
            continue;
        };
        if !md.is_dir() {
            continue;
        }
        let name_os = dir_entry.file_name();
        let Some(name) = name_os.to_str() else {
            continue;
        };
        // Match exact encoded path OR encoded-path + "-<subpath>"
        if name != prefix && !name.starts_with(&prefix_with_sep) {
            continue;
        }

        let Ok(files) = std::fs::read_dir(dir_entry.path()) else {
            continue;
        };
        for f in files.flatten() {
            let p = f.path();
            if p.extension().map(|e| e == "jsonl").unwrap_or(false) {
                if let Some(s) = read_transcript_summary(&p) {
                    out.push(s);
                }
            }
        }
    }
    out
}

pub fn build_agent_status(
    instance_path: &Path,
    transcript: Option<&TranscriptSummary>,
    snap: &ProcessSnapshot,
) -> AgentStatus {
    let mut status = AgentStatus::not_running();
    status.kind = AgentKind::ClaudeCode;

    if let Some(t) = transcript {
        status.session_id = Some(t.session_id.clone());
        status.last_activity_at = Some(t.mtime);
        status.last_message_preview = t.preview.clone();
    }

    let process = find_claude_process(instance_path, snap);
    if let Some(p) = process {
        status.pid = Some(p);
    }

    let fresh = transcript
        .map(|t| (Utc::now() - t.mtime).num_seconds() <= FRESH_ACTIVITY_SECS)
        .unwrap_or(false);

    status.state = match (process.is_some(), transcript, fresh) {
        (false, None, _) => AgentState::NotRunning,
        (true, None, _) => AgentState::Running,
        (_, Some(t), true) => {
            if t.last_role == LastRole::User {
                AgentState::Running
            } else {
                AgentState::AwaitingInput
            }
        }
        (true, Some(_), false) => AgentState::Idle,
        (false, Some(_), false) => AgentState::NotRunning,
    };

    status
}

#[derive(Debug, Deserialize)]
struct TranscriptEntry {
    #[serde(default)]
    role: Option<String>,
    #[serde(default, rename = "type")]
    entry_type: Option<String>,
    #[serde(default)]
    message: Option<serde_json::Value>,
    #[serde(default)]
    content: Option<serde_json::Value>,
    #[serde(default)]
    cwd: Option<String>,
    #[serde(default)]
    timestamp: Option<String>,
}

fn read_transcript_summary(path: &Path) -> Option<TranscriptSummary> {
    let md = std::fs::metadata(path).ok()?;
    let mtime: DateTime<Utc> = md.modified().ok()?.into();
    let session_id = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_string();

    let lines = tail_lines(path, TAIL_LINES)?;
    let mut last_role = LastRole::Other;
    let mut last_seen_user = false;
    let mut last_text: Option<String> = None;
    let mut last_tool: Option<String> = None;
    let mut last_cwd: Option<PathBuf> = None;

    for line in lines.iter() {
        let Ok(entry) = serde_json::from_str::<TranscriptEntry>(line) else {
            continue;
        };
        if let Some(c) = entry.cwd.as_ref() {
            last_cwd = Some(PathBuf::from(c));
        }
        let role = entry
            .role
            .clone()
            .or_else(|| {
                entry
                    .message
                    .as_ref()
                    .and_then(|m| m.get("role"))
                    .and_then(|v| v.as_str().map(String::from))
            })
            .or(entry.entry_type.clone())
            .unwrap_or_default()
            .to_lowercase();

        match role.as_str() {
            "user" => {
                last_role = LastRole::User;
                last_seen_user = true;
            }
            "assistant" => {
                last_role = LastRole::Assistant;
                last_seen_user = false;
                summarize_assistant(&entry, &mut last_text, &mut last_tool);
            }
            _ => {}
        }
    }
    if last_seen_user {
        last_role = LastRole::User;
    }

    let preview = match (last_text.as_ref(), last_tool.as_ref()) {
        (Some(text), Some(tool)) => Some(format!("{} · {}", tool, truncate(text, 100))),
        (Some(text), None) => Some(truncate(text, 140)),
        (None, Some(tool)) => Some(format!("Using {}…", tool)),
        (None, None) => None,
    };

    Some(TranscriptSummary {
        file_path: path.to_path_buf(),
        session_id,
        mtime,
        cwd: last_cwd,
        last_role,
        preview,
    })
}

fn summarize_assistant(
    entry: &TranscriptEntry,
    last_text: &mut Option<String>,
    last_tool: &mut Option<String>,
) {
    let Some(value) = entry.message.as_ref().or(entry.content.as_ref()) else {
        return;
    };
    let content = value.get("content").unwrap_or(value);

    if let Some(s) = content.as_str() {
        let trimmed = s.trim();
        if !trimmed.is_empty() {
            *last_text = Some(trimmed.to_string());
        }
        return;
    }
    if let Some(arr) = content.as_array() {
        let mut any_tool_use = false;
        for item in arr {
            match item.get("type").and_then(|v| v.as_str()) {
                Some("text") => {
                    if let Some(t) = item.get("text").and_then(|v| v.as_str()) {
                        let trimmed = t.trim();
                        if !trimmed.is_empty() {
                            *last_text = Some(trimmed.to_string());
                        }
                    }
                }
                Some("tool_use") => {
                    any_tool_use = true;
                    if let Some(name) = item.get("name").and_then(|v| v.as_str()) {
                        *last_tool = Some(name.to_string());
                    }
                }
                _ => {}
            }
        }
        if !any_tool_use {
            *last_tool = None;
        }
    }
}

fn truncate(s: &str, n: usize) -> String {
    let single = s.replace('\n', " ");
    if single.chars().count() <= n {
        return single;
    }
    let mut out: String = single.chars().take(n).collect();
    out.push('…');
    out
}

fn tail_lines(path: &Path, n: usize) -> Option<Vec<String>> {
    let mut f = std::fs::File::open(path).ok()?;
    let len = f.metadata().ok()?.len();
    let chunk: usize = 64 * 1024;
    let read_len = std::cmp::min(len, chunk as u64);
    let start = len.saturating_sub(read_len);
    f.seek(SeekFrom::Start(start)).ok()?;
    let mut buf = vec![0u8; read_len as usize];
    f.read_exact(&mut buf).ok()?;
    let s = String::from_utf8_lossy(&buf);
    let mut lines: Vec<String> = s.lines().map(|l| l.to_string()).collect();
    if start > 0 && !lines.is_empty() {
        lines.remove(0);
    }
    if lines.len() > n {
        let skip = lines.len() - n;
        lines = lines.into_iter().skip(skip).collect();
    }
    Some(lines)
}

/// Decide whether a (name, exe) pair looks like the Claude Code CLI and NOT
/// Anthropic's Claude Desktop app. The desktop binary lives inside a `.app`
/// bundle on macOS (`/Applications/Claude.app/Contents/MacOS/Claude`) which
/// lowercases to a path containing `claude.app/contents/`. We also require the
/// process name to be exactly `claude` (the CLI installs as `~/.local/bin/claude`).
fn looks_like_claude_cli(name: &str, exe: &str) -> bool {
    if exe.contains("claude.app/contents/")
        || exe.contains("claude desktop")
        || exe.contains("claude.exe ")
    {
        return false;
    }
    name == "claude"
        || name == "claude-code"
        || exe.ends_with("/.local/bin/claude")
        || exe.ends_with("/bin/claude-code")
}

fn find_claude_process(project_path: &Path, snap: &ProcessSnapshot) -> Option<u32> {
    // Canonicalize the project path once so symlinked cwds can match.
    let project_canon =
        std::fs::canonicalize(project_path).unwrap_or_else(|_| project_path.to_path_buf());
    for (pid, name, exe, cwd) in snap.procs.iter() {
        if !looks_like_claude_cli(name, exe) {
            continue;
        }
        if cwd.as_os_str().is_empty() {
            continue;
        }
        if cwd == &project_canon || cwd.starts_with(&project_canon) {
            return Some(*pid);
        }
        // Symlinked cwd: sysinfo may report the user's typed path (e.g. via a
        // symlink) while project_path was canonicalized at add-time. Try
        // canonicalizing the cwd before giving up.
        if let Ok(cwd_canon) = std::fs::canonicalize(cwd) {
            if cwd_canon == project_canon || cwd_canon.starts_with(&project_canon) {
                return Some(*pid);
            }
        }
    }
    None
}

/// Resolve the pid of the claude process backing a session displayed under
/// `instance_path`, searching the whole project subtree.
///
/// "Open terminal" needs the process even when the session was launched in an
/// ancestor directory (e.g. the repo root) but is now shown under a nested
/// worktree instance: there `find_claude_process(instance)` returns `None`
/// because the process cwd (the root) isn't under the worktree. We then look for
/// the claude process whose cwd is the closest ancestor of the instance within
/// the project, falling back to the sole claude process in the project if
/// there's exactly one.
pub fn find_session_process(
    instance_path: &Path,
    project_root: &Path,
    snap: &ProcessSnapshot,
) -> Option<u32> {
    resolve_session_pid(&snap.procs, instance_path, project_root)
}

/// Pure core of [`find_session_process`], testable without a `ProcessSnapshot`.
/// Priority: (1) a claude process rooted at/under the instance, (2) the claude
/// process whose cwd is the closest ancestor of the instance within the project,
/// (3) the sole claude process in the project if there's exactly one.
fn resolve_session_pid(
    procs: &[(u32, String, String, PathBuf)],
    instance_path: &Path,
    project_root: &Path,
) -> Option<u32> {
    let canon = |p: &Path| std::fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf());
    let instance_canon = canon(instance_path);
    let root_canon = canon(project_root);

    let mut best_ancestor: Option<(usize, u32)> = None;
    let mut in_project: Vec<u32> = Vec::new();

    for (pid, name, exe, cwd) in procs.iter() {
        if !looks_like_claude_cli(name, exe) || cwd.as_os_str().is_empty() {
            continue;
        }
        let cwd_canon = canon(cwd);
        // Fast path: a claude process rooted at (or under) the instance itself.
        if cwd_canon == instance_canon || cwd_canon.starts_with(&instance_canon) {
            return Some(*pid);
        }
        if !cwd_canon.starts_with(&root_canon) {
            continue;
        }
        in_project.push(*pid);
        // Prefer a process whose cwd is an ancestor of the instance (the session
        // was launched above the worktree); pick the closest such ancestor.
        if instance_canon.starts_with(&cwd_canon) {
            let depth = cwd_canon.components().count();
            if best_ancestor.is_none_or(|(d, _)| depth > d) {
                best_ancestor = Some((depth, *pid));
            }
        }
    }

    if let Some((_, pid)) = best_ancestor {
        return Some(pid);
    }
    // Last resort: exactly one claude session in the whole project — unambiguous.
    if in_project.len() == 1 {
        return Some(in_project[0]);
    }
    None
}

#[tauri::command]
pub fn agent_status(
    caches: tauri::State<crate::caches::SharedCaches>,
    path: String,
) -> AgentStatus {
    let snap = caches.snapshot_processes();
    let p = Path::new(&path);
    let transcripts = scan_transcripts(p);
    let best = pick_best(&transcripts, p);
    build_agent_status(p, best, &snap)
}

/// Helper used in single-instance lookup (no other instances to defer to).
fn pick_best<'a>(
    transcripts: &'a [TranscriptSummary],
    instance_path: &Path,
) -> Option<&'a TranscriptSummary> {
    transcripts
        .iter()
        .filter(|t| matches_instance(t, instance_path))
        .max_by_key(|t| t.mtime)
}

/// Extract a chronological list of recent messages from a transcript file.
/// Drops user messages that contain only tool_results (i.e. no actual user prose).
pub fn read_recent_messages(path: &Path, limit: usize) -> Vec<TranscriptMessage> {
    let want_lines = std::cmp::max(limit * 4, 128);
    let Some(lines) = tail_lines(path, want_lines) else {
        return vec![];
    };
    parse_messages_from_lines(&lines, limit)
}

/// Pure parser: given JSONL transcript lines, return the most recent up-to-`limit` messages.
pub fn parse_messages_from_lines(lines: &[String], limit: usize) -> Vec<TranscriptMessage> {
    // Pre-scan: collect every tool_use id that has a matching tool_result later
    // in the window, so we can tell whether an interactive prompt was answered.
    let answered = collect_answered_tool_use_ids(lines);

    let mut out: Vec<TranscriptMessage> = Vec::with_capacity(lines.len());
    for line in lines.iter() {
        let Ok(entry) = serde_json::from_str::<TranscriptEntry>(line) else {
            continue;
        };
        let role_raw = entry
            .role
            .clone()
            .or_else(|| {
                entry
                    .message
                    .as_ref()
                    .and_then(|m| m.get("role"))
                    .and_then(|v| v.as_str().map(String::from))
            })
            .or(entry.entry_type.clone())
            .unwrap_or_default()
            .to_lowercase();
        let role = match role_raw.as_str() {
            "user" => MessageRole::User,
            "assistant" => MessageRole::Assistant,
            _ => continue,
        };
        let timestamp = entry
            .timestamp
            .as_deref()
            .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
            .map(|d| d.with_timezone(&Utc));

        let (text, tools, only_tool_results) = extract_message_parts(&entry, &answered);

        // Skip noise: user entries that are only tool_results
        if role == MessageRole::User && text.is_none() && only_tool_results {
            continue;
        }
        // Skip entirely empty entries
        if text.is_none() && tools.is_empty() {
            continue;
        }

        out.push(TranscriptMessage {
            timestamp,
            role,
            text,
            tool_uses: tools,
        });
    }
    if out.len() > limit {
        let skip = out.len() - limit;
        out.drain(0..skip);
    }
    out
}

/// Scan every line for user-side `tool_result` items and return the set of
/// `tool_use_id`s they answer. Used to tell whether an interactive prompt
/// (AskUserQuestion / ExitPlanMode) is still awaiting the user.
fn collect_answered_tool_use_ids(lines: &[String]) -> HashSet<String> {
    let mut answered = HashSet::new();
    for line in lines.iter() {
        let Ok(entry) = serde_json::from_str::<TranscriptEntry>(line) else {
            continue;
        };
        let Some(value) = entry.message.as_ref().or(entry.content.as_ref()) else {
            continue;
        };
        let content = value.get("content").unwrap_or(value);
        let Some(arr) = content.as_array() else {
            continue;
        };
        for item in arr {
            if item.get("type").and_then(|v| v.as_str()) == Some("tool_result") {
                if let Some(id) = item.get("tool_use_id").and_then(|v| v.as_str()) {
                    answered.insert(id.to_string());
                }
            }
        }
    }
    answered
}

/// Build the pending-interaction payload for an interactive `tool_use`, unless
/// it was already answered (its id appears in `answered`).
fn pending_interaction(
    name: &str,
    id: Option<&str>,
    input: Option<&serde_json::Value>,
    answered: &HashSet<String>,
) -> Option<PendingInteraction> {
    if id.map(|i| answered.contains(i)).unwrap_or(false) {
        return None;
    }
    match name {
        "AskUserQuestion" => parse_ask_user_question(input),
        "ExitPlanMode" => parse_exit_plan_mode(input),
        _ => None,
    }
}

fn parse_ask_user_question(input: Option<&serde_json::Value>) -> Option<PendingInteraction> {
    let qs = input?.get("questions")?.as_array()?;
    let questions: Vec<PendingQuestion> = qs
        .iter()
        .filter_map(|q| {
            let question = q.get("question")?.as_str()?.to_string();
            let header = q.get("header").and_then(|v| v.as_str()).map(str::to_string);
            let multi_select = q
                .get("multiSelect")
                .and_then(|v| v.as_bool())
                .unwrap_or(false);
            let options = q
                .get("options")
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|o| {
                            Some(QuestionOption {
                                label: o.get("label")?.as_str()?.to_string(),
                                description: o
                                    .get("description")
                                    .and_then(|v| v.as_str())
                                    .map(str::to_string),
                            })
                        })
                        .collect()
                })
                .unwrap_or_default();
            Some(PendingQuestion {
                header,
                question,
                multi_select,
                options,
            })
        })
        .collect();
    (!questions.is_empty()).then_some(PendingInteraction::Question { questions })
}

fn parse_exit_plan_mode(input: Option<&serde_json::Value>) -> Option<PendingInteraction> {
    let plan = input?.get("plan")?.as_str()?.to_string();
    Some(PendingInteraction::PlanApproval { plan })
}

fn extract_message_parts(
    entry: &TranscriptEntry,
    answered: &HashSet<String>,
) -> (Option<String>, Vec<ToolUseEntry>, bool) {
    let mut text: Option<String> = None;
    let mut tools: Vec<ToolUseEntry> = Vec::new();
    let mut only_tool_results = true;
    let mut saw_any_part = false;

    let Some(value) = entry.message.as_ref().or(entry.content.as_ref()) else {
        return (None, tools, false);
    };
    let content = value.get("content").unwrap_or(value);

    if let Some(s) = content.as_str() {
        let trimmed = s.trim();
        if !trimmed.is_empty() {
            text = Some(truncate(trimmed, 600));
            only_tool_results = false;
            saw_any_part = true;
        }
    } else if let Some(arr) = content.as_array() {
        for item in arr {
            let item_type = item.get("type").and_then(|v| v.as_str()).unwrap_or("");
            saw_any_part = true;
            match item_type {
                "text" => {
                    if let Some(t) = item.get("text").and_then(|v| v.as_str()) {
                        let trimmed = t.trim();
                        if !trimmed.is_empty() {
                            // Append if multiple text parts; cap final size.
                            text = Some(match text {
                                Some(prev) => format!("{prev}\n\n{trimmed}"),
                                None => trimmed.to_string(),
                            });
                            only_tool_results = false;
                        }
                    }
                }
                "tool_use" => {
                    only_tool_results = false;
                    let name = item
                        .get("name")
                        .and_then(|v| v.as_str())
                        .unwrap_or("Tool")
                        .to_string();
                    let id = item.get("id").and_then(|v| v.as_str()).map(str::to_string);
                    let detail = tool_detail(&name, item.get("input"));
                    let pending =
                        pending_interaction(&name, id.as_deref(), item.get("input"), answered);
                    tools.push(ToolUseEntry {
                        name,
                        detail,
                        tool_use_id: id,
                        pending,
                    });
                }
                "tool_result" => {
                    // user-side tool_results — keep only_tool_results truthy
                }
                _ => {
                    only_tool_results = false;
                }
            }
        }
    }

    // Cap any accumulated text to avoid runaway payloads in the UI.
    if let Some(t) = text.as_mut() {
        if t.chars().count() > 1200 {
            *t = t.chars().take(1200).collect::<String>() + "…";
        }
    }

    let _ = saw_any_part;
    (text, tools, only_tool_results)
}

fn tool_detail(name: &str, input: Option<&serde_json::Value>) -> Option<String> {
    let input = input?;
    let pick = |key: &str| {
        input
            .get(key)
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
    };
    let short_path = |p: String| {
        if let Some(home) = dirs::home_dir() {
            let home_s = home.to_string_lossy().into_owned();
            if let Some(rest) = p.strip_prefix(&home_s) {
                return format!("~{rest}");
            }
        }
        p
    };
    match name {
        "Read" | "Write" | "Edit" | "NotebookEdit" => pick("file_path").map(short_path),
        "Bash" => pick("command").map(|c| truncate(&c, 80)),
        "Grep" => pick("pattern"),
        "Glob" => pick("pattern"),
        "WebFetch" => pick("url"),
        "WebSearch" => pick("query"),
        "Task" | "Agent" => pick("description"),
        "AskUserQuestion" => input
            .get("questions")
            .and_then(|q| q.as_array())
            .and_then(|a| a.first())
            .and_then(|q| q.get("header").or_else(|| q.get("question")))
            .and_then(|v| v.as_str())
            .map(|s| truncate(s, 60)),
        "ExitPlanMode" => Some("Review plan".to_string()),
        _ => None,
    }
}

pub fn matches_instance(t: &TranscriptSummary, instance_path: &Path) -> bool {
    match t.cwd.as_ref() {
        Some(cwd) => cwd == instance_path || cwd.starts_with(instance_path),
        // No cwd metadata in the tail (very old transcripts) — fall back to
        // checking whether the transcript directory matches the instance encode.
        None => {
            let dir = t.file_path.parent();
            let expected = crate::claude_path::project_transcripts_dir(instance_path);
            match (dir, expected) {
                (Some(d), Some(e)) => d == e,
                _ => false,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn claude(pid: u32, cwd: &str) -> (u32, String, String, PathBuf) {
        (
            pid,
            "claude".to_string(),
            "/Users/x/.local/bin/claude".to_string(),
            PathBuf::from(cwd),
        )
    }

    #[test]
    fn session_pid_fast_path_at_instance() {
        let procs = vec![claude(100, "/repo")];
        // Instance is the repo root; claude runs there.
        assert_eq!(
            resolve_session_pid(&procs, Path::new("/repo"), Path::new("/repo")),
            Some(100)
        );
    }

    #[test]
    fn session_pid_falls_back_to_ancestor_for_nested_worktree() {
        // The fusily scenario: claude runs at the repo root, but the session is
        // displayed under a nested worktree instance with no process of its own.
        let root = "/repo";
        let worktree = "/repo/.claude/worktrees/feat/backend";
        let procs = vec![
            claude(100, root),     // the real session, at the root
            claude(200, "/other"), // unrelated claude outside the project
        ];
        assert_eq!(
            resolve_session_pid(&procs, Path::new(worktree), Path::new(root)),
            Some(100)
        );
    }

    #[test]
    fn session_pid_prefers_closest_ancestor() {
        let root = "/repo";
        let worktree = "/repo/a/b/wt";
        let procs = vec![
            claude(100, "/repo"),     // ancestor (far)
            claude(101, "/repo/a/b"), // ancestor (closer) — should win
        ];
        assert_eq!(
            resolve_session_pid(&procs, Path::new(worktree), Path::new(root)),
            Some(101)
        );
    }

    #[test]
    fn session_pid_is_none_when_ambiguous_and_no_ancestor() {
        // Two sibling worktree sessions, neither an ancestor of the target, and
        // more than one candidate in the project -> refuse to guess.
        let root = "/repo";
        let target = "/repo/wt-a";
        let procs = vec![claude(100, "/repo/wt-b"), claude(101, "/repo/wt-c")];
        assert_eq!(
            resolve_session_pid(&procs, Path::new(target), Path::new(root)),
            None
        );
    }

    #[test]
    fn session_pid_sole_project_claude_is_used() {
        // Exactly one claude in the project, not an ancestor -> unambiguous, use it.
        let root = "/repo";
        let target = "/repo/wt-a";
        let procs = vec![claude(100, "/repo/wt-b"), claude(200, "/elsewhere")];
        assert_eq!(
            resolve_session_pid(&procs, Path::new(target), Path::new(root)),
            Some(100)
        );
    }

    fn assistant_text_line(text: &str) -> String {
        format!(
            r#"{{"type":"assistant","timestamp":"2026-05-24T08:00:00Z","cwd":"/p","message":{{"role":"assistant","content":[{{"type":"text","text":"{}"}}]}}}}"#,
            text
        )
    }

    fn assistant_tool_line(tool: &str, file: &str) -> String {
        format!(
            r#"{{"type":"assistant","timestamp":"2026-05-24T08:01:00Z","cwd":"/p","message":{{"role":"assistant","content":[{{"type":"tool_use","name":"{}","input":{{"file_path":"{}"}}}}]}}}}"#,
            tool, file
        )
    }

    fn user_text_line(text: &str) -> String {
        format!(
            r#"{{"type":"user","timestamp":"2026-05-24T08:02:00Z","cwd":"/p","message":{{"role":"user","content":[{{"type":"text","text":"{}"}}]}}}}"#,
            text
        )
    }

    fn user_tool_result_line() -> String {
        r#"{"type":"user","timestamp":"2026-05-24T08:03:00Z","cwd":"/p","message":{"role":"user","content":[{"type":"tool_result","content":"ok"}]}}"#.to_string()
    }

    #[test]
    fn parses_mixed_messages() {
        let lines = vec![
            user_text_line("hello"),
            assistant_text_line("hi there"),
            assistant_tool_line("Read", "/p/a.rs"),
            user_tool_result_line(), // should be filtered
            assistant_text_line("done"),
        ];
        let got = parse_messages_from_lines(&lines, 10);
        assert_eq!(got.len(), 4, "user tool_result should be filtered");
        assert_eq!(got[0].role, MessageRole::User);
        assert_eq!(got[0].text.as_deref(), Some("hello"));
        assert_eq!(got[1].role, MessageRole::Assistant);
        assert_eq!(got[1].text.as_deref(), Some("hi there"));
        assert_eq!(got[2].role, MessageRole::Assistant);
        assert_eq!(got[2].tool_uses.len(), 1);
        assert_eq!(got[2].tool_uses[0].name, "Read");
        assert_eq!(got[3].text.as_deref(), Some("done"));
    }

    #[test]
    fn respects_limit() {
        let lines: Vec<String> = (0..20)
            .map(|i| assistant_text_line(&format!("msg{i}")))
            .collect();
        let got = parse_messages_from_lines(&lines, 5);
        assert_eq!(got.len(), 5);
        assert_eq!(got[0].text.as_deref(), Some("msg15"));
        assert_eq!(got[4].text.as_deref(), Some("msg19"));
    }

    #[test]
    fn extracts_tool_detail_for_read() {
        let input = serde_json::json!({"file_path": "/Users/g/p/src/lib.rs"});
        let d = tool_detail("Read", Some(&input));
        assert!(d.is_some());
        assert!(d.unwrap().ends_with("/src/lib.rs"));
    }

    #[test]
    fn extracts_tool_detail_for_bash() {
        let input = serde_json::json!({"command": "ls -la /tmp/some/dir"});
        let d = tool_detail("Bash", Some(&input)).unwrap();
        assert!(d.contains("ls -la"));
    }

    #[test]
    fn returns_none_for_unknown_tool() {
        let input = serde_json::json!({"foo": "bar"});
        assert!(tool_detail("UnknownTool", Some(&input)).is_none());
    }

    #[test]
    fn shortens_home_prefix_in_paths() {
        let home = dirs::home_dir().unwrap().to_string_lossy().into_owned();
        let input = serde_json::json!({"file_path": format!("{home}/dev/p/file.rs")});
        let d = tool_detail("Edit", Some(&input)).unwrap();
        assert!(d.starts_with("~/"), "expected ~/ prefix, got {d}");
    }

    fn ask_question_line(id: &str) -> String {
        format!(
            r#"{{"type":"assistant","timestamp":"2026-05-24T08:01:00Z","cwd":"/p","message":{{"role":"assistant","content":[{{"type":"tool_use","id":"{id}","name":"AskUserQuestion","input":{{"questions":[{{"question":"Pick one?","header":"Choice","multiSelect":false,"options":[{{"label":"A","description":"first"}},{{"label":"B","description":"second"}}]}}]}}}}]}}}}"#
        )
    }

    fn tool_result_line(id: &str) -> String {
        format!(
            r#"{{"type":"user","timestamp":"2026-05-24T08:03:00Z","cwd":"/p","message":{{"role":"user","content":[{{"type":"tool_result","tool_use_id":"{id}","content":"A"}}]}}}}"#
        )
    }

    #[test]
    fn parses_ask_user_question_payload() {
        let input = serde_json::json!({
            "questions": [{
                "question": "Pick one?",
                "header": "Choice",
                "multiSelect": true,
                "options": [
                    {"label": "A", "description": "first"},
                    {"label": "B", "description": null}
                ]
            }]
        });
        let p = parse_ask_user_question(Some(&input)).unwrap();
        let PendingInteraction::Question { questions } = p else {
            panic!("expected Question");
        };
        assert_eq!(questions.len(), 1);
        assert_eq!(questions[0].question, "Pick one?");
        assert_eq!(questions[0].header.as_deref(), Some("Choice"));
        assert!(questions[0].multi_select);
        assert_eq!(questions[0].options.len(), 2);
        assert_eq!(questions[0].options[0].label, "A");
        assert_eq!(
            questions[0].options[0].description.as_deref(),
            Some("first")
        );
        assert_eq!(questions[0].options[1].description, None);
    }

    #[test]
    fn parses_exit_plan_mode_payload() {
        let input = serde_json::json!({"plan": "# Do the thing\n- step"});
        let p = parse_exit_plan_mode(Some(&input)).unwrap();
        assert!(matches!(
            p,
            PendingInteraction::PlanApproval { plan } if plan.contains("Do the thing")
        ));
    }

    #[test]
    fn unanswered_question_is_pending() {
        let lines = vec![user_text_line("hi"), ask_question_line("toolu_1")];
        let got = parse_messages_from_lines(&lines, 10);
        let q = got.last().unwrap();
        assert_eq!(q.tool_uses.len(), 1);
        assert_eq!(q.tool_uses[0].tool_use_id.as_deref(), Some("toolu_1"));
        assert!(matches!(
            q.tool_uses[0].pending,
            Some(PendingInteraction::Question { .. })
        ));
    }

    #[test]
    fn answered_question_is_not_pending() {
        let lines = vec![
            user_text_line("hi"),
            ask_question_line("toolu_1"),
            tool_result_line("toolu_1"),
        ];
        let got = parse_messages_from_lines(&lines, 10);
        // The question message still renders, but with no pending payload.
        let q = got
            .iter()
            .find(|m| m.tool_uses.iter().any(|t| t.name == "AskUserQuestion"))
            .unwrap();
        assert!(q.tool_uses[0].pending.is_none());
    }
}
