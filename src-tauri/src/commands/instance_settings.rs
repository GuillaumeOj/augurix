//! Tauri commands for per-instance overrides (currently the base-branch used by
//! the "diff vs base branch" view). Thin wrappers over `InstanceSettingsStore`.

use crate::instance_settings::SharedInstanceSettings;
use tauri::State;
use uuid::Uuid;

#[tauri::command]
pub fn get_instance_base_branch(
    store: State<SharedInstanceSettings>,
    instance_id: Uuid,
) -> Option<String> {
    store.base_branch(instance_id)
}

#[tauri::command]
pub fn set_instance_base_branch(
    store: State<SharedInstanceSettings>,
    instance_id: Uuid,
    base_branch: Option<String>,
) -> Result<(), String> {
    store
        .set_base_branch(instance_id, base_branch)
        .map_err(|e| e.to_string())
}
