use tauri::State;

use crate::commands::context;
use crate::commands::roots::resolve_active_root;
use crate::domain::{configfile, icons};
use crate::error::AppError;
use crate::paths;
use crate::state::AppState;

pub(crate) async fn fetch_icon_inner(
    source: String,
    cache_dir: std::path::PathBuf,
    proxy: String,
) -> Result<Option<String>, AppError> {
    tauri::async_runtime::spawn_blocking(move || {
        icons::resolve_icon(&source, &cache_dir, Some(&proxy))
    })
    .await
    .map_err(|e| AppError::Other(format!("图标抓取任务失败: {e}")))
}

#[tauri::command]
pub async fn fetch_icon(
    state: State<'_, AppState>,
    source: String,
) -> Result<Option<String>, AppError> {
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
    fetch_icon_inner(source, cache_dir, proxy).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fetch_icon_inner_survives_async_context() {
        let tmp = tempfile::tempdir().unwrap();
        let out = tauri::async_runtime::block_on(fetch_icon_inner(
            "http://127.0.0.1:1/x.ico".to_string(),
            tmp.path().to_path_buf(),
            String::new(),
        ));
        assert!(out.is_ok());
        assert!(out.unwrap().is_none());
    }
}
