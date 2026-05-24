use crate::proc::{run_capturing, GIT_TIMEOUT};
use crate::types::GitStatus;
use std::path::Path;

fn run_git(cwd: &Path, args: &[&str]) -> Result<String, String> {
    run_capturing("git", args, cwd, GIT_TIMEOUT)
}

fn is_git_repo(path: &Path) -> bool {
    run_git(path, &["rev-parse", "--is-inside-work-tree"]).is_ok()
}

/// Return the absolute toplevel of the git repository containing `path`, or `None`.
pub fn repo_root(path: &Path) -> Option<std::path::PathBuf> {
    let s = run_git(path, &["rev-parse", "--show-toplevel"]).ok()?;
    let s = s.trim();
    if s.is_empty() {
        return None;
    }
    let p = std::path::PathBuf::from(s);
    std::fs::canonicalize(&p).ok().or(Some(p))
}

pub fn collect_status(path: &Path) -> GitStatus {
    if !is_git_repo(path) {
        return GitStatus::not_a_repo();
    }
    let raw = match run_git(path, &["status", "--porcelain=v2", "--branch"]) {
        Ok(s) => s,
        Err(_) => {
            return GitStatus {
                is_repo: true,
                ..GitStatus::not_a_repo()
            }
        }
    };
    parse_porcelain_v2(&raw)
}

/// Pure parser for `git status --porcelain=v2 --branch` output.
pub fn parse_porcelain_v2(raw: &str) -> GitStatus {
    let mut status = GitStatus {
        is_repo: true,
        branch: None,
        upstream: None,
        ahead: 0,
        behind: 0,
        modified: 0,
        staged: 0,
        untracked: 0,
        conflicted: 0,
    };

    for line in raw.lines() {
        if let Some(rest) = line.strip_prefix("# branch.head ") {
            status.branch = Some(rest.trim().to_string());
        } else if let Some(rest) = line.strip_prefix("# branch.upstream ") {
            status.upstream = Some(rest.trim().to_string());
        } else if let Some(rest) = line.strip_prefix("# branch.ab ") {
            let mut parts = rest.split_whitespace();
            if let Some(a) = parts.next() {
                status.ahead = a.trim_start_matches('+').parse().unwrap_or(0);
            }
            if let Some(b) = parts.next() {
                status.behind = b.trim_start_matches('-').parse().unwrap_or(0);
            }
        } else if line.starts_with('?') {
            status.untracked += 1;
        } else if line.starts_with('u') {
            status.conflicted += 1;
        } else if line.starts_with('1') || line.starts_with('2') {
            let xy = line.split_whitespace().nth(1).unwrap_or("..");
            let mut chars = xy.chars();
            let staged_ch = chars.next().unwrap_or('.');
            let unstaged_ch = chars.next().unwrap_or('.');
            if staged_ch != '.' {
                status.staged += 1;
            }
            if unstaged_ch != '.' {
                status.modified += 1;
            }
        }
    }
    status
}

#[tauri::command]
pub fn git_status(path: String) -> GitStatus {
    collect_status(Path::new(&path))
}

#[tauri::command]
pub fn git_diff(path: String, scope: String) -> Result<String, String> {
    let cwd = Path::new(&path);
    match scope.as_str() {
        "staged" => run_git(cwd, &["diff", "--staged", "--no-color"]),
        "unstaged" => run_git(cwd, &["diff", "--no-color"]),
        "untracked" => collect_untracked_diff(cwd),
        other => Err(format!("unknown diff scope: {other}")),
    }
}

const UNTRACKED_FILE_BYTES_CAP: usize = 256 * 1024;
const UNTRACKED_TOTAL_BYTES_CAP: usize = 4 * 1024 * 1024;
const UNTRACKED_FILE_COUNT_CAP: usize = 500;

