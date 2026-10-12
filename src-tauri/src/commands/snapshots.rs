use std::path::PathBuf;

use tauri::State;

use crate::commands::{context, roots::resolve_active_root, scan_installed};
use crate::domain::buckets;
use crate::domain::configfile;
use crate::domain::snapshots::{self, RestoreBucketsResult, RestorePlan, SnapshotSummary};
use crate::error::AppError;
use crate::paths;
use crate::state::AppState;

fn snapshots_dir(ctx: &crate::commands::Context) -> PathBuf {
    paths::snapshots_dir(&ctx.appdata)
}

#[tauri::command(async)]
pub fn list_snapshots() -> Result<Vec<SnapshotSummary>, AppError> {
    let ctx = context();
    Ok(snapshots::list(&snapshots_dir(&ctx)))
}

#[tauri::command(async)]
pub fn create_snapshot(state: State<'_, AppState>) -> Result<SnapshotSummary, AppError> {
    let ctx = context();
    let root = resolve_active_root(&ctx, &state).ok_or(AppError::NoRoot)?;
    let config_path = configfile::resolve(&root, &ctx.user_config_home).path;
    let bucket_list = buckets::scan(&root.join("buckets"));
    let apps = scan_installed(&ctx, &root);
    snapshots::create(
        &snapshots_dir(&ctx),
        &root,
        &config_path,
        &bucket_list,
        &apps,
        &paths::profiles_dir(&ctx.appdata),
    )
}

#[tauri::command(async)]
pub fn delete_snapshot(id: String) -> Result<(), AppError> {
    let ctx = context();
    snapshots::delete(&snapshots_dir(&ctx), &id)
}

#[tauri::command(async)]
pub fn restore_snapshot_plan(state: State<'_, AppState>, id: String) -> Result<RestorePlan, AppError> {
    let ctx = context();
    let root = resolve_active_root(&ctx, &state).ok_or(AppError::NoRoot)?;
    let config_path = configfile::resolve(&root, &ctx.user_config_home).path;
    let bucket_list = buckets::scan(&root.join("buckets"));
    let apps = scan_installed(&ctx, &root);
    snapshots::plan(&snapshots_dir(&ctx), &id, &root, &config_path, &bucket_list, &apps)
}

#[tauri::command(async)]
pub fn restore_snapshot_config(state: State<'_, AppState>, id: String) -> Result<(), AppError> {
    let ctx = context();
    let root = resolve_active_root(&ctx, &state).ok_or(AppError::NoRoot)?;
    let config_path = configfile::resolve(&root, &ctx.user_config_home).path;
    snapshots::restore_config(&snapshots_dir(&ctx), &id, &config_path)
}

#[tauri::command(async)]
pub fn restore_snapshot_buckets(state: State<'_, AppState>, id: String) -> Result<RestoreBucketsResult, AppError> {
    let ctx = context();
    let root = resolve_active_root(&ctx, &state).ok_or(AppError::NoRoot)?;
    snapshots::restore_buckets(&snapshots_dir(&ctx), &id, &root)
}
