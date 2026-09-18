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
            commands::runner::cancel_task,
            commands::search::search_packages,
            commands::search::refresh_package_index,
            commands::icons::fetch_icon,
            commands::ops::install_package,
            commands::ops::uninstall_package,
            commands::ops::update_package,
            commands::ops::set_hold,
            commands::ops::bucket_add,
            commands::ops::bucket_remove,
            commands::ops::run_maintenance
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
