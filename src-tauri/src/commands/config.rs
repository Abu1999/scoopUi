use serde::Serialize;
use tauri::State;

use crate::commands::context;
use crate::domain::configfile;
use crate::error::AppError;
use crate::state::AppState;

#[derive(Debug, Clone, Serialize)]
pub struct ScoopConfig {
    pub file_path: String,
    pub portable: bool,
    pub values: serde_json::Value,
}

#[tauri::command]
pub fn get_scoop_config(state: State<'_, AppState>) -> Result<ScoopConfig, AppError> {
    let root = state
        .active_root
        .lock()
        .unwrap()
        .clone()
        .ok_or(AppError::NoRoot)?;
    let ctx = context();
    let resolved = configfile::resolve(&root, &ctx.user_config_home);
    Ok(ScoopConfig {
        file_path: resolved.path.display().to_string(),
        portable: resolved.portable,
        values: configfile::load(&resolved.path),
    })
}
