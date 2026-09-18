use tauri::State;

use crate::commands::context;
use crate::commands::roots::resolve_active_root;
use crate::domain::{configfile, icons};
use crate::error::AppError;
use crate::paths;
use crate::state::AppState;

#[tauri::command(async)]
pub fn fetch_icon(state: State<'_, AppState>, source: String) -> Result<Option<String>, AppError> {
    let ctx = context();
    let cache_dir = paths::icon_cache_dir(&ctx.local_appdata);
    let proxy = resolve_active_root(&ctx, &state)
        .map(|root| configfile::resolve(&root, &ctx.user_config_home))
        .and_then(|cfg| {
            configfile::load(&cfg.path)
                .get("url_proxy")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string())
        })
        .unwrap_or_else(|| icons::default_proxy().to_string());
    Ok(icons::resolve_icon(&source, &cache_dir, Some(&proxy)))
}
