use std::path::PathBuf;

use tauri::State;

use crate::commands::context;
use crate::domain::root::{self, RootInfo};
use crate::error::AppError;
use crate::settings;
use crate::state::AppState;

fn known_roots(ctx: &crate::commands::Context) -> Vec<PathBuf> {
    settings::load(&ctx.settings_dir())
        .known_roots
        .into_iter()
        .map(PathBuf::from)
        .collect()
}

fn active_path(state: &AppState) -> Option<PathBuf> {
    state.active_root.lock().unwrap().clone()
}

fn build_infos(ctx: &crate::commands::Context, state: &AppState) -> Vec<RootInfo> {
    let known = known_roots(ctx);
    let active = active_path(state);
    let global = root::global_dir(ctx.env_global.clone(), ctx.config_global_path(), &ctx.program_data);
    root::candidates(ctx.env_scoop.clone(), ctx.config_root_path(), &known, &ctx.home)
        .iter()
        .map(|c| {
            let is_active = active
                .as_ref()
                .map(|a| a.to_string_lossy().to_lowercase() == c.path.to_string_lossy().to_lowercase())
                .unwrap_or(false);
            root::info(&c.path, c.source, is_active, Some(&global))
        })
        .collect()
}

#[tauri::command]
pub fn detect_roots(state: State<'_, AppState>) -> Result<Vec<RootInfo>, AppError> {
    let ctx = context();
    Ok(build_infos(&ctx, &state))
}

#[tauri::command]
pub fn get_active_root(state: State<'_, AppState>) -> Result<Option<RootInfo>, AppError> {
    let ctx = context();
    let active = active_path(&state).or_else(|| {
        settings::load(&ctx.settings_dir()).active_root.map(PathBuf::from)
    });
    if let Some(path) = active {
        if root::validate(&path).is_ok() {
            *state.active_root.lock().unwrap() = Some(path.clone());
            let global = root::global_dir(ctx.env_global.clone(), ctx.config_global_path(), &ctx.program_data);
            return Ok(Some(root::info(&path, "settings", true, Some(&global))));
        }
    }
    Ok(None)
}

#[tauri::command]
pub fn set_active_root(path: String, state: State<'_, AppState>) -> Result<RootInfo, AppError> {
    let ctx = context();
    let p = PathBuf::from(&path);
    root::validate(&p)?;
    *state.active_root.lock().unwrap() = Some(p.clone());
    let mut s = settings::load(&ctx.settings_dir());
    s.active_root = Some(path.clone());
    if !s.known_roots.iter().any(|k| k.to_lowercase() == path.to_lowercase()) {
        s.known_roots.push(path.clone());
    }
    settings::save(&ctx.settings_dir(), &s)?;
    let global = root::global_dir(ctx.env_global.clone(), ctx.config_global_path(), &ctx.program_data);
    Ok(root::info(&p, "manual", true, Some(&global)))
}

#[tauri::command]
pub fn add_root(path: String, state: State<'_, AppState>) -> Result<RootInfo, AppError> {
    let ctx = context();
    let p = PathBuf::from(&path);
    root::validate(&p)?;
    let mut s = settings::load(&ctx.settings_dir());
    if !s.known_roots.iter().any(|k| k.to_lowercase() == path.to_lowercase()) {
        s.known_roots.push(path.clone());
    }
    settings::save(&ctx.settings_dir(), &s)?;
    let active = active_path(&state);
    let is_active = active
        .as_ref()
        .map(|a| a.to_string_lossy().to_lowercase() == path.to_lowercase())
        .unwrap_or(false);
    let global = root::global_dir(ctx.env_global.clone(), ctx.config_global_path(), &ctx.program_data);
    Ok(root::info(&p, "manual", is_active, Some(&global)))
}
