use crate::claude_path::{claude_projects_root, decode_to_existing_path};
use crate::store::SharedStore;
use crate::types::DiscoveredProject;
use chrono::{DateTime, Utc};
use std::path::PathBuf;
use tauri::State;

#[tauri::command]
pub fn discover_claude_projects(store: State<SharedStore>) -> Vec<DiscoveredProject> {
    let Some(root) = claude_projects_root() else {
        return vec![];
    };
    if !root.exists() {
        return vec![];
    }
    let already: std::collections::HashSet<PathBuf> =
        store.list().into_iter().map(|p| p.path).collect();

    let mut out: Vec<DiscoveredProject> = Vec::new();
    let Ok(entries) = std::fs::read_dir(&root) else {
        return out;
    };
    for entry in entries.flatten() {
        let Ok(meta) = entry.metadata() else { continue };
        if !meta.is_dir() {
            continue;
        }
        let name_os = entry.file_name();
        let Some(encoded) = name_os.to_str() else {
            continue;
        };
        let Some(path) = decode_to_existing_path(encoded) else {
            continue;
        };
        let canonical = std::fs::canonicalize(&path).unwrap_or(path.clone());

        // Find most-recent transcript modification
        let mut latest: Option<DateTime<Utc>> = None;
        if let Ok(files) = std::fs::read_dir(entry.path()) {
            for f in files.flatten() {
                let p = f.path();
                if p.extension().map(|e| e == "jsonl").unwrap_or(false) {
                    if let Ok(md) = f.metadata() {
                        if let Ok(t) = md.modified() {
                            let dt: DateTime<Utc> = t.into();
                            if latest.map(|cur| dt > cur).unwrap_or(true) {
                                latest = Some(dt);
                            }
                        }
                    }
                }
            }
        }
        if latest.is_none() {
            continue;
        }

        let name = canonical
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or(encoded)
            .to_string();

        out.push(DiscoveredProject {
            path: canonical.clone(),
            name,
            last_session_at: latest,
            already_added: already.contains(&canonical),
        });
    }
    out.sort_by_key(|d| std::cmp::Reverse(d.last_session_at));
    out
}
