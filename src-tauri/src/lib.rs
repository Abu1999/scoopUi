mod commands;
pub mod domain;
mod error;
mod paths;
mod settings;
mod state;

pub use error::AppError;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .manage(state::AppState::default())
        .invoke_handler(tauri::generate_handler![
            commands::roots::detect_roots,
            commands::roots::get_active_root,
            commands::roots::set_active_root,
            commands::roots::add_root,
            commands::apps::list_installed_apps,
            commands::buckets::list_buckets,
            commands::config::get_scoop_config,
            commands::runner::run_scoop,
            commands::runner::cancel_task
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
