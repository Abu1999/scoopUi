use tauri::State;

use crate::commands::{context, roots::resolve_active_root};
use crate::domain::buckets::{self, BucketInfo};
use crate::domain::root;
use crate::domain::{scoopcmd, scoopops};
use crate::error::AppError;
use crate::state::AppState;

#[tauri::command(async)]
pub fn list_buckets(state: State<'_, AppState>) -> Result<Vec<BucketInfo>, AppError> {
    let ctx = context();
    let root = resolve_active_root(&ctx, &state).ok_or(AppError::NoRoot)?;
    Ok(buckets::scan(&root.join("buckets")))
}

#[tauri::command]
pub async fn list_known_buckets(state: State<'_, AppState>) -> Result<Vec<String>, AppError> {
    let ctx = context();
    let root_path = resolve_active_root(&ctx, &state).ok_or(AppError::NoRoot)?;
    let global = root::global_dir(ctx.env_global.clone(), ctx.config_global_path(), &ctx.program_data);
    let cmd = scoopcmd::build(
        &root_path,
        Some(&global),
        &["bucket".to_string(), "known".to_string()],
    )?;
    let output = tauri::async_runtime::spawn_blocking(move || scoopcmd::capture(&cmd))
        .await
        .map_err(|e| AppError::Other(format!("已知桶列表获取失败: {e}")))??;
    Ok(scoopops::parse_known_buckets(&output))
}
