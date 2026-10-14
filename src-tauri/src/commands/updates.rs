use tauri::State;

use crate::commands::roots::resolve_scoop_dir;
use crate::commands::search::load_index;
use crate::commands::{context, scan_installed};
use crate::domain::updates::{self, UpdateInfo};
use crate::error::AppError;
use crate::state::AppState;

#[tauri::command]
pub async fn list_updates(state: State<'_, AppState>) -> Result<Vec<UpdateInfo>, AppError> {
    let ctx = context();
    let root = resolve_scoop_dir(&ctx, &state).ok_or(AppError::NoRoot)?;
    let entries = load_index(&state, &root).await?;
    let apps = scan_installed(&ctx, &root);
    Ok(updates::compute(&apps, &entries))
}
