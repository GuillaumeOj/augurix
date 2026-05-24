use crate::caches::SharedCaches;
use crate::commands::{agents, git, github, instances};
use crate::store::SharedStore;
use crate::types::{Instance, InstanceStatus, InstanceWithStatus, Project, ProjectWithInstances};
use futures::future::join_all;
use std::collections::HashMap;
use std::sync::Arc;
use tauri::State;
use uuid::Uuid;

async fn collect_instance(
    instance: Instance,
    caches: SharedCaches,
    transcript: Option<agents::TranscriptSummary>,
) -> InstanceWithStatus {
    let path = instance.path.clone();
    let snap = caches.snapshot_processes();

    // Single git status, then a sequential gh fetch under spawn_blocking so the
    // branch we display is exactly the one used to key the PR cache.
    let git = tokio::task::spawn_blocking({
        let path = path.clone();
        move || git::collect_status(&path)
    })
    .await
    .unwrap_or_else(|_| crate::types::GitStatus::not_a_repo());

    let pr = if git.is_repo {
        let branch = git.branch.clone();
        let caches = caches.clone();
        let path = path.clone();
        tokio::task::spawn_blocking(move || github::collect_pr(&caches, &path, branch.as_deref()))
            .await
            .ok()
            .flatten()
    } else {
        None
    };

    let agent = agents::build_agent_status(&path, transcript.as_ref(), &snap);

    let status = InstanceStatus {
        instance_id: instance.id,
        agent,
        git,
        pr,
        last_refreshed: chrono::Utc::now(),
    };
    InstanceWithStatus { instance, status }
}

/// Group transcripts → most-recent-per-instance, assigning each transcript to its
/// deepest matching instance based on the latest entry's cwd.
fn assign_transcripts(
    instances: &[Instance],
    transcripts: Vec<agents::TranscriptSummary>,
) -> HashMap<Uuid, agents::TranscriptSummary> {
    // Sort instance indices by path depth (deepest first) so a worktree wins
    // over the project root when both match.
    let mut ordered: Vec<&Instance> = instances.iter().collect();
    ordered.sort_by_key(|i| std::cmp::Reverse(i.path.components().count()));

    let mut best: HashMap<Uuid, agents::TranscriptSummary> = HashMap::new();
    for t in transcripts.into_iter() {
        let Some(target) = ordered
            .iter()
            .find(|i| agents::matches_instance(&t, &i.path))
        else {
            continue;
        };
        match best.get(&target.id) {
            Some(cur) if cur.mtime >= t.mtime => {}
            _ => {
                best.insert(target.id, t);
            }
        }
    }
    best
}

async fn collect_project(project: Project, caches: SharedCaches) -> ProjectWithInstances {
    let instances = instances::discover(&project);
    let transcripts = agents::scan_transcripts(&project.path);
    let mut by_instance = assign_transcripts(&instances, transcripts);

    let futures = instances.into_iter().map(|inst| {
        let transcript = by_instance.remove(&inst.id);
        collect_instance(inst, caches.clone(), transcript)
    });
    let instances = join_all(futures).await;
    ProjectWithInstances { project, instances }
}

#[tauri::command]
pub async fn all_projects_with_instances(
    store: State<'_, SharedStore>,
    caches: State<'_, SharedCaches>,
) -> Result<Vec<ProjectWithInstances>, String> {
    let projects = store.list();
    let caches: SharedCaches = Arc::clone(caches.inner());
    let futures = projects
        .into_iter()
        .map(|p| collect_project(p, caches.clone()));
    let mut results = join_all(futures).await;
    results.sort_by(|a, b| {
        b.project.pinned.cmp(&a.project.pinned).then_with(|| {
            a.project
                .name
                .to_lowercase()
                .cmp(&b.project.name.to_lowercase())
        })
    });
    Ok(results)
}

#[tauri::command]
pub async fn project_with_instances(
    store: State<'_, SharedStore>,
    caches: State<'_, SharedCaches>,
    id: Uuid,
) -> Result<ProjectWithInstances, String> {
    let project = store
        .get(id)
        .ok_or_else(|| "project not found".to_string())?;
    let caches: SharedCaches = Arc::clone(caches.inner());
    Ok(collect_project(project, caches).await)
}

#[tauri::command]
pub async fn instance_messages(
    store: State<'_, SharedStore>,
    project_id: Uuid,
    instance_id: Uuid,
    limit: Option<u32>,
) -> Result<Vec<crate::types::TranscriptMessage>, String> {
    let project = store
        .get(project_id)
        .ok_or_else(|| "project not found".to_string())?;
    let instances = instances::discover(&project);
    let transcripts = agents::scan_transcripts(&project.path);
    let mut by_instance = assign_transcripts(&instances, transcripts);
    let _ = instances
        .iter()
        .find(|i| i.id == instance_id)
        .ok_or_else(|| "instance not found".to_string())?;
    let n = limit.unwrap_or(40) as usize;
    let Some(t) = by_instance.remove(&instance_id) else {
        return Ok(vec![]);
    };
    let path = t.file_path.clone();
    let messages = tokio::task::spawn_blocking(move || agents::read_recent_messages(&path, n))
        .await
        .map_err(|e| e.to_string())?;
    Ok(messages)
}

#[tauri::command]
pub async fn instance_status(
    store: State<'_, SharedStore>,
    caches: State<'_, SharedCaches>,
    project_id: Uuid,
    instance_id: Uuid,
) -> Result<InstanceWithStatus, String> {
    let project = store
        .get(project_id)
        .ok_or_else(|| "project not found".to_string())?;
    let instances = instances::discover(&project);
    let transcripts = agents::scan_transcripts(&project.path);
    let mut by_instance = assign_transcripts(&instances, transcripts);

    let instance = instances
        .into_iter()
        .find(|i| i.id == instance_id)
        .ok_or_else(|| "instance not found".to_string())?;
    let transcript = by_instance.remove(&instance.id);
    let caches: SharedCaches = Arc::clone(caches.inner());
    Ok(collect_instance(instance, caches, transcript).await)
}
