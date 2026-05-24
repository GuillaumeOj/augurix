use crate::types::Project;
use anyhow::{anyhow, Context, Result};
use parking_lot::{Mutex, RwLock};
use std::path::{Path, PathBuf};
use std::sync::mpsc::Sender;
use std::sync::Arc;
use uuid::Uuid;

const STORE_FILE: &str = "projects.json";

pub struct ProjectStore {
    path: PathBuf,
    projects: RwLock<Vec<Project>>,
    /// Optional channel used by the watcher to be woken when the project list
    /// changes (add / remove / rename / pin). Send failures are ignored — the
    /// watcher may have exited.
    change_tx: Mutex<Option<Sender<()>>>,
}

pub type SharedStore = Arc<ProjectStore>;

impl ProjectStore {
    /// Load the store from disk.
    ///
    /// On parse failure, the malformed file is preserved as `projects.json.corrupt-<unix>`
    /// and the in-memory list starts empty. Subsequent `persist()` writes a fresh file;
    /// the user can recover from the backup. We deliberately do NOT silently overwrite
    /// the malformed file with `[]`.
    pub fn load(app_data_dir: &Path) -> Result<Self> {
        std::fs::create_dir_all(app_data_dir)
            .with_context(|| format!("creating app data dir {}", app_data_dir.display()))?;
        let path = app_data_dir.join(STORE_FILE);

        let projects = if path.exists() {
            let raw = std::fs::read_to_string(&path)
                .with_context(|| format!("reading {}", path.display()))?;
            match serde_json::from_str::<Vec<Project>>(&raw) {
                Ok(v) => v,
                Err(e) => {
                    let ts = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .map(|d| d.as_secs())
                        .unwrap_or(0);
                    let backup = path.with_extension(format!("json.corrupt-{ts}"));
                    if let Err(rename_err) = std::fs::rename(&path, &backup) {
                        eprintln!(
                            "projects.json parse failed and backup also failed ({rename_err}). \
                             Refusing to start with an empty store to avoid data loss. \
                             Original error: {e}",
                        );
                        return Err(anyhow!(
                            "projects.json is corrupt and could not be backed up: {e}"
                        ));
                    }
                    eprintln!(
                        "projects.json was corrupt — moved to {} and starting with empty list. \
                         Parse error: {e}",
                        backup.display()
                    );
                    Vec::new()
                }
            }
        } else {
            Vec::new()
        };

        Ok(Self {
            path,
            projects: RwLock::new(projects),
            change_tx: Mutex::new(None),
        })
    }

    /// Install a channel used to wake background watchers when the list changes.
    pub fn set_change_notifier(&self, tx: Sender<()>) {
        *self.change_tx.lock() = Some(tx);
    }

    fn notify_changed(&self) {
        if let Some(tx) = self.change_tx.lock().as_ref() {
            // Best-effort wake — if the watcher has exited, just drop.
            let _ = tx.send(());
        }
    }

    /// Persist a candidate snapshot atomically. Returns Ok only if the file is
    /// safely on disk; otherwise the in-memory state is NOT mutated by the
    /// public-facing methods that call this helper.
    fn persist_snapshot(&self, snapshot: &[Project]) -> Result<()> {
        let raw = serde_json::to_string_pretty(snapshot)?;
        let tmp = self.path.with_extension("json.tmp");
        std::fs::write(&tmp, raw).with_context(|| format!("writing {}", tmp.display()))?;
        std::fs::rename(&tmp, &self.path)
            .with_context(|| format!("renaming {} -> {}", tmp.display(), self.path.display()))?;
        Ok(())
    }

    pub fn list(&self) -> Vec<Project> {
        self.projects.read().clone()
    }

    pub fn get(&self, id: Uuid) -> Option<Project> {
        self.projects.read().iter().find(|p| p.id == id).cloned()
    }

    pub fn add(&self, p: Project) -> Result<Project> {
        let canonical = std::fs::canonicalize(&p.path)
            .with_context(|| format!("canonicalize {}", p.path.display()))?;
        let mut p = p;
        p.path = canonical;

        // Build candidate snapshot under a short read lock, persist, then swap.
        let candidate = {
            let guard = self.projects.read();
            if guard.iter().any(|x| x.path == p.path) {
                anyhow::bail!("project already added: {}", p.path.display());
            }
            let mut next = guard.clone();
            next.push(p.clone());
            next
        };
        self.persist_snapshot(&candidate)?;
        *self.projects.write() = candidate;
        self.notify_changed();
        Ok(p)
    }

    pub fn remove(&self, id: Uuid) -> Result<()> {
        let candidate = {
            let guard = self.projects.read();
            guard
                .iter()
                .filter(|p| p.id != id)
                .cloned()
                .collect::<Vec<_>>()
        };
        self.persist_snapshot(&candidate)?;
        *self.projects.write() = candidate;
        self.notify_changed();
        Ok(())
    }

