use std::path::Path;
use std::sync::Arc;

use tauri::State;

use crate::commands::context;
use crate::commands::roots::resolve_active_root;
use crate::domain::index::{self, PackageEntry};
use crate::error::AppError;
use crate::state::{AppState, IndexCache};

pub(crate) fn cached_index(state: &AppState, root: &Path) -> Option<Arc<Vec<PackageEntry>>> {
    let guard = state.index.lock().unwrap();
    guard
        .as_ref()
        .filter(|cache| cache.root == root)
        .map(|cache| cache.entries.clone())
}

pub(crate) fn store_index(state: &AppState, root: &Path, entries: Arc<Vec<PackageEntry>>) {
    *state.index.lock().unwrap() = Some(IndexCache {
        root: root.to_path_buf(),
        entries,
    });
}

pub(crate) async fn load_index(
    state: &AppState,
    root: &Path,
) -> Result<Arc<Vec<PackageEntry>>, AppError> {
    if let Some(entries) = cached_index(state, root) {
        return Ok(entries);
    }
    let build_root = root.to_path_buf();
    let built = tauri::async_runtime::spawn_blocking(move || {
        Arc::new(index::build(&build_root.join("buckets")))
    })
    .await
    .map_err(|e| AppError::Other(format!("索引构建失败: {e}")))?;
    store_index(state, root, built.clone());
    Ok(built)
}

#[tauri::command]
pub async fn search_packages(
    state: State<'_, AppState>,
    query: String,
    limit: Option<usize>,
) -> Result<Vec<PackageEntry>, AppError> {
    let ctx = context();
    let root = resolve_active_root(&ctx, &state).ok_or(AppError::NoRoot)?;
    let entries = load_index(&state, &root).await?;
    Ok(index::search(&entries, &query, limit.unwrap_or(50)))
}

#[tauri::command]
pub async fn refresh_package_index(
    state: State<'_, AppState>,
) -> Result<usize, AppError> {
    let ctx = context();
    let root = resolve_active_root(&ctx, &state).ok_or(AppError::NoRoot)?;
    let build_root = root.clone();
    let built = tauri::async_runtime::spawn_blocking(move || {
        Arc::new(index::build(&build_root.join("buckets")))
    })
    .await
    .map_err(|e| AppError::Other(format!("索引构建失败: {e}")))?;
    let count = built.len();
    store_index(&state, &root, built);
    Ok(count)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::index::PackageEntry;
    use crate::state::AppState;
    use std::path::PathBuf;
    use std::sync::Arc;

    fn entry(name: &str) -> PackageEntry {
        PackageEntry {
            name: name.into(),
            bucket: "main".into(),
            version: "1".into(),
            description: None,
            homepage: None,
            license: None,
            icon: None,
            bin: vec![],
            deprecated: false,
        }
    }

    #[test]
    fn index_cache_reused_for_same_root() {
        let state = AppState::default();
        store_index(
            &state,
            &PathBuf::from("C:\\scoop"),
            Arc::new(vec![entry("git")]),
        );
        let got = cached_index(&state, &PathBuf::from("C:\\scoop")).expect("同 root 应命中缓存");
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].name, "git");
    }

    #[test]
    fn index_rebuilt_when_root_changes() {
        let state = AppState::default();
        store_index(&state, &PathBuf::from("C:\\a"), Arc::new(vec![entry("old")]));
        assert!(cached_index(&state, &PathBuf::from("C:\\b")).is_none());
        store_index(&state, &PathBuf::from("C:\\b"), Arc::new(vec![entry("new")]));
        let got = cached_index(&state, &PathBuf::from("C:\\b")).expect("新 root 应命中新缓存");
        assert_eq!(got[0].name, "new");
    }
}
