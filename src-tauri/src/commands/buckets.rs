use tauri::State;

use crate::commands::{context, roots::resolve_active_root};
use crate::domain::buckets::{self, BucketInfo};
use crate::error::AppError;
use crate::state::AppState;

#[tauri::command(async)]
pub fn list_buckets(state: State<'_, AppState>) -> Result<Vec<BucketInfo>, AppError> {
    let ctx = context();
    let root = resolve_active_root(&ctx, &state).ok_or(AppError::NoRoot)?;
    Ok(buckets::scan(&root.join("buckets")))
}
