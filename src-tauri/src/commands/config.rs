use serde::Serialize;
use tauri::State;

use crate::commands::{context, roots::resolve_scoop_dir};
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
    let root = resolve_scoop_dir(&ctx, &state).ok_or(AppError::NoRoot)?;
    let path = configfile::resolve(&root);
    Ok(ScoopConfig {
        file_path: path.display().to_string(),
        // 配置固定位于环境目录内（Scoop 便携模式）
        portable: true,
        values: configfile::load(&path),
    })
}