    pub fn rename(&self, id: Uuid, name: String) -> Result<Option<Project>> {
        let (candidate, updated) = {
            let guard = self.projects.read();
            let mut next = guard.clone();
            let updated = next.iter_mut().find(|p| p.id == id).map(|p| {
                p.name = name;
                p.clone()
            });
            (next, updated)
        };
        if updated.is_none() {
            // No-op: nothing to persist, nothing to notify.
            return Ok(None);
        }
        self.persist_snapshot(&candidate)?;
        *self.projects.write() = candidate;
        self.notify_changed();
        Ok(updated)
    }

    pub fn set_pinned(&self, id: Uuid, pinned: bool) -> Result<()> {
        let (candidate, changed) = {
            let guard = self.projects.read();
            let mut next = guard.clone();
            let changed = next
                .iter_mut()
                .find(|p| p.id == id)
                .map(|p| {
                    let prev = p.pinned;
                    p.pinned = pinned;
                    prev != pinned
                })
                .unwrap_or(false);
            (next, changed)
        };
        if !changed {
            return Ok(());
        }
        self.persist_snapshot(&candidate)?;
        *self.projects.write() = candidate;
        self.notify_changed();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;
    use tempfile::TempDir;

    fn new_store() -> (TempDir, ProjectStore) {
        let dir = TempDir::new().unwrap();
        let store = ProjectStore::load(dir.path()).unwrap();
        (dir, store)
    }

    fn mk_project(name: &str, path: PathBuf) -> Project {
        Project {
            id: Uuid::new_v4(),
            name: name.to_string(),
            path,
            added_at: chrono::Utc::now(),
            pinned: false,
        }
    }

    #[test]
    fn load_returns_empty_for_missing_file() {
        let (_d, store) = new_store();
        assert!(store.list().is_empty());
    }

    #[test]
    fn corrupt_json_is_backed_up_not_overwritten() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("projects.json");
        std::fs::write(&path, "not json").unwrap();

        let store = ProjectStore::load(dir.path()).unwrap();
        assert!(store.list().is_empty());

        // Original file moved aside, not overwritten with [].
        assert!(!path.exists(), "corrupt file should have been moved");
        let backups: Vec<_> = std::fs::read_dir(dir.path())
            .unwrap()
            .flatten()
            .filter(|e| {
                e.file_name()
                    .to_string_lossy()
                    .starts_with("projects.json.corrupt-")
            })
            .collect();
        assert_eq!(backups.len(), 1);
        let raw = std::fs::read_to_string(backups[0].path()).unwrap();
        assert_eq!(raw, "not json");
    }

    #[test]
    fn add_then_list() {
        let (dir, store) = new_store();
        let proj = mk_project("p1", dir.path().to_path_buf());
        let added = store.add(proj.clone()).unwrap();
        assert_eq!(added.name, "p1");
        let listed = store.list();
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].id, added.id);
    }

    #[test]
    fn add_rejects_duplicate_path() {
        let (dir, store) = new_store();
        let p1 = mk_project("p1", dir.path().to_path_buf());
        let p2 = mk_project("p2", dir.path().to_path_buf());
        store.add(p1).unwrap();
        let err = store.add(p2).unwrap_err();
        assert!(err.to_string().contains("already added"));
        assert_eq!(store.list().len(), 1);
    }

    #[test]
    fn rename_only_persists_on_real_change() {
        let (dir, store) = new_store();
        let proj = mk_project("p1", dir.path().to_path_buf());
        let id = store.add(proj).unwrap().id;

        // Stale id: no-op, returns Ok(None), nothing changes.
        let stale = Uuid::new_v4();
        let r = store.rename(stale, "x".into()).unwrap();
        assert!(r.is_none());

        // Real rename succeeds.
        let r = store.rename(id, "renamed".into()).unwrap();
        assert_eq!(r.unwrap().name, "renamed");
        assert_eq!(store.list()[0].name, "renamed");
    }

    #[test]
    fn set_pinned_is_idempotent() {
        let (dir, store) = new_store();
        let id = store
            .add(mk_project("p1", dir.path().to_path_buf()))
            .unwrap()
            .id;
        store.set_pinned(id, true).unwrap();
        // Second call with same value is a no-op but still Ok.
        store.set_pinned(id, true).unwrap();
        assert!(store.list()[0].pinned);
    }

    #[test]
    fn change_notifier_fires_on_add_remove() {
        let (dir, store) = new_store();
        let (tx, rx) = mpsc::channel();
        store.set_change_notifier(tx);
        let id = store
            .add(mk_project("p1", dir.path().to_path_buf()))
            .unwrap()
            .id;
        rx.recv_timeout(std::time::Duration::from_millis(100))
            .expect("add should notify");
        store.remove(id).unwrap();
        rx.recv_timeout(std::time::Duration::from_millis(100))
            .expect("remove should notify");
    }

    #[test]
    fn rename_to_missing_id_does_not_notify() {
        let (_d, store) = new_store();
        let (tx, rx) = mpsc::channel();
        store.set_change_notifier(tx);
        store.rename(Uuid::new_v4(), "x".into()).unwrap();
        assert!(rx.try_recv().is_err(), "no-op rename should not notify");
    }
}
