use std::path::Path;
use std::sync::Arc;

use tauri::State;

use crate::commands::context;
use crate::commands::roots::resolve_active_root;
use crate::domain::index::{self, PackageEntry};
use crate::error::AppError;
use crate::state::{AppState, IndexCache};

pub(crate) fn ensure_index(
    state: &AppState,
    root: &Path,
    builder: impl FnOnce(&Path) -> Vec<PackageEntry>,
) -> Arc<Vec<PackageEntry>> {
    {
        let guard = state.index.lock().unwrap();
        if let Some(cache) = guard.as_ref() {
            if cache.root == root {
                return cache.entries.clone();
            }
        }
    }
    let entries = Arc::new(builder(root));
    let mut guard = state.index.lock().unwrap();
    *guard = Some(IndexCache {
        root: root.to_path_buf(),
        entries: entries.clone(),
    });
    entries
}

#[tauri::command(async)]
pub fn search_packages(
    state: State<'_, AppState>,
    query: String,
    limit: Option<usize>,
) -> Result<Vec<PackageEntry>, AppError> {
    let ctx = context();
    let root = resolve_active_root(&ctx, &state).ok_or(AppError::NoRoot)?;
    let entries = ensure_index(&state, &root, |r| index::build(&r.join("buckets")));
    Ok(index::search(&entries, &query, limit.unwrap_or(50)))
}

#[tauri::command(async)]
pub fn refresh_package_index(state: State<'_, AppState>) -> Result<usize, AppError> {
    let ctx = context();
    let root = resolve_active_root(&ctx, &state).ok_or(AppError::NoRoot)?;
    let entries = index::build(&root.join("buckets"));
    let count = entries.len();
    *state.index.lock().unwrap() = Some(IndexCache {
        root: root.clone(),
        entries: Arc::new(entries),
    });
    Ok(count)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::index::PackageEntry;
    use crate::state::{AppState, IndexCache};
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
            deprecated: false,
        }
    }

    #[test]
    fn index_cache_reused_for_same_root() {
        let state = AppState::default();
        *state.index.lock().unwrap() = Some(IndexCache {
            root: PathBuf::from("C:\\scoop"),
            entries: Arc::new(vec![entry("git")]),
        });
        let got = ensure_index(&state, &PathBuf::from("C:\\scoop"), |_| {
            vec![entry("other")]
        });
        assert_eq!(got.len(), 1);
        assert_eq!(got[0].name, "git");
    }

    #[test]
    fn index_rebuilt_when_root_changes() {
        let state = AppState::default();
        *state.index.lock().unwrap() = Some(IndexCache {
            root: PathBuf::from("C:\\a"),
            entries: Arc::new(vec![entry("old")]),
        });
        let got = ensure_index(&state, &PathBuf::from("C:\\b"), |_| vec![entry("new")]);
        assert_eq!(got[0].name, "new");
    }
}
