use crate::claude_path::{claude_projects_root, decode_to_existing_path};
use crate::proc::{run_capturing, GIT_TIMEOUT};
use crate::types::{Instance, InstanceKind, Project};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

const SUB_PROJECT_LOOKBACK: Duration = Duration::from_secs(30 * 24 * 60 * 60);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorktreeRecord {
    pub path: PathBuf,
    pub branch: Option<String>,
    pub is_main: bool,
}

fn list_worktrees(repo_path: &Path) -> Vec<WorktreeRecord> {
    match run_capturing(
        "git",
        &["worktree", "list", "--porcelain"],
        repo_path,
        GIT_TIMEOUT,
    ) {
        Ok(raw) => parse_worktree_porcelain(&raw),
        Err(_) => vec![],
    }
}

/// Pure parser for `git worktree list --porcelain` output.
pub fn parse_worktree_porcelain(raw: &str) -> Vec<WorktreeRecord> {
    let mut out: Vec<WorktreeRecord> = vec![];
    let mut cur_path: Option<PathBuf> = None;
    let mut cur_branch: Option<String> = None;
    let mut is_main = true;
    for line in raw.lines() {
        if line.is_empty() {
            if let Some(p) = cur_path.take() {
                out.push(WorktreeRecord {
                    path: p,
                    branch: cur_branch.take(),
                    is_main,
                });
                is_main = false;
            }
            continue;
        }
        if let Some(rest) = line.strip_prefix("worktree ") {
            cur_path = Some(PathBuf::from(rest));
        } else if let Some(rest) = line.strip_prefix("branch ") {
            cur_branch = Some(rest.trim_start_matches("refs/heads/").to_string());
        }
    }
    if let Some(p) = cur_path {
        out.push(WorktreeRecord {
            path: p,
            branch: cur_branch,
            is_main,
        });
    }
    out
}

pub(crate) fn label_for(project_root: &Path, path: &Path) -> String {
    if path == project_root {
        return ".".to_string();
    }
    if let Ok(rel) = path.strip_prefix(project_root) {
        return rel.to_string_lossy().into_owned();
    }
    // Outside project root — show with `~` if under HOME, else absolute.
    if let Some(home) = dirs::home_dir() {
        if let Ok(rel) = path.strip_prefix(&home) {
            return format!("~/{}", rel.to_string_lossy());
        }
    }
    path.to_string_lossy().into_owned()
}

fn scan_sub_sessions(project_root: &Path) -> Vec<PathBuf> {
    let Some(root) = claude_projects_root() else {
        return vec![];
    };
    if !root.exists() {
        return vec![];
    }
    let now = SystemTime::now();
    let cutoff = now
        .checked_sub(SUB_PROJECT_LOOKBACK)
        .unwrap_or(SystemTime::UNIX_EPOCH);

    let mut out: Vec<PathBuf> = vec![];
    let Ok(dirs) = std::fs::read_dir(&root) else {
        return out;
    };
    for entry in dirs.flatten() {
        let Ok(md) = entry.metadata() else { continue };
        if !md.is_dir() {
            continue;
        }
        let name_os = entry.file_name();
        let Some(encoded) = name_os.to_str() else {
            continue;
        };
        let Some(decoded) = decode_to_existing_path(encoded) else {
            continue;
        };
        let canonical = std::fs::canonicalize(&decoded).unwrap_or(decoded);
        if !canonical.starts_with(project_root) || canonical == project_root {
            continue;
        }
        // Check transcript recency
        let mut recent = false;
        if let Ok(files) = std::fs::read_dir(entry.path()) {
            for f in files.flatten() {
                let p = f.path();
                if p.extension().map(|e| e == "jsonl").unwrap_or(false) {
                    if let Ok(fm) = f.metadata() {
                        if let Ok(t) = fm.modified() {
                            if t >= cutoff {
                                recent = true;
                                break;
                            }
                        }
                    }
                }
            }
        }
        if recent {
            out.push(canonical);
        }
    }
    out
}