fn collect_untracked_diff(cwd: &Path) -> Result<String, String> {
    let raw = run_git(cwd, &["ls-files", "--others", "--exclude-standard"])?;
    let mut out = String::new();
    let mut files_seen = 0usize;
    let mut files_emitted = 0usize;
    for rel in raw.lines() {
        if rel.is_empty() {
            continue;
        }
        files_seen += 1;

        if out.len() >= UNTRACKED_TOTAL_BYTES_CAP {
            // Stop appending; record what was skipped and break.
            let remaining = raw.lines().filter(|l| !l.is_empty()).count() - files_emitted;
            out.push_str(&format!(
                "diff --git a/.augurix-truncated b/.augurix-truncated\n\
                 new file mode 100644\n\
                 --- /dev/null\n\
                 +++ b/.augurix-truncated\n\
                 @@ -0,0 +1 @@\n\
                 +(untracked diff truncated at {} bytes; {} more file(s) not shown)\n",
                UNTRACKED_TOTAL_BYTES_CAP, remaining
            ));
            break;
        }
        if files_emitted >= UNTRACKED_FILE_COUNT_CAP {
            let remaining = raw.lines().filter(|l| !l.is_empty()).count() - files_emitted;
            out.push_str(&format!(
                "diff --git a/.augurix-truncated b/.augurix-truncated\n\
                 new file mode 100644\n\
                 --- /dev/null\n\
                 +++ b/.augurix-truncated\n\
                 @@ -0,0 +1 @@\n\
                 +(stopped after {} files; {} more not shown — add to .gitignore)\n",
                UNTRACKED_FILE_COUNT_CAP, remaining
            ));
            break;
        }

        let abs = cwd.join(rel);
        let metadata = match std::fs::metadata(&abs) {
            Ok(m) => m,
            Err(_) => continue,
        };
        if !metadata.is_file() {
            continue;
        }
        out.push_str(&format!("diff --git a/{rel} b/{rel}\n"));
        out.push_str("new file mode 100644\n");
        out.push_str("--- /dev/null\n");
        out.push_str(&format!("+++ b/{rel}\n"));
        files_emitted += 1;
        let size = metadata.len();
        if size > UNTRACKED_FILE_BYTES_CAP as u64 {
            out.push_str(&format!(
                "@@ -0,0 +1 @@\n+(file too large to preview: {} bytes)\n",
                size
            ));
            continue;
        }
        let bytes = match std::fs::read(&abs) {
            Ok(b) => b,
            Err(_) => continue,
        };
        if is_likely_binary(&bytes) {
            out.push_str("Binary file (untracked)\n");
            continue;
        }
        let text = match std::str::from_utf8(&bytes) {
            Ok(t) => t,
            Err(_) => {
                out.push_str("Binary file (untracked)\n");
                continue;
            }
        };
        let line_count = text.split_inclusive('\n').count().max(1);
        out.push_str(&format!("@@ -0,0 +1,{line_count} @@\n"));
        for line in text.split_inclusive('\n') {
            out.push('+');
            out.push_str(line.trim_end_matches('\n'));
            out.push('\n');
        }
    }
    let _ = files_seen;
    Ok(out)
}

fn is_likely_binary(bytes: &[u8]) -> bool {
    let sample = &bytes[..bytes.len().min(8000)];
    sample.contains(&0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_clean_repo() {
        let raw = "# branch.oid abc123\n\
                   # branch.head main\n\
                   # branch.upstream origin/main\n\
                   # branch.ab +0 -0\n";
        let s = parse_porcelain_v2(raw);
        assert!(s.is_repo);
        assert_eq!(s.branch.as_deref(), Some("main"));
        assert_eq!(s.upstream.as_deref(), Some("origin/main"));
        assert_eq!(s.ahead, 0);
        assert_eq!(s.behind, 0);
        assert_eq!(s.modified, 0);
        assert_eq!(s.staged, 0);
        assert_eq!(s.untracked, 0);
        assert_eq!(s.conflicted, 0);
    }

    #[test]
    fn parses_dirty_repo() {
        let raw = "# branch.head feature/x\n\
                   # branch.ab +3 -1\n\
                   1 M. N... 100644 100644 100644 aaa bbb file_staged.rs\n\
                   1 .M N... 100644 100644 100644 aaa bbb file_modified.rs\n\
                   1 MM N... 100644 100644 100644 aaa bbb file_both.rs\n\
                   ? untracked.rs\n\
                   ? another.rs\n\
                   u UU N... 100644 100644 100644 100644 aaa bbb ccc conflicted.rs\n";
        let s = parse_porcelain_v2(raw);
        assert_eq!(s.branch.as_deref(), Some("feature/x"));
        assert_eq!(s.ahead, 3);
        assert_eq!(s.behind, 1);
        assert_eq!(s.staged, 2, "M. and MM both have staged change");
        assert_eq!(s.modified, 2, ".M and MM both have unstaged change");
        assert_eq!(s.untracked, 2);
        assert_eq!(s.conflicted, 1);
    }

    #[test]
    fn parses_detached_head() {
        let raw = "# branch.oid abc\n# branch.head (detached)\n";
        let s = parse_porcelain_v2(raw);
        assert_eq!(s.branch.as_deref(), Some("(detached)"));
        assert!(s.upstream.is_none());
    }

    #[test]
    fn rejects_binary_zero_byte() {
        let mut bytes = vec![b'h'; 100];
        bytes[50] = 0;
        assert!(is_likely_binary(&bytes));
    }

    #[test]
    fn accepts_text() {
        let bytes = b"plain text\n with newlines\n";
        assert!(!is_likely_binary(bytes));
    }
}
