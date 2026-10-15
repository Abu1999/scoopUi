mod commands;
pub mod domain;
mod error;
mod paths;
mod settings;
mod state;

pub use error::AppError;

use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .manage(state::AppState::default())
        .setup(|app| {
            // 首次以新布局启动时，把旧 %APPDATA%\scoopui 下的配置与配置集迁入数据目录。
            let ctx = commands::context();
            let legacy = paths::settings_dir(&ctx.appdata);
            let _ = paths::migrate_legacy_data(&legacy, &ctx.data_dir);
            // 环境配置固定位于环境目录内（Scoop 便携模式）：缺失时补建最小配置。
            let state = app.state::<state::AppState>();
            if let Some(root) = commands::roots::resolve_scoop_dir(&ctx, &state) {
                let _ = domain::bootstrap::ensure_env_config(&root);
            }
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            commands::roots::get_scoop_env,
            commands::roots::adopt_scoop_dir,
            commands::roots::move_scoop_dir,
            commands::roots::remove_scoop_env,
            commands::roots::env_lock_info,
            commands::roots::force_remove_scoop_env,
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
            commands::ops::force_uninstall_package,
            commands::ops::list_app_processes,
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
            commands::bootstrap::get_scoop_bundle_info,
            commands::bootstrap::check_scoop_prereqs,
            commands::bootstrap::extract_scoop_bundle,
            commands::bootstrap::finalize_scoop_bootstrap,
            commands::bootstrap::install_scoop_online,
            commands::profiles::list_profiles,
            commands::profiles::get_profile,
            commands::profiles::save_profile_from_current,
            commands::profiles::import_profile,
            commands::profiles::export_profile,
            commands::profiles::rename_profile,
            commands::profiles::delete_profile,
            commands::profiles::profile_diff,
            commands::profiles::apply_profile_settings,
            commands::migrate::scan_installed_registry,
            commands::migrate::match_registry_apps,
            commands::migrate::run_registry_uninstall,
            commands::ops::register_app_install,
            commands::app_settings::get_app_settings,
            commands::app_settings::set_theme
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
