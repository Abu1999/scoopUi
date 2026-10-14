use tauri::State;

use crate::commands::{context, roots::resolve_scoop_dir, scan_installed};
use crate::domain::installed::InstalledApp;
use crate::error::AppError;
use crate::state::AppState;

#[tauri::command(async)]
pub fn list_installed_apps(state: State<'_, AppState>) -> Result<Vec<InstalledApp>, AppError> {
    let ctx = context();
    let root = resolve_scoop_dir(&ctx, &state).ok_or(AppError::NoRoot)?;
    Ok(scan_installed(&ctx, &root))
}
