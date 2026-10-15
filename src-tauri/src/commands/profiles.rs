use tauri::State;

use crate::commands::{context, roots::resolve_scoop_dir, scan_installed};
use crate::domain::buckets;
use crate::domain::configfile;
use crate::domain::profiles::{self, Profile, ProfileDiff, ProfileSummary};
use crate::error::AppError;
use crate::state::AppState;

#[tauri::command(async)]
pub fn list_profiles() -> Result<Vec<ProfileSummary>, AppError> {
    let ctx = context();
    Ok(profiles::list(&ctx.profiles_dir()))
}

#[tauri::command(async)]
pub fn get_profile(name: String) -> Result<Profile, AppError> {
    let ctx = context();
    profiles::load(&ctx.profiles_dir(), &name)
}

#[tauri::command(async)]
pub fn save_profile_from_current(state: State<'_, AppState>, name: String) -> Result<Profile, AppError> {
    let ctx = context();
    let root = resolve_scoop_dir(&ctx, &state).ok_or(AppError::NoRoot)?;
    let dir = ctx.profiles_dir();
    profiles::validate_profile_name(&name)?;
    if profiles::exists(&dir, &name) {
        return Err(AppError::Other(format!("配置集已存在: {name}")));
    }
    let bucket_list = buckets::scan(&root.join("buckets"));
    let apps = scan_installed(&root);
    let config_path = configfile::resolve(&root);
    let config = configfile::load(&config_path);
    let profile = profiles::snapshot(&name, &bucket_list, &apps, &config);
    profiles::save(&dir, &profile)?;
    Ok(profile)
}

#[tauri::command(async)]
pub fn import_profile(path: String, name: String) -> Result<Profile, AppError> {
    let ctx = context();
    let dir = ctx.profiles_dir();
    profiles::validate_profile_name(&name)?;
    if profiles::exists(&dir, &name) {
        return Err(AppError::Other(format!("配置集已存在: {name}")));
    }
    let text = crate::domain::text::read_text(std::path::Path::new(&path))
        .ok_or_else(|| AppError::Other(format!("读取导入文件失败: {path}")))?;
    let profile = profiles::parse_import(&text, &name)?;
    profiles::save(&dir, &profile)?;
    Ok(profile)
}

#[tauri::command(async)]
pub fn export_profile(name: String, path: String) -> Result<(), AppError> {
    let ctx = context();
    let profile = profiles::load(&ctx.profiles_dir(), &name)?;
    if let Some(parent) = std::path::Path::new(&path).parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&path, serde_json::to_string_pretty(&profile)?)?;
    Ok(())
}

#[tauri::command(async)]
pub fn rename_profile(name: String, new_name: String) -> Result<(), AppError> {
    let ctx = context();
    profiles::rename(&ctx.profiles_dir(), &name, &new_name)
}

#[tauri::command(async)]
pub fn delete_profile(name: String) -> Result<(), AppError> {
    let ctx = context();
    profiles::delete(&ctx.profiles_dir(), &name)
}

#[tauri::command(async)]
pub fn profile_diff(state: State<'_, AppState>, name: String) -> Result<ProfileDiff, AppError> {
    let ctx = context();
    let root = resolve_scoop_dir(&ctx, &state).ok_or(AppError::NoRoot)?;
    let profile = profiles::load(&ctx.profiles_dir(), &name)?;
    let bucket_list = buckets::scan(&root.join("buckets"));
    let apps = scan_installed(&root);
    let config_path = configfile::resolve(&root);
    let config = configfile::load(&config_path);
    Ok(profiles::diff(&profile, &bucket_list, &apps, &config))
}

#[tauri::command(async)]
pub fn apply_profile_settings(
    state: State<'_, AppState>,
    settings: serde_json::Map<String, serde_json::Value>,
) -> Result<(), AppError> {
    let ctx = context();
    let root = resolve_scoop_dir(&ctx, &state).ok_or(AppError::NoRoot)?;
    let config_path = configfile::resolve(&root);
    profiles::apply_settings(&config_path, &settings)
}
