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
            commands::buckets::list_known_buckets,
            commands::config::get_scoop_config,
            commands::runner::run_scoop,
            commands::runner::cancel_task,
            commands::search::search_packages,
            commands::search::refresh_package_index,
            commands::updates::list_updates,
            commands::icons::fetch_icon,
            commands::ops::install_package,
            commands::ops::uninstall_package,
            commands::ops::update_package,
            commands::ops::set_hold,
            commands::ops::bucket_add,
            commands::ops::bucket_remove,
            commands::ops::run_maintenance,
            commands::mirrors::get_mirror_info,
            commands::mirrors::set_bucket_source,
            commands::mirrors::set_scoop_source,
            commands::mirrors::set_url_proxy,
            commands::mirrors::set_proxy,
            commands::mirrors::set_aria2,
            commands::mirrors::test_url_proxy,
            commands::profiles::list_profiles,
            commands::profiles::get_profile,
            commands::profiles::save_profile_from_current,
            commands::profiles::import_profile,
            commands::profiles::export_profile,
            commands::profiles::rename_profile,
            commands::profiles::delete_profile,
            commands::profiles::profile_diff,
            commands::profiles::apply_profile_settings
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
