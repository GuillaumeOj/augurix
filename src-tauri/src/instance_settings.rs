//! Per-instance overrides persistence — a tiny atomic JSON store mirroring
//! `settings.rs`.
//!
//! Currently holds only each instance's base-branch override (for the
//! "diff vs base branch" view): when set, it wins over the auto-detected default
//! branch. Keyed by the stable `instance_id` (`types::instance_id`). On parse
//! failure the malformed file is preserved as
//! `instance_overrides.json.corrupt-<unix>` and we start from defaults, rather
//! than silently clobbering it.

use anyhow::{anyhow, Context, Result};
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use uuid::Uuid;

const OVERRIDES_FILE: &str = "instance_overrides.json";

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct InstanceOverride {
    /// Base branch to diff against, overriding auto-detection. `None` = auto.
    #[serde(default)]
    pub base_branch: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct InstanceOverrides {
    #[serde(default)]
    pub instances: HashMap<Uuid, InstanceOverride>,
}

pub struct InstanceSettingsStore {
    path: PathBuf,
    overrides: RwLock<InstanceOverrides>,
}

pub type SharedInstanceSettings = Arc<InstanceSettingsStore>;

impl InstanceSettingsStore {
    pub fn load(app_data_dir: &Path) -> Result<Self> {
        std::fs::create_dir_all(app_data_dir)
            .with_context(|| format!("creating app data dir {}", app_data_dir.display()))?;
        let path = app_data_dir.join(OVERRIDES_FILE);

        let overrides = if path.exists() {
            let raw = std::fs::read_to_string(&path)
                .with_context(|| format!("reading {}", path.display()))?;
            match serde_json::from_str::<InstanceOverrides>(&raw) {
                Ok(v) => v,
                Err(e) => {
                    let ts = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .map(|d| d.as_secs())
                        .unwrap_or(0);
                    let backup = path.with_extension(format!("json.corrupt-{ts}"));
                    if let Err(rename_err) = std::fs::rename(&path, &backup) {
                        return Err(anyhow!(
                            "instance_overrides.json is corrupt and could not be backed up: \
                             {e} (rename error: {rename_err})"
                        ));
                    }
                    eprintln!(
                        "instance_overrides.json was corrupt — moved to {} and starting with \
                         defaults. Parse error: {e}",
                        backup.display()
                    );
                    InstanceOverrides::default()
                }
            }
        } else {
            InstanceOverrides::default()
        };

        Ok(Self {
            path,
            overrides: RwLock::new(overrides),
        })
    }

    fn persist_snapshot(&self, snapshot: &InstanceOverrides) -> Result<()> {
        let raw = serde_json::to_string_pretty(snapshot)?;
        let tmp = self.path.with_extension("json.tmp");
        std::fs::write(&tmp, raw).with_context(|| format!("writing {}", tmp.display()))?;
        std::fs::rename(&tmp, &self.path)
            .with_context(|| format!("renaming {} -> {}", tmp.display(), self.path.display()))?;
        Ok(())
    }

    /// The base-branch override for `instance_id`, if any.
    pub fn base_branch(&self, instance_id: Uuid) -> Option<String> {
        self.overrides
            .read()
            .instances
            .get(&instance_id)
            .and_then(|o| o.base_branch.clone())
    }

    /// Set (`Some`) or clear (`None`) the base-branch override for `instance_id`.
    /// Clearing removes the entry entirely so the file stays tidy.
    pub fn set_base_branch(&self, instance_id: Uuid, base_branch: Option<String>) -> Result<()> {
        let candidate = {
            let mut next = self.overrides.read().clone();
            match base_branch.filter(|b| !b.trim().is_empty()) {
                Some(b) => {
                    next.instances.entry(instance_id).or_default().base_branch = Some(b);
                }
                None => {
                    next.instances.remove(&instance_id);
                }
            }
            next
        };
        self.persist_snapshot(&candidate)?;
        *self.overrides.write() = candidate;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn defaults_when_missing() {
        let dir = TempDir::new().unwrap();
        let store = InstanceSettingsStore::load(dir.path()).unwrap();
        assert!(store.base_branch(Uuid::nil()).is_none());
    }

    #[test]
    fn set_and_clear_round_trips_through_disk() {
        let dir = TempDir::new().unwrap();
        let id = Uuid::from_u128(42);
        let store = InstanceSettingsStore::load(dir.path()).unwrap();
        store.set_base_branch(id, Some("develop".into())).unwrap();
        assert_eq!(store.base_branch(id).as_deref(), Some("develop"));

        // Reload from disk: the override persisted.
        let reloaded = InstanceSettingsStore::load(dir.path()).unwrap();
        assert_eq!(reloaded.base_branch(id).as_deref(), Some("develop"));

        // Clearing (None / empty) removes it.
        reloaded.set_base_branch(id, None).unwrap();
        assert!(reloaded.base_branch(id).is_none());
        let again = InstanceSettingsStore::load(dir.path()).unwrap();
        assert!(again.base_branch(id).is_none());
    }

    #[test]
    fn empty_string_clears_override() {
        let dir = TempDir::new().unwrap();
        let id = Uuid::from_u128(7);
        let store = InstanceSettingsStore::load(dir.path()).unwrap();
        store.set_base_branch(id, Some("main".into())).unwrap();
        store.set_base_branch(id, Some("  ".into())).unwrap();
        assert!(store.base_branch(id).is_none());
    }

    #[test]
    fn corrupt_file_is_backed_up_not_overwritten() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("instance_overrides.json");
        std::fs::write(&path, "not json").unwrap();

        let store = InstanceSettingsStore::load(dir.path()).unwrap();
        assert!(store.base_branch(Uuid::nil()).is_none());
        assert!(!path.exists(), "corrupt file should have been moved aside");
        let backups = std::fs::read_dir(dir.path())
            .unwrap()
            .flatten()
            .filter(|e| {
                e.file_name()
                    .to_string_lossy()
                    .starts_with("instance_overrides.json.corrupt-")
            })
            .count();
        assert_eq!(backups, 1);
    }
}
