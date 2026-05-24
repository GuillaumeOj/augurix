use crate::caches::SharedCaches;
use crate::proc::{run_capturing, GH_TIMEOUT};
use crate::types::{ChecksRollup, PrState, PrStatus};
use serde::Deserialize;
use std::path::Path;

#[derive(Debug, Deserialize)]
struct GhPr {
    number: u32,
    title: String,
    state: String,
    url: String,
    #[serde(rename = "isDraft", default)]
    is_draft: bool,
    #[serde(rename = "statusCheckRollup", default)]
    status_check_rollup: Vec<GhCheck>,
}

#[derive(Debug, Deserialize)]
struct GhCheck {
    #[serde(default)]
    state: Option<String>,
    #[serde(default)]
    status: Option<String>,
    #[serde(default)]
    conclusion: Option<String>,
}

fn rollup_from_checks(checks: &[GhCheck]) -> ChecksRollup {
    if checks.is_empty() {
        return ChecksRollup::None;
    }
    let mut any_pending = false;
    let mut any_failure = false;
    for c in checks {
        let val = c
            .conclusion
            .clone()
            .or_else(|| c.state.clone())
            .or_else(|| c.status.clone())
            .unwrap_or_default()
            .to_uppercase();
        match val.as_str() {
            "SUCCESS" | "NEUTRAL" | "SKIPPED" => {}
            "FAILURE" | "TIMED_OUT" | "ACTION_REQUIRED" | "CANCELLED" | "STALE" | "ERROR" => {
                any_failure = true;
            }
            "PENDING" | "IN_PROGRESS" | "QUEUED" | "EXPECTED" | "WAITING" | "REQUESTED" => {
                any_pending = true;
            }
            "" => {}
            _ => any_pending = true,
        }
    }
    if any_failure {
        ChecksRollup::Failure
    } else if any_pending {
        ChecksRollup::Pending
    } else {
        ChecksRollup::Success
    }
}

fn parse_state(s: &str, is_draft: bool) -> PrState {
    if is_draft {
        return PrState::Draft;
    }
    match s.to_uppercase().as_str() {
        "OPEN" => PrState::Open,
        "MERGED" => PrState::Merged,
        _ => PrState::Closed,
    }
}

fn has_gh() -> bool {
    let path = match std::env::var_os("PATH") {
        Some(p) => p,
        None => return false,
    };
    for dir in std::env::split_paths(&path) {
        if dir.join("gh").is_file() {
            return true;
        }
        #[cfg(windows)]
        if dir.join("gh.exe").is_file() {
            return true;
        }
    }
    false
}

fn gh_query(path: &Path) -> Option<PrStatus> {
    let raw = run_capturing(
        "gh",
        &[
            "pr",
            "view",
            "--json",
            "number,title,state,url,isDraft,statusCheckRollup",
        ],
        path,
        GH_TIMEOUT,
    )
    .ok()?;
    let pr: GhPr = serde_json::from_str(&raw).ok()?;
    Some(PrStatus {
        number: pr.number,
        title: pr.title,
        state: parse_state(&pr.state, pr.is_draft),
        checks: rollup_from_checks(&pr.status_check_rollup),
        url: pr.url,
        is_draft: pr.is_draft,
    })
}

/// Cached PR lookup. Branch is part of the cache key so a checkout invalidates naturally.
pub fn collect_pr(caches: &SharedCaches, path: &Path, branch: Option<&str>) -> Option<PrStatus> {
    let branch = branch.unwrap_or("");
    if let Some(cached) = caches.get_pr(path, branch) {
        return cached;
    }
    let started = caches.pr_fetch_started();
    if !has_gh() {
        caches.set_pr(path, branch, None, started);
        return None;
    }
    let value = gh_query(path);
    caches.set_pr(path, branch, value.clone(), started);
    value
}

#[tauri::command]
pub fn pr_status(caches: tauri::State<SharedCaches>, path: String) -> Option<PrStatus> {
    collect_pr(&caches, Path::new(&path), None)
}
