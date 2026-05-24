mod caches;
mod claude_path;
mod commands;
mod poller;
mod proc;
mod store;
mod types;
mod watchers;

use std::sync::Arc;
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_store::Builder::default().build())
        .setup(|app| {
            let app_data_dir = app.path().app_data_dir().expect("missing app data dir");
            let store = store::ProjectStore::load(&app_data_dir)?;
            let shared: store::SharedStore = Arc::new(store);
            app.manage(shared.clone());

            let caches: caches::SharedCaches = Arc::new(caches::Caches::new());
            app.manage(caches);

            watchers::spawn(app.handle().clone(), shared.clone());
            poller::spawn(app.handle().clone());

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::projects::list_projects,
            commands::projects::add_project,
            commands::projects::remove_project,
            commands::projects::rename_project,
            commands::projects::set_pinned,
            commands::git::git_status,
            commands::git::git_diff,
            commands::github::pr_status,
            commands::agents::agent_status,
            commands::discover::discover_claude_projects,
            commands::status::all_projects_with_instances,
            commands::status::project_with_instances,
            commands::status::instance_status,
            commands::status::instance_messages,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
