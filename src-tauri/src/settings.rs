//! User settings persistence — a tiny atomic JSON store mirroring `store.rs`.
//!
//! Currently holds only the user's terminal choice (for the "Open terminal"
//! action). On parse failure the malformed file is preserved as
//! `settings.json.corrupt-<unix>` and we start from defaults, rather than
//! silently clobbering it.

use anyhow::{anyhow, Context, Result};
use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use std::sync::Arc;

const SETTINGS_FILE: &str = "settings.json";

/// Which terminal the user wants Augurix to hand off to.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum TerminalChoice {
    AppleTerminal,
    Iterm2,
    Kitty,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Settings {
    /// `None` until the user has picked a terminal — the frontend uses this to
    /// know it should redirect the first "Open terminal" click to /settings.
    #[serde(default)]
    pub terminal: Option<TerminalChoice>,
}

pub struct SettingsStore {
    path: PathBuf,
    settings: RwLock<Settings>,
}

pub type SharedSettings = Arc<SettingsStore>;

impl SettingsStore {
    pub fn load(app_data_dir: &Path) -> Result<Self> {
        std::fs::create_dir_all(app_data_dir)
            .with_context(|| format!("creating app data dir {}", app_data_dir.display()))?;
        let path = app_data_dir.join(SETTINGS_FILE);

        let settings = if path.exists() {
            let raw = std::fs::read_to_string(&path)
                .with_context(|| format!("reading {}", path.display()))?;
            match serde_json::from_str::<Settings>(&raw) {
                Ok(v) => v,
                Err(e) => {
                    let ts = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .map(|d| d.as_secs())
                        .unwrap_or(0);
                    let backup = path.with_extension(format!("json.corrupt-{ts}"));
                    if let Err(rename_err) = std::fs::rename(&path, &backup) {
                        return Err(anyhow!(
                            "settings.json is corrupt and could not be backed up: \
                             {e} (rename error: {rename_err})"
                        ));
                    }
                    eprintln!(
                        "settings.json was corrupt — moved to {} and starting with defaults. \
                         Parse error: {e}",
                        backup.display()
                    );
                    Settings::default()
                }
            }
        } else {
            Settings::default()
        };

        Ok(Self {
            path,
            settings: RwLock::new(settings),
        })
    }

    fn persist_snapshot(&self, snapshot: &Settings) -> Result<()> {
        let raw = serde_json::to_string_pretty(snapshot)?;
        let tmp = self.path.with_extension("json.tmp");
        std::fs::write(&tmp, raw).with_context(|| format!("writing {}", tmp.display()))?;
        std::fs::rename(&tmp, &self.path)
            .with_context(|| format!("renaming {} -> {}", tmp.display(), self.path.display()))?;
        Ok(())
    }

    pub fn get(&self) -> Settings {
        self.settings.read().clone()
    }

    pub fn set_terminal(&self, choice: TerminalChoice) -> Result<Settings> {
        let candidate = {
            let mut next = self.settings.read().clone();
            next.terminal = Some(choice);
            next
        };
        self.persist_snapshot(&candidate)?;
        *self.settings.write() = candidate.clone();
        Ok(candidate)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn defaults_when_missing() {
        let dir = TempDir::new().unwrap();
        let store = SettingsStore::load(dir.path()).unwrap();
        assert!(store.get().terminal.is_none());
    }

    #[test]
    fn set_terminal_round_trips_through_disk() {
        let dir = TempDir::new().unwrap();
        let store = SettingsStore::load(dir.path()).unwrap();
        store.set_terminal(TerminalChoice::Kitty).unwrap();
        assert_eq!(store.get().terminal, Some(TerminalChoice::Kitty));

        // Reload from disk: the choice persisted.
        let reloaded = SettingsStore::load(dir.path()).unwrap();
        assert_eq!(reloaded.get().terminal, Some(TerminalChoice::Kitty));
    }

    #[test]
    fn corrupt_file_is_backed_up_not_overwritten() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("settings.json");
        std::fs::write(&path, "not json").unwrap();

        let store = SettingsStore::load(dir.path()).unwrap();
        assert!(store.get().terminal.is_none());
        assert!(!path.exists(), "corrupt file should have been moved aside");
        let backups = std::fs::read_dir(dir.path())
            .unwrap()
            .flatten()
            .filter(|e| {
                e.file_name()
                    .to_string_lossy()
                    .starts_with("settings.json.corrupt-")
            })
            .count();
        assert_eq!(backups, 1);
    }
}
