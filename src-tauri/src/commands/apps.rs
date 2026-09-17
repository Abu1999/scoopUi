use tauri::State;

use crate::domain::installed::{self, InstalledApp};
use crate::error::AppError;
use crate::state::AppState;

#[tauri::command]
pub fn list_installed_apps(state: State<'_, AppState>) -> Result<Vec<InstalledApp>, AppError> {
    let root = state
        .active_root
        .lock()
        .unwrap()
        .clone()
        .ok_or(AppError::NoRoot)?;
    Ok(installed::scan(&root.join("apps")))
}
