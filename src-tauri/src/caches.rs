use crate::types::PrStatus;
use parking_lot::Mutex;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::{Duration, Instant};
use sysinfo::{ProcessRefreshKind, RefreshKind, System};

const PR_CACHE_TTL: Duration = Duration::from_secs(60);
const SYSTEM_TTL: Duration = Duration::from_secs(3);

#[derive(Clone)]
pub struct ProcessSnapshot {
    /// (pid, name, exe, cwd-or-empty)
    pub procs: Arc<Vec<(u32, String, String, PathBuf)>>,
    taken_at: Instant,
}

type PrCacheKey = (PathBuf, String);

struct PrCacheValue {
    value: Option<PrStatus>,
    /// Instant the fetch STARTED — used so a slow stale fetch can't clobber a
    /// faster, newer one when their writes race.
    started_at: Instant,
    /// Instant the fetch COMPLETED — used for TTL freshness checks.
    completed_at: Instant,
}

pub struct Caches {
    pr: Mutex<HashMap<PrCacheKey, PrCacheValue>>,
    /// Snapshot of running processes, plus a tracker of whether a refresh is
    /// in progress so concurrent callers don't all stampede with their own scan.
    system: Mutex<SystemSlot>,
}

#[derive(Default)]
struct SystemSlot {
    cached: Option<ProcessSnapshot>,
    refreshing: bool,
}

impl Caches {
    pub fn new() -> Self {
        Self {
            pr: Mutex::new(HashMap::new()),
            system: Mutex::new(SystemSlot::default()),
        }
    }

    pub fn get_pr(&self, path: &std::path::Path, branch: &str) -> Option<Option<PrStatus>> {
        let key = (path.to_path_buf(), branch.to_string());
        let guard = self.pr.lock();
        let entry = guard.get(&key)?;
        if entry.completed_at.elapsed() < PR_CACHE_TTL {
            Some(entry.value.clone())
        } else {
            None
        }
    }

    /// Mark the start of a PR fetch and return a stamp the caller must pass to
    /// `set_pr` so we can compare ages and reject stale-races.
    pub fn pr_fetch_started(&self) -> Instant {
        Instant::now()
    }

    /// Store a PR result. Only overwrites an existing entry whose own
    /// `started_at` is earlier than ours — guarantees a slow stale fetch can't
    /// clobber a newer fresh one.
    pub fn set_pr(
        &self,
        path: &std::path::Path,
        branch: &str,
        value: Option<PrStatus>,
        started_at: Instant,
    ) {
        let key = (path.to_path_buf(), branch.to_string());
        let mut guard = self.pr.lock();
        if let Some(existing) = guard.get(&key) {
            if existing.started_at >= started_at {
                // A newer fetch already won — drop ours.
                return;
            }
        }
        guard.insert(
            key,
            PrCacheValue {
                value,
                started_at,
                completed_at: Instant::now(),
            },
        );
    }

    /// Return a fresh-enough process snapshot. If another caller is already
    /// refreshing we briefly back off and re-check rather than starting our
    /// own redundant scan; the previous snapshot is returned if no fresh one
    /// arrives within a short backoff window.
    pub fn snapshot_processes(&self) -> ProcessSnapshot {
        loop {
            {
                let mut slot = self.system.lock();
                if let Some(s) = slot.cached.as_ref() {
                    if s.taken_at.elapsed() < SYSTEM_TTL {
                        return s.clone();
                    }
                }
                if !slot.refreshing {
                    slot.refreshing = true;
                    break; // we are the chosen refresher
                }
                // Someone else is refreshing — fall through to back-off.
            }
            std::thread::sleep(Duration::from_millis(15));
            // If after several short waits the other refresher is still going,
            // serve whatever snapshot we have (even if stale) rather than
            // looping forever.
            let slot = self.system.lock();
            if !slot.refreshing {
                continue;
            }
            if let Some(s) = slot.cached.as_ref() {
                return s.clone();
            }
            // No cache yet AND another caller is refreshing — keep waiting.
        }

        // We hold the conceptual "refresh in progress" lease.
        let snap = scan_processes();
        let mut slot = self.system.lock();
        slot.cached = Some(snap.clone());
        slot.refreshing = false;
        snap
    }
}

fn scan_processes() -> ProcessSnapshot {
    let kind = ProcessRefreshKind::new().with_cwd(sysinfo::UpdateKind::Always);
    let mut sys = System::new_with_specifics(RefreshKind::new().with_processes(kind));
    sys.refresh_processes_specifics(sysinfo::ProcessesToUpdate::All, true, kind);
    let mut procs = Vec::with_capacity(sys.processes().len());
    for (pid, proc_) in sys.processes() {
        let name = proc_.name().to_string_lossy().to_lowercase();
        let exe = proc_
            .exe()
            .map(|e| e.to_string_lossy().to_lowercase())
            .unwrap_or_default();
        let cwd = proc_.cwd().map(|c| c.to_path_buf()).unwrap_or_default();
        procs.push((pid.as_u32(), name, exe, cwd));
    }
    ProcessSnapshot {
        procs: Arc::new(procs),
        taken_at: Instant::now(),
    }
}

pub type SharedCaches = Arc<Caches>;

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::Path;

    fn dummy_pr(n: u32) -> PrStatus {
        PrStatus {
            number: n,
            title: format!("pr {n}"),
            state: crate::types::PrState::Open,
            checks: crate::types::ChecksRollup::Success,
            url: format!("https://example.com/{n}"),
            is_draft: false,
        }
    }

    #[test]
    fn newer_fetch_wins_over_older_stale_write() {
        let c = Caches::new();
        let path = Path::new("/repo");
        let branch = "main";
        let t_old = c.pr_fetch_started();
        std::thread::sleep(Duration::from_millis(2));
        let t_new = c.pr_fetch_started();
        // newer fetch finishes FIRST
        c.set_pr(path, branch, Some(dummy_pr(42)), t_new);
        // older fetch finishes second — must not clobber
        c.set_pr(path, branch, Some(dummy_pr(1)), t_old);
        let got = c.get_pr(path, branch).unwrap();
        assert_eq!(got.unwrap().number, 42);
    }

    #[test]
    fn ttl_expires() {
        let c = Caches::new();
        // Use private API by reaching into struct: simulate an old entry.
        let key = (PathBuf::from("/r"), "b".to_string());
        let now = Instant::now();
        c.pr.lock().insert(
            key.clone(),
            PrCacheValue {
                value: Some(dummy_pr(7)),
                started_at: now,
                completed_at: now - PR_CACHE_TTL - Duration::from_secs(1),
            },
        );
        assert!(
            c.get_pr(Path::new("/r"), "b").is_none(),
            "stale should miss"
        );
    }
}
