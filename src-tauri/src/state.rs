use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use crate::domain::index::PackageEntry;

pub struct IndexCache {
    pub root: PathBuf,
    pub entries: Arc<Vec<PackageEntry>>,
}

#[derive(Default)]
pub struct AppState {
    pub scoop_dir: Mutex<Option<PathBuf>>,
    pub tasks: Mutex<HashMap<String, u32>>,
    pub index: Mutex<Option<IndexCache>>,
}
