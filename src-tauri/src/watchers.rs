use crate::claude_path::claude_projects_root;
use crate::store::SharedStore;
use notify::RecursiveMode;
use notify_debouncer_full::{new_debouncer, DebouncedEvent};
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::time::Duration;
use tauri::{AppHandle, Emitter};

pub fn spawn(app: AppHandle, store: SharedStore) {
    std::thread::spawn(move || {
        if let Err(e) = run(app, store) {
            eprintln!("watcher exited with error: {e}");
        }
    });
}

fn run(app: AppHandle, store: SharedStore) -> anyhow::Result<()> {
    let (events_tx, events_rx) = mpsc::channel::<Result<Vec<DebouncedEvent>, Vec<notify::Error>>>();
    let mut debouncer = new_debouncer(Duration::from_millis(750), None, events_tx.clone())?;

    // Bridge store-change notifications into the same event stream by sending a
    // synthetic empty `Ok(vec![])` event. This wakes the loop so we can re-sync.
    let (store_change_tx, store_change_rx) = mpsc::channel::<()>();
    store.set_change_notifier(store_change_tx);
    let bridge_tx = events_tx.clone();
    std::thread::spawn(move || {
        while store_change_rx.recv().is_ok() {
            let _ = bridge_tx.send(Ok(vec![]));
        }
    });

    let claude_root = claude_projects_root().filter(|p| p.exists());

    // Track project paths and per-project .git paths separately so we can
    // unwatch them when a project is removed.
    let mut watched_projects: HashSet<PathBuf> = HashSet::new();
    let mut watched_gitdirs: HashSet<PathBuf> = HashSet::new();

    // Initial sync — watch claude_root once and every existing project.
    if let Some(root) = claude_root.as_ref() {
        let _ = debouncer.watch(root, RecursiveMode::Recursive);
    }
    sync_project_watches(
        &mut debouncer,
        &mut watched_projects,
        &mut watched_gitdirs,
        &store,
    );

    loop {
        match events_rx.recv() {
            Ok(Ok(events)) => {
                // Only forward to the UI when there were real FS events. The
                // synthetic store-change wake sends an empty vec.
                if !events.is_empty() {
                    let _ = app.emit("status-changed", ());
                }
            }
            Ok(Err(_errs)) => {}
            Err(_) => break,
        }

        sync_project_watches(
            &mut debouncer,
            &mut watched_projects,
            &mut watched_gitdirs,
            &store,
        );
    }
    Ok(())
}

fn sync_project_watches<F>(
    debouncer: &mut notify_debouncer_full::Debouncer<F, notify_debouncer_full::RecommendedCache>,
    watched_projects: &mut HashSet<PathBuf>,
    watched_gitdirs: &mut HashSet<PathBuf>,
    store: &SharedStore,
) where
    F: notify::Watcher,
{
    let current: HashSet<PathBuf> = store.list().into_iter().map(|p| p.path).collect();

    // Add watches for newly added projects.
    for p in &current {
        if !watched_projects.contains(p) && p.exists() {
            let _ = debouncer.watch(p, RecursiveMode::NonRecursive);
            watched_projects.insert(p.clone());
            let git_dir = p.join(".git");
            if git_dir.exists() {
                let _ = debouncer.watch(&git_dir, RecursiveMode::Recursive);
                watched_gitdirs.insert(git_dir);
            }
        }
    }

    // Drop watches for removed projects.
    let stale_projects: Vec<PathBuf> = watched_projects
        .iter()
        .filter(|p| !current.contains(*p))
        .cloned()
        .collect();
    for p in stale_projects {
        let _ = debouncer.unwatch(&p);
        watched_projects.remove(&p);
        let git_dir = p.join(".git");
        if watched_gitdirs.remove(&git_dir) {
            let _ = debouncer.unwatch(&git_dir);
        }
    }

    let _: &Path = std::path::Path::new("");
}
