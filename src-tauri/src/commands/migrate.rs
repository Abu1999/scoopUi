use std::path::PathBuf;

use tauri::{AppHandle, State};

use crate::commands::runner::spawn_command_scoped;
use crate::commands::search::load_index;
use crate::commands::{context, roots::resolve_scoop_dir};
use crate::domain::migrate::{self, MigrationEntry};
use crate::domain::registry::{self, RegistryApp};
use crate::domain::scoopcmd::ScoopCommand;
use crate::error::AppError;
use crate::state::{AppState, TaskScope};

fn known_roots(ctx: &crate::commands::Context, active: Option<&PathBuf>) -> Vec<PathBuf> {
    let mut roots: Vec<PathBuf> = Vec::new();
    if let Some(active) = active {
        roots.push(active.clone());
    }
    roots.push(ctx.global_dir(active.map(|p| p.as_path())));
    roots
}

#[tauri::command]
pub async fn scan_installed_registry(state: State<'_, AppState>) -> Result<Vec<RegistryApp>, AppError> {
    let ctx = context();
    let active = resolve_scoop_dir(&ctx, &state);
    let roots = known_roots(&ctx, active.as_ref());
    let apps = tauri::async_runtime::spawn_blocking(move || registry::scan_registry(&roots))
        .await
        .map_err(|e| AppError::Other(format!("注册表扫描失败: {e}")))?;
    Ok(apps)
}

#[tauri::command]
pub async fn match_registry_apps(state: State<'_, AppState>) -> Result<Vec<MigrationEntry>, AppError> {
    let ctx = context();
    let root = resolve_scoop_dir(&ctx, &state).ok_or(AppError::NoRoot)?;
    let entries = load_index(&state, &root).await?;
    let roots = known_roots(&ctx, Some(&root));
    let apps = tauri::async_runtime::spawn_blocking(move || registry::scan_registry(&roots))
        .await
        .map_err(|e| AppError::Other(format!("注册表扫描失败: {e}")))?;
    Ok(migrate::match_apps(&apps, &entries))
}

#[tauri::command(async)]
pub fn run_registry_uninstall(
    app: AppHandle,
    state: State<'_, AppState>,
    entry_key: String,
    task_id: String,
) -> Result<(), AppError> {
    let uninstall = registry::read_uninstall_string(&entry_key)?;
    let cmd = ScoopCommand {
        program: "cmd.exe".to_string(),
        args: vec!["/C".to_string()],
        raw_args: vec![format!("\"{uninstall}\"")],
        envs: vec![],
        display: format!("uninstall {entry_key}"),
    };
    // 第三方卸载器与 scoop 无关：按条目独立并发（同条目互斥）
    spawn_command_scoped(&app, &state, cmd, task_id, TaskScope::App(entry_key), None)
}
