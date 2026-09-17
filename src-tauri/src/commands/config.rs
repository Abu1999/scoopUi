use serde::Serialize;
use tauri::State;

use crate::commands::{context, roots::resolve_active_root};
use crate::domain::configfile;
use crate::error::AppError;
use crate::state::AppState;

#[derive(Debug, Clone, Serialize)]
pub struct ScoopConfig {
    pub file_path: String,
    pub portable: bool,
    pub values: serde_json::Value,
}

#[tauri::command(async)]
pub fn get_scoop_config(state: State<'_, AppState>) -> Result<ScoopConfig, AppError> {
    let ctx = context();
    let root = resolve_active_root(&ctx, &state).ok_or(AppError::NoRoot)?;
    let resolved = configfile::resolve(&root, &ctx.user_config_home);
    Ok(ScoopConfig {
        file_path: resolved.path.display().to_string(),
        portable: resolved.portable,
        values: configfile::load(&resolved.path),
    })
}
