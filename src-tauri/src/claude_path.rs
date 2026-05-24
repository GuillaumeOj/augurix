use std::path::{Path, PathBuf};

/// Compute the `~/.claude/projects/<encoded>` directory for an absolute project path.
/// Claude Code encodes the absolute path by replacing path separators with `-`.
pub fn encoded_path(project_path: &Path) -> String {
    let s = project_path.to_string_lossy();
    let mut out = String::with_capacity(s.len());
    for ch in s.chars() {
        if ch == '/' || ch == '\\' {
            out.push('-');
        } else {
            out.push(ch);
        }
    }
    out
}

pub fn claude_projects_root() -> Option<PathBuf> {
    dirs::home_dir().map(|h| h.join(".claude").join("projects"))
}

pub fn project_transcripts_dir(project_path: &Path) -> Option<PathBuf> {
    let root = claude_projects_root()?;
    let encoded = encoded_path(project_path);
    let candidate = root.join(&encoded);
    if candidate.exists() {
        return Some(candidate);
    }
    let alt: String = encoded
        .chars()
        .map(|c| if c == '.' { '-' } else { c })
        .collect();
    let candidate = root.join(alt);
    if candidate.exists() {
        return Some(candidate);
    }
    None
}

/// Pure variant of `decode_to_existing_path` for testing — accepts an `exists` predicate.
#[cfg(test)]
pub fn decode_with<F: Fn(&Path) -> bool>(encoded: &str, exists: F) -> Option<PathBuf> {
    let parts: Vec<&str> = encoded.trim_start_matches('-').split('-').collect();
    let mut prefix = PathBuf::from("/");
    if let Some(resolved) = resolve_segments_with(&prefix, &parts, &exists) {
        return Some(resolved);
    }
    prefix.pop();
    resolve_segments_with(&prefix, &parts, &exists)
}

fn resolve_segments_with<F: Fn(&Path) -> bool>(
    base: &Path,
    segments: &[&str],
    exists: &F,
) -> Option<PathBuf> {
    if segments.is_empty() {
        return if exists(base) {
            Some(base.into())
        } else {
            None
        };
    }
    let max_merge = std::cmp::min(4, segments.len());
    // Try LONGEST merges first so dash-containing path components (`my-repo`)
    // win over the deeper interpretation (`my/repo`) when both exist.
    for take in (1..=max_merge).rev() {
        let component = segments[..take].join("-");
        let candidate = base.join(component);
        if exists(&candidate) {
            if take == segments.len() {
                return Some(candidate);
            }
            if let Some(p) = resolve_segments_with(&candidate, &segments[take..], exists) {
                return Some(p);
            }
        }
    }
    None
}

/// Reverse-encode a directory name from `~/.claude/projects/` to an absolute filesystem
/// path. Returns `None` if the resulting path doesn't exist on disk.
///
/// Claude's encoding is lossy: dashes in original path components are indistinguishable
/// from path separators. We resolve ambiguity by checking which decoding actually exists.
///
/// We deliberately do NOT use a fast all-dashes-to-slashes shortcut: when both
/// `/Users/g/my-repo` and `/Users/g/my/repo` exist on disk, the shortcut would
/// pick the wrong one. The backtracking resolver tries the LONGEST merges first
/// so dash-containing path components win over deeper interpretations.
pub fn decode_to_existing_path(encoded: &str) -> Option<PathBuf> {
    let parts: Vec<&str> = encoded.trim_start_matches('-').split('-').collect();
    let mut prefix = PathBuf::from("/");
    if let Some(resolved) = resolve_segments(&prefix, &parts) {
        return Some(resolved);
    }
    prefix.pop();
    resolve_segments(&prefix, &parts)
}

fn resolve_segments(base: &Path, segments: &[&str]) -> Option<PathBuf> {
    resolve_segments_with(base, segments, &|p: &Path| p.exists())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn encodes_unix_path() {
        let p = Path::new("/Users/guillaume/dev/augurix");
        assert_eq!(encoded_path(p), "-Users-guillaume-dev-augurix");
    }

    #[test]
    fn encodes_root() {
        assert_eq!(encoded_path(Path::new("/")), "-");
    }

    fn fs(paths: &[&str]) -> impl Fn(&Path) -> bool {
        let set: HashSet<PathBuf> = paths.iter().map(PathBuf::from).collect();
        move |p: &Path| set.contains(p)
    }

    #[test]
    fn decodes_simple_path() {
        let exists = fs(&["/Users", "/Users/g", "/Users/g/dev", "/Users/g/dev/augurix"]);
        let got = decode_with("-Users-g-dev-augurix", exists);
        assert_eq!(got, Some(PathBuf::from("/Users/g/dev/augurix")));
    }

    #[test]
    fn decodes_subdirectory_under_project() {
        let exists = fs(&[
            "/Users",
            "/Users/g",
            "/Users/g/dev",
            "/Users/g/dev/fusily",
            "/Users/g/dev/fusily/fusily",
            "/Users/g/dev/fusily/fusily/frontend",
        ]);
        let got = decode_with("-Users-g-dev-fusily-fusily-frontend", exists);
        assert_eq!(
            got,
            Some(PathBuf::from("/Users/g/dev/fusily/fusily/frontend"))
        );
    }

    #[test]
    fn returns_none_when_nothing_matches() {
        let exists = fs(&["/Users", "/Users/g"]);
        let got = decode_with("-Users-g-nonexistent-dir", exists);
        assert_eq!(got, None);
    }

    #[test]
    fn prefers_dash_in_name_over_subdirectory_when_both_exist() {
        // /Users/g/my-repo AND /Users/g/my/repo both exist on disk; the encoded
        // dir name is ambiguous. Prefer the longer single component.
        let exists = fs(&[
            "/Users",
            "/Users/g",
            "/Users/g/my",
            "/Users/g/my/repo",
            "/Users/g/my-repo",
        ]);
        let got = decode_with("-Users-g-my-repo", exists);
        assert_eq!(got, Some(PathBuf::from("/Users/g/my-repo")));
    }

    #[test]
    fn falls_back_to_subdirectory_when_only_that_exists() {
        let exists = fs(&["/Users", "/Users/g", "/Users/g/my", "/Users/g/my/repo"]);
        let got = decode_with("-Users-g-my-repo", exists);
        assert_eq!(got, Some(PathBuf::from("/Users/g/my/repo")));
    }
}
