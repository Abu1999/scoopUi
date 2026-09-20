use tauri::State;

use crate::commands::roots::resolve_active_root;
use crate::commands::search::load_index;
use crate::commands::context;
use crate::domain::installed;
use crate::domain::updates::{self, UpdateInfo};
use crate::error::AppError;
use crate::state::AppState;

#[tauri::command]
pub async fn list_updates(state: State<'_, AppState>) -> Result<Vec<UpdateInfo>, AppError> {
    let ctx = context();
    let root = resolve_active_root(&ctx, &state).ok_or(AppError::NoRoot)?;
    let entries = load_index(&state, &root).await?;
    let apps = installed::scan(&root.join("apps"));
    Ok(updates::compute(&apps, &entries))
}
