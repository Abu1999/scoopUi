use std::path::PathBuf;

use tauri::{AppHandle, State};

use crate::commands::runner::spawn_command;
use crate::commands::search::load_index;
use crate::commands::{context, roots::resolve_active_root};
use crate::domain::migrate::{self, MigrationEntry};
use crate::domain::registry::{self, RegistryApp};
use crate::domain::scoopcmd::ScoopCommand;
use crate::error::AppError;
use crate::settings;
use crate::state::AppState;

fn known_roots(ctx: &crate::commands::Context, active: Option<&PathBuf>) -> Vec<PathBuf> {
    let mut roots: Vec<PathBuf> = settings::load(&ctx.settings_dir())
        .known_roots
        .into_iter()
        .map(PathBuf::from)
        .collect();
    if let Some(active) = active {
        roots.push(active.clone());
    }
    roots
}

#[tauri::command]
pub async fn scan_installed_registry(state: State<'_, AppState>) -> Result<Vec<RegistryApp>, AppError> {
    let ctx = context();
    let active = resolve_active_root(&ctx, &state);
    let roots = known_roots(&ctx, active.as_ref());
    let apps = tauri::async_runtime::spawn_blocking(move || registry::scan_registry(&roots))
        .await
        .map_err(|e| AppError::Other(format!("注册表扫描失败: {e}")))?;
    Ok(apps)
}

#[tauri::command]
pub async fn match_registry_apps(state: State<'_, AppState>) -> Result<Vec<MigrationEntry>, AppError> {
    let ctx = context();
    let root = resolve_active_root(&ctx, &state).ok_or(AppError::NoRoot)?;
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
        raw_args: vec![uninstall],
        envs: vec![],
        display: format!("uninstall {entry_key}"),
    };
    spawn_command(&app, &state, cmd, task_id)
}
