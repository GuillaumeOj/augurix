use crate::store::SharedStore;
use crate::types::Project;
use tauri::State;
use uuid::Uuid;

#[tauri::command]
pub fn list_projects(store: State<SharedStore>) -> Vec<Project> {
    let mut list = store.list();
    list.sort_by(|a, b| {
        b.pinned
            .cmp(&a.pinned)
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });
    list
}

#[tauri::command]
pub fn add_project(store: State<SharedStore>, path: String) -> Result<Project, String> {
    let path_buf = std::path::PathBuf::from(&path);
    let name = path_buf
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("project")
        .to_string();
    let project = Project {
        id: Uuid::new_v4(),
        name,
        path: path_buf,
        added_at: chrono::Utc::now(),
        pinned: false,
    };
    store.add(project).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn remove_project(store: State<SharedStore>, id: Uuid) -> Result<(), String> {
    store.remove(id).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn rename_project(
    store: State<SharedStore>,
    id: Uuid,
    name: String,
) -> Result<Option<Project>, String> {
    store.rename(id, name).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn set_pinned(store: State<SharedStore>, id: Uuid, pinned: bool) -> Result<(), String> {
    store.set_pinned(id, pinned).map_err(|e| e.to_string())
}
