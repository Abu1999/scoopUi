use tauri::{AppHandle, State};

use crate::commands::runner::spawn_task;
use crate::commands::{context, roots::resolve_scoop_dir};
use crate::domain::postinstall;
use crate::domain::scoopops;
use crate::error::AppError;
use crate::state::AppState;

#[tauri::command(async)]
pub fn install_package(
    app: AppHandle,
    state: State<'_, AppState>,
    name: String,
    bucket: Option<String>,
    version: Option<String>,
    task_id: String,
) -> Result<(), AppError> {
    let args = scoopops::install_args(&name, bucket.as_deref(), version.as_deref())?;
    spawn_task(&app, &state, args, task_id)
}

#[tauri::command(async)]
pub fn uninstall_package(
    app: AppHandle,
    state: State<'_, AppState>,
    name: String,
    purge: bool,
    task_id: String,
) -> Result<(), AppError> {
    let args = scoopops::uninstall_args(&name, purge)?;
    spawn_task(&app, &state, args, task_id)
}

#[tauri::command(async)]
pub fn update_package(
    app: AppHandle,
    state: State<'_, AppState>,
    name: Option<String>,
    all: bool,
    task_id: String,
) -> Result<(), AppError> {
    let args = scoopops::update_args(name.as_deref(), all)?;
    spawn_task(&app, &state, args, task_id)
}

#[tauri::command(async)]
pub fn set_hold(
    app: AppHandle,
    state: State<'_, AppState>,
    name: String,
    hold: bool,
    task_id: String,
) -> Result<(), AppError> {
    let args = scoopops::hold_args(&name, hold)?;
    spawn_task(&app, &state, args, task_id)
}

#[tauri::command(async)]
pub fn bucket_add(
    app: AppHandle,
    state: State<'_, AppState>,
    name: String,
    repo: Option<String>,
    task_id: String,
) -> Result<(), AppError> {
    let args = scoopops::bucket_add_args(&name, repo.as_deref())?;
    spawn_task(&app, &state, args, task_id)
}

#[tauri::command(async)]
pub fn bucket_remove(
    app: AppHandle,
    state: State<'_, AppState>,
    name: String,
    task_id: String,
) -> Result<(), AppError> {
    let args = scoopops::bucket_remove_args(&name)?;
    spawn_task(&app, &state, args, task_id)
}

#[tauri::command(async)]
pub fn run_maintenance(
    app: AppHandle,
    state: State<'_, AppState>,
    op: String,
    task_id: String,
) -> Result<(), AppError> {
    let args = scoopops::maintenance_args(&op)?;
    spawn_task(&app, &state, args, task_id)
}

#[tauri::command(async)]
pub fn register_app_install(
    state: State<'_, AppState>,
    name: String,
) -> Result<Vec<String>, AppError> {
    let ctx = context();
    let root = resolve_scoop_dir(&ctx, &state).ok_or(AppError::NoRoot)?;
    let dir = root.join("apps").join(&name).join("current");
    postinstall::register(&dir, &name)
}