pub fn discover(project: &Project) -> Vec<Instance> {
    let mut seen: HashSet<PathBuf> = HashSet::new();
    let mut instances: Vec<Instance> = Vec::new();

    let worktrees = list_worktrees(&project.path);
    let main_path = project.path.clone();
    let mut main_added = false;

    for w in worktrees {
        // For the main worktree we always trust the project root we already
        // canonicalized at add-time; this avoids the "duplicate MainWorktree"
        // failure when canonicalize fails on a temporarily-unmounted disk and
        // the fallback raw path doesn't equal main_path.
        let canonical = if w.is_main {
            main_path.clone()
        } else {
            match std::fs::canonicalize(&w.path) {
                Ok(p) => p,
                Err(_) => {
                    // Linked worktree on missing/unmounted path — skip it
                    // rather than insert a phantom row.
                    continue;
                }
            }
        };
        if !seen.insert(canonical.clone()) {
            continue;
        }
        let kind = if w.is_main || canonical == main_path {
            main_added = true;
            InstanceKind::MainWorktree
        } else {
            InstanceKind::LinkedWorktree
        };
        instances.push(Instance {
            id: crate::types::instance_id(project.id, &canonical),
            project_id: project.id,
            kind,
            label: label_for(&main_path, &canonical),
            path: canonical,
            branch_hint: w.branch,
        });
    }

    if !main_added {
        seen.insert(main_path.clone());
        instances.push(Instance {
            id: crate::types::instance_id(project.id, &main_path),
            project_id: project.id,
            kind: InstanceKind::MainWorktree,
            label: ".".to_string(),
            path: main_path.clone(),
            branch_hint: None,
        });
    }

    // Sub-paths are only legitimate instances if they live in the SAME git repo
    // as the project root. Otherwise (e.g. user added a container directory),
    // we'd surface unrelated child repos as fake sub-instances.
    let project_repo_root = crate::commands::git::repo_root(&main_path);
    for sub in scan_sub_sessions(&main_path) {
        if !seen.insert(sub.clone()) {
            continue;
        }
        let sub_repo_root = crate::commands::git::repo_root(&sub);
        if sub_repo_root != project_repo_root {
            continue;
        }
        instances.push(Instance {
            id: crate::types::instance_id(project.id, &sub),
            project_id: project.id,
            kind: InstanceKind::SubProject,
            label: label_for(&main_path, &sub),
            path: sub,
            branch_hint: None,
        });
    }

    // Sort: main first, then linked worktrees, then sub-projects; alphabetical inside.
    instances.sort_by(|a, b| {
        let ord = |k: &InstanceKind| match k {
            InstanceKind::MainWorktree => 0,
            InstanceKind::LinkedWorktree => 1,
            InstanceKind::SubProject => 2,
        };
        ord(&a.kind)
            .cmp(&ord(&b.kind))
            .then_with(|| a.label.to_lowercase().cmp(&b.label.to_lowercase()))
    });

    instances
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_single_main_worktree() {
        let raw = "worktree /repo
HEAD abc
branch refs/heads/main
";
        let got = parse_worktree_porcelain(raw);
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].path, PathBuf::from("/repo"));
        assert_eq!(got[0].branch.as_deref(), Some("main"));
        assert!(got[0].is_main);
    }

    #[test]
    fn parses_main_plus_linked_worktrees() {
        let raw = "worktree /repo
HEAD abc
branch refs/heads/main

worktree /wt/fix
HEAD def
branch refs/heads/bugfix

worktree /wt/exp
HEAD ghi
detached
";
        let got = parse_worktree_porcelain(raw);
        assert_eq!(got.len(), 3);
        assert!(got[0].is_main);
        assert!(!got[1].is_main);
        assert!(!got[2].is_main);
        assert_eq!(got[1].branch.as_deref(), Some("bugfix"));
        assert!(got[2].branch.is_none());
    }

    #[test]
    fn label_relative_for_subpath() {
        let s = label_for(Path::new("/Users/g/p"), Path::new("/Users/g/p/sub/dir"));
        assert_eq!(s, "sub/dir");
    }

    #[test]
    fn label_dot_for_root() {
        let s = label_for(Path::new("/Users/g/p"), Path::new("/Users/g/p"));
        assert_eq!(s, ".");
    }
}
