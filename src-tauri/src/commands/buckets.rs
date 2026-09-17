use tauri::State;

use crate::domain::buckets::{self, BucketInfo};
use crate::error::AppError;
use crate::state::AppState;

#[tauri::command]
pub fn list_buckets(state: State<'_, AppState>) -> Result<Vec<BucketInfo>, AppError> {
    let root = state
        .active_root
        .lock()
        .unwrap()
        .clone()
        .ok_or(AppError::NoRoot)?;
    Ok(buckets::scan(&root.join("buckets")))
}
